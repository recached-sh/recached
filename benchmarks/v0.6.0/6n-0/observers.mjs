// Recached 6N-0: observers that hold a write load on the AOF path that is not
// the connection-gated one. While a key a write touches is WATCHed, or a
// WebSocket client would receive the write, `on_write_logged` cannot defer
// the AOF wait to the connection: every write takes the per-command append.
// These are Recached-specific synchronization workloads with no Redis
// equivalent, so the kit reports them apart from the Redis comparisons.
//
// Usage: node observers.mjs <mode> <host> <tcpPort> <wsPort> <count> <arg> <seconds> <out.json>
//   ws-sync    <count> WebSocket clients, each `SYNC <arg>` (open mode)
//   ws-qsub    <count> WebSocket clients, each `QSUB <arg>`
//   tcp-watch  <count> TCP connections, each WATCHing <arg> keys sampled
//              from redis-benchmark's keyspace (key:%012d, KEYSPACE env)
// Prints READY once every observer is subscribed, then counts frames
// (pushes) and errors until <seconds> pass, and writes a JSON summary.
// Node 22+ (global WebSocket).
import net from 'node:net';
import { writeFileSync } from 'node:fs';

const [mode, host, tcpPort, wsPort, countArg, arg, secondsArg, out] = process.argv.slice(2);
if (!out) {
  console.error('usage: node observers.mjs <ws-sync|ws-qsub|tcp-watch> <host> <tcpPort> <wsPort> <count> <arg> <seconds> <out.json>');
  process.exit(64);
}
const count = Number(countArg);
const seconds = Number(secondsArg);
const keyspace = Number(process.env.KEYSPACE ?? 100000);
const password = process.env.RECACHED_PASSWORD ?? '';
const encode = (a) => `*${a.length}\r\n` + a.map((x) => `$${Buffer.byteLength(String(x))}\r\n${x}\r\n`).join('');
const st = { mode, count, arg, connected: 0, subscribed: 0, frames: 0, bytes: 0, errors: 0, errorSamples: [], closes: 0 };
const noteError = (s) => { st.errors++; if (st.errorSamples.length < 5) st.errorSamples.push(String(s).slice(0, 200)); };
const sockets = [];

function wsObserver(i) {
  return new Promise((resolve) => {
    const ws = new WebSocket(`ws://${host}:${wsPort}`);
    sockets.push(ws);
    let ready = false;
    ws.onopen = () => {
      st.connected++;
      if (password) ws.send(encode(['AUTH', password]));
      ws.send(encode(mode === 'ws-sync' ? ['SYNC', arg] : ['QSUB', arg]));
    };
    ws.onmessage = (e) => {
      const text = typeof e.data === 'string' ? e.data : '';
      st.frames++;
      st.bytes += typeof e.data === 'string' ? Buffer.byteLength(e.data) : (e.data.byteLength ?? 0);
      if (text.startsWith('-')) noteError(text);
      if (!ready) { ready = true; st.subscribed++; resolve(); }
    };
    ws.onerror = (e) => { noteError(e.message ?? 'ws error'); if (!ready) { ready = true; resolve(); } };
    ws.onclose = () => { st.closes++; if (!ready) { ready = true; resolve(); } };
  });
}

function tcpWatcher(i) {
  return new Promise((resolve) => {
    const keys = [];
    for (let k = 0; k < Number(arg); k++) {
      keys.push(`key:${String(Math.floor(((i * 7919 + k * 104729) % keyspace + keyspace) % keyspace)).padStart(12, '0')}`);
    }
    const sock = net.connect(Number(tcpPort), host, () => {
      st.connected++;
      if (password) sock.write(encode(['AUTH', password]));
      sock.write(encode(['WATCH', ...keys]));
    });
    sockets.push(sock);
    let ready = false;
    sock.on('data', (d) => {
      st.frames++;
      st.bytes += d.length;
      if (d.toString().startsWith('-')) noteError(d.toString());
      if (!ready) { ready = true; st.subscribed++; resolve(); }
    });
    sock.on('error', (e) => { noteError(e.message); if (!ready) { ready = true; resolve(); } });
    sock.on('close', () => { st.closes++; if (!ready) { ready = true; resolve(); } });
  });
}

const make = mode === 'tcp-watch' ? tcpWatcher : (mode === 'ws-sync' || mode === 'ws-qsub') ? wsObserver : null;
if (!make) { console.error(`unknown mode ${mode}`); process.exit(64); }
await Promise.all(Array.from({ length: count }, (_, i) => make(i)));
console.log(`READY ${st.subscribed}/${count} subscribed, ${st.errors} errors`);
const subscribedFrames = st.frames;
await new Promise((r) => setTimeout(r, seconds * 1000));
st.pushes = st.frames - subscribedFrames;
for (const s of sockets) { try { s.close ? s.close() : s.destroy(); } catch {} try { s.destroy?.(); } catch {} }
writeFileSync(out, JSON.stringify(st, null, 1));
console.log(JSON.stringify(st));
process.exit(0);
