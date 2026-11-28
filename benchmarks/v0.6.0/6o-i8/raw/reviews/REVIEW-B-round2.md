# Review B, round 2: the revised 6O-I8 root cause

Reviewer B. Read-only: no builds, tests or servers; only python3 over `raw/e7b`, `raw/et`, `raw/et2`, `raw/prof/callers-*.txt` and the summaries.
Reviewed: the rewritten `V0.6O-I8-LARGE-TRANSACTION-CONTENTION.md` (LTC), the rewritten `V0.6O-I8-MEMORY-INVESTIGATION.md` (MEM), the dispositions in `V0.6O-I8-INDEPENDENT-REVIEW.md`, diag v2 (`~/recached-6oi8-bench/diag/i8diag.rs`, `src-h-diag2`), and the configs of `e7b`, `et` and `et2`.

## Verdict: ACCEPTED WITH CHANGES

| Claim | Verdict |
|---|---|
| P's +8% instructions and the bulk of its tx/s loss come from the batched `publish` through jemalloc's thread cache | **Accepted.** This is now well supported (details below) |
| The extra RSS is allocator-retained dirty pages caused by the shape of `publish` | **Accepted** (px and decay 0) |
| The RSS comes from slab emptying that shrinks as fewer objects pass between the thread caches and the arena | **Not supported** by `et`/`et2` (N2) |
| The writers' per-hand-off loss belongs to the allocator interaction | **Not accepted.** The reports contradict themselves on it, and the tcache-off evidence is a floor effect (N1) |
| A +3% phase-1 regression shared by P and px | **Exists, and should appear in the answer** (N3) |

Why the CPU and tx/s cause is now accepted:
- `small.nrequests` is 60.4 k per transaction in H, P and px.
- `nfills` is 10.7 k → 20.5 k per transaction (px 10.8 k), and bin-mutex operations rise about 10.2 k per transaction with about 0 waits.
- The caller graphs put the extra refill and flush time under `publish`'s `Arc` batch (0.80%) and the trimmed-frame drop (0.42%); phase 1's refill share is equal in H and P (0.39% against 0.38%).
- **With the thread cache off, P's instructions equal H's to three decimals** (99.0 M each, `et2`).
- px restores instructions.

## Status of the round-1 findings

| # | Round 1 | Status | Basis |
|---|---|---|---|
| F1 | Critical: H-diag in another regime | **Closed** | `e7b` runs every diag2 primary beside its label's production replica (`replica_bin_suffix: ""`). 0 resyncs in all 9 runs. The primary's diag2 counts nothing per frame (`lock_stream` only swaps `lock` for `try_lock`; hold timed only for groups). Diag2 tx/s (H 100.4) matches production H on the same quiet host (`et` default: 100.5–103.4), so its overhead is negligible. MEM corrected |
| F2 | High: writer arithmetic; px doesn't restore writers | **Partly closed** | The gap is now measured and equal (H 0.141, P 0.135, px 0.126 ms), and the decomposition is correct. But the writer residual is attributed to the allocator in LTC §1 and §5 and denied in §6 and the disposition (N1) |
| F3 | High: px is not a pure ordering ablation | **Closed for CPU and instructions; open for RSS** | `et`/`et2` change only jemalloc, and tcache off makes P's instructions equal H's. For RSS, see N2 |
| F4 | Medium-High: instruction attribution not closed | **Closed** | Caller graphs, `nfills`/`nflushes`, and bin-mutex operations (+10.2 k per transaction, about 70 cycles per extra lock pair, plausible uncontended). My round-1 objection that "a few hundred fills" could not explain the mutex cycles was wrong: fills move about 3–4 objects each here, not about 100. The tcache-off equality closes the instruction budget |
| F5 | Medium: phase-1 cache-miss mechanism | **Closed (withdrawn).** The remaining phase-1 residual is N3 | |
| F6 | Medium: nmalloc and jeprof wording | **Closed** | LTC §1 and MEM §4 now use requests and fill events correctly |
| F7 | Medium: evidence cited before it existed | **Mostly closed** | The pending items are marked. LTC §4.1's Tokio row still reaches the verdict "no material difference" while half its evidence (`e2`–`e4`) is pending, and it omits the +0.9 points of mpsc receive in P (round-1 F3, item 3) |
| F8 | Medium: CPU 1 shared with the replica | **Partly closed** | Now acknowledged (LTC §3.4, `e6` pending). It is now the leading candidate for both residuals (N3) |
| F9 | Low: maximum hold | **Closed** | Per-rep maxima: P 3.58 / 3.76 / 3.71 ms; px 3.33 / 3.25 / 3.32 ms |
| F10 | Low: e1d "the same pages" | **Closed** | |
| F11 | Low: wording | **Closed** | MEM gives the 30-second spreads; P is below H from 16 s; the gauge remark is gone |
| F12 | Low: px's test | **Open, accepted as deferred** | "To be extended before any PR 3" |

