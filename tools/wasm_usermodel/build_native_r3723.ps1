# build_native_r3723.ps1 — build the 0.14.5-ABI NATIVE TWIN of the wasm
# reference fixture (WM.3 D2 follow-up).
#
# Sibling of build_native.ps1 (the r4133 twin). This variant compiles the
# SAME vendored IndMach012a example, but from the r3723 tree, whose
# TGeneratorVars has NO `deltaQNom` slot — the historical 0.14.5/r3723
# 244-byte layout (USERMODEL_ABI.md Appendix A). It is the twin that loads
# ABI-correctly into the pinned dss-python 0.15.7 (backend dss_capi 0.14.5),
# which passes the Generator a 244-byte TGeneratorVars image.
#
# Why a separate tree and not just build_native.ps1 with a different source:
# the IndMach012a model files (IndMach012Model / MainUnit / ParserDel / .dpr)
# are byte-identical r3723 -> r4133 (sha256-verified; USERMODEL_ABI.md P8) —
# ONLY GeneratorVars.pas differs (deltaQNom, 244 vs 252 B). So this build gives
# the IDENTICAL machine math on the 0.14.5 record layout: the single controlled
# variable for the D2 three-way experiment is the engine version, not the model.
#
# Compiles VENDORED Delphi as-is (zero source edits, search paths only):
# .inputs/electricdss-code-r3723-trunk/Version8/Source/IndMach012a/IndMach012a.dpr
# with FPC 3.2.2 ppcrossx64 -Mdelphi -O2 (PIN.txt toolchain), same flags as the
# r4133 build. Built ON DEMAND for the D2 probe; NEVER committed.
#
# Usage:
#   pwsh tools/wasm_usermodel/build_native_r3723.ps1 [-OutDir <dir>] [-Fpc <ppcrossx64.exe>]

param(
    [string]$OutDir = (Join-Path $env:TEMP "indmach012a_native_twin_r3723"),
    [string]$Fpc = "C:\FPC\3.2.2\bin\i386-Win32\ppcrossx64.exe"
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$Src = Join-Path $RepoRoot ".inputs\electricdss-code-r3723-trunk\Version8\Source"
$Dpr = Join-Path $Src "IndMach012a\IndMach012a.dpr"

if (-not (Test-Path $Dpr)) {
    throw "Vendored source missing: $Dpr — re-vendor .inputs/electricdss-code-r3723-trunk (ritual step 0)."
}
if (-not (Test-Path $Fpc)) {
    throw "FPC cross-compiler not found at $Fpc (PIN: fpc==3.2.2 ppcrossx64). Pass -Fpc."
}

New-Item -ItemType Directory -Force $OutDir | Out-Null

# Exact flags from the WP-WM.0 P2 probe (p2_indmach_fpc_build.txt), r3723 tree.
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
Write-Host "0.14.5-ABI native twin: $Dll ($((Get-Item $Dll).Length) bytes)"
Write-Host ("sha256: " + (Get-FileHash -Algorithm SHA256 $Dll).Hash)
