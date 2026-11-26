//! 6O-I2: durable transaction atomicity in the AOF
//! (`benchmarks/v0.6.0/6o-i/V0.6O-I-PROTOCOL-DESIGN.md` §2–§5).
//!
//! In a file whose `FORMAT` carries the `txn` bit, an `EXEC` that logs two
//! or more records writes them between `TXN-BEGIN n` and `TXN-COMMIT n`.
//! The replay applies a complete group, discards an incomplete one at the
//! tail (and cuts the file back to its `TXN-BEGIN`), replays one another
//! server took over one by one, and refuses a malformed one.

use super::aof_format_6oh::{Dir, array, boot, push, records};
use super::mset_atomicity::Pipe;
use super::*;
use core_engine::resp::Value;
use core_engine::store::KeyValueStore;
use std::path::Path;

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

fn nil() -> Value {
    Value::BulkString(None)
}

fn ctl(record: Control) -> Vec<u8> {
    let mut frame = control_frame(record);
    frame[0] = b'*';
    frame
}

fn format_txn() -> Vec<u8> {
    ctl(Control::Format {
        features: FORMAT_FEATURES,
    })
}

fn begin(count: u32) -> Vec<u8> {
    ctl(Control::TxnBegin { count })
}

fn commit(count: u32) -> Vec<u8> {
    ctl(Control::TxnCommit { count })
}

fn begin_words(count: u32) -> Vec<String> {
    vec![
        "RECACHED-CHECKPOINT".into(),
        Control::TxnBegin { count }.id().to_string(),
    ]
}

fn commit_words(count: u32) -> Vec<String> {
    vec![
        "RECACHED-CHECKPOINT".into(),
        Control::TxnCommit { count }.id().to_string(),
    ]
}

/// The words of every record, in order.
fn words(path: &Path) -> Vec<Vec<String>> {
    records(path).into_iter().map(|(_, w)| w).collect()
}

fn get(store: &KeyValueStore, key: &str) -> Value {
    store.execute(Command::Get(key.into()))
}

/// Replay `bytes` as a primary's file; the store and the file afterwards.
async fn replay(bytes: &[u8]) -> (std::io::Result<KeyValueStore>, Vec<u8>) {
    let dir = Dir::new("6oi_replay");
    std::fs::write(dir.aof(), bytes).unwrap();
    let store = KeyValueStore::new();
    let result = replay_aof_format(&store, &dir.aof(), 0, true).await;
    let after = std::fs::read(dir.aof()).unwrap();
    (result.map(|_| store), after)
}

// ── The encoding ─────────────────────────────────────────────────────────

#[test]
fn the_group_records_are_the_designs() {
    // §2.1: ignorable kinds 0x81/0x82, the count as payload, 57 bytes.
    assert_eq!(
        Control::TxnBegin { count: 2 }.id().to_string(),
        "17005734034245943298"
    );
    assert_eq!(
        Control::TxnCommit { count: 2 }.id().to_string(),
        "17005735133757571074"
    );
    for record in [
        Control::TxnBegin { count: 2 },
        Control::TxnCommit { count: u32::MAX },
    ] {
        assert_eq!(Control::of(record.id()), Some(record));
        assert!(is_control_id(record.id()));
    }
    assert_eq!(control_frame(Control::TxnBegin { count: 2 }).len(), 57);
    // The txn bit is must-understand (16–31) and understood here.
    assert_eq!(FEATURE_TXN, 1 << 16);
    assert_eq!(FORMAT_FEATURES, FEATURE_EXPIRY_LOG | FEATURE_TXN);
}

