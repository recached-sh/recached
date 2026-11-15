//! `execute_ref_into` (borrowed command, reply written in place) against
//! `execute_reporting` of the owned command, serialized.
//!
//! The server answers hot commands through the borrowed path when it can and
//! the owned one otherwise, so the two must be indistinguishable on the wire
//! and in the store: same reply bytes, same keyspace, same accounting.

use super::*;
use crate::request::CommandRef;
use smallvec::smallvec;

fn script() -> Vec<CommandRef<'static>> {
    let big: &'static [u8] = Box::leak(vec![b'B'; 70_000].into_boxed_slice());
    vec![
        CommandRef::Get("missing"),
        CommandRef::Set("s", b"value"),
        CommandRef::Get("s"),
        CommandRef::Set("empty", b""),
        CommandRef::Get("empty"),
        CommandRef::Set("bin", b"\x00\xff\r\n$-1\r\n"),
        CommandRef::Get("bin"),
        CommandRef::Set("ключ🔑", "значение".as_bytes()),
        CommandRef::Get("ключ🔑"),
        CommandRef::Set("big", big),
        CommandRef::Get("big"),
        CommandRef::Incr("n"),
        CommandRef::Incr("n"),
        CommandRef::Decr("n"),
        CommandRef::Decr("fresh"),
        CommandRef::Incr("s"),
        CommandRef::Set("max", b"9223372036854775807"),
        CommandRef::Incr("max"),
        CommandRef::Set("min", b"-9223372036854775808"),
        CommandRef::Decr("min"),
        CommandRef::HSet("h", smallvec![("f", b"1".as_slice())]),
        CommandRef::HSet(
            "h",
            smallvec![
                ("f", b"2".as_slice()),
                ("g", b"".as_slice()),
                ("f", b"3".as_slice())
            ],
        ),
        CommandRef::HGet("h", "f"),
        CommandRef::HGet("h", "nope"),
        CommandRef::HGet("nohash", "f"),
        CommandRef::HGet("s", "f"),
        CommandRef::HSet("s", smallvec![("f", b"x".as_slice())]),
        CommandRef::Get("h"),
        CommandRef::Incr("h"),
        CommandRef::MGet(smallvec!["s", "h", "missing", "n", "bin", "empty"]),
        CommandRef::MGet(smallvec!["only"]),
        CommandRef::Set("h", b"string now"),
        CommandRef::HGet("h", "f"),
        CommandRef::Get("expiring"),
    ]
}

fn reply_owned(store: &KeyValueStore, cmd: &CommandRef<'_>) -> Vec<u8> {
    store.execute_reporting(cmd.to_command()).0.serialize()
}

fn compare(make: impl Fn() -> KeyValueStore, what: &str) {
    let owned = make();
    let borrowed = make();
    for store in [&owned, &borrowed] {
        // An expired, unswept key: both paths must read it as missing.
        store.execute(Command::Set(
            "expiring".into(),
            b"gone".to_vec(),
            SetOptions {
                expiry: Some(SetExpiry::Px(1)),
                ..SetOptions::default()
            },
        ));
    }
    std::thread::sleep(std::time::Duration::from_millis(5));
    for (i, cmd) in script().iter().enumerate() {
        let expected = reply_owned(&owned, cmd);
        let mut out = b"prefix".to_vec();
        match borrowed.execute_ref_into(cmd, &mut out) {
            Some(outcome) => {
                assert_eq!(
                    String::from_utf8_lossy(&out[6..]),
                    String::from_utf8_lossy(&expected),
                    "{what}: step {i} {cmd:?}"
                );
                assert_eq!(&out[..6], b"prefix", "{what}: appends, never clears");
                assert_eq!(
                    outcome.error,
                    expected.first() == Some(&b'-'),
                    "{what}: step {i}"
                );
                if let CommandRef::Get(_) = cmd {
                    // As the server counts them: an error is neither a hit
                    // nor a miss.
                    let hit = (!outcome.error).then(|| !expected.starts_with(b"$-1"));
                    assert_eq!(outcome.hit, hit, "{what}: hit {i}");
                }
            }
            None => {
                // Declined: nothing written, nothing run. The caller then
                // runs the owned command, as here.
                assert!(
                    cmd.is_write() && borrowed.has_capacity_limits(),
                    "{what}: step {i}"
                );
                assert_eq!(out, b"prefix", "{what}: a declined command writes nothing");
                assert_eq!(reply_owned(&borrowed, cmd), expected, "{what}: step {i}");
            }
        }
        assert_eq!(
            borrowed.approximate_memory_bytes(),
            owned.approximate_memory_bytes(),
            "{what}: memory after step {i} {cmd:?}"
        );
        assert_eq!(
            borrowed.key_count(),
            owned.key_count(),
            "{what}: keys after step {i}"
        );
    }
    let dump = |s: &KeyValueStore| {
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
        v
    };
    assert_eq!(dump(&borrowed), dump(&owned), "{what}: final keyspace");
}

#[test]
fn borrowed_execution_matches_owned_execution_byte_for_byte() {
    compare(KeyValueStore::new, "uncapped");
}

#[test]
fn borrowed_writes_decline_on_a_capped_store_and_reads_still_match() {
    compare(
        || KeyValueStore::with_config(Some(1_000), None, EvictionPolicy::NoEviction),
        "capped",
    );
}

/// Every borrowed command converts to the owned command its owned decoder
/// builds, and agrees with it on whether it writes and on its key.
#[test]
fn borrowed_commands_agree_with_their_owned_form() {
    for cmd in script() {
        let owned = cmd.to_command();
        let writes = !matches!(mutation_scope(&owned), MutationScope::ReadOnly);
        assert_eq!(cmd.is_write(), writes, "{cmd:?}");
        if writes {
            let MutationScope::Keys(keys) = mutation_scope(&owned) else {
                panic!("{cmd:?}: a borrowed write names its key");
            };
            assert_eq!(keys.as_slice(), &[cmd.write_key().unwrap()], "{cmd:?}");
            assert!(
                probe_eligible(&owned),
                "{cmd:?}: borrowed writes are probed"
            );
        }
    }
}
