//! 6O-I5: resynchronization at transaction-group boundaries
//! (`benchmarks/v0.6.0/6o-i/V0.6O-I-PROTOCOL-DESIGN.md` §7).
//!
//! - A capable replica resumes only at a group boundary; the primary
//!   answers a resume point inside a group with a full synchronization
//!   (rule 2), whatever its backlog still holds.
//! - A replica disconnected inside a group (a full queue, a dropped link)
//!   resumes before it and converges; one disconnected right after a group
//!   resumes after it.
//! - Chains: a middle replica's full sync resets its sub-replicas; a
//!   promoted middle replica goes on grouping for them.

use super::replication_dedup::{Node, node, serve_replicas, wait_for};
use super::*;
use core_engine::resp::Value;
use core_engine::store::write_pause;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

async fn exec(client: &mut WsClient, writes: &[&[&str]]) {
    client.cmd(&["MULTI"]).await;
    for w in writes {
        client.cmd(w).await;
    }
    let reply = client.cmd(&["EXEC"]).await;
    assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
}

/// Stops its follow loop when dropped (review D round 2, R2-1): a loop that
/// outlived its test reconnected to whatever later bound its primary's port,
/// such as another test's fake primary.
#[must_use = "the replica stops following when this is dropped"]
struct Following(Arc<std::sync::atomic::AtomicBool>);

impl Drop for Following {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}


/// Follow `primary_addr` on a thread of its own, reconnecting with the
/// resume state kept across links and cleared after a protocol error, as
/// `run_repl_client` does.
fn follow(replica: &Node, primary_addr: std::net::SocketAddr) -> Following {
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stopped = Arc::clone(&stop);
    let (store, state, tx, watches) = (
        Arc::clone(&replica.store),
        Arc::clone(&replica.state),
        replica.tx.clone(),
        Arc::clone(&replica.watches),
    );
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(async move {
                let mut resume = ReplResume::default();
                loop {
                    if stopped.load(Ordering::Relaxed) {
                        return;
                    }
                    if !state.is_replica() {
                        return;
                    }
                    if let Ok(mut socket) = TcpStream::connect(primary_addr).await
                        && let Err(e) = sync_from_primary(
                            &mut socket,
                            &store,
                            None,
                            &tx,
                            &watches,
                            &state,
                            &mut resume,
                        )
                        .await
                        && e.kind() == std::io::ErrorKind::InvalidData
                    {
                        resume = ReplResume::default();
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            });
    });
    Following(stop)
}

/// Every key of `keys` reads the same on `node` as on `primary`.
async fn converged(primary: &Node, node: &Node, keys: &[String], label: &str) {
    for key in keys {
        let want = primary.store.execute(Command::Get(key.clone()));
        wait_for(&node.store, key, want).await;
    }
    let _ = label;
}

// ── Rule 2 on a real primary ─────────────────────────────────────────────

