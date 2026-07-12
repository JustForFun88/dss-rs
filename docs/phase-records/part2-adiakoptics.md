# DIAKOPTICS / PSTCALC — Part I record (+ Part II sequencing note)

> **Archived verbatim from `STATUS.md` on 2026-07-12** to keep the living handoff lean. Part I
(WP-PF.1 Pstcalc command, WP-PF.2 Monitor mode-4 flicker, WP-AD.1 incidence
matrix + Sparse_Math + exports 53–57) is COMPLETE and frozen here. **Part II**
(A-Diakoptics, WP-AD.2–AD.6) is deliberately outside final acceptance and is
sequenced after MULTITHREADING M2; its records live on the `part2-adiakoptics`
branch, not in this file.

**DIAKOPTICS_PSTCALC Part I COMPLETE (2026-07-11), gate-green.** WP-PF.1 + WP-PF.2 +
WP-AD.1 executed as an ultracode round (3 opus executors + 6 opus auditors + settle
agents in isolated worktrees; coordinator merged `wp-pf2-flicker-settled` (ff),
`wp-ad1-incmatrix-settled` (`e96caaa`), `wp-pf1-pstcalc-settled` (`be2cfc7`); one
trivial conflict in `support/mod.rs`). Full gate on the merged tree: fmt + clippy
clean, `cargo test --workspace` exit 0 (953 lib units, corpus_live 14/14 incl. the
three family gates, all binaries 0 failed; the single "1 ignored" is the pre-existing
`obj/props` doctest). Per-WP records below. `PLAN_SEQUENCE.md` stage 3 is done — next
per the sequence is FINAL ACCEPTANCE (user decision), then UPGRADE_PLAN; Part II
(A-Diakoptics, WP-AD.2–AD.6) stays sequenced after MULTITHREADING M2.

**WP-PF.1 — `Pstcalc` executive command (2026-07-11), gate-green.** `support/pstcalc.rs`
(f64 `PstEngine`, IEC-868 path of `Shared/Pstcalc.pas` 1–474) + `exec/pstcalc.rs`
(`DoPstCalc`, ordinal 96), error #28723 verbatim (upstream "Insuffient" typo kept).
FPC semantics probe-proven end-to-end: `power(x,4)=sqr(sqr(x))` via intpower (docs in
module), banker's `Round` at the two `TODO(compat)` spots; D5 UB clamps in
`SB`/`Gather_Bins` (unreachable one-past-allocation paths; bit-identical reachable
behavior). Gates: `golden_pstcalc.rs` — 25-case oracle golden, **byte-exact**
`GlobalResult` strings across npts/shape/lamp/freq/dt (incl. fractional dt); 6 unit
tests; family deck `modes/pstcalc_cmd.dss` (live no-error + full-model compare).
Audits: code NO FINDINGS; tests 2 Minor — the `compare_global_result: true` flag was
inert (harness captures GlobalResult after its own solve, which clears it on both
engines; oracle-probe-proven) → settled by coordinator (`a45b74f`): flag dropped, note
rewritten. Lane settle agent stalled waiting on a monitor (its branch = exec commit;
nothing was lost — audits had no code findings).

**WP-PF.2 — Monitor mode 4 flicker (2026-07-11), gate-green.** Mode-4 sample body
(`monitor/sample.rs`), `post.rs` `DoFlickerCalculations` (in-place channel rewrite:
Flk←Block-4 level, Pst←600 s interval value, exact ipst/tpst stepping), `PostProcess`
latch wired at the `to_csv`/export/show sites; `support/flicker.rs` = the f32
`FlickerMeter` (Pstcalc.pas 476–687) with the proven Single-storage/Double-arithmetic
model (settle `6e67121`: window-control expressions compute in f64 over f32 operands;
Sterbenz-lemma probe shows the `(t−tPst)≥600` half is unobservable, the
`trunc(600/ts)` half diverges at ts∈{0.05,0.1,…} and the f64 form matches Delphi
exactly). **New upstream-bug finding: the pinned oracle (dss_capi 0.14.5) CRASHES on
mode-4 post-processing** — `export monitor`/`Process` raises access violation #671
(Terminals OOB) while raw `Channel()` reads work. Consequence (audit-verified
legitimate): the mode-4 CSV golden (`golden_flicker.rs`, 8640×3 samples from the
recreated `Examples/Matlab/pst.dss` demo) is a **committed r3723 reference** per
DIAKOPTICS_PSTCALC_PLAN D9(b) — static, replayed at test time, no live EPRI gating;
the family decks `controls/monitor_pst.dss` + `midi_monitor_pst.dss` omit
`export monitor` and live-gate the sampled channels via `check_meters_monitors`
(oracle `Channel()` path, micro tier). `Action=Process` stays a parse-time no-op per
the meter-wide convention (EnergyMeter identical); doc corrected. Follow-up candidate:
document the oracle mode-4 crash in `investigations/` (deterministic upstream crash,
not reproduced — the Rust port post-processes correctly).

