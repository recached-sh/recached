# Review E, round 2: performance and resources (6O-I; PR #200 at 976574a)

Independent reviewer, read-only. No builds or benchmarks were run. Every ratio below was recomputed, paired by rep, from `raw/i5-perf/*/tx.log.txt`, `ord/ord.log.txt` and `stall/stall.log.txt`. The scripts are in this folder:
- `an2.py.txt`: `python3 -I an2.py.txt <tx.log.txt> base,i,p3[,p3nc]`;
- `an_ord_stall.py.txt`: `python3 -I an_ord_stall.py.txt ord|stall <log>`.

## Verdicts

1. **#200 at 976574a: APPROVED. It may merge.** The side table is correct, and I found no defect in the diff `4d46dbc..976574a`. Two optional test and efficiency items are noted (L5, L6).
2. **Performance gate I6: APPROVED WITH CHANGES. It is not yet accurate enough for D6.**
   - Round 1's measurements are now almost all done, and H1, H2 and M1 are closed with good evidence.
   - Two statements in the report contradict its own logs (R2-H1 and R2-M2), and one memory growth is shown in a table but never discussed (R2-M1).
   - With those corrected, and the unexplained items listed as such in D6, the report is sufficient for the maintainer's decision. No further benchmark campaign is required before D6. R2-H1 recommends one cell; the gate can state "unknown" instead.

## 1. PR #200 at 976574a: code review

| Check | Result |
|---|---|
| `ReplFrame` is back to `{offset, Arc<Vec<u8>>}`, 16 B | Yes. The test asserts `size_of::<ReplFrame>() == 16`. `GroupMark` is gone, so **round 1's N1 (silent ungrouping) and N2 (`u32` saturation) no longer apply**. `ReplGroup.bytes` is a full `u64` again, and the only way a group can go ungrouped is a frame count above `u32::MAX`, which cannot fit in memory and is commented |
| `groups` order | Groups are pushed in `publish` under the stream lock, after their frames. Offsets are assigned in the same hold, so the table is sorted by `first` and its entries do not overlap. That is what `partition_point(first <= offset)` needs |
| `group_of(offset)` | It finds the last group with `first ≤ offset`, then checks `offset ≤ last()`. The unit test covers before, first, inside, after, and the gap between two groups |
| Trim | Groups are popped from the front while `last() < oldest`. A group stays while any of its frames is retained, so rule 2 still sees a fragment whose first frame was trimmed. This is the 6O-I worked example, killed by M7/M8. An empty backlog gives `oldest = u64::MAX` and clears the table. `reset_stream` replaces the whole `ReplBacklog`, `groups` included. Cost is amortized O(1) per group |
| `resume_frames` | It runs under the stream lock and the attach barrier, unchanged. It returns groups with `first > requested_offset`. After rule 2, the first missing frame is never inside a group, so every returned group starts at a retained, missing frame. The sender pairs them with `next_if(first == frame.offset)`, so each header precedes its first frame exactly once. On a non-`txn1` link `starts` is ignored |
| Live path | `starts = group.filter(first == frame.offset)`: the header goes on the first frame only. The updated tests in `replication_framing_6oi.rs` and `replica_atomicity_6oi.rs` assert that the other members carry `None` |
| `fan_out` of one frame | `publish(smallvec![bytes])` with `SmallVec<[_; 1]>` for both the input and the `ReplFrame`s: no heap allocation beyond 6O-H's `Arc::new`. `smallvec` is already a workspace dependency, and `Cargo.lock` is untouched. The byte total is now computed only for groups |
| `QueuedReplFrame` | 16 + 32 (`Option<ReplGroup>`) + 8 + 8 = 64 B, against 32 B in 6O-H. That is every queued frame, ordinary ones included. It is still inside the 128 B per-frame queue charge, so the queue's memory bound holds. `ord/` shows no RSS change |
| Mutants | 24/25 killed. M17 (a second header replacing an open group) is equivalent, as claimed: the next member's offset then mismatches the new group's `next()`, so the link still ends with a protocol error, one frame later. No mutant covers the trim loop (L5) |

**The CI read-ahead failure is not related to 976574a.** `a_replica_reads_ahead_while_its_full_sync_checkpoint_runs` writes only single SETs: `fan_out`, no groups. 976574a changes only primary-side code for that path:
- `publish` gains a `SmallVec` and an empty group-trim loop;
- the sender gains `starts = None`.

