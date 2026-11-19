//! 6N-C Option D: `RECACHED_AOF_FANOUT=coalesced`.
//!
//! A write that waits for its own AOF stage before fanning out — any write
//! on a primary with replication, among others — led its own batch at once,
//! so 50 ready replicated clients made 50 `write(2)`s (6N-1 Test B). In
//! `coalesced` mode it queues its frame, yields once so other ready writers
//! join the open batch, then leads and waits. These tests check that this
//! changes how many writes share a `write(2)`, and nothing else: replies
//! still follow their own AOF stage, replicas still receive every write
//! after it, conflicting writes keep one order everywhere, failures keep
//! their contracts, and a cancelled writer strands nothing.

use super::*;
use std::time::Duration;

fn frame(tag: &str) -> Vec<u8> {
    format!("*2\r\n$3\r\nTAG\r\n${}\r\n{}\r\n", tag.len(), tag).into_bytes()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "recached_fanout_{name}_{}_{}",
        std::process::id(),
        next_conn_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn occurrences(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len()).filter(|w| *w == needle).count()
}

#[test]
fn the_mode_parses_strictly_and_defaults_to_inline() {
    assert_eq!(AofFanout::parse(""), Ok(AofFanout::Inline));
    assert_eq!(AofFanout::parse("inline"), Ok(AofFanout::Inline));
    assert_eq!(AofFanout::parse("COALESCED"), Ok(AofFanout::Coalesced));
    for bad in ["watermark", "on", "1", "coalesce", " inline"] {
        assert!(AofFanout::parse(bad).is_err(), "{bad:?} accepted");
    }
}

