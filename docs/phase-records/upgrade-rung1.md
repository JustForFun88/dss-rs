# UPGRADE Rung 1 — detailed records (pre-work, WP-U0.2, WP-U1.1, WP-U1.2)

> **Archived verbatim from `STATUS.md` on 2026-07-12** to keep the living handoff lean. These are the
frozen post-acceptance UPGRADE_PLAN Rung-1 records (multi-oracle test infra,
the WP-U0.2 inventory sweeps, WP-U1.1 parser/property items 1–5, and the
WP-U1.2 numeric long-tail rows). `STATUS.md` keeps the condensed summary + the
WP-U1.2 resume note (rows D3/B3/B5) with a pointer here.

### UPGRADE pre-work (branch `upgrade-test-infra`, 2026-07-07)

**`UPGRADE_PLAN.md` authored + its WP-U0 multi-oracle test infra LANDED** (post-acceptance
plan, PLAN_SEQUENCE stage 3: Rung 1 = dss_capi 0.15.x/r4088-line parity, Rung 2 = OpenDSS
11.0.0.1/r4133; scoping inventories in `docs/upgrade/delta_*.md`). Infra: per-case
`oracle` manifest field (`capi015|r3723|r4088|r4133`) routes a live-compare case to its
target engine (`OraclePool`, ping-verified per spec); new `capi015` oracle_server engine
(dss-python 0.16.0b2/Oddie venv, backend 0.15.0b4 pinned); iteration policy `Rust <=
oracle` for target-rev cases only (default cases stay exact); `corpus_live_opendss`
excludes flipped cases; pilot `modes/upgrade_pilot.dss` live-compares against the official
EPRI **r4133** binary in every `cargo test` (Oddie venv + `bin/` are now mandatory gate
prerequisites). Built on a separate branch so parallel porting continues beside it.

Both WP-U0 audits ran (opus-high, no correctness bug): audit-code Minor = default oracle
had no positive engine-identity assertion (a double env misconfig could re-bind it) —
fixed (`Oracle::new` pins `DSS_ORACLE_ENGINE=capi`; `ping_engine(None)` asserts the
`oddie`/`capi015` flags ABSENT); audit-tests Minor ×2 fixed (`solvable_now_oracle_specs_
are_valid` structural guard; sweep report records `target_rev_excluded` + empty-universe
warning). Accepted-by-design, recorded: the U0 pilot is revision-insensitive — it proves
engine identity (ping) + plumbing, not numeric routing; the first revision-sensitive flip
(WP-U1.2) closes that (noted in the plan); the iterations-`<` note is visible only under
`--nocapture` (ritual note added to UPGRADE_PLAN §1.3-1).

### WP-U0.2 (upgrade inventory sweeps) — branch `wp-u02`, 2026-07-11

The empirical completeness check on the three `delta_*.md` scoping inventories
(UPGRADE_PLAN §WP-U0 follow-up), **before** any porting WP. Report-only: no
engine code, no `oracle` flip, no golden regen. Summaries committed under
`docs/upgrade/sweeps/` (README + 3 per-pair + dsspy); raw JSON stays in
scratchpad (regeneration commands in the README).

**Populations — 378 cases/pair** (solvable_now 245 + asymmetric 36 + controls
57 + modes 40). Headline `ab_compare` counts:

| pair | isolates | match | diverged | error |
|---|---|---|---|---|
| capi(0.14.5) ↔ capi015(0.15.0b4) | dss_capi 0.14.5→0.15.x (Rung 1) | 334 | 17 | 27 |
| oddie:r4088 ↔ oddie:r4133 | EPRI 10.2→11.0 (Rung 2) | 344 | 27 | 7 |
| capi015 ↔ oddie:r4088 | FPC vs Delphi @ r4088 (ledger) | 323 | 24 | 31 |

Plus `dsspy_validation` broad-surface (~40 collections/case): capi015 4265 /
r4088 4593 / r4133 4593 entries; `compare_outputs` on the two EPRI-facing pairs.

**Reconciliation (rows: 2 added, ~35 witness/synthesize annotations across the 3
inventories, full rung attribution).**
- **Rung 1 items with a corpus witness:** B1 (Capacitor ×1.000001), B2/D1
  (Carson 658.85), B5 (GFM Isc1×1000), C2+B7 (**PermissiveProperties strict
  default — 16 decks flip solve→parse-error**, the dominant finding; ledger L2),
  C5/B4 (RegControl), D1–D4 (InvControl), D10/E2 (StorageController/seasonal),
  D14 (DynamicExp, V 0.87). No-witness → synthesize: NCIM, WindGen, force hooks,
  new line-constant paths, TCC.none, Solve/Clear all, meter disabled-skip.
- **Rung 2 items with a witness:** all four protection classes (Relay/Recloser/
  Fuse/SwtControl). Cross-rung attribution **proven**: `capi015↔r4088` shows the
  same protection decks agree to V≈1e-15 (text/order only) ⇒ the protection
  **numeric** overhaul is Rung-2-only. No-witness → synthesize: SinglePhTrip/
  Lockout, fast/slow pickup split, `batchedit where`, AllocateLoad disabled-skip.
