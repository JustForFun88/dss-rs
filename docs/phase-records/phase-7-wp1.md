# Phase 7 — WP7.1 (Line constants & geometry) — archived record

> **Archived from `STATUS.md` (moved 2026-06-22)** to keep the live handoff lean.
> WP7.1 is **complete and gate-green** on the `phase-7-extended-elements` branch;
> these are the frozen per-step records, superseded only by the code and tests.
> The live `STATUS.md` §1e keeps a one-line-per-step summary plus the tracked-open
> plural-cable divergence and a pointer here; the §3/§4/§5 cross-references below
> resolve against `STATUS.md` (the numbering is preserved there). Plan:
> `PHASE7_PLAN.md` §WP7.1.

---

**WP7.1 step 1 — Carson line-constants engine (all 4 specializations) — ✅ done, gate-green, committed.**
- `src/support/line_constants/` — `mod.rs` (`LineConstants` = Pascal
  `TLineConstants`, `General/LineConstants.pas`) + the **four specializations**
  the plan calls for: `oh.rs` (`OhLineConstants`, a plain alias — `TOHLineConstants`
  adds nothing), `cable.rs` (`TCableConstants` shared cable data + its
  `ConductorsInSameSpace` override), `cn.rs` (`CnLineConstants` =
  `TCNLineConstants`), `ts.rs` (`TsLineConstants` = `TTSLineConstants`). No Rust
  inheritance: a `LineConstantsKind` enum (Overhead/ConcentricNeutral/TapeShield)
  on the one struct switches `Calc`/`ConductorsInSameSpace`; the cable arrays are
  allocated only for the cable kinds (`new`/`new_cn`/`new_ts` constructors). Pure
  math engine beside the other `support/` helpers; reuses `support/cmatrix/mod.rs`
  (Kron, invert), `support/line_units/mod.rs`, `support/mathutil/mod.rs`
  (`bessel_i0`/`bessel_i1` for the DERI skin-effect `Zint`). 0-based indices.
- Ported verbatim: base `Calc(f, EarthModel)` (self/mutual Z, the P→invert→Yc
  path), `Get_Zint` (SimpleCarson/FullCarson no-skin vs DERI Bessel skin effect),
  `Get_Ze` (all three earth models — SimpleCarson, FullCarson Tleis series, DERI
  complex earth factor `Fme`), `Kron`/`Reduce`, the unit-converting
  `z_matrix`/`yc_matrix` getters, GMR↔radius defaulting setters, and overhead
  `conductors_in_same_space`. **CN `Calc`** (`CNLineConstants.pas`): append the
  concentric neutrals as extra conductors, build with the strand
  resistance/GMR/`RadCN` power-mean spacing, Kron the neutrals out, build Yc
  directly as the coaxial insulation admittance. **TS `Calc`**
  (`TSLineConstants.pas`): same shape with the tape-shield resistance/GMR.
  **Cable `ConductorsInSameSpace`**: no height check, `0.5*DiaCable` radius for
  neutral conductors. (`TCableConstants.Kron` is identical to the base, so it is
  not re-implemented.)
- **`TODO(compat)`:** truncated upstream constants `mu0 = 12.56637e-7`,
  `Twopi = 6.283185307` (a *distinct* quantity from `2·PI` — FullCarson/Zint use
  full `std::f64::consts::PI`), `e0 = 8.854e-12`; plus the tape-shield `0.3183`
  (truncated `1/pi`) in `ts.rs`. All carry `#[allow(clippy::approx_constant)]`.
  Zero-pivot in `invert`/`kron` unchecked (existing cmatrix `TODO(compat)`).
- **Precondition documented** on `calc`: geometry must be filled first
  (`Rdc`/`radius`/`GMR` init to the `-1.0` sentinel → non-finite entry, not an
  error, if left unset — the geometry layer / `ConductorData`'s `Rdc = Rac/1.02`
  default is responsible). Data flow for steps 2–3:
  `TLineGeometryObj.UpdateLineGeometryData` sets the engine arrays from the wire
  objects (incl. the CN/TS cable fields), then `Calc(f, ActiveEarthModel)` + an
  optional `Reduce`; `capradius` defaults to `radius`.
- **Gate:** 17 inline tests pinned against the dss-python oracle (PIN.txt 0.15.7 /
  backend 0.14.5), probed (via `tools/golden/probe_line_constants_phase7.py`) by
  building the geometry through a `Line` and reading `Rmatrix`/`Xmatrix` (ohm/m) +
  `Cmatrix` (nF/m): 3-phase overhead under **all three earth models** + a 4→3 Kron
  reduce; **3-phase CN cable and TS cable** each under **all three earth models**
  (full Z + coaxial C); **CN and TS cable** at a **non-power-frequency** (f = 5 kHz
  → the radius/`Zi.im`-retained branch, Z only — the oracle `Cmatrix` getter scales
  reported nF by the solve frequency); a **CN cable 4→3 Kron reduce** (3 phases + a
  bare-neutral core, the cable reduced-Z/Yc + `reduced_size>0` re-reduce path); a
  **non-power-frequency** overhead case; a **rho_earth = 200** recalc
  (`set_rho_earth` + `z_matrix` `frho_changed` path); a **unit/length conversion**
  (ohm·km over 2 km); overhead + cable `ConductorsInSameSpace`. Entry-by-entry at
  1e-8 rel. dss-core lib **319 → 336**. Full three-command gate green.
  - *(audit-tests follow-up)* The earlier suite ran the cable `Calc` under DERI /
    60 Hz / unreduced only; the 6 added cable tests close the earth-model,
    high-frequency, and reduction branch gaps the test audit flagged.
- **Deferred (tracked):** the units-converting per-conductor *read* getters
  (`Get_GMR`/`radius`/`Rdc`/`Rac`/`X`/`Y`/`Capradius`) are not ported — they have
  no consumer until the LineGeometry report/dump path; they land in step 3 with it.

