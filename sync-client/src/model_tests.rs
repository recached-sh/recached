//! Model-based randomized tests of the sync state machine.
//!
//! A small simulated server — the real engine for state, a duplicate-
//! suppression high-water mark per client, live-query snapshots and
//! keychange fan-out, the 0.5 dedup barrier and push framing — runs against
//! several real `SyncClient`s. A seeded generator interleaves local writes,
//! deliveries, lost replies, disconnects, client restarts, transient and
//! permanent refusals, and writes from elsewhere. After every step the
//! invariants below are checked; at the end everything is delivered and the
//! final state compared with the model.
//!
//! The model is the set of writes each client was told were accepted, in
//! order. Invariants:
//!
//! * **No silent loss**: every accepted write is, at quiescence, either
//!   applied by the server exactly once or reported refused — never both,
//!   never neither.
//! * **Order**: each identity's writes are applied in the order accepted.
//! * **Durable mirror**: the adapter's persisted outbox always equals the
//!   client's in-memory one.
//! * **Bounded**: a write past the cap is refused before it changes local
//!   state.
//! * **Convergence**: at quiescence every key a client's live queries cover
//!   holds the server's value, and every client is `Ready`.
//!
//! No proptest: the crate is dependency-free by design, and a seeded
//! xorshift gives the same reproducibility — a failure prints its seed and the
//! step log, and `MODEL_SEED=<n>` replays exactly that run.

use super::*;
use std::collections::{BTreeMap, HashMap};

/// xorshift64*: deterministic per seed.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

const KEYS: [&str; 6] = ["a:1", "a:2", "a:3", "b:1", "b:2", "n:1"];
const PATTERNS: [&str; 3] = ["a:*", "b:*", "*"];

/// One server-side connection.
struct Conn {
    to_server: VecDeque<Vec<u8>>,
    to_client: VecDeque<Vec<u8>>,
    patterns: Vec<String>,
    push_frames: bool,
    barrier: bool,
    /// Client ids with a write deferred on this connection (barrier).
    deferred: HashSet<String>,
}

#[derive(Default)]
struct Server {
    store: KeyValueStore,
    /// The duplicate-suppression high-water mark, per client identity.
    hwm: HashMap<String, u64>,
    /// Every DEDUP write the server executed: (client, wire id).
    applied: Vec<(String, u64)>,
    /// 0.5: permanent refusals, so a replay below the mark is refused again
    /// rather than answered `+DUP`.
    refusals: HashMap<(String, u64), String>,
    /// 0.5 server: honours `CLIENT PUSHFRAMES` and `CLIENT DEDUPBARRIER`.
    v5: bool,
    /// Percent chance a DEDUP write is refused with a transient error.
    transient_percent: u64,
}

struct Client {
    sync: SyncClient,
    conn: Option<usize>,
    /// What the adapter persisted: outbox id → frame.
    durable: BTreeMap<u64, Vec<u8>>,
    patterns: Vec<String>,
    cap: usize,
}

struct World {
    rng: Rng,
    server: Server,
    conns: Vec<Option<Conn>>,
    clients: Vec<Client>,
    /// Wire ids each identity was told were accepted, in acceptance order.
    accepted: HashMap<String, Vec<u64>>,
    /// Wire ids reported refused for good.
    refused: HashSet<(String, u64)>,
    log: Vec<String>,
    /// How often each hard path ran, so a passing run is known not to be a
    /// vacuous one.
    stats: HashMap<&'static str, u64>,
}

fn wire_of(frame: &[u8]) -> Option<(String, u64)> {
    let (value, _) = Value::parse(frame).ok()?;
    match Command::from_value(value).ok()? {
        Command::Dedup(client, id, _) => Some((client, id)),
        _ => None,
    }
}

fn new_client(id: &str, epoch: u32, cap: usize) -> SyncClient {
    let mut sync = SyncClient::new(Arc::new(KeyValueStore::new()), id.to_string());
    sync.set_epoch(epoch);
    sync.set_max_pending(cap);
    sync
}

