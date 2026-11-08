mod channel_binding;
mod login;
mod registration;
pub mod secret_store;
pub use channel_binding::{Binding, random_bytes};
pub use login::{AuthenticatedConnection, ClientAttempt, ServerAttempt};
pub use registration::HostCredentials;

pub(crate) struct Suite;
impl opaque_ke::CipherSuite for Suite {
    type OprfCs = opaque_ke::Ristretto255;
    type KeyExchange = opaque_ke::TripleDh<opaque_ke::Ristretto255, sha2::Sha512>;
    type Ksf = opaque_ke::argon2::Argon2<'static>;
}
pub(crate) const CREDENTIAL_ID: &[u8] = b"deskharbor-host-password-v1";
pub(crate) fn ksf() -> opaque_ke::argon2::Argon2<'static> {
    use opaque_ke::argon2::{Algorithm, Argon2, Params, Version};
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(19 * 1024, 2, 1, None).expect("fixed Argon2id parameters"),
    )
}

#[derive(Debug, PartialEq, Eq)]
pub struct AuthError;
impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("authentication failed")
    }
}
impl std::error::Error for AuthError {}
