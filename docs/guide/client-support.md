# Choose a client

Recached runs the same Rust cache engine on the server, in browser WebAssembly, and inside native apps and Rust services. Choose an adapter for your runtime, then check its persistence and sync limits before using it.

## Implemented clients

The server exposes a broader command surface than the client SDKs:

| Runtime | Package | Local reads | Persistence | Reactive API | Status in this checkout |
|---|---|---|---|---|---|
| Backend over RESP | Server package `recached`, binary `recached-server` | Reads run on the server | Snapshot and optional AOF | RESP pub/sub and key notifications | Cache server release candidate |
| Browser | `recached-edge` | WebAssembly memory | Optional IndexedDB WAL and outbox | `onKeyChange`, `onPatternChange`, `onMutation` | Sync layer beta |
| React / Vue in a browser | `@recached/react` / `@recached/vue` | Through `recached-edge` | Same as browser client | Hooks / composables | Browser adapters |
| Android | `dev.recached:recached-android` | Native Rust memory through UniFFI | SQLite local data and outbox | Kotlin `Flow` and `StateFlow` | 0.1.0 preview on Maven Central |
| iOS / macOS | Swift package `Recached` | Native Rust memory through UniFFI | SQLite local data and outbox | `AsyncStream`, `ObservableKey`, `ObservedKey` | 0.1.0 preview via SwiftPM |
| Rust service | `recached-embed` | Native process memory | In-memory store and outbox | `subscribe` pub/sub receiver | Git dependency; unpublished crate |

The native SDKs run Rust directly, without WebAssembly or a WebView. Android supports API 24+ on `arm64-v8a`, `armeabi-v7a`, and `x86_64`. Swift supports iOS 15+ and macOS 12+; `ObservedKey` needs iOS 17+ or macOS 14+.

Flutter, React Native, WASI deployment, and server-side WASM scripting are [planned](/roadmap). The browser React adapter is not a React Native SDK.

## Hydration, observation, and authorization

A connection alone does not fetch an initial copy of the server's keyspace. Register the data you need:

| Client | Subscribe to server state | Observe local changes |
|---|---|---|
| Browser | `cache.liveQuery(pattern)` | `onKeyChange` / `onPatternChange` |
| Kotlin | `cache.watch(pattern)` | `observe` / `observeString` / `observeMatching` |
| Swift | `cache.watch(pattern)` | `observe` / `observeString` / `observeMatching` |
| Rust | `cache.watch(pattern).await?` | No key-observer API; `subscribe(channel)` is pub/sub |

Live queries receive an initial `qstate` snapshot and subsequent changes. On reconnect, the shared `sync-client` replays queued writes before re-subscribing, so the new snapshot includes accepted writes. It also removes matching local keys that the server deleted while the client was offline.

Mobile watch registrations are not persisted. Call `watch` after each open. Local observers do not register watches; they emit the current local value, then re-read on changes. Equal values are suppressed, and slow consumers can skip intermediate values. Observers are not an event log.

Mobile and browser reads can return saved or locally written values before hydration. Rust's embedded client instead returns `NotHydrated` for reads outside a hydrated watch.

## Synchronization status

An open socket is not a synchronized cache. Every client reports one status, driven by protocol events alone:

| Phase | Meaning |
|---|---|
| Disconnected / Connecting | No socket, or one being opened |
| Authenticating | `AUTH`, the protocol opt-ins or the sync token not yet answered |
| Replaying | Writes queued while offline sent, not all answered |
| Hydrating | Snapshots of the patterns watched at connect time not all received |
| Ready | All of the above complete. A pattern watched later hydrates on its own and reports through its subscription state |
| Degraded | Open but impaired: authentication, the scope or a live query refused, or the socket being replaced after a deferred write or an unparseable frame |

The status carries the problem, when there is one, and the time it last became Ready — the moment synchronization completed, never just when a socket opened.

| Client | Status | Per-pattern progress |
|---|---|---|
| Browser | `cache.status()`, `onStatusChange` | `subscriptionState(pattern)`, `onSubscriptionChange` |
| Kotlin | `syncStatus: StateFlow<SyncStatus>` | `subscriptionStates: StateFlow<Map<…>>` |
| Swift | `syncStatus`, `syncStatuses()`, `ObservedSyncStatus` / `ObservableSyncStatus` | `subscriptionStates`, `subscriptionStateChanges()` |
| Rust | `status()`, `status_changes()` (`watch::Receiver`) | `subscription_state(pattern)` |

The older `connectionState` (`CONNECTED` / `.connected`) still describes the socket alone.

Subscriptions select state to hydrate and reconcile; they are not authorization. In the default unscoped server mode, clients receive mutations outside their watched patterns too. Mobile persists those received keys. Configure [sync scopes](/server/sync-scopes) with `RECACHED_SYNC_SECRET` and backend-minted tokens for per-user data.

## Native mobile API surface

Kotlin and Swift expose a focused cache API rather than the entire RESP command catalog:

