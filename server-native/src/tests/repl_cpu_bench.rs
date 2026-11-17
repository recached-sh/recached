//! Replicated-write CPU, in process (v0.6 Phase 6L-D).
//!
//! Drives the real write path (`execute_ordered_write`: ordering stripes,
//! store, AOF/replication log, fan-out) from several writer tasks on a
//! primary with two in-process replicas attached, each drained by a task, and
//! optionally `WS_SUBS` WebSocket subscribers routed everything, each doing
//! what a connection does per push: build its message (`push_message`). The Prometheus
//! recorder is installed, as in production, so per-write metric updates are
//! paid for.
//!
//! ```text
//! cargo test --release -p recached --bin recached-server bench_replicated_set_cpu -- --ignored --nocapture
//! ```
//!
//! Prints CSV: ws, writers, ops/s, process CPU ns/op. CPU is the whole
//! process (writers, runtime, drains), read from /proc/self/stat on Linux or
//! `ps` elsewhere (10 ms resolution; each run lasts seconds).

use super::*;
use crate::sync_router::Routes;
use std::sync::OnceLock;

fn install_recorder() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let _ = metrics_exporter_prometheus::PrometheusBuilder::new().install_recorder();
    });
}

/// User + system CPU seconds of this process.
fn cpu_seconds() -> f64 {
    if let Ok(stat) = std::fs::read_to_string("/proc/self/stat") {
        // Fields after the parenthesised command name; utime and stime are the
        // 12th and 13th of those, in clock ticks (100/s on Linux).
        let rest = stat.rsplit_once(')').map_or("", |(_, r)| r);
        let fields: Vec<&str> = rest.split_whitespace().collect();
        let ticks = |i: usize| {
            fields
                .get(i)
                .and_then(|f| f.parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        return (ticks(11) + ticks(12)) / 100.0;
    }
    let out = std::process::Command::new("ps")
        .args(["-o", "time=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps");
    let text = String::from_utf8_lossy(&out.stdout);
    // "[[hh:]mm:]ss.cc"
    text.trim().split(':').fold(0.0, |acc, part| {
        acc * 60.0 + part.trim().parse::<f64>().unwrap_or(0.0)
    })
}

async fn run_once(writers: usize, ws: bool, ops_per_writer: usize) -> (f64, f64, u64) {
    let hub = ReplHub::new();
    hub.configure_backlog(16 << 20);
    let state = ServerState::for_tests(
        Arc::new(SnapshotConfig {
            path: tmp_path("repl_cpu_bench.rdb"),
            last_save: AtomicI64::new(0),
            checkpoint_id: AtomicU64::new(0),
        }),
        hub,
        false,
    );
    let store = Arc::new(KeyValueStore::new());
    let tx = SyncRouter::new(1 << 16);
    let watches: WatchRegistry = WatchHub::new();
    let mut drains = Vec::new();
    for _ in 0..2 {
        let mut rx = state
            .replicas
            .attach_test_replica(1 << 16, usize::MAX >> 1)
            .await;
        drains.push(tokio::spawn(async move {
            let mut n = 0u64;
            while let Some(frame) = rx.recv().await {
                drop(frame);
                n += 1;
            }
            n
        }));
    }
    let gaps = Arc::new(AtomicU64::new(0));
    let ws_subs: usize = if ws {
        std::env::var("WS_SUBS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8)
    } else {
        0
    };
    let ws_drains: Vec<_> = (0..ws_subs)
        .map(|_| {
            let mut sub = tx.subscribe(Routes::Everything);
            let gaps = Arc::clone(&gaps);
            tokio::spawn(async move {
                loop {
                    match sub.recv().await {
                        Ok(push) => drop(std::hint::black_box(push_message(&push))),
                        Err(_) => {
                            gaps.fetch_add(1, Ordering::Relaxed);
                            break;
                        }
                    }
                }
            })
        })
        .collect();
    let value = "v".repeat(32);
    let cpu0 = cpu_seconds();
    let t0 = std::time::Instant::now();
    let tasks: Vec<_> = (0..writers)
        .map(|w| {
            let (state, store, tx, watches, value) = (
                Arc::clone(&state),
                Arc::clone(&store),
                tx.clone(),
                Arc::clone(&watches),
                value.clone(),
            );
            tokio::spawn(async move {
                for i in 0..ops_per_writer {
                    let cmd = Command::Set(
                        format!("k{w}:{}", i % 10_000),
                        value.clone().into(),
                        SetOptions::default(),
                    );
                    execute_ordered_write(&cmd, &tx, w as u64 + 1, &state, &watches, &store).await;
                }
            })
        })
        .collect();
    for t in tasks {
        t.await.expect("writer");
    }
    let elapsed = t0.elapsed().as_secs_f64();
    let cpu = cpu_seconds() - cpu0;
    let ops = (writers * ops_per_writer) as f64;
    for d in ws_drains {
        d.abort();
    }
    for d in drains {
        d.abort();
    }
    (ops / elapsed, cpu * 1e9 / ops, gaps.load(Ordering::Relaxed))
}

#[test]
#[ignore = "benchmark; run explicitly with --ignored --nocapture"]
fn bench_replicated_set_cpu() {
    install_recorder();
    let reps: usize = std::env::var("REPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let ops: usize = std::env::var("OPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400_000);
    println!("ws,writers,ops_s,cpu_ns_per_op,ws_gaps,reps");
    for ws in [false, true] {
        for writers in [1usize, 4, 8] {
            let mut rows = Vec::new();
            for _ in 0..reps {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(4)
                    .enable_all()
                    .build()
                    .expect("runtime");
                rows.push(rt.block_on(run_once(writers, ws, ops / writers)));
            }
            rows.sort_by(|a, b| a.0.total_cmp(&b.0));
            let (rate, _, _) = rows[rows.len() / 2];
            let mut cpu: Vec<f64> = rows.iter().map(|r| r.1).collect();
            cpu.sort_by(f64::total_cmp);
            let gaps: u64 = rows.iter().map(|r| r.2).sum();
            println!(
                "{ws},{writers},{rate:.0},{:.0},{gaps},{reps}",
                cpu[cpu.len() / 2]
            );
        }
    }
}
