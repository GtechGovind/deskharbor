//! Read-only Windows P0 inspection and signed-in desktop capture.
//! The `service` module supplies a separate, uninstalled isolated-lab harness.
//! API success here is not a secure-desktop, sign-in or input proof.
pub mod service;
use crate::{
    frame::Frame,
    report::{Check, CheckStatus, Display, Inspection, Platform, ProbeError, SessionKind},
};
use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE, LPARAM, RECT},
    Graphics::Gdi::*,
    Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
    System::{
        RemoteDesktop::{ProcessIdToSessionId, WTSGetActiveConsoleSessionId},
        StationsAndDesktops::{
            CloseDesktop, DESKTOP_READOBJECTS, GetProcessWindowStation, GetThreadDesktop,
            GetUserObjectInformationW, OpenInputDesktop, UOI_NAME,
        },
        Threading::{GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId, OpenProcessToken},
    },
    UI::{
        HiDpi::{
            DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            SetThreadDpiAwarenessContext,
        },
        WindowsAndMessaging::MONITORINFOF_PRIMARY,
    },
};

const MAX_EDGE: u32 = 16_384;
const MAX_BYTES: usize = 256 * 1024 * 1024;

fn win_error(operation: &str) -> ProbeError {
    // SAFETY: GetLastError reads thread-local state and has no preconditions.
    let code = unsafe { GetLastError() };
    ProbeError::new("win32_error", format!("{operation}: Win32 error {code}"))
}

