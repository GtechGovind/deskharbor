# Run the platform probes

These are P0 diagnostics, not the installed DeskHarbor remote-access application. None listens on a network port. The prototype has no remote viewer, host password, encrypted transport, clipboard or file-transfer path yet.

## Windows

With Rust1.99.1, Visual Studio x64 C++ tools and Windows SDK installed:

```powershell
./scripts/cargo-msvc.ps1 build --locked --workspace
./target/debug/dh-platform-probe.exe inspect
./target/debug/dh-platform-probe.exe --help
```

Use the display ID returned by inspect. Capture is explicit and refuses to replace an existing file:

```powershell
./target/debug/dh-platform-probe.exe capture --display '\\.\DISPLAY1' --output sample.bmp
```

Only an ordinary desktop capture was tested here. Installed LocalSystem workers, UAC, SAS and sign-in behavior require the separate [isolated Windows lab procedure](windows-proof.md). Do not install the proof service on your everyday workstation.

## Android

The debug APK is an attended, local screen-capture proof with an explicit Stop button. It requests Android's screen-sharing consent; the optional input test requires user-enabled Accessibility and only taps its own visible target. It does not connect to a remote device. See [build and device instructions](android-proof.md). Remove the app when finished to erase its private local proof images/logs. Debug builds are not release packages.

## Linux and macOS

See [Linux build/proof details](linux-proof.md) and [macOS signed-harness details](macos-proof.md). The Linux source now has a restricted linear multi-plane diagnostic path with visible-only publication; unknown layouts/color/driver state fail explicitly and coherent privileged GNOME/GDM capture remains unfinished/disabled. Earlier ZIP binaries describe their recorded single-plane revision. Linux binaries built on Ubuntu26.04 WSL are not qualified Ubuntu24.04 packages. The Swift harness has not been compiled or run here.

## Evidence collection and gate

`python tests/lab/collect_probe.py --probe PATH_TO_PROBE --output NEW_DIRECTORY` runs read-only inspection, retaining raw stdout/stderr and hashes. It never performs a login/logout, injects input or promotes an observation to a platform pass.

`dh-platform-probe gate-template` prints the 24 required lab records with `not_run` status. `dh-platform-probe gate --evidence FILE.json --artifact-root DIRECTORY` validates complete operator attestations, expected profiles, native-lab designation and contained artifact paths/hashes. Exit0 means the supplied evidence set satisfies this P0 checklist, exit2 means blocked, exit1 means invocation/IO/schema error. The checklist cannot independently authenticate physical events or certify a release. Required cases must be genuinely observed, not relabeled to make the gate pass.

Do not share desktop captures containing personal information. The Windows local screenshot produced during development is ignored by Git and excluded from the distributable proof bundle.
