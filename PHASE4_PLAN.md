# Phase 4 — Detailed Execution Plan: Core PD Elements + Catalog Objects

> Companion to `PORTING_PLAN.md` §Phase 4. That file holds the binding decisions and
> the one-paragraph phase summary; **this file is the step-by-step execution plan.**
> It is written so that a session with no prior context can execute it work package
> by work package. Read `CLAUDE.md` and `STATUS.md` first, then follow the WPs in
> order. Each WP ends gate-green (`cargo fmt --all --check && cargo clippy
> --workspace --all-targets -- -D warnings && cargo test --workspace`).

## 0. Rules of engagement (non-negotiable)

1. **Pascal is the spec.** Port loop-for-loop where numerics matter; cite the Pascal
   unit/identifier in doc comments (`Pascal \`TTransfObj.CalcY_Terminal\``). Never
   "simplify the math" — the gates compare against the same Pascal engine.
2. **Settle behavior questions empirically.** When the Pascal is ambiguous, probe the
   pinned oracle (`python`, dss-python 0.15.7 — see `tools/golden/PIN.txt` and the
   pattern in `tools/golden/probe_val.py`). Never guess FPC semantics.
3. **`TODO(compat)`** on every deliberately reproduced upstream quirk; never fix them
   now (see PORTING_PLAN.md §4.1).
4. **`PropFlags::NOT_PORTED`** on every property whose machinery belongs to a later
   phase — a hard parse error beats silently-wrong numbers (pattern: Line's
   `geometry`/`spacing` in `crates/dss-core/src/elements/pd/line.rs`).
5. **Goldens are regenerated only manually** with the pinned oracle. Adding *new*
   golden files for this phase is expected; **never touch the Phase 0–3 goldens.**
6. Commit only on explicit user request, never on `main` directly.
7. If a WP turns out to need something from a later WP, reorder locally but keep the
   tree compiling; do not start two WPs in parallel.
8. **Stop-and-confirm cadence (MANDATORY).** After finishing each small step (a WP,
   or a self-contained sub-step within a WP), run the full gate, **update
   `STATUS.md`** to record the new frontier, then **stop and wait for the user's
   explicit confirmation before starting the next step.** Never chain multiple steps
   without confirmation.

## 1. Entry state (what already exists — do not rebuild)

Phase 3 (★ vertical slice) is complete; see `STATUS.md` for the file-by-file map.
Load-bearing pieces this phase builds on:

| Piece | Where | What you get |
|---|---|---|
| Property engine | `crates/dss-core/src/obj/props.rs` | `ClassProps`/`PropDef`/`PropType`/`PropFlags`, generic parse/get (`parse_into`, `edit_property`), scale-by-function, `NOT_PORTED` |
| Object base | `crates/dss-core/src/obj/base.rs` | `DssObjData`, `DssObject` trait (typed accessors keyed by **1-based** Pascal prop ordinal, `side_effects`, `end_edit`, `make_like`, `as_ckt_element(_mut)`) |
| Element base | `crates/dss-core/src/elements/{ckt,traits}.rs` | `CktElementData`, `CktElement` trait, `ElemRef{cls,idx}`, `ElemStore`, `SysCtx`, `InjCtx`, signal flags (`signal_bus_name_redefined`, `yprim_invalid`) |
| Circuit | `crates/dss-core/src/circuit/circuit.rs` | `Circuit`, `ElemKind`, `add_ckt_element`, `process_bus_defs`, per-kind `Vec<ElemRef>` lists |
| Solution | `crates/dss-core/src/solution/` | `solve_snap` (incl. control-loop skeleton + `check_controls`), `build_y_matrix`, `set_voltage_bases` |
| Executive | `crates/dss-core/src/exec/mod.rs` | full command/option name lists, `DssClass` registry, `edit_active`, Redirect/Compile, Set/Get, error-301 gate |
| Harness | `crates/dss-core/tests/harness/` | golden loader + `assert_complex_close` |
| Math | `crates/dss-core/src/support/` | `CMatrix` (invert, Kron via `do_yprim_calcs` pattern), sym components, `line_units::ConvertLineUnits` |

Worked examples to imitate: `elements/general/spectrum.rs` (simple catalog object),
`elements/pd/line.rs` (full PD element incl. PD base props NormAmps=+1, EmergAmps=+2,
FaultRate=+3, PctPerm=+4, Repair=+5 appended after the class props — Pascal
`PDElements/PDClass.pas` `TPDElementProp`), `elements/pc/load.rs` (heavy
side-effect web).

## 2. Phase target and gate

**Scope** (PORTING_PLAN §Phase 4): `LineCode.pas`, `XfmrCode.pas`, `GrowthShape.pas`
(objects only), `Transformer.pas`, `Capacitor.pas`, `Reactor.pas`, the full
`Line.pas` LineCode path, RegControl/CapControl **as parse-only objects** (their
control behavior is Phase 5 — but the IEEE masters *create* them, so they must
parse, resolve their transformer/capacitor, and sit inert), the
`define_properties!` macro + retrofit. `Spectrum` already exists (Phase 2).

**Gate** (all must pass):

