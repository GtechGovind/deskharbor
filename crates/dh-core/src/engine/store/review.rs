use super::*;
use std::fs;
#[test]
fn review_unrelated_settings_edits_do_not_replace_the_directory_grant() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("inbox");
    fs::create_dir(&dir).unwrap();
    let cfg = Config::new(&t.path().join("state")).unwrap();
    let mut s = HostSettings {
        files: true,
        transfer_directory: Some(dir.to_str().unwrap().into()),
        ..Default::default()
    };
    cfg.configure(&s).unwrap();
    let stamp = cfg.snapshot().unwrap().directory_identity.unwrap();
    fs::rename(&dir, t.path().join("prior")).unwrap();
    fs::create_dir(&dir).unwrap();
    s.clipboard = true;
    cfg.configure(&s).unwrap();
    assert_eq!(
        cfg.snapshot().unwrap().directory_identity.as_deref(),
        Some(stamp.as_str()),
        "clipboard edits must not grant a new directory"
    );
    s.files = false;
    let old_path = s.transfer_directory.take();
    cfg.configure(&s).unwrap();
    s.files = true;
    s.transfer_directory = old_path;
    cfg.configure(&s).unwrap();
    assert_eq!(
        cfg.snapshot().unwrap().directory_identity.as_deref(),
        Some(stamp.as_str())
    );
}
#[test]
fn review_private_storage_cannot_follow_a_replaced_parent() {
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("owner");
    let other = temp.path().join("other");
    let cfg = Config::new(&parent).unwrap();
    let original = cfg.snapshot().unwrap().revision;
    let planted = Config::new(&other).unwrap();
    planted
        .configure(&HostSettings {
            enabled: true,
            ..Default::default()
        })
        .unwrap();
    drop(planted);
    match fs::rename(&parent, temp.path().join("moved")) {
        Ok(()) => {
            fs::rename(&other, &parent).unwrap();
            if let Ok(s) = cfg.snapshot() {
                assert_eq!(s.revision, original, "store accepted planted database")
            }
        }
        Err(e) if cfg!(windows) && matches!(e.raw_os_error(), Some(5 | 32)) => {}
        Err(e) => panic!("unexpected rename error {e}"),
    }
}
#[cfg(windows)]
#[test]
fn review_rejects_a_writable_ancestor_and_preplanted_hard_link() {
    let temp = tempfile::tempdir().unwrap();
    let public = temp.path().join("shared");
    fs::create_dir(&public).unwrap();
    // Change only this disposable fixture. Grant the well-known Everyone SID modify access.
    let acl = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/icacls.exe");
    let grant = std::process::Command::new(&acl)
        .arg(&public)
        .args(["/grant", "*S-1-1-0:(OI)(CI)M"])
        .output()
        .unwrap();
    assert!(grant.status.success());
    match Config::new(&public.join("state")) {
        Err(e) => assert_eq!(e.code, "unsafe_storage"),
        Ok(_) => panic!("accepted an ancestor replaceable by another principal"),
    }
    let creations = temp.path().join("create-only");
    fs::create_dir(&creations).unwrap();
    let grant = std::process::Command::new(&acl)
        .arg(&creations)
        .args(["/grant", "*S-1-1-0:(WD,AD)"])
        .output()
        .unwrap();
    assert!(grant.status.success());
    Config::new(&creations.join("owner"))
        .expect("creating siblings alone cannot replace an existing checked directory");
    let cfg = Config::new(&temp.path().join("safe")).unwrap();
    let external = temp.path().join("external");
    fs::write(&external, b"fixture").unwrap();
    fs::hard_link(external, cfg.files.root().join("daemon.lock")).unwrap();
    assert!(
        cfg.lock_file("daemon.lock").is_err(),
        "preplanted hard link was followed"
    );
    let changed = temp.path().join("later-public");
    let cfg = Config::new(&changed).unwrap();
    let grant = std::process::Command::new(&acl)
        .arg(&changed)
        .args(["/grant", "*S-1-1-0:(DC)"])
        .output()
        .unwrap();
    assert!(grant.status.success());
    assert!(
        cfg.snapshot().is_err(),
        "a running store must detect newly granted delete-child rights"
    );
}
#[cfg(unix)]
#[test]
fn review_private_storage_rejects_untrusted_traversal_and_preplanted_secrets() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let temp = tempfile::tempdir().unwrap();
    let writable = temp.path().join("shared");
    fs::create_dir(&writable).unwrap();
    fs::set_permissions(&writable, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(Config::new(&writable.join("state")).is_err());
    let cfg = Config::new(&temp.path().join("safe")).unwrap();
    let outside = temp.path().join("known");
    fs::write(&outside, [42; 32]).unwrap();
    symlink(outside, cfg.files.root().join("user.key")).unwrap();
    assert!(cfg.key().is_err(), "known external owner key accepted");
}
#[cfg(target_os = "macos")]
#[test]
fn review_private_storage_rejects_an_extended_acl_delete_child_grant() {
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("fixture");
    fs::create_dir(&parent).unwrap();
    let grant = std::process::Command::new("/bin/chmod")
        .args(["+a", "everyone allow delete_child"])
        .arg(&parent)
        .status()
        .unwrap();
    assert!(grant.success());
    assert!(Config::new(&parent.join("state")).is_err());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn review_overlapping_status_readers_cannot_impersonate_a_daemon() {
    use super::super::{DeskEngine, HostBackend};
    let t = tempfile::tempdir().unwrap();
    let e = DeskEngine::new(t.path().into()).unwrap();
    e.set_host_password(Zeroizing::new(b"synthetic-status-owner".to_vec()))
        .await
        .unwrap();
    e.configure_host(HostSettings {
        enabled: true,
        bind_address: "127.0.0.1:0".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    let daemon = e.start_daemon(HostBackend::Synthetic).await.unwrap();
    assert!(e.host_status().await.unwrap().running);
    daemon.stop().await.unwrap();
    let concurrent_probe = e.0.cfg.lock_file("daemon.lock").unwrap();
    concurrent_probe.try_lock().unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(9));
    let jobs = (0..8)
        .map(|_| {
            let e = e.clone();
            let b = barrier.clone();
            std::thread::spawn(move || {
                b.wait();
                e.0.cfg.status().unwrap().1.running
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    for job in jobs {
        assert!(
            !job.join().unwrap(),
            "readers reported the previous ready state"
        )
    }
}
use std::sync::Arc;
