# Review B, round 2: Phase 6O-I PR 2

| | |
|---|---|
| Diff reviewed | `c6bc48e..3c45311` (`server-native`, design doc). The merge `b70c39b` (origin/dev, #195) adds only the 27 evidence and review files under `benchmarks/v0.6.0/6o-i/`. It touches no code |
| Method | Read the diff. Exported `3c45311` to `/home/dennispaler/recached-6oi-bench/review-b/src2` and added my round-1 probes. Ran the full suite and the coalesced subset there, one cargo command at a time. The PR worktree is untouched (`git status` clean at 3c45311) |

## Verdict: **APPROVED**

All seven round-1 findings are resolved. Nothing blocking remains. Two non-blocking nits are listed at the end.

## Round-1 findings

| Id | Round 1 | Fix | Status |
|---|---|---|---|
| F1 | Medium | The T4 test now panics if the folder is missing. The six `.resp` files are in the tree, through the #195 merge. The test runs and replays all six (`replayed == 6`) | **Closed** |
| F2 | Low | §3.6's row now describes what the code does: the group is abandoned, the stray `TXN-COMMIT` is a no-op after the timeless end, the file starts with every member applied, and the rest replays as legacy. The test `a_member_flipped_to_a_push_abandons_its_group_and_loses_nothing` pins this, and my probe P7 gives the same result | **Closed** |
| F3 | Low | Pass 1 now records `last_nested` and refuses the start only if it is `>= replay_start`, checked after the loop, before anything is applied or cut. Checking only the last nested record is correct: `replay_start` is final once the loop ends, so every nested record lies before it exactly when the last one does. A refused file is still left untouched. Probe P6 now returns `Ok`, and the new test covers both sides. Design §3.1 is updated. The §5.3 v0.4.0 exception still holds, because such a record comes after the replay start | **Closed** |
| F4 | Low | `group_markers()` does the one-record check and builds the two markers before the queue lock, in all three paths (inline `append_batch`, coalesced, deferred). `push_batch` writes the group only when `Some(markers)` and `batch.grouping`, which is still decided under the lock. So the only thing that moved out of the lock is the part that never depended on the queue. Side effect: a legacy or 6O-H file now also pays for the check and the markers on every multi-record `EXEC`. That is negligible, and the error log there cannot fire for `broadcast_for_at` frames | **Closed** |
| F5 | Nit | §3.1 row 8 now says a torn record of either encoding is an incomplete tail | **Closed** |
| F6 | Nit | The rename and the directory sync are now separate steps, and a rename that succeeded is never undone. A failed directory sync gets its own warning ("exists but its directory could not be synced"). The leftover `*.partial.*.tmp` is documented in a code comment | **Closed** (residual nit N1) |
| F7 | Nit | A comment in `truncate_then`'s error branch says why no group can be in the window: the caller holds the all-write barrier | **Closed** |

## Re-verification

- Full `recached-server` suite at 3c45311 plus my probes: **834 passed, 0 failed, 11 ignored**. The panics in `--nocapture` output come from tests that expect them (exec_isolation, transaction_recovery_6og); all of those tests pass.
- With `RECACHED_AOF_FANOUT=coalesced`: framing, 6O-H format, transaction and probe tests, **88 passed**.
- My round-1 crash and fault probes still pass on the new code:
  - P2: a concurrent workload cut at every byte.
  - P3: a batch of several groups torn at every byte.
  - P4: a cut that cannot be made inside a group.
  - P1: older binaries' files.
- The torn-write analysis from round 1 still holds. The fix moved only queue-independent work out of the lock; the decision to group and the push of `TXN-BEGIN`, members and `TXN-COMMIT` still happen in one lock hold.

## Remaining nits (non-blocking)

| Id | Severity | Finding | Suggestion |
|---|---|---|---|
| N1 | Nit | When the rename succeeds but the directory sync fails, the new warning is followed by the old error, "could not be saved … cutting them without a copy", and the metric counts `kept=false`. The copy does exist, so the second line contradicts the first | Choose the error text by case, or add a third metric value such as `kept="unsynced"`. This can go in a later PR |
| N2 | Nit (outside this PR) | #195 committed `benchmarks/v0.6.0/6o-i/tools/__pycache__/txn_compat.cpython-314.pyc` to `dev`, and this branch gets it through the merge | Remove it and add `__pycache__/` to `.gitignore` in a later cleanup |
