//! v0.5 synchronization-reliability tests: outbox capacity, reply
//! correlation, wire-id limits, reconnect reconciliation and the lifecycle
//! status. Each drives `SyncClient` with hand-written frames, so every
//! sequence is exact and repeatable.
//!
//! The numbered scenarios (`test_01_…`) are the regression list of the
//! milestone; the rest cover the edges around them.

use super::*;

fn client() -> SyncClient {
    SyncClient::new(Arc::new(KeyValueStore::new()), "client-a".to_string())
}

fn get(c: &SyncClient, key: &str) -> Value {
    c.store().execute(Command::Get(key.to_string()))
}

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

fn set(key: &str, value: &str) -> Vec<u8> {
    to_resp(&["SET", key, value])
}

fn text(frame: &[u8]) -> String {
    String::from_utf8_lossy(frame).into_owned()
}

const OPT_INS: usize = 4;

/// Answer the four `CLIENT` opt-ins as a 0.5 server does: all accepted.
fn ack_opt_ins_v5(c: &mut SyncClient) {
    for _ in 0..OPT_INS {
        assert_eq!(
            c.handle_frame(b"+OK\r\n"),
            Incoming::Reply { retired: None }
        );
    }
}

/// Answer them as a 0.4 server does: the two 0.5 opt-ins refused.
fn ack_opt_ins_v4(c: &mut SyncClient) {
    for reply in [
        &b"+OK\r\n"[..],
        b"+OK\r\n",
        b"-ERR unknown subcommand 'PUSHFRAMES'\r\n",
        b"-ERR unknown subcommand 'DEDUPBARRIER'\r\n",
    ] {
        assert_eq!(c.handle_frame(reply), Incoming::Reply { retired: None });
    }
}

fn qstate(pattern: &str, pairs: &[(&str, &str)]) -> Vec<u8> {
    let mut items = vec![
        Value::BulkString(Some(b"qstate".to_vec())),
        Value::BulkString(Some(pattern.as_bytes().to_vec())),
    ];
    for (k, v) in pairs {
        items.push(bulk(k));
        items.push(bulk(v));
    }
    Value::Array(Some(items)).serialize()
}

fn keychange_push(key: &str, value: Option<&str>) -> Vec<u8> {
    let v = match value {
        Some(v) => bulk(v),
        None => Value::BulkString(None),
    };
    let mut out = b">3\r\n".to_vec();
    out.extend_from_slice(&bulk("keychange").serialize());
    out.extend_from_slice(&bulk(key).serialize());
    out.extend_from_slice(&v.serialize());
    out
}

fn phase(c: &SyncClient) -> SyncPhase {
    c.status().phase
}

// ── Workstream A: outbox capacity ────────────────────────────────────────────

#[test]
fn test_01_a_full_outbox_refuses_the_new_write_and_keeps_every_queued_one() {
    let mut c = client();
    c.set_max_pending(3);
    let queued: Vec<Enqueued> = (0..3)
        .map(|i| {
            c.try_enqueue_write(&set(&format!("k{i}"), "v"), true, false)
                .unwrap()
        })
        .collect();

    assert_eq!(
        c.try_enqueue_write(&set("k3", "v"), true, false),
        Err(EnqueueError::OutboxFull {
            pending: 3,
            capacity: 3
        })
    );
    assert_eq!(c.outbox_len(), 3);
    assert_eq!(c.counters().rejected_writes, 1);

    // Every queued write replays, byte for byte, in order.
    let frames = c.on_open();
    let replayed: Vec<&Vec<u8>> = frames[OPT_INS..].iter().collect();
    assert_eq!(
        replayed,
        queued.iter().map(|e| &e.frame).collect::<Vec<_>>()
    );
}

#[test]
fn a_rejected_write_consumes_no_id_and_leaves_room_once_drained() {
    let mut c = client();
    c.set_max_pending(1);
    let first = c.try_enqueue_write(&set("a", "1"), true, false).unwrap();
    assert!(c.try_enqueue_write(&set("b", "2"), true, false).is_err());
    c.on_open();
    ack_opt_ins_v5(&mut c);
    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply {
            retired: Some(first.id)
        }
    );
    // The refused write took no id, so the next accepted one follows on.
    let next = c.try_enqueue_write(&set("b", "2"), true, true).unwrap();
    assert_eq!(next.id, first.id + 1);
    assert!(next.send_now);
}

