mod framing;
pub use framing::{read_message, write_message};
include!(concat!(env!("OUT_DIR"), "/deskharbor.rs"));

pub const VERSION: u32 = 1;
pub const ALPN: &[u8] = b"deskharbor/1";
pub const DEFAULT_PORT: u16 = 47832;
pub const MAX_MESSAGE: usize = 2 * 1024 * 1024;

pub fn validate_hello(hello: &Hello) -> Result<(), &'static str> {
    if hello.version != VERSION {
        return Err("incompatible protocol version");
    }
    if hello.host_identity.len() != 32 || hello.nonce.len() != 32 {
        return Err("invalid host identity or nonce");
    }
    if hello.capabilities.len() > 32
        || hello
            .capabilities
            .iter()
            .any(|s| s.is_empty() || s.len() > 128 || !s.is_ascii())
    {
        return Err("invalid capabilities");
    }
    Ok(())
}
