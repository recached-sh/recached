//! 6O-D: MSET's atomicity as TCP clients see it.
//!
//! The store-level tests (`core_engine::store::atomicity_tests`) stop an MSET
//! inside the store. These do the same through the server: an MSET from one
//! connection runs on the unordered path and is stopped by the store's
//! test-only `write_pause` hook after writing its first key, with its stripes
//! held; another connection reads its keys meanwhile, through the borrowed
//! `MGET`/`GET` path the server uses for every read.

use super::request_path::Shared;
use core_engine::resp::Value;
use core_engine::store::write_pause;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

pub(super) const WITHIN: Duration = Duration::from_secs(20);

pub(super) struct Pipe {
    io: DuplexStream,
    buf: Vec<u8>,
}

impl Pipe {
    /// [`tcp`](Self::tcp), with the connection's task handle, for a test
    /// that cancels it.
    pub(super) fn tcp_task(shared: &Shared) -> (Pipe, tokio::task::JoinHandle<()>) {
        let (client, server) = tokio::io::duplex(64 * 1024);
        let task = tokio::spawn(shared.serve(server));
        (
            Pipe {
                io: client,
                buf: Vec::new(),
            },
            task,
        )
    }

    pub(super) fn tcp(shared: &Shared) -> Pipe {
        let (client, server) = tokio::io::duplex(64 * 1024);
        tokio::spawn(shared.serve(server));
        Pipe {
            io: client,
            buf: Vec::new(),
        }
    }

    pub(super) async fn send(&mut self, args: &[&str]) {
        let mut req = format!("*{}\r\n", args.len());
        for a in args {
            req.push_str(&format!("${}\r\n{a}\r\n", a.len()));
        }
        self.io.write_all(req.as_bytes()).await.expect("write");
    }

    /// The next reply. Cancel-safe: bytes read so far stay in `buf`.
    pub(super) async fn reply(&mut self) -> Value {
        loop {
            if let Ok((v, n)) = Value::parse(&self.buf) {
                self.buf.drain(..n);
                return v;
            }
            let mut chunk = [0u8; 4096];
            let n = self.io.read(&mut chunk).await.expect("read");
            assert!(n > 0, "connection closed");
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    pub(super) async fn cmd(&mut self, args: &[&str]) -> Value {
        self.send(args).await;
        tokio::time::timeout(WITHIN, self.reply())
            .await
            .expect("a reply")
    }

    /// The reply to a command already sent, if it arrives before a reader of
    /// the stopped write is counted as waiting for it; `None` once one is.
    async fn reply_or_blocked(&mut self, pause: &write_pause::Pause) -> Option<Value> {
        let deadline = Instant::now() + WITHIN;
        while Instant::now() < deadline {
            if pause.readers_blocked() > 0 {
                return None;
            }
            if let Ok(reply) = tokio::time::timeout(Duration::from_millis(2), self.reply()).await {
                return Some(reply);
            }
        }
        panic!("the reader neither answered nor waited for the write");
    }
}

pub(super) fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

/// Block the calling thread until the stopped write is reached. The test
/// body runs on the runtime's `block_on` thread, not a worker, so the
/// connections keep being served meanwhile.
fn reached(pause: &write_pause::Pause) {
    assert!(
        pause.wait_reached(WITHIN),
        "the MSET never reached its stop"
    );
}

/// Test A over TCP: MGET while another connection's MSET is stopped after
/// its first key.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tcp_mget_never_sees_part_of_an_mset() {
    let shared = Shared::new(false);
    let (mut writer, mut reader) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let (a, b) = ("tcpatom:mget:a", "tcpatom:mget:b");
    assert_eq!(
        writer.cmd(&["MSET", a, "old", b, "old"]).await,
        Value::SimpleString("OK".into())
    );
    let pause = write_pause::after_mutation_of(&shared.store, a, &[a, b]);
    writer.send(&["MSET", a, "new", b, "new"]).await;
    reached(&pause);
    reader.send(&["MGET", a, b]).await;
    let during = reader.reply_or_blocked(&pause).await;
    drop(pause);
    assert_eq!(
        tokio::time::timeout(WITHIN, writer.reply())
            .await
            .expect("MSET reply"),
        Value::SimpleString("OK".into())
    );
    let answer = match during.clone() {
        Some(answer) => answer,
        None => tokio::time::timeout(WITHIN, reader.reply())
            .await
            .expect("MGET reply"),
    };
    let old = Value::Array(Some(vec![bulk("old"), bulk("old")]));
    let new = Value::Array(Some(vec![bulk("new"), bulk("new")]));
    assert!(
        answer == old || answer == new,
        "MGET saw part of an MSET: {answer:?} (answered while it was stopped: {})",
        during.is_some()
    );
}

/// Test B over TCP: `GET a` answered new, then `GET b` — sent only after
/// that reply arrived — must not answer old.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tcp_gets_after_seeing_an_mset_key_see_the_rest() {
    let shared = Shared::new(false);
    let (mut writer, mut reader) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let (a, b) = ("tcpatom:seq:a", "tcpatom:seq:b");
    writer.cmd(&["MSET", a, "old", b, "old"]).await;
    let pause = write_pause::after_mutation_of(&shared.store, a, &[a, b]);
    writer.send(&["MSET", a, "new", b, "new"]).await;
    reached(&pause);
    reader.send(&["GET", a]).await;
    let first = reader.reply_or_blocked(&pause).await;
    // GET a answered during the stop: GET b is sent only after that reply.
    let sent_second = first.is_some();
    let second = if sent_second {
        reader.send(&["GET", b]).await;
        reader.reply_or_blocked(&pause).await
    } else {
        None
    };
    drop(pause);
    writer.reply().await;
    let first = match first {
        Some(v) => v,
        None => reader.reply().await,
    };
    let second = match second {
        Some(v) => v,
        None if sent_second => reader.reply().await,
        None => reader.cmd(&["GET", b]).await,
    };
    assert!(
        !(first == bulk("new") && second == bulk("old")),
        "GET {a} answered new and then GET {b}, sent after that reply, answered old: \
         no single point in time had that state"
    );
}

