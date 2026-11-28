# Review A, round 3 (final): Phase 6O-I8 benchmark validity (I8-A)

Reviewer: Review A. Read-only, as before. No builds, campaigns or servers were run. For the recomputation I ran small Python reads over `raw/`, plus `tools/stalls_i8.py` and `tools/summarize_i8.py` at `nice 19`, writing their output to my scratchpad.

## Verdict

| | Verdict |
|---|---|
| **Investigation 1** (large-transaction contention, memory) | **APPROVED WITH CHANGES** (text only) |
| **Investigation 2** (replica with its own AOF) | **APPROVED WITH CHANGES** (text only) |
| **I8-A overall** | **APPROVED WITH CHANGES** |

No Critical or High finding remains, and the measurements are valid for the claims as now framed:
- builds bit-identical to 6O-I;
- comparability enforced and verified by regenerating the summaries;
- quiet-host and placement controls run (e1q, e6);
- failure counts per rate counted from the load's start, with soaks and forced resyncs added.

The changes below are corrections to the report text. Several statements were not updated after `r6`, and a few numbers do not match `raw/`. None of them changes a conclusion.

## 1. Question 1: is Investigation 2's methodology sound for the claims?

**Mostly yes.**

- **Counting from the load's start.**
  - **Sound:** `stalls_i8.py:67-79` counts primary queue-full events in [load_start, load_end], or up to the first bounce in r5. That includes the warm-up, where young-link failures happen.
  - **Recomputed:** I recomputed the table over r1, r2, r4, r5 and r6b (r3 from `stalls.md`). It reproduces every row from 920 to 2,300 tx/s: minutes, disconnects, and intervals with more than 1.0 s of `write(2)`.
  - **Two limits remain:**
    - r6 has no `runs.jsonl` (the driver was SIGTERMed after the OOM), so P's sixth 1,840 tx/s disconnect and its 30 minutes were added by hand. I checked them against `raw/r6/…/primary.stdout.txt`: one queue-full at 16:38:20Z, +638 s after the load start (`start_ms` 1795796862351). They are correct, but no tool reproduces them (R3-11).
    - RRC counts the same r1 runs over the measured window (H 3,680 tx/s: 17), while RST counts from the load's start (21). Both are legitimate; say so where the two reports meet.
- **Episode counting.** Not defined. RST l.15 pools "independent failure episodes at 1,380–1,840 tx/s: H 3, I 1, P 4". That pool mixes:
  - unforced failures: H 1 (r5, +105 s), I 0, P 2 (the r4 chain, and r6 at +638 s);
  - failures within seconds of a *forced* bounce: H 2/8, I 1/8, P 2/8.

  These two have different exposure: one is per unit of time, the other per induced resync. Report them separately, and define an episode explicitly, for example "disconnects whose links were installed less than N s apart" (R3-3).
- **The young-link receive-buffer evidence.** The data support the hypothesis less specifically than the text implies (R3-2):
  - **"Supported, not established" is the right status.**
  - **Overreach:** "The variable is the buffer's size, not the link's age" (RST l.92) is not supported. In `raw/r5/bounces.md`, all 24 forced links started at 0.13–2.43 MB. Every failure came 4–31 s after the install, when every link is small.
  - **Counter-evidence:** of the 9 links still ≤ 2.5 MB 30 s after install, 8 survived to the next bounce.
  - **Collinearity:** buffer size and age are collinear in this data. Both are further confounded with the AOF's age (stated) and with the post-install backlog. A just-installed replica starts behind, holding a full read-ahead (RRC §4 says as much for overload).
  - **Reverse causation:** TCP autotuning sizes `rb` from how fast the application reads, so a slow or stalling replica keeps a small `rb`.
  - **Wording:** "young links, which also have small receive buffers, young AOFs and a backlog".
  - **The data handling itself is correct.** `bounces_i8.py` keys sockets by peer port and reads the replica's side; I confirmed replica-side rows (`127.0.0.1:43266 → :27012`, `rb22589867`) in `r5/ss.txt.gz`. `rb` is correctly described as the limit, not the occupancy.