**WP7.1 step 2a — conductor catalog (`WireData`/`CNData`/`TSData`) — ✅ done,
gate-green, committed.**
- `src/elements/general/conductor_data/mod.rs` (new) — port of Pascal
  `General/{ConductorData,WireData,CNData,TSData,CableData}.pas`. Pascal's type
  hierarchy is `TConductorDataObj → TWireDataObj` and `TConductorDataObj →
  TCableDataObj → TCNDataObj/TTSDataObj`; Rust has no inheritance, so the shared
  blocks live in two private cores — `ConductorDataCore` (the 13 `TConductorData`
  props: Rdc/Rac + units, GMR, radius/diam, norm/emerg amps, Seasons/Ratings,
  CapRadius, with the full side-effect web) and `CableDataCore` (EpsR/InsLayer/
  DiaIns/DiaCable + error checks). Each concrete class embeds the cores it needs
  and maps its own 1-based ordinal onto the relevant block.
- **Property order matches the oracle exactly** (probed): a leaf's own props
  lead, then `CableData`, then `ConductorData` — the Pascal `inherited
  DefineProperties` chain. So WireData = 13 ConductorData props; CNData = 4 own
  (k/DiaStrand/GMRStrand/RStrand) + 4 cable + 13 conductor = 21; TSData = 3 own
  (DiaShield/TapeLayer/TapeLap) + 4 cable + 13 conductor = 20. Each via
  `define_properties!` (ordinals inline — the leaves differ, so the conductor
  table is repeated per class rather than shared by a fn).
- **Side-effect web ported verbatim:** Rac↔Rdc (`×1.02`/`÷1.02`), GMRac→radius
  (`÷0.7788`) + radius-zero error, radius/diam→GMR (`×0.7788`) + CapRadius default,
  GMRunits↔radunits seeding, normamps↔emergamps (`×1.5`), Seasons→`AmpRatings`
  resize; CN DiaStrand→GmrStrand (`0.7788·0.5·Dia`); the critical-error checks
  (k<2, EpsR<1, Ins/Dia/shield/tape positivity, TapeLap∈[0,100]) as deferred
  messages. `diam` shares the radius field via the engine's 0.5 prop scale
  (`Diam` dumps `FRadius/0.5`). **`MakeLike` copies neither `NumAmpRatings` nor
  `AmpRatings`** (Pascal quirk) — a `like=` conductor keeps its own `[ -1]`.
- Defaults reproduced exactly (probed): every spec field inits to the `-1.0`
  sentinel (so a bare WireData dumps `Rdc=-1 … Diam=-2 Ratings=[ -1]`), units
  ordinal 0 dumps `none`, `EpsR=2.3`, `TapeLap=20`, `k=2`. Registered in
  `exec::Dss::new` after the shape classes (Pascal DSSClassDefs.pas registers
  WireData/CNData/TSData after Spectrum, before LineGeometry).
- **Gate:** 8 inline tests (diam/dynamic-default couplings, GMR-seeds-radius,
  MakeLike-skips-ratings, CN strand-GMR default, the k<2 / EpsR<1 / TapeLap-range
  error paths, TS defaults) + 14 oracle-pinned `props.json` scenarios across new
  `props/{wiredata,cndata,tsdata}.json` (default/full/abbrev/diam/gmr-only/
  ratings/makelike for wire; default/full/strand-default/makelike for CN;
  default/full/makelike for TS); `props_roundtrip` green. dss-core lib **336 →
  344**. Full three-command gate green.
- **Deferred (tracked):** the units-converting per-conductor *read* getters
  (`Get_GMR`/`radius`/`Rdc`/`Rac`/`X`/`Y`/`Capradius`) land in step 3 with the
  LineGeometry data-flow.

**WP7.1 step 2b — `LineSpacing` (`TLineSpacingObj`) — ✅ done, gate-green, committed (`620cf89`).**
- `src/elements/general/line_spacing/mod.rs` (new) — port of Pascal
  `General/LineSpacing.pas`. A `DSS_OBJECT` catalog class: 5 props (`nconds`
  [SuppressJSON], `nphases`, `x`, `h` [both `DoubleVArray` sized by `FNConds`
  via `PropertyOffset2 = @FNConds`], `units` [mapped string enum]). Registered
  after `TSData`, before `LineGeometry` (Pascal `DSSClassDefs.pas`).
