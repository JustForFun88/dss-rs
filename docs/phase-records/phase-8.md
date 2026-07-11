# Phase 8 record (`PHASE8_PLAN.md`) — 🚧 IN PROGRESS

> Archived from `STATUS.md §1f` (2026-07-03) to keep the live handoff lean.
> This is the full per-WP/per-step log for the **completed** Phase-8 work
> (WP8.1, WP8.2, and the landed WP8.3 steps). The live `STATUS.md §1f` keeps
> only the compact frontier + the current step. The code and tests are the
> contract. Phase 8 lives on branch `phase-8-reporting` (branched from the
> gate-green Phase-7 tip; Phase 7 is NOT merged to `main`).

Execution plan: **`PHASE8_PLAN.md`** (WP8.1–WP8.8, the reporting/output + full
executive layer). Per-step cadence = the full ritual in `PHASE8_PLAN.md §0` (gate
→ STATUS + commit → `/audit-code` + `/audit-tests` as independent agents → fix +
commit → STATUS sync → stop). Phase 8 is almost entirely *read-and-format*: no
new electrical math, no new solve mode — the risk is faithful report layout and
**not silently faking output**.


Execution plan: **`PHASE8_PLAN.md`** (WP8.1–WP8.8, the reporting/output + full
executive layer). Per-step cadence = the full ritual in `PHASE8_PLAN.md §0` (gate
→ STATUS + commit → `/audit-code` + `/audit-tests` as independent agents → fix +
commit → STATUS sync → stop). Phase 8 is almost entirely *read-and-format*: no
new electrical math, no new solve mode — the risk is faithful report layout and
**not silently faking output**.

