# Acceptance status

The full product's acceptance criteria remain those in the approved plan. This repository currently implements only the first platform investigation stage. A checked-in source file, unit test or successful emulator run does not qualify unattended hosting.

## Tests that can run without physical sign-in access

```powershell
./scripts/cargo-msvc.ps1 test --locked --workspace
./scripts/cargo-msvc.ps1 @('clippy','--locked','--workspace','--all-targets','--','-D','warnings')
./scripts/cargo-msvc.ps1 @('fmt','--all','--','--check')
python -m unittest discover -s tests/lab -v
```

Linux native C tests and the WSL denied-device fixture are documented in [linux-proof.md](linux-proof.md). Android JVM tests run during the [Android build](android-proof.md). The macOS Swift source needs its first native build, then installed-identity testing.

## What is not a passed platform proof

- Windows: ordinary user-session enumeration/capture and refusal of privileged commands. Installed helpers, actual input delivery, boot/sign-in, UAC/SAS and login cycles remain separate cases.
- Linux: unit tests, a sandbox denying prohibited syscalls, and WSL reporting no display. Physical planes, GPU modifiers/composition, installed authorization and GDM/session transitions remain separate cases.
- Android: reducer tests or a debug APK build. OS consent, cleanup and optional input are runtime cases; an emulator result must retain its API/ABI label and cannot satisfy the physical ARM64 profile.
- macOS: uncompiled public API source and launchd templates. Both architecture profiles require real signed installed identities, permissions and LoginWindow/Aqua tests.

## Remaining product milestones

After genuine P0 acceptance: OPAQUE/TLS exporter binding and broker grants; QUIC/TCP channels; peer-authenticated IPC; persistent installed host supervision and session epochs; production capture/input/VP8/H264/textures; Flutter desktop+Android application and FRB; clipboard/file transfer/storage; MSI/PKG/DEB/APK; upgrade/uninstall;50login cycles;8hour soak; offlineLAN/VPN; latency/1080p30 measurements. None is currently reported implemented or passed.
