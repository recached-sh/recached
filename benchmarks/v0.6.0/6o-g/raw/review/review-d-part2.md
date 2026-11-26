## Review D, Part 2: campaign 2 and the profiles

**Verdicts**
- **Campaign 2 methodology: ACCEPT WITH CHANGES.** All 436 cells are present. Pairing and label rotation are intact, every server stayed alive and there were 0 errors. I recomputed every ratio from the tx.log files and they match `V0.6O-G-PERFORMANCE.md`.
- **G4 readiness: ready for a maintainer acceptance decision, with conditions.** The large-transaction reader cost and the capped-store cost are well quantified. Three statements in the write-up need correcting before the maintainer is asked to decide: at-cap cost is "inherent" (D2-3), the worst stall is "bounded by the queue" (D2-4), and "layout" explains the rest of the small-transaction residual (D2-1). The two cheap measurements in D2-1 and D2-2 should also be run first, or the maintainer should explicitly accept going without them.

**Status of Part 1's methodology findings**
- **M-1: partly addressed.** The connection-level in-process benchmark shows parity (get-get 3,066→3,073 ns, set-set 3,250→3,354 ns), but most of the native gap is still unexplained (D2-1).
- **M-2: addressed for 1-worker cells.** The generator used 0.27–0.85 of its 3 CPUs, and its cost per op is the same for both labels (gcap/base 0.98–1.01). The 3-worker cells with 50 clients saturate the host (D2-9).
- **M-3: partly addressed.** There is now a per-cell compiler check and a foreign-CPU metric, but the metric is mostly kernel network time and nothing rejects a cell on it (D2-8). The cmds stage still checks load only once per run: base mset rep 4 started at load 3.43.
- **M-4: addressed.**
  - At-cap cells really evict: 74k–1.13M evictions per cell.
  - Capped cells with readers exist.
  - Readers run on 3 workers, but there is no direct contention metric (D2-6).
  - The replica really streams: 3.4–10.8 s of replica CPU per cell (D2-11).
  - LPUSH and MSET with AOF are now present.
- **M-5: still open.** Readers are closed-loop, and that matters more than the write-up admits (D2-5). The store-level microbenchmark still has only 2 reps. The `g-mem.py` temp-dir leak is fixed.

