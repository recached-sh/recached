# Commands

Recached implements the subset of RESP commands that most applications use. Commands work over both TCP (port 6379) and WebSocket (port 6380).

## Concurrency model

Recached schedules independent commands across worker threads over a sharded keyspace. Commands whose write sets overlap share ordering barriers; unrelated keys can progress concurrently.

**What that guarantees**

- **Single-key commands are atomic.** Read-modify-write commands such as `INCR`, `SETNX`, `GETSET`, `HINCRBY`, `LPUSH`, and `ZADD` hold one entry guard for the decision and mutation.
- **Conflicting writers are ordered.** A multi-key write or `MULTI`/`EXEC` reserves all of its write keys against other server writes until persistence, replication, and notifications have recorded the result.
- **`WATCH` compare-and-swap is sound.** `EXEC` reserves both its write keys and watched keys before checking invalidation, so a writer cannot land between the check and execution.
- **Values are not torn.** A reader sees a value before or after an individual mutation, never a partial value.

**What it does not guarantee**

- **Readers do not join the writer barriers.** Commands such as `MSET` and `SMOVE` update their keys one at a time. A concurrent reader can observe an intermediate cross-key state even though another writer cannot interleave on those keys.
- **`MULTI`/`EXEC` is writer-isolated, not fully isolated.** Another connection cannot write a key reserved by the transaction, but reads may observe results between queued commands.

If correctness depends on values read before a transaction, `WATCH` those keys and retry when `EXEC` returns nil. Use a database or another system with fully isolated multi-key transactions when readers must observe several keys changing as one indivisible state transition.

---

## Core