fn text(frame: &[u8]) -> String {
    String::from_utf8_lossy(frame).into_owned()
}

impl World {
    fn new(seed: u64, clients: usize, v5: bool) -> Self {
        let mut rng = Rng::new(seed);
        let clients = (0..clients)
            .map(|i| {
                let cap = 1 + rng.below(6) as usize;
                Client {
                    sync: new_client(&format!("c{i}"), 1, cap),
                    conn: None,
                    durable: BTreeMap::new(),
                    patterns: Vec::new(),
                    cap,
                }
            })
            .collect();
        Self {
            rng,
            server: Server {
                v5,
                ..Default::default()
            },
            conns: Vec::new(),
            clients,
            accepted: HashMap::new(),
            refused: HashSet::new(),
            log: Vec::new(),
            stats: HashMap::new(),
        }
    }

    fn connected(&self, c: usize) -> bool {
        self.clients[c].conn.is_some()
    }

    // ── client side ──────────────────────────────────────────────────────

    fn send(&mut self, c: usize, frames: Vec<Vec<u8>>) {
        if let Some(conn) = self.clients[c].conn {
            let conn = self.conns[conn].as_mut().expect("live conn");
            conn.to_server.extend(frames);
        }
    }

    fn local_write(&mut self, c: usize) {
        let key = KEYS[self.rng.below(KEYS.len() as u64) as usize];
        let (cmd, frame) = match self.rng.below(3) {
            0 => {
                let v = format!("v{}", self.rng.below(100));
                (
                    Command::Set(key.into(), v.clone().into_bytes(), Default::default()),
                    to_resp(&["SET", key, &v]),
                )
            }
            1 => {
                let n = 1 + self.rng.below(5) as i64;
                (
                    Command::IncrBy(key.into(), n),
                    to_resp(&["INCRBY", key, &n.to_string()]),
                )
            }
            _ => (Command::Del(vec![key.into()]), to_resp(&["DEL", key])),
        };
        let connected = self.connected(c);
        let client = &mut self.clients[c];
        // Preflight, as an adapter does: a refused write must not change
        // local state.
        if let Err(e) = client.sync.check_capacity(true) {
            assert!(
                matches!(e, EnqueueError::OutboxFull { pending, capacity }
                    if pending >= capacity && capacity == client.cap),
                "unexpected refusal {e:?}"
            );
            let before = client.sync.store().execute(Command::Get(key.into()));
            assert_eq!(
                client.sync.store().execute(Command::Get(key.into())),
                before
            );
            self.log.push(format!("c{c} write {key} refused: full"));
            return;
        }
        if matches!(client.sync.store().execute(cmd), Value::Error(_)) {
            // The engine refused it locally (INCRBY on text): nothing queued.
            self.log.push(format!("c{c} write {key} refused locally"));
            return;
        }
        let e = client
            .sync
            .try_enqueue_write(&frame, true, connected)
            .expect("capacity checked");
        client.durable.insert(e.id, e.frame.clone());
        let (who, wire) = wire_of(&e.frame).expect("dedup frame");
        self.accepted.entry(who).or_default().push(wire);
        self.log.push(format!(
            "c{c} write {} id {} wire {wire}",
            text(&frame).replace("\r\n", " "),
            e.id
        ));
        if e.send_now {
            self.send(c, vec![e.frame]);
        }
    }

    fn connect(&mut self, c: usize) {
        if self.connected(c) {
            return;
        }
        let idx = self.conns.len();
        self.conns.push(Some(Conn {
            to_server: VecDeque::new(),
            to_client: VecDeque::new(),
            patterns: Vec::new(),
            push_frames: false,
            barrier: false,
            deferred: HashSet::new(),
        }));
        self.clients[c].conn = Some(idx);
        self.clients[c].sync.on_connecting();
        let frames = self.clients[c].sync.on_open();
        self.log.push(format!("c{c} open conn {idx}"));
        self.send(c, frames);
    }

