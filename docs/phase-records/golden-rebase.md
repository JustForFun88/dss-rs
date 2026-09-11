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

## Condensed records (moved from STATUS.md, 2026-09-03)

> Moved verbatim from `STATUS.md` on 2026-09-03 (STATUS.md archiving round 2);
> order preserved, nothing rewritten. These are STATUS §1's `### GOLDEN_REBASE`
> condensed record blocks (WP-G0 / WP-G2, then WP-G1); the full session records
> of the same work are above in this file.

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
  (`crates/dss-epri/src/dss.rs:1424-1429`, "exactly like dss-python"). That bridge
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
> that do not depend on it landed on `r4133-props` and, after its close-out, on
> `update`. Since decision **D7** (2026-09-04) the independent chains run in
> per-lane worktrees (`.claude/worktrees/lane-*`) and are merged into `update`
> one sub-step at a time; the merge agent regenerates `population.lock.json` on
> the merged tree and takes the union of both sides' `ledger.json` entries, so
> the locks stay fail-on-stale.

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


- **G1.0** (2026-09-04, branch `update`) — **WP-G1's rails, landed before its first surface** (new
  sub-step; coordinator decisions **D1**/**D2**/**D3**, written into the plan's WP-G1 preamble).
  *Half A*: the ten compare-depth manifest flags declared once, in **one** `population.lock.json`
  regen; explicit `channels` on the ten bare `element` ledger exclusions (8 cases);
  `harness::capture_guard` fails a flag-on/capture-empty case instead of comparing 0 == 0.
  *Half B* (`crates/dss-epri/src/modes.rs`): typed per-family unknown-mode sentinels, the 96-row
  `WP_G1_MODES` table, the two-double `F` ABI fix (`DCircuit.pas:27`, `DCmathLib.pas:5`), two
  memory-unsafe arms refused pre-FFI (`DSolution.pas:580-582`, `DBus.pas:803-838`) — **G1.11′
  discharged for the whole WP** (96/96 `Served`). **0** ledger entries (57 / 30 causes unchanged),
  0 golden bytes, no floor, no `lane_diff` owed; mechanics, citations and the pin list in TESTING.md
  §"The unified corpus gate" and §"The r4133 bridge — entry points, mode capability, do-not-call".
  **Commits** `c4b67a6e`, audit settlement `42454b64` + `14bb0f23`, + docs. **Gate**, both lanes:
  fmt + clippy clean, **4 605 / 0 / 5** (4 601 at `c4b67a6e`; 4 499 before G1.0), corpus gate
  523/523, ledger 57 entries / 1 588 hits / 0 stale, goldens untouched, `lane_diff` max |Δ| = 0.

  **Audit settlement** (2026-09-04) — 16 findings: **15 fixed / 1 recorded / 0 refuted**, plus one
  sub-claim refuted by measurement. Major: `ModeEffect::Pure` on the five `GetCurrents` rows
  contradicted D3 — the register now carries the partition (`ReadsIterminalCache` /
  `PoisonsIterminalCache` / `Impure` over 14 rows, including the `Circuit.Losses` walk at
  `Common/Circuit.pas:2436-2443` that both audits missed), pinned by
  `the_capture_order_partition_is_the_one_d3_names`; its *impact* claim is refuted —
  `capture::capture_all_elements` reads Powers-then-Currents and no group-B mode at all, and the
  poisoning measures latent on IEEE13 in snapshot **and** harmonics, so no gated number was ever
  stale. Also fixed: the do-not-call register bites at the chokepoint `Engine::ffi_dispatch` (so the
  worker's raw `ffi` command is refused too), `read_mode` classifies the tag-4 `V` sentinel (`I`/`F`
  cannot be — the sentinel is legal data, recorded in its doc), two `S_SENTINELS` doc errors and the
  `Ucomplex.pas:118-121` off-by-one, `SolvableCase` gains `deny_unknown_fields`, the rigor scanner
  strips **any** visibility and the two flag tables must partition the vocabulary, a pin registry
  (`every_pin_the_g10_record_names_exists_and_is_cited`) refuses a renamed pin the prose still
  claims, and identity pins per family close the "96/96 `Served` proves capability, not
  correctness" gap. The one recorded-not-fixed finding (record length) is carried:
  the record above is trimmed 34 → 22 → 14 lines, still over CLAUDE.md's 5–10 — recorded, not
  hidden.
  *Amended by G1.6b (2026-09-04):* the r4133 mode table grew with its first surface —
  `WP_G1_MODES` **96 → 99** rows and `EXCLUDED_WRITE_MODES` **2 → 3**; the mode-capability
  acceptance re-runs over all 99 and still reports zero misses.

- **G1.9** (2026-09-04, lane `lane-s`, decisions **D3**/**D4**/**D7**) — the five `Circuit`
  aggregates (`DDLL/DCircuit.pas:294`…`:458`; only `Circuit.Losses` is W/var,
  `Common/Circuit.pas:2428-2445`, and AutoTrans is a separate list, `:2272-2273`) and the ten
  `Solution` scalars go live on **both** channels in one commit, unflagged and universal on all
  519 live cases — new `exec/view.rs` accessors, three `harness/aggregates.rs` arms, **no flag,
  no rigor token, no lock regen, no new floor, 0 ledger entries, 0 new `LEDGER_FIELDS`, 0 golden
  bytes**: the value arms inherit `LedgerView::element_rewrites` instead of re-pinning a scoped
  element's echo (~14 rows = the kill criterion). All three kill criteria **NOT met** (feeder
  `max |Δ|/|Losses|` `2.719409449622587e-08` vs 1e-4; 0 `ControlIterations` differences in 3 493
  checkpoints; 0 entries). Derivations, the ledger-inheritance rule and the `Totaliterations` ≡
  `Iteration` / `YCurrents` ≡ `injection` equivalences: `tests/TOLERANCE_NOTES.md` §G1.9,
  TESTING.md and the plan's dated §G1.9 note (the five source settlements). Pins in `G1_9_PINS`:
  `circuit_losses_are_watts_not_kilowatts`, `substation_losses_exclude_autotrans`,
  `losses_skip_shunt_elements`, `line_losses_sum_the_lines_list`,
  `total_power_is_terminal_one_of_every_source`, `total_iterations_is_an_alias_of_iterations`,
  `all_element_losses_follow_creation_order`, `r4133_solution_flags_are_zero_one_ints`,
  `the_five_circuit_aggregate_rows_are_impure`, the three `capture_order.rs` cases.
  Commits: `9757d26c` (surface) + `f27f9598` (settlement) + `44294de7` and this record (docs).
  Gate at `9757d26c`: fmt/clippy clean, **4 678 / 0 / 5 ignored** per lane, 523 manifest cases
  (519 compared) on both channels, 57 ledger entries / 0 stale, `population_lock` green without
  a regen, `lane_diff` **PASS**, max |Δ| = 0.

  *Audit settlement (2026-09-04, `f27f9598`)* — 13 findings, **6 fixed / 5 recorded / 2
  refuted**; no port bug, no ledger entry, no floor moved, no golden byte. **Fixed:**
  `TotalPower`'s value arm no longer drops when a source merely appears in the rewrite map (a
  `currents`-only scope killed a `powers` comparison nothing had excluded) but absorbs that
  source's accepted `powers` divergence — `a_currents_only_scope_leaves_the_total_power_arm_running`,
  proved load-bearing against the old condition; `complex_pair` refuses a `myType=3` reply that
  is not two doubles instead of padding a plausible `(0, 0)` (`DDLL/DCircuit.pas:293-303` sets
  length 1 unconditionally); the two boolean scalars, constant corpus-wide, gain an in-engine
  two-sided witness and TOLERANCE_NOTES loses its "nothing is vacuous" over-claim; the 14 (case,
  channel) pairs whose value arms inherit the element ledger whole are asserted exactly (D11(2)
  visibility); the pin names are machine-checked both ways; both doc placeholders filled — the
  four settlement pins are named in TESTING.md and `tests/TOLERANCE_NOTES.md`. **Recorded:**
  that self-comparison is *inherent* (restating the arm against the oracle's own aggregate is a
  triangle tautology — drafted, refuted by a 200 000-draw search, reverted), so the remedy is
  visibility; `control_iterations` keeps `rust <= oracle` on r4133 (the `iteration-count-delta`
  cause owns that class) though a scratch exact-assert run over the whole corpus measured **0**
  differences; `load_mult` stays exact pending D11's `float_roundtrip` sync; the third
  `USER_MODEL_ERRNOS` copy waits for merge dedup. **Refuted:** the capture reorder's priming
  equivalence *is* gated live (arm P1 rebuilds the oracle's aggregate from its own per-element
  losses on every `warn_and_continue` deck); the untracked droppings were already gone. Gate
  after the settlement, both lanes: fmt + clippy clean, **4 726 / 0 failed / 5 ignored** per lane
  (+48 on 4 678 — the two new `harness::aggregates` cases compile into all 22 binaries carrying
  `mod harness`), corpus gate green on both channels, ledger 57 / 0 stale, no lock or golden
  byte moved; `lane_diff` **not owed** (nothing outside `#[cfg(test)]` and the
  `publish = false` bridge moved).

  merge: lane lane-s -> update, see git log

- **G1.6b** (2026-09-04, lane `lane-m`; **D7** lanes, **D9** the engine fix) — **WP-G1's first surface:
  the `PDElements` walk, live-gated on both channels** — all **13** `IPDElements._columns` plus
  `parent_name`, compared **exactly**, `ParentPDElement` read **last** per element because it hijacks
  `ActiveCktElement` (r4133 `DDLL/DPDELements.pas:88-97`, capi `CAPI/CAPI_PDElements.pas:245-257`).
  **0 ledger entries**: the one divergence is an uninitialized
  read in *both* oracles on in-zone shunt Capacitors/Reactors (r4133 `Meters/EnergyMeter.pas:1868-1869`,
  capi `:1927-1929`; report
  `investigations/to_opendss/56-energymeter-zone-corrupts-shunt-pd-reliability-fields.md`), excluded —
  never enveloped — in `harness::PD_SKIP_FIELDS` and pinned by
  `pd_elements_shunt_reliability_inputs_survive_the_meter_zone` +
  `pd_elements_shunt_branch_flt_rate_survives_the_meter_zone`. **D9**, its own commit ahead of the
  surface: `do_reset_meter_zones` returns to `reprocess_bus_defs`' tail (r4133 `Common/Circuit.pas:2411`,
  capi `:2246`) — `MakeBusList` was bypassing it and left every EnergyMeter an empty zone; pin
  `makebuslist_keeps_the_meter_zones`, one corpus deck changes state. The four zone-derived columns
  compare 0 on every live case (no deck ran `RelCalc`): non-vacuity **owed by G1.6(i)** and
  **discharged there on 2026-09-05** (see its record below). Detail — the
  column list, the exactness derivation, `WP_G1_MODES` 96 → 99, the two deviations from the plan's
  letter: `GOLDEN_REBASE_PLAN.md` §G1.6b as-executed, `TESTING.md` §"The `PDElements` walk".
  Commits: `06808a6d` (D9), `e1e18367` (surface), `c6a3c0a8` (audit settlement) + docs. Gate: five
  commands exit 0 in both lanes, **4 792 passed / 0 failed / 5 ignored**; corpus gate 523/523, ledger
  57 / 1 588 hits / 0 stale, no golden byte and no lock content moved; `lane_diff` `VERDICT: PASS`,
  max |Δ| = 0 on all eight kinds.
  *Audit settlement* (`c6a3c0a8`): 15 rows / 12 distinct findings — **9 fixed / 2 recorded / 1
  refuted**. `PD_SKIP_FIELDS` is now scoped to the element the defect reaches, an in-zone shunt one
  (`harness::pd_skip_applies` over the port's `PdElementView::in_meter_zone`, pin
  `pd_elements_in_meter_zone_is_the_zone_membership_the_skip_rows_need`): ~350 clean cases per channel
  return to the compare and visits == hits on all eight rows. Every row's `pin` must now resolve to a
  `#[test]` (`every_pd_skip_row_pin_is_a_test_that_exists`, drive-proven); the cited `to_opendss`
  report was written; three stale doc claims re-pointed. **Recorded:** `06808a6d` does not build alone
  (its pin calls `pd_elements()`, landed in `e1e18367`) — **the merge agent squashes the pair or merges
  `--no-ff`**; and `assert_pd_skip_rows_are_live`'s `hits == 0` arm stays heap-dependent by construction
  (it fails safe — spurious red, never false green — and its doc comment says so). **Refuted:** the
  `Show Isolated` reset is Pascal-faithful at both revs (capi `ShowResults.pas:2859-2860`, r4133
  `:2537-2538` calls it twice) and runs under the `show_isolated` golden; the corpus deck said to issue
  it has the line commented out. Measured on the way, out of scope: the long-standing `CorpusGuard`
  leak is a **drop-order race**, not a missing sweep — mechanism, negative controls and why it is not
  fixed here are in STATUS's standing follow-up.

  merge: lane lane-m -> update, see git log

