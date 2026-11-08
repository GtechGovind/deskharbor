//! Isolated-lab-only SCM service and fixed-purpose LocalSystem workers.
//! No automatic input: custom SCM controls 128/130/129 opt into lab input/SAS.
//! Entry points do not install, change policy or switch desktops.
use super::*;
use std::{
    fs::File,
    io::Write,
    os::windows::{ffi::OsStrExt, io::FromRawHandle},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{
        ERROR_CALL_NOT_IMPLEMENTED, ERROR_SERVICE_SPECIFIC_ERROR, LocalFree, WAIT_OBJECT_0,
    },
    Security::{
        ACCESS_ALLOWED_ACE,
        Authorization::{
            ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW,
            SDDL_REVISION_1, SE_FILE_OBJECT,
        },
        DACL_SECURITY_INFORMATION, DuplicateTokenEx, GetAce, GetSecurityDescriptorControl,
        GetSecurityDescriptorDacl, GetSecurityDescriptorOwner, IsWellKnownSid,
        OWNER_SECURITY_INFORMATION, RevertToSelf, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES,
        SecurityImpersonation, SetTokenInformation, TOKEN_ALL_ACCESS, TOKEN_DUPLICATE, TOKEN_USER,
        TokenImpersonation, TokenPrimary, TokenSessionId, TokenUser, WinBuiltinAdministratorsSid,
        WinLocalSystemSid,
    },
    Storage::FileSystem::{
        CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL,
        FILE_ATTRIBUTE_REPARSE_POINT, GetFileAttributesW, INVALID_FILE_ATTRIBUTES,
    },
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        },
        Services::*,
        SystemInformation::GetTickCount64,
        Threading::{
            CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW,
            GetExitCodeProcess, PROCESS_INFORMATION, ResumeThread, STARTUPINFOW, SetThreadToken,
            TerminateProcess, WaitForSingleObject,
        },
    },
    UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
        VK_CONTROL, VK_F24, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    },
};

const SERVICE_NAME: &str = "DeskHarborP0Proof";
const INSTALL_EXE: &str = r"C:\Program Files\DeskHarbor\P0\dh-platform-probe.exe";
const OUTPUT: &str = r"C:\ProgramData\DeskHarbor\P0Proof";
const PRIVATE_SDDL: &str = "O:SYG:SYD:P(A;;FA;;;SY)(A;;FA;;;BA)";
const OUTPUT_LIMIT: u64 = 512 * 1024 * 1024;
static STOP: AtomicBool = AtomicBool::new(false);
static SESSION_EVENT: AtomicBool = AtomicBool::new(true);
static SERVICE_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static INPUT_REQUEST: AtomicU64 = AtomicU64::new(0);
static SECURE_INPUT_REQUEST: AtomicU64 = AtomicU64::new(0);
static SAS_REQUEST: AtomicU64 = AtomicU64::new(0);
static INPUT_CONTROLS_ENABLED: AtomicBool = AtomicBool::new(false);
const SERVICE_SDDL: &str = "O:BAG:BAD:P(A;;GA;;;SY)(A;;GA;;;BA)";

unsafe fn service_descriptor_allowed(descriptor: *mut c_void) -> bool {
    let (mut control, mut revision, mut owner, mut defaulted, mut present, mut dacl) =
        (0, 0, null_mut(), 0, 0, null_mut());
    // SAFETY: The caller supplies a live, valid Win32 security descriptor; outputs address live locals.
    let valid = unsafe {
        GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) != 0
            && control & SE_DACL_PROTECTED != 0
            && GetSecurityDescriptorOwner(descriptor, &mut owner, &mut defaulted) != 0
            && (IsWellKnownSid(owner, WinLocalSystemSid) != 0
                || IsWellKnownSid(owner, WinBuiltinAdministratorsSid) != 0)
            && GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) != 0
    };
    if !valid || present == 0 || dacl.is_null() {
        return false;
    }
    // SAFETY: GetSecurityDescriptorDacl supplied a valid ACL embedded in the caller's live descriptor.
    if unsafe { (*dacl).AceCount } != 2 {
        return false;
    }
    let (mut system, mut admins) = (false, false);
    for index in 0..2 {
        let mut pointer = null_mut();
        // SAFETY: The validated ACL has exactly two ACEs; GetAce bounds-checks each index.
        if unsafe { GetAce(dacl, index, &mut pointer) } == 0 || pointer.is_null() {
            return false;
        }
        // SAFETY: The ACL came from Win32. Check ACE header type/size before using ACCESS_ALLOWED_ACE fields.
        let header = unsafe { &*pointer.cast::<windows_sys::Win32::Security::ACE_HEADER>() };
        if header.AceType != 0
            || header.AceFlags != 0
            || usize::from(header.AceSize) < size_of::<ACCESS_ALLOWED_ACE>()
        {
            return false;
        }
        // SAFETY: Type/size identify a valid ACCESS_ALLOWED_ACE; its SID is embedded at SidStart and live with the descriptor.
        let (mask, sid) = unsafe {
            let ace = &*pointer.cast::<ACCESS_ALLOWED_ACE>();
            (ace.Mask, (&ace.SidStart as *const u32).cast_mut().cast())
        };
        if mask != 0x1000_0000 && mask != SERVICE_ALL_ACCESS {
            return false;
        }
        // SAFETY: Win32 owns and validated the ACL's embedded SID.
        if unsafe { IsWellKnownSid(sid, WinLocalSystemSid) } != 0 {
            if system {
                return false;
            }
            system = true;
        }
        // SAFETY: Same valid embedded SID; this query changes no security state.
        else if unsafe { IsWellKnownSid(sid, WinBuiltinAdministratorsSid) } != 0 {
            if admins {
                return false;
            }
            admins = true;
        } else {
            return false;
        }
    }
    system && admins
}

