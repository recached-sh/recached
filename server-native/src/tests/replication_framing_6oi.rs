//! 6O-I3: transaction groups on the replication stream (`caps=txn1`,
//! `benchmarks/v0.6.0/6o-i/V0.6O-I-PROTOCOL-DESIGN.md` §6).
//!
//! - A primary advertises `txn1`; a replica accepts it; never inferred.
//! - An `EXEC`'s frames take consecutive offsets in one stream-lock hold
//!   and, two or more, form a group.
//! - A replica that accepted `txn1` is sent a 24-byte header at offset 0
//!   before each group's first frame; any other replica is sent today's
//!   stream.
//! - A replica buffers a group until its last frame, applies it then, and
//!   never applies or acknowledges part of one: a link that ends inside a
//!   group resumes before it; a malformed group forces a full sync.

use super::replication_dedup::{Node, node, serve_replicas, wait_for};
use super::*;
use core_engine::resp::Value;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

fn nil() -> Value {
    Value::BulkString(None)
}

fn frame(words: &[&str]) -> Vec<u8> {
    let parts: Vec<&[u8]> = words.iter().map(|w| w.as_bytes()).collect();
    resp_push(&parts)
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

/// Follow `primary_addr` for the rest of the test, reconnecting with the
/// resume state kept across links and cleared after a protocol error, as
/// the server's replication loop (`run_repl_client`) does.
fn follow_reconnecting(replica: &Node, primary_addr: std::net::SocketAddr) -> Following {
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stopped = Arc::clone(&stop);
    let (store, state, tx, watches) = (
        Arc::clone(&replica.store),
        Arc::clone(&replica.state),
        replica.tx.clone(),
        Arc::clone(&replica.watches),
    );
    tokio::spawn(async move {
        let mut resume = ReplResume::default();
        loop {
            if stopped.load(Ordering::Relaxed) {
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
    Following(stop)
}

// ── Negotiation ──────────────────────────────────────────────────────────

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_primary_advertises_txn1_with_its_other_capabilities() {
    let primary = node(false, true).await;
    let id = advertised(serve_replicas(&primary).await).await;
    assert!(id.ends_with(";caps=dedup1,expirylog1,txn1"), "{id}");
    // 40-character run id, 28 bytes of parameters: under the 128 limit.
    assert!(id.len() <= 128, "{}", id.len());
    let plain = node(false, false).await;
    let id = advertised(serve_replicas(&plain).await).await;
    assert!(id.ends_with(";caps=expirylog1,txn1"), "{id}");
    // A primary before 6O-I (test hook): no txn1.
    plain
        .state
        .replicas
        .test_without_txn
        .store(true, Ordering::Relaxed);
    let id = advertised(serve_replicas(&plain).await).await;
    assert!(id.ends_with(";caps=expirylog1"), "{id}");
}

// ── What the primary sends ───────────────────────────────────────────────

/// A replica speaking the handshake by hand: accepts `accept`, takes the
/// full sync, then returns the socket positioned at the live stream.
async fn fake_replica(addr: std::net::SocketAddr, accept: &str) -> TcpStream {
    let mut sock = TcpStream::connect(addr).await.unwrap();
    let mut magic = [0u8; 4];
    sock.read_exact(&mut magic).await.unwrap();
    let mut len = [0u8; 2];
    sock.read_exact(&mut len).await.unwrap();
    let mut id = vec![0u8; usize::from(u16::from_le_bytes(len))];
    sock.read_exact(&mut id).await.unwrap();
    let requested = format!(";accept={accept}");
    sock.write_all(&(requested.len() as u16).to_le_bytes())
        .await
        .unwrap();
    sock.write_all(requested.as_bytes()).await.unwrap();
    sock.write_all(&0u64.to_le_bytes()).await.unwrap();
    let mut mode = [0u8; 1];
    sock.read_exact(&mut mode).await.unwrap();
    let mut boundary = [0u8; 8];
    sock.read_exact(&mut boundary).await.unwrap();
    let payloads = if mode[0] == b'D' { 2 } else { 1 };
    for _ in 0..payloads {
        let mut len = [0u8; 4];
        sock.read_exact(&mut len).await.unwrap();
        let mut payload = vec![0u8; u32::from_le_bytes(len) as usize];
        sock.read_exact(&mut payload).await.unwrap();
    }
    sock
}

async fn next_frame(sock: &mut TcpStream) -> (u64, Vec<u8>) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut offset = [0u8; 8];
        sock.read_exact(&mut offset).await.unwrap();
        let mut len = [0u8; 4];
        sock.read_exact(&mut len).await.unwrap();
        let mut bytes = vec![0u8; u32::from_le_bytes(len) as usize];
        sock.read_exact(&mut bytes).await.unwrap();
        (u64::from_le_bytes(offset), bytes)
    })
    .await
    .expect("a frame within 5 s")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_capable_replica_gets_a_header_before_each_group_and_an_older_one_none() {
    let primary = node(false, true).await;
    let addr = serve_replicas(&primary).await;
    let mut capable = fake_replica(addr, "dedup1,expirylog1,txn1").await;
    let mut older = fake_replica(addr, "dedup1,expirylog1").await;
    let mut client = WsClient::connect(primary.ws).await;
    client.cmd(&["SET", "h:solo", "1"]).await;
    exec(&mut client, &[&["SET", "h:a", "1"], &["SET", "h:b", "2"]]).await;
    let solo = primary.state.replicas.current_offset() - 2;
    let first = solo + 1;
    let (a, b) = (frame(&["SET", "h:a", "1"]), frame(&["SET", "h:b", "2"]));

    // The single write: a frame, no header, to both.
    for sock in [&mut capable, &mut older] {
        let (offset, bytes) = next_frame(sock).await;
        assert_eq!((offset, bytes), (solo, frame(&["SET", "h:solo", "1"])));
    }
    // The capable replica: the header, exactly the design's bytes, then
    // the two frames at consecutive offsets.
    let (offset, header) = next_frame(&mut capable).await;
    assert_eq!(offset, 0);
    let mut want = vec![1u8, 1, 0, 0];
    want.extend_from_slice(&2u32.to_le_bytes());
    want.extend_from_slice(&first.to_le_bytes());
    want.extend_from_slice(&((a.len() + b.len()) as u64).to_le_bytes());
    assert_eq!(header, want);
    assert_eq!(next_frame(&mut capable).await, (first, a.clone()));
    assert_eq!(next_frame(&mut capable).await, (first + 1, b.clone()));
    // The older one: the same frames, no header.
    assert_eq!(next_frame(&mut older).await, (first, a));
    assert_eq!(next_frame(&mut older).await, (first + 1, b));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_groups_offsets_are_consecutive_among_concurrent_writers() {
    let primary = node(false, true).await;
    let mut rx = primary
        .state
        .replicas
        .attach_test_replica(1 << 20, 1 << 30)
        .await;
    let mut writers = Vec::new();
    for w in 0..4 {
        let ws = primary.ws;
        writers.push(tokio::spawn(async move {
            let mut client = WsClient::connect(ws).await;
            for i in 0..50 {
                let k = format!("cw:{w}:{i}");
                if i % 2 == 0 {
                    exec(
                        &mut client,
                        &[
                            &["SET", &k, "1"],
                            &["SET", &format!("{k}:b"), "1"],
                            &["INCR", "cw:n"],
                        ],
                    )
                    .await;
                } else {
                    client.cmd(&["SET", &k, "1"]).await;
                }
            }
        }));
    }
    for w in writers {
        w.await.unwrap();
    }
    let mut frames = Vec::new();
    while let Ok(queued) = rx.try_recv() {
        frames.push((queued.offset(), queued.group()));
    }
    assert_eq!(frames.len(), 4 * (25 * 3 + 25));
    let mut groups = 0;
    let mut i = 0;
    while i < frames.len() {
        let (offset, group) = frames[i];
        if let Some(g) = group {
            assert_eq!(g.first, offset, "a group starts at its first frame");
            assert_eq!(g.count, 3);
            // Its other frames follow at consecutive offsets; only the first
            // carries the group (the header goes before it).
            for j in 1..3 {
                assert_eq!(frames[i + j], (g.first + j as u64, None));
            }
            groups += 1;
            i += 3;
        } else {
            i += 1;
        }
    }
    assert_eq!(groups, 100);
}

// ── What a replica does with it ──────────────────────────────────────────

/// A primary speaking the protocol by hand, one connection per call.
struct FakePrimary {
    listener: TcpListener,
    advertised: String,
}

impl FakePrimary {
    async fn new(caps: &str) -> FakePrimary {
        FakePrimary {
            listener: TcpListener::bind("127.0.0.1:0").await.unwrap(),
            advertised: format!("fake-run;caps={caps}"),
        }
    }

    fn addr(&self) -> std::net::SocketAddr {
        self.listener.local_addr().unwrap()
    }

    /// Accept a replica and read its request: (run id requested, offset).
    async fn accept(&self) -> (TcpStream, String, u64) {
        let (mut sock, _) = tokio::time::timeout(Duration::from_secs(5), self.listener.accept())
            .await
            .expect("the replica reconnects")
            .unwrap();
        sock.write_all(REPL_MAGIC).await.unwrap();
        sock.write_all(&(self.advertised.len() as u16).to_le_bytes())
            .await
            .unwrap();
        sock.write_all(self.advertised.as_bytes()).await.unwrap();
        let mut len = [0u8; 2];
        sock.read_exact(&mut len).await.unwrap();
        let mut id = vec![0u8; usize::from(u16::from_le_bytes(len))];
        sock.read_exact(&mut id).await.unwrap();
        let mut offset = [0u8; 8];
        sock.read_exact(&mut offset).await.unwrap();
        let id = String::from_utf8(id).unwrap();
        let run = id.split(';').next().unwrap_or_default().to_string();
        (sock, run, u64::from_le_bytes(offset))
    }

    /// A full sync to `boundary` of an empty keyspace, then the replica's
    /// acknowledgement of it.
    async fn full_sync(sock: &mut TcpStream, boundary: u64) {
        sock.write_all(b"F").await.unwrap();
        sock.write_all(&boundary.to_le_bytes()).await.unwrap();
        let empty = rmp_serde::to_vec(&Vec::<SnapshotEntry>::new()).unwrap();
        sock.write_all(&(empty.len() as u32).to_le_bytes())
            .await
            .unwrap();
        sock.write_all(&empty).await.unwrap();
        let mut ack = [0u8; 8];
        sock.read_exact(&mut ack).await.unwrap();
        assert_eq!(u64::from_le_bytes(ack), boundary);
    }
}

/// Best effort: a replica that refused an earlier frame may have closed the
/// link already, which is what the tests then check.
async fn send(sock: &mut TcpStream, offset: u64, bytes: &[u8]) {
    let mut out = offset.to_le_bytes().to_vec();
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
    let _ = sock.write_all(&out).await;
}

async fn send_header(sock: &mut TcpStream, count: u32, first: u64, bytes: u64) {
    let header = group_header(ReplGroup {
        first,
        count,
        bytes,
    });
    let _ = sock.write_all(&header).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_link_that_ends_inside_a_group_applies_none_of_it_and_resumes_before_it() {
    let fake = FakePrimary::new("expirylog1,txn1").await;
    let replica = node(true, false).await;
    let _following = follow_reconnecting(&replica, fake.addr());
    let (mut sock, run, offset) = fake.accept().await;
    assert_eq!((run.as_str(), offset), ("", 0));
    FakePrimary::full_sync(&mut sock, 10).await;
    send(&mut sock, 11, &frame(&["SET", "g:a", "0"])).await;
    let (a, b) = (frame(&["SET", "g:a", "1"]), frame(&["SET", "g:b", "1"]));
    send_header(&mut sock, 2, 12, (a.len() + b.len()) as u64).await;
    send(&mut sock, 12, &a).await;
    wait_for(&replica.store, "g:a", bulk("0")).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    drop(sock);
    // Nothing of the group was applied, and the resume point is before it.
    let (mut sock, run, offset) = fake.accept().await;
    assert_eq!((run.as_str(), offset), ("fake-run", 11));
    assert_eq!(replica.store.execute(Command::Get("g:a".into())), bulk("0"));
    // A partial sync carries the whole group, header first.
    sock.write_all(b"P").await.unwrap();
    sock.write_all(&13u64.to_le_bytes()).await.unwrap();
    sock.write_all(&2u32.to_le_bytes()).await.unwrap();
    send_header(&mut sock, 2, 12, (a.len() + b.len()) as u64).await;
    send(&mut sock, 12, &a).await;
    send(&mut sock, 13, &b).await;
    let mut ack = [0u8; 8];
    sock.read_exact(&mut ack).await.unwrap();
    assert_eq!(u64::from_le_bytes(ack), 13);
    wait_for(&replica.store, "g:b", bulk("1")).await;
    wait_for(&replica.store, "g:a", bulk("1")).await;
}

/// A group header to send (count, first, bytes), if any, then a frame.
type Send = (Option<(u32, u64, u64)>, u64, Vec<u8>);
/// A label, what follows the full sync, and raw bytes to send first.
type Case = (&'static str, Vec<Send>, Option<Vec<u8>>);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_groups_force_a_full_synchronization() {
    let (a, b) = (frame(&["SET", "m:a", "1"]), frame(&["SET", "m:b", "1"]));
    let len = (a.len() + b.len()) as u64;
    let dedup = frame(&["RECACHED-DEDUP", "cl", "1"]);
    let bad_version = {
        let mut h = group_header(ReplGroup {
            first: 11,
            count: 2,
            bytes: len,
        })
        .to_vec();
        h[12] = 2;
        h
    };
    // Each case: what follows the full sync to offset 10.
    let cases: Vec<Case> = vec![
        (
            "a group of one",
            vec![(Some((1, 11, a.len() as u64)), 11, a.clone())],
            None,
        ),
        (
            "a header for another offset",
            vec![(Some((2, 12, len)), 11, a.clone())],
            None,
        ),
        (
            "a header inside a group",
            vec![
                (Some((2, 11, len)), 11, a.clone()),
                (Some((2, 12, len)), 12, b.clone()),
            ],
            None,
        ),
        (
            "a member at the wrong offset",
            vec![(Some((2, 11, len)), 11, a.clone()), (None, 13, b.clone())],
            None,
        ),
        (
            "members longer than declared",
            vec![
                (Some((2, 11, len - 1)), 11, a.clone()),
                (None, 12, b.clone()),
            ],
            None,
        ),
        (
            "members shorter than declared",
            vec![
                (Some((2, 11, len + 1)), 11, a.clone()),
                (None, 12, b.clone()),
            ],
            None,
        ),
        (
            "a group of more frames than the bound",
            vec![(Some((REPL_GROUP_MAX_FRAMES + 1, 11, len)), 11, a.clone())],
            None,
        ),
        (
            "a group past the bound",
            vec![(Some((2, 11, REPL_GROUP_MAX_BYTES as u64)), 11, a.clone())],
            None,
        ),
        (
            "a DEDUP record inside",
            vec![
                (Some((2, 11, (a.len() + dedup.len()) as u64)), 11, a.clone()),
                (None, 12, dedup.clone()),
            ],
            None,
        ),
        ("an unknown header version", vec![], Some(bad_version)),
    ];
    for (label, frames, raw) in cases {
        let fake = FakePrimary::new("expirylog1,txn1").await;
        let replica = node(true, false).await;
        let _following = follow_reconnecting(&replica, fake.addr());
        let (mut sock, _, _) = fake.accept().await;
        FakePrimary::full_sync(&mut sock, 10).await;
        if let Some(raw) = raw {
            let _ = sock.write_all(&raw).await;
            send(&mut sock, 11, &a).await;
        }
        for (header, offset, bytes) in frames {
            if let Some((count, first, len)) = header {
                send_header(&mut sock, count, first, len).await;
            }
            send(&mut sock, offset, &bytes).await;
        }
        // The replica ends the link and asks for a full sync.
        let (_sock, run, offset) = fake.accept().await;
        assert_eq!((run.as_str(), offset), ("", 0), "{label}");
        assert_eq!(
            replica.store.execute(Command::Get("m:b".into())),
            nil(),
            "{label}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_header_on_a_link_without_txn1_is_a_gap() {
    let fake = FakePrimary::new("expirylog1").await;
    let replica = node(true, false).await;
    let _following = follow_reconnecting(&replica, fake.addr());
    let (mut sock, _, _) = fake.accept().await;
    FakePrimary::full_sync(&mut sock, 10).await;
    send_header(&mut sock, 2, 11, 10).await;
    let (_sock, run, offset) = fake.accept().await;
    assert_eq!((run.as_str(), offset), ("", 0));
}

// ── The four capability combinations (design §6.1) ───────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_combination_of_expirylog1_and_txn1_converges() {
    for (expirylog, txn) in [(false, false), (true, false), (false, true), (true, true)] {
        let primary = node(false, true).await;
        let hub = &primary.state.replicas;
        hub.test_old_primary.store(!expirylog, Ordering::Relaxed);
        hub.test_without_txn.store(!txn, Ordering::Relaxed);
        let replica = node(true, true).await;
        let _following = follow_reconnecting(&replica, serve_replicas(&primary).await);
        let mut client = WsClient::connect(primary.ws).await;
        client.cmd(&["SET", "c4:x", "old", "PX", "1"]).await;
        tokio::time::sleep(Duration::from_millis(5)).await;
        exec(
            &mut client,
            &[
                &["SET", "c4:a", "1"],
                &["APPEND", "c4:x", "new"],
                &["INCR", "c4:n"],
                &["SET", "c4:b", "1"],
            ],
        )
        .await;
        let label = format!("expirylog1={expirylog} txn1={txn}");
        for key in ["c4:a", "c4:x", "c4:n", "c4:b"] {
            let want = primary.store.execute(Command::Get(key.into()));
            for _ in 0..200 {
                if replica.store.execute(Command::Get(key.into())) == want {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert_eq!(
                replica.store.execute(Command::Get(key.into())),
                want,
                "{label}: {key}"
            );
        }
        assert_eq!(
            replica
                .state
                .replicas
                .upstream_expirylog
                .load(Ordering::Acquire),
            expirylog,
            "{label}"
        );
    }
}

/// Review D, D-2: offsets past `u64::MAX` are a protocol error (a full
/// sync), never a panic of the replication task.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offsets_past_u64_max_are_a_protocol_error() {
    let fake = FakePrimary::new("expirylog1,txn1").await;
    let replica = node(true, false).await;
    let _following = follow_reconnecting(&replica, fake.addr());
    let (mut sock, _, _) = fake.accept().await;
    FakePrimary::full_sync(&mut sock, u64::MAX - 1).await;
    let (a, b) = (frame(&["SET", "o:a", "1"]), frame(&["SET", "o:b", "1"]));
    send_header(&mut sock, 2, u64::MAX, (a.len() + b.len()) as u64).await;
    send(&mut sock, u64::MAX, &a).await;
    send(&mut sock, u64::MAX, &b).await;
    let (_sock, run, offset) = fake.accept().await;
    assert_eq!(
        (run.as_str(), offset),
        ("", 0),
        "a full sync, after no panic"
    );
}

/// A resume point taken from a stream applied one frame at a time is never
/// offered to the same run id once it offers `txn1`: the replica asks for a
/// full sync (review A L5, mutant M10).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_resume_point_is_offered_only_in_the_framing_mode_it_was_taken_in() {
    let mut fake = FakePrimary::new("expirylog1").await;
    let replica = node(true, false).await;
    let _following = follow_reconnecting(&replica, fake.addr());
    let (mut sock, run, _) = fake.accept().await;
    assert_eq!(run, "");
    FakePrimary::full_sync(&mut sock, 10).await;
    send(&mut sock, 11, &frame(&["SET", "fm:a", "1"])).await;
    wait_for(&replica.store, "fm:a", bulk("1")).await;
    drop(sock);
    // The same run id, unchanged otherwise: a partial resume.
    let (sock, run, offset) = fake.accept().await;
    assert_eq!((run.as_str(), offset), ("fake-run", 11));
    drop(sock);
    // The same run id, now offering txn1: a full sync.
    fake.advertised = "fake-run;caps=expirylog1,txn1".into();
    let (_sock, run, offset) = fake.accept().await;
    assert_eq!((run.as_str(), offset), ("", 11), "a full sync is asked for");
}