- **The forced-resync design.** Sound for its claim ("a resync under load can chain once more, then holds"):
  - **The procedure:** SIGKILL, then a restart on the replica's own files, an AOF replay (11–21 s), a reconnect and a full sync under load (`i8run.py:558-575`).
  - **The data:** four bounces per run at fixed times. All six runs passed the primary, replica and restart checks. Every post-bounce disconnect is a byte-queue-full, and the r5 logs hold 6 in total: 1 unforced and 5 forced.
  - **Limits, as stated:** one run per build and rate, fixed label order (H, I, P), mid-load recoveries not verified, and the 900 s autosave never fired.
  - **Mixed link-age definitions:** "4 and 8 s into the new link" (P) is measured from the install, while "5 and 44 s in" (H) is measured from the kill. 539 − 495 = 44 s is about 31 s after H's install.
- **The stall mechanism (§4).** Sound within each build: applier stall ≈ link-read gap; `write(2)` time accumulates while the write count collapses. But the r3 diag cell runs **diag v1 on both nodes** (`rc-h-diag` primary and replica). That is H's per-frame-timed replica, the build behind round 1's C1, here at 0.76 CPU and 332 µs/tx, against 300–314 µs/tx in production. H's stall and gap figures in RST §4's table are therefore not like-for-like with I/P (R3-9).

## 2. Question 2: do the numbers reproduce?

- **RST §3, the failure-count table.**
  - **The 1,840 row reproduces:** H 39.4 min, 1 disconnect, 12 intervals > 1 s; I 69.7 min, 0, 5; P 69.7 min, 6, 4. The r6 contribution is manual, as noted above.
  - **So do the rows 920 to 2,300.**
  - **The 3,680–9,200 row is wrong:** H's runs have 13–21 disconnects, not 11–17; I's 16–23, not 12–19; P's 21–23, not 17–19. Intervals > 1.0 s: I 1–9 and P 1–11, not 1–7 and 1–9 (R3-5).
- **RRC.**
  - **Reproduces:** the §2 table matches `raw/r1/cycles.md`, and its survival is now measured from an install during the load. The disconnect counts check out: 361 byte-queue-full events in r1–r3 (305 + 11 + 45), none from the frame cap, and no partial syncs in r1–r5.
  - **Wrong:** l.35, "1.48 s (H)", should be 1.68 s (5,060 tx/s), and the range includes 9,200 tx/s, a rate the methodology rules invalid for replica capacity.
- **LTC §6.3 (e4).** The medians match `raw/e4/summary.md`:

  | P/H | 100 writes | 1,000 writes | 5,000 writes |
  |---|---|---|---|
  | transactions | 1.018 | 1.051 | 0.953 |
  | instructions | 1.016 | 1.020 | 1.054 |
  | RSS | 1.015 | 1.236 | 1.335 |

  The e4b 30-second means match the text: P rep 2 goes 70.2 → 75.1.
- **LTC §6.4 (ex2, ex3).** Every cell of the table matches.
  - ord-w50: P 0.997 and px 1.006 writers; CPU 1.005 and 0.991; instructions 1.018 and 1.008; RSS 1.003 and 1.017.
  - setset: 0.964 and 0.963; instructions 1.030 and 1.010; RSS 1.056 and 1.066; replica 1.109 and 1.102.
  - 1,000-write: 1.053 and 1.043; instructions 1.020 and 0.995; RSS 1.186 and 1.014.
  - ex3: P 0.991 [0.981–1.007], CPU 1.001; I 0.991, 0.999.