#[test]
fn capacity_of_one_exactly_at_and_one_past_the_limit() {
    for capacity in [1usize, 2, 7] {
        let mut c = client();
        c.set_max_pending(capacity);
        for i in 0..capacity {
            assert!(c.check_capacity(true).is_ok());
            c.try_enqueue_write(&set(&format!("k{i}"), "v"), true, false)
                .unwrap();
        }
        // Exactly at the limit: the next one is refused, before and at enqueue.
        assert_eq!(
            c.check_capacity(true),
            Err(EnqueueError::OutboxFull {
                pending: capacity,
                capacity
            })
        );
        assert!(c.try_enqueue_write(&set("over", "v"), true, false).is_err());
        assert_eq!(c.outbox_len(), capacity);
    }
}

#[test]
fn overflow_while_connected_with_writes_in_flight_keeps_reply_matching_intact() {
    let mut c = client();
    c.set_max_pending(2);
    c.on_open();
    ack_opt_ins_v5(&mut c);
    let w1 = c.try_enqueue_write(&set("a", "1"), true, true).unwrap();
    let w2 = c.try_enqueue_write(&set("a", "2"), true, true).unwrap();
    // Both in flight; a third is refused without touching their slots.
    assert!(matches!(
        c.try_enqueue_write(&set("a", "3"), true, true),
        Err(EnqueueError::OutboxFull { .. })
    ));
    for w in [w1, w2] {
        assert_eq!(
            c.handle_frame(b"+OK\r\n"),
            Incoming::Reply {
                retired: Some(w.id)
            }
        );
    }
    assert_eq!(c.outbox_len(), 0);
}

#[test]
fn several_writes_to_one_key_queue_and_replay_in_order() {
    let mut c = client();
    let frames: Vec<Vec<u8>> = ["1", "2", "3"]
        .iter()
        .map(|v| {
            c.try_enqueue_write(&set("k", v), true, false)
                .unwrap()
                .frame
        })
        .collect();
    c.try_enqueue_write(&to_resp(&["INCRBY", "n", "5"]), true, false)
        .unwrap();
    c.try_enqueue_write(&to_resp(&["DEL", "k"]), true, false)
        .unwrap();
    let replay = c.on_open();
    assert_eq!(replay[OPT_INS..OPT_INS + 3], frames[..]);
    assert!(text(&replay[OPT_INS + 3]).contains("INCRBY"));
    assert!(text(&replay[OPT_INS + 4]).contains("DEL"));
}

#[test]
fn the_legacy_policy_must_be_chosen_explicitly() {
    let mut c = client();
    assert_eq!(c.overflow_policy(), OverflowPolicy::Reject);
    c.set_max_pending(1);
    c.set_overflow_policy(OverflowPolicy::EvictOldest);
    let a = c.try_enqueue_write(&set("a", "1"), true, false).unwrap();
    let b = c.try_enqueue_write(&set("b", "1"), true, false).unwrap();
    assert_eq!(b.dropped, Some(a.id));
    assert_eq!(c.counters().evicted_writes, 1);
}

#[test]
fn a_restart_with_a_full_outbox_keeps_every_restored_row_and_refuses_new_ones() {
    // The previous session left more rows than this one's cap — the cap was
    // lowered, say. They are writes the application was told were accepted.
    let mut c = client();
    c.set_max_pending(2);
    let rows: Vec<(u64, Vec<u8>)> = (0..5).map(|i| (i, set(&format!("k{i}"), "v"))).collect();
    let restored = c.restore_outbox(rows, false);
    assert_eq!(restored.rewrites.len(), 5);
    assert_eq!(c.outbox_len(), 5);
    assert!(matches!(
        c.try_enqueue_write(&set("new", "v"), true, false),
        Err(EnqueueError::OutboxFull {
            pending: 5,
            capacity: 2
        })
    ));
}

#[test]
fn a_write_whose_commit_failed_can_be_withdrawn_before_it_is_sent() {
    let mut c = client();
    c.on_open();
    ack_opt_ins_v5(&mut c);
    // Queued as not-yet-sendable while the adapter commits it.
    let e = c.try_enqueue_write(&set("a", "1"), true, false).unwrap();
    assert!(!e.send_now);
    assert!(c.cancel_enqueued(e.id));
    assert_eq!(c.outbox_len(), 0);
    // Nothing in flight, so the next reply retires nothing.
    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply { retired: None }
    );
}

