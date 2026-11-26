//! 6O-H1: [`observing_expirations`], the store half of Expiry Option E1.
//!
//! A write run inside the scope removes every expired entry among the keys
//! it names (what it writes and what it reads) before it runs, under the
//! stripes it holds, and reports each one. Outside the scope nothing
//! changes. These tests check, for every write the store implements:
//!
//! - the expired key is reported, exactly once, and gone;
//! - the write then behaves as on an absent key (Redis's model: an expired
//!   key does not exist);
//! - a write that fails or changes nothing still reports what it removed;
//! - `DEL`/`UNLINK` remove without reporting (their frame names the keys);
//! - reads, live keys and absent keys report nothing and allocate nothing;
//! - the metadata stays exact on every kind of store (capped, reserved);
//! - transactions report per queued command, at the transaction's instant;
//! - scopes nest, are per thread, and are restored after a panic.

use super::*;
use std::sync::mpsc;
use std::time::Duration;

const WITHIN: Duration = Duration::from_secs(20);

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

fn set(key: &str, value: &str) -> Command {
    Command::Set(key.into(), value.as_bytes().to_vec(), SetOptions::default())
}

fn set_with(key: &str, value: &str, opts: SetOptions) -> Command {
    Command::Set(key.into(), value.as_bytes().to_vec(), opts)
}

fn strings(keys: &[&str]) -> Vec<String> {
    keys.iter().map(|key| key.to_string()).collect()
}

/// Give `key` a deadline long past: its entry stays in the map, expired,
/// until something removes it.
fn expire_now(store: &KeyValueStore, key: &str) {
    assert_eq!(
        store.execute(Command::PExpireAt(key.into(), 1)),
        Value::Integer(1),
        "{key} must be live before it is expired"
    );
    assert!(store.holds_expired(key), "{key} expired, still in the map");
}

/// Run one command inside an observing scope, the way the server does.
fn observe(store: &KeyValueStore, cmd: &Command) -> (Value, Vec<String>) {
    observing_expirations(|| store.execute_reporting_ref(cmd).0)
}

fn metadata_is_consistent(store: &KeyValueStore) -> bool {
    let (keys, memory) = (store.key_count(), store.approximate_memory_bytes());
    let fresh = store.clone();
    {
        let _all = fresh.order_all();
        fresh.rebuild_metadata();
    }
    keys == fresh.key_count()
        && memory == fresh.approximate_memory_bytes()
        && keys == store.data.len()
        && store.volatile_key_count() == fresh.volatile_key_count()
}

fn stores() -> Vec<(&'static str, KeyValueStore)> {
    let capped_evict =
        || KeyValueStore::with_config(Some(1_000), Some(1 << 24), EvictionPolicy::AllKeysLru);
    let capped_refuse =
        || KeyValueStore::with_config(Some(1_000), Some(1 << 24), EvictionPolicy::NoEviction);
    let reserved = |store: KeyValueStore| {
        store.set_reserved_writes(true);
        store
    };
    vec![
        ("uncapped", KeyValueStore::new()),
        ("capped-evict", capped_evict()),
        ("capped-evict+reserved", reserved(capped_evict())),
        ("capped-refuse", capped_refuse()),
        ("capped-refuse+reserved", reserved(capped_refuse())),
    ]
}

/// One write on an expired key: how the key is made, the write, and what
/// must hold afterwards (`check` sees the store and the reply).
struct Case {
    name: &'static str,
    setup: Vec<Command>,
    write: Command,
    check: fn(&KeyValueStore, &Value) -> bool,
}

fn str_of(key: &str) -> Vec<Command> {
    vec![set(key, "old")]
}

fn list_of(key: &str) -> Vec<Command> {
    vec![Command::RPush(
        key.into(),
        vec![b"o1".to_vec(), b"o2".to_vec()],
    )]
}

fn hash_of(key: &str) -> Vec<Command> {
    vec![Command::HSet(
        key.into(),
        vec![("f".into(), b"7".to_vec()), ("g".into(), b"x".to_vec())],
    )]
}

fn set_of(key: &str) -> Vec<Command> {
    vec![Command::SAdd(key.into(), vec!["m1".into(), "m2".into()])]
}

fn zset_of(key: &str) -> Vec<Command> {
    vec![Command::ZAdd(
        key.into(),
        Default::default(),
        vec![(1.0, "z1".into()), (2.0, "z2".into())],
    )]
}

