use std::{ffi::c_void, ptr::NonNull};
#[derive(Debug, PartialEq, Eq)]
pub enum CodecError {
    InvalidFrame,
    InvalidPacket,
    NeedKeyframe,
    NativeFailure,
}
impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VP8 {self:?}")
    }
}
impl std::error::Error for CodecError {}
pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub keyframe: bool,
    pub width: u32,
    pub height: u32,
}
pub struct DecodedFrame {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
#[repr(C)]
struct YuvFrame {
    y: *const u8,
    u: *const u8,
    v: *const u8,
    y_stride: i32,
    u_stride: i32,
    v_stride: i32,
    width: u32,
    height: u32,
}
unsafe extern "C" {
    fn dh_vpx_encoder_new(width: u32, height: u32, kbps: u32) -> *mut c_void;
    fn dh_vpx_encoder_free(handle: *mut c_void);
    fn dh_vpx_encode(
        handle: *mut c_void,
        i420: *const u8,
        length: usize,
        force: i32,
        data: *mut *const u8,
        size: *mut usize,
        key: *mut i32,
    ) -> i32;
    fn dh_vpx_decoder_new() -> *mut c_void;
    fn dh_vpx_decoder_free(handle: *mut c_void);
    fn dh_vpx_decode(
        handle: *mut c_void,
        data: *const u8,
        length: usize,
        frame: *mut YuvFrame,
    ) -> i32;
}
pub struct Encoder {
    handle: NonNull<c_void>,
    width: u32,
    height: u32,
    needs_keyframe: bool,
}
// SAFETY: The opaque codec owns its state and has no thread affinity. Rust's
// exclusive &mut self prevents concurrent calls; no borrowed native buffers escape.
unsafe impl Send for Encoder {}
impl Encoder {
    pub fn new(width: u32, height: u32, kbps: u32) -> Result<Self, CodecError> {
        checked_size(width, height)?;
        if !(64..=100000).contains(&kbps) {
            return Err(CodecError::InvalidFrame);
        }
        // SAFETY: Dimensions/bitrate are bounded and the bridge returns an owned
        // initialized encoder or null. Drop releases exactly this handle.
        let handle = NonNull::new(unsafe { dh_vpx_encoder_new(width, height, kbps) })
            .ok_or(CodecError::NativeFailure)?;
        Ok(Self {
            handle,
            width,
            height,
            needs_keyframe: true,
        })
    }
    pub fn request_keyframe(&mut self) {
        self.needs_keyframe = true;
    }
    pub fn encode(&mut self, rgba: &[u8], force: bool) -> Result<EncodedFrame, CodecError> {
        let i420 = rgba_to_i420(self.width, self.height, rgba)?;
        let mut data = std::ptr::null();
        let mut length = 0;
        let mut key = 0;
        // SAFETY: The encoder is exclusively owned, i420 is a complete checked
        // buffer, and output variables are writable for this synchronous call.
        let ok = unsafe {
            dh_vpx_encode(
                self.handle.as_ptr(),
                i420.as_ptr(),
                i420.len(),
                i32::from(force || self.needs_keyframe),
                &mut data,
                &mut length,
                &mut key,
            )
        };
        if ok == 0 || data.is_null() || length == 0 || length > 2 * 1024 * 1024 {
            self.needs_keyframe = true;
            return Err(CodecError::NativeFailure);
        }
        // SAFETY: Success returns length initialized bytes owned by the codec
        // until its next encode. Copy them while the exclusive borrow is held.
        let data = unsafe { std::slice::from_raw_parts(data, length) }.to_vec();
        self.needs_keyframe = false;
        Ok(EncodedFrame {
            data,
            keyframe: key != 0,
            width: self.width,
            height: self.height,
        })
    }
}
impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: This is the unique live owned handle, never exposed to callers.
        unsafe {
            dh_vpx_encoder_free(self.handle.as_ptr());
        }
    }
}
pub struct Decoder {
    handle: NonNull<c_void>,
    needs_keyframe: bool,
}
// SAFETY: The decoder has no thread affinity; all native access is through
// exclusive &mut self, and decoded native pointers are copied before returning.
unsafe impl Send for Decoder {}
impl Decoder {
    pub fn new() -> Result<Self, CodecError> {
        // SAFETY: Constructor has no input pointers and returns owned state/null.
        let handle =
            NonNull::new(unsafe { dh_vpx_decoder_new() }).ok_or(CodecError::NativeFailure)?;
        Ok(Self {
            handle,
            needs_keyframe: true,
        })
    }
    pub fn mark_lost(&mut self) {
        self.needs_keyframe = true;
    }
    pub fn decode(&mut self, packet: &[u8]) -> Result<DecodedFrame, CodecError> {
        let key = match validate_packet(packet) {
            Ok(key) => key,
            Err(error) => {
                self.needs_keyframe = true;
                return Err(error);
            }
        };
        if self.needs_keyframe && !key {
            return Err(CodecError::NeedKeyframe);
        }
        let mut yuv = std::mem::MaybeUninit::<YuvFrame>::uninit();
        // SAFETY: A complete bounded packet is borrowed for the call, handle is
        // exclusively owned, and the bridge initializes yuv only on success.
        let ok = unsafe {
            dh_vpx_decode(
                self.handle.as_ptr(),
                packet.as_ptr(),
                packet.len(),
                yuv.as_mut_ptr(),
            )
        };
        if ok == 0 {
            self.needs_keyframe = true;
            return Err(CodecError::InvalidPacket);
        }
        // SAFETY: Successful bridge call initializes every YuvFrame field.
        let frame = unsafe { yuv.assume_init() };
        let result = copy_yuv(&frame);
        self.needs_keyframe = result.is_err();
        result
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: Unique owned decoder is released once, without borrowed frames.
        unsafe {
            dh_vpx_decoder_free(self.handle.as_ptr());
        }
    }
}
fn checked_size(width: u32, height: u32) -> Result<usize, CodecError> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err(CodecError::InvalidFrame);
    }
    Ok(width as usize * height as usize * 4)
}
fn validate_packet(packet: &[u8]) -> Result<bool, CodecError> {
    if packet.len() < 3 || packet.len() > 2 * 1024 * 1024 || (packet[0] >> 1) & 7 > 3 {
        return Err(CodecError::InvalidPacket);
    }
    let key = packet[0] & 1 == 0;
    let header = if key { 10 } else { 3 };
    if packet.len() < header {
        return Err(CodecError::InvalidPacket);
    }
    let partition = (u32::from_le_bytes([packet[0], packet[1], packet[2], 0]) >> 5) as usize;
    if partition == 0 || partition > packet.len() - header {
        return Err(CodecError::InvalidPacket);
    }
    if key {
        if packet[3..6] != [0x9d, 1, 0x2a] {
            return Err(CodecError::InvalidPacket);
        }
        checked_size(
            u16::from_le_bytes([packet[6], packet[7]]) as u32,
            u16::from_le_bytes([packet[8], packet[9]]) as u32,
        )
        .map_err(|_| CodecError::InvalidPacket)?;
    }
    Ok(key)
}
fn rgba_to_i420(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, CodecError> {
    if rgba.len() != checked_size(width, height)? {
        return Err(CodecError::InvalidFrame);
    }
    let (w, h) = (width as usize, height as usize);
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let mut output = vec![0; w * h + 2 * cw * ch];
    for y in 0..h {
        for x in 0..w {
            let index = (y * w + x) * 4;
            let (r, g, b) = (
                rgba[index] as i32,
                rgba[index + 1] as i32,
                rgba[index + 2] as i32,
            );
            output[y * w + x] = ((66 * r + 129 * g + 25 * b + 128) / 256 + 16).clamp(0, 255) as u8;
        }
    }
    for y in 0..ch {
        for x in 0..cw {
            let (mut r, mut g, mut b, mut count) = (0, 0, 0, 0);
            for dy in 0..2 {
                for dx in 0..2 {
                    let (sx, sy) = (x * 2 + dx, y * 2 + dy);
                    if sx < w && sy < h {
                        let i = (sy * w + sx) * 4;
                        r += rgba[i] as i32;
                        g += rgba[i + 1] as i32;
                        b += rgba[i + 2] as i32;
                        count += 1;
                    }
                }
            }
            r = (r + count / 2) / count;
            g = (g + count / 2) / count;
            b = (b + count / 2) / count;
            output[w * h + y * cw + x] =
                (((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128).clamp(0, 255) as u8;
            output[w * h + cw * ch + y * cw + x] =
                (((112 * r - 94 * g - 18 * b + 128) >> 8) + 128).clamp(0, 255) as u8;
        }
    }
    Ok(output)
}
fn copy_yuv(frame: &YuvFrame) -> Result<DecodedFrame, CodecError> {
    let size = checked_size(frame.width, frame.height)?;
    let (w, h) = (frame.width as usize, frame.height as usize);
    let cw = w.div_ceil(2);
    if frame.y.is_null()
        || frame.u.is_null()
        || frame.v.is_null()
        || frame.y_stride < w as i32
        || frame.u_stride < cw as i32
        || frame.v_stride < cw as i32
        || frame.y_stride > 8192
        || frame.u_stride > 8192
        || frame.v_stride > 8192
    {
        return Err(CodecError::InvalidFrame);
    }
    let mut rgba = vec![0; size];
    for y in 0..h {
        for x in 0..w {
            // SAFETY: The bridge supplies libvpx-owned positive-stride I420 planes
            // for this checked display rectangle. Codec remains borrowed and live.
            let (luma, u, v) = unsafe {
                (
                    *frame.y.add(y * frame.y_stride as usize + x) as i32 - 16,
                    *frame.u.add(y / 2 * frame.u_stride as usize + x / 2) as i32 - 128,
                    *frame.v.add(y / 2 * frame.v_stride as usize + x / 2) as i32 - 128,
                )
            };
            let index = (y * w + x) * 4;
            rgba[index] = ((298 * luma + 409 * v + 128) >> 8).clamp(0, 255) as u8;
            rgba[index + 1] = ((298 * luma - 100 * u - 208 * v + 128) >> 8).clamp(0, 255) as u8;
            rgba[index + 2] = ((298 * luma + 516 * u + 128) >> 8).clamp(0, 255) as u8;
            rgba[index + 3] = 255;
        }
    }
    Ok(DecodedFrame {
        rgba,
        width: frame.width,
        height: frame.height,
    })
}
