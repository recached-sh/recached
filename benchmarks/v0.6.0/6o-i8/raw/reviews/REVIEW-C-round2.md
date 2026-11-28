# Review C, round 2: Phase 6O-I8 replica capacity (Investigation 2)

Reviewer: Review C.

Method: read-only, because campaigns e1q and e6 are running. Nothing was built or run. I used small Python scripts over the files:
- `raw/r4`, `raw/r5` (including `ss.txt.gz`), `raw/r1`–`r3`;
- `tools/stalls_i8.py`, `tools/bounces_i8.py`, `tools/ssmon.sh`;
- the server logs.

The scripts are in the session scratchpad, not in the evidence.

Reports:
- `V0.6O-I8-REPLICA-SUSTAINABLE-THROUGHPUT.md` (RST, rewritten)
- `V0.6O-I8-REPLICA-RESYNC-ROOT-CAUSE.md` (RRC)
- the round-1 dispositions in `V0.6O-I8-INDEPENDENT-REVIEW.md`

## Verdict

- **RST: APPROVED WITH CHANGES.** The gating changes are text only: N1, N2 and N3.

  The rewrite answers round 1's core objections. There is no sharp rate. The report gives failure rates with intervals, the stall distribution, the host's socket settings, and convergence classified by stream or by full sync. `r5` is a good test of resynchronization under load.

  But three statements in §1 and §3 go beyond the data:
  - an age-based immunity ("once a link has run for five minutes …");
  - "no disconnect at 1,380–1,840 in 2-minute runs";
  - "no loop".

  The rate table also silently drops warm-up and `r5` failures.

  No new run is required if TCP autotuning stays labelled a supported hypothesis. If the report wants it as the mechanism, it needs the causal test in N1.
- **RRC: APPROVED WITH CHANGES.** The causal chain stands, as in round 1. Its §1 still carries three statements the RST has since corrected: "converges at every rate", "no partial group applied" and "no stuck sync loop below the sustainable rate" (N8).

## Round-1 findings

| # | Status | What remains |
|---|---|---|
| F1 buffering | **Partly** | Restated with `tcp_rmem`/`tcp_wmem`. The young-link figure is still decomposed wrongly (N6). RRC §4 says "1–2 s" with no young-link caveat. A default-`tcp_rmem` run needs root; the non-root substitute is in N1 |
| F2 no boundary | **Partly** | The curve and intervals are there. The table counts the measured window only and leaves out `r5` (N2) |
| F3 soak | **Partly** | The r4 restart failed on the timeout as predicted, and the report says so. `r5` uses autosave 900 s, but autosave never fired under load, so a periodic checkpoint under load is still untested (N5). `r4` has no socket data |
| F4 resync under load | **Partly** | `r5` is the right experiment. "No loop" ignores r4 P's chain of 5 full syncs (N3) |
| F5 convergence classes | Closed in RST; **open in RRC** §1 | |
| F6 partial-group observability | Closed in RST; **open in RRC** §1 | |
| F7 `write(2)` confirmed, `fdatasync` inferred | Closed | |
| F8 H headroom | Closed | |
| F9 P's 693 ms warm-up stall | Closed | |
| F10 tmpfs-9200 limits | Closed | |
| F11 resync arithmetic | Closed | RRC now includes the socket term |
| F12 stalls per rate | Closed | |
| F13 thresholds | Closed | The definition is now binary: zero disconnects, lag back to 0 in-stream |
| F14 | Closed | |
| F15 nits | Partly | "Blocks 0.35–1.5 s" (RST §1, RRC §4) although r4 shows 2.3 s; see N8 |

## New findings

### N1 (High): the autotuning explanation is supported in direction and rough size, not established; and the "link age" framing is wrong in kind

**What supports it.** In `r5`, I took the replica-side socket (local ephemeral port, peer `port+2`), as `bounces_i8.py` does, and matched its `rb` to the replica's per-second AOF `write(2)` time and the primary's queue-full events:

| Group | Stall | Receive buffer (`rb`) | Link age | Outcome |
|---|---|---|---|---|
| The 5 failures after a forced resync | 0.64, 0.63, 1.49, 0.93, 0.78 s | 1.08, 1.49, 2.37, 0.99, 1.25 MB | 3.6–31 s | disconnected |
| Survivors on grown buffers | I 1.54 s | 26.4 MB | 5 s | survived |
| | I 1.20 s | 22.2 MB | | survived |
| | H 1.23 s | 8.9 MB | | survived |
| Long-lived links late in `r4` (no `ss` there, so buffer size inferred) | up to 2.28 s (I) | | | survived |

So, roughly, the threshold is about 0.6–0.9 s with `rb` ≤ 2.5 MB and above 1.5 s with `rb` ≥ 20 MB. That fits the buffer arithmetic.

