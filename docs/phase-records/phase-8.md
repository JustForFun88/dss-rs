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


---

# Phase 8 — completed WP8.* records moved from STATUS.md

> **Archived verbatim from `STATUS.md` on 2026-07-12** (WP8.8 exit + classify Rounds 2b/2c, the
Phase-8-complete roll-up and WP8.4–8.7 detail, and the §1f inline WP8.1–8.4
completed detail). Superseded only by the code and tests.

**WP8.8 Phase-8 exit COMPLETE (2026-07-10), gate-green — PHASE 8 IS COMPLETE.**
All five exit steps ran; per the port-don't-defer rule the sweep also closed the
whole corpus-used executive-command tail on the way out. **Next: Phase 9
exotics — optional; stopping here is a complete usable simulator**
(PORTING_PLAN cumulative note). Remaining named work: actor mode
(`MULTITHREADING_PLAN.md` M2), A-Diakoptics, Pstcalc, WPG.19 (file-backed
`File=` arrays — now the proven sole blocker of the whole ckt24/SolarRamp
family), WPG.20 (MMF save), `RESONANCE_PLAN.md` WP-R1. **`JSON_EXPORT_PLAN.md`
Stage A + Stage B DONE (2026-07-12, branch `wp-json-a`)** — the full AltDSS JSON
export (single object / class batch / whole circuit); only JSON **import** +
`CAPI_Schema` remain (named §6 follow-ups). Details:

- **Step 1 (marker sweep).** Every stale marker settled: **(a)** `show powers
  e` now emits the three exact Pascal whitespace layouts (Sources `%s %4d`, PC
  `:6:1` + `kW   +j  kvar` header + `'  TERMINAL TOTAL '` label) — the WP8.4
  byte-pass `TODO(WP8)` is gone; **(b)** the **AutoTrans `Ntimes = Nphases`
  arms** of `WriteTerminalCurrents` (`ShowResults.pas:604`, with the
  per-terminal `Inc(k, Ntimes)` block-skip), `ShowPowers` case 1 (`:1190` —
  the post-loop `Inc` there is DEAD, terminal 2 re-reads the first conductor
  block; reproduced) and `ShowNodeCurrentSum` (`:3636`) are ported and pinned
  by three new oracle goldens on a well-conditioned AutoTrans snapshot
  (`gen_show_autotrans`; all passed first run); **(c)**
  `max_bus_name_length`'s byte-pass note settled as final (the upstream
  effective-width quirk is nondeterministic → NOT reproduced, UB rule);
  **(d)** the **dynamics-leave `InvalidateAllPCElements`** is ported
  (`set_mode.rs`): Pascal `OK_for_Dynamics` raises `SystemYChanged` on leaving
  dynamics (Circuit.pas:2331) — real since WP7.7 machines + WPG.13/17 GFM have
  mode-dependent YPrims (the old "inert" note predated them); harmonics-leave
  now raises it unconditionally like Pascal; **(e)** 3 stale `TODO(WP7.7)`
  retagged on-demand.
- **Step 2 (cmd_coverage + tail ports).** Newly ported, each oracle-probed
  live and pinned by 9 new unit tests (`exec/tests/exec_tail.rs`, 18 total):
  Enable/Disable (named = the edit path, `*` = bare `Set_Enabled`; unknown /
  DSS_OBJECT classes are SILENT upstream), SetkVBase (kvll/kvln/positional,
  `Bus x not found.` GlobalResult), Losses (`%10.5g` pair off the ACTIVE
  element; the ActiveCktElement-on-New side effect is not reproduced —
  documented, the corpus use is `select`+`losses`), Summary (GlobalResult text
  byte-exact incl. the `Control Mode =` missing space and the literal
  `(**** %%)` arm), Reconductor (`isPathBetween`/`TraceAndEdit` over the
  meter-zone `parent_pd` chain; errors 28701-28707 verbatim), the
  step-solution family `_InitSnap/_SolveNoControl/_SampleControls/
  _DoControlActions/_ShowControlQueue/_SolveDirect/_SolvePFlow`, `var`
  (`DoVarCmd`: define/echo/list + err 28725 — the parser-vars machinery
  already existed), and the pre-circuit utilities Fileedit/Classes/
  Userclasses/CD/DOScmd (live in the PRE-circuit dispatch upstream,
  `ExecCommands.pas:301-335`; the post-circuit case only holds commented-out
  duplicates — Classes walks the Pascal class order via the Dump table) +
  `Set/Get ShowExport` (`AutoShowExport`, GUI-consumer no-op). **Documented
  residual:** the `DSS_CAPI_PM` actor family — commands NewActor/SolveAll/
  Abort/Clone (10 corpus uses) + options ActiveActor/CPU/Parallel/
  ConcatenateReports (115 uses), owner MULTITHREADING_PLAN M2; zero-corpus-use
  query verbs (Voltages/…/Zsc*) and AlignFile/CvrtLoadshapes on-demand;
  DI_plot/CompareCases/YearlyCurves stay loud (upstream NIL-callback UB).
  Fixed a `cmd_coverage.py` misread (the last `cmd::` arm swallowed the
  catch-all and reported not-ported).
- **Steps 3-4 (checks, gate, final classify).** All 12 `wp:"WP8.*"` manifest
  cases `pending:false`; every `report_decks/` deck wired into
  `gen_reports.py`. Full gate green twice (34 binaries, 0 failures; live
  corpus 217 → **226** decks). Final `DSS_LIVE_CLASSIFY` over the 11
  documented cases + the 24 decks the tail ports unblocked: **+9 solvable_now
  (217→226, 67.5% of entry points)** — Dynamic_Kundur (var), K1 Master_NoPV
  (SetkVBase), Paulo ×2 (AddBusMarker), WampServer testcommandline (var),
  Run_NEV (Export/Select/Show), and the three **8500-node runners**
  Run_8500Node/Run_8500Node_Unbal/GFM Run_8500Node_Unbal (Export/Show). 9
  decks re-sorted to `skipped_unsupported` on their TRUE blocker (the whole
  mm-ckt24 family + Run_Ckt24 + Storage-Quasi Run_Demo1 + P174 SolarRamp →
  WPG.19 file-backed arrays; ckt5-7proc → SolveAll actor script, which also
  kills the one-shot oracle). **New findings (honest, kept
  needs_investigation):** the 4 DOCTechNote decks run on both engines but
  exceed the feeder band (~2e-3 V abs ≈ 2.4e-7 rel node V — same order as the
  proven zero-seq floors; root-cause per the no-rationalizing rule before any
  move); IEEE118Bus master joins the oracle_nonconvergence class (the pinned
  oracle itself diverges). Counts: needs_investigation 35→16 (11 documented +
  4 DOC + 118Bus), unsupported 64→50, COVERAGE.md regenerated (bijection 915).
- **CorpusGuard extended RECURSIVE on both sides** (Rust `corpus_live.rs` +
  Python `corpus_guard.py`, the STATUS WP8.8 candidate): run-created files
  inside pre-existing fixture subdirs are now tracked by relative path and
  removed; an incomplete snapshot walk still disables deletion wholesale (the
  war-story bias). Writes OUTSIDE the case-dir tree (manual `dss-cli` runs)
  stay git-backstop territory — TESTING.md updated. Verified live across the
  full gate + all classify probes: `git status tests/corpus` clean.
- **Step 5.** PORTING_PLAN §Phase 8 marked COMPLETE (executed marker + the
  actor-residual pointer); this STATUS record; phase records live in
  `docs/phase-records/phase-8.md`.
