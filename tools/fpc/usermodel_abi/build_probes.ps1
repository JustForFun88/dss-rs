# WASM_USERMODELS WP-WM.0 — build + run the ABI probes (evidence generator).
# Requires FPC 3.2.2 with the x86_64-win64 cross-compiler (ppcrossx64), the
# vendored .inputs/dss_capi + .inputs/electricdss-code-r3723-trunk, and the
# pinned dss-python oracle (tools/golden/PIN.txt) on `python`.
#
# Usage:  pwsh tools/fpc/usermodel_abi/build_probes.ps1 [-OutDir <scratch>]
# Writes nothing into the repo or .inputs; evidence copies under docs/wasm/probes/
# are refreshed MANUALLY from $OutDir (the goldens discipline — never by CI).
param(
    [string]$OutDir = (Join-Path $env:TEMP 'usermodel_abi_probes'),
    [string]$Fpc = 'C:\FPC\3.2.2\bin\i386-Win32\ppcrossx64.exe'
)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = (Resolve-Path (Join-Path $here '..\..\..')).Path
$capi = Join-Path $root '.inputs\dss_capi\src'
$r3723 = Join-Path $root '.inputs\electricdss-code-r3723-trunk\Version8\Source'
$r4133 = Join-Path $root '.inputs\electricdss-code-r4133-trunk\Version8\Source'
foreach ($p in $capi, $r3723, $r4133) {
    if (-not (Test-Path $p)) { throw "vendored source missing: $p — re-vendor, do not guess" }
}

# 0. regenerate the verbatim record extractions (asserts anchors still match)
python (Join-Path $here 'extract_defs.py')

# release-parity flags: -Mdelphi like src/common-release.cfg; the define that
# governs packing (DSS_CAPI_NO_PACKED_RECORDS) is UNSET, as in every release
# cfg except linux-arm32 (src/windows-x64.cfg carries no such define).
$capiDefines = @('-dDSS_CAPI', '-dDSS_CAPI_MVMULT', '-dDSS_CAPI_INCREMENTAL_Y',
    '-dDSS_CAPI_CONTEXT', '-dDSS_CAPI_PM', '-dDSS_CAPI_ADIAKOPTICS_DISABLED')

# 1. P2 — dss_capi 0.14.5 record-layout probe
$b = Join-Path $OutDir 'build_capi'; New-Item -ItemType Directory -Force $b | Out-Null
& $Fpc -Mdelphi -O3 -CF64 @capiDefines "-Fu$capi\Shared" "-Fi$here" "-FU$b" "-FE$b" (Join-Path $here 'abi_probe.pas')
& (Join-Path $b 'abi_probe.exe') | Tee-Object (Join-Path $OutDir 'p2_offsets_dss_capi.txt')

# 2. P2 twin — r3723 record-layout probe (the headers the example DLL uses)
$b = Join-Path $OutDir 'build_r3723'; New-Item -ItemType Directory -Force $b | Out-Null
& $Fpc -Mdelphi -O3 -CF64 "-Fu$r3723\Shared" "-Fu$r3723\PCElements" "-Fi$r3723\Common" "-FU$b" "-FE$b" (Join-Path $here 'abi_probe_r3723.pas')
& (Join-Path $b 'abi_probe_r3723.exe') | Tee-Object (Join-Path $OutDir 'p2_offsets_r3723.txt')

# 2b. P8 — r4133 record-layout probe (ABI re-freeze to r4133, user decision
# 2026-07-19): the headers the r4133 example DLL / native twin compile against.
# Result: TDynamicsRec 52 B / TDSSCallBacks 256 B UNCHANGED; TGeneratorVars
# 244 -> 252 B (deltaQNom at 176, tail +8). → docs/wasm/probes/p8_offsets_r4133.txt
$b = Join-Path $OutDir 'build_r4133'; New-Item -ItemType Directory -Force $b | Out-Null
& $Fpc -Mdelphi -O3 -CF64 "-Fu$r4133\Shared" "-Fu$r4133\PCElements" "-Fi$r4133\Common" "-FU$b" "-FE$b" (Join-Path $here 'abi_probe_r4133.pas')
& (Join-Path $b 'abi_probe_r4133.exe') | Tee-Object (Join-Path $OutDir 'p8_offsets_r4133.txt')

