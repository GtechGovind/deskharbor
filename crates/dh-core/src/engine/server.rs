use super::{
    host_session::{Host, serve},
    store::Snapshot,
    *,
};
use tokio::{
    sync::oneshot,
    task::{JoinHandle, JoinSet},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostBackend {
    Native,
    Synthetic,
}
pub struct Daemon {
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<EngineResult<()>>>,
}
impl Daemon {
    pub async fn stop(mut self) -> EngineResult<()> {
        self.stop.take();
        self.task
            .take()
            .expect("daemon handle")
            .await
            .map_err(|_| err("host_failed", "Host task failed"))?
    }
}
impl Drop for Daemon {
    fn drop(&mut self) {
        self.stop.take();
    }
}
impl DeskEngine {
    pub async fn start_daemon(&self, backend: HostBackend) -> EngineResult<Daemon> {
        let lock = self.0.cfg.acquire_daemon()?;
        let (tx, rx) = oneshot::channel();
        let (ready, started) = oneshot::channel();
        let engine = self.clone();
        let task = tokio::spawn(async move { run(engine, backend, lock, rx, ready).await });
        started
            .await
            .map_err(|_| err("host_failed", "Host task failed to start"))??;
        Ok(Daemon {
            stop: Some(tx),
            task: Some(task),
        })
    }
}
async fn run(
    engine: DeskEngine,
    backend: HostBackend,
    owner: store::DaemonLease,
    mut stop: oneshot::Receiver<()>,
    ready: oneshot::Sender<EngineResult<()>>,
) -> EngineResult<()> {
    let mut ready = Some(ready);
    loop {
        let snapshot = engine.0.cfg.snapshot()?;
        let active = activate(&engine, backend, &snapshot).await;
        let (mut listener, host, mut info) = match active {
            Ok(value) => value,
            Err(e) => {
                let info = HostInfo {
                    state: "permission_needed".into(),
                    running: false,
                    password_configured: snapshot.password.is_some(),
                    unattended_available: false,
                    addresses: vec![],
                    detail: e.message.clone(),
                };
                (None, None, info)
            }
        };
        let revision = snapshot.revision;
        owner.publish(revision, &info)?;
        status_event(&engine, info.clone());
        if let Some(ready) = ready.take() {
            let _ = ready.send(Ok(()));
        }
        let mut peers = JoinSet::new();
        let mut tick = tokio::time::interval(Duration::from_millis(200));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let stopping = loop {
            tokio::select! {
                _=&mut stop=>break true,
                _=tick.tick()=>{
                    if engine.0.cfg.snapshot()?.revision!=revision{break false}
                    if let Some(h)=&host{let s=h.status();if info.state!=s.state||info.detail!=s.detail{info.state=s.state;info.detail=s.detail;owner.publish(revision,&info)?; status_event(&engine,info.clone());}}
                },
                result=async{listener.as_ref().expect("enabled listener").incoming().await},if listener.is_some()&&peers.len()<16=>{
                    if let (Ok(handshake),Some(host))=(result,host.as_ref()){let host=host.clone(); peers.spawn(async move{if let Ok(ch)=handshake.await {let _=serve(host,ch).await;}});}
                },
                _=peers.join_next(),if !peers.is_empty()=>{}
            }
        };
        if let Some(host) = host {
            host.revoke();
        }
        peers.shutdown().await;
        listener.take();
        if stopping {
            return Ok(());
        }
    }
}
type Active = (Option<dh_net::Listener>, Option<Arc<Host>>, HostInfo);
async fn activate(e: &DeskEngine, backend: HostBackend, s: &Snapshot) -> EngineResult<Active> {
    let mut info = HostInfo {
        state: "stopped".into(),
        running: false,
        password_configured: s.password.is_some(),
        unattended_available: false,
        addresses: vec![],
        detail: "Hosting disabled".into(),
    };
    if !s.settings.enabled {
        return Ok((None, None, info));
    }
    if backend == HostBackend::Native && !dh_platform::ordinary_host_allowed() {
        return Err(err(
            "permission_needed",
            "Run the development host under an ordinary user account",
        ));
    }
    let Some(blob) = &s.password else {
        info.detail = "Set a host password before enabling the listener".into();
        return Ok((None, None, info));
    };
    let addr: std::net::SocketAddr = s.settings.bind_address.parse().map_err(|_| invalid())?;
    if backend == HostBackend::Synthetic && !addr.ip().is_loopback() {
        return Err(err(
            "invalid_address",
            "Synthetic diagnostics are restricted to loopback",
        ));
    }
    let directory = if s.settings.files {
        let denied = || {
            err(
                "directory_required",
                "The selected transfer directory changed or is unavailable. Select it again.",
            )
        };
        let path = s
            .settings
            .transfer_directory
            .as_deref()
            .ok_or_else(denied)?;
        let granted = dh_transfer::GrantedDirectory::open(std::path::Path::new(path))
            .map_err(|_| denied())?;
        if Some(granted.identity()) != s.directory_identity.as_deref() {
            return Err(denied());
        }
        Some(granted)
    } else {
        None
    };
    let host = Arc::new(Host::new(
        backend,
        s.settings.clone(),
        e.0.cfg.credential(blob)?,
        s.revision,
        directory,
    ));
    let listener = dh_net::Listener::bind(addr, e.0.cfg.identity()?, true).await?;
    let status = host.status();
    info.state = status.state;
    info.detail = status.detail;
    info.running = true;
    info.addresses = vec![listener.local_addr().to_string()];
    Ok((Some(listener), Some(host), info))
}
