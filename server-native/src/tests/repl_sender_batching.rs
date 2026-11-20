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
//!
//! 6N-C6 sizes the queue by bytes: each queued frame charges its payload plus
//! `REPL_QUEUE_FRAME_OVERHEAD` against `RECACHED_REPL_BUFFER_BYTES`, and the
//! default frame cap follows that budget (65,536) instead of 4096, which one
//! round of pipelining connections overran (6N-C5 review R1). The tests
//! after `pipelined_rounds_at_default_capacity_with_acks` cover the brief's
//! scenarios A–I: the regression, bursts at the limits, small explicit
//! channels, large frames and accounting, stalled replicas alone and beside a
//! healthy one, cancellation with frames still queued, full and partial
//! recovery, and unauthenticated peers.

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
    serve_with(hub, capacity, None).await
}

/// [`serve`], requiring `password` first when it is set.
async fn serve_with(
    hub: &ReplRegistry,
    capacity: usize,
    password: Option<&str>,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>, Senders) {
    let password = password.map(|p| Arc::new(p.to_string()));
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
            let password = password.clone();
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
                    password,
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

/// The default frame cap with no `RECACHED_REPL_BUFFER`: derived from the
/// default byte budget.
fn default_capacity() -> usize {
    repl_channel_capacity(None, DEFAULT_REPL_QUEUE_BYTES)
}

/// A scripted replica that reads in its own runtime, as `run_repl_client`
/// does, acknowledging each burst it has applied when `acks` is set. It reads
/// until the primary closes the stream or it holds `stop` frames (checked
/// after each frame; `u64::MAX` until the test knows the total), and returns
/// how many frames it read, whether their offsets were contiguous from
/// `after + 1`, and the last offset.
fn spawn_reader(
    replica: TcpStream,
    after: u64,
    acks: bool,
    stop: Arc<AtomicU64>,
) -> tokio::task::JoinHandle<(u64, bool, u64)> {
    tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let (rd, mut wr) = replica.into_split();
            let mut rd = tokio::io::BufReader::with_capacity(64 * 1024, rd);
            let mut got = 0u64;
            let mut last = after;
            let mut contiguous = true;
            while got < stop.load(Ordering::Acquire) {
                let mut h = [0u8; 12];
                if rd.read_exact(&mut h).await.is_err() {
                    break;
                }
                let len = u32::from_le_bytes(h[8..12].try_into().unwrap()) as usize;
                let mut b = vec![0u8; len];
                if rd.read_exact(&mut b).await.is_err() {
                    break;
                }
                let offset = u64::from_le_bytes(h[..8].try_into().unwrap());
                contiguous &= offset == last + 1;
                last = offset;
                got += 1;
                if acks && rd.buffer().is_empty() {
                    let _ = wr.write_all(&last.to_le_bytes()).await;
                }
            }
            (got, contiguous, last)
        })
    })
}

/// Waits for a [`spawn_reader`] to finish, failing rather than hanging if
/// it never does (6N-C6 review L1).
async fn finish(reader: tokio::task::JoinHandle<(u64, bool, u64)>) -> (u64, bool, u64) {
    tokio::time::timeout(Duration::from_secs(60), reader)
        .await
        .expect("the replica's reader never finished")
        .unwrap()
}

/// Rounds of pipelining connections, every one resuming together: each of
/// `writers` queues `pipeline` frames per poll, then yields. They are
/// spawned from a task on the worker, in one poll, so the rounds have the
/// same shape however busy the test's own thread is.
async fn pipelined_rounds(hub: &ReplRegistry, writers: usize, pipeline: usize, rounds: usize) {
    let hub = Arc::clone(hub);
    tokio::spawn(async move {
        let tasks: Vec<_> = (0..writers)
            .map(|w| {
                let hub = Arc::clone(&hub);
                tokio::spawn(async move {
                    for round in 0..rounds {
                        for i in 0..pipeline {
                            hub.fan_out(tag(w, round * pipeline + i)).await;
                        }
                        tokio::task::yield_now().await;
                    }
                })
            })
            .collect();
        for task in tasks {
            task.await.unwrap();
        }
    })
    .await
    .unwrap();
}

