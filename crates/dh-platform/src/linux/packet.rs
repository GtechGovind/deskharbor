//! Local DHP2 scanout packet. Pixels borrow the bounded received packet until composition.
use crate::{frame::Frame, report::ProbeError};
use dh_media::plane_compositor::{
    Blend, Crop, Destination, Kind, MAX_BYTES, MAX_EDGE, MAX_PLANES, PixelFormat, Plane, Rotation,
    Scanout, compose,
};
use std::io::Read;

pub(super) const MAX_PACKET_BYTES: u64 = 24 + MAX_PLANES as u64 * 64 + MAX_BYTES as u64;

pub(super) fn read_packet(reader: &mut impl Read, mode_only: bool) -> Result<Vec<u8>, ProbeError> {
    let io_error = |e: std::io::Error| ProbeError::new("worker_protocol", e.to_string());
    let mut header = [0u8; 24];
    reader.read_exact(&mut header[..4]).map_err(io_error)?;
    let size = if &header[..4] == b"DHF1" {
        12
    } else if &header[..4] == b"DHP2" && !mode_only {
        24
    } else {
        return Err(malformed());
    };
    reader.read_exact(&mut header[4..size]).map_err(io_error)?;
    let field = |offset: usize| {
        u32::from_le_bytes(
            header[offset..offset + 4]
                .try_into()
                .expect("fixed header field"),
        )
    };
    let (width, height) = if size == 12 {
        (field(4), field(8))
    } else {
        (field(8), field(12))
    };
    if width == 0 || height == 0 || width > MAX_EDGE || height > MAX_EDGE {
        return Err(malformed());
    }
    let pixels = u64::from(width) * u64::from(height) * 4;
    let length = if mode_only {
        12
    } else if size == 12 {
        if pixels > MAX_BYTES as u64 {
            return Err(malformed());
        }
        12 + pixels as usize
    } else {
        let count = field(16) as usize;
        let total = field(20) as usize;
        if header[4..8] != [1, 0, 24, 0]
            || count == 0
            || count > MAX_PLANES
            || total > MAX_BYTES
            || pixels > MAX_BYTES as u64
        {
            return Err(malformed());
        }
        24 + 64 * count + total
    };
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| malformed())?;
    bytes.extend_from_slice(&header[..size]);
    bytes.resize(length, 0);
    reader.read_exact(&mut bytes[size..]).map_err(io_error)?;
    let mut trailing = [0u8; 1];
    loop {
        match reader.read(&mut trailing) {
            Ok(0) => break,
            Ok(_) => return Err(malformed()),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(io_error(e)),
        }
    }
    Ok(bytes)
}

fn malformed() -> ProbeError {
    ProbeError::new("worker_protocol", "Invalid bounded DHP2 scanout packet")
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], ProbeError> {
        let end = self.position.checked_add(count).ok_or_else(malformed)?;
        let result = self.bytes.get(self.position..end).ok_or_else(malformed)?;
        self.position = end;
        Ok(result)
    }
    fn u8(&mut self) -> Result<u8, ProbeError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, ProbeError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().map_err(|_| malformed())?,
        ))
    }
    fn u32(&mut self) -> Result<u32, ProbeError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| malformed())?,
        ))
    }
}

