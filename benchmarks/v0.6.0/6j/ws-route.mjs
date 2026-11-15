#!/usr/bin/env node
// Phase 6J-A: WebSocket sync fan-out under load, by routing scenario.
// Node 22+, no npm dependencies. Runs inside recached-6i-bench:local via ws-route.sh.
//
//   node ws-route.mjs clients <ws-url> <secret> <first> <count> <total> <duration-s> <out.json>
//       <count> WebSocket clients numbered <first>.., out of <total> in the
//       run. Each negotiates RESP3, presents a token whose grants depend on
//       SCENARIO (below), and checks every push it receives against its own
//       grants: a push naming a key no grant matches is a leak. Probe writes
//       (`SET p<i>:probe <CLOCK_MONOTONIC ns>`) give propagation latency.
//   node ws-route.mjs writer <tcp-port> <first> <count> <rate> <duration-s> <out.json>
//       Probe writer: SET p<i>:probe round-robin at <rate>/s, plus DEDUP
//       retries (each id twice; the second must answer DUP and not apply).
//
// SCENARIO — grants of client i, and what memtier writes (see ws-route.sh):
//   A  rw=bg:*                every background write matches every client
//   B  client 0 rw=bg:*, others rw=t<i>:*      every write matches one client
//   C  rw=g<i%10>:*           writes to g0:* match one tenant group (1/10)
//   D  rw=t<i>:*              writes to bg:* match nobody (the 6I Mode 4 case)
//   E  i%3: rw=e:<i> exact | rw=t<i>:* | r=e:<i>        writes e:<0..10n>
//   F  rw=*:u<i>              suffix wildcards: no literal prefix to route on
//   G  as D, and every client re-presents a fresh token every RENEW_MS
//   H  as D, and every client QSUBs / QUNSUBs t<i>:q* every CHURN_MS
//   I  as D, with RECONNECT_PCT % of clients reconnecting every RECONNECT_MS
//   J  as D, plus SLOW_PCT % of clients granted rw=bg:* that stop reading
// Every client is also granted rw=p<i>:* for its probe key.
import crypto from 'node:crypto';
import fs from 'node:fs';
import net from 'node:net';

const [mode, ...args] = process.argv.slice(2);
const SCENARIO = process.env.SCENARIO ?? 'D';
const nowNs = () => process.hrtime.bigint();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ── RESP ─────────────────────────────────────────────────────────────────────
function parse(buf, at = 0) {
  const nl = buf.indexOf('\r\n', at);
  if (nl < 0) return null;
  const t = String.fromCharCode(buf[at]);
  const head = buf.toString('latin1', at + 1, nl);
  const rest = nl + 2;
  switch (t) {
    case '+': case ':': case ',': case '#': case '_': return [head, rest];
    case '-': case '!': return [{ error: head }, rest];
    case '$': case '=': {
      const n = Number(head);
      if (n < 0) return [null, rest];
      if (buf.length < rest + n + 2) return null;
      return [buf.toString('latin1', rest, rest + n), rest + n + 2];
    }
    case '*': case '~': case '>': case '%': {
      let n = Number(head);
      if (n < 0) return [null, rest];
      if (t === '%') n *= 2;
      const items = [];
      let r = rest;
      for (let i = 0; i < n; i++) {
        const v = parse(buf, r);
        if (!v) return null;
        items.push(v[0]);
        r = v[1];
      }
      if (t === '>') Object.defineProperty(items, 'isPush', { value: true });
      return [items, r];
    }
    default: return [{ error: `unparsed ${t}` }, buf.length];
  }
}
const encode = (a) => `*${a.length}\r\n` + a.map((x) => `$${Buffer.byteLength(String(x))}\r\n${x}\r\n`).join('');

function quantiles(xs) {
  if (!xs.length) return null;
  xs.sort((a, b) => a - b);
  const q = (f) => xs[Math.min(xs.length - 1, Math.floor(f * xs.length))];
  return { n: xs.length, p50: q(0.5), p95: q(0.95), p99: q(0.99), max: xs[xs.length - 1] };
}

// The server's glob: `*` any run, `?` any one byte, everything else literal.
function glob(p, s) {
  let i = 0, j = 0, star = -1, resume = 0;
  while (i < s.length) {
    if (j < p.length && (p[j] === '?' || p[j] === s[i])) { i++; j++; }
    else if (j < p.length && p[j] === '*') { star = j++; resume = i; }
    else if (star >= 0) { j = star + 1; i = ++resume; }
    else return false;
  }
  while (j < p.length && p[j] === '*') j++;
  return j === p.length;
}

