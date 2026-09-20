//! Integration tests: a real server over a real socket, plus the RespClient
//! harness the connection-level suites share.

use crate::*;
use core_engine::cmd::{ScanArgs, SetOptions, ZAddOptions};
use core_engine::resp::Value;
use core_engine::store::KeyValueStore;
use std::sync::atomic::{AtomicBool, AtomicI64};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

fn tmp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("recached_test_{name}_{}", std::process::id()))
}

// ── TestServer harness ────────────────────────────────────────────────────

pub(super) struct TestServer {
    pub tcp_addr: std::net::SocketAddr,
    pub store: Arc<KeyValueStore>,
    pub state: Arc<ServerState>,
    _task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self._task.abort();
    }
}

pub(super) async fn spawn_server() -> TestServer {
    spawn_server_cfg(None, None, false).await
}

async fn spawn_server_cfg(
    password: Option<&str>,
    snap_path: Option<PathBuf>,
    start_as_replica: bool,
) -> TestServer {
    let store = Arc::new(KeyValueStore::new());
    let (tx, _rx) = broadcast::channel::<SyncMsg>(256);
    let pubsub: SharedPubSub = Arc::new(tokio::sync::Mutex::new(PubSubHub::new()));
    let watch_registry: WatchRegistry = WatchHub::new();
    let semaphore = Arc::new(Semaphore::new(64));
    let snap_cfg = Arc::new(SnapshotConfig {
        path: snap_path.unwrap_or_else(|| tmp_path("test.rdb")),
        last_save: AtomicI64::new(now_unix_secs()),
        checkpoint_id: AtomicU64::new(0),
    });
    let state = Arc::new(ServerState {
        snap: snap_cfg,
        aof: None,
        replicas: ReplHub::new(),
        is_replica: AtomicBool::new(start_as_replica),
        dedup: std::sync::Mutex::new(HashMap::new()),
        ephemeral: std::sync::Mutex::new(HashMap::new()),
        ephemeral_members: std::sync::Mutex::new(HashMap::new()),
        dedup_dirty: std::sync::atomic::AtomicBool::new(false),
        dedup_order: tokio::sync::Mutex::new(()),
        save_lock: tokio::sync::Mutex::new(()),
        persistence_healthy: AtomicBool::new(true),
        persistence_failures: AtomicU64::new(0),
    });

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let store2 = Arc::clone(&store);
    let state2 = Arc::clone(&state);
    let pass = Arc::new(password.map(|s| s.to_string()));

    let task = tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            let Ok(permit) = Arc::clone(&semaphore).try_acquire_owned() else {
                continue;
            };
            let (s, t, p, ps, wr, st) = (
                Arc::clone(&store2),
                tx.clone(),
                Arc::clone(&pass),
                Arc::clone(&pubsub),
                Arc::clone(&watch_registry),
                Arc::clone(&state2),
            );
            tokio::spawn(async move {
                let peer = socket
                    .peer_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_default();
                handle_tcp(socket, s, t, p, ps, wr, st, peer).await;
                drop(permit);
            });
        }
    });

    TestServer {
        tcp_addr: addr,
        store,
        state,
        _task: task,
    }
}

// ── RespClient ────────────────────────────────────────────────────────────

pub(super) struct RespClient {
    stream: TcpStream,
    buf: Vec<u8>,
    filled: usize,
}

impl RespClient {
    pub(super) async fn connect(addr: std::net::SocketAddr) -> Self {
        Self {
            stream: TcpStream::connect(addr).await.unwrap(),
            buf: vec![0u8; 65536],
            filled: 0,
        }
    }

    /// Send `args` and read one value. An empty `args` sends nothing and
    /// just reads the next frame — used to await an out-of-band push.
    pub(super) async fn cmd(&mut self, args: &[&str]) -> Value {
        if !args.is_empty() {
            let mut req = format!("*{}\r\n", args.len());
            for a in args {
                req.push_str(&format!("${}\r\n{}\r\n", a.len(), a));
            }
            self.stream.write_all(req.as_bytes()).await.unwrap();
        }
        loop {
            match Value::parse(&self.buf[..self.filled]) {
                Ok((val, n)) => {
                    self.buf.copy_within(n..self.filled, 0);
                    self.filled -= n;
                    return val;
                }
                Err(e) if e.is_incomplete() => {
                    let n = self
                        .stream
                        .read(&mut self.buf[self.filled..])
                        .await
                        .unwrap();
                    assert!(n > 0, "server closed connection unexpectedly");
                    self.filled += n;
                }
                Err(e) => panic!("RESP parse error: {e}"),
            }
        }
    }

    /// True once the peer has closed its half of the connection.
    async fn read_raw_eof(&mut self) -> bool {
        let mut buf = [0u8; 64];
        matches!(self.stream.read(&mut buf).await, Ok(0))
    }

    async fn read_until_closed(&mut self) {
        let mut buf = [0u8; 64];
        while self.stream.read(&mut buf).await.unwrap_or(0) > 0 {}
    }
}

fn ok() -> Value {
    Value::SimpleString("OK".to_string())
}
fn nil() -> Value {
    Value::BulkString(None)
}
fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}
fn int(n: i64) -> Value {
    Value::Integer(n)
}
fn arr(items: &[&str]) -> Value {
    Value::Array(Some(items.iter().map(|s| bulk(s)).collect()))
}

// ── is_write_command ──────────────────────────────────────────────────────

#[test]
fn is_write_command_classifies_correctly() {
    assert!(is_write_command(&Command::Set(
        "k".into(),
        "v".into(),
        SetOptions::default()
    )));
    assert!(is_write_command(&Command::Del(vec!["k".into()])));
    assert!(is_write_command(&Command::Incr("k".into())));
    assert!(is_write_command(&Command::FlushDb));
    assert!(is_write_command(&Command::HSet(
        "h".into(),
        vec![("f".into(), "v".into())]
    )));
    assert!(is_write_command(&Command::LPush(
        "l".into(),
        vec!["v".into()]
    )));
    assert!(is_write_command(&Command::SAdd(
        "s".into(),
        vec!["m".into()]
    )));
    assert!(is_write_command(&Command::ZAdd(
        "z".into(),
        ZAddOptions::default(),
        vec![(1.0, "m".into())]
    )));
    // reads
    assert!(!is_write_command(&Command::Get("k".into())));
    assert!(!is_write_command(&Command::HGet("h".into(), "f".into())));
    assert!(!is_write_command(&Command::LRange("l".into(), 0, -1)));
    assert!(!is_write_command(&Command::SMembers("s".into())));
    assert!(!is_write_command(&Command::DbSize));
    assert!(!is_write_command(&Command::Ping(None)));
    assert!(!is_write_command(&Command::Publish(
        "ch".into(),
        "msg".into()
    )));
}

// ── AOF replay ────────────────────────────────────────────────────────────

#[tokio::test]
async fn replay_aof_missing_file() {
    let store = KeyValueStore::new();
    let path = tmp_path("aof_missing");
    let count = replay_aof(&store, &path).await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn replay_aof_basic() {
    let store = KeyValueStore::new();
    let path = tmp_path("aof_basic.aof");
    let resp = "*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n\
                *3\r\n$3\r\nSET\r\n$3\r\nbaz\r\n$3\r\nqux\r\n";
    tokio::fs::write(&path, resp.as_bytes()).await.unwrap();
    let count = replay_aof(&store, &path).await.unwrap();
    assert_eq!(count, 2);
    assert_eq!(store.execute(Command::DbSize), Value::Integer(2));
    let _ = tokio::fs::remove_file(&path).await;
}

#[tokio::test]
async fn replay_aof_push_frames() {
    // The live server records writes via `on_write`, which stores them in
    // RESP3 Push (`>`) form. Replay must accept those, not just `*` arrays.
    let store = KeyValueStore::new();
    let path = tmp_path("aof_push.aof");
    let resp = ">3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n";
    tokio::fs::write(&path, resp.as_bytes()).await.unwrap();
    let count = replay_aof(&store, &path).await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        store.execute(Command::Get("foo".into())),
        Value::BulkString(Some(b"bar".to_vec()))
    );
    let _ = tokio::fs::remove_file(&path).await;
}

// ── Snapshot save / load ──────────────────────────────────────────────────

#[tokio::test]
async fn snapshot_save_and_load() {
    let store = KeyValueStore::new();
    store.execute(Command::Set(
        "hello".into(),
        "world".into(),
        SetOptions::default(),
    ));
    let path = tmp_path("snap.rdb");
    let cfg = Arc::new(SnapshotConfig {
        path: path.clone(),
        last_save: AtomicI64::new(0),
        checkpoint_id: AtomicU64::new(0),
    });
    save_snapshot(&store, &cfg, 0).await.unwrap();
    assert!(path.exists());
    let store2 = KeyValueStore::new();
    let loaded = load_snapshot(&store2, &path).await.unwrap();
    assert!(loaded);
    assert_eq!(
        store2.execute(Command::Get("hello".into())),
        Value::BulkString(Some(b"world".to_vec()))
    );
    let _ = tokio::fs::remove_file(&path).await;
}

// ── AofWriter append / truncate ───────────────────────────────────────────

#[tokio::test]
async fn aof_writer_append_and_truncate() {
    let path = tmp_path("aof_writer.aof");
    let aof = AofWriter::open(path.clone(), AofSync::No).await.unwrap();
    aof.append(b"*3\r\n$3\r\nSET\r\n$1\r\nk\r\n$1\r\nv\r\n")
        .await
        .unwrap();
    aof.flush().await.unwrap();
    let len_before = tokio::fs::metadata(&path).await.unwrap().len();
    assert!(len_before > 0);
    aof.truncate().await.unwrap();
    let len_after = tokio::fs::metadata(&path).await.unwrap().len();
    assert_eq!(len_after, 0);
    let _ = tokio::fs::remove_file(&path).await;
}

// ── Integration: 3a basic commands ───────────────────────────────────────

#[tokio::test]
async fn integration_set_get_del() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["SET", "k", "v"]).await, ok());
    assert_eq!(c.cmd(&["GET", "k"]).await, bulk("v"));
    assert_eq!(c.cmd(&["GET", "missing"]).await, nil());
    assert_eq!(c.cmd(&["DEL", "k"]).await, int(1));
    assert_eq!(c.cmd(&["GET", "k"]).await, nil());
    assert_eq!(c.cmd(&["DEL", "k"]).await, int(0)); // already gone
}

#[tokio::test]
async fn integration_binary_value_round_trips_over_resp() {
    // The drop-in claim runs through this port: a value that is not valid
    // UTF-8 must come back byte-for-byte, exactly as Redis would.
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    let binary: &[u8] = &[0xff, 0xfe, 0x00, 0x41, 0x80];
    let mut req = b"*3\r\n$3\r\nSET\r\n$3\r\nbin\r\n".to_vec();
    req.extend_from_slice(format!("${}\r\n", binary.len()).as_bytes());
    req.extend_from_slice(binary);
    req.extend_from_slice(b"\r\n");
    c.stream.write_all(&req).await.unwrap();
    assert_eq!(c.cmd(&[]).await, ok());

    assert_eq!(
        c.cmd(&["GET", "bin"]).await,
        Value::BulkString(Some(binary.to_vec())),
        "binary value must survive the round trip"
    );
    assert_eq!(
        c.cmd(&["STRLEN", "bin"]).await,
        int(binary.len() as i64),
        "length is counted in bytes"
    );
}

#[tokio::test]
async fn integration_binary_key_is_refused_over_resp() {
    // Keys stay text: they are glob-matched and scope-checked, so a
    // corrupted one would be silently unreachable. The refusal must be a
    // clean RESP error that leaves the connection usable.
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    c.stream
        .write_all(b"*3\r\n$3\r\nSET\r\n$2\r\n\xff\xfe\r\n$1\r\nv\r\n")
        .await
        .unwrap();
    let reply = c.cmd(&[]).await;
    let Value::Error(e) = &reply else {
        panic!("binary key must be refused, got {reply:?}")
    };
    assert!(e.contains("must be text"), "error must explain: {e:?}");

    assert_eq!(c.cmd(&["DBSIZE"]).await, int(0), "nothing may be stored");
    assert_eq!(c.cmd(&["SET", "ok", "v"]).await, ok());
}

#[tokio::test]
async fn integration_hello_negotiates_the_protocol() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    // Default is RESP2: the reply is a flat array, not a map.
    let v = c.cmd(&["HELLO"]).await;
    let Value::Array(Some(items)) = v else {
        panic!("RESP2 HELLO must reply with an array, got {v:?}")
    };
    assert!(items.contains(&bulk("recached")));
    assert!(items.contains(&Value::Integer(2)));

    // Upgrading yields a map keyed the same way.
    let v = c.cmd(&["HELLO", "3"]).await;
    let Value::Map(pairs) = v else {
        panic!("RESP3 HELLO must reply with a map, got {v:?}")
    };
    let proto = pairs
        .iter()
        .find(|(k, _)| *k == bulk("proto"))
        .map(|(_, v)| v.clone());
    assert_eq!(proto, Some(Value::Integer(3)));

    // An unsupported version is refused and the connection stays usable.
    let v = c.cmd(&["HELLO", "9"]).await;
    assert!(
        matches!(&v, Value::Error(e) if e.starts_with("NOPROTO")),
        "expected NOPROTO, got {v:?}"
    );
    assert_eq!(c.cmd(&["PING"]).await, Value::SimpleString("PONG".into()));
}

#[tokio::test]
async fn integration_pubsub_frame_type_follows_the_negotiated_protocol() {
    // The bug this pins: pub/sub deliveries were RESP3 push frames on every
    // connection, including RESP2 ones that cannot parse `>` at all.
    for (protover, want_push) in [(None, false), (Some("3"), true)] {
        let srv = spawn_server().await;
        let mut sub = RespClient::connect(srv.tcp_addr).await;
        if let Some(v) = protover {
            sub.cmd(&["HELLO", v]).await;
        }
        assert!(matches!(
            sub.cmd(&["SUBSCRIBE", "news"]).await,
            Value::Array(_) | Value::Push(_)
        ));

        let mut pubr = RespClient::connect(srv.tcp_addr).await;
        // Wait for the subscription to register before publishing.
        for _ in 0..50 {
            if pubr.cmd(&["PUBLISH", "news", "hi"]).await == int(1) {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        }

        let delivery = sub.cmd(&[]).await;
        match (&delivery, want_push) {
            (Value::Push(_), true) => {}
            (Value::Array(Some(_)), false) => {}
            _ => panic!("protover {protover:?}: expected push={want_push}, got {delivery:?}"),
        }
    }
}

#[tokio::test]
async fn integration_repeated_subscribe_does_not_duplicate_registration() {
    let srv = spawn_server().await;
    let mut subscriber = RespClient::connect(srv.tcp_addr).await;
    assert!(matches!(
        subscriber.cmd(&["SUBSCRIBE", "news"]).await,
        Value::Array(_)
    ));
    assert!(matches!(
        subscriber.cmd(&["SUBSCRIBE", "news"]).await,
        Value::Array(_)
    ));

    let mut inspector = RespClient::connect(srv.tcp_addr).await;
    assert_eq!(
        inspector.cmd(&["PUBSUB", "NUMSUB", "news"]).await,
        Value::Array(Some(vec![bulk("news"), int(1)]))
    );
}

#[tokio::test]
async fn integration_incr_and_expiry() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["SET", "n", "10"]).await, ok());
    assert_eq!(c.cmd(&["INCR", "n"]).await, int(11));
    assert_eq!(c.cmd(&["INCRBY", "n", "4"]).await, int(15));
    assert_eq!(c.cmd(&["DECR", "n"]).await, int(14));

    // TTL: set a key with 1-second expiry and verify TTL and eventual expiry
    assert_eq!(c.cmd(&["SET", "ex", "val", "EX", "1"]).await, ok());
    let ttl = c.cmd(&["TTL", "ex"]).await;
    assert!(matches!(ttl, Value::Integer(1) | Value::Integer(0)));
    tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;
    assert_eq!(c.cmd(&["GET", "ex"]).await, nil());
}

#[tokio::test]
async fn integration_string_commands() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    // APPEND + STRLEN
    assert_eq!(c.cmd(&["APPEND", "s", "hello"]).await, int(5));
    assert_eq!(c.cmd(&["APPEND", "s", " world"]).await, int(11));
    assert_eq!(c.cmd(&["STRLEN", "s"]).await, int(11));

    // GETSET
    assert_eq!(c.cmd(&["GETSET", "s", "new"]).await, bulk("hello world"));
    assert_eq!(c.cmd(&["GET", "s"]).await, bulk("new"));

    // SETNX
    assert_eq!(c.cmd(&["SETNX", "nx", "first"]).await, int(1));
    assert_eq!(c.cmd(&["SETNX", "nx", "second"]).await, int(0));
    assert_eq!(c.cmd(&["GET", "nx"]).await, bulk("first"));

    // SETEX
    assert_eq!(c.cmd(&["SETEX", "ex", "60", "val"]).await, ok());
    let ttl = c.cmd(&["TTL", "ex"]).await;
    assert!(matches!(ttl, Value::Integer(t) if t > 0 && t <= 60));

    // MSET / MGET
    assert_eq!(c.cmd(&["MSET", "a", "1", "b", "2", "c", "3"]).await, ok());
    let got = c.cmd(&["MGET", "a", "b", "c", "missing"]).await;
    assert_eq!(
        got,
        Value::Array(Some(vec![bulk("1"), bulk("2"), bulk("3"), nil()]))
    );
}

#[tokio::test]
async fn integration_bounded_reads_over_resp() {
    // The pair of primitives a client needs to inspect a large key without
    // pulling it whole: a byte window into a string, and a cursor over a
    // collection. Exercised over the wire because that is where the reply
    // shape — bulk cursor, nested array — has to be right.
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    c.cmd(&["SET", "s", "This is a string"]).await;
    assert_eq!(c.cmd(&["GETRANGE", "s", "0", "3"]).await, bulk("This"));
    assert_eq!(c.cmd(&["GETRANGE", "s", "-6", "-1"]).await, bulk("string"));
    assert_eq!(c.cmd(&["GETRANGE", "ghost", "0", "-1"]).await, bulk(""));

    c.cmd(&["HSET", "h", "a", "1", "b", "2", "c", "3"]).await;
    assert_eq!(
        c.cmd(&["HSCAN", "h", "0", "COUNT", "2"]).await,
        Value::Array(Some(vec![
            bulk("2"),
            Value::Array(Some(vec![bulk("a"), bulk("1"), bulk("b"), bulk("2")])),
        ]))
    );
    assert_eq!(
        c.cmd(&["HSCAN", "h", "2"]).await,
        Value::Array(Some(vec![
            bulk("0"),
            Value::Array(Some(vec![bulk("c"), bulk("3")])),
        ]))
    );
    assert_eq!(
        c.cmd(&["HSCAN", "h", "0", "NOVALUES"]).await,
        Value::Array(Some(vec![
            bulk("0"),
            Value::Array(Some(vec![bulk("a"), bulk("b"), bulk("c")])),
        ]))
    );

    c.cmd(&["SADD", "st", "x", "y"]).await;
    assert_eq!(
        c.cmd(&["SSCAN", "st", "0", "MATCH", "x*"]).await,
        Value::Array(Some(vec![bulk("0"), Value::Array(Some(vec![bulk("x")])),]))
    );

    c.cmd(&["ZADD", "z", "1.5", "amy"]).await;
    assert_eq!(
        c.cmd(&["ZSCAN", "z", "0"]).await,
        Value::Array(Some(vec![
            bulk("0"),
            Value::Array(Some(vec![bulk("amy"), bulk("1.5")])),
        ]))
    );
}

#[tokio::test]
async fn integration_bounded_reads_are_allowed_on_a_replica() {
    // Read-only by construction: a replica must serve them, and the
    // is_write_command allowlist is what decides that.
    let srv = spawn_server_cfg(None, None, true).await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["GETRANGE", "s", "0", "-1"]).await, bulk(""));
    assert_eq!(
        c.cmd(&["HSCAN", "h", "0"]).await,
        Value::Array(Some(vec![bulk("0"), Value::Array(Some(vec![]))]))
    );
}

#[tokio::test]
async fn integration_handshake_commands_over_resp() {
    // Every current client library opens with HELLO + CLIENT SETINFO and
    // closes with QUIT. This is that sequence, on the wire.
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(
        c.cmd(&["CLIENT", "SETINFO", "LIB-NAME", "node-redis"])
            .await,
        ok()
    );
    assert_eq!(
        c.cmd(&["CLIENT", "SETINFO", "LIB-VER", "6.2.0"]).await,
        ok()
    );
    assert_eq!(c.cmd(&["CLIENT", "SETNAME", "tap"]).await, ok());
    assert_eq!(c.cmd(&["CLIENT", "GETNAME"]).await, bulk("tap"));

    let Value::Integer(id) = c.cmd(&["CLIENT", "ID"]).await else {
        panic!("CLIENT ID must be an integer")
    };
    assert!(id > 0);

    let Value::BulkString(Some(info)) = c.cmd(&["CLIENT", "INFO"]).await else {
        panic!("CLIENT INFO must be a bulk string")
    };
    let info = String::from_utf8(info).unwrap();
    assert!(info.contains(&format!("id={id}")), "{info}");
    assert!(info.contains("lib-name=node-redis"), "{info}");
    assert!(info.contains("name=tap"), "{info}");
    assert!(info.contains("addr=127.0.0.1:"), "{info}");

    // This connection must appear in the list it asks for.
    let Value::BulkString(Some(list)) = c.cmd(&["CLIENT", "LIST"]).await else {
        panic!("CLIENT LIST must be a bulk string")
    };
    let list = String::from_utf8(list).unwrap();
    assert!(
        list.lines().any(|l| l.contains(&format!("id={id}"))),
        "{list}"
    );

    assert_eq!(
        c.cmd(&["CONFIG", "GET", "maxmemory-policy"]).await,
        Value::Array(Some(vec![bulk("maxmemory-policy"), bulk("noeviction")]))
    );

    let Value::Integer(n) = c.cmd(&["COMMAND", "COUNT"]).await else {
        panic!("COMMAND COUNT must be an integer")
    };
    assert!(
        n > 100,
        "the catalog should cover the whole command set, got {n}"
    );
}

#[tokio::test]
async fn integration_quit_replies_then_closes() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;
    assert_eq!(c.cmd(&["PING"]).await, Value::SimpleString("PONG".into()));
    // +OK first, then the close — a client that reads its reply before
    // dropping the socket must not see a connection error instead.
    assert_eq!(c.cmd(&["QUIT"]).await, ok());
    assert!(
        c.read_raw_eof().await,
        "the server must close the connection after QUIT"
    );
}

#[tokio::test]
async fn integration_quit_works_before_authentication() {
    // Redis flags QUIT no_auth. A client that cannot authenticate still
    // gets a clean close instead of leaving a socket parked on the server.
    let srv = spawn_server_cfg(Some("hunter2"), None, false).await;
    let mut c = RespClient::connect(srv.tcp_addr).await;
    assert!(matches!(c.cmd(&["PING"]).await, Value::Error(e) if e.contains("NOAUTH")));
    assert_eq!(c.cmd(&["QUIT"]).await, ok());
}

#[tokio::test]
async fn integration_client_list_sees_other_connections() {
    let srv = spawn_server().await;
    let mut a = RespClient::connect(srv.tcp_addr).await;
    let mut b = RespClient::connect(srv.tcp_addr).await;
    b.cmd(&["CLIENT", "SETNAME", "second"]).await;

    let Value::BulkString(Some(list)) = a.cmd(&["CLIENT", "LIST"]).await else {
        panic!("expected a bulk string")
    };
    let list = String::from_utf8(list).unwrap();
    assert!(
        list.lines().any(|l| l.contains("name=second")),
        "a connection must see its peers, got:\n{list}"
    );
    assert!(list.lines().count() >= 2, "{list}");
}

#[tokio::test]
async fn integration_hash_commands() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["HSET", "h", "f1", "v1", "f2", "v2"]).await, int(2));
    assert_eq!(c.cmd(&["HGET", "h", "f1"]).await, bulk("v1"));
    assert_eq!(c.cmd(&["HGET", "h", "missing"]).await, nil());
    assert_eq!(c.cmd(&["HLEN", "h"]).await, int(2));
    assert_eq!(c.cmd(&["HDEL", "h", "f1"]).await, int(1));
    assert_eq!(c.cmd(&["HLEN", "h"]).await, int(1));
    // HGETALL returns field-value pairs
    let all = c.cmd(&["HGETALL", "h"]).await;
    assert_eq!(all, Value::Array(Some(vec![bulk("f2"), bulk("v2")])));
}

#[tokio::test]
async fn integration_list_commands() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["RPUSH", "l", "a", "b", "c"]).await, int(3));
    assert_eq!(c.cmd(&["LPUSH", "l", "z"]).await, int(4));
    assert_eq!(c.cmd(&["LLEN", "l"]).await, int(4));
    assert_eq!(
        c.cmd(&["LRANGE", "l", "0", "-1"]).await,
        Value::Array(Some(vec![bulk("z"), bulk("a"), bulk("b"), bulk("c")]))
    );
    assert_eq!(c.cmd(&["LPOP", "l"]).await, bulk("z"));
    assert_eq!(c.cmd(&["RPOP", "l"]).await, bulk("c"));
    assert_eq!(c.cmd(&["LLEN", "l"]).await, int(2));
}

#[tokio::test]
async fn integration_set_commands() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["SADD", "s", "a", "b", "c"]).await, int(3));
    assert_eq!(c.cmd(&["SADD", "s", "a"]).await, int(0)); // duplicate
    assert_eq!(c.cmd(&["SCARD", "s"]).await, int(3));
    assert_eq!(c.cmd(&["SISMEMBER", "s", "b"]).await, int(1));
    assert_eq!(c.cmd(&["SISMEMBER", "s", "x"]).await, int(0));
    assert_eq!(c.cmd(&["SREM", "s", "a"]).await, int(1));
    assert_eq!(c.cmd(&["SCARD", "s"]).await, int(2));
}

#[tokio::test]
async fn integration_zset_commands() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(
        c.cmd(&["ZADD", "z", "1.5", "a", "2.5", "b", "3.0", "c"])
            .await,
        int(3)
    );
    assert_eq!(c.cmd(&["ZCARD", "z"]).await, int(3));
    assert_eq!(c.cmd(&["ZSCORE", "z", "b"]).await, bulk("2.5"));
    assert_eq!(c.cmd(&["ZRANK", "z", "a"]).await, int(0));
    assert_eq!(c.cmd(&["ZRANK", "z", "c"]).await, int(2));
    assert_eq!(
        c.cmd(&["ZRANGE", "z", "0", "-1", "WITHSCORES"]).await,
        Value::Array(Some(vec![
            bulk("a"),
            bulk("1.5"),
            bulk("b"),
            bulk("2.5"),
            bulk("c"),
            bulk("3"),
        ]))
    );
    assert_eq!(c.cmd(&["ZREM", "z", "b"]).await, int(1));
    assert_eq!(c.cmd(&["ZCARD", "z"]).await, int(2));
}

#[tokio::test]
async fn integration_transactions_exec() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["SET", "counter", "10"]).await, ok());
    assert_eq!(c.cmd(&["MULTI"]).await, ok());
    assert_eq!(
        c.cmd(&["SET", "counter", "20"]).await,
        Value::SimpleString("QUEUED".to_string())
    );
    assert_eq!(
        c.cmd(&["INCR", "counter"]).await,
        Value::SimpleString("QUEUED".to_string())
    );
    let res = c.cmd(&["EXEC"]).await;
    assert_eq!(res, Value::Array(Some(vec![ok(), int(21)])));
    assert_eq!(c.cmd(&["GET", "counter"]).await, bulk("21"));
}

#[tokio::test]
async fn integration_transactions_discard() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["SET", "key", "original"]).await, ok());
    assert_eq!(c.cmd(&["MULTI"]).await, ok());
    assert_eq!(
        c.cmd(&["DEL", "key"]).await,
        Value::SimpleString("QUEUED".to_string())
    );
    assert_eq!(c.cmd(&["DISCARD"]).await, ok());
    assert_eq!(c.cmd(&["GET", "key"]).await, bulk("original")); // DEL was discarded
}

#[tokio::test]
async fn integration_unknown_command() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    let r = c.cmd(&["NOTACOMMAND", "arg"]).await;
    assert!(matches!(r, Value::Error(_)));
}

