//! 6O-G: in-process cost of one EXEC (`run_transaction_queue`), single
//! thread, no network: the measurement behind the 6O-G phase-2 and stripe
//! optimisations. Runs only with `RECACHED_BENCH_6OG=1` (CI runs ignored
//! tests too): `RECACHED_BENCH_6OG=1 cargo test --release -p recached --bin
//! recached-server exec_cost_6og -- --ignored --nocapture`.
use super::request_path::Shared;
use crate::*;
use core_engine::resp::Value;

fn cmd(args: &[&str]) -> Command {
    Command::from_value(Value::Array(Some(
        args.iter()
            .map(|a| Value::BulkString(Some(a.as_bytes().to_vec())))
            .collect(),
    )))
    .unwrap()
}

#[test]
#[ignore]
fn exec_cost_6og() {
    if std::env::var_os("RECACHED_BENCH_6OG").is_none() {
        eprintln!("exec_cost_6og skipped: set RECACHED_BENCH_6OG=1 to run it");
        return;
    }
    let shared = Shared::new(false);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for i in 0..100_000 {
        shared.store.execute(cmd(&[
            "SET",
            &format!("key:{i:012}"),
            "vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv",
        ]));
    }
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut key = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        format!("key:{:012}", seed % 100_000)
    };
    let v = "v".repeat(32);
    type Shape = fn(&mut dyn FnMut() -> String, &str) -> Vec<Command>;
    let shapes: [(&str, Shape); 5] = [
        ("get-get", |k, _| {
            vec![cmd(&["GET", &k()]), cmd(&["GET", &k()])]
        }),
        ("mget-get", |k, _| {
            let keys: Vec<String> = (0..10).map(|_| k()).collect();
            let mut a = vec!["MGET"];
            a.extend(keys.iter().map(String::as_str));
            vec![cmd(&a), cmd(&["GET", &k()])]
        }),
        ("get-set", |k, v| {
            vec![cmd(&["GET", &k()]), cmd(&["SET", &k(), v])]
        }),
        ("set-set", |k, v| {
            vec![cmd(&["SET", &k(), v]), cmd(&["SET", &k(), v])]
        }),
        ("wide20", |k, v| {
            (0..20).map(|_| cmd(&["SET", &k(), v])).collect()
        }),
    ];
    const N: usize = 300_000;
    for round in 0..3 {
        for (name, shape) in &shapes {
            let queues: Vec<Vec<Command>> = (0..N).map(|_| shape(&mut key, &v)).collect();
            let started = std::time::Instant::now();
            rt.block_on(async {
                for queue in queues {
                    let out = run_transaction_queue(
                        queue,
                        core_engine::store::clock_ms(),
                        &shared.tx,
                        1,
                        &shared.state,
                        &shared.watch_registry,
                        &shared.store,
                        &shared.pubsub,
                    )
                    .await;
                    std::hint::black_box(out);
                }
            });
            let ns = started.elapsed().as_nanos() as f64 / N as f64;
            eprintln!("EXEC_COST round={round} shape={name} ns_per_exec={ns:.0}");
        }
    }
}

/// The same transactions through a whole TCP connection (`handle_tcp` over
/// an in-memory stream, one transaction in flight): `MULTI`, the queued
/// commands, `EXEC`, every reply parsed. Includes what `run_transaction_queue`
/// does not: queueing, `lock_transaction`, the WATCH checks, the reply.
#[test]
#[ignore]
fn exec_cost_conn_6og() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    if std::env::var_os("RECACHED_BENCH_6OG").is_none() {
        eprintln!("exec_cost_conn_6og skipped: set RECACHED_BENCH_6OG=1 to run it");
        return;
    }
    let shared = Shared::new(false);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for i in 0..100_000 {
        shared.store.execute(cmd(&[
            "SET",
            &format!("key:{i:012}"),
            "vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv",
        ]));
    }
    fn enc(out: &mut Vec<u8>, args: &[&str]) {
        out.extend_from_slice(format!("*{}\r\n", args.len()).as_bytes());
        for a in args {
            out.extend_from_slice(format!("${}\r\n{a}\r\n", a.len()).as_bytes());
        }
    }
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut key = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        format!("key:{:012}", seed % 100_000)
    };
    let v = "v".repeat(32);
    const N: usize = 200_000;
    for round in 0..3 {
        for (name, writes) in [("get-get", false), ("set-set", true)] {
            // Pre-encoded: MULTI, two commands, EXEC.
            let requests: Vec<Vec<u8>> = (0..N)
                .map(|_| {
                    let mut out = Vec::new();
                    enc(&mut out, &["MULTI"]);
                    for _ in 0..2 {
                        let k = key();
                        if writes {
                            enc(&mut out, &["SET", &k, &v]);
                        } else {
                            enc(&mut out, &["GET", &k]);
                        }
                    }
                    enc(&mut out, &["EXEC"]);
                    out
                })
                .collect();
            let (client, server) = tokio::io::duplex(1 << 20);
            let started = rt.block_on(async {
                tokio::spawn(shared.serve(server));
                let (mut r, mut w) = tokio::io::split(client);
                let mut buf: Vec<u8> = Vec::with_capacity(1 << 16);
                let mut chunk = vec![0u8; 1 << 16];
                let started = std::time::Instant::now();
                for request in &requests {
                    w.write_all(request).await.unwrap();
                    // OK, QUEUED, QUEUED, the EXEC array.
                    let mut replies = 0;
                    while replies < 4 {
                        match Value::parse(&buf) {
                            Ok((_, n)) => {
                                buf.drain(..n);
                                replies += 1;
                            }
                            Err(_) => {
                                let n = r.read(&mut chunk).await.unwrap();
                                assert!(n > 0);
                                buf.extend_from_slice(&chunk[..n]);
                            }
                        }
                    }
                }
                started
            });
            let ns = started.elapsed().as_nanos() as f64 / N as f64;
            eprintln!("EXEC_CONN round={round} shape={name} ns_per_exec={ns:.0}");
        }
    }
}