// ── Writing groups ───────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_exec_of_two_or_more_records_is_written_as_one_group() {
    let dir = Dir::new("6oi_write");
    let node = boot(&dir).await;
    let mut c = Pipe::tcp(&node);
    // Two writes: a group around exactly their frames.
    c.cmd(&["MULTI"]).await;
    c.cmd(&["SET", "w:a", "1"]).await;
    c.cmd(&["INCR", "w:n"]).await;
    c.cmd(&["EXEC"]).await;
    // One write: one record, no group.
    c.cmd(&["MULTI"]).await;
    c.cmd(&["SET", "w:b", "1"]).await;
    c.cmd(&["EXEC"]).await;
    // Read-only and PUBLISH-only: nothing.
    c.cmd(&["MULTI"]).await;
    c.cmd(&["GET", "w:a"]).await;
    c.cmd(&["PUBLISH", "ch", "m"]).await;
    c.cmd(&["EXEC"]).await;
    // A write that changes nothing, then one that does: one record.
    c.cmd(&["MULTI"]).await;
    c.cmd(&["SET", "w:a", "9", "NX"]).await;
    c.cmd(&["SET", "w:c", "1"]).await;
    c.cmd(&["EXEC"]).await;
    // A write that finds its key expired: its DEL and its frame, a group.
    c.cmd(&["SET", "w:x", "old", "PX", "1"]).await;
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    c.cmd(&["MULTI"]).await;
    c.cmd(&["APPEND", "w:x", "new"]).await;
    c.cmd(&["EXEC"]).await;
    let recs = records(&dir.aof());
    assert!(recs.iter().all(|(array, _)| *array), "{recs:?}");
    let w = words(&dir.aof());
    let at = |rec: &[&str]| w.iter().position(|r| r == rec).unwrap();
    let set_a = at(&["SET", "w:a", "1"]);
    assert_eq!(w[set_a - 1], begin_words(2));
    assert_eq!(w[set_a + 1][0], "SET"); // INCR's frame
    assert_eq!(w[set_a + 2], commit_words(2));
    let set_b = at(&["SET", "w:b", "1"]);
    assert_ne!(w[set_b - 1], begin_words(1));
    assert!(!w.iter().any(|r| r == &begin_words(1)), "{w:?}");
    let set_c = at(&["SET", "w:c", "1"]);
    assert_eq!(w[set_c - 1][0], "SET", "no group for one record: {w:?}");
    let append = at(&["APPEND", "w:x", "new"]);
    assert_eq!(w[append - 1], ["DEL", "w:x"]);
    assert_eq!(w[append - 2], begin_words(2));
    assert_eq!(w[append + 1], commit_words(2));
    // Exactly two groups were written.
    assert_eq!(w.iter().filter(|r| **r == begin_words(2)).count(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_capped_exec_groups_its_eviction_dels_after_their_command() {
    let dir = Dir::new("6oi_evict");
    let store = KeyValueStore::with_config(
        Some(2),
        None,
        core_engine::store::EvictionPolicy::AllKeysLru,
    );
    let node = super::aof_format_6oh::boot_with(&dir, true, store)
        .await
        .unwrap();
    let mut c = Pipe::tcp(&node);
    c.cmd(&["SET", "e:a", "1"]).await;
    c.cmd(&["SET", "e:b", "1"]).await;
    c.cmd(&["MULTI"]).await;
    c.cmd(&["SET", "e:c", "1"]).await;
    c.cmd(&["SET", "e:d", "1"]).await;
    c.cmd(&["EXEC"]).await;
    let w = words(&dir.aof());
    let first = w.iter().position(|r| r == &["SET", "e:c", "1"]).unwrap();
    // BEGIN 4: SET c, its eviction DEL, SET d, its eviction DEL.
    assert_eq!(w[first - 1], begin_words(4), "{w:?}");
    assert_eq!(w[first + 1][0], "DEL");
    assert_eq!(w[first + 2], ["SET", "e:d", "1"]);
    assert_eq!(w[first + 3][0], "DEL");
    assert_eq!(w[first + 4], commit_words(4));
}

// ── Recovery: the reproduction, cut at every byte ────────────────────────

/// Every cut of the file inside an `EXEC`'s records replays all of the
/// transaction or none of it; an incomplete one is cut off, and the node
/// goes on writing and restarting normally (design §3.2, §3.6).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_exec_cut_anywhere_in_its_records_replays_all_or_nothing() {
    let dir = Dir::new("6oi_cut");
    let node = boot(&dir).await;
    let mut c = Pipe::tcp(&node);
    c.cmd(&["SET", "acct:a", "old"]).await;
    c.cmd(&["SET", "acct:b", "old"]).await;
    c.cmd(&["SET", "acct:x", "gone", "PX", "1"]).await;
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let before = std::fs::read(dir.aof()).unwrap();
    c.cmd(&["MULTI"]).await;
    c.cmd(&["SET", "acct:a", "new"]).await;
    c.cmd(&["APPEND", "acct:x", "new"]).await; // its expiry DEL, then its frame
    c.cmd(&["SET", "acct:b", "new"]).await;
    let exec = c.cmd(&["EXEC"]).await;
    assert!(matches!(exec, Value::Array(Some(_))), "{exec:?}");
    let full = std::fs::read(dir.aof()).unwrap();
    drop(c);
    drop(node);
    let mut wrong = Vec::new();
    for cut in before.len()..=full.len() {
        let copy = Dir::new("6oi_cut_copy");
        std::fs::write(copy.aof(), &full[..cut]).unwrap();
        let restarted = boot(&copy).await;
        let got = [
            get(&restarted.store, "acct:a"),
            get(&restarted.store, "acct:b"),
            get(&restarted.store, "acct:x"),
        ];
        let complete = cut == full.len();
        let want = if complete {
            [bulk("new"), bulk("new"), bulk("new")]
        } else {
            [bulk("old"), bulk("old"), nil()]
        };
        if got != want {
            wrong.push((cut, got.clone()));
        }
        // An incomplete one is cut back to the end of the records before it.
        if !complete {
            assert_eq!(std::fs::read(copy.aof()).unwrap(), before, "cut {cut}");
        }
        // It goes on: a write, a restart, the same state plus the write.
        let mut c = Pipe::tcp(&restarted);
        c.cmd(&["SET", "acct:after", "1"]).await;
        drop(c);
        drop(restarted);
        let again = boot(&copy).await;
        assert_eq!(get(&again.store, "acct:after"), bulk("1"), "cut {cut}");
        assert_eq!(get(&again.store, "acct:a"), want[0], "cut {cut}");
    }
    assert!(
        wrong.is_empty(),
        "{} of {} cut(s) did not replay all or nothing, e.g. {:?}",
        wrong.len(),
        full.len() - before.len() + 1,
        wrong.first()
    );
}

// ── Recovery: classification of crafted files (§3.1) ─────────────────────

fn set(k: &str, v: &str) -> Vec<u8> {
    array(&["SET", k, v])
}

#[tokio::test]
async fn a_complete_group_replays_and_its_markers_are_skipped() {
    let bytes = [
        format_txn(),
        begin(2),
        set("a", "1"),
        set("b", "1"),
        commit(2),
        set("c", "1"),
    ]
    .concat();
    let (store, after) = replay(&bytes).await;
    let store = store.unwrap();
    assert_eq!(get(&store, "a"), bulk("1"));
    assert_eq!(get(&store, "b"), bulk("1"));
    assert_eq!(get(&store, "c"), bulk("1"));
    assert_eq!(after, bytes, "a complete file is not touched");
}

#[tokio::test]
async fn an_incomplete_tail_is_discarded_and_cut_back_to_its_begin() {
    let head = [format_txn(), set("a", "0"), set("b", "0")].concat();
    let cases: [(&str, Vec<u8>); 4] = [
        ("begin alone", begin(2)),
        ("one member", [begin(2), set("a", "1")].concat()),
        (
            "all members, no commit",
            [begin(2), set("a", "1"), set("b", "1")].concat(),
        ),
        (
            "a torn commit",
            [
                begin(2),
                set("a", "1"),
                set("b", "1"),
                commit(2)[..30].to_vec(),
            ]
            .concat(),
        ),
    ];
    for (label, tail) in cases {
        let (store, after) = replay(&[head.clone(), tail].concat()).await;
        let store = store.unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(get(&store, "a"), bulk("0"), "{label}");
        assert_eq!(get(&store, "b"), bulk("0"), "{label}");
        assert_eq!(after, head, "{label}: cut back to its TXN-BEGIN");
    }
}

#[tokio::test]
async fn a_group_another_server_appended_after_replays_one_by_one() {
    // A crash left `BEGIN 2; SET a 1`; an older server recovered the file
    // (applying SET a 1) and appended `SET d 1` as a push (T4).
    let bytes = [
        format_txn(),
        set("a", "0"),
        set("b", "0"),
        begin(2),
        set("a", "1"),
        push(&["SET", "d", "1"]),
    ]
    .concat();
    let (store, after) = replay(&bytes).await;
    let store = store.unwrap();
    assert_eq!(
        get(&store, "a"),
        bulk("1"),
        "the state the older server served"
    );
    assert_eq!(get(&store, "b"), bulk("0"));
    assert_eq!(get(&store, "d"), bulk("1"), "nothing it appended is lost");
    assert_eq!(after, bytes, "nothing is cut");
}

#[tokio::test]
async fn the_files_older_binaries_left_replay_here() {
    // T4's files (`raw/i1-compat/t4-files/*.resp`), as each older binary
    // left them: abandoned, never discarded.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../benchmarks/v0.6.0/6o-i/raw/i1-compat/t4-files");
    // Review B F1: the evidence is in the tree; a missing folder fails.
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let mut replayed = 0;
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "resp") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let (store, _) = replay(&bytes).await;
        let store = store.unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(get(&store, "a"), bulk("new"), "{}", path.display());
        assert_eq!(get(&store, "b"), bulk("old"), "{}", path.display());
        assert_eq!(get(&store, "d"), bulk("1"), "{}", path.display());
        replayed += 1;
    }
    assert_eq!(replayed, 6, "one file per older binary");
}