pub(super) fn decode(id: String, packet: &[u8]) -> Result<Frame, ProbeError> {
    if packet.len() as u64 > MAX_PACKET_BYTES {
        return Err(malformed());
    }
    let mut reader = Reader {
        bytes: packet,
        position: 0,
    };
    if reader.take(4)? != b"DHP2" || reader.u16()? != 1 || reader.u16()? != 24 {
        return Err(malformed());
    }
    let width = reader.u32()?;
    let height = reader.u32()?;
    let count = reader.u32()? as usize;
    let total = reader.u32()? as usize;
    if width == 0
        || height == 0
        || width > MAX_EDGE
        || height > MAX_EDGE
        || u64::from(width) * u64::from(height) * 4 > MAX_BYTES as u64
        || count == 0
        || count > MAX_PLANES
        || total > MAX_BYTES
        || packet.len() != 24 + 64 * count + total
    {
        return Err(malformed());
    }
    let mut planes = Vec::with_capacity(count);
    let mut source_bytes = 0usize;
    for _ in 0..count {
        let id = reader.u32()?;
        let z = reader.u32()? as i32;
        let kind = match reader.u8()? {
            0 => Kind::Primary,
            1 => Kind::Overlay,
            2 => Kind::Cursor,
            _ => return Err(malformed()),
        };
        let format = match reader.u8()? {
            0 => PixelFormat::Rgba,
            1 => PixelFormat::Bgra,
            2 => PixelFormat::Bgrx,
            _ => return Err(malformed()),
        };
        let blend = match reader.u8()? {
            0 => Blend::None,
            1 => Blend::Coverage,
            2 => Blend::Premultiplied,
            _ => return Err(malformed()),
        };
        let rotation = match reader.u8()? {
            0 => Rotation::R0,
            1 => Rotation::R90,
            2 => Rotation::R180,
            3 => Rotation::R270,
            _ => return Err(malformed()),
        };
        let flags = reader.u16()?;
        let opacity = reader.u16()?;
        if flags & !3 != 0 {
            return Err(malformed());
        }
        let source_width = reader.u32()?;
        let source_height = reader.u32()?;
        let stride = reader.u32()?;
        let crop = Crop {
            x: reader.u32()?,
            y: reader.u32()?,
            width: reader.u32()?,
            height: reader.u32()?,
        };
        let destination = Destination {
            x: reader.u32()? as i32,
            y: reader.u32()? as i32,
            width: reader.u32()?,
            height: reader.u32()?,
        };
        let bytes = reader.u32()? as usize;
        if bytes > MAX_BYTES || u64::from(stride) * u64::from(source_height) != bytes as u64 {
            return Err(malformed());
        }
        source_bytes = source_bytes
            .checked_add(bytes)
            .filter(|n| *n <= total)
            .ok_or_else(malformed)?;
        let pixels = reader.take(bytes)?;
        planes.push(Plane {
            id,
            z,
            kind,
            width: source_width,
            height: source_height,
            stride,
            pixels,
            format,
            blend,
            opacity,
            crop,
            destination,
            rotation,
            reflect_x: flags & 1 != 0,
            reflect_y: flags & 2 != 0,
        });
    }
    if reader.position != packet.len() || source_bytes != total {
        return Err(malformed());
    }
    let composed = compose(&Scanout {
        width,
        height,
        planes,
    })
    .map_err(|e| ProbeError::new("unsupported_composition", e.to_string()))?;
    Frame::new_rgba(id, composed.width, composed.height, composed.pixels_rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counted {
        bytes: std::io::Cursor<Vec<u8>>,
        consumed: usize,
    }
    impl Read for Counted {
        fn read(&mut self, target: &mut [u8]) -> std::io::Result<usize> {
            let n = self.bytes.read(target)?;
            self.consumed += n;
            Ok(n)
        }
    }
    #[test]
    fn invalid_header_is_rejected_before_reading_large_irrelevant_payload() {
        let mut bytes = b"DHF1\0\0\0\0\x01\0\0\0".to_vec();
        bytes.extend(vec![42; 1024 * 1024]);
        let mut source = Counted {
            bytes: std::io::Cursor::new(bytes),
            consumed: 0,
        };
        assert!(read_packet(&mut source, false).is_err());
        assert_eq!(source.consumed, 12);
        let mut bytes = b"NOPE".to_vec();
        bytes.extend(vec![42; 1024 * 1024]);
        let mut source = Counted {
            bytes: std::io::Cursor::new(bytes),
            consumed: 0,
        };
        assert!(read_packet(&mut source, false).is_err());
        assert_eq!(source.consumed, 4);
    }

    #[test]
    fn bounded_reader_distinguishes_mode_pixels_and_trailing_data() {
        let mode = b"DHF1\x01\0\0\0\x01\0\0\0".to_vec();
        assert_eq!(read_packet(&mut &mode[..], true).unwrap(), mode);
        assert!(read_packet(&mut &mode[..], false).is_err());
        let mut pixels = mode.clone();
        pixels.extend([1, 2, 3, 255]);
        assert_eq!(read_packet(&mut &pixels[..], false).unwrap(), pixels);
        pixels.push(0);
        assert!(read_packet(&mut &pixels[..], false).is_err());
        let fixture = fixture();
        assert_eq!(read_packet(&mut &fixture[..], false).unwrap(), fixture);
    }

    #[test]
    #[ignore = "Build native/linux test targets first; explicit C-to-Rust fixture check"]
    fn native_encoder_fixture_matches_rust_composition() {
        let current = std::env::current_exe().unwrap();
        let helper = current
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("dh-plane-fixture");
        let result = std::process::Command::new(helper)
            .output()
            .expect("make -C native/linux BUILD=... test first");
        assert!(result.status.success());
        let frame = decode("native-synthetic-fixture".into(), &result.stdout).unwrap();
        assert_eq!(frame.pixels_rgba, [128, 0, 127, 255, 0, 0, 255, 255]);
        let helper = current
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("dh-plane-fixture");
        let visible = std::process::Command::new(helper)
            .arg("--visible")
            .output()
            .unwrap();
        assert!(visible.status.success());
        assert_eq!(visible.stdout.len(), 20);
        assert_eq!(&visible.stdout[..4], b"DHF1");
        let frame = super::super::decode("native-visible-fixture".into(), &visible.stdout).unwrap();
        assert_eq!(frame.pixels_rgba, [128, 0, 127, 255, 0, 0, 255, 255]);
    }

    fn header(width: u32, height: u32, count: u32, total: u32) -> Vec<u8> {
        let mut out = b"DHP2\x01\x00\x18\x00".to_vec();
        for field in [width, height, count, total] {
            out.extend(field.to_le_bytes());
        }
        out
    }

    fn record(id: u32, z: i32, kind: u8, width: u32, pixels: &[u8]) -> Vec<u8> {
        let mut out = vec![0; 64];
        out[0..4].copy_from_slice(&id.to_le_bytes());
        out[4..8].copy_from_slice(&z.to_le_bytes());
        out[8] = kind;
        out[9] = 0;
        out[10] = if kind == 0 { 0 } else { 1 };
        out[14..16].copy_from_slice(&u16::MAX.to_le_bytes());
        for (offset, value) in [
            (16, width),
            (20, 1),
            (24, width * 4),
            (28, 0),
            (32, 0),
            (36, width),
            (40, 1),
            (44, 0),
            (48, 0),
            (52, width),
            (56, 1),
            (60, pixels.len() as u32),
        ] {
            out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        out.extend(pixels);
        out
    }

    fn fixture() -> Vec<u8> {
        let mut out = header(2, 1, 2, 12);
        out.extend(record(1, 0, 0, 2, &[0, 0, 255, 255, 0, 0, 255, 255]));
        out.extend(record(2, 1, 2, 1, &[255, 0, 0, 128]));
        out
    }

    #[test]
    fn independently_assembled_packet_composes_cursor_over_primary() {
        let frame = decode("card0:7".into(), &fixture()).unwrap();
        assert_eq!((frame.width, frame.height), (2, 1));
        assert_eq!(frame.display_id, "card0:7");
        assert_eq!(frame.pixels_rgba, [128, 0, 127, 255, 0, 0, 255, 255]);
    }

    #[test]
    fn truncated_trailing_unknown_flags_and_invalid_counts_never_publish() {
        let bytes = fixture();
        for length in 0..bytes.len() {
            assert!(
                decode("x".into(), &bytes[..length]).is_err(),
                "prefix {length}"
            );
        }
        let mut wrong = bytes.clone();
        wrong.push(0);
        assert!(decode("x".into(), &wrong).is_err());
        for (offset, value) in [
            (4, 2),
            (6, 23),
            (16, 0),
            (16, 33),
            (24 + 8, 9),
            (24 + 9, 9),
            (24 + 10, 9),
            (24 + 11, 9),
            (24 + 12, 4),
        ] {
            let mut wrong = bytes.clone();
            wrong[offset] = value;
            assert!(decode("x".into(), &wrong).is_err(), "field {offset}");
        }
    }

    #[test]
    fn duplicate_plane_metadata_is_not_a_plausible_successful_image() {
        let second = 24 + 64 + 8;
        for (offset, value) in [(second, 1u32), (second + 4, 0u32)] {
            let mut wrong = fixture();
            wrong[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(decode("x".into(), &wrong).is_err());
        }
    }

    #[test]
    fn declared_output_payload_and_stride_budgets_are_checked() {
        for (offset, value) in [
            (8, 16385u32),
            (12, u32::MAX),
            (20, u32::MAX),
            (24 + 24, u32::MAX),
            (24 + 60, u32::MAX),
        ] {
            let mut wrong = fixture();
            wrong[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(decode("x".into(), &wrong).is_err());
        }
        let mut wrong = fixture();
        wrong[20..24].copy_from_slice(&8u32.to_le_bytes());
        assert!(decode("x".into(), &wrong).is_err());
    }

    #[test]
    fn explicit_rotation_flags_reach_composition() {
        let mut bytes = header(1, 2, 1, 8);
        let mut plane = record(1, 0, 0, 2, &[255, 0, 0, 255, 0, 255, 0, 255]);
        plane[11] = 1;
        plane[52..56].copy_from_slice(&1u32.to_le_bytes());
        plane[56..60].copy_from_slice(&2u32.to_le_bytes());
        bytes.extend(plane);
        let frame = decode("x".into(), &bytes).unwrap();
        assert_eq!(frame.pixels_rgba, [255, 0, 0, 255, 0, 255, 0, 255]);
    }
}
