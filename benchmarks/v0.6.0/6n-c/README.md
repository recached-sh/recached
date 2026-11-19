# v0.6.0 Phase 6N-C: correctness remediation, coalesced inline AOF, Gate M1

**Outcome:**
- **Correctness:** all five pre-existing defects found in 6N-B are fixed (PRs #128–#132), plus two found by the integration review (#133). Each fix was independently reviewed and CI is green.
- **Option D:** merged as an opt-in mode (#134, `RECACHED_AOF_FANOUT=coalesced`). The default stays `inline`.
- **Gate M1: FAIL.** 0.73× standalone at P=1, 0.25× at P=16. At P=16 `coalesced` is slower than `inline`.
- **Decision: performance investigation required.** Option A was not started.
- **v0.6.0 is not release-ready.**

Start with [the release gate](V0.6N-C-RELEASE-GATE.md).

| Document | Contents |
|---|---|
| [V0.6N-C-RELEASE-GATE.md](V0.6N-C-RELEASE-GATE.md) | Status, the integration gate, the §10 engineering review, the decision, remaining v0.6.0 blockers |
| [V0.6N-C-CORRECTNESS-REMEDIATION.md](V0.6N-C-CORRECTNESS-REMEDIATION.md) | PRs 0a–0e and #133 |
| [V0.6N-C-DEDUP-DURABILITY.md](V0.6N-C-DEDUP-DURABILITY.md) | DEDUP acknowledgement and recovery guarantees |
| [V0.6N-C-OPTION-D-IMPLEMENTATION.md](V0.6N-C-OPTION-D-IMPLEMENTATION.md) | Option D: the change, configuration, preserved properties, tests |
| [V0.6N-C-INDEPENDENT-SECURITY-REVIEW.md](V0.6N-C-INDEPENDENT-SECURITY-REVIEW.md) | Every review finding and its outcome; remaining risks |
| [V0.6N-C-M1-PERFORMANCE.md](V0.6N-C-M1-PERFORMANCE.md) | Gate M1 on the Fedora host: environment, results, ratios, why P=16 regressed |
| [V0.6N-C-TARGETED-RELIABILITY.md](V0.6N-C-TARGETED-RELIABILITY.md) | What was validated, and what was not |

## Reproducing M1

On native Linux, on an idle host, with the 6L Redis and Valkey builds in `~/recached-6l-bench/bin`:

```sh
cargo build --release --locked -p recached --bin recached-server
RC_SHA=$(git rev-parse HEAD) RC_BIN=target/release/recached-server \
  RC_SHA256=$(sha256sum target/release/recached-server | cut -d' ' -f1) \
  bash benchmarks/v0.6.0/6n-c/run-m1.sh
```

The script refuses to run inside Flatpak or when the 1-minute load average is above 1.0. It writes its results to `~/recached-6nc-bench/results-<utc>/`, ending with `summary.md`.
