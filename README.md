# DeskHarbor

Independent LAN/VPN remote access with a Rust engine and Flutter UI.

**Current stage: Milestone 0 native platform proofs. The remote-desktop application is not yet implemented or certified.**

The approved specification requires proving desktop sign-in access before building the application UI or calling v1 usable. This repository contains executable native probes and a fail-closed proof ledger. A successful ordinary-desktop capture is not a sign-in-screen proof. WSL and Android emulators do not certify the required physical hardware.

See [the approved design](docs/product-design.md), [implementation ledger](docs/implementation-ledger.md), and [platform-proof implementation plan](docs/superpowers/plans/2026-11-08-platform-proof.md).

Build/run steps: [installation](docs/installation.md). Actual gate: [0 of24 qualified](docs/proof-gate.json). Evidence requirements: [proof records](docs/proof-records.md). Outstanding product work: [acceptance status](docs/acceptance-tests.md).

The [final expectation audit](docs/final-audit.md) compares all six approved milestones with the actual implementation and records the checks, remaining work and corrected package evidence paths. **The audit is complete; the application is not complete.**

Available implementations are a Windows native inspection/capture and isolated-service proof harness, a restricted Linux DRM/uinput diagnostic, an Android attended MediaProjection proof APK, an uncompiled macOS Swift harness, and the shared evidence CLI. These do not yet provide a remote-control connection.

No host service is silently installed, no Windows session is signed out, and no remote port is opened by the default inspection command. Captures are local diagnostic artifacts, not screenshots sent to a server.
