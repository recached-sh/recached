# Review A, round 1: Phase 6O-I8 benchmark methodology

Reviewer: Review A (independent benchmark-methodology review). Read-only: no builds, tests, benchmarks or servers were run. All numbers below were recomputed with small Python scripts over `benchmarks/v0.6.0/6o-i8/raw/` (runs.jsonl, series.jsonl.gz, perf-stat CSVs, load.txt, server logs).

Scope: `V0.6O-I8-BASELINE.md`, `V0.6O-I8-BENCHMARK-METHODOLOGY.md` (primary), and the measurement claims in `V0.6O-I8-LARGE-TRANSACTION-CONTENTION.md` and `V0.6O-I8-MEMORY-INVESTIGATION.md`; tools in `tools/`; raw campaigns e1, ex, e5, e1d, prof, r1.

## Verdict: NOT APPROVED

The production-build headline in `e1`/`ex` holds up: binaries, workload and pairing are sound, and every number I spot-checked reproduces. The attribution sections do not. Every H-diag run in `e5`, which LARGE §3 and §4.2 and MEMORY §3 rely on, ran with its replica repeatedly dropped and fully resynced. The driver did not catch this and no report mentions it. Several mechanism claims also go beyond the data or contradict it. On top of that, an unexplained background load of about 1 CPU was present during the headline campaigns and is never reported. Fix C1 and H1 to H4 before this is used for a release decision.

## Findings

### C1 (Critical): the e5 H-diag runs had no stable replica; §3 and §4.2 compare unlike regimes

- **Data:** `raw/e5/summary.md`, "full syncs (after prefill)" column, rows `h`: 11, 9 and 14 for the diag cell, and 1, 5 and 0 for the jeprof cell. Lag max is 87.3 M, 29.5 M and 64.6 M frames for H, against about 20 k for I and P. Replica RSS max is 99 to 112 MiB, against about 30 MiB. `raw/e5/runs.jsonl`: `final.primary.replication.recached_replication_syncs_full` is 12, 10 and 15, against 1 after prefill, with `queue_disconnects` 11, 9 and 14. `raw/e5/1-c10k-repl-w4-diag-h/primary.stdout.txt` logs "Replica byte queue full — disconnecting so it can resync" about every 12 s.
- **How long the replica actually streamed:** I counted per-second series rows with `connected_slaves == 1` and lag < 1 M frames inside the measured window. The diag-cell H runs score 8/119, 7/119 and 9/119; every I and P run scores 119/119.
- **Effect:** H-diag effectively ran without a streaming replica and with periodic full syncs. That is why H-diag does 103.9 tx/s and 869 writer ops/s, against 90.8 and 508 for the production H build: the instrumented build is *faster*, a red flag the report does not discuss.
  - The whole LARGE §3 table (phase 1 6.34 against 7.28 ms, stripe hold 9.30 against 10.60 ms, writer stripe wait 4.07 against 8.72 ms, "stripes busy 0.966 against 0.988") rests on these runs.
  - So do §3.3 ("a 14% longer hold costs writers 49%"), §3.1, §4.2's per-transaction bin allocations and dirty pages, and MEMORY §1 and §3's jemalloc `allocated`/`resident` comparison.
  - All 18 e5 runs are `valid=True`, `why=[]`.
- **A less-affected comparison exists:** in `3-c10k-repl-w4-jeprof-h` the replica was attached for 43 of 60 s.
  - All-stripe hold: 10.13 ms (H) against 10.79 ms (P), which is +6.5%, not +14%.
  - `tx_apply` (phase 1): 6.94 against 7.35 ms, which is +0.42 ms, not +0.94 ms.
  - The size of the effects in §3 is therefore overstated by roughly 2×, and the "writers 49%" figure belongs to the broken-replica regime.
- **Fix:**
  - Make the driver record replica disconnects and full syncs over `[load_start, load_end]`. Any non-zero value should put the run in a separate "replica resynced" class, kept out of throughput, diag and paired summaries (see M1).
  - Rerun e5 so the H-diag replica keeps up: replica on a dedicated core, or less per-frame diag overhead (H1).
  - Redo LARGE §3 and §4.2 and MEMORY §3 from runs with a healthy replica, and state the regime explicitly.

