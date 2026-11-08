# DeskHarbor application foundation implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Build the locally verifiable application foundation and cross-platform development build pipeline while native certification proceeds independently.

**Architecture:** Protocol messages have bounded length-prefixed Prost serialization. The broker's session state and OPAQUE proof own authorization; transport workers cannot fabricate grants. User-session data services and the Flutter UI consume the same engine contracts. Synthetic media is explicitly identified until a qualified native source replaces it.

**Tech stack:** Rust 1.99.1, Prost, Tokio, OPAQUE 4.0.1/Argon2id, Quinn 0.11.12, rustls 0.23.46, SQLite, BLAKE3, Flutter 3.47.9, FRB 2.13.0, GitHub Actions.

**Spec:** `docs/product-design.md`, the user's approved full plan, and the continuation instruction to proceed during Hyper-V setup.

## Global constraints

- Direct LAN/VPN, no remote account, relay or coordination service; default port47832.
- Authentication binds TLS exporter, host identity and protocol; passwords never enter persisted device records or logs.
- One incoming controller; epoch/layout changes invalidate stale input; locked/signed-out sessions disable files/clipboard.
- Failed/absent platform capabilities produce explicit errors. No mocked production success.
- Native hardware proofs remain0/24 until real evidence is recorded. Development artifacts are explicitly unqualified.
- GitHub macOS builds and ad-hoc code signatures do not establish Developer ID trust, notarization or LoginWindow permission behavior.

## Review focus

1. Malicious wire lengths must fail before allocating the advertised payload (Task1).
2. Replayed grants, old session/layout generations and different connections must never authorize operations (Task1/2).
3. Password, exporter or host substitution must fail the real cryptographic exchange (Task2/3).
4. Duplicate clipboard updates, partial files and replaced filesystem objects must not cause loops, corrupt commits or escape grants (Task4).
5. Build signatures must identify exact artifacts/revisions without implying native certification or Apple publisher trust (Task6).

## Task 1: Protocol and broker state

Files: `proto/deskharbor.proto`, `crates/dh-protocol/{Cargo.toml,build.rs,src/{lib.rs,framing.rs}}`, `crates/dh-core/{Cargo.toml,src/{lib.rs,session.rs}}`, their integration tests, root workspace.

Interfaces: Prost `Envelope` with typed hello/auth/control/input/clipboard/transfer/media messages; bounded `read_message<R:Read>`/`write_message<W:Write>`; `Broker::authorize(connection, proof, permissions)`, `transition`, `check`, `disconnect`. Authentication proof type will be provided by Task2; no external success boolean.

- [ ] Write executable framing/version/unknown message tests and broker stale epoch/layout, busy, locked policy and input-release tests; run against missing/stub behavior and observe failures.
- [ ] Implement bounded framing and broker lifecycle; run targeted tests and workspace suite, expected0failures.
- [ ] Commit protocol and lifecycle implementation.

## Task 2: OPAQUE authority and protected identity

Files: `crates/dh-auth/{Cargo.toml,src/{lib.rs,login.rs,registration.rs,channel_binding.rs,secret_store.rs}}`, tests; adapt broker to unforgeable proof.

Interfaces: `HostCredentials::register(password)`, `ClientAttempt::start(password)`, server challenge, consumed client/server finish state, `AuthenticatedConnection` with private constructor. Context encodes protocol ID, host identity, connection nonce and TLS exporter; Argon2id parameters fixed on registration/login. Secrets zeroized and excluded from Debug.

- [ ] Write real correct/wrong password, exporter/host substitution, replay, malformed record and persistence round-trip tests; observe failed expectations before implementation.
- [ ] Implement OPAQUE with official crate APIs and protected storage adapters. No production plaintext credentials fallback.
- [ ] Run tests and strict Clippy; commit authentication.

## Task 3: Direct transport and executable connection

Files: `crates/dh-net`, `bins/dh-hostd`, `bins/dh-netd`, `bins/dh-cli`, protocol extension as needed.

Interfaces: transport listener/client with TLS1.3 and QUIC, exporter-bound authentication, pinned identities, timeouts/limits; broker-owned auth; host status and synthetic source diagnostic through CLI. Restricted worker/broker IPC must validate OS peer identity before privileged use; until that is proven run only unprivileged development hosting with explicit status.

- [ ] Write loopback encrypted exchange tests rejecting wrong password, changed identity, unauthenticated commands, replay and UDP-blocked TCP fallback; observe missing behavior.
- [ ] Implement and run real loopback and separate-process localhost tests, without installing services or opening firewall rules.
- [ ] Commit transport and executable foundation.

## Task 4: Persistent devices, clipboard and safe transfers

Files: `crates/dh-storage`, `crates/dh-clipboard`, `crates/dh-transfer`; SQLite migrations and integration tests.

Interfaces: saved device address/name/verified identity only; origin/generation-bounded clipboard updates; 1MiB chunk BLAKE3 transfer with resume/cancel/explicit collision confirmation, destination capability and same-user epoch.