/// A replica's handshake by hand: the mode byte the primary answers a
/// resume request with, and the socket.
async fn resume_request(
    addr: std::net::SocketAddr,
    run: &str,
    offset: u64,
    accept: &str,
) -> (u8, TcpStream) {
    let mut sock = TcpStream::connect(addr).await.unwrap();
    let mut magic = [0u8; 4];
    sock.read_exact(&mut magic).await.unwrap();
    let mut len = [0u8; 2];
    sock.read_exact(&mut len).await.unwrap();
    let mut id = vec![0u8; usize::from(u16::from_le_bytes(len))];
    sock.read_exact(&mut id).await.unwrap();
    let requested = format!("{run};accept={accept}");
    sock.write_all(&(requested.len() as u16).to_le_bytes())
        .await
        .unwrap();
    sock.write_all(requested.as_bytes()).await.unwrap();
    sock.write_all(&offset.to_le_bytes()).await.unwrap();
    let mut mode = [0u8; 1];
    sock.read_exact(&mut mode).await.unwrap();
    (mode[0], sock)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_real_primary_answers_a_resume_inside_a_group_with_a_full_sync() {
    let primary = node(false, true).await;
    let addr = serve_replicas(&primary).await;
    let mut client = WsClient::connect(primary.ws).await;
    client.cmd(&["SET", "rb:pre", "1"]).await;
    exec(
        &mut client,
        &[
            &["SET", "rb:a", "1"],
            &["SET", "rb:b", "1"],
            &["SET", "rb:c", "1"],
        ],
    )
    .await;
    let last = primary.state.replicas.current_offset();
    let first = last - 2;
    let run = primary.state.replicas.run_id();
    let capable = "dedup1,expirylog1,txn1";
    let legacy = "dedup1,expirylog1";
    // Before the group, after it: partial. Inside it: full, for a capable
    // replica; partial for a legacy one, as before 6O-I.
    for (at, accept, partial) in [
        (first - 1, capable, true),
        (first, capable, false),
        (first + 1, capable, false),
        (last, capable, true),
        (first, legacy, true),
    ] {
        let before = primary.state.replicas.syncs();
        let (mode, mut sock) = resume_request(addr, &run, at, accept).await;
        assert_eq!(
            mode == b'P',
            partial,
            "resume at {at} ({accept}): mode {:?}",
            mode as char
        );
        // Counted as served (review D, D-5): one partial or one full sync.
        let mut after = primary.state.replicas.syncs();
        for _ in 0..200 {
            if after != before {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
            after = primary.state.replicas.syncs();
        }
        let want = if partial {
            (before.0 + 1, before.1)
        } else {
            (before.0, before.1 + 1)
        };
        assert_eq!(after, want, "resume at {at} ({accept})");
        if at == first - 1 {
            // The partial sync: three data frames, the header first.
            let mut head = [0u8; 12];
            sock.read_exact(&mut head).await.unwrap(); // boundary, count
            assert_eq!(u32::from_le_bytes(head[8..12].try_into().unwrap()), 3);
            let mut frame = [0u8; 12];
            sock.read_exact(&mut frame).await.unwrap();
            assert_eq!(
                u64::from_le_bytes(frame[..8].try_into().unwrap()),
                0,
                "header"
            );
        }
    }
}

// ── A replica disconnected inside a group ────────────────────────────────

/// The primary's queue to the replica is smaller than a transaction's
/// frames: `fan_out_group` disconnects it while queueing the group. The
/// link's buffered frames are refused once its handle is dropped, so the
/// replica usually sees the link end before the group (review D, D-3: an
/// end inside a group is covered deterministically by
/// `a_link_that_ends_inside_a_group_applies_none_of_it_and_resumes_before_it`).
/// It resumes from the backlog, its readers never see part of a
/// transaction, and it converges.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_replica_whose_queue_overflows_on_groups_resumes_and_converges() {
    let primary = node(false, true).await;
    primary.state.replicas.configure_queue_bytes(4 * 1024);
    let replica = node(true, true).await;
    let _following = follow(&replica, serve_replicas(&primary).await);
    let mut client = WsClient::connect(primary.ws).await;
    client.cmd(&["SET", "sl:sentinel", "0"]).await;
    wait_for(&replica.store, "sl:sentinel", bulk("0")).await;
    let keys: Vec<String> = (0..40).map(|i| format!("sl:k{i}")).collect();
    let stop = Arc::new(AtomicBool::new(false));
    let reader = {
        let (store, stop, keys) = (Arc::clone(&replica.store), Arc::clone(&stop), keys.clone());
        std::thread::spawn(move || {
            let mut mixed = 0usize;
            while !stop.load(Ordering::Relaxed) {
                if let Value::Array(Some(items)) = store.execute(Command::MGet(keys.clone()))
                    && items.iter().any(|v| *v != items[0])
                {
                    mixed += 1;
                }
            }
            mixed
        })
    };
    let (before, _) = primary.state.replicas.syncs();
    let value = "v".repeat(200);
    for round in 0..3 {
        let v = format!("{round}{value}");
        let writes: Vec<Vec<&str>> = keys
            .iter()
            .map(|k| vec!["SET", k.as_str(), v.as_str()])
            .collect();
        let writes: Vec<&[&str]> = writes.iter().map(Vec::as_slice).collect();
        exec(&mut client, &writes).await;
    }
    converged(&primary, &replica, &keys, "slow").await;
    stop.store(true, Ordering::Relaxed);
    assert_eq!(
        reader.join().unwrap(),
        0,
        "a reader saw part of a transaction"
    );
    let (after, _) = primary.state.replicas.syncs();
    assert!(after > before, "it resynchronized partially at least once");
    assert!(primary.state.replicas.queue_disconnects() > 0);
}

/// A primary's side of the handshake by hand: (socket, run id requested,
/// resume offset).
async fn fake_accept(listener: &TcpListener) -> (TcpStream, String, u64) {
    let (mut sock, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .expect("the replica connects")
        .unwrap();
    let advertised = "fake-run;caps=expirylog1,txn1";
    sock.write_all(REPL_MAGIC).await.unwrap();
    sock.write_all(&(advertised.len() as u16).to_le_bytes())
        .await
        .unwrap();
    sock.write_all(advertised.as_bytes()).await.unwrap();
    let mut len = [0u8; 2];
    sock.read_exact(&mut len).await.unwrap();
    let mut id = vec![0u8; usize::from(u16::from_le_bytes(len))];
    sock.read_exact(&mut id).await.unwrap();
    let mut offset = [0u8; 8];
    sock.read_exact(&mut offset).await.unwrap();
    let id = String::from_utf8(id).unwrap();
    (
        sock,
        id.split(';').next().unwrap().to_string(),
        u64::from_le_bytes(offset),
    )
}

/// A fake primary sends a whole group, then closes the link before the
/// replica's acknowledgement can be read: the replica applied it, and asks
/// to resume after it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_replica_that_applied_a_group_resumes_after_it() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let replica = node(true, false).await;
    let _following = follow(&replica, listener.local_addr().unwrap());
    let (mut sock, _, _) = fake_accept(&listener).await;
    sock.write_all(b"F").await.unwrap();
    sock.write_all(&10u64.to_le_bytes()).await.unwrap();
    let empty = rmp_serde::to_vec(&Vec::<SnapshotEntry>::new()).unwrap();
    sock.write_all(&(empty.len() as u32).to_le_bytes())
        .await
        .unwrap();
    sock.write_all(&empty).await.unwrap();
    let mut ack = [0u8; 8];
    sock.read_exact(&mut ack).await.unwrap();
    let frames = [
        resp_push(&[b"SET", b"ack:a", b"1"]),
        resp_push(&[b"SET", b"ack:b", b"1"]),
    ];
    let mut out = group_header(ReplGroup {
        first: 11,
        count: 2,
        bytes: frames.iter().map(|f| f.len() as u64).sum(),
    })
    .to_vec();
    for (i, f) in frames.iter().enumerate() {
        out.extend_from_slice(&(11 + i as u64).to_le_bytes());
        out.extend_from_slice(&(f.len() as u32).to_le_bytes());
        out.extend_from_slice(f);
    }
    sock.write_all(&out).await.unwrap();
    drop(sock);
    let (_sock, run, offset) = fake_accept(&listener).await;
    assert_eq!((run.as_str(), offset), ("fake-run", 12));
    assert_eq!(
        replica.store.execute(Command::Get("ack:b".into())),
        bulk("1")
    );
}

// ── Chains ───────────────────────────────────────────────────────────────

/// A → B → C. A's stream is reset (a part-applied write on A, 6O-G): B
/// takes a full sync and resets its own stream, so C does too; groups keep
/// flowing atomically, and C converges with A.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_middle_replicas_full_sync_resets_its_sub_replicas_and_they_converge() {
    let a = node(false, true).await;
    let b = node(true, true).await;
    let c = node(true, true).await;
    let _following = follow(&b, serve_replicas(&a).await);
    let _following = follow(&c, serve_replicas(&b).await);
    let mut client = WsClient::connect(a.ws).await;
    let keys: Vec<String> = ["ch:a", "ch:b", "ch:c"]
        .iter()
        .map(|k| k.to_string())
        .collect();
    exec(&mut client, &[&["SET", "ch:a", "1"], &["SET", "ch:b", "1"]]).await;
    converged(&a, &c, &keys[..2], "before").await;
    let b_run = b.state.replicas.run_id();
    a.state.replicas.reset_stream();
    exec(&mut client, &[&["SET", "ch:a", "2"], &["SET", "ch:c", "2"]]).await;
    // A key deleted while B and C resynchronize: gone on both.
    exec(&mut client, &[&["DEL", "ch:b"], &["SET", "ch:a", "3"]]).await;
    converged(&a, &b, &keys, "B").await;
    converged(&a, &c, &keys, "C").await;
    assert_ne!(b.state.replicas.run_id(), b_run, "B reset its stream");
}

/// A → B → C, then B is promoted: its stream, and C's link to it, go on;
/// a transaction on B reaches C as one group, applied atomically.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_promoted_middle_replica_keeps_grouping_for_its_sub_replicas() {
    let a = node(false, true).await;
    let b = node(true, true).await;
    let c = node(true, true).await;
    let _following = follow(&b, serve_replicas(&a).await);
    let _following = follow(&c, serve_replicas(&b).await);
    let mut client = WsClient::connect(a.ws).await;
    client.cmd(&["SET", "pm:a", "0"]).await;
    wait_for(&c.store, "pm:a", bulk("0")).await;
    b.state.promote_to_primary(&b.store).await;
    let mut on_b = WsClient::connect(b.ws).await;
    let pause = write_pause::after_transaction_write_of(&c.store, "pm:a", &["pm:a", "pm:b"]);
    exec(&mut on_b, &[&["SET", "pm:a", "1"], &["SET", "pm:b", "1"]]).await;
    assert!(
        pause.wait_reached(Duration::from_secs(5)),
        "C applies B's transaction as one group"
    );
    drop(pause);
    wait_for(&c.store, "pm:b", bulk("1")).await;
    wait_for(&c.store, "pm:a", bulk("1")).await;
}