### H1 (High): the diag overhead is asymmetric between H and I/P, so "their differences are what is used" does not hold

- **Location:** `raw/diag-patch/diag-h.patch.txt:59,67` together with `i8diag.rs.txt:145-170` (BASELINE l.94; LARGE l.41).
- **What is wrong:** in H the instrumented `lock_stream` runs once per frame: about 1.04 M/s (`stream_free_s` in `raw/e5/summary.md`), each call a `try_lock`, a shared static counter `inc()` and a thread-local counter. In I and P it runs once per group, about 93/s. The replica-side hooks (`R_FRAME_APPLY16` per frame, one in 16) are likewise H-heavy. The frame-pointer and jemalloc `stats` features are on in every diag build, but the per-frame cost exists only in H.
- **Consequence:** the overhead is not a common offset that cancels in H−P differences. It plausibly pushed H's replica, already at 0.80 to 0.85 of a shared CPU in production, over its capacity: the cause of C1.
- **Fix:**
  - Within one campaign, run each build's production and diag variants and report the diag overhead per build (tx/s, CPU/tx, replica CPU).
  - Use diag numbers only as within-build decompositions, unless the overhead is shown to be equal.
  - Count frames with per-thread counters rather than a shared atomic.

### H2 (High): an unreported foreign load of about 1 CPU during e1, ex, r1 and e5 reps 1–2

- **Computed from the series:** host busy (`busy_ns`) minus primary, replica and generator (`gen` sched_ns, which `i8run.py:534` does sample) over the measured window.
  - e1: 1.23 to 1.62 CPUs in every run. ex: 1.41 to 1.65.
  - e5 reps 1–2: 1.34 to 1.65. r1: 1.33 to 1.99.
  - e5 rep 3, e1d and prof: 0.34 to 0.44.
  - The roughly +1.1 CPU step disappears in the middle of e5 (rep 3).
- **Why it matters:** on a 6-core host with every core assigned (primary 0–2, replica 1, generator 3–5), an unpinned 1-CPU consumer competes with the servers.
  - The reports never mention it. BASELINE l.106 says foreign CPU "is recorded per run", but the column (`summarize_i8.py:279-282`) includes the generator and nobody reads it.
  - Absolute numbers differ between regimes: H-fp does 104.9 and 98.5 tx/s in quiet `prof`; H-diag goes from 103.9 to 109.4 when the load vanished in e5 rep 3.
  - The build ratio moves with the regime: H-diag/P-diag is 1.115 in reps 1–2 and 1.143 in rep 3.
  - Cross-campaign statements are confounded. Examples: MEMORY l.67, "decay 0 costs H 13.5 → 14.7 ms", compares e1 (loaded) with e1d (quiet); LARGE l.34 compares absolute values against 6O-I.
- **e1d rep 2 also had a swap storm:** SwapFree fell from 3382 to 2151 MiB, `psi_io` averaged 33% during `2-…-decay0-p`, and the run waited 375 s for load. It counted as valid and quiet.
- **The gate cannot see this:** the gate (`i8run.py:319-333`, 1-minute load < 3.0) is dominated by the previous run's own decaying load and cannot detect 1 CPU of foreign work.
- **Fix:**
  - Sample per-CPU `/proc/stat` and snapshot the top foreign processes before and after each run.
  - Gate on measured foreign CPU, excluding our children and the generator, over about 10 s before the run (for example < 0.25 CPU), and flag runs where it is exceeded during the window.
  - Subtract the generator in `foreign_cpus`.
  - Rerun at least a 3-rep e1 subset in the quiet regime to show the 0.948 ratio does not depend on the background load.
  - State the regime of every campaign in the reports.

### H3 (High): sharing CPU 1 interacts with the build under test, and the effect is never quantified

- **Location:** METHODOLOGY l.61 ("its effect is noted where it matters"); LARGE l.6 and l.60.
- **What is wrong:** H's replica uses 0.80 to 0.85 CPU on CPU 1; P's uses 0.65 (e1 series). So the competition between a primary worker and the replica on CPU 1 differs by build.
  - `psi_cpu` "some" is higher in every H run of e1d (11–12% against 8%).
  - The H-diag collapse (C1) shows H's replica sits near its capacity on that shared core.
  - The only mention in LARGE is one 2.68 ms hold attributed to "a preemption on CPU 1".
  - The control (e2, no replica) is still pending (LARGE §5).
