//! Single-lookup write accounting (`run_probed_write`) against the general
//! path and against an independent recomputation.
//!
//! A probed write records the key's size and TTL state under the shard guard
//! that performs it instead of looking the key up before and after. These
//! tests hold it to the general path's results — replies, keyspace, memory
//! estimate, key index — and, under concurrency, to a recomputation from the
//! keyspace itself: if the incrementally maintained counters or the index
//! ever drift from what the map holds, `assert_accounting_exact` fails.

use super::*;
use crate::cmd::SetOptions;
use std::sync::Barrier;

/// The store's memory counter and key index, recomputed from the keyspace
/// alone and compared with what the store maintains incrementally.
fn assert_accounting_exact(store: &KeyValueStore, what: &str) {
    let mut memory = 0usize;
    let mut keys = BTreeSet::new();
    let mut volatile = BTreeSet::new();
    for entry in store.data.iter() {
        let key: &str = entry.key();
        memory += entry_size(key, entry.value());
        keys.insert(key.to_string());
        if entry.value().deadline.get().is_some() {
            volatile.insert(key.to_string());
        }
        assert!(
            !entry.value().value.is_empty_collection(),
            "{what}: empty collection {key} left in the keyspace"
        );
    }
    assert_eq!(
        store.approximate_memory_bytes(),
        memory,
        "{what}: memory counter drifted from the keyspace"
    );
    let shards = store.index_all();
    assert_eq!(
        store.key_count(),
        keys.len(),
        "{what}: published key count disagrees with the keyspace"
    );
    for (i, shard) in shards.iter().enumerate() {
        assert!(
            Arc::ptr_eq(&shard.len, &store.key_count[i]),
            "{what}: installed shard {i} publishes to a counter nobody reads"
        );
        for key in &shard.ordered {
            assert_eq!(index_shard(key), i, "{what}: {key} in the wrong shard");
        }
    }
    let ordered: BTreeSet<String> = shards
        .iter()
        .flat_map(|shard| shard.ordered.iter().map(|k| k.to_string()))
        .collect();
    assert_eq!(
        ordered, keys,
        "{what}: ordered index disagrees with the keyspace"
    );
    let indexed_volatile: BTreeSet<String> = shards
        .iter()
        .flat_map(|shard| shard.volatile.keys.iter().map(|k| k.to_string()))
        .collect();
    assert_eq!(
        indexed_volatile, volatile,
        "{what}: TTL index disagrees with the keyspace"
    );
}

fn set(k: &str, v: &[u8], opts: SetOptions) -> Command {
    Command::Set(k.into(), v.to_vec(), opts)
}

fn ex(expiry: SetExpiry) -> SetOptions {
    SetOptions {
        expiry: Some(expiry),
        ..SetOptions::default()
    }
}

