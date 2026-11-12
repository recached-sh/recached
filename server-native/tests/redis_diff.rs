//! Differential compatibility: run the same command scripts against
//! `recached-server` and a real Redis, and compare every reply.
//!
//! Ignored by default because it needs a Redis: CI starts a pinned
//! `redis:7.2.5` container and runs
//!
//! ```text
//! RECACHED_DIFF_REDIS=127.0.0.1:6379 cargo test -p recached --test redis_diff -- --ignored
//! ```
//!
//! Locally: `redis-server --port 7799 --save '' &` and point the variable at
//! it. Only commands whose replies are deterministic for both servers are
//! compared exactly; TTL readings get a one-second (or 1.5 s for PTTL)
//! allowance. Deviations Recached keeps on purpose are listed in
//! `KNOWN_DEVIATIONS` with their reason — and nowhere else.

use core_engine::resp::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn start_recached() -> (Server, String) {
    let (port, ws) = (free_port(), free_port());
    let bin = std::env::var("RECACHED_TEST_SERVER_BIN")
        .unwrap_or_else(|_| env!("CARGO_BIN_EXE_recached-server").to_string());
    let dir = std::env::temp_dir();
    let child = Command::new(bin)
        .current_dir(&dir)
        .env_clear()
        .env("RECACHED_BIND", "127.0.0.1")
        .env("RECACHED_PORT", port.to_string())
        .env("RECACHED_WS_PORT", ws.to_string())
        .env("RECACHED_METRICS_PORT", "0")
        .env("RECACHED_SAVE_INTERVAL", "0")
        .env(
            "RECACHED_SAVE_PATH",
            dir.join(format!("redis-diff-{port}.rdb")),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start recached-server");
    let addr = format!("127.0.0.1:{port}");
    let deadline = Instant::now() + Duration::from_secs(20);
    while TcpStream::connect(&addr).is_err() {
        assert!(Instant::now() < deadline, "recached did not start");
        std::thread::sleep(Duration::from_millis(25));
    }
    (Server(child), addr)
}

struct Conn {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl Conn {
    fn open(addr: &str) -> Self {
        let stream = TcpStream::connect(addr).unwrap_or_else(|e| panic!("connect {addr}: {e}"));
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        Conn {
            stream,
            buf: Vec::new(),
        }
    }

    fn cmd(&mut self, args: &[&str]) -> Value {
        let mut req = format!("*{}\r\n", args.len()).into_bytes();
        for a in args {
            req.extend_from_slice(format!("${}\r\n{a}\r\n", a.len()).as_bytes());
        }
        self.stream.write_all(&req).unwrap();
        loop {
            match Value::parse(&self.buf) {
                Ok((value, used)) => {
                    self.buf.drain(..used);
                    return value;
                }
                Err(e) if e.is_incomplete() => {
                    let mut chunk = [0u8; 65536];
                    let n = self.stream.read(&mut chunk).expect("read reply");
                    assert!(n > 0, "server closed the connection");
                    self.buf.extend_from_slice(&chunk[..n]);
                }
                Err(e) => panic!("unparseable reply: {e}"),
            }
        }
    }
}

/// One script per case; each starts from an empty keyspace.
fn cases() -> Vec<(&'static str, Vec<Vec<&'static str>>)> {
    let c = |s: &'static str| s.split(' ').collect::<Vec<_>>();
    vec![
        (
            "getdel",
            vec![
                c("SET s hello"),
                c("GETDEL s"),
                c("GETDEL s"),
                c("EXISTS s"),
                c("HSET h f v"),
                c("GETDEL h"),
                c("EXISTS h"),
                c("GETDEL"),
                c("GETDEL a b"),
            ],
        ),
        (
            "getex",
            vec![
                c("SET s v"),
                c("GETEX s"),
                c("TTL s"),
                c("GETEX s EX 100"),
                c("TTL s"),
                c("GETEX s PX 50000"),
                c("PTTL s"),
                c("GETEX s PERSIST"),
                c("TTL s"),
                c("GETEX s PXAT 4102444800123"),
                c("PEXPIRETIME s"),
                c("GETEX s EXAT 4102444801"),
                c("EXPIRETIME s"),
                c("GETEX s px 0"),
                c("GETEX s ex -1"),
                c("GETEX s ex 9223372036854775807"),
                c("GETEX s ex 10 px 10"),
                c("GETEX s persist ex 5"),
                c("GETEX s bogus"),
                c("GETEX s ex"),
                c("GETEX s ex notnum"),
                c("GETEX s exat 1"),
                c("EXISTS s"),
                c("GETEX missing ex 5"),
                c("EXISTS missing"),
                c("HSET h f v"),
                c("GETEX h ex 5"),
                c("TTL h"),
                c("GETEX"),
            ],
        ),
        (
            "expiretime",
            vec![
                c("EXPIRETIME k"),
                c("PEXPIRETIME k"),
                c("SET k v"),
                c("EXPIRETIME k"),
                c("PEXPIREAT k 4102444800123"),
                c("EXPIRETIME k"),
                c("PEXPIRETIME k"),
                c("EXPIRETIME"),
                c("EXPIRETIME a b"),
            ],
        ),
        (
            "renamenx",
            vec![
                c("SET a 1"),
                c("SET b 2"),
                c("RENAMENX a b"),
                c("GET b"),
                c("RENAMENX a c"),
                c("GET c"),
                c("EXISTS a"),
                c("RENAMENX nosuch x"),
                c("RENAMENX c c"),
                c("PEXPIREAT c 4102444800000"),
                c("RENAMENX c d"),
                c("PEXPIRETIME d"),
                c("RENAMENX c"),
            ],
        ),
        (
            "copy",
            vec![
                c("SET a 1"),
                c("COPY a a"),
                c("COPY a d"),
                c("COPY a d"),
                c("COPY a d REPLACE"),
                c("COPY a e DB 0"),
                c("COPY a f DB x"),
                c("COPY a f DB"),
                c("COPY nosuch g"),
                c("EXISTS g"),
                c("COPY a g bogus"),
                c("HSET hh f1 v1"),
                c("COPY hh hh2"),
                c("HGETALL hh2"),
                c("RPUSH l a b c"),
                c("COPY l l2"),
                c("LRANGE l2 0 -1"),
                c("SADD s m"),
                c("COPY s s2"),
                c("SMEMBERS s2"),
                c("ZADD z 1.5 m 2 n"),
                c("COPY z z2"),
                c("ZRANGE z2 0 -1 WITHSCORES"),
                c("PEXPIREAT a 4102444800000"),
                c("COPY a ttlcopy"),
                c("PEXPIRETIME ttlcopy"),
                c("COPY a hh REPLACE"),
                c("TYPE hh"),
                c("COPY a"),
            ],
        ),
        (
            "hstrlen",
            vec![
                c("HSET hh f1 v1 e x"),
                c("HSTRLEN hh f1"),
                c("HSTRLEN hh nope"),
                c("HSTRLEN missing f"),
                c("SET a 1"),
                c("HSTRLEN a f"),
                c("HSTRLEN hh"),
                c("HSTRLEN hh f1 extra"),
            ],
        ),
        (
            "zpop",
            vec![
                c("ZADD z 1 a 1 b 2 c 3 d"),
                c("ZPOPMIN z"),
                c("ZPOPMAX z"),
                c("ZPOPMIN z 0"),
                c("ZPOPMIN z -1"),
                c("ZPOPMIN z 5"),
                c("EXISTS z"),
                c("ZPOPMIN missing"),
                c("ZPOPMIN missing 2"),
                c("SET a 1"),
                c("ZPOPMIN a"),
                c("ZADD z3 1.5 x -2.25 y"),
                c("ZPOPMAX z3 10"),
                c("ZADD z4 1 x"),
                c("ZPOPMIN z4 notanumber"),
                c("ZPOPMIN z4 1 2"),
                c("ZPOPMIN"),
            ],
        ),
    ]
}

/// Deliberate, documented differences. Each entry: (case, command, reason).
const KNOWN_DEVIATIONS: &[(&str, &str, &str)] = &[
    // Recached has one database. Redis copies into db 1; Recached refuses
    // rather than report a copy it did not make. Not in the scripts above
    // (the replies differ by design); asserted below instead.
];

fn ttl_like(cmd: &[&str]) -> Option<i64> {
    match cmd.first().map(|c| c.to_ascii_uppercase()).as_deref() {
        Some("TTL") => Some(1),
        Some("PTTL") => Some(1_500),
        _ => None,
    }
}

fn same(cmd: &[&str], ours: &Value, theirs: &Value) -> bool {
    match (ttl_like(cmd), ours, theirs) {
        (Some(slack), Value::Integer(a), Value::Integer(b)) if *a >= 0 && *b >= 0 => {
            (a - b).abs() <= slack
        }
        // Error text is compared exactly: clients match on it.
        _ => ours == theirs,
    }
}

#[test]
#[ignore = "needs a Redis: set RECACHED_DIFF_REDIS=host:port and pass --ignored"]
fn new_commands_match_redis() {
    let redis = std::env::var("RECACHED_DIFF_REDIS")
        .expect("RECACHED_DIFF_REDIS=host:port of a Redis 7.2.x to compare against");
    let (_server, ours) = start_recached();
    let mut r = Conn::open(&redis);
    let mut o = Conn::open(&ours);
    let version = match r.cmd(&["INFO", "server"]) {
        Value::BulkString(Some(info)) => String::from_utf8_lossy(&info)
            .lines()
            .find_map(|l| l.strip_prefix("redis_version:").map(str::to_string))
            .unwrap_or_default(),
        other => panic!("INFO from Redis: {other:?}"),
    };
    eprintln!("comparing against Redis {version}");

    let mut mismatches = Vec::new();
    for (case, script) in cases() {
        assert_eq!(r.cmd(&["FLUSHDB"]), Value::SimpleString("OK".into()));
        assert_eq!(o.cmd(&["FLUSHDB"]), Value::SimpleString("OK".into()));
        for cmd in &script {
            let theirs = r.cmd(cmd);
            let ours = o.cmd(cmd);
            if !same(cmd, &ours, &theirs)
                && !KNOWN_DEVIATIONS
                    .iter()
                    .any(|(c, k, _)| *c == case && *k == cmd.join(" "))
            {
                mismatches.push(format!(
                    "[{case}] {}\n    redis:    {theirs:?}\n    recached: {ours:?}",
                    cmd.join(" ")
                ));
            }
        }
    }

    // The documented single-database deviation, asserted rather than hidden.
    o.cmd(&["SET", "a", "1"]);
    assert_eq!(
        o.cmd(&["COPY", "a", "b", "DB", "1"]),
        Value::Error("ERR DB index is out of range".into())
    );

    assert!(
        mismatches.is_empty(),
        "{} reply/replies differ from Redis {version}:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