| Task | Kotlin | Swift |
|---|---|---|
| Read bytes / text | `get`, `getString` | `get`, `getString` |
| Read serialized JSON | `getJson(key, path)` | `getJSON(_:path:)` |
| Read key metadata | `exists`, `ttl` | `exists`, `ttl` |
| Read matching entries | `getMatching` | `getMatching` |
| Write text or bytes | `set`, `set(..., ttl)` | `set`, `set(_:_:ttl:)` |
| Delete / increment | `delete`, `incrBy` | `delete`, `incr(_:by:)` |
| Write JSON / merge patch | `jsonSet`, `jsonMerge` | `setJSON`, `mergeJSON` |
| Manage sync | `watch`, `unwatch`, `setSyncToken`, `reconnectNow` | Same names |
| Observe bytes / text / pattern | `observe`, `observeString`, `observeMatching` | Same names |
| Track synchronization | `syncStatus`, `subscriptionStates` (`StateFlow`) | `syncStatus`, `subscriptionStates` plus streams |
| Track socket / queue | `connectionState`, `pendingWrites` (`StateFlow`) | Properties plus `connectionStates()`, `pendingWriteCounts()` |
| Handle permanent refusals | `refusedWrites` (`SharedFlow`) | `refusedWrites()` (`AsyncStream`) |

`getString` fails on non-UTF-8 bytes. `observeString` and `Entry.string` return `null` / `nil` for binary values. Pattern entries contain a key and optional string-value bytes; collection entries have no single-value representation. Native wrappers do not expose individual hash/list/set/sorted-set commands or pub/sub APIs.

`ttl` returns seconds remaining, `-1` without an expiry, and `-2` for a missing key. Positive TTL writes round up to whole seconds. Shared clients negotiate expiry metadata so watched values expire locally while offline; older servers without this feature send values without their expiry. Reads mask expired values, but native observers have no local expiry timer: an offline TTL expiring does not itself emit an update until another invalidation causes a re-read.

## Write durability and replay

A Recached server acknowledgment confirms a cache write, not a MySQL or PostgreSQL transaction. For database-backed business data, send edits through your API and publish committed projections through Recached. The SDK outbox does not deliver application commands to SQL. See [database integration](/guide/database-integration).

A write passes through distinct events, and each SDK says which one a call reports:

| Event | Browser | Mobile | Rust embedded |
|---|---|---|---|
| Applied in local memory | when the write call returns | when the write call returns | when the server's change arrives (live query) |
| Stored on the device | `await cache.flush()` resolves | when the write call returns (one SQLite transaction with its outbox row) | never: the outbox is in memory |
| Applied by the server | the row leaves `pendingWrites` | the row leaves `pendingWrites` | the write call returns `Ok` |
| Survives a server crash | per the server's snapshot / AOF configuration — never implied by an acknowledgment | | |

A write that fails changed nothing. A full outbox refuses the write before it touches local state (`OutboxFullError`, `RecachedException.OutboxFull`, `RecachedError.outboxFull`, `Error::OutboxFull`); a mobile write whose SQLite transaction fails is rolled back in memory and never sent. The default cap is 10,000 writes; the pre-0.5 behaviour — evict the oldest queued write, which then never reaches the server — must be chosen explicitly (`whenFull: 'evict-oldest'`, `evictOldestWhenFull`).

Mobile SQLite uses WAL mode with `synchronous = NORMAL`: committed transactions survive an app kill, while an OS crash or power loss can lose recent commits. Browser IndexedDB writes are asynchronous; a write is in IndexedDB once `flush()` resolves, and failures (quota, storage cleared) reach `onPersistenceError` and reject `flush()`. Browsers do not promise a committed transaction survives a power cut. See [browser persistence](/browser/persistence).

Queued writes retain their original `DEDUP` identities across restarts, and the server suppresses a replay of a write it already applied — across a server crash too, when it runs an AOF and the write was within the fsync window. This is not an unconditional exactly-once guarantee: [the protocol page](/server/protocol#crash-consistency) lists exactly when a retried write can run again (marks swept after 24 idle hours on a busy server, a promoted replica, writes the persistence policy lost). Use application-level idempotency when duplicates are unacceptable.

Other limits affect delivery:

- Permanent server refusals leave the queue and emit a refusal event naming the keys and, separately, the keys no watched pattern covers (`unreconciled`): a fresh snapshot restores the watched ones; the unwatched ones keep the refused local value. Refusal streams are live notifications, not persisted audit logs.
- Transient refusals keep the write queued and trigger reconnect with backoff; a 0.5 server defers every later write on that socket too, so none overtakes it.
- A reconnect snapshot never rolls back a write still queued: keys a queued write covers keep their local value until the write's own acknowledgment brings the server's.
- `set` and whole-document JSON writes resolve by arrival order at the server. Counter deltas add; JSON merge patches preserve fields they do not touch. A write that loses the race is overwritten, with no conflict signal. This is not a general CRDT conflict-resolution model.

Open one mobile instance per database file. Keep its identity and database out of device backups: copying it to another device can make the server treat that device's writes as duplicates. Android's context-based open uses `noBackupFilesDir`; Swift's `open(named:)` excludes its Application Support directory from backups.

## Start with your runtime

Follow [the browser guide](/browser/getting-started), [Android preview guide](/android/getting-started), [Apple preview guide](/ios/getting-started), or [embedded Rust guide](/rust/getting-started). For server-only workloads, use [server installation](/server/installation) and the [command reference](/server/commands).
