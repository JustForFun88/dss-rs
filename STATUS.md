# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

### UNIFIED_GATE Phase 0 — baseline recorded (2026-07-18)

`UNIFIED_GATE_PLAN.md` execution started (parallel worktree agents; Phases
A/B in flight on `ug-phase-a`/`ug-phase-b`, based `449c745`). Phase 0
baseline, tag **`pre-unified-gate`** = `449c745`:

- Full three-command gate wall-clock (measured 2026-07-18 01:52–02:00,
  **under concurrent load** — the ORPHANED_GAPS session was merging og*
  branches into `update` mid-run, so treat as an upper-bound baseline):
  fmt 1.4 s; clippy 50.5 s; `cargo +stable test --workspace` 426.6 s, of
  which the serial one-shot corpus_live suite = 292.6 s (27 tests; ~510
  cases across the four manifests). dss-core lib 1223+ unit tests.
- Populations at baseline: `solvable_now` 293 (oracle: 246 pinned /
  30 capi015 / 10 r4133 / 6 r3723 / 1 r4088), families asymmetric 47 /
  controls 101 / modes 69, `.dss` bijection 915, `known_diffs.json` 25
  entries (the Phase D ledger seed).
- The one red in the baseline run (`json_transformer_micro`) was an
  artifact of compiling mid-merge of og1213 (BHCurrent/BHFlux emission
  before its `SUPPRESS_JSON` fix landed) — not a unified-gate item;
  re-verified at the next merge-window gate.

Wall-clock table (rows appended per plan §6 at Phases B/D/F):

| point | fmt | clippy | test (full) | corpus gate share |
|---|---|---|---|---|
| `pre-unified-gate` (449c745, loaded box) | 1.4 s | 50.5 s | 426.6 s | 292.6 s |

### OG-1.4 AltDSS JSON import `Circuit_FromJSON` (orphaned-gaps round, 2026-07-18)

Ported the whole-circuit AltDSS JSON **reader** — the inverse of the JSON export —
on `og14-json-import` (branch based `883a656`). Closes `ORPHANED_GAPS.md` §1.4.

- **What:** `Dss::circuit_from_json(&mut self, json) -> Result<(),String>`
  (`exec/json_import.rs`) — a loop-for-loop port of `Obj_Circuit_FromJSON_` +
  `loadClassFromJSON` + `busFromJSON` (`CAPI_Obj.pas:2674-2983`), wrapped like
  `Circuit_FromJSON` (`CAPI_Circuit.pas`). Clear → DefaultBaseFreq → MakeNewCircuit
  → PreCommands → per-class load (`PASCAL_CLASS_ORDER`) → ReprocessBusDefs → Bus
  coords → PostCommands. Property application is `ClassProps::fill_from_json` /
  `set_json_value` (`obj/props/class_props/json_set.rs`), the port of
  `FillObjFromJSON` / `SetObjPropertyJSONValue`: walks the new **`AltPropertyOrder`**
  (`class_props/mod.rs`, driven by two new flags `ORDERING_FIRST`/`ORDERING_LAST`
  on LoadShape.MemoryMapping / Line.Switch / Transformer.XfmrCode / Load.PF),
  redirects the `array_alternative` (singular per-winding key → plural array),
  drives the typed struct setters for `ON_ARRAY` scalars, and renders every other
  type to the string its existing `edit_property` parse path reads (a scalar
  double round-trips bit-exactly via `f64::from_str`). A hand-rolled `parse_json`
  (`report/export/json/read.rs`) is the text→`Json` front (no serde_json, same
  reason as the writers).
- **Why the re-export differs from J0:** the imported set-order becomes
  `AltPropertyOrder` (not the original deck order), so `export(J0) != export(import(J0))`
  in general — but the oracle round trip is **idempotent after one cycle**
  (J1 == J2). The test therefore imports the oracle's J0 and requires the re-export
  to equal the oracle's own re-export **J1 byte-for-byte** (oracle `Circuit_FromJSON`
  is reachable on the pin), plus idempotency. Goldens `tests/golden/json_import/`
  (`rt_micro` / `rt_transformer` / `rt_ieee13`), generator `tools/golden/gen_json_import.py`,
  driver `tests/golden_json_import.rs` (+ negative tests: malformed/non-object/
  unknown-class/unknown-prop/missing-Name).
- **Bugs found + fixed in-scope (all gate-green):**
  1. **Transformer constructor `SetAsNextSeq(XHL)`** (`Transformer.pas:848`) was
     not reproduced — so a JSON-imported `X12` (or any never-edited transformer)
     rendered its impedance at the wrong set-order position. Added; the low-seq
     redundant `XHL` defers to canonical `X12`, so `X12` renders first. Safe for
     existing goldens (an explicit `xhl=` overwrites the seq).
  2. **Transformer `set_struct_f64_array`/`set_struct_i32_array`** only handled the
     plural array props (kVs/kVAs/…); extended to the `ON_ARRAY` per-winding
     scalars (RNeut/XNeut/MaxTap/MinTap/RDCOhms/NumTaps) for JSON import.
     `RDCOhms` deliberately leaves `RdcSpecified` to the side effect (marks only
     the *active/last* winding), so winding-1 Rdc is derived at recalc — matching
     the oracle round trip 1:1.
  3. **`add_object` split** into `create_object_no_edit` + `edit_active` so JSON
     import creates an element without the empty pre-fill recalc (a RegControl's
     `RecalcElementData` errors "transformer not set" if run before `FillObjFromJSON`
     applies the ref). Pascal `obj_NewFromClass` does the same.
  4. **`Set` command gaps** the export's PostCommands emit but the executive
     lacked: `%mean`/`%stddev` (default daily shape Set_Mean/Set_StdDev) and
     `genmult` (GenMultiplier). Ported (`set_cmd.rs`, `tables.rs`, LoadShape
     `set_mean`/`set_std_dev`).
- **Deferred (recorded):** the `DynInit` tail of `FillObjFromJSON` (ORPHANED_GAPS
  §1.2, Generator/PVSystem/Storage `DynamicExp` init) — mirrors the export-side
  `DynInit` deferral; not exercised by any covered deck. The public wrapper takes
  no `joptions` (only the import-internal `DSSJSONOptions.Edit` bit matters and it
  is applied internally).
- **Gate:** fmt + clippy clean; `cargo test --workspace` green (all 27 live-corpus
  cases match the oracle — the transformer/add_object changes cause no divergence).
  NOTE: the worktree lacks the Oddie `tools/opendss/.venv` junction (4 corpus cases
  need it); run with `DSS_OPENDSS_PYTHON` pointing at main's venv, else those cases
  abort on setup (environment, not a code failure).

### OG-1.4 AltDSS JSON import — settle/fix round (2026-07-18)

Settled two independent read-only audits of `og14-json-import`. Fixes (all
oracle-probed, gate-green):

- **`Required`-property validation (major, AUDIT-CODE):** ported the missing
  `FillObjFromJSON` branch (`DSSObjectHelper.pas:4955`) — a missing `[Required]`
  key now raises `JSON/<cls>/<name>: required property not provided: "<prop>"` and
  aborts the load instead of silently importing an incomplete element. Added the
  `PropFlags::REQUIRED` bit (absent before — only `RequiredInSpecSet` existed) and
  the check in `class_props/json_set.rs`, then flagged the ~38 **non-redundant**
  Pascal-`Required` props across 28 element tables (bus1/bus2, kV, per-winding
  Bus, MonitoredObj/Element/transformer/capacitor refs, Sensor element+kvbase,
  DynamicExp Expression, XfmrCode kV, VSource bus1+basekV). Redundant twins
  (`buses`/`kVs`) are dropped from `AltPropertyOrder`, so only the exported keys
  are checked — round-trip goldens stay green. Oracle-confirmed the exact message
  on dss-python 0.15.7.
- **Edited default DSS_OBJECT re-exported (major, AUDIT-TESTS):** `fill_active_from_json`
  called `set_default_and_unedited(false)`, but Pascal `FillObjFromJSON` never
  `BeginEdit`s (only `EndEdit`), so it never clears `DefaultAndUnedited`. Removed
  the clear — a JSON-imported default (e.g. `spectrum.defaultload`) now stays
  flagged and is dropped from the re-export, matching the oracle (whose own round
  trip is lossy for edited defaults: J0 2328 B → J1 1458 B). New golden
  `rt_edited_default` pins it.
- **`busFromJSON` kVLN+kVLL conflict now aborts (minor, AUDIT-CODE/TESTS):**
  `bus_from_json` returns `Result`; the conflict propagates as `Err` (oracle
  aborts the whole load, error 20230919) instead of logging-and-continuing.
- **Test coverage (AUDIT-TESTS):** restored the two dropped whole-circuit decks as
  import goldens (`rt_positive_seq` = allowduplicates + cktmodel=positive;
  `rt_edited_default`) and added `rt_generator` (Thevenin-DER). Tightened the
  `unknown_class`/`missing_name` negatives to assert the positive/abort outcome,
  and added `missing_required_property_errors` + `bus_kvln_kvll_conflict_aborts`.
- **Rejected — DuplicatesAllowed (AUDIT-CODE, disproven):** `create_object_no_edit`
  already honors it (`command.rs:987`, gated on `!duplicates_allowed`, not
  unconditional as the audit read). Probe: oracle round trip of two duplicate
  `load.l1` under `AllowDuplicates` → Rust import re-export **byte-identical** to
  the oracle J1 (2 loads each).
- **Deferred (recorded follow-up) — numeric-array length validation (minor,
  AUDIT-CODE):** Pascal `SetObjPropertyJSONValue` rejects wrong-length int/double/
  complex/sym-matrix arrays (`Expected an array of %d …`); the Rust renders to a
  string and reparses without the `Norder` count check. Malformed-hand-authored-
  input only (round-trip exports are always correct length). Left for a follow-up;
  needs the per-type expected-count machinery in `set_json_value`.

Last updated: 2026-07-17 (late evening) — **STATUS RESTRUCTURED + PLAN-COMPLETION AUDIT.** All 16 plan docs were re-verified against the codebase; the records of the *completed* plans (FINAL ACCEPTANCE, JSON export, DIAKOPTICS Part I, UPGRADE Rung 1+2) moved to the new **§1a archive**, and every item those plans handed to a still-unfinished successor is now explicit in **§Standing open follow-ups**. Prior same-day — **TEST-TRIAGE ROUND MERGED** (user-ordered
backlog burn-down; six parallel worktree WPs, each gate-green + audited/verified,
merged wt-t1→t2→t3→t4→t6; DE_PASCALIZE is PAUSED by user order after wave 1 —
wave-2 WIP salvaged to origin as `wt-p5a`/`wt-p1b`/`wt-p1213`, R1 not started).
Records: `docs/phase-records/test-triage-{promotions,ad-classify,monitor-windings,indmach,infra-audit}.md`.
- **Stale-skip promotions (wt-t1).** Kundur2Area NCIM → `solvable_now`
  (`oracle:"capi015"`, kind large; §1.7 two-process bit-identical; Rust matches
  capi015 to 0 on all 33 nodes; warm 1 iter == oracle). **IEEE118Bus NOT
  promoted** — the earlier "port converges" read was a stalled iterate
  (`is_solved=false`); Rust reproduces capi015's NCIM non-convergence
  **byte-identically** (both stall at identical voltages, 100 iters) while EPRI
  r4088/r4133 converge (2 iters) via their newer NCIM PV→PQ switching cadence —
  the documented report-only DIVERGENCES item; re-tagged
  `ncim_pv_pq_switching_divergence` (future rung adopts the cadence). WindGen
  GFLDaily ×2: #263 block was stale (WindGen ported) but both decks are
  multi-step → blocked by the capi015 per-step re-nominalization limit;
  re-noted honestly. `solvable_now` 292→293; lock regenerated.
- **AD classify round (wt-t2).** The 9 `off:unclassified-new-deck` entries in
  `ad_sweep.json` got measured verdicts via DSS_AD_CLASSIFY/DECOMPOSE: 3 → `pf`
  (both 8500-Node masters + GFM twin; gaps ~7e-6 ≪ tier), 6 → measured `off:`
  reasons (indmachtest `ad-floor-above-tier` leg2=2.469e-3; StevensonPflow-3ph
  `ad-switched-divergence` mesh; StevensonPflow `non-3ph-cut-only` pos-seq;
  Kersting4wire ×2 `ad-singular-zone` panic-probed; IEEE30 `too-small`). Bucket
  emptied for ad_sweep (family manifests keep their separate follow-up round).
- **Monitor modes 8/10/12 — PORTED (wt-t3).** The deferred TakeSample bodies
  (winding currents / winding voltages / line-line V, Pascal `Monitor.pas`
  1:1) — the panic path is gone. New pinned golden `monitor_windings` (0.14.5
  oracle via `gen_metering_monitors.py`), unit pins in `exec/tests/monitors.rs`;
  audit settlement TIGHTENED the unit close() band 1e-3→1e-6+1e-7·|x| (measured
  floor ≤4.7e-7). Mode-12 upstream terminal-currents UB documented in
  `investigations/monitor_mode12_terminal_currents_ub.md` (local, gitignored).
- **InductionMachine r4133 — VERDICT: INTENTIONAL breaking change (wt-t4,
  fable-xhigh investigation, adversarially CONFIRMED).** The r4133 fuse
  overhaul (CurveMultiplier is the TCC divisor; RatedCurrent demoted to
  nameplate, NO legacy fallback; default curve none) is announced in the 11.0
  release notes, executed coherently across all four protection classes + COM
  API. Physics: Fuse.f2 carries ~53.5 A ≈ 0.82× of the 65 A link (holds, sound
  transformer-primary fusing) but 53.5 multiples under divisor 1.0 → blows at
  Sec=0 → island → non-convergence; EPRI shipped their own example un-migrated.
  Port keeps r4133 parity (no DIVERGENCES entry — nothing diverges). **Equivalent
  coverage added:** corpus twins `controls/fuse/indmach_r4133/{indmach_snap,
  indmach_dyn}.dss` (single edit `CurveMultiplier=65`, restores the exact
  r4088-era coordination incl. the full relay/recloser fault sequence),
  `oracle:"r4133"`, §1.7-validated, live-gated in the mandatory gate; originals
  stay parked as SETTLED. Harness: `compare_monitor` gained the **proven
  f32-ULP floor** (+ its angular image) — decomposition-proven (99.25% of
  490k-sample diffs are exactly 1 ulp at 2.8e-11 rel f64; TOLERANCE_NOTES
  §monitor-f32-floor); this is a floor-proof band, not a widening.
- **EPRI DLL crash repros.** All four #303 crash classes bisected to minimal
  repro decks + Delphi-source suspects:
  `investigations/epri_dll_crashes_r4088_r4133.md` (local). Upstream-binary
  bugs; port-side correctness re-affirmed; nothing to fix in the port.
- **Test-infra audit (wt-t6).** TESTING.md + TOLERANCE_NOTES synced to the
  tree; findings + dispositions in
  `docs/phase-records/test-triage-infra-audit.md`.
- Gate after merges: fmt/clippy clean, `cargo +stable test --workspace` exit 0;
  population.lock consistency re-proven by deliberate regen (no diff).

**Prior — DE_PASCALIZE wave 1 MERGED (stage 5 opens): R0 +
P1(partial) + P2 + P6**, executed as four parallel port→audit→fix worktrees
(wt-r0 / wt-p1 / wt-p2 / wt-p6, each independently gate-green + opus-audited),
merged into `update` in that order (final merge `e7cfc1e`). UPGRADE_PLAN is
COMPLETE (Rung-2 exit record below); per `PLAN_SEQUENCE.md` the active plan is
now **`DE_PASCALIZE_PLAN.md`**. All four WPs are stratum **[A]** (bit-neutral):
zero golden/tolerance churn — the untouched byte goldens are the equivalence
proof. Full records: `docs/phase-records/depascalize-{r0,p1,p2,p6}.md`.
- **R0** (Part I): `ControlElem` trait + `ControlClass` over all 12 control
  classes (dispatch.rs identification chain collapsed; error prefixes
  byte-identical), `ElemStore::kind()` type-guards (zones/build, take_sample),
  `ConductorData` trait (Wire/CN/TS, ~13 downcasts), typed `present_tap` read.
  Downcasts 766→699 (−67). R2 handoffs recorded (heterogeneous
  sample/do_pending_action, Reg→Transformer / Cap→Capacitor pairs,
  `capture_metered` bare-`&dyn` sites).
- **P1** (partial — 7 families fully converted, each discriminant-pin-tested):
  `DynSolveMode` rename, `AddType`, `SolveAlgorithm`, `LoadStatus`,
  `StorageDispatchMode`, `CoreType` (non-contiguous), `LineType`. The deferred
  remainder (Solution control_mode/load_model/random_type ctx ripple, InvControl
  family, Storage f_state + StorageController via the control-queue i32 channel,
  var_mode, item-7 element families, bare-i32 fields incl. `Winding.connection`
  [P10 dependency], `MonPhase`, Tier-2) is enumerated in the record file
  §Deferred — a P1-continuation WP.
- **P2**: `MonitorModeView` typed decode stored on Monitor (masks 15/16/32/64
  pinned in `from_raw`/`to_raw` only). Audit caught a genuine [A] violation —
  undefined base modes 13/14/15 + junk bits ≥128 were lossily canonicalized —
  fixed: the view carries the verbatim `raw: i32` (lossless boundary), new
  `Undefined` base variant = timestamp-only sample row + general header
  (Pascal `else Exit` / `ClearMonitorStream` else), 2 new pins.
- **P6**: 125 identifier-path `to_lowercase()`→`to_ascii_lowercase()` /
  `eq_ignore_ascii_case` conversions across 53 files; report-text paths
  untouched. Both audits PASS; note recorded: ASCII folding is the
  plan-prescribed byte-based behavior (the Unicode folding was the latent
  divergence for non-ASCII identifiers) — permanent semantics, not a bug.
- Toolchain drift: stable-1.96 clippy flags 4 pre-existing sites
  (`matrices.rs` doubled-`.re` bug pin, `inc_matrix`, windgen test, harness) —
  fixed bit-neutrally in wt-r0 `1e8dcdf`; wt-p1/wt-p6 converged on the same
  fixes (one doc-comment merge conflict, resolved keep-fullest).
- Gate on merged `update` (`e7cfc1e`): fmt clean, clippy clean, `cargo +stable
  test --workspace` exit 0 — 47 suites, dss-core lib 1223, corpus_live 27
  (548 s), 0 failures; `tests/corpus` pristine.
- **Next (wave 2):** R1 typed arenas (opus-xhigh exec + xhigh audits) ∥ P5a
  miette diagnostics ∥ P1-continuation ∥ P12+P13 — then R2 (opus-high) → R3;
  Part III P8/P10/P11/P14/P15 after R2 (P10 needs `Winding.connection` from
  P1-continuation).

**Standing toolchain note:** the gate runs on **`stable`** (`cargo +stable …`),
matching CI (`dtolnay/rust-toolchain@stable`) — no nightly dependency. `dss-core`
carries `#![allow(clippy::collapsible_match)]` (`d85d026`): clippy 0.1.96 (now on
stable) mis-fires that lint on the byte-faithful `match prop { CONST => if cond
{..} }` port idiom, and its autofix even drops `else` branches.

> **Working cadence:** finish one small step → run the full gate → update this
> file → **stop and wait for explicit user confirmation** before the next step.
> The full per-step ritual (gate, STATUS sync, the two audits) is
> **`PHASE7_PLAN.md §0`**, run per **§1e**. (Earlier phases sometimes executed
> several WPs in one pass on explicit user instruction.)

---

## 1. Where we are

### OG-1.5 `CAPI_Schema` JSON-schema export — static core ported (orphaned-gaps round, 2026-07-18)

Branch `og15-capi-schema`. Ported the **static core** of Pascal
`DSS_ExtractSchema(DSS, jsonSchema=True)` (`CAPI_Schema.pas:1252-1521`): the
JSON-Schema (draft 2020-12) envelope (`$schema`/`$id`/`type`/`required`), the ten
reusable global `$defs` (`Complex`, `PComplex`, `SymmetricMatrix`,
`ArrayOrFilePath`, `StringArrayOrFilePath`, `JSONFilePath`, `JSONLinesFilePath`,
`Bus`, `BusConnection`, `DynInitType`), and the static `circuitProperties` head
(`Name`/`DefaultBaseFreq`/`PreCommands`/`PostCommands`/`Bus`).
- New: `crates/dss-core/src/report/export/json/schema.rs` (reuses the existing
  fpjson `Json` tree + `write_pretty`); public `Dss::extract_schema_json()` in
  `exec/view.rs`.
- Test surface: `tools/golden/gen_schema.py` (pin-checked; proves the oracle
  bytes deterministic across two processes; self-validates each rendered fragment
  by verbatim containment in the real 592 KB oracle output), golden
  `tests/golden/json/schema_static_core.json`, driver
  `crates/dss-core/tests/golden_schema.rs` — **byte-equality** on all 10 static
  defs + the 5 head props + the `$id`/`required` envelope.

**Deferred (genuinely orphaned, blocked on unported metadata):** the per-class
walk (`prepareClassJsonSchema`) and per-enum walk (`prepareEnumJsonSchema`) —
i.e. the `<Class>`/`<Class>List`/`<Class>Container` `$defs` triples (**49 class +
21 global enum defs**) and their `circuitProperties` refs — need per-property
metadata the Rust port never carried and which is a large, self-contained
data-entry effort:
- property **help/description** text (`GetPropertyHelp`; ~1109 strings in the
  oracle document),
- per-class **`AltPropertyOrder`** (`$dssPropertyOrder`, 1161 occurrences),
- **`SpecSets`** / `SpecSetNames` / `RequiredInSpecSet` (the `oneOf` blocks, 78),
- enum **`AltNames`/`JSONName`/`JSONUseNumbers`** JSON metadata (not on `DssEnum`),
- ~28 of the ~30 `Units_*` property flags (only `UNITS_HOUR` /
  `UNITS_OHM_PER_LENGTH` exist on `PropFlags` today).

The mission-brief premise that these inputs "already sit inert, ready to feed the
emitter" is only partly true (the `Units_*` family in particular is largely
absent). Recorded as the remaining `ORPHANED_GAPS.md` §1.5 follow-up. Oracle IS
reachable (`lib.DSS_ExtractSchema`) — the blocker is Rust-side metadata, not
oracle access. Gate green (fmt/clippy/test).

**Settle round (2026-07-18).** Two read-only audits reviewed the branch. The one
Major finding (WP as literally briefed = ~90% deferred) is the honestly-disclosed
partial documented above — not a defect; the deferral is empirically justified
(only 2 of ~30 `Units_*` flags carried) and stays open in ORPHANED_GAPS §1.5. Two
Minor findings fixed: (a) `extract_schema_json()` now carries a `# Incomplete`
rustdoc header spelling out that the returned skeleton is not a usable schema
(dangling `required:["Vsource"]` + `circuitProperties` refs whose class `$defs`
are absent); (b) `skeleton_envelope_is_well_formed` gained a byte-level
top-level-member-order assertion against the Pascal envelope order
(`CAPI_Schema.pas:1504-1513`) — serde's object map ignored ordering, so an
envelope reorder previously slipped all four tests.

**Era: post-acceptance DE_PASCALIZE (PLAN_SEQUENCE stage 5).** The 1:1 port
reached FINAL ACCEPTANCE (2026-07-11, referee ACCEPT); UPGRADE Rungs 1–2 are
COMPLETE (2026-07-16/17 — engine behavior = OpenDSS 11.0.0.1 (r4133) except the
documented ledger). Active work is **DE_PASCALIZE_PLAN.md** on the `update`
integration branch: wave 1 (R0 / P1-partial / P2 / P6, all stratum [A]) merged
2026-07-17 — see the frontier block above. Remaining sequence:
DE_PASCALIZE Parts I–III + Stage F → RESONANCE → MULTITHREADING M0–M4;
Part II A-Diakoptics (WP-AD.2–AD.6) after MULTITHREADING M2. The records of the
completed plans (FINAL ACCEPTANCE, JSON, DIAKOPTICS Part I, UPGRADE Rung 1+2) are now
archived in **§1a**; their still-open carried-forward items (TODO(compat) sweep +
HIDE_015X → Stage F, GICMvars → Phase 9, JSON DynInit/Full tail, AggregateProfiles →
AD Part II, user-model DLLs → WASM, IEEE118 NCIM → a future rung) are tracked in
§Standing-open-follow-ups just below.

### OG-1.7 UPFC modes 2/3/5 (orphaned-gaps round, 2026-07-18)

Branch `og17-upfc-modes`. `ORPHANED_GAPS.md` §1.7. **The engine code already
handled all six modes** (0..5) — `elements/pc/upfc/compute.rs`
`get_output_curr`/`get_input_curr`/`check_status`/`calc_upfc_powers` are a
loop-for-loop port of `UPFC.pas` and match the oracle to the printed precision on
modes 2/3/5. The real gap was the **missing test surface**: no corpus deck
exercised modes 2 (StatCOM shunt reactive), 3 (Dual = series V-reg + shunt PF),
or 5 (DoubleRef Dual). Added three live decks under `tests/corpus/controls/upfc/`:
- `upfc_statcom.dss` (mode 2): series path off (Sr0=0), shunt QIdeal ~3.1 kvar
  drives the monitored service-transformer PF to pf=0.95; `element=` mandatory
  (CheckStatus mode 2 = checkPF only); 16 iters.
- `upfc_dual.dss` (mode 3): mode-1 series V-reg to refkV **plus** the synced shunt
  reactive branch (QIdeal ~7570 var); `element=` required or it collapses onto
  mode 1; 25 iters.
- `upfc_doubleref_dual.dss` (mode 5): mode-4 two-band reference (lower band
  engaged, Vbout→refkV2) **plus** the shunt PF branch (QIdeal ~7.8 kvar); 25 iters.

Each GAPS §3-proven (pin solves+converges; two-process bit-identical fingerprint;
feature-sensitive — mode 3 vs mode 1 and mode 5 vs mode 4 share the series Sr0 but
QIdeal→0, isolating exactly the dual shunt branch; mode=0 zeros both). Registered
in the controls `manifest.json` + `CONTROLS_REQUIRED` floor with
`compare_variables:[UPFC.test]` (all 14 state vars) + mode/refkv/pf/element probes;
`controls_cases_match_oracle` green (Rust == pinned dss-python 0.15.7 on full model
+ all UPFC variables + properties). Convergence needs a **reachable** PF target
(pf=0.99 unclamped only with kvarLimit≥20; the kvarLimit clamp otherwise stalls
checkPF at max-control-iterations) and tol1≥0.005 to settle both deadbands — the
non-convergence the pre-existing `upfc_vreg` note warned about. No engine code
changed.

**Settle round (2026-07-18, two audits).** Both audits confirmed the engine port is
faithful (all 6 modes loop-for-loop) with no regressions/simplifications and the
decks feature-sensitive vs the pinned oracle. The single substantive finding (both
audits, minor): the three new decks are all snapshot (`n_steps:1`), so the brief's
"multi-step solve if the mode has temporal state" clause was not literally met even
though UPFC carries genuine cross-step `Sr0/Sr1` shift-register state. **Closed** by
adding a fourth deck `upfc_dual_daily.dss` (mode 3, `n_steps:4`, daily loadshape
0.7/1.0/1.25/0.9): the load ramps each hour so the UPFC re-regulates from the
`Sr0/Sr1` carried over from the prior step. The cross-step channel is proven — at
step1 the dual series deadband does not re-fire so `Sr0` stays at the step0 value
(79.40,-261.23) while mode=1 on the same feeder moves `Sr0` to (111.26,-328.50);
the persisted `Sr0` is exactly the cross-step divergence a snapshot cannot reach.
GAPS §3-proven (4 steps converge 22/16/26/24 iters; two-process bit-identical;
mode 1/0 feature-sensitivity per step); `compare_variables:[UPFC.test]` all 14 vars
+ full model compared PER STEP; `controls_cases_match_oracle` green. Second finding
(untracked `GFM_IEEE8500/IEEE8500u_VLN_Node.txt` solve byproduct) was a pre-declared
unrelated GFM output, not part of this WP's diff — removed from the worktree to keep
`tests/corpus` pristine; nothing to commit there.

### Standing open follow-ups (actionable)

**Carried-forward handoffs — work a *declared-complete* plan deferred to a
successor plan that has NOT finished it** (audited 2026-07-17; surfaced here so the
open item is not buried in the §1a archive):
- **TODO(compat) bug-for-bug sweep → DE_PASCALIZE Stage F — NOT started.** 123
  `TODO(compat)` shims across 71 files still in-tree (PORTING_PLAN §4.1 rule 4, the
  single dedicated post-acceptance cleanup pass, re-assigned to Stage F). Stage F is
  unbuilt (DE_PASCALIZE paused after wave 1) → cleanup unexecuted.
- **UPGRADE §5 exit criterion `rg HIDE_015X` empty → DE_PASCALIZE Stage F — NOT
  started.** 15 `HIDE_015X` refs in 5 files (line / line_geometry / prop_flags /
  save/dump). UPGRADE was declared COMPLETE having consciously waived this own-§5
  criterion to Stage F (byte-neutral, non-rung-blocking); still unmet.
- **IEEE118Bus NCIM PV→PQ switching-cadence → a future UPGRADE rung — NOT started.**
  Port matches its capi015 oracle loop-for-loop incl. non-convergence, but not
  r4133's newer cadence; parked `skipped_needs_investigation`, report-only in
  DIVERGENCES.md.
- **GICMvars export (verb 36) / GICTransformer `WriteVarOutputRecord` → Phase 9 —
  ✅ PORTED 2026-07-18** (orphaned-gaps round OG-1.1, branch `og11-gicmvars`; see
  §OG-1.1 below). Was GAPS WPG.16's only deferred piece.
- **AltDSS JSON `DynInit` tail + Full-mode Transformer WdgCurrents — DONE
  (og1213, 2026-07-18; see §OG-1.2+1.3).** Capacitor CMatrix = proven UB
  non-port (uninitialized heap, nondeterministic across processes). New
  sub-follow-ups surfaced (below).