- **Side effects ported verbatim:** the `nconds` setter reallocates `FX`/`FY`
  to the new count (grown tail zero-filled — Pascal's `ReAllocmem` leaves it
  uninitialized, undefined memory the goldens do not pin) and resets `Units` to
  `ft`; `MakeLike` copies `FNConds`/`NPhases`/`FX`/`FY` then `Units :=
  Other.Units` (overriding the side-effect's ft reset).
- `nconds=0` empties the buffers: Pascal `ReAllocmem(FX, 0)` nils the pointer,
  so `GetDSSArray` returns `''` (not `'[]'`) — `get_f64_array` mirrors this by
  reading an empty coordinate array as nil (audit follow-up; was `'[]'`).
- Oracle-pinned: 14 `linespacing_*` `props.json` scenarios (default, full,
  units=m, array clamp/zero-fill, shrink-nconds truncation+units-reset, makelike,
  zero-nconds empty-string, units mi/kft/km/none/in/cm/mm) + 7 inline unit tests;
  `props_roundtrip` green. dss-core lib **344 → 351** (audit follow-ups added the
  `nconds_grow_*` zero-fill invariant, the `nconds=0`→`''` fix + golden, and the
  negative-`nconds` clamp invariant test — the oracle raises on negative
  `nconds`, so that path is Rust-only). Full three-command gate green.
- **Audit-tests follow-up (`/audit-tests` step 2b, `b5d5201`):** rounded the units
  golden out to **all 9 `LineUnits` ordinals** — added `linespacing_units_{in,cm,mm}`
  (the only Minor finding; the per-class plumbing was already covered by 6
  ordinals + the full mapping by `dss_enum/mod.rs`). Regenerated with the pinned
  oracle; only `linespacing.json` changed (11 → 14). The two Rust-only invariant
  tests (`nconds_grow_*`, negative-`nconds`) needed no change — documented
  divergences with no oracle to pin. Gate green.

**WP7.1 step 2c-i — `LineGeometry` (`TLineGeometryObj`) object + edit state
machine — ✅ done, gate-green, committed.**
- `src/elements/general/line_geometry/mod.rs` (new) — port of Pascal
  `General/LineGeometry.pas` (the object, props, side-effect web, `MakeLike`).
  19 props via `define_properties!` in the exact oracle order
  (`nconds`/`nphases`/`cond`/`wire`/`x`/`h`/`units`/`normamps`/`emergamps`/
  `reduce`/`spacing`/`wires`/`cncable`/`tscable`/`cncables`/`tscables`/`Seasons`/
  `Ratings`/`LineType`). Registered after `LineSpacing` (Pascal
  `DSSClassDefs.pas`).
- **Per-conductor state machine** keyed by `cond=` (`FActiveCond`): `wire=`/
  `cncable=`/`tscable=`/`x=`/`h=`/`units=` route to the active conductor's slot
  internally (no engine change — the active index is object state). `cond` is
  range-guarded `1..=NConds` (Pascal `set_ActiveCond` ignores out-of-range; the
  generic CAPI struct-index error is not reproduced, the transformer `wdg`
  precedent). Units are sticky via `FLastUnit`. `spacing=` copies a
  `LineSpacing`'s coordinates into every conductor (and clears the `X`/`H` set
  marks); `wires=`/`cncables=`/`tscables=` fill all slots (Pascal `SetWires`,
  count-validated, the `AllowAllConductors`/JSON branch skipped as JSON-only).
  Conductor/spacing refs are resolved + **snapshot-cloned** at edit time
  (WP4.2 `FetchLineCode` pattern); the cloned conductors seed `NormAmps`/
  `EmergAmps`/`NumAmpRatings`/`AmpRatings` from the first conductor.
- **New property kind:** `PropType::ObjectRefArray` + `PropDef::object_ref_array`
  + `set_object_ref_array`/`get_object_ref_names` (the `wires`/`cncables`/
  `tscables` `DSSObjectReferenceArrayProperty`; renders `[a, b, c]`, empty `[]`).
- Oracle-pinned: 7 `linegeometry_*` `props.json` scenarios (default —
  `X`/`H`/`Units` skipped, the oracle raises on the unallocated `NConds=0`
  arrays; overhead cond/wire + reduce; spacing form; CN cable; makelike;
  multi-season ratings default; buried-neutral `cncable`+`wires=`) + 10 inline
  unit tests + an exec parse-path test; `props_roundtrip` green. dss-core lib
  **351 → 362**. Full three-command gate green.
- **Audit follow-up (`/audit-code` step 2c-i):** three findings settled against
  the pinned oracle and fixed. (1) The `wire`/`cncable`/`tscable` side effect now
  logs the Pascal 10103 "WireData/CNData/TSData object was not defined" when the
  active conductor is NIL (the generic ObjectRef 401 stays — upstream emits
  both). (2) `PropType::ObjectRefArray` now `Exit`s on the first unresolved token
  (Pascal `DSSObjectHelper` array property), so a bad name no longer drops the
  token and trips a spurious "Unexpected number" count error. (3) The
  `NumAmpRatings>1`/`AmpRatings` ratings-default branches (and the `cond`
  out-of-range clamp) were untested — added the two goldens above plus the
  exec/inline tests; the conductor invariant `NumAmpRatings == len(AmpRatings)`
  makes the array-branch `take(n)` copy identical to Pascal's full-length copy
  (no code change needed there).
- **Audit-tests follow-up (`/audit-tests` step 2c-i, `2b54849`):** the test
  audit found the **tape-shield path had zero executable coverage** (no test ever
  wrote `tscable=`/`tscables=`, so the `conductor_amps` `TsDataObj` arm was dead)
  plus minor happy-path-only gaps. Added 4 oracle-pinned scenarios (linegeometry
  7 → 11), each settled empirically against the pinned oracle first:
  `linegeometry_ts` (scalar `tscable=` + `linetype=ug_ts` — exercises TSData
  resolution, TapeShield kind, the `TsDataObj` amps default, and a non-default
  LineType); `linegeometry_nphases_gt_nconds` (NPhases stored raw at parse —
  confirms the `FLineData.Nphases` clamp is correctly deferred to 2c-ii);
  `linegeometry_seasons_direct` (the `Seasons` resize side effect); and
  `linegeometry_normamps_explicit` (explicit amps survive a later conductor).
  Also corrected the inline `scalar`-helper doc comment, which over-claimed full
  resolution coverage. `props_roundtrip` green (now exercises the TS path).
- **Surfaced, NOT fixed (needs investigation, tracked):** the **plural cable**
  forms `cncables=`/`tscables=` leave the oracle's active conductor at `Cond=1`
  for a bare assignment, whereas Rust's `set_wires` leaves it at `istop` (and
  overhead `wires=` stays at `istop` in the oracle too). The exact rule is
  intricate — after a prior `cond=2 cncable=`, a following `cncables=[…]` reads
  `Cond=2` (a buried-`istart` count-error Exit), and the **vendored
  `LineGeometry.pas` SetWires/ChangeLineConstantsType do not contain this reset**
  (source says `istop`), so the pinned 0.14.5 binary diverges from the vendored
  revision here. Porting it faithfully needs that reconciliation, so it was not
  guessed/hacked and no failing golden was added; the plural-cable forms stay
  un-pinned for now (their CN/TS *data* paths are covered via the scalar
  scenarios). Empirically probed against the oracle (`/audit-tests` follow-up).
