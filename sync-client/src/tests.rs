use super::*;
use core_engine::cmd::Command;

fn client() -> SyncClient {
    SyncClient::new(Arc::new(KeyValueStore::new()), "client-a".to_string())
}

fn get(c: &SyncClient, key: &str) -> Value {
    c.store().execute(Command::Get(key.to_string()))
}

fn bulk(s: &str) -> Value {
    Value::BulkString(Some(s.as_bytes().to_vec()))
}

// ── dedup envelopes ───────────────────────────────────────────────────────────

/// Render a wire frame as text for assertions. Frames are bytes because values
/// may be binary; every frame in these tests has a text payload.
fn t(frame: &[u8]) -> String {
    String::from_utf8_lossy(frame).into_owned()
}

#[test]
fn enqueue_wraps_writes_in_dedup_envelope() {
    let mut c = client();
    let e = c.enqueue_write(&to_resp(&["INCRBY", "n", "2"]), true, false);
    assert_eq!(e.id, 0);
    assert!(!e.send_now);
    assert_eq!(
        t(&e.frame),
        "*6\r\n$5\r\nDEDUP\r\n$8\r\nclient-a\r\n$1\r\n0\r\n$6\r\nINCRBY\r\n$1\r\nn\r\n$1\r\n2\r\n"
    );
    // Ids are monotonic.
    let e2 = c.enqueue_write(&to_resp(&["SET", "k", "v"]), true, false);
    assert_eq!(e2.id, 1);
    assert!(t(&e2.frame).contains("$1\r\n1\r\n"));
}

#[test]
fn epoch_forms_upper_bits_of_wire_id() {
    let mut c = client();
    c.set_epoch(2);
    let e = c.enqueue_write(&to_resp(&["SET", "k", "v"]), true, false);
    let wire_id = (2u64 << 32).to_string();
    assert!(t(&e.frame).contains(&wire_id), "frame: {}", t(&e.frame));
}

#[test]
fn nodedup_writes_pass_through_unwrapped() {
    let mut c = client();
    let plain = to_resp(&["SUBSCRIBE", "news"]);
    let e = c.enqueue_write(&plain, false, false);
    assert_eq!(e.frame, plain);
}

// ── binary values on the write path ──────────────────────────────────────────

#[test]
fn dedup_envelope_preserves_a_binary_payload() {
    // wrap_dedup splices a header onto an already-encoded frame. Parsing that
    // frame as text to do so would corrupt a binary value on the way out — and
    // again on every reconnect replay, since the outbox stores the wrapped form.
    let mut c = client();
    let binary: &[u8] = &[0xff, 0xfe, 0x00, 0x41, 0x80];
    let plain = to_resp_bytes(&[b"SET", b"k", binary]);

    let e = c.enqueue_write(&plain, true, false);

    // The envelope is spliced in front and the original payload is intact.
    assert!(e.frame.starts_with(b"*6\r\n$5\r\nDEDUP\r\n"));
    assert!(
        e.frame.windows(binary.len()).any(|w| w == binary),
        "binary payload must survive dedup wrapping"
    );
    // Byte count: the wrapped frame is the plain frame plus the envelope, so
    // nothing was re-encoded or replaced along the way.
    // Both headers are one digit wide (*3 -> *6), so the wrapped frame is
    // exactly the plain frame plus the three spliced elements.
    assert_eq!(
        e.frame.len(),
        plain.len() + b"$5\r\nDEDUP\r\n$8\r\nclient-a\r\n$1\r\n0\r\n".len(),
        "frame: {:?}",
        e.frame
    );
}

#[test]
fn a_binary_write_replays_unchanged_after_reconnect() {
    let mut c = client();
    let binary: &[u8] = &[0x00, 0xff, 0x1b, 0x80];
    let e = c.enqueue_write(&to_resp_bytes(&[b"SET", b"k", binary]), true, false);

    let frames = c.on_open();
    // CLIENT DELTA ON, then the queued write.
    assert_eq!(frames.len(), 2);
    assert!(t(&frames[0]).contains("DELTA"));
    assert_eq!(
        frames[1], e.frame,
        "replayed frame must be byte-identical to the queued one"
    );
}

#[test]
fn a_binary_pubsub_payload_is_delivered_as_bytes() {
    // Rendering the payload as text here would hand the application a mangled
    // message with no indication anything was lost.
    let mut c = client();
    let binary = vec![0xffu8, 0xfe, 0x41];
    let mut frame = b"*3\r\n$7\r\nmessage\r\n$4\r\nnews\r\n$3\r\n".to_vec();
    frame.extend_from_slice(&binary);
    frame.extend_from_slice(b"\r\n");
    // Re-tag as a Push frame, which is how pub/sub arrives.
    frame[0] = b'>';

    match c.handle_frame(&frame) {
        Incoming::PubSub { channel, message } => {
            assert_eq!(channel, "news");
            assert_eq!(message, binary);
        }
        other => panic!("expected a pub/sub delivery, got {other:?}"),
    }
}

// ── connection lifecycle & replay order ───────────────────────────────────────

