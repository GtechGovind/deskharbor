#![cfg(feature = "vp8")]
use dh_core::engine::{DeskEngine, HostBackend, HostSettings};
use dh_protocol::{Command, Envelope, envelope::Body};
use zeroize::Zeroizing;
fn command(name: &str, arg: &str, epoch: u64) -> Envelope {
    Envelope {
        request_id: 1,
        body: Some(Body::Command(Command {
            name: name.into(),
            argument: arg.into(),
            epoch,
        })),
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn secondary_authority_is_role_single_use_and_bound_to_live_control() {
    let dir = tempfile::tempdir().unwrap();
    let engine = DeskEngine::new(dir.path().into()).unwrap();
    engine
        .set_host_password(Zeroizing::new(b"attachment-loopback-only".to_vec()))
        .await
        .unwrap();
    engine
        .configure_host(HostSettings {
            enabled: true,
            bind_address: "127.0.0.1:0".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let daemon = engine.start_daemon(HostBackend::Synthetic).await.unwrap();
    let addr = engine.host_status().await.unwrap().addresses[0]
        .parse()
        .unwrap();
    let mut control = dh_net::connect(addr, None, false).await.unwrap();
    let pin = control.host_identity();
    control.send(command("control", "", 0)).await.unwrap();
    control
        .authenticate_client(b"attachment-loopback-only")
        .await
        .unwrap();
    let Some(Body::HostStatus(status)) = control.receive().await.unwrap().body else {
        panic!("host status")
    };
    control
        .send(command("ticket", "bulk", status.epoch))
        .await
        .unwrap();
    assert!(matches!(
        control.receive().await.unwrap().body,
        Some(Body::Failure(_))
    ));
    control
        .send(command("ticket", "media", status.epoch))
        .await
        .unwrap();
    let Some(Body::AttachmentTicket(ticket)) = control.receive().await.unwrap().body else {
        panic!("ticket")
    };
    let secret: [u8; 32] = ticket.secret.try_into().unwrap();
    let mut wrong = dh_net::connect(addr, Some(pin), false).await.unwrap();
    assert!(wrong.attach_client("bulk", &secret).await.is_err());
    let mut media = dh_net::connect(addr, Some(pin), false).await.unwrap();
    media.attach_client("media", &secret).await.unwrap();
    let mut replay = dh_net::connect(addr, Some(pin), false).await.unwrap();
    assert!(replay.attach_client("media", &secret).await.is_err());
    control
        .send(command("disconnect", "", status.epoch))
        .await
        .unwrap();
    control.receive().await.unwrap();
    drop(control);
    media
        .send(command("frame", "", status.epoch))
        .await
        .unwrap();
    assert!(media.receive().await.is_err());
    daemon.stop().await.unwrap();
}
