use std::path::Path;
#[derive(Debug, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    pub address: String,
    pub identity: Option<[u8; 32]>,
}
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database failure: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("host identity changed; forget and verify the device again")]
    IdentityChanged,
    #[error("invalid device")]
    Invalid,
    #[error("database is newer than this application")]
    NewerSchema,
}
pub struct Store {
    connection: rusqlite::Connection,
}
impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let mut connection = rusqlite::Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 1 {
            return Err(StorageError::NewerSchema);
        }
        if version == 0 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(include_str!("../migrations/0001_initial.sql"))?;
            transaction.pragma_update(None, "user_version", 1)?;
            transaction.commit()?;
        }
        connection.pragma_update(None, "foreign_keys", true)?;
        Ok(Self { connection })
    }
    pub fn save_device(&self, device: Device) -> Result<(), StorageError> {
        if device.name.trim().is_empty()
            || device.name.len() > 256
            || device.name.contains('\0')
            || device.address.is_empty()
            || device.address.len() > 512
            || device.address.chars().any(char::is_control)
        {
            return Err(StorageError::Invalid);
        }
        let count=self.connection.execute("INSERT INTO devices(name,address,identity) VALUES(?1,?2,?3) ON CONFLICT(address) DO UPDATE SET name=excluded.name,identity=excluded.identity WHERE devices.identity IS NULL OR devices.identity=excluded.identity",rusqlite::params![device.name,device.address,device.identity.as_ref().map(|id|id.as_slice())])?;
        if count != 1 {
            return Err(StorageError::IdentityChanged);
        }
        Ok(())
    }
    pub fn devices(&self) -> Result<Vec<Device>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT name,address,identity FROM devices ORDER BY name COLLATE NOCASE,address",
        )?;
        let devices = statement
            .query_map([], |row| {
                let identity: Option<Vec<u8>> = row.get(2)?;
                let identity = identity
                    .map(|bytes| bytes.try_into().map_err(|_| rusqlite::Error::InvalidQuery))
                    .transpose()?;
                Ok(Device {
                    name: row.get(0)?,
                    address: row.get(1)?,
                    identity,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(devices)
    }
    pub fn forget_device(&self, address: &str) -> Result<(), StorageError> {
        self.connection
            .execute("DELETE FROM devices WHERE address=?1", [address])?;
        Ok(())
    }
}