#[test]
fn on_open_replays_session_then_outbox_in_order() {
    let mut c = client();
    assert!(c.set_password("pw", false).is_none());
    assert!(c.set_sync_token("tok", false).is_none());
    assert!(c.add_live_query("cart:*", false).is_none());
    let w1 = c.enqueue_write(&to_resp(&["SET", "a", "1"]), true, false);
    let w2 = c.enqueue_write(&to_resp(&["SET", "b", "2"]), true, false);

    let frames = c.on_open();
    assert_eq!(frames.len(), 6);
    assert!(t(&frames[0]).contains("AUTH"));
    // Before QSUB, so the live query's own traffic can already be deltas.
    assert!(t(&frames[1]).contains("DELTA"));
    assert!(t(&frames[2]).contains("TOKEN"));
    assert!(t(&frames[3]).contains("QSUB"));
    assert_eq!(frames[4], w1.frame);
    assert_eq!(frames[5], w2.frame);
}

#[test]
fn session_commands_sent_while_open_occupy_reply_slots() {
    let mut c = client();
    // Open with nothing queued: CLIENT DELTA ON is the only session frame,
    // and it occupies a reply slot like any other.
    assert_eq!(c.on_open().len(), 1);
    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply { retired: None }
    );
    // A session command on the live socket...
    let frame = c.set_sync_token("tok", true);
    assert!(frame.is_some());
    // ...must consume the next reply without retiring any outbox row.
    let w = c.enqueue_write(&to_resp(&["SET", "a", "1"]), true, true);
    assert!(w.send_now);
    assert_eq!(
        c.handle_frame(b"*1\r\n$6\r\ncart:*\r\n"),
        Incoming::Reply { retired: None }
    );
    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply {
            retired: Some(w.id)
        }
    );
    assert_eq!(c.outbox_len(), 0);
}

#[test]
fn backoff_doubles_and_caps() {
    // Jitter puts each delay in [nominal/2, nominal]; assert the band rather
    // than an exact value.
    fn band(delay: u32, nominal: u32) {
        assert!(
            delay >= nominal / 2 && delay <= nominal,
            "delay {delay} outside [{}, {nominal}]",
            nominal / 2
        );
    }

    let mut c = client();
    band(c.on_close(), 500);
    band(c.on_close(), 1000);
    band(c.on_close(), 2000);
    for _ in 0..10 {
        c.on_close();
    }
    band(c.on_close(), 30_000);
    // A successful open resets the schedule.
    c.on_open();
    band(c.on_close(), 500);
}

#[test]
fn backoff_is_jittered_across_clients() {
    // The point of jitter: two clients disconnected by the same event must not
    // compute the same schedule, or they reconnect in lockstep.
    let mut a = SyncClient::new(Arc::new(KeyValueStore::new()), "client-a".to_string());
    let mut b = SyncClient::new(Arc::new(KeyValueStore::new()), "client-b".to_string());

    let seq_a: Vec<u32> = (0..6).map(|_| a.on_close()).collect();
    let seq_b: Vec<u32> = (0..6).map(|_| b.on_close()).collect();
    assert_ne!(seq_a, seq_b, "distinct clients must not share a schedule");
}

#[test]
fn backoff_never_returns_zero_or_exceeds_the_cap() {
    // A zero delay would busy-loop reconnects; exceeding the cap would strand
    // a client for longer than intended.
    let mut c = client();
    for _ in 0..40 {
        let d = c.on_close();
        assert!(d > 0, "backoff must never be zero");
        assert!(d <= BACKOFF_CAP_MS, "backoff {d} exceeded the cap");
    }
}

#[test]
fn jitter_spreads_reconnects_over_the_window() {
    // With many clients the delays should occupy a range, not cluster on one
    // value — that is the property that prevents the herd.
    let delays: Vec<u32> = (0..50)
        .map(|i| {
            let mut c = SyncClient::new(Arc::new(KeyValueStore::new()), format!("client-{i}"));
            for _ in 0..3 {
                c.on_close();
            }
            c.on_close() // nominal 4000ms
        })
        .collect();

    let unique: std::collections::HashSet<u32> = delays.iter().copied().collect();
    assert!(
        unique.len() > 20,
        "expected a spread of delays, got {} distinct values",
        unique.len()
    );
    assert!(delays.iter().all(|d| *d >= 2000 && *d <= 4000));
}

// ── acknowledgment & retirement ───────────────────────────────────────────────

#[test]
fn replies_retire_outbox_rows_in_order() {
    let mut c = client();
    let w1 = c.enqueue_write(&to_resp(&["SET", "a", "1"]), true, false);
    let w2 = c.enqueue_write(&to_resp(&["SET", "b", "2"]), true, false);
    let frames = c.on_open();
    // CLIENT DELTA ON, then the two writes.
    assert_eq!(frames.len(), 3);
    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply { retired: None },
        "the delta opt-in retires no outbox row"
    );

    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply {
            retired: Some(w1.id)
        }
    );
    assert_eq!(c.outbox_len(), 1);
    // +DUP is a reply like any other — the row still retires.
    assert_eq!(
        c.handle_frame(b"+DUP\r\n"),
        Incoming::Reply {
            retired: Some(w2.id)
        }
    );
    assert_eq!(c.outbox_len(), 0);
}

