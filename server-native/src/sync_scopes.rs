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
    /// The mutation affects the whole keyspace (`FLUSHDB`), so it is visible
    /// to every scope even though it names no key.
    ///
    /// An empty `keys` used to mean exactly that, which made the filter fail
    /// *open*: a write that `primary_keys` forgot to list would have been
    /// pushed, value and all, to every tenant. An empty key list is now only
    /// visible to a scoped connection when this says so.
    pub(crate) keyspace_wide: bool,
}

impl SyncPush {
    /// A push for `cmd`, which wrote `keys`.
    pub(crate) fn for_command(
        origin: u64,
        cmd: &Command,
        keys: Vec<String>,
        resp: Vec<u8>,
    ) -> Self {
        let keyspace_wide = matches!(cmd, Command::FlushDb)
            || matches!(cmd, Command::Dedup(_, _, inner) if matches!(**inner, Command::FlushDb));
        SyncPush {
            origin,
            keys,
            resp,
            keyspace_wide,
        }
    }
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
    match parse_entry(entry) {
        Entry::Key(grant) => grant,
        // A literal `SYNC` pattern list has no channel concept; keep the old
        // reading of the whole entry as a key pattern.
        Entry::Channel(_) => Grant {
            pattern: entry.to_string(),
            access: Access::Write,
        },
    }
}

/// One scope entry: a key grant, or (since 0.6) a pub/sub channel grant.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    Key(Grant),
    Channel(Grant),
}

/// `r=` / `rw=` grant keys; `chr=` / `chrw=` grant pub/sub channels — `chr`
/// to subscribe, `chrw` to subscribe and publish. A bare entry is a
/// read-write key grant.
///
/// A server before 0.6 reads `chr=news:*` as a bare key pattern beginning
/// `chr=`, which matches no key an application writes: it fails closed.
fn parse_entry(entry: &str) -> Entry {
    let grant = |access, pattern: &str| Grant {
        pattern: pattern.to_string(),
        access,
    };
    match entry.split_once('=') {
        Some(("r", rest)) => Entry::Key(grant(Access::Read, rest)),
        Some(("rw", rest)) => Entry::Key(grant(Access::Write, rest)),
        Some(("chr", rest)) => Entry::Channel(grant(Access::Read, rest)),
        Some(("chrw", rest)) => Entry::Channel(grant(Access::Write, rest)),
        _ => Entry::Key(grant(Access::Write, entry)),
    }
}

/// How pub/sub is authorized on strict (token-scoped) WebSocket connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PubSubPolicy {
    /// Channels need a `chr=`/`chrw=` grant. The default since 0.6.
    Scoped,
    /// Any connection may subscribe to or publish on any channel — the
    /// behaviour before 0.6, kept behind `RECACHED_SYNC_PUBSUB=open`.
    Open,
}

/// Server-wide sync authorization settings, fixed at startup.
#[derive(Debug, Clone)]
pub(crate) struct SyncPolicy {
    /// `RECACHED_SYNC_SECRET`. `Some` turns on strict mode.
    pub(crate) secret: Option<String>,
    pub(crate) pubsub: PubSubPolicy,
    /// A valid `SYNC TOKEN` authenticates a WebSocket connection, so the RESP
    /// password never has to be shipped to a browser
    /// (`RECACHED_WS_AUTH=token`). Off by default outside the multitenant
    /// profile, where the password still gates the socket.
    pub(crate) token_auth: bool,
    /// Tokens must carry an expiry.
    pub(crate) require_expiry: bool,
    /// Tokens must carry a `sub` claim, which namespaces DEDUP state.
    pub(crate) require_subject: bool,
    /// Longest lifetime a token may claim, counted from presentation.
    pub(crate) max_ttl_secs: Option<u64>,
    /// When set, tokens must carry a matching `aud` claim.
    pub(crate) audience: Option<String>,
}

impl SyncPolicy {
    /// Open mode: no secret, nothing enforced. What the server did by default
    /// before 0.6, and still does.
    #[cfg(test)]
    pub(crate) fn open() -> Self {
        SyncPolicy {
            secret: None,
            pubsub: PubSubPolicy::Scoped,
            token_auth: false,
            require_expiry: false,
            require_subject: false,
            max_ttl_secs: None,
            audience: None,
        }
    }

    /// Strict mode with the 0.6 defaults and nothing else required.
    #[cfg(test)]
    pub(crate) fn strict(secret: &str) -> Self {
        SyncPolicy {
            secret: Some(secret.to_string()),
            ..SyncPolicy::open()
        }
    }

