use dh_platform::{gate, report::ProbeError};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    process::ExitCode,
};

const USAGE: &str = "DeskHarbor P0 native probe (no network listener)\n\nusage:\n  dh-platform-probe inspect\n  dh-platform-probe capture --display ID --output FILE.bmp\n  dh-platform-probe gate-template\n  dh-platform-probe gate --evidence FILE.json --artifact-root DIRECTORY\n\nNo arguments performs read-only inspection. Capture is explicit and refuses overwrite.\nWindows isolated-lab entrypoints: windows-service; windows-worker SESSION Default|Winlogon.\nNeither entrypoint installs a service or changes security policy. See docs/windows-proof.md.\nmacOS: use native/macos Probe.swift harness.\n";

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn json(value: &impl serde::Serialize) -> Result<(), ProbeError> {
    let text =
        serde_json::to_string_pretty(value).map_err(|e| ProbeError::new("json", e.to_string()))?;
    println!("{text}");
    Ok(())
}

fn usage() -> ProbeError {
    ProbeError::new("usage", USAGE)
}

fn run(args: &[String]) -> Result<u8, ProbeError> {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["inspect"] => {
            json(&inspect()?)?;
            Ok(0)
        }
        ["--help"] | ["help"] | ["-h"] => {
            print!("{USAGE}");
            Ok(0)
        }
        ["gate-template"] => {
            json(&gate::template())?;
            Ok(0)
        }
        ["gate", "--evidence", file, "--artifact-root", root] => {
            // Limit untrusted JSON before parsing, including files that grow during a read.
            use std::io::Read;
            let mut bytes = Vec::new();
            fs::File::open(file)
                .map_err(io_error)?
                .take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(io_error)?;
            if bytes.len() > 4 * 1024 * 1024 {
                return Err(ProbeError::new(
                    "evidence_too_large",
                    "Evidence JSON exceeds 4 MiB",
                ));
            }
            let evidence = serde_json::from_slice(&bytes)
                .map_err(|e| ProbeError::new("invalid_evidence", e.to_string()))?;
            let report = gate::evaluate(&evidence, Path::new(root))?;
            json(&report)?;
            Ok(if report.ready { 0 } else { 2 })
        }
        ["capture", "--display", display, "--output", output] => {
            if Path::new(output)
                .extension()
                .is_none_or(|v| !v.eq_ignore_ascii_case("bmp"))
            {
                return Err(ProbeError::new(
                    "output_format",
                    "Capture output must use .bmp",
                ));
            }
            let frame = capture(display)?;
            let bitmap = frame.encode_bmp()?;
            let mut file = create_capture(Path::new(output)).map_err(io_error)?;
            file.write_all(&bitmap).map_err(io_error)?;
            file.sync_all().map_err(io_error)?;
            json(
                &serde_json::json!({"output":output,"width":frame.width,"height":frame.height,"display_id":frame.display_id,"scope":"current-process native capture only; not an unattended/sign-in proof","bytes":bitmap.len()}),
            )?;
            Ok(0)
        }
        #[cfg(windows)]
        ["windows-service"] => {
            dh_platform::windows::service::service_entry()?;
            Ok(0)
        }
        #[cfg(windows)]
        ["windows-worker", session, desktop] if matches!(*desktop, "Default" | "Winlogon") => {
            dh_platform::windows::service::worker_entry(
                session.parse().map_err(|_| usage())?,
                desktop,
            )?;
            Ok(0)
        }
        #[cfg(windows)]
        ["windows-input-worker", session, desktop]
            if matches!(*desktop, "Default" | "Winlogon") =>
        {
            dh_platform::windows::service::input_worker_entry(
                session.parse().map_err(|_| usage())?,
                desktop,
            )
        }
        _ => Err(usage()),
    }
}

fn io_error(error: std::io::Error) -> ProbeError {
    ProbeError::new("io", error.to_string())
}

fn create_capture(path: &Path) -> std::io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn capture_is_owner_only_even_under_permissive_umask() {
        struct Restore(u32);
        impl Drop for Restore {
            fn drop(&mut self) {
                // SAFETY: Restores this unit-test process's previous umask.
                unsafe {
                    libc::umask(self.0);
                }
            }
        }
        // SAFETY: This is the sole unit test in this process; no capture threads run here.
        let _restore = Restore(unsafe { libc::umask(0) });
        let path = std::env::temp_dir().join(format!("deskharbor-mode-{}", std::process::id()));
        let file = create_capture(&path).unwrap();
        let mode = file.metadata().unwrap().permissions().mode() & 0o777;
        drop(file);
        fs::remove_file(path).unwrap();
        assert_eq!(mode, 0o600);
    }
}

#[cfg(windows)]
fn inspect() -> Result<dh_platform::report::Inspection, ProbeError> {
    dh_platform::windows::inspect()
}
#[cfg(target_os = "linux")]
fn inspect() -> Result<dh_platform::report::Inspection, ProbeError> {
    dh_platform::linux::inspect()
}
#[cfg(not(any(windows, target_os = "linux")))]
fn inspect() -> Result<dh_platform::report::Inspection, ProbeError> {
    Err(ProbeError::new(
        "platform_harness_required",
        "Use the dedicated Swift macOS or Kotlin Android harness",
    ))
}

#[cfg(windows)]
fn capture(id: &str) -> Result<dh_platform::frame::Frame, ProbeError> {
    dh_platform::windows::capture(id)
}
#[cfg(target_os = "linux")]
fn capture(id: &str) -> Result<dh_platform::frame::Frame, ProbeError> {
    dh_platform::linux::capture(id)
}
#[cfg(not(any(windows, target_os = "linux")))]
fn capture(_: &str) -> Result<dh_platform::frame::Frame, ProbeError> {
    Err(ProbeError::new(
        "platform_harness_required",
        "Use the dedicated Swift macOS or Kotlin Android harness",
    ))
}