- [ ] Test persistence upgrades/identity changes, clipboard echo/size limits, empty/interrupted/corrupt/collision/traversal/symlink files and lock/user changes; observe missing behavior.
- [ ] Implement actual SQLite and filesystem operations with fail-closed destination authority; run complete suite.
- [ ] Commit user-data services.

## Task 5: Flutter application and bridge

Files: `apps/client`, `packages/engine_bridge`, `packages/native_surfaces`, `crates/dh-ffi` following approved file tree.

Interfaces: actual engine connect/disconnect/events/host/device/transfer APIs, native texture boundary for frames. Material3 sidebar/bottom nav, all approved pages and meaningful permission/unavailable states. Do not present fixture status as a native session.

- [ ] Install verified pinned SDK under workspace scratch, generate platform runner scaffolding, and commit lockfiles.
- [ ] Test navigation, host policy and focused session controls with engine errors propagated; implement real controllers/bridge and screens.
- [ ] Analyze/test/build locally possible targets and preserve runnable artifacts.

## Task 6: Hosted build and artifact verification

Files: `.github/workflows/{ci.yml,development-artifacts.yml,release-artifacts.yml}`, `scripts/package-development.*`, `docs/{building,signing}.md`, license/notices.

Interfaces: Windows/Linux/macOS Intel+ARM/Android build artifacts named by exact source revision, checksums, keyless Sigstore/GitHub provenance; macOS ad-hoc signature and explicit unnotarized status; persistent Android signing key stored outside repo/GitHub secret. Release workflow refuses production release with failed qualification gate.

- [ ] Execute packaging scripts on fixture outputs; verify archive contents/checksum failure and qualification denial before implementation.
- [ ] Add least-privilege pinned workflow actions, build matrix, verification documentation and explicit development status.
- [ ] Create authorized public source repository, run hosted workflows, inspect results and repair build failures; publish only accurately labelled development artifacts while qualification is pending.

## Whole-branch completion

- [ ] Run Windows/WSL Rust tests, strict Clippy, native tests, Flutter analyze/tests/builds; record real outputs and omissions.
- [ ] Independent whole-branch review, fix important findings with failing regression tests, then green suite.
- [ ] Update implementation/acceptance ledgers to exact implemented behavior; retain all missing native proofs/signing qualification as pending.

## Task 7: Executable host/controller coordinator

Read `docs/engine-contract.md` core-engine section first; its exact API/DTOs are binding. Own `dh-core`, daemon/CLI binaries, runtime platform adapters, and necessary network/protocol/data integration. Do not edit root Cargo/lockfiles or existing platform proof safety. Request root dependency additions from controller. Parent owns VP8 implementation and build tooling.

- [ ] Write failing real loopback tests for broker-gated host/controller status, one-controller busy, wrong password, reset revocation and fresh reconnect. Use synthetic source explicitly; no real screen capture/input in tests.
- [ ] Implement persistent owner-protected configuration, daemon independent of UI, OS-aware native capability statuses, authenticated control and separate media/bulk channels with single-use bounded attachment authority. Preserve all approved security/epoch/input-release policy.
- [ ] Integrate actual VP8 frames into an Arc-owned latest-frame cache/events; integrate clipboard/files with active-user authority and explicit granted directory. Implement public contract without fake success or placeholder features.
- [ ] Implement `dh-hostd` and `dh-cli` executable entrypoints and tests; verify meaningful session behavior and blocked unavailable native capabilities. Report remaining limitations honestly.
- [ ] Run targeted/full suites with parent-provided native library env, self-review own diff, commit only owned files and report to controller for independent review.

## Task 8: Flutter application and Rust bridge

Read `docs/engine-contract.md` Flutter/bridge section first; its API and ownership are binding. Own only the Flutter app, engine_bridge package and listed FFI files. Coordinate contract changes with controller/Task7/Task9; do not edit root Cargo/lockfile.

- [ ] Use test-driven navigation/host/session behavior, including focused keyboard forwarding, stale-state disabling, password clearing and native permission failures.
- [ ] Implement all approved Material3 screens/controllers, connect them to generated FRB2.13.0 wrappers around actual DeskEngine, and consume Task9 NativeSurface; no Dart pixel traffic.
- [ ] Generate runner scaffolding/bridge and lockfiles, analyze/test all UI behavior, build locally possible targets or give concrete hosted-build steps. No global VS/Flutter changes.
- [ ] Self-review own files, commit only owned paths, write report and return for independent task review.

## Task 9: Native video texture boundary

Read `docs/engine-contract.md` native-surface section first. Own native_surfaces package and FFI surfaces.rs only; do not edit app/lib.rs/api or root Cargo/lockfile.

- [ ] Write failing lease/bounds/lifetime tests through actual surfaces.rs helper seam; validate that frame memory remains alive through renderer release and missing frame clears explicitly.
- [ ] Implement shared C ABI and Windows/Linux/macOS/Android native texture plugins with explicit ownership, RGBA orientation, lifecycle cleanup and dynamic loading of the same Rust engine library.
- [ ] Verify native build integration where available, static/compile checks otherwise; report runtime gaps without claiming actual display proofs.
- [ ] Self-review and commit only owned files, write report, return for independent task review.
