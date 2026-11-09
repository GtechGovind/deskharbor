use dh_media::codecs::vp8::DecodedFrame;
use std::{ffi::c_void, sync::Arc};
#[repr(C)]
#[derive(Debug)]
pub struct DhFrame {
    pub data: *const u8,
    pub length: usize,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub lease: *mut c_void,
}
impl Default for DhFrame {
    fn default() -> Self {
        Self {
            data: std::ptr::null(),
            length: 0,
            width: 0,
            height: 0,
            stride: 0,
            lease: std::ptr::null_mut(),
        }
    }
}
fn acquire_frame(frame: Option<Arc<DecodedFrame>>, out: &mut DhFrame) -> bool {
    *out = DhFrame::default();
    let Some(frame) = frame else { return false };
    let Some(stride) = frame.width.checked_mul(4) else {
        return false;
    };
    let Some(length) = (stride as usize).checked_mul(frame.height as usize) else {
        return false;
    };
    if frame.width == 0
        || frame.height == 0
        || length > isize::MAX as usize
        || frame.rgba.len() != length
    {
        return false;
    }
    let lease = Box::new(frame);
    *out = DhFrame {
        data: lease.rgba.as_ptr(),
        length,
        width: lease.width,
        height: lease.height,
        stride,
        lease: Box::into_raw(lease).cast(),
    };
    true
}
unsafe fn release_frame(lease: *mut c_void) {
    if !lease.is_null() {
        // SAFETY: caller transfers the unique Box pointer from acquire_frame exactly once.
        unsafe {
            drop(Box::from_raw(lease.cast::<Arc<DecodedFrame>>()));
        }
    }
}
/// Acquire immutable, top-left-origin tightly packed RGBA8 pixels.
///
/// # Safety
/// `out` must be null or aligned, writable storage for one DhFrame, with no live
/// lease overwritten. On success caller owns `lease` and must release it once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dh_frame_acquire(session_id: u32, out: *mut DhFrame) -> bool {
    if out.is_null() {
        return false;
    }
    let frame = std::panic::catch_unwind(|| {
        crate::engine()
            .ok()
            .and_then(|e| e.latest_frame(session_id))
    })
    .ok()
    .flatten();
    // SAFETY: caller guarantees writable aligned output; no Rust references escape.
    acquire_frame(frame, unsafe { &mut *out })
}
/// Release a lease after the renderer has stopped reading its bytes.
///
/// # Safety
/// `lease` must be null or the unmodified pointer from one successful acquire.
/// No subsequent read or repeated release is permitted.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dh_frame_release(lease: *mut c_void) {
    // SAFETY: the exported contract carries release_frame's unique ownership obligation.
    unsafe { release_frame(lease) }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(w: u32, h: u32, n: usize) -> Arc<DecodedFrame> {
        Arc::new(DecodedFrame {
            width: w,
            height: h,
            rgba: vec![17; n],
        })
    }
    #[test]
    fn lease_keeps_memory_alive_until_renderer_completion() {
        let f = frame(2, 3, 24);
        let weak = Arc::downgrade(&f);
        let mut out = DhFrame::default();
        assert!(acquire_frame(Some(f.clone()), &mut out));
        drop(f);
        assert!(weak.upgrade().is_some());
        assert_eq!(
            (out.width, out.height, out.stride, out.length),
            (2, 3, 8, 24)
        );
        // SAFETY: the successful acquisition owns a live 24-byte lease.
        unsafe {
            assert_eq!(std::slice::from_raw_parts(out.data, out.length), &[17; 24]);
            release_frame(out.lease);
        }
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn malformed_dimensions_and_lengths_are_rejected() {
        for f in [
            frame(0, 1, 0),
            frame(1, 0, 0),
            frame(2, 3, 23),
            frame(2, 3, 25),
            frame(u32::MAX, 2, 0),
        ] {
            let mut out = DhFrame::default();
            assert!(!acquire_frame(Some(f), &mut out));
            assert!(out.lease.is_null());
            assert!(out.data.is_null());
        }
    }
    #[test]
    fn missing_frame_clears_output() {
        let mut out = DhFrame {
            width: 99,
            length: 99,
            ..DhFrame::default()
        };
        assert!(!acquire_frame(None, &mut out));
        assert_eq!(out.width, 0);
        assert_eq!(out.length, 0);
        assert!(out.data.is_null());
    }
    #[test]
    fn null_release_is_harmless() {
        // SAFETY: the release contract explicitly permits a null lease.
        unsafe {
            release_frame(std::ptr::null_mut());
        }
    }
}
