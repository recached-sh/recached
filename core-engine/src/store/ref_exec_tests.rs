//! `execute_reporting_ref` against `execute_reporting`.
//!
//! The server runs writes by reference so it can keep the command for
//! propagation without cloning it; embedders still run them owned. The two
//! paths share the accounting wrapper and the command primitives, and must be
//! indistinguishable: same replies, same keyspace, same TTLs, same memory
//! estimate, same key count — including on the capped and evicting stores,
//! where the wrapper's rollback and reservation paths run.

use super::*;
use crate::cmd::SetOptions;

fn set(k: &str, v: &[u8], opts: SetOptions) -> Command {
    Command::Set(k.into(), v.to_vec(), opts)
}

fn opts(expiry: Option<SetExpiry>, condition: Option<SetCondition>, get: bool) -> SetOptions {
    SetOptions {
        expiry,
        condition,
        get,
    }
}

/// A deterministic mix of every command the by-reference path implements,
/// with the cases that take the less common branches: options, conditions,
/// type clashes, overflow, duplicate fields, packed-to-table promotion,
/// overwrites that change a key's type or clear its TTL.
fn script() -> Vec<Command> {
    let long = vec![b'L'; 200];
    let mut cmds = vec![
        set("s", b"1", SetOptions::default()),
        set("s", b"two", opts(None, Some(SetCondition::Nx), false)),
        set("s", b"three", opts(None, Some(SetCondition::Xx), true)),
        set("absent", b"x", opts(None, Some(SetCondition::Xx), false)),
        set("t", b"v", opts(Some(SetExpiry::Ex(100)), None, false)),
        set("t", b"w", opts(Some(SetExpiry::KeepTtl), None, true)),
        set("t2", b"v", opts(Some(SetExpiry::Px(5_000)), None, false)),
        set(
            "t3",
            b"v",
            opts(Some(SetExpiry::Pxat(u64::MAX / 2)), None, false),
        ),
        set("t4", b"v", opts(Some(SetExpiry::Ex(u64::MAX)), None, false)),
        set("big", &long, SetOptions::default()),
        set("big", b"", SetOptions::default()),
        set("bin", b"\x00\xff\r\n", SetOptions::default()),
        Command::Incr("n".into()),
        Command::IncrBy("n".into(), 41),
        Command::Decr("n".into()),
        Command::DecrBy("n".into(), -5),
        Command::DecrBy("n".into(), i64::MIN),
        Command::IncrBy("n".into(), i64::MAX),
        Command::Incr("s".into()),
        Command::Incr("bin".into()),
        Command::HSet("h".into(), vec![("f".into(), b"1".to_vec())]),
        Command::HSet(
            "h".into(),
            vec![("f".into(), b"2".to_vec()), ("f".into(), b"3".to_vec())],
        ),
        Command::HSet("h".into(), vec![("g".into(), long.clone())]),
        Command::HSet("s".into(), vec![("f".into(), b"wrongtype".to_vec())]),
        Command::Incr("h".into()),
        set("h", b"now a string", SetOptions::default()),
        Command::MSet(vec![
            ("m1".into(), b"a".to_vec()),
            ("t".into(), b"ttl cleared".to_vec()),
            ("m1".into(), b"b".to_vec()),
        ]),
        Command::HSet("hh".into(), vec![("x".into(), b"y".to_vec())]),
        Command::MSet(vec![("hh".into(), b"replaces a hash".to_vec())]),
    ];
    // Grow one hash past the packed limits field by field, then overwrite.
    for i in 0..40 {
        cmds.push(Command::HSet(
            "grow".into(),
            vec![(format!("f{i}"), format!("v{i}").into_bytes())],
        ));
    }
    for i in 0..40 {
        cmds.push(Command::HSet(
            "grow".into(),
            vec![(format!("f{i}"), b"over".to_vec())],
        ));
    }
    for i in 0..50 {
        cmds.push(set(
            &format!("k{i}"),
            &vec![b'v'; i * 3],
            SetOptions::default(),
        ));
        cmds.push(Command::Incr(format!("c{}", i % 7)));
    }
    cmds
}

