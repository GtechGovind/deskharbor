use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Isolate all SQLite journals, keys and daemon control files under one private directory.
pub fn private_directory(parent: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(parent)?;
    let root = parent.join("deskharbor-private");
    secure(&root)?;
    Ok(root)
}

#[cfg(unix)]
fn secure(root: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    match fs::DirBuilder::new().mode(0o700).create(root) {
        Ok(()) => (),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e),
    }
    let m = fs::symlink_metadata(root)?;
    // SAFETY: geteuid queries the current process without arguments or side effects.
    if !m.is_dir() || m.mode() & 0o077 != 0 || m.uid() != unsafe { libc::geteuid() } {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}
#[cfg(windows)]
fn secure(root: &Path) -> io::Result<()> {
    use std::{
        os::windows::{ffi::OsStrExt, fs::MetadataExt},
        ptr::null_mut,
    };
    use windows_sys::Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, LocalFree},
        Security::{Authorization::*, *},
        Storage::FileSystem::{CreateDirectoryW, FILE_ATTRIBUTE_REPARSE_POINT},
    };
    let name: Vec<u16> = root.as_os_str().encode_wide().chain(Some(0)).collect();
    struct Local(*mut std::ffi::c_void);
    impl Drop for Local {
        fn drop(&mut self) {
            // SAFETY: Returned security descriptors and SID strings are LocalAlloc allocations.
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    let user = super::user::User::current()?;
    let sid = user.sid();
    let sid_name = user.id()?;
    let sddl: Vec<u16> = format!("O:{sid_name}D:P(A;OICI;FA;;;{sid_name})")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut descriptor = null_mut();
    // SAFETY: sddl is null-terminated; descriptor receives an owned LocalAlloc pointer.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let _desired = Local(descriptor);
    let attr = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    // SAFETY: Both the path and security descriptor remain valid through CreateDirectoryW.
    if unsafe { CreateDirectoryW(name.as_ptr(), &attr) } == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_ALREADY_EXISTS as i32) {
            return Err(error);
        }
    }
    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let (mut actual, mut owner, mut acl) = (null_mut(), null_mut(), null_mut());
    // SAFETY: Request owner and ACL backed by actual; own and free the returned descriptor once.
    let code = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut acl,
            null_mut(),
            &mut actual,
        )
    };
    if code != 0 {
        return Err(io::Error::from_raw_os_error(code as i32));
    }
    let _actual = Local(actual);
    let (mut present, mut expected, mut defaulted) = (0, null_mut(), 0);
    // SAFETY: Both descriptors are valid Win32 objects. AclSize bounds the ACL allocation.
    let valid = unsafe {
        !owner.is_null()
            && EqualSid(sid, owner) != 0
            && !acl.is_null()
            && GetSecurityDescriptorDacl(descriptor, &mut present, &mut expected, &mut defaulted)
                != 0
            && !expected.is_null()
            && present != 0
            && std::slice::from_raw_parts(acl.cast::<u8>(), (*acl).AclSize as usize)
                == std::slice::from_raw_parts(expected.cast::<u8>(), (*expected).AclSize as usize)
    };
    if !valid {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}
