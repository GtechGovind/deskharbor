mod packet;

use crate::{
    frame::Frame,
    report::{Check, CheckStatus, Display, Inspection, Platform, ProbeError, SessionKind},
};
use std::{
    ffi::CString,
    fs,
    os::unix::ffi::OsStrExt,
    path::Path,
    process::{Command, Stdio},
};

const MAX_PAYLOAD: u64 = 128 * 1024 * 1024;

fn identity_allowed(ids: [u32; 6], secure: u64, capabilities: [u32; 6]) -> bool {
    ids.iter().all(|id| *id != 0)
        && ids[0] == ids[1]
        && ids[1] == ids[2]
        && ids[3] == ids[4]
        && ids[4] == ids[5]
        && secure == 0
        && capabilities.iter().all(|word| *word == 0)
}

fn require_unprivileged() -> Result<(), ProbeError> {
    #[repr(C)]
    struct Header {
        version: u32,
        pid: i32,
    }
    #[derive(Default, Clone, Copy)]
    #[repr(C)]
    struct Caps {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    let mut ids = [0u32; 6];
    let mut header = Header {
        version: 0x2008_0522,
        pid: 0,
    };
    let mut caps = [Caps::default(); 2];
    // SAFETY: All six pointers reference distinct writable uid_t/gid_t storage; the
    // capget ABI uses the v3 header and two data words for the calling process.
    let (uid_result, gid_result, cap_result, secure) = unsafe {
        (
            libc::getresuid(&mut ids[0], &mut ids[1], &mut ids[2]),
            libc::getresgid(&mut ids[3], &mut ids[4], &mut ids[5]),
            libc::syscall(
                libc::SYS_capget,
                &mut header as *mut Header,
                caps.as_mut_ptr(),
            ),
            libc::getauxval(libc::AT_SECURE),
        )
    };
    let words = [
        caps[0].effective,
        caps[0].permitted,
        caps[0].inheritable,
        caps[1].effective,
        caps[1].permitted,
        caps[1].inheritable,
    ];
    if uid_result != 0
        || gid_result != 0
        || cap_result != 0
        || !identity_allowed(ids, secure, words)
    {
        return Err(ProbeError::new(
            "unprivileged_cli_required",
            "The diagnostic CLI refuses root, set-ID, secure-execution and capability-bearing identities before launching any child. No trusted installed broker exists yet.",
        ));
    }
    Ok(())
}

fn number(s: &str) -> Option<u32> {
    if s.is_empty() || s.starts_with('0') && s.len() > 1 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn ids(id: &str) -> Result<(u32, u32), ProbeError> {
    let pair = id
        .strip_prefix("card")
        .and_then(|s| s.split_once(':'))
        .and_then(|(card, connector)| Some((number(card)?, number(connector)?)))
        .filter(|&(card, connector)| card < 64 && connector != 0);
    pair.ok_or_else(|| {
        ProbeError::new(
            "invalid_display",
            "expected cardN:connector-object-id from inspect",
        )
    })
}

fn malformed() -> ProbeError {
    ProbeError::new(
        "worker_protocol",
        "invalid or incomplete DRM worker response",
    )
}

fn header(packet: &[u8]) -> Result<(u32, u32), ProbeError> {
    let h = packet.get(..12).ok_or_else(malformed)?;
    if &h[..4] != b"DHF1" {
        return Err(malformed());
    }
    let w = u32::from_le_bytes(h[4..8].try_into().unwrap());
    let h = u32::from_le_bytes(h[8..12].try_into().unwrap());
    if w == 0 || h == 0 {
        return Err(malformed());
    }
    Ok((w, h))
}

fn decode(id: String, packet: &[u8]) -> Result<Frame, ProbeError> {
    if packet.starts_with(b"DHP2") {
        return self::packet::decode(id, packet);
    }
    let (w, h) = header(packet)?;
    let size = u64::from(w) * u64::from(h);
    if size > MAX_PAYLOAD / 4 || packet.len() as u64 != 12 + size * 4 {
        return Err(malformed());
    }
    Frame::new_rgba(id, w, h, packet[12..].to_vec())
}

fn worker_error(status: Option<i32>) -> ProbeError {
    let (code, detail) = match status {
        Some(2) => (
            "invalid_display",
            "invalid DRM worker request or card device identity",
        ),
        Some(3) => (
            "permission_denied",
            "DRM card, GETFB2 handles, or PRIME export denied",
        ),
        Some(4) => (
            "no_scanout",
            "no connected active scanout for this connector",
        ),
        Some(5) => (
            "unsupported_composition",
            "ambiguous plane order, unknown/effective color or protection property, or incomplete/nonopaque primary",
        ),
        Some(6) => (
            "unsupported_geometry",
            "missing plane geometry, scaling, crop, rotation, or partial scanout",
        ),
        Some(7) => (
            "unsupported_modifier",
            "capture requires an explicitly linear framebuffer",
        ),
        Some(8) => (
            "unsupported_format",
            "capture requires supported packed linear XRGB8888/ARGB8888 sources",
        ),
        Some(9) => (
            "invalid_buffer",
            "invalid stride, offset, backing size, or capture dimensions",
        ),
        Some(11) => (
            "sandbox_unavailable",
            "worker identity, output pipe, or syscall sandbox unavailable",
        ),
        Some(12) => ("scanout_changed", "display state changed during capture"),
        _ => (
            "drm_unavailable",
            "kernel operation unavailable or worker terminated",
        ),
    };
    ProbeError::new(code, detail)
}

fn run_worker(action: &str, card: u32, connector: u32) -> Result<Vec<u8>, ProbeError> {
    require_unprivileged()?;
    let executable = std::env::current_exe()
        .map_err(|e| ProbeError::new("worker_unavailable", e.to_string()))?
        .with_file_name("dh-drm-worker");
    let mut child = Command::new(executable)
        .args([action, &card.to_string(), &connector.to_string()])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            ProbeError::new(
                "worker_unavailable",
                format!("build dh-drm-worker beside the probe: {e}"),
            )
        })?;
    let mut stdout = child.stdout.take().expect("piped child stdout");
    let read = packet::read_packet(&mut stdout, action == "mode");
    drop(stdout);
    if read.is_err() {
        let _ = child.kill();
    }
    let status = child
        .wait()
        .map_err(|e| ProbeError::new("drm_unavailable", e.to_string()))?;
    if !status.success() {
        if status.code().is_none() {
            return Err(read.err().unwrap_or_else(|| worker_error(None)));
        }
        return Err(worker_error(status.code()));
    }
    read
}