- **Fix:**
  - Add a sensitivity cell with the replica on its own core (for example primary 0–2, replica 3, generator 4–5) for H and P.
  - Wait for e2 before attributing the phase-1 slowdown to the allocator.
  - Report the measured effect in METHODOLOGY §4.

### H4 (High): mechanism claims the data do not support or contradict

1. **"The same cache misses slow the next transaction's phase 1"** (LARGE l.12; l.99 says "the extra cache misses then slow phase 1"). The paired `cache-misses:u` per transaction from e1's perf CSVs is P/H 0.987 (lower) and I/H 1.038 (higher). Same mechanism, opposite directions. LARGE l.36 itself says "fewer cache misses … not memory stalls". L1 and L2 misses were not measured. Either measure them (`L1-dcache-load-misses:u`, `l2_rqsts.miss:u` work at paranoid 2) or remove the causal claim.
2. **"px removes all of it"** (LARGE l.14, l.146) and "one mechanism with three symptoms". px's writers are 0.889 [0.739–1.007] against P's 0.881 [0.873–0.940] (`raw/ex/summary.md`). The writer symptom is not removed. The explanation at l.148 (faster transactions hold the stripes longer) is untested: there is no px diag run. Either measure it (px-diag with a healthy replica) or say the writer effect is unexplained.
3. **"Tokio scheduling: not a material difference"** (LARGE l.87) cites context switches in `raw/e2/`–`raw/e4/`, which do not exist yet. The same line admits the e1 sampler read only the main thread. Mark it pending.

### M1 (Medium): a failed check does not invalidate a run, and replica health during the load is not checked at all

- **Location:** `i8run.py:461`. `valid` stays `True` when the following are appended to `why`: "replica not converged" (l.569–570), "replica digest differs" (l.591–592), "primary does not match the expected keyspace" (l.593–594), and the restart failures (l.607, 612–615). `summarize_i8.py:354` pairs on `valid` alone, so such runs enter the medians.
- **Contradiction:** METHODOLOGY l.24 says "a failure to complete invalidates the run. Only valid runs enter a summary".
- **Missing check:** there is no check for disconnects or full syncs during the load (the root of C1).
- **Fix:** separate *completed* from *passed*. Exclude any run with a failed check, a replica disconnect or a full sync after prefill from the paired ratios, and list it.

### M2 (Medium): driver provenance; the committed driver did not produce e1, ex, r1 or e5

- **Evidence:** `~/recached-6oi8-bench/i8run.py` was last modified at 02:29Z (local 10:29), while e5 was running (02:08–02:56Z). The e1 series has no `ps` field and only main-thread context switches (LARGE l.87 confirms), while ex has `ps`. So at least three driver versions produced the evidence.
- **What is not recorded:** `meta.txt` records binary and i8bench hashes but not the driver's or the summarizer's. METHODOLOGY §2 describes the current driver (per-task context switches, `/proc/<pid>/stat`) as if it applied to every campaign.
- **Fix:** copy `i8run.py` and `summarize_i8.py` with their SHA-256 into each campaign directory and `raw/`. Note which campaigns lack which fields.

### M3 (Medium): the writer metric is too noisy for the way it is presented

- **Data:** e1 paired P/H writers are 0.830 [0.733–1.041], with one rep in five where P beats H. In ex they are 0.881. H alone spans 410 to 569 ops/s across reps and campaigns.
- **Problem:** LARGE §2 bolds "writers −17%", and §3.3 says "the same mechanism costs writers 17%", with no uncertainty caveat.
- **Fix:** more reps or longer windows for the writer metric. Report the sign count (4/5) or a bootstrap interval, and present it as indicative.

### M4 (Medium): cited evidence that does not exist

