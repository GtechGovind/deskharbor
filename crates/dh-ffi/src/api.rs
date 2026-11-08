use crate::engine;
use dh_core::engine as core;
use zeroize::Zeroizing;

// Deliberately bridge only small control metadata. RGBA stays in native texture leases.
#[derive(Clone, Debug)]
pub struct DisplayInfo {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
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
#[derive(Clone, Debug)]
pub struct HostInfo {
    pub state: String,
    pub running: bool,
    pub password_configured: bool,
    pub unattended_available: bool,
    pub addresses: Vec<String>,
    pub detail: String,
}
#[derive(Clone, Debug)]
pub struct HostSettings {
    pub enabled: bool,
    pub bind_address: String,
    pub allow_input: bool,
    pub clipboard: bool,
    pub files: bool,
    pub transfer_directory: Option<String>,
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
#[derive(Clone, Debug)]
pub struct InputRequest {
    pub epoch: u64,
    pub layout: u64,
    pub display_id: String,
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub buttons: u32,
    pub wheel_x: i32,
    pub wheel_y: i32,
    pub usage: u16,
    pub pressed: bool,
    pub text: String,
}

impl From<core::DisplayInfo> for DisplayInfo {
    fn from(v: core::DisplayInfo) -> Self {
        Self {
            id: v.id,
            name: v.name,
            width: v.width,
            height: v.height,
            x: v.x,
            y: v.y,
        }
    }
}
impl From<core::SessionInfo> for SessionInfo {
    fn from(v: core::SessionInfo) -> Self {
        Self {
            id: v.id,
            endpoint: v.endpoint,
            state: v.state,
            transport: v.transport,
            host_identity: v.host_identity,
            epoch: v.epoch,
            layout: v.layout,
            displays: v.displays.into_iter().map(Into::into).collect(),
            selected_display: v.selected_display,
        }
    }
}
impl From<core::HostInfo> for HostInfo {
    fn from(v: core::HostInfo) -> Self {
        Self {
            state: v.state,
            running: v.running,
            password_configured: v.password_configured,
            unattended_available: v.unattended_available,
            addresses: v.addresses,
            detail: v.detail,
        }
    }
}
impl From<core::TransferInfo> for TransferInfo {
    fn from(v: core::TransferInfo) -> Self {
        Self {
            id: v.id,
            session_id: v.session_id,
            filename: v.filename,
            total: v.total,
            transferred: v.transferred,
            state: v.state,
            checksum: v.checksum,
        }
    }
}
impl From<core::SavedDevice> for SavedDevice {
    fn from(v: core::SavedDevice) -> Self {
        Self {
            name: v.name,
            address: v.address,
            identity: v.identity,
        }
    }
}

pub fn initialize(data_dir: String) -> Result<(), String> {
    if data_dir.trim().is_empty() {
        return Err("Application support directory is required".into());
    }
    if crate::ENGINE.get().is_some() {
        return Err("Engine is already initialized".into());
    }
    let value = core::DeskEngine::new(data_dir.into()).map_err(|e| e.to_string())?;
    crate::ENGINE
        .set(value)
        .map_err(|_| "Engine is already initialized".to_owned())
}
pub async fn connect(endpoint: String, password: String) -> Result<SessionInfo, String> {
    engine()?
        .connect(endpoint, Zeroizing::new(password.into_bytes()))
        .await
        .map(Into::into)
        .map_err(|e| e.to_string())
}
pub async fn disconnect(session_id: u32) -> Result<(), String> {
    engine()?
        .disconnect(session_id)
        .await
        .map_err(|e| e.to_string())
}
pub async fn send_input(session_id: u32, event: InputRequest) -> Result<(), String> {
    let kind = match event.kind.as_str() {
        "pointer" => core::InputKind::Pointer {
            x: event.x,
            y: event.y,
            buttons: event.buttons,
            wheel_x: event.wheel_x,
            wheel_y: event.wheel_y,
        },
        "key" => core::InputKind::Key {
            usage: event.usage,
            pressed: event.pressed,
        },
        "text" => core::InputKind::Text(event.text),
        "release_all" => core::InputKind::ReleaseAll,
        _ => return Err("Unknown input event kind".into()),
    };
    engine()?
        .send_input(
            session_id,
            core::InputRequest {
                epoch: event.epoch,
                layout: event.layout,
                display_id: event.display_id,
                kind,
            },
        )
        .await
        .map_err(|e| e.to_string())
}
pub async fn select_monitor(session_id: u32, display_id: String) -> Result<(), String> {
    engine()?
        .select_monitor(session_id, display_id)
        .await
        .map_err(|e| e.to_string())
}
pub async fn clipboard_send_text(session_id: u32, text: String) -> Result<(), String> {
    engine()?
        .clipboard_send_text(session_id, text)
        .await
        .map_err(|e| e.to_string())
}
pub async fn clipboard_request_text(session_id: u32) -> Result<String, String> {
    engine()?
        .clipboard_request_text(session_id)
        .await
        .map_err(|e| e.to_string())
}
pub async fn start_transfer(session_id: u32, local_path: String) -> Result<TransferInfo, String> {
    engine()?
        .start_transfer(session_id, local_path)
        .await
        .map(Into::into)
        .map_err(|e| e.to_string())
}
pub async fn cancel_transfer(transfer_id: u32) -> Result<(), String> {
    engine()?
        .cancel_transfer(transfer_id)
        .await
        .map_err(|e| e.to_string())
}
pub async fn host_status() -> Result<HostInfo, String> {
    engine()?
        .host_status()
        .await
        .map(Into::into)
        .map_err(|e| e.to_string())
}
pub async fn configure_host(settings: HostSettings) -> Result<HostInfo, String> {
    engine()?
        .configure_host(core::HostSettings {
            enabled: settings.enabled,
            bind_address: settings.bind_address,
            allow_input: settings.allow_input,
            clipboard: settings.clipboard,
            files: settings.files,
            transfer_directory: settings.transfer_directory,
        })
        .await
        .map(Into::into)
        .map_err(|e| e.to_string())
}
pub async fn set_host_password(password: String) -> Result<(), String> {
    engine()?
        .set_host_password(Zeroizing::new(password.into_bytes()))
        .await
        .map_err(|e| e.to_string())
}
pub fn devices() -> Result<Vec<SavedDevice>, String> {
    engine()?
        .devices()
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}
pub fn save_device(device: SavedDevice) -> Result<(), String> {
    engine()?
        .save_device(core::SavedDevice {
            name: device.name,
            address: device.address,
            identity: device.identity,
        })
        .map_err(|e| e.to_string())
}
pub fn forget_device(address: String) -> Result<(), String> {
    engine()?.forget_device(address).map_err(|e| e.to_string())
}
pub fn transfers() -> Result<Vec<TransferInfo>, String> {
    Ok(engine()?.transfers().into_iter().map(Into::into).collect())
}
