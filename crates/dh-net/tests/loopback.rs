use dh_auth::HostCredentials;
use dh_net::{Identity, Listener, TransportKind, connect};
use dh_protocol::{Command, Envelope, envelope::Body};
use std::sync::Arc;

async fn exchange(quic: bool) {
    let identity = Identity::generate().unwrap();
    let fingerprint = identity.fingerprint();
    let listener = Listener::bind("127.0.0.1:0".parse().unwrap(), identity, quic)
        .await
        .unwrap();
    let addr = listener.local_addr();
    let credentials = Arc::new(HostCredentials::register(b"long permanent password").unwrap());
    let server = tokio::spawn(async move {
        let mut channel = listener.accept().await.unwrap();
        let proof = channel.authenticate_server(credentials).await.unwrap();
        let binding = proof.binding().clone();
        let request = channel.receive().await.unwrap();
        assert!(
            matches!(request.body,Some(Body::Command(Command { ref name,.. })) if name=="status")
        );
        channel
            .send(Envelope {
                request_id: request.request_id,
                body: Some(Body::Command(Command {
                    name: "accepted".into(),
                    ..Default::default()
                })),
            })
            .await
            .unwrap();
        // Keep the QUIC endpoint alive until the client acknowledges receipt.
        channel.receive().await.unwrap();
        binding
    });
    let mut client = connect(addr, Some(fingerprint), true).await.unwrap();
    assert_eq!(
        client.kind(),
        if quic {
            TransportKind::Quic
        } else {
            TransportKind::Tcp
        }
    );
    let binding = client
        .authenticate_client(b"long permanent password")
        .await
        .unwrap();
    client
        .send(Envelope {
            request_id: 42,
            body: Some(Body::Command(Command {
                name: "status".into(),
                ..Default::default()
            })),
        })
        .await
        .unwrap();
    let reply = client.receive().await.unwrap();
    assert_eq!(reply.request_id, 42);
    client
        .send(Envelope {
            request_id: 43,
            body: Some(Body::Command(Command {
                name: "disconnect".into(),
                ..Default::default()
            })),
        })
        .await
        .unwrap();
    assert!(server.await.unwrap() == binding);
}
#[tokio::test]
async fn quic_authenticated_exchange_has_matching_tls_exporters() {
    exchange(true).await;
}
#[tokio::test]
async fn unavailable_udp_falls_back_to_authenticated_tls_tcp() {
    exchange(false).await;
}

#[tokio::test]
async fn wrong_password_and_unauthenticated_command_are_rejected() {
    for wrong_password in [false, true] {
        let identity = Identity::generate().unwrap();
        let listener = Listener::bind("127.0.0.1:0".parse().unwrap(), identity, false)
            .await
            .unwrap();
        let addr = listener.local_addr();
        let credentials = Arc::new(HostCredentials::register(b"long permanent password").unwrap());
        let server = tokio::spawn(async move {
            let mut ch = listener.accept().await.unwrap();
            ch.authenticate_server(credentials).await.is_err()
        });
        let mut client = connect(addr, None, false).await.unwrap();
        if wrong_password {
            assert!(client.authenticate_client(b"wrong password").await.is_err());
        } else {
            client
                .send(Envelope {
                    request_id: 1,
                    body: Some(Body::Command(Command {
                        name: "input".into(),
                        ..Default::default()
                    })),
                })
                .await
                .unwrap();
        }
        drop(client);
        assert!(server.await.unwrap());
    }
}

#[tokio::test]
async fn remembered_host_identity_change_aborts_tls() {
    let identity = Identity::generate().unwrap();
    let listener = Listener::bind("127.0.0.1:0".parse().unwrap(), identity, false)
        .await
        .unwrap();
    let addr = listener.local_addr();
    let server = tokio::spawn(async move {
        let _ = listener.accept().await;
    });
    assert!(connect(addr, Some([9; 32]), false).await.is_err());
    server.await.unwrap();
}
