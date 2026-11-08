#![cfg(feature = "vp8")]
use dh_media::codecs::vp8::{CodecError, Decoder, Encoder};
#[test]
fn real_vp8_roundtrip_preserves_dimensions_and_red_channel_order() {
    let rgba = [240, 15, 15, 255].repeat(64 * 48);
    let mut encoder = Encoder::new(64, 48, 1000).unwrap();
    let packet = encoder.encode(&rgba, true).unwrap();
    assert!(packet.keyframe);
    assert_eq!(&packet.data[3..6], &[0x9d, 0x01, 0x2a]);
    let mut decoder = Decoder::new().unwrap();
    let frame = decoder.decode(&packet.data).unwrap();
    assert_eq!(
        (frame.width, frame.height, frame.rgba.len()),
        (64, 48, 64 * 48 * 4)
    );
    for pixel in frame.rgba.as_chunks::<4>().0 {
        assert!(pixel[0] > 220 && pixel[1] < 35 && pixel[2] < 35);
        assert_eq!(pixel[3], 255);
    }
}
#[test]
fn decoder_requires_keyframe_after_loss_and_recovers_from_forced_keyframe() {
    let rgba = [15, 240, 15, 255].repeat(64 * 48);
    let mut encoder = Encoder::new(64, 48, 1000).unwrap();
    let first = encoder.encode(&rgba, true).unwrap();
    let second = encoder.encode(&rgba, false).unwrap();
    assert!(!second.keyframe);
    let mut decoder = Decoder::new().unwrap();
    assert!(matches!(
        decoder.decode(&second.data),
        Err(CodecError::NeedKeyframe)
    ));
    decoder.decode(&first.data).unwrap();
    decoder.decode(&second.data).unwrap();
    decoder.mark_lost();
    assert!(matches!(
        decoder.decode(&second.data),
        Err(CodecError::NeedKeyframe)
    ));
    let recovery = encoder.encode(&rgba, true).unwrap();
    assert!(decoder.decode(&recovery.data).is_ok());
}
#[test]
fn malformed_frame_invalidates_interframe_dependencies() {
    let rgba=[15,15,240,255].repeat(65*49);
    let mut encoder=Encoder::new(65,49,1000).unwrap();
    let first=encoder.encode(&rgba,true).unwrap();
    let next=encoder.encode(&rgba,false).unwrap();
    let mut decoder=Decoder::new().unwrap();
    let decoded=decoder.decode(&first.data).unwrap();
    assert_eq!((decoded.width,decoded.height,decoded.rgba.len()),(65,49,65*49*4));
    assert!(decoder.decode(&[255;8]).is_err());
    assert!(matches!(decoder.decode(&next.data),Err(CodecError::NeedKeyframe)));
}
#[test]
fn encoder_and_decoder_reject_unbounded_or_malformed_inputs() {
    assert!(Encoder::new(0, 48, 1000).is_err());
    assert!(Encoder::new(16384, 16384, 1000).is_err());
    let mut encoder = Encoder::new(64, 48, 1000).unwrap();
    assert!(encoder.encode(&[0; 32], true).is_err());
    let mut decoder = Decoder::new().unwrap();
    for bytes in [
        vec![],
        vec![255; 32],
        vec![0; 10],
        vec![0; 2 * 1024 * 1024 + 1],
    ] {
        assert!(decoder.decode(&bytes).is_err());
    }
    // Keyframe with valid sync code but 16,383-square advertised dimensions.
    assert!(
        decoder
            .decode(&[0x10, 0, 0, 0x9d, 1, 0x2a, 0xff, 0x3f, 0xff, 0x3f])
            .is_err()
    );
}
