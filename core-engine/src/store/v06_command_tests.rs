//! Engine-level tests for the commands added in 0.6: GETDEL, GETEX,
//! EXPIRETIME/PEXPIRETIME, RENAMENX, COPY, HSTRLEN, ZPOPMIN/ZPOPMAX.
//!
//! Replies are checked against Redis 7.2.5 output captured while building
//! them; the differential test in `server-native/tests/redis_diff.rs` runs the
//! same cases against a live Redis.

use super::*;
use crate::cmd::{GetExOption, SetOptions};
use std::sync::Arc;

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}
fn nil() -> Value {
    Value::BulkString(None)
}
fn int(n: i64) -> Value {
    Value::Integer(n)
}
fn arr(items: &[&str]) -> Value {
    Value::Array(Some(items.iter().map(|s| bulk(s)).collect()))
}
fn set(store: &KeyValueStore, k: &str, v: &str) {
    store.execute(Command::Set(k.into(), v.into(), SetOptions::default()));
}
fn parse(args: &[&str]) -> Result<Command, String> {
    Command::from_value(Value::Array(Some(args.iter().map(|a| bulk(a)).collect())))
}
fn is_err(v: &Value, prefix: &str) -> bool {
    matches!(v, Value::Error(e) if e.starts_with(prefix))
}

// ── GETDEL ───────────────────────────────────────────────────────────────────

#[test]
fn getdel_returns_and_deletes_strings_only() {
    let s = KeyValueStore::new();
    set(&s, "k", "v");
    assert_eq!(s.execute(Command::GetDel("k".into())), bulk("v"));
    assert_eq!(s.execute(Command::GetDel("k".into())), nil());
    assert_eq!(s.execute(Command::Exists(vec!["k".into()])), int(0));
    s.execute(Command::HSet("h".into(), vec![("f".into(), "v".into())]));
    assert!(is_err(&s.execute(Command::GetDel("h".into())), "WRONGTYPE"));
    // A type error must not destroy the key.
    assert_eq!(s.execute(Command::Exists(vec!["h".into()])), int(1));
}

