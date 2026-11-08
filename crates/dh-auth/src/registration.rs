use crate::{AuthError, CREDENTIAL_ID, Suite, ksf};
use opaque_ke::{
    ClientRegistration, ClientRegistrationFinishParameters, ServerRegistration, ServerSetup,
    rand::rngs::OsRng,
};
use zeroize::Zeroizing;

pub struct HostCredentials {
    pub(crate) setup: ServerSetup<Suite>,
    pub(crate) record: ServerRegistration<Suite>,
}
impl HostCredentials {
    /// Local administrator password setup; never exposed as a remote registration endpoint.
    pub fn register(password: &[u8]) -> Result<Self, AuthError> {
        if !(12..=1024).contains(&password.len()) {
            return Err(AuthError);
        }
        let setup = ServerSetup::<Suite>::new(&mut OsRng);
        let client =
            ClientRegistration::<Suite>::start(&mut OsRng, password).map_err(|_| AuthError)?;
        let response = ServerRegistration::start(&setup, client.message, CREDENTIAL_ID)
            .map_err(|_| AuthError)?;
        let ksf = ksf();
        let finished = client
            .state
            .finish(
                &mut OsRng,
                password,
                response.message,
                ClientRegistrationFinishParameters {
                    ksf: Some(&ksf),
                    ..Default::default()
                },
            )
            .map_err(|_| AuthError)?;
        Ok(Self {
            setup,
            record: ServerRegistration::finish(finished.message),
        })
    }

    /// Sensitive OPAQUE material. Caller must encrypt/protect this before persistence.
    pub fn serialize(&self) -> Zeroizing<Vec<u8>> {
        let setup = self.setup.serialize();
        let record = self.record.serialize();
        let mut output = Zeroizing::new(Vec::with_capacity(12 + setup.len() + record.len()));
        output.extend_from_slice(b"DHA1");
        output.extend_from_slice(&(setup.len() as u32).to_be_bytes());
        output.extend_from_slice(&(record.len() as u32).to_be_bytes());
        output.extend_from_slice(&setup);
        output.extend_from_slice(&record);
        output
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self, AuthError> {
        if bytes.len() < 12 || bytes.len() > 4096 || &bytes[..4] != b"DHA1" {
            return Err(AuthError);
        }
        let setup_len = u32::from_be_bytes(bytes[4..8].try_into().map_err(|_| AuthError)?) as usize;
        let record_len =
            u32::from_be_bytes(bytes[8..12].try_into().map_err(|_| AuthError)?) as usize;
        if setup_len > 2048 || record_len > 2048 || 12 + setup_len + record_len != bytes.len() {
            return Err(AuthError);
        }
        Ok(Self {
            setup: ServerSetup::deserialize(&bytes[12..12 + setup_len]).map_err(|_| AuthError)?,
            record: ServerRegistration::deserialize(&bytes[12 + setup_len..])
                .map_err(|_| AuthError)?,
        })
    }
}
