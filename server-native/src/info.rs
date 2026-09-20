//! Server introspection: INFO, CLIENT, CONFIG, COMMAND, CLUSTER, MODULE,
//! MEMORY and PUBSUB — everything reporting on the server rather than the data.

use crate::*;

/// Redis compatibility level advertised as `redis_version`.
///
/// Clients feature-gate on this field, so it cannot be Recached's own version:
/// a library seeing `redis_version:0.2.3` concludes the server predates
/// everything and disables features it could safely use. 6.2 is the honest
/// floor — RESP3 and `HELLO` exist there, which Recached implements, while
/// nothing in 7.x that Recached lacks (functions, `OBJECT FREQ`, sharded
/// pub/sub) gets advertised. The real version ships alongside it as
/// `recached_version`, the same split KeyDB and Dragonfly use.
pub(crate) const REDIS_COMPAT_VERSION: &str = "6.2.0";

/// Sections `INFO` reports when called with no arguments.
pub(crate) const DEFAULT_INFO_SECTIONS: &[&str] = &[
    "server",
    "clients",
    "memory",
    "persistence",
    "stats",
    "replication",
    "cluster",
    "keyspace",
    "recached",
];

/// Process-wide startup facts, set once by `main`.
///
/// Threaded through a static rather than the connection-handler signatures:
/// these values are immutable for the life of the process and needed only by
/// `INFO`, and the handlers already carry a long parameter list. Tests that
/// exercise `render_info` build their own `ServerFacts` and never touch this.
pub(crate) static SERVER_FACTS: std::sync::OnceLock<ServerFacts> = std::sync::OnceLock::new();

pub(crate) fn server_facts() -> &'static ServerFacts {
    SERVER_FACTS.get_or_init(ServerFacts::default)
}

/// Startup facts `INFO` reports that are fixed for the life of the process.
/// Captured in `main` once rather than re-read from the environment per call.
#[derive(Clone, Debug)]
pub(crate) struct ServerFacts {
    pub(crate) start: SystemTime,
    /// Random per-process identifier, as Redis reports it: 40 hex chars.
    pub(crate) run_id: String,
    pub(crate) tcp_port: u16,
    pub(crate) ws_port: u16,
    pub(crate) max_connections: usize,
    pub(crate) tls_enabled: bool,
    pub(crate) auth_enabled: bool,
    pub(crate) aof_enabled: bool,
}

impl Default for ServerFacts {
    fn default() -> Self {
        Self {
            start: SystemTime::now(),
            run_id: String::new(),
            tcp_port: 6379,
            ws_port: 6380,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            tls_enabled: false,
            auth_enabled: false,
            aof_enabled: false,
        }
    }
}

pub(crate) fn generate_run_id() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    (0..40)
        .map(|_| std::char::from_digit(rng.random_range(0..16), 16).unwrap_or('0'))
        .collect()
}

/// Replication numbers, resolved by the caller.
///
/// The registry's depth and lag accessors are async (they lock per-replica
/// queues), and `render_info` is a pure synchronous formatter so it stays
/// trivially testable — so the caller awaits them and passes the results in.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ReplInfo {
    pub(crate) connected: usize,
    pub(crate) queue_depth: usize,
    pub(crate) lag_frames: u64,
}

/// Last keyspace walk, refreshed every 5s by the metrics sampler.
///
/// `keyspace_sample()` is O(keyspace). A monitoring agent polling `INFO` once a
/// second must not walk every key each time, so `INFO` reports the sample
/// instead. `u64::MAX` means "not sampled yet" — `INFO` then walks once itself,
/// which only happens in the first few seconds of uptime.
pub(crate) static SAMPLED_KEYS: AtomicU64 = AtomicU64::new(u64::MAX);

pub(crate) static SAMPLED_VOLATILE_KEYS: AtomicU64 = AtomicU64::new(u64::MAX);

pub(crate) static SAMPLED_MEMORY_BYTES: AtomicU64 = AtomicU64::new(u64::MAX);

pub(crate) fn store_sampled_keyspace(sample: KeyspaceSample) {
    SAMPLED_KEYS.store(sample.keys as u64, Ordering::Relaxed);
    SAMPLED_VOLATILE_KEYS.store(sample.volatile_keys as u64, Ordering::Relaxed);
    SAMPLED_MEMORY_BYTES.store(sample.memory_bytes as u64, Ordering::Relaxed);
}

