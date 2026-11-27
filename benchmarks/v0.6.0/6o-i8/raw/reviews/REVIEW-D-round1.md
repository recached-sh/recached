# Review D, round 1: Phase 6O-I8 release risk

Reviewer: Review D (independent; did not write the tooling, the analysis or px).

Method: read-only. Nothing was built, run or benchmarked. I read the ten reports, reviews A–C (both rounds), the 6O-I gate (D6) and `V0.6O-I-PERFORMANCE.md` §5.2, the 6O-H performance report §5.5, every `raw/<campaign>/summary.md` named in the brief, `raw/r2/stalls.md`, `raw/r5/bounces.md`, `raw/r1/cycles.md` and `raw/px-patch/px.patch.txt`. I also read `git diff` H..I and I..P for `server_state.rs`, `transaction.rs`, `persistence.rs` and `replication.rs` in `src-p`. Arithmetic below is mine, done from the summaries.

## Verdict

**PASS WITH CONDITIONS. This is provisional.** It holds for the evidence in hand. It is contingent on `e4`, the `gate2` correctness runs and review A round 3, none of which exist yet (§5).

- **No production fix is required before v0.6.0.** PR 3 (px) is optional. It is not ready for approval as written (F4).
- **No BLOCKED criterion is met.** The closest is "repeated full sync below sustainable load", and as an I8 verdict it is not met (§3). But the release gate must carry a dataset-size form of it that I8 did not test (F1).
- **FIX REQUIRED is not supported.** Both D6 cells are now explained. The 10,000-write costs are bounded and have a proven focused repair. The replica behaviour is not a 6O-I regression, so no cause calls for a 6O-I fix.

**Conditions**

1. **C1.** The pending runs complete without contradicting the reports:
   - `gate2` is green on P: the server suite in both AOF fan-out modes, and load-and-chaos.
   - The CI-flake study shows no P-only (or I-only) failure against H.
   - `e4` shows the large-transaction costs shrinking with transaction size, with a primary RSS plateau at every size.

   If `e4` shows an RSS that does not plateau, the verdict becomes BLOCKED. If `gate2` fails on P, it becomes FIX REQUIRED (§5).
2. **C2.** The text corrections in F2, F3, F9–F13 are applied before the gate is signed. They are overclaims, and two of them sit in the operator-facing §4 of the gate.
3. **C3.** The carry-forward list (gate §5) gains the items in F7, at minimum the dataset-size install risk (F1) and the primary-side cost of the no-back-off retry (F6).
4. **C4.** The maintainer records three decisions:
   - D6 closure: accept the 10,000-write costs as explained, bounded, and repairable later.
   - D9: the replica-with-AOF operating limit, with the corrected wording.
   - D8: px either deferred, or approved only after F4's list.
5. **C5 (recommended, about 1.5 h of host time).** Repeat the 1,840 tx/s soak for I and P, rotated (P first), with `ss` sampling (F5). This is the only data point where P looks worse than I, and the two builds have identical replica I/O.

## 1. Classification

Classes:
- (a) an expected, bounded consequence of transaction atomicity.
- (b) a 6O-I regression that has a focused repair.
- (c) an unresolved performance risk that blocks v0.6.0.

Two more labels, because the data need them:
- **R**: an unattributed residual, bounded and small. Carried, not blocking.
- **Pre**: a limit that predates 6O-I. Not a 6O-I class; it is owned by the release gate.

### D6 finding 1: 10,000-write transactions, 4 outside writers, a replica