- **Surprises (added rows):** (1) `IEEE_519` **harmonics r4088→r4133 V 1.3e-2**
  with Y bit-identical, and (2) `InductionMachine` **converged-flip r4088→r4133**
  — both **determinism-probed** (`r4088↔r4088` & `r4133↔r4133` match; each engine
  deterministic ⇒ real cross-build move), contradicting the delta's "solver
  byte-identical" claim; cause (un-diffed hunk vs Delphi build-ulp drift on
  ill-conditioned solves) is for **WP-U2.6** to source-confirm/catalog. (3) A
  0.15.x **binary/MMF-shape access-violation crash** (`#58614`, decks
  `shape_binfiles`/`shape_mmf`/`xycurve_files`) fires in 0.15.0b4 AND EPRI
  r4088/r4133 but not 0.14.5 — no port action (Rust reads them correctly);
  recorded as `delta_capi` B9 and it is why the modes family had to be swept
  one-case-per-process.
- **dsspy noise clarified:** r4088↔r4133 RegControl (354) / Meter (114) diffs are
  a +1 `NumProperties` metadata bump (RegControl.pas byte-identical) + relay-trip
  open-circuit `CalcCurrent` — not behavioral. capi015↔r4088's 130k diffs are the
  known FPC-vs-Delphi Lines/Loads/Transformers format+ulp surface (inventory-only).

**Tooling:** `ab_compare.py` gained a `capi015` engine spec (binds
`DSS_ORACLE_ENGINE=capi015` on the Oddie-venv interpreter). Sweep-hygiene note:
`newton_feeder.dss` (non-converging Newton, hits the 300 s timeout) and the
`#58614` shape crashers poison a shared `ab_compare` engine (`#303 on clear`),
so the modes family is swept one case per process; corpus stays pristine. The
modes-isolation + merge recipe is now committed and turnkey:
`tools/opendss/sweep_modes_isolated.py` + `sweep_merge.py` (merged = 338 corpus
non-modes + 40 isolated modes = 378, verified 0-orphan on all three pairs).

**Audit settle (2026-07-12):** 9 findings settled empirically, all fixed.
(1) capi_vs_capi015 `#2024101` NormAmps read-only witness corrected — the 3 decks
are `StoCtrl_SeasonTarget/{IEEE13NodecktMOD,Run_example}` +
`ADiakoptics/IEEE_123_Bus-G/Torn_Circuit/zone_2/master.dss` (`StoCtrl_Current_PeakShave`
MATCHES; verified in `merged_capi_vs_capi015.json`). (2) IM `r4133↔r4133`
determinism probe **run** (`det_indmach_r4133.json`: both cases match, iters
21/21 & 20/20, `v_max_rel=0`) — the "each engine deterministic" claim now holds
on BOTH sides for BOTH surprises. (3) `pstcalc_cmd` timeout re-bucketed (crash/
timeout, not `#303`) consistently in README. (4) delta_capi reconciliation
completed the inventory→witness direction: 8 in-scope rows (B4-harmonics-abort,
C8, C11, **D6 transformer-seasonal**, D11, D12, D13, E1) flagged **synthesize** —
D6 is report-only NUMERIC and *unwitnessable by `ab_compare`* (no overload/export
channel; `storagecontroller_seasonal` is storage+line, not transformer AmpRatings)
→ needs a dedicated overload-report deck. (5) cross-file bucket-tag collisions
qualified (`B4-r3723`/`A5-r3723` vs this-file's B4/A5) + legend note. (6–9)
per-signal count-table relabels, delta_r4088_r4133 headline caveat → B5/B6,
capi015=r4103 version-gap caveat. No engine/golden/oracle changes; docs only.

### WP-U1.1 (parser & property-system semantics) — branch `wp-u11`, 2026-07-12

Rung 1, exec opus-high. **Item 1 of 5 landed** (ledger L2, the `DblValueNZ`
zero-`kW`/`kVA` clamp); items 2–5 (`ParseAsSymMatrix` incomplete-matrix error,
`AllowNoneItem`/`WasQuoted`, `TCC_Curve.none`, C11 class-command activation)
remain — see "next" below. This session RESUMED a crashed executor's dirty draft
(item 1 only); the draft was re-verified against the spec + re-probed, one real
bug fixed, and the ledger's central gate-consequence claim corrected.

**L2 clamp (adopt EPRI r4133 `DblValueNZ`).** A parsed essential-sizing double in
the open band `(-1e-8, 1e-8)` → `+1e-8`, unconditionally (EPRI default), pre-
scale, in `obj/props/setters.rs::set_obj_double` via new `PropFlags::REPLACE_ZERO`.
Carried by Load `kW`/`kVA`, Generator `kW`/`kVA`, Storage `kW`/`kVA`, PVSystem
`kVA`. Probes I re-ran (`scratch_probe_l2.py`, 2026-07-12): capi015 default keeps
`0`, `+0x200` (PermissiveProperties) clamps; r4088/r4133 clamp by default. We
adopt the r4133 default; we do NOT adopt dss_capi's strict-`NonZero`-error surface
(dss-ext-only). `ParserDel.pas:912 MakeDoubleNZ` confirmed as the exact band-clamp.

**Salvage record (crashed draft):**
- KEPT: the `REPLACE_ZERO` flag + `set_obj_double` clamp, Load/Storage/PVSystem
  `class_props` flags, the exact unit test `zero_kw_kva_clamp_dblvaluenz`, the
  synthesized `modes/upgrade_parser_zerokw.dss` deck (r4133). Both the per-class
  test coverage and the deck's feature-sensitivity were extended in settle (see
  "Settle" below; deck fp is now `02037161f3236a3b` after adding `Load.zpf`).
