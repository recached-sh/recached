//! 6N-C6C: a replica sender reads acknowledgements cancel-safely.
//!
//! A replica acknowledges the cumulative offset it has applied: 8 bytes,
//! little-endian (`run_repl_client`). The sender read them with
//! `read_exact` inside its `select!`, so when a frame won the `select!`
//! after part of an acknowledgement had arrived, the dropped `read_exact`
//! took those bytes with it. Every later acknowledgement was then read
//! across a boundary, and the lag the primary reports (`INFO`
//! `recached_replication_lag_frames`, the Prometheus gauge) was wrong from
//! then on, usually 0 (6N-C5 review F4). Nothing else reads the
//! acknowledged offset.
//!
//! These tests drive the real sender (`handle_replica`) over an in-memory
//! pipe, so they decide exactly which acknowledgement bytes arrive when.

use super::*;
use std::time::Duration;
use tokio::io::DuplexStream;

fn hub() -> ReplRegistry {
    let hub = ReplHub::new();
    hub.configure_dedup(false);
    hub.configure_backlog(1 << 20);
    hub
}

/// Runs the sender on one end of a pipe and completes the handshake from the
/// other as a replica resuming at offset 0. Returns the replica's end and
/// the sender task.
async fn attach(
    hub: &ReplRegistry,
) -> (DuplexStream, tokio::task::JoinHandle<std::io::Result<()>>) {
    let (primary, mut replica) = tokio::io::duplex(1 << 16);
    let sender = tokio::spawn(handle_replica(
        primary,
        Arc::new(KeyValueStore::new()),
        ServerState::for_tests(
            Arc::new(SnapshotConfig {
                path: PathBuf::from("unused.rdb"),
                last_save: AtomicI64::new(0),
                checkpoint_id: AtomicU64::new(0),
            }),
            Arc::clone(hub),
            false,
        ),
        Arc::clone(hub),
        None,
        4096,
        IpAddr::from([127, 0, 0, 1]),
        ReplAuthThrottle::new(),
    ));
    let mut magic = [0u8; 4];
    replica.read_exact(&mut magic).await.unwrap();
    assert_eq!(&magic, REPL_MAGIC);
    let mut run_len = [0u8; 2];
    replica.read_exact(&mut run_len).await.unwrap();
    let mut run_id = vec![0; u16::from_le_bytes(run_len) as usize];
    replica.read_exact(&mut run_id).await.unwrap();
    replica
        .write_all(&(run_id.len() as u16).to_le_bytes())
        .await
        .unwrap();
    replica.write_all(&run_id).await.unwrap();
    replica.write_all(&0u64.to_le_bytes()).await.unwrap();
    let mut mode = [0u8; 1];
    replica.read_exact(&mut mode).await.unwrap();
    assert_eq!(mode[0], b'P');
    let mut rest = [0u8; 12];
    replica.read_exact(&mut rest).await.unwrap();
    assert_eq!(&rest[8..], &0u32.to_le_bytes(), "no frames to catch up");
    wait_for("the replica to register", || {
        hub.count.load(Ordering::Relaxed) == 1
    })
    .await;
    (replica, sender)
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

/// Fans out `n` frames and reads them on the replica's side.
async fn stream(hub: &ReplRegistry, replica: &mut DuplexStream, n: usize) -> u64 {
    let mut last = 0;
    for i in 0..n {
        hub.fan_out(format!("f{i}").into_bytes()).await;
    }
    for _ in 0..n {
        let mut head = [0u8; 12];
        tokio::time::timeout(Duration::from_secs(10), replica.read_exact(&mut head))
            .await
            .expect("the stream stalled")
            .unwrap();
        let mut body = vec![0u8; u32::from_le_bytes(head[8..].try_into().unwrap()) as usize];
        replica.read_exact(&mut body).await.unwrap();
        last = u64::from_le_bytes(head[..8].try_into().unwrap());
    }
    last
}

/// The offset the primary holds as acknowledged by its one replica.
fn acked(hub: &ReplRegistry) -> u64 {
    hub.stream().senders[0].acked.load(Ordering::Relaxed)
}

/// Gives the sender time to read what was just written. The assertions do
/// not depend on it having done so; the reproduction needs it to have.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(50)).await;
}

