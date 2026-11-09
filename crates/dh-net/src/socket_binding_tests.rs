use super::bind_tcp_with_udp;
use std::{io, net::UdpSocket};

#[tokio::test]
async fn socket_binding_retries_ephemeral_tcp_ports_rejected_for_udp() {
    let mut attempts = 0;
    let (tcp, udp) = bind_tcp_with_udp("127.0.0.1:0".parse().unwrap(), |address| {
        attempts += 1;
        if attempts < 3 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "UDP reserved range",
            ));
        }
        UdpSocket::bind(address)
    })
    .await
    .unwrap();
    assert_eq!(attempts, 3);
    assert_eq!(tcp.local_addr().unwrap(), udp.local_addr().unwrap());
}

#[tokio::test]
async fn socket_binding_does_not_change_an_explicit_port() {
    let reservation = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = reservation.local_addr().unwrap();
    drop(reservation);
    let mut attempts = 0;
    let error = bind_tcp_with_udp::<()>(address, |_| {
        attempts += 1;
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "explicit bind denied",
        ))
    })
    .await
    .unwrap_err();
    assert_eq!(attempts, 1);
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[tokio::test]
async fn socket_binding_bounds_retries_and_preserves_failure() {
    let mut attempts = 0;
    let error = bind_tcp_with_udp::<()>("127.0.0.1:0".parse().unwrap(), |_| {
        attempts += 1;
        Err(io::Error::new(io::ErrorKind::AddrInUse, "occupied"))
    })
    .await
    .unwrap_err();
    assert_eq!(attempts, 16);
    assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
}
