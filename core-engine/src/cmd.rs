use crate::resp::Value;
use crate::store::MAX_PATTERN_BYTES;

/// Reject a glob pattern longer than `MAX_PATTERN_BYTES`.
///
/// Matching costs O(pattern × text) and runs once per key for `KEYS`/`SCAN
/// MATCH`, once per key per write for sync scopes, and once per published
/// message per subscriber for `PSUBSCRIBE`. Pattern length was previously
/// bounded only by `proto-max-bulk-len` (64 MB), so one command could occupy the
/// process for an unbounded time. Rejected at parse time so no execution path
/// has to think about it.
fn check_pattern(p: &str) -> Result<(), String> {
    if p.len() > MAX_PATTERN_BYTES {
        return Err(format!(
            "ERR pattern is too long ({} > {} bytes)",
            p.len(),
            MAX_PATTERN_BYTES
        ));
    }
    Ok(())
}

/// `check_pattern` for a list of patterns.
fn check_patterns(ps: &[String]) -> Result<(), String> {
    for p in ps {
        check_pattern(p)?;
    }
    Ok(())
}

// ── SET options ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum SetExpiry {
    Ex(u64),
    Px(u64),
    Exat(u64),
    Pxat(u64),
    KeepTtl,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetCondition {
    Nx,
    Xx,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SetOptions {
    pub expiry: Option<SetExpiry>,
    pub condition: Option<SetCondition>,
    pub get: bool,
}

// ── ZADD options ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ZAddCondition {
    Nx,
    Xx,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ZAddOptions {
    pub condition: Option<ZAddCondition>,
    pub gt: bool,
    pub lt: bool,
    pub ch: bool,
    pub incr: bool,
}

// ── Collection scan options ───────────────────────────────────────────────────

/// Arguments shared by `HSCAN` / `SSCAN` / `ZSCAN`.
///
/// The cursor is an offset into the collection's name-ordered element list —
/// the same scheme keyspace `SCAN` uses — so a cursor is only meaningful when
/// the same `MATCH` is passed on every call of an iteration, as Redis requires.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScanArgs {
    pub cursor: u64,
    pub pattern: Option<String>,
    pub count: Option<usize>,
    /// `HSCAN` only: reply with field names and no values (Redis 7.4).
    pub novalues: bool,
}

// ── Command enum ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Ping(Option<String>),
    Auth(String),
    /// `HELLO [protover]` — protocol negotiation. Connection-level like AUTH:
    /// the store never sees it, because the answer depends on the connection.
    Hello(Option<String>),
    /// `INFO [section ...]` — server introspection. Connection-level like
    /// HELLO: most of the answer (uptime, clients, replication) lives in the
    /// server, not the store, so the store refuses it. An empty section list
    /// means the default set.
    Info(Vec<String>),
    /// `QUIT` — reply `+OK`, then close. Connection-level: only the connection
    /// task can honour it, so the store never sees it.
    Quit,
    /// `CLIENT <subcommand> [arg ...]` — connection introspection. Held as the
    /// raw argument list because every answer depends on the connection or on
    /// the client registry, neither of which the store knows about.
    Client(Vec<String>),
    /// `CONFIG <subcommand> [arg ...]` — server configuration.
    Config(Vec<String>),
    /// `COMMAND [subcommand] [arg ...]` — the command catalog.
    CommandQuery(Vec<String>),
    /// `CLUSTER <subcommand> [arg ...]` — cluster topology. Recached is always
    /// standalone, so the answer is fixed; it still has to be sayable, because
    /// a client that cannot ask assumes the worst rather than assuming none.
    Cluster(Vec<String>),
    /// `MODULE <subcommand> [arg ...]` — loaded modules. Recached has no module
    /// API at all, which makes the list empty rather than unanswerable.
    Module(Vec<String>),
    /// `PUBSUB <subcommand> [arg ...]` — pub/sub introspection. Connection-level
    /// for the same reason as CLIENT: the subscriber hub lives in the server,
    /// and the store has never heard of a channel.
    PubSub(Vec<String>),
    /// `MEMORY <subcommand> [arg ...]` for every subcommand except `USAGE`,
    /// which is [`Command::MemoryUsage`]. Split because the two are not the
    /// same kind of command: `USAGE` reads one key and is scoped like any other
    /// key read, while the rest describe the allocator and are not.
    Memory(Vec<String>),
    // ── Strings ──────────────────────────────────────────────────────────────
    Set(String, Vec<u8>, SetOptions),
    Get(String),
    /// ESET — a SET whose key is owned by the connection that wrote it.
    /// The engine stores it like any string; the *server* deletes it when that
    /// connection closes. Presence, cursors, "who is online".
    ESet(String, Vec<u8>),
    /// EADD — a SADD whose members are bound to the connection that added
    /// them. The server removes a member when the last connection holding it
    /// closes, and deletes the set once it empties. "Who is in this room."
    EAdd(String, Vec<String>),
    Del(Vec<String>),
    Unlink(Vec<String>),
    Append(String, Vec<u8>),
    Strlen(String),
    /// `GETRANGE key start end` — inclusive byte range, negative offsets
    /// counting back from the end. The bounded counterpart of `GET`: reading a
    /// window of a large value must not cost the whole value.
    GetRange(String, i64, i64),
    GetSet(String, Vec<u8>),
    MGet(Vec<String>),
    MSet(Vec<(String, Vec<u8>)>),
    SetNx(String, Vec<u8>),
    SetEx(String, u64, Vec<u8>),
    PSetEx(String, u64, Vec<u8>),
    Incr(String),
    Decr(String),
    IncrBy(String, i64),
    DecrBy(String, i64),
    // ── Expiry ────────────────────────────────────────────────────────────────
    Expire(String, u64),
    PExpire(String, u64),
    ExpireAt(String, u64),
    PExpireAt(String, u64),
    Ttl(String),
    PTtl(String),
    Persist(String),
    // ── Keys ─────────────────────────────────────────────────────────────────
    Exists(Vec<String>),
    Keys(String),
    Scan(u64, Option<String>, Option<usize>),
    DbSize,
    FlushDb,
    Rename(String, String),
    Type(String),
    /// `MEMORY USAGE key [SAMPLES n]` — approximate bytes held by one key.
    /// A key read, and scoped like one: it reports on a key's contents, so a
    /// connection that may not read the key may not measure it either.
    MemoryUsage(String),
    // ── Hash ─────────────────────────────────────────────────────────────────
    HSet(String, Vec<(String, Vec<u8>)>),
    HGet(String, String),
    HGetAll(String),
    HDel(String, Vec<String>),
    HKeys(String),
    HVals(String),
    HLen(String),
    HIncrBy(String, String, i64),
    HIncrByFloat(String, String, f64),
    HExists(String, String),
    HSetNx(String, String, Vec<u8>),
    HMGet(String, Vec<String>),
    /// `HSCAN key cursor [MATCH pat] [COUNT n] [NOVALUES]` — the bounded
    /// counterpart of `HGETALL`, which has to clone an entire hash.
    HScan(String, ScanArgs),
    // ── List ─────────────────────────────────────────────────────────────────
    LPush(String, Vec<Vec<u8>>),
    RPush(String, Vec<Vec<u8>>),
    LPushX(String, Vec<Vec<u8>>),
    RPushX(String, Vec<Vec<u8>>),
    LPop(String, Option<u64>),
    RPop(String, Option<u64>),
    LRange(String, i64, i64),
    LLen(String),
    LIndex(String, i64),
    LSet(String, i64, Vec<u8>),
    LRem(String, i64, Vec<u8>),
    LTrim(String, i64, i64),
    // ── Set ──────────────────────────────────────────────────────────────────
    SAdd(String, Vec<String>),
    SMembers(String),
    SRem(String, Vec<String>),
    SCard(String),
    SIsMember(String, String),
    SMIsMember(String, Vec<String>),
    SInter(Vec<String>),
    SInterStore(String, Vec<String>),
    SUnion(Vec<String>),
    SUnionStore(String, Vec<String>),
    SDiff(Vec<String>),
    SDiffStore(String, Vec<String>),
    SPop(String, Option<u64>),
    SRandMember(String, Option<i64>),
    SMove(String, String, String),
    /// `SSCAN key cursor [MATCH pat] [COUNT n]` — the bounded counterpart of
    /// `SMEMBERS`.
    SScan(String, ScanArgs),
    // ── Sorted Set ───────────────────────────────────────────────────────────
    ZAdd(String, ZAddOptions, Vec<(f64, String)>),
    ZRange(String, i64, i64, bool),
    ZRevRange(String, i64, i64, bool),
    ZRangeByScore(String, String, String, bool, Option<(i64, i64)>),
    ZRevRangeByScore(String, String, String, bool, Option<(i64, i64)>),
    ZScore(String, String),
    ZMScore(String, Vec<String>),
    ZRank(String, String),
    ZRevRank(String, String),
    ZRem(String, Vec<String>),
    ZCard(String),
    ZIncrBy(String, f64, String),
    ZCount(String, String, String),
    /// `ZSCAN key cursor [MATCH pat] [COUNT n]` — pages `member, score` pairs.
    /// Ordered by member, not by score: the cursor is an offset, and only the
    /// member ordering survives a score being updated mid-iteration.
    ZScan(String, ScanArgs),
    // ── JSON ──────────────────────────────────────────────────────────────────
    /// JSET key path value — set JSON at a path (`$` = whole document).
    JSet(String, String, String),
    /// `JGET key [path]` — read JSON at a path, serialized. Defaults to `$`.
    JGet(String, Option<String>),
    /// JMERGE key patch — RFC 7386 JSON Merge Patch against the whole document.
    JMerge(String, String),
    // ── Rate limiting ─────────────────────────────────────────────────────────
    /// RLSET key limit window_secs — configure a sliding-window rate limiter.
    RlSet(String, u64, u64),
    /// RLCHECK key [limit window_secs] — record an attempt and report
    /// [allowed, remaining, retry_after_ms]. The optional limit/window pair
    /// configures the limiter on first use (upsert), for per-IP/per-user keys
    /// where a separate RLSET round-trip per key is impractical.
    RlCheck(String, Option<(u64, u64)>),
    // ── Transactions ─────────────────────────────────────────────────────────
    Multi,
    Exec,
    Discard,
    // ── Pub/Sub ───────────────────────────────────────────────────────────────
    Subscribe(Vec<String>),
    Unsubscribe(Vec<String>),
    PSubscribe(Vec<String>),
    PUnsubscribe(Vec<String>),
    Publish(String, Vec<u8>),
    // ── Observable keys ───────────────────────────────────────────────────────
    Watch(Vec<String>),
    Unwatch(Vec<String>),
    // ── Sync scoping (WebSocket only) ──────────────────────────────────────────
    /// SYNC [TOKEN token | pattern ...] — set this connection's sync scopes.
    /// Interpretation of the arguments (token verification, pattern grants)
    /// happens in the server layer; the store never sees this command.
    Sync(Vec<String>),
    // ── Duplicate-suppressed replay (WebSocket only) ───────────────────────────
    /// DEDUP client_id id command args... — wraps a write with a per-client
    /// monotonic id so an offline-replayed duplicate is skipped. Unwrapped in
    /// the server layer; the store never sees this command.
    Dedup(String, u64, Box<Command>),
    // ── Live queries (WebSocket only) ──────────────────────────────────────────
    /// QSUB pattern — subscribe to a live query: the reply carries the current
    /// state of every key matching the glob pattern, and subsequent mutations
    /// to matching keys arrive as `keychange` pushes. Server-layer only.
    QSub(String),
    /// `QUNSUB [pattern]` — drop one live query, or all of them without an
    /// argument. Server-layer only.
    QUnsub(Option<String>),
    // ── Persistence ───────────────────────────────────────────────────────────
    Save,
    BgSave,
    LastSave,
    // ── Replication ───────────────────────────────────────────────────────────
    ReplicaOfNoOne,
    Unknown(String),
}

