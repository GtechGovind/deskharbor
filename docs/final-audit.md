# DeskHarbor expectation and delivery audit

Audit date: 8 November 2026 (Asia/Calcutta). Reviewed application source: `42bbfa11801f4f5357d767d720e2b78194a03a86`. Later changes for this audit are documentation and exported evidence/packaging corrections.

**Verdict: the approved application is not complete and cannot be finalized as v1.** The current deliverable is a native diagnostic prototype. Milestone0 is partially implemented and unqualified; Milestones1–5 are pending. The platform gate was rerun and correctly returned **ready=false,0/24, exit2**. No platform is certified as a DeskHarbor controller or unattended host.

## Full plan compared with actual code

| Approved milestone | Actual implementation | Acceptance status |
|---|---|---|
|0: prove installed platform capture/input and sign-in lifecycle | Windows native/GDI/service proof source; restricted Linux DRM/uinput diagnostic; Android local consent/capture app; uncompiled Swift harness; operator evidence gate | Incomplete;0/24 qualified |
|1: authenticated offline connections | No protobuf protocol, OPAQUE/Argon2id, TLS binding, authorization grants, protected broker IPC, QUIC or TCP fallback | Not implemented |
|2: persistent desktop host and native adapters | Windows diagnostic workers are not the production host. Trusted Linux broker/general composition and production session epochs/held-input cleanup are absent. No complete Mac host | Not implemented/qualified |
|3: streaming/controller/media/Android roles | No VP8/H264 pipeline, Flutter texture bridge, remote viewer, monitor negotiation or network controller. Android APK captures locally; it does not control a remote computer | Not implemented |
|4: complete UI/clipboard/transfers/storage | No Flutter/Riverpod/go_router/FRB application, clipboard synchronization, file transfer/resume, device database, settings/history or reconnect | Not implemented |
|5: packages and release qualification | No product MSI, signed/notarized PKG, Ubuntu24.04 DEB or release APK. ZIPs are probe bundles. No measured soak,50login cycles, full offline/VPN matrix, upgrade/uninstall or latency result | Not implemented/qualified |

The proposed exact project structure is also incomplete. Cargo has only `dh-platform` and `dh-platform-probe`. The planned `apps/client`, `packages/engine_bridge`, `packages/native_surfaces`, `proto`, production engine crates/binaries, `packaging`, `xtask` and CI workflows are absent. Adding empty files would not satisfy the plan.

## Native platform gaps

- **Windows:** ordinary user-desktop inspection/capture exists. No installed LocalSystem sign-in/UAC/SAS/input lifecycle proof or actual VM boot/sign-in result exists. The VM helper performs read-only Preflight/Plan, explicitly reports zero actions and never creates a guest. Its latest audit preflight is still blocked. Existing service source/unit fixtures cannot establish runtime delivery or unattended safety.
- **macOS:** first native Swift compilation, intended signed installed identities, provisioned permissions and LoginWindow/Aqua/second-user/revocation results are all missing. Launchd templates and signature inventory are not a successful installed host.
- **Linux:** WSLUbuntu26.04 denied-device/no-display behavior is verified, outside the certified target. The prototype deliberately rejects overlays/cursors, tiled/modifier and transformed layouts. A trusted installed broker and complete visible-plane composition still need implementation, followed by physical Ubuntu24.04 Intel/AMD/GDM tests.
- **Android:** the final local proof APK has recorded API37x86_64 emulator consent/capture/foreground/app-stop/notification-stop evidence. Physical ARM64, enabled Accessibility gesture delivery/revocation, actual system projection revocation/secure lock, task removal/process death, protected content and resize coverage remain unqualified. Emulator screen-off did not stop projection and was correctly not labeled a lock test.

These gaps require both additional implementation and appropriate test environments; obtaining lab machines alone would not finish the application. The approved P0 gate remains binding.

## Verification performed for this audit

| Check | Result and limit |
|---|---|
| Windows `cargo test --locked --workspace` |20passed,0failed:17library and3CLI tests |
| Windows strict Clippy and rustfmt | Passed |
| Linux/WSL Rust workspace tests |15passed,0failed:11library,1file-permission and3CLI tests; shared tests overlap Windows |
| Linux native C tests |3test programs passed |
| WSL capture/input negative fixture | Expected permission_denied/no_readable_scanout, no output file; opt-in input refused without its flag |
| Android offline build/test command | Succeeded; Gradle reused up-to-date tasks. XML reports11tests,0failures/errors. No new Android emulator run during this audit |
| Android APK | SHA256 `95cf4652726754dfe8ee89a3232c82f9bc62927ea1bd9c56a3bba6ba2a977a13`, same build as recorded final emulator evidence |
| Python collector tests |3passed |
| PowerShell VM tests | Parser and default/explicit read-only Preflight/Plan cases passed; creation unsupported |
| Current Windows inspection | One display, ordinary user desktop, non-elevated; no input injected or session changed |
| Proof gate | Correctly blocked:0/24, exit2 |
| macOS native build and full product acceptance | Not run; no qualifying environment/product implementation |

## Review and cleanup

An independent read-only review found no newly confirmed Critical/Important defect in the inspected diagnostic implementation. That finding is bounded and does not certify native operation, authentication security or release readiness. It identified the missing product/milestone work above as acceptance blockers.

One packaged-evidence problem was reproduced: Android summary references used repository paths that do not exist inside the extracted probe bundle. The exported summary now resolves final APK and observation references from the bundle root, includes source-to-bundle provenance, and explicitly labels the prior build/AVD as not included. The original repository evidence record remains intact.

Source archives, probe manifests, bundled binary/APK identities, observation references and SHA256 checksum files were checked and refreshed for the handoff. Git-declared CRLF export of `gradlew.bat` is intentional and is not source drift. Private Windows desktop captures, credentials, signing keys, VM disks, build caches and the obsolete pre-review APK remain excluded from the ZIPs. Unrelated earlier TightVNC analysis and the user's existing emulator/device data were not removed or changed.

## Finalization decision and next work

Finalize this **audit and diagnostic handoff only**. Do not describe it as a finished four-platform application or a modern AnyDesk replacement.

Continue Milestone0 by implementing the missing native paths and exercising a real isolated Windows VM, signed Mac installations, the selected physical Linux GPU configurations and Android permission/lifecycle cases. Preserve failures and actual evidence. Only after those proofs pass should the authenticated foundation, complete UI/features and release milestones advance. The existing source/probe archives remain explicitly labeled P0.
