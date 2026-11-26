//! 6O-H4: replicas of a primary that logs its expirations
//! (`caps=expirylog1`, design §8).
//!
//! - Negotiated like `dedup1`; a primary without it gets today's replica.
//! - A capable replica applies frames, and a full-sync snapshot, without
//!   evaluating expiry, runs no sweeper of its own, and masks expired keys
//!   on reads. Promotion turns its sweeper back on, now logged.
//! - A run id resumes partially only in the mode it was applied in.
//! - A full sync checkpoints the replica's own files to the new keyspace;
//!   a failure fails closed.
//! - The wire carries pushes only, whatever a primary sends.

use super::replication_dedup::{Node, follow, follow_for, node, serve_replicas, wait_for};
use crate::*;
use core_engine::cmd::{Command, SetOptions};
use core_engine::resp::Value;
use core_engine::store::KeyValueStore;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

fn nil() -> Value {
    Value::BulkString(None)
}

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

fn set(key: &str, value: &str) -> Command {
    Command::Set(key.into(), value.as_bytes().to_vec(), SetOptions::default())
}

/// A write on `primary` through its ordered path, as a client's.
async fn write(primary: &Node, cmd: Command) -> Value {
    execute_ordered_write(
        &cmd,
        &primary.tx,
        0,
        &primary.state,
        &primary.watches,
        &primary.store,
    )
    .await
}

/// The string `primary` advertises to a replica.
async fn advertised(addr: std::net::SocketAddr) -> String {
    let mut sock = TcpStream::connect(addr).await.unwrap();
    let mut magic = [0u8; 4];
    sock.read_exact(&mut magic).await.unwrap();
    let mut len = [0u8; 2];
    sock.read_exact(&mut len).await.unwrap();
    let mut id = vec![0u8; usize::from(u16::from_le_bytes(len))];
    sock.read_exact(&mut id).await.unwrap();
    String::from_utf8(id).unwrap()
}

async fn until(what: &str, mut f: impl FnMut() -> bool) {
    for _ in 0..400 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("timed out waiting for {what}");
}

