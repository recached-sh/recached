# Review A, round 2: Phase 6O-I8 benchmark methodology

Reviewer: Review A. Read-only, as in round 1. No builds, tests, benchmarks or servers were run while r3 occupies the host. Everything below was recomputed with small Python scripts over `benchmarks/v0.6.0/6o-i8/raw/`.

Scope:
- the dispositions in `V0.6O-I8-INDEPENDENT-REVIEW.md`;
- the updated methodology, `V0.6O-I8-LARGE-TRANSACTION-CONTENTION.md` (LTC) and `V0.6O-I8-MEMORY-INVESTIGATION.md` (MEM);
- the new campaigns e7, e7b, et and et2;
- the replica-capacity work: r1, r2, `V0.6O-I8-REPLICA-SUSTAINABLE-THROUGHPUT.md` (RST), `V0.6O-I8-REPLICA-RESYNC-ROOT-CAUSE.md` (RRC), `tools/cycles_i8.py` and `tools/timeline_i8.py`.

## Verdict: APPROVE WITH CHANGES

The Critical finding is resolved:
- e7b reruns the diag attribution with production replicas, and no replica resyncs.
- The summarizer now has a comparability rule.
- The quiet campaign `et` reproduces the headline ratios on production binaries: P/H 0.936 transactions, 1.028 CPU and 1.081 instructions, against e1's 0.948, 1.033 and 1.082.
- The large-transaction attribution (LTC §3, §4.2 and §4.4) now rests on valid data.

The approval is conditional on the items marked **(gate)** below. The replica-capacity conclusions (RST §1 and §3) are **not yet approvable**: their failure pattern coincides with a disk-busy episode from outside the benchmark (N1), and "sustainable" is defined from 2-minute windows (N2).

## Round-1 findings

| # | Status | Evidence and what remains |
|---|---|---|
| C1 | **Closed** (one caveat) | e7b: 9/9 runs comparable, 0 full syncs after prefill. H's lag max is 45–51 k, so the replica keeps up. `replica_binary` = production `rc-<label>` in every run (`raw/e7b/runs.jsonl`). The replica-side cause rests on one e7 H run (diag2 primary with a diag2 replica: 16 full syncs, 110 tx/s, 1,284 writer ops/s) against three e7b H runs. That is adequate for "exclude e5 H", not for a measured root cause. The diag v2 patch and the `*-diag2` build logs are not in `raw/` (N4). |
| H1 | **Closed** | v2 has no per-frame shared counter; `stream_free_s` is "–" in e7b, and only contended waits are timed. The attribution now uses diag primaries next to production replicas. |
| H2 | **Partly** | The quiet `et` default cell (foreign about 0.37 CPU) reproduces e1's ratios, which settles the headline. But (a) the new generator-net foreign CPU is biased (N3); (b) foreign load and disk activity came back during r2 rep 1 (foreign 0.9–1.0 CPU in H runs, disk 81–96% busy at 1,840–2,070 tx/s), still ungated and unflagged (N1); (c) e1q is pending. LTC l.42 still cites `prof` (frame-pointer builds with `perf record` attached, 2 × 60 s) as the quiet reproduction; cite `et`'s default cell instead. |
| H3 | **Open** (e6 pending) **(gate)** | New evidence makes it more pressing (N5). |
| H4 | **Partly** | The cache-miss claim is withdrawn and the caller graphs are added, both good. The writer explanation now contradicts itself (N6). LTC §4.1 marks the Tokio row as pending. |
| M1 | **Closed for future runs; partly for packed evidence** | The rule is in `summarize_i8.py:337-341,371`, but the packed summaries for e1, ex, e5 and e1d were not regenerated with it (N7). METHODOLOGY §2 l.24 still says "a failure to complete invalidates the run. Only valid runs enter a summary", and does not describe the comparability rule that §6 l.97 points to. The rule counts only *full* syncs (`full_syncs_load`); a disconnect healed by a partial resync would not exclude a run. Add `disconnects_load > 0`. |
| M2 | **Partly** | The §6 table ends at v5, but there are two hashes after it: e7 ran `bfb0f618…`; e7b, et and et2 ran `b13c3791…`, the current file, which adds `replica_bin_suffix`. Add v6. The claim that no change altered "how a run is judged valid" (l.87) is not true of v5, which added `why` entries for replica DBSIZE, TTL and missing keys. |
| M3 | **Closed** | The writer range is reported. |
| M4 | **Partly** | `raw/methodology/verify-negative.txt` exists and shows detection of a tampered value, a deleted key and a TTL. Still wrong: METHODOLOGY l.13 `diag/` (should be `raw/diag-patch/`); LTC l.5 says `-diag2` is in `raw/diag-patch/`, but only the v1 patches are there (N4). |
| M5 | **Closed for v5+** | `i8run.py:597-599` compares DBSIZE, `ttl_set` and `missing`. r1 and r2 ran v2 and v4 without this check; say so in RST and RRC. `ttl_set` counts PTTL −2 (missing key) as a TTL (`i8bench verify`: `!= -1`); the negative test shows `ttl_set=2` after one PEXPIRE and one DEL. Count only values ≥ 0. |
| M6 | **Partly** | The 30-second-mean wording is fixed and the post-load timing is fixed ("meets H at about 10 s, below from 16 s"). MEM l.20, "no positive trend over 120 s in any run", is false: e1 I rep 3 is +5.66, I rep 2 +4.54, P rep 4 +2.30 and P rep 5 +1.74 MiB/min. Write "no consistent trend". A boundedness claim still needs a soak longer than 120 s. |
| M7 | **Partly** | §5's table now has n and windows, but (a) `e7` is listed as "3 × 120 s", while it was aborted after 3 runs (1 rep; `chain.txt`: exit 143); (b) `e7b` and `et2` are missing; (c) e1q, e2–e4, e6 and ex2 are listed without "pending". |
| L1 | **Closed** | The r1 summary covers 24/24 runs. |
| L2 | **Partly** | Stated in §7 as "Tokio's blocking-pool threads only". It is not only those: the generator's threads exit at the window's end, which corrupts `gen_cpus` (N3). |
| L3, L4 | **Closed** (stated as limits) | |
| L5 | **Not addressed** | Not mentioned in §7. Acceptable, but say so. |
| L6 | **Closed** | Paired values are now stated. |
| L7 | **Partly** | §7 says INFO gaps are "missing from the series, not interpolated as valid". `summarize_i8.py:50-60` `interp` skips rows without the field and interpolates linearly across the gap, so counter deltas *are* interpolated. Fix the text or the code. |
| N1 (r1) | **Open** | The METHODOLOGY tool table still omits `cycles_i8.py`, `timeline_i8.py`, `perf_i8.py`, `gate2-tests.sh` and the v2 diag sources. |

