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
        // Under `always` the first batch is drained on the blocking pool
        // while the rest queue, so the split depends on when that thread
        // runs; a few writes either way, against 50.
        let bound = if policy == AofSync::Always { 6 } else { 3 };
        assert!(
            coalesced <= bound,
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
                if occurrences(
                    &std::fs::read(&path).unwrap(),
                    format!("{value}\r\n").as_bytes(),
                ) == 0
                {
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

// ── 6N-C4: what counts as a coalescing gain ───────────────────────────────

fn relaxed(counter: &std::sync::atomic::AtomicU64) -> u64 {
    counter.load(std::sync::atomic::Ordering::Relaxed)
}

/// Frames in batches of two or more: an upper bound on yields that can
/// honestly count as gains, since a gain means another frame joined the
/// yielder's batch (bucket 0 of the histogram holds one-frame batches).
fn frames_in_shared_batches(stats: &AofStats) -> u64 {
    relaxed(&stats.frames) - relaxed(&stats.batch_hist[0])
}

/// The (key, value) of each `SET` frame in `bytes`, in order.
fn set_frames(mut bytes: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    while !bytes.is_empty() {
        let (value, used) = Value::parse(bytes).expect("whole RESP frames");
        bytes = &bytes[used..];
        // Write frames are encoded as RESP3 pushes (`>`).
        let parts = match value {
            Value::Array(Some(parts)) | Value::Push(parts) => parts,
            other => panic!("not a command frame: {other:?}"),
        };
        let text = |v: &Value| match v {
            Value::BulkString(Some(b)) => String::from_utf8(b.clone()).unwrap(),
            other => panic!("not a bulk string: {other:?}"),
        };
        if text(&parts[0]).eq_ignore_ascii_case("SET") {
            out.push((text(&parts[1]), text(&parts[2])));
        }
    }
    out
}

/// Test A. Writer A queues its frame and yields; another writer writes A's
/// batch without adding a frame (here a QSUB-style `tail().settled()`); A
/// resumes. Nothing joined A's batch, so the yield is a **miss**: the 6N-C
/// heuristic (`done.is_complete() || more holders`) counted it a gain
/// because the batch had completed, which is how P=16 never backed off.
/// After `COALESCE_MISSES` such yields the path backs off and the next
/// append leads at once.
#[tokio::test(flavor = "current_thread")]
async fn a_batch_another_writer_wrote_with_nothing_joined_is_a_miss() {
    use std::task::Poll;
    let dir = scratch("false_gain");
    let path = dir.join("f.aof");
    let (w, stats) = AofWriter::open(path.clone(), AofSync::EverySec)
        .await
        .unwrap()
        .with_private_stats();
    w.set_fanout(AofFanout::Coalesced);
    for round in 0..COALESCE_MISSES {
        let f = frame(&format!("a{round}"));
        let fut = w.append_coalesced(&f);
        tokio::pin!(fut);
        assert!(
            futures_util::poll!(fut.as_mut()).is_pending(),
            "A queues and yields"
        );
        // Another writer performs A's batch, adding nothing to it.
        w.tail()
            .expect("A's batch is queued")
            .settled()
            .await
            .unwrap();
        assert_eq!(occurrences(&std::fs::read(&path).unwrap(), &f), 1);
        assert!(
            matches!(futures_util::poll!(fut.as_mut()), Poll::Ready(Ok(()))),
            "A resumes, finds its batch written, and returns"
        );
        let round = u64::from(round);
        assert_eq!(
            relaxed(&stats.fanout_coalesce_gained),
            0,
            "a batch that stayed at one frame is not a gain"
        );
        assert_eq!(relaxed(&stats.fanout_coalesce_missed), round + 1);
        assert_eq!(relaxed(&stats.fanout_found_written), round + 1);
    }
    assert_eq!(
        relaxed(&stats.fanout_coalesce_backoffs),
        1,
        "backed off at the miss threshold"
    );
    // Backed off: the next append does not yield, so it completes on its
    // first poll (`everysec` writes inline).
    let f = frame("after");
    let fut = w.append_coalesced(&f);
    tokio::pin!(fut);
    assert!(
        matches!(futures_util::poll!(fut.as_mut()), Poll::Ready(Ok(()))),
        "a backed-off append leads at once"
    );
    assert_eq!(relaxed(&stats.batches), u64::from(COALESCE_MISSES) + 1);
    assert_eq!(relaxed(&stats.frames), u64::from(COALESCE_MISSES) + 1);
    // The connection gate's counters are its own.
    assert_eq!(
        relaxed(&stats.coalesce_gained) + relaxed(&stats.coalesce_missed),
        0
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test B. Ten writers queue into one open batch before any resumes. The
/// first to resume writes it once for all; the other nine find it written
/// and do not lead again. Each yield that another frame followed is a gain
/// (nine); the last writer's yield, which nothing followed, is a miss.
#[tokio::test(flavor = "current_thread")]
async fn frames_that_join_a_batch_are_gains_and_share_one_write() {
    use std::task::Poll;
    let dir = scratch("true_gain");
    let path = dir.join("f.aof");
    let (w, stats) = AofWriter::open(path.clone(), AofSync::EverySec)
        .await
        .unwrap()
        .with_private_stats();
    let frames: Vec<Vec<u8>> = (0..10).map(|c| frame(&format!("j{c}"))).collect();
    let mut writers: Vec<_> = frames
        .iter()
        .map(|f| Box::pin(w.append_coalesced(f)))
        .collect();
    for writer in &mut writers {
        assert!(futures_util::poll!(writer.as_mut()).is_pending());
    }
    assert_eq!(relaxed(&stats.batches), 0, "nothing written before a lead");
    for (c, writer) in writers.iter_mut().enumerate() {
        assert!(
            matches!(futures_util::poll!(writer.as_mut()), Poll::Ready(Ok(()))),
            "writer {c} completes with its batch's outcome"
        );
        assert_eq!(
            occurrences(&std::fs::read(&path).unwrap(), &frames[c]),
            1,
            "writer {c} returned with its frame in the file once"
        );
    }
    assert_eq!(relaxed(&stats.batches), 1, "one write for all ten");
    assert_eq!(relaxed(&stats.write_calls), 1);
    assert_eq!(relaxed(&stats.frames), 10);
    assert_eq!(relaxed(&stats.fanout_coalesce_gained), 9);
    assert_eq!(relaxed(&stats.fanout_coalesce_missed), 1);
    assert_eq!(relaxed(&stats.fanout_found_written), 9);
    assert_eq!(relaxed(&stats.fanout_coalesce_backoffs), 0);
    let on_disk = std::fs::read(&path).unwrap();
    for f in &frames {
        assert_eq!(occurrences(&on_disk, f), 1);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

struct Pipelines {
    batches: u64,
    gained: u64,
    missed: u64,
    backoffs: u64,
    shared_frames: u64,
}

/// `clients` tasks on one worker, each appending `depth` frames one after
/// another, as a pipelined connection's commands run. With `serialized`,
/// each append is made under one shared lock, like a writer holding every
/// write-order stripe: no other frame can join while it yields. Checks that
/// every frame lands once, in its writer's order, that no append returns
/// before its frame is in the file, and that nothing is left queued.
async fn pipelines(name: &str, clients: usize, depth: usize, serialized: bool) -> Pipelines {
    pipelines_under(AofSync::EverySec, name, clients, depth, serialized).await
}

async fn pipelines_under(
    policy: AofSync,
    name: &str,
    clients: usize,
    depth: usize,
    serialized: bool,
) -> Pipelines {
    let dir = scratch(name);
    let path = dir.join("f.aof");
    let (w, stats) = AofWriter::open(path.clone(), policy)
        .await
        .unwrap()
        .with_private_stats();
    let w = Arc::new(w);
    let lock = Arc::new(tokio::sync::Mutex::new(()));
    let barrier = Arc::new(tokio::sync::Barrier::new(clients));
    let early = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let tasks: Vec<_> = (0..clients)
        .map(|c| {
            let (w, lock, barrier, early, path) = (
                Arc::clone(&w),
                Arc::clone(&lock),
                Arc::clone(&barrier),
                Arc::clone(&early),
                path.clone(),
            );
            tokio::spawn(async move {
                barrier.wait().await;
                for i in 0..depth {
                    let f = frame(&format!("p{c}_{i}"));
                    let _held = if serialized {
                        Some(lock.lock().await)
                    } else {
                        None
                    };
                    w.append_coalesced(&f).await.unwrap();
                    if occurrences(&std::fs::read(&path).unwrap(), &f) != 1 {
                        early.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                }
            })
        })
        .collect();
    for t in tasks {
        tokio::time::timeout(Duration::from_secs(20), t)
            .await
            .expect("no pipeline may hang")
            .unwrap();
    }
    assert_eq!(early.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(w.tail().is_none(), "{name}: a batch was left queued");
    let on_disk = std::fs::read(&path).unwrap();
    let pos = |f: &[u8]| on_disk.windows(f.len()).position(|x| x == f);
    for c in 0..clients {
        let mut last = None;
        for i in 0..depth {
            let f = frame(&format!("p{c}_{i}"));
            assert_eq!(occurrences(&on_disk, &f), 1, "{name}: p{c}_{i}");
            let at = pos(&f);
            assert!(at > last, "{name}: writer {c}'s frames out of order");
            last = at;
        }
    }
    assert_eq!(
        relaxed(&stats.frames),
        u64::try_from(clients * depth).unwrap()
    );
    let _ = std::fs::remove_dir_all(&dir);
    Pipelines {
        batches: relaxed(&stats.batches),
        gained: relaxed(&stats.fanout_coalesce_gained),
        missed: relaxed(&stats.fanout_coalesce_missed),
        backoffs: relaxed(&stats.fanout_coalesce_backoffs),
        shared_frames: frames_in_shared_batches(stats),
    }
}

/// Test C. Fifty pipelines of sixteen on one worker (M1's P=16 shape).
///
/// Independent pipelines must really share writes. In 6N-C each writer that
/// resumed to find its batch written led anyway, writing the batch the
/// pipeline before it had just opened with one frame: about 1.2 frames per
/// write, every yield still judged a gain. Now batches hold many frames,
/// and no yield is counted a gain unless its batch really grew.
///
/// Pipelines that cannot overlap (each append under one shared lock, like a
/// writer holding every stripe) gain nothing from a yield: every yield is
/// a miss, and the back-off keeps them to a few percent of appends.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pipelined_writers_share_writes_and_count_only_real_gains() {
    let total = 50 * 16;
    let run = pipelines("pipelined", 50, 16, false).await;
    assert!(
        run.batches * 8 <= total,
        "{} writes for {total} pipelined frames: under 8 frames per write",
        run.batches
    );
    assert!(
        run.gained <= run.shared_frames,
        "{} gains but only {} frames in shared batches",
        run.gained,
        run.shared_frames
    );

    let run = pipelines("serialized", 50, 16, true).await;
    assert_eq!(run.gained, 0, "nothing can join a serialized append");
    assert!(run.backoffs >= 1, "ineffective yields never backed off");
    let yields = run.gained + run.missed;
    assert!(
        yields * 10 <= total,
        "{yields} futile yields for {total} appends: the back-off did not hold"
    );
}

/// Test D. One writer alone: every yield is a miss, the back-off engages
/// after each pair, and every write completes with no other traffic.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_single_coalesced_writer_backs_off() {
    let run = pipelines("single", 1, 340, false).await;
    // Two misses, then 32 straight leads: 340 appends → 20 probes.
    assert_eq!(run.gained + run.missed, 20, "probes with one writer");
    assert_eq!(run.gained, 0);
    assert_eq!(run.backoffs, 10);
    assert_eq!(run.batches, 340);
}

/// Test E. A writer that finds its batch already written does not lead
/// (6N-C4), so the batch opened since waits for its own writers. If the
/// only one of those is cancelled mid-yield, its guard still writes it:
/// nothing is stranded, nothing is written twice, nobody hangs.
#[tokio::test(flavor = "current_thread")]
async fn skipping_the_lead_strands_nothing_when_the_next_writer_is_cancelled() {
    use std::task::Poll;
    for policy in [AofSync::No, AofSync::EverySec, AofSync::Always] {
        let dir = scratch("skip_cancel");
        let path = dir.join("f.aof");
        let (w, stats) = AofWriter::open(path.clone(), policy)
            .await
            .unwrap()
            .with_private_stats();
        let (fa, fc) = (frame("first"), frame("cancelled"));
        let a = w.append_coalesced(&fa);
        tokio::pin!(a);
        assert!(futures_util::poll!(a.as_mut()).is_pending());
        // Someone else writes A's batch.
        w.tail().unwrap().settled().await.unwrap();
        {
            // C opens the next batch and yields...
            let c = w.append_coalesced(&fc);
            tokio::pin!(c);
            assert!(futures_util::poll!(c.as_mut()).is_pending());
            // ...A resumes, finds its own batch written, and does not lead
            // C's.
            assert!(
                matches!(futures_util::poll!(a.as_mut()), Poll::Ready(Ok(()))),
                "{policy:?}: A completes"
            );
            assert_eq!(relaxed(&stats.fanout_found_written), 1);
            if policy != AofSync::Always {
                // (`always` drains on the blocking pool, which may pick up
                // C's batch on its own.)
                assert_eq!(
                    occurrences(&std::fs::read(&path).unwrap(), &fc),
                    0,
                    "{policy:?}: A led C's batch"
                );
            }
        } // C is cancelled here.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while occurrences(&std::fs::read(&path).unwrap(), &fc) == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "{policy:?}: the cancelled writer's frame was stranded"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let on_disk = std::fs::read(&path).unwrap();
        assert_eq!(occurrences(&on_disk, &fa), 1, "{policy:?}");
        assert_eq!(occurrences(&on_disk, &fc), 1, "{policy:?}");
        // Under `always` the frame is in the file before its batch's
        // `fdatasync` and completion, so wait for that (without leading,
        // which could hide a stranded batch) rather than check at once.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while w.tail().is_some() {
            assert!(
                std::time::Instant::now() < deadline,
                "{policy:?}: something is still queued"
            );
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Test F. Pipelined writes on a replicated primary, through the real write
/// path, in both modes: twenty clients of sixteen `SET`s each, every fourth
/// on one hot key. Each write reaches the AOF and the replica once; the
/// replica's offsets only increase; every key's values come in the same
/// order in the AOF, the replica stream and the client's own sequence; the
/// store ends on the stream's last value; and coalesced writes share
/// `write(2)`s.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pipelined_replicated_writes_keep_one_order_everywhere() {
    const CLIENTS: usize = 20;
    const DEPTH: usize = 16;
    for mode in [AofFanout::Inline, AofFanout::Coalesced] {
        let dir = scratch("repl_pipe");
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
        let barrier = Arc::new(tokio::sync::Barrier::new(CLIENTS));
        let mut tasks = Vec::new();
        for c in 0..CLIENTS {
            let (state, store, tx, watches, barrier, path) = (
                Arc::clone(&state),
                Arc::clone(&store),
                tx.clone(),
                Arc::clone(&watches),
                Arc::clone(&barrier),
                path.clone(),
            );
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                for i in 0..DEPTH {
                    // Staggered, so a quarter of the clients want the hot
                    // key at each step (in lock-step, every yield on it
                    // would be one that nothing can join).
                    let key = if (i + c) % 4 == 3 {
                        "hot".to_string()
                    } else {
                        format!("k{c}")
                    };
                    let value = format!("v{c}_{i}");
                    let cmd = Command::Set(key, value.clone().into_bytes(), Default::default());
                    let reply =
                        execute_ordered_write(&cmd, &tx, c as u64 + 1, &state, &watches, &store)
                            .await;
                    assert_eq!(reply, Value::SimpleString("OK".into()));
                    let needle = format!("\r\n{value}\r\n");
                    assert_eq!(
                        occurrences(&std::fs::read(&path).unwrap(), needle.as_bytes()),
                        1,
                        "{mode:?}: {value} returned before its AOF write"
                    );
                }
            }));
        }
        for t in tasks {
            tokio::time::timeout(Duration::from_secs(20), t)
                .await
                .expect("no write may hang")
                .unwrap();
        }
        let mut streamed = Vec::new();
        let mut offsets = Vec::new();
        while let Ok(Some(q)) =
            tokio::time::timeout(Duration::from_millis(200), replica.recv()).await
        {
            offsets.push(q.offset());
            streamed.extend_from_slice(q.bytes());
        }
        assert!(
            offsets.windows(2).all(|o| o[0] < o[1]),
            "{mode:?}: replication offsets out of order"
        );
        let in_aof = set_frames(&std::fs::read(&path).unwrap());
        let in_stream = set_frames(&streamed);
        assert_eq!(in_aof.len(), CLIENTS * DEPTH, "{mode:?}: AOF frames");
        assert_eq!(in_stream.len(), CLIENTS * DEPTH, "{mode:?}: replica frames");
        let per_key = |frames: &[(String, String)], key: &str| -> Vec<String> {
            frames
                .iter()
                .filter(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .collect()
        };
        let mut keys: Vec<String> = (0..CLIENTS).map(|c| format!("k{c}")).collect();
        keys.push("hot".into());
        for key in &keys {
            let aof_order = per_key(&in_aof, key);
            let stream_order = per_key(&in_stream, key);
            assert_eq!(
                aof_order, stream_order,
                "{mode:?}: {key} differs between AOF and replica"
            );
            if let Some(c) = key.strip_prefix('k') {
                let c: usize = c.parse().unwrap();
                let own: Vec<String> = (0..DEPTH)
                    .filter(|i| (i + c) % 4 != 3)
                    .map(|i| format!("v{c}_{i}"))
                    .collect();
                assert_eq!(aof_order, own, "{mode:?}: {key} not in its client's order");
            }
            let last = stream_order.last().unwrap();
            assert_eq!(
                store.execute(Command::Get(key.clone())),
                Value::BulkString(Some(last.as_bytes().to_vec())),
                "{mode:?}: the store disagrees with the stream on {key}"
            );
        }
        let batches = relaxed(&stats.batches);
        let total = u64::try_from(CLIENTS * DEPTH).unwrap();
        match mode {
            AofFanout::Inline => assert!(batches * 10 >= total * 9, "inline control: {batches}"),
            // A quarter of the writes contend on one key, whose holder
            // yields with nothing able to join; 6N-C made ~1.2 frames per
            // write at this depth.
            AofFanout::Coalesced => assert!(
                batches * 2 <= total,
                "coalesced: {batches} writes for {total} pipelined writes"
            ),
        }
        assert!(
            relaxed(&stats.fanout_coalesce_gained) <= frames_in_shared_batches(stats),
            "{mode:?}: gains without shared batches"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Test C on four workers, the shape a multi-worker server runs in: inline
/// leaders on several threads hand the writer role over through `busy`,
/// tasks migrate between threads across their yields, and under `always`
/// batches drain on the blocking pool. Every frame still lands once, in its
/// writer's order; no append returns before its frame is written; nothing
/// is left queued; and no yield counts as a gain without a shared batch.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pipelined_writers_on_four_workers_strand_nothing() {
    for policy in [AofSync::No, AofSync::EverySec, AofSync::Always] {
        let run = pipelines_under(policy, &format!("mw_{policy:?}"), 32, 16, false).await;
        assert!(
            run.gained <= run.shared_frames,
            "{policy:?}: {} gains, {} frames in shared batches",
            run.gained,
            run.shared_frames
        );
    }
}

/// A flush that seals the batch while its writer yields adds no frame: the
/// yield is a miss, and the writer finds its batch written.
#[tokio::test(flavor = "current_thread")]
async fn a_batch_sealed_by_a_flush_is_a_miss() {
    use std::task::Poll;
    let dir = scratch("sealed");
    let path = dir.join("f.aof");
    let (w, stats) = AofWriter::open(path.clone(), AofSync::EverySec)
        .await
        .unwrap()
        .with_private_stats();
    let f = frame("sealed");
    let a = w.append_coalesced(&f);
    tokio::pin!(a);
    assert!(futures_util::poll!(a.as_mut()).is_pending());
    w.flush().await.unwrap();
    // Joins a fresh batch after the sealed one, not A's.
    let g = frame("later");
    w.append(&g).await.unwrap();
    assert!(matches!(
        futures_util::poll!(a.as_mut()),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(relaxed(&stats.fanout_coalesce_gained), 0);
    assert_eq!(relaxed(&stats.fanout_coalesce_missed), 1);
    assert_eq!(relaxed(&stats.fanout_found_written), 1);
    let on_disk = std::fs::read(&path).unwrap();
    assert_eq!(occurrences(&on_disk, &f), 1);
    assert_eq!(occurrences(&on_disk, &g), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 6N-C5 (6N-C4 review F1): a writer that finds another performer writing
/// does not yield. That performer — here held mid-batch, as another worker's
/// inline leader or the `always` drain would be — sweeps the batch when it is
/// done, so a yield could only delay the writer, and its outcome would teach
/// the back-off nothing. The writer still leads (a no-op while busy) and
/// waits for its batch exactly as before.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_writer_does_not_yield_while_a_performer_is_busy() {
    let dir = scratch("busy_skip");
    let path = dir.join("f.aof");
    let (w, stats) = AofWriter::open(path.clone(), AofSync::EverySec)
        .await
        .unwrap()
        .with_private_stats();
    let w = Arc::new(w);
    let held = frame("held");
    w.hold_batch_containing(b"held");
    let first = {
        let w = Arc::clone(&w);
        let held = held.clone();
        tokio::spawn(async move { w.append_coalesced(&held).await })
    };
    w.wait_until_held().await;
    let (gained, missed, skips) = (
        relaxed(&stats.fanout_coalesce_gained),
        relaxed(&stats.fanout_coalesce_missed),
        relaxed(&stats.fanout_busy_skips),
    );
    let next = frame("next");
    let b = w.append_coalesced(&next);
    tokio::pin!(b);
    // Queued, no yield: pending only on its batch, which the held
    // performer has yet to reach.
    assert!(futures_util::poll!(b.as_mut()).is_pending());
    assert_eq!(relaxed(&stats.fanout_busy_skips), skips + 1);
    assert_eq!(
        relaxed(&stats.fanout_coalesce_gained),
        gained,
        "a busy skip observed a gain"
    );
    assert_eq!(
        relaxed(&stats.fanout_coalesce_missed),
        missed,
        "a busy skip observed a miss"
    );
    w.release_hold();
    first.await.unwrap().unwrap();
    b.await.unwrap();
    let on_disk = std::fs::read(&path).unwrap();
    assert_eq!(occurrences(&on_disk, &held), 1);
    assert_eq!(occurrences(&on_disk, &next), 1);
    let at = |f: &[u8]| on_disk.windows(f.len()).position(|x| x == f).unwrap();
    assert!(
        at(&held) < at(&next),
        "the swept frame was written before the held one"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Writers that queue behind a busy performer, some cancelled while they
/// wait: every frame is swept and written once, in queue order, and the
/// queue is left empty. No writer that skipped its yield is needed to lead.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn frames_queued_behind_a_busy_performer_are_all_swept() {
    let dir = scratch("busy_sweep");
    let path = dir.join("f.aof");
    let (w, stats) = AofWriter::open(path.clone(), AofSync::EverySec)
        .await
        .unwrap()
        .with_private_stats();
    let w = Arc::new(w);
    let held = frame("held");
    w.hold_batch_containing(b"held");
    let first = {
        let w = Arc::clone(&w);
        let held = held.clone();
        tokio::spawn(async move { w.append_coalesced(&held).await })
    };
    w.wait_until_held().await;
    let skips = relaxed(&stats.fanout_busy_skips);
    let frames: Vec<Vec<u8>> = (0..8).map(|i| frame(&format!("queued{i}"))).collect();
    let mut kept = Vec::new();
    for (i, f) in frames.iter().enumerate() {
        let mut append = Box::pin(w.append_coalesced(f));
        assert!(futures_util::poll!(append.as_mut()).is_pending());
        if i % 2 == 0 {
            kept.push(append);
        }
        // Odd writers are cancelled here, their frames already queued.
    }
    assert_eq!(relaxed(&stats.fanout_busy_skips), skips + 8);
    w.release_hold();
    first.await.unwrap().unwrap();
    for append in kept {
        tokio::time::timeout(std::time::Duration::from_secs(10), append)
            .await
            .expect("a writer queued behind the performer was stranded")
            .unwrap();
    }
    // Everything queued has been performed: a fresh append completes too.
    let after = frame("after");
    w.append(&after).await.unwrap();
    assert!(w.tail().is_none(), "frames left queued");
    let on_disk = std::fs::read(&path).unwrap();
    let mut last = 0;
    for f in std::iter::once(&held)
        .chain(frames.iter())
        .chain(std::iter::once(&after))
    {
        assert_eq!(
            occurrences(&on_disk, f),
            1,
            "{}",
            String::from_utf8_lossy(f)
        );
        let at = on_disk
            .windows(f.len())
            .position(|x| x == f.as_slice())
            .unwrap();
        assert!(at >= last, "out of queue order");
        last = at;
    }
    let _ = std::fs::remove_dir_all(&dir);
}
