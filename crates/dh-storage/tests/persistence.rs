use dh_storage::{Device, StorageError, Store};
#[test]
fn devices_survive_reopening_and_identity_replacement_requires_forgetting() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("devices.sqlite");
    let store = Store::open(&path).unwrap();
    store
        .save_device(Device {
            name: "Work PC".into(),
            address: "192.168.1.20:47832".into(),
            identity: Some([3; 32]),
        })
        .unwrap();
    drop(store);
    let store = Store::open(&path).unwrap();
    assert_eq!(
        store.devices().unwrap(),
        vec![Device {
            name: "Work PC".into(),
            address: "192.168.1.20:47832".into(),
            identity: Some([3; 32])
        }]
    );
    assert!(matches!(
        store.save_device(Device {
            name: "Work PC".into(),
            address: "192.168.1.20:47832".into(),
            identity: Some([4; 32])
        }),
        Err(StorageError::IdentityChanged)
    ));
    assert!(matches!(
        store.save_device(Device {
            name: "Work PC".into(),
            address: "192.168.1.20:47832".into(),
            identity: None
        }),
        Err(StorageError::IdentityChanged)
    ));
    store.forget_device("192.168.1.20:47832").unwrap();
    store
        .save_device(Device {
            name: "Verified again".into(),
            address: "192.168.1.20:47832".into(),
            identity: Some([4; 32]),
        })
        .unwrap();
}
#[test]
fn future_schema_is_not_modified_and_sql_names_are_bound_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("future.sqlite");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);
    assert!(matches!(Store::open(&path), Err(StorageError::NewerSchema)));
    assert_eq!(
        rusqlite::Connection::open(&path)
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        99
    );
    let store = Store::open(dir.path().join("normal.sqlite")).unwrap();
    store
        .save_device(Device {
            name: "Robert'); DROP TABLE devices;--".into(),
            address: "workstation.local:47832".into(),
            identity: None,
        })
        .unwrap();
    assert_eq!(store.devices().unwrap().len(), 1);
}