/// 1. One acknowledgement arriving a byte at a time is read whole.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_acknowledgement_split_across_reads_is_read_whole() {
    let hub = hub();
    let (mut replica, _sender) = attach(&hub).await;
    stream(&hub, &mut replica, 300).await;
    for byte in 300u64.to_le_bytes() {
        replica.write_all(&[byte]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    wait_for("the acknowledgement", || acked(&hub) != 0).await;
    assert_eq!(acked(&hub), 300);
    assert_eq!(hub.max_lag_frames().await, 0);
}

/// 2. The reproduction (6N-C5 review F4). Three bytes of an acknowledgement
/// arrive and are read; then a frame wins the sender's `select!`; then the
/// rest of that acknowledgement and a whole second one arrive. With
/// `read_exact`, the three bytes went with the dropped read and the next
/// eight were read across the boundary: 100 << 40 instead of 100, so the
/// primary reported no lag at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_partial_acknowledgement_survives_a_frame_winning_the_select() {
    let hub = hub();
    let (mut replica, _sender) = attach(&hub).await;
    stream(&hub, &mut replica, 100).await;
    let first = 50u64.to_le_bytes();
    replica.write_all(&first[..3]).await.unwrap();
    settle().await;
    // A frame now wins the `select!` over the read holding those 3 bytes.
    let sent = stream(&hub, &mut replica, 1).await;
    assert_eq!(sent, 101);
    replica.write_all(&first[3..]).await.unwrap();
    replica.write_all(&100u64.to_le_bytes()).await.unwrap();
    wait_for("an acknowledgement", || acked(&hub) != 0).await;
    wait_for("the second acknowledgement", || acked(&hub) >= 100).await;
    assert_eq!(
        acked(&hub),
        100,
        "the acknowledgements were read across a boundary"
    );
    assert_eq!(hub.max_lag_frames().await, 1);
}

/// 3. Frames keep flowing while every acknowledgement arrives in three
/// pieces. The acknowledged offset only ever holds a value the replica
/// sent, never more than was sent, and ends at the last one.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn frames_interleaved_with_partial_acknowledgements_keep_the_offsets_exact() {
    let hub = hub();
    let (mut replica, _sender) = attach(&hub).await;
    let mut acknowledged = vec![0u64];
    for _ in 0..20 {
        let applied = stream(&hub, &mut replica, 7).await;
        // Acknowledged once its last piece arrives.
        acknowledged.push(applied);
        let ack = applied.to_le_bytes();
        for piece in [&ack[..3], &ack[3..5], &ack[5..]] {
            replica.write_all(piece).await.unwrap();
            tokio::time::sleep(Duration::from_millis(2)).await;
            // A frame in between each piece, so the read races the channel.
            hub.fan_out(b"between".to_vec()).await;
            let mut head = [0u8; 12];
            replica.read_exact(&mut head).await.unwrap();
            let mut body = [0u8; 7];
            replica.read_exact(&mut body).await.unwrap();
            let now = acked(&hub);
            assert!(
                acknowledged.contains(&now),
                "the primary holds {now}, which the replica never acknowledged"
            );
            assert!(now <= hub.current_offset());
        }
    }
    let last = *acknowledged.last().unwrap();
    wait_for("the last acknowledgement", || acked(&hub) == last).await;
    assert_eq!(hub.max_lag_frames().await, hub.current_offset() - last);
}

/// 4. Several acknowledgements in one read, and one read ending part-way
/// through the next: the highest complete one counts, and the partial one is
/// completed by the following read.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn several_acknowledgements_in_one_read_keep_the_highest() {
    let hub = hub();
    let (mut replica, _sender) = attach(&hub).await;
    stream(&hub, &mut replica, 60).await;
    let mut burst = Vec::new();
    for ack in [10u64, 20, 30] {
        burst.extend_from_slice(&ack.to_le_bytes());
    }
    replica.write_all(&burst).await.unwrap();
    wait_for("the burst", || acked(&hub) == 30).await;
    let mut burst = 40u64.to_le_bytes().to_vec();
    burst.extend_from_slice(&50u64.to_le_bytes()[..5]);
    replica.write_all(&burst).await.unwrap();
    wait_for("the complete one", || acked(&hub) == 40).await;
    settle().await;
    assert_eq!(acked(&hub), 40, "a partial acknowledgement was counted");
    replica.write_all(&50u64.to_le_bytes()[5..]).await.unwrap();
    wait_for("the completed one", || acked(&hub) == 50).await;
    assert_eq!(hub.max_lag_frames().await, 10);
}

