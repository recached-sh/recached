# v0.6.0 Phase 6N-C5: replication fan-out efficiency

Start with the [replication profile](V0.6N-C5-REPLICATION-PROFILE.md). The reports for the optimization, the multi-worker coalescer, the validation and the release gate are added by the later 6N-C5 PRs.

| File | |
|---|---|
| [V0.6N-C5-REPLICATION-PROFILE.md](V0.6N-C5-REPLICATION-PROFILE.md) | What a connected replica costs the primary, and where it goes |
| [`run-c5.sh`](run-c5.sh), [`summarize-c5.py`](summarize-c5.py) | The kit. It is derived from `../6n-c4/run-m1.sh` and adds: several binaries per interleaved run, a user/system CPU split, `uprof` counters and samples, replication socket segments (`ss`), and replica CPU. |
| [`tools/uprof.c`](tools/uprof.c) | User-space `perf_event_open` counters and LBR call-stack sampling. It needs no `perf` and no privileges (`kernel.perf_event_paranoid=2`, one's own processes). Build: `gcc -O2 -Wall -o uprof tools/uprof.c` |
| [`tools/symbolize.py`](tools/symbolize.py) | Symbolizes `uprof record` samples: flat, inclusive, and `--breakdown` by context |
| `fedora-i5-9400f-20261120-profile/` | Counters and CPU, release binary `c61b920`, 3 repetitions |
| `fedora-i5-9400f-20261120-samples/` | Call-stack samples (xz) and their symbolized `profile.md`, symbolized build of `c61b920` |

**Reproduce:** run the following natively, on an idle host:

`RC_BINS="base=<recached-server>" UPROF=<uprof> CELLS="base/rc-coalesced-P1 …" RECORD_CELLS="…" bash run-c5.sh`

Call stacks need a build with symbols:

`CARGO_PROFILE_RELEASE_STRIP=none CARGO_PROFILE_RELEASE_DEBUG=line-tables-only RUSTFLAGS="-C force-frame-pointers=yes"`
