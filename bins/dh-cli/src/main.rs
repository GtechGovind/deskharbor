use dh_core::engine::{DeskEngine, HostSettings};
use std::{
    ffi::OsStr,
    io::{BufRead, IsTerminal},
};
use zeroize::Zeroizing;
fn password() -> Result<Zeroizing<Vec<u8>>, Box<dyn std::error::Error>> {
    if std::io::stdin().is_terminal() {
        return Err("Supply the password on standard input from a protected source".into());
    }
    let mut s = Zeroizing::new(Vec::new());
    use std::io::Read;
    std::io::stdin()
        .lock()
        .take(1027)
        .read_until(b'\n', &mut s)?;
    if s.last() == Some(&b'\n') {
        s.pop();
    }
    if s.last() == Some(&b'\r') {
        s.pop();
    }
    if s.len() > 1024 {
        return Err("password exceeds 1024 bytes".into());
    }
    Ok(s)
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut a = std::env::args_os().skip(1);
    if a.next().as_deref() != Some(OsStr::new("--data-dir")) {
        return Err(
            "usage: dh-cli --data-dir PATH status|password|enable ADDRESS|disable|connect ADDRESS"
                .into(),
        );
    }
    let e = DeskEngine::new(a.next().ok_or("missing data directory")?.into())?;
    let command = a
        .next()
        .and_then(|s| s.into_string().ok())
        .ok_or("missing command")?;
    let argument = a.next().and_then(|s| s.into_string().ok());
    if a.next().is_some() {
        return Err("unexpected argument".into());
    }
    match (command.as_str(), argument) {
        ("status", None) => println!("{}", serde_json::to_string(&e.host_status().await?)?),
        ("password", None) => e.set_host_password(password()?).await?,
        ("enable", Some(address)) => {
            e.configure_host(HostSettings {
                enabled: true,
                bind_address: address,
                ..Default::default()
            })
            .await?;
        }
        ("disable", None) => {
            e.configure_host(HostSettings::default()).await?;
        }
        ("connect", Some(address)) => {
            let s = e.connect(address, password()?).await?;
            println!(
                "{} {} {} {}",
                s.state, s.transport, s.endpoint, s.host_identity
            );
            e.disconnect(s.id).await?;
        }
        _ => return Err("invalid command or arguments".into()),
    }
    Ok(())
}