/// 5. The replica closes its side part-way through an acknowledgement. The
/// partial one is discarded, the sender ends cleanly, nothing stays charged,
/// and the next write unregisters the replica so it can reconnect.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_replica_closing_mid_acknowledgement_ends_the_stream_cleanly() {
    let hub = hub();
    let (mut replica, sender) = attach(&hub).await;
    stream(&hub, &mut replica, 20).await;
    replica.write_all(&10u64.to_le_bytes()).await.unwrap();
    wait_for("the first acknowledgement", || acked(&hub) == 10).await;
    replica.write_all(&20u64.to_le_bytes()[..3]).await.unwrap();
    let pending = Arc::clone(&hub.stream().senders[0].pending_bytes);
    replica.shutdown().await.unwrap();
    let ended = tokio::time::timeout(Duration::from_secs(10), sender)
        .await
        .expect("the sender did not end")
        .unwrap();
    assert!(ended.is_ok(), "{ended:?}");
    assert_eq!(acked(&hub), 10, "the partial acknowledgement was counted");
    hub.fan_out(b"after".to_vec()).await;
    assert_eq!(hub.count.load(Ordering::Relaxed), 0);
    assert_eq!(pending.load(Ordering::Acquire), 0);
}

/// 6. Acknowledgements are cumulative: a replayed or reordered lower one
/// never moves the acknowledged offset back.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acknowledgements_only_ever_advance() {
    let hub = hub();
    let (mut replica, _sender) = attach(&hub).await;
    stream(&hub, &mut replica, 30).await;
    let mut seen = Vec::new();
    for ack in [10u64, 5, 20, 20, 15, 30] {
        replica.write_all(&ack.to_le_bytes()).await.unwrap();
        settle().await;
        seen.push(acked(&hub));
    }
    assert_eq!(seen, [10, 10, 20, 20, 20, 30]);
}

/// 7. The lag the primary reports follows sent minus acknowledged while
/// acknowledgements arrive in pieces, never negative and never past what
/// was sent.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lag_follows_the_acknowledgements_received_in_pieces() {
    let hub = hub();
    let (mut replica, _sender) = attach(&hub).await;
    let sent = stream(&hub, &mut replica, 100).await;
    assert_eq!(hub.max_lag_frames().await, 100);
    for applied in [25u64, 50, 75, 100] {
        let ack = applied.to_le_bytes();
        replica.write_all(&ack[..6]).await.unwrap();
        settle().await;
        hub.fan_out(b"x".to_vec()).await;
        let mut frame = [0u8; 13];
        replica.read_exact(&mut frame).await.unwrap();
        replica.write_all(&ack[6..]).await.unwrap();
        wait_for("the acknowledgement", || acked(&hub) == applied).await;
        assert_eq!(
            hub.max_lag_frames().await,
            hub.current_offset() - applied,
            "lag after acknowledging {applied}"
        );
    }
    assert!(hub.current_offset() > sent);
}

/// 8. The sender is aborted while it holds part of an acknowledgement: it
/// leaves nothing charged, the replica sees the stream end, and the next
/// write unregisters it. (Aborting with frames still queued is covered by
/// `repl_sender_batching`.)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aborting_the_sender_with_a_partial_acknowledgement_strands_nothing() {
    let hub = hub();
    let (mut replica, sender) = attach(&hub).await;
    stream(&hub, &mut replica, 10).await;
    replica.write_all(&10u64.to_le_bytes()[..4]).await.unwrap();
    settle().await;
    let pending = Arc::clone(&hub.stream().senders[0].pending_bytes);
    sender.abort();
    assert!(sender.await.unwrap_err().is_cancelled());
    wait_for("the queue to be released", || {
        pending.load(Ordering::Acquire) == 0
    })
    .await;
    hub.fan_out(b"after".to_vec()).await;
    assert_eq!(hub.count.load(Ordering::Relaxed), 0);
    let mut byte = [0u8; 1];
    assert_eq!(replica.read(&mut byte).await.unwrap(), 0);
}
