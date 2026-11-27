# Review D, round 2 (final): Phase 6O-I8 release risk

Reviewer: Review D.

Method: read-only. I built nothing and ran no campaigns or tests. I read:
- the updated reports, in particular:
  - the release gate, all of it;
  - the correctness report;
  - RST §3.1;
  - LTC §6.3–§6.4;
  - the dispositions in `V0.6O-I8-INDEPENDENT-REVIEW.md`;
- the raw evidence:
  - `raw/tests/index.txt`, plus one flaky-run log (exactly 1 test ran per run);
  - `raw/e4/summary.md`, `raw/e4b/summary.md`, `raw/ex2/summary.md` and `raw/ex3/summary.md`;
  - from `raw/r6/`: `FAILED.txt`, `driver.txt`, the replica's stdout log and the last lines of the series;
  - `raw/r6b/summary.md`;
- the code in `src-h`, `src-p` and, through `git show`, the release tags `v0.5.1` and `v0.5.2`.

## Verdict

**PASS WITH CONDITIONS.** This is final for the phase.

- Round 1's C1–C5 are met; C2 and C3 have small remainders.
- The new autosave and whole-file-replay risk does **not** change the phase verdict:
  - I verified it is pre-existing in H, in P, and in the released v0.5.1 and v0.5.2.
  - It is not caused by anything 6O-I changed.
- It is, however, the most serious **release-level** risk found in this phase.
  - v0.6.0 must not ship until D10 is decided.
  - D10 as framed is close to right, but not specific enough (§2).

Remaining conditions for the phase. None of them needs a new campaign:
1. **R1:** fix the text listed in §3. Four of the fixes are overclaims about convergence. Two are round-1 corrections marked "accepted" that never reached the gate.
2. **R2:** split and sharpen D10 as in §2. Release v0.6.0 only once D10 is decided. If the decision is "document", the documented mitigation must be tested.
3. **R3:** the maintainer records D6, D8, D9 and D10. D10 must also cover review C F10 (the no-back-off retry and the log flood). That part belongs to the same failure chain and has no decision in §2 today.

## 1. Are C1–C5 met?

