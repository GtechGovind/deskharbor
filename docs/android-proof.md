# Android P0 native proof

The native Kotlin probe in `native/android/probe` targets Android 11+ (min API 30), compiles against API 35 and uses framework APIs. This is an attended local proof harness; the Flutter product is still gated. It has no network permission, boot receiver or automatic projection grant. Its architecture-neutral APK can install on ARM64 Android, but **building it does not qualify physical Android support**.

## Build and inspect

Use an installed Android SDK containing platform 35/build-tools 35.0.0 and JDK 17 or 21. The available Android Studio JBR is Java 25 and cannot run this pinned Gradle 8.9. A cached JDK 21 was used for the recorded build.

```powershell
./scripts/android-proof-build.ps1 `
  -Sdk C:/Users/govin/AppData/Local/Android/Sdk `
  -JavaHome C:/Users/govin/.gradle/jdks/eclipse_adoptium-21-amd64-windows.2 `
  -Offline
```

Omit `-Offline` when dependencies are not cached. Google/Maven Central supply the pinned Android Gradle plugin 8.7.3, Kotlin 2.0.0 and JUnit 4.13.2. The standard Gradle wrapper pins Gradle 8.9 and its [official SHA-256](https://services.gradle.org/distributions/gradle-8.9-bin.zip.sha256). The script uses installed SDK aapt2 instead of downloading a second copy. The APK is copied to `native/android/probe/artifacts/deskharbor-android-probe-debug.apk`. It is signed with the local Android **debug** key and is not a release package.

The JVM suite exercises the actual session reducer, unique immutable consent-request epochs, cross-process request ID reservation, failed-persistence denial, null-display readiness and independent resource cleanup. Tests were observed failing before their implementation. They establish those pure Kotlin contracts, not Android service/permission behavior. The recorded test count/result is in the host evidence JSON.

## Lifecycle and evidence

1. Start the probe and press **Start capture — ask Android consent**. Android's system consent dialog opens for every session. Accept or decline there. A declined grant never starts the foreground service.
2. An accepted grant starts the non-exported foreground service with type `mediaProjection` before obtaining the projection. Every OS request has a unique 16-bit ID reserved synchronously in app-private preferences before launch, and an immutable originating epoch in the process-owned request book (retained through Activity recreation). IDs never wrap/reuse; failed persistence or exhaustion refuses the request. Process death discards outstanding authority and subsequent requests reserve new IDs, so restored old results cannot consume new requests. A result only transitions a matching currently-awaiting epoch once; stale grants and denials are inert. The service registers `MediaProjection.Callback` before creating exactly one nonnull virtual display, uses `ImageReader`, and saves one bounded PNG (longest side at most 1280 px) in private app storage. Row stride is honored and RGBA channels are converted explicitly.
3. The notification has **Stop**. The app also has an explicit stop control. `MediaProjection.Callback.onStop`, task removal, service destruction, start failure and capture failure all revoke frame/input authority, release the virtual display, detach/close the reader, unregister/stop projection as appropriate, remove foreground notification and call `stopSelf`. The service returns `START_NOT_STICKY`. A process death cannot recover or reuse a projection consent token.
4. App-private `files/proof.jsonl` is append-only event evidence; `files/state.json` is the latest state. Each event has schema, random run ID, sequence, timestamp, session epoch/phase, foreground/projection/reader/display reference flags, accessibility connection/input authorization, API, ABI, model and event-specific details. The log never serializes the consent intent, UI text, accessibility event contents, account names, addresses or credentials. `frame_saved` adds PNG filename, dimensions, byte length and SHA-256.
5. Stop detaches all live capture resources and rejects queued frames and subsequent input. The local PNG and JSON **remain as explicit proof artifacts**, not an active capture stream; clear app storage or uninstall the disposable probe to delete them. Do not use real confidential screens as test fixtures. `physical_qualification` remains `not_run` in app logs; a signed external lab record must determine physical acceptance.

Reference flags describe the probe's current resource ownership. Each cleanup is attempted independently; a failing release does not prevent closing the reader or removing foreground state. Terminal details include `cleanup_errors`; a nonzero value fails the proof even if references are cleared. A replacement reader closes if resizing/surface configuration fails before ownership transfers. Independently verify termination with `dumpsys activity services org.deskharbor.probe`, `dumpsys media_projection`, notification visibility and a stable event/PNG count after waiting. A JSON terminal event alone cannot establish OS resource release. For abrupt OS process death, no final JSON callback is guaranteed: use OS evidence.

The [Android media projection documentation](https://developer.android.com/media/grow/media-projection) describes consent, foreground service type, single-use sessions and projection stop callbacks. This probe respects protected surfaces; black/redacted secure content is expected. Android 14+ may share one app or the display and may resize the capture; the callback updates the reader surface on the existing virtual display. Android 15+ may stop projection on device lock. These paths require real runtime tests.

## Optional input proof

Open the probe's **Android Accessibility Settings** button, select **DeskHarbor P0 local input probe**, and explicitly enable it through Android. No setting is written or permission silently granted by the app or build script. The service is protected with `BIND_ACCESSIBILITY_SERVICE`, declares `canPerformGestures=true` and `canRetrieveWindowContent=false`, and ignores accessibility event contents. See [Android's accessibility service configuration](https://developer.android.com/guide/topics/ui/accessibility/service).

While capture is active, return to the focused probe and press **Dispatch one tap to local target**. Only its visible target center on the default display is eligible; external displays and multiwindow mode are refused. The visible rectangle and center use target-local coordinates, then the target origin is translated to screen coordinates, and the gesture explicitly names the default display. Require an OS gesture completion plus target hit event, not merely the `dispatchGesture` boolean. Without user-enabled accessibility, after permission revocation, before consent, or after stop, input must be denied. There is no receiver, IPC or network surface accepting arbitrary coordinates. This demonstrates a narrow attended gesture path, not complete remote keyboard/multitouch/multidisplay support. Disable the service in Settings after testing.

## Isolated emulator and physical lab procedure

Use only a newly created task-private writable AVD; share the installed system-image files read-only. Never launch/wipe the user's existing AVD and never issue untargeted `adb install`, `adb shell`, `adb reboot`, or broad `adb kill-server`. On this host the private AVD was created at `C:/Users/govin/Documents/Codex/2026-11-08/de/work/deskharbor/android/avd/DeskHarborP0.avd`, with `ANDROID_AVD_HOME` pointing to its parent, and was launched on dedicated emulator port 5580. Existing `Medium_Phone_API_37.0` was not changed.

After the task-owned device is actually online:

```powershell
$adb = 'C:/Users/govin/AppData/Local/Android/Sdk/platform-tools/adb.exe'
$serial = 'emulator-5580' # Verify this is the private DeskHarborP0 AVD first.
& $adb -s $serial shell getprop ro.boot.qemu.avd_name
& $adb -s $serial install -r ./native/android/probe/artifacts/deskharbor-android-probe-debug.apk
& $adb -s $serial shell am start -n org.deskharbor.probe/.ProbeActivity
# Interact with Android consent/settings on this disposable emulator only.
& $adb -s $serial shell run-as org.deskharbor.probe cat files/state.json
& $adb -s $serial shell run-as org.deskharbor.probe cat files/proof.jsonl
& $adb -s $serial shell dumpsys media_projection
& $adb -s $serial shell dumpsys activity services org.deskharbor.probe
# Shut down only the task-owned emulator after evidence collection.
& $adb -s $serial emu kill
```

Use `adb exec-out run-as org.deskharbor.probe cat files/<exact frame_saved filename>` with a binary-safe writer to export a PNG. Verify its SHA-256 and dimensions against `frame_saved`, and visually inspect known red/green/blue and text fixtures. A screenshot produced by `adb screencap` is not evidence that the probe's MediaProjection worked.

Record each of these individually on Android 11+ ARM64 physical hardware, with exact model/OS/build and raw OS evidence. Emulator evidence must be labeled API/ABI/emulator and cannot satisfy the physical gate.

| Required runtime case | Current qualification |
| --- | --- |
| Consent decline; no foreground/projection resources | not_run |
| Consent accept; actual PNG and hash/color/dimension inspection | not_run |
| Visible foreground notification and continued capture ownership after GUI backgrounding | not_run |
| Explicit app stop and notification stop; all OS resources absent | not_run |
| System projection-chip stop / consent revocation callback; no further frames/input | not_run |
| New session prompts again; stale token/session cannot resume | not_run |
| Optional accessibility disabled/enabled; tap arrives only after explicit enablement | not_run |
| Accessibility revocation; input denial while capture remains attended | not_run |
| Device lock and task removal; cleanup, no silent restart | not_run |
| Process death; OS cleanup, fresh consent required next launch | not_run |
| Protected screen remains protected; no bypass | not_run |
| Rotation/app sharing resize; valid capture and cleanup | not_run |
| Physical Android 11 ARM64 and supported modern ARM64 OS coverage | not_run |

## This host's result

The APK and all **11 JVM tests** built successfully using SDK 35/build-tools 35.0.0, Gradle 8.9, AGP 8.7.3, Kotlin 2.0.0 and cached JDK 21. A repeat incremental build preserved the APK hash. Kotlin compilation is configured in-process with two Gradle workers after daemon connection retries slowed an earlier build. `native/android/probe/evidence/2026-11-08-host.json` records the build/test result and exact artifact hash.

WHPX acceleration was reported usable. The first cold boot stayed adb-offline. The second cold boot of the isolated API 37.0 Google Play x86_64 image, with kernel logging, eventually became usable after initial adbd restarts. The initial APK was installed only on verified `DeskHarborP0`. System consent decline, input denial without accessibility, accepted MediaProjection with a genuine native PNG, active/background foreground service and explicit-stop OS cleanup were exercised. The PNG's hash matched its log and its rendered app contents were visually inspected; dedicated RGB fixtures remain unrun. `artifacts/emulator-2026-11-08` retains this pre-review APK (`e19a298fa0cbec56b6e36264c3b053144c4d571e79cedfcd3082ab6856910296`), PNG and OS logs, so this run is not misattributed to later fixes. Settings ANR prevented explicit accessibility enable/revoke testing. Final build/runtime results are recorded separately in the host evidence JSON. Raw boot diagnostics remain in task-private `work/deskharbor/android`. Android P0 remains blocked on physical ARM64 and the remaining runtime cases.

The final fixed APK is SHA-256 `95cf4652726754dfe8ee89a3232c82f9bc62927ea1bd9c56a3bba6ba2a977a13`. Its separate `artifacts/emulator-final-2026-11-08` evidence records consent denial; input denial with accessibility disabled; accepted native 720×1280 PNG capture (SHA-256 `f9ec57716a30c3f71b6867896bb043a63ebd94ae9dd849af455df09c3bef0365`, matched after export and visually inspected); foreground service while backgrounded; app Stop; a fresh consent prompt for another session; a visible foreground notification and its Stop action. Both stops recorded zero cleanup errors, with OS projection `null` and no active probe projection service. After the notification stop, sequence 16 remained stable over the recorded three-second observation.

Screen-off on this credential-free emulator **left projection active**. It is not an actual secure-lock or system-revocation proof. The projection was then stopped through its visible notification. `MediaProjection.Callback.onStop` cleanup is implemented, but actual lock/revocation callback, task removal, process death, protected surfaces, resize and explicit accessibility enable/revoke remain `not_run`; the physical matrix above also remains `not_run`. The task-owned AVD was verified by name, shut down with serial-targeted `emu kill`, and its emulator processes exited. No existing AVD or unrelated phone was used.