#[test]
fn unacked_rows_survive_reconnect_and_replay() {
    let mut c = client();
    let w = c.enqueue_write(&to_resp(&["INCRBY", "n", "1"]), true, false);
    assert_eq!(c.on_open().len(), 2);
    // Connection dies before the reply arrives.
    c.on_close();
    // The row is still queued; the next open replays the identical frame,
    // behind the session's delta opt-in.
    let frames = c.on_open();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[1], w.frame);
}

#[test]
fn pushes_are_not_replies() {
    let mut c = client();
    let w = c.enqueue_write(&to_resp(&["SET", "a", "1"]), true, false);
    c.on_open();
    // Retire the session's CLIENT DELTA ON reply first.
    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply { retired: None }
    );
    // A mutation push and a keychange arrive before our reply — neither may
    // consume the reply slot.
    assert_eq!(
        c.handle_frame(b">3\r\n$3\r\nSET\r\n$1\r\nx\r\n$1\r\ny\r\n"),
        Incoming::Applied
    );
    assert_eq!(
        c.handle_frame(b"*3\r\n$9\r\nkeychange\r\n$1\r\nx\r\n$1\r\nz\r\n"),
        Incoming::Applied
    );
    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply {
            retired: Some(w.id)
        }
    );
}

#[test]
fn outbox_overflow_drops_oldest() {
    let mut c = client();
    let first = c.enqueue_write(&to_resp(&["SET", "k0", "v"]), true, false);
    for i in 1..MAX_PENDING_WRITES {
        c.enqueue_write(&to_resp(&["SET", &format!("k{i}"), "v"]), true, false);
    }
    let overflow = c.enqueue_write(&to_resp(&["SET", "last", "v"]), true, false);
    assert_eq!(overflow.dropped, Some(first.id));
    assert_eq!(c.outbox_len(), MAX_PENDING_WRITES);
}

// ── frame application ─────────────────────────────────────────────────────────

#[test]
fn qstate_applies_state_and_counts_as_reply() {
    let mut c = client();
    assert!(c.add_live_query("cart:*", false).is_none());
    c.on_open();
    let qstate = "*4\r\n$6\r\nqstate\r\n$6\r\ncart:*\r\n$6\r\ncart:1\r\n$5\r\napple\r\n";
    assert_eq!(
        c.handle_frame(qstate.as_bytes()),
        Incoming::AppliedReply { retired: None }
    );
    assert_eq!(get(&c, "cart:1"), bulk("apple"));
}

#[test]
fn a_resnapshot_drops_keys_the_snapshot_no_longer_carries() {
    // The gap this closes: `keychange` only reports changes seen while the
    // socket was up, so a key deleted or expired during a disconnect had
    // nothing to announce it. Re-hydration used to only ever *add*, leaving
    // the key in local memory with its last value forever.
    let mut c = client();
    assert!(c.add_live_query("cart:*", false).is_none());
    c.on_open();
    c.handle_frame(
        b"*6\r\n$6\r\nqstate\r\n$6\r\ncart:*\r\n$6\r\ncart:1\r\n$5\r\napple\r\n$6\r\ncart:2\r\n$4\r\npear\r\n",
    );
    assert_eq!(get(&c, "cart:1"), bulk("apple"));
    assert_eq!(get(&c, "cart:2"), bulk("pear"));

    // Reconnect. cart:2 went away while we were offline, so the fresh
    // snapshot simply does not mention it.
    c.on_open();
    c.handle_frame(b"*4\r\n$6\r\nqstate\r\n$6\r\ncart:*\r\n$6\r\ncart:1\r\n$6\r\nbanana\r\n");
    assert_eq!(get(&c, "cart:1"), bulk("banana"));
    assert_eq!(get(&c, "cart:2"), Value::BulkString(None));
}

#[test]
fn a_resnapshot_leaves_keys_outside_its_pattern_alone() {
    // Reconciliation is scoped to the snapshot's own pattern; another live
    // query's keys are not its business.
    let mut c = client();
    c.handle_frame(b"*3\r\n$9\r\nkeychange\r\n$6\r\nuser:1\r\n$3\r\nabc\r\n");
    c.on_open();
    c.handle_frame(b"*4\r\n$6\r\nqstate\r\n$6\r\ncart:*\r\n$6\r\ncart:1\r\n$5\r\napple\r\n");
    assert_eq!(get(&c, "user:1"), bulk("abc"));
    assert_eq!(get(&c, "cart:1"), bulk("apple"));
}

