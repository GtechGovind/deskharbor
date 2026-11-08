use crate::{Channel, NetError};
use dh_auth::{AuthenticatedConnection, Binding, ClientAttempt, HostCredentials, random_bytes};
use dh_protocol::{
    AuthAccepted, AuthChallenge, AuthFinish, AuthStart, Envelope, Hello, envelope::Body,
};
use std::sync::Arc;
impl Channel {
    pub async fn authenticate_server(
        &mut self,
        credentials: Arc<HostCredentials>,
    ) -> Result<AuthenticatedConnection, NetError> {
        if self.authenticated {
            return Err(NetError::Protocol("already authenticated"));
        }
        let binding = Binding {
            host: self.host,
            exporter: self.exporter,
            connection: random_bytes(),
        };
        self.send(Envelope {
            request_id: 0,
            body: Some(Body::Hello(Hello {
                version: dh_protocol::VERSION,
                host_identity: self.host.to_vec(),
                nonce: binding.connection.to_vec(),
                capabilities: vec!["control".into()],
            })),
        })
        .await?;
        let Some(Body::AuthStart(start)) = self.receive().await?.body else {
            return Err(NetError::Authentication);
        };
        let bound = binding.clone();
        let (attempt, response) =
            tokio::task::spawn_blocking(move || credentials.challenge(&start.request, bound))
                .await
                .map_err(|_| NetError::Authentication)?
                .map_err(|_| NetError::Authentication)?;
        self.send(Envelope {
            request_id: 0,
            body: Some(Body::AuthChallenge(AuthChallenge { response })),
        })
        .await?;
        let Some(Body::AuthFinish(finish)) = self.receive().await?.body else {
            return Err(NetError::Authentication);
        };
        let proof = attempt
            .finish(&finish.proof)
            .map_err(|_| NetError::Authentication)?;
        self.send(Envelope {
            request_id: 0,
            body: Some(Body::AuthAccepted(AuthAccepted {})),
        })
        .await?;
        self.authenticated = true;
        Ok(proof)
    }
    pub async fn authenticate_client(&mut self, password: &[u8]) -> Result<Binding, NetError> {
        if self.authenticated {
            return Err(NetError::Protocol("already authenticated"));
        }
        let Some(Body::Hello(hello)) = self.receive().await?.body else {
            return Err(NetError::Authentication);
        };
        dh_protocol::validate_hello(&hello).map_err(NetError::Protocol)?;
        if hello.host_identity.as_slice() != self.host {
            return Err(NetError::IdentityChanged);
        }
        let binding = Binding {
            host: self.host,
            exporter: self.exporter,
            connection: hello
                .nonce
                .try_into()
                .map_err(|_| NetError::Authentication)?,
        };
        let (attempt, request) =
            ClientAttempt::start(password).map_err(|_| NetError::Authentication)?;
        self.send(Envelope {
            request_id: 0,
            body: Some(Body::AuthStart(AuthStart { request })),
        })
        .await?;
        let Some(Body::AuthChallenge(challenge)) = self.receive().await?.body else {
            return Err(NetError::Authentication);
        };
        let bound = binding.clone();
        let proof = tokio::task::spawn_blocking(move || attempt.finish(&challenge.response, bound))
            .await
            .map_err(|_| NetError::Authentication)?
            .map_err(|_| NetError::Authentication)?;
        self.send(Envelope {
            request_id: 0,
            body: Some(Body::AuthFinish(AuthFinish { proof })),
        })
        .await?;
        if !matches!(self.receive().await?.body, Some(Body::AuthAccepted(_))) {
            return Err(NetError::Authentication);
        }
        self.authenticated = true;
        Ok(binding)
    }
}
