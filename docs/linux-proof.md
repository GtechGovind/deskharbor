# Linux platform proof

## Result

The Linux P0 gate remains blocked. The WSL run exercised native inspection, worker restrictions and error propagation. No Ubuntu 24.04 physical Intel or AMD result is available. The current continuation adds restricted packed-linear multi-plane composition, but cannot yet reproduce general driver/GNOME configurations or host a controller across GDM and login transitions. The earlier audit/ZIP records describe the preceding single-plane prototype, not this newer source increment.

The recorded Ubuntu 26.04.1 WSL2 run had a virtual vgem card, no connected DRM output and no seat0 console. Explicit capture on card0 returned permission_denied. Selecting the first active output returned no_readable_scanout. Both commands exited 1 and created no file. The input executable without its lab flag exited 2. To repeat on this specific negative fixture, run `python3 native/linux/test_wsl.py --bin-dir target/linux/debug`. [Observed results](../native/linux/evidence/wsl.json).

## Build and run

On x86-64 Linux, use Rust 1.99.1, a C17 compiler, make, Linux UAPI headers including DRM (`drm/drm_mode.h` or `libdrm/drm_mode.h`), and systemd-logind with loginctl. From the repository root, build both native executables beside the CLI. The test command runs pixel, filter and input-event fixtures. It does not create a virtual input device.

```sh
export CARGO_TARGET_DIR="$PWD/target/linux"
cargo build --locked --workspace
make -C native/linux BUILD="$CARGO_TARGET_DIR/debug" test
cargo test --locked --workspace
cargo test --locked -p dh-platform native_encoder_fixture_matches_rust_composition -- --ignored
"$CARGO_TARGET_DIR/debug/dh-platform-probe" inspect
```

A connected DRM output has an ID such as `card1:95`. The suffix is a kernel connector object ID and can change after a reboot or reprobe. `primary` selects the first connected output with a readable active mode in sorted sysfs order. Use an explicit ID on mixed-monitor hosts. A zero mode means inactive or unreadable. The DRM report does not have GNOME's global logical origins or primary preference. Numeric process/console-session fields remain null for nonnumeric logind IDs. Seat lookup uses the local active graphical session and its lock hint. Session classification is observational and cannot authorize input.

```sh
"$CARGO_TARGET_DIR/debug/dh-platform-probe" capture --display card1:95 --output sample.bmp
```

## Capture boundaries

The worker opens only `/dev/dri/card0` through `card63`, verifies the character device major/minor and accepts a connector ID. It requests atomic visibility, then cursor-hotspot and plane-color-pipeline client capabilities before enumeration. After atomic succeeds, optional EINVAL is treated as an unknown older-kernel capability and EOPNOTSUPP as no corresponding driver interface; other errors refuse capture. It requires explicit active CRTC/mode and connector binding/DPMS properties, positive object-specific property policies, and progressive supported mode flags. Full mode identity and mode-blob ID are retained for endpoint comparison. Unknown properties, active color/protection processing and unresolved cursor hotspots are refused. [Kernel client capability behavior](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/drm_ioctl.c) and [property visibility](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/drm_mode_object.c).

Supported inputs are an opaque full-CRTC primary plus up to31known-order overlays/cursors, packed linear XRGB8888/ARGB8888, integer crops, signed/clipped destinations, quarter-turn rotation/reflection and explicit coverage/premultiplied/global opacity. Source data is limited to128MiB in total and final RGBA to128MiB. Unknown/tiled modifiers, YUV/multiplanar formats, fractional crops, scaling filters, ambiguous z order and unsupported processing remain explicit errors. Color fidelity and direct-scanout behavior are unqualified on physical GPUs.

Raw framebuffer planes are composed **inside the worker**. Only the final visible DHF1 RGBA image leaves it: cropped, offscreen and fully occluded pixels are not exported. Independent C publication fixtures verify secret sentinels hidden by crop/opaque overlay never appear on the output pipe. The Rust compositor is a checked reference/portable foundation; DHP2 raw-plane encoding is used only by the synthetic no-device fixture and defensive decoder, not live privileged capture. See [local packet details](linux-scanout-packet.md).

Capture remains **best-effort diagnostic**, not an atomic or coherent snapshot. Endpoint metadata checks catch some changes, but same-buffer writers and A-to-B-to-A commits can escape detection. DMA-BUF CPU synchronization does not serialize independent writers. A privileged/production path requiring coherent output stays disabled until an independently reviewed authorization plus writeback/fence/producer-lease design and race evidence exist. See [DMA-BUF synchronization](https://docs.kernel.org/driver-api/dma-buf.html).

The worker has no listener, command shell, arbitrary path argument or installer. It clears inherited extra descriptors, requires a pipe for output, sets no-new-privileges, disables dumps, and installs an x86-64 seccomp allowlist before DRM queries. The filter allows read-side DRM/PRIME requests on the verified card, DMA-BUF sync, nonexecuting mappings and output to its pipe. Filter setup failure exits the worker. The Makefile installs no system service, ACL or capability.

Opening a card and retrieving another client's framebuffer handles have different permission requirements. The kernel returns zero handles unless the caller is DRM master or has CAP_SYS_ADMIN. Do not elevate this CLI, grant file capabilities, or install it setuid. Both the CLI and worker reject root, saved/set-ID, secure-execution and all capability-bearing identities. Current capture can therefore work only with an already-authorized unprivileged DRM master; ordinary sessions may receive permission_denied. No future capability-bearing service is enabled by this diagnostic. A trusted broker, correct user/session authorization, coherent capture design and installation configuration are still missing. [DRM userspace API](https://docs.kernel.org/gpu/drm-uapi.html).

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
