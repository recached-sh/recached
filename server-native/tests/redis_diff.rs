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
            // 6G: a field named twice in one HSET counts once, last value wins.
            // Recached up to 6F replied 2 to the first command.
            "hset-repeated-field",
            vec![
                c("HSET hd f a f b"),
                c("HGET hd f"),
                c("HSET hd f c g d g e"),
                c("HGETALL hd"),
                c("HLEN hd"),
                c("HSETNX hd g z"),
                c("HDEL hd f f"),
                c("HLEN hd"),
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
            // 6O-B: LPUSH/RPUSH are built straight from the read buffer and
            // run by reference: order, lengths, duplicates, both ends,
            // arity and type errors must stay Redis's.
            "lists-6ob",
            vec![
                c("LPUSH l a b c"),
                c("LRANGE l 0 -1"),
                c("RPUSH l d e"),
                c("LRANGE l 0 -1"),
                c("LPUSH l a a a"),
                c("LLEN l"),
                c("lpush l2 x"),
                c("RPush l2 y z"),
                c("LRANGE l2 0 -1"),
                c("LPOP l"),
                c("RPOP l"),
                c("LRANGE l 0 -1"),
                c("LPUSH l"),
                c("RPUSH"),
                c("SET str v"),
                c("LPUSH str x"),
                c("RPUSH str x"),
                c("GET str"),
                c("LPUSHX nosuch x"),
                c("EXISTS nosuch"),
            ],
        ),
        (
            // 6O-B: SADD counts members it added — once per member, however
            // often named — and re-adds allocate nothing but count nothing.
            "sets-6ob",
            vec![
                c("SADD s a b c"),
                c("SADD s a d"),
                c("SADD s a a a"),
                c("SADD s e e f"),
                c("SCARD s"),
                c("SISMEMBER s a"),
                c("SISMEMBER s z"),
                c("sadd s g"),
                c("SCARD s"),
                c("SADD s"),
                c("SADD"),
                c("SET str v"),
                c("SADD str m"),
                c("TYPE str"),
            ],
        ),
        (
            // 6O-B: MSET and SET on the paths the request-path changes touch.
            "mset-set-6ob",
            vec![
                c("MSET a 1 b 2 c 3"),
                c("MGET a b c"),
                c("MSET a 9 a 10"),
                c("GET a"),
                c("MSET a"),
                c("MSET a 1 b"),
                c("HSET h f v"),
                c("MSET h x"),
                c("TYPE h"),
                c("GET h"),
                c("SET k v EX 100"),
                c("MSET k v2"),
                c("TTL k"),
                c("SET k v"),
                c("SET k v2 NX"),
                c("SET k v3 XX"),
                c("GET k"),
                c("SET k v EX 100"),
                c("SET k v4 KEEPTTL"),
                c("TTL k"),
                c("SET k v5 GET"),
                c("SET nk v XX"),
                c("GET nk"),
            ],
        ),
        (
            // 6O-D: the semantics the MSET reader-atomicity change touches.
            "mset-atomicity-6od",
            vec![
                c("MSET d 1 d 2 d 3"),
                c("GET d"),
                c("SET r v"),
                c("RENAME r r"),
                c("GET r"),
                c("RENAME nokey nokey"),
                c("HINCRBYFLOAT hf f inf"),
                c("EXISTS hf"),
                c("HINCRBYFLOAT hf f -inf"),
                c("HSET hf2 f 1"),
                c("HINCRBYFLOAT hf2 f inf"),
                c("HGET hf2 f"),
                c("RPUSH l x"),
                c("LPOP l"),
                c("TYPE l"),
                c("EXISTS l"),
                c("MSET a 1 b 2"),
                c("EXISTS a b a missing"),
                c("MGET a b missing a"),
                c("SADD s m"),
                c("MSET s str"),
                c("TYPE s"),
                c("MGET s"),
                c("SET t v PX 100000"),
                c("MSET t w"),
                c("PTTL t"),
                c("DEL a b missing"),
                c("MGET a b"),
            ],
        ),
        (
            // `redis-cli --pipe` terminates every mass insert with ECHO.
            "echo",
            vec![c("ECHO hello"), c("ECHO"), c("ECHO a b")],
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
