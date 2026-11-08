//! CF_UNICODETEXT access in the ordinary active console only.
#[cfg(windows)]
mod win {
    use std::{
        io,
        ptr::{null, null_mut},
    };
    use windows_sys::Win32::{
        Foundation::{GlobalFree, HGLOBAL, HWND},
        System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
        UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, HWND_MESSAGE},
    };
    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenClipboard(hwnd: HWND) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn GetClipboardData(format: u32) -> HGLOBAL;
        fn SetClipboardData(format: u32, data: HGLOBAL) -> HGLOBAL;
    }
    const CF_UNICODETEXT: u32 = 13;
    struct Clipboard(HWND);
    impl Drop for Clipboard {
        fn drop(&mut self) {
            // SAFETY: OpenClipboard succeeded on this thread; the hidden window is uniquely owned.
            unsafe {
                CloseClipboard();
                DestroyWindow(self.0);
            }
        }
    }
    fn active() -> io::Result<()> {
        if super::super::inspect().user.is_none() {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(())
    }
    fn open() -> io::Result<Clipboard> {
        active()?;
        let class: Vec<u16> = "STATIC".encode_utf16().chain(Some(0)).collect();
        // SAFETY: STATIC is a system class; HWND_MESSAGE creates a message-only window on this thread.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                null_mut(),
                null_mut(),
                null(),
            )
        };
        if hwnd.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: The live message window belongs to this thread.
        if unsafe { OpenClipboard(hwnd) } == 0 {
            // SAFETY: Ownership has not been transferred.
            unsafe {
                DestroyWindow(hwnd);
            }
            return Err(io::ErrorKind::WouldBlock.into());
        }
        Ok(Clipboard(hwnd))
    }
    pub fn read() -> io::Result<String> {
        let _clipboard = open()?;
        // SAFETY: Clipboard is open until the guard is dropped.
        let h = unsafe { GetClipboardData(CF_UNICODETEXT) };
        if h.is_null() {
            return Err(io::ErrorKind::NotFound.into());
        }
        // SAFETY: CF_UNICODETEXT returns global memory; GlobalSize bounds all subsequent reads.
        let size = unsafe { GlobalSize(h) };
        if !(2..=2 * 1024 * 1024 + 64).contains(&size) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        // SAFETY: Memory is locked and read only within its allocation, on this thread.
        let value = unsafe {
            let p = GlobalLock(h).cast::<u16>();
            if p.is_null() {
                return Err(io::Error::last_os_error());
            }
            let text = std::slice::from_raw_parts(p, size / 2);
            let result = text
                .iter()
                .position(|x| *x == 0)
                .and_then(|n| String::from_utf16(&text[..n]).ok());
            GlobalUnlock(h);
            result.ok_or(io::ErrorKind::InvalidData)?
        };
        active()?;
        if value.len() > 1024 * 1024 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(value)
    }
    pub fn write(text: &str) -> io::Result<()> {
        if text.len() > 1024 * 1024 || text.contains('\0') {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let data: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let _clipboard = open()?;
        active()?;
        // SAFETY: Allocate CF_UNICODETEXT global memory, copy bounded UTF16 bytes, transfer ownership only on success.
        unsafe {
            let h = GlobalAlloc(GMEM_MOVEABLE, 2 * data.len());
            if h.is_null() {
                return Err(io::Error::last_os_error());
            }
            let p = GlobalLock(h);
            if p.is_null() {
                GlobalFree(h);
                return Err(io::ErrorKind::OutOfMemory.into());
            }
            std::ptr::copy_nonoverlapping(data.as_ptr(), p.cast::<u16>(), data.len());
            GlobalUnlock(h);
            if EmptyClipboard() == 0 || SetClipboardData(CF_UNICODETEXT, h).is_null() {
                GlobalFree(h);
                return Err(io::ErrorKind::PermissionDenied.into());
            }
        }
        Ok(())
    }
}
#[cfg(windows)]
pub use win::{read as clipboard_read, write as clipboard_write};
#[cfg(not(windows))]
pub fn clipboard_read() -> std::io::Result<String> {
    Err(std::io::ErrorKind::Unsupported.into())
}
#[cfg(not(windows))]
pub fn clipboard_write(_: &str) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}
