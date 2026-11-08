use std::{fs, process::Command};

fn probe(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_dh-platform-probe"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn unknown_command_and_incomplete_capture_fail_without_side_effects() {
    assert_eq!(probe(&["invented-command"]).status.code(), Some(1));
    let output = probe(&["capture", "--output"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage"));
}

#[test]
fn template_is_complete_but_cannot_pass_gate() {
    let output = probe(&["gate-template"]);
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["proofs"].as_array().unwrap().len(), 24);
    let root = std::env::temp_dir().join(format!("deskharbor-cli-test-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("proof.json"), &output.stdout).unwrap();
    let report = probe(&[
        "gate",
        "--evidence",
        root.join("proof.json").to_str().unwrap(),
        "--artifact-root",
        root.to_str().unwrap(),
    ]);
    assert_eq!(report.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(value["ready"], false);
    assert_eq!(value["passed"], 0);
    fs::remove_file(root.join("proof.json")).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn help_names_read_only_default_and_explicit_capture() {
    let output = probe(&["--help"]);
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("inspect"));
    assert!(help.contains("capture --display"));
    assert!(help.contains("gate-template"));
}
