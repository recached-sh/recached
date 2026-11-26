//! 6O-H3: the AOF expiry-log format and timeless replay
//! (`benchmarks/v0.6.0/6o-h/V0.6O-H-PROTOCOL-DESIGN.md`, revision 2.1).
//!
//! Each test boots nodes the way `main` does: load the snapshot, replay the
//! AOF (`replay_aof_format`), take it over (`start_expiry_log`), serve. A
//! "restart" is a new boot on the same files, with nothing flushed beyond
//! what the writer made durable.

use super::mset_atomicity::{Pipe, bulk};
use super::request_path::Shared;
use crate::*;
use core_engine::resp::Value;
use core_engine::store::{EvictionPolicy, KeyValueStore};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn ok() -> Value {
    Value::SimpleString("OK".into())
}

fn nil() -> Value {
    Value::BulkString(None)
}

/// A fresh directory, removed when dropped.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let dir = std::env::temp_dir().join(format!(
            "recached_6oh_aof_{name}_{}_{}",
            std::process::id(),
            next_conn_id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Dir(dir)
    }
    fn aof(&self) -> PathBuf {
        self.0.join("append.aof")
    }
    fn snap(&self) -> PathBuf {
        self.0.join("dump.rdb")
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A node booted on `dir` as `main` boots one.
async fn boot_with(dir: &Dir, primary: bool, store: KeyValueStore) -> std::io::Result<Shared> {
    let loaded = load_snapshot_checkpoint(&store, &dir.snap()).await?;
    let aof = AofWriter::open(dir.aof(), AofSync::Always).await?;
    let replayed = replay_aof_format(&store, &dir.aof(), loaded.aof_checkpoint, primary).await?;
    start_expiry_log(&aof, &replayed, primary).await?;
    let mut shared = Shared::with_aof(false, Some(Arc::new(aof)));
    shared.store = Arc::new(store);
    let mut state = server_state_for(&shared, dir, loaded.aof_checkpoint);
    state.is_replica = AtomicBool::new(!primary);
    shared.state = Arc::new(state);
    Ok(shared)
}

async fn boot(dir: &Dir) -> Shared {
    boot_with(dir, true, KeyValueStore::new()).await.unwrap()
}

/// `shared`'s server state with its own snapshot file and checkpoint.
fn server_state_for(shared: &Shared, dir: &Dir, checkpoint: u64) -> ServerState {
    ServerState {
        snap: Arc::new(SnapshotConfig {
            path: dir.snap(),
            last_save: AtomicI64::new(now_unix_secs()),
            checkpoint_id: AtomicU64::new(checkpoint),
        }),
        aof: shared.state.aof.clone(),
        replicas: ReplHub::new(),
        is_replica: AtomicBool::new(false),
        dedup: std::sync::Mutex::new(HashMap::new()),
        ephemeral: std::sync::Mutex::new(HashMap::new()),
        ephemeral_members: std::sync::Mutex::new(HashMap::new()),
        ephemeral_in_use: AtomicBool::new(false),
        dedup_dirty: AtomicBool::new(false),
        dedup_order: tokio::sync::Mutex::new(()),
        save_lock: tokio::sync::Mutex::new(()),
        persistence_healthy: AtomicBool::new(true),
        persistence_failures: AtomicU64::new(0),
    }
}

/// The AOF's records: whether each is an array (`*`), and its words.
fn records(path: &Path) -> Vec<(bool, Vec<String>)> {
    let bytes = std::fs::read(path).unwrap_or_default();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let array = bytes[at] == b'*';
        let (value, used) = Value::parse(&bytes[at..]).expect("whole records");
        at += used;
        let items = match value {
            Value::Push(items) | Value::Array(Some(items)) => items,
            other => panic!("not a record: {other:?}"),
        };
        out.push((
            array,
            items
                .into_iter()
                .map(|item| match item {
                    Value::BulkString(Some(b)) => String::from_utf8_lossy(&b).into_owned(),
                    other => format!("{other:?}"),
                })
                .collect(),
        ));
    }
    out
}

const FORMAT_ID: &str = "17005593296757587969";

fn is_format(record: &(bool, Vec<String>)) -> bool {
    record.1 == ["RECACHED-CHECKPOINT", FORMAT_ID]
}

fn push(words: &[&str]) -> Vec<u8> {
    let parts: Vec<&[u8]> = words.iter().map(|w| w.as_bytes()).collect();
    resp_push(&parts)
}

