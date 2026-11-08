use crate::{NetError, TIMEOUT, TransportKind};
use dh_protocol::{Envelope, MAX_MESSAGE};
use prost::Message;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
pub(crate) trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}
pub struct Channel {
    pub(crate) io: Box<dyn Io>,
    pub(crate) host: [u8; 32],
    pub(crate) exporter: [u8; 32],
    pub(crate) kind: TransportKind,
    pub(crate) authenticated: bool,
    pub(crate) _endpoint: Option<quinn::Endpoint>,
    pub(crate) _connection: Option<quinn::Connection>,
}
impl Channel {
    pub fn kind(&self) -> TransportKind {
        self.kind
    }
    pub fn host_identity(&self) -> [u8; 32] {
        self.host
    }
    pub async fn send(&mut self, message: Envelope) -> Result<(), NetError> {
        let limit = if self.authenticated {
            MAX_MESSAGE
        } else {
            8192
        };
        let size = message.encoded_len();
        if message.body.is_none() || size == 0 || size > limit {
            return Err(NetError::Protocol("message exceeds channel bounds"));
        }
        tokio::time::timeout(TIMEOUT, async {
            self.io.write_u32(size as u32).await?;
            self.io.write_all(&message.encode_to_vec()).await?;
            self.io.flush().await
        })
        .await
        .map_err(|_| NetError::Timeout)??;
        Ok(())
    }
    pub async fn receive(&mut self) -> Result<Envelope, NetError> {
        tokio::time::timeout(TIMEOUT, async {
            let size = self.io.read_u32().await? as usize;
            let limit = if self.authenticated {
                MAX_MESSAGE
            } else {
                8192
            };
            if size == 0 || size > limit {
                return Err(NetError::Protocol("message exceeds channel bounds"));
            }
            let mut data = Vec::new();
            data.try_reserve_exact(size)
                .map_err(|_| NetError::Protocol("message allocation failed"))?;
            data.resize(size, 0);
            self.io.read_exact(&mut data).await?;
            let envelope = Envelope::decode(data.as_slice())
                .map_err(|_| NetError::Protocol("invalid message"))?;
            if envelope.body.is_none() {
                return Err(NetError::Protocol("missing or unknown message body"));
            }
            Ok(envelope)
        })
        .await
        .map_err(|_| NetError::Timeout)?
    }
}
pub(crate) struct QuicIo {
    pub send: quinn::SendStream,
    pub recv: quinn::RecvStream,
}
impl AsyncRead for QuicIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}
impl AsyncWrite for QuicIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        AsyncWrite::poll_write(Pin::new(&mut self.send), cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        AsyncWrite::poll_flush(Pin::new(&mut self.send), cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        AsyncWrite::poll_shutdown(Pin::new(&mut self.send), cx)
    }
}