pub(crate) fn sampled_keyspace(store: &KeyValueStore) -> KeyspaceSample {
    let keys = SAMPLED_KEYS.load(Ordering::Relaxed);
    if keys == u64::MAX {
        // Sampler has not run yet — walk once so the first INFO is not blank.
        let sample = store.keyspace_sample();
        store_sampled_keyspace(sample);
        return sample;
    }
    KeyspaceSample {
        keys: keys as usize,
        volatile_keys: SAMPLED_VOLATILE_KEYS.load(Ordering::Relaxed) as usize,
        memory_bytes: SAMPLED_MEMORY_BYTES.load(Ordering::Relaxed) as usize,
    }
}

/// Render bytes the way Redis does for `*_human` fields.
pub(crate) fn human_bytes(bytes: u64) -> String {
    const UNITS: [(&str, f64); 4] = [
        ("G", 1024.0 * 1024.0 * 1024.0),
        ("M", 1024.0 * 1024.0),
        ("K", 1024.0),
        ("B", 1.0),
    ];
    for (suffix, size) in UNITS {
        if bytes as f64 >= size {
            return if suffix == "B" {
                format!("{}B", bytes)
            } else {
                format!("{:.2}{}", bytes as f64 / size, suffix)
            };
        }
    }
    "0B".to_string()
}

pub(crate) fn eviction_policy_name(policy: EvictionPolicy) -> &'static str {
    match policy {
        EvictionPolicy::NoEviction => "noeviction",
        EvictionPolicy::AllKeysLru => "allkeys-lru",
        EvictionPolicy::AllKeysRandom => "allkeys-random",
        EvictionPolicy::VolatileLru => "volatile-lru",
        EvictionPolicy::VolatileTtl => "volatile-ttl",
    }
}

// ── CLIENT / CONFIG / COMMAND ─────────────────────────────────────────────────

/// Wrong-subcommand error in Redis's wording, which some clients match on.
pub(crate) fn unknown_subcommand(container: &str, sub: &str) -> Value {
    Value::Error(format!(
        "ERR Unknown subcommand or wrong number of arguments for '{sub}'. \
         Try {container} HELP."
    ))
}

/// Handle `CLIENT <subcommand>`, mutating this connection's `meta` in place.
///
/// Returns `None` for subcommands Recached does not implement, so the caller
/// can answer with the standard error rather than this function inventing a
/// reply. `SETNAME`, `SETINFO` and the protocol version write through to the
/// registry, which is what makes them visible to another connection's
/// `CLIENT LIST`.
pub(crate) fn handle_client_command(args: &[String], meta: &mut ClientMeta) -> Value {
    let sub = args[0].to_uppercase();
    match (sub.as_str(), args.len()) {
        ("ID", 1) => Value::Integer(meta.id as i64),
        ("INFO", 1) => Value::BulkString(Some(meta.render().into_bytes())),
        ("LIST", 1) => Value::BulkString(Some(client_list_lines().into_bytes())),
        ("GETNAME", 1) => {
            if meta.name.is_empty() {
                Value::BulkString(None)
            } else {
                Value::BulkString(Some(meta.name.clone().into_bytes()))
            }
        }
        ("SETNAME", 2) => {
            // Redis reserves spaces and newlines because the name is echoed
            // into the space-separated CLIENT LIST format.
            if args[1].contains(' ') || args[1].contains('\n') {
                return Value::Error(
                    "ERR Client names cannot contain spaces, newlines or special characters."
                        .to_string(),
                );
            }
            meta.name = args[1].clone();
            publish_client(meta.clone());
            Value::SimpleString("OK".to_string())
        }
        // A Recached extension, in the place Redis puts its other
        // per-connection push toggles (`CLIENT TRACKING`, `CLIENT NO-EVICT`).
        // Opt-in because a client that does not understand `keydelta` would
        // ignore the frame and silently hold a stale local copy — worse than
        // an error — so the server keeps sending whole values until asked.
        ("DELTA", 2) => match args[1].to_uppercase().as_str() {
            "ON" => {
                meta.deltas = true;
                publish_client(meta.clone());
                Value::SimpleString("OK".to_string())
            }
            "OFF" => {
                meta.deltas = false;
                publish_client(meta.clone());
                Value::SimpleString("OK".to_string())
            }
            other => Value::Error(format!("ERR CLIENT DELTA expects ON or OFF, got '{other}'")),
        },
        ("SETINFO", 3) => match args[1].to_uppercase().as_str() {
            "LIB-NAME" => {
                meta.lib_name = args[2].clone();
                publish_client(meta.clone());
                Value::SimpleString("OK".to_string())
            }
            "LIB-VER" => {
                meta.lib_ver = args[2].clone();
                publish_client(meta.clone());
                Value::SimpleString("OK".to_string())
            }
            other => Value::Error(format!("ERR Unrecognized option '{other}'")),
        },
        ("HELP", 1) => Value::Array(Some(
            [
                "CLIENT <subcommand>",
                "ID -- Return this connection's identifier.",
                "INFO -- Return information about this connection.",
                "LIST -- Return information about all connections.",
                "GETNAME -- Return this connection's name.",
                "SETNAME <name> -- Set this connection's name.",
                "SETINFO <LIB-NAME|LIB-VER> <value> -- Identify the client library.",
                "DELTA <ON|OFF> -- Receive compact keydelta frames for mutations that have one.",
            ]
            .iter()
            .map(|l| Value::SimpleString(l.to_string()))
            .collect(),
        )),
        // KILL, UNPAUSE, NO-EVICT and friends are administrative operations
        // with real semantics. Answering +OK without performing them would be
        // worse than saying no: a client would believe a connection had been
        // killed or eviction disabled when nothing happened.
        _ => unknown_subcommand("CLIENT", &args.join(" ")),
    }
}

