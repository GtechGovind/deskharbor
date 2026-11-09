pub mod frame;
pub mod gate;
pub mod report;

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;
pub mod runtime;
/// A checked application-private directory. Its traversal policies are implemented by the runtime.
pub struct PrivateFiles {
    root: std::path::PathBuf,
}

/// This network daemon has no machine-service privilege boundary yet.
pub fn ordinary_host_allowed() -> bool {
    #[cfg(windows)]
    {
        windows::inspect().is_ok_and(|v| {
            !v.elevated
                && v.checks.iter().any(|c| {
                    c.name == "windows.token_elevation_query"
                        && c.status == report::CheckStatus::Passed
                })
        })
    }
    #[cfg(unix)]
    {
        // SAFETY: geteuid reads the effective identity without arguments or side effects.
        unsafe { libc::geteuid() != 0 }
    }
}