    pub(crate) fn is_strict(&self) -> bool {
        self.secret.is_some()
    }

    /// Whether channel commands are checked against channel grants.
    pub(crate) fn scopes_pubsub(&self) -> bool {
        self.is_strict() && self.pubsub == PubSubPolicy::Scoped
    }
}

/// What a verified token grants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedToken {
    pub(crate) grants: Vec<Grant>,
    pub(crate) channels: Vec<Grant>,
    /// Unix seconds after which the token, and every connection scoped by it,
    /// stops being valid.
    pub(crate) expires_at: Option<u64>,
    pub(crate) subject: Option<String>,
}

/// Longest `sub` / `aud` claim value.
const MAX_CLAIM_BYTES: usize = 128;

/// Claim values are identifiers, not free text: anything that could be
/// confused for a separator, or carry a newline into a log line, is refused.
fn valid_claim_value(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_CLAIM_BYTES
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:@/+-".contains(&b))
}

/// Parse the optional claims segment, `sub=<id>;aud=<id>`. Unknown and
/// repeated claims are refused rather than ignored: a minting service that
/// believes it set a restriction this server does not know must find out.
fn parse_claims(raw: &str) -> Result<(Option<String>, Option<String>), &'static str> {
    let mut subject = None;
    let mut audience = None;
    for part in raw.split(';').filter(|p| !p.is_empty()) {
        let (name, value) = part.split_once('=').ok_or("malformed claim")?;
        if !valid_claim_value(value) {
            return Err("malformed claim value");
        }
        let slot = match name {
            "sub" => &mut subject,
            "aud" => &mut audience,
            _ => return Err("unknown claim"),
        };
        if slot.replace(value.to_string()).is_some() {
            return Err("repeated claim");
        }
    }
    Ok((subject, audience))
}

pub(crate) fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn unix_now_millis() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

/// Whether `token` still grants everything a connection holds: every live
/// query pattern, every watched key and — when channels are scoped — every
/// channel and pattern subscription.
pub(crate) fn token_covers_held(
    token: &VerifiedToken,
    qsub_patterns: &HashSet<String>,
    watched_keys: &HashSet<String>,
    channels: &HashSet<String>,
    channel_patterns: &HashSet<String>,
    scopes_pubsub: bool,
) -> bool {
    let live = qsub_patterns.iter().all(|p| {
        token
            .grants
            .iter()
            .any(|g| scope_covers_pattern(&g.pattern, p))
    });
    let watched = watched_keys
        .iter()
        .all(|k| scopes_allow(&token.grants, k, Access::Read));
    let pubsub = !scopes_pubsub
        || (channels
            .iter()
            .all(|c| channel_allowed(&token.channels, c, Access::Read, false))
            && channel_patterns
                .iter()
                .all(|p| channel_allowed(&token.channels, p, Access::Read, true)));
    live && watched && pubsub
}

/// The refusal for a channel command on a strict connection with scoped
/// pub/sub, or `None` when it is allowed (or not a channel command).
pub(crate) fn channel_denial(
    cmd: &Command,
    has_token: bool,
    channels: &[Grant],
) -> Option<Vec<u8>> {
    let wanted = channel_scope(cmd)?;
    if !has_token {
        record_scope_denial("no_token");
        return Some(b"-NOSCOPE send SYNC TOKEN <token> before issuing commands\r\n".to_vec());
    }
    let (name, need, is_pattern) = wanted
        .into_iter()
        .find(|(name, need, is_pattern)| !channel_allowed(channels, name, *need, *is_pattern))?;
    record_scope_denial("channel");
    let what = if is_pattern { "pattern" } else { "channel" };
    let message = if need == Access::Write && channel_allowed(channels, name, Access::Read, false) {
        format!("NOSCOPE channel '{name}' is subscribe-only on this connection")
    } else {
        format!("NOSCOPE {what} '{name}' is outside this connection's sync scopes")
    };
    Some(Value::Error(message).serialize())
}