**WP7.1 step 2c-ii — `LineGeometry` matrix wiring (`UpdateLineGeometryData`/
`CalcMatrices`) — ✅ done, gate-green, committed `0258911` (+ audit-code
follow-up, gate-green).**
- `line_geometry/mod.rs` now holds a real `FLineData: Option<LineConstants>` Carson
  engine (replacing the placeholder `fline_kind` tracker). Ported:
  - `change_line_constants_type` — the Pascal `needNew` allocate/swap (kind ≠ the
    active conductor's choice, or `FLineData` absent / wrong conductor count),
    preserving `Nphases`/`RhoEarth` across the swap; allocates only the three
    concrete kinds (an `Unknown` request leaves the engine, as Pascal).
  - `realloc_conductors` (the `nconds` side effect) now rebuilds a fresh overhead
    engine sized `FNConds` (Pascal's per-conductor `ChangeLineConstantsType`
    loop), `None` when `NConds=0` (Pascal NIL).
  - the `nphases` side effect clamps `FLineData.Nphases` to `min(NPhases, NConds)`
    (the previously-deferred clamp; `UpdateLineGeometryData` later re-sets it
    unclamped, as upstream).
  - `update_line_geometry_data(f, earth_model)` — pushes every conductor's
    geometry into the engine (X/Y in `FUnits`, radius/capradius/GMR/Rdc/Rac, and
    the CN/TS cable extras), sets `Nphases`, clears `data_changed`, runs
    `ConductorsInSameSpace` → `Calc(f, earth_model)` → `Reduce` (if `FReduce`).
    Returns `Err` for the two Pascal abort paths: a NIL conductor slot
    (`raise Exception` "not correctly initialized") and a failed geometry check
    (`ELineGeometryProblem`/`SolutionAbort`).
  - `z_matrix`/`yc_matrix`/`rho_earth`/`set_rho_earth` — the `Get_Zmatrix`/
    `Get_YCmatrix`/`Get_/Set_RhoEarth` accessors (recompute when `data_changed`),
    the public surface step 3's Line consumes.
- **Conductor catalog** (`conductor_data/mod.rs`): new `ConductorGeom`/`CableGeom`
  + `geom()` on each of `WireData`/`CNData`/`TSData` + a `conductor_geom(&dyn)`
  dispatch — the engine inputs Pascal reads off `FWireData[i]` (in the object's
  own unit codes; the engine converts).
- **`MakeLike` divergence (documented):** Pascal rebuilds an overhead engine then
  runs `UpdateLineGeometryData`, which for a *cable* source raises `EInvalidCast`
  (FLineData overhead, conductors CN/TS). We instead **clone the source engine**
  so the kind matches and defer the recompute (`data_changed=true`); observable
  props are unchanged (no matrix props are dumped), so `props_roundtrip` is
  unaffected. Not a `TODO(compat)` (no golden pins it; it averts an upstream
  crash on an untested path).
- Oracle-pinned: 3 new inline matrix tests drive the full object path
  (`nconds`/`cond`/`wire`/`x`/`h`/`units`) and assert Z/Yc against the **same
  dss-python references** the Carson-engine unit tests pin —
  `matrices_overhead_match_oracle` (3-phase OH, DERI, Z + full C),
  `matrices_reduce_neutral_to_phases` (4→3 Kron reduce), `matrices_cn_cable_
  match_oracle` (CN cable param transfer) — plus 2 error-path tests
  (`update_uninitialized_conductor_errors`, `update_conductors_in_same_space_
  errors`). `LineConstants` gained `#[derive(Clone)]`. dss-core lib **362 → 367**.
  Full three-command gate green.
- **Audit-code follow-up (committed separately):** (1) `change_line_constants_type`
  `needNew` restored to Pascal's exact boolean **OR** (choice-changed *or*
  engine-NIL/wrong-count) — previously a `match` that skipped the NIL/count clause
  when the active choice was unchanged; behaviourally identical under the realloc
  invariant, now a literal 1:1 port that self-heals if the invariant is ever broken.
  (2) Test coverage closed for the paths 2c-ii added but left unexercised:
  `matrices_ts_cable_match_oracle` (the **TS** object→engine transfer — DiaShield/
  TapeLayer/TapeLap — vs the engine `ts_cable_deri_3cond` reference),
  `make_like_cn_cable_recomputes` (pins the documented MakeLike clone divergence:
  `like=` a CN geometry does not crash and reproduces the source Z), and
  `z_matrix_recomputes_on_frequency_change` (guards `f` forwarding / the engine's
  `f != FFrequency` recompute branch — all prior matrix tests used 60 Hz only).
  dss-core lib **367 → 370**. Gate green. *(Not addressed: the `Get_Zmatrix`/
  `Get_YCmatrix` pre-existing-`SolutionAbort` NIL gate — it needs the solution
  handle the geometry object lacks; correctly belongs to the step-3 `Line`
  consumer and is tracked there.)*
- **Audit-tests follow-up (`/audit-tests` step 2c-ii, committed `6fa2f1c`):**
  strengthened the matrix unit tests where the object→engine *forwarding* of the
  `z_matrix`/`yc_matrix` args was under-exercised — every prior matrix test used
  `length = 1`, `units = m`, `earth_model = DERI`, so only `f` was proven to reach
  the engine. Added `matrices_overhead_km_scaled` (length = 2 / units = km ⇒ Z/Yc =
  the per-meter result × 1000 × 2 — catches a hardcoded 1.0/meters or a swapped
  length/units arg) and `matrices_overhead_simple_carson` (pins the engine
  `simple_carson_full_3cond` reference — proves `earth_model` is forwarded, not
  hardcoded); extended `matrices_reduce_neutral_to_phases` with the reduced-**Yc**
  assertion (engine `deri_reduce_4cond_to_3` capacitance, previously Z-only); and
  replaced the soft `z_matrix_recomputes_on_frequency_change` `>1.5×` inequality
  with an oracle-pinned 5 kHz recompute (engine `overhead_high_freq_radius_branch`)
  plus the return-to-60 reproduction. Factored the 4× duplicated overhead build
  into a `build_overhead_3()` helper. dss-core lib **370 → 372**. Gate green.
