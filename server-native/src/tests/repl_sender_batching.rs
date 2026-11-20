//! 6N-C5: a replica sender holds its flush until the end of the scheduling
//! round, so one socket send carries the round's frames.
//!
//! The first `try_send` after a drain woke the sender into the worker's LIFO
//! slot, so it ran right after that one writer and flushed its frame almost
//! alone (3.6 frames per flush at P=1 in the 6N-C5 profile). Each flush is a
//! socket send and, on the replica, an acknowledgement the primary reads;
//! that kernel work was most of a replica's cost per write. The sender still
//! drains its channel on every wake, but flushes only when the round ends or
//! a full flush is buffered.
//!
//! These tests drive the real sender (`handle_replica`) over TCP against a
//! scripted replica and check that batching improved and that nothing a
//! replica receives changed: every frame once, in offset order, matching the
//! backlog; the flush cap; backpressure — a small channel, pipelined rounds
//! and large frames do not disconnect a healthy replica, while one that stops
//! reading is dropped and resumes from the backlog; acknowledgements; and
//! cancellation before a held flush.

use super::*;
use std::time::Duration;

/// A primary's replication side, with DEDUP replication off so the scripted
/// replica can speak the 0.5 handshake.
fn hub(backlog: usize) -> ReplRegistry {
    let hub = ReplHub::new();
    hub.configure_dedup(false);
    hub.configure_backlog(backlog);
    hub
}

type Senders = Arc<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>;

/// Serve replicas of `hub` on a local port, one `handle_replica` task per
/// connection, each with a channel of `capacity` frames. Returns the address,
/// the accept loop, and the sender tasks it spawned (to abort one).
async fn serve(
    hub: &ReplRegistry,
    capacity: usize,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>, Senders) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hub = Arc::clone(hub);
    let senders: Senders = Arc::default();
    let spawned = Arc::clone(&senders);
    let server = tokio::spawn(async move {
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let _ = socket.set_nodelay(true);
            let hub = Arc::clone(&hub);
            let sender = tokio::spawn(async move {
                let _ = handle_replica(
                    socket,
                    Arc::new(KeyValueStore::new()),
                    ServerState::for_tests(
                        Arc::new(SnapshotConfig {
                            path: PathBuf::from("unused.rdb"),
                            last_save: AtomicI64::new(0),
                            checkpoint_id: AtomicU64::new(0),
                        }),
                        Arc::clone(&hub),
                        false,
                    ),
                    hub,
                    None,
                    capacity,
                    IpAddr::from([127, 0, 0, 1]),
                    ReplAuthThrottle::new(),
                )
                .await;
            });
            spawned.lock().unwrap().push(sender);
        }
    });
    (addr, server, senders)
}

/// A scripted replica: completes the handshake asking to resume at `from`
/// (a partial synchronization), and returns the socket plus the frames the
/// primary sent to bring it up to the boundary.
async fn attach(addr: std::net::SocketAddr, from: u64) -> (TcpStream, u64, Vec<(u64, Vec<u8>)>) {
    let mut socket = TcpStream::connect(addr).await.unwrap();
    let mut magic = [0u8; 4];
    socket.read_exact(&mut magic).await.unwrap();
    assert_eq!(&magic, REPL_MAGIC);
    let mut run_len = [0u8; 2];
    socket.read_exact(&mut run_len).await.unwrap();
    let mut run_id = vec![0; u16::from_le_bytes(run_len) as usize];
    socket.read_exact(&mut run_id).await.unwrap();
    socket
        .write_all(&(run_id.len() as u16).to_le_bytes())
        .await
        .unwrap();
    socket.write_all(&run_id).await.unwrap();
    socket.write_all(&from.to_le_bytes()).await.unwrap();
    let mut mode = [0u8; 1];
    socket.read_exact(&mut mode).await.unwrap();
    assert_eq!(
        mode[0], b'P',
        "expected a partial synchronization from offset {from}"
    );
    let mut boundary = [0u8; 8];
    socket.read_exact(&mut boundary).await.unwrap();
    let boundary = u64::from_le_bytes(boundary);
    let mut count = [0u8; 4];
    socket.read_exact(&mut count).await.unwrap();
    let mut missing = Vec::new();
    for _ in 0..u32::from_le_bytes(count) {
        missing.push(read_frame(&mut socket).await);
    }
    (socket, boundary, missing)
}

/// One replication frame off the wire: offset, length, bytes.
async fn read_frame(socket: &mut TcpStream) -> (u64, Vec<u8>) {
    let mut offset = [0u8; 8];
    socket.read_exact(&mut offset).await.unwrap();
    let mut len = [0u8; 4];
    socket.read_exact(&mut len).await.unwrap();
    let mut bytes = vec![0u8; u32::from_le_bytes(len) as usize];
    socket.read_exact(&mut bytes).await.unwrap();
    (u64::from_le_bytes(offset), bytes)
}

