# Review D: Phase 6O-I replication offsets and resynchronization (PR 3 + PR 5)

Reviewer: independent (did not write the code). Branch `fix/v0.6o-i-resync-boundaries`, tip `7db53dc`. PR 3 is `6068337..feat/v0.6o-i-replication-framing`, PR 5 is `7db53dc`, and PR 4 (`6c4b89f`) was read as context.
Design: `V0.6O-I-PROTOCOL-DESIGN.md` rev 2.1, §6–§7. 6O-H §8 and the compatibility notes were also used.

## Verdict: **APPROVED WITH CHANGES**

The resynchronization protocol is correct. I found no way for a `txn1` replica to apply part of a group as if it were whole, and no Critical or High findings.

- Rule 2 is complete because the backlog is contiguous.
- The full-sync boundary and a replica's registration can never fall inside a group: `publish` takes the offset with one `fetch_add(count)` under the stream lock.
- The replica's assembler only commits and acknowledges whole groups.

One Medium finding (D-1) concerns the chain property "B's stream describes B's keyspace" after a failed group on B. It should be fixed, or explicitly accepted by the maintainer, before merge. D-2 is a Low robustness fix.

## Findings

| ID | Severity | Finding | Concrete scenario (reproduced) | Required change |
|---|---|---|---|---|
| D-1 | **Medium** | When a member of a group fails or panics on a middle replica B, the members before it are applied to B's store but never relayed. B also does not reset its own stream. Its sub-replicas then follow, and can partially resume on, a stream that no longer describes B's keyspace. Before 6O-I, B applied and relayed members one by one, so B and C stayed consistent with each other. This is a regression in the A→B→C invariant. The primary's equivalent path (`transaction.rs` `converge`) calls `reset_stream`; `apply_repl_group`'s failure path does not. Design §6.4 relies on B's later full sync to reset C, which never comes if B is promoted first, or is delayed while A is unreachable (backoff up to 30 s). | Probe **PD3** (`review_d_probes.rs`), A→B→C, all `txn1`, run with `run_repl_client`:<br>1. B (and C, through B's relay) diverge from A: `SET pd3:list str` on B.<br>2. On A, `EXEC {SET pd3:k1 v; LPUSH pd3:list x}`.<br>3. On B, member 1 applies and member 2 fails with WRONGTYPE, so B has `k1=v`.<br>4. B is promoted inside its 2 s backoff.<br>5. `SET pd3:after 1` on B reaches C.<br>Result: **C `k1` = nil, B `k1` = "v". B's run id is unchanged and B served 0 full syncs.** The divergence is permanent. Control, same scenario with `txn1` off: C `k1` = "v" (consistent). | In `apply_repl_group`'s failure branch, call `state.replicas.reset_stream()` before returning the protocol error, while `_order` is still held. That satisfies `reset_stream`'s "caller excludes attaches" precondition, as `converge` does. It is harmless when member 1 failed, because B's full sync resets anyway. Add PD3 as a regression test: assert B's run id changed and C full-synced. This overlaps review C (PR 4 code); it is reported here because it breaks item 5. |
| D-2 | Low | An offset overflow in the replica's group path panics the replica's replication task. `overflow-checks = true` in release, and `run_repl_client` is a bare `tokio::spawn`, so the replica stops following silently while still serving reads. `ReplGroup::last()` computes `first + count - 1` unchecked. `GroupAssembler`'s `next` uses `saturating_add`, so at `u64::MAX` it accepts the same offset repeatedly. The partial loop's `checked_add(1)` guard covers neither groups nor the streaming loop. | Probe **PD2**:<br>1. A fake authenticated primary (no TLS, or a corrupt one) sends a full sync with boundary `u64::MAX-1`.<br>2. It sends a header `{first: u64::MAX, count: 2}` and two members, both at offset `u64::MAX`; the saturating `next` accepts both.<br>Result: `replication.rs:30` panics with "attempt to add with overflow". The task's `JoinError::Panic` was observed. Reaching it requires a malicious or corrupt primary. | Treat offset overflow as a protocol error (`InvalidData`):<br>- In `parse_group_header`/`receive`, reject a header whose `first.checked_add(count - 1)` is `None`.<br>- Make `next` checked, not saturating.<br>- Optionally refuse a full-sync boundary of `u64::MAX`. |
| D-3 | Low | `a_slow_replica_cut_off_inside_a_group_resumes_before_it_and_converges` does not exercise what its name claims. I instrumented the replica's streaming loop to log when a link ends with a group open. In 4 of 4 runs, no link ended with a group open: the queue-full disconnects happened before any member of the group was flushed. The test passes on whole-group or between-group cuts. Mid-group EOF is still covered deterministically by `a_link_that_ends_inside_a_group_applies_none_of_it_and_resumes_before_it` (the instrumentation fired there), so this is a test-quality issue, not a coverage hole. | Run the test 4× with the `REVIEW-D:` instrumentation and observe 0 open-group endings. | Make it deterministic, or rename it to what it proves. For example, on the primary side, hold the sender after it flushes the header and the first member, then overflow the queue. |
| D-4 | Nit | An acknowledgement for a committed lone frame is withheld while the following group's header or members are already buffered (`rd.is_drained()` is false). The ack waits for the whole group, so lag is over-reported by up to one group and its transfer time. Correctness is unaffected: only committed offsets are ever acknowledged. | A solo write immediately followed by a large EXEC: `acked` stays at the offset before the solo write until the EXEC's last frame arrives. | Optional: acknowledge the committed offset when the assembler is open and the reader would block. |
| D-5 | Nit | No test asserts that rule 2 caused the full sync. The real-primary test checks only the mode byte; neither `recached_replication_midgroup_resumes_total` nor a `syncs()` delta is checked. PD1 shows the `syncs()` delta is easy to assert. | — | Assert `syncs()` (partial +0, full +1) in `a_real_primary_answers_a_resume_inside_a_group_with_a_full_sync`. |

