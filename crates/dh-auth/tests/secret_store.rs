#[cfg(windows)]
#[test]
fn user_dpapi_protects_roundtrip_and_rejects_tampered_ciphertext() {
    use dh_auth::secret_store::{SecretStore, UserSecretStore};
    let secret = b"private host identity and opaque record";
    let ciphertext = UserSecretStore.protect(secret).unwrap();
    assert!(!ciphertext.windows(secret.len()).any(|part| part == secret));
    assert_eq!(&**UserSecretStore.unprotect(&ciphertext).unwrap(), secret);
    let mut tampered = ciphertext;
    let n = tampered.len();
    tampered[n / 2] ^= 1;
    assert!(UserSecretStore.unprotect(&tampered).is_err());
}
