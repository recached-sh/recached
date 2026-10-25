//! Unit tests: the client against hand-written server frames, no socket.
//! `tests/live.rs` covers the same paths against a real server.

use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// A database path removed, with its WAL files, when dropped.
struct TempDb(PathBuf);

impl TempDb {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("recached-mobile-{}-{n}.db", std::process::id()));
        let db = Self(path);
        db.cleanup();
        db
    }

    fn open(&self) -> Arc<RecachedClient> {
        RecachedClient::open(
            self.0.to_string_lossy().into_owned(),
            ClientConfig::default(),
        )
        .expect("open")
    }

    fn cleanup(&self) {
        for suffix in ["", "-wal", "-shm"] {
            let mut p = self.0.clone().into_os_string();
            p.push(suffix);
            let _ = std::fs::remove_file(p);
        }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// A sink that records what the client sends.
#[derive(Default)]
struct Recorder(Mutex<Vec<Vec<u8>>>);

impl FrameSink for Recorder {
    fn send(&self, frame: Vec<u8>) {
        self.0.lock().unwrap().push(frame);
    }
}

impl Recorder {
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap())
            .into_iter()
            .map(|f| String::from_utf8(f).unwrap())
            .collect()
    }
}

fn connect(client: &RecachedClient) -> Arc<Recorder> {
    let sink = Arc::new(Recorder::default());
    client.connection_opened(sink.clone());
    sink
}

fn s(text: &str) -> String {
    text.to_string()
}

const OK: &[u8] = b"+OK\r\n";

fn keychange(key: &str, value: &str) -> Vec<u8> {
    to_resp(&["keychange", key, value])
}

/// Answer the four `CLIENT` opt-ins every open starts with, as a 0.4 server
/// does: `DELTA` and `EXPIRY` accepted, `PUSHFRAMES` and `DEDUPBARRIER`
/// refused — so keychanges stay the plain arrays these tests write.
fn ack_opt_ins(client: &RecachedClient) {
    for reply in [
        OK,
        OK,
        b"-ERR unknown subcommand 'PUSHFRAMES'\r\n",
        b"-ERR unknown subcommand 'DEDUPBARRIER'\r\n",
    ] {
        client.frame_received(reply.to_vec()).unwrap();
    }
}

/// Frames every open sends before the session's sync token and the outbox.
const OPT_INS: usize = 4;

/// The `DEDUP <client> <wire-id>` envelope of a sent write.
fn dedup_header(frame: &str) -> (String, u64) {
    let parts: Vec<&str> = frame.split("\r\n").collect();
    assert_eq!(parts[2], "DEDUP", "not a dedup frame: {frame:?}");
    (parts[4].to_string(), parts[6].parse().unwrap())
}

// ── durability ────────────────────────────────────────────────────────────────

