# DeskHarbor approved design

The user's 8 November 2026 instruction explicitly authorizes implementing the supplied DeskHarbor plan. This document preserves its binding requirements; the chat contains the detailed planned full-product file tree.

## Product

- Independent new application; no VNC/RDP interoperability or OS remote-desktop-server dependency.
- Rust 1.99.1 core; Flutter 3.47.9/Dart UI; Riverpod/go_router; flutter_rust_bridge 2.13.0.
- LAN/VPN direct addresses; no external coordination, relay, account, cloud database, or vendor backend.
- Windows 11 x64 Home/Pro; macOS Apple Silicon and supported Intel; Ubuntu 24.04 LTS x64 GNOME/GDM Wayland Intel/AMD with an active connected display; Android 11+ ARM64.
- Desktop host survives GUI close, lock, logout and OS sign-in transitions. OS must be running and encrypted disk unlocked; no preboot unlock, power-on or sleep support.
- Android controller and attended host; MediaProjection consent each session, AccessibilityService only with explicit OS permission. No silent Android boot hosting or protected-screen bypass.
- Permanent host password through broker-verified OPAQUE/Argon2id, TLS-exporter binding, protected machine secrets and restricted workers. QUIC47832 with TLS/TCP fallback. No privileged 0-RTT.
- Screen/input, Unicode text/PNG/file-offer clipboard, file transfer, monitors, saved devices, reconnect. No audio/chat/recording/printing/browser/v1 VNC.

## Processes and boundaries

GUI/controller; privileged hostd authentication broker; restricted netd; active-console user agent for clipboard/files; narrow privileged capture/input helper; CLI. Session epochs invalidate stale input and file/clipboard access. Only screen/input is allowed while locked/signed out. One incoming controller; multiple outgoing connections.

## Media and UI

VP8 baseline, negotiated native H264 acceleration, native Flutter textures. Complete Linux visible-plane composition, cursor/rotation/scaling. Physical HID keys with host layout, explicit text events, normalized monitor coordinates and layout epochs. Release held input on every loss/transition.

Material3 navy/teal, light/dark, desktop sidebar and Android navigation, 44px minimum controls. Connect, This Computer/Share, Session, Devices, Transfers, Settings and Permissions pages. Explicit readiness and unsupported states. No production UI is built before the platform-proof gate.

## Delivery sequence and release gates

0. Native installed-helper proofs, then 1. authenticated foundation, 2. service/native lifecycle, 3. media/Android, 4. clipboard/transfers/UI, 5. signed packages/release qualification.

**Binding gate:** If a required proof fails, v1 remains blocked. Do not silently substitute VNC/RDP, require an already logged-in desktop, or label partial support complete.

P0 needs Windows boot/signin/lock/unlock/logout/UAC/SAS; actual signed macOS LoginWindow/Aqua daemon/agent with second-user and permission-revocation tests; Ubuntu GDM and session capture/input on Intel and AMD, including overlays/direct-scanout/cursor/rotation/mixed monitors/modifiers; Android consent/foreground/input/stop lifecycle. Ordinary capture, code compilation or emulation alone cannot satisfy those proofs.

Final qualification additionally needs 50 login cycles, eight-hour soak, offline LAN/VPN, UDP-blocked fallback, security/IPC/channel-binding negatives, cross-platform clipboard and files, install/upgrade/uninstall and policy restoration. Performance target1080p30/median150ms on documented physical reference hardware.

## Current implementation scope

Only P0 and the tests/tools needed to evaluate it are currently authorized to advance under the milestone ordering. Future product files are not populated with fake implementations or success returns. Missing physical tests are `not_run`, never `passed`.
