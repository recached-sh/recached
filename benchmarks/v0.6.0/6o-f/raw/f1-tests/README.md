# raw/f1-tests

| File | Build | Note |
|---|---|---|
| `suites-efa8d35/` | PR #172 head `efa8d35` | fmt, clippy, workspace, server suite `--include-ignored` (inline and coalesced) |
| `redis-diff.log`, `redis-diff-efa8d35.log` | `fe37fcd`, `efa8d35` | Redis 7.2.5 differential |
| `reproduce-on-fix-branch.log` | `ba5df43` | The 8 gated reproductions with `RECACHED_REPRODUCE_6OF_COUNTER=1` (failing on purpose) |
| `sigkill-restart-cross-version.log`, `redis-diff-upgrade-downgrade.md` | **`e55f7d5`, the rejected counter-frame fix (`rc-ttl`)** | Evidence for the rejected design only (`V0.6O-F-TTL-DESIGN.md` §2): the `PXAT` representation matched Redis on the fixed-window case and replayed under an older binary. **Not shipped behaviour** |
| `suites-fe37fcd/` | `fe37fcd` | **Partial**: fmt, clippy and the workspace suite (1,519 passed); stopped before the server suites when the review's changes came in. Superseded by `suites-efa8d35/` |
