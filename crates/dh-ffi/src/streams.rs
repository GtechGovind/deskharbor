use crate::{
    api::{HostInfo, SessionInfo, TransferInfo},
    engine,
    frb_generated::StreamSink,
};
use dh_core::engine::EngineEvent;

#[derive(Clone, Debug)]
pub struct EngineEventInfo {
    pub kind: String,
    pub session: Option<SessionInfo>,
    pub host: Option<HostInfo>,
    pub transfer: Option<TransferInfo>,
    pub session_id: Option<u32>,
    pub sequence: u64,
    pub width: u32,
    pub height: u32,
    pub code: String,
    pub message: String,
}

pub async fn subscribe(sink: StreamSink<EngineEventInfo>) -> Result<(), String> {
    let mut receiver = engine()?.subscribe();
    loop {
        let event = match receiver.recv().await {
            Ok(event) => event,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                // Dropped control metadata means the UI must invalidate input rather than assume current epoch.
                if sink
                    .add(EngineEventInfo {
                        kind: "error".into(),
                        session: None,
                        host: None,
                        transfer: None,
                        session_id: None,
                        sequence: 0,
                        width: 0,
                        height: 0,
                        code: "event_gap".into(),
                        message: "Engine updates were missed; reconnect to refresh session state"
                            .into(),
                    })
                    .is_err()
                {
                    break;
                }
                continue;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        };
        let mut out = EngineEventInfo {
            kind: String::new(),
            session: None,
            host: None,
            transfer: None,
            session_id: None,
            sequence: 0,
            width: 0,
            height: 0,
            code: String::new(),
            message: String::new(),
        };
        match event {
            EngineEvent::SessionChanged(value) => {
                out.kind = "session".into();
                out.session = Some(value.into());
            }
            EngineEvent::HostChanged(value) => {
                out.kind = "host".into();
                out.host = Some(value.into());
            }
            EngineEvent::TransferChanged(value) => {
                out.kind = "transfer".into();
                out.transfer = Some(value.into());
            }
            EngineEvent::FrameReady {
                session_id,
                sequence,
                width,
                height,
            } => {
                out.kind = "frame".into();
                out.session_id = Some(session_id);
                out.sequence = sequence;
                out.width = width;
                out.height = height;
            }
            EngineEvent::Error {
                session_id,
                code,
                message,
            } => {
                out.kind = "error".into();
                out.session_id = session_id;
                out.code = code;
                out.message = message;
            }
        }
        if sink.add(out).is_err() {
            break;
        }
    }
    Ok(())
}