/// Every probe-eligible command through every branch that matters to
/// accounting: creation, overwrite, growth and shrink, a TTL gained, kept
/// and lost, an expired key revived, a type clash, a refusal before the
/// slot, a condition that writes nothing, and a vacant slot left vacant.
fn script() -> Vec<Command> {
    let mut cmds = vec![
        set("s", b"1", SetOptions::default()),
        set("s", &[b'x'; 300], SetOptions::default()),
        set("s", b"", SetOptions::default()),
        set("ttl", b"v", ex(SetExpiry::Ex(100))),
        set("ttl", b"vv", ex(SetExpiry::KeepTtl)),
        set("ttl", b"no ttl now", SetOptions::default()),
        set("ttl", b"ttl again", ex(SetExpiry::Px(60_000))),
        set(
            "nx",
            b"a",
            SetOptions {
                condition: Some(SetCondition::Nx),
                ..SetOptions::default()
            },
        ),
        set(
            "nx",
            b"b",
            SetOptions {
                condition: Some(SetCondition::Nx),
                get: true,
                ..SetOptions::default()
            },
        ),
        set(
            "absent",
            b"x",
            SetOptions {
                condition: Some(SetCondition::Xx),
                ..SetOptions::default()
            },
        ),
        set("overflow", b"x", ex(SetExpiry::Ex(u64::MAX))),
        Command::Incr("n".into()),
        Command::IncrBy("n".into(), 1_000_000_007),
        Command::Decr("n".into()),
        Command::DecrBy("n".into(), i64::MIN),
        Command::IncrBy("n".into(), i64::MAX),
        Command::Incr("s".into()),
        Command::Append("a".into(), b"hello".to_vec()),
        Command::Append("a".into(), vec![b'y'; 100]),
        Command::Append("n".into(), b"0".to_vec()),
        Command::HSet("h".into(), vec![("f".into(), b"1".to_vec())]),
        Command::HSet(
            "h".into(),
            vec![("f".into(), b"22".to_vec()), ("g".into(), vec![b'z'; 80])],
        ),
        Command::HIncrBy("h".into(), "count".into(), 5),
        Command::HIncrBy("h".into(), "count".into(), -12),
        Command::HIncrBy("h".into(), "f".into(), i64::MAX),
        Command::HIncrBy("h".into(), "g".into(), 1),
        Command::HSet("s".into(), vec![("f".into(), b"wrongtype".to_vec())]),
        Command::Append("h".into(), b"wrongtype".to_vec()),
        Command::HIncrBy("s".into(), "f".into(), 1),
        set("h", b"a string replaces a hash", SetOptions::default()),
        // 6O-B: the list and set writes, probe-accounted since then. A new
        // key, growth, the inline-to-table switch of a set, re-added members
        // (counted once, not resized), a member named twice in one command,
        // binary list values, both ends, and wrong types either way.
        Command::LPush("l".into(), vec![b"a".to_vec()]),
        Command::LPush(
            "l".into(),
            vec![b"b".to_vec(), vec![b'z'; 200], b"\xff\x00".to_vec()],
        ),
        Command::RPush("l".into(), vec![b"tail".to_vec()]),
        Command::RPush("l2".into(), vec![vec![b'r'; 40]; 3]),
        Command::SAdd("set".into(), vec!["m1".into()]),
        Command::SAdd("set".into(), vec!["m1".into(), "m2".into(), "m2".into()]),
        Command::SAdd(
            "set".into(),
            (0..40).map(|i| format!("member:{i}")).collect(),
        ),
        Command::SAdd(
            "set".into(),
            (0..40).map(|i| format!("member:{i}")).collect(),
        ),
        Command::LPush("s".into(), vec![b"wrongtype".to_vec()]),
        Command::RPush("set".into(), vec![b"wrongtype".to_vec()]),
        Command::SAdd("l".into(), vec!["wrongtype".into()]),
        Command::Expire("l2".into(), 100),
        Command::RPush("l2".into(), vec![b"keeps its ttl".to_vec()]),
        Command::SAdd("ttl".into(), vec!["wrongtype".into()]),
        Command::LPop("l".into(), None),
        // Not probe-eligible: these keep the general path in the mix, on the
        // same keys, so the two paths have to agree with each other.
        Command::Del(vec!["a".into(), "nx".into()]),
        Command::HDel("h2".into(), vec!["x".into()]),
        Command::Expire("s".into(), 100),
        Command::Persist("s".into()),
        Command::MSet(vec![
            ("m".into(), b"1".to_vec()),
            ("s".into(), b"2".to_vec()),
        ]),
    ];
    // A hash grown past the packed representation one field at a time.
    for i in 0..40 {
        cmds.push(Command::HSet(
            "grow".into(),
            vec![(format!("f{i}"), format!("v{i}").into_bytes())],
        ));
        cmds.push(Command::HIncrBy("grow".into(), "n".into(), i));
    }
    cmds
}

