# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

> **Full history archived (2026-08-05).** This file was 16 368 lines; every
> historical session record was moved **verbatim** into `docs/phase-records/`.
> New this round: [`golden-rebase.md`](docs/phase-records/golden-rebase.md),
> [`depascalize-stagef.md`](docs/phase-records/depascalize-stagef.md),
> [`depascalize-w3.md`](docs/phase-records/depascalize-w3.md),
> [`depascalize-r2b-r3.md`](docs/phase-records/depascalize-r2b-r3.md),
> [`depascalize-r1-r2.md`](docs/phase-records/depascalize-r1-r2.md),
> [`depascalize-p-series.md`](docs/phase-records/depascalize-p-series.md),
> [`unified-gate.md`](docs/phase-records/unified-gate.md),
> [`epri-bridge.md`](docs/phase-records/epri-bridge.md),
> [`wasm-usermodels.md`](docs/phase-records/wasm-usermodels.md),
> [`orphaned-gaps.md`](docs/phase-records/orphaned-gaps.md),
> [`bug-wps.md`](docs/phase-records/bug-wps.md),
> [`adopt-015x.md`](docs/phase-records/adopt-015x.md),
> [`era-summaries.md`](docs/phase-records/era-summaries.md),
> [`gate-history.md`](docs/phase-records/gate-history.md),
> [`design-decisions.md`](docs/phase-records/design-decisions.md),
> [`phase-index.md`](docs/phase-records/phase-index.md). Appended to:
> `depascalize-p1.md`, `phase-7.md`, `upgrade-rung1.md`. The pre-existing
> `phase-3..8`, `gaps`, `corpus-rounds`, `part2-adiakoptics`,
> `final-acceptance`, `test-triage-*` records are unchanged.

## 1. Where we are

**Era.** Post-acceptance. The 1:1 behavioral port is finished and
referee-certified (2026-07-11); `PORTING_PLAN.md` is history. The order of all
active work is `PLAN_SEQUENCE.md`; the binding conventions and the five-command
gate are `CLAUDE.md`. The 2026-08-02 user policy is in force: **EPRI r4133 is
the behavioral authority, the pinned dss_capi 0.14.5 is a numeric oracle only,
and upstream bugs are never reproduced in any lane** — the `oracle-parity` lane
has shrunk to a precision-compat lane and is scheduled for full teardown.