/// Scenario A, the regression for the exposure 6N-C5 review R1 recorded.
///
/// A worker's maintenance park (every 61 polls) can come in the middle of a
/// round. It wakes the held flush's deferral, and an acknowledgement's
/// readiness, and either wake queues the sender behind the round's writers
/// not yet run; while it is queued, their `try_send`s cannot move it. At
/// 4096 frames, the pre-6N-C6 default, the channel filled with about 32
/// polls queued while it held 27.6 KB, 0.3% of the byte budget, and the
/// replica was disconnected on every run (6N-C6 root cause). With the cap
/// derived from the byte budget the same rounds fit: every frame arrives
/// once and in order, the replica is never dropped or resynchronized, the
/// sender is never kept from its queue for more than about a round, and
/// the queue stays within both of its bounds. (How deep the queue gets
/// depends on where the parks fall; `one_poll_of_more_than_4096_frames…`
/// passes the old cap deterministically.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pipelined_rounds_at_default_capacity_with_acks() {
    let hub = hub(1 << 24);
    let capacity = default_capacity();
    let (addr, server, _senders) = serve(&hub, capacity).await;
    let (replica, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    const WRITERS: usize = 40;
    const PIPELINE: usize = 128;
    const ROUNDS: usize = 8;
    let total = (WRITERS * PIPELINE * ROUNDS) as u64;
    let reader = spawn_reader(replica, 0, true, Arc::new(AtomicU64::new(total)));
    pipelined_rounds(&hub, WRITERS, PIPELINE, ROUNDS).await;
    let (got, contiguous, _) = finish(reader).await;
    assert!(
        contiguous,
        "the replica received a gap or a reordered frame"
    );
    assert_eq!(
        got, total,
        "the replica was disconnected by a round of pipelines"
    );
    assert_eq!(hub.count.load(Ordering::Relaxed), 1);
    assert_eq!(hub.queue_disconnects(), 0);
    assert_eq!(
        hub.syncs(),
        (1, 0),
        "only the initial partial synchronization"
    );
    let peak = hub.send_queue_peak();
    eprintln!("6N-C6 regression: the sender found at most {peak} frames queued");
    assert!(
        peak <= 2 * (WRITERS * PIPELINE) as u64,
        "the sender was kept from its queue for more than a round ({peak} frames)"
    );
    assert!(hub.send_queue_bytes_peak() <= DEFAULT_REPL_QUEUE_BYTES as u64);
    wait_for("the queue to empty", || {
        hub.stream().senders[0]
            .pending_bytes
            .load(Ordering::Acquire)
            == 0
    })
    .await;
    server.abort();
}

/// Scenario A's boundary: an explicit `RECACHED_REPL_BUFFER=4096` is still a
/// strict frame cap, so the same rounds can overrun it, as they did on every
/// run before 6N-C6 (whether one does depends on where the worker's parks
/// fall). Either way nothing is lost or repeated: the replica holds a
/// contiguous prefix, and if it was disconnected, resuming from its last
/// offset it is served exactly the rest from the backlog.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pipelined_rounds_against_an_explicit_4096_frame_cap_lose_nothing() {
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
    let total = (WRITERS * PIPELINE * ROUNDS) as u64;
    let reader = spawn_reader(replica, 0, true, Arc::new(AtomicU64::new(total)));
    pipelined_rounds(&hub, WRITERS, PIPELINE, ROUNDS).await;
    let (got, contiguous, last) = finish(reader).await;
    assert!(contiguous);
    assert!(
        hub.send_queue_peak() <= 4096,
        "an explicit cap was exceeded"
    );
    eprintln!("6N-C6 explicit 4096-frame cap: the replica received {got} of {total} frames");
    if got == total {
        assert_eq!(hub.queue_disconnects(), 0);
    } else {
        assert_eq!(last, got);
        assert_eq!(hub.queue_disconnects(), 1);
        let (_resumed, boundary, missing) = attach(addr, last).await;
        assert_eq!(boundary, total);
        assert_eq!(missing.len() as u64, total - got);
        assert!(
            missing
                .iter()
                .enumerate()
                .all(|(i, (o, _))| *o == last + i as u64 + 1)
        );
        assert_eq!(
            hub.syncs(),
            (2, 0),
            "both synchronizations from the backlog"
        );
    }
    server.abort();
}

/// Scenario B just above the old limit, deterministically: one connection's
/// poll — a deep pipeline read in one go — queues 6000 frames, and the
/// sender, on the same worker, cannot drain until that poll ends. The old
/// 4096-frame cap disconnected the replica at frame 4097, whatever the
/// scheduling; the default cap holds them all, and the sender finds exactly
/// that many queued when it wakes.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn one_poll_of_more_than_4096_frames_fits_the_default_queue() {
    let hub = hub(1 << 24);
    let (addr, server, _senders) = serve(&hub, default_capacity()).await;
    let (mut replica, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    const FRAMES: usize = 6000;
    {
        let hub = Arc::clone(&hub);
        tokio::spawn(async move {
            for i in 0..FRAMES {
                hub.fan_out(tag(0, i)).await;
            }
        })
        .await
        .unwrap();
    }
    let got = read_frames(&mut replica, FRAMES).await;
    assert!(
        got.iter()
            .enumerate()
            .all(|(i, (o, b))| *o == i as u64 + 1 && *b == tag(0, i))
    );
    assert_eq!(
        hub.count.load(Ordering::Relaxed),
        1,
        "the replica was disconnected"
    );
    assert_eq!(hub.queue_disconnects(), 0);
    assert_eq!(hub.send_queue_peak(), FRAMES as u64);
    let charged: usize = (0..FRAMES)
        .map(|i| repl_queue_charge(tag(0, i).len()))
        .sum();
    assert_eq!(hub.send_queue_bytes_peak(), charged as u64);
    server.abort();
}

