# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-06-12, **Phase 4 WP4.6 done** — Reactor (`TReactorObj`):
two-terminal shunt/series reactor with all four spec types (kvar / R+jX / R&X
matrices / Z1Z2Z0 symmetrical components), `Parallel` R∥X, `Rp`, `LmH`, Bus2
grounded-default. `CalcYPrim` matches the oracle for the kvar, Z1Z2Z0, and
RMatrix/XMatrix paths. Next: WP4.7 (ControlElem + RegControl/CapControl
parse-only). On branch `phase-4-pd-elements`.

> **Working cadence (per PHASE4_PLAN §0.8):** finish one small step → run the full
> gate → update this file → **stop and wait for explicit user confirmation** before
> the next step. Do not chain WPs.

---

## 1. Where we are

| Phase | Scope | Status |
|------|-------|--------|
| 0 | Tooling, oracle, faer spike, CI, Phase-0 goldens | ✅ done (committed) |
| 1 | Shared math (`support/`) + full `TDSSParser` port | ✅ done (commit `729eb77`) |
| 2 | Object model, property engine, executive skeleton | ✅ done (commit `22f861d`) |
| **3** | **★ Vertical slice: parse → circuit → Y matrix → solve → voltages** | ✅ done (merged to `main`, commit `2ac8691`) |
| **4** | Transformer/Capacitor/Reactor/LineCode + `define_properties!` | 🔶 **in progress** — WP4.1 (LineCode) ✅, WP4.2 (ObjectRef/FetchLineCode) ✅, WP4.3a (GrowthShape) ✅, WP4.3b (XfmrCode) ✅, WP4.4 (Transformer) ✅, WP4.5 (Capacitor) ✅, WP4.6 (Reactor) ✅; WP4.7 → 4.10 next |

**Important:** Phases 2–3 live on the `phase-2-object-model` branch (off
`main`), per the repo rule that commits happen only on explicit request and
never directly on the default branch. Phase 3 is **not yet committed**.

> History note: an earlier Phase 3 attempt (assessed this session) was partial
> and numerically wrong (Zs=Z1 line diagonal, 1-terminal VSource, load power
> n× too big, no compensation currents, no oracle gate). It was fully redone
> from the Pascal sources; the old `phase3_sanity.rs` was deleted.

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 125, golden_slice 2, golden_smoke 3,
                            # props_roundtrip 1, dss-parser 62+1, dss-sparse 5
```

### Phase 3 gate (`crates/dss-core/tests/golden_slice.rs`) — green
13 scenarios from `tests/golden/slice.json` (pinned oracle), all matching:
- (a) 2-bus vsource+line+load: node voltages within **1e-9 rel**;
- (b) **IEEE13-flat** (transformers/regulators/caps stripped, linecodes inlined
  as per-line `rmatrix/xmatrix/cmatrix`, 32 nodes, min 0.88 pu): **1e-6 rel**;
- (c) fixed-point iteration counts **exactly equal** (2 for 2-bus family,
  3 for ieee13_flat);
- (d) all **8 load models** exercised (incl. CVR powf variant, delta
  connection, and a 5 km heavy case that drives |V| to 0.916 pu so the
  below-95% interpolation zones execute).

The CLI works end to end: `cargo run -p dss-cli -- script.dss` compiles the
script and prints per-node |V| / angle.

---

## 1b. Phase 4 frontier (branch `phase-4-pd-elements`)

Execution plan: **`PHASE4_PLAN.md`** (WP4.1–WP4.10). Cadence: one small step,
then update this file and wait for confirmation (PHASE4_PLAN §0.8).

**WP4.1 — LineCode — ✅ DONE, gate-green.** Files:
- `src/elements/general/line_code.rs` (`TLineCodeObj`): props 1–27 + Like;
  `CalcMatricesFromZ1Z0` (no 1-phase special case), `Set_NumPhases`,
  `DoKronReduction`, `PropertySideEffects`, `EndEdit`, `MakeLike`; 6 inline tests.
  Registered in `exec/mod.rs` as a `DSS_OBJECT` class after Spectrum.
- **Shared engine additions** (reused by later WPs):
  - `PropFlags::CONDITIONAL_VALUE` + `DssObject::prop_conditional` — sym scalars
    (R1/X1/R0/X0/C1/C0/B1/B0) render `----` once a matrix model is active.
  - Sym-matrix getter format corrected to the oracle's `[v |v v |...]` (no leading
    space, trailing space per element, `|` between rows) in `props.rs::get_value`.
  - Deferred-error buffer on `DssObjData` (`push_error`/`take_errors`), drained in
    `exec::edit_active` after `end_edit` — lets `side_effects`/`EndEdit` emit
    `DoSimpleMsg` (e.g. `Kron` on a 1-phase code → error 103, no-op).
- **Goldens:** 8 LineCode scenarios added to `gen_props.py`; `props.json`
  regenerated with the pinned oracle. `props_roundtrip` green.
- **`TODO(compat)`:** LineCode `Repair` defaults to `0` (matches the oracle's `?`
  getter) although the Pascal ctor sets `HrsToRepair := 3`; the field is
  deprecated/unused (never propagated to lines).
- Gate: `cargo test --workspace` — dss-core lib **93** (was 87), props_roundtrip 1,
  golden_slice 2, golden_smoke 3, dss-parser 62+1, dss-sparse 5. All green.

**WP4.2 — ObjectRef resolution + Line→LineCode fetch — ✅ DONE, gate-green.**
- **Shared engine (`obj/props.rs`, `obj/base.rs`, `exec/mod.rs`):**
  - `PropDef` gained `object_class: Option<&'static str>`. `object_ref(name)`
    keeps the Phase 3 string-storage behavior (Load/VSource shape refs stay
    unresolved until Phase 5); the new `object_ref_class(class, name)` resolves
    at parse time.
  - `ForeignClassesView<'a>` trait + `PropEngine::foreign` field: a read view of
    every class *except* the one being edited. `edit_active` builds it via
    `split_at_mut(ci)` + `split_first_mut` (active class = excluded middle,
    zero unsafe) and threads it into `parse_into`. The `ObjectRef` parse arm
    resolves `cls.find(name)`; on miss it emits the Pascal 401 message
    (`<Full>.<Prop>: <Class> object "<name>" not found.`) and continues with a
    NIL reference.
  - `DssObject::set_object_ref(idx, name, resolved)`: stores the dump name +
    `ElemRef` and lets the element copy data immediately by downcasting the
    `&dyn DssObject` (the `FetchLineCode` pattern). Dump name read back via
    `get_string`.
