//! Darwin directory deletion rights can be granted by extended ACLs independently
//! of mode bits. Use the descriptor security APIs declared by sys/fcntl.h and sys/acl.h.
use super::{Path, denied, fs, io};
use std::{
    ffi::c_void,
    mem::MaybeUninit,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    ptr::null_mut,
};

type Handle = *mut c_void;
unsafe extern "C" {
    fn filesec_init() -> Handle;
    fn filesec_free(spec: Handle);
    #[cfg_attr(target_arch = "x86_64", link_name = "fstatx_np$INODE64")]
    fn fstatx_np(fd: libc::c_int, stat: *mut libc::stat, spec: Handle) -> libc::c_int;
    fn filesec_query_property(spec: Handle, property: u32, valid: *mut libc::c_int) -> libc::c_int;
    fn filesec_get_property(spec: Handle, property: u32, value: Handle) -> libc::c_int;
    fn acl_free(acl: Handle) -> libc::c_int;
    fn acl_valid(acl: Handle) -> libc::c_int;
    fn acl_get_entry(acl: Handle, which: libc::c_int, out: *mut Handle) -> libc::c_int;
    fn acl_get_tag_type(entry: Handle, tag: *mut u32) -> libc::c_int;
    fn acl_get_flagset_np(entry: Handle, set: *mut Handle) -> libc::c_int;
    fn acl_get_flag_np(set: Handle, flag: u32) -> libc::c_int;
    fn acl_get_permset_mask_np(entry: Handle, mask: *mut u64) -> libc::c_int;
    fn acl_get_qualifier(entry: Handle) -> Handle;
    fn mbr_uid_to_uuid(uid: libc::uid_t, uuid: *mut u8) -> libc::c_int;
}
struct FileSecurity(Handle);
impl Drop for FileSecurity {
    fn drop(&mut self) {
        // SAFETY: filesec_init supplied the owned non-null descriptor.
        unsafe {
            filesec_free(self.0);
        }
    }
}
struct AclMemory(Handle);
impl Drop for AclMemory {
    fn drop(&mut self) {
        // SAFETY: These are owned ACL or qualifier allocations documented to use acl_free.
        unsafe {
            acl_free(self.0);
        }
    }
}
fn success(code: libc::c_int) -> io::Result<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
pub(super) fn check(path: &Path, m: &fs::Metadata, ancestor: bool) -> io::Result<()> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_EVTONLY)
        .open(path)?;
    // SAFETY: Initializes an independent security descriptor, without touching the file.
    let spec = unsafe { filesec_init() };
    if spec.is_null() {
        return Err(io::Error::last_os_error());
    }
    let spec = FileSecurity(spec);
    let mut stat = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstatx_np fills both outputs using a live descriptor and the platform stat ABI.
    let stat = unsafe {
        success(fstatx_np(file.as_raw_fd(), stat.as_mut_ptr(), spec.0))?;
        stat.assume_init()
    };
    if stat.st_dev as u64 != m.dev() || stat.st_ino != m.ino() {
        return Err(denied());
    }
    const FILESEC_ACL: u32 = 5;
    let mut present = 0;
    // SAFETY: Successful fstatx_np populated spec; a missing ACL property is distinct from a failed metadata read.
    unsafe {
        success(filesec_query_property(spec.0, FILESEC_ACL, &mut present))?;
    }
    if present == 0 {
        return Ok(());
    }
    let mut acl = null_mut();
    // SAFETY: FILESEC_ACL returns a separately owned acl_t.
    unsafe {
        success(filesec_get_property(
            spec.0,
            FILESEC_ACL,
            (&mut acl as *mut Handle).cast(),
        ))?;
    }
    if acl.is_null() || acl as usize == 1 {
        return Err(denied());
    }
    let acl = AclMemory(acl);
    // SAFETY: acl is the result of a successful query from an existing open file.
    unsafe {
        success(acl_valid(acl.0))?;
    }
    let mut which = 0; // ACL_FIRST_ENTRY
    loop {
        let mut entry = null_mut();
        // SAFETY: A validated ACL owns the returned borrowed entry. Darwin uses EINVAL at the end of iteration.
        if unsafe { acl_get_entry(acl.0, which, &mut entry) } != 0 {
            let error = io::Error::last_os_error();
            return if error.raw_os_error() == Some(libc::EINVAL) {
                Ok(())
            } else {
                Err(error)
            };
        }
        which = -1; // ACL_NEXT_ENTRY
        let mut tag = 0;
        let mut flags = null_mut();
        let mut mask = 0;
        // SAFETY: Entry pointers and flagsets are borrowed from the still-owned ACL.
        unsafe {
            success(acl_get_tag_type(entry, &mut tag))?;
            if tag == 2 {
                continue;
            } // ACL_EXTENDED_DENY cannot confer rights.
            if tag != 1 {
                return Err(denied());
            }
            success(acl_get_flagset_np(entry, &mut flags))?;
            match acl_get_flag_np(flags, 1 << 8) {
                1 => continue,
                0 => {}
                _ => return Err(io::Error::last_os_error()),
            }
            success(acl_get_permset_mask_np(entry, &mut mask))?;
        }
        // Ancestor add-file/add-subdirectory grants alone cannot replace an existing child.
        const REPLACE_OR_RESECURE: u64 =
            (1 << 4) | (1 << 6) | (1 << 8) | (1 << 10) | (1 << 12) | (1 << 13);
        if mask
            & (if ancestor {
                REPLACE_OR_RESECURE
            } else {
                u64::MAX
            })
            == 0
        {
            continue;
        }
        // SAFETY: An allow entry's qualifier is an owned 16-byte GUID. Membership UUID output is 16 bytes.
        let trusted = unsafe {
            let q = acl_get_qualifier(entry);
            if q.is_null() {
                return Err(io::Error::last_os_error());
            }
            let q = AclMemory(q);
            let principal = std::slice::from_raw_parts(q.0.cast::<u8>(), 16);
            let mut matches = false;
            for uid in [libc::geteuid(), 0] {
                let mut uuid = [0u8; 16];
                if mbr_uid_to_uuid(uid, uuid.as_mut_ptr()) != 0 {
                    return Err(denied());
                }
                matches |= principal == uuid;
            }
            matches
        };
        if !trusted {
            return Err(denied());
        }
    }
}
