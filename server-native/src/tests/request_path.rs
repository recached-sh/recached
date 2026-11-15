//! The whole server request path, in process: RESP bytes in, RESP bytes out,
//! through the real `handle_tcp` loop — parse, dispatch, ordering, store,
//! metrics, propagation and reply serialization — with the kernel socket
//! replaced by an in-memory duplex pipe.
//!
//! Two jobs:
//!
//! - **Allocation regression tests** (always run): a pipelined `SET`/`GET`/
//!   `INCR`/`HSET` may allocate at most a fixed number of times per command.
//!   The counting allocator below is thread-local, and the server runs on a
//!   thread of its own, so only the server's allocations are counted.
//! - **A/B benchmark** (`#[ignore]`d, run explicitly in release mode):
//!   ns and allocations per command for each workload, min and median of
//!   `REPS` runs, so two builds can be compared on a host too noisy for
//!   network benchmarks. Run with
//!   `cargo test --release -p recached request_path_bench -- --ignored --nocapture`.
//!   `BENCH_OUT=<file>` appends the rows as CSV.
//!
//! This installs the test binary's `#[global_allocator]`: a counter in front of
//! the allocator the shipped server uses (jemalloc off MSVC).

use crate::*;
use core_engine::resp::Value;
use core_engine::store::KeyValueStore;
use std::alloc::{GlobalAlloc, Layout};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicI64};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ── counting allocator ────────────────────────────────────────────────────────

thread_local! {
    // Const-initialised `Cell`s: reading them allocates nothing, so the
    // allocator cannot re-enter itself.
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
    static BYTES: Cell<u64> = const { Cell::new(0) };
}

#[cfg(not(target_env = "msvc"))]
type Inner = tikv_jemallocator::Jemalloc;
#[cfg(target_env = "msvc")]
type Inner = std::alloc::System;

struct CountingAlloc(Inner);

fn note(size: usize) {
    // `try_with`: TLS is gone during thread teardown, where nothing is tracked.
    let _ = TRACKING.try_with(|on| {
        if on.get() {
            let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
            let _ = BYTES.try_with(|b| b.set(b.get() + size as u64));
        }
    });
}

