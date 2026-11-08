# Supported Scanout Composition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Independent whole-change review follows implementation.

**Goal:** Extend P0 capture from an explicitly single-plane diagnostic toward supported linear multi-plane scanout, including cursor/overlay alpha, crop, clipping and rotation, without claiming native certification.

**Architecture:** A dependency-free `dh-media` Rust compositor accepts validated snapshots. Linux's restricted worker supplies bounded plane metadata/pixels over a versioned local packet; the parent validates the entire packet before composition/publication. Keep the old DHF1 mode packet and normal Windows capture working. Native hardware proof remains separate.

**Tech Stack:** Rust1.99.1; existing Linux C17/DRM worker and seccomp restrictions. No new remote listener, privilege acquisition, VNC/RDP dependency or production Flutter UI.

**Spec:** `docs/product-design.md`, Linux complete-visible-result requirement; user instruction to continue implementation and use virtual machines when possible.

## Global Constraints

- P0 and release acceptance stay blocked until qualifying native observations exist. Mac is currently unavailable; user will enable Hyper-V and restart manually.
- No host feature/policy change, automatic reboot, administrator invocation of user-writable scripts, service installation or input injection during tests.
- Missing/ambiguous GPU metadata, unsupported modifiers/formats/protection/color/filter/layout, stale snapshots and malformed packets must fail explicitly.
- Limits:32planes,16384pixels per edge,128MiB total source pixels and128MiB output. Validate before allocation/publication.
- This increment does not implement VP8/H264, Flutter, networking/authentication, clipboard or files, nor qualify any platform.

## Review Focus

1. A transparent cursor is omitted or channel order is wrong: independent exact-pixel fixtures for coverage/premultiplied alpha and padded BGRA inputs.
2. Source crop, negative destination origins, clipping or90/180/270degree transforms select another pixel: asymmetric fixture images and boundary checks.
3. Untrusted dimensions/stride/count/offset overflows allocate or panic: checked arithmetic and malformed packet fixtures.
4. Plane order/geometry changes or equal ambiguous z values produce plausible wrong output: validate unique plane IDs and z ordering; reject ambiguous order until a native tie-break is known.
5. Software composition is misrepresented as physical GPU correctness: retain unqualified statuses and explicitly reject unknown sampling/color/modifier semantics.

## Task1: Portable composition library

Files: `crates/dh-media/Cargo.toml`, `src/lib.rs`, `src/plane_compositor.rs`, `tests/composition.rs`; root Cargo.toml membership.

Interfaces: `compose(&Scanout)->Result<ComposedFrame,CompositionError>`; snapshot consists of bounded display dimensions and immutable planes. Plane includes unique ID/z, packed RGBA/BGRA/XRGB source dimensions/stride, integer source crop, signed destination origin, quarter-turn rotation, reflection, global16bit opacity, and None/Coverage/Premultiplied blend semantics. Output is opaque RGBA8. This increment accepts one-to-one source/destination geometry; unknown native scaling remains unsupported.

- [ ] Write exact overlay/cursor alpha, ordering, crop/rotation/clip, malformed dimension/stride/ID/z, premultiplied and budget tests; observe failure against a non-implementing stub.
- [ ] Implement checked snapshot validation and deterministic integer composition, including correct output clearing/primary coverage checks.
- [ ] Run crate and full workspace tests, strictClippy and format; commit.

## Task2: Bounded Linux multi-plane packet decoder

Files: `crates/dh-platform/src/linux/{mod.rs,packet.rs}`, dh-platform Cargo.toml; packet fixture tests in packet.rs.

Interfaces: Keep `linux::capture(&str)->Result<Frame,ProbeError>`. Define a little-endian DHP2 packet with version1, output dimensions, count and total payload length; each plane records IDs/z, format/blend/rotation/reflection/opacity, source dimensions/stride/crop, destination geometry and exact byte count. No arbitrary offsets/pointers. Reject trailing data and duplicate IDs/z before composition.

- [ ] Document the exact wire layout and add independently assembled success/error fixtures; observe RED.
- [ ] Decode into Task1 types with fixed limits before allocating; use compositor result for the existing Frame API. Keep legacy DHF1 for mode query/single-plane compatibility.
- [ ] Run full native Windows/WSL Rust tests and strictClippy; commit.

## Task3: Restricted worker integration

Files: `native/linux/{worker.c,scanout.h,test_scanout.c,Makefile}` and dedicated packet tests as required.

- [ ] Add behavioral native fixtures for supported plane metadata/packet bytes and malformed/ambiguous ordering; observe RED.
- [ ] Collect supported connected-CRTC planes and packed linear RGB data; keep device/identity/seccomp restrictions, bounded budgets, before/after snapshots and explicit unsupported errors. Emit DHP2 only after all planes validate.
- [ ] Validate native packet bytes against Rust decoder/compositor fixtures; run nativeC tests and WSL denied/no-display fixture. No GPU completeness claim without physical target evidence.
- [ ] Run independent whole-change review, resolve substantive issues, commit and update the implementation ledger.

## Task4: Resource handoff and status

- [ ] Record user-confirmed manual Hyper-V setup/restart and absent Mac/DeveloperID; supply Microsoft setup command without executing it.
- [ ] Update docs with exact implemented limitations/results and remaining production work; do not label v1 or all artifacts complete.
- [ ] Preserve open-source repository/release authorization for after a qualifying application build; no early production release.
