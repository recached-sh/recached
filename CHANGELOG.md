# Changelog

All notable changes to Recached are documented here.

<<<<<<< Updated upstream
## [0.5.0] (unreleased) — synchronization reliability
=======
## 0.5.0 (2026-10-25) synchronization reliability
>>>>>>> Stashed changes

### Fixed — lost, duplicated or misreported writes
- Two browser tabs sharing one IndexedDB no longer lose writes: each tab adopted the same stored identity with a newer epoch, so the server skipped the older tab's writes as duplicates. Every page load now has its own identity, and IndexedDB rows are keyed per session so tabs cannot overwrite each other's pending writes
- Duplicate suppression survives a server crash: the AOF records each `DEDUP` write inside its envelope (and a mark-only record for writes that changed nothing), startup restores the marks, and a checkpoint writes the dedup sidecar before the snapshot. A retried `INCR` whose reply was lost is no longer applied twice after a restart
- A deferred write can no longer be overtaken: with `CLIENT DEDUPBARRIER ON` (sent by 0.5 clients) the server defers every later write from the client on that socket, so its replay is not answered `+DUP` and dropped
- A permanently refused write whose reply was lost is refused again on replay instead of answered `+DUP` and retired as applied; the server remembers the last 64 refusals per client (AOF and a new `.dedup-refusals` sidecar)
- A replayed write that was applied under an earlier sync token is answered `+DUP` rather than refused under a narrower one
- Writes a browser queues after `subscribe()` are accepted: the WebSocket transport (RESP3) no longer applies the RESP2 subscribe-mode rule, which refused them for good
- Duplicate-suppressed binary writes (`setBytes`, mobile binary `set`) are accepted: the parser no longer checks the whole `DEDUP` envelope against the envelope's own value positions
- A client pipelining a long outbox replay is no longer disconnected (`4001`, "sync stream gap") by its own writes' echoes lagging the fan-out
- Restoring the browser outbox while the socket is already open sends the restored writes at once; held for the next reconnect, they replayed behind newer writes and were skipped as duplicates
- An array reply shaped like a `qstate` or `keychange` (e.g. `RPOP list 3`) is no longer applied as one; with `CLIENT PUSHFRAMES ON` live-query changes arrive as RESP3 pushes
- Wire ids no longer wrap within an epoch, and an exhausted epoch takes a new identity instead of saturating into reuse