fn read_text(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
}

fn loginctl(args: &[&str]) -> Option<String> {
    let output = Command::new("/usr/bin/loginctl")
        .args(["--no-pager", "--no-ask-password"])
        .args(args)
        .env("SYSTEMD_BUS_TIMEOUT", "3s")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn property<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
}

fn console_kind(details: &str) -> SessionKind {
    if property(details, "Remote") != Some("no")
        || property(details, "Active") != Some("yes")
        || !matches!(property(details, "Type"), Some("wayland" | "x11"))
    {
        return SessionKind::Unknown;
    }
    if property(details, "Class") == Some("greeter")
        || property(details, "LockedHint") == Some("yes")
    {
        SessionKind::LockedOrSignIn
    } else if property(details, "Class") == Some("user")
        && property(details, "LockedHint") == Some("no")
    {
        SessionKind::UserDesktop
    } else {
        SessionKind::Unknown
    }
}

fn card_access(name: &str) -> Check {
    let path = Path::new("/dev/dri").join(name);
    let c_path = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: c_path is a valid nul-terminated filename for the duration of access.
    let ok = unsafe { libc::access(c_path.as_ptr(), libc::R_OK) } == 0;
    let detail = if ok {
        format!("{} is readable by this user", path.display())
    } else {
        format!("{}: {}", path.display(), std::io::Error::last_os_error())
    };
    Check {
        name: format!("device_access_{name}"),
        status: if ok {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        detail,
    }
}

pub fn inspect() -> Result<Inspection, ProbeError> {
    require_unprivileged()?;
    let release = read_text("/etc/os-release").unwrap_or_default();
    let kernel = read_text("/proc/sys/kernel/osrelease").unwrap_or_default();
    let os = property(&release, "PRETTY_NAME")
        .unwrap_or("Linux")
        .trim_matches('"');
    let mut checks = Vec::new();
    let mut warnings = vec!["Capture is an unprivileged, best-effort diagnostic. Matching endpoint metadata and DMA-BUF CPU synchronization do not prevent concurrent writers or A-to-B-to-A commits. No coherent or privileged production capture is qualified.".into()];
    let mut displays = Vec::new();
    match fs::read_dir("/sys/class/drm") {
        Err(e) => warnings.push(format!("DRM sysfs unavailable: {e}")),
        Ok(dir) => {
            let mut entries = dir
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| ProbeError::new("drm_inventory", e.to_string()))?;
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let name = entry.file_name().to_string_lossy().into_owned();
                let Some(short) = name.strip_prefix("card") else {
                    continue;
                };
                if number(short).is_some_and(|n| n < 64) {
                    checks.push(card_access(&name));
                    continue;
                }
                let Some((n, _)) = short.split_once('-') else {
                    continue;
                };
                let Some(card) = number(n).filter(|n| *n < 64) else {
                    continue;
                };
                if read_text(entry.path().join("status")).as_deref() != Some("connected") {
                    continue;
                }
                let Some(connector) = read_text(entry.path().join("connector_id"))
                    .and_then(|s| number(&s))
                    .filter(|n| *n != 0)
                else {
                    warnings.push(format!("{name}: kernel connector ID unavailable"));
                    continue;
                };
                let id = format!("card{card}:{connector}");
                let dimensions = run_worker("mode", card, connector).and_then(|data| header(&data));
                let (width, height) = dimensions.unwrap_or_else(|e| {
                    warnings.push(format!("{id}: {e}"));
                    (0, 0)
                });
                displays.push(Display {
                    id,
                    name,
                    x: 0,
                    y: 0,
                    width,
                    height,
                    primary: false,
                });
            }
        }
    }
    if displays.is_empty() {
        warnings.push("No connected DRM connector found.".into());
    } else {
        warnings.push("KMS has no compositor-global logical origin or primary-monitor preference. Origins are placeholders; zero dimensions indicate an unreadable or inactive mode.".into());
    }

    let seat = loginctl(&["show-seat", "seat0", "--property=ActiveSession", "--value"]);
    let process_id = loginctl(&["show-session", "self", "--property=Id", "--value"]);
    let mut kind = SessionKind::Unknown;
    let mut desktop = None;
    match &seat {
        Some(s) if s.is_empty() => kind = SessionKind::NoConsole,
        Some(id) => {
            if let Some(details) = loginctl(&[
                "show-session",
                id,
                "--property=Active",
                "--property=Class",
                "--property=Type",
                "--property=Remote",
                "--property=LockedHint",
                "--property=Desktop",
            ]) {
                kind = console_kind(&details);
                desktop = Some(format!(
                    "seat0 session {id} ({}, {}, {})",
                    property(&details, "Class").unwrap_or("?"),
                    property(&details, "Type").unwrap_or("?"),
                    property(&details, "Desktop").unwrap_or("?")
                ));
            }
        }
        None => warnings.push("systemd-logind seat0 unavailable; console identity unknown.".into()),
    }
    checks.push(Check{name:"physical_gdm_scanout_and_input".into(),status:CheckStatus::NotRun,
        detail:"Inspection does not exercise physical Intel/AMD GDM, user transitions, visible composition, or input.".into()});
    Ok(Inspection {
        schema_version: 1,
        platform: Platform::Linux,
        environment: format!("{os}; kernel {kernel}"),
        // SAFETY: geteuid takes no arguments and returns the process effective uid.
        elevated: unsafe { libc::geteuid() } == 0,
        process_session: process_id.and_then(|s| number(&s)),
        console_session: seat.and_then(|s| number(&s)),
        desktop,
        session_kind: kind,
        displays,
        checks,
        warnings,
    })
}

