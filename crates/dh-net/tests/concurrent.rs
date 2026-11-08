use dh_net::{Identity, Listener, connect};
#[tokio::test]
async fn a_stalled_tls_client_does_not_block_another_handshake() {
    let listener = Listener::bind(
        "127.0.0.1:0".parse().unwrap(),
        Identity::generate().unwrap(),
        false,
    )
    .await
    .unwrap();
    let addr = listener.local_addr();
    let _stalled = tokio::net::TcpStream::connect(addr).await.unwrap();
    let first = listener.incoming().await.unwrap();
    let task = tokio::spawn(async move {
        let second = listener.incoming().await.unwrap();
        let channel = second.await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        drop(channel);
    });
    let peer = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        connect(addr, None, false),
    )
    .await
    .unwrap()
    .unwrap();
    drop(peer);
    drop(first);
    task.await.unwrap();
}
