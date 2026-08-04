# GOLDEN_REBASE — WP-G0 / WP-G2 full session records

> Moved verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

### GOLDEN_REBASE G2.1h — `HEIGHT_UNIT_CHANGE_REREADS_THE_METRES_FIELD` torn down: a height-unit change re-reads the typed number in both lanes (branch `golden-g2`, 2026-08-05)

**Frontier.** WP-G2's eighth teardown and the **last of the eight zero-footprint
G2.1 rows**. `SPLIT_ALIAS_POPULATION` **24 → 23**, `TORN_DOWN_ROWS` carries its
eighth row, and the WP acceptance criterion still holds at this commit:
`git diff --stat -- tests/golden` over the range is **empty** and both lanes'
`oracle_parity_cfg_gate` is green. Next: G2.2a (the existing-exclusion rows —
`IRESIDUAL_FROM_TERMINAL_1` and `BUS_INT_DURATION_WALKS_ALL_BUSES`), which is
also where the pin-walk non-vacuity anchor moves onto a numeric survivor.

**The bug.** `TLineConstants.Set_FuserHeightUnit` moves the unit field and then
re-applies the offset under it — `FuserHeightUnit := Value; Set_FheightOffset(
FheightOffset);`, with upstream's own comment *"This updates the existing value
to fit the new user units"* (r4133
`Version8/Source/General/LineConstants.pas:689-696`). But the two sides of that
call disagree on units, and the same unit says so twice: `FheightOffset` is
declared *"The height is always saved in meters here"* (`:71`, `:97`), while
`Set_FheightOffset`'s argument is a **user-unit** number whose first statement is
`NewHeightOffset_m := Value * To_Meters(FuserHeightUnit)` (`:676-687`). A metres
value is therefore fed to a user-unit parameter and one number is converted
twice: 5 typed as feet is stored 1.524 m and, on a change to inches, re-read as
1.524 *inches* — 0.0387 m instead of 0.127 m, the entire outgoing-unit factor
0.3048 off. The offset lands in the conductor heights that Carson's equations
integrate, so a wrong offset is a wrong Z/Yc. **The correct number is written out
in the same class**: `Get_FheightOffset` (`:396-399`) is exactly
`FheightOffset * From_Meters(FuserHeightUnit)`, the typed number — the fix is
that getter called one statement earlier. Deep dive:
`investigations/issue-28-line-height-units-reread.md`.

**Cited against r4133 only, deliberately.** The `HeightOffset=`/`HeightUnit=`
surface **does not exist** in the pinned dss_capi 0.14.5 backend at all — its 186
`.pas` files contain no `FheightOffset` — so these line numbers must not be
resolved against `.inputs/dss_capi`, where they land on unrelated code. That is
what once made the row read as uncited; the deleted const, the surviving kernel
doc and the register row all now say so in place. The gated deck is on the
`r4133` channel for the same reason.

**What landed.** `compat::HEIGHT_UNIT_CHANGE_REREADS_THE_METRES_FIELD` and both
its `*_IMPL` twins are **deleted**; `LineConstants::set_user_height_unit` reads
`self.height_offset()` unconditionally, before the unit field moves. The default
lane already did this, so **only the parity lane moves** — and only on a path
nothing gated reaches: the sole caller is the Line → Carson push
`makeZFromGeometry`/`makeZFromSpacing`, whose fixed order is `SetEpsRMedium`,
`SetHeightOffset`, `SetUserHeightUnit`
(`line_geometry::matrix::set_line_constants_medium`). On the **first** push into
a freshly built engine the outgoing unit is the constructed `UNITS_M`, where
`From_Meters(m) = 1` makes both readings the same number; every later push
re-enters with the unit unchanged and returns early. The two part only on a
*second* unit change with a non-metre outgoing unit — see the audit settlement
below for the exact reachability. The compat module-doc row
for the *Single-site upstream quirks* section records this row as G2.1h, and the
`matrix.rs` call-order comment no longer names a const that is gone.

**The pin became unconditional.**
`support::line_constants::tests::height_unit_change_rereads_the_typed_number`
lost its `ORACLE_PARITY` branches and carries the
`EXPECTED-VALUE-PIN(HEIGHT_UNIT_CHANGE_REREADS_THE_METRES_FIELD)` marker. Its
first half is unchanged and stays the guard on the gated deck
`modes/upgrade/upgrade_linecs_heightoffset.dss` (`HeightOffset=5 HeightUnit=ft` →
1.524 m, bit-exact, plus the three conductor heights). Its second half is what
the teardown makes assertable: after the ft → in change the offset is the typed
5 inches (0.127 m), the compounded upstream reading (1.524 in) is asserted
**absent** by more than half its own magnitude — a factor 0.3048, some fifteen
orders above the test's 4-ULP round-trip bound (0.695 relative against 8.9e-16),
so the assertion discriminates the fix from a revert rather than measuring
rounding — and the invariant the row exists
for is stated directly: the typed number, and every conductor height derived from
it, survives every unit change. The register's `Evidence::Site` needle is the
fixed statement with its line break and the method body's own indentation
(`"\n        let typed = self.height_offset();"`): the split form had this read
as an `else` arm, four spaces deeper and without the binding, so a restored lane
branch stops matching.

**Zero-footprint.** No golden byte, no `ledger.json` entry and no
`population.lock.json` field moves: everything gated pushes at metres or with the
unit unchanged, so both readings coincide there, and
`upgrade_linecs_heightoffset.dss` is byte-identical in both lanes. Doc surface:
the alias was never cited on the walked surface (measured again — outside
`crates/` it appears only in STATUS, the plan and the gitignored
`investigations/`), so no citation was struck and the non-vacuity floor is
untouched; CLAUDE.md does not name this row (it is not one of the six named
bugs).

**Proof.** Every Pascal citation here was read out of the vendored r4133 source,
and the "absent from 0.14.5" claim re-measured by grepping its 186 `.pas` for
`FheightOffset` (no match). The pin was mutation-checked: restoring upstream's
reading (`let typed = self.height_offset;`) reds
`height_unit_change_rereads_the_typed_number` in **both** lanes; the mutation was
reverted and the tree re-verified green. Both lanes clean on the five gate
commands (`cargo fmt --all --check`; `cargo clippy --workspace --all-targets --
-D warnings` and the same with `--features dss-core/oracle-parity`; `cargo test
--workspace` and the same with the feature — the unconditional 520-case corpus
gate included, exit 0 in both lanes). `lane_diff.ps1`, re-run because a lane
alias was deleted: 520 cases, 3 219 862 records, `max |Δ| = 0` **exactly** on
every gated kind (`conv`, `cur`, `errs`, `iter`, `loss`, `pow`, `v`, `y`), 0
iteration counts drifted, `VERDICT: PASS` — the only entries being the documented
Newton pow/loss divergences on the two `newton` decks (the still-live
`POWERS_REUSE_STALE_NEWTON_ITERMINAL` row G2.3 removes). Tests 0 added, 0
renamed, 0 removed, 0 new
`#[ignore]`. `git diff --stat -- tests/golden` is empty; `git status --short
tests/corpus` clean after both runs (the eight `AutoTrans/*.txt` run artifacts
were removed per file, never recursively).

**Audit settlement (both findings upheld, docs only — no engine change).** Two
independent auditors flagged the same over-stated clause — *"the offset is stored
while the engine still carries its constructed `UNITS_M`"*, which the Stage F.3u
record below states and this teardown's four texts repeated as *"every path that
reaches here today"*. That is a structural guarantee only for the **first** push. The Carson engine is
built at geometry-edit time
(`line_geometry::edit::realloc_conductors`/`change_line_constants_type`,
`LineConstants::new`), snapshot-cloned into the Line by
`elements::pd::line::code::fetch_geometry_code` (`geom.clone()`, replaced only by
a later `geometry=`), and **never rebuilt per Z build** — so every later push
from `line::solve::make_z_from_geometry` re-enters
`set_user_height_unit` with the *previous* build's unit. What actually keeps the
gated corpus on the coinciding branch is a second, different mechanism: the unit
is unchanged on those pushes, so the setter returns early at `if value ==
self.user_height_unit`. The two readings are reachable apart only after an
`Edit Line.<n> HeightUnit=` plus a rebuild — the edit alone does not invalidate
`FZFrequency` (`line::accessors::side_effects`, `HEIGHT_UNIT` arm), a frequency
change does. Measured with a throwaway probe over the real geometry API (two
`set_line_constants_medium` pushes on one persistent geometry, then removed):
push 1 `ft` → 1.524 m; push 2 `in` on the stale `ft` → **0.127 m** (the typed 5
inches, i.e. the fix) against upstream's compounded 0.0387 m; push 3, unit
unchanged → early return, bit-identical. The conclusion the four texts drew is
therefore right and stays — zero footprint, `lane_diff` `max |Δ| = 0`, zero
golden and corpus bytes — but the *reason* was wrong, so the clause is reworded
in all four places (`line_constants/mod.rs` kernel doc, `line_geometry/matrix.rs`
call-order comment, the `TORN_DOWN_ROWS` register comment, the pin doc in
`line_constants/tests.rs`) to name both mechanisms and the edit-plus-rebuild
sequence that separates them. **Follow-up, deliberately not done here:** the
diverging sequence has no deck-level cover — only the unit-level pin. A two-build
corpus case (`HeightOffset=5 HeightUnit=ft`, solve, `Edit Line.l1 HeightUnit=in`,
solve at a new frequency, `r4133` channel with a ledger entry for the upstream
gap) belongs to a WP allowed to move corpus bytes; WP-G2 forbids it (acceptance
criterion: zero bytes under `tests/golden`, and the same discipline applied to
`tests/corpus` across these sub-steps). Recorded for G3/G4. The settlement
changes **comments only** — every `+`/`-` line under `crates/` is a `///` or `//`
line — and was re-verified on the full five-command gate in both lanes plus
`lane_diff.ps1` (520 cases, 3 219 862 records, `max |Δ| = 0` exactly on `conv`,
`cur`, `errs`, `iter`, `loss`, `pow`, `v`, `y`, 0 iteration drift, `VERDICT:
PASS`, the two `newton` decks the only documented entries).

### GOLDEN_REBASE G2.1g — `CIM_WYE_GROUNDED_IS_HARDCODED_TRUE` torn down: the CIM `grounded` flag reads the neutral in both lanes (branch `golden-g2`, 2026-08-04)

**Frontier.** WP-G2's seventh teardown. `SPLIT_ALIAS_POPULATION` **25 → 24**,
`TORN_DOWN_ROWS` carries its seventh row, and the WP acceptance criterion still
holds at this commit: `git diff --stat -- tests/golden` over the range is
**empty** and both lanes' `oracle_parity_cfg_gate` is green. Next: G2.1h
(`HEIGHT_UNIT_CHANGE_REREADS_THE_METRES_FIELD`, `issue-28`), the last of the
eight zero-footprint rows.

