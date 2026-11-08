$cargoArguments = $args
$ErrorActionPreference = 'Stop'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) { throw 'Install Visual Studio C++ build tools (vswhere.exe missing).' }
$installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $installation) { throw 'Visual Studio x64 C++ tools were not found.' }
$vcTools = (Get-ChildItem -LiteralPath (Join-Path $installation 'VC\Tools\MSVC') -Directory | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1).FullName
$sdkRoot = (Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots').KitsRoot10
$sdkVersion = (Get-ChildItem -LiteralPath (Join-Path $sdkRoot 'Lib') -Directory | Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'um\x64\kernel32.lib') } | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1).Name
if (-not $vcTools -or -not $sdkVersion) { throw 'MSVC or Windows SDK x64 libraries were not found.' }
$env:PATH = (Join-Path $vcTools 'bin\Hostx64\x64') + ';' + $env:PATH
$env:LIB = (Join-Path $vcTools 'lib\x64') + ';' + (Join-Path $sdkRoot "Lib\$sdkVersion\um\x64") + ';' + (Join-Path $sdkRoot "Lib\$sdkVersion\ucrt\x64")
& cargo @cargoArguments
exit $LASTEXITCODE