#[test]
fn an_unparseable_frame_is_malformed_not_ignored() {
    // `Ignored` means "understood, nothing to do" and consumes no reply slot,
    // which is right for a push we do not model. A frame we could not parse
    // may well have been a reply the server has already counted — treating it
    // as `Ignored` leaves the inflight FIFO one ahead of reality forever, so
    // every later reply retires the wrong outbox row. The adapter has to drop
    // the socket instead, and only this variant tells it to.
    let mut c = client();
    assert_eq!(
        c.handle_frame(b"*3\r\n$9\r\ntruncated"),
        Incoming::Malformed
    );
    assert_eq!(c.handle_frame(b"not resp at all"), Incoming::Malformed);
}

#[test]
fn keychange_sets_and_deletes() {
    let mut c = client();
    c.handle_frame(b"*3\r\n$9\r\nkeychange\r\n$1\r\nk\r\n$1\r\nv\r\n");
    assert_eq!(get(&c, "k"), bulk("v"));
    c.handle_frame(b"*3\r\n$9\r\nkeychange\r\n$1\r\nk\r\n$-1\r\n");
    assert_eq!(get(&c, "k"), Value::BulkString(None));
}

#[test]
fn pubsub_messages_surface_without_touching_store() {
    let mut c = client();
    let msg = c.handle_frame(b">3\r\n$7\r\nmessage\r\n$4\r\nnews\r\n$5\r\nhello\r\n");
    assert_eq!(
        msg,
        Incoming::PubSub {
            channel: "news".into(),
            message: "hello".into()
        }
    );
}

// ── identity & restore ────────────────────────────────────────────────────────

#[test]
fn client_id_adoption_only_before_writes() {
    let mut c = client();
    assert!(c.adopt_client_id("persisted-id".into()));
    assert_eq!(c.client_id(), "persisted-id");
    c.enqueue_write(&to_resp(&["SET", "a", "1"]), true, false);
    assert!(!c.adopt_client_id("too-late".into()));
    assert_eq!(c.client_id(), "persisted-id");
}

#[test]
fn restore_renumbers_without_collisions_and_preserves_order() {
    let mut c = client();
    // This session already queued one write before restore (connect-first flow).
    let session_write = c.enqueue_write(&to_resp(&["SET", "new", "1"]), true, false);
    // Two rows from the previous session, stored under old ids.
    let old_frame_a = "*5\r\n$5\r\nDEDUP\r\n$8\r\nclient-a\r\n$1\r\n7\r\n$3\r\nDEL\r\n$1\r\na\r\n";
    let old_frame_b = "*5\r\n$5\r\nDEDUP\r\n$8\r\nclient-a\r\n$1\r\n8\r\n$3\r\nDEL\r\n$1\r\nb\r\n";
    let rewrites = c.restore_outbox(vec![(8, old_frame_b.into()), (7, old_frame_a.into())]);

    assert_eq!(
        rewrites
            .iter()
            .map(|(old, new, _)| (*old, *new))
            .collect::<Vec<_>>(),
        vec![(7, 1), (8, 2)]
    );
    // Replay order: restored rows first (oldest first), then this session's.
    let frames = c.on_open();
    assert_eq!(frames.len(), 4);
    assert!(t(&frames[0]).contains("DELTA"));
    assert_eq!(t(&frames[1]), old_frame_a);
    assert_eq!(t(&frames[2]), old_frame_b);
    assert_eq!(frames[3], session_write.frame);

    let next = c.enqueue_write(&to_resp(&["SET", "later", "1"]), true, false);
    assert_eq!(next.id, 3);
}

#[test]
fn restore_cannot_collide_with_a_same_numbered_session_write() {
    let mut c = client();
    let session = c.enqueue_write(&to_resp(&["SET", "new", "1"]), true, false);
    assert_eq!(session.id, 0);
    let old = to_resp(&["SET", "old", "1"]);

    let rewrites = c.restore_outbox(vec![(0, old.clone())]);
    assert_eq!(rewrites[0].1, 1);
    let frames = c.on_open();
    assert_eq!(frames.len(), 3);
    assert!(t(&frames[0]).contains("DELTA"));
    assert_eq!(frames[1..], [old, session.frame]);
}

// ── Sync scopes ───────────────────────────────────────────────────────────────

#[test]
fn sync_scopes_builds_a_sync_frame_from_csv() {
    let mut c = client();
    let frame = c.set_sync_scopes("cart:*,user:1:*", true).unwrap();
    assert!(t(&frame).contains("SYNC"), "{}", t(&frame));
    assert!(t(&frame).contains("cart:*"));
    assert!(t(&frame).contains("user:1:*"));
}

#[test]
fn sync_scopes_ignore_blank_entries_and_whitespace() {
    let mut c = client();
    let frame = c.set_sync_scopes(" cart:* , , user:1:* ,", true).unwrap();
    // Three commas but only two real patterns → SYNC + 2 args.
    assert!(
        t(&frame).starts_with("*3\r\n"),
        "expected 3 parts, got {}",
        t(&frame)
    );
    assert!(t(&frame).contains("cart:*") && t(&frame).contains("user:1:*"));
}

#[test]
fn an_all_empty_scope_list_produces_no_frame() {
    // Sending a bare SYNC would *clear* scopes on the server, which is the
    // opposite of what an accidental empty string intends.
    let mut c = client();
    assert!(c.set_sync_scopes("", true).is_none());
    assert!(c.set_sync_scopes("  ,  , ", true).is_none());
}