/// The configuration parameters `CONFIG GET` reports, resolved from the values
/// actually in force rather than from a table of defaults.
pub(crate) fn config_parameters(
    facts: &ServerFacts,
    store: &KeyValueStore,
) -> Vec<(&'static str, String)> {
    vec![
        (
            "maxmemory",
            store.max_memory_bytes().unwrap_or(0).to_string(),
        ),
        (
            "maxmemory-policy",
            eviction_policy_name(store.eviction_policy()).to_string(),
        ),
        ("maxclients", facts.max_connections.to_string()),
        ("port", facts.tcp_port.to_string()),
        (
            "tls-port",
            if facts.tls_enabled {
                facts.tcp_port.to_string()
            } else {
                "0".to_string()
            },
        ),
        (
            "appendonly",
            if facts.aof_enabled {
                "yes".into()
            } else {
                "no".into()
            },
        ),
        // Recached has a single keyspace. Clients that SELECT anything other
        // than 0 need to know that before they try.
        ("databases", "1".to_string()),
        // Reported as masked, exactly as Redis does: the presence of a
        // password is not a secret, its value is.
        (
            "requirepass",
            if facts.auth_enabled {
                "*".into()
            } else {
                String::new()
            },
        ),
        ("proto-max-bulk-len", MAX_BULK_STRING_BYTES.to_string()),
        ("timeout", "0".to_string()),
        ("save", String::new()),
    ]
}

/// Handle `CONFIG <subcommand>`.
pub(crate) fn handle_config_command(
    args: &[String],
    facts: &ServerFacts,
    store: &KeyValueStore,
) -> Value {
    let sub = args[0].to_uppercase();
    match sub.as_str() {
        "GET" if args.len() >= 2 => {
            let params = config_parameters(facts, store);
            let mut out = Vec::new();
            for (name, value) in &params {
                if args[1..].iter().any(|pat| glob_match(pat, name)) {
                    out.push(Value::BulkString(Some(name.as_bytes().to_vec())));
                    out.push(Value::BulkString(Some(value.clone().into_bytes())));
                }
            }
            Value::Array(Some(out))
        }
        // Recached reads its configuration from the environment at startup and
        // holds it behind an `Arc` for the life of the process, so there is
        // nothing a runtime SET could change. Saying so is better than
        // returning OK and leaving the operator to discover later that the
        // limit they set never applied.
        "SET" if args.len() >= 3 => Value::Error(format!(
            "ERR CONFIG SET is not supported: Recached is configured at startup. \
             Set '{}' through the environment and restart.",
            args[1]
        )),
        "RESETSTAT" if args.len() == 1 => Value::Error(
            "ERR CONFIG RESETSTAT is not supported: counters are exported to Prometheus, \
             where resetting them would break rate calculations."
                .to_string(),
        ),
        _ => unknown_subcommand("CONFIG", &args.join(" ")),
    }
}