#[test]
fn a_committed_write_is_sent_once_and_matched_to_its_reply() {
    let mut c = client();
    c.on_open();
    ack_opt_ins_v5(&mut c);
    let e = c.try_enqueue_write(&set("a", "1"), true, false).unwrap();
    assert_eq!(c.send_enqueued(e.id, true), Some(e.frame.clone()));
    // A second call does not send it twice.
    assert_eq!(c.send_enqueued(e.id, true), None);
    // And a sent write cannot be withdrawn.
    assert!(!c.cancel_enqueued(e.id));
    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply {
            retired: Some(e.id)
        }
    );
}

#[test]
fn only_the_newest_unsent_row_can_be_withdrawn() {
    let mut c = client();
    let a = c.try_enqueue_write(&set("a", "1"), true, false).unwrap();
    let _b = c.try_enqueue_write(&set("b", "1"), true, false).unwrap();
    assert!(!c.cancel_enqueued(a.id));
    assert_eq!(c.outbox_len(), 2);
}

// ── Workstream B: wire ids ───────────────────────────────────────────────────

#[test]
fn test_15_the_last_wire_id_of_an_epoch_is_used_and_the_next_is_refused() {
    let mut c = client();
    c.set_epoch(9);
    c.outbox_seq = MAX_WIRE_SEQ;
    let last = c.try_enqueue_write(&set("a", "1"), true, false).unwrap();
    let wire = ((9u64 << 32) | MAX_WIRE_SEQ).to_string();
    assert!(text(&last.frame).contains(&wire), "{}", text(&last.frame));
    // One more would wrap the low 32 bits onto epoch 9's first id.
    assert_eq!(
        c.try_enqueue_write(&set("b", "1"), true, false),
        Err(EnqueueError::WireIdsExhausted)
    );
    // An unwrapped write carries no wire id, so it is not limited.
    assert!(
        c.try_enqueue_write(&to_resp(&["SUBSCRIBE", "x"]), false, false)
            .is_ok()
    );
}

#[test]
fn test_15_the_epoch_never_saturates_into_reuse() {
    assert_eq!(next_epoch(0), Some(1));
    assert_eq!(next_epoch(u32::MAX - 1), Some(u32::MAX));
    // Saturating here would hand the next session the last one's ids.
    assert_eq!(next_epoch(u32::MAX), None);
}

// ── Workstream B/C: reply correlation ────────────────────────────────────────

#[test]
fn test_08_a_malformed_frame_with_writes_in_flight_retires_none_of_them() {
    let mut c = client();
    c.on_open();
    ack_opt_ins_v5(&mut c);
    let writes: Vec<Enqueued> = (0..3)
        .map(|i| {
            c.try_enqueue_write(&set(&format!("k{i}"), "v"), true, true)
                .unwrap()
        })
        .collect();
    assert_eq!(c.handle_frame(b"$9\r\ntruncated"), Incoming::Malformed);
    assert_eq!(phase(&c), SyncPhase::Degraded);
    assert_eq!(c.status().error, Some(SyncError::MalformedFrame));
    // Nothing retired; every row replays on the next socket.
    assert_eq!(c.outbox_len(), 3);
    c.on_close();
    let replay = c.on_open();
    assert_eq!(
        replay[OPT_INS..].to_vec(),
        writes.into_iter().map(|w| w.frame).collect::<Vec<_>>()
    );
}

#[test]
fn a_reply_shaped_like_a_snapshot_answers_its_write_and_is_not_applied() {
    // An `LPOP list 2` reply whose first element is the text "qstate" used to
    // be taken for a snapshot of pattern "*": every local key deleted, and the
    // write's own reply slot left for the next reply to take.
    let mut c = client();
    c.store().execute(Command::Set(
        "keep".into(),
        b"me".to_vec(),
        Default::default(),
    ));
    c.on_open();
    ack_opt_ins_v4(&mut c);
    let w = c
        .try_enqueue_write(&to_resp(&["LPOP", "list", "2"]), true, true)
        .unwrap();
    let reply = Value::Array(Some(vec![bulk("qstate"), bulk("*")])).serialize();
    assert_eq!(
        c.handle_frame(&reply),
        Incoming::Reply {
            retired: Some(w.id)
        }
    );
    assert_eq!(get(&c, "keep"), bulk("me"));
}

