# SDD ledger — plan: docs/superpowers/plans/2026-11-08-platform-proof.md

## Authorization and baseline

- User explicitly requested implementing the approved DeskHarbor plan on 2026-11-08.
- New repository at the requested output path; existing workspace is not a Git repository. No pre-existing product code or tests.
- Windows Rust1.99.1 available. Process is not elevated. No configured macOS signing/test target. WSL Ubuntu26.04 is present, not the certified Ubuntu24.04 GNOME/GDM physical Intel/AMD environment. Android SDK/emulator are available; no attached devices were reported.

## Preflight interface/conflict scan

| Tasks | Shared boundary | Resolution |
|---|---|---|
| Shared / Windows | Inspection, Display, Frame, ProbeError | Parent owns contract; native agent only implements its directory |
| Shared / Linux | Same contract and CLI dispatch | Linux backend reports actual environment and denies unsupported hardware |
| Shared / Android | Evidence format and P0 scope | Native APK is a proof harness, not the later Flutter product |
| Shared / macOS | Evidence format and gate | Swift harness uses native public APIs; no local Mac certification claim |
| All / release | P0 before features | Missing lab proofs block Milestone1; no empty fake product scaffolding |
| Each task / tests | Actual behavior vs claims | Tests and captures document ordinary-session scope only |

## Rulings

- Ruling: Initialize a fresh isolated repository on `feat/platform-proof` rather than creating a worktree from an unrelated/nonexistent repository — preserves the user's exact destination; cost if wrong is only repository relocation.
- Ruling: Follow the user's explicit P0 gate and implement native proofs before the full product tree — later milestones remain pending until certified hardware evidence exists; cost is a staged rather than immediately complete app.
- Ruling: Never sign out/reboot this active workstation merely to run proof tests — those steps belong to an isolated authorized lab; cost is that local ordinary-session checks cannot clear the sign-in gate.

## Progress

| Task | Implemented and verified here | Remaining |
|---|---|---|
|1 shared contracts/CLI/gate | Bounded RGBA/BMP; real platform dispatch;24case gate with structured operator/observed checks, contained bounded hashes and macOS installed-identity requirements; CLI errors and tests | Actual qualifying lab attestations |
|2 Windows | Native inspection and1920×1080 GDI capture; SCM/System worker source; fixed F24 and policy-gated SAS source; service/file ACL, expiry, desktop and refusal fixtures | Install/service execution, VM sign-in/lock/logout/UAC/SAS/input delivery,50cycles; no native input was sent here |
|3 Linux | DRM inventory; sandboxed restricted linear scanout and F24 helper; Rust/C tests; WSL denied-device/no-display checks | Trusted installed broker; complete plane/modifier composition; physical Ubuntu24.04 Intel/AMD/GDM and transition input |
|4 Android | Real attended Kotlin APK;11 lifecycle/consent/resource JVM tests; API37x86_64 emulator consent/capture/foreground/stop observations | See Android evidence for exact build/run; accessibility enable/revoke, system stop/lock/protected content and physical ARM64 coverage remain unqualified |
|5 macOS/lab runner | Swift public-API harness/signing command/private launchd collectors; structured role/context gate; Python read-only collector3tests | First Swift native build; actual signed installs and LoginWindow/Aqua proofs on both architectures |
|6 review/handoff | Independent read-only initial and final review; substantive findings fixed; Windows20 and Linux15 Rust/CLI tests,3nativeC programs,11AndroidJVM tests,3Python tests; strictRustClippy/format verified; source committed at66f0d32; source/probe ZIPs inspected and hashes checked; private Windows screenshot excluded | Full product and native qualification remain blocked |

## Evidence and review