- `crates/dss-core/tests/golden_feeders.rs` replays the committed **controls-off
  variant scripts** of IEEE13, IEEE37 and IEEE123 (see WP4.9) against new goldens
  `tests/golden/phase4.json`:
  - converged flag and fixed-point **iteration counts exactly equal**;
  - node order exactly equal;
  - node voltages, per-element terminal powers and currents, total power and total
    losses within **1e-6 rel** (1e-9 absolute floor on voltages — reuse the harness
    `assert_complex_close`);
- `props_roundtrip` extended with default-dump + scripted-edit scenarios for every
  new class (LineCode, XfmrCode, GrowthShape, Transformer, Capacitor, Reactor,
  RegControl, CapControl) — regenerate `tests/golden/props.json` via
  `gen_props.py` (manual, pinned oracle);
- the standard repo gate is green.

## 3. Phase-wide design decisions

### 3.1 Object references resolved at parse time (`ObjectRef` becomes real)

Pascal resolves `linecode=mtx601` **immediately during property parsing**
(`TLineObj.FetchLineCode`, called from PropertySideEffects mid-command), and order
matters: in `LineCode=mtx601 Length=2000 units=ft` the fetch copies the code's
impedances/units *before* the `units=ft` side effect runs its relative conversion.
So resolution cannot be deferred to end-of-edit.

Mechanism (extends the Phase 3 edit path in `exec/mod.rs::edit_active`):

1. Add a read-only view of all *other* classes that is alive during an edit:

   ```rust
   /// Read view of every class except the one being edited.
   pub struct ForeignClasses<'a> {
       left: &'a [DssClass],   // classes[..ci]
       right: &'a [DssClass],  // classes[ci+1..]
       split: usize,           // = ci, to map global class indices
   }
   impl<'a> ForeignClasses<'a> {
       pub fn find(&self, class: &str, obj: &str) -> Option<(ElemRef, &'a dyn DssObject)>;
       pub fn get(&self, r: ElemRef) -> Option<&'a dyn DssObject>;
   }
   ```

   In `edit_active`, produce it with `classes.split_at_mut(ci)` and
   `split_first_mut` (all safe; the active class is the excluded middle element).
   Thread it through `PropEngine` so `props.rs::parse_into` can see it.

2. `PropType::ObjectRef` parsing changes from "store lowercased name" to:
   look up `(target_class, name)` via `ForeignClasses::find` (the target class name
   comes from a new `PropDef` field, e.g. `PropDef::object_ref("LineCode", "linecode")`),
   then call a new `DssObject` hook:

   ```rust
   fn set_object_ref(&mut self, idx: usize, name: String,
                     resolved: Option<(ElemRef, &dyn DssObject)>);
   fn get_object_ref(&self, idx: usize) -> Option<&str>; // for dumps: stored name
   ```

   The element stores the **name string** (for `?`/dump round-trips) plus the
   `ElemRef` (indices are stable — nothing is ever deleted except whole-circuit
   `Clear`, PORTING_PLAN §2.1), and may *immediately copy data* out of `resolved`
   by downcasting via `as_any()` (that is exactly what `FetchLineCode` does).
   On lookup failure reproduce the Pascal error message/behavior — check
   `DSSObjectHelper.pas` for the exact wording and whether the edit continues
   (probe the oracle: `new line.l1 linecode=nosuch r1=0.1` then `? line.l1.r1`).

3. Same-class references (`like=` already works) and the existing string-typed refs
   in Load/VSource (daily/spectrum/...) stay as they are until Phase 5 wires them.

### 3.2 Control elements: in the device list, invisible to Y

`RegControl`/`CapControl` are `TDSSCktElement`s in the device list but never build a
YPrim (`TControlElem.CalcYPrim` is empty; `Ymatrix.pas` skips
`(not Enabled) or (Yprim = NIL)`; `TControlElem.GetCurrents` returns zeros).
Rust: their `CktElementData.yprim` stays `None`; **verify**
`solution/ymatrix.rs` skips `yprim: None` elements (it reads `cd.yprim.as_ref()` —
make the skip explicit and add a unit test). `get_currents` fills zeros.
`calc_yprim` is a no-op. RegControl ctor: `nphases=3, nconds=3, nterms=1`
(Pascal `TRegControlObj.Create`).

### 3.3 `ElemKind` and circuit lists grow

Extend `circuit.rs::ElemKind` with `Transformer`, `Capacitor`, `Reactor`,
`Control`. Mirror Pascal `TDSSCircuit.AddCktElement`'s list dispatch
(`Common/Circuit.pas` — read it before coding): transformers, shunt capacitors and
reactors go to the PD list + their own lists where Pascal has one
(`Transformers`, `ShuntCapacitors`, `Reactors`, `RegControls`, `CapControls`,
`DSSControls`); controls go to `DSSControls` (+ per-class list) and **not** to the
PC/PD lists. Keep the Phase 3 pattern: lists are `Vec<ElemRef>`.

### 3.4 `define_properties!` macro (bounded effort)

Goal: kill the per-class boilerplate of (a) 1-based ordinal consts, (b) the
`ClassProps` table, (c) trivial field-mapped accessor arms. Side effects,
`end_edit`, `make_like` bodies and any non-trivial accessor stay hand-written.

