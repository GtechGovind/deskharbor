use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    time::Duration,
};
#[test]
fn independent_daemon_authenticates_a_separate_cli_process() {
    let tmp = tempfile::tempdir().unwrap();
    let cli = std::env::var_os("CARGO_BIN_EXE_dh-cli")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_BIN_EXE_dh-hostd"))
                .with_file_name(format!("dh-cli{}", std::env::consts::EXE_SUFFIX))
        });
    if !cli.exists() {
        assert!(
            Command::new(env!("CARGO"))
                .args(["build", "-p", "dh-cli"])
                .status()
                .unwrap()
                .success()
        );
    }
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
}