/// Whether a strict connection holding `grants` (`None` before a token) may
/// run `cmd` — the same decision the command loop makes, as one predicate, for
/// re-checking a queued transaction at `EXEC`.
pub(crate) fn command_permitted(
    cmd: &Command,
    grants: Option<&[Grant]>,
    channels: &[Grant],
    scopes_pubsub: bool,
) -> bool {
    match command_scope(cmd) {
        CommandScope::KeyLess => {
            !scopes_pubsub || channel_denial_silent(cmd, grants.is_some(), channels)
        }
        CommandScope::Admin => false,
        CommandScope::Keys(keys) => grants.is_some_and(|grants| {
            keys.iter()
                .all(|(key, need)| scopes_allow(grants, key, *need))
        }),
    }
}

/// [`channel_denial`]'s decision without recording a metric or building a
/// reply: true when allowed.
fn channel_denial_silent(cmd: &Command, has_token: bool, channels: &[Grant]) -> bool {
    match channel_scope(cmd) {
        None => true,
        Some(wanted) => {
            has_token
                && wanted.iter().all(|(name, need, is_pattern)| {
                    channel_allowed(channels, name, *need, *is_pattern)
                })
        }
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
        || keys.iter().all(|k| {
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
#[cfg(test)]
pub(crate) fn verify_sync_token(secret: &str, token: &str) -> Result<Vec<Grant>, &'static str> {
    verify_sync_token_at(&SyncPolicy::strict(secret), token, unix_now_secs()).map(|t| t.grants)
}

/// [`verify_sync_token`] against a full policy, at `now` (unix seconds).
///
/// Payload grammar, inside the signed base64url text:
///
/// ```text
/// entries [ "|" expiry [ "|" claims ] ]
/// entries = entry ("," entry)*        ; r=, rw=, chr=, chrw= or bare
/// expiry  = unix seconds, or empty    ; empty only when claims follow
/// claims  = claim (";" claim)*        ; sub=<id>  aud=<id>
/// ```
///
/// A server before 0.6 reads a three-segment payload's `expiry|claims` as one
/// malformed expiry and refuses the token, so the new form fails closed there.
pub(crate) fn verify_sync_token_at(
    policy: &SyncPolicy,
    token: &str,
    now: u64,
) -> Result<VerifiedToken, &'static str> {
    use base64::Engine as _;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let secret = policy
        .secret
        .as_deref()
        .ok_or("no sync secret is configured")?;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let (payload_b64, sig_b64) = token.split_once('.').ok_or("malformed token")?;
    let sig = engine.decode(sig_b64).map_err(|_| "malformed signature")?;
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return Err("invalid signature");
    };
    mac.update(payload_b64.as_bytes());
    if !ct_eq_bytes(&sig, &mac.finalize().into_bytes()) {
        return Err("invalid signature");
    }
    let payload_bytes = engine
        .decode(payload_b64)
        .map_err(|_| "malformed payload")?;
    let payload = String::from_utf8(payload_bytes).map_err(|_| "malformed payload")?;
    let mut segments = payload.splitn(3, '|');
    let patterns_str = segments.next().unwrap_or_default();
    let expiry = segments.next();
    let claims = segments.next();

    let expires_at = match expiry {
        None => None,
        Some("") if claims.is_some() => None,
        Some(e) => Some(e.parse::<u64>().map_err(|_| "malformed expiry")?),
    };
    if let Some(exp) = expires_at
        && now >= exp
    {
        return Err("token expired");
    }
    let (subject, audience) = match claims {
        Some(raw) => parse_claims(raw)?,
        None => (None, None),
    };

    if policy.require_expiry && expires_at.is_none() {
        return Err("token has no expiry, which this server requires");
    }
    if let (Some(max), Some(exp)) = (policy.max_ttl_secs, expires_at)
        && exp.saturating_sub(now) > max
    {
        return Err("token lifetime exceeds RECACHED_SYNC_MAX_TOKEN_TTL");
    }
    if policy.require_subject && subject.is_none() {
        return Err("token has no sub claim, which this server requires");
    }
    if let Some(required) = &policy.audience
        && audience.as_deref() != Some(required.as_str())
    {
        return Err("token audience does not match this server");
    }

    let mut grants = Vec::new();
    let mut channels = Vec::new();
    for entry in patterns_str
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        match parse_entry(entry) {
            Entry::Key(g) => grants.push(g),
            Entry::Channel(g) => channels.push(g),
        }
    }
    if grants.is_empty() && channels.is_empty() {
        return Err("token grants no patterns");
    }
    // A bare `r=` or `rw=` leaves an empty pattern, which matches only the
    // empty key. That is always a minting bug, and one that would otherwise
    // fail silently as a token granting nothing.
    if grants.iter().chain(&channels).any(|g| g.pattern.is_empty()) {
        return Err("token grants an empty pattern");
    }
    // Token patterns reach `glob_match` without passing through the command
    // parser, so the cap `check_pattern` applies to `KEYS`/`SCAN`/`PSUBSCRIBE`
    // has to be repeated here. These are matched once per key per write, so an
    // over-long one is the most expensive place to put a pattern — and a
    // compromised or careless minting service should not be able to.
    if grants
        .iter()
        .chain(&channels)
        .any(|g| g.pattern.len() > core_engine::store::MAX_PATTERN_BYTES)
    {
        return Err("token grants an over-long pattern");
    }
    Ok(VerifiedToken {
        grants,
        channels,
        expires_at,
        subject,
    })
}

