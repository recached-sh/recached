//! 6N-C PR 0a: a DEDUP outcome is never visible before its AOF stage.
//!
//! Finding O1 (6N-B): on the deferred TCP path a DEDUP write committed its
//! mark as soon as its frame was *queued*. A replay of the same
//! `(client, id)` on another connection was then answered `+DUP` — through
//! a gate holding no ticket for the original batch — before that batch's
//! `write(2)`, or its `fdatasync` under `always`. A failure of the batch
//! then lost a write a client had been told was applied. RV-9 widened it:
//! the WebSocket early-replay path reads marks without `dedup_order`, and a
//! permanent refusal was remembered before its frame was appended.
//!
//! Each test holds the AOF performer on the original's batch
//! ([`AofWriter::hold_batch_containing`]) and checks that nothing answers
//! from the outcome until the batch is released.

use super::*;
use std::time::Duration;

struct Node {
    store: Arc<KeyValueStore>,
    state: Arc<ServerState>,
    aof: Arc<AofWriter>,
    tx: SyncTx,
    watches: WatchRegistry,
    dir: PathBuf,
}

impl Drop for Node {
    fn drop(&mut self) {
        self.aof.release_hold();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn node(name: &str, policy: AofSync) -> Node {
    let dir = std::env::temp_dir().join(format!(
        "recached_0a_{name}_{}_{}",
        std::process::id(),
        next_conn_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let aof = Arc::new(
        AofWriter::open(dir.join("append.aof"), policy)
            .await
            .unwrap(),
    );
    let state = ServerState::for_tests_with_aof(
        Arc::new(SnapshotConfig {
            path: dir.join("dump.rdb"),
            last_save: AtomicI64::new(0),
            checkpoint_id: AtomicU64::new(0),
        }),
        ReplHub::new(),
        false,
        Some(Arc::clone(&aof)),
    );
    Node {
        store: Arc::new(KeyValueStore::new()),
        state,
        aof,
        tx: SyncRouter::new(16),
        watches: WatchHub::new(),
        dir,
    }
}

fn dedup(client: &str, id: u64, inner: Command) -> Command {
    Command::Dedup(client.to_string(), id, Box::new(inner))
}

/// Run `cmd` the way a TCP connection does: inside an AOF-gated writer's
/// deferral scope, then flush the gate, as the connection does before its
/// reply reaches the socket. The receiver gets the command's result as soon
/// as the command returns — before the gate is flushed — so a test can tell
/// a deferred AOF wait (the command returns at once) from an inline one;
/// the task's own result is the reply, once the gate let it through.
fn tcp_write(
    node: &Node,
    cmd: Command,
) -> (
    tokio::task::JoinHandle<Value>,
    tokio::sync::oneshot::Receiver<Value>,
) {
    let (store, state, tx, watches) = (
        Arc::clone(&node.store),
        Arc::clone(&node.state),
        node.tx.clone(),
        Arc::clone(&node.watches),
    );
    let (returned_tx, returned) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let gate = AofGatedWriter::new(tokio::io::sink(), Arc::clone(&state));
        let mut socket = tokio::io::BufWriter::new(gate);
        let deferral = socket.get_ref().deferral();
        let reply = with_deferred_aof(deferral, async {
            execute_ordered_write(&cmd, &tx, next_conn_id(), &state, &watches, &store).await
        })
        .await;
        let _ = returned_tx.send(reply.clone());
        tokio::io::AsyncWriteExt::write_all(&mut socket, &reply.serialize())
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::flush(&mut socket).await.unwrap();
        reply
    });
    (task, returned)
}

async fn finished_within<T>(task: &mut tokio::task::JoinHandle<T>, wait: Duration) -> Option<T> {
    match tokio::time::timeout(wait, task).await {
        Ok(result) => Some(result.unwrap()),
        Err(_) => None,
    }
}

/// The O1 counterexample: the original's batch is held before its write; a
/// replay from another connection must not be answered `+DUP` meanwhile,
/// and the mark must not be readable (the WS early-replay path reads it
/// with no lock).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_replay_is_not_answered_before_the_original_reaches_the_aof() {
    for policy in [AofSync::No, AofSync::EverySec, AofSync::Always] {
        let node = node("dup", policy).await;
        node.aof.hold_batch_containing(b"acker-0a");
        let write = dedup("acker-0a", 1, Command::Incr("ctr".into()));
        let (mut original, mut original_returned) = tcp_write(&node, write.clone());
        node.aof.wait_until_held().await;

        assert!(
            node.state.dedup_replay("acker-0a", 1).is_none(),
            "{policy:?}: the mark is visible before its frame was written"
        );
        let (mut replay, _) = tcp_write(&node, write.clone());
        assert!(
            finished_within(&mut replay, Duration::from_millis(300))
                .await
                .is_none(),
            "{policy:?}: a replay was answered while the original's batch was unwritten"
        );
        assert!(
            finished_within(&mut original, Duration::from_millis(50))
                .await
                .is_none(),
            "{policy:?}: the original was answered before its batch was written"
        );
        assert!(
            original_returned.try_recv().is_err(),
            "{policy:?}: the DEDUP write took the deferred path"
        );

        node.aof.release_hold();
        assert_eq!(original.await.unwrap(), Value::Integer(1), "{policy:?}");
        assert_eq!(
            replay.await.unwrap(),
            Value::SimpleString("DUP".into()),
            "{policy:?}"
        );
        let on_disk = std::fs::read(node.dir.join("append.aof")).unwrap();
        assert!(
            on_disk.windows(8).any(|w| w == b"acker-0a"),
            "{policy:?}: the frame never reached the file"
        );
        assert_eq!(
            node.store.execute(Command::Get("ctr".into())),
            Value::BulkString(Some(b"1".to_vec())),
            "{policy:?}: applied once"
        );
    }
}

/// Whether the duplicate-suppression table remembers `id` as refused.
/// Read directly: `dedup_replay` answers a refusal only at or below the
/// client's mark, and the WebSocket replay path reads the same table.
fn remembers_refusal(node: &Node, client: &str, id: u64) -> bool {
    node.state
        .dedup_table()
        .into_iter()
        .any(|(c, _, refused)| c == client && refused.iter().any(|(r, _)| *r == id))
}

/// RV-9: a permanent refusal must not be answered from memory before its
/// refusal frame is in the AOF.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refusal_is_not_answered_before_its_frame_reaches_the_aof() {
    for policy in [AofSync::EverySec, AofSync::Always] {
        let node = node("refusal", policy).await;
        node.store.execute(Command::Set(
            "text".into(),
            b"not a number".to_vec(),
            Default::default(),
        ));
        node.aof.hold_batch_containing(b"refuser-0a");
        let write = dedup("refuser-0a", 7, Command::Incr("text".into()));
        let (mut original, _) = tcp_write(&node, write.clone());
        node.aof.wait_until_held().await;

        assert!(
            !remembers_refusal(&node, "refuser-0a", 7),
            "{policy:?}: the refusal is remembered before its frame was written"
        );
        assert!(
            finished_within(&mut original, Duration::from_millis(100))
                .await
                .is_none(),
            "{policy:?}: the refusal was answered before its frame was written"
        );
        node.aof.release_hold();
        let refused = original.await.unwrap();
        assert!(
            matches!(refused, Value::Error(_)),
            "{policy:?}: {refused:?}"
        );
        assert!(
            remembers_refusal(&node, "refuser-0a", 7),
            "{policy:?}: the refusal is remembered once logged"
        );
        // A later write from the client raises its mark past the refused id;
        // the replay is then refused again, from the remembered refusal.
        let (later, _) = tcp_write(&node, dedup("refuser-0a", 8, Command::Incr("n".into())));
        assert_eq!(later.await.unwrap(), Value::Integer(1));
        assert!(
            matches!(
                node.state.dedup_replay("refuser-0a", 7),
                Some(DedupReplay::Refused(_))
            ),
            "{policy:?}: the replay is refused again"
        );
        let on_disk = std::fs::read(node.dir.join("append.aof")).unwrap();
        assert!(
            on_disk.windows(10).any(|w| w == b"refuser-0a"),
            "{policy:?}: the refusal frame never reached the file"
        );
    }
}

