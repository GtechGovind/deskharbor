# Hosted development builds

Source: [GtechGovind/deskharbor](https://github.com/GtechGovind/deskharbor).
The `Build attested development applications` workflow can compile the product
on hosted runners while local VM/build prerequisites are being prepared.

| Target | Runner | Output and signature |
|---|---|---|
| Windows x64 | Windows 2025 | Portable ZIP; GitHub build provenance |
| Ubuntu x64 | Ubuntu 24.04 | Portable tar.gz; GitHub build provenance |
| macOS ARM64 | macOS 15 ARM64 | App in tar.gz; ad-hoc code signature and provenance |
| macOS x64 | macOS 15 Intel | App in tar.gz; ad-hoc code signature and provenance |
| Android ARM64 | Ubuntu with NDK28.2.13676358 | Persistent-key signed APK, license ZIP and provenance |

The workflow first runs Rust/Flutter checks and packaging-boundary tests, then
builds pinned libvpx and the Rust engine before the Flutter application. A bridge
smoke check uses a new temporary data directory to exercise the actual generated
Rust API without starting hosting, capturing the screen or injecting input.

Download artifacts from a **successful** workflow run. Verify each archive/APK:

```sh
gh attestation verify DOWNLOADED_FILE --repo GtechGovind/deskharbor
```

Compare the attested source commit with the revision you intended to test. Check
the accompanying SHA256 file as well. Android APK verification additionally
requires the public signer fingerprint in `packaging/android/signing-certificate.sha256`.
Private signing material is stored in repository Actions secrets, never in an
artifact. The temporary runner keystore is removed after the APK build.

These are explicitly unqualified development packages. An archive does not
install or activate a privileged service. A Mac build made by GitHub does not
prove LoginWindow/TCC behavior; its ad-hoc signature does not provide Developer
ID or Apple notarization. Windows provenance does not provide Authenticode
publisher recognition. See [signing.md](signing.md) and
[support-matrix.md](support-matrix.md) for the remaining qualification gates.

Production MSI/PKG/DEB installers and release publication remain separate work.
Do not turn a successful controller/UI build into a claim of unattended desktop
support. Native sign-in, Android consent lifecycle, device interoperability,
performance and long-running session tests require their recorded evidence.
