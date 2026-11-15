//! Unit tests for [`SyncRouter`]: candidate selection, the index's upkeep,
//! and mailbox backpressure. End-to-end routing over real sockets is in
//! `tests/sync_routing.rs`.

use super::*;

fn grants(patterns: &[&str]) -> Vec<Grant> {
    patterns.iter().map(|p| Grant::rw(p)).collect()
}

fn set(key: &str) -> Command {
    Command::Set(key.into(), "v".into(), SetOptions::default())
}

/// A push for `cmd` exactly as the write path builds one.
fn push_for(cmd: &Command) -> SyncMsg {
    Arc::new(SyncPush::for_command(
        0,
        cmd,
        push_keys(cmd),
        b"frame".to_vec(),
    ))
}

/// Drain everything queued for `sub` without waiting.
fn drain(sub: &mut SyncSubscription) -> usize {
    let mut n = 0;
    while sub.try_recv().is_ok() {
        n += 1;
    }
    n
}

#[test]
fn placement_is_reduced_to_a_prefix_free_set() {
    let p = Placement::of(Routes::Grants(&grants(&[
        "tenant:a:*",
        "tenant:a:book:*",
        "tenant:a:book:1",
        "tenant:b:x",
        "tenant:b:x",
        "tenant:ab:*",
        "tenant:?:y",
    ])));
    assert!(!p.broad);
    // `tenant:?:y` has the literal prefix `tenant:`, which covers every other
    // prefix and exact key here.
    assert_eq!(p.prefixes, vec![Box::from(&b"tenant:"[..])]);
    assert!(p.exact.is_empty(), "{p:?}");

    let p = Placement::of(Routes::Grants(&grants(&[
        "tenant:a:*",
        "tenant:a:book:*",
        "tenant:a:book:1",
        "tenant:b:x",
        "tenant:b:x",
        "tenant:ab:*",
    ])));
    assert_eq!(
        p.prefixes,
        vec![Box::from(&b"tenant:a:"[..]), Box::from(&b"tenant:ab:"[..])]
    );
    assert_eq!(p.exact, vec![Box::from("tenant:b:x")]);
}

#[test]
fn a_grant_with_no_literal_prefix_is_routed_everything() {
    for pattern in ["*", "*:user1", "?x", "*a*"] {
        let p = Placement::of(Routes::Grants(&grants(&["tenant:a:*", pattern])));
        assert!(p.broad, "{pattern}");
        assert!(p.prefixes.is_empty() && p.exact.is_empty());
    }
    assert!(Placement::of(Routes::Everything).broad);
    assert!(Placement::of(Routes::Nothing).is_empty());
    // A token granting no keys (channels only) is routed — it sees FLUSHDB —
    // but indexed nowhere.
    let none = Placement::of(Routes::Grants(&[]));
    assert!(!none.is_empty() && !none.broad && none.exact.is_empty() && none.prefixes.is_empty());
}

#[test]
fn a_write_to_one_tenant_wakes_only_that_tenant() {
    // The Phase 6J requirement: 200 + 300 + 500 connections in three tenants;
    // a write to tenant A is offered to A's 200 and to nobody else.
    let router = SyncRouter::new(16);
    let mut subs = Vec::new();
    for (tenant, n) in [("A", 200), ("B", 300), ("C", 500)] {
        for _ in 0..n {
            let g = grants(&[&format!("tenant:{tenant}:*")]);
            subs.push((tenant, router.subscribe(Routes::Grants(&g))));
        }
    }
    let queued = router.send(push_for(&set("tenant:A:book:123")));
    assert_eq!(queued, 200);
    let stats = router.stats();
    assert_eq!((stats.events, stats.candidates), (1, 200));
    for (tenant, sub) in &mut subs {
        assert_eq!(drain(sub), usize::from(*tenant == "A"), "tenant {tenant}");
    }
    // A key in no tenant is offered to nobody, and builds no push.
    assert!(!router.would_route(&set("tenant:D:x")));
    assert_eq!(router.send(push_for(&set("tenant:D:x"))), 0);
    // The namespace boundary is part of the prefix.
    assert_eq!(router.send(push_for(&set("tenant:AB:x"))), 0);
}

