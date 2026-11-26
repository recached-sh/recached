# Review D, round 2: Phase 6O-I resync boundaries (PR 3 + PR 5, with PR 4's D-1 fix)

Branch `fix/v0.6o-i-resync-boundaries` at `055cfba`. It includes `fb08adf` (D-2), `cc58268` (D-1, through merge `1c5142d`) and `055cfba` (D-3, D-5). I read the code diff from `7db53dc` and tested a fresh copy at `/home/dennispaler/recached-6oi-bench/review-d/src`. The worktree was not touched.

## Verdict: **APPROVED**

D-1, D-2, D-3 and D-5 are resolved. D-4 is deferred to the release gate, as agreed. Nothing new in the D-1 convergence path harms offsets, resume points or sub-replicas.

I observed one new test flake (R2-1, Low). It is non-blocking and test-only.

## Round-1 findings

| ID | Status | Evidence |
|---|---|---|
| D-1 (Medium) | **Resolved** | See the D-1 section below. |
| D-2 (Low) | **Resolved** | See the D-2 section below. |
| D-3 (Low) | **Resolved** | The test was renamed `a_replica_whose_queue_overflows_on_groups_resumes_and_converges`. Its doc comment now states what it proves, and points to the deterministic mid-group test. |
| D-4 (Nit) | Deferred | Unchanged; noted for the release gate, as agreed. |
| D-5 (Nit) | **Resolved** | `a_real_primary_answers_a_resume_inside_a_group_with_a_full_sync` now asserts the exact `syncs()` delta (partial +1 or full +1) for every resume point. It polls, because the counter is bumped just after the mode byte is written. |

### D-1: what changed and why it is sound

In the failure branch of `apply_repl_group`, the fix calls three things while `_order` (the group's stripes) is still held:
- `reset_stream()`;
- `tx.invalidate_all()`;
- `notify_keyspace_replaced`.

It then returns `InvalidData`.

Probe PD3 was rerun with round-2 assertions:
- **Grouped:** C ends with `k1 = "v"`, the same as B. B's run id changed, B served 1 full sync to C, and C then followed B's post-promotion write.
- **Ungrouped control:** unchanged (C has `k1 = "v"`, no reset).

The new test `a_failed_group_on_a_middle_replica_resets_its_sub_replicas` is deterministic. `promote_to_primary` takes the all-write barrier, which waits for B's group stripes. So the reset has happened before the test's `assert_ne!(run_id)`.

Effect on offsets, resume points and sub-replicas:
- **B's own offsets.**
  - `reset_stream` keeps `offset`; it replaces the run id and empties the backlog under the stream lock.
  - The precondition "caller excludes attaches" holds: `handle_replica`'s all-write barrier waits for the stripes this path holds.
  - A concurrent handshake that advertised the old run id is caught by the run-id re-check under the barrier (`Interrupted`, renegotiate).
- **C's resume point.** C's link closes when its handle is dropped, so C keeps its old (run, offset) on `UnexpectedEof`. When it reconnects, its run id no longer matches B's, so it sends an empty run id and takes a full sync of B's actual keyspace, including the applied prefix. C never resumes partially past the prefix, including when B is promoted first: PD3, and the new test.
- **B's resume point.** The error is `InvalidData`, so `run_repl_client` clears it. B takes a full sync from A, which resets B's stream again. That second reset is harmless: C full-syncs once more, onto A's keyspace.
- **The same path in B's partial loop** (`sync_from_primary`'s partial sync) goes through the same function, under the same stripes, with the same result.
- **Cost.** When the first member fails, nothing was applied and the reset is redundant (B's coming full sync resets anyway). It costs no extra divergence. `notify_keyspace_replaced` takes only the watch registry's mutex, not write stripes, so there is no deadlock with the held `_order`.
- **The success path also changed.** Members now move into `on_group_logged` (AOF group, then `fan_out_group`), before notifications and pushes. The relay is still after the apply, with B's own offsets, as one group. `may_defer` was dropped: B's AOF always uses the non-deferred variants. This does not affect offsets. Notification semantics belong to review C.

### D-2: what changed and why it is sound

- `GroupAssembler`'s `next` is now `checked_add`, and fails with `protocol_error("replication offset overflow")`.
- A header whose `first + count - 1` overflows is refused. `count ≥ 2` is already enforced, so `count - 1` cannot underflow.
- `ReplGroup::last()` is therefore only reached on validated headers.
- The lone-frame case at `committed == u64::MAX` is also refused.

Probe PD2 now ends with `InvalidData: "replication offset overflow"` (no panic). The new test `offsets_past_u64_max_are_a_protocol_error` shows the following reconnect requests a full sync.

## New finding

| ID | Severity | Finding | Scenario | Required change |
|---|---|---|---|---|
| R2-1 | Low (test-only, non-blocking) | `replication_framing_6oi::malformed_groups_force_a_full_synchronization` failed once, at line 454, the `(run, offset) == ("", 0)` assertion. That was one run of the 6O-I suites plus my probes in one binary, with the machine at load average ~11. I could not reproduce it: it passed alone, in 12 runs of three modules, 15 runs of the original set, and 8 runs of a heavier `6oi\|6oh\|replication_dedup\|replica_6l` set. **Most likely cause (hypothesis, not confirmed):** `follow`, `follow_on_own_thread` and similar helpers run on threads with their own runtime, and loop every 20 ms for as long as their node is a replica. They outlive their test, and keep reconnecting with their saved resume point to the port their dead primary used. If the OS gives that port to a later test's `FakePrimary`, the `accept()` returns the stray replica, and the assertion sees a non-empty run id. The test then fails for a reason unrelated to the code under test. | Intermittent; run many replication test modules in one binary under load. | Optional: give the reconnect loops a stop condition tied to the test (a flag, or a `Weak` to the node), or have `FakePrimary::accept` skip a connection that requests an unexpected run id. |

Outside this review's scope: `benchmarks/v0.6.0/6o-i/tools/__pycache__/txn_compat.cpython-314.pyc` is committed. It came from the design PR (`4d21b31`), so it is already on dev.

## What I verified (round 2)

- **Code diff `7db53dc..055cfba`** for `replication.rs` and `server_state.rs`, plus the test diffs. I also read `notify_keyspace_replaced`, and checked the barrier and stripe argument for `reset_stream` from the failure path.
- **Test run on the refreshed copy** (one cargo command): `review_d_probes`, `resync_boundaries_6oi`, `replication_framing_6oi`, `replica_atomicity_6oi`, `write_order_tests`.
  - 36 of 37 passed in the first run. The one failure was R2-1, and it passed on rerun and in 35 more runs of the binary.
  - The expected contained panic at `store.rs:3227` belongs to `a_member_that_panics_is_contained_and_forces_a_full_sync`.
- **Probes rerun against the fixes:**
  - PD1 (mid-group resume on a real primary with a trimmed backlog): unchanged; full sync, safe recovery.
  - PD2: now `InvalidData`.
  - PD3: now converges, with a run-id reset and a full sync served.
- The updated probe source is `review_d_probes.rs` (round-1 version) in this folder. The round-2 edits change PD2 to expect `InvalidData`, and make PD3 assert that C converges, that B's run id changed, and that B served a full sync.

## What I could not verify

- **The cause of R2-1.** It did not reproduce.
- **Release builds, TLS, real network faults, multi-process restarts.** Same limits as round 1.
- **The notification and watch semantics of the new success path** (`notify_replicated_group`, the `removes` computed inside the hold). Review C covers these.
