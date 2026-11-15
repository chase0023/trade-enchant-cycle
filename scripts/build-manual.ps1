# Space-free staging fallback for Windows profiles whose %APPDATA% path
# contains a space. Horizon's `crates/horizon-client/build.rs` passes
# -I<CARGO_MANIFEST_DIR> to windres; MinGW splits it at spaces and the build
# fails with "windres: preprocessing failed". Staging and building under a
# space-free path avoids it.
#
# This script does NOT install the result. README section 7.4 has the copy
# commands that put the built exe into a version folder.
#
#   powershell -ExecutionPolicy Bypass -File .\scripts\build-manual.ps1
#   powershell -ExecutionPolicy Bypass -File .\scripts\build-manual.ps1 -Tests
[CmdletBinding()]
param(
    [string]$DataRoot = (Join-Path $env:APPDATA "horizon-launcher"),
    [string]$Stage = "C:\tec-build\stage",
    [string]$TargetDir = "C:\tec-build\target",
    [switch]$Tests,
    [switch]$NoOffline
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$bundle = Join-Path $root "mixins/trade_enchant_cycle"

foreach ($path in @($Stage, $TargetDir)) {
    if ($path -match '\s') {
        throw "$path contains a space. Choose a path such as C:\tec-build\stage (the point of this script)."
    }
}
if (-not (Test-Path $DataRoot)) { throw "Horizon launcher data not found at $DataRoot" }
if (-not (Test-Path (Join-Path $bundle "horizon-mixin.toml"))) {
    throw "Missing bundle manifest: $bundle\horizon-mixin.toml"
}

$manifest = Get-Content (Join-Path $bundle "horizon-mixin.toml") -Raw
$pinMatch = [regex]::Match($manifest, 'horizon_revision\s*=\s*"([0-9a-f]{40})"')
if (-not $pinMatch.Success) { throw "Could not read a 40-hex horizon_revision from horizon-mixin.toml" }
$pin = $pinMatch.Groups[1].Value
$source = Join-Path $DataRoot "native\source\$pin"
if (-not (Test-Path $source)) {
    throw @"
Horizon source $pin is not downloaded yet ($source).
Run the official build once first (scripts\build-native.ps1). It may fail at
windres, but it downloads the source, the Rust toolchain and horizon-mixins
before compiling. Then run this script again.
"@
}

$mixins = Get-ChildItem (Join-Path $DataRoot "native\tools") -Recurse -Filter "horizon-mixins.exe" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $mixins) { throw "horizon-mixins.exe was not found under $DataRoot\native\tools. Run the official build once to build it." }

$rust = Get-ChildItem (Join-Path $DataRoot "rust") -Directory -ErrorAction SilentlyContinue |
    Where-Object { Test-Path (Join-Path $_.FullName "bin\cargo.exe") } |
    Sort-Object Name -Descending | Select-Object -First 1
if (-not $rust) { throw "The launcher's Rust toolchain was not found under $DataRoot\rust. Run the official build once." }
$cargo = Join-Path $rust.FullName "bin\cargo.exe"
$mingw = Join-Path $rust.FullName "mingw64\bin"

Write-Host "Pinned Horizon source: $pin"
Write-Host "Staging to:            $Stage"
if (Test-Path $Stage) { Remove-Item $Stage -Recurse -Force }
New-Item (Split-Path $Stage -Parent) -ItemType Directory -Force | Out-Null

& $mixins.FullName apply --workspace $source --mods $bundle --out $Stage
if ($LASTEXITCODE -ne 0) { throw "horizon-mixins apply failed with exit code $LASTEXITCODE" }

$env:PATH = "$($rust.FullName)\bin;$mingw;$env:PATH"
$env:CARGO_HOME = Join-Path $DataRoot "cargo"
$env:CARGO_TARGET_DIR = $TargetDir

Push-Location $Stage
try {
    if ($Tests) {
        # The mod's game-side tests (the pure ones are `cargo test` in the
        # project root): they run the real offer generator.
        & $cargo test --locked --offline -p horizon-client --bin horizon-client tec_
        if ($LASTEXITCODE -ne 0) { throw "cargo test failed with exit code $LASTEXITCODE" }
    } else {
        $buildArgs = @("build", "--locked", "--profile", "release", "-p", "horizon-client", "--bin", "horizon-client")
        if (-not $NoOffline) { $buildArgs = @("build", "--locked", "--offline", "--profile", "release", "-p", "horizon-client", "--bin", "horizon-client") }
        & $cargo @buildArgs
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
    }
} finally {
    Pop-Location
}

if (-not $Tests) {
    $exe = Join-Path $TargetDir "release\horizon-client.exe"
    if (-not (Test-Path $exe)) { throw "Build finished but $exe is missing" }
    Write-Host ""
    Write-Host "Built: $exe"
    Write-Host "Install it with the commands in README section 7.4."
}