It does not touch the replica's receive or apply path, or the queue charge. The test checks `queue_disconnects == 0` and `syncs == (0, 1)` before "never caught up", and the failure was the third assertion. So the link stayed up with no resync, and the replica was simply slower than the budget. That budget is 5,120 frames at a test-injected 1 ms `sleep` each, about 5.1 s minimum, within 10 s plus 30 s of polling. That is timer and scheduler starvation: the same job's `pipelined_writers_on_four_workers_strand_nothing` also timed out, and both passed on rerun. It is a pre-existing timing-sensitive test, worth a tracking issue (for example, budget by measured apply rate). It is not a defect of this PR.

## 2. Findings

| ID | Severity | Finding | Evidence (recomputed) | Required change | Blocks I6? |
|---|---|---|---|---|---|
| R2-H1 | High | **A replica with its own AOF is measurably worse with p3, and the report's "both builds converged" is unsupported.** The paragraph says p3 ended every run mid-sync, then says both converged because `DBSIZE` was equal. `DBSIZE` cannot show convergence here: every write overwrites the 100,000 prefilled keys | `e-C` setn100 `REPLICA_AOF=1`, 3 reps each. Base: full syncs 4/4/4, queue disconnects 4/4/4, lag at end 0/0/0. p3: full syncs 5/6/6, disconnects 4/5/5, lag at end 3,264,100 / 881,100 / 18,686,132 frames (in rep 3, lag = offset: nothing acknowledged). `DBSIZE` 100,000 = 100,000 in all 6 | Remove the convergence claim. State that p3 is qualitatively worse in this overload: never back in sync by the end, against base in sync at the end of all 3 runs, cause unknown. Put it in D6. Recommended, not blocking: one cell below saturation (for example setn100 at a fixed lower rate, or set-set) to find whether 6O-I lowers the sustainable write rate of a persisting replica, plus time to lag 0 after the load stops |
| R2-M1 | Medium | **Primary RSS rises 22–45 MiB in the 10,000-write, 3-worker replicated cell.** The table shows 86 → 127 MiB, but the text and the summary never mention it. It is retained at the end of the run, not only a peak. i was not run in this cell, so 6O-I and #200 cannot be separated | `e-C` setn TX_N=10000, 3 workers, REPLICA=1. Paired VmHWM: +21.8 / +34.9 / +44.7 MiB. Current RSS at the end: base 89.0 / 83.8 / 84.4, p3 111.2 / 114.7 / 125.4 (+21.7 / +30.2 / +40.1). Single-worker replica cells: +0–4.5 MiB. AOF-only TX_N=10,000 (campaign 1): no change. The side table holds about 22 groups here, so it is not the cause. Likely suspects: one-burst publication of 10,000 frames into the backlog and queue, and allocator retention across worker threads | Report it in §5.2 and in the summary. Before release (not before merge): one longer run (≥ 120 s) of base vs p3 showing a plateau rather than growth, and i in the same cell. Optionally `MALLOC_ARENA_MAX=1` as an allocator control |
| R2-M2 | Medium | **§7's "converged: yes" column is vacuous, and §7 leaves out that base's 1-worker replica was lagging.** Base's replica RSS is ≈ 24 MiB plus the 64 MiB streaming read-ahead, so it was most likely behind with a full read-ahead. That makes the 1-worker "readers get more done" row partly a comparison against a saturated base replica | `stall/` 1 worker. Base replica VmHWM 87.9–96.3 MiB at TX_N=1,000 and 77.2–95.3 at 10,000; p3 24.5–24.9 and 28.2–29.6. Base reader p50 0.207 ms against p3 0.047–0.055. The log has no lag or offset, and `DBSIZE` is always 100,000 | Drop or rename the column (offsets were not compared). Say that base's 1-worker replica appears saturated (RSS = read-ahead), that p3 applies large groups with less CPU (−11–16% replica CPU per transaction, `e-B`/`e-C`), and that the 1-worker reader gains reflect this. The 3-worker rows are the clean reader-cost numbers for D6 |
| R2-M3 | Medium | **In the 10,000-write cell the extra cost is not only the stream lock: primary CPU rises 5.8% with no AOF and no one-record check in play.** That is ≈ +650 µs per transaction, ≈ 65 ns per member, unexplained. Outside writers lose 22.6%, and the report's "−20%" rounds that | `e-C`: tx/s 0.925 [0.925–0.934]; CPU/tx 1.058 [1.052–1.066]; outside writer ops/s 0.774 [0.737–0.817] (535/530/578 → 437/410/426); writer p99 19.1 → 20.2 ms; tx p50 1.101. By contrast, 1-worker setn100 with a replica is 0.982 CPU (cheaper) | In §5.2 and D6, state both costs as measured and unexplained: lock hold, CPU and writer loss. A profile or hold-time histogram is a follow-up, not a blocker. Quote writers as −23% [−18 to −26%] |
| R2-L1 | Low | **The worst-case replica stall at the default limits is not given.** The near-queue-limit cell was not run. The measured slope allows an estimate | p99 ≈ apply time ≈ 0.35 µs per member (10,000 → 3.5 ms at 1 worker; 2.98 ms at 3). With the default 8 MiB per-replica queue (len + 128 charge, ~75 B SET frames), the largest group a replica can receive is ≈ 40k members ≈ 14 ms. At the 64 MiB group bound (queue and backlog raised) it is ≈ 330k–524k members ≈ 0.1–0.2 s | Add the extrapolation to §7 and D6, labelled as an estimate |
| R2-L2 | Low | Small accuracy fixes, none of which changes a conclusion | (a) The tx/s ratios in the §2.4 and §5 tables are ratios of medians; the paired values are HDD 0.934 [0.919–0.956] (not 0.939), tmpfs 0.973 [0.968–0.995]. (b) Primary RSS with a replica, p3: +2.7–4.6 MiB for 2-write groups (set-set REPLICA 62.3–63.2 against 59.4–59.7; set-set AOF+REPLICA 61.9–63.9 against 59.0–59.4), so "+0–3" understates. (c) Replica RSS at 3 workers, TX_N=10,000: +9.4–13.6 MiB (38.0–42.2 against 28.4–28.9), outside "within 1–9". (d) §1 cites "20 and 100 writes: CPU +2–3% with #200 (§2.3)", but §2.3 is p (617aedb); p3 has setn100 only (tmpfs CPU 1.018, tx/s 0.983 [0.960–0.989]); wide20 was not run on p3. (e) §2.4's "72% more bytes" is the server counter, which includes the prefill; §3's 87% is per transaction; say so. (f) §9 still says campaign 1's AOF bytes "read 0": they read 57 | Correct the text |
| R2-L3 | Low | **Recovery: absolute times are confounded, and peak RSS during replay is missing.** `tail` (exec2's file plus a fragment) restarts in 1.33 / 1.61 s, faster than `exec2` (1.70 / 2.08 s) in both builds. That is unexplained, and it suggests the measurement includes per-workload work besides replay. The paired ratios (0.993, 1.232, 1.229) are fine | `replay-3-interleaved.txt` | One sentence explaining the `tail` < `exec2` difference, or marking it unexplained. Peak RSS during replay is optional |
| R2-L4 | Low | Not run: SET at P=16 with a replica or AOF; a no-transaction reader baseline; catch-up rate; a group near or above the 64 MiB bound (replica HWM; the design's ~450 MiB per link is still an estimate) | `ord/`, `stall/`, `e-C` | List them in §9 as not measured. None blocks: P=1 already shows no change on any changed path, TX_N=1 is a valid non-grouped baseline, and M16 (bound skipped) is killed by tests |
| L5 | Low (PR #200) | No test or mutant covers the side table's trimming. A `groups` table that never shrinks would pass every functional test and leak 24 B per group | `group_table_tests` covers `group_of` only | Add a test: publish more groups than the backlog holds, then assert that `groups.len()` is bounded and that `groups.front().last() ≥` the oldest retained offset. Optionally add a "never trim" mutant |
| L6 | Low (PR #200) | **Side-table cost: 24 B per group, that is 12 B per frame for 2-write workloads.** The code comment ("a frame costs what it did before") is true per frame, not per workload. `resume_frames` scans the whole group table under the stream lock, as it already scans all frames: at a 256 MiB backlog that is ~2M more entries per partial resync | `e-B` +2.7–4.6 MiB; `e-C` 256 MiB backlog: +28–42 MiB (518–523 → 551–561) | Optional: start the group scan with `partition_point` on `first > requested_offset`; shrink entries to `{first: u64, count: u32}` (16 B) by summing `bytes` from the frames at send time |
| N6 | Nit | The differential parser test (from round 1) was not added | — | Optional; no longer blocking |

## 3. Round-1 items: status

| Round 1 | Status | Evidence (recomputed) | Blocks? |
|---|---|---|---|
| H1 set-set `everysec` throughput | **Closed.** The decomposition closes. HDD: wall time per transaction +0.71 µs (1/90,533 → 1/85,027 s) ≈ CPU +0.32 µs + `write(2)` +0.49 µs (0.96 → 1.45 µs/tx). tmpfs: wall +0.24 µs ≈ CPU +0.23 µs, `write(2)` +0.05 µs. `write(2)` calls 0.100/tx and 23 fsyncs per run in every build; ms per `fdatasync` 101 → 121 (median). p3/base tx/s: HDD 0.934 [0.919–0.956], tmpfs 0.973 [0.968–0.995]; coalesced 0.893 [0.879–0.978] (3 reps). The 6% two-write `everysec` loss on an HDD remains for D6 | No (D6 item) |
| H2 ordinary commands | **Closed.** SET/INCR/LPUSH/MSET on no-AOF, `everysec`, replica and `everysec`+replica: paired CPU 0.981–1.009; RSS within ±1 MiB. One outlier: `everysec`+replica MSET tx/s 1.071 [1.056–1.164] (better). P=16 not run (R2-L4) | No |
| H3 replica reader stalls | **Measured.** 3 workers: reader throughput 0.798 [0.779–0.809] at 1,000 writes and 0.688 [0.665–0.729] at 10,000; p99 ×2.79 and ×12.9 (0.23 → 2.98 ms); max 4.9 → 6.2 ms. 1 worker: p99 0.30 → 0.75 and 3.50 ms; max up to 9.35 ms. TX_N=1 unchanged. Report fixes in R2-M2 and R2-L1 | Text fixes only |
| M1 attribution | **Closed.** setn100 on tmpfs: p3 1.018, p3nc 1.002, so the check is all of the remaining per-member cost. set-set: p3 1.032, p3nc 1.026, so the check is 0.6 of 3.2 points and the rest is framing | No |
| M2 backlog memory | **Closed with #200.** Primary RSS with a replica: i +6–13 MiB; p3 +0 (100-write groups) to +4.6 (2-write). Large backlog: +28–42 MiB at 256 MiB, consistent with 24 B per group | No |
| M3 replica resources | **Partly closed.** Replica VmHWM recorded: unchanged at 1 worker; +9–14 MiB at 3 workers, 10k groups. Replica CPU +11% for 2-write groups, −12% for 100-write groups, −16% for 10k groups (e-C). The replica with AOF is a new finding (R2-H1). Catch-up rate and the 64 MiB bound not measured (R2-L4) | R2-H1 text only |
| M4 stream-lock hold | **Measured, cause not separated** (R2-M3) | No (D6 item) |
| M5 syscall counts | **Closed** (server counters in A, B and C) | No |
| M6 AOF+replica, coalesced | **Closed.** AOF+replica set-set 0.945 [0.878–0.979], CPU 1.039; setn100 CPU 1.020. Coalesced as H1 | No |
| L1 paired ranges | **Closed** (`summarize_tx.py`; ±2% floor stated) | No |
| L2 foreign CPU wording | **Closed** | No |
| L3 recovery | **Closed:** sets 0.993, exec2 1.232, tail 1.229 (11 interleaved reps). R2-L3 is minor | No |
| L4 p's `Arc` CPU in the replica cell | **Closed:** p3 setn100 REPLICA CPU 0.982 [0.978–1.005], equal to i | No |
| N1, N2 | **Moot:** `GroupMark` is removed (confirmed in the diff) | — |
| N3, N5 | **Closed:** design §6.3 and §10 are annotated | — |
| N4 | **Closed:** `meta.txt` uses `stat -L` | — |
| N6 | Open, optional | No |

## 4. What D6 should state (reviewer's view)

These are the measured costs the maintainer accepts or rejects with p3:

**Grouped transactions with AOF.**
- Two-write `everysec`: 0.934× on this HDD and 0.973× on tmpfs; coalesced fan-out 0.893×.
- CPU +3.2–3.8% per transaction.
- +114 B and +2 records per grouped transaction.
- Recovery +23% for files of two-write groups.

**Replica readers.**
- With 3 workers, beside looping 1,000- and 10,000-write transactions: −20% and −31% reader throughput; p99 ×2.8 and ×12.9.
- The worst read is the apply time: ≈ 0.35 µs per member, about 3.5 ms at 10,000 writes.
- Estimated about 14 ms at the default queue limit, and 0.1–0.2 s at the 64 MiB bound with raised limits.

**10,000-write transactions with a replica on 3 workers.**
- 0.925× throughput, CPU +5.8%.
- Outside writers −23%.
- Primary RSS +22–45 MiB.
- None of these is explained yet.

**A replica with its own AOF under overload.**
- One or two more full syncs per 20 s.
- Never back in sync by the end of a run, where base was.
- Cause and sustainable rate unknown.

**Memory.**
- Primary RSS with a replica: +0–4.6 MiB at the default backlog, +28–42 MiB at a 256 MiB backlog.
- The ~450 MiB per-link group-buffer worst case is an estimate, not measured.

**Unchanged.** Ordinary commands on every changed path; one-write and read-only transactions; replication alone.