- **Bridge D13** (2026-09-05, lane `lane-b`; coordinator decisions **D13**/**D14**) — the r4133 worker
  issues `Set RegistryUpdate=No` once at init, `Set DefaultBaseFrequency=60` right after it and again
  after every `clear`; `tools/oracle/oracle_server.py` mirrors that reset on the capi channel.
  `HKCU\Software\OpenDSS` is a machine-wide, cross-worktree channel — `TExecutive.Create` reads
  `DefaultBaseFreq` from it (r4133 `Common/DSSGlobals.pas:692-720`, `:718`), `Destroy`/finalization
  writes it back while `UpdateRegistry` holds (`:726-738`, `:951`; `Executive.pas:138`) and `clear`
  never resets it (`:855` runs only at DLL load), so a 50 Hz deck leaked its base frequency into every
  later worker. Pins (`crates/dss-epri/tests/protocol.rs`, self-restoring):
  `init_resets_the_default_base_frequency_to_sixty`, `clear_resets_the_default_base_frequency_to_sixty`,
  `the_worker_never_writes_the_opendss_registry_key`. 0 ledger entries, no floor, no golden byte; the
  rest of the D13/D14 prose (TESTING.md's one-gate-per-worktree rule, the G1.4a record) arrives with the
  G1.4a merge. Commit: `6b0dbd32`. Gate on the merged tree: five commands exit 0 in both lanes,
  **4 924 passed / 0 failed / 5 ignored**, corpus gate 523/523 on both channels, ledger 57 / 1 588 hits
  / 0 stale, `population.lock.json` regenerated byte-identical, `golden.lock.json` unmoved; `lane_diff`
  `VERDICT: PASS`, max |Δ| = 0.

  merge: lane lane-b -> update, see git log

- **G1.3a** (2026-09-04, lane `lane-e`; coordinator decisions D3/D4/D7/D10) — **WP-G1's first
  surface.** Per-element `Enabled` + `CurrentsMagAng`/`VoltagesMagAng`/`Residuals` compare live on
  both oracle channels over **442** cases, additive in `exec/view.rs::snapshot_elements` (r4133
  `DDLL/DCktElement.pas:1058`/`:1082`/`:827`) and captured for **enabled elements only** — `:1099`
  derefs a nil `NodeRef` and kills the worker — hence the one new mode `CktElement.Enabled`
  (`WP_G1_MODES` 96 → **97** on the lane; **100** on `update`, G1.6b's three
  PDElements walk arms merged in). Floors are derived images of the disc `assert_complex_close_c` already
  admits (`tests/TOLERANCE_NOTES.md` §G1.3a: no band moved, D10's √2 refuted), D3's capture order is
  enforced by `tests/capture_order.rs`, and nothing joins `LANE_SKIP_ELEM_POWERS`. Ledger 57 → **58**:
  `capi-capcontrol-time-bus-is-the-capacitors` (r4133 `Controls/CapControl.pas:605`/`:622` vs capi
  0.14.5 `:597-608`, `docs/upgrade/DIVERGENCES.md` L8) pinned by
  `capcontrol_time_voltages_follow_the_monitored_elements_terminal`, plus **13** measured widenings of
  committed `element` scopes; pins `exec::tests::derived_polar::*` (6),
  `harness::derived_polar_floors::*` (15), two `ledger::*`, one `scheduler::*`. As executed:
  `GOLDEN_REBASE_PLAN.md` §G1.3a. Commits: `d8e71991`, audit settlement `588e0bfe`, + docs (this
  record). Gate after the settlement, both lanes: **4 959 / 0 / 5**, fmt + clippy clean, corpus gate
  523/523, `lane_diff` PASS max |Δ| = 0 on every gated kind (4 956 / 0 / 5 at `d8e71991`).

  **Audit settlement** (2026-09-04) — 15 findings: **11 fixed / 3 recorded / 1 refuted**. Major 1: the
  new `voltages_mag_ang` block sliced `node_ref[..yorder]`, and a **disabled** element that grows
  phases keeps a shorter `node_ref` (`elements/ckt.rs:326`/`:382`, `circuit/circuit.rs:735`) — a
  reachable panic in the public `snapshot_elements`, reproduced and now pinned by
  `a_stale_node_ref_shorter_than_yorder_reads_as_ground` (a stale slot reads ground). Major 2: the
  three new `ElemChannels` bools were only ever compared against the constant they came from, so one
  `false` would silently drop a channel on all 442 cases in both lanes — the lane test now asserts
  them field by field, `ElemChannels::ALL` included. Major 3: the sub-step's 25 pins are registered in
  `every_pin_the_g13a_record_names_exists_and_is_cited` (G1.0's rule), which also machine-checks the
  two group counts above. Also fixed: per-sub-channel ledger liveness (`Scope::channels_exceeded`,
  `a_widened_sub_channel_that_masks_nothing_is_reported_stale`) — its first run found four dead masks
  and pruned `powers` from the four `*-injection-ulp` entries (every p_kW/p_kvar sample inside the
  tier floor; the lock digest moved on those four cases only); the residual loop reads
  `chunks(nconds)` instead of a flat offset; `EXCLUDED_WRITE_MODES` gains the `CktElementI(13)` write
  arm; the ledger's polar loops zip the port vector and assert the residual shape; three file:LINE
  citations re-pointed; the record's two placeholders filled and the block trimmed 26 → 14 lines.
  **Recorded, not fixed:** an `exclusion` entry's sub-channels stay unpoliced — it fetches no verdict
  (`LedgerView::excluded`), so their liveness rests on each entry's `measured` provenance; the
  conductor-sum residual band masks about half the `Residuals` samples (derived and disclosed,
  TOLERANCE_NOTES §G1.3a derivation 4 — carry into G1.3b/c); a qualified-form (`* cd.nconds`) needle
  for the de-Pascalization metric is not added, 7 of the 8 such sites being `elements/ckt.rs`'s own
  accessors, the module the convention names; and this settlement is itself over the 5–10-line rule.
  **Refuted:** the two `gic-*-capi` polar widenings were *measured*, not copied from the r4133 row — a
  scoped negative drive prints the capi oracle's own `27.20469355379569` (a live dss-python probe
  reads that value up to the 1-ULP serde_json transport defect D11 pins), and the two channels agree
  bit-for-bit on that deck. They are load-bearing today (dropping them reds the case), so they stay
  and the **merge agent deletes them together with their entries** when lane-b lands D12/D14 ("D12 —
  pending sync"); `makeposseq-cuf-applied-capi` is NOT one of them (D14 keeps that deck capi-gated),
  but its polar samples must be **re-measured at the merge**, once the GICTransformer line moves out.
  Gate re-run in full and green in both lanes (figures above): ledger 58 entries / 0 stale,
  `tests/golden` byte-untouched, no tolerance moved.

  merge: lane lane-e -> update, see git log

- **G1.4a** (2026-09-04, lane `lane-b`, bus chain) — **the bus voltage surface, its divergence-free
  half** (coordinator decisions **D7** lanes, **D8** re-scope, **D11(1)**+**(2)**, **D12**/**D14**,
  **D13**). Live on both channels for every live non-`large*` case: per bus in `BusList` order
  `Nodes`/`kVBase`/`puVoltages`/`VMagAngle`/`puVmagAngle` plus checkpoint-level `AllBusVmagPu` — the
  two arms run identical algorithms (`CAPI_Alt.pas:2143`/`:2251`/`:2573`/`:2540` == r4133
  `DDLL/DBus.pas:319`/`:399`/`:659`/`:690`; `CAPI_Circuit.pas:521` == `DCircuit.pas:481`). Port side:
  a pure read over `Solution.NodeV` (`exec/view.rs::BusVoltageView`); comparator: the shared per-bus
  plumbing G1.5/G1.6(ii) inherit, bands exact images of the node-voltage band — **no** new constant
  (`tests/TOLERANCE_NOTES.md` §"Bus voltage surface").
  **D8:** `SeqVoltages`/`CplxSeqVoltages` and `VLL`/`puVLL` (an r4133 hang, `DDLL/DBus.pas:549-602`)
  moved to a new **G1.4c**; order G1.4a → G1.5 → G1.4c → G1.4b. **D11(1):** the gate's first exact
  float compare (`kv_base`) proved serde_json's default parser 1 ULP low on three oracle floats →
  workspace `float_roundtrip`, a strict strengthening with no band. **D11(2):** a **deck-wide**
  `voltages` exclusion suppresses the three continuous per-bus arrays and nothing else, printed beside
  the causing entry (8 (case, channel) pairs). **D12/D14** (`DIVERGENCES.md` §D12/D14): capi 0.14.5
  disagrees with itself across processes on every `GICTransformer` deck (7/60 vs 0/40; r4133 80/80),
  so those four decks gate on `r4133` alone and `makeposseq_shunt`'s GICTransformer split into the new
  `makeposseq_gic.dss`. **D13** (`TESTING.md` §"One gate or probe per worktree at a time"): the bridge
  no longer leaks `DefaultBaseFreq` through `HKCU\Software\OpenDSS`, in its own commit.
  Measured: **0** new ledger entries from the surface, **0** golden bytes, `golden.lock.json` unmoved;
  ledger 57 → **53**, corpus 523 → **524** / 520 live; the force rule's own guard is
  `FORCED_BUS_POPULATION = (441, 310, 87, 44)`; `WP_G1_MODES` **98** on this lane.
  Pins: `bus_pu_voltages_come_out_in_ascending_node_number_order`,
  `bus_pu_voltages_use_a_unit_base_when_kv_base_is_not_set`,
  `all_bus_vmag_pu_walks_buses_times_internal_node_index`, `the_voltage_exclusion_still_pins_kv_base`,
  `a_suppressed_bus_array_is_named_with_the_entry_that_caused_it`,
  `the_bus_capture_reads_in_one_fixed_order_on_both_transports`,
  `the_bus_forcing_rule_is_every_live_non_large_case`,
  `no_capi_gated_case_instantiates_a_gictransformer`.
  Commits: `6b0dbd32` (D13 bridge) + `be01e413` (the surface) + `10417d99` (audit settlement)
  + docs. Gate at `be01e413`: fmt + clippy clean in both lanes; `cargo test --workspace`
  **4 722 / 0 / 5 ignored** in both lanes, corpus gate 524/524, ledger 53 entries / 1 516 hits /
  0 stale, `tests/golden` untouched; `lane_diff` **PASS**, max |Δ| = 0 on all eight kinds.
  Final gate after the settlement: **4 835 / 0 / 5 ignored** per lane (below).

  *Audit settlement (2026-09-05, this lane, `10417d99`):* **15** findings over the two reports
  (11 distinct) — **9 fixed, 1 fixed in part, 1 recorded, 0 refuted**. Fixed: the D11(2)
  predicate now honours only a **deck-wide**
  `voltages` exclusion (`LedgerView::bus_arrays_suppressed`, no `name_re`/`node_re`, driven both
  ways in `a_suppressed_bus_array_is_named_with_the_entry_that_caused_it`); D2's cross-transport
  check landed live (`the_two_transports_agree_on_the_bus_capture_of_a_gated_both_case` on
  `asymmetric:line/line_asym.dss`, worst |capi − r4133| = 1.5e-11 V at twice the node band);
  `compare_all_bus_vmag_pu` and both angle channels gained negative drives (plus a full-turn
  positive control) and its port-internal length identity now runs on suppressed cases too;
  `epri_worker_bin` rebuilds a bridge older than `crates/dss-epri` (pinned by
  `the_epri_worker_binary_is_not_older_than_its_bridge_sources`); the D13 registry restore deletes
  a value the machine did not have; and `makeposseq-cuf-applied-capi`'s deck-wide blanket was
  re-measured on the 13-element deck (13/13 elements, 3/3 nodes above floor). Recorded: the D14
  two-commit split (landed as one commit; its `population.lock.json` justification corrected — the
  lock moves for D12/D14 alone). No product code, tolerance, golden byte or ledger row moved
  (`lane_diff` not owed); per-finding disposition in `10417d99`'s own diff.
  Lock: the ledger `source` re-measurement moved ONE `population.lock.json` line (the entry's
  content hash), regenerated by the documented procedure. Gate (2026-09-05, this worktree quiet):
  fmt + clippy clean in both lanes, `cargo test --workspace` **4 835 / 0 failed / 5 ignored** in
  both lanes (75 binaries each), corpus gate 524/524, ledger 53 entries / 1 516 hits / 0 stale, the
  D11(2) report still 8 (case, channel) pairs, `tests/golden` untouched.

  *On `update` after the merge (2026-09-05):* the lane's figures are its own; merged with
  G1.9/G1.6b/G1.3a the tree reads ledger **54** entries / 31 causes (58 − the four D12/D14
  `gic-pct-r2-honoured-*-capi{,-props}` deletions), `WP_G1_MODES` **102** (98 on the lane plus
  G1.9/G1.6b/G1.3a's four rows), `FORCED_BUS_POPULATION` unchanged at (441, 310, 87, 44) while
  `FORCED_PROPS_POPULATION`/`FORCED_PDELEMENTS_POPULATION` moved (440, 313, 83, 44) →
  (441, 310, 87, 44) and `FORCED_DERIVED_POPULATION` (442, 315, 83, 44) → (443, 312, 87, 44),
  and `AGGREGATE_VALUE_ARMS_INHERITING_THE_ELEMENT_LEDGER` **14 → 12** — the two `capi_v0145`
  GIC rows went with their entries, a SHRINK of the inheritance (those decks' capi aggregate
  value arms compare against the oracle again). `makeposseq-cuf-applied-capi` keeps BOTH sides'
  work — G1.3a's polar sub-channel widening and G1.4a's 13-element re-measurement — with the
  three polar first-failure samples **re-measured on the merged tree** (the deck's solve moved
  when the GICTransformer left it); all three still exceed their floor. Lane-b's private
  `wrap_deg` folded into lane-e's canonical `wrapped_deg` (D10); the exact `asin`-image
  `bus_angle_band_deg` survives and both sides' tests were kept. Merged-tree checks: fmt +
  clippy clean in both lanes, corpus gate **524/524** in both (`corpus_gate` binary 198 / 0 / 0
  each), ledger 54 entries / **1 564** hits / 0 stale, D11(2) still 8 pairs, r4133 props 1 671
  walks / 151 786 elements, `golden.lock.json` unmoved; the full five-command gate reads
  **5 509 / 0 failed / 5 ignored** per lane and `lane_diff` **PASS** with max |Δ| = 0 on all
  eight kinds (524 cases, 3 220 881 records).

  merge: lane lane-b -> update, see git log
- **G1.3d(i)** (2026-09-04/05, lane `lane-e`; D4/D7/D19/D19′) — per-element `NumTerminals`/
  `NumConductors`/`NumPhases`, `NodeOrder` and `EnergyMeter` on **both** channels, compared exactly
  (r4133 `DDLL/DCktElement.pas:139`/`:144`/`:149`/`:442`/`:1032-1056`; capi
  `CAPI/CAPI_CktElement.pas:182-211`/`:672-687`/`:885-917`). `Enabled` is re-emitted and re-asserted
  because the `NodeOrder` capture predicate (enabled + terminals; `:1048` nil-derefs, capi raises
  15013) rests on it. **0 ledger entries, 0 widenings, 0 golden bytes, no floor**; one D4
  normalization — the no-meter sentinel, folded per channel. Forced on **441** cases
  (`FORCED_ELEMENT_EXTRAS_POPULATION` `(441, 310, 87, 44)` on `update`; `(440, 313, 83, 44)`
  on the lane, re-derived at the merge after G1.4a's D12/D14 corpus flips). Pins: `exec::tests::element_extras::*`
  (8), `harness::element_extras_pins::*` (19, 12 `should_panic` legs),
  `corpus_manifest::extras_population::*` (3), plus
  `scheduler::the_element_extras_forcing_rule_is_every_live_non_large_case` and
  `capture_order::a_group_c_read_may_sit_between_a_group_a_and_a_group_b_read` — 32 in all, held
  against this prose by `oracle_parity_cfg_gate::every_pin_the_g13d1_record_names_exists_and_is_cited`.
  Both G1.3d verdicts (`Lines.Yprim` already witnessed, residual 235/523 `selected_elements`;
  `LineGeometries.R/X/Zmatrix` dropped) and the two settled STOPs (D19/D19′ D9 cherry-pick; the
  census↔live-gate file race) are recorded in TESTING.md and plan §G1.3d. Commits: `e4d99806`
  (D19′ cherry-pick), `b7d7da2a`, audit settlement `c9c4ac09`, + docs (this record). Gate after the
  settlement, both lanes: **5 392 / 0 / 5**, fmt + clippy clean, `corpus_gate` 523/523, ledger 58
  entries / 0 stale, goldens + `population.lock.json` byte-untouched, `lane_diff` PASS max |Δ| = 0
  on all eight gated kinds over 523 cases / 3 220 861 records (5 370 / 0 / 5 at `b7d7da2a`; the
  +22 are one new pin seen from the 22 harness-linking test binaries).
- **G1.3d(i) audit settlement** (2026-09-05, `c9c4ac09`) — 15 findings: **12 fixed / 2 recorded /
  1 refuted**. Fixed: 15 capi `NodeOrder` citations
  re-pointed from the Alt-API twin to `CAPI/CAPI_CktElement.pas:885-917` (`:900-906` = the 15013
  guard), the entry point dss-python really calls; `oracle_meter_name` folds only its own channel's
  sentinel; the D19′ pin asserts the parent *identity* `Line.l1`; the forcing-rule test asserts its
  family-arm premise; four prose defects, this record's length among them. Recorded: `NodeOrder` is
  never oracle-compared on an element disabled *after* a solve — both oracles would answer (r4133
  `Common/CktElement.pas:438-465` keeps `NodeRef`) but no transport says whether it was allocated,
  so the port side stays pinned in-engine (TESTING.md names the residual). Refuted: 18 forced
  single-channel cases DO define an EnergyMeter (live capi probe: `controls/combo/combo_metering.dss`
  → `Transformer.tr` = `em`).

  *On `update` after the merge (2026-09-05):* the lane's figures are its own. On the merged
  tree the D19′ duplicate folds away — the four engine files of `e4d99806` are byte-identical
  to G1.6b's `06808a6d` already on `update`, and the only conflict, the pin
  `makebuslist_keeps_the_meter_zones`, is resolved toward `update`'s `Dss::pd_elements()` walk
  (same three assertions, `Line.l2` parent identity included; lane-e's `branch_parent`/
  `branch_parent_name` helpers go with it, nothing else used them). `FORCED_ELEMENT_EXTRAS_POPULATION`
  re-derived (440, 313, 83, 44) → **(441, 310, 87, 44)** — an INCREASE, G1.4a's D12/D14 corpus
  flips — and it is now the same population as `FORCED_PROPS_POPULATION`/`FORCED_BUS_POPULATION`.
  `WP_G1_MODES` stays `update`'s **102** (G1.3d(i) adds no row and live-compares five of them);
  the extras census keeps lane-e's `corpus_manifest.rs` placement (F5 STOP-2, the structural race
  removal). Merged-tree checks: fmt + clippy clean in both lanes, corpus gate **524/524**
  (`corpus_gate` binary 218 / 0 / 0 default, 218 / 0 / 0 parity), ledger **54** entries /
  **1 564** hits / 0 stale, D11(2) still 8 pairs, r4133 props 1 670 walks / 151 783 elements,
  `population.lock.json` regenerated with **zero cell diff** and `golden.lock.json` unmoved;
  the full five-command gate reads **5 941 / 0 failed / 5 ignored** per lane and `lane_diff`
  **PASS** with max |Δ| = 0 on all eight kinds (524 cases, 3 220 881 records).

  merge: lane lane-e -> update, see git log
- **G1.7** (2026-09-05, lane `lane-s`, decisions **D2**/**D3**/**D4**/**D7**/**D15**/**D16**) — the
  six order-free `Topology` rows (`DDLL/DTopology.pas:65-98`, `:270-390`; capi
  `CAPI_Topology.pas:81-215`, `:369-405`) go live on **both** channels in one commit: new
  `exec/view.rs::topology_view` (the port never memoizes — upstream frees `Branch_List` only at
  `Common/Circuit.pas:703`/`:2308`), captures in `oracle_server.py` and `dss-epri/capture.rs` read
  **strictly last**, a zero-tolerance `harness/topology.rs` comparator, `compare_topology`
  `wired: true` forced on every live non-`large` case (`FORCED_TOPOLOGY_POPULATION`,
  440 = 313/83/44 on the lane, re-derived at the merge — below; seven decks also declaring it
  so the lock sees it) — **no floor,
  0 ledger entries, 0 new `LEDGER_FIELDS`, 0 golden bytes**; the lock moved by exactly seven
  `topo=` tokens. **No FFI added** (G1.0 had bound the six modes; D2 discharged). Two upstream
  defects are asserted, never excluded: the memoized tree (D15 — the four isolation fields
  compared at step 0 and while the port's topology is unmoved, else `oracle(k) == port(0)`) and
  the overlapping-window pair dedup (D16 — `oracle.looped_pairs == window_dedup(port candidates)`,
  `DTopology.pas:286-296`), pinned by `TOPOLOGY_STALE_DECLINES = (16, 135)` and
  `LOOPED_PAIR_WINDOW_DECLINES = (8, 96)` (fail-on-stale both ways over 3 314 compared triples
  on the lane, 3 312 on the merged tree — both constants unmoved)
  plus `topology_pins::{topology_reads_a_freshly_built_tree, looped_pairs_lose_the_straddling_window}`.
  **Two port gaps found and fixed in-part**, in this same commit:
  adjacency routed by `TPDElement.IsShunt` instead of `IsShuntElement` (r4133
  `Common/Utilities.pas:1262-1274`), which hid every GICTransformer loop (pin
  `a_gictransformer_is_a_tree_branch_and_can_close_a_loop`); and `set_nconds` forcing a terminal
  reallocation r4133's `Set_NTerms` guard (`CktElement.pas:386`) skips, which unwired every
  terminal on a no-op `Phases=` re-set (pins `a_no_op_set_nconds_keeps_the_terminal_state`,
  `a_second_makeposseq_keeps_the_circuit_connected`). The B16 gap, the two shape normalizations,
  the settlements and the two `investigations/to_opendss/` reports: `TESTING.md` §"The unified
  corpus gate" + §"The r4133 bridge", `tests/TOLERANCE_NOTES.md` §G1.7, the plan's dated §G1.7
  note; **28** `file.rs:LINE` citations in `TESTING.md` / `tests/TOLERANCE_NOTES.md` were
  re-pointed after `harness/mod.rs` (+6) and `corpus_gate.rs` (+10) shifted under them
  (`operational_docs_line_citations_point_at_the_line_they_name` was red until they were).
  Commits: `8fc32991` (surface, one commit carrying both port-gap fixes) + `898f8a86`
  (audit settlement) + `434a6b51` and this record (docs).
  Gate at `8fc32991`: fmt + clippy clean in both lanes, `cargo test
  --workspace` **5 043 / 0 failed / 5 ignored** per lane (+317 on G1.9's 4 726 — 121 in the
  new `topology_pins` binary, 8 `harness::topology` cases in each of the other 22 of the
  23 binaries carrying `mod harness`, the rest in-engine and gate code), 523 manifest
  cases (519 compared) green on both channels in both lanes, census
  3 314 / (16, 135) / (8, 96), ledger 57 entries / 1 588 hits / 0 stale, `golden_lock` + `population_lock` + `oracle_parity_cfg_gate` green,
  no golden byte; `lane_diff` run (product code moved — `exec/view.rs`, `ckt_tree/mod.rs`,
  `solution/topology.rs`, `elements/ckt.rs`): **PASS**, max |Δ| = 0 on all eight gated kinds
  over 3 220 861 records, 0 iteration drifts.

  **Audit settlement** (2026-09-05, `lane-s`, `898f8a86`) — 15 findings (13 distinct):
  **10 fixed / 3 recorded / 0 refuted**, no port bug, no behavior change, 0 ledger rows.
  Fixed: the "each in its own commit" claim in `TESTING.md` + the plan note (one commit);
  the missing sha; four `harness/mod.rs:A-B` citations, plus the rail that let a range END
  rot — `operational_docs_line_citations_point_at_the_line_they_name` now reads it and
  anchors inside the range (three failure directions driven; it found a fifth stale
  citation); `Fault` named as the second class where `TPDElement.IsShunt` and
  `IsShuntElement` part (r4133 `PDElements/Fault.pas:244`, `:409`, `:114` — off
  `pd_elements`, so unreachable); `get_topology`'s no-cache reason; the harness
  `window_dedup`/`per_pair_dedup` made case-sensitive like the Pascal `=` and the port's
  `==` (`the_dedup_models_match_names_case_sensitively`; census unmoved); a `debug_assert!`
  on the `loop_elem` invariant; `the_oracle_side_shape_arms_have_teeth` now drives arms 1-2
  through the real comparator; the measured trailing-empty population (134/46 reads over
  65/21 cases) in `TESTING.md`; and the G1.9-shaped registry
  `the_g1_7_pins_the_docs_cite_exist_exactly_once` (14 names with expected definition
  counts — `window_dedup` = 2 twins). Recorded, not fixed: the per-case decline table stays
  runtime output (`harness::topology::decline_report()`, quoted in every mismatch message)
  rather than transcribed into a record already over length; the 135 declined case-steps
  have no oracle answer to compare the port's fresh one against (D15's inherent residual —
  a step that starts or stops declining still reds); and **D12 — pending sync**: the four
  GICTransformer decks sit inside the forced topology population, `makeposseq_shunt.dss` on
  the capi channel, until lane-b's flip to `r4133` lands (no masking — a bad capi process
  reds). Gate, both lanes: fmt + clippy clean,
  **5 067 / 0 failed / 5 ignored** (+24 = 23 `mod harness` binaries + the cfg gate), census
  3 314 / (16, 135) / (8, 96) and ledger 57 / 1 588 hits / 0 stale unmoved, no lock or
  golden byte; `lane_diff` re-run: **PASS**, max |Δ| = 0 over 3 220 861 records — the
  final-tree totals for G1.7.

  **Merged-tree checks** (merge into `update`, 2026-09-05).
  `FORCED_TOPOLOGY_POPULATION` re-derived (440, 313, 83, 44) -> **(441, 310, 87, 44)** — G1.4a's
  D12/D14 corpus flips (three `GICTransformer` decks onto `r4133`, the new `makeposseq_gic.dss`),
  so it is again the same population as `FORCED_PROPS_POPULATION`/`FORCED_BUS_POPULATION`; the
  census re-measures **3 312** triples with **D15 `(16, 135)` and D16 `(8, 96)` unmoved**. That
  also discharges the settlement's "D12 — pending sync" clause: on `update` the GICTransformer
  line no longer sits in the capi-gated `makeposseq_shunt.dss` — D14 moved it into the
  `r4133`-gated `makeposseq_gic.dss` — so no capi-gated case in the forced topology population
  instantiates one, which `no_capi_gated_case_instantiates_a_gictransformer` enforces. `WP_G1_MODES` stays `update`'s **102** (G1.7 adds no row — G1.0 had
  bound the six topology modes). Checks: fmt + clippy clean in both lanes, corpus gate
  **524/524** (`corpus_gate` binary 229 / 0 / 0 default), ledger **54** entries / **1 564** hits /
  0 stale, `population.lock.json` regenerated (the same seven `topo=` tokens over 524 rows) and
  `golden.lock.json` unmoved, `oracle_parity_cfg_gate` 18/18, `capture_order` 22/22,
  `topology_pins` 174/174. **Pin 8 re-pointed**: D14 had moved the `new gictransformer.gt`
  line out of `makeposseq_shunt.dss` after lane-s branched, so
  `a_gictransformer_is_a_tree_branch_and_can_close_a_loop` was asserting on a deck that no
  longer holds a GICTransformer (measured `NumLoops` 0 against its literal 1). It now drives
  BOTH halves of the class switch, one deck each: `makeposseq_gic.dss` for the branch half
  (r4133 one-shot, `NumLoops` **1**, `AllLoopedPairs` `[Line.feed, GICTransformer.gt]`) and
  the now GIC-free `makeposseq_shunt.dss` for the converse (r4133 **0** loops and an EMPTY
  candidate list, a strictly sharper guard than the old "no capacitor among the candidates").
  Non-vacuity driven both ways in a scratch copy and restored (sha256 checked): routing by
  `TPDElement.IsShunt` again reds half (a) at `NumLoops` 0 vs 1, and a class switch that
  answers `false` for everything reds half (b) at 4 vs 0. A further **39** `file.rs:LINE`
  citations in `TESTING.md` /
  `tests/TOLERANCE_NOTES.md` were re-pointed where the merge shifted `harness/mod.rs`,
  `corpus_gate.rs`, `corpus_gate/runner.rs`, `exec/view.rs` and `dss-epri/{capture,dss}.rs`
  under them. Conflict resolutions: `capture_order.rs` kept `update`'s marker gate and folded
  G1.7's six order-free rows + the no-cursor test into it (one `Lang`, both `code_of` and
  `code_only`); `runner.rs` runs the PDElements walk and then topology last; `oracle_server.py`
  keeps G1.4a's bus reads ahead of `all_properties` with `capture_topology` strictly after it.

  merge: lane lane-s -> update, see git log

- **G1.6(i)** (2026-09-05, lane `lane-m`; **D7**, **D17a** `Meters.Totals` at the energy tier, **D18/D11**
  the JSON decoder) — **meter extras + the run protocol**, the only WP-G1 sub-step that changes how a case
  is *run*: no live deck ran `CalcReliabilityIndices`, so the gate drives the executive `RelCalc` **once**
  per case, on the last step, on all three engines (it is not idempotent), tolerating errno **52902** alone
  (r4133 `Meters/EnergyMeter.pas:2502`) and comparing the abort. Six manifest-flagged cases compare the
  indices, every section, `CalcCurrent`/`AllocFactors`, `Meters.Totals` and the zone lists' new ordered arm
  — exact but for three cells banded from existing tiers. **0 ledger entries:** the two arrays both oracles
  read uninitialised (r4133 `Meters/MeterElement.pas:45-52`, report
  `investigations/to_opendss/62-metered-sensor-arrays-are-never-initialised.md`) are excluded per (channel,
  field) in `harness::RELIABILITY_SKIP_FIELDS`, pinned
  `meter_alloc_factors_are_zero_until_allocateloads_runs`, and compared live on the new
  `controls:energymeter/midi_relcalc.dss` (523 → **524** cases); G1.6b's two deferrals are discharged
  (`pd_elements_relcalc_fields_are_live_after_relcalc`, `tests/TOLERANCE_NOTES.md:812`). Detail:
  `GOLDEN_REBASE_PLAN.md` §G1.6 as-executed (i), `TESTING.md` §"The `Meters` reliability surface".
  Commits: `e343d9e8` (D11 hunk), `96d7540a` (surface), `bcc835b6` (audit settlement) **+ docs**. Gate:
  five commands exit 0 in both lanes, **5 011 passed / 0 failed / 5 ignored** per lane; corpus gate
  524/524, ledger 57 entries / 0 stale, no golden byte and no lock content moved; `lane_diff`
  `VERDICT: PASS`, max |Δ| = 0 on all eight kinds (524 cases / 3 221 034 records).
  *Audit settlement* (`bcc835b6`): 20 findings — **15 fixed / 3 recorded / 2 refuted**. Fixed: the
  `alloc_factors` band gains the denominator floor its derivation always claimed (no band below `i_abs`,
  loud triage instead of a silent pass); `RelCalc` keeps every error line, not just the first; the
  accumulator pin's order claim is made true by a branch-point fixture whose 3-term sweep sum is
  association-sensitive (`0.6400000000000001`, bit-identical on both oracles); the doc-quoted pins and the
  `kind=large*` cost guard gain register tests; two off-by-one citations and three damaged diagnostic
  strings repaired; the R-1 state-neutrality partition, the `Meters.Totals` 1e-4 exposure and the
  `AllocateLoads` coverage split are written down. **Recorded:** AT-1 — the pin freezing upstream's
  cross-zone accumulator leak stands, the leak being confirmed on BOTH oracles and also making the FIRST
  run depend on meter declaration order (`2.0/3.0` vs `3.0/3.0`); reported as
  `investigations/to_opendss/61-relcalc-cross-zone-accumulator-leak.md`, ordering arm added to the pin, the
  correct-value fix left as an **engine finding** (R-14(d) forbids `solution/meters/reliability.rs` here)
  and carried in STATUS's standing follow-ups because G1.6(ii) gates the columns it perturbs. **Refuted:**
  AT-9 — dss-python raises on the 52902 whatever `EarlyAbort` says (`DSSGlobals.pas:259-265` sets
  `ErrorNumber` unconditionally); AT-5 — the kW-rewriting `AllocateLoads` branch is oracle-pinned in
  `exec::tests::allocation`.

  *Merged into `update` 2026-09-05*, on top of G1.7 — every conflict was G1.7's topology surface
  against this one, resolved by keeping both: `engines.rs` sends both request keys (reliability,
  then topology strictly last), `capture_order.rs` (update's canonical file) gained a
  `reliability` anchor so the topology-last gate asserts topology after the reliability capture
  too, with its own negative case, and TESTING.md / `tests/TOLERANCE_NOTES.md` / this record keep
  both sides' sections with every shifted `file:LINE` citation re-pointed
  (`oracle_parity_cfg_gate::operational_docs_line_citations_point_at_the_line_they_name` walks
  108 + 16 of them). Re-derived on the merged **525**-case tree: `FORCED_TOPOLOGY_POPULATION`
  (441, 310, 87, 44) → **(442, 311, 87, 44)**, the moving case being `midi_relcalc`, while
  `TOPOLOGY_STALE_DECLINES` (16, 135) and `LOOPED_PAIR_WINDOW_DECLINES` (8, 96) did **not** move;
  `WP_G1_MODES` **103**, `LEDGER_FIELDS` **15** (the union), `ledger.json` untouched (54 entries /
  1 564 hits / 0 stale), `population.lock.json` regenerated with no diff beyond the auto-merge and
  `golden.lock.json` unmoved. Merged-tree checks: fmt + clippy clean in both lanes, `corpus_gate`
  **525/525** cases (238 / 0 / 0 in the default lane).

  merge: lane lane-m -> update, see git log
- **G1.5** (2026-09-05, lane `lane-b`, bus chain — **D7**) — **the short-circuit surface**
  (`Bus.Zsc1`/`Zsc0`/`ZscMatrix`/`YscMatrix`/`Isc`/`Voc`) live on both channels for every live
  non-`large*` case, on G1.4a's per-bus capture (`compare_zsc ⇒ compare_bus`, asserted, never
  or-ed): **precomputed state only** — the gate never runs or refreshes a study — the discrete
  "study ran" bit before any number, the matrices row-major over the bus's **internal** node index
  (r4133 `DDLL/DBus.pas:431-459`/`:374-397`/`:351-372`, `Common/Bus.pas:215-229`). Port gap closed
  in-step: `ReduceAlgs`' `kVBase <= 0` branch skipped `Solution.UpdateVBus`
  (r4133 `Meters/ReduceAlgs.pas:500-508`), leaving `Bus.Voc` stale. **0 new ledger entries** (the
  per-channel not-run sentinels are comparator normalizations, **D4**; **D11(2)** narrows here to
  `Voc`/`Isc`), no new tolerance constant, no golden byte; corpus 524 → **525**
  (`modes/faultstudy/faultstudy_micro.dss`, the `micro`-band witness). Rules, floors and pins:
  `TESTING.md`, `tests/TOLERANCE_NOTES.md` §"Short-circuit surface", the plan's §G1.5 note, and the
  `exec::view::bus_sc_tests` / `harness::bus_short_circuit_tests` modules.
  Commits: `7d920701` (surface) + `5d206bdb` (audit settlement) + docs. Gate at `7d920701`: fmt +
  clippy clean and **5 130 / 0 / 5 ignored** per lane, `lane_diff` **PASS**, max |Δ| = 0 over
  4 825 571 values; final tree **5 134 / 0 / 5 ignored** per lane over 75 binaries (the settlement
  moves no executable product statement, so no second `lane_diff` is owed), corpus 525/525, ledger
  53 entries / 1 516 hits / 0 stale, D11(2) 8 (case, channel) pairs, `tests/golden` untouched.
  *Audit settlement (2026-09-05, `5d206bdb`):* 14 findings, 11 distinct — **8 fixed, 2 recorded,
  1 refuted**. Fixed: the `bus_sc_tests` band had abs/rel transposed (now the `micro` tier's own
  `1e-6` + `1e-9`, deck header and manifest note with it); the surface's non-trivial half gained a
  fail-on-stale (`SC_STUDY_POPULATION = (10, 646)`, recorded from the runner, asserted in the gate
  epilogue, pinned both ways) — `port_ran == oracle_ran` is equally true when NEITHER ran; **D2**'s
  cross-transport check now covers the six SC arms; the row-major flatten got the executable guard
  the measured-vacuous transpose demo left it without; the value drives run on both channels; a
  wrong-rev `YMatrix.pas` citation and the 0.42-vs-**0.61** population worst were corrected.
  Recorded: the κ measurement ran after F4's bands (outcome unchanged — branch (i), no band moved);
  the bands are re-used tier constants, derived in `TOLERANCE_NOTES`. Refuted: `update_vbus` cannot
  panic — `nodes`/`ref_no` grow only inside `reprocess_bus_defs`, whose tail re-allocates `vbus`.
  Reports `tmp/g15/audit_{code,tests}/report.md`, table `tmp/g15/settle.md`.

  *On `update` after the merge (2026-09-05):* the lane's figures are its own; merged with
  G1.6(i)'s `midi_relcalc` deck the corpus reads **526** cases / 522 live / 366 `both`, so the two
  decks together moved every live-non-`large` lock: `FORCED_{PROPS,ELEMENT_EXTRAS,PDELEMENTS,BUS,
  ZSC,TOPOLOGY}_POPULATION` are **(443, 312, 87, 44)**, up from the (441, 310, 87, 44) population
  either deck saw, and `FORCED_DERIVED_POPULATION` — that set plus its two `Test/AutoTrans`
  opt-ins — is **(445, 314, 87, 44)**, all seven re-derived by their own tests; while
  `SC_STUDY_POPULATION` **(10, 646)**, `TOPOLOGY_STALE_DECLINES` (16, 135),
  `LOOPED_PAIR_WINDOW_DECLINES` (8, 96), `WP_G1_MODES` 103 and `LEDGER_FIELDS` 15 did **not**
  (`midi_relcalc` runs no fault study, `faultstudy_micro` is single-step and radial).
  `ledger.json` untouched by both sides (54 entries / 31 causes, 1 564 hits / 0 stale),
  `population.lock.json` regenerated with no diff beyond the two case rows, `golden.lock.json`
  unmoved. Merged-tree checks: fmt + clippy clean in both lanes, `corpus_gate` **526/526** cases
  (256 / 0 / 0 in the default lane), D11(2) still 8 (case, channel) pairs, the 24 TESTING.md
  citations the merge shifted re-pointed.

  merge: lane lane-b -> update, see git log

- **G1.3d(ii)** (2026-09-05, lane `lane-e`; D2/D4/D7) — `PhaseLosses` + the five control-derived
  scalars (`NumControls`, `OCPDevIndex`, `OCPDevType`, `HasVoltControl`, `HasSwitchControl`) on both
  channels over the same **440** cases, completing `compare_element_extras` and §G1.3d.
  `CktElement::phase_losses` ports r4133 `Common/CktElement.pas:1078-1120`; the five scalars read the
  derived per-element `ControlElementList` (`Controls/ControlElem.pas:113-131`, re-run by every
  `RecalcElementData`, `Relay.pas:955`) that `Show Controlled` and the reliability sweep share.
  **0 new ledger entries / 0 new causes** (58 / 31), **10** measured widenings onto the new
  `phase_losses` sub-channel, 0 golden bytes, no band moved, `WP_G1_MODES` 97; the floor derives from
  `assert_power_close` (`tests/TOLERANCE_NOTES.md` §G1.3d(ii)) and `PhaseLosses` joins
  `LANE_SKIP_ELEM_POWERS` on the two `newton*` decks (same cache-aware `ComputeIterminal`, red
  measured first on both channels). Unledgered but pinned: the per-edit re-attach
  (`DIVERGENCES.md` L9, `ocp_dev_type_follows_the_last_attach_order`), the disabled OCP control
  (`a_disabled_ocp_control_still_wins_the_ocp_scan`), adjacent defect A-1
  (`section_device_type_is_the_live_ocp_scan_not_the_registration_latch`); A-2 recorded in STATUS,
  owned by G1.6/G1.6b. Detail: plan §G1.3d part (ii), TESTING.md; **34** pins in
  `every_pin_the_g13d2_record_names_exists_and_is_cited` (`exec::tests::element_extras` 20,
  `harness::element_extras_pins` 26, `harness::phase_loss_bands` 10, 3 `ledger::*`, 1
  `capture_order::*`, 1 `exec::tests::reliability::*`). Commits `e6d66d66` + settlement `43108993`
  + docs (this record); gate **5 784 / 0 / 5** per lane (five commands, exit 0, unfiltered),
  `lane_diff` PASS max |Δ| = 0.
- **G1.3d(ii) audit settlement** (2026-09-05, `43108993`) — 18 findings (9 code / 9 tests, all
  Minor/Note; 4 raised by both auditors, so 14 distinct): **13 fixed**, **1 recorded**, 0 refuted.
  The real one: the port re-attached every
  control during `MakePosSeq`, which r4133 never does — its control `MakePosSequence` overrides end
  in `inherited` and never reach `RecalcElementData` (`Relay.pas:1008`, `Recloser.pas:738`,
  `SwtControl.pas:367`, `CapControl.pas:656`, `RegControl.pas:1491`; `Fuse` has none;
  `ExecHelper.pas:3069-3086`), so the re-attach moved out of the shared post-edit tail into
  `exec::command::reattach_edited_control`, pinned by `makeposseq_does_not_reattach_controls`. Also:
  a fail-on-stale population guard for D-ii-1's zero rows (`assert_no_multi_control_element`, 3 pins)
  which **corrected the sub-step's own premise** — the quoted census covered `controls/**` only,
  while the gated population holds **18** multi-control rows (3 184 controlled of 298 565), all of
  them Relay-ONLY lists, so the conclusion stands on the right fact and the counts are now pinned,
  `LANE_SKIP_ELEM_POWERS` locked to its two labels + the 7th `ElemChannels` bit added to the
  anti-tautology asserts, the G1.3d(i) pin-count lock made exact again where no successor owns it and
  the G1.3d(ii) row check made per-group, the newton red re-measured on the **r4133** channel
  (`4.855901044093186e-4 > 8.753018514278278e-6`, `2.4606876731535624e-3 > 7.144000397412528e-5`),
  and 21 wrong Pascal line citations swept (`:1090` `ComputeIterminal`, `:1118-1119` zero-fill,
  DDLL `:637-659`/`:651`, capi `:896`). Recorded, not fixed: `Scope::dead_channels` still polices only
  `divergence` entries — an exclusion covers causes that cannot be re-measured reliably (four of the
  ten widenings sit on D12's self-disagreeing capi GIC decks), reason now in TESTING.md and
  `ledger.rs`.

  *On `update` after the merge (2026-09-05):* the lane's figures are its own. Ten conflicts,
  all resolved keeping both surfaces; the only semantic one is the ledger. The lane's **ten**
  `phase_losses` widenings land as **eight**: `gic-pct-r2-honoured-{gictransformer,midi}-capi`
  were deleted on `update` by G1.4a's **D12/D14** (those decks now gate `r4133`-only), so the
  widening goes with the entry and the r4133 twins keep theirs — ledger **54** entries / 31 causes,
  unchanged by this merge. `makeposseq-cuf-applied-capi` was the one three-way entry (both sides
  edited it): `match` unioned, both sides' `source` paragraphs kept, and its `phase_losses` sample
  **re-measured on the merged tree** — D12/D14 had moved that deck's solve, so the lane's number no
  longer existed; the widening was re-driven alone and still fails
  (`Vsource.source` phase 0, `|Δ| = 5.0822934012897065e1 > 4.385766432859287e-5`), so it is kept.
  Re-derived on the merged 526-case tree and **unmoved**: all seven forced populations
  (443, 312, 87, 44) / `FORCED_DERIVED` (445, 314, 87, 44), `SC_STUDY_POPULATION` (10, 646),
  `TOPOLOGY_STALE_DECLINES` (16, 135), `LOOPED_PAIR_WINDOW_DECLINES` (8, 96), D11(2) 8 (case,
  channel) pairs, `WP_G1_MODES` **103**, `LEDGER_FIELDS` 15; the control census re-measured
  **298 536 / 3 190 / 18 / 18** (the exact `(18, 18)` half unmoved, the two floors moved with the
  merged corpus). One merge gap fixed: `harness/aggregates.rs` (G1.9) builds three `ElementCap`
  literals, which the seven new fields left incomplete — spelled out, not defaulted, per that
  fixture's own rule. Merged-tree checks: `fmt` + `clippy` clean in both lanes, `corpus_gate`
  **526/526** cases (276 / 0 / 0 in the default lane), ledger 54 entries / 1 564 hits / 0 stale,
  `population.lock.json` regenerated (8 rows, `@digest` only), `golden.lock.json` and
  `tests/golden/**` untouched, and the 61 `file.rs:LINE` citations the merge shifted re-pointed
  (diff-mapped) so `operational_docs_line_citations_point_at_the_line_they_name` is green.

  merge: lane lane-e -> update, see git log

- **G1.6(ii)** (2026-09-05, lane `lane-m`; **D7**, **D20**/**D22**) — the eight per-bus reliability
  columns (`IBus._columns`; r4133 `DDLL/DBus.pas:60-73`, `:129-170`) live-compared on both oracle
  channels inside part (i)'s payload: no new flag, same `RelCalc`-once protocol, same 6-case
  population (4 capi / 50 buses, 5 r4133 / 84), **exactly** (`rel = abs = 0`), keys
  `bus:<bus>:<field>` on the existing `reliability` field — **0 ledger entries**, no golden byte, no
  lock moved; `WP_G1_MODES` 103 → **111**, `EXCLUDED_WRITE_MODES` gains `Bus F:4`. **D20/D22**, its
  own commit ahead of the surface: `calc_reliability_indices` regains `AssumeRestoration := …` +
  `TotalUpDownstreamCustomers` (r4133 `Meters/EnergyMeter.pas:2466-2468`) — a **measured capi
  divergence, unreachable on the corpus** (`docs/upgrade/DIVERGENCES.md` §D22). The pins, the Q4
  `Bus.Int_Duration` measurement and the exactness derivation: `TESTING.md` §"The per-bus
  reliability arm", `tests/TOLERANCE_NOTES.md` §"The per-bus columns (G1.6(ii))",
  `GOLDEN_REBASE_PLAN.md` §G1.6 as-executed (ii). Commits: `3e65ae2d` (D20/D22) + `572954e6`
  (surface) + `fc4dfa73` (audit settlement) **+ docs**. Gate (both lanes, after the settlement): the
  five commands exit 0, **6 307 passed / 0 failed / 5 ignored** over 78 binaries, corpus gate
  525/525, ledger 54 entries / 0 stale, no golden byte and no lock moved; `lane_diff` `VERDICT:
  PASS`, max |Δ| = 0 on all eight kinds (525 cases / 3 221 054 records).

  *Audit settlement* (2026-09-05, `fc4dfa73`): 11 findings — **9 fixed / 2 recorded /
  0 refuted**. Fixed: the roll-up guard's "dead code" comment (it is live — `RelCalc restore=y` takes
  it); the restore-regime per-bus columns, now asserted for the PORT exactly in both regimes; the
  `NaN`-agreement claim, corrected to what the transports do (a non-finite fails the decode loudly)
  and pinned by `a_non_finite_reliability_cell_fails_the_decode_on_both_transports`; the eight-column
  non-vacuity, moved out of a log into `every_bus_reliability_column_is_compared_per_bus`; the
  mode-table sum; this record's length, shas and gate; a named owner for the zone-boundary decision
  (`ORPHANED_GAPS.md` §1.20); and the r4133 half of the four pin tables re-measured from scratch here
  — a fresh `epri-worker` capture reproduces all **55** pinned per-bus tuples bit-for-bit.
  **Recorded:** the pre-existing corpus dropping leak (an oracle-side I/O race on a deck-written
  export, unowned by this sub-step — droppings deleted, never staged); and the citation checker
  staying Rust-only, since `.pas` lives in the gitignored `.inputs/` — the 115 Pascal citations this
  sub-step adds were swept mechanically here and all resolve.

  Merged-tree checks: `fmt` + `clippy` clean in both lanes, `corpus_gate` **526/526** cases
  (282 / 0 / 0 in the default lane), ledger 54 entries / 1 564 hits / 0 stale; every census
  re-derived on the merged tree is unmoved — bus reliability capi **4** payload(s) / **50** bus(es)
  and r4133 **5** / **84**, control census **298 536 / 3 190 / 18 / 18**, `SC_STUDY_POPULATION`
  (10, 646), D15 (16, 135) / D16 (8, 96), every `FORCED_*` population, `WP_G1_MODES` **111**;
  `population.lock.json` regenerated with **no diff**, `golden.lock.json` untouched, and the 26
  `file:LINE` citations the merge shifted re-pointed. Two cross-lane marker collisions fixed in
  `corpus_gate.rs`: this sub-step's nested `"buses"` key made both slot markers of
  `the_bus_capture_reads_in_one_fixed_order_on_both_transports` ambiguous against G1.5's
  parenthesised checkpoint slot — re-spelled `capture_all_buses(ckt, want_zsc)` and `"buses": (`.

  Merged-tree gate (both lanes): the five commands exit 0, **7 436 passed / 0 failed / 5 ignored**
  per lane; `lane_diff` `VERDICT: PASS`, max |Δ| = 0 on all eight kinds (526 cases /
  3 221 146 records).

  merge: lane lane-m -> update, see git log