pub fn capture(display_id: &str) -> Result<Frame, ProbeError> {
    require_unprivileged()?;
    let id = if display_id == "primary" {
        inspect()?.displays.into_iter().find(|d| d.width > 0 && d.height > 0).map(|d| d.id)
            .ok_or_else(|| ProbeError::new("no_readable_scanout","no connected display with a readable active mode; inspect device_access checks"))?
    } else {
        display_id.to_owned()
    };
    let (card, connector) = ids(&id)?;
    decode(id, &run_worker("capture", card, connector)?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn privileged_or_mismatched_cli_identity_never_dispatches_a_worker() {
        use super::identity_allowed;
        assert!(identity_allowed([1000; 6], 0, [0; 6]));
        assert!(!identity_allowed([0; 6], 0, [0; 6]));
        assert!(!identity_allowed(
            [1000, 0, 1000, 1000, 1000, 1000],
            0,
            [0; 6]
        ));
        assert!(!identity_allowed(
            [1000, 1000, 0, 1000, 1000, 1000],
            0,
            [0; 6]
        ));
        assert!(!identity_allowed([1000; 6], 1, [0; 6]));
        for index in 0..6 {
            let mut capabilities = [0; 6];
            capabilities[index] = 1;
            assert!(!identity_allowed([1000; 6], 0, capabilities));
        }
    }
    use super::*;
    #[test]
    fn display_identifier_accepts_only_primary_drm_minors_and_numeric_connector() {
        assert_eq!(ids("card5:42").unwrap(), (5, 42));
        for id in [
            "card64:42",
            "card0:0",
            "card-1:20",
            "card05:42",
            "/dev/dri/card0:1",
            "card1:../passwd",
        ] {
            assert_eq!(ids(id).unwrap_err().code, "invalid_display");
        }
    }
    #[test]
    fn worker_protocol_rejects_truncation_and_trailing_pixels() {
        let packet = [68, 72, 70, 49, 1, 0, 0, 0, 1, 0, 0, 0, 90, 80, 70, 255];
        assert_eq!(
            decode("card1:4".into(), &packet).unwrap().pixels_rgba,
            [90, 80, 70, 255]
        );
        assert!(decode("card1:4".into(), &packet[..15]).is_err());
        assert!(decode("card1:4".into(), &[&packet[..], &[0]].concat()).is_err());
        assert!(decode("card1:4".into(), &[0; 16]).is_err());
    }
}
