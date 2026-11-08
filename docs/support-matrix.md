# Target and current support

| Target | Current implementation/evidence | Qualified host/controller |
|---|---|---|
| Windows11 x64 Home/Pro | Native user-desktop inspection/capture ran. SCM helper/input/SAS source compiled and safety fixtures pass. Installed sign-in lifecycle unrun. A real isolated Windows VM is a valid native lifecycle test target. | Neither |
| macOS Apple Silicon/Intel | Swift public-API harness, signed build command, launchd collectors and required identity records. No native build/signing/install/run. | Neither |
| Ubuntu24.04 GNOME/GDM Wayland Intel/AMD | Restricted linear multi-plane composition and unprivileged DRM/uinput diagnostics; visible-only output. WSLUbuntu26.04 negative paths tested; no physical connected scanout, coherent privileged capture or complete general driver composition. | Neither |
| Android11+ ARM64 | Local consent/foreground capture proof APK. Reducer/resource tests and API37 x86_64 emulator runs; optional accessibility and physical ARM64 qualification pending. | Neither |

No OS is advertised as a working DeskHarbor remote controller or unattended host. The24 required platform cases remain unrun in their qualifying environments. Windows VM evidence can qualify the Windows lifecycle cases if the installed native helpers actually satisfy each observation; virtual sign-in must still be observed, not simulated by a UI page. Physical Linux GPU composition cannot be inferred from WSL or an emulated VGA device.
