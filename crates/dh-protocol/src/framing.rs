use crate::{Envelope, MAX_MESSAGE};
use prost::Message;
use std::io::{self, Read, Write};

pub fn read_message(reader: &mut impl Read) -> io::Result<Envelope> {
    let mut prefix = [0; 4];
    reader.read_exact(&mut prefix)?;
    let size = u32::from_be_bytes(prefix) as usize;
    if size == 0 || size > MAX_MESSAGE {
        return Err(invalid("message size out of bounds"));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| io::Error::other("message allocation failed"))?;
    bytes.resize(size, 0);
    reader.read_exact(&mut bytes)?;
    let message =
        Envelope::decode(bytes.as_slice()).map_err(|_| invalid("invalid protocol message"))?;
    if message.body.is_none() {
        return Err(invalid("missing or unknown message body"));
    }
    Ok(message)
}

pub fn write_message(writer: &mut impl Write, message: &Envelope) -> io::Result<()> {
    let size = message.encoded_len();
    if message.body.is_none() || size == 0 || size > MAX_MESSAGE {
        return Err(invalid("message size or body out of bounds"));
    }
    let bytes = message.encode_to_vec();
    writer.write_all(&(size as u32).to_be_bytes())?;
    writer.write_all(&bytes)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