#[test]
fn overlapping_grants_and_broad_subscribers_get_one_offer_each() {
    let router = SyncRouter::new(16);
    let g = grants(&["tenant:*", "tenant:a:*", "tenant:a:k"]);
    let mut overlap = router.subscribe(Routes::Grants(&g));
    let mut broad = router.subscribe(Routes::Grants(&grants(&["*:k"])));
    let mut open = router.subscribe(Routes::Everything);
    let mut exact = router.subscribe(Routes::Grants(&grants(&["tenant:a:k"])));
    let mut other = router.subscribe(Routes::Grants(&grants(&["tenant:a:j"])));
    assert_eq!(router.send(push_for(&set("tenant:a:k"))), 4);
    assert_eq!(drain(&mut overlap), 1);
    assert_eq!(drain(&mut broad), 1);
    assert_eq!(drain(&mut open), 1);
    assert_eq!(drain(&mut exact), 1);
    assert_eq!(drain(&mut other), 0);
}

#[test]
fn routing_never_selects_fewer_than_the_authoritative_filter_allows() {
    // For a spread of grants and keys: whenever `forwards_push` would deliver
    // a push to a connection holding those grants, the router offered it.
    let grant_sets: &[&[&str]] = &[
        &["tenant:a:*"],
        &["tenant:a:k"],
        &["tenant:?:k"],
        &["tenant:a:*:x", "other:*"],
        &["*:k"],
        &["tenant:a:k", "tenant:a:*"],
        &["t*"],
        &["tenant:a:k*"],
        &["ü:*", "日本:?"],
    ];
    let keys = [
        "tenant:a:k",
        "tenant:a:kk",
        "tenant:b:k",
        "tenant:a:y:x",
        "other:1",
        "t",
        "",
        "ü:1",
        "日本:x",
        "tenant:",
    ];
    for set_ in grant_sets {
        let g = grants(set_);
        let router = SyncRouter::new(64);
        let mut sub = router.subscribe(Routes::Grants(&g));
        for key in keys {
            for cmd in [
                set(key),
                Command::Del(vec![key.into(), "tenant:a:k".into()]),
                Command::Copy("tenant:a:k".into(), key.into(), false),
                Command::Rename(key.into(), "tenant:a:k".into()),
                Command::SInterStore(key.into(), vec!["tenant:a:k".into()]),
            ] {
                let push = push_for(&cmd);
                let visible = forwards_push(
                    &push,
                    u64::MAX,
                    Some(&g),
                    true,
                    &HashSet::new(),
                    &HashSet::new(),
                );
                let would = router.would_route(&cmd);
                let offered = router.send(push) == 1;
                drain(&mut sub);
                assert_eq!(would, offered, "{set_:?} {cmd:?}");
                assert!(
                    !visible || offered,
                    "under-selected: grants {set_:?} cmd {cmd:?}"
                );
            }
        }
    }
}

#[test]
fn a_multi_key_push_naming_another_tenant_is_offered_but_not_visible() {
    // COPY / *STORE name their sources. Routing on the destination makes the
    // destination tenant a candidate; the authoritative filter then refuses
    // the frame because it names a key the tenant cannot read (Phase 6F).
    let router = SyncRouter::new(16);
    let b = grants(&["tenant:b:*"]);
    let mut sub = router.subscribe(Routes::Grants(&b));
    for cmd in [
        Command::Copy("tenant:a:secret".into(), "tenant:b:dst".into(), false),
        Command::SUnionStore("tenant:b:dst".into(), vec!["tenant:a:secret".into()]),
        Command::Rename("tenant:b:x".into(), "tenant:a:y".into()),
    ] {
        router.send(push_for(&cmd));
        while let Ok(push) = sub.try_recv() {
            assert!(
                !forwards_push(&push, 1, Some(&b), true, &HashSet::new(), &HashSet::new()),
                "{cmd:?} reached the destination tenant"
            );
        }
    }
    // Routed on the first key named: a RENAME from tenant A into B is not
    // even offered to B.
    assert!(!router.would_route(&Command::Rename("tenant:a:x".into(), "tenant:b:y".into())));
}

