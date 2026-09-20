//! Per-connection bookkeeping and command metrics: the client registry
//! CLIENT LIST reads, and the counters every executed command feeds.

use crate::*;

/// Counters mirrored out of the `metrics` registry so `INFO` can read them.
///
/// `metrics::Counter` and `Gauge` handles are write-only — there is no way to
/// read a recorded value back out — so every number `INFO` reports from the
/// registry needs a plain atomic alongside it. These are the only ones INFO
/// needs; the rest of its fields come from the store or `ServerState`.
pub(crate) static STAT_CONNECTIONS_TOTAL: AtomicU64 = AtomicU64::new(0);

pub(crate) static STAT_CONNECTIONS_ACTIVE: AtomicI64 = AtomicI64::new(0);

pub(crate) static STAT_COMMANDS_TOTAL: AtomicU64 = AtomicU64::new(0);

pub(crate) static STAT_KEYSPACE_HITS: AtomicU64 = AtomicU64::new(0);

pub(crate) static STAT_KEYSPACE_MISSES: AtomicU64 = AtomicU64::new(0);

/// RAII guard that tracks an active connection. Increments on creation,
/// decrements when dropped (i.e. when the handler future completes), and
/// keeps the `CLIENT LIST` registry in step with both.
pub(crate) struct ConnectionGuard {
    pub(crate) id: u64,
}

impl ConnectionGuard {
    pub(crate) fn new(kind: &'static str, meta: ClientMeta) -> Self {
        counter!("recached_connections_total", "type" => kind).increment(1);
        gauge!("recached_connections_active").increment(1.0);
        STAT_CONNECTIONS_TOTAL.fetch_add(1, Ordering::Relaxed);
        STAT_CONNECTIONS_ACTIVE.fetch_add(1, Ordering::Relaxed);
        let id = meta.id;
        publish_client(meta);
        Self { id }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        gauge!("recached_connections_active").decrement(1.0);
        STAT_CONNECTIONS_ACTIVE.fetch_sub(1, Ordering::Relaxed);
        CLIENTS
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.id);
    }
}

// ── Client registry ───────────────────────────────────────────────────────────

/// What `CLIENT INFO` and `CLIENT LIST` report about one live connection.
///
/// The connection task owns the authoritative copy and republishes it whenever
/// a field changes — a name is set, a library identifies itself, the protocol
/// is renegotiated. Publishing on change rather than per command keeps the
/// registry's write lock off the hot path: these events happen a handful of
/// times per connection, commands happen millions of times.
#[derive(Clone, Debug)]
pub(crate) struct ClientMeta {
    pub(crate) id: u64,
    /// Peer address, or empty when the listener could not report one.
    pub(crate) addr: String,
    pub(crate) laddr: String,
    pub(crate) name: String,
    pub(crate) lib_name: String,
    pub(crate) lib_ver: String,
    pub(crate) since: SystemTime,
    pub(crate) resp: u8,
    pub(crate) sub: usize,
    pub(crate) psub: usize,
    /// Set by `CLIENT DELTA ON`: this connection accepts `keydelta` frames in
    /// place of a full `keychange` where the mutation has a compact form.
    pub(crate) deltas: bool,
}

impl ClientMeta {
    pub(crate) fn new(id: u64, addr: String, laddr: String) -> Self {
        Self {
            id,
            addr,
            laddr,
            name: String::new(),
            lib_name: String::new(),
            lib_ver: String::new(),
            since: SystemTime::now(),
            resp: 2,
            deltas: false,
            sub: 0,
            psub: 0,
        }
    }