- **Line (`elements/pd/line.rs`):** `linecode` is now
  `object_ref_class("LineCode", "LineCode")`. New fields `line_code_units`,
  `line_code_ref`, `line_code_name`. `fetch_line_code` ports `TLineObj.FetchLineCode`
  verbatim (copies sym/matrix Z·Yc, Rg/Xg/rho→Kxg, units→`FUnitsConvert`,
  norm/emerg/ratings, zeroes the supplied props' set-order marks, resizes
  phases, recalcs or copies matrices, `NConds := Fnphases`). `units=` side
  effect now reconverts relative to the code's units when a code is set.
  `kill_line_code_specified` (drops the ref on sym/matrix/switch overrides).
  Added `CONDITIONAL_VALUE` + `prop_conditional` so the sym scalars render
  `----` under a matrix model (matches the oracle). Two bug fixes surfaced by
  the new Line property dumps: earth-model default is **DERI (3)**, not
  SIMPLECARSON (Pascal `DSS.DefaultEarthModel := DERI`); the `linecode`
  property name is **`LineCode`** (capitalized, for the 401 message).
- **Goldens:** 4 Line+LineCode scenarios added to `gen_props.py`
  (`line_code_sym`, `line_code_then_units`, `line_code_matrix`,
  `line_code_then_r1`); `props.json` regenerated with the pinned oracle.
- **Numeric check (WP4.2 DoD):** a 2-bus `linecode=` snapshot solves bit-for-bit
  like the oracle (7198.343402 / 7194.911983 V, 2 iterations).
- Gate: dss-core lib **97** (was 93; +4 `exec::tests::line_*`), props_roundtrip 1,
  golden_slice 2, golden_smoke 3, dss-parser 62+1, dss-sparse 5. All green.

**WP4.3a — GrowthShape — ✅ DONE, gate-green.** Files:
- `src/elements/general/growth_shape.rs` (`TGrowthShapeObj`): props 1–6 + Like
  (`NPts, Year, Mult, CSVFile, SngFile, DblFile`). `Year` uses `APPLY_ROUND`
  (FPC banker's rounding, verified: oracle rounds `2002.5 → 2002`).
  `PropertySideEffects` reallocs Year/Multiplier on `NPts`; `EndEdit` →
  `recalc_year_mult`; `get_mult` (cumulative year multiplier, consumed by the
  Phase-5 yearly mode) and `recalc_year_mult` ported verbatim from Pascal
  (note: base year and earlier return 1.0 — multipliers apply to *following*
  years). `MakeLike` copies npts/year/multiplier. 6 inline unit tests.
  Registered in `exec/mod.rs` as a `DSS_OBJECT` class after LineCode.
- **Deferrals:** `CSVFile`/`SngFile`/`DblFile` are `NOT_PORTED` (file input —
  PHASE4_PLAN §5; the gate feeders never use them; hard parse error on set).
- **Goldens:** 5 GrowthShape scenarios added to `gen_props.py`
  (`growthshape_default/_full/_year_rounds/_edit_shrink/_makelike`);
  `props.json` regenerated with the pinned oracle. `props_roundtrip` green.
- Gate: dss-core lib **103** (was 97; +6 growth_shape), props_roundtrip 1,
  golden_slice 2, golden_smoke 3, dss-parser 62+1, dss-sparse 5. All green.

**WP4.3b — XfmrCode — ✅ DONE, gate-green.** Took the "define the shared
`Winding` struct now" branch of PHASE4_PLAN §WP4.3 step 1 (the struct is pure,
fully-specified data — nothing in it depends on Transformer behavior).
- **`src/elements/pd/winding.rs` (`Winding`)** — port of the Pascal `TWinding`
  record (`Transformer.pas` l.162): connection/kVLL/VBase/kVA/puTap/Rpu/Rdcpu/
  RdcOhms/Rneut/Xneut/Y_PPM/RdcSpecified + tap-changer fields, with `Winding::new`
  (= `TWinding.Init`, the 12.47 kV / 1000 kVA wye defaults, RdcOhms=0.26435153)
  and `compute_anti_float_adder` (`Y_PPM = -ppm/(VBase²/VABase1ph)/2`). Shared
  verbatim by Transformer (WP4.4). 2 inline tests.
