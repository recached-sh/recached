#!/usr/bin/env node
// Browser end-to-end checks for recached-edge: real Chromium, real IndexedDB,
// a real recached-server.
//
// What the unit tests cannot reach — IndexedDB transactions, page reloads,
// two tabs sharing one database, a WebSocket to a live server — is exercised
// here through the public SDK, exactly as an application uses it.
//
//   PLAYWRIGHT_CORE=<dir containing node_modules/playwright-core> \
//   RECACHED_SERVER_BIN=<path to recached-server> \
//   node scripts/e2e-browser.mjs
//
// Needs `pkg/` built and `sdk.js` compiled (see README). Playwright is not a
// dependency of this package: point PLAYWRIGHT_CORE at any install of it.
// CHROMIUM may name the browser binary; otherwise Playwright's own is used.
// Exits non-zero on the first failure, naming the scenario.

import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import { createConnection } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';

const ROOT = path.resolve(import.meta.dirname, '..');

function need(name) {
  const v = process.env[name];
  if (!v) {
    console.error(`e2e-browser: set ${name} (see the header of this script)`);
    process.exit(2);
  }
  return v;
}

const require = createRequire(path.join(need('PLAYWRIGHT_CORE'), 'noop.js'));
const { chromium } = require('playwright-core');
const SERVER_BIN = need('RECACHED_SERVER_BIN');

// ── static server for the SDK ────────────────────────────────────────────────

const TYPES = { '.js': 'text/javascript', '.wasm': 'application/wasm', '.html': 'text/html' };

