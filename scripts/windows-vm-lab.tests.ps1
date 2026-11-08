$ErrorActionPreference = 'Stop'
$scriptPath = Join-Path $PSScriptRoot 'windows-vm-lab.ps1'
$tokens = $null
$parseErrors = $null
$null = [System.Management.Automation.Language.Parser]::ParseFile($scriptPath, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count -ne 0) { throw ($parseErrors | Out-String) }
$workspace = Split-Path -Parent $PSScriptRoot
$expectedLabPath = Join-Path $workspace 'work\deskharbor\windows-vm-lab'
$existedBefore = Test-Path -LiteralPath $expectedLabPath
$engine = (Get-Process -Id $PID).Path
$defaultRaw = @(& $engine -NoProfile -NonInteractive -File $scriptPath) -join [Environment]::NewLine
$defaultExit = $LASTEXITCODE
$defaultResult = $defaultRaw | ConvertFrom-Json
if ($defaultExit -ne 2 -or $defaultResult.mode -ne 'Preflight' -or $defaultResult.actions_executed -ne 0) { throw 'No arguments must perform only blocked, read-only preflight' }
foreach ($mode in @('Preflight', 'Plan')) {
    $raw = @(& $engine -NoProfile -NonInteractive -File $scriptPath -Mode $mode -WorkspaceRoot $workspace) -join [Environment]::NewLine
    $exit = $LASTEXITCODE
    $result = $raw | ConvertFrom-Json
    if ($exit -ne 2 -or $result.ready -or $result.actions_executed -ne 0) { throw "$mode must fail closed without a supplied ISO and execute no creation actions" }
    if ($result.schema_version -ne 1 -or $result.p0_proofs_passed -ne 0) { throw 'Invalid preflight proof schema' }
    if ('windows_iso_required' -notin @($result.blockers.code)) { throw 'Missing ISO must be an explicit blocker' }
    if ($result.plan.vm_name -ne 'DeskHarborP0VM' -or $result.plan.switch_type -ne 'Private' -or $result.plan.generation -ne 2 -or $result.plan.start_vm) { throw 'Unexpected isolated VM plan' }
}
if ((Test-Path -LiteralPath $expectedLabPath) -ne $existedBefore) { throw 'Read-only modes changed the VM artifact directory' }
'PASS: PowerShell parse and read-only Preflight/Plan negative JSON; VM creation is unsupported.'
