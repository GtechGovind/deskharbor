# DeskHarbor Platform Proof Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development for independent native probes and an independent whole-change review. Track each deliverable below.

**Goal:** Implement and exercise the approved plan's P0 native probes, preserving the gate before later product features.

**Architecture:** A common Rust inspection/capture/evidence contract and CLI dispatch to actual native backends. Dedicated Android and macOS native harnesses prove their OS-specific APIs. Hardware evidence records its exact scope and does not infer sign-in support from a user-session screenshot.

**Tech Stack:** Rust1.99.1, Win32 APIs, Linux DRM APIs, Swift/ScreenCaptureKit, Kotlin/Android native APIs. No Flutter product UI before P0.

**Spec:** `docs/product-design.md`.

## Global Constraints

- No external coordination/relay/cloud and no VNC/RDP substitution.
- No live-machine logout/reboot, host-service installation, privilege escalation prompt, or security-policy edit as a hidden side effect of tests.
- Native Windows inspection/current-desktop capture can run locally. VM/signed-install/physical-Linux tests require the appropriate isolated lab.
- Unsupported and unrun cases must be explicit. No fake success or certification.
- Prior installer-analysis artifacts remain outside this repository.

## Review Focus

1. Code claims sign-in success from ordinary user-session capture: evidence gate rejects incomplete cases.
2. Native dimensions, byte lengths, row order or GPU modifiers produce truncated/wrong images: independent frame fixtures and fail-closed backend checks.
3. Helper/probe changes the user's input or machine service state unexpectedly: read-only defaults and explicit commands/confirmation.
4. Android consent ends but captured data or a foreground service persists: stop callback clears resources and stops session.
5. Missing hardware/provisioned permissions are mislabeled passed: required proof inventory rejects missing/not-run evidence.

## Tasks

### Task 1: Shared contracts, CLI and proof gate

Files: `crates/dh-platform/src/{lib.rs,report.rs,frame.rs,gate.rs}`; `bins/dh-platform-probe/src/main.rs`; integration tests.

- [x] Write behavioral failing tests for malformed frames, BMP color/row order and missing proof records.
- [x] Implement typed inspection, bounded RGBA frame validation, local image export and evidence evaluation.
- [x] Implement inspect/capture/gate CLI with honest nonzero exit statuses.
- [x] Verify with cargo test; run native inspection and capture.

### Task 2: Windows native backend

Files: `crates/dh-platform/src/windows/`, optional Windows probe/helper assets and docs.

- [x] Read Task1's exact interface and implement real display/session/desktop/elevation inspection.
- [x] Implement real current-desktop GDI capture with resource cleanup and actual input APIs behind explicit opt-in probe commands.
- [x] Provide isolated VM/service proof steps, without signing out or reconfiguring this workstation.
- [x] Run applicable native tests and capture; retain evidence.

Installed input/SAS and sign-in execution remain unrun; source completion does not clear a Windows proof.

### Task 3: Linux native backend and helper

Files: `crates/dh-platform/src/linux/`, `native/linux/` and Linux proof docs.

- [x] Implement actual DRM/device/session inspection and a narrow capture/helper path.
- [x] Exercise denied/unsupported paths and any runnable native checks in WSL.
- [x] Document/encode physical Intel+AMD/GDM complete-plane proofs that WSL cannot establish.
- [x] Run native compile/tests where available; retain exact environment evidence.

General composition and a trusted installed broker are still unfinished; the restricted diagnostic path is not a certified host.

### Task 4: Android proof application

Files: `native/android/probe/`, native services, Android proof docs.

- [x] Build an actual Android app requesting MediaProjection consent and tracking foreground-session lifecycle.
- [x] Implement explicit AccessibilityService configuration, stop cleanup and local capture evidence.
- [x] Build APK with available SDK. Use only an isolated emulator for automated execution; no unrelated connected phone.
- [x] Record which required Android lifecycle tests actually ran and which remain physical-lab work.

### Task 5: macOS native harness and lab runner

Files: `native/macos/`, macOS proof docs, `tests/lab/` and scripts.

- [ ] Implement and natively compile/test Swift ScreenCaptureKit inspection/capture and signed-identity run harness. Source/collectors/signing command are written; first Mac compile/run is unavailable.
- [x] Require the intended daemon/LoginWindow/Aqua security identities and permissions for acceptance.
- [x] Record missing Mac/signing evidence as not_run; test runner/evidence behavior on this host.

### Task 6: Review, evidence and handoff

- [x] Run all available checks; inspect output; document unavailable tests by name.
- [x] Obtain independent code review and address substantive findings with regression tests.
- [x] Commit source, lockfile, tests and ledger; package probe binaries/APK if built.
- [x] Do not advance to Milestone1 while required P0 proofs remain unsatisfied.

Virtual-sign-in follow-up: read-only VM preflight/plan and guarded manual Hyper-V Manager procedure added and independently reviewed. No creation path executes from this user-owned checkout. No VM or guest sign-in has run. The platform gate remains0/24; this checklist tracks diagnostic implementation, not product acceptance.