| Command | Description |
|---|---|
| `PING [message]` | Returns `PONG`, or echoes `message` if provided. Used to test connectivity and measure latency. |
| `AUTH password` | Authenticates the connection. Required on the first command if `RECACHED_PASSWORD` is set. 5 consecutive failures close the connection. |
| `HELLO [protover]` | Reports server info and negotiates the protocol version. `3` switches the connection to RESP3, `2` back to RESP2, no argument reports without changing. Unsupported versions return `-NOPROTO` and leave the connection unchanged. Requires authentication. See [Wire Protocol](/server/protocol#protocol-version-tcp). |
| `INFO [section ...]` | Reports server statistics as a text blob, in Redis's `# Section` / `field:value` format. No arguments returns the default sections; naming sections returns only those, in the order given. `all`, `everything`, and `default` are accepted as aliases for the default set. Unknown section names return nothing rather than an error. Requires authentication. See [INFO](#info) below. |
| `QUIT` | Replies `+OK` and closes the connection. Accepted before authentication and in subscribe mode, so a client can always close cleanly. |
| `CLIENT <subcommand>` | Connection introspection. See [CLIENT](#client) below. |
| `CONFIG GET parameter [parameter ...]` | Reports configuration parameters, matched by glob. See [CONFIG](#config) below. |
| `COMMAND [COUNT\|LIST\|INFO\|DOCS]` | Reports the command catalog. See [COMMAND](#command) below. |
| `CLUSTER <subcommand>` | Refused: Recached is standalone. The flag is in `INFO`. See [CLUSTER and MODULE](#cluster-and-module) below. |
| `MODULE LIST` | An empty array — there is no module API. See [CLUSTER and MODULE](#cluster-and-module) below. |

---

## CLIENT

Every current client library — node-redis, ioredis, redis-py, go-redis — sends `CLIENT SETINFO LIB-NAME` and `CLIENT SETINFO LIB-VER` immediately after `HELLO`, so the server can attribute a connection to the library that opened it.

| Subcommand | Description |
|---|---|
| `CLIENT ID` | This connection's numeric identifier. |
| `CLIENT INFO` | One line describing this connection, in Redis's `key=value` format. |
| `CLIENT LIST` | The same line for every live connection, in connection order. |
| `CLIENT GETNAME` | This connection's name, or nil if unnamed. |
| `CLIENT SETNAME name` | Names this connection. Spaces and newlines are refused, because the name is echoed into the space-separated `CLIENT LIST` format. |
| `CLIENT SETINFO LIB-NAME\|LIB-VER value` | Records the client library's name or version against this connection. |

The reported fields are `id`, `addr`, `laddr`, `name`, `age`, `idle`, `flags`, `db`, `sub`, `psub`, `multi`, `resp`, `lib-name` and `lib-ver`. Redis also reports buffer sizes, file descriptors and an event mask; Recached omits them rather than emitting plausible numbers, since nothing downstream could distinguish an invented `omem` from a real one. Parsers read this format key by key and skip what they do not recognise.

`CLIENT KILL`, `CLIENT NO-EVICT`, `CLIENT NO-TOUCH`, `CLIENT UNPAUSE` and `CLIENT MAINT_NOTIFICATIONS` return an "unknown subcommand" error. Each is an operation with real consequences, and replying `+OK` without performing it would leave the caller believing a connection had been killed or eviction disabled.

---

## CONFIG

| Subcommand | Description |
|---|---|
| `CONFIG GET parameter [parameter ...]` | Returns matching parameter/value pairs. Names may be globs: `CONFIG GET maxmemory*` returns both `maxmemory` and `maxmemory-policy`. An unmatched name yields no pair rather than an error. |
| `CONFIG SET` | Refused with an explanatory error — see below. |

The reported parameters are `maxmemory`, `maxmemory-policy`, `maxclients`, `port`, `tls-port`, `appendonly`, `databases`, `requirepass`, `proto-max-bulk-len`, `timeout` and `save`. Every value is read from what is actually in force: the eviction policy from the store, the ports and connection limit from the same startup facts `INFO` reports. `requirepass` is masked to `*` when a password is set and empty when it is not — whether a password exists is not a secret, its value is.

**`CONFIG SET` is not supported.** Recached reads its configuration from the environment at startup and holds it for the life of the process, so there is nothing a runtime `SET` could change. It returns an error naming the parameter and pointing at the environment variable instead, because returning `+OK` would leave an operator to discover much later that the limit they set never applied. See [Configuration](/server/configuration).

---

## COMMAND

| Subcommand | Description |
|---|---|
| `COMMAND` | Every command in the catalog, as `COMMAND INFO` entries. |
| `COMMAND COUNT` | How many commands the server implements. |
| `COMMAND LIST` | Their names. |
| `COMMAND INFO [name ...]` | Name, arity, flags and key positions. A name the server does not have replies nil in its slot, so the reply stays aligned with the request. |
| `COMMAND DOCS [name ...]` | Summary, group and arity per command. RESP3 returns a map; RESP2 returns the same pairs flattened, as Redis degrades it. |

Arity, flags and key positions are transcribed from a real `redis-server`'s own `COMMAND INFO` rather than written by hand: cluster-aware clients and proxies route on `first_key` and `step`, and a wrong arity makes a client reject a call the server would have accepted. The nine commands with no Redis counterpart — `ESET`, `JSET`, `JGET`, `JMERGE`, `RLSET`, `RLCHECK`, `SYNC`, `DEDUP`, `QSUB`, `QUNSUB` — are declared directly.

Recached has no ACL system and no subcommand tree, so the ACL-categories, tips, key-specs and subcommands elements Redis 7 appends to each `COMMAND INFO` entry are present but empty. A client indexing past the sixth element finds an empty list rather than running off the end of the array.

---

## CLUSTER and MODULE

Recached is a single node with no module API. Both facts are reported the way Redis reports them, which for one of the two is not the obvious way.

| Command | Description |
|---|---|
| `CLUSTER <any subcommand>` | Refused with `ERR This instance has cluster support disabled`. |
| `MODULE LIST` | An empty array. |
| `MODULE LOAD \| LOADEX \| UNLOAD` | Refused. |

**Cluster support is advertised through `INFO`, not through `CLUSTER`.** A `redis-server` that was not started in cluster mode rejects the entire `CLUSTER` container with that exact sentence — it does *not* answer `CLUSTER INFO` with `cluster_enabled:0`, which is a common assumption and a wrong one. The flag lives in `INFO`'s `# Cluster` section, which is where every cluster-aware client actually reads it. Recached now emits that section, in the default set, so a bare `INFO` carries it:

```bash
redis-cli -p 6379 INFO cluster
# Cluster
cluster_enabled:0
```

Copying Redis's refusal verbatim matters more than it looks. Recached previously answered `ERR unknown command`, and that is the one reply a client cannot interpret: "unknown command" reads as *this server is too old to ask*, which is a different branch from *this server is not a cluster*. The sentence above puts a client on the same path it takes against the server it was written for.

`MODULE LIST` returning an empty array is not a workaround — it is the same answer a stock `redis-server` with no modules loaded gives, and it lets a tool distinguish "no modules" from "cannot ask". `LOAD` and `UNLOAD` are refused rather than answered `+OK`, because an operator who believes a module loaded has a harder problem to debug than one who was told no.

---

## INFO

`INFO` reports the server's own state — uptime, client counts, memory, replication topology — in the same line format Redis uses, so `redis-cli info`, monitoring agents, and client library ready-checks all parse it unmodified.

```bash
redis-cli -p 6379 INFO              # every default section
redis-cli -p 6379 INFO replication  # one section
redis-cli -p 6379 INFO server memory
```

### Sections

| Section | Fields |
|---|---|
| `server` | `redis_version`, `recached_version`, `redis_mode`, `os`, `arch_bits`, `process_id`, `run_id`, `tcp_port`, `recached_ws_port`, `recached_tls_enabled`, `recached_auth_enabled`, `uptime_in_seconds`, `uptime_in_days` |
| `clients` | `connected_clients`, `maxclients`, `blocked_clients` |
| `memory` | `used_memory`, `used_memory_human`, `maxmemory`, `maxmemory_human`, `maxmemory_policy`, `recached_max_keys` |
| `persistence` | `loading`, `rdb_changes_since_last_save`, `rdb_last_save_time`, `rdb_bgsave_in_progress`, `aof_enabled` |
| `stats` | `total_connections_received`, `total_commands_processed`, `keyspace_hits`, `keyspace_misses`, `evicted_keys` |
| `replication` | `role`, `connected_slaves`, `connected_replicas`, `recached_replication_queue_depth`, `recached_replication_lag_frames` |
| `cluster` | `cluster_enabled:0` — always, see [CLUSTER and MODULE](#cluster-and-module) |
| `keyspace` | `db0:keys=N,expires=N,avg_ttl=0` — omitted entirely when the keyspace is empty, as in Redis |
| `recached` | `live_queries`, `watched_keys` — Recached-specific, no Redis equivalent |

### Version reporting

`redis_version` reports **`6.2.0`**, not Recached's version. Client libraries feature-gate on that field, and a library reading `redis_version:0.2.3` concludes the server predates everything and disables capabilities it could safely use. 6.2 is the honest floor: RESP3 and `HELLO` exist there and Recached implements both, while nothing newer that Recached lacks gets advertised. Recached's real version ships alongside it as **`recached_version`** — the same split KeyDB and Dragonfly use.

### Field notes

- **`role`** reports `master` or `slave`, matching Redis's wire spelling because tooling greps for exactly those strings. `connected_replicas` is emitted as an alias of `connected_slaves` for readability; both carry the same number.
- **`loading`** is always `0`. Recached loads its snapshot before binding a listener, so a client that can reach the server is never looking at one still loading. Client ready-checks gate on this field.
- **`used_memory`** and keyspace counts come from incrementally maintained counters sampled every 5 seconds. `INFO` does not walk the keyspace. `used_memory` measures logical key/value bytes, not allocator RSS. Expired physical entries leave these counters when the bounded active-expiry task removes them.
- **`maxmemory`** and `recached_max_keys` report `0` when no limit is configured, matching Redis's convention for "unbounded".

### Not implemented

`INFO` does not report the `cpu`, `commandstats`, `latencystats`, or `errorstats` sections. Per-command call counts and error counts are exported to [Prometheus](/server/operations#metrics-endpoint) on port 9091 instead, which is where they belong for dashboards and alerting. `INFO` is for the operator at a terminal and for client ready-checks.

`INFO` does not expose latency sections, but Prometheus exports `recached_command_duration_seconds{command=...}`. Recached does not implement `SLOWLOG`, so use client tracing for individual slow requests. Prefer bounded reads (`HSCAN`, `SSCAN`, `ZSCAN`, `GETRANGE`) over whole-collection replies on large keys.

### Access

`INFO` requires authentication when `RECACHED_PASSWORD` is set, and is rejected on scope-limited WebSocket connections (`-NOSCOPE`) — a connection granted a handful of keys has no business reading server-wide state. See [Sync Scoping](/server/sync-scopes).

---

## Strings

The most common data type. Values are always stored as byte strings; numeric operations parse the value as an integer or float.

| Command | Description |
|---|---|
| `SET key value [EX seconds] [PX ms] [EXAT timestamp] [PXAT ms-timestamp] [NX\|XX] [KEEPTTL] [GET]` | Set a key to a string value. `EX`/`PX`/`EXAT`/`PXAT` set expiry. `NX` only sets if key does not exist. `XX` only sets if key exists. `KEEPTTL` preserves the existing TTL. `GET` returns the old value before overwriting. |
| `GET key` | Returns the value of a key, or nil if the key does not exist or has expired. |
| `GETSET key value` | Sets the key to a new value and returns the old value atomically — the read and the write happen under one lock, so two concurrent callers can never be handed the same old value. Deprecated in Redis 6.2 — prefer `SET key value GET`. |
| `MGET key [key ...]` | Returns the values of multiple keys. Keys that do not exist return nil. |
| `MSET key value [key value ...]` | Sets multiple keys to their respective values in one command. Applied key by key — see [Concurrency model](#concurrency-model): a concurrent reader can observe some keys updated and others not. |
| `ESET key value` | **Ephemeral set.** Stores a string like `SET`, but the key's lifetime is bound to the connection that wrote it — when that connection closes, the server deletes the key and the deletion is pushed to live queries. Writing the same key again transfers ownership to the newest connection, so a second browser tab keeps presence alive when the first closes. Intended for presence, cursors, and "who is online"; use `SET` for anything that should outlive a connection. |
| `SETNX key value` | Set a key only if it does not exist. Returns 1 if set, 0 if the key already existed. |
| `SETEX key seconds value` | Set a key with an integer-second expiry. Equivalent to `SET key value EX seconds`. |
| `PSETEX key milliseconds value` | Set a key with a millisecond-precision expiry. |
| `APPEND key value` | Appends a string to the end of the existing value. If the key does not exist, it is created. Returns the new length. |
| `STRLEN key` | Returns the length of the string stored at key. Returns 0 if the key does not exist. |
| `GETRANGE key start end` | Returns the inclusive byte range `start`..`end` of the string. Negative offsets count back from the end (`-1` is the last byte). `end` clamps to the last byte, but `start` does not: a `start` past the end of the value returns an empty string rather than the final byte, as in Redis. A missing key is an empty string, so every range of it is empty. Use it to read a window of a large value without transferring the whole thing. |
| `INCR key` | Increments the integer value of a key by 1. Creates the key with value 1 if it does not exist. Returns an error if the value is not a valid integer. |
| `DECR key` | Decrements the integer value of a key by 1. Creates the key with value -1 if it does not exist. |
| `INCRBY key increment` | Increments the integer value of a key by the given integer. |
| `DECRBY key decrement` | Decrements the integer value of a key by the given integer. |

---

## Expiry

| Command | Description |
|---|---|
| `EXPIRE key seconds` | Set a timeout on a key in seconds. The key is deleted when the timeout expires. Returns 1 if set, 0 if key does not exist. |
| `PEXPIRE key milliseconds` | Set a timeout in milliseconds. |
| `EXPIREAT key unix-timestamp` | Set an absolute expiry time as a Unix timestamp (seconds). |
| `PEXPIREAT key ms-unix-timestamp` | Set an absolute expiry time as a Unix timestamp in milliseconds. |
| `TTL key` | Returns the remaining time-to-live of a key in seconds. Returns -2 if the key does not exist, -1 if the key has no expiry. |
| `PTTL key` | Returns the remaining TTL in milliseconds. |
| `PERSIST key` | Removes the TTL from a key, making it persistent. Returns 1 if the TTL was removed, 0 if the key has no expiry or does not exist. |

---

## Keys

| Command | Description |
|---|---|
| `DEL key [key ...]` | Deletes one or more keys. Returns the number of keys that were deleted. Keys that do not exist are ignored. |
| `UNLINK key [key ...]` | Non-blocking delete. Semantically equivalent to `DEL` (Recached does not implement async deletion, but `UNLINK` is accepted for client compatibility). |
| `EXISTS key [key ...]` | Returns the number of keys that exist among the provided arguments. A key listed multiple times counts multiple times. |
| `TYPE key` | Returns the type of the value stored at key: `string`, `hash`, `list`, `set`, `zset`, `ratelimit`, or `none` if the key does not exist. |
| `RENAME key newkey` | Renames a key. Returns an error if the source key does not exist. Overwrites `newkey` if it already exists. |
| `KEYS pattern` | Returns all keys matching the glob pattern. `*` matches any sequence of bytes, `?` matches exactly one byte. **Character classes (`[abc]`) are not supported** — brackets match literally. Patterns are capped at 1,024 bytes. Warning: `KEYS *` on a large store is slow — prefer `SCAN`. |
| `SCAN cursor [MATCH pattern] [COUNT count]` | Iterates a maintained ordered key index. Each call examines at most `COUNT` keys (default 10), so work and reply size are bounded independently of total key count. `MATCH` may make a page shorter than `COUNT`. Start with `0` and continue until the returned cursor is `0`. Abandoned cursors are capped at 4,096 sessions. Concurrent changes may be missed or returned twice. |
| `DBSIZE` | Returns the maintained stored-key count in O(1) time. Reads still treat an expired key as missing immediately, but `DBSIZE` may include its physical entry until the bounded active-expiry task removes it. |
| `FLUSHDB [ASYNC]` | Removes all keys from the store. `ASYNC` is accepted but does not change behavior (the flush is always synchronous). |
| `MEMORY USAGE key [SAMPLES count]` | Approximate bytes held by one key — the key name, its value, and a fixed per-entry overhead. Returns nil when the key does not exist or has expired. `SAMPLES` is accepted and ignored. |

### MEMORY USAGE

The figure uses the same logical key/value accounting as eviction. It excludes allocator metadata, fragmentation, connection buffers, replication queues, and other process memory. Use process RSS for host capacity planning.

```bash
redis-cli -p 6379 MEMORY USAGE session:8f21
(integer) 4162
```

It counts the bytes Recached stores — key name, value contents, and 64 bytes of per-entry overhead — not the allocator's true footprint, which Recached does not manage and cannot see. Treat it as a way to compare keys against each other and to find the fat one, not as an exact resident-set contribution.

Redis's `SAMPLES` bounds how much of a nested value it walks before extrapolating. Recached always walks all of it, so the option parses (a client that sends it is not broken) and the count is discarded. The reply is never less accurate than what was asked for.

The other `MEMORY` subcommands — `DOCTOR`, `STATS`, `PURGE`, `MALLOC-STATS` — are refused. They describe an allocator arena that Recached has no equivalent of: it holds Rust values in a concurrent map and has nothing to defragment or free on demand. `INFO memory` reports what it can actually measure.

`CLIENT DELTA ON|OFF` asks for compact `keydelta` push frames in place of whole-value `keychange` frames, where the mutation has a compact form (`APPEND`, `SADD`, `SREM`, `LPUSH`, `RPUSH`, `HSET`, `HDEL`, `ZADD`, `ZREM`). Off by default, because a client that did not understand the frame would ignore it and silently hold a stale copy. See [the protocol reference](/server/protocol#key-deltas-client-delta-on).

`MEMORY USAGE` reads a key, so it is scoped like one: a WebSocket connection granted `cart:*` may measure `cart:42` and not `session:8f21`. See [Sync Scoping](/server/sync-scopes).

---

## Hash

A hash is a map of field-value pairs stored under a single key. Use hashes to store structured objects without serializing to JSON.

| Command | Description |
|---|---|
| `HSET key field value [field value ...]` | Sets one or more fields in a hash. Creates the hash if it does not exist. Returns the number of fields that were added (not updated). |
| `HGET key field` | Returns the value of a specific field. Returns nil if the field or hash does not exist. |
| `HGETALL key` | Returns all field-value pairs of a hash as a flat array: field1, value1, field2, value2, ... |
| `HDEL key field [field ...]` | Deletes one or more fields from a hash. Returns the number of fields removed. |
| `HMGET key field [field ...]` | Returns the values of multiple fields. Non-existent fields return nil. |
| `HKEYS key` | Returns all field names in the hash. |
| `HVALS key` | Returns all values in the hash. |
| `HLEN key` | Returns the number of fields in the hash. |
| `HEXISTS key field` | Returns 1 if the field exists in the hash, 0 otherwise. |
| `HSETNX key field value` | Sets a field only if it does not already exist. Returns 1 if set, 0 if the field already existed. |
| `HINCRBY key field increment` | Increments the integer value of a hash field by the given integer. Creates the field with value 0 before incrementing if it does not exist. |
| `HINCRBYFLOAT key field increment` | Increments the float value of a hash field by the given float. |
| `HSCAN key cursor [MATCH pattern] [COUNT count] [NOVALUES]` | Iterates a hash incrementally: returns the next cursor plus at most `COUNT` field-value pairs (default 10). Start at cursor `0` and continue until the returned cursor is `0`. `MATCH` filters on field names; `NOVALUES` returns field names only. The bounded counterpart of `HGETALL` — prefer it for hashes whose size you do not control. |

### Example

```bash
HSET user:1 name Alice plan pro credits 500
HGET user:1 name          # "Alice"
HGETALL user:1            # ["name", "Alice", "plan", "pro", "credits", "500"]
HINCRBY user:1 credits -50
HGET user:1 credits       # "450"
```

---

## List

A doubly-linked list. Supports push/pop from both ends. Use for queues (`RPUSH` + `LPOP`), stacks (`LPUSH` + `LPOP`), and fixed-length histories (`RPUSH` + `LTRIM`).

| Command | Description |
|---|---|
| `LPUSH key element [element ...]` | Prepends one or more elements to the head of a list. Multiple elements are pushed left-to-right (the last argument ends up at the head). Returns the list length. |
| `RPUSH key element [element ...]` | Appends one or more elements to the tail of a list. Returns the list length. |
| `LPUSHX key element [element ...]` | Like `LPUSH`, but only if the key already exists. Returns 0 if the key does not exist. |
| `RPUSHX key element [element ...]` | Like `RPUSH`, but only if the key already exists. |
| `LPOP key [count]` | Removes and returns the first element (or `count` elements) from the list. Returns nil if the list is empty or does not exist. |
| `RPOP key [count]` | Removes and returns the last element (or `count` elements) from the list. |
| `LRANGE key start stop` | Returns the elements of the list between index `start` and `stop` (inclusive). Negative indices count from the tail: -1 is the last element. |
| `LLEN key` | Returns the length of the list, or 0 if the key does not exist. |
| `LINDEX key index` | Returns the element at the given index. 0 is the first element, -1 is the last. Returns nil if the index is out of range. |
| `LSET key index element` | Sets the list element at the given index to a new value. Returns an error if the index is out of range. |
| `LREM key count element` | Removes occurrences of `element` from the list. `count > 0`: removes from head. `count < 0`: removes from tail. `count = 0`: removes all occurrences. Returns the number of removed elements. |
| `LTRIM key start stop` | Keeps only the elements between `start` and `stop`, removing the rest. Useful for capping list length. |

---

## Set

An unordered collection of unique string members. Supports set operations (intersection, union, difference).

| Command | Description |
|---|---|
| `SADD key member [member ...]` | Adds one or more members to a set. Ignores members that already exist. Returns the number of new members added. |
| `SMEMBERS key` | Returns all members of the set. Order is not guaranteed. |
| `SREM key member [member ...]` | Removes one or more members from a set. Returns the number of members actually removed. |
| `SCARD key` | Returns the number of members in the set. |
| `SISMEMBER key member` | Returns 1 if the member exists in the set, 0 otherwise. |
| `SMISMEMBER key member [member ...]` | Returns an array of 1/0 values for each member, indicating existence. |
| `SINTER key [key ...]` | Returns the intersection of all given sets. |
| `SINTERSTORE destination key [key ...]` | Stores the intersection into `destination` and returns its size. |
| `SUNION key [key ...]` | Returns the union of all given sets. |
| `SUNIONSTORE destination key [key ...]` | Stores the union into `destination` and returns its size. |
| `SDIFF key [key ...]` | Returns the members in the first set that are not in any of the other sets. |
| `SDIFFSTORE destination key [key ...]` | Stores the difference into `destination` and returns its size. |
| `SPOP key [count]` | Removes and returns one or more random members from the set. |
| `SRANDMEMBER key [count]` | Returns one or more random members without removing them. Positive `count`: unique members. Negative `count`: may repeat. |
| `SMOVE source destination member` | Moves a member from one set to another. Returns 1 on success, 0 if the member did not exist in source. Removal and insertion are separate steps, so a concurrent reader can briefly see the member in neither set — see [Concurrency model](#concurrency-model). |
| `SSCAN key cursor [MATCH pattern] [COUNT count]` | Iterates a set incrementally: returns the next cursor plus at most `COUNT` members (default 10). The bounded counterpart of `SMEMBERS`. |

---

## Sorted Set

An ordered collection where each member has a numeric score. Members are unique; scores need not be. Members are always returned in ascending score order.

| Command | Description |
|---|---|
| `ZADD key [NX\|XX] [CH] [INCR] score member [score member ...]` | Adds members with scores. `NX`: only add, never update. `XX`: only update, never add. `CH`: count changed elements (updated + added) instead of just added. `INCR`: add the score to the existing score instead of replacing it. |
| `ZREM key member [member ...]` | Removes members from the sorted set. Returns the number of members removed. |
| `ZINCRBY key increment member` | Adds `increment` to the score of `member`. Creates the member with score `increment` if it does not exist. |
| `ZRANGE key start stop [WITHSCORES]` | Returns members between rank `start` and `stop` (0-based, ascending). Add `WITHSCORES` to include scores. |
| `ZREVRANGE key start stop [WITHSCORES]` | Same as `ZRANGE`, but returns members in descending score order. |
| `ZRANGEBYSCORE key min max [WITHSCORES] [LIMIT offset count]` | Returns members with scores between `min` and `max`. Use `-inf` and `+inf` for open bounds. Use `(min` for exclusive lower bound. |
| `ZREVRANGEBYSCORE key max min [WITHSCORES] [LIMIT offset count]` | Same, but from high score to low. |
| `ZSCORE key member` | Returns the score of a member, or nil if the member does not exist. |
| `ZMSCORE key member [member ...]` | Returns the scores of multiple members. Non-existent members return nil. |
| `ZRANK key member` | Returns the 0-based rank of a member in ascending score order. Returns nil if the member does not exist. |
| `ZREVRANK key member` | Returns the rank in descending score order. |
| `ZCARD key` | Returns the number of members in the sorted set. |
| `ZCOUNT key min max` | Returns the number of members with scores between `min` and `max`. |
| `ZSCAN key cursor [MATCH pattern] [COUNT count]` | Iterates a sorted set incrementally: returns the next cursor plus at most `COUNT` `member, score` pairs (default 10). Ordered by member rather than by score, because the cursor is a position and only the member ordering survives a score changing mid-iteration. |

### Example: leaderboard

```bash
ZADD leaderboard 1500 alice 2200 bob 980 carol
ZREVRANGE leaderboard 0 2 WITHSCORES
# ["bob", "2200", "alice", "1500", "carol", "980"]

ZINCRBY leaderboard 300 carol
ZREVRANK leaderboard carol    # 2 → 1 (moved up)
```

---

## JSON

A native JSON document type — no RedisJSON module needed. Documents are stored parsed, so path reads and partial updates never re-serialize the whole value, and only the change travels to replicas, the AOF, and connected browsers.

| Command | Description |
|---|---|
| `JSET key path value` | Set JSON at a path. `$` is the whole document, `$.user.name` a nested field, `$.items[2].qty` an array element. Intermediate objects are auto-created; array indices must exist. `value` must be valid JSON text. |
| `JGET key [path]` | Read the JSON at a path (default `$`), serialized. Returns nil when the key or path does not exist. Object keys serialize in sorted order — deterministic output. |
| `JMERGE key patch` | RFC 7386 JSON Merge Patch against the whole document: objects merge recursively, `null` fields are removed, arrays and scalars are replaced. A `null` patch deletes the key. Creates the document if missing. |

Paths address exactly one location — wildcards, slices, and filters are not supported. `TYPE` reports `json`.

### Example: partial updates

```bash
JSET doc:42 $ '{"title":"Draft","meta":{"views":0,"draft":true}}'
JGET doc:42 $.meta.views          # "0"
JSET doc:42 $.meta.views 17       # only this field changes
JMERGE doc:42 '{"title":"Final","meta":{"draft":null}}'
JGET doc:42                       # {"meta":{"views":17},"title":"Final"}
```

The browser SDK exposes the same commands as [`jset` / `jget` / `jmerge`](/browser/api-reference#json-documents) with `JSON.stringify`/`parse` handled for you — a `JMERGE` from any client updates every connected browser's local document.

In live queries (`QSUB` / `useKeys`), JSON keys arrive as `["json", document]` — the document travels with the notification, so no follow-up `JGET` is needed.

---

## Rate Limiting

A built-in sliding-window rate limiter — no INCR+EXPIRE races, no Lua scripts. Internally a limiter key stores its config plus the timestamps of allowed attempts inside the window (type name: `ratelimit`). Denied attempts are not recorded, so a client hammering a full limiter does not push its own recovery further away.

| Command | Description |
|---|---|
| `RLSET key limit window` | Configure a limiter: at most `limit` attempts per `window` seconds. Reconfiguring in place keeps already-recorded attempts. Limiters created with `RLSET` persist until `DEL`/`EXPIRE`. |
| `RLCHECK key [limit window]` | Record an attempt. Returns a 3-element array: `[allowed (1\|0), remaining, retry_after_ms]`. With the optional `limit window` pair, the limiter is created on first use — ideal for per-IP or per-user keys where a separate `RLSET` round-trip per key is impractical. Auto-created limiters self-clean: they expire one window after the last attempt. Bare `RLCHECK` on an unconfigured key returns an error. |

The reply maps directly onto standard HTTP rate-limit headers: `remaining` → `X-RateLimit-Remaining`, `retry_after_ms` → `Retry-After`.

### Example: per-IP request limiting

```bash
# 100 requests per minute per client IP — one command per request,
# limiter auto-created on the first attempt and self-cleans when idle.
RLCHECK ip:203.0.113.7 100 60
# 1) (integer) 1        allowed
# 2) (integer) 99       remaining in window
# 3) (integer) 0        retry_after_ms

# ...101st request within the minute:
# 1) (integer) 0
# 2) (integer) 0
# 3) (integer) 58211    → Retry-After: 59
```

### Example: named app-level limiter

```bash
RLSET login:alice 5 300      # 5 login attempts per 5 minutes
RLCHECK login:alice          # check + record one attempt
```

Replication note: `RLSET` config replicates to AOF and replicas; recorded attempts are transient and deliberately do not (streaming every check would flood the write log for state that expires within one window).

---

## Sync Scoping (WebSocket only)

Controls which keys a WebSocket connection receives pushes for and may operate on. Full guide: [Sync Scopes](/server/sync-scopes).

| Command | Description |
|---|---|
| `SYNC` | Returns this connection's current grants, in `r=`/`rw=` notation. |
| `SYNC TOKEN token` | Sets scopes from a token signed with `RECACHED_SYNC_SECRET` (HMAC-SHA256). Required before any key access when the secret is configured (strict mode). Returns the granted entries. |
| `SYNC pattern [pattern ...]` | Sets scopes directly from glob patterns. Only available when no sync secret is configured — a bandwidth filter, not a security boundary. |

Each scope entry may state its access: `r=catalog:*` is read-only, `rw=cart:42:*` is read-write, and a bare pattern is read-write. A write to a read-only key is refused with `-NOSCOPE key '...' is read-only on this connection`. Access is checked per key, so `SINTERSTORE` needs write only on its destination.

On the TCP port, `SYNC` returns an error — backend connections are trusted and unscoped.

### Deduplicated replay envelope

| Command | Description |
|---|---|
| `DEDUP client-id id command args...` | Wraps a write with a per-client monotonic id. If `id` is at or below the highest id already applied for `client-id`, the write is skipped and the reply is `+DUP`. Client ids are 1–64 characters and should be unguessable (the SDK uses `crypto.randomUUID()`). Scope checks, replica rejection, persistence health, and metrics apply to the wrapped command. High-water marks are persisted beside the snapshot and swept after 24 h idle. |

---

## Live Queries (WebSocket only)

A live query delivers the current state of every key matching a glob pattern, then streams every subsequent change to matching keys — initial state plus diffs, not fire-and-forget events. This is the primitive behind reactive UI bindings.

| Command | Description |
|---|---|
| `QSUB pattern` | Subscribe. The reply is `["qstate", pattern, key, value, ...]` — the current state of every live key matching the pattern as flat pairs. Afterwards, every mutation to a matching key — including keys created later — arrives as a `["keychange", key, value]` push; deletions arrive with a nil value. Initial state is capped at 10 000 keys. Up to 64 live queries per connection. |
| `QUNSUB [pattern]` | Drop one live query, or all of them without an argument. |

```bash
QSUB cart:42:*
# 1) "qstate"
# 2) "cart:42:*"
# 3) "cart:42:item:9"     initial state…
# 4) "2"
# …then, when the server (or any client) writes cart:42:item:12:
# ["keychange", "cart:42:item:12", "1"]
```

Under strict sync scoping, `QSUB` patterns must sit inside the connection's granted scopes — a grant of `cart:42:*` covers `QSUB cart:42:*` and narrower prefix patterns. Live-query pushes never interfere with `WATCH` transactions (they travel on a separate internal channel).

### Value shapes

Strings arrive as a bulk string and deletions as nil. Collections arrive **type-tagged** — an array
whose first element names the type — so a subscriber can rebuild the value without a follow-up read:

```text
hash  →  ["hash", field, value, ...]     fields sorted
list  →  ["list", element, ...]          head to tail
set   →  ["set", member, ...]
zset  →  ["zset", member, score, ...]    ascending score
json  →  ["json", document]
```

The tag is what makes the payload unambiguous: a four-element array would otherwise be
indistinguishable between a list of four items and a hash of two pairs. Ordering is deterministic, so
two clients receiving the same notification build identical local state.

Each notification carries the **complete** current value, so the receiver replaces the key rather than
merging — which is what allows a removed member to propagate.

`FLUSHDB` is announced as a single sentinel per subscribed pattern — a `keychange` whose key is the
pattern and whose value is nil — meaning "every key matching this pattern is gone". Announcing each
deleted key would mean one frame per key in the keyspace for one command. The browser SDK expands the
sentinel locally; a hand-written client should do the same.

::: warning Changed in 0.2.2
Before 0.2.2 collections arrived as a bare type name (`"hash"`), and subscribers had to follow up with
`HGETALL`/`LRANGE`. Server and SDK are released in lockstep — run matching versions, since a 0.2.1
client ignores the new shape.
:::

---

## Transactions

Transactions queue commands and run them as one batch at `EXEC`, with optimistic locking through `WATCH`. The server reserves the union of queued write keys until every command and its persistence, replication, and notification effects finish. After `EXEC`, the full result set is broadcast to WebSocket clients.

::: warning Writer isolation is not reader isolation
A conflicting server write cannot interleave between queued commands. Reads do not acquire these ordering barriers, so another connection may observe intermediate results between two commands in the transaction.

- **Nothing runs before `EXEC`.** Queued commands are held until execution.
- **All or nothing on a queue error.** If any command fails to queue, `EXEC` runs none of them and returns `EXECABORT`.
- **`WATCH` closes the check-to-write race.** `EXEC` reserves watched keys before checking invalidation. If one changed after `WATCH`, it runs nothing and returns a nil array.

Use `WATCH` for compare-and-swap. Do not use `MULTI`/`EXEC` when concurrent readers must observe several keys changing atomically.
:::

| Command | Description |
|---|---|
| `MULTI` | Begins a transaction. Subsequent commands are queued, not executed. Returns `OK`. |
| `EXEC` | Executes all queued commands. Returns an array of results, one per queued command — or a nil array if a `WATCH`ed key changed since `WATCH` was issued (optimistic-lock abort). |
| `DISCARD` | Abandons the transaction queue. Returns `OK`. Also clears any `WATCH`ed keys. |

### Example

```bash
MULTI
SET counter 0
INCR counter
INCR counter
EXEC
# 1) OK
# 2) 1
# 3) 2
```

Optimistic locking: `WATCH key [key ...]` before `MULTI` marks those keys. If any watched key is modified by **any** client before `EXEC`, the transaction is aborted and `EXEC` returns a nil array (Redis `WATCH`/`MULTI`/`EXEC` CAS semantics). This works over both the TCP (6379) and WebSocket (6380) ports. `EXEC` and `DISCARD` both clear all watches. Over WebSocket, `WATCH` *additionally* pushes live keychange notifications — see [Observable Keys](#observable-keys) below.

---

## Pub/Sub

Publish/subscribe messaging. Clients can subscribe to channels (exact match) or patterns (glob). Published messages are delivered to all matching subscribers.

Pub/Sub works over both TCP (port 6379) and WebSocket (port 6380).

| Command | Description |
|---|---|
| `SUBSCRIBE channel [channel ...]` | Subscribes the client to one or more channels. The client enters pub/sub mode and can only use pub/sub commands until it unsubscribes. |
| `UNSUBSCRIBE [channel ...]` | Unsubscribes from the given channels. With no arguments, unsubscribes from all channels. |
| `PSUBSCRIBE pattern [pattern ...]` | Subscribes to channels matching a glob pattern. `*` matches any sequence of bytes, `?` matches exactly one byte. **Character classes (`[abc]`) are not supported** — brackets match literally. Patterns are capped at 1,024 bytes. |
| `PUNSUBSCRIBE [pattern ...]` | Unsubscribes from pattern subscriptions. With no arguments, unsubscribes from all patterns. |
| `PUBLISH channel message` | Publishes a message to all subscribers of the given channel and all clients with matching pattern subscriptions. Returns the number of clients that received the message. |
| `PUBSUB CHANNELS [pattern]` | Channels with at least one subscriber. Without a pattern, all of them; with one, those whose name matches. Pattern subscriptions are never listed here — nobody is subscribed to a channel named `news.*`. |
| `PUBSUB NUMSUB [channel ...]` | Flat `[channel, count, channel, count, ...]`. A channel with no subscribers reports `0` rather than being dropped, so the reply can be read by position against the channels you asked about. Pattern subscribers are not counted; that is `NUMPAT`'s job. |
| `PUBSUB NUMPAT` | The number of **distinct** patterns under subscription. Two clients on `news.*` are one pattern, not two. |

`PUBSUB` answers from the live subscriber registry, so it sees exactly what `PUBLISH` would deliver to. A channel disappears from `CHANNELS` when its last subscriber leaves — there is no lingering empty channel, because a channel is nothing more than its subscribers.

`PUBSUB SHARDCHANNELS` and `PUBSUB SHARDNUMSUB` are refused. This is the one place these commands deliberately diverge from Redis, which answers both with an empty array even in standalone mode. It can afford to: `SSUBSCRIBE` and `SPUBLISH` work there, so an empty array honestly means "no shard channels are subscribed yet". Recached implements neither, so the same empty array would invite a client to call `SSUBSCRIBE` and fail. An error says what is true — the question does not apply here.

`PUBSUB` enumerates what every other connection is subscribed to, so it is treated as an admin command and rejected on scope-limited WebSocket connections — the same line `KEYS` sits on. A scoped connection can still `SUBSCRIBE` and `PUBLISH` freely; channels are outside the scope system. Naming a channel and listing them all are different powers.

### Example

```typescript
// Subscriber (Node.js)
const sub = new Redis('redis://127.0.0.1:6379')
await sub.subscribe('events:orders')
sub.on('message', (channel, message) => {
  console.log(`${channel}: ${message}`)
})

// Publisher
const pub = new Redis('redis://127.0.0.1:6379')
await pub.publish('events:orders', JSON.stringify({ id: 123, status: 'shipped' }))
// events:orders: {"id":123,"status":"shipped"}
```

---

## Replication

Replication topology is set at startup with [`RECACHED_REPLICAOF`](/server/configuration#environment-variable-reference). One runtime command exists, for promoting a replica during failover.

| Command | Description |
|---|---|
| `REPLICAOF NO ONE` | Promotes this replica to a primary: it stops following its upstream and begins accepting writes. This is the **only** accepted form — pointing a running server at a new primary (`REPLICAOF host port`) is rejected with an error. To re-point a server, restart it with a different `RECACHED_REPLICAOF`. |

Promotion is always manual. Fence the old primary before sending `REPLICAOF NO ONE`; otherwise both nodes can accept writes after a partition. `RECACHED_FAILOVER_TIMEOUT` is deprecated and ignored. See [manual failover](/server/configuration#manual-failover).

Replication uses the versioned `RCP1` stream. A replica reconnects with its primary run id and last applied offset. The primary sends missing frames from its bounded backlog when available and falls back to a full snapshot otherwise.

---

## Persistence

Snapshot commands write the in-memory store to disk in MessagePack format. The snapshot path and autosave interval are controlled by [`RECACHED_SAVE_PATH` and `RECACHED_SAVE_INTERVAL`](/server/configuration#environment-variable-reference).

| Command | Description |
|---|---|
| `SAVE` | Synchronously creates a snapshot/AOF checkpoint. Returns `OK` only after the snapshot is durable and the covered AOF is truncated; returns `MISCONF` on failure. |
| `BGSAVE` | Starts the same checkpoint in a background task and returns immediately. Reads continue, but writes pause while the checkpoint holds its all-write barrier. |
| `LASTSAVE` | Returns the Unix timestamp (seconds) of the most recent successful snapshot. Returns the server start time if no save has completed yet. |

### Example

```bash
# Trigger a background save and check when it completed
BGSAVE          # +Background saving started
# ... time passes ...
LASTSAVE        # (integer) 1746794400
```

```bash
# Force a synchronous save
SAVE            # +OK
```

---

## Observable Keys

`WATCH` and `UNWATCH` serve two roles in Recached:

1. **Optimistic locking (both transports).** Over TCP (6379) and WebSocket (6380), `WATCH` participates in `MULTI`/`EXEC` exactly like Redis: if a watched key changes before `EXEC`, the transaction aborts (nil array). See [Transactions](#transactions).
2. **Live change notifications (WebSocket only).** Over WebSocket, `WATCH` *additionally* subscribes the connection to keychange pushes: whenever a watched key is mutated by any client, the server sends a push frame to every watching WS connection. TCP connections receive no such push (it would violate the request/response protocol) — they use `WATCH` purely for the CAS guarantee above.

| Command | Description |
|---|---|
| `WATCH key [key ...]` | Marks the given key(s) for optimistic locking, and (over WebSocket) registers the connection for keychange push notifications. Not allowed once `MULTI` has started. |
| `UNWATCH [key ...]` | Stops watching the given keys. With no arguments, clears all watches for this connection. `EXEC` and `DISCARD` also clear all watches. |

### Push message format

When a watched key changes, the server sends a RESP array:

```
["keychange", "key-name", "new-value-or-type-hint"]
```

- For string keys: the third element is the current value.
- For complex types (hash, list, set, sorted set): the third element is the type name (`hash`, `list`, `set`, `zset`). Re-fetch the full value with `HGETALL`, `LRANGE`, `SMEMBERS`, or `ZRANGE`.
- For deleted keys: the third element is nil (`$-1\r\n`).

### Raw WebSocket example

```javascript
const ws = new WebSocket('ws://127.0.0.1:6380')

ws.onopen = () => {
  // Watch a key — send RESP directly
  ws.send('*2\r\n$5\r\nWATCH\r\n$12\r\ncart:user:42\r\n')
}

ws.onmessage = ({ data }) => {
  // Parse RESP push: ["keychange", "cart:user:42", "3"]
  console.log('Key changed:', data)
}

// Stop watching this key
ws.send('*2\r\n$7\r\nUNWATCH\r\n$12\r\ncart:user:42\r\n')

// Stop watching all keys
ws.send('*1\r\n$7\r\nUNWATCH\r\n')
```

When using the `recached-edge` WASM client, `WATCH`/`UNWATCH` are wrapped in the `cache.watch()` / `cache.unwatch()` TypeScript API — you do not need to handle raw RESP.
