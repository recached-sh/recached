//! 6O-C3: plain TCP connections are split with `TcpStream::into_split`.
//! The test servers use that path (`handle_tcp_stream`), so every TCP
//! integration test runs over the owned halves; this module checks what is
//! specific to them.

use super::*;

/// A client sees the end of its connection only once the server has
/// cleaned up after it. A dropped `OwnedWriteHalf` would send FIN as soon
/// as the handler drops its writer, which happens before the cleanup:
/// `TcpWriteHalf` keeps the socket open until the read half goes too.
/// Checked at the moment EOF arrives, with the client on another worker and
/// a cleanup long enough to race (1,000 watches to unregister, 500
/// ephemeral keys to release): none of the connection's ephemeral keys is
/// left, every round.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn eof_reaches_the_client_only_after_the_connections_cleanup() {
    let server = spawn_server().await;
    let mut setup = Vec::new();
    let mut watch = format!("*1001\r\n$5\r\nWATCH\r\n").into_bytes();
    for i in 0..1_000 {
        let k = format!("eof:w:{i}");
        watch.extend_from_slice(format!("${}\r\n{k}\r\n", k.len()).as_bytes());
    }
    let mut gone_early = 0;
    for round in 0..30 {
        setup.clear();
        for i in 0..500 {
            let k = format!("eph:{round}:{i}");
            setup.extend_from_slice(
                format!("*3\r\n$4\r\nESET\r\n${}\r\n{k}\r\n$1\r\nv\r\n", k.len()).as_bytes(),
            );
        }
        setup.extend_from_slice(&watch);
        setup.extend_from_slice(b"*1\r\n$4\r\nQUIT\r\n");
        let mut c = RespClient::connect(server.tcp_addr).await;
        c.stream.write_all(&setup).await.unwrap();
        for _ in 0..502 {
            assert_eq!(c.cmd(&[]).await, Value::SimpleString("OK".into()));
        }
        assert!(c.read_raw_eof().await, "round {round}: the server closed");
        if server.store.key_count() != 0 {
            gone_early += 1;
        }
        // Let the cleanup finish before the next round's count.
        for _ in 0..100 {
            if server.store.key_count() == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }
    assert_eq!(gone_early, 0, "rounds whose EOF arrived before the cleanup");
}

/// The write half is never shut down early by anything else: a reply too
/// large for the buffer, pipelined requests and a client that half-closes
/// its own side all still get every reply, then EOF.
#[tokio::test]
async fn a_half_closed_client_still_gets_every_reply_then_eof() {
    let server = spawn_server().await;
    let mut s = TcpStream::connect(server.tcp_addr).await.unwrap();
    let big = "b".repeat(200_000);
    let mut req = Vec::new();
    for (i, v) in ["x", big.as_str(), "y"].iter().enumerate() {
        req.extend_from_slice(
            format!("*3\r\n$3\r\nSET\r\n$2\r\nk{i}\r\n${}\r\n{v}\r\n", v.len()).as_bytes(),
        );
        req.extend_from_slice(format!("*2\r\n$3\r\nGET\r\n$2\r\nk{i}\r\n").as_bytes());
    }
    s.write_all(&req).await.unwrap();
    s.shutdown().await.unwrap();
    let mut out = Vec::new();
    tokio::time::timeout(std::time::Duration::from_secs(10), s.read_to_end(&mut out))
        .await
        .expect("EOF arrived")
        .unwrap();
    let mut want = Vec::new();
    for v in ["x", big.as_str(), "y"] {
        want.extend_from_slice(b"+OK\r\n");
        want.extend_from_slice(format!("${}\r\n{v}\r\n", v.len()).as_bytes());
    }
    assert_eq!(out.len(), want.len());
    assert!(out == want, "every reply, in order, then EOF");
}