- METHODOLOGY l.57 cites `raw/methodology/verify-negative.txt` (the verifier's negative test). It exists neither in the repository nor in the bench directory, so the claim that a tampered value, a deleted key and a TTL are each detected cannot be checked.
- Also wrong:
  - METHODOLOGY l.13 `diag/` should be `raw/diag-patch/`.
  - LARGE l.134 `raw/px-patch/px.patch` should be `px.patch.txt`.
  - LARGE l.8 lists e2, e3, e4 and ex2 as raw evidence, but they are not packed yet.
- **Fix:** add the negative-test transcript and correct the paths.

### M5 (Medium): replica verification is digest-only and ignores DBSIZE and TTLs

- **Location:** `i8run.py:587-592`, together with `i8bench/src/main.rs:663-706`.
- **What is wrong:** the FNV digest covers the values of keys 0..99,999 only. The replica's `dbsize`, `ttl_set` and, for paced cells, `ok` are computed but never compared. A replica with extra keys or spurious TTLs would pass. The risk is low for this workload (no TTL commands), but METHODOLOGY §3.3 calls this "real convergence".
- **Fix:** also require `vr.dbsize == vp.dbsize` and `vr.ttl_set == vp.ttl_set == 0`, and `vr.ok == true` for paced cells.

### M6 (Medium): the plateau and post-load claims are stated more strongly than the data allow

- **"Within a few MiB"** (MEMORY l.15 and l.37):
  - I's 30-second means move 116.4 → 129.9 (rep 2) and 130.2 → 115.7 (rep 5), swings of 10–15 MiB.
  - Per-run slopes range from −9.6 to +5.7 MiB/min.
  - A 120 s window cannot separate a plateau from slow growth at this noise level.
- **"Within about 10 s of the load stopping, P's RSS falls below H's (59.1 against 60.7)"** (l.19 and l.39):
  - From the series, 14 s after the load P is still at 63.6–67.0 MiB, while H is at 60.4–62.4.
  - 59.1 is the value 60 s after the load.
- **Fix:** state both accurately. A boundedness claim needs a long soak (≥ 30 min) for P.

### M7 (Medium): repetition counts and balance are not as described

- **e1:** 5 reps over 3 labels gives unbalanced positions. H runs first twice and last twice; P runs first only once.
- **r1:**
  - `reps=1`, so there is no rotation at all: H, I, P in that order in every cell, with the rates ascending.
  - The summary still prints "1.000 [1.000–1.000]" as if it were a range.
- **prof** is 2 reps × 60 s.
- **jeprof:**
  - 60 s per run.
  - The heap profile is a single sampled rep per build: rep 1, whose H run had a disconnect.
- **METHODOLOGY l.65** says "supporting experiments over 120 s, 3 reps". That is not true of prof, jeprof or r1, and LARGE §4.3 does not say so either.
- **Fix:**
  - State n and the window per campaign.
  - Use a multiple of the label count, or a Latin square.
  - Suppress the range when n < 2.

### L1 (Low): stale r1 summary

`raw/r1/summary.md` and `summary.json` say "22 runs, 22 valid", but `runs.jsonl` has 24 (raof-100 I and P are missing from the summary). The summary was written at 10:03 local, before the campaign ended. Regenerate it before the resync report quotes it.

### L2 (Low): per-task schedstat sums lose exited threads

- **Location:** `i8run.py:213-222`.
- **What is wrong:** an exited thread's CPU disappears from the sum. The generator's sum drops at every window end, and the replica's thread count varies between 6 and 8 inside r1 windows.
- **Fix:** cross-check against `/proc/<pid>/stat` utime+stime (already sampled as `ps` since ex), which keeps reaped threads.

### L3 (Low): the perf stat window is not tied to the generator's window

- **Location:** `i8run.py:536-542`.
- **What is wrong:** perf starts after the driver's `sleep(warm)` plus the hostperf loader's start-up, while the paced generator starts 200 ms after spawn. The error is sub-percent, but it is systematic.
- **Fix:** start perf on a marker from the generator, or use `perf stat -I 1000` and integrate over `[measure_start_ms, measure_end_ms]`.

### L4 (Low): the paced generator keeps sending after the measured window

- **Location:** `main.rs:404-408`: the loop is bounded by scheduled due time, not by elapsed time.
- **What is wrong:** a connection that falls behind keeps sending its backlog after the window: 3.1 to 4.5 s in raof-100 (`drained_ms − measure_end_ms`). "RSS at load end" and the replica's catch-up then include load outside the window. METHODOLOGY §3.2 ("nothing is skipped") does not say this.
- **Fix:** document it, or stop at `end` and count the unsent transactions as offered but not accepted.

### L5 (Low): process-hygiene holes

- The prefill and verify children are not in their own process group or in `CHILDREN` (`subprocess.run`, l.511, 515, 583).
- `spawn()` registers a child after `Popen` returns, a tiny window in which a signal would orphan it (l.79–85).
- The foreign-process check (l.280–293) runs only before each run and ignores `redis-server`, `valkey-server` and `redis-benchmark`.
- Only the client ports are ownership-checked, not the replication port.

None of these affected the evidence reviewed.

### L6 (Low): inconsistent statistics in the LARGE §2 table

The column is "P / H, paired median [range]", but user cycles "1.038" and cache misses "0.98" are ratios of medians. The paired medians are 1.033 and 0.987.

### L7 (Low): INFO calls stalled for 2 s or more under load

The sampler's INFO timed out 1–3 times in five r1 runs (`info_err`). `INFO replication` takes the stream lock, so the sampler is both perturbed by the load and perturbing it at the highest rates. Record INFO latency and say so.

### N1 (Nit)

- The METHODOLOGY tool table omits `cycles_i8.py`, `timeline_i8.py`, `perf_i8.py` and `gate2-tests.sh`.
- `tools/i8bench/target/` (build output) sits in the phase directory; do not commit it.
- Every run's SIGTERM shutdown writes two 5.8 MB RDB files to the HDD just before the next run starts. This is negligible, but say so.

## Checked and found sound

- **Binaries:** H, I and P hashes in `raw/builds/*.txt` and every campaign's `meta.txt` match 6O-I's `base`, `i` and `p3` exactly (`6o-i/raw/i5-perf/*/meta.txt`). They were built with one toolchain (rustc 1.98.0, LLVM 22.1.8), with no RUSTFLAGS, `--locked`, separate target directories and clean trees (dirty=0). The -fp and -diag builds share settings, and px differs only by its patch (dirty=2).
- **No variant in the headline:** no -fp, -diag or px binary appears in e1's throughput comparison. px appears only in `ex`, as intended.
- **Build timing:** the px build ran inside `gate1`, with no campaign on the host.
- **Workload reproduction:** `i8bench contend` reproduces `txbench setn unrelated` plus writers byte for byte on the wire: key pool `key:{i:012}`, client and writer seeds, `max(20, TX_N)` draws, 32-byte values, one pipelined write per transaction, timing taken after encoding. The only changes are the warm-up, window-only counting and buffer capacities, none of which change the requests. Aborts and errors are 0 in every run.
- **Contend verification is non-vacuous:** primary and replica DBSIZE are both 100,000, missing 0, digest over the same key format the redis-benchmark prefill uses. This fixes 6O-I's vacuous DBSIZE check.
- **Paced expected-keyspace logic is correct:**
  - Key partitions are disjoint per connection, and each connection's transactions are sequential.
  - The last member wins within a transaction, so its value is the expected one.
  - A transaction with any error reply fails conservatively.
  - All r1 runs pass the primary, replica and restart checks.
- **Process control:**
  - Process groups via `start_new_session`.
  - Ownership checked via `INFO server process_id`; `taskset` execs, so the pid matches.
  - Port blocks below the ephemeral range, with a retry on collision.
  - `flock` in chain.sh, and the driver refuses to run inside the sandbox.
- **Sampler coverage:** every run's series spans its measured window, with no gap over 2.3 s. Server `VmSwap` is 0 throughout.
- **Counters:** perf stat counters were not multiplexed (100% running). Their run time (about 150 s) matches on-CPU time × window.
- **Host state:** governor `performance`, frequencies 3.9–4.1 GHz at every run start.
- **Spot checks:** all e1 medians and paired ratios in LARGE §2 and MEMORY §2, the e1d table, the ex table and the e1 perf-derived values (instructions, cycles, IPC, cache misses per transaction) reproduce from raw data. The heap-profile totals (471.6 M and 435.3 M, about 61 k per transaction) match `raw/e5/jeprof-*-alloc-objects.txt` and `total_tx`.
- **Headline:** the production result in `e1`/`ex` (P/H 0.948 transactions, +3.3% CPU, +8.2% user instructions, +34 MiB window RSS) is internally consistent and reproducible from raw data. The open questions are its sensitivity to H2 and H3, not its arithmetic.
