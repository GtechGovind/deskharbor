use crate::{
    Channel, EXPORTER_CONTEXT, EXPORTER_LABEL, Identity, NetError, TIMEOUT, TransportKind,
    channels::QuicIo, tls,
};
use rustls::pki_types::{CertificateDer, ServerName};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
};
pub struct Listener {
    tcp: TcpListener,
    quic: Option<quinn::Endpoint>,
    tls: Arc<rustls::ServerConfig>,
    host: [u8; 32],
}
impl Listener {
    pub async fn bind(
        address: SocketAddr,
        identity: Identity,
        quic: bool,
    ) -> Result<Self, NetError> {
        let tcp = TcpListener::bind(address).await?;
        let address = tcp.local_addr()?;
        let tls = Arc::new(identity.server_config()?);
        let endpoint = if quic {
            let crypto = quinn::crypto::rustls::QuicServerConfig::try_from((*tls).clone())
                .map_err(|_| NetError::Protocol("QUIC TLS config"))?;
            let mut config = quinn::ServerConfig::with_crypto(Arc::new(crypto));
            let mut transport = quinn::TransportConfig::default();
            transport.max_concurrent_bidi_streams(4_u32.into());
            transport.max_concurrent_uni_streams(4_u32.into());
            transport.max_idle_timeout(Some(
                Duration::from_secs(30)
                    .try_into()
                    .map_err(|_| NetError::Protocol("timeout config"))?,
            ));
            config.transport_config(Arc::new(transport));
            Some(quinn::Endpoint::server(config, address)?)
        } else {
            None
        };
        Ok(Self {
            tcp,
            quic: endpoint,
            tls,
            host: identity.fingerprint(),
        })
    }
    pub fn local_addr(&self) -> SocketAddr {
        self.tcp.local_addr().expect("bound listener")
    }
    pub async fn accept(&self) -> Result<Channel, NetError> {
        if let Some(endpoint) = &self.quic {
            tokio::select! {
                incoming=self.tcp.accept()=>{let(stream,_)=incoming?;self.accept_tcp(stream).await},
                incoming=endpoint.accept()=>{
                    let incoming=incoming.ok_or(NetError::Protocol("QUIC listener closed"))?;
                    let connection=tokio::time::timeout(TIMEOUT,incoming).await.map_err(|_|NetError::Timeout)?.map_err(|_|NetError::Protocol("QUIC handshake failed"))?;
                    let mut exporter=[0;32];connection.export_keying_material(&mut exporter,EXPORTER_LABEL,EXPORTER_CONTEXT).map_err(|_|NetError::Protocol("QUIC exporter failed"))?;
                    let(send,mut recv)=tokio::time::timeout(TIMEOUT,connection.accept_bi()).await.map_err(|_|NetError::Timeout)?.map_err(|_|NetError::Protocol("QUIC stream failed"))?;
                    let mut marker=[0;3];
                    tokio::time::timeout(TIMEOUT,recv.read_exact(&mut marker)).await.map_err(|_|NetError::Timeout)?.map_err(|_|NetError::Protocol("QUIC opening marker"))?;
                    if &marker!=b"DH1"{return Err(NetError::Protocol("QUIC opening marker"));}
                    Ok(Channel{io:Box::new(QuicIo{send,recv}),host:self.host,exporter,kind:TransportKind::Quic,authenticated:false,_endpoint:Some(endpoint.clone()),_connection:Some(connection)})
                }
            }
        } else {
            let (stream, _) = self.tcp.accept().await?;
            self.accept_tcp(stream).await
        }
    }
    async fn accept_tcp(&self, stream: TcpStream) -> Result<Channel, NetError> {
        stream.set_nodelay(true)?;
        let stream = tokio::time::timeout(
            TIMEOUT,
            tokio_rustls::TlsAcceptor::from(self.tls.clone()).accept(stream),
        )
        .await
        .map_err(|_| NetError::Timeout)??;
        let exporter = stream.get_ref().1.export_keying_material(
            [0; 32],
            EXPORTER_LABEL,
            Some(EXPORTER_CONTEXT),
        )?;
        Ok(Channel {
            io: Box::new(stream),
            host: self.host,
            exporter,
            kind: TransportKind::Tcp,
            authenticated: false,
            _endpoint: None,
            _connection: None,
        })
    }
}
pub async fn connect(
    address: SocketAddr,
    pin: Option<[u8; 32]>,
    prefer_quic: bool,
) -> Result<Channel, NetError> {
    if prefer_quic && let Ok(channel) = connect_quic(address, pin).await {
        return Ok(channel);
    }
    let tcp = tokio::time::timeout(TIMEOUT, TcpStream::connect(address))
        .await
        .map_err(|_| NetError::Timeout)??;
    tcp.set_nodelay(true)?;
    let connector = tokio_rustls::TlsConnector::from(Arc::new(tls::client_config(pin)?));
    let stream = tokio::time::timeout(
        TIMEOUT,
        connector.connect(
            ServerName::try_from("deskharbor.local").expect("static server name"),
            tcp,
        ),
    )
    .await
    .map_err(|_| NetError::Timeout)??;
    let cert = stream
        .get_ref()
        .1
        .peer_certificates()
        .and_then(|c| c.first())
        .ok_or(NetError::Protocol("no host identity"))?;
    let host = tls::fingerprint(cert.as_ref());
    let exporter = stream.get_ref().1.export_keying_material(
        [0; 32],
        EXPORTER_LABEL,
        Some(EXPORTER_CONTEXT),
    )?;
    Ok(Channel {
        io: Box::new(stream),
        host,
        exporter,
        kind: TransportKind::Tcp,
        authenticated: false,
        _endpoint: None,
        _connection: None,
    })
}
async fn connect_quic(address: SocketAddr, pin: Option<[u8; 32]>) -> Result<Channel, NetError> {
    let bind = if address.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let mut endpoint = quinn::Endpoint::client(bind.parse().expect("static address"))?;
    let crypto = quinn::crypto::rustls::QuicClientConfig::try_from(tls::client_config(pin)?)
        .map_err(|_| NetError::Protocol("QUIC TLS config"))?;
    endpoint.set_default_client_config(quinn::ClientConfig::new(Arc::new(crypto)));
    let connecting = endpoint
        .connect(address, "deskharbor.local")
        .map_err(|_| NetError::Protocol("QUIC connect failed"))?;
    let connection = tokio::time::timeout(Duration::from_secs(2), connecting)
        .await
        .map_err(|_| NetError::Timeout)?
        .map_err(|_| NetError::Protocol("QUIC handshake failed"))?;
    let identity = connection
        .peer_identity()
        .ok_or(NetError::Protocol("no QUIC host identity"))?;
    let certs = identity
        .downcast::<Vec<CertificateDer<'static>>>()
        .map_err(|_| NetError::Protocol("unexpected QUIC identity"))?;
    let host = tls::fingerprint(
        certs
            .first()
            .ok_or(NetError::Protocol("no QUIC certificate"))?
            .as_ref(),
    );
    let mut exporter = [0; 32];
    connection
        .export_keying_material(&mut exporter, EXPORTER_LABEL, EXPORTER_CONTEXT)
        .map_err(|_| NetError::Protocol("QUIC exporter failed"))?;
    let (mut send, recv) = connection
        .open_bi()
        .await
        .map_err(|_| NetError::Protocol("QUIC stream failed"))?;
    // A stream becomes visible to the server only once data is written; this
    // fixed opening marker precedes framed protocol messages on QUIC only.
    send.write_all(b"DH1")
        .await
        .map_err(|_| NetError::Protocol("QUIC opening marker"))?;
    send.flush().await?;
    Ok(Channel {
        io: Box::new(QuicIo { send, recv }),
        host,
        exporter,
        kind: TransportKind::Quic,
        authenticated: false,
        _endpoint: Some(endpoint),
        _connection: Some(connection),
    })
}