#[test]
fn with_push_framing_a_keychange_shaped_reply_is_a_reply() {
    let mut c = client();
    c.on_open();
    ack_opt_ins_v5(&mut c);
    let w = c
        .try_enqueue_write(&to_resp(&["RPOP", "events", "3"]), true, true)
        .unwrap();
    // The popped elements happen to read like a keychange.
    let reply = Value::Array(Some(vec![bulk("keychange"), bulk("x"), bulk("y")])).serialize();
    assert_eq!(
        c.handle_frame(&reply),
        Incoming::Reply {
            retired: Some(w.id)
        }
    );
    assert_eq!(get(&c, "x"), Value::BulkString(None));
}

#[test]
fn with_push_framing_keychange_pushes_never_take_a_reply_slot() {
    let mut c = client();
    c.on_open();
    ack_opt_ins_v5(&mut c);
    let w = c.try_enqueue_write(&set("a", "1"), true, true).unwrap();
    assert_eq!(
        c.handle_frame(&keychange_push("x", Some("z"))),
        Incoming::Applied {
            keys: vec!["x".into()]
        }
    );
    assert_eq!(get(&c, "x"), bulk("z"));
    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply {
            retired: Some(w.id)
        }
    );
}

#[test]
fn push_framing_is_forgotten_with_the_socket() {
    // A reconnect to an older server must not keep reading plain arrays as
    // replies only.
    let mut c = client();
    c.on_open();
    ack_opt_ins_v5(&mut c);
    c.on_close();
    c.on_open();
    ack_opt_ins_v4(&mut c);
    assert_eq!(
        c.handle_frame(b"*3\r\n$9\r\nkeychange\r\n$1\r\nx\r\n$1\r\nz\r\n"),
        Incoming::Applied {
            keys: vec!["x".into()]
        }
    );
}

// ── Workstream D: reconciliation ─────────────────────────────────────────────

#[test]
fn test_05_a_snapshot_never_rolls_back_a_write_still_queued() {
    // Offline edits; on reconnect the server defers the first replayed write
    // (memory pressure, say), so its snapshot does not contain them.
    let mut c = client();
    c.add_live_query("todo:*", false);
    c.store().execute(Command::Set(
        "todo:1".into(),
        b"old".to_vec(),
        Default::default(),
    ));
    c.try_enqueue_write(&set("todo:1", "edited offline"), true, false)
        .unwrap();
    c.store().execute(Command::Set(
        "todo:1".into(),
        b"edited offline".to_vec(),
        Default::default(),
    ));
    c.try_enqueue_write(&set("todo:2", "created offline"), true, false)
        .unwrap();
    c.store().execute(Command::Set(
        "todo:2".into(),
        b"created offline".to_vec(),
        Default::default(),
    ));
    c.on_open();
    ack_opt_ins_v5(&mut c);
    assert!(matches!(
        c.handle_frame(b"-OOM command not allowed\r\n"),
        Incoming::Deferred { .. }
    ));
    // The barrier defers the second as well.
    assert!(matches!(
        c.handle_frame(b"-TRYAGAIN an earlier write from this client was deferred\r\n"),
        Incoming::Deferred { .. }
    ));
    // The snapshot predates both writes.
    let incoming = c.handle_frame(&qstate("todo:*", &[("todo:1", "old")]));
    assert_eq!(
        incoming,
        Incoming::AppliedReply {
            retired: None,
            keys: vec![]
        },
        "neither queued key may be reported changed"
    );
    assert_eq!(get(&c, "todo:1"), bulk("edited offline"));
    assert_eq!(get(&c, "todo:2"), bulk("created offline"));
    assert_eq!(c.outbox_len(), 2);
}