- **AutoTrans JSON array-alternative metadata → NOT started (og1213 discovery).**
  `auto_trans/mod.rs` carries none of the singular/plural `array_alternative` +
  `REDUNDANT` + `ON_ARRAY` JSON metadata the Transformer has, so its default JSON
  sweep renders `Buses/Conns/kVs/kVAs` where the oracle renders `Bus/Conn/kV/kVA`.
  Blocks an AutoTrans JSON golden (incl. Full WdgCurrents, which is *inferred* to
  work via the class-agnostic refresh route — proven post-solve on Transformer,
  but AutoTrans's own getter is not independently oracle-pinned). Out of §1.3 scope.
- **Generator/PVSystem/Storage `ShaftModel`/`ShaftData` hidden under JSON Full →
  NOT started (og1213 discovery).** They carry `PropFlags::NOT_PORTED` →
  `hidden_from_full_enum()` skips them, but the 0.14.5 oracle emits them (`""`).
  Blocks a Generator/Storage *Full* JSON golden; the DynInit tail is pinned in
  default-family combos instead.
- **A-Diakoptics `AggregateProfiles` command + D9(d) official-r3723 AD-replay →
  DIAKOPTICS Part II WP-AD.5 — partial.** `exec/command.rs:69` `NOT_PORTED`; WP-AD.6
  threaded children not started (needs MULTITHREADING M2).
- **User-model native DLLs (Gen/PVSystem/Storage/CapControl UserModel) →
  WASM_USERMODELS — NOT started.** All still `PropFlags::NOT_PORTED`; the sandboxed
  wasmi replacement is unbuilt (`#![forbid(unsafe_code)]` cannot load a DLL).

**Residual floors / parked (documented, not bugs):**
- **ckt24 RegControl/LDC `SubXFMR`** ~4.7e-5 rel tap-current — ultra-switch
  conditioning floor (CF-D), watch on re-touch.
- ~~**UPFC modes 2/3/5**~~ **CLOSED 2026-07-18 (OG-1.7)** — see §OG-1.7 record;
  `midi_relay_dist` deferred (budget); Kersting4wire #567
  UserModel decks parked (no oracle channel tolerates the DoSimpleMsg).
- **UTF-8-BOM edge cases** — GAPS follow-up. (`CapControl.ControlSignal` FOLLOW path
  is in fact *ported* and live in `cap_control` — the old "unported" note was stale
  and is retired.)

Retired (done): combo fuse-save restore (wt-combo); WP-U1.2 D3 / WP-U1.6 tail (all
landed pre-rung-exit); Monitor modes 8/10/12 (test-triage wt-t3).

### OG-1.1 GICMvars export (orphaned-gaps round, 2026-07-18)

Ported `Export GICMvars` (report verb 36) — the last unported GIC surface
(ORPHANED_GAPS §1.1; GAPS WPG.16's punt to a never-materialized "Phase 9").

- **`WriteVarOutputRecord`** → `GicTransformer::var_output_record`
  (`elements/pd/gic_transformer/solve.rs`): `ComputeIterminal`, sum the per-phase
  terminal currents, `GICperPhase = |ΣI|/nphases`; Mvar on the K-factor path
  (`FKfactor·FkV1·GICperPhase/1000`) or the VarCurve path
  (`GetYValue(pu)·FMVArating/√2`, pu = GICperPhase/(MVA·1000/kV1/√3)).
- **Driver** `ExportGICMvar` → `report/export/gic_mvars.rs::export_gic_mvars`:
  walks the GICTransformer class ElementList (creation order, no Enabled filter);
  header `Bus, Mvar, GIC Amps per phase`; `%.8g` cells. Verb 36 routed in
  `exec/report.rs` (default file `EXP_GIC_Mvar.csv`; not solution-guarded, 1:1 with
  Pascal `ExportOptions.pas`).
- **Test surface:** new golden `export_gicmvars` (generator `gen_gic_mvars` in
  `tools/golden/gen_reports.py`) over the GIC-study deck — all three types
  (GSU/YY K-path + Auto VarCurve path); `golden_reports.rs::export_gicmvars_matches_oracle`
  (rel 1e-7/abs 1e-8, the faer-vs-KLU GIC-current floor). Retired the
  `exec/tests/report.rs` GICMvars NOT_PORTED assert → now pins `Estimation`(5) on a
  solved circuit (all remaining unported verbs are solution-guarded).

### OG-1.2+1.3 AltDSS JSON tails — DynInit + Full WdgCurrents (orphaned-gaps round, 2026-07-18)

Branch `og1213-json-tails`. Closes `ORPHANED_GAPS.md` §1.2 and the WdgCurrents
half of §1.3.

- **§1.2 DynInit tail — PORTED.** `obj_to_json_data` now appends the Pascal
  `TDynEqPCE` `"DynInit"` object (`CAPI_Obj.pas:752-759`) for any object whose
  `UserDynInit` is non-empty (Generator/PVSystem/Storage, reached via a new
  `DssObject::as_dyneq` accessor). `DynEqPceData.user_dyn_init` changed from
  `Vec<(String,String)>` to `Vec<(String,DynInitValue)>` where `DynInitValue` is
  `Number(f64)` | `Text(String)`: `parse_dyn_var` uses `make_double_ex` to
  recover Pascal's `requiredRPN`, so a plain constant → JSON number, a
  calc-value operand or RPN constant → JSON string (raw case). The `"DynInit"`
  key is emitted literally (never lowercased, even under LowercaseKeys — matches
  oracle). Golden `dyneq_micro` (Generator + Storage; default-family combos).
- **§1.3 Full WdgCurrents — PORTED (refresh route).** Added
  `Dss::obj_to_json_mut` / `class_batch_to_json_mut`: they run
  `refresh_vterminal_if_marked` before the `&self` builder, so Full-mode
  `READS_VTERMINAL` result strings (Transformer/AutoTrans `WdgCurrents`) render
  from the current solution exactly as Pascal's self-refreshing getter does
  (pre-solve = all-zero phasor list). Golden driver uses the `_mut` routes.
  `transformer_micro` flipped off `skip_full` → Full WdgCurrents now pinned
  byte-exact. Fixed en route: Transformer/AutoTrans `BHCurrent`/`BHFlux` (newer
  r4064 props absent from the 0.14.5 oracle) leaked into Full JSON as `null` —
  now `SUPPRESS_JSON` like their sibling `BHPoints`.
- **§1.3 Capacitor CMatrix — proven UB, NOT reproduced.** The oracle renders
  Capacitor `CMatrix` under Full from an uninitialized `pDoubleArray` (denormal
  garbage: `2.1e-308`, `4.9e-318` …) that **differs across oracle processes**
  (probed twice), for both kvar- and explicit-`cmatrix`-defined caps. Per the
  UB/state-mutating-read rule it is not reproduced (like the multi-meter OOB
  case); decks with capacitors stay `skip_full`.
- Gate green (fmt + clippy + `cargo test --workspace`). Out-of-scope discoveries
  recorded in Standing follow-ups (AutoTrans JSON array-alt metadata; Generator
  ShaftModel/ShaftData hidden under Full).

**Audit-settle round (2026-07-18).** Two read-only audits returned 6 findings
(1 major, 5 minor); settled empirically against the pinned oracle:
- **[FIXED — major] Full WdgCurrents pinned only pre-solve (a no-op).** The
  pre-solve refresh recomputes zeros (NodeV=0), so `transformer_micro` alone
  could not catch a refresh regression. Added golden **`transformer_solved`**:
  the transformer primary is on the energized `sourcebus` feeding a 500 kW load,
  the deck `solve`s, and Full `WdgCurrents` is now a NONZERO phasor list pinned
  byte-exact (obj + batch × 4 Full combos). Verified Rust==oracle bit-for-bit;
  byte-exact is valid post-solve because the getter formats at `%.7g`/`%.5g`
  (Transformer.pas:362), far coarser than faer-vs-KLU last-ULP. Deleting the
  `refresh_vterminal_if_marked` call now fails the gate.
- **[FIXED — minor] DynInit dedup rewrite unpinned.** `dyneq_micro` now assigns
  `Damp` twice (`= 0` number, then `= (1 2 +)` RPN string): pins the Pascal
  `UserDynInit.Delete`+`Add` reorder — the rewrite changes the value type AND
  moves `damp` to the tail. Oracle-confirmed and reproduced byte-exact.
- **[disproven — minor] AutoTrans "proven byte-exact by analogy".** Overstated;
  softened the `gen_json.py` NOTE to "inference, not verified". The refresh route
  is class-agnostic (now proven post-solve via `transformer_solved`), but a
  standalone AutoTrans byte-golden stays blocked by the plural/singular metadata
  gap (already a follow-up below). Real, deferred — not silently dropped.
- **[not-a-defect — minor] WindGen `as_dyneq` override.** `WindGen.pas` is absent
  from the 0.14.5 pinned source (Rust-only forward-port from a newer engine where
  WindGen IS a `TDynEqPCE`); it embeds a real `dyneq` field, so emitting DynInit
  is internally consistent. Cannot appear in any oracle golden/live compare, so
  untestable and harmless — kept for sibling consistency (Generator/PVSystem/Storage).

**Audit-settle round 2 (2026-07-18, post-merge on `update`).** Two further
read-only audits returned findings; most were already remediated in-branch by
38a5e67 (the pre-solve Full WdgCurrents no-op and the DynInit dedup — both re-flagged
against the `afd8853` HEAD, RESOLVED above). One material gap survived and is fixed:
- **[FIXED — major] AutoTrans `WdgCurrents` getter never verified nonzero.** The
  §1.3 deliverable names *both* Transformer AND AutoTrans WdgCurrents. Transformer
  is pinned nonzero by `transformer_solved`, but AutoTrans has its OWN distinct
  series/common/delta getter (`TAutoTransObj.GetAllWindingCurrents`,
  `auto_trans/yterminal.rs`), and every AutoTrans WdgCurrents golden was pre-solve
  all-zeros — indistinguishable from a broken refresh. Added props scenario
  **`autotrans_solved`** (`gen_props.py`): a solved 3-winding YNad1 auto (Series/
  Common/Delta-tertiary, unit from corpus `autotrans_snap.dss` t1) fed on the series
  winding, loads on the 161 kV common + 13.8 kV tertiary. The `?`-query path hits
  `refresh_vterminal_if_marked` (`command.rs:1107`) → reloads Vterminal → runs the
  auto's own getter. `props_roundtrip` now pins `WdgCurrents` NONZERO
  (`549.4296, (-31.051), …`) vs the pinned 0.15.7 oracle; Rust matches numerically.
  A broken refresh would emit all-zeros and fail. Closes the last un-verified §1.3
  getter. (JSON-Full AutoTrans golden stays blocked by the plural/singular metadata
  follow-up below; the props route needs no JSON metadata and closes the gap.)
- **[deferred — minor] DynInit tail not gated under Full mode.** `dyneq_micro` is
  `skip_full` (blocked by the ShaftModel/ShaftData NOT_PORTED Full-render gap,
  already a follow-up). The DynInit append is sweep-independent code fully exercised
  by the default sweep, so residual risk is low; kept as-is. Recorded, not dropped.

---

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # 0 failures (2026-07-17 WP-U2.6 round):
                            # dss-core lib 1213, golden_reports 197 (incl. the
                            # capi015 seasonal pair), corpus_live (292
                            # solvable_now cases live-compared; modes family 58)
                            #   (corpus_live_solvable_cases_match_oracle +
                            #    solvable_now_has_multistep_depth run
                            #    UNCONDITIONALLY — the pinned oracle MUST be
                            #    installed (it fails, not skips, without it);
                            #    only corpus_live_classify is opt-in, via
                            #    DSS_LIVE_CLASSIFY=1 — the growth/classify probe),
                            # dss-parser 62+1, dss-sparse 15
                            #   (6 complex SparseSet + 9 real RealSparseSet —
                            #    WP-U1.7 Stage 1, the NCIM Jacobian path)
# WP-U2.6 opt-in EPRI sweeps (not part of the mandatory gate): both green —
#   DSS_LIVE_OPENDSS=r4133 DSS_LIVE_OPENDSS_ASSERT=1 → 326/70/4/0
#   DSS_LIVE_OPENDSS=r4088 DSS_LIVE_OPENDSS_ASSERT=1 → 329/67/4/0
```

### Phase 5 gate — green  *(detail → `docs/phase-records/phase-5.md`)*
- `golden_feeders_controls.rs`: the unmodified IEEE13/IEEE37/IEEE123 masters
  (controls active) + `ieee34mod1` match the Phase-0 goldens — converged + total
  iterations exact, `YNodeOrder` exact, RegControl `tap_number` / capacitor
  `states` exact, final taps 1e-12 rel (the integer `tap_number` is the exact
  discrete check), V/I/P 1e-6, and every element's full property dump.
- `golden_phase5.rs` vs `tests/golden/phase5/*.json` (`gen_phase5.py`):
  `daily_ieee13`, `duty_2bus`, `eventlog_ieee13`, `capcontrol_micro` — per-step
  `dblHour` + iteration counts exact, **event logs line-for-line** (normalized),
  per-step V 1e-6 (the shape-scaled `Yeq` restamp per Y build, `a6903f1`).

### Checkpointed-model gate (`crates/dss-core/tests/golden_checkpoints.rs`) — green
- `gen_checkpoints.py` → `tests/golden/checkpoints/<scenario>.json` (schema 2,
  one file per scenario; the gate runs every file in the directory, so adding a
  scenario is just adding a file). Unlike the
  other command-replay gates (which compare only converged outputs), this one
  captures the **assembled electrical model after every committed time step** —
  the unfactored system Y, selected element YPrim blocks, the injection vector,
  node voltages, and discrete control state — and compares each to the oracle.
  A stale Y/YPrim fails at the step and matrix entry it first goes wrong, not as
  downstream register drift. Scenarios: `micro_yeq_steps` (control-free daily,
  full-CSC per-step pin), `ieee13_daily` (24-step daily with regulator tap
  changes — full CSC + fingerprint; the direct regression guard for the
  "frozen load Yeq" bug: reverting commit `a6903f1` makes it fail at step 6,
  `Y[634.1]`), `ieee123_snap` (large-feeder fingerprint-only + selected YPrim
  path). Tolerances: `tests/TOLERANCE_NOTES.md`. The assembled Y is compared
  **unfactored** so the `dss-sparse` row equilibration is out of scope.

### Live corpus oracle gate (`crates/dss-core/tests/corpus_live.rs`) — runs unconditionally
See `CORPUS_TEST_PLAN.md`. The whole `electricdss-tst` corpus is **vendored** into
`tests/corpus/electricdss-tst/` (1544 files, 122 MiB; `tools/corpus/vendor.py`,
`.git` excluded, with `SHA256SUMS` + `README.md` provenance) so tests no longer
depend on the temporary `.inputs/electricdss-tst`.
- **Manifest accounting (always-on).** Every `.dss` (915) is in exactly one
  manifest under `tests/corpus/manifests/` (`solvable_now`, `skipped_unsupported`,
  `skipped_oracle_issue`, `skipped_needs_investigation`, `missing_dependency`,
  `not_an_entry_point`). `corpus_manifest.rs` enforces the bijection — no silent
  omissions — and runs in the normal `cargo test`: adding/removing a `.dss` fails
  it until the file is classified.
- **Live comparison (runs unconditionally in `cargo test`; the pinned oracle must
  be installed).** For each of the **293** `solvable_now` cases the gate
  compiles+solves on the Rust engine and on the pinned dss-python oracle
  (`tools/oracle/oracle_server.py`, a
  one-shot subprocess over JSON), and compares the full assembled model per step —
  node order, **full** system Y (entry-by-entry, no fingerprint substitution),
  node voltages, **every** element's currents/powers, selected YPrim blocks (a
  guard fails the case if the oracle returns no YPrim for a named selected
  element), the injection vector, and discrete state — reusing the `harness/mod.rs`
  comparators and the checkpoint gate's tolerance policy.
  - **Three control-diverse 24-step daily runs** — `IEEE13Nodeckt` (wye gang
    reg), `ieee37` (delta, open-delta LDC reg bank) and `IEEE123Master` (multiple
    cascaded reg banks) — each with a meter + three monitors (modes 0/1/2) +
    selected elements, so the **multi-step per-step**, **YPrim**,
    **monitor-channel** and **EnergyMeter-register/zone** paths are all exercised
    live (`compare_monitor`/`compare_meter`, the *same* comparators
    `golden_phase6.rs` now routes through, gated per case by
    `check_meters_monitors`). Incidental master-defined monitors are *not*
    compared — the pinned oracle returns a phantom `Channel(i)` for an unsampled
    monitor (see `tests/TOLERANCE_NOTES.md`).
  - The **IEEE 8500-Node master is promoted** (snapshot; `post: Set
    Maxiterations=20` to converge — the bare probe didn't, which is why the
    classifier had parked it), so the full 8531-node Y, every element's I/P, and a
    YPrim block are compared live at scale (complementing the always-on
    `golden_ieee8500.rs` golden, whose `compare_discrete` also pins the full
    1190-transformer tap set here).
  - **Depth is guarded always-on.** `solvable_now_has_multistep_depth` (no oracle)
    asserts `solvable_now` keeps ≥1 multi-step `check_meters_monitors` case and ≥1
    case with selected elements, so the deep coverage can't silently revert to
    snapshots. The solvable + classify tests **auto-skip (pass)** without the env
    var / oracle, so `cargo test --workspace` stays green everywhere; the
    **`live-oracle` GitHub Actions job** installs the pinned oracle (PIN.txt) and
    runs the **whole `corpus_live` binary** (not a name filter that could green on
    zero matched tests). The oracle server hard-asserts **both** dss-python 0.15.7
    **and** engine 0.14.5 (PIN.txt). No goldens are written; the oracle is
    consulted live.
- **Growth.** `DSS_LIVE_CLASSIFY=1 corpus_live_classify` probes the
  `skipped_needs_investigation` candidates with the full comparison and writes
  `tmp/classify_report.json`; `tools/corpus/apply_classify.py` promotes the
  passing cases into `solvable_now` (and routes oracle/engine failures to the
  right skip bucket). `tools/corpus/coverage_report.py` →
  `tests/corpus/COVERAGE.md` tracks the burn-down toward 100% of entry points.

### Phase 4 gate (`golden_feeders.rs`) — green  *(detail → `docs/phase-records/phase-4.md`)*
The controls-off IEEE13/37/123 variants (`gen_phase4.py`) match `phase4.json`
(pinned oracle): converged + iterations exact (3/3/3), `YNodeOrder` exact
(41/117/278), V 1e-6, every element's I/P 1e-6 (creation order), total
power/losses 1e-6. The Phase-3 `golden_slice.rs` (13 scenarios) stays green; the
CLI runs the real masters (`cargo run -p dss-cli -- script.dss`).

---


---

## 1g. UNIFIED_GATE Phase B — persistent oracle pools + hand-rolled scheduler (branch `ug-phase-b`)

`UNIFIED_GATE_PLAN.md` §4 Phase B. Behavior-identical parallelization of the live
corpus gate; no comparison, tolerance, golden, or `harness` change.

- **Rename + module tree.** `crates/dss-core/tests/corpus_live.rs` → `corpus_gate.rs`
  (git rename, `--follow` preserved) with `#[path="corpus_gate/…"]` submodules:
  `manifest.rs` (schema + loaders + family completeness + AD disposition — schema
  UNCHANGED, `oracle` field still honored, `ORACLE_SPECS` stays), `engines.rs`
  (one-shot `Oracle` + persistent `WorkerPool`/`Worker` + `Channel`), `runner.rs`
  (`run_rust_capture` + `compare_capture` split of `run_and_compare`, CorpusGuard,
  abort/pending), `scheduler.rs` (task grouping + thread pool + proof modes).
- **Persistent pinned pool (D8).** N long-lived `python -u oracle_server.py`
  (`DSS_ORACLE_ENGINE=capi`) workers, ping-verified per process, dedicated
  stdout-line + stderr-drain threads, one in-flight request each, per-request
  deadline `DSS_ORACLE_TIMEOUT_SECS` (default 120) → kill/respawn/retry-once-then-
  fail-case, recycle after 64 cases. Target-rev cases (`capi015`/`r3723`/`r4088`/
  `r4133`) keep the ONE-SHOT `Oracle` path via `Channel::OneShot`.
- **Scheduler (D7a + §3.3).** ONE `#[test]` `corpus_gate_all_cases_match_engines`
  over the union of all four manifests (510 cases); task = case-dir group (cases
  sequential inside → NO per-dir mutexes), longest-first static weight,
  `AtomicUsize` cursor + `std::thread::scope`, T = `DSS_GATE_JOBS` |
  `available_parallelism()` (16), per-channel pool `max(2,T/2)`. Per-case
  `catch_unwind`; the one test fails iff any case failed and prints the COMPLETE
  list (manifest order) — replaces the 4 abort-at-first tests
  (`corpus_live_solvable_cases_match_oracle` + `{asymmetric,controls,modes}_cases_
  match_oracle`). Structural manifest guards, `corpus_live_{classify,properties,
  opendss}`, and the AD sweep are preserved (relocated, not changed).
- **`isolate` (the one permitted early schema addition, §1.2).** Additive
  `isolate: bool` (+ `note`, `isolate ⇒ note` enforced structurally); an isolate
  case runs every execution on a throwaway one-shot worker. Default false; absent
  everywhere until the contamination proof demanded it.

**Contamination proof (§4 Phase B DONE bar).** Ran the full 510-case set three
ways — (a) serial one-shot (`DSS_GATE_SERIAL=1`, T=1, fresh process per case), (b)
persistent parallel, (c) persistent parallel shuffled (`DSS_GATE_SHUFFLE=1234567`) —
each dumping a label-sorted `{verdict, result}` artifact (`DSS_GATE_DUMP`). All
three **bit-identical** — sha256 `e041e018…08116` on all three 1.276 GB dumps,
`diff -q` empty pairwise (settle re-run 2026-07-18).

Three real persistent-worker contamination classes were found and root-caused
(they pass one-shot, fail/pollute persistent) → `isolate: true` (22 cases):
- **AutoAdd process-exit corruption** (`modes/autoadd/autoadd{,_cap}.dss`): the
  documented AutoAdd solve corrupts the dss-python process, so the NEXT case on a
  reused worker access-violates on `clear`. Isolated → the corruption dies with
  the throwaway process.
- **debugtrace held-open trace CSV** (17 solvable_now cases: 7 ckt24-mm masters +
  EPRI/ADiakoptics ckt24 + DOCTechNote + NCIM + StorageTechNote): a `debugtrace`
  RegControl/Storage keeps a fixed-name trace CSV open in the persistent worker,
  so a different worker locks the file (`#303 being used by another process`) and
  the CorpusGuard cannot delete it (pollution). Isolated → the handle releases at
  process exit.
- **File-backed loadshape read drift** (3 `modes/inputformat` decks —
  `shape_mmf`, `shape_filearr`, `shape_binfiles`; found during the settle gate
  re-run, 2026-07-18). A pooled worker reused across many decks intermittently
  (~1-in-3 full-workspace runs) misreads a deck's file-backed loadshape data
  (binary `sng`/`dbl`, `csv`, `MemoryMapping=Yes`) after some prior deck, giving a
  ~2e-3 step-0 node-voltage drift on the ORACLE side. Root-caused empirically: the
  Rust value is bit-stable (`#![forbid(unsafe_code)]` ⇒ race-free; `load_shape`
  has no shared/global mutable state, no mmap); the oracle value drifts only under
  the pool. Never reproduced in 150+ one-shot / in-process-reuse / 12-way
  concurrent-process / sibling-predecessor oracle solves (all correct), and the
  pre-Phase-B one-shot gate + the serial proof run were green — so it is pooled
  cross-deck worker-state contamination, not intrinsic per-process nondeterminism.
  Isolated → a fresh dedicated oracle process per run (exactly the pre-Phase-B
  execution these decks were validated under) removes all predecessor state;
  7/7 consecutive full-workspace gates green post-isolate vs ~2/6 pre-isolate.

`all_properties` heap-UB handling (settled 2026-07-18). The upstream dss_capi
`DoubleSymMatrixProperty` getter (`RMatrix`/`XMatrix`/`CMatrix`/`GMatrix`) and the
shunt-PD reliability inputs render UNINITIALIZED heap memory — order-dependent by
construction (a reused worker's heap carries prior-case residue). Those pairs are
exactly what `harness::skip_prop` (`SKIP_PROPS`) already excludes from the value
compare, so verdicts are unaffected. `write_gate_dump` nulls ONLY those UB values
(keeping the property name) and RETAINS every other property — so the three-way
bit-diff independently re-proves the byte-stability of all 72 k+ gate-asserted
property strings across reused/shuffled workers (2128 `all_properties` blocks
present in the dump, still sha256-identical). This is the ONLY source of
cross-process nondeterminism; every other property is a deterministic function of
the deck.

**Audit dispositions (settle, 2026-07-18).**
- **B1 — dropped `pending`⊕`expect_solve_abort` mutual-exclusivity assert** (both
  audits, low). REAL, latent (0 manifest cases set both, empirically checked). The
  old family-only `assert!` was not carried into the unified classifier, which
  silently resolves the conflict pending-first. FIXED: restored the `assert!` in
  `scheduler::make_case` — now applied to EVERY source (solvable_now + families),
  strictly stronger than the family-only original.
- **B2/B3 — dump stripped the whole `all_properties` block** (audit-code B2 /
  audit-tests B3, low). REAL completeness gap: a within-tolerance oracle-property
  drift on a reused worker could escape both the verdict channel
  (`assert_value_matches_tol`) and a whole-block strip. FIXED: `strip_ub_properties`
  now nulls only the `skip_prop` UB values, retaining all value-asserted properties;
  the three-way bit-diff is still byte-identical (proof above), so the fix strictly
  strengthens the artifact with no regression.
- audit-tests B2 (stray `.claude_tmp_corpus_live_old.rs` leftover) — not present;
  worktree clean, corpus pristine.

**Wall-clock** (settle-run 2026-07-18, 16-core, incl. dump write).

| mode | jobs | pool | wall-clock |
|---|---|---|---|
| serial one-shot (old cost model) | 1 | 2 | 341.9 s |
| persistent parallel | 16 | 8 | 104.3 s (3.3× faster) |
| persistent parallel shuffled | 16 | 8 | 78.1 s |

`tests/corpus` pristine after all runs (the vendored decks are untouched; the
corpus_gate CorpusGuard restores its own case dirs). `cargo fmt/clippy/test` green
in the worktree.

**Open follow-up (pre-existing, non-blocking).** A full `cargo test --workspace`
occasionally leaves a handful of untracked solver EXPORT outputs under
`tests/corpus/electricdss-tst` (e.g. `Test/AutoTrans/Auto3bus_noload_power.txt`,
IEEE8500/StorageControllerTechNote monitor/EXP CSVs). These are `Export`/monitor
files a deck writes to a path the per-case `CorpusGuard` (which guards only the
case's parent dir) does not sweep — a pre-existing CorpusGuard corner case
independent of Phase B (the guard logic is byte-identical to the old gate) and of
this settle's diff. They are untracked (never committed) and path-limited-cleaned
before commit. A future CorpusGuard hardening (guard the export CWD too) would
close it.

---

## 1h. UNIFIED_GATE Phase C — manifest schema v2 (`engines`) + population lock v2 + r4133 channel wiring (branch `ug-phase-c`)

`UNIFIED_GATE_PLAN.md` §4 Phase C (after the A+B integration merge, base `3ad37c8`).
Retires the target-rev one-shot Oracle/Oddie shim for the mandatory gate: the
`r4133` channel now gates through the in-house `epri-worker` pool (Phase A bridge).

- **Schema v2 (§1.2).** `SolvableCase.oracle: Option<String>` → `engines: "capi_v0145"
  | "r4133" | "both"` (default `"both"`; no case is `"both"` this phase). Migration
  preserved semantics exactly: pinned (no `oracle`) → `capi_v0145` (246 solvable_now
  + 162 family); every target-rev case (`capi015`/`r3723`/`r4088`/`r4133`) →
  `r4133` (106 total = 47 solvable_now + 4/34/21 asym/controls/modes). `ORACLE_SPECS`
  deleted; `EngineChannel {CapiV0145,R4133}` + `SolvableCase::engine_channels()`
  replace it. `corpus_manifest.rs` untouched; `.dss` bijection 915 intact.
- **r4133 worker pool (§3.1).** `engines.rs` gains `EpriPool` (persistent
  `epri-worker` processes, mirroring the capi `WorkerPool` lifecycle: per-request
  deadline → kill/respawn/retry-once/recycle-64, `{"epri":true,"rev":"r4133"}` ping
  assert) + `EpriOneShot` (throwaway worker for serial/`isolate` r4133 cases).
  Worker-binary resolution: `DSS_EPRI_WORKER` env → `target/<profile>/epri-worker` →
  OnceLock `cargo build -p dss-epri` fallback. `Channel` now has four arms
  (Capi/Epri × Pool/OneShot). Iteration policy is per-channel: capi_v0145 = exact
  1:1; r4133 = `rust_le_oracle`. Eventlog masks keyed on the channel. The r4133
  request masks `all_properties` off (capi_v0145-only per §1.2 — the bridge has no
  all-props capture).
- **Re-validation (§4/§5 R9) — all 106 r4133 cases gated live against the epri
  bridge.** 95 pass green; **11 deferred** to Phase D (all were `oracle:capi015`,
  the retired 0.15.0b4 line, and reproduce on NEITHER surviving channel — proven by
  flipping all 11 to `capi_v0145` and re-running: 0.14.5 also diverges). The 99
  live-gated cases split by their prior pin (base-commit `oracle` counts across
  the four manifests: capi015 59, r4133 40, r3723 6, r4088 1):
  - **40 were already `oracle:r4133`** → same revision, same value contract; the
    only change is the *transport* (in-house `epri-worker` replaces the Oddie
    one-shot). Phase A cross-validated epri-worker == Oddie **bit-for-bit on the
    same r4133 DLL** over 396 cases, so these are unchanged by construction.
  - **6 `oracle:r3723` + 1 `oracle:r4088` + 48 `oracle:capi015`** (55) → a genuine
    **revision re-pin**: their value contract shifts from "matches r3723/r4088/
    capi015-0.15.0b4" to "matches r4133". These are NOT covered by the Phase A
    bit-for-bit proof (that proof is bridge-equivalence for one DLL, not
    r3723≡r4133 or capi015≡r4133 behavior); the guarantee is the fresh green
    re-validation that Rust == r4133 for each (D4/D5: r3723/r4088/capi015 retired
    to r4133 as the single surviving EPRI line). All 55 gated green. Honest re-pins,
    not no-ops — noted so the revision shift is not mistaken for pure transport.
  - The remaining **11 capi015** cases could NOT re-pin to r4133 (both channels
    diverge) → deferred (see below).
- **`defer_ledger` — the Phase D ledger seam (§4 step c / §5 R9 last resort).** New
  optional field carrying the Phase-D-ledger-seed cause (+ mandatory `wp`, mutually
  exclusive with `pending`/`expect_solve_abort`). A deferred case is parked from live
  oracle comparison but still **Rust-smoke-run** (compile + solve every step must
  converge, no new errors) so a Rust regression can never hide. Membership is
  preserved (counts unchanged; the population lock records the `defer` flag) — the
  plan's "membership never shrinks" holds. The 11 deferrals, by class:
  - **NCIM ×4** (`NCIM/Xmission…Kundur2Area`, `modes/ncim/{ncim_pq,ncim_pv_pq,ncim_midi}`):
    0.14.5 lacks NCIM (`Set algorithm=NCIM` ignored, `Export deltaf` #24713); r4133-11.0
    NCIM converges to a different PV/Q op-point → `Vsource.source` current diverges
    wholesale. wp `WP-U1.7`.
  - **DynExp ×2** (`Dynamic_Expressions/Dynamic_KundurDynExp`, `IBRDynamics_Cases/GFL_IEEE123/
    Run_IEEE123Bus_GFLDaily_DynExp`): capi015 D14 `Exit`-no-op (frozen state); 0.14.5
    swings, r4133 differs → node V ~1.5e-5 rel.
  - **GFM ×3** (`controls/gfm/{gfm_micro,gfm_invcontrol,gfm_dynamics}`): capi015 B5 Isc1
    (drop ×1000) — 0.14.5 system-Y differs (9 entries); r4133 physics MATCHES but
    `Storage.batt.%stored` property STRING is r4133-rounded (92.4320641163609 vs
    92.4321). wp `WPG.10`/`WPG.13`.
  - **RegControl idle ×1** (`controls/regcontrol/regcontrol_idle`): capi015 `idle`
    property (0.15 feature) — 0.14.5 rejects `idle` (#110); r4133 node V ~7e-5 rel.
  - **line_spacing_asym ×1**: r4133 raises **#303 access violation at calcv** (known
    linespacing crash) AND 0.14.5 gives `Line.normamps=230` vs Rust/capi015 730 —
    diverges on BOTH channels (Phase D: r4133 `skip` + capi_v0145 property ledger).
- **Population lock v2 (§1.4).** `Case::rigor` drops `oracle=`, gains
  `engines={…} isolate={…} defer={…}`; the documented asymmetry is fixed — the three
  family manifests are now per-case rigor-covered (`family_rigor` replaces
  `family_paths`). Regenerated via `DSS_UPDATE_POPULATION_LOCK`; reviewed diff =
  field additions + **zero membership loss** (counts 293/47/105/69 bit-stable;
  solvable_now + family key sets identical to base).
- **Report channels kept compiling.** `corpus_live_opendss` + `known_diffs.json` +
  `DSS_LIVE_OPENDSS*` retained (Phase D deletes them); their target-rev exclusion is
  re-keyed on `engines` (contains r4133). `corpus_live_classify`/`_properties`
  re-keyed on `gates_capi()`. `AdSweepCase.oracle` (unused; ad_sweep.json had 0
  oracle values) dropped with its `ORACLE_SPECS` validation.

**Counts (bit-stable at base):** solvable_now 293, asymmetric 47, controls 105,
modes 69, `.dss` bijection 915.

**Gate (three-command, green).** `cargo fmt --all --check` clean; `cargo clippy
--workspace --all-targets -- -D warnings` clean; `cargo test --workspace` all pass
(dss-core lib 1231 + corpus_gate 25 incl. the 514-case unified gate; 1 pre-existing
`ckt24_graph_diagnostic` ignored). Corpus gate wall-clock **with the r4133 channel
active = 64.5 s** at the Phase-C build; the settle re-run (audit-fix build, same
machine) measured **67.4 s** for the `corpus_gate` target (main gate test 60+ s).
`tests/corpus` pristine after runs (path-limited-cleaned 12 pre-existing CorpusGuard
export-CWD leftovers — StorageControllerTechNote monitor CSVs + AutoTrans txt;
STATUS §1g open follow-up, not introduced by Phase C).

Wall-clock table row (plan §3.4 / §6):

| point | mode | jobs | pool | corpus gate |
|---|---|---|---|---|
| Phase C (r4133 channel active) | persistent-parallel | 16 | 8 (per channel) | 64.5 s |
| Phase C settle (audit-fix build) | persistent-parallel | default | default | 67.4 s |

**Deviation from plan (justified).** The brief's ladder step (c) said `pending: true
+ wp`, but `pending` structurally asserts the Rust engine ERRORS (unported feature);
the 11 deferrals are PORTED features that solve cleanly — `pending` would fail the
gate. Introduced `defer_ledger` instead (the plan's "leave a clear seam" for the
Phase D ledger, §1.4): same intent (park from live compare, documented cause + wp,
membership preserved, reviewed lock diff) without the false "must error" contract,
and it adds a Rust-side smoke net `pending` also lacks. Brief header said "76
re-targeted"; the actual re-targeted (oracle-carrying) population is **106** (the
brief's own parenthetical sums to 106); all 106 re-validated.

### Phase C settle — audit dispositions + empirical deferral proof (2026-07-18)

Two independent audits (audit-code, audit-tests) of `3ad37c8..b89b2d3` returned
**four low-severity findings**, all about the 11 `defer_ledger` cases. Each settled
**empirically** against the live r4133 `epri-worker` and the pinned dss-python 0.14.5
oracle in the worktree (not by argument).

**Re-validation table (106 target-rev cases, by prior pin → r4133 channel):**

| prior `oracle` | count | outcome on r4133 channel |
|---|---|---|
| r4133 | 40 | green — transport-only (epri-worker == Oddie bit-for-bit, Phase A) |
| r3723 | 6 | green — revision re-pin, Rust == r4133 |
| r4088 | 1 | green — revision re-pin, Rust == r4133 |
| capi015 | 48 | green — revision re-pin, Rust == r4133 |
| capi015 | 11 | **deferred** — diverges on BOTH surviving channels (proof below) |

**Empirical both-channels-diverge proof for the 11 deferrals** (settle probes, live):
- **GFM ×3** (`gfm_micro`/`gfm_invcontrol`/`gfm_dynamics`): on r4133 the worker
  returns `Storage.batt.%stored` = **"92.4321"** (property getter rounds to 4
  decimals); capi 0.14.5 and Rust return the full-precision **"92.43206411636…"**.
  The `micro` tier allows `1e-6 + 1e-9·|v| ≈ 1.09e-6`, but the string gap is
  **3.59e-5** → the probe genuinely fails on r4133. On capi_v0145 the assembled
  system-Y differs (B5 Isc1 ×1000, DIVERGENCES.md #b5 — Rust adopted the r4133 Isc1).
  Both channels diverge. **Why the sibling `pv_gfm_dynamics` gates r4133 GREEN and
  is NOT deferred** (the audit's specific question): it probes `PVSystem.pv`
  `irradiance`/`pmpp`/`kva`, which the r4133 worker returns as the stable input
  strings **"1"/"800"/"800"** (no state-integrated float, no getter rounding) — the
  distinguishing probe is `%stored`, not the physics, which matches r4133 for all
  four (Rust uses the r4133 Isc1). Verified with the worker on both decks.
- **line_spacing_asym ×1**: the r4133 worker **aborts at compile with #303 access
  violation** (`Error 303 Reported From OpenDSS Intrinsic Function`); capi 0.14.5
  gives `Line.normamps=230` vs Rust/capi015 730. Both channels diverge (confirmed).
- **RegControl idle ×1**: capi 0.14.5 **rejects `idle` with #110** ("Unknown
  parameter idle") — a 0.15 feature; r4133 idle-regulator node V ~7e-5 rel. Both
  diverge (capi side confirmed live).
- **NCIM ×4**: capi 0.14.5 has **no NCIM** (`Solution.algorithm` unknown to the 0.14.5
  API; `Set algorithm=NCIM` ignored) — confirmed; r4133 NCIM converges to a different
  PV/Q op-point than the Rust NCIM port (WP-U1.7). Both diverge.
- **DynExp ×2**: capi015 D14 `Exit`-no-op freezes the state-var seed; 0.14.5 swings,
  r4133 evaluates differently (documented D14, DIVERGENCES.md; STATUS §UPGRADE).
  Both diverge.

**Audit dispositions:**
- **C-1 (both audits) — FIXED.** The deferred-smoke doc comment (`runner.rs`) and
  the `defer_ledger` field doc (`manifest.rs`) overclaimed "a Rust regression can
  never hide behind the deferral." The smoke asserts only per-step convergence +
  unchanged error count — it catches a *convergence/error-surfacing* regression but
  **not** a *numeric-correctness* regression that still converges (no physical value
  is compared). Both comments reworded to scope the guarantee accurately and note
  that full numeric coverage returns with the Phase D ledger. No assertion changed.
- **C-2 (audit-tests) — FIXED.** The re-validation prose conflated bridge-equivalence
  (epri-worker == Oddie on the *same* r4133 DLL, Phase A) with revision-equivalence.
  Reworded above to split the 40 transport-only r4133 cases from the 55 genuine
  revision re-pins (6 r3723 + 1 r4088 + 48 capi015), whose contract shifted to
  "matches r4133" and rests on the fresh green re-validation, not the bit-for-bit
  proof.
- **C-2 (audit-code) — RECORDED, deliberately NOT code-fixed.** `defer_ledger` has no
  *mechanical* guard that the R9 ladder was exhausted (it enforces cause + `wp` +
  mutual-exclusion only). A mechanical guard is infeasible in an oracle-free
  structural test: proving both channels diverge requires running both live oracles,
  which the structural `manifest.rs` tests deliberately do not. The interim controls
  are (a) the reviewable population-lock diff (`defer=1` per case), (b) the mandatory
  documented per-case cause, and (c) — added here — the live both-channels-diverge
  proof above for all 11. Each deferral is a Phase-D ledger seed; the ledger machinery
  (envelope/probe carve-out) lands the mechanical re-gate. Recorded, not masked.

## 1i. UNIFIED_GATE Phase D — divergence ledger + seeding + engine flips (branch `ug-phase-d`)

Lands the gating divergence ledger (`tests/corpus/ledger.json`) that replaces the
report-only `known_diffs.json`, seeds it from a full both-channel measurement,
flips the cleanly-dual-gateable cases to `engines:"both"`, deletes the retired
Oddie/OpenDSS report path, and makes the both-channel gate **deterministic**. Base
`ae4b4ef`.

**Ledger machinery (`corpus_gate/ledger.rs`, 9d5852b + settle 35ad4e2).**
`LedgerRuntime` loads/validates `ledger.json` (kinds `divergence`/`skip`/
`exclusion`) and exposes per-(case,channel) `LedgerView`s that `compare_capture`
consults to partition each comparison field: the untouched `harness` comparator
runs the unscoped remainder; the ledger's envelope/exact-pair assert covers the
scoped part and records the hit. §1.3 envelope: (a) selected values differ ≤
envelope; (b) unselected meet the tier floor; (c) ≥1 selected value **exceeds the
tier floor** else the entry is **stale → gate fails**. `exceeded_floor` is measured
against the untouchable `harness` tier floor, NOT the entry's `max_rel` — so
widening an envelope can never hide a shrinking divergence (a stronger
fail-on-stale than the plan's phrasing). Structural test (oracle-free): unique ids,
case∈manifest, channel∈engines, divergence⇒non-empty match, cause/cause_ref
resolve, regexes compile, probe/property exact-pair-only.

**Seeding (`DSS_GATE_SEED_LEDGER=1`).** Ran every Live/Deferred case against BOTH
channels with no ledger → `tmp/ledger_candidates.json`: 510 cases ×2 = 1020
measurements, 836 match / 157 diverge / 27 error. **Zero** currently-single-channel
case matches on both channels — the inherited flip already captured every free
flip; further `both` requires a hand-reviewed ledger entry per case.

**Gate determinism — the R2 worker-state contamination, surfaced + fixed.** The
both-flip widened the r4133 pool's exposure enough to surface plan R2 live:
persistent pooled workers accumulate state that `clear` does NOT reset (`Set`
options, memory-mapped loadshape handles), so a worker that had served, e.g., a
relay / harmonics / IEEE13-geometry deck would *intermittently* hand the next deck
a stale option/mmap → ~1e-3 divergences on **either** channel that vanish
serial/one-shot (~1 flake per 6 full runs — the same deck matches cleanly in the
seeding). Fixed systemically in `engines.rs`: **`recycle_after()` now defaults to 1
(a fresh worker per case)** — every case sees a never-used worker, so no state is
inherited; the persistent pool keeps only its amortized startup. Wall-clock is
unchanged (respawns overlap across the pool). `DSS_GATE_RECYCLE_AFTER=<n>` raises it
for a faster, non-deterministic dev loop. Complementary `isolate:true` (one-shot,
process exits → releases handles) added for the file-handle-contention decks the
recycle cannot cover within a case: `StoCtrl_SeasonTarget` (EnergyMeter DI CSV held
open), plus defensive isolate on the IEEE13-geometry family + `mmf_singlecol` +
`YgD-Test`. Determinism verified: **4/4** consecutive green full runs post-fix
(after ~10 pre-fix runs that whack-a-mole isolation could not stabilise). `M1/
Master_NoPV` kept **r4133-only** (its both-flip surfaced a capi_v0145
all-properties divergence — a Load renders `PF=1` on the port vs `0.9` on the 0.14.5
oracle — **settled at Phase-D settlement, NOT a bug**: see the settlement addendum
below).

**Deletions (same landing).** `tests/corpus/known_diffs.json`, the
`corpus_live_opendss` test + `KnownDiff`/`load_known_diffs`, `Oracle::opendss` +
`oddie_venv_python` + the ping `want_oddie` arm, and the `DSS_LIVE_OPENDSS*` knobs.
`rg "known_diffs|DSS_LIVE_OPENDSS|Oracle::capi015|corpus_live_opendss" crates tests`
is clean of live code (doc-comment prose only). The `tools/opendss/*.py` report
scripts still reference the removed file — they die in **Phase E** per the brief.

**Lock v2 ledger component (§1.4).** `population_lock.rs::rigor()` gains
`ledger={sorted per-channel entry ids}` (`ledger_tags()`); every ledger add/widen/
flip changes the case's tag → a reviewed lock diff (e.g. `ledger=r4133:r4133-
binaryshape-303`), so the ledger cannot become a silent soft-tolerance backdoor.

**Ledger contents.** 6 entries / 20 causes: 3 `skip` (binary/MMF GrowthShape #303,
IEEE13 line-spacing + line-and-cable-spacing #303 on r4133) + 3 GFM r4133 probe
`divergence` (`Storage.%stored` %.6g Delphi rounding, `num_rel` 1e-6) — the
Phase-C seed, each hit + non-stale every run.

**Canary (fail-on-stale, live, §6).** A temporary all-node voltage divergence entry
on `vsource_asym` (which matches the oracle within the tier floor) was added: the
full gate passed all 514 cases, recorded the entry as applied (9 hits) but
never-exceeded-floor, and **FAILED** with `STALE — every selected value is now
within the tier floor. Prune it.` Reverted immediately. Fail-on-stale proven live.

**defer_ledger retirement — partial (honest).** GFM×3 retired in the Phase-C seed
(r4133 probe ledger). Of the remaining **8**, disposition settled empirically
(seed + focused probes) — the field REMAINS because 4 cases cannot be responsibly
retired:
- **NCIM×4** (`ncim_pq/pv_pq/midi`, `NCIM/Xmission`): capi 0.14.5 lacks NCIM
  (errors); the Rust NCIM port (validated vs the now-retired capi015) converges to a
  **wholesale-different op-point** than r4133 NCIM (Vsource source-current sign-flip
  ~156 A, and the Rust source-bus voltage reads suspiciously exactly-nominal). Not
  tightly fingerprintable (plan: stays single-channel) and possibly a port issue →
  per R3 NOT ledgered; needs a rigorous WP-U1.7 NCIM re-validation. Kept
  `defer_ledger` (Rust-smoke), note corrected.
- **DynExp×2** — ledgerable but deferred for measurement: the port deliberately
  adopts capi015's D14 evaluator (DIVERGENCES §D14, *settled*); the data confirms
  **both** 0.14.5 AND r4133 agree with each other (179425.907) and the port differs
  by ~1.5e-5 — a documented cross-line divergence, not a bug. Retirement needs a
  measured both-channel voltage envelope (follow-up).
- **RegControl idle×1** — capi rejects `idle` (#110); r4133 ~7e-5 regulator-tap
  class. Ledgerable on r4133 with a measured envelope (follow-up).
- **line_spacing_asym×1** — capi node-V ~7e-8 (line-impedance libm floor) +
  `Line.lsp.normamps`/`emergamps` **oracle 0.14.5 = 730/1095, port = 230/345**
  (settled empirically 2026-07-18; the port's 230 min-over-phase is CORRECT per
  r4133 LineGeometry.pas — earlier notes had the direction reversed, now fixed) +
  r4133 #303 skip. Ledgerable capi voltage + property exact-pair, but the discrete
  730→230 jump needs an exact-pair-numeric probe/property scope the current
  machinery lacks (probe path only exact-pairs non-numeric values) → follow-up.

**both% = 342/514 = 66.5%** (single-channel 172: capi_v0145 75, r4133 97). The
plan's ≥90% target is **arithmetically unreachable**, proven by the seeding: 97
cases are r4133-only 0.15/r4133 features 0.14.5 cannot run at all (WindGen, NCIM,
LineConstants upgrades, MonitoredVoltage InvControl, relay-0.15, batchedit-where,
MMF single-col …) → they can never be `both`; 75 are capi-only wholesale-divergent
on r4133 (reduce/makeposseq reductions, Carson geometry/cable upgrades, IEEE_519
harmonics, ckt24 conditioning …) which the plan explicitly keeps single-channel.
Even flipping every fingerprintable candidate caps ~72%. The documented follow-up
flip set (fingerprintable, cause-mapped, envelope-measurable): probe %.6g
display-precision ×~16 (storage/pvsystem), injection-fpc-delphi-ulp ×4
(indmach/combo asym), monitor-seq-magnitude-drift ×1 (`monitor_seqmag`).

**Counts (bit-stable):** solvable_now 293, asymmetric 47, controls 105, modes 69,
`.dss` bijection 915. isolate cases = 33.

**Gate (three-command, green + deterministic).** fmt clean; clippy clean; `cargo
test --workspace` green. Full BOTH gate ≈ 137–160 s at jobs=16 pool=8 recycle=1
(≪ §3.4 ≤10 min). `tests/corpus` pristine.

Wall-clock table row (plan §3.4 / §6):

| point | mode | jobs | pool | recycle | corpus gate |
|---|---|---|---|---|---|
| Phase D (full BOTH gate, ledger active) | persistent-parallel | 16 | 8/ch | 1/case | ~150 s |

**Open follow-ups (Phase D → later):** (1) retire the remaining `defer_ledger` —
DynExp×2 / idle×1 / line_spacing×1 via measured envelopes (line_spacing also needs
the exact-pair-numeric probe scope, below); NCIM×4 needs a WP-U1.7 NCIM op-point
re-validation first. (2) The fingerprintable `both` flip set (~21 cases) to push
toward the ~72% ceiling. (3) `tools/opendss/*.py` report scripts still reference
`known_diffs`/`corpus_live_opendss` → Phase E. (`M1/Master_NoPV` follow-up (3) is
now settled — see addendum.)

### Phase-D settlement (audit dispositions, 2026-07-18)

Two independent xhigh audits (audit-code + audit-tests) of `ae4b4ef..e9a2502`.
Dispositions, settled empirically (drive the live engines / read the Pascal), never
by loosening a tolerance:

- **F1 (both audits, HIGH) — element/monitor envelope only checked the SELECTED
  sub-channels, dropping the unscoped remainder from all comparison** (a matching
  `element`/`monitor` entry made the caller skip the *whole* element/monitor's
  `compare_element`/`compare_monitor`; the in-code doc falsely claimed the rest was
  "still tier-checked"). Latent (no such entries ship yet) but a real clause-(b)
  hole exactly on the brief's R3 focus. **FIXED** by generalizing the `property`
  rewrite pattern: `element_rewrites`/`monitor_rewrite` re-assert the pinned
  sub-channels inside their envelope (clause a) then rewrite ONLY those to the Rust
  values so the untouched `compare_element`/`compare_monitor` tier-checks every
  unscoped channel (clause b). `runner.rs` now always runs the harness comparator.
  False doc comments corrected.
- **F4-code (MEDIUM) — a non-numeric probe scope with no `oracle` pin
  self-certified (marked applied+exceeded, skipped all comparison).** **FIXED**:
  the non-numeric branch now requires an exact `oracle` pin (else panics — discrete
  state is exact-pair only, §1.3), asserts the live Rust value against an optional
  `rust` pin, and marks `exceeded` only when Rust ≠ oracle (so it CAN go stale).
- **F5-tests (MEDIUM) — exact-pair `property`/non-numeric-`probe` entries marked
  `exceeded` unconditionally → fail-on-stale could never fire for them.** **FIXED**:
  both now query the live Rust value and mark `exceeded` only when it still differs
  from the oracle, so a vanished discrete divergence trips STALE. (`mask_line`
  eventlog/ctrlqueue entries already self-detect via the NEVER-APPLIED path when the
  artifact line disappears; `skip` entries are `Kind::Skip`, exempt from the
  divergence-stale check by design.)
- **F2-code (HIGH-ish factual) — the `linespacing-normamps` cause, the
  `line_spacing_asym` defer_ledger note, and STATUS recorded the 730-vs-230
  direction BACKWARDS.** **SETTLED empirically**: pinned dss-python 0.14.5 oracle =
  `Line.lsp.normamps 730 / emergamps 1095` (first-wire ACSR_556 rating); the port =
  `230 / 345` (MIN over phase conductors {556→730, 4-0→340, 1-0→230}=230), matching
  r4133 V8 `LineGeometry.pas:1237-1239`. **The port's 230 is CORRECT** (WP-U1.2 D3
  min-over-phase upgrade) — a wrong-fact-in-the-ledger, not a papered-over bug. Cause
  text, manifest note, and this record corrected. The cause is currently unused (no
  entry references it), so nothing was mis-gated live.
- **F3-tests (MEDIUM) — `M1/Master_NoPV` Load `PF=1`-vs-`0.9`, flagged "possible
  PF-parse bug".** **SETTLED empirically: NOT a bug.** The ~10 diverging loads
  (`Loads_Only.dss`: `kW=0 kvar=0 pf=0.9`) hit the WP-U1.1 **L2 REPLACE_ZERO** clamp
  (`kW`/`kVA` parsed in `(-1e-8,1e-8)` → `+1e-8`, the EPRI r4133 `DblValueNZ`
  default the 0.14.5 oracle lacks; `prop_flags.rs REPLACE_ZERO`). On 0.14.5 `kW=0`
  stays 0 so `LoadSpec kW_kvar` leaves `PFNominal=0.9`; on the port `kW=1e-8` makes
  `kVA>0` so `PFNominal=kW/kVA=1`. The port correctly follows r4133 (which is why M1
  gates cleanly on r4133). The delta touches every zero-load's kw/kva/pf strings —
  **wholesale, not tightly fingerprintable** → per §4-D it stays r4133-only with a
  corrected note cause, not a ledger entry. No code change; the port is right.
- **F1-defer / F3-code (HIGH) — DONE-bar "11 ex-defer_ledger live-gated;
  defer_ledger fully retired" is NOT met; 8 remain Rust-smoke-only.** Deliberately
  **not force-retired** (rationale, per R3 "a ledger entry that papers over a fixable
  bug is the worst outcome"): NCIM×4 is a suspected op-point port bug (needs WP-U1.7)
  and DynExp×2 matches NEITHER surviving oracle — force-ledgering either would pin a
  bug; idle×1 and line_spacing_asym×1 are settled upgrades but need machinery the
  phase doesn't ship (r4133 voltage envelope resp. exact-pair-numeric probe). All 8
  keep `defer_ledger` (Rust-smoke: solve + no-new-errors) with corrected notes and
  the follow-ups above. The `defer_ledger` field is therefore RETAINED, honestly.
- **F5-code/F6-tests (LOW) — both% 66.5% < 90% target.** Disclosed above and
  data-backed (~72% seeding ceiling); the ≥90% target is arithmetically unreachable.
  No action.
- **F6-code (LOW) — `injection` envelopes the whole RHS vector (no node
  sub-selector).** Acknowledged design (injection has no natural per-node selector);
  the planned `injection-fpc-delphi-ulp` entries are whole-vector ulp floors, so
  clause (b) being vacuous is acceptable. No change; noted for the follow-up author.
- **F7-code/F4-tests (LOW) — `tools/opendss/*.py` read the deleted
  `known_diffs.json`; ~18/20 ledger causes are referenced only by prose notes.**
  The python scripts die in **Phase E** per the brief (out of scope here); the
  orphaned causes are the imported known_diffs class-prose kept as reference for the
  single-channel note cases — retained as documentation, no gating impact.

## 1a. Archived — completed plan records (100% done)

> Moved out of the active §1 frontier on 2026-07-17. These are the records of plans whose own work-package scope is closed and gate-green: the 1:1 FINAL ACCEPTANCE, JSON export (Stages A+B), DIAKOPTICS/PSTCALC **Part I**, and the full **UPGRADE** Rung 1 + Rung 2 (r4133 parity). A few carried a documented item forward to a successor plan that has **not** finished it yet (TODO(compat) sweep + HIDE_015X → DE_PASCALIZE Stage F; GICMvars export → Phase 9; JSON DynInit/Full-mode tail → a follow-up WP; IEEE118 NCIM → a future UPGRADE rung) — those open items are surfaced in §1's **Standing open follow-ups**, not buried here. Frozen history — superseded only by the code and tests. In-progress / not-started plans (DE_PASCALIZE, DIAKOPTICS Part II, RESONANCE, MULTITHREADING, WASM_USERMODELS) stay in the active §1 above.

**Late-UPGRADE work records (historical — all landed; kept for the §UPGRADE
cross-refs).**
- **D14 (DynamicExp RPN "index-bug fix") — landed, pulled ahead of WP-U1.6** (branch
  `dynexp-d14`). Upstream `2a8bdb78` adds an `Exit` to `SolveEq` that returns before
  evaluating the RHS, making it a no-op evaluator: DynExp state variables freeze at
  their `InitStateVars` seed (no rotor swing / inverter ramp). Ported 1:1
  (`dynamic_exp.rs::solve_eq`), matching capi015 to the f32 floor (probed: generator
  `speed`/`theta` frozen vs 0.14.5 swing). Unstraddled the parked
  `GFLDaily_DynExp` deck (re-promoted `oracle:capi015`); flipped `Dynamic_KundurDynExp`
  to capi015; re-pinned 7 `exec/tests/dynamics.rs` DynExp gates to the frozen values
  (now D14 regression guards). See DIVERGENCES.md §D14.
- **WP-U1.8 (WindGen + WTG3 dynamics) — LANDED** on branch `wp-u18` (new PC element +
  the general dynamics-entry Y-rebuild fix + the `micro_wtg3_dynamics` floor tier).
  See the UPGRADE record below.
- **WP-U1.7 (NCIM solver)** — **Stages 1–4 core landed** (branch `wt-u17`): Stage 1
  (`RealSparseSet`) + the full `NCIMSolutionHelper.pas` port (`ncim.rs`), PDE_ONLY Y
  build, generator PV fields, `Set Algorithm=NCIM` dispatch, and the
  `IgnoreGenQLimits`/`NCIMQGain` options. Post-audit, every electrical value in
  `exec/tests/ncim.rs` is pinned against capi015 NCIM captures (incl. the PV-bus
  path: regulating / Q-limit PV→PQ / faithful nonconvergence). **Remaining:** the
  capi015 corpus deck matrix (micro/PV/midi decks flip `pending:false`) and the
  `Export Jacobian/deltaF/deltaZ` + `Show PV2PQ_Conversions` reports. See the
  §UPGRADE WP-U1.7 record below.
- **WP-U1.2 (numeric long tail)** — rows B2/D1, D7, D6, B1, D8 landed; **B3-r3723**
  (Load.GrowthFactor Year=0) landed under WP-U1.6; **remaining: D3** (report-only
  spacing ratings — needs an overload-report deck). See the resume note under
  §UPGRADE below.
- **B5 GFM `Isc1` ×1000 — SETTLED (GFM WP, branch `gfm-wp`).** Adopted the r4133
  `Isc1` (drop the `·1000`) in `calc_gfm_yprim`. The feared "injection-vs-YPrim
  gap" does **not** exist at this base: the Rust GFM op-point is already
  Isc1-invariant (proven — the positive-sequence Norton impedance `Zs−Zm = R1+jX1`
  is `Isc1`-free; only the zero sequence moves, and delta/balanced GFM loads see
  only the positive sequence). 8 vendored + 4 controls GFM decks flipped to
  `oracle:capi015` (whole-model green); non-discharging-GFM decks stay 0.14.5-green.
  Also settled the unrelated capi015 daily `CktElement.Losses` staleness quirk (not
  reproduced; harness `loss_w` self-validating skip). See §UPGRADE + DIVERGENCES.md.
- **WP-U1.5 — LANDED complete** and **WP-U1.9 — LANDED complete** (2026-07-16
  round); **WP-U1.4 / WP-U1.6 — PARTIAL** (blockers in the Rung-1 remaining-tail
  list above). Full records in §UPGRADE below.
- Part II A-Diakoptics (AD.2–AD.5) merged 2026-07-12 — records in
  `docs/phase-records/part2-adiakoptics.md`; AD.6 (threaded children) remains
  sequenced after MULTITHREADING M2.

**SKIPPED-SWEEP (branch `skipped-sweep`, 2026-07-12).** Gave every in-scope entry
in `skipped_needs_investigation.json` a real disposition (19 entries; the 2
`upgrade_straddle_gfm_wp` + 1 `upgrade_straddle_u16_dynexp` decks were left to their
dedicated WPs). Probed each on the official EPRI engines (r3723/r4088/r4133 via the
Oddie bridge) and the pinned 0.14.5 oracle; verified Rust behaviour first-hand.
**9 promoted to `solvable_now`, 10 kept parked with refreshed evidence** (settle
2026-07-12 promoted 2 more 4wire-Delta decks — see the settle note below).

- **Root cause found for 6 "oracle_nonconvergence" decks: the deck's `maxiterations`
  cap was simply below what the (convergent) circuit needs** — not a solver defect.
  With `post: ["Set maxiterations=200"]` the pinned 0.14.5 oracle AND Rust converge
  at the **exact same** iteration count with bit-identical node voltages:
  StevensonPflow (90), StevensonPflow-3ph (106), IEEE 30 Bus/Master (19; its own
  sibling `Run_IEEE30.DSS` sets maxiterations=100), 8500-Node/Master-unbal (62),
  GFM_IEEE8500/Master (67), GFM_IEEE8500/Master-unbal (62). Promoted `kind=large`.
- **1 "user_model" deck promoted via a harness change:** `Test/indmachtest/Master.DSS`
  (Generator model=6 with an unvendored user-model DLL). The oracle server learned a
  `warn_and_continue` mode (driven by the case's existing `expect_warnings`): set
  `DSS.Error.EarlyAbort=False` + tolerate the user-model DoSimpleMsg (#567/#570/#1570)
  at compile, every solve, and the priming currents read — 1:1 with the official
  Direct DLL's warn-and-solve. Rust 8 iters == pinned 8, node V bit-identical.
- **Kept parked (refreshed):** `vsctest` + `Torn_Circuit`
  Master/Interconnected (GENUINE non-convergence on r3723/r4088/r4133 even at
  maxiterations=1000); `IEEE118Bus` (convergence is an r4088/r4133-only solver change,
  Rust mirrors r3723=NO → BLOCKED_PENDING UPGRADE); the 2 `oracle_timeout` TnD decks
  (A-Diakoptics not ported → not promotable regardless); `ieee9500_base` (pathological
  voltage-collapse deck, NO on r3723/r4088/r4133); the 2 `conditioning_floor` decks
  (`CIM/IEEE13_Assets`, `SecondaryTestCircuit_modified` — proven cross-solver floors
  re-affirmed by decomposition, no honest band fits).
- Harness change: `tools/oracle/oracle_server.py` (`warn_and_continue`,
  `_USER_MODEL_ERRNOS`, EarlyAbort toggle in `main`, priming retry in
  `capture_all_elements`) + `corpus_live.rs` (send `warn_and_continue` when
  `expect_warnings` is set). `solvable_now` 283→290, `skipped_needs_investigation`
  22→15; `population.lock` regenerated in-commit.

**SKIPPED-SWEEP settle (2026-07-12).** Audit found the two 4wire-Delta `Kersting4wire_Lagging`
/ `Kersting4wire_Leading` decks were parked on a false premise. The "Rust 3 iters vs oracle 2"
claim compared Rust COLD (the deck's internal compile-time Solve, 3) against the oracle WARM
re-solve (2). Measured symmetrically on both engines: **cold 3 / warm 2 on BOTH**, and the live
harness (compile + explicit solve) compares the WARM re-solve → 2 == 2. Warm node V is
bit-identical (Lagging SOURCEBUS.1 7200.002150, Leading 7199.557591; all 12 nodes agree to 6+
figures on both engines). Promoted both via the same `warn_and_continue` path as `indmachtest`
(`kind=feeder`, `expect_warnings=["Not Loaded"]`). `Kersting4wireIndMotor` stays parked, note
corrected: its real blocker is NOT iterations (also cold 3 / warm 2) but a genuine 4th-wire/
neutral-node divergence — its `LineCode.556MCM` declares `nphases=4` yet supplies only a 3×3
(6-entry) cmatrix; safe Rust rejects the malformed 4-phase Cmatrix while the oracle tolerates
it, so PRIMARY.4 diverges 13.363452 vs 13.363170 (|diff| 2.82e-4), above the feeder floor. The
`warn_and_continue`→`expect_warnings` coupling remains non-masking (all 5 opted-in decks are
genuine user-model DoSimpleMsg cases; Rust independently enforces convergence + exact iterations
+ bit-identical V). `solvable_now` 290→292, `skipped_needs_investigation` 15→13; `population.lock`
regenerated in-commit.

> Working cadence and the standing toolchain note are just below; the full
> per-step ritual is `PLAN_SEQUENCE.md` / the active plan's §0.

---

### Condensed era summaries (full records in `docs/phase-records/`)

**FINAL ACCEPTANCE — ACCEPT (2026-07-11).** Max-effort referee round on
`final-acceptance` returned `criteria_met=true`, `blocking_items=[]` (B1 CIM
cross-class panic, B2 8500 save-floor, B3 anti-shrink guard all resolved with
re-verified evidence). §6 criteria met: `corpus_live` live-compares every
`solvable_now` deck (full Y/V/I/P + injection + discrete state per step) at the
calibrated floors; `save_roundtrip` 6/6 (IEEE 13/34/37/123/8500 + structural);
`golden_reports` 193/0; gate green (pinned dss-python 0.15.7 / dss_capi 0.14.5).
Gated population at acceptance **245/335 solvable_now (73.1%)**; the unported tail
is exclusively Phase-9 actor/parallel mode + `CapControl.ControlSignal`, explicitly
outside 1:1 acceptance. **Named non-blocking residuals** (all documented): the
RegControl/LDC `SubXFMR` family, `LVTestCase` entry-0 unenergized, UTF-8 BOM,
`CapControl.ControlSignal`, SecondaryTestCircuit_modified, GFM_IEEE8500 near-floor.
Several were later resolved (see corpus summary). Detail + the §6 criterion→evidence
table + FA settle + FA fix 1/2/3: **`docs/phase-records/final-acceptance.md`**.

**Corpus completeness.** Post-acceptance CF / CF2 / coverage-wave rounds drove
`solvable_now` to **~286/329 entry points (86.9%)**; synthetic families reorganized
into per-element subfolders and grown to **asymmetric 47 / controls 92 / modes 49**
live decks. Full blow-by-blow: **`docs/phase-records/corpus-rounds.md`**.
- Fixed real port bugs (one line each):
  - LongLineCorrection stored-but-never-applied → ported `TLineObj.DoLongLine` (asymmetric wave).
  - DIRECT-mode PCElement `GetCurrents` `LastSolutionWasDirect` shortcut → ported (FIX-DIRECT).
  - Dynamics-mode `ShapeFactor` ignored `ActiveLoadShapeClass` (PVSystem + the VSource/Isource/IndMach012 siblings) → fixed (CF2-G).
  - Element `base_frequency` hardcoded 60 instead of circuit fundamental → fixed (CF-A); resolves `LVTestCase`.
  - UTF-8 BOM not stripped on redirect; undefined-monitor export hard-error; bare-quote inline comment → fixed (CF-A).
  - CapControl `Type=follow` self-monitor dispatch aborted with "Monitored element not set" → fixed (CF-C).
- Proven floors (NOT bugs, reproduced/documented, never band-fudged):
  - `SubXFMR` family = ultra-switch conditioning (1e-8 Ω switch, Y≈1e8 S) + Carson libm floor (CF-D) — the acceptance "RegControl/LDC" label was disproven.
  - #485 control-settling family = hunting-truncation reproduced 1:1; the "divergence" was a harness second-solve measurement artifact (CF2-R).
  - DOCTechNote ×4 + GFMSnap = floating-zeroseq common-mode floor (FA fix 2).
  - SecondaryTestCircuit_modified / IEEE13_Assets = conditioning floors (documented, too wide to band).
  - GFM_IEEE8500 Snap/Daily = near-floor faer-vs-KLU.

**JSON export (A+B) — COMPLETE.** `Obj_ToJSON` / `Batch_ToJSON` (Stage A) +
`Obj_Circuit_ToJSON_` (Stage B), ~130-byte goldens. Detail in
`docs/phase-records/gaps.md`.

**Part I DIAKOPTICS/PSTCALC — COMPLETE; Part II sequenced later.** WP-PF.1
(`Pstcalc` command), WP-PF.2 (Monitor mode-4 flicker), WP-AD.1 (incidence matrix +
`Sparse_Math` + exports 53–57) all gate-green. Part II (A-Diakoptics WP-AD.2–AD.6)
is deliberately outside final acceptance, sequenced after MULTITHREADING M2. Detail:
**`docs/phase-records/part2-adiakoptics.md`**.

**UPGRADE Rung 1 (U0 / U1.1 / U1.2).** Multi-oracle test infra (WP-U0: per-case
`oracle` manifest field routing to capi015 / r3723 / r4088 / r4133; `capi015`
engine; `Rust ≤ oracle` iteration policy for target-rev cases; r4133 pilot in every
`cargo test`). **WP-U0.2** = report-only `ab_compare` inventory sweeps over 378
cases/pair across the three rung pairs + reconciliation of the `delta_*.md` ledgers.
**WP-U1.1 (parser/property semantics) — items 1–5 all landed:** L2 `DblValueNZ`
zero-`kW`/`kVA` clamp, `ParseAsSymMatrix` incomplete-matrix reject, `AllowNoneItem`
(`none` in conductor lists), `TCC_Curve.none`, C11 class-command activation.
**WP-U1.2 (numeric long tail) — landed rows:** B2/D1 (SimpleCarson De
`658.5→658.8530451057239`), D7 (PVSystem dynamics current-limit base
`PanelkW→FkVArating`), D6 (Transformer seasonal AmpRatings drop `1.1×`), B1
(Capacitor Cmatrix YPrim diagonal `×1.000001`), D8 (settled, no code change — not a
0.14.5→0.15.x delta). **WP-U1.3 (InvControl cluster) — all 6 rows settled:** D1/ledger-L1
InvControlDeltaV per-control 2-slot buffer (adopt capi015 fix; the r4133 `i=1`
cursor gating cataloged as a known upstream bug), D2 per-DER basekV, D3
sqrt-guard (EPSILON=1e-12), D4 delta-DER LL monitored voltage (sign-flipping,
capi015==r4133; unit + capi015 deck pinned), D5 no-delta, C8 (a)
`VV_RefReactivePower` removal NOT adopted (r4133 keeps it) + (b) MonBus
#2024111/#2024112 validations. Known limit: the capi015 oracle cannot gate
multi-step decks (per-step capture re-nominalizes shapes) — capi015 corpus cases
are snapshots; follow-up logged for the oracle-infra owner. Full detail:
**`docs/phase-records/upgrade-rung1.md`**.

**WP-U1.5 (EnergyMeter seasonal / allocation / monitor-header) — LANDED (branch
`wt-u15`).** Five spec rows settled (detail in `docs/upgrade/DIVERGENCES.md`):
- **E2 / ledger L4 (SeasonalRating reimplementation) — adopted capi015 = r4133.**
  New global `Circuit::seasonal_rating_idx` synced by
  `solution::meters::sync_seasonal_rating_idx` at every solve (Pascal
  `SyncSeasonalRatingIdx`, `55400a29`); new `CktElement::get_ratings(idx)` trait
  method (`TPDElement.GetRatings`) overridden by Line + Transformer `num_amp_ratings`/
  `amp_ratings`; wired into `export_capacity`/`export_overloads`/`write_overload_report`
  so the seasonal `AmpRatings[idx]` applies to ANY PDElement (0.14.5 restricted
  `DI_Overloads` to lines and never applied it in `Export Overloads`). The
  0.14.5 state-mutating `SeasonalRating := FALSE`-on-miss read is not reproduced
  (precomputed index removes it). Gate: 2 new **capi015** report goldens
  (`export_{overloads,capacity}_seasonal`, `.meta.json` `oracle:capi015`,
  regenerated via the new `DSS_ORACLE_ENGINE=capi015` branch in `gen_reports.py`;
  deck = overhead Line + Transformer + CN cable, all `Seasons=4`, validated
  bit-identical capi015 == oddie:r4133 §1.7) + feature-sensitive `get_ratings`
  unit tests (Line + Transformer). Probe: 0.14.5 reports base `%Normal=134.5`,
  capi015/r4133 the seasonal `336.3` — revision-sensitive.
- **D9 (AllocateLoad/CalcAllocationFactors skip disabled meters/sensors) — adopted
  capi015 (r4115 `fb728364`).** Two `if !enabled` guards in
  `sampling/allocate.rs`. 0.14.5 hit an Access Violation walking a disabled
  meter (UB, not reproduced). Gate: `allocateloads_ignores_disabled_meter`
  (feature-sensitive — meter enabled at zone-build then disabled; factors stay 0.5
  vs 6.3725 enabled).
- **D8-r3723 (manual-ZoneList child from-bus/terminal) — adopted r4133.**
  `zones/build.rs`: `add_new_child(terminals[0].bus_ref, 1)` (was `(NO_BUS, 0)` =
  0.14.5). IS a code delta (verify verdict), but its effect is **masked** in our
  path (from-bus already volt-base-listed; DistFromMeter not propagated for manual
  zones); 0.14.5 AVs so no oracle golden — the memory-safe
  `energymeter_manual_zonelist` test guards it.
- **D16 (zone counter skips disabled + non-PD) — NOT a delta for us.** Already in
  the 0.14.5 baseline and already ported (`zones/build.rs` `if !enabled ||
  !is_pd_element`); 0.15.x only flattened the guard. No code change.
- **E1 / ledger L3 (Monitor header) — keep the dss_capi form.** The quote-removal
  + `MonitorHeader` flag are CSV-render only (flag off by default); probed
  capi015 `Monitors.Header` tokens == 0.14.5 == Rust. No code change; the
  `monitor-header-whitespace` known_diff (KEPT vs EPRI) stays.
- known_diffs burn-down: no seasonal/allocation entry ever existed (reports were
  NOT_PORTED); `meter-zonepce-count` (r3723-only) + `monitor-header-whitespace`
  both document behaviors this WP does not change → retained.
- **Audit fixes (branch `wt-u15`).** (1) `get_ratings` blocker REBUTTED: the
  auditor cited r4133/pre-refactor `NumAmpRatings > 1`, but the port ports
  `55400a29`'s `GetRatings` guard `0 <= idx < NumAmpRatings` (no `>1`); the pinned
  capi015 oracle (0.15.0b4/SVN4103) confirms it — a single-season Line at idx 0
  reports `%Normal == %Emergency` (AmpRatings[0] overrides both). No code change;
  docs corrected to stop misquoting the guard. (2) Set-command sync (real
  divergence): `Set Hour`/`SeasonRating`/`SeasonSignal` now re-sync
  `seasonal_rating_idx` (`55400a29` ExecOptions 3/114/115), verified on capi015
  (`solve; set hour; export` reads the new index) —
  `set_commands_resync_seasonal_rating_idx`. (3) Seasonal report goldens tightened
  to exact `0.0/0.0` (Rust == capi015 byte-for-byte; the faer-vs-KLU floor was
  unnecessary). (4) Added `di_overloads_applies_seasonal_rating` (DI-path seasonal
  wiring).

**WP-U1.9 (PCE force hooks) — LANDED (branch `wt-u19`).** Ported the dss_capi
0.15.0b4 pyControl engine hooks — spec read via `git show 0.15.0b4:` because the
vendored working tree `.inputs/dss_capi_with_git` sits at a later `master`
(`f5728aec`) where these options were **removed** upstream; the capi015 oracle
(tag `e936d210`) still carries them, so it remains the authoritative spec.
Landed: `Set`/`Get` `InjCurrent`/`ITerminal`/`YPrim`/`StateVar`/`IterNumber`/
`CtrlIterNumber`/`IntegrationFlag` + `Flg.ForceInjCurrents`/`ForceYPrim`, honored
in the injection loop (`solution/solution/power_flow.rs` injects the stored
`InjCurrent` directly, per `TPCElement.InjCurrents`) and `ReCalcAllYPrims`
(`solution/ymatrix.rs` skips `CalcYPrim` when `ForceYPrim`); the five PCE
`GetTerminalCurrents` (Load/Generator/PVsystem/Storage/IndMach012) skip the model
recompute when forced. `Set IterNumber`/`CtrlIterNumber`/`IntegrationFlag` are
read-only; `Set PyPath=` + the pyControl component stay NOT_PORTED (loud, §0).
`SampleControlDevices` was already ported (present in 0.14.5, `Solution.pas:1974`
→ `solution/controls/sampling.rs`) — NOT a delta; `delta_capi_0145_015x.md` A3
over-claimed it as new. Parser gained `make_complex`/`parse_as_complex_vector`/
`parse_as_complex_matrix` (`ParserDel.pas`, `(f64,f64)`-tuple, dep-free). Gated by
the live `modes/upgrade_forcehooks.dss` (oracle:capi015, validated bit-identical
across two capi015 processes; `Set InjCurrent=[80 0 80 0 80 0]` moves b2 Vmag
7187.45→7224.14 V) + the capi015-pinned unit suite `exec/tests/force_hooks.rs`
(forced Vmag 7224.143523 @1e-6, frozen `Get InjCurrent`/`ITerminal`, read-only
sets, `Set YPrim` survives a rebuild, `Set/Get StateVar`, and **`Clear` resets
the force flags**). Ledger: `docs/upgrade/DIVERGENCES.md §A3/A5`.

**WP-U1.9 audit follow-up — LANDED (branch `wt-u19`).** Addressed 7 audit
findings against the capi015 oracle. Fixed: `Set/Get AllowForms`/
`AllowProgressBar` now accepted headless no-ops (round-trip, default `No`) —
were erroring "not ported"; `Set/Get StateVar` non-PCE now gives the Pascal 7103
"is not a valid PC element" (guard runs before the 7101 NumVariables check); the
force-hook error arms now `Exit` (break) the option loop like Pascal. Corrected
the false "Set/Get StateVar covered" claim: `Set StateVar` via text is
**upstream-broken** (positional `DoSetCmd` parse never reaches the arm →
capi015 `#303`, reproduced as error + no write); `Get StateVar` is the
functional read path. Grew the unit suite to 12 tests (added: `Set ITerminal`
freeze, Generator 2nd-PCE force-skip, oversize-row `#3004`, natural-syntax `Set
StateVar` error, both 7103 guards, AllowForms round-trip). The `#3004` YPrim
error zeroes capi015's live matrix (its own known error-state imperfection) —
**not reproduced** (scratch-buffer parse leaves the real YPrim intact; transient,
next `ReCalcAllYPrims` recomputes). Ledger updated in DIVERGENCES §A3/A5.

**WP-U1.8 (WindGen + WTG3 dynamics) — LANDED (branch `wp-u18`).** New PC element
`elements/pc/windgen/` (Generator-shaped negative load): aerodynamic power-flow
(`Pm=0.5·ρ·π·Rad²·v³·Cp`, the load shape supplies WIND SPEED not a pu multiplier;
kWBase curtailment + cut-in/cut-out; 4 models 1/2/4/5) + the embedded GE WTG type-3
dynamics (`wtg3.rs`, 1:1 of `WTG3_Model.pas`: PLL, seq-current PI regulators,
LVPL/LVQL ride-through, Cp 5×5 aero, MPPT/torque/pitch/inertia, one-mass swing, the
**odd-substep 50 µs trapezoidal sub-cycle**, all 22 state vars). Harmonics DISABLED
upstream → reproduced as a loud abort. `windgen_model`/`windgen_qmode` enums,
`ElemKind::WindGen`, PASCAL_CLASS_ORDER slot after Generator. 5 live `modes/windgen/`
capi015 decks (snap wye/delta, daily single-step, dynamics, dynamics+fault) + 11
unit tests. Two cross-cutting findings: (1) a **general dynamics-entry Y-rebuild
fix** — `calc_initial_machine_states` now raises `system_y_changed` (Pascal
`InitStateVars`→`SetYprimInvalid`→`SystemYChanged`, lost in the port), without which
the WTG3 Norton injection ran the terminal voltage away; (2) the new
`micro_wtg3_dynamics` tolerance tier (decomposition-proven: snapshot input matches
1e-8, the PLL derivative `×60000` amplifies the near-cancellation `Vq` into a ~1e-5
state / ~1e-6-rel terminal-V floor that DECAYS as the transient settles — a WPG.13
amplification floor, NOT a bug). WindGen energy-meter registers not ported
(`EnergyMeter.SampleAll` never samples WindGenClass — unreachable). Daily deck is
single-step: dss_capi 0.15.x caches per-element `Losses` and WindGen doesn't
invalidate it (bucket-F API quirk, out of scope).
- **WP-U1.8 settle (audit).** (1) The `micro_wtg3_dynamics` tier is empirically
  confirmed a genuine cancellation floor, not a masked state-leak: a per-variable +
  per-node decomposition vs capi015 (throwaway probe, reverted) shows the gap is
  confined to exactly the 3 PLL-derivative-fed vars (`dOmg`/`Pgen`/`Qgen`, ~1e-5
  healthy / ~4e-5 fault) while 14 of 22 vars are bit-exact and the other 5 are ≤5e-7;
  the worst node-V is always WBUS (the terminal bus) at 9.6e-7 rel healthy / 3.4e-6
  fault, so the default `v_rel=1e-7` genuinely fails and `8e-6` covers it at ×2.3
  (not over-loose). (2) Fixed a robustness defect: a 1-phase WindGen entering
  dynamics used to **panic** (OOB in `wtg3` `instrumentation`, which reads V[1..3] —
  the WTG3 model is 3-phase-only; upstream over-reads = heap UB, NOT reproduced). Now
  a loud clean abort — `init_state_vars` aborts non-3φ before the model init, and
  `do_dynamic_mode` guards the per-step path (external `solve` clears the init abort);
  `calc_initial_machine_states` now drains+propagates element init aborts to
  `solution_abort` (also surfaces the latent >3φ silent-garbage path for all
  machines). New unit test `single_phase_dynamics_aborts_cleanly`. (3) The 5
  `windgen/*` decks joined the `MODES_REQUIRED` anti-deletion floor.
- **Resume note (WP-U1.2 remaining).** Row **D3** (report-only spacing ratings —
  overload-report deck) still to port. **B3-r3723** (Load.GrowthFactor Year=0)
  LANDED under WP-U1.6 (branch wt-u16). The golden engine switch
  (`gen_checkpoints::check_pin` `DSS_ORACLE_ENGINE`) and the
  same-commit flip/regen/retire workflow are proven. NB the modes manifest is NOT
  `json.dumps`-round-trippable (mixed manual `\uXXXX` escaping + CRLF) — append new
  cases with a surgical text edit.

**WP-U1.6 (controls & misc long tail) — PARTIAL, LANDED (branch wt-u16).**
Landed rows (each unit/deck-pinned, gate-green):
- **B3-r3723** (own commit) — `Load.GrowthFactor` Year=0 with a GrowthShape now
  tracks the simulated hours (`calcYear=dblHour/8760`; `GetMult(Ceil)` or
  `GetMultIdx(1)` when firstY=0 & <1yr) instead of a flat 1.0. Added GrowthShape
  `get_year`/`get_mult_idx`. **Oracle-validated + deck-gated** (audit-U1.6
  settlement): `git show 0.15.0b4:src/PCElements/Load.pas` carries the rewrite
  verbatim (the working-tree checkout f5728aec predates it, see DIVERGENCES.md
  version note), and capi015 probes confirm 120 kW (factor 1.2) vs 0.14.5's flat
  100 kW. New corpus deck `modes/upgrade/upgrade_growth_year0.dss` (`oracle:
  "capi015"`, snapshot, feature-sensitive 120-vs-100 kW; §1.7 two-process
  determinism confirmed). Unit tests
  `growth_factor_year0_tracks_simulated_hours_with_growthshape` (probe-cited) +
  `get_year_and_mult_idx_are_one_based`.
- **D10** StorageController — (a) `a14c3f1f` FpctkWBandLow typo fix (was reproduced
  as `TODO(compat)`; adopted, `FpctkWBandLow := FkWBandLow/FkWTargetLow*100`);
  (b) `1b3123ce` force a new power flow on control iter 1 when peakshave(-low)
  moves the fleet into (dis)charge even when the condition matched last step
  (added `control_iteration()` to `StorageDispatchEnv`). Both in capi015 0.15.0b4
  (= r4103; `StorageController.pas:547`). **Oracle-validated** (audit-U1.6):
  capi015 `%kWBand=16.667`/`%kWBandLow=20` vs 0.14.5 typo `6.667`/`2`. Unit tests
  `kw_band_low_side_effect_syncs_the_low_pct_pair` (property sync, added under
  audit) + `d10_discharge_transition_forces_resolve_on_first_iteration`
  (force-resolve). Unit-pinned (no single-step corpus witness: property-only sync +
  multi-step force-resolve; precedent B1/D6/D7).
- **D15** (`4366b126`) — `LookupVariable` case-insensitivity: the only
  equivalent in the port (relay) already uses `eq_ignore_ascii_case` (= the fixed
  side); not-a-delta, upper-case query pinned in `lookup_variable_prefix_match`.
- **A7-r3723** — GenController deregistration: the r3723 port never registered the
  class, so `New GenController.…` already errors "not found"; not-a-delta, pinned
  by `gen_controller_class_is_not_registered`.

Not landed (documented for a follow-up — each needs oracle-validated decks and/or a
property-count-comparison flip beyond this pass's safe budget):
- **C5** RegControl `FwdThreshold` (`8a898cba`, SVN r4086) — the flagship: adds 4
  new props (`Idle`/`IdleReverse`/`IdleForward` [new in 0.15.x, absent from the
  r3723 port] + `FwdThreshold`) → RegControl property-count change (entangles the
  RegControl deck property-count compare, C8-class), PLUS the signed
  `RevPowerThreshold`/`FwdPowerThreshold` rework (defaults −100kW/+100kW, EndEdit
  legacy fallback `Fwd:=abs(Rev); Rev:=−Fwd`), the idle-zone `SetPointCalc` logic,
  and the reverse-power detection rewrite. Precise hunks in
  `.inputs/dss_capi_with_git` commit `8a898cba`. Needs the RegControl reverse-power
  deck matrix (legacy-input equivalence + new-property divergence) on capi015.
- **C6** Transformer/AutoTrans `BHpoints`/`BHcurrent`/`BHflux` (`90962ae8`) — 3
  new `Unused` data props on BOTH classes → property-count change entangles every
  default-oracle transformer/autotrans deck (C8-class); needs a coordinated flip.
- **C5-r3723** LoadShape `Mode` prop (22) + `Interpolation` shift 22→23 — new
  prop → property-count change (LoadShape decks) + property-index parity; same
  entanglement class as C6.
- **D12** SwtControl `Normal`/`State` field mapping (`bb9c9785`) — **RE-LANDED**
  (branch wt-u16ind); see the "WP-U1.6 tail" block below. The revert's multi-step
  entanglement was resolved by flipping `swtcontrol_time`/`midi_swtcontrol`/
  `civanlar` to capi015 (multi-step capi015 proven viable) and leaving the
  unaffected `swtcontrol_lock` on 0.14.5.
- **D11** part 2 (`b9bc87b8`: TIMECONTROL requires + uses a monitored element) —
  **LANDED** (branch wt-u16ind); see the "WP-U1.6 tail" block below. Part 1
  (PT/CTPhase validation scope) was already aligned.
- **D13** LoadShape MMF fixes (`c4590d16`) — **LANDED as not-a-delta** (branch
  wt-u16ind); the eager MMF reader already matches the fix, gated by a fresh
  single-column capi015 deck + unit. See the "WP-U1.6 tail" block below.
- **B4-capi** harmonics init-failure abort (`6ad39597`) — the port's
  `solve_harmonic_t_body` ALREADY returns on `!initialize_for_harmonics` (aborts
  the sweep); the `In_ReDirect → Redirect_Abort` nuance is unreachable (no ported
  `init_harmonics` sets `solution_abort`; see `harmonics.rs` doc). Faithful as-is;
  no feature-sensitive deck possible.
- **C4** `SolveAll` (`cmd::SOLVE_ALL`=123) — **LANDED** (branch wt-u16ind); see the
  "WP-U1.6 tail" block below.

**WP-U1.4 (line/cable-constants cluster) — PARTIAL: equivalent-spacing model LANDED
(branch wt-u14).** Ported the **B3/C1 equivalent-spacing model** (dss_capi 0.15.x
`LineSpacing.pas`/`LineConstants.pas`, SVN r3913-era) end-to-end, gate-safe (defaults
preserve 0.14.5 numerics):
- `LineSpacing` gains `Detailed` (bool, default `true`) + `EqDistPhPh`/`EqDistPhN`/
  `AvgPhaseHeight`/`AvgNeutralHeight` (double, default 0) + `EquivalentSpacing() =
  !detailed` + the `Detailed` prop-tracking side effect.
- `LineConstants` engine gains `equivalent_spacing`/`eps_r_medium` (1.0)/`height_offset`
  (0)/`user_height_unit` + the four equivalent distances; `calc_overhead`/`get_ze`/
  `cisp_overhead` branch on equivalent spacing 1:1 with the Pascal. `EpsRMedium`
  (`pfactor /= E0*eps_r_medium`, `E0*1.0==E0`) and `HeightOffset` engine numerics are
  ported default-off (the Line-level *properties* that drive them are deferred, below).
- `LineGeometry` copies the spacing's equivalent state (`apply_spacing`/
  `load_spacing_and_wires`) and threads it into `UpdateLineGeometryData`
  (distances × `To_Meters(FLastUnit)`; skips `SetX`/`SetY`).
- **Validation:** unit `line_geometry::tests::matrices_equivalent_spacing_match_capi015`
  (reduced 3×3 Z = capi015 to 1e-8), new capi015 corpus deck
  `modes/upgrade/upgrade_linecs_eqspacing.dss` (live YPrim compare green; §1.7
  two-process bit-identical), new capi015 props golden
  `props/linespacing_eqspacing.json`. No existing golden/live case moves
  (`Detailed` default true; 0 corpus decks set the props). DIVERGENCES.md §B3/C1.
- **Audit follow-up (both minor findings fixed):** the always-on unit test now also
  pins the equivalent-spacing **Yc/capacitance** branch (reduced 3×3 C = capi015
  `? line.l1.cmatrix` 16.16 / -4.087 nF/mi to 1e-8), so the shunt branch no longer
  relies solely on the live YPrim compare; and the `Set_FUserHeightUnit` meters-value
  re-conversion quirk in `support/line_constants` now carries a greppable
  `TODO(compat)` marker (dead scaffolding today — height_offset is always 0 and the
  Line-level HeightUnit prop is deferred; golden pins it when that slice lands).
- **Remaining WP-U1.4 rows (documented, not landed):** `Line.EpsRMedium`/
  `HeightOffset`/`HeightUnit`/`Conductors` **Line-level properties** — BLOCKED on the
  `compare_all_properties` count-equality harness (adding a property to the
  circuit-element class `Line` breaks every default-oracle feeder's property-table-
  shape assert; needs a harness accommodation for 0.15.x-only trailing props, a
  design decision — the engine numerics are already in place); the merged
  `CNTSLineConstants` mixed-conductor class + `CNData.SemiconLayer` capacitance
  (an engine architectural refactor: 0.15.x moves the CN/TS choice per-conductor via
  `SetCondType(i, CN|TS)` — the port still has per-*engine* CN/TS kinds); `LineCode`
  FaultRate/PctPerm/Repair deprecation (catalog, adds Deprecated/Unused flags);
  LineType enum width; and **WP-U1.2 D3** spacing ratings + overload deck.

**GFM WP (branch `gfm-wp`) — B5 + injection-vs-YPrim + 0.15.x YPrim delta —
SETTLED.** Adopted B5 (`calc_gfm_yprim` `Isc1` drops the `·1000`, dss_capi
`de6a5a42` = SVN r3865). The deliverable-3 "0.15.x GFM Storage YPrim delta" is the
SAME one-line change (Storage/PVSystem share `CalcGFMYprim`; the Storage
`CalcYPrimMatrix` GFM branch is otherwise byte-identical across 0.14.5/0.15.x). The
deliverable-1 "pre-existing injection-vs-YPrim gap" was **disproven at this base** —
the Rust GFM op-point is already Isc1-invariant (positive-seq Norton impedance
`Zs−Zm = R1+jX1` is Isc1-free; only the zero seq moves; delta/balanced GFM loads see
only the positive seq). Empirically: `gfm_micro` `Load.isl`/`islbus` bit-identical
under old vs new `Isc1`, both equal to the bit-identical 0.14.5/capi015 value; the
storage YPrim moves to the capi015 live-probe value. **8 vendored GFM decks flipped
to `oracle:capi015`** (2 re-promoted from `skipped_needs_investigation` +
`CannotPickUpLoad` + 5 Microgrid GFMSnap/SwapRef/8500-GFMSnap whose gated state ends
discharging-GFM) **+ 4 `controls/gfm` decks** — all whole-model live green vs
capi015 (system Y + V/I/P per step). Non-discharging-GFM decks (Microgrid GFMDaily/
Snap-A/B/WholeDaily, 8500 Daily/Unbal) stay 0.14.5-green (their YPrim never reaches
`CalcGFMYprim`; confirmed by per-deck capi015-vs-0.14.5 assembled-Y diff). Also
settled the **unrelated capi015 daily `CktElement.Losses` staleness** (Losses freezes
at step 0 while Powers scale; general 0.15.x quirk, NOT reproduced — Rust matches
0.14.5/r4133 fresh losses; `harness::compare_element` now self-validating-skips the
redundant `loss_w` channel when the oracle's own Losses ≠ Σ its own Powers). New unit
tests: `gfm_calc_yprim_matches_capi015_isc1_no_1000`,
`gfm_norton_positive_seq_admittance_is_isc1_invariant`,
`storage_gfm_micro_op_point_isc1_invariant`. Detail: DIVERGENCES.md §B5 +
§capi015-daily-losses.

**WP-U1.7 (NCIM solver) — Stage 1 landed; Stages 2–4 handed off.** Spec = A1,
`Common/NCIMSolutionHelper.pas` (1048, FPC). **Stage 1 (done, own commit,
gate-green):** the `dss-sparse` **real-valued** KLU-shaped path
(`crates/dss-sparse/src/real.rs`, `RealSparseSet`) that the NCIM Jacobian needs —
Pascal `NewSparseSet` + `SetOptions(…MatrixFormat_DoublePrecisionReal)` +
`SetMatrixElement`/`SolveSparseSet`. Mirrors the complex `SparseSet` (triplet
accumulate in insertion order = CSparse `cs_dupl`; KLU `scale=2` row
equilibration) but over `f64`. **`set_element` ACCUMULATES** (not replace): NCIM
stamps each non-swing diagonal 2×2 block from the PDE-only `Y_ii` (`[B,G;G,−B]`)
in `NCIM_BuildJacobian`, then adds the load/gen injection derivative onto the same
cells in `NCIM_ApplyCurr`; the current-injection Newton diagonal is
`Y_ii_block + g'_ii_block`, so the two stamps must sum (under replace a PQ node
loses its network coupling → wrong Jacobian). 9 unit tests from hand Jacobians
(2×2, a 4×4 two-block CI-shaped Jacobian, insertion-order sum, singular, bad
scaling, zero/rebuild, dim-mismatch, **zero-stamp-dropped**). The accumulate
semantics are proven from the NCIM algorithm AND corroborated by the vendored
EPRI KLUSolve C++ (`VersionC/klusolve/KLUSolve/Source/KLUSystem.cpp`:
`SetMatrixElement`→`AddElement` appends, `GetElement` sums duplicates); the
DSS-Extensions KLUSolveX *fork* (the real `DoublePrecisionReal` format) is not
vendored but inherits the CSparse pipeline. **Settle fix:** `set_element` now
drops a zero value (`if value == 0.0 return`), matching `AddElement`
(`KLUSystem.cpp:442-444`) — an earlier doc comment claimed the no-op but the code
did not implement it, so an exact-zero cell (pure-R load `B`-diagonal, pure-R/-X
branch off-diagonal) would have inflated `nnz`/`Export Jacobian` vs the capi015
oracle in Stage 3; a covering test (`zero_stamp_is_dropped`) was added.
- **Stages 2–4 remaining (integration map for the next executor):**
  - **State** (`solution/solution/state.rs`): add `NCIMSOLVE=2` + the ~15 NCIM
    fields (Solution.pas l.243-271). Node i (1-based, ground=0) → Jacobian
    0-based rows `2*(i-1)`, `2*(i-1)+1`; swing = nodes 1..3 → rows 0..5 (the
    `<6` guards).
  - **Y build PDE_ONLY** (`solution/ymatrix.rs`): add `BuildOption::PdeOnly` —
    stamps **ALL_YPRIM** for PD **or SOURCE** (VSource) elements into the series
    handle; PC elements excluded (YMatrix.pas l.442-497). NCIM reads it back via
    the triplet dump (`coo_entries`) into `ncim_y/row/col`.
  - **NCIM helper** (new `solution/solution/ncim.rs`): port
    `NCIMSolutionHelper.pas` loop-for-loop — `NCIM_GetPowers` (Load ConstZ→ZBus
    else PQ; Gen model 3=PV/4=PQ/else Z), `NCIM_Do{PV,PQ,Z}Bus`,
    `NCIM_CalcInjCurr` (`I=Y·V`, first 6 deltaF=0), `NCIM_BuildJacobian` (fresh
    `RealSparseSet` each iter), `NCIM_GetNumGenerators`, `NCIM_UpdateGenQ`
    (PV↔PQ switching + Q-limits), `NCIM_Init`, `DoNCIMSolution` (repeat:
    CalcInjCurr→BuildJacobian→GetPowers→ApplyCurr→solve→`NodeV -= dV`→Converged→
    UpdateGenQ), `NCIM_Converged` (`max|deltaF| <= ConvergenceTolerance`).
  - **Generator** (`elements/pc/generator/`): `GenVars.delta_q_nom: Vec<f64>`,
    `vtarget`, `ncim_idx`, `NCIM_InitPVBusJac`, a `NCIM_ExPV` flag; GenModel 3
    (PV) / 4 (PQ) semantics + kvarMax/kvarMin. **VSource** `CalcInjCurrAtBus`.
  - **Dispatch**: `do_pflow_solution` match gains `NCIMSOLVE => do_ncim_solution`
    (Solution.pas l.1031-1037); `converged()` gains the NCIM branch (l.730-733);
    `check_controls` resets `ncim_ready=false` + early-returns when
    `system_y_changed && algorithm==NCIM` (l.1182-1186).
  - **Options** (`exec/set_cmd.rs`): add `NCIM` to `solve_alg` at ordinal 2
    (prefix `nc`); new `IgnoreGenQLimits`→`ncim_ignore_q_limit`,
    `NCIMQGain`→`ncim_gen_gain` (ExecOptions.pas l.794-797) + `Get` readback.
  - **Reports**: `Export Jacobian/deltaF/deltaZ`, `Show PV2PQ_Conversions`
    (numeric-token gates).
  - **Decks** (`tests/corpus/modes/ncim/`, all `oracle:"capi015"`,
    `pending:true` until the WP flips): micro PQ-only snapshot; PV-bus generator
    deck (Q-limit hit → PV→PQ via `Show PV2PQ_Conversions` token + iter ≤); midi
    IEEE123-class re-solve. Cross-check one on `oddie:r4088`. Iteration policy:
    Rust ≤ oracle (§1.3-1); first-divergence trajectory dump on any gap.

**WP-COV-PARKED (Refine_BusLevels parked-test closure) — LANDED (branch
wt-coverage).** Root-caused and closed the `#[ignore]`d
`circuit::coverage::tests::refine_bus_levels_reports_paths_on_radial` "infinite
loop": a **test-harness bug**, not a port bug. The test fed its whole 8-line
deck to ONE `Dss::command` call (`command` = Pascal `ProcessCommand`, one
command line) — the lines/loads after `new circuit.covtest …` were consumed as
extra parameters of the `new circuit` command (last `bus1=b5` re-based the
Vsource; no lines existed), so `CalcIncMatrix_O` yielded a 1-bus incidence
matrix (`cols=["b5"]`, `levels=[0]`) whose coverage plateau is 0 — on such a
degenerate input the state machine's sole exit (Circuit.pas:909) genuinely
never fires, faithfully to upstream. WP-AD.5's `Get_paths_4_Coverage` /
`get_longest_path` / `Normalize_graph` and WP-AD.1's `Calc_Inc_Matrix_Org` are
verified correct line-by-line vs r3723 Delphi AND empirically: fed line-by-line
the port bit-matches the official r3723 engine (Oddie probes 2026-07-16,
both < 50 µs wall): cov=0.5 → "0 new paths detected"/`Actual_Coverage
0.666666666666667`; default 0.9 → "2 new paths detected"/`1`. Tests un-ignored
+ a new default-coverage test, both pinning result strings, `ad.actual_coverage`
values, and the `get coverage` formatted strings to the official engine. Also
disproved the old "0.9 unreachable, plateau 5/6" hypothesis (`Buses_Covered`
are index spans; the sum overshoots `Sys_Size`) and corrected the
NOTE(upstream-quirk) at `get_paths_4_coverage` accordingly. No engine code
changed; no new decks.

**WP-U1.7 (NCIM solver) Stages 2–4 core — LANDED (branch `wt-u17`).** Spec = A1,
`Common/NCIMSolutionHelper.pas` (1047, FPC) + the Solution/YMatrix/Generator hooks.
Three commits on top of Stage 1:
- **State** (`solution/solution/state.rs`): `NCIMSOLVE=2`, `NCIM_PQ_NODE`/
  `NCIM_PV_NODE`, the ~15 `ncim_*` fields (1-based-with-slot-0 node arrays; the
  Jacobian's own 0-based `2*(i-1)` layout), and the `Converged` NCIM branch
  (`ncim_converged` = max|deltaF| ≤ ConvergenceTolerance).
- **Y build** (`solution/ymatrix.rs`): `BuildOption::PdeOnly` — stamps the FULL
  (`ALL_YPRIM`) primitive of every PD element **or** SOURCE into the series handle,
  PC elements excluded (Ymatrix.pas l.442-497); NCIM reads it back via `coo_entries`.
- **NCIM helper** (new `solution/solution/ncim.rs`, ~660 lines): loop-for-loop port
  of every `NCIMSolutionHelper.pas` routine — `NCIM_GetPowers` (Load ConstZ→ZBus
  else PQ; Gen model 3=PV/4=PQ/else Z), `Do{PV,PQ,Z}Bus`, `CalcInjCurr` (I=Y·V,
  first-6 deltaF=0), `BuildJacobian` (fresh `RealSparseSet`, `[B,G;G,−B]` blocks,
  swing identity via the `<6` guards, PV-bus `InitPVBusJac` placeholder cells),
  `GetNumGenerators` (PV-bus indexing + Q-limits), `UpdateGenQ` (PV↔PQ switching),
  `Init` (PDE_ONLY build + flat start), `DoNCIMSolution`. KLUSolveX `SetMatrixElement`
  is 1-based → mapped to the 0-based `RealSparseSet::set_element` by `−1`.
- **Generator** (`elements/pc/generator/mod.rs`): `delta_q_nom`/`ncim_idx`/`ncim_expv`
  (transient solver state, not copied by MakeLike — same convention as dynamics state).
- **Dispatch**: `do_pflow_solution` NCIMSOLVE → `do_ncim_solution`; `check_controls`
  resets `ncim_ready=false`+early-returns when `system_y_changed && algorithm==NCIM`.
- **Options** (`exec/set_cmd.rs`/`get_cmd.rs`/`tables.rs`, `dss_enum/registry/solution.rs`):
  `solve_alg` enum gains `NCIM` (ordinal 2, min-abbrev 2 = prefix `nc`);
  `IgnoreGenQLimits`→`ncim_ignore_q_limit`, `NCIMQGain`→`ncim_gen_gain` at ordinals
  129/130 (the `DSS_CAPI_ADIAKOPTICS` block is ifdef'd out of the capi oracle, so the
  NCIM options follow `NUMANodes=128`) + Get readback. `dump3_commands`: the two
  execoptions lines are dropped by the WP-U1.9 `run_deck_dump_exact_block_masked`
  0.15.x-options mask (integration fix at the wt-u17×wt-u19 merge — u17's
  hand-added golden lines were superseded by u19's uniform mask; the golden stays
  the pure 0.14.5-oracle text).
- **Validation**: `exec/tests/ncim.rs` — every electrical assertion now pinned
  against **capi015** NCIM captures (dss_capi 0.15.0b4 / SVN r4103; the pinned 0.14.5
  gate oracle has no NCIM), embedded golden-style, matched to <5e-11 V (faer-vs-KLU
  floor) under a 1e-6 V band: PQ, ConstZ, PV **regulating within Q-limits** (vpu=1.0,
  Q≈1217 kvar, reported `present_kvar` matched), PV **Q-limit → PV→PQ** (vpu=1.01,
  8 iters, Q=1500), and the **faithful shared non-convergence** (vpu=1.02: capi015
  NCIM also stalls at max iters at the identical `|genbus|=7343.55` fixpoint — pinned
  so a future silent "fix" that diverges from the oracle is caught). Plus warm-resolve
  stability and option round-trip. Gate green (fmt/clippy/`cargo test --workspace`).
- **Audit (Stages 2-4) findings addressed (branch `wt-u17`):**
  - PV-bus path is oracle-validated (above); the earlier "PV bus does not converge"
    concern is a **faithful upstream limitation**, not a port bug — capi015 NCIM fails
    on the same aggressive deck node-for-node.
  - The source bus sitting at the ideal EMF (`7199.56+0i`, no droop) under NCIM —
    flagged as an unported `VSource.NCIM_CalcInjCurrAtBus` bug — is the **correct**
    NCIM value (matches capi015 exactly). `NCIM_CalcInjCurrAtBus` is a *reporting*
    path (`GetCurrents` at the source terminal); it does **not** touch node voltages.
    The old vs-`Normal` self-consistency comparison was the wrong baseline and is
    replaced by the vs-oracle pins.
  - `NCIM_GetPowers` now persists `deltaQNom → Qnominalperphase` (Pascal l.121) so the
    reported model-3 generator Q matches the oracle (was a stale-nominal reporting
    divergence). `exec::Dss::generator_present_kw_kvar` reads the solved `(kW,kvar)`.
  - The two new exec-option help rows (129/130) render the catalog-miss placeholder
    (raw key) — verified empirically that the **pinned 0.15.7 catalog lacks both
    keys**, so `help_catalog.rs` is not stale; identical to the `LongLineCorrection`
    precedent, resolved in the acceptance help-catalog regeneration pass.
- **Remaining (Stage 3 infra / reporting — keeps the gate green because NCIM only
  activates on `Set Algorithm=NCIM` and no corpus deck does yet):**
  1. Fold the above decks into the live-gate `tests/corpus/modes/ncim/` matrix
     (`oracle:"capi015"`, §1.7 manifest + population.lock) — the numerics are already
     oracle-pinned in `exec/tests/ncim.rs`; this is the corpus/manifest plumbing.
  2. `Export Jacobian/deltaF/deltaZ` + `Show PV2PQ_Conversions` reports (numeric-token
     gates) and `VSource.NCIM_CalcInjCurrAtBus` (swing-source reported *currents* under
     NCIM — reporting-only, node voltages already correct).
  3. `oddie:r4088` cross-check of one deck (report-only).

**WP-U1.6 tail (harness-independent rows C4/D11/D13/D12) — LANDED (branch
wt-u16ind).** The four rows that need no 0.15.x property allowlist; each
gate-green, one logical commit.
- **C4** `SolveAll` (cmd 123, the `DSS_CAPI_PM`-only command word) now dispatched —
  single-actor semantics = plain `Solve` (`ExecCommands.pas:346`; `IsSolveAll`
  only steers the parallel/A-Diakoptics path). Oracle-confirmed (dss-python
  0.15.7): `SolveAll` solves like `Solve`; the *spaced* `Solve all` errors
  `Object Class "all" not found` (`Solve` + option token `all`) — the port already
  matched that. Unit `solve_all_alias_matches_plain_solve`.
- **D11 part 2** (`b9bc87b8`) — CapControl TIMECONTROL now REQUIRES a monitored
  element and uses it as `effElement` (dropped the `<> TIMECONTROL` guard in
  `recalc`; only FOLLOWCONTROL falls back to the capacitor + terminal 1). **capi015
  probe** (0.15.0b4): `type=time` with no `element=` errors "Element is not set,
  aborting"; `type=time element=line.l1 terminal=2` keeps `Terminal=2` (0.14.5
  forces →1) and binds to the monitored element. Units re-pinned
  (`time_control_requires_monitored_element` + `time_control_uses_monitored_element_terminal`).
  `capcontrol_time.dss` reworked to `terminal=1` (engine-agnostic readback; it has
  a `daily=` load so it can't flip to capi015, and the only moved observable is the
  static `Terminal`) — stays 0.14.5-green. Props golden `capcontrol.json`
  `capcontrol_time` scenario rebased to `element=Line.l1 terminal=1` (both engines
  identical). Part 1 (PT/CTPhase validation scope) was already aligned.
- **D13** (`c4590d16`) — LoadShape MMF fixes = **not-a-delta** (the port's
  forbid-unsafe eager MMF reader already matches). The behaviorally-live hunk is
  the single-column `csvfile=` `CreateMMF` "missing not": 0.14.5 exits on CreateMMF
  success → empty shape → daily solve `#482 Division by zero`; the fix loads it.
  **Probe:** 0.14.5 aborts #482, capi015 (0.15.0b4) drives the load to P/phase
  `[20 40 70 110 160 130 90 50]` kW. New capi015 live deck
  `modes/upgrade/mmf_singlecol/mmf_singlecol.dss` (`n_steps=8`, whole-model;
  **the first multi-step capi015 live deck** — cannot gate 0.14.5, it *is* the
  bug; §1.7 two-process fingerprint `0ead40d7199b0781`) + unit
  `mmf_single_column_csvfile_loads_like_capi015`. Hunks 2/3 (mmDataSizeQ debug
  field, Linux fpMUnMap disposal) have no forbid-unsafe port equivalent.
- **D12** (`bb9c9785`) — SwtControl `Normal`/`State` field mapping **RE-LANDED**
  (the earlier 82d62c3→0c918ab revert is undone). `Normal`→`NormalState`,
  `State`→`PresentState`, `Action`→`CurrentAction` (distinct offsets); side effects
  sync `CurrentAction` from them. Props golden `swtcontrol.json` re-baselined to
  capi015 (probe 2026-07-16; `swtcontrol_locked_then_action` dropped = the
  not-adopted strict read-only #2024106). The revert's blocker (moved `state`/
  `normal` readback on default-oracle multi-step decks) is **resolved**:
  `swtcontrol_time.dss` + `midi_swtcontrol.dss` **flipped to capi015** (armed
  `action=open` opens at its delay; the port reads `State=Closed` until step 3 then
  `Open`, matching capi015 `GetState`=live element; 0.14.5 read `Open` from the
  arm). `civanlar.dss` (snapshot) **flipped to capi015** (`SwtControl.5_11`
  `Action=c` then `edit action=o`: D12/capi015 `Normal=NormalState=closed`, 0.14.5
  `CurrentAction=open`; physics unchanged, target-rev cases don't property-compare).
  `swtcontrol_lock.dss` **unaffected** (locked switch never operates → both
  readbacks `closed` on every engine; it also can't flip since capi015's strict
  read-only rejects its locked `action=` post). Unit
  `d12_normal_and_state_readbacks_are_independent`.
- **Multi-step capi015 is viable** (correcting the earlier L1 "re-nominalization
  blocks multi-step flips" note): the re-nominalization affects only the
  `getYSparse` element-state re-read, NOT the per-step Monitor/probe/eventlog/node-V
  captures — proven 2026-07-16 by the passing `mmf_singlecol` (loadshape-driven
  load, 8 steps) + `swtcontrol_time`/`midi_swtcontrol` (12 steps) capi015 live
  gates. This unblocked the D12 deck flips.
- **Sibling scope (NOT this branch):** C5 RegControl `FwdThreshold`+idle props, C6
  Transformer/AutoTrans BH props, C5-r3723 LoadShape `Mode` — all property-adding
  rows owned by `wt-u16tail`. B4-capi harmonics init-abort stays faithful-as-is
  (no feature-sensitive deck possible). DIVERGENCES.md D11/D12/D13/C4.

**WP-H015 — 0.15.x property-table allowlist (`PROPS_015X`).** Resolves Rung-1
remaining-tail item 1. The corpus property-parity gate
(`harness::compare_all_properties`) asserts the Rust property-table **shape**
(count + name order) against the oracle capture; the pinned default oracle is
dss_capi **0.14.5**, so a deliberately ported 0.15.x-added property (Line
`EpsRMedium`/`HeightOffset`/`HeightUnit`/`Conductors`; RegControl `Idle`/
`IdleReverse`/`IdleForward`/`FwdThreshold`; Transformer+AutoTrans `BHpoints`/
`BHcurrent`/`BHflux`; LoadShape `Mode`) would break nearly every default-oracle
deck's shape assert.
- **Decision (adopted, coordinator-approved): a named per-class allowlist** —
  `PROPS_015X: &[(&str, &[&str])]` in `tests/harness/mod.rs`. A Rust-side prop
  whose `(class, name)` is in the table **and** whose name is absent from the
  oracle capture is excluded from the count/order/name/value walk (a §1.3-style
  *shape* relaxation, NEVER a value-tolerance change). Handles **inserted** props,
  not only trailing (the Rust list is filtered to what a 0.14.5 capture can know,
  then compared position-for-position). If the capture DOES contain the prop
  (capi015-regenerated), it is NOT excluded → full name+value compare, so capi015
  decks keep pinning the new props' values. A non-allowlisted extra/missing/
  misordered prop still fails exactly as before; the count-mismatch panic names
  the allowlist for triage.
- **Implementation:** extracted the list-comparison core into `compare_prop_lists`
  (allowlist injectable) so the shipped-empty table is validated by 6 inline
  `props_015x_tests` self-tests with synthetic data (trailing extra passes;
  inserted extra passes with order preserved; non-allowlisted extra panics;
  allowlisted-present-in-oracle value mismatch panics + match passes; missing +
  misordered panic).
- **Surface survey (item 3):** `compare_all_properties` is the **only** surface
  that asserts Rust property count/order against a 0.14.5-pinned capture.
  `props_roundtrip.rs` + the props goldens (`golden_feeders_controls`,
  `scenario.rs`) iterate only the props present in the (0.14.5) golden — no
  Rust-side shape assert. `save_roundtrip.rs` round-trips through our own engine
  (node-V + discrete state), not oracle property tables. `population_lock.rs`
  fingerprints manifest membership/rigor, not property-table shape. None need the
  allowlist.
- **Ships EMPTY** — rows land with the three sibling Rung-1 WPs that port each
  property (one class per line, merge-friendly; duplicate class rows OR).
- Docs: `tests/TOLERANCE_NOTES.md` §"0.15.x property-table allowlist (shape
  relaxation)"; `TESTING.md` pointer. Gate green (fmt/clippy/`cargo test
  --workspace`) — nothing moves, the table is empty.

**WP-U1.7 (NCIM solver) tail — COMPLETE (branch `wt-u17tail`).** The three
"Remaining (Stage 3 infra / reporting)" items above are done; WP-U1.7 is fully
landed. Spec = `Common/ExportResults.pas` (`ExportJacobian`/`ExportdeltaF`/
`ExportdeltaZ` l.3903-3988), `Common/ShowResults.pas` (`ShowPV2PQGen` l.3978),
`PCElements/vsource.pas` (`NCIM_CalcInjCurrAtBus` l.1225), all 0.15.x.
- **Reports** (`report/export/ncim.rs`, `report/show/pv2pq.rs`, wired at export opts
  62-64 / show opt 35 in `exec/report.rs`; `EXPORT_OPTIONS`/`SHOW_OPTIONS`):
  `Export Jacobian` dumps the last NCIM Jacobian triplets (`RealSparseSet::coo_entries`,
  0-based column-major `Row,Col,Value`); `Export deltaF`/`deltaZ` dump the mismatch/
  correction vectors; `Show PV2PQ_Conversions` lists generators carrying `NCIM_ExPV`.
  Gated vs **capi015** in `crates/dss-core/tests/ncim_reports.rs` (goldens
  `tests/golden/ncim/`, `tools/golden/gen_ncim_reports.py`): Jacobian numeric-token
  compare (row/col exact, value 1e-6 — built from node V pinned <5e-11), PV2PQ
  byte-exact, deltaF/deltaZ **structural** (line count `2·NumNodes+PVphases`, six
  leading swing zeros, converged floor — the vectors are ~1e-11 faer-vs-KLU noise, so
  value-pinning them is meaningless; documented in the test). `Export Jacobian` with
  no NCIM solve raises #222 "Jacobian matrix not built."; deltaF/deltaZ silently no-op
  (unit-pinned in `exec/tests/ncim.rs`).
- **`VSource.NCIM_CalcInjCurrAtBus`** (`exec/view.rs::snapshot_elements` post-pass):
  under NCIM the swing bus sits at the ideal EMF, so `YPrim·V - Iinj` reports ~0; the
  swing source's reported currents are the KCL sum at its bus (− PDE terminal currents,
  + other PCE terminal currents). Reproduces the upstream 0-based/1-based `ElmCurrents`
  **off-by-one** (`TODO(compat)`: the reported phase-A current is the negated phase-**B**
  branch current — deterministic, in-range). Pinned vs capi015 currents/powers/losses
  (`ncim_vsource_reported_currents_match_oracle`).
- **Two engine reporting fixes** the corpus decks exposed: (1) `system_y_csc` now reads
  the **active** Y handle (Pascal's moving `hY`), so after an NCIM `PDE_ONLY` build it
  reports the PDE-only network Y (no load `Yeq`), matching `getYSparse`; (2) NCIM
  generator terminal currents are overridden in the snapshot from the solver's
  `-conj((Pnom+j·deltaQNom)/V)` — the post-NCIM `YPrim`/`Yeq` is stale, so the general
  `YPrim·V-Iinj` recompute no longer collapses to it (`ncim_generator_currents`).
- **Corpus decks** `tests/corpus/modes/ncim/` (all `oracle:"capi015"`, `pending:false`,
  live-compared in `modes_cases_match_oracle`, `population.lock` regenerated):
  `ncim_pq` (micro PQ, 9 nodes, 3 iters), `ncim_pv_pq` (PV→PQ Q-limit, 6 nodes, 8 iters),
  `ncim_midi` (27-node IEEE123-class radial, PV→PQ, 8 iters). Each §1.7-validated on
  capi015 (converged + bit-identical across two processes).
- **`oddie:r4088` cross-check** (report-only, `docs/upgrade/DIVERGENCES.md`): r4088
  (OpenDSS 10.2) supports NCIM and reaches **bit-identical** converged node voltages,
  but converges the PV→PQ decks in 4 iters vs capi015's 8 (PQ-only matches at 3). Pinned
  to capi015; iteration policy already `<=` for capi015 cases; not gated.
- **Audit follow-ups settled** (this branch): (1) the NCIM swing/generator loss override
  in `exec/view.rs` now applies the positive-sequence ×3 that the general `elem.losses()`
  path does, so overridden `loss_w` stays consistent with the per-conductor powers under a
  positive-sequence CktModel (latent — the ncim decks run full 3-phase); (2) the deliberate
  non-reproduction of Pascal's PC-loop `myTerm` accumulation (`NCIM_CalcInjCurrAtBus`
  l.1268 — a stateful cross-element index that can run OOB; we compute it fresh per element,
  identical on the defined path) now carries an explanatory comment beside the `+1`
  `TODO(compat)`; (3) the deltaF/deltaZ structural bound tightened 1e-6→1e-8 (observed max
  ~2.2e-10, ~45x headroom) to catch a systematic ~1e-8-scale offset without value-pinning
  the genuine ~1e-11 cancellation noise.

**WP-U1.4 property tail + WP-U1.2 D3 (branch wt-u14props) — LANDED.** Ported the
0.15.x Line-level property surface + catalog fixes on top of the already-landed
equivalent-spacing engine numerics (the WP-U1.4 partial), gate-safe (existing
line_constants goldens byte-untouched):
- **Line `EpsRMedium`/`HeightOffset`/`HeightUnit`** (Line.pas:59-61,375-431): raw
  double/double + MappedStringEnum(UnitsEnum) props with the `HeightUnit` none->m
  side effect (Line.pas:646). Threaded into `make_z_from_geometry` /
  `make_z_from_spacing` in the exact upstream order (`SetEpsRMedium`,
  `SetHeightOffset`, `SetUserHeightUnit`) *before* the matrix read, so the
  `set_user_height_unit` meters re-conversion `TODO(compat)` is now live. `EpsRMedium`
  divides the shunt Pfactor (moves Yc on both paths); `HeightOffset` folds into the
  equivalent-spacing average heights (observable there) and is a no-op on the
  detailed-coordinate path (SetY re-sets FY) — reproduced 1:1, probe-confirmed on
  capi015. Defaults (1.0/0/m) preserve 0.14.5 numerics. `PROPS_015X` allowlist gains
  `("Line", ["EpsRMedium","HeightOffset","HeightUnit"])` (`Conductors` is the sibling
  wt-u14cnts row). Pins: capi015 props golden `props/linemedium.json`
  (default/set/none/units rendering), capi015 decks `modes/upgrade/
  upgrade_linecs_epsrmedium` + `_heightoffset` (YPrim live compare, §1.7
  bit-identical across two processes). The three props insert at their upstream
  index (shifting NormAmps 31->34), breaking every *index-absolute* 0.14.5-gated
  surface; new `PropFlags::HIDE_015X` defers them from the `Dump` text report +
  AltDSS JSON export, and the `Dump commands` catalog renumbers via a running
  counter (0.14.5 props keep their indices). The `?`/props-table surface still
  exposes them; drop `HIDE_015X` when the Line Dump/JSON/catalog goldens flip to
  capi015 alongside the sibling's `Conductors`.
- **LineType enum width 4->5** (DSSClass.pas:1071): the eight `swt_*` names share the
  4-char prefix `swt_`, so 5-char abbreviations (`swt_l`->swt_ldbrk, ...) fell back to
  `oh` under width 4. One-char fix in `dss_enum/registry/pd.rs`; unit test
  `line_type_abbreviations_widened_to_five_chars` + capi015 deck
  `upgrade_linetype_width` (LineType probes; the 0.14.5 oracle renders these as `oh`).
  No corpus deck used `linetype=`, so the default gate is unaffected.
- **LineCode FaultRate/PctPerm/Repair deprecation** (LineCode.pas:283-288): new
  metadata-only `PropFlags::DEPRECATED`/`UNUSED` set on the three props. Schema-only —
  probe-confirmed capi015 stores the values silently (no runtime warning), so no
  golden/behaviour moves.
- **WP-U1.2 row D3 spacing ratings** (LineGeometry.pas:1060-1064): the Line
  `makeZFromSpacing` throwaway geometry now derives NormAmps/EmergAmps as the *minimum
  over the phase conductors* (`j <= FNphases`), not conductor 1. Localized to
  `load_spacing_and_wires` (the NIL/actualNConds sizing stays the sibling's row); unit
  test `spacing_ratings_min_over_phase_conductors` + capi015 deck
  `upgrade_spacing_ratings` (min 600/400/600 -> 400/500 via probes). The existing
  `asymmetric/line/line_spacing_asym` deck (distinct phase wires) moved NormAmps
  730->230, so it flipped to `oracle:capi015` with ratings probes — YPrim is
  bit-identical 0.14.5==capi015 (ratings-only flip).
- **Remaining WP-U1.4 (sibling wt-u14cnts):** `Line.Conductors` mixed wire/CN/TS
  list, the merged `CNTSLineConstants` per-conductor class, `CNData.SemiconLayer`.
- Gate green (fmt/clippy/`cargo test --workspace`); population.lock regenerated for
  the 4 new decks + the asym oracle flip.

**WP-U1.4 heavy tail (branch wt-u14cnts) — merged TCableConstants per-conductor
model + CNData.SemiconLayer — LANDED.** dss_capi 0.15.x merges the separate
`TCNLineConstants`/`TTSLineConstants` classes into one `TCableConstants`
(`CableConstants.pas`): the CN-vs-TS choice moves from the engine kind to a
per-conductor `FCondType[i]` (`SetCondType`), so one engine carries **mixed
wire/CN/TS conductors** (Kersting). Ported 1:1, defaults byte-green:
- `support/line_constants`: `LineConstantsKind` collapses to `{Overhead, Cable}`;
  new `ConductorType` (Invalid/Cn/Ts/Bare) + per-conductor `fcond_type`/
  `fsemicon_layer`; the two `calc_cn`/`calc_ts` merge into one `calc_cable` (in
  `cable.rs`) branching per conductor, with the `GetDij` equivalent-spacing helper
  and the semicon capacitance branch; `cn.rs`/`ts.rs` keep only their setters.
  `new_cn`/`new_ts` become convenience presets over the merged cable engine.
- `CNData.SemiconLayer` (prop 5, LongBool default `true` = classic
  `ln(RadOut/RadIn)`; `false` = Synergi/Kersting no-semicon
  `ln(RadCN/RadIn)-(1/k)·ln(k·RadStrand/RadCN)`). Threaded through
  `CableGeom::Cn` + `UpdateLineGeometryData`'s `SetSemiconLayer`. Harness
  `PROPS_015X += ("CNData", ["SemiconLayer"])` (inserted prop).
- **Byte-green proof.** A pure-CN/TS geometry reproduces the old `Calc`
  bit-for-bit (the whole `line_constants`/`line_geometry` golden family +
  `props_roundtrip` stay green untouched); `GetDij` = old raw `sqrt` in the
  default path.
- **New numeric surface pinned vs capi015 (1e-8):** engine
  `cn_cable_no_semicon_capacitance` (C 167.168 vs 283.089 nF/km) and LineGeometry
  `matrices_mixed_cn_ts_wire_match_capi015` (mixed CN/TS/wire reduced Z/Yc);
  CNData `cndata_semicon_layer_roundtrip` (Yes/No render + make_like).
- **Gate decks (capi015):** `modes/upgrade/upgrade_linecs_mixed.dss` (mixed
  conductors on one line) + `upgrade_linecs_semicon.dss` (`SemiconLayer=no`) —
  both converge (2 iters), two-process bit-identical, whole-model + YPrim
  live-green. `population.lock` regenerated (modes 58→60). DIVERGENCES.md
  §"Merged TCableConstants".
- **DEFERRED — `Line.Conductors`** (the 0.15.x Line-level mixed-conductor
  *property*, prop 34): the engine (its whole point) is landed and mixed
  conductors already work via a `LineGeometry`; the property itself needs a
  net-new 3-class proxy-array resolver (WireData|CNData|TSData), a JSON-export
  restructure (the `"Conductors"` key today maps from `Wires`), and a Line
  property-table insertion coordinated with the parallel wt-u14props branch
  (Conductors must land at 34, after that branch's EpsRMedium/HeightOffset/
  HeightUnit at 31–33). Left for a follow-up to keep the gate green.
**WP-U1.6 tail (C5 / C6 / C5-r3723) — LANDED (branch wt-u16tail).** Three of the
previously-deferred WP-U1.6 rows, gate-green (fmt/clippy/`cargo test --workspace`),
built on the `PROPS_015X` harness allowlist:

- **C5** RegControl `FwdThreshold` + idle zones (`8a898cba`, SVN r4086, in capi015
  0.15.0b4). `RevThreshold` is now a **signed W** field (default −100 kW, kW→W via
  property `scale`), joined by `FwdThreshold` (+100 kW) and the
  `Idle`/`IdleReverse`/`IdleForward` flags. The reverse-power detection sign moved
  into the stored value (`FwdPower < RevPowerThreshold`, no unary −) so **legacy
  decks are behavior-identical**: the new `EndEdit` fallback (`Fwd:=abs(Rev);
  Rev:=−Fwd`) restores the old symmetric band. The fallback is **per-edit**, via a
  new `DssObjData` BeginEdit boundary (`PrpSequence[NumProps+1]`, wired at the
  executive edit-start) — a later rev-only edit re-symmetrizes and clobbers an
  earlier Fwd, reproduced 1:1 (capi015-probed). The base `MakeLike`
  (`copy_prp_sequence_from`) copies the counter + property slots but **not** the
  boundary slot (Pascal `MakeLike` copies `NumProps+1` ints, excluding index
  `NumProps+1`), so a `New … like=parent` child keeps its own boundary and the
  fallback fires per the child's own edit — audit fix, regression-tested in
  `obj/base/tests.rs`. Idle no-load test ported verbatim
  incl. the `>=/<=` OR (spans the whole axis at the default band; **not** "fixed"
  to AND). **Gate:** capi015 props golden re-baseline
  (`tests/golden/props/regcontrol.json` via `gen_regcontrol_capi015.py`) pinning the
  signed defaults + the two-edit fallback; `PROPS_015X` RegControl row; capi015 deck
  `regcontrol_idle.dss` (idle holds tapnum 0/tap 1.0 where a non-idle reg reaches
  tapnum 15/1.09375, |ΔV|≈0.075 pu; §1.7 two-process deterministic); 5 RegControl
  unit tests. Legacy equivalence rides the unchanged default-oracle
  `regcontrol_reverse.dss`. DIVERGENCES.md §C5.
- **C6** Transformer/AutoTrans `BHpoints`/`BHcurrent`/`BHflux` (`90962ae8`, SVN
  r4064). Three GICharm `Unused` data props on both classes — parse+store only,
  never consumed (no GICharm port): the props, the `BHpoints` realloc side effect
  (zeroes both arrays), and the MakeLike copy. **Upstream crash NOT reproduced**
  (CLAUDE.md UB rule): parsing a non-empty `BHcurrent` **segfaults** the capi015
  backend and reading with `BHpoints>0` errors — the `Unused` DoubleVArray getter
  reads its count from an **unset `PropertyOffset2`** (only `Offset3` is wired) →
  garbage `Norder`. Only the empty default is well-defined (`GetDSSArray` NIL-guards
  to `''`); the port matches (empty Vec ⇒ `''`) and renders the set-state safely.
  **Gate:** capi015 default-state props goldens (`transformer_bh.json`/
  `autotrans_bh.json` via `gen_bh_capi015.py`); `PROPS_015X` rows for both classes;
  set-state unit-pinned on both classes. DIVERGENCES.md §C6.
- **C5-r3723** LoadShape `Mode` — **NOT a delta for us; unported.** EPRI SVN r40xx
  inserts `Mode` at 22 (shifting `Interpolation` 22→23), but dss_capi 0.15.x
  declines it (`// Mode = 22, -- not useful to implement this yet`, `Interpolation
  = 22` in **both** 0.14.5 and 0.15.0b4). capi015 has 23 props, `Interpolation` at
  22, no `Mode` (probed) — the port already matches. Porting Mode would break every
  LoadShape deck's count parity against the oracle, so it stays unported; guard test
  `no_mode_prop_interpolation_stays_at_22`. DIVERGENCES.md §C5-r3723.

(Merge note 2026-07-17: the sibling `wt-u16ind` landed C4/D11-part-2/D12/D13 in
the same round — see its block above — and B4-capi stays faithful-as-is, so
WP-U1.6 is COMPLETE.) `known_diffs.json`: none of C5/C6/C5-r3723 had a prior
Rust↔EPRI entry — nothing to retire.

**WP-U1.4 final row — `Conductors` property — LANDED (branch wt-u14cond); WP-U1.4
now COMPLETE.** The last deferred WP-U1.4 row: the dss_capi 0.15.x `Conductors`
mixed `WireData|CNData|TSData` object-reference-array on both classes (Line prop
**34**, `Line.pas:62`; LineGeometry prop **20**, `LineGeometry.pas:80`), resolved
through a `TProxyClass` created with `fullNames=True`, `.Name = "Conductor"`
(`DSSClass.pas:2603`). Inserting Line prop 34 shifts NormAmps 34→35 (tail +1).
- **The text property is upstream-BROKEN** (probed capi015 0.15.0b4) and is
  reproduced 1:1. `TProxyClass.GetDSSClass` compares an `AnsiLowerCase`d class
  token against the original-case target names, so every real item errors #10103
  "Invalid class (wiredata)…"; a bare item errors #10103 "You must define the
  Conductor class…"; a pre-spacing list errors #402 "No objects are expected!";
  an all-`none` list parses on Line (NIL slots, overhead model) but is rejected on
  LineGeometry (#10103 "At least one valid conductor must be provided"). The text
  getter `? …Conductors` Access-Violates in capi015 (UB, not reproduced). The
  property is thus effectively JSON-only. Ported in `parse_conductor_proxy`
  (`obj/props/class_props/parse.rs`, new `PropDef::object_ref_array_proxy` +
  `object_classes`/`proxy_name` fields), the Line/LineGeometry property tables
  (`HIDE_015X`), Line `set_conductors`/`conductors_phase_choice` + side effect,
  LineGeometry `apply_conductors` side effect. `TODO(compat)` on the GetDSSClass
  case bug (clean fix = compare lowercased names, §6 sweep).
- **JSON export unchanged / no golden movement.** dss_capi emits
  `"Conductors":[FullName…]`; the Rust port already emits the same bytes via the
  `Line.Wires → "Conductors"` `json_name` masquerade (since wt-u14props). The real
  `Conductors` prop carries `HIDE_015X`, so it is invisible to the byte-exact
  0.14.5 Dump / FULL-JSON / `Dump commands` goldens (catalog running-counter skips
  it) and the masquerade keeps owning the JSON key. The flip to a capi015
  Dump/JSON surface + dropping the masquerade is deferred to the §6 sweep:
  `gen_json.py` is 0.14.5-pinned (no capi015 engine switch), so flipping is
  disproportionate for this row (UPGRADE_PLAN §1.4 fallback). Residual (latent,
  untested): the masquerade renders a mixed-class list with one `WireData.` prefix
  vs capi015's per-conductor class — no golden/deck exercises it. DIVERGENCES.md
  §"Line/LineGeometry Conductors (text upstream-broken)".
- **Gate.** `PROPS_015X` gains `Conductors` on the Line row + a new `LineGeometry`
  row (inserted props excluded from the 0.14.5 shape walk);
  `tests/upgrade_conductors.rs` pins all four capi015 diagnostics + the all-`none`
  split. The net-new **resolved-ref** fill (unreachable via the broken text parse;
  the path the §6-fixed parser + a JSON-import round-trip take) is gated by whitebox
  equivalence tests that call `set_object_ref_array(CONDUCTORS)` + the side effect
  directly — Line `conductors_array_matches_buried_neutral_and_oracle` /
  `conductors_array_overhead_matches_wires_and_oracle` /
  `conductors_all_none_after_wires_clears_wires_seq`, LineGeometry
  `conductors_array_matches_mixed_capi015` /
  `conductors_array_defaults_ratings_from_first_valid` — each pinned to the same
  capi015 Z/Yc/ratings as the traditional `wires=`/`cncables=` paths, so
  `set_conductors`/`conductors_phase_choice`/`apply_conductors`/per-conductor
  `change_line_constants_type`/`default_amps_from`/`conductor_choice_of` and the
  last-writer `clear_seq` are covered (audit wt-u14cond, major finding). No
  solvable-corpus / byte-golden case moves; fmt/clippy/`cargo test --workspace`
  green (incl. the live oracle gate). `known_diffs.json`: no prior Rust↔EPRI entry
  (0.14.5 has no `Conductors` prop) — nothing to retire.

**WP-U1.10 — Rung 1 EXITED (2026-07-16, branch wt-u110).** The formal rung-1 exit:
the opt-in EPRI r4088 sweep (`DSS_LIVE_OPENDSS=r4088 DSS_LIVE_OPENDSS_ASSERT=1
cargo test -p dss-core --test corpus_live`) is **green** — every remaining
Rust↔r4088 divergence is a justified `known_diffs.json` entry or a Rung-2 item;
**zero unexplained**.
- **Sweep.** 430 cases (71 target-rev `oracle:capi015` excluded): 313 matched, 113
  known-diverged, 4 known-skipped, **0 NEW**. An informational r3723 ASSERT sweep
  (also green) supplied the prune criterion + confirmed the new-deck classes are
  rev-independent (classic power flow / injection / meter zone / harmonics /
  reduction byte-identical r3723=r4088, `delta_r3723_r4088.md`).
- **No Rung-1 regression (proof spine).** Every swept case is ALSO in the mandatory
  gate vs the pinned dss_capi 0.14.5 oracle, which is green → the port equals the
  FPC oracle on all 55 new divergences → the r4088 gap is purely the
  FPC(0.14.5)↔Delphi(r4088) layer, corroborated by the committed capi015↔r4088
  engine sweep (`docs/upgrade/sweeps/capi015_vs_r4088.md`). A Rung-1 regression
  would have turned the mandatory gate red.
- **Catalog burn-down (11→22 entries).** PRUNED `epri-gendispatcher-propname` (dead
  on both revs — 0 hits; the gendispatcher decks diverge on control-iteration count,
  not a property name. The decks do set `kvarlimit`/`genlist`/`weights`, all seven of
  which exist in dss_capi 0.14.5 `GenDispatcher.pas` so the port accepts them; no
  engine surfaces the original `#34` "Invalid property name" on the swept decks;
  residual iteration delta folded into `iteration-count-delta`). EXTENDED 5 to r4088
  (iteration-count-delta, storage-kwhstored-drift [kWhStored idling-loss drift],
  injection-fpc-delphi-ulp, meter-zonepce-count, harmonics-yfingerprint-drift). NEW:
  7 cross-solver FPC-vs-Delphi floors (autotrans reg-tap, makeposseq, reduce, ckt24
  SubXFMR conditioning, PVSystem-kvar Delphi 6-sf display, Storage-`kw` Delphi 6-sf
  display [`.kw:`-scoped, split from storage-kwhstored-drift], Vsource near-zero
  power), 1 r3723-only (invcontrol-fixpoint-drift-synthetic), 4 `skip` (EPRI r4088
  #303 crashes:
  binary-shape [+r3723], IEEE13 line-spacing, IEEE13 line+cable-spacing,
  CapControlFollow). Full ledger: `docs/upgrade/known_diffs_burndown.md`.
- **Docs.** `tests/corpus/COVERAGE.md` refreshed (solvable_now **295/329 = 89.7%**,
  skipped_needs_investigation 19→10); `tools/corpus/coverage_report.py` fixed to
  report only the true partition buckets — it was globbing all `manifests/*.json`
  and double-counting the `ad_sweep.json` disposition list (295, overlaps
  solvable_now) + the empty `population.lock` row into the total. Marker sweep:
  `rg "NOT_PORTED\(U1"` is empty across all source (pinned; only prose in the plan
  docs references the tag).
- **Rung 1 is COMPLETE** (U1.1–U1.10). Next: Rung 2 (WP-U2.* — r4133 protection
  overhaul + the r4133-side IEEE_519/InductionMachine moves, `delta_r4088_r4133.md`).

**WP-U2.1 — Fuse overhaul (r4133) — LANDED (branch wt-u21).** Ported the
`Controls/fuse.pas` r4088→r4133 delta (rows C3/D1/E3) loop-for-loop:
- **Defaults/semantics:** `RatedCurrent` repurposed to an informational continuous
  rating (default 1.0 → **0.0**, unused in `Sample`); new **`CurveMultiplier`**
  (default 1.0) is the TCC divisor — `GetTCCTime(Cmag/CurveMultiplier)`, not
  `RatedCurrent`; default `FuseCurve` `tlink` → **`none`** (a default-constructed
  fuse **never blows**); new informational `InterruptingRating`. Props 10 → **12**
  (BaseFreq/Enabled/NumProps shift +2). `elements/pd/fuse/{mod,accessors}.rs`.
- **`GetTccCurve('none')` → NIL silently** (E3/D1): new `PropFlags::ALLOW_NONE_REF`
  on the Fuse `FuseCurve` single ref — literal `none` resolves to NIL with **no
  #401** and renders `none` (Pascal `if FuseCurve<>nil then Name else 'none'`).
  Fuse-local; Recloser/Relay keep the not-found path until U2.2/U2.3. Supersedes
  the Rung-1 capi015 clear+#401 pin (`exec::tests::lifecycle` updated to r4133).
- **Property surfaces:** the 0.14.5-absent `CurveMultiplier`/`InterruptingRating`
  carry `HIDE_015X` (byte Dump/`Dump commands`/JSON goldens stay green) + a
  `PROPS_015X` Fuse row (shape walk). The changed defaults `FuseCurve`/`RatedCurrent`
  get `SKIP_PROPS` rows (value mask on the 0.14.5 all-props walk, RegControl
  RevThreshold precedent (e)→(f)); pinned instead on the r4133 side.
- **Goldens:** `tests/golden/props/fuse.json` regenerated to the r4133 surface
  (derived from the retired 0.14.5 golden + Oddie-verified r4133 value deltas —
  no capi engine has the r4133 fuse behavior; `gen_fuse_r4133.py`). Protection
  golden `fuse_blow.json` captured on the **r4133 Oddie** engine (blow trajectory
  preserved via `fusecurve=tlink curvemultiplier=40`, reproducing the old
  RatedCurrent=40 divisor; `gen_protection.py` gained an oddie route).
- **Deck matrix** (`controls/fuse/`, §1.7 two-process-validated on r4133): new
  `fuse_curvemult_blow` (SLG, `curvemultiplier=40` → single-phase blow at Sec=0.4;
  feature-sensitive — `curvemultiplier=1` melts all three; pins the divisor) and
  `fuse_legacy_noblow` (legacy RatedCurrent-only → never blows). Existing
  `fuse_blow_3ph`/`fuse_blow_asym`/`midi_fuse` flipped `oracle:"r4133"` (now
  never-blows; re-probed).
- **Breaking-default fallout:** the fuse-save combo decks `combo/combo_protection`
  and `combo/midi_protection` are entangled with the **unported** Recloser default-
  curve removal (proven inert on r4133) + Relay r4133 wording — they cannot flip
  to r4133 (recloser/relay diverge) nor stay on 0.14.5 (fuse diverges), so their
  fuse tier is temporarily **neutralized** (ratedcurrent raised → never blows on
  0.14.5, matching the port) with a documented deferral to the protection-rung
  completion. Vendored `InductionMachine/{Master.DSS,Run.dss}` go **non-convergent
  under r4133** — proven on the EPRI r4133 engine itself (explicit-curve fuses
  blow at Sec=0, island the transformer) — so they move `solvable_now` →
  `skipped_needs_investigation` (tag `r4133_breaking_nonconvergence`); the port
  correctly reproduces the r4133 divergence. `population.lock` regenerated.
- **known_diffs:** no fuse-scoped entries present (nothing to retire).

**WP-U2.2 — Recloser per-phase rewrite (2026-07-17, branch wt-u22).**
Ported `Controls/Recloser.pas` r4088→r4133 (delta rows B3/B4/C2/D2/D3/E2/E3) into
`elements/control/recloser/` — a full per-phase rewrite validated exactly against
the `oddie:r4133` engine (v11.0.0.1 Charlottesville).
- **Per-phase state machine.** `FPresentState`/`FNormalState` are per-phase state
  arrays; `OperationCount`/`LockedOut`/`ArmedForOpen/Close`/`PhaseTarget`/
  `RecloserTarget` are per-phase with the ganged `IdxMultiPh = NPhases+1` slot
  (frozen at 4). Arrays are 1-based (`[T; RCMAX+2]`, slot 0 unused) — Pascal's
  >3-phase OOB (arrays frozen at IdxMultiPh=4) is UB, **not** reproduced (sized to
  avoid it; ≤3-phase — every real deck — is exact).
- **Single-phase trip/reclose/lockout** (`SinglePhTrip`/`SinglePhLockout`): the
  phase index rides the control-queue **proxy handle** — plumbed through
  `ControlOp::Action{code, proxy}` (dispatch/actions/multi_rate) to
  `Recloser::do_pending_action(code, proxy, …)`; every other control ignores it.
- **Fast/slow pickup split** (`PhFastPickup`/`PhSlowPickup`, `Gnd*`); legacy
  `PhaseTrip`/`GroundTrip` set both. **D2 breaking default:** the A/D default
  curves are removed → a curveless recloser is **inert**. **D3:** inst trip time
  is a bare `0.01` (MechanicalDelay added once at push).
- **Props 24 → 46** with deprecated aliases (`PhaseFast→PhFastCurve`, `Reset→
  ResetTime`, `Delay→MechanicalDelay`, TD renames…), `Lock`/`Reset` actions,
  `EventLog`/`DebugTrace`, `RatedCurrent`/`InterruptingRating`, `Normal`/`State`
  per-phase arrays (`[closed, closed, closed, ]`, ganged scalar or quoted list).
  `ShowEventLog := EventLogDefault` (global **False**) — no override (the oracle
  logs nothing until `EventLog=yes`, empirically confirmed).
- **Event-log wording overhaul (E2/E3):** the r4133 per-phase messages
  (`Phase %d opened on %s (…trip) & locked out (…lockout)` etc.) reproduce the
  oracle **byte-for-byte** (proven for single-phase, ganged, ground and
  pickup-split decks — no mask row needed).
- **Gate.** Family decks `recloser_temp/perm/ground` + midi twins flipped to
  `oracle: "r4133"`. `temp`/`midi_temp` are curveless → inert on r4133, pinning the
  D2 removed-default witness; `ground` trips via the ground curve; `perm`/`midi_perm`
  carry explicit A/D curves and exercise the ganged lockout-to-OPEN sequence (see
  the audit-fix addendum below).
  New decks `recloser_1ph.dss` (single-phase trip/lockout) + `recloser_pickup_split.dss`
  (fast≠slow pickup) — all §1.7-validated on r4133. `props/recloser.json`
  regenerated to the r4133 46-prop surface (props_roundtrip green); the
  `golden_protection` recloser scenarios retired (§1.3-2, superseded by the live
  r4133 gate). **controls family live gate: 96 decks match the oracle.**
- **Event-log mask infra CREATED** (`harness::EVENTLOG_MASKS`, §1.3-3):
  per-oracle-spec `(find→to)` substitutions applied to both engines' lines, never
  dropping/reordering; **the shipped r4133 table is EMPTY** (the port is exact —
  the empty table is the proof), with self-tests + TOLERANCE_NOTES doc, generic
  for WP-U2.3 to extend. Enabling fix: `oracle_server.capture_eventlog` now reads
  the `export eventlog` CSV for the Oddie engine (its `Solution.EventLog`
  accessor returns empty — a bridge gap that blocked every r4133 event-log
  compare).
- **compare_all_properties** skips the Recloser class (its table moved to the
  r4133 46-prop shape; ungateable vs the 0.14.5 oracle — shape is code-verified +
  `recloser.json` gates values by name). Cross-chain combo decks
  (`combo/midi_protection`, `combo/combo_protection`) kept on the 0.14.5 oracle by
  naming the recloser's A/D curves explicitly (identical ganged behavior) and
  dropping their Recloser probe + `compare_eventlog` until Fuse (U2.1) + Relay
  (U2.3) land and the whole suite flips at rung exit (U2.6). population.lock
  regenerated.

**WP-U2.2 audit fixes (2026-07-17, wt-u22).** Three findings addressed:
- **(major) Restored ganged lockout-to-OPEN oracle coverage.** `recloser_perm` +
  `midi_recloser_perm` were flipped to `oracle:"r4133"` but left **curveless** →
  inert no-ops (they asserted nothing about trip/reclose/lockout, matching r4133
  only because both engines did nothing), so the `do_open_ganged` lockout branch was
  validated by a Rust-only unit test against no oracle. Gave both decks explicit
  `phasefast=a phasedelayed=d` (the engine's built-in A/D curves — r4133 D2 removed
  the defaults) so a 3ph permanent fault now drives FAST→reclose→SLOW→reclose→lockout
  → ends `[open,open,open]`, re-validated **exactly** against `oddie:r4133`.
  `midi_recloser_perm` also fixed at its generator (`tools/decks/gen_midi_decks.py`).
  `temp`/`midi_temp` kept curveless **on purpose** as the D2 removed-default witness
  (the reclose-to-CLOSED path is covered by `ground`); manifest notes added to all
  four so the intent is explicit. controls live gate still 96/96 vs r4133.
- **(minor) DebugTrace wording matched to Recloser.pas r4133.** The ground-trip trace
  now emits the distinct inst line (`Gnd Instantaneous Trip`, raw `Cmag`) vs curve
  line (`Gnd %s Curve Trip`, `Cmag/GroundCurveMultiplier`); the single-phase and
  three-phase **curve-branch** traces (`Ph %s (1-Phase)/(3-Phase) Trip`), previously
  missing, are now logged (probe-confirmed). Latent path (no deck sets
  `DebugTrace=yes`); the residual `%.3g`-vs-`{:.3}` sig-fig rendering is absorbed by
  the numeric-skeleton comparator and left as-is.
- **(minor, DEFERRED) quoted single-element `state=[open]` parses as ganged.** Delphi
  branches on `Parser.WasQuoted` (a quoted single-element list sets only phase 1);
  the Rust `set_enum_array` sees only the parsed ordinal array. A faithful fix must
  plumb `WasQuoted` through the **shared** `MappedStringEnumArray` parse dispatch +
  the shared `set_enum_array` trait (which also drives **Fuse**, U2.1/U2.4 territory)
  — out of this WP's recloser-local scope, latent (no deck/test exercises it, no
  oracle channel to validate), and already documented at `accessors.rs:272-277`.

**WP-U2.4 — SwtControl D6 + RatedCurrent, batchedit `where`, TCC/DoNewCmd/AllocateLoad
verify — LANDED (branch wt-u24, base `1287ec4`).** Rung-2 rows C4/C5/C6/D6/E1 of
`delta_r4088_r4133.md`. Spec = the Delphi r4088→r4133 diff (`.inputs/electricdss-code-
r4133-trunk`); oracle = `oddie:r4133`. Gate green (fmt/clippy/`cargo test --workspace`).

- **SwtControl D6 (`elements/control/swt_control/`).** The deprecated `Action` (prop 3)
  now sets the ACTUAL state — like `State` (prop 7) — instead of only the normal state
  (r4088/0.14.5 bug), and fires the "normal defaults to state on first set" side effect
  (Edit supplemental `case 3, 7`). Ported by making `side_effects(ACTION)` mirror
  `STATE`: force `present_state`, default `normal` on first set, push the `SetSwitchClosed`
  RefAction (guarded by `Locked`). Probed on r4133: `action=open` opens the switch
  immediately (no queue/delay, empty event log), `normal` stays as declared (or defaults
  to the action value when Action/State is the first setter), `lock=yes` still ignores it.
- **RatedCurrent (prop 9, C4).** New informational continuous rating (default 0.0; "Not
  used internally for either power flow or reporting"). Parse-accept + store; ordinals
  shift (BaseFreq 9→10, Enabled 10→11, NUM_PROPS 11→12). r4133-only ⇒ hidden from the
  0.14.5-pinned full-enumeration surfaces via the new **`PropFlags::HIDE_R4133`** (sibling
  of `HIDE_015X`; Dump / `Dump commands` / JSON skip it, `?`/props-table still expose it)
  + a **`PROPS_015X`** allowlist row (excludes it from the shape walk on the 0.14.5 AND
  capi015 captures). Added `hidden_from_full_enum()` helper; the four Dump/JSON check
  sites now test both flags.
- **`batchedit … where` conditionals + E1 (`exec/batchedit.rs`, new module).** Faithful
  port of r4133 `DoCheckConditionals`/`DoEvalConditionals`/`DoLocalizeOp_Index`: filter
  regex matches by `>,<,>=,<=,=,!=` combined with `and/or/xor` before editing. The Delphi
  tokenizer quirks are reproduced (probed on r4133): whole clause lowercased; logic op
  chosen by list order `and`>`or`>`xor` (first text anywhere wins → `and` beats an earlier
  `or` and absorbs the rest into the value; any `xor` matches `or` first → behaves as
  `or`); missing property → empty value (numeric 0) → silently false, no abort. `=`/`!=`
  are string compares against the (lowercased) LHS, `>`/`<`/… numeric via `AuxParser`
  DblValue. E1: **every** batchedit now sets `GlobalResult := 'Elements edited: N'`
  (ExecCommands cmd 95), with or without `where`; the without-`where` model effect is
  unchanged (default-oracle decks stay green — they compare the model, not the result
  string). Property resolution is exact case-insensitive (Delphi `PropertyIndex`), not
  abbreviated.
- **TCC `none` + DoNewCmd abort + AllocateLoad — VERIFIED, no port.** (1) TCC_Curve `none`
  reserved-name rejection + the `DoNewCmd` `if Result=0 then Exit` abort: already ported
  (`exec/command.rs` `add_object`, returns before `edit_active`/`dss_objs.push`), tested
  by `tcc_curve_none_is_reserved` (proves NO phantom object created); matches r4133 (probed:
  `new tcc_curve.none` never resolves). (2) AllocateLoad D5 disabled-meter/sensor skip:
  already ported at the loop level (`solution/meters/sampling/allocate.rs`, `if !enabled()
  continue` for CalcAllocationFactors AND AllocateLoad), tested by
  `allocateloads_ignores_disabled_meter` (WP-U1.5 D9, r4115 == r4133 form).
- **Decks / goldens.** New `tests/corpus/modes/batchedit/batchedit_where.dss` (`oracle:
  r4133`, §1.7 two-process determinant): pins the where-filtered edit sets via the live
  r4133 model + per-load kw probes; the `Elements edited: N` token is pinned by the
  `batchedit_where_conditionals_match_r4133` exec_tail unit test (8 probed cases incl. the
  and-first quirk). Flipped `swtcontrol_time.dss` + `midi_swtcontrol.dss` capi015→r4133
  (D6 makes `action=open` open at parse; the capi015 `Closed×3→Open` trajectory was exactly
  the r4088→r4133 move) — render-form probes (state/normal `[open,..]` arrays = E3/U2.5
  scope; Delphi drops prop-5 Delay so `delay` renders 120) dropped, whole-model + empty
  eventlog/ctrlqueue pin the physics. `swtcontrol_lock.dss` stays default-oracle: locked ⇒
  Action ignored ⇒ switch unchanged, but the deeper reason it CANNOT move to r4133 is a
  latent Sample divergence OUTSIDE the r4088→r4133 delta — Delphi (both r4088 and r4133)
  comments out the ENTIRE `TSwtControlObj.Sample` body ("action/lock are instantaneous"),
  whereas our FPC 0.14.5 port still pushes `CTRL_LOCK` onto the control queue; the deck's
  `compare_ctrlqueue` pins that FPC push, so it can only stay on capi015 (documented at the
  `sample()` doc-comment; closing this gap = retiring the Sample body, deferred). **civanlar.dss (vendored corpus) flipped capi015→r4133**:
  its `edit action=o` on the 3 tie switches now opens them at parse (D6), converging in 2
  iters to the open-tie topology — PROVEN bit-identical to r4133 (node0 V (13261.309423,
  -34.747502) both); capi015's Rung-1 control-loop path took 5 iters to a
  physically-equivalent-but-not-bit-identical point (8e-4 above the feeder tier floor — a
  solve-PATH difference, not conditioning). Regenerated `swt_manual.json` protection golden
  on `oddie:r4133` (empty event log; `gen_protection.py` routes it via `ODDIE_SCENARIOS`,
  merged with the WP-U2.1 oddie route). `props/swtcontrol.json`: removed the D6-superseded `swtcontrol_action_open`
  scenario (r4133 forces State=open, not capi015-pinnable) and re-based `swtcontrol_makelike`
  on `state=open` (State=open, the oracle-derived value shared with `swtcontrol_state_open`).
  `population.lock` regenerated (modes 68→69; civanlar oracle). `known_diffs.json` unchanged:
  no swtcontrol-BEHAVIOR entry exists that this delta kills (the `property-format-brackets`
  row is the E3 array render = WP-U2.5 scope). New SwtControl unit tests: `d6_*` (3) +
  `rated_current_parses_and_reads_back`; `action_open_opens_switched_line` rewritten to
  pin the D6 immediate-force (no OPENED event); `batchedit_where_conditionals_match_r4133`.

**WP-U2.3 — Relay per-phase rewrite (r4133, 2026-07-17, branch wt-u23).** Ported
`Controls/Relay.pas` r4088→r4133 (the delta's largest unit: 1403+/808−) into
`elements/control/relay/` (mod/logic/accessors), mirroring the landed recloser
per-phase machinery but from Relay's own Pascal. Delivered B1 (per-phase
`StateArray` for `type=current`: `SinglePhTrip`/`SinglePhLockout`, per-phase
TCC eval of `cBuffer^[i+CondOffset]`, phase index on the control-queue proxy,
`MaxOperatingCount` curve selection, ≥1-phase sampling gate; `IdxMultiPh`
ganged slot drives all non-overcurrent sub-types — those changed *only* by the
`^[IdxMultiPh]` indexing, verified by the r4088↔r4133 per-method diff), B2
(`VoltageLogic` OV/UV over `Vmax_closed`/`Vmin_closed`, reclose still all-phase),
B4 (sample continues while ≥1 phase closed), D3 (inst time bare `0.01`, delay
added once at push), D4 (queued `CTRL_RESET` only resets `OperationCount` for
closed phases — no full `Reset`, no element force), C1 (props **50→71** with 15
deprecated aliases sharing fields + `Normal`/`State` per-phase arrays +
`Lock`/`Reset` actions + `RatedCurrent`/`InterruptingRating`), D7 (first-`State`
side effect defaults `Normal` per phase), E2/E3 (per-phase event wording,
descriptive targets `Gnd Curve + Ph Curve`/`Ph Instantaneous`/…, separate
Phase/Ground Target lines dropped). Two upstream bugs reproduced with
`TODO(compat)`: the **unconditional** `Debug Sample: Relay.<name> FPresentState:
[…]` line on every `Sample` (r4133 forgot the `DebugTrace` guard the recloser
has), and reset events logged as `Recloser.<name>` (copy-paste); both
oracle-verified (oddie:r4133 emits them). **Empirical source-vs-binary
resolution (RUNG2 oracle-authoritative):** the r4133 *source* Edit CASE 5 still
writes `'[5.0]'`/`'[0.5,2,2]'` reclose defaults for voltage/current, but the
r4133 *binary* applies neither — every non-DOC type keeps the constructor
`(0.5,2,2)`/Shots 4 (probed current/voltage/46/47/distance/td21); only DOC forces
`NumReclose 0`. `type_side_effect` matches the binary. Dispatch: Relay `Action`
op now carries the phase proxy. Harness: Relay added to the
`compare_all_properties` skip list (71-prop table can't match 0.14.5, as
Recloser). `relay.json` props golden regenerated (74 props, 10 scenarios incl. a
single-phase-trip scenario), Rust r4133 renders cross-validated against
oddie:r4133 (only report-format diffs remain: `SwitchedObj` empty-echo, and
`reset=20` legacy-collision hitting the new Reset action while `ResetTime`
correctly stays 15). Retired `relay_current` 0.14.5 protection golden (event-log
behavior moved off 0.14.5, as the recloser scenarios). **Decks:** all 9 relay
controls decks (`relay_{oc_sym,4647_asym,voltage,revpower,generic,distance,
td21,doc}` + `midi_relay_4647`) flipped to `oracle:"r4133"` — event log (incl.
the Debug Sample lines) + ctrlqueue + probes match oddie:r4133; `relay_generic`
`delay` 0→0.1 (a delay=0 generic trip fires in-step and oscillates the control
loop, #485 on r4133; 0.1 queues it, both converge). Vendored r4088→r4133
witnesses `Test/{Distance,TD21,Reverse*}RelayTest` + Version8 twins (8 decks)
flipped to r4133. `combo/{combo,midi}_protection` kept on 0.14.5 with the
now-array Relay `state`/`normal` probes dropped (can't compare vs scalar 0.14.5;
`check_meters_monitors` blocks an r4133 flip on the Oddie monitor-header format)
— `delay` probe + ctrlqueue + meters retained. `population.lock` regenerated
(solvable_now 293→292). **Deferred follow-ups:** (1) `59NRelayDemo` (a
`type=voltage` relay across an open point) excluded from `solvable_now` — my
correct B2 + dropped-voltage-reclose move it off 0.14.5, but a ~7e-4 residual vs
oddie:r4133 on this open-point *dynamics* topology needs decomposition triage
(`relay_voltage` passes on r4133, so the standard voltage relay is correct); (2)
the `known_diffs` `eventlog-trailing-space` row (r3723/r4088/r4133) is untouched
— retiring its r4133 portion needs a proving partition re-run, deferred to
WP-U2.6's sweep; (3) new SinglePhTrip/partial-open-voltage synthetic *corpus*
deck families (the brief's synthesize list) not added — single-phase machinery
is covered by inline unit tests (`single_phase_trip_arms_only_faulted_phase`,
`single_phase_do_open_opens_only_that_phase`,
`single_phase_lockout_escalates_to_3ph`) + the props golden, but a live
`oracle:"r4133"` family deck is still owed; (4) the `help_catalog.rs` relay
entries + the `dump3_commands` golden's `[Relay]` block still carry 0.14.5 help
text (props 50→71 renamed) — masked out of the dump golden for now (as
`[WindGen]`), the r4133 relay help-catalog/dump-surface regeneration is WP-U2.5
(protection report/log surface) scope (the property NAMES/values are already
gated by `relay.json` + the live r4133 decks + the `compare_all_properties`
skip). Full mandatory gate green; 57 relay inline unit tests pass.

**WP-U2.3 audit-fix pass (2026-07-17, wt-u23).** (1) *Major — `Normal`/`State`/
`Action` discrete-state parse.* The r4133 surface reused the 0.14.5 `trip`→open
enum alias, so `normal=trip` rendered `[open,open,open]` while oddie:r4133 leaves
it `[closed,closed,closed]` (`InterpretRelayState`, Relay.pas:1237 — first char
`o`/`c` only, no else arm ⇒ non-o/c leaves the slot unchanged). Reworked the
relay-only `relay_state`/`relay_action` enums to reproduce it exactly:
`allow_longer` + `max_chars=1` (leading-char match: `openZ`→open, `cs`→closed)
and `default_value = CTRL_STATE_KEEP` (new sentinel; unmatched ⇒ keep the phase,
no parse error), with the state-array setter/`do_action` skipping KEEP slots.
Empirically re-verified on oddie:r4133 (`trip`/`xyz`→unchanged, `openZ`→open,
`[open trip closed]`→`[open,closed,closed]`); recloser/fuse enums untouched
(their own defs). Fixed the `relay_normal_trip` props golden (`Normal` open→closed)
and added an inline pin `state_parse_is_first_char_only_r4133`. (2) *Minor —
props-golden note.* `relay.json` is a self-referential regression pin (Rust
renders compared back to Rust), not an independent oracle gate; softened the
`oracle.note` to say so and to scope the r4133 cross-validation as a manual
(non-CI) spot check — genuine oracle coverage is the live `oracle:"r4133"` relay
decks. (3) *Minor — `59NRelayDemo`* stays parked in
`skipped_needs_investigation` (tag `relay_voltage_dynamics_residual`) with its
decomposition plan; the ~7e-4 open-point residual is un-triaged (suspected bug
until proven a floor per CLAUDE.md) and the live-f64 trip-time + per-node
trajectory decomposition is owed to WP-U2.6's rung-exit sweep — an honest
deferral, not tolerance-masked. Full mandatory gate re-run green.

**WP-U2.5 — Protection report/log surface (r4133, 2026-07-17, branch wt-u25).**
E-bucket of `delta_r4088_r4133.md` + the WP-U2.3-deferred (commit fb6db95)
relay help-catalog/dump surface. Oracle = oddie:r4133.

- **`Dump commands` help-catalog surface → full r4133 for all four protection
  classes.** The `help_catalog.rs` help text (rendered by `Dump commands`) was
  0.14.5 for the renamed/new Relay/Fuse/SwtControl props. Added
  `tools/golden/r4133_help.py` — a generator supplement holding the r4133
  protection help, captured **verbatim from the oddie:r4133 binary's own `Dump
  commands`** (its `PropertyHelp` arrays = `Version8/Source/Controls/{Relay,
  Recloser,fuse,SwtControl}.pas`). `gen_help_catalog.py` applies it over the
  0.14.5 wheel parse. The supplement ALSO carries the recloser r4133 help that
  WP-U2.2 had **hand-edited** into the "GENERATED" file (+ the earlier
  CNData.SemiconLayer / LineSpacing-0.15.x / A-Diakoptics tear_circuit non-wheel
  edits), so `python gen_help_catalog.py` reproduces the committed file exactly
  instead of silently reverting them (verified: only Relay/Fuse/SwtControl
  runtime values changed vs HEAD; Recloser render byte-identical).
- **`[Relay]` unmasked** from the `dump3_commands` golden (was masked as
  `[WindGen]` since fb6db95); `[Fuse]`/`[SwtControl]` brought to their full
  r4133 12/9-prop shapes by dropping `HIDE_015X`/`HIDE_R4133` from
  CurveMultiplier/InterruptingRating/RatedCurrent (matching Recloser, which
  never carried a HIDE flag). Empirically only `dump3_commands` pinned these —
  no element-Dump or JSON-export golden does — so the un-hide is clean;
  `compare_all_properties` still excludes them from the 0.14.5 property walk via
  the name-based `PROPS_015X` rows (independent of the HIDE flag: the `?`-surface
  always shows them). The four blocks are **self-referential regression pins**
  (our render vs our render; the r4133 property NAMES/VALUES are gated live vs
  oddie:r4133 by the `oracle:"r4133"` controls decks + `props/*.json`).
- **Property-value render `[closed, closed, closed, ]`** — verified the shared
  `MappedStringEnumArray` render already backs Relay/Recloser/Fuse/SwtControl
  Normal/State (nothing to port); Fuse GetPropertyValue special cases 5/6/12
  (FuseCurve→`none`, RatedCurrent/InterruptingRating `%-.6g`) already match r4133.
- **`known_diffs` `property-format-brackets` — NOT retired (empirically still
  live on r4133).** Probed oddie:r4133: EPRI renders `sensor.currents` PLAIN
  (`100,90,80`) while our dss_capi-based port brackets numeric arrays
  (`[ 100 90 80]`, `util.get_dss_array_f64`) — the dss_capi property-system
  rework difference persists on r4133. The row's E3 *protection-state-array*
  portion IS resolved (our `[closed,..]` render now matches EPRI r4133's own
  bracketed `GetPropertyValue` state render, so the row no longer fires for
  those), but the generic numeric-array class stands — a DIVERGENCES ledger item
  owned by WP-U2.6's r4133 ASSERT sweep. Retiring the whole row would re-expose
  it as an uncataloged sweep failure; kept, documented here (deviates from the
  literal U2.4 "retire in U2.5" note, which pre-dated this probe).
- **Event-log §1.3-3.** `compare_eventlog` was already `true` on every non-combo
  protection deck (relay_*/recloser_*/fuse_*/swtcontrol_* + midi twins) — nothing
  to re-enable; the two combo decks (owned by a parallel WP) are untouched.
  `EVENTLOG_MASKS` stays **EMPTY** (the port emits r4133 wording 1:1).
- **Save round-trip.** New `save_roundtrip_protection` (crates/dss-core/tests/
  save_roundtrip.rs): a Relay+Recloser+Fuse+SwtControl deck carrying r4133-surface
  values (PhCurve/PhFastCurve/CurveMultiplier, SinglePhTrip/Lock/RatedCurrent/
  InterruptingRating) `Save circuit`→`clear`→re-compile→re-solve; every control's
  full property list (via `element_properties`) round-trips (numeric-token) +
  node V ≤1e-6. Pins that Save does not drop/corrupt the renamed/new/array surface.
- **§E4 help-only edits.** LineSpacing 7-10 already r4133-aligned (the U1.4
  equivalent-spacing help, "geometric mean distance", matches the r4133 binary
  verbatim — verified). Line prop 20 (units-reset warning) + AutoTrans
  normamps/emergamps "(Read only)" left at 0.14.5: those surfaces are NOT
  r4133-ported (their `dump3_commands` blocks stay byte-exact 0.14.5), so per the
  brief's "where our surface claims r4133 text" qualifier they are out of scope.
- **Pre-existing environmental fix (not WP-U2.5 behavior).** The mandatory gate
  was RED on this machine's base `update` branch (proven by re-running with my
  changes stashed): the official r4133 Oddie DLL prefixes its `export eventlog`
  CSV (and some `Text.Result`) with a UTF-8 BOM (`﻿`), which the oracle capture
  read as plain utf-8 → a lone `['﻿']` line for an inert deck's empty log
  (`recloser_temp`) and a char-boundary panic in `numeric_skeleton` on a
  corpus_live property. Fixed at the source (`capture_eventlog` → `utf-8-sig`) +
  defensively in `numeric_skeleton` (strip a *leading* `﻿`, spurious export
  cruft, never real data). Not a tolerance/divergence change.