**The measurement the plan asked for first, and its answer.** G2.1g's row
carries a caveat: if any committed `cim/` golden *observes* the wye `grounded`
flag at a value the fix moves, the row leaves G2.1 for G2.2c and gets a
`lane_expected_cim` rewrite rule instead of a plain flip. Measured, twice and
from both sides. (a) The goldens do carry the flag — 114 `EnergyConsumer.grounded
= true`, 12 `= false` (the delta arm), 8 `ShuntCompensator.grounded = true`, 1
`LinearShuntCompensator.grounded = false` (`issue-24`'s misprefixed delta node)
— so the question is not vacuous. (b) No golden deck puts a wye neutral anywhere
but ground, so none of those `true`s moves: with the fixed kernel selected in
**both** lanes the parity lane's own byte compare of all 15 CIM goldens is green,
and `lane_expected_cim` — whose two rules rewrite element *names* only, never the
`grounded` value — gained no third entry. G3.4's predicted-diff list therefore
does **not** gain this row.

**The bug.** The CIM writer answers "is this wye point earthed?" with a
hard-coded constant. `BooleanNode(FunPrf, 'ShuntCompensator.grounded', TRUE)`
(`.inputs/dss_capi/src/Common/ExportCIMXML.pas:3700`; r4133
`Version8/Source/Common/ExportCIMXML.pas:3183`) and `BooleanNode(FunPrf,
'EnergyConsumer.grounded', TRUE)` (`:4478`; r4133 `:3854`) are written
unconditionally, and each carries upstream's own `// TODO - check bus 2` — the
authors wrote the open question down and shipped the placeholder. So a wye bank
with `bus2=` on a live bus, and a wye load whose 4th conductor lands on a
neutral-earthing reactor, both export as *solidly grounded*. The calculation is
untouched (the flag lives only in the export), but the export exists to hand the
model to another tool, and solidly-earthed vs isolated/impedance-earthed are
qualitatively different machines for single-phase-fault, zero-sequence and
earth-fault-protection work. Both gating oracles carry the line, so the
2026-08-02 policy applies rather than exempts. Deep dive:
`investigations/issue-23-cim-grounded-always-true.md`.

**What landed.** `compat::CIM_WYE_GROUNDED_IS_HARDCODED_TRUE` and both its
`*_IMPL` twins are **deleted**; the two writers in `cim/export.rs` read the model
unconditionally — the capacitor through `snap.term2_nodes.iter().all(|&n| n ==
0)`, the load through `snap.neutral_node == 0`. The default lane already did
this, so **only the parity lane moves**. The fix is not an invented semantic: it
is the test the **same unit's** transformer writer already applies
(`XfmrTankPhasesAndGround` `:1531-1570` — `if (pXf.NodeRef[j2] = 0)` → "last
conductor is grounded solidly"), pointed at where each shunt class keeps its
neutral — a Capacitor is two-terminal (`Nterms = 2`, `Nconds = Nphases`) and its
wye point *is* terminal 2, literally the "bus 2" the TODO names; a Load is
one-terminal and `SetNcondsForConnection` (`Load.pas:479-492`) gives a wye
connection `Nconds = Nphases + 1`, so its neutral is that terminal's
`Nphases+1`-th conductor. The pre-solve case is unchanged and stays consistent
with the transformer sibling: before `SetNodeRef` has run the node refs are empty
and both readings answer `true`. Each site now cites both oracles' lines in
place, and the compat module-doc row for the *Single-site upstream quirks*
section records this row as G2.1g.

**The pin became unconditional.** `golden_cim::cim_wye_grounded_reads_the_neutral`
(renamed from `cim_wye_grounded_is_lane_split`) lost its `ORACLE_PARITY` branch
and carries the `EXPECTED-VALUE-PIN(CIM_WYE_GROUNDED_IS_HARDCODED_TRUE)` marker.
Its multi-deck shape is what makes it a pin of a *reading* rather than of a
flipped constant, and every half asserts one value in both lanes: the **probe**
deck (capacitor `bus2=nb.1.2.3` on a live bus, load `bus1=b1.1.2.3.4` with `b1.4`
held by `reactor.ng`) must export `false` for both classes; the **control** deck
— the same feeder with default neutrals — must still export `true` for both; and
the **mixed** deck added by the audit settlement below (capacitor
`bus2=nb.1.0.0`, single-phase load with a live neutral) separates `all` from
`any` and exercises the load reading's derived index off the 3-phase path. Its
`only_grounded` reader still asserts exactly one node of each kind, so a later
deck edit that adds a second shunt fails loudly instead of reading the wrong one
— which is why each deck is its own circuit. The register's `Evidence::Site`
needles are the two readings as rustfmt lays them out
(`"\n                snap.term2_nodes.iter().all(|&n| n == 0),"` and
`"\n                snap.neutral_node == 0,"`): in the split form each expression
was the right operand of `alias ||` — the capacitor's wrapped four spaces deeper,
the load's sharing the line with `crate::compat::…` — so a restored lane branch
stops matching either one.

**Zero-footprint.** No golden byte, no `ledger.json` entry and no
`population.lock.json` field moves — the flag is written only by the CIM export,
which no corpus case runs, and the goldens' measurement above settles the rest.
Doc surface: the alias was never cited on the walked surface (measured again —
outside `crates/` it appears only in STATUS, the plan and the gitignored
`investigations/`), so no citation was struck and the non-vacuity floor is
untouched; CLAUDE.md does not name this row (it is not one of the six named
bugs).

**Proof.** Every Pascal citation added here was read out of the vendored sources,
not recalled: `ExportCIMXML.pas:3697-3701` and `:4475-4479` with their TODOs, the
transformer sibling at `:1546-1554`, and on the r4133 side `:3180-3186` and
`:3851-3857`. The pin was mutation-checked: replacing both readings with `true`
(upstream's constant) reds `cim_wye_grounded_reads_the_neutral` in **both** lanes
at the probe assertion (`golden_cim.rs:476`); the mutation was reverted and the
tree re-verified green. Both lanes clean on the five gate commands (`cargo fmt
--all --check`; `cargo clippy --workspace --all-targets -- -D warnings` and the
same with `--features dss-core/oracle-parity`; `cargo test --workspace` and the
same with the feature — the unconditional 520-case corpus gate included).
`lane_diff.ps1`, re-run because a lane alias was deleted: max |Δ| = 0 on every
gated kind, `VERDICT: PASS`, the only entries being the documented Newton
pow/loss divergences on the two `newton` decks (the still-live
`POWERS_REUSE_STALE_NEWTON_ITERMINAL` row G2.3 removes). Tests 0 added, 1
renamed, 0 removed, 0 new `#[ignore]`. `git diff --stat -- tests/golden` is
empty; `git status --short tests/corpus` clean after both runs.

**Audit settlement (fix agent, same branch).** Three findings, all `minor`, none
touching the engine's behaviour, a golden byte, the ledger or a tolerance — the
only non-comment edits are inside two test binaries. All three settled by
change; none deferred, none refuted.

- *The register anchors only the capacitor half* — **real, fixed.**
  `Evidence::Site`/`Evidence::Exclusion` now carry a **slice list**
  (`&'static [&'static str]`, empty list refused) instead of a single needle, and
  this row lists both writers. The finding was right that the shortfall was
  structural rather than an oversight: the old tuple could name one site, so
  reverting `snap.neutral_node == 0` to `true` left
  `every_torn_down_row_keeps_its_pin_and_its_evidence` green while the pin — and
  only the pin — went red. The row→tree direction is what `GOLDEN_REBASE_PLAN.md`
  §G2.0(b) asks this register to mechanize, so half a teardown anchored is a real
  hole in it. Mutation-checked in the fixed shape: the same revert now names the
  missing load needle in the register's own failure text. The list also gives the
  later multi-site rows (G2.2c, G2.2d) a shape to land in; the six pre-existing
  rows became one-element lists with no other change.
- *The `all` reading is a widening of the single-conductor sibling, and answers
  `true` on the degenerate `nterms < 2`* — **real as a documentation gap, no
  behaviour change; narrated at the site.** The reading is deliberate and
  correct: `XfmrTankPhasesAndGround` tests one conductor because a wye *winding*
  has one neutral conductor, whereas a wye capacitor has none (`Nconds = Nphases`
  — `Capacitor.pas:340`, r4133 `:299`) and its terminal-2 conductors are the
  per-phase returns, so a bank is solidly earthed only when *every* return is at
  ground. The `nterms < 2` fallback was confirmed unreachable for the wye arm
  that reads it: `conn=wye` forces `Nterms := 2` (`Capacitor.pas:334-339`; r4133
  `:298`), ported at `elements/pd/capacitor/accessors.rs:210-211`. Both facts now
  sit in the site comment with those citations instead of only in the plan.
- *The pin cannot tell `all` from `any`* — **real, fixed.** Both existing decks
  are uniform (probe: every terminal-2 conductor live; control: every one at
  ground), so the two aggregations agreed on them and the mutation `all → any`
  survived green. Since this reading is the engine's **own** semantic — divergent
  from both gating oracles, with no oracle to fall back on — an under-determined
  pin is the whole exposure. A third **mixed** deck now carries a capacitor
  earthed on two phases and live on the third (`bus2=nb.1.0.0`, the
  impedance-earthed-on-one-phase shape) asserted `false`, plus a single-phase wye
  load with a live neutral asserted `false` — the only deck exercising
  `node_ref[nphases]` off the 3-phase path. Mutation-checked both ways: `all →
  any` now reds the mixed capacitor assertion (`golden_cim.rs:526`) and reverting
  the load reading to `true` still reds the probe; both were reverted and the
  tree re-verified green. No golden byte, no new test file, no deck outside the
  pin.

Gate re-run in full for these edits (five commands, both lanes).
`lane_diff.ps1` re-run as well although the engine change is comment-only:
`VERDICT: PASS`, max |Δ| = 0 on every gated kind, same two documented Newton
entries.

### GOLDEN_REBASE G2.1f — `STORAGE_MULTIFILE_USES_THE_PV_PREFIX` torn down: the Storage `/m` export gets its own prefix (branch `golden-g2`, 2026-08-03)

**Frontier (superseded by G2.1g above).** WP-G2's sixth teardown.
`SPLIT_ALIAS_POPULATION` **26 → 25**,
`TORN_DOWN_ROWS` carries its sixth row, and the WP acceptance criterion still
holds at this commit: `git diff --stat -- tests/golden` over the range is
**empty** and both lanes' `oracle_parity_cfg_gate` is green.

**The bug.** `Export Storage_Meters /m` writes one file per Storage element and
builds its name from a per-class prefix. `WriteMultipleStorageMeterFiles`
(`.inputs/dss_capi/src/Common/ExportResults.pas:2240`) was cloned line-for-line
from `WriteMultiplePVSystemMeterFiles` (`:2076`, prefix at `:2095`) and kept its
`'EXP_PV_'` literal, so a Storage fleet's registers land in the PVSystem
export's own files. The writer emits a header only when the file does not yet
exist and otherwise **appends** (`:2242`), so a PVSystem and a Storage sharing a
name — legal, names are unique per class — interleave their rows under whichever
class's header was written first, and the file no longer says which row is
whose. That it is a copy-paste slip and not a convention is settled inside the
same command: its single-file mode writes `EXP_STORAGEMeters.csv`
(`ExportOptions.pas:411`) next to `EXP_PVMeters.csv` (`:409`), and every other
class carries its own prefix (`EXP_MTR_` `:1792`, `EXP_GEN_` `:1948`). r4133 has
the identical line, live, at `Version8/Source/Common/ExportResults.pas:2280` (the
`Storage2` twin repeats it at `:2335`, but that whole procedure is commented out
— `(*` `:2314` … `*)` `:2368` — so it is dead code, not a second live site), so
both gating oracles have it and the 2026-08-02
policy applies rather than exempts. Deep dive:
`investigations/issue-20-storage-meters-pvsystem-prefix.md`.

**What landed.** `compat::STORAGE_MULTIFILE_USES_THE_PV_PREFIX` and both its
`*_IMPL` twins are **deleted**; `exec/report.rs::gather_register_rows`'
`RegKind::Storage` arm returns `"EXP_STORAGE_"` as a plain tuple element, with
both upstream lines, the cloned source procedure, the sibling prefixes and the
single-file names cited in place. The default lane already did this, so **only
the parity lane moves**; `report.rs` no longer imports `compat` at all. The
compat module-doc row for the *Single-site upstream quirks* section records this
row as G2.1f.

**The pin became unconditional.**
`golden_reports::export_storage_multifile_uses_the_storage_prefix` (renamed from
`export_storage_multifile_prefix_is_lane_split`) lost its `ORACLE_PARITY` branch
and carries the `EXPECTED-VALUE-PIN(STORAGE_MULTIFILE_USES_THE_PV_PREFIX)`
marker. In **both** lanes it now asserts the same three things it used to assert
per-lane: `EXP_STORAGE_ST1.csv` exists, `EXP_PV_ST1.csv` does **not** (so the
prefix cannot drift in either direction), and the produced rows still compare
byte-for-byte against the oracle-anchored single-file golden
`tests/golden/reports/export_storage_meters.txt` — which is what keeps the claim
"only the file *name* moved" load-bearing rather than asserted. The register's
`Evidence::Site` needle is the literal as rustfmt lays it out
(`"\n                    \"EXP_STORAGE_\","`, anchored on the tuple element's
twenty-space indentation **and** its trailing comma): in the split form the same
literal was the tail expression of an `else` block at the same depth with no
comma, so a restored lane branch stops matching.

**Zero-footprint, measured not assumed.** The lanes could only ever differ in a
*file name* produced by `Export Storage_Meters /m`, and nothing gated runs that
switch: the golden generator `tools/golden/gen_reports.py:338` captures the
single-file `EXP_STORAGEMeters.csv`, and no corpus deck exports Storage meters
at all — the only `/m` caller in the tree is this pin. So no golden byte, no
`ledger.json` entry and no `population.lock.json` field moves, and the parity
lane's full suite (the unconditional 520-case corpus gate against both oracle
channels included) is green, which is the measurement confirming the
classification rather than assuming it.

**A gap in the pin walk, found by the deletion.**
`oracle_parity_cfg_gate::branches_on_lane` accepted only the spelling
`ORACLE_PARITY`, while the walk matches per **file**. Deleting this row's pin
branch removed the last `ORACLE_PARITY` token from `tests/golden_reports.rs`,
and `every_lane_split_alias_is_pinned_by_an_expected_value_test` immediately
reddened for three **still-split** rows pinned in that file —
`IRESIDUAL_FROM_TERMINAL_1`, `BUS_INT_DURATION_WALKS_ALL_BUSES`,
`FAULT_DUMP_TAIL_REPRINTS_MINAMPS` — every one of which branches through the
harness alias `lane::PARITY` and had been credited only by a neighbouring test's
constant. The three pins are real; the predicate could not see their spelling.
Fixed by accepting the **qualified** `lane::PARITY` as a second spelling —
legitimate because `harness/lane.rs:78` defines `PARITY` as
`cfg!(feature = "oracle-parity")` and `lane.rs:795-796` asserts it equals
`dss_core::compat::ORACLE_PARITY`. `names_token` keeps the arms apart (`PARITY`
inside `ORACLE_PARITY` is preceded by an identifier character, so it does not
match), and the rejection this predicate exists for — deriving a pin's
expectation from the row's **own** alias — is untouched. The bare word is
deliberately **not** accepted (audit fix, below): unlike `reads_the_lane`, where
a loose match rejects more, here it accepts more, so a prose `PARITY` in a
comment would satisfy the rail.

**Doc surface.** The alias was never cited on the walked doc surface (measured
again here: outside `crates/` the only mentions are STATUS, the plan,
`docs/phase-records/phase-8.md` and the gitignored `investigations/`, none of
which `operational_docs` reads), so no citation was struck and the non-vacuity
floor is untouched. CLAUDE.md names this row nowhere — it is not one of the six
named bugs — so its policy §Status sentence is unchanged. The historical
DE_PASCALIZE F.3l record below keeps its text and gains a supersession marker on
its table row and its paragraph.

**Proof.** Every Pascal citation this commit adds was read out of the vendored
sources, not recalled: `ExportResults.pas:2240`/`:2242` and the PVSystem twin at
`:2076`/`:2095`, the `EXP_MTR_`/`EXP_GEN_` lines `:1792`/`:1948`, the
`ExportOptions.pas:409`/`:411` default names, and on the r4133 side
`WriteMultipleStorageMeterFiles` at `:2260` with the buggy line at `:2280` plus
`WriteMultipleStorage2MeterFiles` at `:2315` with its copy at `:2335` — the
latter inside the commented-out block `:2314`–`:2368`, so `:2280` is the only
live occurrence and the only one the citation leans on. The pin
was mutation-checked: restoring `"EXP_PV_"` in the tuple reds
`export_storage_multifile_uses_the_storage_prefix` in **both** lanes (measured,
same assertion, `golden_reports.rs:3883`); the mutation was reverted and the
tree re-verified green. Both lanes clean on the five commands
(`cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D
warnings` and the same with `--features dss-core/oracle-parity`; `cargo test
--workspace` and the same with the feature — exit 0 in each, the unconditional
520-case corpus gate 53/53 in both). `lane_diff.ps1`, re-run because a lane
alias was deleted: over 520 cases / 3 219 862 records every gated kind
(`conv`/`cur`/`errs`/`iter`/`loss`/`pow`/`v`/`y`) is **identical**, max |Δ| = 0,
zero iteration drift, `VERDICT: PASS`; the only entries are the documented
Newton pow/loss divergences on the two `newton` decks — the still-live
`POWERS_REUSE_STALE_NEWTON_ITERMINAL` row that G2.3 removes. Tests 0 added, 1
renamed, 0 removed, 0 new `#[ignore]`. `git diff --stat -- tests/golden` is
empty; `git status --short tests/corpus` is clean after both runs (the
`Test/AutoTrans/*` export leak deleted by exact name, never committed).

**Audit settlement (fix agent, same branch).** Five findings, all `minor`, none
touching the engine, a golden byte, the ledger or a tolerance — four distinct
defects, since two auditors reported the same stale assert message. Three fixed,
one deferred with its reason.

- *The `branches_on_lane` widening is fail-open* — **real, fixed.** The second
  arm now matches the qualified `lane::PARITY`, not the bare word: in
  `reads_the_lane` (`:1657`) a loose match makes its caller reject more
  (fail-safe), here it makes the caller accept more, so an English `PARITY` in a
  comment satisfied the rail. Measured sufficient before tightening: the only
  bare-word occurrences outside the arm's own definition site are two prose
  comments (`golden_reports.rs:1620`, `corpus_gate/scheduler.rs:358`), every real
  read is written `lane::PARITY` (`golden_reports.rs` ×15, `harness/mod.rs:1343`
  and `:2358`), and `harness/lane.rs` — which spells it bare because it declares
  it — is credited by the first arm through `ORACLE_PARITY` at `:796`. Both
  lanes' `oracle_parity_cfg_gate` stays green, so no row lost its pin.
- *The pin-walk failure message still advertises the rejected `compat::<alias>`
  form and omits `lane::PARITY`* — **real, fixed** (reported twice, by both
  auditors). Message text only: it now names `compat::ORACLE_PARITY` and the
  qualified `lane::PARITY`, and states explicitly that deriving the expectation
  from the row's own alias does not count (F-settle W4). The clause pre-dated
  G2.1f, which rewrote the predicate's doc comment and left the assert text.
- *The r4133 `:2335` "Storage2 copy" citation points inside a commented-out
  block* — **real, fixed.** Verified in the vendored trunk: `(*` at
  `ExportResults.pas:2314`, `*)` at `:2368`, with
  `Procedure WriteMultipleStorage2MeterFiles` (`:2315`) and its `'EXP_PV_'`
  (`:2335`) between them, so the twin is dead code. The load-bearing citation —
  the live `:2280` inside `WriteMultipleStorageMeterFiles` (`:2260`) — is
  unaffected, and the vendored capi source has no `Storage2` procedure at all
  (its only two `EXP_PV_` lines are `:2095` and `:2240`), so nothing about the
  pinned value moves. All five sites reworded (`exec/report.rs`,
  `golden_reports.rs`, the `TORN_DOWN_ROWS` rationale, STATUS ×2).
- *The walk still credits per **file**, so a pin can be credited by a lane branch
  in a neighbouring test* — **real as a mechanism, deliberately deferred**, as
  the auditor itself proposed; its worked example is **refuted**. The mechanism
  is real and predates G2.1f — it is the very gap this teardown exposed, and
  `every_lane_split_alias_is_pinned_by_an_expected_value_test` still evaluates
  `names_token(region, alias) && branches_on_lane(region, alias)` over a whole
  file's test region (`:1094`). The example offered for it does not hold:
  `FAULT_DUMP_TAIL_REPRINTS_MINAMPS` is named at `golden_reports.rs:5614` and
  read as `lane::PARITY` at `:5624` — the *same* helper `fault_dump_expected`,
  not a neighbouring test — so no live row in this tree is credited across a
  function boundary. Closing the mechanism needs per-row pin-fn names for the
  *live* split aliases — a second register beside `TORN_DOWN_ROWS`, which
  already carries them and already scopes its own check with `test_fn_body`
  (`:1671`). That is a new rail, not a teardown, so it is out of G2.1f's scope
  and recorded here as an open item for the next sub-step that touches this walk
  anyway — **G2.2a**, which must re-anchor the walk's non-vacuity const onto a
  numeric survivor. Tightening the spelling above does not widen it: same file
  granularity, one spelling narrower.

### GOLDEN_REBASE G2.1e — `STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL` torn down: the fleet guard asks the state test it reads as (branch `golden-g2`, 2026-08-03)

**Frontier (superseded by G2.1f above).** WP-G2's fifth teardown.
`SPLIT_ALIAS_POPULATION` **27 → 26**,
`TORN_DOWN_ROWS` carries its fifth row, and the WP acceptance criterion still
holds at this commit: `git diff --stat -- tests/golden` over the range is
**empty** and both lanes' `oracle_parity_cfg_gate` is green.

**The bug.** `TStorageControllerObj` ends both of its terminal dispatch branches
by idling the fleet and forcing a re-solve, and guards both with
`if not FleetState = STORE_IDLING` — "Ran out of OOMPH"
(`.inputs/dss_capi/src/Controls/StorageController.pas:1350`) and "Fully charged"
(`:1619`). In Object Pascal `not` binds tighter than `=`, and `FleetState` is an
`Integer`, so the compiler reads `(not FleetState) = STORE_IDLING`, i.e. a
**bitwise complement** compared against 0. That holds for exactly one value,
`-1 = STORE_CHARGING` (`Storage.pas:35-37`): `not 0 = -1`, `not 1 = -2`. The
guard therefore fires only when the fleet is *charging* and is false in the very
state that reaches "Ran out of OOMPH" — discharging. Such a fleet keeps
`STORE_DISCHARGING` and its old dispatched kW, no `PushTimeOntoControlQueue`
re-solves the step, and the event log prints "Fleet has been set to idling
state" anyway (`:1357-1359`, outside the guard). The charge branch is masked:
it is reached *from* charging, the one state the complement happens to accept.
That it is a precedence slip and not a convention is written seven times in the
same unit — every other test of this field is parenthesised, including the
literal `if not (FleetState = STORE_IDLING)` at `:1162` and `:1450`, plus
`:872`, `:969`, `:994`, `:1252`, `:1515`. r4133 carries the identical
unparenthesised pair
(`.inputs/electricdss-code-r4133-trunk/Version8/Source/Controls/StorageController.pas:1771`
and `:2042`), so both gating oracles have it and the 2026-08-02 policy applies
rather than exempts. Deep dive:
`investigations/issue-18-storagecontroller-idle-condition.md`.

**What landed.** `compat::STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL`
and both its `*_IMPL` twins are **deleted**;
`storage_controller/compute.rs::fleet_needs_idling` — the single method both
Pascal sites are ported through — is one unconditional
`self.fleet_state != StorageState::Idling`, with both upstream sites, both r4133
lines and the seven parenthesised siblings cited in place. The default lane
already did this, so **only the parity lane moves**; `compute.rs` no longer
imports `compat` at all. The compat module-doc row for the *Single-site upstream
quirks* section records this row as G2.1e.

**The pin became unconditional, and grew the observable.**
`storage_controller::tests::fleet_idle_guard_fires_unless_the_fleet_is_already_idling`
(renamed from `fleet_idle_guard_is_lane_split`) lost its `ORACLE_PARITY` branch
and carries the
`EXPECTED-VALUE-PIN(STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL)`
marker. It asserts two levels: the predicate over all three fleet states
(`Discharging` is where upstream disagrees), the three complement values that
make `Charging` upstream's only firing state — and, new here, the **observable**
a deck sees: a PeakShave fleet already `Discharging` into a 1 MW overage with
`kWhStored == kWhReserve` takes the "Ran out of OOMPH" branch and must end the
sample with `FleetState = STORE_IDLING`, its member idled, and `STORE_IDLING`
pushed onto the control queue. Without that second level the pin would not
notice a caller that stopped consulting the guard at all — the predicate is only
half the claim the register promises moved. Both levels were mutation-checked
against the upstream kernel (`(!ordinal) == Idling.ordinal()` restored in
`fleet_needs_idling`): each one reds on its own — the predicate at `Discharging`
(`false` vs `true`), the observable at `sc.fleet_state` (`Discharging` vs
`Idling`). Both mutations were reverted; `git diff` over `compute.rs` matches
what this commit ships, so the register's `Evidence::Site` needle
(`"\n        self.fleet_state != StorageState::Idling"`, anchored on the
eight-space function-body indentation) still matches byte-for-byte. No
kernel-vs-kernel test existed for this row, so none was deleted.

**Zero-footprint, measured not assumed.** The lanes could only differ for a
fleet that reaches "Ran out of OOMPH" or "Fully charged" in a state that is
neither idling nor charging — in practice, energy exhausted mid-discharge. No
committed golden and no gated corpus deck drives a StorageController there,
which is why the **default** lane, which has asked the state test since F.3l,
has been byte-green on every golden all along. So no golden byte, no
`ledger.json` entry and no `population.lock.json` field moves; the **parity**
lane's full suite, the unconditional 520-case corpus gate against both oracle
channels included, is green, and that is the measurement confirming the
classification rather than assuming it. `lane_diff.ps1` re-run because a lane
alias was deleted: over 520 cases / 3 219 862 records every gated kind
(`conv`/`cur`/`errs`/`iter`/`loss`/`pow`/`v`/`y`) is **identical**, max |Δ| = 0,
zero iteration drift, `VERDICT: PASS`; the only entries are the documented
Newton pow/loss divergences on the two `newton` decks — the still-live
`POWERS_REUSE_STALE_NEWTON_ITERMINAL` row that G2.3 removes.

**One gate flake, named.** The first `cargo test --workspace` run reddened on
`corpus_gate` with an **oracle-side** I/O error on a deck this row cannot reach:
`[R4133] oracle case failed: DSS error #303 … export losses
file=Auto1bus_HL_losses.txt … I/O error 103` (`Test/AutoTrans/Auto1bus.dss`, an
autotransformer deck with no StorageController). A clean re-run of
`corpus_gate` alone came back 53/53, and the full five-command gate was then
re-run end to end on both lanes, green. The export artifacts the aborted case
left under `tests/corpus/electricdss-tst/Test/AutoTrans/` were removed
file-by-file, never committed; the failure is filed here as an r4133-channel
export-path flake under the parallel scheduler, not a result.

**Doc surface.** The alias was never cited on the walked doc surface (measured
again here: outside `crates/` the only mentions are STATUS, the plan and the
gitignored `investigations/`, none of which `operational_docs` reads), so no
citation was struck and the non-vacuity floor is untouched. CLAUDE.md names this
row nowhere — it is not one of the six named bugs — so its policy §Status
sentence is unchanged. The historical DE_PASCALIZE F.3l record below keeps its
text and gains a supersession marker on its table row and its paragraph.

**Fix pass (audits).** Four findings, all minor; three fixed as stated, the
fourth fixed **against a corrected premise**. No engine line moved. (1) The
`TORN_DOWN_ROWS` row repeated the over-claim G2.1c's fix pass had corrected
three sub-steps earlier: an early-return re-split
(`if compat::… { return (!…) == …; }` followed by the anchored line, or the same
with a `#[cfg]`-guarded `return`) leaves the statement at exactly the eight
spaces the needle anchors on, so a lane branch *can* be restored around it. The
row now takes `Evidence::Site`'s documented fallback — it says the check degrades
to "not deleted outright" for that shape and names what carries the
discrimination instead (the unconditional pin, which asserts the `Discharging`
answer in both lanes; the ghost check; the census tie) — and the sentence above
was corrected the same way. (3) Its CRLF justification was stated as a fact about
the file, which `git ls-files --eol` contradicts (`compute.rs` is `w/lf` right
now, this sub-step's own write). Restated as the portability argument it always
was: `core.autocrlf=true` with no `*.rs` rule in `.gitattributes`, so a fresh
checkout is CRLF and only a single-line needle with a leading `\n` survives both.
(2) The observable staged the bug's other half — the 400 kW upstream leaves
dispatched — and never asserted it; worse, `SetFleetToIdle`'s `env.set_kw(r, 0.0)`
could be deleted without reddening anything. The auditor's suggested fix (clear
`pctkWOut`/`kW_out` in the mock's zero arm) is **refuted**: Pascal `Set_kW`'s zero
arm sets `FState := STORE_IDLING` and nothing else
(`.inputs/dss_capi/src/PCElements/Storage.pas:3444-3461`), and this branch queues
no `SetNominalDEROutput`, so the element genuinely keeps its old `kW_out` —
clearing it would be the falsification. `MockStorage` instead records its `Set_kW`
writes (the `nominal_calls` pattern already in the mock), and the pin asserts
`kw_writes == [0.0]`; deleting the write now reds it (measured). (4) The "Fully
charged" caller (`compute.rs:1037`, Pascal `:1619`) had no observable — true, and
now fixed by `fully_charged_branch_idles_the_fleet`. But the finding's framing is
**refuted**: that site cannot discriminate this row. `DoPeakShaveModeLow` returns
at its `actual_kWh >= total_rating_kWh` skip for a discharging or idling fleet, so
the branch is reachable only with `FleetState = CHARGING` — the one value
upstream's `(not FleetState) = 0` also accepts. Measured: restoring the complement
at that site alone leaves all 42 storage-controller tests green, while hardcoding
the guard `false` there reds the new test and nothing else. The new test is
therefore documented as a caller pin, not a lane pin, and the main pin's doc says
why level 2 lives at the discharge branch.

### GOLDEN_REBASE G2.1d — `REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT` torn down: the merge scans the whole shunt list (branch `golden-g2`, 2026-08-03)

**Frontier.** WP-G2's fourth teardown. `SPLIT_ALIAS_POPULATION` **28 → 27**,
`TORN_DOWN_ROWS` carries its fourth row, and the WP acceptance criterion still
holds at this commit: `git diff --stat -- tests/golden` over the range is
**empty** and both lanes' `oracle_parity_cfg_gate` is green. Next: G2.1e
(`STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL`, `issue-18`), same
ritual.

**The bug.** `DoReduceShortLines`' merge-with-parent branch must not merge a
short line into its parent while the intermediate bus carries a capacitor or a
reactor — the merge deletes that bus, and the compensating element would be
silently relocated. The guard is written as a loop over the *parent* node's
shunt list, but the loop opens on one node and continues on another: `ShuntElement
:= ParentNode.FirstShuntObject()` (`.inputs/dss_capi/src/Meters/
ReduceAlgs.pas:200`) and `ShuntElement := PresentBranch.NextShuntObject()`
(`:209`). Each `TDSSPointerList` carries its own cursor, and the present
branch's still sits where tree construction left it — `Add` sets `ActiveItem :=
Result`, i.e. the last item (`.inputs/dss_capi/src/Shared/DSSPointerList.pas:88`)
— so the very first `Next` runs off the end and returns `NIL` (`:113-131`). The
scan therefore ends after **one** element: a capacitor at position ≥ 2 does not
block the merge and is moved onto the merged line's far bus (`:226`), changing
where reactive power is injected, with no message to the user. The case is
typical rather than exotic: the bus shunt list is filled with PC elements first
and shunt PD elements after, so a capacitor at a bus that also carries any load
is *never* first — the one-element scan is blind exactly where it was written to
look. That it is a slip and not a rule is proven inside the same procedure: the
second loop of this very branch, the one that relocates the shunts after a
successful merge, uses the parent for both calls (`:223`/`:228`), and the
merge-with-child branch forty lines below uses one node throughout
(`:248`/`:257`). r4133 carries the identical pair
(`.inputs/electricdss-code-r4133-trunk/Version8/Source/Meters/
ReduceAlgs.pas:199`/`:206`), so both gating oracles have it and the 2026-08-02
policy applies rather than exempts. Deep dive:
`investigations/issue-31-reduce-parent-shunt-first-only.md`.

**What landed.** `compat::REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT` and both its
`*_IMPL` twins are **deleted**; `exec/reduce.rs::red_short_line_step` scans the
parent's whole shunt list with one unconditional
`parent_shunts.iter().any(red_is_cap_or_reactor)` — byte-for-byte the predicate
the merge-with-child branch twenty lines below has always used — with both
upstream sites and the cursor mechanism cited in place. The default lane already
did this, so **only the parity lane moves**; `reduce.rs` no longer imports
`compat` at all. The compat module-doc row for the *Single-site upstream quirks*
section records this row as G2.1d.

**The pin became unconditional.**
`exec::tests::reduce::short_line_merge_scans_every_parent_shunt` (renamed from
`short_line_parent_shunt_scan_is_lane_split`) lost its `ORACLE_PARITY` branch and
carries the `EXPECTED-VALUE-PIN(REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT)`
marker. Its builder `reduce_shortlines_keeps_b2` now takes a `ShortLineFeeder`
(load at `b2`, capacitor at `b2`, `l1` short or long) instead of one flag, and
the pin asserts **five** inputs on the one feeder
`src —lfeed(long)→ b1 —l1→ b2 —l2(short)→ b3`. With `l1` short: capacitor second
(a load ahead of it) → `b2` stands; capacitor first (alone at `b2`) → `b2`
stands; no capacitor at all → `b2` is eliminated. With `l1` **long** — which
takes `l1`'s own merge-with-**child** arm out of the walk and leaves `l2`'s
merge-with-**parent**, the site under test, as the only reduction available:
load only → `b2` is eliminated; load + capacitor → `b2` stands.
No kernel-vs-kernel test existed for this row, so none was deleted.

The two long-`l1` inputs are the fix agent's answer to the audits (below); the
three short-`l1` ones are what the implementation commit shipped. The `l1`-short
non-vacuity input eliminates `b2` through the merge-with-**child** arm, so on
its own it proves the deck reduces but not that the arm under test can ever
*succeed* — a guard that over-blocks (`!parent_shunts.is_empty()`) passed all
three. Mutation-checked at the fix: over-block → input 4 red; scan the head only
(the upstream bug) → input 1 red; drop the guard → input 1 red. Every mutation
was applied to `red_short_line_step`, run, and reverted; `git diff` over
`exec/reduce.rs` is empty against the implementation commit, so the register's
`Evidence::Site` needle still matches byte-for-byte.

**Zero-footprint, measured not assumed.** The lanes could only differ on a
topology reduced with `ReduceOption=ShortLines` whose parent branch carries ≥ 2
shunts, the first of them not a capacitor/reactor and a later one being one. No
committed golden and no gated corpus deck reduces such a topology — which is why
the **default** lane, which has scanned the whole list since F.3l, has been byte-
green on every golden all along. So no golden byte, no `ledger.json` entry and no
`population.lock.json` field moves; the **parity** lane's full suite, the
unconditional 520-case corpus gate against both oracle channels included, is
green, and that is the measurement confirming the classification rather than
assuming it. `lane_diff.ps1` re-run because a lane alias was deleted: over 520
cases / 3 219 862 records every gated kind
(`conv`/`cur`/`errs`/`iter`/`loss`/`pow`/`v`/`y`) is **identical**, max |Δ| = 0,
zero iteration drift, `VERDICT: PASS`; the only entries are the
documented Newton pow/loss divergences on the two `newton` decks — the still-live
`POWERS_REUSE_STALE_NEWTON_ITERMINAL` row that G2.3 removes.

**Doc surface.** The alias was never cited on the walked doc surface (measured
again here: outside `crates/` the only mentions are STATUS, the plan and the
gitignored `investigations/`, none of which `operational_docs` reads), so no
citation was struck and the non-vacuity floor is untouched. CLAUDE.md names this
row nowhere — it is not one of the six named bugs — so its policy §Status
sentence is unchanged.

**Fix pass (audits).** Three findings — two of them (one filed `major`, one
`minor`) the same defect, all real, all fixed; nothing deferred, no engine line
moved. (1)+(2) The pin's inputs never witnessed the merge-with-parent arm
*succeeding*. Both auditors traced the non-vacuity input (load at `b2`, no
capacitor) to `l1`'s merge-with-**child** arm — with `l1` short the walk reaches
it first, its `present_shunts = [ldb2]` carries no capacitor, so it merges `b2`
out itself and `red_reduce_short_lines`' extra `GoForward` consumes `l2` before
the site under test is ever reached. Confirmed by mutation rather than by
reading: `if !parent_shunts.is_empty()` in place of the scan left all three
inputs green. The deleted parity arm had carried that witness (`assert_eq!(kept,
!ORACLE_PARITY)` positively required `b2` to be merged out **through** the parent
arm), so the teardown had shrunk the pin's mutation coverage against the
register's "the coverage moved rather than evaporated" promise. Fixed with the
two long-`l1` inputs described above; the over-blocking mutation now dies on
input 4, and both auditors' other enumerated mutations were re-checked as still
dying (see the pin paragraph). (3) The historical DE_PASCALIZE F.3l section
(`§F.3l`) still described the row as a live lane split and named the pre-rename
test in the present tense; it keeps its history and gains a supersession marker
on both the table row and the paragraph. Its Pascal citation `Add` sets
`ActiveItem := Count`, `DSSPointerList.pas:66` was wrong twice over — `:68` is
`ActiveItem := 0` in `Create`, and `Add` ends at `:88` with `ActiveItem :=
Result` — and the same correction is already recorded at §"Citations" above, so
the F.3l occurrence is now fixed to match rather than left as the one place that
disagrees. Local-only `investigations/` (the `issue-31` report and the registry's
«Судьба» entry) re-synced to the five inputs.

### GOLDEN_REBASE G2.1c — `SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING` torn down: an undefined rating is not a percentage (branch `golden-g2`, 2026-08-03)

**Frontier.** WP-G2's third teardown. `SPLIT_ALIAS_POPULATION` **29 → 28**,
`TORN_DOWN_ROWS` carries its third row, and the WP acceptance criterion still
holds at this commit: `git diff --stat -- tests/golden` over the range is
**empty** and both lanes' `oracle_parity_cfg_gate` is green. Next: G2.1d
(`REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT`, `issue-31`), same ritual.

**The bug.** `CalcAndWriteSeqCurrents` uses one variable for two different
quantities: it seeds `iNormal := TPDElement(Pelem).NormAmps` — an ampere rating
— and then *overwrites* that seed with the loading `I1/NormAmps*100`, but only
`if iNormal > 0.0` (`.inputs/dss_capi/src/Common/ExportResults.pas:409-414`;
r4133 `Version8/Source/Common/ExportResults.pas:355-358` is the same four lines,
so both gating oracles carry it). The guard reads as division-by-zero
protection, and it is — but on the branch it protects, the variable keeps a
value of the *other* meaning and the *other* dimension, and that is what reaches
the `%Normal`/`%Emergency` columns of the written row (`:428`). So an element
with `normamps=-1` is reported as loaded to −1 %, silently: downstream
processing that thresholds or sorts on that column takes the rating for a
percentage. `normamps=0` is the one input on which the two readings coincide.
The clean fix is what the report's own `else` arm already writes for every other
terminal and every non-PD element (`:416-420`): `0`. Deep dive:
`investigations/issue-12-seqcurrents-normamps-passthrough.md`.

**What landed.** `compat::SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING` and both
its `*_impl` twins are **deleted**; `report/export/seq_currents.rs` computes both
columns through one `pct_of_rating` closure that returns `0.0` for a
non-positive rating, with both upstream sites cited in place. The default lane
already did this, so **only the parity lane moves**; the compat module-doc row
for the *Single-site upstream quirks* section records this row as G2.1c. The
report's second, independent upstream bug — `IRESIDUAL_FROM_TERMINAL_1`, ten
lines below in the same loop — is untouched and still lane-split; G2.2a owns it.

**The pin became unconditional.**
`golden_reports::export_seqcurrents_prints_zero_for_an_undefined_rating`
(renamed from `…_nonpositive_rating_is_the_lane_kernel`) lost its
`ORACLE_PARITY` branch and asserts `(0.0, 0.0)` for `Line.bad`
(`normamps=-1 emergamps=-2`) outright, carrying the
`EXPECTED-VALUE-PIN(SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING)` marker. Its deck
gained a third line, `Line.zero` (`normamps=0 emergamps=0`), placed in series
*ahead of* the load so it carries current: that is the boundary of the surviving
`> 0` guard and the input on which the quirk and the fix agree. The two lines
kill complementary mutations: a sign-blind `abs(rating)` reports `Line.bad`'s
−1 A rating as a loading of +1 % and fails there (it passes on `Line.zero`,
where `abs(0)` is still not `> 0`), while a guard relaxed to `>=` or dropped
altogether divides by `Line.zero`'s undefined rating and fails there (a `>=`
passes on `Line.bad`, whose −1 takes the `else` arm either way). The
positively-rated `Line.good` still proves
the ordinary percentage path is untouched. Non-vacuity was measured, not
assumed: restoring the upstream seed in the closure fails the pin with
`left: (-1.0, -2.0)`. No kernel-vs-kernel test existed for this row, so none was
deleted.

**Zero-footprint, measured not assumed.** The lanes could only differ on an
element with a *negative* `normamps`/`emergamps`, and no committed golden and no
gated corpus deck has one — the corpus's unset ratings are zeros
(`EPRITestCircuits/ckt7/LineCodes_ckt7.dss:137`/`:161`,
`ckt5/WireData_ckt5.dss:5`), where both readings print `0`. So no golden byte, no
`ledger.json` entry and no `population.lock.json` field moves; the **parity**
lane's full suite, the unconditional 520-case corpus gate against both oracle
channels included, is green, which is the measurement that confirms the
classification rather than assuming it. `lane_diff.ps1` re-run because a lane
alias was deleted: over 520 cases / 3 219 862 records every gated kind
(`conv`/`cur`/`errs`/`iter`/`loss`/`pow`/`v`/`y`) is **identical**, max |Δ| = 0,
zero iteration drift; the only entries are the documented Newton pow/loss
divergences on the two `newton` decks — the still-live
`POWERS_REUSE_STALE_NEWTON_ITERMINAL` row that G2.3 removes.

**Doc surface.** The alias was never cited on the walked doc surface (measured
again here: the only mentions outside the code are STATUS and the plan, neither
of which `operational_docs` reads), so no citation was struck and the
non-vacuity floor is untouched. CLAUDE.md names this row nowhere — it is not one
of the six named bugs — so its policy §Status sentence is unchanged. The
`Export SeqCurrents` sentence in `tests/TOLERANCE_NOTES.md` is about the
`Iresidual` row, not this one, and stays for G2.2a.

**Fix pass (audits).** Four findings, all minor, all real, all fixed; nothing
deferred, no engine line moved (the code the auditors traced is correct — every
mutation they enumerated does die on one of the deck's three lines). (1) The
row's `Evidence::Site` slice claimed the same completeness G2.1b's fix pass had
just corrected for the CapControl row: it discriminates a re-split that wraps the
*call pair* in a lane branch, but not one written inside `pct_of_rating` itself
(`} else if compat::… { rating }`), which leaves the anchored line byte-identical.
Re-anchoring on the whole closure is not available — the register's check is a
plain `contains` over the file as checked out, and Rust sources here check out
CRLF (`git ls-files --eol`), which only a single-line needle with a leading `\n`
survives — so the row now takes `Evidence::Site`'s documented fallback: it states
that for that shape the check degrades to "not deleted outright" and names what
carries the discrimination instead (the unconditional pin, which asserts
`(0.0, 0.0)` for `Line.bad` in *both* lanes so any restored lane branch fails it
wherever it is written; the ghost check; the census tie). (2)+(4) — the same
finding twice: the mutant attributions in the deck comment and in the paragraph
above were inverted. Traced against the kernel, `Line.bad` is what kills a
sign-blind `abs` (its −1 A rating would render +1 %) and `Line.zero` is what
uniquely kills a `>=` boundary slip (`-1.0 >= 0.0` is false, so `Line.bad` passes
that mutation while `Line.zero` divides by zero); a dropped guard dies on both.
Both sentences corrected. (3) The `issue-12` deep dive still described the row as
a live lane split, citing the deleted `compat.rs:859`/`:861` twins, the deleted
`undefined_rating` closure and the pre-rename pin — the report bodies for G2.1a
and G2.1b were rewritten in their own sub-steps, and this one had been missed
(the registry's «Судьба» entry, which the plan demands explicitly, *was* correct).
Rewritten to the issue-11/issue-15 shape. `investigations/` is gitignored, so
that half is local-only and outside the gate.

### GOLDEN_REBASE G2.1b — `CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL` torn down: a clone keeps its signal (branch `golden-g2`, 2026-08-03)

**Frontier.** WP-G2's second teardown. `SPLIT_ALIAS_POPULATION` **30 → 29**,
`TORN_DOWN_ROWS` carries its second row, and the WP acceptance criterion still
holds at this commit: `git diff --stat -- tests/golden` over the range is
**empty** and both lanes' `oracle_parity_cfg_gate` is green. Next: G2.1c
(`SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING`, `issue-12`), same ritual.

**The bug.** `TCapControlObj.MakeLike` is a hand-written list of assignments,
and the list is complete only as far as it was carried: it copies the controlled
and monitored element, the capacitor name, the terminal, the whole `ControlVars`
block, the user model, `FpctMinkvar` and `ShowEventLog` — every reference the
object holds **except** the `ControlSignal` shape (`.inputs/dss_capi/src/
Controls/CapControl.pas:445-489`, field `ctrlSignalShape` declared `:169`,
`Create` leaves it `NIL` at `:500`). `ControlType` *is* copied, so the clone of a
`type=Follow` controller stays a Follow controller with nothing to follow: the
first sample takes the `ctrlSignalShape = NIL` branch (`:1150-1168`), raises
message **10362** and sets `SolutionAbort` — a deck that solves fine when the
controller is written out longhand aborts when it is written `like=`. r4133
shares the omission (`Version8/Source/Controls/CapControl.pas:410-465`, field
`myShapeObj` `:76`) and additionally copies the whole `PropertyValue` array
(`:460`), which makes the loss *invisible* to a property read there while the
controller is just as dead. Both gating oracles carry it; the 2026-08-02 policy
therefore applies rather than exempts. Deep dive:
`investigations/issue-15-capcontrol-copy-loses-controlsignal.md`.

**What landed.** `compat::CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL` and both its
`*_impl` twins are **deleted**; `cap_control/accessors.rs::make_like` copies
`control_signal_name` and `ctrl_signal_shape` unconditionally, next to the two
snapshots it already copied, with both upstream sites cited in place. The
default lane already did this, so **only the parity lane moves**; the compat
module-doc row for the *Single-site upstream quirks* section now records that
WP-G2 is emptying it, this row included.

**The pins became unconditional — and gained a second one.**
`cap_control::tests::make_like_copies_the_control_signal` (renamed from
`…_is_the_lane_kernel`) asserts three separable halves literally: the name
(`"sig"`), the reference (`is_some()`), and the consequence — the clone samples
its shape, arms `Close` and raises no error, so copying only the name would
still fail. The new `…::like_on_a_follow_capcontrol_keeps_following` pins the
same row where a user meets it: a deck writing `New CapControl.b like=a` over a
`type=Follow` source, then — read straight after `solve`, before any further
command — no `solution_abort` and no 10362 in the error log, and only then
`? CapControl.b.ControlSignal` = `sig`, which is the property observable
(`accessors.rs` `CONTROLSIGNAL`) and the exec `like=` applier that the unit pin
does not reach. Both carry the
`EXPECTED-VALUE-PIN(CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL)` marker, and both
were confirmed non-vacuous by commenting the two copies out (both fail, on the
name and on the property respectively) — and the deck pin again in the fix pass
against the *harder* half alone, the shape copy removed while the name still
copies. No kernel-vs-kernel test existed for this row, so none was deleted. The trailing `converged_flag` assert is sanity,
not discrimination — this deck converges either way (`solve_snap` never reads
`solution_abort`) — and says so at the site (G2.1b fix pass, below).

**Zero-footprint, measured not assumed.** Exactly one `like=` on a CapControl
exists in the whole tree — `tests/golden/props/capcontrol.json::capcontrol_makelike`
(`New CapControl.cc1 like=base capacitor=cap2`) — and its source `base` is
`type=kvar` with an empty `ControlSignal`, so the copy moves an empty string and
a `None`: the golden's `"ControlSignal": ""` is unchanged. The three decks that
*do* use a Follow controller with a signal (`corpus/controls/capcontrol/
capcontrol_follow.dss`, `…_follow_noshape.dss`, vendored
`Test/CapControlFollow.dss`) contain no `like=` at all. So no golden byte, no
`ledger.json` entry and no `population.lock.json` field moves; the **parity**
lane's full suite, the unconditional 520-case corpus gate against both oracle
channels included, is green, which is the measurement that confirms the
classification rather than assuming it. `lane_diff.ps1` re-run because a lane
alias was deleted: over 520 cases / 3 219 862 records every gated kind
(`conv`/`cur`/`errs`/`iter`/`loss`/`pow`/`v`/`y`) is **identical**, max |Δ| =
0, zero iteration drift; the only entries are the *documented* Newton
pow/loss divergences on the two `newton` decks — the still-live
`POWERS_REUSE_STALE_NEWTON_ITERMINAL` row that G2.3 removes — so the default
lane keeps the parity lane's oracle standing.

**Doc surface.** The alias was never cited on the walked doc surface (measured
again here: the only mentions outside the code are STATUS and the plan, neither
of which the walk reads), so no citation was struck and the non-vacuity floor is
untouched. CLAUDE.md names this row nowhere — it is not one of the six
named bugs — so its policy §Status sentence is unchanged.

**Fix pass (audits).** Three findings, all minor, all real, all fixed; nothing
was deferred. (1)+(3) — the same finding twice: the row's `Evidence::Site` slice
`self.ctrl_signal_shape = other.ctrl_signal_shape.clone();` was byte-identical
in the *pre*-teardown tree (`0e89651f` `accessors.rs:106-109` merely wrapped it
in `if !compat::…`), so the register's rot-check #3 could only catch outright
deletion while `Evidence::Site`'s doc claimed every such slice discriminates a
revert. The slice now carries the leading line break and `make_like`'s own eight
spaces of indentation, so re-wrapping the copy in *any* lane branch re-indents it
past the needle — proven by re-wrapping it and watching
`every_torn_down_row_keeps_its_pin_and_its_evidence` fail. The `\n` matches an LF
and a CRLF checkout alike (`\r\n` contains `\n`). The `Evidence::Site` doc now
states this as the convention for rows whose fixed form is an ordinary statement
the split form also contained — which most of G2.1c–h will be — instead of
claiming discrimination comes free. (2) — the deck pin's third assert
(`converged_flag`) cannot fail on the pre-fix engine: `solve_snap`
(`solution/power_flow.rs:448-494`) never reads `solution_abort` and
`converged_flag` comes from the voltage-mismatch test alone, so this 3-bus deck
converges with or without the copies. The pin now asserts the flag the FOLLOW arm
actually sets (`!solution.solution_abort`) — the *state* half of the abort, next
to the message half the 10362 assert already carried — and keeps `converged_flag`
as an explicitly labelled sanity check. The auditor's suggested placement would
have been vacuous and the probe caught it: `Dss::command` clears `solution_abort`
on every external entry (`exec/command.rs:41-50`), so the assert had to move
*ahead* of the `? CapControl.b.ControlSignal` query; with only the shape copy
removed (name copied, reference not) it then fires, and at the auditor's
placement it would not have.

### GOLDEN_REBASE G2.1a — `stddev_single_point` torn down: one sample has no spread, in both lanes (branch `golden-g2`, 2026-08-03)

**Frontier.** The **first actual teardown** of `GOLDEN_REBASE_PLAN.md` WP-G2 has
landed, on the rails G2.0 laid the day before. `SPLIT_ALIAS_POPULATION` **31 →
30**, `TORN_DOWN_ROWS` carries its first row, and the WP's acceptance criterion
holds: `git diff --stat -- tests/golden` over the range is **empty**, and both
lanes' `oracle_parity_cfg_gate` is green at this commit. Next: G2.1b
(`CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL`, `issue-15`), same ritual.

**The bug.** `RCDMeanAndStdDev` and its three siblings handle a one-element
sample in a dedicated branch, and that branch assigns the mean and then repeats
the *same right-hand side* into the second out-param: `Mean := Data^[1];
StdDev := Data^[1];` — r4133 `Version8/Source/Shared/mathutil.pas:405` (and
`:429` in `CurveMeanAndStdDev`), identical in the pinned dss_capi 0.14.5 at
`:323`, `:349`, `:370`, `:398`, where the copy-paste ran to four procedures
(0.14.5 added the two `Single` variants r4133 does not have). So a `{3.5}`
sample is reported with a "standard deviation" of 3.5 — 100 % spread where by
construction there is none. Both gating oracles carry it, which is exactly why
the 2026-08-02 policy applies rather than exempts: r4133 sharing a defect is not
authority for reproducing it. Deep dive: `investigations/issue-11-stddev-single-point.md`
(English upstream copy in `investigations/to_opendss/`).

**What landed.** `compat::stddev_single_point` and both its `*_impl` twins are
**deleted**; the four `support::mathutil` entry points (`mean_and_std_dev`,
`mean_and_std_dev_single`, `curve_mean_and_std_dev`,
`curve_mean_and_std_dev_single`) return `0.0` outright, each with the upstream
line it declines to reproduce cited in place. The default lane already computed
`0.0`, so **only the parity lane moves**; the compat module-doc inventory row
now records the row as torn down.

**The pins became unconditional** — both of them, since the alias had two:
`mathutil::tests::single_point_std_dev_is_zero` (renamed from
`…_is_the_lane_kernel`) asserts the pair `(v, 0.0)` literally — both out-params,
so an entry point returning nothing fails as loudly as one repeating the sample
— at all four entry points over four magnitudes (`0.0`, `3.5`, `-12.25`,
`1.0e9`); the three non-zero ones are what separate the fixed kernel from
`StdDev := Data[1]`, which reproduces `0.0` for free on a zero sample;
`load_shape::tests::single_point_shape_stddev_property_is_zero` pins the
observable — `? LoadShape.one.stddev` on `npts=1 mult=(0.4)` reads `0` while
`mean` reads `0.4`, so a broken accessor cannot fake it by returning nothing.
Both carry the `EXPECTED-VALUE-PIN(stddev_single_point)` marker. The two
kernel-vs-kernel tests in `compat/tests.rs` (`stddev_alias_is_the_lane_kernel`,
`stddev_single_point_kernels_are_a_deliberate_divergence`) are deleted with the
kernels they compared — they asserted the two impls against each other, which is
meaningless once there is one.

**Zero-footprint, measured not assumed.** No golden and no gated corpus case
reads a *one-point* shape's std-dev: no golden scenario anywhere builds an
`npts=1` shape (measured over `tests/golden/props/{loadshape,tshape,priceshape}.json`
— the shapes that pin a computed `StdDev`, e.g. `loadshape_abbrev`'s `np=3`, all
have ≥ 3 points, and the single-point branch is unreachable for them). So no
golden byte, no `ledger.json` entry and no `population.lock.json` field moves.
The **parity** lane's full suite — including the unconditional 520-case corpus
gate against both oracle channels — is green, which is the measurement that
confirms the classification rather than assuming it. `lane_diff.ps1` re-run
because a lane alias was deleted: max |Δ| = 0, so the default lane keeps the
parity lane's oracle standing.

**Doc surface.** The row was never cited as `compat::stddev_single_point` on the
walked doc surface (G2.0's measurement, re-confirmed here), so no citation was
struck and the non-vacuity floor is untouched. CLAUDE.md's policy §Status
sentence did name "single-point stddev" in prose as *queued* for removal; it now
names WP-G2 as the owner and records this row as done.

**Audit settlement (fix pass, same branch).** Both auditors returned six minor
findings, all record-vs-tree mismatches; none disputed the teardown, and none was
declined. Three (both auditors raised the first) were STATUS text describing
something the tree does not have, corrected above: the pin's `assert_ne!` (it has
none — four literal `assert_eq!`s over four magnitudes) and the zero-footprint
justification (the `props` decks that read a *computed* `stddev` are `np=3`, not
`npts=4 stddev=…`; what carries the claim is that no golden scenario builds an
`npts=1` shape at all — the pin's own doc comment, which said "three magnitudes"
of the four it iterates, is corrected with them). Two were mechanism
fixes: the `TORN_DOWN_ROWS` evidence slice now anchors on the unconditional
kernel `return (data[0], 0.0);` instead of the sentence explaining it — a
comment survives a revert that re-routes the call, code does not, and that is now
the documented `Evidence::Site` convention for G2.1b–h; and
`investigations/issue-11-stddev-single-point.md` (local-only) still described the
lane split as current and cited three deleted tests — rewritten to the post-G2.1a
state, matching the registry entry §3.3 that the implementation pass had already
updated. The sixth named the value's **second** consumer: `std_dev()` feeds the
Gaussian draws at `solution/solution/monte_carlo.rs:188` (`Set random=gaussian`
LoadMultiplier) and `elements/pc/load/nominal.rs:118` (`Load::randomize`), where
a one-point yearly shape used to draw `G01·0.4 + 0.4` — ±100 % of mean, with a
negative tail that turns the load into a source. No gated deck reaches it (every
Monte deck runs `random=none`; `lane_diff` max |Δ| = 0 over the 520 cases), so it
moved no byte, but the path now has its own unconditional pin,
`load::tests::randomize_gaussian_with_single_point_yearly_is_constant`, carrying
the row marker: the shape randomizes to exactly `0.4` whatever the RNG draws.

### GOLDEN_REBASE G2.0 — the teardown gets its rails before the first deletion (branch `golden-g2`, 2026-08-02)

**Frontier.** `GOLDEN_REBASE_PLAN.md` WP-G0 is complete (below); **WP-G2 opens
here with rails only** — no compat kernel, no alias and no engine line was
touched (the fix round edited two `dss-parser/src/compat.rs` doc comments, so
`lane_diff.ps1` was run regardless: max |Δ| = 0), and
`git diff --stat -- tests/golden` is empty. G2.1a (`stddev_single_point`) is the
first actual teardown. WP-G1 may still interleave at sub-step granularity under
§0's constraints (G2.2a before G1.6, G2.4 before any G1 step re-touching
`compare_monitor`), on this one branch.

**What landed — (a) the doc-citation re-anchor.** New TESTING.md subsection
*"Precision-compat rows still split by lane"*: the five **numeric** survivors —
`compat::PI`, `compat::round_f64`, `compat::round_i32` (dss-parser),
`compat::kv_base_search_scale`, `compat::profile_ll_pu_divisor` (dss-core) — with
each row's parity kernel, default kernel and the expected-value test that pins
it, plus the marker convention below. Why it had to land *before* the first
deletion: `oracle_parity_cfg_gate.rs::operational_docs_cite_the_compat_machinery_accurately`
carries a non-vacuity floor of four `compat::` references across the walked doc
surface, and **all seven** live references name rows this WP deletes (CLAUDE.md
×4, `tests/TOLERANCE_NOTES.md` ×1, `tools/golden/gen_props.py` ×1,
`tests/corpus/modes/manifest.json` ×1) — each struck in the commit that deletes
its row. The surface now carries 17 references, 10 of them on rows neither WP-G2
nor WP-G4 touches (they are UPGRADE-line property), so the floor stays
load-bearing through the whole teardown instead of being re-argued per sub-step.
The new lines deliberately spell no compat tag, so none of them can trip the
`TAG_PATH_CITATIONS` "unregistered" assert that fires on a line carrying both the
tag and a `.rs` path.

**(b) `TORN_DOWN_ROWS`, created empty.** In `oracle_parity_cfg_gate.rs` (not a new
binary — `repo_root`/`rust_sources`/`test_region`/`is_pin_candidate`/`names_token`
are private to it, and that file already *is* the compat-machinery register):
`(row name, Kind::{SplitAlias,WholeCase}, Evidence::{Site,Exclusion,Ledger,None},
Option<(pin file, pin fn)>)`. Both censuses are single integers, so "row X was torn
down" and "row X quietly stopped being counted" are the same edit today; the
register makes them different edits.
`every_torn_down_row_keeps_its_pin_and_its_evidence` checks six independent rots:
the pin fn is still **declared** `#[test]` inside a real test region
(`is_pin_candidate`, so a compat module's kernel-vs-kernel test or this bookkeeping
file cannot pose as one — and `test_fn_body`, so a comment mentioning a deleted
test cannot pose as one either); that pin's **body no longer branches on the
lane**, which is what "the exclusion/pin becomes unconditional" actually means; an
`Evidence::Site`/`Exclusion` slice still matches; an `Evidence::Ledger` id still
exists in `tests/corpus/ledger.json` (`Evidence::None` is refused outright until
WP-G4); the two arithmetic ties `31 − count(SplitAlias) == SPLIT_ALIAS_POPULATION`
and `4 − count(WholeCase) == EXIT_POPULATION[WholeCase]`; and — because those ties
are *counts* — that no `SplitAlias` row's name is still in the live split set, so
a delete-one/register-another swap cannot balance them.

**(c) The greppable markers.** `// LANE-EXCLUSION(<row>): <why>` at every
exclusion a teardown makes unconditional and `// EXPECTED-VALUE-PIN(<row>): <why>`
at every pin, checked by `teardown_markers_and_the_register_agree` both ways in
the `markers_in_tree` discipline: a marker naming a row the register does not
carry fails (with the register empty, that is *every* marker — the convention is
live before the first row lands), and a registered row whose recorded pin file
carries no marker naming it fails too — as does one whose evidence is an
`Evidence::Exclusion` whose file carries no `LANE-EXCLUSION` naming it. A *blanket*
exclusion-marker obligation would be wrong (a row whose fix touched no harness has
none), so the row declares which of its evidence is an exclusion and the check
cashes that declaration. Malformed occurrences (outside a comment, empty row name,
missing `):`) are a hard failure rather than a silent skip, since an unparseable
marker is invisible to the grep it exists for; and the walk carries a non-vacuity
floor (`rust_sources > 300`) so an empty walk cannot pass as a clean tree.

**Design decisions worth carrying forward.**

*The pin slot is `Option` but every row must fill it.* WP-G4's rendering rows are
planned as `(SplitAlias, Evidence::None, None)` — the plan's G4 preamble makes the
pin mandatory "only for WP-G2 bug rows". Rather than guess a discriminator the row
shape does not carry, the check demands a pin from **every** row, so the first
pinless row is a deliberate edit to this assert in the commit that lands it. That
is the register's whole purpose applied to itself.

*`#[expect(dead_code)]`, not `#[allow]`, on `Evidence`.* No variant is constructed
while the register is empty. `expect` retires itself: once the last variant gets
its first row the attribute becomes unfulfilled and must be deleted, where an
`allow` would keep covering a variant that later goes unused for real. `Kind` needs
no attribute — both its variants are named as *values* by the census filters.

*The marker strings are assembled at runtime* (`format!("LANE-EXCLUSIO{}(", "N")`),
the same trick `compat_tag()`/`needle()` use, so the file that polices the markers
carries no literal occurrence of one and cannot trip its own walk. The
human-readable spelling therefore lives in TESTING.md, which the doc comment names.

**Negative probes** (each run, observed red, reverted — tree verified clean
afterwards). (1) A register row pointing at a nonexistent pin fn *and* a
nonexistent evidence slice: both reported, plus the missing marker. (2) The same
row made valid (real pin fn, real site slice, tie adjusted): only the
row→marker direction fails — then adding the marker turns the whole file green,
which is the accept path proven end-to-end rather than assumed. (3) A deliberately
wrong tie (a row with the census left at 31): `31 − 1 torn down ≠ 31`. (4) A stray
marker with the register empty: unregistered-marker failure. (5) A marker without
its closing `):`: malformed-shape failure.

**Recorded, not fixed (out of G2.0 scope).** The two-lane table at `TESTING.md:73`
still claims the parity lane reproduces "every upstream quirk" and "never
re-baselines" — stale since the 2026-08-02 policy. `GOLDEN_REBASE_PLAN.md` G5.1
already owns that exact line, so it is left to it rather than rewritten here.

**Audit settlement (fix agent, 2026-08-03).** Nine findings (2 major, 7 minor;
two of the minors were the same defect reported by both auditors). **All nine
fixed** — none refuted, none deferred.

*The two majors were the same class: a rail that claims more than it checks.*
(1) TESTING.md named `make_integer_rounds_ties_to_even` as the `compat::round_i32`
pin; that test (`dss-parser/src/parser/tests.rs:196`) asserts only ties-to-even —
behaviour **both** kernels share — so it cannot fail if the flip is reverted. The
lane pin is `make_integer_out_of_range_is_the_lane_kernel` (`:224`, reads
`ORACLE_PARITY`). The wrong name came from `dss-parser/src/compat.rs:135`, fixed
there too, along with its sibling at `:119` (`growth_shape::tests::…` — the test
lives in `dss-core/src/exec/tests/compat_quirks.rs:290`). This matters beyond a
typo: the plan has each teardown copy that name into the register's pin slot, so a
stale name would seed a row whose "pin" cannot detect a silently reverted fix.
(2) The register checked that a pin *exists*, not that it became *unconditional* —
so a pin rewritten as `if ORACLE_PARITY { /* nothing */ } else { assert_eq!(…) }`
would pass while asserting nothing on the parity side, and once the row leaves the
census `every_lane_split_alias_is_pinned_by_an_expected_value_test` stops looking
at it. Now `test_fn_body` slices the pin's brace-balanced body (a five-state
scanner: raw strings, `{}` placeholders and commented-out code all unbalance a
naive count) and `reads_the_lane` fails it on any of the **three** spellings — the
engine constant `ORACLE_PARITY`, the cfg, and the harness constant
`harness::lane::PARITY` (`lane.rs:78`). The third is not optional: `golden_reports.rs`,
which will hold most of G2.2's exclusion-flavoured pins, branches only that way, so
a check that knew the engine constant alone would wave through exactly the pins the
largest sub-steps produce. Body, not file, because `compat_quirks.rs` legitimately
holds still-split rows' lane-branching pins next door.

*Minors, all real.* Bare-token pin matching (a leftover `// superseded by <fn>`
comment kept a deleted pin "present") → the declaration form `#[test] fn <func>` is
now required. `Evidence::None` accepted for any row → refused until WP-G4's
rendering rows, mirroring the pin slot's "first exception is a visible edit".
Census-as-count instead of set → the live split set is now consulted. No
non-vacuity floor on the marker walk → `rust_sources > 300`. Exclusion markers had
no row→marker direction → `Evidence::Exclusion`. TESTING.md's column header claimed
the parity kernel is "what both gating oracles do", but `round_f64`'s mechanism is
the pinned capi's `TPropertyFlag.ApplyRound` array path, which **r4133 does not
have at all** (no `DSSObjectHelper.pas`, no `ApplyRound`; its `GrowthShape.Year` is
`pIntegerArray` — `GrowthShape.pas:82/224` — i.e. the `round_i32` row) → header
narrowed to "parity kernel", the capi-only mechanism called out in the row, and the
other four rows' r4133 lines cited inline after verification (`Parser/RPN.pas:247`,
`Common/Solution.pas:2541`, `Common/ExportResults.pas:3455/3471/3488`). Last, the
`profile_ll_pu_divisor` row mixed units — "three line-to-line arms" (branches) vs
"the eight line-to-neutral arms" (there are four; eight is the *division* count) →
"the eight line-to-neutral divisions".

