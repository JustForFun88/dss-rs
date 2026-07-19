# build_capuserctl_native.ps1 — build the capuserctl NATIVE TWIN DLL on demand
# (WASM_USERMODELS_PLAN §2.6 plan B, WP-WM.5). The twin is built ONLY to
# empirically confirm the WM.5 round-2 finding — that a native TCapUserControl DLL
# CANNOT drive the r4133 control queue (it has no owner pointer). It is NOT the
# gate oracle (the r4133 built-in VOLTAGE control is — see
# tools/golden/wasm_decks/wasm_capcontrol_oracle.dss). NOT committed as a binary.
#
# Usage:
#   pwsh tools/wasm_usermodel/build_capuserctl_native.ps1
#   # then run the empirical confirmation:
#   $env:DSS_GEN_WM5 = "1"
#   $env:WASM_TWIN_DLL = "<printed path>"
#   cargo test -p dss-epri --test gen_wasm_usermodels_wm5 -- --nocapture --ignored

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$Crate = Join-Path $RepoRoot "tools\wasm_usermodel\models\capuserctl"
$OutDir = Join-Path $env:TEMP "capuserctl_native_twin"
$Artifact = Join-Path $OutDir "capuserctl.dll"

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
Copy-Item (Join-Path $Crate "target\release\capuserctl.dll") $Artifact -Force

Write-Host "native twin: $Artifact"
Write-Host "sha256:      $((Get-FileHash -Algorithm SHA256 $Artifact).Hash.ToLower())"
Write-Host ""
Write-Host "Set WASM_TWIN_DLL to this path and run the WM.5 golden generator with DSS_GEN_WM5=1."