**WP-U2.5 audit fixes (2026-07-17, wt-u25).** Four minor findings addressed:
- `save_roundtrip_protection` now forces DISTINCT mixed per-phase arrays on the
  relay (`normal=(closed open closed) state=(open closed closed)`) so the Save
  round-trip covers serialization + re-parse of a non-default `[open, closed,
  closed, ]` array (and an actual locked-out open phase) — not just the all-closed
  default. Probed: both round-trip exactly (dV=0, iter 2→2).
- `gen_help_catalog.py` comment corrected (it wrongly said Recloser was NOT in the
  supplement; `r4133_help.py` carries the full Recloser block, folded in at U2.5).
  Re-verified: `gen_help_catalog.py` + `cargo fmt` reproduces the committed
  `help_catalog.rs` byte-for-byte.
- `PropFlags::HIDE_R4133` doc now records it has NO live application site after U2.5
  (retained as infrastructure like `HIDE_015X`); the stale harness comment that
  cited the flag for the name-based `PROPS_015X` SwtControl row is corrected.
- dump3 `[Relay]/[Fuse]/[SwtControl]` self-referential circular-derivation +
  save-roundtrip self-consistency are already documented at their sites (no code
  change) — surfaced as sanctioned by UPGRADE_PLAN §1.3-2 / WP8.5.