*Every new check was probed red before being trusted*, on throwaway register rows
(reverted, tree verified clean): a pin that still reads `ORACLE_PARITY`, and a
second that reads `lane::PARITY` instead → the unconditionality failure, naming the
spelling; a pin named only in a comment → the declaration failure;
`Evidence::None` → the evidence failure; a `SplitAlias` row naming a still-split
alias with the arithmetic balanced → the ghost failure; an `Evidence::Exclusion`
whose file carries the pin marker but no exclusion marker → the exclusion-marker
failure. The negative control matters as much: a row pinned by
`make_integer_rounds_ties_to_even` (and one by `show_busflow_matches_oracle`),
whose bodies end immediately above a lane-branching test, reported **nothing** —
which is what proves `balanced_block` stops at the right brace instead of bleeding
into the neighbour.

Gate green in both lanes; `git diff --stat -- tests/golden` still empty. The only
non-test source touched is `dss-parser/src/compat.rs`, doc comments only — no
alias, no kernel — and `lane_diff.ps1` was run anyway: **max |Δ| = 0**.

### GOLDEN_REBASE G0.2 — the regen button gets its guards before it gets a caller (branch `golden-g0`, 2026-08-02)

**Frontier.** `GOLDEN_REBASE_PLAN.md` WP-G0 (safety rails) is **complete**: G0.1
locked the corpus's provenance, G0.2 lands the guarded writer that reads that lock.
WP-G1 (live-gate expansion to fastdss parity) is next. Nothing in WP-G1/G2/G3 has
started, and **no golden byte has moved** — `git status tests/golden` is empty
after full runs of both lanes.

