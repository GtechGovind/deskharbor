use crate::report::ProbeError;

#[derive(Debug)]
pub struct Frame {
    pub display_id: String,
    pub width: u32,
    pub height: u32,
    pub pixels_rgba: Vec<u8>,
}

impl Frame {
    pub fn new_rgba(
        display_id: String,
        width: u32,
        height: u32,
        pixels_rgba: Vec<u8>,
    ) -> Result<Self, ProbeError> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .filter(|&n| n <= 256 * 1024 * 1024)
            .ok_or_else(|| ProbeError::new("frame_too_large", "frame exceeds the 256 MiB bound"))?;
        if width == 0 || height == 0 || width > 16384 || height > 16384 {
            return Err(ProbeError::new(
                "invalid_dimensions",
                "frame dimensions must be 1..16384",
            ));
        }
        if pixels_rgba.len() != expected {
            return Err(ProbeError::new(
                "invalid_pixel_length",
                "RGBA byte length does not match dimensions",
            ));
        }
        Ok(Self {
            display_id,
            width,
            height,
            pixels_rgba,
        })
    }

    pub fn encode_bmp(&self) -> Result<Vec<u8>, ProbeError> {
        // Revalidate because public fields may be changed by a native adapter.
        let width = self.width as usize;
        let height = self.height as usize;
        let rgba_size = width.checked_mul(height).and_then(|n| n.checked_mul(4));
        if self.width == 0
            || self.height == 0
            || self.width > 16384
            || self.height > 16384
            || rgba_size != Some(self.pixels_rgba.len())
            || self.pixels_rgba.len() > 256 * 1024 * 1024
        {
            return Err(ProbeError::new(
                "invalid_frame",
                "invalid frame cannot be exported",
            ));
        }
        let stride = (width * 3 + 3) & !3;
        let image_size = stride * height;
        let size = 54 + image_size;
        let mut out = vec![0_u8; size];
        out[0..2].copy_from_slice(b"BM");
        out[2..6].copy_from_slice(&(size as u32).to_le_bytes());
        out[10..14].copy_from_slice(&54_u32.to_le_bytes());
        out[14..18].copy_from_slice(&40_u32.to_le_bytes());
        out[18..22].copy_from_slice(&self.width.to_le_bytes());
        out[22..26].copy_from_slice(&self.height.to_le_bytes());
        out[26..28].copy_from_slice(&1_u16.to_le_bytes());
        out[28..30].copy_from_slice(&24_u16.to_le_bytes());
        out[34..38].copy_from_slice(&(image_size as u32).to_le_bytes());
        for destination_y in 0..height {
            let source_y = height - 1 - destination_y;
            for x in 0..width {
                let source = (source_y * width + x) * 4;
                let destination = 54 + destination_y * stride + x * 3;
                out[destination..destination + 3].copy_from_slice(&[
                    self.pixels_rgba[source + 2],
                    self.pixels_rgba[source + 1],
                    self.pixels_rgba[source],
                ]);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::Frame;

    #[test]
    fn valid_two_row_capture_has_independent_bgr_bottom_up_fixture() {
        let frame =
            Frame::new_rgba("fixture".into(), 1, 2, vec![255, 0, 0, 255, 0, 0, 255, 255]).unwrap();
        let bytes = frame.encode_bmp().unwrap();
        assert_eq!(&bytes[0..2], b"BM");
        assert_eq!(&bytes[18..26], &[1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&bytes[54..], &[255, 0, 0, 0, 0, 0, 255, 0]);
        assert_eq!(bytes.len(), 62);
    }

    #[test]
    fn zero_and_mismatched_dimensions_never_export_a_capture() {
        assert!(Frame::new_rgba("x".into(), 0, 1, vec![]).is_err());
        assert!(Frame::new_rgba("x".into(), 2, 2, vec![0; 15]).is_err());
        assert!(Frame::new_rgba("x".into(), u32::MAX, u32::MAX, vec![]).is_err());
    }
}
