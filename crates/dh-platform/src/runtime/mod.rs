//! Ordinary user-session runtime. No installed-helper activation or desktop switching.
#[cfg_attr(not(windows), allow(dead_code))]
mod input;
mod private;
#[cfg(windows)]
mod windows_input;
pub use input::NativeInput;
#[cfg(windows)]
mod user;
use crate::report::Display;
pub use private::private_directory;

pub struct Availability {
    pub state: &'static str,
    pub user: Option<String>,
    pub displays: Vec<Display>,
    pub detail: &'static str,
}
pub fn inspect() -> Availability {
    #[cfg(windows)]
    {
        use crate::report::{CheckStatus, SessionKind};
        if let Ok(observation) = crate::windows::inspect() {
            if observation.session_kind == SessionKind::LockedOrSignIn {
                return unavailable("sign_in", "The installed sign-in worker is unqualified");
            }
            if observation.session_kind == SessionKind::UserDesktop
                && !observation.elevated
                && observation.checks.iter().any(|c| {
                    c.name == "windows.token_elevation_query" && c.status == CheckStatus::Passed
                })
            {
                if observation.displays.is_empty() {
                    return unavailable("no_display", "No active monitor");
                }
                let Some(user) = observation.console_session.and_then(user::console_user) else {
                    return unavailable(
                        "permission_needed",
                        "Process principal is not the active console user",
                    );
                };
                return Availability {
                    state: "ready",
                    user: Some(user),
                    displays: observation.displays,
                    detail: "Ordinary user desktop capture and input. Installed sign-in/secure-desktop worker remains unqualified.",
                };
            }
        }
    }
    let detail = match std::env::consts::OS {
        "android" => "Android requires a foreground MediaProjection grant from the current session",
        "macos" => "Signed Aqua capture agent and screen recording permission are required",
        "linux" => "GNOME capture portal or a qualified KMS worker is required",
        _ => "An ordinary active user desktop is required",
    };
    unavailable("permission_needed", detail)
}
fn unavailable(state: &'static str, detail: &'static str) -> Availability {
    Availability {
        state,
        detail,
        user: None,
        displays: vec![],
    }
}
pub fn capture(id: &str) -> Result<crate::frame::Frame, crate::report::ProbeError> {
    #[cfg(windows)]
    {
        crate::windows::capture(id)
    }
    #[cfg(not(windows))]
    {
        let _ = id;
        Err(crate::report::ProbeError::new(
            "permission_needed",
            "Native capture capability is unavailable",
        ))
    }
}

mod clipboard;
pub use clipboard::{clipboard_read, clipboard_write};

pub fn inject_input(
    expected_user: &str,
    display: &Display,
    event: NativeInput,
) -> Result<(), crate::report::ProbeError> {
    #[cfg(windows)]
    {
        windows_input::inject(expected_user, display, event)
    }
    #[cfg(not(windows))]
    {
        let _ = (expected_user, display, event);
        Err(crate::report::ProbeError::new(
            "input_unavailable",
            "Native input is unavailable on this platform",
        ))
    }
}
pub fn release_input(expected_user: &str) -> Result<(), crate::report::ProbeError> {
    #[cfg(windows)]
    {
        windows_input::release(expected_user)
    }
    #[cfg(not(windows))]
    {
        let _ = expected_user;
        Err(crate::report::ProbeError::new(
            "input_unavailable",
            "Native input is unavailable on this platform",
        ))
    }
}
