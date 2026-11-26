# v0.6.0 Phase 6O-H: logged expirations, timeless AOF replay, replication correctness

Implements `6o-f/V0.6O-F-TTL-DESIGN.md`, Expiry Option E.

| Report | Contents |
|---|---|
| `V0.6O-H-PROTOCOL-DESIGN.md` | The AOF marker and authorship rules, replay, file boundaries, replication capability (PR 1) |

Further reports are added by the implementation PRs and the validation PR.

| Raw | Contents |
|---|---|
| `raw/h0-marker-compat/` | `tools/marker_compat.py` run against v0.4.0, v0.5.1, v0.5.2, `410235b`, `f9a364d` |
| `raw/h0-baseline/` | The 6O-F reproductions (`RECACHED_REPRODUCE_6OF_COUNTER=1`) on `f9a364d`: 8 fail, 3 runs of 3 |
| `raw/h0-review-c/` | Review C's own reproductions (replica AOF after a full sync, shutdown race, ephemeral seed and discard, array records, fake primary handshakes) |
