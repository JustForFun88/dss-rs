# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

### BUG WP regcontrol_idle — idle no-load zone: adopt r4133 bounded-AND (2026-07-19)

Root-caused and resolved the `controls/regcontrol/regcontrol_idle.dss`
`defer_ledger` occurrence (ORPHANED_GAPS §1.9). The 8.8 % MV-bus divergence
(port MV.1 = 7030.24 V / tapnum 0 vs r4133 7650.08 V / tapnum 15) was a **port
bug relative to r4133**, not a ledgerable divergence.

- **Cause.** The port faithfully reproduced dss_capi 0.15.x's idle no-load-zone
  test written as an **OR** — `(FwdPower ≥ RevThr) or (FwdPower ≤ FwdThr)` — a
  **tautology** under the default symmetric ±100 kW band (every value is
  ≥ −100 kW OR ≤ +100 kW), so an idling reg NEVER taps for any load. EPRI
  r4088/r4133 use the correct **bounded AND** (`RegControl.pas:1218`):
  `(FwdPower ≤ FwdThr) and (FwdPower ≥ RevThr)` — idle only when the
  through-power is *inside* [RevThr, FwdThr].
- **Dated** (git `.inputs/dss_capi_with_git`): the OR is `8a898cba` "port SVN
  r4086" and is STILL OR at the 0.15.x branch tip (`e936d210`); r4088 & r4133
  have AND; r3723 & 0.14.5 have no idle feature.
- **Live r4133 probes** (`epri-worker`): with the default band idle=yes taps
  identically to idle=no (tapnum 15, MV.1 7650.08 V — FwdPower ≈ 7438 kW, far
  outside the band). Widening `fwdThreshold` to 1e6 kW so the bounded zone
  brackets the ~7.03e6 W throughput makes r4133 *idle* to exactly the port's old
  MV.1 7030.24 V (probe D) — a direct proof of the bounded-AND mechanism.
- **Resolution (verdict b — adopt r4133).** `idle` is 0.15-only (0.14.5 rejects
  #110; the one pinned idle golden `props/regcontrol.json::regcontrol_idlezones`
  pins property readback only, no no-load-zone solve). The port adopts r4133's
  AND (`reg_control/control_loop.rs`) + a regression unit test pinning the
  high-power (out-of-band) tap. Flipped the case from `defer_ledger` to live
  `engines:"r4133"` gating — tap/voltage/full-model now match r4133 exactly.
- **Residual ledgered.** The revThreshold/fwdThreshold **getter convention**
  differs: r4133 returns the `InitPropertyValues` display strings
  (revThreshold '100', fwdThreshold '') decoupled from the signed internal,
  while the port reports the signed effective thresholds in kW (−100 / +100 —
  which matches dss_capi 0.15.x, pinned by `regcontrol_idlezones`). Definitional,
  not physics → exact-pair `probe` divergence
  `r4133-regcontrol-idle-threshold-display` (cause `regcontrol-idle` rewritten).
  Population lock regenerated (one-digest diff: defer 1→0 + ledger digest).
- **Gate green** (`cargo fmt`/`clippy`/`test --workspace`, wall ≈ 340 s;
  dss-core lib 1247 + corpus_gate 25 incl. the full unified gate). Corpus
  pristine.
- **Audit settle (2 opus xhigh audits, both PASS — no weakening).** Three low
  findings settled:
  - *(F1, both audits — fixed)* The `regcontrol_idle.dss` header comment still
    described the removed OR behavior ("tap stays at neutral, |V|≈0.864 pu").
    Rewritten to the adopted r4133 bounded-AND semantics (deep under-voltage →
    large through-power → outside the ±100 kW no-load band → taps to tapnum 15 /
    MV.1 ≈ 7650.08 V, identical to idle=no). Comment-only; solver ignores it.
  - *(F2, audit-tests — fixed)* `TESTING.md` ledger count was stale (25→26
    entries; the r4133 divergence sub-count 20→21). Updated; the "20 documented
    causes" line is unchanged (the new entry reuses the pre-existing
    `regcontrol-idle` cause_ref, present in the causes dict at both base and head).
  - *(F2, audit-code — deliberate NON-fix, rationale recorded)* `end_edit`
    (`accessors.rs:347-348`) mirrors the RevThreshold-only legacy band with
    `Fwd := abs(Rev); Rev := -Fwd`, faithfully porting **dss_capi 0.15.x**
    (`8a898cba` — whose own comment states *"'abs' added to ensure correct
    behavior (RevPowerThreshold < FwdPowerThreshold)"*, an intentional fix of
    EPRI's inverted-band quirk). r4133 `RegControl.pas:503-506` instead does
    plain `kWFwd := kWRev; kWRev := -kWRev`. **Kept the abs**, NOT changed to
    r4133's negate: (1) pre-existing, not touched by this WP (out of scope — the
    WP adopted r4133 only for the no-load *zone* AND); (2) the two conventions
    are behaviorally **identical** on every existing test — all pinned goldens
    (`props/regcontrol.json`) use positive-revThreshold-only or both-set inputs
    where abs≡negate; they diverge only on an untested NEGATIVE-revThreshold-only
    input, which no deck or golden exercises; (3) the port's abs matches the
    capi015-probed `regcontrol_idlezones` props golden convention (RevThreshold
    signed display), so adopting r4133's negate would REINTRODUCE the inverted/
    empty band that dss_capi deliberately fixed and create an unpinned, ungated
    behavior. Verified empirically: filtered gate (`DSS_GATE_ONLY=
    regcontrol/regcontrol_idle`) green, port matches r4133 live, ledger entry
    hit 2×.

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

### DE_PASCALIZE P1b — control-trio integer families → enums (wave 2, branch `wt-p1b-v2`)

Stratum **[A]** bit-neutral. Closes the P1 wave-1 control-trio deferral
(`docs/phase-records/depascalize-p1.md` #Deferred item 7). Salvaged the
interrupted WIP `c842af0` (origin/wt-p1b) by clean cherry-pick onto the
post-classwalk `update` tree; it compiled as-is (only two `cargo fmt` line-wraps
needed), and every discriminant was re-proven before finalizing:
- **Relay** `control_type` → `RelayControlType` (`#[repr(i32)]`, `0,1,3,4,5,6,7,8,9`
  — the `2` ordinal stays unused, `from_ordinal(2)=None`).
- **CapControl** `control_type` → `CapControlType` (`0..5`; USERCONTROL=6 is not
  registered upstream and never set by the port, so the `Sample` match drops its
  `_ => {}` and is exhaustive).
- **RegControl** queue action codes → `RegControlAction` (`TapChange=0`/`Reverse=1`;
  `i32` survives only at the `ControlQueue` push / `DoPendingAction` boundary).

Each ordinal proven against Pascal (`Relay.pas:323-331`, `CapControl.pas:92-100`,
`RegControl.pas:246-247`) **and** the DssEnum registry (`registry/control.rs`
`relay_type`=[0,1,3,4,5,6,7,8,9], `cap_control_type`=[0,1,2,3,4,5]). `i32` remains
at the property parse/report + CIM-export accessors only. Proof (all unchanged):
3 new ordinal-round-trip pin tests + the controls-corpus manifests (105 cases) +
eventlog gate. MonPhase sentinels + the shared `CTRL_*` state channel were not
required and stay deferred (item 7 residue / R0 `control_elem.rs`).

Audit settlement (two independent auditors, no regression): 2 `low` notes.
(1) setter keep-old fallback was not directly exercised → closed with two additive
`set_i32_type_keeps_value_on_unregistered_ordinal` pin tests (relay + cap_control),
no golden/tolerance touched. (2) `relay_type` DssEnum omitting Pascal's
`DefaultValue := 0` → confirmed pre-existing (registry file untouched in-range) and
a string-parse-fallback matter orthogonal to this `[A]` storage-type conversion;
deferred to a registry-fidelity pass (rationale in `depascalize-p1.md`).

### DE_PASCALIZE P13 — VCCS delay line → `RingBuf` (wave 2, branch `wt-p1213-v2`)

Stratum **[A]** bit-neutral. The VCCS z-domain filter's two wrap-around
histories (`z`/`whist`, tapped via the 1-based circular `MapIdx(iu-k+1, fl)` in
`vccs/dynamics.rs`) become a `RingBuf` type whose `tap()` accessor encapsulates
the wraparound (calling the unchanged `map_idx`) and whose `Index`/`IndexMut`
serve the direct head/snapshot access. `y2`/`zlast`/`wlast` stay `Vec` (never
`MapIdx`-tapped). Same slots, same statement order. Proof: new
`ringbuf_tap_reproduces_pascal_map_idx_order` unit test (hardcoded `[1,5,4,3,2]`
tap-order pin + an independent wraparound-range check that every raw index folds
into a live slot `1..=len`) + the 3 oracle-gated Monitor-mode-3
dynamics-trajectory tests (`exec::tests::vccs`, waveform + RMS 1φ/3φ) all
UNCHANGED. Full record: `docs/phase-records/depascalize-p13.md`.

### DE_PASCALIZE P12 — `line_constants` `Vec<Conductor>` (wave 2, branch `wt-p1213-v2`)

Stratum **[A]** bit-neutral. The ~20 parallel per-conductor arrays on
`LineConstants` collapse into one `cond: Vec<Conductor>`; the 11 cable-only
arrays become each conductor's `cable: Option<CableData>` (the typed form of the
Pascal "subclass arrays empty on overhead" trick). Carson/DERI/coaxial kernels
in `mod.rs`/`cable.rs`/`cn.rs`/`ts.rs` read `cond[i].field` / `cable(i).field` —
same arithmetic, same order. FPC-compat helpers untouched. Salvaged the
`origin/wt-p1213` WIP `70cefbb` (mod.rs only, interrupted) by clean cherry-pick,
completed the remaining mod.rs + all cable/cn/ts sites, folded to one commit.
Proof (all unchanged): 20 line-constants unit tests, `golden_line_constants`,
`corpus_gate` checkpoint YPrims. Full record: `docs/phase-records/depascalize-p12.md`.

**Settle (P12+P13 audit dispositions).** Two independent audits (code + tests)
found the pair faithful and bit-neutral; three low-severity notes settled
empirically: (1) a corpus-gate `iteration count differs` on `Test/YgD-Test.dss`
seen once under parallel load, green on an identical re-run — that deck is a bare
`New Line.Line1` (no geometry/linecode, no VCCS), so it touches neither the P12
line_constants geometry kernel nor the P13 RingBuf; pre-existing harness/oracle
parallel-load nondeterminism, NOT a P12/P13 regression, left as-is. (2) The new
ringbuf unit test's second assertion loop restated `tap`'s own body
(`tap(idx) == self[map_idx(idx,len)]`) and could never fail — replaced with an
independent wraparound-range check (`tap` always folds into a live slot
`1..=len`); the `[1,5,4,3,2]` order pin was and is the real behavioral baseline.
(3) A "corpus_gate 11 passed" count in a transient audit-evidence message was a
miscount (the target runs 25 tests) — it never appeared in any committed
artifact (STATUS §gate already states 25), nothing to fix.

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

### BUG WP DynExp — reverted the D14 `SolveEq` no-op; port swings, matches both oracles (branch `bug-dynexp`, 2026-07-19)

The DynExp decks' `defer_ledger` said the port matched **neither** surviving
oracle (0.14.5 and r4133 agree; port off both). Root cause: the earlier **D14**
work adopted upstream `2a8bdb78`'s no-op `SolveEq` (an `Exit` before the RHS is
evaluated) to match the retired **non-gating** capi015 (dss_capi 0.15.x), which
**froze** the DynExp state at its `InitStateVars` seed. Both *gating* oracles run
the full evaluator: vendored **0.14.5** `SolveEq` (`DynamicExp.pas:377`) and EPRI
**r4133** `SolveEq` (`:497`) are byte-identical in structure and integrate — the
rotor swings.

- **First divergence** (Kundur DynExp, 1 dynamics substep): the DynExp derivative
  slot `dspeed` — both oracles compute **-1.6169543e-6**; the D14 no-op port left
  it at **0**. It compounds via the trapezoidal integrator; by the deck's 5 s
  endpoint the port's node V diverged catastrophically at the deep nodes (HT.1:
  oracle 122713 V @ 69.1° vs D14-frozen 193725 V @ 24.8°), while the quasi-ideal
  source bus barely moved (~2.6 V, 1.5e-5 — the misleading "entry 0").
- **Fix**: reverted `dynamic_exp.rs::solve_eq` to the full 0.14.5/r4133 evaluator
  (full `0..cmds.len()` loop, safe `cmds.get(idx+1)` for the benign OOB read, no
  early return, final upload after the loop); restored `get_out_idx`; fixed the
  `dyneq_pce.rs` doc. Cited to `DynamicExp.pas:377`/`:497`.
- **After** (measured live): the reverted port reproduces the oracle's step-1
  `dspeed` -1.6169543e-6 to the f32 monitor floor, and the 5 s endpoint matches
  (rotor `theta` 2.036 rad / `speed` 0.626; node V matches 0.14.5).
- **Gating**: both `Dynamic_KundurDynExp.dss` and `GFLDaily_DynExp` had their
  `defer_ledger` removed. Kundur gates on **both** channels at the feeder floor
  (no ledger entry — the two oracles agree ~4.8e-10, the port matches both). GFL
  gates on **r4133** (off `capi_v0145` for the separate D7 PVSystem-dynamics
  reason, like its non-DynExp sibling). The pre-staged `dynexp-d14` ledger cause
  is removed (no envelope needed).
- **Tests**: the 7 `exec/tests/dynamics.rs` DynExp gates + the `dynamic_exp` unit
  tests restored to their pre-D14 swinging-oracle pins (now guard against
  re-introducing the no-op). All 34 dynexp/dynamics unit tests green.
- **Settled** (two independent opus xhigh audits, `19e0890..5a6c6d4`): audit-code
  returned zero findings (faithful, Pascal-cited semantics correction). audit-tests
  raised two **low/INFO** notes, both settled empirically as *strengthenings, not
  weaknesses* — neither warrants a code change:
  - *GFL DynExp deck is r4133-only, not both.* Verified: its non-DynExp sibling
    `Run_IEEE123Bus_GFLDaily.DSS` is likewise `engines:r4133` for the pre-existing
    WP-U1.2 D7 reason (daily-shape `PanelkW` < `FkVArating` shifts the dynamics
    current limit off 0.14.5). The 0.14.5-side DynExp evaluator proof therefore
    lives in the **Kundur** deck, which gates on **both** channels — the filtered
    corpus gate (`DSS_GATE_ONLY=DynExp`, 3/3 pass) engages `capi_v0145`+`r4133`
    there. So the r4133-only GFL leaves no DynExp channel unverified.
  - *`dynamic_exp` interpreter unit values are hand-derived, not oracle-captured.*
    Matches the pre-D14 state and is the correct comparator for a
    compile/`SolveEq` path the oracle does not expose outside a dynamics solve.
    `kundur_expression_evaluates` computes `d(speed) = -1/mass·(pterm+damp·speed
    −pshaft)` to `1e-15` — which the D14 no-op leaves at 0, so the test is a real
    anti-no-op guard. It is backstopped end-to-end by the oracle-anchored exec
    swing gate (θ 0.42211992/1.7127246 rad = 24.18569/98.131889 deg) and the live
    corpus gate on both channels. No tolerance touched; goldens/harness untouched.

### WASM-UM WP-WM.0 — ABI freeze + probes (branch `wasm-um`, 2026-07-18)

`WASM_USERMODELS_PLAN.md` execution started (WM.0→WM.2 authorized for this
round; WM.3+ deferred — parallel workflows own the element files). WP-WM.0 is
**docs + probe scripts only** (zero engine/product code changes):

- **`docs/wasm/USERMODEL_ABI.md` FROZEN** — export lists (15/13/7 + `memory` +
  `dss_alloc`), packed record offset tables **transcribed from FPC probe
  output** (never from reading), the 32-slot callback import table (module
  `dss_env`) with tiers + census, activation rule, failure/trap/sandbox
  policy, probe-evidence ledger. Probe kit: `tools/fpc/usermodel_abi/`
  (verbatim-extraction script + 2 offset probes + 15-export stub DLL + oracle
  driver + build script); evidence: `docs/wasm/probes/p1..p5*.txt`.
- **P1 (oracle loads native DLL): PASS, all 5 asserts** — pinned dss-python
  0.15.7/0.14.5 loads the FPC-built stub via `Generator.UserModel=`, stub
  state vars appear on the element variable surface, `UserData=` reaches
  `Edit` (len verified), Model=6 snapshot converges with terminal currents ==
  stub `Calc` output **bit-exact**, marshalled V == terminal-1 node voltage at
  4.8e-7 rel (one fixed-point iterate stale by construction — documented in
  the probe). **Plan §2.5 channel 1 CONFIRMED; r3723 fallback not engaged; the
  WM.3 audit-tier escalation clause is moot.**
- **P2 (layouts): packed confirmed** (release cfgs never set
  `DSS_CAPI_NO_PACKED_RECORDS`; probed with `-Mdelphi` + release defines,
  x86_64-win64): `TDynamicsRec` 52 B, `TGeneratorVars` 244 B (unaligned tail
  after the 3 i32s — `#[repr(C)]` would mis-pad, noted in the ABI doc),
  `TDSSCallBacks` 256 B = 32×8. dss_capi 0.14.5 vs r3723 header sets:
  **byte-identical** (twin probe over the real vendored r3723 units).
- **P2 twin decision: PLAN A** — FPC 3.2.2 `ppcrossx64 -Mdelphi` builds the
  **vendored `IndMach012a.dpr` as-is** (search paths only, zero source edits;
  `.res` links); the resulting DLL loads under the pinned oracle, all 14
  machine vars live, Model=6 solve converges with physically-sensible slip
  (−0.0064). Plan B (Rust native-shim twin) not needed.
- **P3 (callback census): `MsgCallBack` only** (`IndMach012Model.pas:474`,
  help text); parser bundled (`ModelParser`), `DoDSSCommand` **unused** ⇒ the
  WM.6 deferral stands. Full 32-slot table in the ABI doc.
- **P4 (toolchain pins):** stable rustc 1.96.0; `wasm32-unknown-unknown`
  target added (machine-global, additive); `wasmi ==1.0.9`
  (`default-features=false`, `features=["simd"]`, the typst-verified pin)
  compiles on stable with a pure-Rust closure (wasmi_core/ir/collections
  1.1.0, wasmparser 0.228.0, bitflags, libm, spin — zero C/FFI) and exposes
  fuel + store-limiter APIs (`instantiate_and_start` is the 1.x spelling).
  Initial `tools/wasm_usermodel/PIN.txt` written. No wasmi blocker ⇒ the pin
  stands.
- **§2.7 upgrade one-line diff check — one real finding:** the four loader
  units + callback vtable + `TDynamicsRec` are contract-identical across
  0.14.5→0.15.x and r3723→r4133 (host-side property→method refactors only),
  **but** 0.15.x and r4088+ insert `deltaQNom: array of Double` into
  `TGeneratorVars` between `Qnominalperphase` and `NumPhases` (+8 tail shift,
  managed reference). Frozen ABI = pinned 0.14.5/r3723 layout; caution
  recorded in the ABI doc §2.2 (never mix ≤r3723-header DLLs with
  r4088/r4133 binaries; an engine upgrade to 0.15.x semantics must revisit by
  recorded decision). Evidence `docs/wasm/probes/p5_upgrade_diff.txt`.

Follow-ups: none blocking WM.1. `TStorageVars`/`TPVSystemVars`/
`TCapControlVars` offset tables are frozen at their owning WPs (WM.4/WM.5) via
the same probe kit (ABI doc §2.4 records this explicitly).

### WASM-UM WP-WM.1 — the `dss-usermodel` crate (branch `wasm-wm1`, 2026-07-18)

The wasmi host per plan §2.1–§2.3; template = the vendored typst plugin host
(`.inputs/typst/.../plugin.rs`, cited in doc comments throughout). New leaf
workspace crate `crates/dss-usermodel/` (`#![forbid(unsafe_code)]`; deps:
`wasmi =1.0.9` pinned in `[workspace.dependencies]` per PIN.txt
(`default-features=false`, `["simd"]`), `num-complex`, `dss-parser` (the owned
AuxParser); dev-deps `wat` + `sha2`). No dss-core changes — the crate is not
consumed yet (WM.3 wires it).

- **`UserModelHost`** (`src/host.rs`): deterministic engine config (relaxed
  SIMD off = typst `:271-272`; `consume_fuel(true)`), module compile, and
  **load-time export-set validation** — `memory`, `dss_alloc`, then the
  interface functions in the **Pascal binding order**
  (`GenUserModel.pas:173-187` / `StoreUserModel.pas:336-348` /
  `CapUserControl.pas:176-182`), so the first missing name is exactly what the
  engine's 569/1569 path reports; present-but-wrong-signature = typed
  `SignatureMismatch` (ABI §6 protocol violation). `InterfaceKind` carries the
  five Pascal loader shapes (Gen15 `new(genvars,dynarec)`, Store15/PV15/Dyna13
  `new(dynarec)`, Cap7 `new()`). `HostConfig`: per-call fuel budget (default
  1e8 — the plan's <1%-of-budget calibration bar is verified against the real
  fixture at WM.2) + 64 MiB memory cap.
- **`UserModelInstance`/`CapControlInstance`** (`src/instance.rs`): one
  `Store` per element binding (plan §2.7 — `Store` is `Send`, M3-safe); guest
  buffers allocated once via `dss_alloc` (genvars 244 B / dynarec 52 B / V+I
  `yorder`×16 / name scratch; grow-only edit + vars buffers) and range-checked
  (`dss_alloc` returning 0 / out-of-range = typed error). Record shuttle =
  `GeneratorVars`/`DynamicsRec` mirrors (`src/records.rs`) serialized
  **field-by-field at the frozen ABI offsets** (never `#[repr(C)]` — the
  packed unaligned tail; unit tests pin every offset against the ABI-doc
  tables). Records written before and read back after **every** call (ABI
  §2); Pascal wrapper quirks reproduced: `Edit` ignored while `FID=0`,
  `Integrate` = `select(id)`+`integrate` (`GenUserModel.pas:123-138`),
  `new`→0 = model-absent (`Get_Exists`), delete-guard on nonzero id.
- **Callbacks** (`src/callbacks.rs` + `src/imports.rs`): the 32-slot
  `TDSSCallBacks` vtable as module `dss_env`, tiered per ABI §4 — tier A
  served from the per-call `Box<dyn Callbacks>` snapshot (default method
  bodies mirror each Pascal nil path, incl. `Exit`-without-touching
  semantics via `Option`), tier B = `Effect` queue (`Msg`,
  `ControlQueuePush` with provisional handles `seed, seed+1, …` from
  `control_queue_next_handle()` — exact Pascal handle sequence under
  drain-in-order), tier C = owned `dss_parser::Parser` in `CallData`
  reproducing `CallBackParser` semantics (`NextParam` returns the **value**
  length and copies the **name**; `GetStrValue` serves `CB_Param` from the
  last `NextParam` incl. the deterministic truncate-on-short-maxlen;
  FPC-exception-on-conversion-error = UB upstream, defined port writes 0).
  All 32 imports exist at link time; `do_dss_command`/`get_result_str`
  (WM.6) and `get_active_element_ptr` (permanent) raise the loud attributed
  `Unsupported` error when called — no silent no-ops (plan §2.9-5).
- **Typed failure contract** (`src/error.rs`, ABI §6): faults recorded by
  imports win (the typst `memory_error` take-pattern), then
  `TrapCode::OutOfFuel` → `FuelExhausted`, `GrowthOperationLimited` (store
  limiter with `trap_on_grow_failure`) → `MemoryCapExceeded`, else `Trap` —
  every variant naming the model and function.
- **Channel-2 protocol tests** (`tests/protocol.rs`, 23 tests, inline-WAT
  guests via dev-dep `wat`): happy-path 15-function round trip (call
  counters + V/I marshalling + records); record byte-exact round trip
  (distinct bit patterns in every field → guest increments `Pshaft`/`t` →
  only those change); tier A full-surface exerciser (all 22 snapshot reads
  incl. copy-semantics counts) + the Pascal nil-path defaults; tier B order +
  handle sequence; tier C parser round trip over the owned AuxParser;
  missing export → exact name + binding-order-first + 13-vs-15-fn sets +
  missing `memory`/`dss_alloc`; wrong signature; trap / fuel / import-OOB /
  host-OOB / alloc-0 / memory-cap → the typed errors; unsupported-import
  trio; CapControl 7-fn round trip; host-API misuse (`Usage`).
  `tests/fixture_pin.rs` = the **hash-vs-PIN scaffold**, self-activating
  (dormant pre-WM.2 state = no fixture + no `sha256(...)` line in PIN.txt;
  any half-state is red; format documented for WM.2) — no `#[ignore]`.

