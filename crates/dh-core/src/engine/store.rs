use super::types::*;
use dh_auth::HostCredentials;
use dh_net::Identity;
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

pub(super) struct Config {
    files: dh_platform::PrivateFiles,
    anchor: dh_transfer::GrantedDirectory,
}
/// Only a daemon ever locks daemon.lock. Each invocation owns a fresh, independently probed lease.
pub(super) struct DaemonLease {
    cfg: std::sync::Arc<Config>,
    _owner: File,
    held: Option<File>,
    instance: String,
}
impl DaemonLease {
    pub fn publish(&self, revision: u64, info: &HostInfo) -> EngineResult<()> {
        self.cfg
            .db()?
            .execute(
                "INSERT OR REPLACE INTO daemon_status VALUES(1,?1,?2,?3)",
                params![
                    self.instance,
                    i64::try_from(revision).map_err(|_| storage())?,
                    serde_json::to_string(info).map_err(|_| storage())?
                ],
            )
            .map_err(|_| storage())?;
        Ok(())
    }
}
impl Drop for DaemonLease {
    fn drop(&mut self) {
        if let Ok(db) = self.cfg.db() {
            let _ = db.execute(
                "DELETE FROM daemon_status WHERE instance=?1",
                [&self.instance],
            );
        }
        self.held.take();
        if let Ok(path) = self.cfg.lease_path(&self.instance) {
            let _ = std::fs::remove_file(path);
        }
    }
}
pub(super) struct Snapshot {
    pub revision: u64,
    pub settings: HostSettings,
    pub password: Option<Vec<u8>>,
    pub directory_identity: Option<String>,
}
fn owner_file_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}
fn revision(row: &rusqlite::Row<'_>) -> rusqlite::Result<u64> {
    let n = row.get::<_, i64>(0)?;
    u64::try_from(n).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, n))
}
fn save_directory(tx: &rusqlite::Transaction<'_>, path: &str) -> EngineResult<()> {
    let directory = dh_transfer::GrantedDirectory::open(Path::new(path)).map_err(|_| {
        err(
            "directory_required",
            "Select an existing absolute transfer directory",
        )
    })?;
    tx.execute(
        "INSERT OR REPLACE INTO approved_directory VALUES(1,?1,?2)",
        params![path, directory.identity()],
    )
    .map_err(|_| storage())?;
    Ok(())
}
#[cfg(test)]
mod review;
impl Config {
    pub fn new(parent: &Path) -> EngineResult<Self> {
        let files = dh_platform::PrivateFiles::open(parent).map_err(|error| {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                err(
                    "unsafe_storage",
                    "Choose a data directory that other users cannot replace or modify.",
                )
            } else {
                storage()
            }
        })?;
        let anchor = dh_transfer::GrantedDirectory::open(files.root()).map_err(|_| storage())?;
        let cfg = Self { files, anchor };
        let mut db = cfg.db()?;
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS config (
                id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL, settings TEXT NOT NULL,
                password BLOB, key BLOB, cert BLOB
            );
            CREATE TABLE IF NOT EXISTS directory_grant (id INTEGER PRIMARY KEY CHECK(id=1), stamp TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS approved_directory (
                id INTEGER PRIMARY KEY CHECK(id=1), path TEXT NOT NULL, stamp TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS daemon_status (
                id INTEGER PRIMARY KEY CHECK(id=1), instance TEXT NOT NULL, revision INTEGER NOT NULL, info TEXT NOT NULL
            );"
        ).map_err(|_| storage())?;
        tx.execute(
            "INSERT OR IGNORE INTO config(id,revision,settings) VALUES(1,1,?1)",
            [serde_json::to_string(&HostSettings::default()).map_err(|_| storage())?],
        )
        .map_err(|_| storage())?;
        // Carry forward the registered object, never reopen its name as a migration.
        let legacy: Option<(String,String)> = tx.query_row(
            "SELECT settings, stamp FROM config, directory_grant WHERE config.id=1 AND directory_grant.id=1",
            [],
            |row| Ok((row.get(0)?,row.get(1)?)),
        ).optional().map_err(|_| storage())?;
        if let Some((settings, stamp)) = legacy {
            let old: HostSettings = serde_json::from_str(&settings).map_err(|_| storage())?;
            if let Some(path) = old.transfer_directory {
                tx.execute(
                    "INSERT OR IGNORE INTO approved_directory VALUES(1,?1,?2)",
                    params![path, stamp],
                )
                .map_err(|_| storage())?;
            }
            tx.execute("DELETE FROM directory_grant", [])
                .map_err(|_| storage())?;
        }
        tx.commit().map_err(|_| storage())?;
        Ok(cfg)
    }
    fn db(&self) -> EngineResult<Connection> {
        let c = Connection::open(self.database("host.sqlite3")?).map_err(|_| storage())?;
        c.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| storage())?;
        Ok(c)
    }
    fn file(&self, name: &str) -> EngineResult<PathBuf> {
        self.files.revalidate().map_err(|_| storage())?;
        let current =
            dh_transfer::GrantedDirectory::open(self.files.root()).map_err(|_| storage())?;
        if current.identity() != self.anchor.identity() {
            return Err(storage());
        }
        self.files.checked(name).map_err(|_| storage())
    }
    pub fn database(&self, name: &str) -> EngineResult<PathBuf> {
        let path = self.file(name)?;
        for suffix in ["-journal", "-wal", "-shm"] {
            self.files
                .checked(&format!("{name}{suffix}"))
                .map_err(|_| storage())?;
        }
        Ok(path)
    }
    pub fn snapshot(&self) -> EngineResult<Snapshot> {
        let (revision, settings, password, approved) = self
            .db()?
            .query_row(
                "SELECT revision,settings,password,path,stamp FROM config LEFT JOIN approved_directory ON approved_directory.id=config.id WHERE config.id=1",
                [],
                |r| Ok((revision(r)?,r.get::<_,String>(1)?,r.get::<_,Option<Vec<u8>>>(2)?,r.get::<_,Option<String>>(3)?.zip(r.get::<_,Option<String>>(4)?))),
            )
            .map_err(|_| storage())?;
        let settings: HostSettings = serde_json::from_str(&settings).map_err(|_| storage())?;
        let directory_identity = approved
            .filter(|(path, _)| Some(path) == settings.transfer_directory.as_ref())
            .map(|(_, id)| id);
        Ok(Snapshot {
            revision,
            settings,
            password,
            directory_identity,
        })
    }
    pub fn configure(&self, s: &HostSettings) -> EngineResult<u64> {
        if s.bind_address.parse::<std::net::SocketAddr>().is_err() {
            return Err(err("invalid_address", "Enter a local IP address and port"));
        }
        if s.files
            && !s
                .transfer_directory
                .as_ref()
                .is_some_and(|p| Path::new(p).is_absolute())
        {
            return Err(err(
                "directory_required",
                "Select an existing absolute transfer directory",
            ));
        }
        let mut db = self.db()?;
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        if s.files {
            let requested = s.transfer_directory.as_deref().ok_or_else(invalid)?;
            let approved: Option<String> = tx
                .query_row("SELECT path FROM approved_directory WHERE id=1", [], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(|_| storage())?;
            match approved.as_deref() {
                None => save_directory(&tx, requested)?, // Initial explicit path supplied by the existing settings API.
                Some(path) if path == requested => {} // Existing object identity survives every settings edit.
                Some(_) => {
                    return Err(err(
                        "directory_required",
                        "Select and grant the new transfer directory explicitly",
                    ));
                }
            }
        }
        let rev = tx
            .query_row(
                "UPDATE config SET settings=?1,revision=revision+1 WHERE id=1 RETURNING revision",
                [serde_json::to_string(s).map_err(|_| storage())?],
                revision,
            )
            .map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())?;
        Ok(rev)
    }
    pub fn grant_directory(&self, path: &str) -> EngineResult<u64> {
        let mut db = self.db()?;
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        save_directory(&tx, path)?;
        let json: String = tx
            .query_row("SELECT settings FROM config WHERE id=1", [], |r| r.get(0))
            .map_err(|_| storage())?;
        let mut settings: HostSettings = serde_json::from_str(&json).map_err(|_| storage())?;
        settings.transfer_directory = Some(path.to_owned());
        let rev = tx
            .query_row(
                "UPDATE config SET settings=?1,revision=revision+1 WHERE id=1 RETURNING revision",
                [serde_json::to_string(&settings).map_err(|_| storage())?],
                revision,
            )
            .map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())?;
        Ok(rev)
    }
    pub fn password(&self, cred: HostCredentials) -> EngineResult<u64> {
        let blob = self.protect(&cred.serialize())?;
        self.db()?
            .query_row(
                "UPDATE config SET password=?1,revision=revision+1 WHERE id=1 RETURNING revision",
                [blob],
                revision,
            )
            .map_err(|_| storage())
    }
    pub fn identity(&self) -> EngineResult<Identity> {
        let mut db = self.db()?;
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        let saved: Option<(Vec<u8>, Vec<u8>)> = tx
            .query_row(
                "SELECT cert,key FROM config WHERE key IS NOT NULL",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|_| storage())?;
        if let Some((cert, key)) = saved {
            return Identity::from_parts(cert, self.unprotect(&key)?).map_err(Into::into);
        }
        let identity = Identity::generate()?;
        tx.execute(
            "UPDATE config SET key=?1,cert=?2 WHERE id=1",
            params![
                self.protect(identity.private_key())?,
                identity.certificate()
            ],
        )
        .map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())?;
        Ok(identity)
    }
    pub fn credential(&self, blob: &[u8]) -> EngineResult<HostCredentials> {
        HostCredentials::deserialize(&self.unprotect(blob)?).map_err(|_| storage())
    }
    #[cfg(windows)]
    fn protect(&self, data: &[u8]) -> EngineResult<Vec<u8>> {
        use dh_auth::secret_store::{SecretStore, UserSecretStore};
        UserSecretStore.protect(data).map_err(|_| storage())
    }
    #[cfg(windows)]
    fn unprotect(&self, data: &[u8]) -> EngineResult<Zeroizing<Vec<u8>>> {
        use dh_auth::secret_store::{SecretStore, UserSecretStore};
        UserSecretStore.unprotect(data).map_err(|_| storage())
    }
    #[cfg(unix)]
    fn key(&self) -> EngineResult<ring::aead::LessSafeKey> {
        use ring::aead::{CHACHA20_POLY1305, LessSafeKey, UnboundKey};
        use std::io::{Read, Write};
        use std::os::unix::fs::OpenOptionsExt;
        let mut key = Zeroizing::new([0u8; 32]);
        let guard = self.lock_file("key.lock")?;
        guard.lock().map_err(|_| storage())?;
        let path = self.file("user.key")?;
        match File::open(&path) {
            Ok(mut f) => f.read_exact(&mut *key).map_err(|_| storage())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // The per-owner key lock serializes publication. A failed write never installs a partial key.
                let pending = self.file("user.key.pending")?;
                let mut f = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .open(&pending)
                    .map_err(|_| storage())?;
                *key = dh_auth::random_bytes();
                f.write_all(&*key)
                    .and_then(|_| f.sync_all())
                    .map_err(|_| storage())?;
                drop(f);
                std::fs::rename(pending, path).map_err(|_| storage())?;
                File::open(self.files.root())
                    .and_then(|directory| directory.sync_all())
                    .map_err(|_| storage())?;
            }
            Err(_) => return Err(storage()),
        }
        Ok(LessSafeKey::new(
            UnboundKey::new(&CHACHA20_POLY1305, &*key).map_err(|_| storage())?,
        ))
    }
    #[cfg(unix)]
    fn protect(&self, data: &[u8]) -> EngineResult<Vec<u8>> {
        use ring::aead::{Aad, Nonce};
        let nonce = dh_auth::random_bytes::<12>();
        let mut b = Zeroizing::new(data.to_vec());
        self.key()?
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(b"DeskHarbor owner store v1"),
                &mut *b,
            )
            .map_err(|_| storage())?;
        Ok([nonce.as_slice(), b.as_slice()].concat())
    }
    #[cfg(unix)]
    fn unprotect(&self, data: &[u8]) -> EngineResult<Zeroizing<Vec<u8>>> {
        use ring::aead::{Aad, Nonce};
        if data.len() < 28 {
            return Err(storage());
        }
        let mut b = Zeroizing::new(data[12..].to_vec());
        let n = self
            .key()?
            .open_in_place(
                Nonce::try_assume_unique_for_key(&data[..12]).map_err(|_| storage())?,
                Aad::from(b"DeskHarbor owner store v1"),
                &mut b,
            )
            .map_err(|_| storage())?
            .len();
        b.truncate(n);
        Ok(b)
    }
    pub fn lock_file(&self, name: &str) -> EngineResult<File> {
        owner_file_options()
            .create(true)
            .truncate(false)
            .open(self.file(name)?)
            .map_err(|_| storage())
    }
    fn lease_path(&self, instance: &str) -> EngineResult<PathBuf> {
        if instance.len() != 32 || !instance.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            return Err(storage());
        }
        self.file(&format!("daemon-{instance}.lease"))
    }
    pub fn acquire_daemon(self: &std::sync::Arc<Self>) -> EngineResult<DaemonLease> {
        let owner = self.lock_file("daemon.lock")?;
        owner
            .try_lock()
            .map_err(|_| err("host_busy", "Another daemon owns this data directory"))?;
        // The old process is gone. Retire stale rows/files without letting an old reader block startup.
        self.db()?
            .execute("DELETE FROM daemon_status", [])
            .map_err(|_| storage())?;
        for entry in std::fs::read_dir(self.files.root()).map_err(|_| storage())? {
            let entry = entry.map_err(|_| storage())?;
            let name = entry.file_name();
            let Some(instance) = name
                .to_str()
                .and_then(|n| n.strip_prefix("daemon-"))
                .and_then(|n| n.strip_suffix(".lease"))
            else {
                continue;
            };
            if let Ok(path) = self.lease_path(instance) {
                let _ = std::fs::remove_file(path);
            }
        }
        let instance = hex(&dh_auth::random_bytes::<16>());
        let held = owner_file_options()
            .create_new(true)
            .open(self.lease_path(&instance)?)
            .map_err(|_| storage())?;
        held.try_lock().map_err(|_| storage())?;
        let lease = DaemonLease {
            cfg: self.clone(),
            _owner: owner,
            held: Some(held),
            instance,
        };
        let snapshot = self.snapshot()?;
        lease.publish(
            0,
            &HostInfo {
                state: "stopped".into(),
                running: false,
                password_configured: snapshot.password.is_some(),
                unattended_available: false,
                addresses: vec![],
                detail: "Host daemon is starting".into(),
            },
        )?;
        Ok(lease)
    }
    fn instance_alive(&self, instance: &str) -> EngineResult<bool> {
        let file = match File::open(self.lease_path(instance)?) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(_) => return Err(storage()),
        };
        match file.try_lock_shared() {
            Ok(()) => Ok(false),
            Err(std::fs::TryLockError::WouldBlock) => Ok(true),
            Err(_) => Err(storage()),
        }
    }
    pub fn status(&self) -> EngineResult<(u64, HostInfo)> {
        let s = self.snapshot()?;
        if let Some((instance, r, i)) = self
            .db()?
            .query_row(
                "SELECT revision,info,instance FROM daemon_status WHERE id=1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(2)?,
                        revision(row)?,
                        row.get::<_, String>(1)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| storage())?
            && self.instance_alive(&instance)?
        {
            return Ok((r, serde_json::from_str(&i).map_err(|_| storage())?));
        }
        Ok((
            s.revision,
            HostInfo {
                state: "stopped".into(),
                running: false,
                password_configured: s.password.is_some(),
                unattended_available: false,
                addresses: vec![],
                detail: if s.settings.enabled {
                    "Start dh-hostd under the active user account"
                } else {
                    "Hosting is disabled"
                }
                .into(),
            },
        ))
    }
}
