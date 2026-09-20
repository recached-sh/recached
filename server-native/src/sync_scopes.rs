//! Sync scopes: signed tokens restricting a WebSocket connection to a set of
//! key patterns, and the per-command classification they are checked against.

use crate::*;

/// One mutation pushed towards WebSocket peers: the RESP push frame plus the
/// keys it touches, so each connection can filter against its sync scopes
/// without re-parsing the frame. Wrapped in `Arc` — the broadcast channel
/// clones the payload once per receiver, so a clone is a refcount bump.
pub(crate) struct SyncPush {
    pub(crate) origin: u64,
    pub(crate) keys: Vec<String>,
    pub(crate) resp: Vec<u8>,
}

pub(crate) type SyncMsg = Arc<SyncPush>;

/// What a grant confers on a key, and what a command requires of one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Access {
    /// Reading the key, and receiving its mutations on the sync fan-out.
    Read,
    /// Mutating the key. Implies `Read`, which is why a read-modify-write
    /// command such as `INCR` or `GETSET` requires only `Write`: no grant
    /// confers write without read, so there is nothing extra to check.
    Write,
}

/// One granted pattern and the access it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Grant {
    pub(crate) pattern: String,
    pub(crate) access: Access,
}

impl Grant {
    /// Whether this grant satisfies a requirement of `need`.
    fn permits(&self, need: Access) -> bool {
        matches!(
            (self.access, need),
            (Access::Write, _) | (Access::Read, Access::Read)
        )
    }
}

#[cfg(test)]
impl Grant {
    /// A read-write grant on `pattern`.
    pub(crate) fn rw(pattern: &str) -> Self {
        Grant {
            pattern: pattern.to_string(),
            access: Access::Write,
        }
    }
    /// A read-only grant on `pattern`.
    pub(crate) fn ro(pattern: &str) -> Self {
        Grant {
            pattern: pattern.to_string(),
            access: Access::Read,
        }
    }
}

/// Parse one scope entry into a grant.
///
/// `rw=cart:42:*` and `r=catalog:*` state the access; a bare pattern is
/// read-write, which is what every scope minted before write scoping existed
/// meant, so old tokens keep working unchanged.
///
/// `r=` and `rw=` are therefore reserved entry prefixes: a pattern whose own
/// text begins with one cannot be expressed. The failure is closed — such an
/// entry grants less than its author wrote, on a different prefix, rather than
/// more — and key patterns do not look like that in practice.
fn parse_grant(entry: &str) -> Grant {
    let (access, pattern) = match entry.split_once('=') {
        Some(("r", rest)) => (Access::Read, rest),
        Some(("rw", rest)) => (Access::Write, rest),
        _ => (Access::Write, entry),
    };
    Grant {
        pattern: pattern.to_string(),
        access,
    }
}

/// True when a mutation touching `keys` is visible to a connection holding
/// `grants`. A mutation with no keys (FLUSHDB) affects every scope.
///
/// Visibility is read access, and every grant confers read, so this does not
/// inspect `access`. Were a write-only grant ever added, the fan-out filter
/// would have to start distinguishing them here.
pub(crate) fn scopes_match(grants: &[Grant], keys: &[String]) -> bool {
    keys.is_empty()
        || keys.iter().any(|k| {
            grants
                .iter()
                .any(|g| core_engine::store::glob_match(&g.pattern, k))
        })
}

/// Count one refused command on a scoped connection.
///
/// A scoped connection that is misconfigured fails silently from the server's
/// side — the page simply stops working — so the refusals are the only signal
/// an operator gets. The `reason` separates the four that mean different
/// things: `read_only` is a grant that is too narrow, `out_of_scope` a grant
/// that is missing, `no_token` a client that never authenticated its scopes,
/// and `admin` a keyspace-wide command that is refused by design and is
/// expected to be non-zero in normal operation.
pub(crate) fn record_scope_denial(reason: &'static str) {
    counter!("recached_scope_denials_total", "reason" => reason).increment(1);
}

/// True when `grants` permit `need` on `key`.
pub(crate) fn scopes_allow(grants: &[Grant], key: &str, need: Access) -> bool {
    grants
        .iter()
        .any(|g| g.permits(need) && core_engine::store::glob_match(&g.pattern, key))
}

/// Conservatively prove that every key matched by `requested` is also covered
/// by `grant`. General glob containment is easy to get subtly wrong. Recached's
/// documented scope form is a literal namespace prefix followed by `*`, so we
/// accept that form (and exact equality) and reject ambiguous wildcard grants.
pub(crate) fn scope_covers_pattern(grant: &str, requested: &str) -> bool {
    if grant == requested || grant == "*" {
        return true;
    }
    let Some(prefix) = grant.strip_suffix('*') else {
        return false;
    };
    if prefix.contains(['*', '?']) {
        return false;
    }
    let requested_prefix = requested
        .find(['*', '?'])
        .map_or(requested, |index| &requested[..index]);
    requested_prefix.starts_with(prefix)
}

