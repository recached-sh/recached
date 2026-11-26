# Review A, round 3: `V0.6O-I-PROTOCOL-DESIGN.md` revision 2.1

Reviewer: independent. Checked against:
- the spec at revision 2.1 (§3.2, §5.2, §8, §11, D5, §13.2);
- the AOF prototype `recached-6oi-aof`, `c3b8295`, rebuilt in its own target directory;
- the PR 4 working tree `recached-6oi-atomic` (on merges of PR 2 and PR 3), read for contradictions with §6.4 only.

No worktree file was modified. Only my own PIDs were killed. Probe: `probe3.py`, Q1 rerun (`probe3-round3-out.txt`).

## Verdict: **APPROVED**

Every round-2 finding is resolved, in the spec and, where it applies, in code. Nothing new rises above Low. One Low and one Nit below can be handled in PR 2's code (no design change needed).

## Round-2 findings: resolution check

| # | Resolved? | Evidence |
|---|---|---|
| R2-M1 | **Yes** | The fix is in the spec (§3.2, D5: the copy is best effort, and the cut always proceeds and is logged and counted) and in code at `c3b8295` (`kept` result → `error!`; `recached_aof_tail_cuts_total{kept}` and `recached_aof_tail_cut_bytes_total`; then `set_len` as in 6O-H). Q1 rerun with a read-only AOF directory: torn plain record, **6O-I starts** (151→142, same as 6O-H); incomplete group, **6O-I starts** (201→115) with an ERROR log naming the offset and size. Test `a_tail_cut_whose_copy_cannot_be_written_still_starts` passes. The Prometheus recorder is installed before the AOF replay (`main.rs` ~260 vs ~554), so the startup counters are actually recorded. |
| R2-L1 | Yes | §11 now says PR 3 is per-frame, today's behaviour exactly, and must not ship without PR 4. PR 4 does what §6.4 specifies (below). |
| R2-L2 | Yes | §3.2 documents the metrics and states that `.cut-` files are never removed by the server, hold user data (0600), and are removed by the operator. |
| R2-L3 | Yes (plan) | PR 6 corpus: every write command, TCP and WebSocket (DEDUP, ephemeral), from each older binary. |
| R2-N1 | Yes | §8 states the pre-existing single-frame read of up to 512 MiB. |
| R2-N2 | Yes | PR 4 test `a_frames_fixed_cost_fits_the_per_frame_charge` passes. |
| R2-N3 | Yes (plan) | PR 6 measures it. |
| R2-N4 | Yes | §5.2 has a new row for a later server's timeless end and its incomplete group. |
| R2-N5 | Accepted | §3.6 already says length corruption is undetectable. |

## PR 4 against §6.4 (no contradiction found)

`apply_repl_group` follows §6.4 step by step:
- It parses every member first (normalize, trailing bytes, clamp) and refuses DEDUP members.
- It takes one `lock_commands` over all members, then one `still_following` check.
- It runs `apply_transaction` at one instant, inside `without_expiry` when `upstream_expirylog`. The closure skips every member after the first error.
- On a panic or a failure it latches only when an AOF exists, and ends the link with a protocol error, which forces a full sync.
- It then does the effects: `on_transaction_logged` (one group in a `txn` file via `append_transaction`; `fan_out_group` to sub-replicas), then notifications and sync pushes per member.

`cargo test` of `replica_atomicity_6oi`, `replication_framing_6oi` and `transaction_framing_6oi` in that tree: 33/33 pass, including the baseline reproduction `a_replica_reader_never_sees_part_of_an_exec`. A full code review is review C's job.

## New findings

| ID | Sev | Finding | Scenario | Required change |
|---|---|---|---|---|
| R3-L1 | Low | A copy that fails part-way leaves a **partial** `.cut-` file. `write_private` opens with create+truncate, then `write_all` fails. The log says the bytes "could not be saved", but a file with the same name pattern remains, and an operator may take it for the complete cut. Under ENOSPC the partial copy also takes the last free space. | Disk with 10 KiB free and a 1 MiB cut from a mid-file length corruption: a 10 KiB `appendonly.aof.cut-<off>-<ms>` remains, the cut proceeds, and the only other record is the ERROR line. | On a failed copy, remove the partial file (best effort). Or write to `<name>.tmp` and rename only after `sync_all`, so a `.cut-` file always holds the whole cut. One-line spec note in §3.2 plus a test. |
| R3-N1 | Nit | Every cut now writes a file and syncs the directory at startup, even for a 9-byte torn tail. | — | None needed. Mention it in the operations doc next to the metrics. |

## Not verified

- PR 5 (rule 2), which is not written yet.
- Real ENOSPC: the copy failure was induced with a read-only directory, which takes the same error path in code.
- Power-loss ordering.