/// Handle `CLUSTER <subcommand>`.
///
/// Recached does not cluster, and this reports that the way Redis does. A
/// `redis-server` that was not started in cluster mode does **not** answer
/// `CLUSTER INFO` with `cluster_enabled:0` — it rejects the whole `CLUSTER`
/// container with this exact sentence, and publishes the flag in `INFO`'s
/// `# Cluster` section instead. Copying the sentence rather than inventing a
/// slot map means a client's "am I clustered" branch takes the same path here
/// as against the server it was written for, and `ERR unknown command` (which
/// is what Recached said before) is the one answer that reads as "too old to
/// ask" rather than "not a cluster".
pub(crate) fn handle_cluster_command(_args: &[String]) -> Value {
    Value::Error("ERR This instance has cluster support disabled".to_string())
}

/// Handle `MODULE <subcommand>`.
///
/// There is no module API, so the loaded-module list is empty — which is a
/// real answer, and the same one a stock `redis-server` gives. `LOAD`,
/// `LOADEX` and `UNLOAD` are refused rather than answered `+OK`, because an
/// operator who believes a module loaded has a harder problem than one who
/// was told no.
pub(crate) fn handle_module_command(args: &[String]) -> Value {
    match (args[0].to_uppercase().as_str(), args.len()) {
        ("LIST", 1) => Value::Array(Some(vec![])),
        ("HELP", 1) => Value::Array(Some(
            [
                "MODULE <subcommand>",
                "LIST -- Return a list of loaded modules. Recached loads none.",
            ]
            .iter()
            .map(|l| Value::SimpleString((*l).to_string()))
            .collect(),
        )),
        _ => unknown_subcommand("MODULE", &args.join(" ")),
    }
}

/// Handle `PUBSUB <subcommand> [arg ...]` against the live subscriber hub.
///
/// Recached has shipped `SUBSCRIBE`, `PSUBSCRIBE` and `PUBLISH` from the start
/// with no way to see any of it: `PUBLISH` returns a delivery count, and that
/// was the only observable. The hub already holds both registries, so these
/// three answers are a read of state that existed all along.
///
/// `SHARDCHANNELS` and `SHARDNUMSUB` are refused rather than answered with the
/// empty array a standalone `redis-server` gives. This is a deliberate
/// divergence: Redis's empty array means "no shard channels are subscribed" on
/// a server where `SSUBSCRIBE` works, and a client reading it would reasonably
/// follow up with one. Recached has no `SSUBSCRIBE` or `SPUBLISH` at all, so
/// the honest answer is that the question does not apply here.
pub(crate) fn handle_pubsub_command(args: &[String], hub: &PubSubHub) -> Value {
    match (args[0].to_uppercase().as_str(), args.len()) {
        // No pattern means every active channel. Redis matches the pattern
        // against channel names with the same globber it uses for keys, and so
        // does this — `glob_match` is the one Recached already applies to
        // PSUBSCRIBE, so a pattern selects here exactly what it would there.
        ("CHANNELS", 1) => Value::Array(Some(
            hub.active_channels()
                .map(|c| Value::BulkString(Some(c.as_bytes().to_vec())))
                .collect(),
        )),
        ("CHANNELS", 2) => Value::Array(Some(
            hub.active_channels()
                .filter(|c| core_engine::store::glob_match(&args[1], c))
                .map(|c| Value::BulkString(Some(c.as_bytes().to_vec())))
                .collect(),
        )),
        // Flat [channel, count, channel, count, ...]. A channel nobody is
        // subscribed to reports 0 rather than being dropped, so a caller that
        // asked about N channels can index the reply by position.
        ("NUMSUB", _) => {
            let mut out = Vec::with_capacity((args.len() - 1) * 2);
            for channel in &args[1..] {
                out.push(Value::BulkString(Some(channel.as_bytes().to_vec())));
                out.push(Value::Integer(hub.subscriber_count(channel)));
            }
            Value::Array(Some(out))
        }
        ("NUMPAT", 1) => Value::Integer(hub.pattern_count()),
        ("HELP", 1) => Value::Array(Some(
            [
                "PUBSUB <subcommand>",
                "CHANNELS [pattern] -- Return the currently active channels.",
                "NUMSUB [channel ...] -- Return the subscriber count per channel.",
                "NUMPAT -- Return the number of distinct subscribed patterns.",
            ]
            .iter()
            .map(|l| Value::SimpleString((*l).to_string()))
            .collect(),
        )),
        _ => unknown_subcommand("PUBSUB", &args.join(" ")),
    }
}