#[test]
fn scopes_are_replayed_on_reconnect() {
    let mut c = client();
    c.set_sync_scopes("cart:*", false);
    let frames = c.on_open();
    assert!(
        frames.iter().any(|f| t(f).contains("cart:*")),
        "scopes must be re-established after reconnect: {frames:?}"
    );
}

#[test]
fn a_sync_token_takes_precedence_over_raw_scope_patterns() {
    // The token carries server-signed scopes; sending raw patterns as well
    // would be redundant and could widen what the connection asks for.
    let mut c = client();
    c.set_sync_scopes("cart:*", false);
    c.set_sync_token("tok-123", false);
    let frames = c.on_open();
    assert!(frames.iter().any(|f| t(f).contains("tok-123")));
    assert!(
        !frames.iter().any(|f| t(f).contains("cart:*")),
        "raw scopes must not be sent alongside a token: {frames:?}"
    );
}

// ── Live queries ──────────────────────────────────────────────────────────────

#[test]
fn live_queries_register_once_and_replay_on_open() {
    let mut c = client();
    c.add_live_query("cart:*", false);
    c.add_live_query("cart:*", false); // idempotent
    c.add_live_query("user:*", false);

    let frames = c.on_open();
    let qsubs = frames.iter().filter(|f| t(f).contains("QSUB")).count();
    assert_eq!(
        qsubs, 2,
        "duplicate patterns must not double-subscribe: {frames:?}"
    );
}

#[test]
fn removing_one_live_query_leaves_the_others() {
    let mut c = client();
    c.add_live_query("cart:*", false);
    c.add_live_query("user:*", false);

    let frame = c.remove_live_query(Some("cart:*"), true).unwrap();
    assert!(
        t(&frame).contains("QUNSUB") && t(&frame).contains("cart:*"),
        "{}",
        t(&frame)
    );

    let frames = c.on_open();
    assert!(
        frames.iter().any(|f| t(f).contains("user:*")),
        "survivor replays"
    );
    assert!(
        !frames.iter().any(|f| t(f).contains("cart:*")),
        "removed query must not replay: {frames:?}"
    );
}

#[test]
fn removing_all_live_queries_sends_a_bare_qunsub() {
    let mut c = client();
    c.add_live_query("cart:*", false);
    c.add_live_query("user:*", false);

    let frame = c.remove_live_query(None, true).unwrap();
    assert!(t(&frame).contains("QUNSUB"), "{}", t(&frame));
    assert!(
        !t(&frame).contains("cart:*"),
        "bare QUNSUB carries no pattern: {}",
        t(&frame)
    );

    let frames = c.on_open();
    assert!(
        !frames.iter().any(|f| t(f).contains("QSUB")),
        "nothing should replay after clearing: {frames:?}"
    );
}

// ── Session frame ordering ────────────────────────────────────────────────────

#[test]
fn on_open_sends_auth_before_scopes_before_queries() {
    // Order is load-bearing: the server rejects scoped commands until it has
    // authenticated, and a QSUB before SYNC would be refused.
    let mut c = client();
    c.set_password("pw", false);
    c.set_sync_token("tok", false);
    c.add_live_query("cart:*", false);

    let frames = c.on_open();
    let pos = |needle: &str| frames.iter().position(|f| t(f).contains(needle));
    let (auth, sync, qsub) = (
        pos("AUTH").unwrap(),
        pos("TOKEN").unwrap(),
        pos("QSUB").unwrap(),
    );
    assert!(auth < sync, "AUTH must precede SYNC: {frames:?}");
    assert!(sync < qsub, "SYNC must precede QSUB: {frames:?}");
}

#[test]
fn on_open_resets_attempt_count_and_inflight() {
    let mut c = client();
    c.on_close();
    c.on_close();
    c.enqueue_write(&to_resp(&["SET", "k", "v"]), true, true);

    c.on_open();
    // Backoff restarts from the floor after a successful open (jittered).
    let d = c.on_close();
    assert!((250..=500).contains(&d), "expected a reset floor, got {d}");
}

#[test]
fn session_frames_are_withheld_until_connected() {
    let mut c = client();
    assert!(
        c.add_live_query("cart:*", false).is_none(),
        "nothing to send while disconnected — it replays on open instead"
    );
    assert!(
        c.add_live_query("user:*", true).is_some(),
        "sent immediately when open"
    );
}

// ── Outbox management ─────────────────────────────────────────────────────────

#[test]
fn clear_outbox_drops_queued_and_inflight_writes() {
    let mut c = client();
    c.enqueue_write(&to_resp(&["SET", "a", "1"]), true, true);
    c.enqueue_write(&to_resp(&["SET", "b", "2"]), true, true);
    assert_eq!(c.outbox_len(), 2);

    c.clear_outbox();
    assert_eq!(c.outbox_len(), 0);
    // A reply arriving after a clear must not retire a row that no longer
    // exists or panic on an empty inflight queue.
    c.handle_frame(b"+OK\r\n");
    assert_eq!(c.outbox_len(), 0);
}

