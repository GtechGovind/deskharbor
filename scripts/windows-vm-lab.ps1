<# Read-only Preflight/Plan only. Run in an ordinary, non-elevated shell.
No VM creation, Hyper-V module import/autoload, host feature edits, elevation,
downloads, boot, ISO mount, existing-object changes, removal or rollback.
Provision only through Hyper-V Manager with administrator-managed storage. #>
[CmdletBinding()]
param([ValidateSet('Preflight', 'Plan')][string]$Mode = 'Preflight', [string]$WorkspaceRoot, [string]$IsoPath)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$vmName = 'DeskHarborP0VM'
$switchName = 'DeskHarborP0Private'
$blockers = [System.Collections.Generic.List[object]]::new()
$labRoot = $null
$iso = $null
$memoryFree = $null
$diskFree = $null
$admin = $false
$hypervisor = $null
function Add-Blocker([string]$Code, [string]$Detail) { $blockers.Add([ordered]@{ code = $Code; detail = $Detail }) }
function Assert-NoReparseAncestors([string]$Path) {
    $cursor = [IO.Path]::GetFullPath($Path)
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw "VM path has a non-directory or reparse ancestor: $cursor" }
        }
        $parent = [IO.Path]::GetDirectoryName($cursor)
        if ($parent -eq $cursor) { break }
        $cursor = $parent
    }
}
try {
    if (-not $WorkspaceRoot) { Add-Blocker 'workspace_required' 'Supply an existing absolute -WorkspaceRoot for read-only workspace disk inspection. VM disks/configuration must never be stored in this checkout.' }
    elseif ($WorkspaceRoot -notmatch '^[A-Za-z]:[\\/]' -or $WorkspaceRoot.Substring(2).Contains(':')) { throw 'WorkspaceRoot must be an absolute local drive path; UNC/device/alternate-stream paths are refused.' }
    else {
        $workspace = [IO.Path]::GetFullPath($WorkspaceRoot).TrimEnd('\', '/')
        if ($workspace.Length -le 3 -or -not (Test-Path -LiteralPath $workspace -PathType Container)) { throw 'WorkspaceRoot must be an existing directory below a local drive root.' }
        Assert-NoReparseAncestors $workspace
        $labRoot = [IO.Path]::GetFullPath((Join-Path $workspace 'work\deskharbor\windows-vm-lab'))
        $allowed = [IO.Path]::GetFullPath((Join-Path $workspace 'work\deskharbor')).TrimEnd('\', '/') + '\'
        if (-not $labRoot.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) { throw 'Resolved path escaped the fixed work\deskharbor child.' }
        Assert-NoReparseAncestors $labRoot
        $driveRoot = [IO.Path]::GetPathRoot($labRoot)
        $drive = Get-CimInstance Win32_LogicalDisk -Filter ("DeviceID='{0}'" -f $driveRoot.Substring(0, 2))
        if (-not $drive -or $drive.DriveType -ne 3) { throw 'VM storage must be a fixed local disk.' }
        $diskFree = [uint64]$drive.FreeSpace
        if ($diskFree -lt 80GB) { Add-Blocker 'workspace_disk_space_low' 'Workspace volume has less than 80 GiB free. Actual administrator-managed VM storage must be checked separately.' }
    }
} catch { Add-Blocker 'workspace_or_disk_query_failed' $_.Exception.Message }
if (-not $IsoPath) { Add-Blocker 'windows_iso_required' 'Provide an existing, legitimately obtained Windows 11 x64 ISO with -IsoPath. No download is performed.' }
else {
    try {
        if ($IsoPath -notmatch '^[A-Za-z]:[\\/]' -or $IsoPath.Substring(2).Contains(':')) { throw 'ISO must be an absolute local path without UNC/device/alternate streams.' }
        $file = Get-Item -LiteralPath $IsoPath -Force
        if ($file.PSIsContainer -or $file.Extension -ine '.iso' -or $file.Length -lt 1GB -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Expected a regular existing .iso file >=1 GiB. Windows image identity and licensing must be verified by the operator.' }
        Assert-NoReparseAncestors $file.DirectoryName
        $iso = $file.FullName
    } catch { Add-Blocker 'windows_iso_invalid' $_.Exception.Message }
}
try {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    $admin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    $identity.Dispose()
    if (-not $admin) { Add-Blocker 'administrator_for_manual_setup_required' 'An authorized administrator must provision the VM in Hyper-V Manager. Keep this checkout helper non-elevated; it never requests elevation.' }
} catch { Add-Blocker 'token_query_failed' $_.Exception.Message }
try {
    $os = Get-CimInstance Win32_OperatingSystem
    $memoryFree = [uint64]$os.FreePhysicalMemory * 1KB
    if ($memoryFree -lt 8GB) { Add-Blocker 'memory_low' 'Require 8 GiB currently free RAM: 4 GiB guest plus 4 GiB host headroom. Other workloads are not stopped automatically.' }
    $computer = Get-CimInstance Win32_ComputerSystem
    $hypervisor = [bool]$computer.HypervisorPresent
    if (-not $hypervisor) { Add-Blocker 'hypervisor_not_present' 'No active hypervisor was reported; host features are never enabled by this script.' }
    if ($computer.NumberOfLogicalProcessors -lt 2) { Add-Blocker 'cpu_insufficient' 'The guest requires two virtual processors.' }
} catch { Add-Blocker 'host_query_failed' $_.Exception.Message }
$trustedManifest = Join-Path ([Environment]::SystemDirectory) 'WindowsPowerShell\v1.0\Modules\Hyper-V\Hyper-V.psd1'
$manifestPresent = Test-Path -LiteralPath $trustedManifest -PathType Leaf
if (-not $manifestPresent) { Add-Blocker 'hyperv_system_module_missing' 'The fixed Windows System32 Hyper-V module manifest is absent. A running hypervisor alone does not provide a Windows VM management environment.' }
# Never discover commands through PSModulePath or import a user-controlled module.
Add-Blocker 'vm_inventory_unverified' 'No Hyper-V module is loaded and no VM/switch query is executed. An administrator must verify DeskHarborP0VM and DeskHarborP0Private are unused in Hyper-V Manager.'
Add-Blocker 'protected_vm_storage_unverified' 'Administrator-owned VM storage and its free space/ACL must be verified in Hyper-V Manager; workspace write permissions are not sufficient.'
$plan = [ordered]@{
    vm_name = $vmName; switch_name = $switchName; switch_type = 'Private'; generation = 2
    secure_boot = $true; secure_boot_template = 'MicrosoftWindows'; vtpm = $true
    guest_memory_bytes = 4GB; processor_count = 2; dynamic_disk_bytes = 64GB
    required_free_ram_bytes = 8GB; required_free_disk_bytes = 80GB
    workspace_review_notes_only = $labRoot; vm_storage = 'Hyper-V administrator-managed protected default storage; never the workspace'
    iso_path = $iso; start_vm = $false; automatic_start = 'Nothing'
    host_feature_changes = $false; network_uplink = $false; downloads = $false
    steps = @('Administrator opens trusted Hyper-V Manager outside this helper', 'Verify fixed VM/switch names unused; leave existing objects untouched', 'Verify administrator-managed protected default VM storage and 80 GiB free', 'Create a new Private switch with no external adapter', 'Create a new Generation 2 VM with a dynamic 64 GiB VHDX in that protected storage', 'Set fixed 4 GiB memory / two processors and disable automatic startup', 'Enable Microsoft Windows Secure Boot and vTPM in VM settings', 'Attach provided Windows 11 ISO to guest DVD', 'Guest installation, boot, service provisioning and sign-in testing remain separate operator actions and not_run')
}
[ordered]@{
    schema_version = 1; observed_at_utc = [DateTime]::UtcNow.ToString('o'); mode = $Mode; ready = ($blockers.Count -eq 0)
    readiness_scope = 'read_only_host_observation_and_manual_vm_plan'; actions_executed = 0
    creation_status = 'unsupported_by_this_read_only_helper'; p0_proofs_passed = 0
    guest_os_installed = $false; guest_boot_or_signin_tested = $false
    host = [ordered]@{ administrator = $admin; hypervisor_present = $hypervisor; free_ram_bytes = $memoryFree; workspace_volume_free_bytes = $diskFree; trusted_hyperv_manifest = $trustedManifest; hyperv_system_manifest_present = $manifestPresent; hyperv_cmdlets_imported = $false; vm_inventory_queried = $false }
    blockers = @($blockers.ToArray()); plan = $plan; created_resources = @()
} | ConvertTo-Json -Depth 8
if ($blockers.Count) { exit 2 }
exit 0