/// The queue's two bounds, as pure functions: what a frame charges, and the
/// frame cap with and without an explicit `RECACHED_REPL_BUFFER`.
#[test]
fn the_queue_charge_and_frame_cap_are_bounded() {
    assert_eq!(REPL_QUEUE_FRAME_OVERHEAD, 128);
    assert_eq!(repl_queue_charge(0), 128);
    assert_eq!(repl_queue_charge(30), 158);
    // A charge that cannot be represented saturates, so it exceeds every
    // budget instead of wrapping into a small one.
    assert_eq!(repl_queue_charge(usize::MAX), usize::MAX);
    assert_eq!(repl_queue_charge(usize::MAX - 127), usize::MAX);
    // An explicit cap is kept exactly, whatever the budget.
    assert_eq!(
        repl_channel_capacity(Some(16), DEFAULT_REPL_QUEUE_BYTES),
        16
    );
    assert_eq!(
        repl_channel_capacity(Some(4096), DEFAULT_REPL_QUEUE_BYTES),
        4096
    );
    assert_eq!(repl_channel_capacity(Some(1 << 22), 1024), 1 << 22);
    // Otherwise the budget sets it: 65,536 frames at the default 8 MiB.
    assert_eq!(default_capacity(), 65_536);
    assert_eq!(DEFAULT_REPL_CHANNEL_CAPACITY, 65_536);
    assert_eq!(repl_channel_capacity(None, 128 * 100), 100);
    assert_eq!(repl_channel_capacity(None, 128 * 100 + 127), 100);
    // At least one frame, and no more than the ceiling.
    assert_eq!(repl_channel_capacity(None, 1), 1);
    assert_eq!(
        repl_channel_capacity(None, usize::MAX),
        REPL_QUEUE_MAX_DERIVED_FRAMES
    );
    assert_eq!(
        repl_channel_capacity(None, 128 << 20),
        REPL_QUEUE_MAX_DERIVED_FRAMES
    );
    // `RECACHED_REPL_BUFFER` as read from the environment (review L3).
    assert_eq!(parse_repl_frame_cap(None), None);
    assert_eq!(parse_repl_frame_cap(Some("4096")), Some(4096));
    assert_eq!(parse_repl_frame_cap(Some(" 16 ")), Some(16));
    assert_eq!(parse_repl_frame_cap(Some("0")), None);
    assert_eq!(parse_repl_frame_cap(Some("-1")), None);
    assert_eq!(parse_repl_frame_cap(Some("4k")), None);
    assert_eq!(parse_repl_frame_cap(Some("")), None);
    assert_eq!(
        parse_repl_frame_cap(Some(&usize::MAX.to_string())),
        Some(REPL_QUEUE_MAX_EXPLICIT_FRAMES)
    );
    assert_eq!(
        repl_channel_capacity(parse_repl_frame_cap(Some("4096")), DEFAULT_REPL_QUEUE_BYTES),
        4096
    );
    assert_eq!(
        repl_channel_capacity(parse_repl_frame_cap(Some("x")), DEFAULT_REPL_QUEUE_BYTES),
        65_536
    );
}

/// The charged bytes of the one registered replica, shared with its handle.
fn charged(hub: &ReplRegistry) -> Arc<AtomicUsize> {
    Arc::clone(&hub.stream().senders[0].pending_bytes)
}

/// Scenario B at the default limits, with nothing draining the queue: the
/// byte budget is reached first, at exactly as many frames as it can
/// charge, and the next frame disconnects the replica. Each queued frame
/// is charged its payload plus the overhead, and nothing more.
#[tokio::test]
async fn a_burst_fills_the_default_queue_to_exactly_its_byte_budget() {
    let hub = hub(1 << 20);
    let _rx = hub
        .attach_test_replica(default_capacity(), DEFAULT_REPL_QUEUE_BYTES)
        .await;
    let pending = charged(&hub);
    let frame = vec![b'v'; 100];
    let fits = DEFAULT_REPL_QUEUE_BYTES / repl_queue_charge(frame.len());
    for _ in 0..fits {
        hub.fan_out(frame.clone()).await;
    }
    assert_eq!(
        hub.count.load(Ordering::Relaxed),
        1,
        "dropped below the budget"
    );
    assert_eq!(pending.load(Ordering::Acquire), fits * 228);
    hub.fan_out(frame.clone()).await;
    assert_eq!(hub.count.load(Ordering::Relaxed), 0, "kept past the budget");
    assert_eq!(hub.queue_disconnects(), 1);
    // The refused frame reserved nothing.
    assert_eq!(pending.load(Ordering::Acquire), fits * 228);
}