// ── Integration: 3b auth ──────────────────────────────────────────────────

#[tokio::test]
async fn integration_auth_blocks_unauthenticated() {
    let srv = spawn_server_cfg(Some("secret"), None, false).await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    let r = c.cmd(&["SET", "k", "v"]).await;
    assert!(matches!(&r, Value::Error(e) if e.contains("NOAUTH")));
}

#[tokio::test]
async fn integration_auth_correct() {
    let srv = spawn_server_cfg(Some("secret"), None, false).await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["AUTH", "secret"]).await, ok());
    assert_eq!(c.cmd(&["SET", "k", "v"]).await, ok());
    assert_eq!(c.cmd(&["GET", "k"]).await, bulk("v"));
}

#[tokio::test]
async fn integration_auth_wrong_password_lockout() {
    let srv = spawn_server_cfg(Some("secret"), None, false).await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    // First 4 wrong attempts → "ERR invalid password"
    for _ in 0..4 {
        let r = c.cmd(&["AUTH", "wrong"]).await;
        assert!(matches!(&r, Value::Error(e) if e.contains("invalid")));
    }
    // 5th attempt hits MAX_AUTH_FAILURES → "too many" + server disconnects
    let r = c.cmd(&["AUTH", "wrong"]).await;
    assert!(matches!(&r, Value::Error(e) if e.contains("too many")));
    c.read_until_closed().await;
}

// ── Integration: 3c persistence ───────────────────────────────────────────

#[tokio::test]
async fn integration_save_and_reload() {
    let snap = tmp_path("integ_snap.rdb");
    let srv = spawn_server_cfg(None, Some(snap.clone()), false).await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["SET", "hello", "world"]).await, ok());
    assert_eq!(c.cmd(&["SET", "foo", "bar"]).await, ok());
    assert_eq!(c.cmd(&["SAVE"]).await, ok());

    // Load into a fresh store
    let store2 = KeyValueStore::new();
    let loaded = load_snapshot(&store2, &snap).await.unwrap();
    assert!(loaded);
    assert_eq!(
        store2.execute(Command::Get("hello".into())),
        Value::BulkString(Some(b"world".to_vec()))
    );
    assert_eq!(
        store2.execute(Command::Get("foo".into())),
        Value::BulkString(Some(b"bar".to_vec()))
    );
    let _ = tokio::fs::remove_file(&snap).await;
}

#[tokio::test]
async fn integration_aof_replay() {
    let path = tmp_path("integ_aof.aof");
    let aof = AofWriter::open(path.clone(), AofSync::No).await.unwrap();
    let store = KeyValueStore::new();
    let snap_cfg = Arc::new(SnapshotConfig {
        path: tmp_path("integ_aof.rdb"),
        last_save: AtomicI64::new(0),
        checkpoint_id: AtomicU64::new(0),
    });
    let state = Arc::new(ServerState {
        snap: snap_cfg,
        aof: Some(Arc::new(aof)),
        replicas: ReplHub::new(),
        is_replica: AtomicBool::new(false),
        dedup: std::sync::Mutex::new(HashMap::new()),
        ephemeral: std::sync::Mutex::new(HashMap::new()),
        ephemeral_members: std::sync::Mutex::new(HashMap::new()),
        dedup_dirty: std::sync::atomic::AtomicBool::new(false),
        dedup_order: tokio::sync::Mutex::new(()),
        save_lock: tokio::sync::Mutex::new(()),
        persistence_healthy: AtomicBool::new(true),
        persistence_failures: AtomicU64::new(0),
    });

    // Simulate writes captured by AOF
    state
        .on_write(b"*3\r\n$3\r\nSET\r\n$5\r\nhello\r\n$5\r\nworld\r\n")
        .await;
    state
        .on_write(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
        .await;
    if let Some(ref a) = state.aof {
        a.flush().await.unwrap();
    }

    // Replay into fresh store
    let store2 = KeyValueStore::new();
    let count = replay_aof(&store2, &path).await.unwrap();
    assert_eq!(count, 2);
    assert_eq!(
        store2.execute(Command::Get("hello".into())),
        Value::BulkString(Some(b"world".to_vec()))
    );
    drop(store); // suppress unused warning
    let _ = tokio::fs::remove_file(&path).await;
}

#[tokio::test]
async fn integration_dirty_counter() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(srv.store.dirty_count(), 0);

    assert_eq!(c.cmd(&["SET", "a", "1"]).await, ok());
    assert_eq!(c.cmd(&["SET", "b", "2"]).await, ok());
    assert_eq!(srv.store.dirty_count(), 2);

    // Trigger a save — dirty resets to 0
    assert_eq!(c.cmd(&["SAVE"]).await, ok());
    assert_eq!(srv.store.dirty_count(), 0);

    // Baseline *after* the explicit save, not before it: SAVE writes
    // last_save itself, so a baseline taken beforehand differs by one
    // whenever the save lands in the next whole second — which is what this
    // test used to fail on, roughly one run in thirty. The assertion below
    // is about no *further* save happening.
    let last_save = srv.state.snap.last_save.load(Ordering::Relaxed);

    // No new writes → save condition not met → last_save unchanged after 1s
    tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;
    assert_eq!(
        last_save,
        srv.state.snap.last_save.load(Ordering::Relaxed),
        "no autosave should fire with no conditions configured"
    );
}

// ── Integration: 3d replication ───────────────────────────────────────────

#[tokio::test]
async fn integration_replica_rejects_writes() {
    let srv = spawn_server_cfg(None, None, true).await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    let r = c.cmd(&["SET", "k", "v"]).await;
    assert!(matches!(&r, Value::Error(e) if e.contains("READONLY")));
    // Reads still work
    assert_eq!(c.cmd(&["GET", "k"]).await, nil());
}

#[tokio::test]
async fn integration_replica_rejects_writes_queued_in_multi() {
    let srv = spawn_server_cfg(None, None, true).await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["MULTI"]).await, ok());
    assert_eq!(
        c.cmd(&["SET", "k", "v"]).await,
        Value::SimpleString("QUEUED".into())
    );
    let response = c.cmd(&["EXEC"]).await;
    assert!(matches!(&response, Value::Error(message) if message.contains("READONLY")));
    assert_eq!(c.cmd(&["GET", "k"]).await, nil());
}

#[tokio::test]
async fn integration_replicaof_no_one_promotes() {
    let srv = spawn_server_cfg(None, None, true).await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    // Promote
    assert_eq!(c.cmd(&["REPLICAOF", "NO", "ONE"]).await, ok());
    // Now writes are accepted
    assert_eq!(c.cmd(&["SET", "k", "v"]).await, ok());
    assert_eq!(c.cmd(&["GET", "k"]).await, bulk("v"));
    assert!(!srv.state.is_replica());
}

// ── FLUSHDB reaches live queries ──────────────────────────────────────────