async fn read_frames(socket: &mut TcpStream, n: usize) -> Vec<(u64, Vec<u8>)> {
    let mut got = Vec::with_capacity(n);
    for _ in 0..n {
        got.push(
            tokio::time::timeout(Duration::from_secs(10), read_frame(socket))
                .await
                .expect("the replica stream stalled"),
        );
    }
    got
}

async fn wait_for(what: &str, mut ready: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !ready() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

fn tag(writer: usize, i: usize) -> Vec<u8> {
    format!("w{writer}:{i}").into_bytes()
}

/// The batching itself. On one worker, a round of ready writers each fans
/// out a frame and yields, as a client connection does between commands.
/// The old sender ran in the LIFO slot after every writer and flushed one
/// frame at a time (about one flush per frame here); now one flush carries
/// the round. Every frame still arrives once, in offset order.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_round_of_writers_shares_one_flush() {
    let hub = hub(1 << 20);
    let (addr, server, _senders) = serve(&hub, 4096).await;
    let (mut replica, boundary, missing) = attach(addr, 0).await;
    assert_eq!((boundary, missing.len()), (0, 0));
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;

    const WRITERS: usize = 32;
    const ROUNDS: usize = 4;
    let (flushes0, frames0) = hub.send_stats();
    let writers: Vec<_> = (0..WRITERS)
        .map(|w| {
            let hub = Arc::clone(&hub);
            tokio::spawn(async move {
                for i in 0..ROUNDS {
                    hub.fan_out(tag(w, i)).await;
                    tokio::task::yield_now().await;
                }
            })
        })
        .collect();
    for writer in writers {
        writer.await.unwrap();
    }
    let got = read_frames(&mut replica, WRITERS * ROUNDS).await;
    assert!(
        got.iter()
            .enumerate()
            .all(|(i, (offset, _))| *offset == i as u64 + 1),
        "offsets are not contiguous from 1"
    );
    wait_for("the sender's counters", || {
        hub.send_stats().1 - frames0 == (WRITERS * ROUNDS) as u64
    })
    .await;
    let flushes = hub.send_stats().0 - flushes0;
    // Per writer, its own frames stay in its own order.
    for w in 0..WRITERS {
        let mine: Vec<_> = got
            .iter()
            .filter(|(_, b)| b.starts_with(format!("w{w}:").as_bytes()))
            .collect();
        assert_eq!(mine.len(), ROUNDS);
        assert!(
            mine.iter().enumerate().all(|(i, (_, b))| *b == tag(w, i)),
            "writer {w} reordered"
        );
    }
    assert!(
        hub.send_deferred() > 0,
        "no flush waited for the end of a round"
    );
    // ~1 flush per round now; the LIFO-slot sender made ~1 per frame.
    assert!(
        flushes * 8 <= (WRITERS * ROUNDS) as u64,
        "{flushes} flushes for {} frames: the sender is not batching rounds",
        WRITERS * ROUNDS
    );
    server.abort();
}

/// A burst far larger than one flush still goes out in flushes of at most
/// `REPL_SEND_BATCH_FRAMES`, in order: a queue already holding a full flush
/// is not held back by the yield.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_burst_larger_than_one_flush_is_capped_and_ordered() {
    let hub = hub(1 << 22);
    let (addr, server, _senders) = serve(&hub, 8192).await;
    let (mut replica, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    let (flushes0, frames0) = hub.send_stats();
    let deferred0 = hub.send_deferred();
    const N: usize = 2_000;
    // One synchronous burst on the worker: fan_out never yields, so the
    // whole burst is queued before the sender runs. (The test body itself
    // runs on the test thread, beside the worker, so it must not do this.)
    let burst = Arc::clone(&hub);
    tokio::spawn(async move {
        for i in 0..N {
            burst.fan_out(tag(0, i)).await;
        }
    })
    .await
    .unwrap();
    let got = read_frames(&mut replica, N).await;
    assert!(
        got.iter()
            .enumerate()
            .all(|(i, (o, b))| *o == i as u64 + 1 && *b == tag(0, i))
    );
    wait_for("the sender's counters", || {
        hub.send_stats().1 - frames0 >= N as u64
    })
    .await;
    assert_eq!(
        hub.send_stats().1 - frames0,
        N as u64,
        "the sender counted frames it did not send"
    );
    let flushes = hub.send_stats().0 - flushes0;
    assert!(
        flushes >= N.div_ceil(REPL_SEND_BATCH_FRAMES) as u64,
        "{flushes} flushes for {N} frames: a flush exceeded REPL_SEND_BATCH_FRAMES"
    );
    // Full flushes go out at once: only the last, partial flush of the burst
    // waits for the end of the round.
    let deferred = hub.send_deferred() - deferred0;
    assert!(
        deferred <= 1,
        "{deferred} deferred flushes for a burst queued in full before the sender ran"
    );
    server.abort();
}