    /// Drop the socket: everything in flight either way is lost — this is
    /// where acknowledgments go missing.
    fn disconnect(&mut self, c: usize) {
        if let Some(idx) = self.clients[c].conn.take() {
            self.conns[idx] = None;
            self.clients[c].sync.on_close();
            self.log.push(format!("c{c} close conn {idx}"));
        }
    }

    /// The app is killed and reopened: memory gone, the durable outbox and
    /// the local copy (mobile-style persistence) survive, the epoch advances.
    /// With `open_first`, the socket opens before the outbox is restored —
    /// the browser's order, where IndexedDB loads asynchronously.
    fn restart(&mut self, c: usize, open_first: bool) {
        self.disconnect(c);
        let old = &self.clients[c];
        let id = old.sync.client_id().to_string();
        let epoch = next_epoch(old.sync.epoch()).expect("epoch");
        let entries = old.sync.store().snapshot();
        let rows: Vec<(u64, Vec<u8>)> = old.durable.iter().map(|(k, v)| (*k, v.clone())).collect();
        let cap = old.cap;
        let patterns = old.patterns.clone();
        let mut sync = new_client(&id, epoch, cap);
        sync.store().restore(entries);
        for p in &patterns {
            sync.add_live_query(p, false);
        }
        self.clients[c] = Client {
            sync,
            conn: None,
            durable: BTreeMap::new(),
            patterns,
            cap,
        };
        if open_first {
            self.connect(c);
        }
        let restored = self.clients[c].sync.restore_outbox(rows, open_first);
        assert_eq!(
            restored.send_now.is_empty(),
            !open_first || restored.rewrites.is_empty()
        );
        self.clients[c].durable = restored
            .rewrites
            .into_iter()
            .map(|(_, new, frame)| (new, frame))
            .collect();
        self.send(c, restored.send_now);
        self.log.push(format!(
            "c{c} restart epoch {epoch} open_first={open_first}"
        ));
        *self.stats.entry("restart").or_default() += 1;
    }

    fn watch(&mut self, c: usize) {
        let p = PATTERNS[self.rng.below(PATTERNS.len() as u64) as usize];
        if self.clients[c].patterns.iter().any(|q| q == p) {
            return;
        }
        self.clients[c].patterns.push(p.to_string());
        let connected = self.connected(c);
        if let Some(f) = self.clients[c].sync.add_live_query(p, connected) {
            self.send(c, vec![f]);
        }
        self.log.push(format!("c{c} watch {p}"));
    }

    /// Deliver the next frame waiting for client `c`.
    fn deliver(&mut self, c: usize) -> bool {
        let Some(idx) = self.clients[c].conn else {
            return false;
        };
        let Some(frame) = self.conns[idx]
            .as_mut()
            .and_then(|k| k.to_client.pop_front())
        else {
            return false;
        };
        let incoming = self.clients[c].sync.handle_frame(&frame);
        self.log.push(format!(
            "c{c} <- {} => {incoming:?}",
            text(&frame).replace("\r\n", " ")
        ));
        let client = &mut self.clients[c];
        match incoming {
            Incoming::Reply { retired } | Incoming::AppliedReply { retired, .. } => {
                if let Some(id) = retired {
                    client.durable.remove(&id);
                }
            }
            Incoming::Refused { retired, keys, .. } => {
                let frame = client
                    .durable
                    .remove(&retired)
                    .expect("refused row persisted");
                let wire = wire_of(&frame).expect("dedup frame");
                assert!(self.refused.insert(wire), "refused twice");
                let frames = client.sync.resubscribe_covering(&keys, true);
                self.send(c, frames);
            }
            Incoming::Deferred { .. } | Incoming::Malformed => {
                self.disconnect(c);
                return true;
            }
            Incoming::Applied { .. } | Incoming::PubSub { .. } | Incoming::Ignored => {}
        }
        let frames = self.clients[c].sync.take_frames_to_send(true);
        if !frames.is_empty() {
            *self.stats.entry("resync").or_default() += 1;
        }
        self.send(c, frames);
        true
    }