## New findings

### N1 (High, gate for RST): the 2,070 tx/s failures coincide with an outside disk-busy episode, and the "P failed fewest" ranking comes from it

- **Data** (`raw/r2/summary.json`, `disk_util`, `psi_io_mean`, `r_aof_ms_per_fsync`, `full_syncs_after_prefill_total`):
  - From 04:08 to 04:21Z (rep 1: `raof-1840-p`, then `raof-2070` H, I and P) the disk was 81–96% busy, with I/O pressure at 25–29%.
  - Every other run was 13–24% busy, with I/O pressure at 4–8%, at the same 2–2.8 MB/s of writes and the same replica AOF rate (14–15.7 MB/s).
  - The driver's `disk()` records only write fields plus `io_ms`, so the extra busy time is reads or another process, not the replica.
  - In that episode H failed 3 times, I 5 times and P 0.
  - In the quiet rep 2, every build failed exactly once, with normal `fdatasync` (150–185 ms). In rep 3, none failed.
- **Consequences:**
  - "P failed the fewest reps at 2,070 tx/s" (RST l.13 and l.15) is an artefact of one noisy rep.
  - "The runs that failed are those with the longest mean fdatasync … so the stalls are the trigger" (RST l.59) is confounded: the long fsyncs belong to the episode, and rep 2's failures had normal fsyncs. P's 309 ms run did not fail.
  - Foreign CPU was also 0.9–1.0 in r2 rep 1's H runs at 1,380–1,840 tx/s.
- **Fix:**
  - Record diskstats reads (`r_ios`, `r_sect`) and per-server `/proc/<pid>/io`.
  - Flag runs whose disk busy time is not explained by the servers' own writes.
  - Repeat the 2,070 cell with more reps once the host is quiet.
  - State the per-build failure counts as indistinguishable at n=3.

### N2 (High, gate for RST): "sustainable" is defined from 2-minute windows, but the failure mode is a random stall

- **What is wrong:** RST l.59 says the failure "is a matter of whether a long enough stall happens within two minutes". If so, 0 failures in 9 × 120 s at 1,840 tx/s bounds the failure rate only loosely: roughly < 1 per 6 minutes at about 63% confidence. It does not establish a sustainable rate. Over an hour, 1,840 may fail as 2,070 does.
- **Also:** the 2,300 tx/s "fails for every build" result comes from r1: one 90 s run per build, in the noisy regime (foreign 1.3–2.0 CPUs).
- **Fix:**
  - Report the boundary as "no failure in N × 120 s", with the ladder's 230 tx/s resolution.
  - Run at least one long soak (≥ 30–60 min) at 1,840 tx/s per build (H and P), or report failures per hour against the rate.

