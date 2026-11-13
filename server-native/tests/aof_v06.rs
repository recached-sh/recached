//! AOF recovery for the 0.6 commands: run each new write against the real
//! binary with `RECACHED_AOF_SYNC=always`, SIGKILL it, start it again on the
//! same files, and read the state back. Every write must replay to exactly
//! what was acknowledged — including the propagated forms (GETDEL as DEL,
//! GETEX as PEXPIREAT/PERSIST/DEL, ZPOP* as ZREM, COPY verbatim).

use core_engine::resp::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
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

fn scratch() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("recached-aof-v06-{nanos:x}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start(dir: &Path) -> (Server, String) {
    let (port, ws) = (free_port(), free_port());
    let bin = std::env::var("RECACHED_TEST_SERVER_BIN")
        .unwrap_or_else(|_| env!("CARGO_BIN_EXE_recached-server").to_string());
    let child = Command::new(bin)
        .current_dir(dir)
        .env_clear()
        .env("RECACHED_BIND", "127.0.0.1")
        .env("RECACHED_PORT", port.to_string())
        .env("RECACHED_WS_PORT", ws.to_string())
        .env("RECACHED_METRICS_PORT", "0")
        .env("RECACHED_SAVE_PATH", dir.join("dump.rdb"))
        .env("RECACHED_AOF_PATH", dir.join("append.aof"))
        .env("RECACHED_AOF_SYNC", "always")
        .env("RECACHED_SAVE_INTERVAL", "0")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start recached-server");
    let addr = format!("127.0.0.1:{port}");
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(mut c) = TcpStream::connect(&addr) {
            // Ready means answering, not merely listening: replay runs first.
            let _ = c.write_all(b"*1\r\n$4\r\nPING\r\n");
            let mut b = [0u8; 16];
            c.set_read_timeout(Some(Duration::from_millis(500))).ok();
            if matches!(c.read(&mut b), Ok(n) if n > 0) {
                break;
            }
        }
        assert!(Instant::now() < deadline, "server did not start");
        std::thread::sleep(Duration::from_millis(25));
    }
    (Server(child), addr)
}

fn cmd(addr: &str, args: &[&str]) -> Value {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut req = format!("*{}\r\n", args.len()).into_bytes();
    for a in args {
        req.extend_from_slice(format!("${}\r\n{a}\r\n", a.len()).as_bytes());
    }
    stream.write_all(&req).unwrap();
    let mut buf = Vec::new();
    loop {
        match Value::parse(&buf) {
            Ok((v, _)) => return v,
            Err(e) if e.is_incomplete() => {
                let mut chunk = [0u8; 65536];
                let n = stream.read(&mut chunk).unwrap();
                assert!(n > 0);
                buf.extend_from_slice(&chunk[..n]);
            }
            Err(e) => panic!("{e}"),
        }
    }
}

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

#[test]
fn new_writes_survive_sigkill_and_replay_exactly() {
    let dir = scratch();
    let (server, addr) = start(&dir);
    let run = |args: &[&str]| cmd(&addr, args);
    for setup in [
        &["SET", "gd", "v"][..],
        &["SET", "gx", "v"],
        &["SET", "gp", "v", "EX", "500"],
        &["SET", "gpast", "v"],
        &["SET", "rn", "v"],
        &["HSET", "cp", "f", "v"],
        &["PEXPIREAT", "cp", "4102444800000"],
        &["ZADD", "zp", "1", "a", "2", "b", "3", "c", "4", "d"],
    ] {
        run(setup);
    }
    assert_eq!(run(&["GETDEL", "gd"]), bulk("v"));
    assert_eq!(run(&["GETEX", "gx", "PXAT", "4102444800777"]), bulk("v"));
    assert_eq!(run(&["GETEX", "gp", "PERSIST"]), bulk("v"));
    assert_eq!(run(&["GETEX", "gpast", "PXAT", "1"]), bulk("v"));
    assert_eq!(run(&["RENAMENX", "rn", "rn2"]), Value::Integer(1));
    assert_eq!(run(&["COPY", "cp", "cp2"]), Value::Integer(1));
    run(&["ZPOPMIN", "zp", "2"]);
    run(&["ZPOPMAX", "zp"]);
    drop(server); // SIGKILL: no shutdown checkpoint.

    let (_server, addr) = start(&dir);
    let run = |args: &[&str]| cmd(&addr, args);
    assert_eq!(run(&["EXISTS", "gd"]), Value::Integer(0), "GETDEL undone");
    assert_eq!(
        run(&["PEXPIRETIME", "gx"]),
        Value::Integer(4_102_444_800_777),
        "GETEX expiry lost"
    );
    assert_eq!(
        run(&["TTL", "gp"]),
        Value::Integer(-1),
        "GETEX PERSIST lost"
    );
    assert_eq!(
        run(&["EXISTS", "gpast"]),
        Value::Integer(0),
        "past GETEX resurrected"
    );
    assert_eq!(run(&["EXISTS", "rn"]), Value::Integer(0));
    assert_eq!(run(&["GET", "rn2"]), bulk("v"));
    assert_eq!(run(&["HGET", "cp2", "f"]), bulk("v"));
    assert_eq!(
        run(&["PEXPIRETIME", "cp2"]),
        Value::Integer(4_102_444_800_000),
        "COPY lost the source's expiry on replay"
    );
    assert_eq!(
        run(&["ZRANGE", "zp", "0", "-1"]),
        Value::Array(Some(vec![bulk("c")]))
    );
    let _ = std::fs::remove_dir_all(&dir);
}
