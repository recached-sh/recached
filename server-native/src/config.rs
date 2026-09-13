//! Environment-driven configuration: limits, ports, allowlists and the
//! parsing rules that decide whether a value is honoured or refuses startup.

use crate::*;

pub(crate) const TCP_READ_BUFFER_BYTES: usize = 16 * 1024; // 16 KB — matches Redis default

pub(crate) const MAX_TCP_READ_BUFFER_BYTES: usize = 64 * 1024 * 1024; // 64 MB per connection

/// Per-connection limits. Compiled-in defaults, overridable at startup because
/// the right value is workload-dependent — Redis exposes `maxmemory-samples`
/// for the same reason. Read once and cached; changing one needs a restart.
pub(crate) fn env_limit(var: &str, default: usize) -> usize {
    std::env::var(var)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(default)
}

/// Commands queued inside one `MULTI`. Override: `RECACHED_MAX_MULTI_QUEUE`.
pub(crate) fn max_multi_queue_len() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| env_limit("RECACHED_MAX_MULTI_QUEUE", 10_000))
}

/// Keys one connection may `WATCH`. Override: `RECACHED_MAX_WATCHES_PER_CONN`.
pub(crate) fn max_watches_per_conn() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| env_limit("RECACHED_MAX_WATCHES_PER_CONN", 1_024))
}

/// Live queries one connection may hold. Override: `RECACHED_MAX_LIVE_QUERIES`.
pub(crate) fn max_qsubs_per_conn() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| env_limit("RECACHED_MAX_LIVE_QUERIES", 64))
}

/// Exact channels plus glob patterns one connection may subscribe to.
/// Override: `RECACHED_MAX_PUBSUB_SUBSCRIPTIONS`.
pub(crate) fn max_pubsub_subscriptions_per_conn() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| env_limit("RECACHED_MAX_PUBSUB_SUBSCRIPTIONS", 1_024))
}

/// Cap on the number of key/value pairs returned as QSUB initial state, so a
/// pattern matching a huge keyspace cannot produce an unbounded reply frame.
/// Keys returned in a live query's initial state.
/// Override: `RECACHED_MAX_QSUB_INITIAL_KEYS`.
pub(crate) fn max_qsub_initial_keys() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| env_limit("RECACHED_MAX_QSUB_INITIAL_KEYS", 10_000))
}

/// Largest complete WebSocket message the browser-facing port will reassemble.
/// Override: `RECACHED_WS_MAX_MESSAGE_BYTES`.
///
/// tungstenite defaults to 64 MiB per message and 16 MiB per frame, and it
/// buffers the whole message before the RESP parser — and therefore before
/// `MAX_BULK_STRING_BYTES` and before the `NOAUTH` check — ever sees a byte.
/// An unauthenticated client that opens a fragmented message and never
/// finishes it holds that much memory per connection: measured at ~68 MiB
/// each, so the default `RECACHED_MAX_CONNECTIONS` of 1024 puts tens of
/// gigabytes within reach of anyone who can reach the port.
///
/// 8 MiB is far above any real browser-sync frame (a `qstate` is bounded by
/// `RECACHED_MAX_QSUB_INITIAL_KEYS`) while capping the exposure at 8 GiB of
/// worst case rather than 64.
pub(crate) fn ws_max_message_bytes() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| env_limit("RECACHED_WS_MAX_MESSAGE_BYTES", 8 * 1024 * 1024))
}

/// Largest single WebSocket frame. Override: `RECACHED_WS_MAX_FRAME_BYTES`.
/// Never larger than one whole message, which is the real bound.
pub(crate) fn ws_max_frame_bytes() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        env_limit("RECACHED_WS_MAX_FRAME_BYTES", 8 * 1024 * 1024).min(ws_max_message_bytes())
    })
}

/// Close code sent to a browser whose mutation stream developed a gap. In the
/// private 4000-4999 range, so it can never collide with an RFC 6455 code.
/// The client resynchronises by reconnecting; see `handle_ws`.
pub(crate) const WS_CLOSE_SYNC_GAP: u16 = 4001;

pub(crate) const BROADCAST_CHANNEL_CAPACITY: usize = 512;

pub(crate) const DEFAULT_MAX_CONNECTIONS: usize = 1024;

pub(crate) const MAX_AUTH_FAILURES: u32 = 5;

