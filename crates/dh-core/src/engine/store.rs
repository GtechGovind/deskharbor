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
    pub root: PathBuf,
}
pub(super) struct Snapshot {
    pub revision: u64,
    pub settings: HostSettings,
    pub password: Option<Vec<u8>>,
    pub directory_identity: Option<String>,
}
fn revision(row: &rusqlite::Row<'_>) -> rusqlite::Result<u64> {
    let n = row.get::<_, i64>(0)?;
    u64::try_from(n).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, n))
}
impl Config {
    pub fn new(parent: &Path) -> EngineResult<Self> {
        let root = dh_platform::runtime::private_directory(parent).map_err(|_| storage())?;
        let cfg = Self { root };
        cfg.db()?.execute_batch("CREATE TABLE IF NOT EXISTS config(id INTEGER PRIMARY KEY CHECK(id=1),revision INTEGER NOT NULL,settings TEXT NOT NULL,password BLOB,key BLOB,cert BLOB); CREATE TABLE IF NOT EXISTS directory_grant(id INTEGER PRIMARY KEY CHECK(id=1), stamp TEXT NOT NULL); CREATE TABLE IF NOT EXISTS live(id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL, info TEXT NOT NULL);").map_err(|_|storage())?;
        cfg.db()?
            .execute(
                "INSERT OR IGNORE INTO config(id,revision,settings) VALUES(1,1,?1)",
                [serde_json::to_string(&HostSettings::default()).map_err(|_| storage())?],
            )
            .map_err(|_| storage())?;
        Ok(cfg)
    }
    fn db(&self) -> EngineResult<Connection> {
        let c = Connection::open(self.root.join("host.sqlite3")).map_err(|_| storage())?;
        c.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| storage())?;
        Ok(c)
    }
    pub fn snapshot(&self) -> EngineResult<Snapshot> {
        let (revision, settings, password,directory_identity): (u64, String, Option<Vec<u8>>,Option<String>) = self
            .db()?
            .query_row(
                "SELECT revision,settings,password,(SELECT stamp FROM directory_grant WHERE id=1) FROM config WHERE id=1",
                [],
                |r| Ok((revision(r)?, r.get(1)?, r.get(2)?,r.get(3)?)),
            )
            .map_err(|_| storage())?;
        Ok(Snapshot {
            revision,
            settings: serde_json::from_str(&settings).map_err(|_| storage())?,
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
                .is_some_and(|p| Path::new(p).is_absolute() && Path::new(p).is_dir())
        {
            return Err(err(
                "directory_required",
                "Select an existing absolute transfer directory",
            ));
        }
        let grant = if s.files {
            let path = s.transfer_directory.as_deref().ok_or_else(invalid)?;
            Some(
                dh_transfer::GrantedDirectory::open(Path::new(path)).map_err(|_| {
                    err(
                        "directory_required",
                        "Could not open the selected directory",
                    )
                })?,
            )
        } else {
            None
        };
        let mut db = self.db()?;
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        tx.execute("DELETE FROM directory_grant", [])
            .map_err(|_| storage())?;
        if let Some(g) = grant {
            tx.execute("INSERT INTO directory_grant VALUES(1,?1)", [g.identity()])
                .map_err(|_| storage())?;
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
        let path = self.root.join("user.key");
        let guard = self.lock_file("key.lock")?;
        guard.lock().map_err(|_| storage())?;
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(mut f) => {
                *key = dh_auth::random_bytes();
                f.write_all(&*key)
                    .and_then(|_| f.sync_all())
                    .map_err(|_| storage())?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                File::open(path)
                    .and_then(|mut f| f.read_exact(&mut *key))
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
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.root.join(name))
            .map_err(|_| storage())
    }
    pub fn daemon_alive(&self) -> EngineResult<bool> {
        match self.lock_file("daemon.lock")?.try_lock() {
            Ok(()) => Ok(false),
            Err(std::fs::TryLockError::WouldBlock) => Ok(true),
            Err(_) => Err(storage()),
        }
    }
    pub fn clear_status(&self) -> EngineResult<()> {
        self.db()?
            .execute("DELETE FROM live", [])
            .map_err(|_| storage())?;
        Ok(())
    }
    pub fn publish(&self, revision: u64, h: &HostInfo) -> EngineResult<()> {
        self.db()?
            .execute(
                "INSERT OR REPLACE INTO live VALUES(1,?1,?2)",
                params![
                    i64::try_from(revision).map_err(|_| storage())?,
                    serde_json::to_string(h).map_err(|_| storage())?
                ],
            )
            .map_err(|_| storage())?;
        Ok(())
    }
    pub fn status(&self) -> EngineResult<(u64, HostInfo)> {
        let s = self.snapshot()?;
        let alive = self.daemon_alive()?;
        if alive
            && let Some((r, i)) = self
                .db()?
                .query_row("SELECT revision,info FROM live WHERE id=1", [], |row| {
                    Ok((revision(row)?, row.get::<_, String>(1)?))
                })
                .optional()
                .map_err(|_| storage())?
        {
            return Ok((r, serde_json::from_str(&i).map_err(|_| storage())?));
        }
        Ok((
            if alive { 0 } else { s.revision },
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
