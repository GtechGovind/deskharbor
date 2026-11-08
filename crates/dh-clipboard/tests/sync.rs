use dh_clipboard::{ClipboardSync, Content, Update};
#[test]
fn png_roundtrip_preserves_bytes_and_rejects_broken_crc() {
    let png = include_bytes!("fixtures/two-pixels.png").to_vec();
    let mut a = ClipboardSync::new([1; 16]);
    let mut b = ClipboardSync::new([2; 16]);
    let update = a.local(Content::Png(png.clone())).unwrap().unwrap();
    assert_eq!(b.receive(update).unwrap(), Some(Content::Png(png.clone())));
    let mut broken = png;
    broken[45] ^= 1;
    assert!(a.local(Content::Png(broken)).is_err());
}
#[test]
fn unicode_roundtrip_deduplicates_remote_watcher_echo_and_old_generations() {
    let mut a = ClipboardSync::new([1; 16]);
    let mut b = ClipboardSync::new([2; 16]);
    let text = Content::Text("Hello नमस्ते 🦀\nsecond line".into());
    let update = a.local(text.clone()).unwrap().unwrap();
    assert_eq!(b.receive(update.clone()).unwrap(), Some(text.clone()));
    assert_eq!(b.local(text).unwrap(), None);
    assert_eq!(b.receive(update.clone()).unwrap(), None);
    assert_eq!(a.receive(update).unwrap(), None);
}
#[test]
fn invalid_or_oversized_updates_do_not_advance_generation() {
    let mut sync = ClipboardSync::new([1; 16]);
    assert!(
        sync.receive(Update {
            origin: [2; 16],
            generation: 1,
            content: Content::Png(vec![1, 2, 3])
        })
        .is_err()
    );
    assert!(
        sync.local(Content::Text("x".repeat(1024 * 1024 + 1)))
            .is_err()
    );
    assert_eq!(
        sync.receive(Update {
            origin: [2; 16],
            generation: 1,
            content: Content::Text("valid retry".into())
        })
        .unwrap(),
        Some(Content::Text("valid retry".into()))
    );
}
