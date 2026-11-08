use super::{Offer, Receiver, TransferError, UserContext};
use cap_std::fs::Dir;
use std::{io, path::Path};

/// Hold a directory descriptor so no request re-resolves a peer-controlled or replaced pathname.
pub struct GrantedDirectory {
    dir: Dir,
    identity: String,
}
impl GrantedDirectory {
    pub fn open(path: &Path) -> Result<Self, TransferError> {
        if !path.is_absolute() {
            return Err(TransferError::Invalid);
        }
        let dir = Dir::open_ambient_dir(path, cap_std::ambient_authority())?;
        let identity = identity(&dir)?;
        Ok(Self { dir, identity })
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn receive(&self, offer: Offer, user: UserContext) -> Result<Receiver, TransferError> {
        Receiver::begin_in(self.dir.try_clone()?, offer, user)
    }
}
#[cfg(unix)]
fn identity(dir: &Dir) -> io::Result<String> {
    use cap_std::fs::MetadataExt;
    let m = dir.dir_metadata()?;
    if m.ino() == 0 {
        return Err(io::ErrorKind::Unsupported.into());
    }
    Ok(format!("{}:{}", m.dev(), m.ino()))
}
#[cfg(windows)]
fn identity(dir: &Dir) -> io::Result<String> {
    use std::{mem::MaybeUninit, os::windows::io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    let mut info = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: Borrow a valid open directory handle and a writable information buffer.
    let info = unsafe {
        if GetFileInformationByHandle(dir.as_raw_handle(), info.as_mut_ptr()) == 0 {
            return Err(io::Error::last_os_error());
        }
        info.assume_init()
    };
    if info.nFileIndexHigh == 0 && info.nFileIndexLow == 0 {
        return Err(io::ErrorKind::Unsupported.into());
    }
    Ok(format!(
        "{}:{}:{}",
        info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow
    ))
}
