# tools/wasm_usermodel — WASM user-model fixtures & toolchain (WASM_USERMODELS plan)

The reference user-model fixtures that replace upstream's native user-written
DLLs over the frozen WASM ABI (`docs/wasm/USERMODEL_ABI.md`). Committed
`.wasm` fixtures are **golden-class artifacts**: regenerated MANUALLY ONLY
with the toolchain pinned in `PIN.txt` (plan §2.6/§2.9-4); a WP never
regenerates them to make itself pass.

## Layout

| Path | What |
|---|---|
| `PIN.txt` | The toolchain + artifact-hash pin (wasmi, rustc, FPC, SHA-256 of each committed `.wasm`) |
| `models/indmach012a/` | The reference model: loop-for-loop Rust port of the vendored r3723 example DLL (`Version8/Source/IndMach012a/`: `IndMach012Model.pas` + `MainUnit.pas` + the units it links) to a `wasm32-unknown-unknown` guest. Workspace-excluded crate. |
| `build_wasm.ps1` | Rebuilds `tests/fixtures/wasm/indmach012a.wasm` reproducibly (clean-build hash-stable) and prints the SHA-256 for `PIN.txt` |
| `build_native.ps1` | Builds the **native twin** on demand — the *same vendored Pascal* compiled by FPC 3.2.2 (`ppcrossx64 -Mdelphi -O2`, plan-A decision from WP-WM.0 P2). Never committed; default output in `%TEMP%` |
| `twin_probe.py` | Drives the native twin through a fixed deterministic scenario (ctypes, packed record images per the frozen ABI offsets) and records every observable value bit-exactly. Default output = evidence (`docs/wasm/probes/p6_twin_expected.txt`); `--rust` output = the generated test fragment `models/indmach012a/tests/twin_expected.rs` |

## The gate chain for the fixture

1. **Native twin = the numeric spec** (upstream's own Pascal, so it is also
   what the pinned dss-python oracle loads at golden generation — plan §2.5
   channel 1, P1-confirmed).
2. `models/indmach012a/tests/twin_parity.rs` pins the Rust port **bit-exact**
   (f64 `to_bits` equality — the model uses only `+ - * / sqrt`, IEEE-exact
   both on x86-64 and in wasm) against the twin-probed values. Run with
   `cargo +stable test` inside `models/indmach012a/` (host target).
3. WM.2 item 4 (integrator): the committed `.wasm` is driven through the
   `dss-usermodel` crate API and pinned against the same
   `p6_twin_expected.txt` values; the PIN hash test keeps the artifact
   drift-free in every `cargo test`.
4. WM.3+: end-to-end decks — pinned oracle *with the native twin DLL loaded*
   vs the Rust engine *with the `.wasm` fixture* (`tools/golden/
   gen_wasm_usermodels.py`, lands at WM.3).

## Rebuilding (manual, deliberate)

```powershell
# the committed wasm fixture (then update PIN.txt with the printed SHA-256)
pwsh tools/wasm_usermodel/build_wasm.ps1

# the native twin (on demand; never committed)
pwsh tools/wasm_usermodel/build_native.ps1            # -> %TEMP%\indmach012a_native_twin
# re-record the expected values (goldens discipline: manual, then commit both outputs)
python tools/wasm_usermodel/twin_probe.py $env:TEMP\indmach012a_native_twin\IndMach012a.dll `
    > docs/wasm/probes/p6_twin_expected.txt
python tools/wasm_usermodel/twin_probe.py $env:TEMP\indmach012a_native_twin\IndMach012a.dll --rust `
    > tools/wasm_usermodel/models/indmach012a/tests/twin_expected.rs
```

## Fidelity notes (see the crate's module docs for the full list)

- The port reproduces the **r3723 unit bodies the DLL links**, not dss-core's
  FPC-RTL helpers: naive `CDIV`/`Cabs` (`Ucomplex.pas`), the numerically
  inverted `Ap2s` matrix (`TcMatrix.Invert` at unit init), truncated
  `0.866025403` / `1.732` constants (`TODO(compat)`).
- `TODO(compat)`: FPC folds the all-constant `3.0/746.0` (HPshaft, var 14) at
  **single** precision — proven by decomposition against the twin (a plain
  f64 quotient misses by 2.6e-8 rel); reproduced as `(3.0f32/746.0f32) as f64`.
- The debug-trace file (`IndMach012_Trace.CSV`) is a no-op in the sandbox (no
  filesystem — ABI §6); `option=debug` still toggles the flag.
- RPN inline math inside quoted `UserData=` values is not implemented: a real
  RPN expression traps loudly (the reference decks never use it; plan §2.6
  sanctions the minimal scanner).