pub(crate) const EVICTION_INTERVAL_SECS: u64 = 1;

pub(crate) const DEFAULT_HANDSHAKE_TIMEOUT_SECS: u64 = 10;

/// Longest replication auth line accepted, in bytes.
pub(crate) const MAX_REPL_AUTH_LINE: usize = 512;

/// Window over which failed replication auth attempts are counted per peer.
pub(crate) const REPL_AUTH_WINDOW: Duration = Duration::from_secs(60);

/// Peers tracked before the throttle sweeps expired entries.
pub(crate) const REPL_AUTH_SWEEP_THRESHOLD: usize = 1024;

// ── private file writes ───────────────────────────────────────────────────────

/// Parse a human-readable memory size string (e.g. "512mb", "1gb", "262144")
/// into a byte count. Returns None on parse failure.
/// Parse `RECACHED_ALLOW_IPS` into exact addresses.
///
/// Every entry must parse. The previous behaviour logged and dropped invalid
/// entries, which quietly narrowed a security control: a mistyped CIDR range
/// like `10.0.0.0/8` produced an allowlist that did not include the hosts the
/// operator wrote, and an entirely invalid list produced an empty one — which
/// rejects *every* connection while the server still reports itself healthy.
/// Refusing to start makes the misconfiguration impossible to miss.
pub(crate) fn parse_allow_ips(raw: &str) -> Result<Vec<IpAddr>, String> {
    let mut ips = Vec::new();
    for entry in raw.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        match IpAddr::from_str(trimmed) {
            Ok(ip) => ips.push(ip),
            Err(_) => {
                return Err(format!(
                    "RECACHED_ALLOW_IPS: '{trimmed}' is not a valid IP address. Exact addresses \
                     only — CIDR ranges and hostnames are not supported. Refusing to start rather \
                     than applying a narrower allowlist than configured."
                ));
            }
        }
    }
    if ips.is_empty() {
        return Err(
            "RECACHED_ALLOW_IPS is set but contains no valid addresses — this would reject every \
             connection. Unset it to accept all connections."
                .to_string(),
        );
    }
    Ok(ips)
}

/// Parse a boolean environment variable, rejecting anything ambiguous.
///
/// Silently treating `RECACHED_REPL_ENABLE=please` as false would leave an
/// operator believing replication was on when it was not; treating it as true
/// would open a port they never asked for. Neither is acceptable for a variable
/// that gates a security boundary, so an unrecognised value refuses to start.
pub(crate) fn parse_env_bool(var: &str, raw: &str) -> Result<bool, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(format!(
            "{var}: '{other}' is not a boolean. Use 1/true/yes/on or 0/false/no/off."
        )),
    }
}

/// Resolve a listening port from the environment, or `default` when unset.
///
/// A typo refuses to start rather than falling back. Silently serving 6379 to
/// an operator who asked for 7000 puts the keyspace on a port they believe is
/// closed — the same reasoning as [`parse_env_bool`]. Port 0 is rejected for
/// the same reason: the OS would assign an arbitrary free port and nothing
/// would be reachable at the address anyone was told to use.
pub(crate) fn parse_env_port(var: &str, default: u16) -> Result<u16, String> {
    match std::env::var(var) {
        Err(_) => Ok(default),
        Ok(raw) => match raw.trim().parse::<u16>() {
            Ok(0) | Err(_) => Err(format!(
                "{var}: '{}' is not a valid port. Use 1-65535.",
                raw.trim()
            )),
            Ok(p) => Ok(p),
        },
    }
}

/// The metrics port, or `None` when metrics are switched off.
///
/// Unlike the data ports, `0` is meaningful here — the documented way to turn
/// the exporter off — so it is answered rather than refused. Everything else is
/// validated the same way, because a typo silently exporting on 9091 is the
/// same failure as a typo silently serving the keyspace on 6379.
pub(crate) fn parse_env_metrics_port() -> Result<Option<u16>, String> {
    const VAR: &str = "RECACHED_METRICS_PORT";
    match std::env::var(VAR) {
        Err(_) => Ok(Some(9091)),
        Ok(raw) => match raw.trim().parse::<u16>() {
            Ok(0) => Ok(None),
            Ok(p) => Ok(Some(p)),
            Err(_) => Err(format!(
                "{VAR}: '{}' is not a valid port. Use 1-65535, or 0 to disable metrics.",
                raw.trim()
            )),
        },
    }
}