| Symptom | Measured, P vs H | Cause (evidence) | Class | Blocks v0.6.0? |
|---|---|---|---|---|
| CPU per tx / user instructions | +3.3% CPU, +8.2% instructions (`e1`, 5 × 120 s). With the replica on CPU 5: +1.6% CPU, instructions still +8.2% (`e6`) | Batched `publish` × jemalloc thread cache. Proven four ways:<br>• requests are equal (60.4 k/tx) while refills double (10.7 k → 20.5 k);<br>• caller graphs put the extra time under `publish`'s `Arc` batch and trim;<br>• with the thread cache off, instructions are equal to three decimals;<br>• px restores 0.997–0.998× in `ex`, `e1q` and `e6` | **(b)**. Not intrinsic to atomicity: px keeps one lock hold, consecutive offsets and the group, and costs nothing | No. Bounded and confined to large groups, pending `e4` |
| Transaction rate | 0.948 (`e1`), 0.938 (`e1q`), **0.969** (`e6`, replica off the primary's CPUs) | About half is placement: the 6O-I replica applies a group in a ~7 ms burst on CPU 1, which a primary worker shares. Of the rest, the allocator part is repaired by px. A phase-1 residual of +1.5–1.8% remains in both P and px | **Placement part: (a)**, a consequence of applying a group at once; specific to the benchmark's layout.<br>**Allocator part: (b).**<br>**Phase-1 residual: R** | No |
| Outside writers | 0.830 [0.733–1.041] (`e1`), 0.927 (`e1q`), 0.926 (`e6`), 0.832 (`e1d`), 0.883 (`et`). p99 +4–9% | Writers run only in the hand-off between transactions, so they follow the transaction rate. **But** there is also a per-hand-off loss in 5 of 6 production campaigns: `e6` −5%, `et` −6%, `prof` −11%, `e1d` −13%, `e1` −16%; `e1q` about 0. It is also present in px in 2 of 3 campaigns (`ex` −5%, `e6` −4%) (F3) | **Rate-coupled part: (b).**<br>**Per-hand-off residual: R.** Consistent, unattributed, not repaired by px. Bounded: writers are never starved and get ~4 ops per hand-off | No, provided it is not presented as fixed by px |
| Primary RSS | Window mean +34 MiB (1.40×); max +47 MiB; `VmHWM` +50 MiB | jemalloc dirty pages from slabs that the batch empties. Live heap equal (54.1 vs 54.2 MiB); decay 0 removes it; px removes it; gone within ~16 s of the load; flat over 120 s | **(b)**, bounded by the decay mechanism. Shown empirically for 32-byte values over 120 s only (F8) | No, unless `e4` breaks the plateau |
| Stream-lock hold | 1.2–1.5 ms per 10,000-frame group (worst 3.8 ms), inside the transaction's own all-stripe hold. No writer waits on it | One hold per group, for consecutive offsets | **(a)** | No |

### D6 finding 2: a replica with its own AOF under overload

| Symptom | Measured | Cause (evidence) | Class | Blocks v0.6.0? |
|---|---|---|---|---|
| More full syncs under overload (6O-I: 5–6 vs 4 per 20 s) | `r1`, 3,680–7,820 tx/s. Survival is I/P 0.82–1.29 s vs H 0.61–0.76 s; notice is I/P 0.29–1.91 s vs H 1.33–4.45 s; wait and install are equal | A faster cycle, not lower capacity. The 6O-I replica works through its read-ahead sooner (one `write(2)` per group), so more cycles fit in a window. Review C: H lies outside the I–P noise band at 8 of 8 overloaded rates | **(a)**, a property of group application. **Not a regression** | No |
| "Still syncing at the end" | After the load, every run converged. Below saturation this was by streaming within 0.25 s; in 13 of 15 overloaded `r1` runs it was by a full sync completed after the load | 6O-I's 20-second driver sampled a point in the cycle and checked only `DBSIZE` | Closed (artifact of the old driver) | No |
| Sustainable rate | No sharp rate.<br>• 1,380 and 1,610 tx/s: 0 disconnects on links established before the load (10.9 and 6.8 min per build).<br>• 1,840: H 1, I 0, P 5 (one chain).<br>• ≥ 2,070: failures in every build | **Pre (6O-H design):**<br>• the link reader is a `select!` branch in the applier's task;<br>• the AOF `write(2)` is synchronous on that worker;<br>• an `fdatasync` tail on the HDD blocks both;<br>• the primary's byte budget then disconnects.<br>With no AOF, every build keeps up at the primary's maximum | **Pre**, an operating limit. The same in every build within the power available | Not for I8. D9 needed; see F1 and F2 for what the release must document |
| Real convergence | Exact in every run: primary against the expected keyspace, value for value; replica digest, `DBSIZE` and TTLs. The restart from the replica's own AOF was verified in `r1`–`r3` and `r5`, but not in `r4` (a 22 GB AOF outlasts the 60 s wait) or tmpfs-9200 (quota) | Benchmarks cannot observe a partial group application. That rests on the code, the clean logs and 6O-I's atomicity tests | Satisfied, with stated limits | No |
| Young-link fragility | 18 of 26 disconnects at ≤ 2,300 tx/s came in the first 30 s. After a forced resync, the new link failed once more in 5 of 24 cases, including H 2/4 and I 1/4 **at 1,380 tx/s** | A small TCP receive buffer on a new link (autotuning). Supported, not established. Confounded with AOF age. Measured only at this host's `tcp_rmem` max of 32 MiB | **Pre** | No. Must be in the operating guidance (F2) |
| Chains of full syncs | `r4` P at 1,840 tx/s: 5 consecutive links failed within 150 s, then the 6th held. One link died **during its own install**: a 6.5 s checkpoint outlasted the 64 MiB read-ahead. `r5`: at most one extra failure per forced resync | Young links, plus an install-time failure mode | **Pre**. The chain itself settled. The install-time mode scales with dataset size and was not tested (F1) | Not for I8. Carried as a release risk (F1) |

## 2. Is a production fix needed before v0.6.0? PR 3 (px)

**It is optional.** D6 asked for the two extreme cells to be measured and explained. Finding 1 now has a proven mechanism, a bound, and a demonstrated repair. Its size is the order of costs D6 already accepted as the price of atomicity: 0.934× for two-write AOF transactions, and −20%/−31% reader throughput beside large groups. Unlike those, this cost is *not* intrinsic, so px is worth having. But nothing here forces it into v0.6.0.

**Benefit: proven in one cell family only.**
- In the 10,000-write cell px restores instructions (0.997–0.998×), RSS (0.98–1.02×) and transactions (0.995–1.026×), consistently across `ex`, `e1q`, `e6` and `e7b`.
- It is **not** proven for:
  - outside writers (F3): 0.889, 1.064 and 0.974 in `ex`, `e1q` and `e6`, while the gate quotes only 1.064;
  - ordinary replicated writes (`ex2` pending; px routes `fan_out` through the new `publish_iter`);
  - 2-write or 1,000-write transactions (`ex2` pending);
  - 100–5,000-write transactions (`e4` runs H, I and P only, not px);
  - a primary with both an AOF and a replica;
  - more than one replica;
  - the paced replica cells.

**Risk: semantics understood, performance edges not.** I re-checked review B's equivalence argument against the patch, and I agree:
- Trimming after each push reaches the longest suffix within the limit at every step, by induction. A frame larger than the backlog empties it in both versions.
- `all()` short-circuiting in P and `gone` in px drop a replica at the same prefix.
- The lock is held throughout, so no reader sees an intermediate state.

Open performance edges:
- **Copies inside the stream lock.** The hold grows with the group's *bytes*. For 10,000 × 32 B it fell (1.48 → 1.21 ms). For a group that does not take all 256 stripes but carries large values, ordinary writers on other stripes *do* queue on the stream lock behind it. No cell measures that.
- **`SmallVec<[bool; 4]>` spills to the heap with more than 4 replicas.** It is built on every publish, single writes included, which reintroduces the per-write allocation that review E H2 removed.

**Missing before a maintainer could approve:**
1. `ex2` results.
2. px in an `e4`-style size sweep.
3. **A restriction to reduce the risk surface (recommended):** leave `fan_out`, the single-frame path, on the existing `publish(smallvec![bytes])`, and apply the per-frame loop only to groups. The ordinary-write path then has no change to measure.
4. One cell with large values in mid-size groups beside ordinary writers on disjoint stripes, measuring writer p99 and the stream-lock hold.
5. Review B F12's differential test against the batched `publish`. It should cover:
   - the frame cap (`channel full`);
   - a closed channel;
   - a frame larger than the backlog;
   - more than 4 replicas;
   - a 1-frame group against a 2-frame group;
   - a concurrent `resume_frames`.
6. The server suite in both fan-out modes, and load-and-chaos, on px.
7. An independent design review of the actual PR, then explicit maintainer approval, as the brief requires.

None of these is large. If they cannot all be done before v0.6.0, ship without px and document the cost.

## 3. BLOCKED criteria

| Criterion | Met? | Reasoning |
|---|---|---|
| Unbounded RSS | No (provisional) | It plateaus in every run, the 30 s means do not drift, it is purged after the load, and decay 0 and px remove it. Caveat: 120 s windows and one value size (F8). `e4` could still overturn this |
| Replica cannot sustain a rate the previous implementation sustained | No | H fails at the same rates (r5 at 1,840 and 1,380 after a resync, r1 at 920, r2 at 2,070). I and P have identical replica I/O, and I had 0 failures at 1,840 in 39 min. P's soak chain is the one adverse point; at about 5 failures in 24 per new link, a run of 4 consecutive failures has probability ≈ 0.002 (review C N3), which suggests worse conditions than chance alone. Hence C5. Caveat: "previous implementation" here is H, which is unreleased (F7) |
| Repeated full sync below sustainable load | **Not met as a 6O-I verdict; partly met as a product property** | The 5-link chain was at 1,840 tx/s, a rate the RST's own definition does not call sustainable for H or P. But at 1,380 tx/s, the best rate measured, a forced resync was followed by a second full sync in H (2/4) and I (1/4). So no rate guarantees that a resync under load ends in one full sync. It is the same or worse in H, so it is not a 6O-I regression, and every chain settled. It must not be written as "never disconnects at 1,380" (F2). The install-time variant is not bounded for realistic datasets (F1) |
| Silent divergence | No | Every run matched exactly; the verifier's negative check passes |
| Incomplete transaction application | Not observed | It cannot be observed by end-state checks. It rests on 6O-I's atomicity tests and the clean logs. `gate2` pending |
| Evidence too noisy | No for the conclusions drawn; yes for any "6O-I is better" claim | The contention ratios are tight on quiet campaigns (instructions ±0.1%). Replica counts cannot exclude a 3–5× difference between builds, and the code argument carries the no-regression conclusion. The gate should not claim more |

**Sharp rate or failure counts?** The brief asked for "sustainable throughput measured for each build". The data show that the replica fails through random stall events, so no rate is sustainable with certainty. Failure counts by rate are the honest answer, and they are measured per build. That answers the brief **legitimately, as a negative**: "no sharp sustainable rate exists on this host; here is the failure curve". It does not redefine the target, provided the gate says so in those words and does not turn "0 disconnects in 11 minutes" into "sustainable" (F2). I would add a conservative stated envelope: "≤ 1,610 tx/s of 100-SET transactions on an established link, on this host, this dataset and `tcp_rmem` 32 MiB".

**Is the replica behaviour a 6O-I regression?** No, on the evidence:
- The overload excess is cycle rate.
- The end-of-run state was a sampling artifact.
- Below overload, no difference is detectable, and none is expected: H, I and P share the reader and applier coupling. I and P differ in the replica's AOF path only by #200's cheaper record check and prebuilt markers (CPU, not I/O).
- The 6O-I replica uses 2.6× less CPU per transaction.

## 4. Findings

| # | Severity | Finding | Fix |
|---|---|---|---|
| F1 | **High** | **The install-time failure mode is not bounded for real datasets.** In `r4` P, link 2 died during its own install: a 6.5 s checkpoint outlasted the 64 MiB read-ahead, which is about 4.6 s of stream at 1,840 tx/s. Every replica campaign used a 100,000 × 32 B keyspace, with installs of 0.5–1.0 s. The full-sync checkpoint writes and fsyncs the whole keyspace, so its duration grows with the dataset. Once install time exceeds (64 MiB read-ahead + 8 MiB queue + sockets) / stream rate, every full sync under that load fails during its install: a full-sync loop far below the stated envelope, each iteration costing the primary a snapshot. This is a 6O-H mechanism (#190), not 6O-I, and 6O-H is unreleased, so it ships in v0.6.0. Neither the RST nor the gate generalizes it | Add it to the gate §5 as a release risk. Before v0.6.0, either measure one cell (e.g. a 1–2 GB keyspace, replica AOF on the HDD, 500–1,380 tx/s, a forced resync) or state the bound explicitly in the operating guidance. Link it to 6O-H's carried "configurable read-ahead, with a metric when full" |
| F2 | Medium | **Gate §4 overstates the envelope.**<br>• "Never disconnected at 1,380 and 1,610 tx/s … in any build" is true only for links established before the load; after a forced resync at 1,380, H failed 2/4 and I 1/4.<br>• "May fail once more within the first minute": `r4` link 3 failed at age 61 s, and P's chain had 5 failures, not 1.<br>• The envelope depends on `tcp_rmem` max 32 MiB; the kernel default of 6 MiB was not run and would tolerate shorter stalls.<br>• It depends on dataset size (F1) | Qualify each bullet: "on an established link", "after a resync, up to 5 consecutive full syncs observed; no bound claimed". Add `tcp_rmem` and dataset size to the operational guidance |
| F3 | Medium | **The px writer benefit is cherry-picked, and the writer residual is understated.**<br>• The gate quotes writers 0.927 → 1.064 (`e1q`); `ex` gives 0.889 and `e6` 0.974.<br>• LTC §5 calls the per-hand-off residual "small next to H's spread" and omits `e6`, the cleanest cell. There it is −5% for P (3.91 vs 4.11) and −4.5% for px (3.92), with tight ranges.<br>• It appears in 5 of 6 production campaigns.<br>• LTC §2.2 says "px restores everything, the outside writers included" | Report all three px writer results. State the residual as "consistent 5–13% per hand-off in most campaigns, unattributed, not reliably removed by px". Add `e6` to the LTC §5 table |
| F4 | Medium | **PR 3 is not approvable as written.** The list is in §2:<br>• `ex2` and px-in-`e4` are missing;<br>• copying inside the lock is unmeasured for large values;<br>• the `SmallVec<[bool;4]>` heap spill with more than 4 replicas;<br>• the F12 differential test, the suites on px, and a PR review are missing | Do §2 items 1–7. Prefer leaving `fan_out` untouched |
| F5 | Medium | **The one adverse replica point is weakly controlled.** `r4` ran H, I, P in sequence (P last), with no `ss` sampling, 1 rep. P's 4-link chain is ≈ 0.002 likely at `r5`'s per-link rate. The no-regression argument then rests on the claim that I and P are the same code, and RST §1 states that too strongly: P's replica AOF path differs from I's by #200's `is_one_record` byte loop and prebuilt markers (`persistence.rs`). That difference is CPU only and leaves the `write(2)` shape unchanged | C5: rerun the soak for I and P, rotated, with `ss`. Correct the wording to "the same I/O path" |
| F6 | Medium | **Review C F10 is carried incompletely.**<br>• The gate lists "retries a full sync every 2 s without back-off" but omits that **each retry costs the primary a snapshot** (214 failed checkpoints in one `r3` run). A replica whose disk is full therefore loads its primary indefinitely.<br>• The per-write WARN+ERROR flood (865,301 pairs) can itself fill the disk the logs share.<br>• "Pre-existing" means pre-6O-I. Whether it predates v0.5.x is not established: 6O-H PERF §5.5 says 6O-H's base performed no full-sync checkpoint, while the code comment calls it pre-existing | Carry both items with the primary-side cost stated and with an owner. Recommend small separate fixes for v0.6.0 (exponential back-off on a failed post-sync checkpoint; rate-limited AOF-failure logging), each under its own approval, or a documented mitigation |
| F7 | Medium | **The carry-forward list is incomplete.** Missing:<br>• F1;<br>• F6's primary cost;<br>• the `tcp_rmem` dependence;<br>• large transactions together with a replica AOF (the replica cells used 100-SET transactions; the 10,000-write cell had no AOF);<br>• more than one replica or sub-replica chains under these loads;<br>• no pre-6O-H (v0.5.1) baseline for the replica-with-AOF cell. "Previous implementation" in this phase is H, which is unreleased, so the release-level comparison is open;<br>• RSS beyond 120 s and at other value sizes (F8);<br>• `r4`'s unverified restart;<br>• the overload stall asymmetry (F12) | Add these to gate §5 |
| F8 | Medium | **RSS boundedness is shown for one shape.** The mechanism (slabs emptied by the batch, kept for the 10 s decay) scales with replicated bytes freed per second. "+34 MiB" is therefore the value for 32-byte values at about 90 tx/s, not a bound. `e4` varies the transaction size but not the value size, and no cell ran longer than 120 s | State the bound relative to the mechanism: about the pages emptied within one decay period, purged within ~16 s after the load. Add one larger-value cell if time permits, or rely on px |
| F9 | Low | Gate I8-C wording:<br>• "chain briefly and settle" describes 5 full syncs over 150 s;<br>• "every run converged exactly" is true in memory, but the restart was not verified in `r4` or tmpfs-9200 | Reword with the numbers |
| F10 | Low | Gate §4 says "without the disk [6O-I] sustains the primary's maximum where 6O-H's does not". This held on tmpfs only, for about 92 s before the quota, with H at a 4–8% higher accepted rate (review C F10). With no AOF, all builds sustain the maximum | Qualify as stated |
| F11 | Low | RST §3.1 says "below about 2.4 per hour **per build** (95%)". With 0 events in about 83 min, the bound is ≈ 2.2/h **pooled** over the three builds; per build (≈ 28 min) it is ≈ 6.5/h | Say "pooled" or give the per-build bound |
| F12 | Low | In overload (`r1`, 3,680–9,200) I and P show longer AOF `write(2)` blocking than H: the largest per-interval totals are 2.1–3.75 s against 1.2–1.7 s, and intervals with more than 1 s number 1–11 against H's 1–2. RRC does not mention it. It is plausibly dirty-page throttling from faster ingestion. Overload only; it does not change the conclusion | Note it in RRC §2 as observed and unattributed |
| F13 | Low/Nit | Stale or inaccurate text:<br>• methodology §5's rep table still shows `e1q`/`e6`/`r4` "pending" and `r3` "running", lacks `r5`, and lists `e2`/`e3` at 3 reps (they ran 2, "provisional");<br>• the LTC header cites `raw/e4/` and `raw/ex2/`, which do not exist yet (a repeat of review A M4);<br>• LTC §1 says "about a third" of phase 1 is placement, but the data give about half (+0.23 → +0.11 ms);<br>• LTC §3.5 is out of order;<br>• the `e6` diag cell's writer ratio (P 0.719, px 0.776, from a noisy H) is unreported;<br>• the jemalloc `mallocs/s` column is arena-level `nmalloc` (H 4.4 M/s vs P 6.4 M/s) and could be misread as more requests | Fix. Label the column `nmalloc (arena)` |
| F14 | Nit | The phase-1 residual (+1.5–1.8% with the replica off CPU 1) exists only with a replica (`e2`: P = H), and the primary's phase-1 code is unchanged H→I (checked). A testable hypothesis is interference from bursty group delivery: the primary's sender task and the replica's 7 ms group apply both run during the next transaction's phase 1, on shared L3 and memory bandwidth | Optional: state it as a hypothesis in LTC §3.4. Not needed for the verdict |

## 5. What remains pending, and how it could change the verdict

| Pending | Could change | How |
|---|---|---|
| `gate2`: the server suite in both fan-out modes, and load-and-chaos, on P | I8-D, and the verdict | Any failure on P → FIX REQUIRED (or BLOCKED for an atomicity or convergence failure) |
| CI-flake study: 10 runs alone and 10 starved, on P and H | I8-D | A failure only on P, or only on I, in replica catch-up → a 6O-I regression to explain before PASS |
| `e4`: 100/1,000/5,000 writes | I8-B and D6 closure | Primary RSS without a plateau at any size → BLOCKED. Costs not shrinking with size (e.g. ≤ 0.95× at 1,000 writes) → the cost is no longer confined to an extreme cell: D6 must be re-accepted at that size, and px becomes recommended for v0.6.0 rather than optional |
| `ex2`: px on ordinary, 2-write and 1,000-write paths | D8 only | A regression on the ordinary path → px cannot be approved as written (keep `fan_out` unchanged, F4). It does not affect the I8 verdict, since px is not merged |
| px suites and the F12 differential test | D8 only | As above |
| Review A round 3 (I8-A) | I8-A | A new validity defect in `e1q`/`e6`/`e2`/`e3`/`r4`/`r5` could reopen I8-B or I8-C |
| C5 soak rerun (recommended) | I8-C confidence | A repeated P-only chain with I clean would undermine the chance explanation and require a look at P's primary-side changes |
| Gate §6 assessment and verdict | — | Still blank. This review's verdict assumes the gate adopts the classification above, with the conditions |
