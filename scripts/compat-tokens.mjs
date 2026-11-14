#!/usr/bin/env node
// Sync-token compatibility checks for scripts/compat-matrix.sh.
//
//   node scripts/compat-tokens.mjs <ws-url> <secret> <old|new>
//
// Runs against a server started with RECACHED_SYNC_SECRET=<secret> (strict
// sync, no RECACHED_PASSWORD) and prints one `ok`/`FAIL` line per check; the
// exit status is the number of failures. `old` expects a 0.4/0.5 server, `new`
// a 0.6 one. Needs Node 22+ (global WebSocket); no npm dependencies.
//
// What each version must do with each token form:
//
//   two-segment `entries|exp`          both accept — 0.5 tokens keep working
//   claims `entries|exp|sub=..`        new accepts; old refuses (fail closed,
//                                      never "accepted without the claims")
//   expired / bad signature            both refuse
//   cross-tenant key                   both refuse with NOSCOPE
//   renewal (same scope, later exp)    new keeps the socket
//   subject change                     new replies, then closes with 4003
//   expiry on a live socket            new closes with 4002
import crypto from 'node:crypto';

const [url, secret, side] = process.argv.slice(2);
if (!url || !secret || !['old', 'new'].includes(side)) {
  console.error('usage: compat-tokens.mjs <ws-url> <secret> <old|new>');
  process.exit(64);
}
if (typeof WebSocket !== 'function') {
  console.error('compat-tokens: Node 22+ is required (global WebSocket)');
  process.exit(64);
}

const now = () => Math.floor(Date.now() / 1000);

function mint(payload, key = secret) {
  const body = Buffer.from(payload).toString('base64url');
  const sig = crypto.createHmac('sha256', key).update(body).digest('base64url');
  return `${body}.${sig}`;
}

// ── minimal RESP reader ─────────────────────────────────────────────────────
// Returns [value, rest] for one complete value at the start of `s`, or null
// when more bytes are needed. Errors become { error: "..." }.
function parse(s) {
  const nl = s.indexOf('\r\n');
  if (nl < 0) return null;
  const head = s.slice(1, nl);
  const rest = s.slice(nl + 2);
  switch (s[0]) {
    case '+': case ':': case ',': case '#': case '_': return [head, rest];
    case '-': case '!': return [{ error: head }, rest];
    case '$': case '=': {
      const n = Number(head);
      if (n < 0) return [null, rest];
      if (rest.length < n + 2) return null;
      return [rest.slice(0, n), rest.slice(n + 2)];
    }
    case '*': case '~': case '>': case '%': {
      let n = Number(head);
      if (s[0] === '%') n *= 2;
      const items = [];
      let r = rest;
      for (let i = 0; i < n; i++) {
        const v = parse(r);
        if (!v) return null;
        items.push(v[0]);
        r = v[1];
      }
      return [items, r];
    }
    default: return [{ error: `unparsed reply ${JSON.stringify(s.slice(0, 40))}` }, ''];
  }
}

function encode(args) {
  let out = `*${args.length}\r\n`;
  for (const a of args) out += `$${Buffer.byteLength(a)}\r\n${a}\r\n`;
  return out;
}

class Conn {
  static open(u) {
    return new Promise((resolve, reject) => {
      const c = new Conn(new WebSocket(u));
      c.ws.onopen = () => resolve(c);
      c.ws.onerror = () => reject(new Error(`cannot connect to ${u}`));
    });
  }

  constructor(ws) {
    this.ws = ws;
    this.buf = '';
    this.replies = [];
    this.waiters = [];
    this.closeCode = null;
    this.closed = new Promise((resolve) => {
      ws.onclose = (e) => {
        this.closeCode = e.code;
        resolve(e.code);
        for (const w of this.waiters.splice(0)) w({ closed: e.code });
      };
    });
    ws.onmessage = async (e) => {
      this.buf += typeof e.data === 'string' ? e.data : Buffer.from(await e.data.arrayBuffer()).toString();
      for (let v = parse(this.buf); v; v = parse(this.buf)) {
        this.buf = v[1];
        // Every value is a reply: the socket stays on RESP2 and subscribes to
        // nothing, so no pushes arrive to be mistaken for one.
        const w = this.waiters.shift();
        if (w) w(v[0]); else this.replies.push(v[0]);
      }
    };
  }