Shape (derive it from the 6 existing hand-written classes; adjust freely):

```rust
define_properties! {
    class GrowthShape, pascal "TGrowthShapeProp";
    obj GrowthShapeObj { data }
    1 NPts      integer    npts: i32                 [SUPPRESS_JSON];
    2 Year      double_array(NPTS) year: Option<Vec<f64>>;
    3 Mult      double_array(NPTS) mult: Option<Vec<f64>>  [REQUIRED_IN_SPEC_SET];
    4 CSVFile   string     csvfile: String           [IS_FILENAME];
    5 SngFile   string     ... [NOT_PORTED];
    6 DblFile   string     ... [NOT_PORTED];
    custom get_string { ... }   // optional escape hatch arms, spliced before the
                                // generated ones
}
```

Expands to: `pub const NPTS: usize = 1; ...`, `pub fn class_props() -> ClassProps`,
and the `get_/set_i32/f64/string/f64_array` match arms inside an
`impl DssObject for ...` skeleton (with `data()/data_mut()/as_any()` included).

**Fallback rule:** the macro is *not* gate-relevant. If after ~half a day it still
fights the existing trait surface, write the new classes by hand exactly like
`spectrum.rs`/`line.rs`, file the macro as a Phase-5 retry, and move on. Retrofit
of Phase 2–3 classes (spectrum, tcc_curve, vsource, load, line) is recommended but
optional; the only contract is the passing test suite.

## 4. Work packages

Execute in order. Estimated relative weight in brackets.

---

### WP4.1 — LineCode catalog object [10%] ✅ DONE (gate-green)

> Status: complete. `crates/dss-core/src/elements/general/line_code.rs` ported and
> registered; 6 inline unit tests + 8 `gen_props.py` LineCode scenarios
> (`props.json` regenerated). Shared engine additions made here:
> `PropFlags::CONDITIONAL_VALUE` + `DssObject::prop_conditional` (sym scalars render
> `----` under a matrix model); sym-matrix getter format fixed to
> `[v |v v |...]`; a deferred-error buffer on `DssObjData`
> (`push_error`/`take_errors`, drained in `edit_active`) so `side_effects`/`EndEdit`
> can emit `DoSimpleMsg` (used by `Kron` on a 1-phase code). `TODO(compat)`:
> LineCode `Repair` defaults to 0 (oracle getter), not the Pascal ctor's 3.


**Pascal:** `General/LineCode.pas` (618 lines). Props `TLineCodeProp` 1–27:
`NPhases, R1, X1, R0, X0, C1, C0, Units, RMatrix, XMatrix, CMatrix, BaseFreq,
NormAmps, EmergAmps, FaultRate, PctPerm, Repair, Kron, Rg, Xg, rho, Neutral, B1,
B0, Seasons, Ratings, LineType` (+ auto `Like`). Key methods:
`PropertySideEffects` (l.349), `EndEdit` (l.403), `MakeLike` (l.420),
`Set_NumPhases` (l.523), `CalcMatricesFromZ1Z0` (l.539), `DoKronReduction` (l.650).

**Rust:** new `crates/dss-core/src/elements/general/line_code.rs`; register in
`elements/general/mod.rs` + the executive class registry (a `dss_object()` class,
like Spectrum — **class registration order matters** for nothing yet, but keep the
Pascal `DSSClassDefs` order: LineCode comes before the circuit element classes).

Steps:
1. Read the whole Pascal unit first. Note the object state: `SymComponentsModel`
   flag, `Z`/`Zinv`/`YC` complex matrices (`CMatrix`), `NeutralConductor`,
   `NumAmpRatings`/`AmpRatings`, `FLineType` (the `line_type` enum from Phase 3 is
   already in `EnumRegistry`), units (reuse `support/line_units.rs`).
2. Port the property table: sym-component props use the same scaled setters Line
   uses (compare with `pd/line.rs` `prop_scale` — LineCode's C1/C0/B1/B0 carry the
   1e-9/µS conversions; copy the *Pascal* scale handling, not Line's, they differ
   in places).
3. Port `Set_NumPhases` (reallocates Z/YC, resets Kron), `CalcMatricesFromZ1Z0`
   (mirror of Line's sym-component fill: Zs=(2Z1+Z0)/3, Zm=(Z0−Z1)/3 — but check
   the Pascal; LineCode has its own copy), `DoKronReduction` (reduces Z and YC by
   the neutral row; sets `NeutralConductor=0`, decrements NPhases — port verbatim
   including its error paths), `PropertySideEffects` (matrix props kill the sym
   model; `Kron=y` triggers reduction in `EndEdit`/side effect — read carefully
   *when* it fires), `MakeLike`.
4. `Seasons`/`Ratings`: store-only (same as Line did in Phase 3).
5. Unit tests inline: 3-phase Z1/Z0 → matrix fill vs hand-computed values; a Kron
   reduction case probed against the oracle (`new linecode.k4 nphases=4 rmatrix=...
   kron=y` then `? linecode.k4.rmatrix` in a probe script — copy
   `tools/golden/probe_val.py` style into a throwaway probe, don't commit goldens
   from it, just transcribe numbers into the unit test with a comment).
