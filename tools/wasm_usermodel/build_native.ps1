# build_native.ps1 — build the NATIVE TWIN of the wasm reference fixture
# (WASM_USERMODELS_PLAN §2.6, plan A — decided at WP-WM.0 P2, evidence
# docs/wasm/probes/p2_indmach_fpc_build.txt).
#
# ABI RE-FREEZE TO r4133 (user decision 2026-07-19): the twin now compiles
# against the r4133 sources (the gate is the in-house r4133 bridge). The
# IndMach012a example dir is byte-identical r3723->r4133 (evidence
# docs/wasm/probes/p8_indmach012a_math_diff.txt) — only GeneratorVars.pas
# changed (deltaQNom, 244->252 B). So the model math is unchanged; the twin is
# rebuilt against r4133 headers so it loads into the r4133 engine bridge with a
# self-consistent 252-byte TGeneratorVars image (p8_twin_r4133_bridge.txt).
#
# Compiles the VENDORED Delphi example user-model DLL as-is (zero source
# edits, search paths only): .inputs/electricdss-code-r4133-trunk/Version8/
# Source/IndMach012a/IndMach012a.dpr with FPC 3.2.2 ppcrossx64 -Mdelphi -O2
# (the toolchain pinned in tools/wasm_usermodel/PIN.txt).
#
# The twin is built ON DEMAND for golden generation / probing and is NEVER
# committed (plan §2.6). Default output goes to a temp dir outside the repo.
#
# Usage:
#   pwsh tools/wasm_usermodel/build_native.ps1 [-OutDir <dir>] [-Fpc <ppcrossx64.exe>]

param(
    [string]$OutDir = (Join-Path $env:TEMP "indmach012a_native_twin"),
    [string]$Fpc = "C:\FPC\3.2.2\bin\i386-Win32\ppcrossx64.exe"
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$Src = Join-Path $RepoRoot ".inputs\electricdss-code-r4133-trunk\Version8\Source"
$Dpr = Join-Path $Src "IndMach012a\IndMach012a.dpr"

if (-not (Test-Path $Dpr)) {
    throw "Vendored source missing: $Dpr — re-vendor .inputs/electricdss-code-r4133-trunk (ritual step 0)."
}
if (-not (Test-Path $Fpc)) {
    throw "FPC cross-compiler not found at $Fpc (PIN: fpc==3.2.2 ppcrossx64). Pass -Fpc."
}

New-Item -ItemType Directory -Force $OutDir | Out-Null

# Exact flags from the WP-WM.0 P2 probe (p2_indmach_fpc_build.txt).
& $Fpc -Mdelphi -O2 `
    ("-Fu" + (Join-Path $Src "IndMach012a")) `
    ("-Fu" + (Join-Path $Src "Shared")) `
    ("-Fu" + (Join-Path $Src "Parser")) `
    ("-Fu" + (Join-Path $Src "PCElements")) `
    ("-Fi" + (Join-Path $Src "Common")) `
    ("-FU" + $OutDir) `
    ("-FE" + $OutDir) `
    $Dpr
if ($LASTEXITCODE -ne 0) { throw "FPC build failed (exit $LASTEXITCODE)" }

$Dll = Join-Path $OutDir "IndMach012a.dll"
if (-not (Test-Path $Dll)) { throw "Build produced no DLL at $Dll" }
Write-Host "native twin: $Dll ($((Get-Item $Dll).Length) bytes)"