/// Upper bound on `RECACHED_WORKER_THREADS`.
///
/// Tokio panics outright above `1 << 15` worker threads. A value anywhere near
/// that is a typo rather than an intent, and a panic during runtime
/// construction happens before the tracing subscriber is installed — so the
/// operator would get a bare backtrace with no clue which variable caused it.
pub(crate) const MAX_WORKER_THREADS: usize = 1024;

/// Worker threads in the Tokio runtime, or `None` to let Tokio decide (one per
/// available core, which is what every release before this one did).
///
/// Exposed because it is the only honest way to A/B the threading model on one
/// machine. `RECACHED_WORKER_THREADS=1` makes command execution single-threaded
/// — the way Redis and Valkey run — so a benchmark can attribute a throughput
/// difference to parallelism rather than to the hardware it happened to run on.
/// It is also the escape hatch for an operator who wants Recached to share a
/// box with something else and not claim every core.
///
/// A typo refuses to start rather than falling back, for the same reason
/// [`parse_env_port`] does: silently running 8 threads for an operator who
/// asked for 2 is a capacity decision made behind their back.
pub(crate) fn worker_threads() -> Result<Option<usize>, String> {
    const VAR: &str = "RECACHED_WORKER_THREADS";
    match std::env::var(VAR) {
        // Absent is the ordinary case: Tokio's default is a good one.
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{VAR}: value is not valid UTF-8.")),
        Ok(raw) => match raw.trim().parse::<usize>() {
            Ok(0) | Err(_) => Err(format!(
                "{VAR}: '{}' is not a thread count. Use 1-{MAX_WORKER_THREADS}, \
                 or leave it unset for one per core.",
                raw.trim()
            )),
            Ok(n) if n > MAX_WORKER_THREADS => Err(format!(
                "{VAR}: {n} exceeds the maximum of {MAX_WORKER_THREADS}."
            )),
            Ok(n) => Ok(Some(n)),
        },
    }
}

