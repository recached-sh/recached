# Review C, round 1: Phase 6O-I8 replica capacity (Investigation 2)

Reviewer: Review C (replica saturation). I wrote none of the tooling, analysis or reports under review.

Method: read-only, because the `r4` soak is running. No builds, tests, servers or benchmarks were run. I used small Python scripts over `benchmarks/v0.6.0/6o-i8/raw/{r1,r2,r3}` (`runs.jsonl`, `series.jsonl.gz`, the server logs and the diag `I8DIAG` lines) and read the code in `src-{h,i,p}`. The scripts are in the session scratchpad and are not part of the evidence.

Reports under review:
- `V0.6O-I8-REPLICA-SUSTAINABLE-THROUGHPUT.md` (RST)
- `V0.6O-I8-REPLICA-RESYNC-ROOT-CAUSE.md` (RRC)

## Verdict

- **RST: NOT APPROVED as written.** The headline "sustainable rate 1,840 tx/s" and the quantitative mechanism ("about 0.5 s of buffering; a longer stall overflows") are contradicted by the packed data (F1, F2). The soak meant to settle the rate (`r4`) cannot meet the report's own definition as configured (F3). "No repeated full sync below the sustainable rate" was never exercised (F4).
- **RRC: APPROVED WITH CHANGES.** The causal chain holds and I could not break it:
  - a blocked AOF `write(2)` starves the link reader in the same task;
  - the primary's byte budget fires;
  - the 2 s reconnect wait overruns the backlog;
  - overload makes cycles, and 6O-I cycles faster because it drains its read-ahead faster.

  Its convergence and "no partial group" wording needs narrowing (F5, F6).

The qualitative conclusions survive:
- the replica's own AOF on this HDD/btrfs is what saturates first;
- P does not lose capacity against H.

What does not survive is a sharp sustainable rate.

## Findings

### F1 (High): the "≈ 0.5 s of buffering" mechanism is contradicted by the diag data; buffering is 1–2+ s and depends on a non-default host sysctl

**Evidence.** `raw/r3/*-raof-2300-diag-*`: I8DIAG `r_group_apply`, `r_apply_run` and `r_read_gap` maxima per second, against the production `recached_aof_write_usec`/`write_calls` deltas and the primary's `send_queue_bytes_peak` and `queue_disconnects` in `series.jsonl.gz`.

**Stalls of 1 s or more that did not disconnect:**

| Run | Stall | Primary queue peak during it |
|---|---|---|
| H rep 1 | 1.20 s and 1.13 s | at most 5.7 MB |
| H rep 2 | 1.46 s | 122 KB |
| I rep 2 | 0.74 s, 1.47 s and 1.08 s, about 4 s of blocked writes within 6 s, including a 2.37 s `fdatasync` | 162 KB |

- **The 8 MiB queue is not where the data went.** At 18 MB/s, a 1.46 s block is about 26 MB of stream. The primary's sent-frame rate stayed at 230,000/s throughout, so the socket buffers absorbed it.
- **The host's TCP buffer is raised.** `/proc/sys/net/ipv4/tcp_rmem` = `4096 131072 33554432`: a 32 MiB receive-buffer maximum, against the 6 MiB kernel default.
- **Stalls without a disconnect are also in production runs:**
  - `r2/2-raof-1380-i` t=+100.8: `write(2)` time 1,524 ms in one second, no disconnect;
  - `r1/1-raof-10-i` t=+37.8: 1,272 ms, no disconnect.
- **Shorter stalls did disconnect:**
  - P rep 1 diag lost its link on a 0.69 s stall at t=−7.2 s (warm-up);
  - `r1/1-raof-10-h` lost its link at 920 tx/s on a stall of 1.9 s or more.

**What is wrong.** Three passages are false as stated:
- RST §1 and §4 ("a stall longer than that overflows the queue");
- RRC §4 ("hold about 0.5 s of stream at 2,300 tx/s").

The primary's queue is only about 0.19 s at 2,300 tx/s (43,000 frames of 195 B charged, about 3.4 MB on the wire). The rest is TCP buffering, which varies with autotuning (smaller early in a connection: see F4) and with `tcp_rmem`. The sustainable rate measured here is therefore a property of this host's sysctl as well as of its disk.