    // ── server side ──────────────────────────────────────────────────────

    fn push_keychange(&mut self, key: &str) {
        let value = match self.server.store.execute(Command::Get(key.into())) {
            Value::BulkString(b) => Value::BulkString(b),
            _ => Value::BulkString(None),
        };
        for conn in self.conns.iter_mut().flatten() {
            if conn.patterns.iter().any(|p| glob_match(p, key)) {
                let items = vec![
                    Value::BulkString(Some(b"keychange".to_vec())),
                    Value::BulkString(Some(key.as_bytes().to_vec())),
                    value.clone(),
                ];
                let mut bytes = Value::Array(Some(items)).serialize();
                if conn.push_frames {
                    bytes[0] = b'>';
                }
                conn.to_client.push_back(bytes);
            }
        }
    }

    /// Process the next frame client `c` sent.
    fn process(&mut self, c: usize) -> bool {
        let Some(idx) = self.clients[c].conn else {
            return false;
        };
        let Some(frame) = self.conns[idx]
            .as_mut()
            .and_then(|k| k.to_server.pop_front())
        else {
            return false;
        };
        let (value, _) = Value::parse(&frame).expect("client frames parse");
        let cmd = Command::from_value(value).expect("client frames are commands");
        let v5 = self.server.v5;
        let mut changed: Vec<String> = Vec::new();
        let reply: Vec<u8> = match cmd {
            Command::Client(args) => {
                let conn = self.conns[idx].as_mut().expect("live conn");
                match (args[0].as_str(), v5) {
                    ("PUSHFRAMES", true) => {
                        conn.push_frames = true;
                        b"+OK\r\n".to_vec()
                    }
                    ("DEDUPBARRIER", true) => {
                        conn.barrier = true;
                        b"+OK\r\n".to_vec()
                    }
                    ("DELTA" | "EXPIRY", _) => b"+OK\r\n".to_vec(),
                    _ => b"-ERR unknown subcommand\r\n".to_vec(),
                }
            }
            Command::QSub(pattern) => {
                let conn = self.conns[idx].as_mut().expect("live conn");
                if !conn.patterns.contains(&pattern) {
                    conn.patterns.push(pattern.clone());
                }
                let mut kvs = self.server.store.matching_key_values(&pattern, usize::MAX);
                kvs.sort_by(|a, b| a.0.cmp(&b.0));
                let mut items = vec![
                    Value::BulkString(Some(b"qstate".to_vec())),
                    Value::BulkString(Some(pattern.into_bytes())),
                ];
                for (k, v) in kvs {
                    items.push(Value::BulkString(Some(k.into_bytes())));
                    items.push(v);
                }
                Value::Array(Some(items)).serialize()
            }
            Command::QUnsub(_) => b"+OK\r\n".to_vec(),
            Command::Dedup(client, id, inner) => {
                let transient = self.rng.chance(self.server.transient_percent);
                let conn = self.conns[idx].as_mut().expect("live conn");
                let refusal = self
                    .server
                    .refusals
                    .get(&(client.clone(), id))
                    .filter(|_| v5);
                if conn.barrier && conn.deferred.contains(&client) {
                    b"-TRYAGAIN an earlier write from this client was deferred\r\n".to_vec()
                } else if self.server.hwm.get(&client).is_some_and(|hwm| id <= *hwm) {
                    // Below the mark the write cannot run again: a remembered
                    // refusal says it was refused rather than applied.
                    match refusal {
                        Some(error) => Value::Error(error.clone()).serialize(),
                        None => b"+DUP\r\n".to_vec(),
                    }
                } else if transient {
                    if conn.barrier {
                        conn.deferred.insert(client);
                    }
                    b"-OOM command not allowed when used memory > 'maxmemory'\r\n".to_vec()
                } else {
                    changed = mutation_keys(&inner, &self.server.store);
                    let response = self.server.store.execute(*inner);
                    if let Value::Error(error) = &response {
                        changed.clear();
                        if !is_transient_refusal(error) {
                            self.server.refusals.insert((client, id), error.clone());
                        }
                    } else {
                        // Applied on a retry after a refusal: forget it.
                        self.server.refusals.remove(&(client.clone(), id));
                        self.server.hwm.insert(client.clone(), id);
                        self.server.applied.push((client, id));
                    }
                    response.serialize()
                }
            }
            other => panic!("model server got an unexpected command {other:?}"),
        };
        for (stat, needle) in [
            ("dup", &b"+DUP"[..]),
            ("barrier", b"-TRYAGAIN"),
            ("deferred", b"-OOM"),
            ("refused", b"-ERR value"),
        ] {
            if reply.starts_with(needle) {
                *self.stats.entry(stat).or_default() += 1;
            }
        }
        self.log.push(format!(
            "server conn {idx} c{c} {} -> {}",
            text(&frame).replace("\r\n", " "),
            text(&reply).replace("\r\n", " ")
        ));
        self.conns[idx]
            .as_mut()
            .expect("live conn")
            .to_client
            .push_back(reply);
        for key in changed {
            self.push_keychange(&key);
        }
        true
    }