// SAFETY: every method forwards to the wrapped allocator with the caller's own
// arguments, so `GlobalAlloc`'s contract is the inner allocator's. The added
// bookkeeping touches only const-initialised thread-local `Cell`s, which never
// allocate and so cannot re-enter.
#[allow(
    unsafe_code,
    reason = "a counting GlobalAlloc is the only way to measure allocations per request; test-only"
)]
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: forwarded unchanged; the caller upheld `alloc`'s contract.
        unsafe { self.0.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: forwarded unchanged.
        unsafe { self.0.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from this allocator, i.e. from `self.0`.
        unsafe { self.0.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        // SAFETY: forwarded unchanged; `ptr` came from `self.0`.
        unsafe { self.0.realloc(ptr, layout, new_size) }
    }
}

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc(tikv_jemallocator::Jemalloc);
#[cfg(target_env = "msvc")]
#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc(std::alloc::System);

// ── harness ───────────────────────────────────────────────────────────────────

const KEYS: usize = 100_000;

fn encode(args: &[&[u8]], out: &mut Vec<u8>) {
    out.extend_from_slice(format!("*{}\r\n", args.len()).as_bytes());
    for a in args {
        out.extend_from_slice(format!("${}\r\n", a.len()).as_bytes());
        out.extend_from_slice(a);
        out.extend_from_slice(b"\r\n");
    }
}

/// The same pseudo-random key order for every build.
fn order(n: usize) -> Vec<usize> {
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    (0..n)
        .map(|_| {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            (state.wrapping_mul(0x2545_F491_4F6C_DD1D) % KEYS as u64) as usize
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Workload {
    Set,
    Get,
    Incr,
    HSet,
    HGet,
    MSet4,
    MGet4,
    HotIncr,
    SetEx,
}

impl Workload {
    const ALL: [Workload; 9] = [
        Workload::Set,
        Workload::Get,
        Workload::Incr,
        Workload::HSet,
        Workload::HGet,
        Workload::MSet4,
        Workload::MGet4,
        Workload::HotIncr,
        Workload::SetEx,
    ];

    fn name(self) -> &'static str {
        match self {
            Workload::Set => "SET",
            Workload::Get => "GET",
            Workload::Incr => "INCR",
            Workload::HSet => "HSET",
            Workload::HGet => "HGET",
            Workload::MSet4 => "MSET x4",
            Workload::MGet4 => "MGET x4",
            Workload::HotIncr => "INCR hot key",
            Workload::SetEx => "SET EX",
        }
    }

    /// `n` pipelined commands of this workload as one RESP byte stream.
    fn frames(self, n: usize) -> Vec<u8> {
        let value = [b'x'; 32];
        let mut out = Vec::with_capacity(n * 64);
        let k = |i: usize, p: &str| format!("{p}:{i:06}");
        for (j, i) in order(n).into_iter().enumerate() {
            match self {
                Workload::Set => encode(&[b"SET", k(i, "key").as_bytes(), &value], &mut out),
                Workload::Get => encode(&[b"GET", k(i, "key").as_bytes()], &mut out),
                Workload::Incr => encode(&[b"INCR", k(i, "ctr").as_bytes()], &mut out),
                Workload::HSet => encode(&[b"HSET", k(i, "h").as_bytes(), b"f1", &value], &mut out),
                Workload::HGet => encode(&[b"HGET", k(i, "h").as_bytes(), b"f1"], &mut out),
                Workload::MSet4 => {
                    let ks: Vec<String> = (0..4).map(|o| k((i + o * 7919) % KEYS, "key")).collect();
                    encode(
                        &[
                            b"MSET",
                            ks[0].as_bytes(),
                            &value,
                            ks[1].as_bytes(),
                            &value,
                            ks[2].as_bytes(),
                            &value,
                            ks[3].as_bytes(),
                            &value,
                        ],
                        &mut out,
                    );
                }
                Workload::MGet4 => {
                    let ks: Vec<String> = (0..4).map(|o| k((i + o * 7919) % KEYS, "key")).collect();
                    encode(
                        &[
                            b"MGET",
                            ks[0].as_bytes(),
                            ks[1].as_bytes(),
                            ks[2].as_bytes(),
                            ks[3].as_bytes(),
                        ],
                        &mut out,
                    );
                }
                Workload::HotIncr => encode(&[b"INCR", b"hot:counter"], &mut out),
                Workload::SetEx => {
                    let _ = j;
                    encode(
                        &[b"SET", k(i, "ttl").as_bytes(), &value, b"EX", b"3600"],
                        &mut out,
                    )
                }
            }
        }
        out
    }
}

fn prefill(store: &KeyValueStore) {
    let mut frames = Vec::new();
    for i in 0..KEYS {
        frames.clear();
        encode(
            &[b"SET", format!("key:{i:06}").as_bytes(), &[b'x'; 32]],
            &mut frames,
        );
        let (v, _) = Value::parse(&frames).expect("valid frame");
        store.execute(core_engine::cmd::Command::from_value(v).expect("valid command"));
        frames.clear();
        encode(
            &[b"HSET", format!("h:{i:06}").as_bytes(), b"f1", &[b'x'; 32]],
            &mut frames,
        );
        let (v, _) = Value::parse(&frames).expect("valid frame");
        store.execute(core_engine::cmd::Command::from_value(v).expect("valid command"));
    }
}

pub(super) struct Shared {
    pub(super) store: Arc<KeyValueStore>,
    pub(super) tx: broadcast::Sender<SyncMsg>,
    pub(super) pubsub: SharedPubSub,
    pub(super) watch_registry: WatchRegistry,
    pub(super) state: Arc<ServerState>,
    /// Held when the run should look like a server with a browser attached:
    /// `tx.receiver_count() > 0` turns on push construction for every write.
    _sync_rx: Option<(broadcast::Receiver<SyncMsg>, PushReceiver)>,
}

impl Shared {
    pub(super) fn new(with_sync_receiver: bool) -> Shared {
        Shared::with_aof(with_sync_receiver, None)
    }

    pub(super) fn with_aof(with_sync_receiver: bool, aof: Option<Arc<AofWriter>>) -> Shared {
        let (tx, _) = broadcast::channel::<SyncMsg>(1024);
        let state = Arc::new(ServerState {
            snap: Arc::new(SnapshotConfig {
                path: std::env::temp_dir()
                    .join(format!("recached_request_path_{}.rdb", std::process::id())),
                last_save: AtomicI64::new(now_unix_secs()),
                checkpoint_id: AtomicU64::new(0),
            }),
            aof,
            replicas: ReplHub::new(),
            is_replica: AtomicBool::new(false),
            dedup: std::sync::Mutex::new(HashMap::new()),
            ephemeral: std::sync::Mutex::new(HashMap::new()),
            ephemeral_members: std::sync::Mutex::new(HashMap::new()),
            ephemeral_in_use: AtomicBool::new(false),
            dedup_dirty: AtomicBool::new(false),
            dedup_order: tokio::sync::Mutex::new(()),
            save_lock: tokio::sync::Mutex::new(()),
            persistence_healthy: AtomicBool::new(true),
            persistence_failures: AtomicU64::new(0),
        });
        let mut shared = Shared {
            store: Arc::new(KeyValueStore::new()),
            tx,
            pubsub: Arc::new(tokio::sync::Mutex::new(PubSubHub::new())),
            watch_registry: WatchHub::new(),
            state,
            _sync_rx: None,
        };
        if with_sync_receiver {
            // Registered the way a WebSocket connection registers, so writes
            // take the ordered path and build their pushes. Only the
            // benchmark asks for this, outside any runtime.
            let rt = tokio::runtime::Builder::new_current_thread()
                .build()
                .expect("runtime");
            shared._sync_rx = Some(rt.block_on(shared.state.replicas.subscribe_pushes(&shared.tx)));
        }
        shared
    }

    pub(super) fn serve<S>(
        &self,
        socket: S,
    ) -> impl std::future::Future<Output = ()> + Send + use<S>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        handle_tcp(
            socket,
            Arc::clone(&self.store),
            self.tx.clone(),
            Arc::new(None),
            Arc::clone(&self.pubsub),
            Arc::clone(&self.watch_registry),
            Arc::clone(&self.state),
            "bench".to_string(),
        )
    }
}

impl Shared {
    /// A WebSocket connection to the same server state, open sync policy.
    pub(super) fn serve_ws<S>(
        &self,
        socket: S,
    ) -> impl std::future::Future<Output = ()> + Send + use<S>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        handle_ws(
            socket,
            Arc::clone(&self.store),
            self.tx.clone(),
            Arc::new(None),
            next_conn_id(),
            Arc::clone(&self.pubsub),
            Arc::clone(&self.watch_registry),
            Arc::clone(&self.state),
            Arc::new(SyncPolicy::open()),
            Arc::new(None),
            "bench-ws".to_string(),
        )
    }
}

pub(super) struct Run {
    pub elapsed: Duration,
    pub allocs: u64,
    pub bytes: u64,
    pub replies: Vec<u8>,
}

/// Stream `frames` through one connection and collect every reply.
///
/// The server runs on a thread of its own with a current-thread runtime, so
/// the thread-local allocation counters see the server and nothing else; the
/// client writes and reads concurrently on this thread so neither side stalls
/// on the pipe's bounded buffer.
pub(super) fn run_connection(shared: &Shared, frames: Vec<u8>) -> Run {
    let (client, server) = tokio::io::duplex(256 * 1024);
    let fut = shared.serve(server);
    let server_thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        ALLOCS.with(|n| n.set(0));
        BYTES.with(|b| b.set(0));
        let started = Instant::now();
        TRACKING.with(|t| t.set(true));
        rt.block_on(fut);
        TRACKING.with(|t| t.set(false));
        let elapsed = started.elapsed();
        (elapsed, ALLOCS.with(Cell::get), BYTES.with(Cell::get))
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let replies = rt.block_on(async move {
        let (mut rd, mut wr) = tokio::io::split(client);
        let writer = async move {
            for chunk in frames.chunks(16 * 1024) {
                wr.write_all(chunk).await.expect("write");
            }
            wr.shutdown().await.expect("shutdown");
            // Keep the write half alive until the reader is done: dropping
            // it would not close the pipe (the read half shares it), but be
            // explicit about the lifetime anyway.
            wr
        };
        let reader = async move {
            let mut out = Vec::new();
            rd.read_to_end(&mut out).await.expect("read");
            out
        };
        let (_wr, out) = tokio::join!(writer, reader);
        out
    });
    let (elapsed, allocs, bytes) = server_thread.join().expect("server thread");
    Run {
        elapsed,
        allocs,
        bytes,
        replies,
    }
}

/// Every reply in `bytes`, asserting there are `n` and none is an error.
fn check_replies(workload: Workload, bytes: &[u8], n: usize) {
    let mut pos = 0;
    let mut count = 0;
    while pos < bytes.len() {
        let (v, used) = Value::parse(&bytes[pos..]).expect("well-formed reply");
        assert!(
            !matches!(v, Value::Error(_)),
            "{}: reply {count} is an error: {v:?}",
            workload.name()
        );
        pos += used;
        count += 1;
    }
    assert_eq!(count, n, "{}: reply count", workload.name());
}

fn measure(workload: Workload, n: usize, with_sync_receiver: bool) -> Run {
    let shared = Shared::new(with_sync_receiver);
    prefill(&shared.store);
    let run = run_connection(&shared, workload.frames(n));
    check_replies(workload, &run.replies, n);
    run
}

// ── allocation regression tests ───────────────────────────────────────────────

/// Allocations per command on the standalone write and read paths.
///
/// Bounds are the measured value after Phase 6H plus a little slack, so a
/// change that adds a per-command allocation (a clone, a `format!`, a boxed
/// future) fails here instead of hiding in benchmark noise.
#[test]
fn pipelined_commands_stay_within_their_allocation_budget() {
    const N: usize = 20_000;
    for (workload, budget) in allocation_budgets() {
        let run = measure(workload, N, false);
        let per = run.allocs as f64 / N as f64;
        assert!(
            per <= budget,
            "{}: {per:.2} allocations per command, budget {budget}",
            workload.name()
        );
    }
}

/// Per-command allocation ceilings asserted by the test above.
pub(super) fn allocation_budgets() -> Vec<(Workload, f64)> {
    vec![
        (Workload::Set, 16.0),
        (Workload::Get, 16.0),
        (Workload::Incr, 16.0),
        (Workload::HSet, 16.0),
        (Workload::HGet, 16.0),
    ]
}

// ── AOF-gated replies ─────────────────────────────────────────────────────────

/// With the AOF on, a pipelined write's wait for its `write(2)` is deferred
/// to the moment its connection next sends anything — but nothing may reach
/// the client before the AOF writes of the commands it answers. Whenever the
/// client has `k` SET replies, the file must already hold `k` SET frames:
/// whole or fragmented input, small replies batched in the buffered writer
/// and a large one written straight through it.
#[test]
fn no_reply_precedes_the_aof_write_of_its_command() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    for chunk in [0usize, 5] {
        let path = std::env::temp_dir().join(format!(
            "recached_gated_aof_{}_{chunk}.aof",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        rt.block_on(async {
            let aof = Arc::new(AofWriter::open(path.clone(), AofSync::EverySec).await.unwrap());
            let shared = Shared::with_aof(false, Some(Arc::clone(&aof)));
            let mut frames = Vec::new();
            encode(&[b"SET", b"big", &vec![b'B'; 100_000]], &mut frames);
            for i in 0..300 {
                encode(&[b"SET", format!("k{i}").as_bytes(), b"v"], &mut frames);
                if i == 150 {
                    encode(&[b"GET", b"big"], &mut frames);
                }
            }
            let (client, server) = tokio::io::duplex(1 << 20);
            let server = tokio::spawn(shared.serve(server));
            let (mut rd, mut wr) = tokio::io::split(client);
            let writer = tokio::spawn(async move {
                if chunk == 0 {
                    wr.write_all(&frames).await.unwrap();
                } else {
                    for part in frames.chunks(chunk * 1000) {
                        wr.write_all(part).await.unwrap();
                        tokio::task::yield_now().await;
                    }
                }
                wr
            });
            let mut got = Vec::new();
            let mut buf = vec![0u8; 64 * 1024];
            let mut oks = 0;
            while oks < 301 {
                let n = rd.read(&mut buf).await.unwrap();
                assert!(n > 0, "server closed early");
                got.extend_from_slice(&buf[..n]);
                oks = got.windows(5).filter(|w| w == b"+OK\r\n").count();
                let on_disk = std::fs::read(&path).unwrap_or_default();
                let logged = on_disk.windows(7).filter(|w| w == b"$3\r\nSET").count();
                assert!(
                    logged >= oks,
                    "chunk {chunk}: {oks} SET replies received but only {logged} SETs in the AOF ({} bytes: {:?})",
                    on_disk.len(),
                    String::from_utf8_lossy(&on_disk[..on_disk.len().min(80)])
                );
            }
            drop(writer.await.unwrap());
            drop(rd);
            let _ = server.await;
        });
        let _ = std::fs::remove_file(&path);
    }
}

// ── A/B benchmark ─────────────────────────────────────────────────────────────

#[test]
#[ignore = "benchmark: run explicitly in release mode"]
fn request_path_bench() {
    const N: usize = 200_000;
    let reps: usize = std::env::var("BENCH_REPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7);
    let label = std::env::var("BENCH_LABEL").unwrap_or_else(|_| "this".to_string());
    let sync = std::env::var("BENCH_SYNC").is_ok_and(|v| v == "1");
    let only = std::env::var("BENCH_ONLY").ok();
    let mut csv = String::new();
    println!(
        "{:<14} {:>10} {:>10} {:>10} {:>11}   (label {label}, sync receiver {sync}, {reps} reps x {N})",
        "workload", "min ns", "median ns", "allocs", "bytes"
    );
    for workload in Workload::ALL {
        if let Some(only) = &only
            && !only
                .split(',')
                .any(|w| w.eq_ignore_ascii_case(workload.name()))
        {
            continue;
        }
        let mut ns: Vec<f64> = Vec::with_capacity(reps);
        let mut allocs = 0.0;
        let mut bytes = 0.0;
        for _ in 0..reps {
            let run = measure(workload, N, sync);
            ns.push(run.elapsed.as_nanos() as f64 / N as f64);
            allocs = run.allocs as f64 / N as f64;
            bytes = run.bytes as f64 / N as f64;
        }
        ns.sort_by(f64::total_cmp);
        let (min, median) = (ns[0], ns[reps / 2]);
        println!(
            "{:<14} {min:>10.1} {median:>10.1} {allocs:>10.2} {bytes:>11.1}",
            workload.name()
        );
        csv.push_str(&format!(
            "{label},{},{sync},{min:.1},{median:.1},{allocs:.2},{bytes:.1},{}\n",
            workload.name(),
            ns.iter()
                .map(|v| format!("{v:.1}"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    if let Ok(path) = std::env::var("BENCH_OUT") {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("BENCH_OUT");
        f.write_all(csv.as_bytes()).expect("write BENCH_OUT");
    }
}

/// Hot-key scaling: `conns` connections each streaming `per_conn` INCRs of one
/// key into a multi-thread runtime with `workers` workers. Reports aggregate
/// ns per command (lower is better) over the whole run.
fn hot_key_run(
    workers: usize,
    conns: usize,
    per_conn: usize,
    frames_for: &dyn Fn() -> Vec<u8>,
) -> Duration {
    let shared = Shared::new(false);
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
        .expect("runtime");
    let client_rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("client runtime");
    let mut pairs = Vec::new();
    for _ in 0..conns {
        let (client, server) = tokio::io::duplex(256 * 1024);
        pairs.push((client, server, frames_for()));
    }
    let started = Instant::now();
    let mut servers = Vec::new();
    let mut clients = Vec::new();
    for (client, server, frames) in pairs {
        servers.push(rt.spawn(shared.serve(server)));
        clients.push(client_rt.spawn(async move {
            let (mut rd, mut wr) = tokio::io::split(client);
            let writer = async move {
                for chunk in frames.chunks(16 * 1024) {
                    wr.write_all(chunk).await.expect("write");
                }
                wr.shutdown().await.expect("shutdown");
                wr
            };
            let reader = async move {
                let mut out = Vec::new();
                rd.read_to_end(&mut out).await.expect("read");
                out
            };
            let (_w, out) = tokio::join!(writer, reader);
            out
        }));
    }
    let outs: Vec<Vec<u8>> = client_rt.block_on(async {
        let mut v = Vec::new();
        for c in clients {
            v.push(c.await.expect("client"));
        }
        v
    });
    let elapsed = started.elapsed();
    rt.block_on(async {
        for s in servers {
            s.await.expect("server");
        }
    });
    let total: usize = outs
        .iter()
        .map(|o| o.iter().filter(|&&b| b == b'\n').count())
        .sum();
    assert_eq!(total, conns * per_conn, "every INCR answered");
    let _ = per_conn;
    elapsed
}

#[test]
#[ignore = "benchmark: run explicitly in release mode"]
fn hot_key_scaling_bench() {
    let per_conn: usize = 25_000;
    let conns = 16;
    let reps: usize = std::env::var("BENCH_REPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let label = std::env::var("BENCH_LABEL").unwrap_or_else(|_| "this".to_string());
    let mut csv = String::new();
    for (shape, frames_for) in [
        (
            "hot INCR",
            Box::new(move || Workload::HotIncr.frames(per_conn)) as Box<dyn Fn() -> Vec<u8>>,
        ),
        (
            "uniform INCR",
            Box::new(move || Workload::Incr.frames(per_conn)) as Box<dyn Fn() -> Vec<u8>>,
        ),
    ] {
        for workers in [1usize, 2, 4] {
            let mut rates: Vec<f64> = (0..reps)
                .map(|_| {
                    let d = hot_key_run(workers, conns, per_conn, &*frames_for);
                    (conns * per_conn) as f64 / d.as_secs_f64()
                })
                .collect();
            rates.sort_by(f64::total_cmp);
            let (max, median) = (rates[reps - 1], rates[reps / 2]);
            println!(
                "{shape:<13} workers {workers}: best {max:>10.0} ops/s  median {median:>10.0} ops/s  ({label})"
            );
            csv.push_str(&format!(
                "{label},{shape},{workers},{max:.0},{median:.0},{}\n",
                rates
                    .iter()
                    .map(|v| format!("{v:.0}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
    }
    if let Ok(path) = std::env::var("BENCH_OUT_HOT") {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("BENCH_OUT_HOT");
        f.write_all(csv.as_bytes()).expect("write BENCH_OUT_HOT");
    }
}