    /// One line in Redis's `CLIENT LIST` format: space-separated `key=value`.
    ///
    /// Only fields Recached can answer truthfully are emitted. Redis also
    /// reports buffer sizes, file descriptors and an event mask; inventing
    /// plausible numbers for those would be worse than leaving them out,
    /// because a client cannot tell a made-up `omem` from a real one. Parsers
    /// read this format key by key and skip what they do not recognise, so a
    /// shorter line is a supported line.
    pub(crate) fn render(&self) -> String {
        let age = self.since.elapsed().unwrap_or_default().as_secs();
        format!(
            "id={} addr={} laddr={} name={} age={} idle=0 flags=N db=0 \
             sub={} psub={} multi=-1 resp={} lib-name={} lib-ver={}",
            self.id,
            self.addr,
            self.laddr,
            self.name,
            age,
            self.sub,
            self.psub,
            self.resp,
            self.lib_name,
            self.lib_ver,
        )
    }
}

/// Live connections, keyed by id. A `BTreeMap` so `CLIENT LIST` comes out in
/// connection order rather than an arbitrary one that shuffles between calls.
pub(crate) static CLIENTS: std::sync::LazyLock<std::sync::RwLock<BTreeMap<u64, ClientMeta>>> =
    std::sync::LazyLock::new(Default::default);

pub(crate) fn publish_client(meta: ClientMeta) {
    CLIENTS
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .insert(meta.id, meta);
}

pub(crate) fn client_list_lines() -> String {
    let clients = CLIENTS.read().unwrap_or_else(|e| e.into_inner());
    let mut out = String::new();
    for meta in clients.values() {
        out.push_str(&meta.render());
        out.push('\n');
    }
    out
}

