# R4133_PROPS - the STATUS §1 frontier narrative

> Moved verbatim from `STATUS.md` on 2026-09-03 (STATUS.md archiving round 2);
> order preserved, nothing rewritten. It holds STATUS §1's chronological frontier
> prose (era, in flight, WP-RP0 through RP3.5), the landed-recap and parked
> paragraphs, and the 2026-08-05 archiving note that closed round 1.

## The §1 frontier narrative (RP0 → RP3.5)

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
and has **§RP3.13** (2026-09-03, the two NCIM port bugs RP3.11's own P0
findings opened — the `ncim.rs:683` panic and the NCIM PV→PQ KCL gap, both
verdict `PORT_BUG`, fixed lane-unconditionally with zero `ledger.json` entries
and zero golden bytes; audit settled the same day — 12 findings, 6 fixed,
6 recorded, 0 refuted, one of them a fourth r4133 defect proven and not
reproduced; record below),
so what is left is §RP3.10 alone, deliberately sequenced *after*
RP4.1 (it blocks §RP5.2, plan §0).
Its four bin-7 root-cause sub-steps are ALL COMPLETE — RP3.1
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
below), which also corrected the premise this sentence carries: r4133 prints
`model=3` not because `Save` means "the store" but because
`TGeneratorObj.GetPropertyValue` has no arm 6 — the virtual getter that **49**
r4133 units answer live.
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

## Frontier recap, parked work and the 2026-08-05 archiving note

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
re-compilability guards ported — four in the sub-step (P1–P4) and three more in
its audit settlement the same day (`XfmrCode` and `DynEqPCE`, which completed the
declared union of both upstreams' `SaveWrite` overrides, plus the
sizing-property hoist for the five curve classes neither upstream guards) — and
**eleven** pins, eight then three, over **0** golden bytes moved by the
settlement and 1 content line + 1 lock digest by the sub-step; §RP3.11 record
above.
**RP3.13 landed 2026-09-03** — opened by RP3.11's own
round-trip measurement, two NCIM port bugs fixed: the `delta_q_nom` sizing panic
(`ncim.rs:683`, the vector sized to 1 where the PQ→PV arm writes per phase) and
the stale PV→PQ reported Q (the machine kept the PF-derived `Q` while the solve
injected the clamped one, so `Export Powers`/`Currents` violated KCL). Both
`PORT_BUG`, fixed lane-unconditionally in one commit with zero ledger entries and
zero golden bytes, plus two more defects of the same family found and fixed on
the way (the flat-start clamp, the swing-`VSource` NCIM arm). Its audit
settlement the same day (`217355da`, 12 findings — 6 fixed, 6 recorded, 0
refuted) added a ninth pin and stopped reproducing a **fourth** r4133 defect,
`CalcInjCurrAtBus`' PC-element sign, which is the only one of the four that
changes a reported number; §RP3.13 record above. **Next: RP3.10** (the reproduced `QMode=0` dispatch, user
go-ahead) and then **WP-RP5** (RP5.1 operational docs, RP5.2 the closing
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