- Meaningful failing tests were observed before implementation for frame row/color order; Windows conversion/session/worker eligibility; native allocation bounds; fixed F24/expiry/SAS policy/SCM ACL/default-to-secure transition; missing/duplicate/tampered/diagnostic-only gate records; calendar timestamps; Linux geometry/layout/color/filter/input and CLI privilege guards; Unix0600 screenshot creation; Android consent epoch/request IDs (including process-restart collision/persistence failure), null display and cleanup; and Python collection/error/qualification behavior. The final relevant suites pass. Unit fixture attestations are never lab evidence.
- Reviewer found default SCM custom-control permissions unsafe. Fixed protected SYSTEM/Admin-only service-object checks before enabling controls and per dispatch, explicit Default/Winlogon controls without fallback, session-change cancellation, and fatal impersonation-reversion handling. Fresh-boot/old-handle caveat is documented for an exclusively controlled VM.
- Reviewer found Android delayed consent, resource-allocation/resize cleanup and cross-display tap hazards. Fixed durable one-shot request allocation, immutable epoch matching, nonnull display/resource gating, independent cleanup and default-display-only local-target gestures.
- Reviewer found privileged Linux sibling dispatch, permissive capture-file mode, connector transformations and weak generic proof records. Fixed the guard before subprocess execution,0600 capture output, explicit transform refusal, structured build-bound observations/signing/context evidence, special/empty/oversized artifact rejection and bounded nonblocking reads.
- Reviewer found macOS agent observations were lost. Both launchd jobs now use an exclusive private per-identity collector; Mac execution remains unrun.
- Final independent review found no remaining Critical/Important issues **for the diagnostic harness only**, conditional on fresh build/test verification. No finding certifies a native OS lifecycle or the full product.
- The virtual-sign-in follow-up added read-only VM preflight/planning. Review rejected an early unexecuted creation path because privileged VM writes through user-owned directories permit path replacement. Removed creation entirely; only a concrete manual Hyper-V Manager procedure with administrator-managed storage remains. The final read-only source/tests passed independent review; VM inventory/storage and actual sign-in stay unverified. No host feature/policy was changed.

## Gate ruling

`docs/proof-gate.json` records **ready=false, passed=0, required=24** and exit2. The cases are required platform proofs; Windows lifecycle testing may use a real isolated Windows11 VM. A VM has not yet been created or tested here. WSL and the x86 Android emulator do not establish the selected physical GPU/ARM64 support matrix.

The user's virtual-sign-in follow-up is accepted: investigate and prepare an isolated Windows VM rather than signing out the user's workstation. Preflight found virtualization active, no installed desktop VM manager or Windows image, and under2GiB free RAM during the existing test run. VM preparation does not convert an unrun sign-in test into a passed case.

After the task-owned Android emulator shut down, observed free memory recovered to about5GiB, still below the8GiB lab margin. The final read-only VM script tests pass. No VM was created, booted or signed in. Its manual Hyper-V Manager procedure and actual blockers are in windows-proof.md/windows-vm-preflight.json.

Milestones1–5 are **pending** under the user's explicit P0 ordering. No Flutter viewer, password authentication, transport, installed production host, clipboard/files, performance claim or release installer is reported complete.

## Final expectation audit — 8 November 2026

The user requested a complete expectation audit and cleanup. Rechecked the full approved plan, source tree, all available test/build commands, current Windows/VM inspection and delivered archives. Fresh checks passed: Windows20, Linux15, three nativeC test programs, three Python tests, read-only PowerShell VM cases, strictRustClippy/format and the Android offline build (up-to-date11test reports, zero failures). No new emulator or installed privileged-host test was represented as run.

Independent review confirmed the diagnostic-only scope and found no new Critical/Important implementation defect within that scope. It identified unresolved exported Android evidence paths; reproduced the failure and corrected only the bundle summary with explicit source-to-bundle provenance. The original source evidence stays unchanged. Revalidated artifacts and refreshed source/bundle archives, manifests and checksums. See final-audit.md and final-audit.json for the full mismatch between the planned product and delivered prototype.

Finalization applies to the audit and P0 handoff. Product readiness remains false and the platform gate remains0/24. Missing Linux composition/broker and other native code are implementation gaps, in addition to unavailable lab proofs. The exact proposed Flutter/Rust production file structure is not yet implemented.