**Fix.**
- Sample `ss -tmi` for the replication socket on both ends at 1 Hz (`rcv_space`, `skmem` `rb`/`r`/`tb`/`t`, `Recv-Q`/`Send-Q`) in the next campaign.
- Record `tcp_rmem`/`tcp_wmem` in `meta.txt` and the methodology.
- Rerun one cell (2,070 or 2,300 tx/s, 3 reps) with `tcp_rmem` max at the default 6 MiB, or with `SO_RCVBUF` pinned.
- Restate the mechanism: "a disconnect when a stall exceeds the buffering, ≈ (8 MiB queue + socket buffers)/rate, which was 0.2 s to more than 1.5 s here".

### F2 (High): there is no rate boundary in the data; failures are stochastic from 920 to 2,300 tx/s, and 1,840's clean runs had no long stall to survive

**Evidence.** The same scans as F1, pooled over every HDD run:

| Rate (tx/s) | H failed | I failed | P failed |
|---|---|---|---|
| 920 (r1) | 1/1 | 0/1 | 0/1 |
| 1,380–1,840 (r1, r2) | 0/12 | 0/12 | 0/12 |
| 2,070 (r2) | 2/3 | 2/3 | 1/3 |
| 2,300 (r1 ladder) | 1/1 | 1/1 | 1/1 |
| 2,300 (r3 diag, HDD, 2 × 120 s) | 0/2 | 0/2 | 1/2, in warm-up |

- Pooled over 2,070–2,300, every build failed 3 of 6 runs.
- The largest excess `write(2)` second per run is below 0.75 s in all 9 r2 runs at 1,840. Stalls of 1.2–1.9 s occurred at 920 (r1 H, I) and at 1,380 (r2 rep 2 I).

**What is wrong.**
- RST §1 "2,300 tx/s failed in every build's ladder run" and §4 "on the HDD every build fails [at 2,300]": in the same campaign as the tmpfs control, 5 of 6 HDD runs at 2,300 sustained. So the tmpfs-2300 against HDD-2300 contrast does not discriminate within r3. Only the 9,200 contrast does.
- RST §1 "1,840 tx/s for H, I and P alike" ignores H's failure at 920.
- 27/27 at 1,840 and below reflects an absence of long stalls in those 18 minutes, not survival of them.

**Fix.**
- Replace "sustainable rate 1,840" with failures per hour against rate, with confidence bounds, pooling r1, r2 and the r3 diag cell. Zero failures in 9 × 135 s bounds the rate below ≈ 8.9 per hour at 95%.
- Report the stall-length distribution per rate: seconds with `write(2)` time ≥ 0.5, 1 and 1.5 s.
- State 2,300 as marginal, like 2,070.

### F3 (High): `r4` as configured cannot satisfy the report's definition; say now what it must show

**Evidence.** `cfg/r4.json` (1 rep × 1,800 s at 1,840 tx/s; labels run h, i, p in sequence) and the current driver `i8run.py` (`d9fbb130…`): `start_server` waits 240 × 0.25 s for `PONG`, and the restart check uses it.

- **AOF size.** The replica's AOF will be about 1,840 × 1,815 s × 100 × ≈ 70 B ≈ 23 GB of logical bytes. The bench sets `RECACHED_SAVE_INTERVAL=0`, so the AOF is never checkpointed.
- **Replay time.** r2 restarts replayed about 1.9 GB in 22–23 s (≈ 85 MB/s), and r3's 6.66 GB AOF already failed with "no PONG". r4's restart will need several minutes, so its "restarted from its own AOF" check will fail on the driver's timeout, not on data.
- **Production default not tested.** The production default is autosave every 900 s (`main.rs:450–461`). The soak therefore never runs a checkpoint (snapshot write and `fdatasync`, then AOF truncation) under load. That is a likely extra stall source on this disk.
- **Statistical power.** One run per build, in sequence, gives no rotation. 0 failures in 30 min bounds a build's failure rate below 6 per hour (95%), or below 2 per hour if the three builds are pooled.

**What r4 must show** for RST's claim to stand:
1. zero disconnects and full syncs after the prefill for the whole 1,815 s, warm-up included;
2. lag bounded, with a stated threshold;
3. RSS slope ≈ 0 over 30 min on both servers;
4. the distribution of replica `write(2)` stalls, with their maximum against the buffer (F1);
5. whether the mean and tail of `fdatasync` drift as the AOF grows to tens of GB;
6. catch-up after the load, and exact primary and replica keyspaces;
7. a restart check that actually completes.