#[test]
fn no_writes_yet_tracks_whether_an_identity_can_still_be_adopted() {
    let mut c = client();
    assert!(c.no_writes_yet(), "fresh client has written nothing");
    c.enqueue_write(&to_resp(&["SET", "k", "v"]), true, true);
    assert!(
        !c.no_writes_yet(),
        "after a write the client id is committed to the wire"
    );
}

#[test]
fn epoch_is_readable_and_settable() {
    let mut c = client();
    assert_eq!(c.epoch(), 0);
    c.set_epoch(7);
    assert_eq!(c.epoch(), 7);
}

// ── Type-tagged collection values ─────────────────────────────────────────────
// A live query used to deliver only a type name for collections, forcing the
// subscriber into a follow-up HGETALL/LRANGE — a network round-trip in a system
// built on local reads. Values now arrive tagged and complete.

/// Build the keychange frame a server sends for `key`, using the real
/// `get_current` encoding rather than a hand-written fixture.
fn keychange_frame(source: &KeyValueStore, key: &str) -> Vec<Value> {
    vec![
        Value::BulkString(Some(b"keychange".to_vec())),
        Value::BulkString(Some(key.as_bytes().to_vec())),
        source.get_current(key),
    ]
}

/// keychange/qstate frames are plain RESP arrays, matching the wire format
/// the existing tests use.
fn push(items: Vec<Value>) -> String {
    String::from_utf8_lossy(&Value::Array(Some(items)).serialize()).into_owned()
}

#[test]
fn keychange_rebuilds_a_hash_without_a_re_read() {
    let source = KeyValueStore::new();
    source.execute(Command::HSet(
        "cart:42".into(),
        vec![("item".into(), "book".into()), ("qty".into(), "2".into())],
    ));

    let mut c = client();
    c.handle_frame(push(keychange_frame(&source, "cart:42")).as_bytes());

    assert_eq!(
        c.store()
            .execute(Command::HGet("cart:42".into(), "item".into())),
        bulk("book")
    );
    assert_eq!(
        c.store()
            .execute(Command::HGet("cart:42".into(), "qty".into())),
        bulk("2")
    );
}

#[test]
fn keychange_rebuilds_lists_in_order() {
    let source = KeyValueStore::new();
    source.execute(Command::RPush(
        "queue".into(),
        vec!["first".into(), "second".into(), "third".into()],
    ));

    let mut c = client();
    c.handle_frame(push(keychange_frame(&source, "queue")).as_bytes());

    assert_eq!(
        c.store().execute(Command::LRange("queue".into(), 0, -1)),
        Value::Array(Some(vec![bulk("first"), bulk("second"), bulk("third")])),
        "list order must survive the round trip"
    );
}

#[test]
fn keychange_rebuilds_sets_and_zsets() {
    let source = KeyValueStore::new();
    source.execute(Command::SAdd(
        "tags".into(),
        vec!["red".into(), "blue".into()],
    ));
    source.execute(Command::ZAdd(
        "board".into(),
        Default::default(),
        vec![(10.0, "alice".into()), (5.0, "bob".into())],
    ));

    let mut c = client();
    c.handle_frame(push(keychange_frame(&source, "tags")).as_bytes());
    c.handle_frame(push(keychange_frame(&source, "board")).as_bytes());

    assert_eq!(
        c.store().execute(Command::SCard("tags".into())),
        Value::Integer(2)
    );
    assert_eq!(
        c.store()
            .execute(Command::ZScore("board".into(), "alice".into())),
        bulk("10")
    );
}

#[test]
fn keychange_rebuilds_json_documents() {
    let source = KeyValueStore::new();
    source.execute(Command::JSet(
        "doc".into(),
        "$".into(),
        "{\"a\":1}".to_string(),
    ));

    let mut c = client();
    c.handle_frame(push(keychange_frame(&source, "doc")).as_bytes());

    assert_eq!(
        c.store().execute(Command::JGet("doc".into(), None)),
        bulk("{\"a\":1}")
    );
}

#[test]
fn a_removed_member_disappears_from_the_local_copy() {
    // The frame carries the complete value, so the key is rebuilt rather than
    // merged — otherwise a removal would never propagate.
    let source = KeyValueStore::new();
    source.execute(Command::SAdd(
        "tags".into(),
        vec!["red".into(), "blue".into()],
    ));

    let mut c = client();
    c.handle_frame(push(keychange_frame(&source, "tags")).as_bytes());
    assert_eq!(
        c.store().execute(Command::SCard("tags".into())),
        Value::Integer(2)
    );

    source.execute(Command::SRem("tags".into(), vec!["red".into()]));
    c.handle_frame(push(keychange_frame(&source, "tags")).as_bytes());

    assert_eq!(
        c.store()
            .execute(Command::SIsMember("tags".into(), "red".into())),
        Value::Integer(0),
        "a member removed on the server must not linger locally"
    );
    assert_eq!(
        c.store().execute(Command::SCard("tags".into())),
        Value::Integer(1)
    );
}

