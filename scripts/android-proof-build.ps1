[CmdletBinding()]
param(
    [string]$Sdk = $env:ANDROID_HOME,
    [string]$JavaHome = $env:JAVA_HOME,
    [switch]$Offline
)
$ErrorActionPreference = 'Stop'
if (-not $Sdk -or -not (Test-Path -LiteralPath "$Sdk/platforms/android-35/android.jar")) { throw 'Pass -Sdk with Android SDK platform 35 and build-tools 35.0.0 installed.' }
if (-not $JavaHome -or -not (Test-Path -LiteralPath "$JavaHome/bin/java.exe")) { throw 'Pass -JavaHome with a JDK 17 or 21 installation. Gradle 8.9 does not support JDK 25.' }
$probeRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../native/android/probe'))
$env:ANDROID_HOME = $Sdk
$env:JAVA_HOME = $JavaHome
$arguments = @('--no-daemon', "-Pandroid.aapt2FromMavenOverride=$Sdk/build-tools/35.0.0/aapt2.exe", ':app:testDebugUnitTest', ':app:assembleDebug')
if ($Offline) { $arguments += '--offline' }
Push-Location -LiteralPath $probeRoot
try {
    & "$probeRoot/gradlew.bat" @arguments
    if ($LASTEXITCODE -ne 0) { throw "Android build or tests failed: exit $LASTEXITCODE" }
    New-Item -ItemType Directory -Force -Path "$probeRoot/artifacts" | Out-Null
    Copy-Item -LiteralPath "$probeRoot/app/build/outputs/apk/debug/app-debug.apk" -Destination "$probeRoot/artifacts/deskharbor-android-probe-debug.apk" -Force
    Get-FileHash -Algorithm SHA256 -LiteralPath "$probeRoot/artifacts/deskharbor-android-probe-debug.apk"
} finally { Pop-Location }
