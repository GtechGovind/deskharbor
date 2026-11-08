mod directory;
use cap_std::fs::{Dir, OpenOptions};
pub use directory::GrantedDirectory;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
pub const CHUNK_SIZE: usize = 1024 * 1024;
#[derive(Clone, Debug)]
pub struct UserContext {
    pub user: String,
    pub epoch: u64,
    pub interactive: bool,
}
pub struct Offer {
    pub filename: String,
    pub size: u64,
    pub checksum: [u8; 32],
}
#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    #[error("file I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid offer, chunk or offset")]
    Invalid,
    #[error("checksum mismatch")]
    Checksum,
    #[error("transfer paused or user context changed")]
    Context,
    #[error("destination already exists; overwrite approval required")]
    Collision,
}
/// The local user agent supplies this directory and OS-derived context after
/// broker authorization. Neither is accepted directly from a network offer.
/// Partial contents live in a private anonymous tempfile, retained across
/// connection interruptions while this receiver remains alive.
pub struct Receiver {
    root: Dir,
    file: File,
    offer: Offer,
    context: UserContext,
    offset: u64,
    paused: bool,
    committed: bool,
}
impl Receiver {
    pub fn begin(
        root: impl AsRef<Path>,
        offer: Offer,
        context: UserContext,
    ) -> Result<Self, TransferError> {
        Self::begin_in(
            Dir::open_ambient_dir(root, cap_std::ambient_authority())?,
            offer,
            context,
        )
    }
    fn begin_in(root: Dir, offer: Offer, context: UserContext) -> Result<Self, TransferError> {
        validate_name(&offer.filename)?;
        if offer.size > 8 * 1024 * 1024 * 1024 * 1024 || !valid_context(&context) {
            return Err(TransferError::Invalid);
        }
        let file = tempfile::tempfile()?;
        Ok(Self {
            root,
            file,
            offer,
            context,
            offset: 0,
            paused: false,
            committed: false,
        })
    }
    fn check(&self, context: &UserContext) -> Result<(), TransferError> {
        if self.paused
            || self.committed
            || !valid_context(context)
            || context.user != self.context.user
            || context.epoch != self.context.epoch
        {
            return Err(TransferError::Context);
        }
        Ok(())
    }
    pub fn write_chunk(
        &mut self,
        context: &UserContext,
        offset: u64,
        bytes: &[u8],
        checksum: [u8; 32],
    ) -> Result<u64, TransferError> {
        self.check(context)?;
        let end = offset
            .checked_add(bytes.len() as u64)
            .ok_or(TransferError::Invalid)?;
        if bytes.is_empty()
            || bytes.len() > CHUNK_SIZE
            || offset != self.offset
            || end > self.offer.size
        {
            return Err(TransferError::Invalid);
        }
        if *blake3::hash(bytes).as_bytes() != checksum {
            return Err(TransferError::Checksum);
        }
        self.file.seek(SeekFrom::Start(offset))?;
        if let Err(error) = self.file.write_all(bytes) {
            self.paused = true;
            return Err(error.into());
        }
        self.offset = end;
        Ok(end)
    }
    pub fn pause(&mut self) {
        self.paused = true;
    }
    pub fn resume(&mut self, context: UserContext) -> Result<u64, TransferError> {
        if self.committed
            || !valid_context(&context)
            || context.user != self.context.user
            || context.epoch < self.context.epoch
        {
            return Err(TransferError::Context);
        }
        self.file.set_len(self.offset)?;
        self.file.sync_all()?;
        self.context = context;
        self.paused = false;
        Ok(self.offset)
    }
    /// Overwrite must be approved locally for this filename, not supplied as a
    /// peer-controlled flag. Collision errors preserve the receiver for approval.
    pub fn finish(
        &mut self,
        context: &UserContext,
        overwrite_approved: bool,
    ) -> Result<(), TransferError> {
        self.finish_authorized(context, overwrite_approved, || Ok(()))
    }
    /// Recheck live authority while verifying and staging, then hold it through atomic publication.
    pub fn finish_authorized<G>(
        &mut self,
        context: &UserContext,
        overwrite_approved: bool,
        mut guard: impl FnMut() -> Result<G, TransferError>,
    ) -> Result<(), TransferError> {
        self.check(context)?;
        if self.offset != self.offer.size || self.file.metadata()?.len() != self.offer.size {
            return Err(TransferError::Invalid);
        }
        self.file.seek(SeekFrom::Start(0))?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = vec![0; CHUNK_SIZE];
        loop {
            let n = self.file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            let _authority = guard()?;
            hasher.update(&buffer[..n]);
        }
        if *hasher.finalize().as_bytes() != self.offer.checksum {
            return Err(TransferError::Checksum);
        }
        let mut random = [0; 16];
        getrandom::fill(&mut random).map_err(|_| TransferError::Invalid)?;
        let suffix = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let staging = format!(".deskharbor-{suffix}.part");
        let mut output = {
            let _authority = guard()?;
            self.root
                .open_with(&staging, OpenOptions::new().write(true).create_new(true))?
        };
        let result = (|| -> Result<(), TransferError> {
            self.file.seek(SeekFrom::Start(0))?;
            let mut staging_buffer = vec![0; CHUNK_SIZE];
            loop {
                let n = self.file.read(&mut staging_buffer)?;
                if n == 0 {
                    break;
                }
                let _authority = guard()?;
                output.write_all(&staging_buffer[..n])?;
            }
            output.sync_all()?;
            Ok(())
        })();
        drop(output);
        if let Err(error) = result {
            let _ = self.root.remove_file(&staging);
            return Err(error);
        }
        let _authority = match guard() {
            Ok(g) => g,
            Err(e) => {
                let _ = self.root.remove_file(&staging);
                return Err(e);
            }
        };
        let published = if overwrite_approved {
            self.root.rename(&staging, &self.root, &self.offer.filename)
        } else {
            self.root
                .hard_link(&staging, &self.root, &self.offer.filename)
        };
        // A leaf rename replaces a symlink entry; it never follows its target.
        // A hard-link publication atomically refuses every existing entry.
        if let Err(error) = published {
            let _ = self.root.remove_file(&staging);
            return if error.kind() == std::io::ErrorKind::AlreadyExists
                || self.root.symlink_metadata(&self.offer.filename).is_ok()
            {
                Err(TransferError::Collision)
            } else {
                Err(error.into())
            };
        }
        self.committed = true;
        if !overwrite_approved {
            let _ = self.root.remove_file(&staging);
        }
        Ok(())
    }
    pub fn cancel(self) -> Result<(), TransferError> {
        drop(self);
        Ok(())
    }
}
fn valid_context(context: &UserContext) -> bool {
    context.interactive
        && context.epoch > 0
        && !context.user.is_empty()
        && context.user.len() <= 256
}
fn validate_name(name: &str) -> Result<(), TransferError> {
    if name.is_empty()
        || name.len() > 240
        || name == "."
        || name == ".."
        || name.ends_with(['.', ' '])
        || name.starts_with(".deskharbor-")
        || name
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
    {
        return Err(TransferError::Invalid);
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ]
    .contains(&stem.as_str())
    {
        return Err(TransferError::Invalid);
    }
    Ok(())
}