- **`src/elements/general/xfmr_code.rs` (`TXfmrCodeObj`)** — props 1–39 + Like.
  Per-winding scalars (`kV/kVA/Tap/%R/RNeut/XNeut/MaxTap/MinTap/RDCOhms/Conn/
  NumTaps`) reuse the existing Double/Integer/MappedEnum prop types — the object
  indexes `Winding[ActiveWinding]` internally (no new engine machinery). The
  plural array forms (`Conns/kVs/kVAs/Taps/%Rs`) and `XSCArray` use the new
  struct-array prop types (below). `PropertySideEffects` ports the winding-edit
  state machine (`windings` realloc+re-Init + XSC grow-to-0.30; `kVA`/`kVAs`
  default Norm/EmergHkVA to 1.1×/1.5×; `%R`/`%Rs`↔`%LoadLoss` split; `X*` set
  NeedsRecalc; `RDCOhms` sets RdcSpecified; `Seasons` resizes Ratings).
  `EndEdit` copies `XHL/XHT/XLT` into the leading XSC slots (≤ 3 windings).
  `MakeLike` via `SetNumWindings` + winding copy. 7 inline tests.
  Registered as a `DSS_OBJECT` class after GrowthShape.
- **Shared engine additions** (reused by Transformer WP4.4):
  - 3 new `PropType`s — `DoubleVArray` (function-sized, count from
    `DssObject::array_size`, e.g. `XSCArray = (NumWindings-1)·NumWindings/2`),
    `DoubleArrayOnStruct` (one double per struct entry, count = an integer prop;
    omitted tokens keep the prior value; rendered `[v, v, ]`), and
    `EnumArrayOnStruct` (enum per struct entry; `[s, s, ]`).
  - `DssObject` trait grew `array_size`, `get/set_struct_f64_array`,
    `get/set_struct_i32_array` (the struct-array setters also advance the active
    index, Pascal `positionPtr^ := intVal`).
- **`%`-name convention:** `pctR→%R`, `pctLoadLoss→%LoadLoss`,
  `pctNoLoadLoss→%NoLoadLoss`, `pctIMag→%IMag`, `pctRs→%Rs` are named directly in
  the prop table (same as Load/Spectrum).
- **Deferrals:** `PullFromTransformer` (reverse copy, only used by Transformer's
  `XfmrCode=`-from-transformer path) deferred to WP4.4; `CSVFile`-style file
  inputs N/A (XfmrCode has none). `VABase`/`Y_PPM` are computed faithfully but
  inert until Transformer consumes them.
- **Goldens:** 6 XfmrCode scenarios added to `gen_props.py`
  (`xfmrcode_default/_full/_wdg_seq/_xscarray/_ratings/_makelike`);
  `props.json` regenerated with the pinned oracle (dump format matched exactly:
  array-on-struct `[a, b, ]`, XSCArray/Ratings the standard `[ a b c]`).
- Gate: dss-core lib **112** (was 103; +7 xfmr_code, +2 winding), props_roundtrip
  1, golden_slice 2, golden_smoke 3, dss-parser 62+1, dss-sparse 5. All green.

**WP4.4 — Transformer — ✅ DONE, gate-green.** The 30% centerpiece. Files:
- `src/elements/pd/transformer.rs` (`TTransfObj`): props 1–49 + PD/CktElement
  tails + Like. `nterms = NumWindings`, `nconds = nphases + 1` (per-winding
  brought-out neutral). Ported verbatim: `SetNumWindings` (realloc windings/XSC/
  terminals/ZB/Y_1Volt/Y_Term matrices), `PropertySideEffects` (winding-edit
  state machine: `kVA`/`kVAs` default Norm/EmergHkVA 1.1×/1.5×; `%R`/`%Rs`↔
  `%LoadLoss` split; `XHL/XHT/XLT/X12/X13/X23` set `XHLChanged` + clear
  XSCArray/XfmrCode set-marks; `XSCArray` clears the reactance set-marks),
  `RecalcElementData` (DeltaDirection, SetTermRef, tap increments, XSC←XHL,
  per-winding VBase with the 1-/2-/3-φ branch, **Rdc recomputed on the
  transformer VABase** not the winding's, anti-float adders, NormAmps/EmergAmps/
  AmpRatings via the wye/delta VFactor), `SetTermRef` (winding↔terminal
  conductor map incl. the delta `RotatePhases` rotation), `CalcY_Terminal`
  (ZB short-circuit matrix → invert → `Y_1Volt = AT·ZB⁻¹·A` → magnetizing branch
  on winding 2 → `Y_Term = AT·Y_1Volt·A` voltage-ratio incidence → anti-float
  adders), `CalcYPrim` (`BuildYPrimComponent` stamps `Y_Term`/`Y_Term_NL` via
  TermRef nphases times; `AddNeutralToY` rneut/xneut grounding + open-neutral
  1e6/Y_PPM; then open-conductor `do_yprim_calcs`), `MakeLike`, `FetchXfmrCode`,
  `Get/Set_PresentTap` (public for Phase-5 RegControl), `GetAllWindingCurrents`/
  `GetWindingCurrentsResult` (the `WdgCurrents` RO dump). 4 inline tests:
  SetTermRef for wye-wye and wye-delta, **YPrim of a 1φ 2-wdg transformer vs the
  oracle (1e-4)**, tap clamp.
  Registered as `ElemKind::Transformer` after Load.