/// The reply to a command already sent on `pipe`, or the one it is waiting
/// for once the stopped write is released.
async fn answer(pipe: &mut Pipe, during: Option<Value>) -> Value {
    match during {
        Some(v) => v,
        None => tokio::time::timeout(WITHIN, pipe.reply())
            .await
            .expect("a reply"),
    }
}

fn pair(a: &str, b: &str) -> Value {
    Value::Array(Some(vec![bulk(a), bulk(b)]))
}

/// Test E: an MSET queued in MULTI/EXEC runs under the transaction's guards
/// and is just as atomic to another connection's MGET.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_mset_inside_exec_is_atomic_to_readers() {
    let shared = Shared::new(false);
    let (mut writer, mut reader) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let (a, b) = ("tcpatom:exec:a", "tcpatom:exec:b");
    writer.cmd(&["MSET", a, "old", b, "old"]).await;
    assert_eq!(
        writer.cmd(&["MULTI"]).await,
        Value::SimpleString("OK".into())
    );
    assert_eq!(
        writer.cmd(&["MSET", a, "new", b, "new"]).await,
        Value::SimpleString("QUEUED".into())
    );
    let pause = write_pause::after_mutation_of(&shared.store, a, &[a, b]);
    writer.send(&["EXEC"]).await;
    reached(&pause);
    reader.send(&["MGET", a, b]).await;
    let during = reader.reply_or_blocked(&pause).await;
    drop(pause);
    assert_eq!(
        tokio::time::timeout(WITHIN, writer.reply())
            .await
            .expect("EXEC reply"),
        Value::Array(Some(vec![Value::SimpleString("OK".into())]))
    );
    let seen = answer(&mut reader, during).await;
    assert!(
        seen == pair("old", "old") || seen == pair("new", "new"),
        "{seen:?}"
    );
}

