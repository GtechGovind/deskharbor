use dh_transfer::{CHUNK_SIZE, Offer, Receiver, TransferError, UserContext};
fn context(epoch: u64) -> UserContext {
    UserContext {
        user: "user-a".into(),
        epoch,
        interactive: true,
    }
}
fn offer(name: &str, bytes: &[u8]) -> Offer {
    Offer {
        filename: name.into(),
        size: bytes.len() as u64,
        checksum: *blake3::hash(bytes).as_bytes(),
    }
}
fn write(
    receiver: &mut Receiver,
    ctx: &UserContext,
    offset: u64,
    bytes: &[u8],
) -> Result<u64, TransferError> {
    receiver.write_chunk(ctx, offset, bytes, *blake3::hash(bytes).as_bytes())
}
#[test]
fn interrupted_transfer_resumes_only_for_same_interactive_user_and_verifies_contents() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = vec![0x5a; CHUNK_SIZE + 17];
    let mut receiver =
        Receiver::begin(dir.path(), offer("example.bin", &bytes), context(1)).unwrap();
    assert_eq!(
        write(&mut receiver, &context(1), 0, &bytes[..CHUNK_SIZE]).unwrap(),
        CHUNK_SIZE as u64
    );
    receiver.pause();
    assert!(
        write(
            &mut receiver,
            &context(1),
            CHUNK_SIZE as u64,
            &bytes[CHUNK_SIZE..]
        )
        .is_err()
    );
    let mut stranger = context(2);
    stranger.user = "user-b".into();
    assert!(receiver.resume(stranger).is_err());
    assert_eq!(receiver.resume(context(2)).unwrap(), CHUNK_SIZE as u64);
    assert!(
        write(
            &mut receiver,
            &context(1),
            CHUNK_SIZE as u64,
            &bytes[CHUNK_SIZE..]
        )
        .is_err()
    );
    write(
        &mut receiver,
        &context(2),
        CHUNK_SIZE as u64,
        &bytes[CHUNK_SIZE..],
    )
    .unwrap();
    receiver.finish(&context(2), false).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("example.bin")).unwrap(),
        bytes
    );
}
#[test]
fn empty_files_cancel_and_explicit_overwrite_work_without_implicit_replacement() {
    let dir = tempfile::tempdir().unwrap();
    Receiver::begin(dir.path(), offer("empty.txt", b""), context(1))
        .unwrap()
        .finish(&context(1), false)
        .unwrap();
    assert_eq!(
        std::fs::metadata(dir.path().join("empty.txt"))
            .unwrap()
            .len(),
        0
    );
    std::fs::write(dir.path().join("existing.txt"), b"original").unwrap();
    let mut collision =
        Receiver::begin(dir.path(), offer("existing.txt", b"new"), context(1)).unwrap();
    write(&mut collision, &context(1), 0, b"new").unwrap();
    assert!(matches!(
        collision.finish(&context(1), false),
        Err(TransferError::Collision)
    ));
    assert_eq!(
        std::fs::read(dir.path().join("existing.txt")).unwrap(),
        b"original"
    );
    let mut replacement =
        Receiver::begin(dir.path(), offer("existing.txt", b"new"), context(1)).unwrap();
    write(&mut replacement, &context(1), 0, b"new").unwrap();
    replacement.finish(&context(1), true).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("existing.txt")).unwrap(),
        b"new"
    );
    Receiver::begin(
        dir.path(),
        offer("cancelled.bin", b"incomplete"),
        context(1),
    )
    .unwrap()
    .cancel()
    .unwrap();
    assert!(!dir.path().join("cancelled.bin").exists());
}
#[test]
fn traversal_reserved_names_corrupt_chunks_and_final_hash_mismatches_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "../escape",
        "/absolute",
        "a/b",
        "a\\b",
        "a:stream",
        "CON.txt",
        "..",
        "trailing.",
        "trailing ",
    ] {
        assert!(
            Receiver::begin(dir.path(), offer(name, b"x"), context(1)).is_err(),
            "{name}"
        );
    }
    let mut receiver =
        Receiver::begin(dir.path(), offer("checksum.bin", b"expected"), context(1)).unwrap();
    assert!(
        receiver
            .write_chunk(&context(1), 0, b"bad", [0; 32])
            .is_err()
    );
    assert!(write(&mut receiver, &context(1), 1, b"bad").is_err());
    write(&mut receiver, &context(1), 0, b"tampered").unwrap();
    assert!(matches!(
        receiver.finish(&context(1), false),
        Err(TransferError::Checksum)
    ));
    assert!(!dir.path().join("checksum.bin").exists());
}
#[cfg(unix)]
#[test]
fn symlink_destination_never_follows_target_outside_granted_directory() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret");
    std::fs::write(&secret, b"original").unwrap();
    std::os::unix::fs::symlink(&secret, root.path().join("link.txt")).unwrap();
    let mut receiver =
        Receiver::begin(root.path(), offer("link.txt", b"safe"), context(1)).unwrap();
    write(&mut receiver, &context(1), 0, b"safe").unwrap();
    assert!(matches!(
        receiver.finish(&context(1), false),
        Err(TransferError::Collision)
    ));
    assert_eq!(std::fs::read(secret).unwrap(), b"original");
}