/// True when `bind_host` can only be reached from this machine.
///
/// A hostname that does not parse as an address is treated as public: the
/// conservative answer is the one that demands a password.
pub(crate) fn bind_is_loopback(bind_host: &str) -> bool {
    if bind_host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    // An IPv6 bind address has to be written bracketed — `[::1]` — because the
    // listeners format it as `{host}:{port}`, and `::1:6379` does not parse.
    // Without stripping them, `[::1]` fails to parse as an address and would be
    // classified as public, demanding a replication password on what is in fact
    // loopback.
    let host = bind_host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(bind_host);
    IpAddr::from_str(host)
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

/// Whether to bind the replication listener, given the environment.
///
/// This listener used to start unconditionally on every node. The stated reason
/// was multi-tier replication — a replica must be able to serve sub-replicas —
/// but the cost was that *every* default deployment opened a port carrying the
/// entire keyspace to anyone who connected: with no `RECACHED_REPL_PASSWORD`,
/// `handle_replica` skips the handshake and sends a full snapshot followed by a
/// live stream of every subsequent write. An operator who set
/// `RECACHED_PASSWORD` had every reason to believe the data was behind
/// authentication, and it was not.
///
/// Two rules now apply. The port is closed unless `RECACHED_REPL_ENABLE` says
/// otherwise, and enabling it on an interface other than loopback without a
/// password refuses to start rather than serving the keyspace unauthenticated.
/// Multi-tier replication still works — a node that serves sub-replicas sets
/// the variable, which is the point: it is now a decision rather than a default.
pub(crate) fn resolve_repl_listen(
    enable: Option<String>,
    bind_host: &str,
    password: Option<&str>,
) -> Result<bool, String> {
    let enabled = match enable.as_deref().map(str::trim) {
        None | Some("") => false,
        Some(v) => parse_env_bool("RECACHED_REPL_ENABLE", v)?,
    };
    if !enabled {
        return Ok(false);
    }
    let has_password = password.is_some_and(|p| !p.is_empty());
    if !has_password && !bind_is_loopback(bind_host) {
        return Err(format!(
            "RECACHED_REPL_ENABLE is set and RECACHED_BIND is '{bind_host}', but \
             RECACHED_REPL_PASSWORD is unset — refusing to start. The replication port serves the \
             entire keyspace to whoever connects, so on any interface reachable from the network \
             it must be authenticated. Set RECACHED_REPL_PASSWORD, or bind to 127.0.0.1."
        ));
    }
    Ok(true)
}

/// Parse `RECACHED_ALLOWED_ORIGINS` into exact origins.
///
/// An origin is scheme + host + optional port and nothing else, so an entry
/// carrying a path is a misunderstanding of what will be compared against — and
/// one that would silently never match. Reject it at startup, in the same
/// spirit as `parse_allow_ips`.
pub(crate) fn parse_allowed_origins(raw: &str) -> Result<Vec<String>, String> {
    let mut origins = Vec::new();
    for entry in raw.split(',') {
        let trimmed = entry.trim().trim_end_matches('/');
        if trimmed.is_empty() {
            continue;
        }
        // Sandboxed iframes and `file://` documents send the literal `null`.
        // An operator may legitimately need to admit them.
        if trimmed.eq_ignore_ascii_case("null") {
            origins.push("null".to_string());
            continue;
        }
        let Some((scheme, authority)) = trimmed.split_once("://") else {
            return Err(format!(
                "RECACHED_ALLOWED_ORIGINS: '{trimmed}' is not an origin — it needs a scheme, e.g. \
                 https://app.example.com."
            ));
        };
        if scheme.is_empty() || authority.is_empty() {
            return Err(format!(
                "RECACHED_ALLOWED_ORIGINS: '{trimmed}' is not an origin — expected \
                 scheme://host[:port]."
            ));
        }
        if authority.contains('/') {
            return Err(format!(
                "RECACHED_ALLOWED_ORIGINS: '{trimmed}' contains a path. An origin is \
                 scheme://host[:port] only, and a browser will never send a path — this entry \
                 could never match."
            ));
        }
        origins.push(trimmed.to_ascii_lowercase());
    }
    if origins.is_empty() {
        return Err(
            "RECACHED_ALLOWED_ORIGINS is set but lists no origins — this would reject every \
             browser. Unset it to accept all origins."
                .to_string(),
        );
    }
    Ok(origins)
}

/// Whether a WebSocket handshake carrying `origin` may proceed.
///
/// Browsers apply neither CORS nor a preflight to WebSockets, so without this
/// check any page a user visits can open a socket to a reachable Recached and
/// act with that user's network position. On the common `ws://localhost:6380`
/// development setup that means every site in every tab.
///
/// `Origin` is not a boundary against a native client, which simply omits the
/// header — and that is why an absent origin is allowed. What it does
/// distinguish is "the application I deployed" from "some other page in the same
/// browser", which is precisely the threat this port faces. An unset allowlist
/// permits everything, matching how an unset `RECACHED_PASSWORD` behaves.
pub(crate) fn origin_allowed(allowed: Option<&[String]>, origin: Option<&str>) -> bool {
    let Some(list) = allowed else {
        return true;
    };
    let Some(origin) = origin else {
        return true;
    };
    let origin = origin.trim().trim_end_matches('/');
    list.iter().any(|a| a.eq_ignore_ascii_case(origin))
}

/// How long a connection may take to complete its TLS and/or WebSocket
/// handshake. Override: `RECACHED_HANDSHAKE_TIMEOUT` (seconds).
///
/// The connection permit is taken before the handshake runs, so without a
/// deadline a client that opens a socket and then says nothing holds one of
/// `RECACHED_MAX_CONNECTIONS` slots indefinitely. A thousand such sockets cost
/// an attacker nothing and stop the server accepting real clients.
pub(crate) fn handshake_timeout() -> Duration {
    static V: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    Duration::from_secs(*V.get_or_init(|| {
        env_limit(
            "RECACHED_HANDSHAKE_TIMEOUT",
            DEFAULT_HANDSHAKE_TIMEOUT_SECS as usize,
        ) as u64
    }))
}

pub(crate) fn parse_memory_bytes(s: &str) -> Option<usize> {
    let s = s.trim().to_lowercase();
    if let Some(n) = s.strip_suffix("gb") {
        n.trim()
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_mul(1024 * 1024 * 1024))
    } else if let Some(n) = s.strip_suffix("mb") {
        n.trim()
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_mul(1024 * 1024))
    } else if let Some(n) = s.strip_suffix("kb") {
        n.trim()
            .parse::<usize>()
            .ok()
            .and_then(|n| n.checked_mul(1024))
    } else {
        s.parse().ok()
    }
}