#[tokio::test]
async fn malformed_groups_refuse_the_start() {
    let head = [format_txn(), set("a", "0")].concat();
    let cases: [(&str, Vec<u8>); 9] = [
        (
            "nested",
            [
                begin(2),
                set("a", "1"),
                begin(2),
                set("b", "1"),
                set("c", "1"),
                commit(2),
            ]
            .concat(),
        ),
        ("a commit never opened", [set("a", "1"), commit(2)].concat()),
        (
            "more members than declared",
            [
                begin(2),
                set("a", "1"),
                set("b", "1"),
                set("c", "1"),
                commit(2),
            ]
            .concat(),
        ),
        (
            "a commit of another count",
            [begin(2), set("a", "1"), set("b", "1"), commit(3)].concat(),
        ),
        (
            "an early commit, then valid-looking records",
            [
                begin(3),
                set("a", "1"),
                set("b", "1"),
                commit(3),
                set("z", "1"),
            ]
            .concat(),
        ),
        (
            "a group of one",
            [begin(1), set("a", "1"), commit(1)].concat(),
        ),
        (
            "a checkpoint inside",
            [
                begin(2),
                set("a", "1"),
                array(&["RECACHED-CHECKPOINT", "7"]),
                set("b", "1"),
                commit(2),
            ]
            .concat(),
        ),
        (
            "a DEDUP record inside",
            [
                begin(2),
                set("a", "1"),
                array(&["RECACHED-DEDUP", "cl", "1"]),
                commit(2),
            ]
            .concat(),
        ),
        (
            // Review A M3: v0.4.0 kept a torn member and appended its
            // acknowledged write after it; the two parse as one record.
            "a torn member with another server's record inside it",
            [
                begin(2),
                set("a", "new"),
                b"*3\r\n$3\r\nSET\r\n$1\r\nb\r\n".to_vec(),
                push(&["SET", "d", "1"]),
            ]
            .concat(),
        ),
    ];
    for (label, body) in cases {
        let bytes = [head.clone(), body].concat();
        let (store, after) = replay(&bytes).await;
        let Err(e) = store else {
            panic!("{label}: accepted");
        };
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData, "{label}");
        assert!(
            e.to_string().contains("AOF corrupted at offset"),
            "{label}: {e}"
        );
        assert_eq!(after, bytes, "{label}: a refused file is not touched");
    }
}

