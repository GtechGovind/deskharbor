//! Same-console, non-elevated WinSta0/Default adapter. Never switches desktops.
use super::input::{Context, Held, NativeInput, Packet, packets, same_display};
use crate::report::{Display, ProbeError};
use std::{mem::size_of, ptr::null_mut, sync::Mutex};
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_NO_TOKEN, GetLastError},
    Security::TOKEN_QUERY,
    System::Threading::{GetCurrentThread, OpenThreadToken},
    UI::{
        HiDpi::{
            DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            SetThreadDpiAwarenessContext,
        },
        Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, MOUSEINPUT, SendInput,
        },
    },
};

static HELD: Mutex<Held> = Mutex::new(Held::empty());

struct PhysicalDpi(DPI_AWARENESS_CONTEXT);
impl PhysicalDpi {
    fn enter() -> Result<Self, ProbeError> {
        // SAFETY: Changes only this thread, using a documented context; restored by Drop.
        let old =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if old.is_null() {
            return Err(ProbeError::new(
                "permission_needed",
                "Physical DPI context unavailable",
            ));
        }
        Ok(Self(old))
    }
}
impl Drop for PhysicalDpi {
    fn drop(&mut self) {
        // SAFETY: This context was returned by SetThreadDpiAwarenessContext on this thread.
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}
fn not_impersonating() -> bool {
    let mut token = null_mut();
    // SAFETY: Requests only query access; no token is modified or activated.
    if unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut token) } != 0 {
        // SAFETY: The successful query returned an owned token handle.
        unsafe {
            CloseHandle(token);
        }
        return false;
    }
    // SAFETY: Read the error immediately after the failed query.
    unsafe { GetLastError() == ERROR_NO_TOKEN }
}
fn observe(expected: &str) -> Option<Vec<Display>> {
    if !not_impersonating() {
        return None;
    }
    let observation = super::inspect();
    (observation.state == "ready" && observation.user.as_deref() == Some(expected))
        .then_some(observation.displays)
}
fn context(expected: &str, layout: Option<&[Display]>) -> Context {
    let Some(now) = observe(expected) else {
        return Context::PrincipalLost;
    };
    if layout.is_some_and(|old| {
        old.len() != now.len() || !old.iter().zip(&now).all(|(a, b)| same_display(a, b))
    }) {
        Context::LayoutChanged
    } else {
        Context::Ready
    }
}
fn active(expected: &str) -> bool {
    matches!(context(expected, None), Context::Ready)
}
fn native(packet: Packet) -> INPUT {
    match packet {
        Packet::Key { vk, scan, flags } => INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        Packet::Mouse { x, y, data, flags } => INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: x,
                    dy: y,
                    mouseData: data,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    }
}
fn send(packet: Packet) -> Result<(), ProbeError> {
    let input = native(packet);
    // SAFETY: INPUT union discriminant matches its initialized member; exactly one live packet.
    // Single-packet calls make success accounting unambiguous (zero or one).
    if unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) } != 1 {
        return Err(ProbeError::new(
            "input_unavailable",
            "SendInput did not deliver the packet (including possible UIPI blocking)",
        ));
    }
    Ok(())
}
pub(super) fn inject(
    expected: &str,
    display: &Display,
    event: NativeInput,
) -> Result<(), ProbeError> {
    let mut held = HELD
        .lock()
        .map_err(|_| ProbeError::new("input_unavailable", "Input state lock poisoned"))?;
    let _dpi = PhysicalDpi::enter()?;
    let Some(layout) = observe(expected) else {
        held.forget();
        return Err(ProbeError::new(
            "desktop_changed",
            "Expected ordinary console principal/desktop unavailable",
        ));
    };
    if !layout.iter().any(|d| same_display(d, display)) {
        return Err(ProbeError::new(
            "stale_layout",
            "Physical monitor geometry changed",
        ));
    }
    if matches!(event, NativeInput::ReleaseAll) {
        return held.release(expected, || active(expected), send);
    }
    let packets = packets(&event, display, &layout, held.buttons)?;
    if let Err(error) = held.deliver(expected, packets, || context(expected, Some(&layout)), send) {
        // A failed text or pointer sequence may already have delivered a down packet.
        // Release only that recorded state, after rechecking the original principal.
        let _ = held.release(expected, || active(expected), send);
        return Err(error);
    }
    Ok(())
}
pub(super) fn release(expected: &str) -> Result<(), ProbeError> {
    let mut held = HELD
        .lock()
        .map_err(|_| ProbeError::new("input_unavailable", "Input state lock poisoned"))?;
    let _dpi = PhysicalDpi::enter()?;
    held.release(expected, || active(expected), send)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_union_conversion_preserves_literal_packet_fields_without_injection() {
        let key = native(Packet::Key {
            vk: 0,
            scan: 0x38,
            flags: 11,
        });
        assert_eq!(key.r#type, 1);
        // SAFETY: native selected the keyboard union member from a Key packet.
        let key = unsafe { key.Anonymous.ki };
        assert_eq!(
            (key.wVk, key.wScan, key.dwFlags, key.time, key.dwExtraInfo),
            (0, 0x38, 11, 0, 0)
        );
        let mouse = native(Packet::Mouse {
            x: 65535,
            y: 0,
            data: (-120i32) as u32,
            flags: 0x1000,
        });
        assert_eq!(mouse.r#type, 0);
        // SAFETY: native selected the mouse union member from a Mouse packet.
        let mouse = unsafe { mouse.Anonymous.mi };
        assert_eq!(
            (mouse.dx, mouse.dy, mouse.mouseData, mouse.dwFlags),
            (65535, 0, 4294967176, 0x1000)
        );
    }
}