**What landed.** `crates/dss-core/tests/harness/regen.rs`: `harness::regen()` — the
shared `DSS_UPDATE_GOLDENS` plumbing — with `harness::snapshot_text()` /
`snapshot_bytes()` routed through it. Without the knob they return
`Outcome::NotRequested` and touch nothing, which is what keeps a driver from
blessing the very output it is about to compare. Armed, every write passes the two
§1.2 guards, read from `golden.lock.json`:

1. **anchor** — anything not anchored `self` is refused (`ExternallyAnchored`): those
   bytes are another engine's capture, and de-anchoring is the reviewed
   `golden_lock.rs::DEANCHORED` edit, never a regen side effect. This is what will
   let G3.3a snapshot 144 of `reports/export*`'s 146 files while the two `capi015`
   `.meta.json` sidecars are skipped, untouched.
2. **producing lane** — a `parity`-produced family is refused from the default build
   (`WrongLane`); `lane-invariant` — only ever set after a cross-lane regen measured
   it — is writable from either. Until WP-G4 the lanes render different bytes, so
   writing a parity family from the default lane would re-baseline the strict lane's
   byte contract onto the other lane's rendering.

Two further refusals fall out of the same read: `Unlocked` (no row — writing there
would smuggle an unfingerprinted artifact into the corpus, the hole G0.1 closed) and
`NoProducingLane` (`anchor: self` with a null `produced_by`, i.e. a hand-edited lock;
`golden_lock.rs` asserts the equivalence, so the rails refuse rather than guess).