struct ServiceHandle(SC_HANDLE);
impl Drop for ServiceHandle {
    fn drop(&mut self) {
        // SAFETY: This guard owns a valid SCM/service handle returned by its open API.
        unsafe {
            CloseServiceHandle(self.0);
        }
    }
}

fn verify_service_acl() -> Result<(), ProbeError> {
    // SAFETY: Null names select only the local SCM and default service database; read-only connect access.
    let manager = unsafe { OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT) };
    if manager.is_null() {
        return Err(win_error("OpenSCManagerW(read-only ACL check)"));
    }
    let manager = ServiceHandle(manager);
    // SAFETY: Fixed NUL-terminated service name, valid local manager handle, READ_CONTROL only.
    let service = unsafe { OpenServiceW(manager.0, wide(SERVICE_NAME).as_ptr(), 0x0002_0000) };
    if service.is_null() {
        return Err(win_error("OpenServiceW(READ_CONTROL)"));
    }
    let service = ServiceHandle(service);
    let mut bytes = 0;
    // SAFETY: A null zero-length buffer requests the required descriptor size, without modifying the service.
    unsafe {
        QueryServiceObjectSecurity(
            service.0,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            null_mut(),
            0,
            &mut bytes,
        );
    }
    if !(20..=65_536).contains(&bytes) {
        return Err(ProbeError::new(
            "service_acl_query",
            "Invalid service security descriptor size",
        ));
    }
    let mut buffer = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
    // SAFETY: The aligned buffer contains at least bytes writable bytes, with matching security information selectors.
    if unsafe {
        QueryServiceObjectSecurity(
            service.0,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            buffer.as_mut_ptr().cast(),
            bytes,
            &mut bytes,
        )
    } == 0
    {
        return Err(win_error("QueryServiceObjectSecurity"));
    }
    // SAFETY: A successful Win32 query produced a valid descriptor in the live aligned buffer.
    if !unsafe { service_descriptor_allowed(buffer.as_mut_ptr().cast()) } {
        return Err(ProbeError::new(
            "unsafe_service_control_acl",
            format!(
                "Refusing input controls: SCM service owner/DACL must match protected SYSTEM/Administrators-only full access, e.g. {SERVICE_SDDL}"
            ),
        ));
    }
    Ok(())
}

// These narrow declarations avoid enabling the broad authentication/registry bindings.
// Both signatures follow the Microsoft Windows SDK headers sas.h and winreg.h.
#[link(name = "sas")]
unsafe extern "system" {
    fn SendSAS(as_user: i32);
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegGetValueW(
        key: HANDLE,
        subkey: *const u16,
        value: *const u16,
        flags: u32,
        kind: *mut u32,
        data: *mut c_void,
        bytes: *mut u32,
    ) -> i32;
}

fn sas_policy_allows_services(value: Option<u32>) -> bool {
    matches!(value, Some(1 | 3))
}

fn request_is_current(request_session: u32, current_session: Option<u32>, age_ms: u32) -> bool {
    request_session != 0 && current_session == Some(request_session) && age_ms < 2000
}

fn f24_pair() -> [INPUT; 2] {
    [0, KEYEVENTF_KEYUP].map(|flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_F24,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0x4448_5030,
            },
        },
    })
}