/// Concurrent writers on four workers: the stream a replica receives is
/// exactly the backlog — the same frames at the same offsets, contiguous,
/// none duplicated or skipped — and a second replica resuming from offset 0
/// is served that same range from the backlog.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_writers_stream_exactly_the_backlog() {
    let hub = hub(1 << 22);
    let (addr, server, _senders) = serve(&hub, 4096).await;
    let (mut live, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    const WRITERS: usize = 8;
    const EACH: usize = 400;
    let writers: Vec<_> = (0..WRITERS)
        .map(|w| {
            let hub = Arc::clone(&hub);
            tokio::spawn(async move {
                for i in 0..EACH {
                    hub.fan_out(tag(w, i)).await;
                    if i.is_multiple_of(3) {
                        tokio::task::yield_now().await;
                    }
                }
            })
        })
        .collect();
    let streamed = read_frames(&mut live, WRITERS * EACH).await;
    for writer in writers {
        writer.await.unwrap();
    }
    assert!(
        streamed
            .iter()
            .enumerate()
            .all(|(i, (o, _))| *o == i as u64 + 1)
    );
    for w in 0..WRITERS {
        let mine: Vec<_> = streamed
            .iter()
            .filter(|(_, b)| b.starts_with(format!("w{w}:").as_bytes()))
            .collect();
        assert_eq!(
            mine.len(),
            EACH,
            "writer {w}: a frame was lost or duplicated"
        );
        assert!(
            mine.iter().enumerate().all(|(i, (_, b))| *b == tag(w, i)),
            "writer {w} reordered"
        );
    }
    let (_resumed, boundary, from_backlog) = attach(addr, 0).await;
    assert_eq!(boundary, (WRITERS * EACH) as u64);
    assert_eq!(
        from_backlog, streamed,
        "the live stream differs from the backlog"
    );
    server.abort();
}

/// A replica that stops reading: writers never block on it, its bounded
/// channel fills and it is disconnected; the batching yield does not make the
/// queue unbounded. Reconnecting from the last offset it applied, it is
/// served exactly the missing range from the backlog, then the live stream
/// continues contiguously.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stalled_replica_is_dropped_and_resumes_from_the_backlog() {
    let hub = hub(1 << 24);
    let (addr, server, _senders) = serve(&hub, 64).await;
    let (stalled, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    let started = std::time::Instant::now();
    let mut written = 0usize;
    // Large frames fill the socket buffers quickly; then the channel.
    let payload = vec![b'x'; 16 * 1024];
    while hub.count.load(Ordering::Relaxed) == 1 {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the stalled replica was never dropped"
        );
        let mut frame = tag(0, written);
        frame.extend_from_slice(&payload);
        hub.fan_out(frame).await;
        written += 1;
        if written.is_multiple_of(16) {
            tokio::task::yield_now().await;
        }
    }
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "writers stalled on a slow replica"
    );
    drop(stalled);

    // The replica applied nothing past the handshake: it resumes from 0.
    let (mut resumed, boundary, missing) = attach(addr, 0).await;
    assert_eq!(boundary, written as u64);
    assert_eq!(missing.len(), written);
    assert!(
        missing
            .iter()
            .enumerate()
            .all(|(i, (o, b))| *o == i as u64 + 1 && b.starts_with(&tag(0, i)))
    );
    wait_for("the replica to register again", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    for i in 0..50 {
        hub.fan_out(tag(1, i)).await;
    }
    let live = read_frames(&mut resumed, 50).await;
    assert!(
        live.iter()
            .enumerate()
            .all(|(i, (o, b))| *o == (written + i + 1) as u64 && *b == tag(1, i))
    );
    server.abort();
}

