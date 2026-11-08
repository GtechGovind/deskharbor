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

The `Build attested development applications` workflow builds archives on Windows,
Ubuntu, Intel macOS and Apple Silicon macOS runners, and a persistently signed
ARM64 Android APK on Ubuntu. Source and Flutter checks gate packaging. Artifacts
are retained on the workflow run rather than published as a qualified release.
Each archive includes its source revision, development qualification status,
dependency lockfiles and license information. Successful workflow execution and
attestation verification must be recorded before describing any artifact as built
or attested. Portable archives do not install desktop services.

## Android package identity

A persistent RSA4096/SHA256 Android signing key has been generated outside the
repository. The private keystore is protected by a random password and an
owner-restricted Windows directory; its local credentials are encrypted with
current-user DPAPI. The workflow receives the keystore/password only through
GitHub Actions secrets. Neither private material nor passwords belong in source.

The public X.509 certificate's SHA256 fingerprint is:

```text
d885be68117d4cc6eb64376cbf12b65bda8233b7ccb783fb15eecdd59634e9fd
```

An Android release build must use this persistent key, then verify the APK with
`apksigner verify --verbose --print-certs` and compare its signer fingerprint.
Debug APKs use a separate debug key. GitHub provenance is verified independently.
This Android certificate does not provide Apple or Windows publisher trust.

References: [GitHub attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations),
[Apple Developer ID](https://developer.apple.com/developer-id/),
[Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).