**WP-U2.1 handoff — combo fuse-save restore (r4133, 2026-07-17, branch wt-combo).**
The WP-U2.1 deferral (STATUS `Standing open follow-ups`, UPGRADE_PLAN WP-U2.1 handoff):
now that Fuse (U2.1) / Recloser (U2.2) / Relay (U2.3) all landed on `update`, the
two cross-chain fuse-save decks are version-consistent on r4133.
- **Decks re-armed.** `combo/combo_protection` + `combo/midi_protection` flipped to
  `oracle:"r4133"`; the fuse-tier neutralization (`ratedcurrent=100000/5000`, which
  existed only so the fuse never blew on 0.14.5) removed. r4133's default `FuseCurve`
  moved `tlink`→`none` (a curveless fuse never blows), so the fuse is given an
  explicit curve: `fusecurve=tlink curvemultiplier=40` (combo) / `=50` (midi) — the
  CurveMultiplier divisor reproduces the pre-WP r4088-era `RatedCurrent=40/50`
  divisor (WP-U2.1 CurveMultiplier semantics). The midi recloser also gained explicit
  `phasefast=a phasedelayed=d` (r4133 D2 removed the built-in defaults). midi is
  generated — fixed at `tools/decks/gen_midi_decks.py` (`PROTECTION_EXTRA`) and
  regenerated.