- **G1.3b** (2026-09-05, lane `lane-e`; D4/D7/D24) — per-element `SeqCurrents`, `SeqVoltages`,
  `SeqPowers` on both channels over the 442 `compare_derived` cases, one new `exec/view.rs` accessor;
  **0 new ledger entries / 0 causes** (58 / 31), **31** measured widenings on 11 `element` scopes,
  0 golden bytes, no band moved. Three cross-channel divergences, none reproduced or ledgered:
  r4133's 1φ-posseq slot/stride defect (`DDLL/DCktElement.pas:760`/`:768` against capi
  `CAPI/CAPI_Alt.pas:555`/`:562`; report `to_opendss/67-seqpowers-posseq-slot-stride.md`) has zero
  r4133 exposure — census (297 896, 79, 0) behind `assert_seq_arm_population`; the n/A `SeqPowers`
  sentinel (`:772` against `:567`) is the channel-scoped `na_seq_power` fold; the two 012 matrices
  cost the r4133-only `SEQ_C012 = 5.229590094302253e-10` (`Shared/mathutil.pas:302-303`+`:562-564`).
  Detail: plan §G1.3b, TESTING.md, TOLERANCE_NOTES §G1.3b; pins **8** `exec::tests::derived_seq` /
  **35** `harness::seq_floors` (22 legs) / **7** `ledger::*`. `40a65ffd` / `3d350ce6` + docs;
  gate **6 570 / 0 / 5** per lane after the settlement (6 547 at `40a65ffd`), `lane_diff` PASS max |Δ| = 0.
