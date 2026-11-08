# Windows P0 native probe and isolated-lab procedure

The Windows backend implements real Win32 inspection and GDI capture of a selected current-desktop monitor. The separate SCM harness is real compiled source for LocalSystem desktop workers, but has **not been installed or executed as a service on this workstation**. Neither API success, compilation nor a user-desktop BMP passes the approved unattended-host gate. Windows input, SAS, transition correctness and all installed-helper proofs remain unproven.

## Current-process probe

From the repository in PowerShell:

```powershell
./scripts/cargo-msvc.ps1 test -p dh-platform
./scripts/cargo-msvc.ps1 build -p dh-platform-probe
New-Item -ItemType Directory -Force artifacts/windows-local | Out-Null
./target/debug/dh-platform-probe.exe inspect > artifacts/windows-local/inspection.json
# Copy the exact display id from inspection.json, including its backslashes.
./target/debug/dh-platform-probe.exe capture --display '\\.\DISPLAY1' --output artifacts/windows-local/display1.bmp
```

Do not upload these BMPs; they can contain the user's desktop. Capture refuses an existing output file. Its metadata reports actual dimensions and file size. Negative virtual monitor origins are preserved; a temporary per-monitor-v2 thread DPI context is restored on return. Each frame has top-down RGBA rows and opaque alpha. Capture checks the console/desktop/layout observation before and after copying pixels and discards a detected transition. The two observations cannot detect every rapid A→B→A transition and are not a session-epoch mechanism.

Inspection reads token elevation, process session, active console session, process window station, thread desktop, readable input desktop and physical monitor bounds. Failed queries have failed checks; they are not interpreted as proof of lock/sign-in. `UserDesktop` requires matching active-console session, `WinSta0`, thread `Default` and input `Default`. A readable input `Winlogon` in the active console maps to `locked_or_sign_in`; it does not distinguish lock, UAC or sign-in. An inaccessible desktop remains unknown. A missing active console maps to `no_console`.

GDI capture uses the screen DC, a compatible memory DC, a top-down 32-bit DIB section, `BitBlt(SRCCOPY | CAPTUREBLT)` and `GdiFlush` before copying the DIB bytes. Guards restore selected objects, delete the bitmap/DC and release the screen DC on success and every error. Edges are limited to 16384 pixels and one frame to 256 MiB before native allocation. GDI may omit cursor, protected video and hardware/direct-scanout planes. Overlay completeness, mixed-DPI/multi-adapter behavior, rotation and pixel content need separate physical checks. [Microsoft documents DIB synchronization and lifetime](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-createdibsection), [input-desktop access](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openinputdesktop), and [thread DPI restoration](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setthreaddpiawarenesscontext).

## Real virtual sign-in is valid; the VM is not yet available

A real isolated Windows 11 VM can establish the Windows P0 boot, sign-in, lock, logout, UAC and SAS lifecycle proofs. It runs the actual Windows logon/secure-desktop implementation and the installed native service; a mock screen or emulated report cannot substitute. Physical Windows hardware is not a prerequisite for those lifecycle proofs. Virtual results must identify the hypervisor, guest edition/build and virtual display; physical GPU/overlay, performance and final hardware qualification remain separate coverage.

`scripts/windows-vm-lab.ps1` supplies **read-only** `Preflight` (the default) and `Plan` JSON. Run it in an ordinary non-elevated PowerShell terminal. It never imports/discovers a Hyper-V module via `PSModulePath`, creates a VM/switch/disk/directory, starts a VM, elevates, downloads media, changes host features/policy or reboots anything. It checks only the fixed System32 Hyper-V module-manifest location, host memory/token/hypervisor state, a supplied ISO's metadata and workspace-volume space. Existing VM/switch inventory and protected VM storage are explicitly unverified. There is no `Create` mode and no privileged execution path in the checkout.

```powershell
# From an ordinary PowerShell 7 shell; do not run this workspace helper elevated.
./scripts/windows-vm-lab.ps1 -WorkspaceRoot 'C:\Users\govin\Documents\Codex\2026-11-08\de'
./scripts/windows-vm-lab.ps1 -Mode Plan -WorkspaceRoot 'C:\Users\govin\Documents\Codex\2026-11-08\de' -IsoPath 'D:\LabMedia\Windows11-x64.iso'
./scripts/windows-vm-lab.tests.ps1
```

