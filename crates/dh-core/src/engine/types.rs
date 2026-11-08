use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct EngineError {
    pub code: String,
    pub message: String,
}
impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for EngineError {}
pub type EngineResult<T> = Result<T, EngineError>;
pub(super) fn err(code: &str, msg: &str) -> EngineError {
    EngineError {
        code: code.into(),
        message: msg.into(),
    }
}
pub(super) fn invalid() -> EngineError {
    err(
        "invalid_request",
        "Invalid request or unexpected peer response",
    )
}
pub(super) fn storage() -> EngineError {
    err("storage", "Could not access the private application store")
}
impl From<dh_net::NetError> for EngineError {
    fn from(value: dh_net::NetError) -> Self {
        use dh_net::NetError::*;
        match value {
            Authentication => err(
                "authentication_failed",
                "Host password authentication failed",
            ),
            IdentityChanged => err("identity_changed", "The host identity changed"),
            Timeout => err("timeout", "Connection timed out"),
            _ => err("disconnected", "Secure connection failed or closed"),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayInfo {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
}
impl From<dh_protocol::Display> for DisplayInfo {
    fn from(d: dh_protocol::Display) -> Self {
        Self {
            id: d.id,
            name: d.name,
            width: d.width,
            height: d.height,
            x: d.x,
            y: d.y,
        }
    }
}
#[derive(Clone, Debug)]
pub struct SessionInfo {
    pub id: u32,
    pub endpoint: String,
    pub state: String,
    pub transport: String,
    pub host_identity: String,
    pub epoch: u64,
    pub layout: u64,
    pub displays: Vec<DisplayInfo>,
    pub selected_display: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostInfo {
    pub state: String,
    pub running: bool,
    pub password_configured: bool,
    pub unattended_available: bool,
    pub addresses: Vec<String>,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostSettings {
    pub enabled: bool,
    pub bind_address: String,
    pub allow_input: bool,
    pub clipboard: bool,
    pub files: bool,
    pub transfer_directory: Option<String>,
}
impl Default for HostSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bind_address: "127.0.0.1:47832".into(),
            allow_input: false,
            clipboard: false,
            files: false,
            transfer_directory: None,
        }
    }
}
#[derive(Clone, Debug)]
pub struct SavedDevice {
    pub name: String,
    pub address: String,
    pub identity: Option<String>,
}
#[derive(Clone, Debug)]
pub struct TransferInfo {
    pub id: u32,
    pub session_id: u32,
    pub filename: String,
    pub total: u64,
    pub transferred: u64,
    pub state: String,
    pub checksum: String,
}
#[derive(Clone)]
pub struct InputRequest {
    pub epoch: u64,
    pub layout: u64,
    pub display_id: String,
    pub kind: InputKind,
}
#[derive(Clone)]
pub enum InputKind {
    Pointer {
        x: f64,
        y: f64,
        buttons: u32,
        wheel_x: i32,
        wheel_y: i32,
    },
    Key {
        usage: u16,
        pressed: bool,
    },
    Text(String),
    ReleaseAll,
}
#[derive(Clone, Debug)]
pub enum EngineEvent {
    SessionChanged(SessionInfo),
    HostChanged(HostInfo),
    TransferChanged(TransferInfo),
    FrameReady {
        session_id: u32,
        sequence: u64,
        width: u32,
        height: u32,
    },
    Error {
        session_id: Option<u32>,
        code: String,
        message: String,
    },
}
pub(super) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(s, "{byte:02x}").expect("string write");
    }
    s
}
pub(super) fn unhex<const N: usize>(s: &str) -> EngineResult<[u8; N]> {
    if s.len() != N * 2 || !s.is_ascii() {
        return Err(invalid());
    }
    let mut out = [0; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(|_| invalid())?;
    }
    Ok(out)
}
