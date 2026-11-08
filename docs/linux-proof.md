# Linux platform proof

## Result

The Linux P0 gate remains blocked. The WSL run exercised native inspection, worker restrictions and error propagation. No Ubuntu 24.04 physical Intel or AMD result is available. The prototype implements a restricted single-plane scanout path. It cannot yet reproduce general GNOME composition or host a controller across GDM and login transitions.

The recorded Ubuntu 26.04.1 WSL2 run had a virtual vgem card, no connected DRM output and no seat0 console. Explicit capture on card0 returned permission_denied. Selecting the first active output returned no_readable_scanout. Both commands exited 1 and created no file. The input executable without its lab flag exited 2. To repeat on this specific negative fixture, run `python3 native/linux/test_wsl.py --bin-dir target/linux/debug`. [Observed results](../native/linux/evidence/wsl.json).

## Build and run

On x86-64 Linux, use Rust 1.99.1, a C17 compiler, make, Linux UAPI headers including DRM (`drm/drm_mode.h` or `libdrm/drm_mode.h`), and systemd-logind with loginctl. From the repository root, build both native executables beside the CLI. The test command runs pixel, filter and input-event fixtures. It does not create a virtual input device.

```sh
export CARGO_TARGET_DIR="$PWD/target/linux"
cargo build --locked --workspace
make -C native/linux BUILD="$CARGO_TARGET_DIR/debug" test
cargo test --locked --workspace
"$CARGO_TARGET_DIR/debug/dh-platform-probe" inspect
```

A connected DRM output has an ID such as `card1:95`. The suffix is a kernel connector object ID and can change after a reboot or reprobe. `primary` selects the first connected output with a readable active mode in sorted sysfs order. Use an explicit ID on mixed-monitor hosts. A zero mode means inactive or unreadable. The DRM report does not have GNOME's global logical origins or primary preference. Numeric process/console-session fields remain null for nonnumeric logind IDs. Seat lookup uses the local active graphical session and its lock hint. Session classification is observational and cannot authorize input.

```sh
"$CARGO_TARGET_DIR/debug/dh-platform-probe" capture --display card1:95 --output sample.bmp
```

## Capture boundaries

The worker opens only `/dev/dri/card0` through `card63`, verifies the character device major/minor and accepts a connector ID. It reads the connector's current encoder and CRTC, enables atomic client properties on its own descriptor, enumerates all exposed planes on that CRTC, and checks the plane rectangles, type, rotation, opacity and selected color/protection properties. It exports a GETFB2 handle to a DMA-BUF, checks the actual backing size, brackets CPU access with DMA_BUF_IOCTL_SYNC and converts the framebuffer's offset and pitch to top-down RGBA. Before emitting pixels it rechecks the display state. This protects against detectable reconfiguration; sequential kernel queries are not a coherent snapshot and cannot exclude an intervening change and reversal.

The accepted case is a single opaque primary plane covering the complete CRTC, unrotated, unscaled, explicitly linear XRGB8888, within the 128 MiB worker limit. Additional active planes including a hardware cursor, cropped geometry, unknown or tiled layouts, multiplanar formats, selected active color transforms and protected output produce explicit errors. The software does not import vendor tiled modifiers or compose independent overlays/cursors. Post-scanout display processing and color fidelity are unqualified. A matching primary framebuffer is diagnostic evidence only.

The worker has no listener, command shell, arbitrary path argument or installer. It clears inherited extra descriptors, requires a pipe for output, sets no-new-privileges, disables dumps, and installs an x86-64 seccomp allowlist before DRM queries. The filter allows read-side DRM/PRIME requests on the verified card, DMA-BUF sync, nonexecuting mappings and output to its pipe. Filter setup failure exits the worker. The Makefile installs no system service, ACL or capability.

