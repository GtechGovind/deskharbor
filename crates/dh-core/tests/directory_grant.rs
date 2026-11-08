#![cfg(feature = "vp8")]
use dh_core::engine::*;
#[tokio::test]
async fn replacing_a_granted_directory_requires_an_explicit_new_grant() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("inbox");
    std::fs::create_dir(&path).unwrap();
    let engine = DeskEngine::new(temp.path().join("config")).unwrap();
    engine
        .set_host_password(zeroize::Zeroizing::new(
            b"directory-fixture-secret".to_vec(),
        ))
        .await
        .unwrap();
    let settings = HostSettings {
        enabled: true,
        bind_address: "127.0.0.1:0".into(),
        files: true,
        transfer_directory: Some(path.to_str().unwrap().into()),
        ..Default::default()
    };
    engine.configure_host(settings.clone()).await.unwrap();
    std::fs::rename(&path, temp.path().join("original")).unwrap();
    std::fs::create_dir(&path).unwrap();
    let daemon = engine.start_daemon(HostBackend::Synthetic).await.unwrap();
    assert!(
        !engine.host_status().await.unwrap().running,
        "replacement directory inherited the previous grant"
    );
    assert!(engine.configure_host(settings).await.unwrap().running);
    daemon.stop().await.unwrap();
}