**What it does not establish:**
- **Small-buffer survivals overlap the failures.** Links with `rb` ≤ 2.5 MB rode through stalls of 0.42–0.75 s; H at 1,380 rode a 0.75 s stall at age 75 s with 1.45 MB. Failures came at 0.63–0.64 s. The 1 Hz stall proxy is too coarse to separate them.
- **Link age is confounded with AOF age.** Every new link is followed by an install that truncates the AOF and writes a snapshot. The initial link starts on a fresh AOF.
- **The disk is busier early in every run.** In the first 160 s of each r4 run, disk writes were 2.0–2.9 MB/s against 1.6 later, and I/O pressure was 8.5–10.9 against 6.3–9.2. That held in H, I and P alike, including I, which never resynced.
- **No socket data for long-lived links.** `r4` has no `ss`, so "a long-running link is large" is inferred for exactly the 76 minutes that carry the "0 after 300 s" claim.

**The variable is the buffer size, not the age:**
- **Some buffers never grow.** In `r5`, 4 of about 29 links stayed at or below 2.5 MB for at least 100 s: H-1840's last link, I-1380's last link, and two of P-1380's.
- **Growth seems to follow backlog, not age.** Links whose Recv-Q filled grew; quiet links did not.
- **A grown link failed.** H-1840 failed unforced at load +105 s with `rb` 22.6 MB on a 1.75 s stall. RST §3.2 cites this very link as "a long-running replica link measured 22.6 MB" without saying it failed.

**What is wrong in the text:**
- RST §1, "Once a link has run for five minutes, no build disconnected at ≤ 2,300 tx/s in 76 minutes". All of that exposure is the three `r4` links at 1,840 tx/s, so the 95% bound is below 2.4 per hour, at 1,840 only, with no buffer data.
- RST §3.2, "concentrated in the first minute of a link". It is concentrated on links with small buffers, and those can be old.

**Fix (text):**
- Re-bin the failures by link age (from the install times) and by `rb`, not by time since the load started.
- State the >300 s bound at 1,840 only.
- Call autotuning "the candidate mechanism, consistent with `r5`'s socket data". Name the AOF-age confound.

**Fix (if the mechanism is to be asserted), one causal test, no root needed:**
- (a) Pin `SO_RCVBUF` on the replica's socket. `rmem_max` is 4 MiB, and pinning disables autotuning. Prediction: the failure threshold no longer depends on link age.
- (b) Or build a diag replica whose link reader runs on its own worker or thread, or whose AOF write goes to `spawn_blocking`. Prediction: young-link failures vanish at 1,840, because the 64 MiB chunk queue keeps filling during a stall.

  This is also the remediation candidate.
- Sample `ss` in every future cell.

### N2 (High): the failure-rate table drops warm-up and `r5` failures, which misstates the 1,840 result and hides P's failures at 2,070 and 2,300

**Evidence.** `tools/stalls_i8.py` counts disconnects only inside `measure_start..measure_end`. It runs over r1–r4 only. The warm-up carries the same offered load, and RST §3.2's early-link table counts from load start.

**What drops out:**
- 8 of r2's 11 disconnects;
- P's only failure at 2,070 (r2 rep 2, −6.2 s), so the table shows P "0 [0–37]";
- P's only failure at 2,300 in r3 (diag rep 1, −5.2 s);
- 3 of I's 4 failures at 2,300 in r1;
- r4 P's first failure (+13.9 s).

**`r5` before the first bounce** gives about 6 × 135 s of unforced exposure at 1,380 and 1,840. In it, H at 1,840 disconnected at +105 s.

RST §1, "at 1,380–1,840 tx/s no disconnect in any build's 2-minute runs", is therefore false as written.

**Fix.**
- Count from load start, or give both windows.
- Add `r5`'s unforced windows to §3.
- Update §1, "Does P lose capacity" and the 2,070 sentence.

### N3 (High): repeated full syncs occurred at 1,840 tx/s; "no loop" generalizes from `r5` alone

**Evidence:** r4 P's logs. In 154 s P took 5 full syncs; 4 consecutive new links failed before one held:

| Link | Connected | Ended | Notes |
|---|---|---|---|
| 1 (initial) | | +13.9 s | 1.15 s stall |
| 2 | +16.4 s | +21.1 s | broke **during its install**: the install took 6.6 s, the longest of 489 installs across r1–r5 (next 3.6 s), so the 64 MiB read-ahead (≈ 4.6 s at 1,840) filled first |
| 3 | | +86.8 s | age 61 s, 0.58 s stall |
| 4 | | +118.0 s | age 29 s, 1.02 s stall |
| 5 | | +149.8 s | age 29.5 s, 1.77 s stall |
| 6 | | held | |

- **Much worse than `r5`.** `r5`'s rate is ≈ 5/24 failures per new link. Four failures in a row at that rate is ≈ 0.002, so this episode ran under worse conditions than `r5` sampled.
- **A second failure mode.** A link can die during its install when the checkpoint outlasts the read-ahead. Neither report describes this.

**What is wrong:**
- RST §1's "Repeated full syncs" row ("No loop … the next full sync held") rests on `r5` only.
- RRC §1 says "no stuck sync loop below the sustainable rate".