#[tokio::test]
async fn group_records_outside_a_txn_file_are_ignorable_no_ops() {
    // A 6O-H file (FORMAT without `txn`): the kinds are ignorable, as they
    // are to 6O-H itself; an incomplete one is not a group here.
    let format_h = ctl(Control::Format {
        features: FEATURE_EXPIRY_LOG,
    });
    let bytes = [format_h, begin(2), set("a", "1")].concat();
    let (store, after) = replay(&bytes).await;
    assert_eq!(get(&store.unwrap(), "a"), bulk("1"));
    assert_eq!(after, bytes);
}

#[tokio::test]
async fn the_snapshots_marker_is_not_moved_by_groups() {
    let dir = Dir::new("6oi_snap");
    let store = KeyValueStore::new();
    let bytes = [
        format_txn(),
        begin(2),
        set("pre", "1"),
        set("pre2", "1"),
        commit(2),
        array(&["RECACHED-CHECKPOINT", "1"]),
        begin(2),
        set("post", "1"),
        set("post2", "1"),
        commit(2),
    ]
    .concat();
    std::fs::write(dir.aof(), &bytes).unwrap();
    replay_aof_format(&store, &dir.aof(), 1, true)
        .await
        .unwrap();
    assert_eq!(get(&store, "pre"), nil(), "in the snapshot, not replayed");
    assert_eq!(get(&store, "post"), bulk("1"));
    assert_eq!(get(&store, "post2"), bulk("1"));
}

