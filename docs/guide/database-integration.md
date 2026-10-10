# Use Recached with MySQL or PostgreSQL

Keep MySQL or PostgreSQL as the source of truth for durable business data. Use Recached to distribute a derived copy into browser and native app caches, where reads use local memory. Send business edits through your application API, commit them to SQL, then synchronize the resulting state through Recached.

::: info Application integration, not a built-in connector
Recached currently has no MySQL/PostgreSQL connector, SQL write-through hook, or acknowledgment of a database commit. You build the API and synchronization worker described here. A Recached acknowledgment means its server accepted a cache write; see [the protocol contract](/server/protocol#the-ordering-invariant-acknowledgment-correlation).
:::

## Commit business changes before publishing cache state

The application backend owns validation and database transactions. Recached distributes the committed state to connected clients:

<figure>
  <img class="light-only" src="/database-architecture-light.svg" alt="A browser or native app sends an edit to the application API. The API commits business rows and an outbox event in one SQL transaction. A worker reads committed events, updates Recached, and Recached pushes state back to clients over WebSocket.">
  <img class="dark-only" src="/database-architecture-dark.svg" alt="A browser or native app sends an edit to the application API. The API commits business rows and an outbox event in one SQL transaction. A worker reads committed events, updates Recached, and Recached pushes state back to clients over WebSocket.">
  <figcaption>SQL owns durable records. Recached and its clients hold derived cache state.</figcaption>
</figure>

For a team rename, use this sequence:

1. The client sends a rename request with the team ID, proposed name, and an application mutation ID.
2. The API authenticates the request, checks team permissions, and validates the name.
3. In one SQL transaction, the API updates the team row and inserts an outbox event describing the committed change. Record the mutation ID in that transaction to recognize retries.
4. After commit, the API reports the saved result and record revision to the client.
5. A worker reads committed outbox events and writes the team projection to Recached over RESP.
6. Recached pushes the value to permitted clients watching the team's keys. Their observers re-read local memory.

A projection is a cache value derived from database records, such as `team:42:profile`:

```json
{"id":42,"name":"Design","revision":17}
```

The API response can arrive before the worker updates Recached. Treat the cache as eventually consistent, and keep the committed response or optimistic UI value visible until the same or a newer database revision reaches the local cache. Reads requiring current authoritative state or transactional decisions should go through the backend.

## Use a database outbox to recover interrupted updates

Writing SQL and Recached in separate steps leaves a failure gap: the database can commit while the cache update fails. Store the row change and its event in the same database transaction instead. The worker can then retry publication without losing the committed change. This is the [transactional outbox pattern](https://docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/transactional-outbox.html).

The worker needs an explicit delivery policy:

- Read only committed events and mark delivery after Recached accepts the cache update.
- Make retries idempotent. Writing a complete committed value with `SET` can safely repeat that value; replaying `INCR` can change a counter twice.
- Preserve ordering for each record. A delayed retry of revision 16 must not overwrite revision 17. Serialize publication per record or implement a guarded update strategy in your integration.
- Include record revisions for client reconciliation. A revision field alone does not prevent an unconditional stale `SET`; the worker must enforce ordering.
- Translate committed database deletions into cache deletions. Cache TTL expiry and eviction do not mean a database row was deleted.

These are integration responsibilities, not guarantees supplied by a Recached SQL adapter. The [outbox pattern reference](https://microservices.io/patterns/data/transactional-outbox.html) also describes duplicate delivery and ordering requirements.

Changes made by jobs, administrators, or other services need the same publication path. Have those writers create outbox events too, or integrate change data capture (CDC), which reads committed database changes and forwards them to your projection worker. Recached does not include that CDC pipeline.

## Give clients read-only access to database-backed keys

Enable strict [sync scopes](/server/sync-scopes) with `RECACHED_SYNC_SECRET`. Your backend mints signed tokens granting clients read access to their authorized projections, for example `r=team:42:*`. Trusted backend workers write those projections through the server's RESP interface; protect that interface with [server authentication and network controls](/server/security).

Browser clients call `liveQuery('team:42:*')`; Kotlin and Swift clients call `watch("team:42:*")`. The initial snapshot and later updates populate local memory. Client observers do not create subscriptions, and watches do not grant permission.

Calling `cache.set()` on an authoritative key applies a local cache edit and queues a Recached write. It does not call your API. A read-only scope refuses that server write, so keep optimistic business edits in separate UI state or an application-owned local store.

## Queue offline business edits separately

An offline app can show whichever values its local cache retained or restored and accept pending edits in its own durable command queue. Send those commands to the application API when connectivity returns. The API validates each command against current database state and returns a committed result or a rejection. Browser and native persistence have different cold-start guarantees; see [client persistence limits](/guide/client-support#write-durability-and-replay).

Use application mutation IDs to recognize repeated requests. Enforce their uniqueness within the authenticated user's or client's namespace in SQL, and save the result with the business change so a retry returns that result. Include an expected record revision when conflicting edits should be rejected or merged by business rules. Show pending, saved, and rejected states explicitly; an offline edit is pending until the API confirms its database commit.

The queues in this design have different responsibilities:

| Queue | Owner | Purpose | Completion |
|---|---|---|---|
| Offline business command queue | Your app | Deliver edits to your application API | API confirms a SQL commit or permanent rejection |
| Database outbox | Your backend | Publish committed database changes into cache projections | Worker records delivery after cache acceptance |
| Recached client outbox | Recached SDK | Replay cache commands to the Recached server | Server accepts or permanently refuses the cache command |

Recached's `pendingWrites` describes the last row, not whether an application edit reached SQL. Its duplicate suppression also does not replace database-level application idempotency. See [client persistence and replay limits](/guide/client-support#write-durability-and-replay).

## Direct client writes still have useful applications

Clients can update temporary shared state directly, and Recached distributes accepted changes to other permitted clients. These writes do not need a SQL transaction when the state can be lost, expired, or regenerated without damaging durable business records.

Useful applications include:

- Presence, such as whether a team member is currently online
- Typing indicators and temporary cursor positions in a shared workspace
- Disposable shared UI state, such as which item someone is previewing

Use narrow writable scopes for these keys. Add expiry where appropriate, and refresh transient state after reconnecting so old activity is not mistaken for current activity. Database-backed records keep the API write path:

| Data | Write path |
|---|---|
| Team settings, saved tasks, orders, inventory | Application API → SQL transaction → outbox worker → Recached |
| Presence, typing indicators, disposable shared UI state | Client → Recached with narrowly scoped write permission |
| Offline edits to database-backed records | Application-owned command queue → API on reconnect |

Keep transient state in a separate namespace from database projections. A token can grant `r=team:42:*` alongside `rw=presence:team:42:user:7:*`, with grants minted according to the authenticated user's permissions. Writable scope checks limit keys; your application still defines acceptable values and business rules.

## Recover the cache from the database

Retaining or delivering outbox events does not make Recached the system of record. Build a way to reconstruct projections from SQL after cache data loss, eviction, or a cold deployment. Coordinate that rebuild with live publication so old snapshot values cannot overwrite newer changes, and reconcile removed rows too.

Use live queries to reconcile client copies after reconnecting. Their snapshots reflect Recached's current cache state, not a fresh SQL query. They cannot repair a database change that your integration never published.

## Understand the limit of copying cache changes into SQL

A listener that copies cache mutations into SQL cannot serve as a reliable business write path by itself. Recached pub/sub is not a durable command log: disconnected subscribers miss messages. Key notifications describe cache state changes, including expiry and eviction, and SQL validation can fail after a cache value has already reached other clients.

If direct SDK writes must become database-backed edits, an additional integration must authorize and validate commands, deliver them durably, commit SQL, and report database completion separately from cache acceptance. That integration is not implemented in Recached today.