/// Test E: WATCH still sees an MSET of a watched key, and EXEC aborts.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn watch_still_aborts_on_an_mset_of_a_watched_key() {
    let shared = Shared::new(false);
    let (mut tx, mut other) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let (a, b) = ("tcpatom:watch:a", "tcpatom:watch:b");
    tx.cmd(&["SET", a, "1"]).await;
    assert_eq!(
        tx.cmd(&["WATCH", a]).await,
        Value::SimpleString("OK".into())
    );
    other.cmd(&["MSET", b, "x", a, "x"]).await;
    tx.cmd(&["MULTI"]).await;
    tx.cmd(&["SET", a, "mine"]).await;
    assert_eq!(tx.cmd(&["EXEC"]).await, Value::Array(None));
    assert_eq!(tx.cmd(&["GET", a]).await, bulk("x"));
}

/// Test G: the all-write barrier (snapshots, QSUB's initial state, a
/// replica's full-sync cut) taken while an MSET is stopped waits for it, so
/// the snapshot it guards has all of the MSET or none of it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_snapshot_cut_during_an_mset_has_all_of_it() {
    let shared = Shared::new(false);
    let mut writer = Pipe::tcp(&shared);
    let (a, b) = ("tcpatom:snap:a", "tcpatom:snap:b");
    writer.cmd(&["MSET", a, "old", b, "old"]).await;
    let pause = write_pause::after_mutation_of(&shared.store, a, &[a, b]);
    writer.send(&["MSET", a, "new", b, "new"]).await;
    reached(&pause);
    let (state, store) = (Arc::clone(&shared.state), Arc::clone(&shared.store));
    let cut = tokio::spawn(async move {
        let _barrier = state.replicas.lock_all_writes().await;
        store
            .snapshot()
            .into_iter()
            .filter(|e| e.key.starts_with("tcpatom:snap:"))
            .map(|e| match e.value {
                core_engine::store::SnapshotValue::Str(v) => (e.key, v.as_slice().to_vec()),
                _ => panic!("not a string"),
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    });
    // Release the write only once the cut is waiting at the gate for it.
    let deadline = Instant::now() + WITHIN;
    while !shared.state.replicas.unordered.is_closed() {
        assert!(Instant::now() < deadline, "the cut never reached the gate");
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert!(
        !cut.is_finished(),
        "the cut completed while the MSET was stopped"
    );
    drop(pause);
    writer.reply().await;
    let snapshot = tokio::time::timeout(WITHIN, cut)
        .await
        .expect("cut")
        .unwrap();
    let values: Vec<&[u8]> = snapshot.values().map(Vec::as_slice).collect();
    assert!(
        values == [b"old".as_slice(), b"old"] || values == [b"new".as_slice(), b"new"],
        "snapshot cut part of an MSET: {snapshot:?}"
    );
}

/// Test H: a client that disconnects while its MGET waits for a stopped
/// MSET leaves nothing held: once the write is released, other clients read
/// and write those keys as usual.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reader_disconnecting_while_it_waits_releases_everything() {
    let shared = Shared::new(false);
    let (mut writer, mut reader) = (Pipe::tcp(&shared), Pipe::tcp(&shared));
    let (a, b) = ("tcpatom:cancel:a", "tcpatom:cancel:b");
    writer.cmd(&["MSET", a, "old", b, "old"]).await;
    let pause = write_pause::after_mutation_of(&shared.store, a, &[a, b]);
    writer.send(&["MSET", a, "new", b, "new"]).await;
    reached(&pause);
    reader.send(&["MGET", a, b]).await;
    assert!(
        reader.reply_or_blocked(&pause).await.is_none(),
        "MGET answered during the stop instead of waiting"
    );
    drop(reader);
    drop(pause);
    writer.reply().await;
    let mut fresh = Pipe::tcp(&shared);
    assert_eq!(fresh.cmd(&["MGET", a, b]).await, pair("new", "new"));
    assert_eq!(
        fresh.cmd(&["MSET", a, "z", b, "z"]).await,
        Value::SimpleString("OK".into())
    );
    assert_eq!(fresh.cmd(&["MGET", a, b]).await, pair("z", "z"));
    assert_eq!(writer.cmd(&["GET", b]).await, bulk("z"));
}