- **G1.3b audit settlement** (2026-09-05, `3d350ce6`) — 15 findings: **12 fixed / 3 recorded / 0 refuted**. The
  arm census now pins the r4133 `0` exactly and rails the two measured counts at the documented
  `SEQ_ARM_CENSUS_FLOORS` (new leg `the_seq_arm_population_fires_when_the_capi_arm_collapses`);
  registry `every_pin_the_g13b_record_names_exists_and_is_cited` added; the plan's D-b1 population
  premise corrected (11 decks raise `CktModel=Positive`, four r4133-gated but 2φ/3φ); six Pascal/pin
  citations (`CAPI_Alt.pas:608` not `:607`, the dangling `a_discrete_seq_slot_*`); both commit-sha
  placeholders in this record and STATUS; this record trimmed. Recorded, not fixed: `SEQ_C012`'s in-tree
  `SymComp::official()` source (2.1e-7 tighter), the n/A magnitude sentinel `1.0` as a convention
  `seq_arm` disambiguates, `dead_channels` policing `divergence` entries only (`ledger.rs:414-418`).
- **G1.3b landed on `update`** (2026-09-05, coordinator decision **D31**) — the merge put the
  sequence surface together with D12/D14's r4133-gated `modes/makeposseq/makeposseq_gic.dss`, the
  first corpus case to reach the 1φ-posseq arm on that channel: 4 element rows at step 0,
  `seq_powers` only (`seq_i`/`seq_v` are right on r4133 and stay compared) — measured by the live
  gate and a one-shot `epri-worker` probe on the merged tree. Settled exactly as the census tripwire
  prescribed: ledger **54 → 55** entries / **31 → 32** causes (`r4133-posseq-seqpowers-slot-gic`,
  scope `element`/`seq_powers`, cause `posseq-seqpowers-slot-stride`, report
  `to_opendss/67-seqpowers-posseq-slot-stride.md`), the both-numbers pin
  `the_gic_posseq_deck_keeps_every_terminal_power_in_its_positive_slot`, an `exclusion`
  naming `seq_powers` widened to neutralize the whole posseq power array (it would otherwise exclude
  nothing — the defect is entirely in the cells the banded-slot rule left alone; `divergence`, the two
  magnitude channels, the n/A arm and the port's own zeros are unchanged, measured slot by slot on all
  three arms by `an_exclusion_scope_neutralizes_the_whole_posseq_seq_powers_array`), and only then
  `SEQ_ARM_CENSUS_MEASURED` `(297 896, 79, 0)` → `(297 867, 78, 4)` (all three re-derived on the merged
  tree; the capi count drops one row because D14 moved the `GICTransformer` off `makeposseq_shunt.dss`),
  now red in both directions
  (`the_seq_arm_population_fires_when_another_deck_brings_the_arm_to_r4133`,
  `the_seq_arm_population_fires_when_the_r4133_arm_stops_gating`) with D24's rail re-cast as a
  bounded delta. `SEQ_ARM_CENSUS_FLOORS` `(200 000, 60)`, every band, the engine and every golden
  byte unchanged; pins **9** `exec::tests::derived_seq` / **36** `harness::seq_floors` (23 legs).

  Merged-tree gate (both lanes): the five commands exit 0, **8 282 passed / 0 failed /
  5 ignored** per lane; `lane_diff` `VERDICT: PASS`, max |Δ| = 0 on all eight kinds
  (526 cases / 3 221 146 records).

  merge: lane lane-e -> update, see git log

