//! The Recached server binary: configuration, listeners, and the background
//! loops that outlive any one connection.
//!
//! Everything else lives in a module named for the job it does. Roughly in the
//! order a write travels through them:
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`config`] | `RECACHED_*` parsing, limits, and which values refuse startup |
//! | [`tls`] | Certificate and key loading, for the listener and for replication |
//! | [`connection`] | The RESP and WebSocket command loops, plus HELLO/AUTH |
//! | [`clients`] | Per-connection bookkeeping and the command metrics counters |
//! | [`sync_scopes`] | Signed tokens restricting a socket to a set of key patterns |
//! | [`propagation`] | Turning an executed command into the frame everyone else replays |
//! | [`server_state`] | Shared state a write passes through: AOF, replicas, role |
//! | [`persistence`] | Snapshots and the append-only file |
//! | [`replication`] | Serving replicas, and following a primary |
//! | [`pubsub`] | The channel/pattern subscriber hub |
//! | [`watch`] | WATCH's compare-and-set registry and live queries |
//! | [`mod@info`] | INFO, CLIENT, CONFIG, COMMAND — reporting on the server, not the data |
//!
//! Modules pull crate-wide names in with `use crate::*`, and this file re-globs
//! each of them, so a shared type is nameable everywhere without a bespoke
//! import list per file. Items shared across modules are `pub(crate)`; nothing
//! here is a public API.
//!
//! **The one duplication worth knowing about:** [`connection`] carries two
//! near-identical command loops, one per transport. They have drifted before —
//! a transaction bug once had to be fixed twice — so a change to one almost
//! always belongs in the other.

// jemalloc isn't available under MSVC (see Cargo.toml); fall back to the
// system allocator there.
#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

mod clients;
mod config;
mod connection;
mod info;
mod persistence;
mod propagation;
mod pubsub;
mod replication;
mod server_state;
mod sync_scopes;
mod tls;
mod watch;

#[cfg(test)]
mod tests;

use clients::*;
use config::*;
use connection::*;
use info::*;
use persistence::*;
use propagation::*;
use pubsub::*;
use replication::*;
use server_state::*;
use sync_scopes::*;
use tls::*;
use watch::*;

use core_engine::catalog;
use core_engine::cmd::{Command, SetExpiry, ZAddCondition};
use core_engine::resp::{MAX_BULK_STRING_BYTES, Value};
use core_engine::store::{
    EvictionPolicy, KeyValueStore, KeyspaceSample, SnapshotEntry, glob_match,
};
use futures_util::{SinkExt, StreamExt};
use metrics::{counter, gauge, histogram};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::ErrorKind;
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, broadcast, mpsc};
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tokio_rustls::rustls::{ClientConfig, RootCertStore, ServerConfig};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_tungstenite::accept_hdr_async_with_config;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{
    ErrorResponse, Request as HandshakeRequest, Response as HandshakeResponse,
};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::protocol::frame::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tracing::{debug, error, info, warn};

// ── metrics ───────────────────────────────────────────────────────────────────

fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Wall-clock milliseconds since the epoch, the unit every stored expiry uses.
///
/// Read once per propagated write so [`broadcast_for`] can turn a *relative*
/// TTL into an absolute deadline — see the comment there for why that matters.
fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Builds the runtime, then hands off to [`run`].
///
/// This is a hand-rolled `#[tokio::main]`: the macro can only take a worker
/// count as a literal, and `RECACHED_WORKER_THREADS` has to be read at startup.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // All runtime configuration is via RECACHED_* env vars; the only flags are
    // --version/-V (required by e.g. the Homebrew formula's install test).
    // Answered before the runtime is built so `--version` costs no threads.
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("recached-server {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Validated before the subscriber exists, so this one message has to go to
    // stderr directly rather than through `error!`.
    let configured_workers = worker_threads().map_err(|e| {
        eprintln!("Configuration error: {e}");
        e
    })?;

    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    if let Some(n) = configured_workers {
        builder.worker_threads(n);
    }
    let runtime = builder.build()?;

    // `num_workers` reports what the runtime actually built, which is the
    // number worth logging: when the variable is unset it is the only place
    // the core count Tokio detected becomes visible. Inside a container that
    // is the cgroup's CPU allowance, not the host's core count.
    let active_workers = runtime.metrics().num_workers();
    runtime.block_on(run(active_workers, configured_workers.is_some()))
}