#[test]
fn a_protected_key_whose_write_turns_out_a_duplicate_is_re_snapshotted() {
    // The write was applied in an earlier session whose reply was lost. Its
    // replay is restored after the socket opened, so it goes out behind the
    // QSUB: the snapshot protects its key, then the reply is `+DUP`, which
    // brings no keychange. Only a fresh snapshot can correct the key.
    let mut c = client();
    c.add_live_query("doc:*", false);
    c.store().execute(Command::Set(
        "doc:1".into(),
        b"mine".to_vec(),
        Default::default(),
    ));
    c.on_open();
    ack_opt_ins_v5(&mut c);
    let old = c.wrap_dedup(&set("doc:1", "mine"), 3);
    let restored = c.restore_outbox(vec![(3, old)], true);
    assert_eq!(restored.send_now.len(), 1);
    // QSUB answered first: doc:1 is protected.
    c.handle_frame(&qstate("doc:*", &[("doc:1", "theirs")]));
    assert_eq!(get(&c, "doc:1"), bulk("mine"));
    assert!(c.take_frames_to_send(true).is_empty());
    // The replay is a duplicate: re-subscribe so the server's value arrives.
    c.handle_frame(b"+DUP\r\n");
    let frames = c.take_frames_to_send(true);
    assert_eq!(frames.len(), 1);
    assert!(text(&frames[0]).contains("QSUB"));
    c.handle_frame(&qstate("doc:*", &[("doc:1", "theirs")]));
    assert_eq!(get(&c, "doc:1"), bulk("theirs"));
}

#[test]
fn an_applied_protected_write_needs_no_resubscribe() {
    let mut c = client();
    c.add_live_query("doc:*", false);
    c.on_open();
    ack_opt_ins_v5(&mut c);
    c.handle_frame(&qstate("doc:*", &[]));
    c.try_enqueue_write(&set("doc:1", "v"), true, true).unwrap();
    c.add_live_query("doc:*", true);
    c.handle_frame(b"+OK\r\n");
    c.handle_frame(&qstate("doc:*", &[]));
    // Its own keychange follows the reply; no extra snapshot is needed.
    assert!(c.take_frames_to_send(true).is_empty());
}

#[test]
fn test_05_a_snapshot_after_a_successful_replay_applies_in_full() {
    let mut c = client();
    c.add_live_query("todo:*", false);
    c.try_enqueue_write(&set("todo:1", "mine"), true, false)
        .unwrap();
    c.store().execute(Command::Set(
        "todo:1".into(),
        b"mine".to_vec(),
        Default::default(),
    ));
    c.on_open();
    ack_opt_ins_v5(&mut c);
    assert!(matches!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply { retired: Some(_) }
    ));
    // Another client changed it after ours, before the snapshot.
    let incoming = c.handle_frame(&qstate("todo:*", &[("todo:1", "theirs")]));
    assert_eq!(
        incoming,
        Incoming::AppliedReply {
            retired: None,
            keys: vec!["todo:1".into()]
        }
    );
    assert_eq!(get(&c, "todo:1"), bulk("theirs"));
}

#[test]
fn test_06_a_key_deleted_upstream_while_offline_is_removed_by_hydration() {
    let mut c = client();
    c.add_live_query("cart:*", false);
    c.on_open();
    ack_opt_ins_v5(&mut c);
    c.handle_frame(&qstate("cart:*", &[("cart:1", "a"), ("cart:2", "b")]));
    c.on_close();
    // Offline: cart:2 deleted on the server. Reconnect.
    c.on_open();
    ack_opt_ins_v5(&mut c);
    let incoming = c.handle_frame(&qstate("cart:*", &[("cart:1", "a")]));
    match incoming {
        Incoming::AppliedReply { keys, .. } => assert!(keys.contains(&"cart:2".to_string())),
        other => panic!("{other:?}"),
    }
    assert_eq!(get(&c, "cart:2"), Value::BulkString(None));
}

#[test]
fn test_07_a_permanent_refusal_retires_its_row_and_names_what_stays_unreconciled() {
    let mut c = client();
    c.add_live_query("cart:*", false);
    c.on_open();
    ack_opt_ins_v5(&mut c);
    c.handle_frame(&qstate("cart:*", &[]));
    let keep = c
        .try_enqueue_write(&set("cart:1", "ok"), true, true)
        .unwrap();
    let refused = c
        .try_enqueue_write(
            &to_resp(&["MSET", "cart:2", "x", "admin:1", "y"]),
            true,
            true,
        )
        .unwrap();
    let after = c
        .try_enqueue_write(&set("cart:3", "ok"), true, true)
        .unwrap();

    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply {
            retired: Some(keep.id)
        }
    );
    assert_eq!(
        c.handle_frame(b"-NOSCOPE key 'admin:1' is outside this connection's sync scopes\r\n"),
        Incoming::Refused {
            retired: refused.id,
            keys: vec!["cart:2".into(), "admin:1".into()],
            unreconciled: vec!["admin:1".into()],
            reason: "NOSCOPE key 'admin:1' is outside this connection's sync scopes".into(),
        }
    );
    assert_eq!(c.counters().refused_writes, 1);
    // The next reply still belongs to the next write.
    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply {
            retired: Some(after.id)
        }
    );
    // The covering live query re-hydrates; the uncovered key cannot.
    let frames = c.resubscribe_covering(&["cart:2".into(), "admin:1".into()], true);
    assert_eq!(frames.len(), 1);
    assert_eq!(
        c.subscription_state("cart:*"),
        Some(SubscriptionState::Hydrating)
    );
}

