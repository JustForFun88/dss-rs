# build_wgturbine_wasm.ps1 — rebuild the committed golden-class fixture
# tests/fixtures/wasm/wgturbine.wasm (WASM_USERMODELS_PLAN §2.6;
# R4133_PROPS_PLAN RP1.3 — the WindGen `UserModel=` guest).
#
# MANUAL-ONLY regeneration with the toolchain pinned in
# tools/wasm_usermodel/PIN.txt (stable rustc, wasm32-unknown-unknown, the locked
# [profile.release] in the fixture crate). A WP never regenerates the fixture to
# make itself pass (plan §2.9-4); after a deliberate rebuild, update the SHA-256
# recorded in PIN.txt.
#
# Usage:  pwsh tools/wasm_usermodel/build_wgturbine_wasm.ps1

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$Crate = Join-Path $RepoRoot "tools\wasm_usermodel\models\wgturbine"
$OutDir = Join-Path $RepoRoot "tests\fixtures\wasm"
$Artifact = Join-Path $OutDir "wgturbine.wasm"

# Deterministic across checkouts: strip the absolute crate path from any
# panic-message metadata embedded in the binary.
$env:RUSTFLAGS = "--remap-path-prefix=$Crate=/wgturbine"

Push-Location $Crate
try {
    cargo +stable build --release --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
}
finally {
    Pop-Location
    Remove-Item Env:\RUSTFLAGS -ErrorAction SilentlyContinue
}

New-Item -ItemType Directory -Force $OutDir | Out-Null
Copy-Item (Join-Path $Crate "target\wasm32-unknown-unknown\release\wgturbine.wasm") $Artifact -Force

$Hash = (Get-FileHash -Algorithm SHA256 $Artifact).Hash.ToLower()
$Size = (Get-Item $Artifact).Length
Write-Host "artifact: $Artifact"
Write-Host "size:     $Size bytes"
Write-Host "sha256:   $Hash"
Write-Host "toolchain: $(cargo +stable --version)"
Write-Host ""
Write-Host "Now update tools/wasm_usermodel/PIN.txt with this SHA-256 (manual step)."