fn array(words: &[&str]) -> Vec<u8> {
    let mut frame = push(words);
    frame[0] = b'*';
    frame
}

fn format_record() -> Vec<u8> {
    array(&["RECACHED-CHECKPOINT", FORMAT_ID])
}

// ── The encoding ─────────────────────────────────────────────────────────

#[test]
fn control_ids_are_the_designs() {
    let format = Control::Format {
        features: FEATURE_EXPIRY_LOG,
    };
    assert_eq!(format.id().to_string(), FORMAT_ID);
    assert_eq!(format.id(), 0xec00_0101_0000_0001);
    assert_eq!(Control::of(format.id()), Some(format));
    assert_eq!(control_frame(format).len(), 57);
    // Not a control record: every checkpoint id a save can reach.
    for id in [0, 1, 42, u64::from(u32::MAX), (0xEC00 << 48) - 1, u64::MAX] {
        assert!(!is_control_id(id) || id >> 48 == 0xEC00, "{id}");
    }
    assert_eq!(Control::of(1), None);
    // Unknown kinds: 0x80 and up ignorable, the rest not; unknown layout not.
    let id = |kind: u64, layout: u64| (0xEC00 << 48) | (kind << 40) | (layout << 32);
    assert_eq!(
        Control::of(id(2, 1)),
        Some(Control::Unknown { ignorable: false })
    );
    assert_eq!(
        Control::of(id(0x7F, 1)),
        Some(Control::Unknown { ignorable: false })
    );
    assert_eq!(
        Control::of(id(0x80, 1)),
        Some(Control::Unknown { ignorable: true })
    );
    assert_eq!(
        Control::of(id(1, 2)),
        Some(Control::Unknown { ignorable: false })
    );
}

