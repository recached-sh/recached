//! Connection handling: the RESP and WebSocket command loops, the listeners
//! they run on, and the HELLO/AUTH handshakes that precede them.

use crate::*;

/// Binds `n` TCP sockets on `addr`, all with `SO_REUSEPORT`, so the OS can
/// distribute incoming connections across multiple accept loops — one per
/// Tokio worker thread. Falls back to a single plain `TcpListener::bind` on
/// platforms that don't support `SO_REUSEPORT`.
pub(crate) fn make_tcp_listeners(addr: &str, n: usize) -> std::io::Result<Vec<TcpListener>> {
    use socket2::{Domain, Socket, Type};
    let socket_addr: std::net::SocketAddr = addr
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let domain = if socket_addr.is_ipv6() {
        Domain::IPV6
    } else {
        Domain::IPV4
    };
    // SO_REUSEPORT — which lets multiple sockets share one port — is Unix-only.
    // Without it, binding a second socket to the same port fails, so fall back
    // to a single accept loop on non-Unix platforms.
    #[cfg(not(unix))]
    let n = 1;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let sock = Socket::new(domain, Type::STREAM, None)?;
        sock.set_reuse_address(true)?;
        #[cfg(unix)]
        sock.set_reuse_port(true)?;
        sock.set_nonblocking(true)?;
        sock.bind(&socket_addr.into())?;
        sock.listen(4096)?;
        let std_listener: std::net::TcpListener = sock.into();
        out.push(TcpListener::from_std(std_listener)?);
    }
    Ok(out)
}

// ── TLS ───────────────────────────────────────────────────────────────────────

/// `EXEC`'s reply when any command failed to queue. Byte-identical to Redis's,
/// because clients match on the `EXECABORT` prefix to tell "the transaction was
/// refused" from "a command inside it returned an error".
pub(crate) const EXECABORT: &[u8] =
    b"-EXECABORT Transaction discarded because of previous errors.\r\n";

/// The reply for a command that must be *refused* at queue time inside `MULTI`,
/// or `None` if it may be queued.
///
/// Redis rejects an unknown verb when it is queued, not when the transaction
/// runs, and that difference is the whole point: a queue-time rejection also
/// poisons the transaction so `EXEC` runs nothing. Recached parses an
/// unrecognised verb into [`Command::Unknown`], which used to queue happily and
/// only error during `EXEC` — leaving every *other* command in the transaction
/// applied. Against a server that deliberately implements a subset of Redis,
/// that is a live hazard: `MULTI; ZPOPMIN q; LPUSH processing x; EXEC` pushed
/// onto `processing` without ever popping `q`, silently, and MULTI is exactly
/// the construct callers reach for to prevent that.
///
/// The wording matches what the store returns for the same command outside a
/// transaction, so a client sees one message for one mistake.
pub(crate) fn queue_time_rejection(cmd: &Command) -> Option<Vec<u8>> {
    match cmd {
        Command::Unknown(name) => {
            Some(Value::Error(format!("ERR unknown command '{}'", name)).serialize())
        }
        _ => None,
    }
}

/// Build a complete initial live-query snapshot or refuse it. Returning a
/// partial snapshot is unsafe because reconnect reconciliation treats absence
/// as deletion and would drop locally held keys that still exist upstream.
pub(crate) fn initial_qstate(
    store: &KeyValueStore,
    pattern: &str,
    limit: usize,
) -> Result<Vec<(String, Value)>, usize> {
    let values = store.matching_key_values(pattern, limit.saturating_add(1));
    if values.len() > limit {
        Err(limit)
    } else {
        Ok(values)
    }
}

/// Encode a pub/sub delivery for a connection speaking protocol `protover`.
///
/// RESP2 has no push type, so a subscribed RESP2 client expects a plain array
/// and cannot parse a `>` frame at all. RESP3 clients want the push type so
/// deliveries are distinguishable from command replies on a multiplexed
/// connection. The WebSocket transport is RESP3 by definition — the sync
/// protocol is specified in terms of push frames — and passes 3.
/// Handle `HELLO [protover]`, updating `protover` in place on success.
///
/// Returns the serialized reply. An unsupported version leaves the connection's
/// current protocol untouched and replies `-NOPROTO`, which is what lets a
/// client probe for RESP3 and fall back cleanly rather than being disconnected.
pub(crate) fn process_hello(
    requested: Option<&str>,
    protover: &mut u8,
    is_authenticated: bool,
    is_replica: bool,
) -> Vec<u8> {
    if let Some(raw) = requested {
        match raw.parse::<u8>() {
            Ok(v @ (2 | 3)) => *protover = v,
            _ => {
                return Value::Error("NOPROTO unsupported protocol version".to_string())
                    .serialize();
            }
        }
    }

    // Pre-auth HELLO reports the protocol but nothing about the server, so an
    // unauthenticated client cannot use it to fingerprint the deployment.
    if !is_authenticated {
        return Value::Error("NOAUTH HELLO must be called with authentication".to_string())
            .serialize();
    }

    let fields = vec![
        ("server", Value::BulkString(Some(b"recached".to_vec()))),
        (
            "version",
            Value::BulkString(Some(env!("CARGO_PKG_VERSION").as_bytes().to_vec())),
        ),
        ("proto", Value::Integer(*protover as i64)),
        ("mode", Value::BulkString(Some(b"standalone".to_vec()))),
        (
            "role",
            Value::BulkString(Some(if is_replica {
                b"replica".to_vec()
            } else {
                b"master".to_vec()
            })),
        ),
        ("modules", Value::Array(Some(vec![]))),
    ];

    if *protover >= 3 {
        Value::Map(
            fields
                .into_iter()
                .map(|(k, v)| (Value::BulkString(Some(k.as_bytes().to_vec())), v))
                .collect(),
        )
        .serialize()
    } else {
        // RESP2 has no map type; Redis flattens to alternating key/value.
        let mut flat = Vec::with_capacity(fields.len() * 2);
        for (k, v) in fields {
            flat.push(Value::BulkString(Some(k.as_bytes().to_vec())));
            flat.push(v);
        }
        Value::Array(Some(flat)).serialize()
    }
}

// ── INFO ─────────────────────────────────────────────────────────────────────

/// Handles an AUTH attempt. Returns `(disconnect, resp_bytes)`.
///
/// `disconnect` is true when the failure count hits MAX_AUTH_FAILURES.
pub(crate) fn process_auth(
    provided: &str,
    expected: &Arc<Option<String>>,
    is_authenticated: &mut bool,
    failures: &mut u32,
) -> (bool, Vec<u8>) {
    match expected.as_ref() {
        // Constant-time compare so a network attacker can't recover the password
        // byte-by-byte from response-timing differences.
        Some(pwd) if ct_eq_bytes(provided.as_bytes(), pwd.as_bytes()) => {
            *is_authenticated = true;
            *failures = 0;
            (false, b"+OK\r\n".to_vec())
        }
        Some(_) => {
            *failures += 1;
            if *failures >= MAX_AUTH_FAILURES {
                (true, b"-ERR too many authentication failures\r\n".to_vec())
            } else {
                (false, b"-ERR invalid password\r\n".to_vec())
            }
        }
        None => (
            false,
            b"-ERR Client sent AUTH, but no password is set\r\n".to_vec(),
        ),
    }
}

