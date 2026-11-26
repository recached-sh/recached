# Review A, round 2: `V0.6O-I-PROTOCOL-DESIGN.md` revision 2

Reviewer: independent (did not write the design). Checked against:
- the spec at revision 2 (§13.1);
- the AOF prototype `recached-6oi-aof`, `b1e65a0`, 19/19 `transaction_framing_6oi` plus the gate test passing;
- the replication prototype `recached-6oi-repl`, `426ce68`, 7/7 `replication_framing_6oi` passing;
- the AOF prototype's debug binary, which I rebuilt in its own target directory, run as a process on crafted files.

Probes: `probe3.py` to `probe5.py` and their `*-out.txt` files in this directory. No worktree file was modified. Only PIDs I started were killed.

## Verdict: **APPROVED WITH CHANGES**

Every round-1 finding is resolved in the spec, and wherever the prototypes cover it, in code (table below). Revision 2 introduces one new Medium: what happens when the `.cut-` copy cannot be written is unspecified, and the prototype then refuses to start where 6O-H starts. That needs a spec decision. The remaining findings are Low or Nit. There is no Critical or High finding.

## Round-1 findings: resolution check

| # | Resolved? | Evidence |
|---|---|---|
| M1 | **Yes** | §3.1 rows 2–3. Groups are recognized only up to the timeless end, and `TXN-*` records after it are no-ops. Code: classifier guard `grouped_head && (group.is_some() \|\| timeless_end == MAX)`; ignorable kinds and non-head `FORMAT` are skipped inside a group; an unknown non-ignorable kind abandons the group, then ends the timeless part. Probe Q4: ignorable kind `0x83` between members → starts, `a=new b=new`. |
| M2 | **Yes** | Row 1 (authorship) is checked first. A push marker does not end the timeless part. The replay-start logic runs after classification. Probe Q3: a 0.5/6O-H-style `>6 DEDUP c 7 SET k v` after a dangling group abandons it, and the node starts with `a=new b=old k=v`. Probe Q5/probe4: after a group abandoned only by a push marker, the file stays `Timeless{txn}`. The next `EXEC` is grouped. Cutting that new group mid-member discards it while keeping the abandoned member, and the outcome is the same on every restart. |
| M3 | **Yes**, with new finding R2-M1 | Nested records are refused in every file (`is_flat_record`). Probe Q2: the P1 file is refused with "a record holding another record". Every tail cut is copied, synced, to `.cut-<offset>-<ms>` first. §3.2, §3.6 and §5.3 now state what is not detected. Probe P5 (`probe5.py`): AOFs written by v0.4.0, v0.5.2 and 6O-G with 22 command types still replay on 6O-I, so D4 broke none of them. |
| M4 | **Yes** | 64 MiB charged and 524,288 frames, both checked from the header before buffering (`GroupAssembler::receive`). Capacity is not taken from the header's claim (`min(count, 1024)`). Per-member check `received ≤ bytes`, and equality at the last member. §8 is recomputed (about 450 MiB). D1 is revised with the primary-side limits. |
| L1 | Yes | §2.4 rule 1 is normative. Test `a_replica_restarted_on_an_incomplete_tail_cuts_it_before_appending`. |
| L2 | Yes | §3.2 and D3 now say "6O-H's re-upgrade limit". |
| L3 | Deferred, acceptable | Recorded as a release-gate follow-up; it is pre-existing. |
| L4 | Yes | `publish` does `fetch_add(count)` once inside the stream lock. The partial loop fails if `groups.is_open()` at its end. |
| L5 | Yes | `ReplResume::txn` is part of the resume-mode match. |
| L6 | Yes | `push_transaction` checks `is_one_record` on every member in release builds and `u32::try_from`. On failure it writes ungrouped and logs. |
| L7 | Yes (spec) | §6.4: stop at the first error; latch only with an AOF. The code is PR 4, not reviewed. |
| L8 | Yes | §5.3 and D2. |
| N1–N5 | Yes | `format_is_grouped` requires `expirylog` and `txn` and no unknown must-understand bit. `whole_frames_len` returns `keep`, so the `torn == 0` early return no longer keeps an open group (gate test `a_cut_never_keeps_part_of_a_transaction_group`). §8 documents the uncounted extents. §6.2 says a later header is `txn2`. §5.3 covers the autosave-off case. |

## New findings