#[test]
fn getdel_hands_a_value_to_exactly_one_caller() {
    const THREADS: usize = 8;
    for round in 0..200 {
        let store = Arc::new(KeyValueStore::new());
        set(&store, "k", &format!("v{round}"));
        let winners: usize = (0..THREADS)
            .map(|_| {
                let store = Arc::clone(&store);
                std::thread::spawn(move || {
                    matches!(
                        store.execute(Command::GetDel("k".into())),
                        Value::BulkString(Some(_))
                    )
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|h| usize::from(h.join().unwrap()))
            .sum();
        assert_eq!(
            winners, 1,
            "round {round}: {winners} callers took the value"
        );
    }
}

#[test]
fn getdel_releases_the_memory_it_was_charged() {
    let s = KeyValueStore::new();
    let before = s.approximate_memory_bytes();
    set(&s, "k", &"x".repeat(4096));
    assert!(s.approximate_memory_bytes() > before);
    s.execute(Command::GetDel("k".into()));
    assert_eq!(s.approximate_memory_bytes(), before);
}

// ── GETEX ────────────────────────────────────────────────────────────────────

#[test]
fn getex_parses_like_redis() {
    assert_eq!(parse(&["GETEX", "k"]), Ok(Command::GetEx("k".into(), None)));
    assert_eq!(
        parse(&["GETEX", "k", "ex", "100"]),
        Ok(Command::GetEx("k".into(), Some(GetExOption::Ex(100))))
    );
    assert_eq!(
        parse(&["GETEX", "k", "PERSIST"]),
        Ok(Command::GetEx("k".into(), Some(GetExOption::Persist)))
    );
    for (args, want) in [
        (
            &["GETEX", "k", "px", "0"][..],
            "ERR invalid expire time in 'getex' command",
        ),
        (
            &["GETEX", "k", "ex", "-1"],
            "ERR invalid expire time in 'getex' command",
        ),
        (
            &["GETEX", "k", "ex", "9223372036854775807"],
            "ERR invalid expire time in 'getex' command",
        ),
        (&["GETEX", "k", "ex", "10", "px", "10"], "ERR syntax error"),
        (&["GETEX", "k", "persist", "ex", "5"], "ERR syntax error"),
        (&["GETEX", "k", "bogus"], "ERR syntax error"),
        (&["GETEX", "k", "ex"], "ERR syntax error"),
        (
            &["GETEX", "k", "ex", "notnum"],
            "ERR value is not an integer or out of range",
        ),
        (
            &["GETEX"],
            "ERR wrong number of arguments for 'getex' command",
        ),
    ] {
        assert_eq!(parse(args).unwrap_err(), want, "{args:?}");
    }
}

#[test]
fn getex_reads_and_changes_expiry() {
    let s = KeyValueStore::new();
    set(&s, "k", "v");
    assert_eq!(s.execute(Command::GetEx("k".into(), None)), bulk("v"));
    assert_eq!(s.execute(Command::Ttl("k".into())), int(-1));
    assert_eq!(
        s.execute(Command::GetEx("k".into(), Some(GetExOption::Ex(100)))),
        bulk("v")
    );
    assert_eq!(s.execute(Command::Ttl("k".into())), int(100));
    assert_eq!(
        s.execute(Command::GetEx("k".into(), Some(GetExOption::Persist))),
        bulk("v")
    );
    assert_eq!(s.execute(Command::Ttl("k".into())), int(-1));
    let far = 4_102_444_800_123;
    s.execute(Command::GetEx("k".into(), Some(GetExOption::PxAt(far))));
    assert_eq!(s.execute(Command::PExpireTime("k".into())), int(far as i64));
    // A deadline already past deletes the key, after replying with its value.
    assert_eq!(
        s.execute(Command::GetEx("k".into(), Some(GetExOption::ExAt(1)))),
        bulk("v")
    );
    assert_eq!(s.execute(Command::Exists(vec!["k".into()])), int(0));
    // Missing key: nil, and nothing is created.
    assert_eq!(
        s.execute(Command::GetEx("nope".into(), Some(GetExOption::Ex(5)))),
        nil()
    );
    assert_eq!(s.execute(Command::Exists(vec!["nope".into()])), int(0));
    s.execute(Command::HSet("h".into(), vec![("f".into(), "v".into())]));
    assert!(is_err(
        &s.execute(Command::GetEx("h".into(), Some(GetExOption::Ex(5)))),
        "WRONGTYPE"
    ));
    assert_eq!(s.execute(Command::Ttl("h".into())), int(-1));
}

// ── EXPIRETIME / PEXPIRETIME ─────────────────────────────────────────────────

#[test]
fn expiretime_reports_absolute_deadlines() {
    let s = KeyValueStore::new();
    assert_eq!(s.execute(Command::ExpireTime("k".into())), int(-2));
    set(&s, "k", "v");
    assert_eq!(s.execute(Command::ExpireTime("k".into())), int(-1));
    assert_eq!(s.execute(Command::PExpireTime("k".into())), int(-1));
    s.execute(Command::PExpireAt("k".into(), 4_102_444_800_123));
    // Floored to seconds, as Redis does.
    assert_eq!(
        s.execute(Command::ExpireTime("k".into())),
        int(4_102_444_800)
    );
    assert_eq!(
        s.execute(Command::PExpireTime("k".into())),
        int(4_102_444_800_123)
    );
    assert!(parse(&["EXPIRETIME"]).is_err());
    assert!(parse(&["EXPIRETIME", "a", "b"]).is_err());
}

// ── RENAMENX ─────────────────────────────────────────────────────────────────

#[test]
fn renamenx_renames_only_onto_a_free_name() {
    let s = KeyValueStore::new();
    set(&s, "a", "1");
    set(&s, "b", "2");
    assert_eq!(s.execute(Command::RenameNx("a".into(), "b".into())), int(0));
    assert_eq!(s.execute(Command::Get("b".into())), bulk("2"));
    assert_eq!(s.execute(Command::RenameNx("a".into(), "c".into())), int(1));
    assert_eq!(s.execute(Command::Get("c".into())), bulk("1"));
    assert_eq!(s.execute(Command::Exists(vec!["a".into()])), int(0));
    assert!(is_err(
        &s.execute(Command::RenameNx("nosuch".into(), "x".into())),
        "ERR no such key"
    ));
    // Onto itself: the destination exists.
    assert_eq!(s.execute(Command::RenameNx("c".into(), "c".into())), int(0));
    // An expired destination does not exist.
    set(&s, "dead", "x");
    s.execute(Command::PExpireAt("dead".into(), 1));
    assert_eq!(
        s.execute(Command::RenameNx("c".into(), "dead".into())),
        int(1)
    );
}

#[test]
fn renamenx_moves_ttl_with_the_value() {
    let s = KeyValueStore::new();
    set(&s, "a", "1");
    s.execute(Command::PExpireAt("a".into(), 4_102_444_800_000));
    s.execute(Command::RenameNx("a".into(), "b".into()));
    assert_eq!(
        s.execute(Command::PExpireTime("b".into())),
        int(4_102_444_800_000)
    );
}

#[test]
fn concurrent_renamenx_onto_one_name_has_one_winner() {
    // Each thread renames its own source onto the same destination; RENAMENX
    // must let exactly one through. The check and the move are serialised by
    // the store's per-key ordering for both keys.
    for round in 0..100 {
        let store = Arc::new(KeyValueStore::new());
        for t in 0..8 {
            set(&store, &format!("src{t}"), &format!("{t}"));
        }
        let wins: i64 = (0..8)
            .map(|t| {
                let store = Arc::clone(&store);
                std::thread::spawn(move || {
                    match store.execute(Command::RenameNx(format!("src{t}"), "dst".into())) {
                        Value::Integer(n) => n,
                        other => panic!("{other:?}"),
                    }
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|h| h.join().unwrap())
            .sum();
        assert_eq!(wins, 1, "round {round}");
        assert_eq!(store.execute(Command::DbSize), int(8));
    }
}

// ── COPY ─────────────────────────────────────────────────────────────────────

#[test]
fn copy_parses_its_options_honestly() {
    assert_eq!(
        parse(&["COPY", "a", "b", "REPLACE"]),
        Ok(Command::Copy("a".into(), "b".into(), true))
    );
    assert_eq!(
        parse(&["COPY", "a", "b", "DB", "0"]),
        Ok(Command::Copy("a".into(), "b".into(), false))
    );
    for (args, want) in [
        (
            &["COPY", "a", "b", "DB", "1"][..],
            "ERR DB index is out of range",
        ),
        (
            &["COPY", "a", "b", "DB", "x"],
            "ERR value is not an integer or out of range",
        ),
        (&["COPY", "a", "b", "DB"], "ERR syntax error"),
        (&["COPY", "a", "b", "bogus"], "ERR syntax error"),
        (
            &["COPY", "a", "a"],
            "ERR source and destination objects are the same",
        ),
        (
            &["COPY", "a"],
            "ERR wrong number of arguments for 'copy' command",
        ),
    ] {
        assert_eq!(parse(args).unwrap_err(), want, "{args:?}");
    }
}

#[test]
fn copy_duplicates_every_type_with_its_expiry() {
    let s = KeyValueStore::new();
    set(&s, "str", "v");
    s.execute(Command::PExpireAt("str".into(), 4_102_444_800_000));
    s.execute(Command::HSet("hash".into(), vec![("f".into(), "v".into())]));
    s.execute(Command::RPush("list".into(), vec!["a".into(), "b".into()]));
    s.execute(Command::SAdd("set".into(), vec!["m".into()]));
    s.execute(Command::ZAdd(
        "zset".into(),
        Default::default(),
        vec![(1.5, "m".into())],
    ));
    s.execute(Command::JSet(
        "json".into(),
        "$".into(),
        r#"{"a":1}"#.into(),
    ));
    for k in ["str", "hash", "list", "set", "zset", "json"] {
        let dst = format!("{k}-copy");
        assert_eq!(
            s.execute(Command::Copy(k.into(), dst.clone(), false)),
            int(1),
            "{k}"
        );
        assert_eq!(
            s.execute(Command::Type(k.into())),
            s.execute(Command::Type(dst.clone())),
            "{k}"
        );
    }
    assert_eq!(
        s.execute(Command::PExpireTime("str-copy".into())),
        int(4_102_444_800_000)
    );
    assert_eq!(
        s.execute(Command::LRange("list-copy".into(), 0, -1)),
        arr(&["a", "b"])
    );
    // The copy is independent: writing it leaves the source alone.
    s.execute(Command::HSet(
        "hash-copy".into(),
        vec![("f".into(), "changed".into())],
    ));
    assert_eq!(
        s.execute(Command::HGet("hash".into(), "f".into())),
        bulk("v")
    );
}

#[test]
fn copy_respects_replace_and_missing_sources() {
    let s = KeyValueStore::new();
    set(&s, "a", "1");
    set(&s, "d", "old");
    assert_eq!(
        s.execute(Command::Copy("a".into(), "d".into(), false)),
        int(0)
    );
    assert_eq!(s.execute(Command::Get("d".into())), bulk("old"));
    assert_eq!(
        s.execute(Command::Copy("a".into(), "d".into(), true)),
        int(1)
    );
    assert_eq!(s.execute(Command::Get("d".into())), bulk("1"));
    assert_eq!(
        s.execute(Command::Copy("nosuch".into(), "e".into(), true)),
        int(0)
    );
    // REPLACE across types: the destination takes the source's type.
    s.execute(Command::HSet("h".into(), vec![("f".into(), "v".into())]));
    assert_eq!(
        s.execute(Command::Copy("a".into(), "h".into(), true)),
        int(1)
    );
    assert_eq!(
        s.execute(Command::Type("h".into())),
        Value::SimpleString("string".into())
    );
}

#[test]
fn copy_is_charged_to_memory_accounting_and_capped() {
    let s = KeyValueStore::new();
    set(&s, "a", &"x".repeat(10_000));
    let one = s.approximate_memory_bytes();
    s.execute(Command::Copy("a".into(), "b".into(), false));
    assert!(s.approximate_memory_bytes() >= one + 10_000);

    // The key cap counts the destination.
    let capped = KeyValueStore::with_max_keys(1);
    set(&capped, "a", "1");
    let reply = capped.execute(Command::Copy("a".into(), "b".into(), false));
    assert!(matches!(reply, Value::Error(_)), "{reply:?}");
    assert_eq!(capped.execute(Command::DbSize), int(1));
}

// ── HSTRLEN ──────────────────────────────────────────────────────────────────

#[test]
fn hstrlen_measures_bytes() {
    let s = KeyValueStore::new();
    s.execute(Command::HSet(
        "h".into(),
        vec![("f".into(), "héllo".into()), ("e".into(), "".into())],
    ));
    assert_eq!(s.execute(Command::HStrLen("h".into(), "f".into())), int(6));
    assert_eq!(s.execute(Command::HStrLen("h".into(), "e".into())), int(0));
    assert_eq!(
        s.execute(Command::HStrLen("h".into(), "nope".into())),
        int(0)
    );
    assert_eq!(
        s.execute(Command::HStrLen("nope".into(), "f".into())),
        int(0)
    );
    set(&s, "str", "v");
    assert!(is_err(
        &s.execute(Command::HStrLen("str".into(), "f".into())),
        "WRONGTYPE"
    ));
}

// ── ZPOPMIN / ZPOPMAX ────────────────────────────────────────────────────────

#[test]
fn zpop_takes_from_each_end_in_score_then_member_order() {
    let s = KeyValueStore::new();
    s.execute(Command::ZAdd(
        "z".into(),
        Default::default(),
        vec![
            (1.0, "a".into()),
            (1.0, "b".into()),
            (2.0, "c".into()),
            (3.0, "d".into()),
        ],
    ));
    assert_eq!(
        s.execute(Command::ZPopMin("z".into(), None)),
        arr(&["a", "1"])
    );
    assert_eq!(
        s.execute(Command::ZPopMax("z".into(), None)),
        arr(&["d", "3"])
    );
    assert_eq!(s.execute(Command::ZPopMin("z".into(), Some(0))), arr(&[]));
    assert_eq!(
        s.execute(Command::ZPopMin("z".into(), Some(5))),
        arr(&["b", "1", "c", "2"])
    );
    // Emptied: the key is gone, as in Redis.
    assert_eq!(s.execute(Command::Exists(vec!["z".into()])), int(0));
    assert_eq!(s.execute(Command::ZPopMin("z".into(), Some(2))), arr(&[]));
    s.execute(Command::ZAdd(
        "f".into(),
        Default::default(),
        vec![(1.5, "x".into())],
    ));
    assert_eq!(
        s.execute(Command::ZPopMax("f".into(), None)),
        arr(&["x", "1.5"])
    );
    set(&s, "str", "v");
    assert!(is_err(
        &s.execute(Command::ZPopMin("str".into(), None)),
        "WRONGTYPE"
    ));
}

#[test]
fn zpop_counts_parse_like_redis() {
    assert_eq!(
        parse(&["ZPOPMIN", "z"]),
        Ok(Command::ZPopMin("z".into(), None))
    );
    assert_eq!(
        parse(&["ZPOPMAX", "z", "3"]),
        Ok(Command::ZPopMax("z".into(), Some(3)))
    );
    for args in [&["ZPOPMIN", "z", "-1"][..], &["ZPOPMIN", "z", "notanumber"]] {
        assert_eq!(
            parse(args).unwrap_err(),
            "ERR value is out of range, must be positive",
            "{args:?}"
        );
    }
    assert_eq!(
        parse(&["ZPOPMIN", "z", "1", "2"]).unwrap_err(),
        "ERR syntax error"
    );
}

#[test]
fn a_huge_zpop_count_costs_the_set_not_the_count() {
    // u64::MAX must neither allocate nor loop beyond the set's size.
    let s = KeyValueStore::new();
    s.execute(Command::ZAdd(
        "z".into(),
        Default::default(),
        vec![(1.0, "a".into())],
    ));
    assert_eq!(
        s.execute(Command::ZPopMin("z".into(), Some(u64::MAX))),
        arr(&["a", "1"])
    );
}

#[test]
fn concurrent_zpop_never_hands_out_a_member_twice() {
    let store = Arc::new(KeyValueStore::new());
    let members: Vec<(f64, String)> = (0..2_000)
        .map(|i| (f64::from(i), format!("m{i}")))
        .collect();
    store.execute(Command::ZAdd("z".into(), Default::default(), members));
    let popped: Vec<String> = (0..8)
        .map(|t| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || {
                let mut mine = Vec::new();
                loop {
                    let cmd = if t % 2 == 0 {
                        Command::ZPopMin("z".into(), Some(7))
                    } else {
                        Command::ZPopMax("z".into(), Some(5))
                    };
                    let Value::Array(Some(items)) = store.execute(cmd) else {
                        panic!()
                    };
                    if items.is_empty() {
                        return mine;
                    }
                    for item in items.iter().step_by(2) {
                        if let Value::BulkString(Some(m)) = item {
                            mine.push(String::from_utf8(m.clone()).unwrap());
                        }
                    }
                }
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();
    let unique: std::collections::HashSet<_> = popped.iter().collect();
    assert_eq!(popped.len(), 2_000);
    assert_eq!(unique.len(), 2_000);
}

#[test]
fn echo_returns_binary_messages_byte_for_byte() {
    // `redis-cli --pipe` ends a mass insert with ECHO and 20 random bytes,
    // and waits for them to come back; refusing non-UTF-8 here broke it.
    let raw: Vec<u8> = (200u8..220).collect();
    let cmd = Command::from_value(Value::Array(Some(vec![
        bulk("ECHO"),
        Value::BulkString(Some(raw.clone())),
    ])))
    .unwrap();
    assert_eq!(
        KeyValueStore::new().execute(cmd),
        Value::BulkString(Some(raw))
    );
    assert!(parse(&["ECHO"]).is_err());
    assert!(parse(&["ECHO", "a", "b"]).is_err());
}

#[test]
fn per_entry_layout_does_not_regrow() {
    // Every key in the store pays for these bytes. v0.5.1 had Entry = 72 and
    // Blob = 40; v0.6 measured the cost at 1M keys and shrank both (Deadline
    // instead of Option<u64>, smallvec's `union` layout). Growing them again
    // should be a decision, not an accident.
    use std::mem::size_of;
    assert_eq!(size_of::<Blob>(), 32, "Blob");
    assert_eq!(size_of::<Deadline>(), 8, "Deadline");
    assert!(
        size_of::<Entry>() <= 56,
        "Entry is {} bytes",
        size_of::<Entry>()
    );
}

#[test]
fn a_zero_deadline_is_stored_as_already_past() {
    assert_eq!(Deadline::at(0).get(), Some(1));
    assert_eq!(Deadline::from(None).get(), None);
    assert_eq!(
        Deadline::from(Some(1_700_000_000_000)).get(),
        Some(1_700_000_000_000)
    );
    let s = KeyValueStore::new();
    set(&s, "k", "v");
    s.execute(Command::PExpireAt("k".into(), 0));
    assert_eq!(s.execute(Command::Exists(vec!["k".into()])), int(0));
}