**In flight.** `R4133_PROPS_PLAN.md` on branch **`r4133-props`** (forked
from `update` @ `2ee6bb00`) — **WP-RP0 COMPLETE**: RP0.1 vendored the G1.1
census to `tests/corpus/props_r4133/`, RP0.2 made re-measurement a permanent
knob (`DSS_PROPS_CENSUS=1` on the corpus gate; both channels, masks bypassed,
collect-don't-panic, `tmp/props_census.json` + the RP0.1 extracts, asserts
nothing). The knob reproduces the vendored rows row-for-row on the spot-checked
`modes:windgen` family and, over the full 438-case re-census, on 209 of 210
structural pairs and all 94 numeric ones. Its residuals are recorded as
findings against the frozen evidence (the data files are not rewritten): a
missing `regcontrol.fwdthreshold` echo pair (888 cells — RP2.3 provisioning
grows to bin 5 = 45 pairs, example rows via RP2.1's `examples_supplement.txt`),
two whole-element gaps on the cursor-disagreement transformers (+22 cells), and
an ASLR'd address inside five `oracle_error` texts; the closing docs pass
amended the plan (§1.1/§1.2/RP0.2/RP2.1/RP2.3) and the vendored README to the
re-measured reality. **WP-RP1 (property-table shape closure) is COMPLETE**: RP1.1
landed the three r4133 upstream-stub rows (Generator `Rneut`/`Xneut` at display
slots 16/17, Sensor `Action` at 13) behind a new `PropFlags::UPSTREAM_STUB` —
store the parse string, log the class's soft message if it has one, never a
parse error, no engine state — so the two shape gaps close (generator 48 → 50
names, sensor 15 → 16). Measured on a generator-heavy `both` case: r4133 shape
classes 1 → 0 and 175 previously uncomparable cells now compare, with the capi
channel bit-unchanged; the `props/` goldens provably do not move (the capture
enumerates the 0.14.5 oracle's own names) and the only bytes that did are the
two predicted `json/` schema artifacts. Its audit round settled **10** findings
(3 major), two of them by measurement: a full post-fix re-census (438 cases)
names the **12 new value pairs** the shape closure makes live — evidence the
frozen extracts structurally cannot hold, now a per-sub-step obligation of
WP-RP1 and the input of RP2.1's `examples_supplement.txt`, one of them a new
bin-7 pair (`generator.d`, an r4133 `Dpu` echo); and the re-armed `HIDE_R4133`
escape got its owner (`ORPHANED_GAPS.md` §2 → GOLDEN_REBASE G3.3c/G3.4) with a
measured blast radius — 4 artifacts over its three carriers then, re-measured to
8 when RP1.2 added the fourth — and zero corpus cases throughout.
**RP1.2** then ported AutoTrans `XfmrCode` for real (r4133 property 39 +
`TAutoTransObj.FetchXfmrCode`): the auto now resolves a library entry and copies
its electrical model field by field, with the autotrans-specific overrides
(winding 1 forced SERIES / winding 2 WYE, `XHL/XHT/XLT → puXHX/puXHT/puXXT`,
`RdcSpecified` on every winding), so the third shape gap closes (52 → 53 names,
r4133 shape classes 3 → 2). Two r4133 statements in that arm are upstream bugs
and are not reproduced: the contradictory `'XFmrCode Property not used with
AutoTrans object.'` (#100131, fired after the code was applied — and measured to
overwrite the real #100180 on the API errno surface) and `NConds := Fnphases + 1`
(the Transformer's conductor rule, which leaves r4133 solving a structurally
wrong 3-phase auto: measured 4 conductors and 92 236 V on a 69 kV winding vs 6
and 38 472 V). Both got upstream reports. Measure-first put a new corpus deck on
the r4133 channel — deliberately **single-phase**, the one width at which the
`NConds` bug is inert (1+1 = 2·1), so the channel really does gate the copy — and
it passes with no ledger entry; the multi-phase form is pinned in-engine
(12 tests, incl. a coded-vs-longhand auto whose Y and node voltages are
bit-identical). The full re-census records the **9** value pairs the closure makes
live, one of them a genuine bin-7 jump (`autotrans.wdgcurrents`) that sits
entirely on capi-only cases and is therefore out of RP4.1's scope. Its audit
round settled **8** findings (1 major), the major one by re-measurement: the
fourth `HIDE_R4133` carrier is the first on a class with committed `Dump`
goldens, so the flag's un-hide blast radius is **8** artifacts and +7
`dump3_commands` rows, not the RP1.1-era 4 and +6 (corpus still untouched).
**RP1.3** then ported WindGen `UserModel`/`UserData` (r4133 properties 18/19) and
the `Model=6` behavior they feed, over the sandboxed WASM host: a WindGen-shaped
shuttle in `dss-usermodel` (the `TWindGenVars` image, measured on FPC and laid
out as a byte-for-byte extension of the Generator record), a committed guest
fixture (`tests/fixtures/wasm/wgturbine.wasm`, reproduced bit-identically from a
wiped target dir), the `WindGenUserModelSlot`, all five model-6 arms (power flow
#567, dynamics #5671 + abort, `InitStateVars`, `IntegrateStates`,
`RecalcElementData`'s `FUpdateModel`) and the model enum widened to 6 — so the
fourth shape gap closes (46 → 48 names, r4133 shape classes **2 → 1**, leaving
only RP1.4's reverse `gendispatcher` row). Reaching model 6 also closed two
substrate gaps the port had carried: the `Xd/Xdp/Xdpp` + `puX*` family (and with
it the record `Zthev` every model's `InitStateVars` should have used) and
`SetNominalGeneration`'s model-6 `Yeq` arm. Two **real engine defects** the
probes found were fixed in the same sub-step — `like=` produced a *dead* user
model with no diagnostic (the slot's `Clone` drops the wasmi instance and every
call site guards on `exists()`), and the two-phase dynamics abort carried no
error number — and an r4133 bug (the `Get_/Set_Variable` user-model tail nested
outside the `else`, so a loaded model overwrites all 22 native variables) is
reported, not reproduced. WindGen has no capi channel at all, so the model-6
numbers are pinned in-engine against the fixture's own documented law (23 tests
after the audit round, each expected value re-derived from the engine's solved
voltages, a 13-row measured-mutation table proving non-vacuity) plus 7 shuttle
tests in `dss-usermodel`. Dormancy is
**proven, not argued**: with no user model bound the whole corpus is
byte-identical in BOTH lanes (`lane_diff.ps1` PASS, 522 cases / 3 220 247
records, `max |Δ| = 0`, dumps byte-equal to the pre-edit baselines). The full
re-census records the **2** value pairs the closure makes live, both in covered
bins and no genuine jump.
**RP1.4** then closed the fifth and last shape row — the only one that runs
backwards — with **zero engine change**: `weights` exists in the port and in
dss_capi, and r4133 loses the NAME to a registration off-by-one
(`NumPropsThisClass = 6` against `PropertyName^[7] := 'Weights'`, so
`TCktElementClass.DefineProperties` overwrites slot 7 with `basefreq`). Measured
on the DLL: `AllPropertyNames` returns 9 names without `weights`, `weights=` is
error #364, `basefreq=[3, 1]` parses **as the weights vector**, and a plain
`basefreq=60` silently sets `FWeights := [60, 0]` (one generator absorbs the
whole redispatch) — reported upstream, fix `NumPropsThisClass = 7`. The
`("GenDispatcher", &["weights"])` row joins `PROPS_015X`, inert on the capi
channel (whose list has the name) and active on r4133 only. **The liveness
question was settled by measurement, not left open**: the census's
`generator.kw/kvar` deltas root-cause entirely to the bug — r4133 rejects the
decks' `weights=[3, 1]` and splits equally (kW 48 % from the port at the worst
of steps 1-11 and 54.5x at step 0, the census's `generator.kw` `max_rel`
5.45e+01), and feeding it the same vector as `basefreq=[3, 1]` reproduces the port's
split to the last displayed digit — so the divergence is a whole-solution one
that no `property` pin could cover, the three decks stay `engines: "capi_v0145"`
and the row is documented *dormant until a gendispatcher deck gates r4133*. The
re-census makes r4133 shape classes **1 → 0** (full census **429 → 0**) with one
new pair (`gendispatcher.enabled`, bin 1, all cells out of scope), the capi
channel byte-identical and no golden, lock or manifest byte moved — so **WP-RP1
is COMPLETE** and its whole-WP acceptance criterion is met.
**WP-RP2 is open and RP2.1 (channel threading + the normalization engine +
replay accounting) is COMPLETE** (2026-08-23, one commit, **zero engine change**
— harness/tests/docs only): the property comparator now knows which oracle it is
talking to (`PropsPolicy`, threaded through both walks), all 12 `SKIP_PROPS`
rows carry a measured r4133 disposition, and a typed 157-row table
(`tests/harness/props_norm.rs`, `BoolFold`/`CaseFold`/`ArrayForm`/`EnumSynonym`)
re-spells — never re-values — the r4133 side of bins 1/2/4. Proven three ways:
offline by the new replay (`props_r4133_replay.rs`, 3 454 example rows, 0
unaccounted, every table row live), per cell by the census knob's new
disposition mode (`DSS_PROPS_CENSUS=claims`: **488 703 in-scope cells claimed**,
549 933 unclaimed and every one of them declared to RP2.2/RP2.3/RP2.4/RP3), and
non-vacuously by scratch probes where a corrupted boolean, a corrupted array
token and a 1e-2 number all still fail through the normalized path. Capi
invariance is now **structural on both seams** — the live comparator and the
census's disposition query each take the channel — and measured: the bounded A/B
gate is identical and the capi census extracts byte-identical (the r4133 ones
gain exactly the pair RP2.1's own `RevThreshold` unmask makes visible). Its two
audits raised 9 findings (1 major, 8 minor), **all 9 fixed** in a follow-up
commit — the major was "the rules' accepted sets are not pinned", closed by
closed-set pins whose power was re-verified against the auditor's own surviving
mutations.
**RP2.2 (enum synonyms + the S6 dossier) is COMPLETE** (2026-08-23, one commit,
**zero engine change** — harness/tests/docs only): all 24 pairs of its closed
list, plus 3 the accounting surfaced and 2 hand-offs RP2.1 left, were read off
the r4133 source and classified with a citation each; the kill criterion did not
fire. Bin 3 split **4/4** — `vsource`/`isource` × `scantype`/`sequence` are true
synonyms and took the table's first `EnumSynonym` rows, while `swtcontrol.action`
(the plan's first suspected divergence, **disproven**: an `EchoParse` stored
before the CASE and left stale by the `Locked` refusal, with the pair's own live
`state` getter witnessing that both engines agree), `monitor.mode` (not a
"decomposition render" but the deck's RPN **source text** `mode=(1 16 +)` echoed
back) and `storagecontroller.modedischarge` (`'UNKNOWN'` is `GetModeString`'s
non-injective catch-all) went to RP2.3. **Three RP3.5+ sub-steps opened and they
block RP4.1** (§0): RP3.5 `line.units` (the plan's second candidate,
**confirmed** — the port's matrix-branch merge writes the saved units and *then*
re-runs the side effects that reset them, where r4133 orders the two the other
way), RP3.6 `line.linecode` (r4133's `switch=yes` arm leaves `FLineCodeSpecified`
TRUE; 5 cells, **all in scope**, so RP4.1 breaks without it) and RP3.7 the
per-phase switch/relay state (r4133 keeps a `pStateArray` per phase; the port one
scalar). Closing bin 3 meant closing its **cells**, not only its pairs: three
bin-2-labelled pairs carry enum-spelling cells, so `capcontrol.type` and
`fault.bus2` joined RP2.3 and `invcontrol.voltage_curvex_ref` — a *live* r4133
enum getter — was re-typed from `CaseFold` to a fifth `EnumSynonym` row. The
replay now claims **755** example rows (0 unaccounted, RP2.2's bucket empty) and
the full claims census measures **514 471 cells / 492 376 in scope** claimed
(`EnumSynonym` 4 619 / 3 817), unclaimed down to 545 568 / 521 841 and bin 3 from
8 pairs to 4. Two dossier findings corrected part A's own reading — r4133's
`MakeLike` *does* copy `ScanType`/`SequenceType` on both classes, and the two
registries are **not** the same list (`ScanType`'s −1 is `None`, `Sequence`'s is
`Negative`), which is why the rows carry two maps and not one. **RP2.3** (the
echo-exclusion table + pins) is next, but the three RP3.5+ sub-steps are what
RP4.1 waits on.
Alongside it, `GOLDEN_REBASE_PLAN.md` WP-G1 on branch **`golden-g1`** (forked
from `update` @ `4d3fc2d7`). WP-G0 (safety rails) and WP-G2 (bug-kernel
teardown) are COMPLETE and merged to `update` (`6e7ee691` / `77e1799a` /
`4d3fc2d7`, all pushed): G2.0 rails + G2.1a…G2.1h + G2.2a–d + G2.3 + G2.4 +
G2.5 + G2.6 landed, `SPLIT_ALIAS_POPULATION` **31 → 11**, `Escape::WholeCase`
**4 → 1**, zero golden bytes moved over the whole WP, and none of the six
CLAUDE.md §"Known upstream bugs" is reproduced in any lane (the eleven
surviving split rows are the WP's fixed point: five numeric precision rows +
six rendering rows, the latter WP-G4's scope).

WP-G1 (live gate to fastdss parity) opened 2026-08-08. **G1.1 fired its plan
kill criterion** (plan: ">~15 new ledger entries, or any entry that cannot
be pinned → the r4133 property surface diverges materially; needs its own
plan"): the unmask was implemented scratch-style and measured by a full-cell
census of the exact gate comparison — **433 of the 438 walked live cases
diverge on r4133 properties** (the walk's population, measured by RP0.2; the
kill-criterion text originally said "~512"): 209 structural (class,prop) pairs (960 129 cells —
rendering-convention deltas: case preservation vs lowercase, boolean wording,
array/empty formats, enum spelling, display-default strings — not
ledgerable/pinnable at all), 94 numeric pairs (95 317 cells — mostly Delphi
`%-.5g`/`%-.6g` display precision; a tail of genuine value jumps needing
root-cause, some possibly port bugs to FIX rather than ledger), 5
property-table shape gaps (Generator `Rneut`/`Xneut`, Sensor `action`,
AutoTrans `XfmrCode`, WindGen `UserModel`/`UserData` missing from the port;
GenDispatcher `weights` the one reverse row). Nothing committed; the capi
channel is unaffected; full census persisted at
`investigations/g1_1_r4133_props/` (local-only). Those are the numbers of the
2026-08-08 walk as measured then; RP0.2's re-census puts the structural side at
**210 pairs / 961 031 cells** (see its record for the three findings that
explain the delta) — the shape and numeric counts are unchanged. **Resolved 2026-08-22 (user
decision): the dedicated plan is authored — `R4133_PROPS_PLAN.md`** (WP-RP0–RP5;
adversarially verified against the repo + census, all findings settled
in-text; hardened same day by a second round — two independent re-verifiers, a
commit-integration audit and an executor-followability audit: 2 majors fixed
in-plan — the replay contract went per-(rust,r4133)-spelling because bin-1
pairs mix BoolFold cells with echo cells (that round named three; the RP0.1
evidence measures **nine** echo-carrying bin-1 pairs, four of them mixed — see
the RP0.1 record below), and RP2.1 gained the
row-by-row r4133 disposition of the channel-blind `SKIP_PROPS`/`LANE_SKIP_PROPS`
— plus a `bins.tsv` RP0.1 artifact, the harness-local channel type note
(`EngineChannel` is `pub(crate)` to corpus_gate), NCIM/probe/fixture pointers,
and ~15 citation/ordering minors). G1.1 is handed to it: RP4.1 delivers the unmask with the kill
criterion re-armed; GOLDEN_REBASE G3.4/G3.5 wait on RP4.1; PLAN_SEQUENCE rows
5a/5b added the same day. Execution of that plan started 2026-08-22 on
`r4133-props` (records below); the local-only census is no longer the single
copy of the evidence — its extracts are vendored by RP0.1 and the whole census
is re-derivable in ~1 min by RP0.2's `DSS_PROPS_CENSUS=1`. G1.2 (ESPVLControl
deck) and G1.3d (discrete extras)
run on — independent of G1.1. Queued behind GOLDEN_REBASE: `WASM_USERMODELS`
follow-ups, RESONANCE, MULTITHREADING, the UPGRADE line.

**Sequenced after / parked.** DIAKOPTICS Part II WP-AD.6 (threaded children,
needs MULTITHREADING M2); the IEEE118Bus NCIM switching-cadence rung; the
`UpgradeRung` escape rows below. Details in *Standing open follow-ups*.

**2026-08-05 — STATUS.md archived.** 16 368 → 415 lines: every historical
record moved verbatim into `docs/phase-records/` (see the pointer above); the
GOLDEN_REBASE records below are condensed here and kept in full in
`golden-rebase.md`. Docs-only commit, no behavior change — no test parses this
file (`oracle_parity_cfg_gate.rs::operational_docs` deliberately excludes it).

### GOLDEN_REBASE WP-G0 / WP-G2 — condensed records

> Full session records (bug analysis, Pascal citations, pin inventories, audit
> settlements) → [`docs/phase-records/golden-rebase.md`](docs/phase-records/golden-rebase.md).
> Every step below ran the full five-command gate green before its commit, and
> every one was audited by two fresh opus auditors plus a fix agent on the same
> branch.

- **G0.1** (`5261266c`, fix `9de32e82`, branch `golden-g0`, 2026-08-02) — the
  provenance lock `tests/golden/golden.lock.json`: 737 rows (`path, sha256,
  anchor, reason, produced_by`), anchors 700 `capi_v0145` / 11 `capi015` / 11
  `r4133` / 10 `r3723` / 1 `fpc_3.2.2` / 4 `self`, enforced fail-on-stale in both
  directions by the new `golden_lock.rs`. No golden byte moved. Audit: 12
  findings, all settled, 0 digests moved.
- **G0.2** (`1a841cb1`, fix `0cad83dc`, 2026-08-02; WP-G0 merged to `update` as
  `34baa5b7`) — `harness::regen()` + `snapshot_text/bytes()`, the guarded writer
  behind `DSS_UPDATE_GOLDENS`: refuses `ExternallyAnchored`, `WrongLane`,
  `Unlocked` and `NoProducingLane`. Still no golden byte moved. Audit: 11
  findings, all settled.
- **G2.0** (`ef1ba28c`, fix `5d712272`, branch `golden-g2`, 2026-08-02) — WP-G2
  rails only, no kernel touched: the TESTING.md re-anchor onto the five numeric
  survivors (`compat::PI`, `round_f64`, `round_i32`, `kv_base_search_scale`,
  `profile_ll_pu_divisor`), the `TORN_DOWN_ROWS` register with its six rot checks
  and two census ties, and the `// LANE-EXCLUSION(<row>)` markers. `lane_diff.ps1`
  re-run: max |Δ| = 0. Audit: 9 findings (2 major), all fixed.
- **G2.1a** (`2d3b3d11`, fix `0e89651f`, 2026-08-03) — `stddev_single_point`
  deleted: a one-element sample has no spread, so all four `support::mathutil`
  entry points return `0.0` (r4133 `mathutil.pas:405/:429`, 0.14.5 ×4). Only the
  parity lane moves. 31 → 30. Audit: 6 minor findings, settled.
- **G2.1b** (`20179793`, docs `b7e5c867`, fix `add7e0ef`, 2026-08-03) —
  `CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL` deleted: `make_like` copies
  `control_signal_name` + `ctrl_signal_shape` unconditionally. 30 → 29. Audit: 3
  minor, all fixed.
- **G2.1c** (`91df2a25`, fix `04d845ab`, 2026-08-03) —
  `SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING` deleted: one `pct_of_rating`
  closure returns `0.0` for a non-positive rating (an undefined rating is not a
  percentage). The same loop's `IRESIDUAL_FROM_TERMINAL_1` is untouched — G2.2a
  owns it. 29 → 28. Audit: 4 minor, all fixed.
- **G2.1d** (`1c7f0501`, fix `68f5dade`, 2026-08-03) —
  `REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT` deleted: `red_short_line_step` scans
  the parent's whole shunt list with the same predicate the merge-with-child
  branch always used. 28 → 27. Audit: 3 findings (1 filed major), settled with a
  non-vacuity input the fix agent added.
- **G2.1e** (`87a64820`, fix `9b5da5a6`, 2026-08-03) —
  `STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL` deleted:
  `fleet_needs_idling` is one unconditional `fleet_state != StorageState::Idling`.
  27 → 26. Audit: 4 minor, three fixed as stated.
- **G2.1f** (`f990dbed`, fix `08131c08`, 2026-08-03) —
  `STORAGE_MULTIFILE_USES_THE_PV_PREFIX` deleted: the `RegKind::Storage` arm of
  `gather_register_rows` returns `"EXP_STORAGE_"` plainly. 26 → 25. Audit: 5
  minor, one refuted with a worked counter-example.
- **G2.1g** (`359a2ca3`, fix `6f178b93`, 2026-08-04) —
  `CIM_WYE_GROUNDED_IS_HARDCODED_TRUE` deleted: both `cim/export.rs` writers read
  the neutral (capacitor `term2_nodes.all(== 0)`, load `neutral_node == 0`) — the
  same test the unit's own transformer writer applies. 25 → 24. Audit: 3 minor,
  fixed with a mixed-grounding deck added to the pin.
- **G2.1h** (`a7b42939`, fix `848802dc`, 2026-08-05) —
  `HEIGHT_UNIT_CHANGE_REREADS_THE_METRES_FIELD` deleted, the **last of the eight
  zero-footprint G2.1 rows**: `set_user_height_unit` reads `height_offset()`
  before the unit field moves, so a typed height is not converted twice (r4133
  `LineConstants.pas:689-696`; the surface does not exist in 0.14.5, so the row is
  cited and gated on `r4133` only). 24 → 23. Audit: both findings upheld,
  docs-only.
- **G2.2a** (2026-08-05) — the first two rows whose teardown a **golden compare
  observes**, both dismantled by making an existing exclusion unconditional
  rather than by moving a golden byte. `IRESIDUAL_FROM_TERMINAL_1`:
  `CalcAndWriteSeqCurrents` applies the `(j-1)*Ncond` offset to its symmetric
  components (r4133 `Version8/Source/Common/ExportResults.pas:323`) but not to
  the residual sum (`:365-366`, dss_capi `:422-424`), so every terminal row
  repeats terminal 1's residual; both lanes now sum the row's own terminal, and
  the golden's `Terminal >= 2` `Iresidual` cells are excluded in **both** lanes
  and pinned by `export_seqcurrents_iresidual_sums_the_rows_own_terminal`
  (derived from `Export Currents`' `Iresid_j`, an independent anchor).
  `BUS_INT_DURATION_WALKS_ALL_BUSES`: `CalcReliabilityIndices` sizes
  `FeederSections` to its own zone (r4133 `Meters/EnergyMeter.pas:2507`) but
  writes bus durations while walking every circuit bus (`:2567-2574`), and the
  section-id zeroing that would clear a foreign id is itself per-zone (`:2472`),
  so with two meters the later one overwrites the earlier one's durations; both
  lanes now walk only their own zone, the `Duration` column of
  `export_busreliability_multimeter` is masked in both lanes and pinned
  literally by `export_busreliability_multimeter_duration_stays_in_the_meters_zone`
  (`B1 = 4`, `B2 = 5`, not upstream's 6/9). 23 → 21; both rows carry
  `Evidence::Exclusion`, the first use of that variant. The pin-walk
  non-vacuity anchor for the integration-test shape moved off the torn-down
  `IRESIDUAL_FROM_TERMINAL_1` onto the numeric survivor `PI`
  (`crates/dss-parser/tests/parser_golden.rs`), which outlives WP-G2 and WP-G4.
  Doc strikes in the same commit: CLAUDE.md's two bug bullets and its WP-G2
  status line, `tests/TOLERANCE_NOTES.md`'s `Iresidual` note (rewritten in
  place — it is a note about the `SeqCurrents` compare policy, and the
  "deliberately-reproduced" section it might otherwise move to is about
  reproductions, which this no longer is). No ledger or `population.lock.json`
  movement: no live gate reads `Bus.Int_Duration` or report text yet (WP-G1's
  G1.6 adds the reliability columns, which is why the plan orders this row
  first), and the corpus gate stayed green in both lanes.
  Audit: 6 minor, **all upheld and fixed** (docs plus one rail), no engine code
  touched. Five were doc-rot the teardown left behind — `run_deck_export_capture`
  and `GateSpec::Mask` still described their pre-G2.2a roles, the
  `branches_on_lane` census still said `golden_reports.rs` ×15 (now ×9), the
  `Evidence` `expect(dead_code)` note still claimed only `Site` is constructed,
  and `GOLDEN_REBASE_PLAN.md`'s G5.1 list still promised a TOLERANCE_NOTES
  *move* that was a rewrite-in-place (both plan lines now say so, so G5.1 does
  not chase it). The sixth was real coverage: the re-anchored pin-walk
  non-vacuity const keys on the bare token `PI`, which
  `parser_golden.rs`'s incidental `f64::consts::PI` satisfies, so deleting the
  deliberate `compat::PI` citation left the anchor green on a std-library
  homonym — reproduced, then closed by re-checking the **qualified**
  `compat::<alias>` spelling in the region the walk credited (the probe now
  fails on that assert alone). `lane_diff.ps1` not re-run: the fix touches no
  compat kernel, lane alias or solver.
- **G2.2b** (2026-08-05) — the two **property**-exclusion rows.
  `monitor_base_frequency`: `TMonitorObj.Create` re-assigns `Basefrequency :=
  60.0` after the inherited `TDSSCktElement.Create` already wrote
  `ActiveCircuit.Fundamental` (`Monitor.pas:472` == r4133 `:552`;
  `CktElement.pas:203`) — left-over, not meant, since `Line.pas:974` /
  `GICLine.pas:373` carry the same statement commented out with "set in base
  class" and EnergyMeter/Sensor never write the field. Its one physical consumer
  is mode-4 flicker (`Monitor.pas:1657` → `Pstcalc.pas:594`, where `fBase = 50`
  picks the IEC 61000-4-15 230 V/50 Hz lamp weighting, `:609-626`).
  `create_object_no_edit` now seeds `base_frequency = fundamental` for every
  element with no Monitor arm at all; `harness::skip_prop`'s `LANE_SKIP_PROPS`
  consultation is unconditional, and the pin was renamed to
  `monitor_basefreq_inherits_the_fundamental` (inherited 50 on a 50 Hz deck,
  unchanged 60 on a 60 Hz one, explicit `basefreq=` still overrides); the
  kernel-vs-kernel test in `compat/tests.rs` went with the two kernels, and the
  neighbouring `all_elements_inherit_the_50hz_base_frequency` simply gained the
  monitor in its element list instead of a lane branch.
  `ISOURCE_BUS2_NEVER_LATCHES`: `TIsourceObj.PropertySideEffects` has no `bus2`
  case (`Isource.pas:221-262`; r4133 `Isource.pas` declares `Bus2Defined` `:61`,
  copies `:335`, clears `:398` and never sets it), so the `bus1` case's
  `if not Bus2Defined then SetBus(2, S2)` clobbers an explicit `Bus2=` parsed
  first — while `Vsource.pas:498` (r4133 `:468`) and `Capacitor.pas:349` latch
  on that very property. The `BUS2` arm now latches in both lanes;
  `props_roundtrip.rs`'s `LANE_SKIP_SCENARIO_PROPS` is unconditional (its
  `lane_skips` assert is now a plain equality against the list length, in both
  lanes) and the pin is `bus2_latches_like_the_sibling_class`. 21 → 19, both
  rows `Evidence::Exclusion`. **`LANE_SKIP_PROP_VALUE_CELLS = 33` did not move**
  — it is the unrelated both-lane sym-matrix exclusion, and the plan calls a
  movement there a finding, not a re-measurement. Doc strikes in the same
  commit: CLAUDE.md's Monitor bullet, its WP-G2 status line and its
  "documented inline" sentence (the row has had `investigations/
  issue-06-monitor-basefrequency-60.md` for a while), and
  `tools/golden/gen_props.py`'s `isource_full` KEEP-THIS-ORDER note — the
  capture still needs the ordering, the port no longer does. No golden byte, no
  ledger and no `population.lock.json` movement; the corpus gate stayed green in
  both lanes, which is the classification check for both rows (the 50 Hz
  `LVTestCase` monitors and the props `Bus2` cell are the only observables).
  `lane_diff.ps1`: max |Δ| = 0.
- **G2.2b fix** (2026-08-05) — three comment corrections, no code: the
  `branches_on_lane` doc's surviving-read citation (`harness/mod.rs:2374`, and
  `skip_prop`'s was the *first* of that file's two reads, not the second);
  `compat.rs`'s "they **are** the two reproduced bugs" prose, now past-tense and
  keyed by row *name* rather than by an ordinal that goes stale every teardown;
  and `props_roundtrip.rs`'s `assert_shape_matches` header, which still said
  "used in the default lane" of an exclusion its own list doc and call site call
  both-lane. **Refuted, not fixed:** the audit claim that `LANE_SKIP_PROPS` is
  unreachable inside `cargo test` — that `compare_all_properties` is opt-in per
  case and only `corpus_live_properties` (`DSS_LIVE_PROPS=1`) turns it on. It
  misses `corpus_gate/scheduler.rs:102-110 force_properties`, which sets the flag
  in code for **every** live `solvable_now` case that gates capi and is not
  `kind: large` — the manifest never needs the key. `LVTestCase/Master.dss` is
  exactly that (`engines: both`, `kind: feeder`, no ledger entry), so the compare
  runs in the mandatory gate. Measured by emptying the list and re-running the
  gated case: `Monitor.line558_vi_vs_time property BaseFreq: actual 50 vs
  expected 60` on the CapiV0145 channel. The exclusion is load-bearing, and its
  going inert is itself loud (that red), so the fail-on-stale counter the finding
  proposed as a safety net would be measuring a failure mode the gate already
  reports. `lane_diff.ps1` not re-run: comments only — no compat kernel, lane
  alias or solver touched.
- **G2.2c** (2026-08-05) — the three **text-transform** rows, all dismantled by
  making an existing oracle-text rewrite unconditional. `FAULT_DUMP_TAIL_
  REPRINTS_MINAMPS`: `TFaultObj.DumpProperties` runs its generic tail from
  `NumPropsThisClass`, which this class defines as `Ord(High(TProp))` = 9 =
  `MinAmps` itself (`Fault.pas:533` with `:134`; r4133 `PDElements/Fault.pas:594`
  with `Const NumPropsthisclass = 9` `:107`), so the loop's first iteration
  reprints the property the custom `~ MinAmps=%.1f` line just wrote — the pair
  `~ MinAmps=3.0` / `~ MinAmps=3`. Every sibling class with that loop writes
  `NumPropsThisClass + 1` (`Transformer.pas:1276`, `AutoTrans.pas:1307`,
  `XfmrCode.pas:663`), so both lanes now start at `NormAmps`;
  `golden_reports::fault_dump_expected` drops the second line of each pair from
  the oracle text in both lanes, and `fault_dump_goldens_carry_the_double_print`
  (the row's pin, now unconditional) holds the four goldens it is applied to to
  one `~ MinAmps=` per Fault, always the custom `%.1f` render.
  `CIM_DELTA_SHUNT_GROUNDED_USES_LINEAR_PREFIX`: one `if` in the CIM
  shunt-compensator writer emits `grounded` under two class prefixes —
  `ShuntCompensator.` for a wye bank (`ExportCIMXML.pas:3700`; r4133 `:3183`) and
  `LinearShuntCompensator.` for a delta one six lines below (`:3706`; r4133
  `:3187`) — and CIM100 declares the property on `ShuntCompensator`, so the delta
  spelling resolves against no class at all. `CIM_ACLINESEGMENT_G0CH_WRITTEN_AS_
  B0CH`: the symmetrical-components line writer closes its `bch`/`gch`/`b0ch`/
  `g0ch` quartet with a second `ACLineSegment.b0ch` (`:4367` after `:4366`; r4133
  `:3756` after `:3755`), a copied line whose value was replaced and whose name
  was not, leaving the segment with no `g0ch` and two contradictory `b0ch` nodes;
  the `PerLengthSequenceImpedance` sibling of the same procedure (`:4521-4522`;
  r4133 `:3895-3896`) spells the quartet correctly. Both lanes now write
  `ShuntCompensator.grounded` and `ACLineSegment.g0ch`; `golden_cim`'s rewrite —
  renamed `lane_expected_cim` → `expected_cim`, and its pin
  `cim_lane_divergences_are_pinned` → `cim_writer_divergences_are_pinned`, since
  neither reads the lane any more — applies both renames to the oracle text in
  both lanes, and the pin gained two assertions the lane branch used to make
  redundant: the expectation is the same length as the oracle and differs from it
  in exactly the counted lines. 19 → 16. Evidence variants: `Site` for the Fault
  row (the engine's `generic_props_from(…, prop::NORMAMPS)`) and for the `g0ch`
  row (`"ACLineSegment.g0ch"` is unique inside the keyed `cim/export.rs`, and
  the needle is the whole one-line call, which the split form could not be);
  `Exclusion` for
  the delta-prefix row, whose engine half a needle **cannot** discriminate — the
  fixed delta arm's `"ShuntCompensator.grounded"` line is byte-identical to the
  wye arm's, which the split form also carried, so the recorded anchor is the
  renamed transform and the row's comment names the pin and the CIM byte compares
  as what carries the engine half. No golden byte, no ledger and no
  `population.lock.json` movement; no doc-surface citation exists for any of the
  three (measured — the doc walk finds none), so no strikes. The corpus gate
  stayed green in both lanes (the CIM writers and the Fault dump have no live
  observable — no gated case exports CIM or dumps a Fault). `lane_diff.ps1`:
  **PASS**, max |Δ| = 0 on every gated kind (3 219 862 records, 520 cases), with
  only the two pre-existing Newton decks in the documented-divergence list.
  Audit: 3 minor, all upheld and fixed, comments/docs only — the `g0ch` row's
  `Evidence::Site` justification claimed tree-wide uniqueness for a literal that
  also appears in `golden_cim.rs` and in the needle itself (reworded to the
  claim that carries the check: uniqueness *inside the keyed file* plus the
  one-line call shape the split form could not have; same correction in
  `STATUS.md` and the local registry, which additionally mis-stated the split
  form's extra indentation as four spaces where it was eight); the G2.1g caveat
  at `GOLDEN_REBASE_PLAN.md:629` still named `lane_expected_cim` (annotated
  **as executed: measured, did not fire**, so the row stayed in G2.1g); and two
  stale citations in `branches_on_lane`'s doc comment (`golden_reports.rs:1620`
  → `:1628`; the `reads_the_lane` line number dropped — the rustdoc link
  resolves without one and the number had drifted ~430 lines). `lane_diff.ps1`
  not re-run for the settle: comments and docs only, no compat kernel, lane
  alias or solver touched.
- **G2.2d** (2026-08-05) — the two **event-log** rows, both label-only.
  `RELAY_SAMPLE_TRACE_IGNORES_DEBUGTRACE`: `TRelayObj.Sample` closes its
  `FPresentState` resync with a bare `AppendtoEventLog('Debug Sample: Relay.' +
  Name, 'FPresentState: …')` (r4133 `Controls/Relay.pas:1325`) — no
  `if DebugTrace`, and not gated on `ShowEventLog` either, so every relay writes
  one debug line per control sample into the user-facing log. Exactly one line
  lost that guard: the Recloser's byte-identical line keeps it
  (`Recloser.pas:1044`), so do the class's own sibling traces (`:1822`, `:1845`),
  and r4088 had no such line in `Sample` at all. `RELAY_RESET_EVENT_IS_LABELLED_
  RECLOSER`: both `CTRL_RESET` arms of `TRelayObj.DoPendingAction` log
  `'Recloser.' + Self.Name` (`Relay.pas:1196`, `:1212`), verbatim copies of
  `Recloser.pas:909`/`:924`, while all eight other events of that procedure
  (`:1087`-`:1176`) write `'Relay.' + Self.Name` and both earlier revisions of
  these two lines label them correctly (r4088 `:971`, 0.14.5 `:1003`). Both lanes
  now route the trace through the `Relay::dbg` helper the port already had and
  name the emitting class on the reset. In `harness::lane::expected_eventlog` the
  two Relay rewrites moved **above** the `if PARITY` early return, so they apply
  in both lanes; the `EVENTLOG_REROUNDED` fold and its `REROUND_VISITS`/
  `REROUND_HITS` accounting deliberately stayed **behind** it — that is the
  `compat::fmt_g` precision row, alive until G4.1, and unconditionalizing it
  would hand the parity lane the native `%g` spelling against FPC-spelled engine
  output (a 1e-5 gap vs `compare_eventlog`'s 1e-6/1e-9 floor on
  `controls:invcontrol/midi_invcontrol_drc.dss`). Same mixed shape as
  `golden_json::lane_expected_json`. 16 → 14. Both rows recorded with
  `Evidence::Site` on the engine kernels (`relay/mod.rs`: the `self.dbg(…)` call
  at the trace block's own twelve spaces — the split form had it four deeper in
  an `else` arm, and it is the file's only `self.dbg(` — and the one-line
  `let reset_device = format!("Relay.{}", …)`, which the split form could not
  be), plus the two `LANE-EXCLUSION` markers on the now-unconditional rewrites in
  `harness/lane.rs`. Pins unconditional: `sample_state_trace_follows_debugtrace`
  (renamed from `…_is_the_lane_guard`; still walks all four `DebugTrace` ×
  `ShowEventLog` combinations, so the fix cannot degrade into "the line is gone")
  and `do_pending_reset_only_resets_opcount_d4` (asserts both directions —
  `Element=Relay.r1,` present, `Element=Recloser.r1,` absent). The two harness
  unit tests that guarded the narrowness of the relabel became unconditional too.
  No golden byte moves — no committed golden captures a relay event log
  (measured) — and no ledger or `population.lock.json` field moves; the only
  non-code edit is the `tests/corpus/controls/manifest.json` note, whose
  parenthetical still claimed the compared log includes the `Debug Sample` lines
  (`note` is not fingerprinted by `Case::rigor`). No doc-surface citation exists
  for either row.
  `lane_diff.ps1`: **max |Δ| = 0** on all eight gated kinds (520 cases, 3 219 862
  records; 0 iteration counts drifted, `VERDICT: PASS`) — run at the settle
  because the teardown edited two compat kernels and the implementation commit
  had not recorded one. The only entries under "documented divergences" are the
  two `modes:newton/` decks that G2.3 owns; no relay deck contributes, as
  expected — `lane_dump` compares solved state, and both rows are event-log
  labels.
  **OPEN — one unexplained `corpus_gate` failure, not dismissed as a flake.**
  During the settle, one default-lane `cargo test --workspace` failed at
  `corpus_gate.rs:127` (the "N of M case(s) failed" panic); the failing case's
  identity was lost to the output filter and **has not been reproduced** in seven
  subsequent full runs (three default `--workspace`, one parity `--workspace`,
  and a dedicated `--test corpus_gate` loop), so it is recorded here rather than
  closed. What is established: it cannot originate in the settle commit — every
  changed line under `crates/` there is a comment (`git diff 35ab18ee..f385d094
  -- crates/` filtered of comment and blank lines is empty), so the built engine
  and test binaries are behaviourally identical to the implementation commit,
  whose own five-command gate was green. The suspected area is corpus-gate
  infrastructure, not the engine: the scheduler runs cases in parallel across
  directories several decks share, and `corpus_gate/runner.rs:42-55` already
  documents nondeterministic pollution of exactly this kind ("reproduced on two
  full `cargo test --workspace` runs, a different file set each time"). The same
  signature was observed live here — the untracked artifact set left under
  `tests/corpus/electricdss-tst/Test/AutoTrans/` differed run to run (15, then 9,
  then 14 files), and that folder's decks emit export names that collide
  case-insensitively on NTFS (`Auto3bus_noload_power.txt` from the positional
  `export powers kva …` form vs the `file=`-form spelling). Next step when this
  is picked up: re-run with `DSS_GATE_JOBS=1` to test the parallelism hypothesis,
  and capture the full panic body rather than a filtered tail. Refuted along the
  way: that `lane_diff.ps1`'s artifact cleanup removed a deck input — the
  `LineConstantsCode.dss` files it deletes are pure `Show` output with no
  `Redirect` consumer anywhere in the corpus.
  **Audit settlement** (2026-08-05, three minor findings, all real, all prose —
  no engine behavior changed): (1) the blast-radius
  enumeration "the 15 gated `oracle: \"r4133\"` cases that carry a relay and
  compare an event log" undercounted by two. Re-measured over the family
  manifests' `compare_eventlog` + `engines` fields and confirmed against
  `population.lock.json`, the set is **17** — nine `controls/relay/`, two
  `controls/combo/`, **two `controls/fuse/indmach_r4133/`** (both instantiate
  `Relay.mfrov/uv`, `Relay.mfr46`, `Relay.mfr47`), four TD21 decks. Corrected in
  `harness/lane.rs` and `oracle_parity_cfg_gate.rs` plus the local
  `investigations/issue-29-…` report that seeded the number. Behaviourally inert
  (the `Debug Sample` drop is unconditional and case-independent, and neither
  indmach deck sets `debugtrace`). The auditor's side claim that
  `population.lock.json` "covers only the vendored electricdss-tst cases" is
  **wrong** — its `family_rigor` map carries the synthetic families too, and both
  indmach rows are in it with `evlog=1 engines=r4133`; the lock was a valid
  source, the original reading of it was not. (2) Two surviving comments still
  said the parity lane "rewrites nothing" (`lane.rs` `assert_reround_cells_are_
  live` doc, and the over-broad-guard comment in
  `eventlog_reround_cells_are_case_scoped_and_fail_on_stale`) — false since the
  two Relay rewrites moved above the `if PARITY` return; both narrowed to "no
  re-round cell runs there". (3) `golden_protection.rs` (:11-15, :229-231) called
  the `Debug Sample` line "unconditional" — still true of upstream r4133, which
  is what those sentences describe, but ambiguous now that the port gates it;
  annotated in place rather than rewritten, since the retirement rationale is
  unchanged. This file is not on the walked doc surface of
  `operational_docs_cite_the_compat_machinery_accurately`, so nothing catches it
  mechanically.
- **G2.3** (2026-08-05) — the **Newton stale `Iterminal`** row
  (`POWERS_REUSE_STALE_NEWTON_ITERMINAL`), the last CLAUDE.md upstream bug still
  reproduced anywhere. `DoNewtonSolution` bumps `SolutionCount` *before* its
  per-iteration `SumAllCurrents` — with the author's own comment "SumAllCurrents
  Uses ITerminal So must force a recalc" (`Common/Solution.pas:944`, the sum at
  `:947-948`) — so every element leaves the loop with `Iterminal` computed at the
  pre-final guess `NodeV_{n-1}` *and marked solved for the live `SolutionCount`*
  (`CktElement.pas:542-550`); `NodeV -= dV` runs only afterwards (`:965-968`). A
  post-solve `Get_Powers`/`Get_Losses` therefore finds the cache mark fresh and
  multiplies the converged `NodeV_n` by the conjugate of the *previous* step's
  current, while `CktElement.Currents` recomputes at `NodeV_n` — one element, one
  read, `S != V·conj(I)`, the identity `Powers` is defined by. Both lanes now
  call `refresh_iterminal` once in `exec::view::snapshot_elements` and feed
  Powers, Losses and Currents from that one current; nothing but a Newton solve
  moves, because after every other algorithm the cache is already invalid at read
  time. 14 → 13.
  **Coverage note — what this costs, and why there was no cheaper option.** The
  staleness is in *every* oracle channel (EPRI v9.8/r3723, v10.2/r4088,
  v11.0/r4133, all fingerprint 0.478 kVA, checked 2026-07-08, plus the pinned
  dss_capi 0.14.5), so no channel reports these decks' powers at the converged
  `NodeV` and gating on `r4133` would not have helped. `LANE_SKIP_ELEM_POWERS`
  (`harness/lane.rs`) therefore became **unconditional**: the two gated
  `modes:newton/newton.dss` and `modes:newton/newton_feeder.dss` decks lose their
  element powers/losses against **both** oracles in **both** lanes (measured
  divergence 4.86e-4 and 2.46e-3 kVA on `Vsource.source` conductor 0 — ~60× and
  ~35× their tier floors, so it could never be mistaken for drift). Everything
  else about those two decks stays oracle-compared in both lanes: element names,
  terminal **currents**, node voltages, the system Y, discrete state and the
  iteration count. What replaces the lost signal is unchanged and now runs
  identically in both lanes — the in-engine tripwire
  `newton_dispatch_leaves_a_valid_but_stale_iterminal_cache` (untouched: it reads
  the solver's leftover cache directly, so it fails if `Set algorithm=Newton`
  ever falls back to `DoNormalSolution`) plus the pin below. No third deck is
  affected: these are the only two gated decks that run a Newton solve.
  **Pin.** `newton_powers_are_the_lane_kernel` → `newton_powers_match_the_normal_
  algorithm`, unconditional: Newton's reported powers/losses equal the *normal*
  algorithm's on the same 3-bus deck (< 1e-8 kVA / < 1e-5 W, measured 3.256e-11
  kVA / 3.329e-8 W) — ten orders of magnitude from the stale reading (5.283e-1
  kVA / 6.248e2 W), with the algorithm-independent currents (4.5e-12 A) as the
  control. The normal algorithm's powers are oracle-gated on ~500 other corpus
  cases, which closes the loop transitively.
  **Bookkeeping.** Register row `Evidence::Exclusion` on `harness/lane.rs`
  (needle: the unconditional `if LANE_SKIP_ELEM_POWERS.contains(&label) {`, which
  under the split read `if !PARITY && …`) rather than the engine kernel:
  `exec/view.rs`'s torn-down form is a bare
  `elem.refresh_iterminal(&sys, &node_v);` at `snapshot_elements`' own
  indentation — byte-identical to the line the *Currents* read three dozen lines
  below already had, split form included — so no needle over that file can
  discriminate a revert, and the row's comment names the pin + tripwire as what
  carries the engine half instead (the same "say so in the row's comment" clause
  `Evidence::Site` documents, used by G2.2c row 2). Both
  `DOCUMENTED_DIVERGENCES` rows deleted from `examples/lane_dump.rs` — the list
  is now **empty**, so every record in the lane dump is held to the ordinary
  bound (it was fail-on-stale in the "entry must still fire" sense only, but
  leaving them would have exempted two now-identical fields from the differential
  gate). Doc strikes in the same commit: CLAUDE.md's bug-5 bullet and the WP-G2
  status line, `TESTING.md`'s lane-diff paragraph, and the
  `tests/corpus/modes/manifest.json` note for `newton.dss` (note text only —
  `population_lock.rs::Case::rigor` does not fingerprint `note`, and
  `population.lock.json` did not move). No golden byte moved (no golden deck runs
  a Newton solve), no ledger entry moved.
  `lane_diff.ps1` (mandatory here — this row *was* the lane differential's only
  documented divergence): **max |Δ| = 0** on all eight gated kinds, 520 cases /
  3 219 862 records, 0 iteration counts drifted, `VERDICT: PASS`, and
  "documented divergences: none present in this dump". The measurement is
  stronger than the previous runs' rather than merely equal to them: the two
  `newton` decks' 74 `pow` and 15 `loss` compared pairs (148 and 30 scalars in
  the dump, counted per `(re, im)` chunk) are now inside the gated `pow`/`loss`
  totals — 1 169 132 and 366 234 pairs respectively — instead of exempt from
  them, and they came back bit-identical; the lanes agree on exactly the
  channels that used to be the reason the list existed.
  **Audit settlement** (2026-08-05, three minor findings; one prose fix here,
  one one-line doc fix, one deviation ratified — no behavior changed, so
  `lane_diff.ps1` was not re-run). (1) This paragraph originally credited the
  two `newton` decks with "1 169 132 `pow` and 366 234 `loss` values" — those
  are the run's **global** per-kind compared columns, copied off the wrong line
  of the report. Re-counted directly on `target/lanes/default.dump` (the 09:08
  run this record describes), the two decks contribute 148 `pow` and 30 `loss`
  scalars = 74 and 15 compared pairs; the conclusion (newly gated, and
  bit-identical) is unchanged, its magnitude was not. (2) `tests/TOLERANCE_NOTES.md`
  still told the reader "**The default lane excludes, it does not loosen**",
  which reads as *only* the default lane excluding. Not falsified by this
  sub-step — `LANE_SKIP_PROPS` was already unconditional at 09f15731 (G2.2b) and
  the file's own Iresidual note says "in both lanes" — but stale since the
  2026-08-02 policy and inconsistent inside one file, so the bullet was reworded
  to "**The lanes exclude, they do not loosen**" with `LANE_SKIP_ELEM_POWERS`
  named as the live unconditional example. No tolerance number moved. The
  section's wider Stage-F framing ("the parity lane keeping the upstream
  answer") still holds for the surviving *precision* rows and stays G5.1's to
  retire. (3) The register row's `Evidence::Exclusion` on `harness/lane.rs`
  instead of an `Evidence::Site` on the engine is **ratified**, not a deviation
  to repair: re-verified that `git show 09f15731:…/exec/view.rs:232-233` is
  byte-for-byte the post-teardown `:201-202` (`cat -A`: sixteen spaces,
  `elem.refresh_iterminal(&sys, &node_v);` then `let cd = elem.cd();`), so no
  single- *or* multi-line needle over that file can discriminate a revert, which
  is exactly the escape clause the last paragraph of `Evidence::Site` documents
  and the shape G2.2c row 2 already used. The recorded slice does not exist in
  the split tree, and the marker obligation `Evidence::Exclusion` carries is met
  at `harness/lane.rs:136-139`.
- **G2.4** (2026-08-06) — the **monitor-channel padding** row
  (`MONITOR_CHANNEL_PADS_THE_UNFLUSHED_STREAM`): the `[0.0]` it reproduced is a
  client artifact, so the row is mostly *reclassified* rather than fixed — it was
  the only row the compat module ever carried whose upstream was not Pascal.
  13 → 12.
  **The blocked measurement, and the owner decision.** The plan's G2.4 section
  asked for a **channel-scoped** normalization firing only for `capi_v0145`, on
  the premise "on r4133 there is no wrapper, so a `[0.0]` capture is a real
  value". The sub-step's first attempt returned **blocked** because that premise
  is false in this tree: the pad is a client-layer artifact on **both** gating
  channels — dss-python pads in `dss/IMonitors.py` (`if cnt == 272: return
  np.zeros((1,))`, 272 = the header-only `ByteStream`), and the `r4133` channel's
  captures come from our own bridge, which replicates that decoder by design
  (`crates/dss-epri/src/dss.rs:625-634`, "exactly like dss-python"). That bridge
  decoder, not any Pascal accessor, is the load-bearing evidence on the r4133
  channel. Channel-scoping was **measured** to red three gated `r4133`
  cases (`modes:time/generaltime.dss`, `generaltime_yearly.dss`,
  `generaltime_duty.dss`). The plan owner chose **resolution (A)** on 2026-08-06
  and this commit amends the falsified plan text: keep the normalization
  channel-**independent** and make it lane-independent. Rejected alternatives,
  recorded so they are not re-proposed: (B) making the `dss-epri` decoder return
  the honest empty channel would not make the bridge engine-faithful either (the
  r4133 accessor's own answer here is `SampleCount` zeros, see below), it would
  only swap one client fabrication for another while breaking the bridge's design
  contract of being a dss-python-shaped reader so both channels' captures stay
  comparable; (C) three ledger entries would name a divergence the gate cannot
  see.
  **The engine half — the settle's correction (2026-08-06, audit finding).** The
  first write-up of this row said the engines return the *empty* channel for an
  unflushed stream and that therefore "no engine value is asserted away". That is
  false for the state actually gated. `Monitors_Get_Channel` keeps its empty
  `DefaultResult` (`CAPI_Monitors.pas:304`) only for `SampleCount <= 0` (`:308`)
  or an invalid index (`:313-320`); the `generaltime*` decks sit at
  `SampleCount > 0` with a header-only stream (`TakeSample` increments the
  counter, `Monitor.pas:1195`, while only `Save` grows the stream, `:1122-1125` —
  and the harness compares `sample_count` strictly, so both sides agree it is 8),
  so the C-API allocates `SampleCount` doubles (`:321`) and fills them from a
  zero-filled `AllocMem` buffer (`:325`) whose `MonitorStream.Read`s all fail at
  EOF: it returns `SampleCount` zeros conjured out of bytes it never wrote.
  r4133's native accessor behaves the same — `DMonitors.pas:509-516` pads
  `myDBLArray := [0]` only while `SampleCount = 0` and at `SampleCount > 0` takes
  the read branch (`:517-541`) over that same unwritten region; so does the COM
  wrapper (`DLL/ImplMonitors.pas:419-465`). So the value ladder is: both engines
  `SampleCount` zeros, both clients `[0.0]`, this port `[]`. The port's answer is
  the correct one under the 2026-08-02 no-bug-reproduction policy — fabricating
  samples from unwritten stream bytes is an upstream defect, not a convention —
  which makes G2.4 *also* a bug fix, not only a reclassification. It owes no
  ledger entry because the defect is **unobservable through either gating
  client**: both short-circuit at `cnt == 272` and never reach the accessor.
  Recorded upstream-ready as
  `investigations/to_opendss/35-monitors-channel-fabricates-zeros.md` (local-only
  folder), the one artifact it produces. All the surfaces that carried the
  overstated citation were corrected in this settle commit (engine doc, pin
  comment, `compat.rs`, `lane.rs`, the `TORN_DOWN_ROWS` entry, plan §G2.4, the
  Stage-F phase record, here).
  **What landed.** The alias, both impls and both cfg arms are gone;
  `Monitor::channel` folds the unflushed case into its index guard and returns
  the empty channel in both lanes (which is what the neighbouring `dbl_hour`
  read of the same stream always did). `harness::lane::expected_monitor_
  channel` lost its `PARITY` early return and is now an unconditional capture
  normalization carrying the row's `LANE-EXCLUSION` marker; no `EngineChannel`
  parameter was threaded through `harness::compare_monitor` — nothing needs one
  under (A). Its shape guards are unchanged: the rewrite fires only at
  `flushed_records == 0` and only on a literal one-element `[0.0]`, so an engine
  that loses real samples, or an oracle that starts reporting something else,
  still fails loudly.
  **The one rot the shape guards stopped catching, and its fix (settle).** Once
  both lanes report `[]`, a client that *stopped* padding would also return `[]`
  — equal to the engine's answer, so the compare would pass and the
  normalization would quietly become dead code. (Pre-G2.4 the parity engine
  emitted `[0.0]`, so that drift failed the length check there.) The first
  write-up claimed "the transform can never rot into a silent pass", which is now
  true only for captures that are neither `[0.0]` nor empty. Fixed rather than
  merely re-worded: `lane.rs` counts placeholder hits vs non-placeholder
  unflushed captures and `assert_monitor_pad_is_live` — called from
  `corpus_gate.rs` beside `assert_reround_cells_are_live`, self-silencing when
  nothing unflushed was visited — fails on any miss, held by the unit test
  `monitor_pad_liveness_is_asserted_not_assumed`. Note the loss was
  oracle-drift detection only: `sample_count` and the header stay strictly
  compared for those monitors, and a flushed monitor is never rewritten.
  **Pin** (this row had none): `monitor_channel_of_an_unflushed_stream_is_empty`
  in `elements/meter/monitor/mod.rs` — four samples of a **three-channel**
  monitor staged in `MonBuffer`, none flushed, every channel empty and `dbl_hour`
  empty beside it, asserted unconditionally, so "empty" means "nothing flushed",
  never "nothing sampled". The multi-channel staging is the settle's second
  correction (the pin's loop originally ran over a one-channel fixture, so
  "every channel" was a single call): it separates the folded guard's two halves
  — after `save()` every in-range channel must carry its own four samples, while
  index `0` and `RecordSize + 1` stay empty on both sides of the flush.
  Register row `Evidence::Site` on the engine kernel: the folded
  `if i < 1 || i > self.record_size || self.flushed_records == 0 {` exists only
  after the teardown (the split had a second early return below it), so unlike
  G2.3 a needle here does discriminate a revert.
  **Bookkeeping.** No golden byte moved (no golden deck leaves a monitor
  unflushed), no ledger entry and no `population.lock.json` field moved, and the
  three `generaltime*` decks stay green on **both** channels in both lanes. No
  doc-surface citation existed for this row (the walked surface never named it);
  the stale claims that *did* exist were corrected in the same commit —
  `GOLDEN_REBASE_PLAN.md` §G2.4 (the falsified premise → the measurement +
  resolution A) and the compat module's "the only row whose upstream is not
  Pascal" sentence, whose provenance is now stated as the client wrapper of both
  oracle read paths. `docs/phase-records/depascalize-stagef.md` keeps its Stage-F
  table as history but carries a dated correction block under it: that table is
  where the overstated `Monitors_Get_Channel` citation was originally written,
  and every later copy of it descends from that cell.
  `lane_diff.ps1` (mandatory — a lane alias was deleted): **max |Δ| = 0** on all
  eight gated kinds (520 cases, 3 219 862 records, 0 iteration counts drifted,
  `VERDICT: PASS`, "documented divergences: none present in this dump"). As
  predicted: monitors are not in the dump set, and the engine's default-lane
  answer did not move — in that lane the teardown is behaviourally inert (the
  alias already selected the empty channel and `expected_monitor_channel`'s
  `PARITY ||` was already false), so only the parity lane's reading changed.
  **Gate flakes seen on the way, both proven infrastructural, both in
  `corpus_gate` and neither reproducible once the corpus tree was clean.**
  (1) With ~28 untracked export artifacts left in `tests/corpus/electricdss-tst`
  by an aborted run, two runs failed with 20 and then 7 *different* cases
  diverging on `[R4133]` step-0 node voltages (rel ~5e-7) — decks with no
  monitors among them; both vanished after deleting the artifacts file by file,
  and the same binary then passed 53/53 twice. (2) Twice, one case failed with
  the oracle's own `Show Voltage LN Nodes` raising DSS error 303 — "Unable to
  create file … The process cannot access the file because it is being used by
  another process" (`GFM_IEEE8500/IEEE8500u_VLN_Node.txt`) — the file-collision
  class `corpus_gate/runner.rs:42-55` already documents, and the same
  still-open item recorded at the G2.2d settle. The final green runs were made
  from a clean corpus tree; the artifacts produced by each run were removed by
  explicit per-file deletion, never a recursive one.
  **One real defect caught by the gate and fixed in the same commit**: the new
  pin first walked its channels as `for i in 1..=m.num_channels()`, which
  `depascalize_metrics_gate::part3_metrics_audited_populations_do_not_grow`
  rejected (P14: 107 `for … in 1..=` loops in `elements/`, ceiling 106). Rewritten
  0-based with the `+ 1` at the 1-based `Channel(i)` boundary — same coverage, the
  port's own indexing convention.
  **Settle gate (2026-08-07).** Full five-command gate re-run from a clean
  corpus tree after the corrections above: `fmt --check` clean, both clippy
  lanes clean, `cargo test --workspace` and
  `cargo test --workspace --features dss-core/oracle-parity` both exit 0 —
  including the new `assert_monitor_pad_is_live` (no miss recorded on either
  lane, so both clients still pad every unflushed monitor the gate visits).
  `git diff --stat -- tests/golden` still empty over the whole range.
  `lane_diff.ps1` was **not** re-run and is not owed: the settle touches no
  compat kernel and no engine code at all — the `src/` diff is doc comments plus
  the `#[cfg(test)]` fixture, so the implementation commit's measured
  `max |Δ| = 0` still stands. Two markdown surfaces edited here
  (`STATUS.md`/`GOLDEN_REBASE_PLAN.md`) and `docs/` are explicitly outside the
  doc gate's walk (`oracle_parity_cfg_gate.rs::operational_docs`, "deliberately
  excluded: plans and records").

- **G2.5** (2026-08-07) — the **three corpus-blocked `WholeCase` bug fixes**:
  GICTransformer `%R2` ignored, Capacitor `MakePosSequence` `Cuf` discarded,
  LoadShape memory-mapped text accept-set. `Escape::WholeCase` **4 → 1** (the
  survivor is the Generator Model=6 user-model row, deferred to
  `WASM_USERMODELS_PLAN`); `SPLIT_ALIAS_POPULATION` does **not** move — none of
  the three was ever a lane split, all three were reproduced identically in both
  lanes, and all three are now fixed in both.
  **The three fixes.** (1) `gic_transformer/solve.rs::recalc` derives winding 2's
  conductance from `%R2`; both oracle revisions copied the `G1` line and renamed
  the base but not the percentage (`GICTransformer.pas:441` == r4133
  `Version8/Source/PDElements/GICTransformer.pas:495`), which the same
  procedure's `else` arm — restoring `FPctR2` **from** `G2` — proves is a slip:
  the two arms are mutual inverses only under `FPctR2`. (2)
  `capacitor/solve.rs::make_pos_sequence` writes `Cs - Cm` through the **array**
  setter in the property's own µF units, plus the `CUF` arm
  `set_struct_f64_array` never had. 0.14.5 aims the scalar `SetDouble` at the
  array property `Cuf` (`Capacitor.pas:814` + `DSSObjectHelper.pas:2812-2834`,
  three scalar arms and no `else`), dropping the value with no error while still
  running the `SpecType := 2` side effect — so the bank computed from the
  `kvar=1200 kv=12.47` creation defaults (20.47 µF, ~5× the intended reactive
  output) and the user's `cmatrix` left the Y build for good; r4133 *does* apply
  it (`Capacitor.pas:829` → `InterpretDblArray`) but re-applies the `1.0e-6`
  scale (`:411`) to an already-farad value, landing 4e-12 F where 4e-6 F was
  meant. Neither revision produces the intended bank, and r4133 spells out what
  was intended, so the port performs that write. (3)
  `load_shape/compute.rs::mmf_text_value` takes the column verbatim through the
  **same** aux parser the class's non-mapped reader uses; both revisions filter
  it through an accept-set of bytes in `[46,58)` (`LoadShape.pas:1374` == r4133
  `Common/Utilities.pas:834`), deleting sign, `+`, the exponent letter and
  whitespace while keeping `/` inside the token. The witness that this is a slip
  and not a dialect is the class itself: two readers, one file, and
  `MemoryMapping=Yes` chooses storage, not meaning.
  **What it cost, measured (2026-08-06, `DSS_GATE_ONLY` on both channels).** Four
  gated decks move against their gating oracle(s) across the solved model —
  `asymmetric:gic/gictransformer_gic.dss` (node V 4.502e-4 V, allowed 1.001e-6),
  `asymmetric:gic/gic_midi.dss` (1.021e-4 vs 1.074e-6), both on **both** channels
  since r4133 carries the identical line; `modes:makeposseq/makeposseq_shunt.dss`
  (1.158e-1 V vs 3.339e-6) and `modes:inputformat/shape_mmf/shape_mmf.dss`
  (1.641e1 V vs 8.179e-6) on `capi_v0145`. Each (deck, channel) is an
  `exclusion` entry in `tests/corpus/ledger.json` — 6 entries — scoped
  **by field**, to the fields measured to move rather than to the whole case:
  `voltages`, `y`, `y_fingerprint`, the **named** YPrim(s) and `element` on all
  four, plus `injection` and the four monitors on `shape_mmf` alone. Two
  candidate scopes were probed and dropped as inert — the injection RHS on the
  GIC and `makeposseq` decks (the GICLine/GICsource drives do not depend on the
  solution) and `shape_mmf`'s `ls_pq` loadshape probes — so those keep comparing
  against the oracle, as does everything else on those decks. (The `ls_pq`
  probes are inert *by construction*, not merely clean: under
  `MemoryMapping=Yes` `mult`/`qmult` return the `(<directive>)` string,
  `LoadShape.pas:1844-1851`/`:1863-1868`, and `SetMaxPandQ` exits at
  `:2044-2049` so `pmax`/`qmax` keep their creation defaults `:1273`. The live
  witness for the MMF readers is the sibling deck.) The `voltages`/`element`/
  `monitor` scopes are **deck-wide**, and the settle below re-measured them
  artifact-by-artifact. The property jumps are pinned instead of skipped:
  3 exact-pair `divergence` entries hold `GICTransformer.tg3/tg5.R2` (0.09522
  ours vs 0.12696 upstream) and `Capacitor.cap_cmat.Cuf`/`NormAmps`/`EmergAmps`.
  Ledger 27 → 36 entries over 23 causes; each `cause` cites its pin by full test
  path, as the plan requires.
  **New ledger machinery, and why it was unavoidable.** All three fixes move the
  assembled **system Y** (the GIC transformer's and the capacitor's YPrim
  directly; `shape_mmf` through `Load.ld_pq`'s `Yeq`), and the ledger had no
  field that could name it — `y`/`y_fingerprint`/`yprim`/`meter` were the
  §1.3-planned-but-unimplemented set the loader **rejects**. G2.5 implements
  them as **exclusion-only** fields (`LedgerView::excluded`,
  `EXCLUSION_ONLY_FIELDS`, refused on a `divergence` by `assert_structural`):
  each names a whole compared
  artifact with no remainder to tier-check and no envelope to re-assert, so the
  only honest statement is "this (case, channel) does not compare it" — still
  hit-accounted, so an exclusion whose scope stops matching fails the gate as
  NEVER APPLIED. `injection` also learned to honour an exclusion (it had a
  `Divergence`-only handler, and the RHS has no sub-selector either way), and
  the probe/property exact-pair *pin* requirement is now scoped to `divergence`
  entries, where it belongs. And because "has a runtime handler" had just become
  **kind-dependent**, the whitelist grew its mirror: `EXCLUSION_FIELDS` refuses
  an `exclusion` on `iterations`/`property`/`eventlog`/`ctrlqueue`, whose
  handlers re-assert a pin or rewrite the oracle's line and would otherwise load
  cleanly and then do nothing — the exact rot `LEDGER_FIELDS` exists to prevent.
  **`shape_mmf.dss` keeps its job; its coverage moved before it was excluded.**
  The deck exists *to observe* the quirk (its `mmpq8.csv` P column is exponent
  notation on purpose), so it is the one that pays. Its unrelated surface — the
  `sngfile=`/`dblfile=` MMF readers, the raw `mult=(sngfile=)` directive form
  (Pascal `CustomSetRaw`), the two-column `pqcsvfile=` reader, the
  `(<directive>)` array-property round-trip and the `Set/Get totaltime` option
  surface — moved to the new sibling deck
  `modes:inputformat/shape_mmf_io/shape_mmf_io.dss`, whose `mmpq8_plain.csv`
  holds the same numbers in plain decimal (so the upstream filter is the
  identity on it) and which gates **clean** against the pinned oracle on the
  first run. Corpus population 520 → 521 cases (517 solvable), which is growth,
  not a shrink; `population.lock.json` regenerated in the same commit and its
  diff is 6 lines. `tools/decks/gen_shape_mmf_fixtures.py` now writes both
  fixture sets.
  **Golden side: zero bytes moved, two field-scoped exclusions added.** Two
  committed artifacts observe the GICTransformer row and were excluded, never
  re-baselined. (a) `tests/golden/props/gictransformer.json`'s
  `gictransformer_auto` scenario — `("gictransformer_auto", "R2")` joins
  `props_roundtrip.rs::LANE_SKIP_SCENARIO_PROPS`, value-only and property-scoped,
  so `%R1`/`%R2`/`R1`/the bases/the `type=Auto` bus promotion stay compared (the
  file's cell-count lock is unchanged: one cell moves from `compared` to
  `lane_skips`). (b) `tests/golden/reports/export_gicmvars.txt` — both value
  columns masked with `GateSpec::Mask` (the `export_busreliability` `Duration`
  precedent from G2.2a), since every row moves (8.8e-4 relative even on `b1`,
  the row furthest from `tg3`) and no row key separates them. Its replacement,
  `export_gicmvars_matches_the_equivalent_ohms_spec`, does more than restate the
  new numbers: it rebuilds the fixture with `tg3` given `R1=0.7935 R2=0.12696` —
  upstream's *effective* conductances, reached through the `R1=`/`R2=` arm
  neither revision ever got wrong — and demands the committed capture back
  inside the golden's own band, so the report path, the quasi-DC solve, the
  K-factor Mvar path and the VarCurve path are re-earned against the oracle
  bytes; then it rebuilds with `R2=0.09522` (`ZBase2·%R2/100`) and demands
  equality with the live `%R` spec, cell for cell; then it asserts the masked
  cells really moved, so the mask cannot buy silence.
  **Pins.** `exec::tests::compat_quirks::gic_transformer_pct_r2_drives_winding_two`
  (the `%R` spec equals its ohms twin on `R1`, `R2` **and** the stamped YPrim, on
  deliberately asymmetric bases where the three candidate readings — 4 Ω, 3 Ω and
  upstream's 1 Ω — are pairwise distinct);
  `capacitor::tests::make_pos_sequence_cmatrix_applies_the_positive_sequence_cuf`
  (the reduced bank is indistinguishable from one declared `phases=1 cuf=4`,
  YPrim for YPrim — an anchor that is neither a captured number nor the fixed
  kernel) and `...::make_pos_sequence_cmatrix` for the emitted action's shape;
  `load_shape::tests::mmf_text_reader_agrees_with_its_non_mapped_twin` (the two
  readers agree bit-for-bit on content carrying `-`, `e-`, `+` and leading
  whitespace, *and* are not the accept-set's readings) plus
  `...::mmf_plaintext_reader_keeps_sign_and_exponent` and
  `...::mmf_accept_set_fix_is_gated_by_exactly_one_deck`, which measures in corpus
  bytes that exactly one deck can see the change — the vendored
  `MemoryMappingLoadShapes/ckt24` files and the new sibling's fixture are all on
  the identity side of the upstream filter.
  **Bookkeeping.** Three `TORN_DOWN_ROWS` rows with `Kind::WholeCase` and
  `Evidence::Ledger` (its first use — the variant's `expect(dead_code)` note
  amended accordingly), each carrying its `EXPECTED-VALUE-PIN` marker; three
  `ESCAPE_REGISTER` rows and their `TODO(compat)` markers deleted;
  `EXIT_POPULATION[WholeCase]` 4 → 1; the two `compat.rs` narrative paragraphs
  rewritten in place; `TESTING.md` (ledger kinds/fields/counts, 521/517) and
  `CLAUDE.md` (521/517) updated; the local `investigations/TODO_COMPAT_REGISTRY.md`
  §2.6/§2.7/§2.8 «fate» entries rewritten. No doc-surface `compat::` citation
  named any of the three rows, so the alias-citation floor is untouched.
  `lane_diff.ps1`: **max |Δ| = 0** on every gated kind — the three fixes are
  single kernels, identical in both lanes, so the lanes stay bit-identical.

- **G2.5 settle** (2026-08-07) — twelve audit findings (1 major, 11 minor; three
  pairs raised independently by both auditors), all settled against measurement
  or source, none dropped and none refuted-only. Nothing in the three engine
  fixes changed; the settle is test machinery + audit trail.
  **The exclusion kind gets the half of fail-on-stale it can honestly carry.**
  Both auditors flagged that `LedgerView::excluded` never calls `mark_exceeded`,
  so an `exclusion` could outlive its cause silently. Half of that is now
  mechanical: a `voltages` scope IS measured node-by-node inside
  `voltage_keep_mask` (the diff and the tier floor are already in hand), so the
  `Kind::Exclusion` arm records the exceed and `assert_all_hit` reports an
  applied-but-never-exceeding voltages exclusion as **STALE** — and every
  exclusion that pays for an engine fix carries one, because moving node voltages
  is what makes such a fix need this kind at all. Two sub-claims of that finding
  are refuted on the record: TESTING.md never overclaimed (its Runtime rules
  already read "every `divergence` must still exceed the tier floor"), and "no
  mechanism can ever notice a revert" is false at the level of the five-command
  gate — reverting the MMF fix reds
  `mmf_plaintext_reader_keeps_sign_and_exponent`, reverting the other two reds
  their pins *and* their sibling exact-pair `divergence` entries. What was
  genuinely missing is the "the divergence disappeared for some other reason,
  prune the entry" signal, and that is what landed. The other half is declined
  with its reason: `y`/`y_fingerprint`/`yprim`/`meter`/`injection`/`monitor`/`probe`/
  `element` exclusions make the runner *skip* the artifact, so a verdict would
  mean a second copy of each comparator inside the ledger — a new drift surface —
  and their anti-rot guard stays the expected-value pin the `cause` names
  (mandatory, registered both ways in `TORN_DOWN_ROWS`; reverting any of the three
  fixes reds a pin whether or not the corpus gate notices). Non-vacuity is a
  canary, not a claim: `a_voltages_exclusion_that_masks_nothing_is_stale` drives
  `assert_all_hit` over a synthetic applied-but-clean entry and asserts the STALE
  text, that the same entry passes once something exceeds, and that a
  coarse-field-only exclusion is *not* policed. All six live entries stayed green
  (full corpus gate, both channels).
  **The deck-wide scopes are now measured, artifact by artifact.** The G2.5
  ledger `source` strings said "every element channel" moved; that was a physical
  argument, not a measurement. Re-measured with a throwaway per-artifact verdict
  probe (`catch_unwind` around each `compare_element`/`compare_monitor`, the
  blanket scopes neutralized, `DSS_GATE_ONLY` on the four decks, both channels):
  `gictransformer_gic` **9/9** elements and all nodes above floor;
  `makeposseq_shunt` **14/14** and all nodes; `shape_mmf` **6/6**
  electrically-connected elements, all 4 monitors and all nodes at every one of
  its 8 steps (the 4 `Monitor` *elements* inside the blanket carry no
  current/power channel at all, so they mask nothing that exists);
  `gic_midi` **12/18** elements and **26/33** nodes — the one deck where the
  blanket is wider than the above-floor set. It is kept, on the record that the
  six sub-floor elements (`GICLine.gl12/gl23/gl34`, `GICTransformer.tg3`,
  `Reactor.gg3`, `Reactor.g2`) and seven sub-floor nodes (`B3.*`, `B3X.*`) are
  **not unaffected** — every one of them moves, they merely land under the floor
  (`B3.1` 9.291e-7 against a 3.139e-6 floor) — and that a 12-name allowlist would
  be twelve claims each needing its own liveness the exclusion kind does not
  have. Every `source` string and the STATUS sentence above now say which scopes
  are deck-wide and what the per-artifact verdict was.
  **Smaller settlements.** `assert_structural` now refuses an `exclusion` scope
  carrying `max_rel`/`max_abs`/`num_rel`/`rust`/`oracle`/`policy`/`line_re` (the
  exclusion path ignores them, so they would read as a promise the gate never
  keeps); `every_exclusion_field_is_honoured_by_the_runtime` drives
  `LedgerView::excluded` synthetically over every whitelisted exclusion field, so
  `probe` and `meter` — which no live entry uses — are proven to apply, along with
  the `name_re`/`steps`/kind selector rules.
  `mmf_accept_set_fix_is_gated_by_exactly_one_deck`
  gained the corpus's **fourth** mapped plain-text fixture,
  `modes/upgrade/mmf_singlecol/mm8.csv` (r4133-gated), which the enumeration had
  missed. Two `Utilities.pas:833` citations corrected to `:834` (the accept-set
  line; `:833` is its comment). `shape_mmf.dss`'s own header and its manifest
  `notes` — the two documents the fix made false — rewritten to say the deck
  exists to *observe* the quirk, that the port reads the file verbatim since
  G2.5, that it is ledger-excluded, and that its reader coverage moved to
  `shape_mmf_io`; no lock field moves (`Case::rigor` does not fingerprint
  `note`/`notes`). The `recalc` doc comment and the `gic-pct-r2-ignored` cause now
  state the true divergence class — **any** `%R`-specified GICTransformer, since a
  deck writing only `%R1=` now takes the `%R2` creation default `0.2`
  (`GICTransformer.pas:409-410`, r4133 `:458-459`) instead of repeating `%R1`;
  no corpus deck has that shape (grep: the two ledgered decks are the only `%R`
  decks and both set both). `ORPHANED_GAPS.md`'s "4 `WholeCase` compat markers /
  unowned policy call" row now names only the Generator Model=6 survivor and
  records the other three as closed by G2.5.
  `population.lock.json` regenerated: the diff is exactly the six edited entries'
  digests on four cases — no rigor field, no case membership. Zero golden bytes
  moved (`git diff --stat -- tests/golden` empty over the whole range).
  No engine kernel changed (the only engine-file edit is a doc comment), so
  `lane_diff.ps1` was not re-run — G2.5's own run already reported max |Δ| = 0
  and nothing since can move it.

- **G2.6** (2026-08-07) — the **`Show` device-name column width**, the capi-only
  row the `4f977d9e` alignment pass missed and the last of WP-G2.
  `compat::max_device_name_length` reproduced a **defect**, not a rendering
  convention: dss_capi's `SetMaxDeviceNameLength` zeroes the unit variable
  (`Common/ShowResults.pas:116`, declared `:82`) and then accumulates the maximum
  inside `with DSS.ActiveCircuit do` (`:117-121`), where the identifier resolves
  to the **shadowing** `TDSSCircuit` field (`Common/Circuit.pas:100`, initialized
  to 30 at `:379`) — so the writers, which read the unit variable, format every
  `Show` name column against **0**. Verified against both sources in this
  sub-step: r4133 has no such field, `MaxDeviceNameLength` lives there only as a
  unit variable (`Version8/Source/Common/ShowResults.pas:66`, loop `:79-90`), and
  its `WriteTerminalPowerSeq` writes the terminal as `j:3` (`:1160`) rather than
  `IntToStr(j)` — two independent reasons the authority cannot glue. **Teardown:**
  the alias and both `_impl` kernels are gone; all seven `report::show` writers
  call `device_name_width` directly, in both lanes.
  **Blast radius, re-measured rather than assumed.** Only the `IntToStr` site
  (`ShowResults.pas:1375`) is tokenizable, so only the three `show_busflow*`
  goldens are affected; every other consumer pads with spaces or `PadDots` runs,
  which `harness::split_fields` drops. Measured by running the whole
  `golden_reports` suite in the **parity** lane after the flip (249 tests green,
  including every `Show` family) and by a tree-wide search for a closing quote
  followed by a non-space in `tests/golden/reports/show_*.txt`: the only hits are
  the three busflow files (5 + 5 + 3 rows) plus four reports that do not use this
  width at all. Their oracle text is de-glued by `golden_reports::busflow_expected`,
  now **unconditional** (`LANE-EXCLUSION(max_device_name_length)`); the parity
  early return and the non-vacuity test's lane arm are gone, while that test's
  capture-reading assert stays (it reads the committed oracle capture, a fact
  about dss_capi 0.14.5, not about our lane — it dies at G3.3b).
  `terminal_total_expected` and its non-vacuity lane arm were deliberately **not**
  touched: they belong to `compat::render_rows` over `PadDots('   TERMINAL
  TOTAL')`, which lives until WP-G4.5.
  **Pin.** `device_name_column_width_is_the_lane_kernel` →
  `exec::tests::compat_quirks::device_name_column_is_sized_from_its_content`
  (`EXPECTED-VALUE-PIN(max_device_name_length)`), unconditional, and strengthened
  while it lost its branches: besides the measured width and both sides of the
  glue boundary (a short name gets its own terminal column; the *longest* name
  fills `width + 2` exactly and still glues), it now asserts the real
  `Show busflow` text from the executive's own formatter, so the seven call sites
  are covered and not just the measuring function. That third claim bites in the
  parity lane, where `compat::render_rows` replays Pascal's `Pad`; the default
  lane's table kernel would separate the columns anyway. (As first written that
  claim did not discriminate at all — corrected at the settle below.)
  **Collateral, found by the parity gate and repaired in the same sub-step.**
  The neighbouring `render_rows` pin `show_table_layout_is_the_lane_kernel`
  claimed "the two `Show Losses` rows' numbers start at the same column only when
  a table sized them" — a discriminator that worked *because* the parity width
  was 0: with an honest width, `Pad` aligns the rows with each other too, so the
  claim went red in the parity lane. It was **not** relaxed: claim 2 is now made
  against each kernel's own sizing rule (parity pads to the engine's circuit-wide
  `width + 2`, the table sizes from the names it actually prints), and the
  fixture was given a **Load** whose name is longer than either Line's — `Show
  Losses` lists only PD elements, so that name sets Pascal's field width without
  ever reaching the table, which re-separates the kernels by ~12 columns. The
  fixture's discriminating property is itself asserted, so a rename cannot make
  the test vacuous.
  **Bookkeeping.** `SPLIT_ALIAS_POPULATION` 12 → 11 with the eleven survivors
  enumerated at the constant; `TORN_DOWN_ROWS` gains the row (`Kind::SplitAlias`,
  `Evidence::Site` on `report/show/bus_powers.rs` — the one call site a golden
  observes, the other six being unobservable by construction now that the alias
  itself is deleted); the F-FMT narrative at `oracle_parity_cfg_gate.rs:488`
  records that six of F.4's seven rows survive and the seventh was never a
  rendering convention; the `compat.rs:47` table row and the `max_bus_name_length`
  note in `report/show/mod.rs` are re-pointed. **No doc-surface citation existed**
  (measured over the walked surface — CLAUDE.md / TESTING.md / TOLERANCE_NOTES /
  ledger / manifests / `tools/**`: zero hits for the alias), so no doc edit was
  owed. The `branches_on_lane` doc-measurement is re-taken: `golden_reports.rs`
  now reads the lane ×4 (was ×6) and, having lost its last split alias, is no
  longer a pin file — so **no** surviving row's pin depends on the `lane::PARITY`
  arm; it stays as the recogniser for the harness spelling, and the doc now says
  so instead of claiming it load-bearing.
  **Local-only docs (gitignored, written in this sub-step).** This row had neither
  an `issue-*` report nor a registry section — it is the one bug the investigation
  series missed. Both were written:
  `investigations/issue-36-show-device-name-column-width-zero.md` and
  `TODO_COMPAT_REGISTRY.md` §3.32, plus a correction to §3.31 (which had counted
  the width among eleven FPC *formatting* places) and a tenth row in
  `investigations/to_opendss/NOT-APPLICABLE-TO-R4133.md` carrying the
  `Circuit.pas:100` field citation. No `to_opendss` report is owed — r4133 does
  not carry the defect.
  Zero golden bytes moved (`git diff --stat -- tests/golden` empty over the
  range); the corpus gate is green in both lanes, which is what would have
  falsified the classification. `lane_diff.ps1` re-run because a compat kernel
  was deleted: **max |Δ| = 0** on all eight gated kinds over 521 cases /
  3 220 212 records, zero drifted iteration counts — as predicted, report text is
  not in the dump set, so the flip is invisible there.

- **G2.6 settle** (2026-08-07) — five auditor findings, four fixed and one
  refuted-then-fixed-anyway; no engine behaviour changed (every edit is a test
  assertion or a doc comment), so `lane_diff.ps1` was not re-run and zero golden
  bytes moved.
  **The pin's third claim was vacuous** (major). It asserted `!row.contains("\"1")`
  on `Line.l1`'s seq-power row, but `Line.l1` reaches `b2` by its **second**
  terminal (`check_bus_reference` returns the matched terminal), so the row
  carries a `2` and the needle could not match at *either* width. Proven by
  mutation, not by reading: forcing `show_bus_powers`' `mdnl` to 0 — the exact
  value G2.6 tore down — left the pin **green** in the parity lane. It now
  asserts that the first two whitespace tokens are `"Line.l1"` and `2` (a glued
  row fails: the mutation reports `["\"Line.l1\"2", "-0.0"]`) **and** that the
  terminal starts no earlier than column `measured + 2`, which a shrunk-but-still-
  separating width fails too (`mdnl = 20` → column 22 against the measured 32).
  Both mutations were re-run against the repaired pin and both go red. The claim
  bites in the parity lane only, and now for a stated reason: the table kernel
  builds columns from cell *text* and ignores the declared width, so no default-lane
  report can observe a width regression — what covers that lane is claim 1, on the
  measuring function both lanes share. Claim 2 is relabelled as what it is: an
  assertion about Pascal's `Pad`, not about a report.
  **`show_table_layout_is_the_lane_kernel` claim 2 is an equality again.** G2.6
  had rewritten it as `kw_col >= width + 2`, which accepts any over-padding on the
  parity side; each row is now reconstructed whole from `Pad(EncloseQuotes(name),
  width + 2) + Format('%10.5f, ', …)` and compared, the way claim 1 treats the
  aggregate line. Verified by mutation: padding the name cell to `width + 12`
  passes the old bound and fails the equality.
  **Doc corrections.** `report/show/powers.rs`'s glue note claimed the honest
  width glues the longest name "in both lanes" — only the parity kernel glues
  (`report::table::render_rows_table_impl` gives every cell its own column);
  `golden_reports.rs::run_feeder_show_expected` still required its transform to be
  the identity in the parity lane, an invariant G2.6 deliberately dropped for
  `busflow_expected` — it now states that the reach is the caller's row
  (identity while a split survives, unconditional once the row is torn down).
  **The `max_bus_name_length` diagnosis was wrong and is now evidence-backed.**
  The note called the backend's effective width "nondeterministic (no single value
  reproduces it)". It is the *same* shadowing defect as the device-name one, one
  identifier over: `SetMaxBusNameLength` assigns 4 to the unit variable
  (`ShowResults.pas:105`, declared `:81`) and max-accumulates inside
  `with DSS.ActiveCircuit do` (`:106-108`) into the shadowing field
  (`Circuit.pas:100`, init 12 at `:380`). Two reachable values, and both are
  visible in committed captures: `show_voltages.txt:4` is `Pad('Bus', …)` from
  `ShowVoltages` (`:414`, outside the `with`) at width 4, while the bus rows below
  it come from `WriteSeqVoltages`, whose whole body is a `with` (`:135`), at width
  12; `show_powers_elem.txt:8` is `Pad('  Bus', …)` from `ShowPowers` (`:1130`,
  outside) at width 4. r4133 has no such field (`ShowResults.pas:65`, loop
  `:75-76`). The disposition is unchanged — the honest width stays — but it now
  rests on the 2026-08-02 policy instead of the UB rule, and no row was ever owed
  because the width only feeds padding no oracle-compared token can see.
  Recorded in the local-only docs: `TODO_COMPAT_REGISTRY.md` §3.32 and a sibling
  section in `investigations/issue-36-*.md` naming it a candidate row for the
  series (no `to_opendss` row: it is capi-only and unobservable).

### R4133_PROPS WP-RP0 — condensed records

> Plan: `R4133_PROPS_PLAN.md`. Branch `r4133-props` off `update` @ `2ee6bb00`.
> Every sub-step runs the plan's per-sub-step ritual: full five-command gate
> before the commit, then two fresh auditors and a fix agent.

- **RP0.1** (2026-08-22) — the G1.1 census evidence vendored to
  `tests/corpus/props_r4133/`. Five byte-identical copies of the local-only
  `investigations/g1_1_r4133_props/` extracts (24 944 bytes: `triage.md` 146
  lines, `summary.json`, `structural_pairs.txt` 209 + 1, `numeric_pairs.txt`
  94 + 1, `shape.txt` 5) plus five derivations of the 270 MiB local census:
  `examples_full.txt` (3 378 rows — every distinct `(rust, r4133)` spelling per
  pair, untruncated; counts sum to all 1 055 446 value cells),
  `bins.tsv` (303 rows, one §1.1 bin each),
  `structural_pairs_in_scope.txt` (198), `numeric_pairs_in_scope.txt` (53) and
  `shape_in_scope.txt` (5 classes, `rows` / `rows_in_scope`),
  and a `README.md` carrying the provenance, the parse rule, the bin rule, the
  in-scope filter and the data traps. **Every derived count matches plan §1.1
  and §1 above**: bins 75 / 61 (59 case + 2 trailing space) / 8 / 21 / 44 / 61 /
  33 = 303 pairs and 297 593 / 93 213 / 4 400 / 122 554 / 442 369 cells; the
  numeric bins re-derived in-scope give 37 / 16; in-scope 198 / 53 pairs over
  920 954 / 90 627 cells across 390 of the 433 divergent cases; shape 429 rows
  full, 149 in scope (generator 137, autotrans 7, windgen 5).
  Rules recorded in `README.md` and reproducible from the vendored files:
  the structural bin is decided by the pair's representative cell — its first
  census row, which is the cell the pair extracts print — through the chain
  bool → empty → case/trim → bracketed-array → enum/singleton; the numeric bin
  is `max_rel >= 1e-4`, a cut inside the census's empty band (largest display
  pair 6.43e-5 `load.pf`, smallest genuine jump 1.00e-3 `invcontrol.lpftau`).
  The in-scope filter is `engines ∈ {both, r4133}` joined by the census case
  label `"<manifest>:<path>"` → `manifests/solvable_now.json` or
  `tests/corpus/<family>/manifest.json`; all 438 census labels resolve and the
  union is exactly the 521-case `population.lock.json` population.
  One wiring edit outside the data: `.gitattributes` gains
  `tests/corpus/props_r4133/** -text` (the repo runs `core.autocrlf=true`, and
  an evidence directory whose whole claim is byte-faithfulness must not be
  EOL-rewritten on checkout). No manifest, ledger, golden or engine source
  moved. Gate: all five commands green.

  **Audit settlement (same day, two fresh auditors → fix agent; 9 findings, all
  settled, none dropped).** Every claim was re-measured against the local census
  (a streaming pass over all 1 055 880 rows joined to the four manifests), and
  every count below is now re-derivable from the vendored files alone and locked
  by a test.

  - **Two majors, both real, both fixed.** (1) *`bins.tsv`'s one bin per pair
    hides intra-pair mixing.* Classifying every cell instead of the pair's
    representative one puts 6 719 cells (0.70 % of the structural population,
    6 591 in scope) in a bin other than their pair's, over **17** structural
    pairs — **15** in scope, not just the three the plan discusses: e.g.
    `load.yearly` (bin 5, 5 995 case-only cells), `relay.switchedobj` (bin 2,
    102 in-scope `''` echoes), `line.wires`/`load.zipv`/`storage.dynadata`/
    `storagecontroller.seasontargets(low)` (bin 5, array-form cells),
    `capcontrol.type`/`invcontrol.voltage_curvex_ref`/`reactor.bus2` (enum cells
    in a case/empty pair). No data is lost — `examples_full.txt`'s per-pair
    multiset equals the census exactly — so this was a summarisation gap; the
    README now carries the full table plus the two consequences: RP2.1's
    admissible-claim assert must be **per example row**, never "the set the
    pair's bin allows", and the label is **order-dependent**, so RP0.2's
    acceptance compares order-invariant cell populations (`cells`,
    `cells_in_scope`, the `examples_full.txt` multiset), not the bin letter, on
    these 17 pairs. (2) *the "three mixed pairs" count is refuted by the
    evidence.* Of bin 1's 75 pairs, **nine** answer with an echo instead of a
    boolean — 3 834 echo cells, **3 293 in scope**: four genuinely mixed
    (`relay.reset` 226 foldable / 44 `0.20`, `relay.distreverse` 32 / 238 `''`,
    `recloser.eventlog` 30 / 200 `''`, `regcontrol.idle` 1 / 887 `''`) and five
    with **no** foldable cell at all (`regcontrol.idleforward` 888,
    `regcontrol.idlereverse` 888, `capcontrol.reset` 446, `recloser.debugtrace`
    230, `upfccontrol.enabled` 13). `relay.reset`'s echo is a stale parse string,
    so it is an `EchoParse` row, not the `EchoDefault` shape the other three
    take; and bin 1 overstates the BoolFold population by the five pure-echo
    pairs. RP2.3 must therefore provision **nine** echo rows, not three — the
    six unnamed pairs carry 2 125 in-scope echo cells (the plan's three carry
    1 168) that would otherwise reach the RP2.1 liveness assert unclaimed.
  - **Minors fixed in the evidence.** The README's EOL sentence was false
    for `triage.md` (LF in the source too — byte-identity itself held, verified
    by SHA-256 on all five copies); the in-scope filter now records that 4 of
    its 462 cases (`shape_binfiles`, `IEEE13_LineAndCableSpacing`,
    `IEEE13_LineSpacing`, `line_spacing_asym`) carry a channel-scoped
    `kind: "skip"` on r4133 in `ledger.json`, so the r4133 compare never reaches
    them — zero cells today, recorded as a definition; the in-scope shape split
    (149 = generator 137 + autotrans 7 + windgen 5), which WP-RP1's acceptance
    is measured against, is vendored as `shape_in_scope.txt` instead of living
    only in the gitignored census; and `tests/corpus/README.md` gained the
    directory in its inventory.
  - **Minors fixed by wiring.** The evidence had no lock at all:
    `crates/dss-core/tests/props_r4133_evidence_lock.rs` (8 tests, both lanes,
    no oracle) now pins SHA-256 + length over the five verbatim copies (24 944
    bytes total) and the `-text` stanza that keeps them raw, the row counts of
    every derived file, the cross-file equalities (`bins.tsv` ↔ pair files ↔
    `examples_full.txt` ↔ in-scope files ↔ `shape_in_scope.txt`, including the
    per-pair cell sums and the 1 055 446 total), the §1.1 per-bin totals with
    the in-scope 198 and the re-derived numeric 37/16, and — re-derived from the
    vendored rows — the two corrected traps above (the 17-pair heterogeneity
    table and the nine bin-1 echo pairs), so the corrections are machine-checked
    rather than prose. Non-vacuity probed: deleting one `examples_full.txt` row
    fails 3 of the 8, flipping one byte of `triage.md` fails the digest.
    `golden_lock.rs::EXCLUDED_TREES` gains the tree with its reason (it is
    outside the §1.2 enumeration deliberately: frozen evidence, no anchor
    describes it, and it is locked by the file above), and
    `oracle_parity_cfg_gate.rs::operational_docs` now names
    `tests/corpus/props_r4133/README.md` individually — the `tests/corpus` walk
    filter drops non-manifest files because upstream's vendored docs are not
    ours, which is exactly false for this one. `TESTING.md` gains the lock's
    section next to the golden/population locks, and `golden.lock.json`'s
    comment was regenerated (`DSS_UPDATE_GOLDEN_LOCK=1`) for the new exclusion —
    comment line only, **no digest moved**.
  - **Roll call (each finding → where it landed).** audit-code: intra-pair bin
    mixing → major (1); `triage.md` EOL → evidence; `relay.reset` as a fourth
    mixed pair → major (2); ledger `kind: skip` cases inside the filter →
    evidence. audit-tests: the refuted mixed-pair count → major (2); no lock on
    the evidence → wiring; representative-cell / order-dependent bin → major
    (1); in-scope shape not vendored → evidence; `tests/corpus/README.md`
    inventory + the doc-gate filter → wiring.
  - **Nothing refuted, nothing deferred silently.** The one item that cannot be
    fixed inside RP0.1's scope is the **plan text**: `R4133_PROPS_PLAN.md`
    §1.1's bin-1 table row, the §1.2 replay bullet, the RP0.1 spec sentence,
    RP2.1's `BoolFold` sizing + claim-semantics sentence and RP2.3's echo-row
    list said "three mixed pairs", and RP2.1's admissibility sentence read as
    per-pair-bin. WP-RP0's rule is stop-and-report on a mismatch between the
    vendored evidence and the plan: reported, then **resolved in the sub-step's
    closing docs commit** — all six sites amended: the §1.1 bin-1 row carries
    the nine-pair correction (four mixed incl. `relay.reset`'s `'0.20'`
    `EchoParse`, five pure-echo, 70 `BoolFold` pairs), §1.2's replay bullet
    states per-example-row admissibility over the 17 heterogeneous pairs (the
    `bins.tsv` label is a representative-cell summary, never the admissibility
    authority), the RP0.1 spec is annotated as-executed, RP2.1 sizes `BoolFold`
    at 70 pairs and declares the five pure-echo pairs for RP2.3 wholesale,
    RP2.3 provisions nine echo rows, and RP0.2's acceptance compares
    order-invariant cell multisets.

- **RP0.2** (2026-08-22) — the census is a permanent knob. `DSS_PROPS_CENSUS=1`
  arms the corpus-gate binary's own `corpus_gate_props_census` test, which walks
  every live non-`large` case (438 — exactly the
  population the 2026-08-08 scratch census walked) on **both** channels with
  `all_properties` forced on, bypassing the §1.1 r4133 masks, compares with the
  plain (un-normalized) comparator in collect-don't-panic mode and writes
  `tmp/props_census.json` + `tmp/props_census/run.json` +
  `tmp/props_census/<channel>/{structural_pairs,numeric_pairs,examples_full,
  shape,summary}` in the RP0.1 extract format. It **asserts nothing** — a
  divergence is the measurement, never a failure — and honors `DSS_GATE_ONLY`
  (an empty match still refuses, as on the gate; the filter is stamped into the
  artifacts). Files:
  `crates/dss-core/tests/corpus_gate/props_census.rs` (row model + extract
  writers, 12 unit tests pinning the vendored row formats, the 40/34-char example
  cuts, Python's `%.2e`, the `Line.b1||b2` pair key, `examples_full`'s ordering,
  the escape/parse round-trip, the heterogeneous-shape counter, the streamed JSON
  document, the loud mode parser, the `?`-surface stability argument and — the
  non-vacuity guard — that the collecting walk is silent on a faithful capture,
  emits exactly one row for one corrupted cell, a shape row plus an unaligned
  count for a dropped prop, and a counted skip for a capi-channel Relay), the
  walk in `scheduler.rs::run_props_census`/`census_one`, the trigger in
  `corpus_gate.rs`, `TESTING.md` (env-var row + a re-measurement procedure next
  to the evidence lock). `DSS_LIVE_PROPS`, the gate's r4133 property mask
  (`scheduler.rs::run_one_case`, `:364`) and the seeding one
  (`::seed_one`, `:720`) are untouched — both still read
  `cc.compare_all_properties = false` on the r4133 channel; knob OFF is today's
  gate exactly, proven by the five-command gate being green.
  - **The census is its own `#[test]`.** `corpus_gate_props_census` is armed by
    the env var; `corpus_gate_all_cases_match_engines` always runs the gate. The
    first cut dispatched the *mandatory* test on `DSS_PROPS_CENSUS`, so a stray
    `=1` in a shell or CI environment turned the one live corpus comparison into
    a green no-op (audit-tests, reproduced). Unset, the census test is a no-op.
  - **One comparator seam — the per-cell decision and the element walk.**
    `harness::value_verdict` is the single per-cell decision
    (`Match`/`Skeleton`/`NumberCount`/`Numeric{max_rel}`);
    `assert_value_matches_tol` is now its asserting wrapper (failure text
    byte-preserved) and `harness::collect_element_divergences` its collecting
    one. The pre-walk policy is shared verbatim too — `oracle_name_set`,
    `filter_015x` (the `PROPS_015X` relief) and `transformer_cursors_disagree`
    are single functions both walks call, after the audit found each of them
    written twice. What remains structurally separate is the loop body itself
    (aborting vs collecting), and it is pinned rather than trusted:
    `census_walk_tests::the_two_walks_agree_on_every_input` runs both over one
    input table and asserts the biconditional *`compare_prop_lists` panics **iff**
    the census reports a row or a blind spot* — the re-ordering case is why the
    blind-spot term is needed (a permutation leaves counts and name sets equal,
    so the census reports two uncomparable cells and no row). Both walks take the
    allowlist as a parameter, so both are testable with an injected table.
    Still owed by RP2.1, and NOT pre-built here: threading `Mode` and a ledger
    view from `run_props_census` → `census_one` → the walk, and the
    normalize → echo → floor → ledger stage the disposition mode adds. Two
    deliberate differences from the gate, both reproducing the scratch census:
    nothing panics, and the Recloser/Relay whole-element skips are
    channel-scoped (capi only —
    plan §1.2), which is where the census's 22 relay/recloser pairs come from.
    `max_rel` is `|a−e|/|e|` (or `|a−e|` when `e == 0`) maximized over the
    numbers that FAIL the floor, pinned by a `harness::comparator_tests` case.
    Its **verification breadth** was overstated in the first cut and is corrected
    here (audit-code, major): the formula reproduces all **95 317**
    `value_numeric` rows of the vendored census at each case's tier floor, but
    95 180 of those carry a SINGLE number, where the "over the offenders" and
    "over all numbers" readings cannot differ, and only **137** carry more (2:92,
    3:26, 4:4, 6:14, 10:1). Of the 137 exactly ONE discriminates them —
    `Line.l1` `cmatrix` on `modes:upgrade/upgrade_spacing_ratings.dss`, recorded
    as `5.833551103975013e-8` (offender-only, at the deck's `micro` floor) and
    not `1.1866505782192496e-7` (over all six numbers). The earlier claim "94 203
    multi-number rows", repeated in three permanent artifacts, matched no
    population in the census at all and is retracted at each site. The
    refactor's non-vacuity is the existing `props_015x_tests` (a value mismatch,
    a missing prop and a misordered prop each still panic) plus the census-side
    guard above. RP2.1 extends this seam for `DSS_PROPS_CENSUS=claims`; the mode
    parser rejects `claims` loudly today rather than silently handing back a
    plain census.
  - **Acceptance (the plan's family-bounded form).** `DSS_GATE_ONLY=modes:windgen`,
    both channels, 5 cases: the knob's 74 r4133 rows equal the local full
    census's 74 rows for that family as cell multisets — and in fact **row-for-row
    in order**. The three pairs whose entire census population lies inside the
    family reproduce their vendored extract lines byte-for-byte, `%.2e` and all:
    `windgen.kva | '3157.89473684211' | '3157.89' | 2.00e-06 | 2`,
    `windgen.kvar | '986.05231553659' | '0' | 9.86e+02 | 4`,
    `windgen.mva | '3.15789473684211' | '3.15789' | 2.00e-06 | 2`. The capi
    channel on those five decks records 5 `oracle_error` rows (0.14.5 has no
    `WindGen` class) instead of failing — the collect contract working.
    **WP-RP0 is closed**: the knob reproduces the vendored rows for the
    spot-checked family on both channels.
  - **Full re-census (not owed, run anyway: 438 cases × 2 channels in 58 s;
    re-run after the audit fixes, r4133 row count unmoved).** 1 056 790 r4133
    rows vs the vendored 1 055 880; 433 divergent cases = 433, 94 numeric
    pairs = 94, 5 shape classes = 5 with all five `shape.txt` rows
    byte-identical. The pair extracts, measured row-by-row (the first cut's
    "209 of 210 structural pairs identical including their example cells and
    counts" was too strong and contradicted this bullet's own finding 2 — audit-
    tests, corrected): **202** of the 209 vendored structural rows are
    byte-identical, **7** carry the +2 count shift of finding 2
    (`transformer.bhcurrent/bhflux/enabled/ratings/sub/xrconst/xscarray`) and
    **1** row is new (`regcontrol.fwdthreshold`, finding 1); of the 94 numeric
    rows **90** are byte-identical and **4** carry the same +2
    (`emergamps/normamps/pctperm/repair`). `examples_full.txt` (newly re-derived
    by the knob) lands **3 349 of 3 378 rows byte-identical and in position**;
    the 29 that move are all `transformer.*` — the finding-2 count bumps
    (Σ = exactly the +22 cells that finding predicts) plus the tie re-orderings
    they cause inside three pairs — with 2 new `regcontrol.fwdthreshold` rows and
    the one `storage.dynadll` row that differs only in backslash escaping (see
    finding 5). The residual is five findings, none of them a knob defect (per
    the vendored `README.md`, a disagreement is a finding, not a rewrite —
    RP0.1's frozen files are NOT touched):
    1. **`regcontrol.fwdthreshold` (888 cells, a whole new bin-5 pair) is missing
       from the vendored census.** r4133 `Controls/RegControl.pas` initializes
       `PropertyValue[1..32]` and stops (`:1423-1459`) while `GetPropertyValue`
       overrides only index 28 (`:820-828`), so props 33–36 — `idle`,
       `idleReverse`, `idleForward` **and** `fwdThreshold` — all answer with the
       `''` echo. The census records the first three at 888 cells each and omits
       the fourth; the port renders `100`. The omission is internally
       inconsistent with its own three siblings, so the pair is real: RP2.3 must
       provision an `EchoDefault` row for it (bin 5 goes 44 → 45 pairs,
       structural 209 → 210 / 960 129 → 961 031 cells). **Consequence for
       RP2.1**: `examples_full.txt` has no `regcontrol.fwdthreshold` row either,
       so that echo row would claim nothing in the offline replay and trip the
       both-ways liveness assert. The knob now measures the missing spellings
       exactly — its `examples_full.txt` carries `regcontrol.fwdthreshold |
       '100' | '' | 864` and `| '800' | '' | 24` (Σ 888) — so RP2.1/RP2.3 must
       either add those two rows to the replay's input or exempt the
       row with this record as its citation. **Decided in the closing docs
       pass (2026-08-22): RP2.1 vendors `examples_supplement.txt` with those
       two rows (provenance = this record) and feeds the replay from both
       files; the plan's §1.1 bin-5 row, §1.2 replay bullet, RP2.1 and RP2.3
       are amended accordingly.** The frozen RP0.1 files stay untouched.
    2. **The two cursor-disagreement transformers are absent whole-element**
       (`asymmetric:transformer/transformer_asym.dss` `Transformer.t3w`,
       `solvable_now:Test/YgD-Test.dss` `Transformer.tr1` — exactly the two the
       harness names at `TRANSFORMER_CURSOR_PROPS`). The scratch census dropped
       the entire element on a cursor disagreement; the gate — and therefore the
       knob — drops only the 13 cursor-contaminated props, so 11 further props
       compare on each: +2 cells on 7 structural pairs (`transformer.bhcurrent/
       bhflux/enabled/ratings/sub/xrconst/xscarray`) and 4 numeric ones
       (`emergamps/normamps/pctperm/repair`, max_rel unmoved). No new pair.
    3. **The 5 `oracle_error` rows differ only in the DLL load address** inside
       the r4133 access-violation text (`…50A3CE5E` vs `…54ADCE5E`, same
       `offset 41CE5E`): ASLR, not a divergence. Same five cases, same #303.
    4. **The vendored `README.md`'s "the walk covered ~512 live cases; 438 of
       them left a row here" is wrong** (audit-code; the fourth finding against
       the frozen evidence, and the plan §1.1 already says 438). Measured from
       the four manifests: 521 cases, 517 live, **438** live non-`large` — and
       that set is EXACTLY the census's 438 case labels (symmetric difference 0).
       So every case the 2026-08-08 walk touched left a row, and the 79 `large`
       decks left none, i.e. were not walked (they are excluded from property
       forcing on the gate too). The knob measures and prints the same 438.
       Recorded, not fixed by RP0.2: the README is RP0.1's frozen evidence and
       RP0.2 does not edit it — it is the outlier against both the plan and the
       data. **Fixed in the closing docs pass (2026-08-22): the README sentence
       now states the measured 438-of-438 population, with the original wording
       quoted in place.**
    5. **The vendored extracts spell the same cell two ways, and the README
       promises more re-derivation than the plain census can give** (audit-code).
       (a) The generator escaped backslashes in `structural_pairs.txt` (Python
       `repr`) but not in `examples_full.txt` / the `*_in_scope` files, so
       `storage.dynadll` reads `'C:\\Users\\…'` in one and `'C:\Users\…'` in the
       other — a live trap for RP2.1's replay, which reads `examples_full.txt`.
       The knob uses ONE quoting for every extract (escaped), which is the single
       row where its `examples_full.txt` deviates from the vendored copy.
       (b) The README's provenance sentence says the knob writes "these same
       extracts", i.e. all of them. RP0.2 now also emits `examples_full.txt` (the
       file RP2.1 actually replays) — but `bins.tsv` needs the §1.1 bin policy
       and the three `*_in_scope` files need the in-scope case filter, neither of
       which the plain census carries; both belong to RP2.1's disposition mode.
       **Fixed in the closing docs pass (2026-08-22): the README's provenance
       paragraph now states what the knob writes as landed (incl.
       `examples_full.txt` and `run.json`), that `bins.tsv`/`*_in_scope` wait
       on RP2.1's `claims` mode, and the uniform-escaping deviation; the
       `storage.dynadll` two-spellings trap is called out in the data-traps
       section with RP2.1 named as the consumer at risk.**
  - **Three things the vendored census could not report, now measured.** The
    `channels` block carries `unaligned_cells` — cells no index-ordered compare
    can reach because the two name lists desynchronized earlier in the element
    (r4133: **10 776**; capi: 0). That is why the vendored
    value population of the five shape-gap classes is a **lower bound**: e.g.
    `windgen.dynout` and `windgen.enabled` sit past r4133's `usermodel`/`userdata`
    insertion at 18/19 and are invisible until WP-RP1 closes the gap (its
    acceptance, "zero `shape_count` rows", is therefore also what unlocks the
    tail). And the **capi channel**, which the r4133-only scratch census never
    walked, comes back with 3 structural + 13 numeric pairs over 34 cases and 21
    `oracle_error`s — all of it outside the gate's own capi compare by
    construction: the census runs ledger-free (the gate's `property` ledger
    scopes are not applied) and forces properties on cases the gate never sends
    to capi at all (e.g. the `engines: r4133` `controls:swtcontrol/*` decks, whose
    `SwtControl.Normal` reads `closed` vs 0.14.5's `open`). And the capi
    channel's `unaligned_cells: 0` used to read as "complete" while whole
    Recloser/Relay elements were dropped there uncounted (audit-code): the block
    now also carries `skipped_elements`/`skipped_element_cells` — capi **312**
    elements / **13 520** cells, r4133 0/0 — plus
    `heterogeneous_shape_classes` (0 on both; a nonzero value would mean
    `shape.txt`'s one-row-per-class format is hiding a second shape, and prints a
    banner). No gate signal.
  - **Audit settlement (2026-08-22, 14 findings from audit-code + audit-tests;
    nothing dropped, nothing refuted).** Both majors and ten of the twelve minors
    are FIXED; one minor is split fixed + recorded (9), one is recorded-only (5),
    and major 2 keeps a recorded remainder the plan assigns to RP2.1:
    (1) major, the `max_rel` "94 203 multi-number rows" overclaim — **fixed** at
    all three sites, re-measured over the local census (95 317 numeric rows, 137
    multi-number, one discriminating cell) with the emulation cross-validated
    against the census's own kind partition (0/960 128 structure rows have equal
    skeletons, 0/95 317 numeric rows have unequal ones).
    (2) major, "the RP2.1 seam is a second copy of `compare_prop_lists`" —
    **fixed** as far as RP0.2 owns it: the three duplicated pre-walk policy
    expressions are now shared functions, the collecting walk takes an injectable
    allowlist, and the two loop bodies are pinned to each other by a
    biconditional test; the `Mode` + ledger threading is **recorded** as RP2.1's
    (the plan assigns the disposition mode there, and a one-variant enum threaded
    through three frames would be untestable plumbing).
    (3) `quote()`'s `\'` breaking the documented parse regex — **fixed**
    (`\x27`, with a round-trip test through the README's own regex rule).
    (4) the `NO_STEP` doc's "`-1` is what the vendored census uses" — **fixed**
    (the five `oracle_error` rows carry `['case','detail','kind']` and no `step`;
    the knob never serializes one either).
    (5) the README's "~512 live cases" — **recorded** as finding 4 above (frozen
    evidence; the measured population is 438 and the plan already says so).
    (6) a malformed oracle payload filed as `rust_error` — **fixed**
    (deserialization moved out of the Rust-side `catch_unwind`).
    (7) capi `unaligned_cells: 0` hiding whole-element skips — **fixed** (the two
    new counters above).
    (8) "the census walks solve→properties, the gate walks it last" — **fixed by
    demonstration**: every `dss.command` `compare_capture` issues between `solve`
    and the property block is a `? Element.Prop` QUERY (`compare_probe`, the
    ledger's probe handlers; the ledger's property rewrite is inside the block),
    `compare_export` compares two strings without touching the engine, a `?`
    query is a read, and the one property-visible cursor (`ActiveWinding`) is
    written only by property SETTERS while `element_properties` re-selects the
    object and refreshes `Vterminal` itself; the permanent test
    `the_props_surface_is_stable_across_the_gates_intervening_comparators`
    replays that traffic on a 3-winding transformer and asserts the `?`-surface
    is byte-identical.
    (9) capi extracts headed `r4133-example` + the un-regenerable derived files —
    **fixed** (headers name their channel; `examples_full.txt` is now emitted and
    reproduces 3 349/3 378 vendored rows in position) and **recorded** for
    `bins.tsv`/`*_in_scope`, which the plain census cannot derive (finding 5).
    (10) audit-tests' restatement of (2), incl. the gate's count-only assert vs
    the census's name-set compare — **fixed** by the same biconditional pin,
    which is what proves the two statements coincide.
    (11) the overstated full-re-census agreement — **fixed** (202/7/1 and 90/4,
    re-measured).
    (12) a stray `DSS_PROPS_CENSUS=1` greening the mandatory gate — **fixed**
    (own `#[test]`).
    (13) artifacts overwriting a fixed `tmp/` path with no record of the filter —
    **fixed** (`gate_only` in the census header and in a new
    `tmp/props_census/run.json`).
    (14) `shape.txt` keeping only the first shape per class — **fixed** as
    reporting: the format stays vendored-faithful (the JSON is the lossless
    record) and `heterogeneous_shape_classes` + a banner make the loss loud.
  - Gate: all five commands green. `lane_diff.ps1` not owed — no solved state,
    no compat kernel; the knob is test-harness code and inert unless the env var
    is set.

### R4133_PROPS WP-RP1 — condensed records

> Plan: `R4133_PROPS_PLAN.md` §WP-RP1. Same branch, same per-sub-step ritual.

- **RP1.1** (2026-08-22) — the three r4133 **upstream-stub** property rows.
  Generator `Rneut`/`Xneut` at display slots 16/17 and Sensor `Action` at slot
  13 now exist in the port, closing the two shape gaps `shape.txt` records
  (generator 48 → 50 names, sensor 15 → 16).
  - **Mechanism — one new flag, one new `PropDef` field.**
    `PropFlags::UPSTREAM_STUB` (bit 85) marks a property r4133 still *registers*
    but no longer implements. `ClassProps::parse_into` handles it ahead of the
    `ptype` match: store the raw parse string through the class's own
    `set_string`, then emit `PropDef::stub_message` if the row carries one, and
    return — never a parse error, no side effect, no recalc. That reproduces
    r4133 exactly: its Edit loop assigns `PropertyValue[]` for every property
    *before* the arm runs (`generator.pas:625`, `Sensor.pas:253`), and the arm
    then either logs a soft `DoSimpleMsg` (5611/5612, `generator.pas:651-652`)
    or does nothing (`TSensorObj.Set_Action` is an empty body,
    `Sensor.pas:850-854`). `PropFlags::NOT_PORTED` could not be reused — it
    hard-errors the write and hides the row from JSON. The per-class
    side-effect hook is the `stub_message` field itself (code + text), so
    Generator logs and Sensor stays silent with no per-class code at all;
    `ClassProps::new` `debug_assert`s that a `stub_message` implies the flag.
  - **Storage.** Plain `String` fields (`Generator::rneut_text`/`xneut_text`,
    default `"0"` per `generator.pas:2567-2568`; `Sensor::action_text`, default
    `""` per `Sensor.pas:807`), read/written by the classes' `get_string`/
    `set_string` arms and copied by `make_like` — r4133's `MakeLike` copies the
    donor's whole property-string array (`generator.pas:830-831`, right after the
    `ClassMakeLike` at `:828`; `Sensor.pas:428`),
    and these are the only properties where that copy is observable in a port
    that otherwise renders live fields.
  - **Ordinal shifts.** Generator `NUM_PROPS` 48 → 50 (`STATUS`..`ENABLED`
    +2), Sensor 15 → 16 (`BASE_FREQ`/`ENABLED` +1). Every `prop::` consumer is
    symbolic (checked by grep across the workspace: no `generator::prop`/
    `sensor::prop` reference outside the two modules). The raw-slot **prose**
    that remains — `make_pos_sequence`'s doc and the four `makeposseq_*` pins in
    `generator/tests.rs` (`slot 19`/`slot 20`, "r4133 slot 27", `PrpSequence^[26]`,
    "dss_capi's stale had_kVA index") — is all in r4133's *CmdMapIndex* space,
    the space old-OpenDSS keys `PropertyValue`/`PrpSequence` by, so the shift
    leaves it correct; both sites now say so explicitly (the audit round added
    the label to `tests.rs`, where the earlier claim that the doc was the only
    such prose was wrong).
  - **Surfaces.** All three rows also carry `PropFlags::HIDE_R4133` — they are
    absent from BOTH pinned captures (dss_capi 0.14.5 deleted Generator's pair
    outright and commented Sensor's out, `.inputs/dss_capi/src/Meters/
    Sensor.pas:39,55`) — so Dump / `Dump commands` / AltDSS JSON / the schema
    output skip them while `?` and the props table expose them. That re-arms a
    flag which had been carrier-free since WP-U2.5; the F.3aa blast-radius
    measurement still isolates its Line/LineGeometry population because the two
    hide flags' carrier sets are now asserted **class-disjoint**. The three rows
    join `PROPS_015X` (name-based, so on the r4133 channel — whose oracle name
    list *does* carry them — `filter_015x` keeps them and they compare in full).
  - **Goldens: measured, not moved.** `tests/golden/props/{generator,sensor}.json`
    do **not** change — `gen_props.py` enumerates the *oracle's*
    `AllPropertyNames` (capi 0.14.5), which can never learn an r4133-only name.
    Measured, not assumed: those two classes' 8 + 6 scenarios were re-run
    through the pinned dss-python (0.15.7 / backend 0.14.5) and compared to the
    committed records — **identical**. (A whole-file `gen_props.py` run is
    impossible in the pinned environment and was so before this sub-step:
    `props/regcontrol.json` is `capi015`-anchored and its `idle=` scenario
    raises #110 on the 0.14.5 backend.) `PROPS_CLASS_FILES`/`PROPS_SCENARIOS`/
    `PROPS_PROPERTY_CELLS` therefore stay 51/322/8343.
    Two `json/` schema artifacts *do* move, both predicted by §1.2: three new
    `port_hidden_property` rows in `schema_divergences.json` — Generator
    `Rneut` index 16 / order **17** and `Xneut` 17 / **18**, Sensor `Action`
    13 / **13** (both fields are the prop's OWN ranks, the convention the four
    Line rows use; on Generator `Like`'s hoist to order 1 puts every order one
    above its index — `Conn` 15/16, `Status` 18/19 — while on Sensor the
    `BooleanAction` `Clear` hoisted to the end cancels that, so order == index:
    `Weight` 12/12, `BaseFreq` 14/14) —
    after which `ported_class_defs_bytes_match_oracle` and
    `full_document_reconciles_with_oracle` pass untouched; and
    `schema_full_port.json` regenerated with `REGEN_SCHEMA_PORT=1`, whose diff
    is **exactly 35 `$dssPropertyIndex` + 35 `$dssPropertyOrder` renumberings**
    inside the Generator and Sensor regions and nothing else (no property block
    added or removed). `golden.lock.json` moved exactly those two digests, in
    this commit. No other golden byte moved anywhere.
  - **Measurement (RP0.2 census knob, `DSS_GATE_ONLY="IEEE 30 Bus"` — a
    5-generator `both`-channel case), before vs after:**
    r4133 shape classes **1 → 0** and cells uncomparable behind a desynchronized
    name list **175 → 0** (before: `generator: rust_count=48 oracle_count=50
    oracle_only=['rneut','xneut']`); the 175 newly comparable cells surface as
    structural pairs 23 → 29 and numeric 15 → 16, which is the WP-RP2/RP3
    workload the shape closure exposes, not a regression. The **capi_v0145
    channel is bit-unchanged**: 0 structural / 0 numeric / 0 shape / 0
    uncomparable, before and after.
  - **Tests.** `exec::tests::upstream_stubs` (new module, 8 tests) pins
    store+echo+message ordering and the `DoSimpleMsg`-not-abort severity, the
    silent Sensor write, the non-numeric round-trip, the display slots
    (`…DispValue, Conn, Rneut, Xneut, Status…` / `…Weight, Action, BaseFreq,
    Enabled` with the two table lengths), the `MakeLike` copy, the Dump/JSON
    absence and the `Save` *presence* (r4133's `SaveWrite` is flag-blind, so a
    deck that wrote `rneut=` gets it back). The "write does not change behavior"
    acceptance is per class:
    `generator_neutral_stubs_move_neither_y_nor_the_solution` — two *independent*
    solves of the same micro deck differing only in whether `rneut=`/`xneut=`
    were written give **bit-identical** Y triplets, iteration count and node
    voltages (the A/B pair rather than a re-`Solve` because re-solving from a
    converged state drifts ~1e-8 for reasons unrelated to these properties),
    plus an in-place `Edit` **followed by `Solve`** — the rebuild is what makes
    that arm discriminate, since `system_y_csc` reads the assembled matrix and
    `CalcYPrim` runs from `BuildYMatrix`, not from `RecalcElementData` — with a
    control write of three live Generator props that must move the rebuilt Y;
    and `sensor_action_moves_neither_sensor_state_nor_the_solution` — the same
    A/B shape plus the sensor's whole property table (its estimator input set)
    bit-identical outside the `Action` cell.
    `compat_quirks::upstream_stub_rows_are_the_measured_set` locks the flag's
    carrier set, that only the Generator pair carries a message (5611/5612), and
    that every carrier is also `HIDE_R4133`;
    `hide_015x_carrier_set_is_the_measured_escape` gained the `HIDE_R4133`
    carrier list and the class-disjointness assert;
    `props_r4133_evidence_lock::rp1_1_closes_the_generator_and_sensor_shape_gaps`
    reads the closure back off the vendored `shape.txt` itself (live table
    length == the census's `oracle_count`, the `oracle_only` names present, the
    frozen `rust_count` still the pre-fix number).
  - **Audit settlement (2026-08-22, `/audit-code` + `/audit-tests`, 10 findings:
    3 major, 7 minor — 10 fixed, 0 refuted, 0 deliberately unfixed).**
    (1) *major, code* — the three new `schema_divergences.json` rows carried an
    off-by-one `$dssPropertyOrder` (16/17/12 where the port's own ranks are
    17/18/13), invisible to the gate because `renumber_field` shifts by
    `count(removed < v)` and the slot below each was occupied. **Fixed** in the
    artifact, its `cause` prose (which now also records the own-rank convention
    and why an error there is absorbed) and in this record; verified against
    `schema_full_port.json` (`Conn` 15/16, `Status` 18/19; `Weight` 12/12,
    `BaseFreq` 14/14) and the Line precedent (`EpsRMedium` 31/27).
    (2) *major, code* — the shape closure makes value pairs live that the frozen
    2026-08-08 extracts structurally cannot contain (while a class carried a
    `shape_count` row its walk stopped at the name disagreement), and nothing
    opened a supplement item. **Fixed by measurement**: a full `DSS_PROPS_CENSUS=1`
    re-run on the post-RP1.1 tree (438 cases, both channels, 56 s) gives r4133
    shape classes **5 → 3**, structural **218**, numeric **98** (frozen extracts:
    209 / 94; the knob's own pre-RP1.1 re-census: 210 / 94), capi channel
    unchanged — so RP1.1 adds exactly **12** pairs and removes none:
    `generator.{debugtrace, enabled, status, dynamiceq, dynout, shaftdata,
    userdata}` + `sensor.enabled` (bins 1/2/5),
    `generator.{kva, maxkvar, minkvar}` (bin 6, max_rel ≤ 5.8e-6 — an order of
    magnitude under the worst display pair) and **`generator.d`** (`'1'` vs
    `'0'`, rel 1.00e+00) in **bin 7**, which grows the in-scope bin-7 population
    16 → 17 and un-closes §1.1's enumerated four-root-cause list. Its cause is already Pascal-read and is an
    **echo**, not a jump: `TGeneratorObj.Create` sets `GenVars.D := 1.0` and
    never `Dpu` (`generator.pas:955-971`), while `InitPropertyValues` echoes
    `Format('%-g', [GenVars.Dpu])` (`:2585`) — RP2.2 triage → an RP2.3 echo row.
    Recorded in the vendored `README.md` §"Pairs the WP-RP1 shape closures make
    live" (with the per-pair table), and the plan now (a) carries the correction
    in §1.1, (b) points RP2.1's `examples_supplement.txt` at it, (c) makes the
    re-measurement a per-sub-step obligation of WP-RP1 (autotrans/windgen next).
    (3) *major, tests* — arm 2 of the Generator acceptance was **vacuous**:
    `system_y_csc` reads the already-assembled `solution.y_system` and
    `CalcYPrim` runs only from `BuildYMatrix` on `Solve`, so "Edit then compare
    Y" held for every property. **Fixed**: the arm now re-`Solve`s, and a control
    write of three live Generator props asserts the rebuilt Y *does* move on that
    deck.
    (4) *minor, code* — `UPSTREAM_STUB` is only sound for `PropType::String`
    (`get_value` dispatches on `ptype`), which nothing enforced and the parse
    comment contradicted. **Fixed**: a second `debug_assert` in `ClassProps::new`,
    corrected comment, and the invariant stated on the flag.
    (5) *minor, code* — the re-armed `HIDE_R4133` escape had no owning row and no
    measured blast radius, unlike its `HIDE_015X` precedent. **Fixed by
    measurement**: disabling the flag's arm and running `cargo test -p dss-core
    --no-fail-fast` moves exactly 4 committed artifacts
    (`json/der_usermodel_{assigned,full}.json`, `json/dyneq_full.json`,
    `reports/dump3_commands.txt` +6 rows) plus `schema_full_port.json` and the
    3 `port_hidden_property` rows, and **no corpus case** (live gate green). Row
    added to `ORPHANED_GAPS.md` §2 naming GOLDEN_REBASE **G3.3c + G3.4** as the
    unblocking sub-steps (the schema half never unblocks: `schema_full_oracle` +
    `schema_divergences` stay frozen); the numbers also live on the flag's doc.
    The un-hide would render real text, not a key-on-miss fallback: all three
    help strings have been in `report/help_catalog.rs` since U2.5.
    *(These figures are the three-carrier measurement and are superseded by
    RP1.2's audit item (1): with the fourth carrier the un-hide moves 8
    artifacts, `dump3_commands` +7 rows, 4 `port_hidden_property` rows — corpus
    still untouched.)*
    (6) *minor, tests* — `prop_flags.rs`'s `HIDE_015X` doc still claimed the
    sibling has "zero carriers today", the empirical basis of `UPGRADE_PLAN` §5's
    unreachability argument. **Fixed** (restated as the U2.5→RP1.1 era, which is
    when the measurement was taken).
    (7) *minor, tests* — `Save` prints the three names and nothing decided or
    tested that. **Settled as correct and pinned**: r4133's `TDSSObject.SaveWrite`
    walks `PrpSequence` with no flag filter (`General/DSSObject.pas:131-165`), so
    a deck that set `rneut=` gets `Rneut=` back from r4133 too — the port matches
    the authority. New pin `save_writes_the_stub_names_like_r4133`; the cost (such
    a deck is not re-compilable by the 0.14.5 backend) is recorded on the flag and
    in `TOLERANCE_NOTES`. Inert: no corpus deck and no `save*` golden writes any
    of the three.
    (8) *minor, tests* — no "write changes nothing" pin for Sensor `Action`.
    **Fixed** (`sensor_action_moves_neither_sensor_state_nor_the_solution`).
    (9) *minor, code* — the `MakeLike` citation `generator.pas:828` is the
    `ClassMakeLike` call; the property-array copy is `:830-831` (**fixed** in all
    four places), and this record's claim that `make_pos_sequence`'s doc was the
    only raw-slot prose was false — `generator/tests.rs` carries four more
    (all upstream `CmdMapIndex` space, all still correct; **fixed** by labelling
    them and correcting the sentence above).
    (10) *minor, code* — two active docs still described the pre-RP1.1
    carrier-free `HIDE_R4133` assert: `DE_PASCALIZE_PLAN.md`'s tripwire
    description and `era-summaries.md`'s "no live application site after U2.5".
    **Fixed** — the plan now describes what the live test actually checks (the
    sibling's carrier set + the two flags' class-disjointness); the two phase
    records got as-of-U2.5 notes rather than rewrites, so no two places
    disagree. (This item was dropped from the first cut of this enumeration,
    which read "9 findings" — restored by the coordinator's closing review.)
  - Gate: all five commands green, re-run after the settlement.
    `lane_diff.ps1` not owed — no solved state and no compat kernel moved
    (proved by the bit-identical A/B above).

- **RP1.2** (2026-08-22) — AutoTrans `XfmrCode`, a **real behavioral port**.
  r4133 property 39 (`Version8/Source/PDElements/AutoTrans.pas:329`, help `:414`)
  and `TAutoTransObj.FetchXfmrCode` (`:520` → `:2339-2396`) now exist in the
  port, closing the third `shape.txt` gap (autotrans 52 → 53 names). dss_capi
  0.14.5 deleted the row outright (`.inputs/dss_capi/src/PDElements/`
  `AutoTrans.pas:76,125`), so there is no capi witness for any of it.
  - **The copy is written field by field, not reused.** The Transformer's
    `fetch_xfmr_code` (`pd/transformer/windings.rs:357-402`) assigns the winding
    vector wholesale and copies the 0.15.x kVA-ratings; neither is r4133's auto
    form. The AutoTrans variant (`pd/auto_trans/windings.rs`) follows the Pascal
    statement by statement, with its three deviations from the Transformer
    routine: the connection override (winding 1 → `SERIES`, winding 2 → `WYE`,
    tertiary and beyond keep the code's own connection, `:2356-2361`), the
    reactance rename (`XHL/XHT/XLT` → `puXHX/puXHT/puXXT`, `:2374-2376`) and
    `RdcSpecified := TRUE` on every winding (`:2367`), which selects
    `RecalcElementData`'s `Rdcpu := RdcOhms/(VBase²/VABase)` branch instead of
    the 85 %-of-ac default that would overwrite the copied `RdcOhms`.
    `MakeLike` carries the code name (`:848`), `Create` starts it `''` (`:905`).
  - **Two upstream bugs in that one arm, neither reproduced** (CLAUDE.md — never,
    in any lane); both got an English report in `investigations/to_opendss/`:
    - `36-autotrans-xfmrcode-not-used-message.md` — `DoSimpleMsg('XFmrCode
      Property not used with AutoTrans object.', 100131)` (`:567`) fires
      unconditionally right after `:520` applied the code. Measured on the
      epri-worker: it is **non-fatal inside a `Compile`** (the errno surface is
      clean afterwards, which is what lets the new deck gate r4133 with no
      `expect_warnings`), but on a per-command `Edit` it lands on the errno
      surface **and overwrites the real #100180** — a mistyped code name is
      reported as "property not used". The port stays silent on the hit path and
      answers a miss with r4133's own `Xfmr Code:<name> not found.` / #100180.
    - `37-autotrans-fetchxfmrcode-nconds.md` — `NConds := Fnphases + 1`
      (`:2353`), the *Transformer's* conductor rule copy-pasted into a class
      whose every other path sets `2 * Fnphases` (`:538`, `:798`, `:891`,
      `SetNumWindings` `:1021`) because the auto carries two conductors per phase
      and `SetNodeRef` aliases the series winding's second end inside that block
      (`:986-1002`). Measured on the released 11.0.0.1 DLL, one 3-phase 115/69
      auto: `NumConductors = 4` and `MID.1 = 92 236 V` on a 69 kV winding, versus
      6 and 38 472 V for the same deck with a trailing `phases=3` (which re-runs
      the correct side effect). Both runs converge and report nothing. The port
      keeps `2 * Fnphases` via `set_nconds`, which is what the Pascal statement
      does structurally (reallocate terminals, flag `BusNameRedefined`) with the
      auto's count.
  - **Measure-first, and the deck is single-phase on purpose.** No corpus deck
    set `xfmrcode=` on an AutoTrans; the new one
    (`asymmetric:autotrans/autotrans_xfmrcode.dss`, `engines: "r4133"` — 0.14.5
    would reject the property outright) is **1-phase** because that is the one
    width where the `NConds` bug is inert (`1 + 1 == 2 · 1`), so the r4133
    channel is a valid oracle for the whole copy instead of comparing against a
    structurally wrong element. Its `XfmrCode` declares BOTH windings `delta`,
    so the winding-1 override is strongly observable (at one phase
    `RecalcElementData` derives the series `VBase` from
    `kVseries = kVLL − Winding[2].kVLL`). Two autos share the code, one left on
    winding 2 and one on winding 1, so the same 16-property probe set reads both
    windings' copied values; the probes are limited to props r4133's
    `GetPropertyValue` re-renders live (`:1798-1896`) and whose exact decimal
    fits `%.7g` — `thermal`/`n`/`m`/`flrise`/`hsrise` are `PropertyValue` echoes
    and `normamps`/`emergamps` are 5-significant-digit renders, so neither is
    probed. Live-validated on r4133 before the port existed (epri-worker
    2026-08-22: compile clean, converged, 5 iters, `SOURCEBUS.1`
    114 908.668456851403, `MID1.1` 66 514.442565085992, `SRC2.1`
    117 228.327082133997, `MID2.1` 68 431.041427177013, both autos
    `NumConductors = 2`), and the gate then matched it at the micro floor with
    **no ledger entry**. Non-vacuity proven in a scratch tree: dropping the
    winding-1 override alone reds the `[R4133]` channel on node voltages
    (|Δ| = 8.7 V against an allowed 1.16e-4).
  - **The multi-phase form is pinned in-engine**, since r4133 cannot witness it:
    `exec::tests::autotrans_xfmrcode` (12 tests) covers the forced
    `Series`/`Wye`/`Delta` on a 3-winding code, every copied scalar, the
    `RdcSpecified` selection (with a control auto that never saw a code), the
    conductor count (`Yorder = 18`, where the reproduced bug gives 12), the miss
    arm (#100180, model and stored name untouched, rebuilt Y identical), the
    empty-name no-op (measured: r4133 logs nothing there either), the absence of
    #100131, the display slot, `MakeLike`, the surface split (`Save` writes the
    code name like r4133's flag-blind `SaveWrite`, `Dump` does not — the RP1.1
    disposition), and — the correctness statement no oracle can make — that a
    coded auto and the same auto written out longhand produce a **bit-identical**
    assembled Y and node-voltage vector — plus (audit round) the guard that keeps
    the copied-cell table honest: no asserted cell may equal what a bare auto of
    the same shape holds anyway.
  - **Mechanism added:** one `PropDef` field, `ref_miss_message`
    (`RefMissMessage { code, prefix }`). dss_capi routed every object reference
    through the property system's single #401 miss path, which the port follows
    for every row that has a capi counterpart; this row has none, and r4133 still
    resolves it through the legacy `FetchXfmrCode`, whose `else` is one
    `DoSimpleMsg` that leaves the stored name and the model exactly as they were
    (`:2394-2395`). The field buys both halves: the r4133 text/number, and a miss
    that returns before `set_object_ref` so nothing is written. Its two
    guarantees are the ones the sibling `stub_message` already has (audit round):
    a `ClassProps::new` invariant (only a `PropType::ObjectRef` row with a named
    `object_class` — the one arm that reads it) and a carrier-set pin
    (`ref_miss_message_rows_are_the_measured_set`).
  - **Surfaces.** The row is `HIDE_R4133` (r4133-only ⇒ absent from BOTH pinned
    tables) and joins `PROPS_015X`; the flag's carrier set assert grows to four
    and (audit round) its doc now says what it always meant — "absent from both
    pinned tables", never "not implemented" — with a re-measured blast radius.
    Bytes moved: `json/schema_full_port.json`
    (21 ordinal renumberings, no new block) and `json/schema_divergences.json`
    (a fourth `port_hidden_property` row at `$dssPropertyIndex` 39 /
    `$dssPropertyOrder` **32**, plus the AutoTrans BH trio's indices 42/43/44 →
    43/44/45), with `golden.lock.json`. The order rank was **measured**, not
    argued (the RP1.1 audit's lesson that the shift rule absorbs an off-by-one):
    regenerating moved the prop that stood at 39/32 — `XRConst` — to 40/33.
    `props/autotrans.json` provably does not move (the capture enumerates the
    0.14.5 oracle's own names, `props_roundtrip` 251 passed unchanged), and no
    `Dump`/`Save`/JSON byte moves (`prop_line` skips a hidden row; the dump's
    generic tail starts at prop 28 and walks over it).
  - **Census re-run (the WP-RP1 per-sub-step obligation).** Two full
    `DSS_PROPS_CENSUS=1` walks — pre-RP1.2 (438 cases, 56.5 s) and post-RP1.2
    (439, 56.8 s), both channels — so the delta is a diff, not a derivation:
    r4133 shape classes **3 → 2**, structural pairs **218 → 222**, numeric
    **98 → 103**, cells hidden behind a desynchronized name list **977 → 347**;
    the `capi_v0145` channel is bit-unchanged (3 / 13 / 0 both times). The **9**
    new pairs are recorded in `tests/corpus/props_r4133/README.md`
    §"Pairs the WP-RP1 shape closures make live" with their bins and cell counts.
    Six are covered machinery (bin 1 `enabled`/`xrconst`, bin 5
    `bhcurrent`/`bhflux`, bin 6 `normamps`/`emergamps` at 2.1e-5/4.0e-5 — an
    order of magnitude under the worst display pair). Of the three bin-7 rows,
    `pctperm`/`repair` are frozen-default echoes of exactly the family bin 7
    already enumerates (`InitPropertyValues` `:1958-1959`; the class's
    `GetPropertyValue` re-renders only PD-tail slots 1–2, `:1885-1888`, so 4–5
    fall through to the string store) — in-scope bin 7 grows 17 → 19, RP2.2
    disposition, RP2.3 landing site. The third, **`autotrans.wdgcurrents`**
    (max_rel 7.54e-2), is a **genuine jump and entirely out of scope**: all 35
    cells sit on `engines: "capi_v0145"` cases (the four `controls:autotrans/*`
    RegControl decks + one `modes:makeposseq/makeposseq_xfmr` cell), 0 in scope.
    It is not a port defect — the same 35 cells match exactly on the capi
    channel, and the two engines' `GetAllWindingCurrents` bodies are
    statement-for-statement identical, so the inputs differ, not the algorithm;
    ~4e-4 on the series winding against ~3–7 % on the common winding is the
    signature of a different landed tap, which is why those decks are capi-only.
    Recorded for RP2.2's closed pair list; no RP3 sub-step opens while it stays
    out of scope.
  - **Audit settlement (2026-08-23, `/audit-code` + `/audit-tests`, 8 findings:
    1 major, 7 minor — 7 fixed, 1 recorded-not-changed, 0 refuted).** Written up
    as **7** items below: the missing `ref_miss_message` invariants were raised
    by both auditors (audit-code's third minor and audit-tests' first) and are
    settled once, in item (3).
    (1) *major, code* — the `HIDE_R4133` un-hide measurement (RP1.1: "4 committed
    artifacts, `dump3_commands` +6 rows, no corpus case") was left describing a
    three-carrier flag while RP1.2 added a fourth **on a new class** — and the
    prose claiming otherwise sat in the sub-step's own record. **Fixed by
    re-measurement** (2026-08-23, the flag's own method: drop its arm from
    `hidden_from_full_enum`, `cargo test -p dss-core --no-fail-fast`): **8**
    committed artifacts move — `json/der_usermodel_{assigned,full}.json`,
    `json/dyneq_full.json`, `json/autotrans_micro.json`,
    `json/autotrans_solved.json` (the AutoTrans FULL views gain `"XfmrCode"`
    between `Bank` and `XRConst`), `reports/dump_autotrans.txt` (56 → 57 rows),
    `reports/dump_autotrans3.txt` (63 → 64) and `reports/dump3_commands.txt`
    (2328 → 2335, i.e. **+7**: one help line each for the two Generator props and
    for AutoTrans `XfmrCode`, four for Sensor `Action`'s multi-line help) — plus
    `schema_full_port.json` and the deletion of the **4** `port_hidden_property`
    rows, and **still no corpus case** (`corpus_gate_all_cases_match_engines`
    green with the arm off). AutoTrans is the first carrier class that owns
    committed `Dump` goldens, which is exactly why the number moved. Both copies
    of the measurement (the flag's doc and `ORPHANED_GAPS.md` §2, the row
    GOLDEN_REBASE G3.3c/G3.4 will read as its scope estimate) now carry it, and
    the carrier-set assert's message points at them.
    (2) *minor, code* — `set_object_ref`'s doc justified the silent empty-name
    arm with "r4133's `SetActive('')` misses too", which would in fact log
    #100180 and make the port's silence a divergence. **Fixed**: the real reason
    is the one its own pin records — r4133's `Edit` loop is
    `WHILE Length(Param)>0 Do` (`AutoTrans.pas:472`), so property 39 is never
    reached; the comment now says that and names the pin.
    (3) *minor, code+tests* — `ref_miss_message` landed without either guarantee
    its model `stub_message` has. **Fixed both ways**: a `ClassProps::new`
    `debug_assert` (the field is read only by the `PropType::ObjectRef` +
    named-`object_class` arm of `parse_into`; anywhere else it is a silently dead
    message) and a carrier-set equality pin,
    `compat_quirks::ref_miss_message_rows_are_the_measured_set`, which also
    re-asserts the shape invariant over the live tables. Reported independently
    by both auditors (audit-code minor 3, audit-tests minor 1) and settled once.
    (4) *minor, tests* — several cells of `every_copied_field_arrives` equalled
    the AutoTrans defaults and could not fail: winding 1's `kV` (115), winding
    3's `Tap`/`MaxTap`/`MinTap`/`NumTaps` and — the only cell for its own copy
    statement — `%LoadLoss`, whose code value was `%r` 0.21 + 0.19 = the default
    0.4. Not a coverage hole (the corpus deck's r4133 probes gate `%loadloss`
    live at one phase, and each field had another witness among the other
    windings), but cells that cannot be trusted as pins. **Fixed at the
    fixture** (winding 1 kV 115 → 138 with the circuit
    base, `%r` 0.21 → 0.33 ⇒ `%loadloss` 0.52, winding 3 off the default
    tap/min/max/numtaps), the table extracted into `PER_WINDING`/`WHOLE_ELEMENT`,
    and a new guard `no_asserted_cell_can_be_read_off_the_defaults` makes the
    rule executable. Both halves proven non-vacuous by probe: deleting
    `self.pct_load_loss = code.pct_load_loss()` now reds
    `every_copied_field_arrives` (it did not before), and pointing one cell back
    at a default reds the guard.
    (5) *minor, code* — an **inherited** divergence the sub-step read past:
    AutoTrans `bank=`. r4133 keeps the assignment commented out
    (`AutoTrans.pas:519`) and answers the write with #100130 (`:566`), so its
    `XfmrBank` is always `''` and every auto is its own CIM bank
    (`ExportCIMXML.pas:3280`); dss_capi 0.14.5 restored the property
    (`AutoTrans.pas:482`) and the port follows it, using the name as the CIM
    bank-grouping key and never logging #100130. **Recorded, not changed** — and
    deliberately so: unlike the `XfmrCode` arm, r4133's pair here is
    self-consistent (a documented limitation, not a bug), so nothing is reported
    upstream, and dropping the property would remove working behavior and move
    CIM output — a decision outside RP1.2's scope with no owning plan. Latent:
    every `bank=` in the corpus is a Transformer, the property echo agrees on
    both channels and the CIM goldens are capi-captured; it becomes observable
    only on an r4133-gated deck that writes `bank=` on an auto (r4133 would log
    #100130 there). The evidence now sits at the property row in
    `auto_trans/mod.rs`.
    (6) *minor, docs* — two stale citations. **Fixed**: the corpus deck's header
    cited `AutoTrans.pas:2358-2362` for the connection override (the `Case i of`
    block is `:2356-2361`, and `:2357` — winding 1 → SERIES — sat outside the
    quoted range), and the upstream report said the second `CASE` block is
    "twelve lines later" (it is 46; now phrased without the count) with the
    `bank=` assignment at `:520` (it is `:519`) — plus, found while fixing those,
    "a real, 150-line routine" for `FetchXfmrCode` (`:2339-2396` is 58). Both
    auditors re-checked every other Pascal line number in the sub-step and found
    them correct. The report edit is **not in this commit**: `investigations/`
    is gitignored (`.gitignore:5`), so it lands in the working tree only.
    (7) *minor, docs* — the frontier said "10 tests" where the record said 11.
    **Fixed**: both now say **12** (the guard of item 4 is the twelfth).
  - Gate: all five commands green, re-run after the settlement. `lane_diff.ps1`
    run for the sub-step: **PASS**, 522 cases / 3 220 247 records, `max |Δ| = 0`
    exactly on every gated kind (conv, cur, errs, iter, loss, pow, v, y), zero
    iteration drift — the default lane stays bit-identical to the parity lane,
    so it keeps precisely the parity lane's oracle standing. Not re-run for the
    settlement: it touched no solved state and no compat kernel (the fixture
    change lives inside an in-engine test's own deck).

- **RP1.3** (2026-08-23) — WindGen `UserModel`/`UserData`, a **real behavioral
  port over the WASM user-model host**. r4133 properties 18/19
  (`Version8/Source/PCElements/WindGen.pas:391-395`, Edit arms `:641-642`,
  `MakeLike :829` + `:834-835`, `GetPropertyValue` 18 `:2903`, defaults `''`
  `:2453-2454`) and the whole `Model=6` behavior they feed now exist, closing the
  fourth `shape.txt` gap (windgen 46 → 48 names). dss_capi 0.14.5 does not carry
  the class **at all**, so there is no capi witness for any of it, the rows join
  no allowlist (`PROPS_015X` untouched), and **no `props/` golden exists or was
  created** (plan §1.2).
  - **The shuttle is WindGen's own, and its layout was measured, not read off.**
    `TWindGenVars` differs from `TGeneratorVars` in three ways
    (`WindGenVars.pas:20-73`): no NCIM `deltaQNom` (so the integers stay at
    176/180/184 and the Thevenin tail keeps the Appendix-A offsets),
    `kVGeneratorBase` → `kVWindGenBase`, and a 13-double aerodynamic tail
    `ag`…`s` behind a managed `PLoss: string` reference. A new FPC probe
    (`tools/fpc/usermodel_abi/abi_probe_windgenvars.pas` →
    `docs/wasm/probes/p10_offsets_windgenvars_r4133.txt`) gives native
    `SizeOf = 356`. The wasm image is **348 B with `PLoss` dropped and the hole
    CLOSED** (`ag`@244, `s`@340) — the rule ABI §2.2b already applies to
    `deltaQNom` — which makes the WindGen image a byte-for-byte **extension** of
    the 244-byte Generator image, pinned by
    `records::tests::windgen_vars_head_matches_generator_vars` and recorded as a
    successor trap in ABI §2.6b + `PIN.txt` (never re-insert the hole).
    `crates/dss-usermodel` gained `WindGenVars`,
    `InterfaceKind::WindGenUserModel` and a generic `IntoShuttle`/`ShuttleVars`
    seam: the call surface became generic instead of growing a field, so
    dss-core compiled unchanged while the host half landed, and a kind/record
    mismatch is a loud `Usage` error (a Generator-sized 244 B buffer *traps* the
    348 B guest — pinned).
  - **Guest fixture.** `tests/fixtures/wasm/wgturbine.wasm` (61 425 B, sha256
    `6d5a9a66…f5471cc2`), source crate `tools/wasm_usermodel/models/wgturbine/`,
    `build_wgturbine_wasm.ps1`, PIN row + `fixture_pin.rs` row —
    **reproduced bit-identically from a wiped target dir**. It is a
    constant-admittance source in power flow and a first-order current+speed lag
    in dynamics, then fills `Pg/Ps/Pr/Pm/s/Cp/Lamda` and the record head; five
    `UserData=` keys; **fifteen** state variables (nine as first built, plus the
    six the audit settlement below added: four more record echoes and the two
    call counters `WgUpdCount`/`WgBadSet`). It **discriminates**: its currents
    are `Y·V` or the integrated state, never a native model-1/2/4/5 answer, and
    it is the only thing in the tree that writes the turbine tail (its `ag`@244
    read is the closed-hole witness).
  - **Ordinal shift.** `USERMODEL=18`/`USERDATA=19`, `DUTYSTART`…`VCUTOUT` +2,
    tails `SPECTRUM=45 BASE_FREQ=46 ENABLED=47`, `NUM_PROPS` 46 → **48** —
    `NumPropsThisClass = 44` (`:255`) + 4 inherited, which is exactly the
    census's `oracle_count`. Every `prop::` consumer is inside `windgen/`
    (grep-verified); the JSON-schema ranks are generated, never hand-written.
  - **Two substrate gaps closed on the way to model 6**, both read off r4133
    rather than inferred: the `Xd/Xdp/Xdpp` + `puXd/puXdp/puXdpp` family with
    `VTarget`, `Zthev`, `VThevHarm`/`ThetaHarm` (`Create :960-965,:940`,
    `RecalcElementData :1368-1370,:1406-1408`, `MakeLike :809,:817-819` — the
    two computation sites keep their *different* operand orders, last-ulp
    faithful), and `SetNominalGeneration`'s model-6 arm
    `Yeq := Cinv(cmplx(0, −Xd))` (`:1332-1345`, which deliberately leaves
    `Yeq95`/`Yeq105` untouched). A third, invisible divergence was corrected
    while doing it: `InitStateVars` formed `Edp`/`Yeq` from the WTG3 model's
    `Zthev` because the port had no `Xdp`, where r4133's body is one
    `With WindGenvars` and both come from the **record**
    (`Zthev = Cmplx(Xdp/XRdp, Xdp)`, `:2500-2505`). Now ported for every model
    and proven inert by the whole-corpus dump below.
  - **Two REAL engine defects, found by probe and fixed in the same sub-step.**
    (1) **`like=` produced a *dead* user model, silently.**
    `ClassArena::make_like_within` hands `make_like` an owned `clone()` and the
    slot's `Clone` drops the live wasmi instance, while every call site guards
    on `exists()` — so `New WindGen.w2 like=w1` echoed `UserModel=<path>`,
    reported **22** variables instead of 31, injected nothing and logged
    nothing (measured before the fix). r4133's `:829` is a `Set_Name`
    (`WindGenUserModel.pas:158-220`) = free + `LoadLibrary` + `FNew`, i.e. an
    **eager fresh instance at the guest's own defaults** (the donor's `UserData`
    is never replayed — `:834-835` copies only the property array, which is what
    `?` echoes). `make_like` now queues a real load, drained before `end_edit`
    so `RecalcElementData`'s `FUpdateModel` (`:1418`) sees it.
    (2) **The two-phase dynamics abort carried no error number** —
    `WindGen.pas:2538-2539` is `DoSimpleMsg(…, 5672)` + `SolutionAbort := TRUE`;
    the port pushed the text with `code: None`. Both are behavior changes inside
    RP1.3's scope, not test-only work. A third, pre-existing swallow was drained
    at the same time: `compute_inj_currents`/`get_currents` **built an
    `ErrorLog` and dropped it**, so every WindGen solve-time diagnostic
    (including the old non-3-phase guard) was invisible; they now feed
    `ctx.errors`/`ctx.solution_abort` and the element's deferred log, as the
    Generator twin always did.
  - **Two r4133 bugs found; neither reproduced** (reports in the local-only
    `investigations/to_opendss/`).
    `38-windgen-get-set-variable-usermodel-tail.md` — `Get_Variable`
    (`:2735-2743`) and `Set_Variable` (`:2777-2784`) put the user-model tail
    OUTSIDE the `if i < 19 … else case i of …` chain instead of in its `else`
    (contrast `generator.pas:2868-2884` and WindGen's own correct `VariableName`
    `:2856-2870`), so with a model loaded every native index `1..=22` satisfies
    `k = i−22 ≤ N` and is overwritten by `FGetVariable(k ≤ 0)`;
    `GetAllVariables` reads through it, so `Show Variables`,
    `AllVariableValues`, `Get StateVar` and monitor mode 3 all see it. The
    evidence is **source-structural, not probe-measured**, and honestly so: the
    bug needs a loaded native WindGen user-model DLL, none exists anywhere
    upstream (r4133 ships only the loader), so the epri-worker cannot exhibit
    it. The port routes `1..=22` native and `>22` to the model, pinned by
    `the_native_variables_stay_native_while_a_model_is_bound`.
    `39-windgen-model7-uninitialized-current-limit.md` — found while writing the
    models-3/7 deferral row: `DoCurrentLimitedPQ` reads
    `PhaseCurrentLimit`/`Model7MaxPhaseCurr` (`:70-71`), which are **never
    assigned** in the unit (the `If GenModel=7` initialiser at
    `generator.pas:1183-1187` was dropped when `SetNominalGeneration` was
    cloned), so upstream's model 7 limits every phase current to zero. Nothing
    to fix here — model 7 is not ported — but the trap is recorded at the
    deferral row so a future port does not transcribe it.
  - **Deferral recorded.** Models **3** (`DoPVTypeGen`) and **7**
    (`DoCurrentLimitedPQ`) stay out (plan §1.3): new `ORPHANED_GAPS.md` **§1.11**
    with the spec, the machinery each needs, the enum as the admission gate, and
    the model-7 trap above. The enum comment and the
    `models_3_and_7_are_still_refused` pin forward-reference that row.
  - **Tests — the fixture's law, never a captured number.**
    `exec::tests::windgen_usermodel` (new module, **18** tests): the guest's
    admittance law and untouched neutral; the turbine tail's write-back
    (`Pg` = Σ Re(V·conj(I)), `Cp`, `Lamda` = `ag`·(1+slip)); the 22 ++ 9
    variable surface with six record-head echoes; the `Get_Variable` pin; the
    tail-only `Set StateVar` routing; ordinals 18/19 and the 48-row table; empty
    defaults; re-instantiation on a second assignment; `like=` (defect 1's pin);
    #567 non-abort and #5671 abort; the first-order lag as a **two-run
    identity** (|e₂ₙ|/|eₙ| = fⁿ: 0.606610 measured vs 0.606515 wanted, 1.6e-4);
    WTG3 never stepping under model 6; the 1-phase `:2516-2522` arm; 5672;
    models 3/7 still refused; dormancy on model 1; and #570 warn-and-fallback
    (with the two WM.3-wide port conventions stated, not hidden: a FAILED load
    still echoes its name where r4133 answers `''`, and the port does not repeat
    #567 per iteration). Non-vacuity proven by **six** mutations applied in-tree
    and reverted (5/8/8/1/4/1 tests red), tabulated in the module doc. Plus
    `crates/dss-usermodel/tests/windgen_shuttle.rs` (7) and three `records.rs`
    unit tests for the codec, whose own non-vacuity probe (shifting `ag` back to
    252 — the "hole never closed" mistake) reds two of them.
  - **Dormancy: proven, whole corpus, both lanes.** With no user model bound the
    new arms must move nothing, and they do not: `lane_diff.ps1` PASS and the
    two lane dumps are **byte-identical to the pre-edit baselines** — default
    226 437 002 B `a155aaa4…8672bbfe`, parity 226 437 001 B `dd7c5d59…226e65af`
    — with the five `modes:windgen/*` slices reproducing their pre-edit sha256s
    in both lanes. (The five decks stay `engines: "r4133"`; no ledger entry, no
    manifest or population-lock byte, no tolerance moved.)
  - **Census re-run (the WP-RP1 per-sub-step obligation).** Two full
    `DSS_PROPS_CENSUS=1` walks over the same 439-case population — pre-RP1.3
    (1 059 272 rows, 55.8 s) and post-RP1.3 (1 059 277 rows, 55.5 s), both
    channels: r4133 shape classes **2 → 1** (only RP1.4's reverse
    `gendispatcher` row left) — i.e. the **5** windgen element rows the plan's
    Outcome predicted, all of them in scope — structural pairs **222 → 224**, numeric
    **103 → 103**, cells hidden behind a desynchronized name list **347 → 192**
    — the 155 that became comparable are exactly WindGen's 31 per element
    (2 count + 29 pushed out of alignment by the ordinal-18 insertion) over the
    five decks. The `capi_v0145` channel's five extracts are **byte-identical**
    before and after. The **2** new pairs are recorded in
    `tests/corpus/props_r4133/README.md` §"Pairs the WP-RP1 shape closures make
    live": `windgen.enabled` (`Yes`/`true`, bin 1) and `windgen.dynout`
    (`''`/`[]`, bin 5), 5 cells each and all in scope — both the exact shapes
    RP1.1 already measured on the Generator, **no bin-6/7 row and no genuine
    jump**, so nothing opens for RP2.2 and no RP3 sub-step. Two facts recorded
    with them: the two newly ported props produce **no pair at all** (`''` on
    both sides in all five decks — no corpus deck binds a WindGen user model),
    and the pre-existing `windgen.kva/kvar/mva` numeric pairs did not move
    (they sit at ordinals ≤ 17, inside the aligned prefix, so they compared even
    while the class carried a shape row).
  - **Goldens: the one artifact §1.2 predicts, plus a prose correction.**
    `json/schema_full_port.json` carries the port's own bytes — verified
    programmatically (both files parsed and compared key by key) that
    `$defs/WindGen` is the **sole** changed key: two property objects added at
    `$dssPropertyIndex`/`$dssPropertyOrder` **18/19**, the 28 props from
    `DutyStart` on shifted **+2/+2**, `Like`'s index 46 → 48 with its hoisted
    order 1 unchanged, the indexless `DynInit` tail's order 47 → 49, and
    `WindGenModel` gaining `"User model"`/6 — no other property field and no
    other class moved. Regenerated with
    `REGEN_SCHEMA_PORT=1` in **both** lanes to bit-identical output, satisfying
    its `produced_by: parity` row, with the one `golden.lock.json` line in the
    same commit. **`schema_divergences.json` does not gain a row**, and that is
    a measurement of what the schema gate does for this class, not an omission:
    WindGen is port-only versus 0.14.5 as a WHOLE CLASS, so it carries a single
    `port_authored_classes` entry (no `$dssPropertyIndex`/`Order` fields to get
    wrong) instead of the per-property `port_hidden_property` rows RP1.1/RP1.2
    had to add — the new props are not `HIDE_R4133` carriers and that flag's
    four-carrier set is unchanged. Its `cause` prose, which still said the
    class's metadata comes from "the dss_capi 0.15.x line", is corrected to name
    r4133 + RP1.3 (a one-sentence edit, its lock digest in the same commit); no
    other golden byte moved anywhere. `tests/TOLERANCE_NOTES.md` is owed
    **nothing** by this sub-step: no tier, floor or allowlist row is involved —
    WindGen has no capi channel, the r4133 decks already gate at the standing
    floors, and every new pin is an expected-value or fixture-law assertion.
    `TESTING.md` likewise: it carries no per-test-file inventory to extend (the
    wasm fixtures are documented by `tools/wasm_usermodel/PIN.txt`,
    `fixture_pin.rs` and the WASM phase record) and RP1.3 adds no env knob — so
    the disposition is recorded here rather than by inventing a section there.
  - Gate: all five commands green on the final tree — the parity
    `cargo test` needed **one re-run**, and the reason is recorded rather than
    swallowed: the first full parity run hit a single occurrence of the
    documented file-backed-loadshape **oracle** flake on
    `modes:upgrade/mmf_singlecol` (step 0, `SOURCEBUS.1`: the r4133 side
    2.1e-3 V below the port against an 8.2e-6 allowance — the ~2e-3 class that
    case's own `isolate: true` note describes). The port side is provably
    bit-stable: the lane dump taken minutes earlier carries exactly the failing
    "actual" (`7196.5232407823805, −4.811934119020634`) and is byte-identical to
    the pre-RP1.3 baseline, so nothing in this sub-step moved it. The case
    passes standalone and the re-run of the whole command is green; the standing
    follow-up below records the recurrence. `lane_diff.ps1` run for the
    sub-step (owed: a dispatch arm was added): **PASS**, 522 cases /
    3 220 247 records, `max |Δ| = 0` exactly on every gated kind (conv, cur,
    errs, iter, loss, pow, v, y), zero iteration drift — the default lane stays
    bit-identical to the parity lane and keeps precisely its oracle standing.
  - **Audit settlement (2026-08-23) — 11 findings (2 major, 9 minor) from two
    independent auditors (audit-code 7, audit-tests 4; the two sets are
    disjoint, so nothing was deduped). Ten fixed, one recorded; none dropped.**
    Every fix carries its own measured mutation, and the module's non-vacuity
    table grew from 6 rows to **13**.
    1. *(major, code)* **The `like=` dead-user-model defect was still live on
       Generator (both slots), PVSystem, Storage and CapControl** — RP1.3 had
       fixed only WindGen and parked the rest as an unowned item, against the
       "port gaps immediately" rule, leaving a silent wrong answer in four
       shipped classes. **FIXED** in all five slots (see the standing-follow-ups
       entry below for the Pascal citations), each with its own pin, each pin
       measured non-vacuous by reverting the fix.
    2. *(minor, code)* The `InitStateVars` record-`Zthev` correction was
       documented as invisible to every oracle channel — **wrong**, and the
       changed path had no test. **FIXED**: the claim is corrected in place, and
       the new `a_dynamic_eq_windgen_still_reports_the_models_variables` binds a
       `DynamicEq=` with a `theta = Edp` init pairing (domain code 9,
       `WindGen.pas:2588`) and re-derives `Cang(Edp)` from the engine's own
       solved terminal V/I at `Zthev = Xdp/XRdp + jXdp`; forming `Edp` from the
       WTG3 impedance instead reds it.
    3. *(minor, code)* Out-of-range `VariableName` answered `"ERROR"`, a
       dss_capi-only seed (`Generator.pas:2736`) that r4133's WindGen does not
       have — and the commit's new comment presented it as Pascal-faithful.
       **FIXED**: WindGen answers `''` (r4133 `:2830-2832`, `:2856-2879` — no
       `'ERROR'` literal in the unit) and the comment says why. The three
       sibling classes that DO gate on capi goldens (Generator, IndMach012,
       Storage) keep the capi seed — recorded here, deliberately out of RP1.3's
       scope, since r4133 drops it there too and that is an UPGRADE-lane call
       with golden bytes attached.
    4. *(minor, code)* `GetAllVariables` dropped the user-model tail on the
       `DynamicExp` path — an unported r4133 arm (`:2804-2807` sits outside the
       if/else) that the new doc comment did not declare. **FIXED**: the tail is
       appended after **either** arm, from a single `variable_base()` that
       `num_variables`/`variable_name`/`get_all_variables`/`set_user_model_variable`
       all share. Upstream's own inconsistency here (a `NumVariables` with no
       `DynamicExp` branch, so the reported count and the filled block disagree)
       is corrected, not reproduced, and both are documented.
    5. *(minor, code)* Two wrong Pascal citations. **FIXED, and re-measured
       against the vendored unit rather than patched by hand**: `FNew`
       `WindGenUserModel.pas:34` → **:33** (6 sites), the 15-function binding
       block `:180-194` → **:194-208** (4 sites), `Set_Edit` `:150-154` →
       **:152-156** (3 sites). The same sweep caught two more the auditors had
       not: `Get_Exists` `:131-139` → **:130-138** and `Integrate` `:141-145` →
       **:140-144**.
    6. *(minor, code)* `ensure_live`'s revive branch had no reachable caller —
       dead code that read as a working fallback. **FIXED**: every engine-side
       call site now goes through a new `take_live_user_model`, which revives a
       cloned slot from its spec (replaying `UserData=`) and reports #569 if the
       re-creation itself fails; `user_model_num_vars` stopped gating on
       `exists()` (an element snapshot's surface used to collapse to the native
       22); the helper `user_model_exists`, unused afterwards, is deleted. Pinned
       by `an_element_snapshot_revives_its_user_model`, which drives the very
       `ClassArena::clone_ckt` API the control dispatch uses.
    7. *(minor, code)* The #567/#5671 suppression for a designated-but-unloadable
       `UserModel=` (and the matching `? …UserModel` echo of an attempted name)
       lived only in a test doc comment. **RECORDED, behavior kept**: new
       `docs/upgrade/DIVERGENCES.md` **§L6** with both behaviors, the r4133
       lines, the evidence, the decision and the gate consequence — the
       native-DLL name can never load in a `forbid(unsafe_code)` engine, so #570
       is the one actionable diagnostic, and following r4133's dynamics abort
       would kill vendored decks wholesale for a model we cannot run either way.
       The pin now cites §L6.
    8. *(major, tests)* The write half of the deliberately-not-reproduced
       `Set_Variable` mis-nesting had **no pin** — reproducing it upstream-style
       left all 18 tests green, because the committed guest silently ignored the
       stray index. **FIXED at the fixture**: `wgturbine` now counts
       out-of-range `SetVariable` calls in a new `WgBadSet` variable, and the pin
       asserts it stays 0 after a native `vwind` write; the same mutation now
       reds exactly that test.
    9. *(minor, tests)* `WgConnEcho` was a default-equal cell (`Conn` of a wye
       WindGen is 0, the same value an unmarshaled field carries), so dropping
       `conn` from the engine→record shuttle passed everything. **FIXED**: new
       `a_delta_windgen_echoes_its_own_connection` (a `conn=delta` deck: `Conn`
       1, `NumConductors` 3) plus a non-default `conn` in the crate-side codec
       fixture; `conn: 0` now reds.
    10. *(minor, tests)* `RecalcElementData`'s `FUpdateModel` — a listed RP1.3
        deliverable — had no test, because the guest's `update_model` was an
        empty body. **FIXED**: the guest now re-reads the boundary record and
        counts the call (`WgUpdCount`), following the `indmach012a` precedent;
        `recalc_element_data_updates_the_bound_model` pins one call per
        `RecalcElementData` **and** the refreshed `kVArating` echo after an
        `Edit … kva=`; deleting the call site reds it.
    11. *(minor, tests)* The 43-field engine→record mapping was exercised for
        ~7 fields, so a wrong source field elsewhere passed. **FIXED** by
        echoing four more, one per region of the image — `Xdp`@88 (head
        doubles, derived `puXdp·1000·kV²/kVA`), `VTarget`@212 (the unaligned
        stretch), `Poles`@268 and `VCutin`@292 (turbine tail, both set off
        their `Create` defaults by the deck) — pinned by
        `the_record_fields_come_from_the_elements_own_sources`; `xdp: g.xd`
        now reds.
    The fixture was therefore **deliberately regenerated** (findings 8/10/11 —
    to make engine behavior observable, never to make a failing test pass, plan
    §2.9-4): `wgturbine.wasm` 60 281 B → **61 425 B**, sha256 `c441df68…` →
    **`6d5a9a66…f5471cc2`**, reproduced bit-identically from a wiped target dir
    with the same pinned toolchain, `PIN.txt` updated with both the new digest
    and the superseded one. No golden, no ledger row, no manifest byte, no
    tolerance and no `tests/corpus` byte moved for any of the eleven.
    **Dormancy re-proven on the settled tree**: the default-lane whole-corpus
    state dump (`examples/lane_dump`, 522 cases) is byte-identical to part A's
    PRE-RP1.3 baseline — 226 437 002 B, sha256
    `a155aaa403dc933b36ba535a8f7bd5c43828eb7c82035be1f04419cd8672bbfe` — so none
    of the eleven settlements moves a solved state anywhere in the corpus.
    `lane_diff.ps1` is not owed again (no compat kernel, lane alias or solver
    touched; the default/parity split is untouched by every change above).

- **RP1.4** (2026-08-23) — GenDispatcher `weights`: the allowlist row + the
  upstream report. **Zero engine change** — the port is right and matches
  dss_capi (`gen_dispatcher/mod.rs:63-67,90`, dispatch `compute.rs:28-79`); the
  fifth and last `shape.txt` row is the only one that runs **backwards**, a
  property the port has and r4133's own table loses.
  - **The r4133 bug, verified line by line.**
    `TGenDispatcher.DefineProperties` names seven properties
    (`Version8/Source/Controls/GenDispatcher.pas:127-133`, `PropertyName^[7] :=
    'Weights'`) but declares six (`NumPropsThisClass = 6`, `:92`) and hands the
    cursor to the base class at the declared count (`ActiveProperty :=
    NumPropsThisClass`, `:148-149`). `TCktElementClass.DefineProperties` writes
    at `ActiveProperty + 1` (`Common/CktElementClass.pas:98-99`), so slot 7
    becomes `basefreq`, slot 8 `enabled`, and `TDSSClass.DefineProperties`
    appends `like` at 9 (`Common/DSSClass.pas:307-308`). `CountProperties` sized
    the array for 6+2+1 = 9, so nothing overruns and nothing complains. The
    class constructor then builds `CommandList` from the **overwritten** names
    (`:103-105`) while the `Edit` dispatch `CASE ParamPointer OF … 7:` is still
    the weights arm (`:200-206`) with no `ELSE` route left to
    `ClassEdit`'s `BaseFrequency :=` (`CktElementClass.pas:53`).
    `InitPropertyValues` collides the same way (`:495` then `:499` →
    `Common/CktElement.pas:1307`).
  - **Measured on the git-tracked r4133 DLL** (epri-worker, three probes):
    `AllPropertyNames` returns **9** names with no `weights`;
    `? GenDispatcher.gd1.weights` → `Property Unknown`; `weights=[3, 1]` →
    **DSS error #364** `Unknown parameter "weights"`; `basefreq=[3, 1]` is
    accepted and `? …basefreq` echoes `3, 1`. The 7th **positional** parameter
    still reaches the weights arm (positional parsing skips the command list,
    `:186`). And a plain, legitimate `basefreq=60` silently sets
    `FWeights := [60, 0]` (`InterpretDblArray` pre-sets `Result := MaxValues`
    and zero-fills, `Common/Utilities.pas:683,789-791`): measured on a
    two-generator dispatcher, g1 2019.63 kW / g2 1000 kW (never dispatched)
    against the correct 1509.81 / 1509.81, with `? …basefreq` still answering
    `60`. Report:
    `investigations/to_opendss/40-gendispatcher-weights-registration-off-by-one.md`
    (fix = `NumPropsThisClass = 7`; no other ordinal moves).
  - **Liveness decided BY MEASUREMENT — the decks stay `capi_v0145`.** The
    census's `generator.kw/kvar` deltas on the three `controls:gendispatcher/*`
    cases root-cause **entirely** to the registration bug: on r4133 the decks'
    `weights=[3, 1]` is rejected, so `FWeights` keeps the `[1, 1]` the `GenList`
    arm installs (`:215-220`) and both machines split equally (step 1: g1 =
    g2 = 382.892 kW) while the port splits 3:1 (532.407 / 232.659). Measured over
    all 12 steps of `gendispatcher.dss`: the kW gap is **48 %** at the worst of
    steps 1-11 (step 6, 1880.18 against 2778.88 / 981.48) and **54.5x at step
    0**, where r4133's `Max(1.0, …)` floor (`:450`) holds BOTH its machines at
    1 kW while the port dispatches g2 to 55.52 — that step-0 cell is the census's
    `generator.kw` pair (`max_rel` 5.45e+01); kvar reaches 1.59e+00. (The record
    first wrote a bare "up to 48 % at every step", which omitted step 0 — the
    larger, and the census's own maximum; corrected in the audit settlement
    below.) Feeding r4133 the same vector through the misregistered
    name (`basefreq=[3, 1]`) reproduces the **port's** split to the last
    displayed digit (532.407 / 232.659; kvar 172.809 / 261.237), which proves the
    dispatch algorithms agree and only the parse does not. That is a
    whole-solution divergence, not a `property`-scoped one, so **no pin could
    cover a flip to `engines: "both"`** (§1.1(e) `property` scopes are per-case
    and divergence-only) — the decks stay capi-only, no manifest or
    `population.lock.json` byte moves, and the row's comment says *dormant until
    a gendispatcher deck gates r4133*. Recorded here and in the row itself; no
    silent third state.
  - **The row.** `("GenDispatcher", &["weights"])` joins `PROPS_015X`
    (`tests/harness/mod.rs`) with the r4133-bug provenance in its comment, and
    the table's own doc comment gains the one sentence that admits the second
    direction (the mechanism is channel-agnostic: `prop_015x` drops a Rust prop
    only when **this** capture's name list lacks it, `filter_015x`
    `mod.rs:1680-1692`). So the row is **inert on the capi channel** — 0.14.5
    counts the enum (`.inputs/dss_capi/src/Controls/GenDispatcher.pas:106`) and
    reports `weights`, so it is kept and fully value-compared, exactly as before
    this sub-step — and relieves the shape walk on r4133 only.
    `tests/TOLERANCE_NOTES.md` §"0.15.x property-table allowlist" gains the
    matching r4133-extension bullet. Non-vacuity is pinned against the
    **shipped** table, not a synthetic one, by
    `props_015x_tests::shipped_gendispatcher_weights_row_is_inert_when_the_oracle_knows_it`:
    the row must be in the shipped table, the r4133-shaped name list lines up,
    and a wrong `Weights` value against a capi-shaped list still panics — with
    the panic MESSAGE asserted, so an unconditional `filter_015x` (which would
    panic on the count instead) cannot satisfy it. That message assert is the
    audit settlement below; as first written the test caught only a value-mask
    regression, and the comment claimed a guarantee it could not discriminate.
  - **Census re-run (the WP-RP1 per-sub-step obligation).** Two full
    `DSS_PROPS_CENSUS=1` walks over the same 439-case population — pre-RP1.4
    (1 059 277 rows, 56.3 s) and post-RP1.4 (1 059 277 rows, 56.5 s), both
    channels: r4133 shape classes **1 → 0**, so with this the full census closes
    **429 → 0** and **WP-RP1's whole-WP acceptance criterion is met**.
    Structural pairs **224 → 225**, numeric **103 → 103**, cells hidden behind a
    desynchronized name list **192 → 0** — those 192 are exactly the 48
    GenDispatcher (element, step) rows — one dispatcher in each of the three
    decks, over 12 + 12 + 24 steps — × **4 each**, where the 4 is what
    `collect_element_divergences` actually counts: 1 for the count delta
    (`|filtered| - |oracle|`, 10 vs 9) plus 3 name disagreements along the zip
    (`weights` vs `basefreq`, `basefreq` vs `enabled`, `enabled` vs `like`); the
    Rust list's tenth entry `like` is never reached, the zip stopping at the
    oracle's nine. (The record first said "the four tail positions the misplaced
    `weights` desynchronized (`weights`/`basefreq`/`enabled`/`like`)" — the
    total was right, the mechanism was not; corrected in the audit settlement
    below.) The row count is unchanged overall because the 48
    `shape_count` rows were replaced one-for-one by 48 new value rows. The
    `capi_v0145` channel's five extracts are **byte-identical** before and after
    (3 structural / 13 numeric / 0 shape / 34 diverging cases / 312 elements
    skipped whole). Exactly **one** new pair, recorded in
    `tests/corpus/props_r4133/README.md` §"Pairs the WP-RP1 shape closures make
    live": `gendispatcher.enabled` (`Yes`/`true`, bin 1, 48 cells, **0 in
    scope** — the decks are capi-only). Of the other three positions the closure
    aligns, `weights` is the one the row drops, and `basefreq` and `like`
    agree cell for cell (`'60'` / `''`, probe-confirmed on both sides). **No
    bin-5/6/7 row and no genuine jump**, so nothing opens for RP2.2 and no RP3
    sub-step.
  - **Goldens: measured, none moved.** The whole diff is tests + docs + the
    gitignored report; `PROPS_015X` lives in the test harness and no capture,
    schema or `props/` artifact reads it. Measured by the full five-command gate
    on the final tree (every golden comparison green) and by `git status` (no
    `tests/golden/` byte, no `golden.lock.json`, no `population.lock.json`, no
    ledger row). **No `lane_diff` is owed** — zero engine change: no compat
    kernel, lane alias or solver was touched, and no product crate byte moved at
    all.
  - **Audit settlement (2026-08-23).** Two auditors raised **9** findings, all
    severity **minor**, **4** from `audit-code` and **5** from `audit-tests`.
    One issue was raised by BOTH (the new harness test's doc comment naming a
    failure mode the test could not discriminate), so the findings cover **8**
    distinct issues: **7 fixed**, **1 recorded here and deliberately not
    fixed**, **0 refuted**. Still zero engine change — every edit is a test, a
    test-only diagnostic print, or a doc.
    1. *(raised by both auditors)* **The test's stated guarantee was not the one
       it could catch.** Both mutation-proved it: an unconditional `filter_015x`
       trips the count assert *inside* the `catch_unwind`, which a bare
       `is_err()` accepts. Fixed two ways — the test now asserts the panic
       MESSAGE names the `Weights` value compare (a count panic no longer
       satisfies it, so it does discriminate the unconditional-filter
       regression), and the doc comment now enumerates what it pins: the value-
       mask regression (nothing else catches it — proven by `audit-tests`'
       mutation B2), the unconditional filter (also caught loudly and first by
       `allowlisted_present_in_oracle_value_mismatch_panics`, so "silently" was
       wrong), and deletion of the row itself.
    2. **"up to 48 %" was written as an unqualified maximum.** Re-measured here
       over all 12 steps of `gendispatcher.dss` on the r4133 DLL: kW 48 % at the
       worst of steps 1-11 (step 6), **54.5x at step 0** — which is the census's
       own `generator.kw` `max_rel` 5.45e+01 — and kvar 1.59e+00. Corrected in
       all six places (this record, the frontier paragraph, the `PROPS_015X` row
       comment, `tests/TOLERANCE_NOTES.md`, the vendored `README.md`, and the
       upstream report, whose table gains steps 0 and 6). The direction of the
       error was harmless — the true gap is larger — but it contradicted the
       project's own measurement.
    3. **The 192-cell decomposition named the wrong mechanism** (the total was
       right). `collect_element_divergences` counts the count delta plus one
       cell per disagreeing zip index; `like` is never walked. Corrected here
       and in the vendored `README.md`, which is the file RP2.1/RP2.2 read.
    4. **The row and `TOLERANCE_NOTES.md` claimed present-tense offline liveness
       through `crates/dss-core/tests/props_r4133_replay.rs`, which does not
       exist yet** (RP2.1's deliverable). Both now name today's only exerciser —
       the harness test above — and put the replay in the future tense. Plan
       §1.1(d) is untouched: it states a requirement, not a fact about the tree.
    5. **Plan §1.2's enumeration of the RP2.1 supplement's sources was stale**
       — it named RP1.1 only ("eleven pairs ... with autotrans/windgen to
       follow"), which also miscounted RP1.1 (12 pairs; eleven was the number in
       already-covered bins). Now closed out at 12 + 9 + 2 + 1 = **24 pairs**,
       matching structural 210 → 225 and numeric 94 → 103, with RP1.4's
       `gendispatcher.enabled` called out as the one reached through a
       `PROPS_015X` row rather than a port.
    6. **`PLAN_SEQUENCE.md` row 5b still read QUEUED.** Flipped to IN FLIGHT
       with WP-RP0/WP-RP1 COMPLETE, in row 5a's own house style; RP5.2 still
       owns the flip to COMPLETE and says so in the row.
    7. **A census run can come back short and read as complete.** `audit-tests`
       saw one run at HEAD report 1 059 237 rows / 224 structural against the
       recorded 1 059 277 / 225, with `cases_walked` still 439 and nothing red;
       two further runs reproduced the recorded numbers exactly, and a fourth,
       run here after the fixes, reproduced them again (439 cases, 1 059 277
       rows; r4133 225 / 103 / 0 shape / 434 diverging / 0 unaligned; capi
       3 / 13 / 0 / 34 / 312 skipped whole). The mechanism is `census_one`: a
       channel hiccup on one case turns
       that case's whole divergence population into ONE `oracle_error` row, and
       those counts lived only in `props_census.json`. Fixed by printing
       `oracle error(s)` / `rust error(s)` on the per-channel summary line and
       recording the measured baseline in `TESTING.md` — **5** on r4133 (the
       #303 crash decks) and **22** on `capi_v0145` (decks the 0.14.5 oracle
       cannot compile or solve: post-0.14.5 spellings, WindGen,
       `modes:upgrade/*`) — so a short run is visible without opening the JSON. The knob still asserts
       nothing about the data by design (RP0.2) — RP4.1's single-census
       acceptance must read that count.
    8. **RECORDED, deliberately not fixed — the dormancy closes the row's live
       coverage, and a weights-free r4133-gating deck was never measured.**
       `audit-tests` is right that no other corpus deck holds a GenDispatcher,
       so after RP4.1 the class gets no live r4133 property compare at all.
       Measured here: with the `weights=` line deleted, r4133 dispatches exactly
       as it does with it present (g1 = g2 = 382.892 kW at step 1, 757.77 at
       step 2 — both engines levelize the weights to 1.0 in the `GenList` arm,
       `GenDispatcher.pas:215-220` and `gen_dispatcher/compute.rs:28-31`) and
       the port answers the same for both machines (`Export Generators`, Max kW
       758, equal kWh) — so such a deck could gate `both` and would make the row
       live. **Not taken inside RP1.4**: the plan's RP1.4 offers exactly two
       outcomes, flip one of the three existing decks or mark the row dormant
       (§RP1.4, "if they are unpinnable, keep the decks capi-only and mark ...
       dormant"). RP1.2 did add a deck, but for a property it had just ported
       and with no `engines` question attached; here the deck exists only to
       give an r4133 channel to a class §1.3 deliberately leaves capi-only
       ("a broader `engines` re-audit is not [in scope]"), and it moves the
       census population, `population.lock.json` and the §1.1 case counts every
       later sub-step quotes. Recorded so the option is
       on the record for RP4.1/RP5, together with the latent trap it must avoid:
       r4133's slot 7 IS the weights arm, so a future r4133-gating GenDispatcher
       deck must not set `basefreq=` either, or the compare puts the port's base
       frequency against r4133's stored weights string (measured: `? gd1.basefreq`
       → `3, 1`). Both notes are in the row comment as well.

### R4133_PROPS WP-RP2 — condensed records

> Plan: `R4133_PROPS_PLAN.md` §WP-RP2. Same branch, same per-sub-step ritual.

- **RP2.1** (2026-08-23) — channel threading, the normalization engine, the
  replay accounting and the census's disposition mode. **Zero engine change**:
  the whole sub-step is `tests/harness/*`, `crates/dss-core/tests/*`, the
  vendored evidence and docs. No golden byte, no `golden.lock.json`, no
  `population.lock.json`, no `ledger.json` entry (the §1.1(e) staging rule:
  r4133 `property` entries land at RP4.1), and the r4133 props live masks are
  untouched — every proof here is offline or annotation.
  - **The seam.** `harness::PropsPolicy { channel, value_policy }` travels
    through both property walks exactly as the shape allowlist does, and carries
    the one hook `normalize(class, prop, rust, oracle)` sitting immediately
    before `assert_value_matches_tol` (and before `value_verdict` in the census
    walk, which still records the RAW strings). Capi-invariance is
    **structural**: every RP2.1 behavior hangs off `is_r4133()`, so a capi policy
    walks the pre-RP2.1 code path. `PropsPolicy::plain` disarms the value half —
    that is the census's permanent RP0.2 baseline, so a rule can never shrink
    what a re-census reports.
  - **`SKIP_PROPS` dispositions — all 12 rows, all MEASURED** (a full census run
    with the rows unmasked on r4133, probe lists reverted): 5 rows compare on
    r4133 (`Fuse.FuseCurve`, `Fuse.RatedCurrent`, `Capacitor.pctperm`,
    `Reactor.pctperm` — 0 divergent cells each; `RegControl.RevThreshold` — 888
    cells, `'-100'` vs `'100'`, an `EchoDefault` at `RegControl.pas:1448`), 7
    stay skipped on both channels with their unmasking cost recorded
    (`Capacitor.CMatrix` 1 059, `Reactor.RMatrix`/`XMatrix` 670 each,
    `Capacitor`/`Reactor.FaultRate` 1 059/670, `Fault.GMatrix` 386,
    `Transformer.WdgCurrents` 571). `LANE_SKIP_PROPS`' Monitor `BaseFreq` and
    `skip_transformer_cursor` stay channel-blind, each with its reason.
    The two consts `SKIP_PROPS_CAPI_ONLY`/`SKIP_PROPS_BOTH_CHANNELS` **partition**
    the table, enforced by a test. Unmasking `RevThreshold` is a deliberate
    **policy** change to the r4133 census population (+1 numeric pair, 888
    cells), not a disagreement with the frozen extracts — those stay frozen; the
    pair is vendored in the supplement instead (864 `'-100'`/`'100'` + 24
    `'-800'`/`'800'`) and **RP2.3 owes it an `EchoDefault` row and its pin, or
    RP4.1 breaks on 888 cells**.
  - **The table.** `tests/harness/props_norm.rs`, **157 rows** = `BoolFold` 77 +
    `CaseFold` 63 + `ArrayForm` 17 + `EnumSynonym` 0 (RP2.2 fills the last), each
    row citing its census pair by `(pair, bin, cells)`. Count locks per kind, a
    sortedness/uniqueness pin, and per-row live counters that are armed but
    dormant. The plan's "21 array-form pairs" derives to **17**: the four
    dropped (`expcontrol.derlist`, `relay.normal`, `relay.state`, `sensor.kvs`)
    have zero claimable example rows and are each named by the plan itself for
    RP2.2 / §1.3, so the kill criterion did **not** fire. `PROPS_ECHO_R4133`
    ships empty and deliberately **unwired** — RP2.3 lands its rows and the
    consult together, so no cell can be masked before a cited row exists to mask
    it. `props_norm.rs` carries the tree's only `#[rustfmt::skip]` (the 157-row
    data table; rustfmt's call width would explode 50 rows into eight lines each).
  - **The evidence.** `tests/corpus/props_r4133/examples_supplement.txt` —
    **76 rows / 26 pairs / 4 541 cells**, measured with the RP0.2 knob on this
    tree: the 24 pairs the WP-RP1 shape closures made live (8 of them carry 2-13
    distinct spellings the README's one-representative records cannot),
    `regcontrol.fwdthreshold` and `regcontrol.revthreshold`. Locked by row count
    and cross-file disjointness in `props_r4133_evidence_lock.rs`.
  - **The replay** (`crates/dss-core/tests/props_r4133_replay.rs`, no oracle, no
    solve): **3 454 example rows = 3 378 frozen + 76 supplement, 0 unaccounted**.
    Claimed **748** (BoolFold 114 / CaseFold 442 / ArrayForm 192); shape
    allowlist, echo and floor claim 0 **by construction**, and each zero is
    asserted rather than assumed. Everything else is *declared* to a named
    owner — RP2.2 170 rows/26 pairs · RP2.3 295/72 · RP2.4 2 100/70 · RP3 7/4 ·
    out-of-scope 134/18 (asserted to carry zero in-scope cells) — so the
    accounting is total from day one and later sub-steps only move rows between
    mechanisms. Liveness **157/157**; on `examples_full.txt` alone exactly the 9
    WP-RP1 rows are dead, which is what the supplement exists to fix. The replay
    **parses the README's WP-RP1 tables**: reformatting that section reds a test
    on purpose.
  - **The disposition mode** (`DSS_PROPS_CENSUS=claims`, the RP0.2 seam RP2.1
    was to extend). Same walk, same rows, each **value** row annotated with the
    chain's verdict through the *shipped* predicates
    (`props_norm::claim_value`, and a new non-asserting
    `LedgerView::property_scope_keys` that shares the gate's own scope selection
    and `name_re` decision with `property_handled_keys` so the two cannot drift —
    pinned by `property_scope_keys_names_the_cells_the_gate_would_handle`, which
    also holds it to naming nothing for an unnamed/foreign-field/non-divergence
    scope and to moving no hit counter). Full population, 2026-08-23,
    439 cases × 2 channels, 1 060 165 rows, 56.8 s, error counts at the recorded
    baselines (5 r4133 / 22 capi — the run is complete, not short). r4133:
    **1 060 039 value cells (1 014 217 in scope), 0 shape rows**; claimed
    **510 106 / 488 703 in scope** = BoolFold 294 519, CaseFold 93 230,
    ArrayForm 122 357; echo 0, floor 0, ledger 0; **UNCLAIMED 549 933 / 525 514**
    over 189 pairs, every one of them landing on the owner §1.1 predicts (bin 5 →
    RP2.3 425 419 in scope, bin 6 → RP2.4 45 552, bin 7 → RP2.3/RP3 45 093, bin 3
    → RP2.2 3 691, the nine bin-1 echo pairs 3 293, bins 2/4 residue 318, the
    WP-RP1/supplement pairs 2 148). RP4.1's acceptance is that in-scope column
    reaching zero. The same run reads the capi channel too, and both halves of
    the design show up there: **0** cells normalized and **11 `ledger-hit`s**
    over 8 pairs (the `makeposseq-cuf-applied-capi-props` entry) — so the ledger
    link is exercised, not dead code, even while the r4133 side must stay at zero
    until RP4.1. That capi zero was *measured* as first landed and is
    **structural** since the audit round: `claim_value`/`for_value` take the
    channel and reach no r4133 link on capi (finding 1 below).
  - **The plan's ~491 000 headline, reconciled by measurement.** Frozen bins
    1/2/4 hold 491 854 in-scope cells; measured claimed is 488 703. Every term of
    the difference is measured and vendored in the README's new dated section:
    −2 092 for the 9 bins-1/2/4 pairs that take no rule row (the five pure-echo
    bin-1 pairs + the four the plan routes elsewhere), −1 519 for the
    heterogeneous halves *inside* pairs the table does hold, +24 population drift
    since 2026-08-08 (RP0.2's cursor correction, RP1.2's autotrans), +436 for the
    9 WP-RP1 pairs. Independent cross-check of the two accountings: the census
    measures **749** claimed spellings against the replay's 748 — the extra one is
    `autotrans.conn | 'series' | 'Series'`, a cell RP1.2's closure created. Since
    the audit round that gap is a **locked term**, not a note:
    `props_r4133_replay::LIVE_ONLY_SPELLINGS` names it, asserts `748 + 1 = 749`,
    asserts the shipped `CaseFold` row still claims it, and asserts it really is
    absent from both vendored files (finding 5 below).
  - **Non-vacuity (scratch probe, deleted after the run).** Driving the real
    `compare_all_properties` on the r4133 channel: the legitimate r4133 spellings
    pass (`Capacitor.c1.enabled` vs `'true'`, `Line.l1.ratings` vs `'(400)'`,
    `EnergyMeter.em1.peakcurrent` vs `'(400, 400, 400)'`) and all four
    corruptions still fail —
    `Capacitor.c1 property Enabled: structure differs (actual "Yes" vs expected "false")`,
    `EnergyMeter.em1 property PeakCurrent: structure differs (actual "[ 400 400 400]" vs expected "(400, 4X0, 400)")`,
    `Line.l1 property Ratings: number 0 differs: actual 400 vs expected 404 (from "[ 400]" vs "[ 404]")`
    (the 1e-2 case with the skeletons deliberately equal, so the red comes from
    the NUMBER), and the same 404 inside r4133's paren form. The probe also
    caught a **latent trap for RP4.1**: `NORM_VISITS`/`NORM_HITS` are per-process
    statics, so three `props_norm` unit tests that called
    `assert_norm_rows_are_live()` would start failing on test *ordering* the
    moment the gate visits rows in the same binary. They now assert a counter
    **delta** of zero instead; the live assertion stays where it belongs, once,
    at the end of the gate.
  - **Capi-invariance, measured twice — and what did NOT stay identical.** Part
    A's A/B (`DSS_GATE_ONLY='controls:'` 105 cases, and `controls:regcontrol/` 8)
    was re-run on the final tree: gate stdout identical line for line (only the
    `filtered out` unit-test count and a `Compiling` line differ), and the
    bounded **plain** census is byte-identical between part A's tree and the
    final one — all 10 per-channel extracts, `run.json` and the 33 MB
    `props_census.json`. Against the **pre-RP2.1** tree the statement is
    narrower and is the one that matters: the five `capi_v0145` extracts are
    byte-identical, while the five `r4133` extracts legitimately gain one pair —
    `regcontrol.revthreshold` (`'-100'`/`'100'`, `2.00e+00`, 269 cells on that
    bounded run; 888 on the full population) — because unmasking that
    `SKIP_PROPS` row on r4133 is this sub-step's own deliberate policy change,
    not a normalization leak. Re-measured in the audit round; the correction is
    vendored in the README (§"The RP2.1 policy change a re-census now reports")
    and in TESTING.md's comparison procedure. **The commit body's third bullet
    ("plain census byte-identical") carries the same imprecision and is
    superseded by this paragraph.** The claims mode adds columns and files only
    in `claims` mode, and a plain run now deletes a previous claims run's three
    files by name rather than leave them to be misread.
  - **Goldens: measured, none moved.** `git status` after the full five-command
    gate shows only the sub-step's own harness/test/doc paths; no artifact under
    `tests/golden/` and no lock file appears. No lane_diff owed (zero engine
    change).
  - **Findings handed forward** (each in no plan list before this sub-step):
    1. **`generator.dynout` is an r4133 RENDERING bug, not an echo** —
       `SetDynOutput` stores `Get_Out_Idx`'s `FVarNames` index
       (`General/DynamicExp.pas:411-437`) while `GetDynOutputStr` decodes it as a
       flat (var, slot) index over `DynSlot = array[0..1]`, so var 5 `theta`
       prints `dpshaft`. Dynamics are unaffected and the port is right; declared
       to **RP2.2** so an RP2.3 echo row cannot mask it, and a candidate
       `investigations/to_opendss/` report.
    2. **`energymeter.peakcurrent` needs BOTH kinds of row** — two of its
       spellings are `ArrayForm`-claimed, the third (`'[ 400]'` vs
       `'((400, 400, 400))'`, 6 cells) is an `EchoDefault`
       (`Meters/EnergyMeter.pas:2637-2664` re-wraps the never-refreshed
       `:2209` default). RP2.3.
    3. **`Fault.GMatrix` (386 cells, `'(0 )'` vs `'()'`)** — surfaced by the
       disposition sweep while the row stays masked on both channels: upstream's
       `If Assigned(Gmatrix)` guard against the port's materialised zero matrix.
       Not in any table and not in the census today; handed to RP2.2's triage.
    4. **RP2.3/RP2.4 must return here**: `MULTI_LINK_ROWS` and the two zero
       `CLAIMED_` locks in the replay force a visit when the echo table or the
       floor starts claiming, and `the_echo_table_and_the_display_floor_claim_nothing_yet`
       is the test that will red.
    5. **Hygiene, unowned — and it bit once.** Corpus/gate runs leave `Export`
       artifacts inside the **vendored** corpus tree (this sub-step's runs left
       seven across `tests/corpus/electricdss-tst/Test/AutoTrans/`, untracked and
       not gitignored; all deleted by name). No corpus-hygiene test catches a
       deck writing into the vendored tree. In the same directory the first full
       `cargo test --workspace` of this sub-step hit a **transient** gate failure
       — `solvable_now:Test/AutoTrans/Auto3bus.dss`, r4133 channel, `export
       losses file=Auto3bus_HL_losses.txt` → `Error 303 … I/O error 103` — which
       did not reproduce: the AutoTrans slice re-ran 9/9 and the full suite came
       back 3 456 passed / 0 failed in both lanes. It cannot come from this
       diff (nothing here executes outside `DSS_PROPS_CENSUS=claims`), and the
       suspicion is exactly the artifact-in-the-vendored-tree hygiene above.
       Recorded so a second sighting is a pattern, not a surprise.
    6. **RP2.2 must read the `swtcontrol` getter for the WHOLE pair.**
       `swtcontrol.normal`/`state` are on RP2.2's S6 list *and* carry an RP2.1
       `ArrayForm` row that claims one cell of 59 each (the single one-token
       spelling); the per-phase renders are still RP2.2's. Same shape, fully
       claimed, on `invcontrol.monbus`/`monbusesvbase`. Added by the audit
       settlement below (finding 8) — a row on `RP22_S6` is not proof RP2.1 left
       the pair alone.
  - **Audit settlement (2026-08-23, one commit on top of `e96d9248`).** Two
    audits (code, tests) raised **9** findings. **Raw count 9** = 5 (audit-code,
    all minor) + 4 (audit-tests: 1 major + 3 minor), i.e. 1 major + 8 minor.
    **Deduped count 9** — the two auditors overlap on exactly one substance, the
    `swtcontrol` `ArrayForm` rows on RP2.2's S6 list, which audit-code raised
    under "needs manual review" (not as a finding) and audit-tests as its finding
    8; settled **once**, as item 8 below. **Settled 9 = 9 fixed + 0
    recorded-without-fix + 0 refuted** — nothing was dropped and nothing needed
    refuting; every one of the nine was real.
    Every fix is test-only, harness-only or documentation; **zero engine byte,
    zero golden/lock/ledger/manifest byte, no tolerance or floor moved**
    (`R4133_DISPLAY_FLOOR` stays `None`). **No `NormRule` predicate changed at
    all** — the answer to the major finding is pins, not edits, so no example row
    moved owner and every replay count lock holds unmoved. The only behavior
    changes anywhere are three, and each *narrows* or *records*: the channel gate
    on `claim_value`/`for_value` (narrows what the measurement layer claims), the
    census's ingest aggregation (records a conflict instead of aborting) and the
    `#`-comment filter (narrows to one file).
    1. *(minor, code)* **The claims census applied the r4133 value policy to
       capi rows.** `Row::annotate` ignored the row's own channel and
       `claim_value`/`for_value` took none, so the "capi normalizes nothing"
       zero was a property of today's data, not the contract — and RP2.4's floor
       would have been consulted on capi scalars once the slot fills. **Fixed:**
       the channel is a parameter of both, `claim_value` refuses every channel
       but `R4133`, and the **ledger** link stays live on capi because it is a
       real capi mechanism. Pinned by `props_norm::the_capi_channel_claims_nothing`
       and `props_census::the_capi_channel_reaches_no_r4133_link`; re-measured
       live (`DSS_PROPS_CENSUS=claims`, `modes:makeposseq/`): capi = 0 normalized
       / **7 `ledger-hit`s**, so the gate is a channel gate and not a mute.
       `R4133_DISPLAY_FLOOR`'s doc, which claimed it was reachable only from
       `ArrayForm`, now names both callers of `numbers_match` and why both are
       r4133-only.
    2. *(minor, code)* **`ChannelExtracts::ingest` could abort a claims run on
       legitimate data.** Its `assert_eq!` declared the chain a pure function of
       `(class, prop, rust, oracle)`; the ledger link is per **(case, channel)**
       by design, so one spelling named by an entry in case A and by none in
       case B legitimately answers two ways — and the population is one deck away
       from it (`gictransformer.r2 | '0.09522' | '0.12696'` already occurs in two
       cases with an entry each). A mode whose contract is that it **asserts
       nothing** would have died after a 56 s / 1e6-row walk. **Fixed:** the two
       verdicts aggregate to the **weakest** (`Disposition`'s variant order runs
       claimed → `UNCLAIMED`, so `max` can only move a spelling toward the work
       list), the conflict is counted in `claims_summary.json`'s
       `mixed_disposition_spellings` and printed as a banner NOTE — the
       `heterogeneous_shape_classes` precedent. Pinned by
       `a_spelling_dispositioned_two_ways_aggregates_to_the_weakest` (both
       arrival orders).
    3. *(minor, code)* **The vendored README did not record RP2.1's own
       population change.** Unmasking `RegControl.RevThreshold` on r4133 is a
       policy change, so a plain re-census on this tree reports a numeric pair
       the frozen extracts cannot have. **Fixed:** the README gains
       §"The RP2.1 policy change a re-census now reports" (the pair, its 888
       cells, the `RegControl.pas:1448` citation, the RP2.3 obligation), the
       RP0.2 corrections section points at it so a reader does not stop at
       "two", and TESTING.md's "Comparing against the vendored files" procedure
       now names all **three** expected r4133-side differences. Re-measured
       (`DSS_PROPS_CENSUS=1`, `controls:regcontrol/`): r4133 gains
       `regcontrol.revthreshold | '-100' | '100' | 2.00e+00 | 269`, capi reports
       no `regcontrol` divergence at all.
    4. *(minor, code)* **STATUS overstated the plain-census A/B.** "Byte-identical
       — all 10 per-channel extracts" is true final-tree-vs-part-A-tree and false
       against the pre-RP2.1 tree, where the five r4133 extracts move by exactly
       finding 3's pair. **Fixed:** the bullet now states both baselines
       separately and says which five files move and why; it also records that
       the commit body of `e96d9248` carries the same imprecision and is
       superseded (history is not rewritten — the audited range stays intact).
    5. *(minor, code)* **The replay's evidence base is one spelling behind the
       live population, with no guard.** `autotrans.conn | 'series' | 'Series'`
       is claimed live and vendored nowhere: its pair has a frozen `bins.tsv`
       row, which is exactly what bars it from the supplement. **Fixed:** it is
       now a term — `LIVE_ONLY_SPELLINGS` + `CLAIMED_TOTAL_LIVE = 749`, with
       `the_live_only_spellings_are_claimed_and_reconcile_the_two_accountings`
       asserting the arithmetic, that the shipped `CaseFold` row still claims the
       spelling, that its pair really is in `bins.tsv`, and that no vendored row
       carries it. Recorded in the README's claims section too (443 = 442 + 1).
    6. **(MAJOR, tests) The typed rules' accepted sets were not pinned.** The
       per-kind tests were sample-based, so a widening no sample touches was
       invisible: the auditor's probes showed `fold_bool` widened to `{on, off}`
       and a sign-stripping `CaseFold` both passing the entire suite — and a
       sign-blind `CaseFold` would fold `regcontrol.revthreshold`'s `'-100'` vs
       `'100'`, **888 real divergent cells**, into silence. **Fixed by pinning
       the boundaries, never by narrowing a rule** (no example row moved owner):
       `boolfold_accepts_a_closed_set_of_spellings` walks an exhaustive universe
       — the six accepted tokens under case/trim, plus 33 near-misses
       (`on/off/1/0/t/f/enabled/disabled/…`) asserted `None` — and re-checks all
       eleven census spellings; `casefold_never_drops_a_character` pins the
       predicate itself (accepts case + outer blanks; refuses a dropped sign,
       digit, leading zero, inner blank, separator, node reference, class
       prefix); `arrayform_separators_are_a_closed_set` pins that only
       `[](),`+whitespace separate. **Verified by re-running the auditor's own
       three surviving mutations in-tree** (`{on}` added to `fold_bool`,
       `trim_start_matches('-')` on `CaseFold`, the liveness assert neutered):
       each reddened exactly its new pin and nothing else — 3 failed / 78 passed
       — and the file was reverted to a byte-identical SHA-256 afterwards.
    7. *(minor, tests)* **`assert_norm_rows_are_live` had no canary.** Its
       counters are private process-global statics, so nothing could make the
       guard fire — and at RP4.1 it becomes the sole live anti-rot guard for all
       157 rows. **Fixed:** the rule moved into `check_rows_are_live(table,
       visits, hits)` and the public helper is a two-line adapter over it, so
       both directions are pinned offline —
       `the_liveness_guard_is_silent_when_dormant_or_live` (dormant, fully live,
       mixed) and `the_liveness_guard_fires_on_a_stale_row` (`should_panic`,
       matching the message that names the row). *Recorded, not fixed:* the
       precedent it cites, `lane::assert_reround_cells_are_live`, has the same
       weakness; it is GOLDEN_REBASE territory and is left alone here.
    8. *(minor, tests; the one both auditors raised)* **Two `ArrayForm` rows sit
       on pairs RP2.2 must still triage.** `swtcontrol.normal`/`state` fold their
       single one-token spelling (`'closed'` vs `'[closed, ]'`, 1 cell of 59
       each) while their 58/31/27-cell per-phase renders stay unclaimed, and both
       pairs are on RP2.2's S6 list. **Kept — narrowing would be wrong**: the
       fold is token-for-token value-preserving and the plan itself assigns the
       21 bin-4 pairs to RP2.1, while bare-vs-delimited is a *named* bin-4
       mechanism (`invcontrol.monbus` `'[A.1, A.2, A.3]'` vs `'A.1 A.2 A.3'`).
       **Fixed as disclosure + pins:** the `ArrayForm` doc now states the
       scalar↔one-element case with its census spellings, the module doc gains
       §"Four rows this table DOES hold sit on RP2.2's S6 list" (naming
       `invcontrol.monbus`/`monbusesvbase` as fully claimed and the two
       `swtcontrol` pairs as claimed-in-part, with the `relay.normal`/`state`
       contrast), `RP22_S6`'s doc says a row on that list is not proof RP2.1 left
       the pair alone, and `arrayform_folds_delimiters_not_contents` pins both
       directions on both `swtcontrol` pairs plus the `relay` twin. *Handed to
       RP2.2:* it must read the `swtcontrol` getter for the **whole** pair, the
       two folded cells included.
    9. *(minor, tests)* **The evidence lock's `#`-line filter had been widened to
       the frozen extracts.** Admitting the supplement's provenance header made a
       `#` line inserted into any frozen file invisible to the row-count lock —
       the only assertion that would have seen it. **Fixed:** both readers
       (`props_r4133_evidence_lock.rs`, `props_r4133_replay.rs`) skip comments
       for `examples_supplement.txt` **only**, and
       `derived_extracts_keep_their_row_counts` additionally asserts no frozen
       row-shaped file contains one.

    **Found while settling, not by either audit — one wrong Pascal citation, in
    six places.** RP2.1 cited `RegControl.pas:1437` for
    `PropertyValue[23] := '100'` (RevThreshold's frozen default) and `:1441` for
    `remoteptratio`'s `PropertyValue[27] := '60'`. Both are off by 11: the real
    lines are **`:1448`** and **`:1452`** (re-read in
    `.inputs/electricdss-code-r4133-trunk`; `:1437` is `PropertyValue[12] :=
    '120'`). The plan's own `InitPropertyValues` span `:1423-1459` and its
    `remoteptratio` `:1452` were right — RP2.1's derived copies were not.
    Corrected in `harness/mod.rs`, `props_r4133_replay.rs`,
    `tests/TOLERANCE_NOTES.md`, the vendored `README.md` and
    `examples_supplement.txt`'s provenance header (a comment line — no data row,
    no count, and the header markers the evidence lock checks are untouched), and
    in this record above. `RegControl.pas:820-827` (the `GetPropertyValue`
    override of index 28) was re-verified and is correct.

- **RP2.2** (2026-08-23) — enum synonyms and the S6 dossier. **Zero engine
  change**: `tests/harness/props_norm.rs`, `tests/harness/mod.rs`,
  `crates/dss-core/tests/props_r4133_replay.rs`, the vendored README, the plan
  and this file. No golden byte, no `golden.lock.json`, no
  `population.lock.json`, no `ledger.json` entry, no manifest byte, and the
  r4133 props live masks untouched (§1.1(e) staging). `R4133_DISPLAY_FLOOR`
  stays `None`; no tolerance moved; no test was deleted, `#[ignore]`d or
  loosened. **Kill criterion: not fired** — every pair fits exactly one of the
  three outcomes.
  - **The dossier, condensed** — pair → verdict → r4133 site → cells (full /
    in scope). The routing table itself lives in code with the same citations
    (`props_r4133_replay.rs::RP22_ROUTING`), so a stale verdict fails a test.

    | pair | verdict | r4133 site | cells |
    |---|---|---|---|
    | `vsource.scantype` | **EnumSynonym** | `Vsource.pas:1323-1349` (no arm 17/18) + `:1300-1301` + `:615-616` + `:355`/`:378-384` | 2 042 / 1 697 |
    | `vsource.sequence` | **EnumSynonym** | same, `:385-391` | 2 042 / 1 697 |
    | `isource.scantype` | **EnumSynonym** | no getter override at all + `:626` + `:396` + `:239`/`:257-263` | 137 / 136 |
    | `isource.sequence` | **EnumSynonym** | same, `:627`/`:397`/`:264-270` | 137 / 136 |
    | `invcontrol.voltage_curvex_ref` | **EnumSynonym** (re-typed from `CaseFold`) | `InvControl.pas:3244-3249` live getter + `:837` | 257 / 147 (3 off-bin) |
    | `swtcontrol.action` | echo → RP2.3 `EchoParse` | `SwtControl.pas:573-620` (no arm 3) + `:192-193` + `:652` + `:417` | 34 / 16 |
    | `monitor.mode` | echo → RP2.3 `EchoParse` | `Monitor.pas` no override + `:359` + `:1843` | 4 / 4 |
    | `storagecontroller.modedischarge` | echo → RP2.3 `LiveSemanticsDiffer` + pin | `StorageController.pas:1200-1214` vs `:2322-2333` | 1 / 1 |
    | `line.spacing` | echo → RP2.3 `EchoParse` | `Line.pas:1347-1429` (no arm 21) + `:1511` | 1 / 0 |
    | `isource.bus2` | echo → RP2.3 `EchoDefault` | `Isource.pas:631` (+ report 14) | 136 / 135 |
    | `isource.yearly` | echo → RP2.3 `EchoDefault` | `Isource.pas:628` + `:286` (object aliased, string not) | 122 / 122 |
    | `load.yearly` | echo → RP2.3 `LiveSemanticsDiffer`, ONE row for the pair | `Load.pas:2346` (live raw string) + `:657` + `:807` | 32 548 / 30 546 |
    | `reactor.bus2` | echo → RP2.3 `EchoParse` (derived snapshot) | `Reactor.pas:1087-1105` (no arm 2) + `:386` + `:419-420` + `:341-358` | 47 / 43 |
    | `invcontrol.monvoltagecalc` | echo → RP2.3 | `InvControl.pas:505`, no arm 25, not in `:2806-2839` | 254 / 144 |
    | `invcontrol.pvsystemlist` | echo → RP2.3 `EchoDefault` | `:512`, no arm 32 | 257 / 147 |
    | `invcontrol.vsetpoint` | echo → RP2.3 `EchoDefault` | `:513`, no arm 33 | 249 / 147 |
    | `expcontrol.derlist` | echo → RP2.3 `LiveSemanticsDiffer` + pin | `ExpControl.pas:696` + `:702-715` vs `:227-234`/`:247-252` | 11 / 11 |
    | `capcontrol.type` | echo → RP2.3 `EchoParse` | no `GetPropertyValue` in the unit + `:178` + `:304-311` | 30 off-bin / 6 |
    | `fault.bus2` | echo → RP2.3 `EchoParse` | `Fault.pas:695-718` (no arm 2) + `:297` + `:672` | 1 off-bin / 0 |
    | `generator.dynout` | echo → RP2.3 `LiveSemanticsDiffer` + pin (**not** RP3.5) | `PCElement.pas:197-209`/`:228-236`, `DynamicExp.pas:411-437`/`:441-465`, `Arraydef.pas:39` | 273 / 2 misrendered |
    | `line.units` | **RP3.5** | `Line.pas:1404` live + `:1627`/`:1721-1726`/`:1791-1796`/`:2326-2331` | 3 / 0 |
    | `line.linecode` | **RP3.6** | `Line.pas:1357` live + `:694-700` + `:626-627` | 5 / **5** |
    | `swtcontrol.normal` / `.state` | **RP3.7 (a)** | `SwtControl.pas:589-599`/`:600-610` + `:37-38`/`:299-305`/`:433-480`/`:532-549` | 59 / 40 each |
    | `relay.normal` / `.state` | **RP3.7 (b)** | `Relay.pas:1407-1428` (loops `ControlledElement.NPhases`) | 1 / 0 each |
    | `invcontrol.monbus` / `.monbusesvbase` | already claimed, RP2.1 `ArrayForm` | `InvControl.pas:3226-3285` (no arm 26/27) + `:2806-2839` | 15 / 7 each |
    | `Fault.GMatrix` | triage only — stays value-skipped on BOTH channels | `Fault.pas:695-718` `If Assigned(Gmatrix)` + `:76-77` | 386 (masked) |
  - **The `EnumSynonym` rows: two maps, not one — and that is a correction.**
    The dossier's first draft equated `'Neg'` with our `'Negative'` on *both*
    properties. It is wrong on `scantype`: the two DSS registries are different
    lists — `'Scan Type'` is `['None', 'Zero', 'Positive']` and `'Sequence
    Type'` is `['Negative', 'Zero', 'Positive']`, both over `[-1, 0, 1]`
    (`obj/dss_enum/registry/solution.rs:22-38`) — so ordinal −1 renders `None`
    on one and `Negative` on the other. A shared map would have folded nothing
    on `scantype` and gone stale silently. Hence `SCAN_TYPE_SYNONYMS`
    (`Positive↔Pos`, `Zero↔Zero`) and `SEQUENCE_TYPE_SYNONYMS`
    (`Positive↔Pos`, `Negative↔Neg`), each closed at the spellings the census
    actually measured plus the frozen `InitPropertyValues` default, matched
    exact-token + case-insensitively, **fail-closed** on anything else.
  - **A second dossier correction, found by re-reading the source.** The draft
    recorded that `TVsource.MakeLike`/`TIsource.MakeLike` do not copy
    `ScanType`/`SequenceType`, which would have been the one desync path between
    r4133's echo store and its live enum. They **do** copy both —
    `Vsource.pas:527-528` and `Isource.pas:324-325`, right beside the
    `FPropertyValue[]` loop at `:566`/`:339`. So no desync path exists and the
    rows rest on a stronger argument than the draft claimed; the "r4133 omission"
    finding is **withdrawn**, and no upstream report was written for it.
  - **The three RP3.5+ sub-steps** (full specs in `R4133_PROPS_PLAN.md`
    §WP-RP3; tier `opus-high+`, the RP3.1–RP3.4 row). **All three block RP4.1**
    (plan §0: "RP4.1 starts only after every RP1–RP3 sub-step is landed,
    including any RP3.5+ sub-step RP2.2's triage opens").
    - **RP3.5 — line length units lost by the matrix-branch merge.**
      `exec/reduce.rs:396-409` sets `l.length_units = len_units_saved` and then
      runs the `RMATRIX/XMATRIX/CMATRIX` side effects, which call
      `reset_length_units` (`elements/pd/line/accessors.rs:478-486` →
      `code.rs:24-28`); r4133 does the two in the opposite order (`Line.pas:1627`
      saves, `:1721-1726`/`:1791-1796` re-edit *after* the impedance edit) and
      `MakePosSequence` re-appends `Units=` for the same reason (`:1596`). Second
      divergence in the same routine: the port's `reset_length_units` clears
      `user_length_units`, which r4133 deliberately preserves (`:2330`, "but do
      not erase FUserLengthUnits, in case of CIM export") — no census cell, so it
      needs its own decision. Evidence: `line.units` 3 cells, **0 in scope**
      (`modes:reduce` is capi-only). Must decide: fix the order in both lanes
      with a pin, or overturn the reading with a probe. A live probe on
      `reduce_mergeparallel`/`midi_reduce` is owed **inside** RP3.5 — RP2.2 read
      both sources and did not build.
    - **RP3.6 — `switch=yes` must not clear the linecode flag.** r4133's arm
      (`Line.pas:694-700`) writes r1/x1/r0/x0/c1/c0/len as fields, kills geometry
      and spacing and resets the units, but leaves `FLineCodeSpecified` TRUE; the
      port calls `kill_line_code_specified()`
      (`elements/pd/line/accessors.rs:488-511`, following 0.14.5's
      `KillLineCodeSpecified`). Consequence beyond the render: the flag picks a
      different `FUnitsConvert` formula on a later `units=` (`:626-627`), and the
      decks (`Examples/StoCtrl_Current_PeakShave/Line.DSS`) put `units=m` after
      `Switch=True`. Evidence: `line.linecode` **5 cells, all 5 in scope** — the
      only RP3.5+ pair the unmask will actually compare, so **RP4.1 breaks on it
      if RP3.6 does not land first**. r4133 is the authority; "capi does it" is
      not evidence (CLAUDE.md).
    - **RP3.7 — per-phase switch and relay state.** (a) r4133 keeps
      `FPresentState`/`FNormalState : pStateArray` per phase
      (`SwtControl.pas:37-38`, `:299-305`), settable phase-by-phase from a quoted
      list (`:453-480`), each phase driving its own conductor (`:532-549`), and
      renders one token per controlled-element phase (`:589-610`); the port holds
      one scalar applied to the whole terminal
      (`elements/control/swt_control/accessors.rs:126-163`, `:270-276`,
      `mod.rs:2`). Every r4133 render the census saw is homogeneous, so **no
      value differs today** — a deck writing `state=(open, closed, closed)` would
      diverge in Y. (b) The mirror on Relay: r4133 renders over the live
      `ControlledElement.NPhases` (`Relay.pas:1407-1428`) while the port renders
      its own per-phase array, unresynced after `MakePosSequence`. Evidence:
      `swtcontrol.normal`/`state` 59/40 each, `relay.normal`/`state` 1/0 each.
      Interim treatment if the port change is deferred: exclusion + pin, and the
      gap recorded in `ORPHANED_GAPS.md`.
  - **`generator.dynout` — RP2.1's hand-off, closed here as RP2.3's, not as a
    sub-step.** The root cause was re-verified against the source rather than
    taken on trust: `TPCElement.SetDynOutput` stores `Get_Out_Idx`'s **variable**
    index (`PCElement.pas:231`; `DynamicExp.pas:411-437`) while
    `GetDynOutputStr` (`:205`) hands it to `Get_VarName` (`DynamicExp.pas:441-465`),
    which decodes it as a flat *(variable, DynSlot column)* index — `mylen =
    length(myProt)` with `DynSlot = array[0..1] of double`
    (`Shared/Arraydef.pas:39`). On the shipped deck (`varnames=[Speed Mass
    PShaft Pterm Damp theta]`, `DynOut=[Speed theta]` → `[0, 5]`),
    `Get_VarName(5)` → row 2, col 1 → `'d' + 'pshaft'`. The dynamics are
    unaffected (`generator.pas:2823-2849` indexes `DynamicEqVals` with the same
    variable index the port uses), so the port is right and the remaining work is
    **one cited `LiveSemanticsDiffer` row plus its expected-value pin**, not an
    investigation — which is exactly what plan §RP2.2's third outcome is *not*
    for. The `CELL_DISPOSITION` comment that declared it to RP2.2 now carries the
    verdict, and the row must be a **tagged** `LiveSemanticsDiffer`, never a
    silent `EchoDefault`, or the 2 misrendered cells are masked without a pin.
    Upstream report written:
    `investigations/to_opendss/41-dynout-readback-renders-wrong-variable.md`
    (gitignored, local-only; next free number — 01..40 with gaps).
  - **`Fault.GMatrix` — RP2.1's other hand-off, settled with no sub-step.**
    `TFaultObj.GetPropertyValue` index 6 emits `'('`, fills the lower triangle
    only `If Assigned(Gmatrix)` (`Fault.pas:703`) and closes with `')'`; an
    `r=`-specified fault never allocates `Gmatrix` (`:76-77`, "single G per phase
    … if Gmatrix not specified"). So `'()'` and the port's `'(0 )'` denote the
    **same** state, "no G matrix specified" — the unset-array render family of
    `generator.dynout`/`autotrans.bhcurrent`, with no port behavior to change.
    The 386 cells therefore stay value-skipped on both channels under the
    existing `SKIP_PROPS`/`SKIP_PROPS_BOTH_CHANNELS` row, whose comment now
    carries the verdict; no unmask (§1.1(e)), no new table row, and RP4.1 owes
    nothing for it.
  - **Closing bin 3 meant closing its CELLS.** The replay's own declaration rule
    put four example rows in RP2.2's bucket that the plan's pair list does not
    name: three bin-2-labelled pairs carrying bin-3 cells, which the vendored
    `README.md` §"A pair's bin is a label" already enumerates. RP2.2 read all
    three getters — `capcontrol.type` (`'pf'`/`'volt'`, 30 cells; the class has
    **no** `GetPropertyValue` override, so `type` echoes the deck token) and
    `fault.bus2` (`'b2.0'` vs `'b2.0.0.0'`, 1 cell; the same derived-bus2
    snapshot as `reactor.bus2`, `Fault.pas:297`) went to RP2.3 beside their
    untouched `CaseFold` rows (the mixed-pair pattern the chain order exists
    for), while `invcontrol.voltage_curvex_ref` is a **live** getter
    (`InvControl.pas:3244-3249`, `0→'rated'`, `1→'avg'`, `2→'avgrated'`) against
    our registry's `['Rated','Avg','RAvg']`, so an exclusion would have been a
    lie and the whole pair was re-typed to an `EnumSynonym` row. That map is
    *stricter* than the `CaseFold` row it replaced (three named token pairs
    instead of "any case-only difference"), so no cell stopped being compared.
    The scope growth is recorded as data
    (`props_r4133_replay::RP22_BEYOND_THE_CLOSED_LIST`), not folded away.
  - **The table.** 157 → **161 rows** = `BoolFold` 77 + `CaseFold` **62** +
    `ArrayForm` 17 + `EnumSynonym` **5**. The `CaseFold` lock moved −1 for the
    re-typed pair. New pins, all in `props_norm.rs`:
    `enumsynonym_folds_the_shipped_scan_and_sequence_spellings` (every census
    spelling of all five rows accepted; the opposite ordinal, the reverse
    direction, the cross-registry `'Negative'`-on-`scantype` trap and seven near
    misses all refused) and `enumsynonym_maps_are_injective` (structural: one
    r4133 token may not name two of our values, no empty map, no `''` entry, and
    each row really carries the map the doc names). The bin↔rule pin gained one
    **counted** exception (`invcontrol.voltage_curvex_ref`, bin 2), and
    `the_pairs_routed_elsewhere_have_no_row` gained the four bin-3 pairs RP2.2
    routed away, so adding a synonym row for any of them reds a test.
    `harness/mod.rs`'s `FOLDABLE` fixture grew to one spelling per rule kind, so
    both channel tests now cover the whole table.
  - **The replay.** 3 454 example rows, **0 unaccounted**, claimed **755**
    (`BoolFold` 114 / `CaseFold` **440** / `ArrayForm` 192 / `EnumSynonym`
    **9**). Declared: RP2.2 **(0, 0, 0)** — the acceptance — RP2.3 450 rows/86
    pairs (was 295/72), RP2.4 2 100/70, RP3 7/4, **RP3.5+ 8 rows / 6 pairs / 5
    in scope**, out-of-scope 134/18. Two new structural guards: `declare` now
    **errors** (rather than declaring to RP2.2) for any pair on the closed list
    or any bin-3 cell that neither the chain claims nor `RP22_ROUTING` names, and
    `rp22_settled_every_pair_it_was_handed` proves the input list is partitioned
    into "claimed by the table" (6 pairs, checked per example row through the
    shipped predicate) and "routed with a citation" (18), with every routing row
    live, citing a `.pas:` line, and owning one of the two admissible outcomes.
  - **The claims census** (`DSS_PROPS_CENSUS=claims`, full population,
    2026-08-23): 439 cases × 2 channels, **1 060 165 rows, 57.3 s**, error counts
    at the recorded baselines (**5** r4133 / **22** capi — complete, not short).
    r4133 claimed **514 471 / 492 376 in scope** (was 510 106 / 488 703):
    `BoolFold` 294 519 / 280 915, `CaseFold` **92 976 / 89 231**, `ArrayForm`
    122 357 / 118 413, **`EnumSynonym` 4 619 / 3 817** (9 spellings, 5 pairs);
    echo 0, floor 0, ledger 0 — all three still load-bearing zeros. **UNCLAIMED
    545 568 / 521 841** over 184 pairs (was 549 933 / 525 514 / 189), with the
    frozen-bin-3 row down from 8 pairs / 4 404 / 3 691 to **4 / 42 / 21**. The
    +4 cells over the frozen `bins.tsv` totals on the four source pairs are the
    same population drift RP2.1 reconciled (+2 per `vsource` pair). Live
    spellings **756** = the replay's 755 + the one `LIVE_ONLY_SPELLINGS` term,
    so `CLAIMED_TOTAL_LIVE` is a measurement, not a guess. Capi: **0** on all
    six r4133 dispositions, 11 `ledger-hit`s — unchanged and by contract.
    Numbers vendored in `tests/corpus/props_r4133/README.md` §"What RP2.2 moved".
  - **Doc corrections this sub-step measured** (frozen extracts untouched):
    `monitor.mode`'s r4133 spelling is a stored **RPN source string**
    (`mode=(1 16 +)` in `Test/Dynamic_Kundur.dss:55-56` and
    `Examples/Dynamic_Expressions/Dynamic_KundurDynExp.dss:66-67`), not a
    "decomposition render" — corrected in the plan's §1.1 bin-3 row and in the
    vendored README's new section; `triage.md`'s §S6 wording is superseded by
    the same README section and stays as the historical triage.
  - **Two upstream-report candidates recorded, not written** (neither is needed
    by this sub-step's exclusions): `GetModeString` has no `MODESCHEDULE` arm
    (`StorageController.pas:1200-1214` vs `:2322-2333`), and
    `TExpControlObj.GetPropertyValue` answers the bare PVSystem list for
    `DERList` (`ExpControl.pas:696` + `:702-715` against the two deliberately
    distinct lists built at `:227-234`/`:247-252`).
  - **Gate: all five green on the frozen tree** — `fmt --check`, both clippy
    lanes, both `cargo test --workspace` lanes, **7 274 tests passed / 0
    failed**, corpus gate included.
  - **Goldens: measured, none moved.** `git status` after the five-command gate
    shows only this sub-step's harness/test/doc paths — no artifact under
    `tests/golden/`, no lock file, no manifest, no ledger. No lane_diff owed
    (zero engine change).
  - **Operational note, so it is not miscounted as a second sighting of RP2.1's
    finding 5.** An earlier gate attempt reported 9 corpus-gate failures, all
    `Error Attempting to open file` / `#303` on `ckt7`,
    `StoCtrl_Current_PeakShave` and `ckt24`. The cause was **local and known**:
    a first gate run's script survived its cancellation and its
    `cargo test --workspace` overlapped a second one on the same working tree,
    so two runs raced on the same deck output files. Killed both, `git clean -fd
    -- tests/corpus`, re-ran once — green. That is a *different* cause from
    RP2.1's unexplained transient, so RP2.1's "a second sighting is a pattern"
    counter does **not** advance. What it does confirm is the underlying
    hygiene gap RP2.1 recorded: corpus runs write `Export`/monitor artifacts
    into the **vendored** tree (this sub-step's runs left ~55 across nine
    directories, all untracked, all removed by name), and nothing fails when
    they appear.

### Live escape register — the 15 surviving `TODO(compat)` markers

The register itself is executable: `oracle_parity_cfg_gate.rs::ESCAPE_REGISTER`
(+ `EXIT_POPULATION`) checks it **both ways** — an unregistered marker fails, a
registered marker that no longer exists fails. Rows below are the prose index;
the site comment carries each row's measured cost.

| Owner | Site | Row |
| --- | --- | --- |
| `UpgradeRung` | `support/line_constants/mod.rs` | truncated `mu0` / `Twopi` (35/520 corpus cases) |
| `UpgradeRung` | `support/line_constants/cable.rs` | truncated `1/pi` = `0.3183` in the tape-shield resistance (8/520) |
| `UpgradeRung` | `util.rs` | Pascal `CALPHA = (-0.5, -0.866025)` (33/520 + `dump_reactor_symcomp`) |
| `UpgradeRung` | `elements/pd/reactor/solve.rs` | the same truncated `CALPHA`, second site |
| `UpgradeRung` | `support/complexutil/mod.rs` | truncated constants from `DSSUcomplex.pas` (`pi`) |
| `UpgradeRung` | `support/complexutil/mod.rs` | rad→deg `57.29577951` — "replace with `f64::atan2`" |
| `UpgradeRung` | `elements/pd/line/mod.rs` | Carson earth-return depth `658.5` (not `658.8530451057239`) |
| `UpgradeRung` | `elements/pd/line/code.rs` | the same `658.5` |
| `UpgradeRung` | `elements/pd/line/accessors.rs` | the same `658.5` |
| `UpgradeRung` | `elements/control/exp_control/accessors.rs` | `FOpenTau := Tresponse / 2.3026` (documented r4133 model constant) |
| `UpgradeRung` | `cim/ieee1547.rs` | `LPFTau * 2.3026` |
| `WholeCase` | `elements/pc/generator/user_model.rs` | the Model=6 dynamics-entry `E1` seed (`wasm_gen_dyn`) — the last of four; the GICTransformer `%R2`, Capacitor `Cuf` and LoadShape MMF rows were torn down by G2.5 |
| `WasmGuest` | `tools/wasm_usermodel/models/indmach012a/src/symcomp.rs` | truncated `sqrt(3)/2` = `0.866025403` |
| `WasmGuest` | `.../indmach012a/src/model.rs` | truncated `sqrt(3)` = `1.732` |
| `WasmGuest` | `.../indmach012a/src/model.rs` | FPC single-precision folding of `3.0/746.0` |

> **Reading note (2026-08-05).** The two lists below are kept **verbatim** from
> the pre-archiving STATUS. Rows closed by a later round say so in place, with
> two exceptions the archiving supersedes: the "`TODO(compat)` bug-for-bug sweep
> → Stage F — NOT started" and "`HIDE_015X` → Stage F — NOT started" handoffs
> were executed by DE_PASCALIZE Stage F (complete, `depascalize-stagef.md`) and
> are now finished by GOLDEN_REBASE WP-G2/WP-G4; the live marker population is
> the 15-row table above, not §5's 2026-07-17 count of 123. Their in-place
> back-references now resolve outside this file: `§OG-1.x … below` in
> [`orphaned-gaps.md`](docs/phase-records/orphaned-gaps.md), `§1a archive` in
> [`era-summaries.md`](docs/phase-records/era-summaries.md). §7 states the
> general forwarding rule.

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
- **AutoTrans JSON array-alternative metadata + golden → ✅ CLOSED 2026-07-26**
  (orphaned-gaps round OG-1.3a, branch `depas-og`; see §OG-1.3a below). The
  metadata premise was **stale**: `430d033` (og15c-B6) had already ported the
  singular/plural `array_alternative` + `REDUNDANT` + `ON_ARRAY` block into
  `auto_trans/mod.rs`, so only the golden was missing. Now pinned by
  `autotrans_micro` + `autotrans_solved`.
- **Generator/PVSystem/Storage `ShaftModel`/`ShaftData` under JSON Full → ✅
  CLOSED 2026-07-26** (OG-1.3a). Re-triaged empirically: the WM.3/WM.4
  NOT_PORTED removal did make them render, and the Full JSON matches the pinned
  0.14.5 oracle exactly (all six surfaces `""`). Pinned by `der_usermodel_full`
  (empty default) and `der_usermodel_assigned` (assigned data strings).
- **DER user-model FILENAME render (`UserModel`/`ShaftModel`/`DynaDLL`) still
  unpinned — OPEN (OG-1.3a settle).** Probed: the oracle stores and renders an
  unresolvable name but raises `#570 … Not Loaded` doing it, so a byte golden
  needs `gen_json.py` to tolerate a `DSSException` on selected commands.
  Deliberately not built — do it together with the WASM loader's own
  error-path gating, not by weakening the generator.
- **WindGen `Spectrum` FullNames render ungated — OPEN (OG-1.3a settle).** The
  other twelve Spectrum-bearing classes are pinned (`spectrum_refs` +
  `der_usermodel_full`); WindGen has no capi channel because the class does not
  exist in the pinned 0.14.5 oracle. Needs an r4133-side JSON channel, or a
  UPGRADE-line rung that gives WindGen an oracle.
- **A-Diakoptics `AggregateProfiles` command + D9(d) official-r3723 AD-replay →
  DIAKOPTICS Part II WP-AD.5 — partial.** `exec/command.rs:69` `NOT_PORTED`; WP-AD.6
  threaded children not started (needs MULTITHREADING M2).
- **User-model native DLLs (Gen/PVSystem/Storage/CapControl UserModel) →
  WASM_USERMODELS COMPLETE (WM.0–WM.7 all done, 2026-07-25).** All six properties
  × four elements (Generator `UserModel`/`ShaftModel` WM.3; Storage
  `UserModel`/`DynaDLL` + PVSystem `UserModel` WM.4; CapControl `UserModel` WM.5;
  callback tail WM.6; exit sweep WM.7) follow the plan §2.4 uniform rule (a
  `.wasm` loads through the sandboxed `dss-usermodel` ABI; a native-DLL name warns
  #570/#1570 + falls back to built-in), gated bit-exact vs the r4133 oracle
  (`indmach012a` + `wm4model` + `capuserctl` twin-pinned `.wasm` fixtures). **Zero
  live `PropFlags::NOT_PORTED`** on any user-model property. `PLAN_SEQUENCE.md`
  stage 9 COMPLETE.
- **`like=` dropped a bound user model on Generator / PVSystem / Storage /
  CapControl — FIXED and pinned by the RP1.3 audit settlement (2026-08-23)**,
  after being found (and measured) while fixing the identical defect on WindGen.
  `ClassArena::make_like_within` hands `make_like` an owned `clone()` of the
  donor and the user-model slot's `Clone` deliberately drops the live wasmi
  instance; every call site then guarded on `exists()`, so the copy echoed
  `UserModel=<path>` and silently ran the built-in model with no diagnostic
  (measured: `wasm_gen_pflow` + `New Generator.g2 like=g1` → **g1 20 variables,
  g2 six**). Upstream's `MakeLike` assigns `UserModel.Name`, which is a
  `Set_Name` = free + `LoadLibrary` + `FNew`, i.e. an eager fresh instance at the
  guest's own defaults (`generator.pas:825-826`, `PVsystem.pas:909`,
  `Storage.pas:1210-1211`, `CapControl.pas:452`). All five slots on the four
  classes now queue a real load exactly as WindGen does — Generator
  `UserModel`+`ShaftModel`, PVSystem `UserModel`, Storage `UserModel`+`DynaDLL`,
  CapControl `UserModel` — each with its own regression pin
  (`wasm_usermodels.rs::like_carries_a_live_user_model_on_both_generator_slots`,
  `wasm_usermodels_wm4.rs::like_carries_a_live_user_model_on_the_wm4_classes`,
  `wasm_usermodels_wm5.rs::like_carries_a_live_user_control`), all three measured
  non-vacuous by reverting the fix. The related `ClassArena::clone_ckt` shadow
  (control dispatch with monitored == switched) is closed on WindGen too: every
  engine-side call site now goes through `take_live_user_model`, which revives a
  snapshot's instance from its spec instead of falling back in silence
  (`an_element_snapshot_revives_its_user_model`). The WM.3/WM.4 slots keep the
  older lazy shape there — their `clone_ckt` path is still unreachable in the
  corpus (a Fuse's switched element is a PD element) and is the one part of this
  item left open.

**Residual floors / parked (documented, not bugs):**
- **The file-backed-loadshape ORACLE flake is not extinct — one recurrence
  2026-08-23** (RP1.3's gate run, parity lane): `corpus_gate` failed on
  `modes:upgrade/mmf_singlecol` step 0 with the **r4133** side 2.1e-3 V below
  the port on `SOURCEBUS.1` (allowance 8.2e-6), the exact ~2e-3 class the
  case's `isolate: true` note documents. The port side is bit-stable (the lane
  dump of the same tree carries the failing "actual" value and is byte-identical
  to the pre-sub-step baseline), the case passes standalone, and the re-run of
  the full command is green. UNIFIED_GATE Phase D's fix — a fresh worker per
  case plus `isolate` on this deck — lowered the rate but has not eliminated it,
  and the Phase-D record's "4/4 consecutive green full runs" is therefore an
  under-sample, not a proof. Next suspect if it recurs: the case-directory
  guard restoring `mm8.csv` while a one-shot worker still has it mapped.
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


## 7. Archived history — index of `docs/phase-records/`

Frozen history, superseded only by the code and tests. Every file is verbatim
STATUS.md text; nothing here is a summary.

**Forwarding rule for `STATUS §X` citations.** Code comments, plans and manifests
across the repo cite this file by section (~70 such references outside
`docs/phase-records/` at archiving time). A citation is history — it names the
section it was written against, and the call sites were deliberately **not**
rewritten. Resolve it in whichever file below holds §X; `rg '§X'
docs/phase-records/` finds it. The moved anchors most often cited: §1a →
`era-summaries.md`; §1b–1f and §2 → `phase-index.md`; §3/§3.4 and §4 →
`design-decisions.md`; §OG-1.x → `orphaned-gaps.md`; §7 (**caution** — the old
"Phase 7 — inherited deferrals" section, unrelated to *this* §7) →
`phase-7.md`; the per-WP records (§WP7.5, §WP8.3, §WP-AD.*, §WM.*, §W3.*,
§R2/R3, …) → the per-plan file named in the table.

| File | What it holds |
| --- | --- |
| `golden-rebase.md` | GOLDEN_REBASE WP-G0 (G0.1/G0.2) and WP-G2 (G2.0, G2.1a–h) full records — the source of §1's condensations |
| `depascalize-stagef.md` | DE_PASCALIZE Stage F: the `oracle-parity` lane split, F.1 → F.5 and the wave-4 settlement (incl. the escape-register steps F.3, F.3aa–F.3ag) |
| `depascalize-w3.md` | DE_PASCALIZE wave 3 (W3.1–W3.5), the wave-2a settler, R3iv, P3 |
| `depascalize-r2b-r3.md` | R2b + R3: the `ElemRef → ElemId` store flip, typed accessors, the `Any`/`as_ckt_element` removal |
| `depascalize-r1-r2.md` | R1 typed arenas (`Idx<T>`/`ElemId`/`Elements`) and the R2 injection seam |
| `depascalize-p1.md` | P1 + the whole P1-tail series (1/n–5/n) and its escape record (all three rows closed by W3.1–W3.3) |
| `depascalize-p-series.md` | P1b, P5a/b/c (miette diagnostics), P8, P9, P10, P11, P12, P13, P14, P15 |
| `depascalize-p2.md`, `-p6.md`, `-p12.md`, `-p13.md`, `-r0.md` | the earlier standalone DE_PASCALIZE records |
| `unified-gate.md` | UNIFIED_GATE Phase 0 and A–F, the pre-E/F cross-phase audit and the post-audit fix round |
| `epri-bridge.md` | the `dss-epri` bridge parity round and capability round (Oddie retirement) |
| `wasm-usermodels.md` | WASM_USERMODELS WM.0–WM.7, the ABI re-freeze to r4133, the D2 sub-bug trace and its fix |
| `orphaned-gaps.md` | OG-1.1 … OG-1.10 (GICMvars, AltDSS JSON tails, `CAPI_Schema` walks, UPFC modes, NCIM cadence, `Export Estimation`) |
| `bug-wps.md` | the standalone BUG work packages: livectx and DynExp |
| `adopt-015x.md` | the 0.15.x adoption sweep fix round and its settle |
| `upgrade-rung1.md` | UPGRADE Rung 1/2 records + the NCIM oracle-of-record re-gate |
| `corpus-rounds.md` | the corpus classification/coverage rounds |
| `part2-adiakoptics.md` | DIAKOPTICS/PSTCALC Part II (AD.1–AD.5) |
| `gaps.md` | the GAPS plan (WPG.1–WPG.21) |
| `final-acceptance.md` | the 1:1 final-acceptance round (§6) and the referee certification |
| `era-summaries.md` | §1a archived completed-plan records + the condensed era summaries |
| `gate-history.md` | the historical gate-state snapshots (pre-`corpus_gate.rs`) |
| `design-decisions.md` | §3 key design decisions & rationale + §4 empirical oracle facts |
| `phase-index.md` | the old §1b–1f/§2 index of the completed phases |
| `phase-3.md` … `phase-8.md`, `phase-7-wp1..7.md` | the per-phase and per-work-package logs |
| `test-triage-*.md` | the test-triage rounds (AD classify, IndMach, infra audit, monitor windings, promotions) |
