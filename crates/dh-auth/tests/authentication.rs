use dh_auth::{Binding, ClientAttempt, HostCredentials};
fn binding() -> Binding {
    Binding {
        host: [1; 32],
        exporter: [2; 32],
        connection: [3; 32],
    }
}
fn exchange(
    host: &HostCredentials,
    password: &[u8],
    server_binding: Binding,
    client_binding: Binding,
) -> bool {
    let (client, request) = ClientAttempt::start(password).unwrap();
    let (server, response) = host.challenge(&request, server_binding).unwrap();
    let Ok(proof) = client.finish(&response, client_binding) else {
        return false;
    };
    server.finish(&proof).is_ok()
}

#[test]
fn real_password_exchange_survives_protected_record_roundtrip() {
    let host = HostCredentials::register(b"correct horse battery staple").unwrap();
    let bytes = host.serialize();
    assert!(!bytes.windows(7).any(|s| s == b"correct"));
    let restored = HostCredentials::deserialize(&bytes).unwrap();
    assert!(exchange(
        &restored,
        b"correct horse battery staple",
        binding(),
        binding()
    ));
    assert!(!exchange(
        &restored,
        b"incorrect password",
        binding(),
        binding()
    ));
}

#[test]
fn tls_exporter_host_and_connection_substitution_each_fail() {
    let host = HostCredentials::register(b"long permanent password").unwrap();
    for index in 0..3 {
        let mut other = binding();
        match index {
            0 => other.host[0] ^= 1,
            1 => other.exporter[0] ^= 1,
            _ => other.connection[0] ^= 1,
        }
        assert!(!exchange(
            &host,
            b"long permanent password",
            binding(),
            other
        ));
    }
}

#[test]
fn old_final_proof_cannot_complete_a_fresh_challenge() {
    let host = HostCredentials::register(b"long permanent password").unwrap();
    let (client, request) = ClientAttempt::start(b"long permanent password").unwrap();
    let (server, response) = host.challenge(&request, binding()).unwrap();
    let proof = client.finish(&response, binding()).unwrap();
    let authenticated = server.finish(&proof).unwrap();
    assert!(authenticated.binding() == &binding());
    let (_, request2) = ClientAttempt::start(b"long permanent password").unwrap();
    let (server2, _) = host.challenge(&request2, binding()).unwrap();
    assert!(server2.finish(&proof).is_err());
}

#[test]
fn malformed_records_messages_and_short_passwords_are_rejected() {
    assert!(HostCredentials::register(b"short").is_err());
    for bytes in [vec![], vec![0; 16], vec![255; 1024]] {
        assert!(HostCredentials::deserialize(&bytes).is_err());
    }
    let host = HostCredentials::register(b"long permanent password").unwrap();
    assert!(host.challenge(&[255; 4], binding()).is_err());
    let (client, request) = ClientAttempt::start(b"long permanent password").unwrap();
    let (server, _) = host.challenge(&request, binding()).unwrap();
    assert!(client.finish(&[0; 4], binding()).is_err());
    assert!(server.finish(&[0; 4]).is_err());
}