#[test]
fn offline_writes_survive_a_restart_with_their_values() {
    let db = TempDb::new();
    {
        let c = db.open();
        c.set(s("k"), b"v".to_vec()).unwrap();
        assert_eq!(c.incr_by(s("n"), 2).unwrap(), 2);
        c.jset(s("doc"), s("$"), s(r#"{"a":1}"#)).unwrap();
        assert_eq!(c.pending_writes(), 3);
    }
    let c = db.open();
    assert_eq!(c.get(s("k")), Some(b"v".to_vec()));
    assert_eq!(c.get_string(s("n")).unwrap().as_deref(), Some("2"));
    assert_eq!(c.get_json(s("doc"), None).as_deref(), Some(r#"{"a":1}"#));
    assert_eq!(c.pending_writes(), 3, "the queued writes came back too");
}

#[test]
fn a_delete_is_durable() {
    let db = TempDb::new();
    {
        let c = db.open();
        c.set(s("k"), b"v".to_vec()).unwrap();
        assert!(c.del(s("k")).unwrap());
    }
    assert!(!db.open().exists(s("k")));
}

#[test]
fn a_refused_command_changes_and_queues_nothing() {
    let db = TempDb::new();
    let c = db.open();
    c.set(s("text"), b"abc".to_vec()).unwrap();
    let err = c.incr_by(s("text"), 1).unwrap_err();
    assert!(matches!(err, RecachedError::Command { .. }), "{err:?}");
    assert_eq!(c.pending_writes(), 1);
}

#[test]
fn binary_values_have_no_string_form() {
    let db = TempDb::new();
    let c = db.open();
    c.set(s("bin"), vec![0xff, 0x00]).unwrap();
    assert_eq!(c.get(s("bin")), Some(vec![0xff, 0x00]));
    assert!(matches!(
        c.get_string(s("bin")),
        Err(RecachedError::NotUtf8 { .. })
    ));
}

// ── sync ──────────────────────────────────────────────────────────────────────

#[test]
fn queued_writes_follow_the_session_on_connect() {
    let db = TempDb::new();
    let c = db.open();
    c.watch(s("cart:*"));
    c.set(s("a"), b"1".to_vec()).unwrap();

    let sent = connect(&c).take();
    assert!(sent[0].contains("DELTA"), "{sent:?}");
    assert!(sent[1].contains("EXPIRY"), "{sent:?}");
    assert!(sent[2].contains("PUSHFRAMES"), "{sent:?}");
    assert!(sent[3].contains("DEDUPBARRIER"), "{sent:?}");
    // Writes before live queries, so the snapshot already contains them.
    assert!(sent[OPT_INS].contains("SET"), "{sent:?}");
    assert!(sent[OPT_INS + 1].contains("QSUB"), "{sent:?}");
    assert_eq!(sent.len(), OPT_INS + 2);
}

#[test]
fn writes_go_out_at_once_while_connected_and_not_after_close() {
    let db = TempDb::new();
    let c = db.open();
    let sink = connect(&c);
    sink.take();

    c.set(s("a"), b"1".to_vec()).unwrap();
    assert_eq!(sink.take().len(), 1);

    let delay = c.connection_closed();
    assert!(delay > 0);
    c.set(s("b"), b"2".to_vec()).unwrap();
    assert!(sink.take().is_empty(), "nothing is sent on a closed socket");
}

#[test]
fn an_acknowledged_write_leaves_the_durable_outbox() {
    let db = TempDb::new();
    {
        let c = db.open();
        let _sink = connect(&c);
        c.set(s("a"), b"1".to_vec()).unwrap();
        ack_opt_ins(&c);
        assert_eq!(c.pending_writes(), 1);
        c.frame_received(OK.to_vec()).unwrap(); // SET
        assert_eq!(c.pending_writes(), 0);
    }
    assert_eq!(db.open().pending_writes(), 0);
}

#[test]
fn a_restart_replays_with_the_same_identity_and_original_wire_ids() {
    // The exactly-once argument: a write the server applied before the app
    // died is replayed under its original wire id, which the server has
    // already seen and skips. New writes use a higher epoch.
    let db = TempDb::new();
    let first = {
        let c = db.open();
        c.set(s("a"), b"1".to_vec()).unwrap();
        let sent = connect(&c).take();
        dedup_header(sent.last().unwrap())
    };

    let c = db.open();
    let sink = connect(&c);
    let replayed = dedup_header(sink.take().last().unwrap());
    assert_eq!(replayed, first, "the replay is byte-identical");

    c.set(s("b"), b"2".to_vec()).unwrap();
    let (client, wire) = dedup_header(&sink.take()[0]);
    assert_eq!(client, first.0, "the client id persists");
    assert!(
        wire >> 32 > first.1 >> 32,
        "a new session has a higher epoch"
    );
}

#[test]
fn server_changes_are_persisted_and_reported() {
    let db = TempDb::new();
    {
        let c = db.open();
        let outcome = c.frame_received(keychange("x", "1")).unwrap();
        assert_eq!(outcome.changed_keys, vec![s("x")]);
        assert!(!outcome.reconnect);
    }
    let c = db.open();
    assert_eq!(c.get(s("x")), Some(b"1".to_vec()));

    // A nil value is a deletion. `to_resp` cannot encode nil, so by hand.
    let frame = b"*3\r\n$9\r\nkeychange\r\n$1\r\nx\r\n$-1\r\n".to_vec();
    assert_eq!(c.frame_received(frame).unwrap().changed_keys, vec![s("x")]);
    drop(c);
    assert!(!db.open().exists(s("x")), "the deletion was saved too");
}

#[test]
fn a_key_deleted_while_offline_is_removed_on_reconnect_and_on_disk() {
    let db = TempDb::new();
    let c = db.open();
    c.watch(s("cart:*"));
    connect(&c);
    ack_opt_ins(&c);
    c.frame_received(to_resp(&[
        "qstate", "cart:*", "cart:1", "apple", "cart:2", "pear",
    ]))
    .unwrap();
    c.connection_closed();

    connect(&c);
    ack_opt_ins(&c);
    let mut outcome = c
        .frame_received(to_resp(&["qstate", "cart:*", "cart:1", "apple"]))
        .unwrap();
    outcome.changed_keys.sort();
    assert_eq!(outcome.changed_keys, vec![s("cart:1"), s("cart:2")]);
    drop(c);

    let c = db.open();
    assert!(c.exists(s("cart:1")));
    assert!(!c.exists(s("cart:2")));
}

#[test]
fn an_unparseable_frame_asks_for_a_reconnect() {
    let db = TempDb::new();
    let c = db.open();
    connect(&c);
    let outcome = c.frame_received(b"*not resp".to_vec()).unwrap();
    assert!(outcome.reconnect);
}

#[test]
fn get_matching_is_sorted_and_marks_collections() {
    let db = TempDb::new();
    let c = db.open();
    c.set(s("p:2"), b"b".to_vec()).unwrap();
    c.set(s("p:1"), b"a".to_vec()).unwrap();
    // A set arrives type-tagged; it has no single value.
    c.frame_received(
        b"*3\r\n$9\r\nkeychange\r\n$3\r\np:3\r\n*2\r\n$3\r\nset\r\n$1\r\nm\r\n".to_vec(),
    )
    .unwrap();
    let found = c.get_matching(s("p:*"));
    assert_eq!(
        found,
        vec![
            Entry {
                key: s("p:1"),
                value: Some(b"a".to_vec())
            },
            Entry {
                key: s("p:2"),
                value: Some(b"b".to_vec())
            },
            Entry {
                key: s("p:3"),
                value: None
            },
        ]
    );
}

#[test]
fn any_key_matches_uses_the_engine_glob_rules() {
    let keys = |ks: &[&str]| ks.iter().map(|k| s(k)).collect::<Vec<_>>();
    assert!(any_key_matches(s("cart:*"), keys(&["user:1", "cart:9"])));
    assert!(!any_key_matches(s("cart:*"), keys(&["user:1"])));
    assert!(any_key_matches(s("user:?"), keys(&["user:7"])));
    // The engine has no character classes: brackets match literally, on the
    // server too, so routing must not be cleverer than the server.
    assert!(!any_key_matches(s("tag:[ab]"), keys(&["tag:b"])));
    assert!(any_key_matches(s("tag:[ab]"), keys(&["tag:[ab]"])));
    assert!(!any_key_matches(s("cart:*"), Vec::new()));
}

// ── refused writes ────────────────────────────────────────────────────────────

const READ_ONLY: &[u8] = b"-NOSCOPE key 'a' is read-only on this connection\r\n";
const NO_TOKEN: &[u8] = b"-NOSCOPE send SYNC TOKEN <token> before issuing commands\r\n";

#[test]
fn a_write_refused_for_good_is_reported_and_leaves_the_durable_outbox() {
    let db = TempDb::new();
    {
        let c = db.open();
        let _sink = connect(&c);
        ack_opt_ins(&c);
        c.set(s("a"), b"1".to_vec()).unwrap();
        let outcome = c.frame_received(READ_ONLY.to_vec()).unwrap();
        assert_eq!(
            outcome.refused,
            Some(RefusedWrite {
                keys: vec![s("a")],
                unreconciled: vec![s("a")],
                reason: s("NOSCOPE key 'a' is read-only on this connection"),
            })
        );
        assert!(!outcome.reconnect);
        assert_eq!(c.pending_writes(), 0);
    }
    assert_eq!(
        db.open().pending_writes(),
        0,
        "replaying it would be refused again"
    );
}

#[test]
fn a_refused_write_to_a_watched_key_asks_for_a_fresh_snapshot() {
    let db = TempDb::new();
    let c = db.open();
    c.watch(s("a*"));
    let sink = connect(&c);
    ack_opt_ins(&c);
    c.frame_received(to_resp(&["qstate", "a*", "a", "server"]))
        .unwrap();
    c.set(s("a"), b"local".to_vec()).unwrap();
    sink.take();

    c.frame_received(READ_ONLY.to_vec()).unwrap();
    let sent = sink.take();
    assert_eq!(sent.len(), 1);
    assert!(
        sent[0].contains("QSUB") && sent[0].contains("a*"),
        "{sent:?}"
    );

    let outcome = c
        .frame_received(to_resp(&["qstate", "a*", "a", "server"]))
        .unwrap();
    assert_eq!(outcome.changed_keys, vec![s("a")]);
    assert_eq!(c.get(s("a")), Some(b"server".to_vec()));
}

#[test]
fn a_write_deferred_by_the_server_stays_queued_across_a_restart() {
    let db = TempDb::new();
    {
        let c = db.open();
        c.set(s("a"), b"offline".to_vec()).unwrap();
        let sink = connect(&c);
        ack_opt_ins(&c);
        sink.take();
        let outcome = c.frame_received(NO_TOKEN.to_vec()).unwrap();
        assert!(outcome.reconnect, "the reconnect replays it");
        assert_eq!(outcome.refused, None);
        assert_eq!(c.pending_writes(), 1);

        // Nothing more goes out on the socket being abandoned.
        c.set(s("b"), b"2".to_vec()).unwrap();
        assert!(sink.take().is_empty());
        c.connection_closed();
    }
    let c = db.open();
    assert_eq!(c.pending_writes(), 2);
    let sent = connect(&c).take();
    let writes: Vec<&String> = sent
        .iter()
        .filter(|f| f.contains("$5\r\nDEDUP\r\n"))
        .collect();
    assert_eq!(writes.len(), 2, "{sent:?}");
    assert!(writes[0].contains("offline"), "replayed in order: {sent:?}");
}

#[test]
fn set_ex_refuses_a_zero_ttl_the_server_would_refuse() {
    let db = TempDb::new();
    let c = db.open();
    let err = c.set_ex(s("k"), b"v".to_vec(), 0).unwrap_err();
    assert!(matches!(err, RecachedError::Command { .. }), "{err:?}");
    assert_eq!(c.pending_writes(), 0);
    assert!(!c.exists(s("k")));
}

// ── expiry ────────────────────────────────────────────────────────────────────

fn expiring_keychange(key: &str, value: &str, ms: i64) -> Vec<u8> {
    Value::Array(Some(vec![
        Value::BulkString(Some(b"keychange".to_vec())),
        Value::BulkString(Some(key.as_bytes().to_vec())),
        Value::Array(Some(vec![
            Value::BulkString(Some(b"px".to_vec())),
            Value::Integer(ms),
            Value::BulkString(Some(value.as_bytes().to_vec())),
        ])),
    ]))
    .serialize()
}

#[test]
fn the_echo_of_an_expiring_write_keeps_its_ttl_on_disk_too() {
    let db = TempDb::new();
    {
        let c = db.open();
        let _sink = connect(&c);
        c.watch(s("k*"));
        c.set_ex(s("k1"), b"v".to_vec(), 600).unwrap();
        c.frame_received(expiring_keychange("k1", "v", 599_990))
            .unwrap();
        let ttl = c.ttl(s("k1"));
        assert!((590..=600).contains(&ttl), "{ttl}");
    }
    let ttl = db.open().ttl(s("k1"));
    assert!((590..=600).contains(&ttl), "after reopening: {ttl}");
}

// ── v0.5 reliability ──────────────────────────────────────────────────────────

fn open_with(db: &TempDb, config: ClientConfig) -> Arc<RecachedClient> {
    RecachedClient::open(db.0.to_string_lossy().into_owned(), config).expect("open")
}

fn capped(max: u32) -> ClientConfig {
    ClientConfig {
        max_pending_writes: Some(max),
        ..ClientConfig::default()
    }
}

#[test]
fn test_01_a_full_outbox_refuses_the_write_before_anything_changes() {
    let db = TempDb::new();
    let c = open_with(&db, capped(2));
    c.set(s("a"), b"1".to_vec()).unwrap();
    c.set(s("b"), b"1".to_vec()).unwrap();
    assert!(matches!(
        c.set(s("c"), b"1".to_vec()),
        Err(RecachedError::OutboxFull {
            pending: 2,
            capacity: 2
        })
    ));
    assert!(!c.exists(s("c")), "the refused write did not apply locally");
    assert!(matches!(
        c.incr_by(s("a"), 1),
        Err(RecachedError::OutboxFull { .. })
    ));
    assert_eq!(
        c.get(s("a")),
        Some(b"1".to_vec()),
        "nor touched a queued key"
    );
    assert_eq!(c.sync_counters().rejected_writes, 2);
    drop(c);
    let c = open_with(&db, capped(2));
    assert_eq!(
        c.pending_writes(),
        2,
        "the queued writes are intact on disk"
    );
    assert!(!c.exists(s("c")));
}

#[test]
fn test_10_a_failed_sqlite_transaction_returns_an_error_and_leaves_nothing_behind() {
    let db = TempDb::new();
    let c = db.open();
    let sink = connect(&c);
    ack_opt_ins(&c);
    c.set(s("k"), b"before".to_vec()).unwrap();
    sink.take();
    c.frame_received(OK.to_vec()).unwrap();

    c.lock().db.break_outbox_for_test();
    assert!(matches!(
        c.set(s("k"), b"after".to_vec()),
        Err(RecachedError::Storage { .. })
    ));
    // Not in memory, not queued, not sent.
    assert_eq!(c.get(s("k")), Some(b"before".to_vec()));
    assert_eq!(c.pending_writes(), 0);
    assert!(
        sink.take().is_empty(),
        "an uncommitted write never goes out"
    );
    // A key the failed write would have created is not left behind either.
    assert!(c.set(s("new"), b"x".to_vec()).is_err());
    assert!(!c.exists(s("new")));

    c.lock().db.repair_outbox_for_test();
    c.set(s("k"), b"later".to_vec()).unwrap();
    assert_eq!(
        sink.take().len(),
        1,
        "the client still sends after recovering"
    );
    drop(c);
    let c = db.open();
    assert_eq!(c.get(s("k")), Some(b"later".to_vec()));
}

#[test]
fn a_restart_with_more_rows_than_the_new_cap_keeps_them_all() {
    let db = TempDb::new();
    {
        let c = open_with(&db, capped(10));
        for i in 0..5 {
            c.set(format!("k{i}"), b"v".to_vec()).unwrap();
        }
    }
    let c = open_with(&db, capped(2));
    assert_eq!(c.pending_writes(), 5);
    assert!(matches!(
        c.set(s("new"), b"v".to_vec()),
        Err(RecachedError::OutboxFull {
            pending: 5,
            capacity: 2
        })
    ));
    let sent = connect(&c).take();
    assert_eq!(
        sent.iter()
            .filter(|f| f.contains("$5\r\nDEDUP\r\n"))
            .count(),
        5
    );
}

#[test]
fn an_exhausted_epoch_takes_a_new_identity_instead_of_reusing_ids() {
    let db = TempDb::new();
    let old_identity = {
        let c = db.open();
        c.set(s("a"), b"1".to_vec()).unwrap();
        let identity = c.lock().sync.client_id().to_string();
        c.lock()
            .db
            .meta_put("epoch", &u32::MAX.to_string())
            .unwrap();
        identity
    };
    let c = db.open();
    let new_identity = c.lock().sync.client_id().to_string();
    assert_ne!(new_identity, old_identity);
    assert_eq!(c.lock().sync.epoch(), 1);
    c.set(s("b"), b"1".to_vec()).unwrap();
    let sent = connect(&c).take();
    let writes: Vec<(String, u64)> = sent
        .iter()
        .filter(|f| f.contains("$5\r\nDEDUP\r\n"))
        .map(|f| dedup_header(f))
        .collect();
    // The queued row keeps the identity it was written under.
    assert_eq!(writes[0].0, old_identity);
    assert_eq!(writes[1], (new_identity, 1u64 << 32 | 1));
}

#[test]
fn the_status_reports_each_phase_once_and_not_ready_on_open() {
    let db = TempDb::new();
    let c = db.open();
    c.watch(s("cart:*"));
    c.set(s("cart:1"), b"x".to_vec()).unwrap();
    assert_eq!(
        c.take_status_change().map(|s| s.phase),
        Some(SyncPhase::Disconnected)
    );
    c.connection_opening();
    assert_eq!(
        c.take_status_change().map(|s| s.phase),
        Some(SyncPhase::Connecting)
    );
    connect(&c);
    assert_eq!(
        c.take_status_change().map(|s| s.phase),
        Some(SyncPhase::Authenticating),
        "an open socket is not synchronized"
    );
    let phases: Vec<Option<SyncPhase>> = [
        OK.to_vec(),
        OK.to_vec(),
        b"-ERR unknown subcommand 'PUSHFRAMES'\r\n".to_vec(),
        b"-ERR unknown subcommand 'DEDUPBARRIER'\r\n".to_vec(),
        OK.to_vec(),
        to_resp(&["qstate", "cart:*", "cart:1", "x"]),
    ]
    .into_iter()
    .map(|f| c.frame_received(f).unwrap().status.map(|s| s.phase))
    .collect();
    assert_eq!(
        phases,
        [
            None,
            None,
            None,
            Some(SyncPhase::Replaying),
            Some(SyncPhase::Hydrating),
            Some(SyncPhase::Ready)
        ]
    );
    assert_eq!(
        c.subscription_state(s("cart:*")),
        Some(SubscriptionState::Current)
    );
    c.connection_closed();
    assert_eq!(
        c.take_status_change().map(|s| s.phase),
        Some(SyncPhase::Disconnected)
    );
}

#[test]
fn test_14_an_oversized_snapshot_degrades_and_keeps_local_rows() {
    let db = TempDb::new();
    let c = db.open();
    c.watch(s("big:*"));
    c.set(s("big:1"), b"mine".to_vec()).unwrap();
    connect(&c);
    ack_opt_ins(&c);
    c.frame_received(OK.to_vec()).unwrap(); // the write
    let outcome = c
        .frame_received(
            b"-ERR live query initial state exceeds 10000 keys; narrow the pattern\r\n".to_vec(),
        )
        .unwrap();
    assert_eq!(outcome.status.map(|s| s.phase), Some(SyncPhase::Degraded));
    assert!(matches!(
        c.subscription_state(s("big:*")),
        Some(SubscriptionState::Failed {
            failure: SubscriptionFailure::SnapshotTooLarge,
            ..
        })
    ));
    assert!(c.exists(s("big:1")));
    drop(c);
    assert!(db.open().exists(s("big:1")), "and on disk");
}

#[test]
fn an_interrupted_open_never_reuses_an_epoch() {
    // The epoch is made durable before any write can use it, so a crash
    // right after `open` still moves the next session past it.
    let db = TempDb::new();
    let first = db.open().lock().sync.epoch();
    let second = db.open().lock().sync.epoch();
    assert!(second > first);
}
