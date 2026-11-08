use dh_transfer::*;
#[test]
fn revoked_authority_prevents_atomic_publication_and_cleans_staging() {
    let root = tempfile::tempdir().unwrap();
    let user = UserContext {
        user: "ordinary-user".into(),
        epoch: 3,
        interactive: true,
    };
    let data = b"example";
    let mut rx = Receiver::begin(
        root.path(),
        Offer {
            filename: "publish".into(),
            size: 7,
            checksum: *blake3::hash(data).as_bytes(),
        },
        user.clone(),
    )
    .unwrap();
    rx.write_chunk(&user, 0, data, *blake3::hash(data).as_bytes())
        .unwrap();
    assert!(matches!(
        rx.finish_authorized::<()>(&user, false, || {
            assert_eq!(
                root.path().read_dir().unwrap().count(),
                0,
                "staging must not start before fresh authority"
            );
            Err(TransferError::Context)
        }),
        Err(TransferError::Context)
    ));
    assert_eq!(root.path().read_dir().unwrap().count(), 0);
}