/// Handle `MEMORY <subcommand>` for everything except `USAGE`, which is a key
/// read and goes to the store.
///
/// `DOCTOR`, `STATS`, `PURGE` and `MALLOC-STATS` all describe an allocator
/// Recached does not manage — it holds Rust values in a `DashMap` and has no
/// arena to report on or free. Saying so beats a fabricated report.
pub(crate) fn handle_memory_command(args: &[String]) -> Value {
    match (args[0].to_uppercase().as_str(), args.len()) {
        ("HELP", 1) => Value::Array(Some(
            [
                "MEMORY <subcommand>",
                "USAGE <key> [SAMPLES <count>] -- Bytes held by one key. SAMPLES is accepted \
                 and ignored: the estimate always covers every element.",
            ]
            .iter()
            .map(|l| Value::SimpleString((*l).to_string()))
            .collect(),
        )),
        ("DOCTOR" | "STATS" | "PURGE" | "MALLOC-STATS", 1) => Value::Error(format!(
            "ERR MEMORY {} is not supported: Recached does not manage its own allocator, \
             so it has nothing to report or free. MEMORY USAGE and INFO memory are the \
             measurements it can make.",
            args[0].to_uppercase()
        )),
        _ => unknown_subcommand("MEMORY", &args.join(" ")),
    }
}

/// `COMMAND INFO`'s per-command reply: name, arity, flags, key positions.
pub(crate) fn command_info_entry(spec: &catalog::CommandSpec) -> Value {
    Value::Array(Some(vec![
        Value::BulkString(Some(spec.name.as_bytes().to_vec())),
        Value::Integer(spec.arity as i64),
        Value::Array(Some(
            spec.flags
                .iter()
                .map(|f| Value::SimpleString((*f).to_string()))
                .collect(),
        )),
        Value::Integer(spec.first_key as i64),
        Value::Integer(spec.last_key as i64),
        Value::Integer(spec.step as i64),
        // ACL categories, tips, key specs and subcommands: Redis 7 appends
        // four more elements here. Recached has no ACL system and no
        // subcommand tree to describe, so it reports them empty rather than
        // omitting them — a client indexing element 6 gets an empty list
        // instead of an out-of-range error.
        Value::Array(Some(vec![])),
        Value::Array(Some(vec![])),
        Value::Array(Some(vec![])),
        Value::Array(Some(vec![])),
    ]))
}

/// `COMMAND DOCS`'s per-command reply. RESP2 clients see the same pairs as a
/// flat array, which is how Redis degrades a map on the older protocol.
pub(crate) fn command_docs_entry(spec: &catalog::CommandSpec, protover: u8) -> Value {
    let fields = vec![
        (
            "summary",
            Value::BulkString(Some(spec.summary.as_bytes().to_vec())),
        ),
        ("since", Value::BulkString(Some(b"1.0.0".to_vec()))),
        (
            "group",
            Value::BulkString(Some(spec.group.as_bytes().to_vec())),
        ),
        ("arity", Value::Integer(spec.arity as i64)),
    ];
    map_or_flat(fields, protover)
}