/// Scenario B for the frame count: zero-length frames, the smallest charge,
/// cannot queue without bound. The default budget admits exactly the
/// default cap of them, 65,536, and refuses the next.
#[tokio::test]
async fn tiny_frames_are_bounded_by_the_overhead_charge() {
    let hub = hub(1 << 10);
    let _rx = hub
        .attach_test_replica(default_capacity(), DEFAULT_REPL_QUEUE_BYTES)
        .await;
    let pending = charged(&hub);
    for _ in 0..default_capacity() {
        hub.fan_out(Vec::new()).await;
    }
    assert_eq!(hub.count.load(Ordering::Relaxed), 1);
    assert_eq!(pending.load(Ordering::Acquire), DEFAULT_REPL_QUEUE_BYTES);
    hub.fan_out(Vec::new()).await;
    assert_eq!(hub.count.load(Ordering::Relaxed), 0);
    assert_eq!(hub.queue_disconnects(), 1);
}

/// Scenario B with an explicit frame cap below the budget: the cap is still
/// strict, and a frame refused for it releases the bytes it had reserved,
/// so the accounting matches what is queued.
#[tokio::test]
async fn an_explicit_frame_cap_is_strict_and_a_refused_frame_releases_its_charge() {
    let hub = hub(1 << 20);
    let mut rx = hub.attach_test_replica(4, DEFAULT_REPL_QUEUE_BYTES).await;
    let pending = charged(&hub);
    for i in 0..4 {
        hub.fan_out(tag(0, i)).await;
    }
    assert_eq!(hub.count.load(Ordering::Relaxed), 1);
    let four = 4 * repl_queue_charge(tag(0, 0).len());
    assert_eq!(pending.load(Ordering::Acquire), four);
    hub.fan_out(tag(0, 4)).await;
    assert_eq!(
        hub.count.load(Ordering::Relaxed),
        0,
        "kept past its frame cap"
    );
    assert_eq!(hub.queue_disconnects(), 1);
    assert_eq!(pending.load(Ordering::Acquire), four);
    // Draining what was queued releases every charge.
    let mut drained = 0;
    while let Ok(queued) = rx.try_recv() {
        assert_eq!(queued.offset(), drained + 1);
        drained += 1;
    }
    assert_eq!(drained, 4);
    assert_eq!(pending.load(Ordering::Acquire), 0);
}