/// DEDUP writes from other clients, and plain writes, stay correct while a
/// DEDUP batch is held, and complete once it is released; plain TCP writes
/// still defer (the fix is limited to DEDUP).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unrelated_writes_are_unaffected_and_plain_writes_still_defer() {
    let node = node("unrelated", AofSync::EverySec).await;
    // A plain write still defers: its command returns while its batch is
    // held, and only the reply waits, at the gate.
    node.aof.hold_batch_containing(b"plainkey");
    let (mut plain, returned) = tcp_write(&node, Command::Incr("plainkey".into()));
    node.aof.wait_until_held().await;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), returned)
            .await
            .expect("a plain write must still take the deferred path")
            .unwrap(),
        Value::Integer(1)
    );
    assert!(
        finished_within(&mut plain, Duration::from_millis(100))
            .await
            .is_none(),
        "the gate must still withhold the reply"
    );
    node.aof.release_hold();
    assert_eq!(plain.await.unwrap(), Value::Integer(1));

    let mut handles = Vec::new();
    for c in 0..8u64 {
        handles.push(
            tcp_write(
                &node,
                dedup(&format!("other-{c}"), 1, Command::Incr(format!("k{c}"))),
            )
            .0,
        );
    }
    for (c, handle) in handles.into_iter().enumerate() {
        assert_eq!(handle.await.unwrap(), Value::Integer(1), "client {c}");
        assert!(matches!(
            node.state.dedup_replay(&format!("other-{c}"), 1),
            Some(DedupReplay::Duplicate)
        ));
    }
}