### Findings
| id | sev | finding | evidence | suggested fix |
|---|---|---|---|---|
| D2-1 | Medium | **Only about a third of the small-transaction native residual is attributed to code; the "layout" explanation for the rest is untested.** Release builds: server CPU per op +4.7% (get-get), +7.7% (exists-get), +6.5% (set-set), consistent in 5/5 reps. The profiling builds don't reproduce it: user cycles per transaction 13,304→12,988 and 14,714→14,692, tx/s ratios 0.99/1.00. But those cycle counts vary ±6% between reps (base set-set 13,719–15,435), so "equal" cannot resolve a 3–5% effect. User instructions are stable and rise +1.4% / +4.1%. I re-ran `prof_lines.py`: the observer checks come to about 620 samples (watch.rs:648/649, sync_router.rs:664, server_state.rs:485), and L1D misses rise +25 per transaction (318→343). Both confirm the write-up's attribution. Evidence that supports layout: replica CPU per transaction +1.2–4.8% on the replica's apply path; LPUSH with AOF +1.2–4.2% CPU per op on a path that only changed `&[resp]` plumbing. | c2-tx/tx.log; c2-prof/prof.log; c2-cmds/results.txt | Run `uprof stat` against the **release** binaries for get-get and exists-get. If instructions rise ~1.4% while cycles rise ~5%, it is IPC (layout). Also run an A/A control: two base builds with different layout perturbations (or `-x86-branches-within-32B-boundaries`; this Coffee Lake CPU has the JCC erratum). Ship the ≤1.5% fixes already listed. |
| D2-2 | Medium | **Ordinary GET/MGET P=1 and P=16, SET and INCR controls were never run on the final binary.** The write-up says g's controls "hold for dev" because #179/#180 only touch transaction code. That contradicts its own layout argument, and LPUSH on gcap already moved +1–4% CPU per op. G4's first bullet (ordinary GET/MGET preserved) is unverified for gcap. | V0.6O-G-PERFORMANCE.md:54; cmds/ uses label g only | Rerun the command controls on rc-gcap (cheap: run-6od, about 15 min). |
| D2-3 | Medium | **The at-cap cost is overstated as "the cost of atomicity under eviction, with or without #180".** `creating_key_count` counts every SET as creating a key, so at the cap 100% of writing transactions take all 256 stripes. Yet about 90% of the keys already exist (≈95k populated, cap 90k), so ≈81% of set-set queues and ≈35% of TX_N=10 queues only overwrite. Costs measured at the cap: 1 worker +21–23% server CPU per op (≈6 µs per transaction, consistent with taking, marking and releasing 256 stripes); wide20 on 3 workers readers 0.783, reader p99 1.43×; TX_N=10 readers 0.918, p99 1.195×. Outside readers almost never read a written key (P ≤ 2×10⁻⁴), so this reader loss is ~100% stripe sharing, not inherent. Also, "evict on every write" is wrong: about 5.8% of writes evict (109k of 1.88M). | c2-capped, c2-stall; store.rs `queue_fits_without_eviction`, `creating_key_count` | Option 1: an exact new-key count under the exclusive gate. Option 2: pre-evict the queue's worst-case new keys before the span, then hold only the queue's own stripes. Either is a design deviation like #180. Estimated gain: at-cap results move toward the "room" ratios (0.985–0.991), recovering about 10–16% of transaction throughput and most of the reader loss. Reword the write-up. |
| D2-4 | Medium | **The stall cells measure the shortest possible span, and several required stall cells are missing.** No large-transaction cell has an observer except AOF `always`, which runs at 9 tx/s, so its span duty is negligible. With AOF everysec, a replica or a watcher, `observed` is true and phase 1 builds frames inside the span, so the hold is longer. Also missing: two or more concurrent looping TX_N=10000 clients (the std mutex is not fair, so reader starvation is possible; this tests the "readers permanently blocked" criterion), a large single collection command inside EXEC (the stall is bounded by data size, not command count), and snapshot or sweeper running during a stall. | g-driver2.sh stall cells; transaction.rs:288 `observed` | Add TX_N=10000 + readers with everysec, with a replica, and with 2–3 transaction clients, plus one big DEL/SUNIONSTORE inside EXEC. Until then, qualify "worst stall = apply time, bounded by the queue". |
| D2-5 | Medium | **Closed-loop reader p99 understates exposure about 3×, and maintainer decision D5 relies on it.** At TX_N≥1000 reader throughput is 0.47, so readers are blocked about 50% of wall time. An open-loop client would then see p99 ≈ span ≈ 5 ms and p95 ≈ 4.4 ms (my estimate), against the reported 1.6 ms. | c2-stall | Report the fraction of reader time blocked, or run fixed-rate readers, before D5. |
| D2-6 | Medium | **Store-stripe contention is inferred, not measured, and worker parking is not separated from stripe sharing.** Readers block in `std::sync::Mutex` on Tokio workers (`wait_for_multi`, `read_one_at` fallback, `read_many` lock path). Evidence of parked workers: server CPU drops 2.03→1.53 (TX_N=1000/10000), 2.07→1.44 (capped with room, 1000) and 2.13→1.36 (at cap, 1000) while readers wait. 3-worker wide20 with 50 clients loses 10.3% transaction throughput with the server at 2.29/3 CPUs and the host at 4.2/6, so it is lock-bound. Base already had the same 80% server-stripe collision odds, but those waits are async. | c2-tx, c2-stall; store.rs:2917, :5633–5770 | In a bench build, count contended store-stripe acquisitions and the nanoseconds spent blocked. Optionally a test build with 4096 store stripes, to isolate sharing from parking. The analytic false-sharing table (99.7% of waits at 2–100 keys, 99% at 1,000, 90% at 10,000) is correct. |
| D2-7 | Low | **FLUSHDB inside EXEC: reader max rose 2,833→10,845 µs, 3.8–4.5× in 3/3 reps.** Readers having to wait for an atomic FLUSHDB is inherent. But most of the ~11 ms is spent dropping the 100k entries while every stripe is marked. Standalone FLUSHDB doesn't block readers. | c2-stall flushdb | Swap the maps under the hold and drop the old ones after release (deferred or background drop). The stall falls to the swap time, in µs. This is not among the listed mitigations. |
| D2-8 | Low | **`foreign_cpus` is not a foreign-load detector.** Its baseline is 0.3–1.8 CPUs and scales with ops/s: `busy_ns` includes /proc/stat irq and softirq time, but schedstat excludes it. There is no reject rule, and the generator is sampled 0.3 s before it exits. Normalised by ops, I found only the outlier already disclosed (base rep 2 get-get, 1.02 vs 0.65) plus capped 1-worker rep 1, which is high for all three labels. | g-tx.sh | Exclude the irq/softirq columns, normalise per op, and add a rerun threshold. |
| D2-9 | Low | **The 3-worker, 50-client cells without readers saturate the host.** Server 1.92–1.95 + generator 1.76–1.84 + kernel 1.72–1.82 ≈ 5.5–5.6 of 6 CPUs. Their tx/s ratios (0.977–0.983) are not server-bound. | c2-tx | Quote server CPU per op (1.041–1.047) for these cells. |
| D2-10 | Low | **Peak RSS at TX_N=10000: +4.6 MB (+13%) uncapped and +7.4 MB (+18%) at the cap; RSS at cell end is elevated too.** The "~500 B/command" figure is an estimate, not a measurement. What is new in gcap: the queue is kept until EXEC ends (base dropped each command as it ran); `writes` grows by doubling from empty (≈88 B per entry); and at the cap the eviction DEL frames are built for every victim even when nothing observes them (`victims`, transaction.rs:349ff). It is bounded by the queue limit. | c2-stall hwm_kb/rss_kb | Pre-size `writes`, build victim frames only when needed, and measure with jemalloc stats. |
| D2-11 | Low | **The replica check is vacuous.** DBSIZE is always 100,000, so a replica that stopped streaming would also pass. Streaming is actually shown by replica CPU, which I confirmed: replica CPU per transaction +1.2–4.8%. | c2-tx REPLICA rows | Compare replication offsets at the end of each cell. |
| D2-12 | Nit | AOF `always` cells (62 tx/s) only measure the disk, and the stall cell's writer p99 of 2.04× is noise (≈740 samples). The 20 s cells were shorter than the brief's preferred 30 s. | — | Already disclosed; keep them out of the conclusions. |

