use super::*;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_acknowledged_publication_survives_a_closed_peer() {
    let pass = b"review-publication-pass";
    let id = vec![1, 2, 3];
    let cred = Arc::new(dh_auth::HostCredentials::register(pass).unwrap());
    let listener = dh_net::Listener::bind(
        "127.0.0.1:0".parse().unwrap(),
        dh_net::Identity::generate().unwrap(),
        true,
    )
    .await
    .unwrap();
    let address = listener.local_addr();
    let (got, rx) = oneshot::channel::<()>();
    let peer = tokio::spawn(async move {
        let mut c = listener.accept().await.unwrap();
        c.authenticate_server(cred).await.unwrap();
        let request = c.receive().await.unwrap();
        let Some(Body::TransferState(finish)) = request.body else {
            panic!("expected finish")
        };
        c.send(msg(
            request.request_id,
            Body::TransferState(TransferState {
                state: "completed".into(),
                ..finish
            }),
        ))
        .await
        .unwrap();
        rx.await.unwrap();
    });
    let mut ch = dh_net::connect(address, None, true).await.unwrap();
    ch.authenticate_client(pass).await.unwrap();
    let Body::TransferState(receipt) = rpc(
        &mut ch,
        1,
        Body::TransferState(TransferState {
            id: id.clone(),
            state: "finish".into(),
            offset: 17,
            ..Default::default()
        }),
    )
    .await
    .unwrap() else {
        panic!("missing publication acknowledgement")
    };
    got.send(()).unwrap();
    peer.await.unwrap();
    assert!(ch.receive().await.is_err(), "peer must have closed");
    confirm_publication(&mut ch, receipt, &id, 17, 2, 1)
        .await
        .expect("already published");
}
