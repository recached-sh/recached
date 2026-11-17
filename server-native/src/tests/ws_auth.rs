//! WebSocket authentication and the sync stream.
//!
//! These tests were written for the 0.5.2 patch (GHSA-3f7f-vqch-f76m). `dev`
//! had already fixed the same leak through push routing (6J), and
//! `sync_routing::an_unauthenticated_socket_is_pushed_nothing` covers the
//! basic case. These tests add the cases 0.5.2 also pinned:
//! - binary frames
//! - AUTH while writes are in flight
//! - a failed AUTH
//! - reconnecting
//! - strict mode with a password
//!
//! Every "sent nothing" check reads raw frames, so a push cannot hide behind
//! a reply the way it can with `cmd`.

use super::*;

const SECRET: &str = "ws-auth-test-secret-0123456789abcdef";

fn token_for(entries: &str) -> String {
    mint_sync_token(SECRET, &format!("{entries}|{}", unix_now_secs() + 3600))
}

/// The RESP encoding of `SET <key> <value>` with a raw byte value.
fn resp_set_bytes(key: &str, value: &[u8]) -> Vec<u8> {
    let mut raw = format!(
        "*3\r\n$3\r\nSET\r\n${}\r\n{key}\r\n${}\r\n",
        key.len(),
        value.len()
    )
    .into_bytes();
    raw.extend_from_slice(value);
    raw.extend_from_slice(b"\r\n");
    raw
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_auth_leaves_the_socket_pushed_nothing() {
    let srv = spawn_ws_server_policy(SyncPolicy::open(), None, Some("pw")).await;
    let mut anon = WsClient::connect(srv.tcp_addr).await;
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(writer.cmd(&["AUTH", "pw"]).await, ok());

    assert_eq!(writer.cmd(&["SET", "secret", "v1"]).await, ok());
    assert_eq!(anon.recv_frame(300).await, None, "sent a frame before AUTH");

    anon.send(&["AUTH", "nope"]).await;
    let refusal = anon.recv_frame(1_000).await.expect("a reply to AUTH");
    assert!(
        refusal.starts_with(b"-"),
        "expected only the refusal, got {:?}",
        String::from_utf8_lossy(&refusal)
    );
    assert_eq!(writer.cmd(&["SET", "secret", "v2"]).await, ok());
    assert_eq!(
        anon.recv_frame(300).await,
        None,
        "sent a frame after a failed AUTH"
    );

    assert_eq!(anon.cmd(&["AUTH", "pw"]).await, ok());
    assert_eq!(writer.cmd(&["SET", "secret", "v3"]).await, ok());
    let push = anon
        .recv_push(2_000)
        .await
        .expect("pushed once authenticated");
    assert!(push.contains("secret") && push.contains("v3"), "{push}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unauthenticated_socket_is_sent_no_binary_push() {
    // A mutation whose bytes are not UTF-8 is pushed as a binary frame.
    let srv = spawn_ws_server_policy(SyncPolicy::open(), None, Some("pw")).await;
    let mut anon = WsClient::connect(srv.tcp_addr).await;
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(writer.cmd(&["AUTH", "pw"]).await, ok());

    let value = [0xff, 0xfe, b'x'];
    assert_eq!(
        writer.cmd_binary(resp_set_bytes("bin:1", &value)).await,
        ok()
    );
    assert_eq!(writer.cmd(&["SET", "txt:1", "plain"]).await, ok());
    assert_eq!(
        anon.recv_frame(300).await,
        None,
        "a frame reached a socket before AUTH"
    );

    assert_eq!(anon.cmd(&["AUTH", "pw"]).await, ok());
    assert_eq!(
        writer.cmd_binary(resp_set_bytes("bin:2", &value)).await,
        ok()
    );
    let frame = anon
        .recv_frame(2_000)
        .await
        .expect("an authenticated socket is pushed binary mutations");
    assert!(
        frame.windows(5).any(|w| w == b"bin:2") && frame.windows(3).any(|w| w == value),
        "{frame:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn auth_during_a_write_stream_sends_nothing_before_its_reply() {
    let srv = spawn_ws_server_policy(SyncPolicy::open(), None, Some("pw")).await;
    let stop = Arc::new(AtomicBool::new(false));
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(writer.cmd(&["AUTH", "pw"]).await, ok());
    let writing = {
        let stop = Arc::clone(&stop);
        tokio::spawn(async move {
            let mut i = 0u64;
            while !stop.load(Ordering::Relaxed) {
                let (key, val) = (format!("stream:{i}"), format!("val{i}"));
                assert_eq!(writer.cmd(&["SET", &key, &val]).await, ok());
                i += 1;
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            i
        })
    };

    let mut anon = WsClient::connect(srv.tcp_addr).await;
    for _ in 0..50 {
        anon.send(&["GET", "stream:0"]).await;
        let frame = anon.recv_frame(1_000).await.expect("a reply to GET");
        assert!(
            frame.starts_with(b"-NOAUTH"),
            "expected only the NOAUTH reply, got {:?}",
            String::from_utf8_lossy(&frame)
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    anon.send(&["AUTH", "pw"]).await;
    assert_eq!(
        anon.recv_frame(1_000).await.as_deref(),
        Some(&b"+OK\r\n"[..]),
        "the first frame after AUTH must be its reply"
    );
    let push = anon
        .recv_push(2_000)
        .await
        .expect("mutations are pushed once authenticated");
    assert!(push.contains("stream:"), "{push}");

    stop.store(true, Ordering::Relaxed);
    let written = writing.await.unwrap();
    assert!(written > 10, "the writer barely ran ({written} writes)");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authentication_does_not_survive_a_reconnect() {
    let srv = spawn_ws_server_policy(SyncPolicy::open(), None, Some("pw")).await;
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(writer.cmd(&["AUTH", "pw"]).await, ok());

    let mut client = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(client.cmd(&["AUTH", "pw"]).await, ok());
    assert_eq!(writer.cmd(&["SET", "r", "1"]).await, ok());
    assert!(client.recv_push(2_000).await.is_some());
    client.ws.close(None).await.unwrap();
    drop(client);

    let mut again = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(writer.cmd(&["SET", "r", "2"]).await, ok());
    assert_eq!(
        again.recv_frame(300).await,
        None,
        "a reconnect inherited AUTH"
    );
    assert_eq!(again.cmd(&["AUTH", "pw"]).await, ok());
    assert_eq!(writer.cmd(&["SET", "r", "3"]).await, ok());
    let push = again.recv_push(2_000).await.expect("pushed after re-AUTH");
    assert!(push.contains('3'), "{push}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_mode_with_a_password_needs_auth_and_a_token() {
    let srv = spawn_ws_server_policy(SyncPolicy::strict(SECRET), None, Some("pw")).await;
    let mut writer = WsClient::connect(srv.tcp_addr).await;
    assert_eq!(writer.cmd(&["AUTH", "pw"]).await, ok());
    let reply = writer
        .cmd(&["SYNC", "TOKEN", &token_for("cart:*,other:*")])
        .await;
    assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");

    let mut client = WsClient::connect(srv.tcp_addr).await;
    let tok = token_for("cart:*");
    // A token without AUTH is refused and grants nothing.
    assert!(
        matches!(client.cmd(&["SYNC", "TOKEN", &tok]).await, Value::Error(e) if e.starts_with("NOAUTH"))
    );
    assert_eq!(writer.cmd(&["SET", "cart:1", "a"]).await, ok());
    assert_eq!(client.recv_frame(300).await, None, "pushed before AUTH");

    // AUTH without a token: strict mode still pushes nothing.
    assert_eq!(client.cmd(&["AUTH", "pw"]).await, ok());
    assert_eq!(writer.cmd(&["SET", "cart:2", "b"]).await, ok());
    assert_eq!(client.recv_frame(300).await, None, "pushed without a token");

    // AUTH and a token: in-scope writes only.
    let reply = client.cmd(&["SYNC", "TOKEN", &tok]).await;
    assert!(matches!(reply, Value::Array(Some(_))), "{reply:?}");
    assert_eq!(writer.cmd(&["SET", "other:3", "hidden"]).await, ok());
    assert_eq!(writer.cmd(&["SET", "cart:3", "c"]).await, ok());
    let push = client.recv_push(2_000).await.expect("in-scope push");
    assert!(push.contains("cart:3"), "{push}");
    assert!(
        !push.contains("other:3") && !push.contains("hidden"),
        "{push}"
    );
    assert_eq!(
        client.recv_frame(300).await,
        None,
        "out-of-scope push leaked"
    );
}