| Condition | Met? | Evidence and remainder |
|---|---|---|
| C1: `gate2` green on P | **Yes** | `index.txt` shows:<br>• P: inline 858/858, coalesced 858/858, load-and-chaos 865/865;<br>• px: 859, 859 and 866;<br>• every run exited 0 on a clean tree (`dirty=0`) |
| C1: CI-flake study | **Yes** | The test alone and starved: 10 + 10 runs each on P and H, 0 failures. The logs show `1 passed … 815 filtered out`, so the filter really ran the test. Starved runs took 18–26 s against 17 s alone, so the starvation had an effect. Load-and-chaos on 2 CPUs: 3× green for each build (H 812/812, P 865/865). Nothing fails only on P. The test-only recommendation (a deadline instead of an iteration budget) is sound |
| C1: `e4` costs shrink with size, RSS plateaus | **Yes, with a caveat** | Costs fall with size:<br>• 100 writes: instructions +1.7%, transactions 1.02×;<br>• 1,000 writes: instructions +2.0%, transactions 1.05×;<br>• 5,000 writes: instructions +5.4%, transactions 0.95×;<br>• 10,000 writes: instructions +8.2%.<br>Below a few thousand writes, 6O-I is faster than H.<br>RSS at 1,000 writes (`e4b`, 2 × 300 s):<br>• rep 1 is flat (75.6–78.2 MiB);<br>• rep 2 is **still rising at the end**: 70.2 → 75.1 MiB, about +0.4 MiB/min over its last 120 s, converging on rep 1's level;<br>• both return to H's level after the load.<br>That is a plateau supported by one rep, the mechanism, and the post-load return. It is not one demonstrated in both reps. 5,000 writes was measured only over 2 × 60 s. Acceptable: the mechanism is bounded by the decay, and 10,000 writes (5 × 120 s) is flat. `e4` is still labelled provisional and should be labelled so in Q2/Q11 |
| C2: overclaims corrected | **Mostly** | Applied in the reports (LTC, RST, §4). **Not applied in the gate:**<br>• round-1 F9: I8-C still says "every run converged exactly … resynchronizations chain briefly and settle";<br>• round-1 F3: gate §3's expected benefit still quotes only `e1q`'s writers (1.064×), although LTC §7 now gives 0.889 / 1.064 / 0.974.<br>Both are marked "accepted: corrected" in the dispositions. New overclaims: §3 |
| C3: carry-forward completed | **Yes** | §5 now holds F1, F6 (with the primary's snapshot cost), F7, F8, F12, the `tcp_rmem` dependence and the v0.5.1 baseline. Remainder: the F12 row asserts a cause ("its writes are a group long") that nothing measured. Say "not attributed" |
| C4: maintainer decisions listed | **Yes**, framing aside | D6, D8, D9 and D10 are in §2. D10 needs sharpening, and review C F10 needs a decision (§2) |
| C5: soak rerun | **Yes** | `r6` (P first, with `ss` sampling):<br>• one disconnect at +638 s, on an established link (`rb` 27.8 MB), after a 2.72 s stall;<br>• no early chain.<br>`r6b` (I): 0 disconnects in 30 min.<br>Across the five soaks, the episodes are H 0, I 0, P 2. That does not distinguish the builds, and `r4`'s chain did not recur. This supports the chance explanation.<br>Two remainders:<br>• `r6`'s P run aborted before `runs.jsonl` was written, so its keyspace verification is **not in the evidence**. The series ends with lag 0 at the primary's last offset; nothing more is recorded (G3).<br>• A 2.72 s stall broke a link with a fully grown buffer. Established links are safe only up to about 2 s stalls on this host, and the gate's §4 numbers already include this |

## 2. The autosave and whole-file-replay risk (D10)

### Verified against the code

| Claim | H (`6068337`) | P (`54ff38a`) | v0.5.1 / v0.5.2 |
|---|---|---|---|
| Autosave fires only if `dirty_count() > 0` | `main.rs:887–894` | identical (`main.rs:887–894`) | identical (`main.rs:703`) |
| `mark_dirty` call sites | `clients.rs:467,470` (client commands); `main.rs:1008` (the expiry sweeper); none in `replication.rs` | identical | identical (`clients.rs:366,369`, `main.rs:812`). v0.5.1's `apply_repl_frame` executes and then calls `state.on_write(bytes)`, with no `mark_dirty` |
| AOF replay reads the whole file | `tokio::fs::read(path)` in the replay function (`persistence.rs:2486`) | `replay_aof_format` (`persistence.rs:2698`) | `replay_aof_recovering` (`persistence.rs:394`) |
| A replica's AOF is truncated | only by its full-sync checkpoint (`checkpoint_locked_with`, 6O-H) | the same | per 6O-H PERF §5.5, not even then (its base "performs no checkpoint") |

So the risk is **not a 6O-I regression, and not a v0.6.0 regression either**. It exists in the released v0.5.1 and v0.5.2, and v0.6.0 is if anything better, because 6O-H truncates the AOF at each full sync.

6O-I adds only its framing bytes: about 114 B per transaction, or about 1.5% of a 100-`SET` transaction's AOF. That speeds up the growth slightly but does not change its kind.

The `r6` replica log confirms that autosave never fired. The last checkpoint was the full sync at 16:38:25Z, and the load ran until 16:58:07Z, more than 900 s with no save. `FAILED.txt`'s phrase "written since its last 900 s autosave checkpoint" is therefore wrong (G11).

### Classification

The class is **Pre** (pre-existing), at **High** severity for the release. It does not meet the phase's BLOCKED criteria:
- The brief's "unbounded RSS" concerns 6O-I's costs.
- This defect is in persistence code 6O-I did not change, and it is present in shipped releases.

I am not using phase scope to hide it. It is the reason for condition R2.

It is worse than "a restart needs RAM ≥ the AOF". The pieces form a chain on any long-running replica with an AOF under steady writes:
1. A replica's AOF grows without bound between full syncs: 23–25 GB per 30 min in this cell.
2. Eventually its disk fills. The AOF write then fails, and persistence is latched unhealthy.
3. Then F6 starts:
   - one WARN and one ERROR per write (865,301 pairs in `r3`);
   - the next full sync's checkpoint fails for lack of space;
   - after that, a full-sync retry every 2 s with no back-off, each one costing the primary a snapshot.
4. A restart before that point is OOM-killed once the AOF outgrows RAM, as in `r6` (about 11 GB of AOF, killed at 12.3 GB RSS).

`r3`'s tmpfs-quota runs already showed steps 2–3. `r6` showed step 4.

Primaries are exposed to step 4 too, with the defaults: autosave every 900 s after one change, and replay of the whole file. At this cell's 14 MB/s, 900 s of AOF is about 12.6 GB, which is the same size that OOM-killed the replica. On a compressing filesystem (btrfs `zstd`, about 7× here), the on-disk size understates the RAM needed.

### Is D10's framing right?

Mostly yes, but D10 should be split, because the two halves differ in fixability and in their effects:

- **D10a: replicas never autosave.**
  - Recommendation: **fix before v0.6.0**, as its own approved PR. This half is the root of the chain above, and it means the configured autosave is silently ignored on replicas.
  - The fix (replicated applies count toward `dirty_count`) is narrow in code. But it activates the periodic replica checkpoint under load, which has never been exercised (review C N5). I found no test that sends `SAVE` to a streaming replica. So it needs:
    - a design note on the checkpoint's group-boundary and resume-offset behaviour;
    - tests;
    - one `r4`/`r6`-cell soak in which the replica checkpoints and restarts from its own files.
  - If it is deferred, "ship with a documented limit" is acceptable only if the documented workaround has been **tested**. There are two candidates:
    - run replicas without an AOF (no test needed: they resync from the primary);
    - schedule `SAVE` on replicas (needs the test above, because it uses the same unexercised path).
- **D10b: replay reads the whole file.**
  - Recommendation: **ship v0.6.0 with a documented sizing rule**, and put streaming replay in the next milestone.
  - The rule: RAM ≥ dataset + the AOF accumulated over one save interval (write rate × record size × interval) + headroom; on compressing filesystems use the logical size.
  - It is pre-existing, it is bounded on primaries by the save interval, and the streaming change is larger.
- **Review C F10 (back-off, log flood)** is step 3 of the same chain. It should be decided under D10, not left "optional" in §7.

## 3. Accuracy of the release gate §1, §2, §6 and §7

| Place | Status | Issue and fix |
|---|---|---|
| §1 I8-A | Accurate | Review A round 3 is approved with changes, and the changes were applied |
| §1 I8-B | Accurate, with a caveat | "The RSS plateaus at 1,000 writes" becomes "flat in one rep, converging in the other (`e4b`)" |
| §1 I8-C | **Overclaim (G3, G4)** | Three fixes:<br>• "Every run converged exactly" becomes "every completed run matched exactly in memory". The restart is unverified in `r4` (timeout), tmpfs-9200 (quota) and `r6` P (OOM), and `r6` P's verification is unrecorded.<br>• "Chain briefly" becomes "chains of up to 5 full syncs in 150 s; no bound on chain length claimed".<br>• "Memory and queues bounded" gains "except the replica's AOF and its replay (D10)".<br>Cite reviews A round 3 and D |
| §1 I8-D | **Overclaim (G3)** | "Every benchmark run converged exactly" becomes "every completed run". The restart failures are reported in the correctness report §3 but contradicted by its first bullet ("*All* of them passed … nothing lost across a crash-restart"), which needs the same qualifier |
| §2 D6, D9 | Accurate | — |
| §2 D8 | Mostly accurate | "Changes nothing elsewhere (`ex2`)" rests on 2 × 60 s runs. For 2-write transactions, px's 0.963× equals P's provisional 0.964×, and `ex3` showed P's 0.964× to be noise; px itself was not in `ex3`. Say "no change detected in 2 × 60 s" |
| §2 D10 | Needs sharpening | Split into D10a and D10b as in §2. State "pre-existing in v0.5.1/v0.5.2 (verified)", not only "the same in H and P". Add F10 |
| §3 expected benefit | **Round-1 F3 not applied (G4)** | Quote writers as 0.889 / 1.064 / 0.974, as LTC §7 now does |
| §4 guidance | Incomplete | Add that a replica's AOF is checkpointed only by a full sync, so size its disk and RAM for that or run it without an AOF. Add the D10b sizing rule for every node |
| §6 Q2, Q11 | Minor | Mark `e4` as "2 × 60 s, provisional" |
| §6 Q3 | Accurate | It now describes both `e4b` reps |
| §6 Q12 | Minor (G12) | The soak sentence omits H's unforced `r5` disconnect at 1,840 tx/s (+105 s). Q2 includes it |
| §6 Q13, Q14 | Minor (G12) | Add "on this host, a 100,000 × 32 B dataset, `tcp_rmem` 32 MiB" to Q13. In Q14, "behind the file's `fdatasync`" becomes "(inferred)" |
| §6 Q16 | **Overclaim (G3)** | "Yes, every run": `r6` P has lag 0 at the end but no recorded verification |
| §6 Q17 | Accurate | It already says "where checked" |
| §6 Q18 | **Incomplete (G5)** | Add the exception: restart memory equals the AOF size, and the replica's AOF is unbounded (D10) |
| §6 Q19–Q23 | Accurate | Q21 agrees with this review: no focused fix is required for 6O-I |
| §7 | Mostly right | Item 1 is well placed and correctly asks for a design first. Add F10 (back-off, rate-limited logging) to item 1 as part of the same chain. Add F1's large-dataset install measurement to item 1's cell plan. Run future restart checks inside a memory cgroup (`systemd-run --user -p MemoryMax=…`): `r6`'s OOM also killed the session helper's processes and aborted the campaign |

## 4. Findings, by severity

| # | Severity | Finding | Fix |
|---|---|---|---|
| G1 | **High** (release; Pre) | The replica's AOF grows without bound between full syncs because replicated writes never mark the store dirty. Replay reads the whole file. Together with F6, this is a chain: disk exhaustion, then a log flood, then a full-sync retry storm against the primary, or an OOM on restart. Verified the same in H, P, v0.5.1 and v0.5.2; not a 6O-I regression | Split D10 (§2). D10a: fix before v0.6.0, or ship with a **tested** mitigation. D10b: a documented sizing rule, with streaming replay next. Decide F10 with D10 |
| G2 | Medium (release; Pre) | Primaries are exposed to the whole-file replay at the default 900 s save interval. At this cell's write rate that is about 12.6 GB, the size that OOM-killed the replica. Compressed filesystems hide it | Give the sizing rule in §4 and the release notes |
| G3 | Medium | Convergence overclaims in gate I8-C, gate I8-D, the correctness report §3 bullet 1, and Q16. The restart is unverified in `r4`, tmpfs-9200 and `r6` P, and `r6` P's keyspace verification is unrecorded (lag 0 only) | Say "every completed run", and list the exceptions |
| G4 | Low–Medium | Two round-1 corrections are marked accepted but are absent from the gate: F9 ("chain briefly", I8-C) and F3 (gate §3 writers) | Apply them |
| G5 | Low | Q18 and I8-C say memory is bounded, ignoring restart memory and the replica's AOF | Add the exception |
| G6 | Low | `e4` is cited in Q2/Q11 without "provisional". The 5,000-write RSS was seen over 60 s only | Label it |
| G7 | Low | `e4b` rep 2 was still rising at the end (+0.4 MiB/min) | Say "converging"; the bound rests on the mechanism and the post-load return |
| G8 | Low | The §5 F12 row asserts a cause ("its writes are a group long") that nothing measured | Say "not attributed" |
| G9 | Low | F10 has no maintainer decision | Fold it into D10 |
| G10 | Low | §7 omits F1's large-dataset measurement and memory containment for restart checks | Add both |
| G11 | Nit | `raw/r6/FAILED.txt`: "since its last 900 s autosave checkpoint" contradicts the finding. The last checkpoint was the full sync at 16:38:25Z | Correct it |
| G12 | Nit | Q12 omits H's `r5` disconnect. Q13 lacks its host and dataset qualifiers. Q14's `fdatasync` cause is inferred | Qualify them |

## 5. Final assessment

- **D6 finding 1 (10,000-write transactions)** is explained and bounded.
  - The costs fall steeply with transaction size, and below a few thousand writes 6O-I is faster than 6O-H.
  - The cost is a 6O-I implementation effect (the batched publish) with a proven optional repair (px), not a cost of atomicity.
  - D6 can be closed.
- **D6 finding 2 (replica with its own AOF)** is explained.
  - It is not a 6O-I regression.
  - It is a 6O-H-era operating limit with no sharp sustainable rate, and §4 documents it honestly.
- **No focused 6O-I production fix is required.** px stays optional, with its gaps listed.
- The phase found one pre-existing release risk, D10/G1, that matters more than anything 6O-I introduced. The phase verdict stands. **v0.6.0's own release gate must not pass until D10 is decided as in §2.**

**Verdict: PASS WITH CONDITIONS (R1–R3).** I8-E can be recorded as approved with these changes.