impl Command {
    pub fn from_value(value: Value) -> Result<Command, String> {
        match value {
            Value::Array(Some(mut arr)) => {
                if arr.is_empty() {
                    return Err("Empty command".to_string());
                }
                let cmd_name = match &arr[0] {
                    Value::BulkString(Some(data)) => String::from_utf8_lossy(data).to_uppercase(),
                    Value::SimpleString(s) => s.to_uppercase(),
                    _ => return Err("Invalid command name type".to_string()),
                };

                // Payload arguments may be arbitrary bytes; identifier
                // arguments may not.
                //
                // Keys, hash fields, set and sorted-set members, glob patterns
                // and channel names are looked up, matched and routed as text,
                // so a non-UTF-8 one cannot be handled faithfully. Rather than
                // lossily converting it — which returns OK and stores something
                // different from what was sent — the command is refused here,
                // before anything is written. Values are exempt: they are
                // stored verbatim. See `is_payload_index`.
                if let Some(pos) = (1..arr.len()).find(|&i| {
                    !is_payload_index(&cmd_name, i)
                        && matches!(&arr[i], Value::BulkString(Some(b)) if std::str::from_utf8(b).is_err())
                }) {
                    return Err(format!(
                        "ERR argument {pos} is not valid UTF-8. Keys, fields, members and \
                         patterns must be text; only values may be binary"
                    ));
                }

                macro_rules! need {
                    ($n:expr) => {
                        if arr.len() < $n {
                            return Err(format!(
                                "ERR wrong number of arguments for '{}' command",
                                cmd_name.to_lowercase()
                            ));
                        }
                    };
                }

                match cmd_name.as_str() {
                    // ── Core ─────────────────────────────────────────────────
                    "PING" => {
                        let msg = if arr.len() > 1 {
                            extract_string(&arr[1])
                        } else {
                            None
                        };
                        Ok(Command::Ping(msg))
                    }
                    "AUTH" => {
                        need!(2);
                        Ok(Command::Auth(extract_string(&arr[1]).unwrap_or_default()))
                    }
                    // Bare HELLO reports the current version without changing it.
                    // Trailing AUTH/SETNAME arguments are not supported and are
                    // rejected by the connection layer rather than ignored.
                    "HELLO" => Ok(Command::Hello(if arr.len() > 1 {
                        Some(extract_string(&arr[1]).unwrap_or_default())
                    } else {
                        None
                    })),
                    // Section names are lowercased here so the connection layer
                    // can compare them directly — Redis matches them
                    // case-insensitively, and `INFO Server` is common in the
                    // wild.
                    "INFO" => Ok(Command::Info(
                        arr[1..]
                            .iter()
                            .map(|v| extract_string(v).unwrap_or_default().to_lowercase())
                            .collect(),
                    )),
                    "QUIT" => Ok(Command::Quit),
                    // The subcommand and its arguments are carried through
                    // verbatim. Parsing them here would split the knowledge of
                    // what each subcommand means across two files, and every
                    // answer is the connection layer's to give anyway.
                    "CLIENT" => {
                        need!(2);
                        Ok(Command::Client(collect_strings(&mut arr[1..])))
                    }
                    "CONFIG" => {
                        need!(2);
                        Ok(Command::Config(collect_strings(&mut arr[1..])))
                    }
                    "COMMAND" => Ok(Command::CommandQuery(collect_strings(&mut arr[1..]))),
                    "CLUSTER" => {
                        need!(2);
                        Ok(Command::Cluster(collect_strings(&mut arr[1..])))
                    }
                    "MODULE" => {
                        need!(2);
                        Ok(Command::Module(collect_strings(&mut arr[1..])))
                    }
                    "PUBSUB" => {
                        need!(2);
                        Ok(Command::PubSub(collect_strings(&mut arr[1..])))
                    }
                    "MEMORY" => {
                        need!(2);
                        let sub = extract_string(&arr[1]).unwrap_or_default().to_uppercase();
                        if sub != "USAGE" {
                            return Ok(Command::Memory(collect_strings(&mut arr[1..])));
                        }
                        need!(3);
                        let key = take_key(&mut arr[2])?;
                        // Redis accepts `SAMPLES n` to bound how much of a
                        // nested value it walks. Recached walks all of it —
                        // `entry_size` is the same measurement the eviction
                        // loop runs, and there is no cheaper approximation to
                        // fall back to — so the option is accepted and the
                        // count ignored. The reply is never less accurate than
                        // what was asked for, only more.
                        match arr.len() {
                            3 => {}
                            5 if extract_string(&arr[3])
                                .unwrap_or_default()
                                .eq_ignore_ascii_case("SAMPLES") =>
                            {
                                // Redis answers a negative SAMPLES with a plain
                                // syntax error rather than a range error;
                                // verified against 7.2.5 rather than assumed.
                                let n = extract_int(&arr[4])?;
                                if n < 0 {
                                    return Err("ERR syntax error".to_string());
                                }
                            }
                            _ => return Err("ERR syntax error".to_string()),
                        }
                        Ok(Command::MemoryUsage(key))
                    }

                    // ── Strings ───────────────────────────────────────────────
                    "SET" => {
                        need!(3);
                        let key = take_key(&mut arr[1])?;
                        let val = take_bytes(&mut arr[2]).unwrap_or_default();
                        let mut opts = SetOptions::default();
                        let mut i = 3usize;
                        while i < arr.len() {
                            let flag = extract_string(&arr[i]).unwrap_or_default().to_uppercase();
                            match flag.as_str() {
                                "EX" => {
                                    i += 1;
                                    if i >= arr.len() {
                                        return Err("ERR syntax error".to_string());
                                    }
                                    let n = extract_int(&arr[i])?;
                                    if n <= 0 {
                                        return Err(
                                            "ERR invalid expire time in 'set' command".to_string()
                                        );
                                    }
                                    opts.expiry = Some(SetExpiry::Ex(n as u64));
                                }
                                "PX" => {
                                    i += 1;
                                    if i >= arr.len() {
                                        return Err("ERR syntax error".to_string());
                                    }
                                    let n = extract_int(&arr[i])?;
                                    if n <= 0 {
                                        return Err(
                                            "ERR invalid expire time in 'set' command".to_string()
                                        );
                                    }
                                    opts.expiry = Some(SetExpiry::Px(n as u64));
                                }
                                "EXAT" => {
                                    i += 1;
                                    if i >= arr.len() {
                                        return Err("ERR syntax error".to_string());
                                    }
                                    let n = extract_int(&arr[i])?;
                                    if n <= 0 {
                                        return Err(
                                            "ERR invalid expire time in 'set' command".to_string()
                                        );
                                    }
                                    opts.expiry = Some(SetExpiry::Exat(n as u64));
                                }
                                "PXAT" => {
                                    i += 1;
                                    if i >= arr.len() {
                                        return Err("ERR syntax error".to_string());
                                    }
                                    let n = extract_int(&arr[i])?;
                                    if n <= 0 {
                                        return Err(
                                            "ERR invalid expire time in 'set' command".to_string()
                                        );
                                    }
                                    opts.expiry = Some(SetExpiry::Pxat(n as u64));
                                }
                                "KEEPTTL" => {
                                    opts.expiry = Some(SetExpiry::KeepTtl);
                                }
                                "NX" => opts.condition = Some(SetCondition::Nx),
                                "XX" => opts.condition = Some(SetCondition::Xx),
                                "GET" => opts.get = true,
                                _ => return Err("ERR syntax error".to_string()),
                            }
                            i += 1;
                        }
                        Ok(Command::Set(key, val, opts))
                    }
                    "GET" => {
                        need!(2);
                        Ok(Command::Get(extract_key(&arr[1])?))
                    }
                    "ESET" => {
                        need!(3);
                        Ok(Command::ESet(
                            extract_key(&arr[1])?,
                            extract_bytes(&arr[2]).unwrap_or_default(),
                        ))
                    }
                    "EADD" => {
                        need!(3);
                        Ok(Command::EAdd(
                            extract_key(&arr[1])?,
                            collect_strings(&mut arr[2..]),
                        ))
                    }
                    "DEL" => {
                        need!(2);
                        Ok(Command::Del(extract_keys(&arr[1..])?))
                    }
                    "UNLINK" => {
                        need!(2);
                        Ok(Command::Unlink(extract_keys(&arr[1..])?))
                    }
                    "APPEND" => {
                        need!(3);
                        let key = take_key(&mut arr[1])?;
                        let val = take_bytes(&mut arr[2]).unwrap_or_default();
                        Ok(Command::Append(key, val))
                    }
                    "STRLEN" => {
                        need!(2);
                        Ok(Command::Strlen(extract_key(&arr[1])?))
                    }
                    "GETRANGE" => {
                        need!(4);
                        Ok(Command::GetRange(
                            extract_key(&arr[1])?,
                            extract_int(&arr[2])?,
                            extract_int(&arr[3])?,
                        ))
                    }
                    "GETSET" => {
                        need!(3);
                        let key = take_key(&mut arr[1])?;
                        let val = take_bytes(&mut arr[2]).unwrap_or_default();
                        Ok(Command::GetSet(key, val))
                    }
                    "MGET" => {
                        need!(2);
                        Ok(Command::MGet(extract_keys(&arr[1..])?))
                    }
                    "MSET" => {
                        if arr.len() < 3 || (arr.len() - 1) % 2 != 0 {
                            return Err(
                                "ERR wrong number of arguments for 'mset' command".to_string()
                            );
                        }
                        let pairs = arr[1..]
                            .chunks_mut(2)
                            .map(|c| {
                                let (k, v) = c.split_at_mut(1);
                                Ok((
                                    take_key(&mut k[0])?,
                                    take_bytes(&mut v[0]).unwrap_or_default(),
                                ))
                            })
                            .collect::<Result<Vec<_>, String>>()?;
                        Ok(Command::MSet(pairs))
                    }
                    "SETNX" => {
                        need!(3);
                        let key = take_key(&mut arr[1])?;
                        let val = take_bytes(&mut arr[2]).unwrap_or_default();
                        Ok(Command::SetNx(key, val))
                    }
                    "SETEX" => {
                        need!(4);
                        let secs = extract_int(&arr[2])?;
                        if secs <= 0 {
                            return Err("ERR invalid expire time in 'setex' command".to_string());
                        }
                        Ok(Command::SetEx(
                            extract_key(&arr[1])?,
                            secs as u64,
                            extract_bytes(&arr[3]).unwrap_or_default(),
                        ))
                    }
                    "PSETEX" => {
                        need!(4);
                        let ms = extract_int(&arr[2])?;
                        if ms <= 0 {
                            return Err("ERR invalid expire time in 'psetex' command".to_string());
                        }
                        Ok(Command::PSetEx(
                            extract_key(&arr[1])?,
                            ms as u64,
                            extract_bytes(&arr[3]).unwrap_or_default(),
                        ))
                    }
                    "INCR" => {
                        need!(2);
                        Ok(Command::Incr(extract_key(&arr[1])?))
                    }
                    "DECR" => {
                        need!(2);
                        Ok(Command::Decr(extract_key(&arr[1])?))
                    }
                    "INCRBY" => {
                        need!(3);
                        Ok(Command::IncrBy(
                            extract_key(&arr[1])?,
                            extract_int(&arr[2])?,
                        ))
                    }
                    "DECRBY" => {
                        need!(3);
                        Ok(Command::DecrBy(
                            extract_key(&arr[1])?,
                            extract_int(&arr[2])?,
                        ))
                    }

                    // ── Expiry ─────────────────────────────────────────────────
                    "EXPIRE" => {
                        need!(3);
                        let secs = extract_int(&arr[2])?;
                        if secs < 0 {
                            return Err("ERR invalid expire time in 'expire' command".to_string());
                        }
                        Ok(Command::Expire(
                            extract_string(&arr[1]).unwrap_or_default(),
                            secs as u64,
                        ))
                    }
                    "PEXPIRE" => {
                        need!(3);
                        let ms = extract_int(&arr[2])?;
                        if ms < 0 {
                            return Err("ERR invalid expire time in 'pexpire' command".to_string());
                        }
                        Ok(Command::PExpire(
                            extract_string(&arr[1]).unwrap_or_default(),
                            ms as u64,
                        ))
                    }
                    "EXPIREAT" => {
                        need!(3);
                        let ts = extract_int(&arr[2])?;
                        if ts < 0 {
                            return Err("ERR invalid expire time in 'expireat' command".to_string());
                        }
                        Ok(Command::ExpireAt(
                            extract_string(&arr[1]).unwrap_or_default(),
                            ts as u64,
                        ))
                    }
                    "PEXPIREAT" => {
                        need!(3);
                        let ts = extract_int(&arr[2])?;
                        if ts < 0 {
                            return Err(
                                "ERR invalid expire time in 'pexpireat' command".to_string()
                            );
                        }
                        Ok(Command::PExpireAt(
                            extract_string(&arr[1]).unwrap_or_default(),
                            ts as u64,
                        ))
                    }
                    "TTL" => {
                        need!(2);
                        Ok(Command::Ttl(extract_string(&arr[1]).unwrap_or_default()))
                    }
                    "PTTL" => {
                        need!(2);
                        Ok(Command::PTtl(extract_string(&arr[1]).unwrap_or_default()))
                    }
                    "PERSIST" => {
                        need!(2);
                        Ok(Command::Persist(
                            extract_string(&arr[1]).unwrap_or_default(),
                        ))
                    }

                    // ── Keys ───────────────────────────────────────────────────
                    "EXISTS" => {
                        need!(2);
                        Ok(Command::Exists(extract_keys(&arr[1..])?))
                    }
                    "KEYS" => {
                        need!(2);
                        let pattern = extract_string(&arr[1]).unwrap_or_default();
                        check_pattern(&pattern)?;
                        Ok(Command::Keys(pattern))
                    }
                    "SCAN" => {
                        need!(2);
                        let args = parse_scan_args(&arr[1], &arr[2..], false)?;
                        Ok(Command::Scan(args.cursor, args.pattern, args.count))
                    }
                    "DBSIZE" => Ok(Command::DbSize),
                    "FLUSHDB" => Ok(Command::FlushDb),
                    "RENAME" => {
                        need!(3);
                        Ok(Command::Rename(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_string(&arr[2]).unwrap_or_default(),
                        ))
                    }
                    "TYPE" => {
                        need!(2);
                        Ok(Command::Type(extract_string(&arr[1]).unwrap_or_default()))
                    }

                    // ── Sync scoping ───────────────────────────────────────────
                    "SYNC" => {
                        let patterns: Vec<String> = arr[1..]
                            .iter()
                            .map(|v| extract_string(v).unwrap_or_default())
                            .collect();
                        // A signed token is checked separately in the server
                        // layer; this covers the inline-pattern form.
                        check_patterns(&patterns)?;
                        Ok(Command::Sync(patterns))
                    }

                    // ── Duplicate-suppressed replay ────────────────────────────
                    "DEDUP" => {
                        need!(4);
                        let client_id = extract_string(&arr[1]).unwrap_or_default();
                        if client_id.is_empty() || client_id.len() > 64 {
                            return Err("ERR DEDUP client id must be 1-64 characters".to_string());
                        }
                        let id = extract_int(&arr[2])?;
                        if id < 0 {
                            return Err("ERR DEDUP id must be non-negative".to_string());
                        }
                        let inner = Command::from_value(Value::Array(Some(arr[3..].to_vec())))?;
                        if matches!(inner, Command::Dedup(_, _, _)) {
                            return Err("ERR DEDUP cannot be nested".to_string());
                        }
                        Ok(Command::Dedup(client_id, id as u64, Box::new(inner)))
                    }

                    // ── Live queries ───────────────────────────────────────────
                    "QSUB" => {
                        need!(2);
                        let pattern = extract_string(&arr[1]).unwrap_or_default();
                        if pattern.is_empty() {
                            return Err("ERR QSUB requires a non-empty pattern".to_string());
                        }
                        check_pattern(&pattern)?;
                        Ok(Command::QSub(pattern))
                    }
                    "QUNSUB" => {
                        let pattern = if arr.len() > 1 {
                            Some(extract_string(&arr[1]).unwrap_or_default())
                        } else {
                            None
                        };
                        if let Some(p) = &pattern {
                            check_pattern(p)?;
                        }
                        Ok(Command::QUnsub(pattern))
                    }

                    // ── JSON ───────────────────────────────────────────────────
                    "JSET" => {
                        need!(4);
                        let key = take_key(&mut arr[1])?;
                        let path = take_string(&mut arr[2]).unwrap_or_default();
                        let doc = take_string(&mut arr[3]).unwrap_or_default();
                        Ok(Command::JSet(key, path, doc))
                    }
                    "JGET" => {
                        need!(2);
                        let path = if arr.len() > 2 {
                            Some(extract_string(&arr[2]).unwrap_or_default())
                        } else {
                            None
                        };
                        Ok(Command::JGet(extract_key(&arr[1])?, path))
                    }
                    "JMERGE" => {
                        need!(3);
                        let key = take_key(&mut arr[1])?;
                        let patch = take_string(&mut arr[2]).unwrap_or_default();
                        Ok(Command::JMerge(key, patch))
                    }

                    // ── Rate limiting ──────────────────────────────────────────
                    "RLSET" => {
                        need!(4);
                        let key = extract_key(&arr[1])?;
                        let (limit, window) = parse_rl_config(&arr[2], &arr[3], "rlset")?;
                        Ok(Command::RlSet(key, limit, window))
                    }
                    "RLCHECK" => {
                        need!(2);
                        let key = extract_key(&arr[1])?;
                        let config = match arr.len() {
                            2 => None,
                            4 => Some(parse_rl_config(&arr[2], &arr[3], "rlcheck")?),
                            _ => {
                                return Err("ERR wrong number of arguments for 'rlcheck' command"
                                    .to_string());
                            }
                        };
                        Ok(Command::RlCheck(key, config))
                    }

                    // ── Hash ───────────────────────────────────────────────────
                    "HSET" | "HMSET" => {
                        if arr.len() < 4 || (arr.len() - 2) % 2 != 0 {
                            return Err(format!(
                                "ERR wrong number of arguments for '{}' command",
                                cmd_name.to_lowercase()
                            ));
                        }
                        let key = extract_key(&arr[1])?;
                        let pairs = arr[2..]
                            .chunks_mut(2)
                            .map(|c| {
                                let (f, v) = c.split_at_mut(1);
                                (
                                    take_string(&mut f[0]).unwrap_or_default(),
                                    take_bytes(&mut v[0]).unwrap_or_default(),
                                )
                            })
                            .collect();
                        Ok(Command::HSet(key, pairs))
                    }
                    "HGET" => {
                        need!(3);
                        Ok(Command::HGet(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_string(&arr[2]).unwrap_or_default(),
                        ))
                    }
                    "HGETALL" => {
                        need!(2);
                        Ok(Command::HGetAll(
                            extract_string(&arr[1]).unwrap_or_default(),
                        ))
                    }
                    "HDEL" => {
                        need!(3);
                        let key = extract_key(&arr[1])?;
                        let fields = arr[2..].iter_mut().filter_map(take_string).collect();
                        Ok(Command::HDel(key, fields))
                    }
                    "HKEYS" => {
                        need!(2);
                        Ok(Command::HKeys(extract_string(&arr[1]).unwrap_or_default()))
                    }
                    "HVALS" => {
                        need!(2);
                        Ok(Command::HVals(extract_string(&arr[1]).unwrap_or_default()))
                    }
                    "HLEN" => {
                        need!(2);
                        Ok(Command::HLen(extract_string(&arr[1]).unwrap_or_default()))
                    }
                    "HINCRBY" => {
                        need!(4);
                        Ok(Command::HIncrBy(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_string(&arr[2]).unwrap_or_default(),
                            extract_int(&arr[3])?,
                        ))
                    }
                    "HINCRBYFLOAT" => {
                        need!(4);
                        let inc = extract_float(&arr[3])?;
                        Ok(Command::HIncrByFloat(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_string(&arr[2]).unwrap_or_default(),
                            inc,
                        ))
                    }
                    "HEXISTS" => {
                        need!(3);
                        Ok(Command::HExists(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_string(&arr[2]).unwrap_or_default(),
                        ))
                    }
                    "HSETNX" => {
                        need!(4);
                        Ok(Command::HSetNx(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_string(&arr[2]).unwrap_or_default(),
                            extract_bytes(&arr[3]).unwrap_or_default(),
                        ))
                    }
                    "HMGET" => {
                        need!(3);
                        let key = extract_key(&arr[1])?;
                        let fields = arr[2..].iter_mut().filter_map(take_string).collect();
                        Ok(Command::HMGet(key, fields))
                    }
                    "HSCAN" => {
                        need!(3);
                        let key = extract_key(&arr[1])?;
                        Ok(Command::HScan(
                            key,
                            parse_scan_args(&arr[2], &arr[3..], true)?,
                        ))
                    }

                    // ── List ───────────────────────────────────────────────────
                    "LPUSH" => {
                        need!(3);
                        let key = extract_key(&arr[1])?;
                        let vals = arr[2..].iter_mut().filter_map(take_bytes).collect();
                        Ok(Command::LPush(key, vals))
                    }
                    "RPUSH" => {
                        need!(3);
                        let key = extract_key(&arr[1])?;
                        let vals = arr[2..].iter_mut().filter_map(take_bytes).collect();
                        Ok(Command::RPush(key, vals))
                    }
                    "LPUSHX" => {
                        need!(3);
                        let key = extract_key(&arr[1])?;
                        let vals = arr[2..].iter_mut().filter_map(take_bytes).collect();
                        Ok(Command::LPushX(key, vals))
                    }
                    "RPUSHX" => {
                        need!(3);
                        let key = extract_key(&arr[1])?;
                        let vals = arr[2..].iter_mut().filter_map(take_bytes).collect();
                        Ok(Command::RPushX(key, vals))
                    }
                    "LPOP" => {
                        need!(2);
                        let key = extract_key(&arr[1])?;
                        let count = if arr.len() > 2 {
                            Some(extract_count(&arr[2])?)
                        } else {
                            None
                        };
                        Ok(Command::LPop(key, count))
                    }
                    "RPOP" => {
                        need!(2);
                        let key = extract_key(&arr[1])?;
                        let count = if arr.len() > 2 {
                            Some(extract_count(&arr[2])?)
                        } else {
                            None
                        };
                        Ok(Command::RPop(key, count))
                    }
                    "LRANGE" => {
                        need!(4);
                        Ok(Command::LRange(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_int(&arr[2])?,
                            extract_int(&arr[3])?,
                        ))
                    }
                    "LLEN" => {
                        need!(2);
                        Ok(Command::LLen(extract_string(&arr[1]).unwrap_or_default()))
                    }
                    "LINDEX" => {
                        need!(3);
                        Ok(Command::LIndex(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_int(&arr[2])?,
                        ))
                    }
                    "LSET" => {
                        need!(4);
                        Ok(Command::LSet(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_int(&arr[2])?,
                            extract_bytes(&arr[3]).unwrap_or_default(),
                        ))
                    }
                    "LREM" => {
                        need!(4);
                        Ok(Command::LRem(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_int(&arr[2])?,
                            extract_bytes(&arr[3]).unwrap_or_default(),
                        ))
                    }
                    "LTRIM" => {
                        need!(4);
                        Ok(Command::LTrim(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_int(&arr[2])?,
                            extract_int(&arr[3])?,
                        ))
                    }

                    // ── Set ────────────────────────────────────────────────────
                    "SADD" => {
                        need!(3);
                        let key = take_key(&mut arr[1])?;
                        let members = arr[2..].iter_mut().filter_map(take_string).collect();
                        Ok(Command::SAdd(key, members))
                    }
                    "SMEMBERS" => {
                        need!(2);
                        Ok(Command::SMembers(
                            extract_string(&arr[1]).unwrap_or_default(),
                        ))
                    }
                    "SREM" => {
                        need!(3);
                        let key = extract_key(&arr[1])?;
                        let members = arr[2..].iter_mut().filter_map(take_string).collect();
                        Ok(Command::SRem(key, members))
                    }
                    "SCARD" => {
                        need!(2);
                        Ok(Command::SCard(extract_string(&arr[1]).unwrap_or_default()))
                    }
                    "SISMEMBER" => {
                        need!(3);
                        Ok(Command::SIsMember(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_string(&arr[2]).unwrap_or_default(),
                        ))
                    }
                    "SMISMEMBER" => {
                        need!(3);
                        let key = extract_key(&arr[1])?;
                        let members = arr[2..].iter_mut().filter_map(take_string).collect();
                        Ok(Command::SMIsMember(key, members))
                    }
                    "SINTER" => {
                        need!(2);
                        Ok(Command::SInter(
                            arr[1..].iter_mut().filter_map(take_string).collect(),
                        ))
                    }
                    "SINTERSTORE" => {
                        need!(3);
                        let dst = extract_string(&arr[1]).unwrap_or_default();
                        let keys = arr[2..].iter_mut().filter_map(take_string).collect();
                        Ok(Command::SInterStore(dst, keys))
                    }
                    "SUNION" => {
                        need!(2);
                        Ok(Command::SUnion(
                            arr[1..].iter_mut().filter_map(take_string).collect(),
                        ))
                    }
                    "SUNIONSTORE" => {
                        need!(3);
                        let dst = extract_string(&arr[1]).unwrap_or_default();
                        let keys = arr[2..].iter_mut().filter_map(take_string).collect();
                        Ok(Command::SUnionStore(dst, keys))
                    }
                    "SDIFF" => {
                        need!(2);
                        Ok(Command::SDiff(
                            arr[1..].iter_mut().filter_map(take_string).collect(),
                        ))
                    }
                    "SDIFFSTORE" => {
                        need!(3);
                        let dst = extract_string(&arr[1]).unwrap_or_default();
                        let keys = arr[2..].iter_mut().filter_map(take_string).collect();
                        Ok(Command::SDiffStore(dst, keys))
                    }
                    "SPOP" => {
                        need!(2);
                        let key = extract_key(&arr[1])?;
                        let count = if arr.len() > 2 {
                            Some(extract_count(&arr[2])?)
                        } else {
                            None
                        };
                        Ok(Command::SPop(key, count))
                    }
                    "SRANDMEMBER" => {
                        need!(2);
                        let key = extract_key(&arr[1])?;
                        let count = if arr.len() > 2 {
                            let n = extract_int(&arr[2])?;
                            // A negative count means "with repetition", so it is
                            // the one count the set's own size does not clamp:
                            // |n| members are generated no matter how small the
                            // set is. Cap it at what a RESP array can carry —
                            // beyond that the reply is an allocation no client
                            // could parse back anyway.
                            if n < 0 && n.unsigned_abs() > crate::resp::MAX_ARRAY_ELEMENTS as u64 {
                                return Err("ERR count is out of range in 'srandmember' command"
                                    .to_string());
                            }
                            Some(n)
                        } else {
                            None
                        };
                        Ok(Command::SRandMember(key, count))
                    }
                    "SMOVE" => {
                        need!(4);
                        Ok(Command::SMove(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_string(&arr[2]).unwrap_or_default(),
                            extract_string(&arr[3]).unwrap_or_default(),
                        ))
                    }
                    "SSCAN" => {
                        need!(3);
                        let key = extract_key(&arr[1])?;
                        Ok(Command::SScan(
                            key,
                            parse_scan_args(&arr[2], &arr[3..], false)?,
                        ))
                    }

                    // ── Sorted Set ─────────────────────────────────────────────
                    "ZADD" => {
                        need!(4);
                        let key = extract_key(&arr[1])?;
                        let mut opts = ZAddOptions::default();
                        let mut i = 2usize;

                        // Parse leading options (until we hit a parseable float)
                        while i < arr.len() {
                            let tok = extract_string(&arr[i]).unwrap_or_default().to_uppercase();
                            match tok.as_str() {
                                "NX" => {
                                    opts.condition = Some(ZAddCondition::Nx);
                                    i += 1;
                                }
                                "XX" => {
                                    opts.condition = Some(ZAddCondition::Xx);
                                    i += 1;
                                }
                                "GT" => {
                                    opts.gt = true;
                                    i += 1;
                                }
                                "LT" => {
                                    opts.lt = true;
                                    i += 1;
                                }
                                "CH" => {
                                    opts.ch = true;
                                    i += 1;
                                }
                                "INCR" => {
                                    opts.incr = true;
                                    i += 1;
                                }
                                _ => break,
                            }
                        }

                        if (arr.len() - i) < 2 || !(arr.len() - i).is_multiple_of(2) {
                            return Err("ERR syntax error".to_string());
                        }

                        let mut pairs = Vec::new();
                        while i < arr.len() {
                            let score = extract_float(&arr[i])?;
                            let member = extract_string(&arr[i + 1]).unwrap_or_default();
                            pairs.push((score, member));
                            i += 2;
                        }

                        if opts.gt && opts.lt {
                            return Err(
                                "ERR GT and LT options at the same time are not compatible"
                                    .to_string(),
                            );
                        }
                        if (opts.gt || opts.lt) && opts.condition == Some(ZAddCondition::Nx) {
                            return Err(
                                "ERR GT, LT, and NX options at the same time are not compatible"
                                    .to_string(),
                            );
                        }

                        if opts.incr && pairs.len() != 1 {
                            return Err("ERR INCR option supports a single increment-element pair"
                                .to_string());
                        }

                        Ok(Command::ZAdd(key, opts, pairs))
                    }
                    "ZRANGE" => {
                        need!(4);
                        let key = extract_key(&arr[1])?;
                        let start = extract_int(&arr[2])?;
                        let stop = extract_int(&arr[3])?;
                        let withscores = arr
                            .get(4)
                            .and_then(extract_string)
                            .map(|s| s.to_uppercase() == "WITHSCORES")
                            .unwrap_or(false);
                        Ok(Command::ZRange(key, start, stop, withscores))
                    }
                    "ZREVRANGE" => {
                        need!(4);
                        let key = extract_key(&arr[1])?;
                        let start = extract_int(&arr[2])?;
                        let stop = extract_int(&arr[3])?;
                        let withscores = arr
                            .get(4)
                            .and_then(extract_string)
                            .map(|s| s.to_uppercase() == "WITHSCORES")
                            .unwrap_or(false);
                        Ok(Command::ZRevRange(key, start, stop, withscores))
                    }
                    "ZRANGEBYSCORE" => {
                        need!(4);
                        let key = extract_key(&arr[1])?;
                        let min = extract_string(&arr[2]).unwrap_or_default();
                        let max = extract_string(&arr[3]).unwrap_or_default();
                        let (withscores, limit) = parse_zrange_opts(&arr[4..])?;
                        Ok(Command::ZRangeByScore(key, min, max, withscores, limit))
                    }
                    "ZREVRANGEBYSCORE" => {
                        need!(4);
                        let key = extract_key(&arr[1])?;
                        let max = extract_string(&arr[2]).unwrap_or_default();
                        let min = extract_string(&arr[3]).unwrap_or_default();
                        let (withscores, limit) = parse_zrange_opts(&arr[4..])?;
                        Ok(Command::ZRevRangeByScore(key, max, min, withscores, limit))
                    }
                    "ZSCORE" => {
                        need!(3);
                        Ok(Command::ZScore(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_string(&arr[2]).unwrap_or_default(),
                        ))
                    }
                    "ZMSCORE" => {
                        need!(3);
                        let key = extract_key(&arr[1])?;
                        let members = arr[2..].iter_mut().filter_map(take_string).collect();
                        Ok(Command::ZMScore(key, members))
                    }
                    "ZRANK" => {
                        need!(3);
                        Ok(Command::ZRank(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_string(&arr[2]).unwrap_or_default(),
                        ))
                    }
                    "ZREVRANK" => {
                        need!(3);
                        Ok(Command::ZRevRank(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_string(&arr[2]).unwrap_or_default(),
                        ))
                    }
                    "ZREM" => {
                        need!(3);
                        let key = extract_key(&arr[1])?;
                        let members = arr[2..].iter_mut().filter_map(take_string).collect();
                        Ok(Command::ZRem(key, members))
                    }
                    "ZCARD" => {
                        need!(2);
                        Ok(Command::ZCard(extract_string(&arr[1]).unwrap_or_default()))
                    }
                    "ZINCRBY" => {
                        need!(4);
                        let inc = extract_float(&arr[2])?;
                        Ok(Command::ZIncrBy(
                            extract_string(&arr[1]).unwrap_or_default(),
                            inc,
                            extract_string(&arr[3]).unwrap_or_default(),
                        ))
                    }
                    "ZCOUNT" => {
                        need!(4);
                        Ok(Command::ZCount(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_string(&arr[2]).unwrap_or_default(),
                            extract_string(&arr[3]).unwrap_or_default(),
                        ))
                    }
                    "ZSCAN" => {
                        need!(3);
                        let key = extract_key(&arr[1])?;
                        Ok(Command::ZScan(
                            key,
                            parse_scan_args(&arr[2], &arr[3..], false)?,
                        ))
                    }

                    // ── Transactions ─────────────────────────────────────
                    "MULTI" => Ok(Command::Multi),
                    "EXEC" => Ok(Command::Exec),
                    "DISCARD" => Ok(Command::Discard),

                    // ── Pub/Sub ───────────────────────────────────────────
                    "SUBSCRIBE" => {
                        need!(2);
                        Ok(Command::Subscribe(
                            arr[1..].iter_mut().filter_map(take_string).collect(),
                        ))
                    }
                    "UNSUBSCRIBE" => Ok(Command::Unsubscribe(
                        arr[1..].iter_mut().filter_map(take_string).collect(),
                    )),
                    "PSUBSCRIBE" => {
                        need!(2);
                        let patterns: Vec<String> =
                            arr[1..].iter_mut().filter_map(take_string).collect();
                        // Matched against every published channel name for the
                        // life of the subscription, so an unbounded pattern here
                        // taxes every PUBLISH, not just one command.
                        check_patterns(&patterns)?;
                        Ok(Command::PSubscribe(patterns))
                    }
                    "PUNSUBSCRIBE" => {
                        let patterns: Vec<String> =
                            arr[1..].iter_mut().filter_map(take_string).collect();
                        check_patterns(&patterns)?;
                        Ok(Command::PUnsubscribe(patterns))
                    }
                    "PUBLISH" => {
                        need!(3);
                        Ok(Command::Publish(
                            extract_string(&arr[1]).unwrap_or_default(),
                            extract_bytes(&arr[2]).unwrap_or_default(),
                        ))
                    }

                    // ── Observable keys ───────────────────────────────────────
                    "WATCH" => {
                        need!(2);
                        Ok(Command::Watch(
                            arr[1..].iter_mut().filter_map(take_string).collect(),
                        ))
                    }
                    "UNWATCH" => Ok(Command::Unwatch(
                        arr[1..].iter_mut().filter_map(take_string).collect(),
                    )),

                    // ── Persistence ───────────────────────────────────────────
                    "SAVE" => Ok(Command::Save),
                    "BGSAVE" => Ok(Command::BgSave),
                    "LASTSAVE" => Ok(Command::LastSave),

                    // ── Replication ───────────────────────────────────────────
                    "REPLICAOF" => {
                        need!(3);
                        let arg1 = extract_string(&arr[1]).unwrap_or_default().to_uppercase();
                        let arg2 = extract_string(&arr[2]).unwrap_or_default().to_uppercase();
                        if arg1 == "NO" && arg2 == "ONE" {
                            Ok(Command::ReplicaOfNoOne)
                        } else {
                            Err("ERR REPLICAOF supports only 'REPLICAOF NO ONE' at runtime"
                                .to_string())
                        }
                    }

                    _ => Ok(Command::Unknown(cmd_name.to_owned())),
                }
            }
            _ => Err("Commands must be RESP Arrays".to_string()),
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

const MAX_KEY_BYTES: usize = 512 * 1024; // 512 KB

fn validate_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("ERR key cannot be empty".to_string());
    }
    if key.len() > MAX_KEY_BYTES {
        return Err(format!(
            "ERR key too large ({} > {} bytes)",
            key.len(),
            MAX_KEY_BYTES
        ));
    }
    Ok(())
}

/// Parse and validate the `limit window_secs` pair shared by RLSET and RLCHECK.
fn parse_rl_config(limit: &Value, window: &Value, cmd: &str) -> Result<(u64, u64), String> {
    let limit = extract_int(limit)?;
    let window = extract_int(window)?;
    if limit <= 0 {
        return Err(format!("ERR limit must be >= 1 in '{}' command", cmd));
    }
    if window <= 0 {
        return Err(format!("ERR window must be >= 1 in '{}' command", cmd));
    }
    Ok((limit as u64, window as u64))
}

/// Move every argument out as a string, for the commands whose arguments are
/// an opaque subcommand tail (`CLIENT`, `CONFIG`, `COMMAND`).
fn collect_strings(vals: &mut [Value]) -> Vec<String> {
    vals.iter_mut().filter_map(take_string).collect()
}

fn extract_key(val: &Value) -> Result<String, String> {
    let key = extract_string(val).unwrap_or_default();
    validate_key(&key)?;
    Ok(key)
}

fn extract_keys(vals: &[Value]) -> Result<Vec<String>, String> {
    vals.iter().map(extract_key).collect()
}

/// Take a string argument by **moving** its bytes out of the parsed frame.
///
/// `extract_string` borrows and therefore copies: `from_utf8_lossy(..).into_owned()`
/// allocates a fresh `String` and memcpys the payload even when the bytes are
/// already valid UTF-8, which they almost always are. Moving reuses the `Vec`
/// the parser already allocated, so a 1 MB `SET` value costs no copy at all.
///
/// The slot is left as a nil array, so an arm must not read the same index
/// twice — call this last, once per argument.
/// Is argument `i` of `cmd_name` a value payload rather than an identifier?
///
/// Payloads are stored verbatim and may be any bytes. Everything else is used
/// as a lookup key, a glob pattern or a routing name, and must be text. Index 0
/// is the command name and is never a payload.
fn is_payload_index(cmd_name: &str, i: usize) -> bool {
    match cmd_name {
        // key value
        "SET" | "ESET" | "APPEND" | "GETSET" | "SETNX" => i == 2,
        // key ttl value
        "SETEX" | "PSETEX" => i == 3,
        // key field value
        "HSETNX" => i == 3,
        // key index value / key count value
        "LSET" | "LREM" => i == 3,
        // channel message
        "PUBLISH" => i == 2,
        // key v1 v2 …
        "LPUSH" | "RPUSH" | "LPUSHX" | "RPUSHX" => i >= 2,
        // key f1 v1 f2 v2 … — values are the odd positions from 3
        "HSET" | "HMSET" => i >= 3 && !i.is_multiple_of(2),
        // k1 v1 k2 v2 … — values are the even positions from 2
        "MSET" => i >= 2 && i.is_multiple_of(2),
        _ => false,
    }
}

/// Moving counterpart of `take_string` for payload arguments: takes the bytes
/// as they arrived, with no UTF-8 conversion of any kind.
fn take_bytes(val: &mut Value) -> Option<Vec<u8>> {
    match std::mem::replace(val, Value::Array(None)) {
        Value::BulkString(Some(data)) => Some(data),
        Value::SimpleString(s) => Some(s.into_bytes()),
        _ => None,
    }
}

fn take_string(val: &mut Value) -> Option<String> {
    match std::mem::replace(val, Value::Array(None)) {
        // `from_utf8` reuses the buffer, so this is a move rather than a copy.
        // `from_value` has already rejected non-UTF-8 arguments, so the lossy
        // branch is unreachable in practice — it is kept only so a future
        // caller that bypasses that check degrades rather than panics.
        Value::BulkString(Some(data)) => Some(
            String::from_utf8(data)
                .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()),
        ),
        Value::SimpleString(s) => Some(s),
        _ => None,
    }
}

