# Review E, round 1: performance and resources (Phase 6O-I, `dev` 0a1f95e, PR #200 617aedb/4d46dbc)

Independent reviewer. Read-only: no builds, tests or benchmarks were run (native benchmarks were running on the host). Every ratio below was recomputed from `raw/i5-perf/*/tx.log.txt` (paired per rep, script in this folder: `an.py`).

## Verdict: **APPROVED WITH CHANGES**

- **PR #200 code: correct.** It may merge after the nits. The byte-loop parser accepts what the old `str::parse` parser accepted, except a leading `+`. The marker table is exact. The 16-byte `GroupMark` saturation is safe against every reader of the frame's group.
- **The 6O-I performance gate is not ready to sign.** Three things are missing:
  - the headline cost: two-write transactions with AOF `everysec` lose 8–11% throughput, and still lose 7.7% with #200. The summary does not report it, and CPU does not explain it;
  - the ordinary-command plan does not exercise the paths 6O-I changed;
  - reader stalls on replicas are not quantified anywhere, and the planned stall script will probably lose its latency percentiles.

  H1–H3 and M1–M6 must be closed before the release gate.

## Findings

| ID | Severity | Finding | Evidence | Required change |
|---|---|---|---|---|
| H1 | High | **The `set-set` AOF `everysec` throughput loss is unexplained, and the summary leaves it out.** Throughput drops by more than CPU rises, and #200 fixes CPU much more than throughput. The draft's summary row gives only "+5–8% CPU". | Paired tx/s, i/base: campaign 1 0.909 [0.825–0.954]; remeasure 0.894 [0.875–0.983]. p/base: 0.923 [0.921–0.980]. All 15 reps are below 1.0. CPU/tx: i 1.080 [1.070–1.098] and 1.088; p 1.036 [1.025–1.044]. p/i: CPU 0.948, tx/s only 1.041. The server is only 0.70–0.75 CPU busy, so the cell is not CPU-bound. p50 rises uniformly: 513–533 µs base, 548–580 i, 524–540 p. After #200, the throughput loss (−7.7%) is about twice the CPU rise (+3.6%), and i and p write the same bytes. So the remaining loss is wall time on the write path, most likely on the HDD/btrfs file, which #200 cannot change | Put the tx/s ratios, with ranges, in the summary and in the gate as a maintainer acceptance item. 6O-G already took set-set `everysec` to 0.988, so the cumulative figure against 6O-F matters for the "AOF P=1" v0.6.0 gate. Isolate the cause: rerun set-set and wide20 `everysec` (base/i/p2) with the data directory on tmpfs, and on an SSD/ext4 if one is available. Record `write(2)`, `fdatasync` and bytes per write, and off-CPU time. State whether the residual follows the disk or the code |
| H2 | High | **"Ordinary commands unchanged by 6O-I" is asserted before any data, and the planned controls cannot test it.** Every replicated write pays the new code, grouped or not. `i-cmds.sh` runs only no-AOF, no-replica cells, where 6O-I changes nothing | `ReplFrame` grew from 16 to 48 B on `dev` (`Option<ReplGroup>` has no niche). With p2 it is 32 B. The growth applies to every backlog slot and every per-replica queue element. `fan_out(bytes)` is now `publish(vec![bytes])`: one allocation for a one-element `Vec`, plus a collected `Vec<ReplFrame>`, per write. 6O-H's `fan_out` made neither. `i-cmds.sh` cells: `rc-noaof-*` only | Add ordinary SET/INCR/LPUSH/MSET at P=1 (and SET at P=16) with a replica attached (primary CPU/op, primary and replica VmHWM), and SET/INCR/MSET at P=1 with AOF `everysec`, base vs p2, 5 reps. Until then, word §4 as "not measured". Optional: a single-frame fast path in `publish` that skips both `Vec`s |
| H3 | High | **Reader stalls on replicas are not quantified anywhere, and a group apply never yields.** `V0.6O-I-REPLICA-ATOMICITY.md` points to `V0.6O-I-PERFORMANCE.md` §7, which is still a placeholder | In the streaming loop, a whole group counts once towards `REPL_APPLY_YIELD_FRAMES` (64), and members `continue` without counting. `apply_transaction` is synchronous, so on a 1-worker replica every other connection waits for the full apply plus effects. On multi-worker replicas, readers of marked stripes block their worker threads (6O-G D2-6). Primary-side reference (6O-G): 10,000 SETs take about 7 ms, about 10 ms with observers. With the default 8 MiB queue, the largest group that reaches a replica is ≈ 40k+ small frames (tens of ms). With the queue and backlog raised to the 64 MiB bound it is up to 524,288 members (hundreds of ms). The planned `i-stall.sh` has four problems: (a) `timeout 18` kills `redis-benchmark -n 100000000`, which then most likely prints no latency summary, so the fallback records only an `rps=` line (verify on one run); (b) 1-worker replica only; (c) no reader baseline without transactions, no reader-throughput column, labels never rotated; (d) 3 reps | Measure replica reader throughput and p50/p99/max at TX_N 0, 100, 1,000 and 10,000, with 1 and 3 replica workers, labels rotated, 5 reps. Use a reader client that reports its own percentiles (for example txbench readers pointed at the replica port, or a fixed `-n` that finishes inside the window). Add one cell with groups near the default queue limit. Extend decision D5 (6O-G) explicitly to replicas, with these numbers |
| M1 | Medium | **The per-member attribution is only half confirmed.** The draft says the extra CPU "is the one-record check". #200's byte loop removes only about half of it | i − base per member: setn100 +51–62 ns, TX_N=1,000 +48 ns, TX_N=10,000 +58 ns (5/5 reps each). p − base, setn100: 1.030 [1.024–1.038], ≈ +28 ns per member still. wide20: p 1.021 [1.008–1.035]; set-set: +0.29 µs per transaction | Prove where the cost lies with a control build that compiles the check out, or with `perf stat` user instructions per transaction (as 6O-G did) for base/i/p2. Restate §2.2 as measured. Option for review A: make L6's invariant structural (an encoder-produced one-record type) instead of re-parsing every member |
| M2 | Medium | **The backlog memory cost is misreported.** It is per backlogged frame, grouped or not, and grows with the backlog setting. "+12–13 MiB" is the default 16 MiB backlog's figure, and campaign 1 itself had +6 MiB in 3 of 5 reps | Primary VmHWM delta, i − base: set-set +6.2–12.7 MiB (campaign 1), +11.8–12.7 (remeasure); setn100 +11.3–12.5 and +6.1–12.5. The deltas are bimodal, which fits a per-slot cost times a `VecDeque` capacity that doubles (≈ 256k frames of ~65 B sit at the 2^18 boundary). p (`Arc`): +8.0–8.9 (set-set), +3.2–4.1 (setn100). Model for p2: half of i's per-slot cost and no per-group allocation, so ≈ +3–6 MiB in both cells. That is better than p for 2-write groups but not for 100-write ones. Scaling: 256 MiB of 65-byte frames ≈ 4.1M slots ≈ +130 MiB (i) or +65 MiB (p2), before power-of-two rounding (up to 2×) | Report the cost as "+16 B per backlogged frame (p2)", with the default-backlog figure and one larger-backlog cell (for example 128 MiB, base vs p2) to confirm linear scaling. Use the r-campaign data for p2 on both cells. Optional design: keep group extents in a per-group side deque trimmed with the backlog (rule 2 needs only "is this a non-first member"; the sender needs count and bytes only at `first`), which makes the per-slot cost zero |
| M3 | Medium | **Replica-side costs are measured only at light load, and replica memory is never recorded.** | Replica CPU/tx for 2-write groups rose 11–12% in every rep (2.10→2.33; 1.97→2.21), and #200 leaves it unchanged (2.21). 100-write groups: −11–12%. The replica runs at ≈ 0.18 CPU, so its catch-up rate is unknown. `i-tx.sh` records only the primary's VmHWM; the replica has no AOF; there is no replica cell at TX_N ≥ 1,000. The design's ~450 MiB worst case per link (§8) is unmeasured | Measure: the replica's catch-up rate for small groups (a burst or a delayed attach); replica VmHWM in every replica cell; one group near the 64 MiB bound (queue and backlog raised) and one above it (refused from the header, then full sync), with replica HWM and time to resync; a replica with AOF `everysec` |
| M4 | Medium | **Primary `fan_out_group` holds the `std::sync::Mutex` stream lock for the whole group.** Other writers' fan-out (on any worker) blocks its thread for that time. Before 6O-I each frame took the lock separately | `publish`: for every frame, `Arc::new` + backlog push + trim, then for each replica a reservation and `try_send`, all in one hold. The REPLICA cells use 1 worker and TX_N ≤ 100, so the effect cannot appear in them | Run a 3-worker primary with a replica, 4 outside SET writers and looping TX_N=10,000: writer p99/max and transaction p50 vs base. Report the hold time per group (a bench-build histogram, or the measured publish ns/frame times count) |
| M5 | Medium | **"`write(2)` and `fdatasync` counts per transaction are unchanged" is asserted, not measured. The driver's AOF bytes are wrong.** | The i5 data contain no syscall or `aof_stats` counters. `aof_bytes=57` in every AOF cell: the FORMAT-only file left by the SIGTERM shutdown save. The report says "read 0" | Read INFO/`aof_stats` (writes, fsyncs, frames per batch) before the SIGTERM, as 6O-G did (0.10 `write(2)` per command), for base/p2 on set-set and setn100 `everysec` and `always`. Stat the AOF before the kill, or drop the column. Fix "read 0" to "read 57" |
| M6 | Medium | **The combined production path is not measured.** The REPLICA cells had no AOF on primary or replica, so a primary with both an AOF and a replica (the non-deferred `append_transaction` plus `fan_out_group`) never ran. Neither did `RECACHED_AOF_FANOUT=coalesced` | `i-tx.sh`: `REPLICA=1` cells have no `RECACHED_AOF_SYNC`. `on_transaction_logged` takes `append_transaction` whenever replication is enabled | Add set-set and setn100 with AOF `everysec` plus a replica, base vs p2. Add one coalesced set-set cell, since 6O-G reported coalesced separately (0.970) |
| L1 | Low | **AOF-cell throughput conclusions rest on noisy medians.** The summarizer reports a ratio of medians, with no paired ranges and no paired CPU ratios | io_psi10 reached 73%. wide20 i/base: 0.904 [0.768–1.048] in campaign 1, 0.991 [0.956–1.052] in remeasure, so the sign of the conclusion flips between campaigns. CPU/tx agrees in both (1.051–1.064; 1.041–1.059). The identical-path control (set-set, no AOF) reads CPU 0.994 and 1.021, so the floor is ±2%. "Non-grouped ... within 1%" is not supported: mset10 1.015 [1.001–1.025], mget-mset 1.021 | `summarize_tx.py`: print the paired median and range for tx/s and CPU/tx (6O-G's convention). Lead with CPU/tx for AOF cells. Report the ±2% floor, and say "within noise", not "within 1%" |
| L2 | Low | **`foreign_cpus` is not only foreign load.** It is /proc/stat busy time minus task schedstat, so it includes the benchmark's own softirq and loopback work (6O-G: it "tracks throughput"). The draft blames "the editor, background agents". The logs have no per-cell timestamps, and the 1-minute load gate (< 3.0) is mostly the cell's own load | campaign 1 and remeasure set-set: about 0.9–1.3 for every label; low-rate cells 0.14 | Reword. Add a UTC timestamp per cell. Optionally record softirq separately |
| L3 | Low | **Recovery time is unmeasured.** Grouped 2-write files are +87% bytes and have twice the records, and pass 1 adds a check per record | `aof-bytes.txt`: 130.9 → 244.9 B/tx, 2 → 4 records per transaction. `i-replay.sh` is queued, not run | Replay time and peak RSS on files with an equal transaction count (set-set, setn100), base vs p2, plus records/s and MB/s |
| L4 | Low | **p (`Arc` extent) cost primary CPU against i in the setn100 replica cell.** The likely cause is refcount traffic on every frame clone (the backlog and each replica queue). p2's `Copy` mark should remove it, but only RSS has been planned for confirmation | p/i CPU/tx 1.022 [1.012–1.028], 5/5 reps (i was 0.980 vs base; p 1.002) | Confirm CPU, not only RSS, from the r-campaign (p2 vs i vs base) |
| N1 | Nit | If `GroupMark::new` returns `None` (`first == 0`), `publish`'s `and_then` silently sends the group ungrouped, so a replica would apply it non-atomically, and nothing is logged. This cannot happen today (offsets start at 1) | `replication.rs` `publish` | `expect` or `debug_assert!` plus an `error!`, as `group_markers` does |
| N2 | Nit | Saturation is safe only while `REPL_GROUP_MAX_BYTES < u32::MAX`, and a unit test is what enforces that. A group over 4 GiB now declares 4,294,967,295 bytes on the wire. A future replica with a larger bound would fail with "longer than its header says": the same full sync, under a misleading message | `group_mark_tests` | `const _: () = assert!(REPL_GROUP_MAX_BYTES < u32::MAX as usize);`. Note the saturation in design §6.2/§8 |
| N3 | Nit | Design §8 says the extent costs 24 B per frame. It is 32 B on `dev` (`Option<ReplGroup>`) and 16 B with #200 | `size_of` reasoning; #200's own test asserts 16 | Update §8 |
| N4 | Nit | `meta.txt` `size=14/19` is the length of the symlink, not of the binary (`stat` without `-L`) | meta.txt; hashes are correct | `stat -L` |
| N5 | Nit | Design §10 estimated "+160%" bytes for a two-SET transaction. The measured figure is +87% (32-byte values) | `aof-bytes.txt` | Cite the measured figure |
| N6 | Nit | `is_one_record` now refuses `+N`. `str::parse::<usize>` accepted it. That is stricter and harmless: such a member falls back to ungrouped with an error, and the debug assert tightens the same way | diff 0a1f95e..617aedb | Add a differential test against the old implementation on random or mutated frames |

## What I verified

**Provenance.** The binaries' sha256 values in `meta.txt` match the labelled host binaries:
- `rc-6oh-6068337` a3922e87;
- `rc-6oi-0a1f95e` f835dfb6;
- `rc-6oi-perf-617aedb` 6de543ad;
- `rc-6oi-perf-4d46dbc` b472a0e1.

base's hash is 6O-H's `comb2` (built from 53a2f1e). `git diff 53a2f1e 6068337` outside benchmarks touches only two test files, so the two are code-identical.

**Summaries.** `summary.md` medians match the raw logs. The per-rep pairing is by rep number, and labels are adjacent in time within each cell (rotated every rep; with 3 labels the rotation is slightly unbalanced over 5 reps, which is acceptable).

**Consistency across reps.** CPU/tx is above base in 5/5 reps for every grouped AOF cell, in both campaigns, for i and for p. Non-grouped and no-AOF cells stay within the ±2% floor.

**The byte-loop parser against the old one.** Old: a `windows(2)` search for CRLF, then `from_utf8` and `parse::<usize>`. New: digits up to `\r`, then CRLF required. The two return the same result for:
- empty digits;
- non-digits, including `-`;
- a bare CR;
- a missing CRLF or end of input;
- overflow (`checked_mul`/`checked_add` against the parse error);
- leading zeros.

The only difference is a leading `+`, now refused. Neither version can index out of bounds: the new one uses `get`, and in the old one `at ≤ len` held at every call. The bulk walk is unchanged. Cost is now O(digits) per element instead of a window scan.

**The prebuilt table.** Index `count − 2` covers 2..=64 (63 entries); `checked_sub` returns `None` for 0 and 1, and `get` for counts of 65 and above. `control_frame` is deterministic (`checkpoint_frame(id)`), and authorship re-encoding happens later in `push_frame`, so sharing the static bytes is safe. `Cow::Borrowed` means no allocation, and the test compares every entry plus the 65 and 70 fallbacks.

**`GroupMark`.**
- `Option<GroupMark>` is 16 B (the `NonZeroU64` niche).
- The frame's group is read in exactly three places: the sender's header (`first == offset`), rule 2 in `resume_frames` (`first < offset`), and a test accessor. All three go through `.group()`.
- On the replica, a saturated header is refused by the `bytes + 128 × count > 64 MiB` check before anything is buffered.
- For every group under 4 GiB the per-member `received > bytes` and final `received != bytes` checks are unaffected.

**Per-replica queue charge.** It still covers the element. `QueuedReplFrame` is 64 B on `dev` and 48 B with p2, plus the `Arc` chunk, which fits inside the 128 B per-frame charge.

**The primary AOF path.** `group_markers` runs before the queue lock (review B F4). One-frame transactions return before the check, which explains why get-set, mset10 and mget-mset stay flat.

## What must still be measured (before the gate)

1. **The p2 (4d46dbc) replica rerun.** Primary CPU/tx and VmHWM for set-set and setn100, plus replica CPU and replica VmHWM. It must confirm that L4's CPU loss is gone and M2's predicted +3–6 MiB.
2. **Ordinary commands on the changed paths** (H2): SET/INCR/LPUSH/MSET with a replica attached, and with AOF `everysec`, base vs p2, CPU/op and RSS.
3. **The set-set `everysec` throughput cause** (H1): tmpfs (and SSD) data directory; `write(2)`/`fdatasync` counts and bytes per write; off-CPU time.
4. **Replica reader stalls** (H3): reader throughput and p50/p99/max at TX_N 0/100/1,000/10,000, with 1 and 3 replica workers, 5 rotated reps. Fix `i-stall.sh`'s latency capture first.
5. **Large groups and slow replicas** (M3, M4): a group near the 64 MiB bound and one above it (replica HWM, refusal, resync time); replica catch-up rate; a 3-worker primary with a replica and TX_N=10,000 plus outside writers (writer p99/max, stream-lock hold).
6. **The combined AOF plus replica path, and coalesced fan-out** (M6).
7. **Recovery time** (L3).
8. **Attribution control** (M1): a build with the check compiled out, or instruction counts.

## Smaller report corrections

- §1 "Ordinary commands unchanged" → "not measured" until item 2.
- §1 grouped `everysec`: add the throughput (i 0.89–0.91, p 0.92).
- "+12–13 MiB" → "+6–13 MiB, per backlogged frame".
- §9 "read 0" → "read 57".
- "Non-grouped ... within 1%" → "within the ±2% noise floor".
