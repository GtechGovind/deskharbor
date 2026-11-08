use opaque_ke::rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};

#[derive(Clone, PartialEq, Eq)]
pub struct Binding {
    pub host: [u8; 32],
    pub exporter: [u8; 32],
    pub connection: [u8; 32],
}
impl Binding {
    pub(crate) fn context(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"deskharbor/1 opaque channel binding\0");
        hash.update(self.host);
        hash.update(self.exporter);
        hash.update(self.connection);
        hash.finalize().into()
    }
}
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut value = [0; N];
    OsRng.fill_bytes(&mut value);
    value
}
