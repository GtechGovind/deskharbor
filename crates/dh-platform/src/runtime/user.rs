use std::{
    io,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree},
    Security::{Authorization::ConvertSidToStringSidW, *},
    System::{
        RemoteDesktop::*,
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
};
pub(super) struct User(Vec<usize>);
impl User {
    pub(super) fn current() -> io::Result<Self> {
        let mut token = null_mut();
        // SAFETY: Requests only TOKEN_QUERY on the current process.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut needed = 0;
        // SAFETY: First call obtains byte size, second fills aligned owned storage; both use the same handle.
        let (user, ok) = unsafe {
            GetTokenInformation(token, TokenUser, null_mut(), 0, &mut needed);
            let mut user = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
            let ok = GetTokenInformation(
                token,
                TokenUser,
                user.as_mut_ptr().cast(),
                needed,
                &mut needed,
            );
            CloseHandle(token);
            (user, ok)
        };
        if ok == 0 {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(Self(user))
    }
    pub(super) fn sid(&self) -> PSID {
        // SAFETY: Successful GetTokenInformation supplied a TOKEN_USER and inline SID in owned aligned storage.
        unsafe { (*self.0.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }
    pub(super) fn id(&self) -> io::Result<String> {
        let mut text = null_mut();
        // SAFETY: The token buffer remains live and the OS returns a nul-terminated LocalAlloc string.
        unsafe {
            if ConvertSidToStringSidW(self.sid(), &mut text) == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut n = 0;
            while *text.add(n) != 0 {
                n += 1
            }
            let id = String::from_utf16_lossy(std::slice::from_raw_parts(text, n));
            LocalFree(text.cast());
            Ok(id)
        }
    }
}
fn field(session: u32, kind: WTS_INFO_CLASS) -> Option<String> {
    let mut ptr = null_mut();
    let mut bytes = 0;
    // SAFETY: WTS initializes a string buffer and size on success; copy within size then release.
    unsafe {
        if WTSQuerySessionInformationW(null_mut(), session, kind, &mut ptr, &mut bytes) == 0 {
            return None;
        }
        let value = if !ptr.is_null() && bytes >= 2 && bytes <= 2048 && bytes.is_multiple_of(2) {
            let slice = std::slice::from_raw_parts(ptr, bytes as usize / 2);
            slice
                .iter()
                .position(|c| *c == 0)
                .and_then(|n| String::from_utf16(&slice[..n]).ok())
        } else {
            None
        };
        WTSFreeMemory(ptr.cast());
        value
    }
}
/// A run-as process can share a console session with a different logged-in principal.
pub(super) fn console_user(session: u32) -> Option<String> {
    let user = field(session, WTSUserName)?;
    let domain = field(session, WTSDomainName)?;
    if user.is_empty() || domain.is_empty() {
        return None;
    }
    let account: Vec<u16> = format!("{domain}\\{user}")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let token = User::current().ok()?;
    let (mut sid_len, mut domain_len, mut kind) = (0, 0, 0);
    // SAFETY: Query account SID/domain sizes then fill bounded buffers and compare to the process token SID.
    let same = unsafe {
        LookupAccountNameW(
            null(),
            account.as_ptr(),
            null_mut(),
            &mut sid_len,
            null_mut(),
            &mut domain_len,
            &mut kind,
        );
        if sid_len == 0 || sid_len > 256 || domain_len > 1024 {
            return None;
        }
        let mut sid = vec![0usize; (sid_len as usize).div_ceil(std::mem::size_of::<usize>())];
        let mut domain = vec![0u16; domain_len as usize];
        LookupAccountNameW(
            null(),
            account.as_ptr(),
            sid.as_mut_ptr().cast(),
            &mut sid_len,
            domain.as_mut_ptr(),
            &mut domain_len,
            &mut kind,
        ) != 0
            && EqualSid(token.sid(), sid.as_mut_ptr().cast()) != 0
    };
    if !same {
        return None;
    }
    Some(format!("{}:{session}", token.id().ok()?))
}