/// RESP3 sends a map; RESP2 has no map type and flattens to alternating
/// key/value entries. Same split `HELLO` already makes.
pub(crate) fn map_or_flat(fields: Vec<(&str, Value)>, protover: u8) -> Value {
    if protover >= 3 {
        Value::Map(
            fields
                .into_iter()
                .map(|(k, v)| (Value::BulkString(Some(k.as_bytes().to_vec())), v))
                .collect(),
        )
    } else {
        let mut flat = Vec::with_capacity(fields.len() * 2);
        for (k, v) in fields {
            flat.push(Value::BulkString(Some(k.as_bytes().to_vec())));
            flat.push(v);
        }
        Value::Array(Some(flat))
    }
}

/// Handle `COMMAND [subcommand]`.
pub(crate) fn handle_command_query(args: &[String], protover: u8) -> Value {
    let Some(sub) = args.first() else {
        // Bare COMMAND: the whole catalog, as COMMAND INFO entries.
        return Value::Array(Some(
            catalog::CATALOG.iter().map(command_info_entry).collect(),
        ));
    };
    match sub.to_uppercase().as_str() {
        "COUNT" if args.len() == 1 => Value::Integer(catalog::CATALOG.len() as i64),
        "LIST" if args.len() == 1 => Value::Array(Some(
            catalog::CATALOG
                .iter()
                .map(|c| Value::BulkString(Some(c.name.as_bytes().to_vec())))
                .collect(),
        )),
        "INFO" => {
            if args.len() == 1 {
                return Value::Array(Some(
                    catalog::CATALOG.iter().map(command_info_entry).collect(),
                ));
            }
            // A name the server does not have replies nil in its slot, so the
            // reply stays positionally aligned with the request.
            Value::Array(Some(
                args[1..]
                    .iter()
                    .map(|n| match catalog::lookup(n) {
                        Some(spec) => command_info_entry(spec),
                        None => Value::Array(None),
                    })
                    .collect(),
            ))
        }
        "DOCS" => {
            let specs: Vec<&catalog::CommandSpec> = if args.len() == 1 {
                catalog::CATALOG.iter().collect()
            } else {
                args[1..]
                    .iter()
                    .filter_map(|n| catalog::lookup(n))
                    .collect()
            };
            // Unknown names are absent from the map rather than nil-filled:
            // COMMAND DOCS is keyed by name, so there is no slot to align.
            let fields: Vec<(&str, Value)> = specs
                .iter()
                .map(|s| (s.name, command_docs_entry(s, protover)))
                .collect();
            map_or_flat(fields, protover)
        }
        _ => unknown_subcommand("COMMAND", &args.join(" ")),
    }
}