fn software_sas_policy() -> Result<Option<u32>, ProbeError> {
    let (mut value, mut kind, mut bytes) = (0u32, 0u32, 4u32);
    let subkey = wide(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System");
    let name = wide("SoftwareSASGeneration");
    let machine = (0x8000_0002u32 as i32 as isize) as HANDLE;
    // SAFETY: The predefined HKLM handle and NUL-terminated constant names are valid. The flags request an exact DWORD from the 64-bit view; no registry write is performed.
    let code = unsafe {
        RegGetValueW(
            machine,
            subkey.as_ptr(),
            name.as_ptr(),
            0x0001_0010,
            &mut kind,
            (&mut value as *mut u32).cast(),
            &mut bytes,
        )
    };
    if code == 2 {
        return Ok(None);
    }
    if code != 0 || kind != 4 || bytes != 4 {
        return Err(ProbeError::new(
            "sas_policy_unreadable",
            format!("SoftwareSASGeneration DWORD query failed: {code}"),
        ));
    }
    Ok(Some(value))
}

fn queue_request(slot: &AtomicU64) -> u32 {
    if !INPUT_CONTROLS_ENABLED.load(Ordering::Acquire) {
        return 5;
    }
    if STOP.load(Ordering::Acquire) {
        return 1061;
    }
    // SAFETY: These no-argument functions only query active console and monotonic uptime.
    let (session, tick) = unsafe { (WTSGetActiveConsoleSessionId(), GetTickCount64() as u32) };
    if session == 0 || session == u32::MAX {
        return 1168;
    }
    let request = (u64::from(session) << 32) | u64::from(tick);
    if slot
        .compare_exchange(0, request, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return 170;
    }
    0
}

fn take_request(slot: &AtomicU64) -> Option<(u32, u32, bool)> {
    let request = slot.swap(0, Ordering::AcqRel);
    if request == 0 {
        return None;
    }
    let requested = (request >> 32) as u32;
    // SAFETY: Both queries are read-only, no-argument Win32 functions.
    let (current, tick) = unsafe { (WTSGetActiveConsoleSessionId(), GetTickCount64() as u32) };
    Some((
        requested,
        request as u32,
        request_is_current(
            requested,
            (current != u32::MAX).then_some(current),
            tick.wrapping_sub(request as u32),
        ),
    ))
}

/// Explicit SCM-launched lab input only. Returns 3 without input on the inactive desktop.
pub fn input_worker_entry(session: u32, desktop: &str) -> Result<u8, ProbeError> {
    let _token = require_local_system()?;
    let descriptor = proof_directory()?;
    let request_tick: u32 = std::env::var("DESKHARBOR_P0_INPUT_REQUEST_TICK")
        .ok()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| {
            ProbeError::new(
                "input_requires_service_request",
                "Explicit input requires the per-child deadline supplied by the lab service",
            )
        })?;
    let start = std::time::Instant::now();
    let before = inspect()?;
    if !worker_observation_allowed(&before, session, desktop) {
        return Ok(3);
    }
    // Refuse a conflicting physical key/modifier state; never release another user's keys.
    for key in [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN, VK_F24] {
        // SAFETY: Each value is a documented virtual-key code; this query changes no input state.
        if unsafe { GetAsyncKeyState(i32::from(key)) } < 0 {
            return Err(ProbeError::new(
                "input_key_already_down",
                "Lab probe requires modifiers and F24 released",
            ));
        }
    }
    // Commit durable intent before any input; if the filesystem is full, send nothing.
    write_proof("input-intent.json", &serde_json::to_vec(&serde_json::json!({"requested_session":session,"requested_desktop":desktop,"input":"F24 down/up","p0_certified":false})).map_err(|e| ProbeError::new("json_error",e.to_string()))?, &descriptor)?;
    let fresh = inspect()?;
    // SAFETY: GetTickCount64 is a read-only monotonic-uptime query.
    let age = unsafe { (GetTickCount64() as u32).wrapping_sub(request_tick) };
    if !worker_observation_allowed(&fresh, session, desktop)
        || !request_is_current(session, fresh.console_session, age)
        || start.elapsed() >= Duration::from_secs(2)
    {
        return Err(ProbeError::new(
            "stale_input_observation",
            "Desktop/session changed or explicit lab input expired before injection",
        ));
    }
    let pair = f24_pair();
    // SAFETY: This explicit LocalSystem-only lab command has validated its active console/desktop. Both initialized INPUT structures form one fixed down/up pair with no arbitrary user-supplied keys.
    let inserted = unsafe { SendInput(2, pair.as_ptr(), size_of::<INPUT>() as i32) };
    let mut release_retry = None;
    if inserted == 1 {
        // SAFETY: A partial two-event insertion may leave our F24 down. Send only its matching key-up on the same worker desktop; do not retry key-down or synthesize other releases.
        release_retry =
            Some(unsafe { SendInput(1, pair[1..].as_ptr(), size_of::<INPUT>() as i32) });
    }
    let after = inspect()?;
    let stable = worker_observation_allowed(&after, session, desktop);
    let evidence = serde_json::json!({"schema_version":1,"scope":"explicit_lab_input_only","requested_session":session,"requested_desktop":desktop,"inserted":inserted,"expected":2,"release_retry":release_retry,"desktop_stable_after":stable,"before":before,"after":after,"status":"api_observation_unvalidated","p0_certified":false});
    write_proof(
        "input-result.json",
        &serde_json::to_vec_pretty(&evidence)
            .map_err(|e| ProbeError::new("json_error", e.to_string()))?,
        &descriptor,
    )?;
    if inserted != 2 || !stable {
        return Err(ProbeError::new(
            "input_unconfirmed",
            "F24 insertion incomplete or desktop/session changed; independent lab observation required",
        ));
    }
    Ok(0)
}

fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}

struct LocalAllocation(*mut c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        // SAFETY: This guard owns a descriptor allocated by a Win32 local-allocation API.
        unsafe {
            LocalFree(self.0);
        }
    }
}

fn private_descriptor() -> Result<LocalAllocation, ProbeError> {
    let mut descriptor = null_mut();
    // SAFETY: The constant UTF-16 SDDL is NUL-terminated; output is a live pointer slot.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide(PRIVATE_SDDL).as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err(win_error("ConvertStringSecurityDescriptor"));
    }
    Ok(LocalAllocation(descriptor))
}

fn require_local_system() -> Result<OwnedHandle, ProbeError> {
    let mut token = null_mut();
    // SAFETY: Requests read/duplicate access to this process's token only.
    if unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_DUPLICATE,
            &mut token,
        )
    } == 0
    {
        return Err(win_error("OpenProcessToken"));
    }
    let token = OwnedHandle(token);
    let mut buffer = [0usize; 64]; // aligned buffer large enough for TOKEN_USER plus a maximum SID
    let mut length = 0;
    // SAFETY: The aligned buffer holds 64 machine words; returned SID pointers are used while it is live.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            size_of_val(&buffer) as u32,
            &mut length,
        )
    } == 0
    {
        return Err(win_error("GetTokenInformation(TokenUser)"));
    }
    // SAFETY: The successful TOKEN_USER query initialized the structure and embedded SID in this live aligned buffer.
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    // SAFETY: The queried SID is valid for this buffer's lifetime.
    if unsafe { IsWellKnownSid(user.User.Sid, WinLocalSystemSid) } == 0 {
        return Err(ProbeError::new(
            "system_identity_required",
            "Lab service and worker commands require LocalSystem; an elevated administrator is not sufficient",
        ));
    }
    Ok(token)
}