fn upstream_expirylog(node: &Node) -> bool {
    node.state
        .replicas
        .upstream_expirylog
        .load(Ordering::Acquire)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_replica_negotiates_the_expiry_log_with_a_new_primary() {
    let primary = node(false, true).await;
    let addr = serve_replicas(&primary).await;
    let id = advertised(addr).await;
    assert!(id.ends_with(";caps=dedup1,expirylog1"), "{id}");
    let replica = node(true, true).await;
    follow(&replica, addr);
    write(&primary, set("ng:a", "1")).await;
    wait_for(&replica.store, "ng:a", bulk("1")).await;
    assert!(upstream_expirylog(&replica));
    // Its own replicas are offered it too: its keyspace came from a
    // primary that logs its expirations.
    assert!(replica.state.writes_expiry_log());
    // Without DEDUP, the capability is offered alone.
    let plain = node(false, false).await;
    let id = advertised(serve_replicas(&plain).await).await;
    assert!(id.ends_with(";caps=expirylog1"), "{id}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_primary_without_the_capability_gets_todays_replica() {
    let primary = node(false, true).await;
    primary
        .state
        .replicas
        .test_old_primary
        .store(true, Ordering::Relaxed);
    let addr = serve_replicas(&primary).await;
    assert!(advertised(addr).await.ends_with(";caps=dedup1"));
    let replica = node(true, true).await;
    follow(&replica, addr);
    // Once it follows: a snapshot would carry the expired entry, which a
    // replica without the capability drops at load, as before.
    write(&primary, set("op:ready", "1")).await;
    wait_for(&replica.store, "op:ready", bulk("1")).await;
    write(&primary, set("op:k", "v")).await;
    write(&primary, Command::PExpireAt("op:k".into(), 1)).await;
    // Both frames applied: present, and expired.
    until("the expired key", || {
        replica.store.key_count() == 2
            && replica.store.execute(Command::Get("op:k".into())) == nil()
    })
    .await;
    assert!(!upstream_expirylog(&replica));
    assert!(!replica.state.writes_expiry_log());
    // Its own sweeper still removes what expired, as before.
    let removed = sweep_expired_tick(
        &replica.store,
        &replica.state,
        &replica.watches,
        &replica.tx,
    )
    .await;
    assert_eq!(removed, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_capable_replica_leaves_expiry_to_its_primary() {
    let primary = node(false, true).await;
    let addr = serve_replicas(&primary).await;
    let replica = node(true, true).await;
    follow(&replica, addr);
    write(
        &primary,
        Command::Set(
            "cr:k".into(),
            b"v".to_vec(),
            SetOptions {
                expiry: Some(core_engine::cmd::SetExpiry::Px(20)),
                ..Default::default()
            },
        ),
    )
    .await;
    until("the key", || replica.store.key_count() == 1).await;
    tokio::time::sleep(Duration::from_millis(40)).await;
    // Expired by its clock: masked on reads, kept in the store, not swept.
    assert_eq!(replica.store.execute(Command::Get("cr:k".into())), nil());
    let removed = sweep_expired_tick(
        &replica.store,
        &replica.state,
        &replica.watches,
        &replica.tx,
    )
    .await;
    assert_eq!(removed, 0, "a capable replica swept on its own");
    assert_eq!(replica.store.key_count(), 1);
    // The primary's sweep removes it, everywhere.
    sweep_expired_tick(
        &primary.store,
        &primary.state,
        &primary.watches,
        &primary.tx,
    )
    .await;
    until("the primary's DEL", || replica.store.key_count() == 0).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn promotion_gives_expiry_back_to_the_promoted_node() {
    let primary = node(false, true).await;
    let addr = serve_replicas(&primary).await;
    let replica = node(true, true).await;
    follow(&replica, addr);
    write(&primary, set("pr:k", "v")).await;
    write(&primary, Command::PExpireAt("pr:k".into(), 1)).await;
    until("the expired key", || {
        replica.store.key_count() == 1
            && replica.store.execute(Command::Get("pr:k".into())) == nil()
    })
    .await;
    assert_eq!(
        sweep_expired_tick(
            &replica.store,
            &replica.state,
            &replica.watches,
            &replica.tx
        )
        .await,
        0
    );
    replica.state.promote_to_primary(&replica.store).await;
    assert!(replica.state.writes_expiry_log());
    let sub = replica
        .state
        .replicas
        .attach_test_replica(64, 1 << 20)
        .await;
    replica.state.replicas.configure_backlog(1 << 20);
    drop(sub);
    assert_eq!(
        sweep_expired_tick(
            &replica.store,
            &replica.state,
            &replica.watches,
            &replica.tx
        )
        .await,
        1,
        "the promoted node expires keys again"
    );
    let id = advertised(serve_replicas(&replica).await).await;
    assert!(id.contains("expirylog1"), "{id}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_run_id_resumes_partially_only_in_the_mode_it_was_applied_in() {
    let primary = node(false, true).await;
    let addr = serve_replicas(&primary).await;
    let replica = node(true, true).await;
    let mut resume = ReplResume::default();
    // Applied with expiry evaluated (the primary hid the capability).
    primary
        .state
        .replicas
        .test_old_primary
        .store(true, Ordering::Relaxed);
    write(&primary, set("md:a", "1")).await;
    follow_for(&replica, addr, &mut resume, 300).await;
    let (_, full) = primary.state.replicas.syncs();
    // Same mode, same run id: partial.
    follow_for(&replica, addr, &mut resume, 300).await;
    assert_eq!(
        primary.state.replicas.syncs(),
        (1, full),
        "a partial resync"
    );
    // The same run id now offered in the other mode: full.
    primary
        .state
        .replicas
        .test_old_primary
        .store(false, Ordering::Relaxed);
    follow_for(&replica, addr, &mut resume, 300).await;
    assert_eq!(primary.state.replicas.syncs().1, full + 1, "a full resync");
    assert!(upstream_expirylog(&replica));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_replica_relays_the_capability_down_its_chain() {
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    let sub = node(true, true).await;
    follow(&replica, serve_replicas(&primary).await);
    write(&primary, set("ch:a", "1")).await;
    wait_for(&replica.store, "ch:a", bulk("1")).await;
    let raddr = serve_replicas(&replica).await;
    assert!(advertised(raddr).await.contains("expirylog1"));
    follow(&sub, raddr);
    write(&primary, set("ch:b", "2")).await;
    wait_for(&sub.store, "ch:b", bulk("2")).await;
    assert!(upstream_expirylog(&sub));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_wire_carries_pushes_only() {
    // Review C, R2-1: a frame received as an array is forwarded, backlogged
    // and logged as a push (an older replica copies received bytes into its
    // AOF, where an array means "written with expirations logged").
    let replica = node(true, true).await;
    let mut downstream = replica
        .state
        .replicas
        .attach_test_replica(64, 1 << 20)
        .await;
    let mut frame = resp_push(&[b"SET", b"wp:k", b"v"]);
    frame[0] = b'*';
    crate::replication::apply_repl_frame_for_tests(
        &replica.store,
        &replica.state,
        &replica.tx,
        &replica.watches,
        &frame,
    )
    .await
    .unwrap();
    assert_eq!(
        replica.store.execute(Command::Get("wp:k".into())),
        bulk("v")
    );
    let forwarded = downstream.recv().await.unwrap();
    assert_eq!(forwarded.bytes()[0], b'>');
}

// ── The replica's own files ─────────────────────────────────────────────

struct Files(PathBuf);

impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A replica node whose state has an AOF and a snapshot in `dir`.
async fn replica_with_files(dir: &Files) -> Node {
    let base = node(true, true).await;
    let aof = AofWriter::open(dir.0.join("append.aof"), AofSync::Always)
        .await
        .unwrap();
    let replayed = replay_aof_format(&base.store, &dir.0.join("append.aof"), 0, false)
        .await
        .unwrap();
    start_expiry_log(&aof, &replayed, false).await.unwrap();
    let hub = ReplHub::new();
    hub.configure_dedup(true);
    hub.configure_backlog(1 << 20);
    let state = ServerState::for_tests_with_aof(
        Arc::new(SnapshotConfig {
            path: dir.0.join("dump.rdb"),
            last_save: AtomicI64::new(0),
            checkpoint_id: AtomicU64::new(0),
        }),
        hub,
        true,
        Some(Arc::new(aof)),
    );
    Node { state, ..base }
}

fn files(name: &str) -> Files {
    let dir = std::env::temp_dir().join(format!(
        "recached_6oh_replica_{name}_{}_{}",
        std::process::id(),
        next_conn_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Files(dir)
}

fn aof_words(dir: &Files) -> Vec<(bool, String)> {
    let bytes = std::fs::read(dir.0.join("append.aof")).unwrap_or_default();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let array = bytes[at] == b'*';
        let (value, used) = Value::parse(&bytes[at..]).unwrap();
        at += used;
        let (Value::Push(items) | Value::Array(Some(items))) = value else {
            panic!()
        };
        let words: Vec<String> = items
            .into_iter()
            .map(|i| match i {
                Value::BulkString(Some(b)) => String::from_utf8_lossy(&b).into_owned(),
                other => format!("{other:?}"),
            })
            .collect();
        out.push((array, words.join(" ")));
    }
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_full_sync_checkpoints_the_replicas_own_files() {
    // Review C, H1: the replica's AOF used to keep its old keyspace's
    // records under the new one's frames.
    for capable in [true, false] {
        let dir = files("ckpt");
        // Something stale in the replica's files from before.
        std::fs::write(
            dir.0.join("append.aof"),
            resp_push(&[b"SET", b"stale", b"1"]),
        )
        .unwrap();
        let primary = node(false, true).await;
        primary
            .state
            .replicas
            .test_old_primary
            .store(!capable, Ordering::Relaxed);
        write(&primary, set("fs:a", "1")).await;
        let replica = replica_with_files(&dir).await;
        assert_eq!(
            replica.store.execute(Command::Get("stale".into())),
            bulk("1")
        );
        follow(&replica, serve_replicas(&primary).await);
        // The full sync first (it carries fs:a), then a frame.
        wait_for(&replica.store, "fs:a", bulk("1")).await;
        write(&primary, set("fs:b", "2")).await;
        wait_for(&replica.store, "fs:b", bulk("2")).await;
        // Applied, then logged: wait for the log too.
        let want = if capable { 2 } else { 1 };
        for _ in 0..200 {
            replica.state.aof.as_ref().unwrap().flush().await.unwrap();
            if aof_words(&dir).len() >= want {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let words = aof_words(&dir);
        let format = (
            true,
            format!(
                "RECACHED-CHECKPOINT {}",
                Control::Format {
                    features: FEATURE_EXPIRY_LOG
                }
                .id()
            ),
        );
        if capable {
            assert_eq!(words, vec![format, (true, "SET fs:b 2".into())], "capable");
        } else {
            assert_eq!(words, vec![(false, "SET fs:b 2".into())], "old primary");
        }
        // Restarted from its files alone, it serves the primary's keyspace.
        let store = KeyValueStore::new();
        let loaded = load_snapshot_checkpoint(&store, &dir.0.join("dump.rdb"))
            .await
            .unwrap();
        replay_aof_format(
            &store,
            &dir.0.join("append.aof"),
            loaded.aof_checkpoint,
            true,
        )
        .await
        .unwrap();
        for (k, want) in [("stale", nil()), ("fs:a", bulk("1")), ("fs:b", bulk("2"))] {
            assert_eq!(
                store.execute(Command::Get(k.into())),
                want,
                "capable={capable} {k}"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_full_sync_checkpoint_fails_closed() {
    // Review C, R2-3: memory and files would describe different keyspaces.
    let dir = files("fail");
    let primary = node(false, true).await;
    write(&primary, set("ff:a", "1")).await;
    let replica = replica_with_files(&dir).await;
    std::fs::create_dir_all(dir.0.join("dump.rdb")).unwrap(); // unwritable
    let mut resume = ReplResume::default();
    follow_for(&replica, serve_replicas(&primary).await, &mut resume, 500).await;
    assert!(
        !replica.state.persistence_is_healthy(),
        "writes refused until SAVE"
    );
    replica.state.promote_to_primary(&replica.store).await;
    let refused = write(&replica, set("ff:x", "1")).await;
    assert!(
        matches!(refused, Value::Error(ref e) if e.starts_with("MISCONF")),
        "{refused:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_replicas_tenant_sockets_get_the_primarys_swept_dels() {
    // Review B, B-5: the replica no longer sweeps; it applies the
    // primary's multi-key DEL, and pushes it per key.
    let primary = node(false, true).await;
    let replica = node(true, true).await;
    follow(&replica, serve_replicas(&primary).await);
    let grants = [crate::sync_scopes::Grant::rw("tenant:a:*")];
    let mut a = replica
        .state
        .replicas
        .subscribe_pushes(&replica.tx, Routes::Grants(&grants))
        .await;
    let frame = resp_push(&[b"DEL", b"tenant:a:1", b"tenant:b:1", b"tenant:a:2"]);
    crate::replication::apply_repl_frame_for_tests(
        &replica.store,
        &replica.state,
        &replica.tx,
        &replica.watches,
        &frame,
    )
    .await
    .unwrap();
    let mut got = Vec::new();
    while let Ok(Ok(push)) = tokio::time::timeout(Duration::from_millis(200), a.recv()).await {
        got.push(String::from_utf8_lossy(&push.resp).into_owned());
    }
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(
        got.iter()
            .all(|p| p.contains("tenant:a:") && !p.contains("tenant:b:"))
    );
}

fn px(k: &str, v: &str, ms: u64) -> Command {
    Command::Set(
        k.into(),
        v.as_bytes().to_vec(),
        SetOptions {
            expiry: Some(core_engine::cmd::SetExpiry::Px(ms)),
            ..Default::default()
        },
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_sweep_queued_behind_a_full_sync_does_not_sweep_its_keyspace() {
    // Review D, D1: a tick that read the mode before a full sync switched it
    // waited on the barrier, then swept the new, timeless keyspace silently:
    // a later PERSIST from the primary then found nothing.
    let primary = node(false, true).await;
    let addr = serve_replicas(&primary).await;
    let replica = node(true, true).await;
    replica.store.execute(px("d1:old", "x", 1)); // something volatile to sweep
    write(&primary, px("d1:k", "v", 900)).await;
    let hold = replica.state.replicas.lock_all_writes().await;
    follow(&replica, addr);
    // Long enough for the sync to queue on the barrier first (its stripes
    // are FIFO): the tick then waits behind it.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(!upstream_expirylog(&replica));
    let (s, st, w, t) = (
        Arc::clone(&replica.store),
        Arc::clone(&replica.state),
        Arc::clone(&replica.watches),
        replica.tx.clone(),
    );
    let tick = tokio::spawn(async move { sweep_expired_tick(&s, &st, &w, &t).await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        write(&primary, Command::Persist("d1:k".into())).await,
        Value::Integer(1)
    );
    tokio::time::sleep(Duration::from_millis(450)).await;
    drop(hold);
    let removed = tick.await.unwrap();
    write(&primary, set("d1:sentinel", "1")).await;
    wait_for(&replica.store, "d1:sentinel", bulk("1")).await;
    assert!(upstream_expirylog(&replica));
    assert_eq!(
        replica.store.execute(Command::Get("d1:k".into())),
        bulk("v"),
        "the tick (removed {removed}) swept a key the primary kept"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_sub_replica_renegotiates_when_its_upstream_changes_mode_mid_handshake() {
    // Review D, D2: B advertised expirylog to C, then (before taking C on)
    // took a full sync from a primary that does not log expirations. C must
    // not stream B's new, legacy keyspace as if it were timeless.
    let b = node(true, true).await;
    b.state
        .replicas
        .upstream_expirylog
        .store(true, Ordering::Release);
    b.store.execute(set("d2:k", "v"));
    b.store.execute(Command::PExpire("d2:k".into(), 300));
    let baddr = serve_replicas(&b).await;
    let hold = b.state.replicas.lock_all_writes().await;
    let c = node(true, true).await;
    let mut resume = ReplResume::default();
    // C reads `caps=…expirylog1` and waits on B's barrier.
    let (cs, cst, ctx, cw) = (
        Arc::clone(&c.store),
        Arc::clone(&c.state),
        c.tx.clone(),
        Arc::clone(&c.watches),
    );
    let first = tokio::spawn(async move {
        let mut socket = TcpStream::connect(baddr).await.unwrap();
        let mut resume = ReplResume::default();
        sync_from_primary(&mut socket, &cs, None, &ctx, &cw, &cst, &mut resume).await
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    // B is synced from an old primary meanwhile.
    b.state
        .replicas
        .upstream_expirylog
        .store(false, Ordering::Release);
    b.state.replicas.reset_stream();
    drop(hold);
    let first = tokio::time::timeout(Duration::from_secs(5), first)
        .await
        .expect("the first handshake ends")
        .unwrap();
    assert!(first.is_err(), "the stale handshake must be dropped");
    assert_eq!(c.store.key_count(), 0, "nothing was streamed on it");
    // Reconnecting, C negotiates what B now streams: no expiry log.
    follow_for(&c, baddr, &mut resume, 400).await;
    assert!(!upstream_expirylog(&c));
    // And C converges with B on B's own (legacy) rules.
    tokio::time::sleep(Duration::from_millis(350)).await;
    follow(&c, baddr);
    tokio::time::sleep(Duration::from_millis(200)).await;
    for f in [
        resp_push(&[b"APPEND", b"d2:k", b"x"]),
        resp_push(&[b"SET", b"d2:sentinel", b"1"]),
    ] {
        crate::replication::apply_repl_frame_for_tests(&b.store, &b.state, &b.tx, &b.watches, &f)
            .await
            .unwrap();
    }
    wait_for(&c.store, "d2:sentinel", bulk("1")).await;
    assert_eq!(
        c.store.execute(Command::Get("d2:k".into())),
        b.store.execute(Command::Get("d2:k".into()))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_replica_reads_ahead_while_its_full_sync_checkpoint_runs() {
    // The primary streams right after the snapshot. A replica that stopped
    // reading for its checkpoint's fsyncs let the frames back up into the
    // primary's queue to it, which overflowed: a disconnect, another full
    // sync, another checkpoint, in a loop (6O-H native lag run).
    let dir = files("readahead");
    let primary = node(false, true).await;
    // Room for a burst, so the primary's own sender keeping up is not what
    // is measured: only the replica draining it is.
    primary.state.replicas.configure_queue_bytes(1024 * 1024);
    write(&primary, set("ra:first", "1")).await;
    let replica = replica_with_files(&dir).await;
    replica
        .state
        .replicas
        .test_checkpoint_delay_ms
        .store(5000, Ordering::Relaxed);
    follow(&replica, serve_replicas(&primary).await);
    // Installed; the checkpoint now takes 5 s, as a slow disk would.
    wait_for(&replica.store, "ra:first", bulk("1")).await;
    // Far more than the primary's queue and both sockets' buffers hold,
    // paced (64 KiB per 2 ms) so the primary's sender keeps up with the
    // writer even on a loaded host.
    let value = "v".repeat(8 * 1024);
    const N: usize = 3072;
    for i in 0..N {
        write(&primary, set(&format!("ra:{i}"), &value)).await;
        if i % 8 == 7 {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }
    let last = format!("ra:{}", N - 1);
    for _ in 0..2000 {
        if replica.store.execute(Command::Get(last.clone())) == bulk(&value) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(primary.state.replicas.queue_disconnects(), 0);
    assert_eq!(primary.state.replicas.syncs(), (0, 1));
    assert!(
        replica.store.execute(Command::Get(last.clone())) == bulk(&value),
        "the replica never caught up"
    );
    assert_eq!(replica.store.key_count(), N + 1);
}