## New findings

### N1. High: the writer residual is attributed to the allocator in one place and denied in another, and the tcache-off evidence for it is a floor effect

- **The contradiction:**
  - LTC §1: "it disappears with jemalloc's thread cache off (writers 0.993×, §4.4), so it belongs to the same allocator interaction".
  - LTC §5: "the per-hand-off loss follows the allocator's behaviour".
  - LTC §6: "px does not restore the outside writers…: the writers' per-hand-off loss is not the allocator (§5)".
  - The disposition of F2: "The writer loss is removed from the allocator root cause."
- **The floor effect** (`raw/et2/summary.md`, tcache off):

  | build | writer ops per hand-off, by rep |
  |---|---|
  | H | 4.10 / 4.21 / 4.17 |
  | P | 4.11 / 4.10 / 4.07 |

  That is the floor: four writers, each with about one operation per hand-off (`waiters_at_release` 3.86–3.92 in `e7b`). With the cache off, every build runs about 36% more instructions at about 80 tx/s. **H loses its second-operation-in-the-gap advantage** (production default H 4.4–6.4 per hand-off), and P cannot fall below the floor. Equal writers there show the metric saturating, not P recovering.
- **px, the cleanest allocator fix,** keeps the per-hand-off loss in production (`ex`): px/H 0.897 / 0.706 / 0.983, the same as P/H 1.02 / 0.90 / 0.92. In diag2 (`e7b`), none of the three builds loses per hand-off (H 4.89, P 5.06, px 5.04).
- **Fix:** make LTC §1 and §5 agree with §6 and the disposition. The per-hand-off residual (about −8% median, noisy: none in `ex`, about −6% in `et`) is **not attributed**. Its only discriminating evidence (px) says it is not the batch. Do not cite `et2` tcache-off for writers, or cite it only with the floor caveat.

### N2. Medium: MEM §3 and LTC §4.4 misread the RSS dose-response

- **Evidence:** `raw/et/summary.md` and `raw/et2/summary.md`, absolute window-mean RSS (MiB) by thread-cache configuration:

  | configuration | P | H |
  |---|---|---|
  | default | 115–119 | 80–84 |
  | 16,000 slots | 116–117 | 89 |
  | 16,000 slots, GC off | 116–119 | 96–100 |
  | thread cache off | 122–126 | 112–126 |

- **What is wrong:**
  - The ratio converges (1.41 → 1.30 → 1.20 → 1.06) because **H's RSS rises**; P's never falls. MEM ("shrinks as fewer objects travel between the caches and the arena") and LTC §4.4 ("with the cache off it falls to +6%") describe the opposite.
  - With 16,000 slots and the GC off, three quarters of P's extra instructions are gone (fewer refills), yet P's RSS does not move. This contradicts the stated pathway, in which the thread cache cannot absorb the batch and so whole slabs empty.
  - The robust facts stand: the extra is dirty pages (diag2: `allocated` equal, dirty 19.1 → 53.8 MiB), decay 0 removes it, and px removes it. The *sub-mechanism* that makes P's batch leave a dirty pool under every thread-cache setting is unexplained.
- **Fix:** correct both passages. Either drop the slab-emptying pathway to "consistent with", or run diag2 under these `_RJEM_MALLOC_CONF` settings and read `resident`, `pdirty` and `stats.arenas.4096.tcache_bytes` (and per-bin `curslabs`/`nonfull_slabs`), which shows what P's resident memory is made of.