**Design decisions worth carrying forward.**

*A refusal is a skip, not a panic.* A family regen legitimately sweeps artifacts it
must not touch (G3.3a above; G3.5's six `capi015` `props/` scenarios), so a panic
would make the documented per-family procedure impossible. Instead every refusal is
announced on stderr **with its remedy** and the artifact is left byte-identical —
which the unit tests assert by re-reading it after the refusal.

*The rails read the lock; they never write it.* `golden_lock.rs` owns the lock and
re-derives every anchor from its registers, so a writer that also rewrote rows could
launder a provenance claim — and 21 test binaries regenerating concurrently would
race over one file. A regen run therefore leaves the lock **stale on purpose**:
`golden_lock.rs` goes red with `DIGEST MOVED` until the operator reviews the diff and
runs `DSS_UPDATE_GOLDEN_LOCK=1`. That red is the reviewed event the design exists to
force; the rails print the command once per run.

*The lock schema is mirrored, and the mirror is bound.* `golden_lock.rs` is a separate
test binary, so its `Anchor`/`ProducedBy`/row types cannot be imported. The harness
mirror carries `deny_unknown_fields` and is checked against the **real** committed
lock by `the_committed_lock_parses_into_this_mirror`: >700 rows parse, every one of
the 733 non-`self` rows is proven unwritable, the born-`self`
`json/schema_full_port.json` is writable from parity and refused from default. A
renamed anchor value or a new row field fails there instead of silently defaulting a
guard into "accept".

**Acceptance (all three guard outcomes proven over a scratch fixture).** Hermetic
tempdir roots + a mini lock: refusal for **every** external anchor (`capi_v0145`,
`r4133`, `r3723`, `capi015`, `fpc_3.2.2`), each leaving the seeded bytes unchanged;
refusal from the non-producing lane **and** an accepted write from the producing one,
in the same test, so neither arm can go untested in either gate run; `lane-invariant`
accepted from both lanes; `Unlocked` and `NoProducingLane` refused; the text/binary
split bound to the lock's own digest classification (a `.bin` path through
`snapshot_text` and a NUL payload in a text artifact both fail loudly); escaping path
shapes (`..`, absolute, backslashes, drive letters) rejected before any guard runs.
Live probe with the knob set: the rails arm, load the real 737-row lock, announce
`SNAPSHOT`/`REFUSED` lines, and the repo tree stays clean.