/// Moving counterpart of `extract_key`, with the same validation.
fn take_key(val: &mut Value) -> Result<String, String> {
    let key = take_string(val).unwrap_or_default();
    validate_key(&key)?;
    Ok(key)
}

/// Borrowing counterpart of `take_bytes`, for payload arguments read without
/// consuming the frame.
fn extract_bytes(val: &Value) -> Option<Vec<u8>> {
    match val {
        Value::BulkString(Some(data)) => Some(data.clone()),
        Value::SimpleString(s) => Some(s.as_bytes().to_vec()),
        _ => None,
    }
}

fn extract_string(val: &Value) -> Option<String> {
    match val {
        Value::BulkString(Some(data)) => Some(String::from_utf8_lossy(data).into_owned()),
        Value::SimpleString(s) => Some(s.clone()),
        _ => None,
    }
}

fn extract_int(val: &Value) -> Result<i64, String> {
    match val {
        Value::BulkString(Some(data)) => String::from_utf8_lossy(data)
            .parse::<i64>()
            .map_err(|_| "ERR value is not an integer or out of range".to_string()),
        Value::SimpleString(s) => s
            .parse::<i64>()
            .map_err(|_| "ERR value is not an integer or out of range".to_string()),
        Value::Integer(i) => Ok(*i),
        _ => Err("ERR value is not an integer or out of range".to_string()),
    }
}