// ── The writer never leaves a group open (§3.5) ──────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_write_inside_a_group_is_cut_back_to_its_begin() {
    let frames = [push(&["SET", "f:a", "1"]), push(&["SET", "f:b", "1"])];
    let frames: Vec<&[u8]> = frames.iter().map(Vec::as_slice).collect();
    let g = [begin(2), set("f:a", "1"), set("f:b", "1"), commit(2)];
    let boundaries = [
        g[0].len() / 2,                            // inside TXN-BEGIN
        g[0].len(),                                // right after it
        g[0].len() + g[1].len(),                   // after the first member
        g[0].len() + g[1].len() + 7,               // inside the second
        g[0].len() + g[1].len() + g[2].len(),      // before TXN-COMMIT
        g[0].len() + g[1].len() + g[2].len() + 20, // inside TXN-COMMIT
    ];
    for torn in boundaries {
        let dir = Dir::new("6oi_fault");
        let aof = AofWriter::open(dir.aof(), AofSync::EverySec).await.unwrap();
        // As the replay of an empty file leaves it.
        aof.set_file_mode(AofFileMode::Empty);
        assert!(aof.begin_expiry_log().await.unwrap());
        let head = std::fs::read(dir.aof()).unwrap();
        aof.inject_fault(AofFault::TornThenFail(torn, std::io::ErrorKind::Other));
        assert!(
            aof.append_transaction(&frames).await.is_err(),
            "torn at {torn}"
        );
        assert_eq!(
            std::fs::read(dir.aof()).unwrap(),
            head,
            "torn at {torn}: nothing of the group stays"
        );
        // The next transaction follows the last complete record.
        aof.append_transaction(&frames).await.unwrap();
        let (store, _) = replay(&std::fs::read(dir.aof()).unwrap()).await;
        assert_eq!(get(&store.unwrap(), "f:b"), bulk("1"), "torn at {torn}");
    }
}

