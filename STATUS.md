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
entirely on capi-only cases and is therefore out of RP4.1's scope (root-caused
2026-09-03 by §RP3.12: an r4133 `UPSTREAM_BUG`, never reproduced). Its audit
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
before the CASE and left stale by the `Locked` refusal, which a live r4133 probe
in the audit settlement confirmed refuses `action=`/`state=` on both engines),
`monitor.mode` (not a
"decomposition render" but the deck's RPN **source text** `mode=(1 16 +)` echoed
back) and `storagecontroller.modedischarge` (`'UNKNOWN'` is `GetModeString`'s
non-injective catch-all) went to RP2.3. **Three RP3.5+ sub-steps opened and they
blocked RP4.1** (§0): RP3.5 `line.units` (the plan's second candidate,
**confirmed** — the port's matrix-branch merge wrote the saved units and *then*
re-ran the side effects that reset them, where r4133 orders the two the other
way; **fixed and landed 2026-08-28**), RP3.6 `line.linecode` (r4133's `switch=yes` arm leaves `FLineCodeSpecified`
TRUE; 5 cells, **all in scope**, so RP4.1 breaks without it; **both parts fixed
and landed 2026-08-29**) and RP3.7 the
per-phase switch/relay state (r4133 keeps a `pStateArray` per phase; the port
held one scalar; **fixed and landed 2026-09-02**, together with (a2)'s lock rule
and Relay's live render bound). Closing bin 3 meant closing its **cells**, not only its pairs: three
bin-2-labelled pairs carry enum-spelling cells, so `capcontrol.type` and
`fault.bus2` joined RP2.3 and `invcontrol.voltage_curvex_ref` — a *live* r4133
enum getter — was re-typed from `CaseFold` to a fifth `EnumSynonym` row. The
replay now claims **755** example rows (0 unaccounted, RP2.2's bucket empty) and
the full claims census measures **514 471 cells / 492 376 in scope** claimed
(`EnumSynonym` 4 619 / 3 817), unclaimed down to 545 568 / 521 841 and bin 3 from
8 pairs to 4. Two dossier findings corrected part A's own reading — r4133's
`MakeLike` *does* copy `ScanType`/`SequenceType` on both classes, and the two
registries are **not** the same list (`ScanType`'s −1 is `None`, `Sequence`'s is
`Negative`), which is why the rows carry two maps and not one. Its two audits
raised 8 findings (6 after dedup, 2 major), **5 fixed and 1 recorded** in a
follow-up commit — the recorded one is a *live* r4133 divergence the dossier
missed on a pair it routed (the port refuses `normal=` on a locked SwtControl,
r4133 applies it; probe-confirmed, engine fix, folded into **RP3.7 (a2)**), and
the two majors were the third `EnumSynonym` map shipping without a closed-set pin
and that same missed divergence.
**RP2.3 (the echo-exclusion table + pins) is COMPLETE** (2026-08-23, one commit,
**zero engine change** — harness/tests/docs only). `PROPS_ECHO_R4133` went from
empty to **81 rows** (50 `EchoDefault` / 7 `EchoParse` / 14
`EmptyCollectionRender` — one new, honestly-named category for the 14 pairs where
r4133's arm is LIVE and merely renders the other empty-collection convention / 10
`LiveSemanticsDiffer`), each with an r4133 `unit:line` citation and a typed
witness (`Capi(n)` / `Pin(name)` / `CapiAndPin`), wired into `compare_prop_lists`
**after** the normalization seam on the r4133 branch only. **Its kill criterion
FIRED**, on six of the 86 declared pairs, and the user ruled both halves: the five
dss_capi-0.14.5 `SilentReadOnly` surfaces (`indmach012.pf`,
`storagecontroller.kwhtotal`/`kwtotal`/`kwhactual`/`kwactual`, **1 064 cells /
772 in scope**) get **no echo row** — r4133 renders a live computed value there
and the port answers `''` only by a capi convention, so under the 2026-08-02
policy the fix is an engine change and they are re-routed loudly into the new
**§RP3.8**, which then **blocked RP4.1** alongside RP3.6/3.7 (RP3.5 landed
2026-08-28, RP3.6 2026-08-29, RP3.7 2026-09-02 and **RP3.8 itself landed
2026-09-02** — see the §RP3.8 record below — so RP3.9 was what was left, and
**RP3.9 itself landed 2026-09-02** too, see the §RP3.9 record below); and
`generator.d`
is **not an echo** either — `Create` initialises `GenVars.D` and never `Dpu`
(`generator.pas:969` vs `:669`/`:2585`), so r4133's `'0'` is its own live value
and `InitStateVars` then runs generator dynamics **undamped** against the
property's documented 1.0 (`:2710`, `:467`). It landed as `LiveSemanticsDiffer` +
pin + upstream report `investigations/to_opendss/42-generator-dpu-never-
initialized.md`, and the three documents that had the mechanism wrong (plan §1.1
RP1.1 note, vendored README §RP1.1, `BIN7_ECHO_SUPPLEMENT`'s comment) are
corrected. Before the exclusions, **nine typed `PROPS_NORM_R4133` rows** were
added (**6 446 cells / 6 370 in scope**), which is what makes the census
attribute those cells to their rule and each row provably live — *not*, as this
record first said, what keeps them inside the live value compare: the audit
settlement below proved the seam's exclusion is pair-scoped and corrected every
document that claimed otherwise. A tenth candidate, `swtcontrol.action`, took
none (6 foldable cells, 0 in scope). **29 expected-value pins** (20 + the nine
the settlement added) landed in the new
`crates/dss-core/tests/props_r4133_pins.rs` — one per witness name, each
compiling the deck the census flagged, asserting the port's live render literally
plus a discriminating second reading, and swept clean of the `Export`/`Show`
artifacts two of those decks write. The full claims census at HEAD (439 cases ×
2 channels, baselines 5 r4133 / 22 capi) measures `echo-row` **488 017 cells /
468 044 in scope** over 169 spellings and all 81 pairs, `UNCLAIMED` down
545 568 → **51 105** (521 841 → **47 427**), capi still **0** on every r4133
disposition.
**Its two audits raised 7 findings (1 major, 6 minor), settled in a follow-up
commit: 6 fixed, 1 recorded with an owner, 0 refuted.** The major one — "the
nine typed rows keep no cell inside the LIVE compare" — is the recorded one, and
it is real: the seam's exclusion is pair-scoped, so on the 20 mixed pairs it also
drops the cells a rule *refused*; the settlement corrected that claim in six
documents, pinned the shipped behaviour, added the per-cell narrowing mechanism
(`ECHO_CARVE_OUTS`) and made the narrowing itself an explicit **RP4.1
precondition**. The six fixed: `reactor.kvar`'s row narrowed by one cell (r4133's
own `MakePosSequence` round-trip, 5.0e-06 — RP2.4's class, not an echo; hence the
census deltas above), a mis-cited `IndMach012.pas` line, an over-broad
`EmptyCollectionRender` doc, the seam ORDER now asserted at the shipped
comparator (it was pinned only in the two offline copies — the swap left the
suite green), both exemption lists pinned literally, and **nine more pins (20 →
29, 63 rows)** for the measured 57 rows whose masked cells sit on
`engines: "r4133"` cases, where a `Capi(n)` witness says nothing.
**RP2.4 (the r4133 props display floor) is COMPLETE — and with it WP-RP2**
(2026-08-23, one commit, **zero engine change** — harness/tests/docs only).
`props_norm::R4133_DISPLAY_FLOOR` went from `None` to **`Some(2e-4)`**, derived
measure-first over all 3 454 vendored spellings: worst display cell
**6.431124e-05** (`load.pf` `'0.747651914485831'` vs `'0.7477'`, `%-.4g`,
`Load.pas:2345` — a formatter the plan did not name), floor **3.110×** above it,
nearest row above the band 1.374769e-03 (**6.874×** over the floor), nearest
genuine jump 4.404256e-03 (22.02×), smallest in-scope genuine jump 5.524501e-02
(276.2×) — the band `(6.431124e-05, 1.374769e-03)` is **empty and 21.38× wide**,
so the number is placed, not tuned. No kill. **The audit settlement (same day,
same commit line) turned the mechanism from a survey into a clause of the
predicate**: as landed, the section asserted that every claimed cell is a Delphi
`Format('%[-].Ng')` render and that "both engines hold the same double", and both
auditors independently disproved it — **55 of the 2 006 claimed spellings (70
cells, 27 pairs)** are gaps no `%.Ng` rounding of our value can produce
(`load.kva`, `vsource.puz*`, `line.b0`/`b1`, `transformer.normamps`, …: a round
trip upstream of a derived quantity, or a plain state difference). The floor now
tests it per cell (`props_norm::display_is_render`: r4133's number must be ours
rounded to the digits r4133 printed), claims **1 951** vendored spellings, and
the 55 are declared to a new **RP3.9** (`RP39_ROUTING`, 27 cited pairs) — back in
`claims_unclaimed_pairs.txt` where WP-RP3 reads, and an RP4.1 precondition
(**settled 2026-09-02**: all 27 pairs `PRECISION_ROUNDTRIP` — §RP3.9 record
below). The
floor VALUE did not move; not one of the 55 has an in-scope cell, so the live
in-scope claim is unchanged at 46 538. The same round also corrected the
`Vsource.pas` line ranges and the `basekv` attribution in the mechanism table
(all three copies), replaced "the capi property compare is exact / at zero
tolerance" with the tier floors it actually runs at (`micro` 1e-9/1e-6, `feeder`
1e-7/1e-5, `midi` 1e-6/1e-4, `micro_wtg3_dynamics` 2e-5/1e-4, now pinned), and
added the guards the auditors named missing (the seam counters tied to the
comparator, the `ArrayForm` wiring, the skeleton clause, the live-only spellings
anchored to their pairs' frozen `max_rel`, a single-number guard on the ceiling
rule). The plan's provisional "smallest genuine jump 1.00e-3 (`invcontrol.lpftau`)" was
**recalibrated**: it is a census-metric number (absolute when the expected side
is 0); under the floor's symmetric metric that cell is rel 1.0 and no
0-vs-nonzero pair can ever be claimed. The floor is a **cell** predicate on the
r4133 branch only (`PropsPolicy::under_display_floor`, after the echo seam), it
covers scalars, bracketed vectors and `|`-separated matrices through one
`numeric_skeleton` path, and its `%-.4g` class-ceiling residual is stated rather
than absorbed (`tests/TOLERANCE_NOTES.md` §"r4133 props display floor", the
plan's ONE sanctioned edit to that file). The full claims census re-measured the
worst cell **live** at exactly 6.431124e-05 and puts `under-floor` at **49 381
cells / 46 538 in scope** over 1 957 spellings and 69 pairs, `UNCLAIMED` down
51 105 → **1 724** (47 427 → **889**, 104 → 59 pairs) with **every one of the
889 attributed to an RP3.x sub-step** (all of them open when RP2.4 measured it;
RP3.1 and RP3.2 have since settled 24 + 4 = **28** of the 889, and they stayed
UNCLAIMED until their six drafted entries **landed at RP4.1 on 2026-09-03**
(§1.1(e); the HEAD census now reads those 28 cells as `ledger-hit` — 28 of the
30 in-scope hits, the other 2 being RP3.4's), whereas
**RP3.3's 2 left the bucket at once**, its exclusion being an echo row the tree
holds now: the 2026-08-24 re-run measures `UNCLAIMED` **1 722 / 887 in scope /
58 pairs** against `echo-row` **488 019 / 468 046 in scope, 170 spellings, 82
pairs**), capi still
**0** on every r4133
disposition (the post-settlement run; before it, 49 451 / 2 012 / 79 and
UNCLAIMED 1 654 / 425 / 37 — the delta is exactly RP3.9's 70 cells).
`DECLARED_RP24` `(2101, 71, 2021)` → **`(0, 0, 0)`** — the sub-step's own
acceptance — and RP2.3's `reactor.kvar` carve-out hand-off is discharged by the
floor claiming it.
**WP-RP3 is OPEN — nothing in it blocked the unmask, which landed 2026-09-03
(§RP4.1 record below): every sub-step
RP4.1 waited on (RP3.1–RP3.4 below, RP3.5–RP3.9 in the records above) has landed,
as has §RP3.12 (2026-09-03, RP3.9's P0 open item — the 34
`controls:autotrans/*` `wdgcurrents` cells, an r4133 `UPSTREAM_BUG` no lane
reproduces; it never blocked the flip, all four of its decks being capi-only),
and has **§RP3.11** (2026-09-03, the `Save`/`Dump` re-serialization surface —
`KEEP_LIVE_PINNED` on both surfaces, the kill criterion firing; record below),
so what is left is §RP3.10 alone, deliberately sequenced *after*
RP4.1 (it blocks §RP5.2, plan §0). Its four bin-7 root-cause sub-steps are ALL
COMPLETE — RP3.1
(`swtcontrol.delay`), RP3.2 (`windgen.kvar`), RP3.3 (`generator.model`) and
RP3.4 (`gictransformer.r2`)** (all 2026-08-24, one commit each, **zero
product-crate bytes and zero
`ledger.json` bytes** — tests and docs only). r4133 never wires `SwtControl`
property 5: its `Edit` `CASE`
(`Version8/Source/Controls/SwtControl.pas:195-218`, the plan's `:194-217` off by
one) has arms 1, 2, 3, 4, 6, 7, 8, 9 and **no 5**, so `delay=` reaches only the
echo store while `TimeDelay` keeps `Create`'s 120.0 (`:310`) and the **live**
getter renders it (`:588`) — a wired property silently ignored, not a queue
delay: `Sample`'s pushes (`:484-507`), `LockCommand`'s own declaration (`:39`)
and `DoPendingAction` (`:396-408`) are all commented out, so the dead code would
not even compile, and `set_States` acts immediately (`:532-549`). capi 0.14.5
wires it (`src/Controls/SwtControl.pas:185`) and the port follows, so **no engine
change was owed**. Reported upstream (`investigations/to_opendss/43-*`, local),
excluded by **two drafted per-case ledger `property` entries** — never a
`PROPS_ECHO_R4133` row, which would misname a live getter — that land at RP4.1
per §1.1(e) (**both landed 2026-09-03**), and held meanwhile by two new pins. `DECLARED_RP3` stays
**`(7, 4, 7)`**: the rows may not leave the work list while the tree holds no
exclusion for them, and the new per-pair work list `RP3_ROUTING` records each of
the four sub-steps' state instead — and, since the same-day audit settlement,
**that move is a hand edit RP4.1 owes** (no link of the chain reads
`ledger.json`, so the tripwire
`the_staged_r4133_property_entries_have_not_landed_yet` reds when the entries
land and says what to do; *discharged 2026-09-03* — RP4.1 landed the eight
entries and moved the accounting mechanically via `RP3_LEDGERED` rather than by
hand, and the tripwire is now
`the_staged_r4133_property_entries_landed_at_rp41`; §RP4.1 record below). The settlement also typed the settled-verdict
contract to plan §WP-RP3's three outcomes, made the r4133 citation and the
witness naming un-shadowable, and turned the 24 = 12 + 12 census decomposition
into a derivation over the corpus.
**RP3.2 landed the same shape from the opposite direction**: r4133's `WindGen`
`kvar` getter *is* wired and live, and reads the **wrong** live field —
`GetPropertyValue` arm 11 (`Version8/Source/PCElements/WindGen.pas:2896`) renders
`presentkvar`, i.e. the *dispatched* `Qnominalperphase·Fnphases/1000`
(`:2297-2300`), where the property documents "the base kvar" (`:365`) and `Edit`
arm 11 (`:629`) stores it in `kvarBase`. Probed live on the r4133 DLL: a deck
typing `kvar=500` renders `0`, and `Edit kvar=777` still renders `0` while `PF`
moves to `0.968058` — parsed, then ignored by the render; `Save Circuit` writes
`kvar=0` back. Our physics already match (probed terminal powers agree on all
five windgen decks), so again **no engine change was owed**: report 44 (local) +
**four** drafted per-case entries (**all four landed 2026-09-03 at RP4.1**) +
four pins, `DECLARED_RP3` unmoved. Its
same-day settlement hardened both census readers (quoted declarations and
`Edit`/`BatchEdit` lines are in scope now), derived the `QMode=` mechanism from
the decks instead of asserting it, and gave the one reproduced upstream bug the
sub-step uncovered an owner — **§RP3.10**, which needs the user's go-ahead and
blocks §RP5.2, not RP4.1.
**RP3.3 is the first WP-RP3 sub-step to close as an ECHO**, and the first to
disprove its own plan text: the plan's "later takes it back to 3"
(`Version8/Source/Common/Solution.pas:1760`) is **dead code** —
`ReversePQ2PV` has no caller anywhere in the trunk (the one call,
`VersionC/Common/Solution.cpp:827`, is commented out), so r4133 leaves a
Q-clamped generator at live model **4** exactly as the port does. What diverges
is only the render: `TGeneratorObj.GetPropertyValue`
(`PCElements/generator.pas:3007-3038`) has **no arm 6**, so `model` falls to
`DSSObject.pas:112-115` and answers the deck's own typed `'3'` forever. Probed
live on the r4133 DLL both ways: `GeneratorsI(10, 4)` moves the field alone and
the render stays `'3'`; `Edit model=4` moves the store and the render follows.
So no engine change and no ledger entry — the exclusion is the **82nd
`PROPS_ECHO_R4133` row** (`EchoParse`, 2 cells, witness `Pin` because both cells
are on `engines: "r4133"` cases), held by
`generator_model_renders_the_live_pv2pq_conversion` on **both** NCIM decks. It
is also the first sub-step to move `DECLARED_RP3` — **`(7, 4, 7)` → `(6, 3, 6)`**
— because an ECHO outcome ships its exclusion in the sub-step's own commit,
which is exactly the condition the no-silent-progress rule reserves the shrink
for.
Its same-day audit settlement corrected two texts that still charged RP4.1 with
retiring "RP3.3's staged entries" (an ECHO stages none), tied the
`ECHO_ROWS_ON_R4133_ONLY_CASES` entry to the derivation that claims to produce it
(plus the table's first `cases` lock), closed three escape hatches in the NCIM
completeness sweep (abbreviated option names, r4133's two-character
`InterpretSolveAlg` value rule, and the `.dss`-only file universe under
`Redirect`), removed a pre-existing counter-race flake from a gated binary, and
opened **§RP3.11** for the one surface no channel compares — `Save`/`Dump`
re-serialization, where the port writes the live `Model=4` against r4133's stored
`model=3`; it ran after RP4.1 and blocked §RP5.2, not the unmask — and
**landed 2026-09-03** as `KEEP_LIVE_PINNED` on both surfaces (§RP3.11 record
below).
**RP3.4 closes the quartet, and it is the one sub-step that root-causes nothing
new**: `gictransformer.r2` is the r4133-channel twin of a divergence
`GOLDEN_REBASE_PLAN.md` G2.5 already fixed and already pinned against capi.
EPRI r4133 carries the same slip byte for byte —
`RecalcElementData`'s `%R` branch builds winding 2's conductance from the **H**
winding's percentage (`Version8/Source/PDElements/GICTransformer.pas:495`
`G2 := 100.0 / (FZBase2 * FPctR1);`, the twin of pinned dss_capi 0.14.5
`src/PDElements/GICTransformer.pas:441`) — and, unlike the plan's own wording
("the same un-honoured `%R2` **echo**"), the render is **not** an echo: property
8 (`R2`, `:130`) is `Format('%.8g',[1.0/G2])` out of `GetPropertyValue` arm 8
(`:723`), a *live computation off a mis-derived field*, while property 14
(`%R2`, `:136`) does echo `FpctR2` (`:729`) and agrees with the port. So the
shape stayed `LEDGER` and **no `PROPS_ECHO_R4133` row was added**: the exclusion
is **two drafted per-case entries** —
`gic-pct-r2-honoured-gictransformer-r4133-props` and
`gic-pct-r2-honoured-midi-r4133-props`, reusing the existing
`gic-pct-r2-ignored` cause verbatim — landing at RP4.1 per §1.1(e) (**both
landed 2026-09-03**) and held meanwhile by two new pins. No new upstream report: report **07** (local) is
already written against r4133 and reproduces exactly this surface, so the next
free number stays **45** (RP3.5 has since taken it). The census derives to **2 cells, both in scope**, one
per `%R`-specified GICTransformer (`gictransformer_gic.dss` `tg3`,
`gic_midi.dss` `tg5`); the corpus's **other 20**, 19 of them on r4133-gating
cases, are ohms-specified and diverge nowhere. `DECLARED_RP3` stays `(6, 3, 6)`
— a LEDGER outcome stages, so three of the four rows are still declared even
with every sub-step run. One correction to the plan's text: the
`linespacing` surface's deck **is** `engines: "both"`, not capi-only by its
`engines` field — it stays capi-only because the r4133 channel is
`kind: "skip"`ped there (`r4133-linespacing-asym-303`, EPRI #303) and because
r4133's own `LineGeometry.pas:1235-1239` agrees with the port anyway.
Its same-day audit settlement completed the ring pin's discriminator (the
`%R1=0.4` edit's `R1` reading was missing, so a no-op edit passed it), gave the
census's `%R1 == %R2` branch its own counter and cause message, made "in scope"
mean `engines=` **and** no ledger `skip` — in all four RP3 census derivations,
via the new `r4133_skipped_cases`, which is exactly the shorthand this sub-step
had disproved — and corrected three citations; no classification, count or
drafted entry moved.
**RP3.5 (`line.units`) landed 2026-08-28 — the first WP-RP3 sub-step whose
outcome is a `FIX`, and so the first to move product-crate bytes and the first
to land (not stage) a ledger entry.** One routine, `TLineObj.MergeWith`, carried
three port defects — **five** once the audit settlement was done — each settled
by a live probe run on three engines (the
r4133 DLL, the pinned capi oracle, the port): **(A)** the matrix-series branch
wrote `Len`/`LengthUnits` *before* the `Rmatrix/Xmatrix/Cmatrix` side effects
whose `ResetLengthUnits` wipes them, where r4133 re-applies them in a separate
`Length=/Units=` Edit *after*
(`Version8/Source/PDElements/Line.pas:1794-1796`, the construction its
`MakePosSequence` reuses at `:1595-1596`) — the port had copied dss_capi
0.14.5's inverted order (`src/PDElements/Line.pas:1806-1817`); **(B)**
`reset_length_units` also cleared `user_length_units`, which **neither** oracle
does (r4133 `:2330` / capi `:2084`, the identical "but do not erase
FUserLengthUnits, in case of CIM export" comment) — a port-authored divergence
from both, measured through its one consumer, `Export CIM100`
(`Conductor.length` 609.6 on both oracles against the port's 2); **(C)** the
sym branch's `Length=`/`Units=` re-apply sat inside the impedance arm, which
both oracles run unconditionally (r4133 `:1724-1726`, capi `:1764-1768`), and
its partner-is-switch half emitted the parser-rejected text `Switch=1` where
r4133 emits `' switch=yes'` (`:1709`), so that arm was a silent no-op. All
three fixed in both lanes and held by **four** oracle-free pins (seven after
the settlement).
`ConvertLineUnits` returns 1.0 whenever a side is `UNITS_NONE`, so
`FUnitsConvert` is 1.0 either way and **no solved-state quantity and no golden
byte moves** — what does move is the property render, and that is the cost the
plan text did not anticipate: `MODES.compare_all_properties` is live on the
capi channel, so the fix reds 3 cells on `modes:reduce/midi_reduce.dss` and the
exclusion ships **now** rather than staging to RP4.1 (§1.1(e) is an r4133 rule,
that channel's props compare being masked) — entry
`reduce-merge-units-restored-midi-capi-props`, 3 measured hits, new cause
`line-merge-length-units-reset`, `population.lock.json` one line. Two plan-text
corrections: `reduce_mergeparallel` contributes **zero** `line.units` cells (all
3 are on `midi_reduce.dss`, whose merges take the matrix branch), and the sym
branch the plan called already-correct was not. A fourth defect is r4133's
alone — `Line.pas:1715` reads `S := ' R0=' + …` where its neighbours append, so
the parallel symmetrical-components merge loses its `R1=`/`X1=` half — reported
as **45** (local; 0 in-scope cells, never reproduced), so the next free number
is **46**. `DECLARED_RP3` stays `(6, 3, 6)`, the frozen census extracts stay
frozen (their `rust='none'` column is historical from here on), and
`DECLARED_RP35 = (8, 6, 5)` is unmoved.
Its audit settlement (2026-08-29) confirmed the classification on both live
oracles and fixed two **further** `MergeWith` defects the sub-step had left: the
routine re-pointed only the *partner's* controls where r4133 re-points both with
the merged name (`:1682-1684`) — the survivor half is the one every reduce
strategy actually uses, and the stale reference made a `shortlines` deck go
singular on the port while r4133 converged — and the sym branch's
`RecalcElementData` (`:1730`) had been deferred to `CalcYPrim`, which is not
equivalent because the call clears `SymComponentsChanged`, the flag gating
`CalcYPrim`'s `C1 /= ConvertLineUnits(UNITS_KFT, LengthUnits)` fix-up, so the
partner-is-switch arm this sub-step had just made reachable came out at
`c1 = 3.6089` against both oracles' `1.1`. It also derived the "3 cells, 0 in
scope" split from the corpus
(`the_rp35_census_decomposition_is_read_off_the_corpus`), pinned the real
`midi_reduce` deck and the two length-derived quantities the restored units
correct, and rewrote the routing comment that still described the defect as
open. One finding was **refuted** by live re-measurement (r4133 renders
`linecode = 'lc'`, not `''`, on a partner-is-switch merge — §RP3.6's premise
stands and is now measured), and one is handed to **RP3.6** with its owner named:
r4133's CIM LineCode-units back-fill matches by the `CondCode` string that
survives the flag being cleared, which the port's `kill_line_code_specified`
throws away. `lane_diff` re-run: PASS, `max |Δ| = 0`.
**RP3.6 part (a) (`line.linecode`, the switch arm) landed 2026-08-29 — the
second `FIX`, and the one RP4.1 actually waits on.** r4133's `switch=` side
effect (`Version8/Source/PDElements/Line.pas:694-700`) assigns
`r1/x1/r0/x0/c1/c0/len` as fields, kills geometry and spacing and resets the
length units, and carries **no** `FLineCodeSpecified` statement — while the two
neighbouring impedance arms each open with one (`6..11, 26..27` at `:685`,
`12..14` at `:691`), so the omission is written arm by arm rather than forgotten
at the end of a block. The flag is read live by the property getter (`3: If
FLineCodeSpecified Then Result := CondCode else Result := ''`, `:1357`) and by
the `units=` arm, which picks `ConvertLineUnits(FLineCodeUnits, NewLengthUnits)`
when it is TRUE and `FUnitsConvert * ConvertLineUnits(LengthUnits,
NewLengthUnits)` when it is FALSE (`:626-627`). dss_capi 0.14.5 **added** the
kill there and flagged it in its own source (`src/PDElements/Line.pas:677`,
`KillLineCodeSpecified(); //TODO: check if this missing is relevant bug`); the
port had copied 0.14.5. r4133 is the behavioral authority (CLAUDE.md
2026-08-02), so the single call is gone from the `SWITCH` arm of
`elements/pd/line/accessors.rs` in **both lanes**; RP3.5's `user_length_units`
line in `reset_length_units` is untouched. (The call-site accounting in this
paragraph as first written — "the six other … each match an r4133 counterpart" —
was wrong twice over and is corrected by the audit settlement below: seven
remained after the deletion, r4133 has eight, and the missing eighth
(`FetchConductorList`, `:1853`) was ported on 2026-08-29.)

Probed live on all three engines (r4133 DLL 11.0.0.1, the pinned 0.14.5 oracle,
the port) before any edit, per the plan's "Do first". **The premise held and the
kill criterion did not fire.** On the corpus shape (`LineCode.99` in metres, the
line `linecode=99 … Switch=True units=m`) exactly one property cell differs —
`linecode`, `'99'` on r4133 against `''` on capi and the port — while `units`,
`length`, `r1`, `x1`, `r0`, `x0`, `c1`, `c0` and the three matrices agree
numerically on all three engines. On the discriminating shape the corpus never
has (a code in **kft**, the line in **m**) the flag proves it is not cosmetic:
r4133 renders `r1 = 0.00328084 = 1/304.8` where capi and the pre-fix port render
`1`, and a later `Edit … units=kft` flips r4133 back to `1` and the other two to
`304.8` — the branch is re-evaluated from the code's units on every `units=`,
never latched. Controls: a switch with no code, and a `switch=` typed *before*
`linecode=` (which `FetchLineCode` re-arms at `:413` — why the corpus's 160
`LVTestCaseNorthAmerican` declarations carry zero cells), agree on all three
engines. **No solved state moves**: read at f64 from the live `YNodeVarray` and
every element's terminal currents, `r4133 vs port` is `max rel |dV| = 3.6e-10`
and `max rel |dI| = 9.7e-08` — *smaller* than `capi vs port` (`4.8e-10` /
`1.9e-07`) on the same deck, whose flag is identical, so the residue is the
documented faer-vs-KLU near-cancellation floor on an ideal switch and not a flag
effect. Both branches of `ConvertLineUnits` evaluate to exactly 1.0 on every one
of the five cells (`m`→`m` and `none`→`m`), so **no impedance and no golden byte
moves** — measured deck by deck across the seven CIM goldens, all 322
`props/*.json` scenarios, `json/*`, `reports/dump3_*`, `reports/save_*` and
`save_roundtrip.rs`, and `golden.lock.json` is unmoved.

What does move is the **live capi comparison**, on exactly five cells over two
`engines: both`, `kind: feeder` cases whose property compare `force_properties`
turns on today — so the exclusion ships **now** rather than staging to RP4.1
(§1.1(e) is an r4133 rule, that channel's props compare being masked):
`line-switch-keeps-linecode-zone2-capi-props` (`line.261249` `99`,
`line.183046` `98`, `line.255376` `99`) and
`line-switch-keeps-linecode-zone3-capi-props` (`line.175078`, `line.249319`,
both `99`), oracle `""` on all five, one new cause `line-switch-kills-linecode`,
`population.lock.json` two lines. No r4133 entry is drafted or staged: after the
RP4.1 unmask these five cells compare and **match**, which is the point of the
sub-step — `line.linecode` is, by `DECLARED_RP35`'s own comment, the only RP3.5+
pair the unmask will actually compare. No `PROPS_ECHO_R4133` row (the getter is
live, not an echo), no `PROPS_NORM_R4133` row, and no `SKIP_PROPS` /
`SKIP_PROPS_CAPI_ONLY` / `LANE_SKIP_PROPS` row — those are `(class, prop)`-keyed
and would blind `line.linecode` on ~77 659 capi cells to cover five.
`DECLARED_RP35 = (8, 6, 5)` is unmoved (a `FIX` does not claim the frozen census
rows, which record the *old* port value), and the frozen extracts under
`tests/corpus/props_r4133/` stay frozen — their `rust=''` column is historical
from here on.

Held by one oracle-free pin in both lanes,
`exec::tests::line_fetch::switch_yes_keeps_the_linecode_and_its_units_conversion`,
which reads the `FUnitsConvert` branch through `r1` as well as the string under
test, so a port that merely stopped erasing the name would still red it.
**Non-vacuity proven by re-running the pre-fix engine** (the kill restored in a
scratch edit, reverted after): the pin fails at `left: "" / right: "lckft"`, and
the two ledger entries fail with
``ledger `line-switch-keeps-linecode-zone2-capi-props` property
line.261249.linecode: rust value "" != pinned rust "99"`` and the same for
`line.175078` on zone_3. Five of the pin's assertions discriminate that way; the
rest are invariance controls that hold on both engines and would catch an
over-broad fix. The "5 cells, all 5 in scope" split is derived from the corpus
by `the_rp36_census_decomposition_is_read_off_the_corpus`, which sweeps every
corpus file for `Line` declarations carrying both a `linecode=` and a `switch=`,
walks each case's `Redirect`/`Compile` closure to find the owners, applies
`force_properties`' rule and the skip-aware r4133 predicate, and reconciles the
products against `examples_full.txt` and `bins.tsv`.

Two plan-text corrections, both factual: the plan and this file named
`Examples/StoCtrl_Current_PeakShave/Line.DSS` as "the affected decks" — it has
the right shape on nine lines but its case is `kind: large`, which
`force_properties` never property-compares, so it contributes **zero** of the
five; the decks are
`ADiakoptics/EPRI_Ckt7-G/Torn_Circuit/zone_2/Branches.dss:93,:95,:479` and
`zone_3/Branches.dss:161,:165` (the census test asserts the counter-claim, not
just the total). And "the capi channel proven unmoved" was wrong as written: the
capi *oracle* is unmoved, the capi *comparison* is not.

`lane_diff.ps1` was **run rather than argued** — the deduction says it is not
owed (no `compat` kernel, no lane alias, and both `ConvertLineUnits` branches
evaluate to exactly 1.0 on every affected line), but these are near-ideal-switch
decks where one ULP on a 1e-6 switch admittance is 1.45 kW
(`crates/dss-core/src/compat.rs:95-115`), so the claim is a measurement:
**PASS, `max |Δ| = 0.000e0` exactly on all eight kinds** (conv 2150, cur
1 169 500, errs 518, iter 2150, loss 366 320, pow 1 169 500, v 375 744,
y 1 738 048 records over 522 cases), 0 iteration counts drifted.
Recorded and handed on, not chased: with `clear_seq(prop::LINECODE)` no longer
running in that arm, `Dump` and `Save Circuit` now print `LineCode=` on a
switched line — toward r4133, which prints `CondCode` unconditionally
(`Line.pas:1273`) and flag-gates its `Save`; but the port's `Save` still emits
`R1..C0` there where r4133 emits none (its `set_as_next_seq(R1..C0)` block is
0.14.5's `PrpSequence` bookkeeping), so a save→reload loses the name to arm 6
while the numbers round-trip identically. (On the *discriminating* shape — a code
in kft, the line in m — the emitted scalars are `R1=1/304.8 …`, not the `R1=1`
this paragraph first said: `Save` writes the getter's value, which is
`R1/FUnitsConvert`. Corrected by the audit settlement below; measured on the
port's own `Save Circuit` output.) No golden or gate reads either surface
on a switched, linecode-bearing line; the general store-vs-live serialization
question, and whether `Dump` should print the raw `CondCode`, belong to
**§RP3.11**. Part **(b)** of RP3.6 — splitting `FLineCodeSpecified` from
`CondCode` so the port keeps the name across a flag kill, and repointing the CIM
LineCode-units back-fill onto the `CondCode` string match r4133 uses
(`ExportCIMXML.pas:3877`, where r4133 writes
`PerLengthSequenceImpedance.r = 0.301/304.8 = 0.00098753281` and capi and the
port write `0.301`) — is a separate change and has **not** landed here. One
incidental, unrelated finding: both oracles emit `<cim:ACLineSegment.b0ch>`
twice on an `ACLineSegment` where the second should be `g0ch`; the port already
writes `g0ch` correctly.

**RP3.6 part (b) (the `FLineCodeSpecified`/`CondCode` split and the CIM units
back-fill) landed 2026-08-29.** r4133 keeps **two** independent pieces of
linecode state where the port kept one. `FLineCodeSpecified`
(`Version8/Source/PDElements/Line.pas:57`) is raised by `FetchLineCode` (`:413`)
and cleared at eight sites — the impedance arm (`:685`), the matrix arm
(`:691`), `FetchLineSpacing` (`:1832`), `FetchConductorList` (`:1853`),
`FetchWireList` (`:1952`), `FetchCNCableList` (`:2016`), `FetchTSCableList`
(`:2075`), `FetchGeometryCode` (`:2131`) — and by the constructor (`:853`).
`CondCode` (`:103`) is written by that same `FetchLineCode` (`CondCode :=
LowerCase(Code)`, `:387`, inside its `IF LineCodeClass.SetActive(Code)` success
branch) and cleared by **nothing but the constructor** (`:825` — the routine
around that line is `TLineObj.Create` itself, whose port counterpart is
`Line::default`, which already starts the name empty). Two surfaces read the raw
name past a kill — `DumpProperties`
(`Writeln(F,'~ ',PropertyName^[3],'=',CondCode)`, `:1273`) and the CIM
LineCode-units back-fill (`if pLine.CondCode = pLnCd.LocalName`,
`Common/ExportCIMXML.pas:3876`) — while the property render (`3: If
FLineCodeSpecified Then Result := CondCode else Result := ''`, `:1357`), the
`units=` conversion branch (`:626-627`) and the CIM
`Conductor.length`/`LineCodeRefNode` branch (`ExportCIMXML.pas:3734-3738`) read
the flag. dss_capi 0.14.5 has **no `CondCode` field at all**: its
`KillLineCodeSpecified` NILs `LineCodeObj` (`src/PDElements/Line.pas:1994-1999`)
and every render goes through the object, so it cannot tell the two apart — and
that is the data model the port had copied.

The port now models both. `line_code_specified` is the flag; `line_code_name` is
`CondCode` and survives every kill; `line_code_ref` — a port convenience with no
r4133 counterpart (r4133's `LineCodeObj` is a *local* of `FetchLineCode`,
`:376`) — carries the flag's lifetime, so a superseded code cannot be resolved
through it. Five product sites, both lanes: `kill_line_code_specified`
(`elements/pd/line/code.rs`) drops the flag, the handle and the set-order mark
and no longer erases the name; `fetch_line_code` raises the flag where r4133
does (`:413`, right after `FLineCodeUnits`); the property-3 render and the
`units=` branch (`elements/pd/line/accessors.rs`) read the flag; `Dump` prints
the raw name unconditionally (`elements/pd/line/dump.rs`, r4133 `:1273`); and
`cim/export.rs::find_line_units_for_linecode` matches the **name**
(`ExportCIMXML.pas:3872-3884`) instead of the live handle, keeping the `enabled`
test, the `Units = UNITS_NONE` precondition, the first-match break and the
case-insensitive compare. `has_line_code` at the ACLineSegment branch is the
flag, matching `:3734`. All eight `kill_line_code_specified` call sites were
re-read against r4133 under the new semantics: seven match a flag-clearing
counterpart one-for-one and stand; the eighth (the `switch=` arm) was already
deleted by part (a).

**Two measured behaviour changes, both toward r4133, both new here.**
(1) `Dump line.<x>` on a line whose code was superseded now prints
`~ LineCode=<code>` where the port (and 0.14.5) printed nothing — RP3.6's probe
deck C measured r4133 answering `''` to `? Line.q.linecode` *and* `lcnone` to
`Dump Line.q` on the same object, which is the split's decisive observation.
Part (a) had handed the `Dump` question to §RP3.11 on the dossier's routing;
under the 2026-08-02 policy it is a plain r4133-vs-0.14.5 render difference with
a one-line fix and no golden byte behind it, so it is settled here instead, and
§RP3.11 keeps only the `Save`-side `set_as_next_seq(R1..C0)` residue —
*settled 2026-09-03 by that sub-step: the 0.14.5 property-tracking stamps stay
(the AltDSS JSON export is captured with them), the re-emitted `R1..C0` are the
live switch values, and the divergence from r4133 is pinned rather than
removed*.
(2) The CIM units back-fill now adopts the units of a line whose flag was
cleared: on `linecode=lcnone units=kft length=2 r1=0.301` (the LineCode declared
without `units=`), r4133 writes `PerLengthSequenceImpedance.r = 0.301/304.8 =
0.00098753281` where 0.14.5 and the pre-split port wrote `0.301` — RP3.5's
number, reproduced by the RP3.6 probe on decks D and E and now produced by the
port. The class this widens is every line that names a code and then overrides
it (impedance, matrix, geometry, spacing, wires, cables), not only the switched
ones part (a) restored.

**Golden blast radius: no committed byte moves — measured, not reasoned.** The
render is the only property surface, and flag-gating it reproduces exactly the
pre-split answer (the old `line_code_ref` fell wherever the flag now falls), so
`props/*.json` — including `line.json::line_code_then_r1`, which pins
`"LineCode": ""` after an arm-6 override — and `json/**` are unmoved; the `Save`
writer is set-order-gated *and* render-gated, so `feeders_controlsoff/*` and
`save_roundtrip` are unmoved. For `Dump`, all six line-bearing report goldens
were read deck by deck: `reports/dump_line_switch` (a switch with no code),
`dump_line_lc`/`dump_line_sym`/`dump_line_geo`, and `reports/dump3_bare`/
`dump3_debug` (`~ LineCode=lc` on a line with **no** override) — none contains a
line that names a code and then supersedes it, so none reaches the changed
branch. For CIM, the six synthetic decks in `tools/golden/cim_decks/` declare
`units=` on every `LineCode` (`lc_sym` kft, `mtx606`/`mtx607` mi, `lc1` kft), so
the `Units = UNITS_NONE` loop never runs there at all; `IEEE13Nodeckt` and
`IEEE123Master` do have codes without units, but neither deck contains a single
line combining a `linecode=` with an override (their switches carry `r1=1e-3`
and no code, and neither issues an `Edit Line.`), so the match set is unchanged.
The two corpus decks that export CIM (`Examples/CIM/IEEE13_{Assets,CDPSM}.dss`)
have the same shape and put `export cim100` last, after the final `Solve`, so
the writer's `pLnCd.Units` mutation — which r4133 performs identically, on a
wider match set than 0.14.5 — cannot reach a solve. Empirically: `golden_cim`
(13), `golden_reports` (305), `golden_json` (305), `props_roundtrip`,
`save_roundtrip` (9) and `golden_lock` (4) are green in both lanes and
`tests/golden/golden.lock.json` did not move. No `tests/corpus/ledger.json`
entry and no `population.lock.json` line moves either: the corpus gate compares
properties through `? name.prop` (the flag-gated getter — `dss-epri`'s
`capture_all_properties` and the capi channel alike) and compares neither `Dump`
text nor CIM XML, so part (b) is invisible to it. Measured rather than deduced:
the **full 521-case gate** was run in both lanes and
`corpus_gate_all_cases_match_engines` is green with no new pin.

Held by two pins, both oracle-free and in both lanes.
`exec::tests::line_fetch::linecode_name_survives_the_flag_that_gates_its_render`
builds probe deck C's shape plus a `geometry=` sibling and reads both surfaces on
the same objects: `? …linecode` is `''` for the two superseded lines and the name
for the control, while `Dump` prints `~ LineCode=lcnone` for all three; it also
pins the arm-6 side effects that came with the kill (`units = none`, `r1 =
0.301`). `golden_cim::cim_linecode_units_backfill_matches_the_condcode_string`
exports CIM100 over five codes — one superseded by `r1=`, one by `geometry=`,
one reached only through a switched line, one declaring its own `units=kft`, one
referenced by nobody — and pins r4133's `0.00098753281` on the first three
against `0.301` for the unreferenced control. **Non-vacuity measured for each,
by reverting the product change and reading the failure text**: restoring the
name-clear in `kill_line_code_specified` fails the engine pin with `Dump Line.q
must print the raw CondCode 'lcnone'` (an empty dump line) and the CIM pin with
`lckill: … left: Some("0.301") right: Some("0.00098753281")` — in that same run
`lcsw` still reads `0.00098753281`, which isolates part (a)'s switch arm from
part (b)'s name survival; restoring `line.line_code_ref.is_some()` in
`find_line_units_for_linecode` fails the CIM pin the same way on `lckill` and
`lcgeo` only; and un-gating the render fails the engine pin at `left: "lcnone" /
right: ""` **and** reds a committed golden (`props_roundtrip: scenario
line_code_then_r1 property LineCode: structure differs (actual "mtx601" vs
expected "")`) — the tripwire part (a) predicted.

`lane_diff.ps1` is not *owed* — part (b) touches two render surfaces and the CIM
writer only, with no impedance, no `Y`, no `compat` kernel and no lane alias, and
the one state mutation in reach (`pLnCd.Units` during CIM export) happens after
the last solve in every deck that reaches it — but it was **run anyway and
reported as a measurement**, the RP3.5 and part-(a) precedent: 522 cases,
3 220 247 records, `max |Δ| = 0.000e0` and `max rel = 0.000e0` on all eight kinds
(`conv`, `cur`, `errs`, `iter`, `loss`, `pow`, `v`, `y`), 0 iteration counts
drifted, **VERDICT: PASS**. The default lane stays bit-identical to the parity
lane, so it keeps the parity lane's oracle standing.

The full five-command gate is green on the committed tree in both lanes (`fmt`,
both `clippy` runs, both `cargo test --workspace` runs — exit 0 read
individually; nothing `#[ignore]`d, no tolerance touched, no golden
regenerated). One hygiene note for whoever runs the workspace suite next: the
`Test/AutoTrans` decks export to *relative* paths and leave a family of ten
untracked `Auto*_{HL,HT,LT}_{current,losses,power}.txt` files inside the tracked
corpus mirror — a corpus-deck defect, not ours; they were deleted and not
committed (`lane_diff.ps1`'s own artifact sweep cleans the same class).

Recorded, not chased — each with an owner, none silently dropped. **(i) The
object-ref miss path.** r4133's `FetchLineCode` else-arm is a single
`DoSimpleMsg('Line Code:' + Code + ' not found for Line object Line.' + Name,
180)` (`:466`) that leaves `CondCode`, the flag and the impedances untouched; the
port routes `linecode=` through the generic `ObjectRef` parse, which logs
0.14.5's #401 text and stores an **empty** name, so a failed `Edit` after a good
`linecode=` erases `CondCode` where r4133 keeps it. The codebase already has the
mechanism for the r4133 shape (`PropDef::ref_miss_message`, used by AutoTrans
`XfmrCode`), but its `{prefix}{name} not found.` format cannot express r4133's
` for Line object Line.<name>` suffix, so adopting it is a message-parity change
rather than a one-liner — it belongs with the r4133 diagnostics work, not here.
Unreachable in the gated corpus and in every golden deck: a sweep of all
`linecode=` tokens against every `New LineCode.<name>` in `tests/corpus`,
`tools/golden`, `tests/golden` and `crates/dss-core/tests` finds exactly one
miss, `reconductor … linecode=oh_750_aac` in
`Examples/Scripts/ReconductorExample.dss`, which is `not_an_entry_point`.
**(ii) `MakeLike`.** Both oracles copy **none** of the linecode state — r4133
`TLine.MakeLike` (`Line.pas:735-787`) and 0.14.5
(`src/PDElements/Line.pas:889-930`) copy the impedances, `Len`,
`SymComponentsModel` and `FCapSpecified` only, so a `like=` line renders
`linecode = ''` on both — while the port copies name, handle, flag,
`line_code_units`, `units_convert`, `length_units` and `user_length_units`. The
new flag joins that copy set so the port's state stays internally consistent
(the pin asserts the pair cannot drift); narrowing the whole set is a separate
change with numeric reach (`length_units`/`units_convert` feed `FUnitsConvert`),
it has no census cell — no corpus deck writes `like=` on a coded line, which is
why `line.linecode` is 5 switch-shaped cells and nothing else — and it belongs
with a class-wide `MakeLike` sweep. **(iii) The
`Conductor.length`/`LineCodeRefNode` branch is unreachable for a switched line
on all three engines** — it sits in the `else` of `if IsSwitch then`
(`ExportCIMXML.pas:3709`), measured on the probe's deck C, where the switch
exports as a bare `LoadBreakSwitch` identically everywhere — so repointing
`has_line_code` onto the flag has no observable consequence today and is a
faithfulness change only. **(iv) Name casing.** r4133 stores `LowerCase(Code)`
and the port stores the *resolved object's* name (lowercased at creation), so
the two agree; the CIM comparison stays `eq_ignore_ascii_case` because both
engines compare two already-lowercased strings, and no corpus deck names a code
in mixed case. **(v) `FetchConductorList`'s flag clear has no port counterpart.**
r4133's `Conductors=` (property 34, dispatched at `:654`) enters
`FetchConductorList`, which opens `FLineCodeSpecified := False;
KillGeometrySpecified;` unconditionally (`:1853-1854`) — a third rule, distinct
from `SetWires`' `FPhaseChoice = Unknown` guard (`:1952`) and from the `switch=`
arm's silence — where the port's `set_conductors` only fills the array. **This
paragraph originally continued "and cannot acquire an observable one", argued
that `FetchLineCode` kills the spacing at its tail so r4133 reaches the statement
with a nil `FLineSpacingObj`, and recorded the counterpart as written, pinned and
then reverted as vacuous. That argument was wrong and is retracted: `:590-591` is
*dss_capi 0.14.5*'s `FetchLineCode` tail (`src/PDElements/Line.pas:581-582`),
not r4133's — r4133's arm-3 side effect is a *plain* `SpacingSpecified := False`
(`:663`), so the spacing object survives the code and the statement is reached
normally.** Ported, with the measurement, by the audit settlement below.

**RP3.6 audit settlement (2026-08-29, one commit over `4b146ab9`).** Fourteen
findings across the two audit agents. Both re-derived the mechanism on the live
oracles and confirmed the sub-step's own classification — outcome stays `FIX` in
both lanes, the census still decomposes to 5 cells / 5 in scope, both
`capi_v0145` entries, their cause and the two lock lines are unchanged, no
upstream bug is reproduced anywhere, no `TODO(compat)` was added, no test was
deleted, `#[ignore]`d or loosened, and no golden byte, frozen extract or ledger
scope moved. What the settlement adds is **three real product defects fixed**
(five statements, all in the family the sub-step opened), **one new pin and two
extended ones**, **one new machine guard**, and seven corrections to the record.

| # | finding | verdict | evidence | action |
|---|---|---|---|---|
| 1 | `FetchConductorList` (`:1853`) has no port counterpart, and (v)'s justification is false | **REAL** | r4133 DLL: `spacing=sp1 linecode=lc1 conductors=[…]` → `? linecode` = `''`, `r1` = `'----'`, **no error**; the port answered `'lc1'`/`'0.1'` plus a spurious #402 | ported (`code.rs::set_conductors`), (v) retracted above |
| 2 | root cause: `fetch_line_code` kills the spacing where r4133 clears a flag | **REAL** | the same probe: reaching `conductors=` at all proves `FLineSpacingObj` survived; `:663` is a plain assignment, `:581-582` is capi's | `spacing_specified` field; the tail is now the flag only |
| 3 | arm 15 calls `Kill*Specified` where r4133 assigns two Booleans (`:696`) | **REAL** (spacing) / **REAL but unobservable** (geometry) | A/B on identical decks: after `switch=yes` r4133 runs a following `conductors=`; after `r1=` it **access-violates**. For geometry every r4133 reader of `FLineGeometryObj` is itself flag-gated, and `? …geometry` reads `''` on both engines | spacing → plain flag drop; geometry left as-is, argued and cited in the arm |
| 4 | the `FUnitsConvert` consequence is pinned only through the rendered `r1` | **REAL** | with the RP3.6(a) kill restored the port solves `A.1 = B.1 = 7197.75809229906`, 6.4e-6 off r4133's `A.1` — 6 400× the pin's tolerance | solve leg added, pinned against the r4133 DLL's `YNodeVarray` |
| 5 | a landed `capi_v0145` property entry has no machine-checked witness pin | **REAL** | deleting `switch_yes_keeps_…` left the suite green (the entry pins both sides, nothing pinned that the port's side is *right*) | `LANDED_PROPERTY_ENTRY_PINS` + `every_landed_property_entry_has_a_witness_pin_that_exists`, both halves proven red |
| 6 | the new `Save Circuit` emission is unpinned; STATUS said `R1=1` | **REAL** | the port emits `… LineCode=lckft … Switch=Yes R1=0.00328083989501312 …` — the getter's value, i.e. `1/304.8` | Save leg added to the pin (+ an arm-6 control); the STATUS sentence corrected in place |
| 7 | the third deliberate CIM-writer divergence is not listed with the other two | **REAL** | zero-footprint today (every `New LineCode` in `tools/golden/cim_decks/*.dss` declares `units=`), so a fourth rewrite would red `cim_writer_divergences_are_pinned` | recorded in `expected_cim`'s doc with the reason it is not a rewrite |
| 8 | "the six other `kill_line_code_specified` call sites" | **REAL** (arithmetic) | seven remained after part (a); r4133 has eight; the port now has all eight | corrected in `ledger.json`'s cause, `STATUS` §RP3.6(a), the routing comment |
| 9 | `line_code_ref` is dead product state with a doc promising a reader | **REAL** | `grep line_code_ref crates/` → writes only, plus one test reader; every product site reads the flag | doc rewritten to say exactly that; the field stays as this class's member of the typed-handle family |
| 10 | four Pascal citations point at capi while reading as r4133 | **REAL** | `:544-547` is capi's `NoPropertyTracking` block; `:590-591`, `:2141`, `:2042` likewise | all four re-cited (capi labelled as capi, r4133 line numbers fixed) |
| 11 | the pin asserts `line.cp.linecode == "lcnone"`, which both oracles contradict | **REAL, OUT-OF-SCOPE** | r4133 DLL: `like=` copies the impedances (`? r1` = `'0.1'`) but not the source (`? linecode` = `''`), on a plain and on a switched coded line | assertion relabelled a **divergence lock**; owner created — `ORPHANED_GAPS.md` §1.12 |
| 12 | the `FIX`-shape guard does not reach `RP22_ROUTING`/RP3.5+ | **REAL, recorded** | `the_bin7_root_cause_pairs_are_routed_to_their_sub_steps` walks `RP3_ROUTING` only; `line.linecode` lives in `RP22_ROUTING` under `Owner::Rp35` | left as-is deliberately — finding 5's guard is the obligation that actually bites, and widening the shape guard is a WP-RP3 accounting change, not an RP3.6 one |
| 13 | `assert_eq!(swk.r1, "0.00328083989501312")` is a self-comparison, not an oracle value | **REAL** | r4133 renders the same f64 as `0.00328084` (`%-.7g`) — the literal is the port's own width | the numeric `1/304.8` assertion now comes **first**; the literal follows, labelled as a render-width lock |
| 14 | `has_line_code`'s repoint onto the flag is unverifiable | **REAL, recorded** | flag and handle rise and fall together at every site, and the `Conductor.length` branch is unreachable for a switch (`ExportCIMXML.pas:3709`) — measured on all three engines by the sub-step's probe | left as a faithfulness change, as part (b) already said; finding 9's doc rewrite is what keeps the handle honest |

**What changed in the product (both lanes, five statements).** The port now
models `SpacingSpecified` the way it models `FLineCodeSpecified` since part (b):
a Boolean field (`Line::spacing_specified`) beside the objects, not a predicate
over them. r4133 raises it only in the `21..22, 24..25, 34` block and only once
both the spacing and a conductor list exist (`:704-713`); `KillSpacingSpecified`
is guarded by it (`:2268`) and takes the objects with it (`:2266-2276`); the
`linecode=` side effect (`:663`) and the `switch=` arm (`:696`) drop the flag
alone. dss_capi 0.14.5 cannot express the difference — its `SpacingSpecified` is
`Assigned(LineSpacingObj) and Assigned(LineWireData)` (`src/PDElements/
Line.pas:2112-2115`) — and the port had copied it. `set_conductors` gained
r4133's `FLineCodeSpecified := False; KillGeometrySpecified;` (`:1853-1854`), so
all **eight** of r4133's clear sites now have a counterpart and the `switch=` arm
still has none. Four measured divergences close with it, all four new here:

* `spacing=sp1 linecode=lc1 conductors=[…]` → `linecode ''`, `r1 '----'`, no
  diagnostic (was `'lc1'`, `'0.1'`, spurious #402);
* `spacing=… wires=[…]` + `switch=yes` + `conductors=[…]` runs (was #402);
* `spacing=` alone keeps `SymComponentsModel`, so `? r1` = `'0.058'`, the class
  default (was `'----'` — 0.14.5's timing);
* `spacing=` + `r1=0.55` + `wires=[…]` runs (was 0.14.5's #18102), because the
  kill is a no-op while the flag is down.

r4133's own answer to the destroyed-spacing cases is an **access violation**
(`FWireDataSize := FLineSpacingObj.NWires` after a `DoSimpleMsg` that does not
`Exit`, `:1850-1856` and `:1948-1955`, and the same shape in `FetchCNCableList`
`:2014-2022` and `FetchTSCableList` `:2073-2081`) — reproduced nowhere: the port
keeps its clean #402 / #18102, and the pin asserts that. Written up as
`investigations/to_opendss/
46-line-fetchconductorlist-nil-spacing-access-violation.md` (gitignored,
local-only; RP3.5 took **45**, so the next free number is **47**), with the
`switch=yes`-vs-`r1=` A/B as its reproduction.

**Blast radius: none, and swept rather than assumed.** Grouping each `New`/`Edit`
with its `~` continuations into one logical command, `tests/corpus`,
`tools/golden`, `tests/golden` and `crates/dss-core/tests` hold **2 651** `Line`
edits that name a `spacing=`, across nine files (`line_spacing_asym`,
`IEEE13_Assets` ×2, `IEEE13_LineAndCableSpacing`, `IEEE13_LineSpacing`,
`ieee9500_base`, `makeposseq_line`, `upgrade_spacing_ratings`, `cim_lines.dss`).
**Zero** of them lack a `wires=`/`cncables=`/`tscables=`/`conductors=` in the
same edit, and **zero** combine `spacing=` with `linecode=` — so the flag rises
and falls at exactly the instants the old predicate did. The one interleaving
that could have disturbed it is real and was checked: 2 622 of those edits are
`ieee9500_base`'s `spacing=… units=ft` / `~ normamps=… emergamps=…` /
`~ wires=[…]`, i.e. the ratings land BETWEEN the spacing and the conductors.
Block 2 used to fire twice (at `spacing=` and again at `wires=`) and now fires
once, at `wires=` — after the ratings either way — so its
`clear_seq(SEASONS…C0)` and `got_ratings_after_spacing_conds = false` land on
the same state, which the 521-case gate then confirms.

`cargo test --workspace` is green in both lanes (4 219 tests, 74 `test result:
ok` blocks each) with **no** golden, `golden.lock.json`, `ledger.json` scope or
`population.lock.json` line moved;
`lane_diff.ps1` was re-run as a measurement rather than argued — the solved state
of the gated corpus is out of reach by the sweep above, but this settlement moves
`spacing_specified`, which `CalcYPrim` branches on (`line/solve.rs`): 522 cases,
3 220 247 records, **`max |Δ| = 0.000e0` and `max rel = 0.000e0` on all eight
kinds** (conv 2 150, cur 1 169 500, errs 518, iter 2 150, loss 366 320,
pow 1 169 500, v 375 744, y 1 738 048), 0 iteration counts drifted,
**VERDICT: PASS**.

**Pins.** `exec::tests::line_fetch::
conductors_clears_the_linecode_flag_and_the_switch_arm_spares_the_spacing` is
new: six legs, each an r4133 measurement, including the `switch=yes`-vs-`r1=` A/B
on identical decks. `switch_yes_keeps_the_linecode_and_its_units_conversion`
gained the solve leg (three r4133 node voltages at rel ≤ 1e-9 — measured gap
2.3e-12 — plus the `1 : 304.8` drop ratio) and the `Save Circuit` leg.
`props_r4133_replay::every_landed_property_entry_has_a_witness_pin_that_exists`
is the new guard. **Non-vacuity measured for every one**, by reverting each
product statement and reading the failure text: the `set_conductors` kills →
`left: "lc1" / right: ""`; the `fetch_line_code` tail → `Line.p.Conductors: No
objects are expected!` (#402); the `switch=` arm → the same on `Line.s`; block
2's `idx != SPACING` → `left: "----" / right: "0.058"`; the `KillSpacingSpecified`
guard → `You must assign the LineSpacing before the Wires Property ("Line.g")`;
re-adding `clear_seq(LINECODE)` to the `switch=` arm → the emitted `New
"Line.swk" …` line without `LineCode=`; restoring part (a)'s kill → the solve leg
reads `A.1 = B.1`; renaming the witness pin → `line-switch-keeps-linecode-zone2-
capi-props (RP3.6) names the witness … but … defines no such #[test]`; emptying
`LANDED_PROPERTY_ENTRY_PINS` → `left: {} right: {…three ids…}`.

**Recorded, not chased.** (a) The geometry half of arm 15: r4133 leaves
`FLineGeometryObj` alive with `GeometrySpecified` down, the port nils it. Every
r4133 reader of that object is flag-gated (`:721`, `:1051`, `:1211`, `:1284`,
`:1371-1393`, `:1403`, `ExportCIMXML.pas:3741`), and `FZFrequency` is re-armed by
the next `FetchGeometryCode`, so the difference is unobservable — probed, `''` on
both engines — and the port has no separate geometry flag to spend on it. Argued
in the arm itself. (b) `MakeLike`'s copy set → `ORPHANED_GAPS.md` §1.12, with the
r4133 measurement. (c) The port's arity check on `wires=` (`Line.<x>: Unexpected
number (n) of wires; expected m objects.`) is 0.14.5's addition
(`src/PDElements/Line.pas:841`); r4133 validates nothing and silently runs an
empty loop. Pre-existing, unrelated to the flag, and the pin avoids the shape.
(d) The `? …spacing` render moved *toward* r4133 as a side effect (it now answers
the surviving object's name where the port used to answer `''`); r4133 echoes
`PropertyValue[21]` there — no getter arm at `:1347-1429` — so this narrows the
`line.spacing` census pair (`Owner::Rp23`) and widens nothing.

**RP3.7 (per-phase switch and relay state) landed 2026-09-02 — `FIX` in both
lanes for all three parts, and the widest RP3 sub-step so far: 50 engine, test,
golden and ledger files (plus this record and three docs), the two control classes
rebuilt on r4133's `pStateArray` model, ten overlaid golden cells and five live
`capi_v0145` ledger entries.** The record below is measured
throughout: every byte quoted from r4133 comes from the vendored EPRI DLL
(`Version 11.0.0.1 (64-bit build) - Charlottesville`) through `epri-worker`, and
every count is derived by a test rather than transcribed.

**The probe ran first, on all three engines, and no kill criterion fired.**
r4133 DLL, the pinned dss-python 0.15.7 / dss_capi 0.14.5, and the port built
out-of-tree by path, over seven purpose-built micro-decks plus the vendored
`civanlar` / `makeposseq_ctrl` shapes; no repo byte was written by the probe.
**(1) Per-phase independence is real, on two independent observables.**
`edit swtcontrol.sw1 state=(open, closed, closed)` renders
`[open, closed, closed, ]` and, after `solve`, zeroes exactly phase 1 while
phases 2/3 keep `-6.953452-12.035939j` / `-6.935750+12.030079j`; the bare ganged
`state=open` zeroes all three. **(2) The render is one token per
controlled-element phase** — `[closed, ]` on a 1-phase element, three tokens on a
3-phase one, four on a 4-phase one (which r4133 accepts although it allocates
three entries), and `[]` when there is no controlled element. **(3) The (a2) lock
rule is exactly the source reading**: under `lock=yes` a ganged `normal=open`
moves `Normal` to `[open, open, open, ]` and so does a *quoted* locked
`normal=(open, closed, open)` → `[open, closed, open, ]` — the guard is on the
property **name**, not the value shape — while locked `state=` / `action=` move
nothing. **(4) Relay's resync is the getter's live bound**, not a state-array
write: `MakePosSequence` (`Relay.pas:1008-1027`) does not touch the arrays, and
`GetPropertyValue` 39/40 loop the live `ControlledElement.NPhases` (`:1407-1428`),
as `Sample`, `RecalcElementData` and `Reset` do. **(5) capi 0.14.5 refuses the
per-phase list on two independent channels**, which the plan text had merged into
one: the locked-write channel (`ConditionalReadOnly`) and the enum-mismatch
channel — `SwtControl`'s `StateEnum.DefaultValue := ord(CTRL_CLOSE)` makes a
quoted list fall back to closed **silently**, while `Relay` sets no default and
raises `#303` (`Common/DSSClass.pas:2440-2532`).

**(a) The per-phase port.** `FPresentState`/`FNormalState` become
`[ControlAction; 7]` arrays bounded by `SW_MAX = 6` (`SWTCONTROLMAXDIM`,
`SwtControl.pas:14`) beside `normal_state_set` (`NormalStateSet`, `:42`), both
initialized all-CLOSED by `Create` (`:299-307`) — which is why a fresh control's
`Normal` moves off the port's old `''`. `interpret_switch_state` is
`InterpretSwitchState` (`:410-482`) ported whole: the name-based lock guard, the
always-ganged `Action` arm, the ganged 1..6 fill for an unquoted token, and the
phase-by-phase branch through a **fresh** `dss_parser::Parser` with r4133's
five-token cap (`i < SWTCONTROLMAXDIM`, `:461`), first-character matching and
unlisted slots left unchanged. It is the **single write mechanics** for all three
properties: `Normal`/`State` reach it through a new raw hook
(`set_enum_array_raw`) and `Action` through `set_i32`, which maps its decoded
ordinal back to the canonical first character — so the interpreter carries no
production-dead branch. The drive model is r4133's *parse-time* one: `set_States`
drives `ControlledElement.Closed[i]` mid-Edit (`:532-549`) and
`RecalcElementData` re-drives every phase at `EndEdit` (`:234` → `:347-355`), the
last write of the two, so the port defers a per-conductor
`RefAction::SetConductorsClosed` (was the whole-terminal `SetSwitchClosed`) at
`recalc`. **Bounds: in-bounds by decision.** r4133 allocates three entries and
reads/writes up to six — a latent heap OOB absorbed by FastMM — so the port keeps
six initialized in-bounds slots and reproduces only the observables: the render
and drive bound `state_size()` = `min(6, controlled-element phases)` (`0` for a
nil element) and the five-token parse cap, which stays deliberately *different*
from the six-token render bound. **`WasQuoted` is plumbed** to the property seam
(`obj/props/class_props/{parse,typed}.rs`, `obj/props/engine.rs`,
`exec/{command,json_import,make_pos_seq}.rs`, `obj/base/mod.rs`) because a
**single** token is the one shape where quoting changes the meaning: measured on
r4133, `state=(open)` gives `[open, closed, closed, ]` where `state=open` gives
`[open, open, open, ]`. Seams with no outer parser (JSON import, the
`MakePosSequence` applier, direct calls) reconstruct it — `value_implies_quoted`:
a leading `(` `[` `{` or quote, or more than one token — and a bare single token
resolves to *ganged*, the only spelling any corpus deck writes. Finally the two
enum registry entries gain `allow_longer` and a `Keep` default, so `state=bogus`
is **silently unchanged** as on r4133 — neither the port's pre-RP3.7 error nor
capi's silent close.

**(a2) The locked-`normal` rule landed with it, in both lanes.** The scalar-era
`Locked` gates are gone from `set_i32` and from `side_effects`; the only guard
left is r4133's own, inside the interpreter. Two facts the sub-step measured
rather than read: the guard refuses `Action`/`State` by property name
(`:416-417`, comment *"Only allowed to change normal state if locked"*), and the
`{Supplemental Actions}` block (`:220-228`) sits **outside** the arm, so
`NormalStateSet` latches even on a write the guard refused — the discriminating
pair is a locked `state=open` followed, after `lock=no`, by another `state=open`
(Normal stays `[closed, closed, closed, ]`) against the same sequence unlocked
(Normal follows to `[open, open, open, ]`). `docs/upgrade/DIVERGENCES.md` §D12,
which asserted the opposite refusal and the scalar field mapping, now carries a
`Settlement (2026-09-02)` paragraph with the per-phase model, the lock rule
verbatim, five pin names and the ledger consequences.

**(b) Relay: one fix, two symptoms — and four more mismatches on the same seam.**
The plan framed (b) as a render gap; the probe showed that a single frozen
`ctrl_snap` produces **both** the three-token render *and* a `Sample` that
resyncs phases 2..3 off the end of a 1-conductor element, which is exactly why
the frozen census cell reads `[closed, open, open, ]` and not
`[closed, closed, closed, ]`. Refreshing `ctrl_snap` inside `make_pos_sequence`
(the same refresh SwtControl took) settles both. Under "port gaps immediately"
the probe's four further r4133 mismatches on that seam were fixed with it: a
quoted single token was read as ganged, a refused or unmatched `action=` did not
run the normal-defaults supplemental (`Relay.pas:616-619` covers internal 19 as
well as 40), and the generic tokenizer honored six tokens where `:1286` honors
five. Relay took the same raw write seam (`interpret_relay_state`,
`Relay.pas:1237-1308`), and its old `values.len() == 1 → ganged` heuristic — the
bug behind the quoted-single-token rows — is deleted. r4133's own 6-phase
initialization OOB is **not** reproduced, per the 2026-08-02 policy: `Create`
allocates three entries and the getter loops the controlled element's six phases,
so slots 4..6 are an out-of-bounds heap read with **no defined value**. The audit
settlement measured it both ways on the same DLL — on `decks/b1_relay6.dss`, which
ends in a `solve`, a fresh relay reads
`[closed, closed, closed, open, open, open, ]` (reproducible 2/2), and on the same
construction stopped before the `solve` it reads all-closed; the flip happens at
the `solve`, i.e. it tracks heap content, not the model. The pins therefore gang to
a known baseline first and assert nothing about a fresh 6-phase render.

**The cell arithmetic — and the measurement that decided it.** `bins.tsv` gives
`swtcontrol.normal` and `swtcontrol.state` **59 cells / 40 in scope** each (the
plan's 80), `relay.normal`/`relay.state` **1 / 0** each. The 80 in-scope cells sit
on three `engines=r4133` decks — `controls:swtcontrol/midi_swtcontrol.dss` (12),
`controls:swtcontrol/swtcontrol_time.dss` (12) and the `civinlar model`
`civanlar.dss` (16 ties × 1 step) — i.e. **not** on the decks the fix reds. The
first census run over those three decks (every earlier sweep had covered only
out-of-scope cases) reports **zero** `Normal`/`State` rows on the r4133 channel:
the 160 SwtControl divergence rows that remain there are `Reset` 40, `enabled`
40, `Delay` 24 (RP3.1's staged entries), `Action` 16 (RP2.3 `EchoParse`) and
`SwitchedObj` 40 — none of them this pair, and RP3.7 moved none of them. Read
independently off the r4133 DLL on the same decks — all 16 civanlar ties (13
closed / 3 open) and both duty decks before and after the manifest post — the
renders are **byte-identical** to the port's. So the disposition of the 80
in-scope cells is **80 / 80 COMPARE**, not excluded and not pinned by an
exclusion, and **no r4133 `property` ledger entry is needed, staged or drafted —
zero, as a measurement rather than a judgement**; a landed one would fail
`assert_all_hit` as NEVER APPLIED at RP4.1. Until the RP4.1 unmask lets a channel
witness them, the port's value there is held by plan §1.1(c)'s holder pin
`exec::tests::controls::swtcontrol_state_renders_per_phase_on_the_r4133_only_decks`.
*(**Witnessed 2026-09-03**: the RP4.1 unmask compared exactly those 80 cells —
40 on `swtcontrol.normal` and 40 on `swtcontrol.state` — and every one matched
without normalization, which is what exposed the two `ArrayForm` rows as stale
and retired them into `RP37_SUPERSEDED`; §RP4.1 record. The holder pin stays.)*
The other **19 cells** are out of scope, on five `capi_v0145` cases, and they are
excluded by **five landed entries under one new cause**
(`swtcontrol-per-phase-state-render`): `controls:swtcontrol/swtcontrol_lock.dss`
carries **both** exposure channels — its `state`/`normal` probes *and* its
full-property compare, over 12 steps ⇒ 48 hits —
`modes:makeposseq/makeposseq_ctrl.dss` 2 (the 1-token `[closed, ]` render after
`MakePosSequence`), and the three `IEEE_519.DSS` copies 4 each (two controls ×
two properties). Every cell is a non-numeric exact pair, so both `oracle` and
`rust` are pinned and each entry goes stale the day the port converges to the
0.14.5 scalar. `ledger.json`: causes **26 → 27**, entries **40 → 45**, hits
**1442 → 1504**; `population.lock.json` moved **exactly five lines**, one
`ledger=` field per case. `relay.normal`/`relay.state` need **no** entry on either
channel: Relay is whole-element-skipped on capi (re-measured on `makeposseq_ctrl`
— 2 elements / 80 cells skipped) and both cells are now byte-identical to r4133.
**`DECLARED_RP35` stays `(8, 6, 5)`** — the contract's own answer, verified twice
(the replay was 131 green before the routing rewrite and 132 after): `declare`
reads the **frozen** `rust` column, so a settled sub-step does not shrink the
bucket, exactly as RP3.5 and RP3.6 recorded. `CLAIMED_ARRAY_FORM` (203),
`ROWS_FROZEN`, the other `DECLARED_*` counts and the props count locks 51 / 322 /
8343 are all unchanged. The four routing rows (`swtcontrol.normal`/`.state`,
`relay.normal`/`.state`) are rewritten as settled `FIX` verdicts, and a new
derivation test
`props_r4133_replay::the_rp37_census_decomposition_is_read_off_the_corpus`
reconciles the decks, `population.lock.json`, the frozen census (per spelling —
58 + 1 and 31 + 27 + 1 — and in total) and `ledger.json` against each other, so
"exactly five entries" is derived rather than asserted in prose.

**Goldens: ten cells, one schema block, one prose line, three digests.** The ten
`Normal`/`State` cells of `tests/golden/props/swtcontrol.json` were **predicted
first, then measured on the authority** — the five scenarios replayed through the
r4133 DLL with the gate's own `clear` + `new circuit.propsprobe` preamble — and
the port matches r4133 byte-for-byte on all ten and the committed golden on all
45 unmoved cells; the artifact's diff is those ten lines plus its provenance
prose. `Action` moves in none of the five scenarios. In
`tests/golden/json/schema_full_port.json` (regenerated in its producing parity
lane) `SwtControl.properties.{Normal,State}` become `type: array` + `items: $ref`
with **no** `default` key — the same **array** shape `Relay` already had, though
the audit settlement corrected the stated reason for the missing default: the
emitter elides one for any property flagged `NO_DEFAULT` (`State`) or
`DYNAMIC_DEFAULT` (`Normal`), decided **before** the sample object is read
(`schema/classes.rs::no_default`), and the empty sample array would elide it
independently. That is why the "shape Relay already had" held for `Normal` but not
for `State`, which carried a 3-element default until the settlement ported r4133's
NIL-element getter guard to Relay (below) and emptied its sample array too.
`schema_full_oracle.json` is untouched and `schema_divergences.json` gains one
amended `cause` line. `golden.lock.json` moved
three digests and no anchor. **The anchor decision A2b left open is: keep
`capi015`.** Precedent and evidence point the same way — WP-U2.4 already overlaid
r4133-only behavior on this same artifact, and the G0.1 provenance lock, six weeks
later, registered it `capi015` *with* that overlay in place; `CAPI015_REASON` is a
statement about the unreproducible 0.15.0b4 generator environment, still exactly
true; and r4133 is demonstrably **not** this artifact's value authority — its
`Action`, `Reset`, `Enabled`, `SwitchedObj` and `Delay` cells still hold
capi-side values r4133 diverges from, so an `R4133_FAMILIES` row
(`props/fuse.json`'s shape) would be a false statement and a `DEANCHORED`
"born-self" row a second one. The ten overlaid cells are recorded in the
artifact's own `oracle.engine` block with their r4133 line citations — and, since
the audit settlement, in the **lock row** as well: the shared `CAPI015_REASON`
could not carry a per-artifact note, so a new register `CAPI015_OVERLAYS` appends
one (the construction `props/fuse.json`'s `R4133_FAMILIES` reason states, now
available to an artifact that stays `capi015`). `golden_lock` (4) and
`props_roundtrip` (1) are green in both lanes with the anchor unmoved.

**Pins.** Unit level, both lanes: `swt_control` 26 → **51**, `relay` 63 → **68**,
`dss-core --lib` **1464** (1462 before B2's two engine-level pins) — the
before-counts re-measured by the audit settlement, which corrected the 30/64 this
paragraph first carried and added two more relay pins (`relay` **70**, lib
**1466**; see the settlement paragraph). The load-bearing ones are
`render_is_one_token_per_controlled_element_phase`,
`nil_controlled_element_renders_the_empty_array`,
`a_quoted_single_token_is_per_phase_a_bare_one_is_ganged`,
`per_phase_write_renders_the_r4133_bytes_through_both_seams`,
`the_property_seam_caps_the_per_phase_parse_at_five_tokens`,
`a_multi_token_value_without_the_quote_flag_is_still_per_phase` (the JSON-import
value-string seam), `the_render_bound_follows_makeposseq`,
`a_locked_state_write_still_runs_the_normal_defaults_supplemental`,
`locked_normal_applies_locked_state_and_action_do_not` (plan §RP3.7(a2)'s required
pin — the renamed `locked_ignores_normal_and_state_writes`, now the full probe
byte sequence through the executive),
`per_phase_state_write_through_the_executive_opens_only_its_phase` and
`per_phase_open_zeros_only_its_phase_currents_on_a_micro_deck` (the solved
per-phase drive), plus the Relay twins
(`a_refused_or_unmatched_action_still_runs_the_normal_defaults_supplemental`,
`the_render_bound_follows_makeposseq`, the cap and quoted-token pins). Two
engine-level pins carry the corpus:
`exec::tests::controls::swtcontrol_state_renders_one_token_per_controlled_phase`
— the witness for all five landed entries, reading the two gated decks from disk
and closing with a 1/2/3-phase discriminator so it cannot pass against a
hardwired three-token string — and the §1.1(c) holder above; both were proven
non-vacuous by corruption. `props_r4133_pins::swtcontrol_action_renders_the_live_
switch_state` moved its two `State` lines to the r4133 DLL's own bytes (`13_14` →
`[closed, closed, closed, ]`, `10_14` → `[open, open, open, ]`) and gets *sharper*
by it: `Action` still renders one word beside an array, and the two still agree.

**The verify-A1 round (12 findings, settled inside this sub-step).** One is
**refuted by measurement and must not be carried anywhere**: the "silent no-op
after a failed edit" defect the probe reported (`probe.md` §8.3/§11.6/§12) was a
probe-harness artifact — the harness printed a delta against a cumulative error
high-water mark while every `Compile` in its loop cleared the error list; with
absolute error lists printed, the later edit **applies** in all six variants and a
second failing edit is reported. Two findings were already fixed by the render
part (the JSON seam's per-phase reconstruction; the `min(6, nphases)` clamp), five
landed as code or doc records — r4133's `Else`-without-`Begin` AuxParser
fall-through (`SwtControl.pas:452-455` and `Relay.pas:1277-1306`, with
`Fuse.pas:569-597`'s correct `Else Begin` as the counter-example, which is what
makes it an upstream slip rather than a design); the lock guard keying on
`LowerCase(ParamName[1])` of an **empty** string for a positional write; the
mid-edit `switchedobj=` re-point that makes r4133 drive the *old* target too; and
the registry comment that called `NO_DEFAULT` "the Pascal default 0" — and one
tolerance was **tightened**: the micro-deck current band went from an ad-hoc
`5e-4` *relative* (≈6.9e-3 A, blind to a ~7 mA per-phase drive error) to the
calibrated `abs 1e-6` element-current floor from `tests/TOLERANCE_NOTES.md`, ~500×
tighter, against full-precision r4133 magnitudes whose worst measured delta is
3.66e-7. The first three are `investigations/to_opendss/` candidates, left
unowned. Two findings are **recorded, with both engines measured**. (i) The
retained 0.14.5 `Sample`/`DoPendingAction` machinery arms on a `normal=` write:
after `edit swtcontrol.sw normal=open` on an unlocked control the port opens the
switch at step 3 of the duty run (`State = [open, open, open, ]`, event log
`Hour=0, Sec=0.5, ControlIter=1, Element=SwtControl.sw, Action=OPENED`) where the
r4133 DLL leaves it closed for all eight steps and logs nothing — r4133 comments
both bodies out. A **locked** `normal=` arms it too, which is new with (a2): the
scalar era refused that write outright, r4133's guard lets `n`ormal through, and
the port now applies it — the switch stays shut (`do_pending_action` is
`!locked`-guarded) but the queue push and the `armed` latch happen (audit
settlement; both arms are in the tripwire). Retiring the machinery has **one**
blocking channel, not two: `controls/swtcontrol/swtcontrol_lock.dss` is gated
`capi_v0145` with `compare_ctrlqueue`, so the capi lane pins the spurious
`CTRL_LOCK` push, and retiring the body means re-gating that deck onto `r4133` —
giving up the only capi deck that both probes and property-compares a SwtControl,
and retiring one of the five entries RP3.7 just landed. That is a channel
decision RP3.7 **chose not to take**, not one it could not make (it edited both
`population.lock.json` and `ledger.json` for other reasons); and the capi015
props golden's `Action` readback is **not** a second blocker — it reads
`current_action` through `get_i32(ACTION)`, which the property side effects
maintain and neither `Sample` nor `DoPendingAction` writes. Corpus exposure is
zero (every corpus `normal=` is a ganged `normal=closed` over an all-closed state,
and `swtcontrol_lock.dss` types `normal=closed` *before* `lock=yes` on the same
`New`), the divergence is held by the tripwire
`swt_control::tests::sample_arms_on_a_normal_write_the_retained_capi_channel`,
whose doc says it must be **deleted, not re-baselined**, when the body goes, and
it is now registered as `ORPHANED_GAPS.md` §1.16 — the only RP3.7 item that moves
a solved result, so a test doc was too weak a home for it.
(ii) The render bound's residual staleness is observable on both engines: after
`edit line.swk phases=1` the r4133 DLL's `? swtcontrol.sw1.state` follows
immediately to `[closed, ]` (its getter loops the live element) while the port
keeps three tokens until the ref is re-resolved by a `switchedobj=` write. That is
architectural — the port's classes hold ref *snapshots* by design — shared with
Relay, and has zero corpus exposure; it is now `ORPHANED_GAPS.md` §1.13. Two
smaller ones: `props_roundtrip` has no write-back path at all, so the audit's
suspicion there is void; and the enum `Keep` default can be narrowed for `State`
but never for `Action`, whose scalar seam needs it to reproduce r4133's no-else
fall-through.

**Recorded, not chased** (each with its evidence; the `tmp/rp37/out_*.txt`
transcripts named here are local, gitignored probe artifacts). (a) The
`RecalcElementData` bits the port never had (`SwtControl.pas:333-344`): the
`ElementTerminal > NTerms` check (`DoErrorMsg` 384) is a **genuine validation
gap** — the port silently clamps while emitting the sibling 387 for a missing
element — and goes to `ORPHANED_GAPS.md` §1.15 together with the `FNphases >
SWTCONTROLMAXDIM` warning; `HasSwtControl := TRUE` (`:344`) is **dead upstream
state** (declared `CktElement.pas:99`, initialized `:213`, set here, never read),
so that half closes by reading rather than deferring. (b) The sibling state seams
RP3.7 did not touch — Recloser's twin defects (the same `values.len() == 1 →
ganged` heuristic and a whole-object lock that refuses `Normal` too, where
`Recloser.pas`'s guard is the same name-based rule), Fuse's missing ganged path,
Relay's absent `ControlledElement = NIL → []` render and its `set_States`
`ArmedForReset` — are `ORPHANED_GAPS.md` §1.14; none has corpus or census
exposure. The NIL **render** was deliberately not landed here on a hunch
(`state_size()` has 14 call sites) and the audit settlement landed it after
measuring it: r4133's guard lives in the getters alone (`Relay.pas:1407`/`:1418`),
so the port carries it in a render-only `render_size()` and leaves the twelve
sensing/reset loops on `state_size()`; §1.14(c) keeps the behavioral half (r4133's
`Reset` restores nothing with a nil element, `:1447`) and gains the same unported
guard on Recloser and Fuse. (c) `Dump`'s store-vs-live echo for
properties 6/7 goes to **§RP3.11** (*settled 2026-09-03 —
`KEEP_LIVE_PINNED`: the port keeps the live render on `Dump` too, and the
divergence is pinned rather than reproduced; record below*): r4133's
`DumpProperties` (`:563-571`) echoes
the stored parse text (`~ Normal=` when never written) while the port's generic
dump renders the live value, consistent with `?`, `all_properties` and `Save`; no
golden and no gated cell renders a SwtControl `Dump` today. `Save` itself
round-trips the array (`save_roundtrip` 9 green — the writer emits
`Normal=[closed, closed, closed, ]` and the outer parser reads the brackets back
as a quoted per-phase list). (d) `MakeLike`: r4133 copies **every** other piece of
SwtControl state and omits `NormalStateSet` alone, whose observable is a clone's
first `state=` write silently collapsing the base's declared `Normal` (measured on
the DLL); the port copies the flag, i.e. does not reproduce it, and the omission
is written up as `investigations/to_opendss/47-swtcontrol-makelike-drops-
normalstateset.md` (gitignored, local-only — RP3.6 took 46, so the next free
number is **48**). Verifying that report turned up a **second, previously
unrecorded r4133 defect**, included in it: the same copy loop bounds itself with
`ControlledElement.Nphases` on a pointer copied verbatim one line earlier with no
nil check, so cloning a base whose `switchedobj=` never resolved
access-violates — `Error 303 … Access violation … Read of address
000000000000007C`, caught, the clone left half-built with `? …Normal` → `[]`. The
port has no equivalent (its clone path is snapshot-based and the `[]` render is
pinned), so there is nothing to fix in-engine. (e) `? relay.x.switchedobj` still
renders the defaulted name where r4133 renders `''` — a pre-existing
`CaseFold`-claimed census pair, not this sub-step's.

**A hazard worth carrying: compiling `IEEE_519.DSS` in place rewrites tracked
vendored corpus files.** The deck ends in `export monitor MPCC` + `show monitor
MPCC`, so a hand probe of the three copies left
`.../HarmonicsTMode/IEEE_519_Mon_mpcc_1.csv`,
`.../HarmonicsTMode/IEEE_519_SavedVoltages.dbl` and
`.../HarmonicsVariableLoad/IEEE_519_Mon_mpcc_1.csv` modified; they were restored
by exact path. The corpus gate is protected by its own `CorpusGuard`, but
`lane_dump` is not: the `lane_diff` run reported "restoring overwritten" on three
tracked files (`Test/LineConstantsCode.DSS` and the two
`IEEE_519_Mon_mpcc_1.csv`) and deleted 24 `Test/AutoTrans/*.txt` plus the
`modes/autoadd` outputs — and, worth recording, its hygiene pass correctly
recognised `ledger.json` and `population.lock.json` as **pre-dirty** and left them
alone, so the 2026-08-02 blind-`git restore` accident did not recur. Two
consequences taken: the entry witness pin **rebuilds** the IEEE_519 control block
instead of compiling the deck (the three copies are byte-identical there, the same
two lines at `:45-46`), and `TESTING.md` §Procedures now says in one sentence that
a hand probe of such a deck runs on a copy.

**Corrections this sub-step forced on its own earlier text.** (i) The corpus
carries **eight** SwtControl decks, not five: the first sweep used
`--include=*.dss` and missed the uppercase `IEEE_519.DSS` copies, which carry 6 of
the 19 out-of-scope cells (2 controls × 2 properties × 3 copies). (ii) The
prediction that the homogeneous cells would "fold into the existing `ArrayForm`
rows" had the right conclusion and the wrong mechanism: the replay reads the
**frozen** `rust` column, so no live render can move a bucket there
(`CLAIMED_ARRAY_FORM` stayed 203), and on the live r4133 channel the two renders
are byte-equal, so folding is never reached. (iii) The plan's part (b) is not
render-only — one frozen snapshot produces two symptoms (the three-token render
*and* `Sample` reading past a 1-conductor element), which is why the frozen census
cell reads `[closed, open, open, ]`; both fall to the one refresh.

**Gate.** All five commands green in both lanes, each exit code read individually:
**4 252 tests per lane** (0 failed; the five `ignored` are pre-existing manual
golden-generator and doctest markers, none in a file this branch touches), corpus
gate **523/523** in both lanes (136.2 s / 135.7 s) with the ledger at **45 entries
/ 1504 hits**, every entry hit and none stale. Both predicted reds closed — the
five capi SwtControl cases on the new entries, and the pre-existing
`props_r4133_pins` red on the two-line pin move — and `props_r4133_replay` 132,
`props_r4133_pins` 40, `props_roundtrip` 1, `golden_lock` 4, `golden_schema` 104,
`population_lock` 1, `oracle_parity_cfg_gate` 10. `lane_diff` was re-run because
the drive shape changed (`SetSwitchClosed` → per-conductor
`SetConductorsClosed`): **PASS**, both dumps rebuilt (216 MB each), 523 cases /
3 220 861 records / ~4.83 M compared values, and **`max |Δ| = 0.000e0` and
`max rel = 0.000e0` on every gated kind** — conv 2 162, cur 1 170 100, errs 519,
iter 2 162, loss 366 476, pow 1 170 100, v 375 816, y 1 738 084, all reported
"(identical)", 0 iteration counts drifted — so the 2026-07-31 baseline holds
exactly and the default lane keeps precisely the parity lane's oracle standing.
No solved-state byte moved in either lane.

**RP3.7 audit settlement (2026-09-02) — 11 findings, all minor, all settled;
`FIX` in both lanes.** Two independent audits (audit-code, audit-tests) raised 12
raw findings; deduped to **11** (no overlap — three of them are different asks on
one subject, the retained 0.14.5 `Sample` glue). **Nine fixed, two fixed with a
sub-claim refuted, none dropped.** Every claim was re-derived here: the r4133
source read line-by-line, four probes replayed on the vendored EPRI DLL
(11.0.0.1), three mutations run and restored, and the pre-commit test counts
re-measured off `82022dab^`.

*Code fixes.* **(1) Relay's `ControlledElement = NIL → '[]'` render is ported.**
r4133 puts that guard in the getters and nowhere else (`Relay.pas:1407`/`:1418`);
re-measured on the DLL, a relay with `switchedobj=line.nosuch` answers `'[]'` for
both properties where the port printed `[closed, closed, closed, ]`. The port
carries it in a render-only `Relay::render_size()` used by `array_size` +
`get_enum_array`, leaving the twelve sensing/reset/`MakeLike` call sites on
`state_size()` — which is what `ORPHANED_GAPS.md` §1.14(c) was really deferring,
and what it keeps (r4133's `Reset` restores nothing with a nil element, `:1447`;
`Sample` faults outright, `:1071`, so there is no observable to port). Pinned by
`relay::tests::nil_controlled_element_renders_the_empty_array`, proven
non-vacuous by mutation. One golden byte follows: the schema emitter reads the
sample object, so `Relay.State` loses its `default: ["closed","closed","closed"]`
in `schema_full_port.json` — five lines, regenerated in the producing parity lane,
predicted before it was run. §1.14 gains (d): the same guard is unported on
Recloser (`Recloser.pas:1377`/`:1388`) and Fuse (`Fuse.pas:690`/`:701`), which
belong with those classes' interpreter ports. **(2) The dead ordinal setters are
now held to their interpreters.** `SwtControl::set_enum_array` (and Relay's twin,
which the audit's framing had as deleted — it is not) is a production-dead second
implementation of the lock guard, the five-slot cap and the `Keep` rule, whose doc
claimed the two "can never drift" while its test asserted literals only: the
auditor's mutation of the interpreter's lock guard left it green with the two
paths disagreeing. Both tests are now **differential** — each row drives two
identical controls, one through the ordinal setter and one through the
interpreter, and asserts the r4133 bytes *and* that the two agree. Re-running that
mutation now reds the SwtControl test on `Normal locked=true`, and the Relay
twin — which had **no test at all** — reds on the same shape. The two docs say
what actually holds them together. **(3) The tripwire covers the locked write.**
`side_effects(NORMAL)` is `sample()`'s arming condition, and (a2) made a locked
`normal=` reach it for the first time (the scalar era refused the write). Measured
in-port: `lock=yes` then `normal=open` arms and queues **2** (the `CTRL_LOCK` push
plus an action push r4133 never makes) while the switch itself stays closed. Both
arms are now in `sample_arms_on_a_normal_write_the_retained_capi_channel`.

*Record fixes.* **(4)** The retained-glue blockers were overstated: there is
**one** channel, not two, and RP3.7 **chose not to** re-gate rather than could
not — both corrections are written into the paragraph that describes the
divergence (above, "Recorded, with both engines measured" (i)), not restated
here. **(5)** That divergence also gets the register row it lacked,
`ORPHANED_GAPS.md` §1.16, with its blocker named. **(6)**
The civanlar pin's stated mechanism was wrong: `Normal` is not untyped there —
every one of the sixteen `New` lines declares `Action=c`, so the Edit supplemental
(`SwtControl.pas:219-228`) copies the closed Present into Normal and latches
`NormalStateSet`, which is exactly why the three later `action=o` edits cannot
move it. **(7)** The unit-pin deltas did not reproduce: the before-counts are 26
and 63 (not 30 and 64), re-measured off `82022dab^` and against `--list`.
**(8)** Three artifacts the commit itself edited still named the deleted pin
`locked_ignores_normal_and_state_writes`; all three now name the rename
(`props/swtcontrol.json`'s provenance block, `DIVERGENCES.md` ×2,
`props_r4133_replay.rs`). `R4133_PROPS_PLAN.md` §RP3.7(a2) and
`docs/phase-records/phase-7-wp2.md` keep the old name deliberately: the first is
the pre-landing instruction that *asked* for the re-point, the second is a frozen
phase record of when the test existed. **(9)** `golden.lock.json`'s
`props/swtcontrol.json` row said only "captured on … 0.15.0b4" although ten of its
cells are now the r4133 DLL's bytes. Since the reason is register-derived (all
eleven capi015 artifacts share `CAPI015_REASON`), the fix is a new register,
`CAPI015_OVERLAYS`, appended per artifact, with its own stale sweep and
well-formedness invariants — the shape `R4133_FAMILIES` uses for
`props/fuse.json`'s derivation. The anchor stays `capi015` (r4133 is not this
artifact's value authority: its `Action`/`Reset`/`Enabled`/`SwitchedObj`/`Delay`
cells still hold capi-side values), and the row now says a regen must repeat the
overlay.

*Two findings whose sub-claim is refuted, with the evidence.* **(10)** "The
missing `default` is the nil-element asymmetry" is **wrong**: the emitter's
`no_default` short-circuits on the property flags — `NO_DEFAULT` on
`SwtControl.State` (pre-existing, untouched by RP3.7) and `DYNAMIC_DEFAULT` on
both `Normal`s — before the sample object is read. The proof is `Relay.Normal`
itself: its sample array is non-empty and it still has no default. What the
finding got right is that "the shape Relay already had" was only half true, since
`Relay.State` carries no flag and did have a default; both statements are
corrected in this record and in `schema_divergences.json`. **(11)** "Re-measurement
contradicts the recorded r4133 6-phase render" is **half right**: replaying the
very deck the record cites reproduces
`[closed, closed, closed, open, open, open, ]` 2/2, and the counter-reading came
from a deck without the trailing `solve` — the same session reads all-closed
before `solve` and the OOB bytes after. So the record was not a mis-measurement
but an over-claim: those three tokens are an uninitialized read with no defined
value, deck- and heap-dependent, and both the STATUS text and the two pin docs now
say so. No assertion moved — the pins gang to a known baseline first.

*Gate after the settlement.* All five commands green in both lanes, each exit code
read individually; unit level `swt_control` **51**, `relay` **70** (+2), `dss-core
--lib` **1466** (+2), workspace **4 254** per lane (0 failed, the same five
pre-existing `ignored`), the corpus gate green in both lanes (`corpus_gate` 131,
144.7 s) over an **untouched** `ledger.json` — 27 causes / 45 entries, every entry
hit, none stale — and `population.lock.json`, `golden_lock` 4 (three digests
moved: `props/swtcontrol.json`, `schema_full_port.json`,
`schema_divergences.json` — all predicted), `golden_schema` 104,
`props_r4133_replay` 132, `props_r4133_pins` 40, `props_roundtrip` 1,
`oracle_parity_cfg_gate` 10. `lane_diff` was **not** re-run: the settlement moves
no solved state — the render guard fires only on an object with no controlled
element, which no corpus deck builds, and everything else is tests, docs and
provenance text.

**RP3.8 (the five read-only text surfaces r4133 renders live) landed
2026-09-02 — `FIX` in both lanes, one engine flag, 25 files, and zero golden
bytes moved.** The five pairs RP2.3's kill ruling re-routed here
(`indmach012.pf`, `storagecontroller.kwhtotal`/`kwtotal`/`kwhactual`/
`kwactual`, 1 064 frozen cells / 772 in scope) now render the live computed
value on `?`, `Dump`, `element_properties` and — since the audit settlement
below — `Save`, in both lanes, with no `cfg`.
Every number below was measured — the r4133 bytes come from the vendored EPRI
DLL (`Version 11.0.0.1 (64-bit build)`) through `epri-worker`, the capi bytes
from the pinned dss-python 0.15.7 / dss_capi 0.14.5, and every count is read
back from a test rather than transcribed.

**The probe ran first and the kill criterion did NOT fire.** All five r4133
renders are reproducible from the port's own state, measured cell-for-cell over
9 decks / 100 solved steps + 9 golden scenarios + 2 pin decks, with zero
exceptions, no dependency on r4133's fleet-iteration order and no stale cache on
the port side. The authority arms, read line by line:
`Controls/StorageController.pas:991-994` → `GetkWhTotal`/`GetkWTotal`/
`GetkWhActual`/`GetkWActual` (declared `:136-139`, bodies `:1162-1197`, **all
four** `Format('%-.8g',…)`), whose live inputs are the *properties*
`FleetkW`/`FleetkWh` (`:188-189` over `Get_FleetkW` `:1019-1029` = `Σ
PresentkW` and `Get_FleetkWh` `:1032-1042` = `Σ kWhStored`); and
`PCElements/IndMach012.pas:1790` `Format('%.6g',[PowerFactor(Power[1])])`,
which is state variable **#21** (`:1988`) by construction. **Two source
corrections the probe forced:** `PowerFactor` lives in
`Common/Utilities.pas:1821`, **not** `Shared/mathutil.pas` (the plan text's
pointer was off by a unit — there is no `PowerFactor` there at all); and
r4133's `Var Sum` write-back (`GetkWhTotal`/`GetkWTotal` are handed the object's
own `TotalkWhCapacity`/`TotalkWCapacity`, `:81-82`/`:991-992`) is a **dead
store** — a whole-tree grep returns exactly six lines (two declarations, the two
property arms, two dead `RecalcElementData` calls `:1107-1108`) and **nothing
reads those fields**, dss_capi 0.14.5 having commented them out outright. It is
the `VSConverter.GetCurrents` hazard in miniature and is **not reproduced**: the
getters re-sum the fleet from scratch on every call, so the store feeds no
render either (measured — a bare `edit storage.sa kwhrated=9999 kwrated=2222`
moves the render to `'15999'`/`'3722'`/`'14799'` with no fleet rebuild). The
capi `''` was re-confirmed on both of its surfaces (`? name.prop` and
`Properties(p).Val` are the same `GetObjPropertyValue` path): 0.14.5 flags the
five `[SilentReadOnly, ReadByFunction]`
(`src/PCElements/IndMach012.pas:288-289` read fn `:264-267`;
`src/Controls/StorageController.pas:416-423` read fns `:309-338`) and never
assigns their `PropertyOffset`, so `DSSObjectHelper.pas:2189` exits at its
`PropertyOffset[Index] <> -1` guard (`:2203-2204`) with the string still empty —
on a solved circuit as much as an unsolved one. **The exposure list needed three
corrections**, all measured by walking every manifest case's full
`Redirect`/`Compile` closure: `Test/indmachtest/Master.DSS` and
`4wire-Delta/Kersting4wire_{Lagging,Leading}.dss` hold **no** IndMach012 (their
only hit is `UserModel=IndMach012a` on a **Generator** — a DLL name matched as a
substring); `StoCtrl_SeasonTarget/IEEE13NodecktMOD.dss` holds no
StorageController (the controller comes from include fragments of
`Run_example.dss`); and three cases the brief missed do hold one
(`controls/combo/midi_controls.dss`, `modes/makeposseq/makeposseq_ctrl.dss`,
`controls/relay/relay_generic.dss`). Final exposure: **22 StorageController +
6 IndMach012 cases**.

**The engine change is one new flag consulted at one site.**
`PropFlags::RENDERS_LIVE_RESULT` (bit 19) is carried **alongside**
`SILENT_READ_ONLY` on exactly those five PropDefs, and
`obj/props/class_props/value.rs` is the only reader of the pair
(`SILENT_READ_ONLY && !RENDERS_LIVE_RESULT → ""`). The three other
`SILENT_READ_ONLY` readers — the JSON export omission (`class_props/json.rs`),
the JSON set refusal (`json_set.rs`) and the schema `readOnly`
(`report/export/json/schema/classes.rs`) — are **byte-unmoved**, which is why no
JSON or schema golden can move by accident, and `golden_json` 117 /
`golden_schema` 104 / `golden_lock` 4 green in both lanes are the proof rather
than the argument. The `&self` getter cannot reach the solution or the Storage
arena, so the same flag does the second job: it marks the property for a refresh
at the existing choke point `Dss::refresh_vterminal_if_marked` (the
`READS_VTERMINAL` precedent, which `prop_flags.rs` had already anticipated for
exactly this sub-step), reached by all three per-read render surfaces — and,
since the settlement below, by `Save` too, which renders whole classes and so
refreshes the store once up front instead
(`Dss::refresh_render_caches_for_save`). **StorageController**
caches a `FleetAggregates` refreshed by a loop-for-loop port of the four
getters, reading four plain numbers per fleet member and writing nothing —
`fleet_aggregates_are_a_pure_read` pins that a read leaves every rendered
property of the controller and of both members identical. **IndMach012** caches
`live_pf`, refreshed on the **fresh** `refresh_iterminal` path (GOLDEN_REBASE
G2.3's choice, deliberately made rather than defaulted; for this class the two
paths coincide, because `GetTerminalCurrents` carries its own `SolutionCount`
guard). **The one real finding of the sub-step lives there:** the naive
in-place recompute on an *unstamped* `Iterminal` cache runs `CalcPFlow` and
**advances the slip-Newton** — a JSON export moved the model, and it broke a
committed golden (`json/spectrum_refs.json`, `Slip` `0.007` →
`0.006947528894572309`). r4133 has the same stateful recompute inside
`ComputeIterminal` and **keeps** the advance: reading `pf` there moves the
machine, the `VSConverter.GetCurrents` family again, so under the 2026-08-02
policy it is not reproduced. Skipping the recompute is not the fix either (it is
how both engines get the number); it now runs on a throwaway `self.clone()` and
only the resulting f64 is kept — bit-identical output, discarded state. Proven
non-vacuous: with the clone removed, `pf_is_a_pure_read` fails
(`0.02` → `0.0162762199608059`) and `golden_json` fails on `Slip`.
**Precision is full precision, not r4133's `%.6g`/`%-.8g`** — the plan's
shorthand, overridden by measurement: every other double in the port renders
through `float_to_str_ex`, emitting a Delphi width from these five alone would
put a lossy string on `Dump`/`Save`/export where every sibling is exact, and the
r4133 channel absorbs the digit difference through RP2.4's measured
`R4133_DISPLAY_FLOOR = 2e-4` + `display_is_render` (each r4133 byte **is** the
port's number rounded). The gaps are ≈5e-10 rel on the aggregates and 4.3e-7 on
`pf`; the live census below measured the worst gated cell at **4.029e-08**, four
orders under the floor. The pins carry r4133's own bytes through `fmt_g(v,6)` /
`fmt_g(v,8)`, so a precision regression is still caught.

**The capi side, measured before it was excluded: 24 cases / 84 distinct
(case, element, property) cells / 1 006 (cell × step) comparisons**, every one a
`value_structure` divergence (a number vs `''`), identical in both lanes — 20
StorageController cases × 4 properties + 4 IndMach012 cases × `PF`; per-property
rows 250/250/250/250 + 6. Two silent holes are recorded rather than fixed:
`StoCtrl_Current_PeakShave/master.dss` holds a controller but is `kind: large`,
so the scheduler never property-compares it at all (which is why the exposure
list has 22 SC cases and only 20 red), and the three `r4133`-only cases have no
property compare until RP4.1 (*they gained one on 2026-09-03 — §RP4.1; the
`SKIP_PROPS` group (g) rows keep masking the five properties on both channels*). The disposition is a `SKIP_PROPS` row group **(g)**
— `("IndMach012","PF")` + the four `("StorageController", …)` — mirrored in
`SKIP_PROPS_CAPI_ONLY` so the two lists still **partition** `SKIP_PROPS`
(12 → 17 and 5 → 10 rows; `SKIP_PROPS_BOTH_CHANNELS` unchanged at 7), per
`(class, property)` rather than per case, cited to the 0.14.5 mechanism and to
the r4133 authority arms, with the whole argument written out in
`tests/TOLERANCE_NOTES.md` §"WP8.5b property parity" (+34 lines): the cell is a
**structure** difference, non-comparable by construction and **never** a
tolerance question — no floor is involved and none moved. No `ledger.json` entry
was needed or written (the divergence is mechanical and case-independent; 24
entries would say one thing), and `ledger.json`, `population.lock.json` and the
frozen `tests/corpus/props_r4133/**` extracts are byte-untouched.

**Zero golden bytes moved, and that was proven by regenerating.** The 21 props
cells that would move if the artifact were a capture of *this* engine
(`props/storagecontroller.json` 16 × `''`→`'0'`, `props/indmach012.json` 5 ×
`''`→`'1'`, both r4133's own bytes) are cells of a **0.14.5 capture**, which
answers `''` forever — so a regen moves nothing, measured by importing
`tools/golden/gen_props.py` itself and re-running its `check_pin()` +
`run_scenario()` per class into a scratch directory: both class files came back
**byte-identical** to the committed artifacts. (A *full* `gen_props.py` run was
deliberately not used: it rewrites all 51 class files including
`props/{recloser,relay}.json`, whose committed values are the port's renders.)
The disposition is therefore the exclusion register `props_roundtrip.rs` already
uses for this exact shape — `LANE_SKIP_SCENARIO_PROPS` **2 → 23 rows**, with
`PROPS_CLASS_FILES` 51 / `PROPS_SCENARIOS` 322 / `PROPS_PROPERTY_CELLS` 8 343
unchanged (`compared` 8 341 → 8 320 is an asserted equality, so the green run is
the proof). **The alternative was considered and is reversible**: overlaying
r4133's `'0'`/`'1'` into the two captures (RP3.7's `CAPI015_OVERLAYS`
precedent) would keep the 21 cells compared, but `golden_lock` asserts an
overlay entry *is* a capi015 artifact and these two are ordinary `capi_v0145`
rows, so it needs a new register — a provenance-model change no plan text
authorizes — for constants (`0` on an empty fleet, `1` on an unpowered machine)
three unit pins already assert against r4133's transcripts. Nothing else moves:
no committed `Dump`/`Show`/`Save` byte holds either class (`~ PF=` appears only
in `dump3_{bare,debug}.txt` and `dump_upfc.txt`, none of which builds one), and
`Save` walks only explicitly-set properties.

**Pins 40 → 43, and the replay gains a third accounting state.**
`props_r4133_pins.rs` adds `indmach012_pf_renders_the_live_power_factor`
(r4133's `'0.909167'`/`'0.904914'` after compile, the full-precision render, and
the gate's own step-0 cell), `storagecontroller_fleet_aggregates_render_the_live_fleet`
(`12000/3000/9600/0` after compile *and* after the solve; r4133's live-edit
bytes `15999/3722/14799` — re-summed, never latched; the 7-member
`7350/1550` fleet with `fmt_g(v,8)` `'2627.3806'`→`'2627.3929'` and
`'-18.81068'`) and `the_silent_readonly_capture_cells_are_empty`, which walks
all 21 capture cells asserting `''` with a writable sibling per class so it
cannot pass vacuously — the witness that the 0.14.5 oracle really is what the
skip rows exclude, since no oracle runs inside a pin binary. Two pin numbers
came out different from the pre-implementation predictions and both are
**schedule** facts, not disagreements: the `14799` live-edit reading reproduces
only when the edit follows exactly **one** solve (r4133's own schedule; with two
the fleet has charged and the port answers `15068.9999996377`), and P0's
per-step `pf` bytes are read-sequence-dependent because the *variables* read
perturbs — so each pin takes its expected value from the schedule it pins. In
the replay, the five pairs are **superseded**, not claimed and not declared: the
frozen example rows record `rust = ''` by capture and cannot be re-frozen, so
feeding a counterfactual spelling to the policy chain would prove nothing and
writing the port's new spelling into that column would be a fabricated
measurement. `DECLARED_RP38 (181, 5, 181) → (0, 0, 0)`, its old value **moved**
into the new `SUPERSEDED_RP38 = (181, 5, 181)`, `RP38_ROUTING` became
`RP38_SUPERSEDED` (now carrying the r4133 arm, the measured live disposition and
the pin per pair) plus `RP38_CAPTURE_PIN`, and the totality assert is now
`claimed + declared + superseded == rows`, with the interception **after** the
chain so a link that ever claimed one of these rows would still be credited.
Three tripwires stand against the failure mode that matters (revert the engine
to `''` and the capi compare agrees again while the skip rows mask nothing):
`the_rp38_pairs_are_superseded_by_the_live_render` asks the **shipped** harness
for each pair (`skip_prop` true on capi, false on r4133), the pins assert
literal r4133 bytes, and `every_echo_row_pin_is_a_test_that_exists` reads the
pin columns both ways.

**The r4133 side stays compared, and that too was measured, not assumed** —
with §1.1(e)'s mask bypassed (`DSS_PROPS_CENSUS=claims`, 27 cases: every case in
the population holding either class): **105 divergent cells, 89 in scope, 103
claimed by RP2.4's display floor** (worst rel **4.029e-08**) **and 2 out of
scope**. `indmach012.pf` produces **zero** divergent cells and structurally
cannot: a power factor is bounded by 1, so r4133's `%.6g` is at most 5e-07
absolute from ours while the property compare's floor is `tol.i_abs = 1e-6` at
every tier — the cell matches before any display floor is consulted (verified
non-vacuous: `Capacitor.cuf` on the same case, gap 3.4e-3, *is* reported).
`kwhtotal` is zero too. **The 2 unclaimed cells are RP4.1's inheritance, already
root-caused** (*disposed 2026-09-03: `makeposseq_ctrl.dss` stays
`engines: capi_v0145` per coordinator decision 5, so no entry was landed and the
two cells are accounted out of scope in the §RP4.1 record's per-owner table*): on `modes:makeposseq/makeposseq_ctrl.dss` the port answers
`kWTotal` `33.3333333333333` and `kWActual` `-0.333333333333333` where r4133
says `100` / `-1` — exactly a factor of `Fnphases` — because r4133's
`TStorageObj.MakePosSequence` emits `' kWrating=%-.5g'`
(`PCElements/Storage.pas:3979-3985`) where the class's property is `kWrated`
(`:647`), so that half of its own edit is an unknown parameter and the rating is
never scaled; dss_capi 0.14.5 fixed the same path by ordinal (`:3340`/`:3349`)
and the port follows it. The case is `capi_v0145`-only, so the r4133 channel
does not gate it today — it is an upstream bug newly *observable* only because
the aggregates now render, and if its `engines` key ever changes it needs a
cited exclusion + pin.

*Gate.* All five commands green in both lanes, each exit code read individually:
**4 271 passed / 0 failed / 5 ignored per lane**, the two totals identical and
the five `ignored` the pre-existing ones (no `#[ignore]` and no name filter was
added). `corpus_gate` **131** over the full 523-case population in both lanes
(146.7 s / 134.9 s) with every ledger entry hit and none stale — the 24 predicted
capi cases all green under the new rows, **zero** r4133-channel reds —
`golden_lock` 4, `golden_schema` 104, `golden_json` 117 (so `spectrum_refs.json`
still holds `Slip = 0.007`), `golden_json_import` 107, `golden_reports` 305,
`props_roundtrip` 1, `props_r4133_evidence_lock` 11, `props_r4133_replay` 132,
`props_r4133_pins` 43, `oracle_parity_cfg_gate` 10, `dss-core --lib` 1 480
(+14). No fmt fix, no clippy fix, no test edit, no count lock moved, no
tolerance touched, no golden re-baselined — the tree that arrived is the tree
that passed. `lane_diff` was re-run because the render path now refreshes caches
and clones an element: **PASS**, 523 cases / 3 220 861 records / ~4.83 M compared
values, **`max |Δ| = 0.000e0` and `max rel = 0.000e0` on all eight gated kinds**
(conv 2 162, cur 1 170 100, errs 519, iter 2 162, loss 366 476, pow 1 170 100,
v 375 816, y 1 738 084, every one "(identical)"), 0 iteration counts drifted — so
the 2026-07-31 bit-identical baseline holds exactly and the default lane keeps
the parity lane's oracle standing. One hidden red was found and fixed on the
way: `oracle_parity_cfg_gate::teardown_markers_and_the_register_agree` walks the
**whole repository** and reserves the WP-G2 teardown marker spellings for rows
of that register (decrements of the `SPLIT_ALIAS_POPULATION` /
`Escape::WholeCase` censuses), which RP3.8 does not produce, so its nine markers
were re-spelled `RP3.8 EXPECTED-VALUE PIN [row]` / `RP3.8 LANE EXCLUSION [row]`
(the settlement below gave that loose vocabulary its own enforcement, so it can
no longer shadow the register); the same walk's `SKIP_DIRS` gained `tmp` (the
gitignored scratch root — a throwaway `.rs` there could red the mandatory gate,
and one did), non-vacuous both ways and, since the settlement, **anchored at the
repository root** exactly like `.gitignore`'s `/tmp`, so a future
`crates/…/src/tmp/` cannot leave the walk.

**Recorded, not chased.** (a) `batchedit '… where <prop> > x'`
(`exec/batchedit.rs:259-270`) is a `&self` `get_value` reader, so it sees the
render cache instead of the live value; unexercised by any golden and by all 523
cases (re-confirmed by the census), and widening the refresh needs `&mut self`
plumbing outside this sub-step → `ORPHANED_GAPS.md` §1.17. (The inventory this
item stood on was **wrong by one** — `Save` is a fifth reader, and the audit
settlement below fixes it; §1.17 now records five readers, four refreshed.) (b)
`Dss::element_variables` still perturbs: state variable #21 runs
`terminal_power` on `self`, so reading an IndMach012's variables on an unstamped
cache advances the slip-Newton — pre-existing, mirroring r4133's `Get_Variable`
→ `Get_Power`, and it is what produced the P0 probe's own `0.908755` trajectory
→ `ORPHANED_GAPS.md` §1.18. (c) r4133's `? pf` keeps that advance where the port
does not; under the corpus gate's schedule this is invisible (every property
read follows a solve, both caches stamped, neither engine recomputes — proven by
probe), and it becomes visible only for a schedule that reads a property
**before** a solve and then solves. If RP4.1 introduces a pre-solve property
compare on an IndMach012 case, that is a cited exclusion + pin, never a "fix"
that reproduces the mutation. (d) `refresh_vterminal_if_marked` now does four
jobs; its doc enumerates all four and it was deliberately not renamed (three
call sites, cosmetic). (e) The golden disposition above is reversible, and the
new capture pin must move with it if a later WP overlays the two artifacts.
**Two upstream reports were written** (English, in the gitignored
`investigations/to_opendss/`, both re-measured first-hand on the DLL):
**48** — `? IndMach012.<n>.PF` before `NodeRef` is assigned **access-violates**
inside `GetCurrents` (`Get_Power` guards only on `FEnabled`,
`CktElement.pas:679`; DSS error #641, "Read of address 0000000000000000",
reproducible in a fresh process on all five golden scenarios, with three
siblings reaching `ComputeIterminal` the same way); the port renders `1` there,
safely. **49** — the `MakePosSequence` `kWrating=`/`kWrated` bug above
(`DSS error #560: Unknown parameter "kWrating"`). A landed pin doc had claimed
the port answers `1` after a solve on an *unenergized* bus; re-measured, that is
false — both engines' machine state goes NaN there (r4133 `? slip` = `'NAN'`,
the port's terminal powers NaN) and the port renders `----`, `float_to_str_ex`'s
NaN spelling. That is a render convention over identical state, not a divergence
and not a pinned value; the doc now says so.

**RP3.8 audit settlement (2026-09-02) — 9 raw findings → 8 after dedup (the
escaped-`\n` artifact was raised by both lenses); one major, seven fixed, one
recorded, none dropped; `FIX` in both lanes.** Every claim was re-derived here
before it was acted on: both probe legs replayed on the vendored EPRI DLL
(11.0.0.1) through `epri-worker`, four mutations run and restored, and the
r4133 bytes measured rather than transcribed.

*The major one — `Save` is a **fifth** `ClassProps::get_value` reader, and it
rendered a cache nobody refreshed.* The sub-step's own inventory said four
(`?`, `Dump`, `element_properties` refreshing at the choke point;
`batchedit … where` not). `Save` (`report/save/save.rs:38`, reached from
`exec/save_circuit.rs` and `exec/report.rs::write_class_file`) is the fifth, and
it emits every property a deck explicitly **set** — and a write to one of these
read-only properties is silently ignored *yet still marks the property set*, so
the serializer really did reach the render caches. Measured, same decks, port vs
r4133: a deck writing `pf=0.5` saved `PF=1` here and `pf=0.886059` upstream —
and `PF=0.886059022116548` here if a `?` happened to precede the save, i.e. an
output that depended on the session's **read history**, which is the exact
contamination shape the corpus gate's three-run artifact exists to forbid; a
deck writing `kWhTotal=42` saved `kWhTotal=0` here and `kWhTotal=6000` upstream.
The same latency was then found on the older `READS_VTERMINAL` marker, where it
**pre-dates RP3.8**: a deck writing `wdgcurrents=` saved an all-zero buffer here
while r4133 saves the solved currents. Fixed once, uniformly:
`Dss::refresh_render_caches_for_save` runs the existing per-object choke point
over the store before the serializer renders (`Save circuit`) or over one class
(`Save <class>`), so all four of its jobs happen and nothing but a marked
property's cache moves. Pinned by
`exec::tests::report::save_renders_the_live_result_properties` — five legs
carrying r4133's own bytes, including the cold/warm equality that is the
read-history regression's tripwire — proven non-vacuous by reverting each call
site. **No committed byte moved:** no corpus deck and no golden writes any of the
six properties (`wdgcurrents *=` has zero hits in the whole corpus), and
`save_roundtrip` / `golden_reports` / `golden_json` are green. `ORPHANED_GAPS.md`
§1.17 now records five readers, four refreshed, with the `batchedit` filter as
the one that stays.

*The other six fixes.* **(2) The flag's holder set is now tied to the dispatch.**
`refresh_live_result_cache` is a hardcoded two-arm `if`, so a third class given
`RENDERS_LIVE_RESULT` would have rendered a stale cache silently;
`exec::tests::report::renders_live_result_holders_have_a_refresh_arm` walks the
registry the executive actually builds and pins the holder set, and the function
now ends in a `debug_assert!` that fires on a holder reaching no arm — mutation:
giving `Transformer.WdgCurrents` the flag reds both. **(3) The two "ignored by a
JSON load" tests now test the flag they cite.** Both passed with
`SILENT_READ_ONLY` dropped, because these five have no `set_f64` arm and
"nothing was stored" holds either way; each now also asserts the property is not
**stamped** into `PrpSequence` (which is what `edit_property` would do without
the flag — and a stamped property is one `Save` emits, the same finding as the
major one seen from the load side). Mutation: dropping the flag now reds both.
**(4) The sub-step marker vocabulary is enforced.**
`oracle_parity_cfg_gate::substep_markers_are_tagged_and_do_not_shadow_the_register`
requires every loose marker to carry its owning `RP<n>.<n>` tag, forbids it from
naming a row that IS in `TORN_DOWN_ROWS` (the copy-paste hazard: such a row must
wear the reserved spelling the register cross-checks), and requires a pin marker
to sit above a real `#[test]` — non-vacuous on today's nine, and mutation-checked
both ways. **(5) The walk's `tmp` skip is anchored at the repository root**, like
`.gitignore`'s `/tmp`, instead of matching any directory named `tmp` at any
depth: the old form rested on a hand-verified premise and would have let a future
`crates/…/src/tmp/` leave the whole-repository walk. **(6) `write_gate_dump`'s
justification is corrected.** Its claim that the strip "costs the artifact
nothing" is false on the r4133 channel, where the gate DOES value-compare the
(e)/(f)/(g) rows the channel-blind `skip_prop_ub` nulls; the doc now states the
loss precisely — a real contamination still fails the artifact through the
`verdict` string, what is lost is the narrower within-tolerance drift signal for
those eight oracle-side renders — and names the channel-aware strip as the
alternative. **(7) Two record defects:** the plan's "Six points" listed seven,
and the pin doc understated its own literal — re-measured, r4133 read **without**
its perturbing pre-read answers `'0.908916'` on that deck, exactly
`fmt_g(0.908915557341299, 6)`, so the step-0 cell is r4133-**corroborated**; only
r4133's own mutation separates the two schedules. (The escaped `\n` in
`props_norm.rs`'s assert message — the finding both lenses raised — is gone.)

*Recorded, not fixed.* **`SKIP_PROPS` has no liveness guard** (pre-existing, all
17 rows): the disposition tests prove every row is decided and that the two
channel lists partition it, but a row that has stopped masking anything is
accepted silently — and for group (g) the failure mode is the engine reverting to
`''`, after which the capi compare AGREES. What covers it today is out of
harness: the expected-value pins and the r4133 channel, each of which reds on
exactly that revert (re-verified by mutation). A per-row mask counter threaded
through the corpus gate would close it in-harness; that is harness work no
sub-step owns, and it is recorded at the `SKIP_PROPS` declaration itself.

*Gate.* All five commands green in both lanes, each exit code read individually:
**4 274 passed / 0 failed / 5 ignored per lane**, the two totals identical and
the five `ignored` the same pre-existing ones (+3 on the sub-step's 4 271 — the
two new engine-side tests and the marker check; no `#[ignore]`, no name filter).
`corpus_gate` **131** over the full 523-case population in both lanes
(138.9 s / 148.4 s) with every ledger entry hit and none stale;
`oracle_parity_cfg_gate` **11** (+1), `dss-core --lib` **1 482** (+2), and
`golden_lock` 4 / `golden_schema` 104 / `golden_json` 117 / `golden_reports` 305
/ `props_roundtrip` 1 / `props_r4133_pins` 43 / `props_r4133_replay` 132 /
`props_r4133_evidence_lock` 11 all unchanged — no golden re-baselined, no
tolerance touched, no count lock moved. `lane_diff` was re-run because the fix
adds a refresh pass on a new surface: **PASS**, 523 cases / 3 220 861 records,
**`max |Δ| = 0.000e0` and `max rel = 0.000e0` on all eight gated kinds** (conv
2 162, cur 1 170 100, errs 519, iter 2 162, loss 366 476, pow 1 170 100, v
375 816, y 1 738 084, every one "(identical)"), 0 iteration counts drifted — the
bit-identical baseline is exactly where RP3.8 left it.

**RP3.9 (the r4133 round-trip residue) landed 2026-09-02 — `PRECISION_ROUNDTRIP`
on all 27 pairs, in one commit, with zero product-crate lines, zero golden bytes
and zero census cells moved.** RP2.4's display floor
(`harness/props_norm.rs`, `R4133_DISPLAY_FLOOR = 2e-4`, `display_is_render`)
refuses **55 vendored spellings / 27 pairs / 70 live cells** because the two
engines hold different doubles rather than one being a rounded render of the
other. Every one of the 27 pairs is now settled by a cited Pascal round-trip
chain and held by an expected-value pin; **none** is a `PORT_BUG`, an
`UPSTREAM_BUG`, a `STATE_DIFFERS` or a `KILL`, so no engine line, no
`TODO(compat)`, no `ledger.json` row and no upstream report is owed by the
sub-step. The sub-step's own commit is three test/evidence files
(`props_r4133_pins.rs` +1189/−1, `props_r4133_replay.rs` +517/−23,
`tests/corpus/props_r4133/README.md` +64), nothing under any crate's `src/`;
the audit settlement below touched the same three and this record, again with
no product-crate line.

**One mechanism explains all 70 cells, and it was measured before it was
written.** Every cell sits on one of the six `modes:makeposseq/makeposseq_*.dss`
decks and is read **after** `MakePosSequence`, where r4133 converts an element
by building a command string (`Format('%-.5g'/'%-.8g', …)`) and re-parsing it
through its own `Edit`, while the port applies typed setters
(`class_props/typed.rs`, WPG.21). The five-digit round trip therefore happens
*upstream* of the getter, and the getter then derives at full precision —
`Load.pas:2326 → :2331-2332 → :1145 → :2352` (`kW/3` and `kvar/3` re-parsed,
then `kVA` recomputed from the rounded **pair**); `Vsource.pas:1397 → :360 →
:473`, where the rounded input is **`BasekV`**, not Z (`R1`/`X1` re-parse
exactly on all six decks), then `:752`/`:766-768`/`:837-842` for
`puz*`/`mvasc*`/`isc3`; `Line.pas:1585-1591 → :1597-1598 → :611 → :1406/:1407`
(`b0`/`b1` from a round-tripped `C`); `Reactor.pas:1158-1162 → :1200-1201 →
:657`/`:666`/`:670-675` → `:1092-1100`; `Capacitor.pas:798-832 → :606/:645-646 →
:1098-1109` (with `Common/Utilities.pas:2600-2607`);
`generator.pas:3054-3066 → :641/:699/:734 → :3018-3021/:3130-3137`;
`Transformer.pas:1982-1994 → :512 → :1119-1130 → :1842-1843`;
`AutoTrans.pas:2021/:2027/:2030 → :1863 → :1662-1690`. The rawest reading is
`load.kw`: the port answers `(400/3)/3` where r4133 answers `44.443` =
`%-.5g(%-.5g(400/3)/3)` — `133.33` re-parsed, divided again and re-rounded —
because `makeposseq_pc.dss` runs `makeposseq` **twice**, which is why no single
`%.Ng` render explains the gap and why the floor was right to refuse it.

**The per-pair verdicts** (`props_r4133_replay.rs`: `RP39_ROUTING` :294 rows,
`RP39_PINS` :4610 verdicts; the table is machine-extracted from those two
consts, not transcribed). Chains: **A** `load.*`, **B** `vsource.*`, **C**
`line.b*` + `autotrans.wdgcurrents`, **D** `reactor.*`/`capacitor.*`, **E**
`generator.*`/`transformer.*`. All 27 verdicts are `PRECISION_ROUNDTRIP`, all
27 dispositions `PIN`.

| pair | rows | in scope | pin (chain) |
|---|---|---|---|
| `load.kva` | 13 | 0 | `load_kva_after_makeposseq_is_the_exact_typed_conversion` (A) |
| `load.kw` | 2 | 0 | `load_kw_kvar_and_xfkva_after_makeposseq_are_the_exact_typed_conversion` (A) |
| `load.kvar` | 1 | 0 | `load_kw_kvar_and_xfkva_…` (A) |
| `load.xfkva` | 1 | 0 | `load_kw_kvar_and_xfkva_…` (A) |
| `vsource.puz0` | 4 | 0 | `vsource_isc3_and_puz_after_makeposseq_use_the_full_precision_basekv` (B) |
| `vsource.puz1` | 4 | 0 | `vsource_isc3_and_puz_…` (B) |
| `vsource.puz2` | 4 | 0 | `vsource_isc3_and_puz_…` (B) |
| `vsource.isc3` | 3 | 3 | `vsource_isc3_and_puz_…` (B) |
| `vsource.mvasc1` | 2 | 2 | `vsource_mvasc1_and_mvasc3_after_makeposseq_use_the_full_precision_basekv` (B) |
| `vsource.mvasc3` | 2 | 2 | `vsource_mvasc1_and_mvasc3_…` (B) |
| `line.b1` | 2 | 2 | `line_b1_and_b0_after_makeposseq_use_the_full_precision_c1` (C) |
| `line.b0` | 2 | 2 | `line_b1_and_b0_…` (C) |
| `autotrans.wdgcurrents` | 1 | 0 | `autotrans_wdgcurrents_after_makeposseq_solve_the_exactly_converted_circuit` (C) |
| `reactor.normamps` | 1 | 0 | `reactor_amps_after_makeposseq_are_the_exact_typed_conversion` (D) |
| `reactor.emergamps` | 1 | 0 | `reactor_amps_…` (D) |
| `reactor.lmh` | 1 | 1 | `reactor_amps_…` (D) |
| `reactor.x` | 1 | 1 | `reactor_amps_…` (D) |
| `reactor.z` | 1 | 1 | `reactor_amps_…` (D) |
| `capacitor.cuf` | 1 | 0 | `capacitor_cuf_and_amps_after_makeposseq_are_the_exact_typed_conversion` (D) |
| `capacitor.normamps` | 1 | 0 | `capacitor_cuf_and_amps_…` (D) |
| `capacitor.emergamps` | 1 | 0 | `capacitor_cuf_and_amps_…` (D) |
| `generator.kva` | 1 | 1 | `generator_ratings_after_makeposseq_are_the_exact_typed_conversion` (E) |
| `generator.kvar` | 1 | 0 | `generator_ratings_…` (E) |
| `generator.maxkvar` | 1 | 1 | `generator_ratings_…` (E) |
| `generator.minkvar` | 1 | 1 | `generator_ratings_…` (E) |
| `transformer.normamps` | 1 | 1 | `transformer_amps_after_makeposseq_are_the_exact_typed_conversion` (E) |
| `transformer.emergamps` | 1 | 1 | `transformer_amps_…` (E) |

**Ten pins (`props_r4133_pins.rs:2193-3402`) cover all 27 pairs, and each
asserts the chain rather than narrating it:** (a) the port's literal render off
`? Class.Name.Prop`; (b) r4133's census literal **recomputed from the port's own
number by the chain's arithmetic inside the test**; (c) a discriminating second
reading — for chains D/E the port is fed r4133's own five-digit token through
`edit` and then prints r4133's literal itself, after which a third reading
restores the exact nameplate and moves the cells back, with `? kv`/`? kW`/
`? kVs` read each time so the edit cannot pass vacuously; (d)
`Version8/Source/*.pas:` cites in the doc comment. The AutoTrans pin hard-codes
none of r4133's command strings: it **computes** every `%-.5g` token from the
port's own post-conversion doubles, replays them as `edit`s, solves twice, and
reproduces r4133's census byte `44.00054, (161.26), 29.32187, (161.26), `
exactly, with the AutoTrans-only replay (`44.00086, …`) asserted as the
discriminator that the chain is deck-wide. Seven local helpers were added —
chiefly `round_g`/`text_g` (FPC `Format('%-.Ng')` as a value and as text, both
through the engine's own `dss_core::util::fmt_g`, the F-FMT seam, which is
literally what `Parser.CmdString := S; Edit` does) and `render`
(= `float_to_str_ex`, the function the `?` getter itself calls).

**A lane trap was found and closed rather than papered over.** The parity lane's
`fmt_g_fpc_impl` and the default lane's `fmt_g_native_impl` spell the *same
stored double* differently on `Load.ld_wye.kW`: `44.4444444444445` vs
`44.4444444444444`. The state is `(400/3)/3 = 44.444444444444446`, which is
**not** `400/9 = 44.44444444444444`, and the FPC kernel distinguishes them at 15
digits where the native one does not. The pin asserts that byte as a **value**
(`render(400.0/3.0/3.0)`) plus a second assertion naming both lane spellings —
**no `cfg`, no tolerance, no `#[ignore]`**; every other repeating-decimal
literal in chains D/E was then checked in both lanes and none differs.

**The accounting shrinks where a measurement allows it and nowhere else.**
`RP39_ROUTING` (`props_r4133_replay.rs:294`) gained a fifth column, the per-pair
**disposition** (all 27 = `PIN`), against the documented set
`RP39_DISPOSITIONS = ["PIN", "FIX", "LEDGER", "OPEN"]` (:703 — `KILL`
deliberately has **no** tag: a killed pair leaves the sub-step and is reported,
not recorded); `RP39_PINS` (:4610) carries 27 rows of `(pair, pin, verdict)`,
all `PRECISION_ROUNDTRIP`, read by `every_echo_row_pin_is_a_test_that_exists`
as a fourth cited set; `the_rp39_pin_list_is_pinned` (:4765) now checks
completeness **both** ways (every pin's pair is `PIN`-disposed, every
`PIN`-disposed pair names a pin, any other disposition names none); and
`the_display_floors_round_trip_residue_is_owned_by_rp39` (:8595) additionally
rejects an unknown disposition tag by name. The new `OPEN_RP39` (:683) is the
shrink: **`(55, 27, 19) → (0, 0, 0)`**. **`DECLARED_RP39` stays `(55, 27, 19)`
on purpose:** it is not a declaration but a *measurement* — `account()` walks
the vendored corpus and buckets whatever `display_class_but_not_a_render` routes
to `Owner::Rp39`, a property of the two engines' doubles. No port render changed
(all 27 verdicts are "the port is exact"), so writing a smaller number would
either fail the guard's own equality or force it to be loosened — exactly the
silent-progress claim the accounting exists to prevent. The rows retire by hand
at RP4.1, as `DECLARED_RP3`'s and `DECLARED_RP35`'s notes already spell out for
the earlier settled sub-steps.

**The census was re-measured after the work, and every bucket is unchanged**
(full walk, both channels, no `DSS_GATE_ONLY`: 440 cases / 1 059 178 rows /
63.7 s; 6 r4133 + 22 capi oracle errors, as before): r4133 `UNCLAIMED`
534 cells / 30 in scope / 292 spellings / 49 pairs, `under-floor`
49 484 / 46 627 / 2 045 / 71, `echo-row` 488 019 / 468 046 / 170 / 82,
`ledger-hit` 0, `mixed_disposition_spellings` 0; capi `ledger-hit` 21 (9 / 15 /
12), `UNCLAIMED` 177 (141 / 29 / 11), **0 cells on every r4133 disposition** —
every figure equal to the pre-work run, delta **0** in every bucket. All 27
RP3.9 pairs appear in `claims_unclaimed_pairs.txt` with `cells_in_scope = 0` and
cell/spelling counts equal line for line. A zero delta is the correct outcome —
a pin does not make a `Link` claim a row — and a non-zero one would have meant a
pin moved a render. **No `property` ledger entry is staged**, because with
`count_in_scope = 0` on all 70 cells an r4133 entry would have nothing to
exclude; the drafts, should a deck's `engines` key ever change, are staged in
`tests/corpus/props_r4133/README.md` (the audit settlement moved them there
out of the gitignored dossiers, where the forward reference would have dangled). One structural note: the census has **no disposition
meaning "settled by an RP3.9 pin"** — the 70 cells still file as `UNCLAIMED`.
That costs nothing today and is RP4.1's to decide, not this sub-step's to
invent. *Decided 2026-09-03 (RP4.1):* no census disposition was added —
acceptance is read as zero UNCLAIMED cells *in scope*, and these 70 cells are
accounted out of scope, owner by owner, in the §RP4.1 record below.

*Gate.* All five commands green in both lanes, each exit code read individually:
**4 285 passed / 0 failed / 5 ignored per lane** over 74 test binaries, the two
totals identical binary for binary and the five `ignored` the same pre-existing
ones (+11 on RP3.8's settled 4 274; no `#[ignore]`, no name filter).
`corpus_gate` **131** over the full 523-case population in both lanes
(142.9 s / 139.7 s) with every ledger entry hit and none stale, zero
NEVER-APPLIED entries and zero reds on either channel; `props_r4133_pins`
43 → **53**, `props_r4133_replay` 132 → **133**, and `props_r4133_evidence_lock`
11 / `oracle_parity_cfg_gate` 11 / `dss-core --lib` 1 482 / `golden_lock` 4 /
`golden_schema` 104 / `golden_json` 117 / `golden_reports` 305 /
`props_roundtrip` 1 all unchanged — no golden re-baselined, no tolerance
consulted or moved, no count lock moved other than the new `OPEN_RP39`, no
`ledger.json` edit and no `TODO(compat)` added.
`tests/corpus/props_r4133/README.md` gained a dated supplement
`### What RP3.9 settled (2026-09-02)` (:871, before `## Files`, on RP3.8's
precedent) and **no recorded number in any earlier section was rewritten**.
`lane_diff` was **not** re-run and does not need to be: the sub-step touched
three test/evidence files and not one line under any crate's `src/`, so no
engine path, `compat` kernel, lane alias or solver moved and the 2026-07-31
`max |Δ| = 0` bit-identical baseline — reproduced by RP3.8 the same day —
stands untouched.

**Recorded, not chased.** (a) r4133's `TStorageObj.MakePosSequence` emits
`' kWrating=%-.5g'` where its own property is `kWrated`
(`PCElements/Storage.pas:3979-3985` vs `:647`), so half of its own edit is DSS
error #560 on `makeposseq_pc.dss` — already reported as
`investigations/to_opendss/49` by RP3.8 and re-confirmed here; the case is
`capi_v0145`-only, so the r4133 channel does not gate it. (b) The **34**
`controls:autotrans/*` `wdgcurrents` cells (`autotrans_both`, `autotrans_reg`,
`midi_autotrans`, `midi_autotrans_both`; 3–7.5 % apart, e.g. `151.5029` vs
`156.2997` A on winding 2) involve **no** `makeposseq` and are a different
mechanism from RP3.9's single tiny `makeposseq_xfmr` cell — out of scope
(capi-only cases), filed `OutOfScope` by the RP2.4 re-filing, and still owed
their own root cause; **nobody owns them yet** and a multi-percent gap in a
solved current is not display-class. The audit round added a lead: those 34
cells are exactly the row count of `numeric_pairs.txt`'s
`autotrans.tap | '1.03125' | '1' | 34` and
`autotrans.taps | '[1, 1.03125, ]' | '[1, 1, ]' | 34`, i.e. the same four
regulator decks, and the divergence is confined to the series/common winding
while winding 1 agrees to 0.04 % — so a **RegControl tap** divergence is the
first thing its owner should read. It is a lead and not a settled cause: the
per-row current ratios are 1.032 / 1.056 / 1.069 / 1.082, not one uniform
1.03125. **Owned and settled 2026-09-03 by §RP3.12** (record below): the lead
was right — the cause is an r4133 `RegControl`-to-`TTransfObj` typecast,
`UPSTREAM_BUG`, never reproduced. (c) The post-`makeposseq` `Save`/`Dump`
surface belongs to **§RP3.11** (*settled 2026-09-03 — `KEEP_LIVE_PINNED`, so
the port keeps saving the live doubles; this is one instance of the recorded,
pinned divergence from r4133's serializer, and no channel compares it*): r4133
saves the five-digit tokens its getter hands back for those indices while the
port saves the exact doubles, so a saved-and-
reloaded converted circuit differs at ~5e-6 on exactly these elements. (d) The
`RP39_ROUTING` `cite` column was left byte-identical here and **corrected in
the audit settlement below** — its vsource rows named a round-tripped `Z`, its
transformer rows a round-tripped `kVA` and its `load.kva` row a round-tripped
`pf`, none of which is the rounded input this sub-step proved. (e)
`elements/pc/vsource/solve.rs:173` and `mod.rs:44-45` cite dss_capi 0.14.5 line
numbers rather than r4133's `Vsource.pas:1390` — product-doc cosmetics,
unacted.

**RP3.9 audit settlement (2026-09-03).** Two fresh auditors (`/audit-code`,
`/audit-tests`, both read-only, over `16edbc2f..648ce284`) confirmed all 27
verdicts independently on both live oracles — every r4133 literal is what the
r4133 DLL prints, every port literal what the pinned 0.14.5 backend prints, and
every chain reproduces arithmetically — and found **no port bug, no weakened
test and nothing lost against the plan**. Their fifteen findings (nine + six,
four of them the same item seen from both sides) were settled against the r4133
source and re-derivation, never against plausibility:

* **FIXED — 11 wrong r4133 line citations** (the one `[Major]`). The formulas
  named in the chain-C and chain-D pin docs are genuinely in r4133; the line
  numbers were not. Verified by dumping the r4133 tree: the `R1=… C1=%-.5g`
  `Format` is `Line.pas:1591` (`:1593` is a comment), `Parser.CmdString := S;
  Edit` `:1597-1598`, `c1 := Parser.Dblvalue*1.0e-9` `:611`, the symcomponents
  `C1_new := C1*1.0e9` branch `:1560-1563`; `Reactor.pas` `kvarPerPhase` `:657`,
  1-phase `PhasekV := kVRating` `:666`, `X` `:670`, `L` `:671`, `NormAmps`
  `:674`, `EmergAmps` `:675`, `CmdString`/`Edit` `:1200-1201`. Corrected in the
  pin docs and inline comments, in this record's mechanism paragraph and in the
  vendored README's chain table.
* **FIXED — three stale or false prose numbers.** The pin block header said
  "all thirteen pairs" (the chains-A-C draft state) where 27 landed; the
  vendored README's count-delta table gave `RP39_PINS` a "before" of "13 rows"
  for a constant that does not exist at `16edbc2f` (`git show
  16edbc2f:…/props_r4133_replay.rs | grep -c RP39_PINS` → 0), now "— (new
  constant)"; and this record's mechanism paragraph said the port answers
  `400/9` and spelled r4133's `44.443` as one `%-.5g`, contradicting its own
  lane-trap paragraph and the pin. The port's double is `(400/3)/3 =
  44.444444444444446` (≠ `400/9`), and r4133's number needs **both** `%-.5g`
  prints, which is what `props_r4133_pins.rs` asserts.
* **FIXED — `RP39_ROUTING`'s `cite` column stated causes this sub-step
  disproved.** Seven vsource rows blamed a round-tripped `Z` (the rounded input
  is `BasekV`; `R1`/`X1` re-parse exactly on all six decks, asserted in the
  pin), the two transformer rows a round-tripped `kVA` (it is the winding kV,
  `Transformer.pas:1982`), the `load.kva` row a round-tripped `pf` (it is the
  `kW`/`kvar` token pair, `:2326 → :1145`), and the four generator rows cited
  `MakePosSequence :3058`/`:3059`/`:3060` where the tokens are at
  `:3059`/`:3060`/`:3061` — and all four of those cells derive from the **kW**
  token, not from their own. All twelve rewritten; the guard only requires
  `.pas:`, so no count moved.
* **FIXED — the 16 vendored spellings that carried a verdict but no
  assertion.** 10 of the 13 `load.kva` spellings and 2 of the 4 spellings of
  each `vsource.puz0/1/2` were covered only at pair level. Both pins now assert
  every vendored spelling: `load_kva_…` gained a (deck, load) loop over the
  remaining ten rows on `makeposseq_line`/`_report`/`_shunt`/`_xfmr`/`_pc`,
  each recomputing r4133's literal from the port's own live `kW`/`kvar` through
  the same round trip (doubled on `_pc`, which converts twice), and
  `vsource_isc3_and_puz_…` gained `pc`/`source` and `shunt`/`source`. All 55
  spellings are now asserted; both lanes stay green, so no lane-dependent `%g`
  spelling hides among them.
* **FIXED — the generator pin's hardcoded `pf`.** It built the kvar chain from
  `g_plain`'s kW (whose own pf is 1.0) plus a literal `0.95`, the pf of the
  elements the cells belong to. It now reads both factors live off
  `Generator.g_kva` and asserts they agree with the kW it already had.
* **FIXED — the one port-vs-port assertion** (`props_r4133_pins.rs`, chain C):
  `fmt_g(2π·f·c1, 13) == fmt_g(b, 13)` compared two live port reads. Both sides
  are now compared against a recorded 13-digit literal per line
  (`4.086610309067` / `4.087172109558`), which is also the honest width — the
  recomputation from the 15-digit-truncated `c1` render lands 2 ulp away.
* **FIXED — the staged ledger drafts pointed at gitignored scratch.** Plan
  §1.1(e) drafts lived only in `tmp/rp39/dossier_*.md`, which the handoff itself
  lists as safe to delete. The draft entry shape (channel `r4133`, one
  `property` row per case/class/name/prop, with the per-chain head of the round
  trip) is now staged in `tests/corpus/props_r4133/README.md`, still **not** in
  `ledger.json`.
* **RECORDED, not fixed — nine port-side expectations written as
  `render(<expr>)`** (`Load.ld_wye.kW`, the reactor/capacitor/generator/
  transformer nameplates and two discriminating reads). The `?` surface renders
  15 significant digits, so the exact double cannot be recovered by parsing it
  back, and a hardcoded byte would red in one lane; every *residue* cell is a
  recorded literal, and `load.kw` — the only residue cell among the nine —
  additionally carries the two-lane `matches!` whose value assertion is
  bit-discriminating in the parity lane. The auditor reached the same reading
  ("mostly unavoidable"); strengthening it belongs to a renderer-level pin, not
  here.
* **RECORDED, not fixed — the 34 `controls:autotrans` `wdgcurrents` cells.**
  New evidence (note (b) above): they are exactly co-populated with the
  `autotrans.tap`/`taps` divergence on the same four capi-only regulator decks,
  which makes a RegControl tap divergence the lead. Still nobody's, still not
  RP3.9's — a multi-percent gap in a solved current is not display-class.
  (**Owned 2026-09-03 by §RP3.12**, record below — verdict `UPSTREAM_BUG`,
  the lead confirmed as the root cause.)
* **Confirmed as correct, no action:** `DECLARED_RP39` staying `(55, 27, 19)`
  while `OPEN_RP39` went to `(0, 0, 0)` (it is a measurement of what
  `display_class_but_not_a_render` refuses, and no port render moved — both
  auditors re-derived the same reading from `account()`); the two amps families
  recorded as `PRECISION_ROUNDTRIP` rather than the plan's shape-2
  `STATE_DIFFERS` (their state differs *because* of the same upstream round
  trip, and r4133's value is reproducible from the port's, which is outcome 1's
  own test); `RP39_DISPOSITIONS` living beside `RP3_SETTLED_SHAPES` (different
  routing tables, both pinned literally); and the chains-D/E pins mutating and
  restoring deck state (every restore is read back, and the tree is clean after
  the run).

*Settlement gate.* All five commands green in both lanes, each exit code read
individually, at the **same totals as the sub-step's own gate — 4 285 passed /
0 failed / 5 ignored per lane over 74 test binaries** (the settlement adds
assertions to existing pins, not tests): `props_r4133_pins` **53**,
`props_r4133_replay` **133**, `props_r4133_evidence_lock` **11**,
`oracle_parity_cfg_gate` **11**, `corpus_gate` green on both channels with every
ledger entry hit and none stale, no `#[ignore]` and no name filter. `lane_diff`
was again not required: the settlement touched the same three test/evidence
files plus this record, and no product crate.

**RP3.12 (the `controls:autotrans/*` `wdgcurrents` gap) landed 2026-09-03,
audit settled the same day — `UPSTREAM_BUG` in r4133, never reproduced, with
zero product-crate lines.** The
sub-step exists because of RP3.9's P0 open item (note (b) and the "RECORDED, not
fixed" bullet above): the **34** `autotrans.wdgcurrents` cells on the four
`controls:autotrans/*` regulator decks (`autotrans_both`, `autotrans_reg`,
`midi_autotrans`, `midi_autotrans_both`; 3-7.5 % apart) had no owner and no root
cause, only RP2.4's display-class `OutOfScope` filing — which a multi-percent
gap in a solved current cannot be.

**Root cause, measured on the live r4133 DLL rather than inferred.** r4133's
`RegControl` reaches its controlled element through five unchecked
`TTransfObj(ControlledElement)` casts (`RegControl.pas:926`, `:1026`, `:1296`,
`:1370`, `:1479`) although `TAutoTransObj = class(TPDElement)`
(`AutoTrans.pas:88`) is not a `TTransfObj` (`Transformer.pas:92`) and
`TAutoWinding` (`AutoTrans.pas:59`) lays its fields after `Rdcohms` at different
offsets than `TWinding` (`Transformer.pas:62`), so `Increment :=
TapIncrement[TapWinding]` (`RegControl.pas:1249`) reads the winding's
`MaxTap` = 1.1 pu and `PendingTapChange := Round(BoostNeeded / Increment) *
Increment` (`:1250`) rounds every realistic boost to zero. The control therefore
never arms — **0** r4133 event-log lines on all four decks against the port's
10-13 — so r4133's numbers are the **unregulated** circuit while the port's (and
the pinned 0.14.5 oracle's, which the port matches exactly) are the regulated
one. The defect is identical in r3723, r4088 and r4133; the DSS-Extensions fork
fixed it while refactoring and the port already carries the fixed shape
(`ControlledTransformer` virtual dispatch,
`elements/pd/transformer/mod.rs:472`), so **no engine line changes**. Upstream
report, gitignored and local-only:
`investigations/to_opendss/50-regcontrol-autotrans-ttransfobj-typecast.md`.

**Exposure: zero gated exposure today, in both lanes and on both channels.** All
four decks are `engines: "capi_v0145"`
(`tests/corpus/manifests/population.lock.json:74-77`) and no `r4133`-gated case
pairs a RegControl with an AutoTrans, which is why the gate is green with
nothing excluded. The divergence is whole-case (node voltages 2.2-2.6 %, the
assembled Y, the AutoTrans branch currents/powers, meters, monitors, the event
log, the control queue and all three manifest probes), so the right instrument
is a case-level `kind: "skip"` on the `r4133` channel, not a field exclusion.
Four such entries are **drafted only** (`tmp/rp312/staged_ledger.md`,
`cause_ref: "regcontrol-autotrans-typecast"`, citing `RegControl.pas:1026`,
`:1296`, `:1479`, `AutoTrans.pas:88` and probes E1/E2/E5): landing one today
would be stale on arrival and fail-on-stale would red the gate, so they wait for
the day a deck gains the channel.

**The pin names both numbers on both legs.** `props_r4133_pins.rs:2997`,
`autotrans_wdgcurrents_stay_regulated_where_r4133_never_taps_the_autotrans`. On
`controls/autotrans/autotrans_reg.dss` the port renders `66.95905, (-28.006),
151.5029, (151.99), ...` with `taps = [1, 1.03125, ]` and
`RegControl.rat.TapNum = 5`; the same deck with `edit RegControl.rat enabled=no`
and the tap put back reproduces r4133's census literal `66.99186, (-28.029),
156.314, (151.97), ...` byte for byte with `taps = [1, 1, ]`. On
`midi_autotrans.dss` it is `73.11371` vs `78.15456` A on the **common**
(wye, winding 2) winding, while the **series** winding agrees to 0.018 %
(`117.2108` vs `117.2323`) — that asymmetry IS the signature of a different
landed tap (`[1, 1.06875, ]` / `TapNum = 11` vs `[1, 1, ]`). Both edits are
read back before the re-solve, so "nothing moved" cannot pass on an edit that
never landed. A second, discriminating reading holds each leg to the ampere-turn
identity `|I_c|/|I_s|` vs `VBase_s*tap_s/(VBase_c*tap_c)` (rel < 1e-5), so the
pin also reds if the winding-current derivation drifts without the tap moving —
proven by a mutation probe that feeds the regulated leg tap 1.0 and reds
(2.2626 vs 2.3333).

**Count locks re-derived from the census artifacts, not edited blind.** New
`DECLARED_RP312 = (8, 1, 0)`: 8 vendored spellings
(`tests/corpus/props_r4133/examples_supplement.txt:127-134` =
`tmp/props_census/r4133/claims.txt:1035-1042`) whose cells are
9+8+3+3+3+3+3+2 = **34**, one pair, and **0** in scope on every spelling and on
the pair total. `DECLARED_OUT_OF_SCOPE` (229, 22, 0) → **(221, 21, 0)**:
229 − 8 = 221 rows, 22 − 1 = 21 pairs — the pair's only other row, the
`makeposseq_xfmr` residue, is already RP3.9's, so the pair leaves the bucket
entirely — and the third column stays 0, because a verdict does not create
scope. RP3.9's `DECLARED_RP39` (55, 27, 19), `OPEN_RP39`, `RP39_ROUTING`,
`RP39_PINS` and `RP39_SETTLED_VERDICTS` are byte-unchanged, and the RP2.4-dated
(229, 22, 0) lines further down — the §RP2.4 record's `DECLARED_OUT_OF_SCOPE`
(134, 18, 0) → (229, 22, 0) paragraph, its `OutOfScope` bucket row and its
`RP24_OUT_OF_SCOPE_ROWS` note; searched by string, not by line, because every
later record shifts them — stay as *that* sub-step's record. Both new locks are
live-enforced: a mutated `(9, 1, 0)` / `(222, 21, 0)` copy reds three walks.

**The false ledger cause is corrected, not left standing as history.**
`tests/corpus/ledger.json:25` carried `autotrans-regcontrol-tap` — "a last-ulp
voltage nudges the tap decision across a boundary ... FPC-vs-Delphi, not a port
bug", exactly the conditioning excuse CLAUDE.md forbids, unchallenged since
`9d5852bc` (2026-07-18). The key is renamed to `regcontrol-autotrans-typecast`
and rewritten with the proven mechanism (no entry ever referenced the old key,
and the gate never flags an unreferenced cause —
`corpus_gate/ledger.rs:1452-1463`), and dated `Correction (2026-09-03, RP3.12)`
notes that keep the original readings went into
`docs/upgrade/sweeps/capi015_vs_r4088.md:43`, `docs/upgrade/DIVERGENCES.md:2086`,
`docs/upgrade/known_diffs_burndown.md:70,150` and
`tests/corpus/props_r4133/README.md` §RP1.2 (:213ff).

*Gate.* All five commands green in both lanes, each exit code read individually:
**4 288 passed / 0 failed / 5 ignored per lane** over 74 test binaries (+3 on
RP3.9's 4 285 — `props_r4133_pins` 53 → **54**, `props_r4133_replay` 133 →
**135**, every other binary unmoved), the same five pre-existing `ignored`, no
`#[ignore]` and no name filter. `corpus_gate` **131** over the full 523-case
population in both lanes with every ledger entry hit and none stale, zero
NEVER-APPLIED entries and zero reds on either channel, and the four `ledger::*`
self-tests green after the rename. No golden re-baselined, no tolerance
consulted or moved, no `TODO(compat)` added. `lane_diff` was **not** owed: the
commit is two test files, `ledger.json` and four docs — not one line under any
crate's `src/` — so no engine path, `compat` kernel, lane alias or solver moved
and the 2026-07-31 `max |Δ| = 0` bit-identical baseline stands.

**Open, recorded not chased.** (a) The four staged `skip` entries had **no
tripwire** that would red if a deck gained the `r4133` channel without them (the
existing `the_staged_r4133_property_entries_have_not_landed_yet` is
`property`-scoped) — **closed the next commit** by the audit settlement below,
which landed the `skip`-scoped analogue instead of deferring it to RP4.1.
(b) Two RP2.4-dated tables in `tests/corpus/props_r4133/README.md` (:585, :823
— the RP3.12 correction at :213ff moved them from :569/:807)
still name `§1.3 (autotrans.wdgcurrents)` as the owner — left as history, their
in-scope column is 0 either way, and the dated correction at :213ff points here.
(c) The two test lanes again dropped seven untracked
`tests/corpus/electricdss-tst/Test/AutoTrans/*.txt` files — the known
overlapping-guard snapshot race recorded in §"Standing open follow-ups" as
"`CorpusGuard` can leak deck-written artifacts under concurrency" (cited by
string, not by line, so no later insert can stale it), third sighting, removed
by exact name; no tracked corpus or golden file moved.

**Audit settlement (2026-09-03, `/audit-code` + `/audit-tests`, 12 findings:
1 major, 6 minor, 5 notes — 11 distinct issues, since both auditors raised the
missing `skip` tripwire — 9 fixed, 2 recorded-not-changed, 0 refuted).** No
product crate is touched by the settlement either, so `lane_diff` stays unowed.

* **[major, tests] The pin's "discriminating second reading" read the pinned
  literal, not the engine.** `common_over_series` was handed `leg`'s `&str`
  parameter on both legs, so on the unregulated leg both operands were
  compile-time constants and the ampere-turn assertion could not fail on any
  tree — which made the doc's claim that it "fails if the winding-current
  derivation drifts, not only if the tap does" (and with it the elimination of
  `GetAllWindingCurrents` / `auto_trans/yterminal.rs` as the site) unbacked.
  **Fixed:** both legs now measure `deck.get("AutoTrans.at.WdgCurrents")` and
  predict from `deck.get("AutoTrans.at.Tap")`, so both sides are live reads.
  Non-vacuity probed: perturbing the live string (`151.5029` → `160.0`) reds
  with `2.389520 vs 2.262626`; the test stays green in both lanes otherwise.
* **[minor, code] "2.0–2.6 V outside the band" was wrong in four landed
  artifacts.** Re-measured from the two decks' own `vreg`/`band`/`ptratio` and
  both engines' node voltages: r4133 leaves `LOW.1`/166 = **118.038 V** against
  the band [119, 121] — **0.96 V** under the edge, 1.96 V under the setpoint —
  and `AT69.1`/332 = **119.251 V** against [122.25, 123.75] — **3.00 V** under
  the edge, 3.75 V under the setpoint; 2.12 % / 2.57 % are the *voltage gaps*
  against the port, not volts. The landed text was the first deck's setpoint
  deviation and the second deck's percentage, both presented as band excursions.
  **Fixed** in all four: the pin doc, the `ledger.json` cause, and the
  `DIVERGENCES.md` / `capi015_vs_r4088.md` correction blocks. It stays prose,
  and now says why: `Deck::get` reads `? Class.Name.Prop`, and neither the port
  nor r4133 gives `AutoTrans` a property that renders a bus voltage (no
  `WdgVoltages`), so asserting it would need harness machinery this sub-step has
  no call to build.
* **[minor, code] STATUS called the COMMON winding "the series winding".**
  `73.11371` vs `78.15456` A are winding 2 (`conn=w`); the series winding
  (`conn=s`) is `117.2108` vs `117.2323` and agrees to 0.018 %. That asymmetry
  IS the tap signature, and the sentence inverted it while contradicting
  STATUS's own RP2.2 record. **Fixed**, with the series figure stated beside it.
* **[minor, code] Three stale self-citations.** The record cited pre-commit
  STATUS line numbers (`:5675`, `:5745`, `:5817-5818`) that its own +121 lines
  had already shifted, and README `:569`/`:807` that its own +16-line correction
  had moved to `:585`/`:823`; and `STATUS.md:4155` still carried "no RP3
  sub-step opens while it stays out of scope", the exact sentence whose README
  twin RP3.12 had corrected. **Fixed:** the STATUS-internal citation is now by
  string (the §RP2.4 record's `DECLARED_OUT_OF_SCOPE` paragraph), which no later
  insert can stale; the README pair is corrected; and the RP2.2 line carries the
  same dated correction its README twin got.
* **[minor, code] The staged draft mis-stated the midi iteration counts.**
  `tmp/rp312/staged_ledger.md` read "iterations 9 vs 4" for `midi_autotrans`;
  the port solves `midi_autotrans` in **10** against r4133's 3 and
  `midi_autotrans_both` in **9** against 4 (the report's `6 / 6 / 10 / 9` row
  was read under the neighbouring table's order). **Fixed in the draft** — which
  is gitignored and never staged — together with the event-log counts, now given
  per deck: 10 / 12 / 10 / 13 on the port, 0 on r4133 everywhere.
* **[minor, tests] The new min-gap lock was one-sided and a round number.**
  `min_rel > 100.0 * floor` left the measurement in prose, against the rule this
  same file wrote down after a mutation walked past `RP24_OUT_OF_SCOPE_MIN_RATIO`
  in its one-sided form. **Fixed:** `RP312_MIN_GAP_RATIO = 153.0` with an upper
  bracket at 154.0 and the measurement named (`3.068975820171114e-2` against the
  2e-4 floor = **153.4x**, `examples_supplement.txt:127`; the largest is 376.9x).
  Both directions probed red.
* **[minor, tests] The existence guard's doc still named three citing tables
  while the assertion unions five.** The drift started at RP3.9. **Fixed:** the
  doc now names `RP39_PINS` and `RP312_UPSTREAM_BUG` too, with what each
  witnesses.
* **[note ×2, both auditors] No tripwire for the four staged `skip` entries.**
  Recorded at landing as an RP4.1 candidate; **fixed here instead** — the new
  `the_autotrans_typecast_cases_pair_their_r4133_channel_with_a_skip_entry`
  asserts the *pairing* both ways: each of the four cases gates r4133 **iff** it
  carries an `r4133` `kind: "skip"` entry citing `regcontrol-autotrans-typecast`
  (whose presence in `causes` is asserted too, so the rename cannot be undone
  silently). `RP312_STAGED_SKIPS` carries the four `(case, entry id)` pairs.
  Probed: pointing one row at an `engines=both` case reds with the entry to
  write.
* **[note, tests] The "r4133's census cell, byte for byte" claim was
  unenforced.** House pattern (the RP3.8/RP3.9 pins paste literals too), and
  true — but nothing tied the literal to the vendored extract. **Fixed** rather
  than recorded: `the_rp312_pin_quotes_the_vendored_census_cells` flattens the
  pin file's string continuations and counts the declared rows whose **both**
  columns appear verbatim; `RP312_WITNESSED_ROWS = 2` locks it
  (`examples_supplement.txt:128` and `:134`). Probed at 1 → red.
* **[note, code] RECORDED, not changed: the ownership predicate selects by pair
  name, not by deck.** `regcontrol_autotrans_typecast_row` cannot do better —
  the vendored `Example` row carries no case column (`pair`, `class`, `prop`,
  `rust`, `r4133`, `cells`, `src`), so no per-deck assertion is derivable from
  it. The deck mapping is anchored instead by the census artifacts, the
  `(8, 1, 0)` count lock, the `min_rel` bracket and the pin's two decks; a row
  of this pair arriving from an unrelated deck surfaces as a count mismatch.
* **[note, tests] RECORDED, not changed: the verdict declares 8 spellings, the
  pin witnesses 2** (10 of 34 cells). In spec — the brief asked for one pin with
  two legs — and the six unwitnessed spellings cannot use this pin's
  construction: their decks (the two `*_both` included) open with a snapshot
  `Solve`, so reproducing r4133's unregulated state needs the RegControl
  disabled in the deck SOURCE, i.e. a deck copy the pins harness does not have.
  It is now a *measured* residual (`RP312_WITNESSED_ROWS`) instead of prose.

*Gate (settlement).* All five commands green in both lanes — **4 290 passed /
0 failed / 5 ignored / 0 filtered out** per lane over 74 binaries, `corpus_gate`
unfiltered over the full 523-case population — with the two new guards
(`props_r4133_replay` 135 → **137**; `props_r4133_pins` stays at 54, its pin
strengthened in place). The known overlapping-guard snapshot race dropped six
untracked `Test/AutoTrans/*.txt` again (fourth sighting), removed by exact name;
no tracked corpus or golden file moved.

**RP3.11 (the `Save`/`Dump` re-serialization surface) landed 2026-09-03 —
`KEEP_LIVE_PINNED` on both surfaces: the kill criterion fires on the one cell
that decides it, so the divergence from r4133's serializer is recorded and pinned
rather than reproduced.** The sub-step exists because of the RP3.3 audit
settlement: every echo row in this plan says the same thing about a *compare*,
and RP3.3 measured for the first time what that same difference does where **no
channel compares at all**. It ran after RP4.1, as plan §0 requires, and blocks
§RP5.2 only.

**The premise the plan opened with is corrected, not inherited.** Pascal
`SaveWrite` writes `PropertyValue[iProp]` (`R4133:General/DSSObject.pas:156`),
but that is `Get_PropertyValue` → the **virtual** `GetPropertyValue` (`:45`,
`:117-120`, *"This is virtual function that may call routine"*), whose base body
returns `FPropertyValue` (`:112-115`) and which r4133 overrides in **49** units
of the live `Version8/Source` tree (53 counting `Deprecated/`;
`grep -rniE "^[[:space:]]*function[[:space:]]+T[A-Za-z0-9_]*\.GetPropertyValue"`
over `Version8/Source`, the `CMD_Lazz` duplicate tree and the base `TDSSObject`
excluded — 54 raw hits; the character class has to admit digits, `_` and a
leading indent, or `TGeneric5Obj`/`TTCC_CurveObj`/`TDynamicExpObj` drop out) —
each answering **live** on a hand-picked index set. So "the store" is not a
*meaning* of `Save` in r4133 at all. Verified first-hand on the one class the
plan quotes: `R4133:PCElements/generator.pas:3007-3038` arms 3 `kv`, 4 `kW`,
5 `pf`, 13 `kvar`, 19/20 `maxkvar`/`minkvar`, 26/27, 34/36 and 37-46 live, and
**has no arm 6** — which is the only reason it prints `model=3` beside a live
`kv`/`kW`/`maxkvar`. `Storage`'s arm list *contains* `propMODEL`
(`R4133:PCElements/Storage.pas:1525-1596`): same property, opposite treatment,
same engine. The live/store split is an artifact of which arms each class's
author happened to write, **not a rule** — which is what kills the per-index
"split" option as well as the store one. The only thing r4133's own comment
documents about `Save` is *membership*: *"Write only properties that were
explicitly set in the final order they were actually set"* (`:138-139`).

**The kill criterion fires, explicitly, and on both surfaces.** Matching r4133 on
`modes:ncim/ncim_pv_pq.dss` means emitting `model=3` while `gen_model == 4` in
the same process — printing a value the engine knows to be superseded, which the
2026-08-02 policy forbids; the compare-side exclusion for that very cell already
exists and is already pinned (`tests/harness/props_norm.rs:1815-1817`,
`generator_model_renders_the_live_pv2pq_conversion`). `Dump` shares the one
getter with `Save`, `?`, the property API and batchedit — exactly as both Pascals
route all of theirs through the one virtual getter — so the same cell decides it,
with the difference that `Dump` prints **all** properties with no `PrpSequence`
filter (`R4133:PCElements/generator.pas:2489-2500`) and therefore has all **88**
committed `dump*` artifacts (44 `.txt` + 44 `.meta.json`, 162 lines) behind the
alternative. Measured on the worked example, of the 10 differing `Dump` rows
exactly **one** is the store-vs-live axis; the other nine belong to axes that
already have owners (r4133's deleted `DumpProperties` overrides, the RP3 name
census, `fmt_g`/WP-G4 float spelling, header quoting, four already-pinned echo
rows).

**The rule the port now states positively, in one paragraph**
(`report/save/save.rs` module doc): **values** = the live field through the one
`ClassProps::get_value`; **membership + order** = the explicitly-set chain
(`prp_sequence` / `next_property_set`), including 0.14.5's property-tracking
stamps; **structure and ordering guards** = the *union* of both upstreams'
`SaveWrite` overrides, because every one of them exists to make the emitted deck
re-compile; and no branch that prints a value the engine knows to be superseded.
Four such guards were unported and landed here, all lane-unconditional, none of
them touching *which* value is printed: **P1** `BusVoltageBases.dss` now ends in
r4133's unconditional `CalcVoltageBases` (`R4133:Common/Circuit.pas:2716-2740`,
two plain `Writeln`s) instead of 0.14.5's `! CalcVoltageBases` — the comment is
that engine's compat-flag side (its own `DSS_CAPI_NOCOMPATFLAGS` branch writes it
uncommented), and RP3.11 measured what it cost: every circuit the port saved
re-compiled with `kVBase = 0` on **every** bus (`bus_kvbase(genbus)` **0.0**
against **7.199557856794634** from r4133's own save), the ncim tree came back NOT
CONVERGED at 15 iterations, and the `expcontrol` PV moved from −0.0060 kvar / 14
iterations to **+307.94 kvar / 53** (per-unit-driven controls read those bases;
the old doc claim *"only affects per-unit reporting, not the absolute-volt
re-solve"* is retracted in place, `exec/save_circuit.rs:572-597`); **P2** the
LoadShape `npts`-first branch of `SaveWrite`
(`R4133:General/DSSObject.pas:144-173`; **both** upstreams guard this class —
0.14.5 does it from the other end, `TLoadShapeObj.SaveWrite` stamping
`PrpSequence[npts] := -999; // make sure Npts prop is first`,
`CAPI:General/LoadShape.pas:2376-2380` — and only this port had neither);
**P3**
`TXYcurveObj.SaveWrite` (`R4133:General/XYcurve.pas:978-1003`, new
`elements/general/xy_curve/save.rs`); **P4** `TRegcontrolObj.SaveWrite`
(`R4133:Controls/RegControl.pas:1399-1421`, new
`elements/control/reg_control/save.rs`), both dispatched beside the four
0.14.5-derived overrides in `report/save/save.rs`. `get_value` is untouched,
`dump.rs` is untouched, and **no `set_as_next_seq`/`clear_seq` site was added or
removed**.

**The `PF=0.88`-class sequence item is EXPLAINED, with no product change — and
the counterfactual is costed, not asserted.** `PrpSequence` is stamped in r4133
**only** by `Set_PropertyValue` (`R4133:General/DSSObject.pas:213-221`), and
`InitPropertyValues` ends in `ClearPropSeqArray` (`:122-129` → `:62-69`), so no
constructor mark survives; `SetAsNextSeq` **does not exist in r4133** (0 hits
over `Version8/Source`). 0.14.5 introduced it as its documented *property
tracking* feature (124 call sites / 20 files), guarded by
`DSSCompatFlag.NoPropertyTracking` whose OFF state is *"following the original
OpenDSS implementation"* (`include/dss_capi.h:497-504`), and the port carries the
six creation seeds verbatim — which is exactly why its ncim line reads
`PF(2) Bus1(3) Phases(4) kV(5) kW(6) Model(7) …`. Nothing is stale: `0.88` is the
live `PFNominal` on **both** engines and r4133's own `Dump` prints `~ pf=0.88`.
The seeds stay because the earlier costing ("0 golden lines") was **incomplete**:
the same bitmap is walked by the **AltDSS JSON export**
(`report/export/json/build.rs:61-70` ← `CAPI_Obj.pas:665-733`), a surface with
**no r4133 counterpart at all**, and the pinned oracle's own bytes are the seeded
chain — `tests/golden/json/vsource_micro.json` emits
`{"Name","MVASC3","MVASC1","BasekV","Bus1"}` for a deck that types neither
`mvasc3` nor `mvasc1` (**15 of the 31** committed AltDSS JSON goldens carry
`MVASC3` — 9 of the 25 in `tests/golden/json/`, all 6 in `json_import/`; and
`circuit_micro.json` likewise emits `Ratings`/`NormAmps`/`EmergAmps` for a line
whose deck typed none of them). Dropping the seeds would trade a measured loss of
agreement with the only oracle that surface has for a hybrid — r4133's membership
over 0.14.5's live values — that matches **neither** upstream. The membership the
port ships is r4133's ∪ {0.14.5 tracking} ∖ {RegControl `tapwinding`}: r4133 does
stamp `Line`'s `Seasons/Ratings/NormAmps/EmergAmps`
(`R4133:PDElements/Line.pas:358-366`), which the port emits in the committed
`tests/golden/adiakoptics/midi_torn_tree.txt:10-11`, and its three
`PrpSequence^[i] := 0` unmark sites all have port `clear_seq` twins. The reverse
direction is pinned too: r4133 stamps `tapwinding` when `winding=` is typed
(`R4133:Controls/RegControl.pas:480-483`), 0.14.5 deliberately dropped it
(*"not really required"*, `CAPI:Controls/RegControl.pas:417`) and the port
followed — round-trip-safe, because re-parsing `Winding=2` re-fires the same
`TapWinding := winding` side effect, asserted by the pin. **No sequence-axis row
has a measured consequence in either direction**: substituting the port's
`Line.dss`, `Vsource.dss`, `ExpControl.dss`, `PVSystem.dss` or `Master.dss` into
r4133's own save changes nothing; only `BusVoltageBases.dss` (P1) and
`Generator.dss` move a number.

**Exposure, measured before the decision and not after it.**

| answer | committed cells it moves | echo-table / channel effect | why not |
|---|---|---|---|
| **A — the store, through `get_value`** (all five readers at once) | **2 049** = 162 `dump*` golden lines + 1 322 feeder-JSON cells + 565 `props/` cells | all **82** `PROPS_ECHO_R4133` rows go stale; **60** `Capi(n)` witnesses over 3 093 comparing cases start failing | the kill criterion — and every one of those goldens is a capture of an oracle that renders **live**, so no regeneration can reconcile them |
| **A2 — a `Save`-only store** | the same `save*` bytes, through a serializer neither upstream has | — | needs a per-property `String` shadow + `InitPropertyValues` tables for ~50 classes; 0.14.5 deleted `FPropertyValue` outright and the port has neither, and `Save` would then disagree with the AltDSS JSON export of the same object |
| **C — split by echo category** (`EchoParse` + `EchoDefault`) | **1 480** = 128 dump lines + 914 feeder cells + 438 props cells | 58 rows retire, 24 stay | it is a transcription of ~49 hand-written `CASE` blocks, not a rule (`model` = store on Generator, live on Storage), and `EchoParse` *is* the print-a-known-stale-value case |
| **KEEP_LIVE_PINNED — landed** | **1** content line + **1** lock digest | none | — |

**Goldens: 2 artifacts, 1 content line + 1 digest — and the capi-oracle question
answered rather than skipped.** `tests/golden/adiakoptics/midi_torn_tree.txt:5`
moves `! CalcVoltageBases` → `CalcVoltageBases` (P1 reaching the A-Diakoptics
`Torn_Circuit` tree through the same writer, `exec/tearing_save.rs:64`), and
`tests/golden/golden.lock.json` re-digests that one row of 737. On this line capi
0.14.5 and r4133 **disagree** and the port follows r4133 — but the moved artifact
is a **self-golden** produced by the port's own partitioner and save writer
(`tests/adiakoptics.rs:565-596`; the lock row's own `"anchor": "self"` /
`"born-self: … no oracle emits a comparable tree"` is the proof), so the
RP3.5/RP3.6 "regenerate from the port, with the argument" precedent applies
vacuously and **no `tools/golden/*.py` was run**. Nothing numeric moved with it:
`adiakoptics` is 34 passed / 1 pre-existing ignored in both lanes *after* the
regeneration, the AD solve gates included, so the P1 A-Diakoptics risk the spec
told I1 to measure is closed on the observed side too. **No
`tests/corpus/ledger.json` entry was drafted or written** — the corpus gate
compares model properties through `compare_all_properties` (`exec/view.rs:394`)
and never reads a `Save` or `Dump` byte on either channel, measured rather than
assumed (`corpus_gate` green on the full population in both lanes, every entry
hit, none stale).

**Literal tests moved: 0. Pins added: 8.** Every `Save`/`Dump` literal keeps its
bytes, because the sequence axis is unchanged and the render axis is unchanged —
`save_class_disabled_load_writes_enabled_no` (`golden_reports.rs:6217-6244`,
keeps `PF=0.88` ×2), the RP3.6 `Save` leg (`exec/tests/line_fetch.rs:1010-1052`,
keeps `R1=…`), RP3.8's `save_renders_the_live_result_properties`,
`save_writes_the_stub_names_like_r4133`,
`save_writes_the_code_name_while_dump_hides_it`, `ckt_model_render_round_trips`,
`save_circuit_writes_master`, `save_forms_structural_file_set`,
`stub_rows_are_absent_from_dump_and_json` and `props_roundtrip`'s
`LANE_SKIP_SCENARIO_PROPS` register with its count assertion — the first two are
re-purposed as *sequence* pins by the new module docs, not edited. The four
re-compilability pins are `save_writes_calcvoltagebases_like_r4133`
(`exec/tests/report.rs:975`), `save_write_puts_npts_first_for_loadshape`
(`:1072`), `xycurve_save_write_puts_npts_first`
(`elements/general/xy_curve/tests.rs:375`) and
`regcontrol_save_write_puts_the_transformer_first`
(`elements/control/reg_control/tests.rs:584`); the four **divergence** pins each
name *both* serializations — `save_renders_the_live_model_after_ncim_pv2pq`
(`report.rs:1202`: the port's `New "Generator.g1" PF=0.88 Bus1=genbus Phases=3
kV=12.47 kW=800 Model=4 Maxkvar=1500 Minkvar=-1500 Vpu=1.01` against r4133's
`New "Generator.g1" bus1=genbus phases=3 kv=12.47 kW=800 model=3 maxkvar=1500
minkvar=-1500 Vpu=1.01`), `dump_renders_the_live_model_after_ncim_pv2pq`
(`:1302`, `~ Model=4` against `~ model=3`),
`save_membership_follows_property_tracking_not_prpsequence` (`:1420`, the
`PF=0.88` head plus the JSON reader's key sequence) and
`save_omits_the_tapwinding_that_r4133_stamps` (`:1507`, the reverse direction,
with the re-parse asserted). Every r4133 byte they quote comes from this
sub-step's `epri-worker` probes (`OpenDSSDirect.dll` 11.0.0.1, rev r4133); the
port's bytes are re-derived live, so a regression on either side breaks the test
rather than the record. One deviation from the spec's letter, measured not
preferred: pin 3 asserts the JSON **key sequence** instead of
`vsource_micro.json`'s verbatim bytes, because the float spelling is the lane's
display kernel (`2.0000000000000000E+003` in parity, `2e3` in default) and is
already pinned by the 123-test `golden_json` binary in both lanes; the oracle's
full parity bytes are quoted in the failure message.

**The round-trip measurement, stated plainly.** r4133's own save round-trips
exactly on `ncim`, `isource_both` and `expcontrol`, and fails
**engine-agnostically** on `capcontrol_pf` (switched-cap step state is in no
`Save`) and `autotrans` (both engines write `Redirect RegControl.dss` before
`AutoTrans.dss` → error 124). The port's save damage was dominated by P1, on
whichever engine re-compiled it; after P1 the one remaining difference on those
five decks is the ncim generator line, where `Model=4` re-compiles to a PQ
machine at pf 0.88 (Q = 800·tan(acos 0.88) = **431.79** kvar) instead of the
authored Q-limited PV machine clamped at 1500 kvar — `|V| genbus.1`
7213.235350 → **7161.277193**, both engines reproducing each other's numbers.
The honest reason is in the pin and belongs in this record too: `Save` renders
live values over an *authored* membership, so on a property the solve mutates it
reproduces neither the authored problem (r4133's answer) nor the full solved
state (the live `kvar` is not in the chain, so it is not printed). That hybrid is
inherent to r4133's design as well — it is what its partial getters produce — and
the port's version of it is the one that never prints a value known to be
superseded.

*Gate.* Landed in **one commit** — P1–P4, the eight pins, the two golden
artifacts and this record together. All five commands green in **both** lanes,
each exit code read individually: **4 435 passed / 0 failed / 5 ignored / 0
filtered out** per lane over **74** test binaries, all 74 `test result: ok` and
the two lanes identical binary for binary (+8 on RP4.1's 4 427, exactly the eight
pins), with the library binary moving 1 482 → **1 490** and every other binary
unmoved — `adiakoptics` 34, `save_roundtrip` 9, `golden_reports` 311,
`golden_json` 123, `props_r4133_pins` 54, `props_r4133_replay` 147,
`props_roundtrip` 1, `corpus_gate` **138** per lane (143.2 s default / 140.5 s
parity, the full 523-case population unfiltered) green on both channels with
every ledger entry hit and none stale. The five ignored are the pre-existing set,
and the 25 `golden_*` binaries were re-run after the gate in both lanes with a
SHA-256 manifest of all 728 files under `tests/golden/` identical before and
after — no byte drift. No `#[ignore]`, no name filter used to claim green, no
tolerance consulted or moved, no `TODO(compat)` added. `pwsh -File
tools/lanes/lane_diff.ps1` was **owed** (the commit touches product `src/`) and
came back **VERDICT: PASS, Δ = 0** on every gated kind over 523 cases /
3 220 861 records — `conv`, `cur`, `errs`, `iter`, `loss`, `pow`, `v`, `y` all
`max |d| = 0.000e0`, 0 iteration counts drifted — so the default lane stays
bit-identical to the parity lane and keeps precisely its oracle standing (the
2026-07-31 baseline, unchanged). Expected, and now measured: P1-P4 change the
bytes an emitted deck carries, not the solved model the dump stream compares.

**Open, recorded not chased.** (a) **P0, release-blocking:** a plain user script
panics a `#![forbid(unsafe_code)]` product crate —
`solution/solution/ncim.rs:683`, *index out of bounds: the len is 1 but the index
is 1*, the PQ→PV arm writing `gobj.delta_q_nom[j]` for `j < nphases` while the
vector is still length 1, because the sizing happens in a branch a
model-4-at-birth generator never enters; an 11-line repro with no `Save`
involved, and r4133 on the identical deck does not crash. (b) **NCIM PV→PQ
reporting violates KCL**: on the unmodified `ncim_pv_pq.dss` the port reports
`Generator.G1` −800 kW / **−431.8 kvar** (42.0103 A) while the solve injects the
clamped −1500 kvar (78.5593 A on r4133) — node voltages and the Line/Load rows
match r4133 digit for digit, so KCL at `genbus` is off by 1068.2 kvar; same
family as the Newton stale-`Iterminal` bug, and it is what makes the saved
`Model=4` line look self-consistent. (a) and (b) want one dedicated sub-step —
an **RP3.13** stub is drafted in the plan. (c) `Master.dss` divergences with **no
measured consequence**: the port prepends `! Saved by dss-rs`, `Set
DefaultBaseFreq=60`, `Set EarthModel=Deri` (r4133 writes none), emits `Redirect
Vsource.dss` after the library block (r4133 writes it first,
`R4133:Common/Circuit.pas:2652-2668`) and never writes `GISCoords.dss`/its
`GIScoords` line (`:2776-2779`). (d) Both engines' saves put `Redirect
RegControl.dss` before `AutoTrans.dss`, so an AutoTrans+RegControl deck
round-trips on **neither** — an upstream defect worth an
`investigations/to_opendss/` report, not a port change. (e) Two `Dump`
store-vs-live rows measured **outside** the 82-row echo table —
`capacitor.faultrate` (r4133 `0` vs port `0.0005`) and `autotrans.tap` (r4133 `1`
vs port `1.06875`, the live regulator tap); since `compare_all_properties` reads
the same getter, §RP5.2 should confirm whether the owning cases are r4133-gated
and, if so, whether a row is missing. (f) r4133 has **64** `DumpProperties`
overrides to the port's 0.14.5-derived **20** (`!DQDV=`, the 34/36 double-paren
wrap, the hardcoded `~ Refuel=False` that contradicts r4133's own getter —
`generator.pas:2495` vs `:3028`): 88 goldens sit on the answer, it has **no**
bearing on the store-vs-live verdict, and it is recorded as an unowned scope
question rather than silently answered. (g) The port has no counterpart of
r4133's `Set_NumPoints` *"keep properties in order for save command"* re-stamp
(`R4133:General/LoadShape.pas:631-636` + `:1665-1677`, `PriceShape.pas:303` +
`:910-916`, `TempShape.pas:302`, `XYcurve.pas:1005-1019` — the setter re-stamps
the sizing property **and then** the array property, so the two stay adjacent in
that order). P2/P3 made it invisible on `Save` for `LoadShape` and `XYcurve`
only; the audit settlement measured the other five classes and closed them with
the port's own sizing-property hoist (P7 below), so `Save` is now guarded on all
seven. What stays open is the **AltDSS JSON export**, which walks the same
un-re-stamped bitmap and is not touched by a Save-time hoist. (h) The divergence runs both ways: r4133's own `SaveWrite`
emits `windgen.kvar=0`, which on reload flattens `PFNominal` to 1.0 and
`kvarMax/kvarMin` to 0 (`tests/props_r4133_replay.rs:1197-1199`) — an upstream
defect on this very surface, where the port is already right; cross-referenced
from the first divergence pin.

**RP3.11 audit settlement (2026-09-03) — the declared "union of both upstreams'
`SaveWrite` overrides" is now actually shipped, and the sizing-property guard
covers every curve class instead of two.** Both auditors landed on the same
substantive gap from opposite sides, and both were right: the sub-step stated a
policy it implemented for 6 of the 8 upstream overrides, and the biggest thing
that policy would have removed — a silent, converging wrong circuit — was still
in the tree. Three product changes, all lane-unconditional, none of them touching
*which* value is printed: **P5** `TXfmrCodeObj.SaveWrite`
(`CAPI:General/XfmrCode.pas:667-745`, new
`elements/general/xfmr_code/save.rs`) — without it a 3-winding code saved as
`New "XfmrCode.xc" … Wdg=3 Conn=wye kV=4.16 kVA=5000 %R=0.7 Tap=0.975`, the
active winding only, and re-compiled into a *different* code that converges;
r4133 has the identical defect (measured: `New "XfmrCode.xc" phases=3 windings=3
Xhl=7 Xht=9 Xlt=8 wdg=3 conn=wye kV=4.16 kVA=5000 %R=0.7 tap=0.975`), 0.14.5
fixed it, and the 2026-08-02 policy forbids reproducing r4133's side. **P6**
`TDynEqPCE.SaveWrite` (`CAPI:PCElements/DynEqPCE.pas:252-273`, the `UserDynInit`
tail, dispatched in `report/save/save.rs` because Pascal appends it after
`inherited`) — closes the Phase-8 deferral named in `elements/pc/dyneq_pce.rs`;
on the vendored `Dynamic_KundurDynExp-steady-state-only.dss` r4133 writes
`… DynOut=[speed,dpshaft,]` and stops, losing all six state-variable
initializers (r4133 has no `UserDynInit` at all), while the port now emits and
re-reads `damp=0 pshaft=P0 pterm=P speed=0 theta=Edp mass="3.5 2 * 2220000000
376.99112 / *"`. **P7** the sizing-property hoist
(`report/save/save.rs::sizing_property`): P2/P3 guarded `LoadShape` and
`XYcurve`, but `TCC_Curve`, `GrowthShape`, `PriceShape`, `TShape` and `Spectrum`
still emitted `npts`/`numharm` **last** whenever a deck re-set it after the
arrays, and that line reloads as zeros. Measured on the live r4133 DLL, three of
the five have the same defect upstream (`New "TCC_Curve.z" C_array=[ 1 2]
T_array=[ 10 5] npts=2`, `New "GrowthShape.g" year=(1, 2, ) mult=(1.05, 1.06, )
npts=2`, `New "Spectrum.sp" harmonic=(1, 3, ) %mag=(100, 30, ) angle=(0, 0, )
NumHarm=2`) and two come out safe only through the `Set_NumPoints` re-stamp this
port does not have (item (g)) — so the guard is the port's own, *hoisting* a
sizing property the deck actually set and never adding a token. Exposure: **0**
golden bytes (every committed `Save` line already carries its sizing property
first, and no golden holds a saved `XfmrCode` or a `DynInit` tail), **0**
tolerances, **0** ledger rows, **3** pins added
(`save_rewrites_xfmrcode_windings_like_capi_0145`,
`save_writes_the_dyn_init_tail_like_capi_0145`,
`save_puts_the_sizing_property_first_for_every_curve_class`, all in
`exec/tests/report.rs`), each with a re-compile leg that asserts the recovered
data.

*Findings, one line each.* **AC-1/T2 (major, FIXED)** — the missing
`TXfmrCodeObj.SaveWrite`: P5 above. **AC-2/T3 (major, FIXED)** — P2's doc claimed
*"r4133 never hits that case"* and *"r4133-only: neither dss_capi 0.14.5 nor this
port had it"*; both are false and both are now corrected in the product doc, in
the pin and in this record: 0.14.5 **does** guard `LoadShape`
(`PrpSequence[npts] := -999`, `CAPI:General/LoadShape.pas:2376-2380`), and r4133
**does** print the token twice on any shape that parses no array property
(measured: `New "LoadShape.ls3" npts=5 npts=5`, `New "LoadShape.ls4" npts=4
npts=4 interval=2`). The port's one-`npts`-first output is therefore a
deliberate non-reproduction, now pinned naming both serializations by the two new
legs of `save_write_puts_npts_first_for_loadshape`. **AC-3 (minor, FIXED)** —
`TDynEqPCE.SaveWrite`: P6 above. **AC-4 (minor, FIXED, doc)** — the two new
overrides route through `save_write_token`, which trims and skips the `----`
sentinel where their Pascals do neither; both deviations are now written down at
the call sites (they can only suppress a token that would not re-parse, and no
reachable property of either class renders blanks or the sentinel). **AC-5
(question, RECORDED)** — the round-trip measurement is pinned, not closed: the
port's saved `ncim` deck still re-compiles to `|V| genbus.1` 7161.277193 against
the original 7213.235350 on **both** engines. That is what the plan's kill
criterion prescribes (the alternative is printing `model=3` beside a live
`gen_model == 4`), and closing it in substance depends on open item (b), owned by
the proposed §RP3.13 — not re-opened here. **AC-6 (note, FIXED)** — the four pin
line citations pointed at the `#[test]` attribute; they now name the `fn` lines
and were re-anchored after this settlement's edits. **T1 (major, FIXED)** — the
five unguarded sizing classes: P7 above, and open item (g) is rewritten (its
claim that P2/P3 make the missing re-stamp *"invisible on Save"* was true for two
classes out of seven; what remains open is the AltDSS JSON export, which a
Save-time hoist cannot reach). **T4 (minor, FIXED)** — none of the pins was
enumerated by anything, so a rename left the suite green while STATUS cited the
name: `props_r4133_replay.rs::RP311_SERIALIZATION_PINS` (11 rows) +
`every_rp311_serialization_pin_exists_and_is_cited` now assert both halves —
every row names a real `#[test]` **and** is still cited by name in `STATUS.md`.
**T5 (minor, FIXED)** — `save_roundtrip`'s snapshot compared only absolute-volt
quantities, so reverting P1 left all nine feeder round trips green;
`bus_kv_bases` now compares every bus's `kVBase` **exactly** across the round
trip, with a pre-save vacuity guard. Proven discriminating: with `!
CalcVoltageBases` restored, **7 of 9** cases red with
`bus "611" kVBase changed across save round-trip (2.4017771198288433 -> 0)`.
**T6 (minor, FIXED)** — the `Dump` verdict pin was three `contains` over a
~160-line artifact; it now reads the live field first (`? generator.g1.model` ==
`4`), asserts the header, asserts the **48** `~` rows are all there, and compares
the `Model`, `kvar` and `PF` rows by exact line. Its `431.79425771047` message no
longer calls that number *"the live dispatched kvar"* — it is the
`PFNominal`-derived nominal (800·tan(acos 0.88)), identical on both engines, and
the message now cross-references open item (b), whose 1068.2 kvar KCL gap is the
same number seen from the Powers side. **T7 (note, FIXED for the deciding pin)** —
`save_renders_the_live_model_after_ncim_pv2pq` now *derives* r4133's half: it
reads the `model=` token out of the deck's own bytes, asserts it is `3` (r4133's
getter has no arm 6, so its `SaveWrite` echoes the parsed token verbatim) and
asserts the live value differs, so both numbers of "the port prints 4 where r4133
prints 3" are measured in-test. **T8 (note, RECORDED, evidence closed)** — the
golden re-run script covered the 25 `golden_*` binaries but not
`tests/adiakoptics.rs`, the owner of the one golden that moved; `adiakoptics` +
`golden_lock` were run explicitly (34 + 4 green, the digest equal to the file's
own SHA-256), and they are part of the full `cargo test --workspace` gate below
in any case. *Gate:* all five commands green in **both** lanes on the
settled tree, each exit code read individually — **4 439 passed / 0 failed / 5
ignored / 0 filtered out** per lane over **74** binaries, all 74 `test result:
ok` and the two lanes identical binary for binary (**+4** on RP3.11's 4 435: the
three new pins, library 1 490 → **1 493**, plus the new citation guard,
`props_r4133_replay` 147 → **148**); `corpus_gate` green on the full unfiltered
population in both lanes with every ledger entry hit and none stale;
`save_roundtrip` 9, `adiakoptics` 34 (+1 pre-existing ignored), `golden_lock` 4.
**0** golden bytes moved (`git status --short tests/golden` empty after the run),
0 ledger rows, 0 tolerances, no `#[ignore]`, no name filter. `pwsh -File
tools/lanes/lane_diff.ps1` was owed (product `src/` moved) and came back
**VERDICT: PASS, Δ = 0** on every gated kind over 523 cases / 3 220 861 records
(`conv`/`cur`/`errs`/`iter`/`loss`/`pow`/`v`/`y` all `max |d| = 0.000e0`, 0
iteration counts drifted) — expected, since the settlement changes the bytes an
emitted deck carries, not the solved model the dump stream compares.

**RP4.1 (`all_properties` unmasked on the r4133 channel) landed 2026-09-03 —
WP-RP4's single sub-step and G1.1's deliverable, in one commit, with zero
product-crate lines, zero golden bytes and zero tolerances moved.** The
per-channel `compare_all_properties = false` clears are gone from both the
**gate** path and the **seeding** path (`corpus_gate/scheduler.rs`), and
`force_properties` now forces the property request on **every live non-`large`
case** — `gates_capi ∪ gates_r4133` is every live case, so the honest spelling
replaced the two-arm test, and the `large` cost guard stays. The **83 r4133-only
non-large** cases get a property check for the first time and the **313
non-`large` `both`** cases get their r4133 property table compared: a full run
now does **1 670** gating r4133 property walks over **151 782** elements,
identically in both lanes. (The audit settlement corrected this figure from
"367 `both`": the cost guard leaves the remaining **54** `both` cases —
every `kind=large*` one — property-unchecked on *both* channels, which is the
plan's deliberate cost guard and an open item for the RP5.2 closing record, not
a regression.) ~24 stale doc claims asserting that the r4133 channel never
property-compares were corrected, or left with a dated "superseded by
R4133_PROPS RP4.1" note where the file is a historical record
(`docs/phase-records/`, `UNIFIED_GATE_PLAN.md`). The commit is **23 files
(+2 576 / −536)** (`59e521e5`; the first draft of this paragraph said 22 files
/ +2 305 / −515 and missed `tools/oracle/README.md` — corrected by the audit
settlement); the only `src/` diffs are comment-only, in a `#[cfg(test)]` module
(`exec/tests/controls.rs`) and in the `publish = false` bridge
(`crates/dss-epri`).

*Precondition A (RP2.3's audit settlement) — the 20 mixed echo rows narrowed
**per cell**, before the flip.* Mechanism (b) of the plan's two, per the
coordinator's ruling: `ECHO_NARROWED` gives each of the 20 rows the measured
`(rust, oracle)` spellings its exclusion still covers — **66** spellings, every
one present verbatim in the frozen census — and `echo_excluded` /
`echo_excluded_r4133` match on them via `row_covers`; the 62 pure echo rows keep
the conservative pair scope, and `ECHO_CARVE_OUTS`' doc was rewritten to own the
inversion for the 20 explicitly (on those rows an *unseen* spelling now falls
**out** of the exclusion and is compared — which is the point of the
precondition). `MULTI_LINK_ROWS` 135 → **0** (no frozen row matches two links
any more), the old population restated as `MIXED_PAIR_NORM_ROWS = 135` = the 20
pairs' 201 census rows − 66. `a_mixed_pairs_echo_row_masks_the_cells_its_rule_refuses`
was **replaced, not deleted**: its first assertion now asserts the cell is
COMPARED on r4133, with the capi and neighbour assertions kept.

*Precondition B (RP3.1's audit settlement) — the staged entries landed and the
accounting moved with them.* The **eight** entries landed verbatim from their
STATUS drafts into `tests/corpus/ledger.json` — `r4133-swtcontrol-delay-ignored-time`
and `-midi` (RP3.1), `r4133-windgen-kvar-dispatched-{daily,delta,dyn,dynfault}`
(RP3.2), `gic-pct-r2-honoured-gictransformer-r4133-props` and
`gic-pct-r2-honoured-midi-r4133-props` (RP3.4) — with the two new causes
`swtcontrol-delay-not-wired` and `windgen-kvar-renders-dispatched-q`; RP3.4's
two reuse the existing `gic-pct-r2-ignored`. Ledger **45 → 53 entries / 27 → 29
causes**. The accounting move needed a mechanism, not a hand edit: `DECLARED_RP3`
is asserted against the measured walk, so RP4.1 landed `RP3_LEDGERED` (the
interception shape `RP38_SUPERSEDED` already uses) with its own bucket lock
`LEDGERED_RP3 = (6, 3, 6)` and a both-ways guard that reads the eight ids back
out of the live ledger; `DECLARED_RP3` is `(0, 0, 0)` as a *measurement*, and the
three `RP3_ROUTING` rows (`swtcontrol.delay`, `windgen.kvar`,
`gictransformer.r2`) are retired to `0, 0` with their verdicts amended to "landed
at RP4.1". The tripwire was renamed
`the_staged_r4133_property_entries_have_not_landed_yet` →
`the_staged_r4133_property_entries_landed_at_rp41` and re-stated positively (the
landed `property`-scoped `r4133` set is exactly `LEDGER_ENTRY_PINS`' eight, each
on the case its pin cites); the "no NEW un-reviewed entry" half moved to the
`RP3_LEDGERED` guard. RP3.12's four `controls:autotrans/*` decks and
`makeposseq_ctrl.dss` keep `engines: capi_v0145` and their drafted skips were NOT
landed (coordinator decision 5), and `DECLARED_RP39` / RP3.12's tables were not
retired (decision 6).

*Residual triage — the kill criterion does not fire.* **Zero** ledger entries and
**zero** pins were born from RP4.1's own residual triage (criterion: more than
~15, or any residual that cannot be pinned → stop; the eight of precondition B do
not count), and no residual failed to be pinned. What the first unmasked run did
find was a **stale exclusion**, not a divergence: the two `ArrayForm` rows
`swtcontrol.normal` and `swtcontrol.state` folded nothing across 40 compared
cells, because RP3.7(a) made both sides spell the array identically
(`Version8/Source/Controls/SwtControl.pas:589-599` / `:600-610`, per-phase
`pStateArray`). The rows are dropped and re-owned by the RP3.8-shaped
`RP37_SUPERSEDED` (`SUPERSEDED_RP37 = (5, 2, 5)`, `Ledger::superseded` now per
table), with `DECLARED_RP35 (8, 6, 5) → (5, 4, 2)` and the six claim locks
re-derived: `NORM_ROWS` 170 → 168, `NORM_ARRAY_FORM_ROWS` 23 → 21,
`CLAIMED_ARRAY_FORM` 203 → 201, `CLAIMED_NORMALIZATION` 854 → 852,
`CLAIMED_TOTAL_LIVE` 855 → 853, `CLAIMED_SPELLINGS_LIVE` 2 982 → 2 980. The live
census independently reports 21 `ArrayForm` pairs / 201 spellings, so the offline
and live accountings now agree on this rule.

*Acceptance — the census at HEAD against the P0 baseline.* One full-population
run, not an aggregation of `DSS_GATE_ONLY` slices: **440 cases × 2 channels,
1 059 178 rows, 56 s**, `gate_only: null`.

| r4133 disposition | P0 cells | HEAD cells | P0 in scope | HEAD in scope | pairs P0 → HEAD |
|---|---|---|---|---|---|
| `UNCLAIMED` | 534 | **528** | 30 | **0** (reading rule below) | 49 → **47** |
| `ledger-hit` | 0 | **6**\* | 0 | **6**\* | 0 → **2** |
| `echo-row` | 488 019 | 488 019 | 468 046 | 468 046 | 82 → 82 |
| `normalized-by-BoolFold` | 294 519 | 294 519 | 280 915 | 280 915 | 77 → 77 |
| `normalized-by-CaseFold` | 99 023 | 99 023 | 95 270 | 95 270 | 65 → 65 |
| `normalized-by-ArrayForm` | 122 754 | 122 754 | 118 744 | 118 744 | 21 → 21 |
| `normalized-by-EnumSynonym` | 4 619 | 4 619 | 3 817 | 3 817 | 5 → 5 |
| `under-floor` | 49 484 | 49 484 | 46 627 | 46 627 | 71 → 71 |
| value cells | 1 058 952 | 1 058 952 | 1 013 449 | 1 013 449 | — |

\* **Second reading rule, to be carried forward:** acceptance is read off the
lossless `props_census.json`, **never** off `claims_summary.json`, which
collapses a mixed-disposition spelling to its weakest cell
(`mixed_disposition_spellings: 1`, `swtcontrol.delay` — 24 in-scope `ledger-hit`
cells on the two entry decks plus 12 out-of-scope `UNCLAIMED` on the capi-only
`swtcontrol_lock.dss`). Streamed per row over all 1 059 178 rows the lossless
record reads: r4133 `UNCLAIMED` in scope **0**, `UNCLAIMED` out of scope **504**,
`ledger-hit` in scope **30** = 24 (`swtcontrol.delay`) + 4 (`windgen.kvar`) + 2
(`gictransformer.r2`) — cell for cell the eight entries' 30 live gate hits, and
exactly the P0 prediction. No new pair appeared (two deletions, zero additions).
The **capi channel A/B is bit-identical**: all 8/8 capi_v0145 census artifacts
byte-equal to the pre-flip baseline.

**Plan-text deviation, recorded verbatim (coordinator decision 1, 2026-09-02):**
"**Acceptance is read as "zero UNCLAIMED cells IN SCOPE"** (the gate never
compares an out-of-scope cell, so the plan's "per-cell accounting closes" applies
to compared cells). No new census disposition is added in RP4.1. P4 MUST
additionally close the out-of-scope accounting in its handoff and STATUS with a
per-owner table of every out-of-scope UNCLAIMED cell (RP3.9 pins / RP3.12 pin /
`RP24_OUT_OF_SCOPE` gendispatcher / G2.5 Cuf), each owner citing its pin or
record, so the total (534 → whatever HEAD reports) is accounted line by line."
The 504 out-of-scope cells — **22 cases / 47 pairs, all on `engines:
capi_v0145` decks** — are accounted per owner:

| cells | dominant pairs | owner and citation |
|---|---|---|
| 195 | `generator.kvar` 104, `generator.kw` 91 (`controls:gendispatcher/*`) | **RP1.4** (decks stay capi-only, artifact = a `PROPS_015X` allowlist row) + **RP2.4** offline: `RP24_OUT_OF_SCOPE` row `("generator.kvar", 1.55e-06, 102)`; r4133 does not register `GenDispatcher.weights` at all |
| 136 | `autotrans.wdgcurrents`/`.tap`/`.taps` and `regcontrol.tapnum`, 34 each | **RP3.12** — `RP312_UPSTREAM_BUG`, cause `regcontrol-autotrans-typecast`, pin `autotrans_wdgcurrents_stay_regulated_where_r4133_never_taps_the_autotrans` (`RegControl.pas:1026/:1296/:1479`, `AutoTrans.pas:88`) |
| 72 | `load.kva`, `vsource.puz*`/`isc3`, `capacitor.*amps`, `line.b*` (`modes:makeposseq/*`) | **RP3.9** — `DECLARED_RP39 = (55, 27, 19)`, verdict `PRECISION_ROUNDTRIP`, pins in `RP39_PINS` |
| 68 | `sensor.kvs` 61, `sensor.currents` 5, `.kws`/`.kvars` 1+1 | **`Owner::OutOfScope`** (RP2.1/RP2.3) — `CELL_DISPOSITION` rows, "plan §1.3 / §RP2.1" |
| 18 | `swtcontrol.delay` on `swtcontrol_lock` plus three `IEEE_519` copies | **RP3.1**'s own cause `swtcontrol-delay-not-wired`; the decks are capi-only, so an entry there would be NEVER APPLIED |
| 8 | `storage.kva/kw/kwrated`, `storagecontroller.kwtotal/kwactual` | **RP2.3** pin `storagecontroller_fleet_aggregates_render_the_live_fleet` (plus RP2.4 for `storage.kw`), `RP38_SUPERSEDED` |
| 1 | `storagecontroller.kwneed` | **RP2.4** — `RP24_OUT_OF_SCOPE` row `(…, 4.96e-06, 1)` |
| 1 | `capacitor.cuf` (`[ 4]` vs `[ 4E-006]`) | **G2.5** — `RP24_OUT_OF_SCOPE` row `("capacitor.cuf", 4.00e-06, 1)` |
| 5 | `line.r1/x1/rmatrix/xmatrix`, `isource.bus1` | **`DECLARED_OUT_OF_SCOPE`** scope arm only, `(221, 21, 0)` — the only five whose owner is a scope arm rather than a cause; flagged, not owed by RP4.1 |
| **504** | **22 cases / 47 pairs** | **total, fully accounted** |

**Second plan-text deviation, recorded verbatim (coordinator decision 2):**
"**RP1.4 staged no ledger entry** (its artifact is a `PROPS_015X` allowlist row;
decks stay capi-only). Correct the phrase in BOTH copies (plan §RP4.1 and the
tripwire doc) in P2." Both copies are corrected: RP4.1 landed **eight** entries,
not "eight plus RP1.4's".

*The re-mask alarm.* `population.lock.json` fingerprints manifest flags and
per-case ledger tags but **no scheduler code**, so re-masking the request would
be invisible to it. RP4.1 therefore adds
`props_norm::assert_r4133_props_compare_ran()`, invoked once from the corpus
gate's epilogue beside the two per-row liveness guards and self-silencing under
`DSS_GATE_ONLY`; it fails when the r4133 property compare never ran, and the
epilogue also prints the live figure. **Deviation from the plan's letter,
deliberately:** the plan spelled the quantity as `visits > 0` summed over the two
tables, but those statics are moved by sibling unit tests in the same binary that
drive the comparator themselves — the sum was already > 0 through the whole
masked era and would stay green under a re-mask — so the counters are bumped at
the single gating call site (`compare_all_properties`'s `PropsChannel::R4133`
arm) and the census walk is deliberately not counted. Both directions are pinned
offline over injected counters. Together with the landed entries (which would go
NEVER APPLIED in `assert_all_hit`) a wholesale re-mask is loud in two independent
ways. The two per-row guards `assert_norm_rows_are_live` /
`assert_echo_rows_are_live`, dormant since RP2.1, are now live and silent.

*Lock.* `population.lock.json` regenerated in the same commit: **8 lines**, all
in `family_rigor`, all moving only the `ledger=` component of the eight entries'
cases. The `props=` component did not move on any line — it is the *manifest*
flag, and RP4.1 changed the scheduler, not a manifest.

*Gate.* All five commands green in both lanes, each exit code read individually:
**4 382 passed / 0 failed / 5 ignored / 0 filtered out** per lane over 74 test
binaries (+92 on the RP3.12 settlement's 4 290 — four new
`harness::props_norm::tests` compiled into each of the 22 harness-linking
binaries, plus `props_r4133_replay`'s own four; `props_r4133_pins` stays at 54,
0 entries and 0 pins being born here), the same five pre-existing `ignored`, no
`#[ignore]` and no name filter. `corpus_gate` **135** per lane over the full
523-case population, ledger **53 entries / 1 534 hits**, none stale, zero
NEVER-APPLIED, zero reds on either channel. Wall clock for `corpus_gate`:
**141.78 s** default / **142.94 s** parity against **142.90 s / 139.68 s** before
the unmask — the flip costs nothing measurable (±2 % run-to-run) against a
budgeted +1–3 min, even though the run now walks 1 670 r4133 property tables.
`lane_diff` was **run to completion** although no product line moved (so it was
not owed): `VERDICT: PASS`, 523 cases / 3 220 861 records / 4 825 419 compared
values, **`max |Δ| = 0.000e0` and `max rel = 0.000e0` on every one of the eight
kinds** (`conv`, `cur`, `errs`, `iter`, `loss`, `pow`, `v`, `y`), zero iteration
drifts — the 2026-07-31 bit-identical default↔parity baseline reproduced
unchanged. The known overlapping-guard snapshot race again dropped untracked
`Test/AutoTrans/*.txt` (fifth sighting; §"Standing open follow-ups"), removed by
exact name; no tracked corpus or golden file moved.

*As-executed corrections to the plan's cited lines* (reported, never edited into
the historical text): `scheduler.rs:357-363` → `:359-365`; `:710-717` →
`:717-721`; `:97-114` → `:97-116`; `harness/mod.rs:1452-1458` → `:3043-3049`
(the cited range is now unrelated power/loss code); `capture.rs:619-664` → fn
`:630-662`, doc `:618-628`; the population figures "96 r4133-only / 366 both /
462 gating" → the lock reads **97 / 367 / 464** (523 cases, 80 `large`, 396
non-large r4133-gating) and must be re-derived, not transcribed.

*Open follow-up (recorded, not RP4.1 work).* `DECLARED_RP35` still declares four
pairs — `line.units` (RP3.5), `line.linecode` (RP3.6), `relay.normal` and
`relay.state` — whose HEAD census shows no divergent cell either, but a
superseded row owes a per-pair live disposition, a cited r4133 getter arm and a
pin, and nobody has produced that trio for them.

*Audit settlement (2026-09-03, RP4.1 — ten findings from the two auditors, four
minor + six notes; audit-code's partial-re-mask note and audit-tests' T3 are the
same issue, so nine dispositions: eight fixed, one recorded, every one settled
against evidence rather than plausibility).*

* **The coverage headline was overstated by 54 cases (audit-code 1) — FIXED.**
  "The **367 `both`** cases get their r4133 property table compared" counted the
  whole `engines: both` population, but `force_properties` keeps the plan's
  `!kind.starts_with("large")` cost guard, and all 79 live `kind=large*` decks are
  `solvable_now`. Re-derived from `population.lock.json`: 523 cases → 519 live →
  **440 forced** = **313** `both` + **83** r4133-only + 44 capi-only, which is
  also exactly the 440 the claims census walks and the only reading consistent
  with the measured 1 670 walks. The three copies of the sentence (this record,
  the frontier note, the plan's as-executed part 2) now say **313 non-`large`
  `both`**, and the **54** `kind=large*` `both` cases that have no property
  compare on *either* channel are recorded as an open item for the RP5.2 closing
  record. The figures are no longer prose: `corpus_gate::scheduler::
  the_property_forcing_rule_is_every_live_non_large_case` walks the four
  manifests without an oracle, asserts the forced set **is** the live non-`large`
  population case for case, and pins the split as
  `FORCED_PROPS_POPULATION = (440, 313, 83, 44)`.
* **That new test is also the missing PARTIAL-re-mask alarm (audit-code note /
  audit-tests 3) — FIXED.** `assert_r4133_props_compare_ran` is a boolean
  (`walks > 0`), so re-adding `gates_capi()` to `force_properties` — which would
  drop all 83 r4133-only cases while the 313 `both` ones keep ~1 300 walks —
  passes it; what caught that until now was `assert_all_hit` on the six landed
  entries whose cases are `engines: "r4133"`, an *implicit* coupling that would
  vanish with those entries. The scheduler test fails on any re-mask, partial
  included, without depending on the ledger; the coupling is now stated in the
  global guard's doc, and the guard is invoked **first** in the gate epilogue so
  "nothing ran at all" reports as one line instead of nineteen row messages.
* **The narrowed echo rows had lost their live staleness detector (audit-tests 1)
  — FIXED.** After precondition A a narrowed row counts only cells it *covers*,
  so `visits == hits` by construction and `check_echo_rows_are_live`'s
  `visits > 0 && hits == 0` arm can never fire on one; a narrowed row going dead
  the way `swtcontrol.normal`/`.state` did would have been invisible, because the
  compensating derivation test reads the **frozen** census. The guard grows the
  mirror-image arm: a narrowed row with `visits == 0` in a run whose seam ran at
  all is stale. Sound on this population — the HEAD claims census gives 19 of the
  20 pairs between 4 and 75 160 in-scope `echo-row` cells, and the twentieth
  (`fault.bus2`) is already the sanctioned `ECHO_ROWS_WITH_NO_IN_SCOPE_CELL` row
  — and both directions are pinned offline over injected counters
  (`the_echo_liveness_guard_is_silent_when_every_narrowed_row_was_visited`,
  `the_echo_liveness_guard_fires_on_an_unvisited_narrowed_row`); the old stale-row
  pin moved from `regcontrol.idle` (now narrowed) to the pair-scoped
  `monitor.mode`, so it still tests the arm it names.
* **`ECHO_NARROWED`'s doc claimed a per-citation review nobody performed
  (audit-code 2) — FIXED as a doc correction, the substance verified.** The 66
  spellings are derived mechanically ("the pair's census rows the typed rule does
  not claim"), i.e. exactly the cells that rule refused, so on the measured
  population the narrowed exclusion is cell-for-cell the old pair-scoped one and
  **the narrowing unmasks nothing the flip would not have unmasked anyway**;
  what it buys is prospective — a *new* spelling on one of these 20 pairs is
  compared instead of inheriting the citation. The doc now says that in the
  shipped table (and no longer says "the spellings its citation actually
  explains"), with the same correction in this record. The auditor's stronger
  option — one assertion per narrowed row tying each spelling to its
  `EchoCategory` shape — was weighed and **not** taken: the categories are prose
  citations of a Pascal getter arm, and turning them into a machine-checkable
  shape is an RP5-sized invention, not a settlement; what protects the 20 pairs
  meanwhile is that an unlisted spelling is now COMPARED.
* **STATUS's own commit-size line was wrong (audit-code 3) — FIXED.** `59e521e5`
  is **23 files, +2 576 / −536**, not 22 / +2 305 / −515; the missing file was
  `tools/oracle/README.md`.
* **The census artifacts a reader opens first still report a non-zero
  (audit-code note) — FIXED in the writer.** `claims_summary.json` and
  `claims_unclaimed_pairs.txt` are **per-spelling** and fold a mixed-disposition
  spelling to its weakest verdict, which is why `swtcontrol.delay` still shows
  "UNCLAIMED, 24 in scope" after its two entries landed. Both artifacts now carry
  the reading rule in themselves — a `reading_rule` field and a header note — and
  the module doc says acceptance is read off the lossless `props_census.json`; the
  stale comment "`mixed_disposition_spellings` — 0 on every measured run" is
  corrected to 1, naming the spelling.
* **`force_properties`' family arm has no `large` guard, and `Row::in_scope` is
  `engines`-only (audit-code notes) — FIXED as assertions/doc.** The family arm's
  premise ("no family deck is `kind=large*`") is now asserted by the new
  scheduler test instead of being inert-by-luck, and `Row::in_scope`'s doc states
  that the gate additionally excludes `kind=large*` and that the two predicates
  coincide only because the census walks the forced population.
* **`RP3_LEDGERED` credited a spelling no landed entry names (audit-tests 2) —
  FIXED.** The interception retires rows per PAIR while an entry pins one exact
  `(rust, oracle)`, so the frozen row `swtcontrol.delay '0' vs '120'` counted as
  ledgered although both entries pin `rust: "0.25"`. Measured at HEAD: its 6 cells
  sit on the three `Version8/Distrib/Examples/**/IEEE_519.DSS` decks, all
  `engines: capi_v0145`, **0 in scope** — no entry is owed, but the accounting now
  says so out loud. New `RP3_LEDGERED_UNNAMED` names the spelling with that
  measurement and the same `SwtControl.pas:195-218` / `:310` citation the entries
  carry, and the guard test checks both ways: an unnamed spelling that is not
  listed fails, and a listed row that stops being unnamed fails.
* **`CLAIMED_TOTAL_LIVE` / `CLAIMED_SPELLINGS_LIVE` enforce only their offline
  half (audit-tests 4) — RECORDED, not fixed.** The identity `cargo test` checks
  is `CLAIMED_NORMALIZATION + LIVE_ONLY_SPELLINGS.len()`; the "and the live census
  measures the same" half rests on an opt-in `DSS_PROPS_CENSUS=claims` run. That
  is deliberate — the mandatory gate must not require an oracle census — and the
  numbers were re-measured at HEAD (21 `ArrayForm` pairs / 201 spellings). The
  limitation is now stated at the constant instead of being implied by its name.

*Gate (settlement).* All five commands green in both lanes on the settled tree —
**4 427 passed / 0 failed / 5 ignored** per lane over 74 binaries (+45 against
RP4.1's 4 382: the two new offline echo pins compile into every
harness-including binary, plus the scheduler test), `corpus_gate` **523/523**
unfiltered in both lanes (147.5 s default / 142.2 s parity) with `assert_all_hit`
green over an untouched `ledger.json`. Nothing product-side moved in the
settlement either (test harness, census writer and docs only), so `lane_diff` was
not owed a second time. The known overlapping-guard snapshot race dropped five
untracked `Test/AutoTrans/*.txt` again (sixth sighting); removed by exact name,
no tracked corpus or golden file moved.

**RP3.9 landed 2026-09-02** (audit settled 2026-09-03) — the display floor's
round-trip residue is settled as 27 pinned `PRECISION_ROUNDTRIP` pairs (the
§RP3.9 record above), so **every RP1–RP3 sub-step the unmask waits on has
landed** (RP3.5 2026-08-28, RP3.6 both parts 2026-08-29, RP3.7 all three parts
2026-09-02, RP3.8 2026-09-02, RP3.9 2026-09-02) and nothing in WP-RP1/2/3 blocks
it any more. **RP3.12 landed 2026-09-03** — RP3.9's P0 open item,
the 34 `controls:autotrans/*` `wdgcurrents` cells, is settled as an r4133
`UPSTREAM_BUG` that no lane reproduces (§RP3.12 record above); it adds no
precondition to the flip, since all four of its decks are capi-only and the
r4133 channel gates none of them. (The two RP3 sub-steps outside that rule by
construction — §RP3.11, landed 2026-09-03 after the flip, and §RP3.10, still
open — block §RP5.2, not the flip, plan §0; see the "Next" line below.)

**RP4.1 landed 2026-09-03** — WP-RP4 is closed and G1.1's deliverable shipped:
`all_properties` is compared on the r4133 channel for every live non-`large`
case, so the **83 r4133-only** non-large cases get a property check for the
first time and the **313 non-`large` `both`** cases get their r4133 property
table compared (1 670 gating property walks over 151 782 elements per full run;
the 54 `kind=large*` `both` cases stay out on the plan's cost guard). Both
preconditions were discharged in the sub-step and before the flip: the **20
mixed echo rows** are narrowed per cell (66 measured spellings, mechanism (b)),
and the **eight** staged `property` entries landed with their accounting
(`RP3_LEDGERED`/`LEDGERED_RP3 = (6, 3, 6)`, `DECLARED_RP3 → (0, 0, 0)`, the
three `RP3_ROUTING` rows retired, the tripwire re-stated positively). The
per-cell accounting closes: **zero UNCLAIMED r4133 cells in scope**, 30 in-scope
`ledger-hit` cells, the 504 out-of-scope cells accounted per owner, and **zero**
ledger entries or pins born from the sub-step's own residual triage (kill
criterion ~15 — it did not fire). Full record in §RP4.1 above.
**RP3.11 landed 2026-09-03**, the first sub-step after the flip — the
`Save`/`Dump` re-serialization surface (plan §RP3.11, opened by the RP3.3 audit
settlement) is settled `KEEP_LIVE_PINNED` on both surfaces, with **seven**
re-compilability guards ported and **eleven** pins — four in the sub-step, three
more in its audit settlement the same day (`XfmrCode` and `DynEqPCE`, which
completed the declared union of both upstreams' `SaveWrite` overrides, plus the
sizing-property hoist for the five curve classes neither upstream guards);
§RP3.11 record above.
**Next: RP3.13** (PROPOSED, opened by RP3.11's own
round-trip measurement — the `ncim.rs:683` panic and the NCIM PV→PQ KCL gap;
user go-ahead required), then **RP3.10** (the reproduced `QMode=0` dispatch, also
user go-ahead) and then **WP-RP5** (RP5.1 operational docs, RP5.2 the closing
record); `GOLDEN_REBASE_PLAN.md` G3.4/G3.5, which waited on this flip, are
unblocked.
Alongside it, `GOLDEN_REBASE_PLAN.md` WP-G1, **opened** on branch `golden-g1`
(forked from `update` @ `4d3fc2d7`) — that branch carries G1.1's scratch census
only and **nothing was committed there**; the WP-G1 sub-steps that actually land
ride **`r4133-props`**, the branch holding the fail-on-stale
`population.lock.json` / `ledger.json` (single-branch lock discipline), which is
where G1.2 landed on 2026-08-29. WP-G0 (safety rails) and WP-G2 (bug-kernel
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
criterion re-armed — *delivered 2026-09-03*, and that kill criterion did **not**
fire (0 ledger entries and 0 pins born from RP4.1's own residual triage; §RP4.1
record above), so GOLDEN_REBASE G3.4/G3.5, which waited on RP4.1, are unblocked; PLAN_SEQUENCE rows
5a/5b added the same day. Execution of that plan started 2026-08-22 on
`r4133-props` (records below); the local-only census is no longer the single
copy of the evidence — its extracts are vendored by RP0.1 and the whole census
is re-derivable in ~1 min by RP0.2's `DSS_PROPS_CENSUS=1`. **G1.2 (ESPVLControl
deck) is DONE** (2026-08-29, on `r4133-props` — the branch that holds the
fail-on-stale lock/ledger; record below); G1.3d (discrete extras)
runs on — independent of G1.1. Queued behind GOLDEN_REBASE: `WASM_USERMODELS`
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

### GOLDEN_REBASE WP-G1 — records

> Plan: `GOLDEN_REBASE_PLAN.md` §WP-G1. G1.1 is handed to `R4133_PROPS_PLAN.md`
> RP4.1 (kill criterion fired, see §1) and **delivered by it on 2026-09-03** —
> the unmask shipped and RP4.1's own kill criterion did not fire. The sub-steps
> that do not depend on it
> land on `r4133-props`, the branch that currently holds the fail-on-stale
> `population.lock.json` / `ledger.json` (single-branch lock discipline).

- **G1.2** (2026-08-29) — **class `ESPVLControl` now has live corpus coverage**
  (it had none: no vendored deck and no family deck instantiated it, and
  `makeposseq_ctrl.dss:10` documents its absence there as deliberate).
  New deck `tests/corpus/controls/espvlcontrol/espvlcontrol.dss` (+ its manifest
  row in `tests/corpus/controls/manifest.json`): **six** ESPVLControls on one
  monitored line over a 12-step daily ramp, `kind=micro`, `n_steps=12`,
  `selected_elements=["*"]`, **8** probe specs, `compare_eventlog=true`,
  `compare_ctrlqueue=true`, `isolate=true`, `ad=off:unclassified-new-deck`
  (the last three added by the audit settlement below). Classification:
  **`engines: "both"` with the r4133 channel ledger-`skip`ped** — gated live on
  `capi_v0145` only, for a measured reason (below). Not `expect_solve_abort`:
  the pinned oracle compiles and solves it cleanly. That is a **third** outcome
  where the plan's acceptance enumerated two ("solves on both channels" /
  `expect_solve_abort` with a reason), so `GOLDEN_REBASE_PLAN.md` §G1.2 now
  carries the as-executed note that says which one landed and why — the G1.1
  precedent for annotating a sub-step rather than leaving its text reading as
  open instructions.
  - *What is LIVE-GATED, and what is only unit-pinned* (the distinction is
    **measured**, not asserted — see the settlement paragraph). Oracle-gated per
    step on `capi_v0145`: (1) `? ESPVLControl.scan.LocalControlWeights` moves
    `''` (pre-solve) → `'[ 1 1 1 1 1]'` (post-solve) **only because `Sample`
    ran** — the one Sample-derived observable the class has, produced by
    `MakeLocalControlList`'s no-list branch sweeping the whole class for
    *enabled* controls (`ESPVLControl.pas:609-611`, type-blind: local, system and
    untyped alike), so both the value and its **length** are two-sided; (2) the
    full 14-property table of all six controls (`compare_all_properties` is
    force-enabled for the `controls` family on the capi channel), including the
    `''` rendering of `plain`'s unset `Type`; (3) the no-op contract — `g1`/`g2`
    hold their input bases (`600`/`0`, `400`/`193.72884193514102`), the event log
    stays **empty** and the control queue stays **empty**. *Not* oracle-gated,
    on any channel: the redispatch arithmetic and `sys`'s whole named-list branch
    (below); their net is `elements/control/espvl_control/tests.rs` +
    `exec/tests/espvl_control.rs`, whose module doc now says so.
  - *All four instantiation shapes are present.* `sys` = SystemController over a
    **named** `LocalControlList` with weights `[3, 1]`; `scan` = SystemController
    with **no** list; `loc1`/`loc2` = LocalControllers whose PVSystem/Storage
    pointer lists are dead upstream (round-trip only); **`plain`** = no `type=`
    at all (`Ftype = 0`: `Type` dumps `''`, `MakeLocalControlList` returns false
    at its `Ftype` gate `ESPVLControl.pas:595`, `Sample` no-ops — yet the control
    still counts as *enabled* in `scan`'s type-blind sweep); `off` = disabled, so
    it never joins `scan`'s fleet of five. Feature sensitivity, all probe-visible
    and two-process byte-identical on the pinned oracle: enabling `off` →
    `'[ 1 1 1 1 1 1]'`; dropping `plain` → `'[ 1 1 1 1]'`; `plain
    type=SystemController` → `plain` itself renders `'[ 1 1 1 1 1]'`; `sys`
    weights `[5, 2]` → `'[ 5 2]'`; `scan type=LocalController` blanks its weights
    entirely (the `Ftype` gate).
  - *The `FkWLimit` crossing is real but buys no coverage.* The head power does
    cross the hardcoded `FkWLimit = 8000 kW` (`ESPVLControl.pas:321`; there is
    **no** `kWLimit` property, so `kvarLimit`'s default reads `4000`) on both
    signs — `PDiff` per step = −5887 / −4520 / −2960 / −1482 / −367 / **+754 /
    +1505 / +380** / −1111 / −2407 / −3879 / −5341 — but the redispatch it
    triggers is **unobservable** (settlement finding 2/7), so the crossing is
    documented, not claimed as gating power.
  - *No `MakePosSequence`.* The ESPVLControl override dereferences the always-NIL
    `ControlledElement` and aborts the oracle
    (`docs/wpg21_makeposseq_probes.md`); `makeposseq_ctrl.dss` is untouched.
  - **Measured upstream finding — r4133 cannot instantiate ESPVLControl at all.**
    The official EPRI r4133 DLL (Version 11.0.0.1) raises `#303 Access violation
    … Read of address 0x0` inside `ProcessCommand` on **every** `New
    espvlcontrol.<name>`, before any property is parsed. Isolated with
    `epri-worker`: `New espvlcontrol.a element=line.l1 terminal=1` on the deck's
    feeder → AV at offset `15440`, read of `0x0`; the bare `New espvlcontrol.a`
    on `clear; New circuit.min basekv=12.47 phases=3 bus1=src` → AV at offset
    `8F3F2D`, read of `0x70`. Not deck-specific, not a port issue: the port and
    the pinned 0.14.5 oracle both build and sample the class. Ledgered as
    `r4133-espvlcontrol-uninstantiable` (`kind=skip`, channel `r4133`) under the
    new cause `epri-espvlcontrol-uninstantiable`; the entry is HIT by the gate.
  - *Lock delta.* `population.lock.json` regenerated: `family_counts.controls`
    **105 → 106**, one new row — after the settlement
    `kind=micro steps=12 sel=1 mm=0 probes=8 vars=0 evlog=1 ctrlq=1 props=0 …
    isolate=1 defer=0
    ledger=r4133:r4133-espvlcontrol-uninstantiable@69c59d7407db83f4`. Ledger now
    40 entries / 26 causes (was 39 / 25).
  - *Corpus population, and a stale count G1.2 inherited.* The gate's walked
    population is **522 → 523 cases** (519 solvable; the 4 abort-by-design are
    unchanged) — growth, not a shrink, and **measured** rather than derived:
    `DSS_GATE_ONLY=espvlcontrol` reports `kept 1/523`, the scheduler's own
    `before` count over the union of the four manifests
    (`scheduler.rs::build_unified_cases`; 294 `solvable_now` + 53 `asymmetric` +
    106 `controls` + 70 `modes`). `CLAUDE.md` and `TESTING.md` both said
    **521 / 517** and were therefore *two* behind, not one: the other case is
    RP1.2's single-phase `asymmetric:autotrans/autotrans_xfmrcode.dss`
    (`8a221016`, asymmetric 52 → 53), which regenerated the lock without
    updating the two prose counts. Both files are corrected to **523 / 519**
    here — the G2.5 precedent, which updated the same two sentences when the
    population went 520 → 521. `TESTING.md`'s two other current-state count
    paragraphs were stale the same way and are re-measured in the same pass, so
    the file does not contradict itself: the corpus-gate section's per-family
    split (`293/47/105/69 = 514` → **294/53/106/70 = 523**, with the `engines`
    split now stated in full — 367 `both` / 59 `capi_v0145` / 97 `r4133`), and
    the ledger's "current contents" (36 entries / 23 causes → **40 / 26**:
    `skip` 4 → 5 by this sub-step, `capi_v0145 divergence` 5 → 8 by RP3.5's one
    and RP3.6(a)'s two). Every one of those numbers is counted off the shipped
    manifests and `ledger.json`, not carried forward. Not touched: the dated
    measurements that quote
    521 (the RP0.1/RP0.2 census population, the vendored
    `props_r4133/README.md`, the escape register's `n/520` blast radii) — those
    record what a walk measured on a given day and are frozen by convention.
  - *`linemedium` mapping (the sub-step's parenthetical).* `linemedium` is a
    **props-golden scenario name, not a class** — no deck is owed for it. Its
    subject matter (`Line.l1` with `EpsRMedium` / `HeightOffset` / `HeightUnit`)
    is already live-gated by `tests/corpus/modes/upgrade/upgrade_linecs_epsrmedium.dss`
    and `tests/corpus/modes/upgrade/upgrade_linecs_heightoffset.dss`. G3.1 carries
    this mapping into `TWINS.md`.
  - **Audit settlement (2026-08-29).** Two auditors, **11 findings** (two of them
    the same defect reported twice, so **10 distinct**): **10 fixed / 0 recorded
    unfixed / 0 refuted** — every finding held up under measurement, and the
    duplicate was fixed once. All eleven are minor; none moved a number the port
    computes. Settled, each against its own evidence:
    1. **Solving the deck corrupts the pinned-oracle process** — CONFIRMED and
       reproduced in a scratch dss-python 0.15.7 process: `compile` the deck,
       solve **once**, then `compile` any deck ⇒ `NumCircuits = 0`,
       `Error.Number = 0`, every later call `(#8888) There is no active circuit!`.
       Discriminated in seven runs: no solve → clean; `expcontrol_basic` in the
       same slot → clean; all controls disabled → clean; **only** the Local
       Controllers enabled → clean; `sys` alone with `kWBand = 1e9` (redispatch
       provably never fires) → **still broken**. So it is the *System Controller*
       `Sample`/`MakeLocalControlList` path, not the type-confused `kWBase`
       write. Today's gate is safe only by two defaults that are not this case's
       contract (`oracle_server.py::run_case` issues a top-level `clear` before
       `Compile` — measured to heal it — and `engines.rs::recycle_after()`
       defaults to one fresh worker per case, overridable via
       `DSS_GATE_RECYCLE_AFTER`). Fixed: the manifest row now carries
       **`isolate: true`** (the flag exists for exactly a proven
       worker-state-contamination case) with the mechanism in its `note`, and the
       "proven no-op" wording is narrowed everywhere it appeared — manifest note,
       deck header, this record, and `elements/control/espvl_control/mod.rs` —
       to *no effect on any observable state of the compiled deck, while the
       upstream class's `Sample` corrupts the host process's global state*.
    2. **The `FkWLimit` band crossing has zero gating power** — CONFIRMED by
       mutation: two 12-step oracle runs of the deck, one as-is and one with
       `kWBand = 1e9` on `sys`+`scan`, snapshotting **405 cells per step** (every
       property of every element + all bus voltages + `Iterations` +
       `ControlIterations` + event log + control queue) differ in **24 cells, all
       of them the mutated `kWBand` cell itself**; two independent base runs were
       byte-identical. So `PDiff`/`HalfkWBand`, the weights, `TotalWeight`, the
       `Max(1.0, …)` floor **and** `sys`'s entire named-list branch (its `'[ 3 1]'`
       is the parse round-trip, identical pre- and post-solve) are invisible on
       every channel. No coverage is actually missing — they are pinned by
       `espvl_control/tests.rs::system_controller_named_list_respects_weights` /
       `_floors_at_one` / `_in_band_does_nothing` — so the fix is honesty, not new
       tests: the manifest note, the deck header and this record now separate
       oracle-gated coverage from unit-pinned coverage explicitly. (Same defect as
       finding 7, reported by both auditors; fixed once.)
    3. **r4133's 12th `ESPVLControl` property `Forecast` was recorded nowhere** —
       CONFIRMED against the source:
       `.inputs/electricdss-code-r4133-trunk/Version8/Source/Controls/ESPVLControl.pas:133`
       (`NumPropsThisClass = 12`) and `:178` (`PropertyName^[12] := 'Forecast'`),
       against 11 in the pinned 0.14.5 and `NUM_PROPS = 14` in the port's
       `class_props`. Fixed: a standing open follow-up now names it, including why
       the R4133_PROPS census can never surface it (finding 1's sibling — r4133
       cannot build the class at all).
    4. **`ControlIterations` was presented as gated** — CONFIRMED: nothing in
       `tests/harness/`, `tests/corpus_gate/`, `dss-epri/src/capture.rs` or
       `tools/golden/*.py` compares it; the only compared iteration count is the
       power-flow `sol.Iterations` (`oracle_server.py:430`). Fixed twice over —
       the claim is corrected here, **and** the observable proxy is now genuinely
       compared (finding 6).
    5. **`ledger.json` lost its trailing newline** — CONFIRMED at the byte level
       (`git cat-file -p 727d2355:tests/corpus/ledger.json` ends `…]\n}`, where
       `bb467974`'s ended `…]\n}\n`, and every sibling manifest ends with one).
       Fixed: newline restored, so the next hand-edit of this fail-on-stale file
       no longer carries a spurious closing-brace hunk. (Reported by both
       auditors — finding 10 is the same defect.)
    6. **The control queue was not compared** — CONFIRMED: `runner.rs:589` only
       fetches the queue when `compare_ctrlqueue` is set, and the request sent
       `"ctrlqueue": false`. Fixed: the row now sets `compare_ctrlqueue: true`.
       Measured first, so it is a real assertion and not a rubber stamp — the
       oracle's queue is `['No events']` (⇒ the normalized empty list) at all 12
       steps, with `ControlIterations = 1` throughout, and the filtered gate is
       green with the comparison on. A port that queued a no-op action now fails
       the corpus gate, not merely the in-crate exec test.
    7. Same defect as finding 2 (the other auditor's wording) — fixed there.
    8. **`exec/tests/espvl_control.rs:1` still claimed "no corpus deck exists for
       this class"** — CONFIRMED by reading it, and it was the last such claim in
       `crates/dss-core/src`. Fixed: the module doc now names the deck and states
       the split of duties, with an explicit instruction not to thin these tests
       on the grounds that a corpus deck exists — they are the *only* net for
       everything finding 2 proved invisible.
    9. **The fourth instantiation shape was missing while the header advertised
       "three of the four"** — CONFIRMED. Fixed: `New espvlcontrol.plain
       element=line.l1 terminal=1` (no `type=`) joins the deck with a `Type` /
       `enabled` / `LocalControlList` / `LocalControlWeights` probe. Measured on
       the oracle: `plain` renders `Type = ''` and `LocalControlWeights = ''`
       before *and* after the solve (the `Ftype` no-op, now live-gated instead of
       mock-only), and `scan`'s fleet becomes `'[ 1 1 1 1 1]'` — the audit's point
       that this makes the fleet count a two-sided assertion. Both new
       sensitivities re-measured (see the shapes bullet).
    10. Duplicate of finding 5 — fixed there.
    11. **`kind=skip` ledger entries self-hit, so the r4133 blackout can never be
       reported stale** — CONFIRMED in `corpus_gate/ledger.rs:272-283`
       (`channel_is_skipped` bumps `hits` unconditionally whenever the case
       dispatches), and correctly identified as pre-existing infrastructure shared
       with the four `r4133-*-303` skips, not something G1.2 introduced. Fixed as
       far as this sub-step's scope allows: the entry's `source` now carries an
       explicit **re-measure obligation** (re-probe with
       `DSS_GATE_SEED_LEDGER=1 DSS_GATE_SEED_ONLY=espvlcontrol` whenever the
       r4133 DLL is re-vendored, and delete the entry if the constructor is
       fixed), and the general limitation is a standing open follow-up covering
       all five skip entries.

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
    out of scope. — **Correction (2026-09-03, RP3.12):** the scope reading
    stands (still 0 in-scope cells) and the last clause does not: RP3.9's P0 open
    item opened §RP3.12, which decomposed the "different landed tap" — r4133's
    `RegControl` reaches its controlled element through an unchecked
    `TTransfObj(ControlledElement)` cast (`RegControl.pas:926`, `:1026`, `:1296`,
    `:1370`, `:1479`) over `TAutoTransObj = class(TPDElement)`
    (`AutoTrans.pas:88`), so `TapIncrement` reads the winding's `MaxTap` and
    r4133 never taps an AutoTrans at all (0 event-log lines on all four decks).
    `UPSTREAM_BUG`, never reproduced; the 8 `controls:autotrans/*` spellings are
    `Owner::Rp312`'s and the ninth (`modes:makeposseq`) stays RP3.9's. The same
    correction is dated into `tests/corpus/props_r4133/README.md` §RP1.2.
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
    §WP-RP3; tier `opus-high+`, the RP3.1–RP3.4 row). **All three blocked
    RP4.1** (plan §0: "RP4.1 starts only after every RP1–RP3 sub-step is landed,
    including any RP3.5+ sub-step RP2.2's triage opens"); **all three have since
    landed — RP3.5 on 2026-08-28, RP3.6 on 2026-08-29, RP3.7 on 2026-09-02** —
    and the fourth sub-step this WP opened, RP3.8, landed 2026-09-02 too, so
    what still blocked RP4.1 from this WP was RP3.9 alone — and **RP3.9
    landed 2026-09-02** as well (27 pairs, all `PRECISION_ROUNDTRIP`; see the
    §RP3.9 record in §1), so this WP no longer blocks the unmask.
    - **RP3.5 — line length units lost by the matrix-branch merge. SETTLED
      2026-08-28 (`FIX`, both lanes; audit settled 2026-08-29) — see the §RP3.5
      record below.**
      `exec/reduce.rs` wrote `l.length_units = len_units_saved` and then ran the
      `RMATRIX/XMATRIX/CMATRIX` side effects, which call `reset_length_units`
      (`elements/pd/line/accessors.rs:478-486` → `code.rs:24-28`); r4133 does the
      two in the opposite order (`Line.pas:1627` saves, `:1721-1726`/`:1791-1796`
      re-edit *after* the impedance edit) and `MakePosSequence` re-appends
      `Units=` for the same reason (`:1596`). Two further divergences in the same
      routine, both from **both** oracles: `reset_length_units` also cleared
      `user_length_units`, which r4133 (`:2330`) and dss_capi 0.14.5 (`:2084`)
      both keep under the identical "in case of CIM export" comment; and the
      sym-branch `Length=`/`Units=` re-apply was nested inside the impedance arm
      (upstream runs it unconditionally), whose partner-is-switch half also
      emitted the parser-rejected token `Switch=1`. Evidence: `line.units` 3
      cells, **0 in scope**, and — corrected by the sub-step's probe — all 3 on
      `modes:reduce/midi_reduce.dss`, **none** on `reduce_mergeparallel`, whose
      merged line renders `km` on all three engines. Cost the plan text did not
      anticipate: the fix reds the **live** `capi_v0145` `all_properties` compare
      on `midi_reduce.dss`, so a capi ledger entry + cause +
      `population.lock.json` rewrite landed with it.
    - **RP3.6 — `switch=yes` must not clear the linecode flag. SETTLED
      2026-08-29 (`FIX`, both lanes; audit settled the same day) in two parts —
      (a) the switch arm and
      (b) the `FLineCodeSpecified`/`CondCode` split plus the CIM units
      back-fill, plus the settlement's `SpacingSpecified` split and r4133's
      eighth flag-clear site; see the three §RP3.6 records above.** r4133's arm
      (`Line.pas:694-700`) writes r1/x1/r0/x0/c1/c0/len as fields, kills geometry
      and spacing and resets the units, but leaves `FLineCodeSpecified` TRUE; the
      port calls `kill_line_code_specified()`
      (`elements/pd/line/accessors.rs:488-511`, following 0.14.5's
      `KillLineCodeSpecified`). Consequence beyond the render: the flag picks a
      different `FUnitsConvert` formula on a later `units=` (`:626-627`), and the
      decks that carry the cells
      (`ADiakoptics/EPRI_Ckt7-G/Torn_Circuit/zone_2/Branches.dss:93,:95,:479` and
      `zone_3/Branches.dss:161,:165`) put `units=m` after `Switch=True`.
      `StoCtrl_Current_PeakShave/Line.DSS` has the same shape but is `kind:
      large`, so it is never property-compared and carries none of them.
      Evidence: `line.linecode` **5 cells, all 5 in scope** — the
      only RP3.5+ pair the unmask will actually compare, so **RP4.1 breaks on it
      if RP3.6 does not land first**. r4133 is the authority; "capi does it" is
      not evidence (CLAUDE.md).
    - **RP3.7 — per-phase switch and relay state. SETTLED 2026-09-02 (`FIX`,
      both lanes, all three parts — (a), (a2) and (b); audit settled the same
      day, which also landed Relay's NIL-element render and opened
      `ORPHANED_GAPS.md` §1.16) — see the §RP3.7 record and its audit-settlement
      paragraph above.** (a) r4133 keeps
      `FPresentState`/`FNormalState : pStateArray` per phase
      (`SwtControl.pas:37-38`, `:299-305`), settable phase-by-phase from a quoted
      list (`:453-480`), each phase driving its own conductor (`:532-549`), and
      renders one token per controlled-element phase (`:589-610`); the port holds
      one scalar applied to the whole terminal
      (`elements/control/swt_control/accessors.rs:126-163`, `:270-276`,
      `mod.rs:2`). Every r4133 render the census saw is homogeneous, so **no
      value differs today** — a deck writing `state=(open, closed, closed)` would
      diverge in Y. **(a2), added by the audit settlement and probe-confirmed:**
      a `normal=` write on a **locked** SwtControl is refused by the port
      (`accessors.rs:151-155`, following 0.14.5's `ConditionalReadOnly`) and
      **applied** by r4133 — `InterpretSwitchState` exits early only for property
      names starting `'a'`/`'s'` (`SwtControl.pas:416-417`, comment "Only allowed
      to change normal state if locked") and property 6 is `'Normal'` (`:128`),
      so arm 6 (`:201-204`) reaches `set_NormalStates` (`:556-561`), which has no
      lock guard. Live r4133 DLL probe: under `lock=yes`, `normal=open` →
      `[open, open, open, ]` while `state=`/`action=` move nothing. The port's own
      Relay implements the r4133 rule and documents it
      (`relay/accessors.rs:416-420`), so this is a port bug, not a decision; the
      §D12 record in `docs/upgrade/DIVERGENCES.md` claimed the opposite and is
      corrected. Zero corpus exposure (no deck writes `normal=` under lock), so
      it is a *fix*, not an exclusion — RP3.7 lands it in both lanes with a pin
      and it may not be deferred to `ORPHANED_GAPS.md`. (b) The mirror on Relay:
      r4133 renders over the live
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
    `r=`-specified fault never allocates `Gmatrix` — `Create` nils it (`:411`)
    and the only writers are `DoGmatrix` (`:196-209`, Edit arm 6 at `:286`) and
    `MakeLike` (`:364-367`). (The audit settlement replaced the first draft's
    citation of the field comment `:76-77`, which is documentation, not
    evidence.) So `'()'` and the port's `'(0 )'` denote the **same** state, "no G
    matrix specified" — the unset-array render family of
    `generator.dynout`/`autotrans.bhcurrent`, with no port behavior to change.
    The 386 cells therefore stay value-skipped on both channels under the
    existing `SKIP_PROPS`/`SKIP_PROPS_BOTH_CHANNELS` row, whose comment now
    carries the verdict; no unmask (§1.1(e)), no new table row, and RP4.1 owes no
    ledger entry for it. What it *did* owe — and now has — is the expected-value
    pin CLAUDE.md requires beside any deliberate exclusion, since
    `both_channel_rows_stay_skipped_everywhere` only asserts the skip is
    configured and its sibling `generator.dynout` gets a tagged exclusion **plus**
    a pin: `harness/mod.rs::skip_props_disposition_tests::
    fault_gmatrix_renders_a_materialised_zero_matrix_when_unset` pins `'(0 )'` at
    1 phase, `'(0 |0 0 |0 0 0 )'` at 3, and the non-zero specified render as the
    discriminator.
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
    lie and the whole pair was re-typed to an `EnumSynonym` row. The two
    predicates are **incomparable, not nested** — a claim this record and the
    map's own doc first got wrong and the audit settlement corrected: the map is
    *narrower* on case-only differences outside the three named ordinals, and
    *wider* by exactly the 3 `'RAvg'`/`'avgrated'` cells (all 3 in scope) that
    the `CaseFold` row refused and whose refusal this sub-step deleted from
    `casefold_folds_case_and_the_two_trailing_blanks`. Those 3 folds are the
    point of the re-typing and are value-preserving by the live-getter argument;
    what would not be acceptable is folding them silently, which is why the count
    is now named at the map, here, and in the vendored README (which had it right
    all along). The scope growth is recorded as data
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
    into "claimed by the table" (checked per example row through the shipped
    predicate) and "routed with a citation", with every routing row live, citing
    a `.pas:` line, and owning one of the two admissible outcomes. **The split,
    stated over the whole 27-pair input** (the audit settlement's correction —
    this bullet and the commit subject `ab2bf041` quoted only the closed list's
    own 6/18 without saying so): `RP22_ROUTING` holds **20** rows (14 → RP2.3,
    6 → RP3.5+) and **7** pairs are claimed outright, i.e. 24 closed-list pairs
    = 18 routed + 6 claimed, plus the 3 `RP22_BEYOND_THE_CLOSED_LIST` pairs
    = 2 routed + 1 claimed.
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
  - **Audit settlement (2026-08-23, one follow-up commit — still zero engine
    change).** The two audits raised **8** raw findings (audit-code 1 major + 4
    minor, audit-tests 1 major + 2 minor). Two pairs are the same finding seen
    twice — the "the re-typed `voltage_curvex_ref` map is stricter" wording and
    the stale `EnumSynonym` row counts were each raised by both auditors — so
    the deduped set is **6** (2 major, 4 minor): **5 fixed, 1 recorded, 0
    refuted** (5 + 1 + 0 = 6 ✔).
    - **RECORDED, not fixed — `swtcontrol.normal` under `Lock`** (major,
      audit-code). The dossier routed `swtcontrol.normal`/`state` to RP3.7 on
      the per-phase question alone and missed a second, *live* divergence in the
      same Edit arm the brief required reading: the port refuses a `normal=`
      write while `Locked`, r4133 applies it. Verified against the source
      (`SwtControl.pas:416-417` guards only `'a'`/`'s'` property names; property
      6 is `'Normal'`, `:128`; `set_NormalStates`, `:556-561`, has no lock
      guard) **and** by a live probe on the vendored r4133 DLL (under
      `lock=yes`: `normal=open` → `[open, open, open, ]`; `state=`/`action=`
      move nothing). The fix is an engine change, which RP2.2's scope forbids,
      so it is recorded — `R4133_PROPS_PLAN.md` §RP3.7 **(a2)** with its
      acceptance, the `RP22_ROUTING` row comment, this record, and a correction
      to `docs/upgrade/DIVERGENCES.md` §D12, whose "matching 0.14.5/r4133"
      claim this refutes. Zero corpus exposure today.
    - **FIXED — the third `EnumSynonym` map shipped with no closed-set pin**
      (major, audit-tests; the RP2.1 major finding's shape, one level down).
      `SCAN_TYPE_SYNONYMS`/`SEQUENCE_TYPE_SYNONYMS` were pinned literally but
      `VOLTAGE_CURVEX_REF_SYNONYMS` only by identity, so an entry folding
      nothing in today's population (the auditor's mutation `("Rated","ravg")` —
      r4133 *parses* `'ravg'` to ordinal 2, `InvControl.pas:837`, against our
      ordinal-0 `'Rated'`) left the whole suite green. `enumsynonym_maps_are_
      injective` now pins all three maps literally.
    - **FIXED — injectivity was checked in one direction only** (minor,
      audit-code). The mirror collision (one of *our* spellings mapping to two
      r4133 tokens) is equally value-destroying and is now asserted in the same
      loop, with the kind's doc restated to promise both directions.
    - **FIXED — the "stricter" claim on the re-typed row** (minor, both
      auditors). The new map is *incomparable* with the `CaseFold` row, not
      narrower: it drops the open-ended case claim but adds the 3 `'RAvg'`/
      `'avgrated'` cells whose refusal this sub-step deleted from
      `casefold_folds_case_and_the_two_trailing_blanks`. Corrected at the map,
      in the test's own comment and in the bullet above; the vendored README had
      it right and is unchanged.
    - **FIXED — the `Fault.GMatrix` triage retired its obligation without a
      pin** (minor, audit-code), plus its `:76-77` citation (a field comment,
      not evidence). Both corrected above and at the `SKIP_PROPS` row; the new
      pin is `fault_gmatrix_renders_a_materialised_zero_matrix_when_unset`.
    - **FIXED — stale counts around the late-added fifth row** (minor, both
      auditors): `props_norm.rs`'s `EnumSynonym` doc ("landed four"), the new
      test's heading ("the four shipped rows"), the replay's
      `CLAIMED_ENUM_SYNONYM` failure message ("four bin-3 synonym rows"),
      `rp22_settled_every_pair_it_was_handed`'s doc ("four … and two", against
      its own seven-pair assertion) and this record's 6/18 routing split (true
      only of the closed list; the whole input is 7 claimed / 20 routed).
    - Gate re-run in full on the settled tree; no golden, lock, ledger, manifest
      or product-crate byte moved, and no test was deleted, `#[ignore]`d or
      loosened. Test count **7 274 → 7 318** (3 637 → **3 659** per lane, ×2
      lanes): the one new pin lands in each of the 22 test binaries that include
      the harness.

- **RP2.3** (2026-08-23) — the echo-exclusion table and its pins. **Zero engine
  change**: `tests/harness/*`, `crates/dss-core/tests/*`, the vendored
  `props_r4133/README.md` and docs. No golden byte, no `golden.lock.json`, no
  `population.lock.json`, no `ledger.json` entry (§1.1(e) staging), the r4133
  props live masks untouched, and `R4133_DISPLAY_FLOOR` still `None` (RP2.4's
  slot). Run as parts A (dossier) / B1 (tables) / B2 (pins, census, docs, the
  one commit).
  - **THE KILL CRITERION FIRED — six rows, and the user ruled both halves.**
    Part A read all 86 pairs of the declared `Owner::Rp23` bucket against the
    r4133 source and found six that cannot be cited to an echo site. Reported
    per the plan's stop-and-consult gate; the user chose (AskUserQuestion,
    2026-08-23):
    - **R1, the five `SilentReadOnly` surfaces** — `indmach012.pf` and
      `storagecontroller.kwhtotal`/`kwtotal`/`kwhactual`/`kwactual`, **1 064
      cells / 772 in scope**. r4133 renders each one LIVE
      (`Version8/Source/PCElements/IndMach012.pas:1790`;
      `Controls/StorageController.pas:991-994` → `GetkWhTotal`/`GetkWTotal`/
      `GetkWhActual`/`GetkWActual`, declared `:136-139`); the port answers `''`
      only because dss_capi 0.14.5 flags them `[SilentReadOnly, ReadByFunction]`
      and the port reproduces that at one class-agnostic gate
      (`obj/props/class_props/value.rs:21-27`). Under the standing 2026-08-02
      policy the 0.14.5 convention yields, so the fix is an ENGINE change — and
      RP2.3 may not touch a product crate. They therefore take **no echo row**
      and are re-routed loudly: `props_r4133_replay::RP38_ROUTING` +
      `DECLARED_RP38 = (181, 5, 181)`, with a new plan **§RP3.8** that **blocks
      RP4.1** (§0's list read RP3.5/3.6/3.7 **+ 3.8**; RP3.5 landed 2026-08-28,
      leaving 3.6/3.7 + 3.8). **Landed 2026-09-02** — the engine renders all
      five live, `RP38_ROUTING` is now `RP38_SUPERSEDED` and `DECLARED_RP38` is
      `(0, 0, 0)`; see the §RP3.8 record in §1.
    - **R2, `generator.d`** — the plan, the vendored README §RP1.1 and
      `BIN7_ECHO_SUPPLEMENT`'s comment all called it an echo "from a frozen
      default whose field says 1.0". Wrong mechanism: the field that is 1.0 is
      `GenVars.D`; the property's field is `GenVars.Dpu`, which `Edit` writes
      (`generator.pas:669`) and `InitPropertyValues` snapshots (`:2585`), and
      which `Create` **never** touches (it sets `D := 1.0`, `:969`). So r4133's
      `'0'` is its own live value, and `InitStateVars` then computes
      `D := Dpu*kVArating*1000/w0 = 0` (`:2710`) — every generator that does not
      set `D=` runs dynamics **undamped**, against the property's documented
      default (`:467`). An upstream initialisation bug; 0.14.5 fixed it
      (`Dpu := 1.0`, `src/PCElements/Generator.pas:1006`) and the port follows.
      Landed as `LiveSemanticsDiffer` + pin + upstream report
      `investigations/to_opendss/42-generator-dpu-never-initialized.md`
      (gitignored, not in the commit); the three wrong texts are corrected.
  - **The table: 81 rows** (86 − the 5 re-routes), each with an r4133
    `unit:line` citation, a measured `cells` column read back against
    `bins.tsv`/the README, and a typed witness. Split **50** `EchoDefault` /
    **7** `EchoParse` / **14** `EmptyCollectionRender` / **10**
    `LiveSemanticsDiffer`.
    `EmptyCollectionRender` is the **one** new category the ruling sanctioned:
    14 pairs where r4133's getter arm is LIVE and merely renders the other
    empty-collection convention (`'[]'` from a loop over zero points, `'()'`
    from a paren wrapper, `''` from an early exit) — all three original tags
    would have been a lie there, and the kill criterion exists to stop exactly
    that. The count is above the plan's ~50–70 estimate because the bucket the
    RP2.1 replay had already declared by name holds 86 pairs (bin 5's 43 + bin
    7's 12 + bin 1's 9 + six bin-2-labelled pairs whose `''`-echo cells the chain
    routes here + bin 3's 3 + bin 4's 2 + the supplement's 11) — measured scope,
    not creep.
  - **The seam.** `PropsPolicy::echo_excluded` sits in `compare_prop_lists`
    **after** the normalization seam and immediately before
    `assert_value_matches_tol`; `continue` drops the VALUE assert only — name
    and index order are still walked, exactly `SKIP_PROPS`' shape. r4133 only:
    the capi arm is `false` for everything and is pinned
    (`the_capi_channel_never_excludes`, plus the real-comparator four-corner
    test `an_echo_row_drops_only_its_own_value_only_on_r4133`). The order is the
    per-cell **attribution**: **20 pairs hold both** a normalization row and an
    echo row, and 135 of their example rows are claimed by the typed rule first
    (`MULTI_LINK_ROWS`). It is **not** a per-cell mask — the exclusion is asked
    per pair, so it covers a mixed pair's refused cells too (the audit
    settlement's recorded finding, below).
  - **Nine typed rows landed FIRST** (part A finding F4, ruling R3):
    `load.yearly`,
    `reactor.bus2`, `invcontrol.monvoltagecalc` (`CaseFold`) and `line.wires`,
    `load.zipv`, `generator.userdata`, `storage.dynadata`,
    `storagecontroller.seasontargets`/`seasontargetslow` (`ArrayForm`) — **6 446
    cells / 6 370 in scope**, which the census then attributes to their rule
    (this record first said they "stay compared"; they do not — see the
    settlement). The tenth measured candidate,
    `swtcontrol.action`, deliberately took **no** row: one foldable spelling, 6
    cells, **0 in scope**, and the row would have loosened a live RP2.2 pin
    (`the_pairs_routed_elsewhere_have_no_row`) for zero live coverage.
  - **Witnesses — 20 expected-value pins** (**29 covering 63 rows** after the
    settlement below), new file
    `crates/dss-core/tests/props_r4133_pins.rs`, covering the **32** rows whose
    value capi does not (or must not only) hold. **21 are pin-only**: the seven
    Recloser/Relay pairs (capi skips those elements whole), ten
    RegControl/Line/Transformer/AutoTrans pairs dropped by
    `PROPS_015X`/`SKIP_PROPS` (five RegControl props, `line.conductors`, both
    `transformer.bh*`, both `autotrans.bh*` — probe-confirmed: the pinned oracle
    reports 35 RegControl property names, the port 39), `windgen.dynout` (no
    capi-gating case holds a WindGen), and the three whose echo cells all sit on
    capi-only cases (`fault.bus2`, `line.spacing`, `swtcontrol.action`). The
    other **11 have a capi witness *and* a pin**: every `LiveSemanticsDiffer` row
    (capi is a numeric oracle, so it cannot carry "ours is the right value") plus
    `energymeter.peakcurrent`, which the plan names. Each compiles the deck
    the census flagged, asserts the port's live `?` render literally and adds a
    discriminating second reading (a deck or an edit where the same getter
    answers otherwise). Own binary on purpose — in `harness/` each pin would
    re-solve its deck in all 22 harness binaries.
    `every_echo_row_pin_is_a_test_that_exists` reads the names back both ways;
    a `DeckDirGuard` sweeps the `Export`/`show eventlog` artifacts two of the
    decks write into the vendored tree (measured: `git status` clean after).
  - **Liveness, both ways.** Offline, in the replay:
    `every_echo_row_claims_at_least_one_example_row` (all 81 exclude ≥1 example
    row) and `every_echo_row_matches_its_cited_evidence` (each `cells` column
    read back against the file it cites). Live: `ECHO_VISITS`/`ECHO_HITS` per row +
    `assert_echo_rows_are_live()` in the gate epilogue, **silent-when-dormant**
    (part A finding F5) with a closed two-row exemption list,
    `ECHO_ROWS_WITH_NO_IN_SCOPE_CELL` = `fault.bus2`, `line.spacing` — both
    measured at 0 in-scope cells, and `fault.bus2` is the one that makes the
    exemption necessary rather than tidy (its pair *is* compared on r4133, so
    its row will be visited and can never hit).
  - **Locks moved, each delta measured** (offline replay + an independent
    Python replication that first reproduced the pre-change baseline exactly):
    `NORM_ROWS` 161→**170**, `CLAIMED_CASE_FOLD` 440→**528**,
    `CLAIMED_ARRAY_FORM` 192→**203**, new `CLAIMED_NORMALIZATION` **854**,
    `CLAIMED_ECHO` 0→**170**, `CLAIMED_TOTAL` **1 024**, `MULTI_LINK_ROWS`
    0→**135** (20 pairs, split per pair), `CLAIMED_TOTAL_LIVE` 756→**855**,
    `ECHO_ROWS` 0→**81** + four per-category locks, `DECLARED_RP23`
    (450, 86, 449)→**(0, 0, 0)** and new `DECLARED_RP38` **(181, 5, 181)**.
    450 = 99 norm + 170 echo + 181 RP3.8, exactly; the RP2.4/RP3/RP3.5/
    OutOfScope triples did not move. (The settlement below re-splits the same 450
    as 99 + **169** echo + 181 + **1** carved out to RP2.4, and moves
    `DECLARED_RP24` with it.)
    `the_echo_table_and_the_display_floor_claim_nothing_yet` was **replaced by a
    successor that asserts strictly more** —
    `the_echo_table_claims_only_its_cited_pairs_and_the_floor_claims_nothing`
    (the floor is still inert over the same walk; the echo link claims ONLY
    cited pairs; the five re-routed pairs stay compared).
  - **Full claims census at HEAD** (`DSS_PROPS_CENSUS=claims`, 439 cases × 2
    channels, 1 060 165 rows, 65.2 s; error counts at the recorded baselines —
    **5** r4133 / **22** capi, so the run is complete, not short):
    `echo-row` **488 018 cells / 468 044 in scope**, 170 spellings, all **81**
    pairs (**488 017 / 169 spellings** after the settlement's carve-out, same
    in-scope count); `CaseFold` 99 023/95 270 (+6 047/+6 039), `ArrayForm`
    122 756/118 744 (+399/+331), `BoolFold` and `EnumSynonym` unmoved;
    claimed **1 008 935 / 966 790** over **231 distinct** pairs (the `pairs`
    column stops being summable this sub-step — 20 pairs carry two
    dispositions); `UNCLAIMED` 545 568 → **51 104** (521 841 → **47 427**), 103
    pairs (**51 105 / 104 pairs** after the settlement, in-scope unchanged).
    capi claims **0** on every r4133 disposition, its 11 `ledger-hit`s
    intact. Term-by-term reconciliation: census claimed spellings 1 025 = the
    replay's `CLAIMED_TOTAL` 1 024 + the one `LIVE_ONLY_SPELLINGS` row (the same
    +1 drift RP2.1/RP2.2 recorded — 1 024 = 1 023 + 1 after the settlement);
    norm-link 855 = `CLAIMED_TOTAL_LIVE`;
    echo 170 = `CLAIMED_ECHO`; residual bin 5 = 1 069 cells = RP3.8's 1 064 +
    `line.linecode`'s 5 (RP3.6).
  - **Hand-offs consumed:** the `energymeter.peakcurrent` third spelling (the
    pair carries BOTH kinds of row and the chain discriminates per cell — 518
    `ArrayForm`, 6 echo), the two zero `CLAIMED_` locks and `MULTI_LINK_ROWS`
    (all three moved deliberately, above), and `generator.dynout` (RP2.2's fixed
    `LiveSemanticsDiffer` verdict, now with its pin).
  - **New hand-offs:** **§RP3.8** (engine sub-step, blocks RP4.1; scope, probe
    procedure, capi-side exclusion and golden-exposure measurement authored in
    the plan); **RP4.1 must narrow the 20 mixed echo rows per cell BEFORE the
    unmask** (the settlement's recorded finding, written into plan §RP4.1 as a
    precondition — afterwards the same gap is a green gate that proves less than
    it says); **RP4.1 must check the zero-damping ledger** — any r4133-gated
    deck running generator dynamics without `D=` may already carry the R2 bug in
    its triage; and **F7's deferral** — `Capacitor`/`Reactor` `FaultRate`
    (1 729 cells) and the three `CMatrix`/`RMatrix`/`XMatrix` rows stay in
    `SKIP_PROPS_BOTH_CHANNELS` with no echo row, because a row would be dead
    offline (no example row) and dead live (the skip keeps the compare away);
    their unmask + row + pin is an RP4.1-or-later decision that must land in one
    commit. Both `SKIP_PROPS` comments record it.
  - **Goldens measured, not assumed:** `git status` after the full gate shows
    only the sub-step's own files; no golden, lock, ledger, manifest or frozen
    census extract moved, and the stray `Export`/eventlog artifacts the pinned
    decks drop into `tests/corpus/electricdss-tst/` are swept by the pin file's
    own guard (verified by `git status` and `git clean -f -- tests/corpus`).
  - Full five-command gate green on the final tree, both lanes; no test was
    deleted, `#[ignore]`d or loosened — the one test that was *replaced*
    (`the_echo_table_and_the_display_floor_claim_nothing_yet`) is replaced by a
    strictly stronger successor. Test count **7 318 → 7 812** (3 659 →
    **3 906** per lane, ×2 lanes): 22 in the new pin binary (20 pins + the deck
    guard's two self-tests), 1 in the replay, and the rest are the harness-side
    guards, which land in each of the 22 test binaries that include the harness.
  - **AUDIT SETTLEMENT** (2026-08-23, follow-up commit, still **zero engine
    change**). The two audits (`audit-code`, `audit-tests`) raised **7 raw
    findings**, none of them a duplicate, so **7 deduped**: **6 fixed + 1
    recorded with an owner + 0 refuted = 7**. Every one was confirmed against
    the r4133 source, the census or a mutation before it was acted on.
    - **RECORDED (major, audit-tests) — the nine typed rows keep no cell inside
      the LIVE compare.** True as stated: `echo_excluded_r4133` answers on row
      presence, so on the 20 mixed pairs the exclusion also drops the cells the
      typed rule REFUSED, and the mitigation this record and three documents
      credited to the chain order ("keeping 6 446 cells inside the value
      compare") is offline attribution only. Reproduced here through the real
      comparator: `Load.Yearly` `'day'` vs `'night'` passes on r4133 and fails on
      capi. What the order really buys — the census disposition and each row's
      liveness hit — is now what the docs say (props_norm's module doc and the
      `PROPS_ECHO_R4133`/`PropsPolicy::echo_excluded` docs, the plan §RP2.3
      as-executed and §1.1, the vendored README, this record), the shipped
      behaviour is pinned by
      `props_policy_tests::a_mixed_pairs_echo_row_masks_the_cells_its_rule_refuses`
      (written to be *replaced* by the narrowing, not deleted), the offline test
      that carried the false name is renamed
      (`…masks_only_what_the_typed_rules_leave` →
      `the_typed_rules_and_the_echo_rows_split_their_shared_pairs_offline`), and
      the narrowing itself — per-cell rows for those 20 pairs — is an explicit
      **RP4.1 precondition** in the plan, with `ECHO_CARVE_OUTS` as the
      mechanism. Not fixed here because it re-opens RP2.3's row shape, which
      plan §1.2 fixes, and would move every count lock plus a full re-census.
    - **FIXED (minor, audit-code) — `reactor.kvar`'s row masked a cell that is
      not an echo.** 606 of the pair's 607 census cells are the frozen `'1200'`
      the row cites; the 607th (`'66.6666666666667'` vs `'66.667'`, 5.0e-06) is
      r4133's own LIVE value, because `TReactorObj.MakePosSequence` builds
      `Format(' kV=%-.5g kvar=%-.5g')` and runs it back through
      `Parser.CmdString`/`Edit` (`Version8/Source/PDElements/Reactor.pas:
      1145-1201`) — dss_capi 0.14.5 uses `SetDouble` there and never a string,
      which is why the capi channel compares that cell and the port matches it.
      Narrowed, not re-explained: new `props_norm::ECHO_CARVE_OUTS` (exact
      `(rust, r4133)` match, never a shape heuristic) takes the cell out of the
      row, and `props_r4133_replay::ECHO_CARVE_OUT_ROUTING` declares it to
      **RP2.4** — where the same round-trip's `reactor.kv` already sits
      (`bins.tsv` bin 6, 5.85e-06). Locks: `CLAIMED_ECHO` 170→**169**,
      `CLAIMED_TOTAL` 1 024→**1 023**, `DECLARED_RP24` (2 100, 70, 2 020)→
      **(2 101, 71, 2 021)**. Re-measured census: `echo-row` 488 018→**488 017**
      (169 spellings), `UNCLAIMED` 51 104→**51 105** (104 pairs) — the cell is on
      a capi-only case, so **no in-scope number moves**.
    - **FIXED (minor, audit-code) — `RP38_ROUTING` cited `IndMach012.pas:1789`
      and `Power[1,ActorID]`.** :1789 is arm 4 (`kWBase`); arm 5 is `:1790` and
      reads `Power[1,ActiveActor]`. Corrected there and in
      `the_silent_readonly_pairs_have_no_echo_row`'s doc; the plan, this file and
      the README already said :1790, so the two records now agree.
    - **FIXED (minor, audit-code) — the `EmptyCollectionRender` doc claimed too
      much.** "The two sides mean the SAME thing… an empty render carries no
      value to preserve" holds for 12 of the 14 rows, not for
      `storagecontroller.seasontargets`/`seasontargetslow`: with `Seasons = 1`
      r4133's `ReturnSeasonTarget` exits before emitting anything
      (`StorageController.pas:2445-2449`) while the port prints the live target,
      237 cells each. The doc now says what all 14 share and names the two
      exceptions with their Pascal, including why the tag still fits (the value
      itself is identical — `SeasonTargets[0] := FkWTarget`, `:883-884`, read
      back at `:1470`).
    - **FIXED (minor, audit-tests) — the seam ORDER was untested where it
      ships.** It was pinned in the two OFFLINE copies of the chain
      (`claim_value`, `Link::ORDER`) and nowhere at `compare_prop_lists`, so
      swapping the two lines there left the whole suite green.
      `props_policy_tests::the_normalization_seam_runs_before_the_exclusion_on_a_mixed_pair`
      drives the real comparator on `reactor.bus2` and asserts the row's
      visit/hit deltas; mutation-checked (the swap reds it, `Some((0,0))` vs
      `Some((1,1))`). Two new `props_norm` accessors (`norm_counters`,
      `echo_counters`) make the counters readable; the two tests use different
      mixed pairs and each leaves `hits > 0` on every row it touches, because
      `cargo test` runs them concurrently with the gate's own
      `assert_*_rows_are_live()` epilogue (measured: the first draft used
      `Load.Daily`, which HAS a `CaseFold` row, and reddened the gate).
    - **FIXED (minor, audit-tests) — two unpinned exemption lists.**
      `ECHO_ROWS_WITH_NO_IN_SCOPE_CELL` (disarms fail-on-stale per row) and the
      replay's `NOT_A_PIN` (disarms the "a witness names a real `#[test]`"
      guard) were iterated as-is, so a third entry would have silently switched
      the guard off for another row. Both are now pinned literally
      (`the_dormant_row_exemption_list_is_pinned`,
      `the_non_pin_exemption_list_is_pinned`), `NOT_A_PIN` hoisted out of the
      test body to a documented const; mutation-checked (a third entry in either
      reds its test).
    - **FIXED (minor, audit-tests) — `Capi(n)` cannot witness cells on
      `engines: "r4133"` cases.** Measured with the full claims census crossed
      against each case's `engines` flag (97 of the 439 walked cases are
      r4133-only): **57 of the 81 rows mask 34 969 such cells**, and 31 of them
      carried a capi-only witness — the exact rule plan §1.2 mechanic (c) states
      ("r4133-only classes/**cases**") and part B2 applied by hand to
      `swtcontrol.action` alone. The population is now a const
      (`ECHO_ROWS_ON_R4133_ONLY_CASES`, sorted, count- and cell-locked) with a
      test that every listed pair names a pin, and the 31 rows got one: **nine
      new pins** (20 → 29 names, 32 → 63 rows), each compiling a deck the census
      named for that pair — `der_amp_limits_render_the_live_sentinel_and_gain`,
      `der_user_model_arrays_render_empty_when_unset`,
      `energymeter_action_and_capcontrol_reset_render_no_pending_command`,
      `fuse_switchedobj_defaults_to_the_monitored_element`,
      `invcontrol_defaults_render_the_live_values`,
      `load_zipv_renders_the_live_seven_element_vector`,
      `pd_element_perm_and_repair_render_the_live_ratings`,
      `reactor_kvar_renders_the_live_rating`,
      `storagecontroller_seasontargets_render_the_live_targets` — plus
      `line_conductors_renders_the_live_conductor_list` extended to `wires`/
      `cncables`/`tscables` (6 231 exposed cells each). All but two carry a
      discriminating second reading; the two that cannot (`energymeter.action`,
      `capcontrol.reset`) are one-shot COMMANDS whose empty re-read is itself the
      pinned fact, asserted before and after the command. The other half of the
      finding — nothing checked that a `Capi` witness names a pair capi can even
      compare — is closed by
      `a_capi_witness_is_a_pair_the_capi_channel_can_compare` (no `Capi` row may
      collide with `SKIP_PROPS`, `PROPS_015X` or the whole-element skip).
    - **Scope kept:** zero product-crate bytes, no golden / lock / ledger /
      manifest / frozen-extract byte, `R4133_DISPLAY_FLOOR` still `None`, the
      §1.1(e) masks untouched, no tolerance anywhere. Census artifacts
      (`tmp/props_census.json`, `tmp/props_census/`, `tmp/`) deleted by name;
      `git status` clean before the commit. Full five-command gate green on the
      final tree, both lanes; test count **7 812 → 8 098** (3 906 → **4 049** per
      lane): nine pins, two replay guards and six harness-side guards that land
      in each of the 22 binaries including the harness. No test deleted,
      `#[ignore]`d or loosened; the one renamed test asserts the same thing under
      a name that describes it.

- **RP2.4** (2026-08-23) — **the r4133 props display floor; WP-RP2 closes with
  it.** **Zero engine change**: four test files + docs, zero product-crate
  bytes, no golden / lock / ledger / manifest / frozen-extract byte, no r4133
  mask moved, no `Tolerances` field or tier touched.
  - **The derivation is a measurement, not the plan's paragraph.** Scored every
    example row of `examples_full.txt` + `examples_supplement.txt` (**3 454
    rows** = every distinct `(rust, r4133)` spelling of every census pair, so
    the worst *spelling* IS the worst *cell*) with the shipped metric
    `props_norm::display_rel`. Worst cell the floor claims **6.431124e-05**
    (`load.pf` `'0.747651914485831'` vs `'0.7477'`, 33 cells, in scope);
    **floor = `2e-4`**, 3.110× above it; nearest row above the band
    **1.374769e-03** (`storagecontroller.kwneed`, 6.874× over the floor);
    nearest genuine value jump **4.404256e-03** (`generator.kvar`, 22.02×);
    smallest in-scope genuine jump **5.524501e-02** (`regcontrol.remoteptratio`,
    276.2×). The band `(6.431124e-05, 1.374769e-03)` is **empty, 21.38× wide** —
    the kill criterion needs >5×, so no kill, and the number is *placed* rather
    than tuned.
  - **Two recalibrations against the plan**, both recorded in
    `tests/TOLERANCE_NOTES.md`: (1) the plan's worst `6.43e-5` on `load.pf` is
    confirmed exactly; (2) the plan's "smallest genuine jump 1.00e-3,
    `invcontrol.lpftau`" is a number in the **census's** metric, which reports
    the ABSOLUTE difference when the expected side is 0
    (`harness::value_verdict`). Under the floor's symmetric metric that cell
    (`'0.001'` vs `'0.0'`) is rel **1.0** — a 0-vs-nonzero pair can never be
    claimed at any magnitude. Do not repeat 1.00e-3 as the floor's upper
    neighbour.
  - **Mechanism, cited site by site** (`Version8/Source/`) — the fixed-precision
    getters, N ∈ {4,5,6,7,8}: `%-.4g` `Load.pas:2345` (`pf`, the worst cell's,
    and a formatter the plan does not name) + `:2353`; `%-.5g`
    `Vsource.pas:1327-1335` and `:1343`, `Transformer.pas:1842-1843`,
    `AutoTrans.pas:1886-1887` + the `MakePosSequence` round-trips
    (`Transformer.pas:1982-1991`, `AutoTrans.pas:2021-2030`,
    `Reactor.pas:1145-1201`); `%.6g` `Storage.pas:1531-1562` and
    `Utilities.pas:2600-2607` `GetDSSArray_Real`; `%-.7g` `Line.pas:1358-1365`,
    `:1406-1407`; `%-.8g` `Vsource.pas:1337-1342` + `:1344`,
    `Reactor.pas:1091-1098`. The plan's "`%-.5g`/`%-.8g`" naming is
    **incomplete, not wrong**. *(As landed, the `Vsource` ranges overlapped the
    wrong formatter and the `%-.5g` row listed `basekv`, which has no getter arm
    at all — corrected by the audit settlement in all three copies; and the
    universal claim "every claimed cell is such a render" was replaced by the
    per-cell clause, see the settlement bullet below.)*
  - **The `%-.4g` residual is stated, not absorbed** (the anti-fudge decision of
    the sub-step). `load.pf`'s theoretical class ceiling is 5e-4, which does not
    fit the band (2.75× under its upper neighbour). The floor comes from the
    population instead: the 293 `load.pf` spellings have min mantissa **5.653**
    (`pf = 0.5653`) → population ceiling **8.845e-05**, and 2e-4 sits 2.26×
    above THAT. Residual risk left loud: a future in-scope load with
    `pf ∈ [0.1, 0.25)` could print up to 5e-4, the floor would **refuse** it and
    the gate would red — the safe, re-derivable direction. Widening to the class
    ceiling would be the fudge the discipline forbids.
  - **Landed as a cell predicate, r4133 only.** `R4133_DISPLAY_FLOOR`
    `None` → `Some(2e-4)`; new `number_rel` (the symmetric metric),
    `display_rel` (whole-cell, over `numeric_skeleton`, so scalars, bracketed
    vectors and `|`-separated matrices go through one path — a scalar-only floor
    would have left every array cell unclaimed), `under_display_floor` (offline)
    and `under_display_floor_r4133` (the seam, with `FLOOR_VISITS`/`FLOOR_HITS`
    for RP4.1); `numbers_match` is the ONE widening point and grew a non-finite
    guard. Seamed into `compare_prop_lists` as the fourth link **after** the
    echo seam via `PropsPolicy::under_display_floor` (gated on `is_r4133()`).
    The replay's floor link now calls the **shipped** predicate — it was a
    scalar-parse copy, which would have left every bracketed/matrix display cell
    unclaimed — and `props_census`'s `under-floor` disposition (built dormant by
    RP2.1) went live.
  - **Locks moved** (the values below are the as-landed ones; the settlement
    bullet carries the four it then moved): `CLAIMED_DISPLAY_FLOOR` 0 →
    **2 006**; `CLAIMED_TOTAL` 1 023 → **3 029** (now the sum of all four links);
    `DECLARED_RP24`
    (2 101, 71, 2 021) → **(0, 0, 0)** — the sub-step's acceptance;
    `DECLARED_OUT_OF_SCOPE` (134, 18, 0) → **(229, 22, 0)**. New locks
    `RP24_OUT_OF_SCOPE_ROWS` 105, `RP24_OUT_OF_SCOPE_MIN_RATIO` 277.0,
    `CEILING_ROUND_MARGIN` 1.01, `LIVE_ONLY_DISPLAY_SPELLINGS` (6) and
    `CLAIMED_SPELLINGS_LIVE` 3 036. **Unmoved and verified:** `MULTI_LINK_ROWS`
    135 (measured 0 overlaps — the floor's 2 006 is a clean addition, not a
    re-partition), `CLAIMED_NORMALIZATION` 854, `CLAIMED_ECHO` 169,
    `CLAIMED_SHAPE_ALLOWLIST` 0, `CLAIMED_TOTAL_LIVE` 855, `NORM_ROWS` 170,
    `PROPS_ECHO_R4133` 81, `ECHO_CARVE_OUT_CELLS` 1, every `SKIP_PROPS*` /
    `PROPS_015X` / scheduler mask.
  - **`DECLARED_RP24`'s 105-row residual is PROVED out, not waved off.** The
    floor claims 1 996 of the bucket's 2 101 rows; the other 105 sit on exactly
    four pairs (`generator.kvar` 102, `capacitor.cuf` 1, `storage.kw` 1,
    `storagecontroller.kwneed` 1) — the four whose FULL-census bin is 7 while
    their in-scope re-derivation is bin 6, so `effective_bin()` routes the pair
    to the floor while its example inventory still carries the spellings that
    made it bin 7. `RP24_OUT_OF_SCOPE` + `row_out_of_scope_by_ceiling` re-declare
    them `Owner::OutOfScope` by a citable argument: `max_rel_in_scope` is the
    maximum over exactly the in-scope cells, so a spelling that exceeds it cannot
    be one of them. Narrow by construction (four cited pairs, numeric rows only,
    ceilings read back from `bins.tsv`), and the tightest row clears its ceiling
    by **277×** against a `%.2e` rounding needing 1.005×. `row_in_scope` uses it,
    so `DECLARED_OUT_OF_SCOPE.2` stays **0** — now a per-ROW zero, stronger than
    the per-pair one it replaces. **The live census confirms it independently:**
    all 12 pairs that carry both an `under-floor` and an `UNCLAIMED` cell report
    `cells_in_scope = 0` on every unclaimed spelling.
  - **Full claims census at HEAD** (`DSS_PROPS_CENSUS=claims`, 439 cases × 2
    channels, 1 060 165 rows, 58.1 s; error baselines exact — **5** r4133 /
    **22** capi, so the run is complete, not short): `under-floor` **49 451
    cells / 46 538 in scope**, **2 012** spellings, **79** pairs; every other
    r4133 disposition **unmoved to the cell** (`BoolFold` 294 519/280 915,
    `CaseFold` 99 023/95 270, `ArrayForm` 122 756/118 744, `EnumSynonym`
    4 619/3 817, `echo-row` 488 017/468 044); claimed **1 058 385 / 1 013 328**
    over **309** distinct pairs; `UNCLAIMED` 51 105 → **1 654** (47 427 →
    **889**), 104 → **37** pairs. **capi: 0 on every r4133 disposition**, its 11
    `ledger-hit`s (4 in scope, 8 pairs) and 88/88 `UNCLAIMED` intact — 99 value
    cells / 92 in scope, unchanged since RP2.1.
    - **The live worst floor-claimed cell is the derivation's own**:
      6.431124e-05 on that same `load.pf` spelling, over the whole 439-case
      population — the frozen extract did not under-sample the tail. All 293
      live `load.pf` spellings are `under-floor`.
    - **Term-by-term reconciliation.** Live floor spellings **2 012** = the
      replay's `CLAIMED_DISPLAY_FLOOR` **2 006** + **6** live-only
      (`vsource.isc1` ×2, `isc3`, `r1`, `x0`, `x1` — the same "+2 cells per
      `vsource` pair" population drift RP2.1/RP2.2 recorded, at spelling
      granularity; all five pairs hold a frozen `bins.tsv` row, so no vendored
      file may carry them). Live floor cells **49 451** = the frozen 49 437 over
      the shared 2 006 rows + **8** (eight rows are +1 live) + **6**. Claimed
      spellings **3 036** = `CLAIMED_TOTAL` 3 029 + 1 `LIVE_ONLY_SPELLINGS` + 6.
      All of it is now an *asserted* term
      (`LIVE_ONLY_DISPLAY_SPELLINGS`, `CLAIMED_SPELLINGS_LIVE` and
      `the_display_floors_live_only_spellings_reconcile_the_claims_census`),
      not narration. `UNCLAIMED`'s shrink is exactly the floor's claim
      (−49 451 / −46 538).
    - **The 889 in-scope residual is fully attributed** (re-grouped by the pair's
      frozen bin): bin 5 → 6 pairs / 1 069 / 777 = **RP3.8** (5 pairs,
      1 064/772) + `line.linecode` (**RP3.6**, 5/5); bin 7 → 20 pairs / 361 /
      **32** = `swtcontrol.delay` (**RP3.1**, 42/24), `windgen.kvar` (**RP3.2**,
      4/4), `generator.model` (**RP3.3**, 2/2), `gictransformer.r2` (**RP3.4**,
      2/2), the other 16 pairs 311/**0**; bin 4 → 8 pairs / 186 / 80 = **RP3.7**
      (`swtcontrol`/`relay` `normal`/`state`) + the four `sensor.*` at 0 in
      scope; `autotrans.wdgcurrents` 34/0, `line.units` 3/0 (**RP3.5**),
      `isource.bus1` 1/0 (§1.3). **Bin 6 is gone from the unclaimed table
      entirely** — RP2.4's acceptance, measured.
  - **Hand-off consumed:** RP2.3's `ECHO_CARVE_OUT_ROUTING` cell
    (`reactor.kvar` `'66.6666666666667'` vs `'66.667'`, 5.0e-06, r4133's own
    `MakePosSequence` `%-.5g` round-trip) is now **claimed by
    `Link::DisplayFloor`**, and `the_carve_outs_are_routed_and_only_they_are`
    chases it to that link instead of asserting a non-empty RP2.4 bucket (which
    at `(0,0,0)` would be vacuous). Its sibling `reactor.kv` (bin 6, 5.85e-06)
    goes the same way. **No new hand-off.** Buckets still open: `Rp3` (7, 4, 7),
    `Rp35` (8, 6, 5), `Rp38` (181, 5, 181), `OutOfScope` (229, 22, 0);
    `Rp22`/`Rp23`/`Rp24` are all `(0, 0, 0)`.
  - **`tests/TOLERANCE_NOTES.md` §"r4133 props display floor"** carries all eight
    required components (scope; measured worst + ratio; the `%[-].Ng` mechanism
    with its Pascal table; the decomposition/empty-band argument; why no coverage
    is lost; scope justification; fix owner; and the relaxes/never-relaxes pair,
    including the `%-.4g` residual and the honest limit that a floor cannot
    separate a display artifact from a genuine sub-2e-4 difference — bounded by
    the capi compare at the case tier floors on every `both` case and by the
    1e-6-class model gate on the `r4133`-only ones). The stale RP2.1 paragraph
    that said the slot was `None` is corrected in the same file. This is the
    plan's ONE sanctioned edit to that file (§1.1). *(Components 3, 5, 7 and 8
    were rewritten by the audit settlement: the mechanism is now a clause, the
    capi bound is the tier floors and not "zero tolerance", and the fix owner is
    "none for what it claims, RP3.9 for what it refuses".)*
  - **Mutation-tested, three ways** (tree restored byte-identical each time):
    the floor made channel-blind → caught by 2 tests; widened 10× to `2e-3` →
    9 tests across 3 binaries, including the master accounting; made scalar-only
    → 5 tests.
  - **Tests: none deleted, three replaced by strictly stronger successors** —
    the dormant `R4133_DISPLAY_FLOOR.is_none()` guard → the literal value plus
    both sides of the boundary (1.8996e-4 in, 2.0996e-4 out) plus the
    0-vs-nonzero and non-finite refusals; the floor-slot placeholder row in
    `the_capi_channel_claims_nothing` → promoted into that test's r4133-claimed
    list (a live channel statement, not a slot guard);
    `..._and_the_floor_claims_nothing` → `..._and_the_floor_only_its_derivation`
    (every floor-claimed row is numeric, inside the floor, and the worst is
    6.431124e-05). Six new: the metric decomposition, the seam's visit/hit
    counters, the two channel tests, the plan's acceptance probe landed
    permanently
    (`the_display_floor_drops_only_the_cell_it_claims_only_on_r4133`), the
    residual scope proof, and the live-only reconciliation.
  - **Goldens measured, not assumed:** `git status` after the full gate showed
    only the sub-step's own files; the stray `Export` artifacts the AutoTrans
    decks drop into `tests/corpus/electricdss-tst/` were swept
    (`git clean -f -- tests/corpus`), census artifacts (`tmp/props_census.json`,
    `tmp/props_census/`, `tmp/`) deleted by name. Full five-command gate green
    on the final tree, both lanes; test count **8 098 → 8 322** (4 049 →
    **4 161** per lane, **+112**): 5 harness-side guards × 22 harness-bearing
    binaries, plus 2 in the replay binary (the residual scope proof and the
    live-only reconciliation).

- **RP2.4 audit settlement** (2026-08-23, one commit, still **zero engine
  change**) — eleven findings from the two auditors, all settled; the two majors
  were the same defect measured two ways.
  - **The majors: the mechanism was a survey, and 55 spellings contradicted it.**
    Part A attributed the claimed population to the Delphi `%[-].Ng` family by
    naming the formatter sites and asserting the conclusion universally in four
    places (`R4133_DISPLAY_FLOOR`, TOLERANCE_NOTES components 3 and 7, STATUS).
    `audit-code` re-scored all 3 454 spellings against "is r4133's number a
    rounding of ours to SOME digit count (both tie rules)" and found **55 rows /
    70 cells / 27 pairs** where it is not; `audit-tests` reached the same
    population from the other side (the pair's own formatter ceiling, 88 rows /
    109 cells / 34 pairs, a superset by a laxer estimator) and additionally ran
    the live census to prove all of them carry `count_in_scope = 0`. Both are
    reproduced here. **Fixed by narrowing, not by re-explaining**: the floor grew
    a second clause, `props_norm::display_is_render` — every number of the r4133
    side must be our number rounded to the digits r4133 *printed*, read off its
    own spelling (`0.5·10^(e−N+1)` + a tie margin + one half-unit of the 15-digit
    grid FPC's conversion may round through). The mechanism claim is now a
    property of the predicate. The 55 are declared to a new **RP3.9**
    (`RP39_ROUTING`: 27 pairs, each with its r4133 site and its round-trip chain
    — `load.kva` recomputed from an already round-tripped `pf`, `vsource.puz*`
    from a round-tripped Z, `line.b0`/`b1` from a round-tripped C, and the
    full-precision `%-g`/`%g` getters where the two engines simply differ), which
    puts them back in `claims_unclaimed_pairs.txt` — the WP-RP3 work list — and
    makes RP4.1 wait on them (plan §0). No cell of the 55 is in scope, so nothing
    the gate compares moved. (**RP3.9 landed 2026-09-02** — all 27 pairs
    `PRECISION_ROUNDTRIP`, its record in §1.)
  - **Locks moved by the settlement:** `CLAIMED_DISPLAY_FLOOR` 2 006 → **1 951**;
    `CLAIMED_TOTAL` 3 029 → **2 974**; `CLAIMED_SPELLINGS_LIVE` 3 036 →
    **2 981**; new `DECLARED_RP39` **(55, 27, 19)** and `RP39_ROUTING` (27 rows,
    per-pair row + in-scope split + citation). `DECLARED_OUT_OF_SCOPE`
    (229, 22, 0), `RP24_OUT_OF_SCOPE_ROWS` 105 and the 277× margin are
    **unmoved** — `declare` runs the mechanism arm first, so the two rows that
    would otherwise have drifted into the ceiling rule (and dropped its measured
    margin to 2.08×) stay RP3.9's.
  - **Live re-census** (439 × 2, 1 060 165 rows, 56.9 s, error baselines exact
    5/22): `under-floor` **49 381 / 46 538 in scope / 1 957 spellings / 69
    pairs**; `UNCLAIMED` **1 724 / 889 / 480 / 59**. **In-scope counts on both
    sides unchanged**, every other disposition unmoved to the cell, capi still 0
    on every r4133 disposition — the settlement moves exactly the 70 cells it
    claims to move.
  - **The other major — "the capi compare is exact / at zero tolerance" is
    false.** `compare_all_properties` hands `compare_prop_lists` the case tier's
    `i_rel`/`i_abs` and `value_verdict` passes at `abs + rel·|e|`. Corrected in
    all five places (TOLERANCE_NOTES components 5 and "scope justification",
    `PropsPolicy::under_display_floor`, `R4133_DISPLAY_FLOOR`, the census
    comment) and replaced by the honest tier statement, now pinned with the two
    kinds that are the hole: `midi` (no `tol_for` arm → 1e-6/1e-4) and
    `micro_wtg3_dynamics` (2e-5/1e-4) do not bound a value below 0.5, `feeder`
    none below 0.05 — `props_policy_tests::the_capi_property_compare_runs_at_the_case_tier_floors`.
  - **Six minors, each with the guard the auditor named missing.** (1) The
    `Vsource.pas` ranges + the `basekv` attribution, fixed in all three copies.
    (2) The "never relaxes a discrete value" bullet, which the metric alone did
    not provide: reworded to what the predicate does, and the general case is now
    covered by the mechanism clause (`'5001'` vs `'5000'`, 2.0e-4, refused —
    pinned). Its two one-sided constants are pinned as measurements now as well:
    `RP24_OUT_OF_SCOPE_MIN_RATIO` is bracketed above (`< 278.0` against the
    measured 277.17×) and `CEILING_ROUND_MARGIN` literally (`== 1.01`), so the
    auditor's 277 → 2 and 1.01 → 200 mutations red. (3) The `ArrayForm` wiring, provably inert (the auditor's mutation
    `numbers_match → ==` ran green): the RP2.1 discrimination row the widening
    had taken back (`'[600,700.007,]'`, 1e-5) is **restored** — the render clause
    refuses it — and a positive display fold (`'[ 600 700.00003]'` vs
    `'[600,700,]'`) now pins the wiring from the other side. (4) The seam
    counters, which no test tied to the seam: the comparator test and the counter
    test both drive `PropsPolicy::under_display_floor` and assert
    `FLOOR_VISITS`/`FLOOR_HITS` move, so the offline-twin swap reds. (5)
    `LIVE_ONLY_DISPLAY_SPELLINGS`, which carried only its length: each of the six
    is now anchored to one of five cited `vsource` pairs and to that pair's own
    frozen `max_rel` (the auditor's `'1.4257'` mutation is 9.09e-5 against
    3.72e-5 — and is no longer a render either). (6) The vacuous
    "differing skeleton" block in the metric test: two rows with the SAME number
    count and a different skeleton (`'[ 400]'` vs `'400'`, `'1 kV'` vs `'1 kW'`)
    now exercise the clause the block claimed to pin. (7) `row_out_of_scope_by_ceiling`
    compared a symmetric metric with the census's offender-only `max_rel`: it now
    **refuses multi-number rows** outright, with the implication written out — on
    a one-number row the census metric dominates `display_rel`, on a multi-number
    row it need not.
  - **Every mutation the auditors landed green now reds**, verified: the
    `tokens_match` widening removal (1 test), the `PropsPolicy → props_norm`
    seam swap (2), the fabricated live-only spelling (1), the skeleton-clause
    deletion (2), plus two of the settlement's own — dropping the mechanism
    clause from the floor (7 tests across the accounting, the two channel tests
    and the metric) and dropping the single-number guard from the ceiling rule
    (1). Test count
    **8 322 → 8 368** (4 161 → **4 184** per lane, **+23**): 1 new harness guard
    (the capi tier floors) × 22 harness-bearing binaries, plus 1 new replay test
    (RP3.9's both-ways proof) — and no test deleted, `#[ignore]`d or loosened.

### R4133_PROPS WP-RP3 — condensed records

> Plan: `R4133_PROPS_PLAN.md` §WP-RP3. Same branch (`r4133-props`), same
> per-sub-step ritual. Four bin-7 root-cause pairs (RP3.1–RP3.4) plus the five
> sub-steps the WP-RP2 triage opened (RP3.5–RP3.7 from RP2.2, RP3.8 from RP2.3's
> kill ruling, RP3.9 from the RP2.4 audit settlement) plus §RP3.12, which RP3.9's
> own P0 open item opened. RP4.1 waits on the first nine of
> them; **RP3.5 landed 2026-08-28 (audit settled 2026-08-29), RP3.6 both parts
> 2026-08-29 (audit settled the same day) and RP3.7 2026-09-02 (audit settled the
> same day: 11 findings, 9 fixed, 2 fixed with a sub-claim refuted, none
> dropped)**, and **RP3.8 landed 2026-09-02** as well (audit settled the same
> day: 8 findings after dedup, one major — `Save` was a fifth, un-refreshed
> `get_value` reader — 7 fixed, 1 recorded, none dropped), and **RP3.9 landed
> 2026-09-02** as well (27 pairs, all `PRECISION_ROUNDTRIP`, no product-crate
> line; audit settled 2026-09-03 — 15 raw findings, 11 distinct: 9 fixed,
> 2 recorded, none touching a verdict), so every WP-RP3 sub-step **RP4.1 waits
> on** has landed and the unmask is no longer blocked here. **§RP3.12 landed
> 2026-09-03** as well (audit settled the same day: 12 findings, 11 distinct —
> 9 fixed, 2 recorded, 0 refuted) — RP3.9's P0 open item, settled
> `UPSTREAM_BUG`/never-reproduced with zero product-crate lines, and no
> precondition to the flip either, its four decks being capi-only. **RP4.1 itself
> landed 2026-09-03** (audit settled the same day: nine dispositions, eight fixed,
> one recorded), taking the eight `property` entries RP3.1/RP3.2/RP3.4 staged into
> `ledger.json` and retiring their `RP3_ROUTING` rows — so nothing below is
> "staged" any more; the §RP4.1 record in §1 carries the landing. Of the WP's
> two sub-steps that run after RP4.1 and block §RP5.2 instead (plan §0),
> **§RP3.11 landed 2026-09-03** (`KEEP_LIVE_PINNED` on both `Save` and `Dump`;
> the kill criterion fired), and only §RP3.10 is left. The RP3.6, RP3.7, RP3.8,
> RP3.9, RP3.11 and RP3.12 records live in §1 above, beside RP3.5's narrative
> one.

- **RP3.1** (2026-08-24) — `swtcontrol.delay`: **a wired property that r4133
  silently ignores.** **Zero product-crate bytes** (the port already behaves
  correctly), **zero `ledger.json` bytes**, zero golden / manifest /
  `population.lock` / frozen-extract bytes, and no r4133 mask moved — the whole
  sub-step is two test files plus docs.
  - **Root cause, verified against the vendored r4133 source, not the plan's
    transcription.** `SwtControl` declares nine properties, the fifth being
    `Delay`. `Edit` stores every token in `PropertyValue[]` first
    (`Version8/Source/Controls/SwtControl.pas:192-193`) and then dispatches on
    the property number — and the `CASE` has arms for 1, 2, 3, 4, 6, 7, 8 and 9
    but **none for 5** (`:195-218`; the plan said `:194-217`, off by one, the
    only drift found). `delay=` therefore falls through to `ClassEdit` as an
    inherited parameter, `TimeDelay` keeps the 120.0 `Create` gave it (`:310`),
    and the getter — which is **LIVE**, `Format('%-.7g',[TimeDelay])` at `:588` —
    answers `120` to a deck that asked for 0.25. dss_capi 0.14.5 wires the
    property through its typed table (`src/Controls/SwtControl.pas:185`) and this
    port follows it (`elements/control/swt_control/accessors.rs:114`, ordinal at
    `mod.rs:51`, `PropDef::double("Delay")` at `mod.rs:78`), so **nothing in the
    engine was owed a change**.
  - **Render-only, and provably so.** The report had to state it is not a
    queue-delay bug, and the evidence is stronger than the plan assumed:
    `Sample`'s two `ControlQueue.Push` calls are commented out wholesale
    (`:484-507`, "Removing because action … and lock are instantaenous"),
    `DoPendingAction` likewise (`:396-408`), `set_States` writes
    `ControlledElement.Closed[]` immediately (`:532-549`) — **and** `LockCommand`
    is commented out of the class declaration too (`:39`), while
    `ActionCommand`/`PresentState`/`Armed` exist in neither `TSwtControlObj` nor
    `TControlElem`. The dead block would not compile if uncommented, so nothing
    upstream can consume `TimeDelay`. Two further facts worth the record: r4133's
    own `InitPropertyValues` (`:654-655`) writes `'120.0'` into slot 5 and then
    immediately `''`, so `Dump` prints the deck's token while `?` prints 120 —
    the engine contradicts itself; and the **sibling copy in the same trunk**
    (`Version8/Source/CMD_Lazz/Controls/SwtControl.pas:179`) still has
    `5: TimeDelay := Parser.DblValue;`, so the arm was lost in a rework rather
    than never written.
  - **Census, confirmed twice** (frozen extracts + a bounded live re-run,
    `DSS_PROPS_CENSUS=claims DSS_GATE_ONLY=swtcontrol`, artifacts deleted):
    42 cells / **24 in scope**, and the in-scope half decomposes exactly as the
    plan claims — **12 + 12** over `controls:swtcontrol/swtcontrol_time.dss` and
    `controls:swtcontrol/midi_swtcontrol.dss` (both `engines: r4133`, one cell
    per step 0..11, ours `0.25` vs r4133 `120`, `max_rel` 0.9979166666666667).
    The other **18** are all `capi_v0145`, hence owed nothing: **12** on
    `controls:swtcontrol/swtcontrol_lock.dss` (the same `~ delay=0.25`, 12 steps
    — it is the third deck of the `'0.25'` spelling, whose frozen row is
    therefore 36 cells, not 24) and **6** on three copies of the vendored
    `IEEE_519.DSS` (`Delay=0.0`, two SwtControls each, 1 step — the `'0'`
    spelling). `civanlar.dss` — 16 SwtControls, `engines: r4133`, in scope —
    types no `delay=` and produces **no** cell, i.e. our unset render already
    equals r4133's 120, measured by its absence. Since the audit settlement the
    whole decomposition is **derived, not transcribed**:
    `props_r4133_replay::the_rp31_census_decomposition_is_read_off_the_corpus`
    sweeps every corpus `.dss` for SwtControl declarations, multiplies each
    case's controls by its `population.lock.json` `steps=`, splits on the lock's
    `engines=`, reconciles the products against `bins.tsv`'s 42/24 and both
    frozen example rows (36 / 6), and requires the in-scope cases to be exactly
    the cases the drafted entries cite — "exactly two entries" is now a measured
    conclusion.
  - **The exclusion shape is a ledger entry, never an echo row** (the plan is
    explicit and the reason is the mechanism: `:588` is live, so calling this an
    echo would be a false statement). Per §1.1(e) the two entries are **drafted
    here and land in RP4.1's unmask commit** — earlier they would fail
    `assert_all_hit` as NEVER APPLIED, the r4133 property compare being masked
    until then. Verbatim, to be copied into `tests/corpus/ledger.json` at RP4.1
    (*copied verbatim and **landed 2026-09-03** in RP4.1's commit `59e521e5`,
    with the cause below; both entries hit — 24 in-scope cells — on the first
    unmasked run*):

    ```json
    "swtcontrol-delay-not-wired": "EPRI r4133 never wires SwtControl property 5 `Delay`: the Edit CASE (Version8/Source/Controls/SwtControl.pas:195-218) has arms 1,2,3,4,6,7,8,9 and NO arm 5, so `delay=` lands only in the echo store (:192-193) while `TimeDelay` keeps its Create value 120.0 (:310) and the LIVE getter renders it (:588, Format('%-.7g',[TimeDelay])). dss_capi 0.14.5 wires the property through its typed table (src/Controls/SwtControl.pas:185, PropertyOffset[ord(TProp.Delay)] := ptruint(@obj.TimeDelay)) and the port follows it, so a deck that sets `delay=` reads back its own value here and 120 there. The divergence is RENDER-ONLY on r4133: nothing consumes TimeDelay there — Sample's queue-pushing body is commented out wholesale (:484-507, pushes at :492/:498, and LockCommand is commented out of the class declaration at :39 so the block no longer even compiles), DoPendingAction likewise (:396-408), and set_States acts immediately (:532-549); the two decks' event log and control queue are empty under r4133 and are live-compared. Per the 2026-08-02 policy the port keeps the correct behavior (the property is wired) and the upstream defect is reported (investigations/to_opendss/43-swtcontrol-delay-not-wired.md, local) + excluded here + pinned by the expected-value pins named in `source`. Exact-pair (a discrete value jump, not display precision)."
    ```

    ```json
    {
      "id": "r4133-swtcontrol-delay-ignored-time",
      "case": "controls:swtcontrol/swtcontrol_time.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^swtcontrol\\.sw\\.delay$",
          "rust": "0.25",
          "oracle": "120"
        }
      ],
      "cause_ref": "swtcontrol-delay-not-wired",
      "source": "R4133_PROPS_PLAN RP3.1 (2026-08-24), drafted in the sub-step and landed at RP4.1 per §1.1(e) — earlier it would fail assert_all_hit as NEVER APPLIED, the r4133 property compare being masked until the unmask. Measured by the bounded claims census (DSS_PROPS_CENSUS=claims DSS_GATE_ONLY=swtcontrol): 12 in-scope cells, one per step 0..11, element SwtControl.sw, ours 0.25 (the deck's `~ delay=0.25`) vs r4133's frozen 120, max_rel 0.9979166666666667. Replacement pin: props_r4133_pins.rs::swtcontrol_delay_wires_the_property.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    ```json
    {
      "id": "r4133-swtcontrol-delay-ignored-midi",
      "case": "controls:swtcontrol/midi_swtcontrol.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^swtcontrol\\.sw\\.delay$",
          "rust": "0.25",
          "oracle": "120"
        }
      ],
      "cause_ref": "swtcontrol-delay-not-wired",
      "source": "R4133_PROPS_PLAN RP3.1 (2026-08-24), drafted in the sub-step and landed at RP4.1 per §1.1(e). Same pair on the second r4133-gating SwtControl deck (the IEEE123-class midi loop tie): 12 in-scope cells, steps 0..11, element SwtControl.sw, ours 0.25 vs r4133 120. The third corpus deck that sets delay= (controls:swtcontrol/swtcontrol_lock.dss) is engines: capi_v0145, where the port and the pinned 0.14.5 oracle agree, so it needs no entry. Replacement pin: props_r4133_pins.rs::swtcontrol_delay_wires_the_property_on_the_midi_tie.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    The shape was checked against the loader
    (`corpus_gate/ledger.rs`): key `format!("{element_lower}.{prop_lower}")` →
    `swtcontrol.sw.delay`, the numeric exact-pair arm requires the `rust` pin
    (`:1093-1107`), `cause_ref` must resolve to a `causes` key (`:1453-1460`),
    and the case's `engines` must contain the channel (`:1426-1435`) — both cases
    are `engines: "r4133"`.
  - **The pins** (`crates/dss-core/tests/props_r4133_pins.rs`, both lanes, no
    oracle): `swtcontrol_delay_wires_the_property` — `SwtControl.sw.Delay` is
    `'0.25'` on `swtcontrol_time.dss`, `'7.5'` after an `edit`, and `'120'` on
    `civanlar.dss` where no deck types `delay=` (three readings, so neither a
    hardwired getter nor a parse-time echo would pass) — and
    `swtcontrol_delay_wires_the_property_on_the_midi_tie` (`'0.25'` → `'3.5'` on
    the loop tie). Because these witness a **drafted ledger entry** and not an
    `EchoRow`, the file's "no un-cited `#[test]`" guard needed a second citation
    list: `props_r4133_replay::LEDGER_ENTRY_PINS`, pinned literally like
    `NOT_A_PIN` and cross-checked against the routing row, with
    `every_echo_row_pin_is_a_test_that_exists` now matching the **union** of the
    two lists both ways.
  - **The accounting move: nothing is claimed, and that is the honest answer.**
    A settled sub-step whose artifact is staged into RP4.1 leaves its rows on the
    work list — so `DECLARED_RP3` stays **`(7, 4, 7)`** and the new
    `RP3_ROUTING` (`props_r4133_replay.rs`, the `RP38_ROUTING`/`RP39_ROUTING`
    shape) carries the per-pair split and each sub-step's verdict:
    `generator.model` 1/1 (RP3.3, open), `gictransformer.r2` 1/1 (RP3.4, open),
    `swtcontrol.delay` **2/2 (RP3.1, settled — cited, drafted, pinned)**,
    `windgen.kvar` 3/3 (RP3.2, open). Its guard re-measures the three columns
    from the walk rather than transcribing them (a sum-preserving swap of two
    pairs' row counts reds it), forbids an open sub-step from owning a pin, and
    (audit settlement, below) checks each settled verdict against the
    obligations of its own **outcome tag**.
  - **Audit settlement (2026-08-24, same day).** Ten minor findings from the
    `audit-code`/`audit-tests` pair — seven distinct issues once the two
    duplicated pairs are merged — all settled in one commit as **4 new guards +
    3 strengthened checks inside the routing guard**, plus the record
    corrections they imply. Every one is *fixed*; none was waved off or
    deferred, and no finding touched the sub-step's premise. Still no
    `ledger.json` byte and no product-crate byte:
    - **The shrink is a hand edit at RP4.1, and the docs said otherwise.** No
      link of the chain reads `tests/corpus/ledger.json` (`Link::ORDER` is four
      links; `declare` routes bin-7 root-cause rows to `Owner::Rp3`
      unconditionally), so `DECLARED_RP3` will **not** move when the entries
      land. `DECLARED_RP3`/`RP3_ROUTING` now say so, plan §RP4.1 carries the
      obligation as a numbered precondition, and the new tripwire
      `the_staged_r4133_property_entries_have_not_landed_yet` reds the moment any
      `property`-scoped `r4133` entry appears in the ledger, with the
      instruction in its message (proved live: flipping its channel filter to
      `capi_v0145` lists the five existing capi property entries).
    - **The settled-verdict contract assumed RP3.1's shape was every shape.**
      Plan §WP-RP3 sanctions three outcomes; the guard demanded a staged ledger
      entry from all of them, so a future RP3.x closing as a port fix or an echo
      row would have had to *loosen* it. Replaced by the typed taxonomy
      `RP3_SETTLED_SHAPES` (`LEDGER` / `ECHO` / `FIX`, pinned literally by
      `the_settled_outcome_taxonomy_is_the_plans_three`): shared obligations
      first, then each tag's own; an unknown tag is a hard failure naming the
      table to extend.
    - **`.pas:` was satisfiable by the capi citation.** A settled verdict must
      now cite the r4133 unit itself (`Version8/Source/` **and** `.pas:`) — the
      audit's own mutation (r4133 cite → prose, capi cite left in place) is red.
    - **"names every witness" was prefix-shadowable.** `verdict.contains(pin)`
      let `…_wires_the_property_on_the_midi_tie` stand in for
      `…_wires_the_property`; the new `names_identifier` requires a whole-token
      match and has its own self-test.
    - **The 24 = 12 + 12 decomposition lived only in prose** (mutating `24` to
      `25` stayed green). `the_rp31_census_decomposition_is_read_off_the_corpus`
      derives it — corpus-wide SwtControl sweep × `population.lock.json`
      `steps=`/`engines=`, reconciled with `bins.tsv` 42/24 and both frozen
      example rows — ties the in-scope cases to the drafted entries one-for-one,
      and requires the verdict to carry the derived figures (the `24 → 25`
      mutation is now red).
    - **Records: the census arithmetic did not close over 42.** "The remaining 6"
      was 18 (the 12 `capi_v0145` cells of `swtcontrol_lock.dss` had been elided
      and the `'0.25'` spelling is 36 cells, not 24) — corrected in the bullet
      above and in the routing verdict; and §1's RP2.4 frontier sentence, which
      still said every one of the 889 in-scope UNCLAIMED cells belongs to an
      *open* RP3.x sub-step, now records RP3.1's 24 as settled-but-staged.
    - **The upstream report understated the bug** (local file, uncommitted): the
      DDLL `Delay` **write** is dropped too — `DSwtControls.pas:135-136` routes
      it through `Set_Parameter` → `DSSExecutive.Command` (`:20-28`), i.e. back
      into the armless `Edit` `CASE`, so only the COM setter
      (`ImplSwtControls.pas:183`, a direct field write) can set what the script
      cannot. Report corrected; the drafted ledger entries never made the claim.
  - **Test count 8 368 → 8 384** (4 184 → **4 192** per lane, **+8** — summed on
    the parity lane's full run, 4 192 passed / 0 failed; the default lane's full
    run is green and its two binaries carry the same +4: `props_r4133_pins`
    31 → 33, `props_r4133_replay` 117 → 123): 2 pins + 2 replay guards from the
    sub-step, 4 more replay guards from its audit settlement, single-binary
    each, no test deleted, `#[ignore]`d or loosened.
    Every new guard was mutation-probed and red as intended (a sum-preserving
    swap of two routing rows' counts; a pin renamed out from under its citation;
    `24 → 25` in the verdict's census; the r4133 citation replaced by prose; the
    short pin dropped in favour of its longer prefix-mate; a control count moved
    against the deck; an unknown outcome tag; the ledger tripwire's channel).
    Report: `investigations/to_opendss/43-swtcontrol-delay-not-wired.md`
    (gitignored, local-only — verified absent from both commits).
    **Open follow-up, out of RP3.1's render-only scope:** the port's `Sample`
    still queues at `time_delay` where r4133's body is dead
    (`swt_control/mod.rs:189-229`, which already carries the NOTE) — a behavioral
    lock-path question owned by whoever retires that body, not by this sub-step.
    Second: `swtcontrol_lock.dss` carries the same 12 divergent cells but is
    `engines: capi_v0145`, so if a later WP gates it on r4133 a third entry is
    due (recorded in entry 2's `source` so the fact cannot be lost).
    Third, for **RP4.1**: landing the staged entries is also an accounting
    commit — retire the settled `RP3_ROUTING` rows, shrink `DECLARED_RP3` by
    exactly those rows and re-state the tripwire (plan §RP4.1 precondition 2).

- **RP3.2** (2026-08-24) — `windgen.kvar`: **a wired property whose r4133 getter
  reads the wrong live field.** **Zero product-crate bytes** (the port already
  behaves correctly), **zero `ledger.json` bytes**, zero golden / manifest /
  `population.lock` / frozen-extract bytes, and no r4133 mask moved — two test
  files plus docs. The plan's three outcomes resolved to **LEDGER**; the kill
  criterion did **not** fire (the probe separated echo from live cleanly).
  - **Root cause, verified against the vendored r4133 source.** `WindGen`
    declares `kvar` as property 11 and documents it as "Specify the **base
    kvar**" (`Version8/Source/PCElements/WindGen.pas:364-365`, verbatim the
    `Generator` help at `Generator.pas:396`). The write path works — `Edit` arm
    11 exists (`:629`, `11: Presentkvar := Parser.DblValue`) and
    `Set_Presentkvar` stores the value in `kvarBase` (`:2996-3009`). The **read**
    path does not: `GetPropertyValue` arm 11 is `Format('%.6g',[presentkvar])`
    (`:2896`) and `Get_Presentkvar` returns
    `WindGenvars.Qnominalperphase*0.001*Fnphases` (`:2297-2300`) — the
    *dispatched* Q. So this is **not** RP3.1's shape (an unwired property) and
    not an echo either: the echo store's own default for the slot is `'60'`
    (`:2446`) and is unreachable, `TDSSObject.Get_PropertyValue` being virtual
    (`DSSObject.pas:117-120`). The axis is **live-field-A vs live-field-B**.
    r4133 contradicts itself inside its own trunk: `Generator` has the
    **identical** getter (`Generator.pas:2402-2405`) and the identical help, yet
    renders the base (`Generator.pas:3018`, `Format('%.6g',[kvarBase])`) — which
    is what the port does (`elements/pc/windgen/accessors.rs:431`, the twin of
    `pc/generator/accessors.rs:495`).
  - **The live probe (summarized; driver = `epri-worker` over the git-tracked
    `tools/opendss/bin/r4133/OpenDSSDirect.dll`, banner `Version 11.0.0.1
    (64-bit build) - Charlottesville`; the temporary port-side example was
    deleted, nothing under `tests/corpus` was written).**
    - **Decisive — not an echo.** A scratch deck typing `kW=3000 kvar=500`:
      r4133 `? WindGen.w1.kvar` → **`0`**, port → `500`. An echo store would have
      printed `500`.
    - **The value IS parsed; only the render ignores it.** `Edit WindGen.w1
      kvar=777` → r4133 kvar `0` **but PF `0.968058`** (= 3000/√(3000²+777²)),
      port `777` / `0.968057839822749` — `Set_Presentkvar`'s side effect fired
      identically on both engines.
    - **Not a port bug — the physics already agree.** Solved terminal powers on
      `WindGen.w1`, the observable independent of the property string, over all
      five corpus decks: Q **−2.13e-05 / −4.22e-05 kvar** in power flow on both
      engines; in dynamics **−37 087.81 (port) vs −37 087.76 (r4133)** and
      **−29 216.72 vs −29 216.67**. The port ports `SetNominalGeneration`
      loop-for-loop, `Else kvarCalc := 0` (`:1320-1321`) included
      (`windgen/nominal.rs:223-225`), so **no engine change was owed** — and
      adopting `presentkvar` as our render would be *reproducing* an upstream
      defect, which the 2026-08-02 policy bars.
    - **Wrong even when the dispatch works.** With `QMode=1` (the PF arm,
      `:1277`) the same deck renders **`363.54`** — the operating-point Q — for a
      typed `kvar=500`; terminal Q −363.540715 (r4133) vs −363.540715423 (port).
    - **In dynamics the render is a stale intermediate.** `:1254` skips the Q
      block, so after `Edit kvar=777` on `windgen_dyn.dss` r4133 renders **777**
      while its own `kvarBase` is **792.718441186736** (the port's value, reached
      through the identical `PFNominal 0.897802`) and the measured terminal Q is
      **−37 087.76 kvar** — three numbers, the render tracking none.
    - **Round-trip model corruption, produced not predicted.** `Save Circuit` on
      r4133 wrote `New "WindGen.w1" … kW=3000 kvar=0 …` for the machine built
      with `kvar=500` (the port wrote `kvar=500`), via `TDSSObject.SaveWrite`
      reading `PropertyValue[]` (`DSSObject.pas:156`) through the virtual getter;
      reloading then also resets `PFNominal`→1.0 and `kvarMax`/`kvarMin`→0
      (`:3001-3008`).
  - **Census, derived and closed over every cell.** 4 cells / **4 in scope**,
    `1 + 1 + 1 + 1` over the four diverging decks (`bins.tsv:303`
    `windgen.kvar numeric 7 - 4 4 9.86e+02 9.86e+02`; `examples_full.txt:3375-3377`
    `'854.95263026673'|'0'|2`, `'726.483157256779'|'0'|1`,
    `'986.05231553659'|'0'|1`). All five `modes:windgen/*` decks are
    `steps=1 … engines=r4133` in `population.lock.json`, one `WindGen.w1` each.
    **No deck types `kvar=` at all** — every value is a side effect: `kVA` unset
    ⇒ `kW·√(1/pf²−1)` (3000/0.95 → 986.05231553659; 1500/0.9 →
    726.483157256779), `kVA` set ⇒ `√(kVA²−(kVA·|pf|)²)` at `Create`'s pf 0.88
    (`:917`) ⇒ kWBase 1584, 854.95263026673 on both dyn decks. The fifth deck,
    `modes:windgen/windgen_snap.dss`, types `pf=1.0`, so **our** base is 0 and
    both engines print `0` — a value coincidence, **not** a mode story: typing
    `kvar=777` there diverges 777 vs 0 (probed, and pinned). Beyond the
    population, exactly **two** further corpus decks declare a WindGen
    (`…/WindGenerator/WindGen_GFL_Dynamics/` and `…/WindGen_QSTS/`
    `Run_IEEE123Bus_GFLDaily.DSS`); both sit in
    `tests/corpus/manifests/skipped_oracle_issue.json` under
    `capi015_multistep_limitation`, own no cell and owe no entry — and each would
    derive a **nonzero** base (~569.97) if ever promoted, i.e. `WindGens × steps`
    new in-scope cells and a re-derived entry count. What r4133 renders on those
    two is **not** claimed: no channel has ever run them, and both type
    `QMode=2` with a real `VV_Curve=`, so they take the volt-var arm
    (`WindGen.pas:1289-1319`) rather than the `Else kvarCalc := 0`
    (`:1320-1321`) the five census decks take. Since this sub-step the
    whole decomposition is **derived, not transcribed**:
    `props_r4133_replay::the_rp32_census_decomposition_is_read_off_the_corpus`
    sweeps every corpus `.dss` for `WindGen` declarations — quoted spelling
    included, and `Edit`/`BatchEdit` lines count as the same element scope since
    the audit settlement — reads each one's `kW=`/`pf=`/`kVA=`/`kvar=` and
    `QMode=`/`VV_Curve=` tokens there, re-derives the base through the two
    `WindGen.pas` branches, splits on the
    lock's `steps=`/`engines=`, reconciles per spelling and in total against the
    frozen extracts (each of whose r4133 columns must be `0`), and requires the
    in-scope cases to be exactly the cases the drafted entries cite. One measured
    correction to the hand-written table it replaced: the QSTS deck writes
    `PF=0.88` explicitly on a `~` continuation rather than inheriting it.
  - **The exclusion shape is a ledger entry, never an echo row** — the getter is
    live, so an echo row would misname the mechanism, and `props_norm.rs` carries
    none for this pair (only `windgen.dynout`). Per §1.1(e) the **four** entries
    (one per diverging case; the fifth deck owes none) are **drafted here and
    land in RP4.1's unmask commit**. Verbatim, to be copied into
    `tests/corpus/ledger.json` at RP4.1 (*copied verbatim and **landed
    2026-09-03** in RP4.1's commit `59e521e5`, with the cause below; all four
    hit — 4 in-scope cells — on the first unmasked run*):

    ```json
    "windgen-kvar-renders-dispatched-q": "EPRI r4133 renders WindGen property 11 `kvar` from the DISPATCHED reactive power instead of the base kvar the property documents: GetPropertyValue arm 11 (Version8/Source/PCElements/WindGen.pas:2896) is Format('%.6g',[presentkvar]) and Get_Presentkvar (:2297-2300) returns WindGenvars.Qnominalperphase*0.001*Fnphases. The Edit arm EXISTS (:629 `11: Presentkvar := Parser.DblValue`) and Set_Presentkvar (:2996-3009) stores the value in kvarBase, so this is NOT an echo: probed live on r4133 via epri-worker, a deck typing `kvar=500` renders `0`, and after `Edit kvar=777` the render is still `0` while `? PF` renders 0.968058 (= 3000/sqrt(3000^2+777^2)) — the value IS parsed and the getter reports a different live field. The rendered zero on these decks comes from a second, independent upstream defect: the steady-state `case WindModelDyn.QMode` (:1276-1322) implements arms 1 (PF) and 2 (Volt-Var) but has NO arm 0, while QMode defaults to 0 (:1020) and both the property help (:429-430 'Q control mode (0:Q, 1:PF, 2:VV).') and WTG3_Model.pas:252 document 0 as constant-Q — so `Else kvarCalc := 0` (:1320-1321) zeroes the dispatch and the getter reports that zero. The render is wrong independently of it: with QMode=1 the same deck renders 363.54 for a typed kvar=500 (the operating-point Q), and in dynamics — where :1254 skips the Q block — it renders Set_Presentkvar's intermediate 777 while kvarBase is 792.718441186736 and the measured terminal Q is -37087.76 kvar. The sibling class settles it inside the same trunk: Generator has the identical Get_Presentkvar (Generator.pas:2402-2405) and the identical property help (:396) yet renders the base (Generator.pas:3018, Format('%.6g',[kvarBase])). Consequence beyond the API: TDSSObject.SaveWrite reads PropertyValue[] (DSSObject.pas:156), which is virtual-dispatched to this getter (DSSObject.pas:117-120), so `Save Circuit` writes `kvar=0` for a machine built with kvar=500 (produced on r4133) and the reload resets PFNominal to 1.0 and kvarMax/kvarMin to 0 (:3001-3008) — silent model corruption. dss_capi 0.14.5 has no WindGen class at all, so there is no second oracle witness; the port renders kvar_base (crates/dss-core/src/elements/pc/windgen/accessors.rs:431), exactly as its Generator does and as r4133's own Generator does. Per the 2026-08-02 policy the port keeps the correct behavior and the upstream defect is reported (investigations/to_opendss/44-windgen-kvar-renders-dispatched-q.md, local) + excluded here + pinned by the expected-value pins named in `source`. The two engines' SOLVED state is unaffected and stays fully compared: probed terminal powers agree on every windgen deck (power-flow Q -2.1e-05/-4.2e-05 kvar on both; dynamics -37087.81 vs -37087.76 and -29216.72 vs -29216.67 kvar). Exact-pair (a discrete value jump, not display precision)."
    ```

    ```json
    {
      "id": "r4133-windgen-kvar-dispatched-daily",
      "case": "modes:windgen/windgen_daily.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^windgen\\.w1\\.kvar$",
          "rust": "986.05231553659",
          "oracle": "0"
        }
      ],
      "cause_ref": "windgen-kvar-renders-dispatched-q",
      "source": "R4133_PROPS_PLAN RP3.2 (2026-08-24), drafted in the sub-step and landed at RP4.1 per §1.1(e) — earlier it would fail assert_all_hit as NEVER APPLIED, the r4133 property compare being masked until the unmask. 1 in-scope cell (steps=1, one WindGen.w1): ours 986.05231553659 = kW*sqrt(1/pf^2-1) for the deck's kW=3000 pf=0.95 (SyncUpPowerQuantities, kVA not set) vs r4133's dispatched 0, max_rel 9.86e+02 — the pair's worst cell, bins.tsv:303. Replacement pin: props_r4133_pins.rs::windgen_kvar_renders_the_base_on_the_daily_deck.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    ```json
    {
      "id": "r4133-windgen-kvar-dispatched-delta",
      "case": "modes:windgen/windgen_snap_delta.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^windgen\\.w1\\.kvar$",
          "rust": "726.483157256779",
          "oracle": "0"
        }
      ],
      "cause_ref": "windgen-kvar-renders-dispatched-q",
      "source": "R4133_PROPS_PLAN RP3.2 (2026-08-24), drafted in the sub-step and landed at RP4.1 per §1.1(e). Same getter on the delta snapshot deck: 1 in-scope cell, ours 726.483157256779 = 1500*sqrt(1/0.9^2-1) vs r4133's 0. The fifth windgen deck, modes:windgen/windgen_snap.dss, needs NO entry: it types pf=1.0, so our kvar_base is 0 and both engines render `0` — a value coincidence, not agreement (typing `kvar=777` there diverges 777 vs 0, probed). Replacement pin: props_r4133_pins.rs::windgen_kvar_renders_the_base_on_the_delta_snapshot.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    ```json
    {
      "id": "r4133-windgen-kvar-dispatched-dyn",
      "case": "modes:windgen/windgen_dyn.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^windgen\\.w1\\.kvar$",
          "rust": "854.95263026673",
          "oracle": "0"
        }
      ],
      "cause_ref": "windgen-kvar-renders-dispatched-q",
      "source": "R4133_PROPS_PLAN RP3.2 (2026-08-24), drafted in the sub-step and landed at RP4.1 per §1.1(e). The kVA-set derivation: the deck types kW=1500 kva=1800 and no pf, so Create's PFNominal 0.88 (WindGen.pas:917) drives RecalcElementData's kVANotSet=false branch (:1377-1378) to kWBase 1584 and kvarBase sqrt(1800^2-1584^2) = 854.95263026673, vs r4133's 0. This deck is also where the upstream render is provably stale rather than merely wrong: :1254 skips the Q block in dynamics, so after `Edit kvar=777` r4133 renders Set_Presentkvar's intermediate 777 while its own kvarBase is 792.718441186736 (the port's value, reached through the identical PFNominal 0.897802) and the measured terminal Q is -37087.76 kvar. Replacement pin: props_r4133_pins.rs::windgen_kvar_renders_the_base_on_the_dynamics_deck.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    ```json
    {
      "id": "r4133-windgen-kvar-dispatched-dynfault",
      "case": "modes:windgen/windgen_dyn_fault.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^windgen\\.w1\\.kvar$",
          "rust": "854.95263026673",
          "oracle": "0"
        }
      ],
      "cause_ref": "windgen-kvar-renders-dispatched-q",
      "source": "R4133_PROPS_PLAN RP3.2 (2026-08-24), drafted in the sub-step and landed at RP4.1 per §1.1(e). The fault-ride-through twin of r4133-windgen-kvar-dispatched-dyn — same kW=1500 kva=1800 pf-default derivation, same 854.95263026673 vs 0, on the deck whose sustained 3ph fault drives the LVPL/LVQL path (terminal Q -29216.72 kvar on the port vs -29216.67 on r4133, i.e. the solved state agrees and only the render diverges). Entries are per case, so this one is its own. Replacement pin: props_r4133_pins.rs::windgen_kvar_renders_the_base_on_the_fault_ride_through_deck.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    Shape checked against the loader (`corpus_gate/ledger.rs`): key
    `format!("{element_lower}.{prop_lower}")` → `windgen.w1.kvar` (`:1051`); no
    `num_rel`, so the **exact-pair-numeric** arm, which requires `rust`
    (`:1088-1107`); `oracle` is asserted against the live upstream render
    (`:1057-1064`), so `"0"` must stay exact; `cause_ref` must resolve to a
    `causes` key (`:1453-1460`); and each case's `engines` must contain the
    channel (`:1426-1435`) — all four are `engines: "r4133"`.
  - **The pins** (`crates/dss-core/tests/props_r4133_pins.rs`, both lanes, no
    oracle), four, one per drafted entry, each running the gate's own sequence
    (compile + the one `solve` the case's `steps=1` rigor prescribes — the gate
    issues that solve itself for every case, `corpus_gate/runner.rs:348-349`, and
    none of the four decks ends on the solve of its own mode: the daily and the
    two dynamics decks end at `Set mode=…` and `windgen_snap_delta.dss` at
    `Calcvoltagebases`, while the two dynamics decks do carry a *snapshot*
    `solve` before their `Set mode=dynamic`) and then the same `?` getter the
    property walk reads:
    `windgen_kvar_renders_the_base_on_the_daily_deck` (`986.05231553659`, then
    `edit kvar=777` → `777` **and** `PF` → `0.968057839822749`, the reading that
    makes the assertion about the value rather than about a constant),
    `windgen_kvar_renders_the_base_on_the_delta_snapshot` (`726.483157256779` →
    `777`, plus the load-bearing third reading on `windgen_snap.dss`: `0` unset,
    `777` once typed — the `civanlar.dss` analogue),
    `windgen_kvar_renders_the_base_on_the_dynamics_deck` (`854.95263026673`, then
    `edit kvar=777` → **`792.718441186736`** at `PF 0.897802095552545`, the
    `kVANotSet=false` re-derivation r4133 also performs and then fails to render)
    and `windgen_kvar_renders_the_base_on_the_fault_ride_through_deck`. All four
    are cited from `props_r4133_replay::LEDGER_ENTRY_PINS` (2 → **6** rows, the
    literal pin extended), so the union guard
    `every_echo_row_pin_is_a_test_that_exists` still matches both ways.
  - **The accounting move: nothing is claimed.** A LEDGER outcome does not empty
    its rows — the exclusion lands at RP4.1 — so `DECLARED_RP3` stays
    **`(7, 4, 7)`** and the `RP3_ROUTING` row keeps its `3, 3` columns, its
    verdict flipped `OPEN` → **`LEDGER — RP3.2 (…)`**. The settled set pinned in
    `the_bin7_root_cause_pairs_are_routed_to_their_sub_steps` was then
    `[("swtcontrol.delay", "RP3.1"), ("windgen.kvar", "RP3.2")]` (RP3.3 put
    `("generator.model", "RP3.3")` at its head the same day); the table's
    state is `generator.model` 1/1 (RP3.3, open), `gictransformer.r2` 1/1 (RP3.4,
    open), `swtcontrol.delay` 2/2 (RP3.1, settled — LEDGER),
    `windgen.kvar` **3/3 (RP3.2, settled — LEDGER)**. RP3.2 is the first
    sub-step to exercise `RP3_SETTLED_SHAPES` as a *taxonomy* rather than as
    RP3.1's shape: the guard's `LEDGER` branch checked the r4133 citation, the
    `RP4.1`/`§1.1(e)` staging clause, the four witnesses as whole identifiers and
    the absence of an echo row.
  - **Test count 8 384 → 8 394** (4 192 → **4 197** per lane, **+5**):
    `props_r4133_pins` 33 → **37** (the four pins), `props_r4133_replay`
    123 → **124** (the census derivation), single-binary each, no test deleted,
    `#[ignore]`d or loosened. Every new assertion was mutation-probed and red as
    intended: the daily pin's expected value flipped one digit; the verdict's
    derived `4 cells, all 4 in scope` mutated to `5`; a witness name dropped from
    the verdict (`names_identifier` red). The completeness sweep proved itself
    *unprompted* — it rejected the inherited claim that neither vendored deck
    types `pf=`, which is how the QSTS `PF=0.88` correction above was found.
    Report: `investigations/to_opendss/44-windgen-kvar-renders-dispatched-q.md`
    (gitignored, local-only — verified absent from the commit).
  - **Flagged, deliberately NOT acted on — and, since the audit settlement,
    OWNED by plan §RP3.10.** **D2 — the missing steady-state `QMode=0`
    (constant-Q) arm, reproduced by the port.**
    `WindGen.pas:1276-1322` implements arms 1 and 2 only; `QMode` defaults to 0
    (`:1020`); the help (`:429-430`) and `WTG3_Model.pas:252` both document
    `0 -> Constant Q`, and the dynamics model implements it (`:1059-1061`,
    `Qord := Qref`); `Else kvarCalc := 0` (`:1320-1321`) therefore zeroes the
    dispatch for every default-configured WindGen, and the port ports it verbatim
    (`windgen/nominal.rs:223-225`) — which is why the corpus gate's power channel
    is green. New probe evidence that it is an omission, not a design choice:
    r4133 dispatches correctly the moment an arm exists (`QMode=1` → −363.54 kvar
    below the aero cap, −985.69 kvar on the daily deck). Under the 2026-08-02
    policy a reproduced upstream bug may not stand, but fixing it **moves solved
    powers on the r4133-gated windgen decks** — directly on the two power-flow
    ones, and on the two dynamics decks only through the snapshot solve they run
    before `Set mode=dynamic` (`:1254` skips the Q block in dynamics), which
    §RP3.10 measures rather than predicts — and is far outside RP3.2's
    property-render scope. It does not interact with this landing: the port
    renders `kvar_base`,
    which the dispatch never touches, so the four drafted `rust` values survive a
    future D2 fix unchanged. **The audit round refused to leave it at "flagged"**
    — a LEDGER outcome bars product-crate bytes, so the site
    (`windgen/nominal.rs:223-225`) carries no note and the item would have lived
    only in prose. It is now plan **§RP3.10** ("the reproduced `QMode=0`
    dispatch"), `opus-xhigh`, with its own precondition (**the user's go-ahead**,
    since it is a both-lane behavior change owing per-case *power*-channel ledger
    entries) and its own place in the ordering: it blocks **§RP5.2**, the closing
    record, and **not RP4.1** — measured, because the unmask compares properties
    and our `kvar` render reads `kvar_base`
    (`elements/pc/windgen/accessors.rs:431`), which the dispatch never writes.
    Two smaller flags, corpus/manifest bytes being
    off-limits this sub-step: `modes/windgen/windgen_snap_delta.dss:3` and its
    manifest note claim "the QMode=0 (constant-Q via PF) reactive dispatch … PF
    at unity here because the aero cap makes kvarCalc saturate" — **both clauses
    are wrong** (there is no arm 0; the `Else` yields 0 outright, now measured);
    and `WindGen.pas:2954`'s `MakePosSequence` gates on `PrpSequence^[19]/[20]`
    and emits ` maxkvar=`/` minkvar=`, but WindGen's properties 19/20 are
    `UserData`/`DutyStart` (`:394`, `:397`) — Generator's indices, a copy/paste
    leftover (`windgen/mod.rs:388` already notes the port registers no such rows).
  - **Audit settlement (2026-08-24, same day).** Ten minor findings from the
    `audit-code`/`audit-tests` pair — eight distinct issues once the two
    duplicated pairs are merged — settled in one commit as **2 hardened readers +
    3 new assertions + one newly owned follow-up**, plus the record corrections
    they imply. None touched the sub-step's premise (LEDGER stands: the probe,
    the census and the four entries are unchanged), and the settlement is again
    **zero product-crate bytes, zero `ledger.json` bytes**, no golden / manifest
    / `population.lock` / frozen-extract byte, no mask move. Test count is
    unchanged at **4 197 per lane** — every new assertion lives inside the
    existing census derivation.
    - **The completeness sweep read only unquoted declarations.**
      `windgen_facts` recognized an element by `lower.starts_with("new
      windgen.")`, so the quoted form `New "WindGen.w1"` — which the vendored
      corpus does use for other classes (`Test/IndMachTest.DSS:108`, `New
      "IndMach012.windgen1"`) — would have walked past "exactly four entries"
      unnoticed. Both census readers now share `element_scope`, which accepts the
      quoted spelling; `swtcontrol_facts` (RP3.1's) inherits the fix.
    - **"No deck types `kvar=`" was swept over declarations only.** An `Edit
      WindGen.w1 kvar=500` was invisible to the reader, and the audit proved it:
      that mutation on `windgen_daily.dss` left the whole replay binary green
      (124 passed) while the port's render moved. `element_scope` now treats
      `Edit`/`BatchEdit` as the same element scope, so the claim is swept over
      the lines that can make it false — the mutation is red (`measured ==
      cited`), on top of the pin that already caught it.
    - **The held-out pair's argument leaned on an unmeasured r4133 value.**
      `RP32_WINDGEN_SKIPPED_DECKS`' doc (and STATUS) said each vendored deck
      "would derive ~569.97 **against r4133's `0`**", but nothing has ever run
      them, and their mechanism is not the census's: both type `QMode=2` with a
      real `VV_Curve=`, i.e. the volt-var arm (`WindGen.pas:1289-1319`), not the
      `Else kvarCalc := 0` (`:1320-1321`) the five census decks take. The claim
      is now the nonzero base alone (`WindGens × steps` cells and a re-derived
      entry count on promotion, not "a cell"), and the `QMode=` token is read off
      every deck by the new `windgen_dispatch`: the five must select **no** arm
      (that IS where r4133's `0` comes from — the mechanism is derived now, not
      asserted) and the two held-out ones must be the volt-var pair the doc
      argues from.
    - **The pins' own justification was false for three of the four decks.**
      "The four decks end at `Set mode=…` and carry no solve of their own" holds
      only for `windgen_daily.dss`: `windgen_snap_delta.dss` ends at
      `Calcvoltagebases`, and both dynamics decks carry a *snapshot* `solve`
      before their `Set mode=dynamic`. The pins are unaffected — the gate issues
      the mode solve itself for every case (`corpus_gate/runner.rs:348-349`), and
      the pins do exactly that — but both copies of the wording (the pins' block
      comment and STATUS) now say what the decks contain.
    - **Citation drift, corrected in every copy.** `Set_Presentkvar`'s "init to
      something reasonable" line is `WindGen.pas:3002`, not `:2998` (a `Var`
      declaration) — wrong in the dynamics pin's doc comment and in the local
      report 44, fixed in both. Re-checking the report's Pascal block turned up a
      second one, report-only: its volt-var arm is `:1289`, not `:1300`. The
      report also attributed the daily deck's `QMode=1`
      flip (terminal Q −985.69 kvar) to its own reproduction deck, which measures
      −363.54 — the two probes are now separated there, as they already were
      here and in the routing verdict.
    - **The commit message of `c46bca42` welded two probe steps.** It reads "a
      typed `kvar=500` renders 0 while PF moves to 0.968058"; at `kvar=500` r4133
      renders PF `0.986394` (= 3000/√(3000²+500²)), and `0.968058` is the result
      of the *later* `Edit kvar=777`. The tree's own records (this section, the
      `RP3_ROUTING` verdict, the drafted cause) already split them correctly, so
      the message stands as written and the correction is recorded here rather
      than by rewriting the sub-step's commit.
    - **D2 stopped being a flag and became a sub-step.** The port reproduces
      r4133's missing steady-state `QMode=0` arm, which the 2026-08-02 policy
      forbids in any lane; RP3.2 could not fix it (a LEDGER outcome bars
      product-crate bytes, so not even a note at the site), which left a real
      policy item living in prose. It is now plan **§RP3.10**, with an explicit
      precondition (the user's go-ahead) and an explicit place in the ordering
      (blocks §RP5.2, not RP4.1). See the flagged bullet above.

- **RP3.3** (2026-08-24) — `generator.model`: **a getter with no arm, echoing the
  deck's own token past a live conversion.** **Zero product-crate bytes** (both
  engines' live state is identical), **zero `ledger.json` bytes**, zero golden /
  manifest / `population.lock` / frozen-extract bytes, and no r4133 mask moved —
  four test files (one of them a single doc-comment count) plus docs. The plan's
  three outcomes resolved to **ECHO** —
  the first sub-step to take that tag, RP3.1 and RP3.2 having both been
  `LEDGER`; the kill criterion did **not** fire
  (a live-field DLL getter separates the model state from the rendered string,
  and the census's factual basis re-verified exactly: 2 cells, exactly
  `modes:ncim/ncim_pv_pq.dss` + `modes:ncim/ncim_midi.dss`).
  - **The plan's premise was wrong, and disproving it was the sub-step.** Plan
    §RP3.3 (and the `RP3_ROUTING` `OPEN` verdict, and the sub-step's own state
    file) said r4133 "later **takes it back to 3**"
    (`Version8/Source/Common/Solution.pas:1760`). The *line* exists —
    `ReversePQ2PV` (`:1743-1768`, declared `:372`) carries the comment
    `// Takes it back to model 3` — but the procedure **has no caller anywhere in
    the trunk**. Exhaustive grep over the vendored r4133 tree returns the
    declaration, the definition, and `VersionC/Common/Solution.cpp:1360` plus
    `VersionC/Common/Solution.cpp:827`, which is the call **commented out**
    (`// ReversePQ2PV(ActorID); - not needed for now (04/01/2024)`).
    `DoNCIMSolution` (`:1095-1161`) ends at its `Until` with nothing after it;
    the sibling `DistGenClusters` (`:1687-1723`) is dead the same way. So a
    Q-clamped generator's live `GenModel` stays **4** after a converged NCIM
    solve, and the only live reversion is the in-loop `:2229`, which needs
    `not myPQOK` (`VNode > myVMax`) and cannot fire on decks clamped *upward*.
  - **The render, and why it is `EchoParse`.** `TGeneratorObj.GetPropertyValue`
    (`Version8/Source/PCElements/generator.pas:3007-3038`) has arms
    3,4,5,7,8,9,13,19,20,26,27,34,36,37,38,40..46 and **no arm 6**, so index 6
    falls to `ELSE Result := Inherited` (`:3035-3036`) →
    `General/DSSObject.pas:112-115` `Result := FPropertyValue[Index]`. The only
    writers of that slot are `InitPropertyValues`' `'1'` (`:2559`) and the `Edit`
    loop's unconditional store write (`:625`) — which runs *before* the CASE
    assigns the live field at `:643`. Both decks type `model=3` explicitly, so
    the store holds the deck's own token, not the class default: `EchoParse`, the
    `relay.reset` shape, not `EchoDefault`. The census read path is exactly that
    getter (`crates/dss-epri/src/capture.rs` `? <element>.<prop>` →
    `Executive/ExecHelper.pas:1755` `ActiveDSSObject.GetPropertyValue`).
  - **The probe — live r4133 DLL through `epri-worker`** (banner
    `Version 11.0.0.1`, `oracle.rev = r4133`), reading a channel independent of
    the property string: `GeneratorsI(9, 0)` is
    `Result := TGeneratorObj(Active).GenModel` (`DDLL/DGenerators.pas:125-134`),
    the field itself. `errno = 0` on every call.
    - decks as shipped: live `GenModel` **4**, render **`'3'`**, `kvar` 0.0,
      converged in 4 iterations, on both;
    - the same decks minus the trailing `Solve`: live 3 → **4** across the solve
      while the render is frozen at `'3'` the whole time (pre-solve present kvar
      `431.79425771046976` / `323.84569328285227`);
    - a **second** `Solve`: still 4 — the behavioural confirmation that
      `ReversePQ2PV` is dead;
    - **the decisive pair, neither of which runs the solver.**
      `GeneratorsI(10, 4)` writes the live field and nothing else
      (`DGenerators.pas:135-147` never touches `PropertyValue`): live → 4, render
      **stays `'3'`** ⇒ the getter does not read `GenModel`. `Edit Generator.g1
      model=4` writes the store (`generator.pas:625`): render **becomes `'4'`**
      ⇒ the getter reads `FPropertyValue[6]`. `Dump` writes `~ model=3` and
      `Save Circuit` writes `model=3` after the converged solve — the same store,
      so r4133's own round trip re-creates a model-3 generator.
    - control: `ncim_pq.dss` has 0 generators live, as its deck says.
  - **FIX and LEDGER were refuted by measurement, not by argument.** The port
    leg (temporary integration test, public API only, removed afterwards)
    reproduces r4133 digit for digit: render 3 → 4 across the solve, present kvar
    `431.79425771046976` / `323.84569328285227` before and `0` after, 4
    iterations, on both decks. So there is no behavioural divergence to fix **on
    any compared channel** — the one surface no channel compares, `Save`/`Dump`
    re-serialization, is bounded and owned by §RP3.11 (the settlement bullet
    below); and
    the `GeneratorsI(10, 4)` probe rules out "live but wrong", which is the only
    shape a ledger entry would have fitted. The port renders the live field
    (`obj/props/class_props/value.rs` → `elements/pc/generator/accessors.rs`) and
    its NCIM (`solution/solution/ncim.rs`) ports r4133's *executed* code
    loop-for-loop, including not restoring the model — which is now known to be
    correct rather than a gap.
  - **Landed: one echo row, one pin, one derivation.** The row is
    `echo("generator", "model", EchoParse, 2, …, Pin(…))`, the **82nd** in
    `PROPS_ECHO_R4133` and the first contributed by a WP-RP3 sub-step rather than
    by RP2.3's declared bucket. Its witness can only be a pin: both cells sit on
    `engines: "r4133"` cases, so the capi channel never value-compares them (a
    `Capi(n)` witness would be the witness-that-cannot-exist the RP2.3 settlement
    made a test) — and `ECHO_ROWS_ON_R4133_ONLY_CASES` gains
    `("generator", "model", 2, 2)`, its most extreme entry, where *every* cell of
    the pair is capi-blind. The pin
    `generator_model_renders_the_live_pv2pq_conversion` covers **both** decks
    (`ncim_midi.dss` had no unit pin at all): it reads `maxkvar` first, which
    names *which* conversion this is (the `:2120` Q-band promote, not the
    `:1935` zero-limits one), asserts the render `4`, types the deck's own token
    back (`edit … model=3` → `3`) and then **re-solves** — `4` again, so the pin
    is on the engine's field and not on the parser's echo.
  - **The census derivation.** `the_rp33_census_decomposition_is_read_off_the_corpus`
    (RP3.1/RP3.2's precedent) derives the `2` instead of transcribing it: the new
    reader `sets_ncim` sweeps all 1 000+ corpus decks for a live `set
    algorithm=NCIM` line — a line mentioning `algorithm` in an unparsable
    spelling is a hard error, not a silent miss — and finds exactly **five**;
    `generator_model_facts` then reads each survivor's `Generator` count and
    `model=` token. Products: `cells = generators × steps` = 1 + 1 = **2**, both
    `engines=r4133` ⇒ 2 in scope, reconciled against `bins.tsv`'s 2/2 and the
    single frozen example row `'4'` vs `'3'`. Two things are *derived* rather
    than asserted: `ncim_pq.dss` runs NCIM and declares no generator, hence no
    cell (the control — the mechanism alone makes nothing); and per case,
    r4133's frozen side must equal the deck's own typed token, which **is** the
    `EchoParse` claim. **Held out, named:** the corpus's two other NCIM decks —
    `IEEETestCases/IEEE118Bus/master_file.dss` (53 `model=3` generators) and
    `Examples/NCIM/Xmission_System_Kundur2Area/Master.dss` (3) — are
    `kind: "large"` and therefore outside the census population ("every live
    case, **non-large**, non-pending/abort/defer",
    `tests/corpus/props_r4133/triage.md` §Method), asserted from
    `population.lock.json`. Their generators live in *redirected* files, so the
    test reads 0 on each master and the real count on the sibling — the reason
    the table carries a sibling column at all.
  - **The accounting move: RP3.3 is the first sub-step that shrinks the
    bucket, and the tag is why.** An ECHO outcome ships its exclusion in the
    sub-step's own commit, so `Link::Echo` claims `generator.model`'s example row
    immediately and `declare` never routes it to `Owner::Rp3` again:
    `DECLARED_RP3` **`(7, 4, 7)` → `(6, 3, 6)`** and the `RP3_ROUTING` row's
    counted columns go **`1, 1` → `0, 0`** while the entry itself stays (the
    table must cover all four `BIN7_ROOT_CAUSE` pairs). Everything else moves
    with it: `PROPS_ECHO_R4133` 81 → **82**, `ECHO_PARSE_ROWS` 7 → **8**,
    `R4133_ONLY_ROWS`/`R4133_ONLY_CELLS` 57 / 34 969 → **58 / 34 971**,
    `CLAIMED_ECHO` 169 → **170** (`CLAIMED_TOTAL` 2 974 → **2 975**, derived) and
    `CLAIMED_SPELLINGS_LIVE` 2 981 → **2 982** — **re-measured**, not bumped: the
    full claims census (`DSS_PROPS_CENSUS=claims`, 439 cases × 2 channels, 65 s)
    reports `echo-row` at **488 019 cells / 468 046 in scope, 170 spellings, 82
    pairs** and `UNCLAIMED` down to **1 722 / 887 / 58 pairs**, and the r4133
    channel's claimed spellings sum to exactly 2 982. `LEDGER_ENTRY_PINS` stays
    at **6 rows** — an ECHO sub-step owns none, which is what the guard's
    per-tag branch enforces.
  - **The guard needed two deliberate extensions, both hardening.** A `0, 0`
    entry breaks two assumptions the routing test made when every settled outcome
    was a LEDGER. (1) Its middle term compared `RP3_ROUTING.len()` against
    `Ledger::owner`'s *pair* count; the table keeps four entries while the ledger
    now knows three, so the term is the entries that still declare rows. (2) The
    live re-measurement (`seen`) no longer contains the pair at all — which on
    its own would make "0, 0" untestable — so a zero-row entry now carries a
    **positive** obligation: its rows must still exist in the corpus, and
    **every** one must be claimed by `Link::Echo` specifically, so a pair that
    vanished for any other reason reds here. A zero-row entry must also be tagged
    `ECHO`; an `OPEN` sub-step may never declare zero rows.
  - **Test count 8 394 → 8 398** (4 197 → **4 199** per lane, **+2**):
    `props_r4133_pins` 37 → **38** (the pin), `props_r4133_replay` 124 → **125**
    (the census derivation), no test deleted, `#[ignore]`d or loosened. Every new
    assertion was mutation-probed and red as intended: the pin's expected `4`
    flipped to `3`; the deck-fact table's generator count perturbed 1 → 2 and the
    held-out sibling's 53 → 52; the verdict's derived `2 cells, all 2 in scope`
    mutated to `3`; the echo row renamed away (the row-set lock, the witness and
    pin guards, the claim accounting and the routing guard's ECHO obligation all
    red at once); and the `RP3_ROUTING` counts restored to `1, 1` (the bucket
    lock red, `(7, 4, 7)` against `(6, 3, 6)`). No upstream report was filed and none was owed:
    the divergence is r4133 rendering its own parse store, i.e. the same class as
    the 81 rows RP2.3 landed without reports.
  - **Corrections this sub-step owes elsewhere.** The plan's §RP3.3 text, the
    `RP3_ROUTING` `OPEN` verdict and this record's own inherited framing all said
    `Solution.pas:1760` "takes it back to 3". It is dead code; the live reversion
    is `:2229`. The plan now carries the as-executed note that says so, and the
    settled verdict states it in the tree.
  - **Audit settlement (2026-08-24, one commit over `fb0e9e7f`).** Seven minor
    findings from the two audit agents, all settled — none waved off, and none
    touched the classification: both auditors re-derived the probe and ECHO
    stands. Still **zero product-crate bytes**, zero `ledger.json` / golden /
    manifest / `population.lock` / frozen-extract bytes, no mask moved.
    - **Two stale hand-offs to RP4.1, corrected in both copies.** The plan's
      §RP4.1 precondition 2 and the tripwire
      `the_staged_r4133_property_entries_have_not_landed_yet` both still listed
      RP3.3 among the sub-steps whose staged ledger entries RP4.1 must land and
      retire. An ECHO outcome stages nothing and retires itself in its own
      commit, so RP4.1's real inheritance is RP1.4's, RP3.2's four and whatever
      RP3.4+ stages — exactly the discipline RP3.3 applied to the `:1760`
      citation and had not applied to its own outcome.
    - **`ECHO_ROWS_ON_R4133_ONLY_CASES`' `(2, 2)` is now derived, not merely
      described.** Its doc said the entry was derived by the census test; that
      test never read the entry, the table's `cases` column had **no** value lock
      anywhere, and the audit's mutation `(2, 2)` → `(2, 5)` shipped green.
      Landed: the accessor `props_norm::r4133_only_exposure`, read by
      `the_rp33_census_decomposition_is_read_off_the_corpus` against **both**
      derived columns; the table-wide sum lock `R4133_ONLY_CASES = 833` (column 3
      had `R4133_ONLY_CELLS`, column 4 had nothing); and the structural invariant
      `cases <= cells` per row — an exposed case contributes at least one cell.
      The audit's mutation now reds in two places at once.
    - **The NCIM completeness sweep's three escape hatches, closed rather than
      argued shut.** (1) *Abbreviated option names*: `Set` resolves through
      `TCommandList.GetCommand` → `THashList.FindAbbrev`, a linear prefix match
      (`Shared/Command.pas:53`/`:65` arm `AbbrevAllowed`,
      `Shared/HashList.pas:335-357`), so `set algo=ncim` selects NCIM in the
      engine and was invisible to a reader that knew only the full spelling —
      and invisible to its hard error too, which keyed on the literal substring.
      The reader now reads every non-empty prefix of `algorithm`, longest first,
      deliberately wider than the engine. (2) *Abbreviated values*: the reader
      tested `value == "ncim"` while `InterpretSolveAlg`
      (`Common/Utilities.pas:575-591`) compares `copy(lowercase(s), 1, 2)`, so
      `Set algorithm=nc` parsed and was then dropped — the new `selects_ncim`
      ports r4133's own two-character rule. (3) *The `.dss`-only universe*:
      `collect_dss` filters on the extension while the corpus really does
      `Redirect` `.txt` scripts, so a `Set algorithm=NCIM` inside one would run
      unswept; `redirected_non_dss_scripts` now walks the transitive
      `Redirect`/`Compile` closure by basename (11 files at HEAD — `WireData.txt`,
      the two `AllocationFactors*.Txt`, the LVTestCase's seven parts,
      `HW_Inverters.txt` — none of which names the option). All three are dormant
      on today's corpus, which is why the settlement also lands the self-test
      `the_ncim_sweep_reads_every_spelling_the_engine_accepts`: a dormant reader
      proves nothing about the completeness claim resting on it. Non-vacuity:
      reverting `selects_ncim` to the equality reds it, and the closure's live
      count is asserted `>= 5`.
    - **The one surface the sub-step could not close now has an owner: §RP3.11.**
      "No behavioural divergence" was true of every compared channel and the
      audit bounded it: r4133's `Save`/`Dump` print the same parse store, so its
      round trip re-creates the model-3 generator, while the port's serializer
      renders the LIVE field (`report/save/save.rs:34-53` goes through
      `ClassProps::get_value` where Pascal `SaveWrite` reads
      `PropertyValue[iProp]`, `General/DSSObject.pas:145-165`). Measured here on
      `ncim_pv_pq.dss` after the converged solve: the port writes
      `New "Generator.g1" PF=0.88 Bus1=genbus Phases=3 kV=12.47 kW=800 Model=4
      Maxkvar=1500 Minkvar=-1500 Vpu=1.01` against r4133's `… kW=800 model=3 …`
      — a re-compiled deck is a PQ generator instead of a Q-limited PV one, and
      the port's line also carries a `PF=0.88` the deck never typed (a
      `PrpSequence` difference, a second class of divergence). No oracle channel
      compares `Save` on these r4133-gating decks, so this is neither a
      regression of RP3.3's commit nor part of the echo classification: the new
      plan §RP3.11 owns both questions, runs **after RP4.1** (the echo table is
      the list of pairs where the two serializers can disagree) and blocks
      §RP5.2, not the unmask. The `RP3_ROUTING` verdict, the plan's as-executed
      note and this record all carry the bound now. *(Settled 2026-09-03 by
      §RP3.11 — `KEEP_LIVE_PINNED` on both surfaces — which also corrected the
      premise stated here: `PropertyValue[iProp]` resolves to the **virtual**
      `GetPropertyValue` (`General/DSSObject.pas:45`, `:117-120`), so r4133
      prints **live** values through 49 classes' hand-picked index sets and
      stores `model` only because `TGeneratorObj.GetPropertyValue` has no arm
      for it. Record in §1 above.)*
    - **A pre-existing flake in a gated binary, removed.** The audit measured
      `harness::props_norm::tests::the_value_chain_resolves_in_order_and_agrees_with_the_seam`
      failing ~1–3 % of runs with "the chain query moved a counter": it
      snapshotted the **process-global** seam counters around its body while
      sibling tests in the same binary deliberately drive those very seams, so
      "the five-command gate was green" could be luck. The counters keep their
      process-global role (the gate's live accounting reads them); the offline
      question — *did MY query reach a counting seam* — is now asked of a new
      per-thread counter `props_norm::seam_touches_here`, which libtest's
      one-thread-per-test model makes exact. Four assertions moved to it (the two
      offline-query tests, the value chain, and the echo seam's exact deltas);
      the "the SHIPPED statics moved" half stays on the globals as `>=`, the
      shape the floor seam test already used. Strictly stronger than what it
      replaces — no sibling can mask a real touch — and the floor test gained
      exact deltas it could not state before. `counter_totals` lost its last
      reader and was deleted rather than kept warm. Non-vacuity: making
      `record_touch` a no-op reds both seam-counting tests; 40 consecutive runs
      of the binary afterwards, 0 failures.
    - **Test count 4 199 → 4 200 per lane (8 398 → 8 400)**: the sweep
      self-test, `props_r4133_replay` 125 → **126**. No test deleted (the removed
      `counter_totals` is a helper, not a `#[test]`), none `#[ignore]`d, none
      loosened; the two `==`→`>=` moves on the global counters are paired with
      strictly exact per-thread assertions.

- **RP3.4** (2026-08-24) — `gictransformer.r2`: **the r4133 twin of an
  already-fixed, already-pinned divergence.** **Zero product-crate bytes**
  (`GOLDEN_REBASE_PLAN.md` G2.5 fixed the engine in both lanes on 2026-08-06),
  **zero `ledger.json` bytes**, zero golden / manifest / `population.lock` /
  frozen-extract bytes, no r4133 mask moved — two test files plus docs. Outcome
  **LEDGER**, and with it **all four bin-7 root-cause pairs are settled**.
  - **The r4133 source shares the capi slip line for line.** `RecalcElementData`
    fills the two conductances from the percentages when `FpctRSpecified`, and
    the winding-2 line reads the **H** winding's percentage:
    `Version8/Source/PDElements/GICTransformer.pas:495`
    `G2 := 100.0 / (FZBase2 * FPctR1);`, the byte-twin of pinned dss_capi 0.14.5
    `src/PDElements/GICTransformer.pas:441`. The reverse branch is right
    (`:497-498` / capi `:445-446`), which is what makes it a slip rather than a
    convention — the two maps are inverses only when the forward one reads
    `FPctR2` — and the creation defaults are independent (`%R1 = %R2 = 0.2`,
    `:458-459`). Which branch runs is set by the `Edit` `CASE`'s "specials":
    arms 13/14 (`%R1`/`%R2`, `:308-309`) set `FpctRSpecified := TRUE` (`:349`),
    arms 7/8 (`R1`/`R2`) set it FALSE (`:343`). On `type=Auto` the `busX` side
    effect promotes the X winding onto terminal 2 (`:323`, and `:340`), which is
    why these decks' whole solved model moves too — already excluded by the G2.5
    entries.
  - **The render is NOT an echo, and that is what fixed the outcome shape.** The
    plan's own text called it "the same un-honoured `%R2` **echo**", which would
    have licensed a `PROPS_ECHO_R4133` row. It is not one: property 8 is `R2`
    (`PropertyName^[8]`, `:130`) and `TGICTransformerObj.GetPropertyValue` arm 8
    is `Format('%.8g', [1.0/G2])` (`:723`; `DumpProperties` prints the same at
    `:663`) — a **live computation** off the mis-derived conductance. Property
    14 is `%R2` (`:136`) and renders the stored `FpctR2` (`:729`), which agrees
    with the port digit for digit — which is exactly why the census carries a
    cell on `r2` and none on `%r2`. capi 0.14.5 reaches the same number by a
    different road (`PropertyOffset[ord(TProp.R2)] := ptruint(@obj.G2)` +
    `TPropertyFlag.InverseValue`, `:233-234`), as does the port
    (`elements/pd/gic_transformer/accessors.rs:60` `R2 => self.g2` behind
    `PropFlags::INVERSE_VALUE`). **So no echo row was added; the exclusion is
    two drafted ledger entries.**
  - **Census, derived per element and closing over every cell.** `bins.tsv:230`
    `gictransformer.r2 numeric 7 - 2 2 2.50e-01 2.50e-01`; the single frozen row
    is `examples_full.txt:1439` `gictransformer.r2 | '0.09522' | '0.12696' | 2`.
    The whole corpus declares **22** GICTransformers in **4** files (and no
    non-`.dss` script declares one — the class is merely *mentioned* by **three**
    syntax-highlight files under `Version8/Distrib/Examples/SyntaxFiles/`
    (`opendss.stx:144`, `OpenDSS_syntax_NotepadPlusPlus.xml:35`,
    `…_V2.xml:28`), by the manifests and by the census extracts; the count was
    "two" as first written and is corrected here, the four-file conclusion being
    unaffected — the derivation sweeps the whole file universe, not this list):

    | case | rigor | declared | `%R`-spec'd | cells | in scope |
    |---|---|---|---|---|---|
    | `asymmetric:gic/gictransformer_gic.dss` | `micro steps=1 engines=both` | 3 (`tg1` GSU `R1=0.12`; `tg2` YY `R1=0.2 R2=0.1`; **`tg3` Auto `%R1=0.2 %R2=0.15`**) | 1 | **1** | **1** |
    | `asymmetric:gic/gic_midi.dss` | `midi steps=1 engines=both` | 3 (`tg1` GSU `R1=0.12`; `tg3` YY `R1=0.2 R2=0.1`; **`tg5` Auto `%R1=0.2 %R2=0.15`**) | 1 | **1** | **1** |
    | `solvable_now:…/GICExample/GIC_Example.dss` | `feeder steps=1 engines=both` | 15 (`T1…T15`, all ohms `R1=`/`R2=`) | 0 | 0 | 0 |
    | `modes:makeposseq/makeposseq_shunt.dss` | `micro steps=1 engines=capi_v0145` | 1 (`gt` GSU `R1=0.1`) | 0 | 0 | 0 |
    | **total** | | **22** | **2** | **2** | **2** |

    `ZBase2 = 138²/300 = 63.48 Ω`, so ours is `63.48*0.15/100 = 0.09522` against
    both oracles' `63.48*0.20/100 = 0.12696`, `rel = 0.25` — the frozen
    `2.50e-01`. `R1 = 396.75*0.2/100 = 0.7935` on **both** engines, which is why
    the census has no `gictransformer.r1` row at all. Two independent
    cross-checks of the same population read, both landed as assertions: the
    class-wide pairs `gictransformer.enabled` (`bins.tsv:45`) and
    `gictransformer.pctperm` (`:229`) each record **22 cells / 21 in scope** =
    `Σ declared × steps` and its r4133-gating subtotal, which owes nothing to the
    `%R` arithmetic; and the **positive measurement** (this sub-step's
    `civanlar.dss`) — the **20** ohms-specified GICTransformers, **19** of them
    on r4133-gating cases and four of them `type=Auto` like `tg3`/`tg5`, produce
    **zero** cells, because the ohms spec sets `FpctRSpecified := FALSE` and
    leaves the untouched reverse branch to answer. A cell needs `%R1 ≠ %R2`, and
    no corpus deck has the `%R1=`-alone blast-radius shape the cause blob warns
    about (asserted, not assumed).
  - **The other four G2.5 property surfaces stay capi-only — with one correction
    to the plan's stated reason.**

    | ledger entry | `property` scopes | case | `engines=` | why r4133 never sees it |
    |---|---|---|---|---|
    | `makeposseq-cuf-applied-capi-props` | 3 (`capacitor.cap_cmat.{cuf,normamps,emergamps}`) | `modes:makeposseq/makeposseq_shunt.dss` | **`capi_v0145`** | the channel is not gated at all |
    | `capi-generator-makeposseq-rating` | 4 (`generator.g_kva.{kva,mva}`, `generator.g_mva.{kva,mva}`) | `modes:makeposseq/makeposseq_pc.dss` | **`capi_v0145`** | same |
    | `capi-linespacing-normamps` | 2 (`line.lsp.{normamps,emergamps}`, + 2 `probe` scopes) | `asymmetric:line/line_spacing_asym.dss` | **`both`** (!) | the ledger holds `r4133-linespacing-asym-303`, `kind: "skip"` (EPRI #303 AV while compiling the `tscables=`/`wires=` spacing), so `ledger.rs::channel_is_skipped` makes `scheduler.rs:355-356` `continue` past the channel — **and** r4133's own `General/LineGeometry.pas:1235-1239` implements the min-over-phase rule the port follows, so there would be nothing to twin |

    2 (gic) + 3 + 4 + 2 = the plan's **11 property scopes over 5 entries**. The
    census confirms it from the other side: `line.normamps`/`line.emergamps` and
    `generator.kva`/`generator.mva` are **absent from `bins.tsv` entirely**, and
    `capacitor.normamps`/`.emergamps` record 4 cells / **0 in scope**. The
    plan's shorthand ("their decks do not gate r4133") is true of the two
    `makeposseq` cases and **false as stated** for `line_spacing_asym.dss`; the
    mechanism above is what the record carries.
  - **The two drafted entries, VERBATIM — they land at RP4.1 per §1.1(e), NOT
    here** (*landed 2026-09-03 in RP4.1's commit `59e521e5`, verbatim, on the
    pre-existing `gic-pct-r2-ignored` cause; both hit — 2 in-scope cells*)**.** Both reuse the existing `gic-pct-r2-ignored` cause unchanged (it
    already names the r4133 lines), and the ids mirror the capi originals'
    `-props` suffix because plain `…-r4133` is taken by the G2.5 solved-model
    exclusions. Schema checked against `corpus_gate/ledger.rs:1420-1560`.

    ```json
    {
      "id": "gic-pct-r2-honoured-gictransformer-r4133-props",
      "case": "asymmetric:gic/gictransformer_gic.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^gictransformer\\.tg3\\.r2$",
          "rust": "0.09522",
          "oracle": "0.12696"
        }
      ],
      "cause_ref": "gic-pct-r2-ignored",
      "source": "R4133_PROPS_PLAN RP3.4 (2026-08-24), drafted in the sub-step and landed at RP4.1 per 1.1(e) - earlier it would fail assert_all_hit as NEVER APPLIED, the r4133 property compare being masked until the unmask. The r4133-channel twin of gic-pct-r2-honoured-gictransformer-capi-props: EPRI r4133 carries the identical forward arm, Version8/Source/PDElements/GICTransformer.pas:495 `G2 := 100.0 / (FZBase2 * FPctR1);`, the byte-twin of pinned dss_capi 0.14.5 src/PDElements/GICTransformer.pas:441, and renders property 8 (`R2`, :130) as Format('%.8g',[1.0/G2]) from GetPropertyValue arm 8 (:723; DumpProperties prints the same at :663) - a LIVE read of the mis-derived conductance, not a parse-store echo, which is why the exclusion here is a ledger entry and not a PROPS_ECHO_R4133 row. Property 14 (`%R2`, :136) renders FpctR2 (:729) and agrees with the port, so only the derived ohms diverge. 1 in-scope cell (steps=1, the deck's one %R-specified GICTransformer, tg3): ours 0.09522 = ZBase2*%R2/100 = 63.48*0.15/100 for the `%R1=0.2 %R2=0.15 kvll1=345 kvll2=138 mva=300` of gictransformer_gic.dss:18-19, vs r4133's 0.12696 = ZBase2*%R1/100, max_rel 2.50e-01 (tests/corpus/props_r4133/bins.tsv:230). The deck's other two GICTransformers use the ohms R1=/R2= spec and the untouched else arm (:497-498), so they produce no cell. Replacement pin: props_r4133_pins.rs::gictransformer_r2_honours_the_x_winding_percentage.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

    ```json
    {
      "id": "gic-pct-r2-honoured-midi-r4133-props",
      "case": "asymmetric:gic/gic_midi.dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {
          "field": "property",
          "name_re": "(?i)^gictransformer\\.tg5\\.r2$",
          "rust": "0.09522",
          "oracle": "0.12696"
        }
      ],
      "cause_ref": "gic-pct-r2-ignored",
      "source": "R4133_PROPS_PLAN RP3.4 (2026-08-24), drafted in the sub-step and landed at RP4.1 per 1.1(e). The r4133-channel twin of gic-pct-r2-honoured-midi-capi-props - same getter (Version8/Source/PDElements/GICTransformer.pas:723 over the :495 slip), same bases, on tg5 of the 6-substation 345 kV ring: gic_midi.dss:27-28 declares `%R1=0.2 %R2=0.15 kvll1=345 kvll2=138 mva=300 type=Auto`, so ZBase2 = 138^2/300 = 63.48 ohm gives ours 0.09522 against r4133's 0.12696. Entries are per case, so this one is its own; 1 in-scope cell (steps=1). The ring's other two GICTransformers (tg1 GSU R1=0.12, tg3 YY R1=0.2 R2=0.1) are ohms-specified and produce no cell - 20 of the corpus's 22 GICTransformers are, 19 of them on r4133-gating cases, and not one of them diverges, which is the measurement that the %R path is the whole divergence class. Replacement pin: props_r4133_pins.rs::gictransformer_r2_honours_the_x_winding_percentage_on_the_ring.",
      "measured": {
        "date": "2026-08-24"
      }
    }
    ```

  - **No new upstream report, and that is a finding, not a skip.**
    `investigations/to_opendss/07-gictransformer-g2-uses-pctr1.md` (local,
    gitignored) is already written **against r4133** — its "The line at fault"
    section reads "As of SVN trunk r4133, `Version8/Source/PDElements/
    GICTransformer.pas`, lines 486-501" and its reproduction is exactly this
    surface (`? GICTransformer.t1.R2` → `0.12696`), with the `else`-arm inverse
    argument and the `type=Auto` `busX` blast radius. The Russian deep-dive is
    `investigations/issue-07-gictransformer-g2-pct-r1.md`. **The next free report
    number stays 45** (taken by RP3.5 on 2026-08-28; 46 is next).
  - **Pins (2, both lanes, no oracle):**
    `gictransformer_r2_honours_the_x_winding_percentage` (`tg3`) and
    `gictransformer_r2_honours_the_x_winding_percentage_on_the_ring` (`tg5`),
    one per drafted entry. Each asserts the actual `0.09522`, not "not
    0.12696", and carries the **discriminating** half the module doc demands:
    an `edit %R2=0.3` moves `R2` to `0.19044` (the setter drives this getter),
    and an `edit %R1=0.4` then moves `R1` to `1.587` while leaving `R2`
    **unmoved** — upstream, whose `G2` is a function of `FPctR1`, would answer
    `0.25392` there. That reading separates the two engines' *mechanisms*, not
    two numbers. Each pin also reads the deck's ohms sibling (`0.1`, the reverse
    branch), the GSU's `Create`-derived `0.38088`, and `%R2` itself (`0.15`, the
    reading that agrees with r4133 and proves the divergence is confined to the
    derived ohms).
  - **Accounting.** `RP3_ROUTING`'s `gictransformer.r2` row flips
    `OPEN` → **`LEDGER`** with the full verdict (r4133 unit citation, `RP3.4`,
    `RP4.1` + `§1.1(e)`, both pins as whole identifiers, the derived census, the
    positive measurement, the class-wide cross-check, the report pointer); its
    counted columns stay **`1, 1`** and `DECLARED_RP3` stays **`(6, 3, 6)`** —
    a LEDGER outcome stages, so the row remains on RP4.1's hand-edit list.
    `LEDGER_ENTRY_PINS` **6 → 8** rows (literal lock moved with it);
    the settled set in
    `the_bin7_root_cause_pairs_are_routed_to_their_sub_steps` becomes all four
    pairs in `RP3_ROUTING` order — the guard needed no relaxation for the
    all-settled state, and its message now says so. New derivation
    `the_rp34_census_decomposition_is_read_off_the_corpus` reads the two cells
    off the corpus (whole **file** universe, not just `.dss`), the
    `population.lock` rigor and the frozen extracts, plus the new per-element
    reader `gictransformer_elements` (`element_tokens` cannot serve: it collapses
    a class to one scope per deck and both gic decks type two different `R1=`)
    and its self-test
    `the_gictransformer_reader_separates_the_percentage_and_ohms_specs` — the
    `%R1=`/`R1=` separator rule is what selects the branch, so a reader that got
    it wrong would answer "zero cells, no entry", green and wrong. Single-claim
    holds: `props_norm` carries rows on `gictransformer.enabled` and
    `.pctperm` but **none** on `.r2`, and the standing
    `!has_echo_row("gictransformer", "r2")` assertion is kept with its comment
    re-worded to the finding. The staged-entries tripwire
    `the_staged_r4133_property_entries_have_not_landed_yet` now names RP3.4's two
    ids in its doc; `tests/corpus/ledger.json` is byte-untouched.
  - **Non-vacuity, proven by mutation (15, all reverted).** Pins: each asserted
    value flipped one digit (`0.09522`→`0.09523` on both decks, the GSU control
    `0.38088`, the `%R2` agreement reading `0.15`) and the discriminator
    rewritten `%R1=0.4`→`%R2=0.4` — five reds. Accounting: the verdict's derived
    split (`2`→`3` `%R` decks), the verdict orphaning its derivation test, the
    census token (`%R2` `0.15`→`0.2`), an ohms sibling's token
    (`R1=0.12`→`0.13`), a `%R` element re-classified as ohms-specified, a dropped
    element row (`GIC_Example` `T15`),
    the class-wide lock (`21`→`20`), the derivation reading `%R1` where it must
    read `%R2`, the outcome tag (`LEDGER`→`ECHO`) and a deleted
    `LEDGER_ENTRY_PINS` citation — ten more reds, three of them in two or three
    tests at once.
  - **Test count 4 200 → 4 204 per lane (8 400 → 8 408)**: `props_r4133_pins`
    38 → **40** (the two witnesses), `props_r4133_replay` 126 → **128** (the
    derivation plus the reader self-test); `props_r4133_evidence_lock` unchanged
    at 11. Both gate lanes green at 4 204 with 0 failed / 0 ignored / 0 filtered.
    No test deleted, `#[ignore]`d or loosened; no tolerance
    exists here to move (the pins compare rendered strings).
  - **Audit settlement (2026-08-24, one commit over `cab2e667`).** Seven minor
    findings from the two audit agents, all settled — none waved off. The
    classification is untouched: both auditors re-derived the mechanism and
    `LEDGER` stands, the census still decomposes to 2 cells / 2 in scope, and the
    two drafted twins are unchanged. Still **zero product-crate bytes**, zero
    `ledger.json` / golden / manifest / `population.lock` / frozen-extract bytes,
    no mask moved, and the test count stays 4 204 per lane (the settlement adds
    assertions, not tests).
    - **The ring pin's discriminator was half a pin, and is now whole.**
      `gictransformer_r2_honours_the_x_winding_percentage_on_the_ring` ended at
      "`%R1=0.4` leaves `R2` unmoved" — a reading a correct engine and a *no-op
      edit* satisfy alike, since the test never read `R1` back. The audit
      measured it: rewriting the edit to `%R9=0.4`, a property that does not
      exist, left the whole binary green (40 passed), while the same mutation on
      the micro-deck twin — which always carried the control — went red. The
      missing `tg5.R1 == "1.587"` reading is added, and the `%R9` mutation now
      reds by name. (The STATUS bullet above always described *both* readings for
      *both* pins; as of this settlement that description is true.)
    - **The census's `%R1 == %R2` branch had no counter, so it could only red as
      something else.** An element whose two percentages coincide renders the same
      number on both engines and carries no cell; the derivation `continue`d past
      it without counting it, so it landed in neither `ohms` nor the diverging
      set and the *next* cross-check failed with "nothing may fall between the
      two" — the wrong cause, and an invitation to relax that check instead of
      extending the classification. Measured both ways with one coordinated
      mutation (deck token `%R2=0.15`→`0.2` on `gic_midi.dss` `tg5`, its
      `RP34_GIC_ELEMENTS` row, and the frozen `bins.tsv`/`examples_full.txt`
      counts 2→1, all reverted): at `cab2e667` it reds `21 != 22`, "nothing may
      fall between the two"; now it reds naming the coincidence and listing the
      element. The three classes (`ohms` / `coincident` / `diverging`) are each
      counted and each adjudicated — `coincident` empty, the classification
      exhaustive, and **every diverging element on an r4133-gating case** (the
      second hole the audit named: a `%R` element diverging on a capi-only case
      would also have "fallen between").
    - **"In scope" no longer means `engines=` alone — the shorthand this very
      sub-step disproved.** RP3.4 found that `asymmetric:line/line_spacing_asym.dss`
      is `engines: "both"` and yet never compared on r4133, and then encoded the
      shorthand in its own guard. New reader `r4133_skipped_cases` applies
      `corpus_gate/ledger.rs::channel_is_skipped`'s own condition (`channel ==
      "r4133"`, `kind == "skip"`) to `tests/corpus/ledger.json` (read-only), and
      all **four** RP3 census derivations now ask both halves of the question
      (RP3.3's r4133-only cases assert the negative directly). No number moved —
      none of the RP3.1/RP3.2/RP3.3/RP3.4 cases is in today's four-entry skip set
      — and the second half is kept non-vacuous by a witness assertion on
      `r4133-linespacing-asym-303`: mutating the reader's channel filter to
      `capi_v0145` reds it.
    - **Three stale citations, corrected in every copy.** The class is mentioned
      by **three** syntax-highlight files, not two (the four-file declaration
      count is unaffected and was re-derived independently); the `continue` past
      a skipped channel is `scheduler.rs:355-356`, not `:355` (both the STATUS
      table and the plan's as-executed note); the routing guard's own doc still
      said "the settled set is pinned literally (RP3.1 and RP3.2 today)" while its
      literal carries all four pairs, and the staged-entries tripwire enumerated
      RP1.4's, RP3.2's four and RP3.4's two while omitting RP3.1's two — it now
      lists all **eight** staged ids by name, matching plan §RP4.1 precondition 2.
    - **Flagged for the user, not ruled in-lane (the one process finding).** The
      sub-step's kill criterion included "a `makeposseq`/`linespacing` deck DOES
      gate r4133", and `line_spacing_asym.dss` *is* `engines: "both"`; part A
      classified that as not-the-kill-criterion itself rather than escalating.
      The substance was re-verified twice (the `kind: "skip"` entry, r4133's own
      agreeing `LineGeometry.pas:1235-1239`, and the census carrying no
      `line.normamps`/`line.emergamps` row at all) and by the new skip-aware
      predicate, so nothing rests on the ruling; it is recorded here and reported
      to the user as a deviation, since a kill-criterion reading is the user's to
      make.

- **RP3.5** (2026-08-28) — `line.units`: **a port fix in both lanes, and the
  first RP3 sub-step whose fix moves the port's own render.** Outcome `FIX` for
  three independent defects in one routine (`TLineObj.MergeWith`), plus one
  upstream report for a fourth that is r4133's alone. It is also the first RP3
  sub-step to land a **live** ledger entry: the fix reds the `capi_v0145`
  `all_properties` compare on `modes:reduce/midi_reduce.dss`, whose channel is
  not masked, so the entry, its cause and the `population.lock.json` rewrite ship
  in this commit rather than staging to RP4.1.
  - **The probe ran first, on three engines** (r4133 `OpenDSSDirect.dll`
    `Version 11.0.0.1 (64-bit build) - Charlottesville`; pinned dss-python 0.15.7
    / dss_capi 0.14.5; the port, built out-of-tree against `dss-core` by path):
    the two vendored `modes:reduce` decks under `CorpusGuard`, plus six
    purpose-built micro-decks for the arms and the CIM surface no corpus deck
    reaches. No repo byte was written by the probe. None of the plan's four
    decision-table kill criteria fired.
  - **(A) The matrix-series branch lost the length units.** r4133 saves them
    (`Version8/Source/PDElements/Line.pas:1627`) and re-applies them with a
    **separate** `Length=%-g  Units=%s` Edit at `:1794-1796` that runs *after*
    the `Rmatrix/Xmatrix` (`:1778-1779`) and `Cmatrix` (`:1791-1792`) Edits,
    because the `12..14` side effect calls `ResetLengthUnits` (`:691-693`); its
    own `MakePosSequence` uses the same construction at `:1595-1596` under the
    comment "Repeat the Length Units to compensate for unexpected reset". dss_capi
    0.14.5 inverted the two — `src/PDElements/Line.pas:1806-1807` writes the
    fields, `:1815-1817` then runs `PropertySideEffects(rmatrix/xmatrix/cmatrix)`
    — and the port had copied that. Measured: r4133 renders `kft` on all three
    merged lines of `midi_reduce.dss` where the port and capi render `none`, and
    the getter is deck-dependent (`mi`/`cm` on two micro-decks whose surviving
    line carries those units, `none` on a switch), i.e. live state and not an
    echo. Fixed in both lanes (`exec/reduce.rs`: `red_set_units` after the side
    effects). **No solved-state quantity moves** — `ConvertLineUnits` returns 1.0
    whenever either side is `UNITS_NONE` (`Shared/LineUnits.pas:110-115`), so
    `FUnitsConvert` stays 1.0 and YPrim, Y, node voltages, currents, powers,
    losses, the node count (88) and the iteration count (5) are identical to
    r4133 before and after. Two *length-derived* quantities do move, both as
    corrections and neither compared anywhere today — `miles_this_line` and the
    meter zone's `line_length_km`; they are pinned by the audit settlement
    below.
  - **(B) `reset_length_units` also cleared `user_length_units`, which neither
    oracle does.** r4133 `:2330` and dss_capi 0.14.5 `:2084` carry the identical
    statement pair under the identical comment, "but do not erase
    FUserLengthUnits, in case of CIM export" — so this was a port-authored
    divergence from **both** oracles and there was no authority question to
    weigh. The plan called it "not observable in the census", which is true and
    not the same as unobservable: an exhaustive grep of both trees finds one
    consumer, `ExportCIMXML` (r4133 `:3707`, `:3735`, `:3877`), and it
    discriminates — on a deck typing `units=kft` before its matrices,
    `Export CIM100` writes `<cim:Conductor.length>609.6</…>` (= `2 × 304.8`) on
    r4133 **and** on capi 0.14.5 against `2` on the port. The line is deleted in
    both lanes. **No CIM golden byte moves**: in every golden CIM deck
    (`cim_lines`, `cim_load`, `cim_shunt`, `cim_xfmr`, `cim_der`,
    `IEEE13Nodeckt`, `IEEE123Master`) `units=` is the last impedance-relevant
    token of every `Line` declaration or absent altogether, so
    `reset_length_units` never runs after a `units=` write — swept, not assumed.
  - **(C) The sym-components branch's two switch arms, a gap the plan called
    already-correct.** The port's `Length=`/`Units=` re-apply sat inside
    `if let Some(v) = rxc`, while both oracles run it unconditionally (r4133
    `:1724-1726`; capi `:1764-1768`, outside `if UseRXC`). The two arms that
    produce no impedance values therefore kept a pre-merge `Len`: measured
    `0.001` (self-is-switch) and `1.1` (partner-is-switch) against `1` on both
    oracles. The partner arm carried a **second**, unrelated defect: the port
    emitted the text `Switch=1`, a transliteration of capi's *typed*
    `SetInteger(ord(TProp.Switch), 1, [])` (`:1736`), where r4133 emits the text
    `' switch=yes'` (`:1709`). `InterpretYesNo` rejects `1` on both engines
    (probed: `edit line.a Switch=1` leaves `switch='False'`, `r1='0.301'` on the
    r4133 DLL), so the arm was a silent no-op and the merged branch kept the
    partner's real impedance where both oracles give it dummy z (`r1 = 1`) —
    live state, not a render. Both halves fixed in both lanes. No vendored corpus
    deck reaches either arm (0 census cells), so no gated case moves; found and
    fixed inside the sub-step per CLAUDE.md's "port gaps immediately". Making the
    partner arm reachable exposed a **third** defect on it, found by the audit
    settlement below and fixed there: the sym branch's deferred
    `RecalcElementData`.
  - **(D) A new r4133 defect in the same routine — reported, never reproduced.**
    `Line.pas:1715` reads `S := ' R0=' + …` where every neighbouring statement
    appends (`S := S + …`), so the parallel symmetrical-components edit string
    loses its `R1=`/`X1=` half. On `modes:reduce/reduce_mergeparallel.dss` r4133
    renders the *un-merged* `r1 = 0.301` / `x1 = 0.667` while `r0`, `x0`, `c1`,
    `c0`, `length` and `units` match the port and capi exactly — the precise
    asymmetry an assignment-instead-of-append predicts — and the four measured
    relative gaps (0.5368421052631595, 0.5368421052631575, 1.5030841450107981,
    1.2866220615751998) reproduce the frozen census cells to the last digit, so
    the census row is not stale. capi 0.14.5 does not carry it (`:1740-1761`
    fills `RXC[1..6]` and sets all six) and neither does the port; the only
    exposing deck is `engines: "capi_v0145"`, so 0 in-scope cells and no
    exclusion is owed. Report:
    `investigations/to_opendss/45-line-mergewith-parallel-drops-r1-x1.md`
    (local). Next free report number is now **46**.
  - **The plan text was wrong about the population and silent about the cost;
    both corrected in place (§RP3.5).** `reduce_mergeparallel` contributes
    **zero** `line.units` cells — all 3 are on `midi_reduce.dss`
    (`Line.l2a~l2b`, `Line.l3a~l3b`, `Line.bb14_15~l9a`), and `Line.b1||b2.units`
    renders `km` on all three engines because that deck's lines are 3-phase
    symmetrical-components and take the sym branch. And the plan's two stated
    outcomes ("a port fix with a pin" / "a cited exclusion") did not anticipate
    the capi channel: `MODES.compare_all_properties` is `true` and
    `force_properties` ORs it in for every `gates_capi()` family case, and
    `(Line, Units)` is in no `SKIP_PROPS` / `PROPS_015X` / `LANE_SKIP_PROPS` row,
    so the fix reds 3 live cells.
  - **Ledger + lock (this commit, not staged).** New `capi_v0145` `divergence`
    entry `reduce-merge-units-restored-midi-capi-props` on
    `modes:reduce/midi_reduce.dss` — three exact-pair `property` scopes
    (`line.l2a~l2b.units`, `line.l3a~l3b.units`, `line.bb14_15~l9a.units`, each
    `rust: "kft"` / `oracle: "none"`) — plus the new cause
    `line-merge-length-units-reset`. Measured 3 hits on the case's one step.
    §1.1(e)'s staging rule is written for **r4133** `property` entries, which are
    staged only because that channel's props compare is masked until RP4.1; the
    capi compare is live, so staging this one would leave the gate red.
    `population.lock.json` moves exactly one line — `modes` `reduce/midi_reduce.dss`
    gains `ledger=capi_v0145:reduce-merge-units-restored-midi-capi-props@01a907f79c55bac9`.
  - **Pins (4 new `#[test]`s here, 3 more in the audit settlement below; all
    oracle-free and green in both lanes).**
    `exec::tests::reduce::merged_matrix_line_keeps_the_surviving_lines_length_units`
    (two micro-decks whose surviving lines carry *different* saved units, so the
    assertion cannot pass against a hardwired getter, plus an un-merged control
    and a post-merge `rmatrix=` reset);
    `exec::tests::reduce::parallel_merge_with_a_switch_restores_length_and_dummy_z`
    (three decks: partner-is-switch, self-is-switch, non-switch control);
    `elements::pd::line::tests::reset_length_units_keeps_the_users_units` (all
    three `reset_length_units` callers, asserting the pair — `length_units`
    cleared **and** `user_length_units` kept);
    `golden_cim::cim_conductor_length_uses_the_users_length_units` (the
    observable half of (B): `units=` first → 609.6, `units=` last → 914.4, no
    `units=` at all → 5, so the pin is a reading and not a constant factor).
    They are **not** in `props_r4133_pins.rs`: that file's guard
    (`props_r4133_replay::every_echo_row_pin_is_a_test_that_exists`) admits only
    `#[test]`s cited by a `PROPS_ECHO_R4133` row or by `LEDGER_ENTRY_PINS`, and
    RP3.5 owns neither (its outcome is `FIX`, and its ledger entry is a *live*
    capi one, not a drafted r4133 one whose window that table exists to cover).
    The house precedent for a landed capi property entry is an in-engine pin
    cited from the fix site — `capacitor::tests::make_pos_sequence_cmatrix_
    applies_the_positive_sequence_cuf` for `makeposseq-cuf-applied-capi`,
    `exec::tests::compat_quirks::gic_transformer_pct_r2_drives_winding_two` for
    `gic-pct-r2-honoured-*` — and that is the shape used here.
  - **Not moved, deliberately.** No `PROPS_ECHO_R4133` row (r4133's index-20
    getter is live at `:1404`; an echo row would be a false statement about the
    mechanism, the same reading RP3.1/RP3.2/RP3.4 made). No r4133 ledger entry
    (0 in-scope cells, and the r4133 props compare is masked until RP4.1, so
    `assert_all_hit` would fail it as NEVER APPLIED). No `props_roundtrip`
    scenario for a merged line — that gate replays dss_capi 0.14.5 values and
    would pin the capi bug as the expectation. `DECLARED_RP35 = (8, 6, 5)` and
    `DECLARED_RP3 = (6, 3, 6)` unchanged: they are read off the **frozen**
    extracts through the shipped chain predicates, which never run the engine, and
    `line.units` still declares a row, so `RP22_ROUTING`'s liveness guard stays
    green. `lane_diff.ps1` was **not owed** — the change carries no
    `#[cfg(feature = …)]` and moves no solved-state quantity on any gated case —
    but was run anyway rather than argued: **VERDICT PASS, `max |Δ| = 0` exactly
    on all eight gated kinds** (conv/cur/errs/iter/loss/pow/v/y, 3 220 247
    records over 522 cases, 0 iteration counts drifted), so the two lanes stay
    bit-identical and the default lane keeps the parity lane's oracle standing.
  - **A convention this sub-step establishes.** RP3.5 is the first RP3 sub-step
    whose fix changes the port's own render, so the frozen
    `tests/corpus/props_r4133/` extracts now record a `rust` value (`'none'`) the
    engine no longer produces. They are a **data** lock recording the 2026-08-08
    measurement (`props_r4133_evidence_lock.rs` re-measures nothing), so they are
    **not** edited; the ledger entry's `source` says so, and later sub-steps that
    fix rather than exclude inherit the same rule.
  - **Audit settlement (2026-08-29, one commit over `9daff660`).** Nine claims
    across the two audit agents — eight headline findings, one of them raised by
    both, plus the two halves of the tests audit's Major. Both auditors
    re-derived the mechanism independently on live oracles and confirmed the
    classification: outcome stays `FIX` in both lanes, the census still
    decomposes to 3 cells / 0 in scope, the ledger entry and its cause are
    unchanged, no upstream bug is reproduced and no `TODO(compat)` was added.
    Seven claims are real and settled here, one is **refuted** by live
    re-measurement, and one is real but outside `MergeWith`, recorded below with
    its owner. Settling the refuted one surfaced **a further defect in
    `MergeWith` itself** — the deferred `RecalcElementData` — fixed in the same
    commit under "port gaps immediately". No test was deleted, `#[ignore]`d or
    loosened, no tolerance exists here to move (every pin compares rendered
    strings), and no golden, frozen extract, `ledger.json` or
    `population.lock.json` byte moves.
    - **`MergeWith` re-pointed only the partner's controls, and named them with
      the pre-rename string — REAL, fixed.** r4133 calls
      `UpdateControlElements` **twice** (`Version8/Source/PDElements/Line.pas:
      1682-1683`), once for the surviving line's own old name and once for the
      partner's, both with `NewName`, and only then assigns `Name := NewName`
      (`:1684`). The port had the partner half alone — and in every reduce
      strategy that is the *vacuous* half: `DoReduceDefault` and
      `DoReduceShortLines` refuse to merge a line out when it `HasControl` or
      `IsMonitored` (`Meters/ReduceAlgs.pas:179-180`, `:347-348`), so a control
      can only ever sit on the **survivor**. Measured live on a `shortlines` deck
      merging `s1` into `s2`: the r4133 DLL renders `? CapControl.cc.element` as
      `line.s1~s2` where the port rendered `Line.s2` — and the port then failed
      to solve that deck (`singular at column 9`) while r4133 converged, because
      the stale reference kept the eliminated bus alive. The port now renames
      first and re-points both old references with the merged name; matching by
      the stable `ElemId` makes the Pascal's name-comparison order irrelevant,
      and renaming first is what lets the `element=` re-edit resolve. Pinned by
      `exec::tests::reduce::merge_repoints_the_controls_of_the_surviving_line`,
      proven non-vacuous by two mutations (drop the self half → the stale name
      and the singular Y; re-point before the rename → `Line.s2`). A control
      class whose property 1 is not spelled `element` (Relay/Recloser/Fuse:
      `MonitoredObj`) takes an unknown-parameter diagnostic and keeps its old
      name on **both** engines — probed on the same deck with a `Relay`, so that
      half is faithful and stays. This also closes a Phase-8 deferral that was
      still open by the "verify the successor of a forward-handoff" rule:
      `docs/phase-records/phase-8.md` recorded "`UpdateControlElements` has no
      runtime deck coverage (synthesized control-on-merged-line deck = a WP8.8
      sweep candidate)" and no later WP built it. It exists now, and it found the
      routine wrong.
    - **`RecalcElementData` was deferred out of the sym branch, and the deferral
      is not equivalent — REAL, fixed.** `MergeWith` ends its
      symmetrical-components branch with `RecalcElementData` (r4133 `:1730`,
      dss_capi 0.14.5 `src/PDElements/Line.pas:1771`); the port deferred it to
      `CalcYPrim` on the `SymComponentsChanged` flag. But that call *clears* the
      flag, and the flag is exactly what `CalcYPrim` tests before running its
      "the user never specified C1/C0" fix-up (`Line.pas:1031-1038`:
      `C1 := C1 / ConvertLineUnits(UNITS_KFT, LengthUnits)`). Every arm whose
      edit string carries `C1=`/`C0=` sets `FCapSpecified` and is immune — which
      is why the gap stayed invisible — but the two switch arms carry no
      impedance at all, and RP3.5's own item (C) had just made the
      partner-is-switch arm reachable. Measured on the sub-step's own d5 deck:
      after the (C) fix the port rendered `c1 = 3.60892388451444`,
      `c0 = 3.28083989501312` (the dummy `1.1 nF`/`1.0 nF` divided by
      `ConvertLineUnits(kft, km) = 0.3048`) where the r4133 DLL **and** capi
      0.14.5 both render `1.1` and `1` — a divergence from both oracles, so no
      authority question. `exec/reduce.rs` now recalcs in place; pinned by
      `exec::tests::reduce::parallel_merge_with_a_switch_recalcs_before_the_cap_fixup`
      (the self-is-switch arm restores `Units=none`, where the factor is 1.0, and
      is the discriminator), red on mutation.
    - **`reduce_mergeparallel` renders `linecode = ''` on r4133 — REFUTED.** The
      tests audit read the sub-step's own probe table as saying the r4133 DLL
      answers `''` on the partner-is-switch merge although its `switch=yes` arm
      (`:694-700`) does not clear `FLineCodeSpecified` — and drew from it that
      §RP3.6's premise was contradicted before RP3.6 starts. Re-measured on the
      same deck with the same driver: **r4133 renders `lc`**, exactly as the
      source says (`FLineCodeSpecified` is written at `:413` and cleared only by the
      impedance arms `:685`/`:691` and by the geometry/spacing/wire fetchers —
      never by arm 15). §RP3.6's premise stands and is now measured, not read.
      What the re-probe *did* find is the mirror image and belongs to RP3.6: the
      port renders `''` there, because its `SWITCH` side effect calls
      `kill_line_code_specified`. RP3.5's own (C) fix made that arm reachable, so
      the divergence is live on a path no corpus deck walks (0 cells) until
      RP3.6 lands — recorded here so RP3.6 inherits a measurement instead of a
      premise.
    - **A CIM divergence the probe measured and the record dropped — REAL, out of
      RP3.5's routine, recorded with an owner.** On a deck whose line names a
      LineCode and then overrides `r1=` *after* `units=`, r4133 back-fills the
      LineCode's units from `FUserLengthUnits` and writes
      `PerLengthSequenceImpedance.r = 0.301/304.8 = 0.00098753281`, because it
      matches by the `CondCode` **string**, which survives the flag being cleared
      (`Common/ExportCIMXML.pas:3877`, `if pLine.CondCode = pLnCd.LocalName`).
      capi 0.14.5 matches by the live object (`LineCodeObj <> NIL`) and writes
      `0.301`; the port follows capi (`cim/export.rs::find_line_units_for_linecode`,
      `line.line_code_ref.is_some()`). Item (B) is what *created the
      precondition* for the back-fill (before RP3.5 the port had no surviving
      `user_length_units` at all), which is why it was measured here — but the
      fix itself is not in `MergeWith`: it needs the port's
      `kill_line_code_specified` to stop clearing `line_code_name` while the
      render still answers `''`, i.e. the `FLineCodeSpecified`-vs-`CondCode`
      split that **§RP3.6 must build anyway**. Owner: **RP3.6**, noted in the
      plan's §RP3.6 text. No golden CIM deck exercises it (all of them name a
      LineCode or an impedance, never both).
    - **r4133 truncates the merged length through `%-g` — REAL as a latent
      r4133-channel divergence, recorded, not reproduced.** r4133 sets the merged
      `Len` only by rendering `TotalLen` into `Format(' Length=%-g  Units=%s')`
      and parsing it back (`:1725`, `:1795`), so its length carries 7 significant
      digits: measured `3.378788` against the port's and capi 0.14.5's
      `3.37878787878788` (rel 2.6e-7) on the sub-step's own micro-deck. The port
      keeps the exact `f64` sum, as capi does by assigning the field (`:1806`);
      reproducing the truncation would move the *capi* channel, which is the one
      that gates every reduce deck. Every vendored `reduce` deck sums to an
      exactly representable length (`4`, `1.4`, `1`, …), so no gated case sees it
      today — but `reduce_breakloop`, `reduce_dangling` and `reduce_laterals` do
      gate r4133, so RP4.1 gets this in writing rather than re-investigating it.
      Written into the pin's doc-comment beside the sentence that used to claim
      `length` "already agreed digit for digit" — true of `midi_reduce`, false of
      the micro-decks the sentence sat next to.
    - **The routing table still described the defect in the present tense — REAL,
      fixed** (both auditors, independently). `props_r4133_replay.rs`'s
      `RP22_ROUTING` comment for `line.units` still read "the port's
      matrix-series branch does the two in the opposite order" and "the port's
      `reset_length_units` clears `user_length_units`", with line references that
      the fix had moved. It is the one place in the tree where the mechanism sits
      beside its routing, and RP3.6's row sits directly under it. Rewritten as a
      settled `FIX` verdict naming both Rust sites, both lanes, the ledger entry,
      the pins and the derivation test; the citation column now reads
      `RP3.5 FIXED (2026-08-28) — …`. `Owner::Rp35`'s doc gained the sentence
      `RP3_ROUTING` already carries: a settled sub-step does **not** leave the
      accounting bucket, because no `Link` reads the ledger.
    - **The census decomposition was prose — REAL, derived now.**
      `the_rp35_census_decomposition_is_read_off_the_corpus` joins the RP3.1–RP3.4
      family: it sweeps the corpus **file universe** for a bare `Reduce` (with
      `files.len() > 1000` as the vendoring tripwire and the one non-script
      carve-out — `SyntaxFiles/opendss.stx`, a keyword list — named and asserted
      unique), reads each deck's `Set ReduceOption=` and whether it declares a
      line that is not 3-phase symmetrical-components, and combines the two: a
      cell needs a strategy that calls `MergeWith` at all
      (`ReduceAlgs.pas:54`/`:214`/`:258`/`:319`/`:361`, never the branch-disabling
      `:61`/`:87`/`:101`/`:372`/`:453`) **and** a line that can take the matrix
      branch (the negation of `Line.pas:1695`). Exactly one deck satisfies both —
      `midi_reduce` — and both halves are load-bearing on today's corpus:
      `reduce_laterals` declares 1-phase laterals but only removes branches, and
      `reduce_mergeparallel` merges but only 3-phase sym lines, which is RP3.5's
      counter-claim to the plan text asserted rather than told. Scope is
      skip-aware (`engines=` **and** no ledger `skip`, `r4133_skipped_cases` +
      the `SKIP_WITNESS` assertion — the RP3.4 house rule), and the products
      reconcile against `bins.tsv` and `examples_full.txt`: 3 cells, 0 in scope.
      Three reduce decks *do* gate r4133, asserted so the zero is a statement
      about this pair and not about the channel.
    - **No pin read the corpus deck the sub-step exists for — REAL, added.**
      `exec::tests::reduce::the_corpus_reduce_decks_merged_lines_render_kft`
      compiles the vendored `modes/reduce/midi_reduce.dss` and asserts the three
      merged lines render `kft` (and `length = 4`), with the deck's own
      discriminators: the un-merged `Line.bb1_2` also reads `kft` and the
      `switch=yes` `Line.tie` reads `none`, all four measured on the r4133 DLL.
      The ledger entry pins the same three cells from the other side, but that
      half needs the pinned dss-python installed; this one needs only the
      vendored deck.
    - **"Nothing else moves" understated the blast radius — REAL, corrected and
      pinned.** The restored units also move two length-derived quantities that
      no property renders and no gated case compares: `miles_this_line` (the
      `4,20:` side effect, `Line.pas:667-670`, feeding the reliability registers)
      and `line_length_km` (feeding the EnergyMeter zone's line length). Both
      were wrong before — miles kept the *pre-merge* value the matrix branch
      never recomputed, and the zone counted a length in miles as kilometres — so
      both are corrections; `exec::tests::reduce::merged_matrix_line_converts_
      its_length_with_the_restored_units` asserts them instead of leaving the
      change silent. The solved-state claim is unchanged and re-verified:
      `units_convert` is 1.0 either way, so Y, V, I, S, losses, node count and
      iteration count do not move.
    - **Test count 4 208 → 4 213 per lane (8 416 → 8 426), +5.** Four
      `exec::tests::reduce` pins
      (`merge_repoints_the_controls_of_the_surviving_line`,
      `parallel_merge_with_a_switch_recalcs_before_the_cap_fixup`,
      `the_corpus_reduce_decks_merged_lines_render_kft`,
      `merged_matrix_line_converts_its_length_with_the_restored_units`) and one
      replay derivation
      (`the_rp35_census_decomposition_is_read_off_the_corpus`). RP3.5 itself had
      taken 4 204 → 4 208 with its four pins and recorded no count; the ledger is
      picked back up here.
    - **Gate.** All five commands green in both lanes, **4 213 tests per lane**
      (0 failed, 0 filtered; the four `ignored` are the pre-existing doctest
      markers), corpus gate 131/131 with the entry still at 3 hits. `lane_diff`
      was re-run because this commit *does* move solved state on the
      partner-is-switch arm (no gated case reaches it): **VERDICT PASS,
      `max |Δ| = 0` exactly** on all eight kinds (conv/cur/errs/iter/loss/pow/v/y,
      3 220 247 records over 522 cases, 0 iteration counts drifted), so the two
      lanes stay bit-identical.

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

- **54 `kind=large*` `engines: both` cases have no property compare on EITHER
  channel — OPEN, by design, owed a decision at RP5.2 (R4133_PROPS RP4.1 audit
  settlement, 2026-09-03).** `scheduler::force_properties` keeps the plan's cost
  guard (`!kind.starts_with("large")`), so of the 367 `both` cases only **313**
  compare their property table; the same guard also leaves 14 r4133-only and 11
  capi-only `large` decks out, but those two never had one. The forced population
  is pinned (`FORCED_PROPS_POPULATION = (440, 313, 83, 44)`, asserted by
  `the_property_forcing_rule_is_every_live_non_large_case`), so this is measured,
  not drifting. What is owed is a decision — accept the gap permanently in the
  RP5.2 closing record, or price a `large`-deck property sweep (the `?`-sweep on
  the biggest feeders is the whole reason for the guard).

- **`DECLARED_RP35`'s four remaining declared pairs owe a per-pair disposition —
  OPEN (R4133_PROPS RP4.1, 2026-09-03).** RP4.1 retired only the two pairs its
  unmask measured (`swtcontrol.normal`/`.state` → `RP37_SUPERSEDED`); `line.units`
  (RP3.5), `line.linecode` (RP3.6), `relay.normal` and `relay.state` keep their
  declared rows (`DECLARED_RP35 = (5, 4, 2)`). The HEAD census shows no divergent
  cell for any of the four either, but a superseded row owes a **per-pair live
  disposition, a cited r4133 getter arm and a pin**, and nobody has produced that
  trio for them. Not RP4.1 work (the same item is recorded in the §RP4.1 record);
  it belongs to whoever closes the WP-RP3 accounting, at the latest RP5.2.

- **r4133 `New espvlcontrol.*` access violation — upstream-report candidate, OPEN
  (GOLDEN_REBASE G1.2, 2026-08-29).** The official EPRI r4133 DLL cannot
  instantiate class `ESPVLControl` at all (`#303`, read of `0x0`; offsets `15440`
  / `8F3F2D` — see the G1.2 record). Measured, isolated to the constructor path,
  and ledgered as `r4133-espvlcontrol-uninstantiable` so the corpus deck gates on
  `capi_v0145`. No port action: the port and the pinned 0.14.5 oracle both build
  and sample the class. What is owed is an English write-up in
  `investigations/to_opendss/` (local-only folder) — out of G1.2's scope.
- **`ESPVLControl.Forecast` — r4133 property 12 absent from the port, OPEN
  (GOLDEN_REBASE G1.2 audit settlement, 2026-08-29).** r4133 declares **12**
  class properties where the pinned dss_capi 0.14.5 declares 11:
  `.inputs/electricdss-code-r4133-trunk/Version8/Source/Controls/ESPVLControl.pas:133`
  (`NumPropsThisClass = 12`) and `:178` (`PropertyName^[12] := 'Forecast'` —
  "Loadshape object containing daily forecast"). The port mirrors capi
  (`elements/control/espvl_control/mod.rs`, `NUM_PROPS = 14` incl. the
  `TCktElementClass` tail + `Like`), so the property is **not implemented**. It is
  also **invisible to the R4133_PROPS census machinery**: the r4133 DLL cannot
  instantiate ESPVLControl at all (the follow-up above), so no census case will
  ever surface it. Whoever picks up the r4133 property line must add it by hand
  from the source; the corpus deck deliberately does not use it (it would not
  parse on 0.14.5).
- **`kind=skip` ledger entries self-hit, so an r4133 blackout can never go stale
  on its own — OPEN infrastructure limitation (surfaced by the GOLDEN_REBASE G1.2
  audit, 2026-08-29; pre-existing).** `corpus_gate/ledger.rs:272-283`
  (`channel_is_skipped`) sets `applied`/`exceeded_floor` and bumps `hits`
  unconditionally whenever the case dispatches, so the fail-on-stale discipline is
  satisfied trivially for `kind=skip` — an entry keeps reporting "1 hit" whether
  or not the upstream defect still exists. This affects all five skip entries
  today (`r4133-espvlcontrol-uninstantiable` + the four `r4133-*-303`). No
  automatic fix is possible without actually running the crashing case, so the
  procedure is manual; it is spelled out in `r4133-espvlcontrol-uninstantiable`'s
  `source` (the four older entries still lack it — adding it there is a separate,
  digest-moving edit): whenever the r4133 binary is re-vendored
  (TESTING.md §"Re-vendor the r4133 binary"),
  re-probe the skip-bearing cases with `DSS_GATE_SEED_LEDGER=1
  DSS_GATE_SEED_ONLY=<case>` and delete any entry whose cause upstream has fixed,
  so the r4133 channel re-lights instead of staying dark forever.
- **`CorpusGuard` can leak deck-written artifacts under concurrency —
  OPEN, out-of-scope observation (seen during GOLDEN_REBASE G1.2, 2026-08-29).**
  One `cargo test --workspace` run left five untracked files in
  `tests/corpus/electricdss-tst/Test/AutoTrans/` (`Auto3bus_*.txt`, written by
  the vendored `Auto3bus.dss:57 export currents file=…`), contradicting
  TESTING.md's "keep `tests/corpus` pristine afterwards". **Intermittent**: a
  filtered two-case run (`DSS_GATE_ONLY=Auto3bus`), the AD test alone, and a
  second full unfiltered `corpus_gate_all_cases_match_engines` run all left the
  tree clean. Consistent with an overlapping-guard snapshot race (cf. the unit
  test `corpus_guard_overlapping_guards_still_sweep`); not attributed further —
  it belongs to the gate infrastructure, not to a G1 sub-step. No gate went red.
  **Second sighting, same day, at the G1.2 settlement's own gate run:** seven
  files in the same directory (`auto3bus_{hl,ht}_{current,losses}.txt`,
  `auto3bus_lt_current.txt`, `autohlt_hl_current.txt`,
  `autohlt_noload_power.txt`), a *different* set from the first five and spelled
  **lowercase** where the first sighting's were `Auto3bus_*` — the
  case-insensitive collision `corpus_gate/runner.rs:42-55` and the G2.2d record
  already describe, now observed on the leak itself. Deleted by name (never a
  recursive delete); both gate lanes were green with them present, so the leak
  still costs nothing but hygiene. **Third and fourth sightings, 2026-09-03, at
  R4133_PROPS §RP3.12's own gate and settlement runs:** seven and then six
  untracked files in the same directory (`AutoHLT_*` / `auto3bus_*`, the
  spelling and the exact set varying again), both lanes green with them present,
  both removed by exact name; no tracked corpus or golden file moved. **Fifth and
  sixth sightings, 2026-09-03, at R4133_PROPS §RP4.1's gate and settlement runs:**
  eight and then five untracked files in the same directory, both lanes green
  with them present, both removed by exact name; `lane_diff.ps1`'s own artifact
  sweep removed the same class in its run without touching the two intended
  working-tree changes. Six sightings make it a pattern, not a fluke:
  whoever picks it up should start with `DSS_GATE_JOBS=1` per the G2.2d note.

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
