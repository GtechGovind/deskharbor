# Build signatures and platform trust

Development artifacts are unqualified builds of the exact source revision. A
successful build is not a sign-in, permission, hardware or performance proof.

The freely available verification path is GitHub artifact attestation using
Sigstore and the workflow's short-lived OpenID Connect identity. No long-lived
private signing key is checked into source or needed on a developer's PC.
Verify downloaded files with:

```sh
gh attestation verify ARTIFACT --repo GtechGovind/deskharbor
```

Checksums detect accidental corruption; attestation additionally binds those
bytes to the repository, source revision and build workflow. Review the verified
workflow/revision. Neither mechanism makes an unqualified application certified.

macOS builds can run on GitHub's Intel and Apple Silicon macOS runners. They can
use an ad-hoc code signature (`codesign --sign -`) without an Apple account.
That signature is not a verified publisher identity and does not supply Apple
notarization. Developer ID Application/Installer certificates and notarization
remain pending. Installed LoginWindow/Aqua permission proofs remain pending even
when a hosted build succeeds. No workflow disables Gatekeeper or installs a
self-signed root in users' trust stores.

Windows development binaries can have detached build attestations without an
Authenticode certificate. They must not be advertised as trusted-publisher signed.
Android APK signing can use an independently generated free persistent key; its
private keystore and passwords belong outside this repository, in protected local
storage or GitHub Actions secrets. A debug key is for test APKs only. Linux
archives use the same independently verifiable build provenance/checksums.

Production MSI/PKG/DEB/APK publication additionally requires the approved product
and native qualification gates. Build provenance does not waive them.

References: [GitHub attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations),
[Apple Developer ID](https://developer.apple.com/developer-id/),
[Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).