- **RELEASE-GATE §6.** These reproduce:
  - Q1 and Q2's ratios: 0.97× (e6), 1.05–1.08× at 1,000 writes (e4, ex2, e4b), 1.02× at 100, 0.99× at 2 writes (ex3), 1.01× with no replica (e2).
  - Q6's "about half": e1q 0.938 against e6 0.969.
  - Q11 (all twelve values), Q16 (13 of 15 overloaded ladder runs via a full sync after the load), Q8–Q10.
  - Q17: I checked post hoc over all 278 runs that have a replica verification in `raw/*/runs.jsonl`, including pre-v5 campaigns. Replica DBSIZE, `ttl_set`, missing keys and digest equal the primary's in every one.
  - §3's e1q px figures: 0.995, 0.997, 1.018 and 1.064.
- **RELEASE-GATE: what does not reproduce, or overstates:**
  - **Q2:** "Replica with AOF: the same failure counts at every rate". At 2,070 tx/s they are 4/6/1, at 1,840 tx/s 1/0/6. RST says "no difference detectable". Use that, here and in I8-C's "the builds do not differ" (R3-7).
  - **Q3:** "flat … over 2 × 300 s at 1,000 writes". e4b's P rep 2 rises 70.2 → 75.1 MiB, slope +1.04 MiB/min; LTC §6.3 describes it correctly as "flattening" (R3-8).
  - **Q12 and §4 l.45:**
    - "four 30-minute soaks" should be **five** (H r4; I r4, r6b; P r4, r6), as should RST l.79's "Over the four soaks".
    - §4 l.45 lists P's r4 chain (+14 to +150 s) under "on an **established** link", but it is the young-link case (R3-6, R3-1).
  - **Q14:** the stall range "0.35–2.3 s" leaves out the 2.72 s stall in r6.

## 3. Question 3: is the methodology rep table (§5) accurate?

The status, labels, reps and windows match each `config.json` and `runs.jsonl`:

| | |
|---|---|
| e1 | 5×120 |
| ex | 3×120 |
| e5 | 3×120 and 3×60 |
| e1d | 3×120 |
| prof | 2×60 |
| e7 | 1 rep, aborted |
| e7b, et, et2 | 3×120 |
| e1q | H/I/P/px, 3×120 |
| e6 | H/P/px, 3×120, both cells |
| e2, e3 | 2×120 |
| e4 | 2×60 per size |
| ex2 | 2×60 |
| e4b | H/P, 2×300 |
| ex3 | 3×120 |
| r1 | 1×90 |
| r2 | 3×120 |
| r3 | 2×120 |
| r4 | 1×1,800, not rotated |
| r5 | 1×600 per rate, 4 bounces |
| r6 | P only, load completed, aborted |
| r6b | I, restart check off |

Inaccuracies:
- **r3's cell is "diag" with diag v1 on both nodes.** §7 still says "Diag v1 … its H runs are excluded; v2 is equal by construction", which is not true of r3 (R3-9).
- **e6 also moved the generator** from CPUs 3–5 to 3–4. §4 (Placement) does not mention e6's placement at all.
- **The §6 driver table is incomplete** (R3-10):
  - r4 ran `d9fbb130…`;
  - e1q, e6, e2–e4, e4b, ex2, ex3, r5, r6 and r6b ran `42147f56…`, the version with `replica_bounce`, `restart_wait` and `restart_check`;
  - the table has a single "v7 … e1q onward" row and does not list r4.
- **§7's foreign-load bullet** still cites `prof` (frame-pointer builds under `perf record`) as the quiet reproduction. `et`'s default cell and `e1q` are the right citations now.

## 4. Round-2 items