### Changed
- **Breaking (beta):** a write past the outbox cap is refused (`OutboxFullError`, `RecachedException.OutboxFull`, `RecachedError.outboxFull`, `Error::OutboxFull`) before it changes anything, instead of evicting the oldest queued write. The old behaviour is opt-in (`whenFull: 'evict-oldest'`, `evictOldestWhenFull`, `evict_oldest_when_full`). A browser cache with no server configured keeps evicting
- **Breaking (beta):** browser write methods throw `OutboxFullError`; `RefusedWrite` and refusal callbacks carry `unreconciled` keys
- A mobile write whose SQLite transaction fails is rolled back in memory and never sent: an error now means nothing happened
- A reconnect snapshot no longer rolls back a write still queued (deferred, or sent after the snapshot's `QSUB`)
- Browser IndexedDB writes resolve on transaction commit; a write's WAL record and outbox row commit in one transaction; server-applied state is persisted for offline cold start (`persistSyncedState`); WAL expiries are absolute, so a reload no longer restarts a TTL
- The sync-client's `enqueue_write` is replaced by `try_enqueue_write`; `restore_outbox` takes whether the socket is open

### Added
- A synchronization status in every SDK — `Disconnected → Connecting → Authenticating → Replaying → Hydrating → Ready`, plus `Degraded` — driven by protocol events, with the current problem, `lastSyncedAt`, and per-live-query states (`status()`/`onStatusChange` in the browser, `syncStatus` in Kotlin and Swift, `status()`/`status_changes()` in Rust)
- Browser `flush()`, `onPersistenceError`, `onWriteRefused`, and `outbox` / `persistSyncedState` options
- Server metrics `recached_dedup_replays_total`, `recached_sync_write_refusals_total`, `recached_dedup_barrier_deferrals_total`, `recached_qsub_snapshots_total`, `recached_qsub_snapshot_duration_seconds`
- Model-based randomized sync tests, real-process crash-recovery tests, and a browser end-to-end script (`wasm-edge/scripts/e2e-browser.mjs`)

### Upgrade notes
- A 0.5 AOF cannot be replayed by 0.4: run `SAVE` before downgrading. Snapshot and `.dedup` formats are unchanged
- Replication still carries bare mutations, so a promoted replica has no duplicate-suppression marks for replicated writes
- Writes after `subscribe()` and binary writes need a 0.5 server; the other client fixes work against 0.4
- Duplicate suppression is not exactly-once; see the protocol page's crash-consistency table

## [0.4.0] (2026-10-11)

- Added `recached-mobile`, the shared UniFFI core for the preview Kotlin/Android and Swift/iOS/macOS SDKs
- Added SQLite persistence for native client data, queued writes, and client identity, including offline cold-start reads and replay after an app restart
- Added Android native-library and Apple XCFramework packaging, generated Kotlin/Swift bindings, checksums, and mobile CI checks
- Added changed-key reporting and per-key snapshots for native persistence and reactive observers
- Reduced unrelated browser, React, and Vue updates with key- and pattern-specific change notifications
- Fixed reconnect snapshots temporarily removing or reverting offline edits by replaying queued writes before re-subscribing
- Kept retryable server-refused writes queued and reported permanent refusals to native clients
- Preserved server expiry in watched client values, including offline reads and native persisted state
- Fixed Kotlin binding error-field compatibility, custom build-output paths in mobile packaging, and Apple smoke checks
- Expanded mobile unit and live-server tests for persistence, replay, reconnect recovery, and expiry
- Added native SDK guides and a client support matrix

## [0.3.5] (2026-10-09)

- Added read-only sync scopes and scope-denial metrics
- **Breaking (beta):** `SYNC` replies now use `r=`/`rw=` grant notation
- Added ephemeral set memberships and improved multi-tab presence
- Added compact sync deltas and fixed mutation delivery
- Fixed command validation, collection cleanup, and replication correctness
- Improved capacity limits, eviction, and concurrent memory accounting
- Reduced CPU and memory overhead across commands, notifications, and outbox acknowledgments

## [0.3.4] (2026-09-13)

- Fixed concurrency, transactions, and expiry and eviction propagation
- Improved command throughput and reduced collection memory usage
- Hardened persistence, memory limits, and configuration validation
- Added partial replica resync and improved browser sync recovery
- Bounded queues, WebSocket messages, and keyspace maintenance
- Fixed authorization, protocol handling, and React subscription lifecycle issues
- Expanded metrics, tests, benchmarks, and operational documentation

## [0.3.3] (2026-09-05)

- Added configurable worker threads and repeatable scaling and comparison benchmarks
- Fixed `GETSET` atomicity and Docker workspace builds
- Hardened collection-count parsing against hangs, data loss, and excessive allocation
- Improved sync recovery, malformed-frame handling, expiry propagation, and snapshot reconciliation
- Added the native `recached-embed` client and session command support
- Added pinned toolchains, dependency audits, CI security gates, and workspace lint policy
- Corrected concurrency, benchmark, and embedded-client documentation

## [0.3.2] (2026-08-07)

- Fixed `incr` and `decr` in the browser SDK
- Fixed React JSON and byte hooks that could crash component trees
- Added TypeScript test suites for the browser, React, and Vue packages
- Added missing package notices and release verification

## [0.3.1] (2026-08-06)

- Fixed npm packaging so the published browser package can be imported and type-checked
- Added package verification to the release workflow
- Documented standalone browser use without a Recached server
- Corrected framework examples, peer dependency floors, and TypeScript configuration

## [0.3.0] (2026-08-03)

- Added `MEMORY USAGE`, Pub/Sub inspection commands, `MODULE LIST`, and cluster status in `INFO`
- Added configurable RESP and WebSocket ports
- Matched Redis behavior for unsupported cluster and sharded Pub/Sub commands
- Fixed metrics disabling, TTL rounding, expiry persistence, and TTL preservation during increments
- Allowed `PUBLISH` in transactions and stopped `EXEC` after queueing errors
- Aligned binary naming, packaging, and command documentation

## [0.2.4] (2026-08-02)

- Added common client compatibility commands and bounded collection scan commands
- Made replication opt-in and strengthened authentication, throttling, TLS, and identity checks
- Added WebSocket origin controls and handshake deadlines
- Restricted persistence-file permissions and bounded parser and glob inputs
- Fixed AOF synchronization and removed glob-matcher allocation
- Aligned `SCAN COUNT` validation and protocol documentation with Redis behavior

## [0.2.3] (2026-08-01)

- Added section-aware `INFO` output
- Reduced duplicate keyspace work in metrics sampling
- Updated repository and package metadata for the project move

## [0.2.2] (2026-07-20)

- Added connection-scoped `ESET` keys, RESP3 negotiation, capacity metrics, and replication acknowledgements
- Added browser outbox visibility and configurable per-connection limits
- Improved sync payloads for collections, `FLUSHDB`, reconnection jitter, and bounded rate-limiter state
- Made values byte-transparent across storage and transport paths
- Fixed Pub/Sub delivery and framing across RESP and WebSocket clients
- Persisted deduplication state and protected browser data during WAL compaction
- Added replication support for connection-scoped writes

## [0.2.1] (2026-07-19)

- Fixed critical browser runtime and persistence failures
- Hardened pattern matching, TLS configuration, and IP allowlist validation
- Fixed browser outbox hydration and resource-borrowing errors
- Changed the project license to Apache-2.0
- Added production, operations, troubleshooting, security, and use-case documentation
- Corrected package metadata, technical claims, command coverage, and benchmarks
- Expanded test coverage and added CI coverage gates

## [0.2.0] (2026-07-11)

- Extracted the platform-neutral `sync-client` state machine from the browser adapter
- Added offline writes, durable outbox replay, reconnection, and deduplicated delivery
- Added native JSON commands and merge support
- Added scoped live queries for browser, React, and Vue clients
- Added sync authorization and built-in sliding-window rate limiting

## [0.1.8] (2026-07-09)

- Improved random set operations with indexed storage
- Reduced write-path synchronization, allocation, metrics, and serialization overhead
- Added reproducible throughput benchmarks
- Added version output and architecture-aware Homebrew packaging

## [0.1.7] (2026-06-12)

- Hardened authentication, replication frames, RESP parsing, and glob matching
- Fixed WebSocket command buffering, transaction watching, AOF replay, and replication relay
- Corrected random set operations, sorted-set validation, TTL overflow, and key limits
- Implemented access-based LRU eviction and incremental `SCAN`
- Fixed React and Vue subscription behavior and repeated WebSocket connections
- Added configurable listener binding and TCP `WATCH` support

## [0.1.6] (2026-05-11)

- Fixed TTL races, expired-key deletion counts, and `ZADD` conditions
- Closed an authentication bypass in direct store execution
- Fixed Pub/Sub resource leaks and blocking locks in async handlers
- Added key-length validation and shared sorted-set score formatting

## [0.1.5] (2026-05-10)

- Added React and Vue integration packages
- Added RESP3 push frames for mutation and Pub/Sub events
- Added browser mutation and message callbacks
- Added bounded replication queues and conditional autosave
- Added browser WAL compaction
- Added timeout-based replica promotion, which is deprecated as of 0.3.4

## [0.1.4] (2026-05-09)

- Added snapshot save, background save, restore, autosave, and shutdown persistence
- Added append-only file persistence and configurable synchronization
- Added primary-replica synchronization and read-only replica mode
- Added configurable connection limits

## [0.1.3] (2026-05-09)

- Added IndexedDB write-ahead logging and browser cache restoration
- Added persistence clearing for sign-out and reset flows
- Added the typed TypeScript SDK wrapper and npm build metadata

## [0.1.2] (2026-05-02)

- Replaced the global store lock with sharded concurrent storage
- Added TLS for RESP and WebSocket listeners
- Added Prometheus metrics
- Added configurable capacity eviction policies
- Added WebSocket key watching and mutation notifications

## [0.1.1] (Initial release)

- Added RESP-compatible TCP and WebSocket servers
- Added strings, collections, expiry, transactions, and Pub/Sub commands
- Added browser mutation synchronization and sender filtering
- Added password, IP, connection, and key-cap safeguards
- Added active expiry and structured logging