// ── Writing the format ───────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_primary_begins_a_new_file_in_the_format_and_writes_arrays() {
    let dir = Dir::new("new");
    let node = boot(&dir).await;
    let mut c = Pipe::tcp(&node);
    c.cmd(&["SET", "a", "1"]).await;
    c.cmd(&["INCR", "n"]).await;
    let recs = records(&dir.aof());
    assert!(is_format(&recs[0]), "{recs:?}");
    assert!(
        recs.iter().all(|(array, _)| *array),
        "every record an array: {recs:?}"
    );
    assert_eq!(recs.len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_replica_leaves_an_empty_file_unmarked() {
    let dir = Dir::new("replica");
    let node = boot_with(&dir, false, KeyValueStore::new()).await.unwrap();
    node.state
        .aof
        .as_ref()
        .unwrap()
        .append(&push(&["SET", "a", "1"]))
        .await
        .unwrap();
    let recs = records(&dir.aof());
    assert_eq!(
        recs,
        vec![(false, vec!["SET".into(), "a".into(), "1".into()])]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_append_path_writes_arrays_in_the_format() {
    // Review C, R2-5: the plain append, a [DEL][frame] pair, DEDUP (applied,
    // no-op, refused), EXEC's batch, the sweeper, the checkpoint's FORMAT,
    // seed and seed deadline, and the startup discard.
    let dir = Dir::new("paths");
    // Kept open past the restart below, so its claim is in the files and no
    // disconnect releases it.
    let held;
    {
        let node = boot(&dir).await;
        let mut c = Pipe::tcp(&node);
        let mut owner = Pipe::tcp(&node);
        c.cmd(&["SET", "p:a", "1"]).await;
        c.cmd(&["PEXPIREAT", "p:a", "1"]).await;
        c.cmd(&["INCR", "p:a"]).await; // [DEL][frame]
        c.cmd(&["DEDUP", "cl", "1", "INCR", "p:d"]).await;
        c.cmd(&["DEDUP", "cl", "2", "HDEL", "p:h", "f"]).await;
        c.cmd(&["DEDUP", "cl", "3", "LSET", "p:none", "0", "x"])
            .await;
        c.cmd(&["MULTI"]).await;
        c.cmd(&["SET", "p:t", "1"]).await;
        c.cmd(&["APPEND", "p:t", "2"]).await;
        c.cmd(&["EXEC"]).await;
        c.cmd(&["SET", "p:sw", "v", "PX", "1"]).await;
        tokio::time::sleep(Duration::from_millis(5)).await;
        sweep_expired_tick(&node.store, &node.state, &node.watch_registry, &node.tx).await;
        assert_eq!(owner.cmd(&["ESET", "p:e", "v"]).await, ok());
        owner.cmd(&["PEXPIRE", "p:e", "600000"]).await;
        assert_eq!(c.cmd(&["SAVE"]).await, ok());
        owner.cmd(&["SET", "p:after", "1"]).await;
        let recs = records(&dir.aof());
        assert!(is_format(&recs[0]), "{recs:?}");
        assert!(recs.iter().all(|(array, _)| *array), "{recs:?}");
        let words: Vec<_> = recs.iter().map(|(_, w)| w[0].as_str()).collect();
        assert_eq!(
            words[1..4],
            ["ESET", "PEXPIREAT", "SET"],
            "seed, its deadline: {words:?}"
        );
        held = owner;
    }
    // The restart discards the abandoned ephemeral key, and logs it.
    let _node = boot(&dir).await;
    let recs = records(&dir.aof());
    assert_eq!(recs.last().unwrap().1, ["DEL", "p:e"], "{recs:?}");
    assert!(recs.iter().all(|(array, _)| *array), "{recs:?}");
    drop(held);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_legacy_file_stays_legacy_until_its_checkpoint() {
    let dir = Dir::new("legacy");
    std::fs::write(dir.aof(), [push(&["SET", "a", "1"])].concat()).unwrap();
    let node = boot(&dir).await;
    let mut c = Pipe::tcp(&node);
    c.cmd(&["SET", "b", "2"]).await;
    let recs = records(&dir.aof());
    assert_eq!(recs.len(), 2, "no FORMAT written into the middle: {recs:?}");
    assert!(recs.iter().all(|(array, _)| !array), "pushes: {recs:?}");
    assert_eq!(c.cmd(&["SAVE"]).await, ok());
    c.cmd(&["SET", "c", "3"]).await;
    let recs = records(&dir.aof());
    assert!(
        is_format(&recs[0]),
        "the checkpoint begins the format: {recs:?}"
    );
    assert!(recs.iter().all(|(array, _)| *array), "{recs:?}");
    drop(node);
    let node = boot(&dir).await;
    let mut c = Pipe::tcp(&node);
    for (k, v) in [("a", "1"), ("b", "2"), ("c", "3")] {
        assert_eq!(c.cmd(&["GET", k]).await, bulk(v));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_torn_format_record_leaves_an_empty_file_that_begins_again() {
    let dir = Dir::new("torn");
    std::fs::write(dir.aof(), &format_record()[..20]).unwrap();
    let node = boot(&dir).await;
    Pipe::tcp(&node).cmd(&["SET", "a", "1"]).await;
    let recs = records(&dir.aof());
    assert!(is_format(&recs[0]) && recs.len() == 2, "{recs:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_snapshot_naming_a_reserved_checkpoint_id_is_refused() {
    let dir = Dir::new("reserved");
    let cfg = SnapshotConfig {
        path: dir.snap(),
        last_save: AtomicI64::new(0),
        checkpoint_id: AtomicU64::new(0),
    };
    save_snapshot(&KeyValueStore::new(), &cfg, 0xEC00 << 48)
        .await
        .unwrap();
    let Err(err) = load_snapshot_checkpoint(&KeyValueStore::new(), &dir.snap()).await else {
        panic!("loaded a snapshot naming a reserved id");
    };
    assert!(err.to_string().contains("reserved"), "{err}");
}

// ── Replaying it ─────────────────────────────────────────────────────────

/// Replay `bytes` as an AOF into a fresh store (no snapshot).
async fn replay_bytes(bytes: &[u8]) -> KeyValueStore {
    let dir = Dir::new("bytes");
    std::fs::write(dir.aof(), bytes).unwrap();
    let store = KeyValueStore::new();
    replay_aof_format(&store, &dir.aof(), 0, true)
        .await
        .unwrap();
    store
}

fn past() -> String {
    (now_unix_ms() - 1_000).to_string()
}

/// The 6O-F counter: written with a deadline, counted, replayed after it.
fn counter(enc: fn(&[&str]) -> Vec<u8>, deadline: &str) -> Vec<u8> {
    [
        enc(&["SET", "c", "5", "PXAT", deadline]),
        enc(&["SET", "c", "6", "KEEPTTL"]),
    ]
    .concat()
}

fn pexpiretime(store: &KeyValueStore, key: &str) -> Value {
    store.execute(Command::PExpireTime(key.into()))
}

#[tokio::test]
async fn a_format_file_replays_on_the_primarys_timeline() {
    let t = past();
    let store = replay_bytes(&[format_record(), counter(array, &t)].concat()).await;
    assert_eq!(
        store.execute(Command::Get("c".into())),
        nil(),
        "expired, not permanent"
    );
    assert_eq!(store.key_count(), 1, "kept until a DEL removes it");
    // A legacy file of the same records resurrects it, as before.
    let store = replay_bytes(&counter(push, &t)).await;
    assert_eq!(store.execute(Command::Get("c".into())), bulk("6"));
    assert_eq!(pexpiretime(&store, "c"), Value::Integer(-1));
}

#[tokio::test]
async fn records_an_older_server_appended_replay_by_its_rules() {
    // The older server swept `s` silently and then created it again with
    // APPEND: its push replays with expiry evaluated, as it ran (`s = "new"`),
    // not merged onto the kept entry (`"oldnew"`).
    let t = past();
    let bytes = [
        format_record(),
        array(&["SET", "s", "old", "PXAT", &t]),
        push(&["APPEND", "s", "new"]),
        array(&["APPEND", "s", "!"]), // after a push: legacy too
    ]
    .concat();
    let store = replay_bytes(&bytes).await;
    assert_eq!(store.execute(Command::Get("s".into())), bulk("new!"));
    assert_eq!(pexpiretime(&store, "s"), Value::Integer(-1));
}

#[tokio::test]
async fn control_records_of_unknown_kinds_end_the_timeless_part_unless_ignorable() {
    let t = past();
    let control = |kind: u64| {
        let id = ((0xEC00u64 << 48) | (kind << 40) | (1 << 32)).to_string();
        array(&["RECACHED-CHECKPOINT", &id])
    };
    for (kind, timeless) in [(0x80, true), (0xFF, true), (0x05, false), (0x7F, false)] {
        let bytes = [
            format_record(),
            array(&["SET", "c", "5", "PXAT", &t]),
            control(kind),
            array(&["SET", "c", "6", "KEEPTTL"]),
        ]
        .concat();
        let store = replay_bytes(&bytes).await;
        let got = store.execute(Command::Get("c".into()));
        assert_eq!(got == nil(), timeless, "kind {kind:#x}: {got:?}");
    }
}

#[tokio::test]
async fn only_a_head_format_with_understood_features_makes_a_file_timeless() {
    let t = past();
    let must = {
        let id = (Control::Format {
            features: FEATURE_EXPIRY_LOG | 1 << 16,
        })
        .id()
        .to_string();
        array(&["RECACHED-CHECKPOINT", &id])
    };
    let ignorable = {
        let id = (Control::Format {
            features: FEATURE_EXPIRY_LOG | 1 << 3,
        })
        .id()
        .to_string();
        array(&["RECACHED-CHECKPOINT", &id])
    };
    let as_push = push(&["RECACHED-CHECKPOINT", FORMAT_ID]);
    for (label, bytes, timeless) in [
        (
            "format second",
            [array(&["PING"]), format_record()].concat(),
            false,
        ),
        ("unknown must-understand bit", must, false),
        ("unknown ignorable bit", ignorable, true),
        ("format written as a push", as_push, true),
    ] {
        let store = replay_bytes(&[bytes, counter(array, &t)].concat()).await;
        let got = store.execute(Command::Get("c".into()));
        assert_eq!(got == nil(), timeless, "{label}: {got:?}");
    }
}

#[tokio::test]
async fn the_timeless_replay_is_a_follower_of_the_logged_evictions() {
    // Cap 2: the primary held a, b, then SET c evicted a (its DEL follows).
    // Replayed as a capped store choosing its own victim, it could remove
    // b; as a follower it applies the primary's choice.
    let bytes = [
        format_record(),
        array(&["SET", "a", "1"]),
        array(&["SET", "b", "1"]),
        array(&["SET", "c", "1"]),
        array(&["DEL", "a"]),
    ]
    .concat();
    let dir = Dir::new("follower");
    std::fs::write(dir.aof(), &bytes).unwrap();
    let store = KeyValueStore::with_config(Some(2), None, EvictionPolicy::AllKeysRandom);
    replay_aof_format(&store, &dir.aof(), 0, true)
        .await
        .unwrap();
    let keys = store.execute(Command::Keys("*".into()));
    let Value::Array(Some(keys)) = keys else {
        panic!("{keys:?}")
    };
    let mut keys: Vec<Value> = keys;
    keys.sort_by_key(|k| format!("{k:?}"));
    assert_eq!(keys, vec![bulk("b"), bulk("c")]);
    assert!(!store.is_follower(), "restored after the replay");
}

// ── Restarts ─────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restarts_and_checkpoints_keep_the_primarys_timeline() {
    // The 6O-F timelines, end to end through restarts and a checkpoint: a
    // fixed window, INCR then PERSIST, INCR then RENAME, LSET of a volatile
    // list, APPEND to a volatile string. Replayed after the deadline.
    let dir = Dir::new("restarts");
    {
        let node = boot(&dir).await;
        let mut c = Pipe::tcp(&node);
        for cmd in [
            &["SET", "w", "1", "PX", "300"][..],
            &["INCR", "w"],
            &["SET", "p", "1", "PX", "300"],
            &["INCR", "p"],
            &["PERSIST", "p"],
            &["SET", "r", "1", "PX", "300"],
            &["INCR", "r"],
            &["RENAME", "r", "r2"],
        ] {
            assert!(!matches!(c.cmd(cmd).await, Value::Error(_)), "{cmd:?}");
        }
        assert_eq!(c.cmd(&["SAVE"]).await, ok());
        for cmd in [
            &["RPUSH", "l", "a", "b"][..],
            &["PEXPIRE", "l", "300"],
            &["LSET", "l", "0", "z"],
            &["SET", "s", "old", "PX", "300"],
            &["APPEND", "s", "+"],
        ] {
            assert!(!matches!(c.cmd(cmd).await, Value::Error(_)), "{cmd:?}");
        }
    }
    tokio::time::sleep(Duration::from_millis(400)).await;
    for round in 0..3 {
        let node = boot(&dir).await;
        let mut c = Pipe::tcp(&node);
        assert_eq!(c.cmd(&["GET", "w"]).await, nil(), "round {round}: window");
        assert_eq!(
            c.cmd(&["GET", "p"]).await,
            bulk("2"),
            "round {round}: persisted"
        );
        assert_eq!(c.cmd(&["TTL", "p"]).await, Value::Integer(-1));
        assert_eq!(
            c.cmd(&["GET", "r2"]).await,
            nil(),
            "round {round}: renamed, expired"
        );
        assert_eq!(
            c.cmd(&["LRANGE", "l", "0", "-1"]).await,
            Value::Array(Some(vec![]))
        );
        assert_eq!(c.cmd(&["GET", "s"]).await, nil(), "round {round}: appended");
        // A new session's writes, and one checkpoint, along the way.
        c.cmd(&["INCR", &format!("round{round}")]).await;
        if round == 1 {
            assert_eq!(c.cmd(&["SAVE"]).await, ok());
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_snapshot_pairs_with_how_its_records_replay() {
    // A snapshot holding an entry whose deadline has passed: kept (and
    // masked) when the AOF after it replays timelessly, or when it is empty
    // on a primary; dropped when it is legacy, or empty on a replica.
    let make = |records: Vec<u8>| async move {
        let dir = Dir::new("pair");
        let store = KeyValueStore::new();
        store.execute(Command::Set("k".into(), b"v".to_vec(), Default::default()));
        store.execute(Command::PExpireAt("k".into(), 1));
        let cfg = SnapshotConfig {
            path: dir.snap(),
            last_save: AtomicI64::new(0),
            checkpoint_id: AtomicU64::new(0),
        };
        let entries = core_engine::store::without_expiry(|| store.snapshot());
        save_snapshot_entries(entries, &cfg, 3).await.unwrap();
        std::fs::write(dir.aof(), records).unwrap();
        dir
    };
    for (label, records, primary, kept) in [
        (
            "timeless AOF",
            [format_record(), array(&["SET", "x", "1"])].concat(),
            true,
            true,
        ),
        ("legacy AOF", push(&["SET", "x", "1"]), true, false),
        ("empty AOF, primary", Vec::new(), true, true),
        ("empty AOF, replica", Vec::new(), false, false),
    ] {
        let dir = make(records).await;
        let node = boot_with(&dir, primary, KeyValueStore::new())
            .await
            .unwrap();
        let present = core_engine::store::without_expiry(|| {
            node.store.execute(Command::Exists(vec!["k".into()]))
        });
        assert_eq!(present == Value::Integer(1), kept, "{label}");
        assert_eq!(
            node.store.execute(Command::Get("k".into())),
            nil(),
            "{label}: masked"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_abandoned_ephemeral_key_stays_gone_after_the_next_session_writes_it() {
    // Review C, M2: discarded at startup but never logged, it came back
    // under the next session's APPEND (`"vx"`) and was then discarded again
    // with the acknowledged write.
    let dir = Dir::new("discard");
    // Kept open, as a connection a crash cuts: no disconnect releases it.
    let _held = {
        let node = boot(&dir).await;
        let mut owner = Pipe::tcp(&node);
        assert_eq!(owner.cmd(&["ESET", "e", "v"]).await, ok());
        (node, owner)
    };
    {
        let node = boot(&dir).await;
        let mut c = Pipe::tcp(&node);
        assert_eq!(c.cmd(&["GET", "e"]).await, nil());
        assert_eq!(c.cmd(&["APPEND", "e", "x"]).await, Value::Integer(1));
    }
    let node = boot(&dir).await;
    assert_eq!(Pipe::tcp(&node).cmd(&["GET", "e"]).await, bulk("x"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_save_does_not_reuse_its_checkpoint_id() {
    // Review C, L2. The snapshot cannot be written (its path is a
    // directory); the marker was, so the next save must use a new id.
    let dir = Dir::new("l2");
    let node = boot(&dir).await;
    std::fs::create_dir_all(dir.snap()).unwrap();
    assert!(node.state.save(&node.store).await.is_err());
    assert_eq!(node.state.snap.checkpoint_id.load(Ordering::SeqCst), 1);
    std::fs::remove_dir_all(dir.snap()).unwrap();
    node.state.save(&node.store).await.unwrap();
    assert_eq!(node.state.snap.checkpoint_id.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_failed_replay_leaves_the_store_as_it_found_it() {
    // A record the store refuses ends the replay with an error; the
    // follower mode of the timeless part is still undone.
    let dir = Dir::new("failed");
    let bytes = [
        format_record(),
        array(&["RPUSH", "l", "a"]),
        array(&["INCR", "l"]), // WRONGTYPE: the replay refuses the file
    ]
    .concat();
    std::fs::write(dir.aof(), &bytes).unwrap();
    let store = KeyValueStore::with_config(Some(10), None, EvictionPolicy::AllKeysLru);
    assert!(
        replay_aof_format(&store, &dir.aof(), 0, true)
            .await
            .is_err()
    );
    assert!(!store.is_follower());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_replica_restarted_on_a_format_file_leaves_the_format_at_once() {
    // Review C, R3-1: a primary's file, the node restarted as a replica of
    // a primary it cannot reach. Its own sweep removes `k` silently; it is
    // promoted and a client appends to `k`. The record must not replay as
    // the format's (it would land on the stale entry and expire with it).
    let dir = Dir::new("r31");
    {
        let node = boot(&dir).await;
        let mut c = Pipe::tcp(&node);
        c.cmd(&["SET", "k", "old", "PX", "100"]).await;
        c.cmd(&["SET", "z", "1"]).await;
    }
    let node = boot_with(&dir, false, KeyValueStore::new()).await.unwrap();
    assert_eq!(
        node.state.aof.as_ref().unwrap().file_mode(),
        AofFileMode::Legacy
    );
    tokio::time::sleep(Duration::from_millis(150)).await;
    sweep_expired_tick(&node.store, &node.state, &node.watch_registry, &node.tx).await;
    assert_eq!(node.store.key_count(), 1, "swept, silently, as a replica");
    node.state.promote_to_primary(&node.store).await;
    let mut c = Pipe::tcp(&node);
    assert_eq!(c.cmd(&["APPEND", "k", "x"]).await, Value::Integer(1));
    let recs = records(&dir.aof());
    assert!(!recs.last().unwrap().0, "written as a push: {recs:?}");
    drop(node);
    let node = boot(&dir).await;
    let mut c = Pipe::tcp(&node);
    assert_eq!(
        c.cmd(&["GET", "k"]).await,
        bulk("x"),
        "the acknowledged write"
    );
    assert_eq!(c.cmd(&["TTL", "k"]).await, Value::Integer(-1));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_checkpoint_truncation_leaves_the_file_legacy() {
    // Review C, R3-2: the truncation's FORMAT write fails; the writer must
    // not go on writing as the format's into a file without its head.
    let dir = Dir::new("r32");
    let aof = AofWriter::open(dir.aof(), AofSync::No).await.unwrap();
    aof.set_file_mode(AofFileMode::Timeless);
    aof.inject_fault(AofFault::Fail(std::io::ErrorKind::Other));
    assert!(aof.truncate_then(true).await.is_err());
    assert_eq!(aof.file_mode(), AofFileMode::Legacy);
    aof.append(&push(&["SET", "a", "1"])).await.unwrap();
    let recs = records(&dir.aof());
    assert!(recs.iter().all(|(array, _)| !array), "{recs:?}");
}