/// The probed path (uncapped store) and the general path (a store whose caps
/// can never bind, which takes the general path for every write) must give
/// the same replies and the same keyspace, and both must account exactly.
#[test]
fn probed_writes_account_exactly_like_the_general_path() {
    let probed = KeyValueStore::new();
    let general = KeyValueStore::with_config(Some(usize::MAX), None, EvictionPolicy::NoEviction);
    // The server's entry point, by reference, run through the whole script
    // on one store: every reply and every accounting step must match the
    // probed (owned) store's, on existing keys, wrong types and grown sets.
    let by_ref_persistent = KeyValueStore::new();
    assert!(!probed.has_capacity_limits() && general.has_capacity_limits());
    for (i, cmd) in script().into_iter().enumerate() {
        let what = format!("step {i} {cmd:?}");
        let a = probed.execute_reporting(cmd.clone());
        let b = general.execute_reporting(cmd.clone());
        assert_eq!(a, b, "{what}: replies");
        assert_eq!(
            probed.approximate_memory_bytes(),
            general.approximate_memory_bytes(),
            "{what}: memory"
        );
        assert_eq!(probed.key_count(), general.key_count(), "{what}: key count");
        assert_accounting_exact(&probed, &what);
        assert_accounting_exact(&general, &what);
        let (c, _) = by_ref_persistent.execute_reporting_ref(&cmd);
        assert_eq!(a.0, c, "{what}: reply by reference");
        assert_eq!(
            by_ref_persistent.approximate_memory_bytes(),
            probed.approximate_memory_bytes(),
            "{what}: memory by reference"
        );
        assert_accounting_exact(
            &by_ref_persistent,
            &format!("{what} by reference, cumulative"),
        );
        // The by-reference entry point the server uses takes the same path.
        let by_ref = KeyValueStore::new();
        let _ = by_ref.execute_reporting_ref(&cmd);
        assert_accounting_exact(&by_ref, &format!("{what} by reference"));
    }
}

/// An entry that expired but was not swept is still in the map and in the TTL
/// index. A probed write that revives it must leave both right.
#[test]
fn probed_writes_revive_expired_keys_with_exact_accounting() {
    let store = KeyValueStore::new();
    store.execute(set("e", b"old", ex(SetExpiry::Px(1))));
    store.execute(Command::HSet(
        "eh".into(),
        vec![("f".into(), b"1".to_vec())],
    ));
    store.execute(Command::PExpire("eh".into(), 1));
    std::thread::sleep(std::time::Duration::from_millis(5));
    for cmd in [
        Command::Incr("e".into()),
        Command::HIncrBy("eh".into(), "f".into(), 3),
        Command::Append("e".into(), b"!".to_vec()),
    ] {
        store.execute(cmd);
        assert_accounting_exact(&store, "revived");
    }
    assert_eq!(
        store.execute(Command::Get("e".into())),
        Value::BulkString(Some(b"1!".to_vec()))
    );
    assert_eq!(store.execute(Command::Ttl("e".into())), Value::Integer(-1));
}

/// 6O-B: the same for the list and set writes. A push to an expired list
/// starts a new one (Redis: the expired key is gone), and the TTL index
/// forgets it.
#[test]
fn probed_list_and_set_writes_revive_expired_keys_with_exact_accounting() {
    let store = KeyValueStore::new();
    store.execute(Command::RPush(
        "el".into(),
        vec![b"old".to_vec(), b"older".to_vec()],
    ));
    store.execute(Command::PExpire("el".into(), 1));
    store.execute(Command::SAdd("es".into(), vec!["old".into()]));
    store.execute(Command::PExpire("es".into(), 1));
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert_eq!(
        store.execute(Command::LPush("el".into(), vec![b"new".to_vec()])),
        Value::Integer(1)
    );
    assert_accounting_exact(&store, "revived list");
    assert_eq!(
        store.execute(Command::SAdd("es".into(), vec!["old".into(), "new".into()])),
        Value::Integer(2)
    );
    assert_accounting_exact(&store, "revived set");
    assert_eq!(store.execute(Command::Ttl("el".into())), Value::Integer(-1));
    assert_eq!(store.execute(Command::Ttl("es".into())), Value::Integer(-1));
    // By reference, as the server runs it.
    store.execute(Command::PExpire("el".into(), 1));
    std::thread::sleep(std::time::Duration::from_millis(5));
    let (reply, _) = store.execute_reporting_ref(&Command::RPush("el".into(), vec![b"x".to_vec()]));
    assert_eq!(reply, Value::Integer(1));
    assert_accounting_exact(&store, "revived list by reference");
}