/// Aborted while it holds a flush for the end of the round (runtime
/// shutdown, or the connection's task being dropped), the sender has already
/// drained its channel into its buffer: no byte charge stays accounted to a
/// dead replica, nothing half-sent reaches it, and the next write
/// unregisters it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn aborting_the_sender_before_its_round_flush_strands_nothing() {
    let hub = hub(1 << 20);
    let (addr, server, senders) = serve(&hub, 4096).await;
    let (mut replica, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    let sender = senders.lock().unwrap().pop().expect("one sender task");
    let (charged, flushed) = {
        let hub = Arc::clone(&hub);
        // On the worker: queue frames (waking the sender into the LIFO
        // slot), then yield. The sender runs, drains them into its buffer and
        // waits for the end of the round, queued behind this task; resuming
        // first, this task aborts it before it flushes. (Relies on tokio
        // 1.54's scheduler: the LIFO slot, and the order deferred yields
        // resume in. A scheduler change makes the assertions below fail, not
        // pass.)
        tokio::spawn(async move {
            let (_, frames0) = hub.send_stats();
            for i in 0..10 {
                hub.fan_out(tag(0, i)).await;
            }
            tokio::task::yield_now().await;
            let charged = {
                let stream = hub.stream();
                stream.senders[0].pending_bytes.load(Ordering::Acquire)
            };
            sender.abort();
            (charged, hub.send_stats().1 - frames0)
        })
        .await
        .unwrap()
    };
    assert_eq!(charged, 0, "the sender had not drained its channel");
    assert_eq!(flushed, 0, "the sender flushed before it was aborted");
    // The abort takes effect when the runtime next polls the task, so keep
    // writing until a write finds the channel closed and unregisters it.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while hub.count.load(Ordering::Relaxed) != 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the dead replica was never unregistered"
        );
        hub.fan_out(b"after".to_vec()).await;
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    // The aborted task's buffer never reached the replica: its stream ends
    // without a frame.
    let mut byte = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(10), replica.read(&mut byte))
        .await
        .expect("the closed link was not noticed");
    assert_eq!(
        read.unwrap(),
        0,
        "a frame reached the replica after the abort"
    );
    server.abort();
}

/// A small channel (`RECACHED_REPL_BUFFER`) is drained on every wake, as
/// before 6N-C5: a round of ready writers does not fill it and disconnect a
/// healthy replica (review F1).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_small_channel_is_drained_on_every_wake() {
    let hub = hub(1 << 20);
    let (addr, server, _senders) = serve(&hub, 16).await;
    let (mut replica, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    const WRITERS: usize = 32;
    let writers: Vec<_> = (0..WRITERS)
        .map(|w| {
            let hub = Arc::clone(&hub);
            tokio::spawn(async move {
                for i in 0..4 {
                    hub.fan_out(tag(w, i)).await;
                    tokio::task::yield_now().await;
                }
            })
        })
        .collect();
    for writer in writers {
        writer.await.unwrap();
    }
    let got = read_frames(&mut replica, WRITERS * 4).await;
    assert!(got.iter().enumerate().all(|(i, (o, _))| *o == i as u64 + 1));
    assert_eq!(
        hub.count.load(Ordering::Relaxed),
        1,
        "a healthy replica was disconnected"
    );
    server.abort();
}

/// The replica's cumulative acknowledgements are still read by a sender that
/// batches: lag returns to zero once it acknowledges everything.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acknowledgements_are_still_read() {
    let hub = hub(1 << 20);
    let (addr, server, _senders) = serve(&hub, 4096).await;
    let (mut replica, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    for i in 0..100 {
        hub.fan_out(tag(0, i)).await;
    }
    let got = read_frames(&mut replica, 100).await;
    assert_eq!(hub.max_lag_frames().await, 100);
    replica.write_all(&got[49].0.to_le_bytes()).await.unwrap();
    replica.write_all(&got[99].0.to_le_bytes()).await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while hub.max_lag_frames().await != 0 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "acknowledgements were not read"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    server.abort();
}

/// One round of pipelining connections on the sender's worker: each poll
/// queues a whole pipeline (128 frames, no AOF wait in between), 5120 frames
/// in all against a 4096-frame channel. A sender that stopped draining until
/// the round ended disconnected a healthy replica (review R1); one that
/// drains on every wake and holds only the flush keeps the queue at about
/// one poll.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn one_round_of_pipelines_is_drained_as_it_is_queued() {
    let hub = hub(1 << 24);
    let (addr, server, _senders) = serve(&hub, 4096).await;
    let (mut replica, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    const WRITERS: usize = 40;
    const PIPELINE: usize = 128;
    let writers: Vec<_> = (0..WRITERS)
        .map(|w| {
            let hub = Arc::clone(&hub);
            tokio::spawn(async move {
                for i in 0..PIPELINE {
                    hub.fan_out(tag(w, i)).await;
                }
            })
        })
        .collect();
    let got = read_frames(&mut replica, WRITERS * PIPELINE).await;
    for writer in writers {
        writer.await.unwrap();
    }
    assert!(got.iter().enumerate().all(|(i, (o, _))| *o == i as u64 + 1));
    assert_eq!(
        hub.count.load(Ordering::Relaxed),
        1,
        "a healthy replica was disconnected by one round of pipelines"
    );
    assert!(
        hub.send_stats().0 * 8 <= (WRITERS * PIPELINE) as u64,
        "the pipelines were not batched"
    );
    server.abort();
}