async fn run(
    active_workers: usize,
    workers_pinned: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // ── runtime ───────────────────────────────────────────────────────────
    // Recached executes commands on every worker, which is the main structural
    // difference from Redis and Valkey. Logging the count makes a throughput
    // number attributable after the fact: a benchmark result without it cannot
    // be told apart from the same server accidentally running on one core.
    if workers_pinned {
        info!(
            "Command execution: {} worker threads (pinned by RECACHED_WORKER_THREADS)",
            active_workers
        );
    } else {
        info!(
            "Command execution: {} worker threads (one per available core)",
            active_workers
        );
    }

    // ── bind address ──────────────────────────────────────────────────────
    // Host/interface all listeners bind to. Defaults to 0.0.0.0 (all
    // interfaces) for backwards compatibility; set RECACHED_BIND=127.0.0.1 to
    // restrict to localhost, which — together with RECACHED_PASSWORD — is
    // strongly recommended unless the server is deliberately public.
    let bind_host = std::env::var("RECACHED_BIND").unwrap_or_else(|_| "0.0.0.0".to_string());
    if bind_host == "0.0.0.0" {
        warn!(
            "Binding all interfaces (0.0.0.0). Set RECACHED_BIND=127.0.0.1 and RECACHED_PASSWORD before exposing this host."
        );
    } else {
        info!("Binding interface {}", bind_host);
    }

    // ── Listening ports ───────────────────────────────────────────────────
    // Defaults are Redis's 6379 and Recached's 6380, so an existing deployment
    // needs no configuration. They are overridable because they were not: the
    // two ports were compiled in, which made a second instance on one host
    // impossible — including a replica alongside its primary — and left no way
    // to move off 6379, the port every commodity scanner probes first.
    let (tcp_port, ws_port) = match (
        parse_env_port("RECACHED_PORT", 6379),
        parse_env_port("RECACHED_WS_PORT", 6380),
    ) {
        (Ok(t), Ok(w)) if t == w => {
            error!("RECACHED_PORT and RECACHED_WS_PORT are both {t}; they must differ.");
            std::process::exit(1);
        }
        (Ok(t), Ok(w)) => (t, w),
        (Err(msg), _) | (_, Err(msg)) => {
            error!("{msg}");
            std::process::exit(1);
        }
    };

    // ── Prometheus metrics ────────────────────────────────────────────────
    // `0` means "do not export", which the reference has always documented and
    // the code never honoured: `0` parsed fine, and binding `host:0` hands the
    // listener an OS-assigned ephemeral port. An operator who set it to turn
    // metrics *off* got them served on an unpredictable port instead — the
    // opposite of what they asked for, and unlikely to be noticed.
    let metrics_port = match parse_env_metrics_port() {
        Ok(p) => p,
        Err(msg) => {
            error!("{msg}");
            std::process::exit(1);
        }
    };
    match metrics_port {
        None => info!("Prometheus metrics DISABLED (RECACHED_METRICS_PORT=0)."),
        Some(port) => {
            let metrics_addr: std::net::SocketAddr = match format!("{}:{}", bind_host, port).parse()
            {
                Ok(a) => a,
                Err(_) => {
                    error!(
                        "RECACHED_BIND '{bind_host}' and RECACHED_METRICS_PORT {port} do not \
                             form a valid address. An IPv6 bind host must be bracketed, as in \
                             [::1]."
                    );
                    std::process::exit(1);
                }
            };
            if let Err(e) = metrics_exporter_prometheus::PrometheusBuilder::new()
                .with_http_listener(metrics_addr)
                .install()
            {
                // Almost always a second instance on the same host: the data
                // ports are configurable, so this one has to be too, or the
                // collision simply moves here — and it used to arrive as a
                // panic with a backtrace note, which reads like a bug in
                // Recached rather than two servers wanting one port.
                error!(
                    "Could not start the Prometheus exporter on {metrics_addr}: {e}. \
                     If another Recached instance is already running on this host, give this one \
                     its own RECACHED_METRICS_PORT (as well as RECACHED_PORT and \
                     RECACHED_WS_PORT), or set RECACHED_METRICS_PORT=0 to disable metrics."
                );
                std::process::exit(1);
            }
            info!("Prometheus metrics at http://{}/metrics", metrics_addr);
        }
    }

    // ── auth ──────────────────────────────────────────────────────────────
    let password = std::env::var("RECACHED_PASSWORD").ok();
    let global_password = Arc::new(password);

    if global_password.is_some() {
        info!("Authentication ENABLED. Clients must send 'AUTH <password>'.");
    } else {
        warn!("Authentication DISABLED. Set RECACHED_PASSWORD to enable.");
    }

    // ── sync scoping ──────────────────────────────────────────────────────
    let sync_secret: Arc<Option<String>> = Arc::new(
        std::env::var("RECACHED_SYNC_SECRET")
            .ok()
            .filter(|s| !s.is_empty()),
    );
    if sync_secret.is_some() {
        info!(
            "Sync scoping ENABLED (strict): WebSocket clients receive no pushes and no key access until they present 'SYNC TOKEN <token>'."
        );
    } else {
        warn!(
            "Sync scoping DISABLED: every WebSocket client receives every mutation. Set RECACHED_SYNC_SECRET before exposing port {} to untrusted clients.",
            ws_port
        );
    }

    // ── IP allowlist ──────────────────────────────────────────────────────
    let allowed_ips: Option<Arc<Vec<IpAddr>>> = match std::env::var("RECACHED_ALLOW_IPS").ok() {
        None => None,
        Some(raw) => match parse_allow_ips(&raw) {
            Ok(ips) => Some(Arc::new(ips)),
            Err(msg) => {
                error!("{msg}");
                std::process::exit(1);
            }
        },
    };

    if let Some(ips) = &allowed_ips {
        info!("IP allowlist ENABLED: {:?}", ips);
    } else {
        warn!("IP allowlist DISABLED. Accepting all connections.");
    }

    // ── WebSocket origin allowlist ────────────────────────────────────────
    let allowed_origins: Arc<Option<Vec<String>>> =
        Arc::new(match std::env::var("RECACHED_ALLOWED_ORIGINS").ok() {
            None => None,
            Some(raw) => match parse_allowed_origins(&raw) {
                Ok(list) => Some(list),
                Err(msg) => {
                    error!("{msg}");
                    std::process::exit(1);
                }
            },
        });

    if let Some(list) = allowed_origins.as_ref() {
        info!("WebSocket origin allowlist ENABLED: {:?}", list);
    } else {
        warn!(
            "WebSocket origin allowlist DISABLED. Any web page a user visits can open a socket to \
             port {} — browsers apply neither CORS nor a preflight to WebSockets. Set \
             RECACHED_ALLOWED_ORIGINS before exposing this port to a browser.",
            ws_port
        );
    }

    // ── store ─────────────────────────────────────────────────────────────
    let max_keys = match std::env::var("RECACHED_MAX_KEYS") {
        Ok(value) => Some(value.trim().parse::<usize>().map_err(|_| {
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "RECACHED_MAX_KEYS must be a non-negative integer",
            )
        })?),
        Err(std::env::VarError::NotPresent) => None,
        Err(error) => return Err(error.into()),
    };

    let max_memory_bytes = match std::env::var("RECACHED_MAX_MEMORY") {
        Ok(value) => Some(parse_memory_bytes(&value).ok_or_else(|| {
            std::io::Error::new(
                ErrorKind::InvalidInput,
                "RECACHED_MAX_MEMORY must be a non-overflowing byte count or KB/MB/GB value",
            )
        })?),
        Err(std::env::VarError::NotPresent) => None,
        Err(error) => return Err(error.into()),
    };

    let eviction_raw = std::env::var("RECACHED_EVICTION").unwrap_or_default();
    let eviction_policy = match eviction_raw.to_lowercase().as_str() {
        "" | "noeviction" => EvictionPolicy::NoEviction,
        "allkeys-lru" | "lru" => EvictionPolicy::AllKeysLru,
        "allkeys-random" | "random" => EvictionPolicy::AllKeysRandom,
        "volatile-lru" => EvictionPolicy::VolatileLru,
        "volatile-ttl" | "ttl" => EvictionPolicy::VolatileTtl,
        _ => {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                format!("RECACHED_EVICTION has unknown policy '{eviction_raw}'"),
            )
            .into());
        }
    };

    if max_keys.is_some() || max_memory_bytes.is_some() {
        info!(
            "Key limit: {:?}, memory limit: {:?} bytes, eviction: {:?}",
            max_keys, max_memory_bytes, eviction_policy
        );
    }

    let mut store_inner = KeyValueStore::with_config(max_keys, max_memory_bytes, eviction_policy);
    // Eviction sample size — the knob Redis exposes as `maxmemory-samples`.
    // Configured before the store is shared, so no interior mutability is needed.
    store_inner.set_eviction_sample(env_limit("RECACHED_EVICTION_SAMPLE", 10));
    let store = Arc::new(store_inner);

    // ── snapshot persistence ──────────────────────────────────────────────
    let save_path = PathBuf::from(
        std::env::var("RECACHED_SAVE_PATH").unwrap_or_else(|_| "recached.rdb".to_string()),
    );

    // RECACHED_SAVE takes priority: "900:1,300:10,60:10000" (secs:changes pairs).
    // Falls back to RECACHED_SAVE_INTERVAL (single-condition, 1 change required).
    let save_conditions: Vec<SaveCondition> = if let Ok(s) = std::env::var("RECACHED_SAVE") {
        parse_save_conditions(&s)
            .map_err(|error| std::io::Error::new(ErrorKind::InvalidInput, error))?
    } else {
        let interval: u64 = std::env::var("RECACHED_SAVE_INTERVAL")
            .ok()
            .map(|value| {
                value.trim().parse().map_err(|_| {
                    std::io::Error::new(
                        ErrorKind::InvalidInput,
                        "RECACHED_SAVE_INTERVAL must be a non-negative integer",
                    )
                })
            })
            .transpose()?
            .unwrap_or(900);
        if interval > 0 {
            vec![SaveCondition {
                secs: interval,
                changes: 1,
            }]
        } else {
            vec![]
        }
    };

    let loaded_snapshot = load_snapshot_checkpoint(&store, &save_path)
        .await
        .map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!("failed to load snapshot {}: {e}", save_path.display()),
            )
        })?;

    let snap_cfg = Arc::new(SnapshotConfig {
        path: save_path,
        last_save: AtomicI64::new(now_unix_secs()),
        checkpoint_id: AtomicU64::new(loaded_snapshot.aof_checkpoint),
    });

    // ── AOF ───────────────────────────────────────────────────────────────
    let aof_path = std::env::var("RECACHED_AOF_PATH").ok().map(PathBuf::from);
    let aof_sync_raw = std::env::var("RECACHED_AOF_SYNC").unwrap_or_default();
    let aof_sync = match aof_sync_raw.to_lowercase().as_str() {
        "always" => AofSync::Always,
        "no" => AofSync::No,
        "" | "everysec" => AofSync::EverySec,
        _ => {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                format!("RECACHED_AOF_SYNC has unknown mode '{aof_sync_raw}'"),
            )
            .into());
        }
    };

    let aof: Option<Arc<AofWriter>> = if let Some(path) = aof_path {
        match AofWriter::open(path.clone(), aof_sync).await {
            Ok(w) => {
                replay_aof_from_checkpoint(&store, &path, loaded_snapshot.aof_checkpoint)
                    .await
                    .map_err(|e| {
                        std::io::Error::new(
                            e.kind(),
                            format!("failed to replay AOF {}: {e}", path.display()),
                        )
                    })?;
                let writer = Arc::new(w);
                info!(
                    "AOF enabled: {:?} (sync={})",
                    path,
                    match aof_sync {
                        AofSync::Always => "always",
                        AofSync::EverySec => "everysec",
                        AofSync::No => "no",
                    }
                );
                // `always` fsyncs inside the AOF lock, so every write in the
                // process waits for one disk barrier — measured at roughly
                // 20 ms per append on APFS, i.e. tens of writes per second
                // rather than tens of thousands. That is the honest cost of the
                // guarantee, but an operator who picked it casually will read
                // the result as a hang, so say so at startup.
                if aof_sync == AofSync::Always {
                    warn!(
                        "RECACHED_AOF_SYNC=always fsyncs on every write and serialises all writers \
                         behind it — expect write throughput in the tens per second. Use everysec \
                         unless you genuinely cannot lose one second of writes."
                    );
                }
                Some(writer)
            }
            Err(e) => {
                return Err(std::io::Error::new(
                    e.kind(),
                    format!("failed to open configured AOF {}: {e}", path.display()),
                )
                .into());
            }
        }
    } else {
        None
    };

    // ── TLS ───────────────────────────────────────────────────────────────
    // Resolved before replication: the replication listener uses the same
    // certificate, so it has to exist before that listener is spawned.
    let tls_acceptor: Option<TlsAcceptor> = load_tls_acceptor();
    if tls_acceptor.is_some() {
        info!(
            "TLS ENABLED (cert={}, key={})",
            std::env::var("RECACHED_TLS_CERT").unwrap_or_default(),
            std::env::var("RECACHED_TLS_KEY").unwrap_or_default()
        );
    } else {
        warn!("TLS DISABLED. Set RECACHED_TLS_CERT and RECACHED_TLS_KEY to enable.");
    }
    let tls_acceptor = Arc::new(tls_acceptor);

    // ── connection limiter ────────────────────────────────────────────────
    // Resolved before replication because the replication listener shares this
    // budget: a flood of replica connections must not be able to starve real
    // clients, and `maxclients` should mean the total.
    let max_connections = std::env::var("RECACHED_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_CONNECTIONS);
    info!("Max connections: {}", max_connections);
    let semaphore = Arc::new(Semaphore::new(max_connections));

    // ── Replication ───────────────────────────────────────────────────────
    let replicaof = std::env::var("RECACHED_REPLICAOF").ok();
    let repl_port: u16 = std::env::var("RECACHED_REPL_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6381);
    let repl_password: Option<String> = std::env::var("RECACHED_REPL_PASSWORD").ok();
    let repl_channel_capacity: usize = std::env::var("RECACHED_REPL_BUFFER")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n: &usize| n > 0)
        .unwrap_or(DEFAULT_REPL_CHANNEL_CAPACITY);
    let repl_queue_bytes = std::env::var("RECACHED_REPL_BUFFER_BYTES")
        .ok()
        .and_then(|value| parse_memory_bytes(&value))
        .filter(|&bytes| bytes > 0)
        .unwrap_or(DEFAULT_REPL_QUEUE_BYTES);
    let repl_backlog_bytes = std::env::var("RECACHED_REPL_BACKLOG_BYTES")
        .ok()
        .and_then(|value| parse_memory_bytes(&value))
        .filter(|&bytes| bytes > 0)
        .unwrap_or(DEFAULT_REPL_BACKLOG_BYTES);
    let failover_timeout_secs: Option<u64> = std::env::var("RECACHED_FAILOVER_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n| n > 0);

    // Whether to bind the replication listener at all. Refuses to start on a
    // network-reachable interface without a password rather than serving the
    // keyspace unauthenticated.
    let repl_listen = match resolve_repl_listen(
        std::env::var("RECACHED_REPL_ENABLE").ok(),
        &bind_host,
        repl_password.as_deref(),
    ) {
        Ok(v) => v,
        Err(msg) => {
            error!("{msg}");
            std::process::exit(1);
        }
    };

    // Outbound replication TLS. Configured separately from the listener's TLS
    // because the two directions are independent: this node may serve replicas
    // over TLS, follow a primary over TLS, both, or neither.
    let repl_tls: Option<(TlsConnector, String)> = match std::env::var("RECACHED_REPL_TLS_CA")
        .ok()
        .filter(|s| !s.is_empty())
    {
        None => None,
        Some(ca) => match load_repl_tls_connector(&ca) {
            Ok(connector) => {
                let servername = repl_tls_servername(
                    replicaof.as_deref().unwrap_or_default(),
                    std::env::var("RECACHED_REPL_TLS_SERVERNAME").ok(),
                );
                info!(
                    "Replication client TLS ENABLED (CA={}, verifying primary as '{}')",
                    ca, servername
                );
                Some((connector, servername))
            }
            Err(msg) => {
                error!("{msg}");
                std::process::exit(1);
            }
        },
    };

    if replicaof.is_some() && repl_tls.is_none() {
        warn!(
            "Replication to the primary is PLAINTEXT — the password and the entire keyspace cross \
             the network unencrypted, and the primary's identity is not verified. Set \
             RECACHED_REPL_TLS_CA, or keep replication on a private network."
        );
    }

    if !repl_listen {
        info!(
            "Replication server DISABLED — port {} is not bound. Set RECACHED_REPL_ENABLE=1 on any \
             node that serves replicas (including a replica serving sub-replicas).",
            repl_port
        );
    } else if repl_password.is_some() {
        info!(
            "Replication server ENABLED on port {} with auth (RECACHED_REPL_PASSWORD is set).",
            repl_port
        );
    } else {
        warn!(
            "Replication server ENABLED on port {} WITHOUT a password, on loopback only. It serves \
             the entire keyspace to whoever connects — set RECACHED_REPL_PASSWORD before binding \
             any other interface.",
            repl_port
        );
    }

    let is_replica_start = replicaof.is_some();
    let replicas: ReplRegistry = ReplHub::new();
    if repl_listen {
        replicas.configure_backlog(repl_backlog_bytes);
        replicas.configure_queue_bytes(repl_queue_bytes);
        info!(
            "Replication buffers: backlog={} bytes, per-replica queue={} bytes / {} frames",
            repl_backlog_bytes, repl_queue_bytes, repl_channel_capacity
        );
    }

    // ── server state ──────────────────────────────────────────────────────
    let state = Arc::new(ServerState {
        snap: Arc::clone(&snap_cfg),
        aof,
        replicas: Arc::clone(&replicas),
        is_replica: std::sync::atomic::AtomicBool::new(is_replica_start),
        dedup: std::sync::Mutex::new(HashMap::new()),
        ephemeral: std::sync::Mutex::new(HashMap::new()),
        dedup_dirty: std::sync::atomic::AtomicBool::new(false),
        dedup_order: tokio::sync::Mutex::new(()),
        save_lock: tokio::sync::Mutex::new(()),
        persistence_healthy: AtomicBool::new(true),
        persistence_failures: AtomicU64::new(0),
    });
    gauge!("recached_persistence_healthy").set(1.0);

    // Restore duplicate-suppression bookkeeping before accepting connections, so a
    // client replaying an unacknowledged write after a restart is recognised
    // rather than applied twice.
    state.load_dedup().await.map_err(|e| {
        std::io::Error::new(
            e.kind(),
            format!(
                "failed to load dedup sidecar {}: {e}",
                state.dedup_path().display()
            ),
        )
    })?;

    if let Some(aof) = &state.aof
        && aof.sync == AofSync::EverySec
    {
        let state_flush = Arc::clone(&state);
        let aof_flush = Arc::clone(aof);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                if let Err(e) = aof_flush.flush().await {
                    state_flush.record_persistence_failure("aof_fsync", &e);
                }
            }
        });
    }

    // ── autosave ──────────────────────────────────────────────────────────
    if !save_conditions.is_empty() {
        let store_snap = Arc::clone(&store);
        let state_snap = Arc::clone(&state);
        let conditions = save_conditions;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(1));
            ticker.tick().await; // skip immediate first tick
            loop {
                ticker.tick().await;
                let now = now_unix_secs();
                let last = state_snap.snap.last_save.load(Ordering::Relaxed);
                let elapsed = now.saturating_sub(last).max(0) as u64;
                let dirty = store_snap.dirty_count();
                if dirty > 0
                    && conditions
                        .iter()
                        .any(|c| elapsed >= c.secs && dirty >= c.changes)
                {
                    let _ = state_snap.save(&store_snap).await;
                }
            }
        });
        info!("Autosave active → {:?}", snap_cfg.path);
    } else {
        info!(
            "Autosave disabled (RECACHED_SAVE=0 or RECACHED_SAVE_INTERVAL=0). Use SAVE or BGSAVE manually."
        );
    }

    // ── broadcast channel (mutation sync) ────────────────────────────────
    // Carries (sender_conn_id, resp_encoded_mutation). WS receivers skip their
    // own messages. Created before replication so a replica can push the writes
    // it receives from the primary to its own local WebSocket clients.
    let (tx, _rx) = broadcast::channel::<SyncMsg>(BROADCAST_CHANNEL_CAPACITY);

    // ── start replication ─────────────────────────────────────────────────
    // Opt-in. The listener may run on a replica as well as a primary, so a
    // replica can serve sub-replicas (multi-tier replication) — but it is a
    // decision the operator makes, not a port that appears by default.
    if repl_listen {
        let store_r = Arc::clone(&store);
        let snap_r = Arc::clone(&snap_cfg);
        let reg_r = Arc::clone(&replicas);
        let pwd_r = repl_password.clone().map(Arc::new);
        let cap_r = repl_channel_capacity;
        let host_r = bind_host.clone();
        let allowed_r = allowed_ips.clone();
        let sem_r = Arc::clone(&semaphore);
        let thr_r = ReplAuthThrottle::new();
        let tls_r = Arc::clone(&tls_acceptor);
        tokio::spawn(async move {
            run_repl_server(
                host_r, repl_port, store_r, snap_r, reg_r, pwd_r, cap_r, allowed_r, sem_r, thr_r,
                tls_r,
            )
            .await;
        });
    }
    if is_replica_start && let Some(primary_addr) = replicaof {
        let store_r = Arc::clone(&store);
        let state_r = Arc::clone(&state);
        let pwd_r = repl_password.clone();
        let fo_r = failover_timeout_secs;
        let tx_r = tx.clone();
        let tls_r = repl_tls;
        tokio::spawn(async move {
            run_repl_client(primary_addr, store_r, state_r, pwd_r, fo_r, tx_r, tls_r).await;
        });
        if failover_timeout_secs.is_some() {
            info!(
                "Running as replica — RECACHED_FAILOVER_TIMEOUT is deprecated and ignored; fence the old primary, then use REPLICAOF NO ONE"
            );
        } else {
            info!(
                "Running as replica — writes are rejected until manually promoted with REPLICAOF NO ONE"
            );
        }
    }

    // ── pub/sub hub ───────────────────────────────────────────────────────
    let pubsub: SharedPubSub = Arc::new(tokio::sync::Mutex::new(PubSubHub::new()));

    // ── watch registry ────────────────────────────────────────────────────
    let watch_registry: WatchRegistry = WatchHub::new();

    // ── background expiry & eviction ──────────────────────────────────────
    // Spawned after the watch registry because the sweep now has to announce
    // what it removed. Expiry is the one removal no client commanded: reads
    // only mask an expired entry, so this sweep is the sole remover, and
    // without a keychange a replica keeps the key — with its last value —
    // forever.
    {
        let store_sweep = Arc::clone(&store);
        let state_sweep = Arc::clone(&state);
        let registry_sweep = watch_registry.clone();
        let tx_sweep = tx.clone();
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(EVICTION_INTERVAL_SECS));
            loop {
                interval.tick().await;
                let memory_over_limit = store_sweep
                    .max_memory_bytes()
                    .is_some_and(|limit| store_sweep.approximate_memory_bytes() > limit);
                if store_sweep.volatile_key_count() == 0 && !memory_over_limit {
                    continue;
                }
                // Active expiry and eviction can remove keys no client named.
                // Share the all-write barrier with snapshots and QSUB initial
                // state so removal plus notification is one observable step.
                let _write_guards = state_sweep.replicas.lock_all_writes().await;
                // Bound maintenance work per tick. The cursor advances through
                // the TTL-only dense index, so large persistent keyspaces cost
                // nothing here and large volatile keyspaces are amortized.
                let expired = store_sweep.sweep_expired_reporting_budget(256);
                if !registry_sweep.is_empty() {
                    notify_removed(&registry_sweep, &expired).await;
                }
                if store_sweep.max_memory_bytes().is_some() {
                    let (_, evicted) = store_sweep.try_evict_for_memory_reporting();
                    if !evicted.is_empty() {
                        store_sweep.mark_dirty();
                        apply_eviction_effects(
                            &evicted,
                            &tx_sweep,
                            0,
                            &state_sweep,
                            &registry_sweep,
                            &store_sweep,
                        )
                        .await;
                    }
                }
            }
        });
    }

    // ── Capacity & sync metrics ───────────────────────────────────────────
    // Traffic counters are event-driven, but capacity is a level, not an event:
    // memory, key count and eviction rate have to be sampled. Without these an
    // operator cannot answer "am I near the cap?" or "is eviction thrashing?"
    // from a dashboard — see docs/server/operations.md.
    {
        let store_m = Arc::clone(&store);
        let state_m = Arc::clone(&state);
        let registry_m = watch_registry.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                ticker.tick().await;
                // One read of the incrementally maintained counters feeds both
                // gauges and the cached sample used by INFO.
                let sample = store_m.keyspace_sample();
                store_sampled_keyspace(sample);
                gauge!("recached_memory_bytes").set(sample.memory_bytes as f64);
                gauge!("recached_keys").set(sample.keys as f64);
                counter!("recached_evictions_total").absolute(store_m.evicted_count());
                counter!("recached_persistence_errors_total", "operation" => "all")
                    .absolute(state_m.persistence_failures.load(Ordering::Relaxed));
                let last_save = state_m.snap.last_save.load(Ordering::Relaxed);
                gauge!("recached_last_save_age_seconds")
                    .set(now_unix_secs().saturating_sub(last_save).max(0) as f64);
                if let Some(aof) = &state_m.aof
                    && let Ok(metadata) = tokio::fs::metadata(&aof.path).await
                {
                    gauge!("recached_aof_bytes").set(metadata.len() as f64);
                }
                gauge!("recached_replicas_connected")
                    .set(state_m.replicas.count.load(Ordering::Relaxed) as f64);
                gauge!("recached_replication_queue_depth")
                    .set(state_m.replicas.max_queue_depth().await as f64);
                gauge!("recached_replication_queue_bytes")
                    .set(state_m.replicas.max_queue_bytes().await as f64);
                gauge!("recached_replication_lag_frames")
                    .set(state_m.replicas.max_lag_frames().await as f64);
                gauge!("recached_live_queries")
                    .set(registry_m.watched_patterns.load(Ordering::Relaxed) as f64);
                gauge!("recached_watched_keys")
                    .set(registry_m.watched_keys.load(Ordering::Relaxed) as f64);
                gauge!("recached_dedup_clients_tracked")
                    .set(state_m.dedup.lock().map(|m| m.len()).unwrap_or(0) as f64);
            }
        });
    }

    // Startup facts for INFO. Recorded once the configuration is fully
    // resolved and before any listener binds, so no connection can observe the
    // defaults.
    let _ = SERVER_FACTS.set(ServerFacts {
        start: SystemTime::now(),
        run_id: generate_run_id(),
        tcp_port,
        ws_port,
        max_connections,
        tls_enabled: tls_acceptor.is_some(),
        auth_enabled: global_password.is_some(),
        aof_enabled: state.aof.is_some(),
    });

    // ── listeners ─────────────────────────────────────────────────────────
    let n_accept = num_cpus::get();
    let tcp_listeners = make_tcp_listeners(&format!("{}:{}", bind_host, tcp_port), n_accept)?;
    info!(
        "TCP server listening on {}:{} ({} accept loop(s))",
        bind_host, tcp_port, n_accept
    );

    let ws_listener = TcpListener::bind(format!("{}:{}", bind_host, ws_port)).await?;
    info!("WebSocket server listening on {}:{}", bind_host, ws_port);

    // Spawn one accept loop per CPU core, each with its own SO_REUSEPORT socket.
    // The OS load-balances incoming connections across all loops.
    for tcp_listener in tcp_listeners {
        let store_tcp = Arc::clone(&store);
        let tx_tcp = tx.clone();
        let pass_tcp = Arc::clone(&global_password);
        let allowed_tcp = allowed_ips.clone();
        let sem_tcp = Arc::clone(&semaphore);
        let pubsub_tcp = Arc::clone(&pubsub);
        let tls_tcp = Arc::clone(&tls_acceptor);
        let watch_tcp = Arc::clone(&watch_registry);
        let snap_tcp = Arc::clone(&state);

        tokio::spawn(async move {
            loop {
                match tcp_listener.accept().await {
                    Ok((socket, addr)) => {
                        let _ = socket.set_nodelay(true);
                        if let Some(allowed) = &allowed_tcp
                            && !allowed.contains(&addr.ip())
                        {
                            debug!("TCP: rejected IP {}", addr.ip());
                            continue;
                        }
                        let permit = match Arc::clone(&sem_tcp).try_acquire_owned() {
                            Ok(p) => p,
                            Err(_) => {
                                warn!("TCP: connection limit reached, dropping {}", addr);
                                continue;
                            }
                        };
                        let s = Arc::clone(&store_tcp);
                        let t = tx_tcp.clone();
                        let p = Arc::clone(&pass_tcp);
                        let ps = Arc::clone(&pubsub_tcp);
                        let wr = Arc::clone(&watch_tcp);
                        let tls = Arc::clone(&tls_tcp);
                        let sc = Arc::clone(&snap_tcp);
                        tokio::spawn(async move {
                            let _permit = permit;
                            if let Some(acc) = tls.as_ref() {
                                // Bounded: the permit is already held, so a peer
                                // that opens a socket and never negotiates would
                                // otherwise occupy a slot indefinitely.
                                match tokio::time::timeout(handshake_timeout(), acc.accept(socket))
                                    .await
                                {
                                    Ok(Ok(tls_stream)) => {
                                        handle_tcp(
                                            tls_stream,
                                            s,
                                            t,
                                            p,
                                            ps,
                                            wr,
                                            sc,
                                            addr.to_string(),
                                        )
                                        .await
                                    }
                                    Ok(Err(e)) => {
                                        warn!("TCP TLS handshake failed from {}: {}", addr, e)
                                    }
                                    Err(_) => {
                                        debug!("TCP TLS handshake from {} timed out", addr)
                                    }
                                }
                            } else {
                                handle_tcp(socket, s, t, p, ps, wr, sc, addr.to_string()).await;
                            }
                        });
                    }
                    Err(e) => warn!("TCP accept error: {}", e),
                }
            }
        });
    }

    // ── graceful shutdown via oneshot channel ────────────────────────────
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to register SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = sigterm.recv() => {},
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        let _ = shutdown_tx.send(());
    });

    loop {
        tokio::select! {
            biased;

            res = ws_listener.accept() => {
                match res {
                    Ok((socket, addr)) => {
                        let _ = socket.set_nodelay(true);
                        if let Some(allowed) = &allowed_ips
                            && !allowed.contains(&addr.ip())
                        {
                            debug!("WS: rejected IP {}", addr.ip());
                            continue;
                        }
                        let permit = match Arc::clone(&semaphore).try_acquire_owned() {
                            Ok(p) => p,
                            Err(_) => {
                                warn!("WS: connection limit reached, dropping {}", addr);
                                continue;
                            }
                        };
                        let s = Arc::clone(&store);
                        let t = tx.clone();
                        let p = Arc::clone(&global_password);
                        let ps = Arc::clone(&pubsub);
                        let wr = Arc::clone(&watch_registry);
                        let tls = Arc::clone(&tls_acceptor);
                        let sc = Arc::clone(&state);
                        let ss = Arc::clone(&sync_secret);
                        let ao = Arc::clone(&allowed_origins);
                        let id = next_conn_id();
                        tokio::spawn(async move {
                            let _permit = permit;
                            if let Some(acc) = tls.as_ref() {
                                match tokio::time::timeout(handshake_timeout(), acc.accept(socket)).await {
                                    Ok(Ok(tls_stream)) => {
                                        handle_ws(tls_stream, s, t, p, id, ps, wr, sc, ss, ao, addr.to_string()).await
                                    }
                                    Ok(Err(e)) => warn!("WS TLS handshake failed from {}: {}", addr, e),
                                    Err(_) => debug!("WS TLS handshake from {} timed out", addr),
                                }
                            } else {
                                handle_ws(socket, s, t, p, id, ps, wr, sc, ss, ao, addr.to_string()).await;
                            }
                        });
                    }
                    Err(e) => warn!("WS accept error: {}", e),
                }
            }

            _ = &mut shutdown_rx => {
                info!("Shutdown signal received, saving final snapshot...");
                match state.save(&store).await {
                    Ok(()) => info!("Done. Goodbye."),
                    Err(e) => error!(error = %e, "final snapshot failed; shutting down with the existing persistence files intact"),
                }
                break;
            }
        }
    }

    Ok(())
}

// ── TCP handler ───────────────────────────────────────────────────────────────