Workspace edits: members + `[workspace.dependencies].wasmi` +
`[profile.dev.package.dss-usermodel]` opt-3 (safety knobs pinned) in the root
`Cargo.toml`. Follow-ups for WM.2: commit the fixture + PIN hash line
(activates the scaffold), verify the fuel-calibration bar (<1% of 1e8 per
reference-model call).

### WASM-UM WP-WM.2 items 1–3 — IndMach012a fixture port + pinned `.wasm` + native twin (branch `wasm-wm2`, 2026-07-18)

Items 1–3 of plan §WP-WM.2 (+ the item-4 data pre-stage); item 4 (fixture
self-gate through the `dss-usermodel` API) and item 5 (audits) belong to the
integrator/workflow. Zero product-code changes — everything lives under
`tools/wasm_usermodel/`, `tests/fixtures/wasm/`, `docs/wasm/probes/`.

- **Item 1 — the port.** `tools/wasm_usermodel/models/indmach012a/`
  (workspace-excluded crate, `[workspace]` opt-out, zero deps, cdylib+rlib):
  loop-for-loop port of r3723 `IndMach012Model.pas` (machine math, slip
  clamp, dSdP, dynamic/pflow currents, trapezoidal `Integrate`, 14-variable
  surface — `model.rs`), `MainUnit.pas` (ModelList/ActiveModel semantics incl.
  the delete-clears-active quirk and the guarded/unguarded nil-ActiveModel
  split — `mainunit.rs`), the units the DLL links: `Ucomplex.pas` **as this
  source defines it** (naive `CDIV`/`Cabs`, NOT dss-core's `cdiv_fpc` Smith
  helpers — `cmath.rs`), `mathutil`/`Ucmatrix` sym-comp path with `Ap2s`
  produced by a verbatim `TcMatrix.Invert` port at init (`symcomp.rs`), and a
  minimal `ParserDel`/`Command`/`HashList` scanner (`parser.rs`; RPN-in-quotes
  reduced to a loud trap — plan-§2.6-sanctioned, decks never use it). Boundary:
  `records.rs` codecs at the frozen ABI offsets (never `#[repr(C)]`),
  `wasm_exports.rs` = the 15 exports + `dss_alloc` over a safe allocation
  registry; exactly ONE `unsafe` expression in the crate (the `dss_env.
  msg_callback` import call). Pascal citations throughout.
  `TODO(compat)`×3: truncated `0.866025403` (SetAMatrix), truncated `1.732`
  (Compute_dSdP), and **a new find** — FPC folds the all-constant `3.0/746.0`
  (HPshaft var 14) at *single* precision (both operands single-exact ⇒ FPC
  lowest-common-precision constant folding), reproduced as
  `(3.0f32/746.0f32) as f64` and proven by decomposition (plain f64 quotient
  misses the twin by 2.6e-8 rel; every other value bit-exact).
- **Item 2 — pinned artifact.** `tests/fixtures/wasm/indmach012a.wasm`
  committed (79045 B; exports = the 15 + `memory` + `dss_alloc`, sole import
  `dss_env.msg_callback` — verified by wasm section parse). Reproducible
  build proven (clean rebuild ⇒ identical SHA-256):
  `pwsh tools/wasm_usermodel/build_wasm.ps1` (stable rustc 1.96.0,
  `--remap-path-prefix`, locked release profile). PIN.txt updated with the
  exact command + `sha256=1849db0c…9b0ebd` (the WM.1 hash-vs-PIN test binds
  to it at integration).
- **Item 3 — native twin (plan A) + item-4 pre-stage.**
  `build_native.ps1` builds the vendored `IndMach012a.dpr` on demand (FPC
  3.2.2 ppcrossx64, exact P2 flags; `%TEMP%` output, never committed).
  `twin_probe.py` (ctypes over the frozen packed layouts, struct-offset
  asserts) drives the twin through a deterministic lifecycle — New →
  var-name surface (incl. StrLCopy truncation + out-of-range no-write) →
  initial vars → Edit (abbrev `maxs`, case `Xm`, `option=variableslip`,
  slip→Speed write) → 5 pflow `Calc` iterations (slip fixed-point) → `Init`
  → 3 dynamics steps × predictor/corrector `Calc`+`Integrate` → SetVariable
  → GetAllVars → Select edges → `help` (MsgCallBack text) → second instance
  + Delete — and records ~200 values bit-exactly:
  `docs/wasm/probes/p6_twin_expected.txt` (evidence) + generated
  `tests/twin_expected.rs` (`--rust` mode). `tests/twin_parity.rs` replays
  the identical scenario on the Rust port and asserts **f64-bit-exact
  equality on every value** — green (2/2; `cargo +stable test` in the crate).
  The integrator pins the committed `.wasm` against the same values through
  the WM.1 crate API (item 4).
- **Hygiene:** `.gitignore` +`tools/wasm_usermodel/models/*/target/`;
  fixture crate is fmt/clippy-clean on host and wasm targets (not part of
  the repo gate — workspace-excluded by design). Machine-global additions:
  none required beyond WM.0's pins (the stray `rustup target add` on the
  default *nightly* toolchain during this session is additive-only; the
  fixture builds with `+stable` per PIN).
- Deviation note: plan §2.6 sketches the guest as "`#![no_std]`-lean"; the
  crate uses std (wasm32 std = the allocator/panic machinery only — no WASI,
  no imports beyond `dss_env`, verified in the artifact's import section).
  Chosen to keep the boundary in safe Rust (registry over `Box<[u8]>`); the
  sandbox/determinism contract is unaffected.

### WASM-UM WM.1+WM.2 integration — merge + WM.2 item 4 fixture self-gate (branch `wasm-um`, 2026-07-19)

Merged `wasm-wm1` (WM.1, fast-forward) then `wasm-wm2` (WM.2 items 1–3; sole
conflict = STATUS.md section placement, resolved by union — both records kept
in WP order). Workspace `Cargo.toml`/`Cargo.lock` merged clean (WM.2's fixture
crate is workspace-excluded by design). Cross-WP items neither side could do
alone:

- **WM.2 item 4 — fixture self-gate:**
  `crates/dss-usermodel/tests/fixture_self_gate.rs` drives the COMMITTED
  `tests/fixtures/wasm/indmach012a.wasm` through the full crate API chain
  (`UserModelHost::load` Gen15 export validation → `UserModelInstance` record
  shuttle → guest math) with hand-fed V/records, replaying the
  `twin_probe.py` scenario S1–S11+S13 and pinning every recorded value
  **f64-bit-exact** against the native-twin constants (`twin_expected.rs`,
  evidence `docs/wasm/probes/p6_twin_expected.txt`): ~200 pins — currents (5
  pflow + 6 dynamics calc), slip fixed-point, all 14 vars at 6 checkpoints,
  GenVars `Speed` write-backs (new/edit/init/setvar), var names, `help`
  MsgCallBack text byte-identical through the `dss_env` effect queue. S12/S14
  (foreign-id select, two models in one guest) are single-shared-DLL
  artifacts the per-element-instance design deliberately does not expose
  (plan §2.7); pinned instead: own-id select round-trip + a fresh-guest
  second instance (id=1 again, post-Create pins verbatim) + delete clears
  `exists`. GREEN — the committed binary and the WM.1 shuttle agree with the
  FPC twin bit-for-bit.
- **Hash-vs-PIN scaffold now ACTIVE:** with the fixture + PIN sha256 line
  committed, `fixture_pin.rs::committed_fixture_hash_matches_pin` takes the
  active branch and verifies `1849db0c…9b0ebd` — green.
- **WM.1 fuel-calibration follow-up settled:** the plan's bar (<1% of the
  1e8 default per reference-model call) is proven by a second self-gate run
  under `fuel_per_call = DEFAULT/100` — the entire scenario (instantiation
  included) completes with no `FuelExhausted`.
- Fixture crate's own `twin_parity` suite re-verified green post-merge;
  full three-command gate green at default settings; `tests/corpus` pristine
  (stray solver outputs from an aborted run removed path-limited). One
  transient on the first full-gate run: `corpus_gate` CapiV0145
  `asymmetric:isource/isource_snap.dss` step-0 voltage off by 3.6e-3 (>floor
  8.2e-6), NOT reproducible — same binary re-run green twice (513→514/514 and
  the full workspace re-run), diff touches no dss-core code. Suspected
  cross-session oracle-server contention (parallel workflows active); watch
  if it recurs — a reproducible hit would need the CLAUDE.md prove-it
  discipline, not a shrug.

### WASM-UM WM.0–WM.2 settle — audit dispositions (branch `wasm-um`, 2026-07-19)

Two independent `opus-xhigh` audits of `ae4b4ef..4051d833` (audit-code +
audit-tests): **verdict faithful, zero Critical/Major**; both independently
re-derived the ABI offsets, rebuilt the native twin AND the wasm fixture
(byte-identical to the PIN), and re-ran the full gate. Findings settled
empirically:

- **WM-AUD-1 (Minor, FIXED — doc):** the frozen ABI doc omitted the
  `get_node_voltages` ground-slot indexing decision (native
  `GetPtrToSystemVarrayCallBack` returns the raw `Solution.NodeV` pointer
  whose offset-0 element IS ground, `Solution.pas:88/:198`; the crate contract
  serves `NodeV[1..NumNodes]` ground-excluded). Recorded via the doc's
  recorded-decision mechanism (header note + §4 row 17 + indexing note with
  the porting consequence) **before WM.3 wires the slot**; contract unchanged.
- **WM-AUD-2 (Minor, premise DISPROVEN by probe; residual recorded):** FPC
  3.2.2 `Val` was probed directly (ppcrossx64 x64 exe —
  `docs/wasm/probes/p7_fpc_val_domain.txt`): it **accepts** `inf`/`nan` (any
  case) and leading spaces, exactly like Rust `parse::<f64>()` — the fixture
  matches the spec there. Residual Rust-wider domain (`infinity`,
  trailing/tab whitespace via `trim`) is unreachable (tokenizer never yields
  whitespace-padded unquoted tokens; quoted branch is RPN upstream, whose own
  tokenizer skips whitespace; no deck feeds `infinity`). Deliberately NOT
  changed. Lesson captured: the fixture **source is hash-frozen with the
  artifact** — a comment-only parser.rs edit shifts panic-`Location` line
  numbers and changes the built wasm hash (verified: pristine rebuild = the
  pinned `1849db0c…`, +10-comment-lines rebuild = `7da8ee46…`), so fixture
  notes live in probes/STATUS, never as source edits without a deliberate
  re-pin.
- **WM-T4 (Minor, FIXED):** `fixture_pin.rs`'s pre-WM.2 dormant arm
  (no fixture + no PIN line = pass) retired — the fixture is permanent as of
  WM.2, so both halves are now required unconditionally (simultaneous
  deletion of fixture + PIN line is red). Strictly strengthens the gate.
- **WM-T1 (Informational, ACCEPTED — plan-sanctioned):** the fixture crate's
  `twin_parity` suite + fmt/clippy are workspace-excluded by design (plan
  §2.6); the committed artifact IS gated every `cargo test` (self-gate ~200
  bit pins + hash-vs-PIN). Follow-through: the plan's WM.7 exit sweep now
  lists an explicit `twin_parity`+fmt/clippy re-run for the fixture crates.
- **WM-T3 (Informational, ACCEPTED as designed):** twin scenarios S12/S14
  (foreign-id select, two models in one guest) are single-shared-DLL
  artifacts the per-element-instance design never exposes (plan §2.7);
  crate-level surrogates pin the equivalent paths — already documented in the
  self-gate header and the integration record.
- **WM-AUD-4 / WM-T2 (watch item, STANDS):** the one non-reproducible
  `corpus_gate` transient (isource_snap step-0, 3.6e-3 vs floor 8.2e-6; green
  on 4 total re-runs across author+auditor) stays a recorded watch — a
  reproducible hit gets the CLAUDE.md prove-it discipline.
- **WM-AUD-3 (environmental):** a parallel session's corpus_gate run wrote
  stray solver outputs into this worktree during the audit; the committed
  range was verified clean at audit start. `tests/corpus` re-verified
  pristine at settle.

### WASM-UM ABI re-freeze to r4133 (pre-WM.3, branch `wasm-r4133`, 2026-07-19)

User decision 2026-07-19: the project gates on the in-house **r4133** bridge
(`crates/dss-epri`), so the frozen user-model ABI + native twin move from the
0.14.5/r3723 layout to r4133. Executed as the ABI doc's own recorded-decision
procedure (header entry (b) + updated §2.2 + Appendix A). **Probe, not assume:**

- **P8 r4133 offsets** (`abi_probe_r4133.pas` → `docs/wasm/probes/p8_offsets_r4133.txt`):
  FPC `-Mdelphi` probe of the r4133 headers. `TDynamicsRec` 52 B and
  `TDSSCallBacks` 256 B **byte-identical** r3723→r4133; `TGeneratorVars`
  **244→252 B** — `deltaQNom` (`array of Double`, 8-B managed ref, NCIM-only)
  at offset 176, tail shifted +8 (`NumPhases` 176→184, `VthevMag` 188→196,
  `XRdp` 236→244).
- **P8 model-math diff** (`p8_indmach012a_math_diff.txt`): the IndMach012a
  example dir (`IndMach012Model.pas`/`MainUnit.pas`/`ParserDel.pas`/`.dpr`) is
  **byte-identical** r3723→r4133 (sha256s); the sole delta is
  `GeneratorVars.pas`. **Model math UNCHANGED.**
- **P8 twin-in-r4133-engine** (`p8_twin_r4133_bridge.txt`): the twin rebuilt
  from r4133 (252-B layout) **loads + runs in the r4133 engine via the
  `epri-worker` bridge** — model=6 power-flow converged with the 14 IndMach012a
  machine vars live (Slip=−0.006407, puRs/puXm/MaxSlip echoing UserData,
  Is1/Ir1/StatorLoss/HPshaft computed) + 10 dynamics steps (Monitor mode=3
  Slip/Freq series). This is the authoritative re-derivation through the r4133
  channel (never Rust-vs-Rust).

**Decision:** the frozen **native** `TGeneratorVars` is now r4133 (252 B,
§2.2a). The **wasm marshaled image is UNCHANGED at 244 B** (§2.2b): `deltaQNom`
is engine-only and a managed reference has no wasm-linear-memory meaning, so it
**never crosses** the boundary; the +8 shift is native-side only. Consequently
**zero wasm-side bytes changed** — proven empirically:

- `twin_probe.py` ctypes image updated to the r4133 252-B layout (`deltaQNom`
  slot, nil); driving the r4133 twin re-derives `twin_expected.rs` +
  `p6_twin_expected.txt` **bit-identical** to the r3723 pins (193 constants,
  all value lines byte-equal).
- committed `tests/fixtures/wasm/indmach012a.wasm` **unchanged** (PIN hash
  `1849db0c…` holds); the fixture self-gate
  (`committed_fixture_matches_native_twin_bit_exact`) stays **bit-exact green**.
- `crates/dss-usermodel::records::GeneratorVars` stays 244 B — **doc-comment +
  §2.2b framing only**, no code change; the offset-pin tests still assert
  176/188/236.

`build_native.ps1` retargeted r3723→r4133; `build_probes.ps1` gained the P8
step. Full three-command gate green at defaults; `tests/corpus` pristine
(deck driven from an isolated scratch dir, never the corpus). Out of scope
(unchanged): WM.3 element integration, any dss-core/manifest/dss-epri code.

**Settlement (two independent audits, 2026-07-19).** The code audit returned
empty (faithful; wasm side is doc + pin only, native re-freeze backed by
reproduced probe evidence). The test audit raised two **low, by-design**
observations, both settled empirically and **deliberately not "fixed"** (there
is no regression to fix):

- *WM3-1 — the r4133 re-derivation is gated by nothing new in the suite (it
  verifies the old pins, which are identical).* Settled: the IndMach012a model
  units (`IndMach012Model`/`MainUnit`/`ParserDel`/`.dpr`) were re-hashed here and
  are **byte-identical r3723→r4133** (sha256 match p8); the sole delta is the
  engine-only `deltaQNom` in `GeneratorVars.pas`, which never reaches the model.
  Identical math ⇒ identical twin outputs, so the unchanged `twin_expected.rs`,
  unchanged `.wasm`, and bit-exact-green self-gate ARE the correct, sufficient
  verification; the r4133 bridge run (`p8_twin_r4133_bridge.txt`) confirms the
  r4133-compiled twin loads + runs in the r4133 engine. FPC/wasm cannot rebuild
  in CI — this is inherent golden discipline, not a coverage gap; adding a CI
  gate is impossible and unnecessary.
- *WM3-2 — the new native 252-B offsets are asserted only in the ungated
  `twin_probe.py`; the gated `records.rs` test asserts the 244-B wasm image.*
  Settled: the r4133 offset probe (`abi_probe_r4133.pas`) was **re-compiled and
  re-run here** — output byte-identical to `p8_offsets_r4133.txt`
  (`TGeneratorVars` 252 B, `deltaQNom`@176, `NumPhases`@184, `VthevMag`@196),
  matching `twin_probe.py`'s ctypes asserts. The native record has **no host
  codec** in `dss-usermodel` and **never crosses the wasm boundary**, so there
  is nothing in the product to gate it against; the manual twin tool — which
  must match the native DLL byte-for-byte — is its correct and only home. The
  gated `records.rs` pin covers exactly what crosses (the 244-B wasm image). By
  design, not a regression.

### WASM-UM WP-WM.3 — Generator integration + oracle gate (branch `wasm-wm3`, 2026-07-19)

The flagship WP: Generator `UserModel=`/`UserData=`/`ShaftModel=`/`ShaftData=`
over WASM, all `generator.pas` §1.1 call sites, first end-to-end oracle gate
through the r4133 bridge (recorded decision — not pinned dss-python).

**Landed (plan §WP-WM.3 items 1–6):**
1. **Element wiring** — `ShaftModel`/`ShaftData` flip from `NOT_PORTED` to the
   §2.4 uniform rule; `EndEdit` dispatch order (UserModel→UserData,
   ShaftModel→ShaftData) queues a *deferred* load resolved by the executive
   (`exec/command.rs`, path → `.wasm` bytes or `None`); `update_model` after
   `RecalcElementData` (`nominal.rs`). New `generator/user_model.rs`:
   `GenUserModelSlot` (Pascal `TGenUserModel`) wrapping `dss-usermodel`.
2. **Call sites** — `DoUserModel` power-flow (`solve.rs::do_user_model`, negate
   into `InjCurrent`), GenModel=6 dynamics + shaft `FCalc` (`dynamics.rs`),
   `InitStateVars`/`IntegrateStates` FInit/FIntegrate for both models, the
   state-variable surface (`num_variables`/`variable_name`/`get_all_variables`/
   `set_variable` concatenate built-in ++ UserModel ++ ShaftModel). The dropped
   model-6 dynamics error (`dynamics.rs:239-254`) is now a surfaced #5671.
3. **Decks** `tools/golden/wasm_decks/wasm_gen_{pflow,dyn,vars,edit}.dss`
   (`@FIXTURE@` twin form), oracle-validated.
4. **Gate** — `crates/dss-core/tests/wasm_usermodels.rs` (channel 1, hermetic:
   committed `.wasm` vs committed r4133-oracle goldens); goldens generated by
   `crates/dss-epri/tests/gen_wasm_usermodels.rs` (env-gated `WASM_TWIN_DLL`,
   manual only). **pflow/vars/edit match the r4133 oracle at ~1e-14** (feeder
   floor); the `Get StateVar` single-variable path matches too.
5. **Invariant** — the five `expect_warnings` corpus decks + `solvable_now.json`
   manifest byte-unchanged; corpus_live gate green (see item-5 line below).
6. Audits pending (workflow spawns them).