/// Verify a signed sync-scope token and return the grants it carries.
///
/// Token format: `base64url(payload) "." base64url(hmac_sha256(secret, base64url(payload)))`
/// where payload is comma-separated scope entries with an optional
/// `|<unix_expiry_secs>` suffix. Each entry is a glob pattern, optionally
/// prefixed `r=` (read-only) or `rw=`; bare entries are read-write. The HMAC is
/// computed over the *encoded* payload string, so minting in JS is one
/// `createHmac` call on the base64url text — no byte-level canonicalisation
/// questions.
pub(crate) fn verify_sync_token(secret: &str, token: &str) -> Result<Vec<Grant>, &'static str> {
    use base64::Engine as _;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let (payload_b64, sig_b64) = token.split_once('.').ok_or("malformed token")?;
    let sig = engine.decode(sig_b64).map_err(|_| "malformed signature")?;
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload_b64.as_bytes());
    if !ct_eq_bytes(&sig, &mac.finalize().into_bytes()) {
        return Err("invalid signature");
    }
    let payload_bytes = engine
        .decode(payload_b64)
        .map_err(|_| "malformed payload")?;
    let payload = String::from_utf8(payload_bytes).map_err(|_| "malformed payload")?;
    let (patterns_str, expiry) = match payload.split_once('|') {
        Some((p, e)) => (p, Some(e)),
        None => (payload.as_str(), None),
    };
    if let Some(e) = expiry {
        let exp: u64 = e.parse().map_err(|_| "malformed expiry")?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now >= exp {
            return Err("token expired");
        }
    }
    let grants: Vec<Grant> = patterns_str
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(parse_grant)
        .collect();
    if grants.is_empty() {
        return Err("token grants no patterns");
    }
    // A bare `r=` or `rw=` leaves an empty pattern, which matches only the
    // empty key. That is always a minting bug, and one that would otherwise
    // fail silently as a token granting nothing.
    if grants.iter().any(|g| g.pattern.is_empty()) {
        return Err("token grants an empty pattern");
    }
    // Token patterns reach `glob_match` without passing through the command
    // parser, so the cap `check_pattern` applies to `KEYS`/`SCAN`/`PSUBSCRIBE`
    // has to be repeated here. These are matched once per key per write, so an
    // over-long one is the most expensive place to put a pattern — and a
    // compromised or careless minting service should not be able to.
    if grants
        .iter()
        .any(|g| g.pattern.len() > core_engine::store::MAX_PATTERN_BYTES)
    {
        return Err("token grants an over-long pattern");
    }
    Ok(grants)
}

/// What a command touches, for scope enforcement on token-scoped WebSocket
/// connections.
#[derive(Debug)]
pub(crate) enum CommandScope {
    /// No key access (PING, AUTH, MULTI, SYNC, pub/sub) — always allowed.
    KeyLess,
    /// Touches exactly these keys, each with the access it requires. Every
    /// one must be permitted by a grant at that access.
    Keys(Vec<(String, Access)>),
    /// Keyspace-wide or administrative — denied on scoped connections.
    Admin,
}

