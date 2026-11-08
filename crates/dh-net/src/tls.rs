use crate::NetError;
use rustls::{
    ClientConfig, DigitallySignedStruct, ServerConfig, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use zeroize::Zeroizing;
pub struct Identity {
    certificate: Vec<u8>,
    key: Zeroizing<Vec<u8>>,
}
impl Identity {
    pub fn generate() -> Result<Self, NetError> {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec!["deskharbor.local".into()])
                .map_err(|_| NetError::Protocol("identity generation failed"))?;
        Self::from_parts(
            cert.der().to_vec(),
            Zeroizing::new(signing_key.serialize_der()),
        )
    }
    pub fn from_parts(certificate: Vec<u8>, key: Zeroizing<Vec<u8>>) -> Result<Self, NetError> {
        if certificate.len() > 16384 || key.len() > 16384 {
            return Err(NetError::Protocol("identity too large"));
        }
        let identity = Self { certificate, key };
        identity.server_config()?;
        Ok(identity)
    }
    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint(&self.certificate)
    }
    pub fn certificate(&self) -> &[u8] {
        &self.certificate
    }
    /// Sensitive private key; persist only through a protected SecretStore.
    pub fn private_key(&self) -> &[u8] {
        &self.key
    }
    pub(crate) fn server_config(&self) -> Result<ServerConfig, NetError> {
        let mut config =
            ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_protocol_versions(&[&rustls::version::TLS13])?
                .with_no_client_auth()
                .with_single_cert(
                    vec![CertificateDer::from(self.certificate.clone())],
                    PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key.to_vec())),
                )?;
        config.alpn_protocols = vec![dh_protocol::ALPN.to_vec()];
        config.max_early_data_size = 0;
        Ok(config)
    }
}
pub(crate) fn fingerprint(cert: &[u8]) -> [u8; 32] {
    Sha256::digest(cert).into()
}
pub(crate) fn client_config(pin: Option<[u8; 32]>) -> Result<ClientConfig, NetError> {
    let mut config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(PasswordAuthenticatedHost { pin }))
            .with_no_client_auth();
    config.alpn_protocols = vec![dh_protocol::ALPN.to_vec()];
    config.enable_early_data = false;
    Ok(config)
}
/// First contact is authenticated by OPAQUE bound to this TLS certificate and
/// exporter. Persist the fingerprint only after authentication. Always verify
/// TLS CertificateVerify signatures; this is not unauthenticated encryption.
#[derive(Debug)]
struct PasswordAuthenticatedHost {
    pin: Option<[u8; 32]>,
}
impl ServerCertVerifier for PasswordAuthenticatedHost {
    fn verify_server_cert(
        &self,
        end: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if self.pin.is_some_and(|pin| pin != fingerprint(end.as_ref())) {
            return Err(rustls::Error::General("host identity changed".into()));
        }
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::General("TLS1.2 disabled".into()))
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