- **Audit settlement (2026-07-10, two fresh independent opus agents: 0
  Critical/Major).** audit-code: port faithful across all seven focus areas
  (mode-leave observable proven identical incl. harmonics iteration counts;
  AutoTrans k-indexing; PowersFamily byte-exact; every ordinal/error text
  verified; recursive guard "strictly safer"); its one Minor — the CD/`Set
  DataPath=` non-writable-dir scratch fallback (`DSSGlobals.pas:562-568`) —
  settled as the pre-existing recorded narrowing (not oracle-pinnable,
  corpus-unreachable; now documented at `do_cd_cmd` too). audit-tests: every
  spot-checked oracle pin reproduced live; goldens feature-sensitive; nothing
  loosened. Its two Minors settled by STRENGTHENING: **(1)** the per-family
  `show powers e` whitespace layouts are now byte-pinned
  (`show_powers_elem_autotrans_layout_bytes` — headers/bus-rows/totals
  verbatim vs the golden; degenerate |S|≈0 rows excluded: their `0.0`/`-0.0`
  render sign is faer-vs-KLU residual noise, numerically pinned by the
  tokenizing twin); **(2)** Reconductor now pins all seven error surfaces
  (#28701/#28706 added, both re-probed live). Its Question settled: NEW
  CorpusGuard self-test (`corpus_guard_restores_case_dir_recursively` —
  vendored preserved, overwrite restored, subdir/DI-tree pollution swept).
  Owed follow-up (recorded): root-cause the DOCTechNote×4 live_mismatch
  (~2.4e-7 rel) per the no-rationalizing rule. **→ DONE (FA fix 2, 2026-07-11):
  proven zero-seq common-mode floor, migrated to `large_floating_zeroseq`.**

**needs_investigation burn-down, round 2 — AutoTrans family (2026-07-10,
user-directed "проверь автотрансформатор построчно").** Element EXONERATED
with bit-level proof: on all probed family decks the assembled system Y is
BIT-IDENTICAL across engines at every stage (pins every Transformer/XfmrCode/
AutoTrans YPrim formula AND the whole 8-edit deck sequence — stronger than an
eyeball line-by-line), iteration counts equal at every solve, and the
constant-P load loop is self-consistent. The V gaps are the CROSS-SOLVER
one-shot LU floor of the κ≈1e12 construction (mvasc3=2e6 source + 1e-6 Ω
switches + floating delta tertiary): faer-vs-KLU on bit-identical (Y, I)
reproduces the entire engine gap (5.505e-2 vs 5.472e-2 V at LOW;
scipy+rowscale 6.9e-2). Migration ATTEMPTED and REVERTED: the gate itself
proved the floor contaminates the element channels — the Vsource no-load
current (0.15 A resolved through the 1.7e7 S source) inherits dI = Y_src·dV ≈
6.2e-2 A (40%) and powers dS = V·dI ≈ 12 kVA; admitting that needs ~500×
i_abs loosening = forbidden fudging masking real short-circuit-current
regressions. Family stays documented-skipped (tag `near_ideal_source_floor`,
full proof in each note + TOLERANCE_NOTES §near-ideal-source); counts
unchanged (solvable 194, needs_investigation 34, of which 9 are this closed
class). Also measured for WP-R1: iterative refinement DIVERGES on the
floating tertiary (one step 1.3e-3 → 1492 V — the u·κ≳1 limit live) →
divergence guard added to RESONANCE_PLAN WP-R1.

**needs_investigation burn-down, round 3 (2026-07-10, user-directed "иди
дальше").** Fresh `DSS_LIVE_CLASSIFY` sweep over the 25 remaining cases; 14
root-caused and migrated (solvable 203 → 217, needs_investigation 25 → 11):
**(a)** 4 tier-misclassifications → kind `large` (IEEE13_CDPSM, ckt7 Master,
EPRI_Ckt5-G torn ×2 — their old probes ran at `feeder` bands; all clear the
`large` floors, per the default-classification policy). **(b)** PV
`currentkvarLimit` pair → `large_near_ideal_source` (deliberate Thevenin
Z=1e-8 Ω ≈ 7e7 S; Vsource dI = Y_src·(~3 ulp dV) = 1.9e-4 A while the
PVSystem itself matches). **(c)** NEW tier `large_floating_zeroseq` (`large` +
v_abs 3e-2): TestDDRegulator (amplification 1.4e10), DG_Prot_Fdr (4.2e11), and
the newly root-caused LVTestCaseNorthAmerican Master/SecPar — its 230/13.8 kV
substation transformers are DELTA-DELTA and every distribution transformer is
delta on the MV side, so the whole 13.8 kV system floats in zero-seq: the
entire gap is an identical complex common-mode shift on all MV buses
(Master 2.354e-3 V, per-bus differential 8.2e-7; SecPar 1.94e-3 V, ≤1e-5;
Y pattern identical, worst entry 1.28e-15 rel = libm last-ulp in the
LineGeometry line-constants). **(d)** NEW tier `large_ultra_switch` (`large` +
i_abs 2e-3): ADiakoptics ckt24 + EPRI_Ckt7-G torn pairs — the 1e-8 Ω stitching
pseudo-switch (Y≈1e8 S) turns a <2-f64-ulp cross-engine (V1−V2) difference
into dI = 6.5e-4 A on a 375 A flow (arithmetic bit-floor; the f32-looking
values are the coarse dyadic near-cancellation grid, both engines produce
them). Remaining 11: 10 oracle-blocked (timeouts/non-convergence — nothing to
fix port-side) + ieee9500_base — ROOT-CAUSED on the user's question: the deck
DATA is pathological — its 480 V DER microgrid island sits at the edge of
voltage collapse and `Storage.battery1` (charging) / `battery2` (idling) tip
the snapshot over the fixed-point stability boundary (|V| at M2001-ESS1 grows
~×2000/iteration to NaN). NO engine solves it as vendored (pinned oracle,
official EPRI r3723/r4088/r4133, Rust — all diverge identically); with both
storages disabled it converges on the oracle AND the Rust port in the SAME
121 iterations (battery1=discharging: 130) — an iteration-exact parity data
point on an extremely marginal system. Tag `deck_unsolvable_all_engines`;
deck/scenario bug upstream, nothing to fix port-side. Full proofs:
TOLERANCE_NOTES §floating-zeroseq, §ultra-switch, §near-ideal-source.

**Round 2c — user-directed migration ("перенеси в solvable — мы же всё равно
решаем эти схемы", 2026-07-10).** With the floor proven by decomposition
(round 2b — the sanctioned path for a band change), the 9 AutoTrans decks
moved to `solvable_now` under a new `large_near_ideal_source` tier: `large` +
`v_rel` 5e-6 (family worst 1.46e-6, ×3.4) + `i_abs` 0.1 A (user-set; measured
worst 9.375e-2 A = 94% of band — a future trip is a re-triage signal, not a
widen signal). The Y channel keeps the tight `large` floors and is the
regression sentinel (bit-identical today; the family's unique surface is
YPrim assembly). Counts: solvable 194 → 203, needs_investigation 34 → 25.

**Round 2b — second user challenge ("такие большие значения = баг в порте,
найди и устрани"), per-element decomposition (2026-07-10, Auto1bus-step1,
hex-bit transport).** Every remaining channel closed, no bug exists to fix:
**(1)** substituting the oracle's NodeV bit-exactly into the Rust engine
reproduces all 8 elements' Currents AND Powers **bit-for-bit** (ulp = 0;
Vsource alone at 2.3e-10 A = half an ulp of the ≈3.35e6 A cancelling
`Yprim·V − Iinj` operands) — the entire 6.2e-2 A currents gap is the V vector,
none of it the element/report formulas. **(2)** RHS: 1 of 42 components off by
exactly 1 ulp (phase-2 source `inj.im`, libm sin/cos last bit); same-solver
substitution measures its effect at 1.5e-11 V — innocent. **(3)** residual
parity: ‖Y·V−I‖₂ = 7.1e-2 (KLU) vs 1.3e-1 (faer) — the oracle sits at the same
junk floor, so no refinement could close the family below KLU's own error.
**(4)** third solver: scipy `splu` on the same bits lands 51.6 V from BOTH
engines (uniform on the six floating-tertiary nodes = zero-seq common mode)
with a *better* residual (4.8e-2) than the oracle's V (5.6e-2) — the
mathematically-equivalent solution set spans ~51 V; faer↔KLU's 3.7e-3 V gap is
four orders tighter. Full numbers in TOLERANCE_NOTES §near-ideal-source.

**needs_investigation burn-down, round 1 (2026-07-10, user-directed).** Three
real port gaps found by the triage, fixed 1:1 and pinned by migrating their
decks into the live gate: **(1)** Monitor mode-7 (Storage state) had a header
promising `record_size = 5` with a deferred sample body — every yearly export
panicked in `to_csv` (`StoCtrl_Current_PeakShave`); body now ported from
Monitor.pas l.1298 (PresentkW/Presentkvar/kWhStored/%stored/State). **(2)**
`Dss::regcontrol_tap_numbers` didn't mirror the oracle iterator's
enabled-filter (`RegControls.First/.Next` skip disabled; verified live), so
`BatchEdit RegControl..* enabled=False` decks compared 12 taps vs 0. **(3)**
StorageController parse-time semantics: Pascal `RecalcElementData` ends EVERY
edit line by `MakeFleetList` + `SetFleetToExternal` + `SetAllFleetValues` — the
Rust port deferred that to the first Sample, losing the observable residue (a
controller defined across `~` lines pushes its DEFAULT %reserve/rates onto the
scan-all fleet before `elementList=` shrinks it; SupportRun pins Storage.A..E
at %Reserve=25 from exactly that). Now runs eagerly via
`storage_controller_recalc_fleet` from the executive's edit tail; also fixed
the Pascal local-`kWNeeded` SHADOW in `DoPeakShaveModeLow` (the `kWneed`
property only ever reflects the discharge path — the charge path's value is
local). Also ported en route: the `Wait` executive command (a silent no-op
while `Parallel_enabled` is false — ExecCommands.pas `ord(Cmd.Wait)`; the
StoCtrl deck's `Add_Issues.dss` issues a bare `wait`). The 8
StorageControllerTechNote decks pass the FULL live compare incl. property
parity, and the yearly `StoCtrl_Current_PeakShave/master.dss` (8760-step, EPRI
ckt7 + StorageController I-peakshave) passes the full live compare too — all 9
migrated to `solvable_now` (185→194; needs_investigation 43→34). **Gate
runtime:** the yearly deck cost ~27 min at opt-0, so the dev/test profile now
carries `opt-level=3` overrides for the engine crates + all deps (workspace
Cargo.toml; TESTING.md) with `overflow-checks`/`debug-assertions` explicitly
pinned `true` — release-speed engine, dev-profile safety, same gate command.


---

**Phase 8 — COMPLETE 2026-07-10** (`PHASE8_PLAN.md` —
reporting/exports/Save; branch **`phase-8-reporting`**, branched from the
gate-green Phase-7 tip; WP8.8 exit record in §1). **WP8.1–8.7 COMPLETE + audited** (WP8.5 steps 1–6
incl. `Save circuit` + the classify pass; WP8.6 incl. step 7; WP8.7
ReduceAlgs), **plus GAPS WPG.1 + WPG.14 landed + audited** — records in this
frontier below; `solvable_now` **178 (53.1%)**. Older per-step detail
(chronological) follows. **WP8.5 (Save/Dump)** began with **Dump steps 1–3a +
Save step 4, gate-green** (single-object
`Dump <class>.[name|*] [debug]`: the `report/save/dump.rs` generic base — the
3-kind `TDSSObject`/`TDSSCktElement`/`TPCElement` chain — + `#903`/`#256` errors).
**Dump step 2 (2026-07-05):** the **per-winding / matrix `DumpProperties`
overrides** — Transformer (+ the `debug` `ZB`/`Y_OneVolt`/`Y_Terminal`/`TermRef`
complex lower-triangle block), Line, LineCode, LineGeometry, XfmrCode (each a
co-located `dump_body`; dispatch made `&mut` so the LineGeometry override can walk
`ActiveCond` per conductor, Pascal `LineGeometry.pas:669`). **Three real
byte-fidelity fixes landed:** (1) `fmt_g`'s low scientific threshold was C's
`exp < -4`, but FPC `%g` keeps fixed for one more decade — `exp < -5`
(precision-independent; empirically oracle-verified over prec 8/15,
`TODO(compat)` at `util::fmt_g`); (2) `TTransfObj.WdgCurrents` rendered lowercase
`e` (was `fmt_g`) — FPC `%g` is uppercase `E`, routed through `report::format::g`;
(3) the dump now refreshes each ckt element's `Vterminal` from the solved
`node_v` (Pascal `ComputeVTerminal`) so `WdgCurrents` reads the live solution, not
a stale/zero buffer. Property display-case corrected for Line/Transformer/
LineGeometry tails (LineCode/XfmrCode already correct; matching stays
case-insensitive). golden_phase8 **130→142** (11 new byte-exact dump goldens + 1 `?`-query
regression this step; 16 dump goldens total);
the bare-`dump`/`solution`/aux forms + the 8 remaining overrides (Capacitor/Fault/
VSource/UPFC/RegControl/Monitor/EnergyMeter/Spectrum) are TODO(WP8) step 3.
**Audits (both ran on `53d9006`):** audit-code — one Minor real fix:
`get_all_winding_currents` dropped Pascal's `not Enabled` guard
(`Transformer.pas:1530`), so a post-solve `enabled=no` transformer dumped stale
non-zero `WdgCurrents` where the oracle prints `0` — guard restored + pinned by
`dump_transformer_disabled`. audit-tests — one High coverage gap closed: the Line
`LengthMult = Len` matrix-fold branch (geometry/spacing lines) was unexercised
(both test lines had empty geometry) → added `dump_line_geo` (a Carson-geometry
line, `length=2`, byte-exact — proving Rust's geometry-`Z` embeds length like
Pascal) + `dump_line_switch` (`Switch=Yes`). Follow-up (deeper fix): the same
stale-`Vterminal` read also broke the **`?`-query** path — `? transformer.x.
wdgcurrents` after a solve returned **all-zeros** where the oracle recomputes live
(`6.535435, (-57.242), 312.9242, …`; pre-existing, `&self` getters can't reach
`node_v`). `do_query_cmd` now refreshes `Vterminal` before `get_value`, mirroring
the Dump/Export paths — pinned byte-exact by `query_wdgcurrents_refreshes_vterminal`
(golden_phase8 **141→142**). Property `Save` is unaffected (it emits only
explicitly-set properties, never the read-only `WdgCurrents` result). **Follow-up
(fundamental fix, 2026-07-05, gate-green):** full Pascal+Rust audit proved the
stale-cache class is exactly the `WdgCurrents` family — the whole upstream property
table has only three live-solution reads (`Transformer.WdgCurrents`; unported
`AutoTrans.WdgCurrents`, vterminal-only too; `IndMach012.pf`, which upstream
renders `''` **always** — `PropertyOffset` stays `-1` so the `GetObjPropertyValue`
guard skips the read function even post-solve, probe-proven — Rust's
`SILENT_READ_ONLY → ""` is byte-correct, its doc rationale corrected). The blanket
`?`/Dump `compute_vterminal` (a Pascal-divergent write on *every* element) is
replaced by a declarative Rust-only `PropFlags::READS_VTERMINAL` on the prop def +
one choke point `Dss::refresh_vterminal_if_marked` used by both surfaces (a future
AutoTrans port inherits correctness by setting the flag; an iterminal-needing
property must add a separate marker with iterminal-then-vterminal order — VSource
EMF side effect). New golden `query_indmach012_pf_empty_after_solve` freezes the
post-solve `''` probe (golden_phase8 **142→143**). **Two byte-fidelity gaps + TWO real bugs found + fixed:**
property names now carry the oracle **display case** (`Bus1`/`kV`/`NormAmps`,
Reactor done; matching stays case-insensitive) and `float_to_str` now emits FPC
`FloatToStr`'s **15-sig-fig** form (was 17-digit round-trip; both masked by
`props_roundtrip`'s numeric compare); **audit** then found (Finding 1) `Terminal
Bus Ref` = `0` vs Pascal `-1` for an unset terminal, and — via the sym-components
coverage golden — a **latent Phase-4 reactor bug**: `stamp_series` transposed the
series-stamp bottom-left block (`(j+n,i)` vs Pascal `(i+n,j)`), harmless for
symmetric reactor Y but corrupting the asymmetric induction-motor (`Z1≠Z2`) YPrim /
an unbalanced solve — both fixed, full suite green. golden_phase8 **125→130**; lib
**744→748**. REACTORTest unblocked (migration deferred to Dump completion). The
stamp-bug class is now guarded corpus-wide: **asymmetric live gate** —
`tests/corpus/asymmetric/` (now **27** synthetic decks: every stamping element +
combinations + the per-element midi wave, asymmetric configs, unbalanced solves)
live-compared at micro tolerance by `corpus_live.rs::asymmetric_cases_match_oracle`
(details in §1f). **Controls live gate COMPLETE** (`CONTROL_COVERAGE_PLAN.md`,
steps 1–5 + the midi network gate + the per-element midi wave, 2026-07-05):
**`tests/corpus/controls/`, 37 decks** (LTC/cap/volt-var/storage/dispatch +
protection + metering + combos + the ~94-node midi network), live-compared with
element-state channels (probes / variables / eventlog / ctrlqueue); it caught
**three further real port bugs, all fixed** — two in the
bare-field-write-vs-`Set_YprimInvalid` family (`update_all_storage` and
`InvDispEnv::der_set_nominal`, each dropping Pascal's `SystemYChanged` side
effect) and the InvControl/ExpControl fleet-`nphases` last-member-wins rule
(`InvControl.pas:916`). Details in §1f. **`GAPS_PLAN.md` authored (2026-07-05)** —
the test-blocked-deferral closure plan (the WP7.9 "zero corpus cases → skip"
correction PHASE8_PLAN §1 promised): 14 deferrals inventoried (Monte/LD/AutoAdd/
GeneralTime/Newton/… + the stale "Plot-blocked" TD21+GFM tracked-opens
reassessed), **16 oracle-validated decks** landed as the third live-gate family
`tests/corpus/gaps/` (manifest, every case `pending: true`; two-process
determinism + feature-sensitivity proven on the pinned oracle), WPG.1–17
packaged; executes after/alongside the remaining Phase-8 WPs (§1f).
**PHASE8_PLAN tail refresh (2026-07-05, docs/decks only):** WP8.1–8.4
collapsed to done-markers; WP8.5 steps 3–6, WP8.6 and WP8.7 rewritten to
Sonnet-executable detail from a fresh Pascal deep-read + oracle probes, with
the test fixtures authored up front — 7 golden fixture decks at
`tools/golden/phase8_decks/` (dump3, dump_capacitor, save_forms, interp,
distrib, uuids+csv) and 12 live staging decks at `tests/corpus/gaps/`
(`wp: "WP8.6"/"WP8.7"`: batchedit ×2, the 8 reduce strategies + `Remove` +
midi_reduce), all two-process oracle-validated + feature-sensitive (details
in §1f). **Dump step 3a COMPLETE (2026-07-06), gate-green:** the 8 remaining
leaf `DumpProperties` overrides — Capacitor, Fault, VSource, UPFC, RegControl,
Monitor, EnergyMeter, Spectrum — each a co-located `dump_body` dispatched from
`report/save/dump/overrides.rs` (every ported class's Pascal override is now
wired; only the Phase-9-deferred AutoTrans/GICLine are outstanding). New
`report/save/dump.rs` shared prefix `prefix_pc` (header + `! ENABLED` + Complete
Y-block + `! VARIABLES`, mirroring `prefix_ckt`) and the EnergyMeter-only
`energy_meter_branch_list` helper (the `Branch List:` zone-tree walk needs the
full class registry to resolve each branch/shunt name, which a per-object
`DumpCtx` can't reach — precomputed by the dispatcher like the PC `variables`
block, threaded through a new `DumpCtx.branch_list` field). Verified against the
pinned oracle by direct probe (not guessed): the Fault `MinAmps` double-print
quirk (`NumPropsThisClass = Ord(High(TProp)) = 9 = MinAmps`, so the generic tail
reprints it), the Monitor `// Sec=`/`// BaseFrequency=%.1g` comment lines, and
the Capacitor `SpecType=` bare (no `~`) line all matched the implementation
written from the Pascal source alone — but the probe caught **two real
byte-fidelity bugs**, both fixed: (1) `format::fpc_sci_w` (`Str(v:width)`) had
no floor under `width=8`, so `width=0` (`Sec: 0` in `Monitor.pas`) produced
`"0E+000"` instead of the oracle's `" 0.0E+000"` — FPC's minimum scientific
representation reserves an explicit sign slot AND floors the fraction digits at
1, not 0; fixed (`frac = (width-8).max(1)`, explicit `' '`/`'-'` sign prefix,
mantissa formatted from `v.abs()`) — the one existing caller (`show_convergence`
at width 14) is bit-identical before/after (verified full-suite green); (2)
`util::float_to_str` (`FloatToStrEx`/complex-property renderer) emitted a
lowercase-`e` exponent in its scientific branch (a `TODO(compat)` already
flagged this as unreproduced, "no in-scope dump value reaches scientific
notation" — VSource's `puZIdeal=[1E-6, 0.001]` now does) — fixed to uppercase
`E` (matching `report::format::g`'s existing fixup exactly, probe-confirmed
`1E-6` not `1E-06`/`1e-6`). **Also landed:** the systematic PropDef display-case
pass for all 8 classes (Reactor-convention: `Bus1`/`kV`/`NormAmps`/…), catching
several real mismatches only visible byte-exact — `MVASC3`/`MVASC1`/`X1R1`/
`X0R0`/`puZIdeal`/`BasekV`/`BaseMVA` (VSource), `RefkV` (UPFC), `CMatrix`/`Cuf`/
`States`/`Conn`/`kV` (Capacitor), `3PhaseLosses`/`VBaseLosses`/`Option`/
`Element`/`Terminal`/`Action` (EnergyMeter), `Element`/`Terminal`/`Mode`/
`Action`/`Residual` (Monitor) — plus the universal `BaseFreq`/`Enabled`/
`Spectrum` tails; Fault/RegControl/Spectrum needed only the tail fix (RegControl
and Spectrum were already fully correct). 10 new byte-exact dump goldens
(`dump_{vsource,upfc,regcontrol,monitor,energymeter,spectrum,fault,
fault_gmatrix,capacitor_cmatrix,capacitor_steps}`, golden_phase8 **143→153**)
over the pre-validated `dump3.dss`/`dump_capacitor.dss` fixtures; the Capacitor
pair uses a new `run_deck_dump_exact_masked` (drops the `~ CMatrix=(`/
`~ FaultRate=`/`~ pctPerm=` ASLR-garbage line prefixes from **both** sides —
the golden was captured pre-stripped, the Rust output stripped at compare time,
since Rust's own values are correct and therefore genuinely different text).
**Both audits ran on `2548a17` (opus): no correctness bug in the port.**
**audit-tests — one real Major coverage gap, closed:** the Monitor `// Buffer=`
sample-float render (the fixed `2+Fnconds*4` wrap quirk) was unpinned by any
golden — the fixture monitor never sampled. Root cause turned out to be
structural, not a fixable fixture gap: every solve algorithm that calls
`MonitorClass.SampleAll` also calls `MonitorClass.SaveAll` at its end
(`SolutionAlgs.pas`, every `SolveDaily`/`SolveYearly`/… loop), and
`TMonitorObj.Save` unconditionally resets `BufPtr := 0` — probe-confirmed a
3-step `solve mode=daily` still dumps `// Bufptr=0`. A non-empty buffer is
real Pascal state (only reachable mid-`TakeSample`, between individual solve
steps) the executive can never observe from script — the same "oracle-
unreachable" class as the `show_controlqueue` row-body gap (WP8.4 step 7).
Closed the same way: extracted `render_buffer` and pinned its wrap-width/
`%.1f`/comma-layout behavior with 3 unit tests instead of a golden (lib
**748→751**). Two accepted Minor gaps (no action): Capacitor's garbage-masked
`CMatrix`/`FaultRate`/`pctPerm` render is covered by proxy (`dump_fault`'s
unmasked `FaultRate`/`pctPerm`, `dump_reactor`/`dump_linecode_matrix`'s
`CMatrix` shape) since it can never be oracle-pinned directly (upstream UB);
Capacitor `SpecType=2` (Cuf-only) is untested but provably the same code path
as the tested SpecType 1/3 (only the integer + numeric values differ).
**audit-code — no correctness bug; two Minor `TODO(compat)`/`NOT_PORTED`
tagging gaps, closed:** the Fault `MinAmps` double-print (a real deterministic
upstream bug, byte-pinned by `dump_fault`/`dump_fault_gmatrix`) had no
greppable `TODO(compat)` tag — added. The Monitor buffer-flush model gap
(Pascal's `BufferSize` is a fixed `1024`, flushed-and-`BufPtr`-reset on fill
**or** at the end of every multi-step solve; the port's `mon_buffer` never
flushes, so `Dump` on a solved+sampled monitor would diverge — untested, no
gate fixture samples before dumping) had no `NOT_PORTED` marker — added,
cross-referenced from `render_buffer`'s doc. Two Question-level probe
extrapolations flagged (Monitor's `%.1g`→15-sig `BaseFrequency` render and the
`Sec:0` frac-floor, both confirmed only at the one value the fixture exercises,
`60`/`0.0`) — already honestly hedged in the code's own doc comments as
probe-derived, not asserted as general FPC rules; no further action pending a
fixture that exercises a different value.
**WP8.5 step 4 COMPLETE (2026-07-07), gate-green** (landed via a parallel
worktree agent, merged by cherry-pick): `do_save_cmd` replaces the stub —
the `SaveCommands` `[class,file,dir,keepdisabled]` positional-or-named parse
(`keepdisabled` parsed+ignored, `ExecHelper.pas:780`), `CompareTextShortest`
dispatch in Pascal order; `save circuit` stays a clearly-marked
NOT_PORTED(WP8.5 step 5) stub. `save`/`save meters` = Monitor.Save as a
**documented structural no-op** (the Rust monitor merges MonBuffer+
MonitorStream into one Vec; probe-proven the oracle writes NO monitor file)
+ per-EnergyMeter `SaveRegisters` → `MTR_<name>.csv` (`Year, <year>,` header
+ `"<RegName>",<:0:0>` rows; GlobalResult = the RELATIVE csv name, err 526).
`save voltages` = `Solution.SaveVoltages` (`%-.7g` |V|/angle, GlobalResult =
full path even on write failure, err 488). `save <class>` = `WriteClassFile`
(`Utilities.pas:1134-1210`: default filename = bare class name, NO
extension; create-then-delete-on-0-records; err 718/247). NEW
`report/save/save.rs` serializer pair shared with step 5: `WriteDSSObject`
(`New "Class.name"` always-quoted + ` ENABLED=NO` for a disabled CktElement
+ `HasBeenSaved` mark) / `SaveWrite` (ONLY explicitly-set props in set order
via `DssObjData::next_property_set`, `----` sentinel skip) /
`CheckForBlanks`. `DssObjData` gained `has_been_saved` (persists across save
commands — probe-proven a 2nd `save load` deletes the file; test-pinned).
Load's PropDef display case corrected (oracle `AllPropertyNames` probe).
Probe-pinned quirks: a disabled load serializes `… Enabled=No ENABLED=NO`
(both the property and the WriteDSSObject suffix); Pascal doubles the path
delimiter in GlobalResult (`…\\load`) — not reproduced, path-equivalent,
documented. Goldens: `save_mtr` byte-exact (all 67 registers),
`save_voltages` + `save_class_load` via `compare_export` at rel=0/abs=0
(token-exact — no tolerance needed) over `save_forms.dss` via
`gen_reports.py::gen_save_decks`.
**WP8.6 steps 1–6 COMPLETE (2026-07-07), gate-green** (two parallel worktree
agents, merged by cherry-pick; corpus migration = step 7 still pending, see
below). Step 1: `tools/cmd_coverage.py` (stdlib-only; replicates
`TCommandList` abbreviation matching over the vendored corpus vs the
dispatched set). Dispatch: cmd ordinals MakeBusList=59, Interpolate=62,
Distribute=68, Uuids=86, SetBusXY=91, BatchEdit=95, GISCoords=118 + arms.
Step 2 BatchEdit (`do_batch_edit_cmd`): `regex` crate (workspace dep),
case-insensitive UNANCHORED match, parser position saved/rewound per match
exactly like Pascal, silent (no count message), error 240/267 with
CRLF+CmdString; `tests/corpus/modes` `batchedit.dss`+`midi_batchedit.dss`
flipped `pending:false`, live compare green. Step 3: MakeBusList =
`if bus_name_redefined { reprocess_bus_defs }`; GISCoords = documented no-op.
Step 4: SetBusXY (write-after-every-param, err 28721/28722) + Interpolate
(`solution/meters/interpolate.rs`: `InterpolateCoordinates` +
`CalcBusCoordinates` loop-for-loop; errs 277/283/529; the Pascal `buses[0]`
OOB for a NO_BUS from-bus = safe `.get()` per the UB rule); golden
`export_buscoords_interp` byte-exact (pins the oracle's zone-end order —
c2 first). Step 5 Distribute (`exec/distribute.rs`): the four
`Write*Generators` writers loop-for-loop, errs 721/722, `what=Load`
unconditional `DistLoads.dss` rename, Uniform divides by the FULL load count
incl. disabled (probe-proven `Utilities.pas:1349`), Skip's trailing space;
`how=Random` ported with fresh entropy, never golden-gated; 4 goldens
(Proportional/Uniform/Skip/Load) token-exact. Step 6 Uuids/Export Uuids:
`exec/uuids_cmd.rs` (comma-CSV, err 242, brace-wrap, `=`-names →
AddHashedUuid, circuit/Bus/Class dispatch, StartUuidList reset first) + NEW
`crates/dss-core/src/cim/` seeding the WPG.18 storage shape (UuidHash/
UuidList/UuidKeyList, StartUuidList/FreeUuidList/GetHashedUuid/AddHashedUuid/
GetDevUuid/WriteHashedUUIDs, FPC braced-uppercase GUID render) + lazily-
created v4 UUID slots on DssObjData/Bus/Circuit (`uuid` crate);
`DoExportCmd` calls `DefaultCircuitUUIDs` on EVERY export keyword
(`ExportOptions.pas:188`); `Export Uuids` (keyword 25) byte-exact golden
incl. the 3 auto hashed keys; probe-proven `Text.Result` stays EMPTY after
`export uuids` — reproduced. `cmd_coverage.py` corpus tail after this WP:
15 unported commands (SetkVBase 158, Wait 30, var 20, _SolveDirect 11, …),
11 unported options (TotalTime 40, LoadShapeClass 10, StepTime 10, …).
**Six Opus audits (code+tests × step 4 / WP8.6-part1 / WP8.6-part2) ran on
the pre-merge worktree commits** — findings settled below (§audit follow-up).
**WP8.5 step 3b (whole-circuit / aux Dump forms) COMPLETE (2026-07-07,
`34b15a1`), gate-green + audited.** Bare `dump`/`dump debug` (Circuit.DebugDump
header + every CktElement/DSSObj/Solution with Leaf=TRUE), `dump solution`
(`Solution.DumpProperties`, the full `Set …` option list incl. the
Leaf-gated lines + the Complete lower-triangle system-Y block), `dump
commands` (`DumpAllDSSCommands` over a **generated help catalog** —
`tools/golden/gen_help_catalog.py` parses the pinned wheel's gettext `.mo`
(1697 entries) into `report/help_catalog.rs`), `dump buslist`/`devicelist`
(**THashList port**: `MakeHash`, bucket layout, `DumpToFile`'s three
sections — the buslist-prints-only-LINEAR rule *derived* from `HashList.pas`
(TAltHashList vs THashList), not special-cased), `dump alloc*`
(`DumpAllocationFactors`: kW/PF-spec loads print NOTHING, probe-proven). 7
new byte-exact dump3 goldens (golden_reports 166→173). **The load-bearing new
machinery: a loop-for-loop port of FPC 3.2.2 Grisu1 float→ASCII**
(`flt_core.inc` `str_real` + `FloatToStrFIntl` ffGeneral post) as the
`fmt_g`/`float_to_str` backend (TODO(compat) documents the half-away-from-zero
`round_digits` re-round). **Audits (opus, both fresh agents): no correctness
bug.** audit-tests Major — the "20k-value oracle-validated battery" was an
uncommitted throwaway → **settled by committing a permanent FPC-RTL battery**:
`tests/golden/fmt_battery.csv` — 13 198 deterministic f64 bit patterns
(subnormals, 10^±320, the exp<-5 fixed/sci threshold band, tie-bands,
digit-count boundaries, seeded random) × 7 render forms (`FloatToStr`,
`fmt_g` sig 2/5/8/15, `Str(v:0/14)`) rendered by the real FPC 3.2.2 RTL
(`ppcrossx64`; generator `tools/fpc/fmt_battery/`) — **92 386 renders, 0
mismatches**, gated by `crates/dss-core/tests/fmt_battery.rs` (also closes
the subnormal/extreme-exponent Minor). Accepted + recorded: the three
whole-circuit dump goldens pin `Set editor=NotePad.exe` (Windows-pinned
project — a platform guard only if the suite ever runs cross-OS); `Set
LDCurve=` renders empty (LD mode NOT_PORTED → WPG.3, marker present);
`help_catalog.rs` is pinned transitively via `dump3_commands` (standard
generated-golden convention). Also landed: `tests/golden/feeders_controlsoff`
fixtures un-tethered from git-ignored `.inputs` (redirects → the tracked
`tests/corpus/electricdss-tst` mirror + generator re-pointed) — fixes
`golden_feeders` in fresh worktrees.

**Parallel-fleet round (2026-07-07): five isolated-worktree agents (WP8.5
step 5, WP8.7, WPG.14, WPG.1, WP8.6 step 7), merged by cherry-pick; each
branch got its own opus audit-code+audit-tests pair on the pre-merge commit,
findings settled by the coordinator (below). Full gate re-verified green on
the merged head (fmt/clippy/`cargo test --workspace`, 32 binaries, 0
failures, live corpus at 178 decks).**


---

**WP8.5 step 5 (`Save circuit` + round-trip gate) COMPLETE (2026-07-07),
gate-green + audited.** `TDSSCircuit.Save` (Circuit.pas:2409-2988) +
`WriteVsourceClassFile`/`WriteClassFile` + `TEnergyMeterObj.SaveZone`
(EnergyMeter.pas:2585-2807) as `exec/save_circuit.rs`, replacing the step-5
stub: whole-circuit multi-file emit in verbatim Pascal order (library
classes → Vsource `Edit`-first → SaveFeeders per-enabled-meter zone subdirs
(branch→Branches/Transformers, shunt→Loads/Generators/Capacitors/Shunts,
controls written after their element for Xfmr/Branch/Gen/Cap but NOT loads,
the load-allocation `allocationfactor` side effect, empty files
deleted+unlisted) → SaveDSSObjects → BusVoltageBases (`! CalcVoltageBases`
commented, oracle-proven) → BusCoords (always created) → Master.dss with
relative Redirects). `DSSSaveFlag` enum faithful; command path = empty set,
flag-gated branches dormant. **Gap ported in-step:** the Transformer
`SaveWrite` override (Transformer.pas:1045) — per-winding-scalar→
array-property rewrite (`elements/pd/transformer/save.rs`); the generic
serializer had dropped winding 1. **Gate:** `tests/save_roundtrip.rs` —
IEEE13/37/123 solve→save→clear→recompile→resolve, voltages ≤1e-6 rel by node
name + warm-re-solve iteration count exact; `save_forms` structural file-SET
test (oracle-probe-proven set incl. the `em1/` feeder subdir). **Audit
settlements:** `New Circuit.<name>` now emits the lowercase `LocalName`
(Circuit.pas:386; the port had used `CaseName` — behaviorally cosmetic,
fixed for fidelity); the SaveZone path gained a **numeric** gate (snapshot
voltage round-trip on `save_forms`, closing the audit-tests Major that the
feeder path was structurally-only gated); recorded accepted notes — the
Transformer rare per-winding scalar tail (RdcOhms/RNeut/…) is unexercised by
the gate decks, the file-set baseline is a dated oracle probe without a
committed capture script, and the round-trip is self-consistency by design
(§2.4). The step's original audit-code agent returned an empty result — a
fresh audit-code re-run covers this code (findings settled in the follow-up
records here).

**WP8.7 (ReduceAlgs full) COMPLETE (2026-07-07), gate-green + audited.** New
`exec/reduce.rs` (~1200 lines, executive-level — reduction mutates through
the edit machinery): `TLineObj.MergeWith` (Line.pas:1631-1840; sym-component
sets through the property-edit path so SetDouble scaling + side effects
reproduce; matrix-series element-wise `(Z1·len1+Z2·len2)/TotalLen`;
matrix-parallel `Len/2` "assume equal" as a write-only no-op TODO(compat));
all eight `ReduceAlgs.pas` strategies + `IsShortLine`; `ReduceZone`
dispatch (both `reduce_deferred_msg()` stubs gone); `Set KeepList=`
(`DoKeeperBusList`); the `Remove` command (`cmd::REMOVE=107` →
`DoRemoveBranches` incl. the KeepLoad `Eq_<elem>_<frombus>` equivalent
load). Gate: all ten `tests/corpus/modes` reduce decks `pending:false`, live
compare green at micro tolerance (merged names `l1~l2`/`s1~s2`/`b1||b2`,
disabled partners, node counts 15→12/15→9/16→14/94→88, KeepList block, x2
skip, `Load.eq_l2_b2`); `golden_autoadd_reduce` extended with an
oracle-captured post-Reduce re-solve. **Audit findings, settled:** (1)
*Major:* the `kVBase<=0` fallback read the never-refreshed `bus.vbus` →
now reads the live `NodeV[RefNo[1]]` (the `UpdateVBus` equivalent;
upstream's `VBus=NIL` read is a NIL-deref — UB, gated per the CLAUDE.md
rule). (2) the laterals shunt re-bus loop now runs **unconditionally**
(Pascal `:505-513` sits outside the KeepLoad block: `Bus1= kV=1` when
KeepLoad=No, BusName stays empty). (3) `MergeWith` sets
`YPrimInvalid`+`SystemYChanged` up front (CktElement.pas:240 setter
semantics). (4) control repointing now replays the full `element=` property
edit (Line.pas:1849 `ParsePropertyValue`) so the stored ElementName renders
right in Save/Dump. (5) TODO(compat): the parent cap/reactor scan checks
**only the parent's FIRST shunt** — upstream mixes cursors
(`ParentNode.FirstShuntObject()` but `PresentBranch.NextShuntObject()`,
ReduceAlgs.pas:200-210) and `TDSSPointerList.Add` leaves `ActiveItem=Count`,
so the first `Next` overflows → NIL (proven from `DSSPointerList.pas`
sources); a cap/reactor at parent-shunt position ≥2 does not block the
merge upstream — reproduced. (6) `Some(NO_BUS)` to-bus guard in dangling
(Pascal `ToBusRef>0`). Recorded (no action): `UpdateControlElements` has no
runtime deck coverage (synthesized control-on-merged-line deck = a WP8.8
sweep candidate); the manifest "disabled partner" expectations are asserted
via node_order/Y/voltage equality rather than an explicit enabled-flag
channel.

**WP8.6 step 7 / WP8.5 step 6 (corpus classify/migrate) COMPLETE
(2026-07-07), gate-green.** Live-reprobed all 59 `skipped_unsupported` decks
tagged with the landed verbs. `solvable_now` **169→178 (53.1%)**: the 2
Dump-blocked decks (REACTORTest, Split-Phase_IEEE_TIA), 4 SetBusXY
IEEE-TIA-LV masters, 3 ADiakoptics Torn_Circuit feeders
(MakeBusList/GISCoords) — all pass the always-on live gate (178-deck run
green). 28 candidates retagged to their real remaining blocker (var, GFM
combi ×18, file-backed arrays, SeasonRating/Signal, ControlSignal, CIM100).
22 moved to `skipped_needs_investigation` as **real newly-visible findings**
(not fixed — manifest-only WP): AutoTrans Auto1bus/Auto3bus + several EPRI
meshed Torn_Circuit decks diverge above tolerance (reproducible, must be
root-caused per the no-rationalizing rule); 2 TnDSystem decks hit oracle
non-convergence; `StoCtrl_Current_PeakShave` hits a **real engine panic**
(index out of bounds, len==idx==17520 — a yearly/DI buffer sized hourly,
indexed finer); 8 StorageControllerTechNote decks expose a **verified engine
gap** — `exec/view.rs::regcontrol_tap_numbers()` does not skip disabled
RegControls, unlike the oracle's `RegControls.First/.Next` (live-probe
proven; likely one-line fix, tag `regcontrol_enabled_filter`). COVERAGE.md
regenerated (bijection 915 holds).

**Final audit wave (2026-07-07, three fresh opus agents): the Save-circuit
CODE audit (re-run — the fleet-round attempt had returned an empty result),
an audit of the coordinator's own follow-up commits, and an audit-tests pass
over tests/fixtures/manifests. Findings settled:**
- **Save-circuit code: faithful 1:1 on the reachable path, no Critical/Major**
  (independently re-verified: body order, clear-flags, Edit-first vsource,
  SaveZone routing + control emission + allocation side effect, the
  Transformer per-winding rewrite incl. the `PrpSpecified`-guarded scalar
  tail, relative-Redirect math, flag ordinals, the lowercase-name fix).
  Fixed from its Minors: a mid-save file-write failure no longer reports the
  "Circuit saved" GlobalResult (Pascal `Success` chain / err-434 semantics —
  the first error text is returned instead); the simplified
  `Path::is_absolute()` dir resolution (Pascal's bare-root `\foo` drive-
  prefixing / drive-relative `C:foo` cases dropped) is now documented at the
  resolution site as a recorded narrowing. Recorded Questions: the serializer
  renders via live `get_value` where Pascal emits the cached
  `PropertyValue[i]` (a WP8.5-step-4-wide decision — revisit at WP8.5b, whose
  property-parity sweep compares exactly this surface); the fleet-control
  in-zone omission stays documented-unobservable.
- **Coordinator follow-ups: one real Major found + fixed** — the f32 patch
  left `read_csv_file`/`read_pq_csv_file`/`read_dbl_file` without Pascal's
  head-of-reader `UseFloat64` (`LoadShape.pas:1044/:970/:1220`), so a stale
  `sP` from an earlier `sngfile=` would keep winning the lookup after a
  CSV/Dbl re-read (`sngfile=… csvfile=…` in one edit) — reset added to all
  three + regression pin `csv_after_sng_ends_single_storage`. Also fixed:
  stale GrowthShape doc comments still claiming CSV year-rounding; MakeLike
  now drops `s_h` for a fixed interval (symmetry with `dH`). The reduce.rs
  fixes were independently re-derived and confirmed (incl. the
  `DSSPointerList` cursor analysis behind the first-shunt-only TODO(compat)).
- **audit-tests: verification strengthened, nothing weakened** — fmt_battery
  unconditional + fully parsed; the f32 feature-sensitivity claim
  independently confirmed (f32-off produces different bits and fails the
  gate); all shape fixtures regenerate byte-exact from the committed
  generator (a working-tree CRLF artifact of `core.autocrlf` noted for any
  future byte-compare hygiene check); classify manifests internally
  consistent (bijection 915, solvable_now 178) — the `279f703` commit
  message's stale 168→177 counts are a traceability nit only; parking the
  root-caused `regcontrol_enabled_filter` gap + the 17520 engine panic in
  `skipped_needs_investigation` is recorded as deliberate (tracked above,
  next-step candidates), not a silent skip.

**next = WP8.5b (corpus property parity addendum — design pre-approved,
§PHASE8_PLAN WP8.5b; it also revisits the cached-vs-re-rendered SaveWrite
question) → WP8.7/WP8.5 residual sweep items above → WP8.8 phase exit;
GAPS_PLAN WPGs continue in parallel (WPG.2/3 next by tier).**
**WP8.4 (Show) steps 1–16
gate-green** (Buses/Losses/Taps/Voltages/Currents/Powers seq+elem + Elements +
Result/EventLog/Ratings/Variables/Mismatch/monitor + step 7: Convergence/Y/
controlqueue/kvbasemismatch + step 8: Meters/Generators register tables +
step 9: Overloads/Unserved + step 10: FaultStudy + step 11: Yprim (+ the
`Select` command / active-ckt-element surface) + step 12: Loops/Zone (the
EnergyMeter zone-tree pair) + step 13: Controlled (the PD→controls map via the
new `CktElement::controlled_element` accessor) + step 14: LineConstants (the
LineGeometry Carson R/jX/L/C matrix dump + seq-component summary, two files) +
step 15: busflow (`ShowBusPowers` seq+elem, reusing the extracted per-bus/
per-element voltage/current/power helpers) + **step 16: Isolated/Topology** (the
circuit-wide CktTree pair, over the new `solution/topology.rs` `GetTopology`/
`GetIsolatedSubArea` builder) + dispatcher; **~32 `Show` reports ported**). **All
real `Show` reports are now ported** (incl. `deltaV` — the step-4 delta-winding
node_ref deferral is **resolved**: later Phase-7 transformer work fixed the layout,
so `Show DeltaV` now writes the `Transformer.SUB` rows the oracle does). Remaining
`Show` no-ops: only `autoadded`/`QueryLog` (headless FireOffEditor); the `#24700`
unknown-keyword error is ported. Full Phase-8 detail is in **§1f**; the current
frontier:

- **WP8.1 COMPLETE** (dispatch skeleton + output-path machinery + `Export Counts`
  + the `compare_export` golden harness).
- **WP8.2 COMPLETE** — the solution/power/symmetrical-component/per-terminal/matrix
  export families (`Voltages`…`Currents`…`Yprims`/`Y`/`SeqZ`/`Summary`/`Result`) +
  the mutable element-walk infra; the completion gate migrated the `Export`-unblocked
  corpus (`solvable_now` **88→119**, COVERAGE **26.3%→35.5%**) + landed the Rust
  `CorpusGuard`. The "9 decks hang" and "live-gate flake" tracked-opens are both
  **RESOLVED** (§1f Issue-1/Issue-2).
- **WP8.3 COMPLETE (steps 1–5 + both audit follow-ups), gate-green** — the
  device/meter/reliability/log exports (`Monitors`/`Meters`/DER/`EventLog`/
  `Faultstudy`/`BusReliability`…/`Sections`/`Profile`) + the `TSystemMeter` core
  and the full demand-interval (`DI_*`) file machinery + its `Set`/`Set year=`
  wiring (§2.6). The completion gate migrated `solvable_now` **119→168** (COVERAGE
  **50.1%**), incl. fixing the **silent Spectrum `CSVFile` no-op** two IEEE_519
  harmonicT decks exposed. The two independent audits found **no correctness bug**;
  two LOW code findings fixed (`Export Profile` `1732.0` `TODO(compat)` marker; the
  `Spectrum.read_csv_file` byte-position EOF guard, oracle-confirmed) + five
  oracle-pinned coverage tests (incl. the multi-meter `Bus_Int_Duration` cross-zone
  bug — filed upstream + in-range regime gated). golden_phase8 **59**; lib **731**.
  Detail in §1f.
- **WP8.4 (Show reports) — step 5 COMPLETE, gate-green** (audited with step 6, see
  the next bullet). The `do_show_cmd` dispatcher (`ShowOptions.pas` option/solve-guard) + the new
  `report/show/` module of fixed-width text formatters: `Show Buses`/`Losses`/`Taps`
  + `panel`→#999 (step 1); `Voltages` seq (step 2); `Currents`/`Powers` seq (step 3);
  `Voltages` node/elem + `Currents` elem + `Elements` (step 4); **`Powers` elem +
  `Result`/`EventLog`/`Ratings`/`Variables`** (step 5). Remaining unported keywords
  (meters/zone/topology/lineconstants/yprim/y/faults/mismatch/…) stay *silent*
  headless no-ops. Shared machinery: `format.rs` `Pad`/`PadDots`/width formatters, the
  whitespace+comma `compare_export` tokenizer (`sep: ' '`) + `ColSel::AfterToken` +
  `GateSpec::MinCols` (PF-of-degenerate-power gate). **Key finding (step 5):**
  `MaxBusNameLength` is an **inconsistent per-report backend quirk** (`ShowVoltages`→12,
  `ShowPowers`→~5, even in isolation) — *not* a consistent floor, so the step-4
  floor-12 `TODO(compat)` was withdrawn; instead the comparator **drops pure dot-run
  tokens** (`PadDots` padding carries no data), making the gate immune to the quirk,
  and `max_bus_name_length` keeps the clean source value. `MaxDeviceNameLength=0`
  `TODO(compat)` stands. golden_phase8 **59→74**.
- **WP8.4 (Show reports) — steps 5–6 COMPLETE + audited, gate-green.** Step 6:
  `Show monitor` (`TranslateToCSV`, corpus×24 — reuses the monitor CSV; golden via the
  daily monitor fixture) + `Show Mismatch` (`ShowNodeCurrentSum`, per-node KCL sum).
  golden_phase8 **74→78**. **Both audits ran on steps 5–6** (`263898f`): **one real
  bug found + fixed** — the `Show Result` output filename (`Result.txt` → the spec's
  `Result.csv`, `ShowOptions.pas:432`) + its/`EventLog`'s `GlobalResult` side-effect
  (`write_show_global`). The audit-tests-recommended **`Show Variables` golden
  (generator fixture) surfaced a real Phase-7 port bug**: the generator's `w0` (base
  angular frequency) was 0 pre-dynamics, so the classic `Frequency` state var read 0
  not 60 — fixed by initialising `w0 = TwoPi·base_frequency` at construction
  (`Generator.pas:986`); safe across the full suite (dynamics `InitStateVars`
  overwrites `w0` anyway). Test follow-ups: `Show Mismatch` upgraded from a structural
  smoke test to a **value golden** (pins `Max Current` via the new `ColSel::FromEnd`,
  gates the `Current Sum`/`%error` faer-vs-KLU residuals via `GateSpec::Mask` — those
  are inherently not cross-engine-pinnable); new `show_variables`/`show_result`
  goldens; the powers PF gate doc corrected (`min(kW,kvar)`, not kVA) + tolerance
  tightened `abs 1e-3→2e-4`; a powers code-1 whitespace-variant `TODO(WP8)`
  breadcrumb. **All 19 `Show` goldens converted to exact equality** (`rel=0, abs=0`):
  against a fixed oracle-bytes golden the deterministic Rust output is byte-identical,
  so the prior fuzzy tolerances only hid that — 14 are fully exact, the other 5 gate
  out only the genuinely-non-comparable cells (near-zero faer-vs-KLU cancellation
  residuals V0/V2/I0/I2/I1/`|I|`/kvar, skipped incl. exact-0 via `GateSpec::MinCols`)
  + one real `%10.5f` rounding-boundary straddle (`mismatch` Max Current → the
  `1.1e-5` render floor). **DeltaV deferred** (silent no-op, `TODO(WP8)`):
  `WriteElementDeltaVoltages`' `NodeRef[i+NCond]` cross-terminal read yields 0 rows for
  the delta-primary
  `Transformer.sub` where the oracle writes 3 — the delta-winding node_ref layout
  needs investigation (deltaV is not corpus-used).
- **WP8.4 (Show reports) — step 7 COMPLETE, gate-green** (the diagnostic/matrix
  cluster): `Show Convergence` (`Solution.WriteConvergenceReport` — per-node saved
  error/`|V|`/`Vbase` + Max Error footer, over the existing `error_saved`/
  `vmag_saved`/`node_vbase`/`max_error` solution arrays), `Show Y` (`ShowY` — the
  assembled system Y, lower triangle by columns, `[row,col] = G + jB` `%13.10g`,
  reusing `system_y_csc` + the column-major-lower-triangle order that matches KLU's
  `GetTripletMatrix`), `Show controlqueue` (`ControlQueue.WriteQueue` — the pending
  action queue, drained to the header alone after a converged snapshot; new
  `ControlQueue::queue_rows` accessor), and `Show kvbasemismatch`
  (`ShowkVBaseMismatch` — loads/generators >10% off their bus base, LN/LL forms +
  the per-family header). New `report/show/matrix.rs`; the three diagnostics in
  `report/show/diagnostics.rs`; dispatcher arms 4/26/27/30 in `exec/report.rs`. New
  `format::fpc_sci_w` reproduces FPC `Str(v:width)` (scientific, `width-8` frac
  digits, ≥3-digit exponent) for the convergence `:14` columns. golden_phase8
  **78→83**: `show_{convergence,y,controlqueue,kvbasemismatch}` on solved IEEE13 +
  the synthesized `show_kvbasemismatch_vals` (4 kV-mismatched load/gen elements
  exercising both LN/LL forms + the GENERATOR block). **All 5 are exact equality**
  (`rel=0, abs=0`), incl. the convergence `|V|` column: the preemptive `rel=1e-6`
  "7th-sig printing floor" shipped with step 7 was never exercised — the produced
  file is byte-identical to the oracle golden (faer-vs-KLU voltage gap is orders
  below the 7-sig print step) — so it was tightened back to exact per the
  no-unproven-floors rule; Y's G/B are byte-identical (bit-exact assembled
  Y on the LineCode-based IEEE13). **Remaining unported Show keywords** (all still
  *silent* no-ops, `TODO(WP8)` in `do_show_cmd`): `Controlled` (needs the
  `ControlElementList` accessor), `Meters`/`Generators`/`Zone`/`Overloads`/
  `Unserved`, `FaultStudy`, `Yprim` (needs the active-element surface + its
  non-`CircuitName_` filename), `LineConstants`, `Isolated`/`Loops`/`Topology`
  (CktTree walks), `busflow` (`ShowBusPowers`), `autoadded`/`QueryLog` (headless
  FireOffEditor no-ops), `deltaV` (deferred, above).
  **Both audits ran on step 7.** **audit-code — one real Minor bug found + fixed:**
  `Show Convergence` (arm 4) + `Show controlqueue` (arm 27) were setting
  `@lastshowfile`, but Pascal dispatches them *inline* with only `FireOffEditor`
  (`ShowOptions.pas:187-197`/`404-414`) and does **not** — fixed via
  `write_show_named(…, set_last=false)`, the `write_show` doc corrected, and pinned
  by the new `show_lastshowfile_semantics` test (Y/kvbasemismatch set it;
  Convergence/controlqueue don't). **audit-tests — one real Major gap + closed:**
  `show_controlqueue` only exercised the drained (empty) queue, so the new
  `queue_rows`/row-formatting path shipped uncovered. The row body is **unreachable
  via the executive** — a `show controlqueue` after any `solve` always sees a
  drained queue (probe-proven, incl. the low-level `SolveNoControl`+`Sample` split),
  so no oracle golden can reach it; covered instead by a `control_queue_row_format`
  unit test against the Pascal `WriteQueue` format. The Sec `%-.g` precision (FPC
  empty-precision `ffGeneral`) is unverifiable against the always-drained oracle
  queue → documented `TODO(compat)`, 6-sig stand-in flagged for the WP8.8 byte pass.
  Also refactored `run_feeder_show` to locate the report by its fixed
  `<CaseName_><suffix>` name in the datapath (the oracle-generator glob) — robust to
  the `@lastshowfile` split and still filename-pinning. Tracked-not-fixed (audit
  notes): `show_kvbasemismatch` (plain IEEE13) is near-vacuous but backstopped by
  `_vals`; the `show_convergence` Error column is exact-compared (`rel=0`) —
  honest today (all `0.00000`), a robustness note only.
- **WP8.4 (Show reports) — step 8 COMPLETE, gate-green** (the register tables):
  `Show Meters` (`ShowMeters` → `EMout.txt`, dispatcher arm 9) and `Show Generators`
  (`ShowGenMeters` → `GenMeterOut.txt`, arm 8) — each element's accumulated
  energy-meter registers in Pascal's fixed-width layout: the register-name legend
  (`Reg i = <name>`, meters only), the per-register column header, then one
  `%10.0f`-per-register row per **enabled** element (a disabled element emits only
  its trailing newline — Pascal writes the newline outside the `if Enabled` guard;
  the blank line is stripped by the comparator). New `report/show/meters.rs` reuses
  the WP8.3 register access (`EnergyMeter::register_names`/`registers`, the
  class-fixed `GEN_REGISTER_NAMES`) and the WP8.3 register fixtures
  (`REGISTER_A_POST` metered IEEE13, `REGISTER_B_POST` g1/g2 + disabled g3). golden
  `show_meters`/`show_generators` (golden_phase8 **83→86**), both **exact equality**
  (`rel=0, abs=0`): the register values are the same daily-solved meter/generator
  paths `corpus_live.rs` + `export_meters`/`export_generators` already pin (both
  render `%10.0f`), so every rounded integer cell is byte-identical; g3's absence
  pins the enabled-filter. **Remaining unported Show keywords** (all still *silent*
  no-ops, `TODO(WP8)`): `Controlled`, `Zone`/`Isolated`/`Loops`/`Topology` (CktTree
  walks), `Overloads`/`Unserved`, `FaultStudy`, `Yprim` (needs the active-ckt-element
  surface + its non-`CircuitName_` filename), `LineConstants`, `busflow`
  (`ShowBusPowers`), `autoadded`/`QueryLog` (headless FireOffEditor no-ops), `deltaV`
  (deferred). **Both independent audits ran (`059c2aa`): no correctness bug** — the
  two formatters reproduce `ShowMeters`/`ShowGenMeters` field-for-field (banner/
  legend/header widths, the enabled-filter, the disabled-element blank line, the
  empty-list guards, `%10.0f`), and the dispatcher wiring (filenames/solution-guard/
  `@lastshowfile`) is faithful. Two follow-ups landed (both **test-only**): **(F1)**
  the generator `$` register sits **exactly** on the `%10.0f` half-boundary
  (`7.5 → 8`) — documented at the `show_generators` pin as *stable*, not a
  knife's-edge: it derives from the stiff clean `kWh = 300` (`model=1` holds
  P = 100 kW → ∫P dt = 300 exactly, bit-identical both engines, no faer-vs-KLU
  residual) and `7.5 → 8` under both round-half-to-even and round-half-away, so the
  exact compare cannot straddle; **(F2)** three coverage goldens added —
  `show_meters_multi` (two **partitioned-zone** meters: em1 stops at em2, so the two
  rows carry distinct per-zone registers → pins the legend-emitted-once-from-`meters[0]`
  + per-row-registers path) and `show_meters_none` / `show_generators_none` (the
  empty-list banner branches: `No Energymeter Elements Defined.` / the two-line
  Generators banner). golden_phase8 **86→89**. Left as noted (not a bug): the
  tokenizer is field-width-blind (the whole Show family's known limit, F3).
- **WP8.4 (Show reports) — step 9 COMPLETE, gate-green** (the overload/unserved
  pair): `Show Overloads` (`ShowOverloads` → `Overload.txt`, dispatcher arm 16) and
  `Show Unserved` (`ShowUnserved` → `Unserved.txt`, arm 17) — the symmetrical-
  component PD-element overload report (one row per enabled non-capacitor PDElement
  whose terminal-1 max phase current exceeds its normal/emergency rating: `Element
  Term I1 IOver %Normal %Emerg I2 %I2/I1 I0 %I0/I1`, `%3d`/`%8.1f`/`%8.2f`) and the
  Loads over their voltage-drop criterion (`name bus kW EEN UE`, `%8.0f`/`%9.3f`; a
  nonempty trailing param → the `UE_Only` emergency criterion). New
  `report/show/overloads.rs`/`unserved.rs` reuse the WP8.3 seq-current
  (`for_each_enabled_elem`/`SymComp`) and Load-criterion (`unserved`/
  `exceeds_normal`) machinery that `export_overloads`/`export_unserved` already pin;
  the Show **layout** differs (no `kVAOver` column; fixed-width fields; the degenerate
  `NormAmps<=0` branch emits the literal `     0.0`). golden `show_{overloads,
  overloads_unbal,unserved,unserved_ue}` (golden_phase8 **89→93**) reuse the
  `DECK_GROUPS` export decks (ovl/ovl2/uns/uns2 — single-sourced by index, no drift)
  via the new deck-based `run_deck_show`/`_gen_show_deck_group` helpers; **all four
  exact equality** (`rel=0, abs=0`) — the seq currents / EEN·UE factors are the same
  `corpus_live`-pinned paths, and on the clean synthetic decks every `%8.1f`/`%9.3f`
  cell is byte-identical (incl. ovl2's unbalanced I2/I0 + the `normamps=0`
  degenerate-column row, and uns2's healthy-load exclusion). No corpus migration
  (`Show` was never a blocker — the deferred keywords stayed *silent* no-ops, so no
  deck was parked on them). **Both audits ran (`d44e2b3`): no correctness bug.**
  audit-code confirmed both formatters + arm 16/17 reproduce `ShowOverloads`/
  `ShowUnserved` field-for-field (headers, terminal-1-only walk, the `>=3`/`<3`-phase
  split, capacitor exclusion, the overload guard, every degenerate `     0.0`
  literal, `Show`'s no-uppercase `pLoad.Name`, filenames, `@lastshowfile`). audit-tests
  found the goldens sound (trusted-oracle bytes, single-sourced deck, exact-equality
  justified) but flagged two **Minor** coverage gaps — closed with two follow-up
  goldens (golden_phase8 **93→95**): `show_overloads_1ph` (a dedicated deck: a
  **1-phase** overloaded line with `emergamps=0` + a small overloaded capacitor —
  pins the three branches ovl/ovl2 miss: the `<3`-phase seq fallback, the
  `EmergAmps<=0` degenerate `%Emerg`, and the capacitor-skip) and `show_unserved_normal`
  (`show unserved` on `uns2` — the **normal**-criterion healthy-load exclusion, a
  distinct `ExceedsNormal` filter the single-load `uns` deck couldn't pin). Left as
  noted (not a bug, whole-Show-family limits): the token comparator is field-width-blind
  (widths are per-report backend quirks, deliberately not reproduced), and the
  `.norm()`/round-half-even vs FPC `Cabs`/round-half-away last-ULP floor is a Phase-8-wide
  formatter property (documented in `format.rs`), not a step-9 regression — watch for a
  `%.Nf` half-boundary straddle if new fixtures land boundary values.
  **Remaining unported Show keywords** (all still *silent*
  no-ops, `TODO(WP8)`): `Controlled` (needs the `ControlElementList` accessor),
  `Zone`/`Isolated`/`Loops`/`Topology` (CktTree walks), `FaultStudy`, `Yprim` (needs
  the active-ckt-element surface + its non-`CircuitName_` filename), `LineConstants`,
  `busflow` (`ShowBusPowers`), `autoadded`/`QueryLog` (headless FireOffEditor
  no-ops), `deltaV` (deferred).
- **WP8.4 (Show reports) — step 10 COMPLETE, gate-green** (the FaultStudy report):
  `Show Faults` (`ShowFaultStudy` → `FaultStudy.txt`, dispatcher arm 6) — the
  three-section short-circuit report over the **precomputed** bus `Zsc`/`Ysc`/`VBus`/
  `BusCurrent` a prior `Solve mode=faultstudy` (WP7.9) populated (read-only,
  PHASE8_PLAN §2.1): (1) all-node bolted currents (`|BusCurrent[i]|` `%15.0f` + `X/R`
  of `Zbus = VBus[i]/BusCurrent[i]` `%5.1f`, `N/A` on zero current), (2) one-node-to-
  ground SLG (`|VBus[iphs]/Zsc[iphs,iphs]|` + the pu node voltages `|VBus[i] −
  Zsc[i,iphs]·IFault|`, faulted node ≈ 0), (3) adjacent node-node L-L via a
  `GFault = 10000 S` scratch inversion of `Ysc` (`iphs < iphs2` only, no wrap). New
  `report/show/fault_study.rs` reuses the WP7.9 fault state + the `export_fault_study`
  scratch-inversion math; the two scalar complex divisions use the FPC-faithful
  `cdiv_fpc` (matching the oracle's `ucomplex` `/`), the node-node inversion the
  FPC-faithful `CMatrix::invert`. golden `show_faultstudy` (golden_phase8 **95→96**),
  feeder-based (IEEE13 + `solve mode=faultstudy`, the `export_faultstudy`/`SeqZ`
  fixture shape) via `run_feeder_show`, **exact equality** (`rel=0, abs=0`) on the
  first try: the fault currents/voltages derive from the bit-exact assembled Y +
  FPC-faithful matrix ops, so every `%15.0f`/`%12.0f`/`%5.1f`/`%10.3f` cell is
  byte-identical Rust↔oracle (no straddle). No corpus migration (`Run_NEV.dss`, the
  one deck with `show faults`, stays blocked on `Select`/`show yprim`). **Both
  audits ran (`535d893`): no correctness bug in the port** (audit-code verified
  `ShowFaultStudy` field-for-field incl. `cdiv_fpc`/`get_xr`/`CMatrix::invert`
  fidelity and confirmed the `None`-`Zsc`/`Ysc` `continue` guard correctly protects
  against the exact **upstream UB** — Pascal `ZFault.CopyFrom(nil)` access-violates
  on that path). Two **audit-tests** findings, both addressed: (Major D) the golden
  doc's "pinned to 1e-8 by `corpus_live.rs`" was **false** (corpus_live snapshot-
  solves, never enters faultstudy, never compares `Zsc`/`Ysc`/`BusCurrent`) — doc
  rewritten to cite the real aggregate pins (`export_faultstudy` `%.2f` +
  `export_seqz`) and state the per-node amps/X-R/pu matrices are pinned by this
  golden alone at print precision (the fault state carries the ~1e-8 faer-vs-KLU
  floor from the `Zsc` re-solves; exact holds only because no cell straddles a
  `%.Nf` boundary on this feeder). (Major A) two branches were unexercised by the
  based-IEEE13 golden. **Empirical correction (probe-proven):** the oracle
  access-violation is on a **cold** `solve mode=faultstudy` (no prior converged
  solve → unallocated `Zsc`), *not* the `kVBase<=0` report itself — with a prior
  `solve mode=snap` the oracle renders the unbased L-N-Volts report fine. So the two
  branches split by reachability: (1) the cold path is genuine UB — our engine
  guards it (refuses the cold faultstudy, then the `None`-`Zsc` guard yields the
  section-1 `N/A` branch, no panic), pinned by the **`fault_study_cold_solve_is_safe`
  unit test** (no golden — the oracle crashes there); (2) the `kVBase<=0` `%10.1f`
  L-N-Volts branch IS oracle-comparable → pinned by the **`show_faultstudy_unbased`
  golden** (an unbased deck via `run_deck_show`, exact equality, also pinning a
  1-node lateral bus). golden_phase8 **96→97**; lib 732→733.
- **WP8.4 (Show reports) — step 11 COMPLETE, gate-green** (Yprim + the
  active-ckt-element surface): the `Select` executive command (Pascal `DoSelectCmd`,
  `ExecHelper.pas:670`, ordinal 6 — `do_select_cmd` in `exec/command.rs`) sets the
  new `Dss.active_ckt_element: Option<(cls,idx)>` (+ the element's active terminal;
  `SetActiveBus` skipped as inert, like `Open`/`Close`), and `Show Yprim`
  (`ShowYPrim`, dispatcher arm 25) dumps that active element's primitive Y — the
  `G` (conductance) then `jB` (susceptance) **lower triangles** by rows (`%13.10g`),
  from `cd.yprim` (`GetYprimValues(ALL_YPRIM)`); a `Nil` Yprim prints `Yprim matrix
  is Nil`. New `report/show/yprim.rs`; the filename is the one Show exception —
  `<ParentClass.Name>_<Name>_Yprim.txt` with **no** `CircuitName_` prefix
  (`ShowOptions.pas:395`), written via the new `write_show_path` (raw
  `<OutputDir><filename>`; `write_show_named` now delegates to it). golden
  `show_yprim` (golden_phase8 **97→98**): `Select line.650632` + `show yprim` on
  solved IEEE13, **exact equality** (`rel=0, abs=0`) — the LineCode-based line's
  primitive Y is bit-exact Rust↔oracle, so every `G`/`jB` cell is byte-identical;
  the `*_Yprim.txt` glob also pins the no-`CircuitName_` filename. `Select` is now a
  real command (was a `not_ported` stub); may unblock corpus decks
  (`Run_NEV.dss` used `Select`/`show yprim`) at the next classify pass.
  **Both audits ran (`c66d0c0`). audit-code found ONE real Major bug + two Minor
  error-fidelity gaps, all fixed:** (Major) `active_ckt_element` was **not reset in
  `do_clear_cmd`**, so `select X → clear → new circuit → show yprim` indexed the
  emptied class objects → an **OOB panic** (confirmed by the auditor's probe) — fixed
  by resetting it alongside `active_class`; (Minor) an unknown non-empty class emitted
  #246 instead of Pascal's **#903** ("Object Class … not found") and dropped the
  **fallback to the previously-referenced class** — `do_select_cmd` restructured to
  match `SetObjectClass` (log #903, keep `active_class`, fall through), oracle-probed
  (`select badclass.l2` after a Line select → #903 + selects `l2`); (Minor) the
  active-terminal parse used a value-based `>0` instead of Pascal's `Length(Param)>0`
  (a present out-of-range terminal must leave the active terminal **unchanged**, not
  reset to 1) — fixed. **audit-tests** flagged the `Select` command's error/edge
  branches as uncovered (Major) + the no-`CircuitName_` convention + the Yprim
  no-op/Nil arms (Minor) — closed with the new **`exec/tests/select.rs` (9 tests,
  lib 733→742)**: the Clear-reset regression (the Major bug's guard), the #903+fallback,
  the terminal parse incl. the OOR-unchanged case, #245, the DSS_OBJECT/`circuit.<name>`
  no-ops, the Yprim no-op-without-Select, and the `Line_l1_Yprim.txt` filename (no
  `CircuitName_`).
- **WP8.4 (Show reports) — step 12 COMPLETE, gate-green** (the EnergyMeter
  zone-tree pair): `Show Loops` (`ShowLoops` → `Loops.txt`, dispatcher arm 21) and
  `Show Zone <meter>` (`ShowMeterZone` → `<CircuitName_>ZoneOut_<meter>.txt`, arm 14)
  — both walk the meter's **already-built** zone tree (the WP6.4 `MakeMeterZoneLists`
  `BranchList`): `Show Loops` writes one line per parallel/looped branch across all
  meter zones (`(mtr) Class.UPPERCASE(Name): PARALLEL WITH|LOOPED TO
  LoopLineObj.FullName`), `Show Zone` the whole zone as a `Level`-tab-indented
  branch/shunt tree with the inline `(PARALLEL:LoopLineObj.Name)`/
  `(LOOP:LoopLineObj.FullName)` + `(Sensor: …)` annotations. New
  `report/show/meter_zone.rs` reads the persisted `sequence_list` (== the
  `First`/`GoForward` walk order) + the new `sequence_nodes` map to reach each
  branch's `TreeNode` (`is_parallel`/`is_looped`/`loop_elem`/`level()`/`shunts`) with
  no cursor mutation; new `EnergyMeter::branch_list`/`sequence_nodes` +
  `TreeNode::level` accessors. Zone `SensorObj` = the meter itself
  (`(Sensor: EnergyMeter.<m>)`, set at zone build, `build.rs:167`/291/350). golden
  `show_{loops,zone,loops_mesh,zone_mesh}` (golden_phase8 **98→102**): radial metered
  IEEE13 (header-only loops + the 32-line zone tree) + a **synthesized meshed deck**
  (a 3-line loop b1-b2-b3-b1 + a line parallel to `la`) exercising the PARALLEL/LOOP
  branches of both reports. **All four compared BYTE-EXACT** (not the field-width-blind
  token diff the padded Show tables must use) — these pure-text reports have no
  `MaxBusNameLength`/`PadDots` backend quirk, so the deterministic `TABCHAR`
  indentation + trailing spaces are reproducible in full (new
  `assert_show_bytes_eq` + `run_{feeder,deck}_show_exact`, sharing the extracted
  `produce_{feeder,deck}_show` replay helper). The two `ShowMeterZone` error branches
  (#221 empty name, #220 meter-not-found — file still written, `GlobalResult` set)
  are golden-uncoverable (they push an error) → pinned by the new
  `show_zone_error_paths` unit test (lib **742→743**). No corpus migration (`Show`
  was never a blocker). **Both audits ran.** **audit-code — one real Major bug found +
  fixed:** `Show Zone` on a **found-but-disabled** meter (`BranchList = NIL`) wrote the
  2-line header, but Pascal guards the *entire* body — header included — on
  `BranchList <> NIL` (`ShowResults.pas:2453`), so the oracle writes an **empty** file
  (probe-confirmed 46 bytes vs 0); a silent, gate-invisible divergence (no error, not
  corpus-reachable) — fixed by moving the header inside the `Some(tree)` guard,
  regression-guarded by the new `show_zone_disabled_meter_is_empty` unit test (also
  closes audit-tests' None-`BranchList` gap for both reports). One **Minor** fixed: the
  `Show Zone` header echoed the stored (lowercased) meter name; Pascal uses the raw
  command `Param` (native case, like the filename) — probe-confirmed (`show zone EM1`
  → header `EnergyMeter EM1`, SensorObj still `EnergyMeter.em1`) — fixed by threading
  `param` into the header. **audit-tests — two Minor coverage gaps + closed:** the
  multi-meter `show_loops` outer loop was single-meter-only → new `show_loops_multi`
  golden (a 2-meter deck: zone A loop + zone B parallel, so **both** meters emit rows,
  pinning header-once + per-meter `(mtr)` prefix); the None-`BranchList` branch →
  closed by the disabled-meter unit test above. Both confirmed the `(Sensor: NIL)` /
  `loop_elem == None` arms are **unreachable by construction** (correct 1:1 ports of
  defensive Pascal `else`s, `build.rs:167`/334-337). golden_phase8 **102→103**; lib
  **743→744**.
- **WP8.4 (Show reports) — step 13 COMPLETE, gate-green** (`Show Controlled`):
  `ShowControlledElements` (`ShowResults.pas:3905` → `ControlledElements.csv`,
  dispatcher arm 33) — every PD element carrying a control, then `, <control
  FullName> ` per control (Pascal `Format(', %s ', …)`, native case, trailing
  space). **Ported the missing model piece** the report needs (per
  [[port-gaps-immediately]]): the Rust port materialises **no** `HasControl` flag /
  `ControlElementList` (only the forward control→element ref), so a new
  `CktElement::controlled_element()` trait accessor (default `None`, overridden by
  all 11 controls → `self.ccd.controlled_element`) lets `report/show/controlled.rs`
  **derive** the PD→controls map by scanning `ckt.controls` (creation order) and
  matching — read-only (PHASE8_PLAN §2.1), reproducing the exact observable
  (`ControlElementList` insertion order == control creation order == `ckt.controls`
  order; a reassigned control follows its *current* target = Pascal's remove-then-add
  final state). golden `show_controlled` (golden_phase8 **103→104**): solved IEEE13's
  three regulator RegControls → their Transformers, compared **byte-exact**
  (`run_feeder_show_exact` — pure names, no `MaxBusNameLength`/`PadDots` quirk), also
  pinning the `*_ControlledElements.csv` filename. No corpus migration (`Show` never
  blocked a deck). **Both audits ran.** **audit-code — one real Major bug found +
  fixed:** the override was added to only 11 controls; **Fuse** (the 12th
  `TControlElem`, living under `elements/pd/fuse/` not `elements/control/`, so the
  control-dir grep missed it) fell through to the `None` default → every fuse-switched
  line **silently vanished** from the report (uncaught by the RegControl golden +
  `corpus_live` solve-only compare). Fixed (Fuse override) + pinned by the new
  `show_controlled_multi` golden's `Line.l3, Fuse.fu1 ` line. One **Minor** documented
  (not materialised): the derive-at-read uses `ckt.controls` creation order, but
  Pascal's remove-then-add **re-appends** a *re-edited* control to the end of its
  target's list — the two disagree only for ≥2 controls on one element with a
  post-creation element-ref edit (probe-only, no corpus deck; the faithful fix =
  materialise the whole `ControlElementList`, disproportionate) — noted at the
  `controlled_element()` doc. **audit-tests — Major coverage gap closed:** the feeder
  golden exercises only single-control-per-PD + the `RegControl` override; the new
  synthesized **`show_controlled_multi`** deck golden (byte-exact, `run_deck_show_exact`)
  pins the repeated-control `, %s , %s ` loop + creation-order ordering (Line.l1
  Recloser→Relay, Line.l2 two SwtControls) and **all five** PD-targeting overrides
  (swt/recloser/relay/fuse/capcontrol). golden_phase8 **104→105**.
- **WP8.4 (Show reports) — step 14 COMPLETE, gate-green** (`Show LineConstants`):
  `ShowLineConstants` (`ShowResults.pas:3244`, dispatcher arm 24) — for every
  `LineGeometry`, the per-unit-length **R / jX / susceptance / L / C** matrices
  (lower triangle, `%.6g`) at the parsed `freq`/`units`/`rho` (defaults
  `DefaultBaseFreq`/`kft`/`100`) + the **order-3 equivalent symmetrical-component
  summary** (Z1/Z0 + L1/L0, C1/C0, surge impedance, propagation velocity). Writes
  **two** files: `<CircuitName_>LineConstants.txt` (the report, sets `@lastshowfile`)
  and `LineConstantsCode.dss` (a LineCode script, same dir, **no** `CircuitName_`
  prefix, via `write_show_path`). New `report/show/line_constants.rs` reuses the
  **WP7.1** `LineGeometryObj::{z_matrix,yc_matrix}` Carson recompute (`set_rho_earth`
  + the requested freq/units/earth-model) + `CMatrix::{order,get,invert}` +
  `LineUnits::{as_str,to_per_meter}`; the report's L/C post-scaling uses the full
  `TwoPi` (Pascal `DSSGlobals.TwoPi = 2·PI`, distinct from the Carson engine's
  truncated internal `TWOPI`). golden `show_lineconstants` (a synthesized 3-cond `g3`
  + 1-cond `g1` geometry deck) verifies **both** files **byte-exact** (`g3` exercises
  the matrices + seq summary; `g1` the order≠3 no-seq branch) — the Carson values are
  bit-exact Rust↔oracle bar the proven transcendental libm floor, and every `%.6g`
  cell lands byte-identical (no 6-sig straddle). **Found + fixed a real byte-fidelity
  bug** surfaced by this first byte-exact *numeric* report: `format::g` emitted a
  lowercase-`e` exponent (C printf) but FPC `Format('%g')` emits **uppercase `E`**
  (`1.19304E-6`) — fixed in `format::g` (+ `g_w`/`g_left_w` routed through it); the
  value-parsing gate was case-blind so it hid until now. golden_phase8 **105→106**.
  No corpus migration (`Show` never blocked a deck). **audit-tests follow-up (2
  coverage goldens):** the step-14 golden used only default args + a non-empty
  geometry list → two branches unexercised: (1) **`show_lineconstants_mi250`**
  (`show lineconstants 60 mi 250`) pins the `freq`/`units`/`rho` arg-parse arms AND
  that `rho=250` (earth-return) + `units=mi` propagate into the Carson recompute
  (values, `ohms per mi` labels, `To_per_Meter` velocity) — also **proves the
  `set_rho_earth`-then-recompute path is correct** (a fresh geometry's `fline_data`
  is built at parse, so the rho takes effect; the suspected rho-drop is a non-issue),
  byte-exact on both files; (2) **`show_lineconstants_empty`** (a geometry-less deck)
  pins the header-only path. golden_phase8 **106→108**. **audit-code — no correctness
  bug** (rho/full-`TwoPi`/matrix-math/units/two-file all verified faithful + byte-exact
  incl. the auditor's own `mi250`/`freq=50` probes; the `twopi=TAU` confirmed —
  `ShowResults.pas` pulls `DSSGlobals.TwoPi=2·PI`, not the truncated Carson one). One
  **Minor** fixed: the geometry-compute error path silently dropped Pascal's **#9934**
  `Error computing line constants for …` message — the formatter now returns it and
  the dispatcher extends the error log (unreachable for a validly-parsed geometry, so
  golden-uncoverable; the safe skip still does not reproduce Pascal's post-log NIL-`Z`
  fault). One coverage golden added on the audit's recommendation: **`show_lineconstants_f50`**
  (`show lineconstants 50`, freq≠DefaultBaseFreq) pins the non-default-frequency Carson
  propagation. (Pre-existing, out-of-scope: `fmt_g` NaN/Inf formatting for a
  pathological negative-surge-radicand order-3 geometry — unreachable for real lines.)
  golden_phase8 **108→109**.
- **WP8.4 (Show reports) — step 15 COMPLETE, gate-green** (`Show busflow`):
  `ShowBusPowers` (`ShowResults.pas:1493`, dispatcher arm 23) — the power flow around
  a named bus, two forms: **code 0** (seq) the bus's seq voltages + per-element seq
  currents (all terminals) + seq powers (the `CheckBusReference`-matched terminal);
  **code 1** (elem) the node voltages + per-terminal branch currents (PD residual) +
  branch power flow. New `report/show/bus_powers.rs` filters every element by
  `check_bus_reference` and **reuses the extracted per-bus / per-element helpers**:
  `voltages::{seq_voltage_row, bus_voltage_block}` (pulled out of `show_voltages`/
  `show_voltages_nodes`, byte-preserving — the 4 voltage goldens still pass),
  `currents::{get_i0i1i2, write_seq_currents, write_terminal_currents}` (`write_seq_currents`
  = the Pascal `WriteSeqCurrents` with `NormAmps=EmergAmps=0`), `powers::write_terminal_power_seq`
  (`WriteTerminalPowerSeq`, incl. the 1-/2-phase `S1` special cases), plus a local
  `WriteTerminalPower`. The `#219 Bus not found` error + the `<BusName|BusPower>_{seq|
  elem}_{kVA|MVA}.txt` filename are in the dispatcher. goldens `show_busflow` +
  `show_busflow_elem` on solved IEEE13 **bus 675** (a fully-energised leaf) — the seq
  form **fully exact** (`rel=0, abs=0`), the elem form exact bar the **capacitor's
  ~0-kW / PF** faer-vs-KLU residual cells (gated on the row's kW `<1e-4`). Bus 675 was
  chosen over a richer junction (671) whose `%10.5g` kvar cell lands on a 5-sig
  rounding boundary (a print straddle). golden_phase8 **109→111**. No corpus migration
  (`Show` never blocked a deck). **Both audits found no correctness bug** (audit-code
  verified `ShowBusPowers` field-for-field incl. `check_bus_reference`, the seq
  currents-all-terminals / powers-matched-terminal asymmetry, the element-section
  PD-residual + PC/Faults order, both `write_terminal_power*` layouts, MVA scaling,
  and the extraction faithfulness; audit-tests confirmed the goldens sound). **Both
  audits' recommended coverage landed** (5 tests): `show_busflow_mva`/`_mva_elem`
  (the `Show`-path's only MVA coverage — `×0.001` + MW/Mvar/MVA headers),
  `show_busflow_1ph`/`_1ph_elem` (bus 611 — the `<3`-phase seq path
  `WriteSeqVoltages`<3 / `WriteTerminalPowerSeq` `S1`), and the
  `show_busflow_unknown_bus_errors` unit test (`#219`, golden-uncoverable) — all
  reusing the shared `busflow_seq_policy`/`busflow_elem_policy`; golden_phase8 **→120**.
  Tracked (WP8.8 byte pass, not gated — the token comparator masks it): FPC `Format('%g')`
  prints **fixed** notation for a value in ~[1e-5,1e-4) where C/`fmt_g` switches to
  scientific (`0.000043053` vs `4.3053E-5`) — a `format::g` band divergence with no
  current byte-exact test in that range.
- **WP8.4 (Show reports) — step 16 COMPLETE, gate-green** (`Show Isolated` +
  `Show Topology` — the circuit-wide CktTree pair): the new `solution/topology.rs`
  ports Pascal `GetIsolatedSubArea` (`CktTree.pas:624`) + its 5 connectivity helpers
  (`GetSources/GetPC/GetShuntPD/FindAllChildBranches`) and `GetTopology`
  (`Circuit.pas:3034`) — the same bus-adjacency BFS `make_meter_zone_lists` runs per
  meter, but circuit-wide from the source, reusing the WP6.4 `CktTree` primitives +
  `build_active_bus_adjacency_lists` + `all_terminals_closed` + the loop/parallel
  detection. Unlike the read-only reports these **mutate** element flags
  (`CHECKED`/`IS_ISOLATED`/`terminals_checked`) + bus `bus_checked` (via `ClassStore`,
  the mutable `ElemStore`), exactly as Pascal does. `report/show/isolated.rs`
  (`ShowIsolated` → `Isolated.txt`, arm 7) builds the source tree + one sub-area per
  unreached PD element and lists the not-connected buses / isolated sub-networks /
  isolated enabled elements / connected tree; `report/show/topology.rs` (`ShowTopology`
  → two files `TopoTree.txt`+`TopoSumm.txt`, arm 28) the TABCHAR-indented branch/shunt
  tree with the `(PARALLEL:…)`/`(LOOP:…)`/`(Sensor:…)`/`(Control:…)`/`(Meter:…)`
  annotations (the control annotations derive from the `ckt.controls` scan, like
  `Show Controlled`, since `HAS_CONTROL` is not materialised) + the level/loop/parallel/
  isolated/switch counts (`ShowTreeView` is a headless no-op). goldens
  `show_{isolated,topology}` on metered IEEE13 (the connected tree + the 1-loop
  regulator topology) + `show_{isolated_iso,topology_mesh}` on a **synthesized** deck
  (a PARALLEL line pair + a switched line/SwtControl + a fully ISOLATED island) — all
  compared **byte-exact** (pure text: names + tree levels + TABCHAR indent, no backend
  width quirk), covering the non-empty isolated/parallel/switch branches the radial
  IEEE13 misses (the deck is unsolved — the island is singular — and the topology walk
  still resolves it: the port processes buses at `calcvoltagebases`, no
  `ReprocessBusDefs` needed). golden_phase8 **111→115**. No corpus migration.
  **audit-tests follow-up — one Major coverage gap closed:** the two `ShowIsolated`
  sections the IEEE13 + mesh goldens leave empty (the "ENABLED ELEMENTS ARE ISOLATED"
  `"FullName"  Buses: "bus"` list + the 1-based `get_bus(j)` walk, and the "BUSES NOT
  CONNECTED TO ANY POWER DELIVERY ELEMENT" list) → the new `show_isolated_orphan`
  deck golden (an orphan Load + Generator on PD-less, source-disconnected buses)
  pins both, byte-exact (golden_phase8 **115→121** incl. the step-15 busflow follow-up).
  Tracked low-payoff (not added): the `(Sensor:…)` topology annotation (needs a
  solved+metered+sensored deck), the `>30`-level `(* level *)` tab overflow, and the
  no-source empty-tree branch. **audit-code follow-up — two real Major `ShowIsolated`
  bugs found + fixed:** (A) the isolated-**sub-area** selection was missing the
  `Enabled` guard (`ShowResults.pas:2915` `if TestElement.Enabled`), so a **disabled**
  PD element (in `ckt_elements` but not the adjacency lists) would print a spurious
  `*** START SUBAREA ***` block — fixed + pinned by `show_isolated_disabled`; (B)
  `ShowIsolated` was missing its `if BusNameRedefined then ReprocessBusDefs`
  (`:2859`, the one Show report that resolves bus refs itself), so on a compiled-but-
  **unsolved** circuit the terminal `bus_ref`s stay `NO_BUS` and the walk yields a
  degenerate one-source tree — fixed (reprocess in dispatcher arm 7, `ShowTopology`
  correctly left without it) + pinned by `show_isolated_unsolved`. One **Minor** (the
  inert `ToBusReference`, unread by the reports) filled for walk-fidelity. golden_phase8
  **122→124**. (The `controls_of` multi-control annotation order the auditor flagged
  is already validated by `show_controlled_multi` — same `ckt.controls` derive.)
- **WP8.4 finalize — dispatch tail (partial), gate-green.** Now that every real
  `Show` keyword is ported, `do_show_cmd` gains the Pascal **`#24700`** unknown-keyword
  error (`ShowOptions.pas:119-124`, pushed + return before the solve-guard) and
  explicit **silent no-op** arms for the three headless-FireOffEditor keywords
  `autoadded`(1) / `QueryLog`(32) / `deltaV`(31) (deltaV keeps its `TODO(WP8)`) —
  replacing the blanket `_ => {}`. Safe: all 21 distinct corpus `Show` keywords (incl.
  the ambiguous `v`/`y`/`f`/`mon`) map to ported arms (verified — corpus_live stays
  green). New `show_unknown_and_deferred_keywords` unit test. golden_phase8 **→122**.
- **WP8.4 finalize — `Show DeltaV` (the last deferred report), gate-green.**
  `ShowDeltaV` + `WriteElementDeltaVoltages` (`ShowResults.pas:3822`/`339`, arm 31,
  in the solve-guard set): the voltage across each enabled 2-terminal element
  (Sources/PD/PC), per conductor `NodeV[term1] − NodeV[term2]` — magnitude / percent
  (0 when the terminals' kVBase differ, e.g. a transformer) / base-kV / angle
  (`%12.5g`/`%6.1f`). New `report/show/delta_v.rs`. **The step-4 deferral is
  resolved**: the `NodeRef[i+NCond]` cross-terminal read now resolves both terminals'
  buses for the delta-primary `Transformer.SUB` (3 rows, matching the oracle) — later
  Phase-7 transformer node_ref work fixed the layout that produced 0 rows at step 4.
  golden `show_deltav` on solved IEEE13 (exact equality — same solved `node_v` the
  voltage goldens pin). golden_phase8 **→125**. **next = the corpus classify/migrate
  pass** (Show was never a blocker, so likely a no-op — refresh + confirm), then the
  STATUS full sync + WP8.4 close.
- **WP8.5 (Save/Dump) — Dump step 1 COMPLETE, gate-green.** The `Dump`
  single-object forms (`Dump <class>.<name> [debug]` / `Dump <class>.* [debug]`,
  Pascal `DoPropertyDump`, `ExecHelper.pas:1194`): new `report/save/` module
  (`dump.rs` = the generic base reproducing Pascal's 4-level `TDSSObject`→
  `TDSSCktElement`→`TPCElement` chain, selected per element kind — plain / non-PC
  CktElement / PCElement — with `dump.rs::overrides` downcast-dispatching the
  per-class leaf overrides) + the **Reactor** override (`reactor/dump.rs`: the NIL-
  matrix skip, the un-`~` `RMatrix=`/`XMatrix=` lines, the `%-.8g` Z/LmH forms) +
  the `#903` (`SetObjectClass` fail) / `#256` (object-not-found) errors + the
  `dump_one_object` dispatcher (`exec/report.rs`, precomputing the PC `! VARIABLES`
  values via the mutable element walk). The whole-circuit forms (bare `Dump` /
  `Dump debug` + `Circuit.DebugDump` header, `Dump solution`, `Dump commands`/
  `buslist`/`devicelist`/`alloc`) are scoped TODO(WP8) step 3; the 13 non-Reactor
  overrides are TODO(WP8) step 2 (until each lands, its class dumps via the generic
  base). Property lines reuse `ClassProps::get_value` (Pascal `PropertyValue[i] ==
  GetPropertyValue(i)`). goldens `dump_reactor`/`dump_reactor_debug` (synthesized
  reactor deck: r1 series R+X + rz R/X-matrices; `debug` adds the CktElement
  NPhases/…/NodeRef/Terminal Status/Bus Ref + the `%13.10g` YPrim G/B) **byte-exact**
  (golden_phase8 **125→127**). **Two latent byte-fidelity bugs found + fixed**
  (both surfaced by this first byte-exact property dump; `props_roundtrip`'s numeric
  compare masked both): **(1)** the port stored property names in ad-hoc lowercase
  (`bus1`/`kv`/`normamps`) but Pascal `PropertyName[i]` (what Dump/Save emit) is the
  **display case** (`Bus1`/`kV`/`NormAmps`) — corrected for Reactor, pinned by the
  golden; matching stays case-insensitive (`CommandList` lowercases both sides), so
  no parse regression, and **Save's round-trip gate is case-insensitive** so only
  Dump byte goldens need each class's names corrected (class-by-class as they land);
  **(2)** `float_to_str` was `format!("{v}")` (Rust's 17-digit shortest-round-trip)
  where FPC `FloatToStr` is `ffGeneral`/**15 sig figs** — fixed to `fmt_g(v, 15)`
  (full workspace suite green, 745 lib tests, so no numeric-gate regression; the
  scientific-exponent FPC form is a scoped `TODO(compat)`, unreached by in-scope
  dumps). REACTORTest unblocked (converges clean on Rust) — migration deferred to
  the Dump-completion classify pass (PHASE8_PLAN §WP8.5 step 4).
  **Both audits ran (`5550cf5`) — one real Dump byte bug + ONE latent Phase-4
  reactor bug found, both fixed:** **(audit-code Finding 1, Major)** `Terminal Bus
  Ref` printed `0` for an unresolved terminal (disabled element), but Pascal
  `Terminal.BusRef = -1` "not set" (`Terminal.pas:41`) renders `-1` — fixed +
  pinned by `dump_reactor_disabled`. **(audit-tests #1 → a real latent bug)** the
  coverage golden for a symmetrical-components reactor (`SpecType=4`, Z1≠Z2 — the
  induction-motor `KerstingMotor`) exposed that `reactor/solve.rs::stamp_series`
  wrote the two-terminal series stamp's **bottom-left block at `(j+n, i)` instead
  of Pascal's `(i+n, j)`** (`Reactor.pas:936`) — invisible for every *symmetric*
  reactor Y (R+X / matrices, all prior cases) but **transposes the asymmetric
  sym-components YPrim**, corrupting an *unbalanced* solve (a balanced solve is
  unaffected — verified Rust≡oracle node V). Fixed; full suite green (no
  symmetric-reactor regression). Also pinned by a **solve-level** regression test
  `asymmetric_sym_components_reactor_unbalanced_solve` (an unbalanced deck — the
  transposed stamp violates KCL `I_t1+I_t2=(Y−Yᵀ)V1≠0`, so reverting the fix fails
  it by ~20 kA — plus per-conductor currents vs the pinned oracle). Coverage added
  for every audit gap: goldens
  `dump_reactor_symcomp` (nonzero Z1/Z2/Z0 + the single-object form),
  `dump_reactor_disabled` (`! DISABLED` + the `-1` fix), `dump_loadshape` (the
  generic plain-`TDSSObject` path — LoadShape names already match the oracle) +
  unit tests `float_to_str_is_15_sig_figs` (pins the 15-sig fix directly) and
  `dump_generic_base_ordering` (the PC `! VARIABLES`/props-after and non-PC
  props-before-`! ENABLED` orderings, oracle-probe-confirmed). Tracked (not fixed,
  low): the `#903` message drops Pascal's `CRLF+CmdString` suffix (consistent with
  the existing `do_select_cmd` #903 convention); `#256` doesn't create an empty
  `PropertyDump.txt` (unobservable — `GlobalResult` unchanged); the systematic
  property display-name pass (needed for step-3 bare-`dump`; done class-by-class as
  Dump goldens land). golden_phase8 **127→130**; lib **745→748**. **Dump step 2**
  (the five per-winding/matrix overrides + the `fmt_g`/`WdgCurrents`/`Vterminal`
  fixes + both audit follow-ups) is recorded in the header §1 frontier above.
  Original port map below (still current for the remaining steps):
- **GAPS_PLAN authored (2026-07-05): the test-blocked-deferral closure plan +
  the third live-gate family.** `GAPS_PLAN.md` (repo root) inventories every
  Phase-4–7 deferral whose only blocker was a missing corpus deck — the WP7.9
  "zero corpus cases → skip" anti-pattern PHASE8_PLAN §1 promised to correct —
  and packages it as work packages (now WPG.1–13 + the WPG.17 exit sweep): Monte1/2/3 + MonteFault, LD1/LD2 (+ `Set
  LDCurve=`), AutoAdd, `mode=Time`, the Newton algorithm, CapControl
  `Follow`+`ControlSignal`, binary shape-file inputs, Reactor `RCurve`/`LCurve`,
  InvControl Exponential + Storage VW/VV_VW, StorageController seasonal targets
  (+ `Set SeasonRating/SeasonSignal=`), Relay Generic/TD21, GFM. The two
  Phase-7 "Plot-blocked, zero payoff" tracked-opens are **stale**: the TD21
  corpus decks carry pre-WP7.2 `unsupported_class=relay` tags, and the
  IBRDynamics `GFM_IEEE123` decks compile — blocked only by BatchEdit (WP8.6)
  + GFM itself. **16 synthesized decks** landed at `tests/corpus/gaps/` +
  `manifest.json` (the `asymmetric`/`controls` family pattern; every case
  `pending: true` until its WPG ports the feature — the future
  `gaps_cases_match_oracle` must assert a pending case errors *loudly*, never
  skips). Every deck probe-validated on the pinned oracle: **bit-identical
  across two separate oracle processes** and **feature-sensitive** (seasonal
  event log diverges without `SeasonRating`; harmonic channels shift without
  `RCurve`/`LCurve`). Probe-proven facts recorded in the plan: FPC
  `mathutil.pas` `initialization Randomize` time-seeds the RNG per process →
  RNG-carried values can never be oracle-pinned (gates use `Set random=none`;
  a single-Fault deck makes MonteFault deterministic — `Trunc(Random*1)+1=1`);
  the oracle **segfaults at process exit** after an AutoAdd solve (solve +
  results intact — capture before teardown); `InvControl.ControlModel` parses
  only the ordinal, not the enum name; the option spelling is `Set
  SeasonRating`, not `SeasonalRating`. Also removed the stray empty
  `c/DI_yr_0/` artifact dir. No engine code touched (decks/plan/docs only).
- **GAPS_PLAN extended (2026-07-05): the unported ELEMENT classes.** A registry
  diff (Pascal `DSSClassDefs.pas` vs `exec/construct.rs`) found five
  upstream-registered classes with no Rust port: **Isource** (541 ln),
  **AutoTrans** (2065 ln — both were in the PORTING_PLAN crate sketch but never
  assigned to a phase), **GICLine**/**GICTransformer**/**GICsource** (pulled in
  from Phase 9). Now GAPS_PLAN §1b + WPG.14–16 (exit sweep = WPG.17), each with
  Sonnet-executable staged steps (Pascal line refs, Rust module templates,
  registration slots, gates, corpus reclassification). **18 element decks** at
  `tests/corpus/gaps/` covering the {snapshot × time-series × snapshot→
  time-series} × {micro × midi} matrix (Isource 7, AutoTrans 7, GIC 4 —
  async/both N/A upstream for GIC; midi decks generated on the shared
  scaffold via `gen_midi_decks.py`, 94–100 nodes) — all §3-validated on the
  pinned oracle (two-process bit-identical; the reg decks control-active:
  10–12 tap events; `gicsource` splice probes: `line.bus2 → gic_<name>`).
  The family is now formally a **staging area** (GAPS_PLAN §3.1): a graduated
  deck moves to `asymmetric`/`controls`/`modes` in the WP that flips its
  `pending`, and WPG.17 deletes the emptied dir. Probe-proven upstream facts
  recorded: GICTransformer parses `%R1` (not `pctR1`); AutoTrans
  `WdgCurrents` = the `READS_VTERMINAL` family; corpus `Auto1bus`/`Auto3bus`
  need no new class (stale `fault` tags — regular transformers); `GICsource`
  has zero corpus decks. `ControlledTransformer` + user-model DLL classes
  verified not-registered upstream (nothing to port). No engine code touched
  (decks/plan/docs only).
- **PHASE8_PLAN tail refresh (2026-07-05): the remaining WPs made
  Sonnet-executable + their test corpus pre-built.** WP8.1–8.4 collapsed to
  one-line done-markers (records live here, not in the plan); WP8.5 steps
  3–6 / WP8.6 / WP8.7 / WP8.8 rewritten with exact Pascal line refs (from a
  scoped deep-read of DoSaveCmd/Circuit.Save/WriteClassFile/DoPropertyDump +
  the 8 remaining DumpProperties overrides; DoBatchEditCmd/DoInterpolateCmd/
  DoDistributeCmd/DoUuidsCmd; ReduceAlgs.pas + Line.MergeWith + ReduceZone/
  KeepList) and per-step gates over pre-validated decks. **Probe-proven
  oracle facts recorded in the plan** (all 2026-07-05, two-process): (1)
  `dump commands` help texts come from the wheel's gettext catalog
  `dss/messages/properties-en-US.mo` (1697 entries; `Command.*`/`Option.*`/
  `<Class>.<prop>` keys; miss → the key itself) → plan adds a generated
  `help_catalog.rs`; (2) **upstream garbage reads in DumpProperties**:
  Capacitor `CMatrix`/`FaultRate`/`pctPerm` print ASLR denormals ALWAYS,
  Reactor `FaultRate`/`pctPerm` do so whenever an EnergyMeter exists
  (Line/Transformer/Fault/Load stay clean) — not reproduced (UB rule),
  masked-golden strategy planned + investigations/ report at WP8.5 step 3;
  (3) `save` (meters) writes only `MTR_<name>.csv` with a RELATIVE
  GlobalResult, Monitor.Save is stream-internal (no file); `save load`
  emits an extension-less file `load`; `save circuit` = 14-file dir incl.
  per-meter `SaveZone` subdirs and an always-created (possibly empty)
  `BusCoords.dss`; Master.dss carries a wall-clock stamp line (round-trip
  gate ignores it); (4) `distribute what=Load` overrides an explicit
  `file=` to `DistLoads.dss`; `how=Random` is FPC time-seeded (never
  golden-gated); (5) `export uuids` auto-creates hashed keys
  `Station=…`/`GeoRgn=…`/`SubGeoRgn=…`, generates random v4 for anything
  not preloaded, and leaves `Text.Result` EMPTY. **Decks:** 7 fixture decks
  + `uuids_pre.csv` at `tools/golden/phase8_decks/` (README records the
  facts above; wired into gen_phase8.py by their WPs) and 12 gaps staging
  cases (`batchedit`/`midi_batchedit` with regex-semantics probes; 8
  `reduce_*` strategy decks incl. keeplist + `reduce_remove`
  (`Load.eq_l2_b2` equivalent pinned by probes) + `midi_reduce` 94→88
  nodes, 3 merges), manifest notes carrying the oracle-verified post-reduce
  element lists (`l1~l2`, `s1~s2`, `b1||b2`, disabled partners, node
  counts). gen_midi_decks.py gained `midi_batchedit`/`midi_reduce`
  (existing decks regenerate byte-identical). No engine code touched.
- **Per-element midi wave (2026-07-05, gate-green): every micro scenario
  replayed on the midi scaffold** — 12 per-element asymmetric decks
  (`midi_vsource_asym` … `midi_upfc_asym`, incl. VCCS behind a 12.47/0.36
  service transformer and the proven UPFC chain fed from a backbone phase) +
  13 per-class controls decks (`midi_regcontrol` … `midi_sensor`, incl. nested
  EnergyMeter zones and the loop-tie SwtControl open), all from
  `gen_midi_decks.py`'s shared scaffold; gates now 27 asymmetric + 37 controls
  decks, all micro tolerance. **FOURTH real port bug caught**
  (`midi_invcontrol`, InvControl element conductor count 3 vs oracle 1): the
  parse-time fleet resolver took the control's terminal phase count from the
  FIRST enabled DER, but Pascal's recalc loop assigns `FNphases :=
  ControlledElement[i].NPhases` per member — the LAST wins (InvControl.pas:916;
  ExpControl.pas:408 has the same shape, fixed too; Pascal itself carries a
  "what if these are different sizes" TODO). Invisible with equal-phase fleets
  (all micro decks); a mixed 3φ+1φ fleet exposes it. Fixed in
  `exec/command.rs` + new `ForeignClasses::last_enabled`. Also fixed: oracle
  capture counted the C-API empty-array placeholder `['NONE']` as a
  one-element `ZonePCE` list (nested-meter zone with genuinely no PCE); and a
  deck-scaffold bug (the swtcontrol deck was switching the DISABLED loop tie —
  a disabled element's YPrim view diverges between engines by construction).
- **Midi network gate (2026-07-05, gate-green): the IEEE123-class synthetic
  network** (`tools/decks/gen_midi_decks.py`, deterministic generator; decks
  committed) — the asymmetric configs + control/protection density at the
  MINIMAL scale that reproduces large-network failure modes (~94 nodes,
  14-segment backbone, loop + parallel segment, 3 voltage levels, mixed
  3/2/1-phase laterals, many controls per control round):
  `asymmetric/midi_asym.dss` (micro tolerance holds at 94 nodes),
  `controls/midi_controls.dss` (cascaded LTC + 3×1φ reg bank + 2 kvar
  CapControls + InvControl over 2 PVs + StorageController, daily 24 h),
  `controls/midi_protection.dss` (relay→recloser→fuse fuse-save race at the
  deep-lateral end). **THIRD real port bug caught** (midi_controls hour 2,
  +2 iterations, systematic under perturbation — not a knife edge): when a
  StorageController flips the fleet state and an InvControl refreshes the same
  Storage in ONE control round, `InvDispEnv::der_set_nominal` consumed
  `StateChanged` → `yprim_invalid` but dropped Pascal `Set_YprimInvalid`'s
  `SystemYChanged` side effect (CktElement.pas:245) — the oracle rebuilds Y in
  `CheckControls` (Solution.pas:1155) within the round (proven by `Set log=yes`
  control-marker diff: oracle "Building Whole Y Matrix" at ControlIter=1, port
  at ControlIter=2), the port ran the next round on the stale-state YPrim.
  Fixed in `dispatch.rs` (env gained `system_y_changed`). Same bug class as
  the step-2 `update_all_storage` find — the bare-field-write-vs-Pascal-setter
  family; unreachable from micro decks (needs two control classes touching one
  Storage in the same round). Deck-authoring lessons recorded in the plan doc:
  kvar-CapControl deadband must exceed its own bank size (hunts otherwise);
  voltage-mode CapControl under regulators never toggles.
- **Controls live gate (2026-07-05, `CONTROL_COVERAGE_PLAN.md`, ALL 5 steps
  COMPLETE — 22 decks, gate-green).** Steps 3–5 on top of the below: **protection**
  (recloser temp/perm reclose+lockout, relay 51, relays 46/47 on parallel
  branches — asymmetry-only trips, per-phase SLG fuse blow, delayed SwtControl
  open via manifest `post`; duty mode, per-step event log + pending reclose
  shots in the control queue), **metering** (energymeter sym/asym — all
  registers incl. Overload/EEN/losses + zone membership; monitor modes 0/1/2/3;
  sensor mapping probes), **combos** (`combo_protection` fuse-save coordination
  under an EnergyMeter+monitor; `combo_voltvar_asym` LTC + kvar-CapControl +
  volt-var InvControl interplay — a voltage-mode CapControl under an LTC never
  toggles, hence kvar mode; `combo_metering`), and `compare_eventlog` opt-in on
  the three daily IEEE feeders (13/37/123) in `solvable_now`. **Oracle capture
  caveat found:** on a step whose solve rebuilt Y mid-step (fault applying at
  ontime, protection trip opening a switch) the pinned engine's
  `getYSparse(False)` returns None — `gen_checkpoints._get_y_sparse` retries
  after the executive `BuildY` (proven trajectory-neutral: identical per-step
  iterations/voltages/event log with and without); `getYSparse(True)` must NOT
  be used (it corrupts the solution vector — YNodeVarray returns
  injection-scale garbage, empirically). **Original steps 1–2 record:** the
  live gate extended with **element-specific state
  channels** (all opt-in per manifest case): property **probes** (oracle
  `Properties(p).Val` vs the Rust `?` query, numeric-skeleton compare),
  **PC-element state variables** (`AllVariableValues` vs `element_variables`),
  the cumulative **event log** per step (`Solution.EventLog` vs `event_log()`,
  the phase7-protection policy), and the pending **control queue** (new
  `Dss::control_queue_rows`, `%.9g` QueueItem format); the mandatory full-model
  compare additionally gained per-element **losses** (`CktElement.Losses`
  channel, tolerance = the summed per-conductor power policy — new
  `ElementSnapshot.loss_w`) and `selected_elements: ["*"]` (every YPrim-bearing
  element). Decks in **`tests/corpus/controls/`** (runner
  `controls_cases_match_oracle` + structural guard, `CONTROLS_REQUIRED` floor):
  step 1 `regcontrol_sym` (24-step daily LTC, taps 0→3→0, 8 events); step 2
  `regcontrol_asym` (3×1φ bank, per-phase-unequal taps, 21 events),
  `capcontrol_sym`/`capcontrol_asym` (voltage / kvar+current-CT-phase-3 modes,
  verified switching both directions), `invcontrol_vv_sym` (rolling-avg daily
  VOLTVAR), `invcontrol_vvvw_asym` (CombiMode VV_VW over 3×1φ PVs, per-phase kW
  caps + kvar), `storagectrl_peakshave` / `storagectrl_time` (charge/idle/
  discharge trajectories, per-step kWhstored/State probes),
  `gendispatcher` (weighted 3:1 daily redispatch). **REAL PORT BUG found and
  fixed by the new gate** (storagectrl_peakshave step 6, +1 iteration):
  `update_storage`'s end-of-step state flip (full → idling) set
  `cd.yprim_invalid` as a bare field write, dropping Pascal
  `Set_YprimInvalid`'s side effect of raising `Solution.SystemYChanged`
  (`CktElement.pas:245`) — the next step then injected through the **stale
  charging YPrim** on its first iteration (extra ~61 A/phase RHS, proven by
  first-iterate injection-vector diff; V/kWh state bit-identical to ≤1e-15
  entering the step) and needed an extra iteration to reach the same fixpoint.
  Fixed in `time_series.rs::update_all_storage` (propagates `yprim_invalid` →
  `system_y_changed`); all 9 decks + full corpus green at micro tolerance.
  (The step-2 "next" — protection/metering/combos — is the steps-3–5 work
  recorded at the top of this entry.)
- **WP8.5 follow-up — asymmetric live gate (2026-07-05, gate-green).** The reactor
  stamp bug class generalized into a standing guard: **`tests/corpus/asymmetric/`**
  — 14 synthetic decks covering **every stamping element** (Vsource / Reactor /
  Capacitor / Line / Transformer / Fault / Load models 1–5+8 / Generator models
  1–3 / PVSystem / Storage / IndMach012 / VCCS / UPFC) + 2 **combination** decks
  (series chain; meshed loop w/ circulating-tap transformer + IndMach012) in
  deliberately asymmetric configurations: `Z1≠Z2` sym-components sources+reactors
  (the non-reciprocal-YPrim class of the fixed bug), **FULL asymmetric matrix
  inputs** (pin the parser's `ParseAsSymMatrix` lower-triangle-wins overwrite
  order — Pascal symmetrizes, `ParserDel.pas:616`), per-phase-unequal 1φ bank
  taps, 1φ/2φ subsets, delta conns, series capacitor, neutral-impedance +
  ungrounded-wye windings — all solved **unbalanced** and live-compared by
  `corpus_live.rs::asymmetric_cases_match_oracle` with the full mandate (V, full
  system Y, every element's currents/powers, named YPrim blocks — the direct
  transposed-stamp catch regardless of excitation) at **micro (1e-9) tolerance**;
  oracle-free `asymmetric_manifest_is_complete` pins the dir↔manifest bijection,
  the 14-deck per-element floor (`ASYMMETRIC_REQUIRED`), and `selected_elements`
  non-empty per case. **VSConverter deliberately excluded** (upstream
  state-mutating `GetCurrents`; stays gated in `exec/tests/vs_converter.rs`).
  **Proven floor found while calibrating** (decomposition, per CLAUDE.md — not a
  tolerance sweep): a bus whose only ground path is the transformer
  `ppm_antifloat` (~1e-6) adder — a delta tertiary or ungrounded-wye secondary
  serving only L-L loads — has a near-singular **common mode** where faer-vs-KLU
  last-ulp noise reaches ~1.7e-7 rel from iteration 1 (phase-to-phase voltages
  match to ~1e-12 V, ALL YPrims + system Y match ≤1e-12; ONLY the bus common
  mode differs, and iteration counts drift at tight tol). Not a port bug and not
  reproducible-pinnable; the decks pin such buses **physically** with small
  wye-grounded capacitors (the cable-capacitance surrogate every real feeder
  has), never by widening tolerances.
- **WP8.5 (Save/Dump) — exploration/port map.** The port map (from a
  scoped explore pass) so the next session resumes without re-reading:
  - **Reuse (the oracle-validated primitive):** `ClassProps::get_value(obj, idx,
    enums)` (`obj/props/class_props/value.rs:16`) — the exact renderer the `?` query
    (`exec/command.rs:549` `do_query_cmd`) + `props_roundtrip` use. Plus
    `num_properties()`/`property_name(idx)` (`class_props/mod.rs`) and, for Save's
    set-order walk, `DssObjData::next_property_set(after)` +
    `set_as_next_seq`/`prp_specified` (`obj/base/mod.rs:64-93`). Save/Dump are thin
    string-joins over these — **no new formatting**.
  - **Dump** (Pascal `DoPropertyDump` `ExecHelper.pas:1194`; `TDSSObject.DumpProperties`
    `DSSObject.pas:110`): `New "FullName"` + `~ name=get_value` for `1..num_properties`
    (Leaf=true). `TDSSCktElement.DumpProperties` (`CktElement.pas:918`) adds
    `! ENABLED`/`! DISABLED` + (Complete/`debug`) NPhases/Nconds/Nterms/Yorder/NodeRef/
    Terminal-status/Bus-ref/YPrim. **Complication: ~17 element `DumpProperties`
    overrides** (Reactor `Reactor.pas:966` skips NIL R/X-matrices + custom `~ Z1=[…]`
    8-sig, etc.) — each must be reproduced faithfully (byte gate). File
    `<OutputDir><CircuitName_>PropertyDump.txt`; forms `Dump <class>.<name>` /
    `Dump <class>.*` / `Dump` (all: every CktElement + DSSObj + Solution) /
    `Dump debug` / `Dump solution`. Corpus: 3 decks (`dump reactor`×2, `dump
    transformer`) — needs synthesized fixtures + the byte gate.
  - **Save** (Pascal `DoSaveCmd` `ExecHelper.pas:744`; `Circuit.Save` `Circuit.pas:2409`;
    `WriteClassFile` `Utilities.pas:1134`; `WriteDSSObject`+`SaveWrite`
    `Utilities.pas:1221`/`DSSObject.pas:145`): per-object `New "Class.name"` + ` name=
    CheckForBlanks(value)` for each `next_property_set` prop (SET props only, set order)
    + ` ENABLED=NO` if a disabled ckt element. `Save circuit` = a fresh `<Name>NNN`
    subdir + the library-class `WriteClassFile`s + `SaveDSSObjects` (class-ordered) +
    `SaveVoltageBases`(`BusVoltageBases.dss`) + `SaveBusCoords`(`BusCoords.dss`) +
    `SaveMasterFile`(`Master.dss` header/`Redirect` list/footer) + `SaveOpenTerminals`
    + `SaveFeeders` (per-meter zone subdirs). **New plumbing needed:** subdir creation
    (no `SetCurrentDSSDir` yet), the `Save meters` primitives `Monitor::save`/
    `EnergyMeter::save_registers` (NOT ported), and the **`save_roundtrip.rs`** gate
    (re-compile+re-solve = identical V, per §1 gate #3 — round-trip, not byte-match).
    `SaveFeeders`: `Feeder` is **not** an instantiable class (only a CIM-XML label,
    probe-confirmed) → the meter-zone path, documented not skipped. Stubs at
    `exec/report.rs` `do_save_cmd`/`do_dump_cmd`; dispatch `command.rs:135-136`.
  - **Suggested sub-steps:** (1) Dump (base + ckt-element + the 17 overrides + the
    dispatcher forms + a byte golden on the corpus `dump reactor`/`transformer` +
    synthesized `dump … debug`/`dump all`); (2) `Save <class>` (WriteClassFile) +
    `Dump`-shared `get_value` reuse; (3) `Save circuit` + `save_roundtrip.rs`; (4)
    `Save meters`/`voltages` (needs the Monitor/EnergyMeter save primitives). Then the
    `Save`/`Dump` corpus migration.
- **WP8 goldens exactness audit — ✅ COMPLETE (2026-07-04), gate-green.** All 93
  `compare_export` compares in `golden_phase8.rs` re-measured cell-by-cell against
  their oracle captures (a temporary harness audit mode collecting max deviations
  instead of asserting): **74 are parse-value-identical** → pinned at exact
  equality (`rel=0, abs=0`; the never-exercised `EXPORT_REL`/`EXPORT_ABS`/
  `YMATRIX_REL`/`LOSSES_ABS`-class preemptive print floors deleted, incl. the
  Voltages/8500-Voltages angle 0.11, Powers/SeqPowers 0.11, P_byphase-MVA 0.0011,
  Taps 1e-4, monitors 1e-4/1e-5, registers 0.5, Loads 0.05, reliability 1e-8/1e-9,
  capacity/faultstudy/SeqZ/Y/Yprims/overloads/unserved/sections/profile/
  allocation floors, and the `show_convergence` |V| `rel=1e-6`). The **19**
  non-exact compares all trace to three observed classes, each kept at its
  measured floor: (1) last-digit render straddles — `P_byphase` kVA 1e-3 (kept
  0.0011), `Losses` 7-sig 65.34585↔86 (kept `rel=1e-6`), `show_mismatch` %10.5f
  (kept 1.1e-5); (2) near-zero cancellation residuals — `Losses` noise cells
  ≤1.28e-8 W (`abs` 1e-6→**1e-7**), `SeqVoltages`/`SeqCurrents` residuals (abs
  1e-9/1e-8 kept, ratio-cell abs →**1e-9**, `rel` 1e-4→**0**), `Currents`/
  `ElemCurrents`/`ElemPowers`/`YCurrents` (`abs` 1e-6→**1e-8/1e-10/1e-10/1e-13**,
  `rel`→0; noise-angle `PrevCol` gates kept, `ElemVoltages` gates dropped —
  byte-exact); (3) the DI files — the only genuine non-print floor, the per-step
  faer-vs-KLU diff integrated over 24 daily solves (measured 1.5e-8 rel):
  `rel` 1e-6→**5e-8**, `abs` 1e-8→0. Gates kept only where observed firing on
  noise (show_losses/voltages/currents/currents_elem/powers_elem, seqcurrents I1,
  ang_tol PrevCol, the two Summary DateTime Masks + show_mismatch residual Masks);
  no-op/unfired ColTols removed. `tests/TOLERANCE_NOTES.md` WP8 sections rewritten
  to the exact-by-default regime. golden_phase8 84/84 green.


---

- **WP8.1 (Report infrastructure) — ✅ COMPLETE, gate-green** (`82b50fe` dispatch
  skeleton + GUI/`Plot`/`Visualize` headless no-ops with the #301 pre-circuit
  guard; `929145c` output-path machinery + `Set DataPath=` + `Export Counts`
  end-to-end + the `compare_export` golden harness). The `Show`-silent vs
  `Export/Save/Dump`-loud-`NOT_PORTED` asymmetry is forced + proven-safe.

- **WP8.2 (Export: solution outputs) — ✅ COMPLETE, gate-green.** The full export
  families, read-only/mutating over the solved circuit: bus/node (`Voltages`/
  `BusCoords`/`NodeNames`/`YNodeList`), aggregate power (`Powers`/`Losses`/
  `P_byphase` + the `for_each_enabled_elem`/`export_with_mut` mutable element-walk
  infra + the MVA `Parm2` pre-parse), symmetrical-component (`SeqVoltages`/
  `SeqCurrents`/`SeqPowers` + the `ColTol::gate` denominator gate), per-terminal
  (`Currents`/`NodeOrder`/`ElemCurrents`/`ElemVoltages`/`ElemPowers`/`Taps` + the
  `ColSel` harness refactor), and matrix/summary (`Yprims`/`Y`/`SeqZ`/`Summary`/
  `Result`). **Completion gate:** the IEEE8500 `Voltages`/`Summary`/`Counts`
  goldens + the `Export`-unblocked corpus migration (`solvable_now` **88→119**,
  COVERAGE **26.3%→35.5%**) + the Rust `CorpusGuard` (corpus stays pristine under
  report-writing decks). Two tracked-opens **RESOLVED** here (detail in the
  archive): the "9 decks hang" was a debug-build watchdog artifact (all 9 converge,
  ≤3.4s release), and the rare live-gate flake was a per-process convergence misfire
  *inside the pinned oracle* (now retried in-process) — neither a Rust bug.

- **WP8.3 (Export: device/reliability + logs) — ✅ COMPLETE (steps 1–5 + both
  audit follow-ups), gate-green.** The device/meter/reliability/log exports over
  the solved circuit + the demand-interval file machinery: `Monitors` (step 1);
  the `Meters`/`Generators`/`Loads`/`PVSystem_Meters`/`Storage_Meters` register
  dumps (step 2, which surfaced + fixed the never-wired DER `SampleAll`/`ResetAll`
  solve-loop tail); `EventLog`/`ErrorLog` (step 3a, + the missing circuit-build
  `LogThisEvent` markers); `Faultstudy` (step 3b, read-only over the WP7.9
  `Ysc`/`BusCurrent`); `BusReliability`/`BranchReliability`/`Capacity`/`Overloads`/
  `Unserved`/`AllocationFactors`/`Sections`/`Profile` (step 3c, over the `RelCalc`
  fields — incl. the meter `SectionCount`/`FeederSections` persistence, the seven
  `Profile` `PhasesToPlot` branches, and the multi-meter `Bus_Int_Duration`
  cross-zone bug — filed + gated, see below); the `TSystemMeter` core + the full demand-interval
  (`DI_*`/phase-voltage/overload/volt-exception) writers + their `Set` handlers +
  the `Set year=` `Set_Year` side effects + the Solve*/DI open-close wiring
  (step 4, §2.6); and the completion gate (step 5) — 47 probe-clean decks migrated
  + the **silent `Spectrum.CSVFile` no-op** fixed → 2 IEEE_519 harmonicT decks
  (`solvable_now` **119→168**, COVERAGE **50.1%**). The two independent audits
  (`bacaf13` code, `8d58a8e` tests) found **no correctness bug**; two LOW code
  findings fixed (the `Export Profile` `1732.0` truncated-√3 `TODO(compat)` marker;
  the `Spectrum.read_csv_file` byte-position `(F.Position+1) < F.Size` EOF guard,
  an oracle-confirmed divergence over `str::lines()`), and five oracle-pinned tests
  closed the coverage gaps (`Set year=` lifecycle, the five `Get` DI echoes, the
  `NPhases<3` overload I2-column mapping, the Spectrum-`FileLoad` deck round-trip,
  and — after the user flagged the parked multi-meter `Bus_Int_Duration` note — the
  cross-zone reliability contamination, now **empirically settled**: a new upstream
  bug report (`investigations/reliability_bus_int_duration_oob_bug_report.md`), the
  in-range regime gated by `export_busreliability_multimeter`, the out-of-range OOB
  proven-nondeterministic across processes). Full per-step + audit detail in
  [`docs/phase-records/phase-8.md`](docs/phase-records/phase-8.md).
  golden_phase8 **59**; lib **731**; `solvable_now` **168**.
- **WP8.4 (Show reports) — step 1 COMPLETE, gate-green.** `do_show_cmd` is now a
  real dispatcher (option/solve-guard per `ShowOptions.pas`), routing the first
  ported keywords to fixed-width text formatters in the new `report/show/` module:
  `Show Buses`/`Losses`/`Taps` + `Show panel`→#999. Unported `Show` keywords stay a
  *silent* headless no-op (the `solvable_now` `Show Power`/`Voltage`/… decks don't
  regress). Shared machinery: `format.rs` `Pad`/`PadDots`/`EncloseQuotes` +
  width-aware `%W.Df`/`%Wd`/`%W.Pg` formatters, the `@lastshowfile`/`last_show_file`
  bookkeeping, and a new **whitespace+comma tokenizer** in `harness::compare_export`
  (`sep: ' '`, `header_lines: 0`) that diffs the fixed-width tables token-for-token.
  golden_phase8 **59→62** (`gen_show_reports` + `show_{buses,losses,taps}`). The two
  independent audits found **no correctness bug** (the three formatters reproduce
  `ShowBuses`/`ShowLosses`/`ShowRegulatorTaps` field-for-field; the goldens are
  genuine pinned-oracle bytes). Three follow-ups fixed: (audit-tests) the
  `show_losses` policy split into per-column floors — the coarse `%8.2f` `% of Power`
  floor (`abs=0.011`) isolated to token index 2 via `col_tol`, kW/kvar held to the
  tight default (so a small-cell formatting regression fails); (audit-code) the
  deferred `Show` keywords + the deferred unknown→#24700 now carry a greppable
  `TODO(WP8)` tag (the WP8.8 exit sweep) instead of prose-only — the deferral stays a
  *silent* no-op by design (erroring would regress the live `Show Power`/`Voltage`
  decks); and the `Pad`/`max_*_name_length` width helpers switched to byte length
  (`str::len`) for byte-1:1 with Pascal `Length(AnsiString)`.
- **WP8.4 (Show reports) — step 2 COMPLETE, gate-green.** `Show Voltages` code 0
  (Pascal `ShowVoltages` case 0 + `WriteSeqVoltages`): the symmetrical-component
  voltages by bus — V1 (kV)/pu/V2/%V2·V1⁻¹/V0/%V0·V1⁻¹, the bare `Show Voltage`/`v`
  default (82 live decks). The dispatcher's ptr-13 arm ports the `ShowOptions.pas`
  option parse (first param `LL`→phase-phase file `VLL`, else `VLN`; second param
  `N`/`E`→the node/element form). The angle-bearing node/element forms
  (`ShowOptionCode` 1/2) stay a silent no-op with a greppable `TODO(WP8)`. New
  `report/show/voltages.rs` reuses the `SymComp`/`bus.find` helpers; note Show's
  `<3`-node V1 = `|V|` of the first node **unconditionally** (Pascal
  `WriteSeqVoltages`, unlike `ExportSeqVoltages`' `PositiveSequence` gate). golden
  `show_voltages` (golden_phase8 **62→63**). Both independent audits found **no
  correctness bug** (the `<3`-node unconditional-first-node V1 trap is handled
  right, and the ptr-13 LL/N/E parse + code-1/2 silent no-op match `ShowOptions.pas`
  exactly). One audit-tests LOW fixed: the `show_voltages` golden abs tightened
  **1e-5 → 1e-8** (the proven faer-vs-KLU floor is 1e-12 — one `sourcebus` V0 cell;
  every significant cell is bit-identical), so a real small-cell error can no longer
  hide under the old blanket 1e-5.
- **WP8.4 (Show reports) — step 3 COMPLETE, gate-green.** `Show Currents` +
  `Show Powers` code 0 (Pascal `ShowCurrents`/`ShowPowers` case 0 +
  `WriteSeqCurrents`/`GetI0I1I2`): the per-element sequence currents (I1/I2/%I2·I1⁻¹
  /I0/%I0·I1⁻¹/%Normal/%Emergency, with `Cmax`-based ratings, the CAP exclusion, and
  the unconditional `<3`-phase I1) and sequence powers (P1/Q1/P2/Q2/P0/Q0 + PD
  terminal-1 excess + the total-loss footer). The ptr-3/ptr-12 dispatcher arms port
  the `ShowOptions.pas` residual/`m`/`e` option+filename parse (`Curr_Seq`,
  `Power_seq_{kVA|MVA}`); the element forms (code 1) stay a `TODO(WP8)` no-op. golden
  `show_currents`/`show_powers` (golden_phase8 **63→65**). **Two findings settled
  during the step:** (1) the oracle's `SetMaxDeviceNameLength` is empirically **0**
  in the pinned dss_capi 0.14.5 (device-name-independent — probe-proven; the vendored
  source would give 16), so the `Paddots` device-name column is never padded —
  reproduced 1:1 with a `TODO(compat)` (`max_device_name_length → 0`), which also
  makes the step-1 `Show Losses` names byte-faithful; (2) the `%I2/I1`/`%I0/I1` ratio
  gate threshold is **1e-6 A** — provably between the one noise row (a switch's
  floating terminal, `I1 ≈ 1.8e-12 A`) and the smallest *real* current (`Line.671680`,
  `5.8e-4 A`, whose ratio IS checked); an 8-order gap, so `1e-6` never gates a physical
  current (a coarser `1e-3` would wrongly skip the real row). Also fixed a
  **usability bug**: `gen_phase8.py` now sets `DSS.AllowEditor = False` so
  regenerating the `Show` goldens no longer spawns a Notepad per report. Both
  independent audits ran: **no correctness bug** (the seq math, `Cmax` ratings, CAP
  exclusion, unconditional `<3`-phase I1, `×0.003` power scaling, the footer-loss
  walk, and the `mdnl=0` `TODO(compat)` are all faithful; the `1e-6 A` gate proven
  to skip only the one `1.8e-12 A` noise cell). Two Minor **byte-faithfulness** code
  findings fixed (the numeric comparator masked both): the currents `%s %3d` literal
  space between name and terminal, and the powers footer `%6.1f` field width (was
  widthless); plus the continuation-label width switched to the un-uppercased byte
  length.
- **WP8.4 (Show reports) — step 4 COMPLETE, gate-green.** The remaining
  solution-report Shows' **element/node forms** + `Show Elements`: `Show Voltages`
  code 1 (`WriteBusVoltages` — line-ground **and** line-line by bus & node,
  mag/angle/pu/base-kV, the `jj`-cursor node walk + the wrapping LL partner) and code
  2 (`WriteElementVoltages` — node-ground by element, Sources+PD then PC); `Show
  Currents` code 1 (`WriteTerminalCurrents` — per-terminal branch currents + the PD
  residual row, Sources+PD+Faults then PC); and `Show Elements` (`ShowElements` +
  `WriteElementRecord` — the element↔bus listing, PD then PC, split into the main
  `Elements.txt` + the `_Disabled.txt` companion; the optional class-name filter).
  The dispatcher arms 3/13 now route codes 0/1(/2), and the new **arm 5** parses the
  class filter and writes the two files (disabled first, no `@lastshowfile`; main
  sets it — via the new `write_show_named(set_last)` split). golden_phase8 **65→69**
  (`show_{voltages_node,voltages_elem,currents_elem,elements}`). **One finding settled
  during the step:** the pinned dss_capi 0.14.5 `SetMaxBusNameLength` **floors at 12**,
  not the source's 4 — probe-proven `max(12, longest_bus_name)` (a 20-char bus widens
  to 20, a 5-char one floors at 12), so IEEE13 (`sourcebus`=9) pads to 12. Reproduced
  1:1 (`TODO(compat)` in `report/show/mod.rs::max_bus_name_length`), same
  backend-vs-source class as the `MaxDeviceNameLength=0` finding; it only shows up in
  the dot-padded `WriteBusVoltages` column (the space-padded reports are
  token-invariant, which is why steps 1–3 didn't surface it). Both independent audits
  ran (`0ceb615`); **no correctness bug** — audit-code verified every walk set, index
  translation, formula and dispatcher arm against Pascal + the oracle. **audit-code
  follow-ups** (2, both settled empirically): (1) `WriteElementVoltages` (Voltages
  code 2) writes the per-element separating blank line **outside** the `Enabled`
  guard, so a *disabled* element still emits its blank — reproduced (raw
  `walk_element_voltages` over all refs, blank per element; oracle-probed on a
  disabled-load deck, byte-match); (2) the floor-12 `TODO(compat)` refined — the
  oracle floors the **data** (`PadDots`) rows at 12 but the **column-header** row's
  `Bus` at 4 (a backend within-report inconsistency, probe-measured), a masked
  whitespace-only byte divergence noted for the WP8.8 byte pass. **audit-tests
  follow-ups** (3): (1) the angle `%.1f` printing-floor overrides were mis-indexed —
  the first row of each bus carries an extra `..` dots token (and the elem form
  splits `(pu)` into two tokens), shifting the angle off the fixed `Index`; fixed via
  a new **content-relative `ColSel::AfterToken("/_")`** selector (the angle is the
  column after the `/_` glyph, robust to the row shift); (2) the `show_currents_elem`
  angle gate raised `1e-6 → 1e-4 A` — this report's residual rows push the noise floor
  to ~1.08e-5 A (633/634 near-balanced-transformer residuals) while the smallest real
  current is 5.72e-4 A, so `1e-4` brackets them (the `1e-6` code-0 threshold didn't);
  (3) two coverage goldens added — `show_elements_class` (the class-filter path) +
  `show_voltages_ll_node` (the LL branch). Also fixed a step-4 **test-tree leak**: the
  `show_reports_are_silent_noops` unit test ran `Show Voltage LN Nodes` (now a *real*
  report since step 4) with no datapath → wrote `t_VLN_Node.txt` into the source tree;
  set a scratch datapath + switched to still-unported keywords. golden_phase8 **69→71**.
- **WP8.4 (Show reports) — steps 5–8 + the WP8 goldens-exactness audit** continue in
  the **header frontier** (§ top): that block carries the recent-step detail (step 5
  `Powers` elem + `Result`/`EventLog`/`Ratings`/`Variables`/`Mismatch`/`monitor`;
  step 6; step 7 the diagnostic/matrix cluster `Convergence`/`Y`/`controlqueue`/
  `kvbasemismatch`; step 8 the register tables `Meters`/`Generators`; and the
  exact-equality re-measure) until the **WP8.4-completion archive pass** folds the
  whole Show record into [`docs/phase-records/phase-8.md`](docs/phase-records/phase-8.md),
  the established phase-record pattern. The current active step / next is authoritative
  in the header frontier.