**Fix.**
- Before r4's verification phase (or in a rerun), raise the restart wait for long cells, for example to 15 min, or make it proportional to the AOF size.
- Record the AOF size before the kill. `data_bytes` is taken after the restart has rewritten it: 57 B in every r2 run.
- Report the bound, not "sustained".
- Add a soak with the default autosave.

### F4 (High): "no repeated full sync below the sustainable rate" was never tested, and the data point to the post-(re)connect period as the weak one

**Evidence.**
- **The definition never syncs under load.** Every run's only sync before the load is the initial one, at zero load, before the prefill (RRC §5). At 1,840 or below, only `r1/1-raof-10-h` (920 tx/s, one sample) took a full sync under load; it then recovered.
- **Failures cluster at the start.** All 11 r2 disconnects happened in the first 21 s of the 135 s load, 8 of them in the warm-up: −12.2, −7.2, +0.8 (H rep 1); −13.2, −9.2, −4.2, +0.8, +5.8 (I rep 1); +5.8 (I rep 2); −6.2 (P rep 2); +5.8 (H rep 2).
- **Back-to-back full syncs.** `r2/1-raof-2070-h` took 3 in a row 2.6–5 s apart, and I rep 1 took 5. P's only r3 diag failure was in the warm-up too.
- **A candidate explanation.** Each new link restarts TCP autotuning (F1) and each install writes a checkpoint to the same disk. If so, recovery from one full sync is exactly what is fragile.

**Fix.** Add an "attach and resync under load" cell at 1,840 (and at 1,380): ≥ 5 trials per build, counting full syncs within 60 s after the first. Ways to induce the first full sync under load:
- start the replica mid-load;
- `SIGSTOP` the replica for 3 s;
- or kill the replication connection.

Until then, RRC §1's "no stuck sync loop below the sustainable rate (§3)" is unsupported.

### F5 (Medium): "every run converged" mixes stream catch-up with convergence by a post-load full sync

**Evidence.** `r1` replica logs:
- 12 of the 15 runs at 3,680 tx/s and above completed a full-sync install after the measured window: 0.1–10.5 s after it, with two in `raof-100-i` and `-p`.
- `catchup_s` in those runs is the time for that install, not for streaming.
- The restart check then replays an AOF truncated at that install, so it tests the snapshot, not the grouped AOF.

**What is wrong.** Three statements overstate what was measured, for the overloaded runs:
- RRC §1: "every build converges, in 0.25–5.5 s, at every rate up to the primary's limit";
- RST §1: "60 runs … reached the primary's last offset";
- CORRECTNESS §3: "no divergence even with 13–23 full syncs".

What those runs show is that a snapshot of a quiescent keyspace installs correctly. The sustained r2 runs are genuine stream catch-ups (≤ 0.25 s, no full sync).

**Fix.**
- Classify every run's convergence as "by stream" or "by a full sync after the load".
- Report the AOF bytes the restart replayed.
- Note the catch-up range inconsistency: RST says 0–6 s, RRC 0.25–5.5 s, and the measured r1 maximum is 6.01 s.

### F6 (Medium): "no partial group applied" cannot be observed by these benchmarks

**Evidence.** `i8bench verify` compares only the final state. In the overloaded runs every full sync replaces the keyspace, and later transactions overwrite most keys. A transiently applied half-group is therefore invisible.

The positive evidence is:
- the code: `apply_repl_group` applies all members, or on failure resets the stream and full-syncs;
- the absence of protocol, "replicated command failed", "panicked" or mid-group-resume lines (I checked: none in any r1/r2/r3 primary or replica log);
- `tests::replica_atomicity_6oi`.

**Fix.**
- Say which evidence supports the claim and that the benchmark cannot observe it.
- Optionally add an online probe. Each transaction's last member sets `commit:<c>` = seq. A replica-side reader then checks, in one atomic read, that no member key of connection *c* carries a seq above `commit:<c>`. First confirm that a replica `MGET` is atomic with respect to a group apply (it must take the stripes).

### F7 (Medium): the stall is in `write(2)` (confirmed); that it waits on `fdatasync` is inferred