Opening a card and retrieving another client's framebuffer handles have different permission requirements. The kernel returns zero handles unless the caller is DRM master or has CAP_SYS_ADMIN. Do not elevate this CLI, grant file capabilities, or install it setuid. This worker rejects root and secure-execution/set-ID identities. It accepts unprivileged execution and is structured to permit a future trusted non-root service with only CAP_SYS_ADMIN. There is no provisioned trusted broker, caller-authentication boundary or reviewed service configuration in this P0 implementation. A restricted service with a trusted executable path, session-epoch checks and independently validated capture/input authorization is required before operational use. [DRM userspace API](https://docs.kernel.org/gpu/drm-uapi.html) · [DMA-BUF CPU access](https://docs.kernel.org/driver-api/dma-buf.html).

## Input proof

Additional review hardening rejects known nonzero panel-orientation, underscan/border and scaling-mode properties instead of presenting them as correctly composed output. The Rust CLI refuses root, saved/set-ID, secure-execution and capability-bearing identities before any child process can run, including loginctl and its sibling worker. This prototype must remain unprivileged; a future separately authenticated broker is not implemented.

`dh-uinput-tap` is a separate one-shot lab executable. With no arguments it exits 2 without opening a device. A lab operator with existing device access can explicitly request a temporary virtual keyboard on the local seat. The only declared key is F24, with one down/SYN/up/SYN sequence. The process destroys the virtual keyboard after delivery and closing its descriptor also destroys it on failure or process exit. It rejects root, secure execution, inherited/permitted capabilities and any device other than the expected uinput character node. Its seccomp filter only permits the required uinput operations, a write to that descriptor, a short wait and exit.

On a dedicated lab desktop with F24 unbound, observe the named virtual device (`DeskHarbor lab F24`) and a focused local test receiver. Explicitly run `target/linux/debug/dh-uinput-tap --confirm-local-lab-f24`. Exit 0 demonstrates event submission and cleanup; record independent receiver evidence to establish delivery. Exit 3 indicates denied device access, 10 indicates an unavailable device/operation, and 11 indicates a rejected identity or sandbox setup. No key event was sent in WSL. Input cannot yet be routed to a selected console epoch. Full keyboard/pointer handling, stale-input rejection, held-key cleanup across transitions and sign-in control remain open. [Kernel uinput interface](https://docs.kernel.org/input/uinput.html).

## Physical qualification

Collect artifacts separately on Ubuntu 24.04 LTS x64 GNOME/GDM Wayland on physical Intel and AMD GPUs with an active connected display. Preserve kernel/Mesa/compositor versions, PCI and DRM driver identity, hardware cursor/plane state, modifier values, output modes, session and capture times, and the identities of the worker and test reviewer. Use dedicated synthetic accounts and known nonsecret image targets. For each gate record supply the expected result, observed result, independent reference and artifact hash.

| Gate records per GPU vendor | Required observation | Current state |
|---|---|---|
| `linux_intel_gdm`, `linux_amd_gdm` | On the installed isolated helper, capture the physical GDM console after boot, then user A, lock, unlock, logout and user B. Compare each frame to an independent observation of the actual display. Denied permissions and missing connectors must prevent publication. | not_run |
| `linux_intel_complete_planes`, `linux_amd_complete_planes` | Compare the visible result including an independently positioned hardware cursor and overlay, direct-scanout fullscreen content, rotation/reflection, fractional scaling, mixed displays, hotplug and driver-specific tiled/modifier surfaces. Record atomic plane metadata, display IDs, transformation and independently observed pixel targets in every configuration. | not_run |
| `linux_intel_input_transitions`, `linux_amd_input_transitions` | On the isolated seat, verify physical-key and pointer arrival at GDM, user, lock and second-user desktops. Switch epochs while keys/buttons are held, disconnect the controller and revoke permissions. Verify releases and no delivery to the old or wrong seat. | not_run |

A WSL capture error is expected; a real-device unsupported modifier, skipped overlay or missing cursor blocks the physical proof. The approved sequence requires finishing and validating the missing privileged service, composition and input paths before Milestone 1.
