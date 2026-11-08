use crate::{Channel, NetError};
use dh_protocol::{Attach, Envelope, envelope::Body};
use ring::hmac;
impl Channel {
    fn attachment_context(&self, role: &str) -> Vec<u8> {
        [
            b"DeskHarbor attached channel v1\0".as_slice(),
            &self.host,
            &self.exporter,
            role.as_bytes(),
        ]
        .concat()
    }
    pub async fn attach_client(&mut self, role: &str, secret: &[u8; 32]) -> Result<(), NetError> {
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret);
        let tag = hmac::sign(&key, &self.attachment_context(role));
        self.send(Envelope {
            request_id: 0,
            body: Some(Body::Attach(Attach {
                role: role.into(),
                proof: tag.as_ref().into(),
            })),
        })
        .await?;
        if !matches!(self.receive().await?.body,Some(Body::Command(c)) if c.name=="attached") {
            return Err(NetError::Authentication);
        }
        self.authenticated = true;
        Ok(())
    }
    /// The broker consumes the designated single-use secret after this verification,
    /// before acknowledging or performing any attached work.
    pub fn verify_attachment(
        &mut self,
        role: &str,
        secret: &[u8; 32],
        proof: &[u8],
    ) -> Result<(), NetError> {
        hmac::verify(
            &hmac::Key::new(hmac::HMAC_SHA256, secret),
            &self.attachment_context(role),
            proof,
        )
        .map_err(|_| NetError::Authentication)?;
        self.authenticated = true;
        Ok(())
    }
}
