#!/usr/bin/env node
// Phase 6I-B Mode 4 / 6I-G: browser-synchronization load against a strict
// (token-scoped) Recached server. Node 22+, no npm dependencies.
//
//   node ws-sync.mjs clients <ws-url> <secret> <first> <count> <duration-s> <out.json>
//       <count> WebSocket clients, client i scoped to its own tenant
//       (`rw=t<i>:*|exp|sub=user<i>`), as one browser user per tenant. Each
//       negotiates RESP3 (pushes arrive as `>` frames), presents its token,
//       and every 4th also WATCHes its tenant's probe key. Records:
//         - propagation latency of probe writes (value = CLOCK_MONOTONIC ns at
//           the writer; same host, so the clocks agree across processes),
//         - any push naming a key outside its tenant (a cross-tenant leak),
//         - reconnects: RECONNECT_PCT % of clients every RECONNECT_MS close
//           and reconnect; time to an accepted token, and whether pushes
//           resume.
//   node ws-sync.mjs writer <tcp-port> <first> <count> <rate> <duration-s> <out.json>
//       Probe writer over TCP: SET t<i>:probe <ns> round-robin over the
//       tenants at <rate>/s total, plus DEDUP retries (each id sent twice; the
//       second must answer DUP and must not apply).
//
// Each prints a JSON summary to <out.json> when the duration ends.
import crypto from 'node:crypto';
import fs from 'node:fs';
import net from 'node:net';

const [mode, ...args] = process.argv.slice(2);
const nowNs = () => process.hrtime.bigint();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ── RESP ─────────────────────────────────────────────────────────────────────
// Parses one value from `buf` (a Buffer) at `at`; returns [value, next] or null.
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

