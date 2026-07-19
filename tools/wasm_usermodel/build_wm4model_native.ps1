# build_wm4model_native.ps1 — build the wm4model NATIVE TWIN DLL on demand
# (WASM_USERMODELS_PLAN §2.6 plan B, WP-WM.4). The twin is the r4133-oracle side
# of the WM.4 gate: the SAME Rust model core (tools/wasm_usermodel/models/
# wm4model) compiled as a native cdylib the r4133 engine loads as a Storage
# `DynaDLL=` / PVSystem `UserModel=` DLL. NOT committed as a binary — built on
# demand for MANUAL golden generation (like the FPC IndMach012a twin).
#
# Usage:
#   pwsh tools/wasm_usermodel/build_wm4model_native.ps1
#   # then:
#   $env:WASM_TWIN_DLL = "<printed path>"
#   cargo test -p dss-epri --test gen_wasm_usermodels_wm4 -- --nocapture --ignored

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$Crate = Join-Path $RepoRoot "tools\wasm_usermodel\models\wm4model"
$OutDir = Join-Path $env:TEMP "wm4model_native_twin"
$Artifact = Join-Path $OutDir "wm4model.dll"

Push-Location $Crate
try {
    # Host target = x86_64-pc-windows-msvc (must match the 64-bit r4133 DLL).
    cargo +stable build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
}
finally {
    Pop-Location
}

New-Item -ItemType Directory -Force $OutDir | Out-Null
Copy-Item (Join-Path $Crate "target\release\wm4model.dll") $Artifact -Force

Write-Host "native twin: $Artifact"
Write-Host "sha256:      $((Get-FileHash -Algorithm SHA256 $Artifact).Hash.ToLower())"
Write-Host ""
Write-Host "Set WASM_TWIN_DLL to this path and run the WM.4 golden generator."