- **Still open (tracked):** the step-2c-i plural-cable `cncables=`/`tscables=`
  active-conductor divergence (above) — independent of the matrix wiring.

**WP7.1 step 3a — Line `geometry=` Carson path (`FetchGeometryCode`/
`FMakeZFromGeometry`) — ✅ done, gate-green, committed.**
- `line/mod.rs`: un-`NOT_PORTED` the **`geometry`** scalar ref
  (`object_ref_class("LineGeometry", "geometry")`); the other geometry forms
  (`spacing`/`wires`/`cncables`/`tscables`) stay `NOT_PORTED` for step 3b. New
  fields `geometry_obj: Option<LineGeometryObj>` (snapshot-cloned at resolve time,
  the WP4.2 `FetchLineCode` pattern), `geometry_name`, `fz_frequency` (Pascal
  `FZFrequency`, `-1` sentinel).
- Ported verbatim: **`FetchGeometryCode`** (clone the geometry, push a pre-set
  `rho` in, copy NormAmps/EmergAmps/NumAmpRatings/AmpRatings/LineType, set
  `NPhases := geom.Nconds` *reduce-aware* + `set_nconds`, clear the superseded
  sym/linecode seq marks, `SymComponentsModel := False`); **`FMakeZFromGeometry(f)`**
  (the `f = FZFrequency` skip-guard, then `Z := geom.Zmatrix[f, len, units]` /
  `Yc := geom.YCmatrix[…]` under the Line's `FEarthModel` — Z/Yc are **total**,
  length+units already folded in); **`KillGeometrySpecified`**. `set_object_ref`
  fetches on resolve (the `linecode` pattern); the sym/matrix/switch side effects
  and a post-`geometry` `rho=` now drive `KillGeometrySpecified` / push `rho` into
  the geometry; the `phases=` guard rejects a phase change under a geometry (as
  under a matrix model — it reverts `nphases` and logs 18101; see the audit
  follow-up below). `MakeLike` carries the three new fields.
- **`CalcYPrim` split into two paths** (Pascal `CalcYPrim`): the geometry branch
  inverts the total `Z` directly (no length/freq/Rg/Xg scaling) and adds the
  **full** `Yc/2` shunt; the sym/linecode branch is byte-for-byte the old code
  (per-unit-length Z scaled by length·freq + earth return). Shared Kron embed +
  CAP_EPSILON + open-conductor tail.
- **`LineGeometryObj`** gained the public accessors `FetchGeometryCode` consumes
  (`nconds` reduce-aware = Pascal `Get_Nconds`; `norm_amps`/`emerg_amps`/
  `num_amp_ratings`/`amp_ratings`/`line_type`) + a manual `Debug` (it owns
  `Box<dyn DssObject>` conductor slots and `Line` derives `Debug`).
- **Deferred F2 now lands (audit follow-up):** a geometry `Zmatrix` error
  (`ELineGeometryProblem`/NIL conductor) is recorded via `push_error`; the Y-build
  loop (`ymatrix::build_y_matrix`) drains the queued message into `env.errors` and
  sets `solution_abort` — the faithful equivalent of Pascal `SolutionAbort` + Exit
  (the trait still has no direct abort channel, so the drain is the sink). To keep
  the abort robust across re-solves, `update_line_geometry_data` now clears
  `data_changed` only on a *successful* calc (Pascal clears it before the check but
  relies on the exception halting the solve outright). Test:
  `line_geometry_conductors_in_same_space_aborts_solve`.
- **Audit follow-ups (matrix getter / rho / FYprimFreq):** `GetZmatScale`/
  `GetYCScale` (the `rmatrix`/`xmatrix`/`cmatrix` getter) now divide the stored
  *total* matrix by `Len` when a geometry is attached (Pascal Line.pas:261-283) —
  previously echoed the total (off by `Len`); test
  `line_geometry_rmatrix_is_per_unit_length`. A `rho=` without a geometry no longer
  invalidates YPrim (Pascal invalidates only when a geometry is present,
  Line.pas:772-780). The geometry branch no longer writes `FYprimFreq` (Pascal sets
  it only in the per-unit-length path).
- **Audit follow-ups (18101 / singular-matrix abort) — the two deferred items,
  settled live against the oracle and fixed.** (1) **18101:** an illegal `phases=`
  change on a matrix/geometry model now logs `Illegal change of number of phases for
  "Line.<name>"` (Pascal Line.pas:643, `DoSimpleMsg` — so it reverts `nphases` but
  does *not* set `SolutionAbort`; `Redirect_Abort` is not modeled). Probe-confirmed
  exact text/number. Test `line_illegal_phase_change_reverts_and_logs`. (2)
  **Singular series Z (error 183):** `CalcYPrim` no longer silently embeds
  `epsilon·I` and continues — it pushes a `Matrix Inversion Error for Line "…"`
  message and exits, and the Y-build drain sets `solution_abort`. The probe showed
  Pascal `DoErrorMsg` sets `SolutionAbort := True` *unconditionally*
  (DSSGlobals.pas:265), so the solve aborts in **both** `EARLY_ABORT` modes (oracle
  default = `True`, DSSGlobals.pas:781) and `BuildYMatrix` Exits before adding any
  primitive — the embed was dead weight (NOT_PORTED). `calc_yprim` now does Pascal
  `ClearYPrim` (Line.pas:1170) up front, so both abort paths leave the element
  contributing nothing. Test `line_singular_matrix_aborts_solve`.