pub(crate) fn command_scope(cmd: &Command) -> CommandScope {
    match cmd {
        Command::Ping(_)
        | Command::Auth(_)
        | Command::Hello(_)
        | Command::Multi
        | Command::Exec
        | Command::Discard
        | Command::Subscribe(_)
        | Command::Unsubscribe(_)
        | Command::PSubscribe(_)
        | Command::PUnsubscribe(_)
        | Command::Publish(_, _)
        | Command::Sync(_)
        // QSUB patterns are scope-checked against the grant in the WS handler.
        | Command::QSub(_)
        | Command::QUnsub(_)
        // QUIT and CLIENT describe the connection itself, which a scoped
        // connection is entitled to know about; COMMAND describes the server's
        // vocabulary, which is public.
        | Command::Quit
        | Command::Client(_)
        | Command::CommandQuery(_)
        // CLUSTER and MODULE answer the same sentence to everyone — "not a
        // cluster", "no modules" — and describe no state a scope could protect.
        | Command::Cluster(_)
        | Command::Module(_)
        // Every MEMORY subcommand other than USAGE is refused outright, so
        // there is nothing here to scope either. USAGE reads a key and is
        // classified with the key commands below.
        | Command::Memory(_)
        | Command::Unknown(_) => CommandScope::KeyLess,

        Command::Keys(_)
        | Command::Scan(_, _, _)
        | Command::DbSize
        | Command::FlushDb
        | Command::Save
        | Command::BgSave
        | Command::LastSave
        // INFO reports server-wide state — uptime, client counts, keyspace
        // size, replication topology. A connection scoped to a handful of keys
        // has no business reading it.
        | Command::Info(_)
        // CONFIG reports server-wide limits and whether auth is on. Same
        // reasoning as INFO: not for a connection scoped to a few keys.
        | Command::Config(_)
        // PUBSUB enumerates every channel every other client is subscribed to.
        // A scoped connection can already SUBSCRIBE to any channel it can name
        // — channels are outside the scope system entirely — but naming and
        // listing are different powers, the same way GET is scoped and KEYS is
        // Admin. NUMSUB and NUMPAT ride along rather than splitting the family
        // across two scopes for one subcommand's worth of difference.
        | Command::PubSub(_)
        | Command::ReplicaOfNoOne => CommandScope::Admin,

        // ── Single-key reads ────────────────────────────────────────────
        Command::Get(k)
        | Command::Strlen(k)
        | Command::GetRange(k, _, _)
        | Command::Ttl(k)
        | Command::PTtl(k)
        | Command::Type(k)
        | Command::MemoryUsage(k)
        | Command::HGet(k, _)
        | Command::HGetAll(k)
        | Command::HKeys(k)
        | Command::HVals(k)
        | Command::HLen(k)
        | Command::HExists(k, _)
        | Command::HMGet(k, _)
        | Command::HScan(k, _)
        | Command::SScan(k, _)
        | Command::ZScan(k, _)
        | Command::LRange(k, _, _)
        | Command::LLen(k)
        | Command::LIndex(k, _)
        | Command::SMembers(k)
        | Command::SCard(k)
        | Command::SIsMember(k, _)
        | Command::SMIsMember(k, _)
        // SRANDMEMBER reads; SPOP removes and is a write, below.
        | Command::SRandMember(k, _)
        | Command::ZRange(k, _, _, _)
        | Command::ZRevRange(k, _, _, _)
        | Command::ZRangeByScore(k, _, _, _, _)
        | Command::ZRevRangeByScore(k, _, _, _, _)
        | Command::ZScore(k, _)
        | Command::ZMScore(k, _)
        | Command::ZRank(k, _)
        | Command::ZRevRank(k, _)
        | Command::ZCard(k)
        | Command::ZCount(k, _, _)
        | Command::JGet(k, _) => CommandScope::Keys(vec![(k.clone(), Access::Read)]),

        // ── Single-key writes ───────────────────────────────────────────
        // Read-modify-write commands (INCR, APPEND, GETSET, RLCHECK) are
        // classified here and require only Write: every grant that confers
        // write confers read too.
        Command::ESet(k, _)
        | Command::Set(k, _, _)
        | Command::Append(k, _)
        | Command::GetSet(k, _)
        | Command::SetNx(k, _)
        | Command::SetEx(k, _, _)
        | Command::PSetEx(k, _, _)
        | Command::Incr(k)
        | Command::Decr(k)
        | Command::IncrBy(k, _)
        | Command::DecrBy(k, _)
        | Command::Expire(k, _)
        | Command::PExpire(k, _)
        | Command::ExpireAt(k, _)
        | Command::PExpireAt(k, _)
        | Command::Persist(k)
        | Command::HSet(k, _)
        | Command::HDel(k, _)
        | Command::HIncrBy(k, _, _)
        | Command::HIncrByFloat(k, _, _)
        | Command::HSetNx(k, _, _)
        | Command::LPush(k, _)
        | Command::RPush(k, _)
        | Command::LPushX(k, _)
        | Command::RPushX(k, _)
        | Command::LPop(k, _)
        | Command::RPop(k, _)
        | Command::LSet(k, _, _)
        | Command::LRem(k, _, _)
        | Command::LTrim(k, _, _)
        | Command::SAdd(k, _)
        | Command::SRem(k, _)
        | Command::SPop(k, _)
        | Command::ZAdd(k, _, _)
        | Command::ZRem(k, _)
        | Command::ZIncrBy(k, _, _)
        // RLCHECK records the attempt it reports on — a write, not a probe.
        | Command::RlSet(k, _, _)
        | Command::RlCheck(k, _)
        | Command::JSet(k, _, _)
        | Command::JMerge(k, _) => CommandScope::Keys(vec![(k.clone(), Access::Write)]),

        // ── Multi-key reads ─────────────────────────────────────────────
        // WATCH and UNWATCH observe a key's changes, which is read access.
        Command::MGet(keys)
        | Command::Exists(keys)
        | Command::SInter(keys)
        | Command::SUnion(keys)
        | Command::SDiff(keys)
        | Command::Watch(keys)
        | Command::Unwatch(keys) => {
            CommandScope::Keys(keys.iter().map(|k| (k.clone(), Access::Read)).collect())
        }

        // ── Multi-key writes ────────────────────────────────────────────
        Command::Del(keys) | Command::Unlink(keys) => {
            CommandScope::Keys(keys.iter().map(|k| (k.clone(), Access::Write)).collect())
        }
        Command::MSet(pairs) => {
            CommandScope::Keys(pairs.iter().map(|(k, _)| (k.clone(), Access::Write)).collect())
        }
        // RENAME destroys the source; SMOVE removes the member from it. Both
        // ends are writes.
        Command::Rename(src, dst) | Command::SMove(src, dst, _) => CommandScope::Keys(vec![
            (src.clone(), Access::Write),
            (dst.clone(), Access::Write),
        ]),
        // The store family reads its sources and writes only the destination,
        // which is the case that makes access per key rather than per command:
        // `SINTERSTORE mine r=theirs` must not demand write on `theirs`.
        Command::SInterStore(dst, keys)
        | Command::SUnionStore(dst, keys)
        | Command::SDiffStore(dst, keys) => {
            let mut all: Vec<(String, Access)> =
                keys.iter().map(|k| (k.clone(), Access::Read)).collect();
            all.push((dst.clone(), Access::Write));
            CommandScope::Keys(all)
        }

        // Scope enforcement applies to the wrapped command.
        Command::Dedup(_, _, inner) => command_scope(inner),
    }
}