#[test]
fn test_14_an_oversized_snapshot_is_refused_and_reconciles_nothing() {
    let mut c = client();
    c.add_live_query("big:*", false);
    c.store().execute(Command::Set(
        "big:1".into(),
        b"local".to_vec(),
        Default::default(),
    ));
    c.on_open();
    ack_opt_ins_v5(&mut c);
    assert_eq!(
        c.handle_frame(b"-ERR live query initial state exceeds 10000 keys; narrow the pattern\r\n"),
        Incoming::Reply { retired: None }
    );
    assert_eq!(get(&c, "big:1"), bulk("local"));
    assert_eq!(
        c.subscription_state("big:*"),
        Some(SubscriptionState::Failed {
            failure: SubscriptionFailure::SnapshotTooLarge,
            reason: "ERR live query initial state exceeds 10000 keys; narrow the pattern".into()
        })
    );
    let status = c.status();
    assert_eq!(status.phase, SyncPhase::Degraded);
    assert!(matches!(
        status.error,
        Some(SyncError::SubscriptionFailed {
            failure: SubscriptionFailure::SnapshotTooLarge,
            ..
        })
    ));
}

#[test]
fn test_13_a_refused_token_on_reconnect_hydrates_nothing_and_keeps_writes_queued() {
    let mut c = client();
    c.set_sync_token("expired-token", false);
    c.add_live_query("cart:*", false);
    c.store().execute(Command::Set(
        "cart:1".into(),
        b"local".to_vec(),
        Default::default(),
    ));
    let w = c
        .try_enqueue_write(&set("cart:1", "offline"), true, false)
        .unwrap();
    c.on_open();
    ack_opt_ins_v5(&mut c);
    assert_eq!(phase(&c), SyncPhase::Authenticating);
    // SYNC TOKEN refused; the write then has no token, which can clear.
    c.handle_frame(b"-NOSCOPE invalid sync token: token expired\r\n");
    assert_eq!(phase(&c), SyncPhase::Degraded);
    assert_eq!(
        c.status().error,
        Some(SyncError::ScopeRefused {
            reason: "NOSCOPE invalid sync token: token expired".into()
        })
    );
    assert!(matches!(
        c.handle_frame(b"-NOSCOPE send SYNC TOKEN <token> before issuing commands\r\n"),
        Incoming::Deferred { .. }
    ));
    // The QSUB is refused too: nothing out of scope is hydrated, nothing local
    // is reconciled away.
    c.handle_frame(b"-NOSCOPE pattern is outside this connection's sync scopes\r\n");
    assert_eq!(get(&c, "cart:1"), bulk("local"));
    assert_eq!(c.outbox_len(), 1);
    assert!(matches!(
        c.subscription_state("cart:*"),
        Some(SubscriptionState::Failed {
            failure: SubscriptionFailure::OutOfScope,
            ..
        })
    ));

    // With a fresh token the reconnect replays the same write.
    c.on_close();
    c.set_sync_token("fresh-token", false);
    let replay = c.on_open();
    assert!(replay.contains(&w.frame));
    assert!(text(&replay[OPT_INS]).contains("fresh-token"));
}