/// Scenario B through the real sender: bursts of twice the old 4096 limit,
/// separated by chances to drain, from many concurrent writers on two
/// workers. The healthy replica keeps every frame, in order, connected.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_bursts_twice_the_old_limit_keep_a_healthy_replica() {
    let hub = hub(1 << 24);
    let (addr, server, _senders) = serve(&hub, default_capacity()).await;
    let (replica, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    const BURSTS: usize = 4;
    const WRITERS: usize = 64;
    const PIPELINE: usize = 128;
    let total = (BURSTS * WRITERS * PIPELINE) as u64;
    let reader = spawn_reader(replica, 0, true, Arc::new(AtomicU64::new(total)));
    for burst in 0..BURSTS {
        let writers: Vec<_> = (0..WRITERS)
            .map(|w| {
                let hub = Arc::clone(&hub);
                tokio::spawn(async move {
                    for i in 0..PIPELINE {
                        hub.fan_out(tag(w, burst * PIPELINE + i)).await;
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.await.unwrap();
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let (got, contiguous, _) = finish(reader).await;
    assert!(contiguous);
    assert_eq!(got, total);
    assert_eq!(hub.count.load(Ordering::Relaxed), 1);
    assert_eq!(hub.queue_disconnects(), 0);
    assert!(hub.send_queue_bytes_peak() <= DEFAULT_REPL_QUEUE_BYTES as u64);
    server.abort();
}

/// Scenario C: small explicit channels (`RECACHED_REPL_BUFFER` = 16, 64,
/// 256) flush on every wake. A round that fits the channel never disconnects
/// a healthy replica, wherever the sender lands in it. A burst larger than
/// the channel, queued before the sender can run, disconnects it — the
/// configured limit — and it resumes from the backlog with nothing lost or
/// repeated.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn small_explicit_channels_keep_rounds_that_fit_and_resume_after_one_that_does_not() {
    for capacity in [16usize, 64, 256] {
        let hub = hub(1 << 22);
        let (addr, server, _senders) = serve(&hub, capacity).await;
        let (mut replica, _, _) = attach(addr, 0).await;
        wait_for("the replica to register", || {
            hub.count.load(Ordering::Relaxed) == 1
        })
        .await;
        // Rounds of capacity/2 writers, one frame each per round.
        let writers = capacity / 2;
        const ROUNDS: usize = 4;
        let tasks: Vec<_> = (0..writers)
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
        for task in tasks {
            task.await.unwrap();
        }
        let fitting = writers * ROUNDS;
        let got = read_frames(&mut replica, fitting).await;
        assert!(got.iter().enumerate().all(|(i, (o, _))| *o == i as u64 + 1));
        assert_eq!(
            hub.count.load(Ordering::Relaxed),
            1,
            "capacity {capacity}: a round that fits disconnected a healthy replica"
        );
        assert_eq!(hub.queue_disconnects(), 0);

        // One task queues capacity + 1 frames without yielding: the sender,
        // on the same worker, cannot drain in between.
        let burst = {
            let hub = Arc::clone(&hub);
            tokio::spawn(async move {
                for i in 0..=capacity {
                    hub.fan_out(tag(9, i)).await;
                }
            })
        };
        burst.await.unwrap();
        assert_eq!(hub.count.load(Ordering::Relaxed), 0, "capacity {capacity}");
        assert_eq!(hub.queue_disconnects(), 1);
        // Unregistered, its connection is closed without sending what was
        // queued (review M1): it ends after at most a contiguous prefix.
        // Resuming from the last frame it received, it is served exactly the
        // rest of the burst from the backlog.
        let mut last = fitting as u64;
        let mut rest = Vec::new();
        replica.read_to_end(&mut rest).await.unwrap();
        let mut at = 0;
        while rest.len() - at >= 12 {
            let offset = u64::from_le_bytes(rest[at..at + 8].try_into().unwrap());
            let len = u32::from_le_bytes(rest[at + 8..at + 12].try_into().unwrap()) as usize;
            if rest.len() - at - 12 < len {
                break; // cut off mid-frame: a replica never applies it
            }
            assert_eq!(offset, last + 1);
            last = offset;
            at += 12 + len;
        }
        let (_resumed, boundary, missing) = attach(addr, last).await;
        assert_eq!(boundary, (fitting + capacity + 1) as u64);
        assert!(missing.iter().enumerate().all(|(i, (o, b))| {
            *o == last + i as u64 + 1 && *b == tag(9, (*o as usize) - fitting - 1)
        }));
        assert_eq!(missing.len() as u64, boundary - last);
        server.abort();
    }
}

/// Scenario D: a frame of exactly the budget less the overhead fits; one
/// byte more is refused before it reserves anything; and a large frame
/// leaves no room for even an empty one.
#[tokio::test]
async fn frames_near_the_byte_budget_are_charged_exactly() {
    const BUDGET: usize = 1 << 20;
    let exact = hub(4 << 20);
    let _rx = exact.attach_test_replica(default_capacity(), BUDGET).await;
    let pending = charged(&exact);
    exact
        .fan_out(vec![b'x'; BUDGET - REPL_QUEUE_FRAME_OVERHEAD])
        .await;
    assert_eq!(exact.count.load(Ordering::Relaxed), 1);
    assert_eq!(pending.load(Ordering::Acquire), BUDGET);
    exact.fan_out(Vec::new()).await;
    assert_eq!(exact.count.load(Ordering::Relaxed), 0);
    assert_eq!(pending.load(Ordering::Acquire), BUDGET);

    let over = hub(4 << 20);
    let _rx = over.attach_test_replica(default_capacity(), BUDGET).await;
    let pending = charged(&over);
    over.fan_out(vec![b'x'; BUDGET - REPL_QUEUE_FRAME_OVERHEAD + 1])
        .await;
    assert_eq!(
        over.count.load(Ordering::Relaxed),
        0,
        "kept a frame past the budget"
    );
    assert_eq!(
        pending.load(Ordering::Acquire),
        0,
        "a refused frame stayed charged"
    );
}

/// Scenario D through the real sender: frames of 256 KiB against a 1 MiB
/// budget from concurrent writers. Whatever the replica's fate, the sender
/// never finds more than the budget charged, and a disconnected replica
/// resumes with nothing lost.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn large_frames_never_queue_past_the_byte_budget() {
    const BUDGET: usize = 1 << 20;
    let hub = hub(1 << 26);
    hub.configure_queue_bytes(BUDGET);
    let (addr, server, _senders) = serve(&hub, repl_channel_capacity(None, BUDGET)).await;
    let (replica, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    const WRITERS: usize = 16;
    const EACH: usize = 8;
    let total = (WRITERS * EACH) as u64;
    let reader = spawn_reader(replica, 0, true, Arc::new(AtomicU64::new(total)));
    let payload = vec![b'x'; 256 * 1024];
    let writers: Vec<_> = (0..WRITERS)
        .map(|w| {
            let hub = Arc::clone(&hub);
            let payload = payload.clone();
            tokio::spawn(async move {
                for i in 0..EACH {
                    let mut frame = tag(w, i);
                    frame.extend_from_slice(&payload);
                    hub.fan_out(frame).await;
                    tokio::task::yield_now().await;
                }
            })
        })
        .collect();
    for writer in writers {
        writer.await.unwrap();
    }
    let (got, contiguous, last) = finish(reader).await;
    assert!(contiguous);
    assert!(hub.send_queue_bytes_peak() <= BUDGET as u64);
    if got < total {
        assert!(hub.queue_disconnects() >= 1);
        let (_resumed, boundary, missing) = attach(addr, last).await;
        assert_eq!(boundary, total);
        assert_eq!(missing.len() as u64, total - got);
    }
    server.abort();
}

/// Writes 1 KiB frames as writer `w` until `until` holds, yielding every 64
/// frames and checking that no replica's queue is charged past `budget`.
/// Fails if that takes 20 s: writers never wait on a replica. Returns how
/// many frames it wrote.
async fn write_until(
    hub: &ReplRegistry,
    w: usize,
    budget: usize,
    pace: Option<Duration>,
    mut until: impl FnMut() -> bool,
) -> usize {
    let started = std::time::Instant::now();
    let payload = vec![b'x'; 1024];
    let mut written = 0usize;
    while !until() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "writers stalled, or a stalled replica was never dropped"
        );
        let mut frame = tag(w, written);
        frame.extend_from_slice(&payload);
        hub.fan_out(frame).await;
        written += 1;
        if written.is_multiple_of(64) {
            for replica in &hub.stream().senders {
                assert!(replica.pending_bytes.load(Ordering::Acquire) <= budget);
            }
            match pace {
                Some(pause) => tokio::time::sleep(pause).await,
                None => tokio::task::yield_now().await,
            }
        }
    }
    written
}

/// Scenario E at the default limits: a replica that stops reading. Its
/// socket buffers fill, then its queue, which never holds more than the
/// byte budget; it is disconnected without writers ever waiting on it, and
/// resumes with exactly the missing range from the backlog. The
/// disconnect and both synchronizations are counted in `INFO`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stalled_replica_at_the_default_limits_is_dropped_within_its_budget() {
    let hub = hub(64 << 20);
    let (addr, server, _senders) = serve(&hub, default_capacity()).await;
    let (stalled, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    let written = write_until(&hub, 0, DEFAULT_REPL_QUEUE_BYTES, None, || {
        hub.count.load(Ordering::Relaxed) == 0
    })
    .await;
    assert_eq!(hub.queue_disconnects(), 1);
    assert!(hub.send_queue_bytes_peak() <= DEFAULT_REPL_QUEUE_BYTES as u64);
    // More frames than the old 4096-frame cap were outstanding when it was
    // dropped: the budget, not a frame count, decided.
    assert!(written > 4096, "dropped after {written} frames");
    drop(stalled);

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
    assert_eq!(hub.syncs(), (2, 0));
    let info = hub.info().await;
    assert_eq!(info.queue_disconnects, 1);
    assert_eq!((info.syncs_partial, info.syncs_full), (2, 0));
    server.abort();
}

/// A replica resuming this primary's run from offset `from`, which the
/// backlog no longer retains: the primary answers with a full
/// synchronization. Returns the socket and the snapshot's offset; live
/// frames follow it.
async fn attach_full(addr: std::net::SocketAddr, from: u64) -> (TcpStream, u64) {
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
        mode[0], b'F',
        "expected a full synchronization from offset {from}"
    );
    let mut boundary = [0u8; 8];
    socket.read_exact(&mut boundary).await.unwrap();
    let mut len = [0u8; 4];
    socket.read_exact(&mut len).await.unwrap();
    let mut snapshot = vec![0u8; u32::from_le_bytes(len) as usize];
    socket.read_exact(&mut snapshot).await.unwrap();
    (socket, u64::from_le_bytes(boundary))
}

/// Scenario H, when the backlog has moved on: a stalled replica dropped for
/// its budget, whose offset the backlog no longer retains, is refused a
/// partial synchronization and takes a full one; the live stream then
/// continues from the snapshot's offset with nothing skipped or repeated.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_dropped_replica_past_the_backlog_takes_a_full_synchronization() {
    let hub = hub(256 * 1024);
    let (addr, server, _senders) = serve(&hub, default_capacity()).await;
    let (stalled, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    let written = write_until(&hub, 0, DEFAULT_REPL_QUEUE_BYTES, None, || {
        hub.count.load(Ordering::Relaxed) == 0
    })
    .await;
    drop(stalled);
    // Offset 0 left the 256 KiB backlog long ago.
    let (mut full, boundary) = attach_full(addr, 0).await;
    assert_eq!(boundary, written as u64);
    wait_for("the replica to register again", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    for i in 0..50 {
        hub.fan_out(tag(1, i)).await;
    }
    let live = read_frames(&mut full, 50).await;
    assert!(
        live.iter()
            .enumerate()
            .all(|(i, (o, b))| *o == boundary + i as u64 + 1 && *b == tag(1, i))
    );
    assert_eq!(hub.syncs(), (1, 1));
    server.abort();
}

/// Scenario F: two replicas, one reading and one stalled. The stalled one is
/// dropped on its own budget; the healthy one, with its own budget, keeps
/// every frame in order and stays connected; neither queue is ever charged
/// past its budget, so together they hold at most two budgets.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stalled_replica_is_dropped_while_a_healthy_one_keeps_every_frame() {
    let hub = hub(64 << 20);
    let (addr, server, _senders) = serve(&hub, default_capacity()).await;
    let (healthy, _, _) = attach(addr, 0).await;
    let (stalled, _, _) = attach(addr, 0).await;
    wait_for("both replicas to register", || {
        hub.count.load(Ordering::Relaxed) == 2
    })
    .await;
    let stop = Arc::new(AtomicU64::new(u64::MAX));
    let reader = spawn_reader(healthy, 0, true, Arc::clone(&stop));
    // Paced to a rate the healthy replica sustains (about 64 MB/s).
    let pace = Some(Duration::from_millis(1));
    let mut written = write_until(&hub, 0, DEFAULT_REPL_QUEUE_BYTES, pace, || {
        hub.count.load(Ordering::Relaxed) < 2
    })
    .await;
    assert_eq!(
        hub.count.load(Ordering::Relaxed),
        1,
        "the healthy replica was dropped"
    );
    assert_eq!(hub.queue_disconnects(), 1);
    written += write_until(&hub, 1, DEFAULT_REPL_QUEUE_BYTES, pace, {
        let mut n = 0;
        move || {
            n += 1;
            n > 1000
        }
    })
    .await;
    stop.store(written as u64, Ordering::Release);
    let (got, contiguous, last) = finish(reader).await;
    assert!(contiguous);
    assert_eq!((got, last), (written as u64, written as u64));
    assert_eq!(hub.count.load(Ordering::Relaxed), 1);
    assert!(hub.send_queue_bytes_peak() <= DEFAULT_REPL_QUEUE_BYTES as u64);
    drop(stalled);
    server.abort();
}

/// Scenario G with frames still in the channel: the sender is aborted
/// before it ever drains them. Dropping its receiver drops them, releasing
/// every byte they were charged; nothing reaches the replica; the next
/// write unregisters it; and resuming, it is served each frame exactly once.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn aborting_the_sender_with_frames_still_queued_releases_their_charge() {
    let hub = hub(1 << 20);
    let (addr, server, senders) = serve(&hub, default_capacity()).await;
    let (mut replica, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    let sender = senders.lock().unwrap().pop().expect("one sender task");
    let pending = charged(&hub);
    let queued = {
        let hub = Arc::clone(&hub);
        let pending = Arc::clone(&pending);
        // One poll on the worker: the sender, woken into the LIFO slot, runs
        // only after it, and by then it is aborted.
        tokio::spawn(async move {
            for i in 0..10 {
                hub.fan_out(tag(0, i)).await;
            }
            let queued = pending.load(Ordering::Acquire);
            sender.abort();
            queued
        })
        .await
        .unwrap()
    };
    assert_eq!(queued, 10 * repl_queue_charge(tag(0, 0).len()));
    wait_for("the aborted sender's queue to be released", || {
        pending.load(Ordering::Acquire) == 0
    })
    .await;
    let mut after = 0usize;
    while hub.count.load(Ordering::Relaxed) != 0 {
        assert!(after < 1000, "the dead replica was never unregistered");
        hub.fan_out(tag(1, after)).await;
        after += 1;
    }
    let mut byte = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(10), replica.read(&mut byte))
        .await
        .expect("the closed link was not noticed");
    assert_eq!(
        read.unwrap(),
        0,
        "a frame reached the replica after the abort"
    );
    let (_resumed, boundary, missing) = attach(addr, 0).await;
    assert_eq!(boundary, (10 + after) as u64);
    assert_eq!(missing.len(), 10 + after);
    assert!(missing.iter().enumerate().all(|(i, (o, b))| {
        *o == i as u64 + 1 && *b == if i < 10 { tag(0, i) } else { tag(1, i - 10) }
    }));
    server.abort();
}

/// Scenario I: a peer with the wrong replication password, and one that
/// authenticates but sends an invalid resume request, are refused before a
/// replica queue exists: writes meanwhile reach no queue and charge nothing,
/// and only an authenticated, valid handshake registers.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unauthenticated_and_invalid_handshakes_never_reach_a_replica_queue() {
    let hub = hub(1 << 20);
    let (addr, server, senders) = serve_with(&hub, default_capacity(), Some("s3cret")).await;

    // A peer that connects and has not authenticated yet, held open while
    // writes go on: it is still in the auth handshake, with no queue.
    let mut midway = TcpStream::connect(addr).await.unwrap();
    midway.write_all(b"s3c").await.unwrap();
    wait_for("the peer's handshake task", || {
        senders.lock().unwrap().len() == 1
    })
    .await;
    for i in 0..100 {
        hub.fan_out(tag(2, i)).await;
    }
    assert_eq!(hub.count.load(Ordering::Relaxed), 0);
    assert!(hub.stream().senders.is_empty());
    drop(midway);

    let mut wrong = TcpStream::connect(addr).await.unwrap();
    wrong.write_all(b"guess\n").await.unwrap();
    let mut reply = Vec::new();
    wrong.read_to_end(&mut reply).await.unwrap();
    assert!(
        reply.starts_with(b"-ERR"),
        "{:?}",
        String::from_utf8_lossy(&reply)
    );

    let mut invalid = TcpStream::connect(addr).await.unwrap();
    invalid.write_all(b"s3cret\n").await.unwrap();
    let mut ok = [0u8; 4];
    invalid.read_exact(&mut ok).await.unwrap();
    assert_eq!(&ok, b"+OK\n");
    let mut magic = [0u8; 4];
    invalid.read_exact(&mut magic).await.unwrap();
    let mut run_len = [0u8; 2];
    invalid.read_exact(&mut run_len).await.unwrap();
    let mut run_id = vec![0; u16::from_le_bytes(run_len) as usize];
    invalid.read_exact(&mut run_id).await.unwrap();
    // A run id longer than the 128 bytes a resume request may carry.
    invalid.write_all(&1000u16.to_le_bytes()).await.unwrap();
    let mut rest = Vec::new();
    let _ = invalid.read_to_end(&mut rest).await;
    assert!(rest.is_empty(), "the invalid request was answered");

    for i in 0..100 {
        hub.fan_out(tag(0, i)).await;
    }
    assert_eq!(hub.count.load(Ordering::Relaxed), 0);
    assert!(hub.stream().senders.is_empty());
    let finished: Vec<_> = senders.lock().unwrap().drain(..).collect();
    assert_eq!(finished.len(), 3);
    for task in finished {
        task.await.unwrap();
    }
    assert_eq!(hub.syncs(), (0, 0));
    server.abort();
}

