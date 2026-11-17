#!/usr/bin/env node
// Phase 6L-G: WebSocket checks the 6J soak did not cover. Node 22+, no npm dependencies.
//
//   node ws-6l.mjs narrow <ws-url> <tcp-port> <secret> <first> <count> <duration-s> <out.json>
//       Scope narrowing. Client i alternates every CYCLE_MS between
//         wide    rw=n<i>:*        (odd i also hold QSUB n<i>:q*)
//         narrow  rw=n<i>:a:*
//       by re-presenting SYNC TOKEN on the same socket (same `sub`). A writer
//       over TCP (AUTH from SOAK_PASSWORD) sets n<i>:a:x, n<i>:b:x and n<i>:q1
//       round-robin at WRITE_RATE/s. Checks:
//         - every push names a key inside the grants in force when it arrived
//           (clients run CLIENT PUSHFRAMES ON, so keychanges are pushes, not
//           arrays a client could take for a reply);
//           (frames on one socket are ordered, so grants switch at the reply);
//         - a narrowing that revokes a held QSUB is answered, then closed 4003;
//         - a narrowing that revokes nothing closes nothing, and in-scope
//           pushes keep arriving after it.
//   node ws-6l.mjs replica <ws-url> <secret> <first> <count> <out.json>
//       Clients on the REPLICA's WebSocket port, scoped rw=t<i>:* (the tenants
//       ws-sync.mjs writes probes for), running until SIGTERM. They reconnect
//       after every close and record close codes: a full resync of the live
//       replica must close them with 4001. Pushes are leak-checked, and probe
//       pushes give primary-write to replica-push latency.
import crypto from 'node:crypto';
import fs from 'node:fs';
import net from 'node:net';

const [mode, ...args] = process.argv.slice(2);
const nowNs = () => process.hrtime.bigint();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let serials = 0;

// ── RESP (as 6j/ws-route.mjs) ────────────────────────────────────────────────
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

function mint(secret, grants, sub) {
  const body = Buffer.from(`${grants.map((p) => `rw=${p}`).join(',')}|${Math.floor(Date.now() / 1000) + 3600}|sub=${sub}`)
    .toString('base64url');
  return `${body}.${crypto.createHmac('sha256', secret).update(body).digest('base64url')}`;
}

function pushKeys(frame) {
  const cmd = String(frame[0] ?? '').toUpperCase();
  if (cmd === 'KEYCHANGE' || cmd === 'KEYDELTA') return [frame[1]];
  if (cmd === 'MSET') return frame.slice(1).filter((_, k) => k % 2 === 0);
  if (cmd === 'DEL' || cmd === 'UNLINK') return frame.slice(1);
  if (cmd === 'FLUSHDB') return [];
  return [frame[1]];
}

// One WebSocket client. `onPush(c, frame)`; `onClose(c, code, reason)`.
function connect(url, hooks) {
  return new Promise((resolve) => {
    const ws = new WebSocket(url);
    ws.binaryType = 'arraybuffer';
    const c = { ws, buf: Buffer.alloc(0), pending: [], closing: false, open: false, serial: ++serials, trail: [] };
    // Last events on this socket, for leak samples: what arrived, in order.
    c.note = (e) => { c.trail.push(e); if (c.trail.length > 8) c.trail.shift(); };
    // `onReply` runs synchronously when the reply is parsed, before any frame
    // behind it: Node can deliver several messages before an `await` resumes.
    c.send = (a, onReply) => new Promise((r) => {
      c.pending.push((v) => { if (onReply) onReply(v); r(v); });
      ws.send(encode(a));
    });
    ws.onopen = () => { c.open = true; resolve(c); };
    ws.onerror = () => { if (!c.open) resolve(null); };
    ws.onclose = (e) => {
      c.open = false;
      c.note(`close ${e.code}`);
      for (const w of c.pending.splice(0)) w({ error: 'closed' });
      hooks.onClose?.(c, e.code, e.reason);
    };
    ws.onmessage = (e) => {
      const chunk = typeof e.data === 'string' ? Buffer.from(e.data, 'latin1') : Buffer.from(e.data);
      c.buf = c.buf.length ? Buffer.concat([c.buf, chunk]) : chunk;
      let at = 0;
      for (let v = parse(c.buf, at); v; v = parse(c.buf, at)) {
        at = v[1];
        if (Array.isArray(v[0]) && v[0].isPush) { c.note(`push ${v[0][0]} ${v[0][1]}`); hooks.onPush(c, v[0]); }
        else { c.note(`reply ${JSON.stringify(v[0]).slice(0, 40)}`); const w = c.pending.shift(); if (w) w(v[0]); }
      }
      c.buf = c.buf.subarray(at);
    };
  });
}