### Classification of the residuals
- **Small uncapped transactions (0.93–0.97 tx/s):** avoidable code about 1.5% (the observer loads, unsized `writes`, the read-only phase-2 future). The remaining 2–5% is probably a build or layout artifact; unproven (D2-1).
- **3-worker wide20 (0.897 / 0.914):** store-stripe false sharing (P(stripe collision) ≈ 80% vs P(key collision) ≈ 0.4%), made worse by blocking waits on the workers.
- **At-cap capped cells:** mostly avoidable implementation and design choice: an over-counting fit check plus eviction inside the span. Reader waits there are ~100% false sharing (D2-3).
- **Stall readers:** at TX_N=100 about 99.7% false sharing; at 1,000 about 99%; at 10,000 about 90%. That leaves at most ~5 of the ~53 points of reader-throughput loss as inherent (reads that hit a written key). Worker parking adds to this but is unquantified (D2-6).
- **FLUSHDB reader max:** inherent that readers wait; the duration is avoidable (D2-7).
- **Peak RSS growth:** avoidable in part and bounded (D2-10).

### What I verified
- Recomputed every paired ratio and the derived server CPU per op, generator cost per op and foreign CPU per op from all three tx.log files.
- Checked cell completeness (250/90/96 rows) and the guard columns: no rows dead or with errors; waits up to 135 s.
- Replica dbsize and CPU in all 10 replica rows.
- Evictions in the at-cap cells.
- Profile counters: instructions, cycles and L1D misses per transaction.
- The flat-profile diff by function across 3 reps per label.
- The write-up's line attribution in `transaction::run_transaction_queue`, re-run with `prof_lines.py` (gcap get-get).
- By reading the code:
  - every SET is counted as creating a key;
  - readers wait in a blocking `std::sync::Mutex`;
  - victim frames are built without a need check;
  - the server takes `lock_transaction` stripes (async, SipHash) independently of the store stripes.
- The write-up's false-sharing probabilities.
- The mem probe's subscriber drop, which matches the documented bound.

### What I could not verify
- Whether the release-build residual is really layout (that needs counters on the release binaries).
- How much of the stall loss is due to parked workers.
- Behaviour of the stall cells with observers attached, with several large-transaction clients, or with snapshot or sweeper running.
- Open-loop reader latency.
- GET/MGET performance on the final binary.
- Cancellation behaviour in phase 2.
- I ran no benchmarks or builds. The only scratch files are my scripts in `/tmp/claude-1000/-home-dennispaler-Documents-OpenSource/391e114a-2abb-4095-bed7-84f8bc88ffb4/scratchpad/review-d2/`.