pub(crate) fn command_name(cmd: &Command) -> &'static str {
    match cmd {
        Command::Ping(_) => "ping",
        Command::Auth(_) => "auth",
        Command::Hello(_) => "hello",
        Command::Info(_) => "info",
        Command::Quit => "quit",
        Command::Client(_) => "client",
        Command::Config(_) => "config",
        Command::CommandQuery(_) => "command",
        Command::Cluster(_) => "cluster",
        Command::Module(_) => "module",
        Command::PubSub(_) => "pubsub",
        Command::Memory(_) | Command::MemoryUsage(_) => "memory",
        Command::Get(_) => "get",
        Command::ESet(_, _) => "eset",
        Command::Set(_, _, _) => "set",
        Command::Del(_) => "del",
        Command::Unlink(_) => "unlink",
        Command::Append(_, _) => "append",
        Command::Strlen(_) => "strlen",
        Command::GetRange(_, _, _) => "getrange",
        Command::GetSet(_, _) => "getset",
        Command::MGet(_) => "mget",
        Command::MSet(_) => "mset",
        Command::SetNx(_, _) => "setnx",
        Command::SetEx(_, _, _) => "setex",
        Command::PSetEx(_, _, _) => "psetex",
        Command::Incr(_) => "incr",
        Command::Decr(_) => "decr",
        Command::IncrBy(_, _) => "incrby",
        Command::DecrBy(_, _) => "decrby",
        Command::Expire(_, _) => "expire",
        Command::PExpire(_, _) => "pexpire",
        Command::ExpireAt(_, _) => "expireat",
        Command::PExpireAt(_, _) => "pexpireat",
        Command::Ttl(_) => "ttl",
        Command::PTtl(_) => "pttl",
        Command::Persist(_) => "persist",
        Command::Exists(_) => "exists",
        Command::Keys(_) => "keys",
        Command::Scan(_, _, _) => "scan",
        Command::DbSize => "dbsize",
        Command::FlushDb => "flushdb",
        Command::Rename(_, _) => "rename",
        Command::Type(_) => "type",
        Command::HSet(_, _) => "hset",
        Command::HGet(_, _) => "hget",
        Command::HGetAll(_) => "hgetall",
        Command::HDel(_, _) => "hdel",
        Command::HKeys(_) => "hkeys",
        Command::HVals(_) => "hvals",
        Command::HLen(_) => "hlen",
        Command::HIncrBy(_, _, _) => "hincrby",
        Command::HIncrByFloat(_, _, _) => "hincrbyfloat",
        Command::HExists(_, _) => "hexists",
        Command::HSetNx(_, _, _) => "hsetnx",
        Command::HMGet(_, _) => "hmget",
        Command::HScan(_, _) => "hscan",
        Command::LPush(_, _) => "lpush",
        Command::RPush(_, _) => "rpush",
        Command::LPushX(_, _) => "lpushx",
        Command::RPushX(_, _) => "rpushx",
        Command::LPop(_, _) => "lpop",
        Command::RPop(_, _) => "rpop",
        Command::LRange(_, _, _) => "lrange",
        Command::LLen(_) => "llen",
        Command::LIndex(_, _) => "lindex",
        Command::LSet(_, _, _) => "lset",
        Command::LRem(_, _, _) => "lrem",
        Command::LTrim(_, _, _) => "ltrim",
        Command::SAdd(_, _) => "sadd",
        Command::SMembers(_) => "smembers",
        Command::SRem(_, _) => "srem",
        Command::SCard(_) => "scard",
        Command::SIsMember(_, _) => "sismember",
        Command::SMIsMember(_, _) => "smismember",
        Command::SInter(_) => "sinter",
        Command::SInterStore(_, _) => "sinterstore",
        Command::SUnion(_) => "sunion",
        Command::SUnionStore(_, _) => "sunionstore",
        Command::SDiff(_) => "sdiff",
        Command::SDiffStore(_, _) => "sdiffstore",
        Command::SPop(_, _) => "spop",
        Command::SRandMember(_, _) => "srandmember",
        Command::SMove(_, _, _) => "smove",
        Command::SScan(_, _) => "sscan",
        Command::ZAdd(_, _, _) => "zadd",
        Command::ZRange(_, _, _, _) => "zrange",
        Command::ZRevRange(_, _, _, _) => "zrevrange",
        Command::ZRangeByScore(_, _, _, _, _) => "zrangebyscore",
        Command::ZRevRangeByScore(_, _, _, _, _) => "zrevrangebyscore",
        Command::ZScore(_, _) => "zscore",
        Command::ZMScore(_, _) => "zmscore",
        Command::ZRank(_, _) => "zrank",
        Command::ZRevRank(_, _) => "zrevrank",
        Command::ZRem(_, _) => "zrem",
        Command::ZCard(_) => "zcard",
        Command::ZIncrBy(_, _, _) => "zincrby",
        Command::ZCount(_, _, _) => "zcount",
        Command::ZScan(_, _) => "zscan",
        Command::Multi => "multi",
        Command::Exec => "exec",
        Command::Discard => "discard",
        Command::Subscribe(_) => "subscribe",
        Command::Unsubscribe(_) => "unsubscribe",
        Command::PSubscribe(_) => "psubscribe",
        Command::PUnsubscribe(_) => "punsubscribe",
        Command::Publish(_, _) => "publish",
        Command::Watch(_) => "watch",
        Command::Unwatch(_) => "unwatch",
        Command::Save => "save",
        Command::BgSave => "bgsave",
        Command::LastSave => "lastsave",
        Command::ReplicaOfNoOne => "replicaof",
        Command::JSet(_, _, _) => "jset",
        Command::JGet(_, _) => "jget",
        Command::JMerge(_, _) => "jmerge",
        Command::RlSet(_, _, _) => "rlset",
        Command::RlCheck(_, _) => "rlcheck",
        Command::Sync(_) => "sync",
        Command::QSub(_) => "qsub",
        Command::QUnsub(_) => "qunsub",
        // Metrics count the wrapped command, not the envelope.
        Command::Dedup(_, _, inner) => command_name(inner),
        Command::Unknown(_) => UNKNOWN_COMMAND,
    }
}