- **Shared engine additions** (reused by Capacitor/Reactor later):
  - `circuit.rs`: `ElemKind::Transformer` + `transformers` list (PD list + own).
  - `props.rs`: two new `PropType`s — `BusOnStruct` (transformer `bus`, the
    active winding's terminal) and `BusesOnStruct` (`buses`, `[b1, b2, ]`),
    with `PropDef::bus_on_struct`/`buses_on_struct`.
  - `base.rs`: `DssObject::{set_active_struct_bus, get_active_struct_bus,
    set_struct_buses, get_struct_buses}`.
  - `dss_enum.rs`: `core_type` (`Core Type`, default shell=0; non-sequential
    ordinals `0,1,3,4,5,9`) and `lead_lag` (`Phase Sequence` reused for
    `LeadLag`, lag=0/lead=1) enums.
  - `xfmr_code.rs`: public read accessors for `FetchXfmrCode`.
- **Deferrals:** GIC path (`frequency < 0.51`, `GICBuildYTerminal`) — Phase 7,
  unreachable at 60 Hz (`Y_Terminal_FreqMult` stays 1.0). `GetLosses` load/no-load
  split, `MakePosSequence`, `SaveWrite` — not needed by the Phase-4 gate (totals
  use the base `losses`); port when a gate requires them.
- **Goldens:** 7 Transformer scenarios in `gen_props.py` (default, sub, wdg-seq,
  3-winding, xscarray, xfmrcode-fetch, makelike); `props.json` regenerated with
  the pinned oracle. `props_roundtrip` green (dump format matched exactly incl.
  `Buses`/`Conns` `[a, b, ]`, `WdgCurrents` `mag, (ang), `, the recomputed
  RDCOhms/NormAmps/EmergAmps).
- Gate: dss-core lib **116** (was 112; +4 transformer), props_roundtrip 1,
  golden_slice 2, golden_smoke 3, dss-parser 62+1, dss-sparse 5. All green.

**WP4.5 — Capacitor — ✅ DONE, gate-green.** Files:
- `src/elements/pd/capacitor.rs` (`TCapacitorObj`): props 1–13 + PD/CktElement
  tails + Like. Two-terminal model (`nterms=2` wye / `nterms=1` delta), three
  `SpecType`s (1=kvar+kV, 2=Cuf+kV, 3=CMatrix). Ported verbatim:
  `PropertySideEffects` (Bus1→default Bus2 = grounded-zero node + clear Bus2
  set-mark; Conn→nterms/nconds; Bus2→shunt/series detect via `StripExtension`;
  NumSteps→array realloc + single-step→multi-step kvar/R/XL split; XL→auto-R
  `|XL|/1000`; Harm→`DoHarmonicRecalc`; States→`FindLastStepInService`),
  `RecalcElementData` (per-phase kV wye/delta branch; FC from kvar using
  `FkvarRating[1]`; `FTotalkvar`; harmonic-filter `FXL`; default Norm/Emerg amps
  unless `*Specified`), `MakeYprimWork` (per-step admittance: wye `:=`/delta
  `AddElement`/cmatrix, series-filter ZL invert-add-invert; **the work matrix is
  reused across steps without clearing, faithful to the Pascal**), `CalcYPrim`
  (accumulate energized steps into shunt-or-series, mirror tiny `×1e-10`
  diagonals into the other matrix, then open-conductor `do_yprim_calcs`),
  `set_NumSteps`/`set_LastStepInService`/`FindLastStepInService`, `MakeLike`.
  5 inline tests: default shape, **YPrim 3φ wye 600 kvar @4.16 (j0.034670858),
  1φ wye 100 kvar @2.4 (j0.017361111), and a 3φ `CMatrix` bank (j5.65e-4 diag /
  -j1.13e-4 off) vs the oracle**, NumSteps kvar split.
  Registered as `ElemKind::Capacitor` after Transformer.
- **Shared engine additions** (reused by Reactor later):
  - `circuit.rs`: `ElemKind::Capacitor` + `shunt_capacitors` list (PD + own,
    mirroring Pascal `AddCktElement` CAP_ELEMENT).
  - `props.rs`: two new `PropType`s — `IntegerArray` (capacitor `States` over
    `NumSteps`, `PropDef::int_array`) and `DoubleSymMatrix` (capacitor `CMatrix`,
    a real lower-triangle sym matrix rendered `(...)` not `[...]`,
    `PropDef::double_sym_matrix`). `base.rs`: `DssObject::{get,set}_i32_array`.
- **Oracle bug (traced to source):** the oracle's `DoubleSymMatrixProperty`
  *getter* (`DSSObjectHelper.pas:2318`) is missing a pointer dereference — it
  reads `@Cmatrix` (the address of the `pDoubleArray` field) reinterpreted as the
  array, instead of `Cmatrix^` (the heap data, the way `ComplexPartSymMatrix`
  does with `PCMatrix(...)^`). The `darray <> NIL` guard never fires (a field
  address is never NIL), so it **always** dumps the pointer bytes + adjacent
  fields as doubles — garbage every time, set or not. The *setter*
  (`:3598`, `PPDouble(dataPtr)^`) is correct, so the matrix is stored fine and
  YPrim is right. We therefore **canonicalize CMatrix to zeros on both sides**:
  `gen_props.py`'s new `zero_garbage` filter rewrites every number in the
  captured value to `0` (magnitude-agnostic — pins only the matrix skeleton), and
  the Rust `DoubleSymMatrix` getter emits a zero matrix of the declared order
  (`TODO(compat)` — clean fix renders the stored array). The real cmatrix→YPrim
  path is covered by the dedicated unit test instead.
  `Set_ConductorClosed`/incremental-Y and `MakePosSequence` not ported (Phase 5+).
- **Goldens:** 7 Capacitor scenarios in `gen_props.py` (default, kvar, cuf,
  cmatrix, numsteps+states, series-XL, makelike), CMatrix zeroed via
  `zero_garbage`; `props.json` regenerated. `props_roundtrip` green.
- Gate: dss-core lib **121** (was 116; +5 capacitor), props_roundtrip 1,
  golden_slice 2, golden_smoke 3, dss-parser 62+1, dss-sparse 5. All green.

**WP4.6 — Reactor — ✅ DONE, gate-green.** Files:
- `src/elements/pd/reactor.rs` (`TReactorObj`): props 1–19 + PD/CktElement tails
  + Like. Two-terminal model (Capacitor/Fault connection rules), four `SpecType`s
  (1=kvar+kV, 2=R+jX [also `Z`/`LmH`], 3=R/X matrices, 4=Z1Z2Z0 sym components).
  Ported verbatim: `PropertySideEffects` (Bus1→default Bus2 = grounded-zero node +
  clear Bus2 set-mark; Bus2→shunt/series via `StripExtension`; Conn→nterms/nconds;
  Phases→nconds/yorder; kvar/Rmatrix/Xmatrix/X/Z/LmH→SpecType; Z1→SpecType 4 +
  Z2/Z0 default-to-Z1; Rp→`RpSpecified`; LmH→`Z.im = L·2π·f`), `RecalcElementData`
  (kvar→`Z.im`/`L` + default amps; R+jX→`L`; `Gp` from `Rp`; parallel-matrix
  `Gmatrix`/`Bmatrix` via `etk_invert`), `CalcYPrim` (GIC <0.5 Hz R-only path;
  wye `:=`/delta `AddElement` for spec 1/2; series-matrix invert-and-stamp for
  spec 3; parallel-matrix G+jB stamp; Z1Z2Z0 build-Z-invert-stamp for spec 4;
  shunt diagonal mirror with the 1φ-grounding-reactor exception), `MakeLike`.
  4 inline tests: default shape + **YPrim vs oracle for 3φ kvar (−j0.00321542),
  3φ Z1Z2Z0, and 3φ RMatrix/XMatrix series**.
  Registered as `ElemKind::Reactor` after Capacitor.
- **Reused (no new shared machinery):** `PropType::Complex` (Z1/Z2/Z0/Z),
  `DoubleSymMatrix` (RMatrix/XMatrix — same always-garbage oracle getter as
  Capacitor's CMatrix, see below), `object_ref`+`NOT_PORTED` (RCurve/LCurve →
  XYcurve, Phase 5 WP5.1), `etk_invert` (parallel-matrix inverse). Confirmed the
  shared `Complex` getter renders `[%g, %g]` exactly (updated its stale
  `TODO(phase4)` note to "verified"). `circuit.rs`: `ElemKind::Reactor` +
  `reactors` list (PD + own).
- **Same oracle bug as Capacitor:** `RMatrix`/`XMatrix` use the broken
  `DoubleSymMatrixProperty` getter (always denormal garbage), so all 10 Reactor
  scenarios `zero_garbage` both; the matrix→YPrim path is covered by the unit
  test instead. `TODO(compat)` for the truncated `CALPHA = (-0.5, -0.866025)`
  literal (DSSGlobals.pas:74) used in the Z1Z2Z0 off-diagonals.
- **Goldens:** 10 Reactor scenarios in `gen_props.py` (default, kvar, rx, z, lmh,
  z1z2z0, matrix, parallel, rp, makelike); `props.json` regenerated.
  `props_roundtrip` green.
- `MakePosSequence`/`GetLosses`-Rp-branch not ported (Phase 5+, control/reporting).
- Gate: dss-core lib **125** (was 121; +4 reactor), props_roundtrip 1,
  golden_slice 2, golden_smoke 3, dss-parser 62+1, dss-sparse 5. All green.

**Next:** WP4.7 — ControlElem base + RegControl/CapControl (parse-only). See
PHASE4_PLAN §WP4.7.

---

## 2. What Phase 3 built (file by file)

### Circuit model (`src/circuit/`)
- `bus.rs` — slim `Bus` (`TDSSBus`): `nodes`/`ref_no` allocation lists,
  `kv_base`, coords, `allocate_bus_state`. Node allocation itself lives in
  `Circuit::add_bus` (it needs the global counter).
- `circuit.rs` — `Circuit` (`TDSSCircuit` subset): `add_ckt_element` (device
  list + per-kind `Vec<ElemRef>` lists + 1-based handle), `add_bus`
  (find-or-create + the "Caution: Magic" node_buffer→global-ref rewrite),
  `process_bus_defs` (defaults 1..np then ground; `parse_as_bus_name`
  override; negative-node check), `reprocess_bus_defs` (save → rebuild →
  `allocate_bus_state` → restore kv_base/coords/voltages),
  `set_bus_name_redefined` (raises `system_y_changed`, like the Pascal
  property setter; the ctor starts with the flag **true**), `node_name(i)` =
  the oracle's `YNodeOrder` format (`BUSNAME.N` uppercased), `losses`.
- `terminal.rs` — `Terminal` (`TPowerTerminal`): `term_node_ref`,
  `conductors_closed`, `bus_ref`.

### Element base (`src/elements/`)
- `ckt.rs` — `CktElementData` (`TDSSCktElement` fields): nphases/nconds/
  nterms/yorder, `node_ref`, `vterminal`/`iterminal`/`inj_current`,
  `yprim`/`yprim_series`/`yprim_shunt`, `set_nterms`/`set_nconds` (realloc
  semantics incl. the nterms=0 trick), `set_bus`/`get_bus` (1-based),
  `set_node_ref`, `compute_vterminal`, `do_yprim_calcs` (open-conductor Kron
  reduction with EPSILON pinning), `set_enabled`, and the **signal flags**
  `signal_bus_name_redefined` / `yprim_invalid` (see §3.2).
- `traits.rs` — `CktElement` trait (`RecalcElementData`/`CalcYPrim`/
  `InjCurrents`/`GetCurrents`/`ComputeIterminal`/`Get_Losses` with the PD
  default = Yprim·Vterminal and the PC compensation override per element);
  `ElemRef{cls,idx}` + `ElemStore` (the executive's class registry seen as
  Pascal's pointer lists); `SysCtx` (snapshot of the `ActiveCircuit.Solution`
  scalars elements read); `InjCtx{node_v, currents}`.
- `pc/vsource.rs` — full `TVsource` port: 2-terminal, ZSpecType 1/2/3 with
  `quad_solver` R0 path, sym/asym Z matrix (`CALPHA.conj()`), `get_vmag`
  (n-phase `2·sin(π/n)` formula), quasi-ideal Yprim branch, `inj_currents`
  via `GetVterminalForSource`, `get_currents = Yprim·V − InjCurrent`,
  spec-type switching side effects with `clear_seq` lists.
- `pc/load.rs` — full `TLoad` port: `set_nominal_load` (status/mode factor,
  per-phase W/var, Yeq/Yeq95/Yeq105/Yeq105I/ILow/I95/M95/IBase), `recalc`
  (LoadSpecType interplay, y_neut incl. 1e6 solid ground, yq_fixed), Yprim
  (wye neutral row/col + `·1.000001`; delta `add_sym`; series diag `1e-10`),
  **all 8 `Do*` models verbatim** with the VBaseLow/95/105 zones, the
  compensation-current `inj_currents`/`get_currents` pair, and the full
  kW/PF/kvar/kVA/xfkVA/kWh/Cfactor spec-set side-effect web.
- `pd/line.rs` — `TLine` sym + matrix paths: Zs=(2Z1+Z0)/3 / Zm=(Z0−Z1)/3,
  `CAP_EPSILON` on the series diagonal, Rg/Xg/rho earth correction with kxg,
  per-property scale functions via `DssObject::prop_scale`
  (GetZSeqScale/GetCSeqScale/GetZmatScale/GetYCScale/GetB1B0Scale), units
  conversion side effects, switch defaults, matrix props killing the sym
  model. `linecode`/`geometry`/`spacing`/`wires`/`cncables`/`tscables` are
  flagged `PropFlags::NOT_PORTED` → setting them is a hard parse error.

### Solution (`src/solution/`)
- `solution.rs` — `Solution` (`TSolutionObj` state incl. both sparse sets +
  `ActiveY` selector) and the free functions taking `(ckt, env)`:
  `solve` (mode dispatcher + growth factor) → `solve_snap` (control loop,
  `Iteration = total` at the end) → `solve_circuit` → `do_pflow_solution`
  (`solve_y_direct` init once) → `do_normal_solution` (the fixed-point loop);
  `solve_direct`, `solve_zero_load_snapshot` (SERIESONLY), `converged`
  (NodeVbase-relative |V| error), `solve_system` (factor+solve via
  dss-sparse), `set_voltage_bases` (zero-load solve + nearest legal base,
  with the 0.001732 `TODO(compat)`), `SolveMode` (ordinals = COM/TSolveMode).
- `ymatrix.rs` — `build_y_matrix`: reprocess buses when redefined → fresh
  `SparseSet` → recalc Yprims (all if `frequency_changed`, else invalid
  only) → stamp enabled elements (`CMatrix::to_row_major()` because
  dss-sparse `add_primitive_matrix` is row-major) → `allocate_vi` →
  `initialize_node_vbase`.

### Executive (`src/exec/mod.rs`) — Phase 2 skeleton replaced
- **Full Pascal name lists** registered: all 123 `TExecCommand` names (incl.
  the `DSS_CAPI_PM` extras) and all 119 `TExecOption` names with the Pascal
  spelling replacements (`vr`→`var`, `tilde`→`~`, `SetOpt`→`Set`, `pct`→`%`,
  `cls`→`class`, `typ`→`type`, `obj`→`object`) — abbreviation ownership now
  matches the oracle for *everything*, not a subset.
- `command()` = `ProcessCommand`: Compile/Redirect early exit; the
  before-circuit command set; the **error-301 circuit gate** for everything
  else; the property-reference fallback (`Line.l1.r1=0.2` style) via
  `SetObject` + rebuilt command line.
- `New circuit.x` → `MakeNewCircuit`: creates the `Circuit` then feeds
  `New object=vsource.source Bus1=SourceBus <remainder>` back through the
  executive. Second circuit → the MaxCircuits=1 error.
- `AddObject`: circuit-element path (`requires_circuit`, duplicate warning
  that **skips the edit**, `add_ckt_element` with `ElemKind`), DSS_OBJECT path
  unchanged.
- `Set`/`Get` (`DoSetCmd`/`DoGetCmd` + the `_NoCircuit` variants): year,
  frequency, mode, random, number, tolerance, maxiterations, loadmodel,
  loadmult, norm/emerg volts, %growth, allowduplicates, zonelock,
  voltagebases (1000-slot `parse_as_vector`), algorithm, controlmode,
  cktmodel, basefrequency, maxcontroliter, casename, log,
  DefaultBaseFrequency, NeglectLoadY, miniterations. Unimplemented options
  record "not ported in Phase 3".
- `Solve` = `DoSetCmd(1)` → `do_solve_cmd` → `solution::solve`;
  `CalcVoltageBases` → `set_voltage_bases`; `BuildY` = invalidate PC
  elements; `Init` = `solution_initialized=false`.
- `Redirect`/`Compile` = `DoRedirect`: path resolution against
  `current_dir` (+`.dss` fallback), Pascal block-comment semantics (`/*` only
  at line start, `*/` anywhere), `solution_abort` → `redirect_abort`,
  compile-keeps-dir / redirect-restores-dir, `@lastfile` parser vars.
- `ClassStore` implements `ElemStore` over the class registry; `SolveEnv`
  gets the aux parser (bus-name parsing is config-identical to the main one).
- **Flag propagation** after each edit: `signal_bus_name_redefined` →
  `Circuit::set_bus_name_redefined(true)`; `yprim_invalid && enabled` →
  `system_y_changed = true` (see §3.2).

### Property engine extensions (`src/obj/`)
- New `PropType`s: `Bus`, `Complex`, `DoubleFArray`, `SymMatrixReal/Imag`,
  `Enabled`, `ObjectRef`. New flags: `SCALED_BY_FUNCTION` (scale from
  `DssObject::prop_scale`), `NOT_PORTED` (hard parse error).
- `DssObject` grew: `as_any`, `as_ckt_element(_mut)`, `set/get_bus_name`,
  `set/get_complex`, `set/get_matrix_part`, `prop_scale`.
- `EnumRegistry` grew: scan/sequence/connection/vsource_model/load_model/
  load_status/line_type/**solve_mode** (26 spellings, COM ordinals)/
  solve_alg/control_mode/random_mode/default_load_model/ckt_model.

### dss-cli
- `main.rs`: `dss-cli <script.dss>` → `Compile`, print errors (exit 1 if
  any), print convergence summary + per-node |V|/angle table.

### Gate artifacts
- `tools/golden/gen_slice.py` — 13 scenarios (2-bus parameterized on the
  load; IEEE13-flat inline-matrix script with lengths converted ft→mi since
  the per-line matrices carry the linecodes' per-mile units).
- `tests/golden/slice.json` — committed golden (YNodeOrder, complex node
  voltages, iterations, converged per scenario).
- `crates/dss-core/tests/golden_slice.rs` — the gate test (a–d above).
- `props_roundtrip.rs` now runs the oracle's preamble (`clear` +
  `new circuit.propsprobe`) before each scenario — `?` is circuit-gated.

---

## 3. Key design decisions & rationale (Phase 3)

### 3.1 Element storage stays in the executive; the solver sees `ElemStore`
Kept Phase 2's `Vec<Box<dyn DssObject>>` per class. The circuit holds
`Vec<ElemRef>` lists (`ckt_elements`, `sources`, `lines`, `loads`, `pc/pd`),
and the solution machinery walks them through the `ElemStore` trait +
`as_ckt_element_mut()` bridge. This replaces Pascal's raw pointer lists with
zero unsafe and no double ownership. (Plan §2.1's typed arenas remain an
option for a later refactor — passing tests are the only contract.)

### 3.2 Signal flags instead of `ActiveCircuit` globals *(load-bearing)*
Pascal property setters write circuit globals mid-edit
(`ActiveCircuit.BusNameRedefined := True`, `Solution.SystemYChanged := TRUE`
via `Set_YprimInvalid`). Elements here set `cd.signal_bus_name_redefined` /
`cd.yprim_invalid`, and the executive propagates **after** the edit loop.
Equivalent because nothing reads those globals mid-edit; verified against the
solve flow (the first reader is `BuildYMatrix` at solve time).

### 3.3 Compensation-current loads
Loads are stamped into Y **and** inject `Yprim·V − model current` (Pascal
`StickCurrInTerminalArray` sign conventions), which is what makes the
fixed-point converge in the same iteration count as the oracle — iteration
equality in the gate is the regression test for this.

### 3.4 Context structs of disjoint borrows
`SysCtx` snapshots the solution scalars for `RecalcElementData`/`CalcYPrim`;
`InjCtx` carries `&[node_v]` + `&mut currents`; `SolveEnv` carries
store/parser/vars/errors. No `RefCell`, no globals.

### 3.5 `NOT_PORTED` property flag
Line's catalog references (linecode/geometry/…) hard-error on set instead of
silently parsing into nothing — a script that needs Phase 4 machinery cannot
produce silently-wrong numbers.

---

## 4. Empirical oracle facts added this phase

- `?`, `Edit`, `~`, `Solve`, `Set`, `Get` are **circuit-gated** in
  `ProcessCommand` (error 301); `New`, `Clear` and a fixed list are not.
  (`gen_props.py` always ran `new circuit.propsprobe` — the replay must too.)
- `Solution.Iterations` (COM) = `Solution.Iteration` = the **total** over
  control iterations, assigned at the end of `SolveSnap`.
- `YNodeOrder` = `MapNodeToBus` order 1..NumNodes = exactly the order
  `ProcessBusDefs` allocates global node refs in element-creation order;
  `YNodeVarray` matches `NodeV[1..]` re/im interleaved.
- A per-line matrix spec has **no separate matrix units**: `units=` applies
  to both the matrices and the length (the original IEEE13 worked because the
  *linecode* carried `units=mi` while the line length was in ft). The flat
  variant therefore converts lengths to miles.
- IEEE13-flat (no regulators/transformers/caps): min |V| ≈ 0.88 pu,
  3 iterations; 2-bus family: 2 iterations.

---

## 5. `TODO(compat)` / deferrals (Phase 3 additions)

- `util.rs`: `CALPHA = (-0.5, -0.866025)` (truncated), `CDOUBLEONE`,
  truncated `0.001732` in `set_voltage_bases`, `pdeg_to_complex` with
  `57.29577951` — all marked `TODO(compat)`.
- Newton algorithm (`Set algorithm=newton`) errors "not ported in Phase 3".
- Harmonics/dynamics/yearly/duty solve modes error out; `is_harmonic_model`
  paths in elements exist but are unreachable until Phase 7.
- Line: `linecode`/`geometry`/`spacing`/`wires`/`cncables`/`tscables`
  properties are `NOT_PORTED` (Phase 4); `Seasons`/`Ratings` stored only.
- Load: loadshape/growthshape/CVRcurve object refs stored as strings only
  (no shape lookup until Phase 5); `interpolate_y95i_ylow` family ported.
- Executive: `Show`/`Export`/`Dump`/`Select`/`Enable`/`Disable`/`Open`/
  `Close`/`BatchEdit`/… record "not ported in Phase 3". `Set hour/sec/time/
  stepsize` need DynaVars wiring (Phase 5).
- `Set DataPath` not ported; `current_dir` only follows Redirect/Compile.

Grep `rg "TODO\(compat\)"` for the full marker list.

---

## 6. How to run / regenerate

```bash
# Gate (must be green before any commit)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Run a script
cargo run -p dss-cli -- path\to\script.dss

# Regenerate goldens (MANUAL ONLY, pinned versions in tools/golden/PIN.txt)
python tools/golden/gen_props.py     # -> tests/golden/props.json   (Phase 2)
python tools/golden/gen_slice.py     # -> tests/golden/slice.json   (Phase 3)
```
Oracle pin: Python 3.12.4, dss-python 0.15.7, dss-python-backend 0.14.5
(the same dss_capi release vendored in `.inputs/dss_capi`). `python` works in
this environment; the `py` launcher is broken — use `python` directly.

---

## 7. Next session — Phase 4 (core PD elements + catalog objects)

> **A full step-by-step execution plan now exists: `PHASE4_PLAN.md`** (work
> packages WP4.1–WP4.10, Pascal line references, design decisions, gate
> procedure). `PHASE5_PLAN.md` is written too. Start from PHASE4_PLAN.md;
> the summary below predates it and is kept for context.

From PORTING_PLAN §Phase 4:
- `Transformer.pas`, `Capacitor.pas`, `Reactor.pas`, `LineCode.pas`,
  `XfmrCode.pas`, the full Line LineCode path; `GrowthShape`, `Spectrum`
  (objects only).
- Introduce the `define_properties!` macro and retrofit the Phase 3 classes
  (the hand-written `match idx` accessor arms in vsource/load/line are the
  natural target).
- **Gate**: `golden_ieee13.rs` / `golden_ieee37.rs` with `controlmode=off`
  (Phase-0 goldens for those feeders are already committed) — voltages/
  powers/losses to 1e-6; property-dump tests for all new classes.

Enabling facts: the `ObjectRef` property type currently stores lowercased
names; LineCode needs real object resolution (executive-level lookup at
parse time, like Pascal's `FetchLineCode`). `ElemKind` needs Transformer/
Capacitor/Reactor variants and `AddCktElement`'s corresponding lists.

---

## 8. Outstanding action

Phase 3 is **complete and gate-green but uncommitted** (working tree on
`phase-2-object-model`, which contains the committed Phase 2). Actions:
1. Commit Phase 3 on this branch (or a new `phase-3-vertical-slice` branch)
   — only on explicit request.
2. Review + merge to `main`.
3. Start Phase 4 (§7).