/// A single autosave condition: save if `changes` or more writes have
/// accumulated within `secs` seconds of the last save.
pub(crate) struct SaveCondition {
    pub(crate) secs: u64,
    pub(crate) changes: u64,
}

/// Parse `RECACHED_SAVE` value: comma-separated `seconds:changes` pairs.
/// Example: `"900:1,300:10,60:10000"` → save after 1 change in 15 min,
/// 10 changes in 5 min, or 10 000 changes in 1 min — whichever comes first.
pub(crate) fn parse_save_conditions(s: &str) -> Result<Vec<SaveCondition>, String> {
    if s.trim().is_empty() || s.trim() == "0" {
        return Ok(Vec::new());
    }
    s.split(',')
        .map(|pair| {
            let pair = pair.trim();
            let (secs, changes) = pair
                .split_once(':')
                .ok_or_else(|| format!("RECACHED_SAVE: invalid condition '{pair}'"))?;
            let secs = secs
                .trim()
                .parse::<u64>()
                .map_err(|_| format!("RECACHED_SAVE: invalid seconds in '{pair}'"))?;
            let changes = changes
                .trim()
                .parse::<u64>()
                .map_err(|_| format!("RECACHED_SAVE: invalid change count in '{pair}'"))?;
            if secs == 0 || changes == 0 {
                return Err(format!(
                    "RECACHED_SAVE: seconds and changes must be positive in '{pair}'"
                ));
            }
            Ok(SaveCondition { secs, changes })
        })
        .collect()
}

// ── Replication server (primary side) ────────────────────────────────────────

#[cfg(test)]
// SAFETY: every `set_var`/`remove_var` below is serialised by this
// module's `ENV_LOCK`, so no other thread in the process reads or writes
// the same variable concurrently. Edition 2024 made these `unsafe`
// precisely because of that race; the lock is what discharges it.
#[allow(
    unsafe_code,
    reason = "process-global env mutation, serialised by ENV_LOCK"
)]
mod limit_config_tests {
    use super::*;

