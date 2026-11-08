//! Per-user protected secrets for development hosting. Installed machine-service
//! credentials require a separate broker-owned, ACL-restricted machine store.
use std::io;
use zeroize::Zeroizing;
pub trait SecretStore {
    fn protect(&self, plaintext: &[u8]) -> io::Result<Vec<u8>>;
    fn unprotect(&self, ciphertext: &[u8]) -> io::Result<Zeroizing<Vec<u8>>>;
}
pub struct UserSecretStore;
#[cfg(not(windows))]
impl SecretStore for UserSecretStore {
    fn protect(&self, _: &[u8]) -> io::Result<Vec<u8>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn unprotect(&self, _: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[cfg(windows)]
impl SecretStore for UserSecretStore {
    fn protect(&self, plaintext: &[u8]) -> io::Result<Vec<u8>> {
        crypt(plaintext, false).map(|v| v.to_vec())
    }
    fn unprotect(&self, ciphertext: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
        crypt(ciphertext, true)
    }
}

#[cfg(windows)]
fn crypt(input: &[u8], decrypt: bool) -> io::Result<Zeroizing<Vec<u8>>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    };
    use zeroize::Zeroize;
    if input.is_empty() || input.len() > 64 * 1024 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let blob = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // SAFETY: Input is a valid borrowed byte slice for the call; no prompt or
    // optional parameters are supplied. DPAPI initializes and owns output.
    let ok = unsafe {
        if decrypt {
            CryptUnprotectData(
                &blob,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptProtectData(
                &blob,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: Successful DPAPI returns cbData initialized bytes in a LocalAlloc
    // allocation. Copy into a zeroizing owner, erase and release that allocation.
    let result = unsafe {
        let bytes = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let result = Zeroizing::new(bytes.to_vec());
        bytes.zeroize();
        LocalFree(output.pbData.cast());
        result
    };
    Ok(result)
}