- **G1.4c** (2026-09-05, lane `lane-b`, bus chain — **D7**) — the bus **sequence** and **line-to-line** arms
  (`SeqVoltages`/`CplxSeqVoltages`/`VLL`/`puVLL`) live on both oracle channels, on G1.4a's per-bus capture:
  no new flag, no lock move, no golden byte, **0** ledger rows. Per **D4**/**D8**/**D21** the port answers
  S-SEQ / S-VLL — r4133's own stated intent (`DDLL/DBus.pas:299` against its node-*count* test `:298`;
  `Common/ShowResults.pas:193-194` against the pre-wrap poll `:575-584`); every divergent bus is closed by a
  positive mechanism assertion (**D15**/**D16**) behind four fail-on-stale populations and the sequence-transform
  term `SEQ_C012` (deduped with G1.3b's at the merge, D21: the tight row sum, never the rounded-up `5.30e-10`),
  and r4133's `VLL` hang is refused per bus by the new `crates/dss-epri` register (**D2**). Details:
  `TESTING.md`, `tests/TOLERANCE_NOTES.md`, `DIVERGENCES.md` §G1.4c, the plan's §G1.4 note,
  `investigations/to_opendss/` 64-66. Commits `74cb0ef6` (surface) + `6fc63848` (settlement) + docs; both
  lanes **5 339 / 0 / 5 ignored** over 75 binaries, corpus **525/525**, ledger 53 / 0 stale, `lane_diff`
  **PASS** max |Δ| = 0.
  **Audit settlement** (15 findings, 12 distinct — 7 fixed / 5 recorded / 0 refuted):
  the port's own `VLL`/`puVLL` are now asserted on **every** bus, not only where the oracle's walk
  coincides with S-VLL (AC-1, live-proven on NEV `13kvbus`); `puVLL` and r4133 non-vacuity drives
  added. Recorded: `Export SeqVoltages`' ground substitution → `ORPHANED_GAPS.md` §1.19.
- **G1.4b** (2026-09-05, lane `lane-b`, bus chain — **D7**, split by **D26**) — the bus **distance**
  surface (`Bus.Distance`, `AllBusDistances`, `AllNodeDistances`) live on both channels on G1.4a's
  per-bus capture: no new flag, no force rule, no golden byte, and the only `population.lock.json` move
  is one `ledger=` digest cell. All three publish the ONE zone-build field `DistFromMeter` (written at
  `Meters/EnergyMeter.pas:1833-1838`), read by reference and compared **exactly** (`rel = abs = 0`,
  `tests/TOLERANCE_NOTES.md` §"Bus distance surface") behind `DISTANCE_POPULATION = (867, 79_137)`.
  **D26** moved the at-bus half to a new **G1.4d** (plan amendment, unseen by the user); **D29** step 1
  was measured and refused (r4133's `MergeWith` renames a line without updating `DeviceList` —
  `PDElements/Line.pas:1684`, `to_opendss/68`), so `modes:reduce/midi_reduce.dss` stays capi-gated behind
  the new **`distance`** ledger field: one entry `reduce-merge-units-lost-midi-capi-distance`
  (54 → **55**) pinned by `the_reduced_midi_deck_reports_the_merged_lines_kft_distances`, and D9's
  `MakeBusList` fix is pinned live by
  `the_make_bus_list_decks_report_the_zone_distances_both_oracles_measure`. Details: `TESTING.md`, the
  plan's §G1.4 note. Commits `1aa08d9c` (surface) + `03565bf7` (settlement) + docs; both lanes
  **7 741 / 0 / 5 ignored** over 79 binaries, corpus **526/526**, ledger **55** / **1 567** hits /
  0 stale, `lane_diff` **PASS** max |Δ| = 0.
  **Audit settlement** (2026-09-06; 15 findings, 12 distinct — 10 fixed / 2 recorded / 0 refuted): the
  comparator gained the committed offline drives its own doc claimed
  (`harness::bus_distance_comparator_tests`), a METERED `both` cross-transport pin
  (`the_two_transports_agree_on_the_bus_distances_of_a_metered_both_case`), the population guard's growth
  direction, the `G1_4B_PINS` registry, D29's channel rule as a machine guard
  (`no_r4133_gated_case_reduces_by_merging`), eight re-pointed citations and two corrected populations.
  Recorded: the oracles' `[0.0]` no-circuit sentinel (documented at the accessor; the port keeps `[]`) and
  the third per-(case, channel, step) `all_bus_voltages()` rebuild (pre-existing pattern → G1.4d).

  Merged-tree checks (2026-09-06 landing of both blocks): `fmt` + `clippy` clean in both lanes,
  `oracle_parity_cfg_gate` **21/21** (both sides' pin registries), `population_lock` 3 / `golden_lock` 4
  with `tests/golden/**` untouched, `corpus_gate` **526/526** cases in the default lane, ledger **56**
  entries / 1 571 hits / **0** stale, the sequence-arm census `(297 867, 78, 4)`, `DISTANCE_POPULATION`
  (867, 79 137) and the four seq/VLL populations re-derived unchanged, `population.lock.json` regenerated
  (one `ledger=` cell), and the 47 `file.rs:LINE` citations the merge shifted re-pointed. The two lanes'
  012-matrix term was deduped to ONE constant (`harness::SEQ_C012`, D21): the analytic ceiling
  `2·Δsin60/3 = 5.229591574599605e-10` reconciles G1.4c's rounded-up `5.30e-10` with G1.3b's tight row sum
  `5.229590094302253e-10`, and the tight value is the one kept — G1.4c's live worst
  (`5.229587392548124e-10`) is `0.99999948 ×` it, and the row sum bounds every gap this surface can
  measure, so no band widened and nothing on either surface moved. The full five-command gate on the merged
  tree is green in BOTH lanes (**8 740 / 0 / 5 ignored** each, `corpus_gate` 354 per lane) and `lane_diff`
  **PASS** with max |Δ| = 0 on all eight kinds.

  merge: lane lane-b -> update, see git log

- **G1.3c** (2026-09-06, lane `lane-e`; D4/D7/D24) — per-element `CplxSeqCurrents`, `CplxSeqVoltages`
  and `TotalPowers` on both channels over the same 442 `compare_derived` cases, closing the flag at
  thirteen fields; **0 new ledger entries / 0 causes**, **31** measured widenings on 11 `element`
  scopes on the lane (**25** on **9** at the landing — D12/D14 had deleted two of them; ledger
  **56** / 32), 0 golden bytes, no band moved, `WP_G1_MODES` unchanged (**111** on `update`). Neither G1.3b divergence
  reaches here (measured): the n/A sentinel is `(-1, 0)` on both engines (r4133
  `DDLL/DCktElement.pas:60`/`:106`, capi `CAPI/CAPI_Alt.pas:268`/`:324`) and D-b1's posseq defect
  lives only in mode 9 — no fold, no census (D24). `TotalPowers` joins `LANE_SKIP_ELEM_POWERS` as its
  fourth channel on the two `newton*` decks (`GetPhasePower` opens with `ComputeIterminal`, r4133
  `Common/CktElement.pas:1049`; ~20x the band on both channels); the `CplxSeq*` pair stays compared,
  stronger than fastdss. Detail: plan §G1.3c, TESTING.md, TOLERANCE_NOTES §G1.3c; **55** pins — 7
  `exec::tests::derived_totals` + 1 `exec::tests::newton`, 33 `harness::cplx_seq_and_total_power_floors`,
  13 `ledger::*`, 1 `capture_order::*`. `548bc7b8` / `a15e2ae3` + docs; gate **7 319 / 0 / 5** per lane
  after the settlement (**7 318** at `548bc7b8`), `lane_diff` PASS max |Δ| = 0 (523 cases / 3 220 861).
- **G1.3c audit settlement** (2026-09-06, `a15e2ae3`) — 11 findings: **7 fixed / 4 recorded / 0
  refuted**. Fixed: the missing registry `every_pin_the_g13c_record_names_exists_and_is_cited`
  (55 pins, both group counts — the one **major**); a fourth `require_capture` rail on `cseq_v_re`
  (`runner.rs:718`) driven empty on **both** channels, plus the r4133 leg of the `CplxSeqCurrents`
  rail; ~20 Pascal anchors re-pointed at the construct they name (copy loops `:906-910`/`:952-956`
  vs capi's copy-free `ResultPtr` writes, `setlength` `:900`/`:946`/`:1118`, `GetPhasePower` `:1120`,
  `cmulreal` `:1132`, guards `:878`/`:906`); the ledger's G1.3c date. Recorded: `dead_channels`
  polices `divergence` entries only (**114** `element` sub-channel names at the landing, 103 on exclusions — noted in
  `ledger.rs`), the three `envelope_element` branches no divergence selects yet (fixture-covered;
  inventing a row to make them live is the mask WP-G1 forbids), procedure ranges stopping 1-2 lines
  short of the closing `end;`, and coordinator note "D28", absent in this lane. Carried out of range and closed at the
  landing: the `file.rs:LINE` tripwire reads only TESTING.md / TOLERANCE_NOTES, so
  `harness/mod.rs` and `runner.rs` still cited `ledger.rs:1695-1697` / `:1702` for
  `clone_element_cap` / `rewrite_element_selected`; both were re-pointed at `:2292-2294` /
  `:2301` on the merged tree.

  Merged-tree checks (2026-09-06 landing into `update`): `fmt` + `clippy` clean in both lanes,
  `oracle_parity_cfg_gate` **22/22** (the G1.3c registry beside G1.3b's, G1.4b's and G1.4c's, both
  citation walkers), `capture_order` 25, `population_lock` 3 / `golden_lock` 4 with `tests/golden/**`
  untouched, `corpus_gate` **526/526** cases in the default lane, ledger **56** entries / 1 571 hits /
  **0** stale. Populations re-derived on the merged binary, none moved: sequence-arm census
  `(297 867, 78, 4)` (D31's `posseq_r4133 == 4` holds), `DISTANCE_POPULATION` (867, 79 137), the four
  seq/VLL populations (10, 129) / (4, 54) / (16, 196) / (2, 12), D15 (16, 135), D16 (8, 96), the
  control census (298 536, 3 190, 18, 18). `population.lock.json` regenerated: **9** rows, `ledger=`
  the only moved token. The union kept both sides whole — G1.3c's 31 widenings land as **25** on
  **9** entries (D12/D14 had already deleted the two capi GIC entries it also widened; their r4133
  twins carry the three sub-channels) and `makeposseq-cuf-applied-capi`'s three G1.3c samples were
  **re-measured** here, one scoped drive per sub-channel, because D14 moved that deck's whole solve.
  D31's exclusion widening stays `seq_powers`-only: modes 13/14 write the posseq value into the
  correct slot on both engines (G1.3c F1), so `cplx_seq_*` keeps the banded-slot rule under **both**
  ledger kinds — now driven that way by
  `the_cplx_rewrite_and_the_cplx_envelope_cover_the_same_slots` and by the two `should_panic` rails.
  The full five-command gate on the merged tree is green in BOTH lanes (**9 522 / 0 / 5 ignored** each,
  `corpus_gate` 400 per lane) and `lane_diff` **PASS** with max |Δ| = 0 on all eight kinds (526 cases /
  3 221 146 records).

  merge: lane lane-e -> update, see git log

- **G1.8** (2026-09-05, lane `lane-s`, decisions **D2**/**D3**/**D4**/**D7**) — the four flat
  incidence quantities (`Solution.IncMatrix`, `Laplacian`, `IncMatrixRows`, `IncMatrixCols`; r4133
  `DDLL/DSolution.pas:542-667`, capi `CAPI/CAPI_Solution.pas:860-1021`) go live on **both** channels:
  `exec/view.rs::inc_matrix_view` drives `CalcIncMatrix`+`CalcLaplacian` through the real dispatch
  with the getter's `IncMat_Ordered` branch (`DSolution.pas:616-631`), and both captures read the
  pair **strictly last** (it moves `ActiveCktElement`, `Common/Solution.pas:3007-3010`, and must
  follow the read that memoizes `Branch_List`). Forced on every live non-`large` case (440 =
  313/83/44, six of them declaring it): **no floor, 0 ledger entries, 0 golden bytes**, lock +6
  `incm=`, no FFI or mode added. **Settlement S-INC** — both oracles advance the row cursor for
  *every* reactor (`:3039`, outside the `:3015` guard, unlike `:2885`/`:2938`/`:2986`) — so the port
  emits dense rows (own commit) and the comparator asserts upstream's numbering positively:
  `INC_UPSTREAM_ROW_DECLINES = (4, 5)` fail-on-stale both ways plus `inc_matrix_pins::{the_incidence_row_cursor_skips_a_shunt_reactor,
  the_row_cursor_settlement_holds_on_the_corpus_witness, the_laplacian_is_blind_to_the_row_cursor}`.
  Q2/Q3 of the same walk stay reproduced under the §WP-G2 register; **§G3.2c is re-scoped here** (8
  `*_flat_*` stems go, 20 `*_org_*` stay). Everything else: `TESTING.md` §"The unified corpus gate",
  `tests/TOLERANCE_NOTES.md` §G1.8, the plan's §G1.8 note, and the name registry
  `oracle_parity_cfg_gate::the_g1_8_pins_the_docs_cite_exist_exactly_once`.
  Commits `2cadc808` (dense rows) + `f3436c77` (surface, lock regen and docs) + `166bae9b` (audit
  settlement) + `24348239` and this record (docs). Gate, both lanes: fmt + clippy clean, `cargo test
  --workspace` **5 434 / 0 failed / 5 ignored** per lane at `f3436c77` (+391 on G1.7's 5 043; **5 435**
  on the settled tree, the one new `capture_order` test), 523/523 on both channels, ledger 57 / 0
  stale, 3 314 compared triples / declines (4, 5), `DSS_GATE_DUMP` bit-identical three ways, no
  golden byte, `lane_diff` **PASS** max |Δ| = 0 over 3 220 861 records.
  *Audit settlement* (`166bae9b`): 12 findings — **8 fixed / 4 recorded / 0 refuted**, detail in the
  commit and in `tests/harness/inc_matrix.rs`'s census doc. The load-bearing fix: the S-INC census
  arms off the manifests (`scheduler::inc_matrix_requested_channels`, per channel), so a deleted or
  one-channel-narrowed comparator call site reds instead of self-silencing; also the capi transport's
  kill-criterion refusals gated from source
  (`capture_order::the_capi_incidence_transport_refuses_a_shape_it_was_not_written_for`), four names
  added to the registry, and three Pascal citations plus the Q2 row numbers corrected (both oracles
  emit row 5, the port's dense row is 4). Recorded: Q2/Q3 stay WP-G2 rows with the teardown's exit
  value now in their pins; `large*` stays out; six `harness/mod.rs:LINE` citations in other WPs' text
  are left alone — five already pointed at unrelated content at `1314431a`, so the +7 shift is not
  what broke them (the two operational docs' own citations did move, +7 / +9).

  **Merged-tree checks** (merge into `update`, 2026-09-05).
  `FORCED_INC_MATRIX_POPULATION` re-derived (440, 313, 83, 44) -> **(443, 312, 87, 44)**, again
  equal to `FORCED_PROPS_POPULATION`/`FORCED_TOPOLOGY_POPULATION` as the constant asserts; the six
  moving cases are G1.4a's D12/D14 flips (`asymmetric:gic/gic_midi.dss`,
  `asymmetric:gic/gictransformer_gic.dss`, `solvable_now:.../GICExample/GIC_Example.dss`, `both` ->
  `r4133`) and the three micro decks the other lanes added (`modes:makeposseq/makeposseq_gic.dss`
  `r4133`, `modes:faultstudy/faultstudy_micro.dss` and `controls:energymeter/midi_relcalc.dss`
  `both`). The S-INC census re-measures **3 316** compared (case, step, channel) triples
  (1 644 `capi_v0145` / 1 672 `r4133`) with `INC_UPSTREAM_ROW_DECLINES` **(4, 5) unmoved**; G1.7
  (3 316 / (16, 135) / (8, 96)), G1.3d(ii)'s control census (298 536 / 3 190 / 18 / 18) and
  `SC_STUDY_POPULATION` (10, 646) are unmoved too. `WP_G1_MODES` stays `update`’s **103**
  (G1.8 binds no new mode). Conflicts settled by keeping both surfaces: one canonical
  `crates/dss-core/tests/capture_order.rs` (the G1.8 block re-pointed at `update`'s `read_source` /
  `CAPI_CALLS` / `R4133_CALLS` and its synthetic `Anchors` given `update`'s `reliability` slot, so
  the topology-last rule covers G1.6(i) there too), both pin registries in
  `oracle_parity_cfg_gate.rs`, both capture blocks in `oracle_server.py`, and both new sections in
  `TESTING.md` / `tests/TOLERANCE_NOTES.md`. Checks: fmt + clippy clean in both lanes, corpus gate
  **526/526** (`corpus_gate` binary 287 / 0 / 0 default), ledger **54** entries / **1 564** hits /
  0 stale (G1.8 adds no row), `population.lock.json` regenerated (six `incm=0` -> `incm=1` tokens,
  nothing else moved), `golden.lock.json` and `tests/golden/**` untouched, 53 `file.rs:LINE`
  citations the merge shifted re-pointed so
  `operational_docs_line_citations_point_at_the_line_they_name` is green, and no `CorpusGuard`
  dropping was left behind by the merge gate.

- **G1.10a** (2026-09-06, lane `lane-s`, decisions **D7**/**D25**/**D30**/**D32**/**D33**/**D35**) — the
  **created-file SET** goes live on both channels: for one (case, channel, port-run), the set of
  entries the run creates under the case dir (`OutputDirectory` after `Compile`, r4133
  `Common/DSSGlobals.pas:962`), `/`-joined, a trailing `/` marking a created directory, ASCII-case
  folded, exact at `rel = abs = 0`. Forced on every live non-`large` case (443 = 312/87/44, six of
  them declaring it, one `kind=large`): **no floor**, **1** ledger entry, **0** golden bytes, lock
  +6 `runf=` and one `ledger=` digest, `LEDGER_FIELDS` 15 → 16 (`run_files`, the third per-VALUE
  exclusion field). Three producers now share ONE classification (`dss_epri::guard::classify_created`,
  the `corpus_guard.py` twin, the gate's outer guard), which also defines the surface against
  concurrency — no descent into a pre-existing subdirectory (9 measured members, all belonging to a
  sibling case) — and closes the sibling-case deletion hazard everywhere. **R-18 struck and
  replaced by measurement:** the two ORACLES disagree on the spelling (r4133
  `Executive/ExportOptions.pas:333-356` upper, capi `src/Executive/ExportOptions.pas:314,343,345`
  lower, plus r4133 lowercasing deck-supplied stems), so the port keeps capi's and the comparator
  folds ASCII case — `DIVERGENCES.md` §R-18, pin
  `the_two_oracle_spellings_of_auto1bus_fold_to_one_member`. Structural normalizations, 0 rows:
  the harmonics scratch `<CircuitName_>SavedVoltages.dbl` (r4133 `Common/Utilities.pas:1512-1521`)
  split symmetrically and counted, `SCRATCH_FILE_DECLINES = (9, 9)` fail-on-stale both ways +
  `the_harmonics_scratch_file_is_declined_on_the_nev_deck`. The ONE entry is
  `r4133-visualize-writes-a-dssview-file-pair` (cause `visualize-dssview-file-pair`, `name_re` on
  the two `_pq` names) + `visualize_writes_a_dssview_pair_on_r4133_and_a_json_payload_in_the_port`
  — a product divergence, so `DIVERGENCES.md` and no `to_opendss` note.
  Four sub-parts landed ahead of or beside the surface, each measured, none of them a ledger row:
  **D25** the bridge issues `Set Editor=rundll32.exe` at init (r4133 fires `FireOffEditor` on every
  `Show`/`Dump` unguarded; ~900 orphaned Notepads across the lanes) — own commit,
  `init_overrides_the_os_editor_and_never_writes_it_back`; **D30(1)** the event-log capture reads
  `Solution.EventLog` in memory instead of `export eventlog` (59 of the first drive's 61 reds; the
  two byte-identical corpus-wide) — own commit, `the_in_memory_event_log_equals_the_exported_file` +
  `the_event_log_capture_creates_no_file`; **D32(1)** Storage `DebugTrace` was a **port gap** on the
  authority channel (both oracles write `STOR_<name>.CSV` at edit time, r4133
  `PCElements/Storage.pas:1073-1085`) — ported in its own commit, open-append-close, name compared
  here and contents handed to G1.10b (the two ORACLES disagree there in 16 columns, FPC `%-.g` = 2
  significant digits vs Delphi ~15); **D32(2)/D33** the leak is closed loudly (a dropping surviving
  the sweep fails the case naming its producer; the capi transport clears before sweeping, guarded
  because dss_capi 0.14.5 faults on that `clear` after an AutoAdd solve — recorded in
  `DIVERGENCES.md`, pinned by
  `a_capi_worker_whose_teardown_clear_raises_replies_in_full_then_exits_for_respawn`) and
  `runner::CorpusGuard` claims the canonical case directory across both oracle captures and the port
  run (`corpus_guard_serializes_two_threads_in_one_case_directory`; the concurrent producer was a
  sibling `#[test]`, not the scheduler). Sharing that one classification on every platform ungates
  `dss_epri::guard` (pure `std::fs`) and deletes the runner's `cfg(not(windows))` twin, so
  `crates/dss-core/Cargo.toml` moves `dss-epri` from the Windows-only dev-dependencies to plain
  `[dev-dependencies]` (D33(3), accepted by D35(2)). That claim **cost nothing**: the gate's wall time went
  221.7 s to 188.6 / 186.8 / 165.8 s over three post-change default-lane drives
  (−33.1 / −34.9 / −55.9 s) and 174.0 s on parity. Everything else: `TESTING.md` §"G1.10a — the
  created-file SET" (the surface, the fold, the population, the leak rule, the per-directory claim,
  the per-RUN capture-order rule, rider C3), `tests/TOLERANCE_NOTES.md` §G1.10a, the plan's §G1.10
  as-executed note, and the name registry
  `oracle_parity_cfg_gate::the_g1_10_pins_the_docs_cite_exist_exactly_once`. The self-check stage
  added the one §4.3 deliverable the micro-parts had left as prose —
  `guard::tests::the_python_twin_shares_this_fixture_and_passes_its_self_test`, which compares the
  two guards' `SELF_TEST_*`/`const` fixture lists and RUNS the Python twin's self-test inside
  `cargo test` (drift drive: a one-name edit to `corpus_guard.py` reds it).
  **D35** settled the one gate red the sub-step produced: F4a's four `for … in 1..=` loops building
  the trace header pushed DE_PASCALIZE **P14** to 110 over its ceiling 106, so all four were
  rewritten 0-based with the `+ 1` written at the 1-based user-API label — the ceiling was **not**
  nudged (106, and the population now sits exactly on it) and the rendered header is byte-identical
  (`storage_debugtrace_opens_the_trace_file_at_edit_time`); the same commit hands the two
  cross-transport `#[test]`s (`corpus_gate.rs:821`, `:984`), the last producers running in a case
  directory without a claim, the same `CorpusGuard`, after which F4f measured the intermittent
  single-case capi red absent in four consecutive default-lane drives — it did recur once later, on
  a loaded machine (see the gate note below).
  Commits `11386d96` (F4a, Storage `DebugTrace`), `48af5a74` (D35), `9029ec42` (F0, bridge
  editor), `8a6f2e73` (F2a, in-memory event log), `a6d7f1fd` (the surface), `728332b6` (the audit
  settlement below) + docs. Gate at step 2, both lanes: `cargo fmt --all --check` rc 0,
  clippy clean, **8 366 passed / 0 failed / 5 ignored** per lane over 81 binaries;
  `corpus_gate` **308 / 0 failed / 0 ignored** per lane with **526/526** cases green
  (218.9 s default, 173.8 s parity), ledger **55** entries / **1 566** hits / 0 stale and no unhit
  entry, `SCRATCH_FILE_DECLINES` `(9, 9)` identical on both lanes, `run_files_pins` **238 / 0**,
  `capture_order` **34 / 0**, `oracle_parity_cfg_gate` **21 / 0**, `population_lock` **3 / 0**,
  `golden_lock` **4 / 0** with `tests/golden/**` untouched. Two earlier parity drives, taken while
  three other lanes were building on the same machine (CPU pinned at 100 %), each carried one
  `capi_v0145` `oracle timeout after 120s` on a `ckt24` deck, and the second also the single-case
  `espvlcontrol` "You must create a new circuit object first" the D35(3) fix was measured against;
  both cases pass scoped on the same tree and the quiet third drive is the 526/526 above — an
  environment artifact recorded, not a divergence, and the `espvlcontrol` recurrence is carried to
  the settle stage as an open flake. Hygiene: the drives left 2 + 19 `Test/AutoTrans/*.txt`
  droppings of the STATUS-tracked `kind=large*` leak (no run-file probe brackets those cases), and the
  self-check's three scoped drives 9 more (`Auto1bus_*`, same mechanism) —
  removed by name, `git status --short tests/corpus` clean. `lane_diff`, owed by F4a's writer, was RUN at
  the gate stage and passed: max |Δ| = **0.000e0** on all eight gated kinds (errs 522, iter 2 165, loss
  366 496, pow 1 170 182, v 375 842, y 1 738 268; 0 iteration counts drifted), and again after the
  settlement below.
  **Audit settlement** (2026-09-06, `728332b6`) — 15 rows (11 distinct): **9 fixed / 5 recorded / 1 refuted**
  (`tmp/g110a/settle.md`). Fixed: the two rails that could pass vacuously (the D30(1) protocol test
  `expect`s its `run_files` key; `CaseResult::sweep_failed` is an `Option` behind the presence rail,
  negative-driven by `a_transport_reply_without_a_sweep_report_fails_the_case` and
  `a_transport_reporting_a_leaked_dropping_fails_the_case`), the outer guard's swallowed removal
  errors (`runner::CorpusGuard::sweep_created` returns its survivors and `Drop` names them —
  `the_outer_guard_reports_a_created_file_it_cannot_remove`; the D32(2) loudness now reaches all
  three producers), a **port gap** in F4a's writer (`exec/json_import.rs` never drained
  `open_debug_traces`, while dss_capi opens the file from `PropertySideEffects`
  (`src/PCElements/Storage.pas:765` → `:868-885`) on every write path, the JSON reader included —
  `storage_debugtrace_survives_a_json_model_round_trip`, proven by a negative drive), and two
  mis-pointed citations (`DIVERGENCES.md`'s `ExecHelper.pas:4071` is a `FireOffEditor` in `DoSave`;
  the real chain is `:3672` → `Plot/DSSPlot.pas:3642`/`:3746`/`:3758` → `Plot/DSSGraph.pas:109`/
  `:114-115`/`:125`/`:128`, and `TOLERANCE_NOTES.md`'s `runner.rs:733,508`). The one **major**
  finding (D35(3)'s STOP clause, after the `espvlcontrol` recurrence) is settled with a mechanism
  rather than another drive: a `compile` that leaves neither a circuit nor an error can only be a
  SHORT READ of the master file (`exec/solve.rs::do_redirect` errors loudly on a missing or
  unreadable one), so `run_rust_capture` asserts the circuit exists and prints the deck's size on
  disk — the next occurrence diagnoses itself; the flake stays OPEN in STATUS, no retry loop, no
  case re-driven. Recorded, measured and deliberately unchanged: shape (i)'s lost sweep under a
  pre-existing subdirectory (D30(2); escape hatch = shape (ii)), r4133's `InShowResults` suppression
  (`PCElements/Storage.pas:2407`) still unported and handed to G1.10b with the contents surface, the
  engine-scratch SUFFIX match (symmetric on both sides and counted by `SCRATCH_FILE_DECLINES`, so an
  unforeseen name is a population move, never a hidden decline), and this block's length. Refuted:
  the handoff's liveness decomposition (763 is right; the terms are −5 `skip` pairs +11 fixtures,
  and no landed doc states them).
  **And the settle stage's own gate found the root cause of the STATUS-tracked `Test/AutoTrans/*`
  residue** (it red `run_files_pins::the_two_oracle_spellings_of_auto1bus_fold_to_one_member`, 0 names
  instead of 9, with no `sweep_failed` anywhere): `Test/` holds 36 manifest cases and
  `Test/AutoTrans/` five, two different claim keys running concurrently by design, and the parent's
  guard photographed the CHILD directory too — so when the child's own guard swept its export, the
  parent's `restore` found the file missing and wrote it back. Both Rust guards now leave a GONE
  entry gone (an overwritten one is still restored; the Python twin already read before it wrote),
  proven by `a_parent_guard_does_not_resurrect_a_sibling_cases_swept_output` (deterministic
  interleaving, red before the fix) and measured end to end: 9 leaked files before, **0** after over
  the settlement's full drives.
  Gate after the settlement, both lanes: `cargo fmt --all --check` rc 0, clippy clean,
  **8 371 passed / 0 failed / 5 ignored** per lane over 81 binaries, corpus **526/526** cases,
  ledger **55** entries / **1 566** hits / 0 unhit, `SCRATCH_FILE_DECLINES (9, 9)` and 763 set
  compares identical in both lanes, `golden.lock.json` and `tests/golden/**` untouched, `lane_diff`
  **PASS** (max |Δ| = 0.000e0 on all eight kinds, run after the `json_import` fix), and **zero**
  untracked corpus droppings after four consecutive full drives — the residue that reds
  `run_files_pins` when it survives.
  **Merged into `update`** (2026-09-06; this sub-step's seven commits and G1.8's five, which had not
  landed yet). Conflicts settled by keeping both surfaces: `LEDGER_FIELDS` is the union **17**
  (`distance` + `run_files`; `EXCLUSION_ONLY_FIELDS` 8, `EXCLUSION_FIELDS` 13,
  `PER_VALUE_EXCLUSION_FIELDS` 4), `ledger.json` the union **57** entries / **33** causes, both pin
  registries (`the_g1_4b_pins…`, `the_g1_10_pins…`) and both record blocks kept, and the **109**
  `file:LINE` citations the merge shifted re-pointed (diff-mapped, `operational_docs_line_citations…`
  green). Merged-tree checks: `fmt` + clippy clean in both lanes, corpus **526/526** cases / 0 failed,
  ledger **57** entries / **1 573** hits / 0 stale, `SCRATCH_FILE_DECLINES (9, 9)` over 763 set
  compares, `INC_UPSTREAM_ROW_DECLINES (4, 5)`, D15 `(16, 135)` / D16 `(8, 96)`, sequence-arm census
  `(297 867, 78, 4)`, `DISTANCE_POPULATION (867, 79 137)`, short-circuit `(10, 646)`,
  `population.lock.json` regenerated (12 cells: six `incm=1`, six `runf=1`, one `ledger=` on
  `Test/YgD-Test.dss`), `golden.lock.json` and `tests/golden/**` untouched. The first merged-tree drive
  measured `(8, 8)`: two untracked, **gitignored** `*_SavedVoltages.dbl` droppings in the main tree
  (`Examples/HarmonicsVariableLoad`, `IEEETestCases/13Bus` — the latter from 2026-09-04) read as
  pre-existing and suppressed a decline; deleted by name, the re-drive measured the pinned `(9, 9)`
  and left zero droppings. Frontier (USER WIND-DOWN): `lane-b` G1.4d lands last; F0′ (**D39**),
  G1.10b (STOPPED at spec), G1.10c, the G1.11′ docs close-out and WP-G3 open the next session.

  merge: lane lane-s -> update, see git log

  **+ F0′** (2026-09-11, lane `lane-m`, **D39**/**D41** — a follow-up commit, not a sub-step; sha
  `6a987289`) — the bridge gags report auto-display with r4133's own switches (`Set AllowForms=No`, then
  `Set ShowReports=No`/`Set ShowExport=No` behind a throwaway circuit, options 138/71 answering `#301`
  without one, `Executive/ExecOptions.pas:645-649`), leaving D25's editor no-op only as the safety net for
  the **12** of **55** `FireOffEditor` sites upstream leaves unguarded (`to_opendss/73`). No report is
  suppressed — only the viewer launch (`Common/ShowResults.pas:401-403`): 56 = 56 created entries over 14
  report decks, so **0** ledger rows, 0 golden bytes, no lock cell, `lane_diff` not owed (`dss-epri` only).
  Layers in `TESTING.md` + `tools/opendss/README.md`; pins
  `report_switches_survive_a_compile_and_gag_every_guarded_editor_site`,
  `the_editor_safety_net_covers_the_sites_no_switch_guards` (`G1_10_PINS`). Gate: five commands green in
  BOTH lanes, `corpus_gate` 526/526, ledger 57 entries / 0 unhit.

  **Audit settlement** (`<SHA2>`): 13 findings — **10 fixed / 3 recorded / 0 refuted**. The one with teeth:
  `ShowExport` is a unit global five live corpus decks set themselves, so it leaked into every later case of
  a pooled worker — `Engine::clear` now re-asserts both switches per case (the D13 shape; pin
  `clear_re_asserts_the_report_switches`, `Yes`/`Yes` measured before the fix). Also fixed: the created-file
  set is asserted whole (was `len() >= 8`); the safety-net `Dump` runs the editor `Engine::new` installed so
  its notepad tripwire can fire, both process diffs attributed by command line (`command_lines_reads_this_process`);
  the corpus deck counts, three stale `dss.rs` citations and the D41 record/plan/STATUS shape. Recorded:
  layer 1 cannot be falsified in-worker (`DSSI(8, 0)` precedes it — measured, documented); no probe fires the
  OS editor (**D38**); `LINE_CITED_DOCS` stays out of the plan/record until G5.1 (**D37(10)**).

