use dh_auth::{AuthenticatedConnection, Binding, ClientAttempt, HostCredentials};
use dh_core::{Broker, HostState, Operation::*, PolicyError};
use std::sync::LazyLock;
static HOST: LazyLock<HostCredentials> =
    LazyLock::new(|| HostCredentials::register(b"long permanent password").unwrap());
fn binding(n: u8) -> Binding {
    Binding {
        host: [1; 32],
        exporter: [2; 32],
        connection: [n; 32],
    }
}
fn authenticate(n: u8) -> AuthenticatedConnection {
    let (client, request) = ClientAttempt::start(b"long permanent password").unwrap();
    let (server, response) = HOST.challenge(&request, binding(n)).unwrap();
    server
        .finish(&client.finish(&response, binding(n)).unwrap())
        .unwrap()
}
fn ready() -> Broker {
    Broker::new(HostState::Ready {
        user: "user-a".into(),
    })
}

#[test]
fn grant_is_connection_bound_and_only_one_controller_is_accepted() {
    let mut broker = ready();
    let grant = broker
        .authorize(authenticate(3), vec![View, Input, Clipboard, Files])
        .unwrap();
    assert_eq!(broker.check(&grant, &binding(3), View, 1, 1), Ok(()));
    assert_eq!(
        broker.check(&grant, &binding(4), Input, 1, 1),
        Err(PolicyError::Unauthorized)
    );
    assert!(matches!(
        broker.authorize(authenticate(4), vec![View]),
        Err(PolicyError::Busy)
    ));
    broker.disconnect(&binding(3));
    assert_eq!(
        broker.check(&grant, &binding(3), View, 1, 1),
        Err(PolicyError::Unauthorized)
    );
    assert!(broker.authorize(authenticate(4), vec![View]).is_ok());
}

#[test]
fn transition_releases_keys_and_mouse_and_revokes_old_epoch() {
    let mut broker = ready();
    let grant = broker
        .authorize(authenticate(3), vec![View, Input, Clipboard, Files])
        .unwrap();
    broker.key(&grant, &binding(3), 1, 1, 0xe1, true).unwrap();
    broker
        .pointer(&grant, &binding(3), 1, 1, 0.5, 0.25, 1)
        .unwrap();
    let release = broker.transition(HostState::Locked);
    assert_eq!(release.keys, [0xe1]);
    assert_eq!(release.buttons, 1);
    assert_eq!(broker.epoch(), 2);
    assert_eq!(
        broker.check(&grant, &binding(3), Input, 1, 1),
        Err(PolicyError::StaleEpoch)
    );
    assert_eq!(
        broker.check(&grant, &binding(3), Files, 2, 1),
        Err(PolicyError::StaleEpoch)
    );
}

#[test]
fn signed_out_or_locked_sessions_allow_only_granted_view_and_input() {
    for state in [HostState::Locked, HostState::SignIn] {
        let mut broker = Broker::new(state);
        let grant = broker
            .authorize(authenticate(3), vec![View, Input, Clipboard, Files])
            .unwrap();
        assert_eq!(broker.check(&grant, &binding(3), View, 1, 1), Ok(()));
        assert_eq!(broker.check(&grant, &binding(3), Input, 1, 1), Ok(()));
        assert_eq!(
            broker.check(&grant, &binding(3), Clipboard, 1, 1),
            Err(PolicyError::PermissionDenied)
        );
        assert_eq!(
            broker.check(&grant, &binding(3), Files, 1, 1),
            Err(PolicyError::PermissionDenied)
        );
    }
}

#[test]
fn layout_changes_release_input_and_reject_stale_or_nonfinite_coordinates() {
    let mut broker = ready();
    let grant = broker.authorize(authenticate(3), vec![Input]).unwrap();
    assert_eq!(
        broker.pointer(&grant, &binding(3), 1, 1, f64::NAN, 0.0, 0),
        Err(PolicyError::InvalidInput)
    );
    assert_eq!(
        broker.pointer(&grant, &binding(3), 1, 1, 1.1, 0.0, 0),
        Err(PolicyError::InvalidInput)
    );
    broker.key(&grant, &binding(3), 1, 1, 4, true).unwrap();
    assert_eq!(broker.replace_layout().keys, [4]);
    assert_eq!(broker.layout(), 2);
    assert_eq!(
        broker.key(&grant, &binding(3), 1, 1, 4, true),
        Err(PolicyError::StaleLayout)
    );
    assert_eq!(
        broker.key(&grant, &binding(3), 1, 2, 4, true),
        Err(PolicyError::StaleLayout)
    );
}