/// Run `threads` workers for `rounds`, each calling `work(thread, round)`,
/// released together so they really overlap.
fn race(threads: usize, rounds: usize, work: impl Fn(usize, usize) + Sync) {
    let barrier = Barrier::new(threads);
    std::thread::scope(|scope| {
        for t in 0..threads {
            let (barrier, work) = (&barrier, &work);
            scope.spawn(move || {
                barrier.wait();
                for r in 0..rounds {
                    work(t, r);
                }
            });
        }
    });
}

#[test]
fn many_writers_on_one_key_account_exactly() {
    let store = KeyValueStore::new();
    race(8, 2_000, |t, r| {
        let cmd = match (t + r) % 5 {
            0 => set("hot", &vec![b'v'; (r * 7) % 200], SetOptions::default()),
            1 => Command::Append("hot".into(), b"++".to_vec()),
            2 => Command::Incr("hot".into()),
            3 => set("hot", b"1", ex(SetExpiry::Ex(100))),
            _ => set("hot", b"2", ex(SetExpiry::KeepTtl)),
        };
        store.execute(cmd);
    });
    assert_accounting_exact(&store, "one hot key");
}

/// 6O-B: pushes and adds racing each other, a type change, a pop and
/// expiry on the same keys, by reference as the server runs them. Every
/// list push lands exactly once (no lost update under the stripe), and the
/// accounting is exact.
#[test]
fn list_and_set_writers_racing_on_shared_keys_account_exactly() {
    let store = KeyValueStore::new();
    let pushes = AtomicUsize::new(0);
    race(8, 2_000, |t, r| {
        let cmd = match (t + r) % 6 {
            0 | 1 => {
                pushes.fetch_add(1, Ordering::Relaxed);
                Command::LPush("q".into(), vec![vec![b'v'; (r * 5) % 90]])
            }
            2 => Command::SAdd(
                "members".into(),
                vec![format!("m{}", r % 50), format!("m{}", t)],
            ),
            3 => Command::RPush("mixed".into(), vec![b"x".to_vec()]),
            4 => Command::SAdd("mixed".into(), vec!["y".into()]),
            _ => Command::PExpire("mixed".into(), 1),
        };
        let _ = store.execute_reporting_ref(&cmd);
    });
    assert_accounting_exact(&store, "list and set writers");
    assert_eq!(
        store.execute(Command::LLen("q".into())),
        Value::Integer(pushes.load(Ordering::Relaxed) as i64),
        "every push landed once"
    );
}

#[test]
fn set_racing_del_accounts_exactly() {
    let store = KeyValueStore::new();
    race(8, 2_000, |t, r| {
        let key = format!("k{}", r % 4);
        if t % 2 == 0 {
            store.execute(set(
                &key,
                &vec![b'x'; (r * 13) % 500],
                SetOptions::default(),
            ));
        } else {
            store.execute(Command::Del(vec![key]));
        }
    });
    assert_accounting_exact(&store, "SET vs DEL");
}