### N3 (Medium): the generator-net foreign CPU is wrong in about half the runs

- **Location:** `summarize_i8.py:333-337`.
- **What is wrong:** the generator's per-thread schedstat sum collapses when its worker threads exit at the window's end. Example, `e7b/1-…-diag2-h`: `gen.sched_ns` 53.1 s at +119 s, then 26.3 s at +120 s, with 7 → 2 threads.
  - Interpolating at `measure_end` therefore yields a bimodal `gen_cpus` (0.15–0.16 against 0.37–0.41 in e7b, et and et2).
  - `foreign_cpus_net` is inflated by about 0.2 CPU correspondingly (0.54–0.62 against 0.34–0.40).
  - The same field gives negative generator CPU in r1. The sum `gen_cpus + foreign_cpus_net` is constant, which confirms the cause.
- **Fix:** use the generator's `/proc/<pid>/stat` utime+stime (the `ps` field, already sampled and cumulative over exited threads), or stop interpolating at the first sample after the thread count drops.
- **Affected:** the per-run foreign values in summaries and in METHODOLOGY §7's ranges.

### N4 (Medium): diag v2 evidence is not packed

- **What is missing:** `raw/diag-patch/` holds only the v1 patch (`diag-{h,i,p}.patch.txt`, `i8diag.rs.txt`). `raw/builds/` has no `build-*-diag2.txt`.
- **Where v2 lives:** the bench directory (`diag/apply_diag.py` and `diag/i8diag.rs`, now v2, with v1 copies alongside). LTC §3's "the timers cost the same in every build (nothing is counted per frame)" cannot be checked from the repository.
- **Fix:** pack the v2 patches per tree (h, p, px), the applier and the diag2 build logs with their hashes. The e7b `meta.txt` hashes are `61b72631…`, `76f47fc3…` and `9103aba9…`.
- **Also:** `meta.txt` should record the replica binary's hash when `replica_bin_suffix` differs from the primary's.

### N5 (Medium, supports H3): H's throughput varies with its replica's lag; P's does not vary

- **Data:** in the quiet campaigns, H's transaction rate spreads by 3–5% across reps:
  - et default: 100.5, 102.5, 103.4;
  - et 16k: 102.2, 103.6, 106.5;
  - et2 16k-nogc: 103.4, 103.8, 108.4.
  - P stays within 0.4–0.6%: 95.8–96.2, 96.5–97.0 and 100.4–100.8.
- **Correlation:** within a campaign, H's rate correlates with its replica's mean lag over the window: r = 0.90 (et), 0.76 (et2), 0.98 (e7b), 0.73 (e1), from my series extraction. P's lag is pinned at about 20 k.
- **Interpretation:** the direction of causality is not established. Still, H's baseline varies and is coupled to a replica running at 0.8+ of a CPU it shares with a primary worker. So the P/H ratio depends partly on how H's replica fared in each rep.
- **Fix:** this is the case for e6 (the replica on its own CPU) before the 0.94–0.95 ratio is quoted as build-intrinsic. Report H's lag next to its throughput.

### N6 (Medium): the writer explanation contradicts itself, and the tcache-off control is a regime change

- **The contradiction:**
  - LTC §1 (l.16) and §5 (l.160) say the writers' per-hand-off loss "disappears with jemalloc's thread cache off … so it belongs to the same allocator interaction", and that it "follows the allocator's behaviour".
  - §6 l.179 says "the writers' per-hand-off loss is not the allocator".
- **The evidence against the allocator account:**
  - px restores the allocator's behaviour: refills 10.8 k against H's 10.7 k (§4.2). Yet in `ex`, px's writers are 0.889× and its writer operations per hand-off 4.52 against H's 4.76.
  - tcache-off drops every build about 20% (H 80 tx/s) and changes the hand-off timing for all builds. Equality in that regime does not show that the default-regime loss is allocator-caused.
- **Fix:** pick one statement. The data support: "the per-hand-off writer loss is not explained; it is absent in the diag builds and under tcache-off, and present in px".

### N7 (Medium): the packed summaries for e1, ex, e5 and e1d predate the new summarizer