- Oracle-pinned: 3 inline `geometry_tests` drive a `Line` through the property
  engine + a `build_overhead_3` geometry and assert `Z`/`Yc` == the geometry's
  total matrices entry-by-entry, anchored to the `deri_full_3cond` diagonal, plus
  the Zinv Kron embed and `Yc/2` shunt (`geometry_path_builds_oracle_z_and_yc`);
  the length/units forward (`…_length_units_scale_the_total_z`: 2 km = 1 m ×
  1000 × 2); and `sym_scalar_detaches_geometry` (an `r1=` after `geometry=` runs
  `KillGeometrySpecified`). + 1 exec test `line_geometry_specified_resolves_and_solves`
  (full parse → `LineGeometry` foreign-class resolve → solve; `r1` hidden = `----`).
  dss-core lib **372 → 376** (+ 4 audit follow-up tests → **380**). Full
  three-command gate green.
**WP7.1 step 3b — Line `spacing=`/`wires=`/`cncables=`/`tscables=` Carson path
(`FetchLineSpacing`/`SetWires`/`LoadSpacingAndWires`/`FMakeZFromSpacing`) — ✅
done, gate-green, committed (`c2a81d0` + audit follow-ups `5eda50a`/`4aeda24`).**
- `line/mod.rs`: un-`NOT_PORTED` the four props — `spacing` (scalar
  `object_ref_class("LineSpacing")`), `wires`/`cncables`/`tscables` (array
  `object_ref_array` over WireData/CNData/TSData). New `Line` fields
  `line_spacing_obj: Option<LineSpacingObj>`, `line_wire_data:
  Vec<Option<Box<dyn DssObject>>>` (Pascal `LineWireData`), `fphase_choice:
  ConductorChoice` (`FPhaseChoice`), `got_ratings_after_spacing_conds`. The
  trait-object `Vec` forces a **manual `Clone`/`Debug`** (the `LineGeometryObj`
  precedent — the derive is gone).
- Ported verbatim: **`FetchLineSpacing`** (Line.pas:1853 — drop linecode/geometry,
  `NPhases := spacing.NPhases`, allocate the empty `NWires`-slot wire array);
  **`SetWires`** (Line.pas:803 — overhead `istart=1` when `FPhaseChoice=Unknown`,
  else bare neutrals at `istart=NPhases+1`; count-validate `(NWires-istart+1)`;
  seed `NormAmps`/`EmergAmps`/ratings from the wires); **`KillSpacingSpecified`**
  (Line.pas:2042) wired into `FetchLineCode`/`FetchGeometryCode` and the
  sym/matrix/switch side effects; **`SpacingSpecified`**; the
  `spacing`/`wires`/`cncables`/`tscables` `PropertySideEffects` (the three Pascal
  case blocks — cable forms pick the model, the common block switches off the sym
  model + clears the superseded marks, ratings-after-conds latch). **`LoadSpacingAndWires`**
  (LineGeometry.pas) added on `LineGeometryObj`: builds a throwaway geometry from
  the spacing + conductors, picks OH/CN/TS from the wire kinds, runs the Carson
  calc. **`FMakeZFromSpacing`** (Line.pas:1964): the `pGeo` temp-geometry path →
  **total** `Z`/`Yc` (length+units folded in), so `CalcYPrim` reuses the geometry
  branch (`total_z_path = geometry || spacing_specified`); the `rmatrix`/`cmatrix`
  per-unit-length getters and the `Yc/2` shunt likewise.
- **Routing decision (probed):** the `wires=` prop runs the `SetWires` state
  machine (with the buried-neutral `istart`); `cncables=`/`tscables=` use Pascal's
  *generic* `DSSObjectReferenceArrayProperty` fill (`set_cables`, fill from
  conductor 1, no `istart`), with the side effect setting `FPhaseChoice`. Routing
  the cables through `SetWires` would break the buried-neutral case
  (`cncables=[3] wires=[1 bare neutral]`, NWires=4/NPhases=3) — confirmed against
  the oracle.
- **Gate:** `probe_line_spacing_phase7.py` builds each form **both** ways in the
  oracle (`geometry=` vs `spacing=`+`wires=`/`cncables=`) and confirms they agree
  **exactly** (maxdiff 0) for overhead, CN, and CN+bare-neutral. 5 inline tests
  pin the spacing `Z`/`Yc` to the **same `deri_full_3cond`/CN oracle anchors** the
  step-3a + line_geometry tests use (overhead `Z00`+`Yc00`, CN `Z00`, buried-
  neutral `Z00`) and assert entry-by-entry equality with the equivalent geometry:
  `spacing_wires_match_geometry_and_oracle`, `spacing_cncables_match_geometry`,
  `spacing_buried_neutral_via_cncables_then_wires` (validates the `istart` offset
  + 4→3 Kron reduce), `set_wires_wrong_count_errors` (18102 count error),
  `sym_scalar_detaches_spacing` (`KillSpacingSpecified`). dss-core lib **380 →
  385**. Full three-command gate green.
- **Also fixed (faithfulness):** `FetchLineCode` now calls
  `KillSpacing`/`KillGeometry` (Line.pas:590-591 tail — a `linecode=` supersedes a
  prior spacing/geometry; was a latent step-3a gap).