// ── Where groups are written (§2.3) ──────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn only_a_txn_file_gets_groups_until_its_checkpoint() {
    for (label, head) in [
        (
            "a 6O-H file",
            ctl(Control::Format {
                features: FEATURE_EXPIRY_LOG,
            }),
        ),
        ("a legacy file", push(&["SET", "l", "1"])),
    ] {
        let dir = Dir::new("6oi_upgrade");
        std::fs::write(dir.aof(), &head).unwrap();
        let node = boot(&dir).await;
        let mut c = Pipe::tcp(&node);
        c.cmd(&["MULTI"]).await;
        c.cmd(&["SET", "u:a", "1"]).await;
        c.cmd(&["SET", "u:b", "1"]).await;
        c.cmd(&["EXEC"]).await;
        assert!(
            !words(&dir.aof()).contains(&begin_words(2)),
            "{label}: no group before the checkpoint"
        );
        assert_eq!(c.cmd(&["SAVE"]).await, Value::SimpleString("OK".into()));
        c.cmd(&["MULTI"]).await;
        c.cmd(&["SET", "u:a", "2"]).await;
        c.cmd(&["SET", "u:b", "2"]).await;
        c.cmd(&["EXEC"]).await;
        let w = words(&dir.aof());
        assert_eq!(
            w[0],
            [
                "RECACHED-CHECKPOINT",
                &Control::Format {
                    features: FORMAT_FEATURES
                }
                .id()
                .to_string()
            ],
            "{label}"
        );
        assert!(
            w.contains(&begin_words(2)),
            "{label}: grouped after it: {w:?}"
        );
        drop(c);
        drop(node);
        let again = boot(&dir).await;
        assert_eq!(get(&again.store, "u:b"), bulk("2"), "{label}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restarts_and_checkpoints_keep_grouping() {
    let dir = Dir::new("6oi_lifecycle");
    for round in 0..3 {
        let node = boot(&dir).await;
        let mut c = Pipe::tcp(&node);
        let v = round.to_string();
        c.cmd(&["MULTI"]).await;
        c.cmd(&["SET", "r:a", &v]).await;
        c.cmd(&["SET", "r:b", &v]).await;
        c.cmd(&["EXEC"]).await;
        if round == 1 {
            c.cmd(&["SAVE"]).await;
            c.cmd(&["MULTI"]).await;
            c.cmd(&["INCR", "r:n"]).await;
            c.cmd(&["INCR", "r:n"]).await;
            c.cmd(&["EXEC"]).await;
        }
        assert!(words(&dir.aof()).contains(&begin_words(2)), "round {round}");
        drop(c);
        drop(node);
        let again = boot(&dir).await;
        assert_eq!(get(&again.store, "r:a"), bulk(&v));
        assert_eq!(get(&again.store, "r:b"), bulk(&v));
        if round >= 1 {
            assert_eq!(get(&again.store, "r:n"), bulk("2"));
        }
    }
}

// ── Review A, round 1 ────────────────────────────────────────────────────

/// M1: kinds any reader may skip are skipped inside a group too, and a
/// non-ignorable kind this server does not know (a later writer's) abandons
/// it; after the timeless part ends, group records are no-ops.
#[tokio::test]
async fn later_writers_control_kinds_follow_the_6oh_rules_inside_groups() {
    let unknown = |kind: u64| {
        let id = (CONTROL_TAG << 48) | (kind << 40) | (1 << 32) | 7;
        array(&["RECACHED-CHECKPOINT", &id.to_string()])
    };
    // An ignorable kind between members: not a member.
    let bytes = [
        format_txn(),
        begin(2),
        set("a", "1"),
        unknown(0x83),
        set("b", "1"),
        commit(2),
    ]
    .concat();
    let (store, _) = replay(&bytes).await;
    let store = store.unwrap();
    assert_eq!((get(&store, "a"), get(&store, "b")), (bulk("1"), bulk("1")));
    // A non-ignorable unknown kind: abandoned, its members one by one, and
    // what follows replays as 6O-H replays it.
    let bytes = [
        format_txn(),
        begin(3),
        set("a", "1"),
        unknown(0x05),
        set("b", "1"),
    ]
    .concat();
    let (store, after) = replay(&bytes).await;
    let store = store.unwrap();
    assert_eq!((get(&store, "a"), get(&store, "b")), (bulk("1"), bulk("1")));
    assert_eq!(after, bytes, "not a tail to cut");
    // Group records after the timeless part ended: no-ops.
    let bytes = [
        format_txn(),
        push(&["SET", "p", "1"]),
        begin(2),
        set("a", "2"),
        commit(5),
    ]
    .concat();
    let (store, _) = replay(&bytes).await;
    assert_eq!(get(&store.unwrap(), "a"), bulk("2"));
}

/// M2: authorship first. Whatever a push is (a DEDUP-wrapped write, as
/// every browser write is, or an older server's checkpoint marker from a
/// SAVE that crashed before truncating), inside an open group it abandons
/// the group.
#[tokio::test]
async fn any_push_inside_an_open_group_abandons_it_first() {
    let dangling = [format_txn(), set("a", "0"), begin(2), set("a", "1")].concat();
    // (a) v0.5.2 and later wrap a WebSocket write in its DEDUP envelope.
    let bytes = [
        dangling.clone(),
        push(&["DEDUP", "cl", "7", "SET", "k", "v"]),
    ]
    .concat();
    let (store, after) = replay(&bytes).await;
    let store = store.unwrap();
    assert_eq!((get(&store, "a"), get(&store, "k")), (bulk("1"), bulk("v")));
    assert_eq!(after, bytes);
    // (b) Its SAVE wrote `RECACHED-CHECKPOINT 3` and crashed after renaming
    // snapshot 3: the replay starts after the marker, and starts.
    let dir = Dir::new("6oi_push_marker");
    let bytes = [dangling, push(&["RECACHED-CHECKPOINT", "3"])].concat();
    std::fs::write(dir.aof(), &bytes).unwrap();
    let store = KeyValueStore::new();
    let replayed = replay_aof_format(&store, &dir.aof(), 3, true)
        .await
        .unwrap();
    assert_eq!(get(&store, "a"), nil(), "before the snapshot's marker");
    // The marker does not end the timeless part (6O-H): the file stays a
    // `txn` file, and the abandoned group is classified the same way on
    // every later replay.
    assert_eq!(replayed.mode, AofFileMode::Timeless);
    assert!(replayed.grouped);
}

/// M3: the bytes a tail cut removes are kept, synced, beside the file.
#[tokio::test]
async fn the_bytes_a_tail_cut_removes_are_kept_beside_the_file() {
    let dir = Dir::new("6oi_cut_kept");
    let head = [format_txn(), set("a", "0")].concat();
    let tail = [begin(2), set("a", "1")].concat();
    std::fs::write(dir.aof(), [head.clone(), tail.clone()].concat()).unwrap();
    replay_aof_format(&KeyValueStore::new(), &dir.aof(), 0, true)
        .await
        .unwrap();
    assert_eq!(std::fs::read(dir.aof()).unwrap(), head);
    let kept: Vec<_> = std::fs::read_dir(&dir.0)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().contains(".cut-"))
        .collect();
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(
        kept[0]
            .to_string_lossy()
            .contains(&format!("append.aof.cut-{}-", head.len())),
        "{kept:?}"
    );
    assert_eq!(std::fs::read(&kept[0]).unwrap(), tail);
    // Written under a temporary name and renamed (review A R3-L1): none left.
    let temporary = std::fs::read_dir(&dir.0)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .to_string_lossy()
                .ends_with(".tmp")
        })
        .count();
    assert_eq!(temporary, 0);
}