/// The same for bytes: a round of large frames that would pass the replica's
/// byte budget if held in the channel (review R4).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn large_frames_do_not_overflow_a_batching_sender() {
    let hub = hub(1 << 26);
    hub.configure_queue_bytes(1 << 20);
    let (addr, server, _senders) = serve(&hub, 4096).await;
    let (mut replica, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    const WRITERS: usize = 32;
    const EACH: usize = 2;
    let payload = vec![b'x'; 32 * 1024];
    let writers: Vec<_> = (0..WRITERS)
        .map(|w| {
            let hub = Arc::clone(&hub);
            let payload = payload.clone();
            tokio::spawn(async move {
                for i in 0..EACH {
                    let mut frame = tag(w, i);
                    frame.extend_from_slice(&payload);
                    hub.fan_out(frame).await;
                }
                tokio::task::yield_now().await;
            })
        })
        .collect();
    let got = read_frames(&mut replica, WRITERS * EACH).await;
    for writer in writers {
        writer.await.unwrap();
    }
    assert!(got.iter().enumerate().all(|(i, (o, _))| *o == i as u64 + 1));
    assert_eq!(
        hub.count.load(Ordering::Relaxed),
        1,
        "a healthy replica was disconnected by one round of large frames"
    );
    server.abort();
}

/// The exposure review R1 found, recorded rather than fixed (6N-C5): at the
/// default capacity, 40 pipelining writers on the sender's worker that all
/// resume together each round, and a replica that acknowledges every burst
/// like `run_repl_client`. The sender, queued behind them at a round
/// boundary, is overrun and the replica disconnected. It is not new: on
/// the 6N-C5 host this disconnected 10 of 10 runs with the held flush and 8
/// of 10 with the pre-6N-C5 sender (flush on every wake), where the
/// acknowledgement's wake queues the sender behind the writers instead.
/// A measurement, not a pass/fail on the outcome: it checks that whatever
/// the replica received is contiguous from offset 1, and prints whether the
/// replica was disconnected (`--ignored --nocapture` to see it). Ignored
/// because it is slow and timing-dependent.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "measures a pre-existing exposure (6N-C5 review R1); prints the outcome"]
async fn pipelined_rounds_at_default_capacity_with_acks() {
    let hub = hub(1 << 24);
    let (addr, server, _senders) = serve(&hub, 4096).await;
    let (replica, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    const WRITERS: usize = 40;
    const PIPELINE: usize = 128;
    const ROUNDS: usize = 8;
    let total = WRITERS * PIPELINE * ROUNDS;
    let reader = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let (rd, mut wr) = replica.into_split();
            let mut rd = tokio::io::BufReader::with_capacity(64 * 1024, rd);
            let mut got = 0usize;
            let mut contiguous = true;
            while got < total {
                let mut h = [0u8; 12];
                if rd.read_exact(&mut h).await.is_err() {
                    return (got, contiguous);
                }
                let len = u32::from_le_bytes(h[8..12].try_into().unwrap()) as usize;
                let mut b = vec![0u8; len];
                if rd.read_exact(&mut b).await.is_err() {
                    return (got, contiguous);
                }
                let last = u64::from_le_bytes(h[..8].try_into().unwrap());
                got += 1;
                contiguous &= last == got as u64;
                if rd.buffer().is_empty() {
                    let _ = wr.write_all(&last.to_le_bytes()).await;
                }
            }
            (got, contiguous)
        })
    });
    let writers: Vec<_> = (0..WRITERS)
        .map(|w| {
            let hub = Arc::clone(&hub);
            tokio::spawn(async move {
                for round in 0..ROUNDS {
                    for i in 0..PIPELINE {
                        hub.fan_out(tag(w, round * PIPELINE + i)).await;
                    }
                    tokio::task::yield_now().await;
                }
            })
        })
        .collect();
    for writer in writers {
        writer.await.unwrap();
    }
    let (got, contiguous) = reader.await.unwrap();
    assert!(
        contiguous,
        "the replica received a gap or a reordered frame"
    );
    eprintln!(
        "6N-C5 R1 measurement: the replica received {got} of {total} frames ({})",
        if got == total {
            "kept up"
        } else {
            "disconnected: one round overran the channel"
        }
    );
    server.abort();
}