struct DpiContext(DPI_AWARENESS_CONTEXT);
impl DpiContext {
    fn physical_pixels() -> Result<Self, ProbeError> {
        // SAFETY: A documented constant changes only this thread's DPI context.
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if previous.is_null() {
            return Err(win_error("SetThreadDpiAwarenessContext"));
        }
        Ok(Self(previous))
    }
}
impl Drop for DpiContext {
    fn drop(&mut self) {
        // SAFETY: This value was returned by SetThreadDpiAwarenessContext on this thread.
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: This guard exclusively owns a valid token/process/thread/job handle returned by its creation API.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn elevation() -> Result<bool, ProbeError> {
    let mut token = null_mut();
    // SAFETY: The pseudo handle is valid; token points to writable local storage.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(win_error("OpenProcessToken"));
    }
    let token = OwnedHandle(token);
    // SAFETY: TOKEN_ELEVATION is a plain zero-initializable Win32 structure.
    let mut value: TOKEN_ELEVATION = unsafe { zeroed() };
    let mut returned = 0;
    // SAFETY: The buffer matches the selected information class and remains live during the call.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            (&mut value as *mut TOKEN_ELEVATION).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    } == 0
    {
        return Err(win_error("GetTokenInformation(TokenElevation)"));
    }
    if returned as usize != size_of::<TOKEN_ELEVATION>() {
        return Err(ProbeError::new(
            "invalid_token_information",
            "Unexpected TOKEN_ELEVATION size",
        ));
    }
    Ok(value.TokenIsElevated != 0)
}

fn object_name(handle: HANDLE) -> Result<String, ProbeError> {
    if handle.is_null() {
        return Err(win_error("desktop/window-station handle"));
    }
    let mut bytes = 0;
    // SAFETY: A null buffer with length zero requests the required buffer size.
    unsafe {
        GetUserObjectInformationW(handle, UOI_NAME, null_mut(), 0, &mut bytes);
    }
    if !(2..=65_536).contains(&bytes) || bytes % 2 != 0 {
        return Err(ProbeError::new(
            "invalid_object_name",
            "Invalid desktop/window-station name size",
        ));
    }
    let mut buffer = vec![0u16; bytes as usize / 2];
    // SAFETY: The allocated UTF-16 buffer has exactly bytes writable bytes.
    if unsafe {
        GetUserObjectInformationW(
            handle,
            UOI_NAME,
            buffer.as_mut_ptr().cast(),
            bytes,
            &mut bytes,
        )
    } == 0
    {
        return Err(win_error("GetUserObjectInformationW"));
    }
    let end = buffer.iter().position(|v| *v == 0).ok_or_else(|| {
        ProbeError::new(
            "invalid_object_name",
            "Unterminated desktop/window-station name",
        )
    })?;
    String::from_utf16(&buffer[..end]).map_err(|_| {
        ProbeError::new(
            "invalid_object_name",
            "Invalid UTF-16 desktop/window-station name",
        )
    })
}

fn input_desktop_name() -> Result<String, ProbeError> {
    // SAFETY: Requests read-only access to the input desktop without switching desktops.
    let desktop = unsafe { OpenInputDesktop(0, 0, DESKTOP_READOBJECTS) };
    if desktop.is_null() {
        return Err(win_error("OpenInputDesktop"));
    }
    let result = object_name(desktop);
    // SAFETY: This call closes only the handle just acquired by OpenInputDesktop.
    unsafe {
        CloseDesktop(desktop);
    }
    result
}

fn session_kind(
    process: Option<u32>,
    console: Option<u32>,
    station: Option<&str>,
    desktop: Option<&str>,
    input: Option<&str>,
) -> SessionKind {
    if console.is_none() {
        return SessionKind::NoConsole;
    }
    if process.is_none()
        || process != console
        || !station.is_some_and(|v| v.eq_ignore_ascii_case("WinSta0"))
    {
        return SessionKind::Unknown;
    }
    if input.is_some_and(|v| v.eq_ignore_ascii_case("Winlogon")) {
        return SessionKind::LockedOrSignIn;
    }
    if desktop.is_some_and(|v| v.eq_ignore_ascii_case("Default"))
        && input.is_some_and(|v| v.eq_ignore_ascii_case("Default"))
    {
        return SessionKind::UserDesktop;
    }
    SessionKind::Unknown
}

fn pixel_length(width: u32, height: u32) -> Result<usize, ProbeError> {
    let length = (width as usize)
        .checked_mul(height as usize)
        .and_then(|v| v.checked_mul(4));
    if width == 0
        || height == 0
        || width > MAX_EDGE
        || height > MAX_EDGE
        || length.is_none_or(|v| v > MAX_BYTES)
    {
        return Err(ProbeError::new(
            "invalid_capture_dimensions",
            "Capture must have nonzero edges <=16384 and at most 256 MiB",
        ));
    }
    Ok(length.expect("checked above"))
}

fn rgba_from_bgra(width: u32, height: u32, mut bytes: Vec<u8>) -> Result<Vec<u8>, ProbeError> {
    if bytes.len() != pixel_length(width, height)? {
        return Err(ProbeError::new(
            "invalid_capture_buffer",
            "BGRA length does not match dimensions",
        ));
    }
    for pixel in bytes.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    Ok(bytes)
}

#[derive(Default)]
struct MonitorEnumeration {
    displays: Vec<Display>,
    error: Option<ProbeError>,
}

unsafe extern "system" fn enumerate_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> i32 {
    // SAFETY: EnumDisplayMonitors calls synchronously with the live enumeration context supplied below.
    let context = unsafe { &mut *(data as *mut MonitorEnumeration) };
    // SAFETY: MONITORINFOEXW is a plain zero-initializable Win32 structure.
    let mut info: MONITORINFOEXW = unsafe { zeroed() };
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    // SAFETY: The pointer addresses the first member of a correctly sized extended structure.
    if unsafe { GetMonitorInfoW(monitor, &mut info.monitorInfo) } == 0 {
        context.error = Some(win_error("GetMonitorInfoW"));
        return 0;
    }
    let rect = info.monitorInfo.rcMonitor;
    let width = i64::from(rect.right) - i64::from(rect.left);
    let height = i64::from(rect.bottom) - i64::from(rect.top);
    if width <= 0 || height <= 0 || width > i64::from(u32::MAX) || height > i64::from(u32::MAX) {
        context.error = Some(ProbeError::new(
            "invalid_monitor_bounds",
            "Win32 returned invalid monitor bounds",
        ));
        return 0;
    }
    let end = info
        .szDevice
        .iter()
        .position(|v| *v == 0)
        .unwrap_or(info.szDevice.len());
    let id = String::from_utf16_lossy(&info.szDevice[..end]);
    context.displays.push(Display {
        name: id.clone(),
        id,
        x: rect.left,
        y: rect.top,
        width: width as u32,
        height: height as u32,
        primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
    });
    1
}

fn displays() -> Result<Vec<Display>, ProbeError> {
    let _dpi = DpiContext::physical_pixels()?;
    let mut context = MonitorEnumeration::default();
    // SAFETY: The callback and its data context remain valid for this synchronous enumeration.
    let result = unsafe {
        EnumDisplayMonitors(
            null_mut(),
            null(),
            Some(enumerate_monitor),
            (&mut context as *mut MonitorEnumeration) as LPARAM,
        )
    };
    if let Some(error) = context.error {
        return Err(error);
    }
    if result == 0 {
        return Err(win_error("EnumDisplayMonitors"));
    }
    context.displays.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(context.displays)
}

fn record<T>(
    checks: &mut Vec<Check>,
    name: &str,
    result: Result<T, ProbeError>,
    detail: impl FnOnce(&T) -> String,
) -> Option<T> {
    match result {
        Ok(value) => {
            checks.push(Check {
                name: name.into(),
                status: CheckStatus::Passed,
                detail: detail(&value),
            });
            Some(value)
        }
        Err(error) => {
            checks.push(Check {
                name: name.into(),
                status: CheckStatus::Failed,
                detail: error.to_string(),
            });
            None
        }
    }
}

pub fn inspect() -> Result<Inspection, ProbeError> {
    let mut checks = Vec::new();
    let elevated = record(
        &mut checks,
        "windows.token_elevation_query",
        elevation(),
        |v| format!("elevated={v}; this reports the current token, not an elevation request"),
    )
    .unwrap_or(false);
    let mut process_id = 0;
    // SAFETY: The process ID is current and the output pointer addresses a live u32.
    let process_result =
        if unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut process_id) } != 0 {
            Ok(process_id)
        } else {
            Err(win_error("ProcessIdToSessionId"))
        };
    let process_session = record(
        &mut checks,
        "windows.process_session",
        process_result,
        |v| format!("process session {v}"),
    );
    // SAFETY: This query takes no arguments and changes no state.
    let active = unsafe { WTSGetActiveConsoleSessionId() };
    let console_session = (active != u32::MAX).then_some(active);
    checks.push(Check {
        name: "windows.active_console_query".into(),
        status: CheckStatus::Passed,
        detail: console_session.map_or_else(
            || "no attached active console (0xffffffff)".into(),
            |v| format!("active console session {v}"),
        ),
    });
    // SAFETY: These APIs return borrowed handles associated with the current process/thread; they are not closed by this probe.
    let station = record(
        &mut checks,
        "windows.window_station",
        object_name(unsafe { GetProcessWindowStation() }),
        |v| v.clone(),
    );
    // SAFETY: GetCurrentThreadId is valid and GetThreadDesktop returns a borrowed handle.
    let desktop = record(
        &mut checks,
        "windows.thread_desktop",
        object_name(unsafe { GetThreadDesktop(GetCurrentThreadId()) }),
        |v| v.clone(),
    );
    let input = record(
        &mut checks,
        "windows.input_desktop_read",
        input_desktop_name(),
        |v| v.clone(),
    );
    let session_kind = session_kind(
        process_session,
        console_session,
        station.as_deref(),
        desktop.as_deref(),
        input.as_deref(),
    );
    let displays = record(
        &mut checks,
        "windows.display_enumeration",
        displays(),
        |v| {
            format!(
                "{} display(s); physical pixel coordinates, including negative virtual origins",
                v.len()
            )
        },
    )
    .unwrap_or_default();
    for name in [
        "windows.boot_signin",
        "windows.lock_unlock",
        "windows.logout",
        "windows.uac_secure_desktop",
        "windows.sas",
        "windows.fifty_login_cycles",
    ] {
        checks.push(Check { name: name.into(), status: CheckStatus::NotRun, detail: "Requires isolated Windows VM/physical lab and installed privileged helper; current-process inspection cannot establish this proof".into() });
    }
    Ok(Inspection { schema_version: 1, platform: Platform::Windows, environment: format!("Windows {} current-process Win32/GDI probe; installed-helper capability remains unproven", std::env::consts::ARCH), elevated, process_session, console_session, desktop, session_kind, displays, checks, warnings: vec!["UserDesktop is an observation of matching console/WinSta0/Default names, not proof of unlocked-screen pixel content. Desktop transitions may race observation.".into(), "GDI may omit protected video, hardware overlay/direct-scanout planes and cursor. This backend does not prove full-product capture or input support.".into(), "Failed elevation queries report elevated=false and a failed check; inspect check statuses before using this report.".into()] })
}