#[test]
fn qstate_delivers_complete_collections_on_subscribe() {
    // The initial state of a live query is now usable immediately.
    let source = KeyValueStore::new();
    source.execute(Command::HSet(
        "cart:1".into(),
        vec![("item".into(), "pen".into())],
    ));
    source.execute(Command::RPush("cart:2".into(), vec!["a".into()]));

    let mut c = client();
    let frame = push(vec![
        Value::BulkString(Some(b"qstate".to_vec())),
        Value::BulkString(Some(b"cart:*".to_vec())),
        Value::BulkString(Some(b"cart:1".to_vec())),
        source.get_current("cart:1"),
        Value::BulkString(Some(b"cart:2".to_vec())),
        source.get_current("cart:2"),
    ]);
    c.handle_frame(frame.as_bytes());

    assert_eq!(
        c.store()
            .execute(Command::HGet("cart:1".into(), "item".into())),
        bulk("pen")
    );
    assert_eq!(
        c.store().execute(Command::LLen("cart:2".into())),
        Value::Integer(1)
    );
}

#[test]
fn an_unknown_type_tag_is_ignored_rather_than_guessed() {
    // Forward compatibility: a newer server sending a type this client does not
    // know must not corrupt the local copy.
    let mut c = client();
    c.handle_frame(
        push(vec![
            Value::BulkString(Some(b"keychange".to_vec())),
            Value::BulkString(Some(b"k".to_vec())),
            Value::Array(Some(vec![
                Value::BulkString(Some(b"futuretype".to_vec())),
                Value::BulkString(Some(b"payload".to_vec())),
            ])),
        ])
        .as_bytes(),
    );
    assert_eq!(get(&c, "k"), Value::BulkString(None));
}

// ── FLUSHDB propagation ───────────────────────────────────────────────────────

#[test]
fn flushdb_sentinel_clears_every_key_matching_the_pattern() {
    // The server announces a flush once per registered pattern rather than once
    // per deleted key — a keyspace-sized frame storm for a single command.
    let mut c = client();
    c.add_live_query("cart:*", false);
    c.handle_frame(b"*3\r\n$9\r\nkeychange\r\n$11\r\ncart:item:1\r\n$1\r\na\r\n");
    c.handle_frame(b"*3\r\n$9\r\nkeychange\r\n$11\r\ncart:item:2\r\n$1\r\nb\r\n");
    c.handle_frame(b"*3\r\n$9\r\nkeychange\r\n$7\r\nother:1\r\n$1\r\nc\r\n");
    assert_eq!(get(&c, "cart:item:1"), bulk("a"));

    // Sentinel: nil value whose "key" is the registered pattern.
    c.handle_frame(b"*3\r\n$9\r\nkeychange\r\n$6\r\ncart:*\r\n$-1\r\n");

    assert_eq!(get(&c, "cart:item:1"), Value::BulkString(None));
    assert_eq!(get(&c, "cart:item:2"), Value::BulkString(None));
    assert_eq!(
        get(&c, "other:1"),
        bulk("c"),
        "keys outside the pattern must be untouched"
    );
}

#[test]
fn a_nil_for_an_unregistered_pattern_deletes_only_that_key() {
    // Without this distinction, a literal key that happens to contain a glob
    // character would wipe unrelated data.
    let mut c = client();
    c.handle_frame(b"*3\r\n$9\r\nkeychange\r\n$5\r\nkey:1\r\n$1\r\na\r\n");
    c.handle_frame(b"*3\r\n$9\r\nkeychange\r\n$5\r\nkey:2\r\n$1\r\nb\r\n");

    // Not a registered live query — treat it as an ordinary single-key delete.
    c.handle_frame(b"*3\r\n$9\r\nkeychange\r\n$5\r\nkey:1\r\n$-1\r\n");

    assert_eq!(get(&c, "key:1"), Value::BulkString(None));
    assert_eq!(get(&c, "key:2"), bulk("b"), "unrelated key survives");
}

#[test]
fn flushdb_sentinel_is_harmless_when_nothing_matches() {
    let mut c = client();
    c.add_live_query("cart:*", false);
    c.handle_frame(b"*3\r\n$9\r\nkeychange\r\n$6\r\ncart:*\r\n$-1\r\n");
    assert_eq!(c.store().execute(Command::DbSize), Value::Integer(0));
}

// ── Configurable outbox cap ───────────────────────────────────────────────────

#[test]
fn outbox_cap_defaults_to_the_documented_limit() {
    assert_eq!(client().max_pending(), MAX_PENDING_WRITES);
}

#[test]
fn a_lower_cap_evicts_sooner() {
    // The right depth depends on how long a client may be offline and how large
    // its writes are, so it is configurable rather than fixed.
    let mut c = client();
    c.set_max_pending(3);
    for i in 0..3 {
        let e = c.enqueue_write(&to_resp(&["SET", &format!("k{i}"), "v"]), true, false);
        assert!(e.dropped.is_none(), "within the cap, nothing is evicted");
    }
    let e = c.enqueue_write(&to_resp(&["SET", "k3", "v"]), true, false);
    assert!(e.dropped.is_some(), "past the cap the oldest is evicted");
    assert_eq!(c.outbox_len(), 3, "depth stays at the cap");
}

