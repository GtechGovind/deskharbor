use dh_protocol::{
    Envelope, Hello, MAX_MESSAGE, VERSION, envelope::Body, read_message, write_message,
};
use std::io::{self, Read};

#[test]
fn incompatible_or_unbounded_hello_is_rejected_before_authentication() {
    let mut hello = Hello {
        version: 1,
        host_identity: vec![1; 32],
        nonce: vec![2; 32],
        capabilities: vec!["vp8".into()],
    };
    assert!(dh_protocol::validate_hello(&hello).is_ok());
    hello.version = 2;
    assert!(dh_protocol::validate_hello(&hello).is_err());
    hello.version = 1;
    hello.nonce.pop();
    assert!(dh_protocol::validate_hello(&hello).is_err());
    hello.nonce.push(2);
    hello.capabilities = vec!["x".repeat(129)];
    assert!(dh_protocol::validate_hello(&hello).is_err());
}

#[test]
fn fragmented_round_trip_preserves_request_and_typed_body() {
    let message = Envelope {
        request_id: 91,
        body: Some(Body::Hello(Hello {
            version: VERSION,
            host_identity: vec![7; 32],
            nonce: vec![3; 32],
            capabilities: vec!["vp8".into()],
        })),
    };
    let mut bytes = Vec::new();
    write_message(&mut bytes, &message).unwrap();
    struct Fragment<'a>(&'a [u8]);
    impl Read for Fragment<'_> {
        fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
            if self.0.is_empty() || dst.is_empty() {
                return Ok(0);
            }
            dst[0] = self.0[0];
            self.0 = &self.0[1..];
            Ok(1)
        }
    }
    let decoded = read_message(&mut Fragment(&bytes)).unwrap();
    assert_eq!(decoded.request_id, 91);
    let Some(Body::Hello(hello)) = decoded.body else {
        panic!("hello body lost")
    };
    assert_eq!(hello.capabilities, ["vp8"]);
    assert_eq!(hello.nonce, [3; 32]);
}

#[test]
fn oversized_prefix_is_rejected_without_reading_payload() {
    struct PrefixOnly {
        prefix: io::Cursor<[u8; 4]>,
        payload_reads: usize,
    }
    impl Read for PrefixOnly {
        fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
            if self.prefix.position() == 4 {
                self.payload_reads += 1;
                return Err(io::Error::other("payload was touched"));
            }
            self.prefix.read(dst)
        }
    }
    let mut reader = PrefixOnly {
        prefix: io::Cursor::new((MAX_MESSAGE as u32 + 1).to_be_bytes()),
        payload_reads: 0,
    };
    assert_eq!(
        read_message(&mut reader).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(reader.payload_reads, 0);
}

#[test]
fn empty_unknown_and_truncated_envelopes_fail_closed() {
    for bytes in [
        &[0, 0, 0, 0][..],
        &[0, 0, 0, 3, 8][..],
        &[0, 0, 0, 2, 8, 1][..],
        &[0, 0, 0, 2, 255, 255][..],
    ] {
        assert!(read_message(&mut &bytes[..]).is_err());
    }
    assert!(
        write_message(
            &mut Vec::new(),
            &Envelope {
                request_id: 0,
                body: None
            }
        )
        .is_err()
    );
}
