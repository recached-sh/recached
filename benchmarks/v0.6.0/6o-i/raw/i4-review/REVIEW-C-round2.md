# Review C, round 2: Phase 6O-I PR 4 fixes (`cc58268`)

Reviewed `git diff 8127709 HEAD -- server-native`, which is PR 4's own fix commit `cc58268` on top of the PR 2 and PR 3 merges. The merged PR 2 and PR 3 parts are out of scope.

## Verdict: **APPROVED**

H1 and M1 are resolved, and each new test is shown to catch its bug by reverting only that fix in a copy. The convergence on the failure path, which runs under the group's order stripes, cannot deadlock. The fix introduces nothing above Low. Two Nits and one Low test-harness weakness remain; none blocks merge.

## Round-1 findings

| ID | Status | Evidence |
|---|---|---|
| H1 (notifications built from the state at the end of the group) | **Resolved** | `notify_replicated_group` (`transaction.rs`) builds `AppliedWrite`s and reuses `NotificationPlan` and `plan.changes`/`plan.flushes`/`notify_flushdb_after`, exactly as `run_transaction_queue` does. `removes` is computed inside the `apply_transaction` closure, right after each member runs, within the hold (`stored_nothing` reads only the member's destination, which is in the held set, so the read never waits). `removes` is pushed only for members that succeeded; on the success path every member succeeded, so `zip(commands, removes)` covers them all. `watched_after_write()` is called once, after all writes, as `EXEC` does. **Shown to catch H1:** with the old per-member `notify_write` restored in a copy (env toggle), `a_delta_live_query_on_a_replica_converges_after_a_group` and `a_group_that_sets_then_removes_a_key_invalidates_a_replica_watch` both FAIL. At HEAD both pass. |
| M1 (applied prefix of a failed group not converged downstream) | **Resolved** | The failure path now runs `reset_stream()`, `tx.invalidate_all()` and `notify_keyspace_replaced()` with `_order` held, before it returns `InvalidData`. This is the same set the full-sync install runs under its barrier, so promotion before the reconnect can no longer leave a sub-replica resuming partially past the prefix. **Shown to catch M1:** with the three calls disabled in a copy, `a_failed_group_on_a_middle_replica_resets_its_sub_replicas` fails 3/3 (`B reset its stream`: run id unchanged). At HEAD it passes 10/10. |
| L1 (test gaps) | Addressed / accepted | The docstring is corrected. The AOF-latch-on-failure test asserts that the prefix stays (`fa:a`), that the rest is skipped, and that persistence is latched. A borrowed TCP read is not added (no TCP listener in the test `Node`), which is accepted. `txn1` without `expirylog1` is covered by PR 3's `every_combination_of_expirylog1_and_txn1_converges`, which is accepted. |
| L2 (extra group copy) | Resolved | `ServerState::on_group_logged(Vec<Vec<u8>>)` lends the frames to the AOF, then moves them into `fan_out_group`. The branches are the same as `on_transaction_logged(.., may_defer = false)`; the deferred-client branch it drops was unreachable with `false`. See N3 for a new copy on the push side. |
| L3 (cancellation) | Resolved (comment) | — |
| N1, N2 | Unchanged, as agreed | — |

## Convergence under the order stripes: deadlock and misbehaviour check

- **What runs:** `reset_stream()` takes the stream `Mutex`, then the `run_id` `RwLock`, both synchronous and never held across an `.await`. `invalidate_all()` takes a synchronous read lock on the router table. `notify_keyspace_replaced()` takes the watch-registry `tokio::Mutex` and calls `fan_out`.
- **Lock order:** none of these locks is held by anyone while that holder waits for an order stripe or the unordered gate. The order "order stripes, then registry" is the one already used by every replicated single frame (`notify_write` under `lock_commands`), by `EXEC` phase 2, and by the full-sync install (all stripes, then `reset_stream`, `invalidate_all`, `notify_keyspace_replaced`). So there is no new edge in the lock graph.
- **Store stripes:** by the time these calls run, `apply_transaction` has returned, which means its hold has been dropped or `repair_after_panic` has released everything. `key_is_live` reads in `notify_keyspace_replaced` therefore take no store stripe owned by this thread.
- **Promotion, attach, checkpoint and QSUB:** a promotion, a sub-replica attach, a checkpoint or a QSUB snapshot waiting on the barrier proceeds after the group's error return. The attach then sees the new run id and takes a full sync. The promotion keeps the reset stream. Both are correct.
- **Where notifications go:** `notify_keyspace_replaced` announces only keys under WATCH. Live queries (QSUB) are WebSocket-only (`handle_ws`) and are covered by `invalidate_all`, which reaches every subscriber in the router table. That is the same coverage the full-sync install relies on. The primary's `converge` announces the failed command's keys instead; the replica's whole-keyspace announcement is a superset for WATCH.
- **Repeated reset:** the later full sync from the primary calls `reset_stream` again, which only rotates the run id once more and is harmless.

## New findings

| ID | Severity | Finding | Required change |
|---|---|---|---|
| R2-L1 | Low | **The convergence check in `a_failed_group_on_a_middle_replica_resets_its_sub_replicas` can pass without the fix.** The test harness `follow_on_own_thread` keeps reconnecting after `promote_to_primary` (unlike `run_repl_client`, it never checks `is_replica`). A promoted B therefore still takes a full sync from A, which resets B's stream and makes C resync. That alone can satisfy the final `wait_for(c, "fg:k1")`. The test catches M1 only through `assert_ne!(run_id, b_run)`, which relies on the promotion happening inside the harness's 20 ms reconnect delay. In 3/3 runs with the fix removed, the run id was unchanged and the test failed. | Stop the harness loop once `!state.is_replica()`, as `run_repl_client` does, so the last assertion proves C converged from the promoted B. Optional. |
| R2-N3 | Nit | When any WebSocket client is routed, `pushes` clones **every** member (`members.iter().cloned()`) before `on_group_logged` takes them, and `push_replicated` then copies each routed member again (`bytes.to_vec()`). That is up to two extra copies of a large group, where before the fix there was one for each routed member. | Optionally keep only the members for which `tx.would_route(cmd)` holds (or pass owned bytes into `push_replicated`). Correctness is unaffected: the push decision is still made after the write, so a client routed later reads the store. |
| R2-N4 | Nit | A group's WATCH/QSUB notifications are now all sent before all of its sync pushes. On the primary, `EXEC` phase 2 interleaves them per write. The two go through separate queues to a connection, so no client-visible ordering contract changes. | None. |

## What I ran

All runs were in a refreshed copy of HEAD `cc58268` (`/home/dennispaler/recached-6oi-bench/review-c/src`); no worktree was touched.

- **Atomicity tests:** `tests::replica_atomicity_6oi` passes, 11/11, and stays green in 10 consecutive runs.
- **Full test binary:** `recached-server` passes, with 846 passed, 0 failed and 11 ignored.
- **Reverting each fix:** in the copy, env toggles brought back the old per-member `notify_write`, or skipped the failure-path convergence. In each case the corresponding new tests fail, as shown in the round-1 table above.
- **Clippy:** `cargo clippy --locked -p recached --all-targets -- -D warnings` is clean on the copy. The copy differs from HEAD only by the test toggles.

## Not verified

- A borrowed TCP read on a replica while a group applies. I rely on 6O-G's coverage of the same store code.
- Performance (PR 6).