### N3. Medium: a +3% phase-1 regression shared by P and px is missing from the answer, and px's recovery partly masks it

- **Evidence** (`raw/e7b`, per-rep phase 1 and phase 2, ms):

  | build | phase 1 | phase 2 |
  |---|---|---|
  | H | 6.85 / 6.86 / 6.76 | 1.48 |
  | P | 7.05 / 7.09 / 7.08 | 1.71 |
  | px | 7.01 / 7.14 / 7.06 | 1.21 |

- **What is wrong:** P's +0.45 ms hold is +0.23 ms in phase 2 (the batch) plus +0.23 ms in phase 1 (not the batch: px has it too).
  - px reaches H's tx/s (diag2 0.998×) only because its phase 2 is 0.27 ms *cheaper* than H's.
  - The same holds with the thread cache off (`et2`: P 1.015×, where P's cheaper publish code outweighs the phase-1 residual).
  - LTC §1's transactions row, and the release gate's "Expected benefit 1.026×", present the batch as the whole loss. A per-frame publish recovers H's tx/s, but leaves a 6O-I phase-1 regression hidden behind 6O-I's cheaper publish code.
  - The two unexplained residuals, phase 1 +3% and writers per hand-off, are both **shared by P and px**. That points at one non-publish change in 6O-I. The prime suspect is the replica's group apply (7 ms bursts on CPU 1, a primary CPU), which `e6` tests.
- **Fix:** add the phase-1 residual to the §1 table and to the gate's expected benefit as an open item, and pair it with the writer residual under the `e6` test.

### N4. Low: an unsupported "every hand-off" claim, and a display bug

- LTC §5: "at least four writer operations complete in every hand-off of every run". The per-hand-off minimum is not measured, and `waiters_at_release` averages 3.86–3.92 (maximum 4), so some hand-offs start with fewer than four writers queued. Say "on average about four".
- `raw/e7b/summary.md` prints `waiters_at_release` as `0.0 / 0 / 0`; `summary.json` has mean 3.86 and maximum 4. The summarizer formats this count as if it were nanoseconds. Fix the formatting of value-type stats.

### N5. Low/Nit: "the I/P diag v1 numbers agree with v2" (LTC §7) is loose

- P diag v1 against v2: phase 1 7.28 against 7.08 ms, tx/s 93.2 against 96.1, writer ops per hand-off 4.55 against 5.06. The foreign load also differed.
- They agree in direction, not in value.

## What I tried to disprove this round and could not

1. **"diag2 still distorts H."** It has no per-frame counters on the primary, runs with a production replica, has 0 resyncs, and its tx/s matches production H on the same quiet host.
2. **"nrequests is not comparable across builds."** It is 60,412–60,433 per transaction across all 9 runs. jemalloc 5.3 merges the threads' request counts at every epoch refresh, and the reporter advances the epoch each second.
3. **"The extra mutex time is contention."** Bin-mutex waits are 0.008–0.08 per second. The extra is about 10.2 k uncontended lock operations per transaction, matching the +9.8 k fills.
4. **"The +8% instructions are code changes, not the allocator."** With the thread cache off, P = H to three decimals (99.0 M). Non-allocator code differences net to about 0, so the default-configuration +6 M is the thread-cache interaction. That leaves no other source for the instructions.
5. **"The large intermediate vectors matter."** P makes 27 large requests per transaction against H's and px's 25, a difference of 2. They are negligible for instructions, which tcache-off equality confirms. They can no longer be blamed for anything but a few hundred KB of churn.
6. **"The stream lock or store stripes delay writers."** Contended single-frame acquisitions are 15–42 per second with means of about 1 µs. A writer holds its stripes for 6.9–7.2 µs in every build. Nothing new contradicts the stripe ordering.

## What would move this to ACCEPTED

- **N1:** one consistent statement that the writers' per-hand-off residual is unattributed (px keeps it), with `et2` not used as evidence for it.
- **N2:** corrected RSS dose-response wording, or diag2 jemalloc statistics under those settings.
- **N3:** the phase-1 +3% residual stated in LTC §1 and the gate, and tied to `e6`.