#[test]
fn restored_rows_are_sent_at_once_when_the_socket_is_already_open() {
    // The browser opens its socket before IndexedDB has finished loading. A
    // restored row held for the next reconnect would replay *after* this
    // session's writes, whose higher wire ids the server already accepted, and
    // be skipped as a duplicate.
    let mut c = client();
    c.on_open();
    ack_opt_ins_v5(&mut c);
    let old =
        "*6\r\n$5\r\nDEDUP\r\n$8\r\nclient-a\r\n$1\r\n7\r\n$3\r\nSET\r\n$1\r\na\r\n$1\r\n1\r\n";
    let restored = c.restore_outbox(vec![(7, old.as_bytes().to_vec())], true);
    assert_eq!(restored.send_now, vec![old.as_bytes().to_vec()]);
    assert_eq!(phase(&c), SyncPhase::Replaying);
    let new_id = restored.rewrites[0].1;
    // Sent before anything this session queues, and matched to its reply.
    let later = c.try_enqueue_write(&set("b", "1"), true, true).unwrap();
    assert_eq!(
        c.handle_frame(b"+DUP\r\n"),
        Incoming::Reply {
            retired: Some(new_id)
        }
    );
    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply {
            retired: Some(later.id)
        }
    );
    assert_eq!(phase(&c), SyncPhase::Ready);
}

// ── Workstream C: lifecycle ──────────────────────────────────────────────────

#[test]
fn the_full_lifecycle_moves_on_protocol_events_only() {
    let mut c = client();
    c.set_password("pw", false);
    c.add_live_query("a:*", false);
    c.try_enqueue_write(&set("a:1", "x"), true, false).unwrap();

    assert_eq!(phase(&c), SyncPhase::Disconnected);
    c.on_connecting();
    assert_eq!(phase(&c), SyncPhase::Connecting);
    c.on_open();
    assert_eq!(
        phase(&c),
        SyncPhase::Authenticating,
        "an open socket is not ready"
    );
    c.handle_frame(b"+OK\r\n"); // AUTH
    assert_eq!(phase(&c), SyncPhase::Authenticating, "opt-ins outstanding");
    ack_opt_ins_v5(&mut c);
    assert_eq!(phase(&c), SyncPhase::Replaying);
    c.handle_frame(b"+OK\r\n"); // the write
    assert_eq!(phase(&c), SyncPhase::Hydrating);
    assert_eq!(
        c.subscription_state("a:*"),
        Some(SubscriptionState::Hydrating)
    );
    c.handle_frame(&qstate("a:*", &[("a:1", "x")]));
    assert_eq!(phase(&c), SyncPhase::Ready);
    assert_eq!(
        c.subscription_state("a:*"),
        Some(SubscriptionState::Current)
    );
    c.on_close();
    assert_eq!(phase(&c), SyncPhase::Disconnected);
    assert_eq!(
        c.subscription_state("a:*"),
        Some(SubscriptionState::Pending)
    );
}

#[test]
fn states_with_nothing_to_do_are_skipped_but_ready_still_waits_for_the_server() {
    let mut c = client();
    c.on_open();
    // No password, writes or live queries — but the opt-ins are unanswered.
    assert_eq!(phase(&c), SyncPhase::Authenticating);
    ack_opt_ins_v4(&mut c);
    assert_eq!(phase(&c), SyncPhase::Ready);
}

#[test]
fn ready_degrades_on_a_refused_live_query_and_recovers_when_it_is_dropped() {
    let mut c = client();
    c.on_open();
    ack_opt_ins_v5(&mut c);
    assert_eq!(phase(&c), SyncPhase::Ready);
    c.add_live_query("huge:*", true);
    c.handle_frame(b"-ERR live query initial state exceeds 10 keys; narrow the pattern\r\n");
    assert_eq!(phase(&c), SyncPhase::Degraded);
    c.remove_live_query(Some("huge:*"), true);
    c.handle_frame(b"+OK\r\n");
    assert_eq!(phase(&c), SyncPhase::Ready);
}

#[test]
fn ready_degrades_on_a_deferred_write_until_the_socket_is_replaced() {
    let mut c = client();
    c.on_open();
    ack_opt_ins_v5(&mut c);
    c.try_enqueue_write(&set("a", "1"), true, true).unwrap();
    c.handle_frame(b"-MISCONF persistence is unhealthy\r\n");
    assert_eq!(phase(&c), SyncPhase::Degraded);
    c.on_close();
    let status = c.status();
    assert_eq!(status.phase, SyncPhase::Disconnected);
    assert_eq!(
        status.error,
        Some(SyncError::WriteDeferred {
            reason: "MISCONF persistence is unhealthy".into()
        }),
        "the cause of the disconnect stays visible"
    );
    c.on_open();
    ack_opt_ins_v5(&mut c);
    c.handle_frame(b"+OK\r\n");
    assert_eq!(
        c.status(),
        SyncStatus {
            phase: SyncPhase::Ready,
            error: None
        }
    );
}

