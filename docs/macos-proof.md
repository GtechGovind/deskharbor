# macOS P0 harness — not yet compiled or certified

No Mac or signing identity is connected to this workspace. Every macOS acceptance case remains **not_run**. These files are a public-API harness for an operator-controlled Mac lab, not a working unattended host.

On macOS14+ with Xcode command line tools, run `sh native/macos/build-signed.sh 'Developer ID Application: …' TEAM_ID`. It compiles the Swift package, creates three distinct signed executable identities and verifies the signing team. It does not install anything. Preserve complete compiler output; no successful build has been observed here.

The probe implements:

- `inspect`: active display geometry, console identity, process UID, Screen Recording/Accessibility permission queries and codesign inventory. Queries never request permissions automatically.
- `collect`: writes that inspection as a unique owner-only JSON file under the executing identity's home `.deskharbor-p0-evidence` directory. It refuses a symlink, foreign-owned or nonprivate evidence directory. Both launchd manifests use this mode so agent observations survive after the diagnostic process exits. Retrieve `/var/root/.deskharbor-p0-evidence` for root LoginWindow/daemon jobs and each lab user's home equivalent for Aqua jobs; confirm the actual execution UID/home in the lab. Root daemon and LoginWindow observations can share the root evidence directory but retain distinct collector role, executable/signature, PID and timestamp. This path has not yet been exercised on a Mac.
- `capture DISPLAY_ID FILE.png`: a bounded ScreenCaptureKit/SCScreenshotManager capture, refuses overwrite, owner-only output. Permission denial is an error; no fallback to a different process or remote desktop product.
- `input-test --acknowledge-isolated-lab`: explicitly sends a fixed F8 down/up only when Accessibility is granted. An operator must confirm the visible effect; event submission is not a passed input test.

The launchd manifests are **inspection-only** diagnostic jobs. They do not supervise a persistent host or establish capture/input at LoginWindow. A root daemon query is not proof that a LoginWindow/Aqua agent can capture or inject input. No entitlement or TCC bypass is assumed.

Use disposable, backed-up Apple Silicon and Intel lab Macs. Install the exact signed binaries into root-owned `/Library/DeskHarbor/P0`, record their hashes, designated requirements, team, OS version and architecture. Install the diagnostic plist in the appropriate LaunchDaemons/LaunchAgents directory with root ownership and mode0644. Validate with `plutil -lint` before bootstrapping. Provision OS permissions for each intended executable identity using supported OS administration. Do not reuse a development terminal's permission as proof for the installed agent.

Capture and input must then be exercised **from the actual LoginWindow/Aqua bootstrap/security context**, including post-signout and a second user. The inspection-only manifests need an operator-controlled lab job configuration for each explicit capture/input invocation. Record launchd domain, UID, audit session, signing verification, video/pixel evidence, observer input confirmation, logout transition and revoked-permission failures. Remove diagnostic jobs with launchctl bootout, then remove only these P0 files after the lab run.

The four proof cases per architecture are LoginWindow capture+input, post-signout capture+input, second-user separation, and permission revocation. Missing or failing public-API access blocks the selected v1 requirement. Do not claim generic macOS sign-in support from a normal Aqua screenshot or from the source in this repository.

Public API references: [Apple capture sample](https://developer.apple.com/documentation/screencapturekit/capturing-screen-content-in-macos), [SCScreenshotManager](https://developer.apple.com/documentation/screencapturekit/scscreenshotmanager), [Screen Capture permission query](https://developer.apple.com/documentation/coregraphics/cgpreflightscreencaptureaccess()).