- FIXED (real bug): the draft flagged Generator **`MVA`** with `REPLACE_ZERO`.
  r4133 `generator.pas:664` reads `MVA` (prop 27) through plain `DblValue*1000`
  — NOT `DblValueNZ` (only prop 26 `kVA` clamps). Removed the flag; documented the
  upstream asymmetry (only WindGen's `MVA` clamps, U1.8). Fixed in
  `generator/mod.rs`, `prop_flags.rs` doc, and the ledger decision text.
- CORRECTED (falsified claim): the draft ledger claimed the clamp "moves no
  default-oracle observable." **False.** A Load with both `kW=0` **and** `kvar=0`
  ends in the `KwKvar` spec; the clamp makes `kVA=1e-8>0`, so `RecalcElementData`
  recomputes `PF = kW/kVA = 1` (un-clamped `kVA=0` skips the recompute, keeping the
  parsed `pf`). So `? load.pf` reads `1` (Rust≡r4088/r4133) vs `0.9` (0.14.5).
  Probe `scratch_probe_pf.py`. Corpus scan (`/tmp/scan_zero.py`): the breaking
  pattern hits exactly one mandatory-gate deck, `epri_dpv/M1/Master_NoPV.dss`
  (feeder, 8 zero-loads). Per §1.2 it is **flipped to `oracle: "r4133"` in this
  same commit**; the whole-model live compare passes green against r4133 (target-
  rev cases drop `compare_all_properties` per §1.3-2, so the PF readback is no
  longer compared vs the 0.14.5 oracle it deliberately mismatches). No golden
  migration (no byte-golden covers M1); no `known_diffs.json` change.

**known_diffs burn-down:** none (no zero-kW entry existed @ r3723; Rust now matches
r4133). **Iteration policy (§1.3-1):** M1 is the only newly-flipped case; the
`--nocapture` gate run emits **no** `NOTE Rust converged …` line for it — Rust
matched r4133's iteration count exactly (no suspicious strict `<`).

**Settle (audit findings, 2026-07-12).** Two real gaps in item 1's regression
coverage were closed:
- **Clamp untested on Generator/Storage/PVSystem + MVA asymmetry unpinned.** The
  only clamp test was Load-only, so removing `REPLACE_ZERO` from the other three
  classes — or re-adding it to Generator `MVA` (the exact crashed-draft bug) —
  passed silently. Added `zero_kw_kva_clamp_dblvaluenz` to Generator (also asserts
  `MVA=0 → kVA rating 0`, NOT clamped) and Storage (`kW=0` clamp witnessed via the
  `Set_kW` state resolving DISCHARGING not IDLING), and `zero_kva_clamp_dblvaluenz`
  to PVSystem. All four verified feature-sensitive (each fails when the clamp is
  disabled).
- **Deck not feature-sensitive (§1.7-3).** `upgrade_parser_zerokw.dss`'s numeric
  channels all sit below the 1e-8 clamp floor, so it passed identically with and
  without the clamp. Added `Load.zpf` (`kW=0` AND `kvar=0`) + a `?pf` probe: under
  the clamp its `KwKvar` spec recomputes `PF 0.9→1`, un-clamped it stays `0.9`
  (re-probed r4133/r4088 → `pf=1`; Rust `KwKvar` path → `pf=1`). The `?pf` probe
  now diverges `0.9` vs `1` (|Δ|=0.1 » floor) if the clamp regresses. Deck
  two-process determinism re-validated on r4133 (fp `02037161f3236a3b`).
- Corrected the deck header comment (was: "capi015 keeps a literal 0"; the real
  reason it can't run on capi015 is the `Generator.kVA=0` `NonZero` rejection) and
  refreshed the manifest note to match.

**Item 2 landed (`ParseAsSymMatrix` incomplete-matrix reject — adopt EPRI r4133).**
`Parser::parse_as_sym_matrix` now returns `OrderFound` (rows that supplied ≥1
value) instead of always `ExpectedOrder`; `ClassProps::parse_into` rejects a
matrix with `OrderFound < order` (both the `SymMatrix*` and `DoubleSymMatrix`
arms): it logs the r4133 message ("The matrix entered does not match with the
expected order…") and keeps the property's prior value — the DoSimpleMsg-and-
continue semantics of `ParserDel.pas:741`. The FPC line (0.14.5/0.15.x) silently
zero-filled the missing rows; capi015 does too. This is a **deliberate ledger
exception** (DIVERGENCES.md §ParseAsSymMatrix): we favor the r4133 end-target over
the capi015 Rung-1 oracle.
- **No live oracle is possible.** capi015 zero-fills (comparing against it = a
  forbidden §1.2 mismatch); oddie r4133 **hangs** the moment an incomplete rmatrix
  leaves a linecode's series Zmatrix inconsistent (probed: `rmatrix=(0.4|0.1 0.4)`
  never returns under Oddie; any solve after such a reject times out). So r4133 can
  neither compile nor solve the feature — §1.7's "solves on target oracle" is
  unattainable. Gated instead by **feature-sensitive Rust unit tests**: `dss-parser`
  `sym_matrix_returns_order_found_for_incomplete_input`; `dss-core` line_code
  `incomplete_sym_matrix_rejected_keeps_default` (asserts the reject message AND the
  default-symmetric revert, not the zero-fill) + `complete_sym_matrix_still_accepted`.
- **Gate-safe.** A full corpus scan (`scan_incomplete_matrix.py`) finds the one
  incomplete-matrix witness (`4wire-Delta/Kersting4wireIndMotor.dss`, 556MCM
  linecode 3-row cmatrix) already in `skipped_oracle_issue.json` (IndMach012a
  user-model, unrelated), so no mandatory-gate case supplies an incomplete matrix —
  the full `cargo test --workspace` is green with the default change.
- known_diffs: nothing to retire (0.14.5 and the port both zero-filled at r3723).

**Item 3 landed (`AllowNoneItem` — `none` in conductor lists — adopt capi015).**
New `PropFlags::ALLOW_NONE_ITEM` on Line + LineGeometry `Wires`/`CNCables`/
`TSCables`; the `ObjectRefArray` parse resolves a `none` token to a NIL slot (no
"not found" error) — capi015 accepts it, 0.14.5 errors #40303. Threaded
`ObjectRefArrayItem = Option<(name, ElemRef, view)>` through `set_object_ref_array`
→ `set_wires`/`set_cables` (storages already `Vec<Option<…>>`). Exposed
`Parser::is_quoted()` (item 3 "WasQuoted plumbing"; the parser already tracked it,
WP-U2 consumes it). Parser/storage plumbing ONLY — the mixed-conductor-list
*numerics* (the `Conductors` property) are WP-U1.4; a `none` conductor alone is
degenerate (capi015 `#303`s the geometry), so no live deck (§1.7 unattainable).
Gate-safe (no corpus deck uses `none` in a list; the `Option` thread kept the
non-`none` path byte-identical). Pinned by `line_fetch::conductor_list_accepts_none_entry`
(feature-sensitive — a non-`none` missing name still errors) + parser `is_quoted`.

**Item 4 landed (`TCC_Curve.none`).** (a) `new TCC_Curve.none` is rejected (423,
no object) in `add_object` — capi015 == r4133. (b) The C7 `AllowNone`-on-single-ref
(Recloser/Fuse curve `=none`) is **observably a no-op in capi015** — its AllowNone
branch NILs the ref then the unconditional `if otherObj=NIL` fires #401 anyway
(DSSObjectHelper.pas:862), bit-identical to the not-found path the port already
takes — so the port sets NO flag (a silent clear would diverge). r4133 diverges
(stores literal `none`, no #401) — Rung-2 note. Pinned by
`lifecycle::tcc_curve_none_is_reserved` + `fuse_curve_none_clears_with_error_like_capi015`.

**Item 5 landed (C11 / SVN r3875 class-command activation).** The port collapses
`LastClassReferenced`+`ActiveDSSClass` into one `active_class`, so every already-
ported SetObjectClass-equivalent already reproduces the fix. The fix's only
newly-affected consumers, `Set Class=`/`Set Object=` SET-options, were NOT_PORTED
— now ported (opts 1/12 `Type`/`Class` → `set_object_class` activate; opts 2/13
`Element`/`Object` → `set_object`, now also setting `ActiveCktElement`). Gate-safe
(0 corpus uses). Pinned by `select::set_class_activates_and_set_object_selects` +
`set_class_unknown_errors_keeps_previous`. Follow-up (separate, NOT r3875): a bare
`? prop` querying the ActiveCktElement is still unported in `do_query_cmd` — owner
for a later WP.

**Each item's decision + probe transcript + gate consequence is in
`docs/upgrade/DIVERGENCES.md` (§ParseAsSymMatrix, §AllowNoneItem, §TCC_Curve none,
§Class-command activation).** No live oracle exists for items 2/3/4b (capi015 and
r4133 each disagree in ways that can't be gated — documented per §1.4); they are
pinned by feature-sensitive Rust unit tests, the honest gate for a behavior neither
oracle can drive.

**Settle pass (items 2-5 audit, 2026-07-12).** Eight findings triaged empirically
(capi015 0.16.0b2 + 0.14.5 probes; `SetObject`/`SetObjectClass` are byte-identical
across the delta, so the Pascal at `.inputs/dss_capi` is authoritative):
- **`Set Object=badclass.l1` fall-back — FIXED.** `set_object` aborted on an
  unknown class qualifier; Pascal (and both oracles, probed) instead log #903, keep
  `LastClassReferenced`, and resolve the name against the previous class (`→Line.l1`).
  `set_object` now mirrors `do_select_cmd`; a NIL class emits #905. New test
  `set_object_unknown_class_qualifier_falls_back`. This also witnesses item-5's
  bare-resolution against the oracle (prior "matches oracle" claim was Pascal-only).
- **`Set Class/Object` before a circuit — REFUTED (no fix).** `DoSetCmd_NoCircuit`
  has an `else` arm (`ExecOptions.pas:303`) emitting #301; both oracles raise #301
  (probed). The port's catch-all already emits #301 — not a silent no-op.
- **DoubleSymMatrix reject arm — TEST ADDED.** New
  `line_fetch::incomplete_double_sym_matrix_rejected_keeps_default` (Reactor RMatrix):
  reject error logged + stored value unset (revert), asserted via `get_f64_array`
  because the DoubleSymMatrix *readback* is an all-zeros `TODO(compat)` bug-repro
  that can't distinguish revert from zero-fill.
- **`fuse_curve_none` test — CLARIFIED.** Now defines a real `TCC_Curve.tlink` and
  snapshots the error count so the `none`-clear (#401 + revert to "", probed on both
  oracles) is isolated from the previously-undefined-curve error.
- **`wires=(w none)` readback — no fix (UB).** `? …wires` on a NIL slot is a capi015
  **Access Violation** (probed); no defined oracle string, so the port's `[w, ]` is
  a deterministic non-reproduction of the crash. Documented in the test + DIVERGENCES.
- **3 of 4 corpus micro-decks absent (Major) — confirmed justified (no fix).** A
  solvable, non-mismatching oracle deck is genuinely unattainable for items 2/3/4
  (capi015 zero-fills / AVs, r4133 hangs on solve-after-reject, TCC `none` is a
  rejection); each is documented as §1.7-unattainable in DIVERGENCES and pinned by
  feature-sensitive unit tests — the honest gate for a behavior no oracle can drive.
- **2 of 9 tests are regression-guards (accounting) — no fix.** `complete_sym_matrix_still_accepted`
  and `fuse_curve_none…` guard behavior not changed by this WP; honestly labelled as
  guards in their comments/DIVERGENCES, not counted as new-code coverage.

**Next (resume point):** WP-U1.1 items 1–5 all landed (settle pass closed). Next WP
is U1.2 (numeric long tail) per the plan; U1.4 owns the conductor-list-with-`none`
numerics that this item's plumbing enables.

### WP-U1.2 (numeric long tail) — branch `wp-u12`, 2026-07-12

Rung 1, exec sonnet-high. Each spec row = one same-commit package (port the
cited hunk → flip the live cases whose observables move to `oracle: "capi015"`
→ regenerate only affected goldens with the capi015 engine → retire matching
`known_diffs` entries). Rows: B1, B2/D1, D6, D8, D7, B5, D3, B3-r3723.

**Infrastructure — golden-generator engine switch (§1.5).** `gen_checkpoints.py::
check_pin` now honours `DSS_ORACLE_ENGINE` (`capi` default = pinned 0.15.7/
backend 0.14.5; `capi015` = Oddie-venv dss-python 0.16.0b2/backend 0.15.0b4,
`PIN_OPENDSS.txt`, rebinds `_get_y_sparse` to the no-`factor` fastdss form) and
stamps the golden's top-level `oracle.engine_spec` provenance — so a mixed golden
tree is self-describing. Both golden generators (`gen_checkpoints`,
`gen_der_lines_harmonics`) share it. Unknown engine → loud exit.

**Row B2/D1 — SimpleCarson De `658.5 → 658.8530451057239` — LANDED.** The first
revision-SENSITIVE flip (closes the WP-U0 note that the pilot proves engine
identity but not numeric routing). `LineConstants::get_ze` (SimpleCarson) adopts
the corrected De; the upstream INCONSISTENCY is reproduced 1:1 — `Line`'s own
`Kxg` keeps `658.5` under `TODO(compat)` at the three `elements/pd/line/*` sites
(ledger `DIVERGENCES.md §B2/D1`). Probe proof (`/tmp/probe_carson.py`): `Xmatrix[0]`
`9.0807e-1` (0.14.5) vs `9.0811e-1` (capi015), rel ~3e-5 » the 1e-6 Y floor.
Same-commit package: **18 Carson-geometry decks flipped to `oracle: "capi015"`**
(2 `Test/Cable*`, 12 `MonitoredVoltage/{Local,Mon}_voltage_*-2`, 4
`4Bus-*`/`YYD-Master-step1`) — corpus_live green (204 s, B2 is the sole mover:
no Cmatrix caps → B1 untouched); new capi015 golden
`line_constants/line_geometry_carson.json`; three Carson Rust unit-test
references moved to capi015 (only the earth-return reactance, and CN/TS-reduced
resistance, shift). `known_diffs`: none matched (0.14.5 and the port both used
658.5) — nothing to retire.

**Row D7 — PVSystem dynamics current-limit base `PanelkW → FkVArating` — LANDED.**
`IntegrateStates` `iMaxPPhase` (`pvsystem/dynamics.rs`). Storage already used
`FkVArating` in both revs (unchanged). Unit-test-only same-commit package (D7
moves NO live corpus deck): `pvsystem_dynamics_mode3` ch21 `Max. Amps`
23.149570 → 27.779484 (=×kVA/PanelkW=600/500, matches capi015 exactly);
`pvsystem_dynexp_dynamics_mode3` `dit@0` 1055150.8 → 1054757.2 (the deck's isp
sat at the old clamp boundary; the new 600-base boundary releases it — a
deterministic ~0.035% single-step shift; the binding settled pins are unchanged
and match both engines). `pv_gfm_dynamics.dss` has `kVA=Pmpp=800` so D7 is a
NO-OP there (stays default oracle, probe-confirmed). Ledger §D7.

**Row B5 — GFM `Isc1` ×1000 removal — DEFERRED (open follow-up).** Adopting the
`Isc1` change moved the Rust GFM operating point (`gfm_micro` `Load.isl` 400→368 kW
vs capi015), but a direct two-engine probe proves the Pascal op-point is
**Isc1-INVARIANT** (0.14.5 == capi015 = 127094.3908 W/φ bit-identical despite the
Yf move). Root cause = a **pre-existing Rust GFM power-flow injection-vs-YPrim
consistency gap** (the injection does not track `YPrim·Vset`, so the op-point is
Isc1-sensitive where the Pascal engines' is not) that B5 merely unmasks — needs a
dedicated GFM investigation, out of the numeric-long-tail scope. B5 + its 4 GFM
live-deck flips reverted; ledger §B5 has the full evidence. **OPEN FOLLOW-UP for a
GFM WP.**

**Row D6 — Transformer seasonal AmpRatings drop `1.1 *` — LANDED.**
`transformer/yterminal.rs` (`1.1 * r → r`, `Transformer.pas:1058`/SVN r4033).
`NormMaxHkVA`'s own 1.1 (110% norm rating) is a different quantity, unchanged.
No live/golden witness (the seasonal override is NOT_PORTED — WP-U1.5 E2 owns it;
non-seasonal reports use `norm_amps`; `Ratings` readback = kVARatings), so pinned
by a feature-sensitive unit test (`seasonal_amp_ratings_drop_the_1_1_factor`).
Ledger §D6.

**Row B1 — Capacitor Cmatrix YPrim diagonal ×1.000001 — LANDED.**
`capacitor/solve.rs` SpecType-3 `_ =>` arm gains the ×1.000001 diagonal loop
before `invert()` (`Capacitor.pas MakeYprimWork`). Only reached for a Cmatrix cap
WITH series R/XL (`has_zl`); revision-sensitive garbage(1e-23 @0.14.5)→finite
(@capi015). No corpus witness (corpus caps are shunt-kvar, cmatrix hits are
LineCodes), so oracle-validated unit test
(`cmatrix_with_series_reactance_yprim_matches_capi015`, capi015 probe values
pinned to 1e-11/1e-12). A live modes deck was prepped but the manifest's mixed
manual unicode-escaping + CRLF blocks a clean append — the unit test carries the
same capi015 numbers. Ledger §B1.

**Row D8 — Transformer X13/X23 TrapZero — SETTLED, no code change (not an
observable delta).** The `TrapZero` flag on X12/X13/X23 (and `NonZero` on
`XSCArray`) is already in the 0.14.5 baseline — commit `69fca934` predates the
0.14.5 tag, so the flags+values (7/35/30) are byte-identical across 0.14.5 and
0.15.x; this is not a 0.14.5→0.15.x delta at all. Probed: 0.14.5 and capi015 give
BIT-IDENTICAL results for X13=0 (`?XHT=3500`; solved Vmin=0.124819 both) — both
reach the same trapped default via the Xsc build. The Rust port ALREADY traps
(`mod.rs trap_zero(7/35/30)` + unconditional `setters.rs`), so it matches both;
the `XSCArray` NonZero STRICT error is the C2/L2 strict surface not adopted.
Pinned by `three_winding_x13_x23_trap_zero_to_default`. Ledger §D8 (mirrors
WP-U1.1's D5/D8-r3723 "not a delta for us").

**Rows D3/B3 — NOT started (budget); see resume note.** D3 is report-only
(spacing ratings — overload-report deck). B3-r3723 (Load.GrowthFactor Year=0 from
dblHour/8760) needs a growthshape + multi-hour year-0 run.

**Resume note (WP-U1.2 remaining):** rows D3 (report-only spacing ratings) and
B3-r3723 (Load.GrowthFactor Year=0) still to port; the golden engine switch
(`gen_checkpoints::check_pin` `DSS_ORACLE_ENGINE`) and the same-commit workflow
are proven (B2/D1, D7, D6, B1, D8). B5's GFM gap is the one hard blocker (a
control-consistency bug, not a numeric constant) — needs a dedicated GFM WP. NB
the modes manifest is NOT json.dumps-round-trippable (mixed manual `\uXXXX`
escaping + CRLF) — append new cases with a surgical text edit, not a full JSON
rewrite.

**Settle (audit dispositions, 2026-07-12).** Five findings settled empirically;
the branch was rebuilt from base `58007c9` (backup `backup-wp-u12-pre-settle`) so
every fix lands in-place rather than as forward churn:
- **Corpus pollution (Major, ×2 — code+tests).** 15 generated Monitor-export CSVs
  (`ExpControl/*_Mon_pv1*.csv`, ~1.296M lines) had been swept into the vendored
  corpus by a blanket add on the docs commit. They are solve artifacts (absent
  from pristine `.inputs`, from base `58007c9`, and from `main`; referenced only
  by the upstream plot scripts, by no test/manifest). REMOVED — and removed by
  rewriting the docs commit, not a forward `git rm`, so the ~1.29M-line blob never
  enters merged history. The live gate's `CorpusGuard` RAII already deletes them
  when `Master.dss` regenerates them each run (>2 MiB ⇒ name-tracked sweep-on-drop),
  so `git status tests/corpus` stays clean without them committed; verified by the
  full gate below leaving the corpus pristine.
- **Same-commit slip on B2/D1 (Major).** The lock refresh (`3200c41`) that fixed
  the gate-red `4d3faaf` (flipped 18 decks in `solvable_now` but not
  `population.lock`) is now FOLDED into the single B2/D1 commit — no gate-red
  commit remains in history; `population_lock_matches_manifests` is green at every
  commit.
- **D8 ledger misattribution (Minor).** DIVERGENCES §D8 + this record + the D8
  commit message claimed 0.15.x "added" the `TrapZero`/`NonZero` flags. Refuted
  against the vendored sources: the flags+values are byte-identical between
  `.inputs/dss_capi` (0.14.5) and `.inputs/dss_capi_with_git` (0.15.x) — commit
  `69fca934` predates the 0.14.5 tag. Text corrected; the no-code-change decision
  is unchanged (and better justified).
- **18-flip strict-necessity note (Minor, informational).** All 18 flips are
  additive, genuinely Carson-geometry, and B2-attributable; a few (e.g.
  `4Bus-DY-Bal` Q ≈ 8.5e-8 rel) sit near the f64 floor. Rust now emits the capi015
  numbers so each flip is correct; no case dropped coverage. No defect — recorded
  as-is.

### WP-U1.3 (InvControl cluster) — branch `wp-u13`, 2026-07-12

**Six spec rows settled** (D1/D2/D3/D4/D5 + C8) — ledger
`docs/upgrade/DIVERGENCES.md` §L1/D2/D3/D4/D5/C8. Spec = `dss_capi_with_git`
`InvControl.pas` diffed against 0.14.5. Oracle = capi015 (probes cross-checked
vs oddie r4133 for the L1 arbitration).

- **D1 / ledger L1 — InvControlDeltaV per-control 2-slot buffer (adopt fix;
  r4133 keeps a 9-year bug).** `EPRI r4133` (and 0.14.5) only advance the FIRST
  InvControl's `FVpuSolution` cursor (gated on element-list `i=1`), so a 2nd+
  control's `voltagechangesolution` latches at 0 → its volt-var hysteresis is
  wrong. Probe `scratch_probe_l1.py`: capi015 pv2 settles at −14.01 kvar, r4133
  oscillates −9.2/−12.8/…; pv1 identical. The r3723 port already computed the
  fixed value (it toggles each object's own cursor unconditionally — it never
  had the `i=1` gating); this WP makes the **buffer form** faithful to 0.15.x
  (`[f64;2]`, init −1, toggle 0↔1) — numerically identical, no deck/golden moves.
  Pinned by the feature-sensitive unit test
  `d1_buffer_2slot_tracks_last_two_pu_voltages` + a snapshot capi015 multi-DER
  deck (`invcontrol/invcontrol_multi_vv_wye.dss`). The hysteresis divergence
  itself is **not** live-gatable (see the capi015-multi-step note below).
- **D2 — per-DER base-voltage cross-leak (adopt).** `UpdateInvControl` now
  renormalizes each DER's MonBus voltage by ITS OWN `FVBase`
  (`ctrl_vars[j].f_vbase`, was `ctrl_vars[0]`). No-op for non-MonBus /
  homogeneous fleets (every corpus InvControl is self-monitored). Unit-pinned
  (`d2_update_uses_per_der_basekv_not_first_der`).
- **D3 — watt-priority `Sqrt(kVA²−kW²)` guard (adopt).** Guard the radicand
  `|.|<EPSILON=1e-12 → 0` before `Sqrt` (was `Sqrt(<0)=NaN`); the same
  `EPSILON` fixes the neighbouring guard the r3723 port had as `f64::EPSILON`.
  Unit-pinned (`d3_watt_priority_sqrt_guard_zeroes_tiny_negative_radicand`).
- **D4 — delta-DER monitored voltage is line-to-line (adopt, capi015==r4133).**
  A delta controlled DER now monitors LL (`Vterminal[j]−Vterminal[next]`), not
  LN. Probe `scratch_probe_d4.py`: the LL/LN gap flips the var SIGN (+520 vs −31
  kvar). New snapshot capi015 deck `invcontrol/invcontrol_vv_delta.dss`;
  unit-pinned by `d4_delta_der_monitors_line_to_line_voltage` (a balanced delta
  reads √3·pu via the LL path) + its wye control `d4_wye_der_monitors_line_neutral_voltage`.
  Two
  default-oracle decks provably moved: `midi_controls`/`midi_invcontrol` each had
  a delta `pv3` under an InvControl — they are multi-step + carry unported
  U1.5/U1.6 control deltas, so they cannot flip to capi015; `pv3` is changed to
  `conn=wye` in both (documented at the site), the delta coverage moving to the
  dedicated deck. `gfm_invcontrol` unaffected (GFM ignores the monitored voltage).
- **D5 — InvControl9611: not a delta for us.** The compat flag is OFF-by-default
  in both 0.14.5 and 0.15.x; the r3723 port already reproduces the fixed branch
  (`update_deltaq_factor`). No code change.
- **C8 — (a) `VV_RefReactivePower` removal NOT adopted; (b) MonBus validations
  adopted.** (a) capi015 removes the property (#110 on write, 36 props); **r4133
  KEEPS it** (probe: 37 props, readback `varmax`) as does 0.14.5 — so per the
  plan's "r4133 wins" default the port **keeps** it (adopting the removal would,
  via the controls-family `compare_all_properties` count check, force the
  combo/midi InvControl decks onto capi015 and entangle unported U1.5/U1.6
  deltas — out of proportion; recorded as a not-adopted divergence). (b) the two
  MonBus guards (#2024111 missing-nodes at parse, #2024112 invalid-bus solve
  abort) ARE ported (capi015-specific messages, fire only on malformed input, 0
  corpus MonBus uses); unit-pinned.

**Oracle-infra finding (follow-up).** The **capi015 oracle cannot gate a
multi-step deck**: `oracle_server.run_case`'s per-step capture re-nominalizes
time-varying elements (loads/PV shapes read their step-0/nominal value while
`dbl_hour` advances) — witnessed `scratch_probe_srv3.py` (capi015 stuck at
nominal; default 0.14.5 steps correctly). All 18 pre-existing capi015 corpus
cases are `n_steps=1` for this reason; both WP-U1.3 capi015 decks are snapshots.
The D1 time-series hysteresis divergence is therefore unit-test-pinned, not
live-gated. A dedicated fix (capture element/injection state before the
`getYSparse`/fingerprint that re-nominalizes) would unlock multi-step capi015 for
future WPs — logged for the oracle-infra owner, out of WP-U1.3 scope.


---

> Appended verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

### NCIM re-gate — oracle-of-record flip capi015→r4133 (branch `ncim-regate`, 2026-07-20)

User-approved decision (2026-07-20): **r4133 is the oracle-of-record for NCIM.**
The capi015 0.15.0b4 probe venv is retired (gone from disk), so it can never again
be a live oracle; the only live NCIM-capable channel is r4133 via `epri-worker`.
The capi015 `NCIM_CalcInjCurrAtBus` off-by-one is no longer worth carrying — drop it
and live-gate.

- **Report path fixed.** `exec/view.rs::ncim_swing_source_currents` re-derived
  loop-for-loop against r4133 `PCElements/VSource.pas` `TVsourceObj.CalcInjCurrAtBus`
  (l.1085, reached from `GetCurrents` l.1194-1195 under `Algorithm=NCIMSOLVE`). r4133
  fills `Yorder+1` `ElmCurrents` with an **offset write** `GetCurrents(@(ElmCurrents[1]))`
  (l.1123 PD / l.1158 PC) then reads `ElmCurrents[(myTerm*stride)+j]`, `j:=1..NPhases`
  (l.1135 / l.1169) — a 1-based read of the offset-written array = **unshifted**. The
  port dropped the `+1` on both loop indices and the `TODO(compat)` was removed. The
  PC-loop stateful `myTerm` cross-element accumulation (r4133 still has it: `myTerm:=0`
  once at l.1146, not reset) stays NON-reproduced (UB refusal, comment re-cited to
  r4133 lines).
- **Re-pinned vs live r4133.** `ncim_vsource_reported_currents_match_oracle` now pins
  the r4133 `Vsource.source` currents/powers/losses captured via `epri-worker`
  (r4133 = `Version 11.0.0.1 (64-bit build) - Charlottesville`, own probe 2026-07-20).
  Before (capi015 shifted): conductor 0 = `70.71692 + 55.78569i` A. After (r4133
  unshifted): conductor 0 = `-83.67029 + 33.34980i` A (= negated Line.l1 terminal-1
  conductor 0); per-conductor power `-602.38906 - 240.10383i` kVA, losses
  `-1807167.18 - 720311.50i`. The node-V / regulation / warm-restart pins did NOT move
  (they already match r4133 ~1e-12) — all 10 `exec::tests::ncim` tests green.
- **4 cases live-gated on r4133, NO ledger entry.** `defer_ledger` removed from
  `modes/ncim/{ncim_pq,ncim_pv_pq,ncim_midi}` + `solvable_now Xmission_System_Kundur2Area`;
  filtered gate `4/4 passed`. The whole-model compare (node V digit-identical ~1e-12,
  system Y, the Vsource swing current/power/loss now matching to the tier floor, the
  NCIM generators) is clean, and the **warm-resolve** iteration count matches r4133
  EXACTLY (`ncim_pq=2, ncim_pv_pq=2, ncim_midi=2, Kundur=1`; no `rust<oracle` NOTE) — so
  the anticipated iterations divergence never materializes in the gate (it was a
  cold-solve artifact) and NOTHING is ledgered (a stale entry would fail the gate). The
  `ncim-oppoint` ledger cause is rewritten to the resolved reality (documentary), and
  the population lock flips `defer=1→0` on all 4 (the only lock change).
- **Docs.** `docs/upgrade/DIVERGENCES.md` NCIM section gained the flip decision with
  r4133 source lines + probe numbers; `tools/golden/gen_ncim_reports.py` header marks
  the capi015 venv retired and `tests/golden/ncim/` frozen/unregenerable (goldens
  byte-untouched — solver internals unaffected by the swing-report path).

**Settle (2026-07-20).** Two independent read-only audits (audit-code + audit-tests)
over `d0b59a9..853edff`. Audit-code verdict FAITHFUL, audit-tests verdict verification
STRENGTHENED — both re-derived the four mandate items from r4133 `VSource.pas` and each
ran its OWN `epri-worker` r4133 probe (`Version 11.0.0.1 (64-bit build) - Charlottesville`)
that reproduced the new `ncim_vsource_reported_currents_match_oracle` pins bit-for-bit
(independent solve, not port self-agreement). Settlement re-verified the load-bearing
invariants: `tests/golden/ncim/` byte-untouched (empty diff vs base); `ledger.json` has
NO new/widened structural row (only the documentary `ncim-oppoint` cause prose changed —
no `id`/`channel`/`case`/`max_rel`/`num_rel` edit → no disguised tolerance loosening);
`population.lock.json` diff is exactly the four `defer=1→0` flips on the NCIM fingerprints
and nothing else. Full three-command gate green at defaults; corpus pristine; 186 `.pas`
under `.inputs/dss_capi`.

Finding dispositions:

- **`ncim-deck-comment-stale-name` (audit-code, low) → DELIBERATE NON-FIX.**
  Reproduced: `git grep NCIM_CalcInjCurrAtBus` (excluding append-only STATUS history)
  leaves exactly one live hit — `tests/corpus/modes/ncim/ncim_pq.dss:7`, a DSS `!`
  comment still naming the retired capi015 `NCIM_CalcInjCurrAtBus` rather than the
  r4133 `CalcInjCurrAtBus` the port now targets. Proven inert: it is a comment line
  (not parsed — the deck solves identically), and `population_lock.rs` fingerprints
  only manifest fields + ledger entries, never deck bytes, so the stale name touches
  no gate, pin, or lock. Every *behavioral* citation is already r4133 (`view.rs` doc +
  comments, `exec/tests/ncim.rs`, `DIVERGENCES.md`, `ledger.json`, `modes/manifest.json`
  note). Not fixed because the settlement mandate constrains the corpus to path-limited
  cleanup only and the `.dss` deck fixtures were outside this WP's declared edit surface
  by design; editing a corpus deck comment is neither. Left as a one-line follow-up for
  a future corpus-touching pass to re-cite. No behavior/gate/pin impact either way.
- **audit-tests: no findings.**