#[test]
fn a_refused_password_degrades_the_session() {
    let mut c = client();
    c.set_password("wrong", false);
    c.on_open();
    c.handle_frame(b"-WRONGPASS invalid username-password pair\r\n");
    ack_opt_ins_v5(&mut c);
    assert_eq!(phase(&c), SyncPhase::Degraded);
    assert_eq!(
        c.status().error,
        Some(SyncError::AuthFailed {
            reason: "WRONGPASS invalid username-password pair".into()
        })
    );
}

#[test]
fn test_11_a_new_live_query_hydrates_while_the_others_stay_current() {
    let mut c = client();
    c.add_live_query("a:*", false);
    c.on_open();
    ack_opt_ins_v5(&mut c);
    c.handle_frame(&qstate("a:*", &[]));
    assert_eq!(phase(&c), SyncPhase::Ready);

    c.add_live_query("b:*", true);
    assert_eq!(
        c.subscription_state("a:*"),
        Some(SubscriptionState::Current)
    );
    assert_eq!(
        c.subscription_state("b:*"),
        Some(SubscriptionState::Hydrating)
    );
    assert_eq!(
        phase(&c),
        SyncPhase::Ready,
        "a later live query does not take the connection out of Ready"
    );
    c.handle_frame(&qstate("b:*", &[("b:1", "x")]));
    assert_eq!(
        c.subscription_state("b:*"),
        Some(SubscriptionState::Current)
    );
}

#[test]
fn a_query_resubscribed_twice_is_current_only_after_the_last_snapshot() {
    let mut c = client();
    c.add_live_query("a:*", false);
    c.on_open();
    ack_opt_ins_v5(&mut c);
    c.add_live_query("a:*", true); // a second QSUB in flight
    c.handle_frame(&qstate("a:*", &[]));
    assert_eq!(
        c.subscription_state("a:*"),
        Some(SubscriptionState::Hydrating)
    );
    c.handle_frame(&qstate("a:*", &[]));
    assert_eq!(
        c.subscription_state("a:*"),
        Some(SubscriptionState::Current)
    );
}

#[test]
fn status_changes_are_reported_once_each() {
    let mut c = client();
    assert_eq!(
        c.take_status_change().map(|s| s.phase),
        Some(SyncPhase::Disconnected)
    );
    assert_eq!(c.take_status_change(), None);
    c.on_open();
    assert_eq!(
        c.take_status_change().map(|s| s.phase),
        Some(SyncPhase::Authenticating)
    );
    c.handle_frame(b"+OK\r\n");
    assert_eq!(c.take_status_change(), None, "still authenticating");
    for _ in 1..OPT_INS {
        c.handle_frame(b"+OK\r\n");
    }
    assert_eq!(
        c.take_status_change().map(|s| s.phase),
        Some(SyncPhase::Ready)
    );
}

#[test]
fn subscription_changes_are_reported_per_pattern() {
    let mut c = client();
    c.add_live_query("a:*", false);
    c.add_live_query("b:*", false);
    c.take_subscription_changes();
    c.on_open();
    let changes = c.take_subscription_changes();
    assert_eq!(
        changes,
        vec![
            ("a:*".to_string(), Some(SubscriptionState::Hydrating)),
            ("b:*".to_string(), Some(SubscriptionState::Hydrating)),
        ]
    );
    ack_opt_ins_v5(&mut c);
    c.handle_frame(&qstate("a:*", &[]));
    assert_eq!(
        c.take_subscription_changes(),
        vec![("a:*".to_string(), Some(SubscriptionState::Current))],
        "only the pattern that changed"
    );
    c.remove_live_query(Some("b:*"), true);
    assert_eq!(
        c.take_subscription_changes(),
        vec![("b:*".to_string(), None)]
    );
}

#[test]
fn errors_carry_the_server_text_and_never_the_token() {
    let mut c = client();
    c.set_sync_token("secret-token-value", false);
    c.on_open();
    ack_opt_ins_v5(&mut c);
    c.handle_frame(b"-NOSCOPE invalid sync token signature\r\n");
    let shown = format!("{:?}", c.status());
    assert!(!shown.contains("secret-token-value"), "{shown}");
}