#[derive(Default)]
struct CaptureSurface {
    screen: HDC,
    memory: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}
impl Drop for CaptureSurface {
    fn drop(&mut self) {
        // SAFETY: Only successfully acquired, owned resources are released. Restore the selected object before deleting its bitmap; borrowed stock objects are not deleted.
        unsafe {
            if !self.previous.is_null() {
                SelectObject(self.memory, self.previous);
            }
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap);
            }
            if !self.memory.is_null() {
                DeleteDC(self.memory);
            }
            if !self.screen.is_null() {
                ReleaseDC(null_mut(), self.screen);
            }
        }
    }
}

pub fn capture(display_id: &str) -> Result<Frame, ProbeError> {
    capture_current_desktop(display_id, false)
}

fn capture_current_desktop(display_id: &str, secure_worker: bool) -> Result<Frame, ProbeError> {
    let _dpi = DpiContext::physical_pixels()?;
    let before = inspect()?;
    let allowed = before.session_kind == SessionKind::UserDesktop
        || (secure_worker
            && before.session_kind == SessionKind::LockedOrSignIn
            && before
                .desktop
                .as_deref()
                .is_some_and(|v| v.eq_ignore_ascii_case("Winlogon")));
    if !allowed {
        return Err(ProbeError::new(
            "desktop_unavailable",
            "Current-desktop GDI capture requires the expected observed active-console desktop; secure/sign-in support remains unproven",
        ));
    }
    let display = before
        .displays
        .iter()
        .find(|d| d.id == display_id)
        .ok_or_else(|| {
            ProbeError::new(
                "display_not_found",
                format!("No current monitor with ID {display_id:?}"),
            )
        })?;
    let length = pixel_length(display.width, display.height)?;
    let mut surface = CaptureSurface {
        // SAFETY: Null HWND requests a screen DC, owned until ReleaseDC by the guard.
        screen: unsafe { GetDC(null_mut()) },
        ..CaptureSurface::default()
    };
    if surface.screen.is_null() {
        return Err(win_error("GetDC(screen)"));
    }
    // SAFETY: The screen DC is valid; the new memory DC is owned by the guard.
    surface.memory = unsafe { CreateCompatibleDC(surface.screen) };
    if surface.memory.is_null() {
        return Err(win_error("CreateCompatibleDC"));
    }
    // SAFETY: BITMAPINFO is a plain zero-initializable Win32 structure.
    let mut info: BITMAPINFO = unsafe { zeroed() };
    info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = display.width as i32;
    info.bmiHeader.biHeight = -(display.height as i32); // negative height requests top-down rows
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    info.bmiHeader.biSizeImage = length as u32;
    let mut pixels: *mut c_void = null_mut();
    // SAFETY: The valid bitmap description is bounded before allocation; the OS owns the returned memory until DeleteObject.
    surface.bitmap = unsafe {
        CreateDIBSection(
            surface.screen,
            &info,
            DIB_RGB_COLORS,
            &mut pixels,
            null_mut(),
            0,
        )
    };
    if surface.bitmap.is_null() || pixels.is_null() {
        return Err(win_error("CreateDIBSection"));
    }
    // SAFETY: Both objects are valid and the bitmap is selected in only this memory DC.
    let previous = unsafe { SelectObject(surface.memory, surface.bitmap) };
    if previous.is_null() || previous as isize == -1 {
        return Err(win_error("SelectObject"));
    }
    surface.previous = previous;
    // SAFETY: Source/destination dimensions are bounded; signed virtual origins come directly from monitor enumeration.
    if unsafe {
        BitBlt(
            surface.memory,
            0,
            0,
            display.width as i32,
            display.height as i32,
            surface.screen,
            display.x,
            display.y,
            SRCCOPY | CAPTUREBLT,
        )
    } == 0
    {
        return Err(win_error("BitBlt"));
    }
    // SAFETY: Flush synchronizes GDI writes before directly reading the DIB memory.
    if unsafe { GdiFlush() } == 0 {
        return Err(win_error("GdiFlush"));
    }
    // SAFETY: CreateDIBSection allocated length bytes for this 32-bit uncompressed, top-down bitmap, and its guard is still live.
    let bgra = unsafe { std::slice::from_raw_parts(pixels.cast::<u8>(), length) }.to_vec();
    let after = inspect()?;
    let same_display = after.displays.iter().any(|d| {
        d.id == display.id
            && d.x == display.x
            && d.y == display.y
            && d.width == display.width
            && d.height == display.height
    });
    if after.session_kind != before.session_kind
        || before.desktop != after.desktop
        || before.process_session != after.process_session
        || before.console_session != after.console_session
        || !same_display
    {
        return Err(ProbeError::new(
            "desktop_changed",
            "Console, input desktop or monitor layout changed during capture; frame discarded",
        ));
    }
    Frame::new_rgba(
        display.id.clone(),
        display.width,
        display.height,
        rgba_from_bgra(display.width, display.height, bgra)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra_rows_preserve_top_down_order_and_force_opaque_alpha() {
        let got = rgba_from_bgra(1, 2, vec![3, 2, 1, 0, 50, 100, 200, 0]).unwrap();
        assert_eq!(got, [1, 2, 3, 255, 200, 100, 50, 255]);
    }

    #[test]
    fn pixel_conversion_rejects_invalid_or_unbounded_dimensions() {
        for (width, height, pixels) in [
            (0, 1, vec![]),
            (1, 1, vec![0; 3]),
            (1, 1, vec![0; 5]),
            (32768, 32768, vec![]),
            (u32::MAX, 2, vec![]),
        ] {
            assert!(rgba_from_bgra(width, height, pixels).is_err());
        }
    }

    #[test]
    fn native_allocation_budget_is_checked_before_a_pixel_buffer_exists() {
        assert_eq!(pixel_length(2, 2).unwrap(), 16);
        assert_eq!(pixel_length(16_384, 4096).unwrap(), 268_435_456);
        assert!(pixel_length(16_384, 4097).is_err());
        assert!(pixel_length(16_385, 1).is_err());
        assert!(pixel_length(0, 1).is_err());
    }

    #[test]
    fn user_desktop_requires_matching_active_console_station_and_input_desktop() {
        assert_eq!(
            session_kind(
                Some(2),
                Some(2),
                Some("WinSta0"),
                Some("Default"),
                Some("Default")
            ),
            SessionKind::UserDesktop
        );
        assert_eq!(
            session_kind(
                Some(1),
                Some(2),
                Some("WinSta0"),
                Some("Default"),
                Some("Default")
            ),
            SessionKind::Unknown
        );
        assert_eq!(
            session_kind(
                Some(2),
                Some(2),
                Some("Service-0x0-3e7$"),
                Some("Default"),
                Some("Default")
            ),
            SessionKind::Unknown
        );
        assert_eq!(
            session_kind(Some(2), Some(2), Some("WinSta0"), Some("Default"), None),
            SessionKind::Unknown
        );
        assert_eq!(
            session_kind(
                Some(2),
                Some(2),
                Some("WinSta0"),
                Some("Default"),
                Some("Winlogon")
            ),
            SessionKind::LockedOrSignIn
        );
        assert_eq!(
            session_kind(
                Some(2),
                None,
                Some("WinSta0"),
                Some("Default"),
                Some("Default")
            ),
            SessionKind::NoConsole
        );
    }
}
