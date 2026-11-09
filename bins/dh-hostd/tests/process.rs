use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    time::Duration,
};
#[test]
fn independent_daemon_authenticates_a_separate_cli_process() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let build = Command::new(env!("CARGO"))
        .args(["build", "--jobs", "1", "--manifest-path"])
        .arg(manifest)
        .args(["-p", "dh-cli", "--message-format=json-render-diagnostics"])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "CLI build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let cli = std::str::from_utf8(&build.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find_map(|artifact| {
            (artifact["reason"] == "compiler-artifact" && artifact["target"]["name"] == "dh-cli")
                .then(|| {
                    artifact["executable"]
                        .as_str()
                        .map(std::path::PathBuf::from)
                })
                .flatten()
        })
        .expect("Cargo must identify the executable built from this checkout");
    let cfg = |cmd: &str| {
        let mut c = Command::new(&cli);
        c.arg("--data-dir").arg(tmp.path()).arg(cmd);
        c
    };
    let mut passwd = cfg("password")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    passwd
        .stdin
        .take()
        .unwrap()
        .write_all(b"distinct-process-password\n")
        .unwrap();
    assert!(passwd.wait().unwrap().success());
    assert!(cfg("enable").arg("127.0.0.1:0").status().unwrap().success());
    let mut daemon = Command::new(env!("CARGO_BIN_EXE_dh-hostd"))
        .arg("--data-dir")
        .arg(tmp.path())
        .arg("--synthetic-loopback")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let output = daemon.stdout.take().unwrap();
    let _guard = Child(daemon);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(output).read_line(&mut line);
        let _ = tx.send(line);
    });
    let line = rx.recv_timeout(Duration::from_secs(12)).unwrap();
    let status: serde_json::Value = serde_json::from_str(&line).unwrap();
    let addr = status["addresses"][0].as_str().unwrap();
    let mut connection = cfg("connect")
        .arg(addr)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    connection
        .stdin
        .take()
        .unwrap()
        .write_all(b"distinct-process-password\n")
        .unwrap();
    let result = connection.wait_with_output().unwrap();
    assert!(result.status.success());
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(text.contains("connected"));
    assert!(!text.contains("password"));
    // Kill without running the daemon's Rust destructors. Leave its published row in place.
    drop(_guard);
    let owner = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(tmp.path().join("deskharbor-private/daemon.lock"))
        .unwrap();
    owner.try_lock().unwrap();
    let observers = (0..4)
        .map(|_| cfg("status").stdout(Stdio::piped()).spawn().unwrap())
        .collect::<Vec<_>>();
    for child in observers {
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            status["running"], false,
            "status readers must not revive a crashed host"
        );
    }
}