/// L1: 6O-I never appends after an open group. A replica restarted on its
/// own `txn` file (it leaves the format at startup and writes pushes) cuts
/// the incomplete tail before its first append.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_replica_restarted_on_an_incomplete_tail_cuts_it_before_appending() {
    let dir = Dir::new("6oi_replica_tail");
    let head = [format_txn(), set("a", "0")].concat();
    std::fs::write(dir.aof(), [head.clone(), begin(2), set("a", "1")].concat()).unwrap();
    let node = super::aof_format_6oh::boot_with(&dir, false, KeyValueStore::new())
        .await
        .unwrap();
    assert_eq!(get(&node.store, "a"), bulk("0"));
    node.state
        .aof
        .as_ref()
        .unwrap()
        .append(&push(&["SET", "r", "1"]))
        .await
        .unwrap();
    let bytes = std::fs::read(dir.aof()).unwrap();
    assert_eq!(&bytes[..head.len()], head.as_slice());
    assert_eq!(&bytes[head.len()..], push(&["SET", "r", "1"]).as_slice());
}

/// Review A R2-M1: a copy that cannot be written does not stop the start;
/// the cut goes ahead in place, as before 6O-I.
#[cfg(unix)]
#[tokio::test]
async fn a_tail_cut_whose_copy_cannot_be_written_still_starts() {
    use std::os::unix::fs::PermissionsExt;
    let dir = Dir::new("6oi_cut_readonly");
    let head = [format_txn(), set("a", "0")].concat();
    std::fs::write(dir.aof(), [head.clone(), begin(2), set("a", "1")].concat()).unwrap();
    std::fs::set_permissions(&dir.0, std::fs::Permissions::from_mode(0o555)).unwrap();
    let store = KeyValueStore::new();
    let result = replay_aof_format(&store, &dir.aof(), 0, true).await;
    std::fs::set_permissions(&dir.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    result.unwrap();
    assert_eq!(get(&store, "a"), bulk("0"));
    assert_eq!(std::fs::read(dir.aof()).unwrap(), head);
}

/// Review B F2: a member's `*` flipped to `>` abandons its group (a push
/// inside an open group, §3.1 row 1); the push ends the timeless part, the
/// stray `TXN-COMMIT` is then a no-op after it, and no member is lost.
#[tokio::test]
async fn a_member_flipped_to_a_push_abandons_its_group_and_loses_nothing() {
    let bytes = [
        format_txn(),
        begin(2),
        set("a", "1"),
        push(&["SET", "b", "1"]),
        commit(2),
    ]
    .concat();
    let dir = Dir::new("6oi_flipped");
    std::fs::write(dir.aof(), &bytes).unwrap();
    let store = KeyValueStore::new();
    let replayed = replay_aof_format(&store, &dir.aof(), 0, true)
        .await
        .unwrap();
    assert_eq!((get(&store, "a"), get(&store, "b")), (bulk("1"), bulk("1")));
    assert_eq!(replayed.mode, AofFileMode::Legacy);
    assert!(!replayed.grouped);
}

/// Review B F3: a nested record the snapshot already holds (before the
/// replay start) is never applied, so it does not refuse the start.
#[tokio::test]
async fn a_nested_record_before_the_replay_start_does_not_refuse_it() {
    let nested = [
        b"*3\r\n$3\r\nSET\r\n$1\r\nb\r\n".to_vec(),
        push(&["SET", "d", "1"]),
    ]
    .concat();
    let bytes = [
        nested.clone(),
        array(&["RECACHED-CHECKPOINT", "1"]),
        set("after", "1"),
    ]
    .concat();
    let dir = Dir::new("6oi_nested_before");
    std::fs::write(dir.aof(), &bytes).unwrap();
    let store = KeyValueStore::new();
    replay_aof_format(&store, &dir.aof(), 1, true)
        .await
        .unwrap();
    assert_eq!(get(&store, "after"), bulk("1"));
    // After the replay start: refused.
    let (store, _) = replay(&[set("x", "1"), nested].concat()).await;
    assert!(store.is_err());
}
