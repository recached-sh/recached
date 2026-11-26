# v0.6.0 Phase 6O-I: durable transaction atomicity, AOF framing, replica atomic application

Transaction atomicity across three boundaries:

1. **Primary runtime:** readers never see part of an `EXEC` (6O-G, unchanged).
2. **AOF recovery:** an incomplete transaction is never replayed as a partial write.
3. **Capable replicas:** a transaction group becomes visible to readers all at once, and resynchronization never resumes inside one.

| PR | Branch | Content |
|---|---|---|
| #195 | `design/v0.6o-i-transaction-framing` | protocol design (approved), baseline reproductions, old-binary evidence |
| #196 | `feat/v0.6o-i-aof-framing` | `FORMAT(expirylog, txn)`, `TXN-BEGIN`/`TXN-COMMIT`, recovery classification |
| #197 | `feat/v0.6o-i-replication-framing` | `caps=txn1`, consecutive group offsets, group headers, the replica's assembler |
| #198 | `feat/v0.6o-i-replica-atomic-apply` | groups applied through `KeyValueStore::apply_transaction` |
| #199 | `fix/v0.6o-i-resync-boundaries` | no partial resync inside a group |
| #200 | `perf/v0.6o-i-group-check` | cheaper framing and the backlog's group side table (measured in PR 6; review E approved) |
| PR 6 | `bench/v0.6o-i-validation` | these reports, tools and raw evidence |

## Reports

| Report | |
|---|---|
| `V0.6O-I-BASELINE.md` | `dev` `6068337`; both defects reproduced |
| `V0.6O-I-PROTOCOL-DESIGN.md` | the approved protocol (revision 2.1) |
| `V0.6O-I-AOF-FRAMING.md` | the writer and the replay |
| `V0.6O-I-CRASH-RECOVERY.md` | outcomes, crash points, `fsync` policies |
| `V0.6O-I-REPLICATION-PROTOCOL.md` | `txn1`, the wire, the assembler |
| `V0.6O-I-REPLICA-ATOMICITY.md` | atomic application, readers, failure |
| `V0.6O-I-RESYNC-BOUNDARIES.md` | offsets, rule 2, the worked example, chains |
| `V0.6O-I-COMPATIBILITY.md` | older binaries in every direction |
| `V0.6O-I-CORRECTNESS-TESTS.md` | tests, the Redis 7.2.5 differential, fault injection, mutants |
| `V0.6O-I-INDEPENDENT-REVIEW.md` | reviews A–E |
| `V0.6O-I-PERFORMANCE.md` | native measurements |
| `V0.6O-I-RELEASE-GATE.md` | gates I1–I7, decisions, the engineering assessment |

## Tools (`tools/`)

| Tool | Does |
|---|---|
| `txn_compat.py` | real binaries on crafted group files |
| `corpus_compat.py` | every write command from each older binary, replayed on 6O-I |
| `repl_compat.py` | replication between real processes of every pairing |
| `redis_tx_diff.py` | the transaction scenarios against Redis 7.2.5, with AOF restarts |
| `aof_bytes.py` | AOF bytes and records per transaction |
| `replay_time.py` | AOF recovery time |
| `summarize_tx.py` | summaries of the transaction campaigns (paired per-rep ratios, AOF counters, replica RSS) |
| `summarize_ord.py` | summaries of the ordinary-command runs |
| `summarize_stall.py` | summaries of the replica reader-stall runs |
| `bench/` | the campaign drivers: `i-tx.sh` and the scripts that ran it (`i-campaign.sh`, `p-campaign.sh`, `r-campaign.sh`, `e-tx*.sh`), ordinary commands (`i-ord.sh`), reader stalls (`i-stall2.sh`), recovery (`e-replay*.sh`), and the sequencing (`e-chain.sh`) |

## Raw evidence (`raw/`)

`i0-baseline/` reproductions · `i1-compat/` old-binary checks and T4 files · `i1-review/` review A · `i2-review/` review B · `i3-review/` review D · `i4-review/` review C · `i5-perf/` measurements · `i5-review/` review E · `i6-compat/` compatibility with the 6O-I binary · `i7-correctness/` the Redis differential · `mutation/` mutants. Logs are stored as `.txt` and AOF files as `.resp` (`*.log` and `*.aof` are ignored).