  cmd(...args) {
    if (this.closeCode !== null) return Promise.resolve({ closed: this.closeCode });
    this.ws.send(encode(args));
    if (this.replies.length) return Promise.resolve(this.replies.shift());
    return new Promise((resolve, reject) => {
      const t = setTimeout(() => reject(new Error(`no reply to ${args[0]} ${args[1] ?? ''}`)), 5000);
      this.waiters.push((v) => { clearTimeout(t); resolve(v); });
    });
  }

  waitClose(ms) {
    return Promise.race([this.closed, new Promise((r) => setTimeout(() => r(null), ms))]);
  }

  close() { try { this.ws.close(); } catch { /* already closed */ } }
}

// ── checks ───────────────────────────────────────────────────────────────────

let failures = 0;
const ok = (m) => console.log(`ok    tokens(${side}): ${m}`);
const fail = (m) => { console.log(`FAIL  tokens(${side}): ${m}`); failures++; };
const isErr = (v) => v && typeof v === 'object' && !Array.isArray(v) && 'error' in v;
const show = (v) => JSON.stringify(v);

async function expectAccepted(label, token) {
  const c = await Conn.open(url);
  const r = await c.cmd('SYNC', 'TOKEN', token);
  if (isErr(r) || r?.closed !== undefined) fail(`${label} was refused: ${show(r)}`);
  else ok(`${label} is accepted`);
  return c;
}

async function expectRefused(label, token) {
  const c = await Conn.open(url);
  const r = await c.cmd('SYNC', 'TOKEN', token);
  if (isErr(r)) ok(`${label} is refused (${r.error.split(' ')[0]})`);
  else fail(`${label} was accepted: ${show(r)}`);
  // Refused means no scope at all, not some default one.
  const g = await c.cmd('GET', 'compat:a:k');
  if (isErr(g) && /NOSCOPE/.test(g.error)) ok(`${label}: the socket stays unscoped`);
  else fail(`${label}: a refused token left the socket able to read (${show(g)})`);
  c.close();
}

try {
  const exp = now() + 3600;

  // Two-segment: the 0.5 form, which 0.6 must keep accepting.
  const a = await expectAccepted('two-segment token', mint(`rw=compat:a:*|${exp}`));
  const w = await a.cmd('SET', 'compat:a:k', 'v');
  if (w === 'OK') ok('two-segment token: write inside the grant');
  else fail(`two-segment token: write inside the grant answered ${show(w)}`);
  const x = await a.cmd('GET', 'compat:b:k');
  if (isErr(x) && /NOSCOPE/.test(x.error)) ok('cross-tenant read is refused with NOSCOPE');
  else fail(`cross-tenant read answered ${show(x)}`);
  const y = await a.cmd('SET', 'compat:b:k', 'stolen');
  if (isErr(y) && /NOSCOPE/.test(y.error)) ok('cross-tenant write is refused with NOSCOPE');
  else fail(`cross-tenant write answered ${show(y)}`);
  a.close();

  await expectRefused('expired token', mint(`rw=compat:a:*|${now() - 60}`));
  await expectRefused('token signed with another secret', mint(`rw=compat:a:*|${exp}`, `${secret}-other`));

  const claims = mint(`rw=compat:a:*|${exp}|sub=compat-user-1`);
  if (side === 'old') {
    // Fail closed: an old server must not accept the token minus its claims.
    await expectRefused('claims-bearing token', claims);
  } else {
    const c = await expectAccepted('claims-bearing token', claims);

    // Renewal: same grants and subject, later expiry — the socket stays.
    const renew = await c.cmd('SYNC', 'TOKEN', mint(`rw=compat:a:*|${exp + 600}|sub=compat-user-1`));
    const ping = await c.cmd('PING');
    if (!isErr(renew) && ping === 'PONG' && c.closeCode === null) ok('renewal keeps the socket open');
    else fail(`renewal: reply ${show(renew)}, then ${show(ping)}, close ${c.closeCode}`);

    // A different subject is a different principal: reply, then close 4003.
    await c.cmd('SYNC', 'TOKEN', mint(`rw=compat:a:*|${exp}|sub=compat-user-2`));
    const code = await c.waitClose(3000);
    if (code === 4003) ok('a subject change closes the socket with 4003');
    else fail(`a subject change: close code ${code}`);

    // Expiry binds the connection: close 4002 once the token lapses.
    const e = await expectAccepted('short-lived token', mint(`rw=compat:a:*|${now() + 2}|sub=compat-user-1`));
    const ecode = await e.waitClose(6000);
    if (ecode === 4002) ok('expiry closes a live socket with 4002');
    else fail(`expiry on a live socket: close code ${ecode}`);
  }
} catch (err) {
  fail(`aborted: ${err.message}`);
}

process.exit(failures);
