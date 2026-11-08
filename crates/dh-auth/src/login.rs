use crate::{AuthError, Binding, CREDENTIAL_ID, HostCredentials, Suite, ksf};
use opaque_ke::{
    ClientLogin, ClientLoginFinishParameters, CredentialFinalization, CredentialRequest,
    CredentialResponse, ServerLogin, ServerLoginParameters, rand::rngs::OsRng,
};
use zeroize::Zeroizing;

pub struct ClientAttempt {
    state: ClientLogin<Suite>,
    password: Zeroizing<Vec<u8>>,
}
pub struct ServerAttempt {
    state: ServerLogin<Suite>,
    binding: Binding,
}
/// Created only after the authority has validated the final OPAQUE proof.
/// Deliberately not Clone, Deserialize or constructible by a network worker.
pub struct AuthenticatedConnection {
    binding: Binding,
}
impl AuthenticatedConnection {
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
}

impl ClientAttempt {
    pub fn start(password: &[u8]) -> Result<(Self, Vec<u8>), AuthError> {
        if password.is_empty() || password.len() > 1024 {
            return Err(AuthError);
        }
        let result = ClientLogin::<Suite>::start(&mut OsRng, password).map_err(|_| AuthError)?;
        Ok((
            Self {
                state: result.state,
                password: Zeroizing::new(password.to_vec()),
            },
            result.message.serialize().to_vec(),
        ))
    }
    pub fn finish(self, response: &[u8], binding: Binding) -> Result<Vec<u8>, AuthError> {
        if response.len() > 4096 {
            return Err(AuthError);
        }
        let context = binding.context();
        let ksf = ksf();
        let result = self
            .state
            .finish(
                &mut OsRng,
                &self.password,
                CredentialResponse::deserialize(response).map_err(|_| AuthError)?,
                ClientLoginFinishParameters {
                    context: Some(&context),
                    ksf: Some(&ksf),
                    ..Default::default()
                },
            )
            .map_err(|_| AuthError)?;
        Ok(result.message.serialize().to_vec())
    }
}
impl HostCredentials {
    pub fn challenge(
        &self,
        request: &[u8],
        binding: Binding,
    ) -> Result<(ServerAttempt, Vec<u8>), AuthError> {
        if request.len() > 4096 {
            return Err(AuthError);
        }
        let context = binding.context();
        let result = ServerLogin::start(
            &mut OsRng,
            &self.setup,
            Some(self.record.clone()),
            CredentialRequest::deserialize(request).map_err(|_| AuthError)?,
            CREDENTIAL_ID,
            ServerLoginParameters {
                context: Some(&context),
                ..Default::default()
            },
        )
        .map_err(|_| AuthError)?;
        Ok((
            ServerAttempt {
                state: result.state,
                binding,
            },
            result.message.serialize().to_vec(),
        ))
    }
}
impl ServerAttempt {
    pub fn finish(self, proof: &[u8]) -> Result<AuthenticatedConnection, AuthError> {
        if proof.len() > 4096 {
            return Err(AuthError);
        }
        let context = self.binding.context();
        self.state
            .finish(
                CredentialFinalization::deserialize(proof).map_err(|_| AuthError)?,
                ServerLoginParameters {
                    context: Some(&context),
                    ..Default::default()
                },
            )
            .map_err(|_| AuthError)?;
        Ok(AuthenticatedConnection {
            binding: self.binding,
        })
    }
}