// Grants for client i, as [access, pattern] pairs.
function grantsFor(i, slow) {
  const g = [['rw', `p${i}:*`]];
  if (slow) { g.push(['rw', 'bg:*']); return g; }
  switch (SCENARIO) {
    case 'A': g.push(['rw', 'bg:*']); break;
    case 'B': g.push(['rw', i === 0 ? 'bg:*' : `t${i}:*`]); break;
    case 'C': g.push(['rw', `g${i % 10}:*`]); break;
    case 'E': g.push(i % 3 === 0 ? ['rw', `e:${i}`] : i % 3 === 1 ? ['rw', `t${i}:*`] : ['r', `e:${i}`]); break;
    case 'F': g.push(['rw', `*:u${i}`]); break;
    default: g.push(['rw', `t${i}:*`]);
  }
  return g;
}

function mint(secret, i, slow) {
  const entries = grantsFor(i, slow).map(([a, p]) => `${a}=${p}`).join(',');
  const body = Buffer.from(`${entries}|${Math.floor(Date.now() / 1000) + 3600}|sub=user${i}`).toString('base64url');
  return `${body}.${crypto.createHmac('sha256', secret).update(body).digest('base64url')}`;
}

// Keys a push frame names: the write's own frame is [cmd, key, ...] except the
// multi-key forms; keychange notifications are ["keychange", key, ...].
function pushKeys(frame) {
  const cmd = String(frame[0] ?? '').toUpperCase();
  if (cmd === 'KEYCHANGE' || cmd === 'KEYDELTA') return [frame[1]];
  if (cmd === 'MSET') return frame.slice(1).filter((_, k) => k % 2 === 0);
  if (cmd === 'DEL' || cmd === 'UNLINK') return frame.slice(1);
  if (cmd === 'FLUSHDB') return [];
  return [frame[1]];
}

// ── slow client: a raw socket that completes the handshake, then stops reading
function openSlow(url, secret, i) {
  return new Promise((resolve) => {
    const u = new URL(url);
    const sock = net.connect(Number(u.port), u.hostname);
    const key = crypto.randomBytes(16).toString('base64');
    const frame = (text) => {
      const payload = Buffer.from(text);
      const mask = crypto.randomBytes(4);
      const len = payload.length;
      const head = len < 126 ? Buffer.from([0x81, 0x80 | len])
        : Buffer.from([0x81, 0x80 | 126, len >> 8, len & 0xff]);
      const body = Buffer.alloc(len);
      for (let k = 0; k < len; k++) body[k] = payload[k] ^ mask[k & 3];
      return Buffer.concat([head, mask, body]);
    };
    let got = Buffer.alloc(0);
    sock.once('connect', () => sock.write(`GET / HTTP/1.1\r\nHost: ${u.host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: ${key}\r\nSec-WebSocket-Version: 13\r\n\r\n`));
    const onData = (d) => {
      got = Buffer.concat([got, d]);
      if (got.indexOf('\r\n\r\n') < 0) return;
      sock.off('data', onData);
      sock.write(frame(encode(['HELLO', '3'])));
      sock.write(frame(encode(['SYNC', 'TOKEN', mint(secret, i, true)])));
      // Give the replies a moment to arrive, then stop reading for good: the
      // kernel buffers fill and the server's send to this socket blocks.
      setTimeout(() => { sock.pause(); resolve(sock); }, 200);
    };
    sock.on('data', onData);
    sock.on('error', () => resolve(null));
  });
}