/// Fifty appenders ready together on one worker, each holding nothing else:
/// `append` leads at once (one `write(2)` each); `append_coalesced` lets
/// them share a few. Each append returns only once its frame is in the
/// file, in both modes.
async fn ready_appenders(policy: AofSync, coalesced: bool) -> (u64, u64) {
    let dir = scratch("ready");
    let path = dir.join("f.aof");
    let (w, stats) = AofWriter::open(path.clone(), policy)
        .await
        .unwrap()
        .with_private_stats();
    let w = Arc::new(w);
    let barrier = Arc::new(tokio::sync::Barrier::new(50));
    let early = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let tasks: Vec<_> = (0..50)
        .map(|c| {
            let (w, barrier, early, path) = (
                Arc::clone(&w),
                Arc::clone(&barrier),
                Arc::clone(&early),
                path.clone(),
            );
            tokio::spawn(async move {
                barrier.wait().await;
                let f = frame(&format!("w{c}"));
                if coalesced {
                    w.append_coalesced(&f).await.unwrap();
                } else {
                    w.append(&f).await.unwrap();
                }
                if occurrences(&std::fs::read(&path).unwrap(), &f) != 1 {
                    early.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            })
        })
        .collect();
    for t in tasks {
        tokio::time::timeout(Duration::from_secs(10), t)
            .await
            .expect("no append may hang")
            .unwrap();
    }
    let on_disk = std::fs::read(&path).unwrap();
    for c in 0..50 {
        assert_eq!(occurrences(&on_disk, &frame(&format!("w{c}"))), 1);
    }
    let _ = std::fs::remove_dir_all(&dir);
    (
        stats.batches.load(std::sync::atomic::Ordering::Relaxed),
        early.load(std::sync::atomic::Ordering::SeqCst),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn fifty_ready_writers_share_writes_only_when_coalesced() {
    for policy in [AofSync::No, AofSync::EverySec, AofSync::Always] {
        let (inline, early) = ready_appenders(policy, false).await;
        assert_eq!(early, 0);
        let (coalesced, early) = ready_appenders(policy, true).await;
        assert_eq!(early, 0, "{policy:?}: an append returned before its write");
        assert!(
            coalesced <= 3,
            "{policy:?}: 50 ready coalesced writers took {coalesced} writes"
        );
        if policy != AofSync::Always {
            // Under `always` a batch runs on the blocking pool, so others
            // queue behind it even inline; the control is the inline drains.
            assert_eq!(inline, 50, "{policy:?}: the control, one write each");
        }
    }
}

/// A coalesced append cancelled during its yield (its client went away)
/// still has its frame written, with no other traffic to lead it.
#[tokio::test(flavor = "current_thread")]
async fn a_writer_cancelled_mid_yield_strands_nothing() {
    for policy in [AofSync::No, AofSync::EverySec, AofSync::Always] {
        let dir = scratch("cancel");
        let path = dir.join("f.aof");
        let w = AofWriter::open(path.clone(), policy).await.unwrap();
        let f = frame("cancelled");
        {
            let fut = w.append_coalesced(&f);
            tokio::pin!(fut);
            // One poll: queued, yielding.
            let polled = futures_util::poll!(fut.as_mut());
            assert!(polled.is_pending(), "{policy:?}: did not yield");
        } // dropped here
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while occurrences(&std::fs::read(&path).unwrap(), &f) == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "{policy:?}: a cancelled writer's frame was never written"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Under `always` a failed `fdatasync` fails every coalesced writer of the
/// batch and stops the writer (in-process tests latch instead of exiting);
/// under `everysec` a failed write is reported to every writer of the
/// batch and the writer goes on.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_failed_batch_fails_every_coalesced_writer_with_its_contract() {
    for (policy, fault) in [
        (
            AofSync::Always,
            AofFault::FsyncFails(std::io::ErrorKind::Other),
        ),
        (AofSync::EverySec, AofFault::Fail(std::io::ErrorKind::Other)),
    ] {
        let dir = scratch("fail");
        let w = Arc::new(AofWriter::open(dir.join("f.aof"), policy).await.unwrap());
        w.inject_fault(fault);
        let barrier = Arc::new(tokio::sync::Barrier::new(10));
        let tasks: Vec<_> = (0..10)
            .map(|c| {
                let (w, barrier) = (Arc::clone(&w), Arc::clone(&barrier));
                tokio::spawn(async move {
                    barrier.wait().await;
                    w.append_coalesced(&frame(&format!("f{c}"))).await
                })
            })
            .collect();
        let mut failed = 0;
        for t in tasks {
            if tokio::time::timeout(Duration::from_secs(10), t)
                .await
                .expect("no append may hang")
                .unwrap()
                .is_err()
            {
                failed += 1;
            }
        }
        assert!(failed >= 1, "{policy:?}: the failure reached nobody");
        let after = w.append_coalesced(&frame("after")).await;
        match policy {
            AofSync::Always => {
                assert_eq!(failed, 10, "always: every writer after the failure fails");
                assert!(after.is_err(), "always: the writer stopped");
            }
            _ => assert!(after.is_ok(), "everysec: the writer goes on"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// End to end on a replicated primary: concurrent writes through the real
/// write path share AOF writes in `coalesced` mode; every write reaches
/// the AOF and the replica stream once; writes to one key keep one order in
/// the AOF, the replica stream and the store; and no command returns before
/// its frame is in the file.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn replicated_writes_coalesce_and_keep_their_order() {
    for mode in [AofFanout::Inline, AofFanout::Coalesced] {
        let dir = scratch("repl");
        let path = dir.join("append.aof");
        let (w, stats) = AofWriter::open(path.clone(), AofSync::EverySec)
            .await
            .unwrap()
            .with_private_stats();
        w.set_fanout(mode);
        let aof = Arc::new(w);
        let hub = ReplHub::new();
        hub.configure_backlog(16 << 20);
        let mut replica = hub.attach_test_replica(1 << 14, usize::MAX >> 1).await;
        let state = ServerState::for_tests_with_aof(
            Arc::new(SnapshotConfig {
                path: dir.join("dump.rdb"),
                last_save: AtomicI64::new(0),
                checkpoint_id: AtomicU64::new(0),
            }),
            hub,
            false,
            Some(Arc::clone(&aof)),
        );
        let store = Arc::new(KeyValueStore::new());
        let (tx, watches) = (SyncRouter::new(16), WatchHub::new());
        let barrier = Arc::new(tokio::sync::Barrier::new(52));
        let early = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let mut tasks = Vec::new();
        // 50 independent keys, plus two writers racing on one hot key.
        for c in 0..52usize {
            let (state, store, tx, watches, barrier, early, path) = (
                Arc::clone(&state),
                Arc::clone(&store),
                tx.clone(),
                Arc::clone(&watches),
                Arc::clone(&barrier),
                Arc::clone(&early),
                path.clone(),
            );
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                let (key, value) = if c < 50 {
                    (format!("k{c}"), format!("v{c}"))
                } else {
                    ("hot".to_string(), format!("hot{c}"))
                };
                let cmd = Command::Set(key, value.clone().into_bytes(), Default::default());
                let reply =
                    execute_ordered_write(&cmd, &tx, c as u64 + 1, &state, &watches, &store).await;
                assert_eq!(reply, Value::SimpleString("OK".into()));
                if occurrences(&std::fs::read(&path).unwrap(), value.as_bytes()) == 0 {
                    early.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            }));
        }
        for t in tasks {
            tokio::time::timeout(Duration::from_secs(10), t)
                .await
                .expect("no write may hang")
                .unwrap();
        }
        assert_eq!(
            early.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "{mode:?}: a write returned before its AOF write"
        );
        let mut streamed = Vec::new();
        while let Ok(Some(q)) =
            tokio::time::timeout(Duration::from_millis(200), replica.recv()).await
        {
            streamed.push(q.bytes().to_vec());
        }
        assert_eq!(
            streamed.len(),
            52,
            "{mode:?}: every write reached the replica once"
        );
        let on_disk = std::fs::read(&path).unwrap();
        for c in 0..50 {
            assert_eq!(occurrences(&on_disk, format!("v{c}\r\n").as_bytes()), 1);
        }
        // The hot key: the AOF order, the replica order and the store's
        // final value agree.
        let pos = |hay: &[u8], tag: &str| {
            hay.windows(tag.len())
                .position(|w| w == tag.as_bytes())
                .unwrap()
        };
        let aof_first = if pos(&on_disk, "hot50") < pos(&on_disk, "hot51") {
            "hot50"
        } else {
            "hot51"
        };
        let repl_order: Vec<&str> = streamed
            .iter()
            .filter_map(|f| {
                ["hot50", "hot51"]
                    .into_iter()
                    .find(|t| f.windows(t.len()).any(|w| w == t.as_bytes()))
            })
            .collect();
        assert_eq!(repl_order.len(), 2);
        assert_eq!(
            repl_order[0], aof_first,
            "{mode:?}: AOF and replica disagree on the hot key"
        );
        let last = repl_order[1];
        assert_eq!(
            store.execute(Command::Get("hot".into())),
            Value::BulkString(Some(last.as_bytes().to_vec())),
            "{mode:?}: the store disagrees with the stream"
        );
        let batches = stats.batches.load(std::sync::atomic::Ordering::Relaxed);
        match mode {
            AofFanout::Inline => assert!(batches >= 45, "inline control: {batches} writes"),
            AofFanout::Coalesced => assert!(batches <= 6, "coalesced: {batches} writes for 52"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