**`props_roundtrip.rs` population lock.** `PROPS_CLASS_FILES = 51`,
`PROPS_SCENARIOS = 322`, `PROPS_PROPERTY_CELLS = 8343`, asserted as equalities in
both lanes, plus a per-file "holds no scenarios" guard. G0.1 closed the *file* half
from the corpus side (a deleted `props/` artifact fails the lock as `STALE ROW`),
and its digests do see a scenario dropped **inside** a file — what neither sees is
the same drop made across a deliberate `DSS_UPDATE_GOLDEN_LOCK=1` regen (the digest
moves with the bytes), nor anything about the **driver**: what this file loads,
replays and actually compares is a separate question from what is committed. 50
files pass a non-emptiness check exactly as well as 51 do. The cell constant closes
the level below: `compared + lane_skips`, both counted at the comparison site, so a
code path that stops comparing cells moves a number here even with the corpus
untouched (`lane_skips` is itself tied to `LANE_SKIP_SCENARIO_PROPS.len()`, 0 in
parity). Probe: flipping the scenario constant to 321 fails with the intended
message; reverted.

**Docs.** TESTING.md gains "Golden provenance lock (`golden_lock.rs`) and the
self-golden write rails" beside the population-lock section (row schema, the four
lock assertions, the anchor histogram, `EXCLUDED_TREES`, the two write guards, and
the fact that no driver calls them yet), the R1–R4 regeneration rules as a new
Procedures entry ("Regenerate a self-golden"), and `DSS_UPDATE_GOLDENS` /
`DSS_UPDATE_GOLDEN_LOCK` in the environment-variable table. The existing "Regenerate
a golden" procedure now says what it covers — every externally anchored artifact
plus, for now, the two `self` `props/` rows `gen_props.py` still writes.
`tools/golden/README.md`'s regen paragraph is replaced by a pointer to those three
TESTING.md sections plus the per-file lock.