// ── clients ──────────────────────────────────────────────────────────────────
async function clients(url, secret, first, count, total, duration, out) {
  first = Number(first); count = Number(count); total = Number(total); duration = Number(duration);
  const reconnectPct = Number(process.env.RECONNECT_PCT ?? (SCENARIO === 'I' ? 20 : 0));
  const reconnectMs = Number(process.env.RECONNECT_MS ?? 1000);
  const renewMs = Number(process.env.RENEW_MS ?? 1000);
  const churnMs = Number(process.env.CHURN_MS ?? 1000);
  const slowPct = Number(process.env.SLOW_PCT ?? (SCENARIO === 'J' ? 10 : 0));
  const stats = { pushes: 0, probeLat: [], leaks: 0, leakSamples: [], reconnects: 0, reconnectMs: [],
    reconnectFailures: 0, closesUnexpected: 0, closeReasons: {}, errors: 0, errorSamples: [], connectFailures: 0,
    renewals: 0, renewFailures: 0, churns: 0, churnFailures: 0, slowOpened: 0, slowClosedByServer: 0 };
  const noteError = (what, v) => { stats.errors++; if (stats.errorSamples.length < 5) stats.errorSamples.push({ what, v }); };

  function open(i) {
    return new Promise((resolve) => {
      const started = nowNs();
      const ws = new WebSocket(url);
      ws.binaryType = 'arraybuffer';
      const grants = grantsFor(i, false).map(([, p]) => p);
      const c = { i, ws, grants, buf: Buffer.alloc(0), pending: [], closing: false,
        send: (a, cb = () => {}) => { c.pending.push(cb); ws.send(encode(a)); } };
      ws.onopen = () => {
        c.send(['HELLO', '3']);
        c.send(['SYNC', 'TOKEN', mint(secret, i, false)], (v) => {
          if (v && v.error) { noteError('token', v); resolve(null); ws.close(); return; }
          resolve({ c, ms: Number(nowNs() - started) / 1e6 });
        });
      };
      ws.onerror = () => { stats.connectFailures++; resolve(null); };
      ws.onclose = (e) => {
        if (!c.closing) {
          stats.closesUnexpected++;
          const why = `${e.code} ${e.reason}`.trim();
          stats.closeReasons[why] = (stats.closeReasons[why] ?? 0) + 1;
        }
      };
      ws.onmessage = (e) => {
        const chunk = typeof e.data === 'string' ? Buffer.from(e.data, 'latin1') : Buffer.from(e.data);
        c.buf = c.buf.length ? Buffer.concat([c.buf, chunk]) : chunk;
        let at = 0;
        for (let v = parse(c.buf, at); v; v = parse(c.buf, at)) {
          at = v[1];
          const val = v[0];
          if (Array.isArray(val) && val.isPush) onPush(c, val);
          else { const w = c.pending.shift(); if (w) w(val); }
        }
        c.buf = c.buf.subarray(at);
      };
    });
  }

  function onPush(c, frame) {
    stats.pushes++;
    for (const key of pushKeys(frame)) {
      if (typeof key !== 'string') continue;
      // A FLUSHDB sentinel keychange names the QSUB pattern itself.
      if (!c.grants.some((p) => glob(p, key)) && !(String(frame[0]).toLowerCase() === 'keychange' && key.endsWith('*'))) {
        stats.leaks++;
        if (stats.leakSamples.length < 5) stats.leakSamples.push({ client: c.i, frame: frame.slice(0, 3) });
      }
    }
    if (frame[0] === 'SET' && frame[1] === `p${c.i}:probe` && typeof frame[2] === 'string') {
      const lat = Number(nowNs() - BigInt(frame[2])) / 1e6;
      if (lat >= 0 && lat < 600000) stats.probeLat.push(lat);
    }
  }

  const conns = new Array(count);
  const slow = [];
  const connectMs = [];
  const nSlow = Math.floor((count * slowPct) / 100);
  for (let k = 0; k < count; k += 50) {
    const wave = [];
    for (let j = k; j < Math.min(count, k + 50); j++) {
      if (j < nSlow) wave.push(openSlow(url, secret, first + j).then((s) => {
        if (!s) return; stats.slowOpened++; slow.push(s);
        s.on('close', () => stats.slowClosedByServer++);
      }));
      else wave.push(open(first + j).then((r) => { if (r) { conns[j] = r.c; connectMs.push(r.ms); } }));
    }
    await Promise.all(wave);
  }
  const connected = conns.filter(Boolean).length + stats.slowOpened;
  console.log(`READY ${connected}/${count}`);

  const endAt = Date.now() + duration * 1000;
  const loops = [];
  if (reconnectPct > 0) loops.push((async () => {
    while (Date.now() + reconnectMs < endAt) {
      await sleep(reconnectMs);
      const n = Math.max(1, Math.floor((count * reconnectPct) / 100));
      const batch = [];
      for (let r = 0; r < n; r++) {
        const j = nSlow + Math.floor(Math.random() * (count - nSlow));
        const old = conns[j];
        if (!old) continue;
        old.closing = true;
        old.ws.close();
        conns[j] = null;
        batch.push(open(first + j).then((res) => {
          if (res) { conns[j] = res.c; stats.reconnects++; stats.reconnectMs.push(res.ms); }
          else stats.reconnectFailures++;
        }));
      }
      await Promise.all(batch);
    }
  })());
  if (SCENARIO === 'G') loops.push((async () => {
    while (Date.now() + renewMs < endAt) {
      await sleep(renewMs);
      for (const c of conns) if (c && !c.closing) {
        c.send(['SYNC', 'TOKEN', mint(secret, c.i, false)], (v) => { if (v && v.error) stats.renewFailures++; else stats.renewals++; });
      }
    }
  })());
  if (SCENARIO === 'H') loops.push((async () => {
    let on = false;
    while (Date.now() + churnMs < endAt) {
      await sleep(churnMs);
      on = !on;
      for (const c of conns) if (c && !c.closing) {
        c.send([on ? 'QSUB' : 'QUNSUB', `t${c.i}:q*`], (v) => { if (v && v.error) { stats.churnFailures++; noteError('churn', v); } else stats.churns++; });
      }
    }
  })());
  await sleep(Math.max(0, endAt - Date.now()));
  await Promise.all(loops);
  const mem = process.memoryUsage();
  fs.writeFileSync(out, JSON.stringify({
    scenario: SCENARIO, first, count, total, connected, connect: quantiles(connectMs), pushes: stats.pushes,
    probe: quantiles(stats.probeLat), leaks: stats.leaks, leakSamples: stats.leakSamples,
    reconnects: stats.reconnects, reconnectFailures: stats.reconnectFailures,
    reconnect: quantiles(stats.reconnectMs), renewals: stats.renewals, renewFailures: stats.renewFailures,
    churns: stats.churns, churnFailures: stats.churnFailures,
    slowOpened: stats.slowOpened, slowClosedByServer: stats.slowClosedByServer,
    closesUnexpected: stats.closesUnexpected, closeReasons: stats.closeReasons,
    connectFailures: stats.connectFailures, errors: stats.errors, errorSamples: stats.errorSamples,
    nodeRssMb: Math.round(mem.rss / 1048576),
  }));
  for (const c of conns) if (c) { c.closing = true; c.ws.close(); }
  for (const s of slow) s.destroy();
  process.exit(0);
}