/// An element count that must not be negative.
///
/// `extract_int` yields `i64`, and casting that straight to `u64` turns `-1`
/// into `u64::MAX`. Downstream that reads as "every element" — `SPOP key -1`
/// emptied the whole set — or as a loop whose bound outlives the process.
/// Redis answers a negative count here with an error, so this does too, and
/// every other numeric argument in this parser already guards its sign the
/// same way (`SET EX`, `SETEX`, the `EXPIRE` family, `RLSET`, `SCAN COUNT`).
fn extract_count(val: &Value) -> Result<u64, String> {
    let n = extract_int(val)?;
    u64::try_from(n).map_err(|_| "ERR value is out of range, must be positive".to_string())
}

fn extract_float(val: &Value) -> Result<f64, String> {
    match val {
        Value::BulkString(Some(data)) => {
            let s = String::from_utf8_lossy(data);
            if s == "inf" || s == "+inf" {
                Ok(f64::INFINITY)
            } else if s == "-inf" {
                Ok(f64::NEG_INFINITY)
            } else {
                s.parse::<f64>()
                    .map_err(|_| "ERR value is not a valid float".to_string())
            }
        }
        Value::SimpleString(s) => s
            .parse::<f64>()
            .map_err(|_| "ERR value is not a valid float".to_string()),
        Value::Integer(i) => Ok(*i as f64),
        _ => Err("ERR value is not a valid float".to_string()),
    }
}

/// Parse the `cursor [MATCH pat] [COUNT n] [NOVALUES]` tail shared by `SCAN`
/// and the collection scans. `novalues_ok` gates the HSCAN-only token, so
/// `SSCAN k 0 NOVALUES` is a syntax error rather than a silently ignored
/// argument — a client that asked for a shape it did not get is worse off than
/// one told no.
fn parse_scan_args(
    cursor: &Value,
    tokens: &[Value],
    novalues_ok: bool,
) -> Result<ScanArgs, String> {
    let cursor = extract_string(cursor)
        .unwrap_or_default()
        .parse::<u64>()
        .map_err(|_| "ERR invalid cursor".to_string())?;
    let mut args = ScanArgs {
        cursor,
        ..Default::default()
    };
    let mut i = 0usize;
    while i < tokens.len() {
        let opt = extract_string(&tokens[i])
            .unwrap_or_default()
            .to_uppercase();
        match opt.as_str() {
            "MATCH" => {
                i += 1;
                if i >= tokens.len() {
                    return Err("ERR syntax error".to_string());
                }
                let pattern = extract_string(&tokens[i]);
                if let Some(p) = &pattern {
                    check_pattern(p)?;
                }
                args.pattern = pattern;
            }
            "COUNT" => {
                i += 1;
                if i >= tokens.len() {
                    return Err("ERR syntax error".to_string());
                }
                // Redis rejects a non-positive COUNT rather than treating it as
                // "unbounded"; casting it to usize here would wrap into one.
                let n = extract_int(&tokens[i])?;
                if n < 1 {
                    return Err("ERR syntax error".to_string());
                }
                args.count = Some(n as usize);
            }
            "NOVALUES" if novalues_ok => args.novalues = true,
            _ => return Err("ERR syntax error".to_string()),
        }
        i += 1;
    }
    Ok(args)
}