#[test]
fn keyspace_wide_pushes_reach_everyone_keyless_ones_only_the_broad_set() {
    let router = SyncRouter::new(16);
    let mut scoped = router.subscribe(Routes::Grants(&grants(&["tenant:a:*"])));
    let mut tokenless = router.subscribe(Routes::Nothing);
    let mut open = router.subscribe(Routes::Everything);
    assert_eq!(router.send(push_for(&Command::FlushDb)), 3);
    let wrapped = Command::Dedup("c".into(), 1, Box::new(Command::FlushDb));
    assert!(router.would_route(&wrapped));
    assert_eq!(router.send(push_for(&wrapped)), 3);
    assert_eq!(
        (drain(&mut scoped), drain(&mut tokenless), drain(&mut open)),
        (2, 2, 2)
    );
    // A keyless push that is not keyspace-wide is offered to the broad set
    // only — and the authoritative filter refuses it to scoped sockets.
    let keyless = Arc::new(SyncPush {
        origin: 0,
        keys: Vec::new(),
        resp: Vec::new(),
        keyspace_wide: false,
    });
    assert_eq!(router.send(keyless), 1);
    assert_eq!(
        (drain(&mut scoped), drain(&mut tokenless), drain(&mut open)),
        (0, 0, 1)
    );
}

#[test]
fn routes_follow_renewal_narrowing_and_close() {
    let router = SyncRouter::new(16);
    let mut sub = router.subscribe(Routes::Nothing);
    assert!(!router.is_active(), "a tokenless socket routes nothing");
    assert_eq!(router.send(push_for(&set("tenant:a:k"))), 0);

    sub.set_routes(Routes::Grants(&grants(&["tenant:a:*"])));
    assert!(router.is_active());
    assert_eq!(router.send(push_for(&set("tenant:a:k"))), 1);
    // Renewal with the same grants changes nothing.
    sub.set_routes(Routes::Grants(&grants(&["tenant:a:*"])));
    assert_eq!(router.stats().prefixes, 1);
    // Narrowing.
    sub.set_routes(Routes::Grants(&grants(&["tenant:a:x:*"])));
    assert_eq!(router.send(push_for(&set("tenant:a:k"))), 0);
    assert_eq!(router.send(push_for(&set("tenant:a:x:1"))), 1);
    // Widening to a broad grant, and back.
    sub.set_routes(Routes::Grants(&grants(&["*"])));
    assert_eq!(router.send(push_for(&set("anything"))), 1);
    sub.set_routes(Routes::Grants(&grants(&["tenant:b:k"])));
    assert_eq!(router.send(push_for(&set("anything"))), 0);
    assert_eq!(router.send(push_for(&set("tenant:b:k"))), 1);
    assert_eq!(drain(&mut sub), 4);

    drop(sub);
    let stats = router.stats();
    assert_eq!(
        (
            stats.subscribers,
            stats.routed,
            stats.broad,
            stats.exact_keys,
            stats.prefixes
        ),
        (0, 0, 0, 0, 0)
    );
    assert!(!router.is_active());
}

#[test]
fn shared_buckets_survive_one_subscriber_leaving() {
    let router = SyncRouter::new(16);
    let g = grants(&["tenant:a:*", "tenant:a:k"]);
    let first = router.subscribe(Routes::Grants(&g));
    let mut second = router.subscribe(Routes::Grants(&g));
    drop(first);
    assert_eq!(router.send(push_for(&set("tenant:a:k"))), 1);
    assert_eq!(drain(&mut second), 1);
}

#[test]
fn reconnect_churn_leaves_no_index_behind() {
    let router = SyncRouter::new(4);
    let keep = router.subscribe(Routes::Grants(&grants(&["keep:*"])));
    for round in 0..50 {
        let subs: Vec<_> = (0..200)
            .map(|i| {
                let g = grants(&[&format!("t{i}:*"), &format!("e:{round}:{i}")]);
                router.subscribe(Routes::Grants(&g))
            })
            .collect();
        for (i, sub) in subs.iter().enumerate() {
            if i % 2 == 0 {
                sub.set_routes(Routes::Everything);
            }
        }
        drop(subs);
    }
    let stats = router.stats();
    assert_eq!(
        (
            stats.subscribers,
            stats.routed,
            stats.broad,
            stats.exact_keys,
            stats.prefixes
        ),
        (1, 1, 0, 0, 1)
    );
    drop(keep);
    assert_eq!(router.stats().subscribers, 0);
}