6. Extend `tools/golden/gen_props.py` with LineCode scenarios (defaults; sym spec;
   matrix spec; kron; units interplay), regenerate `tests/golden/props.json`,
   make `props_roundtrip` pass.

**DoD:** gate green; props round-trip green incl. new scenarios.

---

### WP4.2 — ObjectRef resolution + Line→LineCode fetch [12%] ✅ DONE (gate-green)

> Status: complete. `ForeignClassesView<'a>` trait + `PropEngine::foreign` thread
> a read view of every class except the active one (built in `edit_active` via
> `split_at_mut(ci)` + `split_first_mut` — zero unsafe) into `parse_into`.
> `PropDef::object_class` distinguishes resolved refs (`object_ref_class`, e.g.
> Line's `LineCode`) from the Phase-3 string-storage refs (`object_ref`, kept for
> Load/VSource shapes until Phase 5). `DssObject::set_object_ref` copies the
> resolved object's data immediately (downcast). `TLineObj.FetchLineCode` ported
> verbatim in `line.rs::fetch_line_code`; `units=` reconverts relative to the
> code's units; `kill_line_code_specified` drops the ref on sym/matrix/switch
> overrides; sym scalars got `CONDITIONAL_VALUE`/`prop_conditional`. Lookup miss
> emits the Pascal 401 message and continues with a NIL ref. Two latent Phase-3
> Line bugs fixed (exposed by the new Line dumps): earth-model default is **DERI
> (3)** not SIMPLECARSON; the `linecode` property's canonical name is **`LineCode`**
> (capitalized, for the 401 text). 4 `gen_props.py` Line+LineCode scenarios added,
> `props.json` regenerated. 2-bus `linecode=` snapshot solves bit-for-bit vs the
> oracle. Gate: dss-core lib **97** (+4 `exec::tests::line_*`), all suites green.

**Pascal:** `PDElements/Line.pas` `TLineObj.FetchLineCode` (l.492–~575) and the
`linecode` arm of `PropertySideEffects` (l.626).

Steps:
1. Implement §3.1 (`ForeignClasses`, `PropDef::object_ref(target_class, name)`,
   `set_object_ref`/`get_object_ref`) and thread it through `edit_active` →
   `PropEngine` → `parse_into`. Keep `make_like` working (it borrows two objects of
   the *same* class — untouched by this change).
2. Convert Line's `linecode` property from `NOT_PORTED` to a real
   `object_ref("LineCode", ...)`. Port `FetchLineCode` verbatim:
   - copies BaseFrequency; copies R1/X1/R0/X0/C1/C0 + `SymComponentsModel` only if
     the code is a sym model, else clears the flag;
   - copies Rg/Xg/rho and recomputes `Kxg = Xg / ln(658.5·sqrt(rho/BaseFrequency))`;
   - `FLineCodeUnits = code.units; FUnitsConvert = ConvertLineUnits(FLineCodeUnits,
     LengthUnits)`;
   - copies NormAmps/EmergAmps/ratings;
   - **zeroes the PrpSequence entries** of spacing/geometry/r1/x1/r0/x0/C1/C0/B1/B0/
     Seasons/Ratings/NormAmps/EmergAmps (the non-compat branch — we follow the
     modern default, `DSS_EXTENSIONS_COMPAT` off; this is what makes `? line.x.r1`
     and dumps match the oracle after a linecode set);
   - the tail (`if Fnphases <> LineCodeObj.FNphases`) re-sizes the line phases —
     read it to the end of the procedure and port it all.
3. `units=` *after* `linecode=` must convert relative to the code's units (the
   Phase 3 units side effect already handles relative conversion — verify against
   the oracle with a probe: linecode in mi, line `units=ft`, dump `rmatrix`).
4. Tests: extend `gen_props.py` with Line+LineCode scenarios (sym code, matrix
   code, code-then-units, code-then-r1-override) and regenerate `props.json`.
   Add a `gen_slice.py`-style numeric probe? Not needed — WP4.9's feeders cover the
   numerics.

**DoD:** gate green; a 2-bus scenario with `linecode=` solves identically to the
oracle (quick manual check via dss-cli + probe script; the committed gate comes in
WP4.9).

---

### WP4.3 — XfmrCode + GrowthShape (catalog objects) [6%]

**Pascal:** `General/XfmrCode.pas` (671 lines, props 1–39 — same winding-property
web as Transformer minus buses/bank/etc.), `General/GrowthShape.pas` (289 lines,
props 1–6: `NPts, Year, Mult, CSVFile, SngFile, DblFile`).

Steps:
1. **Do this WP after WP4.4 has defined the `Winding` struct** if you prefer —
   or define the shared winding data now in a common module
   (`elements/pd/transformer/winding.rs`) and let XfmrCode hold `Vec<Winding>`.
   Pascal duplicates the fields; mirror Pascal (`TXfmrCodeObj` has its own winding
   array) but sharing the struct definition is fine — it's data, not behavior.
