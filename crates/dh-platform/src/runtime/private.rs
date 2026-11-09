use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Isolate all SQLite journals, keys and daemon control files under one private directory.
pub fn private_directory(parent: &Path) -> io::Result<PathBuf> {
    let parent = checked_parent(parent)?;
    let root = parent.join("deskharbor-private");
    secure(&root)?;
    Ok(root)
}
#[cfg(windows)]
mod access;
#[cfg(target_os = "macos")]
mod darwin;
fn denied() -> io::Error {
    io::ErrorKind::PermissionDenied.into()
}
fn checked_parent(parent: &Path) -> io::Result<PathBuf> {
    let absolute = std::path::absolute(parent)?;
    if absolute
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(denied());
    }
    let mut physical = PathBuf::new();
    for part in absolute.ancestors().collect::<Vec<_>>().into_iter().rev() {
        if let Some(name) = part.file_name() {
            physical.push(name)
        } else {
            physical = part.into()
        }
        let m = match fs::symlink_metadata(&physical) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                #[cfg(unix)]
                let created = {
                    use std::os::unix::fs::DirBuilderExt;
                    fs::DirBuilder::new().mode(0o700).create(&physical)
                };
                #[cfg(windows)]
                let created = fs::create_dir(&physical);
                match created {
                    Ok(()) => (),
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
                    Err(e) => return Err(e),
                }
                fs::symlink_metadata(&physical)?
            }
            Err(e) => return Err(e),
        };
        // macOS has root-owned redirects such as /var -> /private/var. Only follow one from
        // an already verified parent, then verify every directory in the resolved chain.
        #[cfg(unix)]
        if m.is_symlink() {
            use std::os::unix::fs::MetadataExt;
            if m.uid() != 0 {
                return Err(denied());
            }
            physical = fs::canonicalize(&physical)?;
            for a in physical.ancestors() {
                check_dir(a, &fs::symlink_metadata(a)?)?;
            }
            continue;
        }
        check_dir(&physical, &m)?;
    }
    Ok(physical)
}
#[cfg(unix)]
fn check_dir(_path: &Path, m: &fs::Metadata) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: Query only the process's current effective UID.
    let uid = unsafe { libc::geteuid() };
    if !m.is_dir()
        || (m.uid() != uid && m.uid() != 0)
        || (m.mode() & 0o022 != 0 && m.mode() & 0o1000 == 0)
    {
        return Err(denied());
    }
    #[cfg(target_os = "macos")]
    darwin::check(_path, m, true)?;
    Ok(())
}
#[cfg(windows)]
fn check_dir(path: &Path, m: &fs::Metadata) -> io::Result<()> {
    if !m.is_dir() {
        return Err(denied());
    }
    access::check(path, true)
}
impl crate::PrivateFiles {
    pub fn open(parent: &Path) -> io::Result<Self> {
        let root = private_directory(parent)?;
        let files = Self { root };
        for entry in fs::read_dir(&files.root)? {
            let entry = entry?;
            let name = entry.file_name();
            files.checked(name.to_str().ok_or_else(denied)?)?;
        }
        Ok(files)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// A long-lived daemon must stop using a storage path if a later ACL change makes it replaceable.
    pub fn revalidate(&self) -> io::Result<()> {
        for ancestor in self
            .root
            .ancestors()
            .skip(1)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            check_dir(ancestor, &fs::symlink_metadata(ancestor)?)?;
        }
        verify_private_root(&self.root)
    }
    pub fn checked(&self, name: &str) -> io::Result<PathBuf> {
        if name.is_empty() || Path::new(name).file_name() != Some(std::ffi::OsStr::new(name)) {
            return Err(denied());
        }
        let path = self.root.join(name);
        let m = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(path),
            Err(e) => return Err(e),
        };
        if !m.is_file() {
            return Err(denied());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            // SAFETY: The returned effective UID identifies the application owner.
            let owner = unsafe { libc::geteuid() };
            if m.uid() != owner
                || m.nlink() != 1
                || m.mode() & 0o022 != 0
                || (name == "user.key" && (m.mode() & 0o077 != 0 || m.len() != 32))
            {
                return Err(denied());
            }
        }
        #[cfg(target_os = "macos")]
        match darwin::check(&path, &m, false) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
        #[cfg(windows)]
        match access::check(&path, false) {
            Ok(()) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
        Ok(path)
    }
}

#[cfg(unix)]
fn secure(root: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    match fs::DirBuilder::new().mode(0o700).create(root) {
        Ok(()) => (),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e),
    }
    verify_private_root(root)
}
#[cfg(unix)]
fn verify_private_root(root: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let m = fs::symlink_metadata(root)?;
    // SAFETY: geteuid queries the current process without arguments or side effects.
    if !m.is_dir() || m.mode() & 0o077 != 0 || m.uid() != unsafe { libc::geteuid() } {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    #[cfg(target_os = "macos")]
    darwin::check(root, &m, false)?;
    Ok(())
}
#[cfg(windows)]
fn secure(root: &Path) -> io::Result<()> {
    windows_root(root, true)
}
#[cfg(windows)]
fn verify_private_root(root: &Path) -> io::Result<()> {
    windows_root(root, false)
}
#[cfg(windows)]
fn windows_root(root: &Path, create: bool) -> io::Result<()> {
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
    if create && unsafe { CreateDirectoryW(name.as_ptr(), &attr) } == 0 {
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