// ── narrow ───────────────────────────────────────────────────────────────────
async function narrow(url, port, secret, first, count, duration, out) {
  first = Number(first); count = Number(count); duration = Number(duration);
  const cycleMs = Number(process.env.CYCLE_MS ?? 3000);
  const writeRate = Number(process.env.WRITE_RATE ?? 200);
  const st = { clients: count, pushes: 0, leaks: 0, leakSamples: [], narrowings: 0, widenings: 0,
    revokeCloses: 0, revokeMissing: 0, unexpectedCloses: 0, closeReasons: {}, inScopeAfterNarrow: 0,
    tokenFailures: 0, tokenFailureSamples: [], qsubs: 0, qsubFailures: 0, reopenFailures: 0, writes: 0, writeErrors: 0 };
  const wide = (i) => [`n${i}:*`];
  const tight = (i) => [`n${i}:a:*`];
  const endAt = Date.now() + duration * 1000;

  const hooks = {
    onPush(c, frame) {
      st.pushes++;
      for (const key of pushKeys(frame)) {
        if (typeof key !== 'string') continue;
        const isPattern = String(frame[0]).toLowerCase() === 'keychange' && key.endsWith('*');
        if (!isPattern && !c.grants.some((p) => glob(p, key))) {
          st.leaks++;
          if (st.leakSamples.length < 5) st.leakSamples.push({ client: c.i, socket: c.serial, narrowed: c.narrowed,
            grants: c.grants, frame: frame.slice(0, 2), trail: [...c.trail] });
        } else if (c.narrowed && key.startsWith(`n${c.i}:a:`)) st.inScopeAfterNarrow++;
      }
    },
    onClose(c, code, reason) {
      if (c.closing) return;
      if (c.expect4003 && code === 4003) { c.expect4003 = false; c.got4003?.(); return; }
      st.unexpectedCloses++;
      const why = `${code} ${reason}`.trim();
      st.closeReasons[why] = (st.closeReasons[why] ?? 0) + 1;
    },
  };

  const token = async (c, grants) => {
    const v = await c.send(['SYNC', 'TOKEN', mint(secret, grants, `narrow${c.i}`)], (r) => {
      // The server applies the grants before it writes this reply, so every
      // frame after it is judged by them.
      if (!(r && r.error)) { c.grants = grants; c.narrowed = grants.some((p) => p.endsWith(':a:*')); c.note(`grants ${grants}`); }
    });
    if (v && v.error) {
      st.tokenFailures++;
      if (st.tokenFailureSamples.length < 5) st.tokenFailureSamples.push(v.error);
      return false;
    }
    return true;
  };
  const open = async (i, grants) => {
    const c = await connect(url, hooks);
    if (!c) return null;
    c.i = i; c.grants = []; c.narrowed = false;
    // Without push framing a keychange is a plain array, indistinguishable
    // from a reply by type, and would be taken for the next command's reply.
    if ((await c.send(['CLIENT', 'PUSHFRAMES', 'ON'])) !== 'OK') return null;
    return (await token(c, grants)) ? c : null;
  };
  const qsub = async (c) => {
    const v = await c.send(['QSUB', `n${c.i}:q*`]);
    if (v && v.error) st.qsubFailures++; else { st.qsubs++; c.holds = true; }
  };

  async function cycle(i) {
    let c = await open(i, wide(i));
    if (!c) { st.reopenFailures++; return; }
    if (i % 2) await qsub(c);
    await sleep(Math.random() * cycleMs);
    while (Date.now() + 2 * cycleMs < endAt) {
      await sleep(cycleMs);
      if (!c.open) { c = await open(i, wide(i)); if (!c) { st.reopenFailures++; return; } }
      // Narrow. A held QSUB on n<i>:q* is outside n<i>:a:*, so it is revoked: 4003.
      const held = !!c.holds;
      if (held) c.expect4003 = true;
      const closed = held ? new Promise((r) => { c.got4003 = r; }) : null;
      const ok = await token(c, tight(i));
      if (!ok) { c.expect4003 = false; continue; }
      st.narrowings++;
      if (held) {
        const got = await Promise.race([closed.then(() => true), sleep(2000).then(() => false)]);
        if (got) st.revokeCloses++; else { st.revokeMissing++; c.expect4003 = false; }
        c.closing = true; c.ws.close();
        c = await open(i, tight(i));
        if (!c) { st.reopenFailures++; return; }
      }
      await sleep(cycleMs);
      if (!c.open) { c = await open(i, wide(i)); if (!c) { st.reopenFailures++; return; } }
      // Widen: revokes nothing, so the socket stays open.
      if (await token(c, wide(i))) {
        st.widenings++;
        if (i % 2) await qsub(c);
      }
    }
    c.closing = true;
    c.ws.close();
  }

  async function writer() {
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
    if (process.env.SOAK_PASSWORD && (await cmd('AUTH', process.env.SOAK_PASSWORD)) !== 'OK') {
      console.error('AUTH refused'); process.exit(1);
    }
    let n = 0, next = Date.now();
    while (Date.now() < endAt) {
      const i = first + (n % count);
      const ts = String(nowNs());
      const rs = await Promise.all([cmd('SET', `n${i}:a:x`, ts), cmd('SET', `n${i}:b:x`, ts), cmd('SET', `n${i}:q1`, ts)]);
      for (const r of rs) if (r === 'OK') st.writes++; else st.writeErrors++;
      n++;
      next += 1000 / writeRate;
      const wait = next - Date.now();
      if (wait > 0) await sleep(wait);
    }
    sock.end();
  }

  const all = [];
  for (let k = 0; k < count; k++) { all.push(cycle(first + k)); if (k % 50 === 49) await sleep(100); }
  console.log(`READY ${count}`);
  await Promise.all([...all, writer()]);
  fs.writeFileSync(out, JSON.stringify(st));
  process.exit(0);
}

