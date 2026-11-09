use super::{Path, denied, fs, io};
use std::{
    mem::MaybeUninit,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::{GENERIC_ALL, GENERIC_WRITE, LocalFree},
    Security::{
        self,
        Authorization::{ConvertSidToStringSidW, GetSecurityInfo, SE_FILE_OBJECT},
    },
    Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_DELETE_CHILD, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_READ_ATTRIBUTES, FILE_WRITE_ATTRIBUTES, FILE_WRITE_EA, GetFileInformationByHandle,
        READ_CONTROL, WRITE_DAC, WRITE_OWNER,
    },
};

struct Local(*mut std::ffi::c_void);
impl Drop for Local {
    fn drop(&mut self) {
        // SAFETY: Values wrapped here are owned LocalAlloc security descriptors or SID strings.
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn trusted(sid: Security::PSID, user: &super::super::user::User) -> bool {
    if sid.is_null() {
        return false;
    }
    // SAFETY: The security descriptor owns each SID and user keeps its token SID live.
    if unsafe {
        Security::EqualSid(sid, user.sid()) != 0
            || Security::IsWellKnownSid(sid, Security::WinLocalSystemSid) != 0
            || Security::IsWellKnownSid(sid, Security::WinBuiltinAdministratorsSid) != 0
    } {
        return true;
    }
    // The Windows volume root may be owned by the TrustedInstaller service.
    let mut p = null_mut();
    // SAFETY: ConvertSidToStringSidW returns an owned null-terminated SID on success.
    if unsafe { ConvertSidToStringSidW(sid, &mut p) } == 0 {
        return false;
    }
    let _text = Local(p.cast());
    let expected = "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464";
    // SAFETY: Read up to the terminator returned by the OS; both allocation and SID remain valid.
    unsafe {
        let mut i = 0;
        for c in expected.encode_utf16() {
            if *p.add(i) != c {
                return false;
            }
            i += 1;
        }
        *p.add(i) == 0
    }
}

/// Ancestors may grant read/list and create-subdirectory rights (e.g. the Windows volume root),
/// but no untrusted principal may replace protected descendants or change security metadata.
/// Sensitive leaves may grant access only to the owner, administrators and OS service principals.
pub(super) fn check(path: &Path, directory: bool) -> io::Result<()> {
    let user = super::super::user::User::current()?;
    let file = fs::OpenOptions::new()
        .access_mode(READ_CONTROL | FILE_READ_ATTRIBUTES)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let mut info = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: The handle is borrowed and the output receives one initialized information record.
    let info = unsafe {
        if GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) == 0 {
            return Err(io::Error::last_os_error());
        }
        info.assume_init()
    };
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory
        || (!directory && info.nNumberOfLinks != 1)
    {
        return Err(denied());
    }
    let (mut owner, mut acl, mut descriptor) = (null_mut(), null_mut(), null_mut());
    // SAFETY: Win32 provides the owner and DACL within a single returned LocalAlloc descriptor.
    let code = unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            Security::OWNER_SECURITY_INFORMATION | Security::DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut acl,
            null_mut(),
            &mut descriptor,
        )
    };
    if code != 0 {
        return Err(io::Error::from_raw_os_error(code as i32));
    }
    let _descriptor = Local(descriptor);
    if !trusted(owner, &user) || acl.is_null() {
        return Err(denied());
    }
    let forbidden = if directory {
        GENERIC_ALL
            | GENERIC_WRITE
            | DELETE
            | FILE_DELETE_CHILD
            | WRITE_DAC
            | WRITE_OWNER
            | FILE_WRITE_ATTRIBUTES
            | FILE_WRITE_EA
    } else {
        u32::MAX
    };
    // SAFETY: GetSecurityInfo returned a present DACL in the still-owned descriptor.
    let count = unsafe { (*acl).AceCount };
    for index in 0..count {
        let mut raw = null_mut();
        // SAFETY: GetAce bounds the index and returns an ACE belonging to this descriptor.
        if unsafe { Security::GetAce(acl, index.into(), &mut raw) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: Every DACL ACE begins with an ACE_HEADER.
        let header = unsafe { &*raw.cast::<Security::ACE_HEADER>() };
        const ALLOW: u8 = 0;
        const DENY: u8 = 1;
        if u32::from(header.AceFlags) & Security::INHERIT_ONLY_ACE != 0 || header.AceType == DENY {
            continue;
        }
        if header.AceType != ALLOW
            || (header.AceSize as usize) < std::mem::size_of::<Security::ACCESS_ALLOWED_ACE>()
        {
            return Err(denied());
        }
        // SAFETY: A standard allow ACE contains its access mask and variable-length SID at SidStart.
        let ace = unsafe { &*raw.cast::<Security::ACCESS_ALLOWED_ACE>() };
        let sid = std::ptr::addr_of!(ace.SidStart).cast_mut().cast();
        if ace.Mask & forbidden != 0 && !trusted(sid, &user) {
            return Err(denied());
        }
    }
    Ok(())
}