# 3. P1 — 15-export stub DLL + pinned-oracle load probe
$b = Join-Path $OutDir 'build_stub'; New-Item -ItemType Directory -Force $b | Out-Null
& $Fpc -Mdelphi -O2 -CF64 "-Fu$capi\Shared" "-Fi$here" "-FU$b" "-FE$b" (Join-Path $here 'genstub.pas')
python (Join-Path $here 'probe_oracle_load.py') (Join-Path $b 'genstub.dll') | Tee-Object (Join-Path $OutDir 'p1_oracle_load.txt')

# 2c. P9 — r4133 TCapControlVars layout probe (WP-WM.5): compiles the REAL
# vendored r4133 unit Version8/Source/Controls/CapControlVars.pas in its
# -dUSER_DLL variant (uses only ucomplex + ControlActionDefs.txt, no engine
# closure) — the byte-identical record the engine assembles for the CapControl
# PublicDataStruct (CapControl.pas:518). Result: 184 B; EControlAction 1 B (no
# {$Z4}); Voverride Boolean (1 B). → docs/wasm/probes/p9_offsets_capcontrolvars_r4133.txt
$b = Join-Path $OutDir 'build_ccvars'; New-Item -ItemType Directory -Force $b | Out-Null
& $Fpc -Mdelphi -O3 -CF64 -dUSER_DLL "-Fu$r4133\Controls" "-Fu$r4133\Shared" "-Fi$r4133\Controls" "-FU$b" "-FE$b" (Join-Path $here 'abi_probe_capcontrolvars.pas')
& (Join-Path $b 'abi_probe_capcontrolvars.exe') | Tee-Object (Join-Path $OutDir 'p9_offsets_capcontrolvars_r4133.txt')

# 2d. P10 — r4133 TWindGenVars layout probe (R4133_PROPS_PLAN RP1.3): compiles
# the REAL vendored r4133 unit Version8/Source/PCElements/WindGenVars.pas — the
# record TWindGenUserModel.FNew receives (WindGenUserModel.pas:34). Result:
# 356 B; NO deltaQNom (the three integers keep the Appendix-A offsets
# 176/180/184, so the head through XRdp@236 is byte-identical to the
# TGeneratorVars WASM image); a MANAGED `PLoss: string` reference at 244 ahead
# of the 13-double turbine tail.
# → docs/wasm/probes/p10_offsets_windgenvars_r4133.txt
$b = Join-Path $OutDir 'build_wgvars'; New-Item -ItemType Directory -Force $b | Out-Null
& $Fpc -Mdelphi -O3 -CF64 "-Fu$r4133\Shared" "-Fu$r4133\PCElements" "-Fi$r4133\Common" "-FU$b" "-FE$b" (Join-Path $here 'abi_probe_windgenvars.pas')
& (Join-Path $b 'abi_probe_windgenvars.exe') | Tee-Object (Join-Path $OutDir 'p10_offsets_windgenvars_r4133.txt')

# 4. P2 plan-A check — the VENDORED IndMach012a.dpr, as-is (zero source edits)
$b = Join-Path $OutDir 'build_indmach'; New-Item -ItemType Directory -Force $b | Out-Null
& $Fpc -Mdelphi -O2 "-Fu$r3723\IndMach012a" "-Fu$r3723\Shared" "-Fu$r3723\Parser" "-Fu$r3723\PCElements" "-Fi$r3723\Common" "-FU$b" "-FE$b" (Join-Path $r3723 'IndMach012a\IndMach012a.dpr')
Write-Host "IndMach012a.dll built: $((Get-Item (Join-Path $b 'IndMach012a.dll')).Length) bytes"
