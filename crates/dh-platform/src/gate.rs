//! A lab evidence gate, not an inference from successful compilation or one screenshot.
use crate::report::{CheckStatus, ProbeError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Read,
    path::{Component, Path},
};

const REQUIRED: &[(&str, &str)] = &[
    ("windows_boot_sign_in", "windows_11_x64"),
    ("windows_lock_unlock", "windows_11_x64"),
    ("windows_logout", "windows_11_x64"),
    ("windows_uac", "windows_11_x64"),
    ("windows_sas", "windows_11_x64"),
    ("windows_gui_independent", "windows_11_x64"),
    ("mac_arm64_loginwindow", "macos_arm64"),
    ("mac_arm64_logout", "macos_arm64"),
    ("mac_arm64_second_user", "macos_arm64"),
    ("mac_arm64_permission_revocation", "macos_arm64"),
    ("mac_intel_loginwindow", "macos_x64"),
    ("mac_intel_logout", "macos_x64"),
    ("mac_intel_second_user", "macos_x64"),
    ("mac_intel_permission_revocation", "macos_x64"),
    ("linux_intel_gdm", "ubuntu_24_04_intel"),
    ("linux_intel_complete_planes", "ubuntu_24_04_intel"),
    ("linux_intel_input_transitions", "ubuntu_24_04_intel"),
    ("linux_amd_gdm", "ubuntu_24_04_amd"),
    ("linux_amd_complete_planes", "ubuntu_24_04_amd"),
    ("linux_amd_input_transitions", "ubuntu_24_04_amd"),
    ("android_consent", "android_arm64"),
    ("android_foreground", "android_arm64"),
    ("android_input_permission", "android_arm64"),
    ("android_capture_stop", "android_arm64"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub path: String,
    pub sha256: String,
    #[serde(default)]
    pub kind: ArtifactKind,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    #[default]
    Diagnostic,
    Observation,
    Signing,
    Permissions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub check: String,
    pub expected: String,
    pub observed: String,
    pub artifact: String,
    pub locator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledRole {
    pub role: String,
    pub path: String,
    pub identifier: String,
    pub team_id: String,
    pub designated_requirement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacContext {
    pub roles: Vec<InstalledRole>,
    pub launch_domain: String,
    pub security_session: String,
    pub permission_state: String,
    pub signing_artifact: String,
    pub permission_artifact: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    pub operator: String,
    pub observed_utc: String,
    pub build_id: String,
    pub checks: Vec<Observation>,
    pub macos: Option<MacContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    pub id: String,
    pub profile: String,
    pub environment: String,
    pub status: CheckStatus,
    pub observations: String,
    pub artifacts: Vec<Artifact>,
    #[serde(default)]
    pub attestation: Option<Attestation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub schema_version: u32,
    pub build_id: String,
    pub proofs: Vec<Proof>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateReport {
    pub ready: bool,
    pub required: usize,
    pub passed: usize,
    pub blockers: Vec<String>,
    pub qualification: String,
}

pub fn template() -> Evidence {
    Evidence {
        schema_version: 1,
        build_id: String::new(),
        proofs: REQUIRED
            .iter()
            .map(|(id, profile)| Proof {
                id: (*id).into(),
                profile: (*profile).into(),
                environment: String::new(),
                status: CheckStatus::NotRun,
                observations: String::new(),
                artifacts: vec![],
                attestation: None,
            })
            .collect(),
    }
}

pub fn evaluate(evidence: &Evidence, artifact_root: &Path) -> Result<GateReport, ProbeError> {
    let root = artifact_root
        .canonicalize()
        .map_err(|e| ProbeError::new("artifact_root", e.to_string()))?;
    let mut blockers = Vec::new();
    if evidence.schema_version != 1 {
        blockers.push("Unsupported evidence schema".into());
    }
    if evidence.build_id.trim().is_empty() {
        blockers.push("Missing build identity".into());
    }
    let mut seen = HashSet::new();
    for proof in &evidence.proofs {
        if !seen.insert(&proof.id) {
            blockers.push(format!("{}: duplicate proof", proof.id));
        }
        if !REQUIRED.iter().any(|(id, _)| *id == proof.id) {
            blockers.push(format!("{}: unknown proof", proof.id));
        }
    }
    let mut passed = 0;
    for (id, profile) in REQUIRED {
        let matches: Vec<_> = evidence.proofs.iter().filter(|p| p.id == *id).collect();
        if matches.len() != 1 {
            blockers.push(format!("{id}: exactly one result required"));
            continue;
        }
        let proof = matches[0];
        let problem = if proof.status != CheckStatus::Passed {
            Some("not passed".into())
        } else if proof.profile != *profile {
            Some("wrong hardware/OS profile".into())
        } else if proof.environment != "native_lab" {
            Some("native laboratory proof required".into())
        } else if proof.observations.trim().is_empty() {
            Some("missing observations".into())
        } else if proof.artifacts.is_empty() {
            Some("missing evidence artifacts".into())
        } else if let Err(problem) = validate_attestation(proof, &evidence.build_id) {
            Some(problem)
        } else {
            proof
                .artifacts
                .iter()
                .find_map(|a| verify_artifact(a, &root).err())
        };
        if let Some(problem) = problem {
            blockers.push(format!("{id}: {problem}"));
        } else {
            passed += 1;
        }
    }
    Ok(GateReport { ready: blockers.is_empty(), required: REQUIRED.len(), passed, blockers,
        qualification: "Checks operator-supplied lab attestations and artifact integrity; does not independently verify hardware, actions, or signing identity. Passing this gate permits further development, not product release.".into() })
}

pub fn required_checks(id: &str) -> &'static [&'static str] {
    match id {
        "windows_boot_sign_in" => &["capture", "input", "boot_sign_in"],
        "windows_lock_unlock" => &["capture", "input", "lock_unlock"],
        "windows_logout" => &["capture", "input", "logout"],
        "windows_uac" => &["capture", "input", "uac_secure_desktop"],
        "windows_sas" => &["capture", "secure_attention"],
        "windows_gui_independent" => &["capture", "input", "gui_closed"],
        "android_consent" => &["consent_accept", "consent_deny", "fresh_consent"],
        "android_foreground" => &["capture", "foreground_service", "background_gui"],
        "android_input_permission" => &["input", "permission_denial", "permission_revocation"],
        "android_capture_stop" => &["projection_stop", "cleanup", "no_stale_frames"],
        id if id.starts_with("mac_") && id.ends_with("loginwindow") => {
            &["capture", "input", "loginwindow"]
        }
        id if id.starts_with("mac_") && id.ends_with("logout") => &["capture", "input", "logout"],
        id if id.starts_with("mac_") && id.ends_with("second_user") => {
            &["capture", "input", "user_isolation"]
        }
        id if id.starts_with("mac_") && id.ends_with("permission_revocation") => {
            &["permission_denial", "cleanup"]
        }
        id if id.starts_with("linux_") && id.ends_with("gdm") => &["capture", "input", "gdm"],
        id if id.starts_with("linux_") && id.ends_with("complete_planes") => &[
            "reference_comparison",
            "cursor_overlays",
            "direct_scanout",
            "rotation",
            "mixed_monitors",
            "modifiers",
        ],
        id if id.starts_with("linux_") && id.ends_with("input_transitions") => {
            &["input", "session_transitions", "release_held_input"]
        }
        _ => &[],
    }
}

fn validate_attestation(proof: &Proof, build: &str) -> Result<(), String> {
    let a = proof
        .attestation
        .as_ref()
        .ok_or("missing structured operator attestation")?;
    if a.operator.trim().is_empty() || a.build_id != build || !valid_utc(&a.observed_utc) {
        return Err("attestation needs operator, matching build identity and UTC timestamp".into());
    }
    let mut paths = HashSet::new();
    if !proof.artifacts.iter().all(|a| paths.insert(&a.path)) {
        return Err("duplicate artifact paths".into());
    }
    for check in required_checks(&proof.id) {
        let entries: Vec<_> = a
            .checks
            .iter()
            .filter(|entry| entry.check == *check)
            .collect();
        if entries.len() != 1 {
            return Err(format!("missing or duplicate observed check: {check}"));
        }
        let entry = entries[0];
        if entry.expected.trim().is_empty()
            || entry.observed.trim().is_empty()
            || entry.locator.trim().is_empty()
            || !proof
                .artifacts
                .iter()
                .any(|a| a.path == entry.artifact && a.kind == ArtifactKind::Observation)
        {
            return Err(format!(
                "{check}: expected/observed result and located observation artifact required; diagnostics alone do not qualify"
            ));
        }
    }
    if proof.id.starts_with("mac_") {
        let mac = a
            .macos
            .as_ref()
            .ok_or("installed macOS security context required")?;
        let valid_session = if proof.id.ends_with("loginwindow") || proof.id.ends_with("logout") {
            mac.security_session == "loginwindow"
        } else if proof.id.ends_with("second_user") {
            mac.security_session == "aqua_second_user"
        } else {
            matches!(mac.security_session.as_str(), "aqua" | "loginwindow")
        };
        let permission = if proof.id.ends_with("permission_revocation") {
            "revoked_and_denied"
        } else {
            "granted"
        };
        if mac.launch_domain.trim().is_empty()
            || !valid_session
            || mac.permission_state != permission
        {
            return Err("wrong macOS launch/security/permission context".into());
        }
        let mut team = None;
        for role in ["hostd", "agent", "capture-helper"] {
            let matches: Vec<_> = mac.roles.iter().filter(|r| r.role == role).collect();
            if matches.len() != 1 {
                return Err(format!("missing/duplicate installed macOS {role} identity"));
            }
            let r = matches[0];
            if r.path != format!("/Library/DeskHarbor/P0/dh-{role}")
                || r.identifier != format!("org.deskharbor.p0.{role}")
                || r.team_id.len() != 10
                || !r
                    .team_id
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
                || r.designated_requirement.trim().is_empty()
            {
                return Err(format!("invalid installed macOS {role} signing identity"));
            }
            if team.is_some_and(|t| t != &r.team_id) {
                return Err("macOS role signing teams differ".into());
            }
            team = Some(&r.team_id);
        }
        for (path, kind) in [
            (&mac.signing_artifact, ArtifactKind::Signing),
            (&mac.permission_artifact, ArtifactKind::Permissions),
        ] {
            if !proof
                .artifacts
                .iter()
                .any(|a| &a.path == path && a.kind == kind)
            {
                return Err("missing macOS signature-verification or permission artifact".into());
            }
        }
    }
    Ok(())
}

fn valid_utc(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return false;
    }
    let number = |range: std::ops::Range<usize>| -> Option<u32> {
        b[range].iter().try_fold(0u32, |n, digit| {
            digit
                .is_ascii_digit()
                .then_some(n * 10 + u32::from(digit.wrapping_sub(b'0')))
        })
    };
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        number(0..4),
        number(5..7),
        number(8..10),
        number(11..13),
        number(14..16),
        number(17..19),
    ) else {
        return false;
    };
    let leap = year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    year >= 1970 && day >= 1 && day <= days && hour < 24 && minute < 60 && second < 60
}

fn verify_artifact(artifact: &Artifact, root: &Path) -> Result<(), String> {
    let path = Path::new(&artifact.path);
    if artifact.path.is_empty()
        || artifact.path.contains(['\\', ':'])
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("artifact path must be a portable relative path without traversal".into());
    }
    if artifact.sha256.len() != 64 || !artifact.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid SHA-256".into());
    }
    let original = root.join(path);
    if !fs::symlink_metadata(&original)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("artifact must be a regular file, not a symlink or device".into());
    }
    let resolved = original
        .canonicalize()
        .map_err(|e| format!("artifact: {e}"))?;
    if !resolved.starts_with(root) {
        return Err("artifact escapes evidence directory".into());
    }
    // Evidence trees must be private and immutable during validation. Reject special
    // files before opening; O_NONBLOCK also closes the FIFO substitution hang.
    if !fs::symlink_metadata(&resolved)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("artifact must be a file".into());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let mut file = options.open(&resolved).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    const MAX_ARTIFACT: u64 = 1024 * 1024 * 1024;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_ARTIFACT {
        return Err("artifact must be a regular file <=1 GiB".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("artifact reparse point refused".into());
        }
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut total = 0u64;
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > MAX_ARTIFACT {
            return Err("artifact grew beyond 1 GiB".into());
        }
        hash.update(&buffer[..count]);
    }
    if total == 0 || format!("{:x}", hash.finalize()) != artifact.sha256.to_ascii_lowercase() {
        return Err("artifact SHA-256 mismatch".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::CheckStatus;
    use std::{fs, path::Path};

    #[test]
    fn utc_observation_date_must_exist_on_the_calendar() {
        assert!(valid_utc("2024-02-29T23:59:59Z"));
        for invalid in [
            "2026-02-29T00:00:00Z",
            "2026-13-08T00:00:00Z",
            "2026-11-08T24:00:00Z",
            "not a real timestamp T Z",
            "2026-11-08T00:00:00+00:00",
        ] {
            assert!(!valid_utc(invalid), "{invalid}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn fifo_artifact_returns_an_error_instead_of_waiting_for_a_writer() {
        use std::os::unix::ffi::OsStrExt;
        use std::{ffi::CString, sync::mpsc, time::Duration};
        let root = std::env::temp_dir().join(format!("deskharbor-fifo-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let path = CString::new(root.join("pipe").as_os_str().as_bytes()).unwrap();
        // SAFETY: The string is live and terminated; mkfifo only creates this test's private path.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        let (tx, rx) = mpsc::channel();
        let worker_root = root.clone();
        std::thread::spawn(move || {
            let _ = tx.send(verify_artifact(
                &artifact("pipe", ArtifactKind::Observation),
                &worker_root,
            ));
        });
        let response = rx.recv_timeout(Duration::from_secs(2));
        fs::remove_file(root.join("pipe")).unwrap();
        fs::remove_dir(root).unwrap();
        assert!(response.expect("FIFO validation must not block").is_err());
    }

    #[test]
    fn a_hash_alone_is_not_a_human_lab_attestation() {
        let root = std::env::temp_dir().join(format!(
            "deskharbor-attestation-fixture-{}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("evidence.bin"), b"abc").unwrap();
        let mut evidence = template();
        evidence.build_id = "test-fixture".into();
        for proof in &mut evidence.proofs {
            proof.status = CheckStatus::Passed;
            proof.environment = "native_lab".into();
            proof.observations = "ordinary desktop report reused as a purported pass".into();
            proof.artifacts = vec![artifact("evidence.bin", ArtifactKind::Diagnostic)];
        }
        let report = evaluate(&evidence, &root).unwrap();
        fs::remove_file(root.join("evidence.bin")).unwrap();
        fs::remove_dir(root).unwrap();
        assert!(!report.ready);
        assert_eq!(report.passed, 0);
    }

    #[test]
    fn an_empty_or_unrun_lab_can_never_release_the_gate() {
        let result = evaluate(&template(), Path::new(".")).unwrap();
        assert!(!result.ready);
        assert_eq!(result.required, 24);
        assert_eq!(result.passed, 0);
        assert!(
            result
                .blockers
                .iter()
                .any(|s| s.contains("windows_boot_sign_in"))
        );
        assert!(
            result
                .blockers
                .iter()
                .any(|s| s.contains("linux_amd_complete_planes"))
        );
    }

    #[test]
    fn duplicate_results_cannot_replace_missing_proofs() {
        let mut evidence = template();
        evidence.proofs = vec![
            Proof {
                id: "windows_boot_sign_in".into(),
                profile: "windows_11_x64".into(),
                environment: "native_lab".into(),
                status: CheckStatus::Passed,
                observations: "real fixture".into(),
                artifacts: vec![],
                attestation: None,
            };
            24
        ];
        assert!(!evaluate(&evidence, Path::new(".")).unwrap().ready);
    }

    #[test]
    fn complete_native_lab_fixture_releases_only_after_artifact_hash_verification() {
        // The list comes from the accepted product's platform requirements, not the evaluator.
        let cases = [
            ("windows_boot_sign_in", "windows_11_x64"),
            ("windows_lock_unlock", "windows_11_x64"),
            ("windows_logout", "windows_11_x64"),
            ("windows_uac", "windows_11_x64"),
            ("windows_sas", "windows_11_x64"),
            ("windows_gui_independent", "windows_11_x64"),
            ("mac_arm64_loginwindow", "macos_arm64"),
            ("mac_arm64_logout", "macos_arm64"),
            ("mac_arm64_second_user", "macos_arm64"),
            ("mac_arm64_permission_revocation", "macos_arm64"),
            ("mac_intel_loginwindow", "macos_x64"),
            ("mac_intel_logout", "macos_x64"),
            ("mac_intel_second_user", "macos_x64"),
            ("mac_intel_permission_revocation", "macos_x64"),
            ("linux_intel_gdm", "ubuntu_24_04_intel"),
            ("linux_intel_complete_planes", "ubuntu_24_04_intel"),
            ("linux_intel_input_transitions", "ubuntu_24_04_intel"),
            ("linux_amd_gdm", "ubuntu_24_04_amd"),
            ("linux_amd_complete_planes", "ubuntu_24_04_amd"),
            ("linux_amd_input_transitions", "ubuntu_24_04_amd"),
            ("android_consent", "android_arm64"),
            ("android_foreground", "android_arm64"),
            ("android_input_permission", "android_arm64"),
            ("android_capture_stop", "android_arm64"),
        ];
        let root =
            std::env::temp_dir().join(format!("deskharbor-proof-fixture-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("evidence.bin"), b"abc").unwrap();
        fs::write(root.join("signing.txt"), b"abc").unwrap();
        fs::write(root.join("permissions.txt"), b"abc").unwrap();
        let proofs = cases
            .iter()
            .map(|(id, profile)| Proof {
                id: (*id).into(),
                profile: (*profile).into(),
                environment: "native_lab".into(),
                status: CheckStatus::Passed,
                observations: "Independent test fixture only; never a certification report".into(),
                artifacts: vec![
                    artifact("evidence.bin", ArtifactKind::Observation),
                    artifact("signing.txt", ArtifactKind::Signing),
                    artifact("permissions.txt", ArtifactKind::Permissions),
                ],
                attestation: Some(fixture_attestation(id)),
            })
            .collect();
        let mut evidence = Evidence {
            schema_version: 1,
            build_id: "test-fixture".into(),
            proofs,
        };
        let success = evaluate(&evidence, &root).unwrap();
        assert!(success.ready);
        assert_eq!(success.passed, 24);
        evidence.proofs[6]
            .attestation
            .as_mut()
            .unwrap()
            .macos
            .as_mut()
            .unwrap()
            .security_session = "aqua".into();
        assert!(!evaluate(&evidence, &root).unwrap().ready);
        evidence.proofs[6].attestation = Some(fixture_attestation("mac_arm64_loginwindow"));
        evidence.proofs[0]
            .attestation
            .as_mut()
            .unwrap()
            .checks
            .clear();
        assert!(!evaluate(&evidence, &root).unwrap().ready);
        evidence.proofs[0].attestation = Some(fixture_attestation("windows_boot_sign_in"));
        fs::write(root.join("evidence.bin"), b"tampered").unwrap();
        assert!(!evaluate(&evidence, &root).unwrap().ready);
        fs::write(root.join("evidence.bin"), b"abc").unwrap();
        evidence.proofs[0].environment = "current_desktop".into();
        assert!(!evaluate(&evidence, &root).unwrap().ready);
        evidence.proofs[0].environment = "native_lab".into();
        evidence.proofs[0].artifacts[0].path = "../escape.bin".into();
        assert!(!evaluate(&evidence, &root).unwrap().ready);
        fs::remove_file(root.join("evidence.bin")).unwrap();
        fs::remove_file(root.join("signing.txt")).unwrap();
        fs::remove_file(root.join("permissions.txt")).unwrap();
        fs::remove_dir(root).unwrap();
    }

    fn artifact(path: &str, kind: ArtifactKind) -> Artifact {
        Artifact {
            path: path.into(),
            kind,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
        }
    }

    fn fixture_attestation(id: &str) -> Attestation {
        Attestation {
            operator: "unit-test operator; NOT actual lab evidence".into(),
            observed_utc: "2026-11-08T00:00:00Z".into(),
            build_id: "test-fixture".into(),
            checks: required_checks(id)
                .iter()
                .map(|check| Observation {
                    check: (*check).into(),
                    expected: "fixture expected result".into(),
                    observed: "fixture observation only".into(),
                    artifact: "evidence.bin".into(),
                    locator: format!("test fixture for {id}/{check}"),
                })
                .collect(),
            macos: id.starts_with("mac_").then(|| MacContext {
                roles: ["hostd", "agent", "capture-helper"]
                    .iter()
                    .map(|role| InstalledRole {
                        role: (*role).into(),
                        path: format!("/Library/DeskHarbor/P0/dh-{role}"),
                        identifier: format!("org.deskharbor.p0.{role}"),
                        team_id: "TESTTEAM01".into(),
                        designated_requirement: "test fixture only".into(),
                    })
                    .collect(),
                launch_domain: "fixture domain".into(),
                security_session: if id.ends_with("second_user") {
                    "aqua_second_user"
                } else {
                    "loginwindow"
                }
                .into(),
                permission_state: if id.ends_with("permission_revocation") {
                    "revoked_and_denied"
                } else {
                    "granted"
                }
                .into(),
                signing_artifact: "signing.txt".into(),
                permission_artifact: "permissions.txt".into(),
            }),
        }
    }
}