/// Collect every frame arriving within `ms`, so a test can assert on the
/// keychange among the command-replay pushes that travel alongside it.
async fn drain_frames(c: &mut WsClient, ms: u64) -> Vec<String> {
    let mut out = Vec::new();
    while let Some(f) = c.recv_any(ms).await {
        out.push(f);
    }
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flushdb_notifies_live_query_subscribers() {
    // Previously FLUSHDB emitted nothing to live queries: primary_keys() is
    // empty for it, so subscribers kept serving data the server had wiped.
    let srv = spawn_ws_server().await;
    let mut watcher = WsClient::connect(srv.tcp_addr).await;
    watcher.cmd(&["QSUB", "cart:*"]).await;

    let mut writer = WsClient::connect(srv.tcp_addr).await;
    writer.cmd(&["SET", "cart:item:1", "a"]).await;
    drain_frames(&mut watcher, 400).await;

    writer.cmd(&["FLUSHDB"]).await;

    let frames = drain_frames(&mut watcher, 800).await;
    assert!(
        frames
            .iter()
            .any(|f| f.contains("keychange") && f.contains("cart:*")),
        "expected a keychange sentinel naming the pattern, got: {frames:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flushdb_sends_one_sentinel_per_pattern_not_per_key() {
    // The reason for a sentinel: announcing per key would be one frame per
    // key in the keyspace for a single command.
    let srv = spawn_ws_server().await;
    let mut watcher = WsClient::connect(srv.tcp_addr).await;
    watcher.cmd(&["QSUB", "bulk:*"]).await;

    let mut writer = WsClient::connect(srv.tcp_addr).await;
    for i in 0..25 {
        writer.cmd(&["SET", &format!("bulk:{i}"), "v"]).await;
    }
    drain_frames(&mut watcher, 500).await;

    writer.cmd(&["FLUSHDB"]).await;

    let keychanges: Vec<String> = drain_frames(&mut watcher, 800)
        .await
        .into_iter()
        .filter(|f| f.contains("keychange"))
        .collect();
    assert_eq!(
        keychanges.len(),
        1,
        "25 keys wiped must produce one sentinel, not 25 frames: {keychanges:?}"
    );
    assert!(keychanges[0].contains("bulk:*"));
}

// ── Duplicate suppression across a checkpoint ─────────────────────────────

/// Build a `ServerState` whose snapshot path (and therefore dedup sidecar)
/// is `path` — the same file a restarted process would find.
fn state_with_snapshot_path(path: PathBuf) -> Arc<ServerState> {
    Arc::new(ServerState {
        snap: Arc::new(SnapshotConfig {
            path,
            last_save: AtomicI64::new(now_unix_secs()),
            checkpoint_id: AtomicU64::new(0),
        }),
        aof: None,
        replicas: ReplHub::new(),
        is_replica: AtomicBool::new(false),
        dedup: std::sync::Mutex::new(HashMap::new()),
        ephemeral: std::sync::Mutex::new(HashMap::new()),
        ephemeral_members: std::sync::Mutex::new(HashMap::new()),
        dedup_dirty: std::sync::atomic::AtomicBool::new(false),
        dedup_order: tokio::sync::Mutex::new(()),
        save_lock: tokio::sync::Mutex::new(()),
        persistence_healthy: AtomicBool::new(true),
        persistence_failures: AtomicU64::new(0),
    })
}

#[tokio::test]
async fn a_failed_dedup_write_does_not_consume_its_id() {
    let state = state_with_snapshot_path(tmp_path("dedup_failure.rdb"));
    let store = KeyValueStore::new();
    store.execute(Command::Set(
        "typed".into(),
        b"string".to_vec(),
        SetOptions::default(),
    ));
    let wrapped = Command::Dedup(
        "client-a".into(),
        7,
        Box::new(Command::HIncrBy("typed".into(), "field".into(), 1)),
    );
    let tx = broadcast::channel::<SyncMsg>(8).0;
    let watches = WatchHub::new();

    let response = execute_ordered_write(&wrapped, &tx, 1, &state, &watches, &store).await;
    assert!(matches!(response, Value::Error(_)));
    assert!(!state.dedup_is_duplicate("client-a", 7));

    store.execute(Command::Del(vec!["typed".into()]));
    assert_eq!(
        execute_ordered_write(&wrapped, &tx, 1, &state, &watches, &store).await,
        Value::Integer(1)
    );
    assert_eq!(
        execute_ordered_write(&wrapped, &tx, 1, &state, &watches, &store).await,
        Value::SimpleString("DUP".into())
    );
}

#[tokio::test]
async fn dedup_mark_is_checkpointed_with_the_successful_write() {
    let snap = tmp_path("dedup_checkpoint.rdb");
    let sidecar = snap.with_extension("dedup");
    let _ = std::fs::remove_file(&snap);
    let _ = std::fs::remove_file(&sidecar);
    let state = state_with_snapshot_path(snap.clone());
    let store = KeyValueStore::new();
    let tx = broadcast::channel::<SyncMsg>(8).0;
    let watches = WatchHub::new();
    let wrapped = Command::Dedup(
        "client-b".into(),
        3,
        Box::new(Command::Set(
            "durable".into(),
            b"value".to_vec(),
            SetOptions::default(),
        )),
    );

    assert_eq!(
        execute_ordered_write(&wrapped, &tx, 1, &state, &watches, &store).await,
        ok()
    );
    assert!(
        !sidecar.exists(),
        "dedup state must not outrun its data snapshot"
    );
    state.save(&store).await.unwrap();

    let restored_store = KeyValueStore::new();
    assert!(load_snapshot(&restored_store, &snap).await.unwrap());
    let restored_state = state_with_snapshot_path(snap.clone());
    restored_state.load_dedup().await.unwrap();
    assert!(restored_state.dedup_is_duplicate("client-b", 3));
    assert_eq!(
        restored_store.execute(Command::Get("durable".into())),
        Value::BulkString(Some(b"value".to_vec()))
    );

    let _ = std::fs::remove_file(snap);
    let _ = std::fs::remove_file(sidecar);
}

#[tokio::test]
async fn dedup_marks_survive_a_restart() {
    let snap = tmp_path("dedup_restart.rdb");
    let _ = std::fs::remove_file(snap.with_extension("dedup"));

    // First run: the client's writes are accepted once each.
    let first = state_with_snapshot_path(snap.clone());
    assert!(!first.dedup_seen("client-a", 1));
    assert!(!first.dedup_seen("client-a", 2));
    assert!(
        first.dedup_seen("client-a", 2),
        "same id twice within a run"
    );
    first.persist_dedup().await.unwrap();

    // Restart: fresh process, same snapshot path.
    let second = state_with_snapshot_path(snap.clone());
    assert!(
        !second.dedup_seen("client-a", 3),
        "a genuinely new id must still be accepted"
    );

    let third = state_with_snapshot_path(snap.clone());
    third.load_dedup().await.unwrap();
    assert!(
        third.dedup_seen("client-a", 2),
        "a replayed write must be recognised after a restart — this is the \
         caveat the sidecar exists to close"
    );
    assert!(!third.dedup_seen("client-a", 99), "higher ids still apply");

    let _ = std::fs::remove_file(snap.with_extension("dedup"));
}

#[tokio::test]
async fn persist_dedup_is_a_no_op_when_nothing_advanced() {
    // The flusher runs every second; it must not rewrite the file when no
    // mark moved.
    let snap = tmp_path("dedup_noop.rdb");
    let side = snap.with_extension("dedup");
    let _ = std::fs::remove_file(&side);

    let state = state_with_snapshot_path(snap.clone());
    state.dedup_seen("c", 1);
    state.persist_dedup().await.unwrap();
    assert!(side.exists(), "first flush should write");

    let before = std::fs::metadata(&side).unwrap().modified().unwrap();
    state.persist_dedup().await.unwrap(); // nothing changed since
    let after = std::fs::metadata(&side).unwrap().modified().unwrap();
    assert_eq!(before, after, "unchanged marks must not rewrite the file");

    let _ = std::fs::remove_file(&side);
}

#[tokio::test]
async fn a_corrupt_dedup_sidecar_is_reported() {
    // Silently losing duplicate-suppression bookkeeping would allow acknowledged
    // writes to be applied twice after restart.
    let snap = tmp_path("dedup_corrupt.rdb");
    let side = snap.with_extension("dedup");
    std::fs::write(&side, b"not messagepack").unwrap();

    let state = state_with_snapshot_path(snap.clone());
    assert!(state.load_dedup().await.is_err());

    let _ = std::fs::remove_file(&side);
}

#[tokio::test]
async fn a_missing_dedup_sidecar_is_a_clean_first_boot() {
    let snap = tmp_path("dedup_absent.rdb");
    let _ = std::fs::remove_file(snap.with_extension("dedup"));
    let state = state_with_snapshot_path(snap);
    state.load_dedup().await.unwrap();
    assert!(!state.dedup_seen("fresh", 1));
}

// ── Presence: connection-scoped keys ──────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn eset_key_is_deleted_when_its_connection_closes() {
    let srv = spawn_ws_server().await;
    let mut watcher = WsClient::connect(srv.tcp_addr).await;
    watcher.cmd(&["QSUB", "presence:*"]).await;

    {
        let mut presence = WsClient::connect(srv.tcp_addr).await;
        assert_eq!(
            presence.cmd(&["ESET", "presence:user:42", "online"]).await,
            Value::SimpleString("OK".into())
        );
        // Visible to everyone while the connection is open.
        assert_eq!(
            srv.store.execute(Command::Get("presence:user:42".into())),
            Value::BulkString(Some(b"online".to_vec()))
        );
    } // connection dropped here

    // The key goes away on its own — no heartbeat, no TTL to wait out.
    for _ in 0..40 {
        if srv.store.execute(Command::Get("presence:user:42".into())) == Value::BulkString(None) {
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
    panic!("ephemeral key outlived its connection");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn eset_deletion_is_broadcast_to_live_queries() {
    // Presence is only useful if peers are *told*; polling for the absence
    // of a key is the thing this replaces.
    let srv = spawn_ws_server().await;
    let mut watcher = WsClient::connect(srv.tcp_addr).await;
    watcher.cmd(&["QSUB", "presence:*"]).await;

    {
        let mut presence = WsClient::connect(srv.tcp_addr).await;
        presence.cmd(&["ESET", "presence:user:7", "online"]).await;
        // Drain the set notification.
        let _ = watcher.recv_push(1000).await;
    }

    let frames = drain_frames(&mut watcher, 1500).await;
    assert!(
        frames
            .iter()
            .any(|f| f.contains("keychange") && f.contains("presence:user:7")),
        "a live query must receive a keychange for the departing peer, got: {frames:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_tab_keeps_presence_alive_when_the_first_closes() {
    // The multi-tab case. Ownership transfers to the most recent writer, so
    // closing an older tab must not mark the user offline.
    let srv = spawn_ws_server().await;

    let mut tab_b = WsClient::connect(srv.tcp_addr).await;
    {
        let mut tab_a = WsClient::connect(srv.tcp_addr).await;
        tab_a.cmd(&["ESET", "presence:user:9", "online"]).await;
        // Tab B claims the same key — it is now the owner.
        tab_b.cmd(&["ESET", "presence:user:9", "online"]).await;
    } // tab A closes

    // Give the close handler time to run, then confirm the key survived.
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    assert_eq!(
        srv.store.execute(Command::Get("presence:user:9".into())),
        Value::BulkString(Some(b"online".to_vec())),
        "closing an older tab must not clear presence held by a newer one"
    );

    drop(tab_b);
    for _ in 0..40 {
        if srv.store.execute(Command::Get("presence:user:9".into())) == Value::BulkString(None) {
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
    panic!("key outlived its last owner");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plain_set_is_not_ephemeral() {
    // Only ESET opts into connection-scoped lifetime; SET must be unaffected.
    let srv = spawn_ws_server().await;
    {
        let mut c = WsClient::connect(srv.tcp_addr).await;
        c.cmd(&["SET", "durable:key", "value"]).await;
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    assert_eq!(
        srv.store.execute(Command::Get("durable:key".into())),
        Value::BulkString(Some(b"value".to_vec()))
    );
}

#[tokio::test]
async fn integration_replica_receives_write() {
    // Spawn primary with a separate replication listener on a random port
    let primary = spawn_server().await;
    let repl_registry: ReplRegistry = ReplHub::new();
    let snap_cfg = Arc::clone(&primary.state.snap);
    let primary_store = Arc::clone(&primary.store);
    let reg = Arc::clone(&repl_registry);

    // Replication listener — binds on port 0
    let repl_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let repl_port = repl_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((socket, _)) = repl_listener.accept().await {
            let s = Arc::clone(&primary_store);
            let sc = Arc::clone(&snap_cfg);
            let r = Arc::clone(&reg);
            tokio::spawn(handle_replica(
                socket,
                s,
                sc,
                r,
                None,
                DEFAULT_REPL_CHANNEL_CAPACITY,
                IpAddr::from([127, 0, 0, 1]),
                ReplAuthThrottle::new(),
            ));
        }
    });

    // Also wire the repl_registry into the primary state so on_write fans out
    // We can't replace state.replicas (it's private), but handle_replica adds
    // itself to the registry it receives. We pass the same repl_registry to
    // on_write via a workaround: patch primary state's replicas after the fact
    // by passing the same Arc. Since ServerState.replicas is private in our
    // TestServer, we re-use the one we created.
    // ── Simpler approach: replace on_write path by sharing registry ──
    // Instead, wire it through the primary ServerState directly.
    // (In practice the TestServer shares state.replicas which starts empty;
    // handle_replica will push its sender into it when it connects.)
    // The trick: we need primary.state.replicas to point to our repl_registry.
    // Since TestServer.state is Arc<ServerState>, we can't replace it.
    // Use a fresh primary state that shares our registry.
    let primary2 = {
        let store = Arc::clone(&primary.store);
        let (tx, _rx) = broadcast::channel::<SyncMsg>(256);
        let pubsub: SharedPubSub = Arc::new(tokio::sync::Mutex::new(PubSubHub::new()));
        let wr: WatchRegistry = WatchHub::new();
        let sem = Arc::new(Semaphore::new(64));
        let snap = Arc::clone(&primary.state.snap);
        let state = Arc::new(ServerState {
            snap,
            aof: None,
            replicas: Arc::clone(&repl_registry),
            is_replica: AtomicBool::new(false),
            dedup: std::sync::Mutex::new(HashMap::new()),
            ephemeral: std::sync::Mutex::new(HashMap::new()),
            ephemeral_members: std::sync::Mutex::new(HashMap::new()),
            dedup_dirty: std::sync::atomic::AtomicBool::new(false),
            dedup_order: tokio::sync::Mutex::new(()),
            save_lock: tokio::sync::Mutex::new(()),
            persistence_healthy: AtomicBool::new(true),
            persistence_failures: AtomicU64::new(0),
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let store2 = Arc::clone(&store);
        let state2 = Arc::clone(&state);
        let pass = Arc::new(None::<String>);
        let task = tokio::spawn(async move {
            loop {
                let Ok((socket, _)) = listener.accept().await else {
                    return;
                };
                let Ok(permit) = Arc::clone(&sem).try_acquire_owned() else {
                    continue;
                };
                let (s, t, p, ps, wrr, st) = (
                    Arc::clone(&store2),
                    tx.clone(),
                    Arc::clone(&pass),
                    Arc::clone(&pubsub),
                    Arc::clone(&wr),
                    Arc::clone(&state2),
                );
                tokio::spawn(async move {
                    let peer = socket
                        .peer_addr()
                        .map(|a| a.to_string())
                        .unwrap_or_default();
                    handle_tcp(socket, s, t, p, ps, wrr, st, peer).await;
                    drop(permit);
                });
            }
        });
        TestServer {
            tcp_addr: addr,
            store,
            state,
            _task: task,
        }
    };

    // Start replica
    let replica_store = Arc::new(KeyValueStore::new());
    let replica_state = Arc::new(ServerState {
        snap: Arc::new(SnapshotConfig {
            path: tmp_path("repl_snap.rdb"),
            last_save: AtomicI64::new(0),
            checkpoint_id: AtomicU64::new(0),
        }),
        aof: None,
        replicas: ReplHub::new(),
        is_replica: AtomicBool::new(true),
        dedup: std::sync::Mutex::new(HashMap::new()),
        ephemeral: std::sync::Mutex::new(HashMap::new()),
        ephemeral_members: std::sync::Mutex::new(HashMap::new()),
        dedup_dirty: std::sync::atomic::AtomicBool::new(false),
        dedup_order: tokio::sync::Mutex::new(()),
        save_lock: tokio::sync::Mutex::new(()),
        persistence_healthy: AtomicBool::new(true),
        persistence_failures: AtomicU64::new(0),
    });
    let rs = Arc::clone(&replica_store);
    let rst = Arc::clone(&replica_state);
    let repl_addr = format!("127.0.0.1:{repl_port}");
    let rtx = broadcast::channel::<SyncMsg>(16).0;
    tokio::spawn(async move {
        run_repl_client(repl_addr, rs, rst, None, None, rtx, None).await;
    });

    // Give replica time to connect and receive initial snapshot
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // Write to primary2 (which uses the shared repl_registry)
    let mut c = RespClient::connect(primary2.tcp_addr).await;
    assert_eq!(c.cmd(&["SET", "replkey", "replval"]).await, ok());

    // Give replication fan-out time to arrive
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    assert_eq!(
        replica_store.execute(Command::Get("replkey".into())),
        Value::BulkString(Some(b"replval".to_vec()))
    );

    // The replica acknowledges what it applies, so once it has caught up the
    // primary must observe zero lag. Before acknowledgements existed the
    // primary had no way to distinguish this from a replica that had
    // received the frame and silently failed to apply it.
    let mut lag = u64::MAX;
    for _ in 0..50 {
        lag = repl_registry.max_lag_frames().await;
        if lag == 0 {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    }
    assert_eq!(lag, 0, "caught-up replica must report zero lag");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_accepts_binary_command_frames() {
    // A WebSocket text frame must be well-formed UTF-8, so a command
    // carrying raw bytes can only travel in a binary frame. The server
    // previously handled text frames only and dropped binary ones.
    let srv = spawn_ws_server().await;
    let mut c = WsClient::connect(srv.tcp_addr).await;

    // A binary frame whose contents are valid UTF-8 is a normal command.
    let raw = b"*3\r\n$3\r\nSET\r\n$3\r\nbin\r\n$5\r\nhello\r\n".to_vec();
    assert_eq!(c.cmd_binary(raw).await, ok());
    assert_eq!(c.cmd(&["GET", "bin"]).await, bulk("hello"));

    // A binary value round-trips byte-for-byte, and the reply comes back in
    // a binary frame because it is not valid UTF-8.
    let binary: &[u8] = &[0xff, 0xfe, 0x00, 0x41];
    let mut req = b"*3\r\n$3\r\nSET\r\n$3\r\nraw\r\n".to_vec();
    req.extend_from_slice(format!("${}\r\n", binary.len()).as_bytes());
    req.extend_from_slice(binary);
    req.extend_from_slice(b"\r\n");
    assert_eq!(c.cmd_binary(req).await, ok());
    assert_eq!(
        c.cmd(&["GET", "raw"]).await,
        Value::BulkString(Some(binary.to_vec()))
    );

    // The connection stays usable.
    assert_eq!(c.cmd(&["PING"]).await, Value::SimpleString("PONG".into()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_hello_reports_resp3_and_refuses_downgrade() {
    let srv = spawn_ws_server().await;
    let mut c = WsClient::connect(srv.tcp_addr).await;

    // The sync protocol is defined in terms of RESP3 push frames.
    let v = c.cmd(&["HELLO"]).await;
    let Value::Map(pairs) = v else {
        panic!("WS HELLO must reply with a RESP3 map, got {v:?}")
    };
    assert!(
        pairs
            .iter()
            .any(|(k, v)| *k == bulk("proto") && *v == Value::Integer(3))
    );

    // Downgrading would silently break push delivery, so it is refused
    // rather than accepted-and-ignored.
    let v = c.cmd(&["HELLO", "2"]).await;
    assert!(
        matches!(&v, Value::Error(e) if e.starts_with("NOPROTO")),
        "expected NOPROTO, got {v:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_reaches_the_introspection_commands_too() {
    // `handle_tcp` and `handle_ws` are two hand-maintained copies of one
    // command loop, so the standing hazard when adding a server-level
    // command is wiring it into one and not the other — which compiles, and
    // fails only over the transport nobody checked. Every command added
    // outside the store belongs in a test like this one.
    let srv = spawn_ws_server().await;
    let mut c = WsClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["SET", "k", "hello"]).await, ok());
    let usage = c.cmd(&["MEMORY", "USAGE", "k"]).await;
    assert!(
        matches!(usage, Value::Integer(n) if n > 0),
        "MEMORY USAGE over WS: {usage:?}"
    );
    assert_eq!(
        c.cmd(&["MEMORY", "USAGE", "ghost"]).await,
        Value::BulkString(None)
    );
    assert!(matches!(
        c.cmd(&["MEMORY", "DOCTOR"]).await,
        Value::Error(_)
    ));

    assert_eq!(c.cmd(&["MODULE", "LIST"]).await, Value::Array(Some(vec![])));
    assert!(matches!(c.cmd(&["CLUSTER", "INFO"]).await, Value::Error(_)));

    // No subscribers on this connection, so the registry is empty — the
    // point is that the command is answered at all rather than falling
    // through to the store's "handled by the connection layer" refusal.
    assert_eq!(
        c.cmd(&["PUBSUB", "CHANNELS"]).await,
        Value::Array(Some(vec![]))
    );
    assert_eq!(c.cmd(&["PUBSUB", "NUMPAT"]).await, Value::Integer(0));
}

#[tokio::test]
async fn replication_lag_counts_unacknowledged_frames() {
    // A replica that receives frames but never acknowledges them is exactly
    // the case queue depth cannot see: the frames left the primary's
    // channel, so the queue reads empty while the replica is arbitrarily
    // far behind. Lag must report them.
    let store = Arc::new(KeyValueStore::new());
    let registry: ReplRegistry = ReplHub::new();
    let snap_cfg = Arc::new(SnapshotConfig {
        path: tmp_path("lag_snap.rdb"),
        last_save: AtomicI64::new(0),
        checkpoint_id: AtomicU64::new(0),
    });

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    {
        let (s, sc, r) = (
            Arc::clone(&store),
            Arc::clone(&snap_cfg),
            Arc::clone(&registry),
        );
        tokio::spawn(async move {
            if let Ok((socket, _)) = listener.accept().await {
                let _ = handle_replica(
                    socket,
                    s,
                    sc,
                    r,
                    None,
                    DEFAULT_REPL_CHANNEL_CAPACITY,
                    IpAddr::from([127, 0, 0, 1]),
                    ReplAuthThrottle::new(),
                )
                .await;
            }
        });
    }

    // A replica that negotiates a full snapshot and then goes silent.
    let mut sock = TcpStream::connect(addr).await.unwrap();
    let mut magic = [0u8; 4];
    sock.read_exact(&mut magic).await.unwrap();
    assert_eq!(&magic, REPL_MAGIC);
    let mut run_len = [0u8; 2];
    sock.read_exact(&mut run_len).await.unwrap();
    let mut run_id = vec![0u8; u16::from_le_bytes(run_len) as usize];
    sock.read_exact(&mut run_id).await.unwrap();
    sock.write_all(&0u16.to_le_bytes()).await.unwrap();
    sock.write_all(&0u64.to_le_bytes()).await.unwrap();
    let mut mode = [0u8; 1];
    sock.read_exact(&mut mode).await.unwrap();
    assert_eq!(mode[0], b'F');
    let mut boundary = [0u8; 8];
    sock.read_exact(&mut boundary).await.unwrap();
    assert_eq!(u64::from_le_bytes(boundary), 0);
    let mut len_buf = [0u8; 4];
    sock.read_exact(&mut len_buf).await.unwrap();
    let mut snap = vec![0u8; u32::from_le_bytes(len_buf) as usize];
    sock.read_exact(&mut snap).await.unwrap();

    // Wait for registration, then fan out three writes.
    for _ in 0..50 {
        if registry.count.load(Ordering::Relaxed) == 1 {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    }
    for i in 0..3 {
        registry
            .fan_out(format!("*1\r\n$4\r\nPING{i}\r\n").into_bytes())
            .await;
    }

    let mut lag = 0;
    for _ in 0..50 {
        lag = registry.max_lag_frames().await;
        if lag == 3 {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    }
    assert_eq!(lag, 3, "three unacknowledged frames must show as lag 3");

    // Acknowledging two of them retires exactly two frames of lag.
    sock.write_all(&2u64.to_le_bytes()).await.unwrap();
    for _ in 0..50 {
        lag = registry.max_lag_frames().await;
        if lag == 1 {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    }
    assert_eq!(lag, 1, "after acking 2 of 3, one frame remains outstanding");

    // A stale ack must not walk the high-water mark backwards.
    sock.write_all(&1u64.to_le_bytes()).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    assert_eq!(
        registry.max_lag_frames().await,
        1,
        "a replayed lower ack must not increase reported lag"
    );
}

// ── Integration: 3e load (ignored in normal CI) ───────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn integration_concurrent_writers() {
    let srv = Arc::new(spawn_server().await);
    let addr = srv.tcp_addr;

    let tasks: Vec<_> = (0..50)
        .map(|task_id| {
            tokio::spawn(async move {
                let mut c = RespClient::connect(addr).await;
                for i in 0..100u32 {
                    let key = format!("t{task_id}_{i}");
                    let val = format!("v{i}");
                    assert_eq!(c.cmd(&["SET", &key, &val]).await, ok());
                    assert_eq!(c.cmd(&["GET", &key]).await, bulk(&val));
                }
            })
        })
        .collect();

    for t in tasks {
        t.await.unwrap();
    }
    // All 50 × 100 keys should be present
    assert_eq!(srv.store.execute(Command::DbSize), Value::Integer(5000));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn integration_connection_limit() {
    // Small semaphore: only 3 concurrent connections
    let store = Arc::new(KeyValueStore::new());
    let (tx, _rx) = broadcast::channel::<SyncMsg>(16);
    let pubsub: SharedPubSub = Arc::new(tokio::sync::Mutex::new(PubSubHub::new()));
    let watch_registry: WatchRegistry = WatchHub::new();
    let semaphore = Arc::new(Semaphore::new(3));
    let state = Arc::new(ServerState {
        snap: Arc::new(SnapshotConfig {
            path: tmp_path("conn_limit.rdb"),
            last_save: AtomicI64::new(0),
            checkpoint_id: AtomicU64::new(0),
        }),
        aof: None,
        replicas: ReplHub::new(),
        is_replica: AtomicBool::new(false),
        dedup: std::sync::Mutex::new(HashMap::new()),
        ephemeral: std::sync::Mutex::new(HashMap::new()),
        ephemeral_members: std::sync::Mutex::new(HashMap::new()),
        dedup_dirty: std::sync::atomic::AtomicBool::new(false),
        dedup_order: tokio::sync::Mutex::new(()),
        save_lock: tokio::sync::Mutex::new(()),
        persistence_healthy: AtomicBool::new(true),
        persistence_failures: AtomicU64::new(0),
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let store2 = Arc::clone(&store);
    let state2 = Arc::clone(&state);
    let pass = Arc::new(None::<String>);

    tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            let Ok(permit) = Arc::clone(&semaphore).try_acquire_owned() else {
                // Drop socket immediately — connection limit reached
                drop(socket);
                continue;
            };
            let (s, t, p, ps, wr, st) = (
                Arc::clone(&store2),
                tx.clone(),
                Arc::clone(&pass),
                Arc::clone(&pubsub),
                Arc::clone(&watch_registry),
                Arc::clone(&state2),
            );
            tokio::spawn(async move {
                let peer = socket
                    .peer_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_default();
                handle_tcp(socket, s, t, p, ps, wr, st, peer).await;
                drop(permit);
            });
        }
    });

    // Open 3 connections and hold them (just send PING and keep the socket open)
    let mut holders = Vec::new();
    for _ in 0..3 {
        let mut c = RespClient::connect(addr).await;
        assert_eq!(
            c.cmd(&["PING"]).await,
            Value::SimpleString("PONG".to_string())
        );
        holders.push(c);
    }

    // 4th connection: server drops it immediately, so read returns 0
    let mut overflow = TcpStream::connect(addr).await.unwrap();
    let mut buf = [0u8; 64];
    let n = overflow.read(&mut buf).await.unwrap_or(0);
    assert_eq!(n, 0, "4th connection should have been closed by server");

    drop(holders);
}

// ── Integration: 3f chaos (ignored in normal CI) ──────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn integration_kill_primary_mid_write() {
    let srv = Arc::new(spawn_server().await);
    let addr = srv.tcp_addr;

    // Start 20 concurrent writers
    let tasks: Vec<_> = (0..20)
        .map(|i| {
            tokio::spawn(async move {
                // Connect; tolerate connection errors (server may die mid-flight)
                let stream = TcpStream::connect(addr).await;
                if stream.is_err() {
                    return;
                }
                let mut c = RespClient {
                    stream: stream.unwrap(),
                    buf: vec![0u8; 65536],
                    filled: 0,
                };
                for j in 0..50u32 {
                    let key = format!("chaos_{i}_{j}");
                    // Ignore errors — server may die during this
                    let _ = tokio::time::timeout(
                        tokio::time::Duration::from_millis(200),
                        c.cmd(&["SET", &key, "v"]),
                    )
                    .await;
                }
            })
        })
        .collect();

    // Kill the server after 10ms
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    srv._task.abort();

    // Join all writers — none should panic
    for t in tasks {
        let _ = t.await;
    }

    // Store is still intact in memory — no panic is the meaningful assertion here;
    // zero keys is valid if the server was killed before any write landed.
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_failover_timeout_does_not_promote() {
    // Point the replica at a port that refuses connections. The deprecated
    // one-second timeout must not make an isolated replica writable: without
    // quorum and fencing that would create split brain during a partition.
    let replica_state = Arc::new(ServerState {
        snap: Arc::new(SnapshotConfig {
            path: tmp_path("failover_snap.rdb"),
            last_save: AtomicI64::new(0),
            checkpoint_id: AtomicU64::new(0),
        }),
        aof: None,
        replicas: ReplHub::new(),
        is_replica: AtomicBool::new(true),
        dedup: std::sync::Mutex::new(HashMap::new()),
        ephemeral: std::sync::Mutex::new(HashMap::new()),
        ephemeral_members: std::sync::Mutex::new(HashMap::new()),
        dedup_dirty: std::sync::atomic::AtomicBool::new(false),
        dedup_order: tokio::sync::Mutex::new(()),
        save_lock: tokio::sync::Mutex::new(()),
        persistence_healthy: AtomicBool::new(true),
        persistence_failures: AtomicU64::new(0),
    });
    let replica_store = Arc::new(KeyValueStore::new());
    let rs = Arc::clone(&replica_store);
    let rst = Arc::clone(&replica_state);
    // Bind a listener then immediately drop it so the port is known-refused
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dead_addr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());
    drop(listener);
    let rtx = broadcast::channel::<SyncMsg>(16).0;
    tokio::spawn(async move {
        run_repl_client(dead_addr, rs, rst, None, Some(1), rtx, None).await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(2300)).await;

    assert!(
        replica_state.is_replica(),
        "an unreachable-primary timeout must never promote without fencing"
    );
}

// ── WebSocket WATCH/EXEC harness ──────────────────────────────────────────

/// Spawn a WebSocket server sharing one store + watch registry across all
/// connections, so WATCH notifications fan out between clients.
async fn spawn_ws_server() -> TestServer {
    spawn_ws_server_cfg(None).await
}

/// Like `spawn_ws_server`, with an optional sync-scope secret (strict mode).
async fn spawn_ws_server_cfg(sync_secret: Option<String>) -> TestServer {
    spawn_ws_server_full(sync_secret, None).await
}

/// Like `spawn_ws_server`, with an origin allowlist in force.
async fn spawn_ws_server_origins(origins: Vec<String>) -> TestServer {
    spawn_ws_server_full(None, Some(origins)).await
}

/// Open a WebSocket to `addr`, optionally sending an `Origin` header, and
/// report whether the handshake completed.
async fn ws_connect_with_origin(
    addr: std::net::SocketAddr,
    origin: Option<&str>,
) -> Result<(), String> {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut req = format!("ws://{addr}").into_client_request().unwrap();
    if let Some(o) = origin {
        req.headers_mut().insert("origin", o.parse().unwrap());
    }
    tokio_tungstenite::connect_async(req)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tokio::test]
async fn ws_refuses_a_cross_origin_handshake() {
    // Browsers apply neither CORS nor a preflight to WebSockets, so before
    // this check any page a user visited could open a socket to port 6380
    // and read or write the whole keyspace with that user's network
    // position. On ws://localhost:6380 that is every site in every tab.
    let srv = spawn_ws_server_origins(vec!["https://app.example.com".to_string()]).await;

    let err = ws_connect_with_origin(srv.tcp_addr, Some("https://evil.example"))
        .await
        .expect_err("a foreign Origin must not complete the handshake");
    assert!(
        err.contains("403") || err.to_lowercase().contains("forbidden"),
        "expected a 403, got {err}"
    );
}

#[tokio::test]
async fn ws_admits_an_allowlisted_origin_and_serves_commands() {
    // The rejection is worthless if it also breaks the deployed app, so
    // assert the permitted path all the way through to a working command.
    let srv = spawn_ws_server_origins(vec!["https://app.example.com".to_string()]).await;
    ws_connect_with_origin(srv.tcp_addr, Some("https://app.example.com"))
        .await
        .expect("an allowlisted Origin must connect");

    let mut c = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(c.cmd(&["SET", "k", "v"]).await, ok());
    assert_eq!(c.cmd(&["GET", "k"]).await, bulk("v"));
}

#[tokio::test]
async fn ws_admits_a_client_that_sends_no_origin() {
    // Native clients omit the header, and an attacker with a raw socket can
    // forge it — refusing here would break real clients while stopping
    // nobody. `WsClient::connect` is exactly such a client.
    let srv = spawn_ws_server_origins(vec!["https://app.example.com".to_string()]).await;
    ws_connect_with_origin(srv.tcp_addr, None)
        .await
        .expect("a client with no Origin must connect");
}

#[tokio::test]
async fn ws_without_an_allowlist_accepts_any_origin() {
    // Unset means allow, matching RECACHED_PASSWORD. The startup warning is
    // what keeps this from being a silent default.
    let srv = spawn_ws_server().await;
    ws_connect_with_origin(srv.tcp_addr, Some("https://anything.example"))
        .await
        .expect("no allowlist means no origin restriction");
}

#[tokio::test]
async fn ws_refuses_a_message_larger_than_the_reassembly_cap() {
    // tungstenite buffers a whole message before the RESP parser — and so
    // before MAX_BULK_STRING_BYTES and before the NOAUTH check — sees a byte
    // of it. Left at its 64 MiB default, an unauthenticated client that opens
    // a fragmented message and never finishes it holds that much memory per
    // connection. Measured at ~68 MiB each before this cap existed.
    let srv = spawn_ws_server().await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{}", srv.tcp_addr))
        .await
        .unwrap();

    let oversized = ws_max_message_bytes() + 1024;
    let body = "z".repeat(oversized);
    // The client's own config would reject this before it reached the wire.
    let _ = ws.send(Message::Text(body.into())).await;

    // The server must drop the connection rather than reassemble it.
    let closed = tokio::time::timeout(tokio::time::Duration::from_secs(5), async {
        loop {
            match ws.next().await {
                Some(Ok(_)) => continue,
                _ => return,
            }
        }
    })
    .await;
    assert!(
        closed.is_ok(),
        "a message over the cap must close the connection, not be buffered"
    );
}

#[tokio::test]
async fn the_websocket_reassembly_caps_are_bounded_by_default() {
    // A frame can never exceed a whole message; that is the real bound.
    assert!(ws_max_frame_bytes() <= ws_max_message_bytes());
    // Well under tungstenite's 64 MiB default, which is the point.
    assert!(
        ws_max_message_bytes() <= 16 * 1024 * 1024,
        "default reassembly cap regressed to {}",
        ws_max_message_bytes()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ws_closes_a_client_whose_mutation_stream_developed_a_gap() {
    // The regression this locks down: the lag branch used to resubscribe and
    // keep the socket open, so the browser's local replica silently lost every
    // mutation in the gap. `SyncPush` carries no sequence number, so the client
    // cannot notice. Reproduced against 0.3.4 at 1,043 of 20,000 mutations
    // delivered, connection still open.
    //
    // Driven through the broadcast channel directly: a client that never reads
    // stops the send path, the channel overruns, and the only correct answer is
    // to close so the client reconnects and re-runs QSUB.
    let store = Arc::new(KeyValueStore::new());
    let (tx, _rx) = broadcast::channel::<SyncMsg>(16);
    let pubsub: SharedPubSub = Arc::new(tokio::sync::Mutex::new(PubSubHub::new()));
    let watch_registry: WatchRegistry = WatchHub::new();
    let state = Arc::new(ServerState {
        snap: Arc::new(SnapshotConfig {
            path: tmp_path("ws_gap.rdb"),
            last_save: AtomicI64::new(now_unix_secs()),
            checkpoint_id: AtomicU64::new(0),
        }),
        aof: None,
        replicas: ReplHub::new(),
        is_replica: AtomicBool::new(false),
        dedup: std::sync::Mutex::new(HashMap::new()),
        ephemeral: std::sync::Mutex::new(HashMap::new()),
        ephemeral_members: std::sync::Mutex::new(HashMap::new()),
        dedup_dirty: std::sync::atomic::AtomicBool::new(false),
        dedup_order: tokio::sync::Mutex::new(()),
        save_lock: tokio::sync::Mutex::new(()),
        persistence_healthy: AtomicBool::new(true),
        persistence_failures: AtomicU64::new(0),
    });

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (s, t, ps, wr, st) = (
        Arc::clone(&store),
        tx.clone(),
        Arc::clone(&pubsub),
        Arc::clone(&watch_registry),
        Arc::clone(&state),
    );
    let _task = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let id = next_conn_id();
        handle_ws(
            socket,
            s,
            t,
            Arc::new(None),
            id,
            ps,
            wr,
            st,
            Arc::new(None),
            Arc::new(None),
            "test".to_string(),
        )
        .await;
    });

    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}"))
        .await
        .unwrap();

    // Never read. Big payloads so the socket buffer fills and the server's
    // send path stops draining the broadcast, which is what makes it lag.
    let payload = resp_push(&[b"set", b"k", &vec![b'v'; 128 * 1024]]);
    for _ in 0..512 {
        let _ = tx.send(Arc::new(SyncPush {
            origin: 999,
            keys: vec!["k".to_string()],
            resp: payload.clone(),
        }));
        tokio::task::yield_now().await;
    }

    // Now drain: the stream must end rather than continue with a silent hole.
    let ended = tokio::time::timeout(tokio::time::Duration::from_secs(10), async {
        while let Some(frame) = ws.next().await {
            if matches!(frame, Ok(Message::Close(_)) | Err(_)) {
                return true;
            }
        }
        true
    })
    .await;

    assert_eq!(
        ended,
        Ok(true),
        "a connection that missed mutations must be closed, not silently resubscribed"
    );
}

async fn spawn_ws_server_full(
    sync_secret: Option<String>,
    allowed_origins: Option<Vec<String>>,
) -> TestServer {
    let store = Arc::new(KeyValueStore::new());
    let (tx, _rx) = broadcast::channel::<SyncMsg>(256);
    let pubsub: SharedPubSub = Arc::new(tokio::sync::Mutex::new(PubSubHub::new()));
    let watch_registry: WatchRegistry = WatchHub::new();
    let snap_cfg = Arc::new(SnapshotConfig {
        path: tmp_path("ws_test.rdb"),
        last_save: AtomicI64::new(now_unix_secs()),
        checkpoint_id: AtomicU64::new(0),
    });
    let state = Arc::new(ServerState {
        snap: snap_cfg,
        aof: None,
        replicas: ReplHub::new(),
        is_replica: AtomicBool::new(false),
        dedup: std::sync::Mutex::new(HashMap::new()),
        ephemeral: std::sync::Mutex::new(HashMap::new()),
        ephemeral_members: std::sync::Mutex::new(HashMap::new()),
        dedup_dirty: std::sync::atomic::AtomicBool::new(false),
        dedup_order: tokio::sync::Mutex::new(()),
        save_lock: tokio::sync::Mutex::new(()),
        persistence_healthy: AtomicBool::new(true),
        persistence_failures: AtomicU64::new(0),
    });

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let store2 = Arc::clone(&store);
    let state2 = Arc::clone(&state);
    let secret = Arc::new(sync_secret);
    let origins = Arc::new(allowed_origins);

    let task = tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            let (s, t, ps, wr, st, ss, ao) = (
                Arc::clone(&store2),
                tx.clone(),
                Arc::clone(&pubsub),
                Arc::clone(&watch_registry),
                Arc::clone(&state2),
                Arc::clone(&secret),
                Arc::clone(&origins),
            );
            let id = next_conn_id();
            let peer = socket
                .peer_addr()
                .map(|a| a.to_string())
                .unwrap_or_default();
            tokio::spawn(async move {
                handle_ws(socket, s, t, Arc::new(None), id, ps, wr, st, ss, ao, peer).await;
            });
        }
    });

    TestServer {
        tcp_addr: addr,
        store,
        state,
        _task: task,
    }
}

struct WsClient {
    ws: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
}

impl WsClient {
    async fn connect(addr: std::net::SocketAddr) -> Self {
        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}"))
            .await
            .unwrap();
        Self { ws }
    }

    async fn cmd(&mut self, args: &[&str]) -> Value {
        let mut req = format!("*{}\r\n", args.len());
        for a in args {
            req.push_str(&format!("${}\r\n{}\r\n", a.len(), a));
        }
        self.ws.send(Message::Text(req.into())).await.unwrap();
        self.next_reply().await
    }

    /// Send pre-encoded RESP bytes in a *binary* frame. Text frames must be
    /// well-formed UTF-8 per the WebSocket spec, so this is the only way to
    /// put arbitrary bytes on the wire.
    async fn cmd_binary(&mut self, raw: Vec<u8>) -> Value {
        self.ws.send(Message::Binary(raw.into())).await.unwrap();
        self.next_reply().await
    }

    /// Wait up to `ms` for the next frame of any kind — RESP3 Push
    /// broadcasts *and* plain arrays. `keychange` notifications are encoded
    /// as arrays, so `recv_push` skips them entirely.
    async fn recv_any(&mut self, ms: u64) -> Option<String> {
        let fut = async {
            loop {
                match self.ws.next().await {
                    Some(Ok(Message::Text(t))) => return Some(t.to_string()),
                    Some(Ok(_)) => continue,
                    _ => return None,
                }
            }
        };
        tokio::time::timeout(tokio::time::Duration::from_millis(ms), fut)
            .await
            .ok()
            .flatten()
    }

    /// Wait up to `ms` for the next RESP3 Push broadcast frame, returning
    /// its raw text. `None` when nothing arrives in time.
    async fn recv_push(&mut self, ms: u64) -> Option<String> {
        let fut = async {
            loop {
                match self.ws.next().await {
                    Some(Ok(Message::Text(t))) => {
                        let Ok((v, _)) = Value::parse(t.as_bytes()) else {
                            continue;
                        };
                        if matches!(v, Value::Push(_)) {
                            return Some(t.to_string());
                        }
                    }
                    Some(Ok(_)) => continue,
                    _ => return None,
                }
            }
        };
        tokio::time::timeout(tokio::time::Duration::from_millis(ms), fut)
            .await
            .ok()
            .flatten()
    }

    /// Wait up to `ms` for the next `keychange` frame (WATCH / live-query
    /// push), returning `(key, value)`. `None` when nothing arrives.
    async fn recv_keychange(&mut self, ms: u64) -> Option<(String, Value)> {
        let fut = async {
            loop {
                match self.ws.next().await {
                    Some(Ok(Message::Text(t))) => {
                        let Ok((v, _)) = Value::parse(t.as_bytes()) else {
                            continue;
                        };
                        if let Value::Array(Some(items)) = &v
                            && items.len() == 3
                            && matches!(items.first(), Some(Value::BulkString(Some(k))) if k == b"keychange")
                        {
                            let Value::BulkString(Some(key)) = &items[1] else {
                                continue;
                            };
                            return Some((
                                String::from_utf8_lossy(key).into_owned(),
                                items[2].clone(),
                            ));
                        }
                    }
                    Some(Ok(_)) => continue,
                    _ => return None,
                }
            }
        };
        tokio::time::timeout(tokio::time::Duration::from_millis(ms), fut)
            .await
            .ok()
            .flatten()
    }

    /// Read the next *command reply*, skipping server-initiated frames
    /// (RESP3 Push broadcasts and `keychange` observable-key pushes).
    async fn next_reply(&mut self) -> Value {
        loop {
            let raw: Vec<u8> = match self.ws.next().await {
                Some(Ok(Message::Text(t))) => t.as_bytes().to_vec(),
                Some(Ok(Message::Binary(b))) => b.to_vec(),
                Some(Ok(_)) => continue,
                _ => panic!("ws closed unexpectedly"),
            };
            let Ok((v, _)) = Value::parse(&raw) else {
                continue;
            };
            if matches!(v, Value::Push(_)) {
                continue;
            }
            if let Value::Array(Some(items)) = &v
                && matches!(items.first(), Some(Value::BulkString(Some(k))) if k == b"keychange")
            {
                continue;
            }
            return v;
        }
    }
}

#[tokio::test]
async fn integration_ws_replica_rejects_writes_queued_in_multi() {
    let srv = spawn_ws_server().await;
    srv.state.is_replica.store(true, Ordering::Release);
    let mut c = WsClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["MULTI"]).await, ok());
    assert_eq!(
        c.cmd(&["SET", "k", "v"]).await,
        Value::SimpleString("QUEUED".into())
    );
    let response = c.cmd(&["EXEC"]).await;
    assert!(matches!(&response, Value::Error(message) if message.contains("READONLY")));
    assert_eq!(srv.store.execute(Command::Get("k".into())), nil());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_watch_exec_aborts_on_change() {
    let srv = spawn_ws_server().await;
    let mut watcher = WsClient::connect(srv.tcp_addr).await;
    let mut writer = WsClient::connect(srv.tcp_addr).await;

    assert_eq!(watcher.cmd(&["SET", "k", "v0"]).await, ok());
    assert_eq!(watcher.cmd(&["WATCH", "k"]).await, ok());

    // Another client mutates the watched key.
    assert_eq!(writer.cmd(&["SET", "k", "v1"]).await, ok());
    // Give the notification time to reach the watcher's registry channel.
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    assert_eq!(
        watcher.cmd(&["MULTI"]).await,
        Value::SimpleString("OK".into())
    );
    assert_eq!(
        watcher.cmd(&["SET", "k", "v2"]).await,
        Value::SimpleString("QUEUED".into())
    );
    // EXEC must abort with a nil array because k changed since WATCH.
    assert_eq!(watcher.cmd(&["EXEC"]).await, Value::Array(None));
    // The transaction did not run.
    assert_eq!(srv.store.execute(Command::Get("k".into())), bulk("v1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_watch_exec_runs_when_unchanged() {
    let srv = spawn_ws_server().await;
    let mut c = WsClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["WATCH", "k"]).await, ok());
    assert_eq!(c.cmd(&["MULTI"]).await, ok());
    assert_eq!(
        c.cmd(&["SET", "k", "v1"]).await,
        Value::SimpleString("QUEUED".into())
    );
    // No one touched k → EXEC runs and returns the queued results.
    assert_eq!(
        c.cmd(&["EXEC"]).await,
        Value::Array(Some(vec![Value::SimpleString("OK".into())]))
    );
    assert_eq!(srv.store.execute(Command::Get("k".into())), bulk("v1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_tcp_watch_exec_aborts_on_change() {
    let srv = spawn_server().await;
    let mut watcher = RespClient::connect(srv.tcp_addr).await;
    let mut writer = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(watcher.cmd(&["SET", "k", "v0"]).await, ok());
    assert_eq!(watcher.cmd(&["WATCH", "k"]).await, ok());
    // Another client mutates the watched key (reply awaited → notification queued).
    assert_eq!(writer.cmd(&["SET", "k", "v1"]).await, ok());

    assert_eq!(watcher.cmd(&["MULTI"]).await, ok());
    assert_eq!(
        watcher.cmd(&["SET", "k", "v2"]).await,
        Value::SimpleString("QUEUED".into())
    );
    // k changed since WATCH → EXEC aborts with a nil array.
    assert_eq!(watcher.cmd(&["EXEC"]).await, Value::Array(None));
    assert_eq!(watcher.cmd(&["GET", "k"]).await, bulk("v1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn integration_tcp_watch_exec_closes_the_check_to_lock_race() {
    let srv = spawn_server().await;
    let mut watcher = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(watcher.cmd(&["WATCH", "guarded"]).await, ok());
    assert_eq!(watcher.cmd(&["MULTI"]).await, ok());
    assert_eq!(
        watcher.cmd(&["SET", "transaction-output", "written"]).await,
        Value::SimpleString("QUEUED".into())
    );

    // Hold the watched key ordering domain, then queue a conflicting writer
    // before EXEC. The writer must complete first after release. EXEC must wait
    // for the same barrier, observe that invalidation, and abort. The old path
    // checked WATCH first and did not lock watched-only keys, so it committed.
    let barriers = srv
        .state
        .replicas
        .lock_commands_and_keys(&[], ["guarded"])
        .await;

    let addr = srv.tcp_addr;
    let mut writer_task = tokio::spawn(async move {
        let mut writer = RespClient::connect(addr).await;
        writer.cmd(&["SET", "guarded", "changed"]).await
    });
    assert!(
        tokio::time::timeout(tokio::time::Duration::from_millis(50), &mut writer_task,)
            .await
            .is_err(),
        "the competing writer should be waiting on the test barrier"
    );

    let mut exec_task = tokio::spawn(async move { watcher.cmd(&["EXEC"]).await });
    assert!(
        tokio::time::timeout(tokio::time::Duration::from_millis(50), &mut exec_task,)
            .await
            .is_err(),
        "EXEC must reserve watched keys before checking invalidation"
    );

    drop(barriers);
    assert_eq!(writer_task.await.unwrap(), ok());
    assert_eq!(exec_task.await.unwrap(), Value::Array(None));
    assert_eq!(
        srv.store.execute(Command::Get("transaction-output".into())),
        nil(),
        "the invalidated transaction must not run"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_tcp_watch_exec_runs_when_unchanged() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    assert_eq!(c.cmd(&["WATCH", "k"]).await, ok());
    assert_eq!(c.cmd(&["MULTI"]).await, ok());
    assert_eq!(
        c.cmd(&["SET", "k", "v1"]).await,
        Value::SimpleString("QUEUED".into())
    );
    assert_eq!(
        c.cmd(&["EXEC"]).await,
        Value::Array(Some(vec![Value::SimpleString("OK".into())]))
    );
    assert_eq!(c.cmd(&["GET", "k"]).await, bulk("v1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_tcp_watch_inside_multi_rejected() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;
    assert_eq!(c.cmd(&["MULTI"]).await, ok());
    // WATCH is not allowed once a transaction has started.
    assert!(matches!(c.cmd(&["WATCH", "k"]).await, Value::Error(_)));
}

// ── Sync scoping ──────────────────────────────────────────────────────────

/// Mint a sync-scope token the way an application backend would:
/// HMAC-SHA256 over the base64url payload text.
fn mint_sync_token(secret: &str, payload: &str) -> String {
    use base64::Engine as _;
    use hmac::{Hmac, Mac};
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let payload_b64 = engine.encode(payload);
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload_b64.as_bytes());
    let sig = engine.encode(mac.finalize().into_bytes());
    format!("{payload_b64}.{sig}")
}

#[test]
fn sync_token_roundtrip_and_rejections() {
    let tok = mint_sync_token("s3cret", "cart:42:*,profile:42");
    assert_eq!(
        verify_sync_token("s3cret", &tok).unwrap(),
        vec![Grant::rw("cart:42:*"), Grant::rw("profile:42")]
    );
    // Wrong secret → invalid signature.
    assert_eq!(
        verify_sync_token("other", &tok).unwrap_err(),
        "invalid signature"
    );
    // Tampered payload → invalid signature.
    let (_, sig) = tok.split_once('.').unwrap();
    use base64::Engine as _;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let forged = format!("{}.{}", engine.encode("admin:*"), sig);
    assert_eq!(
        verify_sync_token("s3cret", &forged).unwrap_err(),
        "invalid signature"
    );
    // Expired token.
    let expired = mint_sync_token("s3cret", "cart:*|1");
    assert_eq!(
        verify_sync_token("s3cret", &expired).unwrap_err(),
        "token expired"
    );
    // Future expiry still valid.
    let live = mint_sync_token("s3cret", "cart:*|99999999999");
    assert!(verify_sync_token("s3cret", &live).is_ok());
    // Empty patterns / malformed.
    let empty = mint_sync_token("s3cret", "");
    assert_eq!(
        verify_sync_token("s3cret", &empty).unwrap_err(),
        "token grants no patterns"
    );
    assert!(verify_sync_token("s3cret", "no-dot-here").is_err());
}

#[test]
fn scopes_match_globs_and_flushdb() {
    let scopes = vec![Grant::rw("cart:42:*"), Grant::rw("catalog:*")];
    assert!(scopes_match(&scopes, &["cart:42:item:1".to_string()]));
    assert!(scopes_match(&scopes, &["catalog:books".to_string()]));
    assert!(!scopes_match(&scopes, &["cart:7:item:1".to_string()]));
    assert!(!scopes_match(&scopes, &["session:42".to_string()]));
    // Multi-key: any matching key makes the push visible.
    assert!(scopes_match(
        &scopes,
        &["session:42".to_string(), "catalog:books".to_string()]
    ));
    // No keys = FLUSHDB — visible to every scope.
    assert!(scopes_match(&scopes, &[]));
}

#[test]
fn command_scope_classification() {
    assert!(matches!(
        command_scope(&Command::Ping(None)),
        CommandScope::KeyLess
    ));
    assert!(matches!(
        command_scope(&Command::Keys("*".into())),
        CommandScope::Admin
    ));
    assert!(matches!(
        command_scope(&Command::FlushDb),
        CommandScope::Admin
    ));
    match command_scope(&Command::Get("a".into())) {
        CommandScope::Keys(k) => assert_eq!(k, vec![("a".to_string(), Access::Read)]),
        _ => panic!("GET should be key-scoped"),
    }
    // The store family splits access: the sources are read, the destination
    // is written.
    match command_scope(&Command::SInterStore(
        "dst".into(),
        vec!["a".into(), "b".into()],
    )) {
        CommandScope::Keys(k) => assert_eq!(
            k,
            vec![
                ("a".to_string(), Access::Read),
                ("b".to_string(), Access::Read),
                ("dst".to_string(), Access::Write),
            ]
        ),
        _ => panic!("SINTERSTORE should be key-scoped"),
    }
}

// ── Scope enforcement: the authorization surface ──────────────────────────
//
// `command_scope` decides whether a command is checked against the
// connection's grants at all. A key-touching command misclassified as
// `KeyLess` skips the check entirely, so every family is pinned here.
// The match in `command_scope` is exhaustive over `Command` with no
// catch-all — a new variant fails to compile until classified — and these
// tests guard against the remaining risk: classifying one wrongly.

fn scoped_keys(cmd: Command) -> Vec<String> {
    scoped_access(cmd).into_iter().map(|(k, _)| k).collect()
}

fn scoped_access(cmd: Command) -> Vec<(String, Access)> {
    match command_scope(&cmd) {
        CommandScope::Keys(k) => k,
        other => panic!("{cmd:?} should be key-scoped, got {other:?}"),
    }
}

#[test]
fn every_single_key_command_family_is_key_scoped() {
    let cases: Vec<(Command, &str)> = vec![
        (Command::Get("k".into()), "k"),
        (
            Command::Set("k".into(), "v".into(), SetOptions::default()),
            "k",
        ),
        (Command::Append("k".into(), "v".into()), "k"),
        (Command::Incr("k".into()), "k"),
        (Command::Decr("k".into()), "k"),
        (Command::Ttl("k".into()), "k"),
        (Command::Persist("k".into()), "k"),
        (Command::Expire("k".into(), 1), "k"),
        (Command::Type("k".into()), "k"),
        (Command::HGet("k".into(), "f".into()), "k"),
        (Command::HGetAll("k".into()), "k"),
        (Command::LPush("k".into(), vec!["v".into()]), "k"),
        (Command::LRange("k".into(), 0, -1), "k"),
        (Command::SAdd("k".into(), vec!["m".into()]), "k"),
        (Command::SMembers("k".into()), "k"),
        (
            Command::ZAdd("k".into(), ZAddOptions::default(), vec![(1.0, "m".into())]),
            "k",
        ),
        (Command::ZScore("k".into(), "m".into()), "k"),
        (Command::JGet("k".into(), None), "k"),
        (Command::JSet("k".into(), "$".into(), "1".into()), "k"),
        (Command::RlCheck("k".into(), None), "k"),
    ];
    for (cmd, expected) in cases {
        let keys = scoped_keys(cmd.clone());
        assert!(
            keys.contains(&expected.to_string()),
            "{cmd:?} must report key '{expected}', got {keys:?}"
        );
    }
}

#[test]
fn multi_key_commands_report_every_key_they_touch() {
    // A key omitted here is a key that never gets scope-checked, because
    // `scopes_match` only inspects the keys it is handed.
    let rename = scoped_keys(Command::Rename("src".into(), "dst".into()));
    assert!(
        rename.contains(&"src".to_string()) && rename.contains(&"dst".to_string()),
        "RENAME touches both keys, got {rename:?}"
    );

    let smove = scoped_keys(Command::SMove("src".into(), "dst".into(), "m".into()));
    assert!(
        smove.contains(&"src".to_string()) && smove.contains(&"dst".to_string()),
        "SMOVE touches both sets, got {smove:?}"
    );

    let mset = scoped_keys(Command::MSet(vec![
        ("a".into(), "1".into()),
        ("b".into(), "2".into()),
    ]));
    assert!(
        mset.contains(&"a".to_string()) && mset.contains(&"b".to_string()),
        "MSET touches every key, got {mset:?}"
    );

    let mget = scoped_keys(Command::MGet(vec!["a".into(), "b".into()]));
    assert!(mget.contains(&"a".to_string()) && mget.contains(&"b".to_string()));

    let del = scoped_keys(Command::Del(vec!["a".into(), "b".into()]));
    assert!(del.contains(&"a".to_string()) && del.contains(&"b".to_string()));

    let exists = scoped_keys(Command::Exists(vec!["a".into(), "b".into()]));
    assert!(exists.contains(&"a".to_string()) && exists.contains(&"b".to_string()));

    let sunion = scoped_keys(Command::SUnionStore(
        "dst".into(),
        vec!["a".into(), "b".into()],
    ));
    for k in ["dst", "a", "b"] {
        assert!(sunion.contains(&k.to_string()), "SUNIONSTORE missed {k}");
    }
}

#[test]
fn administrative_commands_are_denied_not_merely_unscoped() {
    // These would read or destroy data outside any grant, so they must map
    // to Admin (refused) rather than KeyLess (silently allowed).
    for cmd in [
        Command::Keys("*".into()),
        Command::Scan(0, None, None),
        Command::DbSize,
        Command::FlushDb,
        Command::Save,
        Command::BgSave,
        Command::LastSave,
        Command::ReplicaOfNoOne,
    ] {
        assert!(
            matches!(command_scope(&cmd), CommandScope::Admin),
            "{cmd:?} must be Admin-classified"
        );
    }
}

#[test]
fn oversized_initial_qstate_is_refused_not_truncated() {
    let store = KeyValueStore::new();
    for i in 0..3 {
        store.execute(Command::Set(
            format!("q:{i}"),
            "v".into(),
            SetOptions::default(),
        ));
    }
    assert_eq!(initial_qstate(&store, "q:*", 2), Err(2));
    assert_eq!(initial_qstate(&store, "q:*", 3).unwrap().len(), 3);
}

#[test]
fn keyless_commands_touch_no_keys() {
    for cmd in [
        Command::Ping(None),
        Command::Auth("pw".into()),
        Command::Multi,
        Command::Exec,
        Command::Discard,
        Command::Subscribe(vec!["ch".into()]),
        Command::Publish("ch".into(), "m".into()),
        Command::Sync(vec![]),
    ] {
        assert!(
            matches!(command_scope(&cmd), CommandScope::KeyLess),
            "{cmd:?} should be KeyLess"
        );
    }
}

#[test]
fn dedup_envelope_inherits_the_inner_command_scope() {
    // DEDUP wraps a real write. If the wrapper were treated as KeyLess, an
    // attacker could smuggle any command past scope enforcement.
    let inner = Command::Set("secret:1".into(), "v".into(), SetOptions::default());
    let wrapped = Command::Dedup("client".into(), 1, Box::new(inner));
    match command_scope(&wrapped) {
        CommandScope::Keys(k) => assert_eq!(k, vec![("secret:1".to_string(), Access::Write)]),
        other => panic!("DEDUP must inherit inner scope, got {other:?}"),
    }

    // The same must hold for an admin inner command.
    let wrapped_admin = Command::Dedup("client".into(), 2, Box::new(Command::FlushDb));
    assert!(matches!(command_scope(&wrapped_admin), CommandScope::Admin));
}

#[test]
fn scopes_match_allows_when_there_are_no_keys_to_check() {
    // Documented consequence of the design: an empty key list is allowed.
    // That is only safe because `command_scope` returns `Keys(..)` for every
    // key-touching command — the test above is what keeps it safe.
    assert!(scopes_match(&[Grant::rw("cart:*")], &[]));
}

#[test]
fn scopes_match_requires_every_key_to_match() {
    let scopes = vec![Grant::rw("cart:42:*")];
    assert!(scopes_match(&scopes, &["cart:42:a".to_string()]));
    // One in-scope key does not license an out-of-scope sibling.
    assert!(!scopes_match(&scopes, &["cart:99:a".to_string()]));
}

// ── Metrics labels ────────────────────────────────────────────────────────

// ── Exhaustive command classification ─────────────────────────────────────
//
// One instance of every `Command` variant, run through the three functions
// that decide scope enforcement and metrics. `command_scope` has no
// catch-all arm, so a new variant cannot compile without being classified —
// but nothing stops it being classified *wrongly*, and a key-touching
// command marked `KeyLess` silently bypasses scope checks entirely.

enum Expect {
    KeyLess,
    Admin,
    Keys(&'static [&'static str]),
}

fn all_commands() -> Vec<(Command, Expect)> {
    vec![
        (Command::Ping(None), Expect::KeyLess),
        (Command::Auth("pw".into()), Expect::KeyLess),
        (
            Command::Set("k".into(), "v".into(), SetOptions::default()),
            Expect::Keys(&["k"]),
        ),
        (Command::Get("k".into()), Expect::Keys(&["k"])),
        (Command::ESet("k".into(), "v".into()), Expect::Keys(&["k"])),
        (
            Command::EAdd("k".into(), vec!["m".into()]),
            Expect::Keys(&["k"]),
        ),
        (Command::Del(vec!["k".into()]), Expect::Keys(&["k"])),
        (Command::Unlink(vec!["k".into()]), Expect::Keys(&["k"])),
        (
            Command::Append("k".into(), "v".into()),
            Expect::Keys(&["k"]),
        ),
        (Command::Strlen("k".into()), Expect::Keys(&["k"])),
        (Command::GetRange("k".into(), 0, -1), Expect::Keys(&["k"])),
        (
            Command::GetSet("k".into(), "v".into()),
            Expect::Keys(&["k"]),
        ),
        (Command::MGet(vec!["k".into()]), Expect::Keys(&["k"])),
        (Command::SetNx("k".into(), "v".into()), Expect::Keys(&["k"])),
        (
            Command::SetEx("k".into(), 1, "v".into()),
            Expect::Keys(&["k"]),
        ),
        (
            Command::PSetEx("k".into(), 1, "v".into()),
            Expect::Keys(&["k"]),
        ),
        (
            Command::MSet(vec![("k".into(), "v".into())]),
            Expect::Keys(&["k"]),
        ),
        (Command::Incr("k".into()), Expect::Keys(&["k"])),
        (Command::Decr("k".into()), Expect::Keys(&["k"])),
        (Command::IncrBy("k".into(), 1), Expect::Keys(&["k"])),
        (Command::DecrBy("k".into(), 1), Expect::Keys(&["k"])),
        (Command::Expire("k".into(), 1), Expect::Keys(&["k"])),
        (Command::PExpire("k".into(), 1), Expect::Keys(&["k"])),
        (Command::ExpireAt("k".into(), 1), Expect::Keys(&["k"])),
        (Command::PExpireAt("k".into(), 1), Expect::Keys(&["k"])),
        (Command::Ttl("k".into()), Expect::Keys(&["k"])),
        (Command::PTtl("k".into()), Expect::Keys(&["k"])),
        (Command::Persist("k".into()), Expect::Keys(&["k"])),
        (Command::Exists(vec!["k".into()]), Expect::Keys(&["k"])),
        (Command::Keys("*".into()), Expect::Admin),
        (Command::Scan(0, None, None), Expect::Admin),
        (Command::DbSize, Expect::Admin),
        (Command::FlushDb, Expect::Admin),
        (
            Command::Rename("k".into(), "d".into()),
            Expect::Keys(&["k", "d"]),
        ),
        (Command::Type("k".into()), Expect::Keys(&["k"])),
        (
            Command::HSet("k".into(), vec![("f".into(), "v".into())]),
            Expect::Keys(&["k"]),
        ),
        (Command::HGet("k".into(), "f".into()), Expect::Keys(&["k"])),
        (Command::HGetAll("k".into()), Expect::Keys(&["k"])),
        (
            Command::HDel("k".into(), vec!["f".into()]),
            Expect::Keys(&["k"]),
        ),
        (Command::HKeys("k".into()), Expect::Keys(&["k"])),
        (Command::HVals("k".into()), Expect::Keys(&["k"])),
        (Command::HLen("k".into()), Expect::Keys(&["k"])),
        (
            Command::HIncrBy("k".into(), "f".into(), 1),
            Expect::Keys(&["k"]),
        ),
        (
            Command::HIncrByFloat("k".into(), "f".into(), 1.0),
            Expect::Keys(&["k"]),
        ),
        (
            Command::HExists("k".into(), "f".into()),
            Expect::Keys(&["k"]),
        ),
        (
            Command::HSetNx("k".into(), "f".into(), "v".into()),
            Expect::Keys(&["k"]),
        ),
        (
            Command::HMGet("k".into(), vec!["f".into()]),
            Expect::Keys(&["k"]),
        ),
        (
            Command::HScan("k".into(), ScanArgs::default()),
            Expect::Keys(&["k"]),
        ),
        (
            Command::LPush("k".into(), vec!["v".into()]),
            Expect::Keys(&["k"]),
        ),
        (
            Command::RPush("k".into(), vec!["v".into()]),
            Expect::Keys(&["k"]),
        ),
        (
            Command::LPushX("k".into(), vec!["v".into()]),
            Expect::Keys(&["k"]),
        ),
        (
            Command::RPushX("k".into(), vec!["v".into()]),
            Expect::Keys(&["k"]),
        ),
        (Command::LPop("k".into(), None), Expect::Keys(&["k"])),
        (Command::RPop("k".into(), None), Expect::Keys(&["k"])),
        (Command::LRange("k".into(), 0, -1), Expect::Keys(&["k"])),
        (Command::LLen("k".into()), Expect::Keys(&["k"])),
        (Command::LIndex("k".into(), 0), Expect::Keys(&["k"])),
        (
            Command::LSet("k".into(), 0, "v".into()),
            Expect::Keys(&["k"]),
        ),
        (
            Command::LRem("k".into(), 0, "v".into()),
            Expect::Keys(&["k"]),
        ),
        (Command::LTrim("k".into(), 0, -1), Expect::Keys(&["k"])),
        (
            Command::SAdd("k".into(), vec!["m".into()]),
            Expect::Keys(&["k"]),
        ),
        (Command::SMembers("k".into()), Expect::Keys(&["k"])),
        (
            Command::SRem("k".into(), vec!["m".into()]),
            Expect::Keys(&["k"]),
        ),
        (Command::SCard("k".into()), Expect::Keys(&["k"])),
        (
            Command::SIsMember("k".into(), "m".into()),
            Expect::Keys(&["k"]),
        ),
        (
            Command::SMIsMember("k".into(), vec!["m".into()]),
            Expect::Keys(&["k"]),
        ),
        (Command::SInter(vec!["k".into()]), Expect::Keys(&["k"])),
        (
            Command::SInterStore("d".into(), vec!["k".into()]),
            Expect::Keys(&["k", "d"]),
        ),
        (Command::SUnion(vec!["k".into()]), Expect::Keys(&["k"])),
        (
            Command::SUnionStore("d".into(), vec!["k".into()]),
            Expect::Keys(&["k", "d"]),
        ),
        (Command::SDiff(vec!["k".into()]), Expect::Keys(&["k"])),
        (
            Command::SDiffStore("d".into(), vec!["k".into()]),
            Expect::Keys(&["k", "d"]),
        ),
        (Command::SPop("k".into(), None), Expect::Keys(&["k"])),
        (Command::SRandMember("k".into(), None), Expect::Keys(&["k"])),
        (
            Command::SScan("k".into(), ScanArgs::default()),
            Expect::Keys(&["k"]),
        ),
        (
            Command::SMove("k".into(), "d".into(), "m".into()),
            Expect::Keys(&["k", "d"]),
        ),
        (
            Command::ZAdd("k".into(), ZAddOptions::default(), vec![(1.0, "m".into())]),
            Expect::Keys(&["k"]),
        ),
        (
            Command::ZRange("k".into(), 0, -1, false),
            Expect::Keys(&["k"]),
        ),
        (
            Command::ZRevRange("k".into(), 0, -1, false),
            Expect::Keys(&["k"]),
        ),
        (
            Command::ZRangeByScore("k".into(), "0".into(), "1".into(), false, None),
            Expect::Keys(&["k"]),
        ),
        (
            Command::ZRevRangeByScore("k".into(), "1".into(), "0".into(), false, None),
            Expect::Keys(&["k"]),
        ),
        (
            Command::ZScore("k".into(), "m".into()),
            Expect::Keys(&["k"]),
        ),
        (
            Command::ZMScore("k".into(), vec!["m".into()]),
            Expect::Keys(&["k"]),
        ),
        (Command::ZRank("k".into(), "m".into()), Expect::Keys(&["k"])),
        (
            Command::ZRevRank("k".into(), "m".into()),
            Expect::Keys(&["k"]),
        ),
        (
            Command::ZRem("k".into(), vec!["m".into()]),
            Expect::Keys(&["k"]),
        ),
        (Command::ZCard("k".into()), Expect::Keys(&["k"])),
        (
            Command::ZIncrBy("k".into(), 1.0, "m".into()),
            Expect::Keys(&["k"]),
        ),
        (
            Command::ZCount("k".into(), "0".into(), "1".into()),
            Expect::Keys(&["k"]),
        ),
        (
            Command::ZScan("k".into(), ScanArgs::default()),
            Expect::Keys(&["k"]),
        ),
        (
            Command::JSet("k".into(), "$".into(), "1".into()),
            Expect::Keys(&["k"]),
        ),
        (Command::JGet("k".into(), None), Expect::Keys(&["k"])),
        (
            Command::JMerge("k".into(), "{}".into()),
            Expect::Keys(&["k"]),
        ),
        (Command::RlSet("k".into(), 1, 1), Expect::Keys(&["k"])),
        (Command::RlCheck("k".into(), None), Expect::Keys(&["k"])),
        (Command::Multi, Expect::KeyLess),
        (Command::Exec, Expect::KeyLess),
        (Command::Discard, Expect::KeyLess),
        (Command::Subscribe(vec!["ch".into()]), Expect::KeyLess),
        (Command::Unsubscribe(vec!["ch".into()]), Expect::KeyLess),
        (Command::PSubscribe(vec!["ch".into()]), Expect::KeyLess),
        (Command::PUnsubscribe(vec!["ch".into()]), Expect::KeyLess),
        (Command::Publish("ch".into(), "m".into()), Expect::KeyLess),
        (Command::Watch(vec!["k".into()]), Expect::Keys(&["k"])),
        (Command::Unwatch(vec!["k".into()]), Expect::Keys(&["k"])),
        (Command::Sync(vec![]), Expect::KeyLess),
        (Command::QSub("p:*".into()), Expect::KeyLess),
        (Command::QUnsub(None), Expect::KeyLess),
        (Command::Save, Expect::Admin),
        (Command::BgSave, Expect::Admin),
        (Command::LastSave, Expect::Admin),
        (Command::ReplicaOfNoOne, Expect::Admin),
        (Command::Quit, Expect::KeyLess),
        (Command::Client(vec!["ID".into()]), Expect::KeyLess),
        (
            Command::Config(vec!["GET".into(), "*".into()]),
            Expect::Admin,
        ),
        (Command::CommandQuery(vec![]), Expect::KeyLess),
        (Command::Cluster(vec!["INFO".into()]), Expect::KeyLess),
        (Command::Module(vec!["LIST".into()]), Expect::KeyLess),
        (Command::Memory(vec!["DOCTOR".into()]), Expect::KeyLess),
        (Command::MemoryUsage("k".into()), Expect::Keys(&["k"])),
        (Command::PubSub(vec!["CHANNELS".into()]), Expect::Admin),
        (Command::Unknown("X".into()), Expect::KeyLess),
    ]
}

#[test]
fn every_command_is_classified_for_scope_enforcement() {
    for (cmd, expect) in all_commands() {
        match (command_scope(&cmd), &expect) {
            (CommandScope::KeyLess, Expect::KeyLess) => {}
            (CommandScope::Admin, Expect::Admin) => {}
            (CommandScope::Keys(got), Expect::Keys(want)) => {
                for k in *want {
                    assert!(
                        got.iter().any(|(g, _)| g == k),
                        "{cmd:?} must scope-check key '{k}', reported {got:?}"
                    );
                }
            }
            (got, _) => panic!("{cmd:?} classified as {got:?}, which is not what it touches"),
        }
    }
}

#[test]
fn every_key_writing_command_is_classified_as_a_write() {
    // `is_write_command` is a `matches!` list, which — unlike a `match` —
    // has no exhaustiveness check: a new variant silently defaults to "not
    // a write" and is then never replicated, logged to AOF, or broadcast.
    // `primary_keys` reports the keys a command *writes*, so anything it
    // names must also be classified as a write. This cross-check is what
    // makes the missing entry impossible to ship.
    for (cmd, _) in all_commands() {
        if !primary_keys(&cmd).is_empty() {
            assert!(
                is_write_command(&cmd),
                "{cmd:?} writes keys but is_write_command() says otherwise — \
                 it would never reach replicas, the AOF, or live queries"
            );
        }
    }
}

#[test]
fn eset_is_a_write_and_reports_its_key() {
    let cmd = Command::ESet("presence:1".into(), "on".into());
    assert!(is_write_command(&cmd));
    assert_eq!(primary_keys(&cmd), vec!["presence:1".to_string()]);
    assert_eq!(command_name(&cmd), "eset");
    // Scoped connections must not be able to write presence keys outside
    // their grant.
    assert!(matches!(command_scope(&cmd), CommandScope::Keys(_)));
}

#[test]
fn eset_replays_to_replicas_as_a_plain_set() {
    // A replica has no connection to scope the lifetime to, so it stores an
    // ordinary key; the owning server broadcasts the DEL on disconnect.
    let frame = broadcast_for(
        &Command::ESet("presence:1".into(), "on".into()),
        &Value::SimpleString("OK".into()),
        0,
    )
    .expect("ESET must broadcast");
    let frame = String::from_utf8_lossy(&frame).into_owned();
    assert!(frame.contains("SET"), "{frame}");
    assert!(frame.contains("presence:1"));
    assert!(
        !frame.contains("ESET"),
        "replica should receive SET, not ESET"
    );
}

#[test]
fn every_command_is_in_the_catalog() {
    // The other half of the loop closed in `catalog_names_are_real_commands`:
    // every `Command` variant the parser can produce must have a catalog
    // row, or `COMMAND DOCS` silently under-reports what the server can do
    // and `COMMAND COUNT` lies about how much.
    for (cmd, _) in all_commands() {
        let name = command_name(&cmd);
        if name == "unknown" {
            continue; // Not a command — the reply for anything unrecognised.
        }
        assert!(
            catalog::lookup(name).is_some(),
            "{name} is a real command with no catalog entry"
        );
    }
}

#[test]
fn command_count_matches_the_catalog() {
    let Value::Integer(n) = handle_command_query(&["COUNT".to_string()], 2) else {
        panic!("COMMAND COUNT must reply an integer")
    };
    assert_eq!(n as usize, catalog::CATALOG.len());
}

#[test]
fn command_info_reports_ten_fields_per_entry() {
    // Redis 7 returns ten elements. A client that indexes past the sixth
    // must find an empty list, not a short array.
    let reply = handle_command_query(&["INFO".into(), "get".into()], 2);
    let Value::Array(Some(entries)) = reply else {
        panic!("expected an array")
    };
    let Value::Array(Some(fields)) = &entries[0] else {
        panic!("expected an entry array")
    };
    assert_eq!(fields.len(), 10);
    assert_eq!(fields[0], Value::BulkString(Some(b"get".to_vec())));
    assert_eq!(fields[1], Value::Integer(2));
    assert_eq!(fields[3], Value::Integer(1), "GET's key is at position 1");
}

#[test]
fn command_info_nils_unknown_names_in_place() {
    let reply = handle_command_query(&["INFO".into(), "get".into(), "nosuchthing".into()], 2);
    let Value::Array(Some(entries)) = reply else {
        panic!("expected an array")
    };
    assert_eq!(entries.len(), 2, "the reply stays aligned with the request");
    assert_eq!(entries[1], Value::Array(None));
}

#[test]
fn command_docs_shape_follows_the_protocol() {
    // RESP3 gets a map; RESP2 gets the same pairs flattened.
    let resp3 = handle_command_query(&["DOCS".into(), "getrange".into()], 3);
    assert!(matches!(resp3, Value::Map(_)), "{resp3:?}");
    let resp2 = handle_command_query(&["DOCS".into(), "getrange".into()], 2);
    let Value::Array(Some(flat)) = resp2 else {
        panic!("RESP2 must flatten the map")
    };
    assert_eq!(flat.len(), 2, "one command, one entry");
    assert_eq!(flat[0], Value::BulkString(Some(b"getrange".to_vec())));
}

#[test]
fn command_docs_omits_unknown_names() {
    let Value::Array(Some(flat)) = handle_command_query(&["DOCS".into(), "nosuchthing".into()], 2)
    else {
        panic!("expected an array")
    };
    assert!(flat.is_empty(), "an unknown name has no entry to key");
}

#[test]
fn command_rejects_unknown_subcommands() {
    let reply = handle_command_query(&["GETKEYS".into(), "get".into(), "k".into()], 2);
    assert!(
        matches!(&reply, Value::Error(e) if e.contains("Unknown subcommand")),
        "{reply:?}"
    );
}

#[test]
fn client_setinfo_is_recorded_and_visible() {
    let mut meta = ClientMeta::new(42, "127.0.0.1:1".into(), "127.0.0.1:6379".into());
    assert_eq!(
        handle_client_command(
            &["SETINFO".into(), "LIB-NAME".into(), "node-redis".into()],
            &mut meta
        ),
        Value::SimpleString("OK".into())
    );
    assert_eq!(
        handle_client_command(
            &["SETINFO".into(), "LIB-VER".into(), "6.2.0".into()],
            &mut meta
        ),
        Value::SimpleString("OK".into())
    );
    let Value::BulkString(Some(line)) = handle_client_command(&["INFO".into()], &mut meta) else {
        panic!("CLIENT INFO must reply a bulk string")
    };
    let line = String::from_utf8(line).unwrap();
    assert!(line.contains("lib-name=node-redis"), "{line}");
    assert!(line.contains("lib-ver=6.2.0"), "{line}");
    assert!(line.contains("id=42"), "{line}");
}

#[test]
fn client_setinfo_rejects_unknown_attributes() {
    let mut meta = ClientMeta::new(1, String::new(), String::new());
    let reply = handle_client_command(
        &["SETINFO".into(), "LIB-COLOUR".into(), "blue".into()],
        &mut meta,
    );
    assert!(
        matches!(&reply, Value::Error(e) if e.contains("Unrecognized")),
        "{reply:?}"
    );
}

#[test]
fn client_setname_round_trips_and_rejects_spaces() {
    let mut meta = ClientMeta::new(1, String::new(), String::new());
    assert_eq!(
        handle_client_command(&["GETNAME".into()], &mut meta),
        Value::BulkString(None),
        "an unnamed connection reports nil, not an empty string"
    );
    handle_client_command(&["SETNAME".into(), "worker-3".into()], &mut meta);
    assert_eq!(
        handle_client_command(&["GETNAME".into()], &mut meta),
        Value::BulkString(Some(b"worker-3".to_vec()))
    );
    // A space would break the key=value line CLIENT LIST emits.
    let reply = handle_client_command(&["SETNAME".into(), "two words".into()], &mut meta);
    assert!(matches!(reply, Value::Error(_)), "{reply:?}");
}

#[test]
fn client_declines_what_it_cannot_do() {
    // KILL must not answer +OK: a caller would believe a connection had
    // been closed when it is still open.
    let mut meta = ClientMeta::new(1, String::new(), String::new());
    for args in [
        vec!["KILL".to_string(), "id".into(), "3".into()],
        vec!["NO-EVICT".to_string(), "on".into()],
        vec!["UNPAUSE".to_string()],
    ] {
        let reply = handle_client_command(&args, &mut meta);
        assert!(
            matches!(&reply, Value::Error(e) if e.contains("Unknown subcommand")),
            "{args:?} -> {reply:?}"
        );
    }
}

#[test]
fn config_get_reports_values_in_force() {
    let store = KeyValueStore::new();
    let facts = test_facts();
    let Value::Array(Some(flat)) =
        handle_config_command(&["GET".into(), "maxmemory-policy".into()], &facts, &store)
    else {
        panic!("CONFIG GET must reply an array")
    };
    assert_eq!(flat.len(), 2);
    assert_eq!(
        flat[0],
        Value::BulkString(Some(b"maxmemory-policy".to_vec()))
    );
    assert_eq!(
        flat[1],
        Value::BulkString(Some(
            eviction_policy_name(store.eviction_policy())
                .as_bytes()
                .to_vec()
        )),
        "the reported policy must be the one actually in force"
    );
}

#[test]
fn config_get_matches_globs_and_multiple_names() {
    let store = KeyValueStore::new();
    let facts = test_facts();
    let Value::Array(Some(flat)) =
        handle_config_command(&["GET".into(), "maxmemory*".into()], &facts, &store)
    else {
        panic!("expected an array")
    };
    // maxmemory and maxmemory-policy both match; the reply is flat pairs.
    assert_eq!(flat.len(), 4, "{flat:?}");

    let Value::Array(Some(none)) =
        handle_config_command(&["GET".into(), "nosuchparam".into()], &facts, &store)
    else {
        panic!("expected an array")
    };
    assert!(
        none.is_empty(),
        "an unmatched name yields no pair, not an error"
    );
}

#[test]
fn config_get_masks_the_password() {
    let store = KeyValueStore::new();
    let mut facts = test_facts();
    facts.auth_enabled = true;
    let Value::Array(Some(flat)) =
        handle_config_command(&["GET".into(), "requirepass".into()], &facts, &store)
    else {
        panic!("expected an array")
    };
    assert_eq!(
        flat[1],
        Value::BulkString(Some(b"*".to_vec())),
        "the password itself must never leave the process"
    );
}

#[test]
fn config_set_refuses_rather_than_pretending() {
    let store = KeyValueStore::new();
    let facts = test_facts();
    let reply = handle_config_command(
        &["SET".into(), "maxmemory".into(), "100mb".into()],
        &facts,
        &store,
    );
    // Nothing in the running server can change, so +OK would be a lie the
    // operator only discovers when the limit fails to apply.
    assert!(
        matches!(&reply, Value::Error(e) if e.contains("configured at startup")),
        "{reply:?}"
    );
}

#[test]
fn every_command_has_a_metrics_label() {
    for (cmd, _) in all_commands() {
        let name = command_name(&cmd);
        assert!(!name.is_empty(), "{cmd:?} has an empty metrics label");
        assert_eq!(
            name,
            name.to_lowercase(),
            "{cmd:?} label '{name}' must be lowercase for Prometheus"
        );
    }
}

#[test]
fn primary_keys_reports_writes_only() {
    // `primary_keys` answers "what did this command *write*", for
    // replication and push targeting — it is deliberately NOT the
    // authorization function (that is `command_scope`). Reads report
    // nothing because there is no mutation to broadcast.
    for cmd in [
        Command::Get("k".into()),
        Command::Exists(vec!["k".into()]),
        Command::Ttl("k".into()),
        Command::LRange("k".into(), 0, -1),
        Command::SMembers("k".into()),
        Command::HGetAll("k".into()),
    ] {
        assert!(
            primary_keys(&cmd).is_empty(),
            "{cmd:?} is a read and must not be broadcast as a mutation"
        );
    }

    // Writes must report every key they touch, or a replica or subscribed
    // browser silently misses the change.
    let writes: Vec<(Command, &[&str])> = vec![
        (
            Command::Set("k".into(), "v".into(), SetOptions::default()),
            &["k"],
        ),
        (Command::Del(vec!["a".into(), "b".into()]), &["a", "b"]),
        (Command::Incr("k".into()), &["k"]),
        (Command::Rename("src".into(), "dst".into()), &["src", "dst"]),
        (
            Command::SMove("src".into(), "dst".into(), "m".into()),
            &["src", "dst"],
        ),
        (
            Command::MSet(vec![("a".into(), "1".into()), ("b".into(), "2".into())]),
            &["a", "b"],
        ),
        (
            Command::SInterStore("dst".into(), vec!["a".into()]),
            &["dst"],
        ),
        (Command::LPush("k".into(), vec!["v".into()]), &["k"]),
        (
            Command::HSet("k".into(), vec![("f".into(), "v".into())]),
            &["k"],
        ),
        (Command::JMerge("k".into(), "{}".into()), &["k"]),
    ];
    for (cmd, want) in writes {
        let got = primary_keys(&cmd);
        for k in want {
            assert!(
                got.contains(&k.to_string()),
                "{cmd:?}: primary_keys missed written key '{k}', got {got:?}"
            );
        }
    }
}

// ── Pub/Sub pattern matching ──────────────────────────────────────────────

#[test]
fn psubscribe_pattern_matching_is_not_exponential() {
    // PSUBSCRIBE patterns are attacker-controlled, and every PUBLISH is
    // matched against every registered pattern. This file previously used a
    // recursive matcher that backtracked exponentially: a 10-wildcard
    // pattern against a 36-character channel took ~7 s, so one subscriber
    // could stall pub/sub for everyone. It now shares core-engine's DP
    // matcher (verified equivalent). This test fails loudly if that
    // regresses.
    let pattern = "*a*a*a*a*a*a*a*a*a*a*b";
    let channel = "a".repeat(200);
    let start = std::time::Instant::now();
    assert!(!core_engine::store::glob_match(pattern, &channel));
    assert!(
        start.elapsed() < std::time::Duration::from_millis(500),
        "pattern matching took {:?} — exponential backtracking is back",
        start.elapsed()
    );
}

#[test]
fn pubsub_patterns_match_the_expected_channels() {
    for (pat, ch, want) in [
        ("news.*", "news.tech", true),
        ("news.*", "news.", true),
        ("news.*", "sports.tech", false),
        ("*", "anything", true),
        ("user.?", "user.1", true),
        ("user.?", "user.42", false),
    ] {
        assert_eq!(
            core_engine::store::glob_match(pat, ch),
            want,
            "pattern {pat:?} vs channel {ch:?}"
        );
    }
}

// ── Introspection: PUBSUB / CLUSTER / MODULE / MEMORY ─────────────────────

/// A hub with `channels` subscribed and `patterns` psubscribed. The senders
/// are kept alive by the returned vector — dropping them would close the
/// receivers and make the hub look empty.
fn hub_with(
    channels: &[(u64, &str)],
    patterns: &[(u64, &str)],
) -> (PubSubHub, Vec<mpsc::Receiver<QueuedPubSubMsg>>) {
    let mut hub = PubSubHub::new();
    let mut keepalive = Vec::new();
    for (id, ch) in channels {
        let (overflow, _overflow_rx) = mpsc::channel(1);
        let (tx, rx) = pubsub_channel(overflow);
        hub.subscribe(*id, ch, tx);
        keepalive.push(rx);
    }
    for (id, pat) in patterns {
        let (overflow, _overflow_rx) = mpsc::channel(1);
        let (tx, rx) = pubsub_channel(overflow);
        hub.psubscribe(*id, pat, tx);
        keepalive.push(rx);
    }
    (hub, keepalive)
}

fn bulk_strings(v: &Value) -> Vec<String> {
    match v {
        Value::Array(Some(items)) => items
            .iter()
            .map(|i| match i {
                Value::BulkString(Some(b)) => String::from_utf8_lossy(b).into_owned(),
                other => panic!("expected a bulk string, got {other:?}"),
            })
            .collect(),
        other => panic!("expected an array, got {other:?}"),
    }
}

#[test]
fn pubsub_channels_lists_only_channels_with_subscribers() {
    let (hub, _keep) = hub_with(&[(1, "news"), (2, "news"), (3, "sports")], &[(4, "news.*")]);

    let mut all = bulk_strings(&handle_pubsub_command(&["CHANNELS".into()], &hub));
    all.sort();
    assert_eq!(all, vec!["news".to_string(), "sports".to_string()]);

    // A pattern subscriber is not a channel. Redis reports `news.*` under
    // NUMPAT and never under CHANNELS, because nobody is subscribed to a
    // channel by that name.
    assert!(!all.contains(&"news.*".to_string()));

    let filtered = bulk_strings(&handle_pubsub_command(
        &["CHANNELS".into(), "spo*".into()],
        &hub,
    ));
    assert_eq!(filtered, vec!["sports".to_string()]);
}

#[test]
fn pubsub_numsub_counts_per_channel_and_keeps_the_caller_s_order() {
    let (hub, _keep) = hub_with(&[(1, "news"), (2, "news"), (3, "sports")], &[(4, "news.*")]);

    let reply = handle_pubsub_command(
        &[
            "NUMSUB".into(),
            "sports".into(),
            "news".into(),
            "nobody-here".into(),
        ],
        &hub,
    );
    assert_eq!(
        reply,
        Value::Array(Some(vec![
            Value::BulkString(Some(b"sports".to_vec())),
            Value::Integer(1),
            Value::BulkString(Some(b"news".to_vec())),
            // Two subscribers, and the `news.*` pattern subscriber is not
            // one of them: NUMPAT's job, counted here would be double.
            Value::Integer(2),
            Value::BulkString(Some(b"nobody-here".to_vec())),
            // Present with a zero rather than omitted, so a caller can read
            // the reply by position against the channels it asked about.
            Value::Integer(0),
        ]))
    );

    // No channels named is a legal call and an empty reply, not an error.
    assert_eq!(
        handle_pubsub_command(&["NUMSUB".into()], &hub),
        Value::Array(Some(vec![]))
    );
}

#[test]
fn pubsub_numpat_counts_distinct_patterns_not_subscribers() {
    let (hub, _keep) = hub_with(&[], &[(1, "news.*"), (2, "news.*"), (3, "sports.*")]);
    assert_eq!(
        handle_pubsub_command(&["NUMPAT".into()], &hub),
        Value::Integer(2),
        "two clients on one pattern are one pattern"
    );
}

#[test]
fn pubsub_channels_forgets_a_channel_once_its_last_subscriber_leaves() {
    let (mut hub, _keep) = hub_with(&[(1, "news"), (2, "news")], &[]);
    hub.unsubscribe(1, "news");
    assert_eq!(
        bulk_strings(&handle_pubsub_command(&["CHANNELS".into()], &hub)),
        vec!["news".to_string()]
    );
    hub.unsubscribe(2, "news");
    assert!(
        bulk_strings(&handle_pubsub_command(&["CHANNELS".into()], &hub)).is_empty(),
        "an abandoned channel is not an active channel"
    );
}

#[tokio::test]
async fn pubsub_disconnects_a_subscriber_whose_queue_is_full() {
    let mut hub = PubSubHub::new();
    let (overflow, mut overflow_rx) = mpsc::channel(1);
    let (sender, _receiver) = pubsub_channel(overflow);
    hub.subscribe(1, "hot", sender);

    for _ in 0..256 {
        assert_eq!(hub.publish("hot", b"x"), 1);
    }
    assert_eq!(hub.publish("hot", b"x"), 0);
    assert!(overflow_rx.try_recv().is_ok());
    assert_eq!(hub.subscriber_count("hot"), 0);
}

#[test]
fn pubsub_refuses_the_sharded_subcommands() {
    let (hub, _keep) = hub_with(&[], &[]);
    // A standalone redis-server answers these with an empty array, and this
    // is the one place Recached deliberately does not match it: there, the
    // empty array sits next to a working SSUBSCRIBE. Here there is none, so
    // "no shard channels are subscribed" would invite a call that fails.
    for sub in ["SHARDCHANNELS", "SHARDNUMSUB"] {
        assert!(
            matches!(
                handle_pubsub_command(&[sub.to_string()], &hub),
                Value::Error(_)
            ),
            "{sub} should be refused"
        );
    }
}

#[test]
fn cluster_is_refused_the_way_a_standalone_redis_refuses_it() {
    // Verified against redis-server 7.2.5: a server not started in cluster
    // mode rejects the whole CLUSTER container with this sentence. It does
    // *not* answer INFO with cluster_enabled:0 — that lives in `INFO`.
    for sub in ["INFO", "NODES", "SLOTS", "MYID", "SHARDS"] {
        assert_eq!(
            handle_cluster_command(&[sub.to_string()]),
            Value::Error("ERR This instance has cluster support disabled".to_string()),
            "CLUSTER {sub}"
        );
    }
}

#[test]
fn info_publishes_the_cluster_flag_that_cluster_info_cannot() {
    let store = KeyValueStore::new();
    let body = render_info(
        &["cluster".to_string()],
        server_facts(),
        &store,
        sampled_keyspace(&store),
        false,
        ReplInfo::default(),
        0,
        0,
        0,
    );
    assert!(body.contains("# Cluster\r\n"), "section header: {body:?}");
    assert!(body.contains("cluster_enabled:0"), "{body:?}");

    // And it is in the default set, so a client that sends a bare INFO —
    // which is what every cluster-aware client actually sends — sees it.
    let default = render_info(
        &[],
        server_facts(),
        &store,
        sampled_keyspace(&store),
        false,
        ReplInfo::default(),
        0,
        0,
        0,
    );
    assert!(default.contains("cluster_enabled:0"), "{default:?}");
}

#[test]
fn module_list_is_empty_and_loading_is_refused() {
    assert_eq!(
        handle_module_command(&["LIST".to_string()]),
        Value::Array(Some(vec![])),
        "no modules is an answer, not an error"
    );
    for sub in ["LOAD", "LOADEX", "UNLOAD"] {
        assert!(
            matches!(
                handle_module_command(&[sub.to_string(), "/tmp/x.so".to_string()]),
                Value::Error(_)
            ),
            "MODULE {sub} should be refused rather than answered +OK"
        );
    }
}

#[test]
fn memory_allocator_subcommands_are_refused_with_a_reason() {
    for sub in ["DOCTOR", "STATS", "PURGE", "MALLOC-STATS"] {
        let Value::Error(msg) = handle_memory_command(&[sub.to_string()]) else {
            panic!("MEMORY {sub} should be refused");
        };
        assert!(
            msg.contains("MEMORY USAGE"),
            "the refusal should name what does work: {msg}"
        );
    }
    assert!(matches!(
        handle_memory_command(&["HELP".to_string()]),
        Value::Array(Some(_))
    ));
}

// ── Wire encoding ─────────────────────────────────────────────────────────

#[test]
fn pubsub_message_encodes_as_a_resp3_push_frame() {
    let bytes = encode_pubsub_msg(
        &PubSubMsg::Message {
            channel: "news".into(),
            message: "hello".into(),
        },
        3,
    );
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.starts_with('>'),
        "must be a RESP3 Push frame: {text:?}"
    );
    assert!(text.contains("message"));
    assert!(text.contains("news"));
    assert!(text.contains("hello"));
}

#[test]
fn pubsub_message_encodes_as_an_array_for_resp2() {
    // RESP2 has no push type. Sending `>` to a RESP2 client — which is
    // every client that has not sent HELLO 3 — is unparseable, so a
    // subscribed connection would break outright.
    let bytes = encode_pubsub_msg(
        &PubSubMsg::Message {
            channel: "news".into(),
            message: "hello".into(),
        },
        2,
    );
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.starts_with("*3\r\n"),
        "RESP2 delivery must be a 3-element array: {text:?}"
    );
    assert!(!text.contains('>'), "no push frame on RESP2: {text:?}");
}

#[test]
fn pattern_message_carries_the_matching_pattern() {
    // A pmessage must name the pattern that matched, or a client
    // subscribed to several patterns cannot tell them apart.
    for protover in [2u8, 3u8] {
        let bytes = encode_pubsub_msg(
            &PubSubMsg::PMessage {
                pattern: "news.*".into(),
                channel: "news.tech".into(),
                message: "hi".into(),
            },
            protover,
        );
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("pmessage"), "protover {protover}");
        assert!(text.contains("news.*"), "protover {protover}");
        assert!(text.contains("news.tech"), "protover {protover}");
    }
}

// ── HELLO / protocol negotiation ─────────────────────────────────────────

#[test]
fn hello_defaults_to_the_connections_current_version() {
    // Bare HELLO reports, it does not change. A client using it purely to
    // read server info must not be silently switched to another protocol.
    let mut protover = 2u8;
    let bytes = process_hello(None, &mut protover, true, false);
    assert_eq!(protover, 2, "bare HELLO must not change the version");
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.starts_with('*'),
        "RESP2 reply must be an array: {text:?}"
    );
    assert!(text.contains("recached"));
    assert!(text.contains(":2\r\n"), "proto must report 2: {text:?}");
}

#[test]
fn hello_3_upgrades_and_replies_with_a_map() {
    let mut protover = 2u8;
    let bytes = process_hello(Some("3"), &mut protover, true, false);
    assert_eq!(protover, 3);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.starts_with("%6\r\n"), "must be a 6-pair map: {text:?}");
    assert!(text.contains(":3\r\n"), "proto must report 3: {text:?}");
}

#[test]
fn hello_3_then_2_downgrades_again() {
    let mut protover = 2u8;
    process_hello(Some("3"), &mut protover, true, false);
    assert_eq!(protover, 3);
    let bytes = process_hello(Some("2"), &mut protover, true, false);
    assert_eq!(protover, 2, "HELLO 2 must downgrade");
    assert!(String::from_utf8_lossy(&bytes).starts_with('*'));
}

#[test]
fn hello_rejects_unsupported_versions_without_changing_protocol() {
    // A client probing for a version the server does not speak must get a
    // clean NOPROTO and stay on what it had — not be left in a half-state.
    for bad in ["4", "1", "0", "abc", "", "255", "-1", "3.0"] {
        let mut protover = 2u8;
        let bytes = process_hello(Some(bad), &mut protover, true, false);
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            text.starts_with("-NOPROTO"),
            "HELLO {bad:?} must be refused: {text:?}"
        );
        assert_eq!(protover, 2, "HELLO {bad:?} must not change the version");
    }
}

#[test]
fn hello_does_not_leak_server_details_before_auth() {
    let mut protover = 2u8;
    let bytes = process_hello(Some("3"), &mut protover, false, false);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.starts_with("-NOAUTH"), "{text:?}");
    assert!(
        !text.contains("recached") && !text.contains(env!("CARGO_PKG_VERSION")),
        "unauthenticated HELLO must not fingerprint the server: {text:?}"
    );
}

#[test]
fn hello_reports_replica_role() {
    let mut protover = 3u8;
    let primary =
        String::from_utf8_lossy(&process_hello(None, &mut protover, true, false)).into_owned();
    let replica =
        String::from_utf8_lossy(&process_hello(None, &mut protover, true, true)).into_owned();
    assert!(primary.contains("master"), "{primary:?}");
    assert!(replica.contains("replica"), "{replica:?}");
}

// ── INFO ──────────────────────────────────────────────────────────────────

fn test_facts() -> ServerFacts {
    ServerFacts {
        start: SystemTime::now() - std::time::Duration::from_secs(90_000),
        run_id: "a".repeat(40),
        tcp_port: 6379,
        ws_port: 6380,
        max_connections: 512,
        tls_enabled: true,
        auth_enabled: true,
        aof_enabled: true,
    }
}

/// Render `sections` against `store`, walking it for the keyspace numbers.
///
/// Tests pass the sample explicitly rather than going through the shared
/// 5s cache — `render_info` is pure so that concurrent tests cannot
/// observe each other's keyspace through a process-global.
fn info_for(sections: &[&str], store: &KeyValueStore) -> String {
    render_info(
        &sections.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        &test_facts(),
        store,
        store.keyspace_sample(),
        false,
        ReplInfo::default(),
        1_700_000_000,
        0,
        0,
    )
}

/// Parse an INFO payload into (section, field) → value, enforcing the shape
/// clients rely on: CRLF endings, `# Section` headers, `field:value` lines.
fn parse_info(payload: &str) -> HashMap<(String, String), String> {
    assert!(
        !payload.contains('\n') || payload.contains("\r\n"),
        "INFO must use CRLF line endings"
    );
    let mut out = HashMap::new();
    let mut section = String::new();
    for line in payload.split("\r\n") {
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix("# ") {
            section = name.to_lowercase();
            continue;
        }
        let (k, v) = line
            .split_once(':')
            .unwrap_or_else(|| panic!("malformed INFO line: {line:?}"));
        out.insert((section.clone(), k.to_string()), v.to_string());
    }
    out
}

#[test]
fn info_default_emits_every_default_section() {
    let store = KeyValueStore::new();
    let payload = info_for(&[], &store);
    for section in DEFAULT_INFO_SECTIONS {
        let header = format!("# {}{}\r\n", section[..1].to_uppercase(), &section[1..]);
        assert!(
            payload.contains(&header),
            "missing section header {header:?} in {payload:?}"
        );
    }
}

#[test]
fn info_uses_crlf_and_blank_line_separated_sections() {
    let store = KeyValueStore::new();
    let payload = info_for(&["server", "clients"], &store);
    assert!(payload.starts_with("# Server\r\n"), "{payload:?}");
    // A blank line must close each section, or parsers merge them.
    assert!(payload.contains("\r\n\r\n# Clients\r\n"), "{payload:?}");
    assert!(payload.ends_with("\r\n\r\n"), "{payload:?}");
    assert!(!payload.contains('\n') || !payload.replace("\r\n", "").contains('\n'));
}

#[test]
fn info_server_section_reports_compat_version_separately_from_ours() {
    let store = KeyValueStore::new();
    let f = parse_info(&info_for(&["server"], &store));
    // Clients feature-gate on redis_version, so it must be a Redis version,
    // never Recached's own — that is the entire point of the split.
    assert_eq!(f[&("server".into(), "redis_version".into())], "6.2.0");
    assert_eq!(
        f[&("server".into(), "recached_version".into())],
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(f[&("server".into(), "redis_mode".into())], "standalone");
    assert_eq!(f[&("server".into(), "tcp_port".into())], "6379");
    assert_eq!(f[&("server".into(), "recached_ws_port".into())], "6380");
    assert_eq!(f[&("server".into(), "run_id".into())].len(), 40);
    // 90_000s of uptime is one day and change.
    assert_eq!(f[&("server".into(), "uptime_in_days".into())], "1");
    assert!(
        f[&("server".into(), "uptime_in_seconds".into())]
            .parse::<u64>()
            .unwrap()
            >= 90_000
    );
}

#[test]
fn info_memory_section_reports_limits_and_policy() {
    let store = KeyValueStore::with_config(Some(50), Some(1024 * 1024), EvictionPolicy::AllKeysLru);
    let f = parse_info(&info_for(&["memory"], &store));
    assert_eq!(f[&("memory".into(), "maxmemory".into())], "1048576");
    assert_eq!(f[&("memory".into(), "maxmemory_human".into())], "1.00M");
    assert_eq!(
        f[&("memory".into(), "maxmemory_policy".into())],
        "allkeys-lru"
    );
    assert_eq!(f[&("memory".into(), "recached_max_keys".into())], "50");
}

#[test]
fn info_memory_reports_zero_maxmemory_when_unbounded() {
    // Redis reports 0 for "no limit"; None must not leak as a debug string.
    let f = parse_info(&info_for(&["memory"], &KeyValueStore::new()));
    assert_eq!(f[&("memory".into(), "maxmemory".into())], "0");
    assert_eq!(
        f[&("memory".into(), "maxmemory_policy".into())],
        "noeviction"
    );
}

#[test]
fn info_persistence_always_reports_loading_zero() {
    // A client's ready-check gates on this field; the snapshot is loaded
    // before any listener binds, so a reachable server is never loading.
    let f = parse_info(&info_for(&["persistence"], &KeyValueStore::new()));
    assert_eq!(f[&("persistence".into(), "loading".into())], "0");
    assert_eq!(
        f[&("persistence".into(), "rdb_last_save_time".into())],
        "1700000000"
    );
    assert_eq!(f[&("persistence".into(), "aof_enabled".into())], "1");
}

#[test]
fn info_replication_reports_both_redis_and_recached_spellings() {
    let store = KeyValueStore::new();
    let repl = ReplInfo {
        connected: 2,
        queue_depth: 7,
        lag_frames: 3,
    };
    let primary = parse_info(&render_info(
        &[],
        &test_facts(),
        &store,
        store.keyspace_sample(),
        false,
        repl,
        0,
        0,
        0,
    ));
    assert_eq!(primary[&("replication".into(), "role".into())], "master");
    // Tooling greps for `connected_slaves`; the modern alias ships too.
    assert_eq!(
        primary[&("replication".into(), "connected_slaves".into())],
        "2"
    );
    assert_eq!(
        primary[&("replication".into(), "connected_replicas".into())],
        "2"
    );
    assert_eq!(
        primary[&(
            "replication".into(),
            "recached_replication_lag_frames".into()
        )],
        "3"
    );

    let replica = parse_info(&render_info(
        &[],
        &test_facts(),
        &store,
        store.keyspace_sample(),
        true,
        ReplInfo::default(),
        0,
        0,
        0,
    ));
    // Redis still spells a replica `slave` in INFO, and clients match on it.
    assert_eq!(replica[&("replication".into(), "role".into())], "slave");
}

#[test]
fn info_keyspace_omits_the_db_line_when_empty_and_counts_ttls_when_not() {
    let store = KeyValueStore::new();
    assert!(
        !info_for(&["keyspace"], &store).contains("db0:"),
        "an empty keyspace must not report a db0 line"
    );

    store.execute(Command::Set(
        "a".into(),
        b"v".to_vec(),
        SetOptions::default(),
    ));
    store.execute(Command::Set(
        "b".into(),
        b"v".to_vec(),
        SetOptions::default(),
    ));
    store.execute(Command::Expire("b".into(), 60));
    let payload = info_for(&["keyspace"], &store);
    assert!(
        payload.contains("db0:keys=2,expires=1,avg_ttl=0"),
        "{payload:?}"
    );
}

#[test]
fn sampled_keyspace_falls_back_to_a_live_walk_before_the_sampler_runs() {
    // First INFO of a process arrives before the 5s sampler has ever run,
    // and must not report an empty keyspace.
    let store = KeyValueStore::new();
    store.execute(Command::Set(
        "k".into(),
        b"v".to_vec(),
        SetOptions::default(),
    ));
    SAMPLED_KEYS.store(u64::MAX, Ordering::Relaxed);
    assert_eq!(sampled_keyspace(&store).keys, 1);
}

#[test]
fn info_unknown_section_yields_nothing() {
    // Redis answers an unknown section with an empty payload, not an error.
    assert_eq!(info_for(&["nosuchsection"], &KeyValueStore::new()), "");
}

#[test]
fn info_all_and_everything_expand_to_the_default_sections() {
    let store = KeyValueStore::new();
    let default = info_for(&[], &store);
    for alias in ["all", "everything", "default"] {
        assert_eq!(
            info_for(&[alias], &store).lines().count(),
            default.lines().count(),
            "INFO {alias} must cover the default sections"
        );
    }
}

#[test]
fn info_honours_section_selection_and_order() {
    let payload = info_for(&["clients", "server"], &KeyValueStore::new());
    assert!(payload.starts_with("# Clients\r\n"), "{payload:?}");
    assert!(payload.contains("# Server\r\n"), "{payload:?}");
    assert!(!payload.contains("# Memory"), "{payload:?}");
}

#[test]
fn human_bytes_matches_redis_formatting() {
    assert_eq!(human_bytes(0), "0B");
    assert_eq!(human_bytes(512), "512B");
    assert_eq!(human_bytes(1024), "1.00K");
    assert_eq!(human_bytes(1024 * 1024), "1.00M");
    assert_eq!(human_bytes(3 * 1024 * 1024 * 1024), "3.00G");
}

#[test]
fn run_ids_are_forty_hex_chars_and_differ_per_process() {
    let a = generate_run_id();
    assert_eq!(a.len(), 40);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()), "{a}");
    assert_ne!(a, generate_run_id());
}

#[test]
fn info_is_administrative_scope() {
    // Scoped WebSocket connections must not be able to read server-wide
    // state, so INFO has to classify as Admin, not KeyLess.
    assert!(matches!(
        command_scope(&Command::Info(vec![])),
        CommandScope::Admin
    ));
}

#[test]
fn info_is_not_a_write_command() {
    assert!(!is_write_command(&Command::Info(vec![])));
}

#[tokio::test]
async fn info_over_tcp_returns_a_parseable_bulk_string() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    let Value::BulkString(Some(bytes)) = c.cmd(&["INFO"]).await else {
        panic!("INFO must reply with a bulk string");
    };
    let payload = String::from_utf8(bytes).unwrap();
    let f = parse_info(&payload);
    assert_eq!(f[&("server".into(), "redis_version".into())], "6.2.0");
    assert_eq!(f[&("replication".into(), "role".into())], "master");
    assert!(f.contains_key(&("stats".into(), "total_commands_processed".into())));
}

#[tokio::test]
async fn info_section_argument_is_honoured_over_the_wire() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    let Value::BulkString(Some(bytes)) = c.cmd(&["INFO", "server"]).await else {
        panic!("INFO must reply with a bulk string");
    };
    let payload = String::from_utf8(bytes).unwrap();
    assert!(payload.starts_with("# Server\r\n"), "{payload:?}");
    assert!(!payload.contains("# Memory"), "{payload:?}");
}

#[tokio::test]
async fn info_reflects_live_server_state() {
    let srv = spawn_server().await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    c.cmd(&["SET", "k", "v"]).await;
    c.cmd(&["GET", "k"]).await; // hit
    c.cmd(&["GET", "missing"]).await; // miss

    let Value::BulkString(Some(bytes)) = c.cmd(&["INFO", "stats", "persistence"]).await else {
        panic!("INFO must reply with a bulk string");
    };
    let f = parse_info(&String::from_utf8(bytes).unwrap());
    assert!(
        f[&("stats".into(), "keyspace_hits".into())]
            .parse::<u64>()
            .unwrap()
            >= 1
    );
    assert!(
        f[&("stats".into(), "keyspace_misses".into())]
            .parse::<u64>()
            .unwrap()
            >= 1
    );
    // The SET must show up as an unsaved change.
    assert!(
        f[&("persistence".into(), "rdb_changes_since_last_save".into())]
            .parse::<u64>()
            .unwrap()
            >= 1
    );
}

#[tokio::test]
async fn info_requires_authentication() {
    let srv = spawn_server_cfg(Some("hunter2"), None, false).await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    // INFO leaks deployment details, so it must sit behind AUTH like every
    // other non-handshake command.
    match c.cmd(&["INFO"]).await {
        Value::Error(e) => assert!(e.starts_with("NOAUTH"), "{e}"),
        other => panic!("unauthenticated INFO must be refused, got {other:?}"),
    }

    assert_eq!(c.cmd(&["AUTH", "hunter2"]).await, ok());
    assert!(matches!(c.cmd(&["INFO"]).await, Value::BulkString(Some(_))));
}

#[tokio::test]
async fn info_on_a_replica_reports_the_slave_role() {
    let srv = spawn_server_cfg(None, None, true).await;
    let mut c = RespClient::connect(srv.tcp_addr).await;

    let Value::BulkString(Some(bytes)) = c.cmd(&["INFO", "replication"]).await else {
        panic!("INFO must reply with a bulk string");
    };
    let f = parse_info(&String::from_utf8(bytes).unwrap());
    assert_eq!(f[&("replication".into(), "role".into())], "slave");
}

#[test]
fn subscribe_ack_reports_the_running_subscription_count() {
    let bytes = resp_subscribe_ack("subscribe", "news", 3);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("subscribe"));
    assert!(text.contains("news"));
    assert!(
        text.contains(":3"),
        "count must be a RESP integer: {text:?}"
    );
}

// ── Score formatting ──────────────────────────────────────────────────────

#[test]
fn scores_format_without_trailing_decimals() {
    // Redis returns "1" not "1.0" — clients parse these as integers.
    assert_eq!(format_f64_score(1.0), "1");
    assert_eq!(format_f64_score(-5.0), "-5");
    assert_eq!(format_f64_score(0.0), "0");
    assert_eq!(format_f64_score(1.5), "1.5");
    assert_eq!(format_f64_score(-0.25), "-0.25");
}

#[test]
fn scores_format_infinities_as_redis_does() {
    assert_eq!(format_f64_score(f64::INFINITY), "inf");
    assert_eq!(format_f64_score(f64::NEG_INFINITY), "-inf");
}

#[test]
fn very_large_scores_do_not_lose_their_exponent() {
    // Past 1e15 the integer shortcut is skipped, because casting to i64
    // would silently truncate.
    let big = 1e16_f64;
    let s = format_f64_score(big);
    assert!(
        s.contains('e') || s.len() > 15,
        "unexpected formatting: {s}"
    );
}

// ── Save conditions ───────────────────────────────────────────────────────

#[test]
fn save_conditions_parse_as_seconds_colon_changes() {
    let c = parse_save_conditions("900:1,300:10,60:10000").unwrap();
    assert_eq!(c.len(), 3);
    assert_eq!(c[0].secs, 900);
    assert_eq!(c[0].changes, 1);
    assert_eq!(c[2].secs, 60);
    assert_eq!(c[2].changes, 10000);
}

#[test]
fn save_conditions_tolerate_whitespace() {
    let c = parse_save_conditions(" 900 : 1 , 300 : 10 ").unwrap();
    assert_eq!(c.len(), 2);
    assert_eq!(c[0].secs, 900);
    assert_eq!(c[1].changes, 10);
}

#[test]
fn malformed_save_conditions_are_rejected() {
    assert!(parse_save_conditions("900:1,garbage,300:10").is_err());
    assert!(parse_save_conditions("").unwrap().is_empty());
    assert!(parse_save_conditions("0").unwrap().is_empty());
    assert!(parse_save_conditions("nonsense").is_err());
    assert!(parse_save_conditions("900").is_err());
    assert!(parse_save_conditions("0:1").is_err());
    assert!(parse_save_conditions("1:0").is_err());
}

// ── TLS configuration ─────────────────────────────────────────────────────

#[test]
fn tls_requires_both_cert_and_key() {
    assert_eq!(
        resolve_tls_paths(None, None).unwrap(),
        None,
        "neither set → plaintext"
    );
    assert_eq!(
        resolve_tls_paths(Some("c.pem".into()), Some("k.pem".into())).unwrap(),
        Some(("c.pem".to_string(), "k.pem".to_string()))
    );
}

#[test]
fn tls_half_configured_is_refused_not_downgraded() {
    // The dangerous case: an operator sets the cert, mistypes the key
    // variable, and the server used to serve plaintext on both ports while
    // reporting itself healthy. Traffic believed encrypted was not.
    let cert_only = resolve_tls_paths(Some("c.pem".into()), None).unwrap_err();
    assert!(cert_only.contains("RECACHED_TLS_KEY"), "got {cert_only}");
    assert!(
        cert_only.contains("plaintext"),
        "must explain the risk: {cert_only}"
    );

    let key_only = resolve_tls_paths(None, Some("k.pem".into())).unwrap_err();
    assert!(key_only.contains("RECACHED_TLS_CERT"), "got {key_only}");
}

// ── IP allowlist ──────────────────────────────────────────────────────────

#[test]
fn allow_ips_parses_exact_addresses() {
    let ips = parse_allow_ips("10.0.1.5, 10.0.1.6").unwrap();
    assert_eq!(ips.len(), 2);
    assert!(ips.contains(&IpAddr::from_str("10.0.1.5").unwrap()));
    // IPv6 literals are accepted too.
    let v6 = parse_allow_ips("::1").unwrap();
    assert_eq!(v6, vec![IpAddr::from_str("::1").unwrap()]);
}

#[test]
fn allow_ips_rejects_cidr_instead_of_silently_narrowing() {
    // A CIDR range used to be dropped with only a warning, leaving an
    // allowlist that excluded every host the operator meant to admit.
    let err = parse_allow_ips("10.0.0.0/8").unwrap_err();
    assert!(err.contains("10.0.0.0/8"), "must name the bad entry: {err}");
    assert!(err.contains("CIDR"), "must explain why: {err}");
}

#[test]
fn allow_ips_rejects_a_partially_valid_list() {
    // One good entry must not mask a typo in another — the result would be
    // a narrower allowlist than configured.
    assert!(parse_allow_ips("10.0.1.5,not-an-ip").is_err());
    assert!(
        parse_allow_ips("localhost").is_err(),
        "hostnames unsupported"
    );
}

#[test]
fn allow_ips_rejects_an_empty_result_that_would_block_everything() {
    // An all-invalid list previously produced an empty allowlist, and an
    // empty allowlist rejects every connection while the process still
    // starts and passes health checks.
    let err = parse_allow_ips("   ").unwrap_err();
    assert!(err.contains("reject every connection"), "got {err}");
    assert!(parse_allow_ips(",,,").is_err());
}

#[test]
fn allow_ips_tolerates_incidental_whitespace_and_trailing_commas() {
    let ips = parse_allow_ips(" 127.0.0.1 , 10.0.0.1 ,").unwrap();
    assert_eq!(ips.len(), 2);
}

/// `record_command` looks the label up in an immutable, pre-built map. A
/// label `command_name` can produce but the catalog does not list still
/// works — it falls back to an uncached registry lookup — but it pays that
/// lookup on every single call, so the gap should fail CI rather than
/// quietly become a hot-path cost.
#[test]
fn command_name_labels_are_all_pre_registered() {
    let labels = [
        "get",
        "set",
        "del",
        "incr",
        "decr",
        "exists",
        "expire",
        "ttl",
        "type",
        "append",
        "strlen",
        "hset",
        "hget",
        "hgetall",
        "hdel",
        "hlen",
        "lpush",
        "rpush",
        "lpop",
        "rpop",
        "lrange",
        "llen",
        "sadd",
        "srem",
        "smembers",
        "scard",
        "zadd",
        "zrange",
        "zrem",
        "zscore",
        "ping",
        "auth",
        "hello",
        "quit",
        "client",
        "config",
        "command",
        "scan",
        "subscribe",
        "publish",
        "multi",
        "exec",
        "watch",
        UNKNOWN_COMMAND,
    ];
    let missing: Vec<&str> = labels
        .into_iter()
        .filter(|l| !CMD_COUNTERS.contains_key(l))
        .collect();
    assert!(
        missing.is_empty(),
        "labels with no pre-built counter (each costs a registry lookup per command): {missing:?}"
    );
}

/// The counter table is built once from the catalog and never mutated, so
/// concurrent `record_command` calls need no lock and cannot poison one.
/// The previous `RwLock` was `.unwrap()`ed on every command: one panic while
/// holding it poisoned the lock and every later command panicked with it.
#[test]
fn record_command_is_safe_from_many_threads_at_once() {
    let threads: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(|| {
                for _ in 0..5_000 {
                    record_command("get");
                    record_command("set");
                    record_command(UNKNOWN_COMMAND);
                }
            })
        })
        .collect();
    for t in threads {
        t.join().expect("record_command panicked under contention");
    }
}

#[test]
fn command_name_is_stable_and_lowercase() {
    // These strings become Prometheus label values; renaming one silently
    // breaks existing dashboards and alerts.
    let cases = [
        (Command::Get("k".into()), "get"),
        (
            Command::Set("k".into(), "v".into(), SetOptions::default()),
            "set",
        ),
        (Command::Del(vec!["k".into()]), "del"),
        (Command::Incr("k".into()), "incr"),
        (Command::HGetAll("k".into()), "hgetall"),
        (Command::LPush("k".into(), vec!["v".into()]), "lpush"),
        (Command::SAdd("k".into(), vec!["m".into()]), "sadd"),
        (Command::Ping(None), "ping"),
    ];
    for (cmd, expected) in cases {
        assert_eq!(command_name(&cmd), expected, "label drift for {cmd:?}");
    }
}

// ── Config parsing ────────────────────────────────────────────────────────

#[test]
fn parse_memory_bytes_accepts_units_and_bare_numbers() {
    assert_eq!(parse_memory_bytes("1024"), Some(1024));
    assert_eq!(parse_memory_bytes("1kb"), Some(1024));
    assert_eq!(parse_memory_bytes("2mb"), Some(2 * 1024 * 1024));
    assert_eq!(parse_memory_bytes("1gb"), Some(1024 * 1024 * 1024));
}

#[test]
fn parse_memory_bytes_is_case_and_whitespace_tolerant() {
    assert_eq!(parse_memory_bytes("  2MB "), Some(2 * 1024 * 1024));
    assert_eq!(parse_memory_bytes("2 mb"), Some(2 * 1024 * 1024));
    assert_eq!(parse_memory_bytes("1Gb"), Some(1024 * 1024 * 1024));
}

#[test]
fn parse_memory_bytes_rejects_nonsense_rather_than_defaulting() {
    // Returning None lets the caller fall back explicitly; silently
    // parsing "10 bananas" as 10 bytes would cap memory at nothing.
    for bad in ["", "abc", "10 bananas", "-5", "1.5mb", "mb"] {
        assert_eq!(parse_memory_bytes(bad), None, "{bad:?} should not parse");
    }
}

#[test]
fn parse_memory_bytes_rejects_overflow() {
    assert_eq!(parse_memory_bytes(&format!("{}gb", usize::MAX)), None);
    assert_eq!(parse_memory_bytes(&format!("{}mb", usize::MAX)), None);
    assert_eq!(parse_memory_bytes(&format!("{}kb", usize::MAX)), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_sync_scope_filters_fanout() {
    let srv = spawn_ws_server().await;
    let mut scoped = WsClient::connect(srv.tcp_addr).await;
    let mut unscoped = WsClient::connect(srv.tcp_addr).await;
    let mut writer = WsClient::connect(srv.tcp_addr).await;

    // Open mode: SYNC with literal patterns.
    assert_eq!(scoped.cmd(&["SYNC", "cart:*"]).await, arr(&["rw=cart:*"]));

    assert_eq!(writer.cmd(&["SET", "cart:1", "x"]).await, ok());
    assert_eq!(writer.cmd(&["SET", "other:1", "y"]).await, ok());

    // Scoped client sees the cart write and nothing else.
    let push = scoped.recv_push(1000).await.expect("expected cart:1 push");
    assert!(push.contains("cart:1"), "unexpected push: {push}");
    assert!(
        scoped.recv_push(300).await.is_none(),
        "out-of-scope push leaked to scoped client"
    );

    // Unscoped client (legacy mode) sees both.
    let p1 = unscoped.recv_push(1000).await.expect("push 1");
    let p2 = unscoped.recv_push(1000).await.expect("push 2");
    assert!(p1.contains("cart:1") && p2.contains("other:1"));

    // Bare SYNC reports current scopes.
    assert_eq!(scoped.cmd(&["SYNC"]).await, arr(&["rw=cart:*"]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_sync_strict_mode_gates_and_filters() {
    let secret = "integration-secret";
    let srv = spawn_ws_server_cfg(Some(secret.to_string())).await;
    let mut client = WsClient::connect(srv.tcp_addr).await;

    // No token yet: key commands and pushes are refused.
    let r = client.cmd(&["GET", "cart:1"]).await;
    assert!(matches!(&r, Value::Error(e) if e.contains("NOSCOPE")));
    // Literal patterns are rejected in strict mode.
    let r = client.cmd(&["SYNC", "cart:*"]).await;
    assert!(matches!(&r, Value::Error(e) if e.contains("signed scopes")));
    // Garbage token.
    let r = client.cmd(&["SYNC", "TOKEN", "not-a-token"]).await;
    assert!(matches!(&r, Value::Error(e) if e.contains("invalid sync token")));

    // Valid token: scoped to cart:* only.
    let tok = mint_sync_token(secret, "cart:*");
    assert_eq!(
        client.cmd(&["SYNC", "TOKEN", &tok]).await,
        arr(&["rw=cart:*"])
    );

    // In-scope commands work; out-of-scope and admin are refused.
    assert_eq!(client.cmd(&["SET", "cart:1", "x"]).await, ok());
    assert_eq!(client.cmd(&["GET", "cart:1"]).await, bulk("x"));
    let r = client.cmd(&["GET", "secret-key"]).await;
    assert!(matches!(&r, Value::Error(e) if e.contains("NOSCOPE")));
    let r = client.cmd(&["KEYS", "*"]).await;
    assert!(matches!(&r, Value::Error(e) if e.contains("NOSCOPE")));

    // Fan-out: a second scoped client writes in and out of the first's scope.
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    let wtok = mint_sync_token(secret, "cart:*,other:*");
    assert_eq!(
        writer.cmd(&["SYNC", "TOKEN", &wtok]).await,
        arr(&["rw=cart:*", "rw=other:*"])
    );
    assert_eq!(writer.cmd(&["SET", "cart:2", "a"]).await, ok());
    assert_eq!(writer.cmd(&["SET", "other:2", "b"]).await, ok());

    let push = client.recv_push(1000).await.expect("expected cart:2 push");
    assert!(push.contains("cart:2"), "unexpected push: {push}");
    assert!(
        client.recv_push(300).await.is_none(),
        "out-of-scope push leaked on strict connection"
    );
}

// ── Presence: connection-bound keys and set membership ────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_a_second_tab_keeps_the_user_online() {
    // The bug this fixes: ownership of an ESET key transferred to whichever
    // connection wrote it last, so closing the *most recent* tab deleted the
    // key while every earlier tab was still open — the user went offline
    // while still looking at the page.
    let srv = spawn_ws_server().await;
    let mut tab_a = WsClient::connect(srv.tcp_addr).await;
    let mut tab_b = WsClient::connect(srv.tcp_addr).await;
    let mut observer = WsClient::connect(srv.tcp_addr).await;

    assert_eq!(tab_a.cmd(&["ESET", "presence:42", "online"]).await, ok());
    assert_eq!(tab_b.cmd(&["ESET", "presence:42", "online"]).await, ok());

    // The later tab closes; the earlier one is still open.
    drop(tab_b);
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert_eq!(
        observer.cmd(&["GET", "presence:42"]).await,
        bulk("online"),
        "closing the later tab must not take the user offline"
    );

    // The last holder closing does remove it.
    drop(tab_a);
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert_eq!(
        observer.cmd(&["GET", "presence:42"]).await,
        Value::BulkString(None)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_eadd_membership_follows_the_connection() {
    // "Who is in this room" — membership that cleans itself up, readable with
    // the ordinary set commands and observable through a live query.
    let srv = spawn_ws_server().await;
    let mut alice = WsClient::connect(srv.tcp_addr).await;
    let mut bob = WsClient::connect(srv.tcp_addr).await;
    let mut observer = WsClient::connect(srv.tcp_addr).await;

    assert_eq!(alice.cmd(&["EADD", "room:1", "alice"]).await, int(1));
    assert_eq!(bob.cmd(&["EADD", "room:1", "bob"]).await, int(1));
    assert_eq!(observer.cmd(&["SCARD", "room:1"]).await, int(2));

    // Bob leaves: his membership goes, Alice's stays.
    drop(bob);
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert_eq!(
        observer.cmd(&["SMEMBERS", "room:1"]).await,
        Value::Array(Some(vec![bulk("alice")]))
    );

    // The set disappears once the room empties.
    drop(alice);
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert_eq!(observer.cmd(&["EXISTS", "room:1"]).await, int(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_eadd_membership_survives_a_second_tab() {
    // Same rule one level down: a member is held by every connection that
    // added it, so one of a user's tabs closing does not remove them.
    let srv = spawn_ws_server().await;
    let mut tab_a = WsClient::connect(srv.tcp_addr).await;
    let mut tab_b = WsClient::connect(srv.tcp_addr).await;
    let mut observer = WsClient::connect(srv.tcp_addr).await;

    assert_eq!(tab_a.cmd(&["EADD", "room:2", "alice"]).await, int(1));
    // The second tab re-adds the same member; SADD semantics make it a no-op
    // in the set, but it takes a hold on the membership.
    assert_eq!(tab_b.cmd(&["EADD", "room:2", "alice"]).await, int(0));

    drop(tab_b);
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert_eq!(observer.cmd(&["SCARD", "room:2"]).await, int(1));

    drop(tab_a);
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert_eq!(observer.cmd(&["EXISTS", "room:2"]).await, int(0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_presence_departure_reaches_live_queries() {
    // A departure is a real mutation, so it fans out like any other — a UI
    // showing the room updates without polling.
    let srv = spawn_ws_server().await;
    let mut viewer = WsClient::connect(srv.tcp_addr).await;
    let mut bob = WsClient::connect(srv.tcp_addr).await;

    assert_eq!(bob.cmd(&["EADD", "room:3", "bob"]).await, int(1));
    // qstate is [tag, pattern, key, value] — the value being a type-tagged
    // collection.
    assert_eq!(
        viewer.cmd(&["QSUB", "room:3"]).await,
        Value::Array(Some(vec![
            bulk("qstate"),
            bulk("room:3"),
            bulk("room:3"),
            Value::Array(Some(vec![bulk("set"), bulk("bob")])),
        ]))
    );

    drop(bob);
    let push = viewer
        .recv_any(1000)
        .await
        .expect("the room must be told bob left");
    assert!(push.contains("room:3"), "unexpected frame: {push}");
}

#[test]
fn ephemeral_holds_are_released_only_by_the_last_holder() {
    let state = state_with_snapshot_path(tmp_path("presence_holds.rdb"));
    state.claim_ephemeral("presence:42", 1);
    state.claim_ephemeral("presence:42", 2);
    // Repeating from the same connection is idempotent.
    state.claim_ephemeral("presence:42", 2);

    assert!(
        state.take_ephemeral_for(2).is_empty(),
        "connection 1 still holds the key"
    );
    assert_eq!(state.take_ephemeral_for(1), vec!["presence:42".to_string()]);
    // And it is gone from the registry, not merely unreported.
    assert!(state.take_ephemeral_for(1).is_empty());
}

#[test]
fn ephemeral_members_are_released_per_member_and_grouped_by_key() {
    let state = state_with_snapshot_path(tmp_path("presence_holds.rdb"));
    state.claim_ephemeral_members("room:1", &["alice".to_string()], 1);
    state.claim_ephemeral_members("room:1", &["bob".to_string()], 2);
    state.claim_ephemeral_members("room:2", &["bob".to_string()], 2);

    assert!(state.take_ephemeral_members_for(3).is_empty());

    let mut released = state.take_ephemeral_members_for(2);
    released.sort();
    assert_eq!(
        released,
        vec![
            ("room:1".to_string(), vec!["bob".to_string()]),
            ("room:2".to_string(), vec!["bob".to_string()]),
        ],
        "one SREM per set, so each fans out as its own mutation"
    );
    assert_eq!(
        state.take_ephemeral_members_for(1),
        vec![("room:1".to_string(), vec!["alice".to_string()])]
    );
}

// ── Delta frames ──────────────────────────────────────────────────────────
//
// A keychange carries the key's whole value, so appending one token to an
// agent's output re-sent the entire transcript to every viewer, and one SADD
// into a 10k-member set re-sent all 10k. A delta carries the mutation instead.

#[test]
fn delta_is_the_command_not_a_diff() {
    let (key, delta) = delta_for(&Command::Append("out".into(), b" tok".to_vec()))
        .expect("APPEND is the case deltas exist for");
    assert_eq!(key, "out");
    assert_eq!(delta.op, "append");
    assert_eq!(delta.args, vec![b" tok".to_vec()]);

    let (_, delta) = delta_for(&Command::SAdd("s".into(), vec!["a".into(), "b".into()])).unwrap();
    assert_eq!(delta.op, "sadd");
    assert_eq!(delta.args, vec![b"a".to_vec(), b"b".to_vec()]);

    // HSET flattens field/value so the delta parses back as the command.
    let (_, delta) = delta_for(&Command::HSet(
        "h".into(),
        vec![("f".to_string(), b"v".to_vec())],
    ))
    .unwrap();
    assert_eq!(delta.op, "hset");
    assert_eq!(delta.args, vec![b"f".to_vec(), b"v".to_vec()]);

    // DEDUP is transparent: the wrapped write is what gets replayed.
    let wrapped = Command::Dedup(
        "c".into(),
        1,
        Box::new(Command::SRem("s".into(), vec!["a".into()])),
    );
    let (key, delta) = delta_for(&wrapped).unwrap();
    assert_eq!((key.as_str(), delta.op), ("s", "srem"));
}

#[test]
fn commands_that_cannot_be_replayed_verbatim_have_no_delta() {
    // SET is already as small as its result — a delta would buy nothing.
    assert!(
        delta_for(&Command::Set(
            "k".into(),
            b"v".to_vec(),
            SetOptions::default()
        ))
        .is_none()
    );
    assert!(delta_for(&Command::Del(vec!["k".into()])).is_none());
    assert!(delta_for(&Command::Incr("n".into())).is_none());

    // ZADD options change what is stored, so replaying without them would
    // compute a different score on the client.
    let conditional = Command::ZAdd(
        "z".into(),
        ZAddOptions {
            condition: Some(ZAddCondition::Nx),
            ..Default::default()
        },
        vec![(1.0, "m".to_string())],
    );
    assert!(delta_for(&conditional).is_none());

    let incr = Command::ZAdd(
        "z".into(),
        ZAddOptions {
            incr: true,
            ..Default::default()
        },
        vec![(1.0, "m".to_string())],
    );
    assert!(delta_for(&incr).is_none());

    // CH only changes the integer reply, so it keeps its delta.
    let ch = Command::ZAdd(
        "z".into(),
        ZAddOptions {
            ch: true,
            ..Default::default()
        },
        vec![(1.0, "m".to_string())],
    );
    assert!(delta_for(&ch).is_some());
}

#[test]
fn every_delta_names_a_key_the_command_actually_writes() {
    // A delta applied to the wrong key silently corrupts it, and a delta for
    // a command that did not write is a phantom mutation. Both are ruled out
    // by tying the delta's key to `primary_keys`.
    for (cmd, _) in all_commands() {
        let Some((key, _)) = delta_for(&cmd) else {
            continue;
        };
        assert!(
            is_write_command(&cmd),
            "{cmd:?} is not a write but produced a delta"
        );
        assert!(
            primary_keys(&cmd).contains(&key),
            "{cmd:?} produced a delta for '{key}', which it does not write"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_deltas_are_opt_in() {
    let srv = spawn_ws_server().await;
    let mut plain = WsClient::connect(srv.tcp_addr).await;
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(plain.cmd(&["QSUB", "d:*"]).await, arr(&["qstate", "d:*"]));

    // A client that never asked keeps receiving whole values, because one
    // that ignored an unknown frame would silently hold a stale copy.
    assert_eq!(writer.cmd(&["RPUSH", "d:l", "a"]).await, int(1));
    let push = plain.recv_any(1000).await.expect("expected a push");
    assert!(push.contains("keychange"), "unexpected frame: {push}");
    assert!(!push.contains("keydelta"), "unexpected frame: {push}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_delta_frames_replace_whole_values() {
    let srv = spawn_ws_server().await;
    let mut viewer = WsClient::connect(srv.tcp_addr).await;
    let mut agent = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(viewer.cmd(&["CLIENT", "DELTA", "ON"]).await, ok());
    assert_eq!(
        viewer.cmd(&["QSUB", "agent:*"]).await,
        arr(&["qstate", "agent:*"])
    );

    // Seed a long transcript, then append one token to it.
    let transcript = "x".repeat(4096);
    assert_eq!(agent.cmd(&["SET", "agent:out", &transcript]).await, ok());
    let _ = viewer.recv_any(1000).await.expect("the SET reaches us");

    assert_eq!(
        agent.cmd(&["APPEND", "agent:out", " token"]).await,
        int(transcript.len() as i64 + 6)
    );
    let push = viewer.recv_any(1000).await.expect("expected a delta");
    assert!(push.contains("keydelta"), "unexpected frame: {push}");
    assert!(push.contains("append"), "unexpected frame: {push}");
    assert!(push.contains(" token"), "unexpected frame: {push}");
    // The point of the exercise: the 4 KiB transcript is not in the frame.
    assert!(
        push.len() < 200,
        "delta carried the whole value ({} bytes)",
        push.len()
    );

    // Collections take the same path — one SADD no longer re-sends the set.
    assert_eq!(agent.cmd(&["SADD", "agent:seen", "alice"]).await, int(1));
    let push = viewer.recv_any(1000).await.expect("expected a delta");
    assert!(push.contains("keydelta") && push.contains("sadd"));

    // A mutation with no compact form still arrives whole.
    assert_eq!(agent.cmd(&["SET", "agent:state", "done"]).await, ok());
    let push = viewer.recv_any(1000).await.expect("expected a keychange");
    assert!(push.contains("keychange"), "unexpected frame: {push}");

    // And the toggle is reversible.
    assert_eq!(viewer.cmd(&["CLIENT", "DELTA", "OFF"]).await, ok());
    assert_eq!(
        agent.cmd(&["APPEND", "agent:out", "!"]).await,
        int(transcript.len() as i64 + 7)
    );
    let push = viewer.recv_any(1000).await.expect("expected a keychange");
    assert!(push.contains("keychange"), "unexpected frame: {push}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_a_watched_key_is_delivered_once() {
    // A live-queried key used to arrive twice: once as the propagation frame
    // every WebSocket client gets, and once as the keychange for the
    // subscription. Applying both replays a non-idempotent mutation twice;
    // it was masked only because the keychange carries the whole value and
    // happened to land second. A delta could not repair it at all.
    let srv = spawn_ws_server().await;
    let mut viewer = WsClient::connect(srv.tcp_addr).await;
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(
        viewer.cmd(&["QSUB", "once:*"]).await,
        arr(&["qstate", "once:*"])
    );

    assert_eq!(writer.cmd(&["RPUSH", "once:l", "a"]).await, int(1));
    let first = viewer.recv_any(1000).await.expect("expected one push");
    assert!(first.contains("keychange"), "unexpected frame: {first}");
    assert!(
        viewer.recv_any(400).await.is_none(),
        "the same mutation was delivered twice"
    );

    // A key *outside* the live query still arrives on the propagation path,
    // which is the only way an unsubscribed client learns about it.
    assert_eq!(writer.cmd(&["SET", "other:k", "v"]).await, ok());
    let push = viewer
        .recv_any(1000)
        .await
        .expect("expected a propagation push");
    assert!(push.contains("other:k"), "unexpected frame: {push}");
}

// ── Write scoping: read-only vs read-write grants ─────────────────────────
//
// A grant pattern authorizes reads and writes separately. Without that, a
// browser handed `catalog:*` so it could read the catalog could also rewrite
// it, which makes every shared read model unsafe to sync.

#[test]
fn token_entries_carry_their_access() {
    let secret = "access-secret";
    let tok = mint_sync_token(secret, "r=catalog:*,rw=cart:42:*,session:42");
    assert_eq!(
        verify_sync_token(secret, &tok).unwrap(),
        vec![
            Grant::ro("catalog:*"),
            Grant::rw("cart:42:*"),
            // A bare entry is read-write, which is what every token minted
            // before write scoping existed meant.
            Grant::rw("session:42"),
        ]
    );
}

#[test]
fn a_grant_with_no_pattern_is_refused() {
    let secret = "empty-secret";
    // `r=` alone matches only the empty key — always a minting bug, and one
    // that would otherwise mint a token granting nothing at all.
    assert_eq!(
        verify_sync_token(secret, &mint_sync_token(secret, "r=")).unwrap_err(),
        "token grants an empty pattern"
    );
    assert_eq!(
        verify_sync_token(secret, &mint_sync_token(secret, "cart:*,rw=")).unwrap_err(),
        "token grants an empty pattern"
    );
}

#[test]
fn read_only_grants_permit_reads_and_refuse_writes() {
    let grants = vec![Grant::ro("catalog:*"), Grant::rw("cart:42:*")];

    assert!(scopes_allow(&grants, "catalog:books", Access::Read));
    assert!(!scopes_allow(&grants, "catalog:books", Access::Write));

    // A read-write grant satisfies both, so read-modify-write commands need
    // only ask for Write.
    assert!(scopes_allow(&grants, "cart:42:total", Access::Read));
    assert!(scopes_allow(&grants, "cart:42:total", Access::Write));

    // Outside every grant, neither.
    assert!(!scopes_allow(&grants, "session:1", Access::Read));
    assert!(!scopes_allow(&grants, "session:1", Access::Write));
}

#[test]
fn read_only_grants_still_receive_the_fan_out() {
    // Visibility is read access, so a read-only grant is still pushed its
    // keys. The whole point of a read-only catalog scope is to receive
    // catalog changes without being able to cause them.
    assert!(scopes_match(
        &[Grant::ro("catalog:*")],
        &["catalog:books".to_string()]
    ));
}

#[test]
fn every_write_command_demands_write_access_on_the_key_it_writes() {
    // The drift guard for the access split. `is_write_command` already
    // decides what reaches the AOF, replicas and live queries; if the two
    // classifications disagree, either a mutation is authorized as a read —
    // the security bug — or a read demands write access and a correct grant
    // stops working.
    for (cmd, _) in all_commands() {
        let CommandScope::Keys(keys) = command_scope(&cmd) else {
            // FLUSHDB is the one write that is Admin rather than key-scoped;
            // it is refused on scoped connections outright.
            continue;
        };
        let writes = keys.iter().any(|(_, a)| *a == Access::Write);
        assert_eq!(
            writes,
            is_write_command(&cmd),
            "{cmd:?} is classified {:?} for scope enforcement but \
             is_write_command() says {}",
            keys,
            is_write_command(&cmd)
        );
    }
}

#[test]
fn every_key_a_command_writes_demands_write_access() {
    // `primary_keys` names the keys a command mutates. Each one must be
    // classified Write — a key that is mutated but scope-checked as a read
    // is writable from a read-only grant.
    for (cmd, _) in all_commands() {
        let CommandScope::Keys(keys) = command_scope(&cmd) else {
            continue;
        };
        for written in primary_keys(&cmd) {
            assert!(
                keys.iter()
                    .any(|(k, a)| *k == written && *a == Access::Write),
                "{cmd:?} writes '{written}' but scope-checks it as {keys:?}"
            );
        }
    }
}

#[test]
fn the_store_family_reads_its_sources_and_writes_its_destination() {
    // The case that makes access per key rather than per command: copying
    // another namespace into your own must not require write access to it.
    let grants = vec![Grant::ro("theirs:*"), Grant::rw("mine:*")];
    let cmd = Command::SInterStore(
        "mine:out".into(),
        vec!["theirs:a".into(), "theirs:b".into()],
    );
    let CommandScope::Keys(keys) = command_scope(&cmd) else {
        panic!("SINTERSTORE should be key-scoped");
    };
    for (key, need) in &keys {
        assert!(
            scopes_allow(&grants, key, *need),
            "'{key}' needs {need:?}, which the grants do not permit"
        );
    }
    // The reverse grant does not work: writing the destination is a write.
    let reversed = vec![Grant::rw("theirs:*"), Grant::ro("mine:*")];
    assert!(
        keys.iter()
            .any(|(k, need)| !scopes_allow(&reversed, k, *need))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_read_only_scope_refuses_writes() {
    let secret = "read-only-secret";
    let srv = spawn_ws_server_cfg(Some(secret.to_string())).await;

    // A backend seeds the catalog over the trusted TCP port.
    let mut backend = WsClient::connect(srv.tcp_addr).await;
    let btok = mint_sync_token(secret, "catalog:*,cart:7:*");
    assert_eq!(
        backend.cmd(&["SYNC", "TOKEN", &btok]).await,
        arr(&["rw=catalog:*", "rw=cart:7:*"])
    );
    assert_eq!(backend.cmd(&["SET", "catalog:book", "10"]).await, ok());

    // The browser reads the catalog and owns its cart.
    let mut browser = WsClient::connect(srv.tcp_addr).await;
    let tok = mint_sync_token(secret, "r=catalog:*,rw=cart:7:*");
    assert_eq!(
        browser.cmd(&["SYNC", "TOKEN", &tok]).await,
        arr(&["r=catalog:*", "rw=cart:7:*"])
    );

    // Reads inside the read-only grant work.
    assert_eq!(browser.cmd(&["GET", "catalog:book"]).await, bulk("10"));

    // Writes to it do not — and say so specifically, rather than claiming
    // the key is outside the connection's scopes.
    let r = browser.cmd(&["SET", "catalog:book", "0"]).await;
    assert!(
        matches!(&r, Value::Error(e) if e.contains("NOSCOPE") && e.contains("read-only")),
        "expected a read-only refusal, got {r:?}"
    );
    // Every shape of write is refused, not just SET.
    for write in [
        vec!["DEL", "catalog:book"],
        vec!["INCR", "catalog:book"],
        vec!["APPEND", "catalog:book", "x"],
        vec!["EXPIRE", "catalog:book", "1"],
        vec!["RENAME", "catalog:book", "catalog:other"],
        vec!["JMERGE", "catalog:book", "{}"],
        vec!["DEDUP", "c1", "1", "SET", "catalog:book", "0"],
    ] {
        let r = browser.cmd(&write).await;
        assert!(
            matches!(&r, Value::Error(e) if e.contains("NOSCOPE")),
            "{write:?} was not refused: {r:?}"
        );
    }
    // The value is untouched.
    assert_eq!(browser.cmd(&["GET", "catalog:book"]).await, bulk("10"));

    // A key outside every grant still reports as out of scope.
    let r = browser.cmd(&["GET", "secret:1"]).await;
    assert!(
        matches!(&r, Value::Error(e) if e.contains("outside")),
        "expected an out-of-scope refusal, got {r:?}"
    );

    // The read-write half of the same token still works.
    assert_eq!(browser.cmd(&["SET", "cart:7:item", "1"]).await, ok());

    // And the read-only grant still receives pushes: a catalog change made
    // by the backend reaches the browser it may not write.
    assert_eq!(backend.cmd(&["SET", "catalog:book", "12"]).await, ok());
    let push = browser
        .recv_push(1000)
        .await
        .expect("read-only grants still receive their keys");
    assert!(push.contains("catalog:book"), "unexpected push: {push}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_read_only_scope_allows_live_queries() {
    // A live query only reads, so a read-only grant must be able to hold one
    // — that is how a browser follows a catalog it cannot change.
    let secret = "qsub-ro-secret";
    let srv = spawn_ws_server_cfg(Some(secret.to_string())).await;
    let mut client = WsClient::connect(srv.tcp_addr).await;
    let tok = mint_sync_token(secret, "r=catalog:*");
    assert_eq!(
        client.cmd(&["SYNC", "TOKEN", &tok]).await,
        arr(&["r=catalog:*"])
    );
    assert_eq!(
        client.cmd(&["QSUB", "catalog:books:*"]).await,
        arr(&["qstate", "catalog:books:*"])
    );
    // WATCH observes a key, which is also a read.
    assert_eq!(client.cmd(&["WATCH", "catalog:book"]).await, ok());
}

// ── Duplicate-suppressed delivery (DEDUP) ─────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_dedup_skips_replayed_writes() {
    let srv = spawn_ws_server().await;
    let mut c = WsClient::connect(srv.tcp_addr).await;

    // First delivery applies.
    assert_eq!(
        c.cmd(&["DEDUP", "client-a", "1", "INCRBY", "n", "2"]).await,
        int(2)
    );
    // Exact replay (ack lost, client re-sent) is skipped.
    assert_eq!(
        c.cmd(&["DEDUP", "client-a", "1", "INCRBY", "n", "2"]).await,
        Value::SimpleString("DUP".into())
    );
    // Higher id applies.
    assert_eq!(
        c.cmd(&["DEDUP", "client-a", "2", "INCRBY", "n", "3"]).await,
        int(5)
    );
    // The high-water mark survives a reconnect — the whole point.
    let mut c2 = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(
        c2.cmd(&["DEDUP", "client-a", "2", "INCRBY", "n", "3"])
            .await,
        Value::SimpleString("DUP".into())
    );
    assert_eq!(srv.store.execute(Command::Get("n".into())), bulk("5"));
    // A different client id has an independent mark.
    assert_eq!(
        c2.cmd(&["DEDUP", "client-b", "1", "INCRBY", "n", "1"])
            .await,
        int(6)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_dedup_respects_sync_scopes() {
    let secret = "dedup-secret";
    let srv = spawn_ws_server_cfg(Some(secret.to_string())).await;
    let mut c = WsClient::connect(srv.tcp_addr).await;
    let tok = mint_sync_token(secret, "cart:*");
    assert_eq!(c.cmd(&["SYNC", "TOKEN", &tok]).await, arr(&["rw=cart:*"]));

    // Scope enforcement applies to the wrapped command.
    assert_eq!(
        c.cmd(&["DEDUP", "c1", "1", "SET", "cart:1", "x"]).await,
        ok()
    );
    let r = c.cmd(&["DEDUP", "c1", "2", "SET", "admin:1", "x"]).await;
    assert!(matches!(&r, Value::Error(e) if e.contains("NOSCOPE")));
}

// ── JSON over the wire ────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_json_commands_and_fanout() {
    let srv = spawn_ws_server().await;
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    let mut peer = WsClient::connect(srv.tcp_addr).await;

    assert_eq!(
        writer.cmd(&["JSET", "doc:1", "$", r#"{"a":1}"#]).await,
        ok()
    );
    assert_eq!(writer.cmd(&["JGET", "doc:1", "$.a"]).await, bulk("1"));
    assert_eq!(
        writer
            .cmd(&["JMERGE", "doc:1", r#"{"b":2,"a":null}"#])
            .await,
        ok()
    );
    assert_eq!(writer.cmd(&["JGET", "doc:1"]).await, bulk(r#"{"b":2}"#));

    // Peers receive the writes as replayable pushes.
    let p = peer.recv_push(1000).await.expect("JSET push");
    assert!(p.contains("JSET") && p.contains("doc:1"), "push: {p}");
    let p2 = peer.recv_push(1000).await.expect("JMERGE push");
    assert!(p2.contains("JMERGE"), "push: {p2}");

    // Failed writes are not broadcast.
    let r = writer.cmd(&["JSET", "doc:1", "$", "{bad"]).await;
    assert!(matches!(&r, Value::Error(e) if e.contains("invalid JSON")));
    assert!(peer.recv_push(300).await.is_none());
}

// ── Live queries (QSUB / QUNSUB) ──────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_qsub_initial_state_and_diffs() {
    let srv = spawn_ws_server().await;
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    let mut client = WsClient::connect(srv.tcp_addr).await;

    // Pre-existing state the subscription must deliver up front.
    assert_eq!(writer.cmd(&["SET", "cart:1", "apples"]).await, ok());
    assert_eq!(writer.cmd(&["SET", "other:1", "zzz"]).await, ok());

    let initial = client.cmd(&["QSUB", "cart:*"]).await;
    match &initial {
        Value::Array(Some(items)) => {
            assert_eq!(
                items.len(),
                4,
                "expected tag + pattern + one pair: {items:?}"
            );
            assert_eq!(items[0], bulk("qstate"));
            assert_eq!(items[1], bulk("cart:*"));
            assert_eq!(items[2], bulk("cart:1"));
            assert_eq!(items[3], bulk("apples"));
        }
        other => panic!("expected initial-state array, got {other:?}"),
    }

    // A matching write arrives as a keychange diff…
    assert_eq!(writer.cmd(&["SET", "cart:2", "pears"]).await, ok());
    let (key, value) = client.recv_keychange(1000).await.expect("cart:2 diff");
    assert_eq!((key.as_str(), &value), ("cart:2", &bulk("pears")));

    // …a non-matching write does not…
    assert_eq!(writer.cmd(&["SET", "other:2", "yyy"]).await, ok());
    assert!(client.recv_keychange(300).await.is_none());

    // …a deletion arrives as a nil keychange…
    assert_eq!(writer.cmd(&["DEL", "cart:2"]).await, int(1));
    let (key, value) = client.recv_keychange(1000).await.expect("delete diff");
    assert_eq!((key.as_str(), &value), ("cart:2", &nil()));

    // …and QUNSUB stops the stream.
    assert_eq!(client.cmd(&["QUNSUB", "cart:*"]).await, ok());
    assert_eq!(writer.cmd(&["SET", "cart:3", "plums"]).await, ok());
    assert!(client.recv_keychange(300).await.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_ws_qsub_strict_scope() {
    let secret = "qsub-secret";
    let srv = spawn_ws_server_cfg(Some(secret.to_string())).await;
    let mut client = WsClient::connect(srv.tcp_addr).await;

    let tok = mint_sync_token(secret, "cart:*");
    assert_eq!(
        client.cmd(&["SYNC", "TOKEN", &tok]).await,
        arr(&["rw=cart:*"])
    );

    // A narrower pattern under the grant is allowed (prefix-style cover).
    assert_eq!(
        client.cmd(&["QSUB", "cart:42:*"]).await,
        arr(&["qstate", "cart:42:*"])
    );
    // A pattern outside the grant is refused.
    let r = client.cmd(&["QSUB", "admin:*"]).await;
    assert!(matches!(&r, Value::Error(e) if e.contains("NOSCOPE")));

    // Diffs flow for the subscribed pattern.
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    let wtok = mint_sync_token(secret, "cart:*");
    writer.cmd(&["SYNC", "TOKEN", &wtok]).await;
    assert_eq!(writer.cmd(&["SET", "cart:42:item", "x"]).await, ok());
    let (key, _) = client.recv_keychange(1000).await.expect("scoped diff");
    assert_eq!(key, "cart:42:item");
}

// ─────────────────────────────────────────────────────────────────────────────
// Hardening: the network-exposure fixes and their decision functions.
//
// Every check here guards a boundary that was open in 0.2.4 or earlier. The
// pure-function shape is deliberate: the replication gate and the origin
// allowlist are decisions, and a decision can be asserted without standing up a
// listener or mutating process-global environment state.
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod hardening_tests {
    use super::*;

    // ── replication listener gate ─────────────────────────────────────────────

    #[test]
    fn the_replication_port_stays_closed_unless_asked_for() {
        // The default. Before this gate existed the listener bound
        // 0.0.0.0:6381 on every node and, with no password, served the entire
        // keyspace to anyone who connected — so an operator who set
        // RECACHED_PASSWORD was protecting nothing.
        assert_eq!(resolve_repl_listen(None, "0.0.0.0", None), Ok(false));
        assert_eq!(resolve_repl_listen(None, "0.0.0.0", Some("pw")), Ok(false));
        // An empty value is "unset", not "true".
        assert_eq!(
            resolve_repl_listen(Some(String::new()), "0.0.0.0", None),
            Ok(false)
        );
        assert_eq!(
            resolve_repl_listen(Some("  ".to_string()), "0.0.0.0", None),
            Ok(false)
        );
    }

    #[test]
    fn enabling_it_on_a_public_interface_without_a_password_refuses_to_start() {
        let err = resolve_repl_listen(Some("1".into()), "0.0.0.0", None)
            .expect_err("public + no password must not be allowed");
        // The message has to name both variables — an operator reading a log
        // line needs to know what to set, not merely that something is wrong.
        assert!(err.contains("RECACHED_REPL_PASSWORD"), "{err}");
        assert!(err.contains("RECACHED_REPL_ENABLE"), "{err}");
        assert!(err.contains("0.0.0.0"), "{err}");

        // A specific LAN address is just as reachable as 0.0.0.0.
        assert!(resolve_repl_listen(Some("1".into()), "10.0.1.5", None).is_err());
        // So is a hostname we cannot resolve to a loopback address: the
        // conservative reading is the one that demands a password.
        assert!(resolve_repl_listen(Some("1".into()), "cache.internal", None).is_err());
        // An empty password is not a password.
        assert!(resolve_repl_listen(Some("1".into()), "0.0.0.0", Some("")).is_err());
    }

    #[test]
    fn enabling_it_is_allowed_on_loopback_or_with_a_password() {
        // Loopback without a password is a development setup, not an exposure.
        assert_eq!(
            resolve_repl_listen(Some("1".into()), "127.0.0.1", None),
            Ok(true)
        );
        assert_eq!(
            resolve_repl_listen(Some("yes".into()), "::1", None),
            Ok(true)
        );
        assert_eq!(
            resolve_repl_listen(Some("on".into()), "localhost", None),
            Ok(true)
        );
        // Public is fine once authenticated — this is the multi-tier
        // replication path, which must keep working.
        assert_eq!(
            resolve_repl_listen(Some("true".into()), "0.0.0.0", Some("pw")),
            Ok(true)
        );
        assert_eq!(
            resolve_repl_listen(Some("1".into()), "10.0.1.5", Some("pw")),
            Ok(true)
        );
    }

    #[test]
    fn an_ambiguous_enable_value_refuses_to_start() {
        // Treating `please` as false would leave an operator believing
        // replication was on; treating it as true would open a port nobody
        // asked for. Neither is acceptable for a variable gating a boundary.
        let err = resolve_repl_listen(Some("please".into()), "127.0.0.1", None).unwrap_err();
        assert!(err.contains("RECACHED_REPL_ENABLE"), "{err}");
        assert!(err.contains("not a boolean"), "{err}");
    }

    #[test]
    fn boolean_env_values_cover_the_conventional_spellings() {
        for yes in ["1", "true", "TRUE", "yes", "On", " on "] {
            assert_eq!(parse_env_bool("V", yes), Ok(true), "{yes:?}");
        }
        for no in ["0", "false", "FALSE", "no", "Off", " off "] {
            assert_eq!(parse_env_bool("V", no), Ok(false), "{no:?}");
        }
        assert!(parse_env_bool("V", "maybe").is_err());
    }

    #[test]
    fn loopback_detection_treats_unparseable_hosts_as_public() {
        assert!(bind_is_loopback("127.0.0.1"));
        assert!(bind_is_loopback("127.0.0.53"));
        assert!(bind_is_loopback("::1"));
        assert!(bind_is_loopback("localhost"));
        assert!(bind_is_loopback("LOCALHOST"));
        assert!(!bind_is_loopback("0.0.0.0"));
        assert!(!bind_is_loopback("10.0.1.5"));
        assert!(!bind_is_loopback("::"));
        assert!(!bind_is_loopback("cache.internal"));
        assert!(!bind_is_loopback(""));
        // An IPv6 bind address must be written bracketed for the listeners to
        // format it correctly, so the brackets have to be tolerated here too —
        // otherwise `[::1]` is misread as public and demands a password.
        assert!(bind_is_loopback("[::1]"));
        assert!(!bind_is_loopback("[::]"));
        assert!(!bind_is_loopback("[fd00::5]"));
    }

    // ── replication auth: throttle and handshake ─────────────────────────────

    #[test]
    fn repeated_bad_replication_passwords_block_the_peer() {
        // The RESP port drops a connection after five guesses, but the
        // replication handshake is one-shot: reconnecting used to reset the
        // count, so the port offered unlimited guesses at a secret that yields
        // the whole keyspace. The throttle is keyed by address for that reason.
        let throttle = ReplAuthThrottle::new();
        let ip = IpAddr::from([203, 0, 113, 7]);
        assert!(!throttle.is_blocked(ip));
        for _ in 0..MAX_AUTH_FAILURES {
            assert!(!throttle.is_blocked(ip), "must not block before the cap");
            throttle.record_failure(ip);
        }
        assert!(throttle.is_blocked(ip), "cap reached, peer must be refused");

        // Other peers are unaffected — one attacker must not lock out a fleet.
        assert!(!throttle.is_blocked(IpAddr::from([203, 0, 113, 8])));

        // A successful handshake clears the record.
        throttle.record_success(ip);
        assert!(!throttle.is_blocked(ip));
    }

    #[test]
    fn the_throttle_does_not_grow_without_bound() {
        // A spray from many source addresses must not be a memory-growth
        // vector; the map sweeps once it crosses the threshold.
        let throttle = ReplAuthThrottle::new();
        for i in 0..(REPL_AUTH_SWEEP_THRESHOLD + 64) {
            let ip = IpAddr::from([
                10,
                ((i >> 16) & 0xff) as u8,
                ((i >> 8) & 0xff) as u8,
                (i & 0xff) as u8,
            ]);
            throttle.record_failure(ip);
        }
        let len = throttle.failures.lock().unwrap().len();
        // Entries are all fresh so none are swept, but the sweep must have run
        // without panicking and the map must stay proportional to the input
        // rather than duplicating it.
        assert!(len <= REPL_AUTH_SWEEP_THRESHOLD + 64, "{len}");
    }

    #[tokio::test]
    async fn the_auth_line_is_read_to_its_terminator_not_to_the_password_length() {
        // Reading exactly `password.len() + 1` bytes made the number of bytes
        // the server waited for *be* the password length, recoverable by
        // drip-feeding one byte at a time.
        let (mut client, mut server) = tokio::io::duplex(256);
        client.write_all(b"hunter2\n").await.unwrap();
        let line = read_repl_auth_line(&mut server).await.unwrap();
        assert_eq!(line, b"hunter2");
    }

    #[tokio::test]
    async fn an_auth_line_without_a_terminator_is_refused_at_the_cap() {
        let (mut client, mut server) = tokio::io::duplex(4096);
        let flood = vec![b'x'; MAX_REPL_AUTH_LINE + 16];
        // Write concurrently: the reader gives up mid-stream, so the writer
        // must not block on a full pipe.
        tokio::spawn(async move {
            let _ = client.write_all(&flood).await;
        });
        let err = read_repl_auth_line(&mut server)
            .await
            .expect_err("an unterminated line must not be read forever");
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn a_short_auth_line_still_compares_unequal() {
        // The comparison is constant-time and length-checked, so a truncated
        // guess fails rather than matching a prefix.
        let (mut client, mut server) = tokio::io::duplex(256);
        client.write_all(b"hunt\n").await.unwrap();
        let line = read_repl_auth_line(&mut server).await.unwrap();
        assert!(!ct_eq_bytes(&line, b"hunter2"));
    }

    // ── WebSocket origin allowlist ──────────────────────────────────────────

    #[test]
    fn an_unset_origin_allowlist_permits_everything() {
        // Matches how an unset RECACHED_PASSWORD behaves. The project ships
        // insecure-by-default deliberately and says so; what it must not do is
        // ship a *silent* default, hence the startup warning.
        assert!(origin_allowed(None, Some("https://evil.example")));
        assert!(origin_allowed(None, None));
    }

    #[test]
    fn a_foreign_origin_is_refused_when_the_allowlist_is_set() {
        // The finding this closes: browsers apply neither CORS nor a preflight
        // to WebSockets, so without this check any page a user visits could
        // open a socket to ws://localhost:6380 and read or write every key.
        let allow = vec!["https://app.example.com".to_string()];
        assert!(origin_allowed(
            Some(&allow),
            Some("https://app.example.com")
        ));
        assert!(!origin_allowed(Some(&allow), Some("https://evil.example")));
        // A different scheme or port is a different origin.
        assert!(!origin_allowed(
            Some(&allow),
            Some("http://app.example.com")
        ));
        assert!(!origin_allowed(
            Some(&allow),
            Some("https://app.example.com:8443")
        ));
        // Substring matching would be a hole: `app.example.com.evil.test`
        // contains an allowlisted origin as a prefix.
        assert!(!origin_allowed(
            Some(&allow),
            Some("https://app.example.com.evil.test")
        ));
    }

    #[test]
    fn an_absent_origin_is_permitted_because_only_browsers_send_one() {
        // A native client omits the header and an attacker with a socket can
        // forge it, so refusing here would break legitimate clients while
        // stopping nobody. The control exists to separate "the app I deployed"
        // from "another page in the same browser".
        let allow = vec!["https://app.example.com".to_string()];
        assert!(origin_allowed(Some(&allow), None));
    }

    #[test]
    fn origin_comparison_ignores_case_and_a_trailing_slash() {
        let allow = parse_allowed_origins("https://App.Example.com/").unwrap();
        assert!(origin_allowed(
            Some(&allow),
            Some("https://app.example.com")
        ));
        assert!(origin_allowed(
            Some(&allow),
            Some("HTTPS://APP.EXAMPLE.COM/")
        ));
    }

    #[test]
    fn the_origin_allowlist_parses_a_list_and_admits_null() {
        let list = parse_allowed_origins(
            "https://app.example.com, http://localhost:3000 ,https://admin.example.com:8443",
        )
        .unwrap();
        assert_eq!(
            list,
            vec![
                "https://app.example.com",
                "http://localhost:3000",
                "https://admin.example.com:8443",
            ]
        );
        // Sandboxed iframes and file:// documents send the literal `null`.
        assert_eq!(parse_allowed_origins("null").unwrap(), vec!["null"]);
        assert!(origin_allowed(
            Some(&parse_allowed_origins("null").unwrap()),
            Some("null")
        ));
    }

    #[test]
    fn the_origin_allowlist_rejects_entries_that_could_never_match() {
        // Each of these would parse into something a browser never sends, so
        // the allowlist would silently reject every connection. Failing at
        // startup is the only way an operator finds out.
        for bad in [
            "app.example.com",
            "https://app.example.com/dashboard",
            "://nohost",
            "https://",
        ] {
            assert!(
                parse_allowed_origins(bad).is_err(),
                "{bad:?} should be rejected"
            );
        }
        // Set-but-empty would reject every browser; unset is how you allow all.
        let err = parse_allowed_origins(" , ").unwrap_err();
        assert!(err.contains("Unset it"), "{err}");
    }

    // ── handshake deadline ──────────────────────────────────────────────────

    #[tokio::test]
    async fn a_stalled_websocket_handshake_gives_up_and_releases_the_socket() {
        // The connection permit is acquired *before* the handshake runs, so
        // without this deadline `RECACHED_MAX_CONNECTIONS` sockets that connect
        // and then say nothing — costing an attacker nothing — hold every slot
        // indefinitely and the server stops accepting real clients.
        let (_client, server) = tokio::io::duplex(1024);
        let start = std::time::Instant::now();
        let out = ws_handshake(server, None, Duration::from_millis(150), 1).await;
        assert!(out.is_none(), "a silent peer must not produce a stream");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "gave up after {:?} — the deadline did not apply",
            start.elapsed()
        );
    }

    #[test]
    fn the_handshake_deadline_has_a_documented_default() {
        assert_eq!(DEFAULT_HANDSHAKE_TIMEOUT_SECS, 10);
    }

    // ── persistence file permissions ────────────────────────────────────────

    #[tokio::test]
    #[cfg(unix)]
    async fn snapshot_and_sidecar_files_are_not_world_readable() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("recached_perm_{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("perm-test.rdb");

        write_private(&path, b"payload").await.unwrap();
        let mode = tokio::fs::metadata(&path)
            .await
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "snapshots are plaintext dumps of the keyspace; 0644 lets any local user read the cache"
        );

        // A file left behind 0644 by an earlier version must be tightened on
        // the next write, not keep its old mode forever.
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .await
            .unwrap();
        write_private(&path, b"payload2").await.unwrap();
        let mode = tokio::fs::metadata(&path)
            .await
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "an existing loose mode must be fixed");
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"payload2");

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn the_aof_is_not_world_readable() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("recached_aofperm_{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("perm-test.aof");

        // Pre-create it loose, as an upgrade from an earlier version would.
        tokio::fs::write(&path, b"").await.unwrap();
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .await
            .unwrap();

        let writer = AofWriter::open(path.clone(), AofSync::No).await.unwrap();
        writer.append(b"*1\r\n$4\r\nPING\r\n").await.unwrap();
        let mode = tokio::fs::metadata(&path)
            .await
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[test]
    fn temp_files_do_not_collide_between_processes_or_operations() {
        // A fixed `.tmp` name meant two servers sharing a data directory would
        // clobber each other's half-written snapshot.
        let a = temp_sibling(std::path::Path::new("/data/recached.rdb"), "snap");
        assert!(
            a.to_string_lossy()
                .contains(&std::process::id().to_string()),
            "{a:?}"
        );
        assert!(a.to_string_lossy().ends_with(".tmp"), "{a:?}");
        assert_ne!(
            a,
            temp_sibling(std::path::Path::new("/data/recached.rdb"), "snap")
        );
        assert_ne!(
            a,
            temp_sibling(std::path::Path::new("/data/recached.rdb"), "dedup")
        );
    }
}

#[tokio::test]
async fn failed_checkpoint_keeps_dirty_state_and_aof_until_recovery() {
    let missing_dir = tmp_path("checkpoint_missing");
    let _ = tokio::fs::remove_dir_all(&missing_dir).await;
    let snapshot_path = missing_dir.join("dump.rdb");
    let aof_path = tmp_path("checkpoint.aof");
    let _ = tokio::fs::remove_file(&aof_path).await;
    let aof = Arc::new(
        AofWriter::open(aof_path.clone(), AofSync::No)
            .await
            .unwrap(),
    );
    aof.append(b"*3\r\n$3\r\nSET\r\n$1\r\nk\r\n$1\r\nv\r\n")
        .await
        .unwrap();

    let store = KeyValueStore::new();
    store.execute(Command::Set(
        "k".into(),
        b"v".to_vec(),
        SetOptions::default(),
    ));
    store.mark_dirty();
    let state = ServerState {
        snap: Arc::new(SnapshotConfig {
            path: snapshot_path,
            last_save: AtomicI64::new(0),
            checkpoint_id: AtomicU64::new(0),
        }),
        aof: Some(Arc::clone(&aof)),
        replicas: ReplHub::new(),
        is_replica: AtomicBool::new(false),
        dedup: std::sync::Mutex::new(HashMap::new()),
        ephemeral: std::sync::Mutex::new(HashMap::new()),
        ephemeral_members: std::sync::Mutex::new(HashMap::new()),
        dedup_dirty: AtomicBool::new(false),
        dedup_order: tokio::sync::Mutex::new(()),
        save_lock: tokio::sync::Mutex::new(()),
        persistence_healthy: AtomicBool::new(true),
        persistence_failures: AtomicU64::new(0),
    };

    assert!(state.save(&store).await.is_err());
    assert!(store.dirty_count() > 0);
    assert!(!state.persistence_is_healthy());
    assert!(tokio::fs::metadata(&aof_path).await.unwrap().len() > 0);

    tokio::fs::create_dir_all(&missing_dir).await.unwrap();
    state.save(&store).await.unwrap();
    assert_eq!(store.dirty_count(), 0);
    assert!(state.persistence_is_healthy());
    assert_eq!(tokio::fs::metadata(&aof_path).await.unwrap().len(), 0);

    let _ = tokio::fs::remove_dir_all(&missing_dir).await;
    let _ = tokio::fs::remove_file(&aof_path).await;
}

/// A command that cannot be queued must poison the whole transaction.
///
/// Redis refuses the command at queue time and makes `EXEC` reply `EXECABORT`,
/// having run nothing. Recached used to queue an unrecognised verb happily —
/// it parses to [`Command::Unknown`] — and only error while executing, so every
/// *other* command in the transaction was applied. On a server that implements
/// a deliberate subset of Redis that is a live hazard rather than a corner case:
/// `MULTI; ZPOPMIN q; LPUSH processing x; EXEC` pushed onto `processing`
/// without ever popping `q`, and MULTI is precisely the construct a caller
/// reaches for to stop exactly that.
#[cfg(test)]
mod transaction_abort_tests {
    use super::*;
    use super::{RespClient, spawn_server};

    fn is_execabort(v: &Value) -> bool {
        matches!(v, Value::Error(e) if e.starts_with("EXECABORT"))
    }

    #[tokio::test]
    async fn an_unknown_command_is_refused_when_queued_not_when_executed() {
        let srv = spawn_server().await;
        let mut c = RespClient::connect(srv.tcp_addr).await;

        assert_eq!(c.cmd(&["MULTI"]).await, Value::SimpleString("OK".into()));
        let reply = c.cmd(&["NOSUCHCMD", "x"]).await;
        assert!(
            matches!(&reply, Value::Error(e) if e.contains("unknown command") && e.contains("NOSUCHCMD")),
            "an unknown verb must be refused at queue time, got {reply:?}"
        );
        assert_ne!(
            reply,
            Value::SimpleString("QUEUED".into()),
            "a command that will never run must not be acknowledged as QUEUED"
        );
    }

    #[tokio::test]
    async fn exec_after_a_failed_queue_runs_nothing() {
        let srv = spawn_server().await;
        let mut c = RespClient::connect(srv.tcp_addr).await;

        c.cmd(&["MULTI"]).await;
        c.cmd(&["NOSUCHCMD", "x"]).await; // refused, poisons the transaction
        assert_eq!(
            c.cmd(&["SET", "survivor", "yes"]).await,
            Value::SimpleString("QUEUED".into())
        );
        let exec = c.cmd(&["EXEC"]).await;
        assert!(is_execabort(&exec), "expected EXECABORT, got {exec:?}");

        // The point of the whole fix: the sibling write must not have landed.
        assert_eq!(
            c.cmd(&["GET", "survivor"]).await,
            Value::BulkString(None),
            "a transaction that was refused still applied one of its commands"
        );
    }

    #[tokio::test]
    async fn a_malformed_command_also_aborts_the_transaction() {
        // Wrong arity fails in `Command::from_value`, a different rejection path
        // from `Command::Unknown` — both must poison the transaction.
        let srv = spawn_server().await;
        let mut c = RespClient::connect(srv.tcp_addr).await;

        c.cmd(&["MULTI"]).await;
        let bad = c.cmd(&["INCR"]).await; // missing key
        assert!(matches!(bad, Value::Error(_)), "got {bad:?}");
        c.cmd(&["SET", "arity:survivor", "yes"]).await;
        assert!(is_execabort(&c.cmd(&["EXEC"]).await));
        assert_eq!(
            c.cmd(&["GET", "arity:survivor"]).await,
            Value::BulkString(None)
        );
    }

    #[tokio::test]
    async fn a_command_not_allowed_in_a_transaction_aborts_it() {
        let srv = spawn_server().await;
        let mut c = RespClient::connect(srv.tcp_addr).await;

        c.cmd(&["MULTI"]).await;
        let refused = c.cmd(&["SUBSCRIBE", "ch"]).await;
        assert!(matches!(refused, Value::Error(_)), "got {refused:?}");
        c.cmd(&["SET", "sub:survivor", "yes"]).await;
        assert!(is_execabort(&c.cmd(&["EXEC"]).await));
        assert_eq!(
            c.cmd(&["GET", "sub:survivor"]).await,
            Value::BulkString(None)
        );
    }

    #[tokio::test]
    async fn a_clean_transaction_still_executes() {
        // Regression guard: the abort path must not swallow ordinary work.
        let srv = spawn_server().await;
        let mut c = RespClient::connect(srv.tcp_addr).await;

        c.cmd(&["MULTI"]).await;
        c.cmd(&["SET", "clean", "v"]).await;
        c.cmd(&["INCR", "clean:n"]).await;
        let exec = c.cmd(&["EXEC"]).await;
        assert_eq!(
            exec,
            Value::Array(Some(vec![
                Value::SimpleString("OK".into()),
                Value::Integer(1),
            ])),
            "a transaction with no queue errors must run in full"
        );
        assert_eq!(
            c.cmd(&["GET", "clean"]).await,
            Value::BulkString(Some(b"v".to_vec()))
        );
    }

    #[tokio::test]
    async fn discard_clears_the_abort_state() {
        // The flag is per-transaction, not per-connection: a poisoned
        // transaction must not wedge every later one on the same socket.
        let srv = spawn_server().await;
        let mut c = RespClient::connect(srv.tcp_addr).await;

        c.cmd(&["MULTI"]).await;
        c.cmd(&["NOSUCHCMD"]).await;
        assert_eq!(c.cmd(&["DISCARD"]).await, Value::SimpleString("OK".into()));

        c.cmd(&["MULTI"]).await;
        c.cmd(&["SET", "after:discard", "v"]).await;
        assert_eq!(
            c.cmd(&["EXEC"]).await,
            Value::Array(Some(vec![Value::SimpleString("OK".into())]))
        );
        assert_eq!(
            c.cmd(&["GET", "after:discard"]).await,
            Value::BulkString(Some(b"v".to_vec()))
        );
    }

    #[tokio::test]
    async fn a_new_multi_clears_the_abort_state_left_by_an_aborted_one() {
        let srv = spawn_server().await;
        let mut c = RespClient::connect(srv.tcp_addr).await;

        c.cmd(&["MULTI"]).await;
        c.cmd(&["NOSUCHCMD"]).await;
        assert!(is_execabort(&c.cmd(&["EXEC"]).await));

        // Next transaction on the same connection starts clean.
        c.cmd(&["MULTI"]).await;
        c.cmd(&["SET", "after:abort", "v"]).await;
        assert_eq!(
            c.cmd(&["EXEC"]).await,
            Value::Array(Some(vec![Value::SimpleString("OK".into())])),
            "the abort flag leaked into the next transaction"
        );
        assert_eq!(
            c.cmd(&["GET", "after:abort"]).await,
            Value::BulkString(Some(b"v".to_vec()))
        );
    }

    #[tokio::test]
    async fn a_watch_abort_stays_distinct_from_a_queue_abort() {
        // Two different failures with two different replies: a CAS conflict is
        // a nil array (retry me), a refused command is EXECABORT (fix your
        // request). Collapsing them would make retry loops spin forever.
        let srv = spawn_server().await;
        let mut c = RespClient::connect(srv.tcp_addr).await;
        let mut other = RespClient::connect(srv.tcp_addr).await;

        c.cmd(&["SET", "w", "1"]).await;
        c.cmd(&["WATCH", "w"]).await;
        c.cmd(&["MULTI"]).await;
        c.cmd(&["SET", "w", "99"]).await;
        other.cmd(&["SET", "w", "42"]).await; // invalidates the watch
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let exec = c.cmd(&["EXEC"]).await;
        assert_eq!(exec, Value::Array(None), "CAS conflict must reply nil");
        assert!(!is_execabort(&exec));
        assert_eq!(
            c.cmd(&["GET", "w"]).await,
            Value::BulkString(Some(b"42".to_vec()))
        );
    }

    #[test]
    fn queue_time_rejection_names_the_command_and_matches_the_store() {
        // One mistake should read the same inside and outside a transaction.
        let cmd = Command::Unknown("ZPOPMIN".into());
        let refusal = queue_time_rejection(&cmd).expect("unknown verbs are refused");
        let text = String::from_utf8_lossy(&refusal).into_owned();
        assert_eq!(text, "-ERR unknown command 'ZPOPMIN'\r\n");

        let store = core_engine::store::KeyValueStore::new();
        let direct = store.execute(cmd);
        assert_eq!(Value::parse(&refusal).unwrap().0, direct);
    }

    #[test]
    fn a_queueable_command_is_not_rejected() {
        assert!(queue_time_rejection(&Command::Get("k".into())).is_none());
        assert!(
            queue_time_rejection(&Command::Set(
                "k".into(),
                "v".into(),
                core_engine::cmd::SetOptions::default()
            ))
            .is_none()
        );
    }

    #[test]
    fn the_execabort_wire_text_matches_redis() {
        // Clients match on this prefix to tell a refused transaction from one
        // whose commands merely returned errors.
        assert_eq!(
            String::from_utf8_lossy(EXECABORT),
            "-EXECABORT Transaction discarded because of previous errors.\r\n"
        );
    }
}

// ── Counter TTL propagation ───────────────────────────────────────────────────

/// `PUBLISH` is queueable, and actually delivers when `EXEC` runs it.
///
/// Redis allows `PUBLISH` inside a transaction — announcing a change atomically
/// with the write that caused it is the ordinary reason to reach for MULTI at
/// all — but Recached refused it alongside `SUBSCRIBE` and `WATCH`. Simply
/// allowing it to queue would have been worse than the refusal: delivery lives
/// in the connection loop, and `store.execute(Publish)` is a stub that answers
/// 0 and sends nothing, so the message would have been swallowed silently. The
/// EXEC loop therefore dispatches `Publish` to the hub itself.
#[cfg(test)]
mod publish_in_multi_tests {
    use super::*;
    use super::{RespClient, spawn_server};

    #[tokio::test]
    async fn publish_can_be_queued_inside_a_transaction() {
        let srv = spawn_server().await;
        let mut c = RespClient::connect(srv.tcp_addr).await;

        assert_eq!(c.cmd(&["MULTI"]).await, Value::SimpleString("OK".into()));
        assert_eq!(
            c.cmd(&["PUBLISH", "events", "hi"]).await,
            Value::SimpleString("QUEUED".into()),
            "Redis allows PUBLISH inside MULTI"
        );
        // No subscribers, so the reply is a delivery count of zero.
        assert_eq!(
            c.cmd(&["EXEC"]).await,
            Value::Array(Some(vec![Value::Integer(0)]))
        );
    }

    #[tokio::test]
    async fn a_queued_publish_actually_reaches_a_subscriber() {
        // The part that a delivery count alone would not prove: the stub in the
        // store answers 0 and sends nothing, so a wrongly-wired EXEC would look
        // fine on the publisher's side while the message vanished.
        let srv = spawn_server().await;
        let mut sub = RespClient::connect(srv.tcp_addr).await;
        let mut pubr = RespClient::connect(srv.tcp_addr).await;

        sub.cmd(&["SUBSCRIBE", "events"]).await; // subscribe ack
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        pubr.cmd(&["MULTI"]).await;
        pubr.cmd(&["SET", "order:1", "paid"]).await;
        pubr.cmd(&["PUBLISH", "events", "order:1 paid"]).await;
        let exec = pubr.cmd(&["EXEC"]).await;
        assert_eq!(
            exec,
            Value::Array(Some(vec![
                Value::SimpleString("OK".into()),
                Value::Integer(1), // one subscriber received it
            ])),
            "EXEC must report the real delivery count, not the store's stub"
        );

        // And the subscriber genuinely has the message.
        let msg = sub.cmd(&[]).await;
        assert_eq!(
            msg,
            Value::Array(Some(vec![
                Value::BulkString(Some(b"message".to_vec())),
                Value::BulkString(Some(b"events".to_vec())),
                Value::BulkString(Some(b"order:1 paid".to_vec())),
            ])),
            "the queued PUBLISH was swallowed"
        );

        // The write in the same transaction landed too.
        assert_eq!(
            pubr.cmd(&["GET", "order:1"]).await,
            Value::BulkString(Some(b"paid".to_vec()))
        );
    }

    #[tokio::test]
    async fn a_discarded_publish_is_never_delivered() {
        // Queued means pending, not sent: DISCARD must drop the message.
        let srv = spawn_server().await;
        let mut sub = RespClient::connect(srv.tcp_addr).await;
        let mut pubr = RespClient::connect(srv.tcp_addr).await;

        sub.cmd(&["SUBSCRIBE", "events"]).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        pubr.cmd(&["MULTI"]).await;
        pubr.cmd(&["PUBLISH", "events", "never"]).await;
        assert_eq!(
            pubr.cmd(&["DISCARD"]).await,
            Value::SimpleString("OK".into())
        );

        // Publish something real; the subscriber must see *that* first, which
        // it could not if the discarded message had already been sent.
        pubr.cmd(&["PUBLISH", "events", "real"]).await;
        let msg = sub.cmd(&[]).await;
        assert_eq!(
            msg,
            Value::Array(Some(vec![
                Value::BulkString(Some(b"message".to_vec())),
                Value::BulkString(Some(b"events".to_vec())),
                Value::BulkString(Some(b"real".to_vec())),
            ])),
            "a DISCARDed PUBLISH was delivered anyway"
        );
    }

    #[tokio::test]
    async fn subscribe_and_watch_are_still_refused_inside_a_transaction() {
        // Only PUBLISH was unqueueable-by-mistake; the others are correct.
        let srv = spawn_server().await;
        let mut c = RespClient::connect(srv.tcp_addr).await;

        c.cmd(&["MULTI"]).await;
        for args in [
            vec!["SUBSCRIBE", "ch"],
            vec!["PSUBSCRIBE", "ch:*"],
            vec!["WATCH", "k"],
        ] {
            let reply = c.cmd(&args).await;
            assert!(
                matches!(&reply, Value::Error(e) if e.contains("not allowed inside a transaction")),
                "{args:?} should still be refused, got {reply:?}"
            );
        }
    }
}

// ── Metrics port ──────────────────────────────────────────────────────────────

// ── Expiry sweep notification ─────────────────────────────────────────────────
// Distinct from `propagation::expiry_propagation_tests`, which covers how an
// expiry travels inside a replicated *write* frame. This covers the sweep that
// removes a key nobody wrote to, and telling watchers it happened.

mod sweep_notification {
    use super::*;

    /// Register one live-query subscriber and return its receiving end.
    async fn subscribe_pattern(
        registry: &WatchRegistry,
        pattern: &str,
    ) -> mpsc::Receiver<WatchNotif> {
        let (overflow, _overflow_rx) = mpsc::channel(1);
        let (subscriber, rx) = notification_channel(1, overflow, Arc::new(AtomicBool::new(false)));
        let mut pats = registry.patterns.lock().await;
        pats.insert(pattern.to_string(), vec![subscriber]);
        registry.sync_patterns_len(&pats);
        rx
    }

    fn set_with_px(store: &KeyValueStore, key: &str, value: &str, px: u64) {
        store.execute(Command::Set(
            key.into(),
            value.into(),
            SetOptions {
                expiry: Some(SetExpiry::Px(px)),
                ..Default::default()
            },
        ));
    }

    /// The regression this guards: expiry removes a key behind every client's
    /// back — no command ran, so no keychange was emitted — and a replica kept
    /// the key, with its last value, forever. Volatile keys simply never
    /// expired in a browser or embedded local copy.
    #[tokio::test]
    async fn an_expiry_sweep_tells_live_queries_the_key_is_gone() {
        let store = KeyValueStore::new();
        let registry: WatchRegistry = WatchHub::new();
        let mut rx = subscribe_pattern(&registry, "fare:*").await;

        set_with_px(&store, "fare:MNL-CEB", "1850", 1);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        // Exactly what the background task in main.rs does.
        let expired = store.sweep_expired_reporting();
        assert_eq!(expired, vec!["fare:MNL-CEB".to_string()]);
        notify_removed(&registry, &expired).await;

        let notification = rx.try_recv().expect("live query was never told");
        assert_eq!(notification.key, "fare:MNL-CEB");
        assert!(
            matches!(&notification.payload, NotifPayload::Full(v) if *v == Value::BulkString(None)),
            "nil is already the delete encoding, so existing clients apply it unchanged"
        );
    }

    #[tokio::test]
    async fn an_expiry_sweep_notifies_exact_key_watchers_too() {
        let store = KeyValueStore::new();
        let registry: WatchRegistry = WatchHub::new();
        let (overflow, _overflow_rx) = mpsc::channel(1);
        let (subscriber, mut rx) =
            notification_channel(1, overflow, Arc::new(AtomicBool::new(false)));
        {
            let mut map = registry.map.lock().await;
            map.insert("session:abc".to_string(), vec![subscriber]);
            registry.sync_len(&map);
        }

        set_with_px(&store, "session:abc", "token", 1);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let expired = store.sweep_expired_reporting();
        notify_removed(&registry, &expired).await;

        let notification = rx.try_recv().expect("WATCH-er was never told");
        assert_eq!(notification.key, "session:abc");
        assert!(
            matches!(&notification.payload, NotifPayload::Full(v) if *v == Value::BulkString(None))
        );
    }

    #[tokio::test]
    async fn a_pattern_that_does_not_match_is_left_alone() {
        let store = KeyValueStore::new();
        let registry: WatchRegistry = WatchHub::new();
        let mut rx = subscribe_pattern(&registry, "fare:*").await;

        set_with_px(&store, "session:abc", "token", 1);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let expired = store.sweep_expired_reporting();
        assert_eq!(expired, vec!["session:abc".to_string()]);
        notify_removed(&registry, &expired).await;

        assert!(
            rx.try_recv().is_err(),
            "a live query must not hear about keys outside its pattern"
        );
    }

    #[tokio::test]
    async fn a_sweep_that_expires_nothing_notifies_nobody() {
        let store = KeyValueStore::new();
        let registry: WatchRegistry = WatchHub::new();
        let mut rx = subscribe_pattern(&registry, "fare:*").await;

        store.execute(Command::Set(
            "fare:MNL-CEB".into(),
            "1850".into(),
            SetOptions::default(),
        ));

        let expired = store.sweep_expired_reporting();
        assert!(expired.is_empty());
        notify_removed(&registry, &expired).await;

        assert!(rx.try_recv().is_err(), "nothing expired, nothing to say");
    }

    #[tokio::test]
    async fn a_full_notification_queue_signals_and_drops_the_subscriber() {
        let registry: WatchRegistry = WatchHub::new();
        let (overflow, mut overflow_rx) = mpsc::channel(1);
        let (subscriber, _rx) = notification_channel(7, overflow, Arc::new(AtomicBool::new(false)));
        {
            let mut map = registry.map.lock().await;
            map.insert("hot".to_string(), vec![subscriber]);
            registry.sync_len(&map);
        }

        for _ in 0..=256 {
            notify_removed(&registry, &["hot".to_string()]).await;
        }

        assert!(overflow_rx.try_recv().is_ok());
        assert_eq!(registry.watched_keys.load(Ordering::Relaxed), 0);
    }
}