// ── clients ──────────────────────────────────────────────────────────────────
async function clients(url, secret, first, count, duration, out) {
  first = Number(first); count = Number(count); duration = Number(duration);
  const reconnectPct = Number(process.env.RECONNECT_PCT ?? 5);
  const reconnectMs = Number(process.env.RECONNECT_MS ?? 5000);
  const mint = (i) => {
    const body = Buffer.from(`rw=t${i}:*|${Math.floor(Date.now() / 1000) + 3600}|sub=user${i}`).toString('base64url');
    return `${body}.${crypto.createHmac('sha256', secret).update(body).digest('base64url')}`;
  };
  const stats = { pushes: 0, probeLat: [], leaks: 0, leakSamples: [], reconnects: 0, reconnectMs: [],
    reconnectFailures: 0, closesUnexpected: 0, closeReasons: {}, errors: 0, connectFailures: 0, resumedAfterReconnect: 0 };

  function open(i) {
    return new Promise((resolve) => {
      const started = nowNs();
      const ws = new WebSocket(url);
      ws.binaryType = 'arraybuffer';
      const c = { i, ws, buf: Buffer.alloc(0), pending: [], ready: false, closing: false, sawPushSinceOpen: false };
      ws.onopen = () => {
        // RESP3 so pushes are distinguishable from replies; then the token.
        c.pending.push(() => {});
        ws.send(encode(['HELLO', '3']));
        c.pending.push((v) => {
          if (v && v.error) { stats.errors++; resolve(null); ws.close(); return; }
          c.ready = true;
          resolve({ c, ms: Number(nowNs() - started) / 1e6 });
        });
        ws.send(encode(['SYNC', 'TOKEN', mint(i)]));
        if (i % 4 === 0) { c.pending.push(() => {}); ws.send(encode(['WATCH', `t${i}:probe`])); }
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
          // A `>` frame, or an array nobody is waiting for (a keychange
          // notification on a RESP2 framing), is a push.
          if (Array.isArray(val) && (val.isPush || !c.pending.length)) onPush(c, val);
          else { const w = c.pending.shift(); if (w) w(val); }
        }
        c.buf = c.buf.subarray(at);
      };
    });
  }

  function onPush(c, frame) {
    stats.pushes++;
    c.sawPushSinceOpen = true;
    // A mutation push is the write's own frame: name, then key(s). Keychange
    // notifications (WATCH) carry the key second as well. Anything naming a
    // key outside t<i>: is a leak.
    const keys = frame.slice(1).filter((x) => typeof x === 'string' && /^[a-z]+\d*:/.test(x));
    const key = frame[1];
    if (typeof key === 'string' && /^t\d+:/.test(key) && !key.startsWith(`t${c.i}:`)) {
      stats.leaks++;
      if (stats.leakSamples.length < 5) stats.leakSamples.push({ client: c.i, frame: frame.slice(0, 3) });
    }
    void keys;
    if (frame[0] === 'SET' && key === `t${c.i}:probe` && typeof frame[2] === 'string') {
      const lat = Number(nowNs() - BigInt(frame[2])) / 1e6;
      if (lat >= 0 && lat < 600000) stats.probeLat.push(lat);
    }
  }

  const conns = new Array(count);
  const connectMs = [];
  // Connect in waves so the server's accept queue is not the measurement.
  for (let k = 0; k < count; k += 50) {
    const wave = [];
    for (let j = k; j < Math.min(count, k + 50); j++) wave.push(open(first + j).then((r) => { if (r) { conns[j] = r.c; connectMs.push(r.ms); } }));
    await Promise.all(wave);
  }
  const connected = conns.filter(Boolean).length;
  console.log(`READY ${connected}/${count}`);

  const endAt = Date.now() + duration * 1000;
  const reconnector = (async () => {
    while (Date.now() + reconnectMs < endAt) {
      await sleep(reconnectMs);
      const n = Math.max(1, Math.floor((count * reconnectPct) / 100));
      for (let r = 0; r < n; r++) {
        const j = Math.floor(Math.random() * count);
        const old = conns[j];
        if (!old) continue;
        old.closing = true;
        old.ws.close();
        const res = await open(first + j);
        if (res) {
          conns[j] = res.c; stats.reconnects++; stats.reconnectMs.push(res.ms);
          setTimeout(() => { if (res.c.sawPushSinceOpen) stats.resumedAfterReconnect++; }, Math.min(3000, endAt - Date.now()));
        } else { conns[j] = null; stats.reconnectFailures++; }
      }
    }
  })();
  await sleep(Math.max(0, endAt - Date.now()));
  await reconnector;
  const mem = process.memoryUsage();
  fs.writeFileSync(out, JSON.stringify({
    first, count, connected, connect: quantiles(connectMs), pushes: stats.pushes,
    probe: quantiles(stats.probeLat), leaks: stats.leaks, leakSamples: stats.leakSamples,
    reconnects: stats.reconnects, reconnectFailures: stats.reconnectFailures,
    reconnect: quantiles(stats.reconnectMs), resumedAfterReconnect: stats.resumedAfterReconnect,
    closesUnexpected: stats.closesUnexpected, closeReasons: stats.closeReasons,
    connectFailures: stats.connectFailures, errors: stats.errors,
    nodeRssMb: Math.round(mem.rss / 1048576),
  }));
  for (const c of conns) if (c) { c.closing = true; c.ws.close(); }
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
  const client = `bench-${process.pid}`;
  while (Date.now() < endAt) {
    const t = first + (n % count);
    const r = await cmd('SET', `t${t}:probe`, String(nowNs()));
    if (r !== 'OK') st.errors++; else st.probes++;
    if (n % 20 === 0) {
      // DEDUP retry: the same id twice. The first applies, the retry must be
      // answered DUP and leave the counter alone.
      const id = String(n + 1);
      const a = await cmd('DEDUP', client, id, 'INCR', `t${t}:dedup`);
      const b = await cmd('DEDUP', client, id, 'INCR', `t${t}:dedup`);
      if (a && !a.error) st.dedupFirst++; else st.dedupWrong++;
      if (b === 'DUP') st.dedupDup++; else st.dedupWrong++;
    }
    n++;
    next += interval;
    const wait = next - Date.now();
    if (wait > 0) await sleep(wait);
  }
  // Every first DEDUP applied once: the counters sum to the number of firsts.
  let total = 0;
  for (let i = 0; i < count; i++) { const v = await cmd('GET', `t${first + i}:dedup`); total += Number(v || 0); }
  st.dedupApplied = total;
  fs.writeFileSync(out, JSON.stringify(st));
  sock.end();
  process.exit(0);
}

if (mode === 'clients') await clients(...args);
else if (mode === 'writer') await writer(...args);
else { console.error('usage: ws-sync.mjs clients|writer …'); process.exit(64); }