| # | Status | Verified |
|---|---|---|
| N1 (disk episode) | **Closed** | stated in RST §3 and methodology §7; disk reads now in the series (r4 and later have `r_ios`/`r_sect`) |
| N2 (2-minute "sustainable") | **Closed** | replaced by failure counts per rate, the 30-minute soaks and the forced resyncs |
| N3 (generator CPU) | **Closed** | `summarize_i8.py:340-352` uses the generator's `ps` utime+stime; `gen_cpus` is now 0.37–0.39 in every e7b run, against the bimodal 0.16/0.38 before |
| N4 (diag v2 evidence) | **Closed** | `raw/diag-patch/diag2-{h,p,px}.patch.txt`, `i8diag-v2.rs.txt`, `apply_diag-v2.py.txt`, `raw/builds/build-*-diag2.txt` |
| N5 (H's variance, placement) | **Closed** | e6 run: P/H 0.969, against e1q's 0.938 |
| N6 (writer contradiction) | **Partly** | now "not attributed", but see R3-4 |
| **N7** (stale summaries) | **Closed** | every `raw/*/summary.json` has `comparable`, `checks_failed` and `gen_cpus`; e5 now excludes 5 H runs (jeprof rep 3's H, with no resync, is comparable). I regenerated e7b, e1 and r2 with the current `tools/summarize_i8.py` (`5bf1ce12…`) from the unpacked runs: byte-identical to `raw/*/summary.md`. The rule now also catches disconnects and partial resyncs (`summarize_i8.py:355-361`) |
| **N8** (cycle metrics) | **Closed** | `cycles_i8.py` counts disconnects in [measure_start, load_end] and measures survival only from installs during the load. RRC draws conclusions only at 3,680–7,820 tx/s (11–19 disconnects per run); 2,300 is marked "too few for medians"; "I tracks P at every rate" is gone. (The implementation measures survival from the last install ≥ load_start, which may fall in the warm-up; this is harmless because the counted disconnects are inside the window.) |
| N9 (btrfs zstd) | **Closed** | stated in RST §4 and methodology §7 |
| N10 (tools) | **Closed** | `timeline_i8.py` reads gzipped series and blanks samples more than 1.5 s away |
| N11 | **Mostly closed** | RST l.19's "60 runs" is still unexplained: r2 + r3 hold 66, or 62 without the 4 quota runs |

## 5. Findings (round 3), ranked

**Medium (text: correct before the gate is signed)**

- **R3-1. Statements not updated after r6.**
  - RST §3.1 l.83: "With no further disconnect after 150 s in any of the three runs, an established link's failure rate at 1,840 tx/s is below about 2.4 per hour pooled … 6.5 per hour per build". r6's disconnect at +638 s, on an established link with a 27.8 MB buffer after a 2.72 s stall, is a counter-example reported 4 lines earlier.
  - RST §3.2 l.87: "none after 300 s". Same counter-example.
  - RELEASE-GATE §4 l.45 lists P's young-link chain under "established link".
  - **Fix:** recompute the established-link bound over r4, r6 and r6b: 1 failure in about 125 established-link minutes at 1,840 tx/s. Move the r4 chain to the young-link bullet.
- **R3-2. "The variable is the buffer's size, not the link's age" (RST l.92) overreaches.** As detailed in §1: every forced link starts small, failures occur only in the first seconds, and 8 of 9 links that stayed small survived. Size, age, AOF age and post-install backlog are confounded, and autotuning is driven by the replica's read rate. **Fix:** keep "supported, not established" and drop the size-versus-age claim. The causal test (`SO_RCVBUF` pinned) is the right one.
- **R3-3. Episodes are undefined, and forced and unforced failures are pooled** (RST l.15, "H 3, I 1, P 4"). **Fix:** define an episode, and report unforced episodes per hour (H 1, I 0, P 2) separately from failures per forced resync (2/8, 1/8, 2/8).
- **R3-4. The writer residual does appear in instrumented builds.** LTC §1 (l.16) and §5 (l.191) say it "does not appear in the instrumented builds". In `raw/e6/` (cell `c10k-repl-cpu5-w4-diag2`, diag v2 primaries), P's writers are 0.719 [0.663–0.963] of H's. Per hand-off, P is 4.01–4.21 against H's 5.40, 5.95 and 4.22. Only e7b shows equality. **Fix:** "absent in e7b; present in e6's diag cell".

**Low (numbers and wording)**

- **R3-5.** RST §3 table row 3,680–9,200: the correct values are H 13–21, I 16–23, P 21–23 disconnects per run (from the load's start), and intervals > 1 s of H 1–2, I 1–9, P 1–11.
- **R3-6.** "Four 30-minute soaks" should be five (RST l.79, RELEASE-GATE §4 l.45, Q12).
- **R3-7.** RELEASE-GATE Q2 "the same failure counts at every rate" and I8-C "the builds do not differ" should read "no difference detectable".
- **R3-8.** RELEASE-GATE Q3 calls e4b flat. Rep 2 rose 70.2 → 75.1 MiB (+1.04 MiB/min) and was still flattening at 300 s.
- **R3-9.** The r3 diag cell runs diag v1 on both nodes, with H's per-frame-timed replica. State this in methodology §5/§7 and RST §4, and do not compare H's stall or read-gap figures with I/P's.
- **R3-10.** Add the two missing driver hashes (`d9fbb130…` for r4; `42147f56…` for e1q onward, with bounces and the restart options) to methodology §6.
- **R3-11.** r6's numbers come from logs, not a run record. Reconstruct a `runs.jsonl` record from `load.txt`, the series and the logs (marked "reconstructed"), so that `stalls_i8.py` covers it. Otherwise footnote the manual entries in the 1,840 row.
- **R3-12.** LTC §5's "4 of the 6 production campaigns" omits e1d (H 4.89, P 4.26 per hand-off: −13%) and counts `prof` (frame-pointer builds). State the selection rule, or list every campaign.

**Nit**

- RRC l.35: H's largest `write(2)` accumulation is 1.68 s, not 1.48 s, and the range includes the invalid 9,200 tx/s rate.
- RST l.104: define link age consistently, from the install.
- RELEASE-GATE Q14: the stall range runs to 2.72 s.
- RST l.19: explain the "60 runs".
- Methodology §7: cite `et` and `e1q`, not `prof`.
- Methodology §4: describe e6's placement (replica on CPU 5, generator on 3–4).

## 6. Checked and sound in round 3

- **Comparability:** the new rule (any failed check; any disconnect, full or partial resync in a contention cell) is implemented and applied to every packed campaign, verified by regeneration (N7).
- **Generator CPU:** measured from cumulative `/proc/<pid>/stat` (N3).
- **e1q** (3 reps, foreign 0.39–0.43 CPU) repeats e1 within its ranges: P/H 0.938 transactions, 1.030 CPU, 1.081 instructions, 1.413 RSS.
- **e6:** the replica on its own core (CPU 5, generator on 3–4) isolates the placement. P/H 0.969 [0.966–0.970], instructions unchanged at 1.082; px 1.021.
- **e2/e3** (no replica; no writers) localize the cost to the replication path: e2 instructions 1.001; e3 0.957 transactions, 1.083 instructions.
- **The soaks and r5:**
  - r4, r5 and r6b accepted 1.000 of the offered rate.
  - Every primary and replica keyspace with a run record matched (r6's verification was lost with its record).
  - r5's six restarts from the replica's own AOF matched.
  - r4's restart checks failed (no PONG within 60 s on a 22 GB AOF); this is reported, not hidden.
- **The `r6` OOM is real:** a restart on about 11 GB of AOF reached 12.3 GB RSS (`raw/r6/FAILED.txt`). It is correctly carried as a pre-existing release risk. The benchmark host itself was disturbed by it: desktop helpers were killed and the driver got SIGTERM. That run's data stop at the restart, and r6b ran afterwards as a separate campaign.
- **Process control and provenance:** every new campaign records its driver's SHA-256 (`42147f56…`, `d9fbb130…`), and the replica binary is recorded per run.
