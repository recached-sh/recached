//! End-to-end tests for scope-aware push routing (v0.6 Phase 6J-A).
//!
//! Real WebSocket connections against a real server. Routing decides which
//! connections a push is *offered* to; each connection's own scope check
//! still decides what it is sent. These tests pin both halves: unrelated
//! connections are not offered pushes, and nothing a connection may see is
//! lost — across token changes, registration races, multi-key writes from
//! principals that span tenants, and disconnects.

use super::*;

const SECRET: &str = "sync-routing-test-secret-0123456789abcdef";

fn token_for(entries: &str, sub: &str) -> String {
    mint_sync_token(
        SECRET,
        &format!("{entries}|{}|sub={sub}", unix_now_secs() + 3600),
    )
}

async fn strict_server() -> TestServer {
    spawn_ws_server_policy(SyncPolicy::strict(SECRET), None, None).await
}

async fn scoped(server: &TestServer, entries: &str, sub: &str) -> WsClient {
    let mut c = WsClient::connect(server.tcp_addr).await;
    let reply = c.cmd(&["SYNC", "TOKEN", &token_for(entries, sub)]).await;
    assert!(
        matches!(reply, Value::Array(Some(_))),
        "token refused: {reply:?}"
    );
    c
}

/// The bulk-string parts of a push frame.
fn push_parts(raw: &str) -> Vec<String> {
    match Value::parse(raw.as_bytes()) {
        Ok((Value::Push(items), _)) => items
            .into_iter()
            .filter_map(|v| match v {
                Value::BulkString(Some(b)) => Some(String::from_utf8_lossy(&b).into_owned()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

impl WsClient {
    /// Send a command and return its reply plus every push frame that
    /// arrived before it, in order. `cmd` discards those pushes.
    async fn cmd_collecting(&mut self, args: &[&str]) -> (Value, Vec<Vec<String>>) {
        let mut req = format!("*{}\r\n", args.len());
        for a in args {
            req.push_str(&format!("${}\r\n{}\r\n", a.len(), a));
        }
        self.ws.send(Message::Text(req.into())).await.unwrap();
        let mut pushes = Vec::new();
        loop {
            let raw = match self.ws.next().await {
                Some(Ok(Message::Text(t))) => t.to_string(),
                Some(Ok(_)) => continue,
                other => panic!("ws closed while waiting for a reply: {other:?}"),
            };
            match Value::parse(raw.as_bytes()) {
                Ok((Value::Push(_), _)) => pushes.push(push_parts(&raw)),
                Ok((v, _)) => return (v, pushes),
                Err(_) => continue,
            }
        }
    }

    /// Every push frame received within `ms` of quiet.
    async fn pushes_within(&mut self, ms: u64) -> Vec<Vec<String>> {
        let mut out = Vec::new();
        while let Some(raw) = self.recv_push(ms).await {
            out.push(push_parts(&raw));
        }
        out
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tenant_write_is_offered_only_to_that_tenant() {
    let server = strict_server().await;
    let mut writer = scoped(&server, "rw=*", "writer").await;
    let mut groups: Vec<(&str, Vec<WsClient>)> = Vec::new();
    for (t, n) in [("A", 20), ("B", 30), ("C", 50)] {
        let mut v = Vec::new();
        for i in 0..n {
            v.push(scoped(&server, &format!("rw=tenant:{t}:*"), &format!("{t}{i}")).await);
        }
        groups.push((t, v));
    }
    let before = server.router.stats();
    assert_eq!(before.routed, 101);
    assert_eq!(writer.cmd(&["SET", "tenant:A:book:123", "v1"]).await, ok());
    let after = server.router.stats();
    // A's 20 and the writer itself, whose `*` grant is broad (it then drops
    // its own write as the origin).
    assert_eq!(after.candidates - before.candidates, 21);
    assert_eq!(after.events - before.events, 1);
    for (t, clients) in &mut groups {
        for c in clients.iter_mut() {
            let got = c.pushes_within(if *t == "A" { 1_000 } else { 100 }).await;
            if *t == "A" {
                assert_eq!(got.len(), 1, "tenant A client missed the write: {got:?}");
                assert_eq!(got[0].get(1).map(String::as_str), Some("tenant:A:book:123"));
            } else {
                assert!(got.is_empty(), "tenant {t} was pushed {got:?}");
            }
        }
    }
    // A connection's own write is offered back to it (it is a candidate)
    // and dropped there as the origin: counted as filtered, never sent.
    assert!(writer.pushes_within(100).await.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn widening_a_token_routes_the_new_scope_without_losing_a_write() {
    let server = strict_server().await;
    let mut writer = scoped(&server, "rw=tenant:a:*,rw=tenant:c:*", "writer").await;
    let mut c = scoped(&server, "rw=tenant:a:*", "alice").await;
    writer.cmd(&["SET", "tenant:c:k", "before"]).await;
    assert!(c.pushes_within(150).await.is_empty(), "not yet in scope");
    let (reply, _) = c
        .cmd_collecting(&[
            "SYNC",
            "TOKEN",
            &token_for("rw=tenant:a:*,rw=tenant:c:*", "alice"),
        ])
        .await;
    assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
    writer.cmd(&["SET", "tenant:c:k", "after"]).await;
    let got = c.pushes_within(1_000).await;
    assert!(
        got.iter()
            .any(|p| p.get(2).map(String::as_str) == Some("after")),
        "{got:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn narrowing_a_token_stops_routing_the_dropped_scope() {
    let server = strict_server().await;
    let mut writer = scoped(&server, "rw=tenant:a:*", "writer").await;
    let mut c = scoped(&server, "rw=tenant:a:*", "alice").await;
    // Narrower, but still covering everything the connection holds (it holds
    // nothing), so the connection stays open.
    let (reply, _) = c
        .cmd_collecting(&["SYNC", "TOKEN", &token_for("rw=tenant:a:x:*", "alice")])
        .await;
    assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
    writer.cmd(&["SET", "tenant:a:y", "dropped"]).await;
    writer.cmd(&["SET", "tenant:a:x:1", "kept"]).await;
    let got = c.pushes_within(500).await;
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].get(1).map(String::as_str), Some("tenant:a:x:1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_connection_routed_mid_stream_misses_no_write_after_its_read() {
    // A writer streams increasing values into one key while a connection
    // presents its token and reads the key. Every write after that read must
    // reach the connection: its last value must be the writer's last.
    for round in 0..8u64 {
        let server = strict_server().await;
        let key = "tenant:a:counter";
        let mut writer = scoped(&server, "rw=tenant:a:*", "writer").await;
        const N: u64 = 400;
        let feed = tokio::spawn(async move {
            for i in 1..=N {
                let v = i.to_string();
                assert_eq!(writer.cmd(&["SET", key, &v]).await, ok());
            }
        });
        let mut c = WsClient::connect(server.tcp_addr).await;
        tokio::time::sleep(Duration::from_micros(round * 700)).await;
        let (reply, mut seen) = c
            .cmd_collecting(&["SYNC", "TOKEN", &token_for("rw=tenant:a:*", "reader")])
            .await;
        assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
        let (read, more) = c.cmd_collecting(&["GET", key]).await;
        seen.extend(more);
        let read: u64 = match read {
            Value::BulkString(Some(b)) => String::from_utf8_lossy(&b).parse().unwrap_or(0),
            _ => 0,
        };
        feed.await.expect("writer");
        seen.extend(c.pushes_within(300).await);
        let values: Vec<u64> = seen
            .iter()
            .filter(|p| p.get(1).map(String::as_str) == Some(key))
            .filter_map(|p| p.get(2).and_then(|v| v.parse().ok()))
            .collect();
        // Per-key order holds, and nothing after the read went missing.
        assert!(
            values.windows(2).all(|w| w[0] < w[1]),
            "out of order: {values:?}"
        );
        let last = values.last().copied().unwrap_or(0).max(read);
        assert_eq!(last, N, "round {round}: read {read}, pushes {values:?}");
        let after_read: Vec<u64> = values.iter().copied().filter(|v| *v > read).collect();
        let expected: Vec<u64> = (read + 1..=N).collect();
        assert_eq!(
            after_read, expected,
            "round {round}: a write after the read was lost"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bridge_principal_cannot_leak_source_keys_through_routing() {
    // A principal granted two tenants copies between them. Routing offers the
    // destination tenant every frame whose first key it owns; none of those
    // naming the source may be sent (Phase 6F S10/S11).
    let server = strict_server().await;
    let mut bridge = scoped(&server, "rw=tenant:a:*,rw=tenant:b:*", "bridge").await;
    let mut b = scoped(&server, "rw=tenant:b:*", "bob").await;
    let mut b_live = scoped(&server, "rw=tenant:b:*", "bob").await;
    assert!(matches!(
        b_live.cmd(&["QSUB", "tenant:b:*"]).await,
        Value::Array(Some(_))
    ));
    bridge.cmd(&["SET", "tenant:a:secret", "A-SECRET"]).await;
    bridge.cmd(&["SADD", "tenant:a:set", "A-MEMBER"]).await;
    for cmd in [
        &["COPY", "tenant:a:secret", "tenant:b:copy"][..],
        &["SUNIONSTORE", "tenant:b:u", "tenant:a:set"],
        &["SINTERSTORE", "tenant:b:i", "tenant:a:set", "tenant:a:set"],
        &["SDIFFSTORE", "tenant:b:d", "tenant:a:set"],
        &["RENAME", "tenant:b:copy", "tenant:a:moved"],
        &["MSET", "tenant:b:m", "1", "tenant:a:m", "2"],
        &["DEL", "tenant:b:m", "tenant:a:m"],
    ] {
        let reply = bridge.cmd(cmd).await;
        assert!(!matches!(reply, Value::Error(_)), "{cmd:?}: {reply:?}");
    }
    bridge.cmd(&["SET", "tenant:b:done", "1"]).await;
    let pushed = b.pushes_within(500).await;
    let text = format!("{pushed:?}{}", b_live.drain_text(300).await);
    // No frame may name a key of tenant A. (The copied *value* is B's own
    // once it is in `tenant:b:copy`; B's live query is sent it, correctly.)
    assert!(!text.contains("tenant:a"), "leaked: {text}");
    assert!(
        !pushed.iter().flatten().any(|part| part == "A-SECRET"),
        "a mutation push carrying the source was sent to B: {pushed:?}"
    );
    // B still gets B's own writes, after all of the above.
    assert!(
        pushed
            .iter()
            .any(|p| p.get(1).map(String::as_str) == Some("tenant:b:done")),
        "{pushed:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn open_mode_routes_everything_until_sync_narrows_it() {
    let server = spawn_ws_server_policy(SyncPolicy::open(), None, None).await;
    let mut writer = WsClient::connect(server.tcp_addr).await;
    let mut all = WsClient::connect(server.tcp_addr).await;
    let mut some = WsClient::connect(server.tcp_addr).await;
    assert!(matches!(
        some.cmd(&["SYNC", "cart:*"]).await,
        Value::Array(Some(_))
    ));
    // Each connection is registered once it has answered a command.
    assert_eq!(all.cmd(&["PING"]).await, Value::SimpleString("PONG".into()));
    writer.cmd(&["SET", "cart:1", "a"]).await;
    writer.cmd(&["SET", "other:1", "b"]).await;
    let to_all = all.pushes_within(500).await;
    let to_some = some.pushes_within(300).await;
    assert_eq!(to_all.len(), 2, "{to_all:?}");
    assert_eq!(to_some.len(), 1, "{to_some:?}");
    assert_eq!(to_some[0].get(1).map(String::as_str), Some("cart:1"));
    let stats = server.router.stats();
    assert_eq!((stats.broad, stats.prefixes), (2, 1), "{stats:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tokenless_connections_are_registered_but_never_routed() {
    let server = strict_server().await;
    let mut writer = scoped(&server, "rw=tenant:a:*", "writer").await;
    let mut anon = WsClient::connect(server.tcp_addr).await;
    assert!(matches!(anon.cmd(&["PING"]).await, Value::SimpleString(_)));
    let stats = server.router.stats();
    assert_eq!((stats.subscribers, stats.routed), (2, 1));
    writer.cmd(&["SET", "tenant:a:k", "v"]).await;
    assert!(anon.pushes_within(200).await.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn disconnects_and_reconnects_leave_no_routes_behind() {
    let server = strict_server().await;
    let mut writer = scoped(&server, "rw=tenant:*", "writer").await;
    for round in 0..5 {
        let mut clients = Vec::new();
        for i in 0..40 {
            clients.push(scoped(&server, &format!("rw=tenant:{i}:*,rw=e:{round}:{i}"), "u").await);
        }
        writer.cmd(&["SET", "tenant:3:k", "v"]).await;
        let got = clients[3].pushes_within(1_000).await;
        assert_eq!(got.len(), 1, "round {round}: {got:?}");
        drop(clients);
    }
    let mut settled = server.router.stats();
    for _ in 0..200 {
        if settled.subscribers == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
        settled = server.router.stats();
    }
    assert_eq!(
        (
            settled.subscribers,
            settled.routed,
            settled.exact_keys,
            settled.prefixes
        ),
        (1, 1, 0, 1),
        "{settled:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unauthenticated_socket_is_pushed_nothing() {
    // Regression (pre-6J, found by the 6J-E review): with a RESP password and
    // no sync secret, a WebSocket that never sent AUTH was refused every
    // command but still pushed every mutation, keys and values included.
    let server = spawn_ws_server_policy(SyncPolicy::open(), None, Some("pw")).await;
    let mut anon = WsClient::connect(server.tcp_addr).await;
    assert!(
        matches!(anon.cmd(&["GET", "secret"]).await, Value::Error(e) if e.starts_with("NOAUTH"))
    );
    let mut writer = WsClient::connect(server.tcp_addr).await;
    assert_eq!(writer.cmd(&["AUTH", "pw"]).await, ok());
    assert_eq!(writer.cmd(&["SET", "secret", "v"]).await, ok());
    let leaked = anon.pushes_within(300).await;
    assert!(leaked.is_empty(), "pushed before AUTH: {leaked:?}");
    assert_eq!(
        server.router.stats().routed,
        1,
        "only the authenticated socket is routed"
    );
    // Authenticating starts the stream.
    assert_eq!(anon.cmd(&["AUTH", "pw"]).await, ok());
    assert_eq!(writer.cmd(&["SET", "secret", "w"]).await, ok());
    let got = anon.pushes_within(1_000).await;
    assert_eq!(got.len(), 1, "{got:?}");
}