**Golden schema fix (real bug in the WIP).** The golden keyed `variables` by
name in a JSON **map**, which SILENTLY COLLAPSED the dyn deck's 34-var surface to
20 (binding the same IndMach012a as both `UserModel=` and `ShaftModel=` gives 6
built-in + 14 + 14, the 14 ShaftModel names duplicating the UserModel's). The
prior run misread this collapse as "oracle reports 20 vs Rust 34" and got stuck
on a non-existent asymmetry. Fixed: order-preserving parallel `variable_names`/
`variable_values` arrays; a live r4133 probe confirms the oracle reports **34**
(Rust = Pascal `NumVariables` = oracle). Added `all_wasm_decks_have_consistent_goldens`.

**Two MEASURED dss_capi-0.14.5-vs-r4133 divergences (Generator DYNAMICS).** The
dyn deck is the first dynamics comparison against r4133 and exposed them (probed
both, per the brief's "if the two disagree, STOP and record"):
- **D1 — swing-damping default.** dss_capi 0.14.5 `generator.pas:1006` sets
  `Dpu:=1.0` (→ `D=Dpu*kVArating*1000/w0≈13263`); r4133 `generator.pas:968` sets
  `D:=1.0` in the ctor but **never `Dpu`**, so `InitStateVars` recomputes
  `D:=Dpu*…=0`. Rust ports dss_capi (D≈13263), r4133 default D=0. Proven by the
  oracle `DebugTrace` (predictor dSpeed +0.0025 with D≈0 vs −1.165 with D≈13263).
  The dyn deck now sets `D=1` explicitly (both engines agree); recorded in
  `DIVERGENCES.md` L5.
- **D2 — residual dynamics trajectory gap.** With D matched, a residual survives,
  MEASURED at the deck's end state (settlement flipped `numeric=true` transiently
  to quantify): ~5e-4 rel on machine currents (Is1/Ir1), ~1e-3 on losses
  (StatorLoss/RotorLoss/HPshaft), ~1e-4 on Slip, up to ~5e-4 on node voltages,
  dSpeed ~3e-2 (the `Pshaft+TracePower` near-cancellation amplifies the current
  gap). These are **4–6 orders above the faer-vs-KLU floor (1e-8)** — an
  engine-behavior difference, not solver rounding, so a numeric gate is impossible
  without a ~1e-1 band = forbidden fudging. **DECOMPOSITION (settlement, per
  CLAUDE.md prove-by-decomposition):** WM.3's NEW code is exonerated — the WASM
  `FCalc` handoff is bit-exact (Model=6 SNAPSHOT `wasm_gen_pflow` matches r4133 at
  ~1e-14 incl. Is1/slip) and the guest math is bit-exact to the native twin (WM.2
  `fixture_self_gate`). So the gap lives in the **shared multi-step Generator
  dynamics coupling** (dynamics-Norton/Zthev entry + the per-step network re-solve
  feeding Vterminal back to the identical guest), the SAME code family the proven
  D1 `Dpu` divergence sits in — not the new user-model transport. Independent
  corroboration that Rust's Generator dynamics tracks its 0.14.5 spec oracle:
  `exec/tests/dynamics.rs` (Kundur steady + fault) matches pinned dss-python 0.15.7
  to the f32 monitor floor. Pinning the single r4133 source line (like D1's) needs
  the 0.14.5-ABI twin DLL → the OPEN follow-up (deviation (a)); **not a Rust bug on
  the WM.3 surface, not a tolerance-loosening candidate.**

**Gate design consequence.** Rust ports dss_capi 0.14.5 (its pinned oracle);
forcing its dynamics to match r4133 would DIVERGE from the spec, and loosening
the harness floor is forbidden. So the dyn deck gates the **version-independent**
facts robustly — convergence, error-free (no WASM trap/protocol fault through the
user *and* shaft FInit/FIntegrate/FCalc), and the full ordered **34-variable
surface (names + count)** vs the oracle (proving the shaft loaded and the
NumVars/GetAllVars/GetVarName plumbing) — and does NOT floor-compare the
confounded dynamics trajectory (`gate_deck(.., numeric=false)`). pflow/vars/edit
remain full-numeric (~1e-14), proving the WASM `calc`/`edit`/state-var transport.

**Deviations / open items.** (a) D2's single-line r4133 root-cause is the OPEN
follow-up (the gap is decomposed and scoped OUT of the WM.3 surface — see D2
above); needs the 0.14.5-ABI twin DLL. (b) Channel-3 diagnostic (IndMach012a-over-
WASM vs the built-in IndMach012 element) NOT implemented — deferred (the channel-1
gate is the WP's oracle proof; channel 3 is a reported non-gating nicety, and the
two elements' different host coupling makes it a loose visual diagnostic, not a
clean comparison — kept deferred rather than adding a meaningless assertion). (c)
The pinned dss-python 0.14.5 secondary channel skipped (needs the r3723-built twin
— the recorded decision permits skipping if not cheap).

**Settlement (post-audit, two xhigh audits, 2026-07-19).** Five findings settled
empirically:
- **WM3-1 (HIGH, silent fallback on trap) — FIXED.** The inject path built the
  trap / missing-Model=6 diagnostics in a LOCAL `ErrorLog` and dropped them (the
  `inj_currents` trait method has no `Result`). Added `errors`+`solution_abort`
  channels to `InjCtx` (traits.rs); `Generator::inj_currents` drains them into the
  solution `ErrorLog` (= `Dss::errors()`) and lifts `SolutionAbort` from any
  `abort`-flagged diagnostic. A wasm `calc` **trap** is now a hard `abort` (ABI
  §6); the missing dynamics model (#5671) aborts (Pascal `generator.pas:1944`); the
  power-flow #567 surfaces non-abort (Pascal `DoSimpleMsg`). `get_currents`'
  recompute routes to the element deferred-error log instead of dropping. Guarded
  so a NATIVE-DLL `UserModel=` the wasm host cannot load (already loud via #570
  "Not Loaded, falls back") does NOT also spew #567/#5671 — that keeps the corpus
  gate green (indmachtest / Kersting4wire `UserModel=Indmach012a`). New regression
  test `model6_without_usermodel_surfaces_diagnostic`.
- **WM3-3 (GenVars read-back was a 9-field whitelist) — FIXED.** `apply_gen_vars`
  now writes back the FULL 244-B image's mapped f64/Complex fields (the ABI §2
  "unconditional read-back" contract), not a state-only subset; only the structural
  ints (`num_phases`/`num_conductors`/`conn`) stay element-owned (documented).
  Behaviour-neutral for the fixture (it mutates only `Speed`; untouched fields
  round-trip bit-exact) — gate stays green while the frozen contract is honoured.
- **WM3-4 (`set_variable` 1..6 was a silent no-op) — FIXED.** Ported the classic
  GenVars setters (`generator.pas:2650-2663`): Speed/Theta/PShaft/dSpeed/dTheta with
  the unit conversions, index-3 read-only #564, i<1 #565, DynamicEq #566. Reachable
  via the capi015 `set StateVar=x <elem> <var> <value>` form (the natural positional
  syntax is the upstream-broken misroute already reproduced in
  `force_hooks.rs`). New regression test `set_statevar_classic_genvars_mutates`.
- **WM3-5 / audit-tests-2 (state-var floor 1e-6/1e-5 uncalibrated) — RECALIBRATED
  (tightened).** The comment falsely claimed "1e-6 measured"; measuring shows every
  macro state var agrees to <1e-13 rel and only the near-zero quadrature currents
  Is2/Ir2 (~4.3e-7, |abs| gap ~1.4e-13) are loose. New floor `(1e-8 rel, 1e-12
  abs)`: rel = the `feeder` voltage class the machine vars inherit, abs = the
  measured near-zero floor + ~7x margin — **100x tighter (rel) / 1e7x tighter (abs)**
  than before, never loosened. Now Is2/Ir2 are constrained by an abs band instead
  of an 1e-5 band 8 orders above the signal.
- **audit-code WM3-5 (doc inaccuracy) — FIXED.** The `variable_name` comment
  claimed "the gate is designed not to compare shaft names"; corrected to state the
  gate DOES compare all 34 names and why the (correct, non-reproduced-UB) shaft
  names coincide with the buggy upstream `FGetVarName` read.
- **WM3-2 / audit-tests-1 (dynamics numeric gate) — settled by decomposition, no
  code change.** See D2 above: the trajectory gap is proven OUT of the WM.3 surface
  and is a version-divergence family, so structural-only gating is correct; a
  numeric r4133 gate would require forbidden loosening. Monitor mode-1/3 capture is
  subsumed by the same confound (the mode-3 channel IS the 34-var surface, already
  gated).

### WM.3 D2 follow-up — the ~5e-4 gap is a PORT BUG, not a version divergence (branch `wm3-d2`)

The WM.3 settlement HYPOTHESISED D2 (the residual ~5e-4 `wasm_gen_dyn`
Rust-vs-r4133 trajectory gap) is a dss_capi-0.14.5-vs-r4133 engine-version
divergence (D1 family) and gated the deck structurally. The follow-up built the
**0.14.5-ABI twin** and ran the disambiguation the settlement deferred. **That
hypothesis is DISPROVEN — VERDICT (b), PORT BUG.** Evidence
`docs/wasm/probes/p_d2_threeway_portbug.txt`.

- **ABI check (empirical, source-level).** dss_capi 0.14.5 `TGeneratorVars` = 244 B
  (== r3723 GeneratorVars.pas, byte-identical); r4133 inserts `deltaQNom`@176 →
  252 B. The ABI **differs**, so the committed 252-B twin can't drive the pinned
  dss-python 0.14.5. Built the **0.14.5-ABI twin** from r3723 V8 IndMach012a
  (`tools/wasm_usermodel/build_native_r3723.ps1`; model files sha256-identical to
  the r4133 twin — only GeneratorVars differs, so the sole controlled variable is
  the engine version). sha256 `090393…89E0A`, loads + solves the full deck on
  pinned dss-python 0.15.7/0.14.5.
- **Three-way (end-state).** A = Rust+wasm, B = 0.14.5+244B-twin, C = r4133 golden.
  **B vs C ≤ 1.06e-13 on every quantity** (slip/Is1/Ir1/losses/HPshaft/dSpeed/node
  V) — the two engine VERSIONS agree to the faer-vs-KLU floor; there is **no**
  0.14.5-vs-r4133 divergence here. **A vs C == A vs B** (Is1 5.3e-4, Ir1 5.3e-4,
  losses ~1.1e-3, dSpeed 3.4e-2, node-V.im 5.1e-4, slip 9.8e-5) — Rust diverges
  from BOTH oracles, incl. its own pinned 0.14.5 spec, by the identical amount. So
  it is Rust's port that is wrong, not r4133.
- **First divergence.** Step 0 (snapshot, `calc_pflow`) is BIT-IDENTICAL A==B==C
  (1e-14). Step 1 (first dynamics step) already diverges: **Slip matches (2.4e-6)
  but Is1 is off 5.5e-4** for both the user and shaft model instances (which agree
  with each other to ~2e-6, as in the oracle). `is1 = (v1-e1)/zsp` with slip and
  zsp (snapshot) matched → the divergence is in the dynamic flux `e1`/terminal
  voltage `v1` of the Model=6 dynamics network solve.
- **NOT conditioning.** `Set tolerance=1e-12 maxiterations=1000` on both engines
  leaves the ~5e-4 gap intact (each engine's own value moves <7e-6) — they
  converge tightly to DIFFERENT fixpoints (per CLAUDE.md's tighten-the-loop rule),
  a genuine state divergence, not a Norton/Zthev convergence-band artifact.
- **Sub-bug #1 FIXED (`generator/user_model.rs::shaft_model_fcalc`).** Pascal
  `DoDynamicMode:2038` `ShaftModel.FCalc(Vterminal, Iterminal)` OVERWRITES the live
  `Iterminal` (last write), which `IntegrateStates`' `ComputeIterminal` then reuses
  → `TracePower` reads the SHAFT model's currents. Rust discarded them into a
  scratch buffer (kept the user currents). Fixed to write back (Pascal-cited).
  Step-1 `dSpeed` −75.66→−84.80 toward oracle −89.24; **end-state effect negligible
  (Is1 unchanged)** because sub-bug #2 dominates. Contained to Model=6+ShaftModel
  (only `wasm_gen_dyn`; no corpus case); full gate green.
- **Sub-bug #2 OPEN (dominant).** The Model=6 dynamic-current fixpoint is off ~5e-4
  from step 1 with slip matched. Guest math is bit-exact to the twin
  (`fixture_self_gate`), so the dss-core Generator dynamics host feeds the guest
  state differing from what Pascal feeds the twin (checked-and-matched: snapshot
  1e-14, slip, Vterminal mag+angle, w0/Mmass/D/Pshaft). Line-level pin needs
  guest-internal `e1`/`t0p` tracing = rebuilding the WM.4-constraint-frozen fixture
  `.wasm`, out of scope here. **Left as the OPEN follow-up port bug.**
- **Gate design.** The dyn deck stays STRUCTURAL (`numeric=false`) until sub-bug #2
  is fixed — flipping it to numeric now would need a forbidden ~1e-1 band. No
  tolerance/golden/ledger touched. **DIVERGENCES.md gets NO entry** — D2 is a port
  bug, not a version divergence (an entry would misrepresent the finding).

**Settle (two independent read-only audits of `1d94256..cbdff5c`; per-finding
dispositions).** Both audits verified the change does NOT weaken behavior/coverage
(no test/harness/golden/tolerance/ledger/corpus file touched; the fix is
Pascal-faithful; verdict logic is anti-rationalizing — PORT BUG, not a version
hand-wave). Corpus pristine, 186 `.pas` under `.inputs/dss_capi`, full three-command
gate green. Findings settled empirically:

- **AUDIT-CODE D2-1 (medium) — mandate verdict-(b) "fix it" only partly met; the
  DOMINANT sub-bug #2 left OPEN → REGISTER-AS-OPEN (deliberate deferral, not
  resolved).** Reproduced: `p_d2_threeway_portbug.txt:98-111` + gate line
  `wasm_usermodels.rs:352` `gate_deck("wasm_gen_dyn", false)` both stand; sub-bug #2
  is a PROVEN, dominant, still-OPEN port bug. Not fixed here because a line-level pin
  needs guest-internal `e1`/`t0p` tracing, which requires rebuilding the fixture
  `.wasm` — and the fixture crates are frozen by the live parallel **WM.4** workflow
  (`.claude/worktrees/wtWM4`, branch `wasm-wm4`, confirmed active). A fix without that
  trace would violate CLAUDE.md prove-cause discipline (guessing). Disposition: D2 is
  recorded as a **proven-and-open port bug**, NOT a resolved one — see Open
  follow-ups below; the coordinator must carry it forward.
- **AUDIT-CODE D2-2 (low) — `d2_step_0145.py` docstring said "21 dynamics steps" but
  the loop is `range(1, 6)` = 5 → FIXED.** Docstring corrected to state the first 5
  steps (`range(1, 6)`) and that step 1 already exposes the divergence, with the full
  end-state captured by `d2_probe_0145.py`. Cosmetic; no behavior/verdict effect.
- **AUDIT-TESTS D2-1 (low) — shaft-FCalc fix has no numeric regression guard →
  DEFERRED to the sub-bug #2 fix (deliberate).** Reproduced and sharpened: the fix's
  ONLY observable signal is step-1 `dSpeed` (−75.66→−84.80); the deck END-STATE (`Is1`
  unchanged) does not move because sub-bug #2 dominates. So neither the structural
  gate NOR an end-state numeric golden could pin this fix — a guard would need
  per-step (step-1) oracle capture, i.e. new golden infra bound to the WM.4-frozen
  fixtures. The proper trajectory guard therefore arrives WITH the sub-bug #2 fix,
  when the whole trajectory becomes numerically gateable at proven floors. No
  tolerance touched.
- **AUDIT-TESTS D2-2 (low) — structural gate cannot detect worsening of the ~5e-4
  bug; interim known-bad band suggested → interim band DECLINED, DEFERRED (deliberate).**
  An end-state "known-bad-within-N%" band was considered and declined: the trajectory
  quantities span orders (Is1 ~5e-4 … dSpeed ~3.4e-2), so a hand-picked band is
  miscalibration/flake-prone; it would institutionalize a bug we intend to FIX (per
  mandate 3(b) the gate flips to numeric ON the fix, not around it); and the shared
  driver `wasm_usermodels.rs` is also live under WM.4 (conflict risk). Per the mandate
  the numeric gate (and any numeric bound) is explicitly gated on fixing sub-bug #2 —
  done then, at proven floors, never a fudge band now. No tolerance loosened.

**Open follow-up (carry forward):** WM.3 **D2 sub-bug #2** — Model=6 dynamic-current
fixpoint off ~5e-4 from step 1 (slip matched) — is a PROVEN, OPEN Generator-dynamics
port bug (NOT a version divergence, NOT resolved). Fix requires guest-internal
`e1`/`t0p` tracing = an instrumented rebuild of the WM.4-frozen fixture `.wasm`;
unblocks after WM.4 releases the fixture crates. On fixing it: re-measure D2 and flip
`wasm_gen_dyn` to `numeric=true` at proven floors (adds the missing regression guard
for both sub-bugs). `wasm_usermodels.rs:352` stays `false` until then.

### WASM-UM WP-WM.4 — Storage (DynaDLL + UserModel) + PVSystem (UserModel) (branch `wasm-wm4`, 2026-07-19)

Repeats the WM.3 pattern on two more elements. Storage `UserModel=` (15-fn
`TStoreUserModel`) + `DynaDLL=` (13-fn `TStoreDynaModel`) and PVSystem
`UserModel=` (15-fn `TPVsystemUserModel`) over WASM, gated through the r4133
bridge (recorded decision — not pinned dss-python).

**Landed (plan §WP-WM.4 items 1–4):**
1. **Storage element wiring** — `UserModel`/`UserData` flip from `NOT_PORTED` to
   the §2.4 uniform rule; `DynaDLL`/`DynaData` move from the inline warn to the
   deferred-load path (same warn-and-fallback for a native-DLL name, #1570). New
   `UserModelSlot::Dyna`; new `storage/user_model.rs` (`StorageUserModelSlot`,
   both interface kinds). Call sites: `DoUserModel` (VoltageModel=3 pflow,
   `Storage.pas:2103`), `DoDynaModel` (dynamics, `:2211` — Vterminal=NodeV,
   `StickCurr(-DESSCurr)` per phase), `InitStateVars` `FInit` (`:2777`),
   `IntegrateStates` `Integrate` (`:2859`), `RecalcElementData` `FUpdateModel`
   (`:1286`), the `IsUserModel` SOC-skip guard (`:2502`), and the full state-var
   surface (`num_variables`/`variable_name`/`get_all_variables`/`set_variable`
   append UserModel then DynaModel, `:3092-3322`).
2. **PVSystem element wiring** — `UserModel`/`UserData` flip; new
   `pvsystem/user_model.rs` (`PvUserModelSlot`). Call sites: `DoUserModel`
   (pflow + dynamics VoltageModel=3, `PVsystem.pas:1822`/`:1885`), `IntegrateStates`
   `Integrate` (`:2278`), `UpdateModel` (`:1146`), the state-var surface
   (`:2449-2640`). PVSystem `InitStateVars` has NO user-model call (Pascal
   `:2174` never calls `FInit` — verified).
3. **Fixture** — one authored model `tools/wasm_usermodel/models/wm4model/`
   (workspace-excluded): a 3-phase inverter that is a constant admittance in power
   flow and a first-order current lag in dynamics, dispatched on
   `TDynamicsRec.SolutionMode` (like IndMach012a's `Calc`). ONE `.wasm` serves
   BOTH the PVSystem `UserModel=` (15-fn) and Storage `DynaDLL=` (13-fn) gates
   (`new(dynarec)` shape; 13-fn = 15-fn minus save/restore). Reads inputs from
   `V` + `TDynamicsRec` only — NO host callbacks, NO `TStorageVars`/`TPVSystemVars`
   image crosses, so the **frozen ABI is unchanged** (no StorageVars/PVSystemVars
   offset table needed — the plan §2.4 probe extension is not required for a model
   that does not use `get_public_data`). Committed `tests/fixtures/wasm/wm4model.wasm`
   (60587 B, `sha256=dad9e74b…b77b7`, PIN.txt + `fixture_pin.rs` hash-vs-PIN).
   Native twin = the SAME Rust core as a native cdylib (plan §2.6 plan B;
   `build_wm4model_native.ps1`, sha256 `b6934757…1575`, NOT committed) — the r4133
   engine LOADS + RUNS it as Storage DynaDLL / PVSystem UserModel. Decks
   `tools/golden/wasm_decks/wasm_{pv_pflow,storage_dyn}.dss` (`@FIXTURE@` twin
   form), goldens generated by `crates/dss-epri/tests/gen_wasm_usermodels_wm4.rs`
   (env-gated `WASM_TWIN_DLL`, manual), hermetic replay
   `crates/dss-core/tests/wasm_usermodels_wm4.rs`.
4. **Invariant** — the `SimpleStorageTest*` `expect_warnings` corpus decks
   re-verified green with ZERO manifest edits (`git status tests/corpus/manifests`
   clean; full `cargo test --workspace` green — corpus gate 514/514).

**No silent fallback (WM.3 precedent, WM3-1) — settled.** Storage/PVSystem
`inj_currents` built the user/dyna-model diagnostics in a LOCAL `ErrorLog` and
DROPPED them (the same drop the WM.3 generator fix repaired). Fixed: `inj_currents`
drains into `ctx.errors` + lifts `ctx.solution_abort` for `abort`-flagged wasm
traps (ABI §6); `get_currents` routes to the element deferred-error log. New
regression tests `storage/pvsystem_model3_without_usermodel_surfaces_diagnostic`
(#567). Guarded so a native-DLL `UserModel=` name (already warned #1570 at load,
`user_model_name` non-empty) does NOT re-spew #567 — keeps the corpus gate green.

**Both gates are FULL NUMERIC vs the r4133 oracle (no version-divergence
concession, unlike WM.3 D2):**
- `wasm_pv_pflow` (PVSystem UserModel, VoltageModel=3, snapshot): node voltages +
  user-model vars (Iout1/G/B/Tau) match at **worst rel 2.7e-13**.
- `wasm_storage_dyn` (Storage DynaDLL, 2-step dynamics): matches at **worst rel
  4.3e-13**. Unlike the WM.3 Generator D2 divergence (swing-damping/Zthev-Norton
  coupling), the Storage `DoDynaModel` is a **pure per-phase current injection**
  (`StickCurrInTerminalArray(-DESSCurr)`, no swing damping, no Zthev) — there is
  no cross-version coupling to diverge, so the multi-step trajectory gates
  numerically too (MEASURED, per the brief's prove-cause discipline; no ledger
  entry — nothing to ledger).

**Base-surface finding (recorded, not a bug).** The r4133 oracle reports EMPTY
names + `-9999.99` for the 9 base InvDynVars (Storage/PVSystem) outside dynamics,
while the dss_capi-0.14.5 Rust port names/computes them — a base element-variable
surface version divergence, orthogonal to the WM.4 user-model subject. The gate
compares base names only where the oracle is non-empty and does not floor-compare
base var VALUES; the user-model tail (Iout1/G/B/Tau) is fully name+value gated.

**Deviations from the brief.** (a) The plan §2.4 anticipated freezing
`TStorageVars`/`TPVSystemVars` offset tables "with a probe extension" — NOT
required here: the authored fixtures read only `V` + `TDynamicsRec` (no
`get_public_data`), so no StorageVars/PVSystemVars image crosses the boundary and
the frozen ABI is unchanged. Recorded, no ABI change. (b) One `.wasm` + one native
twin serve both decks (the model dispatches on SolutionMode) rather than two
fixtures — the plan sketched a `storagedyn` model; this generalizes it to also
carry the PV pflow case, halving the fixture surface with no loss of coverage
(both the 13-fn DynaDLL and the 15-fn UserModel export sets + call sites are
exercised end-to-end vs the oracle).

**Settle round (2026-07-19).** Two read-only audits (code + tests) reviewed the
branch; every finding was reproduced then settled. No high/medium findings; all
six were low.

- **T-WM4-1 (tests, base var values not gated) — FIXED (strengthened).** Ran
  both decks and dumped Rust-vs-oracle for every base var: ALL non-sentinel base
  vars (physical quantities + the `9999`/`0` operation flags — kWh, kWOut,
  kvarOut, kWTotalLosses, …) match the r4133 oracle at **f64-floor, worst rel
  4.4e-16** — including the Storage 2-step dynamics trajectory (the old doc's
  "trajectory version-divergence confound" is empirically false for these vars).
  `gate_deck` now floor-compares every var except the empty-named `-9999.99`
  InvDynVar sentinels (the real base-surface version divergence). Strictly
  stronger, no tolerance touched. Still green (worst gap unchanged: node voltages
  2.7e-13 / 4.3e-13, the faer-vs-KLU last-ulp signature).
- **T-WM4-2 (tests, dead `numeric=false` branch) — FIXED (removed).** With the
  Storage dynamics trajectory proven to match numerically (T-WM4-1), the
  structural-only path had no live use and no coverage. Removed the `numeric`
  parameter; `gate_deck` is now unconditionally the full-numeric gate.
- **T-WM4-3 (tests, oracle twin shares the model core) — accepted, no fix.**
  Inherent to authored fixtures (no vendored Storage/PVSystem user-model example
  exists) and the sanctioned WM.3 pattern; the cross-engine check is genuine at
  the ENGINE/ABI level (the ~1e-13 faer-vs-KLU voltage gap proves r4133 solved it
  independently, and the goldens carry r4133-only `-9999.99` sentinels the Rust
  port does not emit). Disclosed in `gen_wasm_usermodels_wm4.rs` and above.
- **C-WM4-2 (code, PVSystem scalar `get_pv_variable` lacked the UserModel tail)
  — FIXED.** Threaded `&mut self, sys, node_v` through `get_pv_variable` /
  `get_all_pv_variables` (+ the one accessor caller) and added the
  `i > NumPVSystemVariables → get_user_model_variable` route + `PvUserModelSlot::
  get_variable`, mirroring the Storage sibling and Pascal `PVsystem.pas:2453-2461`.
  (Functionally the user tail was already surfaced via the plural `get_all_variables`
  append; this removes the scalar-helper inconsistency.)
- **C-WM4-3 (code, `refresh_var_cache` swallowed guest traps) — FIXED.**
  `refresh_var_cache` (Storage + PVSystem) now returns `LiveResult` and propagates
  a trapping `num_vars`/`get_var_name` via `?` instead of `.unwrap_or(0)` /
  `.unwrap_or_default()`; the callers (`new`/`edit`/`update_model`/…) already
  drain it, so a trap is surfaced loudly (plan §2.9-5) rather than silently
  dropping the model's state-var tail.
- **C-WM4-1 (code, `#567` suppressed for a named-but-unloaded model) — accepted,
  no fix.** Reproduced: the VoltageModel=3 `DoUserModel` path guards `#567` behind
  `user_model_name.is_empty()`, so a named-but-unloaded model (native-DLL name or
  a bad `.wasm`) does not emit the per-solve `#567` Pascal produces on
  `not UserModel.Exists`. Kept deliberately: (1) the failed-load state is already
  **surfaced loudly** once via `#1570` at load (satisfies plan §2.4 rule-4); (2)
  for the dominant wasm case — a native-DLL name — Pascal would LOAD the DLL
  (`Exists=true`, zero `#567`), so per-solve `#567` would diverge *further* from
  Pascal, not less; (3) diagnostic-only (both paths inject Yprim-only, numerically
  identical); (4) empirically UNEXERCISED — no corpus deck sets Storage/PVSystem
  `UserModel=`+VoltageModel=3 (SimpleStorageTest uses `DynaDLL=` with the default
  voltage model in snapshot mode, so `DoUserModel` is never reached). The primary
  intent — `#567` for a genuine no-model VoltageModel=3 — is enforced and tested
  (`{storage,pvsystem}_model3_without_usermodel_surfaces_diagnostic`).
- **SimpleStorageTest invariant re-verified** (settle): `git status tests/corpus`
  clean; `SimpleStorageTest*` decks green under the full `cargo test --workspace`.

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

### OG-1.5b `CAPI_Schema` per-enum walk + class-walk groundwork (orphaned-gaps round, 2026-07-18)

Branch `og15b-schema-full`. Continues OG-1.5 (static core) toward the full
`DSS_ExtractSchema(jsonSchema=True)` document. **Delivered, byte-exact:** the
per-enum walk — `prepareEnumJsonSchema` (`CAPI_Schema.pas:111-162`) + the 21-entry
`DSS.Enums` global list ported directly from `DSSClass.pas:1058-1196` (names /
ordinals / alt-names, with the symbolic ordinals resolved from their Pascal enum
declarations, never seeded from the oracle JSON).
- New `report/export/json/schema/enums.rs` (`schema.rs` → `schema/mod.rs`
  directory module). `global_enum_defs()` reproduces all 21 enum `$defs`
  byte-for-byte incl. the AltNames-drop (LengthUnit/SolveMode), AltNamesValid=false
  (CoreType), the `Wye/Delta` connection remap, and the `oneOf` hybrids
  (MonitoredPhase, PlotProfilePhases).
- `gen_schema.py` now captures the 21 enum `$defs` (self-validated by verbatim
  containment in the 592 KB oracle output); `golden_schema.rs`
  `global_enum_defs_bytes_match_oracle` byte-tests them. Gate green.

**Groundwork for the class walk (not yet ported):** `tools/golden/extract_schema_help.py`
extracts the property-help catalog from the dss_capi gettext resource
`dss/messages/properties-en-US.mo` (the source `TDSSClass.GetPropertyHelp` reads,
`DSSClass.pas:2166` — NOT the schema JSON, so faithful, not circular). Validated:
that catalog reproduces **every** schema-emitted description (948 main + the
specset clones) via `<Class>.<proplower>` leaf lookup + the array-alternative name
redirect (Transformer/AutoTrans/XfmrCode Conn→Conns etc.); no class-parent
fallback is ever needed (`DynamicExp.Like` correctly falls to the literal key). The
`AltPropertyOrder`/`$dssPropertyOrder` computation is fully located
(`DSSClass.pas:1830` `nextByZOrder` + `:1934-2010`; `zorderNextStart=-999`,
`zorderNextEnd=999`, `MakeLike=-1000`; `Ordering_First/Last` used by only 4
classes). Defaults are read from the port's live sample object (constructors
compute them, e.g. Capacitor `norm_amps`), and the fpjson float formatter
(`fpjson_float`) + `get_json_value` accessors are reused.

**Still open (the remaining ~bulk of §1.5):** the per-class walk
(`prepareClassJsonSchema`) — 49 class `$defs` + `List`/`Container` triples +
`circuitProperties` refs + ~40 class-local enum defs. The dominant remaining cost
is per-property metadata the port never carried: the `Units_*` `PropFlags` family
(only 2 of ~30 exist; nearly every electrical class needs them), the
`Required`/`Ordering_First`/`Ordering_Last`/`PowerFactorLimits`/`PDElement` flags,
the 24 classes' `SpecSets`, and the local enums' `AltNames`/`JSONName`. This is a
large, self-contained data-entry effort (recorded in ORPHANED_GAPS §1.5). The
`# Incomplete` caveat on `Dss::extract_schema_json` therefore stays until the class
walk lands. No engine behavior changed.

**Settlement (two audits, 2026-07-18).** Both audits confirmed the delivered
enum surface is faithful, byte-exact vs the oracle (zero unexplained diffs, no
loosened tolerance, no golden regenerated from Rust output), and that the scope
shortfall is honestly disclosed, not hidden. Dispositions:
- *DONE-bar completeness not met* (audit-code OG15b-1 / audit-tests F1, F2) —
  **acknowledged, deferred.** The remaining bulk (49-class walk + 4 metadata
  families + the expected-divergence inventory / full-document byte-compare) is a
  large self-contained implementation, not a settlement fix; it stays open above
  and in ORPHANED_GAPS §1.5. The `# Incomplete` caveat is retained deliberately.
  For the *delivered* enum surface there is no divergence inventory because zero
  divergences were observed — all 21 global enum `$defs` byte-match oracle 0.14.5
  (stricter than an inventory: any diff fails `global_enum_defs_bytes_match_oracle`
  at zero tolerance).
- *Enum walk not wired into the runtime `extract_schema_json`* (OG15b-2 / F1) —
  **won't-fix (deliberate), documented.** `global_enum_defs()` is verified
  groundwork; the oracle `$defs` order is 10 static → 21 enums → class triples
  (`CAPI_Schema.pas:1476-1502`), so the enums are spliced by the class walk that
  assembles + byte-gates the full document. Wiring a standalone 10+21-def
  intermediate now would surface an unverified integrated state the class walk
  supersedes immediately; the skeleton stays the skeleton.
- *Stale module doc* (OG15b-3) — **fixed.** `schema/mod.rs` no longer lists the
  per-enum walk as "Deliberately NOT ported"; it now points at the `enums`
  submodule and scopes the deferral to the per-class walk + class-local enums.
- *Help-catalog provenance* (OG15b-4) — **acknowledged, deferred with the class
  walk.** Confirmed empirically: the `.mo` is genuinely absent from the vendored
  source, and `TDSSClass.GetPropertyHelp` → `DSSHelp` → `TMOFile('locale/en_US.mo')`
  (`DSSClass.pas:1052`, `DSSGlobals.pas:724`) reads exactly that gettext catalog,
  so sourcing from the pinned package's `properties-en-US.mo` is the engine's true
  source, not oracle-JSON seeding. `extract_schema_help.py` is committed as an
  unwired one-off; `help.rs` is neither generated-in-tree nor consumed. When the
  class walk consumes it, re-derive from the `.po` at rev 0.14.5 in
  `.inputs/dss_capi_with_git` for vendored provenance and re-scrutinize.
- *Stale golden bookkeeping* (F3) — **fixed.** `gen_schema.py` no longer lists the
  now-ported 21 global enums under `deferred_enum_def_*`; the golden was
  regenerated with the pinned oracle (592679 bytes, deterministic across 2
  processes) — a 24-line deletion of the two contradictory keys only, every
  byte-gated fragment (`global_defs`/`enum_defs`/`circuit_head`) unchanged.

### OG-1.5c `CAPI_Schema` per-class walk — infra + pilot (2026-07-18)

Branch `og15b-schema-full` (continues OG-1.5b). Ported the per-class walk
`prepareClassJsonSchema` (`CAPI_Schema.pas:325-1134`) loop-for-loop as
`report/export/json/schema/classes.rs::class_schema`, exposed via
`Dss::schema_class_def(class_name)`. This is the **infra stage** of a
multi-agent rollout: all shared machinery is built and proven byte-exact on a
6-class **pilot** (the first six `DSSClassList` classes: LineCode, LoadShape,
TShape, PriceShape, XYcurve, GrowthShape); the remaining 43 classes are ported
by sequential class-batch agents, then the full-document splice by an
integration agent.

Machinery built (all shared):
- **`classes.rs`** — the walker: the `PropertyTypeJson` mapping reconstructed from
  the port's merged `PropType`+flags (`pascal_jtype`), `extractUnits`
  (`extract_units`), the scalar/array/matrix/enum/object-ref default extraction
  with the exact Pascal elision rules, `$dssPropertyOrder` from the ported
  `AltPropertyOrder` (`index_of_in`), `$dssLength`/`$dssShape`/`$dssPropertyIndex`,
  the `SpecSets`→`oneOf`+`toRemove`+`RequiredInSpecSet` block (incl. the
  redundant-member abort), and class-local enum `$defs`.
- **`PropFlags`** widened to `u128`; the **`Units_*` family** completed (26 new,
  all 28 now present), plus `PD_ELEMENT`/`POWER_FACTOR_LIMITS` and a port-only
  **`BOOLEAN_ACTION`** marker (restores `BooleanActionProperty`'s `writeOnly` +
  end-ordering, which the `PropType::Boolean` merge dropped — LineCode `Kron`).
- **`spec_sets.rs`** — schema-side port of `TDSSClass.SpecSets`/`SpecSetNames`
  (`RequiredInSpecSet` stays the per-`PropDef` flag, matching Pascal).
- **`enums.rs`** — the enum rendering factored into a shared `render_enum` +
  `enum_json_name` + `is_global_enum_json_name`, reused for class-local enums;
  `EnumMeta` builds them from the port's `DssEnum` (+ an `enum_overrides` hook for
  the AltName-dropping / renumbering / integer locals the batches need).
- **help** reused from `report::help_catalog::dss_help` (`<Class>.<proplower>`) —
  no duplicate table.
- **`gen_schema.py`** captures each ported class's oracle fragment verbatim
  (`SCHEMA_CLASSES`, `slice_class_def`); **`golden_schema.rs`
  ::ported_class_defs_bytes_match_oracle** byte-compares Rust vs oracle **after**
  applying the committed **expected-divergence inventory**
  (`tests/golden/json/schema_divergences.json`) with **fail-on-stale** (a
  divergence whose occurrence count no longer matches fails; canary-verified).

Pilot result: 5/6 classes byte-exact; LineCode byte-exact **after** its 3
documented r4133 divergences (FaultRate/PctPerm/Repair `deprecated:true` — the
port follows dss_capi 0.15.x/r4133 per WP-U1.4; the pinned 0.14.5 oracle omits
the flag). Two genuine port-metadata fixes made along the way: LineCode `NPhases`
lost a spurious `NonNegative|NonZero` (Pascal has none — it validates in the
`Set_NumPhases` side effect; the flag both over-rejected `nphases<=0` at parse
and leaked `exclusiveMinimum:0` into the schema), and the shape classes' file
props gained the `NPts` `size_prop` (`getSizePropertyIndex`'s `GlobalCount`
branch → `$dssLength`). `extract_schema_json` still returns the skeleton (its
`# Incomplete` caveat stands until the integration splice). Gate green
(fmt/clippy/test); `tests/corpus` pristine. Pattern doc for the batch agents in
the coordinator's scratchpad (`schema_pattern.md`).

#### OG-1.5c batch B2 — sources + Load byte-exact; XfmrCode/Line deferred (2026-07-18)

Delivered **Vsource, Isource, VCCS, Load** byte-exact vs the pinned 0.14.5
oracle (added to `SCHEMA_CLASSES`; `ported_class_defs_bytes_match_oracle`
covers them). Zero divergence-inventory entries. Work was per-class metadata
(the `Units_*`/`NonNegative`/`NoDefault`/`DynamicDefault`/`RequiredInSpecSet`/
`PowerFactorLimits` flags from each `DefineProperties`) plus `spec_sets.rs`
entries for Vsource (4 `oneOf`; the 5th `R0,X0,R1,X1` set auto-aborts on the
Redundant members) and Load (5 `oneOf`).

Two **shared walker-infra gaps** the pilot never exercised were fixed in
`classes.rs` (both needed by every remaining batch):
- **scalar `Bus` default** — the walker read `get_string(propIndex)` for a
  `#/$defs/BusConnection` prop; a Bus value is the terminal's bus name
  (`class_props/json.rs:101`, `obj.GetBus(PropertyOffset)`), so it now reads
  `get_bus_name(pd.size_prop)` (matches Pascal; unset ⇒ elided).
- **scalar `DSSObjectReferenceProperty`** — the enum/object-ref chain handled
  mapped enums but not scalar object refs, so `LineCode`/`Yearly`/`Spectrum`/
  `BP1`… emitted no `type`. Added the `:818-895` arm: `type:string`
  (+`minLength`/`maxLength` for a fixed-class ref, `object_class != Some("")`),
  default = the referenced object's name when set (e.g. `Spectrum:"default"`).

`enum_overrides` gained two arms (the port's runtime `DssEnum` carries no
AltNames/`JSONUseNumbers` — schema-only): **`Connection`** (`Wye`/`Delta`
AltNames, so a sequential `Conn` default renders `"Wye"`) and **`Load: Model`**
(`JSONUseNumbers=true` integer enum + `ConstantPQ`… AltNames, matching
`Load.pas:339`). Note: Load `kW`/`kvar`/`kVA` carry **no** `Units_*` flag — the
pinned 0.14.5 oracle backend has none; the vendored source added them
post-0.14.5 (`dss_capi 90c572e4` "AltDSS-Schema: More units"), an engine-inert
schema-only change the port has not adopted (kept byte-exact vs the oracle,
zero inventory).

**Deferred: XfmrCode + Line.** Both are winding/conductor struct-array classes
whose winding props (`kV`/`kVA`/`Tap`/`%R`/`Conn` with array-alternative to the
plural `kVs`/…, and `RNeut`/`XNeut`/`MaxTap`/`MinTap`/`NumTaps`/`RDCOhms` as
direct `DoubleOnStructArray`/`IntegerOnStructArray`) render as **per-winding
arrays** with `$dssIterator:"Wdg"`. The port models them as plain scalars, so
the schema needs scalar-on-struct-array render-as-array support + the
`$dssIterator` metadata (`PropertyIteratorPropertyIndex`, `CAPI_Schema.pas:929`)
— shared infra with Transformer/AutoTrans (batches B3/B6). Line additionally
carries r4133 divergences (the `Conductors`/`EpsRMedium`/`HeightOffset` adds,
the `Wires`→`Conductors` JSON rename) needing inventory. Both are removed from
`SCHEMA_CLASSES` (like LineGeometry) pending that struct-array build-out; the
gate stays green.

#### OG-1.5c batch B4 — generation + storage byte-exact; Relay deferred (2026-07-18)

Delivered **Generator, GenDispatcher, Storage, StorageController** byte-exact vs
the pinned 0.14.5 oracle (added to `SCHEMA_CLASSES`). Per-class metadata: the
`Units_*` family (kV/kW/kVA/kvar/kWh/Hz/ToD-hour), `NoDefault`/`DynamicDefault`
on the derived doubles (Generator Maxkvar/Minkvar/kVA/kvar, Storage
kWhStored/AmpLimit), `PowerFactorLimits`+`RequiredInSpecSet` on the PF/kW/kvar
spec-set members, plus `spec_sets.rs` for Generator (`kW,pf`/`kW,kvar`) and
Storage (`kWRated,PF`/`kWRated,kvar`). Enum overrides: `Generator: Model`
(`JSONUseNumbers` integer enum + `ConstantPQ`… AltNames) and the two
`StorageController` mode enums (AltNames drop the `I-Peakshave`→`IPeakshave`
hyphen, `StorageController.pas:292-299`). BaseFreq gained the missing
`DynamicDefault`+`Units_Hz` (`CktElementClass.pas:96`) on all four classes.

**Shared walker/infra fixes** (pilot never exercised these; every remaining
batch benefits):
- **`MappedStringEnumArray` default getter** — the enum-array default read only
  `get_struct_i32_array` (on-struct-array enums, e.g. Transformer `Conns`); a
  plain per-phase `MappedStringEnumArray` (Relay/Fuse `Normal`/`State`) reads via
  `get_enum_array`. Split by `ptype` in `classes.rs`.
- **`DynInit` synthetic property** — `TDynEqPCEClass` classes (Generator, Storage,
  PVSystem) get a trailing `DynInit` (`$ref:DynInitType`, `$dssPropertyOrder =
  maxZorder+1`), `CAPI_Schema.pas:1106-1113`. Detected by the two defining props
  (`DynamicEq`+`DynOut`).
- **`READ_ONLY` PropFlags** (new, schema-only) — a Pascal `SilentReadOnly` prop
  with a *real* `PropertyOffset` (StorageController `kWNeed`, Storage `SafeMode`):
  schema `readOnly`+no-default, yet still returns its value on `?`/props and is
  kept in the JSON export (unlike function-only `SILENT_READ_ONLY` → `''`/omit).
- **`DoubleArray` + IndirectCount** — GenDispatcher `Weights` (Pascal
  `DoubleArrayProperty`) and StorageController `Weights` (`DoubleDArrayProperty`)
  were mis-modeled as `DoubleVArray` (rendered `numberArray`); now `DoubleArray`
  (renders `ArrayOrFilePath` + `$dssLength: GenList`/`ElementList`) with the count
  resolved via `get_i32(<listProp>)` = FListSize/FleetSize. The four
  StorageController fleet-aggregate readbacks became `Double`+`SILENT_READ_ONLY`
  (were `String`); Storage `%Idlingkvar` (Pascal `DeprecatedAndRemoved`) became
  `SUPPRESS_JSON` (skipped from schema + order, exactly as the walker skips a
  DeprecatedAndRemoved ptype).

**Divergence inventory (+2, both Generator):** `units:"kW"` (×2, in both spec
sets) and `units:"kvar"` (×1) — dss_capi `90c572e4` "AltDSS-Schema: More units"
added `Units_kW`/`Units_kvar` to Generator.kW/kvar *after* 0.14.5; the port
follows the vendored source, so the oracle backend omits the unit (same wave as
the existing Capacitor.kvar entry). Storage kW/kVA/kvar `Units_*` predate 0.14.5
(oracle has them) → byte-exact, no inventory.

**Relay deferred** to integration as a **port-authored (r4133) class** (like
LineGeometry/WindGen): the port ports OpenDSS r4133-trunk Relay (75 props —
`PhCurve`/`OC_GndCurve`/`PhPickup` renamed from the 0.14.5 `PhaseCurve`/
`GroundCurve`/`PhaseTrip`, plus `DOC_*`/`SinglePhTrip`/`Lock`/`RatedCurrent`/
`InterruptingRating`/per-phase `Normal`/`State` arrays), diverging **structurally**
from the pinned 0.14.5 oracle (53 props, classic names). It cannot be byte-compared
against 0.14.5 and needs a port-output reference; removed from `SCHEMA_CLASSES`
with a deferral note in `gen_schema.py`.

#### OG-1.5c batch B5 — protection + PV byte-exact; Recloser/SwtControl deferred (2026-07-18)

Delivered **PVSystem, UPFC, UPFCControl, ESPVLControl, IndMach012** byte-exact vs
the pinned 0.14.5 oracle, and **Fuse** byte-exact after 4 documented r4133
divergences (all added to `SCHEMA_CLASSES`).

**Per-class metadata fixes:**
- **BaseFreq** gained the missing `DynamicDefault`+`Units_Hz` (`CktElementClass.pas:97`)
  on all six B5 classes that lacked it (Recloser/Fuse/SwtControl/PVSystem/UPFC/
  UPFCControl/ESPVLControl/IndMach012 — the deferred two keep it too).
- **UPFC**: `PowerFactorLimits` (PF), `Units_Hz` (Frequency), `Units_kvar`
  (kvarLimit) — schema-only flags that were deliberately omitted pre-schema
  (`UPFC.pas:242-257`); `UPFC: Mode` enum override (`JSONUseNumbers` integer enum,
  AltNames `VoltageRegulator`… strip spaces/parens, `UPFC.pas:188-193`).
- **IndMach012**: `Units_kV` on kV, `Required` on kW/kVA (`IndMach012.pas:322-337`).
- **ESPVLControl**: the three `*Weights` were mis-modeled as `DoubleVArray`
  (`numberArray`); now `DoubleArray` + IndirectCount over the matching name-list
  (`ESPVLControl.pas:203-219`) → `ArrayOrFilePath` + `$dssLength: <List>`, count via
  new `get_i32(<List>)` = the list size.
- **PVSystem**: `Units_kV` (kV), `PowerFactorLimits`+`RequiredInSpecSet` (PF),
  `PVSystem: Model` enum override (integer, `ConstantP_PF`/`ConstantY`/`UserModel`),
  the `PF`/`kvar` spec sets (`spec_sets.rs`, `PVsystem.pas:432-439`), an explicit
  `json_name("PTCurve")` for `P-TCurve` (Pascal `PropertyNameJSON`, `PVsystem.pas:430`
  — the default `-`→`__` map gave `P__TCurve`), and `READ_ONLY` (not
  `SILENT_READ_ONLY`) on SafeMode so the schema is `readOnly`+no-default while the
  text/JSON dump still reads the live Yes/No (cf. Storage.SafeMode).
- **Fuse** (byte-exact after divergences): dropped-metadata bugs fixed — `Units_s`
  on Delay and `Deprecated` on Action (both present in 0.14.5 and dss_capi HEAD,
  `dss_capi_with_git` `src/PDElements/fuse.pas`).

**Shared walker fix:** the `MappedStringEnumArray` default was read from the
full fixed-`FUSEMAXDIM` buffer (Fuse/SwtControl rendered 6 `closed` states) —
now truncated to `array_size(propIndex)` (`GetFuseStateSize`), exactly as the
JSON dump already does (`class_props/json.rs:168` `.take(n)`), matching Pascal
`GetIntegers`' `aDim[0]` count (`CAPI_Schema.pas:800`). Pilot/B1-B4 never hit
this (Relay/Fuse/SwtControl were the first per-phase enum-array classes; Relay
deferred).

**Divergence inventory (+4, all Fuse):** the port ports OpenDSS **r4133** Fuse
(WP-U2.1): `FuseCurve` default (`Tlink`→`none`) + help, `RatedCurrent`
repurposed from the TCC divisor (default 1.0) to an informational rating
(default 0.0) + help — two `port_changed_line` (default+description 2-line
blocks, embedded-CRLF to disambiguate the shared `0.0` default) — plus the two
new props `CurveMultiplier` (the new divisor) and `InterruptingRating`, two
`port_extra_property` at ordinals 11/12. Cause = r4133 Version8 `Controls/fuse.pas`.

**Recloser deferred** to integration as a **port-authored (r4133) class** (like
Relay/LineGeometry): the port ports OpenDSS r4133-trunk Recloser (46 props —
`PhaseFast`/`PhaseDelayed`/`GroundFast`/… renamed to `PhFastCurve`/`PhSlowCurve`/
`GndFastCurve`/… with the classic names kept as deprecated aliases, plus
`MechanicalDelay`/`SinglePhTrip`/`Lock`/`Reset`/`RatedCurrent`/`InterruptingRating`),
diverging **structurally** from the pinned 0.14.5 oracle (24 props, classic
names). Needs a port-output reference; removed from `SCHEMA_CLASSES`.

**SwtControl deferred** to integration as a **port-authored (r4133/0.15.x)
class**: the port adopted the 0.15.x property model (D12/WP-U1.6 — Normal→
NormalState, State→PresentState instead of the shared 0.14.5 CurrentAction; the
State `ReadByFunction=GetState` "no controlled element → CTRL_NONE" semantics
are intentionally not modeled in the accessor), plus the OpenDSS r4133
`RatedCurrent` prop and the 0.15.x help catalog (deprecated Delay, per-phase
Normal/State help). Its Normal/State schema **defaults** therefore differ from
the 0.14.5 oracle in getter semantics (not a clean line diff; State renders
`closed` where both oracle versions render `null` via GetState), so it needs a
port-output reference. The inert `Units_s` (Delay) + BaseFreq schema flags were
still corrected in source; the behavioral `Reset` BooleanAction `writeOnly` and
the State getter are left for the integration pin. Removed from `SCHEMA_CLASSES`.

#### OG-1.5c batch B6 — GIC + inverter/converter controls + metering byte-exact (2026-07-18)

Delivered all ten B6 classes byte-exact vs the pinned 0.14.5 oracle:
**GICsource, InvControl, ExpControl, GICLine, GICTransformer, VSConverter,
Monitor, EnergyMeter, Sensor** with zero divergences, and **AutoTrans** byte-exact
after 3 documented r4064 BH-curve divergences (`port_hidden_property`, mirroring
Transformer). All added to `SCHEMA_CLASSES`.

**Per-class metadata fixes:**
- **BaseFreq** gained the missing `DynamicDefault`+`Units_Hz`
  (`CktElementClass.pas:97`) on all ten classes (they lacked it).
- **AutoTrans** (the Transformer struct-array sibling): wired the per-winding
  `array_alternative` redirects (kV/kVA/Tap/%R/Bus/Conn → their plurals) +
  `REDUNDANT` plurals, `ON_ARRAY` on the no-plural scalars (RDCOhms/MaxTap/MinTap/
  NumTaps) — with new struct-array getters in `accessors.rs` for those four (the
  schema sweep reads them) — `INTEGER_STRUCT_INDEX` on Wdg, the `XHX,XHT,XXT` /
  `XscArray` spec sets, units (kV/Thermal-hour/FLRise-HSRise-°C/NormHkVA-EmergHkVA-
  kVA), `AutoTrans: Connection` enum override (`Wye`/`Delta`/`Series` AltNames), and
  **removed the erroneous `SUPPRESS_JSON` from NormAmps/EmergAmps** (AutoTrans has
  no such Pascal override, unlike Transformer). (`AutoTrans.pas:320-557`)
- **InvControl**: `RoCEnum.JSONName` override (`InvControlRateOfChangeMode`),
  `InvControl: Control Model` enum → `JSONUseNumbers` integer enum, `Units_s` on
  LPFTau, and `VV_RefReactivePower` (Pascal `DeprecatedAndRemoved`) → `SUPPRESS_JSON`
  (skips schema/JSON export + AltPropertyOrder while the `?` surface keeps '', like
  Storage `%Idlingkvar`). (`InvControl.pas:444-449,525,571`)
- **GICsource/GICLine**: the `Volts,Angle` / `EN,EE,Lat1,Lon1,Lat2,Lon2` spec sets
  + units (deg/Hz/V·km⁻¹/V/Ω/µF); GICLine's Spectrum+BaseFreq are `SuppressJSON`
  after inherited → `SUPPRESS_JSON_LATE` (`GICLine.pas:249-250`). GICLine Angle
  carries no unit (unlike GICsource's).
- **GICTransformer**: `R1,R2` / `pctR1,pctR2` spec sets + units (Ω/kV/MVA).
- **VSConverter**: `Units_ohm` on RAC/XAC. **ExpControl**: `Units_s` on VRegTau.
- **Sensor**: `kWs,kvars` / `currents` spec sets, `Clear` → `BOOLEAN_ACTION`
  (writeOnly + ordering-last), `Units_kV` on kVBase. **Monitor**: Element default.
- **EnergyMeter**: `Option` StringList default (`[E,R,C]`), `3PhaseLosses` →
  `json_name("ThreePhaseLosses")`, SAIFI/SAIFIkW/SAIDI/CAIDI/CustInterrupts →
  `READ_ONLY` (schema `readOnly`+no-default, props keeps stored value).

**Monitor/EnergyMeter Element default = `Vsource.source`:** Pascal `Create`
defaults the metered element to the first circuit element (the auto-source,
`Monitor.pas:482`/`EnergyMeter.pas:959`); the oracle schema runs
`new circuit.defaults` (`CAPI_Schema.pas:1264`) so the sample sees it. The port's
constructors now default `element_full_name` to `"Vsource.source"` (every real
deck overrides it via the Element property before solve; only the all-default
sample's schema default observes it).

**Shared walker fix:** `string_array_default` now handles `PropType::StringList`
(EnergyMeter `Option`/`ZoneList`, InvControl `MonBus`) via `get_string_list` — the
anticipated batch-owned extension (previously returned `None`).

**Divergence inventory (+3, all AutoTrans):** BHPoints/BHCurrent/BHFlux — the
r4064 (dss_capi 90962ae8) GICharm BH-curve props the port carries `SUPPRESS_JSON`
(port-only, absent from 0.14.5), occupying `$dssPropertyIndex` 42-44 and shifting
the following indices +3 (`$dssPropertyOrder` untouched). Three `port_hidden_property`.

#### OG-1.5c integration — full-document splice + XfmrCode/Line + port-authored pins (2026-07-19)

Closed §1.5. `Dss::extract_schema_json` now emits the **whole**
`DSS_ExtractSchema(jsonSchema=True)` document (the `# Incomplete` caveat is gone):
`schema::assemble_full_document` splices the 10 static + 21 enum `$defs`, then per
class in `schema::DSS_CLASS_LIST_ORDER` (Pascal `DSS.DSSClassList`, 49 oracle
classes + WindGen after Generator) the `$defs/<Class>` + `<Class>List`/
`<Class>Container` triple + `circuitProperties` container ref, and the envelope
(`CAPI_Schema.pas:1479-1513`). Byte-verified 181 `$defs` (10+21+50·3).

**XfmrCode + Line finished byte-exact** (the two B2-deferred struct/spec-set
classes; added to `SCHEMA_CLASSES` → 45 byte-gated):
- **XfmrCode** — wired the Transformer winding pattern (array_alternative kV/kVA/
  Tap/%R/Conn→plurals + `REDUNDANT`, `ON_ARRAY` on RNeut/XNeut/Max/MinTap/RDCOhms/
  NumTaps + struct getters, `INTEGER_STRUCT_INDEX` on Wdg), units (kV/Ω/hour/°C/
  kVA), `DYNAMIC_DEFAULT` on NormHkVA/EmergHkVA/XSCArray, the `X12,X13,X23`/`XscArray`
  spec sets. 1 divergence: 3 `exclusiveMinimum` lines (X12/X13/X23 `NonZero` added
  post-0.14.5, git `69fca934`, first in 0.14.6a1).
- **Line** — 5 spec sets (LineCode/LineGeometry/"Spacing, Wires"/Z0Z1C0C1/ZMatrix,
  the B0/B1 set auto-aborts) + `REQUIRED_IN_SPEC_SET` members, missing units
  (rho Ωm, C0/C1/CMatrix nF), RMatrix/XMatrix `NO_DEFAULT`, BaseFreq
  `DynamicDefault`+`Units_Hz`. **Walker fix** (`schema/classes.rs`): the schema now
  skips `hidden_from_full_enum()` (HIDE_015X/HIDE_R4133) props, matching the JSON
  dump (`build.rs`) — the 0.14.5-pinned schema must not emit 0.15.x/r4133 props.
  4 divergences: EpsRMedium/HeightOffset/HeightUnit/Conductors are HIDE_015X (in
  `AltPropertyOrder`), a new `port_hidden_property` variant carrying `order` renumbers
  both `$dssPropertyIndex` and `$dssPropertyOrder` (+4 on the tail).

**Port-authored classes (5, not byte-gated vs 0.14.5)** — pinned to the port's own
bytes and inventoried (`schema_divergences.json::port_authored_classes`, cause-attributed):
LineGeometry (0.15.x Conductors model), Relay/Recloser (r4133 renamed protection
curves), SwtControl (0.15.x per-phase Normal/State getter semantics), WindGen (the
50th class, absent from the 49-class 0.14.5 oracle).

**Verification** (`golden_schema.rs`, 9 tests): `ported_class_defs_bytes_match_oracle`
(45 classes after divergences); `full_document_matches_port_golden`
(`schema_full_port.json`, regression); `full_document_reconciles_with_oracle`
(vs the verbatim `schema_full_oracle.json` — scaffolding byte-identical, each class
region == `schema_class_def`, byte-gated classes == oracle-after-divergences, WindGen
scaffolding == the substituted-name formula); `every_class_is_gated_or_inventoried`
(gated ⊕ port-authored split, fail-on-stale). Both fail-on-stale canaries proven
(falsified divergence count → per-class fails; removed port-authored entry → split
fails). **Divergence inventory total: 30 per-property (LineCode 1, CNData 1,
LineSpacing 5, XfmrCode 1, Line 4, Capacitor 1, Transformer 3, AutoTrans 3,
RegControl 5, Generator 2, Fuse 4) + 5 port-authored classes.**
Full gate green; `tests/corpus` pristine.

**Settle round (2026-07-19, two audits).** Both audits confirmed the class walk is
a faithful `prepareClassJsonSchema` port with no weakening: full-document equality
is real byte equality, goldens are oracle-sourced (`gen_schema.py` pin-check +
two-process determinism), and fail-on-stale is real (exact occurrence counts,
gated ⊕ authored XOR). Three low findings, all settled empirically:
- **Completeness guard (FIXED).** `extract_schema_json` drove the walk only from
  the static `DSS_CLASS_LIST_ORDER`; a future-registered class not added to the
  list would be silently omitted (Pascal walks the *live* `DSS.DSSClassList`).
  Added a registry-coverage assert (`class_defs.len() == self.classes.len()`) at
  the walk site — every listed class is already proven registered, so equal counts
  make it a bijection. Exercised by every `extract_schema_json` call (the skeleton
  + full-document tests). Confirmed 50 registered == 50 listed today.
- **4 port-authored classes get no oracle content byte-gate (accepted).**
  LineGeometry/Relay/Recloser/SwtControl exist in the 0.14.5 oracle but their
  content is pinned only to `schema_full_port.json` (self-referential); WindGen is
  the 50th class, absent from the oracle. Empirically confirmed genuine structural
  divergence, not a lazy escape hatch: property-set deltas LineGeometry ±10, Relay
  +22/−10 (renamed protection curves), Recloser +23. **SwtControl** — the audit's
  specific worry — was verified in detail: beyond the extra `RatedCurrent` prop its
  `$dssPropertyOrder` remaps non-monotonically (Reset 8→10 while others shift by the
  insert), the oracle carries `writeOnly` on Reset that the port lacks, and Lock/
  Delay/Normal/State help+defaults all changed (r4133 per-phase NormalState/
  PresentState model). That order remap is **not expressible** in the per-property
  divergence DSL (`renumber_field` only does a uniform decrement), so byte-gating
  SwtControl would require weakening the DSL — port-authored is the honest call.
  Left as an accepted residual per brief_schema item 3. Follow-up (non-blocking):
  an independent r4133 `DSS_ExtractSchema` oracle (via the opt-in EPRI channel)
  could byte-gate these four; not built (that channel never gates commits).
- **`REGEN_SCHEMA_PORT` env-guard no-op (accepted).** The port-golden test
  rewrites-and-returns when the var is set — identical to the established
  `DSS_REGEN_AD_GOLDEN` pattern in `adiakoptics.rs`, a repo-wide convention, not a
  new weakening; both oracle goldens remain pin-checked. No change.

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
  WASM_USERMODELS — Generator (WM.3), Storage + PVSystem (WM.4) DONE;
  CapControl (WM.5) remaining.** Infrastructure (WM.0–WM.2) + Generator
  `UserModel`/`ShaftModel` (WM.3) + Storage `UserModel`/`DynaDLL` and PVSystem
  `UserModel` (WM.4) all follow the plan §2.4 uniform rule (WASM `.wasm` loads;
  a native-DLL name warns + falls back), gated bit-exact vs the r4133 oracle
  (`indmach012a.wasm` + `wm4model.wasm` fixtures, twin-pinned). Only CapControl
  `UserModel` still carries `PropFlags::NOT_PORTED` → WM.5.

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

## 1k. UNIFIED_GATE pre-E/F cross-phase audit (branch `ug-audit`, 2026-07-19)

Two independent xhigh read-only audits (code-fidelity + verification-strength) of
the WHOLE delivered range `pre-unified-gate` (`449c745`) .. post-Phase-D `update`
(`a0ac274`), before Phases E/F run. **Per-phase verdicts: 0 PASS, A PASS, B PASS,
C PASS, D PASS-at-settled-bar** — the delivered gate verifies MORE than the
pre-range gate (whole-range `tests/harness` diff = one `fn`→`pub fn` visibility
change; no tolerance/floor/assert weakened; lock membership 514/514 zero loss;
corpus pristine). Both audits re-derived the shipped ledger from the live engines
(r4133 renders `Storage.%stored` to 6 sig figs vs full-f64 pinned oracle, inside
the `num_rel 1e-6` envelope; the #303 line-spacing crash reproduces live) — no
entry papers over a fixable port bug. 16 findings (2 medium, rest low), settled
here empirically; every fix is rigor-ADDING, no tolerance/envelope/assert loosened,
no golden touched.

**Fixed (this branch, canary-proven live):**
- **UGA-1/T1 (medium) — envelope widening escaped the population lock**: the
  lock's ledger tag was entry-id-only, so raising `max_rel`/`num_rel`, adding a
  scope, or cutting `steps` on an EXISTING entry produced no lock diff —
  contradicting plan §1.4/§5-R3 and the STATUS §1i claim. `ledger_tags()` now
  fingerprints each entry as `id@FNV-1a64(full entry JSON)`; lock regenerated
  (diff = exactly the 6 ledgered cases gaining `@digest`). Canary: widening
  `r4133-gfm-micro-pctstored` `num_rel` 1e-6→1e-2 now FAILS `population_lock`
  (proven, reverted).
- **UGA-T2 (medium) — `property` ledger scope half-enforced §1.3**: no mandatory
  `oracle` pin (a bare `name_re`-only scope masked with zero assertion), `rust`
  pin ignored, `num_rel` ignored. `property_handled_keys` now mirrors the probe
  path exactly (Phase-D F4/F5 parity): numeric-skeleton `num_rel` envelope
  against the live Rust `?`-value + tier-floor staleness, non-numeric requires
  the `oracle` pin + honors the `rust` pin; structurally a probe/property scope
  now REQUIRES `oracle` or `num_rel`. Canary-proven on `vsource_asym`
  (`Vsource.source.basekv`): wrong pin fails loudly ("oracle 12.47 != pinned
  9999"), `num_rel` entry applies with 1 hit. (Unblocks the ORPHANED_GAPS §1.9
  `line_spacing_asym` exact-pair-numeric retirement.)
- **UGA-2 — latent skip full-bypass**: no rule prevented a case's ONLY gating
  channel(s) from all being `skip`-ledgered (→ zero verification, not even the
  Rust smoke). `assert_structural` now requires ≥1 non-skipped channel per
  skip-bearing case (canary: a capi skip added to the r4133-skipped
  `IEEE13_LineSpacing` fails structurally). All 3 shipped skips sit on
  `engines:"both"` cases — latent only.
- **UGA-T4 — no scope-field allowlist**: a typo'd or §1.3-but-unimplemented
  field (`yprim`/`y_fingerprint`/`meter`/`global_result`) compiled fine and
  silently never applied (masked inside a multi-scope entry by per-entry hit
  accounting). `compile_scope` now rejects anything outside the 9 implemented
  fields, loudly (canary-proven).
- **UGA-T3 — `line_re` masks could never go stale on content-matched lines**:
  `mask_line` marked exceeded unconditionally on regex match. It now records a
  live divergence only when the trailing-whitespace artifact is actually present
  (`line != line.trim_end()`), so a vanished artifact trips STALE. (Mask power
  was always bounded to `trim_end`; 0 such entries ship.)
- **UGA-T5 — `DSS_GATE_ONLY` matching nothing greened a 0/0 run** (and skipped
  fail-on-stale): a leftover exported env var could silently neuter the gate.
  The scheduler now panics on zero retained cases (canary-proven).
  (`DSS_GATE_SEED_LEDGER` remains a deliberate, loudly-bannered report mode.)
- **UGA-3/T8 — stale in-code docs** contradicting delivered mechanics:
  `corpus_gate.rs`/`scheduler.rs` "target-rev cases keep the one-shot Oracle
  path" (retired in Phase C), `engines.rs` EpriPool "recycle after 64 cases"
  (default 1 since Phase D), `manifest.rs` "valid `oracle` field" (deleted in
  Phase C). Corrected.
- **UGA-4 — dangling follow-up ownership**: 5 manifest `wp` pointers still named
  `WP-UG-D` (a COMPLETE phase that deliberately did not retire them). The 4
  ledgerable-but-unretired defer_ledger cases (DynExp×2, `regcontrol_idle`,
  `line_spacing_asym`) now point at **ORPHANED_GAPS §1.9** (new entry, full
  retirement recipe); the solvable_now NCIM Xmission case now points at
  **WP-U1.7** like its 3 NCIM siblings. (`wp` is not in the rigor fingerprint —
  no lock impact.)
- **UGA-T6 — ubuntu CI leg structurally red post-Phase-C/D**: the mandatory gate
  spawns `epri-worker` (`#[cfg(windows)]`, exits 1 on Linux) for 439
  r4133/both-gated cases. `ci.yml` matrix reduced to `windows-latest` with the
  reason documented in place.

**Deliberately NOT fixed (recorded with rationale):**
- **UGA-5 — plan §2.2 all-properties enumeration sub-item not implemented** in
  the bridge ("implement anyway for report tooling parity"): `capture.rs`
  fail-louds on an `all_properties` request (verified — no fake-empty dump) and
  the scheduler masks it off per-channel; gating is unaffected (property parity
  is capi_v0145-only by plan). Recorded as an accepted §2.2 deviation: implement
  only if report tooling ever needs it (`DSSPut_Command("? name.Like")` +
  `DSSElementV`).
- **UGA-6 — "Rust runs once per case" (§3.3/D7) violated for `engines:"both"`**:
  `compare_with_result` re-runs the deterministic Rust engine per channel (2×).
  Cost-only (~150 s full gate ≪ 10 min target), disclosed in the scheduler
  comment; caching the capture across channels is not worth the seam. Accepted.
- **UGA-7 — Phase D "≥90% both" target missed (342/514 = 66.5%)**: already
  disclosed in §1i with the seeding-data proof that ~72% is the arithmetic
  ceiling (97 r4133-only-feature cases can never be both; 75 capi-only
  wholesale-divergent stay single-channel per plan). The fingerprintable ~21-case
  flip set remains the follow-up. No action here.
- **UGA-T7 — `skip` entries can never go stale by construction**: a crash cannot
  be observed without sending the deck, and no channel would notice the crash
  disappearing (unlike the old report mode). Inherent to the design; the audit
  re-reproduced #303 live on 2026-07-19. Re-validation is manual:
  `DSS_GATE_SEED_LEDGER=1 DSS_GATE_SEED_ONLY=<case>` sends the deck to r4133 and
  reports. Accepted with this documented procedure.
- **UGA-T9 — P5a `show_busflow_unknown_bus_errors` assert reshape** (exact
  message equality → code 219 + `contains("not found")`): P5a is trusted context
  with its own audit trail; the assert gained the numeric-code dimension and the
  reshape is disclosed in the P5a record. The only text-weaker assert in the
  whole range — noted for completeness, no action.

**Phase E handoff (confirmed, NOT fixed here per brief):** `tools/opendss/*.py`
report scripts still read the deleted `known_diffs.json` / reference
`corpus_live_opendss` — they die in Phase E with the Oddie venv + `bin/r3723` +
`bin/r4088` (junction-safe protocol, CLAUDE.md). Phase F additionally rewrites
TESTING.md/CLAUDE.md (the in-code module docs were already fixed here).

**Gate:** `cargo fmt --all --check` + `cargo clippy --workspace --all-targets
-- -D warnings` + `cargo test --workspace` green at defaults (full BOTH corpus
gate 514/514, all 6 ledger entries hit, fail-on-stale active); `tests/corpus`
pristine; junctions intact.

## 1l. UNIFIED_GATE post-audit fix round (branch `ug-fixround`, 2026-07-19)

Closes the three actionable "deliberately not fixed" items from the §1k pre-E/F
audit triage. Every envelope MEASURED live on both engines (full 510-case ×2 seed +
`DSS_LEDGER_MEASURE` gate runs); R3 discipline throughout — no ledger entry papers
over a fixable port bug.

**Item 1 — fingerprintable follow-up flip set → `engines:"both"`.** Seeded all live
cases ×2 channels, triaged each single-channel case by first-divergence class.
**18 flipped** (both% 342→360 = 66.5%→**70.0%**, near the seeding-proven ~72%
ceiling); 4 measured NOT tightly fingerprintable stay `capi_v0145` (`note` cause):
- **storage/PVSystem 6-sig-fig display** ×12 — r4133 Delphi renders
  kw/kwhstored/%stored/kvar/kwtarget + PVSystem kvar to 6 sig figs, port+pinned-0.14.5
  full f64. `probe` `num_rel` **1e-5** (= 2× the 6-sig-fig class ceiling 5e-6; corpus
  max 4.9e-6 @ peakshave). storagectrl_{peakshave,support,ipeakshave,loadshape,
  chargelow}, midi_storagectrl, invcontrol_storage_vw, expcontrol_basic +
  modes time/{duty,generaltime,generaltime_duty,generaltime_yearly}.
- **injection+element fpc-delphi-ulp** ×4 (IndMach asymmetric) — `injection`
  whole-vector (max_abs 1e-5; seen 4e-6) + `element` currents/powers/losses
  (max_abs 2e-5 incl. a Line loss near-cancellation 9e-6; max_rel 1e-8);
  voltages/yprim/discrete tier-clean. combo/{combo_mesh,midi}_asym,
  indmach/{indmach,midi_indmach}_asym.
- **monitor seq-magnitude drift** ×1 (monitor_seqmag) — 3 `monitor` channel scopes:
  mseq ch4 (V2 mag, max_abs 5e-6, seen 2.4e-6), mseq ch5 (V2 angle, max_abs 1e-3 =
  angular image of the mag floor, seen 5.95e-4), mseqmag ch2 (|V|3, 5e-6). NB the
  ledger `channel_idx` is 0-based (the harness "channel N" display is 1-based).
- **stayed capi_v0145** (measured, not fingerprintable): storagectrl_{time,follow},
  invcontrol_storage_vv_vw, invcontrol_expmodel — r4133 event-log CONTENT differences
  (parallel-build `StorageController1` actor-suffix; `DER`-vs-`PVSYSTEM/STORAGE OUTPUT`
  wording) the trailing-whitespace-only eventlog mask (UGA-T3) cannot normalize.

**Item 2 — defer_ledger retirement (ORPHANED_GAPS §1.9).** 1 of 4 retired; 3
confirmed NOT ledgerable per R3, reasons sharpened. defer_ledger remaining: **7**
(regcontrol_idle + DynExp×2 + NCIM×4 [WP-U1.7]).
- **line_spacing_asym RETIRED** → `engines:"both"`: capi_v0145 exact-pair-NUMERIC
  property+probe scopes (Line.lsp.normamps 730→230, emergamps 1095→345 — the
  min-over-phase D3 upgrade, port CORRECT per r4133 LineGeometry.pas) + r4133 `#303`
  skip. Needed the exact-pair-numeric machinery §1i said was lacking — ADDED to
  `probe_handled`/`property_handled` (num_rel absent + oracle/rust pins ⇒ exact float
  pin, not an envelope). Line.lspc D3-invariant (165/247.5, no divergence).
- **regcontrol_idle STAYS** defer_ledger: the earlier "~7e-5 sub-tap" note was WRONG —
  the live r4133 idle regulator settles MV.1 **~8.7% (623 V)** off the port, WHOLESALE.
  Per R3 an 8.7% gap at the regulated bus smells like a port idle-RegControl bug
  (tap init), not an upstream ulp difference → NOT ledgered; needs a WP.
- **DynExp×2 STAY** defer_ledger (Dynamic_KundurDynExp, GFL_IEEE123 DynExp):
  re-measured — port matches NEITHER oracle (0.14.5 AND r4133 agree with each other,
  port ~1.5e-5 off BOTH). Force-ledgering would pin a port-side DynExp-evaluator bug.

**Item 3 — plan §2.2 all-properties enumeration in dss-epri** (audit UGA-5, dropped
without deviation). Implemented: `DSSElementV` FFI binding (mode 0 = AllPropertyNames)
+ `capture_all_properties` (byte-faithful port of
`oracle_server.capture_all_properties`: `? name.Like` activate → property list →
`? name.prop` values; read LAST per step). The run_case fail-loud is gone. Smoke
`#[test]` proves the round-trip (IEEE13: 38 elements, 1628 property values).
**Gating semantics UNCHANGED**: the scheduler still masks `all_properties` off on the
r4133 request (property parity stays capi_v0145-only per plan) — the r4133 all-props
path is capability-only (report tooling). Unsafe stays inside dss-epri (SAFETY comment;
V-protocol copied immediately).

**Ledger measurement aid.** `DSS_LEDGER_MEASURE=1` makes the numeric handlers print
the live divergence per scope (env-gated stderr, NO gating-semantics change) so
envelopes are sized to the measured max — the "every envelope measured live" (R3)
operation, repeatable.

**Ledger:** 25 entries (was 6): +12 storage/pv display, +4 injection-ulp, +1 monitor,
+1 line_spacing exact-pair (capi) + 1 skip (r4133). Structural test green; every entry
HIT (1041 total hits) and non-stale in the full gate.

**Gate (three-command, defaults, green):** fmt clean; clippy clean; `cargo test
--workspace` green — corpus gate 514/514, 25 ledger entries all hit, fail-on-stale
active, population lock regenerated (diff = 18 engine flips + 18 ledger tags), dss-epri
smoke green (all_properties round-trip). `tests/corpus` pristine; junctions intact.

**Post-audit settlement (2026-07-19).** Two independent audits (audit-code, audit-tests)
of the fix round returned FOUR low-severity findings; each settled empirically (live
engines, code path, gating topology), no tolerance/envelope loosened:
- **exact-pair-numeric drift-safety (audit-code F1 / audit-tests F2) — FIXED.** The
  exact-pair-NUMERIC branch only marked stale on `rust==oracle`, so a port regression to
  a THIRD value (still `!= oracle`) with no `rust` pin would pass silently. Added a
  MANDATORY `assert!(sc.rust.is_some(), …)` to both `probe_handled` and
  `property_handled` (mirrors the non-numeric path's mandatory `oracle` guard).
  Strengthening only; the sole live exact-pair-numeric entry (`capi-linespacing-normamps`)
  pins `rust`=230/345, so the gate stays green. Closes the last soft spot in the new
  machinery.
- **all_properties smoke wording (audit-tests F1) — FIXED (wording).** The smoke check
  re-reads the same `? name.prop` getter that built the dump (proves enumeration
  non-empty + getter determinism, NOT value correctness vs an independent baseline).
  Reworded "round-trip verified" → "dump non-empty + getter re-read consistent" and
  expanded the comment: capability-only report tooling per §2.2; the pinned capi oracle,
  not this smoke, gates property correctness.
- **storage-display 1e-5 vs gfm 1e-6 for %stored (audit-tests F3) — NO CHANGE (no
  defect), rationale recorded.** Not an arbitrary looseness: storagecontroller cases are
  `engines=both`, so the **capi channel strict-gates `Storage.%stored` at full f64** via
  `compare_all_properties` (the r4133 storage-display scopes are `probe`, which never
  remove %stored from the capi all-properties compare) — a real port %stored drift is
  caught there. The r4133 `num_rel`=1e-5 only absorbs Delphi's 6-sig-fig display across
  the whole Storage class (incl. large-mantissa kw/kwhstored, ceiling 5e-6). gfm cases
  are `engines=r4133`-only (0.14.5's Isc1 default shifts system-Y ×1000 → capi cannot
  gate), so their 1e-6 is the SOLE %stored check and must be tight. Different gating
  topology, no masking.
- **line_spacing property scopes "inert" (audit-tests F4) — NO CHANGE (not a defect),
  scopes retained.** The finding reads the static lock `props=0`, but `force_properties`
  sets `compare_all_properties=true` at RUNTIME for asymmetric-family Live capi cases
  (`ASYMMETRIC.compare_all_properties=true`; `line_spacing_asym` `engines=both` →
  `gates_capi`). So on the capi channel the deck DOES capture all_properties and both
  `property` scopes (Line.lsp.normamps/emergamps) run and are pinned — confirmed by the
  green gate now that the mandatory-`rust` guard above is active on them.
- Gate re-run recovered from a transient Windows incremental-compilation linker flake
  (`LNK2019` anon.llvm/serde_json symbols, unrelated to the edits) by clearing
  `target/debug/incremental` and rebuilding with `CARGO_INCREMENTAL=0` — green.

## 1m. UNIFIED_GATE Phase E — retire the Python EPRI stack + r3723/r4088 (branch `ug-phase-e`)

`UNIFIED_GATE_PLAN.md` §4-E / §6-E executed: the retired opt-in EPRI-python
channel (Oddie bridge, dss-python 0.16.0b2/backend wheels, r3723/r4088 binaries)
is gone. The unified gate's two live oracles are unchanged — `capi_v0145`
(pinned dss-python via `tools/oracle/oracle_server.py`) and `r4133` (in-house
`crates/dss-epri` bridge over the git-tracked `bin/r4133` DLL).

**Deleted (git rm, 27 files):** `tools/opendss/` scripts
`ab_compare.py`, `smoke.py`, `dsspy_crosscheck.py`, `gen_ad_reference.py`,
`probe_59n.py`, `sweep_modes_isolated.py`, `sweep_merge.py`, `xcheck_bridge.py`;
`dsspy_validation/` (5 files); `wheels/` (2 beta wheels + SHA256SUMS);
`PIN_OPENDSS.txt`; `bin/r3723/**` (5) + `bin/r4088/**` (5).

**Pruned to r4133-only:** `revisions.json`, `bin/SHA256SUMS` (verified
`sha256sum -c` OK), `bin/README.md`, `vendor_binaries.py` (REVISIONS dict +
README template), `README.md` (rewritten: r4133 artifact + Rust `epri-worker`
bridge + re-vendor procedure). `oracle_server.py` pruned to the `capi` engine
only — removed the `capi015`/`oddie` `make_engine` arms, `_oddie_get_y_sparse`,
`_read_pin_opendss`, `OPENDSS_DIR`/`REPO_ROOT`, the Oddie `capture_eventlog`
export-CSV branch, and the `clear` cmd (only the deleted `xcheck_bridge.py` used
it); `DSS_ORACLE_ENGINE` now accepts only `capi` (anything else exits non-zero,
no silent pass). Stale comments referencing the deleted harness fixed in
`corpus_guard.py` and `epri-worker.rs`.

**rg sweep verdict** (`rg -i "oddie|capi015|r3723|r4088|dss_python_backend|0.16.0b2"`):
no LIVE code/config wiring to the retired channel remains — no manifest carries an
`oracle:"r3723|r4088|capi015"` field (all gating is via `engines`, values
`both`/`capi_v0145`/`r4133`); no executable reference (import/spawn/config) to any
deleted script survives; CI/`Cargo.toml`/`.gitignore` clean. All remaining matches
are legitimate-survivor classes: (a) historical docs (STATUS, `docs/plans-archive`,
`docs/upgrade/*`, `docs/phase-records/*`, `docs/wasm/*`) and root plan files
(`*_PLAN.md`, `PLAN_SEQUENCE.md`, `README.md`); (b) `CLAUDE.md`/`TESTING.md` (Phase
F rewrites these); (c) behavioral-spec / oracle-provenance citations in
`crates/dss-core/src/**` and test docs (which oracle calibrated a value —
`capi015`/`oddie:r4133`/`r4088`, analogous to STATUS records, not live wiring);
(d) golden generators `tools/golden/gen_*.py` (UNTOUCHED per the gate rules; frozen
manual tooling. NB — corrected at Phase F settle per re-review finding F2: the
original "gen_protection.py … still valid" wording here was overstated. The r4133
`ODDIE_SCENARIOS` arm of `gen_protection.py` and ALL of `gen_flicker.py` need
`from dss import IOddieDSS`, absent in pinned 0.15.7, and their environment —
Oddie venv, 0.16.0b2 wheels, `PIN_OPENDSS.txt`, r3723 binaries — was deleted in
this phase: they are frozen dead paths; regen would need a git-history restore.
Goldens are frozen so the gate is unaffected. See TESTING.md §Frozen historical
generator arms);
(e) frozen corpus fixtures — `.dss` deck comments + manifest `note`/`ledger` cause
provenance; (f) committed AD trusted-baseline data `tests/data/adiakoptics/r3723_ref/`
+ its PROVENANCE.txt; (g) WASM/FPC ABI lineage (`tools/fpc/usermodel_abi`,
`docs/wasm`, `WASM_USERMODELS_PLAN.md`) referencing the vendored SOURCE tree
`electricdss-code-r3723-trunk`, not the retired binary channel; (h) retirement-guard
assertions in `corpus_gate/engines.rs` that assert the pinned ping never reports an
`oddie`/`capi015` marker (they enforce the retirement and stay green).

**venv:** `tools/opendss/.venv` in main is already empty (2026-07-19 incident);
the coordinator removes that empty dir in MAIN separately (junction-safe, CLAUDE.md).
This worktree's `.venv` is a junction — not touched.

**Settlement (two audits, `ae3e6bd..c060c0b`):** five low-severity findings, all
non-gating; three fixed, two recorded non-fixes:
- *Fixed* — `tools/opendss/README.md`: added the plan-required (§1.1) note that
  the frozen A-Diakoptics baseline (`tests/data/adiakoptics/r3723_ref/`, consumed
  by `ad_reference.rs`) has no regen tool anymore and must be reimplemented over
  `epri-worker` if ever re-run (harvester `gen_ad_reference.py` was deleted).
- *Fixed* — `tools/oracle/README.md`: this KEPT live doc still described the
  retired `DSS_ORACLE_ENGINE=oddie` rebind, the "EPRI/Oddie channel", and the
  renamed `corpus_live.rs`/`corpus_live_opendss`; rewritten to match the pruned
  `oracle_server.py` (capi-only, exits non-zero on any other engine).
- *Fixed* — Gate paragraph below now carries exact counts/exit-codes/wall-clock.
- *Non-fix (recorded)* — `tools/golden/gen_checkpoints.py`'s `capi015` regen arm
  (`_read_pin_opendss`) still references the deleted `tools/opendss/PIN_OPENDSS.txt`.
  Left UNTOUCHED per the binding gate rule (golden generators `tools/golden/gen_*.py`
  are frozen): it is a **dead path** — reachable only via `DSS_ORACLE_ENGINE=capi015`,
  whose engine (dss-python 0.16.0b2) was retired here, and the four `"oracle":"capi015"`
  goldens are frozen. The live `capi` gate path never touches it. No gate impact.
- *Non-fix (deferred)* — `CLAUDE.md` + `TESTING.md` still describe the retired
  opt-in channel as live. Plan §4-F explicitly defers rewriting both to **Phase F**
  (survivor class (b) below); a forward-deferral to verify Phase F completes, not a
  Phase E defect.

**Gate** (defaults, worktree `wtE` @ settlement, both oracle channels live):
- `cargo fmt --all --check` → exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` → exit 0.
- `cargo test --workspace` → exit 0: 1893 passed, 0 failed, 2 ignored across
  58 test binaries; wall-clock ~172 s.
`tests/corpus` pristine (`git status tests/corpus` clean); junctions intact.

## 1n. UNIFIED_GATE Phase F — docs + final acceptance (branch `ug-phase-f`)

`UNIFIED_GATE_PLAN.md` §4-F Scope A (docs) + §6 final acceptance (clean-clone
gate) executed. The fix-round re-review (Phase F brief Scope B) ran as an
independent parallel audit — its findings settle in its own record, not here.
Base `20d03f7`.

**Docs rewritten — every claim re-verified against the live code, none copied
from stale prose:**

- **TESTING.md** (full rewrite): layer map = unit / golden / **unified corpus
  gate** / corpus hygiene (the opt-in EPRI row is gone); new sections for the
  gate architecture (one `#[test]`, 514 cases = 293+47+105+69, `engines`
  both=360 / r4133=96 / capi_v0145=58, `defer_ledger`×7, `isolate`×33, module
  tree, per-case worker recycle default 1) and the divergence ledger (3 kinds,
  the a/b/c envelope clauses, exact-pair pins incl. the mandatory `rust` pin
  on exact-pair-numeric, the 9 implemented scope fields, fail-on-stale +
  never-applied + hit accounting, lock `id@digest` tags, manual `skip`
  re-validation); procedures rewritten/added: add-a-corpus-case (`engines`
  discipline), **triage-a-divergence-into-the-ledger** (R3 rules: prove
  not-a-port-bug first, measure with the seed report / `DSS_LEDGER_MEASURE`,
  commit ledger + lock together), **re-vendor the r4133 binary**, **run the
  seeding report**; env-var table re-verified knob-by-knob against
  `corpus_gate/{scheduler,engines,ledger}.rs` + `oracle_server.py` (NB: no
  `DSS_GATE_POOL` exists — pool size is derived `max(2, jobs/2)`, documented
  as such); frozen historical generator arms documented (`gen_checkpoints.py`
  capi015 arm referencing the deleted `PIN_OPENDSS.txt` — the Phase E recorded
  non-fix, now documented instead of edited — plus `gen_bh_capi015`/
  `gen_regcontrol_capi015`/`gen_fuse_r4133`); golden-family table refreshed
  (`json_import/`, `gen_schema.py` rows added).
- **CLAUDE.md**: project identity reframed — the 1:1 port is the FINISHED
  stage (final acceptance 2026-07-11); direction = pure idiomatic Rust / wasm
  user models / new methods & models / r4133-and-beyond, with PORTING_PLAN.md
  as the historical record; the invariant list updated: the `dss-epri` unsafe
  carve-out is the sole `forbid(unsafe_code)` exception (matches
  PORTING_PLAN.md §scope note + `dss-epri/src/lib.rs`); the stale
  opt-in-Oddie bullet replaced by the two-channel unified gate + ledger; the
  gate section now describes `corpus_gate.rs`
  (`corpus_gate_all_cases_match_engines`, both oracles mandatory,
  Windows-only r4133 bridge, ledger fail-on-stale); the TODO(compat) wipe
  pointer retargeted to DE_PASCALIZE Stage F. Ritual step-0, the (freshly
  hardened) worktree junction protocol, conventions, and the MCP section
  untouched.
- **tools/opendss/README.md** (Phase E rewrite reviewed; gaps fixed): the
  "editor/registry suppression inside the bridge" claim was FALSE against the
  code (`rg RegistryUpdate|rundll32` over `dss-epri` = no match; that pair
  belonged to the retired Oddie `make_engine` and survives only in the frozen
  `gen_protection.py`) — replaced with the real mechanism (`DSSI(8,0)` ⇒
  `NoFormsAllowed`, `dss.rs`); added the never-`FreeLibrary` rule (the Phase A
  deadlock root-cause) and a "Gate wiring" section (worker resolution order,
  ping markers, ledger pointer, all-properties = capability-only).
- **Surgical de-staling of other live docs still naming retired machinery**
  (small justified scope addition): root `README.md` (deleted
  `known_diffs.json` → gating ledger; `corpus_live.rs` → `corpus_gate.rs`;
  opt-in-channel paragraph → two-oracle gate; forbid-scope wording),
  `tests/TOLERANCE_NOTES.md` (the plan-§7 "**ledger is not a tolerance**"
  paragraph added; `corpus_live`→`corpus_gate` renames; the Oddie-eventlog
  paragraph rewritten to the `dss-epri` CSV/BOM capture; EVENTLOG_MASKS keying
  updated to channels), `tests/corpus/README.md` + `COVERAGE.md`,
  `tools/oracle/{oracle_server,corpus_guard}.py` comment renames. Golden
  generators, goldens, tolerances, ledger entries, manifests: UNTOUCHED.

**Wall-clock before/after (plan §3.4 / §6 — consolidated from the phase
records):**

| point | mode | corpus-gate share | full `cargo test` |
|---|---|---|---|
| Phase 0 baseline (`pre-unified-gate` 449c745, loaded box) | serial one-shot, 1 channel | 292.6 s | 426.6 s |
| Phase B settle (capi_v0145 only; incl. dump write; serial ref 341.9 s) | persistent parallel, jobs 16 / pool 8 | 104.3 s | — |
| Phase C (r4133 channel live, 514 single-channel cases) | persistent parallel, jobs 16 / pool 8 | 64.5 s (settle build 67.4 s) | — |
| Phase D (full BOTH gate + ledger) | persistent parallel, recycle=1 | ~150 s (137–160) | — |
| Phase E settlement (warm build) | defaults | — | ~172 s (1893 pass / 2 ignored / 58 binaries) |
| Phase F worktree `wtF` (cold build; warm re-run 162 s) | defaults | — | 365 s (fmt 2 s, clippy 51 s) |
| Phase F clean clone (cold build, the §6 acceptance run) | defaults | — | 303 s (fmt 5 s, clippy 50 s) |

Net: from a 292.6 s serial single-channel corpus pass to a ~150 s
**two-channel** (BOTH-gated, ledger-checked) pass — roughly double the oracle
coverage at half the wall-clock, ≪ the plan's ≤10 min target.

**Clean-clone gate (plan §6 final acceptance) — GREEN.** `git clone --branch
ug-phase-f --single-branch e:/RustProject/dss-rs <Temp>\claude\dss-clean` at
head `cacb388`; clean checkout, **no `.inputs`, no venv** — the git-tracked
r4133 DLL + the system-python pinned oracle (verified `dss-python 0.15.7`) are
the only external deps, which IS the plan's proof. Three-command gate at
defaults: fmt exit 0 (5 s) / clippy `-D warnings` exit 0 (50 s) / `cargo
+stable test --workspace` exit 0 (303 s, both oracle channels live). The only
residue was the known §1g CorpusGuard export-CWD corner (untracked
StorageControllerTechNote CSVs, throwaway clone). NB a first attempt cloned
under the deep per-session scratchpad and **failed at checkout on Windows
MAX_PATH** (121-char root + the 166-char longest corpus path = 287 > 260;
`core.longpaths` is unset by default, and the Delphi DLL's file I/O is not
long-path-aware anyway) — a clean clone must sit at a short root, as a normal
user clone does.

**Tooling note:** the TortoiseSVN CLI tools were installed machine-globally on
2026-07-19 for the `.inputs` re-vendor (svn peg-revision checkouts of the EPRI
SVN source trees after the second junction-wipe incident).

**Gate (worktree `wtF`, defaults, both channels live):** fmt exit 0 (2 s);
clippy `-D warnings` exit 0 (51 s); `cargo +stable test --workspace` exit 0
(365 s, cold build). `tests/corpus` pristine after runs (path-limited clean of
the known §1g export-CWD leftovers); junctions intact. The `unified-gate-v1`
tag is created at settle on the final integrated head (deliberately not in
this record).

### Phase F settle — three audits + the Scope B re-review (2026-07-19)

Three independent reviews returned: **audit-code** (docs, 4 findings),
**audit-tests** (acceptance, 4 findings), and the **user-mandated fable
re-review of the opus fix round** (Scope B — the "independent parallel audit"
the record above pointed at; it materialized, verdict below). Both doc audits
independently re-verified essentially every TESTING.md/CLAUDE.md claim against
the live code and found the rewrites honest; audit-tests additionally
corroborated the clean-clone acceptance in a second fresh clone (fmt/clippy
exit 0 reproduced; its `cargo test` was still running clean at report cutoff —
the phase's own clean-clone run above is the §6 acceptance evidence).

**Scope B re-review verdict: PASS, nothing gating.** 11 of ~19 new ledger
envelopes plus the gfm class re-derived from the LIVE engines with an
independent protocol driver: every envelope is a measured upstream divergence
(Delphi 6-sig-fig display rendering, whole-model fpc-vs-delphi transcendental
ulp, seq-transform drift, the discrete normamps min-over-phase jump, hard #303
crashes); measured provenance values reproduce digit-for-digit (4.900e-06,
1.313e-06, 4.05e-06, 5.95e-04, 1.62e-06 …); the storage-display 1e-5 vs gfm
1e-6 split is empirically justified (mantissa-class ceilings 4.95e-6 vs
8.8e-7; the capi channel strict-gates the same probes at full f64); the two
R3-critical non-retirements are correct — regcontrol_idle is a genuine ~8.8 %
/ 620 V wholesale divergence (r4133 taps to 15 despite `idle=yes`; the port
holds tap 1.0 = the retired capi015 semantics — the follow-up WP should check
the dss_capi-0.15-vs-EPRI idle delta before assuming a port tap-init bug), and
Kundur DynExp is matches-neither (the two oracles agree to 4.8e-10 while the
port is 1.52e-5 off BOTH — ledgering it would have hidden a potential port-side
DynExp-evaluator bug). Eventlog stay-capi divergences verified real on live
r4133 (actor-suffix `StorageController1.`, STORAGE/DER wording). All-props
confirmed capability-only (scheduler masks it off every r4133 request; FFI
copies immediately, SAFETY-documented). Informational: 13 of 20 ledger causes
are channel-narrowing documentation with no current entry (Phase D heritage).

**Finding dispositions (all settled empirically at settle, in this commit):**

- *PF-1 / F-PHF-1 (Scope D silently dropped) — CONFIRMED, fixed.* The phase
  agent skipped brief Scope D; all four items are now done: **F1**
  `tools/corpus/README.md` no longer claims `dsspy_crosscheck.py` "moved"
  (deleted, Phase E); **F2** TESTING.md's frozen-arms list now includes
  `gen_flicker.py` (entirely Oddie/r3723) and `gen_protection.py`'s
  `ODDIE_SCENARIOS` arm, the regen procedure scopes itself to capi arms, and
  the §1m survivor-class (d) overstatement is corrected in place; **F3** the
  `probe_59n.py` reproducibility regression is recorded (TESTING.md §Retired
  probe scripts — the relay/tests.rs + skipped-manifest citations are
  historical; manifest/test text untouched per the brief); **F4** the five
  remaining stale `xcheck_bridge.py` present-tense comments in `dss-epri`
  (epri-worker.rs, lib.rs, capture.rs, dss.rs ×2) now say the cross-check was
  retired with its stack.
- *PF-2 — CONFIRMED, fixed* (the F2 items above).
- *PF-3 / F-PHF-2 (Scope B existence) — resolved:* the re-review ran and its
  record is this section.
- *PF-4 (env table incomplete) — CONFIRMED, fixed:* `DSS_EPRI_ACTOR_TIMEOUT_SECS`
  (default 300, `dss.rs::wait_for_actor`), `DSS_EPRI_DLL`, `DSS_EPRI_EXPECT`
  (smoke overrides) added to the TESTING.md table.
- *F-PHF-3 (tag) — done at settle:* annotated `unified-gate-v1` created on the
  final head as the last act (plan §6).
- *F-PHF-4 — noted:* the audit's corroboration clone was cleaned up; its gate
  was green through fmt/clippy and mid-`cargo test` (zero failures observed)
  at cutoff.
- *RR-1 (non-numeric exact-pair oracle-only pin hole) — CONFIRMED, fixed
  (tightening, latent — no live entry exercises the path):* the `rust` pin is
  now MANDATORY in the non-numeric probe/property arms of
  `corpus_gate/ledger.rs` (mirroring the fix-round F1/F2 numeric fix), and
  `assert_structural` now statically rejects any exact-pair scope (no
  `num_rel`) without a `rust` pin — the drift hole is closed in both arms and
  at load time.
- *RR-2 (skip provenance text) — CONFIRMED, fixed:* the
  `r4133-linespacing-asym-303` ledger `source` now records the live-re-derived
  crash site (compile of the `tscables=[…]` line, offset 41CE5E — not calcv,
  which is the IEEE13_LineSpacing sibling). Free-text-only change; the skip
  itself was re-derived correct. `population.lock.json` regenerated via the
  sanctioned `DSS_UPDATE_POPULATION_LOCK=1` path (the entry digest covers the
  full serialized entry by design); diff verified to touch only that case's
  ledger tag.

Settle gate + the `unified-gate-v1` tag: recorded in the commit that carries
this section (gate results in the final report).

## 1j. DE_PASCALIZE P5a — miette diagnostics: the type + both channels (branch `wt-p5a-v2`)

`DE_PASCALIZE_PLAN.md` §P5a executed (P5b spans / P5c CLI presentation out of
scope). One `miette`-based diagnostic type now backs every engine error channel.

- **`crates/dss-core/src/diag.rs`** (new): `DssDiagnostic { message, code:
  Option<u32>, abort, span, src, help }` with a hand-written `miette::Diagnostic`
  impl (`code()` → `dss::eNNN`, `severity()` flips on `abort`) + unit tests, per
  the plan sketch. `miette = { version = "7", default-features = false }` (no
  `fancy`) in the workspace + dss-core. A small `ErrorLog(Vec<DssDiagnostic>)`
  newtype with `push(impl Into<DssDiagnostic>)` + `texts()` reduces churn: bare
  `String`/`&str` pushes stay valid (→ `code: None`), numbered sites push
  `DssDiagnostic::msg(text, Some(NNN))`. `DssDiagnostic: Deref<str>` so the
  ubiquitous `errors().iter().any(|e| e.contains(..))` presence checks keep
  working (text is a display convenience, not the error's identity — the code is).
- **Central log** flipped: `Dss.errors: ErrorLog`; `Dss::errors() ->
  &[DssDiagnostic]` + `Dss::error_texts() -> Vec<String>`. **Deferred channel**
  (`obj/base/mod.rs`) → `Vec<DssDiagnostic>` keeping the separate `deferred_abort`
  bool so the `exec/command.rs` drain order is byte-for-byte unchanged.
- **Control-loop trait channels — policy = RETYPE (not wrap-at-sink).** The four
  `fn push_error(&mut self, msg: String)` points (2 trait decls in
  `inv_control`/`storage_controller`, their impls in `solution/controls/dispatch.rs`
  + the two test envs) were retyped to `fn push_error(&mut self, diag:
  DssDiagnostic)`. Reason: their own doc-comments name "the 14403 named-missing
  error" — these sinks carry real Pascal codes (14403, 2024112) that wrap-at-sink
  would drop. Concrete param keeps the traits object-safe (they are used `dyn`).
- **Error codes** = ONLY Pascal `DoSimpleMsg`/`DoErrorMsg` numbers. Assigned to
  every push site whose adjacent comment cites one (two `rg` passes incl.
  multi-line receivers), each verified against `.inputs/dss_capi`, plus a few
  exact-message matches found incidentally (8877, 99933/99934, 482, 566). ~80
  sites carry codes; uncited/port-specific messages stay `None` (never invented).
  NOT done: an exhaustive reverse Pascal lookup of every uncited message
  (unbounded, mis-assignment-prone) — out of P5a scope.
- **Settlement pass (audit-code F1/F2/F3, all fixed).** (F1) generator
  `do_dynamic_mode` phases-else was mis-coded 5672 → corrected to **5671**
  (generator.pas:1984 — the P5a comment had taken the number from the *different*
  procedure `InitStateVars`, gen.pas:2357/code 5672, which is ported separately at
  `init_state_vars_impl`). (F2) `interpret_time_step_size` S2-parse-failure arm
  (and the empty-string guard, same `'Error in specification of StepSize: %s'`
  message) was mis-coded 99934 → corrected to **99933** (ExecOptions.pas:335);
  99934 is a *different* message (units-else, :346) and stays on the units arm.
  (F3) completed the missed-code sweep — bare-string pushes carrying an
  unambiguous single Pascal number were coded: 484 (Sampling, Solution.pas:1990),
  131 (Load-Duration, ExecOptions.pas:484), 283/277 (EnergyMeter disabled/not
  found, ExecHelper.pas:3157/3160), 718 (WriteClassFile ×2, Utilities.pas:1204),
  240 (obj=Class.Name, ExecHelper.pas:219), 267 (BatchEdit, ExecHelper.pas:313),
  721 (overwrite guard, ExecHelper.pas:3713), 567 (user-model missing ×3 —
  gen/pv/storage). Deliberately left `None`: the three "Error opening file" /
  "could not be opened" sites (`command.rs` 1657/1663/1680) merge two Pascal
  branches with *different* codes (615/617, 613/58613, 70401/70501/70502) so no
  single code is faithful; and the IterNumber/CtrlIterNumber/IntegrationFlag
  read-only site (`set_cmd.rs`), whose old comment cited a phantom code
  (25040103) absent from the Pascal source — comment corrected, code stays `None`.
- **Text consumers re-baselined once:** `Export ErrorLog` now writes `[dss::eNNN]
  message` (bare message when uncoded); frozen. The only error-log golden
  (`export_errorlog.txt`) is an empty dump → byte-identical, no regeneration.
  Numeric goldens untouched (`git status tests/golden` clean). The `#219`
  show-busflow assert rewritten to `code == Some(219)` + substring.
- `From<ParserError>`/`SparseError`/`SingularMatrix` for `DssDiagnostic` land in
  `diag.rs`; the "Error Encountered in Solve: {e}" catch sites carry code 482.

## 1l. DE_PASCALIZE P15 — `dss-sparse` allocation & indexing hygiene [A] (branch `wt-p15`)

Stratum **[A] bit-neutral** — the solver hot path. Same arithmetic, same
summation order; the checkpoint Y goldens + `corpus_live` are the bit-exact/floor
proof. Base `update@13dde5c`. Items 1–6 of `DE_PASCALIZE_PLAN §P15`, plus the M1
benchmark baseline the WP is measured against.

**What changed (all bit-neutral):**
1. **`SparseSet` reuse across Y rebuilds.** `build_y_matrix` no longer throws away
   the sparse set every rebuild — `reuse_or_new_sparse` reuses it when the node
   count is unchanged (`ymatrix.rs`), `zero()` now *retains* the assembled-matrix
   skeleton, dedup cache, LU symbolic analysis and row-equilibrated matrix. A
   value-only rebuild (tap change, per-step load `Yeq`) refactors without
   re-analyzing; a renumber-with-same-count is caught by the stamp-pattern check
   (item 2) and falls back to a fresh build.
2. **Dedup-mapping cache in `assemble`** (`AssembleCache`, the subtle item). The
   first assembly of a pattern records `map` (triplet→cell), `first` (first
   occurrence = assign, else `+=`), `keys` (cell→(r,c) signature) and
   `cell_to_csc` (cell→CSC position). A same-pattern rebuild validates the `(r,c)`
   stamp sequence (`O(nnz)` int compare, no hashing), re-accumulates each cell in
   stamp order — **bit-identical** to the HashMap path incl. `-0.0` (assign on
   first, `+=` after) — and scatters verbatim into the existing CSC value buffer.
   Any mismatch/`zero()`-to-new-pattern rebuilds the cache. Insertion-order
   summation is preserved (the shared kernel; **no** faer-native dedup, per the
   plan's permanent ban).
3. **Killed per-element `to_row_major()`** in the stamping loop:
   `SparseSet::add_primitive_matrix_col_major` reads the `TcMatrix` column-major
   storage directly, traversed in the exact same row-major `(i,j)` order (triplet
   insertion order = dedup summation order unchanged) — one fewer transpose
   allocation per element per rebuild.
4. **`build_scaled` reuse:** the row-equilibrated matrix is kept in `self.scaled`;
   a same-pattern refactor overwrites its value buffer in place (no `vals.to_vec()`
   + `symbolic.to_owned()` copy). Row-max via `zip`, per-column value slices.
5. **Caller-side per-iteration alloc:** `Solution::solve_system_into` reuses a
   `solve_rhs` scratch field (`mem::take`-swapped for the disjoint borrow) instead
   of `currents[1..].to_vec()` every fixed-point iteration. `rcond` documented as
   cold-path *accept* (no scratch fields — keeps the hot state small).
6. **Idiom sweep [A]:** `solve_one` index loop → `zip`; `find_islands` recursive
   `find` → iterative two-pass path compression (recursion was unbounded on a
   degenerate ~8500-node chain); `get_element`/`coo_entries` `zip`. Same treatment
   applied to `RealSparseSet` (NCIM Jacobian) where it transfers — the `zip`
   idioms; the reuse/dedup cache does **not** transfer (NCIM rebuilds the Jacobian
   fresh each iteration, so there is nothing to reuse).

**Bit-neutrality proof.** New dss-sparse unit tests:
`cached_fast_path_matches_fresh_bitwise` (a reused `zero()`+restamp set vs a fresh
build — every assembled-Y value and every solved-x value bit-identical via
`to_bits()`), `cache_invalidates_on_pattern_change`,
`cache_reaccumulates_new_values_not_stale`. Engine level:
`checkpoint_scenarios_match_oracle` green (assembled Y + **exact iteration
counts** + node voltages unchanged); full `corpus_live` green at floors.
`tests/corpus` pristine.

**Benches (MULTITHREADING_PLAN M1 — created here; `crates/dss-core/benches/`,
criterion, `default-features=false`).** `snapshot_8500` (end-to-end compile+solve),
`ybuild_8500` (`Dss::rebuild_system_y` — whole-Y rebuild in isolation),
`lu_factor_solve` (`SparseSet` zero→restamp→factor→solve at 8500 scale). Median,
release bench profile, before → after:

| bench | before | after | Δ |
|---|---|---|---|
| `ybuild_8500/rebuild_whole_y` | 4.73 ms | 4.33 ms | ~8% (rest is the per-element YPrim recompute, not P15's target) |
| `lu_factor_solve/zero_restamp_factor_solve` | 10.32 ms | 4.41 ms | **~57%** (symbolic-LU reuse + dedup cache) |
| `snapshot_8500/compile_solve` | 301 ms | 186 ms | **~38%** (rebuild reuse over the control-iteration Y rebuilds) |

Numbers carry load-contention noise (parallel worktrees); the relative wins,
especially `lu_factor_solve`, are the architectural signal. `daily_ieee8500` (the
4th M1 bench) is left to the MULTITHREADING M1 owner (needs the meters/monitors
time-series harness; not required by P15's DoD).

**Deviations.** (a) `CMatrix::to_row_major` kept as a documented `pub` utility
(no live callers after item 3; removing an unrequested `pub` method is out of P15
scope). (b) `rebuild_system_y` added as a `pub` benchmark entry point (the only
public way to drive `build_y_matrix` in isolation for `ybuild_8500`). (c) The full
three-command gate is run once on the final tree rather than per intermediate
commit — the steps are monotonic bit-neutral and the oracle gate is expensive
under worktree load contention; every commit builds.

**Audit settlement (two independent audits over `13dde5c..cb2311c`).** Both
returned ACCEPT with only low/medium *coverage* notes — no bit-neutrality,
correctness, or tolerance findings (the col-major traversal was independently
re-derived bit-identical to `add_primitive_matrix(&to_row_major())` by
construction). The three coverage gaps are closed with targeted gating unit tests
in `crates/dss-sparse/src/tests.rs`:
- **T1 (medium) — `find_islands` untested + zero callers.** Added
  `find_islands_components_and_deep_chain`: verifies component labels on a mixed
  two-island + isolated-node case, and runs the iterative path-compression on a
  degenerate 20 000-node chain (the stated stack-safety motive) — completes
  without overflow and returns one island. `find_islands` remains a
  caller-less public `KLUSolve FindIslands` mirror (kept as API surface, not
  removed — out of P15 scope); it is now *exercised*, not only reasoned.
- **T2 (low) — no gating test asserted the fast path was taken.** Added
  `same_pattern_rebuild_takes_fast_path`: asserts the `pattern_reused` fast-path
  sentinel is set on a same-pattern value-only rebuild and *cleared* on the
  first build and on a `(r,c)`-sequence change. A silent fallback to the slow
  path (a pure perf regression the bit-neutral tests miss) now fails `cargo test`.
- **T3 (low) — item 3's live `add_primitive_matrix_col_major` had no direct
  test.** Added `col_major_stamp_matches_row_major_transpose_bitwise`: a
  bit-for-bit differential between the column-major read and the row-major
  transpose fed through the old path, incl. a ground node to exercise the skip.

Code-audit lows are process/observation, not defects: **P15-1** (engine-level
fast-path coverage depends on value-only corpus rebuilds) is evidenced by the
`snapshot_8500` ~38% win, which comes precisely from the control-iteration Y
rebuilds hitting the reuse path on a real 8500-node deck; **P15-2** (gate run
once on the final tree) is discharged here — the full three-command gate was
re-run green at defaults on the settled tree. dss-sparse unit tests: 18 → 21.

## 1m. DE_PASCALIZE P9 — `CMatrix` ergonomics [A] (branch `wt-p9`)

Stratum **[A] bit-neutral** — `support/cmatrix/mod.rs`. Base `update@afba752`
(post-P15: the sparse assemble reads CMatrix column-major storage directly via
`add_primitive_matrix_col_major`). Goal: replace the repeated raw column-major
`idx` closure / `j*n+i` offset pattern with typed accessors, without changing any
arithmetic or statement order.

**Added accessors.** `Index<(usize,usize)>` / `IndexMut<(usize,usize)>` (element
access `m[(i,j)]`, resolving the offset through the one private `idx` helper);
`col(j)`/`col_mut(j)` (a whole column is one contiguous span in column-major
storage); `columns()` (column-slice iterator); `row(i)`/`row_mut(i)` (strided row
iterators).

**Internals rewritten on them (bit-neutral):** `set`/`add`/`get` →
`self[(i,j)]`; `is_col_row_zero` → `row(n).chain(col(n))`; `zero_row` →
`row_mut`; `zero_col` → `col_mut`; `avg_diagonal`/`avg_off_diagonal`/`mv_mult` →
`self[(i,j)]`; `invert` → the local `idx` closure removed, every `a[idx(i,j)]` →
`self[(i,j)]` (identical offset `j*l+i == col*n+row`), trailing negation loop →
`self.negate()` (verbatim `for v in &mut self.values { *v = -*v }`);
`mtrx_mult` → the per-column copy scratch dropped, feeds `b.col(j)` straight into
`mv_mult`. The `cdiv_fpc` Smith-division cross-term and the no-row-exchange
Gauss-Jordan pivot sequence are untouched (algorithm identity → Stage F).

**Statement-order proof.** The `invert`/`mv_mult` diff is a pure index-notation
swap: `git diff` shows every kernel statement byte-identical except
`a[idx(i,j)]`→`self[(i,j)]`, which the `idx` method proves compute the same flat
offset. Pinning tests green **unchanged**: `cdiv_fpc_matches_fpc_smith_not_naive`
(bit-exact Smith division, both branches), `invert_*` round-trips, the checkpoint
per-element YPrim goldens, `transformer_yprim_bitexact`, line-constants, and the
P15 seam differential `col_major_stamp_matches_row_major_transpose_bitwise`
(traversal order load-bearing). Full `corpus_live` green at floors; `tests/corpus`
pristine. New unit tests pin the accessors:
`index_ops_match_get_set_and_column_major_layout`,
`col_and_row_iterators_walk_the_expected_entries`.

**Deviations / left in place (documented, not defects):**
- `mathutil::etk_invert` keeps its own local `idx` closure — it operates on a raw
  `&mut [f64]` slice (real-matrix Gauss-Jordan), not `CMatrix`; a separate kernel
  outside P9's file scope, statement order owned by Stage F.
- `diakoptics/matrices.rs` link-prim extraction keeps its flat 1-based `cValues`
  k-stride walk over `yprim.values()` — a bespoke Pascal-faithful stride
  reproduction (with defensive `.get()` out-of-range skips), not the `(i,j)` idx
  pattern; converting it risks a behavior change and is out of P9 scope.
- `CMatrix::to_row_major` kept (unchanged `pub` utility, as under P15).
- External `mv_mult`/`get`/`set`/whole-slice `values_mut` call sites already use
  the ergonomic public API; the only manual `j*order+i` offsets elsewhere index
  raw parse/JSON buffers, not `CMatrix` storage — out of scope.

## 1a. Archived — completed plan records (100% done)

> Moved out of the active §1 frontier on 2026-07-17. These are the records of plans whose own work-package scope is closed and gate-green: the 1:1 FINAL ACCEPTANCE, JSON export (Stages A+B), DIAKOPTICS/PSTCALC **Part I**, and the full **UPGRADE** Rung 1 + Rung 2 (r4133 parity). A few carried a documented item forward to a successor plan that has **not** finished it yet (TODO(compat) sweep + HIDE_015X → DE_PASCALIZE Stage F; GICMvars export → Phase 9; JSON DynInit/Full-mode tail → a follow-up WP; IEEE118 NCIM → a future UPGRADE rung) — those open items are surfaced in §1's **Standing open follow-ups**, not buried here. Frozen history — superseded only by the code and tests. In-progress / not-started plans (DE_PASCALIZE, DIAKOPTICS Part II, RESONANCE, MULTITHREADING, WASM_USERMODELS) stay in the active §1 above.

**Late-UPGRADE work records (historical — all landed; kept for the §UPGRADE
cross-refs).**
- **D14 (DynamicExp RPN "index-bug fix") — REVERTED 2026-07-19 (BUG WP DynExp).**
  ~~landed, pulled ahead of WP-U1.6~~ Superseded: adopting the 0.15.x `2a8bdb78`
  no-op `SolveEq` was a mistake — it targeted the retired **non-gating** capi015
  and the port matched neither surviving oracle. Both gating channels (pinned
  0.14.5 `DynamicExp.pas:377` AND EPRI r4133 `:497`) run the full RHS evaluator
  and integrate. `solve_eq` is restored to that full evaluator and the DynExp
  gates re-pinned to the swinging-oracle values. See the **BUG WP DynExp** record
  in §1 and DIVERGENCES.md §D14.
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

## EPRI bridge parity round — `dss-epri` covers every retired-Oddie task (branch `epri-parity`, 2026-07-19)

User mandate: FULL functional parity — every task the retired Oddie/dss-python
bridge ever did must be doable through the in-house Rust bridge
(`crates/dss-epri` drives the same official r4133 engine natively). Closes the
Phase-E re-review findings F2 (dead golden-regen r4133 arms) and F3 (lost
`probe_59n` reproduction).

### Consumer inventory (everything the Oddie bridge ever served, at `ae3e6bd^`)

| consumer (pre-Phase-E) | task | disposition |
|---|---|---|
| `ab_compare.py` | corpus A/B divergence reports between engine revisions | retired-by-design — the unified two-channel gate (`capi_v0145` + `r4133` in `corpus_gate.rs`) replaced the reporting channel |
| `sweep_modes_isolated.py` / `sweep_merge.py` | WP-U0.2 `ab_compare` crash-isolation + merge helpers | retired-by-design — `ab_compare` consumers; the sweep reports are frozen in `docs/upgrade` |
| `dsspy_validation/` + `wheels/` + venv | vendored dss-python full-API crosscheck | retired-by-design — one-time Rung-1 validation, superseded by the permanent gate |
| `dsspy_crosscheck.py` | classifier-manifest crosscheck vs DSS-Python's validation list (textual, engine-free) | retired-by-design — one-time promotion audit; results absorbed into the manifests |
| `smoke.py` | binary self-smoke | parity restored in Phase A — `epri-worker --smoke` / `crates/dss-epri/src/smoke.rs` + `tests/smoke.rs` |
| `xcheck_bridge.py` | Oddie-vs-Rust-bridge bit-diff | retired-by-design — self-obsoleting Phase-A fidelity proof (its PASS licensed the retirement) |
| `gen_ad_reference.py` | A-Diakoptics matrix reference harvest (r3723) | retired-by-design with documented contingency — baseline frozen (`r3723_ref/` + PROVENANCE); `tools/opendss/README.md` mandates reimplementation over `epri-worker` if ever re-run |
| `oracle_server.py` oddie/capi015 arms | opt-in oracle engines for the live test | retired-by-design — the gate channels replaced them |
| `gen_protection.py` r4133 arm (`make_oddie`; fuse_blow, swt_manual) | golden regeneration | **parity restored (this round)** — `make_epri` drives `epri-worker` via the `IOddieDSS`-shaped shim; payload byte-parity proven below |
| `gen_flicker.py` (r3723) | flicker golden regeneration | **parity restored (this round)** — rewritten over `epri-worker` on r4133 (the only vendored official revision); cross-revision payload byte-parity proven below |
| `probe_59n.py` | 59N artifact reproduction (WP-U2.6) | **parity restored (this round)** — recreated over `epri-worker`; artifact reproduced verbatim |
| *(adjacent, not Oddie)* `gen_bh_capi015.py` / `gen_regcontrol_capi015.py` / `gen_ncim_reports.py` / `gen_der_lines_harmonics.py` capi015 runs / `gen_checkpoints.check_pin` capi015 arm / `gen_reports.py` capi015-gated seasonal arm | capi015 (dss-python 0.16.0b2 / dss_capi 0.15.0b4) golden generation | outside the mandate — capi015 was the *Python-engine* beta channel, not the official-binary Oddie bridge; the Rust bridge cannot (different binary) and should not drive it. Goldens frozen; the capi015 arms fail loudly (missing `PIN_OPENDSS.txt`), no silent pass. |
| `gen_fuse_r4133.py` / `r4133_help.py` | derived golden (`git show` overlay) / frozen r4133 `PropertyHelp` data — no engine | unaffected — engine-free; their `oddie:r4133` provenance markers are historical (original Oddie captures, recorded in their notes); neither imports `IOddieDSS` |

### Protocol extension (gate-neutral)

New `epri-worker` commands `exec` / `read` / `chdir`
(`crates/dss-epri/src/script.rs`) — the generic scripting surface replacing
Oddie's `Text.Command` + per-property reads; one `read` = one DLL accessor +
per-call error poll (dss-python raise-on-error parity), `exec` = strict errno +
actor-idle barrier (the async-solve race fix applies to scripted `solve`/inline
solves too). New FFI: `SolutionV` (mode 0 EventLog — the raw `EventStrings`
`Hour=…, Sec=…` lines; the empty-log `None` placeholder is written WITHOUT a
terminator and normalizes to `[]`, matching the retired Oddie decode — pinned by
the committed `swt_manual` golden; deliberately a separate decode from the
gate's `decode_string_array`) and `BUSF(0)` kVBase; plus `CircuitS(4)`
SetActiveBus and `MonitorsS(2)` monitor-select on existing entry points. Gate
paths untouched: `run`/`ping`/`clear`/`quit` handlers, `capture.rs`,
comparators, scheduler are byte-unchanged and the gate never sends the new
commands. SAFETY rules upheld (immediate copies, `deny(unsafe_op_in_unsafe_fn)`,
per-boundary SAFETY docs). Worker-level smoke:
`crates/dss-epri/tests/protocol.rs` spawns the real binary and pins the exact
event-log line format the committed protection goldens store
(`Hour=0, Sec=0.2, ControlIter=1, Element=Fault.f, Action=**APPLIED**`), the
read shapes (solution scalars, node arrays, element powers/currents, variables,
monitor channels, bus kVBase), the empty-log `[]` normalization, and the
error path (`ok:false`, worker stays alive).

Python client `tools/opendss/epri_worker.py`: `EpriWorker` (spawn + line-JSON,
binary resolution mirroring the gate's `epri_worker_bin`: env override →
target/{release,debug} → one cargo build) and `EpriEngine`, an
`IOddieDSS`-shaped shim so `gen_protection.build()` + `capture_element()` run
unchanged and issue the same per-property DLL call sequence Oddie did.

### Parity proof (committed goldens UNTOUCHED — scratch regen + byte-compare)

| golden | committed capture | regen path | verdict |
|---|---|---|---|
| `protection/fuse_blow.json` | Oddie r4133 | epri-worker r4133 | `scenario` payload **byte-identical** (7 071 serialized bytes: per-step voltages, event log, final elements); diff confined to the `oracle` provenance block (the raw DLL version string lacks the retired wrapper's `\nDSS-Python version: 0.16.0b2` suffix) |
| `protection/swt_manual.json` | Oddie r4133 | epri-worker r4133 | `scenario` payload **byte-identical** (8 255 bytes); same provenance-only diff (the committed block also carries the older `oddie:r4133` marker shape from its original capture flow, which even the old generator would no longer emit) |
| `flicker/pst_demo.json` | Oddie **r3723** | epri-worker **r4133** | every payload key **byte-identical** across engine revisions — `raw_mag` (494 142 serialized bytes), `flk` (588 324), `pst` (547 074), `kvbase`, `times`, `deck`, `n`/`nphases`/`fbase`; only `oracle` differs. The official flicker meter + this deck's power flow are revision-stable; the committed golden stays the frozen r3723 capture. |

Zero committed-file changes (`git status tests/golden` clean); corpus pristine
after all runs (enforced for the flicker regen by the settle round's DataPath
redirect — see below). `DSS_GOLDEN_OUT` env (new) redirects both generators'
output for scratch parity runs; default remains the committed tree.

### F3 — probe_59n reproduction restored

`tools/opendss/probe_59n.py` recreated over `epri-worker`; run 2026-07-19
reproduces the documented artifact verbatim: `Relay.State =
'[closed, closed, closed, ]'` (byte-equal to the port's `render_state_array()`),
`Line.line1 |I|max = 1381.156 A` (the "~1381 A" cited in `relay/tests.rs`),
`f = 78.88 Hz` at t=1.0 s, wander `[67.16, 115.00] Hz` over the next 15 s →
`PROBE OK`, exit 0. `relay/tests.rs` doc comment updated to cite the bridge
driver (comment-only); `skipped_needs_investigation.json` untouched.

Docs: `tools/opendss/README.md` layout rows for `epri_worker.py`/`probe_59n.py`;
`tools/golden/README.md` generator-exceptions note. `TESTING.md`/`CLAUDE.md`
deliberately untouched (Phase F owns them; its one stale F2 sentence reconciles
after both merge).

### Settle (2026-07-19) — two opus xhigh audit dispositions

Both audits independently re-derived the acceptance (scratch regen through the
real `epri-worker` + byte-compare, probe_59n rerun, protocol smoke) and
confirmed it: payload byte-parity holds for all three goldens, zero committed
bytes changed, gate semantics untouched. Findings settled:

- **Flicker-regen corpus leak (audit-code fp-1 low / audit-tests PARITY-1
  medium) — CONFIRMED, FIXED in the driver.** A standalone
  `python tools/golden/gen_flicker.py` left an untracked 730 KB
  `pst_Mon_pst_1.csv` inside `tests/corpus/.../Examples/Scripts/`
  (reproduced), falsifying the original "corpus pristine" evidence and the
  generator's own comment. Root cause (settled against the r4133 Pascal): the
  DLL initializes `DataDirectory`/`OutputDirectory` from the
  registry-persisted `DataPath` (`HKCU\Software\OpenDSS\MainSect`,
  `ReadDSS_Registry`) = the directory of the last deck ANY local run
  `Compile`d (probe_59n's 59NRelayDemo → `Examples/Scripts/`); this deck is
  exec'd line-by-line, so nothing re-pointed it, and `export monitor` wrote
  there. A `CorpusGuard` over the deck dir could NOT cover it (the leak lands
  in a different, run-history-dependent corpus dir), so the fix is a
  deterministic `set DataPath="<temp>/dss_rs_flicker_export"` before the
  export (`Set DataPath` also ChDirs the worker — harmless post-deck;
  `DSSGlobals.SetDataPath`, r4133 line 934). Re-verified: standalone regen
  leaves `git status` clean, the CSV lands in the temp dir, and every payload
  key of the regen is STILL byte-identical to the committed golden AND to the
  pre-fix regen (the redirect changes zero payload bytes). Golden untouched.
- **Flicker byte-count figures (audit-code fp-2 low) — CONFIRMED, FIXED.**
  The parity-table row overstated the serialized sizes; re-measured from the
  committed golden (`json.dumps` per key): `raw_mag` 494 142, `flk` 588 324,
  `pst` 547 074 (table corrected above). The parity VERDICT itself was
  independently re-verified byte-true by both audits and by the settle rerun.
- **Inventory enumeration (audit-tests PARITY-2 low) — CONFIRMED, FIXED.**
  `gen_reports.py`'s capi015-gated seasonal arm and `r4133_help.py` (frozen
  r4133 PropertyHelp data) carry `oddie:r4133`/oddie-venv provenance mentions
  but import no `IOddieDSS`; added to their existing inventory classes
  (capi015 Python-engine row / engine-free frozen-data row). No parity target
  was missed — all 7 actual `IOddieDSS` importers were already dispositioned.

Settle gate: fmt/clippy/test all exit 0 (`cargo test --workspace` 7 min 05 s,
0 failures across all binaries; corpus-live 25/25). One settle-gate run left
six deck-authored export leftovers (`Test/AutoTrans/auto3bus_*`,
`GFM_IEEE8500/IEEE8500u_EXP_*`) in `tests/corpus` — the pre-existing
corpus-guard parallel-run race (`corpus_guard.py` docstring: incomplete
snapshot = never delete; end-of-run `git status tests/corpus` + path-limited
clean is the documented recovery, applied). Not introduced by this round (gate
capture paths byte-unchanged); open follow-up for the gate-hygiene backlog.

## EPRI capability round (Round 2) — `dss-epri` covers everything the Oddie bridge COULD DO, and beyond (branch `epri-capability`, 2026-07-19)

User mandate: **"the Rust FFI bridge must cover everything the python Oddie
bridge COULD DO — and beyond."** Round 1 (above) closed *usage* parity (every
retired-Oddie task doable through the bridge). This round closes **capability**
parity: the full API surface the Oddie/dss-python bridge exposed over the same
r4133 engine, mapped and bound — plus cheap "beyond" items. Additive and
gate-neutral: the `run`/`ping`/`clear` capture path, `capture.rs`, the
comparators and the `corpus_gate` scheduler are byte-untouched; the new commands
are never sent by the gate.

### Coverage table — zero unclassified exports

The r4133 `OpenDSSDirect.dll` export table was dumped with a throwaway stdlib
PE-export parser (no new deps) and cross-checked against the `exports` clause of
`Version8/Source/DDLL/OpenDSSDirect.dpr`. **164 exports, all classified:**

| class | count | reached via | binding status |
|---|---|---|---|
| uniform family entry points (42 families × present `I`/`F`/`S`/`V`) | 147 | generic `ffi` `(family, kind, mode, arg)` dispatch (`src/families.rs`) | 29 already typed (gate capture) + **118 newly reachable**; all 147 now generic |
| standalone gate-path exports | 6 | typed `Engine` methods | bound pre-R2 (Phase A / R1): `DSSPut_Command`, `ErrorCode`, `ErrorDesc`, `InitAndGetYparams`, `GetCompressedYMatrix`, `getIpointer` |
| Y-matrix / injection helpers | 9 | `ymatrix` command | **newly bound (R2)**: `ZeroInjCurr`, `GetSourceInjCurrents`, `GetPCInjCurr`, `SystemYChanged`, `BuildYMatrixD`, `UseAuxCurrents`, `AddInAuxCurrents`, `getVpointer`, `SolveSystem` |
| Delphi RTL debug symbols | 2 | — | **skip-by-design (non-API)**: `__dbk_fcall_wrapper`, `dbkFCallWrapperAddr` (madExcept/debug hooks, not engine surface) |

The 42 families and the ABI shapes each exports (a `None` marks a shape the
family lacks — the registry spells out every real export symbol, since names are
not always `NameX`: `Bus`→`BUSI…`, `Loads`→`DSSLoads…`, `DSSProperties` is a
bare `S`):

`ActiveClass isv · Bus ifsv · CapControls ifsv · Capacitors ifsv · Circuit ifsv
· CktElement ifsv · CmathLib fv · CtrlQueue iv · DSS isv · DSSElement isv ·
DSSExecutive is · DSSProgress is · DSSProperties s · Fuses ifsv · GICSources
ifsv · Generators ifsv · Isource ifsv · LineCodes ifsv · Lines ifsv · Loads ifsv
· LoadShape ifsv · Meters ifsv · Monitors isv · PDElements ifs · PVsystems ifsv
· Parallel iv · Parser ifsv · Reactors ifsv · Reclosers ifsv · ReduceCkt ifs ·
RegControls ifsv · Relays isv · Sensors ifsv · Settings ifsv · Solution ifsv ·
Storages ifsv · SwtControls ifsv · Topology isv · Transformers ifsv · Vsources
ifsv · WindGens ifsv · XYCurves ifsv` = 147 entry points.

Skip-by-design detail: **`DSSProgress` (I,S)** is a headless progress-form
no-op — it IS bound in the family table (so the bridge literally covers
everything Oddie could call), but there is no observable engine state to assert
on, so no dedicated smoke drives it. The r4133 DDLL exports **no** plotting /
DSSGraph / registry / file-dialog symbols at all (the `Forms`/`Plot` units
compile in but export nothing), so the skip list stays limited to `DSSProgress`
+ the two Delphi debug symbols — the whole rest of the surface is bound.

### Systematic binding (`crates/dss-epri`, SAFETY rules upheld)

- **`src/families.rs`** (new): the 42-family registry + generic dispatch. `FnI/F/S/V`
  symbols loaded per family into a `FamilyTable` (case-insensitive lookup); one
  `Engine::ffi_dispatch(FfiCall)` reaches every mode of every family. Decodes the
  V-protocol by `myType` (1=int / 2=double / 3=complex re/im / 4=string / 5=bytes)
  with a **raw** string split (no gate-path monitor-header space strip — the gate's
  `decode_string_array` is untouched), and supports V **setters** (array-in via
  `myPointer`, e.g. `LoadShapeV(2)` PMult write). Unit-tested (`decode_v` by tag,
  raw string split, `encode_v_set`→`decode_v` round-trip).
- **`ffi.rs`**: `YMatrixFns` (the 9 standalone Y-helpers, transcribed 1:1 from
  `DYMatrix.pas`) + the family table, loaded in `Dll::load` and handed to `Engine`
  via `leak_into_parts`. `sym` is now `pub(crate)`. All FFI stays behind
  `deny(unsafe_op_in_unsafe_fn)` + per-boundary `// SAFETY`; DLL still never
  `FreeLibrary`'d; NoFormsAllowed stays set; V-buffers copied out immediately.
- **`dss.rs`**: `ffi_dispatch` + `ym_*`/`v_pointer`/`y_dims`/`solve_system` typed
  wrappers (each with a SAFETY note); `FfiCall`/`FfiOut` types.
- **`script.rs`**: `handle_ffi` + `handle_ymatrix` (parse/serialize only; all FFI
  is in `dss.rs`).

### New worker protocol surface (gate-neutral)

Four additive `epri-worker` commands (`src/bin/epri-worker.rs`):

- **`ffi`** — generic `{family, kind, mode, iarg|farg|sarg, vset?}` → kind-tagged
  reply `{kind, value|data, type, n, errno, error}`. Reaches every DDLL family
  mode: scalar get/set (i/f/s), array get (v getter), array set (v setter via
  `vset:{type,data}`).
- **`ymatrix`** — the standalone Y-matrix/injection helpers by op name
  (`y_dims`, `vpointer`, `ipointer`, `solve_system`, `system_y_changed`,
  `use_aux_currents`, `build_y`, `zero_inj`, `get_source_inj`, `get_pc_inj`,
  `add_aux`).
- **`caps`** — structured capability handshake: `protocol_version`, oracle
  identity, the family manifest (name + kinds), `family_count`,
  `family_entry_points`, the command list, and the `ymatrix` op list.
- **`batch`** — many `exec` commands in one round-trip (stop-on-first-error;
  `{replies, ran, failed_at}`).

### Beyond (what the Python/Oddie bridge never had)

1. **Per-call structured errno surface** — every `ffi`/`ymatrix` reply carries the
   `ErrorCode`/`ErrorDesc` polled *right after* the call (`{errno, error}`),
   non-fatally. dss-python only raised/aggregated; here the caller sees the exact
   engine errno per call (smoked: no-active-LoadShape read → `#61001` surfaced).
2. **Batched multi-command exec** (`batch`) — compile+build+solve in one
   round-trip instead of one command per line.
3. **Structured version/capability handshake** (`caps`) — the whole reachable
   surface introspectable in one message.
4. **Worker-pool crash isolation + recycling** (already in the gate's `EpriPool`,
   `crates/dss-core/tests/corpus_gate/engines.rs`, untouched here) — a per-request
   deadline → kill/respawn/retry-once, recycle-after-N, one-shot per case for
   serial/isolate. The single-process Oddie/dss-python host had none of this;
   documented as the standing "beyond" the transport already provides.

### Smoke evidence

- `cargo test -p dss-epri --lib`: 4 `families` unit tests green (decode-by-tag,
  raw string split, `encode`↔`decode` round-trip, `vset_len` element-count).
- `crates/dss-epri/tests/protocol.rs::capability_surface_end_to_end` (new, drives
  the REAL r4133 DLL): `caps` (42 families / 147 entry points / proto 2 / commands
  present / family-shape spot-checks), `batch` build, `ffi` i/s/v getters
  **cross-checked equal to the typed channel** (`Circuit` NumNodes == node-order
  len; `AllElementNames` == typed read), `ffi` V-set **round-trip** (LoadShape
  PMult `[1,1,1]`→set→`[5,6,7]`), `ymatrix` (`y_dims.n_bus`==NumNodes, `vpointer`
  shape `2*(N+1)`, `system_y_changed`, `solve_system` == KLU success 1 — the
  engine's own `Solution.pas` `IF SolveSystem(...) = 1` success test), structured errno
  `#61001`, and error paths (unknown family / unknown kind / absent ABI shape all
  `ok:false`, worker survives). The pre-existing `scripting_surface_end_to_end`
  smoke is byte-untouched.

Gate: fmt/clippy/`cargo test --workspace` all green at defaults (corpus pristine
after runs; a StorageControllerTechNote guard-race leftover was path-limited
cleaned — same standing gate-hygiene backlog item as Round 1, not introduced
here). Base `09d03e5`.

### Settle (two opus-xhigh audits, 2026-07-19)

Two independent xhigh audits of the round; export table re-derived independently
(164 exports = 147 family + 6 gate-path + 9 Y-helpers + 2 Delphi debug — zero
unclassified, headline confirmed). Both audits agreed the binding is faithful
and gate-neutral. Findings settled empirically (drove the worker + live DLL):

- **F1 (high, FFI-safety — the round's #1 focus): FIXED.** The generic V-set
  `call_v_set` passed the array's **byte** length as `mySize`, but the r4133 SET
  path treats `mySize` as an **element (point)** count — it clamps `LoopLimit :=
  min(mySize, NumPoints)` and steps `myPointer` one element per iteration
  (`DLoadShape.pas` PMult write; `DXYCurves.pas` XArray write). A byte count (8×
  for doubles) defeats the clamp, so the DLL over-reads the Rust buffer whenever
  the supplied array is shorter than the target's point count — a real OOB read.
  Fix: new `families::vset_len` passes the element count; `call_v_set`'s SAFETY
  note now states the true invariant (reads ≤ `min(size, NumPoints)` elements)
  and the caller-supplies-full-array contract for the setters (`DXYCurves`) that
  ignore `mySize` entirely. New tests prove element-count semantics: a 3-element
  write into a **5-point** LoadShape fills points 1..3 and clamps there
  (`[10,20,30,2,2]`, `written == 3`) — the exact `len < NumPoints` case the old
  byte-count code would have over-read; plus a `vset_len` unit test.
- **F2 (low, robustness): FIXED.** Seven DYMatrix ops
  (`system_y_changed`/`use_aux_currents`/`build_y`/`add_aux`/`vpointer`/
  `ipointer`/`solve_system`) hit unguarded `ActiveCircuit.Solution` in the DLL,
  so sending them before a circuit is compiled nil-derefs and kills the worker.
  `handle_ymatrix` now rejects the crash set with a clean error when
  `circuit_name()` is empty (`CircuitS(0)` is nil-guarded → `""` with no
  circuit). New test `ymatrix_before_compile_is_guarded_not_a_crash` drives all
  seven on a fresh worker: each returns `ok:false` and the worker stays alive.
- **Test-coverage holes (audit-tests, all closed):** batch **stop-on-first-error**
  path (bad middle command → `ran == 2`, `failed_at == 1`, trailing command not
  run); generic **`f`-kind getter happy path** (`Solution.Frequency` mode 0 ==
  60); the 7 previously-unexercised **ymatrix ops** (`use_aux_currents`,
  `build_y`, `zero_inj`, `get_source_inj`, `get_pc_inj`, `add_aux`, `ipointer`);
  and `system_y_changed` upgraded from a presence-only check to a real
  **read/write round-trip** (set-true→reads 1, set-false→reads 0).

Smoke now: `cargo test -p dss-epri` = 4 lib unit tests + `protocol.rs`'s 3
end-to-end tests (`scripting_surface_end_to_end`,
`capability_surface_end_to_end`, `ymatrix_before_compile_is_guarded_not_a_crash`)
all green against the real r4133 DLL. Nothing deliberately left unfixed.

## 0.15.x adoption sweep — fix round (branch `fix-015x-sweep`, base `57fbcde`)

The sweep-015x adversarial review (wf_9daaa9d9-13b) re-verified all 41
dss_capi-0.15.x adoptions under the new CLAUDE.md rule (0.15.x is not an
authority) and flagged 5 BUG + 1 SUSPECT. This fix round settles all eight brief
items; every code fix re-checked the decisive r4133 citation in
`.inputs/electricdss-code-r4133-trunk` and, where live-observable, ran an OWN
epri-worker (r4133) / pinned-oracle probe before/after. All gate-neutral today
(zero corpus/golden movement); each is a deterministic deck-reachable r4133
divergence the port now matches.

- **#1 l4e2 seasonal GetRatings (`62b0f14`).** `traits.rs::get_ratings` gates the
  `AmpRatings` override on `num_amp_ratings() > 1` (r4133 PDElement.pas:351,
  re-read: `(RatingIdx <= NumAmpRatings) and (NumAmpRatings > 1)`). 0.15.x
  `55400a29` dropped the guard → a single-season element silently hid its user
  `normamps`/`emergamps` under `SeasonRating` (a bug vs r4133 + physics). Kept the
  memory-safe `0<=idx<num` bound (r4133's `<=` is an off-end read). Flipped the
  single-season line pin; DIVERGENCES L4/E2 updated.
- **#2 c5 RegControl rev-only fallback (`2070d50`).** `reg_control/accessors.rs`
  `end_edit`: sign-preserving `Fwd:=Rev; Rev:=-Rev` (r4133 RegControl.pas:499-507,
  re-read — NO abs). 0.15.x `8a898cba`'s `abs` inverts the band for a negative
  `revThreshold`; own probes: r4133 AND pinned 0.14.5 both abort #485, port now
  #485 (was: converged) — the D14 signature. Unit-pinned; DIVERGENCES C5 abs
  attribution + stale idle-OR paragraph refreshed to bounded-AND.
- **#3 a3a5 PCE force-hook fidelity (`5f73d36`).** (a) WindGen `get_currents`
  gains the `ForceInjCurr` guard (r4133 WindGen.pas:2148, re-read) — the 6th of
  six flag-checking classes; pin `windgen_force_inj_freezes_iterminal` reads the
  corpus snapshot/GetCurrents path (own r4133 probe: frozen
  `[-89.23,-11.20,34.91,...]`; toggle-verified fails ~48 A off without the guard —
  the previous `get ITerminal` unit test was non-discriminating). (b) Moved the
  force check INSIDE each class's `inj_currents` (r4133 Load.pas:1922 et al.: skip
  only `calc_inj_current_array`, keep the unconditional set-nominal preamble +
  inherited add); dropped the central force-branch in `power_flow.rs` so the force
  is inert for VCCS/UPFC/VSConverter (0 `ForceInjCurr` hits in r4133). (c)
  DIVERGENCES A3/A5 corrected: the hooks EXIST in EPRI r4088+/r4133, gated
  engines=r4133.
- **#4 allownoneitem `none` conductor lists (`919b76e`).** `load_spacing_and_wires`
  compacts NIL slots (r4133 LineGeometry.pas:1190-1262, re-read): recount
  actualNConds/actualNPhases, skip-NIL copy with original spacing coords
  (byte-identical for lists with no NIL). `make_z_from_spacing` gains the #181021
  phase-mismatch guard (r4133 Line.pas:2213-2219). A Line `wires=(w none)` now
  compacts + solves (own r4133 probe I1=`(21.802597,-0.001427)`; port matches to
  a faer-vs-KLU floor). Dropped ALLOW_NONE_ITEM from the three LineGeometry lists
  (r4133:346-396 reject #10103; own probe: r4133 #10103, 0.14.5 #40303); kept it
  on Line-level + Conductors. Pin rewritten (geometry-reject + line-accept-solve).
- **#5 conductors-1to1 case-insensitive match (`04b3f4b`).** `parse_conductor_proxy`
  resolves a class-prefixed item by `eq_ignore_ascii_case` (r4133
  `LowerCase(CondClass)` dispatch, re-read LineGeometry.pas:485-503), dropping the
  reproduced capi015 `GetDSSClass` case-bug `TODO(compat)` — a capi015-ONLY
  breakage (r4133 has no `TProxyClass` and parses `conductors=` natively + solves).
  Own r4133 probe: `Conductors=[WireData.w wiredata.w]` converges,
  I1=`(21.801759,0.027069)`; port matches to a floor. Flipped the pin to
  resolve+solve; bare-name/#402/all-none-geometry stay rejected per-channel (own
  probes: r4133 bare #181023, all-none-geometry #303 AV — UB not reproduced).
  Refreshed the stale "upstream-broken" comments; DIVERGENCES retitled.
- **#6 b1 capacitor SUSPECT — doc-only (`0bb7d0b`).** Kept the code (physics +
  VSConverter precedent). Corrected DIVERGENCES §B1: the `×1.000001` on SpecType-3
  is capi015-ONLY — re-read r4133 Capacitor.pas (`×1.000001` in the `1,2`
  Line-Line arm only; `3:` CMatrix arm bare `Invert; add ZL; Invert`); own r4133
  probe: cmatrix cap + R/XL → Q≈1.4e-15 (garbage) == 0.14.5, vs -76.2 kvar without
  R/XL. Added the per-channel pre-registered ledger policy for a future
  SpecType-3+ZL witness.
- **#7 §4 doc follow-ups (`0bb7d0b`).** D12 (3 SwtControl decks are engines=r4133,
  WP-U2.4 D6); TCC_Curve(b) superseded by WP-U2.1 (silent-clear r4133,
  `fuse_curve_none_clears_silently_like_r4133`); D13 mmf deck engines=r4133; D4
  r4133 cite :2184 (re-read); c5r3723 forward-note r4133 LoadShape Mode@22 to
  UPGRADE Rung-2 (re-read LoadShape.pas:246-247); D2 MonBus "0 corpus uses" stale
  (`invcontrol_monbus.dss` exists, valid deck).
- **#8 NCIM defer correction — re-gate BLOCKED, kept deferred (this commit).**
  The four NCIM defer_ledger texts + `ledger.json` `ncim-oppoint` cause claimed a
  "different PV/Q operating point". **DISPROVEN by own r4133 probes:** converged
  node voltages are DIGIT-IDENTICAL r4133-vs-port to a faer-vs-KLU floor
  (`ncim_pq` all 9 nodes ~1e-12, iters 3==3; `ncim_pv_pq` all nodes ~1e-12, iters
  port 8 / r4133 4). The re-gate was ATTEMPTED but is **genuinely blocked**: the
  reported `Vsource.source` swing current diverges by a phase rotation (port
  `~[c2,c3,-c1]` matching its capi015 NCIM oracle vs r4133 `[c1,c2,c3]`) — a real
  reported-current divergence that is NOT a different op-point and NOT a tolerance/
  iteration issue. Because a divergence that may be a port bug must be
  proven-not-a-bug before ledgering (R3), it is left UNDER INVESTIGATION (a
  suspected port `NCIM_CalcInjCurrAtBus` phase-indexing quirk) — the honest
  outcome, not a papered-over ledger row. Corrected the `ncim-oppoint` cause + the
  4 defer_ledger texts (op-point NOT different; the residuals are the iteration
  cadence [dss_capi NCIMSolutionHelper refactor] + the swing-current divergence).
  The historical STATUS WP-U1.7/NCIM "different op-point / suspected port bug"
  wording is likewise superseded by this record. NEVER a tolerance loosening.
  **Follow-up:** investigate the NCIM swing-source current phase rotation (port vs
  r4133); if a port bug, fix + re-gate; if a proven capi015-vs-r4133 reporting
  difference, ledger it per-channel and re-gate the 4 cases.

Gate: full three-command gate green at defaults before every commit
(`cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D
warnings`; `cargo test --workspace` — dss-core lib 1251 tests, corpus
`corpus_gate_all_cases_match_engines` ok on all 514 cases, ~150-300s wall). Corpus
kept pristine (path-limited cleanup only). No tolerance loosened; no new ledger
row (item 8 blocker left open, not ledgered).

### Settle round (two independent read-only audits, code + tests)

Every finding reproduced empirically before disposition; no tolerance loosened,
corpus pristine, `#8` NCIM blocker re-derived from the exact golden-generating
oracle build.

- **NCIM swing-current "suspected port bug" → PROVEN a faithful capi015
  reproduction, NOT a port bug (audit-code F3, audit-tests F1, medium).** Read the
  exact oracle build the NCIM goldens/pins were generated from — dss_capi
  **0.15.0b4 (e936d210, SVN 4103)**, not the standard pinned 0.14.5 (which has no
  NCIM at all). Its `NCIM_CalcInjCurrAtBus` fills `ce.GetCurrents(ElmCurrents)` at
  index 0 then reads `ElmCurrents[j]` (`j:=1..NPhases`) — a one-conductor shift.
  The port's `+1` in `exec/view.rs::ncim_swing_source_currents` reproduces this
  0.15.0b4 shift **exactly** (unit test `ncim_vsource_reported_currents_match_oracle`
  green). r4133 VSource.pas:1123 FIXED the shift (`GetCurrents(@(ElmCurrents[1]))`,
  an offset write) — so the port matches capi015 and diverges from r4133, a
  determinate, defined capi015-vs-r4133 upstream divergence (known-bug policy,
  already a documented `TODO(compat)`). Disposition: **corrected** the 4 NCIM
  defer_ledger texts + `ledger.json ncim-oppoint` cause + the `view.rs` comment to
  state the proven cause (dropped the false "suspected port bug / phase-indexing
  quirk / UNDER INVESTIGATION" framing); re-gate **kept deferred** (a genuine,
  now-proven blocker — the swing-current channel differs by the upstream fix; a
  live r4133 re-gate needs a per-channel ledger row scoping `Vsource.source`
  reported currents, never a tolerance loosening — recorded as the re-gate recipe).
  Text-only edits: population-lock safe (defer_ledger boolean-hashed
  `population_lock.rs:150`, `ledger_tags` digests only `entries`) — lock green.
- **line_fetch geometry-reject assertions loosened to bare smoke checks
  (audit-tests F3, low) → FIXED.** Restored discriminating message assertions in
  `conductor_none_geometry_rejects_but_line_accepts_and_solves`: geometry-level
  `none` now asserts the port's `WireData object "none" not found.` (mirrors 0.14.5
  #40303 / r4133 #10103) and the `nope` control asserts `object "nope" not found`
  — a future unrelated parse failure no longer greens the pin.
- **Force sub-fix (c) — VCCS/UPFC/VSConverter force-inertness untested
  (audit-tests F2, low) → FIXED.** Added `force_inj_current_is_inert_for_vsconverter`
  (`force_hooks.rs`): forcing a VSConverter's `InjCurrent` with a ≈5 kA vector must
  not move the solve (0 `ForceInjCurr` in r4133/0.15.0b4; the central force-branch
  was removed). Discriminating — honoring the force would shift the stiff `src`
  (|Z|≈0.051 Ω) by ≈255 V; the inert re-solve drifts only by the fixpoint floor
  (≈2e-5 V), gated at 1e-2 V.
- **Force sub-fix (b) — time-series set_nominal preamble untested (audit-tests F2,
  low) → deliberate documented non-fix.** The unconditional `set_nominal_*` preamble
  is structurally preserved: the restructure moved the force check INSIDE each
  flag-checking class's `inj_currents` (skipping only `calc_inj_current_array`), so
  the preamble always runs before the skip — verified by source (audit + re-read of
  `load/accessors.rs`). A black-box time-series observable that isolates
  preamble-runs-vs-skipped from the frozen injection is not cleanly separable (the
  channel is the nominal `Yeq`, swamped by the forced injection); the per-class
  code shape is the guarantee. Tracked follow-up if a Yeq-freeze probe is later
  built.
- **>2-conductor `none` compaction untested (audit-code F2, low) → FIXED
  (coverage).** Added `line_none_conductor_compaction_over_two_conductors_matches_
  explicit`: the audit's exact flagged case (4-conductor spacing, `wires=(w w w
  none)`) is compacted (skip-NIL copy, actualNConds 4→3, original-position coords)
  and its solve is bit-identical (<1e-6 V) to the explicit 3-conductor spacing at
  the same phase coordinates — whose path is oracle-validated by
  `line_spacing_specified_resolves_and_solves`. Confirms the larger-compaction path
  the port's FReduce-based sizing handles without the literal Pascal `NPhases :=
  pGeo.Nconds` reestablishment.
- **Two `TODO(compat)` deleted in `parse.rs` outside Stage F (audit-code F1, low)
  → deliberate non-fix, convention exception intended.** Brief item 5 authorized it
  and it is r4133-justified: the removed sites reproduced the capi015-ONLY
  `TProxyClass.GetDSSClass` case-bug that r4133 does not have (r4133 parses
  `conductors=` natively via `LowerCase` dispatch and solves). Removing a
  `TODO(compat)` whose behavior is being **corrected toward r4133** (not a precision
  improvement, no golden — HIDE_015X) is the right move; the Stage-F rule guards
  golden-pinned precision compat, not a capi015-only breakage being fixed.
