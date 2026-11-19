//! `appendfsync always` fail-stop, end to end (6N-0B, finding D1).
//!
//! A failed AOF write or fsync under `always` must never be answered as a
//! success: the server stops before the affected write is acknowledged and
//! a restart replays exactly what was. That ends a process, so each case
//! runs the real server in a child process: this test binary re-executed
//! for [`fail_stop_child`], which calls the server's own `run` with the
//! production fail-stop (`RECACHED_TEST_AOF_FAIL_STOP=exit`) and one
//! injected fault (`RECACHED_TEST_AOF_FAULT=<fault>@<marker>`: the first
//! AOF batch containing `<marker>` suffers it). Both variables exist only in
//! test builds.
//!
//! The parent talks RESP over a plain socket and observes each reply
//! itself, so "no successful acknowledgement" is checked on the wire, not
//! inferred from timing.

use crate::persistence::AOF_FAIL_STOP_EXIT_CODE;
use core_engine::resp::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CHILD: &str = "RECACHED_FAIL_STOP_CHILD";
const CHILD_TEST: &str = "tests::aof_fail_stop::fail_stop_child";

/// The child: a real server, configured entirely from the environment.
#[test]
#[ignore = "the subprocess harness's child; its parent tests run it"]
fn fail_stop_child() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    let workers: usize = std::env::var("RECACHED_FAIL_STOP_WORKERS")
        .ok()
        .and_then(|w| w.parse().ok())
        .unwrap_or(1);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
        .expect("child runtime");
    let result = runtime.block_on(crate::run(workers, true));
    std::process::exit(i32::from(result.is_err()));
}