/// Review M1: a replica that stops reading but keeps its connection open is
/// unregistered for its full queue, and its connection is then closed at
/// once. Its sender, blocked writing to a socket nobody reads, used to stay
/// there with every frame still queued — up to the byte budget, outside every
/// gauge — until the peer read again or TCP gave up. Now the sender ends and
/// every charge is released while the peer still holds its end open, and
/// the peer finds its stream closed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unregistered_replica_that_keeps_its_socket_open_is_closed() {
    let hub = hub(64 << 20);
    let (addr, server, senders) = serve(&hub, default_capacity()).await;
    let (mut stalled, _, _) = attach(addr, 0).await;
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    let sender = senders.lock().unwrap().pop().expect("one sender task");
    let pending = charged(&hub);
    write_until(&hub, 0, DEFAULT_REPL_QUEUE_BYTES, None, || {
        hub.count.load(Ordering::Relaxed) == 0
    })
    .await;
    tokio::time::timeout(Duration::from_secs(10), sender)
        .await
        .expect("the unregistered replica's sender is still blocked on its socket")
        .unwrap();
    assert_eq!(pending.load(Ordering::Acquire), 0, "frames stayed charged");
    // Still open on the peer's side: it reads what reached its buffers,
    // then finds the stream closed.
    let mut drained = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(10), stalled.read_to_end(&mut drained))
        .await
        .expect("the connection was not closed");
    // Closed (EOF) or reset: either way it is gone.
    drop(read);
    server.abort();
}