/// Parse `[WITHSCORES] [LIMIT offset count]` options for ZRANGEBYSCORE / ZREVRANGEBYSCORE.
fn parse_zrange_opts(tokens: &[Value]) -> Result<(bool, Option<(i64, i64)>), String> {
    let mut withscores = false;
    let mut limit = None;
    let mut i = 0usize;
    while i < tokens.len() {
        let opt = extract_string(&tokens[i])
            .unwrap_or_default()
            .to_uppercase();
        match opt.as_str() {
            "WITHSCORES" => {
                withscores = true;
                i += 1;
            }
            "LIMIT" => {
                if i + 2 >= tokens.len() {
                    return Err("ERR syntax error".to_string());
                }
                let offset = extract_int(&tokens[i + 1])?;
                let count = extract_int(&tokens[i + 2])?;
                limit = Some((offset, count));
                i += 3;
            }
            _ => return Err("ERR syntax error".to_string()),
        }
    }
    Ok((withscores, limit))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn bulk(s: &str) -> Value {
        Value::BulkString(Some(s.as_bytes().to_vec()))
    }

    fn array(parts: &[&str]) -> Value {
        Value::Array(Some(parts.iter().map(|s| bulk(s)).collect()))
    }

    // ── legacy string/key parsing unchanged ─────────────────────────────────
    #[test]
    fn ping_no_arg() {
        assert_eq!(
            Command::from_value(array(&["PING"])).unwrap(),
            Command::Ping(None)
        );
    }
    #[test]
    fn set_plain() {
        let cmd = Command::from_value(array(&["SET", "k", "v"])).unwrap();
        assert_eq!(
            cmd,
            Command::Set("k".into(), "v".into(), SetOptions::default())
        );
    }
    #[test]
    fn set_ex_nx() {
        let cmd = Command::from_value(array(&["SET", "k", "v", "EX", "10", "NX"])).unwrap();
        assert_eq!(
            cmd,
            Command::Set(
                "k".into(),
                "v".into(),
                SetOptions {
                    expiry: Some(SetExpiry::Ex(10)),
                    condition: Some(SetCondition::Nx),
                    get: false,
                }
            )
        );
    }

    // ── Hash ──────────────────────────────────────────────────────────────────
    #[test]
    fn hset_single() {
        let cmd = Command::from_value(array(&["HSET", "h", "f", "v"])).unwrap();
        assert_eq!(
            cmd,
            Command::HSet("h".into(), vec![("f".into(), "v".into())])
        );
    }
    #[test]
    fn hset_multi() {
        let cmd = Command::from_value(array(&["HSET", "h", "f1", "v1", "f2", "v2"])).unwrap();
        assert_eq!(
            cmd,
            Command::HSet(
                "h".into(),
                vec![("f1".into(), "v1".into()), ("f2".into(), "v2".into())]
            )
        );
    }
    #[test]
    fn hmset_alias() {
        let cmd = Command::from_value(array(&["HMSET", "h", "f", "v"])).unwrap();
        assert!(matches!(cmd, Command::HSet(..)));
    }
    #[test]
    fn hget_ok() {
        assert_eq!(
            Command::from_value(array(&["HGET", "h", "f"])).unwrap(),
            Command::HGet("h".into(), "f".into())
        );
    }
    #[test]
    fn hincrby_ok() {
        assert_eq!(
            Command::from_value(array(&["HINCRBY", "h", "f", "5"])).unwrap(),
            Command::HIncrBy("h".into(), "f".into(), 5)
        );
    }

    // ── List ──────────────────────────────────────────────────────────────────
    #[test]
    fn lpush_multi() {
        let cmd = Command::from_value(array(&["LPUSH", "l", "a", "b", "c"])).unwrap();
        assert_eq!(
            cmd,
            Command::LPush("l".into(), vec!["a".into(), "b".into(), "c".into()])
        );
    }
    #[test]
    fn rpop_with_count() {
        let cmd = Command::from_value(array(&["RPOP", "l", "3"])).unwrap();
        assert_eq!(cmd, Command::RPop("l".into(), Some(3)));
    }
    #[test]
    fn lrange_ok() {
        assert_eq!(
            Command::from_value(array(&["LRANGE", "l", "0", "-1"])).unwrap(),
            Command::LRange("l".into(), 0, -1)
        );
    }

    // ── Set ───────────────────────────────────────────────────────────────────
    #[test]
    fn sadd_multi() {
        let cmd = Command::from_value(array(&["SADD", "s", "a", "b"])).unwrap();
        assert_eq!(cmd, Command::SAdd("s".into(), vec!["a".into(), "b".into()]));
    }
    #[test]
    fn sinter_multi_keys() {
        let cmd = Command::from_value(array(&["SINTER", "s1", "s2", "s3"])).unwrap();
        assert_eq!(
            cmd,
            Command::SInter(vec!["s1".into(), "s2".into(), "s3".into()])
        );
    }
    #[test]
    fn smismember_ok() {
        let cmd = Command::from_value(array(&["SMISMEMBER", "s", "a", "b"])).unwrap();
        assert_eq!(
            cmd,
            Command::SMIsMember("s".into(), vec!["a".into(), "b".into()])
        );
    }

    // ── ZSet ──────────────────────────────────────────────────────────────────
    #[test]
    fn zadd_basic() {
        let cmd = Command::from_value(array(&["ZADD", "z", "1.5", "member"])).unwrap();
        assert_eq!(
            cmd,
            Command::ZAdd(
                "z".into(),
                ZAddOptions::default(),
                vec![(1.5, "member".into())]
            )
        );
    }
    #[test]
    fn zadd_nx_ch() {
        let cmd = Command::from_value(array(&["ZADD", "z", "NX", "CH", "1.0", "m"])).unwrap();
        assert_eq!(
            cmd,
            Command::ZAdd(
                "z".into(),
                ZAddOptions {
                    condition: Some(ZAddCondition::Nx),
                    gt: false,
                    lt: false,
                    ch: true,
                    incr: false
                },
                vec![(1.0, "m".into())]
            )
        );
    }
    #[test]
    fn zadd_incr() {
        let cmd = Command::from_value(array(&["ZADD", "z", "INCR", "2.0", "m"])).unwrap();
        assert!(matches!(
            cmd,
            Command::ZAdd(_, ZAddOptions { incr: true, .. }, _)
        ));
    }
    #[test]
    fn zadd_incr_multiple_pairs_error() {
        let r = Command::from_value(array(&["ZADD", "z", "INCR", "1", "a", "2", "b"]));
        assert!(r.is_err());
    }
    #[test]
    fn zrange_withscores() {
        let cmd = Command::from_value(array(&["ZRANGE", "z", "0", "-1", "WITHSCORES"])).unwrap();
        assert_eq!(cmd, Command::ZRange("z".into(), 0, -1, true));
    }
    #[test]
    fn zrangebyscore_inf() {
        let cmd = Command::from_value(array(&["ZRANGEBYSCORE", "z", "-inf", "+inf"])).unwrap();
        assert_eq!(
            cmd,
            Command::ZRangeByScore("z".into(), "-inf".into(), "+inf".into(), false, None)
        );
    }
    #[test]
    fn zrangebyscore_limit() {
        let cmd = Command::from_value(array(&[
            "ZRANGEBYSCORE",
            "z",
            "0",
            "100",
            "WITHSCORES",
            "LIMIT",
            "0",
            "10",
        ]))
        .unwrap();
        assert_eq!(
            cmd,
            Command::ZRangeByScore("z".into(), "0".into(), "100".into(), true, Some((0, 10)))
        );
    }
    #[test]
    fn unknown_command() {
        assert!(matches!(
            Command::from_value(array(&["BLPOP", "k", "0"])).unwrap(),
            Command::Unknown(_)
        ));
    }
    #[test]
    fn non_array_input() {
        assert!(Command::from_value(Value::SimpleString("PING".into())).is_err());
    }

    // ── INFO ──────────────────────────────────────────────────────────────
    #[test]
    fn info_without_arguments_requests_the_default_sections() {
        assert_eq!(
            Command::from_value(array(&["INFO"])).unwrap(),
            Command::Info(vec![])
        );
    }
    #[test]
    fn info_lowercases_section_names() {
        assert_eq!(
            Command::from_value(array(&["INFO", "Server"])).unwrap(),
            Command::Info(vec!["server".into()])
        );
    }
    #[test]
    fn info_accepts_multiple_sections() {
        assert_eq!(
            Command::from_value(array(&["INFO", "server", "REPLICATION"])).unwrap(),
            Command::Info(vec!["server".into(), "replication".into()])
        );
    }
    #[test]
    fn info_is_case_insensitive_like_every_other_command() {
        assert_eq!(
            Command::from_value(array(&["info"])).unwrap(),
            Command::Info(vec![])
        );
    }

    // ── Phase 4: Transactions ─────────────────────────────────────────────
    #[test]
    fn multi_exec_discard_parse() {
        assert_eq!(
            Command::from_value(array(&["MULTI"])).unwrap(),
            Command::Multi
        );
        assert_eq!(
            Command::from_value(array(&["EXEC"])).unwrap(),
            Command::Exec
        );
        assert_eq!(
            Command::from_value(array(&["DISCARD"])).unwrap(),
            Command::Discard
        );
    }

    // ── Phase 4: Pub/Sub ──────────────────────────────────────────────────
    #[test]
    fn subscribe_parse() {
        let cmd = Command::from_value(array(&["SUBSCRIBE", "ch1", "ch2"])).unwrap();
        assert_eq!(cmd, Command::Subscribe(vec!["ch1".into(), "ch2".into()]));
    }
    #[test]
    fn subscribe_requires_channel() {
        assert!(Command::from_value(array(&["SUBSCRIBE"])).is_err());
    }
    #[test]
    fn unsubscribe_no_args() {
        let cmd = Command::from_value(array(&["UNSUBSCRIBE"])).unwrap();
        assert_eq!(cmd, Command::Unsubscribe(vec![]));
    }
    #[test]
    fn psubscribe_parse() {
        let cmd = Command::from_value(array(&["PSUBSCRIBE", "news.*"])).unwrap();
        assert_eq!(cmd, Command::PSubscribe(vec!["news.*".into()]));
    }
    #[test]
    fn publish_parse() {
        let cmd = Command::from_value(array(&["PUBLISH", "chan", "hello"])).unwrap();
        assert_eq!(cmd, Command::Publish("chan".into(), "hello".into()));
    }
    #[test]
    fn publish_requires_args() {
        assert!(Command::from_value(array(&["PUBLISH", "chan"])).is_err());
    }

    // ── Strings (parsing) ─────────────────────────────────────────────────────

    #[test]
    fn get_parse() {
        assert_eq!(
            Command::from_value(array(&["GET", "k"])).unwrap(),
            Command::Get("k".into())
        );
    }

    #[test]
    fn del_multi_key_parse() {
        assert_eq!(
            Command::from_value(array(&["DEL", "a", "b", "c"])).unwrap(),
            Command::Del(vec!["a".into(), "b".into(), "c".into()])
        );
    }

    #[test]
    fn unlink_parse() {
        assert_eq!(
            Command::from_value(array(&["UNLINK", "a", "b"])).unwrap(),
            Command::Unlink(vec!["a".into(), "b".into()])
        );
    }

    #[test]
    fn append_parse() {
        assert_eq!(
            Command::from_value(array(&["APPEND", "k", "v"])).unwrap(),
            Command::Append("k".into(), "v".into())
        );
    }

    #[test]
    fn strlen_parse() {
        assert_eq!(
            Command::from_value(array(&["STRLEN", "k"])).unwrap(),
            Command::Strlen("k".into())
        );
    }

    #[test]
    fn getset_parse() {
        assert_eq!(
            Command::from_value(array(&["GETSET", "k", "v"])).unwrap(),
            Command::GetSet("k".into(), "v".into())
        );
    }

    #[test]
    fn mget_parse() {
        assert_eq!(
            Command::from_value(array(&["MGET", "a", "b", "c"])).unwrap(),
            Command::MGet(vec!["a".into(), "b".into(), "c".into()])
        );
    }

    #[test]
    fn mset_parse() {
        assert_eq!(
            Command::from_value(array(&["MSET", "a", "1", "b", "2"])).unwrap(),
            Command::MSet(vec![("a".into(), "1".into()), ("b".into(), "2".into())])
        );
    }

    #[test]
    fn mset_odd_args_error() {
        assert!(Command::from_value(array(&["MSET", "a", "1", "b"])).is_err());
    }

    #[test]
    fn setnx_parse() {
        assert_eq!(
            Command::from_value(array(&["SETNX", "k", "v"])).unwrap(),
            Command::SetNx("k".into(), "v".into())
        );
    }

    #[test]
    fn setex_parse() {
        assert_eq!(
            Command::from_value(array(&["SETEX", "k", "60", "v"])).unwrap(),
            Command::SetEx("k".into(), 60, "v".into())
        );
    }

    #[test]
    fn psetex_parse() {
        assert_eq!(
            Command::from_value(array(&["PSETEX", "k", "5000", "v"])).unwrap(),
            Command::PSetEx("k".into(), 5000, "v".into())
        );
    }

    #[test]
    fn incr_decr_parse() {
        assert_eq!(
            Command::from_value(array(&["INCR", "k"])).unwrap(),
            Command::Incr("k".into())
        );
        assert_eq!(
            Command::from_value(array(&["DECR", "k"])).unwrap(),
            Command::Decr("k".into())
        );
    }

    #[test]
    fn incrby_decrby_parse() {
        assert_eq!(
            Command::from_value(array(&["INCRBY", "k", "5"])).unwrap(),
            Command::IncrBy("k".into(), 5)
        );
        assert_eq!(
            Command::from_value(array(&["DECRBY", "k", "3"])).unwrap(),
            Command::DecrBy("k".into(), 3)
        );
    }

    // ── Expiry (parsing) ──────────────────────────────────────────────────────

    #[test]
    fn expire_pexpire_parse() {
        assert_eq!(
            Command::from_value(array(&["EXPIRE", "k", "60"])).unwrap(),
            Command::Expire("k".into(), 60)
        );
        assert_eq!(
            Command::from_value(array(&["PEXPIRE", "k", "5000"])).unwrap(),
            Command::PExpire("k".into(), 5000)
        );
    }

    #[test]
    fn expireat_pexpireat_parse() {
        assert_eq!(
            Command::from_value(array(&["EXPIREAT", "k", "9999999999"])).unwrap(),
            Command::ExpireAt("k".into(), 9999999999)
        );
        assert_eq!(
            Command::from_value(array(&["PEXPIREAT", "k", "9999999999000"])).unwrap(),
            Command::PExpireAt("k".into(), 9999999999000)
        );
    }

    #[test]
    fn ttl_pttl_parse() {
        assert_eq!(
            Command::from_value(array(&["TTL", "k"])).unwrap(),
            Command::Ttl("k".into())
        );
        assert_eq!(
            Command::from_value(array(&["PTTL", "k"])).unwrap(),
            Command::PTtl("k".into())
        );
    }

    #[test]
    fn persist_parse() {
        assert_eq!(
            Command::from_value(array(&["PERSIST", "k"])).unwrap(),
            Command::Persist("k".into())
        );
    }

    // ── Keys (parsing) ────────────────────────────────────────────────────────

    #[test]
    fn exists_parse() {
        assert_eq!(
            Command::from_value(array(&["EXISTS", "a", "b"])).unwrap(),
            Command::Exists(vec!["a".into(), "b".into()])
        );
    }

    #[test]
    fn keys_parse() {
        assert_eq!(
            Command::from_value(array(&["KEYS", "user:*"])).unwrap(),
            Command::Keys("user:*".into())
        );
    }

    #[test]
    fn scan_parse_with_count() {
        assert_eq!(
            Command::from_value(array(&["SCAN", "0", "MATCH", "*", "COUNT", "10"])).unwrap(),
            Command::Scan(0, Some("*".into()), Some(10))
        );
    }

    #[test]
    fn handshake_commands_parse() {
        assert_eq!(
            Command::from_value(array(&["QUIT"])).unwrap(),
            Command::Quit
        );
        assert_eq!(
            Command::from_value(array(&["CLIENT", "SETINFO", "LIB-NAME", "node-redis"])).unwrap(),
            Command::Client(vec![
                "SETINFO".into(),
                "LIB-NAME".into(),
                "node-redis".into()
            ])
        );
        assert_eq!(
            Command::from_value(array(&["CONFIG", "GET", "maxmemory*"])).unwrap(),
            Command::Config(vec!["GET".into(), "maxmemory*".into()])
        );
        // Bare COMMAND is valid and means "the whole catalog".
        assert_eq!(
            Command::from_value(array(&["COMMAND"])).unwrap(),
            Command::CommandQuery(vec![])
        );
        assert_eq!(
            Command::from_value(array(&["COMMAND", "DOCS", "get"])).unwrap(),
            Command::CommandQuery(vec!["DOCS".into(), "get".into()])
        );
        // CLIENT and CONFIG need a subcommand; the arity check is the parser's.
        assert!(Command::from_value(array(&["CLIENT"])).is_err());
        assert!(Command::from_value(array(&["CONFIG"])).is_err());
    }

    #[test]
    fn memory_usage_parses_as_a_key_read_and_the_rest_as_a_container() {
        assert_eq!(
            Command::from_value(array(&["MEMORY", "USAGE", "k"])).unwrap(),
            Command::MemoryUsage("k".into())
        );
        // SAMPLES is accepted and its count discarded — the measurement always
        // walks the whole value, so there is nothing for the count to bound.
        assert_eq!(
            Command::from_value(array(&["MEMORY", "usage", "k", "samples", "5"])).unwrap(),
            Command::MemoryUsage("k".into())
        );
        assert_eq!(
            Command::from_value(array(&["MEMORY", "USAGE", "k", "SAMPLES", "0"])).unwrap(),
            Command::MemoryUsage("k".into())
        );
        // Every other subcommand stays a container command, answered by the
        // server layer, so that MEMORY HELP and MEMORY DOCTOR get their own
        // replies instead of a parse error naming the wrong thing.
        assert_eq!(
            Command::from_value(array(&["MEMORY", "DOCTOR"])).unwrap(),
            Command::Memory(vec!["DOCTOR".into()])
        );
        assert!(Command::from_value(array(&["MEMORY"])).is_err());
    }

    #[test]
    fn memory_usage_rejects_what_redis_rejects() {
        // Each of these is `ERR syntax error` on redis-server 7.2.5, including
        // the negative SAMPLES — which is a syntax error there, not a range
        // error, so it is one here too.
        for bad in [
            vec!["MEMORY", "USAGE"],
            vec!["MEMORY", "USAGE", "k", "SAMPLES"],
            vec!["MEMORY", "USAGE", "k", "SAMPLES", "-1"],
            vec!["MEMORY", "USAGE", "k", "BOGUS", "1"],
            vec!["MEMORY", "USAGE", "k", "SAMPLES", "1", "EXTRA"],
        ] {
            assert!(
                Command::from_value(array(&bad)).is_err(),
                "{bad:?} should not parse"
            );
        }
        assert_eq!(
            Command::from_value(array(&["MEMORY", "USAGE", "k", "SAMPLES", "-1"])).unwrap_err(),
            "ERR syntax error"
        );
    }

    #[test]
    fn cluster_module_and_pubsub_parse_as_containers() {
        assert_eq!(
            Command::from_value(array(&["CLUSTER", "INFO"])).unwrap(),
            Command::Cluster(vec!["INFO".into()])
        );
        assert_eq!(
            Command::from_value(array(&["MODULE", "LIST"])).unwrap(),
            Command::Module(vec!["LIST".into()])
        );
        assert_eq!(
            Command::from_value(array(&["PUBSUB", "NUMSUB", "a", "b"])).unwrap(),
            Command::PubSub(vec!["NUMSUB".into(), "a".into(), "b".into()])
        );
        // A bare container is an arity error, as it is for CLIENT and CONFIG.
        assert!(Command::from_value(array(&["CLUSTER"])).is_err());
        assert!(Command::from_value(array(&["MODULE"])).is_err());
        assert!(Command::from_value(array(&["PUBSUB"])).is_err());
    }

    #[test]
    fn getrange_parse() {
        assert_eq!(
            Command::from_value(array(&["GETRANGE", "k", "0", "-1"])).unwrap(),
            Command::GetRange("k".into(), 0, -1)
        );
        assert!(Command::from_value(array(&["GETRANGE", "k", "0"])).is_err());
        assert!(Command::from_value(array(&["GETRANGE", "k", "0", "x"])).is_err());
    }

    #[test]
    fn hscan_parse_full_options() {
        assert_eq!(
            Command::from_value(array(&[
                "HSCAN", "h", "12", "MATCH", "u:*", "COUNT", "50", "NOVALUES"
            ]))
            .unwrap(),
            Command::HScan(
                "h".into(),
                ScanArgs {
                    cursor: 12,
                    pattern: Some("u:*".into()),
                    count: Some(50),
                    novalues: true,
                }
            )
        );
    }

    #[test]
    fn sscan_and_zscan_parse_without_novalues() {
        assert_eq!(
            Command::from_value(array(&["SSCAN", "s", "0"])).unwrap(),
            Command::SScan("s".into(), ScanArgs::default())
        );
        assert_eq!(
            Command::from_value(array(&["ZSCAN", "z", "0", "COUNT", "3"])).unwrap(),
            Command::ZScan(
                "z".into(),
                ScanArgs {
                    count: Some(3),
                    ..Default::default()
                }
            )
        );
        // NOVALUES is HSCAN-only. Accepting and ignoring it would hand back a
        // reply shape the caller did not ask for.
        assert!(Command::from_value(array(&["SSCAN", "s", "0", "NOVALUES"])).is_err());
    }

    #[test]
    fn scan_family_rejects_bad_cursors_and_counts() {
        for cmd in [
            vec!["SCAN", "abc"],
            vec!["HSCAN", "h", "abc"],
            vec!["SSCAN", "s", "-1"],
        ] {
            let err = Command::from_value(array(&cmd)).unwrap_err();
            assert_eq!(err, "ERR invalid cursor", "{cmd:?}");
        }
        for cmd in [
            vec!["HSCAN", "h", "0", "COUNT", "0"],
            vec!["SCAN", "0", "COUNT", "-5"],
            vec!["ZSCAN", "z", "0", "MATCH"],
            vec!["SSCAN", "s", "0", "BOGUS"],
        ] {
            let err = Command::from_value(array(&cmd)).unwrap_err();
            assert_eq!(err, "ERR syntax error", "{cmd:?}");
        }
    }

    #[test]
    fn rename_parse() {
        assert_eq!(
            Command::from_value(array(&["RENAME", "src", "dst"])).unwrap(),
            Command::Rename("src".into(), "dst".into())
        );
    }

    #[test]
    fn type_parse() {
        assert_eq!(
            Command::from_value(array(&["TYPE", "k"])).unwrap(),
            Command::Type("k".into())
        );
    }

    #[test]
    fn dbsize_flushdb_parse() {
        assert_eq!(
            Command::from_value(array(&["DBSIZE"])).unwrap(),
            Command::DbSize
        );
        assert_eq!(
            Command::from_value(array(&["FLUSHDB"])).unwrap(),
            Command::FlushDb
        );
    }

    // ── Hash (parsing) ────────────────────────────────────────────────────────

    #[test]
    fn hkeys_hvals_hexists_parse() {
        assert_eq!(
            Command::from_value(array(&["HKEYS", "h"])).unwrap(),
            Command::HKeys("h".into())
        );
        assert_eq!(
            Command::from_value(array(&["HVALS", "h"])).unwrap(),
            Command::HVals("h".into())
        );
        assert_eq!(
            Command::from_value(array(&["HEXISTS", "h", "f"])).unwrap(),
            Command::HExists("h".into(), "f".into())
        );
    }

    #[test]
    fn hdel_multi_field_parse() {
        assert_eq!(
            Command::from_value(array(&["HDEL", "h", "f1", "f2"])).unwrap(),
            Command::HDel("h".into(), vec!["f1".into(), "f2".into()])
        );
    }

    #[test]
    fn hlen_parse() {
        assert_eq!(
            Command::from_value(array(&["HLEN", "h"])).unwrap(),
            Command::HLen("h".into())
        );
    }

    // ── List (parsing) ────────────────────────────────────────────────────────

    #[test]
    fn lpushx_rpushx_parse() {
        assert_eq!(
            Command::from_value(array(&["LPUSHX", "l", "v"])).unwrap(),
            Command::LPushX("l".into(), vec!["v".into()])
        );
        assert_eq!(
            Command::from_value(array(&["RPUSHX", "l", "v"])).unwrap(),
            Command::RPushX("l".into(), vec!["v".into()])
        );
    }

    #[test]
    fn lindex_lset_lrem_ltrim_parse() {
        assert_eq!(
            Command::from_value(array(&["LINDEX", "l", "0"])).unwrap(),
            Command::LIndex("l".into(), 0)
        );
        assert_eq!(
            Command::from_value(array(&["LSET", "l", "0", "v"])).unwrap(),
            Command::LSet("l".into(), 0, "v".into())
        );
        assert_eq!(
            Command::from_value(array(&["LREM", "l", "1", "v"])).unwrap(),
            Command::LRem("l".into(), 1, "v".into())
        );
        assert_eq!(
            Command::from_value(array(&["LTRIM", "l", "0", "9"])).unwrap(),
            Command::LTrim("l".into(), 0, 9)
        );
    }

    // ── Negative counts ───────────────────────────────────────────────────────

    #[test]
    fn a_negative_pop_count_is_refused_rather_than_wrapped() {
        // `extract_int` yields i64 and these three used to cast straight to
        // u64, so `-1` arrived as 18446744073709551615. `SPOP key -1` then
        // emptied the whole set and `LPOP key -1` span for centuries holding
        // the shard guard. Redis answers all three with an error.
        for cmd in ["LPOP", "RPOP", "SPOP"] {
            let err = Command::from_value(array(&[cmd, "k", "-1"]))
                .expect_err(&format!("{cmd} with a negative count must be refused"));
            assert_eq!(err, "ERR value is out of range, must be positive");
        }
    }

    #[test]
    fn a_zero_pop_count_is_still_allowed() {
        // Zero is not negative: Redis answers it with an empty array.
        assert_eq!(
            Command::from_value(array(&["LPOP", "k", "0"])).unwrap(),
            Command::LPop("k".into(), Some(0))
        );
        assert_eq!(
            Command::from_value(array(&["SPOP", "k", "0"])).unwrap(),
            Command::SPop("k".into(), Some(0))
        );
    }

    #[test]
    fn srandmember_keeps_its_sign_but_not_an_unbounded_magnitude() {
        // Negative is meaningful here — it means "with repetition" — so unlike
        // the pop counts it is kept, and only its magnitude is capped.
        assert_eq!(
            Command::from_value(array(&["SRANDMEMBER", "s", "-3"])).unwrap(),
            Command::SRandMember("s".into(), Some(-3))
        );
        let err = Command::from_value(array(&["SRANDMEMBER", "s", "-9223372036854775808"]))
            .expect_err("a magnitude beyond a parseable reply must be refused");
        assert_eq!(err, "ERR count is out of range in 'srandmember' command");
    }

    // ── Set (parsing) ─────────────────────────────────────────────────────────

    #[test]
    fn spop_parse() {
        assert_eq!(
            Command::from_value(array(&["SPOP", "s"])).unwrap(),
            Command::SPop("s".into(), None)
        );
        assert_eq!(
            Command::from_value(array(&["SPOP", "s", "3"])).unwrap(),
            Command::SPop("s".into(), Some(3))
        );
    }

    #[test]
    fn srandmember_parse() {
        assert_eq!(
            Command::from_value(array(&["SRANDMEMBER", "s"])).unwrap(),
            Command::SRandMember("s".into(), None)
        );
        assert_eq!(
            Command::from_value(array(&["SRANDMEMBER", "s", "-5"])).unwrap(),
            Command::SRandMember("s".into(), Some(-5))
        );
    }

    #[test]
    fn sinterstore_sdiffstore_parse() {
        assert_eq!(
            Command::from_value(array(&["SINTERSTORE", "dst", "s1", "s2"])).unwrap(),
            Command::SInterStore("dst".into(), vec!["s1".into(), "s2".into()])
        );
        assert_eq!(
            Command::from_value(array(&["SDIFFSTORE", "dst", "s1", "s2"])).unwrap(),
            Command::SDiffStore("dst".into(), vec!["s1".into(), "s2".into()])
        );
        assert_eq!(
            Command::from_value(array(&["SUNIONSTORE", "dst", "s1"])).unwrap(),
            Command::SUnionStore("dst".into(), vec!["s1".into()])
        );
    }

    // ── Sorted Set (parsing) ──────────────────────────────────────────────────

    #[test]
    fn zmscore_parse() {
        assert_eq!(
            Command::from_value(array(&["ZMSCORE", "z", "a", "b"])).unwrap(),
            Command::ZMScore("z".into(), vec!["a".into(), "b".into()])
        );
    }

    #[test]
    fn zrevrangebyscore_parse() {
        assert_eq!(
            Command::from_value(array(&["ZREVRANGEBYSCORE", "z", "+inf", "-inf"])).unwrap(),
            Command::ZRevRangeByScore("z".into(), "+inf".into(), "-inf".into(), false, None)
        );
    }

    #[test]
    fn zrevrangebyscore_with_limit_parse() {
        let cmd = Command::from_value(array(&[
            "ZREVRANGEBYSCORE",
            "z",
            "100",
            "0",
            "LIMIT",
            "0",
            "5",
        ]))
        .unwrap();
        assert_eq!(
            cmd,
            Command::ZRevRangeByScore("z".into(), "100".into(), "0".into(), false, Some((0, 5)))
        );
    }

    #[test]
    fn zrevrange_parse() {
        assert_eq!(
            Command::from_value(array(&["ZREVRANGE", "z", "0", "-1"])).unwrap(),
            Command::ZRevRange("z".into(), 0, -1, false)
        );
    }

    #[test]
    fn zrank_zrevrank_zcard_zcount_parse() {
        assert_eq!(
            Command::from_value(array(&["ZRANK", "z", "m"])).unwrap(),
            Command::ZRank("z".into(), "m".into())
        );
        assert_eq!(
            Command::from_value(array(&["ZREVRANK", "z", "m"])).unwrap(),
            Command::ZRevRank("z".into(), "m".into())
        );
        assert_eq!(
            Command::from_value(array(&["ZCARD", "z"])).unwrap(),
            Command::ZCard("z".into())
        );
        assert_eq!(
            Command::from_value(array(&["ZCOUNT", "z", "-inf", "+inf"])).unwrap(),
            Command::ZCount("z".into(), "-inf".into(), "+inf".into())
        );
    }

    // ── Duplicate-suppressed replay ───────────────────────────────────────────

    #[test]
    fn dedup_parse_wraps_inner_command() {
        assert_eq!(
            Command::from_value(array(&["DEDUP", "client-a", "42", "INCRBY", "k", "2"])).unwrap(),
            Command::Dedup(
                "client-a".into(),
                42,
                Box::new(Command::IncrBy("k".into(), 2))
            )
        );
    }

    #[test]
    fn dedup_parse_rejections() {
        // Too few args, bad id, oversized client id, nesting.
        assert!(Command::from_value(array(&["DEDUP", "c", "1"])).is_err());
        assert!(Command::from_value(array(&["DEDUP", "c", "-1", "SET", "k", "v"])).is_err());
        let long = "x".repeat(65);
        assert!(Command::from_value(array(&["DEDUP", &long, "1", "SET", "k", "v"])).is_err());
        assert!(
            Command::from_value(array(&[
                "DEDUP", "c", "1", "DEDUP", "c", "2", "SET", "k", "v"
            ]))
            .is_err()
        );
    }

    // ── Rate limiting ───────────────────────────────────────────────────────

    #[test]
    fn rlset_parse() {
        assert_eq!(
            Command::from_value(array(&["RLSET", "api", "100", "60"])).unwrap(),
            Command::RlSet("api".into(), 100, 60)
        );
    }

    #[test]
    fn rlset_rejects_non_positive_config() {
        assert!(Command::from_value(array(&["RLSET", "api", "0", "60"])).is_err());
        assert!(Command::from_value(array(&["RLSET", "api", "10", "0"])).is_err());
        assert!(Command::from_value(array(&["RLSET", "api", "-1", "60"])).is_err());
    }

    #[test]
    fn rlcheck_bare_parse() {
        assert_eq!(
            Command::from_value(array(&["RLCHECK", "api"])).unwrap(),
            Command::RlCheck("api".into(), None)
        );
    }

    #[test]
    fn rlcheck_inline_config_parse() {
        assert_eq!(
            Command::from_value(array(&["RLCHECK", "ip:1.2.3.4", "5", "60"])).unwrap(),
            Command::RlCheck("ip:1.2.3.4".into(), Some((5, 60)))
        );
    }

    #[test]
    fn rlcheck_partial_config_errors() {
        assert!(Command::from_value(array(&["RLCHECK", "api", "5"])).is_err());
        assert!(Command::from_value(array(&["RLCHECK"])).is_err());
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    #[test]
    fn save_bgsave_lastsave_parse() {
        assert_eq!(
            Command::from_value(array(&["SAVE"])).unwrap(),
            Command::Save
        );
        assert_eq!(
            Command::from_value(array(&["BGSAVE"])).unwrap(),
            Command::BgSave
        );
        assert_eq!(
            Command::from_value(array(&["LASTSAVE"])).unwrap(),
            Command::LastSave
        );
    }
}

#[cfg(test)]
mod parse_coverage_tests {
    use super::*;

    fn bulk(s: &str) -> Value {
        Value::BulkString(Some(s.as_bytes().to_vec()))
    }

    fn array(parts: &[&str]) -> Value {
        Value::Array(Some(parts.iter().map(|s| bulk(s)).collect()))
    }

    fn parse(parts: &[&str]) -> Result<Command, String> {
        Command::from_value(array(parts))
    }

    // ── SET expiry options ────────────────────────────────────────────────────
    // Four near-identical parse blocks (EX/PX/EXAT/PXAT). Copy-paste bugs hide
    // in exactly this shape, so each variant is pinned to its own unit.

    #[test]
    fn set_parses_each_expiry_unit() {
        for (kw, expected) in [
            ("EX", SetExpiry::Ex(10)),
            ("PX", SetExpiry::Px(10)),
            ("EXAT", SetExpiry::Exat(10)),
            ("PXAT", SetExpiry::Pxat(10)),
        ] {
            let cmd = parse(&["SET", "k", "v", kw, "10"]).unwrap();
            match cmd {
                Command::Set(_, _, opts) => assert_eq!(
                    opts.expiry,
                    Some(expected),
                    "{kw} mapped to the wrong expiry unit"
                ),
                other => panic!("{kw}: expected Set, got {other:?}"),
            }
        }
    }

    #[test]
    fn set_expiry_options_are_case_insensitive() {
        let cmd = parse(&["SET", "k", "v", "ex", "10"]).unwrap();
        assert!(matches!(cmd, Command::Set(_, _, o) if o.expiry == Some(SetExpiry::Ex(10))));
    }

    #[test]
    fn set_rejects_non_positive_expiry_for_every_unit() {
        for kw in ["EX", "PX", "EXAT", "PXAT"] {
            for bad in ["0", "-1"] {
                let err = parse(&["SET", "k", "v", kw, bad]).unwrap_err();
                assert!(
                    err.contains("invalid expire time"),
                    "{kw} {bad} should be rejected, got: {err}"
                );
            }
        }
    }

    #[test]
    fn set_rejects_expiry_keyword_with_no_value() {
        for kw in ["EX", "PX", "EXAT", "PXAT"] {
            let err = parse(&["SET", "k", "v", kw]).unwrap_err();
            assert!(err.contains("syntax error"), "{kw}: got {err}");
        }
    }

    #[test]
    fn set_parses_keepttl_nx_xx_and_get() {
        assert!(matches!(
            parse(&["SET", "k", "v", "KEEPTTL"]).unwrap(),
            Command::Set(_, _, o) if o.expiry == Some(SetExpiry::KeepTtl)
        ));
        assert!(matches!(
            parse(&["SET", "k", "v", "NX"]).unwrap(),
            Command::Set(_, _, o) if o.condition == Some(SetCondition::Nx)
        ));
        assert!(matches!(
            parse(&["SET", "k", "v", "XX"]).unwrap(),
            Command::Set(_, _, o) if o.condition == Some(SetCondition::Xx)
        ));
        assert!(matches!(
            parse(&["SET", "k", "v", "GET"]).unwrap(),
            Command::Set(_, _, o) if o.get
        ));
    }

    #[test]
    fn set_rejects_unknown_options() {
        assert!(parse(&["SET", "k", "v", "BOGUS"]).is_err());
    }

    // ── Optional-argument commands ────────────────────────────────────────────

    #[test]
    fn lpop_and_rpop_take_an_optional_count() {
        assert_eq!(
            parse(&["LPOP", "l"]).unwrap(),
            Command::LPop("l".into(), None)
        );
        assert_eq!(
            parse(&["LPOP", "l", "3"]).unwrap(),
            Command::LPop("l".into(), Some(3))
        );
        assert_eq!(
            parse(&["RPOP", "l"]).unwrap(),
            Command::RPop("l".into(), None)
        );
        assert_eq!(
            parse(&["RPOP", "l", "2"]).unwrap(),
            Command::RPop("l".into(), Some(2))
        );
    }

    #[test]
    fn jget_path_is_optional() {
        assert_eq!(
            parse(&["JGET", "doc"]).unwrap(),
            Command::JGet("doc".into(), None)
        );
        assert_eq!(
            parse(&["JGET", "doc", "$.a"]).unwrap(),
            Command::JGet("doc".into(), Some("$.a".into()))
        );
    }

    #[test]
    fn jmerge_requires_a_patch() {
        assert!(parse(&["JMERGE", "doc"]).is_err());
        assert_eq!(
            parse(&["JMERGE", "doc", "{\"a\":1}"]).unwrap(),
            Command::JMerge("doc".into(), "{\"a\":1}".into())
        );
    }

    // ── Live queries ──────────────────────────────────────────────────────────

    #[test]
    fn over_long_glob_patterns_are_rejected_at_parse_time() {
        // Matching is O(pattern x text) and runs once per key for KEYS, once per
        // key per write for sync scopes, and once per published message per
        // subscriber for PSUBSCRIBE. Length was previously bounded only by
        // proto-max-bulk-len (64 MB), so one command could occupy the process
        // for an unbounded time.
        let long = "a".repeat(MAX_PATTERN_BYTES + 1);
        let cases: Vec<Vec<String>> = vec![
            vec!["KEYS".into(), long.clone()],
            vec!["SCAN".into(), "0".into(), "MATCH".into(), long.clone()],
            vec![
                "HSCAN".into(),
                "h".into(),
                "0".into(),
                "MATCH".into(),
                long.clone(),
            ],
            vec![
                "SSCAN".into(),
                "s".into(),
                "0".into(),
                "MATCH".into(),
                long.clone(),
            ],
            vec![
                "ZSCAN".into(),
                "z".into(),
                "0".into(),
                "MATCH".into(),
                long.clone(),
            ],
            vec!["QSUB".into(), long.clone()],
            vec!["QUNSUB".into(), long.clone()],
            vec!["PSUBSCRIBE".into(), long.clone()],
            vec!["PUNSUBSCRIBE".into(), long.clone()],
            vec!["SYNC".into(), long.clone()],
            vec!["SYNC".into(), "ok:*".into(), long.clone()],
        ];
        for args in cases {
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let err = parse(&refs)
                .expect_err(&format!("{:?} should reject an over-long pattern", refs[0]));
            assert!(
                err.contains("pattern is too long"),
                "{}: got {err}",
                refs[0]
            );
        }
    }

    #[test]
    fn a_pattern_exactly_at_the_cap_is_accepted() {
        // Off-by-one here would reject the largest legitimate pattern.
        let at_cap = "a".repeat(MAX_PATTERN_BYTES);
        assert_eq!(
            parse(&["KEYS", &at_cap]).unwrap(),
            Command::Keys(at_cap.clone())
        );
        assert!(parse(&["PSUBSCRIBE", &at_cap]).is_ok());
        assert!(parse(&["SYNC", &at_cap]).is_ok());
    }

    #[test]
    fn ordinary_patterns_are_unaffected_by_the_cap() {
        assert_eq!(
            parse(&["KEYS", "user:*"]).unwrap(),
            Command::Keys("user:*".into())
        );
        assert!(parse(&["SCAN", "0", "MATCH", "cart:42:*"]).is_ok());
        assert!(parse(&["QSUB", "cart:*"]).is_ok());
    }

    #[test]
    fn qsub_requires_a_non_empty_pattern() {
        assert_eq!(
            parse(&["QSUB", "cart:*"]).unwrap(),
            Command::QSub("cart:*".into())
        );
        let err = parse(&["QSUB", ""]).unwrap_err();
        assert!(err.contains("non-empty pattern"), "got {err}");
        assert!(parse(&["QSUB"]).is_err(), "QSUB with no pattern");
    }

    #[test]
    fn qunsub_pattern_is_optional() {
        // Bare QUNSUB drops every subscription; with a pattern it drops one.
        assert_eq!(parse(&["QUNSUB"]).unwrap(), Command::QUnsub(None));
        assert_eq!(
            parse(&["QUNSUB", "cart:*"]).unwrap(),
            Command::QUnsub(Some("cart:*".into()))
        );
    }

    // ── Set commands ──────────────────────────────────────────────────────────

    #[test]
    fn set_family_parses_and_enforces_arity() {
        assert_eq!(
            parse(&["SREM", "s", "a", "b"]).unwrap(),
            Command::SRem("s".into(), vec!["a".into(), "b".into()])
        );
        assert_eq!(parse(&["SCARD", "s"]).unwrap(), Command::SCard("s".into()));
        assert_eq!(
            parse(&["SISMEMBER", "s", "a"]).unwrap(),
            Command::SIsMember("s".into(), "a".into())
        );

        for short in [vec!["SREM", "s"], vec!["SCARD"], vec!["SISMEMBER", "s"]] {
            assert!(parse(&short).is_err(), "{short:?} should fail arity check");
        }
    }

    // ── Float coercion ────────────────────────────────────────────────────────
    // Used by ZADD/ZINCRBY scores, where infinities are meaningful sentinels
    // rather than errors.

    #[test]
    fn float_extraction_accepts_infinities() {
        assert_eq!(extract_float(&bulk("inf")).unwrap(), f64::INFINITY);
        assert_eq!(extract_float(&bulk("+inf")).unwrap(), f64::INFINITY);
        assert_eq!(extract_float(&bulk("-inf")).unwrap(), f64::NEG_INFINITY);
    }

    #[test]
    fn float_extraction_accepts_other_value_shapes() {
        assert_eq!(
            extract_float(&Value::SimpleString("1.5".into())).unwrap(),
            1.5
        );
        assert_eq!(extract_float(&Value::Integer(7)).unwrap(), 7.0);
    }

    #[test]
    fn float_extraction_rejects_non_numeric() {
        for bad in [
            bulk("abc"),
            Value::SimpleString("nope".into()),
            Value::Array(None),
        ] {
            let err = extract_float(&bad).unwrap_err();
            assert!(err.contains("not a valid float"), "got {err}");
        }
    }

    // ── Unknown commands ──────────────────────────────────────────────────────

    #[test]
    fn unknown_command_is_reported_not_rejected() {
        // Parsing succeeds so the server can answer with a proper error reply
        // naming the command, rather than dropping the connection.
        assert_eq!(
            parse(&["NOSUCHCOMMAND", "a"]).unwrap(),
            Command::Unknown("NOSUCHCOMMAND".into())
        );
    }
}

#[cfg(test)]
mod arity_and_error_tests {
    use super::*;

    fn bulk(s: &str) -> Value {
        Value::BulkString(Some(s.as_bytes().to_vec()))
    }

    fn array(parts: &[&str]) -> Value {
        Value::Array(Some(parts.iter().map(|s| bulk(s)).collect()))
    }

    fn parse(parts: &[&str]) -> Result<Command, String> {
        Command::from_value(array(parts))
    }

    /// Every command below must reject a call with too few arguments. Table
    /// driven because the arity guard is copy-pasted per command — the failure
    /// mode is one arm being off by one, which only a per-command case catches.
    #[test]
    fn commands_reject_too_few_arguments() {
        let too_short: &[&[&str]] = &[
            &["GET"],
            &["SET", "k"],
            &["SETEX", "k", "10"],
            &["PSETEX", "k", "10"],
            &["SETNX", "k"],
            &["GETSET", "k"],
            &["APPEND", "k"],
            &["STRLEN"],
            &["GETRANGE", "k", "0"],
            &["CLIENT"],
            &["CONFIG"],
            &["INCR"],
            &["DECR"],
            &["INCRBY", "k"],
            &["DECRBY", "k"],
            &["MGET"],
            &["MSET", "k"],
            &["EXPIRE", "k"],
            &["EXPIREAT", "k"],
            &["PEXPIRE", "k"],
            &["TTL"],
            &["PTTL"],
            &["PERSIST"],
            &["DEL"],
            &["EXISTS"],
            &["TYPE"],
            &["RENAME", "k"],
            &["KEYS"],
            &["HSET", "h", "f"],
            &["HGET", "h"],
            &["HDEL", "h"],
            &["HEXISTS", "h"],
            &["HLEN"],
            &["HGETALL"],
            &["HKEYS"],
            &["HVALS"],
            &["HMGET", "h"],
            &["HINCRBY", "h", "f"],
            &["HINCRBYFLOAT", "h", "f"],
            &["HSETNX", "h", "f"],
            &["HSCAN", "h"],
            &["LPUSH", "l"],
            &["RPUSH", "l"],
            &["LPOP"],
            &["RPOP"],
            &["LLEN"],
            &["LRANGE", "l", "0"],
            &["LINDEX", "l"],
            &["LSET", "l", "0"],
            &["LREM", "l", "0"],
            &["LTRIM", "l", "0"],
            &["SADD", "s"],
            &["SREM", "s"],
            &["SMEMBERS"],
            &["SCARD"],
            &["SISMEMBER", "s"],
            &["SINTER"],
            &["SUNION"],
            &["SDIFF"],
            &["SMOVE", "a", "b"],
            &["SPOP"],
            &["SRANDMEMBER"],
            &["SSCAN", "s"],
            &["ZADD", "z", "1"],
            &["ZSCORE", "z"],
            &["ZREM", "z"],
            &["ZCARD"],
            &["ZRANK", "z"],
            &["ZINCRBY", "z", "1"],
            &["ZRANGE", "z", "0"],
            &["ZREVRANGE", "z", "0"],
            &["ZCOUNT", "z", "0"],
            &["ZSCAN", "z"],
            &["JSET", "j", "$"],
            &["JGET"],
            &["JMERGE", "j"],
            &["RLSET", "k", "10"],
            &["RLCHECK"],
            &["SUBSCRIBE"],
            &["PUBLISH", "ch"],
            &["QSUB"],
            &["AUTH"],
            &["REPLICAOF", "NO"],
            &["DEDUP", "client", "1"],
        ];

        for parts in too_short {
            assert!(
                parse(parts).is_err(),
                "{parts:?} should fail its arity check but parsed"
            );
        }
    }

    /// The matching positive case for the same commands: a minimal valid call
    /// must parse. Without this, an arity guard that rejects *everything* would
    /// still pass the test above.
    #[test]
    fn minimal_valid_calls_parse() {
        let valid: &[&[&str]] = &[
            &["GET", "k"],
            &["SET", "k", "v"],
            &["SETEX", "k", "10", "v"],
            &["PSETEX", "k", "10", "v"],
            &["SETNX", "k", "v"],
            &["GETSET", "k", "v"],
            &["APPEND", "k", "v"],
            &["STRLEN", "k"],
            &["INCR", "k"],
            &["DECR", "k"],
            &["INCRBY", "k", "2"],
            &["DECRBY", "k", "2"],
            &["MGET", "a"],
            &["MSET", "k", "v"],
            &["EXPIRE", "k", "10"],
            &["TTL", "k"],
            &["PERSIST", "k"],
            &["DEL", "k"],
            &["EXISTS", "k"],
            &["TYPE", "k"],
            &["RENAME", "a", "b"],
            &["KEYS", "*"],
            &["HSET", "h", "f", "v"],
            &["HGET", "h", "f"],
            &["HDEL", "h", "f"],
            &["HEXISTS", "h", "f"],
            &["HLEN", "h"],
            &["HGETALL", "h"],
            &["HKEYS", "h"],
            &["HVALS", "h"],
            &["HMGET", "h", "f"],
            &["HINCRBY", "h", "f", "1"],
            &["HINCRBYFLOAT", "h", "f", "1.5"],
            &["HSETNX", "h", "f", "v"],
            &["LPUSH", "l", "v"],
            &["RPUSH", "l", "v"],
            &["LLEN", "l"],
            &["LRANGE", "l", "0", "-1"],
            &["LINDEX", "l", "0"],
            &["LSET", "l", "0", "v"],
            &["LREM", "l", "0", "v"],
            &["LTRIM", "l", "0", "-1"],
            &["SADD", "s", "m"],
            &["SREM", "s", "m"],
            &["SMEMBERS", "s"],
            &["SCARD", "s"],
            &["SISMEMBER", "s", "m"],
            &["SINTER", "a"],
            &["SUNION", "a"],
            &["SDIFF", "a"],
            &["SMOVE", "a", "b", "m"],
            &["SPOP", "s"],
            &["SRANDMEMBER", "s"],
            &["ZADD", "z", "1", "m"],
            &["ZSCORE", "z", "m"],
            &["ZREM", "z", "m"],
            &["ZCARD", "z"],
            &["ZRANK", "z", "m"],
            &["ZINCRBY", "z", "1", "m"],
            &["ZRANGE", "z", "0", "-1"],
            &["ZREVRANGE", "z", "0", "-1"],
            &["ZCOUNT", "z", "0", "10"],
            &["JSET", "j", "$", "1"],
            &["JGET", "j"],
            &["JMERGE", "j", "{}"],
            &["RLSET", "k", "10", "60"],
            &["RLCHECK", "k"],
            &["SUBSCRIBE", "ch"],
            &["UNSUBSCRIBE", "ch"],
            &["PUBLISH", "ch", "msg"],
            &["QSUB", "p:*"],
            &["AUTH", "pw"],
            &["PING"],
            &["DBSIZE"],
            &["FLUSHDB"],
        ];

        for parts in valid {
            assert!(parse(parts).is_ok(), "{parts:?} should parse but errored");
        }
    }

    #[test]
    fn unsubscribe_with_no_arguments_means_all_channels() {
        // Documented behaviour: a bare UNSUBSCRIBE drops every subscription, so
        // it must parse rather than fail an arity check.
        assert_eq!(
            parse(&["UNSUBSCRIBE"]).unwrap(),
            Command::Unsubscribe(vec![])
        );
        assert_eq!(
            parse(&["UNSUBSCRIBE", "ch"]).unwrap(),
            Command::Unsubscribe(vec!["ch".into()])
        );
        // SUBSCRIBE, by contrast, needs at least one channel.
        assert!(parse(&["SUBSCRIBE"]).is_err());
    }

    #[test]
    fn unsupported_commands_fall_through_to_unknown() {
        // INCRBYFLOAT (the string variant) is deliberately not implemented —
        // only HINCRBYFLOAT is. Unsupported verbs parse as `Unknown` so the
        // server can answer with a proper error naming the command rather than
        // dropping the connection.
        assert_eq!(
            parse(&["INCRBYFLOAT", "k", "1.5"]).unwrap(),
            Command::Unknown("INCRBYFLOAT".into())
        );
        assert!(matches!(
            parse(&["HINCRBYFLOAT", "h", "f", "1.5"]).unwrap(),
            Command::HIncrByFloat(..)
        ));
    }

    // ── REPLICAOF ─────────────────────────────────────────────────────────────

    #[test]
    fn replicaof_accepts_only_no_one() {
        assert_eq!(
            parse(&["REPLICAOF", "NO", "ONE"]).unwrap(),
            Command::ReplicaOfNoOne
        );
        // Case-insensitive, like every other keyword.
        assert_eq!(
            parse(&["REPLICAOF", "no", "one"]).unwrap(),
            Command::ReplicaOfNoOne
        );
    }

    #[test]
    fn replicaof_rejects_repointing_at_runtime() {
        // Re-pointing a live server is deliberately unsupported — it requires a
        // restart with a new RECACHED_REPLICAOF. The error must say so.
        let err = parse(&["REPLICAOF", "127.0.0.1", "6379"]).unwrap_err();
        assert!(err.contains("REPLICAOF NO ONE"), "got {err}");
    }

    // ── DEDUP ─────────────────────────────────────────────────────────────────
    // The duplicate-suppression envelope. A malformed client id or id must be refused
    // rather than silently treated as a fresh write.

    #[test]
    fn dedup_validates_the_client_id() {
        assert!(
            parse(&["DEDUP", "", "1", "GET"]).is_err(),
            "empty client id"
        );
        let long = "x".repeat(65);
        assert!(
            parse(&["DEDUP", &long, "1", "GET"]).is_err(),
            "client id over 64 chars"
        );
        assert!(parse(&["DEDUP", "client", "1", "PING"]).is_ok());
    }

    #[test]
    fn dedup_rejects_a_non_numeric_id() {
        assert!(parse(&["DEDUP", "client", "notanumber", "PING"]).is_err());
    }

    // ── SYNC ──────────────────────────────────────────────────────────────────

    #[test]
    fn sync_accepts_zero_or_more_patterns() {
        // Bare SYNC clears scopes; with patterns it sets them.
        assert_eq!(parse(&["SYNC"]).unwrap(), Command::Sync(vec![]));
        assert_eq!(
            parse(&["SYNC", "cart:*", "user:1:*"]).unwrap(),
            Command::Sync(vec!["cart:*".into(), "user:1:*".into()])
        );
    }

    // ── Numeric argument validation ───────────────────────────────────────────

    #[test]
    fn integer_arguments_reject_non_numeric_input() {
        for parts in [
            vec!["INCRBY", "k", "abc"],
            vec!["DECRBY", "k", "abc"],
            vec!["EXPIRE", "k", "abc"],
            vec!["LRANGE", "l", "0", "abc"],
            vec!["LINDEX", "l", "abc"],
            vec!["SETEX", "k", "abc", "v"],
        ] {
            assert!(
                parse(&parts).is_err(),
                "{parts:?} should reject a non-numeric argument"
            );
        }
    }

    #[test]
    fn setex_and_psetex_reject_non_positive_ttls() {
        for cmd in ["SETEX", "PSETEX"] {
            for ttl in ["0", "-5"] {
                let err = parse(&[cmd, "k", ttl, "v"]).unwrap_err();
                assert!(
                    err.contains("invalid expire time"),
                    "{cmd} {ttl}: got {err}"
                );
            }
        }
    }

    #[test]
    fn rate_limiter_config_rejects_zero_limit_or_window() {
        let err = parse(&["RLSET", "k", "0", "60"]).unwrap_err();
        assert!(err.contains("limit must be >= 1"), "got {err}");
        let err = parse(&["RLSET", "k", "10", "0"]).unwrap_err();
        assert!(err.contains("window must be >= 1"), "got {err}");
    }

    // ── Frame-level validation ────────────────────────────────────────────────

    #[test]
    fn non_array_frames_are_rejected() {
        // Commands arrive as RESP arrays; anything else is a protocol error.
        for v in [
            Value::SimpleString("PING".into()),
            Value::Integer(1),
            Value::BulkString(Some(b"PING".to_vec())),
        ] {
            assert!(Command::from_value(v).is_err());
        }
    }

    #[test]
    fn empty_command_array_is_rejected() {
        let err = Command::from_value(Value::Array(Some(vec![]))).unwrap_err();
        assert!(err.contains("Empty command"), "got {err}");
    }

    #[test]
    fn command_name_may_arrive_as_a_simple_string() {
        // Some clients send the verb as a simple string rather than a bulk one.
        let v = Value::Array(Some(vec![Value::SimpleString("PING".into())]));
        assert_eq!(Command::from_value(v).unwrap(), Command::Ping(None));
    }

    #[test]
    fn non_string_command_name_is_rejected() {
        let v = Value::Array(Some(vec![Value::Integer(42)]));
        assert!(Command::from_value(v).is_err());
    }

    #[test]
    fn command_names_are_case_insensitive() {
        for name in ["get", "Get", "GET", "gEt"] {
            assert!(
                matches!(parse(&[name, "k"]).unwrap(), Command::Get(_)),
                "{name} should parse as GET"
            );
        }
    }

    #[test]
    fn empty_keys_are_rejected() {
        // An empty key is never valid — it would be indistinguishable from a
        // missing argument once stored.
        let err = parse(&["GET", ""]).unwrap_err();
        assert!(err.contains("key cannot be empty"), "got {err}");
    }
}

#[cfg(test)]
mod zero_copy_parse_tests {
    use super::*;

    fn bulk(s: &str) -> Value {
        Value::BulkString(Some(s.as_bytes().to_vec()))
    }

    fn parse(parts: &[&str]) -> Result<Command, String> {
        Command::from_value(Value::Array(Some(parts.iter().map(|s| bulk(s)).collect())))
    }

    /// Arguments are now *moved* out of the parsed frame rather than copied, so
    /// the failure mode is an arm reading the same index twice and getting an
    /// empty string the second time. These assert full round-trips through the
    /// converted commands.

    #[test]
    fn set_preserves_key_value_and_options_together() {
        // SET reads index 1 and 2 by move, then scans 3.. for options — the
        // exact shape that breaks if a moved slot were re-read.
        match parse(&["SET", "k", "v", "EX", "60", "NX"]).unwrap() {
            Command::Set(key, val, opts) => {
                assert_eq!(key, "k");
                assert_eq!(val, b"v");
                assert_eq!(opts.expiry, Some(SetExpiry::Ex(60)));
                assert_eq!(opts.condition, Some(SetCondition::Nx));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_large_value_survives_the_move_intact() {
        // The whole point of moving: a 1 MB payload is no longer memcpy'd.
        let big = "x".repeat(1024 * 1024);
        match parse(&["SET", "k", &big]).unwrap() {
            Command::Set(_, val, _) => {
                assert_eq!(val.len(), big.len());
                assert_eq!(
                    val,
                    big.as_bytes(),
                    "payload must be byte-identical after the move"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_binary_value_reaches_the_command_untouched() {
        // The move-based parse must not reintroduce a UTF-8 conversion on the
        // value path: the bytes have to arrive exactly as they were sent.
        let raw = vec![0xff, 0xfe, b'o', b'k'];
        let frame = Value::Array(Some(vec![
            bulk("SET"),
            bulk("k"),
            Value::BulkString(Some(raw.clone())),
        ]));
        match Command::from_value(frame).expect("a binary value is valid") {
            Command::Set(key, val, _) => {
                assert_eq!(key, "k");
                assert_eq!(val, raw);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_binary_key_is_still_refused() {
        // Keys are matched and routed as text, so this position stays strict
        // even though the value beside it does not.
        let frame = Value::Array(Some(vec![
            bulk("SET"),
            Value::BulkString(Some(vec![0xff, 0xfe])),
            bulk("v"),
        ]));
        let err = Command::from_value(frame).expect_err("binary key must be refused");
        assert!(err.contains("must be text"), "got {err:?}");
    }

    #[test]
    fn utf8_multibyte_arguments_are_not_mistaken_for_binary() {
        // The validation runs over every argument, so an over-eager check would
        // break ordinary non-ASCII payloads.
        match parse(&["SET", "ключ", "日本語 ✓"]).unwrap() {
            Command::Set(key, val, _) => {
                assert_eq!(key, "ключ");
                assert_eq!(val, "日本語 ✓".as_bytes());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn paired_arguments_keep_their_pairing() {
        // MSET and HSET split each chunk into two mutable halves; a mistake
        // there would swap or blank alternate fields.
        match parse(&["MSET", "a", "1", "b", "2"]).unwrap() {
            Command::MSet(pairs) => assert_eq!(
                pairs,
                vec![
                    ("a".to_string(), b"1".to_vec()),
                    ("b".to_string(), b"2".to_vec())
                ]
            ),
            other => panic!("{other:?}"),
        }
        match parse(&["HSET", "h", "f1", "v1", "f2", "v2"]).unwrap() {
            Command::HSet(key, pairs) => {
                assert_eq!(key, "h");
                assert_eq!(
                    pairs,
                    vec![
                        ("f1".to_string(), b"v1".to_vec()),
                        ("f2".to_string(), b"v2".to_vec())
                    ]
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bulk_member_lists_keep_every_element_in_order() {
        match parse(&["RPUSH", "l", "a", "b", "c"]).unwrap() {
            Command::RPush(key, vals) => {
                assert_eq!(key, "l");
                assert_eq!(vals, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
            }
            other => panic!("{other:?}"),
        }
        match parse(&["SADD", "s", "m1", "m2"]).unwrap() {
            Command::SAdd(key, members) => {
                assert_eq!(key, "s");
                assert_eq!(members, vec!["m1", "m2"]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn key_validation_still_applies_to_moved_keys() {
        // take_key must validate exactly as extract_key did.
        assert!(parse(&["SET", "", "v"]).is_err(), "empty key rejected");
        assert!(parse(&["APPEND", "", "v"]).is_err());
        assert!(parse(&["MSET", "", "1"]).is_err());
    }

    #[test]
    fn json_commands_carry_path_and_document_separately() {
        match parse(&["JSET", "doc", "$.a", "{\"x\":1}"]).unwrap() {
            Command::JSet(key, path, val) => {
                assert_eq!(key, "doc");
                assert_eq!(path, "$.a");
                assert_eq!(val, "{\"x\":1}");
            }
            other => panic!("{other:?}"),
        }
    }
}
