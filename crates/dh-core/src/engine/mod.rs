mod bulk;
mod client;
mod host_session;
mod server;
mod store;
mod types;
use dh_media::codecs::vp8::DecodedFrame;
use dh_protocol::{Command, Envelope, envelope::Body};
pub use server::{Daemon, HostBackend};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};
use tokio::sync::{broadcast, mpsc, oneshot};
pub use types::*;
use types::{err, invalid, storage};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct DeskEngine(Arc<Inner>);
struct Inner {
    cfg: store::Config,
    devices: Mutex<dh_storage::Store>,
    events: broadcast::Sender<EngineEvent>,
    sessions: Mutex<HashMap<u32, Arc<ClientSession>>>,
    transfers: Mutex<HashMap<u32, bulk::TransferRecord>>,
    next: AtomicU32,
}
struct ClientSession {
    info: Mutex<SessionInfo>,
    tx: mpsc::Sender<(Body, oneshot::Sender<EngineResult<Body>>)>,
    frame: Mutex<Option<Arc<DecodedFrame>>>,
    address: std::net::SocketAddr,
    pin: [u8; 32],
    quic: bool,
    origin: [u8; 16],
    generation: std::sync::atomic::AtomicU64,
}
fn next_id(id: &AtomicU32) -> EngineResult<u32> {
    id.try_update(Ordering::Relaxed, Ordering::Relaxed, |i| i.checked_add(1))
        .map_err(|_| err("id_exhausted", "Identifier space exhausted"))
}
fn cmd(name: &str, arg: &str, epoch: u64) -> Body {
    Body::Command(Command {
        name: name.into(),
        argument: arg.into(),
        epoch,
    })
}
fn msg(id: u64, body: Body) -> Envelope {
    Envelope {
        request_id: id,
        body: Some(body),
    }
}
fn offline() -> EngineError {
    err("disconnected", "Session has disconnected")
}
fn status_event(e: &DeskEngine, h: HostInfo) -> HostInfo {
    let _ = e.0.events.send(EngineEvent::HostChanged(h.clone()));
    h
}
impl DeskEngine {
    pub fn new(data_dir: PathBuf) -> EngineResult<Self> {
        let cfg = store::Config::new(&data_dir)?;
        let store =
            dh_storage::Store::open(cfg.root.join("devices.sqlite3")).map_err(|_| storage())?;
        Ok(Self(Arc::new(Inner {
            cfg,
            devices: Mutex::new(store),
            events: broadcast::channel(128).0,
            sessions: Mutex::default(),
            transfers: Mutex::default(),
            next: AtomicU32::new(1),
        })))
    }
    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.0.events.subscribe()
    }
    pub fn latest_frame(&self, session_id: u32) -> Option<Arc<DecodedFrame>> {
        self.session(session_id).ok()?.frame.lock().unwrap().clone()
    }
    fn session(&self, id: u32) -> EngineResult<Arc<ClientSession>> {
        self.0
            .sessions
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(offline)
    }
    pub fn devices(&self) -> EngineResult<Vec<SavedDevice>> {
        Ok(self
            .0
            .devices
            .lock()
            .unwrap()
            .devices()
            .map_err(|_| storage())?
            .into_iter()
            .map(|d| SavedDevice {
                name: d.name,
                address: d.address,
                identity: d.identity.map(|i| hex(&i)),
            })
            .collect())
    }
    pub fn save_device(&self, d: SavedDevice) -> EngineResult<()> {
        let id = d.identity.as_deref().map(unhex).transpose()?;
        self.0
            .devices
            .lock()
            .unwrap()
            .save_device(dh_storage::Device {
                name: d.name,
                address: d.address,
                identity: id,
            })
            .map_err(|e| match e {
                dh_storage::StorageError::IdentityChanged => err(
                    "identity_changed",
                    "Forget the saved device before changing its identity",
                ),
                dh_storage::StorageError::Invalid => invalid(),
                _ => storage(),
            })
    }
    pub fn forget_device(&self, address: String) -> EngineResult<()> {
        self.0
            .devices
            .lock()
            .unwrap()
            .forget_device(&address)
            .map_err(|_| storage())
    }
    pub async fn host_status(&self) -> EngineResult<HostInfo> {
        Ok(self.0.cfg.status()?.1)
    }
    async fn wait_config(&self, rev: u64) -> EngineResult<HostInfo> {
        tokio::time::timeout(Duration::from_secs(12), async {
            loop {
                let (r, h) = self.0.cfg.status()?;
                if r >= rev {
                    return Ok(status_event(self, h));
                }
                tokio::time::sleep(Duration::from_millis(40)).await;
            }
        })
        .await
        .map_err(|_| {
            err(
                "timeout",
                "Configuration saved. Host daemon has not acknowledged it",
            )
        })?
    }
    pub async fn configure_host(&self, settings: HostSettings) -> EngineResult<HostInfo> {
        let r = self.0.cfg.configure(&settings)?;
        self.wait_config(r).await
    }
    pub async fn set_host_password(&self, password: Zeroizing<Vec<u8>>) -> EngineResult<()> {
        let cred =
            tokio::task::spawn_blocking(move || dh_auth::HostCredentials::register(&password))
                .await
                .map_err(|_| storage())?
                .map_err(|_| {
                    err(
                        "invalid_password",
                        "Host passwords must contain 12 to 1024 bytes",
                    )
                })?;
        let rev = self.0.cfg.password(cred)?;
        self.wait_config(rev).await?;
        Ok(())
    }
    pub fn transfers(&self) -> Vec<TransferInfo> {
        let mut t = self
            .0
            .transfers
            .lock()
            .unwrap()
            .values()
            .map(|t| t.info.clone())
            .collect::<Vec<_>>();
        t.sort_by_key(|t| t.id);
        t
    }
}
impl ClientSession {
    async fn rpc(&self, body: Body) -> EngineResult<Body> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send((body, tx)).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => err("busy", "Session command queue is full"),
            _ => offline(),
        })?;
        rx.await.map_err(|_| offline())?
    }
}