The ISO path is an example; the operator must provide an existing legitimate Windows 11 x64 installation ISO. The helper does not fetch it. A blocked/read-only report exits 2 and explicitly reports zero actions and zero P0 proofs. `ready` describes this limited observation only; VM creation and sign-in are never marked performed. The tracked `docs/windows-vm-preflight.json` contains the actual host observation, with the generated chat workspace as `WorkspaceRoot`, rather than the source checkout. Existing Windows PowerShell 5.1 execution policy blocks local `-File` invocation on this host; it was left unchanged. The tests ran through the available PowerShell 7 runtime, and the Windows PowerShell 5.1 parser was checked separately.

To provision the lab, an authorized administrator should use the trusted installed **Hyper-V Manager**, outside this checkout:

1. First verify Hyper-V management is already available, a legitimate Windows 11 ISO exists, and at least **8 GiB RAM is currently free**: 4 GiB for the guest and 4 GiB for host headroom. Check at least **80 GiB free on the actual protected VM-storage volume**. Workspace free space does not prove storage readiness. No other workload is stopped by this helper, and host feature installation is outside its scope.
2. Verify `DeskHarborP0VM` and `DeskHarborP0Private` are unused. If either exists, stop this setup and inspect it; do not alter/reuse an unrelated object. Create a new **Private** virtual switch with no external adapter, NAT or host uplink.
3. In Hyper-V settings, select administrator-managed protected default storage for this new VM's configuration and disk. Verify its owner/DACL excludes untrusted writers. **Never put a privileged VM's configuration or VHDX in this user-owned workspace**, including its `work` directory. A path check cannot prevent the workspace owner replacing an ancestor with a junction after a privileged check.
4. Create only the new `DeskHarborP0VM`, Generation 2, with **4 GiB fixed memory**, two virtual processors, a **64 GiB dynamically expanding VHDX** in that protected storage and the new private switch. Disable automatic startup. With the new VM off, enable **Secure Boot / Microsoft Windows** and **Trusted Platform Module** in its Security settings. Attach the supplied ISO to its virtual DVD drive. [Microsoft documents Generation 2 VM creation](https://learn.microsoft.com/en-us/powershell/module/hyper-v/new-vm?view=windowsserver2025-ps), [private switches](https://learn.microsoft.com/en-us/powershell/module/hyper-v/new-vmswitch?view=windowsserver2025-ps), and [virtual TPM](https://learn.microsoft.com/en-us/powershell/module/hyper-v/enable-vmtpm?view=windowsserver2025-ps).
5. Start **that guest only** in Hyper-V Manager and install Windows using legitimate media/licensing and isolated lab accounts. Guest installation/OOBE and any required offline-compatible provisioning remain operator work; this plan does not silently attach Internet connectivity or bypass setup/security requirements. Take a clean guest snapshot before service experiments. None of these steps reboots or signs out the development host.
6. Inside the guest, follow the signed binary, private file/service ACL and service setup below. Then boot the guest to Windows sign-in, hold each target state, validate actual pixels/input/SAS independently and retain the required evidence/attestations. Use guest restarts and guest sign-in/out for these proofs. Installing Windows, running Hyper-V or capturing an ordinary signed-in desktop alone passes no lifecycle proof.

This procedure is concrete VM preparation, **not a virtual sign-in result**. The current blockers are retained in the JSON. Actual VM creation, guest installation, signed service execution and all guest sign-in/UAC/SAS/50-cycle observations remain `not_run`.

## Compiled service/worker contract

These CLI entrypoints do not install or start a service:

```text
dh-platform-probe.exe windows-service
dh-platform-probe.exe windows-worker SESSION Default|Winlogon
dh-platform-probe.exe windows-input-worker SESSION Default|Winlogon
```

`windows-service` enters `StartServiceCtrlDispatcherW` for service name `DeskHarborP0Proof`. Direct execution outside SCM fails. The service checks LocalSystem identity, receives stop/session-change controls, queries the active console, and launches only the fixed worker command from `C:\Program Files\DeskHarbor\P0\dh-platform-probe.exe`. It duplicates its own primary token, assigns the active console session with `SetTokenInformation(TokenSessionId)`, and uses `CreateProcessAsUserW` with `lpDesktop` set to `WinSta0\Default` or `WinSta0\Winlogon`. No shell, arbitrary executable/command, caller-chosen output directory, token from another user, network listener or inherited handle is used. Missing token privileges, access restrictions or job-object constraints fail visibly rather than falling back.

Workers start suspended, enter an unnamed job with `KILL_ON_JOB_CLOSE`, then resume. They have a ten-second supervised timeout; service stop terminates only its own worker, and service exit closes the job. This is a privileged laboratory proof harness, **not a production broker or restricted worker security design**. It does not implement authenticated controller IPC, session epochs, clipboard/files, general input delivery or production privilege reduction. Explicit custom SCM controls exercise only a fixed F24 pair and policy-gated native SAS, as described below. Neither is invoked at startup or during periodic capture.

`windows-worker` refuses identities other than LocalSystem. It validates requested/actual session, thread desktop and current input desktop before attempting capture. It writes inspection and capture outcome locally; a captured frame is marked `captured_unvalidated`, never `passed`. An inactive worker desktop, inaccessible desktop, failed capture or missing display exits unsuccessfully and is retained as an observation. The current harness captures only the primary/first monitor per sample. An installed worker's inspection still lists mandatory P0 checks as `not_run` because a single sample cannot establish transitions or input.

Output is fixed to `C:\ProgramData\DeskHarbor\P0Proof`. Both that directory and its `DeskHarbor` parent must be pre-provisioned with a protected DACL containing exactly full access for SYSTEM and Builtin Administrators, with a SYSTEM or Administrators owner. The fixed installation directories and executable require the same ACL. Reparse-point install/output objects, mismatched ACLs and untrusted owners are rejected. The service does not repair preexisting folders. Every evidence file uses the explicit private descriptor and `CREATE_NEW`. Output is bounded to 512 MiB/4096 files; dispatch is bounded to 1000 rounds. Reaching a limit stops evidence collection with an error. Archive locally and remove only lab-owned evidence between runs. Service events are sampled on session notifications and at five-second intervals; short UAC prompts may be missed.

## Isolated Windows 11 Home/Pro VM setup — not run here

Use a disposable VM snapshot with an unlocked OS disk, active connected display and local test accounts. Keep the existing secure-desktop/UAC/SAS policies and record them; do not weaken them to make a test pass. Keep the VM off external networks. Review and sign the binary using the lab identity and retain certificate/build/OS/device metadata. This harness has not been signed or installed by this change.

Provision the directories and executable as the VM administrator. The following uses **Windows PowerShell 5.1 (`powershell.exe`)** because its .NET Framework overload creates directories with an initial security descriptor. Do not run these commands on the development workstation. Replace only `$labBuiltExe` with the reviewed binary source path; the installed destination remains fixed.

```powershell
$labBuiltExe = 'D:\reviewed-build\dh-platform-probe.exe'
$privateSddl = 'O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)'
$directoryAcl = New-Object System.Security.AccessControl.DirectorySecurity
$directoryAcl.SetSecurityDescriptorSddlForm($privateSddl)
$labDirectories = @(
  'C:\Program Files\DeskHarbor',
  'C:\Program Files\DeskHarbor\P0',
  'C:\ProgramData\DeskHarbor',
  'C:\ProgramData\DeskHarbor\P0Proof'
)
foreach ($labDirectory in $labDirectories) {
  if (Test-Path -LiteralPath $labDirectory) { throw "Review existing directory first: $labDirectory" }
  [System.IO.Directory]::CreateDirectory($labDirectory, $directoryAcl) | Out-Null
}
Copy-Item -LiteralPath $labBuiltExe -Destination 'C:\Program Files\DeskHarbor\P0\dh-platform-probe.exe'
$fileAcl = New-Object System.Security.AccessControl.FileSecurity
$fileAcl.SetSecurityDescriptorSddlForm($privateSddl)
Set-Acl -LiteralPath 'C:\Program Files\DeskHarbor\P0\dh-platform-probe.exe' -AclObject $fileAcl
Get-AuthenticodeSignature -LiteralPath 'C:\Program Files\DeskHarbor\P0\dh-platform-probe.exe'
Get-FileHash -Algorithm SHA256 -LiteralPath 'C:\Program Files\DeskHarbor\P0\dh-platform-probe.exe'
New-Service -Name DeskHarborP0Proof -BinaryPathName '"C:\Program Files\DeskHarbor\P0\dh-platform-probe.exe" windows-service' -StartupType Manual
# This protects the SCM service object, separately from every file/directory ACL.
# Do this BEFORE starting the service. Default service ACLs are insufficient.
sc.exe sdset DeskHarborP0Proof 'O:BAG:BAD:P(A;;GA;;;SY)(A;;GA;;;BA)'
if ($LASTEXITCODE -ne 0) { throw 'Could not restrict the service control ACL' }
sc.exe sdshow DeskHarborP0Proof
```

Provision while the disposable VM is exclusively controlled by the administrator, with no untrusted session or process running. **Fresh-boot the VM after applying the restricted service DACL and before starting the service or permitting a standard-user session.** Tightening a DACL does not revoke service handles already opened under the old default DACL; runtime descriptor checks cannot identify or revoke those handles. This is another reason this lab harness is not a production authorization boundary. Do not execute input tests on a shared machine.

After that fresh boot, in the VM administrator's new PowerShell session:

```powershell
Start-Service -Name DeskHarborP0Proof
Get-Service -Name DeskHarborP0Proof
```

Directory setup intentionally refuses existing targets. If setup stops halfway, inspect the lab snapshot rather than rerunning blindly. Review each `service.json`, `worker.json` and `service-error.json` in the protected output. Worker failure on the inactive desktop is expected; a secure-desktop capture failure is a real failed proof, not permission success. A zero service dispatch exit or an SCM `Running` status is not a passed screenshot/input proof.

Once an administrator verifies the manual service in the VM, configure automatic startup **inside that VM only** for the boot/sign-in test:

```powershell
Set-Service -Name DeskHarborP0Proof -StartupType Automatic
# Operator-controlled isolated-VM reboot; never execute on the development host.
Restart-Computer
```

Use the VM console to hold each target state for at least ten seconds. Independently validate the BMP's expected visible content against the console; retain timings, build hash, session and desktop metadata. Required states are boot before first user sign-in, sign-in, locked, unlocked, logout back to sign-in, second user/session switch, and UAC on the secure desktop. Open and cancel an ordinary UAC prompt manually in the lab; do not change consent/secure-desktop policy. General held-input release and session-epoch rejection remain unimplemented; the fixed F24 probe below cannot establish those requirements.

SAS remains an unproven gate item. The explicit service control below calls the real `SendSAS` service API only under an existing policy allowing services; no SAS policy modification is supplied. `SendInput` Ctrl+Alt+Delete is not used as SAS. Manually pressing SAS at the VM console may prepare a capture state but cannot demonstrate remote SAS support.

## Explicit input controls — isolated lab only, not executed here

The service refuses to report `Running` or enable custom input controls until it reads its SCM service object's owner and protected DACL. The only accepted allow ACEs give SYSTEM and Builtin Administrators full service access; any other principal, unprotected default DACL, missing owner, unexpected ACE or incomplete rights is refused. It rechecks this descriptor before each input/SAS action. File/directory ACLs do not authorize service controls. Do not rely on the default `New-Service` service object ACL: [Microsoft documents that local authenticated users receive user-defined service-control rights by default](https://learn.microsoft.com/en-us/windows/win32/services/service-security-and-access-rights). The runtime never changes this ACL itself.

As a **standard, non-administrator user inside the disposable VM**, test that each command below returns access denied (Win32 error 5), creates no input intent/result and causes no input or SAS. Record the user's token and exact errors. These commands must be rejected by SCM before delivery to the service:

```powershell
sc.exe control DeskHarborP0Proof 128
sc.exe control DeskHarborP0Proof 130
sc.exe control DeskHarborP0Proof 129
```

Only after that negative test, the VM administrator may opt into exactly one action at a time:

| SCM control | Explicit action |
|---|---|
| `128` | One F24 down/up pair on the active console's `WinSta0\Default` desktop only |
| `130` | One F24 down/up pair on the active console's `WinSta0\Winlogon` desktop only |
| `129` | One `SendSAS(FALSE)` call from the registered service, only if existing policy explicitly allows services |

For example, **inside the isolated VM only**, `sc.exe control DeskHarborP0Proof 130` requests secure-desktop F24. It never falls back to another desktop. A Default request cannot be retargeted to Winlogon. Hold the intended desktop stable, release modifiers/F24, and use a trusted instrumented lab receiver or independent observation of the intended OS response. F24 is deliberately unlikely to activate a normal command, but configured shortcuts can still respond; a lab administrator must choose a controlled test environment. A `SendInput` count of two reports only queued API events, not receipt or correct behavior.

Custom requests bind the active console session and expire after two seconds. Pending requests are cleared on session notifications and service stop. A worker gets an immutable expected desktop and a deadline through a private per-child environment; it rechecks active console/input desktop immediately before the fixed atomic two-event `SendInput` call, then records another observation. It refuses already-held modifiers/F24. If the API inserts only the first event, it attempts only the matching key-up and records whether that release was queued. It never retries key-down. Inactive desktop exit code 3 sends no input; no second desktop is tried. Requests queued while a capture worker is busy may expire; issue a fresh explicit control only after inspecting the recorded outcome.

These checks are observations, **not session epochs**: an A→B→A desktop transition between observations can escape detection. Session-notification clearing cannot revoke an API call already in progress, and this source does not prove held-input release across all desktop transitions. Those cases remain required lab failures/not-run until the production epoch/release design is implemented and qualified.

For control `129`, the service reads the existing 64-bit HKLM `Software\Microsoft\Windows\CurrentVersion\Policies\System\SoftwareSASGeneration` DWORD. Only values 1 (services) and 3 (services and accessibility applications) are accepted; absent, inaccessible, malformed, 0, 2 and unknown values deny the action. The mapping is defined by the installed Microsoft `WinLogon.admx` policy. The code duplicates its own SYSTEM token into the requested console session, impersonates that token for the single service `SendSAS(FALSE)` call, and immediately reverts. Failure to revert terminates the service process immediately, without further file or SCM work. No policy write occurs. If the intended default installation policy denies software SAS, this is a real blocked product requirement; do not change policy merely to obtain a passing result.

`SendSAS` returns no success acknowledgement. The JSON therefore records invocation only; independently observe and timestamp the correct console transition and later capture before assigning a passed proof. [Microsoft documents service identity, policy and session targeting for SendSAS](https://learn.microsoft.com/en-us/windows/win32/api/sas/nf-sas-sendsas) and [the event count, UIPI and existing-key-state limitations of SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput). Durable intent/result files are private local evidence, not proof certifications.

## Fifty-cycle and recovery record

Do not infer fifty login cycles from service sample count. For each of 50 actual VM cycles retain a record with cycle number, cold/warm boot as applicable, starting/ending session IDs, OS build/edition, binary and certificate hashes, sign-in/lock/unlock/logout timestamps, UAC/SAS results, expected/actual content, input acknowledgements/releases, worker/job cleanup, evidence filenames/hashes, and pass/fail/not-run. A cycle is incomplete while input/SAS remains unvalidated. Retain failures and original evidence; do not relabel missing rows as passed. Home and Pro and the physical reference hardware need their own coverage records.

For a failed test, stop the service in the VM, retain files and Event Viewer/SCM errors, and restore the snapshot. Explicit lab cleanup after evidence has been archived:

```powershell
Stop-Service -Name DeskHarborP0Proof
sc.exe delete DeskHarborP0Proof
# Inspect the exact fixed directories and their contents before manual removal.
Get-Item -LiteralPath 'C:\Program Files\DeskHarbor\P0', 'C:\ProgramData\DeskHarbor\P0Proof'
```

No policy restoration is needed from this harness because it never changes policy; record any independent lab changes and restore those separately. Do not remove directories owned by an existing product installation.

## Verification ledger

Windows fixture RED was observed before implementation: BGRA conversion returned `not_implemented`, active-console desktop classification returned `Unknown`, and secure-worker eligibility rejected a valid Winlogon fixture. The implemented pure fixtures cover independent BGR/RGB bytes, top-down row order, opaque alpha, malformed/bounded dimensions, active-console/window-station/input desktop mismatch and guarded secure-worker observations. Unit tests and Clippy run on the local Windows toolchain; exact commands and native metadata are retained by the repository's P0 evidence ledger.

Not run by this task: installed service, LocalSystem worker launch, service crash/stop job cleanup, secure-desktop pixel validation, boot/sign-in/lock/unlock/logout/UAC/SAS input, second-user transition, Windows Home/Pro matrix, physical multi-monitor/DPI/overlay completeness, fifty actual login cycles and soak. The P0 gate must remain blocked for these cases. The source-only harness makes lab execution concrete; it does not certify Windows unattended hosting.