// ── main ─────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_tcp<S>(
    socket: S,
    store: Arc<KeyValueStore>,
    tx: broadcast::Sender<SyncMsg>,
    password: Arc<Option<String>>,
    pubsub: SharedPubSub,
    watch_registry: WatchRegistry,
    state: Arc<ServerState>,
    peer: String,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let conn_id = next_conn_id();
    let mut client_meta = ClientMeta::new(
        conn_id,
        peer,
        format!("127.0.0.1:{}", server_facts().tcp_port),
    );
    let _guard = ConnectionGuard::new("tcp", client_meta.clone());
    let (mut reader, raw_writer) = tokio::io::split(socket);
    let mut writer = tokio::io::BufWriter::with_capacity(32 * 1024, raw_writer);
    let mut buf = Vec::<u8>::new();
    let mut read_pos: usize = 0;
    // Bytes the in-flight frame needs before another parse attempt can
    // possibly succeed; 0 means "try now". See the gate in the read arm.
    let mut need: usize = 0;
    let mut read_buf = [0u8; TCP_READ_BUFFER_BYTES];
    // Reused for every response on this connection — avoids a Vec allocation
    // per command (significant under pipelining).
    let mut resp_buf = Vec::<u8>::with_capacity(4 * 1024);
    let mut is_authenticated = password.is_none();
    let mut auth_failures: u32 = 0;
    // RESP2 until the client negotiates otherwise. Defaulting to 2 keeps every
    // existing client working: they never send HELLO and must not start
    // receiving RESP3-only types.
    let mut protover: u8 = 2;
    let mut multi_queue: Option<Vec<Command>> = None;
    // Set when a command inside the open MULTI could not be queued. `EXEC` then
    // refuses the whole transaction — see `EXECABORT`.
    let mut multi_dirty = false;
    let mut subscribed_channels: HashSet<String> = HashSet::new();
    let mut subscribed_patterns: HashSet<String> = HashSet::new();
    let (overflow_tx, mut overflow_rx) = mpsc::channel(1);
    let (ps_tx, mut ps_rx) = pubsub_channel(overflow_tx.clone());
    // WATCH state for optimistic-lock transactions over TCP. Unlike the WS
    // handler, TCP clients are not sent keychange pushes — WATCH is pure CAS.
    let mut watched_keys: HashSet<String> = HashSet::new();
    let mut watch_dirty = false;
    let (watch_subscriber, mut watch_rx) = notification_channel(conn_id, overflow_tx.clone());

    'outer: loop {
        let is_subscribed = !subscribed_channels.is_empty() || !subscribed_patterns.is_empty();
        // Republish only when the counts moved: CLIENT LIST has to see live
        // subscription state, but taking the registry's write lock once per
        // command would put it on the hot path.
        if client_meta.sub != subscribed_channels.len()
            || client_meta.psub != subscribed_patterns.len()
        {
            client_meta.sub = subscribed_channels.len();
            client_meta.psub = subscribed_patterns.len();
            publish_client(client_meta.clone());
        }

        tokio::select! {
            result = reader.read(&mut read_buf) => {
                match result {
                    Ok(0) => break,
                    Ok(n) => {
                        if (buf.len() - read_pos) + n > MAX_TCP_READ_BUFFER_BYTES {
                            warn!("TCP connection exceeded max buffer size, closing");
                            break 'outer;
                        }
                        buf.extend_from_slice(&read_buf[..n]);
                        // A frame that cannot possibly be complete is not worth
                        // re-parsing. `Value::parse` starts from the beginning
                        // every time, rebuilding — and reallocating — every
                        // bulk string it has already seen, so a large multi-bulk
                        // arriving over hundreds of segments used to re-copy
                        // everything received so far on each one. `need` is the
                        // parser's lower bound on the finished frame; until the
                        // buffer holds that much, skip the work entirely.
                        if buf.len() - read_pos < need {
                            continue 'outer;
                        }
                        'parse: loop {
                            // Completeness is decided by the non-allocating
                            // measure. `Value::parse` restarts from the first
                            // byte every call, so asking *it* whether a frame
                            // had arrived meant rebuilding — and reallocating —
                            // every element received so far, once per segment,
                            // and discarding all of it. `frame_len` walks the
                            // headers and steps over payloads; `parse` below
                            // then runs once, on a frame known to be whole.
                            match Value::frame_len(&buf[read_pos..]) {
                                Ok(_) => {}
                                Err(e) if e.is_incomplete() => {
                                    // Measured from `read_pos`, and compaction
                                    // moves that to 0, so the bound stays valid.
                                    need = e.needed();
                                    // Compact: drop already-parsed bytes.
                                    buf.drain(..read_pos);
                                    read_pos = 0;
                                    break 'parse;
                                }
                                Err(e) => {
                                    warn!("TCP protocol error: {}", e);
                                    let _ = writer.write_all(b"-ERR Protocol error\r\n").await;
                                    buf.clear();
                                    read_pos = 0;
                                    need = 0;
                                    break 'parse;
                                }
                            }
                            match Value::parse(&buf[read_pos..]) {
                                Ok((value, consumed)) => {
                                    read_pos += consumed;
                                    let cmd = match Command::from_value(value) {
                                        Ok(c) => c,
                                        Err(e) => {
                                            // A frame that will not parse (bad arity,
                                            // malformed argument) inside an open MULTI
                                            // poisons the transaction, as in Redis.
                                            if multi_queue.is_some() { multi_dirty = true; }
                                            let r = Value::Error(e).serialize();
                                            if writer.write_all(&r).await.is_err() { break 'outer; }
                                            continue 'parse;
                                        }
                                    };

                                    // AUTH is always processed immediately
                                    if let Command::Auth(ref pwd) = cmd {
                                        let (disconnect, resp) = process_auth(
                                            pwd, &password, &mut is_authenticated, &mut auth_failures,
                                        );
                                        if writer.write_all(&resp).await.is_err() { break 'outer; }
                                        if disconnect {
                                            let _ = writer.flush().await;
                                            break 'outer;
                                        }
                                        continue 'parse;
                                    }

                                    if let Command::Hello(ref requested) = cmd {
                                        let resp = process_hello(
                                            requested.as_deref(),
                                            &mut protover,
                                            is_authenticated,
                                            state.is_replica(),
                                        );
                                        if writer.write_all(&resp).await.is_err() { break 'outer; }
                                        // CLIENT LIST reports resp= per connection,
                                        // so a renegotiation has to reach the registry.
                                        if client_meta.resp != protover {
                                            client_meta.resp = protover;
                                            publish_client(client_meta.clone());
                                        }
                                        continue 'parse;
                                    }

                                    // QUIT is answered before the auth gate and
                                    // before the subscribe-mode gate, as in Redis:
                                    // a client that cannot authenticate, or is
                                    // parked in subscribe mode, still deserves a
                                    // clean close rather than a dropped socket.
                                    if matches!(cmd, Command::Quit) {
                                        let _ = writer.write_all(b"+OK\r\n").await;
                                        let _ = writer.flush().await;
                                        break 'outer;
                                    }

                                    if !is_authenticated {
                                        if writer.write_all(b"-NOAUTH Authentication required.\r\n").await.is_err() {
                                            break 'outer;
                                        }
                                        continue 'parse;
                                    }

                                    // ── Transactions ──────────────────────────────
                                    match &cmd {
                                        Command::Multi => {
                                            let resp = if multi_queue.is_some() {
                                                b"-ERR MULTI calls can not be nested\r\n".to_vec()
                                            } else {
                                                multi_queue = Some(Vec::new());
                                                multi_dirty = false;
                                                b"+OK\r\n".to_vec()
                                            };
                                            if writer.write_all(&resp).await.is_err() { break 'outer; }
                                            continue 'parse;
                                        }
                                        Command::Discard => {
                                            let resp = if multi_queue.take().is_some() {
                                                // DISCARD also flushes WATCH state.
                                                unregister_all_watches(&watch_registry, conn_id, &mut watched_keys).await;
                                                while watch_rx.try_recv().is_ok() {}
                                                watch_dirty = false;
                                                multi_dirty = false;
                                                b"+OK\r\n".to_vec()
                                            } else {
                                                b"-ERR DISCARD without MULTI\r\n".to_vec()
                                            };
                                            if writer.write_all(&resp).await.is_err() { break 'outer; }
                                            continue 'parse;
                                        }
                                        Command::Exec => {
                                            match multi_queue.take() {
                                                None => {
                                                    if writer.write_all(b"-ERR EXEC without MULTI\r\n").await.is_err() { break 'outer; }
                                                }
                                                // A command failed to queue: run nothing and
                                                // say so, rather than applying the rest.
                                                Some(_) if multi_dirty => {
                                                    multi_dirty = false;
                                                    unregister_all_watches(&watch_registry, conn_id, &mut watched_keys).await;
                                                    while watch_rx.try_recv().is_ok() {}
                                                    watch_dirty = false;
                                                    if writer.write_all(EXECABORT).await.is_err() { break 'outer; }
                                                }
                                                Some(queue) => {
                                                    if state.is_replica()
                                                        && queue.iter().any(is_write_command)
                                                    {
                                                        unregister_all_watches(
                                                            &watch_registry,
                                                            conn_id,
                                                            &mut watched_keys,
                                                        )
                                                        .await;
                                                        while watch_rx.try_recv().is_ok() {}
                                                        watch_dirty = false;
                                                        if writer
                                                            .write_all(b"-READONLY You can't write against a read only replica.\r\n")
                                                            .await
                                                            .is_err()
                                                        {
                                                            break 'outer;
                                                        }
                                                        continue 'parse;
                                                    }
                                                    if queue.iter().any(is_write_command)
                                                        && !state.persistence_is_healthy()
                                                    {
                                                        unregister_all_watches(
                                                            &watch_registry,
                                                            conn_id,
                                                            &mut watched_keys,
                                                        )
                                                        .await;
                                                        while watch_rx.try_recv().is_ok() {}
                                                        watch_dirty = false;
                                                        let error = Value::Error(
                                                            "MISCONF persistence is unhealthy; writes are disabled until SAVE succeeds"
                                                                .to_string(),
                                                        )
                                                        .serialize();
                                                        if writer.write_all(&error).await.is_err() {
                                                            break 'outer;
                                                        }
                                                        continue 'parse;
                                                    }
                                                    // Reserve both the queued write keys and WATCH
                                                    // keys before the CAS check. A conflicting write
                                                    // that was already in flight completes first and
                                                    // queues its notification; no new one can enter
                                                    // between this check and transaction execution.
                                                    let _write_guards = if store.has_capacity_limits() {
                                                        state.replicas.lock_all_writes().await
                                                    } else {
                                                        state
                                                            .replicas
                                                            .lock_commands_and_keys(
                                                                &queue,
                                                                watched_keys.iter().map(String::as_str),
                                                            )
                                                            .await
                                                    };
                                                    while watch_rx.try_recv().is_ok() {
                                                        watch_dirty = true;
                                                    }
                                                    if watch_dirty {
                                                        // A watched key changed since WATCH — abort with nil array.
                                                        unregister_all_watches(&watch_registry, conn_id, &mut watched_keys).await;
                                                        while watch_rx.try_recv().is_ok() {}
                                                        watch_dirty = false;
                                                        if writer.write_all(&Value::Array(None).serialize()).await.is_err() { break 'outer; }
                                                    } else {
                                                        let mut results = Vec::with_capacity(queue.len());
                                                        for qcmd in queue {
                                                            let resp = match qcmd {
                                                                // Delivery lives in the connection loop, not the
                                                                // store — `store.execute(Publish)` is a stub that
                                                                // answers 0 and sends nothing. Queuing PUBLISH
                                                                // without this arm would silently swallow the
                                                                // message, which is worse than refusing it.
                                                                Command::Publish(ref channel, ref message) => {
                                                                    let count = pubsub.lock().await.publish(channel, message);
                                                                    Value::Integer(count)
                                                                }
                                                                _ if is_write_command(&qcmd) => {
                                                                    let (resp, evicted) = execute_and_record_with_evictions(&store, qcmd.clone());
                                                                    apply_write_effects(&qcmd, &resp, &tx, 0, &state, &watch_registry, &store).await;
                                                                    apply_eviction_effects(&evicted, &tx, 0, &state, &watch_registry, &store).await;
                                                                    resp
                                                                }
                                                                _ => execute_and_record(&store, qcmd),
                                                            };
                                                            results.push(resp);
                                                        }
                                                        unregister_all_watches(&watch_registry, conn_id, &mut watched_keys).await;
                                                        while watch_rx.try_recv().is_ok() {}
                                                        watch_dirty = false;
                                                        let out = Value::Array(Some(results)).serialize();
                                                        if writer.write_all(&out).await.is_err() { break 'outer; }
                                                    }
                                                }
                                            }
                                            continue 'parse;
                                        }
                                        _ => {}
                                    }

                                    // If inside MULTI, queue the command
                                    if let Some(ref mut queue) = multi_queue {
                                        // Every branch that does not reach `+QUEUED` also
                                        // sets `multi_dirty`: the client asked for a
                                        // transaction containing this command, and it will
                                        // not be there, so running the remainder would
                                        // apply something the caller never asked for.
                                        //
                                        // Refused here rather than at EXEC — an unknown
                                        // verb must not sit in the queue looking accepted.
                                        let refusal = queue_time_rejection(&cmd).or_else(|| match &cmd {
                                            Command::Subscribe(_) | Command::Unsubscribe(_)
                                            | Command::PSubscribe(_) | Command::PUnsubscribe(_)
                                            | Command::Watch(_) | Command::Unwatch(_)
                                            | Command::QSub(_) | Command::QUnsub(_) => Some(
                                                b"-ERR Command not allowed inside a transaction\r\n".to_vec(),
                                            ),
                                            _ if queue.len() >= max_multi_queue_len() => Some(
                                                b"-ERR transaction queue limit reached\r\n".to_vec(),
                                            ),
                                            _ => None,
                                        });
                                        match refusal {
                                            Some(err) => {
                                                multi_dirty = true;
                                                if writer.write_all(&err).await.is_err() { break 'outer; }
                                            }
                                            None => {
                                                queue.push(cmd);
                                                if writer.write_all(b"+QUEUED\r\n").await.is_err() { break 'outer; }
                                            }
                                        }
                                        continue 'parse;
                                    }

                                    // ── Pub/Sub commands ──────────────────────────
                                    match cmd {
                                        Command::Subscribe(channels) => {
                                            for ch in channels {
                                                let is_new = !subscribed_channels.contains(&ch);
                                                if is_new
                                                    && subscribed_channels.len()
                                                        + subscribed_patterns.len()
                                                        >= max_pubsub_subscriptions_per_conn()
                                                {
                                                    if writer.write_all(b"-ERR pubsub subscription limit per connection reached\r\n").await.is_err() { break 'outer; }
                                                    continue;
                                                }
                                                if subscribed_channels.insert(ch.clone()) {
                                                    pubsub.lock().await.subscribe(conn_id, &ch, ps_tx.clone());
                                                }
                                                let count = subscribed_channels.len() + subscribed_patterns.len();
                                                let ack = resp_subscribe_ack("subscribe", &ch, count);
                                                if writer.write_all(&ack).await.is_err() { break 'outer; }
                                            }
                                        }
                                        Command::Unsubscribe(channels) => {
                                            let targets: Vec<String> = if channels.is_empty() {
                                                subscribed_channels.drain().collect()
                                            } else {
                                                channels.into_iter().filter(|c| subscribed_channels.remove(c)).collect()
                                            };
                                            for ch in &targets {
                                                pubsub.lock().await.unsubscribe(conn_id, ch);
                                                let count = subscribed_channels.len() + subscribed_patterns.len();
                                                let ack = resp_subscribe_ack("unsubscribe", ch, count);
                                                if writer.write_all(&ack).await.is_err() { break 'outer; }
                                            }
                                            if targets.is_empty() {
                                                let ack = resp_subscribe_ack("unsubscribe", "", 0);
                                                if writer.write_all(&ack).await.is_err() { break 'outer; }
                                            }
                                        }
                                        Command::PSubscribe(patterns) => {
                                            for pat in patterns {
                                                let is_new = !subscribed_patterns.contains(&pat);
                                                if is_new
                                                    && subscribed_channels.len()
                                                        + subscribed_patterns.len()
                                                        >= max_pubsub_subscriptions_per_conn()
                                                {
                                                    if writer.write_all(b"-ERR pubsub subscription limit per connection reached\r\n").await.is_err() { break 'outer; }
                                                    continue;
                                                }
                                                if subscribed_patterns.insert(pat.clone()) {
                                                    pubsub.lock().await.psubscribe(conn_id, &pat, ps_tx.clone());
                                                }
                                                let count = subscribed_channels.len() + subscribed_patterns.len();
                                                let ack = resp_subscribe_ack("psubscribe", &pat, count);
                                                if writer.write_all(&ack).await.is_err() { break 'outer; }
                                            }
                                        }
                                        Command::PUnsubscribe(patterns) => {
                                            let targets: Vec<String> = if patterns.is_empty() {
                                                subscribed_patterns.drain().collect()
                                            } else {
                                                patterns.into_iter().filter(|p| subscribed_patterns.remove(p)).collect()
                                            };
                                            for pat in &targets {
                                                pubsub.lock().await.punsubscribe(conn_id, pat);
                                                let count = subscribed_channels.len() + subscribed_patterns.len();
                                                let ack = resp_subscribe_ack("punsubscribe", pat, count);
                                                if writer.write_all(&ack).await.is_err() { break 'outer; }
                                            }
                                            if targets.is_empty() {
                                                let ack = resp_subscribe_ack("punsubscribe", "", 0);
                                                if writer.write_all(&ack).await.is_err() { break 'outer; }
                                            }
                                        }
                                        Command::Publish(channel, message) => {
                                            let count = pubsub.lock().await.publish(&channel, &message);
                                            let resp = Value::Integer(count).serialize();
                                            if writer.write_all(&resp).await.is_err() { break 'outer; }
                                        }

                                        Command::Watch(keys) => {
                                            let new_count = keys.iter().filter(|k| !watched_keys.contains(*k)).count();
                                            if watched_keys.len() + new_count > max_watches_per_conn() {
                                                if writer.write_all(b"-ERR watch limit per connection reached\r\n").await.is_err() { break 'outer; }
                                            } else {
                                                {
                                                    let mut reg = watch_registry.map.lock().await;
                                                    for key in &keys {
                                                        if watched_keys.insert(key.clone()) {
                                                            reg.entry(key.clone()).or_default().push(watch_subscriber.clone());
                                                        }
                                                    }
                                                    watch_registry.sync_len(&reg);
                                                }
                                                if writer.write_all(b"+OK\r\n").await.is_err() { break 'outer; }
                                            }
                                        }
                                        Command::Unwatch(keys) => {
                                            let targets: Vec<String> = if keys.is_empty() {
                                                watched_keys.drain().collect()
                                            } else {
                                                keys.into_iter().filter(|k| watched_keys.remove(k)).collect()
                                            };
                                            {
                                                let mut reg = watch_registry.map.lock().await;
                                                for key in &targets {
                                                    if let Some(subs) = reg.get_mut(key) {
                                                        subs.retain(|sub| sub.conn_id != conn_id);
                                                        if subs.is_empty() { reg.remove(key); }
                                                    }
                                                }
                                                watch_registry.sync_len(&reg);
                                            }
                                            if watched_keys.is_empty() {
                                                while watch_rx.try_recv().is_ok() {}
                                                watch_dirty = false;
                                            }
                                            if writer.write_all(b"+OK\r\n").await.is_err() { break 'outer; }
                                        }

                                        cmd => {
                                            // In subscribe mode only ping is allowed
                                            if is_subscribed && !matches!(cmd, Command::Ping(_)) {
                                                let err = b"-ERR only (P)SUBSCRIBE / (P)UNSUBSCRIBE / PING / QUIT allowed in subscribe mode\r\n";
                                                if writer.write_all(err).await.is_err() { break 'outer; }
                                                continue 'parse;
                                            }
                                            // Replica: reject writes
                                            if state.is_replica() && is_write_command(&cmd) {
                                                let err = b"-READONLY You can't write against a read only replica.\r\n";
                                                if writer.write_all(err).await.is_err() { break 'outer; }
                                                continue 'parse;
                                            }
                                            // Snapshot commands — handled here (async I/O, not in execute())
                                            match &cmd {
                                                Command::Save => {
                                                    let response = match state.save(&store).await {
                                                        Ok(()) => b"+OK\r\n".to_vec(),
                                                        Err(e) => Value::Error(format!(
                                                            "MISCONF snapshot failed: {e}"
                                                        ))
                                                        .serialize(),
                                                    };
                                                    if writer.write_all(&response).await.is_err() { break 'outer; }
                                                    continue 'parse;
                                                }
                                                Command::BgSave => {
                                                    let s = Arc::clone(&store);
                                                    let st = Arc::clone(&state);
                                                    tokio::spawn(async move {
                                                        let _ = st.save(&s).await;
                                                    });
                                                    if writer.write_all(b"+Background saving started\r\n").await.is_err() { break 'outer; }
                                                    continue 'parse;
                                                }
                                                Command::LastSave => {
                                                    let ts = state.snap.last_save.load(Ordering::Relaxed);
                                                    if writer.write_all(&Value::Integer(ts).serialize()).await.is_err() { break 'outer; }
                                                    continue 'parse;
                                                }
                                                Command::Info(sections) => {
                                                    let repl = ReplInfo {
                                                        connected: state.replicas.count.load(Ordering::Relaxed),
                                                        queue_depth: state.replicas.max_queue_depth().await,
                                                        lag_frames: state.replicas.max_lag_frames().await,
                                                    };
                                                    let body = render_info(
                                                        sections,
                                                        server_facts(),
                                                        &store,
                                                        sampled_keyspace(&store),
                                                        state.is_replica(),
                                                        repl,
                                                        state.snap.last_save.load(Ordering::Relaxed),
                                                        watch_registry.watched_patterns.load(Ordering::Relaxed) as u64,
                                                        watch_registry.watched_keys.load(Ordering::Relaxed) as u64,
                                                    );
                                                    let resp = Value::BulkString(Some(body.into_bytes())).serialize();
                                                    if writer.write_all(&resp).await.is_err() { break 'outer; }
                                                    continue 'parse;
                                                }
                                                Command::Client(args) => {
                                                    let resp = handle_client_command(args, &mut client_meta).serialize();
                                                    if writer.write_all(&resp).await.is_err() { break 'outer; }
                                                    continue 'parse;
                                                }
                                                Command::Config(args) => {
                                                    let resp = handle_config_command(args, server_facts(), &store).serialize();
                                                    if writer.write_all(&resp).await.is_err() { break 'outer; }
                                                    continue 'parse;
                                                }
                                                Command::CommandQuery(args) => {
                                                    let resp = handle_command_query(args, protover).serialize();
                                                    if writer.write_all(&resp).await.is_err() { break 'outer; }
                                                    continue 'parse;
                                                }
                                                Command::Cluster(args) => {
                                                    let resp = handle_cluster_command(args).serialize();
                                                    if writer.write_all(&resp).await.is_err() { break 'outer; }
                                                    continue 'parse;
                                                }
                                                Command::Module(args) => {
                                                    let resp = handle_module_command(args).serialize();
                                                    if writer.write_all(&resp).await.is_err() { break 'outer; }
                                                    continue 'parse;
                                                }
                                                Command::Memory(args) => {
                                                    let resp = handle_memory_command(args).serialize();
                                                    if writer.write_all(&resp).await.is_err() { break 'outer; }
                                                    continue 'parse;
                                                }
                                                Command::PubSub(args) => {
                                                    let resp = handle_pubsub_command(args, &*pubsub.lock().await).serialize();
                                                    if writer.write_all(&resp).await.is_err() { break 'outer; }
                                                    continue 'parse;
                                                }
                                                Command::ReplicaOfNoOne => {
                                                    state.promote_to_primary();
                                                    if writer.write_all(b"+OK\r\n").await.is_err() { break 'outer; }
                                                    continue 'parse;
                                                }
                                                _ => {}
                                            }
                                            let response = if is_write_command(&cmd) {
                                                execute_ordered_write(
                                                    &cmd,
                                                    &tx,
                                                    0,
                                                    &state,
                                                    &watch_registry,
                                                    &store,
                                                )
                                                .await
                                            } else {
                                                execute_and_record(&store, cmd)
                                            };
                                            resp_buf.clear();
                                            response.serialize_into(&mut resp_buf);
                                            if writer.write_all(&resp_buf).await.is_err() {
                                                break 'outer;
                                            }
                                        }
                                    }
                                }
                                Err(e) if e.is_incomplete() => {
                                    // Measured from `read_pos`, and compaction
                                    // moves that to 0, so the bound stays valid.
                                    need = e.needed();
                                    // Compact: drop already-parsed bytes, reset cursor.
                                    buf.drain(..read_pos);
                                    read_pos = 0;
                                    break 'parse;
                                }
                                Err(e) => {
                                    warn!("TCP protocol error: {}", e);
                                    let _ = writer.write_all(b"-ERR Protocol error\r\n").await;
                                    buf.clear();
                                    read_pos = 0;
                                    need = 0;
                                    break 'parse;
                                }
                            }
                        }
                        // Flush all responses for this read batch in one syscall.
                        if writer.flush().await.is_err() {
                            break 'outer;
                        }
                    }
                    Err(e) => {
                        warn!("TCP read error: {}", e);
                        break;
                    }
                }
            }

            msg = ps_rx.recv(), if is_subscribed => {
                match msg {
                    Some(m) => {
                        if writer
                            .write_all(&encode_pubsub_msg(&m.message, protover))
                            .await
                            .is_err()
                        {
                            break;
                        }
                        // `writer` is a BufWriter, and a delivery is not a
                        // response to anything this connection sent — nothing
                        // else is going to flush it. Without this a subscriber
                        // that only listens receives nothing until it happens
                        // to send a command or 32 KB of pushes accumulate.
                        if writer.flush().await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }

            // A watched key changed: mark the transaction dirty so a following
            // EXEC aborts. TCP clients get no keychange push (WATCH is pure CAS).
            notif = watch_rx.recv(), if !watched_keys.is_empty() => {
                if notif.is_some() {
                    watch_dirty = true;
                }
            }

            overflow = overflow_rx.recv() => {
                if overflow.is_some() {
                    warn!("TCP conn {} notification queue overflowed; disconnecting", conn_id);
                }
                break;
            }
        }
    }

    if !subscribed_channels.is_empty() || !subscribed_patterns.is_empty() {
        pubsub.lock().await.unsubscribe_all(conn_id);
    }
    unregister_all_watches(&watch_registry, conn_id, &mut watched_keys).await;
}

// ── WebSocket handler ─────────────────────────────────────────────────────────

/// Complete the WebSocket handshake, enforcing the origin allowlist and a
/// deadline. Returns `None` when the connection was refused, failed, or stalled
/// — in every case the caller simply drops the socket and its permit.
///
/// Split out from `handle_ws` so both the origin decision and the timeout are
/// reachable from a test without standing up a listener.
///
/// `result_large_err`: the error type is tungstenite's `ErrorResponse`, which is
/// an `http::Response` — its size is the handshake callback's signature, not
/// ours, and boxing it would not satisfy the trait.
#[allow(clippy::result_large_err)]
pub(crate) async fn ws_handshake<S>(
    socket: S,
    allowed_origins: Option<&[String]>,
    timeout: Duration,
    conn_id: u64,
) -> Option<tokio_tungstenite::WebSocketStream<S>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let check_origin = |req: &HandshakeRequest,
                        resp: HandshakeResponse|
     -> Result<HandshakeResponse, ErrorResponse> {
        let origin = req
            .headers()
            .get("origin")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        if origin_allowed(allowed_origins, origin.as_deref()) {
            return Ok(resp);
        }
        warn!(
            "WS conn {}: refused Origin {:?} — not in RECACHED_ALLOWED_ORIGINS",
            conn_id, origin
        );
        let mut err = ErrorResponse::new(Some(
            "Origin not allowed. Add it to RECACHED_ALLOWED_ORIGINS to permit this page."
                .to_string(),
        ));
        *err.status_mut() = StatusCode::FORBIDDEN;
        Err(err)
    };

    // Bound reassembly before it happens. Without these, tungstenite's 64 MiB
    // message / 16 MiB frame defaults apply, and a client that never finishes a
    // fragmented message holds that memory having sent no command at all — so
    // neither the RESP parser's limits nor the `NOAUTH` check are reached.
    let config = WebSocketConfig::default()
        .max_message_size(Some(ws_max_message_bytes()))
        .max_frame_size(Some(ws_max_frame_bytes()));

    match tokio::time::timeout(
        timeout,
        accept_hdr_async_with_config(socket, check_origin, Some(config)),
    )
    .await
    {
        Ok(Ok(ws)) => Some(ws),
        Ok(Err(e)) => {
            warn!("WS handshake failed on conn {}: {}", conn_id, e);
            None
        }
        Err(_) => {
            debug!("WS handshake on conn {} timed out", conn_id);
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_ws<S>(
    socket: S,
    store: Arc<KeyValueStore>,
    tx: broadcast::Sender<SyncMsg>,
    password: Arc<Option<String>>,
    conn_id: u64,
    pubsub: SharedPubSub,
    watch_registry: WatchRegistry,
    state: Arc<ServerState>,
    sync_secret: Arc<Option<String>>,
    allowed_origins: Arc<Option<Vec<String>>>,
    peer: String,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let mut client_meta = ClientMeta::new(
        conn_id,
        peer,
        format!("127.0.0.1:{}", server_facts().ws_port),
    );
    // The WebSocket transport speaks RESP3 from the first frame.
    client_meta.resp = 3;
    let _guard = ConnectionGuard::new("ws", client_meta.clone());
    let Some(ws_stream) = ws_handshake(
        socket,
        allowed_origins.as_deref(),
        handshake_timeout(),
        conn_id,
    )
    .await
    else {
        return;
    };

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mut rx = tx.subscribe();
    let mut is_authenticated = password.is_none();
    let mut auth_failures: u32 = 0;
    let mut multi_queue: Option<Vec<Command>> = None;
    // Set when a command inside the open MULTI could not be queued. `EXEC` then
    // refuses the whole transaction — see `EXECABORT`.
    let mut multi_dirty = false;
    let mut subscribed_channels: HashSet<String> = HashSet::new();
    let mut subscribed_patterns: HashSet<String> = HashSet::new();
    let (overflow_tx, mut overflow_rx) = mpsc::channel(1);
    let (ps_tx, mut ps_rx) = pubsub_channel(overflow_tx.clone());
    let mut watched_keys: HashSet<String> = HashSet::new();
    // Set when any watched key changes; EXEC aborts (returns nil) if true.
    let mut watch_dirty = false;
    let (watch_subscriber, mut watch_rx) = notification_channel(conn_id, overflow_tx.clone());
    // Sync scopes for this connection. `strict` (RECACHED_SYNC_SECRET set)
    // means: no pushes and no key commands until a signed token is presented.
    // Without a secret, scopes are an opt-in bandwidth filter (legacy fan-out
    // of everything when None).
    let strict = sync_secret.is_some();
    let mut sync_scopes: Option<Vec<Grant>> = None;
    // Live-query subscriptions (QSUB). Keychange notifications for matching
    // keys arrive on their own channel so they never dirty WATCH transactions.
    let mut qsub_patterns: HashSet<String> = HashSet::new();
    let (qsub_subscriber, mut q_rx) = notification_channel(conn_id, overflow_tx);

    // Replies go out as *text* frames whenever the RESP bytes are valid UTF-8,
    // which is the overwhelming majority and is what every existing client
    // expects. A reply carrying a value that is not valid UTF-8 goes out as a
    // *binary* frame instead of being mangled by a lossy conversion, which is
    // what made raw binary values round-trip only over the TCP port.
    macro_rules! ws_send {
        ($bytes:expr) => {{
            let bytes: &[u8] = $bytes;
            let msg = match std::str::from_utf8(bytes) {
                Ok(text) => Message::Text(text.into()),
                Err(_) => Message::Binary(bytes.to_vec().into()),
            };
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }};
    }

    'outer: loop {
        let is_subscribed = !subscribed_channels.is_empty() || !subscribed_patterns.is_empty();

        tokio::select! {
            msg = ws_receiver.next() => {
                match msg {
                    // Binary frames carry the same RESP bytes as text frames.
                    // They exist so a client can write a value that is not
                    // valid UTF-8 — impossible over a text frame, which the
                    // WebSocket spec requires to be well-formed UTF-8.
                    Some(Ok(frame @ (Message::Text(_) | Message::Binary(_)))) => {
                        let raw: Vec<u8> = match &frame {
                            Message::Text(t) => t.as_bytes().to_vec(),
                            Message::Binary(b) => b.to_vec(),
                            _ => unreachable!("pattern restricts to text and binary"),
                        };
                        let (value, _) = match Value::parse(&raw) {
                            Ok(v) => v,
                            Err(e) => {
                                let err = Value::Error(format!("ERR Protocol error: {}", e)).serialize();
                                ws_send!(&err);
                                continue;
                            }
                        };

                        let cmd = match Command::from_value(value) {
                            Ok(c) => c,
                            Err(e) => {
                                // A frame that will not parse inside an open MULTI
                                // poisons the transaction, as in Redis.
                                if multi_queue.is_some() { multi_dirty = true; }
                                let err = Value::Error(e).serialize();
                                ws_send!(&err);
                                continue;
                            }
                        };

                        // AUTH
                        if let Command::Auth(ref pwd) = cmd {
                            let (disconnect, resp) = process_auth(
                                pwd, &password, &mut is_authenticated, &mut auth_failures,
                            );
                            ws_send!(&resp);
                            if disconnect { break; }
                            continue;
                        }

                        // The WebSocket sync protocol is specified in terms of
                        // RESP3 push frames, so this transport is always RESP3
                        // and HELLO cannot downgrade it — a client asking for 2
                        // is refused rather than silently left on 3.
                        if let Command::Hello(ref requested) = cmd {
                            let mut ws_protover: u8 = 3;
                            let resp = match requested.as_deref() {
                                Some("2") => Value::Error(
                                    "NOPROTO the WebSocket transport requires RESP3".to_string(),
                                )
                                .serialize(),
                                other => process_hello(
                                    other,
                                    &mut ws_protover,
                                    is_authenticated,
                                    state.is_replica(),
                                ),
                            };
                            ws_send!(&resp);
                            continue;
                        }

                        if matches!(cmd, Command::Quit) {
                            ws_send!(b"+OK\r\n");
                            break 'outer;
                        }

                        if !is_authenticated {
                            let resp = Value::Error("NOAUTH Authentication required.".to_string()).serialize();
                            ws_send!(&resp);
                            continue;
                        }

                        // ── Sync scoping ──────────────────────────────────────
                        if let Command::Sync(ref args) = cmd {
                            let resp = handle_sync_command(args, (*sync_secret).as_deref(), &mut sync_scopes, conn_id);
                            ws_send!(&resp);
                            continue;
                        }
                        // Token-scoped mode: check every command against this
                        // connection's granted scopes before it runs (including
                        // commands about to be queued inside MULTI).
                        if strict {
                            match command_scope(&cmd) {
                                CommandScope::KeyLess => {}
                                CommandScope::Admin => {
                                    record_scope_denial("admin");
                                    ws_send!(b"-NOSCOPE keyspace-wide and administrative commands are not available on scoped WebSocket connections\r\n");
                                    continue;
                                }
                                CommandScope::Keys(keys) => {
                                    let Some(ref scopes) = sync_scopes else {
                                        record_scope_denial("no_token");
                                        ws_send!(b"-NOSCOPE send SYNC TOKEN <token> before issuing commands\r\n");
                                        continue;
                                    };
                                    // Two distinct denials: the key is outside
                                    // every grant, or it is inside a read-only
                                    // one and the command would write it. They
                                    // are reported apart — in the error and in
                                    // the metric — so a misconfigured grant is
                                    // distinguishable from a missing one.
                                    if let Some((denied, need)) = keys
                                        .iter()
                                        .find(|(k, need)| !scopes_allow(scopes, k, *need))
                                    {
                                        let err = Value::Error(if *need == Access::Write
                                            && scopes_allow(scopes, denied, Access::Read)
                                        {
                                            record_scope_denial("read_only");
                                            format!(
                                                "NOSCOPE key '{}' is read-only on this connection",
                                                denied
                                            )
                                        } else {
                                            record_scope_denial("out_of_scope");
                                            format!(
                                                "NOSCOPE key '{}' is outside this connection's sync scopes",
                                                denied
                                            )
                                        })
                                        .serialize();
                                        ws_send!(&err);
                                        continue;
                                    }
                                }
                            }
                        }

                        // ── Transactions ──────────────────────────────────────
                        match &cmd {
                            Command::Multi => {
                                let resp = if multi_queue.is_some() {
                                    b"-ERR MULTI calls can not be nested\r\n".to_vec()
                                } else {
                                    multi_queue = Some(Vec::new());
                                    multi_dirty = false;
                                    b"+OK\r\n".to_vec()
                                };
                                ws_send!(&resp);
                                continue;
                            }
                            Command::Discard => {
                                let resp = if multi_queue.take().is_some() {
                                    // DISCARD also flushes WATCH state.
                                    unregister_all_watches(&watch_registry, conn_id, &mut watched_keys).await;
                                    while watch_rx.try_recv().is_ok() {} // drop stale notifications
                                    watch_dirty = false;
                                    multi_dirty = false;
                                    b"+OK\r\n".to_vec()
                                } else {
                                    b"-ERR DISCARD without MULTI\r\n".to_vec()
                                };
                                ws_send!(&resp);
                                continue;
                            }
                            Command::Exec => {
                                match multi_queue.take() {
                                    None => {
                                        ws_send!(b"-ERR EXEC without MULTI\r\n");
                                    }
                                    // A command failed to queue: run nothing and
                                    // say so, rather than applying the rest.
                                    Some(_) if multi_dirty => {
                                        multi_dirty = false;
                                        unregister_all_watches(&watch_registry, conn_id, &mut watched_keys).await;
                                        while watch_rx.try_recv().is_ok() {}
                                        watch_dirty = false;
                                        ws_send!(EXECABORT);
                                    }
                                    Some(queue) => {
                                        if state.is_replica()
                                            && queue.iter().any(is_write_command)
                                        {
                                            unregister_all_watches(
                                                &watch_registry,
                                                conn_id,
                                                &mut watched_keys,
                                            )
                                            .await;
                                            while watch_rx.try_recv().is_ok() {}
                                            watch_dirty = false;
                                            ws_send!(b"-READONLY You can't write against a read only replica.\r\n");
                                            continue;
                                        }
                                        if queue.iter().any(is_write_command)
                                            && !state.persistence_is_healthy()
                                        {
                                            unregister_all_watches(
                                                &watch_registry,
                                                conn_id,
                                                &mut watched_keys,
                                            )
                                            .await;
                                            while watch_rx.try_recv().is_ok() {}
                                            watch_dirty = false;
                                            ws_send!(
                                                &Value::Error(
                                                    "MISCONF persistence is unhealthy; writes are disabled until SAVE succeeds"
                                                        .to_string(),
                                                )
                                                .serialize()
                                            );
                                            continue;
                                        }
                                        // Match the TCP path: reserve queued write keys and
                                        // WATCH keys before checking invalidation, so a write
                                        // cannot land in the check-to-EXEC gap.
                                        let _write_guards = if store.has_capacity_limits() {
                                            state.replicas.lock_all_writes().await
                                        } else {
                                            state
                                                .replicas
                                                .lock_commands_and_keys(
                                                    &queue,
                                                    watched_keys.iter().map(String::as_str),
                                                )
                                                .await
                                        };
                                        while watch_rx.try_recv().is_ok() {
                                            watch_dirty = true;
                                        }
                                        if watch_dirty {
                                            // A watched key changed since WATCH — abort: return
                                            // a nil array and run nothing (Redis CAS semantics).
                                            unregister_all_watches(&watch_registry, conn_id, &mut watched_keys).await;
                                            while watch_rx.try_recv().is_ok() {} // drop stale notifications
                                            watch_dirty = false;
                                            ws_send!(&Value::Array(None).serialize());
                                        } else {
                                            let mut results = Vec::with_capacity(queue.len());
                                            for qcmd in queue {
                                                let resp = match qcmd {
                                                    // See the TCP path: delivery lives here,
                                                    // not in the store.
                                                    Command::Publish(ref channel, ref message) => {
                                                        let count = pubsub.lock().await.publish(channel, message);
                                                        Value::Integer(count)
                                                    }
                                                    _ if is_write_command(&qcmd) => {
                                                        let (resp, evicted) = execute_and_record_with_evictions(&store, qcmd.clone());
                                                        apply_write_effects(&qcmd, &resp, &tx, conn_id, &state, &watch_registry, &store).await;
                                                        apply_eviction_effects(&evicted, &tx, conn_id, &state, &watch_registry, &store).await;
                                                        resp
                                                    }
                                                    _ => execute_and_record(&store, qcmd),
                                                };
                                                results.push(resp);
                                            }
                                            // EXEC always flushes WATCH state. Drain any
                                            // self-notifications the queued writes produced so
                                            // they can't dirty a later transaction.
                                            unregister_all_watches(&watch_registry, conn_id, &mut watched_keys).await;
                                            while watch_rx.try_recv().is_ok() {}
                                            watch_dirty = false;
                                            let out = Value::Array(Some(results)).serialize();
                                            ws_send!(&out);
                                        }
                                    }
                                }
                                continue;
                            }
                            _ => {}
                        }

                        // Queue if inside MULTI
                        if let Some(ref mut queue) = multi_queue {
                            // Mirrors the TCP path: anything that does not reach
                            // `+QUEUED` poisons the transaction so EXEC runs nothing.
                            let refusal = queue_time_rejection(&cmd).or_else(|| match &cmd {
                                Command::Subscribe(_) | Command::Unsubscribe(_)
                                | Command::PSubscribe(_) | Command::PUnsubscribe(_)
                                | Command::Watch(_) | Command::Unwatch(_)
                                | Command::QSub(_) | Command::QUnsub(_) => Some(
                                    b"-ERR Command not allowed inside a transaction\r\n".to_vec(),
                                ),
                                _ if queue.len() >= max_multi_queue_len() => Some(
                                    b"-ERR transaction queue limit reached\r\n".to_vec(),
                                ),
                                _ => None,
                            });
                            match refusal {
                                Some(err) => {
                                    multi_dirty = true;
                                    ws_send!(&err);
                                }
                                None => {
                                    queue.push(cmd);
                                    ws_send!(b"+QUEUED\r\n");
                                }
                            }
                            continue;
                        }

                        // ── Pub/Sub commands ──────────────────────────────────
                        match cmd {
                            Command::Subscribe(channels) => {
                                for ch in channels {
                                    let is_new = !subscribed_channels.contains(&ch);
                                    if is_new
                                        && subscribed_channels.len() + subscribed_patterns.len()
                                            >= max_pubsub_subscriptions_per_conn()
                                    {
                                        ws_send!(b"-ERR pubsub subscription limit per connection reached\r\n");
                                        continue;
                                    }
                                    if subscribed_channels.insert(ch.clone()) {
                                        pubsub.lock().await.subscribe(conn_id, &ch, ps_tx.clone());
                                    }
                                    let count = subscribed_channels.len() + subscribed_patterns.len();
                                    ws_send!(&resp_subscribe_ack("subscribe", &ch, count));
                                }
                            }
                            Command::Unsubscribe(channels) => {
                                let targets: Vec<String> = if channels.is_empty() {
                                    subscribed_channels.drain().collect()
                                } else {
                                    channels.into_iter().filter(|c| subscribed_channels.remove(c)).collect()
                                };
                                for ch in &targets {
                                    pubsub.lock().await.unsubscribe(conn_id, ch);
                                    let count = subscribed_channels.len() + subscribed_patterns.len();
                                    ws_send!(&resp_subscribe_ack("unsubscribe", ch, count));
                                }
                                if targets.is_empty() {
                                    ws_send!(&resp_subscribe_ack("unsubscribe", "", 0));
                                }
                            }
                            Command::PSubscribe(patterns) => {
                                for pat in patterns {
                                    let is_new = !subscribed_patterns.contains(&pat);
                                    if is_new
                                        && subscribed_channels.len() + subscribed_patterns.len()
                                            >= max_pubsub_subscriptions_per_conn()
                                    {
                                        ws_send!(b"-ERR pubsub subscription limit per connection reached\r\n");
                                        continue;
                                    }
                                    if subscribed_patterns.insert(pat.clone()) {
                                        pubsub.lock().await.psubscribe(conn_id, &pat, ps_tx.clone());
                                    }
                                    let count = subscribed_channels.len() + subscribed_patterns.len();
                                    ws_send!(&resp_subscribe_ack("psubscribe", &pat, count));
                                }
                            }
                            Command::PUnsubscribe(patterns) => {
                                let targets: Vec<String> = if patterns.is_empty() {
                                    subscribed_patterns.drain().collect()
                                } else {
                                    patterns.into_iter().filter(|p| subscribed_patterns.remove(p)).collect()
                                };
                                for pat in &targets {
                                    pubsub.lock().await.punsubscribe(conn_id, pat);
                                    let count = subscribed_channels.len() + subscribed_patterns.len();
                                    ws_send!(&resp_subscribe_ack("punsubscribe", pat, count));
                                }
                                if targets.is_empty() {
                                    ws_send!(&resp_subscribe_ack("punsubscribe", "", 0));
                                }
                            }
                            Command::Publish(channel, message) => {
                                let count = pubsub.lock().await.publish(&channel, &message);
                                ws_send!(&Value::Integer(count).serialize());
                            }

                            Command::Watch(keys) => {
                                let new_count = keys
                                    .iter()
                                    .filter(|k| !watched_keys.contains(*k))
                                    .count();
                                if watched_keys.len() + new_count > max_watches_per_conn() {
                                    ws_send!(b"-ERR watch limit per connection reached\r\n");
                                } else {
                                    {
                                        let mut reg = watch_registry.map.lock().await;
                                        for key in &keys {
                                            if watched_keys.insert(key.clone()) {
                                                reg.entry(key.clone())
                                                    .or_default()
                                                    .push(watch_subscriber.clone());
                                            }
                                        }
                                        watch_registry.sync_len(&reg);
                                    } // reg dropped before await
                                    ws_send!(b"+OK\r\n");
                                }
                            }
                            Command::Unwatch(keys) => {
                                let targets: Vec<String> = if keys.is_empty() {
                                    watched_keys.drain().collect()
                                } else {
                                    keys.into_iter().filter(|k| watched_keys.remove(k)).collect()
                                };
                                {
                                    let mut reg = watch_registry.map.lock().await;
                                    for key in &targets {
                                        if let Some(subs) = reg.get_mut(key) {
                                            subs.retain(|sub| sub.conn_id != conn_id);
                                            if subs.is_empty() {
                                                reg.remove(key);
                                            }
                                        }
                                    }
                                    watch_registry.sync_len(&reg);
                                }
                                // Once nothing is watched, clear the dirty flag and drop any
                                // queued notifications so a later WATCH/MULTI/EXEC starts clean.
                                if watched_keys.is_empty() {
                                    while watch_rx.try_recv().is_ok() {}
                                    watch_dirty = false;
                                }
                                ws_send!(b"+OK\r\n");
                            }

                            Command::QSub(pattern) => {
                                // Strict mode: the requested pattern must sit inside a
                                // granted scope. Matching the pattern text itself does not
                                // prove containment, so ambiguous wildcard grants are only
                                // accepted on exact equality.
                                if strict {
                                    // A live query only reads, so any grant
                                    // covering the pattern is enough.
                                    let allowed = sync_scopes.as_ref().is_some_and(|scopes| {
                                        scopes
                                            .iter()
                                            .any(|g| scope_covers_pattern(&g.pattern, &pattern))
                                    });
                                    if !allowed {
                                        record_scope_denial("qsub_pattern");
                                        ws_send!(b"-NOSCOPE pattern is outside this connection's sync scopes\r\n");
                                        continue 'outer;
                                    }
                                }
                                if !qsub_patterns.contains(&pattern)
                                    && qsub_patterns.len() >= max_qsubs_per_conn()
                                {
                                    ws_send!(b"-ERR live query limit per connection reached\r\n");
                                    continue 'outer;
                                }
                                // Snapshot and registration are one ordered step, but
                                // serialization and socket I/O happen after releasing all
                                // write barriers. A slow client therefore cannot pause
                                // unrelated writers.
                                let qstate = {
                                    let _snapshot_guards =
                                        state.replicas.lock_all_writes().await;
                                    match initial_qstate(
                                        &store,
                                        &pattern,
                                        max_qsub_initial_keys(),
                                    ) {
                                        Ok(values) => {
                                            if qsub_patterns.insert(pattern.clone()) {
                                                let mut pats =
                                                    watch_registry.patterns.lock().await;
                                                pats.entry(pattern.clone())
                                                    .or_default()
                                                    .push(qsub_subscriber.clone());
                                                watch_registry.sync_patterns_len(&pats);
                                            }
                                            Ok(values)
                                        }
                                        Err(limit) => Err(limit),
                                    }
                                };
                                let kvs = match qstate {
                                    Ok(values) => values,
                                    Err(limit) => {
                                        ws_send!(
                                            &Value::Error(format!(
                                                "ERR live query initial state exceeds {limit} keys; narrow the pattern"
                                            ))
                                            .serialize()
                                        );
                                        continue 'outer;
                                    }
                                };
                                // Tagged reply so clients can recognise it among
                                // interleaved frames: ["qstate", pattern, k, v, ...]
                                let mut items = Vec::with_capacity(kvs.len() * 2 + 2);
                                items.push(Value::BulkString(Some(b"qstate".to_vec())));
                                items.push(Value::BulkString(Some(pattern.clone().into_bytes())));
                                for (k, v) in kvs {
                                    items.push(Value::BulkString(Some(k.into_bytes())));
                                    items.push(v);
                                }
                                ws_send!(&Value::Array(Some(items)).serialize());
                            }
                            Command::QUnsub(pattern) => {
                                let targets: Vec<String> = match pattern {
                                    Some(p) => {
                                        if qsub_patterns.remove(&p) {
                                            vec![p]
                                        } else {
                                            vec![]
                                        }
                                    }
                                    None => qsub_patterns.drain().collect(),
                                };
                                if !targets.is_empty() {
                                    let mut pats = watch_registry.patterns.lock().await;
                                    for p in &targets {
                                        if let Some(subs) = pats.get_mut(p) {
                                            subs.retain(|sub| sub.conn_id != conn_id);
                                            if subs.is_empty() {
                                                pats.remove(p);
                                            }
                                        }
                                    }
                                    watch_registry.sync_patterns_len(&pats);
                                }
                                ws_send!(b"+OK\r\n");
                            }

                            cmd => {
                                if is_subscribed && !matches!(cmd, Command::Ping(_)) {
                                    ws_send!(b"-ERR only (P)SUBSCRIBE / (P)UNSUBSCRIBE / PING / QUIT allowed in subscribe mode\r\n");
                                    continue 'outer;
                                }
                                // Replica: reject writes
                                if state.is_replica() && is_write_command(&cmd) {
                                    ws_send!(b"-READONLY You can't write against a read only replica.\r\n");
                                    continue 'outer;
                                }
                                // Snapshot commands
                                match &cmd {
                                    Command::Save => {
                                        let response = match state.save(&store).await {
                                            Ok(()) => b"+OK\r\n".to_vec(),
                                            Err(e) => Value::Error(format!(
                                                "MISCONF snapshot failed: {e}"
                                            ))
                                            .serialize(),
                                        };
                                        ws_send!(&response);
                                        continue 'outer;
                                    }
                                    Command::BgSave => {
                                        let s = Arc::clone(&store);
                                        let st = Arc::clone(&state);
                                        tokio::spawn(async move {
                                            let _ = st.save(&s).await;
                                        });
                                        ws_send!(b"+Background saving started\r\n");
                                        continue 'outer;
                                    }
                                    Command::LastSave => {
                                        let ts = state.snap.last_save.load(Ordering::Relaxed);
                                        ws_send!(&Value::Integer(ts).serialize());
                                        continue 'outer;
                                    }
                                    Command::Info(sections) => {
                                        let repl = ReplInfo {
                                            connected: state.replicas.count.load(Ordering::Relaxed),
                                            queue_depth: state.replicas.max_queue_depth().await,
                                            lag_frames: state.replicas.max_lag_frames().await,
                                        };
                                        let body = render_info(
                                            sections,
                                            server_facts(),
                                            &store,
                                            sampled_keyspace(&store),
                                            state.is_replica(),
                                            repl,
                                            state.snap.last_save.load(Ordering::Relaxed),
                                            watch_registry.watched_patterns.load(Ordering::Relaxed) as u64,
                                            watch_registry.watched_keys.load(Ordering::Relaxed) as u64,
                                        );
                                        ws_send!(&Value::BulkString(Some(body.into_bytes())).serialize());
                                        continue 'outer;
                                    }
                                    Command::Client(args) => {
                                        ws_send!(&handle_client_command(args, &mut client_meta).serialize());
                                        continue 'outer;
                                    }
                                    Command::Config(args) => {
                                        ws_send!(&handle_config_command(args, server_facts(), &store).serialize());
                                        continue 'outer;
                                    }
                                    Command::CommandQuery(args) => {
                                        // The WebSocket transport is RESP3-only,
                                        // so the catalog always replies as a map.
                                        ws_send!(&handle_command_query(args, 3).serialize());
                                        continue 'outer;
                                    }
                                    Command::Cluster(args) => {
                                        ws_send!(&handle_cluster_command(args).serialize());
                                        continue 'outer;
                                    }
                                    Command::Module(args) => {
                                        ws_send!(&handle_module_command(args).serialize());
                                        continue 'outer;
                                    }
                                    Command::Memory(args) => {
                                        ws_send!(&handle_memory_command(args).serialize());
                                        continue 'outer;
                                    }
                                    Command::PubSub(args) => {
                                        ws_send!(&handle_pubsub_command(args, &*pubsub.lock().await).serialize());
                                        continue 'outer;
                                    }
                                    Command::ReplicaOfNoOne => {
                                        state.promote_to_primary();
                                        ws_send!(b"+OK\r\n");
                                        continue 'outer;
                                    }
                                    _ => {}
                                }
                                // Ephemeral keys are owned by the connection that wrote
                                // them until another claims them; the close handler deletes
                                // whatever is still ours. Claimed outside the write-effects
                                // branch below, which only runs when a peer, replica, AOF or
                                // watcher is present — ownership must be recorded even on a
                                // standalone server with no listeners.
                                let response = if is_write_command(&cmd) {
                                    execute_ordered_write(
                                        &cmd,
                                        &tx,
                                        conn_id,
                                        &state,
                                        &watch_registry,
                                        &store,
                                    )
                                    .await
                                } else {
                                    execute_and_record(&store, cmd)
                                };
                                ws_send!(&response.serialize());
                            }
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        warn!("WS error on conn {}: {}", conn_id, e);
                        break;
                    }
                    None => break,
                }
            }

            result = rx.recv() => {
                match result {
                    Ok(push) if push.origin != conn_id => {
                        // Scope filter: with scopes set, only matching keys are
                        // forwarded. Without scopes, legacy mode forwards
                        // everything; strict mode forwards nothing until a
                        // token has been presented.
                        let visible = match &sync_scopes {
                            Some(scopes) => scopes_match(scopes, &push.keys),
                            None => !strict,
                        };
                        if visible {
                            ws_send!(&push.resp);
                        }
                    }
                    Ok(_) => {}
                    // The fan-out overran this connection. Resubscribing here
                    // would keep the socket open having silently skipped `n`
                    // mutations: `SyncPush` carries no sequence number, so the
                    // client cannot detect the gap, and its local replica stays
                    // wrong for as long as the page is open. There is no way to
                    // resynchronise in place — the same reasoning `SyncClient`
                    // already applies to an unparseable frame — so close the
                    // socket and let the client reconnect, which replays AUTH,
                    // SYNC TOKEN and every QSUB and returns a fresh `qstate`.
                    //
                    // This matches what the live-query path already does when
                    // its notification queue overflows.
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(
                            "WS conn {} lagged, missed {} mutations; closing so the client resynchronises",
                            conn_id, n
                        );
                        counter!("recached_sync_lag_disconnects_total").increment(1);
                        let _ = ws_sender
                            .send(Message::Close(Some(CloseFrame {
                                code: CloseCode::Library(WS_CLOSE_SYNC_GAP),
                                reason: "sync stream gap; reconnect to resynchronise".into(),
                            })))
                            .await;
                        break;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }

            msg = ps_rx.recv(), if is_subscribed => {
                match msg {
                    Some(m) => {
                        let bytes = encode_pubsub_msg(&m.message, 3);
                        ws_send!(&bytes);
                    }
                    None => break,
                }
            }

            notif = watch_rx.recv(), if !watched_keys.is_empty() => {
                if let Some(notif) = notif {
                    // A watched key changed: mark the transaction dirty (so a
                    // following EXEC aborts) and still push the keychange to the
                    // client for the observable-keys feature.
                    watch_dirty = true;
                    let bytes = encode_keychange(&notif.key, &notif.value);
                    ws_send!(&bytes);
                }
            }

            // Live-query keychange: same frame as WATCH pushes, but never
            // dirties transactions.
            notif = q_rx.recv(), if !qsub_patterns.is_empty() => {
                if let Some(notif) = notif {
                    let bytes = encode_keychange(&notif.key, &notif.value);
                    ws_send!(&bytes);
                }
            }

            overflow = overflow_rx.recv() => {
                if overflow.is_some() {
                    warn!("WS conn {} notification queue overflowed; disconnecting", conn_id);
                }
                break;
            }
        }
    }

    if !subscribed_channels.is_empty() || !subscribed_patterns.is_empty() {
        pubsub.lock().await.unsubscribe_all(conn_id);
    }
    if !watched_keys.is_empty() {
        let mut reg = watch_registry.map.lock().await;
        for key in &watched_keys {
            if let Some(subs) = reg.get_mut(key) {
                subs.retain(|sub| sub.conn_id != conn_id);
                if subs.is_empty() {
                    reg.remove(key);
                }
            }
        }
        watch_registry.sync_len(&reg);
    }
    unregister_all_qsubs(&watch_registry, conn_id, &mut qsub_patterns).await;

    // Delete ephemeral keys this connection still owns and fan the deletions
    // out, so every subscriber sees the peer go away immediately rather than
    // waiting for a heartbeat TTL to lapse.
    let expired = state.take_ephemeral_for(conn_id);
    if !expired.is_empty() {
        let del = Command::Del(expired);
        execute_ordered_write(&del, &tx, conn_id, &state, &watch_registry, &store).await;
    }
}
