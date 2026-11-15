# Build the native mod through the Horizon launcher.
#
#   powershell -ExecutionPolicy Bypass -File .\scripts\build-native.ps1
#   powershell -ExecutionPolicy Bypass -File .\scripts\build-native.ps1 -Launcher "C:\path\horizon-launcher.exe"
#
# The launcher applies mixins/trade_enchant_cycle/horizon-mixin.toml to Horizon's own source and
# compiles the game with it, then installs a `native-<commit>-<key>` version
# for the instance. Close the game first.
[CmdletBinding()]
param(
    [string]$Launcher,
    [string]$Bundle
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
if (-not $Bundle) { $Bundle = Join-Path $root "mixins/trade_enchant_cycle" }
if (-not (Test-Path (Join-Path $Bundle "horizon-mixin.toml"))) {
    throw "Missing bundle manifest: $(Join-Path $Bundle 'horizon-mixin.toml')"
}
if (-not $Launcher) {
    $cmd = Get-Command "horizon-launcher.exe" -ErrorAction SilentlyContinue
    if ($cmd) {
        $Launcher = $cmd.Source
    } else {
        $candidates = @(
            (Join-Path $env:LOCALAPPDATA "Programs\horizon-launcher\horizon-launcher.exe"),
            (Join-Path $env:USERPROFILE "Downloads\horizon-launcher-windows-x64\horizon-launcher.exe")
        )
        $Launcher = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    }
}
if (-not $Launcher -or -not (Test-Path $Launcher)) {
    throw @"
The Horizon launcher was not found.
Pass -Launcher "C:\path\to\horizon-launcher.exe", or set HORIZON_LAUNCHER.
The launcher must have Minecraft 26.3 prepared and the Nightly
nightly-2026-11-01-c9d474d installed (README section 4).
"@
}
Write-Host "Building $Bundle with $Launcher ..."
& $Launcher native-build $Bundle --trust-native-code
exit $LASTEXITCODE