#[test]
fn a_full_mailbox_is_a_gap_and_nothing_after_it_is_delivered() {
    let router = SyncRouter::new(2);
    let g = grants(&["k:*"]);
    let mut slow = router.subscribe(Routes::Grants(&g));
    let mut fast = router.subscribe(Routes::Grants(&g));
    for i in 0..2 {
        assert_eq!(router.send(push_for(&set(&format!("k:{i}")))), 2);
    }
    assert_eq!(drain(&mut fast), 2);
    // The third does not fit `slow`: it has a gap, and `fast` is unaffected.
    assert_eq!(router.send(push_for(&set("k:2"))), 1);
    assert_eq!(slow.try_recv().err(), Some(TryPushRecvError::Lagged));
    // Pushes queued before the gap are not delivered after it either.
    assert_eq!(slow.try_recv().err(), Some(TryPushRecvError::Lagged));
    // A gapped mailbox is not offered more work.
    assert_eq!(router.send(push_for(&set("k:3"))), 1);
    assert_eq!(drain(&mut fast), 2);
}

#[tokio::test]
async fn recv_reports_a_gap_and_shutdown() {
    let router = SyncRouter::new(1);
    let mut sub = router.subscribe(Routes::Everything);
    router.send(push_for(&set("a")));
    router.send(push_for(&set("b")));
    assert_eq!(sub.recv().await.err(), Some(PushRecvError::Lagged));

    let mut ok = router.subscribe(Routes::Everything);
    router.send(push_for(&set("c")));
    assert_eq!(
        ok.recv().await.map(|p| p.keys.clone()).ok(),
        Some(vec!["c".to_string()])
    );
    // A waiting receiver wakes for a push.
    let waiter = tokio::spawn(async move { ok.recv().await.map(|p| p.keys.clone()) });
    tokio::task::yield_now().await;
    router.send(push_for(&set("d")));
    assert_eq!(
        waiter.await.ok().and_then(Result::ok),
        Some(vec!["d".to_string()])
    );
}

#[test]
fn concurrent_routing_and_registration_deliver_only_to_candidates() {
    // Writers route while subscribers come, change routes and go. Every push
    // a subscriber receives must match its grants at some point in its life
    // (it only ever holds `own:<n>:*` or nothing), and the index must end empty.
    let router = SyncRouter::new(1024);
    let stop = Arc::new(AtomicBool::new(false));
    let writers: Vec<_> = (0..4)
        .map(|w| {
            let router = Arc::clone(&router);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                let mut i = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    let key = format!("own:{}:{i}", (i + w) % 16);
                    router.send(push_for(&set(&key)));
                    i += 1;
                }
            })
        })
        .collect();
    let churners: Vec<_> = (0..4)
        .map(|c| {
            let router = Arc::clone(&router);
            std::thread::spawn(move || {
                for round in 0..300u64 {
                    let n = (round + c) % 16;
                    let mine = format!("own:{n}:");
                    let g = grants(&[&format!("{mine}*")]);
                    let mut sub = router.subscribe(Routes::Nothing);
                    sub.set_routes(Routes::Grants(&g));
                    std::thread::yield_now();
                    while let Ok(push) = sub.try_recv() {
                        assert!(
                            push.keys.iter().all(|k| k.starts_with(&mine)),
                            "{:?}",
                            push.keys
                        );
                    }
                    sub.set_routes(Routes::Nothing);
                    drop(sub);
                }
            })
        })
        .collect();
    for c in churners {
        c.join().expect("churner");
    }
    stop.store(true, Ordering::Relaxed);
    for w in writers {
        w.join().expect("writer");
    }
    let stats = router.stats();
    assert_eq!((stats.subscribers, stats.routed, stats.prefixes), (0, 0, 0));
}

#[test]
fn a_channel_only_token_still_sees_a_keyspace_wide_push() {
    // 6J-E F5: with no key grants the connection used to be unrouted, and if
    // it was the only connection, FLUSHDB was not offered to it at all.
    let router = SyncRouter::new(8);
    let mut sub = router.subscribe(Routes::Grants(&[]));
    assert!(router.is_active());
    assert!(router.would_route(&Command::FlushDb));
    assert_eq!(router.send(push_for(&Command::FlushDb)), 1);
    assert_eq!(router.send(push_for(&set("any"))), 0);
    assert_eq!(drain(&mut sub), 1);
}