fn validate_private_path(
    path: &Path,
    directory: bool,
    expected: &LocalAllocation,
) -> Result<(), ProbeError> {
    let path_w = wide(path);
    // SAFETY: The path is NUL-terminated and this query changes no state.
    let attributes = unsafe { GetFileAttributesW(path_w.as_ptr()) };
    if attributes == INVALID_FILE_ATTRIBUTES
        || (attributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory
        || attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(ProbeError::new(
            "unsafe_proof_directory",
            format!(
                "{} must be a provisioned {} without reparse points",
                path.display(),
                if directory { "directory" } else { "file" }
            ),
        ));
    }
    let (mut owner, mut dacl, mut descriptor) = (null_mut(), null_mut(), null_mut());
    // SAFETY: Outputs refer to the descriptor allocation returned by this read-only query.
    let error = unsafe {
        GetNamedSecurityInfoW(
            path_w.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    if error != 0 {
        return Err(ProbeError::new(
            "directory_security_query_failed",
            format!("GetNamedSecurityInfoW: {error}"),
        ));
    }
    let actual = LocalAllocation(descriptor);
    let (mut control, mut revision, mut present, mut defaulted, mut expected_dacl) =
        (0, 0, 0, 0, null_mut());
    // SAFETY: Both descriptors are valid live Win32 allocations; all output pointers address matching scalar types.
    let valid = unsafe {
        GetSecurityDescriptorControl(actual.0, &mut control, &mut revision) != 0
            && control & SE_DACL_PROTECTED != 0
            && (IsWellKnownSid(owner, WinLocalSystemSid) != 0
                || IsWellKnownSid(owner, WinBuiltinAdministratorsSid) != 0)
            && GetSecurityDescriptorDacl(
                expected.0,
                &mut present,
                &mut expected_dacl,
                &mut defaulted,
            ) != 0
    };
    if !valid || dacl.is_null() || expected_dacl.is_null() {
        return Err(ProbeError::new(
            "unsafe_proof_directory",
            "Proof directory requires a protected SYSTEM/Administrators-only DACL and trusted owner",
        ));
    }
    // SAFETY: Both non-null ACLs are embedded in valid descriptors; AclSize is the allocated ACL byte length.
    let same = unsafe {
        (*dacl).AclSize == (*expected_dacl).AclSize
            && std::slice::from_raw_parts(dacl.cast::<u8>(), (*dacl).AclSize as usize)
                == std::slice::from_raw_parts(
                    expected_dacl.cast::<u8>(),
                    (*expected_dacl).AclSize as usize,
                )
    };
    if !same {
        return Err(ProbeError::new(
            "unsafe_proof_directory",
            format!(
                "{} DACL does not exactly match the documented private lab ACL",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn proof_directory() -> Result<LocalAllocation, ProbeError> {
    let descriptor = private_descriptor()?;
    // The lab administrator provisions these paths. Never repair or follow untrusted preexisting folders.
    for path in [r"C:\ProgramData\DeskHarbor", OUTPUT] {
        validate_private_path(Path::new(path), true, &descriptor)?;
    }
    Ok(descriptor)
}

fn write_proof(
    kind: &str,
    bytes: &[u8],
    descriptor: &LocalAllocation,
) -> Result<PathBuf, ProbeError> {
    let mut occupied = 0u64;
    let mut count = 0;
    for entry in
        std::fs::read_dir(OUTPUT).map_err(|e| ProbeError::new("proof_output_io", e.to_string()))?
    {
        let entry = entry.map_err(|e| ProbeError::new("proof_output_io", e.to_string()))?;
        let metadata = entry
            .path()
            .symlink_metadata()
            .map_err(|e| ProbeError::new("proof_output_io", e.to_string()))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(ProbeError::new(
                "unsafe_proof_output",
                "Unexpected non-file entry in private evidence directory",
            ));
        }
        occupied = occupied.saturating_add(metadata.len());
        count += 1;
    }
    if count >= 4096 || occupied.saturating_add(bytes.len() as u64) > OUTPUT_LIMIT {
        return Err(ProbeError::new(
            "proof_output_limit",
            "Local evidence limited to 512 MiB /4096 files; archive evidence in the lab before restarting",
        ));
    }
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| ProbeError::new("clock_error", e.to_string()))?
        .as_millis();
    let sequence = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = Path::new(OUTPUT).join(format!("{time}-{}-{sequence}-{kind}", std::process::id()));
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    // SAFETY: The internally generated filename has no user-controlled path segments. CREATE_NEW refuses preexisting links; explicit security gives only SYSTEM/Admin access.
    let handle = unsafe {
        CreateFileW(
            wide(&path).as_ptr(),
            windows_sys::Win32::Foundation::GENERIC_WRITE,
            0,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        )
    };
    if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        return Err(win_error("CreateFileW(private evidence)"));
    }
    // SAFETY: CreateFileW returned an owned valid file handle; ownership transfers to File exactly once.
    let mut file = unsafe { File::from_raw_handle(handle) };
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| ProbeError::new("proof_output_io", e.to_string()))?;
    Ok(path)
}

fn worker_observation_allowed(report: &Inspection, session: u32, desktop: &str) -> bool {
    let kind = match desktop {
        "Default" => SessionKind::UserDesktop,
        "Winlogon" => SessionKind::LockedOrSignIn,
        _ => return false,
    };
    report.process_session == Some(session)
        && report.console_session == Some(session)
        && report.desktop.as_deref() == Some(desktop)
        && report.session_kind == kind
        && report.checks.iter().any(|check| {
            check.name == "windows.input_desktop_read"
                && check.status == CheckStatus::Passed
                && check.detail == desktop
        })
}

/// Fixed-purpose child; callable by the CLI but refuses every identity except LocalSystem.
pub fn worker_entry(session: u32, desktop: &str) -> Result<(), ProbeError> {
    let _token = require_local_system()?;
    let descriptor = proof_directory()?;
    let report = inspect()?;
    let allowed = worker_observation_allowed(&report, session, desktop);
    let mut result = serde_json::json!({ "schema_version": 1, "scope": "isolated_lab_system_desktop_observation_only", "requested_session": session, "requested_desktop": desktop, "inspection": report, "capture": { "status": "not_run", "reason": "worker/session/input desktop mismatch" }, "p0_certified": false });
    let capture_result = if allowed {
        report
            .displays
            .iter()
            .find(|d| d.primary)
            .or(report.displays.first())
            .map(|display| capture_current_desktop(&display.id, desktop == "Winlogon"))
            .transpose()
    } else {
        Ok(None)
    };
    let mut failure = None;
    match capture_result {
        Ok(Some(frame)) => {
            let path = write_proof("capture.bmp", &frame.encode_bmp()?, &descriptor)?;
            result["capture"] = serde_json::json!({ "status": "captured_unvalidated", "file": path, "width": frame.width, "height": frame.height, "rgba_bytes": frame.pixels_rgba.len(), "warning": "Pixel content/input/transition correctness requires independent lab validation; API success alone is not a passed proof" });
        }
        Ok(None) => {
            failure = Some(ProbeError::new(
                "inactive_worker_desktop",
                "Worker is not attached to the current active console/input desktop or no monitor exists",
            ));
        }
        Err(error) => {
            result["capture"] = serde_json::json!({ "status": "failed", "error": error });
            failure = Some(error);
        }
    }
    write_proof(
        "worker.json",
        &serde_json::to_vec_pretty(&result)
            .map_err(|e| ProbeError::new("json_error", e.to_string()))?,
        &descriptor,
    )?;
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(())
}

fn launch_worker(session: u32, desktop: &str, input_tick: Option<u32>) -> Result<u32, ProbeError> {
    let source = require_local_system()?;
    let executable = std::env::current_exe()
        .map_err(|e| ProbeError::new("service_executable", e.to_string()))?;
    if !executable
        .as_os_str()
        .eq_ignore_ascii_case(std::ffi::OsStr::new(INSTALL_EXE))
    {
        return Err(ProbeError::new(
            "service_install_path",
            format!(
                "Lab service must execute the fixed administrator-provisioned binary {INSTALL_EXE}"
            ),
        ));
    }
    let descriptor = private_descriptor()?;
    for path in [
        r"C:\Program Files\DeskHarbor",
        r"C:\Program Files\DeskHarbor\P0",
    ] {
        validate_private_path(Path::new(path), true, &descriptor)?;
    }
    validate_private_path(Path::new(INSTALL_EXE), false, &descriptor)?;
    // SAFETY: An unnamed job is local to this service, with default non-inheritable security.
    let job = unsafe { CreateJobObjectW(null(), null()) };
    if job.is_null() {
        return Err(win_error("CreateJobObjectW"));
    }
    let job = OwnedHandle(job);
    // SAFETY: The job information structure is plain zero-initializable Win32 data.
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: Valid job handle and matching initialized information structure.
    if unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    } == 0
    {
        return Err(win_error("SetInformationJobObject(KILL_ON_JOB_CLOSE)"));
    }
    let mut primary = null_mut();
    // SAFETY: Duplicates this LocalSystem token to a separately owned primary token without modifying the source token.
    if unsafe {
        DuplicateTokenEx(
            source.0,
            TOKEN_ALL_ACCESS,
            null(),
            SecurityImpersonation,
            TokenPrimary,
            &mut primary,
        )
    } == 0
    {
        return Err(win_error("DuplicateTokenEx"));
    }
    let primary = OwnedHandle(primary);
    // SAFETY: The token is owned; session is a live u32 queried from the active console by the service.
    if unsafe {
        SetTokenInformation(
            primary.0,
            TokenSessionId,
            (&session as *const u32).cast(),
            size_of::<u32>() as u32,
        )
    } == 0
    {
        return Err(win_error(
            "SetTokenInformation(TokenSessionId); LocalSystem/SeTcbPrivilege required",
        ));
    }
    // SAFETY: STARTUPINFOW and PROCESS_INFORMATION are plain zero-initializable Win32 structures.
    let (mut startup, mut process): (STARTUPINFOW, PROCESS_INFORMATION) =
        unsafe { (zeroed(), zeroed()) };
    startup.cb = size_of::<STARTUPINFOW>() as u32;
    let mut desktop_name = wide(format!(r"WinSta0\{desktop}"));
    startup.lpDesktop = desktop_name.as_mut_ptr();
    let application = wide(INSTALL_EXE);
    let action = if input_tick.is_some() {
        "windows-input-worker"
    } else {
        "windows-worker"
    };
    let mut command = wide(format!("\"{INSTALL_EXE}\" {action} {session} {desktop}"));
    // Build a private per-child environment; never mutate this multithreaded service's environment.
    let mut environment = Vec::<u16>::new();
    if let Some(tick) = input_tick {
        let mut variables: Vec<_> = std::env::vars_os()
            .filter(|(key, _)| {
                !key.eq_ignore_ascii_case(std::ffi::OsStr::new("DESKHARBOR_P0_INPUT_REQUEST_TICK"))
            })
            .collect();
        variables.push((
            "DESKHARBOR_P0_INPUT_REQUEST_TICK".into(),
            tick.to_string().into(),
        ));
        variables.sort_by_key(|(key, _)| key.to_string_lossy().to_uppercase());
        for (key, value) in variables {
            let mut item = key;
            item.push("=");
            item.push(value);
            environment.extend(wide(item));
        }
        environment.push(0);
    }
    // SAFETY: All strings/buffers are live and NUL-terminated; desktop and arguments are fixed internal choices. No handles are inherited and no shell is used.
    if unsafe {
        CreateProcessAsUserW(
            primary.0,
            application.as_ptr(),
            command.as_mut_ptr(),
            null(),
            null(),
            0,
            CREATE_NO_WINDOW
                | CREATE_SUSPENDED
                | if input_tick.is_some() {
                    CREATE_UNICODE_ENVIRONMENT
                } else {
                    0
                },
            if environment.is_empty() {
                null()
            } else {
                environment.as_ptr().cast()
            },
            null(),
            &startup,
            &mut process,
        )
    } == 0
    {
        return Err(win_error("CreateProcessAsUserW(fixed desktop worker)"));
    }
    let process_handle = OwnedHandle(process.hProcess);
    let _thread_handle = OwnedHandle(process.hThread);
    // SAFETY: The new process is still suspended, so it cannot execute before the kill-on-close job owns it.
    if unsafe { AssignProcessToJobObject(job.0, process_handle.0) } == 0 {
        let error = win_error("AssignProcessToJobObject");
        // SAFETY: This is exclusively the suspended worker just created by this function.
        unsafe {
            TerminateProcess(process_handle.0, 125);
        }
        return Err(error);
    }
    // SAFETY: This is the owned initial thread of the suspended worker.
    if unsafe { ResumeThread(_thread_handle.0) } == u32::MAX {
        return Err(win_error("ResumeThread(worker)"));
    }
    for _ in 0..40 {
        // SAFETY: The process handle is owned and valid; waits are bounded to 250ms.
        if unsafe { WaitForSingleObject(process_handle.0, 250) } == WAIT_OBJECT_0 {
            let mut exit = 0;
            // SAFETY: The output points to live storage and the process handle is valid.
            if unsafe { GetExitCodeProcess(process_handle.0, &mut exit) } == 0 {
                return Err(win_error("GetExitCodeProcess"));
            }
            return Ok(exit);
        }
        if STOP.load(Ordering::Acquire) {
            break;
        }
    }
    // SAFETY: Only the fixed worker process created above is terminated on timeout/SCM stop, never another session process.
    unsafe {
        TerminateProcess(process_handle.0, 124);
        WaitForSingleObject(process_handle.0, 2000);
    }
    Err(ProbeError::new(
        "worker_timeout",
        "Owned lab worker stopped after 10 seconds or service stop",
    ))
}

fn set_status(state: u32, error: u32) {
    let handle = SERVICE_HANDLE.load(Ordering::Acquire);
    if handle.is_null() {
        return;
    }
    let status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: if state == SERVICE_RUNNING {
            SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SESSIONCHANGE
        } else {
            0
        },
        dwWin32ExitCode: if error == 0 {
            0
        } else {
            ERROR_SERVICE_SPECIFIC_ERROR
        },
        dwServiceSpecificExitCode: error,
        dwCheckPoint: 0,
        dwWaitHint: if state == SERVICE_STOP_PENDING {
            15000
        } else {
            0
        },
    };
    // SAFETY: SCM gave this status handle; status is a fully initialized local structure.
    unsafe {
        SetServiceStatus(handle, &status);
    }
}

fn invoke_service_sas(
    session: u32,
    tick: u32,
    descriptor: &LocalAllocation,
) -> Result<&'static str, ProbeError> {
    let source = require_local_system()?;
    let policy = software_sas_policy()?;
    if !sas_policy_allows_services(policy) {
        return Err(ProbeError::new(
            "sas_policy_denied",
            "Existing SoftwareSASGeneration must explicitly allow services (DWORD1or3); policy is never modified",
        ));
    }
    let mut primary = null_mut();
    // SAFETY: Duplicate only this service's verified LocalSystem token, without altering the original.
    if unsafe {
        DuplicateTokenEx(
            source.0,
            TOKEN_ALL_ACCESS,
            null(),
            SecurityImpersonation,
            TokenPrimary,
            &mut primary,
        )
    } == 0
    {
        return Err(win_error("DuplicateTokenEx(SAS primary)"));
    }
    let primary = OwnedHandle(primary);
    // SAFETY: Set the owned duplicated token's session to the explicitly requested active-console session.
    if unsafe {
        SetTokenInformation(
            primary.0,
            TokenSessionId,
            (&session as *const u32).cast(),
            size_of::<u32>() as u32,
        )
    } == 0
    {
        return Err(win_error("SetTokenInformation(SAS session)"));
    }
    let mut impersonation = null_mut();
    // SAFETY: The duplicate impersonation token preserves the requested session ID and SYSTEM identity.
    if unsafe {
        DuplicateTokenEx(
            primary.0,
            TOKEN_ALL_ACCESS,
            null(),
            SecurityImpersonation,
            TokenImpersonation,
            &mut impersonation,
        )
    } == 0
    {
        return Err(win_error("DuplicateTokenEx(SAS impersonation)"));
    }
    let impersonation = OwnedHandle(impersonation);
    write_proof("sas-intent.json", &serde_json::to_vec(&serde_json::json!({"session":session,"existing_policy":policy,"action":"SendSAS(FALSE)","p0_certified":false})).map_err(|e| ProbeError::new("json_error",e.to_string()))?, descriptor)?;
    // SAFETY: Read-only no-argument active-console and monotonic-clock queries.
    let (current, age) = unsafe {
        (
            WTSGetActiveConsoleSessionId(),
            (GetTickCount64() as u32).wrapping_sub(tick),
        )
    };
    if !request_is_current(session, (current != u32::MAX).then_some(current), age)
        || STOP.load(Ordering::Acquire)
    {
        return Err(ProbeError::new(
            "stale_sas_request",
            "Console changed or explicit SAS request expired; no SAS invoked",
        ));
    }
    // SAFETY: Null thread selects this service-loop thread. Impersonating a verified SYSTEM token in the intended console session directs service SendSAS to that session.
    if unsafe { SetThreadToken(null(), impersonation.0) } == 0 {
        return Err(win_error("SetThreadToken(SAS console)"));
    }
    // SAFETY: Called only from this registered LocalSystem SCM service after explicit admin control129, durable intent, current-console validation and an existing permissive service SAS policy. FALSE declares service identity. No fallible Rust work occurs before the immediate RevertToSelf; this void API has no success acknowledgement.
    unsafe {
        SendSAS(0);
    }
    // SAFETY: Revert immediately after the single bounded service API call; the service thread must never continue other work impersonating a console token.
    if unsafe { RevertToSelf() } == 0 {
        // SAFETY: Failure to revert must not permit any further filesystem/SCM work under impersonation. Terminate only this service process; kill-on-close jobs clean up its owned workers.
        unsafe {
            TerminateProcess(GetCurrentProcess(), 126);
        }
        std::process::abort();
    }
    Ok(
        "SendSAS invoked; void API gives no success result; independent console transition validation required",
    )
}

