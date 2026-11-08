use super::{
    client::{attach, check_reply},
    host_session::Host,
    *,
};
use dh_auth::Binding;
use dh_net::Channel;
use dh_protocol::{TransferChunk, TransferOffer, TransferState};
use dh_transfer::{CHUNK_SIZE, Offer, UserContext};
use std::{
    fs::File,
    io::{Read, Seek},
    sync::atomic::AtomicBool,
};
pub(super) struct TransferRecord {
    pub info: TransferInfo,
    cancel: Arc<AtomicBool>,
}
fn progress(e: &Inner, id: u32, state: &str, n: Option<u64>, checksum: Option<String>) {
    let mut transfers = e.transfers.lock().unwrap();
    let Some(t) = transfers.get_mut(&id) else {
        return;
    };
    t.info.state = state.into();
    if let Some(n) = n {
        t.info.transferred = n
    }
    if let Some(c) = checksum {
        t.info.checksum = c
    }
    let _ = e.events.send(EngineEvent::TransferChanged(t.info.clone()));
}
fn cancelled() -> EngineError {
    err("cancelled", "Transfer cancelled")
}
impl DeskEngine {
    pub async fn start_transfer(&self, id: u32, local_path: String) -> EngineResult<TransferInfo> {
        let s = self.session(id)?;
        let path = PathBuf::from(local_path);
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(invalid)?
            .to_owned();
        let file =
            File::open(&path).map_err(|_| err("file_access", "Source file could not be opened"))?;
        let m = file.metadata().map_err(|_| invalid())?;
        if !m.is_file() {
            return Err(invalid());
        }
        let transfer = next_id(&self.0.next)?;
        let info = TransferInfo {
            id: transfer,
            session_id: id,
            filename,
            total: m.len(),
            transferred: 0,
            state: "queued".into(),
            checksum: String::new(),
        };
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut t = self.0.transfers.lock().unwrap();
            if t.values().any(|r| {
                r.info.session_id == id
                    && matches!(r.info.state.as_str(), "queued" | "transferring")
            }) {
                return Err(err("busy", "A transfer is already active"));
            }
            if t.len() >= 128 {
                let oldest = t
                    .iter()
                    .filter(|(_, r)| !matches!(r.info.state.as_str(), "queued" | "transferring"))
                    .map(|(i, _)| *i)
                    .min()
                    .ok_or_else(|| err("busy", "Transfer limit reached"))?;
                t.remove(&oldest);
            }
            t.insert(
                transfer,
                TransferRecord {
                    info: info.clone(),
                    cancel: cancel.clone(),
                },
            );
        }
        let _ = self
            .0
            .events
            .send(EngineEvent::TransferChanged(info.clone()));
        let e = self.clone();
        let job = info.clone();
        tokio::spawn(async move {
            let result = send(&e, &s, &job, file, cancel).await;
            let state = match &result {
                Ok(()) => "completed",
                Err(e) if e.code == "cancelled" => "cancelled",
                Err(_) => "failed",
            };
            progress(&e.0, job.id, state, None, None);
            if let Err(error) = result {
                let _ = e.0.events.send(EngineEvent::Error {
                    session_id: Some(id),
                    code: error.code,
                    message: error.message,
                });
            }
        });
        Ok(info)
    }
    pub async fn cancel_transfer(&self, id: u32) -> EngineResult<()> {
        let records = self.0.transfers.lock().unwrap();
        let t = records.get(&id).ok_or_else(invalid)?;
        if !matches!(t.info.state.as_str(), "queued" | "transferring") {
            return Err(err("transfer_finished", "Transfer has already ended"));
        }
        t.cancel.store(true, Ordering::Release);
        Ok(())
    }
}
async fn rpc(ch: &mut Channel, seq: u64, body: Body) -> EngineResult<Body> {
    ch.send(msg(seq, body)).await?;
    let e = ch.receive().await?;
    if e.request_id != seq {
        return Err(invalid());
    }
    check_reply(e.body.ok_or_else(invalid)?)
}
async fn send(
    e: &DeskEngine,
    s: &ClientSession,
    t: &TransferInfo,
    mut file: File,
    cancel: Arc<AtomicBool>,
) -> EngineResult<()> {
    let epoch = s.info.lock().unwrap().epoch;
    let stop = cancel.clone();
    let (mut file, hash) = tokio::task::spawn_blocking(move || -> EngineResult<_> {
        let mut h = blake3::Hasher::new();
        let mut buf = vec![0; CHUNK_SIZE];
        loop {
            if stop.load(Ordering::Acquire) {
                return Err(cancelled());
            }
            let n = file.read(&mut buf).map_err(|_| invalid())?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        file.rewind().map_err(|_| invalid())?;
        Ok((file, *h.finalize().as_bytes()))
    })
    .await
    .map_err(|_| invalid())??;
    let mut ch = attach(s, "bulk", epoch).await?;
    let id = dh_auth::random_bytes::<16>().to_vec();
    let offer = TransferOffer {
        id: id.clone(),
        filename: t.filename.clone(),
        size: t.total,
        blake3: hash.to_vec(),
        epoch,
    };
    let mut seq = 1;
    rpc(&mut ch, seq, Body::TransferOffer(offer)).await?;
    progress(&e.0, t.id, "transferring", Some(0), Some(hex(&hash)));
    let mut offset = 0;
    let mut buf = vec![0; CHUNK_SIZE];
    while offset < t.total {
        if cancel.load(Ordering::Acquire) {
            return Err(cancelled());
        }
        let n = file.read(&mut buf).map_err(|_| invalid())?;
        if n == 0 {
            return Err(invalid());
        }
        let chunk = TransferChunk {
            id: id.clone(),
            offset,
            data: buf[..n].into(),
            blake3: blake3::hash(&buf[..n]).as_bytes().to_vec(),
        };
        seq += 1;
        let Body::TransferState(r) = rpc(&mut ch, seq, Body::TransferChunk(chunk)).await? else {
            return Err(invalid());
        };
        offset += n as u64;
        if r.id != id || r.offset != offset || r.state != "receiving" {
            return Err(invalid());
        }
        progress(&e.0, t.id, "transferring", Some(offset), None);
    }
    if cancel.load(Ordering::Acquire) {
        return Err(cancelled());
    }
    let Body::TransferState(r) = rpc(
        &mut ch,
        seq + 1,
        Body::TransferState(TransferState {
            id: id.clone(),
            state: "finish".into(),
            offset,
            ..Default::default()
        }),
    )
    .await?
    else {
        return Err(invalid());
    };
    if r.state != "completed" || r.offset != t.total || r.id != id {
        return Err(invalid());
    }
    ch.send(msg(seq + 2, cmd("close", "", epoch))).await?;
    Ok(())
}
struct Active(Arc<Host>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.data.lock().unwrap().bulk_busy = false;
    }
}
fn user(h: &Host, b: &Binding, epoch: u64, layout: u64) -> EngineResult<UserContext> {
    let d = h.refresh();
    d.check(b, crate::Operation::Files, epoch, layout)?;
    let crate::HostState::Ready { user } = d.broker.state() else {
        return Err(err("permission_denied", "An active user is required"));
    };
    Ok(UserContext {
        user: user.clone(),
        interactive: true,
        epoch,
    })
}
pub(super) async fn receive(
    h: Arc<Host>,
    mut ch: Channel,
    b: Binding,
    epoch: u64,
    layout: u64,
) -> EngineResult<()> {
    user(&h, &b, epoch, layout)?;
    {
        let mut d = h.data.lock().unwrap();
        if d.bulk_busy {
            return Err(err("busy", "File receiver is busy"));
        }
        d.bulk_busy = true;
    }
    let _active = Active(h.clone());
    let env = ch.receive().await?;
    let Body::TransferOffer(o) = env.body.ok_or_else(invalid)? else {
        return Err(invalid());
    };
    if o.epoch != epoch || o.id.len() != 16 {
        return Err(invalid());
    }
    let ctx = user(&h, &b, epoch, layout)?;
    let id = o.id;
    let mut rx = h
        .directory
        .as_ref()
        .ok_or_else(invalid)?
        .receive(
            Offer {
                filename: o.filename,
                size: o.size,
                checksum: o.blake3.try_into().map_err(|_| invalid())?,
            },
            ctx,
        )
        .map_err(|_| invalid())?;
    ch.send(msg(env.request_id, cmd("accepted", "", epoch)))
        .await?;
    let mut offset = 0;
    loop {
        let env = ch.receive().await?;
        let ctx = user(&h, &b, epoch, layout)?;
        let body = match env.body {
            Some(Body::TransferChunk(c)) if c.id == id => {
                offset = rx
                    .write_chunk(
                        &ctx,
                        c.offset,
                        &c.data,
                        c.blake3.try_into().map_err(|_| invalid())?,
                    )
                    .map_err(|_| invalid())?;
                Body::TransferState(TransferState {
                    id: id.clone(),
                    state: "receiving".into(),
                    offset,
                    ..Default::default()
                })
            }
            Some(Body::TransferState(s))
                if s.id == id && s.offset == offset && s.state == "finish" =>
            {
                let result = rx.finish_authorized(&ctx, false, || {
                    let guard = h.refresh();
                    guard
                        .check(&b, crate::Operation::Files, epoch, layout)
                        .map_err(|_| dh_transfer::TransferError::Context)?;
                    Ok(guard)
                });
                let reply = match result {
                    Ok(()) => Body::TransferState(TransferState {
                        id,
                        state: "completed".into(),
                        offset,
                        ..Default::default()
                    }),
                    Err(e) => Body::Failure(dh_protocol::Failure {
                        code: if matches!(e, dh_transfer::TransferError::Collision) {
                            "collision"
                        } else {
                            "invalid_request"
                        }
                        .into(),
                        message: String::new(),
                    }),
                };
                ch.send(msg(env.request_id, reply)).await?;
                let _ = ch.receive().await;
                return Ok(());
            }
            _ => return Err(invalid()),
        };
        ch.send(msg(env.request_id, body)).await?;
    }
}