## Required example: a mid-group resume (item 7)

Probe **PD1** runs §7.3's worked example on a real primary (`handle_replica` over TCP), with the real replica code (`sync_from_primary`) starting from a crafted resume point. In this run, G = offsets 2–4 (design: 101–103).

1. Primary: `SET rd:pre`, then `EXEC {SET rd:a; SET rd:b; SET rd:c}` (G = 2..=4, `count`=3, `bytes`=90), then `SET rd:z`.
2. The backlog is shrunk so the next write trims it to begin inside G. After `SET rd:y` it holds `[(3, G), (4, G), (5, -), (6, -)]`: G's tail alone.
3. By hand, a capable replica (`accept=…,txn1`) asks to resume at **2** (inside G). The primary answers **'D' (full sync)**: rule 2.
4. By hand, a legacy replica (`accept=dedup1,expirylog1`) asks at **2**. The primary answers **'P'**, boundary 6, **count 4** (data frames 3, 4, 5, 6), first frame offset **3** with no header. This is rule 3, as designed: a legacy replica never had group atomicity.
5. By hand, a capable replica asks at **1** (G's boundary). Frame 2 is trimmed, so the primary answers a full sync.
6. **Real replica, resume point inside G.** The replica's keyspace holds `rd:pre` and `rd:a` only: it applied offset 2 alone, as a buggy replica would, or one that applied the stream before it negotiated `txn1` and kept the wrong mode. It is given `ReplResume{run, offset 2, expirylog, txn}`.
   - Result: the primary served **partial +0, full +1**.
   - The replica ends with `rd:a`, `rd:b`, `rd:c`, `rd:z`, `rd:y` all = "1" and resume offset 6.
   - **It recovered safely; no fragment was applied.**
7. Control: a real replica at **4** (G's end) receives **partial +1, full +0** and only the frames after G.

### Attempts to get a mid-group resume past rule 2

None succeeded.

| Attempt | Why it is rejected or impossible |
|---|---|
| Backlog trims leave G's tail at the front | `requested+1 < front` gives a full sync when `first` is gone. A request inside G is caught by rule 2 (`group.first < next.offset` ⇔ `first ≤ r`). Shown by PD1 and the hub unit test. |
| Missing list starts at a group's first frame, but the group continues past `current` / the `count` | On the primary, `missing` is cloned under the stream lock, and `publish` backlogs a whole group in one hold, so the backlog's tail is always whole groups. `current` is read under the all-write barrier, which excludes `EXEC` phase 2 and a replica's group apply. On the replica, the partial loop ends with `groups.is_open()`, which is a protocol error and leads to a full sync. |
| `requested == current` early return | `offset` advances by `count` in one `fetch_add` inside the lock, so `current` is never inside a group. |
| Lock-free offset readers (`resume_frames`'s `current`, `INFO`, `handle_replica`'s boundary) | They see either the value before the group or after it, never inside. If a publish ever slipped between the boundary read and registration (it cannot under the barrier), the replica would see an offset gap, take `InvalidData`, and full-sync. |
| Concurrent `fan_out` / `fan_out_group` | Both go through `publish` under one `std::sync::Mutex`, so their offsets are contiguous. Registration (`senders.push`) takes the same lock, so a sender is never registered mid-group. The test `a_groups_offsets_are_consecutive_among_concurrent_writers` passes. |
| `reset_stream` / run-id rotation | The run id is replaced while the stream lock is held and the backlog is cleared, so an old run id gives a full sync. `handle_replica` re-checks `run_id` under the barrier. Both callers (`converge`, the replica's full-sync install) hold stripes or the barrier, so they cannot race `resume_frames`. The run id is regenerated on every start and never persisted. |
| Queue-full disconnect in the middle of a group | `retain`'s `all()` stops at the first frame that does not fit and drops the handle. The sender may already have flushed the header and some members; the replica buffers them, sees EOF (`UnexpectedEof`, so the resume point is kept), and resumes at `first-1`. Covered deterministically by the fake-primary test. |
| Read-ahead during a full sync | It starts at the registration point, which is a boundary. If the link fails, the read-ahead is dropped and the resume point is the boundary recorded by the install. A fresh `GroupAssembler` parses the read-ahead. |
| A replica that applied frames before negotiating `txn1` | `ReplResume.txn` must equal the advertised `txn1`, or the replica sends an empty run id and gets a full sync. In production every 6O-I primary advertises and offers `txn1` unconditionally. The resume point is in memory only, so a restart gives a full sync. |
| Promotion while assembling | The members are dropped. `still_following` fails inside the stripes that promotion's barrier excludes. |

## What I verified

- **Code reading.**
  - `replication.rs`: `publish`/`fan_out`/`fan_out_group`, `resume_frames`, `handle_replica` (handshake, barrier, mode/boundary/count, sender loop, acks), `write_repl_frame_to`/`group_header`, `run_repl_client`, `ReplResume`, `sync_from_primary` (mode memory, partial loop, streaming loop), `GroupAssembler`, `parse_group_header`, `apply_repl_group`, `ChunkReader`, `reading_ahead`, `Registered`.
  - Also `server_state.rs` (`on_transaction_logged`, `promote_to_primary`, `writes_expiry_log`) and `transaction.rs` (phase 2, `converge`).
- **Item 1 (partial resync).**
  - `count` is `missing.len()`, data frames only; headers are written in addition, only on a `txn1` link, only at `group.first`.
  - Rule 2 holds, and legacy behavior is unchanged (PD1).
  - The replica refuses a partial sync that ends with a group open, and checks `resume.offset == boundary`.
- **Item 2 (backlog trimming).** No fragment is applied as whole: PD1 and the unit test.
- **Item 3 (acknowledgements).**
  - Acks are sent only after `Received::Frame` or `Received::Group`. Headers and members `continue`.
  - `resume.offset` advances only after a whole group applies.
  - The boundary ack is sent after a partial sync only when `resume.offset == boundary`.
  - `acked` uses `fetch_max`. `max_lag_frames` reads `sent` under the stream lock, so it never sees a group's middle.
- **Item 4 (mixed capabilities).**
  - `expirylog1`×`txn1`: the 4 combinations converge.
  - Old replica of a new primary: no headers; legacy partial resume anywhere.
  - New replica of an old primary: a header is treated as a gap, and `resume.txn=false`.
  - Mode memory: a mismatch sends an empty run id.
- **Item 5 (chains).**
  - B relays after applying, with its own offsets.
  - B's full sync calls `reset_stream` under the barrier.
  - Promotion keeps the stream.
  - A restart loses the in-memory resume point.
  - Exception: D-1.
- **Item 6 (full-sync fallback).**
  - Every assembler and group-apply refusal is `InvalidData`, and `run_repl_client` clears `resume` on `InvalidData`.
  - EOF and connection errors keep it.
  - A failed checkpoint clears it explicitly.
  - Malformed and oversize groups are covered by tests.
- **Tests run** on a copy (`/home/dennispaler/recached-6oi-bench/review-d/src`, debug build):
  - `replication_framing_6oi` (8), `resync_boundaries_6oi` (6), `write_order_tests::{a_capable_replica_never_resumes_inside_a_group, backlog_only_resumes…}`: all pass.
  - The broader filter `replic|repl_|transaction_recovery_6og`: 149 passed, 1 ignored.
  - Probes PD1–PD3 pass as written; PD3 only prints the divergence it demonstrates.
  - The probe source is saved next to this file (`review_d_probes.rs`). The copy also adds `#[cfg(test)]` accessors (`ReplResume::review_d`, `review_d_parts`, `ReplHub::review_d_backlog`) and a temporary `eprintln!` instrumentation for D-3.

## What I could not verify

- **Real TLS links, real network faults, multi-process restarts.** All runs were in-process over loopback TCP.
- **Release-build behavior.** The overflow in D-2 was shown in a debug build; release has `overflow-checks = true` per `Cargo.toml`, so it should behave the same.
- **A deterministic queue-full cut with a group open on a real primary.** It is not reproducible without a sender hook; the replica-side behavior is covered by the fake-primary test.
- **Oversize groups (>64 MiB) end to end on a primary with raised queue and backlog.** Only the replica's header refusal is tested; the possibility of a full-sync livelock under repeated huge `EXEC`s is reasoned, not run (design D1).
- **Whether every non-transaction `fan_out` caller is excluded by the all-write barrier.** This is pre-6O-I (6O-G/6O-H scope). Grouping does not depend on it, because registration is atomic with `publish`.
- **The barrier and `UnorderedGate` internals and `apply_transaction`'s marking.** These were taken as established by earlier reviews (6O-E/6O-G, and review C for PR 4).