/// Per-command counter handles, resolved through the metrics registry once and
/// then reused — the registry lookup (key construction + shard lock) is too
/// expensive to pay on every command. Keyed by the `&'static str` from
/// `command_name`.
///
/// Built in one shot from the command catalog on first use, which is after
/// `main` has installed the recorder, and never written again. It used to be an
/// `RwLock<HashMap>` filled in as each command was first seen, which cost a
/// read-lock acquisition on *every command on every connection* — one
/// contended atomic in the hot path of a server whose whole job is throughput —
/// and `.unwrap()`ed the lock, so a single panic anywhere holding it would
/// poison the lock and make every subsequent command panic for the life of the
/// process. An immutable map needs no lock and cannot be poisoned.
pub(crate) static CMD_COUNTERS: std::sync::LazyLock<HashMap<&'static str, metrics::Counter>> =
    std::sync::LazyLock::new(|| {
        core_engine::catalog::CATALOG
            .iter()
            .map(|spec| {
                (
                    spec.name,
                    counter!("recached_commands_total", "command" => spec.name),
                )
            })
            .chain(std::iter::once((
                UNKNOWN_COMMAND,
                counter!("recached_commands_total", "command" => UNKNOWN_COMMAND),
            )))
            .collect()
    });

/// Label used for a command the parser did not recognise. Not a catalog row, so
/// it is registered alongside them.
pub(crate) const UNKNOWN_COMMAND: &str = "unknown";

pub(crate) fn record_command(name: &'static str) {
    STAT_COMMANDS_TOTAL.fetch_add(1, Ordering::Relaxed);
    match CMD_COUNTERS.get(name) {
        Some(c) => c.increment(1),
        // A name `command_name` can produce but the catalog does not list.
        // `command_name_labels_are_all_pre_registered` fails CI if that ever
        // happens, so this is a correctness backstop, not a routine path.
        None => counter!("recached_commands_total", "command" => name).increment(1),
    }
}

pub(crate) static KEYSPACE_HITS: std::sync::LazyLock<metrics::Counter> =
    std::sync::LazyLock::new(|| counter!("recached_keyspace_hits_total"));

pub(crate) static KEYSPACE_MISSES: std::sync::LazyLock<metrics::Counter> =
    std::sync::LazyLock::new(|| counter!("recached_keyspace_misses_total"));

/// Executes `cmd`, recording metrics and the dirty counter. Takes the command
/// by value — the hot path hands it straight to the store without a clone;
/// callers that still need the command afterwards (write fan-out) clone first.
pub(crate) fn execute_and_record(store: &KeyValueStore, cmd: Command) -> Value {
    execute_and_record_with_evictions(store, cmd).0
}

/// Execute and retain implicit capacity evictions for the propagation layer.
/// An error can still carry evictions when a multi-key insert exhausted the
/// eligible victim set after removing some keys, so those removals mark the
/// store dirty and must not be discarded with the failed command response.
pub(crate) fn execute_and_record_with_evictions(
    store: &KeyValueStore,
    cmd: Command,
) -> (Value, Vec<String>) {
    let name = command_name(&cmd);
    let is_write = is_write_command(&cmd);
    let is_get = matches!(cmd, Command::Get(_));
    let started = std::time::Instant::now();
    let (response, evicted) = store.execute_reporting(cmd);
    histogram!("recached_command_duration_seconds", "command" => name)
        .record(started.elapsed().as_secs_f64());
    record_command(name);
    if matches!(response, Value::Error(_)) {
        counter!("recached_command_errors_total", "command" => name).increment(1);
        if !evicted.is_empty() {
            store.mark_dirty();
        }
    } else if is_write {
        store.mark_dirty();
    }
    if is_get {
        match &response {
            Value::BulkString(Some(_)) => {
                KEYSPACE_HITS.increment(1);
                STAT_KEYSPACE_HITS.fetch_add(1, Ordering::Relaxed);
            }
            Value::BulkString(None) => {
                KEYSPACE_MISSES.increment(1);
                STAT_KEYSPACE_MISSES.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }
    (response, evicted)
}

// ── TCP listeners ─────────────────────────────────────────────────────────────

// TCP mutation broadcasts use id=0; WS/TCP pubsub connections get ids ≥ 1.
pub(crate) static NEXT_CONN_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_conn_id() -> u64 {
    NEXT_CONN_ID.fetch_add(1, Ordering::Relaxed)
}

// ── pub/sub ───────────────────────────────────────────────────────────────────