**Scope.** No driver calls the helpers (that is WP-G3), no golden byte moved, no new
dependency, no tolerance touched. Gate green in both lanes.

**Audit round (two fresh auditors, opus-high; fix agent, same tier).** Eleven
findings, one major, no duplicates lost:

- *The rails' announcements were invisible in their own documented command* (major,
  real). libtest captures `eprintln!` and discards it for a **passing** test — and a
  regen run passes by design. Every documented regen command now ends in
  `-- --nocapture` (TESTING.md mechanics, the `LOCK_REGEN_CMD` const the refusal
  messages print, the module doc, which now explains why), and `Outcome` is
  `#[must_use]`, so a WP-G3 driver asserts its family's expected refusals in code
  instead of trusting stderr. The same flag was missing from the *lock* regen
  command, whose `SEEDED`/`RE-ANCHORED` provenance announcements are the G0.1 review
  surface; it is there now.
- *`is_binary_artifact` was duplicated from `golden_lock.rs` with nothing binding
  the copies* (real). Both are now tied to the same third source of truth: new
  `the_binary_classifier_matches_gitattributes` pins the `.gitattributes` binary
  pathspecs inside the §1.2 lock scope (set equality, so a new binary family cannot
  appear unnoticed) and asserts the predicate against them, both directions, over
  every locked path. Negative probe: predicate flipped to `.PROBE` → red naming a
  real `reports/*.bin` row; reverted.
- *The documented API did not exist* (real): `harness/mod.rs` only declared
  `pub mod regen`, so the plan's, TESTING.md's and STATUS's `harness::regen()` /
  `harness::snapshot_text()` spelling would not resolve. Added
  `pub use regen::{regen, snapshot_bytes, snapshot_text}` (module in the type
  namespace, fn in the value namespace), with `allow(unused_imports)` for the same
  reason the module already allows `dead_code` — until WP-G3 wires the first driver,
  every test binary's subset of it is empty.
- *The armed public entry points were never exercised* (real): every guard test drove
  the inner `Rails` methods, so a delegation that skipped `write_text`'s binary-path
  assert would have passed. `snapshot_{text,bytes}` now delegate to a testable
  `snapshot_*_with(Option<&Rails>, …)`, and
  `the_armed_public_helpers_delegate_through_the_guards` drives an accepted write, a
  refused one, a raw stream and the binary-path rejection through the public shape.
- *TESTING.md claimed `self` artifacts "are regenerated by the rails"* (real, and it
  contradicted two other sentences in the file): no driver calls the rails yet, and
  all four `self` rows are written today by paths that bypass them — `gen_props.py`
  (`props/{recloser,relay}.json`), `DSS_REGEN_AD_GOLDEN` (`adiakoptics.rs:578`) and
  `REGEN_SCHEMA_PORT` (`golden_schema.rs:629`). The procedure now names all three,
  says WP-G3/G3.6 routes them through the rails, and `REGEN_SCHEMA_PORT` joins the
  env-var table beside `DSS_REGEN_AD_GOLDEN`.
- *The props lock's stated rationale was wrong* (real): the golden lock digests
  content, so a scenario dropped inside a class file **does** red it. The true hole
  is a deliberate lock regen (the digest moves with the bytes) and the driver side,
  which no digest describes; the comment, the assertion message and the STATUS
  paragraph above say that now. The auditors' companion suggestion landed too:
  `PROPS_PROPERTY_CELLS = 8343`, checked as `compared + lane_skips` counted at the
  comparison sites, closes the level below the scenario count.

**Deliberately not fixed (recorded, not dropped).**