/// Build the `INFO` payload for `sections` (empty = the default set).
///
/// The format is load-bearing: `# Section` header, `field:value` lines, CRLF
/// throughout, and a blank line between sections. Every Redis client and
/// monitoring agent parses exactly that shape, so it is covered by tests rather
/// than left to formatting drift. Unknown section names yield no output, which
/// is what Redis does.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_info(
    sections: &[String],
    facts: &ServerFacts,
    store: &KeyValueStore,
    sample: KeyspaceSample,
    is_replica: bool,
    repl: ReplInfo,
    last_save: i64,
    live_queries: u64,
    watched_keys: u64,
) -> String {
    let wanted: Vec<&str> = if sections.is_empty()
        || sections
            .iter()
            .any(|s| s == "all" || s == "everything" || s == "default")
    {
        DEFAULT_INFO_SECTIONS.to_vec()
    } else {
        sections.iter().map(|s| s.as_str()).collect()
    };

    let uptime = facts
        .start
        .elapsed()
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let mut out = String::new();

    for section in wanted {
        let body = match section {
            "server" => {
                format!(
                    "redis_version:{}\r\n\
                     recached_version:{}\r\n\
                     redis_mode:standalone\r\n\
                     os:{}\r\n\
                     arch_bits:{}\r\n\
                     process_id:{}\r\n\
                     run_id:{}\r\n\
                     tcp_port:{}\r\n\
                     recached_ws_port:{}\r\n\
                     recached_tls_enabled:{}\r\n\
                     recached_auth_enabled:{}\r\n\
                     uptime_in_seconds:{}\r\n\
                     uptime_in_days:{}\r\n",
                    REDIS_COMPAT_VERSION,
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS,
                    usize::BITS,
                    std::process::id(),
                    facts.run_id,
                    facts.tcp_port,
                    facts.ws_port,
                    u8::from(facts.tls_enabled),
                    u8::from(facts.auth_enabled),
                    uptime,
                    uptime / 86_400,
                )
            }
            "clients" => {
                // A negative count would mean the guard accounting is broken;
                // clamp rather than emit a value no client can parse.
                let active = STAT_CONNECTIONS_ACTIVE.load(Ordering::Relaxed).max(0);
                format!(
                    "connected_clients:{}\r\n\
                     maxclients:{}\r\n\
                     blocked_clients:0\r\n",
                    active, facts.max_connections,
                )
            }
            "memory" => {
                let used = sample.memory_bytes as u64;
                let max = store.max_memory_bytes().unwrap_or(0) as u64;
                format!(
                    "used_memory:{}\r\n\
                     used_memory_human:{}\r\n\
                     maxmemory:{}\r\n\
                     maxmemory_human:{}\r\n\
                     maxmemory_policy:{}\r\n\
                     recached_max_keys:{}\r\n",
                    used,
                    human_bytes(used),
                    max,
                    human_bytes(max),
                    eviction_policy_name(store.eviction_policy()),
                    store.max_keys().unwrap_or(0),
                )
            }
            "persistence" => {
                // `loading` is what a client's ready-check reads to decide the
                // server can serve traffic. Recached loads its snapshot before
                // it binds a listener, so a client that can reach us is never
                // looking at a loading server: the answer is always 0.
                format!(
                    "loading:0\r\n\
                     rdb_changes_since_last_save:{}\r\n\
                     rdb_last_save_time:{}\r\n\
                     rdb_bgsave_in_progress:0\r\n\
                     aof_enabled:{}\r\n",
                    store.dirty_count(),
                    last_save,
                    u8::from(facts.aof_enabled),
                )
            }
            "stats" => {
                format!(
                    "total_connections_received:{}\r\n\
                     total_commands_processed:{}\r\n\
                     keyspace_hits:{}\r\n\
                     keyspace_misses:{}\r\n\
                     evicted_keys:{}\r\n",
                    STAT_CONNECTIONS_TOTAL.load(Ordering::Relaxed),
                    STAT_COMMANDS_TOTAL.load(Ordering::Relaxed),
                    STAT_KEYSPACE_HITS.load(Ordering::Relaxed),
                    STAT_KEYSPACE_MISSES.load(Ordering::Relaxed),
                    store.evicted_count(),
                )
            }
            "replication" => {
                // Redis still spells these `slave`; tooling greps for exactly
                // that, so the compatible spelling is authoritative and the
                // `replica` names are emitted alongside it.
                format!(
                    "role:{}\r\n\
                     connected_slaves:{}\r\n\
                     connected_replicas:{}\r\n\
                     recached_replication_queue_depth:{}\r\n\
                     recached_replication_lag_frames:{}\r\n",
                    if is_replica { "slave" } else { "master" },
                    repl.connected,
                    repl.connected,
                    repl.queue_depth,
                    repl.lag_frames,
                )
            }
            "keyspace" => {
                // Redis omits the db line entirely when the database is empty.
                if sample.keys == 0 {
                    String::new()
                } else {
                    format!(
                        "db0:keys={},expires={},avg_ttl=0\r\n",
                        sample.keys, sample.volatile_keys,
                    )
                }
            }
            // How a cluster-aware client actually learns it is talking to a
            // single node. `CLUSTER INFO` is not that channel: a `redis-server`
            // built for standalone answers it with an error, not with
            // `cluster_enabled:0`, so this line is the only place the answer
            // exists. Reporting it costs one line and stops a client from
            // guessing.
            "cluster" => "cluster_enabled:0\r\n".to_string(),
            // Recached-specific: the live-query machinery has no Redis analogue,
            // so it gets its own section rather than being smuggled into one.
            "recached" => {
                format!(
                    "live_queries:{}\r\n\
                     watched_keys:{}\r\n",
                    live_queries, watched_keys,
                )
            }
            _ => continue,
        };

        let title = {
            let mut c = section.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => continue,
            }
        };
        out.push_str(&format!("# {}\r\n{}\r\n", title, body));
    }

    out
}