**Confirmed (I tried to break it and could not):**
- **The write is inline on the worker thread.** `lead()` performs the batch with `std::fs::File` on the async task (`persistence.rs` ~1325–1370). `flush()` runs `sync_data` on the blocking pool without holding `busy` (~2372).
- **The reader shares the applier's task.** It is a `select!` branch in the applier's task (`replication.rs` ~2665–2740), so it cannot run while that task's thread is blocked.
- **The diag gap matches the group apply.** `r_read_gap` equals `r_group_apply` to within 0.1 ms in every diag stall.
- **The production counters show a blocked write.** In the stall second, `write(2)` time ≈ the stall (700 ms of 693; 1,248 for 1,202; 1,502 for 1,460), write calls drop by 85–99%, and replica CPU falls to 66–102 ms.
- **Blocked, not preempted.** Voluntary context switches fall, from about 2,200/s to 28–520, and non-voluntary ones do not rise.
- **Alternatives ruled out:**
  - CPU-1 sharing and Tokio scheduling;
  - `lock_commands` (an async wait would let the reader run);
  - `apply_transaction` (CPU-bound, but CPU is low in the stall seconds);
  - checkpoints (autosave is off, and none ran outside full syncs).

**Inferred:** that the blocker is the same file's `fdatasync`. The evidence is co-occurrence (0.8–2.4 s fsyncs complete in the same or next second) plus known btrfs inode-lock behaviour. It is not shown that the cause is not a btrfs transaction commit, extent allocation or writeback.

**Fix.**
- Add one HDD cell with `RECACHED_AOF_SYNC=no`. If the stalls vanish, the attribution is proven.
- Add an off-CPU stack of the worker thread during a stall: `/proc/<tid>/stack` sampled at 10–20 Hz, or `offcputime`.
- Ideally, check one rate on ext4 or xfs.

**Also.** The coupling is the shared task, not only the single worker: a multi-worker replica would still starve its reader. This matters for any remediation, such as a separate reader task or a `write(2)` off the worker.

### F8 (Medium): "not CPU" and "the build does not change it" understate H's lack of headroom

**Evidence.**
- At 1,840–2,300 tx/s H's replica uses 0.58–0.76 CPU, against 0.22–0.26 for I/P.
- After the 1.46 s stall in r3 H rep 2, H's replica ran at 0.91–0.99 CPU for about 4 s. Its un-acked lag rose from 0.2 M to 1.12 M frames before it drained. P needs about a third of the CPU for the same catch-up. A second stall inside H's catch-up window compounds; for P it mostly does not.

**What is wrong.** RST §3, "the sustainable rate does not depend on the build because what limits it is the disk's stalls", is a hypothesis. The data cannot resolve it: pooled 2,070–2,300 failures are 3/6 for every build. If anything, P should tolerate stalls better.

**Fix.** Write "indistinguishable at n = 6 runs per build at 2,070–2,300", and keep RST §1's "No" (P does not lose capacity) on the basis of CPU headroom plus the equal counts.

### F9 (Medium): the diag table omits the event that decided the cell

**Evidence.** RST §4 lists P's longest stalls as 376/347 and 640/547 ms, the measured window only. The cell's only disconnect followed a 693 ms stall at t = −7.2 s, in the warm-up. The table also does not say that 5 of 6 diag runs sustained (F2).

**Fix.** Include the warm-up, and list per run: the disconnects and the longest stall before each.

### F10 (Medium): the tmpfs-9200 claim needs its limits stated

**Evidence:** `r3` replica logs and `runs.jsonl`.
- The quota hit at +91.7, +93.1 and +94.4 s of the 120 s window in I rep 1, I rep 2 and P rep 2 (P rep 1 also hit it).
- All 4 I/P runs fail the restart check ("no PONG").
- Rep 1 I and P then looped through 109 and 102 full syncs, each checkpoint failing on quota.
- H was compared at a higher accepted rate (8,636–8,637 tx/s, against 7,984–8,327), because its primary did less fan-out while H was disconnected.

**Fix.**
- RST §1 and §4 should say "streamed about 92 s at 7,980–8,330 tx/s without a resync, then hit the quota; crash recovery not verified".
- For review D (release risk): a replica whose post-sync checkpoint keeps failing retries a full sync about every 2 s with no back-off (214 failed checkpoints in r3), each costing the primary a snapshot. It logs one WARN and one ERROR per failed AOF append (865,301 pairs in r3), with no rate limit.

### F11 (Low): the "every resync is full" arithmetic needs one more term

The 2 s wait alone exceeds the 16 MiB backlog above about 1,250 tx/s (16 MiB / 67 B ≈ 250,000 frames). In-flight socket data (up to about 32 MiB here, F1) can exceed it by itself. RRC §1 is right in effect.

**Fix.** State the condition: partial resync needs backlog ≥ queue + socket buffers + (notice + wait) × rate. Without it, a remediation that only shortens the wait would not give partial resyncs.