**WP8.1 (Report infrastructure) — ✅ COMPLETE, gate-green (sub-steps 1+2).**
- **sub-step 1 — dispatch skeleton + GUI no-ops — done, gate-green (`82b50fe`).**
  The new top-level **`crate::report`** module (`report/mod.rs`): the
  `EXPORT_OPTIONS` (57, ADIAKOPTICS-off → `High=Laplacian`; confirmed: the pinned
  build defines `DSS_CAPI_ADIAKOPTICS_DISABLED`, `common-release.cfg:6`) and
  `SHOW_OPTIONS` (34) name tables in exact `TExportOption`/`TShowOption` ordinal
  order, each built into a `CommandList` (abbreviation-matched like the oracle's
  `ExportCommands`/`ShowCommands`). The routers live in **`exec/report.rs`**
  (`impl Dss`, like every other command router — they drive the private
  parser/circuit/error state and delegate formatting to `crate::report`):
  `do_export_cmd` (keyword match → unknown-keyword #24713 → scoped `NOT_PORTED`
  per keyword), `do_show_cmd` (keyword match → faithful **silent** no-op),
  `do_save_cmd`/`do_dump_cmd` (scoped `NOT_PORTED`). `command.rs` routes
  `Export`/`Save`/`Dump`/`Show` and the `Plot`/`Visualize` **headless no-op** in
  the **post-circuit** dispatch. No real report formatting yet (WP8.2–8.5).
  - **The Show-silent vs Export/Save/Dump-loud asymmetry is forced + correct.**
    The always-on live gate (`corpus_live.rs`) asserts `errors().is_empty()`, and
    **44 `solvable_now` decks** run `Show Power`/`Show Voltage`/`Show f` (all
    valid `TShowOption` keywords). So the unported-`Show` path is a **silent**
    no-op — faithful per §2.5 (a `Show` changes no electrical state; WP8.4 lands
    the real formatters + the 24700/24701/24702/999 errors + a targeted text
    golden, which is what proves it non-fake). `Export`/`Save`/`Dump` decks are
    all in `skipped_unsupported` (never in the live gate), so they **loudly**
    record a scoped `NOT_PORTED` — a half-ported `Export` never silently emits
    nothing (the plan's §WP8.1-step-1 intent). Audit-code swept the **whole**
    corpus (incl. redirected sub-files): the 21 distinct `Show` keywords are all
    valid — no `panel`, no unknown — so deferring those errors to WP8.4 is proven
    safe.
  - **audit-code (independent agent): one MAJOR, fixed.** `Plot`/`Visualize` were
    first placed in the *pre-circuit* no-op arm, which swallowed the oracle's #301
    "create a circuit first" guard — the oracle's dispatch gate errors #301 for
    `plot`/`visualize`/`show` *before* a circuit exists (oracle-probed:
    dss-python 0.15.7). **Fixed** by moving `Plot`/`Visualize`/`Show` to the
    **post-circuit** dispatch, so before a circuit they fall through to the
    generic #301 guard (matching the oracle) and after a circuit they are clean
    no-ops; the `report_verbs_before_circuit_error_301` test now pins the #301.
    Verified-correct: all option names/ordinals, the `cmd` ordinals
    (SAVE=7/PLOT=12/DUMP=16/EXPORT=34/VISUALIZE=73), `do_export_cmd`'s #24713
    message + the 1-based `ParamPointer` convention, and the module split.
  - **audit-tests (independent agent): sound + non-vacuous** (mutation-verified —
    forcing `do_show_cmd` to error fails the silent-no-op test; forcing
    `do_export_cmd` to no-op fails the loud-stub test). Strengthened: pin the
    *quoted* `"Voltages"` (not a substring that 4 siblings share), assert the
    plot snapshot is non-empty, pin the full #24713 message.
  - **Tracked-deferred (not bugs):** the `Export` circuit/solution gates
    (#24711/#24712) land with the real exporters in WP8.2; the `Show`
    panel/unknown/solution errors (999/24700/24701/24702) + real formatters in
    WP8.4; `Visualize` on an *unsolved* circuit errors #24722 on the oracle —
    not reproduced (no corpus deck reaches it; Visualize is a §2.5 no-op).
    Out-of-scope (pre-existing): `EXEC_COMMANDS` is missing `Abort`(124)/`Clone`
    (125) from the pinned PM build (no effect on any current ordinal/abbreviation;
    future corpus-hygiene pass). lib stays **713**; `solvable_now` **88**.
- **sub-step 2 — output-path machinery + `Export Counts` end-to-end + the CSV/text
  golden harness — done, gate-green (`929145c`).** Lands the first **real report**
  through the whole output path:
  - `report/output.rs` — `export_path` (Pascal `DoExportCmd`: explicit filename wins
    verbatim, else `<OutputDirectory><CircuitName_><default>` where `CircuitName_ =
    <CaseName>_`). `report/export/counts.rs` — `ExportCounts` (Pascal
    `ExportResults.pas:2965`): the `Format: DSS Class Name = Instance Count` text
    dump of every class + instance count, in registration order.
  - **Output-dir state on `Dss`:** `output_directory` (Pascal `OutputDirectory`,
    only `Set DataPath=` moves it — not `Compile`/`Redirect`, which move only
    `current_dir`) + `last_result_file` (`SetLastResultFile`, exposed via
    `Dss::last_result_file()` so the golden harness reads the produced file).
    **`Set DataPath=` wired** (`apply_data_path`, Pascal `SetDataPath`: create-dir +
    #907, both with and without a circuit — a common top-of-script pattern; the
    non-writable→scratch fallback is NOT_PORTED). `do_export_cmd` Counts(26) →
    write the file + `@lastexportfile`.
  - **The new golden harness `compare_export`** (PHASE8_PLAN §2.3, in `tests/harness`):
    tokenizes both files by the report separator, matches a fixed header block
    verbatim, then compares each data row field-by-field — numbers within tolerance
    (reusing `assert_value_matches_tol`), identifiers case-insensitively. Two row
    policies: `ExactOrdered` (the contract for most exports) and `RustSubsetByKey`
    (Counts: the Rust class registry is a **proven proper subset** of the oracle's,
    so every ported class's count is pinned exactly while the not-yet-registered
    oracle classes are ignored — documented in `tests/TOLERANCE_NOTES.md`, **not** a
    blanket relaxation; tightens to `ExactOrdered` once the registry is complete).
  - **Gate:** `gen_phase8.py` captures the oracle's `Export Counts` →
    `tests/golden/phase8/export_counts.{txt,meta.json}` (the meta carries the deck so
    the Rust + oracle fixtures can't drift); `golden_phase8.rs` replays the deck,
    exports to a process-unique temp dir, and subset-compares vs the oracle (the
    default-item counts `TCC_Curve=10`/`Spectrum=7` + the fixture `Line=2`/`Load=1`
    pinned). Self-test green. lib stays **713** (713→**714** with the new
    `counts.rs` unit test); golden_phase8 **1**; `solvable_now` 88.
  - **Micro-deviations from the plan (documented):** (1) the `csv` crate is **not**
    added yet — `Counts` is `=`-separated text, not CSV; `csv` lands in WP8.2 with the
    first real CSV export (avoids an unused dep). (2) `report/format.rs` is **not**
    created yet — `Counts` needs no float formatting; `format.rs` lands in WP8.2 when
    the first numeric export needs it. (3) the test uses a process-unique
    `std::env::temp_dir()` subdir, **not** the `tempfile` dev-dep (dependency-free;
    `tempfile` can be adopted later if isolation needs grow).
  - **audit-tests follow-up (one MAJOR, fixed).** `RowPolicy::RustSubsetByKey`
    iterated only the Rust rows, so a *dropped* class / empty report body passed
    silently (mutation-proven: header-only + missing-`Line`/`Load` bodies both
    passed) — a "report bug that hides" (PHASE8_PLAN §1). **Fixed:** the policy now
    takes a `require` key set (the deck-created + default-item classes —
    line/load/vsource/tcc_curve/spectrum/loadshape/growthshape) asserted present in
    the Rust output. The auditor confirmed the baseline is a genuine pinned-oracle
    capture (not self-generated), the deck is single-sourced via `meta.json`, and a
    wrong count / extra Rust class / bad header all fail correctly. Minor: the
    "tightens to `ExactOrdered`" claim also needs the Rust registration order
    reconciled to the oracle `DSSClassList` (they differ) — TOLERANCE_NOTES updated.
  - **audit-code follow-up (no Critical/Major; 2 Minors fixed).** Faithful port of
    `ExportCounts`/the path resolution/`SetDataPath`/`SetLastResultFile` confirmed
    (incl. `objects.len() == ElementList.Count`, the `casename_` filename prefix,
    no-circuit `DataPath` allowance, write-failure surfaced not swallowed). **Fixed:**
    (1) `apply_data_path` used `create_dir_all` (creates missing parents) vs Pascal's
    single-level `CreateDir` (#907 if a parent is missing) → switched to `create_dir`;
    (2) `write_report` baked in the **Export-specific** `@lastexportfile` — moved to
    the export router so reusing the generic writer for Show/Save (WP8.4/8.5) won't
    wrongly set it (Show sets neither; Save sets `@lastfile` + `GlobalResult`).
    Surfaced-not-fixed (latent, documented): the trailing-filename read is hoisted
    before the per-report pre-parse — a `TODO(WP8.2)` now warns that reports
    8/9/15/17/… must move their `Parm2` parse ahead of it. Nits (no fix, no corpus
    path): relative explicit filename resolves vs process cwd; no-circuit Counts is
    graceful where the oracle AVs; the scratch/empty-`DataPath` omissions are
    NOT_PORTED.
**WP8.2 (Export: solution outputs) — 🚧 IN PROGRESS.**
- **sub-step 1 — bus/node solution exports — done, gate-green (`71067f7`).**
  The first **real solution reports**, read-only over the solved circuit
  (PHASE8_PLAN §2.1, plan-step 1):
  - **`report/format.rs`** — the shared Pascal number formatters: `g(v, sig)`
    (`Format('%…g')`, delegating to the existing `util::fmt_g`) and `fixed(v,
    decimals)` (`Format('%…f')`). The width/justification prefixes (`%10.6g`,
    `%-13.11g`) only space-pad, which the comparator trims, so they're dropped.
  - **Four exports** in `report/export/` (one file each, loop-faithful to
    `ExportResults.pas`): `export_voltages` (`ExportVoltages:277` — per-bus
    node mag/angle/pu, ascending-node-number scan via `FindIdx`, zero-filled to
    MaxNumNodes), `export_bus_coords` (`ExportBusCoords:3064` — `CheckForBlanks`
    + `%-13.11g`, no header), `export_node_names` (`ExportNodeNames:3699` —
    `BusName.NodeNum`, original-case → lowercase since the HashList is lowercase;
    gate compares identifiers case-insensitively), `export_ynode_list`
    (`ExportYNodeList:3784` — node names in Y-order via `MapNodeToBus`, quoted
    uppercase).
  - **`Export` solution guard (#24712)** wired into `do_export_cmd`
    (`ExportOptions.pas:163-177`): the solution-reading exports (`ptr` ∈
    `1..24, 28..32, 35, 46..51`) need `Solution.NodeV` allocated (`node_v.len() >
    1`), else "The circuit must be solved before you can do this." The **no-circuit**
    half (#24711) is **unreachable** — `ProcessCommand`'s generic pre-circuit guard
    (#301, `command.rs`) fires first (Export is not in the OK-before-circuit list,
    `ExecCommands.pas:364`; the message matches the oracle's #301 verbatim). The
    router's `write_export` helper resolves the path + writes + sets the
    export-specific `@lastexportfile` (shared by Counts).
  - **Harness `ColTol`** (per-column tolerance, `tests/harness`): `ExportPolicy`
    gains `col_tol`, matched against the header column names. Needed because the
    `Angle%d` columns are `%6.1f` (one decimal) — two independent solves round
    that last 0.1 digit independently (a ±0.1 *formatting* floor), while every
    `%g` magnitude/pu column keeps the tight default. **Not** a blanket
    relaxation; documented in `tests/TOLERANCE_NOTES.md`. The voltage golden is a
    **report-layout** gate (header/columns/order/scaling), not the physics gate —
    voltages stay pinned to 1e-8 by the live `corpus_live` model compare.
  - **Gate:** `gen_phase8.py` compiles + solves the unmodified IEEE13 master
    (`tests/corpus`, single-sourced via each report's `meta.json`) and captures
    the oracle's four reports → `tests/golden/phase8/export_{voltages,buscoords,
    nodenames,ynodelist}.{txt,meta.json}`; `golden_phase8.rs` replays the same
    master + `solve`, exports into a scratch `datapath`, and diffs via
    `compare_export` (`ExactOrdered`). golden_phase8 **1→5**; lib unit-test count
    unchanged (the formatters are gated end-to-end, no new inline tests). The WP8.1
    `export_records_scoped_not_ported` unit test updated: `Voltages` is now ported,
    so it pins the #24712 guard + a still-unported `elem`→ElemCurrents NOT_PORTED.
  - **Micro-deviation (documented):** the `csv` crate is still **not** added — all
    four reports are bespoke layouts (`ExportVoltages`'s zero-fill / `%g` columns,
    the headerless coord/node lists) that `Writeln`-style `String` building
    expresses directly; `csv` lands if/when a plain RFC-4180 table export makes it
    pay (re-evaluate at the element exports). `report/format.rs` now exists.
  - **audit-code (independent agent): faithful — no Critical/Major/Minor, 2 doc
    nits fixed.** Empirically reconfirmed against the live oracle: unsolved
    `export voltages` → #24712 / no-circuit → #301 (so #24711 IS unreachable); the
    ptr-set `1..=24|28..=32|35|46..=51` reproduces the per-keyword solve
    requirement (buscoords/ynodelist need a solve, nodenames doesn't); the default
    filenames + `<case>_` prefix are byte-exact; a {2,3}-node bus emits the trailing
    zero-fill group; NodeNames is lowercase+trailing-space byte-identical. **Fixed:**
    the `node_names.rs` doc (the HashList lowercases on store, so the oracle *also*
    emits lowercase — byte-identical, not merely case-insensitively equal) and the
    `export_with` doc (circuit presence comes from the #301 dispatch gate, which
    also covers ptr 39/NodeNames that skips the solution guard).
  - **audit-tests (independent agent): genuine + non-vacuous — all 6 mutations +
    the guard caught.** Mutation-verified FAILs: dropped zero-fill, Angle/pu swap
    (the loose angle tol does **not** rescue it), kV-vs-V scaling, reordered buses,
    empty/header-only body, dropped node loop; and disabling the #24712 guard fails
    the unit test. Confirmed the goldens are real pinned-oracle captures (`check_pin`
    0.15.7/0.14.5), single-sourced via `meta.json`, and that `corpus_live` gates
    IEEE13 voltages at 1e-8 so the export golden is legitimately a layout gate.
    **Fixed (Minor 1):** the `Angle` override was `rel 1e-3`/`abs 0.11` — but the
    `%6.1f` floor is purely *additive*, and the angle is `arg(V)` (independent of
    |V|/pu, so the "pinned by magnitude+pu" rationale was wrong). Tightened to **rel
    0 / abs 0.11** (the exact proven printing floor) with the rationale corrected in
    `golden_phase8.rs` + the `ColTol` doc + `TOLERANCE_NOTES.md`. **Nit 2:** the
    "primary gate is corpus_live" wording now states precisely that corpus_live
    gates the *engine* voltage physics (daily mode) while the export golden gates
    the *snapshot report-layer transform*. **Nit 3:** the unit test now sets a
    scratch `datapath` (defensive — every branch errors before a write, but a future
    write-reaching branch must not pollute the source tree).
- **sub-step 2a — the aggregate PD/PC power exports — done, gate-green** (`668bd18` +
  audit follow-ups `1fda0a6`/`91a091c`). The first
  **element** exports (read/compute over the solved circuit, PHASE8_PLAN §2.1 plan-step
  2): `Powers` (`ExportPowers:1075` — per-terminal kW/kvar of every PD then PC element
  + each PD's terminal-1 normal/emergency excess kVA), `Losses` (`ExportLosses:1185` —
  per-PD total/load/no-load losses in W/var via `GetLosses`), `P_byphase`
  (`ExportPbyphase:1224` — per-conductor kW/kvar over the full `Yorder`, **no**
  positive-seq `×3`, formed directly from `ComputeVterminal`/`ComputeIterminal`).
  - **The mutable element-walk infrastructure** (the defining new piece): the element
    exports call the mutating terminal getters (`Power`/`GetLosses`/`ComputeIterminal`/
    `ComputeVterminal`), so they can't use the read-only `fn(&Circuit)` formatter shape.
    New `report::export::for_each_enabled_elem(&mut [DssClass], &[ElemRef], f)` walks a
    circuit element list under the `if Enabled` guard, and `Dss::export_with_mut` hands
    the formatter the disjoint `(&mut classes, &circuit, &sys, &node_v)` borrow (the
    `snapshot_elements` pattern). `exec::registry` is now `pub(crate) mod` so the report
    formatters can name `DssClass`. The element formatters are `pub(crate)` (they expose
    the `pub(crate)` `DssClass`).
  - **The `Powers`/`P_byphase` MVA/kVA `Parm2` pre-parse** (`ExportOptions.pas:190-199`):
    `do_export_cmd` now consumes the `m…`→MVA flag for ptr 9/19 **before** the trailing
    filename, fixing the WP8.1 `TODO(WP8.2)` hoist (the other Parm2-consuming reports
    8/15/17/20-21/32/51 land in later WPs). The dispatch arms 9/19/24 route through
    `export_with_mut`.
  - **`ElemPowers` deferred to sub-step 2c** (the `WriteElem*` family). An oracle probe
    found its Vsource power is an **intrinsic `WriteElemPowers` artifact**: `Export
    ElemPowers` writes the Vsource `−612.936` even in complete isolation, ≠ the oracle's
    own `CktElement.Powers` (`−612.729`, which `corpus_live` already pins and the Rust
    matches) — `WriteElemPowers`'s `ComputeVterminal; ComputeIterminal` sequence
    re-derives the source current differently from the canonical terminal power.
    `ElemCurrents` (probe-confirmed) does **not** have the quirk (it matches canonical),
    so the three `Elem*` reports go together in 2c where the source-element semantics can
    be settled (port-the-bug vs canonical, the WP7.6 / VSConverter precedent).
  - **Gate:** `gen_phase8.py` captures the oracle's `Powers`/`Losses`/`P_byphase` on
    solved IEEE13 → `tests/golden/phase8/export_{powers,losses,p_byphase}.{txt,meta.json}`;
    `golden_phase8.rs` replays + diffs via `compare_export` (`ExactOrdered`). All real
    columns (kW/kvar/W) — no angle/sequence — so the floors are the plain printing floors
    (`Powers` `%11.1f` → `rel 0`/`abs 0.11`; `P_byphase` `%10.3f` → `abs 0.0011`; `Losses`
    `%.7g` → the default `%g` floor), documented in `tests/TOLERANCE_NOTES.md`, **not** a
    physics relaxation (the engine V/I/P is pinned to 1e-8 by `corpus_live`). golden_phase8
    **5→8**; lib stays **719** (formatters gated end-to-end, no new inline tests);
    `solvable_now` **88** (no migration — the `Export` decks need the full export set; the
    migration lands at the WP8.2 completion gate).
  - **audit-code follow-up (independent agent): faithful — no Critical/Major, 1 nit fixed,
    1 coverage gap → audit-tests.** Verified field-by-field vs Pascal: the headers (incl. the
    double spaces), the PD-then-PC iteration order, the excess-kVA terminal-1 gating, the
    MVA/kVA `Parm2` pre-parse (incl. the Pascal non-`m`-token-swallowed-as-Parm2 quirk), and
    the highest-risk **`Power[j]`-`×3` vs `P_byphase`-no-`×3`** distinction — all correct. The
    Vsource omission from the Powers PC section is probe-confirmed correct (`ElemKind::Source`
    files into `sources`, never `pc_elements`), and the `ElemPowers` deferral is probe-confirmed
    real. **Fixed (nit):** the formatters uppercased the whole `Class.Name`; Pascal uppercases
    only the element name (`DSSClassName.UPPER(Name)`) — added `format::upper_elem_name` (split
    on the first `.`, uppercase only the suffix) so the raw output is byte-faithful, not merely
    case-insensitively equal. The MVA-path coverage gap is handled in the audit-tests follow-up.
  - **Coverage note (audit-corrected):** `excess_kva_norm` **is** value-exercised — IEEE13's
    XFM1 + lines 650632/632670/670671 carry above-`NormAmps` current, so the `Powers`
    `P_Normal`/`Q_Normal` columns are non-zero (the overload `factor > 0` branch is gated). Only
    the *emergency* twin `excess_kva_emerg` is all-`0.0` (no IEEE13 line exceeds `EmergAmps`);
    it is byte-identical logic to the gated norm branch (different rating field), so it is
    tracked-minor — `Export Overloads`/`Capacity` (WP8.3) value-exercise the emergency rating.
  - **audit-tests follow-up (independent agent): sound + non-vacuous, 2 Majors + 1 Minor
    fixed.** Confirmed the goldens are genuine pinned-oracle captures (`check_pin` 0.15.7/0.14.5,
    `meta.json` single-sources the deck, Rust diffs *its own* produced file), `ExactOrdered`
    enforces row count + per-row field count + PD-then-PC order, and Powers `rel=0/abs=0.11`
    is the correct tight floor (mutation-verified the gate is non-vacuous). **Fixed:**
    (1) **[Major]** `P_byphase` had a superfluous `rel=1e-4` band that mutation-provably masked
    a ~0.005% per-conductor scale drift — the `%10.3f` floor is purely additive (empirical max
    divergence exactly 1e-3, one ULP), so tightened to `rel=0`/`abs=0.0011` (the Powers twin's
    discipline). (2) **[Major]** the MVA `opt=1` path (the `m…` `Parm2` flag → MW/Mvar headers +
    the extra `×0.001`) was wired this WP but untested — **added** `export_powers_mva` +
    `export_p_byphase_mva` goldens (`export powers mva` / `p_byphase mva` on solved IEEE13),
    backstopping the scale + header (a missing `×0.001` prints kW ~1000× larger and fails loudly).
    (3) **[Minor]** `Losses` reused the 6-sig Voltages `EXPORT_REL=1e-4`, ~1000× looser than its
    `%.7g` floor; I **measured** the actual divergence (max 1.53e-7 rel on a substantial loss =
    the 7-sig 1-ULP floor; the only large-rel cells are near-zero noise ≤5.6e-9 W absorbed by
    `abs`) and tightened to a dedicated `LOSSES_REL=1e-6` (≈6× over the proven floor), correcting
    the `TOLERANCE_NOTES.md` rationale (no cancellation floor materializes on IEEE13; if a metered
    feeder later shows one it must be proven by decomposition, not by widening `rel`). golden_phase8
    **8→10**. *audit-tests* also confirmed a second feeder adds only engine-physics variety (already
    gated by `corpus_live`), not report-layout coverage, so IEEE13-only is adequate for §2.3 here.
- **sub-step 2b — the symmetrical-component family — done, gate-green** (`5eb351a`). The
  three sequence exports on solved IEEE13: `SeqVoltages` (`ExportSeqVoltages:177`,
  bus read-only — `Phase2SymComp` over named nodes 1/2/3 + `PctNemaUnbalance` over the
  line-to-line voltages), `SeqCurrents` (`ExportSeqCurrents:431` + `CalcAndWriteSeqCurrents:352`
  — element walk Sources→PD→PC→Faults; the PD pass alone writes the `%Normal`/`%Emergency`
  rating columns on terminal 1), `SeqPowers` (`ExportSeqPowers:1311` — PD then PC, sequence
  powers `S = V012·conj(I012)` printed `S.re·0.003` = per-seq VA→3φ kW, PD rows carry the
  4 excess-kVA columns on terminal 1). Files `report/export/seq_{voltages,currents,powers}.rs`;
  dispatch arms ptr 2/4/10 (`SeqPowers` ptr 10 never pre-parses the MVA flag — `ExportOptions.pas:191`
  traps only 9/19 — so `opt = 0` always; the MVA path is ported-but-unreached). The existing
  `mathutil::pct_nema_unbalance` + `SymComp::default()` (`phase_to_sym`) are reused.
  - **`TODO(compat)` (`seq_currents.rs`):** `Iresidual` sums the *terminal-1* conductors for
    **every** terminal row (Pascal `CalcAndWriteSeqCurrents` indexes `cBuffer^[i]`, not
    `cBuffer^[(j-1)*Ncond+i]`) — an upstream quirk reproduced verbatim; clean fix = per-terminal
    slice.
  - **New harness machinery — `ColTol::gate` (denominator gate).** `SeqCurrents`' ratio columns
    (`%I2/I1`/`%I0/I1`/`%NEMA`) divide by `I1`, which at an open/unloaded terminal is a near-zero
    cancellation residual (~1e-12 A): there I1/I2/I0 are noise (pinned to 0 by the magnitudes'
    `abs`) so the ratio is a faer-vs-KLU **noise/noise** form (`Line.671680.2`: oracle `%I2/I1=147.7`
    vs Rust `61.8`; the live f64 confirms both engines compute I1≈I2≈2e-12 there, terminal 1 matches
    to 6 sig, Iresidual matches exactly — proven cancellation floor, not a port bug). `ColTol::gate`
    skips a ratio cell only where the oracle's denominator is `0 < |I1| < thresh` (the **band-limit**
    lower bound keeps the 32 *exactly*-zero rows' `0==0` checks — Pascal prints those ratios as 0).
    Net on IEEE13: **1** genuine-noise row skipped, **54** rows' ratios still checked; magnitude
    columns checked on every row. A proven cancellation floor (decomposition), **not** a relaxation —
    `tests/TOLERANCE_NOTES.md`.
  - **Gate:** `gen_phase8.py` captures the oracle's `SeqVoltages`/`SeqCurrents`/`SeqPowers` →
    `tests/golden/phase8/export_seq{voltages,currents,powers}.{txt,meta.json}`; `golden_phase8.rs`
    replays + diffs (`ExactOrdered`). SeqVoltages/SeqCurrents = 6-sig magnitudes (`EXPORT_REL=1e-4`,
    `abs=1e-9`/`1e-8` = the **measured** volt/amp residual floor) + 4-sig ratio columns (`%`-prefix
    `ColTol`, `rel=1e-3`); SeqPowers = the `Powers` additive floor (`rel=0`, `abs=0.11`). golden_phase8
    **10→13**; lib stays **719** (formatters gated end-to-end). `solvable_now` **88** (no
    migration — the `Export` decks need the full export set; migration at the WP8.2 completion
    gate).
  - **audit-code follow-up (independent agent): faithful — no Critical/Major/Minor, no fixes.**
    Verified field-by-field vs Pascal: header byte-exactness (incl. double spaces / `p.u.,Base kV`),
    the Sources→PD→PC→Faults vs PD→PC iteration orders, `k=(j-1)*ncond+i` indexing, the zero/pos/neg
    `v012[0]/[1]/[2]`↔`V012[1]/[2]/[3]` mapping, `S.re·0.003` (+ excess no-`0.003`, PD-term-1-only,
    12-vs-8 fields), the ratings `>0` guards, the NEMA LL-vs-phase input split, and the `Iresidual`
    `TODO(compat)`. Settled **[Question]** SymComp `precise()` vs official → confirmed correct
    (oracle default is `precise()`; "official" only under the off-by-default `BadPrecision` compat
    flag). **[Nit]** `norm_amps`/`emerg_amps` fetched before the `do_ratings` guard — harmless (no
    side effects, consumed only under the guard). Tracked-untested-but-faithful: the Faults walk
    (no Fault objects in IEEE13) + the `<3`-node/positive-sequence else-branches.
  - **audit-tests follow-up (independent agent): sound + non-vacuous, 2 Minors fixed + 1 tracked.**
    Mutation-confirmed the gate catches a real per-row ratio scale error, a magnitude error on a
    gated row, and a `%Normal` error, while ignoring genuine noise. **Fixed:** (1) **[Minor]** the
    gate `|I1|<1e-6` over-skipped the 32 *exactly*-zero rows (a planted nonzero `%NEMA` there passed
    silently — mutation-proven); **band-limited** to `0 < |I1| < 1e-6` so those rows' `0==0` ratios
    stay checked (only the 1 genuine-noise row is now skipped). (2) **[Minor]** the `SeqCurrents`
    `abs=1e-3` magnitude floor was ~6 orders too loose (looser than the smallest real printed
    `I1=5.8e-4 A`); **measured** the actual per-column residual (max abs 1e-11 V / 1e-9 A) and
    tightened to `abs=1e-9` (SeqVoltages) / `1e-8` (SeqCurrents) — proven floor, not a guess.
    **(3) tracked [Minor]:** the `SeqPowers` MVA path is unreachable by dispatch (no golden possible);
    the Faults walk + 2-phase nonzero-`%NEMA` path need a synthesized fixture (deferred — no corpus
    deck). golden_phase8 stays **13** (tighter floors, no new tests).
- **sub-step 2c — the per-terminal/per-conductor element exports — done, gate-green.** The six
  remaining WP8.2 element/tap reports on solved IEEE13: `Currents` (`ExportCurrents:612` +
  `CalcAndWriteCurrents:518` — per-terminal/-conductor `|I|`/angle over the widest element `MaxCond×
  MaxTerm`, zero-filled, + a per-terminal `Iresid`; walk Sources→PD→Faults→PC via `GetCurrents`),
  `NodeOrder` (`ExportNodeOrder:750` + `WriteNodeList:716` — `"Elem", Nterms, Nconds, node#…` via
  `GetNodeNum` = `MapNodeToBus[NodeRef].NodeNum`), `ElemCurrents`/`ElemVoltages`/`ElemPowers`
  (`WriteElem*:799/881/962` — per-conductor `|I|`/`|V|`/kW·kvar over `ComputeIterminal`/
  `ComputeVterminal`), and `Taps` (`ExportTaps:3729` — per-RegControl controlled-transformer
  present/min/max tap + increment + `TapPosition` + winding + reverse/cogen mode). Files
  `report/export/{currents,node_order,elem,taps}.rs`; dispatch arms ptr 3/40/41/42/43/44. New router
  helpers `export_elem_ordered` (the `WriteElem*` per-element `IsSolved` #222001 guard: header-only +
  the error once when unsolved) and `export_with_classes` (read-only `(&[DssClass], &Circuit)` for
  `Taps`, which downcasts RegControl + Transformer). New RegControl accessors `tr_winding`/
  `in_reverse_mode`/`in_cogen_mode` (the runtime mode flags, distinct from the `cogen=` property).
  - **The `ElemPowers` Vsource order fix (a real divergence caught, not rationalized).** Our first
    cut reproduced Pascal's textual `ComputeVterminal; ComputeIterminal` order and the golden failed:
    Rust `Q_1 = -612.936` vs oracle `-612.729`. Root cause proven: Pascal's post-solve
    `ComputeIterminal` is a no-op (`ITerminalUpdated = TRUE`) so `Vterminal` stays `NodeV`, but our
    `compute_iterminal` re-runs `GetCurrents`, and `TVsourceObj.GetCurrents` **overwrites** `Vterminal`
    with the source EMF `[Vsource; 0]` (≈ but ≠ `NodeV` — the isolated-source `-612.936` 2a surfaced).
    Fix: call `compute_iterminal` **before** `compute_vterminal` so `Vterminal = NodeV` at the power
    product — reproducing the oracle's observable `NodeV·conj(I)` (`-612.729`). Inert for every other
    element (their `GetCurrents` also sets `Vterminal = NodeV`). Not a `TODO(compat)`; documented at
    `elem.rs` + `tests/TOLERANCE_NOTES.md`.
  - **New harness machinery — `ColSel` (name-prefix | index-parity) for `ColTol`.** The `ElemCurrents`/
    `ElemVoltages` headers are **truncated** (`…, I_1, Ang_1, ...`) — they name only the first mag/angle
    pair — so the existing header-name-prefix `ColTol` can't reach the later angle columns. Refactored
    `ColTol.prefix: String` → `sel: ColSel` (`Prefix(..)` | `Parity{start, parity}`) and `gate:
    Option<(usize,f64)>` → `Option<GateSpec>` (`Col(col,thresh)` | `PrevCol(thresh)`). The angle columns
    use `Parity{start, 1}` (every other column from `start`) with `rel=0`/`abs=0.011` (the `%8.2f`
    additive floor), gated on their **paired magnitude** (`PrevCol` = the preceding column) so the angle
    of a near-zero residual/open-terminal/grounded-neutral conductor (faer-vs-KLU noise) is skipped where
    `0 < |mag| < 1e-6` — the band-limit keeps exactly-zero rows' `0.00==0.00` checks. `Currents` (full
    header, all pairs) uses `Parity{1,1}`; the `SeqCurrents`/`SeqVoltages`/`Voltages` `Prefix` overrides
    are unchanged (mechanically ported to the new enum). A proven cancellation floor, not a relaxation.
  - **Gate:** `gen_phase8.py` captures the oracle's six reports on solved IEEE13 →
    `export_{currents,nodeorder,elemcurrents,elemvoltages,elempowers,taps}.{txt,meta.json}`;
    `golden_phase8.rs` replays + diffs (`ExactOrdered`). golden_phase8 **13→19**; lib **722** (formatters
    gated end-to-end, no new inline tests — the WP8.1 `export_records_scoped_not_ported` case 2 switched
    from the now-ported `elem` to the still-unported `summary`). `solvable_now` **88** (no migration — the
    full WP8.2 export set + the completion gate lands next).
  - **Tracked-untested-but-faithful:** the `WriteElem*`/`WriteNodeList` unsolved-circuit #222001 path
    (header-only + one error; Pascal emits it once per element — the observable file + error presence
    match, no gate checks the count) and the `Currents` Faults walk (no Fault objects in IEEE13) are
    code-faithful but unexercised (no unsolved/Fault corpus/golden deck).
  - **audit-code (independent agent): faithful — no findings.** Loop-for-loop reconfirmed vs Pascal:
    all six formatters (headers, `%10.6g`/`%8.2f`/`%8.5f` formats, name casing/quoting), the `Currents`
    `MaxCond=1`/`MaxTerm=2` width grown over *all* `ckt_elements` (verified `add_ckt_element` pushes
    regardless of `enabled`), the `GetNodeNum` ground branch, the `winding_tap_data` destructure order +
    `TapPosition` FPC-Round, the runtime `In{Reverse,Cogen}Mode` flags, and the ptr/filename map — all
    correct. The `ElemPowers` order inversion verified **load-bearing** (the export hits a
    `compute_iterminal` cache miss, so `get_currents` re-runs and clobbers `vterminal` to the EMF; the
    trailing `compute_vterminal` restores `NodeV`). The 222001-once-vs-per-element deviation is
    doc-acknowledged (observable file + error presence match), not a finding. Nothing to fix.
  - **audit-tests follow-up (independent agent): genuine + strong, 1 LOW fixed.** Mutation-verified all
    6 goldens non-vacuous — the `-612.729→-612.936` pre-fix regression fails loudly, magnitude/angle/
    node/tap mutations each fail, and the `PrevCol` angle gate skips **only** genuine faer-vs-KLU noise
    (a real 5.7e-4 A angle stays checked, a 6.2e-11 residual angle is skipped; `Parity` targets the
    angle columns, never masking a magnitude — a 0.005 `ElemCurrents` magnitude drift fails). Floors
    confirmed proven, meta single-sources the deck, goldens are genuine `check_pin` oracle captures.
    **Fixed [LOW]:** switching the unit test's case 2 to `summary` dropped the `elem`→`ElemCurrents`
    *earliest-wins* abbreviation coverage; **re-added** as case 4 (`export elem` on a solved circuit
    emits `…_EXP_ElemCurrents.csv`, no error) — an assertion inside the existing test, so lib stays **722**.
- **side fix — VSConverter reporting made side-effect-free (WP7.8 follow-up).** Re-verifying the
  WP7.8 oracle-bug claim (user request) both **confirmed it beyond doubt** and refined it: the Pascal
  `GetCurrents` → `GetInjCurrents(ComplexBuffer)` self-aliased `MVMult` was reproduced **bit-exact**
  (~1 ulp) by simulating the aliased row-wise product from the converged voltages; the symptom is
  **backend-`mvmult`-size-dependent** — `Yorder=8` (default 4-phase): product returns all zeros →
  reported AC currents = plain `Yprim·V` (the famous 1248 A vs physical 390 A) + DC falls into the
  `Pac==0 → 1000·kW` default, reads stable-but-wrong; `Yorder=4` (the `vsc0/vsc1test` decks): reads
  are **non-idempotent** (×|Y|² growth per read: 1.1e5→5.1e7→1.8e10 A) and one `Currents` read
  between two solves **poisons the next solve** via the corrupted `ComplexBuffer` tail (vsc0test:
  re-solve diverges, 8.15 kV → 950 kV). Full upstream bug report: `tmp/vsconverter_bug_report.md`
  (for dss-extensions/dss_capi). Also re-confirmed `skipped_oracle_issue` is the right class: both
  decks match the oracle on voltages/iterations/all other elements; only the converter's own
  reported rows differ. **Fixed in the port:** `get_currents` had an accidental extra deviation —
  it overwrote `cd.inj_current` (Pascal's reporting writes only the `ComplexBuffer` scratch,
  leaving the solver's `InjCurrent` lag state intact), so a mid-run currents read would have
  shifted the next step's `Pac` lag vs the oracle (a future cross-step state-leak, the InvControl
  lesson). `compute_inj_currents` now **returns** the vector; the solve path stores it, the
  reporting path subtracts a local — observation no longer perturbs solver state. Values
  unchanged (probe-verified on both decks + gate; lib stays 722).
  **Follow-up sweep (same class, all elements):** audited every `get_currents`/reporting path for
  observation-perturbs-state deviations vs Pascal. Machine family (Generator/Load/PVSystem/
  Storage/IndMach012) — **faithful, no change**: Pascal's own `GetTerminalCurrents` recomputes
  `InjCurrent` on read behind the `IterminalSolutionCount` guard, and the port replicates guard
  and all. UPFC `Vbin`/`Vbout` refresh + VCCS `Vterminal`/`sV1` refresh on read are Pascal's own
  side effects — kept. PD elements/default trait impl — pure. `get_all_variables` — read-only
  everywhere. **Fixed (latent, same shape as VSConverter): VSource, VCCS, UPFC** — their
  `get_currents` overwrote `cd.inj_current` where Pascal writes the `ComplexBuffer` scratch; all
  three injections are pure functions of state/voltages (no lag), so the overwrite was
  value-identical today and only latent — normalized anyway to the `compute_inj_currents()`
  return-a-vector pattern (solve path stores, reporting subtracts a local). UPFC's SR0/SR1
  registers advance only in `upload_currents` (UPFCControl-clocked), never on read — verified
  both engines. ISource/GIC not yet ported — port them with this pattern from the start.
- **fix — `Compile` must move the report `OutputDirectory` (WP8.1 `929145c` porting bug).** Pascal
  `DoRedirect` runs `SetDataPath(DSS, CurrDir)` for **Compile** both before processing the deck and
  in the `finally` (`ExecHelper.pas:546/651`), so after any `Compile` the default-named exports land
  **next to the compiled deck**; the WP8.1 port pinned `output_directory` to the startup cwd (the
  field doc even asserted "not Compile/Redirect" — a mis-read of the Pascal), so every default-named
  `Export` after a `Compile` landed in the process cwd instead. Oracle-verified both ways (dss-python
  0.15.7 probe: default name + explicit *relative* name both resolve to the deck dir; pre-fix Rust
  wrote both to the process cwd). Invisible to `golden_phase8` because every driver issues
  `Set DataPath=` after compiling and reads the file back via `last_result_file`. **Fixed** in
  `do_redirect` (entry + exit re-assert, so a nested Compile can't leave the dirs at the inner deck;
  plain `Redirect` still moves neither), plus the two same-root relative-path holes: `Set DataPath=`
  and the explicit `Export <x> <file>` filename now resolve against the engine's `current_dir` (the
  virtual mirror of Pascal's chdir'd process cwd), not the OS cwd. New lib test
  `compile_moves_output_directory_redirect_does_not` pins all four behaviors (lib **723**).
  Two follow-ups from the same review sweep: `SeqCurrents` `%Normal`/`%Emergency` now print the raw
  rating when it is non-positive (Pascal seeds `iNormal := NormAmps` and only overwrites when `> 0`,
  `ExportResults.pas:409-414`; was forced to 0 — unpinnable on IEEE13, all ratings positive), and the
  `golden_phase8` `SeqCurrents` I1-gate is scoped to `%I…`/`%NEMA` so `%Normal`/`%Emergency` (NormAmps
  denominators) stay checked on the gated noise row (TOLERANCE_NOTES updated).
- **sub-step 3 — the matrix/summary exports — done, gate-green.** The five remaining WP8.2
  solution exports: `Yprims` (`ExportYprim:2680` — every enabled PD/PC element's primitive Y, device
  order, `Class.NAME` header + `Yorder` rows of `%-13.10g` re/im pairs), `Y` (`ExportY:2727` — the
  assembled system Y, sparse-triplet `Row,Col,G,B` lower-triangle when `t…`/`TripletOpt`, else the
  dense node-by-node `+j`-cell form), `SeqZ` (`ExportSeqZ:2822` — per-bus `Zsc1`/`Zsc0` R/X/|Z| +
  `X/R` ratios, `Get_Zsc1`/`Get_Zsc0`), `Summary` (`ExportSummary:2985` — the one-row status line;
  **appends** if the file exists, header only on create), and `Result` (`ExportResult:3770` — dump
  the `@result` parser var). Files `report/export/{yprims,y_matrix,seq_z,summary,result}.rs`; dispatch
  arms ptr 16/17/18/27/45; new router helpers `export_y_to_file` (the #222 `Y Matrix not Built.`
  guard + the `system_y_csc` COO) and `export_summary_to_file` (the append-vs-create decision + the
  `total_power`/`losses`/pu-extreme gather). New Parm2 pre-parse for ptr 17 (the `t…` triplet flag,
  ahead of the filename, mirroring the 9/19 MVA trap). `GetMaxPUVoltage`/`GetMinPUVoltage` ported
  (`summary.rs`), plus a dependency-free civil-date `current_datetime_string` (Howard-Hinnant
  `civil_from_days`) for the timestamp.
  - **`Result` is always `null` — a pinned-oracle-faithful, not a fake.** `ExportResult` dumps the
    `@result` parser var. The pinned oracle is a `DSS_CAPI_PM` build, so the `@result := GlobalResult`
    update at the `ProcessCommand` tail (`ExecCommands.pas:704`) is compiled out (`{$IFNDEF
    DSS_CAPI_PM}`); `@result` therefore stays at its `ParserDel.pas:875` init `'null'` **forever**
    (oracle-probed: `null` after compile+solve+`?Line.phases`, even though `Text.Result` was `'3'`).
    Our engine likewise never writes `@result` (`ParserVars::new` seeds it `"null"`), reproducing the
    observable with zero divergence — documented at `result.rs`.
  - **`SeqZ` gated on a FaultStudy solve.** `Zsc` is only allocated by the FaultStudy solve
    (`Bus::zsc = None` on a snapshot → `get_zsc1`/`get_zsc0` = 0 → the degenerate all-zero/1000-ratio
    report both engines produce). So the `SeqZ` golden uses its own fixture — a fresh compile +
    `solve mode=faultstudy` (`meta.post`), generated separately (`gen_seqz`) since the study mutates
    NodeV and can't share the snapshot loop — pinning the real per-bus `Zsc1`/`Zsc0` (the same values
    `exec/tests/fault_study.rs` pins to 1e-9·mag).
  - **`Y` dense is unpinnable-by-CSV, so the triplet form is gated.** The dense `ExportY` glues `+j`
    onto every imaginary token (`%-13.10g, +j %-13.10g,`), so those fields don't parse as numbers —
    the `compare_export` comparator can't diff them. The **triplet** form is clean `Row,Col,G,B` CSV,
    so the golden pins `export y triplet` (lower triangle `r>=c`, column-major, sorted `(col,row)` to
    match KLU `GetTripletMatrix` order). Both forms are ported faithfully; the dense values are
    already pinned entry-by-entry by the checkpoint + live full-Y gates.
  - **`Summary` `DateTime` masked; the append behaviour honoured.** Column 0 is `DateTimeToStr(Now)`
    (wall-clock, non-deterministic) — masked via the new `ColSel::Index(0)` + `GateSpec::Mask` harness
    additions (a genuine non-deterministic column, not a value relaxation; `TOLERANCE_NOTES`). Every
    other column is deterministic (text `Status`/`Mode`/`ControlMode` case-insensitive; integer counts
    exact; `%g` scalars at the 5–6-sig `EXPORT_REL` floor). `TotalMW/Mvar = -total_power()·0.001` (the
    `GetTotalPowerFromSources = -Σ source.power[1]` negation), `MWLosses/Mvar = losses()·1e-6`. The
    golden uses the shared compile+solve fixture (the two sides self-consistent).
  - **Yprims element set = PD ∪ PC ∪ sources ∪ faults.** Pascal filters `is TPDElement or is
    TPCElement`; in our model `TVsourceObj` is a PCElement (our `ElemKind::Source`, `sources` list) and
    `TFaultObj` a PDElement (our `ElemKind::Fault`, `faults` list), so the walk unions those four ref
    lists (keyed `(cls,idx)`, `ElemRef` not being `Hash`) and emits in device (`ckt_elements`) order.
  - **Gate:** `gen_phase8.py` captures the six oracle reports (5 on solved IEEE13 + `SeqZ` on the
    faultstudy fixture) → `export_{yprims,y_triplet,summary,result,seqz}.{txt,meta.json}`;
    `golden_phase8.rs` replays + diffs (`ExactOrdered`; `YMATRIX_REL = 1e-6` for the 10-sig Y/Yprim
    cells, `EXPORT_REL` for the 6-sig SeqZ magnitudes, `rel=1e-3` for the 4-sig `X/R` ratios). The
    WP8.1 `export_records_scoped_not_ported` case 2 moved off the now-ported `summary` to the
    still-unported `eventlog` (ptr 33, WP8.3, not solution-guarded). golden_phase8 **19→24**; lib
    **723** (no new inline tests). `solvable_now` **88** (no migration — that lands in the completion
    gate).
- **WP8.2 completion gate — done, gate-green (WP8.2 COMPLETE).** Closes WP8.2 with the
  bus/summary exports at scale + the `Export`-unblocked corpus migration.
  - **IEEE8500 bus/summary goldens.** `gen_phase8.py::gen_ieee8500_reports` captures the
    oracle's `Voltages`/`Summary`/`Counts` on the solved IEEE 8500-Node feeder (8531 nodes,
    6103 devices; `Set Maxiterations=20` to converge, exactly as `golden_ieee8500.rs`);
    `golden_phase8.rs::export8500_reports_match_oracle` compiles the master **once** (the new
    `run_shared_exports`, avoiding 3 heavy recompiles) and diffs all three — the 4876-bus
    Voltages row set + `%6.1f` angle floor, the masked-DateTime Summary status row, the
    `RustSubsetByKey` Counts (Line=3703/Transformer=1190/…). Completes the PORTING_PLAN §Phase 8
    export-diff over 13/34/37/123/**8500** (the 8500 per-element/matrix dumps stay
    omitted-as-enormous — the established 8500 discipline). `Counts` sets no `GlobalResult` in
    the oracle, so its fixture path is built from CaseName+suffix. golden_phase8 **24→25**.
  - **Corpus migration (`solvable_now` 88→119, +31; COVERAGE 26.3%→35.5%).** Probed **all 192**
    `skipped_unsupported` decks on the current Rust engine (throwaway watchdog probe): **38 now
    run error-free** — unblocked by the WP8.2 solution exports + the Show/Plot/Visualize no-ops
    (+ WP7 class ports whose stale `unsupported_*` tags never got re-probed). Moved those to
    `skipped_needs_investigation`, ran the live `DSS_LIVE_CLASSIFY` full-model probe: **31 match
    the oracle** and were promoted to `solvable_now` — the line-constants/cable decks
    (`CableParameters`/`Cable_constants`/`IEEELineGeometry`/`TriplexLineCodeCalc`/`LineConstants`/
    `BundleDemo`), the Distance/`59N` relay demos, the `StorageTechNote/Example_9_*` set, the VCCS
    `HWDyn`/`HWPLL`/`HWPLL3`/(Examples)`DG_Prot_Fdr`, and the IEEE feeders
    (`Run_IEEE123Bus`/`123Switches`/`Run_IEEE30`/P174 voltage-profile 123+8500/`ODRegTest`/
    `TestAuto`/`YgD`/`MultiCircuit`). The **19 that didn't match stay in `needs_investigation`** —
    **none were Rust-engine errors** (so none bounced back to `skipped_unsupported`): 7
    newly-surfaced (`AutoHLT`×2 / `TestDDRegulator` / `IEEE13_CDPSM` / LVTestCase `Master`+`SecPar`
    = small live mismatches ~1e-8..5e-7 rel, retagged `live_mismatch`; `vsctest` = oracle
    non-convergence) + the 12 pre-existing tracked-opens. This is also the corpus-hygiene re-probe
    the WP7.5 audit flagged (a clean Rust compile ≠ migratable — the live gate is the arbiter).
    `skipped_unsupported` 192→154; `COVERAGE.md` refreshed.
  - **New live-gate infra — the Rust `CorpusGuard` (`corpus_live.rs`).** A migrated deck's
    `Export`/`Show`/`Save` writes report files next to the deck (Pascal `Compile` sets
    `OutputDirectory := <case dir>`), and a `debugtrace=yes` element writes a `STOR_<name>.csv`
    trace — pure pollution of the vendored corpus fixture, which the live gate never reads.
    `CorpusGuard` (RAII, mirror of the oracle server's `_CorpusGuard`) snapshots the case dir
    **before both engines run** and on drop deletes created files + restores overwritten ones. The
    before-both-engines placement is load-bearing: the `STOR_storage1.csv` the oracle writes (the
    Rust port doesn't port `debugtrace` file output) and can't self-delete (dss-python holds the
    handle open) is caught by this outer guard. Git-verified: the corpus stays pristine after the
    full gate.
  - **Issue-1 resolution (RESOLVED 2026-07-03 — "9 decks hang the Rust engine >40s" was a
    watchdog artifact, no hang exists).** Re-probed all 9 (plus the 2 sibling
    `StorageControllerTechNote` decks) sequentially on an idle machine with a 900s watchdog +
    post-run state inspection (`get hour`, `converged_flag`, `solution_count`): **every deck
    completes and converges**, with only the expected `not ported` errors. Release build:
    0.5–3.4s each. Debug build: the 8 `StorageControllerTechNote/*Run` finish in 6–28s (all
    `hour=24`, 39–115 total solves — the control loop terminates normally; no StorageController
    bug), `ExpControl/Master` 27s (5 circuits × 86400-step 1s daily, `hour=24`), `master_ckt24`
    6s (yearly ×100), and `8500-Node/P174_Run_360kW_PV` **128s** (2900-step 1s duty on IEEE8500 —
    genuinely slow in debug, finite). The engine is byte-identical to the one the original probe
    ran (`git diff b862bee..HEAD -- crates/` touches only test files), so the ">40s" hits were
    the original probe's methodology: debug-build wall-clock plus CPU contention from its own
    detached timed-out threads (a 40s watchdog leaves the thread running and crunching through
    the rest of the 192-deck sweep; P174 alone exceeds 40s in debug even when idle). All 9 stay
    in `skipped_unsupported` on their **real** blockers, with manifest tags/notes refreshed
    (11 entries): the 8 TechNote decks → `unsupported_command=BatchEdit,Export` (the stale
    `unsupported_class=Storage` dropped — Storage/StorageController are ported, WP7.4),
    `P174_Run_360kW_PV` → `unsupported_feature=file-backed-arrays` (its old
    `unsupported_command=Plot` tag was stale too — Plot/Visualize are silent no-ops; the sole
    remaining error is the file-backed `PVCurve` LoadShape), `ExpControl/Master` gains
    `unsupported_command=Export`. The remaining **19 stale `unsupported_class=Storage`
    entries were then re-probed the same way** (release, 600s watchdog, state inspection) —
    all 19 complete fast and converge; all retagged to their real blockers: the 14
    GFL/GFM decks (IBRDynamics_Cases + Microgrid/GridFormingInverter) → BatchEdit + the
    InvControl GFM combi mode (`mode=7 combi=0` raises the loud WP7.7 deferral and the
    solve aborts), Paulo_Example → AddBusMarker+Export, Run_Demo1 → CloseDI+file-backed
    arrays, SolarRamp → file-backed arrays only (its stale `unsupported_command=Plot`
    dropped — Plot/Visualize are no-ops), and the 2 StoCtrl_* decks → DI/season Set
    options + MakeBusList/Wait/CloseDI/BatchEdit. **That sweep also exposed and fixed a
    real port bug:** `capture_metered` (Monitor) classified Storage as plain
    `MeteredKind::PcElement` and *nothing* ever produced `MeteredKind::Storage`, so the
    Pascal mode-7 class check (`CLASSMASK = STORAGE_ELEMENT`, Monitor.pas
    `RecalcElementData` case 7) could never pass — every valid `Monitor mode=7
    element=Storage.*` errored "is not a storage device!" (and in both StoCtrl_* decks
    cascaded into a bogus singular-Y solve abort; post-fix both run their full yearly sim,
    hour=8760, converged). Fix: Storage gets its own `MeteredKind::Storage` arm and the
    mode-3 check accepts `PcElement | Storage` (Pascal mode 3 is `BASECLASSMASK =
    PC_ELEMENT`). Oracle-verified (pinned dss-python: Storage mode=7 OK, Load mode=7 →
    `#2016002`, Storage mode=3 OK) + 2 new unit tests in `exec/tests/storage.rs`
    (`storage_accepts_mode7_monitor`, `mode7_monitor_rejects_non_storage`). Mode-7
    *sampling* stays deferred (header-only, as before); no `unsupported_class=Storage`
    tags remain in any manifest.
  - **Issue-2 root cause (RESOLVED — the "rare live-gate flake" was never a concurrency race).**
    The `Test/YgD-Test.dss step 0: oracle did not converge` flake was root-caused empirically to a
    **per-process convergence misfire in the pinned engine itself** (dss_capi 0.14.5): on a fresh
    process's first compile of this deck (the mid-deck open-phase rewire `Transformer.tr1.wdg=1
    bus=HV.1.2.4` adds node `HV.4`), the post-rewire solve hits MaxIterations (`Converged=false`,
    iters=15, a bit-identical wrong V every time) with **no DSS error raised**, in **~16% of fresh
    processes** — measured at the same rate with 0 and 48 CPU-burner processes (load-irrelevant;
    the "1-in-5 under `--workspace`" correlation was a sampling illusion — 3× green in isolation has
    51% probability at this rate) and independent of `PYTHONHASHSEED`. A `clear`+recompile **in the
    same process heals it** (bistable, uninitialized-memory-style; never persisted past the 2nd
    recompile in 75 trials), and the healed result is the one deterministic fixpoint the Rust engine
    matches. Every file-race hypothesis was tested and **refuted for this signature**: a lock on the
    `Show` report file raises `#303`, a locked/missing deck raises `#243`, truncated deck reads
    either error or converge — all surface as `ok=false` (oracle error), never `converged=false`.
    **Fix:** `oracle_server.run_case` now retries the whole case in-process (≤3 attempts, loud
    stderr log) when any checkpoint reports non-convergence; a legitimately non-converging case
    still fails after 3 identical attempts, so nothing is masked. Verified: 30/30 one-shot YgD runs
    converge (retry fired 2×, healed both). **Bonus real bug found & fixed while refuting the race
    hypotheses:** the Python `_CorpusGuard.__enter__` wrapped its whole snapshot loop in one
    `except OSError` — a transient lock on ONE file mid-snapshot truncated the `names` set and
    `__exit__` then **deleted every corpus file sorting after it** (demonstrated live: killed 4
    `Test/` decks, restored from git). It now mirrors the Rust guard: per-file `try`, plus
    `_snapshot_ok` gating all deletion. The oracle-output-redirection idea (per-case temp-copy
    compile) is therefore NOT needed for this issue and stays unscheduled; the #303/#243 lock
    windows it would close have never fired in practice.
  - **Gate:** `cargo fmt`/`clippy`/`test` green; golden_phase8 **25**; lib **723**; the always-on
    `corpus_live` full-model compare green over **all 119** `solvable_now` cases (105 s; a clean
    full-workspace re-run confirmed green after the isolated flake above).
    **WP8.2 COMPLETE.**
  - **audit-code follow-up (independent agent): faithful — no Critical/Major; 1 defensive fix.**
    Verified `CorpusGuard` is a semantically-exact mirror of the oracle's `_CorpusGuard`
    (RESTORE_MAX, delete-created/restore-overwritten, drop order: `dss` drops before `_guard`
    so engine file handles close first), `run_shared_exports` genuinely single-sources + diffs
    all three reports, and `gen_ieee8500_reports` faithfully captures the oracle. **Fixed
    (defensive):** the empty-snapshot hazard — if `CorpusGuard::new`'s initial `read_dir` failed
    (transient EMFILE / AV-or-indexer lock), `names` would be empty and Drop would treat every
    file as run-created and delete the whole feeder dir; added a `snapshot_ok` flag that disables
    deletion on a failed snapshot (safe to diverge from the Python mirror here — test infra, not
    a ported algorithm). Surfaced-not-fixed (mirror-the-oracle limits, no gate effect): the guard
    is non-recursive (immediate case dir only), doesn't content-restore >2 MiB overwritten files
    or remove created subdirs, and corpus-pristineness stays best-effort (git-verified, no
    automated gate) — all matching the oracle guard.
  - **audit-tests follow-up (independent agent): sound + strengthening — no weakening,
    mutation-verified.** The auditor independently ran the always-on gate (**119/119** matched
    the oracle, 74 s) and mutation-tested the 8500 goldens: a +1% magnitude, a >0.11 angle, a
    Transformer-count +1, and a NumNodes +1 each FAIL; the DateTime mask + a sub-0.11 angle
    correctly PASS; the `RustSubsetByKey` require-set is non-vacuous (a dropped Line row FAILs).
    Confirmed the goldens are genuine `check_pin` oracle captures (single-sourced via `meta.json`),
    the 31 promotions are `kind=feeder` (tightest v-rel-1e-8 tier) probed under the exact
    conditions the gate re-runs, none vacuous, the 19 non-matches correctly stayed out, and the
    `CorpusGuard` cannot mask a failure (it only touches on-disk files the gate never reads).
    **Surfaced-not-fixed (pre-existing, cosmetic):** the two `PV_currentkvarLimit_*`
    `needs_investigation` notes read `…|diff| = 1.7e-4 > allowed 1.420` — arithmetically
    impossible because `apply_classify`'s `note[:300]` cap truncates the trailing `allowed
    1.42e-4` mid-token; the comparator itself is correct (`{:e}`), the cases are pre-existing
    tracked-opens kept out of the gate, a tooling nit outside WP8.2 scope.
- **WP8.2 follow-up — the remaining solution-family exports (`VoltagesElements`/`YVoltages`/
  `YCurrents`), done, gate-green.** These three read-only solution exports were listed in
  PHASE8_PLAN WP8.2 step 1 but skipped in the sub-steps (an in-family gap, not a phase deferral).
  `VoltagesElements` (ptr 35, `ExportVoltagesElements`/`WriteElementVoltagesExportFile`) — the
  by-element companion to `Voltages`: per-element/terminal/conductor `Node`/`Magnitude`(kV)/
  `Angle`/`pu` + per-terminal `Bus`/`BasekV`, ragged rows (only each element's `NTerms` blocks),
  the conductor-1-only BasekV (with the Pascal grounded-first-conductor omission quirk reproduced),
  walked Sources→PD→Faults→PC. `YVoltages` (47) / `YCurrents` (48, `ExportY{Voltages,Currents}`) —
  the raw Y-ordered node vectors (`NodeV` / `Solution.Currents`), one `re, im` per node, no header.
  Files `report/export/{voltages_elements,y_voltages,y_currents}.rs`; dispatch arms 35/47/48
  (35 via `export_with_mut` for the element walk, 47/48 pure `fn(&Circuit)`). Gate: goldens
  `export_{voltageselements,yvoltages,ycurrents}` on solved IEEE13 (`VoltagesElements` angles at the
  `%6.3f` additive floor, magnitudes/vectors at `EXPORT_REL`) — matched the oracle first-run.
  golden_phase8 **25→28**; lib **725**. Now every ptr in the `EXPORT_OPTIONS` solution family is
  ported; the remaining unported keywords are WP8.3 (device/meter) + Phase 9 (CIM/GIC/A-Diakoptics)
  + the faithfully-errored CDPSM set. (No corpus re-migration run here — a future `DSS_LIVE_CLASSIFY`
  pass can pick up any deck these three unblock.)
- **WP8.3 step 1 — `Export Monitors` (`Monitor.TranslateToCSV`), done, gate-green.** The
  device-export sub-block opens with the monitor CSV writer (ptr 15, `ExportOptions.pas:471`
  → `TMonitorObj.TranslateToCSV`, `Meters/Monitor.pas:1690`). Pieces:
  - **`util::comma_text`** — FPC `TStringList.CommaText` (`GetDelimitedText`, non-strict): join
    with `,`, quoting (and doubling embedded quotes on) any item that is empty or holds a char
    `<= ' '` / the delimiter / the quote char. This is the monitor **header line**, which the
    golden compares **verbatim** — so the quoting must be byte-exact (`"S1 (kVA)"`, `"Tap (pu)"`).
  - **`Monitor::to_csv`** — serializes the in-memory f32 buffer: header (`comma_text`), then per
    sample `hr:0:0, s:0:5` + `, %-.6g` per channel (`RecordSize` values). Pascal's `Save` /
    `CloseMonitorStream` are no-ops (every sample is already in `mon_buffer`, no pending-buffer /
    disk spill); `FireOffEditor` is the GUI no-op; `GlobalResult` is set by the caller.
  - **dispatch (`exec/report.rs`)** — `export_monitors`: the monitor-name **pre-parse** (case-
    preserved `Parm2`, ahead of the ignored trailing filename); `name=='all'` (case-sensitive,
    Pascal) walks `ckt.monitors` in creation order, else a case-insensitive name lookup; empty →
    #251, unknown → #250. Each monitor writes its fixed `Get_FileName`
    `<OutputDir><CircuitName_>Mon_<Name>_1.csv` (the `_1` = PM-build primary-context `DSS._Name`,
    oracle-confirmed via `GlobalResult` at gen time — the same PM build the always-`null` `Result`
    keys on); `@lastexportfile`/`@lastfile` end on the last file.
  - **gate** — `export_mon_{vi,pow,tap}` on a daily-solved (`number=3`) IEEE13: mode 0 (general
    V/I, unquoted paired mag/angle header), mode 1 (power `S (kVA)`/`Ang` — the quoted-header
    path), mode 2 (the single quoted `Tap (pu)` channel). Shared one compile via
    `run_shared_exports`. The header line is verbatim-pinned; the data rows parse numbers out
    (incl. the `hour`/`t(sec)` time cols the live gate's channel compare skips) at `rel=1e-4`/
    `abs=1e-3` (the f32-stream + `%-.6g` print floor is ~1e-6 rel; the monitor *values* stay
    pinned to f32-of-1e-8 by the always-on `corpus_live` daily monitor compare). Matched the oracle
    **first-run**, no fudging. golden_phase8 **28→29**; lib **725→726** (`comma_text` unit test).
    No corpus migration here — the `Export Monitors`-blocked decks migrate at the WP8.3 completion
    gate (step 5), the WP8.2 batching pattern.
  - **audit-code follow-up (independent agent): faithful — no Critical/Major; 2 Minor fixed.**
    The auditor confirmed the record stride/slot layout, the `hr:0:0`/`s:0:5` + `%-.6g` formatting,
    the harmonic-mode uniformity, the case-15 control flow (all/find/#250/#251), the `_1` filename,
    and the solve guard all match Pascal. **Fixed:** (1) the last-file bookkeeping — Pascal sets
    `SetLastResultFile` + `@lastexportfile` **once after** the loop (so a zero-monitor `Export
    Monitors all` clears them to `''`), but the port set them per-iteration; now tracks the last
    written path and applies the bookkeeping once (degenerate edge — no gate exposure, but now
    faithful). (2) `comma_text` asserted **unverified** FPC empty-item quoting (`""`), which is dead
    for the monitor contract (no header label is ever empty) and contradicts "empirical over guessed
    FPC semantics" — dropped the empty-item special-case + its unit-test assertion, documenting why.
    **Surfaced-not-fixed (Nits):** the `\n` line terminator (Pascal emits CRLF) is the project-wide
    Phase-8 export convention, masked by the LF-normalizing golden pipeline; and the deferred sample
    modes (4/7/8/10/12, `sample.rs`) would emit header-only CSVs — out of scope (those decks stay in
    `skipped_unsupported` until the WP8.3 completion migration).
  - **audit-tests follow-up (independent agent): sound + non-vacuous — mutation-verified; 1 Minor
    tightened, 1 doc fix.** The auditor mutation-tested the golden: reversing channel order, dropping
    the header quoting, dropping the `_1` suffix, zeroing the hour column, and a `%-.6g`→4-sig
    precision cut each **FAIL** (a 6→5-sig cut is absorbed by design). Confirmed the goldens are
    genuine `check_pin` oracle captures (not self-comparison), single-sourced via the meta, over the
    real IEEE13 master, with the hour/sec time columns (skipped by the live channel compare) newly
    pinned here. **Fixed:** (1) the `abs=1e-3` floor was ~3 orders looser than the stated ~1e-6 —
    **measured** the actual Rust↔oracle gap (max_abs=0, max_rel=0: the f32+`%-.6g` prints are
    byte-identical for this fixture) and tightened to `abs=1e-5` (~100× over the ~1e-7 theoretical
    near-zero-angle floor, so it never flakes yet no longer masks). (2) the doc comment overclaimed
    the *values* are 1e-8-gated by `corpus_live` — reworded to "the sampling *code path* is 1e-8-gated
    on equivalent monitors; this golden cross-checks these exact values at the print floor." Counts
    unchanged (lib **726**, golden_phase8 **29**) — the empty-item change dropped one assertion, not
    a test.
- **WP8.3 step 2 — the register/load dumps (`Meters`/`Generators`/`Loads`/`PVSystem_Meters`/
  `Storage_Meters`), done, gate-green.** ptrs 12/13/14/49/50 (`ExportOptions.pas` case
  dispatch → `ExportMeters`/`ExportGenMeters`/`ExportLoads`/`Export{PVSystem,Storage}Meters`,
  `ExportResults.pas:1909…2446`). Pieces:
  - **`report/export/registers.rs`** — the register-dump line formatters: `register_header`
    (`Year, LDCurve, Hour, <label>` + `, "<regName>"` per register — compared **verbatim**, so the
    `, ` separators + quoting are byte-exact) + `register_row` (`Year`/`LDCurve`/`Hour`/
    `Pad('"'+UPPER(name)+'"',14)` + `%10.0f` per register) + the Gen/PV/Storage class register-name
    constants (`Generator.pas:462`/`PVsystem.pas:392`/`Storage.pas:492`; the EnergyMeter names come
    from the per-object `register_names()` — they encode the zone voltage bases).
  - **`report/export/loads.rs`** — `ExportLoads`: the present allocation view (`Load, Connected KVA,
    Allocation Factor, Phases, kW, kvar, PF, Model`), a static Load-field read (no solve state).
  - **dispatch (`exec/report.rs`)** — `export_registers(kind)` gathers each enabled element's rows
    (downcasting `EnergyMeter`/`Generator`/`PVSystem`/`Storage`) + the single-file **append-vs-create**
    writer (`register_need_rewrite`: rewrite unless the file exists AND starts `Year`, mirroring
    `WriteSingle*MeterFile`) + the **`/m` multi-file switch** (`WriteMultiple*MeterFiles`: one
    `<OutputDir><prefix><UPPER name>.csv` per element, `@lastexportfile` = the literal `/m` Pascal
    quirk). One `TODO(compat)`: the Storage `/m` prefix is `EXP_PV_` (an upstream copy-paste bug at
    `ExportResults.pas:2240`). `LoadDurCurveObj` is unmodeled (LoadDuration mode deferred) → the
    `LDCurve` column is always empty (`NameIfNotNil(nil)=''`, faithful for every non-LD deck).
  - **REAL GAP found + fixed (the export surfaced it):** the DER register-sampling tail of
    `TEnergyMeter.SampleAll` (`GeneratorClass`/`StorageClass`/`PVSystemClass.SampleAll`, EnergyMeter.pas
    l.928-931) + the `ResetAll` tail (`ResetRegistersAll`, l.895-897) was **never wired** into the Rust
    solve loop — the Generator/PVSystem/Storage `take_sample`/`reset_registers` methods existed (Phase 7)
    but nothing called them, so the DER energy registers stayed **0**. Without the fix `Export
    Generators/PVSystem_Meters/Storage_Meters` would emit all-zero rows = silently-faked output
    (PHASE8_PLAN §1 forbids). `solution/meters/sampling/take_sample.rs` now samples + resets
    Generator+Storage+PVSystem after the meter sweep — **unconditional** (Pascal doesn't gate it on a
    meter existing), which is why the DER registers accumulate even with no `EnergyMeter` defined. The
    change is register-only w.r.t. the solve **except** the faithful `UseFuel` generator path
    (`take_sample`→`check_on_fuel`→`gen_active`, which zeroes a spent generator's injection, matching
    Pascal `TGeneratorObj.TakeSample`); `UseFuel` defaults off, so no existing golden / corpus_live case
    moved (full gate re-run green).
  - **gate** — two synthesized goldens (no corpus deck uses these keywords → PHASE8_PLAN §1 synthesize):
    **(A)** plain-IEEE13 + an EnergyMeter, daily 3 → `Meters` + `Loads` (the meter registers match the
    oracle to ~1e-8 — the same daily meter path `corpus_live` pins — so `%10.0f` is identical); **(B)**
    IEEE13 + a Generator + a PVSystem + a Storage on bus 675 (off the metered zone), daily 3 → the DER
    register dumps. The DER is kept **out of the metered, regulated zone**: adding it inside shifts the
    metered-element local power ~3e-5 rel (a real small solve interaction) that straddles the meter's
    `3358.5` Max kW rounding boundary — the split keeps every value tightly oracle-pinned with **no
    tolerance masking** (register rows `rel=0`/`abs=0.5`; Loads `abs=0.05`). Plus a `/m`
    self-consistency test (the per-meter `EXP_MTR_EM1.csv` carries the same values as the single-file
    golden). Header lines verbatim-pinned; values parse out. Matched the oracle **first-run** (after the
    DER-sampling fix), no fudging. golden_phase8 **29→32**; lib **726** (net).
  - **audit-code follow-up (independent agent): faithful — no Critical/Major; 2 Minor + 2 Nit
    settled.** The auditor verified every formatter/filename/append/`/m`/`TODO(compat)` against Pascal
    and the DER-wiring args/order against `EnergyMeter.pas:928-931`/`895-897`. **Fixed:** (1) the STATUS
    claim that the DER wiring is "read-only w.r.t. the solve" was imprecise — the `UseFuel` generator
    path (`take_sample`→`check_on_fuel`→`gen_active`) *does* feed the solve; reworded (`UseFuel` defaults
    off, so the gate still confirms nothing moved). (2) `ExportLoads` now emits Pascal's **unconditional**
    `FSWriteln` (a blank line for a disabled load) so the output is byte-faithful, not just
    golden-equivalent. (3) `register_need_rewrite` reads only the first line (`FSReadLn`), not the whole
    append-log. (4) a comment documents that `sample_all_der`'s enabled-guard lives inside each
    `take_sample`. **Surfaced-not-fixed (Nit, pre-existing/systemic):** the whole Rust export family
    models the last-written path via `last_result_file`, not Pascal's `GlobalResult`/`AppendGlobalResult`
    — so the `/m` tail reproduces `@lastexportfile="/m"` but not the comma-joined `GlobalResult` file
    list; out of scope for this step (not introduced here), tracked for the WP8.x `GlobalResult` pass.
  - **audit-tests follow-up (independent agent): sound + non-vacuous (mutation-verified); 1 Major + 3
    Minor strengthened.** The auditor confirmed the goldens are genuine oracle captures (not
    self-comparison), the `rel=0`/`abs=0.5` register floor is tight (a +1 mutation fails loudly), the
    header-verbatim pin covers the register-name set/quoting, and the two-fixture split removes a
    printing-boundary flake rather than masking a divergence. **Strengthened:** (1) **Major** — the
    single-file **append** path had zero coverage (every golden used a fresh dir → only the create
    branch ran); added `export_meters_append_accumulates` (two exports → one header + two rows). (2)
    element **ordering** + the **enabled-filter** were vacuous on the single-element DER dumps; fixture
    B now has two enabled generators (g1→g2 pins creation order) + a disabled g3 (must not appear). (3)
    the Storage `/m` `EXP_PV_` copy-paste-bug prefix is now pinned by `export_storage_multifile_uses_pv_prefix`
    (asserts `EXP_PV_ST1.csv` exists, `EXP_STORAGE_ST1.csv` does not). (4) the Loads `%5.3f`
    AllocFactor/PF columns get a tight per-column `abs=5e-4` instead of the coarse `0.05` default.
    **Recorded (no fix, inherent):** the DER *own* registers are pinned nowhere finer than the `%10.0f`
    `abs=0.5` report resolution (no corpus deck uses these keywords) — a sub-0.5-unit DER-integration
    error would be invisible; the wiring fix itself is well-guarded (0→~300, dramatic). golden_phase8
    **32→34**.
- **WP8.3 step 3a — `EventLog`/`ErrorLog` dumps + the missing Circuit-build event markers, done,
  gate-green.** ptrs 33/52 (`ExportEventLog`/`ExportErrorLog`, `ExportResults.pas:3296`/`:3303`), each a
  `TStringList.SaveToFile` of `DSS.EventStrings`/`DSS.ErrorStrings` — no header, one entry per line.
  Pieces:
  - **`report/export/logs.rs`** — `export_event_log`/`export_error_log` (the shared `save_string_list`:
    each entry on its own terminated line, empty file when the list is empty). **dispatch (`exec/report.rs`)**
    — `export_event_log_to_file` (reads `ckt.solution.event_log`) / `export_error_log_to_file` (reads
    `Dss::errors`, the analog of `ErrorStrings` — the `DoSimpleMsg` record-and-continue log); defaults
    `EXP_EventLog.csv` / `EXP_ErrorLog.txt`. Neither is solution-guarded (33/52 ∉ the #24712 ptr set).
  - **REAL GAP found + fixed (the export surfaced it):** the three `LogThisEvent` markers in the
    **circuit-build** methods — `ReprocessBusDefs` (`Circuit.pas:2169` "Reprocessing Bus Definitions")
    and `DoResetMeterZones` (`:2152`/`:2156` "Resetting Meter Zones"/"Done Resetting Meter Zones") — were
    **never wired**: those methods were ported in Phase 2/3 *before* the `EventLog` type existed (Phase 5),
    and when the event log landed only the *solution-layer* call sites (`Solution.pas`/`Ymatrix.pas`) were
    retrofitted. Under `Set Log=yes` our engine therefore emitted a marker stream missing those 3 lines.
    Added `Circuit::log_this_event` (the gated `if LogEvents then LogThisEvent`, stamping the solution's
    clock/iteration) + wired the 3 sites (`circuit.rs` `reprocess_bus_defs`, `zones/mod.rs`
    `do_reset_meter_zones`, the two bracketing `ResetMeterZonesAll` even with no meters, per Pascal). The
    fourth Pascal marker `ReallocDeviceList` ("Reallocating Device List", `Circuit.pas:2072`/`:2997`) has
    **no Rust equivalent** — our device list is an auto-growing `HashList` with no manual hash-resize step
    (Pascal reallocs once `NumDevices > 2·InitialAllocation`, i.e. >1800 devices). It never fires during
    *solve* (an `AddCktElement` build-time call), but it *would* appear on a >1800-device circuit **built
    while `Set Log=yes` is already on** — a `NOT_PORTED` comment marks the drop at `add_ckt_element` (the
    standard idiom sets `LogEvents` after the definitions; no corpus deck logs a build that large, so no
    gate exercises it).
  - **gate** — golden_phase8 **34→37**, both synthesized (no corpus deck exports these): **(1)**
    `export_eventlog` — IEEE13 + per-RegControl `eventlog=yes` + **`Set Log=yes`**, driving a **swinging
    load** (1200 kW on regulated bus 675 with a 1×/2×/0.5× day shape) so all three regulators move taps,
    daily-3 → the **full** LogThisEvent marker stream (the now-complete Circuit-build markers → Yprim
    recalc → Y build → per-iteration/control markers → Solution Done) **plus** 18 regulator
    `AppendToEventLog` tap-change lines (`CHANGED n TAPS TO <pu>` — non-integer tap values, so the
    numeric-tolerance path is genuinely exercised), compared **line-for-line with numbers parsed out** (the
    phase5 event-log policy `assert_value_matches_tol`, 1e-6 rel) — matched the oracle **first-run after
    the marker fix** (133 lines; the tap decisions are the same logic the phase5 `daily_ieee13` gate pins).
    **(2)** `export_errorlog` — a clean IEEE13 solve logs no `DoSimpleMsg` → an empty dump (pins the
    plumbing + `EXP_ErrorLog.txt` naming). **(3)** `export_errorlog_captures_errors` — a Rust-only content
    gate (cross-engine error *message text* is not a Phase-8 axis): a recoverable `DoSimpleMsg` (unknown
    property on an existing element) must reach the exported file (guards the dump against silently
    dropping `Dss::errors`). No existing event-log gate regressed (phase5/phase7_protection green — the new
    markers fire only under `log_events`). lib **726→728** (`logs.rs` unit tests).
  - **audit-code follow-up (independent agent): faithful — no Critical/Major; 2 Minor recorded.** The
    auditor verified the two formatters, the ptr-33/52 dispatch + filenames + solve-guard classification,
    and the three new `LogThisEvent` markers (ordering/guards/clock fields) all match Pascal, the last
    line-for-line vs the oracle golden. **Recorded (surfaced-not-fixed, both tracked-open, out of Phase-8
    scope):** (1) **`Export ErrorLog` content is not oracle-faithful** — Pascal builds each `ErrorStrings`
    entry as `Format('(%d) %s', [ErrorNumber, S])` (`DSSGlobals.pas:275`), a `(errnum)` prefix + exact
    wording our port has no `ErrorNumber` concept to reproduce; the empty dump (the common case) is exact,
    but a non-empty one diverges — a cross-cutting **error-subsystem-fidelity** item, honestly gated
    Rust-side only (never against the oracle); the `logs.rs`/`report.rs` comments were softened to say so.
    (2) **`Reallocating Device List` marker** — my "never appears" claim was overstated: it *can* fire on a
    >1800-device circuit built under `Set Log=yes`; softened the wording + added a `NOT_PORTED` comment at
    `add_ckt_element`. Pre-existing (out of this commit): the incremental-Y "Building Whole Y Matrix --
    using incremental method" variant (`Ymatrix.pas:369`) is not ported — inert on ordinary decks (the
    golden shows the non-incremental line).
  - **audit-tests follow-up (independent agent): sound — goldens are genuine oracle captures, not
    self-comparison; 1 Minor + 1 Nit fixed.** The auditor confirmed the line-count-then-per-line compare
    catches a missing/extra marker and the 3 new Circuit-build markers are genuinely gated (golden lines
    1-3). **Fixed:** (1) **Minor** — the EventLog fixture moved **no** taps (bare IEEE13 daily-3), so the
    per-RegControl `eventlog=yes` produced **zero** tap-change lines and every golden number was an integer
    (the numeric-tolerance path was never exercised) — the docstring's tap-change claim was aspirational.
    Added the swinging load so all three regulators tap (18 `CHANGED … TO <pu>` float lines); the golden is
    now genuinely non-vacuous (33→133 lines) and our engine matches it first-run. (2) **Nit** — tightened
    `export_errorlog_captures_errors` to assert the exact `bogusproperty` token (dropped a dead
    `|| contains("bogus")` disjunct). **Recorded (no fix):** the explicit-filename `Export eventlog <path>`
    arg + `@lastexportfile` bookkeeping is covered generically by the shared `write_export` path (other
    phase8 export tests), not re-asserted here.
- **WP8.3 step 3b — `Faultstudy` (ptr 11, `ExportFaultStudy`), done, gate-green.** The first of the
  step-3b device/reliability exports: a read-only formatter over the **WP7.9-precomputed** per-bus
  short-circuit state (§2.1). `report/export/fault_study.rs` (`fn(&Circuit) -> String`, dispatch arm 11
  via `export_with`, default `EXP_FAULTS.csv`): header `Bus,  3-Phase,  1-Phase,  L-L`, one row per bus:
  - **3-phase** = `max |BusCurrent[i]|` over the bus nodes (the Norton current `Solve mode=faultstudy`
    computed via `Ysc·VBus`, `compute_isc`);
  - **1-phase** = the worst single-phase-to-ground fault: for each node, `YFault = Ysc` (copy) + `GFault`
    (`10000+j0`) at `(iphs,iphs)`, `CMatrix::invert`, `VFault = YFault⁻¹·BusCurrent`, current =
    `|VFault[iphs]·GFault|`;
  - **L-L** = the worst node-node fault: `+GFault` at `(iphs,iphs)`/`(iphs2,iphs2)` and `-GFault` on the
    symmetric off-diagonal (`iphs2` wraps last→first), same invert/mvmult; on a single-node bus
    `iphs==iphs2` ⇒ `VFault[iphs]-VFault[iphs2]=0` ⇒ 0.00 (matches oracle 611/652).
  All `%10f` (FPC `%f` = 2 decimals; `Pad(UPPER(name),12)`). **No global re-solve, no hidden mutation** —
  the `CMatrix` (`support/cmatrix`, `new`/`copy_from`/`add`/`add_sym`/`invert`/`mv_mult`) scratch is
  entirely local per bus. On a snapshot (no faultstudy) `Ysc=None`/`BusCurrent=0` ⇒ all-`0.00` rows, the
  same degenerate output the oracle produces (faithful, not a fake). **Gate:** golden `export_faultstudy`
  on the faultstudy-solved IEEE13 (its own fixture — `FAULTSTUDY_POST=["solve mode=faultstudy"]`, like
  `SeqZ`; the study mutates NodeV so it can't share the snapshot loop) matched the oracle **first-run, no
  fudging**. A rel-tolerance sweep proved the currents match **≤1e-8 rel** (green unchanged at `rel=1e-8`),
  so the honest floor is the `%.2f` **printing floor** (`rel=0`/`abs=0.011` — the `Powers`/`P_byphase`
  discipline; the faultstudy `Zsc`/`Ysc` are pinned to 1e-9·mag by `exec/tests/fault_study.rs`, and the
  `YFault` inversions run the same bit-faithful `CMatrix::invert` on both engines). golden_phase8
  **37→38**; lib **728→729** (the formatter is gated end-to-end; the +1 is the audit-tests degenerate-path
  guard below); `solvable_now` **119** (no migration — the `Export Faultstudy`/`Capacity` corpus decks need
  the full step-3b set + are otherwise blocked; migration at the step-5 completion gate).
  - **audit-code follow-up (independent agent): FAITHFUL — no Critical/Major/Minor; 2 Nits recorded,
    no fix.** Loop-for-loop reconfirmed vs `ExportFaultStudy` (`ExportResults.pas:1526`): header byte-exact
    (double spaces), 3φ = `max|BusCurrent|`, the 1φ/L-L `YFault` stamp/invert/mvmult (Ysc-not-Zsc base,
    0-based↔1-based, the `iphs2` wrap, the single-node `iphs==iphs2`⇒0), `%10f`=2dp + `Pad(UPPER,12)`, ptr
    11 in the #24712 guard set with no Parm2 pre-parse, the read-only `fn(&Circuit)` claim, and the `CMatrix`
    API (incl. `add_sym` adding once on the diagonal, matching Pascal `AddElemsym`'s `if i<>j` guard at
    `Ucmatrix.pas:287`). **Nits (surfaced-not-fixed, not gate-observable):** (1) the magnitudes use
    `Complex64::norm()` (libm `hypot`) vs FPC `Cabs` (naive `√(re²+im²)`, the `cabs_fpc` form) — a ≤1-ULP
    (~1e-13 rel) deviation far below the `%.2f`/`abs=0.011` floor, and consistent with the whole report
    layer's `.norm()` convention (`currents.rs` etc.) — not worth a `TODO(compat)`; (2) the `YFault`/`VFault`
    scratch is reallocated per `iphs` iteration where Pascal allocates once and `CopyFrom`-reuses —
    behaviorally identical (each `copy_from`/`mv_mult` fully overwrites), a minor efficiency-only difference.
  - **audit-tests follow-up (independent agent): SOUND + non-vacuous; 1 coverage gap closed.** Confirmed the
    golden is a genuine pinned-oracle capture (`check_pin` 0.15.7/0.14.5, single-sourced via `meta.json`, Rust
    diffs its own produced file), the `rel=0`/`abs=0.011` floor is the honest `%.2f` printing floor (and is
    *stricter* than the ≤1e-8-rel sweep on the high-magnitude buses — a flat 0.011 A on 650's 2.1 MA is a
    ~5e-9 rel check, so no masking), and the `ExactOrdered`+verbatim-header gate catches a swapped 1φ/L-L
    column / wrong `GFault` / Zsc-base / dropped row. **Closed the one gap** the auditor flagged (the
    `Ysc==None` degenerate branch — the golden always runs a faultstudy first, so it never exercised it):
    added `export_faultstudy_snapshot_ysc_none_is_zeroed` (a Rust-only structural guard — a plain snapshot
    `solve` ⇒ `Ysc=None` ⇒ 1φ/L-L columns structurally `0.00`, header + 4-field rows + ≥2 buses asserted,
    no oracle needed since the zeros are the None-branch by construction). lib **728→729**.
- **WP8.3 step 3c (part 1) — `BusReliability`/`BranchReliability`/`Capacity` (ptrs 37/38/6), done,
  gate-green.** The RelCalc bus/branch outputs + the max-current/rating capacity report.
  `report/export/reliability.rs`: `export_bus_reliability` (dispatch 37 via `export_with`, default
  `EXP_BusReliability.csv`) walks `ckt.buses` writing `CheckForBlanks(UPPER(name))` + `BusFltRate`/
  `Bus_Num_Interrupt`/`BusTotalNumCustomers`/`BusCustInterrupts`/`Bus_Int_Duration`/`BusTotalMiles`
  (`%-.11g` + `%d`); `export_branch_reliability` (dispatch 38 via `export_with_classes`, default
  `EXP_BranchReliability.csv`) does two enabled-PDElement passes — pass 1 = `MaxCustomers` (max FROM-bus
  `BusTotalNumCustomers`), pass 2 = the per-branch `BranchFltRate`/`AccumulatedBrFltRate`/customers/
  `Bus_Num_Interrupt`/`BranchTotalCustomers·Bus_Num_Interrupt`/`BusCustDurations`/`AccumulatedMilesDownStream`/
  `(MaxCustomers-BranchTotalCustomers)·Accum…`/`SAIFI` (`BusCustInterrupts/BusTotalNumCustomers` or 0). FROM
  bus = `terminals[from_terminal-1].bus_ref`. `report/export/capacity.rs`: `export_capacity` (dispatch 6 via
  `export_with_mut`, default `EXP_CAPACITY.csv`) walks PDElements — `compute_iterminal`, `Imax` = max
  `|iterminal[i]|` over `0..nphases`, `%normal`/`%emergency` = `Imax/NormAmps·100` / `Imax/EmergAmps·100`
  (0 if either rating is 0), `Power[1]·0.001` kW/kvar, `BranchNumCustomers`/`BranchTotalCustomers`/`nphases`,
  and `kVBase` of `terminals[0].bus_ref`. The `DSS.SeasonalRating`/`SeasonSignal` branch of
  `CalcAndWriteMaxCurrents` is **kept-deferred** (documented in the formatter) — always the element's own
  ratings, faithful for every non-seasonal deck. **No re-run, no mutation of shared state** — all three read
  the fields `RelCalc` (Pascal `DoLambdaCalcs`) populated. **Gate:** a synthesized deck fixture (PHASE8_PLAN
  §1 — no corpus deck exports these keywords): the proven `relcalc_head_recloser_matches_oracle` feeder
  (src→b1→b2, `faultrate`/`pctperm`/`repair` per line, `numcust` loads, a recloser with `phasetrip`/
  `groundtrip=100000` so the snapshot doesn't trip → `Capacity` reads real 15.8 A / 300 kW currents while
  `RelCalc` still completes) `solve mode=snap` then `relcalc`. A new deck-based golden runner in
  `golden_phase8.rs` (`run_deck_export` + `DeckMeta` = report/fixture/suffix/deck, the no-master `Counts`
  pattern) drives `export_{busreliability,branchreliability,capacity}`. Matched the oracle **first-run, no
  fudging** — the reliability columns are `%-.11g` pure `RelCalc` arithmetic identical on both engines (the
  reliability unit tests pin the same accumulators to ~1e-12), so `rel=1e-8`/`abs=1e-9`; capacity `Imax`/
  kW/kvar keep the 6-sig `EXPORT_REL`, the `%8.2f` %-columns `abs=0.011`, the `%-.3g` kVBase `rel=1e-3`.
  golden_phase8 **38→41**; lib **729** (no unit test — the formatters are gated end-to-end); `solvable_now`
  **119** (no migration — the `Export`-reliability/capacity corpus decks migrate at the step-5 completion
  gate with the rest of step 3c).
  - **audit-code follow-up (independent agent): FAITHFUL — no Critical/Major/Minor behavioral defect; 1
    traceability nit fixed.** Loop-for-loop reconfirmed vs `ExportBusReliability`/`ExportBranchReliability`/
    `ExportCapacity` + `CalcAndWriteMaxCurrents`: headers byte-exact, every column→field mapping + `%d`-vs-
    `%-.11g`/`%10.6g`/`%8.2f`/`%-.3g` typing correct, both BranchReliability compound terms (`BranchTotalCustomers·
    Bus_Num_Interrupt`; `(MaxCustomers−BranchTotalCustomers)·Accum…` with the subtraction as `i32−i32` before the
    `f64` cast) verified numerically (l2 = 25·0.43 = 10.75; (35−25)·1 = 10), the SAIFI / zero-rating divide guards
    exact, FROM-bus/`nphases`/`terminals[0]` indexing correct, the two-pass `MaxCustomers`, the read-only claim, and
    the degenerate no-`RelCalc` all-zero path all faithful; oracle provenance confirmed (the goldens carry FPC width/
    trailing-space artifacts Rust doesn't emit). **Nit (fixed):** the kept-deferred `SeasonalRating` branch was
    documented in prose but lacked the greppable `NOT_PORTED` tag — added `NOT_PORTED (seasonal-rating)` to
    `capacity.rs` (same deferral as `StorageController`'s seasonal target; false-by-default → the non-seasonal
    `NormAmps`/`EmergAmps` path matches the oracle bit-for-bit). Comment-only, gate re-run green.
  - **audit-tests follow-up (independent agent): SOUND + non-vacuous; 1 wording nit + 2 coverage gaps closed.**
    Confirmed the three goldens are genuine pinned-oracle captures single-sourced with the Rust deck via
    `meta.json` (Rust replays `deck`, diffs its own file), that the reports carry **discriminating non-zero**
    values (capacity `Imax` 15.78/10.52, kW 300/200; bus B1 Lambda 0.27; branch l2 SAIFI 0.43 / Cust-Miles 10),
    that `RelCalc` is genuinely exercised (a dropped population would zero the `rel=1e-8` columns and fail), and
    that every tolerance is an honest floor (reliability = pure fault-data arithmetic; capacity `%8.2f`→`abs=0.011`,
    `%-.3g`→`rel=1e-3`). **Nit (fixed):** the capacity docstring mislabeled the `EXPORT_REL=1e-4` two-solves floor
    as a "6-sig printing floor" — reworded to the corpus-wide faer-vs-KLU convention. **Gaps closed (2 Rust-only
    structural guards, no oracle needed — the step-3b `Ysc=None` precedent):** (1) `export_reliability_without_
    relcalc_is_zeroed` — no prior `RelCalc` ⇒ the RelCalc-computed bus/branch columns are all `0` (the branch
    Num-/Total-Customers cols 3/4 are **deliberately not** asserted — they are populated by the meter-zone build at
    solve time, existing Phase-6 behavior the with-RelCalc golden already pins, not garbage); (2) `export_capacity_
    reliability_skip_disabled_pd` — a disabled PD line is filtered out of both `Capacity` and `BranchReliability`
    (the shared fixture has no disabled element, so the golden couldn't see an enabled-filter regression). The two
    residual guard-branch gaps (SAIFI zero-customers, Capacity zero-rating) stay uncovered — both are simple
    reconfirmed-by-audit-code `else` arms with no gate-observable payoff (recorded, not silently dropped).
    golden_phase8 **41→43**; lib **729**.
- **WP8.3 step 3c (part 2) — `Overloads`/`Unserved`/`AllocationFactors` (ptrs 7/8/34), done, gate-green.**
  `report/export/overloads.rs` (the `export_with_mut` PDElement walk — terminal-1 `Cmax`=max phase
  `|Iterminal|`, symmetrical-component I0/I1/I2 via `SymComp`, rows only when a positive rating is
  exceeded; capacitors skipped like Pascal's `CAP_ELEMENT`; the row built **char-for-char** to reproduce
  the degenerate `NormAmps<=0`/`EmergAmps<=0` column shift), `report/export/unserved.rs` (a **mutable**
  Load walk — downcast `&mut Load`, `Unserved`/`ExceedsNormal` recompute vmin-pu from the present solution
  and latch `EEN`/`UE_Factor`; the `Export Unserved u…` pre-parse selects the UE/emergency criterion),
  `report/export/alloc_factors.rs` (immutable Load walk — `Load.<name>.AllocationFactor=`/`.CFactor=` for
  ConnectedkVA/kWh spec loads only, native-case name, no `Enabled` filter matching Pascal `for pLoad in
  Loads`). Five synthesized deck fixtures (`ovl`/`ovl2`/`uns`/`uns2`/`alloc`, PHASE8_PLAN §1). **Audit
  follow-up (audit-code + audit-tests, both fresh agents):** neither found a primary-port bug (audit-code:
  "faithful field-by-field, no regression"; audit-tests: "solid, no masking") — both flagged only coverage
  gaps. Closing them **revealed a real degenerate-path defect**: the `NormAmps<=0` column-shift row was not
  byte-faithful — Pascal's `Format('%8.2f, ', [I1])` trailing `, ` doubles with the branch's
  `Separator+'0.0'` to leave an **empty AmpsOver field** (`… 7.91, ,      0.0`), which the piecewise builder
  collapsed to one `0.0`. Fixed to mirror Pascal's separator placement exactly (I1 written WITH the trailing
  `, `; the `>0` branch rides it, the `<=0` branch doubles it). Two new goldens pin it: `export_overloads_
  unbal` (a 1φ load on a 3φ line → **nonzero I2/I0**, pinning `phase_to_sym` past the balanced all-zero deck;
  + a `normamps=0` line → the shift row) and `export_unserved_ue` (the emergency `Unserved` criterion + a
  healthy load excluded); the `alloc` fixture gained a plain-`kw` load (no-emit branch) and a **disabled**
  ConnectedkVA load (still emits — pins the no-`Enabled`-filter walk). golden_phase8 **43→48**; lib **729**
  (formatters gated end-to-end); `solvable_now` **119** (unchanged — the Export-overloads/unserved corpus
  decks migrate at the step-5 completion gate).
- **WP8.3 step 3c (part 3) — `Sections`/`Profile` (ptrs 51/32), done, gate-green.** `Sections`: the
  meter now **persists** `SectionCount` + `FeederSections` (the `FeederSection` struct moved to
  `energymeter/mod.rs`; `calc_reliability_indices` writes both back on success and zeroes only the
  count on the no-OCP abort, exactly Pascal's field lifecycle), read back by `export_sections` with the
  `meter=<name>` pre-parse (`CompareTextShortest` incl. the empty-ParamName quirk; unknown name → all
  meters). `Profile`: the branch-list voltage profile over each meter's `SequenceList` + zone-build
  `DistFromMeter`, all seven `PhasesToPlot` selector branches (default/all/primary/ll3ph/llall/
  llprimary/explicit-digit `IntValue`) + the `WriteNewLine` layout and the header's appended `Title=…`
  tail. **Ported-immediately gaps** (no-deferral rule): `Set/Get Markercode|Nodewidth` handlers +
  `Circuit.node_marker_code/width` (Circuit.pas 16/1 defaults — Profile echoes them per row).
  **Upstream OOB found:** with ≥2 meters, Pascal's `Bus_Int_Duration` sweep (EnergyMeter.pas:2521)
  walks *all* circuit buses and indexes `FeederSections[BusSectionID]` from *another* meter's zone —
  in-range ids are a deterministic cross-zone overwrite (reproduced); out-of-range ids are an unchecked
  heap read (unpinnable garbage) — Rust skips that write (documented at the guard; NOT a `TODO(compat)`
  — no defined upstream value). Goldens: a synthesized two-meter recloser+fuse deck (`export_sections`,
  `export_sections_meter`) + 7 Profile variants on metered IEEE13 (`run_shared_exports`) + the
  no-RelCalc/unknown-meter structural edges.

- **WP8.3 step 4 — `TSystemMeter` core + the demand-interval (`DI_`) machinery (§2.6), done, gate-green.**
  New `solution/meters/demand_interval.rs`. `MeterStream` reproduces the `MemoryMap_lib` observable
  emission (strings verbatim, doubles `", "`-separated `%-g` 15-sig); the `Append*` re-open paths are
  **proven dead upstream** (no `AppendAllDIFiles` caller in 0.14.5) so files are always created fresh.
  `SystemMeter` (`Clear`/`Integrate`/`TakeSample`/`Reset`/`Save`) sampled in `SampleAll` from
  `GetTotalPowerFromSources` + `Circuit.Losses`; state (+ the whole `EmDiState` class-level DI state)
  lives on `Circuit` so exec and the solve loop share it. Per-meter DI files + the **phase-voltage
  report**: the `TakeSample` pu-voltage accumulators (`VphaseMax/Min/Accum/Count`, the `jiIndex`
  layout, the `|V|/kVBase` 1000·pu quirk) in the zone walk; `DI_Totals`/`EnergyMeterTotals`/`Totals`/
  `SystemMeter` writers; the overload (`DI_Overloads`, incl. the <3-phase per-phase current mapping via
  `MapNodeToBus` ≡ Pascal's FirstBus re-parse) and voltage-exception (`DI_VoltExceptions`, primary + LV
  scans) reports. Wiring: `OpenAllDIFiles` at daily/yearly/peak-day solve head (duty opens nothing,
  faithfully), the daily/duty `finally` close, yearly staying open (pinned by a structural test), `Set
  DemandInterval/DIVerbose=` → `ResetAll`, `Overloadreport/Voltexceptionreport/SampleEnergyMeters=`,
  `CloseDI`, `Clear` flushing open files, and the **`Set year=` `Set_Year` side effects** (DI close +
  clock reset + `ResetAll`) that were missing from the YEAR handler — a real gap the step closed.
  Goldens: 9 DI files from one daily IEEE13 fixture (meter + `PhaseVoltageReport` + all four switches),
  each diffed vs the oracle. **Two upstream-garbage findings documented:** opening the DI files with an
  unbuilt zone makes the oracle render *uninitialized heap memory* as PHV vbase labels (unpinnable —
  the fixture pre-solves so both engines' headers are deterministic; our unbuilt-zone rendering is the
  sane zero-vbase form), and the zone vbase list collects **from-buses only** (IEEE13 ⇒ a single
  4.16 kV PHV group — bus 634 is nobody's from-bus).

- **WP8.3 step 5 — completion gate, done, gate-green.** A release `dss-cli` probe over all 154
  `skipped_unsupported` decks found **49** now compile+run clean (the InvControl daily family +
  PVSystemTest + the two IEEE_519 harmonics decks + RevRegTest + the InverterTechNote pair); moved to
  candidates and live-classified: **47** matched the oracle full-model and migrated. The two IEEE_519
  (harmonicT) decks diverged — the Rust final NodeV was the *fundamental*, the oracle's the last
  harmonic — root-caused to the **silent `Spectrum.CSVFile` no-op** (a Phase-2 `TODO(phase2+)` that
  stored the filename and loaded nothing, so `CollectAllFrequencies` saw only 60 Hz and the harmonic
  sweep never ran): `TSpectrumObj.ReadCSVFile` ported via the WP5.2b deferred-`FileLoad` path (+ a unit
  test), after which **both decks matched the oracle live and migrated**. `solvable_now` **119→168**,
  COVERAGE **35.5%→50.1%**; corpus stays pristine (probe outputs `git clean`ed; CorpusGuard 0 dirt).

- **WP8.3 steps 3c p3 + 4 + 5 — audit-code follow-up (two independent agents, code + tests, parallel
  scoped briefs over `2af06b2^..HEAD`): no correctness bug.** The port was verified loop-for-loop vs
  the cited Pascal (`ExportProfile`/`ExportSections`, the full `TSystemMeter`/DI machinery,
  `Spectrum.ReadCSVFile`, `Set_Year`, the Solve*/DI open-close wiring). Two **LOW** code findings, both
  settled + fixed (`bacaf13`): (1) the `Export Profile` L-L divisor `1732.0` (truncated `1000·√3`)
  reproduced Pascal faithfully but lacked the `TODO(compat)` marker the greppable-cleanup convention
  requires (added, citing `dispatch.rs`); (2) `Spectrum.read_csv_file` walked `str::lines()` instead of
  Pascal's byte-position `(F.Position+1) < F.Size` guard (Spectrum.pas:297) — an **oracle-confirmed**
  divergence (probe: a file `…\n3, 50, 0\n5` reads 2 rows, not 3; a trailing blank line likewise), now
  ported byte-faithfully (+ `read_csv_file_reproduces_pascal_eof_guard`, LF + CRLF, oracle-pinned).
  lib **730→731**.

- **WP8.3 steps 3c p3 + 4 + 5 — audit-tests follow-up (`8d58a8e`): sound, no weakening.** The auditor
  ran the whole gate live (the `corpus_manifest` bijection, the 168-case live oracle compare,
  `golden_phase8`, provenance byte-reproduced) — no weakened assertion, no silent skip, no
  self-comparison, no toy fixture. Three **LOW** coverage gaps + the code auditor's coverage notes,
  closed by four **oracle-pinned** tests (golden_phase8 **54→58**):
  `set_year_closes_di_and_rolls_the_year_directory` (the `Set year=` DI lifecycle, the twin of the
  `closedi` yearly test); `di_set_get_option_echoes_match_oracle` (the five `Get` report-switch echoes
  + `Markercode`/`Nodewidth`, defaults + post-set values probed); `di_overloads_single_phase_mapping_
  matches_oracle` (a synthesized phase-2-only overloaded lateral → the current lands in the **I2**
  column via `MapNodeToBus.node_num`, the discriminating case the 3-phase `di_overloads` golden can't
  catch; new `gen_di_overloads_1ph`); `spectrum_csvfile_loads_through_executive` (the deferred-`FileLoad`
  path + the new EOF guard, end to end). `SolveDuty`'s close-only DI path is symmetric to the tested
  yearly open-then-`closedi` path.

- **WP8.3 — multi-meter `Bus_Int_Duration`: upstream bug filed + in-range reproduction gated
  (`export_busreliability_multimeter`).** The audit-tests note first parked this as "not value-pinned";
  a follow-up investigation (prompted by the user's no-deferral rule) settled it empirically instead of
  asserting. `CalcReliabilityIndices`'s duration loop walks **every** circuit bus (EnergyMeter.pas:2521),
  not the meter's zone, so with ≥2 meters a bus carries a `BusSectionID` from another meter and is indexed
  into *this* meter's `FeederSections` — a genuine upstream memory-safety bug, now written up at
  `investigations/reliability_bus_int_duration_oob_bug_report.md`. **Two regimes, both proven by oracle
  probe:** (a) **in-range** id = a *deterministic* cross-zone overwrite the port reproduces bus-for-bus —
  now **gated** by `export_busreliability_multimeter_matches_oracle` (a no-OOB two-meter deck: both meters
  2 sections, so every foreign read is in range); (b) **out-of-range** id = an OOB heap read, **proven
  nondeterministic** (across 4 fresh processes the first slot past the array reads a stable 0 from zeroed
  slack, later slots read live garbage — 1.5e-311 / 3.1e-314 / 2.5e-290 / 6.0e-118), so safe Rust's
  `.get() → None` skip is correct and there is nothing to pin (not a `TODO(compat)`). `reliability.rs`'s
  comment now cites the proof rather than an unproven "unpinnable" assertion. golden_phase8 **58→59**.

**WP8.3 COMPLETE (steps 1–5 + both audit follow-ups), gate-green.** next = **WP8.4 (Show reports)** —
upgrade the current `Show` no-op to real text reports (shares field logic with the WP8.2 exports).