unsafe extern "system" fn handler(control: u32, _: u32, _: *mut c_void, _: *mut c_void) -> u32 {
    match control {
        SERVICE_CONTROL_STOP => {
            INPUT_CONTROLS_ENABLED.store(false, Ordering::Release);
            clear_input_requests();
            STOP.store(true, Ordering::Release);
            set_status(SERVICE_STOP_PENDING, 0);
            0
        }
        SERVICE_CONTROL_SESSIONCHANGE => {
            clear_input_requests();
            SESSION_EVENT.store(true, Ordering::Release);
            0
        }
        SERVICE_CONTROL_INTERROGATE => 0,
        128 => queue_request(&INPUT_REQUEST),
        129 => queue_request(&SAS_REQUEST),
        130 => queue_request(&SECURE_INPUT_REQUEST),
        _ => ERROR_CALL_NOT_IMPLEMENTED,
    }
}

fn clear_input_requests() {
    INPUT_REQUEST.store(0, Ordering::Release);
    SECURE_INPUT_REQUEST.store(0, Ordering::Release);
    SAS_REQUEST.store(0, Ordering::Release);
}

fn service_loop() -> Result<(), ProbeError> {
    let _token = require_local_system()?;
    let descriptor = proof_directory()?;
    verify_service_acl()?;
    INPUT_CONTROLS_ENABLED.store(true, Ordering::Release);
    set_status(SERVICE_RUNNING, 0);
    let mut last = std::time::Instant::now() - Duration::from_secs(5);
    let mut jobs = 0;
    while !STOP.load(Ordering::Acquire) {
        for (slot, desktop, control) in [
            (&INPUT_REQUEST, "Default", 128),
            (&SECURE_INPUT_REQUEST, "Winlogon", 130),
        ] {
            if let Some((session, tick, current)) = take_request(slot) {
                verify_service_acl()?;
                let result = if current {
                    launch_worker(session, desktop, Some(tick))
                } else {
                    Err(ProbeError::new(
                        "stale_input_request",
                        "Explicit request expired or active console changed; no input sent",
                    ))
                };
                write_proof("input-dispatch.json", &serde_json::to_vec_pretty(&serde_json::json!({"control":control,"session":session,"expected_desktop":desktop,"result":result,"p0_certified":false})).map_err(|e| ProbeError::new("json_error", e.to_string()))?, &descriptor)?;
            }
        }
        if let Some((session, tick, current)) = take_request(&SAS_REQUEST) {
            verify_service_acl()?;
            let result = if current {
                invoke_service_sas(session, tick, &descriptor)
            } else {
                Err(ProbeError::new(
                    "stale_sas_request",
                    "Explicit SAS request expired or active console changed; SAS not invoked",
                ))
            };
            write_proof("sas-result.json", &serde_json::to_vec_pretty(&serde_json::json!({"control":129,"session":session,"result":result,"p0_certified":false})).map_err(|e| ProbeError::new("json_error", e.to_string()))?, &descriptor)?;
        }
        if SESSION_EVENT.swap(false, Ordering::AcqRel) || last.elapsed() >= Duration::from_secs(5) {
            last = std::time::Instant::now();
            // SAFETY: A read-only no-argument console-session query.
            let session = unsafe { WTSGetActiveConsoleSessionId() };
            let mut results = Vec::new();
            if session != u32::MAX {
                for desktop in ["Default", "Winlogon"] {
                    if STOP.load(Ordering::Acquire) {
                        break;
                    }
                    results.push(serde_json::json!({ "desktop": desktop, "result": launch_worker(session, desktop, None) }));
                }
            }
            let evidence = serde_json::json!({ "schema_version": 1, "scope": "isolated_lab_service_dispatch", "session": (session != u32::MAX).then_some(session), "workers": results, "p0_certified": false });
            write_proof(
                "service.json",
                &serde_json::to_vec_pretty(&evidence)
                    .map_err(|e| ProbeError::new("json_error", e.to_string()))?,
                &descriptor,
            )?;
            jobs += 1;
            if jobs >= 1000 {
                return Err(ProbeError::new(
                    "service_sample_limit",
                    "Lab service stops after 1000 dispatch rounds",
                ));
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(())
}

unsafe extern "system" fn service_main(_: u32, _: *mut *mut u16) {
    let name = wide(SERVICE_NAME);
    // SAFETY: The function pointer has SCM's required ABI, and context is unused.
    let handle = unsafe { RegisterServiceCtrlHandlerExW(name.as_ptr(), Some(handler), null()) };
    if handle.is_null() {
        return;
    }
    SERVICE_HANDLE.store(handle, Ordering::Release);
    set_status(SERVICE_START_PENDING, 0);
    let result = service_loop();
    INPUT_CONTROLS_ENABLED.store(false, Ordering::Release);
    clear_input_requests();
    if let Err(error) = &result
        && let Ok(descriptor) = proof_directory()
    {
        let _ = write_proof(
            "service-error.json",
            &serde_json::to_vec(error).unwrap_or_default(),
            &descriptor,
        );
    }
    set_status(SERVICE_STOPPED, u32::from(result.is_err()));
    SERVICE_HANDLE.store(null_mut(), Ordering::Release);
}

/// Enter SCM dispatch only. Does not create/start/install a service.
pub fn service_entry() -> Result<(), ProbeError> {
    let mut name = wide(SERVICE_NAME);
    let table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: name.as_mut_ptr(),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: null_mut(),
            lpServiceProc: None,
        },
    ];
    // SAFETY: The NUL-terminated service table remains live throughout the blocking SCM dispatcher.
    if unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) } == 0 {
        return Err(win_error(
            "StartServiceCtrlDispatcherW; this command must be started by the isolated-lab SCM",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> Inspection {
        Inspection {
            schema_version: 1,
            platform: Platform::Windows,
            environment: "fixture".into(),
            elevated: true,
            process_session: Some(2),
            console_session: Some(2),
            desktop: Some("Winlogon".into()),
            session_kind: SessionKind::LockedOrSignIn,
            displays: vec![],
            checks: vec![Check {
                name: "windows.input_desktop_read".into(),
                status: CheckStatus::Passed,
                detail: "Winlogon".into(),
            }],
            warnings: vec![],
        }
    }

    #[test]
    fn secure_worker_never_accepts_wrong_session_desktop_or_unknown_observation() {
        let mut observation = report();
        assert!(worker_observation_allowed(&observation, 2, "Winlogon"));
        assert!(!worker_observation_allowed(&observation, 3, "Winlogon"));
        assert!(!worker_observation_allowed(&observation, 2, "Default"));
        observation.desktop = Some("Default".into());
        assert!(!worker_observation_allowed(&observation, 2, "Winlogon"));
        observation.desktop = Some("Winlogon".into());
        observation.checks[0].status = CheckStatus::Failed;
        assert!(!worker_observation_allowed(&observation, 2, "Winlogon"));
        observation.checks[0].status = CheckStatus::Passed;
        observation.console_session = None;
        assert!(!worker_observation_allowed(&observation, 2, "Winlogon"));
    }

    #[test]
    fn sas_requires_existing_explicit_service_permission() {
        assert!(sas_policy_allows_services(Some(1)));
        assert!(sas_policy_allows_services(Some(3)));
        for policy in [None, Some(0), Some(2), Some(4), Some(u32::MAX)] {
            assert!(!sas_policy_allows_services(policy));
        }
    }

    #[test]
    fn service_control_acl_refuses_authenticated_users_and_unprotected_defaults() {
        for (sddl, allowed) in [
            (SERVICE_SDDL, true),
            ("O:BAG:BAD:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;CR;;;AU)", false),
            ("O:BAG:BAD:(A;;GA;;;SY)(A;;GA;;;BA)", false),
            ("O:BAG:BAD:P", false),
        ] {
            let mut pointer = null_mut();
            assert_ne!(
                // SAFETY: Valid constant SDDL and live output pointer, allocated by Win32 for this fixture.
                unsafe {
                    ConvertStringSecurityDescriptorToSecurityDescriptorW(
                        wide(sddl).as_ptr(),
                        SDDL_REVISION_1,
                        &mut pointer,
                        null_mut(),
                    )
                },
                0
            );
            let descriptor = LocalAllocation(pointer);
            // SAFETY: descriptor is a valid live Win32 security descriptor allocation.
            assert_eq!(unsafe { service_descriptor_allowed(descriptor.0) }, allowed);
        }
    }

    #[test]
    fn explicit_input_request_expires_or_dies_when_console_changes() {
        assert!(request_is_current(2, Some(2), 1999));
        assert!(!request_is_current(2, Some(2), 2000));
        assert!(!request_is_current(2, Some(3), 0));
        assert!(!request_is_current(2, None, 0));
        assert!(!request_is_current(0, Some(0), 0));
    }

    #[test]
    fn same_console_lock_transition_cannot_retarget_a_default_desktop_request() {
        let secure = report();
        let mut user = report();
        user.desktop = Some("Default".into());
        user.session_kind = SessionKind::UserDesktop;
        user.checks[0].detail = "Default".into();
        assert!(worker_observation_allowed(&user, 2, "Default"));
        assert_eq!(user.console_session, secure.console_session);
        assert!(!worker_observation_allowed(&secure, 2, "Default"));
        assert!(!worker_observation_allowed(&user, 2, "Winlogon"));
    }

    #[test]
    fn f24_fixture_contains_only_one_key_down_and_matching_key_up() {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
        let pair = f24_pair();
        assert_eq!(pair[0].r#type, INPUT_KEYBOARD);
        assert_eq!(pair[1].r#type, INPUT_KEYBOARD);
        // SAFETY: The tested function must create INPUT_KEYBOARD structures, whose active union field is ki.
        let (down, up) = unsafe { (pair[0].Anonymous.ki, pair[1].Anonymous.ki) };
        assert_eq!(down.wVk, 0x87);
        assert_eq!(up.wVk, 0x87);
        assert_eq!(down.dwFlags, 0);
        assert_eq!(up.dwFlags, KEYEVENTF_KEYUP);
        assert_eq!(down.wScan, 0);
        assert_eq!(up.wScan, 0);
        assert_eq!(down.dwExtraInfo, up.dwExtraInfo);
    }
}