#[test]
fn a_cap_of_zero_is_clamped_to_one() {
    // A zero cap would discard every write immediately; clamp rather than
    // silently break the client.
    let mut c = client();
    c.set_max_pending(0);
    assert_eq!(c.max_pending(), 1);
    c.enqueue_write(&to_resp(&["SET", "k", "v"]), true, false);
    assert_eq!(c.outbox_len(), 1);
}

// ── delta frames ──────────────────────────────────────────────────────────────
//
// A delta is the mutation, not a diff, so applying one is executing that
// command against the local store — the same engine the server ran it on.

#[test]
fn on_open_asks_for_deltas_before_subscribing() {
    let mut c = client();
    assert!(c.add_live_query("cart:*", false).is_none());
    let frames = c.on_open();
    let texts: Vec<String> = frames.iter().map(|f| t(f)).collect();
    let delta = texts.iter().position(|f| f.contains("DELTA")).unwrap();
    let qsub = texts.iter().position(|f| f.contains("QSUB")).unwrap();
    assert!(
        delta < qsub,
        "the opt-in must precede QSUB so the live query's own traffic is compact"
    );
}

#[test]
fn an_append_delta_is_applied_to_the_local_value() {
    let mut c = client();
    c.store().execute(Command::Set(
        "out".into(),
        b"hello".to_vec(),
        Default::default(),
    ));
    assert_eq!(
        c.handle_frame(b"*4\r\n$8\r\nkeydelta\r\n$3\r\nout\r\n$6\r\nappend\r\n$6\r\n world\r\n"),
        Incoming::Applied
    );
    assert_eq!(get(&c, "out"), bulk("hello world"));
}

#[test]
fn collection_deltas_are_applied_to_the_local_collection() {
    let mut c = client();
    assert_eq!(
        c.handle_frame(
            b"*5\r\n$8\r\nkeydelta\r\n$4\r\ntags\r\n$4\r\nsadd\r\n$1\r\na\r\n$1\r\nb\r\n"
        ),
        Incoming::Applied
    );
    assert_eq!(
        c.store().execute(Command::SCard("tags".into())),
        Value::Integer(2)
    );
    assert_eq!(
        c.handle_frame(b"*4\r\n$8\r\nkeydelta\r\n$4\r\ntags\r\n$4\r\nsrem\r\n$1\r\na\r\n"),
        Incoming::Applied
    );
    assert_eq!(
        c.store().execute(Command::SCard("tags".into())),
        Value::Integer(1)
    );

    // A push delta lands in order on the local list.
    assert_eq!(
        c.handle_frame(b"*4\r\n$8\r\nkeydelta\r\n$1\r\nq\r\n$5\r\nrpush\r\n$1\r\nx\r\n"),
        Incoming::Applied
    );
    assert_eq!(
        c.handle_frame(b"*4\r\n$8\r\nkeydelta\r\n$1\r\nq\r\n$5\r\nrpush\r\n$1\r\ny\r\n"),
        Incoming::Applied
    );
    assert_eq!(
        c.store().execute(Command::LRange("q".into(), 0, -1)),
        Value::Array(Some(vec![bulk("x"), bulk("y")]))
    );
}

#[test]
fn a_delta_never_consumes_a_reply_slot() {
    // Same invariant as keychange: notifications are not replies.
    let mut c = client();
    let w = c.enqueue_write(&to_resp(&["SET", "a", "1"]), true, false);
    c.on_open();
    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply { retired: None },
        "the delta opt-in's own reply"
    );
    assert_eq!(
        c.handle_frame(b"*4\r\n$8\r\nkeydelta\r\n$1\r\nk\r\n$6\r\nappend\r\n$1\r\nz\r\n"),
        Incoming::Applied
    );
    assert_eq!(
        c.handle_frame(b"+OK\r\n"),
        Incoming::Reply {
            retired: Some(w.id)
        }
    );
}

#[test]
fn a_malformed_or_unknown_delta_is_reported_rather_than_swallowed() {
    // Dropping one silently would leave the local copy wrong with no signal.
    let mut c = client();
    // Too few parts.
    assert_eq!(
        c.handle_frame(b"*3\r\n$8\r\nkeydelta\r\n$1\r\nk\r\n$6\r\nappend\r\n"),
        Incoming::Ignored
    );
    // An op that is not a replayable mutation.
    assert_eq!(
        c.handle_frame(b"*4\r\n$8\r\nkeydelta\r\n$1\r\nk\r\n$3\r\nget\r\n$1\r\nx\r\n"),
        Incoming::Ignored
    );
    // An op that does not exist at all.
    assert_eq!(
        c.handle_frame(b"*4\r\n$8\r\nkeydelta\r\n$1\r\nk\r\n$6\r\nnosuch\r\n$1\r\nx\r\n"),
        Incoming::Ignored
    );
}