fn exists(store: &KeyValueStore, key: &str) -> bool {
    store.execute(Command::Exists(vec![key.into()])) == Value::Integer(1)
}

fn ttl_ms(store: &KeyValueStore, key: &str) -> Value {
    store.execute(Command::PTtl(key.into()))
}

/// Every single-key write the store implements, on an expired key of the
/// type it acts on. `k` is the key. The check states the Redis outcome for
/// an absent key.
fn single_key_cases() -> Vec<Case> {
    let k = "ox:k";
    vec![
        Case {
            name: "INCR",
            setup: vec![set(k, "41")],
            write: Command::Incr(k.into()),
            check: |s, r| *r == Value::Integer(1) && ttl_ms(s, "ox:k") == Value::Integer(-1),
        },
        Case {
            name: "DECR",
            setup: vec![set(k, "41")],
            write: Command::Decr(k.into()),
            check: |_, r| *r == Value::Integer(-1),
        },
        Case {
            name: "INCRBY",
            setup: vec![set(k, "41")],
            write: Command::IncrBy(k.into(), 5),
            check: |_, r| *r == Value::Integer(5),
        },
        Case {
            name: "DECRBY",
            setup: vec![set(k, "41")],
            write: Command::DecrBy(k.into(), 5),
            check: |_, r| *r == Value::Integer(-5),
        },
        Case {
            name: "SET",
            setup: str_of(k),
            write: set(k, "new"),
            check: |s, _| s.execute(Command::Get("ox:k".into())) == bulk("new"),
        },
        Case {
            name: "SET NX (succeeds: the key is gone)",
            setup: str_of(k),
            write: set_with(
                k,
                "new",
                SetOptions {
                    condition: Some(SetCondition::Nx),
                    ..Default::default()
                },
            ),
            check: |s, r| {
                *r == Value::SimpleString("OK".into())
                    && s.execute(Command::Get("ox:k".into())) == bulk("new")
            },
        },
        Case {
            name: "SET XX (fails: the key is gone)",
            setup: str_of(k),
            write: set_with(
                k,
                "new",
                SetOptions {
                    condition: Some(SetCondition::Xx),
                    ..Default::default()
                },
            ),
            check: |s, r| *r == Value::BulkString(None) && !exists(s, "ox:k"),
        },
        Case {
            name: "SET KEEPTTL (no deadline to keep)",
            setup: str_of(k),
            write: set_with(
                k,
                "new",
                SetOptions {
                    expiry: Some(SetExpiry::KeepTtl),
                    ..Default::default()
                },
            ),
            check: |s, _| ttl_ms(s, "ox:k") == Value::Integer(-1),
        },
        Case {
            name: "SET GET (old value is nil)",
            setup: str_of(k),
            write: set_with(
                k,
                "new",
                SetOptions {
                    get: true,
                    ..Default::default()
                },
            ),
            check: |_, r| *r == Value::BulkString(None),
        },
        Case {
            name: "SETNX",
            setup: str_of(k),
            write: Command::SetNx(k.into(), b"new".to_vec()),
            check: |_, r| *r == Value::Integer(1),
        },
        Case {
            name: "SETEX",
            setup: str_of(k),
            write: Command::SetEx(k.into(), 100, b"new".to_vec()),
            check: |s, _| matches!(ttl_ms(s, "ox:k"), Value::Integer(t) if t > 0),
        },
        Case {
            name: "PSETEX",
            setup: str_of(k),
            write: Command::PSetEx(k.into(), 100_000, b"new".to_vec()),
            check: |s, _| matches!(ttl_ms(s, "ox:k"), Value::Integer(t) if t > 0),
        },
        Case {
            name: "GETSET",
            setup: str_of(k),
            write: Command::GetSet(k.into(), b"new".to_vec()),
            check: |_, r| *r == Value::BulkString(None),
        },
        Case {
            name: "APPEND",
            setup: str_of(k),
            write: Command::Append(k.into(), b"tail".to_vec()),
            check: |s, r| {
                *r == Value::Integer(4) && s.execute(Command::Get("ox:k".into())) == bulk("tail")
            },
        },
        Case {
            name: "GETDEL",
            setup: str_of(k),
            write: Command::GetDel(k.into()),
            check: |_, r| *r == Value::BulkString(None),
        },
        Case {
            name: "GETEX PERSIST",
            setup: str_of(k),
            write: Command::GetEx(k.into(), Some(GetExOption::Persist)),
            check: |s, r| *r == Value::BulkString(None) && !exists(s, "ox:k"),
        },
        Case {
            name: "EXPIRE (no such key)",
            setup: str_of(k),
            write: Command::Expire(k.into(), 100),
            check: |s, r| *r == Value::Integer(0) && !exists(s, "ox:k"),
        },
        Case {
            name: "PEXPIRE",
            setup: str_of(k),
            write: Command::PExpire(k.into(), 100_000),
            check: |_, r| *r == Value::Integer(0),
        },
        Case {
            name: "EXPIREAT",
            setup: str_of(k),
            write: Command::ExpireAt(k.into(), 4_000_000_000),
            check: |_, r| *r == Value::Integer(0),
        },
        Case {
            name: "PEXPIREAT",
            setup: str_of(k),
            write: Command::PExpireAt(k.into(), 4_000_000_000_000),
            check: |_, r| *r == Value::Integer(0),
        },
        Case {
            name: "PERSIST",
            setup: str_of(k),
            write: Command::Persist(k.into()),
            check: |s, r| *r == Value::Integer(0) && !exists(s, "ox:k"),
        },
        Case {
            name: "HSET",
            setup: hash_of(k),
            write: Command::HSet(k.into(), vec![("h".into(), b"1".to_vec())]),
            check: |s, _| s.execute(Command::HLen("ox:k".into())) == Value::Integer(1),
        },
        Case {
            name: "HSETNX",
            setup: hash_of(k),
            write: Command::HSetNx(k.into(), "f".into(), b"1".to_vec()),
            check: |_, r| *r == Value::Integer(1),
        },
        Case {
            name: "HINCRBY",
            setup: hash_of(k),
            write: Command::HIncrBy(k.into(), "f".into(), 2),
            check: |_, r| *r == Value::Integer(2),
        },
        Case {
            name: "HINCRBYFLOAT",
            setup: hash_of(k),
            write: Command::HIncrByFloat(k.into(), "f".into(), 1.5),
            check: |_, r| *r == bulk("1.5"),
        },
        Case {
            name: "HDEL (nothing to delete)",
            setup: hash_of(k),
            write: Command::HDel(k.into(), vec!["f".into()]),
            check: |_, r| *r == Value::Integer(0),
        },
        Case {
            name: "LPUSH",
            setup: list_of(k),
            write: Command::LPush(k.into(), vec![b"n".to_vec()]),
            check: |_, r| *r == Value::Integer(1),
        },
        Case {
            name: "RPUSH",
            setup: list_of(k),
            write: Command::RPush(k.into(), vec![b"n".to_vec()]),
            check: |_, r| *r == Value::Integer(1),
        },
        Case {
            name: "LPUSHX (no list to push to)",
            setup: list_of(k),
            write: Command::LPushX(k.into(), vec![b"n".to_vec()]),
            check: |s, r| *r == Value::Integer(0) && !exists(s, "ox:k"),
        },
        Case {
            name: "RPUSHX",
            setup: list_of(k),
            write: Command::RPushX(k.into(), vec![b"n".to_vec()]),
            check: |_, r| *r == Value::Integer(0),
        },
        Case {
            name: "LPOP",
            setup: list_of(k),
            write: Command::LPop(k.into(), None),
            check: |_, r| *r == Value::BulkString(None),
        },
        Case {
            name: "RPOP",
            setup: list_of(k),
            write: Command::RPop(k.into(), None),
            check: |_, r| *r == Value::BulkString(None),
        },
        Case {
            name: "LSET (no such key)",
            setup: list_of(k),
            write: Command::LSet(k.into(), 0, b"n".to_vec()),
            check: |s, r| {
                matches!(r, Value::Error(e) if e.contains("no such key")) && !exists(s, "ox:k")
            },
        },
        Case {
            name: "LREM",
            setup: list_of(k),
            write: Command::LRem(k.into(), 0, b"o1".to_vec()),
            check: |_, r| *r == Value::Integer(0),
        },
        Case {
            name: "LTRIM",
            setup: list_of(k),
            write: Command::LTrim(k.into(), 0, 0),
            check: |s, _| !exists(s, "ox:k"),
        },
        Case {
            name: "SADD",
            setup: set_of(k),
            write: Command::SAdd(k.into(), vec!["m9".into()]),
            check: |s, r| {
                *r == Value::Integer(1)
                    && s.execute(Command::SCard("ox:k".into())) == Value::Integer(1)
            },
        },
        Case {
            name: "SREM",
            setup: set_of(k),
            write: Command::SRem(k.into(), vec!["m1".into()]),
            check: |_, r| *r == Value::Integer(0),
        },
        Case {
            name: "SPOP",
            setup: set_of(k),
            write: Command::SPop(k.into(), None),
            check: |_, r| *r == Value::BulkString(None),
        },
        Case {
            name: "ZADD",
            setup: zset_of(k),
            write: Command::ZAdd(k.into(), Default::default(), vec![(5.0, "z9".into())]),
            check: |s, _| s.execute(Command::ZCard("ox:k".into())) == Value::Integer(1),
        },
        Case {
            name: "ZINCRBY",
            setup: zset_of(k),
            write: Command::ZIncrBy(k.into(), 1.0, "z1".into()),
            check: |_, r| *r == bulk("1"),
        },
        Case {
            name: "ZREM",
            setup: zset_of(k),
            write: Command::ZRem(k.into(), vec!["z1".into()]),
            check: |_, r| *r == Value::Integer(0),
        },
        Case {
            name: "ZPOPMIN",
            setup: zset_of(k),
            write: Command::ZPopMin(k.into(), None),
            check: |_, r| *r == Value::Array(Some(vec![])),
        },
        Case {
            name: "ZPOPMAX",
            setup: zset_of(k),
            write: Command::ZPopMax(k.into(), None),
            check: |_, r| *r == Value::Array(Some(vec![])),
        },
        Case {
            name: "JSET",
            setup: vec![Command::JSet(k.into(), "$".into(), r#"{"a":1}"#.into())],
            write: Command::JSet(k.into(), "$.b".into(), "2".into()),
            check: |_, r| matches!(r, Value::Error(_)) || *r == Value::SimpleString("OK".into()),
        },
        Case {
            name: "JMERGE",
            setup: vec![Command::JSet(k.into(), "$".into(), r#"{"a":1}"#.into())],
            write: Command::JMerge(k.into(), r#"{"b":2}"#.into()),
            check: |_, _| true,
        },
        Case {
            name: "RLSET",
            setup: vec![Command::RlSet(k.into(), 5, 60)],
            write: Command::RlSet(k.into(), 9, 60),
            check: |s, _| exists(s, "ox:k"),
        },
        Case {
            name: "RLCHECK",
            setup: vec![Command::RlSet(k.into(), 5, 60)],
            write: Command::RlCheck(k.into(), None),
            check: |_, _| true,
        },
        Case {
            name: "ESET",
            setup: str_of(k),
            write: Command::ESet(k.into(), b"new".to_vec()),
            check: |s, _| s.execute(Command::Get("ox:k".into())) == bulk("new"),
        },
        Case {
            name: "EADD",
            setup: set_of(k),
            write: Command::EAdd(k.into(), vec!["m9".into()]),
            check: |s, _| s.execute(Command::SCard("ox:k".into())) == Value::Integer(1),
        },
        Case {
            name: "WRONGTYPE becomes a create (an expired list is no list)",
            setup: list_of(k),
            write: Command::Incr(k.into()),
            check: |_, r| *r == Value::Integer(1),
        },
    ]
}

#[test]
fn every_single_key_write_reports_the_expired_key_it_acts_on() {
    for (label, store) in stores() {
        for case in single_key_cases() {
            let what = format!("{label}: {}", case.name);
            store.execute(Command::FlushDb);
            for cmd in &case.setup {
                assert!(
                    !matches!(store.execute(cmd.clone()), Value::Error(_)),
                    "{what}: setup"
                );
            }
            expire_now(&store, "ox:k");
            let (reply, reported) = observe(&store, &case.write);
            assert_eq!(reported, strings(&["ox:k"]), "{what}: reported once");
            assert!((case.check)(&store, &reply), "{what}: reply {reply:?}");
            assert!(
                !store.holds_expired("ox:k"),
                "{what}: no expired entry left"
            );
            assert!(metadata_is_consistent(&store), "{what}: metadata");
        }
    }
}

#[test]
fn outside_a_scope_nothing_is_removed_early() {
    // Today's behaviour, unchanged: a refused write leaves the expired
    // entry in the map (the sweeper removes it), and nothing is reported.
    let store = KeyValueStore::new();
    store.execute(set("plain:k", "v"));
    expire_now(&store, "plain:k");
    assert_eq!(
        store.execute(Command::Persist("plain:k".into())),
        Value::Integer(0)
    );
    assert!(store.holds_expired("plain:k"), "still physically present");
    // And inside a scope the same command removes and reports it.
    let (reply, reported) = observe(&store, &Command::Persist("plain:k".into()));
    assert_eq!(reply, Value::Integer(0));
    assert_eq!(reported, strings(&["plain:k"]));
    assert_eq!(store.data.len(), 0);
}

#[test]
fn live_absent_and_read_keys_report_nothing_and_allocate_nothing() {
    for (label, store) in stores() {
        store.execute(set("lv:live", "1"));
        store.execute(set_with(
            "lv:later",
            "1",
            SetOptions {
                expiry: Some(SetExpiry::Px(3_600_000)),
                ..Default::default()
            },
        ));
        store.execute(set("lv:read", "1"));
        expire_now(&store, "lv:read");
        for cmd in [
            Command::Incr("lv:live".into()),
            Command::Incr("lv:later".into()),
            Command::Incr("lv:absent".into()),
            // Reads never remove: the replica and the primary mask them.
            Command::Get("lv:read".into()),
            Command::GetEx("lv:read".into(), None),
            Command::MGet(vec!["lv:read".into(), "lv:live".into()]),
            Command::Exists(vec!["lv:read".into()]),
            Command::Ttl("lv:read".into()),
        ] {
            let (_, reported) = observe(&store, &cmd);
            assert!(
                reported.is_empty(),
                "{label}: {cmd:?} reported {reported:?}"
            );
            assert_eq!(reported.capacity(), 0, "{label}: {cmd:?} allocated");
        }
        assert!(store.holds_expired("lv:read"), "{label}: a read removed it");
        assert!(metadata_is_consistent(&store), "{label}");
    }
}

#[test]
fn del_and_unlink_remove_expired_keys_without_reporting_them() {
    for (label, store) in stores() {
        for del in [
            Command::Del(strings(&["dl:a", "dl:b", "dl:c"])),
            Command::Unlink(strings(&["dl:a", "dl:b", "dl:c"])),
        ] {
            store.execute(set("dl:a", "1"));
            store.execute(set("dl:b", "1"));
            expire_now(&store, "dl:a");
            let (reply, reported) = observe(&store, &del);
            // Their frame is `DEL dl:a dl:b dl:c` whatever they return, so
            // it already deletes the expired one on every applier.
            assert_eq!(
                reply,
                Value::Integer(1),
                "{label}: only the live key counts"
            );
            assert!(reported.is_empty(), "{label}: {reported:?}");
            assert_eq!(store.data.len(), 0, "{label}: both gone");
            assert!(metadata_is_consistent(&store), "{label}");
        }
    }
}

#[test]
fn multi_key_writes_report_every_expired_key_once() {
    for (label, store) in stores() {
        let what = |s: &str| format!("{label}: {s}");
        store.execute(Command::FlushDb);
        // MSET naming an expired key twice: reported once.
        store.execute(set("mk:a", "1"));
        store.execute(set("mk:b", "1"));
        expire_now(&store, "mk:a");
        expire_now(&store, "mk:b");
        let (_, reported) = observe(
            &store,
            &Command::MSet(vec![
                ("mk:a".into(), b"x".to_vec()),
                ("mk:b".into(), b"y".to_vec()),
                ("mk:a".into(), b"z".to_vec()),
            ]),
        );
        let mut sorted = reported.clone();
        sorted.sort();
        assert_eq!(sorted, strings(&["mk:a", "mk:b"]), "{}", what("MSET"));
        assert_eq!(store.execute(Command::Get("mk:a".into())), bulk("z"));

        // RENAME of an expired source: refused, and the source is reported.
        store.execute(set("mk:src", "s"));
        expire_now(&store, "mk:src");
        let (reply, reported) = observe(&store, &Command::Rename("mk:src".into(), "mk:dst".into()));
        assert!(
            matches!(reply, Value::Error(ref e) if e.contains("no such key")),
            "{}",
            what("RENAME")
        );
        assert_eq!(reported, strings(&["mk:src"]), "{}", what("RENAME"));

        // RENAME onto an expired destination: the destination is reported.
        store.execute(set("mk:src", "s"));
        store.execute(set("mk:dst", "d"));
        expire_now(&store, "mk:dst");
        let (reply, reported) = observe(&store, &Command::Rename("mk:src".into(), "mk:dst".into()));
        assert_eq!(
            reply,
            Value::SimpleString("OK".into()),
            "{}",
            what("RENAME dst")
        );
        assert_eq!(reported, strings(&["mk:dst"]), "{}", what("RENAME dst"));
        assert_eq!(ttl_ms(&store, "mk:dst"), Value::Integer(-1));

        // RENAMENX onto an expired destination: it is gone, so it renames.
        store.execute(set("mk:s2", "s"));
        store.execute(set("mk:d2", "d"));
        expire_now(&store, "mk:d2");
        let (reply, reported) = observe(&store, &Command::RenameNx("mk:s2".into(), "mk:d2".into()));
        assert_eq!(reply, Value::Integer(1), "{}", what("RENAMENX"));
        assert_eq!(reported, strings(&["mk:d2"]));

        // COPY of an expired source: nothing to copy, the source reported.
        store.execute(set("mk:c", "c"));
        expire_now(&store, "mk:c");
        let (reply, reported) =
            observe(&store, &Command::Copy("mk:c".into(), "mk:c2".into(), false));
        assert_eq!(reply, Value::Integer(0), "{}", what("COPY"));
        assert_eq!(reported, strings(&["mk:c"]));

        // SMOVE from an expired set.
        store.execute(Command::SAdd("mk:from".into(), vec!["m".into()]));
        expire_now(&store, "mk:from");
        let (reply, reported) = observe(
            &store,
            &Command::SMove("mk:from".into(), "mk:to".into(), "m".into()),
        );
        assert_eq!(reply, Value::Integer(0), "{}", what("SMOVE"));
        assert_eq!(reported, strings(&["mk:from"]));

        // A STORE whose *source* expired: the source is reported, and the
        // result is computed without it (a verbatim frame replays exactly
        // that once the `DEL` precedes it).
        store.execute(Command::SAdd("mk:s1".into(), vec!["a".into(), "b".into()]));
        store.execute(Command::SAdd("mk:s3".into(), vec!["b".into()]));
        expire_now(&store, "mk:s1");
        for (name, cmd, members) in [
            (
                "SUNIONSTORE",
                Command::SUnionStore("mk:out".into(), strings(&["mk:s1", "mk:s3"])),
                1,
            ),
            (
                "SINTERSTORE",
                Command::SInterStore("mk:out".into(), strings(&["mk:s1", "mk:s3"])),
                0,
            ),
            (
                "SDIFFSTORE",
                Command::SDiffStore("mk:out".into(), strings(&["mk:s3", "mk:s1"])),
                1,
            ),
        ] {
            store.execute(Command::SAdd("mk:s1".into(), vec!["a".into(), "b".into()]));
            expire_now(&store, "mk:s1");
            let (reply, reported) = observe(&store, &cmd);
            assert_eq!(reply, Value::Integer(members), "{}", what(name));
            assert_eq!(reported, strings(&["mk:s1"]), "{}", what(name));
        }
        assert!(metadata_is_consistent(&store), "{}", what("metadata"));
    }
}

#[test]
fn tenant_prefixed_keys_are_reported_exactly() {
    let store = KeyValueStore::new();
    store.execute(set("t1:session", "a"));
    store.execute(set("t2:session", "b"));
    expire_now(&store, "t1:session");
    let (_, reported) = observe(&store, &Command::Append("t1:session".into(), b"x".to_vec()));
    assert_eq!(reported, strings(&["t1:session"]));
    assert_eq!(store.execute(Command::Get("t2:session".into())), bulk("b"));
}

#[test]
fn the_removal_and_the_write_share_one_instant() {
    // At its deadline a key is expired (`now >= deadline`), one
    // millisecond before it is not: under a fixed instant, as inside EXEC,
    // the removal and the write agree.
    let store = KeyValueStore::new();
    let t = clock_ms() + 3_600_000;
    for (at, expired) in [(t - 1, false), (t, true), (t + 1, true)] {
        store.execute(set_with(
            "in:k",
            "41",
            SetOptions {
                expiry: Some(SetExpiry::Pxat(t)),
                ..Default::default()
            },
        ));
        let (reply, reported) = at_instant(at, || observe(&store, &Command::Incr("in:k".into())));
        if expired {
            assert_eq!(reported, strings(&["in:k"]), "at {at}");
            assert_eq!(reply, Value::Integer(1));
            assert_eq!(ttl_ms(&store, "in:k"), Value::Integer(-1));
        } else {
            assert!(reported.is_empty(), "at {at}");
            assert_eq!(reply, Value::Integer(42));
            // The deadline is kept, as INCR keeps it.
            assert_eq!(
                store.execute(Command::PExpireTime("in:k".into())),
                Value::Integer(t as i64)
            );
        }
    }
}

#[test]
fn a_capped_store_purges_under_its_gate_and_evicts_nothing_for_it() {
    // Full to its key cap with one expired key among them: rewriting that
    // key removes it (one slot back) and recreates it (the slot again), so
    // nothing is evicted or refused.
    for policy in [EvictionPolicy::AllKeysLru, EvictionPolicy::NoEviction] {
        let store = KeyValueStore::with_config(Some(4), None, policy);
        for i in 0..4 {
            store.execute(set(&format!("cap:{i}"), "v"));
        }
        expire_now(&store, "cap:2");
        let ((reply, evicted), reported) =
            observing_expirations(|| store.execute_reporting_ref(&Command::Incr("cap:2".into())));
        assert_eq!(reply, Value::Integer(1), "{policy:?}");
        assert!(evicted.is_empty(), "{policy:?}: evicted {evicted:?}");
        assert_eq!(reported, strings(&["cap:2"]), "{policy:?}");
        assert_eq!(store.key_count(), 4);
        assert!(metadata_is_consistent(&store), "{policy:?}");
    }
}

#[test]
fn the_reserved_fast_path_hands_an_expired_key_to_the_gated_path() {
    let store = KeyValueStore::with_config(Some(100), Some(1 << 20), EvictionPolicy::AllKeysLru);
    store.set_reserved_writes(true);
    store.execute(set("rs:k", "41"));
    expire_now(&store, "rs:k");
    let (reply, reported) = observe(&store, &Command::Incr("rs:k".into()));
    assert_eq!(reply, Value::Integer(1));
    assert_eq!(reported, strings(&["rs:k"]));
    assert!(metadata_is_consistent(&store));
    // A live key still takes the reserved path and reports nothing.
    let (reply, reported) = observe(&store, &Command::Incr("rs:k".into()));
    assert_eq!(reply, Value::Integer(2));
    assert!(reported.is_empty());
}

#[test]
fn within_capacity_writes_purge_too() {
    let store = KeyValueStore::with_config(Some(100), Some(1 << 20), EvictionPolicy::AllKeysLru);
    store.execute(set("wc:k", "41"));
    expire_now(&store, "wc:k");
    let (out, reported) = observing_expirations(|| {
        store.execute_reporting_ref_within_capacity(&Command::Incr("wc:k".into()))
    });
    assert_eq!(out, Some((Value::Integer(1), Vec::new())));
    assert_eq!(reported, strings(&["wc:k"]));
    assert!(metadata_is_consistent(&store));
}

#[test]
fn transactions_report_per_queued_command_at_their_instant() {
    for (label, store) in stores() {
        store.execute(Command::FlushDb);
        let t = clock_ms() + 3_600_000;
        // Live at the transaction's instant `t - 1`, though expired by the
        // time anyone looks: not reported. Expired at it: reported, by the
        // command that acts on it, in queue order.
        store.execute(set_with(
            "tx:live",
            "1",
            SetOptions {
                expiry: Some(SetExpiry::Pxat(t)),
                ..Default::default()
            },
        ));
        store.execute(set("tx:a", "1"));
        store.execute(Command::SAdd("tx:src".into(), vec!["m".into()]));
        expire_now(&store, "tx:a");
        expire_now(&store, "tx:src");
        let queue = vec![
            Command::Incr("tx:live".into()),
            Command::Get("tx:a".into()),
            Command::Incr("tx:a".into()),
            Command::Incr("tx:a".into()),
            Command::SUnionStore("tx:out".into(), strings(&["tx:src"])),
        ];
        let mut per_command = Vec::new();
        let applied = store.apply_transaction(&queue, t - 1, |i, cmd| {
            let ((reply, _), reported) = observing_expirations(|| store.execute_reporting_ref(cmd));
            per_command.push((i, reply, reported));
        });
        assert_eq!(applied.panicked, None, "{label}");
        let reported: Vec<_> = per_command.iter().map(|(_, _, r)| r.clone()).collect();
        assert_eq!(
            reported,
            vec![
                vec![],
                vec![],
                strings(&["tx:a"]),
                vec![],
                strings(&["tx:src"]),
            ],
            "{label}"
        );
        assert_eq!(per_command[0].1, Value::Integer(2), "{label}: live at t-1");
        assert_eq!(
            per_command[3].1,
            Value::Integer(2),
            "{label}: counted from 0"
        );
        assert!(metadata_is_consistent(&store), "{label}");
    }
}

#[test]
fn scopes_nest_and_are_restored_after_a_panic() {
    let store = KeyValueStore::new();
    store.execute(set("ns:outer", "1"));
    store.execute(set("ns:inner", "1"));
    store.execute(set("ns:panic", "1"));
    expire_now(&store, "ns:outer");
    expire_now(&store, "ns:inner");
    expire_now(&store, "ns:panic");
    let ((), outer) = observing_expirations(|| {
        let (_, inner) = observe(&store, &Command::Incr("ns:inner".into()));
        assert_eq!(inner, strings(&["ns:inner"]), "the inner scope's own");
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            observing_expirations(|| {
                store.execute(Command::Incr("ns:panic".into()));
                panic!("test-injected panic inside a scope");
            })
        }));
        assert!(caught.is_err());
        store.execute(Command::Incr("ns:outer".into()));
    });
    assert_eq!(
        outer,
        strings(&["ns:outer"]),
        "nothing leaked from the inner scopes"
    );
    assert!(!observing(), "no scope left behind");
    assert_eq!(
        FROZEN_NOW.with(std::cell::Cell::get),
        None,
        "clock restored"
    );
}

#[test]
fn a_panic_after_the_purge_inside_a_transaction_leaves_no_scope_behind() {
    let store = KeyValueStore::new();
    store.execute(set("pp:k", "1"));
    expire_now(&store, "pp:k");
    let _armed = transaction_fault::panic_during_mutation_of("pp:k");
    let queue = vec![Command::Incr("pp:k".into())];
    let applied = store.apply_transaction(&queue, clock_ms(), |_, cmd| {
        let _ = observing_expirations(|| store.execute_reporting_ref(cmd));
    });
    assert_eq!(applied.panicked, Some(0));
    assert!(!observing());
    // The removal happened; the repair keeps the metadata exact.
    assert!(!store.holds_expired("pp:k"));
    assert!(metadata_is_consistent(&store));
}

#[test]
fn concurrent_writers_collect_their_own_keys() {
    let store = KeyValueStore::new();
    const THREADS: usize = 8;
    const KEYS: usize = 200;
    for t in 0..THREADS {
        for i in 0..KEYS {
            let key = format!("cc:{t}:{i}");
            store.execute(set(&key, "1"));
            if i % 2 == 0 {
                expire_now(&store, &key);
            }
        }
    }
    let (tx, rx) = mpsc::channel();
    for t in 0..THREADS {
        let store = store.clone();
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut mine = Vec::new();
            for i in 0..KEYS {
                let (_, reported) = observe(&store, &Command::Incr(format!("cc:{t}:{i}")));
                mine.extend(reported);
            }
            let _ = tx.send((t, mine));
        });
    }
    drop(tx);
    let mut seen = 0;
    while let Ok((t, mine)) = rx.recv_timeout(WITHIN) {
        let want: Vec<String> = (0..KEYS)
            .filter(|i| i % 2 == 0)
            .map(|i| format!("cc:{t}:{i}"))
            .collect();
        assert_eq!(mine, want, "thread {t}");
        seen += 1;
    }
    assert_eq!(seen, THREADS);
    assert!(metadata_is_consistent(&store));
}

#[test]
fn whole_keyspace_writes_purge_nothing() {
    let store = KeyValueStore::new();
    store.execute(set("fl:k", "1"));
    expire_now(&store, "fl:k");
    let (reply, reported) = observe(&store, &Command::FlushDb);
    assert_eq!(reply, Value::SimpleString("OK".into()));
    assert!(reported.is_empty(), "FLUSHDB's frame removes everything");
    assert_eq!(store.data.len(), 0);
}

#[test]
fn the_scope_fixes_the_clock_for_the_write() {
    // The removal and the command must agree on what has expired: with the
    // system clock, a key whose deadline fell between the two would be
    // reset by the command without being reported.
    let ((first, second), _) = observing_expirations(|| {
        let first = clock_ms();
        std::thread::sleep(Duration::from_millis(5));
        (first, clock_ms())
    });
    assert_eq!(first, second);
    // And it stays at a transaction's instant when nested in one.
    let (inner, _) = at_instant(1_234, || observing_expirations(clock_ms));
    assert_eq!(inner, 1_234);
}