// ── replica ──────────────────────────────────────────────────────────────────
async function replica(url, secret, first, count, out) {
  first = Number(first); count = Number(count);
  const t0 = Date.now();
  const st = { clients: count, pushes: 0, leaks: 0, leakSamples: [], closes: {}, closeTimeline: {},
    reconnects: 0, reconnectMs: [], tokenFailures: 0, probeLat: [] };
  const hooks = {
    onPush(c, frame) {
      st.pushes++;
      for (const key of pushKeys(frame)) {
        if (typeof key !== 'string') continue;
        const isPattern = String(frame[0]).toLowerCase() === 'keychange' && key.endsWith('*');
        if (!isPattern && !glob(`t${c.i}:*`, key)) {
          st.leaks++;
          if (st.leakSamples.length < 5) st.leakSamples.push({ client: c.i, frame: frame.slice(0, 2) });
        }
      }
      if (frame[0] === 'SET' && frame[1] === `t${c.i}:probe` && typeof frame[2] === 'string') {
        const lat = Number(nowNs() - BigInt(frame[2])) / 1e6;
        if (lat >= 0 && lat < 600000 && st.probeLat.length < 200000) st.probeLat.push(lat);
      }
    },
    onClose(c, code) {
      if (c.closing) return;
      const k = String(code);
      st.closes[k] = (st.closes[k] ?? 0) + 1;
      const sec = String(Math.floor((Date.now() - t0) / 1000));
      if (Object.keys(st.closeTimeline).length < 5000) {
        st.closeTimeline[sec] ??= {};
        st.closeTimeline[sec][k] = (st.closeTimeline[sec][k] ?? 0) + 1;
      }
      reopen(c.i);
    },
  };
  const conns = new Map();
  let stopping = false;
  async function open(i) {
    const c = await connect(url, hooks);
    if (!c) return null;
    c.i = i;
    if ((await c.send(['CLIENT', 'PUSHFRAMES', 'ON'])) !== 'OK') { c.closing = true; c.ws.close(); return null; }
    const v = await c.send(['SYNC', 'TOKEN', mint(secret, [`t${i}:*`], `ruser${i}`)]);
    if (v && v.error) { st.tokenFailures++; c.closing = true; c.ws.close(); return null; }
    conns.set(i, c);
    return c;
  }
  async function reopen(i) {
    const started = Date.now();
    for (let backoff = 250; !stopping; backoff = Math.min(4000, backoff * 2)) {
      await sleep(backoff);
      if (stopping) return;
      if (await open(i)) { st.reconnects++; st.reconnectMs.push(Date.now() - started); return; }
    }
  }
  for (let k = 0; k < count; k++) {
    if (!(await open(first + k))) reopen(first + k);
    if (k % 50 === 49) await sleep(50);
  }
  console.log(`READY ${conns.size}/${count}`);
  const finish = () => {
    stopping = true;
    const res = { ...st, reconnectMs: quantiles(st.reconnectMs), probe: quantiles(st.probeLat),
      runSecs: Math.round((Date.now() - t0) / 1000) };
    delete res.probeLat;
    fs.writeFileSync(out, JSON.stringify(res));
    for (const c of conns.values()) { c.closing = true; try { c.ws.close(); } catch {} }
    process.exit(0);
  };
  process.on('SIGTERM', finish);
  process.on('SIGINT', finish);
  await new Promise(() => {});
}

if (mode === 'narrow') await narrow(...args);
else if (mode === 'replica') await replica(...args);
else { console.error('usage: ws-6l.mjs narrow|replica …'); process.exit(64); }