2. GrowthShape: object only — store `npts`, `year[]`, `mult[]`; port
   `PropertySideEffects`; the interpolation getter (`GetMult`) is consumed in
   Phase 5 (`set year` + yearly mode), port it now if trivial, else leave a stub
   with a doc note. File-input props: `NOT_PORTED`.
3. XfmrCode: port the property table + `SetNumWindings` + `PropertySideEffects` +
   `MakeLike`. `PullFromTransformer` (l.582) can wait until Transformer exists
   (it is used by `Transformer.XfmrCode` round-trips? — **no**: it is the
   *reverse* direction, used by `New XfmrCode... FromTransformer=`? Check the
   Pascal call site; if unused by parsing, defer with a comment).
4. `gen_props.py` scenarios for both; regenerate `props.json`.

**DoD:** gate green; props round-trip green.

---

### WP4.4 — Transformer [30%, the heart of the phase]

**Pascal:** `PDElements/Transformer.pas` (1845 lines). Props `TTransfProp` 1–49
(see l.88: `Phases, Windings, Wdg, Bus, Conn, kV, kVA, Tap, pctR, RNeut, XNeut,
Buses, Conns, kVs, kVAs, Taps, XHL, XHT, XLT, XSCArray, Thermal, n, m, FLRise,
HSRise, pctLoadLoss, pctNoLoadLoss, NormHkVA, EmergHkVA, Sub, MaxTap, MinTap,
NumTaps, SubName, pctIMag, ppm_Antifloat, pctRs, Bank, XfmrCode, XRConst, X12,
X13, X23, LeadLag, WdgCurrents, Core, RDCOhms, Seasons, Ratings` + PD base + Like).

**Rust:** `crates/dss-core/src/elements/pd/transformer.rs` (+ `winding.rs` if you
split). Register as a circuit class with `ElemKind::Transformer`.

Steps (port in this order; keep the Pascal open side by side):

1. **`TWinding`** (l.~160): `connection, kvll, vbase, kva, pu_tap, rpu, rdcpu,
   rdcohms, rneut, xneut, y_ppm, rdc_specified, tap_increment, min_tap, max_tap,
   num_taps` + `Init()` defaults (read the Pascal `TWinding.Init` l.1363 — e.g.
   `MaxTap=1.10, MinTap=0.90, NumTaps=32, Rneut=-1 (open)`) and
   `ComputeAntiFloatAdder` (l.1357: `Y_PPM = -ppm_factor/(SQR(VBase)·VABase1ph)/2`
   — copy the sign and the /2 exactly from the source).
2. **Object skeleton**: 2-terminal-per-winding model — `nterms = NumWindings`,
   `nconds = Fnphases + 1` per terminal (Pascal sets `NConds := Fnphases + 1`;
   the extra conductor is the neutral brought out per winding). Defaults from the
   ctor: 2 windings, 3 phases, XHL=7%, etc. — transcribe the ctor literally.
3. **`SetNumWindings`** (l.898): reallocates winding array, XSC
   (`(n-1)·n/2` entries — see `XscSize`), TermRef, and the four CMatrices
   (`ZB` order n−1, `Y_1Volt`/`Y_1Volt_NL` order n, `Y_Term`/`Y_Term_NL` order 2n);
   sets `Yorder = nconds·nterms`; invalidates Yprim.
4. **`PropertySideEffects`** (l.620): the winding-editing state machine. `Wdg=`
   sets `ActiveWinding`; `Bus/Conn/kV/kVA/Tap/pctR/RNeut/XNeut` write through to
   `Winding[ActiveWinding]`; the plural forms (`Buses/Conns/kVs/kVAs/Taps`)
   iterate windings (growing `NumWindings` if the array is longer — check!);
   `XHL/XHT/XLT/X12/X13/X23` write XSC slots ÷100 and set `XHLChanged`;
   `XSCArray` writes all slots ÷100; `kVA`/`kVAs` also set NormHkVA/EmergHkVA
   defaults (1.1×/1.5× — verify factors in source); `pctRs` distributes;
   `XfmrCode=` triggers `FetchXfmrCode` (l.2042) — ObjectRef to WP4.3's class,
   copies everything (port verbatim);
   `WdgCurrents` is a **read-only result property** (`GetWindingCurrentsResult`,
   l.~346 — returns formatted mag/angle string; port the `Format('%.7g, (%.5g), ')`
   formatting faithfully, FPC `%g` semantics already exist from the Phase 2 dump
   work); `Core`/`LeadLag` are mapped enums (`LeadLag`: lead/lag/ansi/euro — find
   the enum in the Pascal DefineProperties and add to `EnumRegistry`);
   `Sub`, `SubName`, `Bank`, `Thermal/n/m/FLRise/HSRise`, `Seasons/Ratings`:
   store-only.
5. **`RecalcElementData`** (l.921): ZB/term allocation guarded by `XHLChanged`;
   validation of winding connections (the delta-winding rNeut warning), computes
   `VABase = Winding[1].kVA·1000`, per-winding `VBase` (wye: kVLL/√3·1000 with the
   1-φ/2-φ special cases — **read the exact branch**, it keys off `Fnphases`),
   `ZBase`, `ppm` adders, `DeltaDirection` (the `HVLeadsLV`/LeadLag logic),
   thermal copy, `SetTermRef`.