#[test]
fn a_mailbox_is_bounded_by_bytes_as_well_as_pushes() {
    // 6J-E F2: a connection that stops reading pins at most its byte budget
    // (plus one frame), not 512 frames of any size.
    let router = SyncRouter::with_byte_budget(512, 1_000);
    let big = |n: usize| {
        Arc::new(SyncPush {
            origin: 0,
            keys: vec!["k".into()],
            resp: vec![b'x'; n],
            keyspace_wide: false,
        })
    };
    let mut fast = router.subscribe(Routes::Everything);
    // An empty mailbox takes one push larger than the whole budget.
    assert_eq!(router.send(big(5_000)), 1);
    assert_eq!(drain(&mut fast), 1);
    // Then the budget holds: 600 + 600 > 1,000.
    assert_eq!(router.send(big(600)), 1);
    assert_eq!(router.send(big(600)), 0);
    assert_eq!(fast.try_recv().err(), Some(TryPushRecvError::Lagged));
    // Released bytes are reusable: a reader that keeps up never gaps.
    let mut steady = router.subscribe(Routes::Everything);
    for _ in 0..100 {
        router.send(big(900));
        assert_eq!(drain(&mut steady), 1);
    }
}

#[test]
fn deciding_a_push_does_not_fit_and_recording_the_gap_are_one_step() {
    // 6J-E F4: a push dropped for want of room, then a slot freed and a later
    // push queued, then the gap recorded — the later push was deliverable.
    // Now the full decision sets the gap in the same compare-exchange, so a
    // push offered after a drop is refused, and one taken after the gap is
    // not delivered even if it was queued before it.
    let router = SyncRouter::new(2);
    let mut sub = router.subscribe(Routes::Everything);
    assert_eq!(router.send(push_for(&set("a"))), 1);
    assert_eq!(router.send(push_for(&set("b"))), 1);
    assert_eq!(router.send(push_for(&set("x"))), 0, "full: dropped");
    // A slot frees only by taking a push, and taking one now reports the gap.
    assert_eq!(sub.try_recv().err(), Some(TryPushRecvError::Lagged));
    assert_eq!(
        router.send(push_for(&set("y"))),
        0,
        "nothing after the drop"
    );
    assert_eq!(sub.try_recv().err(), Some(TryPushRecvError::Lagged));
}

#[test]
fn a_huge_grant_list_is_placed_quickly_and_routed_broad() {
    // 6J-E F3: placement was O(exact × prefixes) and a 1M-pattern SYNC list
    // was indexed entry by entry under the router's write lock.
    let many: Vec<Grant> = (0..50_000)
        .flat_map(|i| [Grant::rw(&format!("e:{i}")), Grant::rw(&format!("p{i}:*"))])
        .collect();
    let started = std::time::Instant::now();
    let p = Placement::of(Routes::Grants(&many));
    assert!(p.broad, "over MAX_INDEXED_GRANTS grants routes as broad");
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
    // Just under the cap: indexed, and the binary-search reduction is exact.
    let mut under: Vec<Grant> = (0..MAX_INDEXED_GRANTS / 2)
        .map(|i| Grant::rw(&format!("p{i}:*")))
        .collect();
    under.extend((0..MAX_INDEXED_GRANTS / 2).map(|i| Grant::rw(&format!("p{}:{i}", i % 7))));
    let p = Placement::of(Routes::Grants(&under));
    assert!(!p.broad);
    assert!(
        p.exact.is_empty(),
        "every exact key is under a kept prefix: {:?}",
        p.exact
    );
    assert_eq!(p.prefixes.len(), MAX_INDEXED_GRANTS / 2);
}

#[test]
fn taking_a_push_reports_a_gap_that_opened_while_it_was_queued() {
    // The window `try_recv`'s first check cannot cover: the push was queued
    // before the gap, and the gap opened before it was taken. Releasing it
    // reads the gap in the same atomic step, so it is not delivered.
    let router = SyncRouter::new(1);
    let mut sub = router.subscribe(Routes::Everything);
    assert_eq!(router.send(push_for(&set("queued"))), 1);
    assert_eq!(router.send(push_for(&set("dropped"))), 0);
    let taken = sub.rx.try_recv().expect("the first push is queued");
    assert!(
        sub.mailbox.release(&taken),
        "the gap must be seen on release"
    );
    // And on a mailbox with no gap, release reports none and frees the room.
    let mut ok = router.subscribe(Routes::Everything);
    assert_eq!(
        router.send(push_for(&set("a"))),
        1,
        "only the mailbox without a gap"
    );
    let taken = ok.rx.try_recv().expect("queued");
    assert!(!ok.mailbox.release(&taken));
    assert_eq!(router.send(push_for(&set("b"))), 1);
}