### F12 (Low): "its frequency grows with the write rate" is not shown between 920 and 2,070

Stalls of 1 s or more were seen at 920 and 1,380 tx/s and none at 1,840 (F2). In the overloaded rates the counts include the full syncs' own checkpoint I/O.

**Fix.** Give per-rate stall counts from the production counters, excluding the seconds around installs.

### F13 (Low): the definition has no thresholds

"Lag stays bounded" and "CPU and RSS bounded" have no numbers. The largest sustained lag, 0.26 M frames, is 1.4 s of stream.

**Fix.** Give thresholds (for example lag < 0.5 M frames and recovered within 10 s; RSS slope < 0.1 MiB/min over ≥ 10 min).

### F14 (Low): the diag v1 replica in H's diag cell

H-diag v1's per-frame replica timers cost little here: 330 µs/tx, against production H's 300–314 at 2,070 and 314–407 in r2; replica CPU 0.76. They do not distort the stall lengths, which the production AOF counters corroborate. They do bring H closer to CPU saturation during catch-up (F8). H's "applier stall" is a 64-frame run, a different unit from I/P's group apply; that is harmless for the maxima.

**Fix.** Note both.

### F15 (Nit)

- "H's replica would run out of CPU near 2,400 tx/s" uses r1's noisy 403–423 µs. r2's 300–407 µs gives 2,500–3,300.
- "Blocks 0.35–1.5 s": 0.35 is the smallest per-run maximum, not a floor.
- RRC §3 and §4 still say "Pending" although r2 and r3 are done.
- METHODOLOGY §4 says data are "never on tmpfs", while r3 deliberately uses tmpfs.
- RRC §2's "the same effect" as campaign C: 90 s counts of 15/23/21 at 9,200 scale to 3.3/5.1/4.7 per 20 s; say so.

## Tried to disprove, could not

- **The stall location.** It is a blocked `write(2)` on the worker that runs the link reader (F7 evidence). It is not Tokio scheduling, CPU-1 sharing, the stripe lock, the apply itself or a checkpoint.
- **The disconnect cause.** Every disconnect is the primary's byte budget: 305/305 in r1, 11/11 in r2 and 45/45 in r3 logged "Replica byte queue full". There are 0 "channel full" lines, 0 partial resyncs and 0 "resume inside a transaction group" lines. The replicas logged no protocol or apply errors.
- **The expected keyspace.** Each connection owns disjoint keys and sends its transactions in sequence. The last acknowledged `(seq, member)` per key is recorded in member order, so a key picked twice in one transaction ends at its later member. Failed transactions are not recorded, which would surface as a mismatch: fail-safe.
- **Verification.**
  - `verify` reads all 100,000 values.
  - The restart is standalone (no `RECACHED_REPLICAOF`) and reads the replica's own snapshot and AOF.
  - Every r1/r2 replica had `dbsize` 100,000 and `ttl_set` 0 post hoc, although driver v2/v4 did not gate on them.
  - The 4 failing tmpfs-9200 runs are correctly marked not comparable.
- **I against P.** The replica streaming path is identical in I and P (diffed). I's and P's run-to-run spread in `cycles.md` (for example notice 0.60 against 1.32 s at 5,060) is the noise floor. H's longer notice (1.3–4.5 s) and shorter survival (0.61–0.76 s, against 0.82–1.29 s for I/P) lie outside it at every overloaded rate, 8 of 8 comparisons. The cycle arithmetic (survival + notice + 2 s + install) reproduces the full-sync counts per 90 s. The "faster cycles, not lower capacity" explanation stands, as does P's per-transaction CPU (0.34–0.40× H's).
- **The controls.** Without a replica AOF, every build's replica kept up at the primary's maximum (about 8,100 tx/s), with 0 resyncs in 6/6 runs.
- **Memory and queues.** The bounds hold by design:
  - the primary's queue: 8 MiB charged plus the frame cap;
  - the read-ahead and the chunk queue: 64 + 64 MiB;
  - the group buffer: 64 MiB charged, with a frame cap.

  The measured RSS stayed flat within the 120 s windows. Primary RSS 57–88 MiB; replica RSS up to about 150 MiB in overload, alike across builds. Boundedness over time still needs r4 (F3).
- **The 2,070 confound** (r2 rep 1, 04:08–04:21Z) is handled correctly now. Rep 2 had one failure per build on a quiet disk, rep 3 none.
