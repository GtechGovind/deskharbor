# SDD ledger — plan: docs/superpowers/plans/2026-11-08-capture-composition.md

Baseline:8938429. User reiterated full implementation, public open-source repository, local signed artifacts and release publication once complete. User confirmed they can enable Hyper-V/restart and have no Mac/DeveloperID currently available.

Ruling: continue missing P0 capture implementation while VM setup is pending — keeps the approved milestone order and makes useful native progress without pretending that software fixtures are hardware proof; cost is that this is still not a usable remote application.

Ruling: work in the existing dedicated project checkout on new feature branch feat/capture-composition — preserves the user's exact source location and audited predecessor; no unrelated worktree or main branch changes.

Preflight interfaces: dh-media is independent; dh-platform consumes its frame/composition types; Linux C emits the documented packet. Parent owns all files inline to prevent interface drift. Existing DHF1 mode query and Windows API remain unchanged.

Task1: implemented; eight independent compositor cases observed RED, then GREEN. Initial strictClippy rejected a test's chunks_exact use; changed to as_chunks and verified the whole Windows workspace and strictClippy before completion.

Task2: implemented; independent DHP2 success and rotation fixtures observed RED, then GREEN; malformed header/prefix/flags/count/stride/budget and duplicate/trailing tests pass. Added header-first bounded ingress after review, with counted-reader and truncated/trailing mode fixtures observed RED then GREEN.

Task3: implemented within diagnostic limits; native packet/property/mode/capability/identity/render/publication fixtures had observed RED then GREEN. Six C test programs pass. The explicit C-to-Rust native fixture was observed RED for missing native encoding, then again for raw output on the visible-only path, then GREEN for both synthetic DHP2 and composed DHF1. Three new C fixture programs also pass ASan/UBSan with leak detection. WSL remains a denied-device/no-display negative test, not GPU evidence.

Task4: user-confirmed Hyper-V enable/restart handoff and absent Mac/DeveloperID recorded. Microsoft enable command supplied, never executed here; no automatic reboot, service install, certificate creation, repository publication or release. Public GitHub/release authorization remains for after a qualifying application build. Current source is an increment beyond the previously audited P0 ZIPs, which remain historical artifacts.

## Review rulings

Ruling: never send raw scanout buffers across a future privilege boundary — review identified cropped/offscreen/occluded pixel disclosure; actual worker now composes internally and publishes only visible DHF1 pixels. DHP2 is restricted to synthetic no-device fixtures and defensive decoder tests; cost is maintaining checked C publication logic alongside the portable Rust reference.

Ruling: use positive policies per DRM object, explicit bindings, progressive full mode identity, client capabilities3/6/7 and known optional error semantics — hidden driver state must not be mistaken for complete visible output; cost is explicit unsupported status on layouts/properties not yet understood.

Ruling: endpoint observations and DMA synchronization do not establish pixel coherence — current worker and CLI reject all privileged/capability/set-ID identities; coherent privileged production capture remains disabled pending a reviewed broker/fence/writeback/producer-lease design. Cost is no GDM/production-host proof from this diagnostic.

Final independent read-only review after the fix pass found no unresolved Critical/Important issues within this diagnostic scope. Declined native hardware fidelity, driver behavior, protected-output races, installed authorization and other OS lifecycle claims remain blocked, not assumed passed.

## Verification

- Windows:28tests passed; strictworkspaceClippy and rustfmt passed.
- WSL/Linux:30default Rust/CLI tests passed, one explicit native encoder test ignored by default and separately run/passed (31total executed); strictworkspaceClippy passed. Shared fixtures overlap Windows.
- NativeC: six test programs passed; three newly added fixture programs passed ASan/UBSan with leak detection.
- Native runtime: WSL permission_denied/no_readable_scanout and no-input-without-opt-in cases passed. No real GPU scanout, VM sign-in, Mac build or production remote connection was performed.

The overall approved application is still incomplete. P0 remains0/24 and no full-app release qualification was changed.