function serveSdk() {
  const server = createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    if (url.pathname === '/') {
      res.writeHead(200, { 'content-type': 'text/html' });
      res.end('<!doctype html><title>e2e</title>');
      return;
    }
    const file = path.join(ROOT, path.normalize(url.pathname));
    if (!file.startsWith(ROOT)) {
      res.writeHead(403).end();
      return;
    }
    try {
      statSync(file);
      res.writeHead(200, { 'content-type': TYPES[path.extname(file)] ?? 'application/octet-stream' });
      res.end(readFileSync(file));
    } catch {
      res.writeHead(404).end();
    }
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

// ── recached-server ──────────────────────────────────────────────────────────

const freePort = () =>
  new Promise((resolve) => {
    const s = createServer();
    s.listen(0, '127.0.0.1', () => {
      const { port } = s.address();
      s.close(() => resolve(port));
    });
  });

async function startServer(dir) {
  const [tcp, ws] = [await freePort(), await freePort()];
  const child = spawn(SERVER_BIN, [], {
    cwd: dir,
    env: {
      RECACHED_BIND: '127.0.0.1',
      RECACHED_PORT: String(tcp),
      RECACHED_WS_PORT: String(ws),
      RECACHED_METRICS_PORT: '0',
      RECACHED_SAVE_INTERVAL: '0',
      RECACHED_SAVE_PATH: path.join(dir, 'dump.rdb'),
    },
    stdio: 'ignore',
  });
  for (let i = 0; i < 200; i++) {
    if (await canConnect(ws)) return { child, tcp, ws };
    await new Promise((r) => setTimeout(r, 25));
  }
  throw new Error('recached-server did not start');
}

const canConnect = (port) =>
  new Promise((resolve) => {
    const s = createConnection({ port, host: '127.0.0.1' }, () => (s.end(), resolve(true)));
    s.on('error', () => resolve(false));
  });

/** Run one RESP command on the TCP port and return the raw reply text. */
function resp(port, ...args) {
  return new Promise((resolve, reject) => {
    const s = createConnection({ port, host: '127.0.0.1' });
    let out = Buffer.alloc(0);
    s.on('data', (d) => {
      out = Buffer.concat([out, d]);
      s.end();
    });
    s.on('end', () => resolve(out.toString('latin1')));
    s.on('error', reject);
    const enc = (a) => {
      const b = Buffer.isBuffer(a) ? a : Buffer.from(String(a));
      return Buffer.concat([Buffer.from(`$${b.length}\r\n`), b, Buffer.from('\r\n')]);
    };
    s.write(Buffer.concat([Buffer.from(`*${args.length}\r\n`), ...args.map(enc)]));
  });
}

// ── scenarios ────────────────────────────────────────────────────────────────

/** In the page: import the SDK and keep a log of status phases. */
const PRELUDE = `
  window.sdk = await import('/sdk.js');
  window.phases = [];
  window.track = (cache) => cache.onStatusChange((s) => window.phases.push(s.phase));
  window.until = async (fn, ms = 5000) => {
    const end = Date.now() + ms;
    while (Date.now() < end) { if (await fn()) return true; await new Promise(r => setTimeout(r, 20)); }
    throw new Error('timed out waiting for ' + fn);
  };
`;

async function page(context, base) {
  const p = await context.newPage();
  p.on('pageerror', (e) => console.error('  page error:', e.message));
  await p.goto(base);
  await p.evaluate(`(async () => { ${PRELUDE} })()`);
  return p;
}

const scenarios = [];
const scenario = (name, fn) => scenarios.push({ name, fn });

scenario('offline writes survive a reload and replay once connected', async ({ context, base, srv }) => {
  const p = await page(context, base);
  await p.evaluate(`(async () => {
    const c = await sdk.createCache({ persistence: true });
    c.set('offline:a', '1');
    c.incr('offline:n', 5);
    await c.flush();
  })()`);
  await p.reload();
  await p.evaluate(`(async () => { ${PRELUDE} })()`);
  const got = await p.evaluate(`(async () => {
    const c = await sdk.createCache({ persistence: true, connect: { url: 'ws://127.0.0.1:${srv.ws}' } });
    const restored = { a: c.get('offline:a'), n: c.get('offline:n'), pending: c.pendingWrites() };
    await until(() => c.status().phase === 'ready' && c.pendingWrites() === 0);
    return restored;
  })()`);
  if (got.a !== '1' || got.n !== '5' || got.pending !== 2) throw new Error(JSON.stringify(got));
  if ((await resp(srv.tcp, 'GET', 'offline:n')) !== '$1\r\n5\r\n') throw new Error('INCR not applied once');
  await p.close();
});

scenario('two tabs sharing one IndexedDB lose no writes', async ({ context, base, srv }) => {
  // Before 0.5 both tabs adopted the stored identity with different epochs:
  // the newer tab's writes made the older tab's be skipped as duplicates.
  const tabs = [await page(context, base), await page(context, base)];
  for (const [i, t] of tabs.entries()) {
    await t.evaluate(`(async () => {
      window.c = await sdk.createCache({ persistence: true, connect: { url: 'ws://127.0.0.1:${srv.ws}' } });
      await until(() => c.status().phase === 'ready');
      window.tab = ${i};
    })()`);
  }
  for (let round = 0; round < 10; round++) {
    for (const t of tabs) await t.evaluate(`c.set('tabs:' + tab + ':' + ${round}, 'v')`);
  }
  for (const t of tabs) await t.evaluate(`until(() => c.pendingWrites() === 0)`);
  for (let tab = 0; tab < 2; tab++) {
    for (let round = 0; round < 10; round++) {
      const r = await resp(srv.tcp, 'GET', `tabs:${tab}:${round}`);
      if (r !== '$1\r\nv\r\n') throw new Error(`tabs:${tab}:${round} lost (${JSON.stringify(r)})`);
    }
  }
  for (const t of tabs) await t.close();
});

scenario('the status reaches ready only after replay and hydration', async ({ context, base, srv }) => {
  const p = await page(context, base);
  const phases = await p.evaluate(`(async () => {
    const c = await sdk.createCache();
    track(c);
    c.liveQuery('status:*');
    c.raw.connect('ws://127.0.0.1:${srv.ws}');
    await until(() => c.status().phase === 'ready');
    return phases;
  })()`);
  const want = ['connecting', 'authenticating', 'hydrating', 'ready'];
  const seen = want.filter((ph) => phases.includes(ph));
  if (seen.join() !== want.join() || phases.indexOf('ready') < phases.indexOf('hydrating')) {
    throw new Error(JSON.stringify(phases));
  }
  const last = await p.evaluate(`(async () => sdk && window.phases.length)()`);
  if (!last) throw new Error('no status events');
  await p.close();
});

scenario('a full outbox throws OutboxFullError and changes nothing', async ({ context, base }) => {
  const p = await page(context, base);
  const r = await p.evaluate(`(async () => {
    const c = await sdk.createCache({ persistence: true, outbox: { maxPending: 2 } });
    await c.clearPersistence();
    c.set('full:a', '1');
    c.set('full:b', '1');
    let error;
    try { c.set('full:a', 'changed'); } catch (e) { error = e; }
    return {
      isClass: error instanceof sdk.OutboxFullError,
      pending: error && error.pending,
      capacity: error && error.capacity,
      a: c.get('full:a'),
    };
  })()`);
  if (!r.isClass || r.pending !== 2 || r.capacity !== 2 || r.a !== '1') throw new Error(JSON.stringify(r));
  await p.close();
});

scenario('an IndexedDB failure is reported and flush rejects', async ({ context, base }) => {
  const p = await page(context, base);
  const r = await p.evaluate(`(async () => {
    const c = await sdk.createCache({ persistence: true });
    const failures = [];
    c.onPersistenceError((op, msg) => failures.push(op + ': ' + msg));
    const put = IDBObjectStore.prototype.put;
    IDBObjectStore.prototype.put = function () {
      throw new DOMException('quota exceeded (injected)', 'QuotaExceededError');
    };
    c.set('quota:k', 'v');
    let rejected = null;
    try { await c.flush(); } catch (e) { rejected = String(e); }
    IDBObjectStore.prototype.put = put;
    await until(() => failures.length > 0);
    return { failures, rejected, failuresInStatus: c.status().persistenceFailures, inMemory: c.get('quota:k') };
  })()`);
  if (!r.rejected || !r.failures[0].includes('quota') || r.failuresInStatus < 1 || r.inMemory !== 'v') {
    throw new Error(JSON.stringify(r));
  }
  await p.close();
});

scenario('an offline reload shows the last synchronized server state', async ({ context, base, srv }) => {
  await resp(srv.tcp, 'SET', 'synced:1', 'from-server');
  const p = await page(context, base);
  await p.evaluate(`(async () => {
    const c = await sdk.createCache({ persistence: true, connect: { url: 'ws://127.0.0.1:${srv.ws}' } });
    c.liveQuery('synced:*');
    await until(() => c.get('synced:1') === 'from-server');
    await c.flush();
  })()`);
  await resp(srv.tcp, 'DEL', 'synced:gone');
  await p.reload();
  await p.evaluate(`(async () => { ${PRELUDE} })()`);
  const v = await p.evaluate(`(async () => (await sdk.createCache({ persistence: true })).get('synced:1'))()`);
  if (v !== 'from-server') throw new Error(`got ${JSON.stringify(v)}`);
  await p.close();
});

scenario('writes after subscribe() reach the server', async ({ context, base, srv }) => {
  const p = await page(context, base);
  await p.evaluate(`(async () => {
    const c = await sdk.createCache({ connect: { url: 'ws://127.0.0.1:${srv.ws}' } });
    await until(() => c.status().phase === 'ready');
    c.subscribe('news');
    c.set('aftersub:k', 'v');
    await until(() => c.pendingWrites() === 0);
  })()`);
  if ((await resp(srv.tcp, 'GET', 'aftersub:k')) !== '$1\r\nv\r\n') throw new Error('write refused');
  await p.close();
});

scenario('a binary write reaches the server byte for byte', async ({ context, base, srv }) => {
  const p = await page(context, base);
  await p.evaluate(`(async () => {
    const c = await sdk.createCache({ connect: { url: 'ws://127.0.0.1:${srv.ws}' } });
    await until(() => c.status().phase === 'ready');
    c.setBytes('bin:k', new Uint8Array([0, 255, 27, 128]));
    await until(() => c.pendingWrites() === 0);
  })()`);
  const r = await resp(srv.tcp, 'GET', 'bin:k');
  if (r !== '$4\r\n\x00\xff\x1b\x80\r\n') throw new Error(JSON.stringify(r));
  await p.close();
});

// ── run ──────────────────────────────────────────────────────────────────────

const dir = mkdtempSync(path.join(tmpdir(), 'recached-e2e-'));
const http = await serveSdk();
const base = `http://127.0.0.1:${http.address().port}/`;
const srv = await startServer(dir);
const browser = await chromium.launch(
  process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {},
);
let failed = 0;
try {
  for (const { name, fn } of scenarios) {
    // A fresh profile per scenario: its own IndexedDB, nothing carried over.
    const context = await browser.newContext();
    try {
      await fn({ context, base, srv });
      console.log(`ok    ${name}`);
    } catch (e) {
      failed++;
      console.log(`FAIL  ${name}\n      ${e.message}`);
    } finally {
      await context.close();
    }
  }
} finally {
  await browser.close();
  srv.child.kill('SIGKILL');
  http.close();
  rmSync(dir, { recursive: true, force: true });
}
console.log(`\n${scenarios.length - failed} passed, ${failed} failed`);
process.exit(failed ? 1 : 0);
