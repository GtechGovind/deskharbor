mod authentication;
mod channels;
mod tls;
mod transport;
pub use channels::Channel;
pub use tls::Identity;
pub use transport::{Listener, connect};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportKind {
    Quic,
    Tcp,
}
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("transport error: {0}")]
    Io(#[from] std::io::Error),
    #[error("authentication failed")]
    Authentication,
    #[error("protocol error: {0}")]
    Protocol(&'static str),
    #[error("host identity changed")]
    IdentityChanged,
    #[error("connection timed out")]
    Timeout,
    #[error("TLS error: {0}")]
    Tls(#[from] rustls::Error),
}
pub(crate) const EXPORTER_LABEL: &[u8] = b"EXPORTER-DeskHarbor-Authentication-v1";
pub(crate) const EXPORTER_CONTEXT: &[u8] = b"deskharbor/1";
pub(crate) const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
mod attachment;