#[test]
fn incr_racing_expiry_accounts_exactly() {
    let store = KeyValueStore::new();
    race(6, 1_500, |t, r| {
        let key = format!("c{}", r % 3);
        match t {
            0 => {
                store.execute(set(&key, b"0", ex(SetExpiry::Px(1))));
            }
            1 => {
                let _ = store.sweep_expired_reporting_budget(64);
            }
            2 => {
                store.execute(Command::PExpire(key, 1));
            }
            _ => {
                store.execute(Command::Incr(key));
            }
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert_accounting_exact(&store, "INCR vs expiry");
    // And after the sweeper has removed whatever expired.
    while !store.sweep_expired_reporting_budget(1024).is_empty() {}
    assert_accounting_exact(&store, "INCR vs expiry, swept");
}

#[test]
fn hset_racing_hdel_accounts_exactly() {
    let store = KeyValueStore::new();
    race(8, 2_000, |t, r| {
        let key = format!("h{}", r % 3);
        let field = format!("f{}", r % 5);
        if t % 2 == 0 {
            store.execute(Command::HSet(key, vec![(field, vec![b'v'; r % 90])]));
        } else if t % 4 == 1 {
            // Can empty the hash, which then must leave the keyspace.
            store.execute(Command::HDel(key, vec![field]));
        } else {
            store.execute(Command::HIncrBy(key, "n".into(), 1));
        }
    });
    assert_accounting_exact(&store, "HSET vs HDEL");
}

#[test]
fn probed_writes_racing_multi_key_commands_account_exactly() {
    let store = KeyValueStore::new();
    race(8, 1_500, |t, r| {
        let a = format!("m{}", r % 4);
        let b = format!("m{}", (r + 1) % 4);
        match t % 4 {
            0 => {
                store.execute(Command::MSet(vec![
                    (a, vec![b'1'; r % 50]),
                    (b, vec![b'2'; r % 70]),
                ]));
            }
            1 => {
                store.execute(Command::Del(vec![a, b]));
            }
            2 => {
                store.execute(Command::Rename(a, b));
            }
            _ => {
                store.execute(set(&a, &vec![b'3'; r % 120], SetOptions::default()));
                store.execute(Command::Incr(b));
            }
        }
    });
    assert_accounting_exact(&store, "probed vs multi-key");
}

/// Capped stores keep the general path (eviction, rollback); the probed
/// path must not leak into it. Eviction racing writes still accounts exactly.
#[test]
fn capped_store_with_eviction_racing_writes_accounts_exactly() {
    let store = KeyValueStore::with_config(Some(64), Some(16 * 1024), EvictionPolicy::AllKeysLru);
    race(8, 1_000, |t, r| {
        let key = format!("e{}", (t * 1_000 + r) % 200);
        match r % 3 {
            0 => store.execute(set(&key, &vec![b'x'; r % 300], SetOptions::default())),
            1 => store.execute(Command::Incr(format!("n{}", r % 10))),
            _ => store.execute(Command::HSet(key, vec![("f".into(), vec![b'y'; r % 100])])),
        };
    });
    assert_accounting_exact(&store, "capped with eviction");
    assert!(store.key_count() <= 64);
}

/// `execute_reporting_ref_within_capacity` runs a write exactly when it can
/// neither evict nor be refused for capacity, and a declined write changes
/// nothing; an accepted one gives what the ordinary path gives.
#[test]
fn within_capacity_runs_only_writes_that_cannot_evict() {
    let snapshot = |s: &KeyValueStore| {
        let mut v: Vec<String> = s
            .snapshot()
            .into_iter()
            .map(|e| {
                format!(
                    "{} {}",
                    e.key,
                    serde_json::to_value(&e.value).unwrap_or_default()
                )
            })
            .collect();
        v.sort();
        (v, s.approximate_memory_bytes(), s.key_count())
    };
    // Key cap: a new key fits until the cap, an existing key always does.
    let keys = KeyValueStore::with_config(Some(3), None, EvictionPolicy::AllKeysLru);
    for k in ["a", "b", "c"] {
        let (reply, evicted) = keys
            .execute_reporting_ref_within_capacity(&set(k, b"v", SetOptions::default()))
            .expect("fits under the key cap");
        assert_eq!(reply, Value::SimpleString("OK".into()));
        assert!(evicted.is_empty());
    }
    let before = snapshot(&keys);
    assert!(
        keys.execute_reporting_ref_within_capacity(&set("d", b"v", SetOptions::default()))
            .is_none()
    );
    assert!(
        keys.execute_reporting_ref_within_capacity(&Command::Incr("e".into()))
            .is_none()
    );
    assert_eq!(snapshot(&keys), before, "a declined write changes nothing");
    assert!(
        keys.execute_reporting_ref_within_capacity(&set("a", b"w", SetOptions::default()))
            .is_some()
    );
    assert!(
        keys.execute_reporting_ref_within_capacity(&Command::FlushDb)
            .is_none()
    );
    // The ordinary path still evicts for the declined write.
    let (_, evicted) = keys.execute_reporting_ref(&set("d", b"v", SetOptions::default()));
    assert_eq!(evicted.len(), 1);
    assert_accounting_exact(&keys, "key cap");

    // Memory cap: a write fits while its worst case stays below the point
    // where eviction starts; a write with no bound never does.
    let mem = KeyValueStore::with_config(None, Some(4_096), EvictionPolicy::AllKeysLru);
    let mut accepted = 0;
    for i in 0..200 {
        let cmd = set(&format!("m{i}"), &[b'x'; 100], SetOptions::default());
        let before = snapshot(&mem);
        match mem.execute_reporting_ref_within_capacity(&cmd) {
            Some((_, evicted)) => {
                assert!(evicted.is_empty(), "step {i}: an accepted write evicted");
                accepted += 1;
            }
            None => {
                assert_eq!(snapshot(&mem), before, "step {i}: declined but changed");
                let target = 4_096 - 4_096 / EVICTION_HEADROOM_DIVISOR;
                assert!(
                    mem.approximate_memory_bytes() + 100 + 64 + 4 > target,
                    "step {i}"
                );
                break;
            }
        }
        assert_accounting_exact(&mem, "memory cap");
    }
    assert!(accepted > 10 && accepted < 200, "accepted {accepted}");
    // A removal cannot grow the store, so it fits even here.
    let (reply, evicted) = mem
        .execute_reporting_ref_within_capacity(&Command::Del(vec!["m0".into()]))
        .expect("a removal fits");
    assert_eq!((reply, evicted.len()), (Value::Integer(1), 0));
    assert_accounting_exact(&mem, "memory cap after DEL");
}

// ── 6O-C: the borrowed collection writes ──────────────────────────────────────

fn resp_frame(args: &[&[u8]]) -> Vec<u8> {
    let mut out = format!("*{}\r\n", args.len()).into_bytes();
    for a in args {
        out.extend_from_slice(format!("${}\r\n", a.len()).as_bytes());
        out.extend_from_slice(a);
        out.extend_from_slice(b"\r\n");
    }
    out
}

/// Run `frame` on `store` as the TCP loop does with no persistence: borrowed
/// when the decoder borrows it and the store takes it (`execute_ref_into`),
/// owned otherwise. The reply's bytes, and whether it was borrowed.
fn run_as_server(store: &KeyValueStore, frame: &[u8]) -> (Vec<u8>, bool) {
    let mut decoder = crate::request::RequestDecoder::new();
    let mut out = Vec::new();
    match decoder.decode_any(frame) {
        crate::request::DecodedAny::Borrowed(cmd, _) => {
            if store.execute_ref_into(&cmd, &mut out).is_some() {
                return (out, true);
            }
            let (reply, _) = store.execute_reporting_ref(&cmd.to_command());
            reply.serialize_into(&mut out);
            (out, false)
        }
        crate::request::DecodedAny::Owned(Ok((Ok(cmd), _))) => {
            let (reply, _) = store.execute_reporting_ref(&cmd);
            reply.serialize_into(&mut out);
            (out, false)
        }
        crate::request::DecodedAny::Owned(other) => panic!("not a command: {other:?}"),
    }
}

/// Run `frame` owned: decoded the same way, then always `to_command` and the
/// server's owned entry point — the path a borrowed write took before 6O-C.
fn run_owned(store: &KeyValueStore, frame: &[u8]) -> Vec<u8> {
    let mut decoder = crate::request::RequestDecoder::new();
    let cmd = match decoder.decode(frame) {
        Ok((Ok(cmd), _)) => cmd,
        other => panic!("not a command: {other:?}"),
    };
    let (reply, _) = store.execute_reporting_ref(&cmd);
    let mut out = Vec::new();
    reply.serialize_into(&mut out);
    out
}

fn collection_script() -> Vec<Vec<u8>> {
    let mut members: Vec<Vec<u8>> = (0..200).map(|i| format!("m{i}").into_bytes()).collect();
    members.insert(0, b"SADD".to_vec());
    members.insert(1, b"big".to_vec());
    let members: Vec<&[u8]> = members.iter().map(Vec::as_slice).collect();
    let mut f: Vec<Vec<u8>> = vec![
        resp_frame(&[b"LPUSH", b"l", b"a"]),
        resp_frame(&[b"LPUSH", b"l", b"b", b"c"]),
        resp_frame(&[b"RPUSH", b"l", b"", b"\xff\x00\r\n"]),
        resp_frame(&[b"RPUSH", b"l", b"1", b"2", b"3", b"4", b"5", b"6", b"7"]),
        resp_frame(&[b"LRANGE", b"l", b"0", b"-1"]),
        resp_frame(&[b"SET", b"str", b"v"]),
        resp_frame(&[b"LPUSH", b"str", b"x"]),
        resp_frame(&[b"SADD", b"str", b"x"]),
        resp_frame(&[b"SADD", b"l", b"x"]),
        resp_frame(&[b"SADD", b"set", b"a", b"b", b"a"]),
        resp_frame(&[b"RPUSH", b"set", b"x"]),
        resp_frame(&[b"SADD", b"set", b"b", b"c"]),
        resp_frame(&[b"SADD", b"set", b"c"]),
        resp_frame(&[b"SADD", b"set", b"caf\xC3\xA9", b""]),
        resp_frame(&[b"SMEMBERS", b"set"]),
        // The set's inline-to-table switch, then re-adds that copy nothing.
        resp_frame(&members),
        resp_frame(&members),
        resp_frame(&[b"SCARD", b"big"]),
        // An emptied list is gone; a push recreates it.
        resp_frame(&[b"LPOP", b"l", b"100"]),
        resp_frame(&[b"EXISTS", b"l"]),
        resp_frame(&[b"RPUSH", b"l", b"again"]),
        // TTLs: a push keeps the list's TTL; an expired key is recreated.
        resp_frame(&[b"EXPIRE", b"l", b"100"]),
        resp_frame(&[b"LPUSH", b"l", b"ttl"]),
        resp_frame(&[b"PERSIST", b"l"]),
        resp_frame(&[b"PEXPIRE", b"set", b"1"]),
    ];
    f.push(Vec::new()); // a pause for the expiry, see the test
    f.extend([
        resp_frame(&[b"SADD", b"set", b"new"]),
        resp_frame(&[b"SMEMBERS", b"set"]),
        resp_frame(&[b"EXISTS", b"set"]),
        resp_frame(&[b"DEL", b"l", b"set", b"big"]),
        resp_frame(&[b"LPUSH", b"l", b"after"]),
    ]);
    f
}

/// Every collection write the server now runs borrowed replies and accounts
/// exactly as the owned path did: on one persistent store per path, every
/// reply byte, the memory estimate, the key count and an independent
/// recomputation of the accounting agree at every step. Several values per
/// push (more than the inline capacity), duplicates within and across
/// SADDs, wrong types both ways, an emptied list, a TTL kept across a push
/// (`PERSIST` answers 1 only if it was) and a revived expired key are all
/// in the script.
#[test]
fn borrowed_collection_writes_reply_and_account_like_the_owned_path() {
    let borrowed = KeyValueStore::new();
    let owned = KeyValueStore::new();
    for (i, frame) in collection_script().iter().enumerate() {
        if frame.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }
        let what = format!("step {i} {:?}", String::from_utf8_lossy(frame));
        let (b, was_borrowed) = run_as_server(&borrowed, frame);
        let collection = [b"$5\r\nLPUSH".as_slice(), b"$5\r\nRPUSH", b"$4\r\nSADD"]
            .iter()
            .any(|name| frame[frame.iter().position(|&c| c == b'$').unwrap()..].starts_with(name));
        assert!(
            !collection || was_borrowed,
            "{what}: a collection write ran owned"
        );
        let o = run_owned(&owned, frame);
        assert_eq!(
            String::from_utf8_lossy(&b),
            String::from_utf8_lossy(&o),
            "{what}: reply"
        );
        assert_eq!(
            borrowed.approximate_memory_bytes(),
            owned.approximate_memory_bytes(),
            "{what}: memory"
        );
        assert_eq!(borrowed.key_count(), owned.key_count(), "{what}: key count");
        assert_accounting_exact(&borrowed, &format!("{what} borrowed"));
    }
}

/// Borrowed pushes and adds from eight threads on shared keys: no push is
/// lost (LLEN equals the pushes made), every distinct member is reported
/// added exactly once across all threads (the SADD replies sum to the
/// set's size), and the accounting is exact — with owned pops, deletes and
/// type changes racing on a third key.
#[test]
fn borrowed_pushes_and_adds_racing_lose_nothing_and_account_exactly() {
    let store = KeyValueStore::new();
    let pushes = AtomicUsize::new(0);
    let added = AtomicUsize::new(0);
    race(8, 2_000, |t, r| {
        let frame = match (t + r) % 6 {
            0 => {
                pushes.fetch_add(1, Ordering::Relaxed);
                resp_frame(&[b"LPUSH", b"q", &vec![b'v'; (r * 5) % 90]])
            }
            1 => {
                pushes.fetch_add(2, Ordering::Relaxed);
                resp_frame(&[b"RPUSH", b"q", b"x", format!("{t}:{r}").as_bytes()])
            }
            2 => resp_frame(&[
                b"SADD",
                b"pool",
                format!("m{}", (t * 7 + r) % 64).as_bytes(),
                format!("m{}", r % 64).as_bytes(),
            ]),
            3 => resp_frame(&[b"LPUSH", b"churn", b"c"]),
            4 => resp_frame(&[b"SADD", b"churn", b"c"]),
            _ => resp_frame(&[b"LPOP", b"churn"]),
        };
        let (reply, _) = run_as_server(&store, &frame);
        if frame.starts_with(b"*4\r\n$4\r\nSADD\r\n$4\r\npool") {
            let n: usize = std::str::from_utf8(&reply[1..reply.len() - 2])
                .unwrap()
                .parse()
                .unwrap();
            added.fetch_add(n, Ordering::Relaxed);
        }
        if (t + r) % 97 == 0 {
            store.execute(Command::Del(vec!["churn".into()]));
        }
    });
    assert_accounting_exact(&store, "borrowed writers");
    assert_eq!(
        store.execute(Command::LLen("q".into())),
        Value::Integer(pushes.load(Ordering::Relaxed) as i64),
        "every borrowed push landed once"
    );
    assert_eq!(
        store.execute(Command::SCard("pool".into())),
        Value::Integer(64)
    );
    assert_eq!(
        added.load(Ordering::Relaxed),
        64,
        "each member reported added once"
    );
}

/// A capped store declines every borrowed write, having changed nothing:
/// capacity reservations belong to the owned path, which then runs it.
#[test]
fn a_capped_store_declines_borrowed_collection_writes() {
    let store = KeyValueStore::with_config(Some(1_000), None, EvictionPolicy::NoEviction);
    for parts in [
        vec![b"LPUSH".as_slice(), b"l1", b"v"],
        vec![b"RPUSH", b"l2", b"v"],
        vec![b"SADD", b"s", b"m"],
    ] {
        let frame = resp_frame(&parts);
        let mut decoder = crate::request::RequestDecoder::new();
        let crate::request::DecodedAny::Borrowed(cmd, _) = decoder.decode_any(&frame) else {
            panic!("{parts:?} not borrowed");
        };
        let (keys, memory) = (store.key_count(), store.approximate_memory_bytes());
        let mut out = Vec::new();
        assert!(store.execute_ref_into(&cmd, &mut out).is_none());
        assert!(out.is_empty(), "nothing written");
        assert_eq!(store.key_count(), keys, "nothing run");
        assert_eq!(store.approximate_memory_bytes(), memory);
        let (reply, was_borrowed) = run_as_server(&store, &frame);
        assert!(!was_borrowed);
        assert_eq!(reply, b":1\r\n");
    }
    assert_accounting_exact(&store, "capped");
}