    /// Serialises the tests in this module.
    ///
    /// Environment variables are process-global and `cargo test` runs tests on
    /// parallel threads, so a test that sets `RECACHED_MAX_*` races any test
    /// reading the same variable — which is why `set_var` is `unsafe`. This
    /// surfaced as `compiled_defaults_match_the_documented_values`
    /// intermittently observing an override (`7`) instead of a default
    /// (`10_000`): it passed locally and failed in CI purely on thread timing.
    ///
    /// Poisoning is ignored deliberately: one failing test must not cascade
    /// into unrelated failures in the rest of the module.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn worker_threads_defaults_to_tokio_when_unset() {
        let _guard = env_guard();
        unsafe { std::env::remove_var("RECACHED_WORKER_THREADS") };
        assert_eq!(worker_threads(), Ok(None));
    }

    #[test]
    fn worker_threads_accepts_a_positive_count() {
        let _guard = env_guard();
        unsafe { std::env::set_var("RECACHED_WORKER_THREADS", " 4 ") };
        assert_eq!(worker_threads(), Ok(Some(4)));
        unsafe { std::env::set_var("RECACHED_WORKER_THREADS", "1") };
        assert_eq!(worker_threads(), Ok(Some(1)), "1 is the Redis-shaped case");
        unsafe { std::env::remove_var("RECACHED_WORKER_THREADS") };
    }

    #[test]
    fn worker_threads_refuses_nonsense_rather_than_falling_back() {
        let _guard = env_guard();
        // Silently running one-per-core for an operator who asked for 2 is a
        // capacity decision made behind their back — the same reasoning that
        // makes a bad RECACHED_PORT refuse to start.
        for bad in ["", "  ", "abc", "0", "-1", "1.5", "1025"] {
            unsafe { std::env::set_var("RECACHED_WORKER_THREADS", bad) };
            assert!(
                worker_threads().is_err(),
                "{bad:?} should refuse startup, not fall back"
            );
        }
        unsafe { std::env::remove_var("RECACHED_WORKER_THREADS") };
    }

    #[test]
    fn worker_threads_accepts_the_documented_maximum() {
        let _guard = env_guard();
        unsafe { std::env::set_var("RECACHED_WORKER_THREADS", MAX_WORKER_THREADS.to_string()) };
        assert_eq!(worker_threads(), Ok(Some(MAX_WORKER_THREADS)));
        unsafe { std::env::remove_var("RECACHED_WORKER_THREADS") };
    }

    #[test]
    fn env_limit_falls_back_to_the_default() {
        let _guard = env_guard();
        // Unset, empty, non-numeric, and zero all mean "use the default" —
        // a zero limit would disable the feature rather than tune it.
        assert_eq!(env_limit("RECACHED_DEFINITELY_UNSET_VAR_XYZ", 64), 64);
        for bad in ["", "  ", "abc", "0", "-5", "1.5"] {
            unsafe { std::env::set_var("RECACHED_TEST_LIMIT", bad) };
            assert_eq!(env_limit("RECACHED_TEST_LIMIT", 64), 64, "input {bad:?}");
        }
        unsafe { std::env::remove_var("RECACHED_TEST_LIMIT") };
    }

    #[test]
    fn env_limit_accepts_a_positive_override() {
        let _guard = env_guard();
        unsafe { std::env::set_var("RECACHED_TEST_LIMIT_OK", " 256 ") };
        assert_eq!(
            env_limit("RECACHED_TEST_LIMIT_OK", 64),
            256,
            "whitespace tolerated"
        );
        unsafe { std::env::remove_var("RECACHED_TEST_LIMIT_OK") };
    }

    #[test]
    fn overrides_are_read_from_the_documented_variable_names() {
        let _guard = env_guard();
        // A bulk rename once rewrote these string literals along with the
        // function names, leaving variables like `RECACHED_max_watches_per_conn()`
        // that no operator would ever set — the override silently did nothing.
        // Assert the names the docs promise.
        for (var, default) in [
            ("RECACHED_MAX_MULTI_QUEUE", 10_000usize),
            ("RECACHED_MAX_WATCHES_PER_CONN", 1_024),
            ("RECACHED_MAX_LIVE_QUERIES", 64),
            ("RECACHED_MAX_PUBSUB_SUBSCRIPTIONS", 1_024),
            ("RECACHED_MAX_QSUB_INITIAL_KEYS", 10_000),
            ("RECACHED_EVICTION_SAMPLE", 10),
        ] {
            assert!(
                var.chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()),
                "{var} is not a plausible environment variable name"
            );
            unsafe { std::env::set_var(var, "7") };
            assert_eq!(env_limit(var, default), 7, "{var} override ignored");
            unsafe { std::env::remove_var(var) };
        }
    }

    #[test]
    fn compiled_defaults_match_the_documented_values() {
        let _guard = env_guard();
        // These appear in docs/server/operations.md; drift would mislead
        // operators sizing a deployment.
        assert_eq!(max_multi_queue_len(), 10_000);
        assert_eq!(max_watches_per_conn(), 1_024);
        assert_eq!(max_qsubs_per_conn(), 64);
        assert_eq!(max_pubsub_subscriptions_per_conn(), 1_024);
        assert_eq!(max_qsub_initial_keys(), 10_000);
    }
}

/// The TCP and WebSocket ports were compiled in, so two instances could not
/// share a host — a replica beside its primary was impossible — and there was
/// no way off 6379, the first port any commodity scanner probes.
///
/// The port is not itself a security control (`RECACHED_BIND`, the password,
/// TLS and the allowlists are), so it is safe to expose. What is *not* safe is
/// a typo falling back to the default: an operator who asked for 7000 and
/// silently got 6379 believes a port is closed that is in fact serving the
/// keyspace. So a bad value refuses to start.
#[cfg(test)]
// SAFETY: every `set_var`/`remove_var` below is serialised by this
// module's `ENV_LOCK`, so no other thread in the process reads or writes
// the same variable concurrently. Edition 2024 made these `unsafe`
// precisely because of that race; the lock is what discharges it.
#[allow(
    unsafe_code,
    reason = "process-global env mutation, serialised by ENV_LOCK"
)]
mod port_config_tests {
    use super::*;

    /// `std::env` is process-global; serialise the tests that mutate it.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_var<T>(key: &str, value: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var(key).ok();
        unsafe {
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
        let out = f();
        unsafe {
            match prev {
                Some(p) => std::env::set_var(key, p),
                None => std::env::remove_var(key),
            }
        }
        out
    }

