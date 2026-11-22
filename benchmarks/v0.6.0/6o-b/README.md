# v0.6.0 Phase 6O-B: the no-persistence request path

Verdict and scorecard: [V0.6O-B-RELEASE-GATE.md](V0.6O-B-RELEASE-GATE.md).

| Report | Content |
|---|---|
| [V0.6O-B-BASELINE.md](V0.6O-B-BASELINE.md) | The no-persistence matrix on `dev` `51ae15c` against standalone Redis 7.2.5 and Valkey 9.1.0: SET, LPUSH, SADD, MSET × 1/50 clients × P=1/16, and GET/INCR/ZADD controls |
| [V0.6O-B-CPU-PROFILE.md](V0.6O-B-CPU-PROFILE.md) | Where a command's time goes: the server CPU's whole budget (task + inline softirq), syscalls per command, client wake-ups, user-space samples per command, one client vs 50, in-process layers vs the network |
| [V0.6O-B-OPTIMIZATION-DESIGN.md](V0.6O-B-OPTIMIZATION-DESIGN.md) | Ranked candidates with their measured cost, the prototype, and what was rejected |
| [V0.6O-B-IMPLEMENTATION.md](V0.6O-B-IMPLEMENTATION.md) | The changes (PR 2, PR 3), rationale, rollback |
| [V0.6O-B-CORRECTNESS-TESTS.md](V0.6O-B-CORRECTNESS-TESTS.md) | New tests, suites in both fan-out modes, Redis differential, mutants |
| [V0.6O-B-INDEPENDENT-REVIEW.md](V0.6O-B-INDEPENDENT-REVIEW.md) | Findings and resolutions |
| [V0.6O-B-PERFORMANCE.md](V0.6O-B-PERFORMANCE.md) | Before and after on the native host (V1 matrix, V3 precision re-run), multiple workers, the post-change profile, AOF carryover, uncertainty |
| [V0.6O-B-RELEASE-GATE.md](V0.6O-B-RELEASE-GATE.md) | B1–B5, the fourteen questions, the v0.6.0 scorecard, blockers, follow-ups, next milestone |

| Kit and tools | |
|---|---|
| [`run-6ob.sh`](run-6ob.sh), [`summarize-6ob.py`](summarize-6ob.py) | The kit, derived from `../6o-a/run-6oa.sh`. It adds a clients dimension; the 6L battery's commands in redis-benchmark's own shapes; exact on-CPU time (schedstat); the server CPU's softirq and idle time (`/proc/stat`), TCP segments and the generator's context switches; and optional syscall counts |
| [`tools/sysc.c`](tools/sysc.c), [`tools/sysc_read.py`](tools/sysc_read.py) | An `LD_PRELOAD` shim that counts a server's I/O syscalls and the time inside them, with no strace, perf or bpftrace on the host. Diagnostic cells only |
| [`tools/tcpcost.c`](tools/tcpcost.c) | `send`/`write` × `recv`/`read` on one loopback TCP pair, with no wake-ups: whether the syscall kind matters (it does not) |
| [`tools/rusage.py`](tools/rusage.py) | The generator's rusage (voluntary context switches) |
| [`focus-6ob.args`](focus-6ob.args) | Focus set for `../6n-c5/tools/symbolize.py` |
| [`b0.sh`](b0.sh), [`v.sh`](v.sh), [`v2.sh`](v2.sh), [`v3.sh`](v3.sh) | Drivers that wait for a quiet host |
| [`tools/decomp.py`](tools/decomp.py) | The server CPU's per-op budget from a `results.txt`: task, softirq, idle, generator sleeps |
| [`mutation/mutate-6ob.py`](mutation/mutate-6ob.py) | Mutants for PR 2 and PR 3 |

Reproduce on a quiet native Linux host (the server on CPU 0, redis-benchmark on CPUs 3–5):

```sh
RC_BINS="base=<rc-base> new=<rc-new>" RC_SHAS="base=<sha> new=<sha>" ALTERNATE=1 REPS=3 \
CELLS="base/rc-noaof-set-c50-P1 new/rc-noaof-set-c50-P1 redis-noaof-set-c50-P1" \
  bash benchmarks/v0.6.0/6o-b/run-6ob.sh
```

Syscall counts: build `tools/sysc.c` (`gcc -O2 -shared -fPIC -o sysc.so sysc.c -ldl`) and pass `SYSC_SO=<sysc.so> SYSC_CELLS="<cells>"`. Samples: `UPROF=<uprof> RECORD_CELLS="<cells>"` with a profiling build, then `../6n-c5/tools/symbolize.py <samples> <binary> --ops N --cycles C --breakdown $(cat focus-6ob.args)`.

| Raw data (`fedora-i5-9400f-20261122-*`) | |
|---|---|
| `b0/` | The baseline matrix, `51ae15c` |
| `b1-samples/`, `diag-cpu0/`, `diag-sysc/`, `diag-proto/`, `diag-tcpcost/`, `layered/` | The profile's samples and diagnostics |
| `ab/` | The A/B of PR 2, PR 3 and both, before merge |
| `v1/` | The validation matrix, `51ae15c` vs `d5563d7`, Redis, Valkey, 3 reps; the drivers' log |
| `v2-mw/`, `v3-mw/` | Three workers; `v3-mw` adds the `RECACHED_REPLY_COALESCE=off` control |
| `v3-precision/` | LPUSH and MSET at c50 P=1 with ~30 s cells |
| `v3-samples/` | User-space samples of the merged build |