- *A `FROZEN` register in `golden_lock.rs` (+ a `Refusal::Frozen` variant) so §1.2's
  frozen set cannot be de-anchored by a future `DEANCHORED` directory glob.* Not
  done here, for three reasons. §1.2 specifies this guard exactly as implemented
  ("refuse any artifact whose lock anchor ≠ `self`") and assigns frozen-set
  bookkeeping to **G3.6** ("This is the **initial** frozen set — G3.1/G3.2a may add
  rows, each with a reason, mirrored in G3.6"); every named frozen path is non-`self`
  today, hence already hard-refused; and the hypothesized de-anchoring is not silent
  — it takes an added register entry carrying a written reason, `golden_lock.rs` reds
  with `WRONG ANCHOR` until the lock is deliberately regenerated, and that regen
  announces `RE-ANCHORED <path>`. Carried to G3.6 as an explicit item.
- *Syncing `golden_lock.rs`'s four spellings of the lock-regen command with the new
  `-- --nocapture`.* One of them is `COMMENT`, which is baked into the committed
  `tests/golden/golden.lock.json`; this WP may not move a golden byte, and changing
  the const without rewriting the lock would put the two out of sync silently.
  TESTING.md and the rails' printed remedy carry the flag; the first sub-step allowed
  to move the lock (G3.6) syncs the four citations.
- *The over-long commit body of `1a841cb1`.* Rewriting landed history is out of
  scope and the content is accurate; the one-clause-per-bullet discipline applies
  from this commit on.

### GOLDEN_REBASE G0.1 — the golden corpus gets a provenance lock before it gets a regen button (branch `golden-g0`, 2026-08-02)

**Record.** `GOLDEN_REBASE_PLAN.md` WP-G0 (safety rails) opened here; G0.2 above
completes it. No golden byte moved.

**Why now.** WP-G3 will convert most golden families into self-snapshots and WP-G4
will regenerate them again when the FPC print kernels die. That hands the project a
capability it has never had — the ability to *rewrite* a golden — and with it the
failure mode "never regenerate" made impossible: a red gate silenced by
re-baselining the bytes that were supposed to catch it. The lock lands **before**
the first regen helper exists, so every future byte movement is a loud, reviewed
diff.

**What landed.** `tests/golden/golden.lock.json` — **737 rows**, one per committed
golden artifact: all 727 files under `tests/golden/**` plus the 10-file registered
out-of-tree witness `crates/dss-core/tests/data/adiakoptics/r3723_ref/` (§1.2 lock
scope). Each row is `{path, sha256, anchor, reason, produced_by}`. Anchor histogram
after the audit round's re-anchoring (below): **700 `capi_v0145`**, **11
`capi015`** (the enumerated dead-0.15.x-beta set — `props/{linemedium, autotrans_bh,
linespacing_eqspacing, regcontrol, swtcontrol, transformer_bh}.json`,
`ncim/{pq,pv_qlimit}.meta.json`, `line_constants/line_geometry_carson.json`,
`reports/{export_capacity_seasonal,export_overloads_seasonal}.meta.json`; each was
verified to declare `capi015` in its own provenance block — spelled `engine_spec` in
the seven `props/`+`line_constants/` dumps and `oracle` in the four `.meta.json`
sidecars, so a re-derivation from `engine_spec` alone would drop four), **11
`r4133`** (`flicker/`, `protection/`, `wasm_usermodels/`, `props/fuse.json` —
externally anchored, *not* frozen-`capi_v0145`, the §1.2 disposition re-confirmed in
G3.6), **10 `r3723`** (the A-Diakoptics witness), **1 `fpc_3.2.2`**
(`fmt_battery.csv`, retires with the print kernels at G4.6), **4 `self`**
(`adiakoptics/midi_torn_tree.txt`, `json/schema_full_port.json`,
`props/{relay,recloser}.json`).

New test `crates/dss-core/tests/golden_lock.rs`, modeled on `population_lock.rs`,
asserting fail-on-stale in both directions: (1) every artifact on disk has a row,
(2) every row has an artifact — which also closes the `props_roundtrip.rs` hole
(that test asserts only that the scenario list is non-empty, so deleting a `props/`
class file removed coverage silently), (3) every digest matches, (4) **every**
anchor is a *registered* decision — each row's `anchor` and `reason` must equal what
the in-test registers (`DEANCHORED`, `CAPI015_ARTIFACTS`, `R4133_FAMILIES`,
`FPC_ARTIFACT`, `R3723_TREE`, with `capi_v0145` as the residue) derive for its path,
every register entry must cover at least one locked row, and `anchor == self` holds
iff `produced_by` is set (the `ESCAPE_REGISTER` both-ways discipline, applied to all
five registers). Regen knob: `DSS_UPDATE_GOLDEN_LOCK=1`.

**Three design decisions worth carrying forward.**

*The digest is over committed content, not working-tree bytes — and that is also a
known blind spot.* `core.autocrlf` is
on here, so a text golden is CRLF on disk and LF in git (`di_em1_phv.txt`: 689 vs
685 bytes). Hashing raw bytes would make the lock valid only under one checkout
configuration. The lock therefore hashes CRLF→LF-normalized bytes for text
artifacts and raw bytes for `reports/*.bin` (declared `binary` in `.gitattributes`
precisely so git never EOL-munges them). This reproduces git's check-in filter:
verified over **all 737 artifacts** that the normalized length equals the blob size,
that no text-classified artifact contains a NUL byte (the classification would
otherwise be silently wrong — the test now hard-fails on one), and — independently,
through `git cat-file --batch` — that **every one of the 737 digests equals the
sha256 of the actual committed blob**, 0 mismatches. The classifier is now bound to
the attribute it emulates: the test parses `.gitattributes`, keeps the `binary`/`-text`
pathspecs inside the locked roots, and asserts `is_binary_artifact(p)` equals the
declaration for **every** scanned path, both directions (probe: a `.bin` dropped into
`tests/golden/json/` fails with "is_binary_artifact() says true, .gitattributes says
false"). *The blind spot:* because the digest is the blob, a change that flips only a
text artifact's line endings moves no digest — it also moves no committed byte, since
git's check-in filter normalizes it away. Recording the working tree's EOL shape
instead would make the lock red on any LF checkout, which is the exact failure the
normalization prevents, so the claim is narrowed rather than the mechanism changed.
**Carried to G4.3:** the `JSON_LINE_BREAK` teardown cannot use the lock diff as its
only review artifact (recorded in the module docs, assertion 3).

*`DEANCHORED` is per family, not per file.* A per-file self register would just be a
second copy of the lock. The first two entries are **born-`self`** (never
oracle-anchored, so nothing was de-anchored to create them): the emitted
`Torn_Circuit` tree is our own partitioner's output with no oracle counterpart, and
`schema_full_port.json` is the port's own `DSS_ExtractSchema` document (its external
half — `schema_full_oracle.json` + `schema_divergences.json` — stays `capi_v0145`
and frozen). Both are regenerated today by paths that bypass the rails
(`DSS_REGEN_AD_GOLDEN`, `REGEN_SCHEMA_PORT`); G3.6 routes them through
`harness::snapshot_*`, and the lock reasons say so. The audit round added the two
`props/` rows below, which were de-anchored by the WP-U2 control rewrites long before
this lock existed.

*`produced_by` is a restriction, never a claim.* `null` on every oracle-anchored row
(nothing in this repo produces those bytes); `parity` on every self row. Parity is
the §1.2 conservative default — until WP-G4 the lanes render different bytes, so the
lane holding the strictest byte contract is the producer. `lane-invariant` is
reserved for families a cross-lane regen has *measured*, which is G4.6's job; it is
never assumed here. The field was bookkeeping when G0.1 landed and became
load-bearing in G0.2, where `snapshot_*` refuses to write from the non-producing
lane.

**Regen cannot move provenance.** `DSS_UPDATE_GOLDEN_LOCK=1` recomputes digests and
re-derives `anchor`/`reason` from the registers — it never carries them over from the
stored row, so a hand-edited anchor is reset rather than made sticky, and every reset
is announced (`RE-ANCHORED <path>: <from> -> <to>`). A path no register recognizes is
announced too (`SEEDED <path> as CapiV0145 — confirm this artifact really came from
that source`) instead of silently acquiring the residue; only the *measured*
`produced_by` of a `self` row survives, and a stored `self` without a register entry
still **panics**. De-anchoring is therefore a reviewed Rust edit, not a side effect of
pressing the regen button.

**Negative probes (eight, each reverted; tree left byte-identical).** Corrupt one
golden byte → `DIGEST MOVED`. Delete `props/regcontrol.json` → `STALE ROW`. Add an
unlisted file under `tests/golden/` → `UNFINGERPRINTED`. Flip a row's anchor to
`self` → `UNREGISTERED SELF ANCHOR`. Null a self row's `produced_by` →
`PRODUCED_BY MISMATCH`. Point a `DEANCHORED` pattern at a nonexistent family →
`STALE DEANCHORED ENTRY` **and** `UNREGISTERED SELF ANCHOR` (both register
directions). Register the non-self `flicker/` family → `REGISTERED FAMILY, NON-SELF
ANCHOR`. Run the regen knob against a hand-flipped `self` anchor → the de-anchoring
panic. A ninth check was added *because* of a probe: the probe's throwaway extra JSON
key was silently ignored, so `Artifact`/`GoldenLock` now carry
`deny_unknown_fields` — a mistyped lock key is a lost assertion, and it fails loudly
(`unknown field sha_256`).

**Scope.** `sha2 = "0.10"` added as a `dss-core` dev-dependency; it was already in
`Cargo.lock` via `dss-usermodel`'s fixture-hash scaffold, so the dependency closure
does not grow. TESTING.md was deliberately untouched in this sub-step — the "Golden
provenance lock" section and the R1–R4 regen rules belong to G0.2 (landed above),
and the lock's own module documentation was the authority until then. The
`comment` field inside `golden.lock.json` still carries that "until then" wording;
it is documentation, never compared, and rewriting it would move a byte under
`tests/golden` — G3.6 refreshes it in a commit that legitimately regenerates the
lock.

**Audit round (two fresh auditors, 12 findings; all settled, no golden byte moved).**
The round found one class of real defect and one class of missing enforcement, and
they turned out to be the same defect: *the initial anchors were assigned from the
plan's directory enumeration, not from each artifact's own provenance block, and
nothing in the test compared the two.*

*Three `props/` rows were anchored to a witness that provably cannot exist.*
`props/fuse.json` declares `engine_spec: "r4133"` and is DERIVED — the retired 0.14.5
dump supplies the rendering the port reproduces bit-for-bit, and every value delta
overlaid on it was verified on the EPRI r4133 engine, because *no* capi-line engine
has the r4133 fuse surface (`gen_fuse_r4133.py:11`). It is now `r4133`.
`props/relay.json` and `props/recloser.json` declare an r4133-Oddie capture whose own
notes say the committed values are **the port's own renders** — relay.json calls
itself "a REGRESSION PIN (self-consistency), not an independent oracle gate" — so
they are now `self` + `produced_by: parity` with `DEANCHORED` entries recording the
manual (non-CI) oddie:r4133 cross-validation a regen must repeat. Genuine r4133
coverage for both classes lives in the live controls-family decks. A converse sweep
over every JSON artifact (classify the declared engine strings, compare with the
locked anchor) found these three and no others; the two non-JSON-declaring families
(`protection/`, `wasm_usermodels/`) were checked against their generators.

*The registers are now the invariant, not a seeding heuristic.* Previously only the
`self` anchor was guarded both ways; `CAPI015_ARTIFACTS`/`R4133_FAMILIES`/
`FPC_ARTIFACT`/`R3723_TREE` were read solely when seeding a path the lock had never
seen, which is exactly why the three wrong anchors passed green. Now every row's
`anchor` and `reason` must equal `seed_metadata(path)` (diff line `WRONG ANCHOR`),
every register entry must cover a locked row (`STALE …  ENTRY`), and the regen arm
re-derives instead of preserving. Probed live: the pre-fix lock failed with exactly
the four predicted lines and nothing else; pointing `FPC_ARTIFACT` at a nonexistent
file produced both `STALE FPC_ARTIFACT` and `WRONG ANCHOR` (both directions).
`CAPI015_ARTIFACTS.len() == 11` is kept — it encodes the plan's "never grows" — but is
no longer the only guard: a deletion is now caught corpus-side.

*Three documentation claims were corrected to what was actually measured.* The
capi015 set declares its provenance as `engine_spec` in seven files and `oracle` in
the four `.meta.json` sidecars, not `engine_spec` in all eleven (the old wording would
have led a future reader to re-derive a 7-element set). The `flicker/` reason claimed
an r4133 capture; the committed bytes are the frozen **r3723** Oddie capture that the
r4133 regen reproduces byte-identically (`gen_flicker.py:14-27`) — the `r4133` anchor
stands as the only surviving regeneration path, and the reason now says so. It also
claimed "the only Pst oracle gate"; §1.2 gives that role to `pstcalc/`, so it is now
"the monitor mode-4 Pst gate".

*Two scope facts are now recorded rather than implied.* `crates/dss-metis/tests/golden`
(31 tracked METIS 5.2.1 fixtures, manually regenerated per `gen_metis_reference.md`)
is a second out-of-tree golden tree that §1.2's enumeration leaves out; rather than
invent a seventh anchor value the plan does not define, it is a **named exclusion**
(`EXCLUDED_TREES`, fail-on-stale: the tree must exist and stay outside `ROOTS`) for
G3.6 to revisit, and the lock's COMMENT no longer says "every committed golden
artifact". `is_binary_artifact` is bound to `.gitattributes` (above).

**Proof.** All five gate commands green in order (re-run after the audit round);
`cargo test --workspace` and its parity twin both 0 failed, corpus gate green inside
each. `git diff --stat -- tests/golden` **empty** — the only addition under
`tests/golden/` is the new lock file itself; no existing golden byte moved. The
audit-round regen moved **0 digests**: the whole diff is four rows' `anchor`/`reason`
cells (`props/{fuse,relay,recloser}.json` + the `flicker/` reason), verified row by
row against the pre-fix lock.
