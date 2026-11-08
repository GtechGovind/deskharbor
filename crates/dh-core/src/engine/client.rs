use super::*;
use dh_net::{Channel, TransportKind};
use dh_protocol::{HostStatus, Input, input::Kind};
use std::{net::SocketAddr, sync::Weak};
use zeroize::Zeroizing;

async fn address(endpoint: &str) -> EngineResult<SocketAddr> {
    if endpoint.is_empty() || endpoint.len() > 512 || endpoint.chars().any(char::is_control) {
        return Err(invalid());
    }
    if let Ok(a) = endpoint.parse() {
        return Ok(a);
    }
    if let Ok(ip) = endpoint.parse() {
        return Ok(SocketAddr::new(ip, dh_protocol::DEFAULT_PORT));
    }
    let name = if endpoint.contains(':') {
        endpoint.into()
    } else {
        format!("{endpoint}:{}", dh_protocol::DEFAULT_PORT)
    };
    tokio::time::timeout(Duration::from_secs(5), tokio::net::lookup_host(name))
        .await
        .map_err(|_| err("timeout", "Address lookup timed out"))?
        .map_err(|_| err("invalid_address", "Could not resolve host address"))?
        .next()
        .ok_or_else(invalid)
}
pub(super) fn check_reply(body: Body) -> EngineResult<Body> {
    if let Body::Failure(f) = body {
        // Never echo arbitrary peer-controlled strings into the GUI, logs or diagnostics.
        let (code, message) = match f.code.as_str() {
            "host_busy" => ("host_busy", "Another controller is connected"),
            "permission_needed" => (
                "permission_needed",
                "Required user or native capability unavailable",
            ),
            "permission_denied" => ("permission_denied", "Operation is not granted"),
            "stale_epoch" => ("stale_epoch", "Session epoch changed"),
            "stale_layout" => ("stale_layout", "Monitor layout changed"),
            "collision" => ("collision", "A file already exists at the destination"),
            "busy" => ("busy", "Another operation is active"),
            "no_display" => ("no_display", "Monitor no longer available"),
            "disconnected" => ("disconnected", "Session was revoked"),
            _ => ("invalid_request", "Peer rejected the request"),
        };
        return Err(err(code, message));
    }
    Ok(body)
}
fn session_state(status: &HostStatus) -> EngineResult<&'static str> {
    match status.state.as_str() {
        "ready" => Ok("connected"),
        "permission_needed" => Ok("permission_needed"),
        "sign_in" => Ok("sign_in"),
        "no_display" => Ok("no_display"),
        _ => Err(invalid()),
    }
}
fn update(info: &mut SessionInfo, h: HostStatus) -> EngineResult<bool> {
    let state = session_state(&h)?;
    if h.epoch == 0
        || h.layout == 0
        || h.displays.len() > 64
        || h.displays.iter().any(|d| {
            d.id.is_empty()
                || d.id.len() > 512
                || d.width == 0
                || d.height == 0
                || d.width > 16384
                || d.height > 16384
        })
    {
        return Err(invalid());
    }
    let displays: Vec<_> = h.displays.into_iter().map(Into::into).collect();
    let changed = info.state != state
        || info.epoch != h.epoch
        || info.layout != h.layout
        || info.displays != displays;
    info.state = state.into();
    info.epoch = h.epoch;
    info.layout = h.layout;
    info.displays = displays;
    if !info.displays.iter().any(|d| d.id == info.selected_display) {
        info.selected_display = info
            .displays
            .first()
            .map(|d| d.id.clone())
            .unwrap_or_default();
    }
    Ok(changed)
}
impl DeskEngine {
    pub async fn connect(
        &self,
        endpoint: String,
        password: Zeroizing<Vec<u8>>,
    ) -> EngineResult<SessionInfo> {
        let id = next_id(&self.0.next)?;
        let mut info = SessionInfo {
            id,
            endpoint: endpoint.clone(),
            state: "connecting".into(),
            transport: String::new(),
            host_identity: String::new(),
            epoch: 0,
            layout: 0,
            displays: vec![],
            selected_display: String::new(),
        };
        let _ = self
            .0
            .events
            .send(EngineEvent::SessionChanged(info.clone()));
        let result = self.establish(&mut info, password).await;
        if let Err(e) = &result {
            info.state = if e.code == "host_busy" {
                "host_busy"
            } else {
                "disconnected"
            }
            .into();
            let _ = self.0.events.send(EngineEvent::SessionChanged(info));
            let _ = self.0.events.send(EngineEvent::Error {
                session_id: Some(id),
                code: e.code.clone(),
                message: e.message.clone(),
            });
        }
        result
    }
    async fn establish(
        &self,
        info: &mut SessionInfo,
        password: Zeroizing<Vec<u8>>,
    ) -> EngineResult<SessionInfo> {
        let address = address(&info.endpoint).await?;
        let saved = self
            .devices()?
            .into_iter()
            .find(|d| d.address == info.endpoint);
        let pin = saved
            .as_ref()
            .and_then(|d| d.identity.as_deref())
            .map(unhex)
            .transpose()?;
        let mut channel = dh_net::connect(address, pin, true).await?;
        let quic = channel.kind() == TransportKind::Quic;
        info.transport = if quic { "quic" } else { "tls_tcp" }.into();
        info.state = "authenticating".into();
        let _ = self
            .0
            .events
            .send(EngineEvent::SessionChanged(info.clone()));
        channel.send(msg(0, cmd("control", "", 0))).await?;
        channel.authenticate_client(&password).await?;
        drop(password);
        let pin = channel.host_identity();
        info.host_identity = hex(&pin);
        let ready = check_reply(channel.receive().await?.body.ok_or_else(invalid)?)?;
        let Body::HostStatus(status) = ready else {
            return Err(invalid());
        };
        update(info, status)?;
        if let Some(dev) = saved
            && dev.identity.is_none()
        {
            self.save_device(SavedDevice {
                identity: Some(info.host_identity.clone()),
                ..dev
            })?
        }
        let (tx, rx) = mpsc::channel(32);
        let s = Arc::new(ClientSession {
            tx,
            info: Mutex::new(info.clone()),
            frame: Mutex::default(),
            address,
            pin,
            quic,
            origin: dh_auth::random_bytes(),
            generation: std::sync::atomic::AtomicU64::new(1),
        });
        self.0.sessions.lock().unwrap().insert(info.id, s.clone());
        let _ = self
            .0
            .events
            .send(EngineEvent::SessionChanged(info.clone()));
        let weak = Arc::downgrade(&self.0);
        tokio::spawn(control_actor(weak.clone(), s.clone(), channel, rx));
        tokio::spawn(media_loop(weak, s));
        Ok(info.clone())
    }
    pub async fn disconnect(&self, id: u32) -> EngineResult<()> {
        let s = self.session(id)?;
        s.rpc(cmd("disconnect", "", 0)).await?;
        self.0.sessions.lock().unwrap().remove(&id);
        *s.frame.lock().unwrap() = None;
        Ok(())
    }
    pub async fn select_monitor(&self, id: u32, display_id: String) -> EngineResult<()> {
        let s = self.session(id)?;
        let epoch = s.info.lock().unwrap().epoch;
        let Body::HostStatus(h) = s.rpc(cmd("display", &display_id, epoch)).await? else {
            return Err(invalid());
        };
        let mut info = s.info.lock().unwrap();
        update(&mut info, h)?;
        info.selected_display = display_id;
        *s.frame.lock().unwrap() = None;
        let _ = self
            .0
            .events
            .send(EngineEvent::SessionChanged(info.clone()));
        Ok(())
    }
    pub async fn send_input(&self, id: u32, i: InputRequest) -> EngineResult<()> {
        let kind = match i.kind {
            InputKind::Pointer {
                x,
                y,
                buttons,
                wheel_x,
                wheel_y,
            } => Kind::Pointer(dh_protocol::Pointer {
                x,
                y,
                buttons,
                wheel_x,
                wheel_y,
            }),
            InputKind::Key { usage, pressed } => Kind::Key(dh_protocol::Key {
                usb_hid_usage: usage.into(),
                pressed,
            }),
            InputKind::Text(text) => Kind::Text(text),
            InputKind::ReleaseAll => Kind::ReleaseAll(true),
        };
        self.session(id)?
            .rpc(Body::Input(Input {
                epoch: i.epoch,
                layout: i.layout,
                display_id: i.display_id,
                kind: Some(kind),
            }))
            .await?;
        Ok(())
    }
    pub async fn clipboard_send_text(&self, id: u32, text: String) -> EngineResult<()> {
        let s = self.session(id)?;
        if text.len() > 1024 * 1024 || text.contains('\0') {
            return Err(invalid());
        }
        let epoch = s.info.lock().unwrap().epoch;
        let generation = s
            .generation
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| invalid())?;
        s.rpc(Body::Clipboard(dh_protocol::Clipboard {
            epoch,
            origin: s.origin.to_vec(),
            generation,
            mime: "text/plain".into(),
            data: text.into_bytes(),
        }))
        .await?;
        Ok(())
    }
    pub async fn clipboard_request_text(&self, id: u32) -> EngineResult<String> {
        let s = self.session(id)?;
        let epoch = s.info.lock().unwrap().epoch;
        let Body::Clipboard(c) = s.rpc(cmd("clipboard_get", "", epoch)).await? else {
            return Err(invalid());
        };
        if c.epoch != epoch || c.mime != "text/plain" || c.data.len() > 1024 * 1024 {
            return Err(invalid());
        }
        String::from_utf8(c.data).map_err(|_| invalid())
    }
}
type Requests = mpsc::Receiver<(Body, oneshot::Sender<EngineResult<Body>>)>;
async fn exchange(ch: &mut Channel, id: u64, body: Body) -> EngineResult<Body> {
    ch.send(msg(id, body)).await?;
    let e = ch.receive().await?;
    if e.request_id != id {
        return Err(invalid());
    }
    check_reply(e.body.ok_or_else(invalid)?)
}
async fn control_actor(
    engine: Weak<Inner>,
    s: Arc<ClientSession>,
    mut ch: Channel,
    mut rx: Requests,
) {
    let mut heartbeat = tokio::time::interval(Duration::from_secs(1));
    let mut sequence = 1;
    loop {
        let request = tokio::select! {r=rx.recv()=>match r{Some(r)=>Some(r),None=>break},_=heartbeat.tick()=>None};
        if engine.strong_count() == 0 {
            break;
        }
        let (body, tx) =
            request.map_or_else(|| (cmd("status", "", 0), None), |(b, t)| (b, Some(t)));
        let closing = matches!(&body,Body::Command(c) if c.name=="disconnect");
        let reply = exchange(&mut ch, sequence, body).await;
        sequence += 1;
        let broken = reply
            .as_ref()
            .is_err_and(|e| matches!(e.code.as_str(), "disconnected" | "timeout"));
        if tx.is_none()
            && let Ok(Body::HostStatus(h)) = &reply
        {
            let mut info = s.info.lock().unwrap();
            if update(&mut info, h.clone()).unwrap_or(true) {
                *s.frame.lock().unwrap() = None;
                if let Some(e) = engine.upgrade() {
                    let _ = e.events.send(EngineEvent::SessionChanged(info.clone()));
                }
            }
        }
        if let Some(tx) = tx {
            let _ = tx.send(reply);
        }
        if closing || broken {
            break;
        }
    }
    let mut info = s.info.lock().unwrap();
    info.state = "disconnected".into();
    *s.frame.lock().unwrap() = None;
    if let Some(e) = engine.upgrade() {
        e.sessions.lock().unwrap().remove(&info.id);
        let _ = e.events.send(EngineEvent::SessionChanged(info.clone()));
    }
}
pub(super) async fn attach(s: &ClientSession, role: &str, epoch: u64) -> EngineResult<Channel> {
    let Body::AttachmentTicket(mut ticket) = s.rpc(cmd("ticket", role, epoch)).await? else {
        return Err(invalid());
    };
    if ticket.role != role {
        return Err(invalid());
    }
    let bytes = Zeroizing::new(std::mem::take(&mut ticket.secret));
    let key: &[u8; 32] = bytes.as_slice().try_into().map_err(|_| invalid())?;
    let mut ch = dh_net::connect(s.address, Some(s.pin), s.quic).await?;
    ch.attach_client(role, key).await?;
    Ok(ch)
}
async fn media_loop(engine: Weak<Inner>, s: Arc<ClientSession>) {
    loop {
        let info = s.info.lock().unwrap().clone();
        if info.state == "disconnected" || engine.strong_count() == 0 {
            return;
        }
        if info.state == "connected" {
            let result = stream(&engine, &s, &info).await;
            if let Err(e) = result
                && let Some(engine) = engine.upgrade()
            {
                let _ = engine.events.send(EngineEvent::Error {
                    session_id: Some(info.id),
                    code: e.code,
                    message: e.message,
                });
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
async fn stream(engine: &Weak<Inner>, s: &ClientSession, start: &SessionInfo) -> EngineResult<()> {
    let mut ch = attach(s, "media", start.epoch).await?;
    let mut decoder = dh_media::codecs::vp8::Decoder::new()
        .map_err(|_| err("codec", "VP8 decoder unavailable"))?;
    let mut sequence = 0;
    loop {
        let Body::VideoFrame(v) =
            exchange(&mut ch, sequence + 1, cmd("frame", "", start.epoch)).await?
        else {
            return Err(invalid());
        };
        if v.codec != "vp8"
            || v.sequence <= sequence
            || v.epoch != start.epoch
            || v.layout != start.layout
            || v.display_id != start.selected_display
        {
            return Err(invalid());
        }
        let frame = decoder
            .decode(&v.data)
            .map_err(|_| err("codec", "Invalid VP8 frame"))?;
        if (frame.width, frame.height) != (v.width, v.height) {
            return Err(invalid());
        }
        sequence = v.sequence;
        {
            let info = s.info.lock().unwrap();
            if info.state != "connected"
                || info.layout != v.layout
                || info.epoch != v.epoch
                || info.selected_display != v.display_id
            {
                return Ok(());
            }
            *s.frame.lock().unwrap() = Some(Arc::new(frame));
            let Some(e) = engine.upgrade() else {
                return Ok(());
            };
            let _ = e.events.send(EngineEvent::FrameReady {
                session_id: info.id,
                sequence,
                width: v.width,
                height: v.height,
            });
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
