use super::{server::HostBackend, *};
use crate::{Broker, HostState, Operation, PolicyError};
use dh_auth::{Binding, HostCredentials};
use dh_clipboard::{ClipboardSync, Content, Update};
use dh_net::Channel;
use dh_protocol::{AttachmentTicket, Command, Display, HostStatus, envelope::Body};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

pub(super) struct Host {
    pub backend: HostBackend,
    pub settings: HostSettings,
    pub directory: Option<dh_transfer::GrantedDirectory>,
    credential: Arc<HostCredentials>,
    pub data: Mutex<Data>,
}
pub(super) struct Data {
    pub broker: Broker,
    displays: Vec<Display>,
    pub selected: String,
    detail: String,
    tickets: HashMap<String, Ticket>,
    clip: ClipboardSync,
    synthetic_clip: String,
    pub bulk_busy: bool,
    revoked: bool,
}
struct Ticket {
    key: Zeroizing<[u8; 32]>,
    binding: Binding,
    epoch: u64,
    layout: u64,
    expires: Instant,
}
pub(super) struct Status {
    pub state: String,
    pub detail: String,
}
fn observed(backend: HostBackend) -> (HostState, Vec<Display>, String) {
    if backend == HostBackend::Synthetic {
        return (
            HostState::Ready {
                user: "synthetic-loopback-user".into(),
            },
            vec![Display {
                id: "synthetic:0".into(),
                name: "Synthetic diagnostic display".into(),
                width: 64,
                height: 48,
                x: 0,
                y: 0,
            }],
            "Explicit synthetic loopback diagnostics. No OS screen, input or clipboard access."
                .into(),
        );
    }
    let n = dh_platform::runtime::inspect();
    let state = match (n.state, n.user) {
        ("ready", Some(user)) => HostState::Ready { user },
        ("no_display", _) => HostState::NoDisplay,
        ("sign_in", _) => HostState::SignIn,
        _ => HostState::PermissionNeeded,
    };
    (
        state,
        n.displays
            .into_iter()
            .map(|d| Display {
                id: d.id,
                name: d.name,
                width: d.width,
                height: d.height,
                x: d.x,
                y: d.y,
            })
            .collect(),
        n.detail.into(),
    )
}
fn label(s: &HostState) -> &'static str {
    match s {
        HostState::Ready { .. } => "ready",
        HostState::Locked | HostState::SignIn => "sign_in",
        HostState::NoDisplay => "no_display",
        HostState::PermissionNeeded => "permission_needed",
    }
}
pub(super) fn policy(e: PolicyError) -> EngineError {
    let (c, m) = match e {
        PolicyError::Busy => ("host_busy", "Host already has a controller"),
        PolicyError::Unauthorized => ("disconnected", "Session authority was revoked"),
        PolicyError::StaleEpoch => ("stale_epoch", "Session epoch has changed"),
        PolicyError::StaleLayout => ("stale_layout", "Display layout has changed"),
        PolicyError::PermissionDenied => ("permission_denied", "Operation is not granted"),
        PolicyError::Unavailable => (
            "permission_needed",
            "Required native capability is unavailable",
        ),
        PolicyError::InvalidInput => ("invalid_input", "Invalid input event"),
    };
    err(c, m)
}
impl Host {
    pub fn new(
        backend: HostBackend,
        settings: HostSettings,
        credential: HostCredentials,
        revision: u64,
        directory: Option<dh_transfer::GrantedDirectory>,
    ) -> Self {
        let (state, displays, detail) = observed(backend);
        Self {
            backend,
            settings,
            directory,
            credential: Arc::new(credential),
            data: Mutex::new(Data {
                broker: Broker::with_epoch(state, revision),
                selected: displays.first().map(|d| d.id.clone()).unwrap_or_default(),
                displays,
                detail,
                tickets: HashMap::new(),
                clip: ClipboardSync::new(dh_auth::random_bytes()),
                synthetic_clip: String::new(),
                bulk_busy: false,
                revoked: false,
            }),
        }
    }
    pub fn refresh(&self) -> std::sync::MutexGuard<'_, Data> {
        let mut d = self.data.lock().unwrap();
        let (s, displays, detail) = observed(self.backend);
        if d.broker.state() != &s {
            d.broker.transition(s);
            d.synthetic_clip.clear();
            d.clip = ClipboardSync::new(dh_auth::random_bytes());
            d.tickets.clear();
        }
        if d.displays != displays {
            d.broker.replace_layout();
            d.tickets.clear();
            d.selected = displays.first().map(|d| d.id.clone()).unwrap_or_default();
            d.displays = displays;
        }
        d.detail = detail;
        d
    }
    pub fn revoke(&self) {
        let mut d = self.data.lock().unwrap();
        d.revoked = true;
        d.broker.revoke_all();
        d.tickets.clear();
    }
    pub fn status(&self) -> Status {
        let d = self.refresh();
        Status {
            state: label(d.broker.state()).into(),
            detail: d.detail.clone(),
        }
    }
}
impl Data {
    pub fn check(
        &self,
        b: &Binding,
        operation: Operation,
        epoch: u64,
        layout: u64,
    ) -> EngineResult<()> {
        let grant = self.broker.refresh(b).map_err(policy)?;
        self.broker
            .check(&grant, b, operation, epoch, layout)
            .map_err(policy)
    }
    fn status(&self, synthetic: bool) -> Body {
        Body::HostStatus(HostStatus {
            state: label(self.broker.state()).into(),
            epoch: self.broker.epoch(),
            layout: self.broker.layout(),
            displays: self.displays.clone(),
            synthetic,
        })
    }
}
struct Connected(Arc<Host>, Binding);
impl Drop for Connected {
    fn drop(&mut self) {
        let mut d = self.0.data.lock().unwrap();
        d.broker.disconnect(&self.1);
        d.tickets.retain(|_, t| t.binding != self.1);
    }
}
fn failure(error: EngineError) -> Body {
    Body::Failure(dh_protocol::Failure {
        code: error.code,
        message: error.message,
    })
}
pub(super) async fn serve(host: Arc<Host>, mut channel: Channel) -> EngineResult<()> {
    let first = channel.receive().await?;
    match first.body {
        Some(Body::Attach(a)) => {
            let (binding, epoch, layout) = {
                let mut d = host.refresh();
                let t = d
                    .tickets
                    .get(&a.role)
                    .filter(|t| t.expires > Instant::now())
                    .ok_or_else(offline)?;
                channel.verify_attachment(&a.role, &t.key, &a.proof)?;
                d.check(&t.binding, role_op(&a.role)?, t.epoch, t.layout)?;
                let t = d.tickets.remove(&a.role).expect("verified ticket");
                (t.binding, t.epoch, t.layout)
            };
            channel.send(msg(0, cmd("attached", "", epoch))).await?;
            match a.role.as_str() {
                "media" => media(host, channel, binding, epoch, layout).await,
                "bulk" => super::bulk::receive(host, channel, binding, epoch, layout).await,
                _ => Err(invalid()),
            }
        }
        Some(Body::Command(c)) if c.name == "control" => {
            let proof = channel.authenticate_server(host.credential.clone()).await?;
            let b = proof.binding().clone();
            let result = {
                let mut d = host.refresh();
                let mut ops = vec![Operation::View];
                if host.settings.allow_input {
                    ops.push(Operation::Input)
                }
                if host.settings.clipboard {
                    ops.push(Operation::Clipboard)
                }
                if host.settings.files {
                    ops.push(Operation::Files)
                }
                if d.revoked {
                    Err(offline())
                } else {
                    d.broker
                        .authorize(proof, ops)
                        .map_err(policy)
                        .map(|_| d.status(host.backend == HostBackend::Synthetic))
                }
            };
            let reply = match result {
                Ok(s) => s,
                Err(e) => {
                    channel.send(msg(0, failure(e))).await?;
                    let _ = channel.receive().await;
                    return Ok(());
                }
            };
            let _lease = Connected(host.clone(), b.clone());
            channel.send(msg(0, reply)).await?;
            loop {
                let env = channel.receive().await?;
                let body = env.body.ok_or_else(invalid)?;
                let close = matches!(&body,Body::Command(c) if c.name=="disconnect");
                let response = control(&host, &b, body).unwrap_or_else(failure);
                channel.send(msg(env.request_id, response)).await?;
                if close {
                    let _ = channel.receive().await;
                    return Ok(());
                }
            }
        }
        _ => Err(invalid()),
    }
}
fn role_op(role: &str) -> EngineResult<Operation> {
    match role {
        "media" => Ok(Operation::View),
        "bulk" => Ok(Operation::Files),
        _ => Err(invalid()),
    }
}
fn control(host: &Host, b: &Binding, body: Body) -> EngineResult<Body> {
    let mut d = host.refresh();
    let grant = d.broker.refresh(b).map_err(policy)?;
    let epoch = d.broker.epoch();
    let layout = d.broker.layout();
    let ok = cmd("ok", "", epoch);
    match body {
        Body::Command(Command {
            name,
            argument,
            epoch: request_epoch,
        }) => match name.as_str() {
            "status" => Ok(d.status(host.backend == HostBackend::Synthetic)),
            "disconnect" => {
                d.broker.disconnect(b);
                d.tickets.clear();
                Ok(ok)
            }
            "ticket" => {
                let op = role_op(&argument)?;
                d.check(b, op, request_epoch, layout)?;
                if argument == "bulk" && d.bulk_busy {
                    return Err(err("busy", "A file transfer is active"));
                }
                let key = Zeroizing::new(dh_auth::random_bytes());
                let result = Body::AttachmentTicket(AttachmentTicket {
                    role: argument.clone(),
                    secret: key.to_vec(),
                });
                d.tickets.insert(
                    argument,
                    Ticket {
                        key,
                        binding: b.clone(),
                        epoch,
                        layout,
                        expires: Instant::now() + Duration::from_secs(10),
                    },
                );
                Ok(result)
            }
            "display" => {
                d.check(b, Operation::View, request_epoch, layout)?;
                if !d.displays.iter().any(|v| v.id == argument) {
                    return Err(err("no_display", "Unknown monitor"));
                }
                if argument != d.selected {
                    d.broker.replace_layout();
                    d.tickets.clear();
                    d.selected = argument
                }
                Ok(d.status(host.backend == HostBackend::Synthetic))
            }
            "clipboard_get" => {
                d.check(b, Operation::Clipboard, request_epoch, layout)?;
                let text = if host.backend == HostBackend::Synthetic {
                    d.synthetic_clip.clone()
                } else {
                    dh_platform::runtime::clipboard_read()
                        .map_err(|_| err("permission_needed", "User clipboard could not be read"))?
                };
                Ok(Body::Clipboard(dh_protocol::Clipboard {
                    data: text.into_bytes(),
                    mime: "text/plain".into(),
                    epoch,
                    ..Default::default()
                }))
            }
            _ => Err(invalid()),
        },
        Body::Clipboard(c) => {
            d.check(b, Operation::Clipboard, c.epoch, layout)?;
            if c.mime != "text/plain" {
                return Err(invalid());
            }
            let update = Update {
                origin: c.origin.try_into().map_err(|_| invalid())?,
                generation: c.generation,
                content: Content::Text(String::from_utf8(c.data).map_err(|_| invalid())?),
            };
            let mut pending = d.clip.clone();
            if let Some(Content::Text(text)) = pending.receive(update).map_err(|_| invalid())? {
                if host.backend == HostBackend::Synthetic {
                    d.synthetic_clip = text
                } else {
                    dh_platform::runtime::clipboard_write(&text).map_err(|_| {
                        err("permission_needed", "User clipboard could not be written")
                    })?
                }
            }
            d.clip = pending;
            Ok(ok)
        }
        Body::Input(input) => {
            d.check(b, Operation::Input, input.epoch, input.layout)?;
            if d.selected != input.display_id {
                return Err(err("stale_layout", "Input targets a different monitor"));
            }
            if host.backend != HostBackend::Synthetic {
                return Err(err("permission_needed", "Native input worker unavailable"));
            }
            use dh_protocol::input::Kind;
            match input.kind.ok_or_else(invalid)? {
                Kind::Key(k) => d
                    .broker
                    .key(
                        &grant,
                        b,
                        input.epoch,
                        input.layout,
                        k.usb_hid_usage.try_into().map_err(|_| invalid())?,
                        k.pressed,
                    )
                    .map_err(policy)?,
                Kind::Pointer(p) => d
                    .broker
                    .pointer(&grant, b, input.epoch, input.layout, p.x, p.y, p.buttons)
                    .map_err(policy)?,
                Kind::Text(t) if t.len() <= 8192 && !t.contains('\0') => (),
                Kind::ReleaseAll(true) => {
                    d.broker.release_all();
                }
                _ => return Err(invalid()),
            }
            Ok(ok)
        }
        _ => Err(invalid()),
    }
}
async fn media(
    host: Arc<Host>,
    mut ch: Channel,
    b: Binding,
    epoch: u64,
    layout: u64,
) -> EngineResult<()> {
    use dh_media::codecs::vp8::Encoder;
    let mut encoder: Option<Encoder> = None;
    let mut sequence = 0u64;
    loop {
        let req = ch.receive().await?;
        if !matches!(req.body,Some(Body::Command(c)) if c.name=="frame") {
            return Err(invalid());
        }
        let (id, w, h) = {
            let d = host.refresh();
            d.check(&b, Operation::View, epoch, layout)?;
            let v = d
                .displays
                .iter()
                .find(|v| v.id == d.selected)
                .ok_or_else(invalid)?;
            (v.id.clone(), v.width, v.height)
        };
        let pixels = if host.backend == HostBackend::Synthetic {
            let mut pixels = vec![0u8; (w * h * 4) as usize];
            for (x, p) in pixels.chunks_exact_mut(4).enumerate() {
                p.copy_from_slice(&[((x % w as usize) * 3) as u8, 128, sequence as u8, 255]);
            }
            pixels
        } else {
            let f = dh_platform::runtime::capture(&id)
                .map_err(|_| err("permission_needed", "Capture is unavailable"))?;
            if f.width != w || f.height != h {
                return Err(invalid());
            }
            f.pixels_rgba
        };
        let codec = || err("codec", "VP8 encoding failed");
        if encoder.is_none() {
            encoder = Some(Encoder::new(w, h, 1600).map_err(|_| codec())?)
        }
        let f = encoder
            .as_mut()
            .unwrap()
            .encode(&pixels, sequence.is_multiple_of(60))
            .map_err(|_| codec())?;
        host.refresh().check(&b, Operation::View, epoch, layout)?;
        sequence += 1;
        ch.send(msg(
            req.request_id,
            Body::VideoFrame(dh_protocol::VideoFrame {
                epoch,
                layout,
                display_id: id,
                sequence,
                codec: "vp8".into(),
                keyframe: f.keyframe,
                width: w,
                height: h,
                data: f.data,
            }),
        ))
        .await?;
    }
}