    /// A write from outside the clients: another backend, a TCP user.
    fn foreign_write(&mut self) {
        let key = KEYS[self.rng.below(KEYS.len() as u64) as usize];
        if self.rng.chance(30) {
            self.server.store.execute(Command::Del(vec![key.into()]));
        } else {
            let v = format!("x{}", self.rng.below(100));
            self.server
                .store
                .execute(Command::Set(key.into(), v.into_bytes(), Default::default()));
        }
        self.log.push(format!("server foreign write {key}"));
        self.push_keychange(key);
    }

    // ── invariants ───────────────────────────────────────────────────────

    fn check_step(&self) {
        for (i, c) in self.clients.iter().enumerate() {
            let memory: BTreeMap<u64, Vec<u8>> = c.sync.outbox.iter().cloned().collect();
            assert_eq!(memory, c.durable, "c{i}: durable outbox drifted");
        }
        // Each identity's applied writes are in acceptance order and unique.
        let mut by_client: HashMap<&str, Vec<u64>> = HashMap::new();
        for (client, id) in &self.server.applied {
            by_client.entry(client).or_default().push(*id);
        }
        for (client, ids) in by_client {
            assert!(
                ids.windows(2).all(|w| w[0] < w[1]),
                "{client}: applied out of order or twice: {ids:?}"
            );
        }
    }

    fn quiesce(&mut self) {
        self.server.transient_percent = 0;
        for _round in 0..10_000 {
            let mut progressed = false;
            for c in 0..self.clients.len() {
                if !self.connected(c) {
                    self.connect(c);
                    progressed = true;
                }
                while self.process(c) {
                    progressed = true;
                }
                while self.deliver(c) {
                    progressed = true;
                }
            }
            self.check_step();
            if !progressed {
                return;
            }
        }
        panic!("did not quiesce");
    }

    fn check_final(&self) {
        let applied: HashSet<(String, u64)> = self.server.applied.iter().cloned().collect();
        assert_eq!(
            applied.len(),
            self.server.applied.len(),
            "a write was executed twice"
        );
        for (client, wires) in &self.accepted {
            for wire in wires {
                let key = (client.clone(), *wire);
                let a = applied.contains(&key);
                let r = self.refused.contains(&key);
                assert!(
                    a ^ r,
                    "{client} wire {wire}: applied={a} refused={r} — {}",
                    if !a && !r { "silently lost" } else { "both" }
                );
            }
        }
        for (i, c) in self.clients.iter().enumerate() {
            assert_eq!(c.sync.outbox_len(), 0, "c{i}: outbox not drained");
            assert_eq!(
                c.sync.status(),
                SyncStatus {
                    phase: SyncPhase::Ready,
                    error: None
                },
                "c{i}: not ready at quiescence"
            );
            for key in KEYS {
                if c.patterns.iter().any(|p| glob_match(p, key)) {
                    assert_eq!(
                        c.sync.store().execute(Command::Get(key.into())),
                        self.server.store.execute(Command::Get(key.into())),
                        "c{i}: {key} diverged from the server"
                    );
                }
            }
        }
    }