6. **`SetTermRef`** (l.1127): the winding↔terminal conductor map. Port the loop
   verbatim (it differs for 1-phase vs multi-phase and wye/delta; `DeltaDirection`
   selects rotation). This is pure index bookkeeping — unit-test it directly
   (3-phase 2-winding wye-delta: transcribe expected TermRef from a debug print of
   the Pascal logic done by hand on paper, or probe the oracle's `WdgCurrents`
   format on a known case later; at minimum assert symmetry/coverage invariants).
7. **`CalcY_Terminal`** (l.1859) — the numerics core; port line by line:
   - `freq < 0.51` → GIC path: leave `unimplemented` with an engine-error message
     "not ported (Phase 7/9)" — unreachable at 60 Hz;
   - `ZeroTapFix` inner helper (tap 0 → 0.0001);
   - ZB build: diag `Cmplx(Rmult·(W1.Rpu+Wi+1.Rpu), Freqmult·XSC[i])·ZBase` with
     `ZBase = 1/(VABase/Fnphases)`; off-diag formula with the running `k` index
     over XSC — copy the index dance exactly;
   - `ZB.Invert()`; on inversion error → DoErrorMsg 117 and ZB := EPSILON·I
     (`EPSILON` = the same constant `do_yprim_calcs` uses);
   - Y_1Volt/Y_1Volt_NL assembly via the A/AT incidence vectors (port the loops,
     not the comment's matrix algebra);
   - magnetizing branch: `Y_1Volt_NL.AddElement(2,2, Cmplx(pctNoLoadLoss/100/Zbase,
     -pctImag/100/Zbase/Freqmult))` — **winding 2, hardcoded**;
   - Y_Term/Y_Term_NL via the 2n×n voltage-ratio incidence (`1/(VBase·tap)`,
     `−1/(VBase·tap)`);
   - anti-float adders `(0, W[i].Y_PPM)` on both conductors of each winding;
   - cache `Y_Terminal_FreqMult`.
8. **`CalcYPrim`** (l.1171): rebuild Yprim_Series/Shunt; calls `CalcY_Terminal`
   when frequency changed (or `XRConst`); then `BuildYPrimComponent` (l.1795:
   stamps Y_Term into the phase-expanded YPrim via TermRef) and `AddNeutralToY`
   (l.1754: rneut/xneut grounding branches incl. the rneut<0 open-neutral 1e-12
   leakage — port the constants exactly); then the standard
   `do_yprim_calcs` open-conductor handling from `CktElementData`.
9. **Taps**: `Get_/Set_PresentTap` (l.1385/1393 — clamps to Min/MaxTap, snaps to
   `TapIncrement` steps, sets `YprimInvalid` only when changed). Phase 5's
   RegControl drives this; expose it as a public method on the Rust type.
10. **`GetLosses`** override (l.1635: total = standard, but load/no-load split via
    Y_Term_NL injection — Phase 4 goldens only check total losses; port the
    override anyway, it is short) and `GetAllWindingCurrents` (l.1514) +
    `GetWindingVoltages` (l.1581) + `RotatePhases` (l.1665) — needed by
    `WdgCurrents` dumps now and RegControl in Phase 5.
11. **Inline unit tests**: (a) `SetTermRef` map for 3φ wye-wye and wye-delta;
    (b) YPrim of the IEEE13 `Sub` transformer (3φ, Δ-Y, kvs 115/4.16, XHL=0.008,
    %r tiny) vs oracle — probe with a throwaway script
    (`dss.ActiveCircuit.ActiveCktElement.YPrim` in dss-python gives the matrix);
    transcribe a handful of entries (corner, diag, off-diag) at 1e-9 rel into the
    test with a comment naming the probe;
    (c) tap snap/clamp behavior of `Set_PresentTap`.
12. `gen_props.py` scenarios: defaults; `wdg=` sequencing; plural arrays;
    `XfmrCode=`; `Taps=[...]`; `%LoadLoss` (note: the **property name is
    `%LoadLoss`** — Phase 3's pct→`%` name replacement table must contain it;
    verify `pctLoadLoss → %LoadLoss`, `pctNoLoadLoss → %NoLoadLoss`,
    `pctIMag → %IMag`, `pctR → %R`, `pctRs → %Rs`, `ppm_Antifloat → ppm_Antifloat`
    — read `DefineProperties` l.371 for the authoritative spellings).

**DoD:** gate green; transformer props round-trip; YPrim unit test matches oracle.

---

### WP4.5 — Capacitor [8%]

**Pascal:** `PDElements/Capacitor.pas` (974 lines). Props 1–13: `Bus1, Bus2,
Phases, kvar, kV, Conn, CMatrix, Cuf, R, XL, Harm, NumSteps, States` (+ PD base).

Steps:
1. Spec-type machinery: kvar / Cuf / CMatrix select the spec (`SpecType`);
   `PropertySideEffects` (l.296) keeps kvar↔Cuf consistent and reallocates the
   per-step arrays (`FC, FXL, FR, FHarm, FStates` are **per step**, `Fkvarrating`
   etc. — read the field list at the top of the unit).
2. `Bus2` default: shorted to Bus1's nodes grounded (`Setbus(2, ...)` with `.0.0.0`
   — the side effect builds the bus2 string; port exactly, incl. `Shunt` flag).
