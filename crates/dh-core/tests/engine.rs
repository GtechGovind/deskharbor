use dh_core::engine::*;
use std::{fs, time::Duration};
use zeroize::Zeroizing;
fn password(s: &str) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(s.as_bytes().to_vec())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn authenticated_loopback_is_exclusive_and_revoked_by_password_change() {
    let dir = tempfile::tempdir().unwrap();
    let host = DeskEngine::new(dir.path().join("host")).unwrap();
    let client = DeskEngine::new(dir.path().join("client")).unwrap();
    let share = dir.path().join("share");
    fs::create_dir(&share).unwrap();
    host.set_host_password(password("loopback-first-secret"))
        .await
        .unwrap();
    let settings = HostSettings {
        enabled: true,
        bind_address: "127.0.0.1:0".into(),
        allow_input: true,
        clipboard: true,
        files: true,
        transfer_directory: Some(share.to_str().unwrap().into()),
    };
    assert!(!host.configure_host(settings.clone()).await.unwrap().running);
    let service = host.start_daemon(HostBackend::Synthetic).await.unwrap();
    let status = host.host_status().await.unwrap();
    assert!(status.running && !status.unattended_available && status.detail.contains("synthetic"));
    let addr = status.addresses[0].clone();
    assert_eq!(
        client
            .connect(addr.clone(), password("incorrect-password"))
            .await
            .unwrap_err()
            .code,
        "authentication_failed"
    );
    let mut events = client.subscribe();
    let session = client
        .connect(addr.clone(), password("loopback-first-secret"))
        .await
        .unwrap();
    assert_eq!(session.state, "connected");
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if let EngineEvent::FrameReady { session_id, .. } = events.recv().await.unwrap() {
                assert_eq!(session_id, session.id);
                break;
            }
        }
    })
    .await
    .unwrap();
    let frame = client.latest_frame(session.id).unwrap();
    assert_eq!((frame.width, frame.height), (64, 48));
    assert_eq!(
        client
            .connect(addr.clone(), password("loopback-first-secret"))
            .await
            .unwrap_err()
            .code,
        "host_busy"
    );
    client
        .clipboard_send_text(session.id, "hello 世界".into())
        .await
        .unwrap();
    assert_eq!(
        client.clipboard_request_text(session.id).await.unwrap(),
        "hello 世界"
    );
    let input = InputRequest {
        epoch: session.epoch - 1,
        layout: session.layout,
        display_id: session.selected_display.clone(),
        kind: InputKind::Key {
            usage: 4,
            pressed: true,
        },
    };
    assert_eq!(
        client.send_input(session.id, input).await.unwrap_err().code,
        "stale_epoch"
    );
    let src = dir.path().join("payload.txt");
    fs::write(&src, b"verified file").unwrap();
    let tx = client
        .start_transfer(session.id, src.to_str().unwrap().into())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let t = client
                .transfers()
                .into_iter()
                .find(|t| t.id == tx.id)
                .unwrap();
            if t.state == "completed" {
                break;
            }
            assert_ne!(t.state, "failed");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        fs::read(share.join("payload.txt")).unwrap(),
        b"verified file"
    );
    // A separate owner process/engine writes the password while the daemon and client are live.
    let admin = DeskEngine::new(dir.path().join("host")).unwrap();
    admin
        .set_host_password(password("loopback-second-secret"))
        .await
        .unwrap();
    assert!(client.clipboard_request_text(session.id).await.is_err());
    let addr = host.host_status().await.unwrap().addresses[0].clone();
    assert_eq!(
        client
            .connect(addr.clone(), password("loopback-first-secret"))
            .await
            .unwrap_err()
            .code,
        "authentication_failed"
    );
    let fresh = client
        .connect(addr, password("loopback-second-secret"))
        .await
        .unwrap();
    assert_ne!(session.id, fresh.id);
    client.disconnect(fresh.id).await.unwrap();
    assert!(client.latest_frame(fresh.id).is_none());
    service.stop().await.unwrap();
    assert!(!admin.host_status().await.unwrap().running);
}
#[test]
fn device_pin_is_persistent_and_cannot_be_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let engine = DeskEngine::new(dir.path().into()).unwrap();
    let dev = SavedDevice {
        name: "host".into(),
        address: "127.0.0.1:47832".into(),
        identity: Some("01".repeat(32)),
    };
    engine.save_device(dev.clone()).unwrap();
    let reopened = DeskEngine::new(dir.path().into()).unwrap();
    assert_eq!(reopened.devices().unwrap()[0].identity, dev.identity);
    assert!(
        reopened
            .save_device(SavedDevice {
                identity: Some("02".repeat(32)),
                ..dev.clone()
            })
            .is_err()
    );
    reopened.forget_device(dev.address).unwrap();
    assert!(engine.devices().unwrap().is_empty());
}