- **G1.4d** (2026-09-06, lane `lane-b`, bus chain — **D7**, split out of G1.4b by **D26**; commits
  `62c616eb` the two r4133 mode rows `Pure` → **Impure**, `1acc1f53` the surface, `dff755b5` the
  settlement, + docs) — the bus **at-bus lists** (`Bus.AllPCEatBus`/`AllPDEatBus`) live on both channels
  on G1.4a's per-bus walk. The port answers **S4**, neither oracle's criterion (r4133
  `Common/Circuit.pas:1513`/`:1559`, capi `:1746-1767`): `Dss::all_bus_elements` publishes the answer AND
  the raw terminal facts, over which `harness::compare_bus_at_bus` replays each channel's own walk
  (**D15**/**D16**/**D21**) and COUNTS the residue into four fail-on-stale populations — **(279, 279)** /
  **(18, 147)** / **(8, 11)** / **(0, 0)**. D26's premise that capi drops disabled elements is refuted by
  measurement (15 of 223 (disabled element, own bus) pairs ARE listed), which is what makes that channel
  an equality. **0** ledger rows (55 / 31 causes), no flag, no lock cell, no golden byte; pins in
  `G1_4D_PINS`, details in `TESTING.md` §"The bus at-bus surface" and `to_opendss/` 69-71. Both lanes
  **7 892 / 0 / 5 ignored** over 79 binaries, corpus **526/526**, ledger **55** / **1 567** hits / 0
  stale, `lane_diff` **PASS** max |Δ| = 0.

  **Audit settlement** (2026-09-06, `dff755b5`; 13 findings, 12 distinct — 7 fixed / 5 recorded / 0 refuted): the
  comparator gained the one direction no channel assertion can state (`assert_port_at_bus_is_s4` — each
  oracle's walk is a projection of the same facts, so a silently shortened port list vanished from both
  sides of it; armed in a scratch copy it now reds 3/3 cases, where the same corruption left F4's full run
  526/526 green), an offline drive for capi's node-less fallback arm (its reply re-measured on a fixture of
  its own), one drive per remaining population direction, and D34's owner for `ORPHANED_GAPS.md` §1.21
  (scheduled at the plan's §G5.2). Recorded: r4133's terminal-1/2 shunt filter and capi's terminal-window
  arm (corpus exposure re-measured **0** for both), the deliberate order-insensitivity, the
  prose-vs-constant check **D37(1)** defers to G1.11′, and two spec claims the landed code refutes.
  Test-only change (no product file moved, so no `lane_diff` owed): both lanes **7 940 / 0 / 5 ignored**
  over 79 binaries, corpus **526/526**, the four populations and the ledger (55 / 1567 hits) unmoved.

  Merged-tree checks (2026-09-06 landing into `update`, which had gained G1.3c, G1.8 and G1.10a since the
  base): `fmt` + clippy clean in both lanes, `oracle_parity_cfg_gate` **25/25** (every pin registry from
  G1.7/G1.8/G1.9/G1.4b/G1.10/G1.4d plus the citation walker), `population_lock` 37 / `golden_lock` 4 /
  `capture_order` 3, and the full default-lane `corpus_gate` **452 passed / 0 failed**, **526/526** cases in
  175.5 s. Nothing of G1.4d moved on the bigger population: at-bus **(279, 279) / (18, 147) / (8, 11) /
  (0, 0)** exactly as the lane measured, `WP_G1_MODES` **111** with **20** `ModeEffect::Impure` rows (the
  census the lane corrected, re-derived here over the eleven rows G1.8/G1.10a added), ledger **57** entries
  / 33 causes / **1 573** hits / 0 unhit / 0 stale (G1.4d adds none, so the union is `update`'s), and the
  neighbouring populations all re-derived unchanged — sequence-arm `(297 867, 78, 4)`, distance
  `(867, 79 137)`, D15 `(16, 135)` / D16 `(8, 96)`, S-INC `(4, 5)`, `SCRATCH_FILE_DECLINES (9, 9)`,
  short-circuit `(10, 646)`, seq/VLL `(10, 129)`/`(4, 54)`/`(16, 196)`/`(2, 12)`. `population.lock.json`
  regenerated with an empty diff (the surface sets no manifest flag); `golden.lock.json` and `tests/golden/`
  untouched. Two documents met at the merge: `ORPHANED_GAPS.md` §1.20 was already taken by G1.6(i)'s
  zone-boundary deferral, so G1.4d's executive-command entry landed as **§1.21** and its two citations (the
  plan's §G5.2 duty line and this record) were re-pointed; the 43 `file.rs:LINE` citations the merge shifted
  were re-pointed by diff-mapping and re-checked by
  `operational_docs_line_citations_point_at_the_line_they_name`.

  merge: lane lane-b -> update, see git log