**Fix.** State the observed maximum: "a chain of 4 consecutive failures of new links (r4 P, 1,840 tx/s), then held; in `r5`, at most one". Describe the install-time failure. Do not claim a bound on chain length.

### N4 (Medium): "no build difference" holds only as "not detectable"; the strongest support is the code, not the counts

**Counts of independent episodes** at 1,380–1,840 tx/s, from load start, over r2, r4 and r5 (forced and unforced):

| Build | Episodes | Made up of |
|---|---|---|
| H | 3 | r5-1840 unforced; r5-1380 after two forced resyncs |
| I | 1 | |
| P | 3 | the r4 chain; r5-1840 after two forced resyncs |

These cannot exclude a 3–5× difference.

**The code argument:**
- I and P share the replica path.
- I diffed the I→P primary code. It changes only the backlog's group bookkeeping and the frame container. The wire behaviour is unchanged.
- So treating P and I as one process is justified a priori, and it should be stated that way.

**Two qualifications:**
- r4 ran H, I and P in sequence without rotation, so P ran last.
- P's per-hour figure in r4, "8 [2–20]", is a Poisson interval over clustered events: each failure creates a young link that fails more easily. Report episodes or chains, not a rate.

### N5 (Medium): autosave never fired under load in `r5`, and mid-load crash recovery is never checked

- **Autosave.** The 900 s interval is longer than both the 120 s between bounces and the 600 s load. In `r5` the only checkpoints under load were the full-sync installs. RST §3.1's "`r5` runs the production autosave" implies more than was tested. A periodic checkpoint under load is still unmeasured.
- **Mid-load recovery.** The 24 mid-load SIGKILL restarts replayed 0.6–1.5 GB of AOF with no torn-tail or group warnings, which is good. But each replayed state was overwritten by the full sync that followed, so the recovered state was never verified.

**Fix:**
- State the autosave limit, or run one cell with `RECACHED_SAVE_INTERVAL` of 60–120 s.
- Optionally verify the restarted replica standalone, against a per-connection prefix of acknowledged transactions, before it reconnects.

### N6 (Low): young-link buffering is decomposed wrongly

RST §3.2 and §3.3 say the buffering on a young link is "the primary's 8 MiB queue, about 0.6 s". The queue is charged at 67 + 128 B per frame: 43,000 frames, 430 transactions, about 0.23 s at 1,840 and 0.31 s at 1,380 tx/s. The rest of a young link's ≈ 0.6 s is:
- the primary's send buffer (`tb` 4.19 MB in every `ss` sample);
- the young receive buffer.

### N7 (Low): limits of the socket data

- Sampling began about 71 s into the first `r5` run's load, so H-1840's initial link was unobserved before that.
- The "at start" value in `bounces.md` is the first 1 Hz sample.
- `rb` is a limit, not occupancy: the Recv-Q peaked at 16.5 MB inside a 22.6 MB buffer just before H's failure.
- There is no socket data in r1–r4.

### N8 (Low/Nit)

- RRC §1 still says:
  - "every build converges, in 0.25–5.5 s, at every rate";
  - "no partial group applied";
  - "no stuck sync loop below the sustainable rate".

  Apply the RST's F5, F6 and N3 wording.
- RRC §4: add the young-link ≈ 0.6 s.
- "Blocks 0.35–1.5 s": r4 reached 2.3 s.
- RST §1's Partial group row ("361 disconnects, all full syncs") should now include r4 and r5.
- The "measured min" column rounds 1.5 min to 1 or 2.
- RST §3.1 says P had "4 disconnects in 30 min" but lists 5 events (+14 s is warm-up). Say "4 in the window, 5 in the load".

## Before I would approve outright

Text only:
1. **N1:** buffer size, not age; the >300 s bound stated at 1,840 only; autotuning as a candidate with its confound.
2. **N2:** rates counted from load start, with `r5`'s unforced windows; §1 corrected.
3. **N3:** the r4 P chain and the install-time failure; "no loop" removed.
4. **N4:** the power statement and the code argument.
5. **N5:** the autosave limit.
6. **N8:** RRC §1 wording.

Optional, only if autotuning is to be asserted as the mechanism: the `SO_RCVBUF` pin, or the reader-thread diag ablation (N1).

## Tried to disprove, could not

- **The tool reads the right socket.** `bounces_i8.py` takes the replica's socket (peer = replication port), not the primary's, whose `rb` would be the acknowledgement direction and meaningless here.
- **`r5` results.** Every forced resync installed within 0.5–1.0 s of `PONG`. All 6 runs matched exactly, including a meaningful restart that replayed 8–16 s of AOF. No replay warnings appeared.
- **Primary-side wire differences between I and P.** None that matter (diffed).
- **`r4` H and I.** 0 disconnects in 30 min. RSS flat (slopes 0.02 and 0.07 MiB/min). Exact keyspaces. They rode through stalls of 1.56 s (H) and 2.28 s (I) late in the run.
- **An outside reader in r4.** Disk reads stayed at or below 0.46 MB/s throughout.