**WP-AD.1 — incidence matrix + Sparse_Math + exports 53–57 (2026-07-11), gate-green.**
`support/sparse_math.rs` (SparseInt/SparseComplex 1:1 COO: insert
accumulate-else-append, insertion-order storage, multiply/add drop quirks
`re<>0 AND im<>0` pinned at Pascal :654/:868, Rank row-echelon walk incl. the last-row
off-by-one), `solution/inc_matrix.rs` (`Calc_Inc_Matrix`/`Calc_Inc_Matrix_Org` with
hierarchical ordering + `Inc_Mat_levels`, the four element walks incl. series-only
Capacitor/Reactor filters), commands `CalcIncMatrix`/`CalcIncMatrix_O`/`CalcLaplacian`
(#8877 "Indidence…" verbatim), exports 53–57 byte-exact with oracle default filenames
(`Inc_Matrix.csv` … `Laplacian.csv`, ExportOptions.pas:417–425). Gates: 28 goldens
over IEEE13/IEEE123 + purpose-built series-cap/series-reactor decks via
`tools/golden/gen_inc_matrix.py` — the tests auditor independently regenerated all 28
from the pinned oracle: **0 diffs**; 21 tests incl. negative #8877; settle `351fefe`
added filename pins + 4 hand-traced complex-op unit tests. Audits: code NO FINDINGS;
tests 2 Minor → fixed in settle. `Refine_BusLevels` stays refused (AD-gated → Part II).


---

## Part II records (moved verbatim from the part2-adiakoptics branch STATUS at the update-integration merge, 2026-07-12)

**PARKED TEST (needs investigation, user decision 2026-07-12):**
`circuit::coverage::tests::refine_bus_levels_reports_paths_on_radial` is
`#[ignore]`d. The WP-AD.5 `Get_paths_4_Coverage` state machine (ported 1:1;
sole exit at Circuit.pas:909) never terminates on the test's 6-bus radial,
even after requesting a reachable `set coverage=0.5` — the first hypothesis
(unreachable 0.9 default; see the NOTE(upstream-quirk) at the function) proved
insufficient. Needs a trace of `Inc_Mat_Levels` / per-path `Buses_Covered` on
the official r3723 engine vs ours. Surfaced at the part2 consolidation gate:
the AD5 line's own full gate was never witnessed (killed mid-run) and its
audits never ran (the round stopped on `gate_green=false`), so the hang
shipped unreviewed. `Refine_BusLevels` itself stays ported/enabled.

**WP-AD SAVE — save round-trip fidelity (branch `save-fidelity`, 2026-07-12;
settle pass).** Owns the `off:save-roundtrip-*` buckets (the D7 leg1 gap WP-AD.4
proved is a `save circuit` defect, not AD). Each of the 40 `save-roundtrip-geometry`
decks was root-caused with the `DSS_AD_CLASSIFY` (total gap) + `DSS_AD_DECOMPOSE`
(leg1 save vs leg2 AD) probes, against the pinned dss-python 0.15.7 Save oracle +
vendored `dss_capi 0.14.5` Pascal.

- **Three `SaveWrite` port bugs FIXED (real; emission oracle-checked via the CLI
  `save circuit` + reload).**
  1. `LineGeometry` explicit conductor table (`Cond=/Wire=/X=/H=/Units=`): the
     generic serializer collapsed the per-conductor slots (each re-set once per
     conductor) to one, emitting **only the last conductor** → geometry-built line
     reloaded with a wrong/unbuildable `Z` (leg1≈1). Ported `TLineGeometryObj.
     SaveWrite` (`line_geometry/save.rs`).
  2. `LineGeometry` `spacing=+cncables=/tscables=` form: the plural `CNCables`/
     `TSCables` props (flagged `Redundant`-with-singular in Pascal) fell to the
     generic arm and re-emitted `CNCables=[…]` **after** the conductor block, so
     the reload aborted (`Unexpected number of objects`). Ignore them like the
     singular `cncable`/`tscable` (matches Pascal's redundancy remap).
  3. `Line` inline spacing (`Spacing=+Wires=/CNCables=/TSCables=`): the three arrays
     all alias one `LineWireData`, so the generic save emitted the **whole** array
     under **every** kind that was set — a mixed `TSCables=[TS_1/0] Wires=[CU_1/0]`
     line saved as `TSCables=[ts_1/0, cu_1/0] Wires=[…]` and the reload aborted
     (`TSData "cu_1/0" not found`). Ported `TLineObj.SaveWrite` (`line/save.rs`) —
     contiguous same-catalog runs. `save_roundtrip.rs` gains the geometry witness
     and the line/cable-spacing witness (mixed TS/CN/wire conductors, the pre-fix
     reload-abort reproduced).
- **AD sweep manifest re-attributed from evidence (36 of 40 geometry decks).**
  Corpus `pf` 26→33 — **7 promoted** (each gate-verified AD-vs-normal < 2e-3 tier):
  IEEE13_LineGeometry/SpacingGeometry, **IEEE13_LineAndCableSpacing** (the Line
  fix), ckt5/Master_ckt5, 4Bus-{DY,OYOD,YY}-Bal. Clean-save decks (leg1 « tier)
  moved to their true blocker: `non-3ph-cut-only` (4 cable + 7 InvControl
  MonitoredVoltage), `too-small` (epri_dpv M1, LVTestCase/LineConstants),
  `ad-nonconvergent` (MultiCircuit, DOCTechNote 1_1/1_2/2_1/2_2, LVTestCase/Master),
  `ad-divergent` (4Bus-{GrdYD,YD}-Bal, YYD-Master{,-step1}, ckt24 ×3 leg2≈1),
  `ad-singular-zone` (NEV ×2 — AD zone-matrix panic `row<n && col<n`; the normal
  solve is fine, so it is an AD-engine gap not a save one), `save-roundtrip-control`
  (CapControlFollow — cap-bank runtime state, controls-off reload can't re-switch).
- **4 decks stay `save-roundtrip-geometry` (genuine leg1 save gap, still unfixed).**
  IEEE13_Assets (leg1 8.68) + DG_Prot_Fdr (leg1 1.0): structural — the
  `spacing=+wires=` **named**-LineGeometry reduced-`Z` path still round-trips wrong
  (distinct from the three fixed paths); open follow-up. ckt5/Run_ckt5 + Storage
  Run_Demo1 (leg1 2.55e-2 at the same ckt5 node): a moderate controls-off residual
  (likely a regulator/LTC not re-settling); sub-element unverified.
- **Note — the previous commit `e27d1f3` STATUS text was fictional** (claimed
  promotions never applied to `ad_sweep.json`; "pf 37→43" corresponded to nothing
  committed). This settle applies the real manifest changes and the two additional
  save fixes above.

**WP-AD.5 — coverage paths, Refine_BusLevels, ckt24 driver, LineGeometry save
fix (branch `wp-ad5`, 2026-07-12).** Landed:

- **Coverage-path tracing + `Refine_BusLevels`** (`circuit/coverage.rs`):
  `get_longest_path`/`Append2PathsArray`/`Normalize_graph`/`Get_paths_4_Coverage`
  ported 1:1 from official `Circuit.pas:778-913`; command 110 un-refused →
  `"<n> new paths detected"` (`ExecCommands.pas:691`). Working arrays
  (`longest_paths`/`path_idx`/`buses_covered`/`new_graph`) on `AdTearing`.
  `get Coverage` corrected to report `Actual_Coverage` (`%-g`,
  `ExecOptions.pas:1086`), not the requested value (was a WP-AD.2 stopgap).
  NOTE(upstream-quirk) at the `get_longest_path` descent (potential negative-index
  OOB read not reproduced, D5).
- **LineGeometry `SaveWrite` fix** (`elements/general/line_geometry/save.rs`): a
  REAL, previously-undetected `save circuit` defect. The generic serializer
  collapsed the per-conductor array props (`wire`/`x`/`h`/`units`) to the single
  last-set conductor, so any reloaded `LineGeometry` had undefined conductors
  ("WireData is not correctly initialized") — every LineGeometry save round-trip
  (incl. ckt24 AD init) failed to solve. Ported `TLineGeometryObj.SaveWrite`
  (dss_capi 0.14.5) to emit `~ Cond=i wire=.. X=.. h=.. units=..` per conductor.
  Semantic save round-trip (193 tests) + report goldens (6) still green.
- **ckt24 real-corpus AD driver** (`adiakoptics.rs::ckt24_driver`): drives the
  vendored `master_ckt24.dss` setup prefix + the deck's OWN manual 2/4-zone
  `LinkBranches`+`UseMyLinkBranches` cases (D10). Gates the **D7 AD leg**
  (AD solve vs saved-interconnected normal solve): CLEAN at ~7.6e-6 / 9.5e-6 for
  both partitions. Decomposition (D7): the ~1.8e-2 total gap is entirely the save
  round-trip leg (leg1, secondary-service nodes) — the AD engine itself is exact.
- Tear_Circuit `help_catalog` note updated to reflect Rust-side support.
- **NOTE marker inventory** (greppable, plan D2/D5): `NOTE(subst-metis)` × 6 in
  `exec/tearing.rs`, `exec/tearing_save.rs`, `support/partition.rs` (the exec-vs-
  in-process METIS + 4.0→5.2.1 version step); `NOTE(upstream-quirk)` × 20 across
  `circuit/coverage.rs`, `exec/diakoptics/{matrices,solve}.rs`, `exec/tearing*.rs`,
  `solution/inc_matrix.rs`, `solution/solution/power_flow.rs`,
  `support/{flicker,partition,sparse_math}.rs`.

**Open findings (WP-AD.5, filed for follow-up — NOT masked):**
- **ckt24 yearly-24 AD time-series divergence** (~1.1e-1 @ both secondary AND
  primary nodes, base>500 V, not a small-base artifact — decomposition-proven leg2,
  NOT save round-trip). Snapshot AD leg is clean, so the divergence is in the
  **multi-step time-series AD path** (WP-AD.3), likely a cross-step child load-state
  issue. `manual_2zone_yearly_runs_and_advances` exercises the path (converges +
  advances the clock) and PRINTS the gap ungated (§5: not tolerance-widened). Root
  cause is a WP-AD.3 concern, outside WP-AD.5's coverage/aggregate charter.
- **ckt24 save round-trip fidelity at secondary-service nodes** (~1.8e-2, leg1):
  after the LineGeometry fix, a residual `save circuit` gap remains at `_SEC_`
  nodes (triplex/service-transformer serialization). Same class as the WP-AD.4
  `off:save-roundtrip-*` decks. A save-circuit concern (D7: "fix there, not in AD").
- **ckt24 auto-tear does not converge** — expected: the deck header states "the
  buses for the lines are backwards ... the automatic partitioning will not work",
  so ckt24 mandates manual `LinkBranches`. No auto-tear gate for ckt24 (the
  synthesized midi/macro auto-tear gates in `ad_solve_gate` cover D2).

**Deferred (WP-AD.5, recorded — not silently dropped):**
- **`AggregateProfiles` + `Aggregate` command + `Aggregated_model/` golden** — the
  `Get_paths_4_Coverage` prerequisites, `Disable_All_DER` consumer (upstream
  comments it out at official `Circuit.pas:1663` — no live caller), and raw
  loadshape `p_mult_raw`/`q_mult_raw` accessors are in place, but the full port
  (`Circuit.pas:1616-1874`: per-zone EnergyMeter placement → snapshot solve →
  per-zone yearly-shape aggregation → `save circuit Dir=Aggregated_model`) was not
  completed this session. `AggregateProfiles` remains a scoped NOT_PORTED
  (`exec/command.rs`). This is the main remaining §0.1 command-surface item.
- **D9(d) — official-r3723 AD-replay compare machinery** (the WP-AD.4 leftover):
  the `AdSweepCase.oracle` field + `Oracle::opendss(rev)` bridge exist, but the
  `oracle_server.py` AD mode (replay `set Num_SubCircuits`/`ADiakoptics` on r3723
  with a `wait` after every solve, then return post-AD voltages/powers) was not
  built, so no sweep case carries `oracle:"r3723"` yet and the 4 non-actor AD-master
  decks (EPRI_Ckt5-G/Ckt7-G, IEEE_123_Bus-G, ckt24) stay in `skipped_oracle_issue`.
  The rust-vs-rust AD sweep (WP-AD.4) remains the primary AD gate; D9(d) is the
  official-reference strengthening layer.

**WP-AD.4 — corpus-wide AD↔normal sweep (branch `wp-ad4`, 2026-07-12).** The
user-mandated A-Diakoptics gate. Two parts landed:

- **Child `DO_CTRL_ACTIONS` fan-out + `GETCTRLMODE`** (the explicit AD.3 deferral).
  `ad_check_controls` now ports the official `CheckControls` AD branch
  (Solution.pas:1248, ActorID=1): the coordinator fans `DO_CTRL_ACTIONS` to each
  child (`sample_do_control_actions` + `check_fault_status` + child-Y rebuild) and
  ANDs their `ControlActionsDone`, instead of sampling its own (controls-off)
  controls. `set controlmode=`/`maxcontroliter=` on the AD coordinator now fans
  `GETCTRLMODE` to the children (`exec/set_cmd.rs`). Controls-off decks unchanged.
- **The sweep** (`corpus_ad_matches_normal_mode`, rust-vs-rust, always-on): every
  `solvable_now` entry (`manifests/ad_sweep.json`, bijective) **and** every family
  deck (mandatory `ad` field, loader-enforced) carries a disposition
  `full|pf|off:<specific-reason>`. `pf`/`full` decks are solved both ways and
  node-V compared at `AD_SWEEP_TIER=2e-3` (Torn_Circuit → per-case temp datapath,
  never the vendored deck). Population: **37 pf** (26 corpus + 11 family), 0
  `full`, 382 `off` (of 419); **zero `off:unclassified`**. Sweep wall-time **~22s**
  (no cap needed). Dispositions are evidence-backed (`DSS_AD_CLASSIFY` probe +
  `DSS_AD_DECOMPOSE` D7 leg split).

**Headline finding (AD engine vindicated).** The canonical feeders (IEEE13/34/37/
123, 8500-Node, …) show 2–66 % AD-vs-normal gaps, but the D7 decomposition proves
the gap is **100 % the save round-trip leg** (`Master_Interconnected.dss` reload
loses regulator/transformer/geometry/relative-file fidelity) while the AD leg
proper is clean at the fixture floor (IEEE13 leg1=5.9e-2 vs leg2 AD=**1.6e-6**;
ieee37 1.16e-1 vs **1.3e-5**; 8500 2.5e-1 vs **7.6e-6**). These are `off:save-
roundtrip-*` — a save-circuit gap to fix in save, not AD (per D7). Off-class
census after the settle-pass decomposition (corpus only; specific, categorized):
non-3ph-cut-only 106, save-roundtrip-{geometry 40, relpath 26, regxfmr 17, relay
8, autotrans 3, userdll 2}, already-torn-artifact 18, too-small 11, ad-islanded-
divergence 10, ad-nonconvergent 7, mode-outside-AD-scope 4, ad-switched-divergence
3, ad-floor-above-tier 3, ad-regulator-divergence 2 (plus the family classes).
The `ad-*` classes are OPEN AD-engine defects (see the settle-pass record below),
not undecomposed buckets.

**Bugs found (open items for follow-up, NOT this WP's gate):** (1) the save
round-trip fidelity gap above (regulator/transformer state + LineGeometry/WireData
ordering + relative data-file paths not preserved through `Master_Interconnected`
save/reload) blocks promoting ~68 corpus feeders from `off:save-roundtrip-*` to
`pf` — a save-circuit task.

**WP-AD.4 SETTLE PASS (branch `wp-ad4`, 2026-07-12) — audit findings resolved
empirically.** Every off:ad bucket was decomposed and the audit's Major/Minor
findings settled with evidence:

- **`full` now has real active-control coverage** (was 0, un-gated). New
  synthesized fixture `tests/data/adiakoptics/adreg.dss` (midi radial + a head
  RegControl LTC, zone-local) drives `adiakoptics.rs::full_zone_local_regcontrol_
  matches_normal`: the ONE gate on the child `DO_CTRL_ACTIONS` fan-out active
  path. Proof is a controlled experiment — the AD tear re-seeds each zone from the
  transformer's *declared* tap (1.0), so the **controls-OFF** AD arm lands 4.3 %
  off normal (one tap ratio); the **controls-STATIC** AD arm re-establishes the
  regulator **inside the child zone** (its tap events live in the child eventlog,
  not the coordinator's) and matches normal at 3.2e-5. That clean/diverged split
  is only possible if the child fan-out actually sampled + operated the zone
  control. (A capcontrol fixture was tried first and abandoned: the CapControl's
  reset/initial-state latches inconsistently across the base-solve/AD-reseed
  paths — a plain shunt cap is clean under AD at 3.6e-5, but adding the capcontrol
  makes even the controls-off gap 4 % purely from the bank's reset state, so it
  cannot isolate the fan-out. The regulator's integer tap has no such ambiguity.)
  No corpus/family deck qualifies for `full` (their regcontrol/capcontrol decks
  are `non-3ph-cut-only` or don't tear), so the sweep stays `full=0` — the
  capability is gated in `adiakoptics.rs` where synthesized fixtures live (§0.2).
- **off:ad-* bucket fully decomposed** (`DSS_AD_DECOMPOSE` on every deck), mislabels
  fixed, real AD bugs filed with specific reasons:
  - *AD vindicated → moved to save-roundtrip:* the distance/TD21 **relay** decks
    (8) had leg2 **2.3e-15** (AD clean) with leg1=2.4e3 → `off:save-roundtrip-relay`;
    the **autotrans** Auto1bus decks (3) had leg1>>1 (save reload broken) →
    `off:save-roundtrip-autotrans`.
  - *Out of AD scope:* the 3 **harmonics** IEEE_519 decks → `off:mode-outside-AD-scope`.
  - *AD↔normal gap classes, filed here and later shown to be upstream A-Diakoptics
    limitations (6 representatives driven on official r3723 — see the ad-bugs
    root-cause + settle record below in §1):* `off:ad-regulator-divergence`
    (ODRegTest leg2=1.5e15, TestDDRegulator leg2=0.88); `off:ad-switched-divergence`
    (IEEE123Switches leg2=19, civanlar/SecPar leg2=1.0 —
    open-switch/reconfiguration/meshed secondary); `off:ad-islanded-divergence`
    (10 GFM/GFL/ISource grid-forming microgrids — GFM/GFL have no firm source
    reference; the one ISource deck diverges in the torn main-feeder zone, ours
    milder than official); `off:ad-nonconvergent` (7 islanded GFM/GFL decks whose
    AD arm does not even init/converge). The generic `ad-singular-zone`/`ad-divergent`
    corpus labels are gone (the two synthesized *family* decks still use them).
- **Real AD-arm panic fixed** (was `off:ad-probe-panic`). `Sensor::update_current_
  vector`/`_for_wls` indexed `sensor_current[i]` for `i<nphases` while the buffer
  was still unsized (kW set mid-edit, before `RecalcElementData` allocates —
  reachable via a `save circuit`-emitted sensor). Now sizes the buffer to match
  Pascal's `AllocateSensorObjArrays` (transient; recalc re-zeros, TakeSample
  recomputes). The two sensor decks reclassify: `sensor_map` → `off:too-small`
  (2-line feeder, no two ≥2-bus zones), `midi_sensor` → `off:save-roundtrip-control`
  (interconnected save loses the metered `Line.bb1_2` reference).
- **Disposition validator hardened.** `ad_disposition_is_valid` now checks off:
  reasons against a closed `AD_OFF_REASONS` allowlist (was "any non-empty
  string") — a new deck can't invent an unreviewed reason to dodge the sweep, and
  the `ad-*` defect classes stay a bounded, greppable list.
- **GETCTRLMODE 1:1 fix.** `ad_send_get_ctrl_mode` set the child's
  `DefaultControlMode` from the coordinator's `DefaultControlMode`; official
  Solution.pas:3261 sets it from the coordinator's live `ControlMode`. Corrected
  (equal at every dispatch point, but now exact). Sweep `ad_solve_ad`'s `full`
  path no longer hardcodes `controlmode=static` — it re-asserts the deck's own
  declared control mode (symmetric with the normal arm).
- **pf/full comparison scope (node-V is sufficient).** The sweep compares node
  voltages, which on the shared interconnected network *determines* element
  currents (`I=Yprim·V`) and powers (`S=V·conj(I)`) — a stitch error leaving all
  voltages right but a flow wrong is not physically realizable for a shared
  element. Confirmed empirically: an element-power cross-check over every `pf`
  deck tracked the node-V gap with no independent divergence; its only
  above-node-floor residuals were transformer/line **loss** channels on the
  short-circuit decks (worst 2.2e-2 on `ieee37_SC_Currents` `line.l6` — the
  documented cancellation-floor class, not a gate-able quantity). The
  monitors/eventlog behavioral channel is gated by the `adreg` full test.

- **AD-master migration (EPRI_Ckt5-G/7-G, IEEE_123_Bus-G, ckt24 masters) + D9(d)
  `oracle:"r3723"` sweep cases: still DEFERRED (recorded, not silent).** These
  stay in `skipped_oracle_issue`/`skipped_needs_investigation`; `ad_sweep.json`
  carries no `oracle` field. Rationale: Part II has no pinned oracle by design
  (§0.2); D9(d) is an EPRI *reference* channel, and the plan places the Oddie
  AD-replay harness + EPRI reference-channel hardening in **WP-AD.5**. The AD
  numbers are validated (a) rust-vs-rust at the D7 tier — the WP's stated
  contract — across the 37 `pf` decks + the `adiakoptics.rs` D7 fixtures, and (b)
  the child control fan-out is now gated end-to-end by the `adreg` full test.
  Building the external r3723 AD-replay compare is the WP-AD.5 task.

**WP-AD.4 AD-BUGS ROOT-CAUSE ROUND (branch `ad-bugs`, base `2193bd0`, 2026-07-12;
settled 2026-07-12).** Root-caused the four `off:ad-*` AD-engine divergence classes
WP-AD.4 filed. **Verdict: they are UPSTREAM A-Diakoptics limitations, not dss-rs port
bugs** — the AD arm of the OFFICIAL r3723 engine, driven on the IDENTICAL cut, fails
the same or worse on every representative driven. Common root cause: each topology
makes `Tear_Circuit` isolate a zone with **no adequate in-zone voltage reference**, so
the child `hY` is singular / near-singular and the boundary stitch is solver-dependent
(faer picks a different null-space vector than KLU, or both blow up). This falsifies
the over-broad premise in `diakoptics/solve.rs` that "the loads' `Yeq` shunts anchor
every node to ground" — true only for wye/grounded loads.

Method (CLAUDE.md don't-rationalize-conditioning discipline + plan D7/D9): cheap D7
`DSS_AD_DECOMPOSE` repro per class (leg1=save round-trip, leg2=AD proper); then drove
**OFFICIAL r3723 AD via the Oddie bridge on the IDENTICAL cut** (`wait` after every
solve; child cut extracted from our engine's `link_branches`, forced on official with
`set LinkBranches=[…] + UseMyLinkBranches=True`) — the decisive ours-vs-upstream test.
Evidence below is this reference box's r3723 DLL, controls-off.

**Scope of the "upstream" proof (per-deck vs per-class).** SIX class representatives
were driven on official r3723 across the filing + settle rounds; this settle re-drove
**four** (ODRegTest, IEEE123Switches, TestDDRegulator, Microgrid/ISource), and civanlar
+ GFM_IEEE123 are carried from the filing round. The *magnitudes* are near-singular
blow-ups and are ill-conditioned — reproducible on this box (ODRegTest 2.9119e16 ×3,
IEEE123Switches 42.57 kV ×2, both matching the filing round's numbers, which validates
the method) but environment-sensitive: an auditor on a less-faithful setup (the
`123Bus/IEEELineCodes.DSS` `../` stub unresolved) measured ODRegTest ~2e17 /
IEEE123Switches ~61 kV. **The load-bearing invariant is the CHARACTER (no convergence
/ >10× over-voltage / hang), not the exact number.** The other members of these four
classes (islanded 10, nonconvergent 7+, switched 3, …) are inferred from the shared
topology mechanism, NOT individually driven — the per-deck official-AD replay that
would turn that inference into a running (report-only) check, and close the standing
coverage hole where `off:ad` decks are counted-not-solved in the sweep, is the tracked
**WP-AD.5** task, not a silent wontfix.

| Deck (cut) | Class | Our AD gap | Official r3723 AD, SAME cut | Root cause |
|---|---|---|---|---|
| `ODRegTest` (`Line.l2`) | ad-regulator | leg2 1.46e15 @ loadbus | **2.9119e16** @ loadbus.1, `conv=False` (worse) | loadbus zone = **delta-only** loads, no ground path → singular child Y |
| `TestDDRegulator` (`Line.line2`) | ad-regulator | leg2 0.88 @ regbus3.4 | AD-init OK then final solve **HANGS** (killed 90 s) | loadbus.2/.3 **floating** (phase-1-only wye load) → exactly singular; no answer on either |
| `civanlar` (single link) | ad-switched | leg2 1.0 @ bus1.1 | bus1 wrong on official too (3135 V vs ~13 kV) | **meshed** net (loops 5_11/10_14/7_16) — a single link cut cannot separate a mesh |
| `IEEE123Switches` (`Line.l58`) | ad-switched | leg2 19.5 @ 160r.1 | **42.57 kV** @ 79.2, `conv=False` (worse) | open switches strand a regulator boundary → 40 kV on a 2.4 kV system |
| `Microgrid/ISource` (`Line.650632`) | ad-islanded | leg1 2.01 / leg2 2.00 / **total 0.66** @ 634.1 | `conv=True` but **13.4** @ 634.1 (3935 V vs 273 V) — worse | NOT the ISource island (675/692 determinate & correct on both); the torn **main-feeder zone** (632/634/671 xfmr secondaries) reseed diverges |
| `GFM_IEEE123` (auto-tear) | ad-nonconvergent | AD init refuses | blows up (official auto-tear) | GFM inverter = PC current injection, no Y reference |

Two rows corrected by this settle vs the filing round: (a) **TestDDRegulator** —
official does NOT give a "bounded 7.2 kV"; on this box the AD solve *hangs* (child
zone exactly singular). Neither engine yields a meaningful AD answer — "both valid up
to null space" becomes "neither produces a usable AD answer." (b) **Microgrid/ISource**
— the earlier row's official column was an ours-side inference ("island degenerate")
and mislabeled the mechanism. Driven on official at our exact cut it is a REAL
measurement: the ISource island is determinate (wye loads ground it) and correct on
both engines; the AD divergence lives in the torn main-feeder transformer-secondary
zone, and **ours (0.66) is milder than official's (13.4)**. Its D7 legs are
co-equal (leg1 2.01 ≈ leg2 2.00, total 0.66) so it is neither a clean save-roundtrip
nor a clean AD-leg deck; it stays `off:ad-islanded-divergence` by deck type (an
ISource microgrid whose AD arm exceeds tier), with zero gate effect either way.

The dispositions stay `off:` (correct: singular/near-singular torn zone → no correct
AD answer on either engine). Reframed everywhere: `AD_OFF_REASONS` comment
(`corpus_live.rs`), the `ad_sweep.json` overlay comment, and the `NOTE(upstream-quirk)`
block at the AD solve site (`diakoptics/solve.rs`). No behavioral code change — our
engine already keeps these off the gate; and on every representative driven, our
failure mode is no worse than official's. Reproduction recipe (any future auditor):
`DSS_AD_DECOMPOSE=<rel-path>` for the D7 legs + our cut, then the Oddie sequence above
against `tools/opendss/bin/r3723/OpenDSSDirect.dll` (durable per-deck replay = WP-AD.5).


**DIAKOPTICS_PSTCALC Part I COMPLETE (2026-07-11), gate-green.** WP-PF.1 + WP-PF.2 +
WP-AD.1 executed as an ultracode round (3 opus executors + 6 opus auditors + settle
agents in isolated worktrees; coordinator merged `wp-pf2-flicker-settled` (ff),
`wp-ad1-incmatrix-settled` (`e96caaa`), `wp-pf1-pstcalc-settled` (`be2cfc7`); one
trivial conflict in `support/mod.rs`). Full gate on the merged tree: fmt + clippy
clean, `cargo test --workspace` exit 0 (953 lib units, corpus_live 14/14 incl. the
three family gates, all binaries 0 failed; the single "1 ignored" is the pre-existing
`obj/props` doctest). Per-WP records below. `PLAN_SEQUENCE.md` stage 3 is done — next
per the sequence is FINAL ACCEPTANCE (user decision), then UPGRADE_PLAN; Part II
(A-Diakoptics, WP-AD.2–AD.6) stays sequenced after MULTITHREADING M2.

**WP-PF.1 — `Pstcalc` executive command (2026-07-11), gate-green.** `support/pstcalc.rs`
(f64 `PstEngine`, IEC-868 path of `Shared/Pstcalc.pas` 1–474) + `exec/pstcalc.rs`
(`DoPstCalc`, ordinal 96), error #28723 verbatim (upstream "Insuffient" typo kept).
FPC semantics probe-proven end-to-end: `power(x,4)=sqr(sqr(x))` via intpower (docs in
module), banker's `Round` at the two `TODO(compat)` spots; D5 UB clamps in
`SB`/`Gather_Bins` (unreachable one-past-allocation paths; bit-identical reachable
behavior). Gates: `golden_pstcalc.rs` — 25-case oracle golden, **byte-exact**
`GlobalResult` strings across npts/shape/lamp/freq/dt (incl. fractional dt); 6 unit
tests; family deck `modes/pstcalc_cmd.dss` (live no-error + full-model compare).
Audits: code NO FINDINGS; tests 2 Minor — the `compare_global_result: true` flag was
inert (harness captures GlobalResult after its own solve, which clears it on both
engines; oracle-probe-proven) → settled by coordinator (`a45b74f`): flag dropped, note
rewritten. Lane settle agent stalled waiting on a monitor (its branch = exec commit;
nothing was lost — audits had no code findings).

**WP-PF.2 — Monitor mode 4 flicker (2026-07-11), gate-green.** Mode-4 sample body
(`monitor/sample.rs`), `post.rs` `DoFlickerCalculations` (in-place channel rewrite:
Flk←Block-4 level, Pst←600 s interval value, exact ipst/tpst stepping), `PostProcess`
latch wired at the `to_csv`/export/show sites; `support/flicker.rs` = the f32
`FlickerMeter` (Pstcalc.pas 476–687) with the proven Single-storage/Double-arithmetic
model (settle `6e67121`: window-control expressions compute in f64 over f32 operands;
Sterbenz-lemma probe shows the `(t−tPst)≥600` half is unobservable, the
`trunc(600/ts)` half diverges at ts∈{0.05,0.1,…} and the f64 form matches Delphi
exactly). **New upstream-bug finding: the pinned oracle (dss_capi 0.14.5) CRASHES on
mode-4 post-processing** — `export monitor`/`Process` raises access violation #671
(Terminals OOB) while raw `Channel()` reads work. Consequence (audit-verified
legitimate): the mode-4 CSV golden (`golden_flicker.rs`, 8640×3 samples from the
recreated `Examples/Matlab/pst.dss` demo) is a **committed r3723 reference** per
DIAKOPTICS_PSTCALC_PLAN D9(b) — static, replayed at test time, no live EPRI gating;
the family decks `controls/monitor_pst.dss` + `midi_monitor_pst.dss` omit
`export monitor` and live-gate the sampled channels via `check_meters_monitors`
(oracle `Channel()` path, micro tier). `Action=Process` stays a parse-time no-op per
the meter-wide convention (EnergyMeter identical); doc corrected. Follow-up candidate:
document the oracle mode-4 crash in `investigations/` (deterministic upstream crash,
not reproduced — the Rust port post-processes correctly).

**WP-AD.3 — A-Diakoptics engine (in progress, staged; branch `wp-ad3`).**
Stage list: (1) matrices ✅ · (2a) init machine + matrices-on-real-coordinator +
options + get_Statistics ✅ · **(2b) the AD solve stitch ✅** · (3) exports 58–61 ✅
· **(4) D7 calibration + snapshot/time-series + EPRI/r3723 refs ✅** (settle round 2).

**Stage 3 exports (gate-green):** `Export ZLL|ZCC|Contours|Y4` (keywords 58–61,
`report/export/adiakoptics.rs`, official `ExportResults.pas:3541–3627`) —
compressed-coordinate CSV (`Row,Col,Value(Real), Value(Imag)`; Contours is real-part
only, `Row,Col,Value`), default files `ZLL.csv`/`ZCC.csv`/`C.csv`/`Y4.csv`, values
via FPC `float_to_str`. **When `ADiakoptics=false`** the export *body* is a no-op
(Pascal `if ADiakoptics` — no file, `GlobalResult` untouched), but `DoExportCmd`'s
tail (`ExportOptions.pas:503–507`) still sets `LastResultFile`/`@lastfile`/
`@lastexportfile` to the resolved (never-written) path **unconditionally** — now
reproduced 1:1 (was previously a total no-op). Gates: header + per-line VALUE match
vs the built matrices (ZLL/ZCC/Y4 float fields, not just the field count), Contours
±1 real-only, and the false-flag path (last-file set, no file on disk, empty
GlobalResult). Keywords registered in `EXPORT_OPTIONS` as a recorded departure
(compiled out of the pinned oracle, §0.2).

**Stage 1 (gate-green):** the four matrix builders in `exec/diakoptics/matrices.rs`
— 1:1 port of official `Diakoptics.pas` (D10): `Calc_C_Matrix` (contours, substring
node lookup D5), `Calc_ZLL` (inverted 3×3 link-Yprim self-block), `Calc_ZCC`
(per-column `Y_torn·z=c` via the cached `dss-sparse` factorization → ZCT, then
`ZCC = Contoursᵀ·ZCT + ZLL`, `re≠0 AND im≠0` drop D5), `Calc_Y4` (`ZCC⁻¹` via
`CMatrix::invert`, the double-`.re` drop D5) + `AdMsg` + `ad_find_element`. 4 unit
tests recompute the D1 invariants on a tiny inline link feeder. `NOTE(upstream-quirk)`
at each D5 site.

**Stage 2a (gate-green):** the `ADiakopticsInit` state machine (`exec/diakoptics/
engine.rs`, `Diakoptics.pas:541`) — states 0–9: tear (`ADiakoptics_Tearing`, shared
with the `Tear_Circuit` cmd) → `ClearAll` + recompile `Torn_Circuit/
Master_Interconnected.dss` into the coordinator + build each child zone engine
(`ad_children: Vec<Dss>`, D3 ownership) + disable `zone_*` meters + open link branches
+ build the torn Y + `Calc_C/ZLL/ZCC/Y4` on the REAL interconnected coordinator +
`SendIdx2Actors` + close links + `get_Statistics` + the progress-string summary.
Options wired: `set ADiakoptics=yes` → init (deferred past `do_set_cmd`'s field
borrow via a `pending_ad_init` flag); `=no` clears the flag only; `get ADiakoptics`;
`Solve` resets `AD_Init`. Integration gates on the midi feeder (2 zones): flag flip
+ summary, Contours ±1-per-column, ZLL block, **Y4·ZCC ≈ I to 1e-6 with ZCT populated
(369 nz)** + the D1(a) `ZCC = CᵀZCT + ZLL` re-derivation recomputed on the real init
(non-circular, catches a bad transpose/RHS/ZLL that `Y4·ZCC≈I` cannot), the
`get_Statistics` **value golden** (46.34% reduction / 13.64% max imbalance / 6.818%
avg; `fmt_g`=`floattostrf(ffgeneral,4)` + Pascal f32-array narrowing per D4),
`=no` clears flag-only, and init-without-prior-solve fails. The CPU clamp
(`Num_SubCkts ≤ CPU_Cores−2`) is ported → AD gates assume ≥4 cores (D6).

**Stage 2b (gate-green — the AD solve stitch).** `ad_solve` dispatch (Direct →
`SolveDirect` AD branch; Snapshot → the `SolveSnap` control loop wrapping the AD
`DoNormalSolution` fixed-point; Daily/Yearly/Duty/Peak/Time → coordinator clock-step
re-entering the snapshot solve); `Solve_Diakoptics` coordinator stitch (SOLVE_AD1 →
`Vpartial`=contour-pair NodeV diffs → `Y4·Vpartial` → `Ic=Contours·Vpartial` →
SOLVE_AD2); `ad_init_actors` = `INIT_ADIAKOPTICS` (`Start_Diakoptics` for actors > 2
+ `IndexBuses` on every child); the child-side `solve_ad`/`update_isrc`/
`ad_solve_into_parent` driven (were `#[allow(dead_code)]`). Newton is NOT AD-aware
(verified: official `DoNormalSolution` only branches to `Solve_Diakoptics` on the
fixed-point path) → an AD deck set to Newton falls through to the per-child
fixed-point, documented. The child `DO_CTRL_ACTIONS` fan-out was deferred to
WP-AD.4 — **now landed** (`ad_check_controls` ports the official AD branch: the
coordinator fans `DO_CTRL_ACTIONS` to the children + ANDs their
`ControlActionsDone`; see the WP-AD.4 record at the top of §1).

**r3723 Oddie probe (re-run by the resume executor, own transcript, 2026-07-12;
`solve mode=snap`, `controlmode=off`, `Num_SubCircuits=2`; scripts in scratchpad
`ad_probe3.py`/`ad_probe_childv.py`/`probe_state2.py`).** Settles the two blocking
questions:
- *Child Y non-singularity:* with `Start_Diakoptics` disabling a zone's sources, the
  loads' `Yeq` shunts (stamped into Y as the fixed-point accelerator) anchor every
  node to ground → the reference-free zone is near-singular but solvable; faer factors
  it, **no** KLU tiny-pivot/regularization is involved. Port uses ordinary faer with no
  guard; a genuinely singular Y → normal `SolutionAbort` (never silently regularized).
- *Child voltage maintenance:* official **FREEZES** each child's own `NodeV` at its
  state-2 standalone solve for the entire AD run — `SolveSystem` writes only into the
  coordinator array (**proven**: macro actor-3 `NodeV` moves `0.000e+00` between init
  and the post-AD read). That frozen state-2 solve is already within `7.9e-5` of
  interconnected (the reference-free zone; the source zone's isolated solve is 25% off
  at its cut node — it lacks the downstream current — but that node is corrected by the
  boundary `Ic`). The earlier (crashed-draft) claim that official "tracks" the child
  was a misread of that 5e-5 residual.
- *Method floor + tolerance stability:* AD-vs-normal max rel `|V|` is a **stable** floor
  that does NOT collapse as tol tightens 1e-4 → 1e-10 — midi 3.265e-5 → 3.254e-5, macro
  1.318e-4 flat; iteration counts `itN == itA`. Proof the engines share the fixpoint
  (D1 leg 3 / D7). Rust matches: midi 3.21e-5 (oracle 3.25e-5), macro 1.319e-4 (oracle
  1.318e-4). Tiers ×4 recorded in `tests/TOLERANCE_NOTES.md` §AD; permanent tighten-proof
  tests `{midi,macro}_d7_gap_stable_under_tighten`.

**Salvage/reset ledger (resume protocol).** The crashed executor's ~815-line dirty
draft was competent and gate-green; evaluated file-by-file against official r3723 +
re-run probe: **SALVAGED** `engine.rs` (init wiring), `solve.rs` (dispatch/stitch —
verified loop-for-loop vs `Solve_Diakoptics`/`SolveAD`/`Start_Diakoptics`/`IndexBuses`),
`mod.rs`/`time_series.rs` (re-exports), the `tests/adiakoptics.rs` D7 gate, and
`power_flow.rs`'s scatter parent-write. **CORRECTED** the `ad_solve_into_parent` child
re-seed: kept the line (it recovers the oracle floor — a brief-sanctioned faer↔KLU
compensation on the near-singular reference-free zone) but **rewrote its false
justification** (official freezes, does not track) with the honest probe result, per
`ad_solve_into_parent`/`solve.rs` module docs. Confirmed by experiment: a byte-faithful
freeze (no re-seed) passes midi (3.80e-5) but diverges macro to 3.46e-3 @ M180 (26× the
floor). Open item for auditors/WP-AD.4: root-cause the reference-free-zone faer↔KLU gap
so the re-seed can be dropped.

**Stage 4 official-reference gate (D9 b/c) — IEEE-13 + IEEE-123 ✅.**
`tests/ad_reference.rs` replays TWO EPRI AD examples with the identical **manual**
cut on both engines and compares the built matrices **and** the post-AD SOLVED node
voltages against **fresh r3723 references** harvested by
`tools/opendss/gen_ad_reference.py` (committed under `tests/data/adiakoptics/
r3723_ref/{ieee13,ieee123}/` with `PROVENANCE.txt`):
- **IEEE-13** (`Line.670671`, 2 zones): ZLL 3.8e-15, ZCC 6.2e-8, Y4 2.7e-8, **41
  solved node voltages 1.2e-6** — all faer↔KLU last-ulp.
- **IEEE-123** (r3723's own auto-tear links `[Line.l10, Line.l73]` → 3 zones, TWO
  reference-free = the multi-link D9c case): ZLL 3.0e-15, ZCC 7.4e-10, Y4 4.9e-10,
  **278 solved node voltages 4.85e-6**.

The solved-voltage legs close audit finding #7 (the AD SOLVE output now has an
external trusted-baseline gate, not just AD↔normal self-consistency) and #8 (the
IEEE-123 multi-link D9c harvest+compare). `gen_ad_reference.py` grew a `--tree`
(subdir-redirect decks) + comma-separated `--link` (multi-link) mode and now exports
`voltages.csv` (post-AD actor-1 NodeV). Finding retained: the trunk's own
`References/SolveDirect/ADiakoptics_matrixes/*.csv` are **STALE** (older deck
revision, ~20% reactance drift), so the gate pins fresh harvests, never the trunk CSVs.

**WP-AD.3 audit settle (opus-xhigh).** Findings settled empirically against official
r3723:
- *get_Statistics formatting (Major, fixed):* hand-rolled `fmt_g42` replaced with the
  FPC-bit-exact `crate::util::fmt_g(x, 4)` = `floattostrf(ffgeneral,4)`; and the
  `unbalance/ASize : Array of single` f32 narrowing reproduced per D4 (`GReduct/
  MaxImbal/AvgImbal : Double`). Output unchanged on midi (46.34/13.64/6.818), now
  pinned as a **value golden** (was determinism+substring only).
- *AD-off export (Minor, fixed):* the `export_ad` no-op was total; Pascal
  `DoExportCmd`'s tail still runs `SetLastResultFile`+`@lastexportfile` (only gated by
  `Not AbortExport`). Now sets the last-file state to the resolved never-written path,
  body still skipped — truly 1:1; test + doc corrected.
- *State-2 child abort (Minor, aligned):* the check added `!errors().is_empty()` on top
  of `SolutionAbort`; Pascal (Diakoptics.pas:644) breaks on `SolutionAbort` only, and a
  benign `DoSimpleMsg` child message does not set it — so the extra arm would spuriously
  fail init where official proceeds. Narrowed to `SolutionAbort` + a no-circuit clause
  (the Rust analog of a nil child actor after a total compile failure).
- *D5 drop quirks unpinned (Major, fixed):* the fixture R+jX topology never yields a
  drop-eligible entry, so the integration assertions held vacuously. The two quirks are
  now extracted to `zct_keep`/`y4_keep` and pinned directly by unit tests fed
  drop-eligible values (the doubled-`.re` Y4 bug: `re=0,im≠0` dropped) — a "cleanup" to
  `re≠0 OR im≠0` fails them.
- *ZCC assembly baseline (Major, partially fixed):* only the circular `Y4·ZCC≈I` existed;
  added the non-circular D1(a) `ZCC = CᵀZCT + ZLL` re-derivation to BOTH the unit test
  and the real init. The `ad_children` field doc was corrected (it is rebuilt each init,
  not cleared by `=no`/`Clear`).
- *Stage-4 external-baseline gate (was deferred): now CLOSED* — see the Stage 4
  record above (IEEE-13 + IEEE-123 matrices + solved voltages vs fresh r3723).

**WP-AD.3 audit settle ROUND 2 (opus-xhigh, 2026-07-12).** The Stage-2b/Stage-4
audit's 8 findings settled empirically against official r3723 (Oddie):
- *Newton+AD dispatch (Major, FIXED + finding refuted):* `ad_solve_snap` now honors
  `Set algorithm=Newton` — official `DoPFLOWsolution` (Solution.pas:1125) dispatches
  `CASE Algorithm`, and `DoNewtonSolution` has NO ADiakoptics branch (`SolveSystem(dV,1)`
  = full `@V[1]`), so Newton solves the *closed interconnected coordinator* directly.
  Ported via a faithful `ad_do_pflow_solution` (Newton → coordinator `do_newton_solution`;
  default → the `Solve_Diakoptics` fixed point) and the false "matches upstream" comment
  corrected. **Refutes the finding's premise** that Newton and fixed-point AD "differ by
  the method floor": A-Diakoptics is EXACT, so both land on the same interconnected
  fixpoint to f64 ulp (midi 7e-13, macro 1.3e-12; `{midi,macro}_newton_ad_matches_
  fixedpoint_ad`).
- *Re-seed conditioning "asserted not proven" (Major, PROVEN + doc corrected):* the
  Newton path is an independent in-engine ground truth (full coordinator solve, no
  children/re-seed) — the re-seeded fixed-point matches it to f64 ulp, *proving by
  decomposition* the re-seed recovers the exact interconnected answer. This also
  **corrected two false claims**: the AD floor is NOT a "first-order boundary
  approximation / fixed distance" (the stitch is exact) — it is the interconnected-
  coordinator-vs-original-deck difference, shared by all AD paths and oracle-matched;
  and the frozen-divergence faer↔KLU attribution is downgraded from asserted fact to an
  explicitly-labeled, not-yet-bit-proven hypothesis (open item retained). Docs at
  `ad_solve_into_parent` + `solve.rs` + `TOLERANCE_NOTES §AD`.
- *DoPFLOWsolution head drops (Minor, FIXED):* `ad_do_pflow_solution` now runs the
  per-control-iteration `Inc(SolutionCount)` + `VoltageBaseChanged→InitializeNodeVbase`
  guard (Solution.pas:1092-1094), previously omitted.
- *AD solved voltages no oracle gate (Minor→closed by #7):* the AD SOLVE output is now
  oracle-gated (see Stage 4 voltage legs), no longer self-consistency-only.
- *IndexBuses past-end value (Minor, FIXED):* `past_end` corrected `src_bus.len()+1`
  → `+2` to byte-match Pascal's `LocalBusIdx := j+1` after the completed search (SrcBus
  carries a trailing empty slot; unreachable/bounds-checked, cosmetic fidelity).
- *Time-series D7 gates absent (Major, FIXED):* added `midi_daily24_matches_normal` +
  `macro_yearly168_matches_normal` — they drive `ad_solve_time_series` (previously ZERO
  coverage), assert the clock advanced the full horizon, and pin the §AD floor.
- *No post-AD oracle voltage compare (Major, FIXED) + IEEE-123 multi-link (Major, FIXED):*
  see the Stage 4 record — findings #7 and #8 both closed.

**WP-AD.1 — incidence matrix + Sparse_Math + exports 53–57 (2026-07-11), gate-green.**
`support/sparse_math.rs` (SparseInt/SparseComplex 1:1 COO: insert
accumulate-else-append, insertion-order storage, multiply/add drop quirks
`re<>0 AND im<>0` pinned at Pascal :654/:868, Rank row-echelon walk incl. the last-row
off-by-one), `solution/inc_matrix.rs` (`Calc_Inc_Matrix`/`Calc_Inc_Matrix_Org` with
hierarchical ordering + `Inc_Mat_levels`, the four element walks incl. series-only
Capacitor/Reactor filters), commands `CalcIncMatrix`/`CalcIncMatrix_O`/`CalcLaplacian`
(#8877 "Indidence…" verbatim), exports 53–57 byte-exact with oracle default filenames
(`Inc_Matrix.csv` … `Laplacian.csv`, ExportOptions.pas:417–425). Gates: 28 goldens
over IEEE13/IEEE123 + purpose-built series-cap/series-reactor decks via
`tools/golden/gen_inc_matrix.py` — the tests auditor independently regenerated all 28
from the pinned oracle: **0 diffs**; 21 tests incl. negative #8877; settle `351fefe`
added filename pins + 4 hand-traced complex-op unit tests. Audits: code NO FINDINGS;
tests 2 Minor → fixed in settle. `Refine_BusLevels` stays refused (AD-gated → Part II).

**WP-AD.2 Stage A — `dss-metis` crate (2026-07-11), gate-green (COMPLETE).**
Workspace crate `crates/dss-metis` (`#![forbid(unsafe_code)]`), the safe-Rust 1:1
source port of the METIS 5.2.1 `METIS_PartGraphKway` -> `MlevelKWayPartitioning`
path (plan D2). The **whole pipeline** is ported and the open item is closed:
`part_graph_kway(xadj, adjncy, vwgt?, adjwgt?, nparts) -> (part, edgecut)` replays
every committed `.part.N` golden **bit-exact** (k in {2,3,4,8} over all 6 fixtures =
24/24; `tests/golden_part.rs`), and its returned edgecut equals the C driver's for
all 24 (independent cross-check + induced-cut self-consistency).
- `rng.rs` — GKRAND MT19937-64 + `GK_MKRANDOM` ops, pinned bit-exact vs the C build.
  The single global stream is re-seeded to 4321 at each `SetupCtrl` (kmetis entry,
  then again inside `InitKWayPartitioning`'s `METIS_PartGraphRecursive`), and
  `RefineKWay` continues that stream — modeled exactly.
- `graph.rs` — the `.graph` reader/writer + CSR (`io.c::ReadGraph`/`WriteGraph`).
- `pqueue.rs` — the GKlib bucket-locator binary max-heap (`rpq`, `gk_mkpqueue.h`);
  `sort.rs` — the GKlib inline quicksort (`ikvsorti`, `gk_mksort.h`, glibc-derived,
  unstable → the equal-key order is part of the contract).
- `part/` — `SetupCtrl`/`CheckParams` (`options.c`), `SetupGraph` (`graph.c`),
  `CoarsenGraph` SHEM/RM + 2-hop (`Any`/`All`) + `CreateCoarseGraph` htable/dtable
  contraction (`coarsen.c`, `bucketsort.c`), `MlevelKWayPartitioning` +
  `InitKWayPartitioning` (`kmetis.c`), the recursive-bisection bootstrap
  (`pmetis.c` `MlevelRecursiveBisection`/`MultilevelBisect`/`SplitGraphPart`,
  `initpart.c` Grow/Random bisection, `fm.c` `FM_2WayCutRefine`, `balance.c`
  Bnd/General2WayBalance, `refine.c` project/params), and greedy k-way refinement
  (`kwayrefine.c` project/params/boundary, `kwayfm.c` `Greedy_KWayCutOptimize` with
  the `UpdateMovedVertexInfoAndBND`/`UpdateAdjacentVertexInfoAndBND`/`UpdateQueueInfo`
  macros inlined). `idx_t=i32`, `real_t=f32`; every mixed int/float expression keeps
  the C's implicit-conversion order (f32 rounding order is load-bearing).
- **Reachability (default path, `ncon==1`):** `contig`/`minconn` (`contig.c`/
  `minconn.c`), the volume objective, `BlockKWayPartitioning` (`dbglvl&512`),
  `dropedges`, and every multi-constraint routine are proven unreached (ctrl flags
  0 / all fixtures single-constraint) and deliberately not ported — documented at
  the call sites + `part::mod`.
- **Robustness:** beyond the 20 committed goldens, the port was cross-checked
  bit-exact against the C original on 190 additional off-corpus combos (radial to
  2000 vtx, meshes to 50×25, Erdős–Rényi, star clusters forcing 2-hop, a
  disconnected 3-component graph forcing the BFS-restart, weighted+unweighted
  forcing SHEM vs RM, k up to 32) via a throwaway scratch harness (not committed) —
  0 divergences.
- Golden infra: fixtures + `.part.{2,3,4,8}` generated OFFLINE from the C original
  (MinGW gcc 13.2.0 + libmetis static, minimal gpmetis-default driver); procedure
  in `tools/golden/gen_metis_reference.md`. The C original reproduces all 24
  committed goldens bit-exact.

Stage B (tearing / `.graph` engine round-trip, `Create_MeTIS_Zones`) can now build
on the completed `part_graph_kway`.

**WP-AD.2 Stage B — tearing machinery, COMPLETE (2026-07-11, gate-green).**
Behavioral spec = official r3723 Delphi (`Common/Circuit.pas`, plan D10). The
partition + zone machinery (part 1) plus the torn-file emission + zone meters +
PConn (this WP) are both landed; the earlier follow-up deferral is CLOSED (see the
Stage-B completion record below). Landed:
- **Circuit AD fields** (`circuit/tearing.rs::AdTearing`, wired as `Circuit.ad`):
  `Coverage`/`Actual_Coverage`, `Num_SubCkts` (ctor default `CPU_Cores-1`, D6),
  `Link_Branches`, `PConn_Names`/`PConn_Voltages`, `Locations`, `BusZones`,
  `MeTISZones`, `UseUserLinks`, `VIndex`, and the `SparseComplex` matrix slots
  `Contours/ContoursT/ZLL/ZCT/ZCC/Y4/Ic` as typed WP-AD.3 placeholders
  (Circuit.pas:205–231/321).
- **`Create_MeTIS_graph`** 1:1 (`exec/tearing.rs::build_metis_graph`,
  Circuit.pas:1213): incidence (hierarchical `Calc_Inc_Matrix_Org`) → per-column
  dedup of parallel branches → phase-count edge weights (Transformer weight 1).
  The OpenDSS `.graph` text writer (`support/partition.rs::write_opendss_graph`)
  reproduces the byte-exact quirky format incl. the dropped column-0 line
  (`NOTE(upstream-quirk)`).
- **`support/partition.rs`** — the file round-trip glue over `dss-metis`
  in-process (`NOTE(subst-metis)`: exec→in-process + METIS 4.0→5.2.1 step; the
  upstream `GetNumEdges` repair loop is not ported — our edge count is exact
  because we partition the canonical symmetric graph, not the header-corrupted
  text). Writes `<graph>.part.<N>` in kmetis output format. dss-core now depends
  on `dss-metis`.
- **`Create_MeTIS_Zones`** parsing 1:1 (Circuit.pas:1350): the D5 first-line-swap
  quirk (`NOTE(upstream-quirk)`), the ≥2-consecutive-bus zone rule, `Locations`/
  `BusZones` fill, the final `inc(Locations[j])`.
- **`Tear_Circuit` both branches** (Circuit.pas:1880): auto (`dss-metis`) and the
  official manual-links branch (`get_PDE_Bus1_Location`, `get_line_bus`);
  `Link_Branches` from `Locations` via `get_IncMatrix_Row` (the +1-adjusted
  offset reproduced). Result string `"Sub-Circuits Created: N"` (Diakoptics.pas:526).
- **Executive surface**: the `Tear_Circuit`/`AggregateProfiles` commands and the
  `Num_SubCircuits`/`Coverage`/`LinkBranches`/`UseMyLinkBranches`/`ADiakoptics`
  options are **compiled out of the vendored/oracle build** (§0.2), so they are
  absent from `EXEC_COMMANDS`/`EXEC_OPTIONS` (which the oracle-pinned `Dump
  commands` golden mirrors byte-exact). Registered here by **dispatch
  interception** (`command.rs`, `set_cmd.rs`, `get_cmd.rs`) as a recorded
  departure — the engine behaves like a `DSS_CAPI_ADIAKOPTICS` build without
  perturbing that golden. `set ADiakoptics` and `AggregateProfiles` are scoped
  refusals pointing at WP-AD.3/AD.5.
- Tests: `crates/dss-core/tests/adiakoptics.rs` — synthesized radial 3-phase midi
  (~40-bus) + macro (~200-bus) feeders, `set Num_SubCircuits=2/3; Tear_Circuit`,
  asserting zone count + `GlobalResult`, link branches are 3-phase Lines,
  balanced `.part.N`; manual-links cut; 1-zone request; option set/get round-trip.
  Unit tests in `support/partition.rs` + `exec/tearing.rs` (graph text byte-exact,
  other-terminal pairing, zones split, class-prefix).

**Stage-B completion — torn-file emission + zone meters + PConn (2026-07-11,
this WP, gate-green).** Closes the earlier follow-up deferral. `Tear_Circuit` now
runs the full official `ADiakoptics_Tearing(AddISrc=False)` orchestration
(Diakoptics.pas:511–534) and writes the on-disk `Torn_Circuit/` sub-project tree.
- **Zone `EnergyMeter` placement + `PConn` capture** (`exec/tearing.rs::
  place_zone_meters`, Circuit.pas:1941–2032): a prior-solve gate (`converged_flag`
  — errors honestly if the power flow never converged, since PConn reads
  `Solution.NodeV`); disables all pre-existing meters; per location derives the
  link PDE (`Inc_Mat_Rows[get_IncMatrix_Row]`), the point-of-connection bus via
  `get_Line_Bus(link,2)` (**Lines-only** search — a non-Line link reports error
  5008 "Line not found", matching official), the 3-phase `PConn_Voltages`
  (`ctopolardeg(NodeV)` → mag/1000, angle°), and issues `New EnergyMeter.Zone_<i+1>
  element=<PDE> terminal=1 option=R action=C`. The vestigial `Term_volts[0] -
  Term_volts[1]` |V| difference (computed-but-never-read in r3723; terminal is
  hard-coded 1) is documented `NOTE(upstream-quirk)` and not reproduced (D5).
- **Torn-file emission** (`exec/tearing_save.rs`): `Save_SubCircuits` (fresh
  `Torn_Circuit` dir + reuse of `exec/save_circuit.rs` `save circuit`),
  `Format_SubCircuits` (`Master_Interconnected.dss` support-line filter +
  per-zone `Master.dss` + per-zone `VSource.dss` from the measured PConn via
  `fmt_g(v,8)` = FPC `floattostrF(ffGeneral,8,3)`), `AppendIsources` (the
  A-Diakoptics `AddISrc=TRUE` edge sources — ported though the tear path passes
  FALSE), `Disable_All_DER` verbatim (WP-AD.3 caller). `NOTE(subst-metis)`: the
  filter is matched case-insensitively and the zone-header cut is anchored on the
  `New Circuit` line, because our round-trip-faithful save master casing/header
  differs from the official DSS `Save` — the structure otherwise matches the
  vendored official `ckt24/Torn_Circuit` reference exactly (validated by eye).
- **Gates** (`tests/adiakoptics.rs`, committed fixtures `tests/data/adiakoptics/
  {midi,macro}.dss` reusable by AD.3/AD.4): committed byte-stable Torn_Circuit
  golden (`tests/golden/adiakoptics/midi_torn_tree.txt`, regen
  `DSS_REGEN_AD_GOLDEN=1`); round-trip compile+solve of the interconnected + every
  per-zone master (converged, sane voltages); zone-**connectivity** recompute from
  `.graph` adjacency + `.part.N` labels; link branches asserted as real 3-phase
  `Line` elements via the engine (not a name-prefix check); negative paths
  (transformer manual link → "Line not found"; tear before solve → honest error);
  + the pre-existing count/balance/dedup/option tests migrated onto the fixtures.
  A `#[ignore]`d `ckt24_graph_diagnostic` records the vendored `.graph` shape.
Gate: fmt + workspace clippy (`-D warnings`) clean; `cargo test --workspace`
(pinned live oracle) exit 0.

**Stage-B completion — audit settlement (2026-07-12, gate-green).** Two auditors
(code + tests) filed 7 findings against the completion; each settled against the
official r3723 Delphi source (D10).
- **Zone masters dropped `Set DefaultBaseFreq` (Minor, real bug — FIXED).** Our
  round-trip-faithful save emits `Set DefaultBaseFreq` *before* `New Circuit`
  (a `NOTE(subst-metis)` addition the official `SaveMasterFile` omits, so the
  circuit picks it up at `TDSSCircuit.Create`, `Fundamental := DefaultBaseFreq`,
  Circuit.pas:416). Zone-`k` masters anchored their global section on the
  `New Circuit` line, so that pre-header line was dropped — zone-1 and
  `Master_Interconnected.dss` kept the deck frequency while zones 2+ silently
  defaulted to 60 Hz (latent for any non-60 Hz AD deck; not triggered by the
  all-60 Hz fixtures). Fixed: `tearing_save.rs::zone_pre_header` re-emits the
  `Clear`…`New Circuit` header lines before each `New Circuit.Zone_k` so all
  sub-circuits are frequency-consistent. Golden regenerated (one added line in
  `zone_2/Master.dss`); new unit test `zone_pre_header_carries_default_base_freq`.
- **PConn boundary sources pinned only by the self-golden (Major — FIXED).**
  Added `pconn_sources_match_solved_nodev`: an **independent** numeric cross-check
  that re-derives each zone's point-of-connection from the link `Line`'s bus-2 and
  its boundary voltage from the *solved* `NodeV` (public bus API), then asserts the
  **emitted** `VSource.dss` `basekv`/`angle` match (with a ~7.2 kV L-N sanity
  bound ruling out a `/1000` slip). Catches wrong-terminal / wrong-bus / angle-sign
  / scale errors the byte-golden alone would freeze in.
- **`VSource.dss` case-insensitivity dependency (Minor — recorded, no fix).** The
  boundary source is written to `VSource.dss` (capital S, 1:1 with official
  `Format_SubCircuits`) while the copied support redirect names `Vsource.dss`;
  these coincide only on a case-insensitive FS (Windows/NTFS = the official DSS +
  this project platform, D10). Inherited verbatim from upstream — changing the
  emitted case would diverge from official. Documented `NOTE(upstream-quirk)` at
  `write_zone_vsources`.
- **`get_Line_Bus` not-found path (Minor — recorded, no fix, D5).** Official falls
  through to the *restored* previously-active element's bus (a stale, state-
  dependent read, Circuit.pas:1204–1206); the port yields an empty
  point-of-connection + the honest 5008 error instead. D5: state-dependent reads
  not reproduced. Comment added at the call site.
- **No cross-check vs the vendored official `Torn_Circuit` reference (Minor —
  tracked TODO(WP-AD.3)).** The D9(b) reference-fixture harvest (cross-checking the
  two deliberate `Format_SubCircuits` deviations against
  `Examples/ADiakoptics/ckt24/Torn_Circuit/**`) is WP-AD.3 scope; TODO marker added
  at `torn_tree_matches_golden`.
- **`ckt24_graph_diagnostic` builds no our-side graph (Minor — tracked
  TODO(WP-AD.5)).** Plan-sanctioned log-only; the "our vs vendored" `.graph` diff
  needs the ckt24 master-prefix compile driver (WP-AD.5). TODO marker added.
- **Round-trip is solvability-smoke (Minor — deferral made explicit).** Numeric
  AD↔normal equivalence at the §AD tier is D7/AD.3; the boundary values themselves
  are now numerically pinned by `pconn_sources_match_solved_nodev`. TODO(WP-AD.3)
  noted at `torn_tree_roundtrip_solves`.
Gate: fmt + workspace clippy (`-D warnings`) clean; `cargo test --workspace` exit 0.

**WP-AD.2 Stage B — audit settlement (2026-07-11, gate-green).** Two auditors
(code + tests) filed 11 findings; each settled empirically against the r3723
Delphi source (D10) and probed on the r3723 binary via the Oddie bridge (D9a).
- **`set LinkBranches` off-by-one (Major/Critical, real bug — fixed).** The
  official setter reserves an empty index-0 reference placeholder
  (`ExecOptions.pas:842–844`: `setlength(Link_Branches, Count+1); for i:=1 to
  Count do Link_Branches[i]:=myList[i-1]`); both `Tear_Circuit` branches skip
  index 0 and the sub-circuit count is `length(Link_Branches)`. The Rust setter
  stored a 0-based list with no placeholder, so a single user link tore to **1**
  sub-circuit, not 2. Oddie-probed r3723: `[line.main10]` → "Sub-Circuits
  Created: 2", `[line.main5, line.main10]` → 3. Fixed by prepending the empty
  placeholder in the `linkbranches` setter. `get LinkBranches` also corrected to
  the official per-element `AppendGlobalResult` form (placeholder vanishes, no
  brackets — `line.main10`), matching the probe.
- **Vacuous manual-links test (Critical — fixed).** The old test only asserted
  `get LinkBranches` echoed the set value. Rewritten to pin the empirically
  confirmed cut counts (1 link → 2, 2 links → 3) and the exact `get` echo — now
  a real regression guard for the placeholder + manual-cut path.
- **`Create_MeTIS_graph` weight/dedup unexercised (Major — fixed).** Added a
  transformer + parallel-line feeder test that reads the emitted `.graph` and
  pins the Transformer-weight-1 rule, the 3-phase Line weight, and the
  parallel-branch dedup (5 branches → 4 distinct edges in the header).
- **Zone balance not asserted (Major — fixed).** The 3-zone and macro tests now
  assert per-zone balance (catches a 1-vs-N partition that a bare non-empty
  check missed). Full zone-*connectivity* recompute rides on the deferred
  torn-file round-trip (below), where each zone is compiled and solved.
- **`Num_SubCkts` default `.max(1)` clamp (Minor — fixed).** Removed; now
  `CPU_Cores-1` verbatim (Circuit.pas:606; D6 → never gated).
- **`nphases_bus2` misnomer + `unwrap_or(0)` (Minor — fixed/recorded).** Renamed
  to `pde_bus2_name`; the `unwrap_or(0)` weight-on-unresolved-row divergence from
  Pascal's stale-`ActiveCktElement` read is documented as unreachable
  (`NOTE(upstream)`), a defined 0 preferred over a stale-state read.
- **D5 first-line swap on the un-dropped canonical partition (Minor —
  recorded, no fix).** Plan-sanctioned (D5 reproduce the swap 1:1; D2 partition
  the full canonical graph in-process; D2 accepts auto-tear zone shapes differ
  from upstream). The swap is fixture-pinned deliberately; the future
  `Torn_Circuit` self-golden will pin it by intent, not accident.
- **Torn-file emission ~40% of Stage B was deferred (Major — now CLOSED).**
  Deliverables 5–6 (zone `EnergyMeter` placement, `PConn_Voltages` capture,
  `Save_SubCircuits`/`Format_SubCircuits`/`AppendIsources`/`Disable_All_DER`, the
  committed `Torn_Circuit/` fixture golden + round-trip compile/solve tests) were
  the outstanding Stage-B work at settlement time. They are now landed — see the
  "Stage-B completion" record above (`exec/tearing_save.rs`, the meter/PConn loop
  in `place_zone_meters`, and the fixture golden + round-trip/connectivity gates).
  The terminal-orientation |V| difference is documented `NOTE(upstream-quirk)` as
  vestigial dead code in r3723 (not reproduced, D5).

**WP-AD.2 Stage A — audit settlement (2026-07-11, gate-green).** Two auditors
(code + tests) filed 6 Minor findings; each settled empirically against the C spec
(`.inputs/METIS`,`.inputs/GKlib`) via the offline gcc-13.2.0 reference build. Real
fidelity/coverage gaps fixed 1:1, non-issues recorded:
- **`graph.rs` reader dropped `io.c::ReadGraph` validation (fixed).** The cited
  canonical reader errexits on non-positive edge weights (`io.c:135`), negative
  `vsize`/`vwgt` (`io.c:102/115`), missing size/weight fields, and `ncon>0` without
  a vwgt fmt digit (`io.c:67`); the port silently accepted them. Restored all
  checks (new `GraphError` variants + tests). Also switched the header parse from
  `filter_map` (skips non-numeric tokens) to `sscanf` field-counting (stops at the
  first non-integer). No gate impact — all fixtures are well-formed; the throwaway
  golden `driver.c` uses a permissive replica reader, but graph.rs cites and now
  faithfully reproduces the *canonical* `io.c::ReadGraph`.
- **RM coarsening path had no committed gate (fixed).** Added `mesh120u`, the
  fmt=0 (unweighted) variant of `mesh120`: all-equal weights ⇒ `eqewgts` true ⇒
  **Match_RM** at level 0 (the fmt=1 fixtures are SHEM-only). Its C `.part.{2,3,4,8}`
  goldens replay bit-exact — the port's RM branch now matches the C original in the
  committed suite (6 fixtures × 4 k = 24/24).
- **`nparts==1` early return untested (fixed).** Added a direct unit test pinning
  the all-zeros / zero-cut result (`kmetis.c:70,74`); the gpmetis driver rejects
  `nparts<2`, so it cannot be golden-gated.
- **`pqueue.rs` / `sort.rs` had no isolating unit tests (fixed).** Added
  oracle-grade tests harvested from the real `GK_MKPQUEUE(rpq,…)` and
  `GK_MKQSORT(ikv_t,…)` macros (standalone probes, gcc 13.2.0) pinning the exact
  extraction / unstable-tie-break order — localizing a regression the end-to-end
  golden replay would only surface as a label mismatch.
- **Only 1/5 fixtures from a real feeder (no change — accepted).** Adequate for a
  Stage-A METIS-in-isolation C-vs-Rust gate: the C reference is the oracle
  regardless of graph origin, the port is bit-exact on 24 combos + one real graph
  (`ckt24norm`) + 190 off-corpus combos, and Stage B owns the real DSS round-trip.
  Recorded so breadth is tracked.
- Invariant unit tests (matching maximality, coarsening weight conservation) from
  the original brief remain covered transitively by the bit-exact golden replay
  (any violation perturbs labels); left to the test-audit's discretion, not added
  redundantly.