- **Race re-exercised end-to-end** (probe-verified on oddie:r4133): recloser FAST
  shot clears the fault, on reclose the fault persists and the fuse melts the faulted
  phase on the delayed cycle (`Fuse.fz PHASE 1 BLOWN` at Sec=0.5 combo / 0.9 midi),
  the recloser recloses and restores service except the blown phase, and the
  substation backup relay never trips. Both decks converge every step; two-process
  determinant bit-identical (§1.7).
- **Probes/eventlog restored** (dropped by WP-U2.2/U2.3 while the tiers were
  version-split): `Recloser.r` (`state`/`normal` [+`shots`/`numfast` on midi]),
  `Fuse.fz` (`state`/`normal`/`fusecurve`/`curvemultiplier`), `Relay.backup`
  (`state`/`normal` [+`delay` on midi]), and `compare_eventlog:true`. The r4133 relay
  `Debug Sample: FPresentState` lines + the fuse `PHASE n BLOWN` line reproduce the
  oracle (the Rust port's TODO(compat) relay lines + fuse blow). controls live gate
  **96→98** decks matched.
- **Pre-existing infra bug fixed** (`tools/oracle/oracle_server.py::capture_eventlog`,
  NOT combo-scoped but the enabling infra): Delphi's `export eventlog` writes a UTF-8
  BOM, so an EMPTY Oddie event log read back as `['﻿']` (one lone-BOM line) and a
  non-empty first record was BOM-glued. This made the length assert (0 vs 1) fire on
  every empty r4133 event-log step and the numeric-skeleton comparator panic on the
  multi-byte BOM — pristine `update` was RED here on `recloser_temp` step 0 (proven by
  running the pristine controls gate). Fix strips the BOM per line: empty→`[]`, line-0
  matches the BOM-free Rust `event_log()` / capi `Solution.EventLog`. Restores the
  whole r4133 event-log channel (recloser/relay decks too), not just the combo pair.
- **Meter/monitor compare RE-ENABLED** on these two decks (audit fix 2026-07-17;
  `check_meters_monitors: true`). The blocker was the Oddie monitor CSV `Header`:
  Delphi renders it with a leading space after each comma + a trailing comma, so
  `Monitors.Header` reads back `['V1',' VAngle1',…,'']` (leading-space columns +
  trailing empty) which the harness `compare_monitor` exact-header assert could not
  match against the clean dss_capi/Rust `['V1','VAngle1',…]`. Fixed by normalizing
  the header in `compare_monitor` (trim each column + drop trailing whitespace-only
  columns) before the equality assert — a test-comparator normalization that does
  not weaken the check (channel COUNT + every channel's samples are still asserted).
  **Audit fix (2026-07-17, wt-combo).** `compare_monitor` is shared with the strict
  golden path (`golden_metering_monitors.rs`/`golden_ieee8500.rs`/`scenario.rs`), so
  the normalization is now applied to the **expected side only**, keeping the Rust
  `view.header` strict: `monitor_view().header` is built structurally in
  `monitor/header.rs` (`push("V1")`/`push(format!("P{i}W{j}"))`, no CSV round-trip)
  → clean by construction, so a genuine Rust-side header defect (leading space /
  phantom trailing column) still fails, exactly as before combo restore. The two
  oracle-capture normalizations (`_lst` trailing-empty drop, `capture_eventlog` BOM
  strip) only ever remove empty/whitespace/BOM content that is never a real element
  name or event record, so they cannot equalize distinct values. `controls_cases_match_oracle`
  green (26 cases, oddie:r4133).
  A second Oddie array artifact surfaced under the re-enabled compare and was fixed
  the same way: the Oddie `ZonePCE`/`AllBranchesInZone`/`AllEndElements` string
  arrays carry a trailing empty element (`['load.a',…,'']`), so `capture_all_meters`'s
  `_lst` helper (which already dropped the `['NONE']` placeholder) now also strips
  empty/whitespace-only entries — else midi's zone PCE set read 33 (32 real + phantom
  empty) vs Rust's 32.
  Empirically verified vs oddie:r4133: EnergyMeter registers match (duty mode does
  not integrate → registers 0 / Max drag-hands -1e50, the identical `-1.0e50`
  sentinel on both engines) with the zone branch/end/PCE membership compared as a
  set, and the mode-0 V/I monitor channels match the already-pinned full-model
  trajectory. The product-side header normalization (what the Rust engine *emits*
  — the WP-U1.5 E1 `monitor-header-whitespace` known_diff) is unrelated and stays
  scoped there.
- `population.lock` unchanged: the two combo decks are controls-family cases (the
  lock fingerprints per-case rigor flags only for `solvable_now.json`; the synthetic
  families track counts + path lists, both unchanged here).
  `known_diffs.json` untouched (no combo-scoped entry).

**WP-U2.6 — 59NRelayDemo decomposition (2026-07-17, branch wt-59n).** Owed at the
Rung-2 exit. Reproduced the deck's ~7e-4 residual vs oddie:r4133 and found the
**first diverging quantity** by a per-step trajectory on both engines (Rust
instrumentation vs an Oddie r4133 probe reading live-f64 `YNodeVarray` +
`AllVariableValues`, not the f32 monitor). **PORT BUG FOUND + FIXED:** the relay
`state_size()` sized the per-phase state arrays + `VoltageLogic` loop by the
relay's OWN Nphases, but Pascal forces `Nphases := MonitoredElement.NPhases`
(RecalcElementData:906) — for this deck the 1-phase broken-delta PT — while every
state-array path (`VoltageLogic` Relay.pas:2852, `GetPropertyValue` 39/40, Sample,
Reset) iterates `Min(RELAYCONTROLMAXDIM, ControlledElement.NPhases)` = the 3-phase
switched Line1. The relay's own count feeds only `vbase`/`cBuffer`/`CondOffset`
(read separately via `ccd.cd.nphases`/`mon_offset`), so the two counts coincide
for every existing relay deck (mon==ctrl phases) and this is the only deck that
distinguishes them. With the wrong count=1 the VoltageLogic loop read only
`cBuffer[1]` (the 3V0 ≈438 V) leaving `Vmag`>0, so the relay tripped Line1
(438/277=1.58 pu ≫ 0.3 pu pickup); count=3 makes the loop's final *phantom* phase
(beyond the PT terminal's 2 conds) read 0 → `Vmag`=0 → the `IF Vmag>0` guard fails
→ `OVTime`=-1 → no trip, matching oddie:r4133 (state `[closed,closed,closed]` all
10 steps, iteration counts identical every step). Fix: `state_size` → `ctrl_snap`
(controlled element) nphases; regression-guarded by the new relay unit test
`voltage_relay_open_point_sizes_state_by_controlled_nphases_59n` (a 1-ph-monitored
/ 3-ph-switched voltage relay must not trip on the open-point 3V0). **Residual
after the fix = proven chaotic-dynamics floor, NOT gateable:** the un-tripped
generator (D=1) pole-slips under the sustained fault on BOTH engines (θ 12→777°,
f 60→79 Hz by t=1.0). Rust then matches oddie:r4133 to the solver floor with
identical iteration counts, and the generator live-f64 state matches to machine-eps
early (dθ 7e-15, df 1.4e-14 at t=0.1); the pole-slip's positive Lyapunov exponent
amplifies the faer-vs-KLU last-ulp difference exponentially (~2×/step): node-V Linf
4.5e-6 (t=1.1) → 7.8e-3 (t=1.9) → O(1 V) by t≈2.5, unbounded. The harness
checkpoint (deck runs `number=10` inline to t=1.0 during compile, then n_steps≥1
solves → t≥2.0) necessarily lands in this chaotic regime, where no fixed tolerance
honestly bounds the node-V (the t=2.0 value 1.9e-5 is a deterministic chaotic dip,
not a floor) — masking it with a tolerance is exactly what CLAUDE.md forbids. So
the deck STAYS in `skipped_needs_investigation`, re-tagged
`relay_voltage_dynamics_chaos_floor` with the full decomposition; the fix itself is
proven vs oddie:r4133 and unit-gated. Mandatory gate green.

*Audit fixes (2026-07-17, wt-59n).* (1) The same phase-count port bug lived in
`TRelayObj.MakeLike` (accessors.rs): the per-phase `FPresentState`/`FNormalState`
copy loop was bounded by the source relay's OWN Nphases (= MonitoredElement.NPhases)
instead of `Min(RELAYCONTROLMAXDIM, ControlledElement.Nphases)` (Relay.pas:683). For
an asymmetric `like=` source (mon != ctrl phases) an OPEN state latched on a high
phase was dropped (left CTRL_CLOSE). Fixed to reuse `state_size()` (ctrl_snap is
already copied from the source before the loop, so the count matches Pascal);
regression-guarded by `make_like_copies_state_by_controlled_nphases`. (2) The
chaos-floor classification (finding: proof is prose from the non-gating Oddie r4133
channel, not a checked-in artifact) stays as documented — the `state_size` fix is
independently proven (Pascal citations + full live gate green + unit test) and the
chaotic pole-slip residual is legitimately NOT tolerance-maskable per CLAUDE.md, so
continued parking is the correct (and only honest) call. No code change. Mandatory
gate re-run green.

**WP-U2.5 audit fixes, round 2 (2026-07-17, wt-u25).** Three minor findings:
- `numeric_skeleton` BOM strip **narrowed to leading-only** (`strip_prefix('﻿')`,
  was a whole-string `replace`). The whole-string strip was broader than the
  actual Oddie failure mode (a *leading* export BOM that mid-slices the ASCII
  loop) and would have silently swallowed a spurious interior BOM on one side.
  The else-branch now advances char-wise, so an interior BOM flows into the
  skeleton and surfaces as a structure mismatch (flagged, no panic) — the primary
  fix (oracle_server `capture_eventlog` → `utf-8-sig`, scoped to the IOddieDSS
  path) is unchanged. New `comparator_tests::bom_strip_is_leading_only` pins both
  halves (leading stripped ⇒ match; interior kept ⇒ flagged).
- dump3 protection help-blocks are a self-referential pin (our-render → golden →
  our-render) with no automated oracle gate on the help TEXT: **sanctioned** —
  UPGRADE_PLAN §1.3-2 deliberately has no byte-exact-vs-Delphi help gate; the
  `r4133_help.py` transcription was independently re-verified verbatim against the
  vendored r4133 Pascal (Fuse/SwtControl/Relay `PropertyHelp`). No code change.
- `known_diffs` `property-format-brackets` mask: **retained by design** (see the
  WP-U2.5 note above — the generic numeric-array class is empirically still live
  on oddie:r4133). It lives ONLY in the opt-in, report-only EPRI A/B channel
  (never gates commits), `reason_contains` requires ALL substrings
  (`["probe ","structure differs"]`, `.all()` at corpus_live.rs), and zero-hit
  entries self-report — so a stale mask surfaces. Closure is owned by WP-U2.6's
  r4133 ASSERT sweep, not U2.5.

*Audit fixes, round 2 (2026-07-17, wt-59n).* Follow-up findings all flagged the
same confidence-inflation risk: the post-fix `chaos_floor` classification and the
r4133 no-trip outcome rested on non-checked-in prose from the opt-in Oddie r4133
channel, verifiable only by manual eyeballing. Addressed by turning the prose into
a **checked-in, re-runnable oracle read**, `tools/opendss/probe_59n.py`, and
confirming both claims empirically on the r4133 engine: (A) the engine's OWN
`Relay.State` property reads `[closed, closed, closed, ]` — byte-identical to the
port's `render_state_array()` and the unit test's asserted `no_trip` — with
Line.line1 still at ~1381 A, so the encoded outcome reflects oddie:r4133, not a
self-pinned value (the unit-test doc comment now cites this); (B) the generator
frequency leaves 60 Hz (78.88 Hz at t=1.0) and wanders unboundedly over 67–115 Hz
across the next 15 s = pole-slip, so no fixed node-V tolerance honestly bounds the
faer-vs-KLU residual (the parking is correct, not a masked bug). The probe exits 0
iff r4133 reads all-closed. Manifest note + unit-test doc updated to point at the
probe; the residual stays parked (unchanged), now backed by a regenerable artifact
rather than eyeball-only prose. No engine code changed; mandatory gate re-run green.

**GAPS (WPG.*), Phase 8, Phase 7.** The per-WP GAPS_PLAN records (WPG.1/10/12/13/
14/15/16/17/18/19/20/21 + CIM XML export stages) are archived in
**`docs/phase-records/gaps.md`**. Phase 8 (reporting/executive) is COMPLETE — detail
in **`docs/phase-records/phase-8.md`**. Phase 7 (DER/protection/line-constants/
harmonics/dynamics) is COMPLETE on `phase-7-extended-elements` (not merged to `main`)
— roll-up in §1e and **`docs/phase-records/phase-7.md`**.

**Prior — UPGRADE Rung 2 EXITED — WP-U2.6 the 11.0.0.1 (r4133)
parity claim (branch wt-u26).** The opt-in EPRI sweep
`DSS_LIVE_OPENDSS=r4133 DSS_LIVE_OPENDSS_ASSERT=1` is **GREEN** (326 matched, 70
known-diverged, 4 known-skipped, **0 NEW** of 400; 103 target-rev `oracle`-flipped
cases excluded — gated in the mandatory gate). Every surviving Rust↔r4133
divergence is a documented `known_diffs.json` class: FPC↔Delphi last-ulp /
display-precision floors, dss_capi's bracketed numeric-array PropertyValue render
(`property-format-brackets` ×10 — the WP-U2.5-deferred numeric-array class,
**closed** here), EPRI's InvControl event-log trailing space
(`eventlog-trailing-space` ×6 — the WP-U2.3-deferred class, **keeps** its r4133 tag,
still witnessed), or the four EPRI-DLL #303 crash decks. The **58** raw NEW
divergences were all dispositioned (mandatory gate green ⇒ port == pinned 0.14.5 ⇒
the r4133 gap is purely FPC↔Delphi, never a Rung-2 regression): 48 cases (44 diff
+ 4 skip) extend an existing r4088 floor/skip entry on a **behaviorally-identical
r4088=r4133 path** (source-verified; 14 entries), 10 cases → 3 new entries
(`storage-pctstored-display-precision`, `monitor-seq-magnitude-drift`,
`harmonics-ieee519-r4133`); separately 2 entries narrowed to r3723
(`monitor-header-whitespace` now handled by the harness header-normalization,
`meter-zonepce-count` decks now match). **Direction check** — `r4088` re-run green
(329 matched, 0 NEW after the same %stored/monitor_seqmag cataloging); the only
sweep-set difference vs r4133 is the r4133-only IEEE_519 harmonics move + the
r4088-only harmonics-Y witness = exactly the r4088→r4133 delta this rung owns. The
**IEEE_519 harmonics surprise is source-confirmed "nothing to port"**
(SolutionAlgs/Load/Spectrum/YMatrix byte-identical r4088=r4133; Solution.pas diff =
progress-form + commented debug only) — a determinism-proven build-drift amplified
by the 519-filter near-resonance, cataloged `harmonics-ieee519-r4133`; the
InductionMachine converged-flip was already resolved by WP-U2.1. `DIVERGENCES.md`
+ `known_diffs_burndown.md` carry the full Rung-2 exit record; `PLAN_SEQUENCE.md`
marks UPGRADE COMPLETE; new root `README.md` states the parity claim. **Engine
behavior = OpenDSS 11.0.0.1 (r4133) except the documented ledger.** `rg
"NOT_PORTED\(U2"` empty; zero `pending` upgrade decks. Mandatory gate (fmt +
clippy + `cargo test --workspace`) green.

**Audit fixes (post-merge, wt-u26).** Wording-accuracy pass on the Rung-2 ledger
after re-`cmp`ing the vendored r4088/r4133 Version8 trees: the summary phrase
"byte-identical r4088=r4133" is literally false for the **solver**
(`Common/Solution.pas` differs — progress-form/GUI plumbing + a commented-out debug
`WriteLn`, numerically inert) and `PDElements/AutoTrans.pas` (two read-only
PropertyHelp strings). All non-`Solution.pas`/`AutoTrans.pas` units named in these
entries (`PCElements/`, `Meters/Monitor.pas`, `SolutionAlgs.pas`, `YMatrix.pas`,
`ReduceAlgs.pas`, injection/reduction/ckt24 feeder) ARE byte-identical, so the
behavioral conclusion (no algorithm changed) stands. Reworded the overstated
claims to "behaviorally identical (only progress-form/PropertyHelp text differs)"
in `known_diffs.json` (iteration-count-delta, ckt24-regcontrol-conditioning),
`DIVERGENCES.md`, `known_diffs_burndown.md`, and this record. Also: documented that
the `monitor-header-whitespace` r3723 tag is likely already dead (the
`compare_monitor` header normalization is rev-independent — a future r3723 re-sweep
prunes it), and scope-noted `harmonics-ieee519-r4133`'s deliberately-broad match
(mirrors sibling floor entries; re-triage a materially different IEEE_519 move). No
code/behavior change; mandatory gate re-run green.

**Prior — WP-U2.5 (protection report/log surface) + WP-U2.6
(59NRelayDemo decomposition) MERGED.** WP-U2.6 found + fixed a relay port bug:
`state_size()` / `MakeLike` sized the per-phase state arrays by the relay's OWN
Nphases, but the state-array paths iterate `Min(RELAYCONTROLMAXDIM,
ControlledElement.NPhases)` (Pascal Relay.pas) — the two counts diverge only on a
1-ph-monitored / 3-ph-switched voltage relay (59NRelayDemo), where count=1 made
the VoltageLogic loop read the 3V0 open-point residual and spuriously trip;
count=3 (controlled) reads the phantom phase → 0 → no trip, matching oddie:r4133.
Unit-gated (`voltage_relay_open_point_sizes_state_by_controlled_nphases_59n`,
`make_like_copies_state_by_controlled_nphases`); the deck itself STAYS in
`skipped_needs_investigation` (re-tagged `relay_voltage_dynamics_chaos_floor`) —
post-fix residual is a proven chaotic pole-slip floor, not tolerance-maskable.
The r4133 protection `Dump commands`
help-catalog surface is complete for all four classes: `[Relay]` unmasked from
the `dump3_commands` golden (71-prop r4133 shape), `[Fuse]`/`[SwtControl]`
brought to their full r4133 12/9-prop shapes (HIDE flags dropped — matching the
already-r4133 Recloser), all pinned self-referentially against our own render.
The r4133 help text now lives in a generator supplement (`tools/golden/
r4133_help.py`, captured verbatim from the oddie:r4133 binary's own `Dump
commands`), which ALSO folds in the recloser help that WP-U2.2 had hand-edited
into the "GENERATED" `help_catalog.rs` (+ the CNData/LineSpacing/tear_circuit
pre-r4133 edits) — so `python gen_help_catalog.py` is fully reproducible again.
Property-render `[closed, closed, closed, ]` verified shared across all four
classes (already landed). New `save_roundtrip_protection` gate: a
Relay+Recloser+Fuse+SwtControl deck round-trips its full r4133 property surface
(renamed/new props + array renders) through our own `Save circuit`→re-parse.
compare_eventlog was already enabled on every non-combo protection deck (combo
decks untouched — parallel WP owns them); EVENTLOG_MASKS stays EMPTY. Two
findings: (1) the `known_diffs` `property-format-brackets` row was NOT retired —
empirically it still masks a LIVE r4133 divergence (our `[ 100 90 80]` numeric-
array render vs EPRI's plain `100,90,80`, e.g. sensor.currents); only its E3
protection-state-array portion is resolved (our `[closed,..]` now matches EPRI
r4133's own bracketed state render) — the numeric-array class is a DIVERGENCES
ledger item owned by WP-U2.6's sweep. (2) A **pre-existing** environmental gate
failure (reproduces on pristine `update`): the r4133 Oddie DLL emits a UTF-8 BOM
in `export eventlog` / some `Text.Result` that the oracle capture didn't strip —
fixed at the source (`capture_eventlog` → `utf-8-sig`) + defensively in
`numeric_skeleton`. §E4 help-only: LineSpacing already r4133-aligned; Line prop
20 / AutoTrans "(Read only)" left 0.14.5 (those surfaces don't claim r4133).

**Also merged this session — WP-U2.1 handoff RESTORED (branch wt-combo):** the
combo fuse-save decks `combo/{combo,midi}_protection` flipped to `oracle:"r4133"`
with the fuse tier re-armed (`fusecurve=tlink curvemultiplier=40/50`, reproducing
the pre-WP r4088-era RatedCurrent divisor) and the Recloser probe + Relay
`state`/`normal` probe + `compare_eventlog` restored (dropped by WP-U2.2/U2.3
while the tiers were version-split). The classic three-tier fuse-save race
(backup relay → midline recloser → lateral fuse) now runs end-to-end on the
version-consistent r4133 chain, two-process-determinant + live-compared vs
oddie:r4133 (controls live gate 96→**98**). Also fixed a **pre-existing** oracle
capture bug uncovered here: `capture_eventlog` returned a lone `['﻿']` for
an EMPTY Oddie event log (Delphi BOM) and BOM-glued the first record — which made
`recloser_temp`/etc. r4133 event-log compares RED on this machine; now the BOM is
stripped per line so empty→`[]` and line-0 matches the BOM-free Rust log. The
`Standing open follow-ups` combo-restore entry is retired. **Audit fixes
(2026-07-17):** meter/monitor oracle compare RE-ENABLED on both decks
(`check_meters_monitors: true`) — the harness `compare_monitor` now normalizes
the Delphi monitor-CSV header artifact (leading-space + trailing-empty columns,
`['V1',' VAngle1',…,'']`) so the Oddie r4133 header compares against the clean
dss_capi/Rust header without weakening the channel-count/value asserts. Verified
against oddie:r4133: EnergyMeter registers match (duty mode → 0 / -1e50 drag-hand
sentinel, identical on both engines) and the mode-0 V/I monitor channels match
the already-pinned full-model trajectory. The product-side monitor-header
normalization (what the Rust engine *emits*) remains WP-U1.5 E1 scope; this is a
test-comparator normalization only.

**Prior frontier — Rung 2 wave 2 MERGED: WP-U2.3 (Relay r4133
per-phase rewrite) landed on `update`** via a port→audit→fix worktree chain
(wt-u23, opus-audited, major finding fixed in-branch; full mandatory gate
green, 47/47 suites). The delta's largest unit (Controls/Relay.pas
r4088→r4133) ported loop-for-loop: per-phase StateArray for type=current
(SinglePhTrip/Lockout, phase-proxy queue), VoltageLogic closed-phase OV/UV
(B2), CTRL_RESET opcount-only (D4), inst single-count (D3), props 50→71 with
15 aliases + Normal/State arrays. Two upstream bugs reproduced with
TODO(compat), oracle-verified (unconditional `Debug Sample` event line;
reset events logged as `Recloser.<name>`); the r4133 source-vs-binary
voltage/current-reclose-default divergence settled empirically for the BINARY
(oracle-authoritative). Audit fix: Normal/State/Action discrete parse is
first-char-only (`o`/`c`, else keep) per r4133 `InterpretRelayState` — the
0.14.5 `trip`→open alias dropped, re-proven on oddie:r4133. 9 relay controls
decks + 8 vendored Distance/TD21 decks flipped to `oracle:"r4133"`;
relay_current 0.14.5 golden retired; relay.json props golden regenerated (74
props, self-referential regression pin — noted as such). INFRA: the missing
Oddie r4133 venv created from vendored wheels (was blocking the whole r4133
channel). solvable_now **292/329** (59NRelayDemo → `skipped_needs_investigation`:
its ~7e-4 open-point voltage-relay residual is now **DECOMPOSED** — a real
`state_size` phase-count port bug (fixed on wt-59n) plus a proven chaotic
pole-slip floor; see the WP-U2.6 59N record below). Wave 1 (WP-U2.1
Fuse / U2.2 Recloser / U2.4 SwtControl+batchedit-where) merged earlier the
same day; Rung 1 EXITED 2026-07-16. Integration branch is `update` (pushed to
origin); main untouched until an explicit merge request. **Next: WP-U2.5
(protection report/log surface — incl. r4133 relay/recloser help-catalog +
dump3 `[Relay]` unmasking, per-phase `[closed,...]` renders, Save round-trip),
then WP-U2.6 (rung exit: r4133 ASSERT sweep + combo-deck restores +
59NRelayDemo decomposition — **59N done on wt-59n: `state_size` phase-count port
bug fixed + chaotic pole-slip floor proven; see the WP-U2.6 record**).**

**Deferred to the §6 sweep** (documented, was never rung-blocking; now also a
plan-wide exit criterion in `UPGRADE_PLAN.md` §5): JSON/Dump golden surface
flip to capi015 + dropping the Wires→"Conductors" JSON masquerade
(gen_json.py is hard-pinned to the 0.14.5 oracle; HIDE_015X retained on the
0.15.x-only Line/LineGeometry props — see DIVERGENCES §Conductors).

**PARKED TEST — RESOLVED (2026-07-16, branch wt-coverage):**
`circuit::coverage::tests::refine_bus_levels_reports_paths_on_radial` is
un-ignored and green. Root cause was the **test harness, not the port**: the
test passed its whole 8-line deck string to a single `Dss::command` call —
`command` is Pascal `ProcessCommand` (ONE command line), so everything after
`new circuit.covtest …` became extra parameters of that command (the trailing
`bus1=b5` from load ld2 landed on the Vsource; no lines were ever created).
`CalcIncMatrix_O` on that 1-bus circuit yields `Inc_Mat_Cols=["b5"]`,
`levels=[0]`: every traced path covers 0, the coverage plateau is 0, and the
state machine's sole exit (Circuit.pas:909, "changed AND >= Coverage") can then
never fire — a genuine, upstream-faithful degenerate-input nontermination fed
by a corrupted circuit. Fed line-by-line, the port terminates instantly and
bit-matches the official r3723 engine (Oddie probe 2026-07-16, both variants
< 50 µs): `set coverage=0.5` → "0 new paths detected", `Actual_Coverage =
0.666666666666667`; default 0.9 → "2 new paths detected", `Actual_Coverage =
1`. Both are now pinned in the tests (including the `get coverage` strings).
The second old hypothesis ("0.9 default unreachable, plateau 5/6") was also
disproven: `Buses_Covered` entries are bus-index SPANS whose sum overshoots
`Sys_Size` (r3723 reaches 1.0); the function's NOTE(upstream-quirk) was
corrected accordingly. See the WP-COV-PARKED record in §UPGRADE.

**AD dispositions — `off:unclassified-new-deck` bucket (2026-07-12):** at the
part2→update integration merge, every deck added after the WP-AD.4 sweep
(9 skipped-sweep promotions in `ad_sweep.json` + 63 new family decks: windgen,
U1.3 invcontrol, LINE-DEEP, coverage waves) received the explicit pending
disposition `off:unclassified-new-deck` (allowlisted in `AD_OFF_REASONS` with
the same note). This is a declared backlog, not a measured verdict — a
follow-up classification round runs DSS_AD_CLASSIFY/DSS_AD_DECOMPOSE over the
bucket and retires the reason.

## 1b–1d. Completed-phase records (archived)

The full work-package logs for the completed, merged phases (and the completed
Phase-7/Phase-8 work packages) live under `docs/phase-records/` to keep this
handoff lean. They are frozen history, superseded only by the code and tests.
The two roll-ups that back the compact §1e/§1f frontier below:
[`phase-7.md`](docs/phase-records/phase-7.md) (the WP7.1–7.10 record) and
[`phase-8.md`](docs/phase-records/phase-8.md) (the completed WP8.1/8.2/8.3-step
detail). The per-phase entries:

- **Phase 3** — the vertical-slice file-by-file map (circuit model / element base /
  solution / executive / property engine) — still the architectural reference §2
  points to. → [`docs/phase-records/phase-3.md`](docs/phase-records/phase-3.md)
- **Phase 4** — PD elements (Transformer/Capacitor/Reactor), catalog objects
  (LineCode/XfmrCode/GrowthShape), the Line→LineCode fetch path, parse-only
  RegControl/CapControl, the `define_properties!` macro, and the controls-off
  feeder gate. → [`docs/phase-records/phase-4.md`](docs/phase-records/phase-4.md)
- **Phase 5** — controls + time series: XYcurve / LoadShape / TShape /
  PriceShape, ControlQueue + event log, RegControl/CapControl behavior, the
  control loop (`Sample_DoControlActions`), and the time-series solve modes.
  → [`docs/phase-records/phase-5.md`](docs/phase-records/phase-5.md)
- **Phase 6** — meters + topology: CktTree, Generator, Monitor, EnergyMeter +
  zone build, registers/TakeSample, reliability (`RelCalc`), Sensor + load
  allocation, the GenDispatcher/StorageController/AutoAdd/ReduceAlgs skeletons,
  and the 8500-node gate. Merged to `main` `b98223a`.
  → [`docs/phase-records/phase-6.md`](docs/phase-records/phase-6.md)
- **Phase 7 WP7.1** (Line constants & geometry) — the Carson engine, the
  WireData/CNData/TSData/LineSpacing/LineGeometry catalog, Line's geometry/spacing
  Carson path, the corpus migration, and the offline geometry golden. **Complete +
  gate-green on the `phase-7-extended-elements` branch (not yet merged);** the live
  §1e keeps a step summary + the tracked-open plural-cable note.
  → [`docs/phase-records/phase-7-wp1.md`](docs/phase-records/phase-7-wp1.md)
- **Phase 7 WP7.2** (Protection) — Fault, SwtControl, Fuse, Recloser, Relay (9
  sub-types), reliability activation (`HasOCPDevice` + live `RelCalc`), and the
  step-4 gate (the `phase7_protection` trip/reclose golden, the `Open`/`Close` exec
  verbs, the SwtControl corpus migration). **Complete + gate-green on the
  `phase-7-extended-elements` branch (not yet merged);** the live §1e keeps a
  per-step summary + the Phase-7 carry-forward rules + the `DG_Prot_Fdr` tracked-open.
  → [`docs/phase-records/phase-7-wp2.md`](docs/phase-records/phase-7-wp2.md)
- **Phase 7 WP7.3** (DER A) — `DynamicExp` (the diff-eq catalog object + its RPN
  expression interpreter), `InvBasedPceData` (the shared inverter PC-element base),
  and `PVSystem` (the power-flow PV element + zone admission + the Monitor mode-3
  fix + the GFM loud-abort). **Complete + gate-green on the branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp3.md`](docs/phase-records/phase-7-wp3.md)
- **Phase 7 WP7.4** (DER B) — the `Storage` element (the charge/idle/discharge state
  machine + integrated SOC) and the real `StorageController` fleet/dispatch (replacing
  the WP6.8 skeleton), plus the Storage-specific YPrim-rebuild fix. **Complete +
  gate-green on the branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp4.md`](docs/phase-records/phase-7-wp4.md)
- **Phase 7 WP7.5** (DER C, steps 1–4) — `RollAvgWindow`, the full `InvControl` (8
  modes + LPF/RiseFall + MonBus, both PVSystem and Storage DERs), `ExpControl`
  (the adaptive-`Vreg` volt-var control), and the step-4 corpus burn-down review.
  **COMPLETE + gate-green on the branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp5.md`](docs/phase-records/phase-7-wp5.md)
- **Phase 7 WP7.6** (Harmonics, steps 1–3) — the harmonics solve mode: the
  current-source family (VSource + Load) + the `SolveHarmonic`/`SolveHarmonicT`
  driver, the Thevenin DER family (Generator/PVSystem/Storage behind their
  subtransient reactance), and the monitor harmonic header + the `Set mode=`
  monitor/meter reset (Pascal `Set_Mode` tail); harmonics corpus burn-down is 0
  migratable (Phase-8/`Isource`/FaultStudy-blocked). **COMPLETE + gate-green on the
  branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp6.md`](docs/phase-records/phase-7-wp6.md)
- **Phase 7 WP7.7** (Dynamics core, steps 1–3b cont.) — the `SolveDynamic`
  predictor/corrector driver + per-element dynamics state machinery for Generator /
  PVSystem / Storage / IndMach012, Monitor mode 3, the `Open`-verb fix, the
  `set_ITerminalUpdated` stamp sweep, and the DynEqPCE user-`DynamicExp` integration
  for all three PCE families. **✅ COMPLETE** (steps 1–4; GFM deferred,
  tracked-open). The completed-step detail (incl. all audit follow-ups) is archived;
  the live §1e keeps the concise per-step summary.
  → [`docs/phase-records/phase-7-wp7.md`](docs/phase-records/phase-7-wp7.md)

---

## 1e. Phase 7 record (branch `phase-7-extended-elements`) — ✅ COMPLETE

The full per-WP roll-up (WP7.1–WP7.10 step summaries, decisions, audits, gate
detail, and the cross-cutting carry-forward rules) is **archived** at
[`docs/phase-records/phase-7.md`](docs/phase-records/phase-7.md); the deeper
per-step detail lives in the sibling `phase-7-wp{1..7}.md` archives. Phase 7 is
COMPLETE + gate-green on the branch, **NOT merged to `main`** (per-phase merge =
explicit-request-only HARD STOP). Headline: **WP7.1** line constants & geometry,
**WP7.2** protection (Fault/Fuse/Recloser/Relay/SwtControl + reliability
activation), **WP7.3–7.5** DER (DynamicExp/InvBasedPCE/PVSystem;
Storage/StorageController; InvControl/ExpControl), **WP7.6** Harmonics, **WP7.7**
Dynamics core (SolveDynamic + Generator/PVSystem/Storage/IndMach012 + DynEqPCE),
**WP7.8** Converter/FACTS (VSConverter/VCCS/UPFC+UPFCControl/ESPVLControl),
**WP7.9** FaultStudy (AutoAdd/Monte/LD/Feeder empirically deferred — zero corpus
cases), **WP7.10** exit. Retro audit (WP7.7 step 4 → WP7.8): no Critical/Major
correctness bug. Two real port bugs found+fixed in WP7.5 (the cross-step
`FFlagVWOperates` latch + the missing post-`DoPendingAction` `LoadsNeedUpdating`),
each a [[dont-rationalize-conditioning]] instance. **Tracked-open** (both
Plot-blocked, zero corpus payoff): GFM grid-forming mode + Generic/TD21 relay
`Sample`. lib **713**; `solvable_now` **88** at Phase-7 exit.

---

## 1f. Phase 8 record (`PHASE8_PLAN.md`) — ✅ COMPLETE (2026-07-10; WP8.8 exit record in §1)

Execution plan: **`PHASE8_PLAN.md`** (WP8.1–WP8.8, the reporting/output + full
executive layer; per-step cadence = `PHASE8_PLAN.md §0`). Phase 8 is almost
entirely *read-and-format* — no new electrical math, no new solve mode; the risk
is faithful report layout and **not silently faking output**. The full per-step
log for the **completed** work (decisions, audits, gate detail, the real-gap
write-ups) is **archived** at
[`docs/phase-records/phase-8.md`](docs/phase-records/phase-8.md). Headline:
**WP8.1** report infrastructure, **WP8.2** solution exports, **WP8.3** device/
reliability + logs, **WP8.4** `Show` reports, **WP8.5** `Save circuit`, **WP8.6**
executive verbs, **WP8.7** ReduceAlgs, **WP8.8** exit + corpus classify — all
COMPLETE, gate-green. The full per-step WP8.1–WP8.4 detail is archived there.

---

## 2. What Phase 3 built (file-by-file map) — archived

The Phase-3 vertical-slice **file-by-file architectural map** moved to
[`docs/phase-records/phase-3.md`](docs/phase-records/phase-3.md) (2026-06-21) to
keep this handoff lean. It is still the architectural reference §3/§4/§5 below
build on — only its location changed.

---

## 3. Key design decisions & rationale

### 3.1 Element storage stays in the executive; the solver sees `ElemStore`
`Vec<Box<dyn DssObject>>` per class; the circuit holds `Vec<ElemRef>` lists;
solution machinery walks them through `ElemStore` + `as_ckt_element_mut()`.
Zero unsafe, no double ownership.

### 3.2 Signal flags instead of `ActiveCircuit` globals *(load-bearing)*
Elements set `cd.signal_bus_name_redefined`/`cd.yprim_invalid`; the executive
propagates after the edit loop. Equivalent because nothing reads the globals
mid-edit (first reader is `BuildYMatrix` at solve time).

### 3.3 Compensation-current loads
Loads are stamped into Y **and** inject `Yprim·V − model current`; iteration
equality in the gates is the regression test for this.

### 3.4 Parse-time reference snapshots + deferred cross-element writes (Phase 4)
Pascal resolves object references mid-parse against live pointers and lets
recalc *read* (and `Set_TapNum` *write*) the target at any time. The Rust edit
loop holds only a read view of foreign classes, so:
- reads needed later (control `RecalcElementData` at `EndEdit`) come from a
  `RefSnapshot` captured at resolution time — same staleness semantics as
  Pascal (a control refreshes only on its own recalc);
- writes (`TapNum`) become queued `RefAction`s the executive applies right
  after the edit, with the writer keeping its snapshot in sync via the
  identical clamp. Nothing observes the target in between.

### 3.5 `NOT_PORTED` property flag
Catalog/machinery references that belong to later phases hard-error on set —
a script that needs unported machinery cannot produce silently-wrong numbers.

### 3.6 Controls are invisible to Y
`ElemKind::Control` elements join the device list (so `ProcessBusDefs` walks
them in creation order — node order matches the oracle) but never the PD/PC
lists; `yprim` stays `None` and the Y build skips them.

---

## 4. Empirical oracle facts (cumulative highlights)

- `?`, `Edit`, `~`, `Solve`, `Set`, `Get` are **circuit-gated** (error 301).
- `Solution.Iterations` = the total over control iterations, assigned at the
  end of `SolveSnap`.
- `YNodeOrder` = `ProcessBusDefs` allocation order over enabled elements in
  creation order — **including control elements** (their bus is set in
  `RecalcElementData`; they add no nodes in the IEEE feeders).
- The `DoubleSymMatrixProperty` getter is broken upstream (reads a field
  address as the array) — Capacitor `CMatrix`, Reactor `RMatrix`/`XMatrix`
  always dump garbage; canonicalized to zeros on both sides.
- `Circuit.Losses` skips shunt PD elements; `Circuit.TotalPower` = Σ sources
  `Power[1]`·1e-3 (not negated); element `Powers` = `GetPhasePower`·1e-3 over
  all conductors/terminals.
- RegControl `TapNum` get/set maps tap↔integer through the *controlled
  winding's* (TapWinding) Min/Max/Increment: `tapnum=5` on a 32-tap winding
  moves the tap to 1.03125; reads back 5. `winding=` resets `TapWinding`.
- CapControl `type=time` (and `follow`) forces `Terminal=1` and monitors the
  capacitor itself; a missing `capacitor=`/`element=` raises (303); `vbus=`
  set during parse warns "Did you wait until buses were defined?" and reverts
  the flag (bus list doesn't exist yet) — faithfully reproduced.
- Controls-off feeders: IEEE13 41 nodes, IEEE37 117, IEEE123 278; all solve in
  exactly 3 fixed-point iterations.

---

## 5. `TODO(compat)` / deferrals

Grep `rg "TODO\(compat\)"` for the full marker list (**123 sites across 71 files** as of 2026-07-17 — Phase 7/8/GAPS/UPGRADE added many; the whole set is wiped in one DE_PASCALIZE Stage F pass, not yet run). Notable:
truncated `CALPHA`/`pi`/`0.001732`/`57.29577951` constants, FPC banker's
`Round` shims, LineCode `Repair`=0 default, the `DoubleSymMatrix` zero-matrix
getter.

`NOT_PORTED` (hard parse error; every site points at its phase):
- Line `geometry` — **ported (WP7.1 step 3a)**: resolves a `LineGeometry`, runs
  `FetchGeometryCode` + `FMakeZFromGeometry` (the Carson `Zmatrix`/`YCmatrix`).
  Line `spacing`/`wires`/`cncables`/`tscables` stay `NOT_PORTED` until step 3b
  (the `FetchLineSpacing`/`SetWires`/`FMakeZFromSpacing` path, PORTING_PLAN
  §Phase 7 sub-block 1).
- Reactor `RCurve`/`LCurve` — Phase 5 (XYcurve) — XYcurve is now ported; the
  fetch is still `NOT_PORTED` (only the harmonic `CalcYPrim` consumes it, Phase 7).
- CapControl `ControlSignal` — Phase 5 (LoadShape); still `NOT_PORTED` (the
  `Follow` control type that consumes it has no corpus case — WP5.6's `Sample`
  records the Pascal abort error if reached); `UserModel`/`UserData` — never
  (no DLL loading in safe Rust).
- LoadShape `CSVFile` — **ported (WP5.2b)** via the deferred-`FileLoad` path.
  `SngFile`/`DblFile`/`PQCSVFile` (binary/2-col input) stay `NOT_PORTED` until a
  gate needs them. Single-precision arrays + `MemoryMapping` (MMF) and
  `Action=DblSave`/`SngSave` (binary output) — not ported (no corpus case).
- TempShape (`TShape`)/PriceShape `CSVFile` — **ported (WP5.2c)** via the same
  deferred-`FileLoad` path. `SngFile`/`DblFile` (binary input) and
  `Action=DblSave`/`SngSave` (binary output) stay `NOT_PORTED`.
- GrowthShape `CSVFile`/`SngFile`/`DblFile` — file-input machinery, when a
  gate needs it.

Other deferrals: Transformer GIC path (<0.51 Hz) + harmonics interplay
(the frequency-scaled Y + the <0.51 Hz branch are **exercised by WP7.6
harmonics**; the GIC *elements* stay Phase 9); RegControl/CapControl
`Sample`/`DoPendingAction` **wired into the
control loop (WP5.7)**; RegControl/ControlQueue debug-trace files (flag
stored, no file — port with Monitors, Phase 6+); `MakePosSequence` everywhere
(Phase 6+); `BusCoords` **ported (WP5.8)**; Monitors/EnergyMeters
`sample_all`/`EndOfTimeStepCleanup` are no-op hook stubs at the SolveDaily/
Yearly/Duty call sites (Phase 6); the **dynamics** (WP7.7), **harmonics/harmonicT**
(WP7.6) and **faultstudy** (WP7.9) solve modes are **ported**; the Newton algorithm
and the Monte-Carlo/load-duration/AutoAdd/`SolveGeneralTime` solve modes keep the
"Unknown solution mode" error (no corpus case — WP7.9 empirical decision);
the report verbs are **routed (Phase 8)**: `Export Counts` (WP8.1) and the
bus/node solution exports `Voltages`/`BusCoords`/`NodeNames`/`YNodeList` (WP8.2
sub-step 1) are **real**, the remaining `Export` keywords + `Save`/`Dump` record a
scoped `NOT_PORTED` (real formatters in WP8.2–8.5), `Show` is a faithful silent
no-op (real `ShowResults` in WP8.4), `Plot`/`Visualize` are headless no-ops
(§2.5); `Select`/... remaining executive verbs still record "not ported"
(Phase 8 WP8.6).

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
python tools/golden/gen_props.py       # -> tests/golden/props/<class>.json   (Phase 2+)
python tools/golden/gen_slice.py       # -> tests/golden/slice.json           (Phase 3)
python tools/golden/gen_phase4.py      # -> tests/golden/phase4/*.dss + phase4.json
python tools/golden/gen_phase5.py      # -> tests/golden/phase5/<scenario>.json
python tools/golden/gen_phase6.py      # -> tests/golden/phase6/<scenario>.json
python tools/golden/gen_checkpoints.py # -> tests/golden/checkpoints/<scenario>.json
```
Oracle pin: Python 3.12.4, dss-python 0.15.7, dss-python-backend 0.14.5
(the same dss_capi release vendored in `.inputs/dss_capi`). `python` works in
this environment; the `py` launcher is broken — use `python` directly.

**Official-EPRI-OpenDSS oracle (opt-in, `tools/opendss/` — 2026-07-07):**
vendored EPRI `OpenDSSDirect.dll` r3723 (9.8.0.1) / r4088 (10.2.0.1) / r4133
(11.0.0.1) driven through the AltDSS Oddie bridge (dss-python 0.16.0b2 in a
separate venv, `PIN_OPENDSS.txt`), reusing `oracle_server.py` unchanged via
`DSS_ORACLE_ENGINE=oddie`. For inventorying upstream changes ahead of porting
them; the mandatory gate is untouched. Workflows (see `tools/opendss/README.md`):
`DSS_LIVE_OPENDSS=<rev> cargo test ... corpus_live_opendss` → report
`tmp/opendss_report_<rev>.json`, now partitioned against the triage catalog
`tools/opendss/known_diffs.json` (2026-07-07, modeled on DSS-Python
`KNOWN_COM_DIFF`; substring match on case label + first-failure reason, every
entry states its cause, zero-hit entries warn). r3723: 150 matched / 82
known-diverged / **0 new** of 232 — all 82 triaged into 11 classes (19 EPRI
InvControl max-iter failures, 25 InvControl fixpoint drift, 10 iteration
deltas, 8 monitor-header whitespace, 4 property-format brackets, 4
injection FPC-vs-Delphi ulp, 3 storage kWhStored drift, 3 meter ZonePCE
count, 3 event-log trailing space, 2 GenDispatcher prop-name, 1 harmonics
Y-fingerprint) — so `DSS_LIVE_OPENDSS_ASSERT=1` (fails only on NEW) is green
for r3723. Caveat: comparison stops at a case's first divergence — a known
first divergence masks later ones in that case (accepted for inventory).
`ab_compare.py --a oddie:r3723 --b oddie:r4133` → upstream-change inventory
(baseline: 109/168 match; deltas in distance relays, harmonics, InvControl
iteration behavior); `--known-diffs tools/opendss/known_diffs.json` relabels
fully-triaged cases `known_diverged` (entries carry `ab_contains` where this
tool's issue wording differs) and exits 0 when only known diffs remain.
Two operational gotchas, both handled: (1) EPRI's Delphi `FireOffEditor`
ShellExecutes the editor on every `Show`/`Export` with NO `NoFormsAllowed`
check and Oddie can't set `AllowEditor` — a corpus sweep opened hundreds of
Notepads; `make_engine()` now issues `Set RegistryUpdate=No` + `Set
Editor=rundll32.exe` (silent no-op; registry write suppressed so the user's
OpenDSS editor setting is untouched) — verified on all 3 revisions with a
`Show` deck, zero spawns. (2) `.inputs/electricdss-tst` is now a re-checkout
with different EOLs: `tools/corpus/vendor.py --force` produces a ~1544-file
EOL-only diff — clean run pollution with `git restore tests/corpus` instead;
re-vendor only deliberately.

**DSS-Python validation harness, vendored (`tools/opendss/dsspy_validation/`
— 2026-07-07):** copy of DSS-Python `fastdss` `tests/`
`_settings`/`save_outputs`/`compare_outputs` (BSD-3, attribution headers,
local edits marked `# dss-rs:`): full-API-state dumps (~40 collections/case,
206 upstream-curated cases, all present in our corpus) zipped per engine +
offline tolerant diff (their `KNOWN_COM_DIFF` catalog kept as upstream) —
broad-surface upstream inventory complementing `ab_compare.py`. Adaptations:
corpus → vendored copy, engine spec `DSS_EXTENSIONS_TEST_ODDIE=oddie:<rev>`
via `revisions.json` (+ expect_version hard check), COM branch dropped, our
`RegistryUpdate=No`+`Editor=rundll32.exe` suppression, per-case `CorpusGuard`
(lifted move-only into `tools/oracle/corpus_guard.py`, shared with
oracle_server), results → `tmp/dsspy_validation/`, and `(Oddie)`-prefixed
DSSException skips for API exports absent from older official DLLs (r3723
lacks `Transformers_Get_LossesByType`, `StoragesI`, ...). **Its `capi` side
is dss_capi 0.15.0b4 — NOT the pinned 0.14.5 oracle; inventory only, never
feeds goldens/gate.** pandas+xmldiff pinned into the Oddie venv
(`PIN_OPENDSS.txt`). Sweeps must end with `git status tests/corpus` (guard is
non-recursive; a sweep-created *subdirectory* — 123Bus `Run_YearlySim` makes
`16Nov2011/` — escapes it: `git clean -fd` that path). Full-sweep baseline
2026-07-07: capi 199/206 captured, oddie:r3723 189/206 (its 19 misses = the
`epri-invcontrol-maxiter` #485 class, 1:1 with known_diffs), compare
processes 3885 zip entries. The two beta packages are vendored as wheels in
`tools/opendss/wheels/` (+SHA256SUMS; offline `--find-links` install proven)
— setup no longer depends on the pre-releases staying on PyPI.

**DSS-Python corpus cross-check (`tools/corpus/dsspy_crosscheck.py` —
2026-07-07):** diffs DSS-Python's own 206-case validation list
(`.inputs/DSS-Python/tests/_settings.py::test_filenames`, extracted textually
— importing that module binds a DSS engine) against our six classifier
manifests → `tmp/dsspy_crosscheck.{json,md}`. Measured split: 133
solvable_now / 59 skipped_unsupported / 8 needs_investigation / 5
oracle_issue / 1 not_an_entry_point = **73 promotion candidates** (35
unblock at WP8.6 BatchEdit alone); all 206 exist in the vendored corpus.
`L!`-prefixed cases (55) are run line-by-line upstream with interactive
commands filtered — recorded per case so promotion work doesn't naively
`Compile` them.

---

## 7. Phase 7 — inherited deferrals & architecture in place

> **The current frontier** (active step, branch, what's next, commit state) lives
> in the header up top and in **§1e** — not restated here, to avoid the two drifting
> apart. This section is the stable Phase-7 reference: what the phase inherits and
> what is already wired for it. Execute per `PHASE7_PLAN.md §0` (six
> independently-gated sub-blocks, risk-ascending: line constants → protection →
> DER → harmonics → dynamics → faultstudy/AutoAdd-modes/`Feeder`).

**What Phase 7 inherits / must finish (deferrals Phase 6 left explicit):**
- **DER classes** `Storage`/`PVSystem` (+ `InvControl`/`ExpControl`) and the real
  `StorageController` behavior — ✅ **all done**: `PVSystem` (WP7.3), `Storage` +
  `StorageController` (WP7.4), and `InvControl` + `ExpControl` (WP7.5) (the WP6.8
  StorageController parse-only skeleton is replaced by the real fleet dispatch;
  `is_zone_pce` now admits Storage/PVSystem).
- **Protection** `Relay`/`Recloser`/`Fuse`/`SwtControl`/`Fault` — ✅ **done
  (WP7.2)**: all five classes ported on the control sweep, the `Open`/`Close` exec
  verbs landed, and an enabled Relay/Recloser/Fuse sets `Flg.HasOCPDevice` so
  `RelCalc` no longer aborts (#52902) and `GetOCPDeviceType` is live — the
  SAIFI/SAIDI/section math runs on a protected zone.
- **Line constants** `WireData/CNData/TSData/CableData/LineSpacing/LineGeometry`
  + Carson — ✅ **done (WP7.1)**: Line's
  `geometry`/`spacing`/`wires`/`cncables`/`tscables` resolve and drive the Carson
  Z/Yc (one plural-cable reset + the `DG_Prot_Fdr` ~3e-5 line-Y precision case
  tracked-open, §1e).
- **Harmonics** (`DoHarmonicMode` for VSource/Load + Generator/PVSystem/Storage,
  the frequency sweep + the harmonic monitor header) — ✅ **done (WP7.6)**.
- **Dynamics** (Generator/Storage `DoDynamicMode`, state vars beyond names/count)
  + `MakePosSequence` everywhere; Monitor modes 3/4/7/8/10/12 build their header
  but defer the sample body; Transformer GIC (<0.51 Hz) elements — WP7.7+ / Phase 9.
- **AutoAdd solve mode** (`circuit/auto_add.rs` skeleton) — needs aux-current
  injection (`UseAuxCurrents`) + meter-register sampling in the solve loop; the
  options round-trip but the mode keeps its "Unknown solution mode" error.
- **ReduceAlgs** zone reduction — blocked on the unported `TLineObj.MergeWith`.

**Architecture already in place for Phase 7:** the control loop dispatches
through `ElemStore::{obj,pair_mut,triple_mut}` + `DssObject::as_any_mut`; the
meter/monitor `sample_all_monitors_and_meters`/`end_of_time_step_cleanup` hooks
have real bodies; the zone-build dispatcher (`solution/meters/mod.rs`) fires from
`build_y_matrix` after bus reprocessing; `TakeSample`/`Integrate` + the
reliability fault-rate sweep are ported. Still Phase 8: the `SystemMeter`
register core and all demand-interval/phase-voltage/`Show`/`Export` files.

---

## UNIFIED_GATE Phase A — `dss-epri` bridge + `epri-worker` + smoke + xcheck (session record)

New test-only crate `crates/dss-epri` (`publish = false`) — the **only** crate
without `#![forbid(unsafe_code)]` (carries `#![deny(unsafe_op_in_unsafe_fn)]` +
`#[cfg(windows)]` + module `// SAFETY` docs; carve-out documented in
`PORTING_PLAN.md` §1). It drives the official EPRI `OpenDSSDirect.dll` (r4133) via
`libloading` as a second live oracle (UNIFIED_GATE_PLAN.md R1/§2). Layout: `ffi.rs`
(raw `cdecl` externs + load with `LOAD_WITH_ALTERED_SEARCH_PATH` so sibling
`KLUSolve.dll` resolves), `dss.rs` (command + V-protocol decode + error polling),
`capture.rs` (`CaseResult` assembly, byte-mirroring `oracle_server.py` +
`gen_checkpoints.py`), `guard.rs` (corpus-guard port), `src/bin/epri-worker.rs`
(persistent `ping`/`run`/`quit` line-JSON worker + `--smoke`). Root `Cargo.toml`:
added the workspace member, `libloading` workspace dep, and the dev opt-level-3
override.

**Root-cause war story (load-bearing, do not re-litigate).** The DLL deadlocked
when driven from Rust — every call appeared to hang. Diagnosed by minidump: the
process parks in `NtUserMsgWaitForMultipleObjectsEx` inside `OpenDSSDirect.dll`.
The trigger is **`FreeLibrary` on `libloading::Library` drop**: the r4133 DLL's
unit finalization tears down its Delphi solver **actor thread** through a
message-pumping `TThread.WaitFor` that never completes headless (no VCL
`Application`/`WakeMainThread`). The hang *looked* mid-execution only because the
`Engine` (local in `run_smoke`) dropped — and FreeLibrary'd — before the report
printed. The Python/Oddie host never hit it because interpreter shutdown doesn't
`FreeLibrary` the DLL. **Fix:** never unload — `Dll::leak()` (`mem::forget` the
`Library`); the OS reclaims it at process exit (actor thread already terminated,
so DllMain-detach is clean). Confirmed by a minimal raw-FFI reproducer:
libloading-load hangs at drop, `mem::forget(lib)` cures it (single-thread, no
message pump needed). All the earlier COM/FP/message-pump/two-thread experiments
were chasing the wrong symptom and were reverted.

**Capture-decode notes** (verified against raw-DLL ctypes + Oddie probes):
V-protocol `mySize` is always **bytes**; type tags 1=int / 2=double / 3=complex
(flat re/im) / 4=string / 5=byte-stream. String arrays: strip **one** trailing
`\0`, split on `\0`, then lstrip a single leading space from element 0 (the Oddie
monitor-header first-column artifact — `[' V1', ...] → ['V1', ...]`; no other
array's element 0 has a leading space, so applying it universally is a no-op).
Monitor channels decoded from the raw `ByteStream` exactly like `IMonitors.Channel`
(272-byte header, `record_size = int32@offset8 + 2`, f32 records). The DDLL solve
is **async** (dispatches `SIMULATE` to the actor and returns), so `solve()` polls
`ParallelV(1)` = `ActorStatus` until done.

**DONE-bar evidence:**
- `epri-worker --smoke` (10/10 runs, exit 0, stderr→/dev/null):
  `version OK: Version 11.0.0.1 (64-bit build) - Charlottesville`
  `IEEE13 solved: 41 nodes, 2 iterations`
  `CSC export OK: n=41, nnz=267, voltages bit-identical (solution-neutral)`
  `getIpointer OK: len=84 (= 2*(NumNodes+1))`
- Smoke `#[test]` (`crates/dss-epri/tests/smoke.rs`) runs under `cargo test
  --workspace`, oracle-free.
- **xcheck** (`tools/opendss/xcheck_bridge.py`, temporary — Phase E deletes it):
  drives all 396 cases of the r4133 `corpus_live_opendss` universe (240
  solvable_now + 43 asymmetric + 66 controls + 47 modes; #303 `skip` decks
  excluded identically both sides) through the Python/Oddie r4133 engine AND the
  Rust `epri-worker`, bit-diffing the raw `CaseResult` — twice, second pass
  order-shuffled (seed 1337). Result (settle re-run, ~4m50s wall): **396 matched
  of 396** on pass 1 (manifest order) AND pass 2 (shuffled) → `XCHECK PASS: both
  passes bit-identical over 396 cases`, exit 0. `diverged = ok_mismatch =
  both_err = 0` (matched == universe). `tests/corpus` verified pristine after
  (CorpusGuard on both sides).
- fmt + clippy (`-D warnings`) clean on `dss-epri`.

**Audit dispositions (two independent audits of the phase diff).**
- **F1/A1 — `both_err` not gated in `xcheck_bridge.py` (real, FIXED).** The PASS
  boolean was `clean = not diverged and not ok_mismatch`, so a case that errored
  symmetrically on BOTH engines was silently absorbed — a latent fake-success
  channel (arithmetically inert for the reported clean run, but a weaker guarantee
  than the DONE bar). Strengthened to `... and not both_err`: the universe excludes
  solve-abort/pending cases, so every case must yield a comparable `CaseResult` on
  both engines; a symmetric error is a hole, not a pass. Empirically settled — the
  full re-run above with the stricter gate still PASSES 396/396 (both_err = 0), so
  the fix closes the channel without any false failure. Strengthening only; no
  tolerance/assertion weakened.
- **F2 — additive `clear` command in `oracle_server.py` (real deviation,
  ACCEPTED, no change).** UNIFIED_GATE_PLAN D8/§3.2 noted "no protocol change
  needed server-side." Phase A added a `clear` handler (release the circuit + any
  held loadshape MMF handle so the two processes can compile the same case). It is
  purely additive and non-breaking: `corpus_live.rs` never sends `clear`; `run`/
  `ping`/`quit` are untouched; `oracle_server.py` is not in the brief §4 "untouched
  consumers" list. Used only by the temporary `xcheck_bridge.py` (Phase E deletes
  both the tool and its need for the command). Kept as a justified, harmless
  deviation.
- **F3 — universal leading-space strip on string-array element[0] (not a bug,
  documented).** `decode_string_array` lstrips one leading space from element[0]
  of every V-protocol string array. Settled empirically + by grammar, not by
  universe coincidence: the other arrays (node order, element/register/variable
  names, zone lists) are whitespace-delimited DSS identifiers that can never begin
  with a space → the strip is a guaranteed no-op on them; the monitor CSV header
  (the one array whose first token carries a leading space) is exactly the intended
  target. Confirmed bit-for-bit against Oddie over all 396 cases. Doc comment
  strengthened to record the grammar guarantee. No behavior change.

Follow-ups (STATUS, non-blocking): none block Phase A. Phase E removes
`xcheck_bridge.py` and, with it, the `oracle_server.py` `clear` handler's only
consumer (drop the handler then).
