use dh_core::engine::{DeskEngine, HostBackend};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--data-dir")) {
        return Err("usage: dh-hostd --data-dir PATH [--synthetic-loopback]".into());
    }
    let root = args.next().ok_or("missing data directory")?;
    let backend = match args.next() {
        None => HostBackend::Native,
        Some(s) if s == "--synthetic-loopback" => HostBackend::Synthetic,
        _ => return Err("unknown backend".into()),
    };
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let engine = DeskEngine::new(root.into())?;
    let service = engine.start_daemon(backend).await?;
    println!("{}", serde_json::to_string(&engine.host_status().await?)?);
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate())?;
        tokio::select! {result=tokio::signal::ctrl_c()=>result?,_=terminate.recv()=>()}
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;
    service.stop().await?;
    Ok(())
}