    const VAR: &str = "RECACHED_TEST_PORT_VAR";

    #[test]
    fn an_unset_variable_keeps_the_default() {
        with_var(VAR, None, || {
            assert_eq!(parse_env_port(VAR, 6379), Ok(6379));
        });
    }

    #[test]
    fn a_valid_port_is_taken_verbatim() {
        for (raw, want) in [
            ("7000", 7000u16),
            ("1", 1),
            ("65535", 65535),
            (" 6380 ", 6380),
        ] {
            with_var(VAR, Some(raw), || {
                assert_eq!(parse_env_port(VAR, 6379), Ok(want), "raw {raw:?}");
            });
        }
    }

    #[test]
    fn a_typo_refuses_to_start_rather_than_serving_the_default() {
        // The whole point: silently falling back would put the keyspace on a
        // port the operator believes is closed.
        for raw in ["seven thousand", "63.79", "-1", "65536", "99999", ""] {
            with_var(VAR, Some(raw), || {
                let err = parse_env_port(VAR, 6379).unwrap_err_or_else_msg(raw);
                assert!(err.contains(VAR), "{raw:?} -> {err}");
                assert!(err.contains("not a valid port"), "{raw:?} -> {err}");
            });
        }
    }

    #[test]
    fn port_zero_is_refused() {
        // The OS would assign an arbitrary free port, so nothing would be
        // reachable at the address anyone was told to use.
        with_var(VAR, Some("0"), || {
            assert!(parse_env_port(VAR, 6379).is_err());
        });
    }

    /// Small helper so the loop above reads as one assertion per case.
    trait UnwrapErrMsg {
        fn unwrap_err_or_else_msg(self, raw: &str) -> String;
    }
    impl UnwrapErrMsg for Result<u16, String> {
        fn unwrap_err_or_else_msg(self, raw: &str) -> String {
            match self {
                Ok(p) => panic!("{raw:?} should have been refused, got port {p}"),
                Err(e) => e,
            }
        }
    }
}

// ── PUBLISH inside MULTI ──────────────────────────────────────────────────────

/// `RECACHED_METRICS_PORT=0` disables the exporter.
///
/// The reference has always documented this; the code never honoured it. `0`
/// parsed fine and `bind("host:0")` hands the listener an OS-assigned ephemeral
/// port, so an operator switching metrics *off* got them served on an
/// unpredictable port instead — the opposite of the request, and unlikely to be
/// noticed until something scraped it.
#[cfg(test)]
// SAFETY: every `set_var`/`remove_var` below is serialised by this
// module's `ENV_LOCK`, so no other thread in the process reads or writes
// the same variable concurrently. Edition 2024 made these `unsafe`
// precisely because of that race; the lock is what discharges it.
#[allow(
    unsafe_code,
    reason = "process-global env mutation, serialised by ENV_LOCK"
)]
mod metrics_port_tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    const VAR: &str = "RECACHED_METRICS_PORT";

    fn with_metrics_port<T>(value: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var(VAR).ok();
        unsafe {
            match value {
                Some(v) => std::env::set_var(VAR, v),
                None => std::env::remove_var(VAR),
            }
        }
        let out = f();
        unsafe {
            match prev {
                Some(p) => std::env::set_var(VAR, p),
                None => std::env::remove_var(VAR),
            }
        }
        out
    }

    #[test]
    fn zero_means_disabled_not_an_ephemeral_port() {
        with_metrics_port(Some("0"), || {
            assert_eq!(
                parse_env_metrics_port(),
                Ok(None),
                "0 must switch the exporter off, not bind an OS-assigned port"
            );
        });
    }

    #[test]
    fn unset_keeps_the_default_port() {
        with_metrics_port(None, || {
            assert_eq!(parse_env_metrics_port(), Ok(Some(9091)));
        });
    }

    #[test]
    fn a_real_port_is_taken_verbatim() {
        with_metrics_port(Some("9092"), || {
            assert_eq!(parse_env_metrics_port(), Ok(Some(9092)));
        });
    }

    #[test]
    fn a_typo_refuses_to_start_and_mentions_the_disable_value() {
        with_metrics_port(Some("nine thousand"), || {
            let err = parse_env_metrics_port().unwrap_err();
            assert!(err.contains(VAR), "{err}");
            assert!(err.contains("0 to disable"), "{err}");
        });
    }
}
