# usermodel_abi — WP-WM.0 ABI-freeze probes (WASM_USERMODELS plan)

**Status: CAPTURED & FROZEN (2026-07-18).** The probe outputs under
`docs/wasm/probes/` are the *sole* source of the offset tables frozen in
`docs/wasm/USERMODEL_ABI.md` — per the plan rule, never derived from reading
the Pascal or from memory.

## What each piece is

| File | Purpose |
|---|---|
| `extract_defs.py` | Mechanically copies the boundary record declarations **verbatim** from vendored dss_capi 0.14.5 into `dss_capi_defs.inc` (provenance SHA-256 inside). Asserts the 32-slot callback vtable. |
| `dss_capi_defs.inc` | Generated — do not edit. `TGeneratorVars`, `TDSSCallBacks` + support aliases. `TDynamicsRec` is NOT here: probes compile the real vendored `src/Shared/Dynamics.pas` unit directly. |
| `abi_probe.pas` | P2: prints `SizeOf` + every field offset of `TDynamicsRec`/`TGeneratorVars`/`TDSSCallBacks`, compiled with release parity (`-Mdelphi`, packing define unset ⇒ packed). → `p2_offsets_dss_capi.txt` |
| `abi_probe_r3723.pas` | P2 twin: same tables from the **r3723** headers the canonical example DLL compiles against (real vendored units + the verbatim `DSSCallBackStructDef.pas` include). → `p2_offsets_r3723.txt`. Result: byte-identical to dss_capi 0.14.5. |
| `abi_probe_r4133.pas` | **P8** (ABI re-freeze to r4133, 2026-07-19): same tables from the **r4133** headers (the twin's compile target now). → `p8_offsets_r4133.txt`. Result: `TDynamicsRec` 52 B / `TDSSCallBacks` 256 B **unchanged**; `TGeneratorVars` **252 B** with `deltaQNom` at 176 and the tail +8. |
| `abi_probe_windgenvars.pas` | **P10** (`R4133_PROPS_PLAN.md` RP1.3): `SizeOf` + every field offset of **r4133 `TWindGenVars`** (`PCElements/WindGenVars.pas`), the record `TWindGenUserModel.FNew` receives (`WindGenUserModel.pas:34`). → `p10_offsets_windgenvars_r4133.txt`. Result: **356 B**, **no** `deltaQNom` (so the head through `XRdp`@236 is byte-identical to the `TGeneratorVars` **wasm** image), a managed `PLoss: string` reference at 244, then the 13-double turbine tail 252…356. |
| `genstub.pas` | P1: minimal 15-export Generator user-model stub DLL with recognizable `Calc` output. |
| `probe_oracle_load.py` | P1: drives `genstub.dll` through the pinned dss-python oracle (`Generator.UserModel=`, vars surface, `UserData=`→`Edit`, Model=6 solve, V/I marshalling). → `p1_oracle_load.txt` |
| `build_probes.ps1` | Reproduces every build + run with the exact flags (FPC 3.2.2 `ppcrossx64`, x86_64-win64). Also builds the **vendored `IndMach012a.dpr` as-is** (plan-A twin check). |

## Findings (frozen 2026-07-18; details in `docs/wasm/USERMODEL_ABI.md` + STATUS §WASM-UM)

- **P1 PASS** — the pinned oracle (dss-python 0.15.7 / dss_capi 0.14.5 win64)
  loads and drives a native user-model DLL end-to-end: §2.5 **channel 1
  confirmed**, r3723 fallback not needed.
- **P2** — all three records packed; `TDynamicsRec` 52 B, `TGeneratorVars`
  244 B, `TDSSCallBacks` 256 B (32×8). dss_capi 0.14.5 and r3723 layouts
  **byte-identical**. **Plan A confirmed**: FPC `-Mdelphi` builds the vendored
  `IndMach012a.dpr` unmodified (search paths only), and that DLL loads + solves
  under the pinned oracle (`p2_indmach_fpc_build.txt`).
- **P3** — IndMach012a invokes exactly **one** callback slot: `MsgCallBack`
  (`IndMach012Model.pas:474`, help text). `DoDSSCommand` unused ⇒ WM.6 stays
  deferred. It bundles its own `ParserDel` (host parser callbacks unused).
- **P4** — stable rustc 1.96.0 + `wasm32-unknown-unknown`; `wasmi =1.0.9`
  (`default-features=false`, `["simd"]`) compiles pure-Rust (bitflags/libm/
  spin/wasmparser); fuel + store-limit APIs present. Pin recorded in
  `tools/wasm_usermodel/PIN.txt`.
- **§2.7 check** — loader units unchanged in contract across 0.14.5→0.15.x and
  r3723→r4133; **but** 0.15.x/r4088+ insert `deltaQNom: array of Double` into
  `TGeneratorVars` (layout shift). Our frozen ABI = the pinned 0.14.5/r3723
  layout (`p5_upgrade_diff.txt`).
- **P8 (2026-07-19, ABI re-freeze to r4133)** — `abi_probe_r4133.pas` probes
  the r4133 headers empirically: `TGeneratorVars` **252 B** (`deltaQNom` @176,
  tail +8); `TDynamicsRec` 52 B / `TDSSCallBacks` 256 B unchanged
  (`p8_offsets_r4133.txt`). The **frozen native** layout is now r4133; the wasm
  marshaled image stays the 244-B subset (`deltaQNom` never crosses). See
  `docs/wasm/USERMODEL_ABI.md` §2.2 + Appendix A.
- **P10 (2026-08-23, RP1.3)** — `TWindGenVars` is a **different record shape**
  from `TGeneratorVars`, measured at **356 B**: the 22 leading doubles, the
  three integers at 176/180/184 (no NCIM `deltaQNom`), the same unaligned
  Thevenin tail through `XRdp`@236, then `PLoss: string` (an 8-byte managed
  AnsiString reference) at 244 and 13 turbine doubles `ag`…`s` at 252…348. The
  wasm marshaled image drops the managed reference and closes the hole exactly
  as §2.2b does for `deltaQNom` — **348 B**, `ag`@244 — which makes the wasm
  `WindGenVars` image a strict byte-for-byte **extension** of the wasm
  `GeneratorVars` image. See `docs/wasm/USERMODEL_ABI.md` §2.6.

## Regenerating

```powershell
pwsh tools/fpc/usermodel_abi/build_probes.ps1 -OutDir $env:TEMP\usermodel_abi_probes
# then copy the p*.txt outputs into docs/wasm/probes/ MANUALLY (goldens discipline)
```