/// The DEDUP client id a write is recorded under on a connection whose token
/// names a subject.
///
/// Client ids are chosen by clients, so without this one principal who learns
/// another's id could read that client's remembered refusals (which echo key
/// names) or raise its mark so its next writes are answered `+DUP` and dropped.
/// Hashing the subject in gives every principal its own id space; the same
/// subject maps to the same id across token renewals and reconnects, so a
/// write whose reply was lost still reads as applied under a new token.
///
/// The result begins with `~`, which client-supplied ids may not (see
/// [`reserved_dedup_client`]), so a raw id can never collide with one.
pub(crate) fn namespaced_dedup_client(subject: &str, client: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"recached-dedup-ns-v1\0");
    hasher.update(subject.as_bytes());
    hasher.update(b"\0");
    hasher.update(client.as_bytes());
    let digest = hasher.finalize();
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(41);
    out.push('~');
    for byte in digest.iter().take(20) {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

/// Client ids beginning with `~` are the server's namespaced form and may not
/// be sent by a client.
pub(crate) fn reserved_dedup_client(client: &str) -> bool {
    client.starts_with('~')
}

/// Channels a command touches, for pub/sub authorization on strict
/// connections: `(channel or pattern, access, is_pattern)`. `None` for every
/// command that is not a channel command; unsubscribing needs no grant.
pub(crate) fn channel_scope(cmd: &Command) -> Option<Vec<(&str, Access, bool)>> {
    match cmd {
        Command::Subscribe(channels) => Some(
            channels
                .iter()
                .map(|c| (c.as_str(), Access::Read, false))
                .collect(),
        ),
        Command::PSubscribe(patterns) => Some(
            patterns
                .iter()
                .map(|p| (p.as_str(), Access::Read, true))
                .collect(),
        ),
        Command::Publish(channel, _) => Some(vec![(channel.as_str(), Access::Write, false)]),
        _ => None,
    }
}

/// True when `channels` grants `need` on a channel, or — for a pattern
/// subscription — provably covers every channel the pattern can match.
pub(crate) fn channel_allowed(
    channels: &[Grant],
    name: &str,
    need: Access,
    is_pattern: bool,
) -> bool {
    channels.iter().any(|g| {
        g.permits(need)
            && if is_pattern {
                scope_covers_pattern(&g.pattern, name)
            } else {
                core_engine::store::glob_match(&g.pattern, name)
            }
    })
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
        // QUIT and most CLIENT subcommands describe the connection itself,
        // which a scoped connection is entitled to know about; COMMAND
        // describes the server's vocabulary, which is public.
        | Command::Quit
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

        // CLIENT LIST exposes every connected peer, so unlike CLIENT ID and
        // CLIENT SETNAME it is server-wide metadata.
        Command::Client(args)
            if args
                .first()
                .is_some_and(|arg| arg.eq_ignore_ascii_case("LIST")) =>
        {
            CommandScope::Admin
        }
        Command::Client(_) => CommandScope::KeyLess,

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
        | Command::JGet(k, _)
        | Command::ExpireTime(k)
        | Command::PExpireTime(k)
        | Command::HStrLen(k, _) => CommandScope::Keys(vec![(k.clone(), Access::Read)]),

        // ── Single-key writes ───────────────────────────────────────────
        // Read-modify-write commands (INCR, APPEND, GETSET, RLCHECK) are
        // classified here and require only Write: every grant that confers
        // write confers read too.
        Command::ESet(k, _)
        | Command::EAdd(k, _)
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
        | Command::JMerge(k, _)
        | Command::GetDel(k)
        // Write even without an option, as Redis's ACL categories have it
        // (`@write`): a read-only grant reads with GET.
        | Command::GetEx(k, _)
        | Command::ZPopMin(k, _)
        | Command::ZPopMax(k, _) => CommandScope::Keys(vec![(k.clone(), Access::Write)]),

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
        // COPY reads its source and writes its destination: `r=` on the
        // source is enough, as for the STORE family.
        Command::Copy(src, dst, _) => CommandScope::Keys(vec![
            (src.clone(), Access::Read),
            (dst.clone(), Access::Write),
        ]),
        Command::Rename(src, dst) | Command::RenameNx(src, dst) | Command::SMove(src, dst, _) => CommandScope::Keys(vec![
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

/// The outcome of one `SYNC` command.
pub(crate) enum SyncOutcome {
    /// Answered; the connection's scopes are unchanged.
    Unchanged(Vec<u8>),
    /// A token was presented and refused. Counted toward the connection's
    /// failure limit, like a wrong password.
    Refused(Vec<u8>),
    /// New scopes are in force from now on.
    Scoped {
        reply: Vec<u8>,
        token: VerifiedToken,
    },
}

/// Grants echoed in the same notation they are written in, so a client can
/// tell a read-only grant from a read-write one and a channel from a key.
fn grants_reply(grants: &[Grant], channels: &[Grant]) -> Vec<u8> {
    let keys = grants.iter().map(|g| match g.access {
        Access::Read => format!("r={}", g.pattern),
        Access::Write => format!("rw={}", g.pattern),
    });
    let chans = channels.iter().map(|g| match g.access {
        Access::Read => format!("chr={}", g.pattern),
        Access::Write => format!("chrw={}", g.pattern),
    });
    Value::Array(Some(
        keys.chain(chans)
            .map(|text| Value::BulkString(Some(text.into_bytes())))
            .collect(),
    ))
    .serialize()
}

/// Handle the SYNC command for one WebSocket connection. Forms:
///   `SYNC`                 — list this connection's current scopes
///   `SYNC TOKEN <token>`   — set scopes from a signed token (requires
///                            `RECACHED_SYNC_SECRET` on the server)
///   `SYNC <pattern> [...]` — set scopes directly (only allowed when no
///                            secret is configured — a bandwidth filter, not
///                            an authorization boundary)
pub(crate) fn handle_sync_command(
    args: &[String],
    policy: &SyncPolicy,
    current: Option<(&[Grant], &[Grant])>,
    conn_id: u64,
    now: u64,
) -> SyncOutcome {
    match args {
        [] => {
            let (grants, channels) = current.unwrap_or((&[], &[]));
            SyncOutcome::Unchanged(grants_reply(grants, channels))
        }
        [kw, token] if kw.eq_ignore_ascii_case("token") => {
            if !policy.is_strict() {
                return SyncOutcome::Unchanged(
                    b"-ERR SYNC TOKEN requires RECACHED_SYNC_SECRET to be configured on the server\r\n"
                        .to_vec(),
                );
            }
            match verify_sync_token_at(policy, token, now) {
                Ok(token) => {
                    info!(
                        "WS conn {} scoped via token: {:?} channels {:?} sub {:?} exp {:?}",
                        conn_id, token.grants, token.channels, token.subject, token.expires_at
                    );
                    SyncOutcome::Scoped {
                        reply: grants_reply(&token.grants, &token.channels),
                        token,
                    }
                }
                Err(e) => {
                    record_scope_denial("bad_token");
                    SyncOutcome::Refused(
                        Value::Error(format!("ERR invalid sync token: {}", e)).serialize(),
                    )
                }
            }
        }
        patterns => {
            if policy.is_strict() {
                return SyncOutcome::Unchanged(
                    b"-ERR this server requires signed scopes: use SYNC TOKEN <token>\r\n".to_vec(),
                );
            }
            let pats: Vec<Grant> = patterns
                .iter()
                .filter(|p| !p.is_empty())
                .map(|p| parse_grant(p))
                .filter(|g| !g.pattern.is_empty())
                .collect();
            if pats.is_empty() {
                return SyncOutcome::Unchanged(
                    b"-ERR SYNC requires at least one pattern\r\n".to_vec(),
                );
            }
            info!("WS conn {} sync scopes set: {:?}", conn_id, pats);
            SyncOutcome::Scoped {
                reply: grants_reply(&pats, &[]),
                token: VerifiedToken {
                    grants: pats,
                    channels: Vec::new(),
                    expires_at: None,
                    subject: None,
                },
            }
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