fn state(store: &KeyValueStore) -> String {
    let mut entries: Vec<String> = store
        .snapshot()
        .into_iter()
        .map(|e| {
            // Relative TTLs are computed from the clock, which moves between
            // the two runs; compare whether a deadline exists, and absolute
            // ones exactly.
            let ttl = match e.expires_at_ms {
                Some(at) if at > u64::MAX / 4 => format!("at {at}"),
                Some(_) => "relative".to_string(),
                None => "none".to_string(),
            };
            format!(
                "{} {} {}",
                e.key,
                // Through `serde_json::Value`, whose objects keep their keys
                // sorted: a hash's fields serialize in HashMap order, which
                // differs between two stores.
                serde_json::to_value(&e.value).expect("snapshot value serializes"),
                ttl
            )
        })
        .collect();
    entries.sort();
    entries.join("\n")
}

fn compare(make: impl Fn() -> KeyValueStore, what: &str) {
    let owned = make();
    let borrowed = make();
    for (i, cmd) in script().into_iter().enumerate() {
        let by_ref = borrowed.execute_reporting_ref(&cmd);
        let by_value = owned.execute_reporting(cmd.clone());
        assert_eq!(by_ref, by_value, "{what}: step {i} {cmd:?}");
        assert_eq!(
            borrowed.approximate_memory_bytes(),
            owned.approximate_memory_bytes(),
            "{what}: memory after step {i} {cmd:?}"
        );
        assert_eq!(
            borrowed.key_count(),
            owned.key_count(),
            "{what}: key count after step {i} {cmd:?}"
        );
    }
    assert_eq!(state(&borrowed), state(&owned), "{what}: final keyspace");
}

#[test]
fn by_reference_execution_matches_owned_execution() {
    compare(KeyValueStore::new, "uncapped");
}

#[test]
fn by_reference_execution_matches_owned_execution_when_capped() {
    // A key cap and a memory cap that refuse: the wrapper's reservation and
    // rollback paths both run. (An evicting policy samples its victims at
    // random, so two stores would legitimately evict different keys.)
    compare(
        || KeyValueStore::with_config(Some(20), None, EvictionPolicy::NoEviction),
        "max-keys noeviction",
    );
    compare(
        || KeyValueStore::with_config(None, Some(4_096), EvictionPolicy::NoEviction),
        "max-memory noeviction",
    );
}

/// The owned `MSET` used to insert a fresh key and entry for every pair; it
/// now overwrites in place like `SET`. The key the index shares with the map
/// must survive that, or SCAN/RANDOMKEY would hold a stale allocation.
#[test]
fn mset_overwrite_keeps_the_indexed_key() {
    let store = KeyValueStore::new();
    store.execute(set("a", b"1", SetOptions::default()));
    store.execute(Command::MSet(vec![("a".into(), b"2".to_vec())]));
    store.execute_reporting_ref(&Command::MSet(vec![("a".into(), b"3".to_vec())]));
    assert_eq!(store.key_count(), 1);
    assert_eq!(
        store.execute(Command::Get("a".into())),
        Value::BulkString(Some(b"3".to_vec()))
    );
    assert_eq!(
        store.execute(Command::Keys("*".into())),
        Value::Array(Some(vec![Value::BulkString(Some(b"a".to_vec()))]))
    );
}

/// INCR's stack formatter against `i64::to_string`, at every edge.
#[test]
fn incr_formats_like_to_string() {
    let mut cases = vec![
        0,
        1,
        -1,
        9,
        10,
        -10,
        99,
        100,
        i64::MAX,
        i64::MIN,
        i64::MIN + 1,
    ];
    let mut n: i64 = 1;
    while let Some(next) = n.checked_mul(10) {
        cases.extend([n - 1, n, n + 1, -n, -(n - 1)]);
        n = next;
    }
    for n in cases {
        assert_eq!(
            format_i64(n, &mut [0u8; 20]),
            n.to_string().as_bytes(),
            "{n}"
        );
    }
}
