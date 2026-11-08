# DeskHarbor

Independent LAN/VPN remote access with a Rust engine and Flutter UI.

**Under active development. Native platform qualification remains pending; this is not a certified remote-desktop release.**

Product implementation now proceeds alongside the native proofs, as authorized by the user's continuation instruction. This repository contains executable native probes and a fail-closed proof ledger. A successful ordinary-desktop capture is not a sign-in-screen proof. WSL and Android emulators do not certify the required physical hardware.

See [the approved design](docs/product-design.md), [implementation ledger](docs/implementation-ledger.md), and [platform-proof implementation plan](docs/superpowers/plans/2026-11-08-platform-proof.md).

Build/run steps: [installation](docs/installation.md). Actual gate: [0 of24 qualified](docs/proof-gate.json). Evidence requirements: [proof records](docs/proof-records.md). Outstanding product work: [acceptance status](docs/acceptance-tests.md).

The [final expectation audit](docs/final-audit.md) compares all six approved milestones with the actual implementation and records the checks, remaining work and corrected package evidence paths. **The audit is complete; the application is not complete.**

Current continuation: [capture composition progress](docs/capture-composition-ledger.md) adds `dh-media` reference composition, visible-only native publication, stricter driver-state policies and bounded packet reads. The earlier audit/P0 ZIPs describe their recorded revisions. This newer source still provides diagnostics, with no qualified platform or finished remote-access application.

Available implementations are a Windows native inspection/capture and isolated-service proof harness, a restricted Linux DRM/uinput diagnostic, an Android attended MediaProjection proof APK, an uncompiled macOS Swift harness, and the shared evidence CLI. These do not yet provide a remote-control connection.

The [application foundation](docs/application-ledger.md) additionally implements bounded Protocol Buffers, real OPAQUE/Argon2id authentication bound to TLS exporter/host/connection, QUIC and TLS1.3/TCP fallback, broker session/layout policy, SQLite saved identities, clipboard validation/echo suppression, and checksum-verified resumable transfer receivers. Executable app integration and the Flutter UI remain in progress. Transfer resume currently retains an active receiver across connection interruption; restart-persistent transfer journals are unfinished.

New DeskHarbor source is [MIT licensed](LICENSE). [Signing documentation](docs/signing.md) distinguishes freely verifiable build provenance and development signatures from OS publisher trust. GitHub builds do not replace the outstanding sign-in/hardware tests.

No host service is silently installed, no Windows session is signed out, and no remote port is opened by the default inspection command. Captures are local diagnostic artifacts, not screenshots sent to a server.