// ── probe writer ─────────────────────────────────────────────────────────────
async function writer(port, first, count, rate, duration, out) {
  first = Number(first); count = Number(count); rate = Number(rate); duration = Number(duration);
  const sock = net.connect(Number(port), '127.0.0.1');
  sock.setNoDelay(true);
  await new Promise((r) => sock.once('connect', r));
  let buf = Buffer.alloc(0);
  const waiters = [];
  sock.on('data', (d) => {
    buf = buf.length ? Buffer.concat([buf, d]) : d;
    let at = 0;
    for (let v = parse(buf, at); v; v = parse(buf, at)) { at = v[1]; const w = waiters.shift(); if (w) w(v[0]); }
    buf = buf.subarray(at);
  });
  const cmd = (...a) => new Promise((r) => { waiters.push(r); sock.write(encode(a)); });
  const st = { probes: 0, errors: 0, dedupFirst: 0, dedupDup: 0, dedupWrong: 0, dedupApplied: 0 };
  const endAt = Date.now() + duration * 1000;
  const interval = 1000 / rate;
  let next = Date.now();
  let n = 0;
  const client = `bench-${process.pid}-${crypto.randomBytes(4).toString('hex')}`;
  while (Date.now() < endAt) {
    const t = first + (n % count);
    const r = await cmd('SET', `p${t}:probe`, String(nowNs()));
    if (r !== 'OK') st.errors++; else st.probes++;
    if (n % 20 === 0) {
      const id = String(n + 1);
      const a = await cmd('DEDUP', client, id, 'INCR', `p${t}:dedup`);
      const b = await cmd('DEDUP', client, id, 'INCR', `p${t}:dedup`);
      if (a && !a.error) st.dedupFirst++; else st.dedupWrong++;
      if (b === 'DUP') st.dedupDup++; else st.dedupWrong++;
    }
    n++;
    next += interval;
    const wait = next - Date.now();
    if (wait > 0) await sleep(wait);
  }
  let total = 0;
  for (let i = 0; i < count; i++) { const v = await cmd('GET', `p${first + i}:dedup`); total += Number(v || 0); }
  st.dedupApplied = total;
  fs.writeFileSync(out, JSON.stringify(st));
  sock.end();
  process.exit(0);
}

if (mode === 'clients') await clients(...args);
else if (mode === 'writer') await writer(...args);
else { console.error('usage: ws-route.mjs clients|writer …'); process.exit(64); }