| ID | Sev | Finding | Concrete scenario | Required change |
|---|---|---|---|---|
| R2-M1 | Medium | §3.2/D5 say every tail cut is first copied to `<aof>.cut-…`, but not what happens when the copy **cannot be written**. The prototype propagates the error (`write_private(...)?`, `sync_parent_dir(...)?`), so the node **refuses to start**. This hits the most common recovery path, a torn tail after any crash, and plain torn records too, not only groups. 6O-H shrinks the file in place, which needs no free space and no directory write. Without a copy, a node that crashed on a full disk (ENOSPC, EDQUOT) or whose AOF directory is not writable now stays down, reads included. A mid-file length corruption (P7 class) needs free space as large as the rest of the file. | Probe Q1: AOF directory mode 0555, file writable. A torn plain record: **6O-H starts** (151→142 bytes); **6O-I refuses**, `PermissionDenied: failed to replay AOF …`. An incomplete group: **6O-H starts**; **6O-I refuses**. | Specify the failure behaviour and add it to D5. Either refuse with an actionable message (free space, or set an explicit override such as `RECACHED_AOF_CUT_WITHOUT_COPY=on`), or proceed with an error-level log when the cut is small (e.g. within one batch or one record) and refuse only above a threshold. Add a test for whichever is chosen. Document the extra free space a large cut needs. |
| R2-L1 | Low | §11 says PR 3 applies a group "in order, under the union of their replication stripes" / "under one stripe hold". The PR 3 code does not: `apply_repl_group` calls `apply_repl_frame` per member, so each member takes its own stripes and runs its own `still_following` check. The replica's own AOF and sub-replicas also get the members one by one, ungrouped, in a `txn` file. | A promotion between members k and k+1 leaves the promoted node holding part of a transaction permanently. It never resyncs, the same as today's per-frame apply. A replica restarted as a primary replays its own ungrouped members, prefix included. | Fix §11's text to "today's per-frame visibility and effects until PR 4", and do not release PR 3 without PR 4. PR 4: one `lock_commands` over the union, one `still_following`, then `apply_transaction`, then AOF via `append_transaction` and relay via `fan_out_group`, as §6.4 says. |
| R2-L2 | Low | `.cut-` files have no lifecycle. One is written at every startup that finds a torn tail (crash loops included). The files are kept indefinitely, hold user data (0600, which is correct), and are invisible to monitoring. A key deleted later (e.g. on erasure request) survives in them. | A crash-looping container under `everysec` accumulates one file per restart. | Add a metric and log line (count, total bytes), and document retention and cleanup in operations docs. Optionally, prune copies older than N or above M bytes. |
| R2-L3 | Low | D4 (nested record → refuse) applies to **every** file, legacy ones included. The evidence is the T4 files plus my 22-command sample (P5). It does not cover WebSocket DEDUP envelopes from 0.5/0.6, ephemeral writes or every write command. | A legacy file from a release that ever wrote a nested element would stop 6O-I from starting where 6O-H starts. No such writer is known; `dedup_wrap` flattens. | PR 6: a compat corpus that generates AOFs with every write command, through TCP and WebSocket (DEDUP), on each older binary, and replays them on 6O-I. |
| R2-N1 | Nit | §8's 450 MiB worst case excludes the pre-existing single-frame read. `read_repl_frame` allocates up to `MAX_REPL_FRAME_BYTES` (512 MiB) before the assembler can compare the member with the header's remaining `bytes`. | A member claiming 512 MiB inside a group whose header declares 1 KiB is allocated, then refused. | Say so in §8, or check a member's `len` against the open group's remaining bytes before allocating. |
| R2-N2 | Nit | §8 assumes a parsed `Command`'s fixed part is ≤ 128 B. | — | PR 4: add a `size_of::<Command>()` assertion or test. |
| R2-N3 | Nit | `is_one_record` now runs on every member of every grouped `EXEC` in release builds (non-allocating, O(bytes)). | — | Include it in PR 6's performance comparison. |
| R2-N4 | Nit | A later writer's file in which an unknown non-ignorable kind ends the timeless part, followed by that writer's incomplete group: 6O-I treats the `TXN-*` records as no-ops and replays the prefix. This is consistent with 6O-H's downgrade semantics, but unstated. | — | One line in §5.2's "later server's file" row. |
| R2-N5 | Nit | `is_flat_record` catches the merge P1 produces. Other alignments of a torn record and an appended push mostly fail to parse (refused), and a rare alignment can still parse flat. | — | None beyond §3.6's existing "not detected" row. |

## Verified (new in revision 2 and the prototypes)

**AOF (PR 2 prototype)**
- `push_transaction` is used by every `EXEC` path: `append_transaction`, `append_coalesced_transaction` and `append_client_transaction` (deferred included). One `queue_only`/`enqueue` edit means one queue-lock hold.
- `grouping` is derived from `author && queue.grouped` on each append.
- `grouped` is reset on truncation, on the first legacy append, on `leave_expiry_log` and on a failed `truncate_then`. It is set by `control()` and `truncate_then` from `FORMAT_FEATURES`.
- Classifier rows match §3.1, including order. `discard_from` bounds both pass 2 (`replay_end`) and `complete`. The cut copy is written and the parent directory synced before `set_len`.
- `replayed.grouped = grouped_head && mode == Timeless`.
- `start_expiry_log` sets the format before any append.

**Replication (PR 3 prototype)**
- `publish` has one stream-lock hold and one `fetch_add(count)`.
- Group extents are set on every frame. A replica that fails reservation mid-group is dropped.
- Headers are sent only when `with_txn` (offered and accepted), on both the partial and the live paths, before `frame.offset == group.first`.
- `GroupAssembler`: nested header, unknown version/kind/reserved, `count < 2`, wrong `first`, count and charge bounds, offset gaps, too long or too short, and a header on a non-`txn1` link (a gap) are all protocol errors.
- `ReplResume::txn` is set on full and partial syncs.
- Acks are sent only after a frame or a whole group is applied.

**Rule 2:** absent from the PR 3 code, as stated (PR 5); `resume_frames` is unchanged. A correct capable replica's resume point is always a boundary, since the assembler commits only whole groups. So the gap is defence-in-depth only until PR 5.

## Not verified

- PR 4 (atomic replica apply, failure latch, effects) and PR 5 (rule 2): neither is written yet.
- Real disk-full behaviour (probe Q1 used a read-only directory to make the copy fail; ENOSPC follows the same error path in code).
- `size_of::<Command>()`.
- Power-loss ordering.