- **Audit-code follow-up (`/audit-code` step 3b):** one Major finding fixed —
  `cncables=`/`tscables=` *before* any `spacing=` left `LineWireData` unallocated
  and **silently no-op'd into the sym model**; the oracle raises error 402 (`No
  objects are expected!`) at that generic-array fill (probe-confirmed), so
  `set_cables` now reproduces 402 instead of swallowing it (`set_wires` already had
  the parallel 18102 guard). Test `cncables_without_spacing_errors`. dss-core lib
  **385 → 386**. The audit also *settled four divergence risks against the live
  oracle and found the port already faithful* (no change): cncables/tscables use a
  partial fill from conductor 1 with **no** count-#406 check (a NIL slot aborts at
  solve with the exact "WireData is not correctly initialized" text — matched);
  the spacing path uses the **line's** `FEarthModel`, not the ambient
  `ActiveEarthModel` (CN mixed-earth-model probe: global Carson + line Deri ⇒ Deri
  Z, identical to the geometry path); `AllowAllConductors` is JSON-only; and
  `GetZmatScale`/`GetYCScale` include `SpacingSpecified` (Line.pas:261-283).
- **Audit-tests follow-up (`/audit-tests` step 3b):** two coverage gaps closed.
  (1) The **tape-shield form had zero executable coverage** (no test wrote
  `tscables=`, so the `TapeShield`/`TsDataObj` arms were dead — the same gap the
  step-2c-i test audit caught for LineGeometry): added inline
  `spacing_tscables_match_geometry`, oracle-pinned (`Z00 = 4.675825330004e-04`,
  probed). (2) The inline `spacing_*` tests call `set_object_ref_array` **directly**,
  bypassing the executive's `wires=[…]` array parse and never querying the
  `spacing`/`wires` dumps (the new `get_string`/`get_object_ref_names` accessors):
  added the full-pipeline exec test `line_spacing_specified_resolves_and_solves`
  (`exec/tests/line_fetch.rs` — `New Line … spacing=s wires=[w w w]` → solve, with
  `?spacing`="s" / `?wires`="[w, w, w]" round-trip), the step-3a
  `line_geometry_specified_resolves_and_solves` parallel. dss-core lib **386 →
  388**.
- **Audit Question settled (cncables/tscables count > NWires):** probed — the
  oracle fills the `NWires` slots and **silently drops the extras** (`cncables=[4]`
  on a 3-wire spacing solves identically to `cncables=[3]`); `set_cables` already
  does exactly this (`if k < NWires`), so the port was already faithful. Pinned by
  `cncables_excess_count_drops_extras`. dss-core lib **388 → 389**. No open
  step-3b tails remain.
**WP7.1 step 4 — geometry/spacing corpus feeder migration — ✅ done, gate-green.**
Rather than blanket-staging the 64 `WireData`/`LineGeometry`/`LineSpacing`/
`CNData`/`TSData`-tagged feeders into `needs_investigation`, each blocker was
diagnosed; two were **real port gaps** and fixed:
- **`Set EarthModel=` was not ported** (line-constants relevant). Added
  `DSS.DefaultEarthModel` (`exec/mod.rs`/`construct.rs`, init DERI=3), the
  `Set EarthModel=Carson|FullCarson|Deri` option (`set_cmd.rs`/`tables.rs` ord 81,
  Pascal `ExecOptions.pas:630`), and the copy into each new `TLineObj.FEarthModel`
  at creation (`command.rs add_object`, Pascal `Line.pas:998`), per-line
  `earthmodel=` still overriding. Unblocked all `4Bus-*`. Test
  `set_earthmodel_seeds_new_line_default`.
- **RegControl `TapNum` read a stale snapshot.** A direct `Transformer.X.Taps=`
  edit (the IEEE13 geometry scripts' manual-tap + `controlmode=off` epilogue)
  moves the winding tap without going through the control, so the parse-time
  `tap_snap` went stale and `regcontrol_tap_numbers()` reported 0 instead of the
  oracle's 10. Pascal `Get_TapNum` reads the **live** `PresentTap[TapWinding]`;
  the exec view now does too (`RegControl::tap_num_live`/`controlled_ref`,
  `view.rs`). Unblocked the IEEE13 geometry/spacing variants. Test
  `regcontrol_tap_number_reads_live_transformer_after_manual_tap`.
- **`Show` no-op stub** (`command.rs`/`tables.rs` ord 8): `Show` is Phase 8
  (`ShowResults.pas`, reporting) and never alters the electrical solution, so it
  is stubbed like `Plot`/`Panel` (user-approved). Feeders no longer hard-error on
  it.
- **Oracle hardening** (`tools/oracle/oracle_server.py`): `Show`/`Export` fire the
  OS editor (notepad) and write report files into the corpus. Added
  `AllowEditor=False` (suppress the editor) + a `_CorpusGuard` (snapshot the case
  dir, delete created files + restore overwritten ones after each run). The live
  gate now stays byte-clean even when a case contains `Show`.
- **Result:** **+15** geometry/cable feeders **oracle-verified** (full live
  model) and promoted to `solvable_now` (**17→32**): the 5 `IEEE13_*`
  geometry/spacing/cable variants, `TextTsCable750MCM`, the 5 `4Bus-*` +
  `4Bus-YYD`, `NEVMASTER`, `epri_dpv/M1`, and `ADiakoptics/ckt24/zone_2`. Gate
  time ~20s. dss-core lib **389 → 391**.
- **Deferred (honestly tagged), not regressions:** **11** feeders solve on Rust
  (Show no-op) and matched the oracle in a one-off probe but are kept **out of the
  always-on gate** because the oracle runs their active `Show`/`Export` (Phase 8)
  — `Show LineConstants` writes a transient `LineConstantsCode.dss` that races the
  `corpus_manifest` bijection (`skipped_unsupported`, tag `unsupported_command=Show`,
  promote when Show is ported). **3** large EPRI/ADiakoptics feeders differed
  ~1e-5 on one connector line's power in a big mesh (all canonical geometry/cable
  feeders matched exactly) → were `needs_investigation`, **since root-caused and
  resolved** (see step-4 follow-up below). **2** Stevenson cases: the **oracle
  itself** doesn't converge → `needs_investigation`. **3** ShortCircuit cases use
  `solve mode=faultstudy` (not ported) → `unsupported_mode=faultstudy`. The
  remaining `WireData`-tagged feeders were re-tagged with their **real** current
  blockers (`PVSystem`/`InvControl`, `var`, `MakeBusList`/`GISCoords`, `Fault`/
  `Relay`/`Recloser`/`vccs`, file-backed arrays) — the stale geometry-class tags
  are gone.

### 1e-follow-up — 3 EPRI/ADiakoptics power divergences resolved (`c7c6649`)

The 3 large meshed cases deferred above (`EPRITestCircuits/ckt5`,
`ADiakoptics/EPRI_Ckt5-G/.../zone_2`, `ADiakoptics/TnDSystem/.../zone_2`)
diverged from the oracle on **one connector line's power** at ~3.6e-6 rel.
Diagnosed (full per-element V/I/P/YPrim probe of both engines) — **not a
line-constants bug**:
- The offenders are **near-zero-impedance connectors**: 1.5 m `BUSBAR` segments
  and `switch=y` lines, |Yprim| ≈ 4.6e6. Their through-current
  `I = Yprim·(V1−V2)` is a **catastrophic cancellation** of two large terms.
- Those huge admittances make the system Y **ill-conditioned** (cond ≈ 1e7), so
  any backward-stable solver leaves ~4e-8 rel roundoff on node voltages (faer
  here vs the oracle's KLU). Proven a **floor, not premature convergence**:
  tightening the solve to 1e-9 / 12 iterations leaves it unchanged. That 4e-8
  amplifies through the cancellation to ~3.6e-6 rel in the current, hence
  identically in `P = V·conj(I)`.
- **Port faithful:** the line YPrim is **bit-identical** to the oracle
  (max|d|=0), V matches to 4e-8 (25× tighter than the gate's 1e-6), currents
  match, and Pascal `TLineObj` has no special power/current path (switch
  constants `r1=x1=r0=x0=1, c1=1.1e-9, c0=1e-9, len=1e-3` match Line.pas:689-694
  byte-for-byte).
- **The gate flagged only the power** because the harness floors were
  inconsistent: a flat 1e-4 A current floor absorbs the ~7e-5 A error, a flat
  1e-4 kW power floor doesn't (same error is `|V|·δI` ≈ 5e-4 kW at 7.2 kV).
- **Fix:** `assert_power_close` (`harness/mod.rs`) — the power abs floor is the
  **image of the current floor through the terminal voltage**,
  `i_abs·max(1, |V_kv|)`, `|V_kv| = |P|/|I|` (self-consistent under
  positive-sequence ×3). Forgives only power error that is the exact image of an
  already-accepted current error; YPrim/V/current (all at unchanged 1e-6 / 1e-4
  A) still pin a real regression independently. Documented in
  `tests/TOLERANCE_NOTES.md`; contract pinned by `harness_power_floor.rs` (3 unit
  tests). Both audits (code + tests) returned faithful/clean. `solvable_now`
  **32→35**, `needs_investigation` 12→9; full gate green (corpus_live 35 cases,
  lib 392, all golden gates unaffected).

### 1e WP7.1 step 5 — targeted golden (`phase7/line_geometry*.json`) — ✅ done, gate-green

The §1 two-tier gate's **tier-1 targeted golden** for WP7.1 — the *focused*
regression guard the live corpus gate does **not** replace: it is committed (pins
the Carson numbers in git, visible in a diff) and runs **offline** (no oracle
install needed to catch a regression), whereas
`corpus_live_solvable_cases_match_oracle` consults the oracle live and *fails*
without it. The deliverable PHASE7_PLAN §1 / §3-WP7.1-step-4 names but the step-4
commit (`d418eba`, live-corpus migration only) had left unbuilt.
- `tools/golden/gen_phase7.py` → `tests/golden/phase7/<scenario>.json` (schema 1,
  command-replay like phase5/6; reuses `gen_checkpoints.capture_yprim` /
  `capture_element` / `check_pin`, so the YPrim layout is identical to the
  checkpoint + live gates — column-major, re/im split). Driven by
  `crates/dss-core/tests/golden_phase7.rs` (runs **every** `*.json` in the dir; a
  `for must in [...]` guard pins that all four paths stay represented, so an edit
  can't silently drop a path's coverage).
- Each scenario builds a small circuit (`circuit` source → geometry `Line` →
  3-phase `Load`), solves once, and pins: converged + iteration count + node
  order exact, node voltages 1e-6 rel, the **Line YPrim entry-by-entry** (the
  Carson Z/Yc — the new math under test, §1 focused gate 1), and every element's
  terminal currents/powers (the voltage-scaled `assert_power_close` floor).
- 5 scenarios from the oracle-verified probe decks
  (`probe_line_constants_phase7.py` / `probe_line_spacing_phase7.py`):
  `line_geometry` (3-phase overhead `geometry=`, no reduce), `line_geometry_reduce`
  (3 phases + a neutral, `reduce=yes` — the Kron reduce path), `line_spacing`
  (`spacing=` + `wires=`), `cable_cn` (CN cable `geometry=`+`cncable=`), `cable_ts`
  (TS cable `geometry=`+`tscable=`). `line_geometry` and `line_spacing` capture a
  **bit-identical** Line YPrim — the geometry and spacing paths agree (as
  `probe_line_spacing_phase7.py` showed maxdiff 0), now pinned offline.
- Gate: `golden_phase7` 1; full three-command gate green on **stable**
  (`cargo +stable …`, matching CI). dss-core lib stays 392 (integration test, not
  a lib unit test). **WP7.1 complete; next = WP7.2 (Protection).**
- **Audit-tests follow-up (`/audit-tests` step 5):** the audit found the
  **tape-shield form had no offline golden scenario** (only CN) — the same TS
  asymmetry the step-2c-i (`2b54849`) and step-3b audits caught for the inline
  tests, now in the targeted golden. (Not a true hole: inline tests pin the TS
  Carson matrices and the live gate covers `TextTsCable750MCM` end-to-end; but the
  offline golden lacked CN/TS parity.) Added `cable_ts` (geometry=`+`tscable=`,
  mirroring `cable_cn`), regenerated → 5 scenarios; the `for must in […]` guard now
  pins all five. A throwaway-probe proof (a perturbed YPrim entry → the gate fails
  with a precise `Yprim[0,0] differs … |diff|=5.0e-1 > allowed 1.0e-3` delta)
  confirmed the comparison is wired, not a no-op. `audit-code` was N/A (this step
  changed no implementation — only test infra, the `collapsible_match` allow, and
  docs). Gate green.