3. `set_NumSteps` (l.849 — redistributes ratings across steps), `set_States`
   (l.827), `FindLastStepInService` (l.861), `AddStep`/`SubtractStep` (l.1021/1034
   — Phase 5 CapControl uses them; port now, they're small), `set_LastStepInService`
   (l.878).
4. `RecalcElementData` (l.614) and `MakeYprimWork` (l.905) + `CalcYPrim` (l.695):
   per-step Y assembly with series R/XL when specified, wye vs delta, the
   `Cuf`/`kvar` admittance formulas — port verbatim. `Harm`/`R`/`XL` arrays parse
   even though the harmonic model is Phase 7 (values must round-trip).
5. `Set_ConductorClosed` (l.1055) — capacitor uses terminal closing to implement
   step states; read it to understand `States`, then port.
6. Tests: YPrim probe vs oracle for IEEE13 `Cap1` (3φ 600 kvar @4.16) and `Cap2`
   (1φ); props scenarios in `gen_props.py` (kvar spec, cuf spec, numsteps=3 +
   states, series XL).

**DoD:** gate green; props round-trip.

---

### WP4.6 — Reactor [6%]

**Pascal:** `PDElements/Reactor.pas` (997 lines). Props 1–19: `Bus1, Bus2, Phases,
kvar, kV, Conn, RMatrix, XMatrix, Parallel, R, X, Rp, Z1, Z2, Z0, Z, RCurve,
LCurve, LmH` (+ PD base). None of the gate feeders use Reactor — it is in scope
for completeness and is small.

Steps:
1. Spec types: kvar/kV, R+X (series), Z, Z1Z2Z0 (sym → matrix in `CalcYPrim`),
   RMatrix/XMatrix, LmH. `Parallel` flag and `Rp` (parallel resistance).
2. `RCurve`/`LCurve` reference XYcurve (a Phase 5 class): flag both
   `NOT_PORTED` with a comment pointing at PHASE5_PLAN WP5.1.
3. Port `PropertySideEffects` (l.325), `RecalcElementData` (l.624), `CalcYPrim`
   (l.702 — long; it has separate branches per spec type; port them all, they
   share structure with Capacitor/Line), `GetLosses` (l.1017).
4. Tests: YPrim probe vs oracle for (a) kvar-spec 3φ wye, (b) Z1Z2Z0 spec,
   (c) RMatrix/XMatrix spec; props scenarios.

**DoD:** gate green; props round-trip.

---

### WP4.7 — ControlElem base + RegControl/CapControl (parse-only) [8%]

**Pascal:** `Controls/ControlElem.pas` (153 lines), `Controls/RegControl.pas`
(props 1–32, `RecalcElementData` l.567), `Controls/CapControl.pas` (props 1–23,
`RecalcElementData` l.572).

Steps:
1. `ControlElemData` (embeds `CktElementData`): `element_name`,
   `element_terminal`, `controlled_element: Option<ElemRef>`,
   `monitored_element: Option<ElemRef>`, `time_delay`, `dbl_trace_param`,
   `show_event_log`. Port `Set_ControlledElement`/`Set_MonitoredElement` semantics
   (they maintain back-references in Pascal via `HasControl` flags on the target —
   check what Phase 5 needs and keep it minimal: store the ElemRef; the
   `RemoveSelfFromControlElementList` machinery only matters for element deletion,
   which we don't support).
2. `ElemKind::Control` + circuit lists (§3.3). `yprim: None` forever; zeros
   `get_currents`; no-op `calc_yprim` (§3.2 — add the ymatrix skip test here).
3. RegControl: full property table + `PropertySideEffects` (l.406; `transformer=`
   is an ObjectRef to the Transformer class — resolved via §3.1; `Reset` is an
   action property; `TapNum` get/set maps tap↔integer via `Get_TapNum` l.1150 —
   port now, it only needs Transformer tap accessors), `RecalcElementData` (l.567:
   resolves the transformer, validates winding, sets `nphases/nconds` from it,
   `SetBus(1, <winding bus>)`, allocates VBuffer/CBuffer). `Sample`/
   `DoPendingAction` → record engine error `"RegControl action: not ported until
   Phase 5"` (unreachable under `controlmode=off`).
4. CapControl: same treatment (`element=`, `capacitor=` ObjectRefs; `type=` enum
   current/voltage/kvar/time/pf — add `EnumRegistry` entry; `UserModel/UserData`
   → `NOT_PORTED`, no DLLs ever). `RecalcElementData` resolves both refs and
   validates terminal.
5. `gen_props.py` scenarios for both classes (over a small circuit with a
   transformer + capacitor so the refs resolve — remember scenarios run after the
   `new circuit.propsprobe` preamble).
6. **Node-order check** (the silent killer): adding RegControl objects must not
   change `YNodeOrder` (they attach to existing buses). The WP4.9 feeders verify
   this; if node order diverges, the bug is in how control elements participate in
   `process_bus_defs` — Pascal includes them in the bus-def scan like any element
   (their bus string is set in RecalcElementData).

**DoD:** gate green; props round-trip; IEEE13 master *parses* end-to-end with
no errors up to (not including) `Solve` — manual check via dss-cli on a copy with
solve commented out.

---

### WP4.8 — `define_properties!` macro + retrofit [8%, bounded]

Implement §3.4. Use GrowthShape (or whichever WP4.3 class you wrote last) as the
first consumer, then retrofit Spectrum and TCC_Curve (smallest), then — only if the
macro holds up — Capacitor/Reactor. **Time-box it**; the fallback rule in §3.4
applies. The retrofit must be purely mechanical: `git diff` of `props.json`
round-trip stays empty, no golden regeneration.

**DoD:** gate green; at least 2 classes use the macro (or a documented fallback
note in STATUS.md if abandoned).

---

### WP4.9 — Controls-off variant scripts, goldens, gate test [10%]

This is the phase gate. Both engines must run **the same committed script**.

1. **Variant generation** (new `tools/golden/gen_phase4.py`):
   For each of `13Bus/IEEE13Nodeckt.dss`, `37Bus/ieee37.dss`,
   `123Bus/IEEE123Master.dss`:
   - read the master line by line;
   - **drop** lines whose first token, case-insensitively, is `solve` or
     `buscoords` (BusCoords is a Phase 8 command; it is numerically irrelevant);
   - **rewrite** `redirect <f>` / `compile <f>` arguments to be relative to the
     variant's own directory (`tests/golden/phase4/`), i.e. prefix
     `../../../.inputs/electricdss-tst/Version8/Distrib/IEEETestCases/<case-dir>/`
     (compute, don't hardcode; nested redirects inside the corpus — e.g.
     `13Bus/IEEELineCodes.DSS` → `../IEEELineCodes.DSS` — need **no** rewriting
     because the current-dir follows the redirect chain);
   - **append** two lines: `Set controlmode=OFF` and `Solve`;
   - write to `tests/golden/phase4/<name>_controlsoff.dss` (committed).
2. **Golden capture** (same script): compile each variant with the pinned oracle
   (`dss.Text('compile <abs path>')`), then record into
   `tests/golden/phase4.json` (schema 1, one scenario per feeder):
   `script` (repo-relative path), `converged`, `iterations`
   (`dss.ActiveCircuit.Solution.Iterations`), `node_order` (`YNodeOrder`),
   `v_re`/`v_im` (`YNodeVarray`), `total_power` (`TotalPower`), `losses`
   (`Losses`), and per-element `powers`/`currents` for **every** element
   (`First/Next` iteration — copy the snapshot helpers from
   `tools/golden/generate.py`). Mind the dss-python pin check (reuse `check_pin`).
3. **Rust gate test** `crates/dss-core/tests/golden_feeders.rs`:
   loads `phase4.json`; for each scenario builds the absolute script path from
   `CARGO_MANIFEST_DIR`, runs `dss.command("compile \"<path>\"")`, asserts no
   engine errors, then: converged + iterations **exact**, node order exact,
   voltages / element powers / element currents / total power / losses at
   **1e-6 rel** via the harness. Element iteration order must match the oracle's
   First/Next order — that is creation order; assert on names, not just values.
4. **Expected debugging order** when a feeder mismatches (it will):
   voltage divergence localizes by node → find the element whose Yprim differs →
   probe that single element's YPrim in dss-python vs a unit test. Iteration-count
   mismatch with matching voltages → convergence/loads path, compare
   `set_nominal_load` factors. Node-order mismatch → bus-def processing order
   (check control elements, WP4.7 step 6).

**DoD:** all three feeders pass at the listed tolerances; full repo gate green.

---

### WP4.10 — Phase exit

1. Sweep: `rg "TODO\(compat\)"` — every new quirk marked; `rg "NOT_PORTED"` —
   every deferral points at its phase.
2. Rewrite `STATUS.md` (same structure as the Phase 3 rewrite: what was built
   file-by-file, design decisions, oracle facts learned, deferrals, "next =
   Phase 5 → PHASE5_PLAN.md").
3. Full gate. Commit **only on explicit user request** (suggest branch
   `phase-4-pd-elements` if Phase 3 has been merged by then; otherwise stack on
   the existing branch as directed).

## 5. Deferred in this phase (mark `NOT_PORTED` where reachable)

- Reactor `RCurve`/`LCurve` (XYcurve — Phase 5 WP5.1).
- CapControl `UserModel`/`UserData` (never — no DLL loading in safe Rust; error).
- Transformer GIC path (`frequency < 0.51`), harmonics interplay (Phase 7).
- GrowthShape/XfmrCode file-input props (`CSVFile`/`SngFile`/`DblFile`) unless the
  corpus needs them (it doesn't for the gate feeders).
- `BusCoords` command (Phase 8; stripped from variants). The Phase 5 gate compiles
  the *unmodified* masters, which do call `BusCoords` — so Phase 5 ports it
  (see PHASE5_PLAN WP5.8); leave the executive stub recording "not ported" for now.
- RegControl/CapControl `Sample`/`DoPendingAction` (Phase 5 WP5.5/5.6).