- **Evidence:** `raw/{e1,ex,e5,e1d}/summary.{md,json}` were written at 11:44, have no `comparable`, `gen_cpus` or exclusion list, and `prof` has no `summary.json` at all.
- **Consequences:**
  - `raw/e5/summary.md` still presents H-diag paired ratios (writers 0.492, etc.) with no exclusion.
  - METHODOLOGY §6 l.97 ("applied … to every campaign alike") is not true of the packed evidence.
- **Fix:** regenerate every packed summary with the current summarizer and record its hash (`5e9604f6…`).

### N8 (Medium): the RRC cycle metrics are measured too loosely for the conclusions drawn

- **What the tool does:** `tools/cycles_i8.py:53-56` measures a cycle's "survival" from the previous install, which for the first disconnect is the *initial* sync about 3.4 s before the load. For example, in `r1/raof-25` the installs are at −3.4/−3.3 s, so the first survival includes the prefill tail and the warm-up. Disconnects during the 15 s warm-up are counted too (I at 2,300 tx/s: disconnects at 3.3, 7.6 and 12.6 s).
- **Where it matters:** at the rates with 1–4 cycles, a median of 1–4 values from a single run (r1 has n=1) cannot support RRC l.43 ("P's survival is equal or longer at every overloaded rate"). It also contradicts l.41, "I tracks P at every rate": at 2,300 tx/s, I's survival is 3.9 s against P's 30.1 s and its disconnects 4 against 2, with identical replica code.
- **Fix:**
  - Start survival at max(install, `measure_start`) and count only disconnects inside the measured window.
  - Draw the overload conclusions only from rates with ≥ 10 cycles (3,680 tx/s and up, where they look sound).
  - Drop "at every rate".

### N9 (Low): the disk results reflect this filesystem and highly compressible synthetic values

The AOF is written at 14 MB/s and reaches the disk at 1.5–2.1 MB/s: about 7× from btrfs `zstd:1`, because the values are `cc:seq:member:` padded with `v`. btrfs compression runs in kernel workers, so its CPU counts as foreign. The sustainable rate and the `fdatasync` latencies are therefore specific to the host and the data. State this in RST's cell definition. Ideally, check one rate with incompressible values, or on a non-compressing mount.

### N10 (Low): timeline and cycle tools

- `timeline_i8.py:102` reads the uncompressed `series.jsonl`, so it cannot run on `raw/` (gzipped).
- `timeline_i8.py:120` takes the nearest row regardless of distance; mark values more than 1.5 s away.
- `cycles_i8.py` matches "queue full" and "fell too far behind" on the primary. If a single event ever logs both lines, it is counted twice. That did not happen here: r1 has 305 byte-queue-full lines and 0 frame-cap lines.

### N11 (Nit)

- RST l.17 says "57 runs"; r1 and r2 hold 60 (24 + 36). State which 3 are excluded (probably the 9,200 tx/s ladder runs, accepted 0.958–0.971).
- METHODOLOGY l.41 still says `foreign_cpus` includes the generator, while the summaries now print the net value; describe both.

## Checked and sound in round 2

- **e7b design:** diag v2 primaries with each label's production replica. Labels are rotated over 3 reps, all 9 runs are comparable and every digest is equal.
- **e7b numbers:** the LTC §3 medians reproduce from `raw/e7b/summary.md` (transactions/s 100.4/96.1/101.1, writers 490/487/505). P/H is 0.958 for transactions and 1.028 for CPU.
- **et/et2 ablation:** these change only `_RJEM_MALLOC_CONF` on the production H/P binaries (hashes in `meta.txt`), are paired within campaign, and give results consistent with LTC §4.4: tcache off gives instructions 1.000 and transactions 1.015; the 16k cache without GC gives instructions 1.023.
- **Writer per hand-off:** the LTC §5 table reproduces from e1 (H 5.52, P 4.62).
- **r2 sustained runs:** at 1,380–1,840 tx/s, all 27 runs accepted 1.000, had 0 full syncs and disconnects, lag max 0.26 M frames, catch-up ≤ 0.25 s, and passed the primary, replica and restart checks. The 2,070 tx/s failure counts (3,1,0 / 5,1,0 / 0,1,0) match. The r2 failures total 11 byte-queue-full disconnects.
- **r1 logs:** 305/305 disconnects are byte-queue-full, there are 0 partial syncs, and the full-sync installs number 325, consistent with RRC §1.
- **Driver v5/v6:** replica DBSIZE, `ttl_set` and `missing` are compared (l.597–599). The driver hashes itself into `meta.txt`, and the restart uses `replica_binary`. The committed `tools/i8run.py` is byte-identical to the bench copy that ran e7b, et and et2.