struct Server {
    child: std::process::Child,
    addr: String,
    log: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Server {
    fn log_tail(&self) -> String {
        let log = std::fs::read_to_string(&self.log).unwrap_or_default();
        let lines: Vec<&str> = log.lines().collect();
        lines[lines.len().saturating_sub(30)..].join("\n")
    }

    /// The child's exit status, waiting up to `limit`.
    fn exit_code(&mut self, limit: Duration) -> Option<i32> {
        let deadline = Instant::now() + limit;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.code();
            }
            if Instant::now() > deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn expect_fail_stop(&mut self) {
        let code = self.exit_code(Duration::from_secs(20));
        assert_eq!(
            code,
            Some(AOF_FAIL_STOP_EXIT_CODE),
            "expected the AOF fail-stop exit; log:\n{}",
            self.log_tail()
        );
        let log = std::fs::read_to_string(&self.log).unwrap_or_default();
        assert!(
            log.contains("fatal: AOF") && log.contains("appendfsync always"),
            "the fail-stop must say why; log:\n{log}"
        );
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("recached-failstop-{name}-{nanos:x}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[derive(Clone, Copy)]
struct Opts<'a> {
    sync: &'a str,
    fault: Option<&'a str>,
    workers: usize,
    replication: bool,
    /// `RECACHED_TEST_AOF_HOLD=<ms>@<marker>`: hold the batch performer on
    /// the first batch holding `<marker>` for `<ms>` before its write.
    hold: Option<&'a str>,
}

const ALWAYS: Opts<'static> = Opts {
    sync: "always",
    fault: None,
    workers: 1,
    replication: false,
    hold: None,
};

fn start(dir: &Path, opts: Opts<'_>) -> Server {
    // A port picked free can be taken before the child binds it; retry.
    for attempt in 0..3 {
        let (port, ws) = (free_port(), free_port());
        let log = dir.join(format!("server-{port}.log"));
        let out = std::fs::File::create(&log).unwrap();
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args([CHILD_TEST, "--exact", "--ignored", "--nocapture"])
            .args(["--test-threads", "1"])
            .current_dir(dir)
            .env_clear()
            .env(CHILD, "1")
            .env("RECACHED_FAIL_STOP_WORKERS", opts.workers.to_string())
            .env("RECACHED_TEST_AOF_FAIL_STOP", "exit")
            .env("RECACHED_BIND", "127.0.0.1")
            .env("RECACHED_PORT", port.to_string())
            .env("RECACHED_WS_PORT", ws.to_string())
            .env("RECACHED_METRICS_PORT", "0")
            .env("RECACHED_SAVE_PATH", dir.join("dump.rdb"))
            .env("RECACHED_AOF_PATH", dir.join("append.aof"))
            .env("RECACHED_AOF_SYNC", opts.sync)
            .env("RECACHED_SAVE_INTERVAL", "0")
            .env("RUST_LOG", "info")
            .stdout(Stdio::from(out.try_clone().unwrap()))
            .stderr(Stdio::from(out));
        if let Some(fault) = opts.fault {
            cmd.env("RECACHED_TEST_AOF_FAULT", fault);
        }
        // The suite's own mode, when it is run in one (see `AofFanout`).
        if let Ok(mode) = std::env::var("RECACHED_AOF_FANOUT") {
            cmd.env("RECACHED_AOF_FANOUT", mode);
        }
        if let Some(hold) = opts.hold {
            cmd.env("RECACHED_TEST_AOF_HOLD", hold);
        }
        if opts.replication {
            cmd.env("RECACHED_REPL_ENABLE", "1")
                .env("RECACHED_REPL_PORT", free_port().to_string());
        }
        let mut server = Server {
            child: cmd.spawn().expect("spawn the child server"),
            addr: format!("127.0.0.1:{port}"),
            log,
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if server.child.try_wait().unwrap().is_some() {
                break; // exited before ready: retry on new ports
            }
            if let Ok(mut c) = TcpStream::connect(&server.addr) {
                c.set_read_timeout(Some(Duration::from_millis(500))).ok();
                if c.write_all(b"*1\r\n$4\r\nPING\r\n").is_ok()
                    && matches!(c.read(&mut [0u8; 16]), Ok(n) if n > 0)
                {
                    return server;
                }
            }
            assert!(
                Instant::now() < deadline,
                "child server did not start; log:\n{}",
                server.log_tail()
            );
            std::thread::sleep(Duration::from_millis(25));
        }
        eprintln!(
            "child exited before ready (attempt {attempt}); log:\n{}",
            server.log_tail()
        );
    }
    panic!("child server failed to start three times");
}

fn frame(args: &[&str]) -> Vec<u8> {
    let mut req = format!("*{}\r\n", args.len()).into_bytes();
    for a in args {
        req.extend_from_slice(format!("${}\r\n{a}\r\n", a.len()).as_bytes());
    }
    req
}

/// One connection, reading replies as they come.
struct Conn {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl Conn {
    fn open(addr: &str) -> Self {
        let stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        Conn {
            stream,
            buf: Vec::new(),
        }
    }

    fn send(&mut self, args: &[&str]) {
        self.stream.write_all(&frame(args)).unwrap();
    }

    /// The next reply, or `None` once the server closed or reset the
    /// connection. A read timeout is a test failure: the server neither
    /// answered nor went away.
    fn reply(&mut self) -> Option<Value> {
        loop {
            match Value::parse(&self.buf) {
                Ok((v, used)) => {
                    self.buf.drain(..used);
                    return Some(v);
                }
                Err(e) if e.is_incomplete() => {}
                Err(e) => panic!("bad reply: {e}"),
            }
            let mut chunk = [0u8; 65536];
            match self.stream.read(&mut chunk) {
                Ok(0) => return None,
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::ConnectionReset
                            | std::io::ErrorKind::ConnectionAborted
                            | std::io::ErrorKind::BrokenPipe
                    ) =>
                {
                    return None;
                }
                Err(e) => panic!("no reply and no close: {e}"),
            }
        }
    }

    fn cmd(&mut self, args: &[&str]) -> Option<Value> {
        self.send(args);
        self.reply()
    }
}

fn ok() -> Value {
    Value::SimpleString("OK".into())
}

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

fn get(addr: &str, key: &str) -> Value {
    Conn::open(addr).cmd(&["GET", key]).expect("GET answered")
}

/// The core case for each fault: an acknowledged write before it, the
/// faulted write never acknowledged, the fail-stop exit, and a restart that
/// replays the acknowledged write. Returns the faulted key's value after
/// the restart, which depends on whether its frame reached the file.
fn fault_stops_before_ack(name: &str, fault: &str, opts: Opts<'_>) -> Value {
    let dir = scratch(name);
    let fault = format!("{fault}@boom");
    let mut server = start(
        &dir,
        Opts {
            fault: Some(&fault),
            ..opts
        },
    );
    let mut c = Conn::open(&server.addr);
    assert_eq!(c.cmd(&["SET", "a", "1"]), Some(ok()));
    let reply = c.cmd(&["SET", "boom", "1"]);
    assert_eq!(
        reply,
        None,
        "{name}: a write whose AOF {fault} failed under always was answered; log:\n{}",
        server.log_tail()
    );
    server.expect_fail_stop();
    drop(server);

    let server = start(&dir, ALWAYS);
    assert_eq!(
        get(&server.addr, "a"),
        bulk("1"),
        "{name}: acked write lost"
    );
    let boom = get(&server.addr, "boom");
    // The AOF still takes appends after recovery.
    let mut c = Conn::open(&server.addr);
    assert_eq!(c.cmd(&["SET", "after", "1"]), Some(ok()));
    drop(server);
    let server = start(&dir, ALWAYS);
    assert_eq!(get(&server.addr, "after"), bulk("1"));
    let _ = std::fs::remove_dir_all(&dir);
    boom
}

#[test]
fn always_write_failing_before_any_byte_stops_without_acknowledging() {
    let boom = fault_stops_before_ack("enospc", "fail:enospc", ALWAYS);
    assert_eq!(
        boom,
        Value::BulkString(None),
        "no byte of it reached the file"
    );
}

#[test]
fn always_torn_write_stops_and_the_file_replays() {
    // 9 bytes of the frame land, then ENOSPC: the cut-back leaves the file
    // on a whole frame, so the restart replays it.
    let boom = fault_stops_before_ack("torn", "torn:9:enospc", ALWAYS);
    assert_eq!(boom, Value::BulkString(None), "the torn frame was cut");
}

#[test]
fn always_fsync_failure_stops_without_acknowledging() {
    // The frame reached the file, so after the restart the write may
    // replay: its outcome is unknown to the client, which never got an
    // answer. What must not happen is an answer.
    let boom = fault_stops_before_ack("fsync", "fsync:eio", ALWAYS);
    assert!(
        boom == bulk("1") || boom == Value::BulkString(None),
        "{boom:?}"
    );
}

#[test]
fn always_failure_with_replication_enabled_stops_without_acknowledging() {
    // Replication routes writes through the non-deferred append, which
    // the connection gate does not cover: the fail-stop has to.
    let boom = fault_stops_before_ack(
        "repl",
        "fail:eio",
        Opts {
            replication: true,
            ..ALWAYS
        },
    );
    assert_eq!(boom, Value::BulkString(None));
}

#[test]
fn always_short_write_is_completed_and_acknowledged() {
    // A short write(2) is not a failure: the rest is written and the
    // write is acknowledged and durable.
    let dir = scratch("short");
    let server = start(
        &dir,
        Opts {
            fault: Some("short:3@boom"),
            ..ALWAYS
        },
    );
    let mut c = Conn::open(&server.addr);
    assert_eq!(c.cmd(&["SET", "boom", "1"]), Some(ok()));
    drop(server); // SIGKILL
    let server = start(&dir, ALWAYS);
    assert_eq!(get(&server.addr, "boom"), bulk("1"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn everysec_failure_keeps_its_contract_and_does_not_stop() {
    // Unchanged by the fail-stop: under everysec the failed write's reply
    // is the normal one (it never promised an fsync), persistence latches
    // unhealthy, and later writes fail closed until a SAVE succeeds.
    let dir = scratch("everysec");
    let mut server = start(
        &dir,
        Opts {
            sync: "everysec",
            fault: Some("fail:enospc@boom"),
            ..ALWAYS
        },
    );
    let mut c = Conn::open(&server.addr);
    assert_eq!(c.cmd(&["SET", "boom", "1"]), Some(ok()));
    let refused = c.cmd(&["SET", "next", "1"]).expect("answered");
    assert!(
        matches!(&refused, Value::Error(e) if e.starts_with("MISCONF")),
        "{refused:?}"
    );
    assert_eq!(server.exit_code(Duration::from_millis(300)), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn always_failure_during_a_checkpoint_stops_and_keeps_acked_writes() {
    let dir = scratch("checkpoint");
    let mut server = start(
        &dir,
        Opts {
            fault: Some("fail:eio@RECACHED-CHECKPOINT"),
            ..ALWAYS
        },
    );
    let mut c = Conn::open(&server.addr);
    assert_eq!(c.cmd(&["SET", "a", "1"]), Some(ok()));
    assert_eq!(c.cmd(&["SAVE"]), None, "SAVE must not report success");
    server.expect_fail_stop();
    drop(server);
    let server = start(&dir, ALWAYS);
    assert_eq!(get(&server.addr, "a"), bulk("1"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn always_failure_in_the_shutdown_checkpoint_exits_as_a_fail_stop() {
    // SIGTERM saves a final checkpoint. When its AOF write fails, the
    // process must not exit 0 as after a clean shutdown.
    let dir = scratch("shutdown");
    let mut server = start(
        &dir,
        Opts {
            fault: Some("fail:eio@RECACHED-CHECKPOINT"),
            ..ALWAYS
        },
    );
    let mut c = Conn::open(&server.addr);
    assert_eq!(c.cmd(&["SET", "a", "1"]), Some(ok()));
    let status = Command::new("kill")
        .args(["-TERM", &server.child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    server.expect_fail_stop();
    drop(server);
    let server = start(&dir, ALWAYS);
    assert_eq!(get(&server.addr, "a"), bulk("1"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dedup_retry_after_a_fail_stop_is_applied_exactly_once() {
    // Whether or not the failed frame reached the file, the client's retry
    // of the unanswered write must leave the counter at one.
    for fault in ["fail:enospc", "fsync:eio"] {
        let dir = scratch("dedup");
        let spec = format!("{fault}@boomctr");
        let mut server = start(
            &dir,
            Opts {
                fault: Some(&spec),
                ..ALWAYS
            },
        );
        let mut c = Conn::open(&server.addr);
        let write = ["DEDUP", "client-failstop-1", "1", "INCR", "boomctr"];
        assert_eq!(c.cmd(&write), None, "{fault}: answered");
        server.expect_fail_stop();
        drop(server);

        let server = start(&dir, ALWAYS);
        let mut c = Conn::open(&server.addr);
        let retry = c.cmd(&write).expect("retry answered");
        assert!(
            retry == Value::Integer(1) || retry == Value::SimpleString("DUP".into()),
            "{fault}: {retry:?}"
        );
        assert_eq!(get(&server.addr, "boomctr"), bulk("1"), "{fault}");
        drop(server);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// 6N-C PR 0a (finding O1): a replay of a DEDUP write on another connection,
/// sent while the original's batch is still being made durable, must not be
/// answered `+DUP` — that batch then fails, and the process stops with the
/// write in neither memory nor the file. The batch is held for long enough
/// that the replay surely arrives inside the window; before the fix it was
/// answered at once from the mark the deferred original had committed.
#[test]
fn a_dedup_replay_during_a_failing_always_batch_is_never_answered() {
    for fault in ["fsync:eio", "fail:eio"] {
        let dir = scratch("dedupreplay");
        let spec = format!("{fault}@boomdup");
        let mut server = start(
            &dir,
            Opts {
                fault: Some(&spec),
                hold: Some("800@boomdup"),
                workers: 2,
                ..ALWAYS
            },
        );
        let write = ["DEDUP", "client-replay-0a", "1", "INCR", "boomdup"];
        let mut first = Conn::open(&server.addr);
        first.send(&write);
        std::thread::sleep(Duration::from_millis(200));
        let mut second = Conn::open(&server.addr);
        let replay = second.cmd(&write);
        assert_eq!(
            replay,
            None,
            "{fault}: the replay was answered while the original's batch was failing; log:\n{}",
            server.log_tail()
        );
        assert_eq!(first.reply(), None, "{fault}: the original was answered");
        server.expect_fail_stop();
        drop(server);

        // Nobody was answered, so either outcome is allowed; the retry
        // resolves it to exactly one application.
        let server = start(&dir, ALWAYS);
        let mut c = Conn::open(&server.addr);
        let retry = c.cmd(&write).expect("retry answered");
        assert!(
            retry == Value::Integer(1) || retry == Value::SimpleString("DUP".into()),
            "{fault}: {retry:?}"
        );
        assert_eq!(get(&server.addr, "boomdup"), bulk("1"), "{fault}");
        drop(server);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Many clients on four workers share AOF batches; one batch fails. Every
/// write any client saw acknowledged must survive the restart, and the
/// write in the failed batch must not have been acknowledged.
#[test]
fn always_failure_in_a_shared_batch_acknowledges_only_durable_writes() {
    let dir = scratch("shared");
    let mut server = start(
        &dir,
        Opts {
            fault: Some("torn:20:eio@boom"),
            workers: 4,
            ..ALWAYS
        },
    );
    let addr = server.addr.clone();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let writers: Vec<_> = (0..16)
        .map(|c| {
            let addr = addr.clone();
            let stop = std::sync::Arc::clone(&stop);
            std::thread::spawn(move || {
                let mut conn = Conn::open(&addr);
                let mut acked = Vec::new();
                for round in 0..10_000u32 {
                    if stop.load(std::sync::atomic::Ordering::Relaxed) && round > 0 {
                        break;
                    }
                    // A pipeline of 8, so batches hold several clients'
                    // frames.
                    let keys: Vec<String> = (0..8).map(|i| format!("k:{c}:{round}:{i}")).collect();
                    let mut req = Vec::new();
                    for k in &keys {
                        req.extend_from_slice(&frame(&["SET", k, k]));
                    }
                    if conn.stream.write_all(&req).is_err() {
                        return acked;
                    }
                    for k in keys {
                        match conn.reply() {
                            Some(v) if v == ok() => acked.push(k),
                            Some(other) => panic!("unexpected reply {other:?}"),
                            None => return acked,
                        }
                    }
                }
                acked
            })
        })
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    let mut c = Conn::open(&addr);
    assert_eq!(c.cmd(&["SET", "boom", "x"]), None, "boom was acknowledged");
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    server.expect_fail_stop();
    let acked: Vec<String> = writers
        .into_iter()
        .flat_map(|w| w.join().unwrap())
        .collect();
    assert!(
        !acked.is_empty(),
        "the clients made no progress before the fault"
    );
    drop(server);

    let server = start(&dir, ALWAYS);
    let mut c = Conn::open(&server.addr);
    let mut lost = Vec::new();
    for chunk in acked.chunks(500) {
        let mut args = vec!["MGET"];
        args.extend(chunk.iter().map(String::as_str));
        let Some(Value::Array(Some(values))) = c.cmd(&args) else {
            panic!("MGET failed");
        };
        for (k, v) in chunk.iter().zip(values) {
            if v != bulk(k) {
                lost.push(k.clone());
            }
        }
    }
    assert!(
        lost.is_empty(),
        "{} of {} acknowledged writes lost, e.g. {:?}",
        lost.len(),
        acked.len(),
        &lost[..lost.len().min(5)]
    );
    assert_eq!(get(&server.addr, "boom"), Value::BulkString(None));
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}