/// Handle the SYNC command for one WebSocket connection, returning the RESP
/// reply. Forms:
///   `SYNC`                 — list this connection's current scopes
///   `SYNC TOKEN <token>`   — set scopes from a signed token (requires
///                            `RECACHED_SYNC_SECRET` on the server)
///   `SYNC <pattern> [...]` — set scopes directly (only allowed when no
///                            secret is configured — a bandwidth filter, not
///                            an authorization boundary)
pub(crate) fn handle_sync_command(
    args: &[String],
    secret: Option<&str>,
    scopes: &mut Option<Vec<Grant>>,
    conn_id: u64,
) -> Vec<u8> {
    /// Grants are echoed in the same notation they are written in, so a
    /// client can tell a read-only grant from a read-write one.
    fn patterns_reply(grants: &[Grant]) -> Vec<u8> {
        Value::Array(Some(
            grants
                .iter()
                .map(|g| {
                    let text = match g.access {
                        Access::Read => format!("r={}", g.pattern),
                        Access::Write => format!("rw={}", g.pattern),
                    };
                    Value::BulkString(Some(text.into_bytes()))
                })
                .collect(),
        ))
        .serialize()
    }
    match args {
        [] => patterns_reply(scopes.as_deref().unwrap_or(&[])),
        [kw, token] if kw.eq_ignore_ascii_case("token") => {
            let Some(secret) = secret else {
                return b"-ERR SYNC TOKEN requires RECACHED_SYNC_SECRET to be configured on the server\r\n"
                    .to_vec();
            };
            match verify_sync_token(secret, token) {
                Ok(patterns) => {
                    info!("WS conn {} scoped via token: {:?}", conn_id, patterns);
                    let reply = patterns_reply(&patterns);
                    *scopes = Some(patterns);
                    reply
                }
                Err(e) => Value::Error(format!("ERR invalid sync token: {}", e)).serialize(),
            }
        }
        patterns => {
            if secret.is_some() {
                return b"-ERR this server requires signed scopes: use SYNC TOKEN <token>\r\n"
                    .to_vec();
            }
            let pats: Vec<Grant> = patterns
                .iter()
                .filter(|p| !p.is_empty())
                .map(|p| parse_grant(p))
                .filter(|g| !g.pattern.is_empty())
                .collect();
            if pats.is_empty() {
                return b"-ERR SYNC requires at least one pattern\r\n".to_vec();
            }
            info!("WS conn {} sync scopes set: {:?}", conn_id, pats);
            let reply = patterns_reply(&pats);
            *scopes = Some(pats);
            reply
        }
    }
}

#[cfg(test)]
mod scope_containment_tests {
    use super::scope_covers_pattern;

    #[test]
    fn prefix_grants_cover_only_narrower_patterns() {
        assert!(scope_covers_pattern("cart:*", "cart:42:*"));
        assert!(scope_covers_pattern("cart:*", "cart:42:item:?"));
        assert!(scope_covers_pattern("*", "anything:*"));
        assert!(!scope_covers_pattern("cart:42:*", "cart:*"));
        assert!(!scope_covers_pattern("cart:*", "user:*"));
    }

    #[test]
    fn ambiguous_glob_grants_require_exact_equality() {
        assert!(scope_covers_pattern("tenant:?", "tenant:?"));
        assert!(!scope_covers_pattern("tenant:?", "tenant:*"));
        assert!(!scope_covers_pattern("tenant:*:item:*", "tenant:1:item:*"));
    }
}

// ── connection identity ──────────────────────────────────────────────────────
