# Review B, round 1: the 6O-I8 large-transaction root cause

Reviewer B (independent; did not author the analysis). Mandate: try to disprove the proposed root cause.
Scope: `benchmarks/v0.6.0/6o-i8/V0.6O-I8-LARGE-TRANSACTION-CONTENTION.md` (LTC), `V0.6O-I8-MEMORY-INVESTIGATION.md` (MEM), their raw data (`raw/e1`, `raw/ex`, `raw/e1d`, `raw/e5`, `raw/prof`), `raw/px-patch/px.patch.txt`, `raw/diag-patch/`, `tools/summarize_i8.py`, and the sources in `~/recached-6oi8-bench/src-{h,p,px}`.
Read-only: nothing was built or run. The only computation was python3 over the packed logs and summaries (the per-second split in F1 and the symbol diff in F4).

## Verdict: ACCEPTED WITH CHANGES (narrowed scope)

| Claim | Verdict |
|---|---|
| The extra primary RSS (+34 MiB) is allocator-retained dirty pages, not live data, and goes away with the per-frame publish | **Accepted.** jemalloc `allocated` is equal, `resident − active ≈ pdirty`, decay 0 removes the extra (`e1d`), px removes it (`ex`), and it is released after the load |
| P's tx/s loss and its +8% instructions come from the shape of 6O-I's `publish` | **Accepted as "the batch shape of `publish`"**: px restores instructions (0.998×), CPU (1.005×) and tx/s (1.026×) |
| …and specifically from jemalloc thread-cache refill and flush | **Plausible, not isolated.** px changes four things at once (F3), and the instruction budget is not closed (F4) |
| Phase 1 got 0.94 ms slower because of allocator state (cache misses) | **Not accepted.** The baseline is contaminated (F1) and the mechanism contradicts the report's own cache-miss counts (F5) |
| The 14% longer stripe hold explains writers −49% (diag) and −17% (production): "one mechanism, three symptoms" | **Not accepted.** The arithmetic does not hold (F2), and px does not restore the writers (0.889× against P's 0.881×) |

## Findings

### F1. Critical: the H-diag baseline is in a different regime (replica resyncing, writers 20× faster), and MEM says the opposite

- **Evidence:** `raw/e5/summary.md`, the "full syncs (after prefill)" and "lag max" columns, and each run's `primary.stdout.txt`:

  | e5 H run | `Replica byte queue full` disconnects | lag max (frames) |
  |---|---|---|
  | diag rep 1 | 11 | 87 M |
  | diag rep 2 | 9 | 29 M |
  | diag rep 3 | 14 | 65 M |
  | jeprof rep 1 | 1 | 41 M |
  | jeprof rep 2 | 5 | 14 M |
  | jeprof rep 3 | 0 | 29 M |

  Every I and P e5 run, and every production run (`e1`, `ex`, `e1d`, `prof`), had 0 disconnects. H-diag also differs from production H in ways that cannot be overhead:

  | | H-diag | H production |
  |---|---|---|
  | tx/s | 103.9 | 90.8 |
  | writer ops/s | 869 | 508 |
  | writer p50 | 48–818 µs | about 10 ms |
  | writer ops per transaction hand-off | 8.1–11.4 | 4.5–6.4 |
  | replica RSS max | 99–112 MiB | 26 MiB |

  I/P-diag behave like production (writer p50 about 10 ms, about 4.5 ops per hand-off).
- **What is wrong:**
  - LTC §3 says "Diag numbers carry the instrumentation's overhead; their *differences* are what is used". H-diag is faster than production, not slower: the instrumentation changed its behaviour, and the H − P differences mix that change with the build difference.
  - Every H number in LTC §3 and §4.2 rests on these runs: phase 1 6.34 ms, hold 9.30 ms, writer wait 4.07 ms, "stripes busy 0.966", writers ×0.49, and the jemalloc H baseline (dirty 29.0 MiB, nmalloc 40.2 k, resident 91.1 MiB). While a full sync runs, the primary also builds a snapshot, which adds allocations and dirty pages to H.
  - **MEM §3 table and §5 state "no disconnects" and "No run disconnected the replica". That is false for the e5 H runs**, which are MEM's jemalloc evidence.
- **Partial mitigation, measured here:** I split H-diag's per-second I8DIAG deltas by replica state, using the connect and disconnect events in `primary.stdout.txt`:

  | H-diag, seconds with the replica connected | |
  |---|---|
  | phase 1 | 6.10–6.41 ms |
  | stripe hold | 9.04–9.52 ms |
  | hand-off gap | ≈ 0.32 ms |
  | writer stripe acquisitions | 757–886/s |

  The phase-1 gap to P therefore survives, but at about 0.8–0.9 ms. The one resync-free H run (jeprof rep 3) gives a smaller effect: phase 1 6.94 against P 7.35 ms (+0.42 ms, not +0.94), and hold 10.13 against 10.79 ms (+6.5%, not +14%). Yet its writers still fall ×0.51 (782 → 399).
- **Fix:**
  - State the resyncs in LTC §3 and MEM §3 and correct the MEM sentences.
  - Re-run H-diag in a configuration where its replica keeps up. Options: a larger `RECACHED_REPL_BUFFER_BYTES`, or uninstrumented replica code paths (the H replica pays per-frame `one_in_16` and per-64-frame `Instant::now`, while I/P pay per group).
  - Add a **px-diag** build. Comparing P-diag with px-diag is the comparison that isolates the change; H-diag is not needed for the attribution.

### F2. High: the writer mechanism (LTC §3.3) is arithmetically wrong, and px refutes it

- **Evidence:** the `raw/e5/summary.md` diag medians and the `raw/ex/summary.md` paired table.
- **What is wrong:**
  - With two closed-loop transaction clients and exclusive all-stripe holds, the period per transaction is hold + gap, and the free fraction is gap / (hold + gap).
    - H-diag: 1/103.9 = 9.62 ms period, against 9.30 ms held, so the gap is 0.33 ms.
    - P-diag: 1/93.2 = 10.73 ms period, against 10.60 ms held, so the gap is 0.13 ms.
  - A 14% longer hold with H's gap would cut the free fraction only from 3.4% to 3.0%. The observed 1.2% comes from **the hand-off gap shrinking 0.33 → 0.13 ms**. LTC never mentions the gap, and nothing in the allocator story predicts it.
  - Per hand-off, H-diag writers get about 2 operations each and P-diag writers about 1. A writer's second operation fits inside a 0.33 ms gap but not inside a 0.13 ms one.
  - "Writer throughput follows that free fraction (3.4% → 1.2%; writer ops ×0.49)" does not hold either: ×0.35 against ×0.49.
  - Production refutes "one mechanism, three symptoms". **px restores tx/s and instructions, but its writers are 0.889× of H, the same as P's 0.881×** (`raw/ex`). LTC §6 explains this away ("their throughput is a remainder"), but if the writer loss came from P's longer hold, removing the cost would bring the writers back. It does not.
  - Production writer ops per hand-off are noisy: H 4.5–6.4, P 4.2–5.8, px 4.3–4.5. In `ex`, H and P both have a median of 4.8 per hand-off.
  - The production −17% (`e1`, range 0.733–1.041) is asserted to be "the same mechanism" with no production measurement at all.
- **Fix:**
  - Drop the writers from the root cause. Report them as an open, separate effect of hand-off dynamics.
  - If they matter for the gate, measure the hand-off gap and the writer ops per hand-off in H, P and px (a px-diag and a corrected H-diag, F1).
  - Candidate causes to test:
    - work landing on the workers at the hand-off: P's batch `try_send` wakes the replica sender with 10,000 frames at the end of `publish`;
    - the I/P replica's 7 ms group apply bursts on CPU 1, which is also a primary CPU;
    - the reply serialization of the releasing transaction.

### F3. High: px is not a pure "order of allocation" ablation

- **Evidence:** `raw/px-patch/px.patch.txt`. P is `src-p/server-native/src/replication.rs:532ff` and `server_state.rs:826`. The same is acknowledged in `V0.6O-I8-RELEASE-GATE.md` §3 ("Memory" and "Correctness" rows).
- **What px changes besides interleaving allocations with frees:**
  1. It removes two large allocations per transaction: the outer `Vec<Vec<u8>>` (10,000 × 24 B ≈ 240 KB) and the `SmallVec<[ReplFrame;1]>` (≈ 160 KB). Both are above jemalloc 5.3's 32 KiB `tcache_max`, so they take the arena's large path (extent mutexes and the emap) every time.
  2. It fuses three to four passes over 10,000-element arrays (copy, wrap, push, then `retain`/`all`) into one.
  3. It interleaves `try_send` to the replica's mpsc with publication. P queues all 10,000 frames at the end, so the sender task's queue depth and wake pattern change. In P the receive side grows: `Rx::pop` +0.35 and `try_recv` +0.56 points of primary cycles (my diff of `raw/prof/*/perf-primary.top.txt`, scaled by cycles per transaction).
  4. It moves the 10,000 payload copies inside the stream lock.
- **What is wrong:** LTC §6 ("That is the attribution: the cost is the batch order, not the grouping or its single lock hold") and LTC §4.3 attribute everything px fixes to item 0 (the ordering). px confirms the batch *shape* of `publish`, not jemalloc's thread cache specifically.
- **Fix:** run at least one discriminating control:
  - **(a) Allocator-only control, no code change.** Run P and H with a larger thread cache: `_RJEM_MALLOC_CONF=tcache_nslots_small_max:<N>` (with `lg_tcache_nslots_mul`) as a dose-response over N = 200 / 2,000 / 8,000. Optionally also run `tcache:false` on both. If the thread-cache story is right, P's instructions and RSS converge on H's as N grows.
  - **(b) Ordering-only control.** Add a px variant that keeps P's batched `try_send` and intermediate vectors and only interleaves the trim with the pushes. Or the converse: keep the batched allocations and interleave only the sends.
  - **(c) Dose-response by size** in the pending `e4` (100 / 1,000 / 5,000 writes). The thread-cache mechanism predicts no effect while a transaction's objects per size class stay under about 200, and an effect from there on. State that prediction before `e4` is read.

### F4. Medium-High: the instruction attribution is inferred from cycle shares and does not close

- **Evidence:** `raw/prof/summary.md` and `perf-*.top.txt`, produced by `tools/perf_i8.py` with `-g none --sort symbol`. The reports therefore carry no call graphs; the callers were never examined.
- **What is wrong:**
  - **The cycle budget.** P's jemalloc share grows +2.27 points and the `pthread_mutex_*` share +1.46 points: about 1.9 M cycles per transaction, which is the whole cycle increase. The report then attributes the whole **+5.9 M instructions** to that code. That would need an IPC of about 3 in code that includes lock-prefixed atomic operations (about 20 cycles each), which is implausible.
  - **The lock-operation count.** nmalloc's +20 k objects per transaction move in fills and flushes of up to about 100 objects each. That is a few hundred extra lock operations per transaction, which cannot explain +0.76 M cycles in `pthread_mutex_*` unless the locks are *contended*. The report says the opposite ("a failed `trylock` is a handful of instructions").
  - **The caller assumption.** That the `pthread_mutex_*` samples belong to jemalloc is likely (Rust's std and tokio use futexes), but it is assumed, not shown.
  - **A missed symbol.** `tcache_bin_flush_edatas_lookup` has no `_rjem_` prefix, so the classifier misses it: about 0.7–1.0% in every build, +0.37 points H → P.
  - **Other growth.** Outside jemalloc there is +0.9 points of mpsc receive (F3) and +1.3 points in EXEC and store code. The net replication-code saving is −2.8 points.
- **Fix:**
  - Run `perf record -e instructions:u` (or `perf report --sort symbol` on an instructions event), and `perf report --call-graph=caller -S pthread_mutex_trylock` on the existing `perf-*.data`.
  - In the diag builds, which already have stats compiled in, read jemalloc's own counters: `stats.arenas.4096.small.{nrequests,nfills,nflushes}`, `stats.arenas.4096.bins.<j>.{nfills,nflushes,curslabs,nonfull_slabs}`, and the mutex profiles (`stats.arenas.4096.bins.<j>.mutex.{num_ops,num_wait,num_spin_acq,total_wait_time}`, `stats.mutexes.*`). These measure the refills, flushes, slab emptying and lock contention the report asserts.

### F5. Medium: the "phase 1 is slowed by the allocator" mechanism is unsupported and self-contradictory

- **Evidence:** LTC §1 ("The same cache misses slow the next transaction's phase 1") and §4.2 ("the extra cache misses then slow phase 1's own allocations") against LTC §2 ("*fewer* cache misses", 850 k → 834 k per transaction) and `raw/prof`.
- **What is wrong:**
  - **No cache-miss increase.** The only cache-miss measurement shows no increase.
  - **The cycles do not account for the wall time.** In production, the user cycles of phase-1-ish code (`purge_expired_for_write`, `run_transaction_queue`, `memcmp`) grow about 0.7 M cycles, about 0.17 ms per transaction. The diag wall time of phase 1 grows 0.94 ms (0.42 ms in the one clean run, F1).
  - **Neither cycles nor sampling explains the rest.** Kernel time per transaction is equal in `ex` (H 729, P 729 µs) and minor faults are 2–6 per transaction. Whatever lengthens phase-1 wall time beyond that is not on-CPU user code. It could be preemption, migration, or the diag regime itself.
- **Fix:**
  - Either drop the phase-1 claim, or measure phase-1 thread CPU time against wall time (`CLOCK_THREAD_CPUTIME_ID` around phase 1 in a diag build) in P-diag against px-diag.
  - LTC §3.4's "Both are explained in §4 by the allocator" should be weakened to "consistent with".

### F6. Medium: nmalloc and jeprof are compared loosely, and "two extra trips per member" is wrong

- **Evidence:** LTC §1 and §4.2, MEM §4, `raw/diag-patch/i8diag.rs.txt` (it reads `stats.arenas.4096.small.nmalloc`), jemalloc 5.3.0 (`Cargo.lock`: `tikv-jemalloc-sys 0.6.1+5.3.0`).
- **What is wrong:**
  - **What nmalloc counts.** In 5.3, `bin.nmalloc` increases by the *number of objects filled* into a thread cache, plus direct bin allocations; flushes add to `ndalloc`. The +20 k per transaction is objects moved through the bins in batches of up to about 100, about 200 fills, not "two extra trips per member to the arena's bins" (LTC §1). The interpretation in §4.2 ("a cache refill does [count]") is right in direction but wrong in unit.
  - **The jeprof comparison.** The "same requests" figure (about 61 k per transaction) comes from `prof_accum` with `lg_prof_sample:16`, which is a statistical estimate. It covers the whole run (prefill, warm-up, 60 s, drain, 20 s post), divided by a transaction count extrapolated from the measured rate. The H jeprof profile also comes from runs with full syncs (1 and 5; F1). It is adequate to within about ±2%, but nmalloc counts only the measured window, so the two are not like for like.
- **Fix:** use `small.nrequests` (requests, including thread-cache hits) and `small.nfills`/`nflushes` over the same measured window, and restate §1 accordingly.

### F7. Medium: citations of evidence that does not exist yet

- LTC §4.1 rejects "Tokio scheduling and wakeups" partly on "context switches per transaction in `raw/e2/`–`raw/e4/`". Those campaigns are queued (`~/recached-6oi8-bench/queue.txt`), have no data in `raw/`, and the `e1` sampler recorded 0 context switches.
- LTC §6.2 says the server suite and load-and-chaos "were run on px (`V0.6O-I8-CORRECTNESS-VALIDATION.md`)". That file does not exist, and `V0.6O-I8-RELEASE-GATE.md` §3 says "*pending*".
- **Fix:** mark both as pending or remove them until the data exists.

### F8. Medium (an alternative to address explicitly): CPU 1 is shared with the replica

- The replica is pinned to CPU 1, which is one of the primary's three CPUs. It uses 0.66–0.82 of that CPU, and in I/P it applies each group in a 7 ms burst (`r_group_apply`) that coincides with the next transaction's phase 1.
- The report never discusses this alternative. I tested it as far as the data allows:
  - In H-diag, phase 1 is 6.1–6.4 ms with the replica streaming and 5.9–6.2 ms while it is disconnected: only about 4% (F1 split).
  - px has P's replica pattern (replica CPU 0.844× against P's 0.841×) yet recovers tx/s.
  - **So px refutes CPU-1 sharing as the cause of the production tx/s loss.** It does not test the diag phase-1 numbers or the writers (F2).
- **Fix:** add this alternative, with the px argument, to the LTC §4.1 table.

### F9. Low: the maximum stream-lock hold (6.3 ms)

- **Evidence:** the e5 per-rep maxima of `stream_hold_g`:

  | build | rep 1 | rep 2 | rep 3 |
  |---|---|---|---|
  | P | 3.75 | 6.29 | 3.73 ms |
  | I | 3.93 | 3.81 | 3.46 ms |

- **What is wrong:** "6.3 ms in P (3.9 ms in I)" compares one outlier second against I's typical worst; the rep maxima are otherwise equal. The value is wall time on a `std::sync::Mutex` and includes any preemption of the holder (CPU 1 is shared). The diag "p99" columns are log2-bucket upper bounds, so 16,777 µs means a value below 2^24 ns, not a percentile.
- **Fix:** report per-rep maxima and call the value wall time. Note that px moves 10,000 `to_vec` copies inside the hold (the gate lists this as *pending*). Writers are unaffected, but `INFO replication` (sampled every second) and other users of the stream lock wait longer.

### F10. Low: e1d (decay 0) "the same pages" and cross-campaign comparisons

- At decay 0, P has +648 minor faults per transaction (about 2.6 MiB), more than P's per-transaction churn of replication objects (about 10,000 × (80 + 48) B ≈ 1.3 MB) plus the two large intermediate vectors (≈ 0.4 MB, which px also removes). "The same pages" is plausible in kind, not shown in amount.
- e1d also ran with about 1.1 CPU less foreign load than `e1`/`ex` ("foreign+gen" 0.66–0.70 against about 1.76). The cross-campaign sentence "decay 0 costs every build CPU (H 13.5 → 14.7 ms per transaction)" is confounded, although conservatively so.
- Within-campaign pairing is fine.

### F11. Low/Nit: wording not supported by the series

- MEM §2 says "every run's four 30-second means stay within a few MiB". The spreads reach 9.4 MiB (P rep 4: 113.8 → 123.2) and 14.5 MiB (I rep 5: 130.2 → 115.7). Still no trend, so "plateau" stands.
- MEM §2 says "within about 10 s of the load stopping, P's RSS falls below H's". In `e1` rep 2, at +10 s P is 64.4 MiB and H 63.2 MiB; P is below H only from about +15–17 s.
- LTC §6 credits px's 1.026× to "H … looked up the backlog gauge once per frame". The metrics recorder is disabled (`RECACHED_METRICS_PORT=0`), so `gauge!` is a no-op. px's CPU per transaction is 1.005×, not lower, so its tx/s gain is not explained by doing less work; the range [0.992–1.046] includes 1.

### F12. Low: px's semantic equivalence and its test

- **Equivalent as far as I can tell.**
  - Trimming after each push reaches the same backlog as trimming at the end. By induction, the backlog after each push is the longest suffix whose bytes fit the limit, and this also holds when a single frame exceeds the limit.
  - A replica whose byte budget runs out stops at the same prefix and is removed.
  - The side-table group is pushed before the frames, but it is trimmed after them, as in P.
  - `r.sent` is stored per frame in both.
- **What the test does not cover.** It compares px with a hand-written model, not with P's `publish`. It does not exercise `TrySendError::Full` (the frame cap), `Closed` mid-group, or a frame larger than the backlog limit.
- **Fix:** add a differential property test that runs P's `publish` and px's `publish_iter` on the same inputs and compares backlog, groups, queued frames and disconnect counters, including those three cases.

## What I tried to disprove and could not

1. **"The extra RSS is live data, queues, backlog, thread stacks or replica buffering."**
   - jemalloc `allocated` is H 54.5 against P 55.0 MiB in every diag rep. `resident − active` ≈ `pdirty × 4 KiB` (P: 56.9 against 51.6–52.9 MiB) and `pmuzzy` = 0. Threads are 9 in every build, and `RssFile` is equal.
   - The extra disappears with decay 0 (P 62.3 against H 63.2 MiB) and with px (0.980×), and decays after the load.
   - The H-diag contamination (F1) shifts the baseline numbers but not this conclusion: the production-build controls (`e1d`, `ex`) carry it on their own.
2. **"Phase 1 code changed."** `diff` of `src-h` and `src-p`: `core-engine` is identical, and `transaction.rs` differs only by a replica-side `notify_replicated_group`. Phase 1 is unchanged code.
3. **"The CPU increase is kernel time or page faults."**
   - At the default decay, `ex` gives H 729 against P 729 µs of kernel time per transaction, and 2–6 minor faults per transaction. The +0.54 ms is user time.
   - The production per-transaction period grows by about 0.57 ms, about the CPU added. The tx/s loss is consistent with extra work on the serial critical path.
4. **"The replica (or CPU-1 sharing) causes the tx/s loss."** px has P's replica behaviour (group apply, the same replica CPU per transaction) and recovers tx/s (F8).
5. **"Writers wait on the stream lock."**
   - In code, `lock_write` → `lock_stripe_list` takes the write-order stripes before `publish`, and EXEC holds all 256 stripes (`stripe_wait_all` n/s equals tx/s) across phases 1 and 2.
   - In I/P-diag, which are not contaminated, contended single-frame acquisitions are 20–27/s with a mean ≤ 1 µs.
   - Not disproved.
6. **"Writers wait on store stripes."** In code, a writer takes store stripes only after its write-order stripes, which EXEC holds until after phase 2; two transactions cannot overlap. That holds by construction. It is unmeasured (no store-stripe timer), but I found no path that contradicts it in this cell.
7. **"The summarizer's math is wrong."**
   - Per-transaction CPU uses schedstat interpolated to the generator's measured window. Instructions come from a `perf stat -p` started after the warm-up for exactly `measure` seconds, divided by the measured-window transactions.
   - Diag means are Δtotal/Δcount over the window, and maxima are maxima of per-interval maxima. Paired ratios are medians of per-rep ratios. RSS window statistics, slope and 30-second means are computed as described.
   - No error found, beyond the bucket-bound "p99" semantics (F9) and the classifier miss (F4).
8. **"I ≠ P."** The tables agree (instructions 1.085 against 1.082, RSS 1.46 against 1.40, with I's extra live memory in `allocated`).

## What would move this to ACCEPTED

- **F1:** report the H-diag resyncs and correct MEM's "no run disconnected"; base the §3 attribution on P-diag against px-diag.
- **F2:** remove the writers from the root cause, or measure the hand-off gap in production and px.
- **F3/F4:** add one allocator-only control (the thread-cache capacity dose-response) or one ordering-only px variant, plus instruction-event and caller profiles and jemalloc's nfills/nflushes/mutex counters.
- **F5, F7:** weaken or remove the phase-1 cache-miss mechanism, and the citations of evidence that does not exist yet.
