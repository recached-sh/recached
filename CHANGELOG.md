# Changelog

All notable changes to Recached are documented here.

## [Unreleased]

- Added read-only sync scopes: a scope entry may now be written `r=catalog:*` (read-only) or `rw=cart:42:*` (read-write), and a bare pattern stays read-write, so existing tokens are unchanged. Previously a single grant authorized reads *and* writes, so a browser handed `catalog:*` in order to read a shared catalog could also overwrite it — which made every shared read model unsafe to sync. Access is checked per key rather than per command, so `SINTERSTORE mine:out theirs:a` requires write only on the destination. Read-only grants still receive the mutation fan-out and may hold `QSUB` live queries and `WATCH`
- **Breaking (beta):** `SYNC` and `SYNC TOKEN` now echo grants in `r=`/`rw=` notation (`rw=cart:*` where the reply was `cart:*`), so the reply round-trips back into `SYNC`. No shipped client parses this reply
- A scope entry with an empty pattern (`r=`) is now refused at token verification instead of minting a token that matches only the empty key

## [0.3.4] (2026-09-13)

- Fixed concurrent writes, transactions, `WATCH`, and expiry and eviction propagation
- Removed three costs from the command path: the mutation fan-out ran even with no WebSocket clients attached, the key index rebuilt itself on writes that changed nothing, and every RESP length header was parsed by validating UTF-8 and running the generic `str::parse`. `SET` improves from 316,857 to 546,746 ops/s on one worker thread and from 769,823 to about 1,520,000 on four
- Fixed quadratic write cost on collections: hashes, lists, sets and sorted sets now track their own size instead of being walked before and after every write. `HSET` into a 100k-field hash goes from 2,838 to 380,228 ops/s and no longer degrades as the collection grows
- Made snapshots, AOF, and dedup persistence atomic, fail-closed, and observable
- Bounded pub/sub, live-query, and replication queues and added partial replica resync
- Bounded keyspace maintenance and reduced small-value and collection memory use, including halving `EntryValue` from 96 to 48 bytes for every key in the store
- Improved live-query snapshots and collection hydration
- Closed a WebSocket sync client that falls behind the mutation fan-out instead of silently resubscribing, so a browser can no longer hold a permanently stale local replica while reporting itself in sync
- Capped WebSocket message and frame reassembly at 8 MiB, configurable, replacing the 64 MiB library default that an unauthenticated client could hold per connection
- Expanded metrics, fault coverage, benchmarks, and operational documentation, including `recached_sync_lag_disconnects_total`, WebSocket close code `4001`, and a current Linux thread-scaling measurement
- Closed replica transaction and strict live-query scope authorization bypasses
- Enforced `noeviction` memory caps and rejected ambiguous persistence and capacity configuration
- Fixed RESP framing, integer overflow, duplicate pub/sub registration, and crash-safe browser outbox restoration
- Fixed React cache ownership and subscription lifecycle handling

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