    fn step(&mut self) {
        let c = self.rng.below(self.clients.len() as u64) as usize;
        match self.rng.below(100) {
            0..=24 => self.local_write(c),
            25..=44 => {
                self.process(c);
            }
            45..=64 => {
                self.deliver(c);
            }
            65..=71 => self.disconnect(c),
            72..=81 => self.connect(c),
            82..=83 => self.restart(c, false),
            84..=85 => self.restart(c, true),
            86..=89 => self.watch(c),
            90..=95 => self.foreign_write(),
            _ => {
                self.server.transient_percent = [0, 0, 10, 30][self.rng.below(4) as usize];
            }
        }
        self.check_step();
    }
}

/// Run one seeded sequence, printing the seed and the recent steps on failure.
/// Returns how often each hard path ran.
fn run(
    seed: u64,
    steps: usize,
    clients: usize,
    v5: bool,
    transient: bool,
) -> HashMap<&'static str, u64> {
    let mut world = World::new(seed, clients, v5);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for _ in 0..steps {
            world.step();
            if !transient {
                world.server.transient_percent = 0;
            }
        }
        world.quiesce();
        world.check_final();
    }));
    if let Err(panic) = outcome {
        eprintln!(
            "model failure: seed {seed} (v5={v5}, {clients} clients, {steps} steps). \
             Replay with MODEL_SEED={seed}. Last steps, newest first:"
        );
        for line in world.log.iter().rev().take(
            std::env::var("MODEL_TAIL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
        ) {
            eprintln!("  {line}");
        }
        std::panic::resume_unwind(panic);
    }
    world.stats
}

fn total(into: &mut HashMap<&'static str, u64>, from: HashMap<&'static str, u64>) {
    for (k, v) in from {
        *into.entry(k).or_default() += v;
    }
}

fn seeds(default: std::ops::Range<u64>) -> Vec<u64> {
    match std::env::var("MODEL_SEED") {
        Ok(s) => vec![s.parse().expect("MODEL_SEED is a number")],
        Err(_) => default.collect(),
    }
}

#[test]
fn model_no_write_is_lost_duplicated_or_reordered_against_a_v5_server() {
    let mut stats = HashMap::new();
    for seed in seeds(0..400) {
        total(&mut stats, run(seed, 300, 2, true, true));
    }
    if std::env::var("MODEL_SEED").is_err() {
        for path in ["dup", "barrier", "deferred", "refused", "restart", "resync"] {
            assert!(
                stats.get(path).copied().unwrap_or(0) > 0,
                "the model never exercised {path}: {stats:?}"
            );
        }
    }
}

#[test]
fn model_three_clients_long_run() {
    for seed in seeds(1_000..1_040) {
        run(seed, 2_000, 3, true, true);
    }
}

#[test]
fn model_a_v5_server_without_transient_refusals_loses_nothing() {
    for seed in seeds(2_000..2_200) {
        run(seed, 300, 2, true, false);
    }
}

/// A 0.4 server has two gaps the model finds, both because one high-water
/// mark per client cannot tell "applied" from "not applied" below it:
///
/// * no dedup barrier — a write accepted after a deferred one raises the mark
///   past it, and the deferred write's replay is skipped as a duplicate;
/// * no refusal memory — a permanently refused write whose reply was lost is
///   answered `+DUP` on replay, so the client retires it as applied.
///
/// This pins them as the documented limit of syncing with an older server.
#[test]
fn model_a_v4_server_can_report_an_unapplied_write_as_applied() {
    let lost = (0..400u64).any(|seed| {
        let mut world = World::new(seed, 2, false);
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for _ in 0..300 {
                world.step();
            }
            world.quiesce();
            world.check_final();
        }))
        .is_err()
    });
    assert!(lost, "expected the v4 model to show the deferred-write gap");
}
