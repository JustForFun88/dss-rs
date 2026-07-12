# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-07-11.

**WPG.21 port — MakePosSequence + 6 synthesized decks (2026-07-11), gate-green
(fmt/clippy/`cargo test --workspace` incl. `modes_cases_match_oracle`).** The last
GAPS_PLAN §1b "on-demand" deferral closed as an ultracode round (5 opus worktree
executors A1/B/C/D/A2 + settle, opus-xhigh code audit + opus-high tests audit;
coordinator merged via `wpg21-integration` → `b9349e0`, main was mid-DIAKOPTICS so
integration branched off `947d439`; 2 trivial both-added conflicts hand-resolved
in `exec/command.rs` + `modes/manifest.json`). What landed: `cmd::MAKE_POS_SEQ = 60`
dispatch (`ExecCommands.pas:81` — NOT 74, the plan brief's error, caught by the
executor) + `exec/make_pos_seq.rs` (`DoMakePosSeq` ExecHelper.pas:3035: creation-order
walk, per-element `PosSeqCtx`, applier VM reproducing `DSSObjectHelper.pas:3060`
typed-setter bracketing — bare Set* = single edit + own recalc); base bus rename
(`CktElement.pas:1101` StripExtension + the IsGroundBus `.1/.2/.3`-substring quirk,
direct write, no redefine signal); typed prop setters (`class_props/typed.rs`, no
f64→string round-trip, seq-mark+side-effects on success only); all **33 Pascal
overrides** (7 PD + 12 PC + 3 meters + 11 controls; UPFC/IndMach012 = empty
`no_base()`). Upstream quirks reproduced with `TODO(compat)`: Generator
`PrpSequence[26]/[27]` guard reads Xdp/Xdpp (not kVA/MVA — they never divide);
Storage's missing BeginEdit + dangling EndEdit (one extra recalc); Capacitor
SpecType-3 `SetDouble(Cuf)` on a DoubleArray = silently-discarded write; Load ÷3
regardless of phases (÷3-again on second run, deck-pinned). NOT reproduced (UB, per
CLAUDE.md): the probe-proven oracle Access-violation configs
(GenDispatcher/ESPVL/UPFCControl with `element=`; InvControl/ExpControl NIL/empty-DER
— `docs/wpg21_makeposseq_probes.md`) → safe-skips + unit tests, never decks. Gates:
6 oracle-validated decks in `modes/` (`makeposseq_{line,xfmr,shunt,pc,ctrl,report}`,
§3 protocol: two-process bit-identical + feature-sensitive, live full-model compare
green), ~80 unit tests asserting exact `PosSeqAction` sequences, 4 exec tests
(creation-order adoption, off-phase-1 transformer disable with dotted buses kept,
idempotency, ground-`.0`). Audits: code = faithful, 1 Minor (missing `TODO(compat)`
tag) fixed; tests = 2 Major (6 untested trivial overrides; VSConverter mislabeled
"no-op" in deck note — it sets Phases=2/Ndc=1, excluded from decks for its known
GetCurrents bug) + 3 Minor — all 6 findings settled (`223611f`, 12 new tests).
Integration-surfaced fixes (audit-verified): Capacitor `set_struct_f64_array(kvar)`;
`set_obj_double` scalar-only guard (string path unaffected — parse.rs routes arrays
separately); `transformer_taps`/`refresh_vterminal_if_marked` skip disabled elements
(Pascal enabled-walk; prevents stale-node_ref OOB on the disable path).

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
· (4) D7 calibration ✅ + EPRI/r3723 refs (in progress).

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
fixed-point, documented. The child `DO_CTRL_ACTIONS` fan-out is WP-AD.4 — the
WP-AD.3/D7 gates run `controlmode=off`; `ad_check_controls` currently samples the
coordinator's controls (benign for controls-off; the faithful AD-branch child
delegate is a WP-AD.4 item, flagged for auditors).

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

**Stage 4 official-reference gate (D9 b/c) — IEEE-13 ✅.** `tests/ad_reference.rs`
replays the EPRI IEEE-13 AD example with the identical **manual** cut
(`set LinkBranches=[Line.670671] UseMyLinkBranches=True`) and compares the built
`ZLL`/`ZCC`/`Y4` against **fresh r3723 references** harvested by the new
`tools/opendss/gen_ad_reference.py` (committed at `tests/data/adiakoptics/r3723_ref/
ieee13/` with `PROVENANCE.txt`). Result: Rust **bit-matches live r3723** — ZLL
3.8e-15 (f64 ulp), ZCC 6.2e-8, Y4 2.7e-8 (faer↔KLU last-ulp). Finding: the trunk's
own `References/SolveDirect/ADiakoptics_matrixes/*.csv` are **STALE** (older deck
revision, ~20% reactance drift; live r3723 on the current deck agrees with Rust
bit-for-bit), so the gate pins fresh harvests, never the committed trunk CSVs.
**D9(c) IEEE_123_Bus-G** (explicit-LinkBranches harvest via the same script) remains
open — the harvester is parameterized for it; resume point.

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
- **Deferred (Stage 4, honestly open):** the EPRI first-party IEEE-13 CSVs
  (`.../References/SolveDirect/ADiakoptics_matrixes/ieee13nodeckt_{ZCC,ZLL,Y4}.csv`, D9b)
  and the r3723 `gen_ad_reference.py` IEEE_123 harvest (D9c) are vendored/planned but not
  yet consumed — the AD matrices still have no *external* trusted-baseline value
  comparison (only in-test re-derivation + inverter self-consistency). Tracked as the
  remaining Stage-4 numeric gate, alongside the Stage-2b AD solve stitch and D7
  calibration.

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

**WPG.19/20 audit settlement (2026-07-11), gate-green.** Two auditors + the full
gate; every finding verified against the Pascal spec and the pinned oracle, all
real ones fixed 1:1 (no fudging):
- **`InterpretDblArray` malformed-token silent 0.0 → stop-and-shrink + #705
  (fixed).** `util::read_dbl_array_text` swallowed a non-numeric file token
  (`unwrap_or(0.0)`, kept reading, no diagnostic) where Pascal raises `DoSimpleMsg`
  #705, sets `Result := i-1` and BREAKs (`Utilities.pas:515-521`). Oracle-proven
  (`mult=(file=…)` with row 3 = `abc` → npts shrinks to 2, mult=[0.1,0.2], #705).
  Now returns `(Vec<f64>, Option<usize>)`; both callers (`compute::apply_interp_file`,
  `command::apply_generic_dbl_array_file`) stop at the bad row (length = `i-1`,
  count shrinks) and `push_error` the #705 diagnostic.
- **Stale manifest `master_ckt24-nomm.dss` (fixed).** Was tagged
  `unsupported_feature=file-backed-arrays` with a now-FALSE note ("not supported yet
  (WPG.19)"); the deck compiles+converges via the CLI (7522 nodes, 2 iters, 0
  errors). Re-tagged `skipped_unsupported → skipped_needs_investigation`
  (`live_mismatch_regcontrol_ldc_tap`), sharing the -mm sibling's real SubXFMR
  RegControl/LDC blocker.
- **Trailing-newline restored** on the four manifests edited this branch
  (`solvable_now`, `skipped_needs_investigation`, `skipped_unsupported`,
  `modes/manifest.json`).
- **`binsave_mmf` generator self-check added.** `gen_loadshape_binsave_mmf` now
  rebuilds the expected `GlobalResult` from the hand-written `result_files`/
  `result_tags` (hoisted to single-sourced constants) and `sys.exit`s if it != the
  captured oracle string — closing the Rust-vs-handwritten gap (proven to bite on a
  wrong tag; goldens byte-unchanged).
- **Full gate green.** fmt + clippy clean; `cargo test --workspace` exit 0 (914 unit
  + corpus_live all green). NOTE: `modes_cases_match_oracle` needs the opt-in EPRI
  Oddie venv (`tools/opendss/.venv`, gitignored → absent in this worktree); run with
  `DSS_OPENDSS_PYTHON` pointing at main's venv, or it panics environmentally (pre-
  existing `r4133` case, unrelated to WPG.19/20).

**WPG.20 port — MMF-shape binary save (`Action=SngSave/DblSave` under
`MemoryMapping=Yes`) (2026-07-11), gate-green (golden_reports + load_shape unit).**
The trio-era LOUD-NOT_PORTED refusal in `load_shape/compute.rs::queue_shape_save`
is removed. **Mandatory oracle probe first** (dss-python 0.15.7, per the
STATUS-mandated `Assigned(dQ)`-under-MMF question): **Case A** — an MMF LoadShape
with `mult=(sngfile=…) qmult=(sngfile=…)` under `action=sngsave`/`dblsave` writes
BOTH `<name>_P` and `<name>_Q`; the bytes equal the f32-narrowed source values
(sng file = f32 as-is; dbl file = those f32 widened to f64), and `GlobalResult` is
`mult=[…],  Qmult=[…]`. **Case B** — an MMF LoadShape WITHOUT `qmult` writes ONLY
`<name>_P` (NO `_Q` file; `GlobalResult` has no `Qmult=` clause) — confirming
`Assigned(dQ)` is true iff a `qmult=` MMF directive was given (Pascal
`CustomSetRaw` :791-802 allocs a 2-elem `dQ` sentinel). **Key invariant verified:**
the port's eager MMF read (`read_mmf_raw`/`finish_mmf`) already populated `p_mult`
and (iff `qmult=` given) `q_mult` with the identical record semantics the oracle's
save-time `InterpretDblArrayMMF` re-read uses, and `q_mult.is_some()` == Pascal
`Assigned(dQ)`. So **guard removal alone is correct** — the existing non-MMF
snapshot body emits byte-exact bytes; no separate `InterpretDblArrayMMF` re-read
path was needed. Pinned by a new byte-exact golden `binsave_mmf_matches_oracle` (6
`.bin` goldens: `mp` = sng-P + sng-Q → both `_P`/`_Q`; `md` = dbl-P + no-Q → only
`_P`, `md_Q.*` asserted ABSENT) via `gen_reports.py::gen_loadshape_binsave_mmf`
(MMF source fixtures `tools/golden/report_decks/binsave_mmf_{p,q}.{sng,dbl}`,
`@FIXTURES@`-token-resolved on both engines) + exact per-action `GlobalResult`
rebuilt against scratch, and the load_shape unit test
`action_save_mmf_queues_eager_read_values` (replaces the old
`action_save_mmf_refuses_loudly`). Nothing left NOT_PORTED in the shape-save path.

**WPG.19 port — non-MemoryMapped file-backed numeric arrays (2026-07-11),
gate-green (modes + load_shape + props_roundtrip).** The Pascal `InterpretDblArray`
`file=`/`sngfile=`/`dblfile=` grammar (`Common/Utilities.pas:461-566`) for array
properties WITHOUT `MemoryMapping=Yes` — the proven sole blocker of the whole
ckt24 `MemoryMappingLoadShapes` family. **Two sites:** (1) LoadShape
`Mult`/`PMult`/`QMult`/`Hour` via `set_f64_array_raw` (`load_shape/accessors.rs`)
— now handles the non-MM directive (Hour too; Pascal `CustomSetRaw` has no MMF
branch for Hour), deferred as a `FileLoad::interp` and applied by
`apply_interp_file` (`compute.rs`) with the Pascal shrink rule: **mult/pmult
shrink `NumPoints`** (`:770`), **qmult/hour do NOT** (`:781/806`; their short-file
UB tail is not reproduced). (2) the generic double-array property path
(`class_props/parse.rs` + `DSSObjectHelper.pas:616-636`) — a file directive on
ANY class (`Spectrum %mag`, `XYcurve Xarray/Yarray`, …) is queued on
`DssObjData` (`GenericDblArrayFile`), read in the executive drain
(`exec/command.rs::apply_generic_dbl_array_file`) and applied via the typed
accessors with the generic `integerPtr^ := InterpretDblArray(...)` size-prop
shrink (probe-proven: `Spectrum NumHarm 8→4` on a 4-row `%mag` file). **Ordering
fix:** `action=normalize`/`ln` runs AFTER the (now-deferred) file read — probe-
proven the oracle normalizes the *read* peak — so `do_action` defers Normalize to
`run_deferred_actions` (drained after the file loads) when a file directive is
pending; a numeric `mult` still normalizes inline (no regression). `%result%`
resolves to `LastResultFile` in the drain (ported; not gate-committed to avoid
`Export`-content coupling — proven only in the oracle probe). New live-gate deck
`tests/corpus/modes/shape_filearr/` (fixtures via
`tools/decks/gen_shape_filearr_fixtures.py`): covers file/column=2/header=yes/
sngfile/dblfile/short-shrink/normalize + the two SITE-2 generics; oracle-validated
(compiles+solves+converges, 2 iters; full fingerprint bit-identical across two
oracle processes `6225f27521de809e`; feature-sensitive — stripping the directives
empties the shapes and the daily solve access-violates on the oracle). All 33
modes decks match the oracle (property-parity ON). **Corpus re-classify (honest,
NOT forced):** `Version8/Distrib/Examples/MemoryMappingLoadShapes/ckt24/master_ckt24.dss`
(yearly, 100 steps, 7522 nodes) now compiles + converges (2 iters, hour=100) and
its full complex node V matches the oracle to **5.6e-8 rel max** (median 2.8e-8,
faer-vs-KLU floor) after the entire 100-step trajectory — so WPG.19 reads/drives
the load shapes identically and its former blocker is gone. But the full live gate
(`kind="large"`) exposed a **new, deeper true blocker unrelated to WPG.19**: the
regulator-controlled substation transformer `Transformer.SubXFMR`
(`Regcontrol.SubXFMR_Regulator` winding=2 vreg=123 band=3 R=7 LDC delay=45)
diverges at the per-element **current** level — |diff| 7.2e-4 A (~4.7e-5 rel) at
step 0, exceeding the large-tier current tolerance while V stays regulated to
floor: the known RegControl/LDC tap-trajectory divergence family (MEMORY:
AutoTrans/RegControl stale tap). So it was **reclassified
`skipped_unsupported → skipped_needs_investigation`** (live-mismatch bucket, not
`solvable_now`); `solvable_now` stays 226 (67.5%). The 7 sibling ckt24 masters
(base + `-mm-*`/`-nomm`/`Run_Ckt24`) share the same former WPG.19 blocker (now
gone) and stay tagged pending individual triage.

**WP8.8 Phase-8 exit COMPLETE (2026-07-10), gate-green — PHASE 8 IS COMPLETE.**
All five exit steps ran; per the port-don't-defer rule the sweep also closed the
whole corpus-used executive-command tail on the way out. **Next: Phase 9
exotics — optional; stopping here is a complete usable simulator**
(PORTING_PLAN cumulative note). Remaining named work: actor mode
(`MULTITHREADING_PLAN.md` M2), A-Diakoptics, Pstcalc, WPG.19 (file-backed
`File=` arrays — now the proven sole blocker of the whole ckt24/SolarRamp
family), WPG.20 (MMF save), `JSON_EXPORT_PLAN.md`, `RESONANCE_PLAN.md` WP-R1.
Details:

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
  (~2.4e-7 rel) per the no-rationalizing rule.

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

**WPG.17 exit sweep COMPLETE — GAPS_PLAN executed (2026-07-09).** All three
exit-sweep items hold: **(1)** every `NOT_PORTED`/`no corpus case` marker
points at a live owner (never/UB/Phase-9/WPG.19/WPG.20/`JSON_EXPORT_PLAN.md`),
no test-absence deferral survives, and the registry diff vs `DSSClassDefs.pas`
is clean (49/49); **(2)** no `pending: true` remains in any family manifest;
**(3)** full gate + live corpus green, COVERAGE refreshed, PORTING_PLAN
cross-linked ("GAPS_PLAN executed", §Phase 9). Executed with the user-directed
"port, don't leave" extension — see the GAPS_PLAN §WPG.17 closure addendum for
the six ports of this round and the named follow-ups (WPG.19 non-MM `File=`
arrays, WPG.20 MMF-save, JSON plan in the repo root). **Corpus phase:** the
final `DSS_LIVE_CLASSIFY` pass probed all 43 `needs_investigation` candidates —
honestly 0 newly solvable (all sit on documented floors); new §3-validated
family decks this round: `xycurve_files`, `shape_mmf`, `gfm_dynamics`,
`pv_gfm_dynamics` (all in the anti-deletion floors); `FreqScan/Run_Scan.dss`
migrated to `solvable_now` (unblocked by the Plot-round `AddBusMarker`/
`ClearBusMarkers`; full live compare green). **The audit-requested IEEE123-GFM
spot-migration produced a REAL finding:** `Run_IEEE123Bus_GFMSnap.DSS` runs on
both engines but EXCEEDS the live band (step 0 entry 278, |diff| 9.03e-5 >
allowed 2.96e-5, ~3.7e-7 rel node V) — the earlier "purely scope" retag was
inaccurate; the deck now sits in `skipped_needs_investigation` with the
measured divergence and the whole `ieee123-gfm` family's notes cite it.
**RESOLVED 2026-07-09 (decomposition protocol executed): proven conditioning
floor, not a port bug.** The whole above-band gap is the zero-sequence common
mode of the two floating delta buses (StoBus/PVBus — delta xfmr winding +
delta DER, no zero-seq ground path): pinned only by the transformer anti-float
adder (−j1.4468e-6 S = 2·Y_PPM, measured from the live Y) vs ~452 S diagonal →
amplification ~3.1e8 × f64 ε ≈ 3.4e-8 rel per solve, iterating to the observed
2.6–3.2e-7 rel. Proof: system Y bit-identical, iteration counts equal at every
stage, differential/L-L quantities ≤2.3e-10 rel, all non-DER nodes in-band
(8.1e-9 rel); deck minus the DER pair collapses to 8.2e-9 rel; InvControl
removal changes nothing (GFM exonerated). Same un-pinnable class as AutoAuto —
NOT a TODO(compat), must not be "fixed". Full proof in the deck's manifest
note. **Sharpened 2026-07-10 (bitwise audit on user challenge):** engine solve
is deterministic and bit-equal to a fresh faer solve of the exported (Y, I);
the last solve's RHS bit-equals the captured injection and node_v bit-equals
the solve output (no index permutation, no stale I); Storage/PVSystem/
StickCurr/CalcVTerminalPhase/DoNormalSolution verified line-by-line vs Pascal.
Clean hex-bit-transport measurement (decimal-JSON transport perturbs the last
ulp — serde_json parses floats non-roundtrip without `float_roundtrip` — and
×3e8 poisons junk-subspace numbers): the whole gap is faer's **one-shot** LU
rounding in the anti-float subspace (zero-seq residual 8.7e-11 A vs KLU
7.3e-12 A, ~12×), not iteration accumulation; ONE iterative-refinement step →
2.1e-11 A / 9.4e-6 V, 3× under the band. Fix owner: `RESONANCE_PLAN.md` WP-R1
(second acceptance case added there). faer 0.24.4 probed: bit-identical to
0.24.0, no help. **Migrated to `solvable_now` (2026-07-10, user-directed):**
new tolerance tier `large_floating_delta` = `large` with `v_abs` 5e-4 V only
(the proven common-mode junk; ~1.8e-6 rel at the 277 V DER buses; everything
else — Y/YPrim/injection/currents/powers, which are L-L-based and immune to
the common mode — stays at `large` floors). Documented in
tests/TOLERANCE_NOTES.md §floating-delta; full live compare green (iterations
exact, all other channels at `large`). Retighten the tier to `large` when
WP-R1's refinement lands. COVERAGE: solvable_now 184→**185**,
needs_investigation 44→43. COVERAGE: solvable_now **184
(54.9%)**, unsupported 66→64, needs_investigation 43→44. Hygiene follow-up
(purged in `84b8dfa`): the `DSS_LIVE_CLASSIFY` oracle-side probe and a manual
`dss-cli` run wrote outputs (DI files / monitor CSVs) NEXT TO the vendored
decks — the CorpusGuard redirect covers the harness's Rust side but not the
classify oracle process cwd; extend it before the next classify pass (WP8.8
candidate — DONE at WP8.8: recursive CorpusGuard on both sides). Merge to
`main` remains explicit-request-only. WP8.8 executed 2026-07-10 (see the §1
frontier record) — **Phase 8 COMPLETE; next = Phase 9 (optional).**

**GFM audit settlement, part 1 (2026-07-09; both auditors: 0 Critical/Major on
the GFM math — every numeric pin independently reproduced on the oracle).**
audit-tests' Major settled: the FaultStudy/MonteFault GFM tests were smoke-only
("converges") against specific deterministic oracle observables — now pinned
(`storage_term1_kw` helper; MonteFault |P| = 748.7156 kW ±0.05, FaultStudy ≈0
— both matched the oracle on first run). The merge dropped the gfm branch's
all-no-op timing arm (unreachable duplicate + 107-divergent; `TotalTime` keeps
the MMF Pascal-faithful settable arm, `ExecOptions.pas:683-684`), and the
`set_time_elapsed_options_are_silent_noops` doc was corrected accordingly.
Owed to part 2 (the corpus phase): a PV-GFM dynamics live deck (§3 protocol)
and the IEEE123-GFM spot-migration both auditors requested to validate the
"trajectory-scope" retag.

**WPG.17 port — dynamics-mode GFM + StepTime (2026-07-09).** The WPG.13
deferral is closed: the grid-forming black-start droop now runs in dynamics
mode for **both** Storage and PVSystem — `IntegrateStates` GFM sub-branch
(`VDelta`→`ISPDelta` ramp + `FixPhaseAngle`, Storage.pas l.2886-2918 /
PVsystem.pas l.2324-2355), `DoDynamicMode` internal-voltage-source injection
(`BaseV = BasekV·1000·it[0]/iMaxPPhase`), Storage `CheckIfDelivering` (SOC
recharge + `Get_Variable` state 2/3/4), and PVSystem `InitStateVars` `it:=0`
GFM seed. Storage keeps `IMaxPhase` LOCAL / PVSystem overwrites `iMaxPPhase`
(reproduced exactly). New live-gate deck `tests/corpus/controls/gfm_dynamics.dss`
(islanded Storage black-starts to ~0.977 pu / −400.7 kW; two-process
bit-identical; non-convergent WITHOUT GFM) + storage/pvsystem unit pins, all
oracle-verified. **Dispatch refusal removed entirely** — Dynamic/FaultStudy/
MonteFault all drive the real GFM injection now. Empirically corrected the
spec's finding-5: the MonteFault "GFM Access Violation" is actually the
**empty-Faults NIL-deref** (`PickAFault`/`Randomize`, reproduces with plain
GFL too) — upstream UB, not GFM-related; the port already skips it safely
(`pick_a_fault → None`). `set steptime`/`processtime` are now silent no-ops
(Pascal has no Set arm for 106/108); the gfm branch's all-no-op timing arm was
dropped at merge — `TotalTime`(107) keeps the MMF branch's Pascal-faithful
settable arm. GFM/GFL_IEEE123 skip-notes retagged (Plot is a headless no-op,
steptime/dynamics-GFM ported; they stay deferred purely as a
full-feeder-trajectory scope decision — spot-check owed, see the GFM audit
settlement). Gate green.

**MMF audit settlement (2026-07-09; audit-code found 1 real Major, fixed).**
The port had dropped Pascal's `if UseMMF or ExternalMemory then Exit` guard at
the head of `SetMaxPandQ` (`LoadShape.pas:2048`), so `MaxP`/`MaxQ` were
recomputed from the eager MMF data (port 5.01/0.65 vs oracle 1/0 on the
`ls_pq` shape) — a latent mis-scale for any `useactual` load fed by an MMF
shape. **Fixed** (`compute.rs::set_max_p_and_q` early-returns on `use_mmf`),
pinned three ways: `pmax`/`qmax` added to the `ls_pq` manifest probe (live
oracle-compared), a new unit test (`mmf_leaves_max_p_and_q_at_defaults`,
peak≠1 fixture), and the guard comment cites the oracle probe. audit-tests
Minors settled: both new WPG.17 decks (`xycurve_files`, `shape_mmf`) added to
the `MODES_REQUIRED` anti-deletion floor; the sng/dbl MMF equivalence unit
tests now also pin the exact widened f64 values from the known bytes (were
Rust-vs-Rust only); the accept-set test doc corrected (only row 0 of the
non-uniform input is Pascal-faithful — past row 0 upstream is stride-misaligned
UB; the test pins the char filter, uniform files are deck-gated). Accepted:
the deck's `Get totaltime` line is non-gating decoration (wall-clock timers are
never numerically compared — documented in the manifest note). Open follow-up
(loud, honest): `Action=SngSave/DblSave` on an MMF shape keeps the trio-era
NOT_PORTED refusal — removing it needs an oracle probe of the MMF-save Q-side
semantics (`Assigned(dQ)` under MMF) before the bytes can be trusted. **[CLOSED
2026-07-11 by WPG.20 — probed, guard removed, byte-exact golden landed; see the
WPG.20 record at the top.]**

**WPG.17 port: LoadShape MemoryMapping + Set/Get TotalTime (2026-07-09).**
Ported the two former `master_ckt24` blockers. **(a) LoadShape `MemoryMapping=Yes`**
(`LoadShape.pas`): the MMF file readers (`sngfile`/`dblfile`/`csvfile`/`pqcsvfile`
properties + the raw `mult=(sngfile=…)` `CustomSetRaw` directive) now *eager-read*
the whole file into the f64 `p_mult`/`q_mult` with the MMF-path semantics —
`InterpretDblArrayMMF` text accept-set (`[46,58)`, drops sign/exponent →
`TODO(compat)`), `sngfile` widened into `dP` (f64, not `sP`), no `NumPoints`
shrink, the `(<mmFileCmd>)` property round-trip incl. the PQ display quirk
(`mult`→`(file=… column=2)`, `qmult`→`()`). The actual mmap I/O is not ported (no
observable numerics); single-column `csvfile=` under MMF is an upstream
div-by-zero (not reproduced). New hooks `DssObject::set_f64_array_raw` /
`f64_array_dump_override` (default no-op; only LoadShape opts in) on the
`DoubleArray` arm. **(b) `Set/Get processtime|totaltime|steptime`**
(`ExecOptions.pas:106-108`): only `totaltime` is settable; `processtime`/`steptime`
are Get-only no-ops on Set. The wall-clock timers are NOT accumulated (stay 0, per
the monitor channels-11/12 convention) — only the deterministic round-trip is
gated. **Deck:** new self-contained `tests/corpus/modes/shape_mmf/` (sng/dbl/raw
MMF + a pq shape whose exponent P column makes MMF observably differ from non-MM);
oracle-proven (converges; 2-process bit-identical fingerprint; feature-sensitive)
and live-matched. `master_ckt24.dss` retagged `unsupported_feature=file-backed-arrays`
(only the non-MM `File=` arrays LS_PhaseB/C remain, WPG.1); the 6 `var` decks stay
blocked. Full gate green.

**Trio audit settlement (2026-07-09, both independent opus auditors: 0
Critical/Major).** The one real Minor — the LoadShape Q-file `GlobalResult`
clause dropped Pascal `AppendGlobalResult`'s `', '` join (oracle emits
`],  Qmult=[`, comma + two spaces; `DSSGlobals.pas:452-459`) — fixed in
`exec/command.rs::write_shape_save` and pinned by new per-action
`GlobalResult` assertions in `binsave_matches_oracle` (all six action strings
exact). Coverage findings settled: LoadShape-side save tests added (no-`qmult`
→ no `_Q`, P-undefined error 622/623, MMF-refusal guard pinned);
`PreserveNodeVoltages` gains the missing RENUMBER-path unit test
(`vbus_restore_lands_on_renumbered_refs` — voltages follow bus node positions
to their new refs, the feature's actual point). The mid-solve
`flushed_records` Question accepted + documented at `monitor/sample.rs::add_dbl`
(unreachable from script; every multi-step loop ends with `SaveAll`).

**WPG.17 port — SngSave/DblSave + PreserveNodeVoltages + Monitor 1024-flush
(2026-07-09).** Three sweep-surfaced follow-ups landed. **(1)** LoadShape/TShape/
PriceShape `Action=SngSave/DblSave` binary writers (Pascal `SaveToDblFile`/
`SaveToSngFile`): the `do_action` hook can't reach `OutputDirectory`/`GlobalResult`,
so it queues a new `ShapeSave` (obj/base) drained in `edit_active` — mirrors the
`FileLoad` deferral. LoadShape splits `<name>_P`/`<name>_Q` (Q only `if Assigned(dQ)`),
TShape/PriceShape write the bare `<name>`; raw little-endian f32/f64. Pinned by a
**byte-exact** golden (`binsave_matches_oracle`, 8 `.bin` goldens via new
`gen_reports.py::gen_loadshape_binsave`). MMF-backed save stays LOUD-NOT_PORTED.
**[Superseded 2026-07-11 by WPG.20 — MMF-backed save is now ported + byte-exact
goldened; see the WPG.20 record at the top.]**
**(2)** `PreserveNodeVoltages` (`Ymatrix.pas:298/449`): `update_vbus`/
`restore_node_v_from_vbus` (Solution.pas:2377/2392) now bracket `build_y_matrix`
(was an inert NOT_PORTED note); net no-op while node count is stable (harmonics/
dynamics goldens unmoved), flag-sensitive unit tests. **(3)** Monitor 1024-single
flush: new `bufptr` cursor (Pascal `BufPtr`/`AddDblToBuffer`) drives the
`DumpProperties // Bufptr=`/`// Buffer=` remainder; both dump goldens unchanged.
Gate green except the pre-existing Oddie-venv-gated `modes_cases_match_oracle`
(opt-in EPRI/upgrade channel, `tools/opendss/.venv` absent in worktree —
environmental, orthogonal to these changes).

**Plot audit settlement (2026-07-09, both independent opus auditors: 0
Critical/Major).** audit-tests' one Major — the `min=` `TODO(compat)`
reproduction was unpinned — settled by extending `gen_plot_callback.py` 13→22
oracle-captured variants (min=0/min=5 asymmetry, showloops→MeterZones, C2/C3,
profilescale=120kft, type=xyz fallback, daisy buslist, a styled-arms combo, and
`circuit_marked`); the 13 original goldens regenerated byte-stable (determinism
proof) and the Rust driver passes all 22 — the port was already faithful in
every previously-unpinned arm. audit-code's one Minor — the 27-field `Markers`
object was display-complete but not user-settable — settled by PORTING the
marker-style `Set`/`Get` option arms (`ExecOptions.pas:615-682/978-1041`,
options 74-105 marker subset + the missing `Get Daisysize` `%-.6g`), pinned by
the `circuit_marked` golden end-to-end plus `marker_set_options_reach_payload_and_get`.
Its Question on `channels=` negative cast settled by reproducing the FPC
`Round`→`Cardinal` modulo-2^32 wrap (was saturating; `channels_cap_and_negative_wrap`
pins cap-at-51 + the wrap); the `buslist=` fresh-parser Question accepted
(tokenization identical for the value-list grammar; `file=` branch stays a
documented NOT_PORTED). Unsolved-guard unit coverage extended from `C` to all
guarded letters (`A C D G M P Z`).

**WPG.17 port: Plot/Visualize callback surface DONE (2026-07-09).** Ported
`DoPlotCmd` (`PlotOptions.pas:182`) — the full option parse (`PlotCommands`
abbrev table `PLOT_OPTIONS`, all 23 `ParamPointer` arms) + the `plotParams`
JSON assembly (30-key payload, `Markers` object, `BusMarkers[]`) + the #24732
unsolved guard — plus `AddBusMarker`/`ClearBusMarkers` (`ExecHelper.pas:4382` /
`Circuit.pas:3069`) and the `Visualize` JSON emission (`ExecHelper.pas:4184`).
Native `Dss::register_plot_callback`/`unregister_plot_callback`
(`exec/mod.rs`): a registered callback is the single opt-in gate that subsumes
BOTH Pascal gates (`NoFormsAllowed=False` AND `DSSPlotCallback<>NIL`) — the
console-only `AllowForms`/error-5096 machinery stays NOT_PORTED (no headless
analog). New `interpret_color_name`/`color_to_html` (`util.rs`, VCL `clXXX`
palette pinned by the golden), `BusMarker` + 27 marker fields on `Circuit`,
`DaisySize` + `Set Daisysize=` on `Dss`. TODO(compat): `type=Losses→LoadShape`
(letter L unconditional, PlotOptions.pas:274), unrecognized-type→'Circuit'
default, `min=` always flags `MinScaleIsSpecified`. Tests: 10 unit
(`exec/plot/tests.rs`, gating/guards/payloads/markers/visualize) + color unit +
a golden capture (`tools/golden/gen_plot_callback.py` via the oracle's
`DSS_RegisterPlotCallback`, 13 variants, `golden_plot_callback.rs` structural
compare). DI-family (`DI_plot`/`CompareCases`/`YearlyCurves`) stays NOT_PORTED
(upstream calls the callback with no NIL guard = UB). Full gate green.

**WPG.17 port: XYcurve file-input props (2026-07-09).** Ported `CSVFile`/
`SngFile`/`DblFile` (props 5/6/7, Pascal `Common/Utilities.pas`
`DoCSVFile`/`DoSngFile`/`DoDblFile` with `pA=XValues`, `pB=YValues`,
`OnlyLoadB=False`, `RoundA=False`; side-effect wiring `XYcurve.pas:337-361`)
via the deferred `FileLoad` path (mirrors spectrum/load_shape). `NOT_PORTED`
dropped from all three; XYcurve now has no remaining unported props. CSV sets
`npts:=i` unconditionally, sng/dbl shrink-only-when-short; malformed trailing
binary fragment is upstream UB (NOT reproduced, full-row gate). New live deck
`tests/corpus/modes/xycurve_files/` (fixtures via `tools/decks/gen_xycurve_fixtures.py`)
GAPS-3 oracle-validated; unit tests replace `file_props_are_not_ported`.
**Audit settlement (2 independent opus auditors, 0 Critical/Major):** one
factually-wrong comment fixed (a non-empty non-numeric CSV token *raises*
upstream — 58614 abort, `NumPoints` unchanged — it does not yield DblValue=0;
the port keeps the established spectrum convention of substituting 0.0,
divergent on malformed files only, now documented at `read_csv_file`); the
deferred-`FileLoad` same-command reordering (file prop + later array prop)
confirmed as a pre-existing accepted architectural limitation across all shape
classes and documented at `obj/base/mod.rs::FileLoad` (no corpus deck hits it).
One auditor independently re-ran the GAPS §3 proof (feature-sensitivity
fingerprint flip + inline-arrays bit-identity `00471ed4…`).

**WPG.17 exit sweep IN PROGRESS (2026-07-09).** Item-1 marker sweep + registry
diff done: the Rust registry matches `DSSClassDefs.pas` `CreateDSSClasses`
class-for-class (49/49); no `pending: true` remains in any family manifest
(item 2 already holds). The sweep found and fixed: **(a)** a real render bug —
`dump solution` hardcoded an empty `Set LDCurve=` where Pascal
`NameIfNotNil(LoadDurCurveObj)` (`Solution.pas:1816`) renders the WPG.3 curve
name (fixed in `report/save/dump/solution.rs`, new unit test
`dump_solution_renders_ldcurve_name`); **(b)** the `Visualize` guard errors —
`DoVisualizeCmd` (`ExecHelper.pas:4099`) runs #24722 (unsolved circuit) and
#282 (element not found) BEFORE the NIL-callback no-op, so they are
engine-observable and are now ported (`exec/command.rs::do_visualize_cmd`; the
wrong-type #282 arm is dead upstream — `GetCktElementIndex` resolves via
`Handle`, 0 for general objects — reproduced by resolving circuit-element
classes only; `Plot` stays a total no-op — `DoPlotCmd` exits before any guard
when the callback is NIL, `PlotOptions.pas:202-213`); **(c)** the defensive
unknown-solution-mode error now carries the Pascal-exact `#481` text (every
mode is ported; only the exec-intercepted AutoAdd hits the arm); **(d)** a
stale-owner comment pass — ReduceAlgs/`Save circuit`/MonteFault/Line-`spacing=`
/AutoAdd-`MakeBusList` claims of "NOT_PORTED" corrected to their landed WPs,
GFM dynamics stubs retagged from "WP7.7 GFM step" to the WPG.13 deferral (incl.
the two user-visible abort strings), `construct.rs` "unported classes" notes
dropped. **Sweep-surfaced porting follow-ups (user-directed 2026-07-09: port,
don't leave):** XYcurve `CSVFile`/`SngFile`/`DblFile` (the one surviving
test-absence deferral, PHASE5-era), LoadShape `MemoryMapping=yes` + `Set/Get
TotalTime` (the "no corpus deck uses it" claim was stale — 7
`MemoryMappingLoadShapes/ckt24` decks are blocked on exactly this), binary
shape outputs `SngSave`/`DblSave` (PHASE8 §4 scope), `PreserveNodeVoltages`
(`Ymatrix.pas:298/449`), the Monitor 1024-sample flush model, and the
dynamics-mode GFM branch (WPG.13 deferral — the one large item). Executing via
an ultracode multi-agent round (opus spec/port agents, opus-xhigh audits).

**WPG.13 GFM grid-forming mode — power-flow model + InvControl arm COMPLETE,
gate-green (2026-07-09).** Ported the grid-forming inverter voltage-source model
for **Storage** and **PVSystem** (snapshot/daily/direct/time-series): the shared
`TInvDynamicVars.CalcGFMYprim` (short-circuit admittance, R0/X0 1.9/5.7 defaults,
`a:=10` QuadSolver literal reproduced) + `CalcGFMVoltage` (balanced internal
phasors at `BaseV`) + `FixPhaseAngle` on `InvDynamicVars` (`inv_based_pce.rs`),
plus each element's `DoGFM_Mode` (`InjCurrent = YPrim·Vinternal`, the `IComp>0`
BaseV shrink) and the `TInvBasedPCE.GetCurrents` override (`Curr = YPrim·Vnode −
InjCurrent`). A GFM PCE is now a **voltage source**: it injects with the sources
(`GetSourceInjCurrents → GetPCInjCurr(TRUE)`) and is skipped in the ordinary PC
pass (`GetPCInjCurr(FALSE)`, the `is_gfm()` split in `solution/power_flow.rs`).
The `dispatch.rs:70` Storage/PVSystem GFM aborts are removed. Cites: `Storage.pas`
`CalcYPrimMatrix`/`DoGFM_Mode`/`CalcStorageModelContribution`, `PVsystem.pas`
`DoGFM_Mode`/`CalcYPrimMatrix` GFM branch (l.1300), `InvDynamics.pas`
`CalcGFMYprim`/`CalcGFMVoltage`, `InvBasedPCE.pas` `GetCurrents`/`CheckAmpsLimit`.
Also ported **InvControl `mode=GFM`** (ordinal 7): the `Sample` amps-limiter/
overload arm (`CheckAmpsLimit` sets `dynVars.IComp` → drives the next solve's
`DoGFM_Mode` BaseV shrink; `CheckOLInverter`) and the `DoPendingAction` overload-
drops-GFM arm, wired through new `InvDispatchEnv` GFM hooks. Two new
oracle-validated, feature-sensitive, two-process-deterministic controls decks
gate it live: `gfm_micro.dss` (islanded Storage GFM — island energised ~0.998 pu
/ ~400 kW WITH `ControlMode=GFM` vs a DEAD island 0 pu / 0 kW without) and
`gfm_invcontrol.dss` (InvControl mode=GFM `AmpLimit=400` amps-limiter BITES: ~158
kW throttled vs ~400 kW unlimited). Storage & PVSystem snapshot GFM bit-match the
oracle (islbus 2397.53 V, pvbus 277.06 V, 2 iters). Found+fixed a real bug: the
`check_amps_limit` currents were written to a scratch buffer while `get_currents`
still flagged the `Iterminal` cache fresh → the post-solve `Get_Powers` read a
stale (0) `Iterminal` → 0 kW. Fixed to `refresh_iterminal` (Pascal
`GetCurrents(Iterminal)` writes the real array). **NOT_PORTED (honest, deferred):**
the **dynamics-mode** GFM branch (`DoDynamicMode`/`IntegrateStates` GFM,
`VDelta`/`ISPDelta` droop, per-phase GFM `SolveModulation`) — the numerically
hardest path. The `GFM_IEEE123`/`GFL_IEEE123` corpus family stays **deferred**
(not `solvable_now`): re-probed, the block MOVED from the InvControl GFM mode-gate
to orthogonal blockers — `Plot Profile` (Snap decks) and `set steptime` + dynamics
GFM (Daily decks); forcing them → live gate red. `skipped_unsupported.json` notes
refreshed. Generator has no GFM (synchronous machine; `generator.pas` carries no
GFM code) — doc corrected. Gate: `cargo test --workspace` 0 failures; controls
live 53 (52 matched + 1 abort).
**WPG.16 GIC family (GICTransformer + GICLine + GICsource) — COMPLETE, live-green
(2026-07-09).** Ported all three GIC classes 1:1 from
`PDElements/GICTransformer.pas` (595) / `PCElements/GICLine.pas` (679) /
`PCElements/GICsource.pas` (478). **GICTransformer** (`elements/pd/gic_transformer/`):
shunt PDElement, pure-conductance blocks (GSU one-G1-block / Auto+YY two-block),
R1/R2 stored as G1/G2 via `INVERSE_VALUE`, `%R1`/`%R2` on the kV²/MVA base
(`FpctRSpecified` toggle), `SetBusX` write-fn promotes Nterms 2→4, `Type=Auto`
ties Bus2:=Bus3, VarCurve XYcurve ref, K default 2.2. Reproduced the upstream
`RecalcElementData` quirk `G2 := 100/(FZbase2·FpctR1)` (uses **%R1**, not %R2 —
`TODO(compat)` in `solve.rs`, oracle-confirmed R2=0.12696 for tg3). **GICLine**
(`elements/pc/gic_line/`): 2-terminal zero-seq Thevenin source, `Compute_VLine`
geodesy (ΔLat=Lat2−Lat1), series RL·FreqMult + blocking cap `Xc=−1/(2π·f·C·1e-6)`,
`DumpProperties` override (VE/VN/Z-Matrix). **GICsource** (`elements/pc/gic_source/`):
NON_PCPD Line-splicer — the executive resolves the same-named Line through the
foreign view (`edit_active`), `recalc` inserts a `gic_<name>` bus and rewrites the
Line's Bus2 via a new `RefAction::SetElementBus`; sign-flipped geodesy
(ΔLat=Lat1−Lat2), fixed series G=10000. Registered at the Pascal DSSClassList
slots (GICsource after IndMach012; GICLine/GICTransformer after ExpControl) with
new `ElemKind::GicLine`/`GicTransformer` (PC/PD lists). **One faer-vs-KLU guard:**
the series-only `CalcVoltageBases` snapshot floats a GICTransformer's X-side bus
(shunt-only reachability) → faer errors where KLU tolerates; mirrored the Reactor's
tiny (1e-10) series-diagonal stamp — invisible to the full-YPrim compare (only
`yprim_series` carries it). Wired the trivially-shared `LatLongCoords` command
(→ `do_bus_coords_cmd(true)`, swap-XY) so `GICExample/GIC_Example.dss` compiles
clean. **Gates:** the 4 asymmetric GIC decks (`gicline_gic`/`gictransformer_gic`/
`gicsource_gic`/`gic_midi`) flipped `pending:false` — full live compare + property
parity + the bus2 splice probes all green; `props_roundtrip` (+13 GIC scenarios),
6 GIC `Dump` goldens, and the `dump3_commands` golden (GIC sections rejoined) all
pass; `GIC_Example.dss` migrated `skipped_unsupported`→`solvable_now` (44 nodes,
matches oracle; COVERAGE 182→183, unsupported 67→66). No `NOT_PORTED` on the
element path (`WriteVarOutputRecord`/`Export GICMvar` is the only deferred piece —
the consuming export verb is unported, tracked separately, not the element).

**WPG.15 audit + settlement (2026-07-09), gate-green.** Independent opus audit
(code xhigh + tests high) of the merged AutoTrans work: **port faithful 1:1**
(SetNodeRef / CalcY_Terminal / GICBuildYTerminal / GetCurrents fold / Get_Losses
AUTOTRANS special case / Line `ConvertZinvToPosSeqR` / RegControl proxy all
verified loop-for-loop), **tests clean** (0 findings — all 7 decks are
feature-sensitive via `selected_elements ["*"]` YPrim bijection). The audit
**independently re-confirmed the deck integrity** flagged during the two-agent
race: the `autotrans_gic` tampering is fully reversed (net diff vs the `18d88a7`
original = one comment; the switch line + `mvasc3=2e6` are restored — pins both
the Line-GIC branch and AutoTrans `GICBuildYTerminal`), and the `autotrans_snap`
physical-source edit is a legitimate conditioning-floor fix (real non-zero
winding currents, no tolerance loosened; `AutoAuto.dss` correctly moved to
`needs_investigation` rather than edited). Two **minor, provably-masked**
findings settled: (1) the `calc_y_terminal` GIC gate reconstructs
`FreqMult·BaseFrequency` instead of the true `Solution.Frequency` — divergent
only on a mid-solve `recalc(1.0)` at <0.51 Hz, always overwritten by CalcYPrim
before any read → **documented in-code** as benign (not `TODO(compat)`; the Line
port reads `sys.frequency` directly). (2) `winding_currents_result` uses
`.norm()`/`.to_degrees()` vs the FPC `cabs`/`cdang` `TODO(compat)` helpers —
**pre-existing, copied verbatim from Transformer** (`transformer/yterminal.rs`),
masked by `%g`; a de-compat-pass follow-up for BOTH classes, not a WPG.15
regression. Separately-noted pre-existing follow-up (Fable, out of WPG.15
scope): `energymeter::capture_metered` doesn't count `Fault` as a PD element
(Pascal `BASECLASSMASK` would). Full `cargo test --workspace` green on the
merged head (`a408a30`, 32 binaries, 0 fail, live corpus incl.).

**WPG.10 — InvControl `mode=voltwatt` + `combimode=VV_VW` over Storage COMPLETE
(2026-07-08), gate-green.** The Storage-typed volt-watt branches now dispatch
instead of erroring (`guard_storage_vw` deleted). `Calc_PBase`
(`InvControl.pas:2850`) gains the Storage `%Available` (yaxis 0) arm reading the
*live* `TStorageObj.DCkW` (Pascal sets `FDCkW:=0` for Storage and reads the
`DCkW` property; plumbed via a new `InvDispatchEnv::der_storage_dckw` →
`Storage::dckw`, bumped `pub(super)`→`pub(crate)`) — yaxis 1/2/3 are identical
to PVSystem. `CalcPVWcurve_limitpu` (`InvControl.pas:2950`) gains the Storage
charge/discharge curve pick by `StorageState`+`FVWStateRequested` (discharging →
`voltwatt_curve`; charging-with-CH → `voltwattCH_curve`; the requested-flip swaps
them; idling / charging-no-CH → no limit `1.0`). `DoPendingAction`'s VOLTWATT
(`:1376`) and VV_VW (`:1456`) Storage arms push `kWRequested`(+`kvarRequested`) +
`SetNominalDEROutput`; the Storage `FVWOperation` reset compares `|presentkW|`
(no `|PLimitVW|>0` guard) and the two Storage event strings are verbatim (note
the `to ** kW=` / `to** kW=` comma/space quirks vs PVSystem — kept so the event
log compares equal). The actual VW biting is the already-ported Storage
`kWOut_Calc` (`storage/nominal.rs`) requesting/limiting region, driven by
`VWmode`+`kWRequested`; `der_set_nominal` already propagates a Storage
state-flip's `system_y_changed` (the WP7.4 concern the old defer-note raised).
`DerSnap` carries `storage_state`/`vw_state_requested`. Decks
`invcontrol_storage_vw.dss` + `invcontrol_storage_vv_vw.dss` flipped
`pending:false` — controls live gate **46 matched / 4 pending** (per-step Storage
P/Q/%stored/state + event log + meters/monitors + full model exact vs the pinned
oracle). The two `tests.rs` pins were extended from "errors loudly" to the real
dispatch, plus a new charging-CH-curve-selection unit test. No
`TODO(compat)`/`NOT_PORTED` added. Scope: only the `inv_control` module, the
`dispatch.rs` env bridge, the `Storage::dckw` visibility, and the 2 controls
manifest flags.

**WPG.10 audit settlement (2026-07-08):** the two live decks were a **dead gate** —
their source (`mvasc3=200`) + line held the storage bus at V≈0.998 pu, below the
`vw` knee (x=1.05), so `CalcPVWcurve_limitpu` returned `PLimitVWpu=1.0` (flat
region) and the battery discharged at FULL / then depleted to the 20% reserve and
idled *regardless of the InvControl* (removing it reproduced the identical
trajectory). The port code was **audited clean** — the gate, not the logic, was
inert. Both decks **strengthened** so the volt-watt law genuinely bites (weak
source `mvasc3=20` + resistive line `r1=2.0` + a 2 MW discharge lift the bus into
the limiting region; battery sized so it never depletes over the 6 steps).
Oracle-proven per GAPS §3 (two-process bit-identical + feature-sensitive
WITH-vs-WITHOUT): **VW** settles at ~792 kW @ V≈1.038 pu (vs full 2000 kW @ 1.088
WITHOUT); **VV_VW** at ~1306 kW + 657 kvar absorbed @ V≈1.022 pu (vs 2000 kW + 0
kvar); %stored at step 6 diverges 61.9→35.1 (VW) / 50.5→35.1 (VV_VW) — far above
the micro tolerance. The faithful port matches the oracle exactly (same fixpoint
AND iteration count: 20 for VW, 36 for VV_VW). Minor coverage gap closed: the
yaxis-0 `%Available` live-`DCkW` `Calc_PBase` branch and the `FVWStateRequested`
curve-swap now have dedicated `inv_control/tests.rs` unit tests (the gate decks
use the yaxis-1 default + no requested-flip).

**WPG.15 AutoTrans Stage C (2026-07-08): RegControl + corpus — COMPLETE, live-green.**
RegControl's `transformer=` now resolves against **both** classes (Pascal
`Transf_Or_AutoTrans_ProxyClass`, `RegControl.pas:264`) via a new
`PropDef::object_ref_two_classes` + a `parse.rs` second-class fallback; the
control-loop dispatch (`solution/controls/dispatch.rs`) and `RegControl`'s
`set_object_ref` accept either `Transformer` or `AutoTrans`. **AutoTrans
implements `ControlledTransformer`** (tap accessors `present_tap`/`set_present_tap`
(`:1432`)/`winding_tap_data`/`wdg_connection`/`base_voltage`, `GetWindingVoltages`
Series arm (`:1604`), `power_into` reading the terminal's `TermNodeRef` since the
`SetNodeRef` magic desyncs it from the flat `NodeRef`). Added a `full_name()` to
the trait so the Series-connection guard (`RegControl.pas:1009`) reports the
concrete class. **Monitor mode-2 tap monitor** now accepts AutoTrans alongside
Transformer (`Monitor.pas:542-543`). The 4 controls decks
(`autotrans_reg`/`autotrans_both`/`midi_autotrans`/`midi_autotrans_both`) flipped
`pending:false`, **live-green** (event logs equal, `tapnum`/`taps`/`wdgcurrents`
probes exact, the `both` decks pin the snapshot→daily transition). **Corpus
re-classify:** all AutoTrans corpus decks now compile+solve (0 errors — AutoTrans
+ `BatchEdit` land it); `AutoAuto.dss` (both copies) moved
`unsupported_class=autotrans` → `skipped_needs_investigation` (class ported; a
near-ideal-source ~5e-9-rel node-V floor, same class as autotrans_snap — vendored,
its short-circuit checks need `mvasc3=2e6`); Auto1bus/Auto3bus/AutoHLT stay (they
build the unit from *regular* transformers, unchanged by WPG.15). COVERAGE.md
refreshed (unsupported 73→71, needs_investigation 41→43). WPG.15 is now COMPLETE
(A/B/C all merged-ready). fmt/clippy/`cargo test --workspace` green.

**WPG.15 Stage C closing audit (2026-07-08):** the remaining Transformer-only
dispatch points now accept either proxy member via
`transformer::as_controlled_transformer` — `Export`/`Show Taps` rows
(`report/{export,show}/taps.rs`), the live `TapNum` view
(`exec/view.rs::regcontrol_tap_numbers`) and the property-read resync
(`exec/command.rs`); EnergyMeter's metered-element PD check
(`energymeter/accessors.rs`, Pascal `BASECLASSMASK = PD_ELEMENT`) counts
AutoTrans (no transformer special-casing: `IsTransformerElement` matches
XFMR_ELEMENT only, `Utilities.pas:728`). The Series-connection guard now
reproduces the Pascal **exception** semantics: `RegControl::sample` returns
`Result`, and the dispatch maps the raise to `SampleControlDevices`'
(`Solution.pas:1974`) error-484 + "Solution aborted." path (it previously logged
per-phase and kept solving). The proxy not-found message renders Pascal's
`TProxyClass` name `(Transformer|AutoTrans)`. Checked the `enabled=yesa` corpus
quirk against the spec: dss_capi `InterpretYesNo` (`Utilities.pas:400`) reads the
*first char* → `yesa` = TRUE; the Rust `interpret_yes_no` is identical (the
GAPS_PLAN "parses as false" warning does not apply to this engine; the decks
carrying it stay `needs_investigation` regardless).

**WPG.15 deck-edit audit + Line GIC port (2026-07-08).** Re-verified both
Stage-B corpus-deck edits against the ORIGINAL decks with a full decomposition
probe (oracle vs Rust, per-element YPrim/current diffs, iteration counts):
- `autotrans_snap` (60 Hz): the edit is **legitimate** — on the original
  near-ideal source deck every element YPrim matches the oracle to ≤1e-16 rel
  (AutoTrans.t1 2e-20, t2 1e-18), iteration counts are equal (2=2), the auto
  units' currents are at/below the V-noise floor (t1 2.3e-8 rel, t2 3e-15),
  and only the `mvasc3=2e6` Vsource + `r1=1e-6` switch currents diverge
  (5.4e-5 rel = a ~1e-9-rel V wobble divided by 1e-6 Ω) with node-V at
  ~2.8e-8 rel — a proven faer-vs-KLU conditioning floor, not a maskable bug.
  The physical-source deck pins the same auto model at the tighter micro tier.
- `autotrans_gic` (0.1 Hz): the edit had **sidestepped a real gap** — the
  original deck's 33 % `Line.line1` YPrim divergence was the **unported Line
  GIC branch** (`TLineObj.ConvertZinvToPosSeqR`, `Line.pas:1297/2086`: below
  0.51 Hz the series Zinv collapses to the diagonal positive-sequence
  resistance `Zs−Zm`, X dropped — cross-phase coupling vanishes), not
  conditioning (the deck's 0.1 Hz state is zero-current on both engines, so no
  cancellation exists there). **Ported the branch** into `line/solve.rs`
  (per the "port gaps immediately" rule) and **restored the original deck**,
  which now passes the full micro-tier compare (Line YPrim 1.2e-16 rel) and
  pins both the Line GIC conversion and AutoTrans `GICBuildYTerminal`. No other
  live deck solves below 0.51 Hz (the remaining GIC decks are WPG.16-pending).

**WPG.15 AutoTrans Stage B (2026-07-08): the auto electrical model — live-green.**
Ported the solve path loop-for-loop: `CalcYPrim` (`AutoTrans.pas:1199` —
`BuildYPrimComponent` for series+shunt, **no `AddNeutralToY`**); the `SetNodeRef`
"Magic happens here" node aliasing (`:875`, series winding's 2nd node → common
winding's 1st) wired through a **new virtual `CktElement::set_node_ref`** the
circuit build now dispatches; the `GetCurrents` series→X fold (`:1663`);
`GICBuildYTerminal` (`:1823`, the `Frequency<0.51` resistance-only branch, ppm as
conductance) selected in `CalcY_Terminal`. **Found + fixed a real port bug I
missed in Stage A:** `TDSSCktElement.Get_Losses` has an **AUTOTRANS_ELEMENT
special case** (`CktElement.pas:618`) — sum power into only the *first* `Nphases`
conductors of each terminal, skipping the second-half, so the series current
(aliased onto the common node and folded by `GetCurrents`) is not double-counted
(the base path gave −130 MW vs the oracle's 0.4 MW). Overrode `losses()`
accordingly. **Decomposition proof (CLAUDE.md conditioning rule):** the AutoTrans
element YPrim + assembled system Y are **bit-exact to ~3e-13** vs the oracle
(probed both engines); the residual was purely the near-ideal EPRI source
(`mvasc3=2e6` + an `r1=1e-6` switch, copied from AutoAuto for its short-circuit
CHECKS) making the *source-side* Vsource/switch currents an un-pinnable
near-cancellation of the ~1e-8 convergence floor. `autotrans_snap`/`autotrans_gic`
do a load-flow, so both re-fed from a **physical 345 kV source directly on the
auto** (the auto units stay faithful) — clean at the tightest `micro` tier. (The
GIC deck also surfaced a *Line* `switch=yes`+`r0=` cross-phase YPrim quirk,
sidestepped by the same direct feed — a Line-model note, not AutoTrans.) The 3
asymmetric decks (`autotrans_snap`/`autotrans_gic`/`midi_autotrans_asym`) flipped
`pending:false`, **live-green** (full YNodeV/currents/powers/losses/YPrim +
`wdgcurrents` probes). The 4 controls decks still error (RegControl→AutoTrans is
Stage C). fmt/clippy/`cargo test --workspace` green. Next: Stage C (RegControl
proxy + flip the 4 controls decks).

**WPG.15 AutoTrans Stage A (2026-07-08): the class skeleton — props + dump +
`RecalcElementData`/`CalcY_Terminal`, no solve.** New module
`crates/dss-core/src/elements/pd/auto_trans/` (mod/windings/yterminal/accessors/
dump/save/tests), cloned from the ported Transformer and adapted to the auto:
**41 class props** (49 incl. the PDClass/CktElement tails + Like) in Pascal
`TAutoTransProp` order (`AutoTrans.pas:364`) — `XHX/XHT/XXT` (trap_zero 7/35/30),
a dedicated `AutoTransConnectionEnum {wye=0,delta=1,series=2}` (registered in
`EnumRegistry`, aliases y/ln→wye, ll→delta, s→series), **no XfmrCode, no
RNeut/XNeut**, `Core`/`RDCOhms` in the winding-definition section, and
`WdgCurrents` carrying `READS_VTERMINAL`. `PropertySideEffects` (`:574`) force
wdg1=Series/wdg2=Wye; `RecalcElementData` (`:919`) derives the series `kVSeries`
VBase, `Rdc`, anti-float and Norm/EmergAmps (the default `RDCOhms=5.957…` /
`NormAmps=6.194…` are oracle-exact); `CalcY_Terminal` (`:1856`, incl. the auto
`ZCorrected`/`puXst`/`GICBuildYTerminal` corrections) is ported for the dump. The
**solve path (`CalcYPrim`, the `SetNodeRef` node aliasing, the `GetCurrents`
fold) is `NOT_PORTED` behind a loud error** (owner: Stage B) — a solve of an
AutoTrans-bearing circuit aborts (the ymatrix builder lifts the queued error to
`SolutionAbort`), so the 7 pending corpus decks stay red. Registered in
`construct.rs` between IndMach012 and InvControl (Pascal DSSClassDefs.pas:270),
with `ElemKind::AutoTrans` → `pd_elements` + a separate `auto_transformers` list
(Pascal `AUTOTRANS_ELEMENT`, NOT `Transformers`) and `is_pd_element` recognition
(meter zones + isolated report). **Gate A green:** `props_roundtrip`
(`tests/golden/props/autotrans.json`, 6 scenarios) + byte-exact
`dump_autotrans`/`dump_autotrans3` goldens + the `[AutoTrans]` section rejoined
`dump3_commands` (unstripped) + 4 module unit tests; fmt/clippy/`cargo test
--workspace` all green (13 corpus pending unchanged). Next: Stage B (the auto
electrical model + flip the 3 asymmetric decks).

**GAPS round-2 (2026-07-08): five more GAPS ports (WPG.4/5/6/9/11) + four
audit-fix worktrees, all merged to `phase-8-reporting` (HEAD `e82517b`); each
port had an opus `/audit-code` + `/audit-tests` pair and each fix an opus
audit-tests — full gate green after every merge (fmt/clippy/`cargo test
--workspace`; live corpus: modes family now 0 pending, controls 6, asymmetric 7
= 13 pending total). Every audit-code returned 0 correctness bugs across the 5
ports; every audit-fix was opus-verified feature-sensitive via mutation
trials.** Landed:
- **WPG.6 — Newton (`DoNewtonSolution`)**: `Set algorithm=Newton` via the
  current-injection loop; `newton.dss` + new `newton_feeder.dss` live-green
  (exact iters — oracle-confirmed 2/3 — 1e-9 V/I). Surfaced + reproduced a proven
  upstream quirk: post-Newton `Powers`/`Losses` read a one-step-stale `Iterminal`
  (`S ≠ V·conj(I)`, both P and Q; `Powers` is per-conductor `V·conj(I)`, no √3),
  `TODO(compat)` in `exec/view.rs::snapshot_elements`, oracle-probe-proven
  (`investigations/newton_stale_iterminal_bug_report.md`; CLAUDE.md bug index
  4→5). It is the *only* Newton-vs-normal gate discriminator (V/I/iters coincide).
  **De-compat note:** the quirk is **live-oracle-pinned** (no golden), and the
  EPRI channel confirms it is present in every official rev incl. the latest
  (v9.8/v10.2/v11.0, all fingerprint 0.478 kVA) — so removal is NOT an oracle bump
  but a documented live-gate exclusion (VSConverter-style gate-around) + a
  replacement Newton assertion, best a Rust-only white-box `do_newton_solution`
  unit test (OPEN follow-up; capped-iteration/black-box can't distinguish Newton
  from normal here — proven).
- **WPG.4 — Monte Carlo M1/M2/M3/MF + FPC RNG**: FPC 3.2.2 RTL MT19937
  (`support/mathutil/rng.rs`) + SolveMonte1/2/3/MonteFault; monte1/2/3/montefault
  live-green under `random=none`. RNG pins **confirmed captured-FPC-exact** — an
  `ppcrossx64` x86_64/SSE2 run reproduced all 20 pins bit-for-bit (`tools/fpc/
  mtwist/`, self-contained probe + captured output; the earlier
  "transcription-equivalence assumed" is closed). Audit-fix added 17 fixed-seed
  RNG-dispatch tests (§2.1-compliant, externally-derived).
- **WPG.5 — AutoAdd (`TAutoAdd.Solve`)**: capacity search + `UseAuxCurrents`/
  `AddInAuxCurrents`; `autoadd.dss` live-green (winner b3, teardown-segfault
  handled). Audit-fix added the CAPADD gate `autoadd_cap.dss` (winner b2 +
  `Capacitor.cadd1` oracle-pinned) and tightened the improvement-figure floor
  1e-4 → 1e-10 (measured faer-vs-KLU delta 1.66e-12).
- **WPG.9 — InvControl `ControlModel=Exponential` (TPICtrl PI)** across
  VV/AVR/DRC/VV_DRC; `invcontrol_expmodel.dss` live-green. Audit-fix unit-covered
  the DRC/VV_DRC/AVR branches (only VOLTVAR was exercised).
- **WPG.11 — StorageController seasonal targets + `Set SeasonRating/
  SeasonSignal`**; `storagecontroller_seasonal.dss` live-green (fixed the deck's
  unbounded XYcurve extrapolation that hung the oracle itself). Audit-fix
  unit-covered the 3 `get_dynamic_target` fallbacks.
- Accepted deviations (documented, not bugs): WPG.4 random-mode draw-count (§2.1
  non-pinnable), WPG.5 `SetGeneratorDispRef`/`GlobalResult` (unreachable), WPG.11
  season flags on `Circuit` vs `Dss` (unreachable Clear).
Done GAPS WPGs now: 1,2,3,4,5,6,7,8,9,11,14. **Remaining: WPG.10 (InvControl
VW/VV_VW Storage), WPG.12 (Relay TD21/Generic), WPG.13 (GFM), WPG.15 (AutoTrans),
WPG.16 (GIC family), WPG.18 (CIM XML) + WPG.17 (exit sweep). Then WP8.8 phase
exit + the explicit-request-only merge to `main`.**

**Parallel WPG/WP8.5b round (2026-07-08): five isolated-worktree agents merged
into `phase-8-reporting` (HEAD `de6f56c`), each with its own opus
`/audit-code` + `/audit-tests` pair; full gate green after merge** (fmt/clippy/
`cargo test --workspace`, 0 failures; corpus_live: modes 22 matched/6 pending,
asymmetric 29/7, controls 42 matched + **1 abort-both**/8 pending, **178 solvable
cases, 169 with full property parity**). Landed:
- **WPG.8 — Reactor `RCurve`/`LCurve`** (harmonic freq-dependent R/L; `GetYValue(FYprimFreq)`
  in Hz, GIC-clamped; snapshot-clone XYcurve). Deck `reactor_rlcurve` live.
- **WPG.3 — LD1/LD2 + `Set LDCurve`** (`SolveLD1`/`SolveLD2`, option 27). **Fixed 2 real
  Load-dispatch bugs:** the Monte2/3+LD1/2 `SetNominalLoad` arm (missing → ~40-unit divergence)
  and — via audit — **`SolveMode::PeakDay` loads skipping their daily shape** (pre-existing latent;
  Pascal `Load.pas:1082` PEAKDAY = GrowthFactor + CalcDailyMult, no LoadMultiplier; only Load omitted
  it). New oracle-gated `peakday.dss`. Decks `ld1`/`ld2` live.
- **WPG.7 — CapControl `type=Follow` + `ControlSignal`** (FOLLOWCONTROL Sample arm, snapshot-clone
  LoadShape). **Fixed a real Major (via audit):** the NIL-`ControlSignal` branch dropped Pascal's
  `SolutionAbort:=True` — now routed through a new **first-class "both engines abort" harness path**
  (`expect_solve_abort`/`run_and_compare_abort`) gated by `capcontrol_follow_noshape.dss`; `sample()`
  is now `#[must_use]`. Deck `capcontrol_follow` live.
- **WPG.2 — `SolveGeneralTime` (mode=Time)** + a monitor Save/flush-fidelity refactor
  (`flushed_records` cursor; Pascal `Channel`/`dblHour` read only the flushed stream; `TODO(compat)`
  `[0.0]` placeholder for dss-python's header-only `IMonitors.Channel`). **Ported a KNOWN unported
  corpus-used option in-step: `Set LoadShapeClass=`** (`ActiveLoadShapeClass`; Load/Gen/Storage/PV
  GENERALTIME dispatch; `Load.pas:1056`/`Generator.pas:1129`/`Storage.pas:1318`/`PVsystem.pas:1174`) —
  the root cause of the deck being non-discriminating. Decks `generaltime`/`_yearly`/`_duty` live.
- **WP8.5b — Corpus property parity** (the addendum): oracle `all_properties` + `element_properties`
  accessor + harness `compare_all_properties`/`SKIP_PROPS` (each exclusion proof-cited in
  TOLERANCE_NOTES). **Found + fixed a real latent bug the live-model gate had missed for phases:
  `RegControl.TapNum` rendered a stale `tap_snap`** (Pascal `Get_TapNum` reads the transformer's live
  `PresentTap[TapWinding]`) — now resynced at the `refresh_vterminal_if_marked` choke point. Property
  gate ON for asymmetric + controls + modes + solvable-feeders (169/178); comparator hardened to a
  whole-string identity guard + case-exact value compare + a cursor-agreement transformer-skip gate.
Merge note: WPG.3's `solve_ld1/ld2` lacked `save_all_monitors` (a no-op on their branch, real after
WPG.2's flush refactor) — added at merge so LD1/LD2 monitors flush (else `[0.0]`); `solve_general_time`
correctly stays flush-free. (WP8.8 executed 2026-07-10 — see the §1 frontier
record; the merge to `main` stays explicit-request-only.)

**Test-infra cleanup (2026-07-07):** the `tests/corpus/gaps/` staging family is
**dissolved** — its 46 oracle-validated decks now live in their permanent
families (`asymmetric/` +9, `controls/` +13, the new `modes/` family: 24 solve-
mode/algorithm/input-format/executive-verb decks) with `pending: true` in the
family manifests, and the **pending loud-error gate is implemented**
(`corpus_live.rs::assert_pending_errors_loudly`, run by the shared
`family_cases_match_oracle` machinery that replaced the copy-pasted
asymmetric/controls runners; all 46 pending decks proven to error loudly,
2026-07-07). Graduation is now a manifest **flag-flip**, not a file move
(GAPS_PLAN §3.1 reworked; PHASE8_PLAN WP8.6/8.7 paths synced). Deck bytes
untouched. Multi-file cases live in a deck-named subfolder
(`modes/shape_binfiles/`); `gen_gaps_binshapes.py` → `tools/decks/
gen_shape_fixtures.py`. **known_diffs v2:** the EPRI triage catalog moved
`tools/opendss/known_diffs.json` → **`tests/corpus/known_diffs.json`** (next
to the manifests it triages) and gained a `kind` field — `diff` (default,
reason-matched divergence triage) vs `skip` (case not expected to
run/converge on the listed revs; skipped up front, reported under
`known_skipped` by both `corpus_live_opendss` and `ab_compare.py`). Schema
documented in the catalog's own comment block; still never consulted by the
mandatory gate.

**Porting-era `phaseN` names retired (2026-07-07).** The golden dirs,
generators, and Rust gates carried porting-phase numbers that mean nothing on
their own. Renamed to what they cover (all `git mv`; goldens **not**
regenerated — the sole content edit was 3 embedded variant-path strings in
`feeders_controlsoff.json`, no numerics). The `phase7` bucket (60 files, name
said "line constants" but held DER + harmonics too) was **split by content**
into three families sharing one `harness::scenario` gate. This table is the
old→new map; historical STATUS entries and the `PHASEn_PLAN.md` docs keep the
old names on purpose (they date the work).

| old | new |
|---|---|
| `tests/golden/phase4.json` + `phase4/` | `feeders_controlsoff.json` + `feeders_controlsoff/` |
| `tests/golden/phase5/` | `timeseries_controls/` |
| `tests/golden/phase6/` | `metering_monitors/` |
| `tests/golden/phase7/` (60) | split → `line_constants/` (5) + `der_controls/` (45) + `harmonics/` (10) |
| `tests/golden/phase7_protection/` | `protection/` |
| `tests/golden/phase8/` | `reports/` |
| `tools/golden/gen_phase4.py` | `gen_feeders_controlsoff.py` |
| `tools/golden/gen_phase5.py` | `gen_timeseries_controls.py` |
| `tools/golden/gen_phase6.py` | `gen_metering_monitors.py` |
| `tools/golden/gen_phase7.py` | `gen_der_lines_harmonics.py` (one file, scenario→dir map) |
| `tools/golden/gen_phase7_protection.py` | `gen_protection.py` |
| `tools/golden/gen_phase8.py` | `gen_reports.py` |
| `tools/golden/phase8_decks/` | `report_decks/` |
| `tools/golden/probe_line_{constants,spacing}_phase7.py` | `probe_line_{constants,spacing}.py` |
| `golden_phase5.rs` | `golden_timeseries_controls.rs` |
| `golden_phase6.rs` | `golden_metering_monitors.rs` |
| `golden_phase7.rs` | split → `golden_line_constants.rs` + `golden_der_controls.rs` + `golden_harmonics.rs` |
| `golden_phase7_protection.rs` | `golden_protection.rs` |
| `golden_phase8.rs` | `golden_reports.rs` |

Already-semantic names kept as-is: `checkpoints/`, `props/`, `slice`,
`allocation`, `autoadd_reduce`, `gendispatcher`, `ieee*`, `reliability`,
`parser`, `golden_feeders.rs`, `golden_feeders_controls.rs`, `golden_smoke.rs`.

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

**WPG.14 (Isource) COMPLETE (2026-07-07), gate-green + audited.** New
`elements/pc/isource/` (vsource-template; 11 props in Pascal enum order),
`ElemKind::Source` registered right after VSource (DSSClassDefs.pas:198;
class order pinned by the regenerated `dump3_commands` — `[Isource]` restored
between `[VSource]`/`[VCCS]`). All-zero YPrim; `GetBaseCurr`/`GetInjCurrents`
loop-for-loop incl. the harmonic ScanType vs fundamental Sequence rotation
and the `|Freq−SrcFrequency|<EPSILON2` gate. TODO(compat): Isource never
latches `Bus2Defined` (no `TProp.Bus2` case in PropertySideEffects, unlike
VSource) — **both directions now oracle-pinned** in
`tests/golden/props/isource.json` (6 scenarios): `bus1= bus2=` sticks
(`isource_full`), `bus2= bus1=` is clobbered to the grounded-Y default
(`isource_bus2_clobbered_by_bus1`, audit follow-up). A thin non-override
`dump_body` gives the NON_PCPD `TPCElement` the right dump ordering
(byte-exact `dump_isource{,_debug}` goldens). 7 corpus decks flipped
`pending:false` across asymmetric/controls/modes; `Microgrid/ISource`
migrated → solvable_now. **Audit: port correct 1:1, no blockers** (full
corpus_live re-run green in-worktree); settled: the VCCS-position comment
wording fixed; the harmonic-rotation and monitor-channel coverage notes
recorded (aggregate injection is oracle-pinned across 7 harmonics; per-unit
rotation isolation left to the family gate).

**WPG.1 (shape file inputs) COMPLETE (2026-07-07), gate-green + audited.**
LoadShape `SngFile`/`DblFile`/`PQCSVFile` (own `Read*File` readers), TShape/
PriceShape `SngFile`/`DblFile` (shared `ScalarShapeCore` over
`Utilities.pas` `DoSngFile`/`DoDblFile`), GrowthShape `CSVFile`/`SngFile`/
`DblFile`; the deferred-`FileLoad` mechanism gained a `binary` flag
(executive reads raw bytes; `DssObject::apply_binary_file_load`). Both
binary branches ported (fixed = bare LE f32/f64 stream; `Interval=0` =
`(hour,value)` pairs). **The agent also found + fixed a real oracle-harness
bug**: `oracle_server.py::capture_probes` used `ckt.SetActiveElement`, which
silently no-ops for `DSS_OBJECT` classes (LoadShape/TShape/…) — every probe
on such a class read a stale CktElement; rerouted through the `? element.prop`
executive query (verified bit-identical for CktElement probes; guarded
incidentally by the now-live DSS_OBJECT probes). **Audit findings, settled:**
(1) *Major (real port bug, oracle-proven):* GrowthShape `read_csv_file`
rounded the year column, but Pascal `DoCSVFile` **ignores its RoundA
argument** (the rounding loop exists only in `DoSngFile`/`DoDblFile`) —
fractional CSV years now kept verbatim (unit test + live `g_csvf` probe);
Sng/Dbl still round (`g_sng` probe). (2) *Major (tests) + Minor (code):* the
single-precision storage path was scoped out with the deck built to avoid
it → **settled by porting Pascal's f32 storage for real** (`s_p`/`s_h`
authoritative + widened f64 views): `UseFloat32`/`UseFloat64` at the exact
Pascal call sites, `GetMultAtHourSingle` (mixed f32/f64 interpolation),
`RCDMeanAndStdDevSingle`/`CurveMeanAndStdDevSingle` (f32 `S` accumulator;
FPC's `Sqrt(Single)` overload; the `0.5` literal typed Single in the
all-single product — every rounding step **bit-exact vs an FPC 3.2.2 probe**,
committed as `tools/fpc/single_prec_probe.pas`, asserted verbatim in
`sng_single_storage_matches_fpc_bit_exact`), `DoNormalizeSingle`, MakeLike
singles. New live case `ls_sng0i` (anchors at even hours, full-significand
mults → the daily solve *interpolates* in single precision) +
`t_sng0`/`g_csvf`/`g_sng` probes — **feature-sensitivity proven: disabling
the f32 path fails `modes_cases_match_oracle`; enabled, it is green.**
`sQ` single storage is script-unreachable (greppable NOT_PORTED note); the
float32 truncated-pair no-shrink is upstream uninitialized-heap UB, not
reproduced (greppable marker).

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

**Phase 7 COMPLETE** (WP7.1–WP7.10, branch `phase-7-extended-elements`,
gate-green) but **NOT merged to `main`** (per-phase merge = explicit-request-only
HARD STOP; `phase-8-reporting` builds on top of it). Roll-up + archives in **§1e**
([`docs/phase-records/phase-7.md`](docs/phase-records/phase-7.md) +
`phase-7-wp{1..7}.md`). Tracked-open Phase-7 deferrals (both Plot-blocked, zero
corpus payoff): the **GFM grid-forming inverter mode** and the **Generic/TD21
relay `Sample`**. Current scores: dss-core **lib 742**, **`solvable_now` 168**;
oracle pinned to dss-python 0.15.7 (backend = dss_capi 0.14.5,
`tools/golden/PIN.txt`).

Phase 7 = DER, protection, line constants, harmonics, dynamics (PORTING_PLAN.md
§Phase 7, the largest phase ~18%). Earlier phases merged to `main` (newest first):
**Phase 6** (WP6.1–WP6.10 — meters/monitors/topology/Generator + the 8500-node gate
+ the live corpus gate; `--no-ff` `b98223a`, `main` not pushed to origin)
→ [record](docs/phase-records/phase-6.md); **Phase 5** (`10d3550`), **Phase 4**
(`5f27a25`). Their full logs and the per-WP detail live under `docs/phase-records/`
(§1b–1d indexes them) and the §1 table below.

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

**WPG.18 audit (all Stages A–F) + settlement (2026-07-09), gate-green.** Full
five-way line-for-line audit of the ~8500-line CIM exporter (writer/dispatch/UUID;
IEEE1547; scaffolding/EnergySource/DER/loads/ECP; caps/CapControl/reactors/lines/
switches/catalog; transformers/AutoTrans/banks/RegControl) against
`ExportCIMXML.pas`/`ExportOptions.pas`/`NamedObject.pas`/`Circuit.pas`. **Verdict:
faithful 1:1** on every gated and common path; **three defects found + fixed**, all
golden-uncaught (content-neutral or on branches no gate deck reaches):
- **[Major] IEEE1547 empty-`DERList` branch dropped the `Enabled` guard**
  (`WriteCIM` `3044`/`3052`): the fallback iterating `ckt.storages`/`pv_systems`
  gated only on the downcast, so a **disabled** DER would leak a
  `DERDynamics.PowerElectronicsConnection` ref + overwrite the nameplate. Fixed
  with the faithful enabled check (`ieee1547.rs`). Empirically the branch is hard
  to reach — an InvControl with no `DERList=` **auto-populates** it with all DER,
  so the *named*-DER branch (which includes disabled DER, matching Pascal
  `SetElementActive`) fires instead; that named branch is now byte-verified vs the
  oracle (a scratch deck with a disabled PV/Storage → 0 diff, 3 refs incl.
  disabled, both engines).
- **[Major] fragments-mode `@lastexportfile`/`last_result_file`** pointed at the
  last file written (`<base>_DYN.xml`) instead of Pascal's suffix-less base
  (`SetLastResultFile(DSS, FileName)`, `ExportOptions.pas:635` — `ExportCDPSM`
  appends `_<PRF>.xml` to its own by-value copy). The 7 XML files are byte-exact
  (goldens compare content only, so uncaught), but the executive state a script
  reads via `$lastexportfile$` diverged. Fixed to point at the base
  (`exec/report.rs`).
- **[Minor] `base_v_name`/`op_lim_v_name`** used native `{:.4}` (ties-to-even) not
  the ties-away `ff_fixed` that `op_lim_i_name` uses — **latent** (identical on
  every realistic base voltage; a divergence needs an odd multiple of 0.03125),
  the tie STATUS had deferred to the exit sweep. Aligned all three name-formatters
  on `ff_fixed` now (zero golden change); WPG.17 no longer owns this item.

**Accepted (tracked, not defects):** the IEEE1547 `FindSignalTerminals`/`MonBus`
signal path deviations (PC-scan omits caps/reactors, creation-order vs class-list
ordering, obj-type fallback for exotic monitored classes, `FinishNameplate` 0/0)
— all **non-byte-gated** (no solvable oracle deck runs `export cim100` with a
`MonBus`) and documented in-module; the 5 `TODO(compat)` markers (double-`b0ch`
typo, wye-cap hard-grounded, delta/wye `grounded`-prefix, load `allow_sec`, the
`2.3026`≈ln10) all justified reproductions; the case-3 XfmrCode synthesis is a
transient no-mutation view (safer than Pascal); the Gen/Load ECP `.spectrum` raw
string vs PV/Storage `spectrum_obj` name is output-identical for valid decks.
Post-fix gate green (fmt/clippy/`cargo test --workspace` incl. `golden_cim` 9/9).

**WPG.18 CIM Stage F (DER + IEEE1547 + fragments mode) COMPLETE, gate-green**
(2026-07-09, on `phase-8-reporting @ 7c74344`). The last WPG.18 stage — every
`NOT_PORTED` arm is now a real port (`rg NOT_PORTED crates/dss-core/src/cim/` =
empty). Three parts, all **byte-exact vs the pinned oracle** (combined **and**
the 7 fragment files):
- **DER sweeps** (`ExportCIMXML.pas:3503-3612`, in `cim/export.rs`): Generator →
  `SynchronousMachine` (+ `RotatingMachine.p/q/ratedS/ratedU`), PVSystem →
  `PhotovoltaicUnit` + `PowerElectronicsConnection` (maxP/minP, maxIFault,
  ratedS/U, the kvarlimit-set-vs-default `maxQ`/`minQ` arms), Storage →
  `BatteryUnit` (ratedE/storedE/`BatteryStateEnum`) + `PowerElectronicsConnection`.
  Shared `attach_der_phases`/`write_der_phase` (the non-3φ `SynchronousMachinePhase`
  / `PowerElectronicsConnectionPhase` breakdown incl. the `<0.25 kV` s1/s2 split),
  `ConverterControlEnum`, and `add_generator_ecp`/`add_solar_ecp`/`add_storage_ecp`
  (the `Gen:`/`PV:`/`Bat:` EnergyConnectionProfile keys, the PV T-shape trio).
  Constants `GEN=83`/`STORAGE=171`/`PVSYSTEM=195` added to `cktelem_dss_obj_type`.
- **IEEE1547 controller** (new `cim/ieee1547.rs`, `ExportCIMXML.pas:2318-3183`):
  one reused `Ieee1547Controller` pulls each enabled InvControl then ExpControl.
  `PullFromInvControl` (the vvc/voltwatt/voltwattCH/wattvar curve-scan loops incl.
  the `dec(i)` re-scan, the mode/combi enable logic, the mode-3 DRC-as-AVR curve
  synthesis), `PullFromExpControl`, `SetDefaults` catA/catB, `SetPhotovoltaic`/
  `SetStorageNameplate` + `FinishNameplate`, `WriteCIM` (`DERIEEEType1` +
  `DERNameplateData`/`Applied` + VoltVar/WattVar/ConstPF/ConstQ/VoltWatt settings)
  → all into the **Dyn** profile. Reproduces the defined upstream quirk that the
  reused controller **appends** DER names in `PullFromExpControl` (`pDERNames.Add`,
  not `Assign`) so the ExpControl's `DERIEEEType1` references BOTH controls' DERs.
  New public getters on `InvControl`/`ExpControl` (curves + mode/scalar fields) —
  no behavior change. `TODO(compat)` on the truncated `2.3026` (= `ln 10`).
  **NOT byte-gated (honest):** the `FindSignalTerminals`/`RemoteInputSignal` path
  (only reached with a control `MonBus`) is ported (a bus-name `getPDEatBus`/
  `getPCEatBus` scan) but unverified — no *solvable* oracle deck runs `export
  cim100` with a MonBus InvControl (probed: they abort the oracle solve). The
  gated local-monitoring case (empty `MonBuses` ⇒ no signals) is what `cim_der`
  exercises.
- **Fragments mode** (`Export CIM100Fragments`, ptr 20): `writer.rs` gains a
  stateful `Writer` carrying both modes — combined writes one FUN buffer,
  fragments routes each line to its per-profile buffer with `WriteCimLn`'s
  auto-`StartFreeInstance` and `EndInstance`'s close-every-open-profile logic
  (`FD_Create`/`FD_Destroy`, `4729-4763`). The combined path stays byte-identical
  (all 7 pre-existing goldens unchanged). `exec/report.rs` ptr 20 wired to
  `export_cim100_fragments` (`<CaseName>_CIM100_<PRF>.xml`). All `writer::`/
  `power_xfmr`/`export` call sites migrated `&mut String` → `&mut Writer`
  (mechanical; the combined output is unchanged, proven by the 7 unchanged
  goldens + a scratch cim_xfmr fragment A/B vs oracle).

**Gate:** new deck `cim_der.dss` (Gen 3φ+1φ+daily; PV 3φ default-limit + 3φ
kvarMax-set + 2φ 0.208 secondary + daily/TDaily; Storage discharging/idling/2φ
charging; InvControl volt-var catB curve; ExpControl) → combined golden
`cim_der.xml` (1256 lines) **and** 7 fragment goldens `cim_der_<PRF>.xml`; tests
`cim_der` + `cim_der_fragments` in `golden_cim.rs`. `gen_cim.py` gained a
`"fragments"` case flag + the 7-file completeness proof. Full gate green: `cargo
fmt --check`, `cargo clippy --workspace -D warnings`, `cargo test --workspace`
(incl. `corpus_live` 13/13 + `golden_cim` 9/9). **WPG.18 CIM XML export is now
complete (Stages A–F).** Remaining in GAPS_PLAN: **WPG.17 (exit sweep)**.

**WPG.18 CIM Stage E (transformers + AutoTrans + banks + RegControl) COMPLETE,
gate-green** (branch `worktree-agent-a32f944d01be61c20`, 2026-07-09; base reset
from the stale toy-repo `d04fbd4` to `phase-8-reporting @ 5f114cf`, `.inputs` +
`tools/opendss/.venv` junctions recreated — the known worktree-race traps).
Ported `Common/ExportCIMXML.pas:3783-4270` into a new byte-faithful split module
`crates/dss-core/src/cim/power_xfmr.rs` (extracted per SPLITTING_RULES as the arm
landed; `export.rs` calls `write_transformers` then `write_reg_controls` where its
two Stage-E `NOT_PORTED` guards sat). **AutoTrans sweep** (`3804-3932`): always
balanced-3φ `PowerTransformerEnd`s + `TransformerMeshImpedance`/
`TransformerCoreAdmittance` (no tanks), vector group `YNa`(2w)/`YNad1`(3w); i=1→Y
ungrounded, i=2→A grounded (rground/xground=0), i≥3→D clock=1; core `b = -%imag/
100/zbase` (note the **minus**, unlike regular transformers). **Three regular
transformer cases** (`3934-4155`): case 1 (no code & 3φ) → `PowerTransformerEnd` +
mesh/core with the grounded/rground/xground branch on `Winding.Connection`/`Rneut`/
`NodeRef[(i-1)·Nconds+Nphases]`; case 2 (has code) → `TransformerTank` +
`TransformerTankEnd` (via `XfmrTankPhasesAndGround`, `1531`) referencing the code's
`TransformerTankInfo`; case 3 (no code & not 3φ) → the transformer's own winding
web is written as a synthesized `CIMXfmrCode_<name>` tank-info (Pascal
`PullFromTransformer` reproduced as a transient view — no circuit mutation). **All
XfmrCodes written unconditionally** (`WriteXfmrCode`, `2133`: `TransformerTankInfo`
+ per-winding `TransformerEndInfo` + `NoLoadTest` + per-pair `ShortCircuitTest`),
real codes first (class order) then the synthesized case-3 codes (transformer
order). **Banks** (`TCIMBankObject`, `698-805`, ported as a local `CimBank`
struct): `BuildVectorGroup` from accumulated winding phases/connections/ground/
clock, `PowerTransformer` written last in insertion order (autotrans banks, then
transformer banks), leading `=` stripped from no-bank names. **RegControl arm**
(`4198-4270`): skipped for AutoTrans-controlled regs (only `Transformer`);
`TapChangerControl` (mode=voltage, targetValue=Vreg, targetDeadband=Bandwidth, LDC,
reversible block, `maxLimitVoltage`=Vlimit|MaxTap·v1) + `RatioTapChanger`
(stepVoltageIncrement=100·TapIncrement, highStep=NumTaps div 2, SSH
`TapChanger.step` = the **live** `tap_num_live` reading — guards the RegControl
stale-tap bug from MEMORY). New writers `winding_connection_kind_node`(1620)/
`winding_connection_enum`(1440)/`transformer_control_enum`(1482, a no-op — the
Pascal body is commented out). Constants `XFMR_DSS_OBJ_TYPE=34`(=32|2),
`AUTOTRANS_DSS_OBJ_TYPE=298`(=296|2) added to `cktelem_dss_obj_type`. Read-only
getters added (each cites its Pascal field): Transformer `winding_kvll`/`wdg_kva`/
`wdg_resistance`/`winding_rneut`/`winding_xneut`/`winding_num_taps`/`xsc_val`/
`windings`/`xsc`/`pct_no_load_loss`/`pct_imag`/`norm_max_hkva`/`emerg_max_hkva`/
`xfmr_bank`/`xfmr_code_ref`; AutoTrans `winding_kvll`/`wdg_kva`/`wdg_resistance`/
`xsc_val`/`pct_no_load_loss`/`pct_imag`/`xfmr_bank`; RegControl `vreg`/`bandwidth`/
`pt_ratio`/`ct_rating`/`ldc_r`/`ldc_x`/`ldc_active`/`tap_delay`/`vlimit`/
`is_reversible`/`reverse_neutral`/`rev_delay`/`rev_power_threshold`/`rev_r`/`rev_x`/
`rev_vreg`/`rev_bandwidth`. **Gate E golden** `cim_xfmr.dss` (2×AutoTrans YNad1/YNa
+ case-1 Δ-Y + 3-winding Δ-tertiary + case-2 XfmrCode tank + a 3-unit single-phase
regulator bank = case 3 + RegControl), byte-exact vs the pinned oracle
(`tests/golden/cim/cim_xfmr.xml`, 2174 lines; fixture 297 keys). The
solver-dependent SSH `TapChanger.step` matched byte-exact ⇒ Rust and oracle
converge to the same regulator taps. **Two full corpus-feeder goldens** land too:
**IEEE13** (`IEEE13Nodeckt.xml`, 4042 lines — sub/XFM1 case 1 + 3 single-phase
regulators case 3 + RegControl + 37 ACLineSegments) and **IEEE123**
(`ieee123.xml`, 15 242 lines — 7 regulators + XFM1 + 16 LoadBreakSwitches + 359
ACLineSegments; the master only defines the feeder so the golden harness gained a
`post` command list = `["solve"]`, buscoords omitted → 0,0 positions). Generating
IEEE13 surfaced **one pre-existing Stage-A bug**: `op_lim_i_name` used Rust's
native `{:.1}` (round-half-to-**even**), but FPC `FloatToStrF(ffFixed)` rounds
ties **away from zero** — IEEE13's reg `EmergAmps = 2499/2.4 = exactly 1041.25`
gave oracle `1041.3` vs Rust `1041.2`, so the `OpLimI=…` key missed the fixture
and drew a random v4. Fixed with a faithful `ff_fixed` helper (scale → `round()`
[ties-away] → rebuild from the scaled integer); non-tie values are unchanged, so
Stage A–D goldens are unaffected (`base_v_name`/`op_lim_v_name` carry the same
latent tie issue — noted for the exit sweep; no feeder hits it yet).
`rg NOT_PORTED crates/dss-core/src/cim/` = the single remaining **Stage F** arm
(Generator/PVSystem/Storage/InvControl/ExpControl + fragments mode).

**WPG.18 CIM Stage D (capacitors + CapControls + series reactors) COMPLETE,
gate-green** (branch `worktree-agent-a5a4c8b61573a0b06`, 2026-07-09; base reset
from the stale toy-repo `d04fbd4` to `phase-8-reporting @ b05e6f6`, `.inputs` +
`tools/opendss/.venv` junctions recreated — the known worktree-race traps).
Replaced the three Stage D `NOT_PORTED` arms with the real ports
(`Common/ExportCIMXML.pas:3684-3781`, `4274-4292`). **ShuntCapacitor sweep**
(`3684-3731`) → `LinearShuntCompensator`: `bPerSection = 0.001·Totalkvar/NomKV²/
NumSteps`, `nomU = 1000·NomKV`, wye → `grounded=TRUE`+`b0PerSection=bPerSection`
vs delta → `grounded=FALSE`+`b0PerSection=0` (reproduced upstream quirk: the delta
branch emits `grounded` under the `LinearShuntCompensator.` prefix, wye under
`ShuntCompensator.` — `TODO(compat)`), `normal/maximumSections=NumSteps`,
`aVRDelay` = the last controlling CapControl's `OnDelayVal`, SSH `sections` = count
of in-service steps (`States[i]>0`), then **AttachCapPhases** (`1703`, non-3φ:
per-phase `LinearShuntCompensatorPhase`, `bph = bPerSection/NPhases`, delta →
`DeltaPhaseString`) + `WriteTerminals(NormAmps,EmergAmps)`. **CapControl sweep**
(`3733-3781`) → `RegulatingControl`: Location→cap loc, `RegulatingCondEq`→cap,
`.Terminal` = `GetTermUuid(MonitoredElement, ElementTerminal)`, `MonitoredPhaseNode`
from ported **FirstPhaseString** (`1391`) shifted by `PTPhase`, `RegulatingControlEnum`
by type (current/voltage/kvar/time/pf → currentFlow/voltage/reactivePower/
timeScheduled/powerFactor; FOLLOW → no `.mode` line, matching the Pascal `case`),
`discrete=TRUE`, `enabled`, `targetValue = val·0.5·(v1+v2)`, `targetDeadband =
val·(v2−v1)` where val/v1/v2 depend on control type. **Reactor sweep** (`4274-4292`)
→ `SeriesCompensator`: `r/x/r0/x0` all from `Z.re/.im` (r0=r, x0=x), `WriteTerminals`.
New writer helpers `regulating_control_enum`/`monitored_phase_node`; new constants
`CAP_DSS_OBJ_TYPE = 106` (`CAP_ELEMENT 104 | PD 2`), `REACTOR_DSS_OBJ_TYPE = 138`
(`REACTOR_ELEMENT 136 | PD 2`), `CAP_CTRL_*` ordinals, and `cktelem_dss_obj_type`
(class-name → `DSSObjType`, resolving a CapControl's *monitored* element's terminal
key generically — the port carries no runtime `DSSObjType`; covers Vsource/Line/
Load/Capacitor/Reactor, returns `None` → a loud error for classes Stage E/F add).
Added read-only getters citing the Pascal fields: `Capacitor::{total_kvar,nom_kv,
num_steps,connection,norm_amps,emerg_amps}`, `Reactor::{z,norm_amps,emerg_amps}`,
`CapControl::{control_type,pt_phase,pt_ratio_val,ct_ratio_val,on_value,off_value,
pf_on_value,pf_off_value,on_delay_val}` — no behavior change. Gate deck
`cim_shunt.dss` (wye+delta+1φ caps, a voltage-mode + a current-mode CapControl,
a series reactor) → byte-exact golden `tests/golden/cim/cim_shunt.xml` (788 lines).
Full `cargo test --workspace` green (861 unit + corpus_live 13/13; golden_cim 4/4).
Remaining `NOT_PORTED` arms: Stage E (Transformer/AutoTrans/RegControl), Stage F
(Generator/PVSystem/Storage/InvControl/ExpControl). **Next = Stage E** (transformers
+ AutoTrans + regulators).

**WPG.18 CIM Stage C (lines + switches + conductor catalog) COMPLETE, gate-green**
(branch `worktree-agent-aa4705404a39d2918`, 2026-07-09; base reset from the stale
toy-repo `d04fbd4` to `phase-8-reporting @ 253102f`, `.inputs` + `tools/opendss/.venv`
junctions recreated — the known worktree-race traps). Replaced the seven Stage C
`NOT_PORTED` arms (Line + LineCode/WireData/TSData/CNData/LineGeometry/LineSpacing)
with the real ports (`Common/ExportCIMXML.pas:4294-4625`). **Line/switch sweep**
(`4294-4408`): the ACLineSegment impedance-source cascade (LineCode →
`ACLineSegment.PerLengthImpedance` ref; else Geometry/Spacing →
`ACLineSegment.WireSpacingInfo` ref; else symmetric-3φ inline `r/x/bch/…` — with
the reproduced upstream double-`b0ch` typo `4367`; else the on-the-fly `_PUZ`
`PerLengthPhaseImpedance` + lower-triangular `PhaseImpedanceData`), the
LoadBreakSwitch/Fuse/Breaker/Recloser branch via **ParseSwitchClass** (`451`,
scanning `Circuit.controls`), **AttachLinePhases** (`1627`)/**AttachSwitchPhases**
(`1661`) driven by ported **PhaseOrderString** (`548`). **LineCode catalog**
(`4493-4547`): `PerLengthSequenceImpedance` (sym-3φ) / else `PerLengthPhaseImpedance`,
with the `Units=UNITS_NONE` fix-up loop. **Conductor catalog**: `OverheadWireInfo`
(**WriteWireData** `2277`), `TapeShieldCableInfo` (+**WriteCableData** `2232` +
**WriteTapeData** `2250`), `ConcentricNeutralCableInfo` (+**WriteConcData** `2262`),
`WireSpacingInfo` + `WirePosition` for both **LineGeometry** (`4575`) and
**LineSpacing** (`4601`). CIM refs from a `Line` resolve the **master** catalog
object's UUID by name (`class_obj_uuid`) — the `Line` carries only snapshot clones
with their own un-preloaded UUID slots. New writer helpers `phase_side_node`/
`conductor_usage_enum`/`conductor_insulation_enum`; new `LINE_DSS_OBJ_TYPE = 50`
(`LINE_ELEMENT 48 | PD_ELEMENT 2`). Added read-only getters to `line_geometry/mod.rs`
(`nwires`/`fx`/`fy`/`funits`/`conductor_is_overhead`/`conductor`, citing the Pascal
`NWires`/`Xcoord`/`Ycoord`/`Units`/`PhaseChoice`/`ConductorData` accessors) — no
behavior change. Gate deck `cim_lines.dss` (coded sym + coded matrix 3φ/1φ +
sym-inline + matrix-inline PUZ + geometry + spacing + CN/TS cable lines + a Fuse
switch, all from IEEE13-derived known-good data) → byte-exact golden
`tests/golden/cim/cim_lines.xml` (1525 lines). All Stage C output derives from
**input** data (catalog values, input matrices), never a Carson solve, so the golden
is solver-independent (byte-exact regardless of faer-vs-KLU). Full `cargo test
--workspace` green (861 unit + corpus_live 13/13; golden_cim 3/3). Remaining
`NOT_PORTED` arms: Stage D (Capacitor/Reactor), Stage E (Transformer/AutoTrans/
RegControl), Stage F (Generator/PVSystem/Storage/InvControl/ExpControl). **Next =
Stage D** (shunt capacitors + series reactors).

**WPG.18 CIM Stage B (loads) COMPLETE, gate-green** (branch
`worktree-agent-a716de72c8222fb0c`, 2026-07-09; base reset from the stale toy-repo
`d04fbd4` to `phase-8-reporting @ 879f39e`, `.inputs`/`.venv`/`tools/opendss/.venv`
junctions recreated — the known worktree-race traps). Replaced the Stage A
`Load (EnergyConsumer)` `NOT_PORTED` arm with the real **EnergyConsumer** sweep
(`Common/ExportCIMXML.pas:4448-4491`): per-load `StartInstance` +
`CircuitNode`/`VbaseNode`, the `FLoadModel` → `LoadResponseCharacteristic` id map
(model 8/Zipv writes no `LoadResponse` node — no CIM arm), `EnergyConsumer.p/q`
(SSH), `customerCount`, wye→`grounded=true`/delta→`grounded=false` (the
`TODO(compat)` hard-coded-grounded quirk `4478`), plus **AttachLoadPhases**
(`1749`) + **AttachSecondaryPhases** (`1736`) → per-phase `EnergyConsumerPhase`
(the 3-phase early-exit, the split-secondary `s1`/`s2` branch, and the
non-secondary per-phase loop, driven by ported **PhaseString** `491` +
**DeltaPhaseString** `638`). Added the **ECP** machinery (`TECPObject` +
`ECPList`/`ECPHash` as a keyed insertion-ordered `EcpList`, `AddLoadECP` `1130`):
one `EnergyConnectionProfile` per distinct DSS shape/spectrum profile, keyed
`Load:<daily>:<duty>:<growth>:<yearly>:<cvr>:<spectrum>` — the spectrum position
is `NameIfNotNil(SpectrumObj)` (the default `defaultload` appears in the **key**
but the `dssSpectrum` **node** is default-suppressed), and the yearly position
picks up OpenDSS's daily→yearly copy. `op_limits`/ECP scratch hoisted to
`export_cdpsm` scope and threaded through EnergySource + EnergyConsumer; the
closing `EnergyConnectionProfile` (`4628`) and `OperationalLimitSet`/`CurrentLimit`
(`4658`) sweeps are now live (loads pass `norm=emerg=0`, so no op-limits yet).
New writer helpers `phase_kind_node`/`shunt_connection_kind_node`; new
`cim/export.rs` load `DSSObjType = 59` (`LOAD_ELEMENT 56 | PC_ELEMENT 3`, the
`GetTermUuid` key prefix). Gate deck `cim_load.dss` (wye/delta/motor/CVR 3-phase +
1-/2-phase secondaries + a daily-shape ECP; models 1..5) → byte-exact golden
`tests/golden/cim/cim_load.xml` (759 lines). Note: a load-bearing circuit with no
series branches (lines/transformers = Stage C/E) leaves every bus base at 0 in the
oracle's `SetVoltageBases` (deterministic — the 3-process fixture check + oracle
probe agree), so every non-3-phase load reads as secondary here; all branches are
still exercised and the export is byte-identical. Full `cargo test --workspace`
green (corpus_live 13/13, 90.5s). Remaining `NOT_PORTED` arms: Stage C
(Lines/LineCode/WireData/TSData/CNData/LineGeometry/LineSpacing), Stage D
(Capacitor/Reactor), Stage E (Transformer/AutoTrans/RegControl), Stage F
(Generator/PVSystem/Storage/InvControl/ExpControl). **Next = Stage C** (lines +
switches + conductor catalog).

**WPG.18 CIM XML export — Stage A COMPLETE, gate-green** (branch
`worktree-agent-a6aed205a5f4a066e`, 2026-07-09; base reset from a stale toy-repo
`d04fbd4` to `phase-8-reporting @ eba653e`, `.inputs`/`tools/opendss/.venv`
junctions recreated — both known worktree-race traps). Ported the CIM100 XML
writer core + the whole `ExportCDPSM` skeleton (`Common/ExportCIMXML.pas:
3203-4707`) top-to-bottom (GAPS_PLAN WPG.18 decision 7): regions/substation/
feeder/location, the six fixed `OperationalLimitType`s, the `BaseVoltage` +
op-limit-set sweep over `LegalVoltageBases`, the bus → `TopologicalNode`/
`ConnectivityNode` sweep, the swing-bus `TopologicalIsland`, the fixed
7-model `LoadResponseCharacteristic` catalog (unconditional — oracle-probed to
fire even on a load-free circuit), the closing `OperationalLimitSet`/
`CurrentLimit` sweep, plus the full **EnergySource** (Vsource) per-object
sweep. Every not-yet-ported class arm (Generator/PVSystem/Storage/InvControl/
ExpControl/Capacitor/Reactor/Transformer/AutoTrans/RegControl/Line/LineCode/
WireData/TSData/CNData/LineGeometry/LineSpacing) is a scoped `NOT_PORTED`
error firing only when the circuit actually contains instances of that class
(`cim/export.rs::not_ported_if_any`) — never a silent drop; the file still
completes every ported section. New module `crates/dss-core/src/cim/{writer,
export}.rs` (writer = pure formatter: `WriteCimLn`/`Start(Free)Instance`/
`EndInstance`/node helpers/`StartCIMFile`, combined-mode-only per Stage A
scope; export = the `ExportCDPSM` control flow), `Uuid::to_cim_string`
(`NamedObject.pas:64` `UUIDToCIMString` — braces-stripped **uppercase**, no
leading underscore, oracle-probed; the WP brief's "leading `_`, lowercase"
description does not match the source or the oracle and was not followed),
`CimExporter::{get_term_uuid, get_base_v_uuid, get_op_lim_v_uuid,
get_op_lim_i_uuid}` (the UUID-surface wrappers over the WP8.6 substrate).
`exec/report.rs` wires `Export CIM100` (ptr 21) to `cim::export::export_cdpsm`
via the `subs`/`subg`/`g`/`fil`/`fid`/`sid`/`sg`/`rg` option loop
(`ExportOptions.pas:227-259`, `AssignNewUUID`'s brace-wrap-then-parse +
error-303-on-malformed-GUID); `Export CIM100Fragments` (ptr 20) is a single
top-level `NOT_PORTED` until Stage F. Gate mechanism (decision 1-2): new
`tools/golden/gen_cim.py` runs the 3-process fixture recipe (compute fixture →
produce golden → repeat-and-assert-bit-identical) against the pinned oracle;
new `crates/dss-core/tests/golden_cim.rs` replays the same deck
(`tools/golden/cim_decks/cim_src.dss`) + fixture and byte-compares
(CRLF-normalized only, zero tolerance) against `tests/golden/cim/cim_src.xml`
— green. Full `cargo test --workspace` green (incl. the live corpus oracle
gate, 87s). **Next = Stage B** (loads + `WriteLoadModel`/ECP machinery,
`rg "NOT_PORTED" crates/dss-core/src/cim/` still non-empty through Stage E).

**WPG.12 Relay `TD21`/`Generic` Sample logic COMPLETE, gate-green** (branch
`worktree-agent-aa1a1dd57a1e8d4d4`, 2026-07-08). Ported the two previously-deferred
Relay sub-type `Sample` logics (`Controls/Relay.pas`), removing the `NOT_PORTED`
dispatch stubs (and the `record_not_ported_once`/`not_ported_logged` latch).
**`GenericLogic`** (`Relay.pas:1122`): trips one-shot-to-lockout when a monitored
PC element's state variable leaves `[UnderTrip, OverTrip]`; `MonitorVarIndex` is
resolved in `recalc` via `LookupVariable` (`PCElement.pas:201` — case-insensitive
prefix match) against the monitored element's state-variable names captured at
`monitoredobj=` resolution (recalc has no live foreign element), and the value is
read from the live element via `get_all_variables[i-1]` (≡ Pascal `Get_Variable(i)`,
`generator.pas:2639`). Pascal's error 385 (monitored element not a PC element) is
folded into the 386 not-found path (a non-PC element exposes no variable names) —
no corpus deck exercises Generic relays, so it is **unit-tested only**.
**`TD21Logic`** (`Relay.pas:1447`): the differential time-distance relay — a
per-cycle ring buffer of terminal V/I sized on the first dynamics step from
`DynaVars.h`/`Frequency` (`round(1/60/dt+0.5)` samples, FPC banker's; the `1/60`
literal is hard-coded upstream), forming pre-fault-referenced increments `dV`/`dI`
and a half-reach directional pickup (`-(Vloop/Iloop)` in Q1 + `|Uhsd|²/|Uref|² > 1`);
the ring advances + `td21_quiet` decrements only on predictor (`IterationFlag ==
NewTimeStep`) iterations, and `DoPendingAction` sets the post-op `td21_quiet` windows
(`pt+1` on open, `pt/2` on close/reset). New TD21 ring-buffer state lives on the
`Relay` struct (constructor `td21_i = -1`, everything else 0/empty; `MakeLike` does
**not** copy it, per Pascal). Migrated the four TD21 corpus decks
(`Test/{,Reverse}TD21RelayTest.DSS` + the two `Version8/Distrib/Examples/DistanceRelays/`
copies) `skipped_unsupported → solvable_now` (**178→182**, COVERAGE
**53.1%→54.3%**) with `compare_eventlog: true`: all four pass the always-on live
dynamics gate (`mode=dynamic stepsize=0.001 number=1200`) — event log **and** final
dynamic state exact vs the pinned oracle (`corpus_live: 182 matched, 173 with full
property parity`). No `TODO(compat)`
added; the one defensive divergence is a `td21_pt < 1` guard that skips the ring math
when `dt <= 0` (Pascal would `mod 0`-crash; no deck sets `stepsize <= 0`). New relay
unit tests: `lookup_variable_prefix_match`, Generic over/under/in-band + recalc-386,
TD21 ring-alloc + forward-fault trip + reverse no-trip (relay lib tests 39→44).

**WPG.12 audit settlement (2026-07-08).** (1) **Eventlog gating clarified** — the
four migrated TD21 vendored decks do **not** set `eventlog=yes`, and Pascal gates
the relay trip/reclose/lockout log lines on `ShowEventLog` (default FALSE), so
`compare_eventlog` pins only the Fault APPLIED/CLEARED lines, **not** a relay
trip/reclose/lockout sequence. That is not a regression escape: the TD21 trip +
lockout is gated by the FULL-MODEL final dynamic-state (V/I/P) compare — a relay
that fails to trip and lock out collapses the final voltages and fails the
compare — with the fault-clear timing in the log as an indirect proxy. (2)
**Swallowed-error fixed** — the TD21 coarse-time-step guard (error 388,
`Relay.pas:1460`) and the out-of-range monitored-terminal check (error 384,
`Relay.pas:813`) both go through Pascal `DoErrorMsg`, which sets
`SolutionAbort := True` (`DSSGlobals.pas:265`); the port previously only recorded
the message and solved on. Error 388 now returns an abort request up
`Relay::sample` → the dispatch layer lifts it into `Solution.solution_abort` (the
`Sample`-time analogue of CapControl's abort); error 384 sets a new
`DssObjData::deferred_abort` (a `push_error_abort` = `DoErrorMsg` vs `push_error`
= `DoSimpleMsg` distinction) that the executive lifts after `end_edit`. Errors
385/386 use `DoSimpleMsg` (no abort) and stay record-only. New relay unit tests
pin both aborts + the 385/386 non-abort; no vendored deck exercises a coarse
step (all use `stepsize=0.001`), so the four TD21 decks are unaffected.

| Phase | Scope | Status |
|------|-------|--------|
| 0 | Tooling, oracle, faer spike, CI, Phase-0 goldens | ✅ done (committed) |
| 1 | Shared math (`support/`) + full `TDSSParser` port | ✅ done (commit `729eb77`) |
| 2 | Object model, property engine, executive skeleton | ✅ done (commit `22f861d`) |
| 3 | ★ Vertical slice: parse → circuit → Y matrix → solve → voltages | ✅ done (commit `2ac8691`) |
| **4** | **Transformer/Capacitor/Reactor/LineCode + controls (parse-only) + macro + feeder gate** | ✅ done (merged to main, `5f27a25`); `PHASE4_PLAN.md` |
| **5** | **LoadShape/XYcurve/controls behavior, control queue, time modes + feeder gate (controls active)** | ✅ done (merged to main, `10d3550`); `PHASE5_PLAN.md` |
| **6** | **Meters/Monitors/topology/Generator + 8500-node gate + live corpus gate** | ✅ done (merged to main, `b98223a`); `PHASE6_PLAN.md` |
| 7 | Extended elements: DER, protection, line constants, harmonics, dynamics | ✅ **COMPLETE** (WP7.1–WP7.10) — `PHASE7_PLAN.md`; branch `phase-7-extended-elements`, gate-green, **NOT merged to `main`** (explicit-request-only HARD STOP). WP7.1–7.6 (line constants, protection, DER, harmonics), WP7.7 (Dynamics core), WP7.8 (Converter/FACTS), WP7.9 (FaultStudy + AutoAdd/Feeder-deferred), WP7.10 (phase exit). Tracked-open deferrals: GFM grid-forming mode + Generic/TD21 relay `Sample` (both Plot-blocked, 0 corpus payoff). Per-step detail in §1e + `docs/phase-records/phase-7-wp{1..6}.md` |
| **8** | **Reporting: Export/Show/Save/Dump + executive tail + full ReduceAlgs** | ✅ **COMPLETE (2026-07-10)** — `PHASE8_PLAN.md` WP8.1–WP8.8 all executed (WP8.8 exit record in §1; live corpus 226 decks / 67.5%). History: **WP8.1 COMPLETE, gate-green** (dispatch skeleton + GUI no-ops `82b50fe`; output-path machinery + `Export Counts` + the `compare_export` golden harness `929145c`). **WP8.2 COMPLETE, gate-green:** sub-step 1 (`71067f7`) = bus/node solution exports; sub-step 2a (`668bd18`) = the aggregate PD/PC power exports `Powers`/`Losses`/`P_byphase` + the mutable element-walk infra + the MVA/kVA `Parm2` pre-parse; **sub-step 2b** = the symmetrical-component family `SeqVoltages`/`SeqCurrents`/`SeqPowers` + the `ColTol::gate` denominator-gate harness machinery; **sub-step 2c** = the per-terminal/per-conductor element exports `Currents`/`NodeOrder`/`ElemCurrents`/`ElemVoltages`/`ElemPowers`/`Taps` + the `ColSel` name-prefix\|index-parity harness refactor + the `ElemPowers` Vsource order fix; **sub-step 3** = the matrix/summary exports `Yprims`/`Y`/`SeqZ`/`Summary`/`Result` + the `Y` triplet Parm2 flag + the `Summary` append/`DateTime`-mask (`ColSel::Index`+`GateSpec::Mask`) + the `SeqZ`-faultstudy fixture + the PM-build-faithful always-`null` `Result`; **completion gate** = the IEEE8500 `Voltages`/`Summary`/`Counts` goldens (`run_shared_exports`) + the `Export`-unblocked corpus migration (`solvable_now` **88→119**, COVERAGE **26.3%→35.5%**) + the Rust `CorpusGuard` (corpus stays pristine under report-writing decks). The "9 decks hang" tracked-open is **RESOLVED — no hang** (all complete + converge; watchdog artifact; stale tags refreshed, see §1f). Branch `phase-8-reporting`. **WP8.3 COMPLETE (steps 1–5 + both audit follow-ups), gate-green** — the device/meter/reliability/log exports (`Monitors`/`Meters`/DER/`EventLog`/`Faultstudy`/`BusReliability`…/`Sections`/`Profile`) + the `TSystemMeter` core and the full demand-interval (`DI_*`) file machinery + its `Set`/`Set year=` wiring (§2.6); the completion gate migrated `solvable_now` **119→168** (COVERAGE **50.1%**, incl. the silent `Spectrum.CSVFile` no-op fix). Both independent audits found **no correctness bug**; 2 LOW code findings fixed (the `Export Profile` `1732.0` `TODO(compat)` marker; the `Spectrum.read_csv_file` byte-position EOF guard, oracle-confirmed) + 5 oracle-pinned coverage tests (golden_phase8 **59**; lib **731**). **WP8.4 (Show) step 1 COMPLETE, gate-green** — the `do_show_cmd` dispatcher + `report/show/` (`Show Buses`/`Losses`/`Taps` + `Show panel`→#999; unported keywords stay silent no-ops) + the `format.rs` `Pad`/`PadDots`/width formatters + the whitespace+comma `compare_export` tokenizer (golden_phase8 **59→62**). **WP8.4 step 2 COMPLETE, gate-green** — `Show Voltages` code 0 (`WriteSeqVoltages`, the seq V1/V2/V0 + %ratios form, the bare `Show Voltage`/`v` default) + the ptr-13 LL/node/elem option parse (codes 1/2 angle-forms deferred, `TODO(WP8)`); golden_phase8 **62→63**. **WP8.4 step 3 COMPLETE, gate-green** — `Show Currents`/`Powers` code 0 (seq forms, ptr-3/12 option parse); the empirically-zero `MaxDeviceNameLength` reproduced (`TODO(compat)`, probe-proven); the `%I2/I1` ratio gate at the proven `1e-6 A` floor; `gen_phase8.py DSS.AllowEditor=False` (no Notepad spawn); golden_phase8 **63→65**. **WP8.4 step 4 COMPLETE, gate-green** — the element/node forms `Show Voltages` code 1/2 (`WriteBusVoltages`/`WriteElementVoltages`) + `Show Currents` code 1 (`WriteTerminalCurrents`, +residual) + `Show Elements` (`ShowElements`/`WriteElementRecord`, two-file main+`_Disabled`); the `SetMaxBusNameLength` floor-12 backend divergence reproduced (`TODO(compat)`, probe-proven `max(12,longest)`); golden_phase8 **65→69**. Both audits clean (no correctness bug); follow-ups: disabled-element blank line (`WriteElementVoltages`, byte-probed), floor-12 header/data refinement, angle-floor `ColSel::AfterToken` selector, currents-elem gate `1e-6→1e-4`, +2 coverage goldens (class-filter, LL node), + a step-4 test-tree-leak fix (`show_reports_are_silent_noops` datapath); golden_phase8 **69→71**. **Steps 5–6 COMPLETE + audited** — `Powers` elem + `Result`/`EventLog`/`Ratings`/`Variables`/`Mismatch`/`monitor`; the step-4 floor-12 `TODO(compat)` **withdrawn** (`MaxBusNameLength` is an inconsistent per-report backend quirk → comparator drops pure dot-runs); harness `ColSel::AfterToken`/`FromEnd` + `GateSpec::MinCols`; **real bugs found+fixed** (`Show Result` filename `.txt→.csv`; generator `w0` snapshot-`Frequency`); golden_phase8 **71→78**. **Step 7 COMPLETE, gate-green** (the diagnostic/matrix cluster) — `Show Convergence`/`Y`/`controlqueue`/`kvbasemismatch` (`report/show/matrix.rs` + the three diagnostics; dispatcher arms 4/26/27/30; new `format::fpc_sci_w` FPC-`Str(v:width)`; `ControlQueue::queue_rows`); golden_phase8 **78→83** (all exact equality except the convergence `|V|` 7-sig printing floor). **Step 8 COMPLETE, gate-green** — the register tables `Show Meters` (`ShowMeters`→`EMout.txt`) + `Show Generators` (`ShowGenMeters`→`GenMeterOut.txt`), reusing the WP8.3 register fixtures (`%10.0f` integer registers, exact equality); **both audits clean** (no correctness bug) + 3 coverage goldens (multi-meter + the two empty-list banners) + the `$`=7.5 half-boundary documented; golden_phase8 **83→89**. **Step 9 COMPLETE, gate-green** — the overload/unserved pair `Show Overloads` (`ShowOverloads`→`Overload.txt`, arm 16) + `Show Unserved` (`ShowUnserved`→`Unserved.txt`, arm 17), reusing the WP8.3 seq-current/Load-criterion machinery + the `DECK_GROUPS` export decks via the new deck-based `run_deck_show`; 4 goldens exact equality; both audits clean (no correctness bug) + 2 audit-tests coverage goldens (1-phase/emergamps=0/cap-skip + normal-criterion exclusion); golden_phase8 **89→95**. **Step 10 COMPLETE, gate-green** — the FaultStudy report `Show Faults` (arm 6): 3 sections over the precomputed `Zsc`/`Ysc`/`VBus`/`BusCurrent` (WP7.9), reusing `export_fault_study` math + `cdiv_fpc`; feeder golden + `show_faultstudy_unbased` (kVBase<=0 L-N-Volts) + a cold-solve safety-net unit test (oracle UB not reproduced); golden_phase8 **95→97**. **Step 11 COMPLETE, gate-green** — `Show Yprim` (`ShowYPrim`, arm 25) + the `Select` command / `active_ckt_element` surface: the active element's primitive-Y G/jB lower triangles (`%13.10g`) to `<Class>_<name>_Yprim.txt` (no `CircuitName_`, via `write_show_path`); golden `show_yprim` (Select line.650632 + solved IEEE13), exact equality; golden_phase8 **97→98**. **Step 12 COMPLETE, gate-green** — the EnergyMeter zone-tree pair `Show Loops` (`ShowLoops`→`Loops.txt`, arm 21) + `Show Zone <meter>` (`ShowMeterZone`→`ZoneOut_<meter>.txt`, arm 14), walking the WP6.4-built meter `BranchList` via the persisted `sequence_list`+new `sequence_nodes`/`branch_list`/`TreeNode::level` accessors (`report/show/meter_zone.rs`); 4 goldens (radial IEEE13 + a synthesized meshed loop/parallel deck) compared **byte-exact** (`assert_show_bytes_eq` — no backend width quirk) + the `show_zone_error_paths` unit test (#221/#220); **both audits ran** — audit-code Major (disabled-meter `Show Zone` wrote the header vs the oracle's empty file, `BranchList<>NIL` guard) + Minor (header raw-`Param` case) fixed + regression test; audit-tests multi-meter `show_loops` golden + None-`BranchList` unit test; golden_phase8 **98→103**, lib **744**. **Step 13 COMPLETE** — `Show Controlled` (`ShowControlledElements`→`ControlledElements.csv`, arm 33) + the new `CktElement::controlled_element()` accessor (11 control overrides) deriving the PD→controls map from `ckt.controls`; goldens `show_controlled` + `show_controlled_multi` byte-exact (golden_phase8 **103→105**; audit-code Major = the missing **Fuse** override, fixed + pinned). **Step 14 COMPLETE** — `Show LineConstants` (`ShowLineConstants`, arm 24): the LineGeometry Carson R/jX/L/C matrix dump + order-3 seq-component summary, two files (`LineConstants.txt` + `LineConstantsCode.dss`), reusing the WP7.1 `z_matrix`/`yc_matrix`; golden `show_lineconstants` byte-exact on both files (golden_phase8 **105→106**); fixed a `format::g` byte-fidelity bug (FPC `%g` uppercase-`E` exponent). **Steps 15–16 + finalize COMPLETE + audited** — `Show busflow` (`ShowBusPowers`, extracting reusable per-bus/per-element helpers), `Show Isolated`/`Topology` (the new `solution/topology.rs` `GetTopology`/`GetIsolatedSubArea` circuit-wide CktTree builder; audit-code found+fixed two `ShowIsolated` Major bugs — the missing `Enabled` sub-area filter + the missing `ReprocessBusDefs`), the `#24700` unknown-keyword error + the `autoadded`/`QueryLog` headless no-ops, and `Show DeltaV` (the step-4 delta-winding node_ref deferral resolved). **WP8.4 COMPLETE — every real `Show` report ported + audited.** golden_phase8 **125**. **WP8.5 (Save/Dump) IN PROGRESS — Dump steps 1–3a COMPLETE + audited, gate-green** (single-object `Dump <class>.[name\|*] [debug]`: `report/save/dump.rs` generic 3-kind base + all 12 in-scope leaf overrides — Reactor/Transformer/Line/LineCode/LineGeometry/XfmrCode (step 2) + Capacitor/Fault/VSource/UPFC/RegControl/Monitor/EnergyMeter/Spectrum (step 3a) — + `#903`/`#256`; the bare/`solution`/aux whole-circuit forms are step 3b). Fixed several latent byte-fidelity gaps along the way (property display-name case pass across all 12 overrides; `float_to_str` 15-sig + uppercase-`E`; `format::fpc_sci_w`'s width<9 sign-slot floor) + real bugs the coverage goldens exposed (Terminal Bus Ref `-1`; a Phase-4 reactor `stamp_series` asymmetric-YPrim transpose). golden_phase8 **153**; lib **751**. **Controls live gate COMPLETE** (`CONTROL_COVERAGE_PLAN.md` steps 1–5 + midi network + per-element midi wave: asymmetric 14→27 + controls **37** decks, element-state channels; **3 real port bugs found+fixed** — 2× dropped `SystemYChanged`, fleet-`nphases` last-wins). **`GAPS_PLAN.md` authored** + `tests/corpus/gaps/` (oracle-validated decks, the staging live family, all `pending: true` until their WPG ports them). **next = Dump step 3b** (bare `dump`/`dump debug`/`dump solution` + the `commands`/`buslist`/`devicelist`/`alloc` aux forms). Detail in §1f |

### UPGRADE pre-work (branch `upgrade-test-infra`, 2026-07-07)

**`UPGRADE_PLAN.md` authored + its WP-U0 multi-oracle test infra LANDED** (post-acceptance
plan, PLAN_SEQUENCE stage 3: Rung 1 = dss_capi 0.15.x/r4088-line parity, Rung 2 = OpenDSS
11.0.0.1/r4133; scoping inventories in `docs/upgrade/delta_*.md`). Infra: per-case
`oracle` manifest field (`capi015|r3723|r4088|r4133`) routes a live-compare case to its
target engine (`OraclePool`, ping-verified per spec); new `capi015` oracle_server engine
(dss-python 0.16.0b2/Oddie venv, backend 0.15.0b4 pinned); iteration policy `Rust <=
oracle` for target-rev cases only (default cases stay exact); `corpus_live_opendss`
excludes flipped cases; pilot `modes/upgrade_pilot.dss` live-compares against the official
EPRI **r4133** binary in every `cargo test` (Oddie venv + `bin/` are now mandatory gate
prerequisites). Built on a separate branch so parallel porting continues beside it.

Both WP-U0 audits ran (opus-high, no correctness bug): audit-code Minor = default oracle
had no positive engine-identity assertion (a double env misconfig could re-bind it) —
fixed (`Oracle::new` pins `DSS_ORACLE_ENGINE=capi`; `ping_engine(None)` asserts the
`oddie`/`capi015` flags ABSENT); audit-tests Minor ×2 fixed (`solvable_now_oracle_specs_
are_valid` structural guard; sweep report records `target_rev_excluded` + empty-universe
warning). Accepted-by-design, recorded: the U0 pilot is revision-insensitive — it proves
engine identity (ping) + plumbing, not numeric routing; the first revision-sensitive flip
(WP-U1.2) closes that (noted in the plan); the iterations-`<` note is visible only under
`--nocapture` (ritual note added to UPGRADE_PLAN §1.3-1).

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 748, golden_feeders 1,
                            # golden_feeders_controls 4, golden_phase5 1,
                            # golden_phase6 1, golden_phase7 1,
                            # golden_phase7_protection 1, golden_phase8 130,
                            # golden_checkpoints 1, golden_ieee8500 1,
                            # golden_reliability 1, golden_allocation 1,
                            # golden_gendispatcher 1, golden_autoadd_reduce 1,
                            # golden_slice 2, golden_smoke 3, props_roundtrip 1,
                            # corpus_manifest 1, corpus_live 3 (168 solvable_now
                            #   cases live-compared)
                            #   (corpus_live_solvable_cases_match_oracle +
                            #    solvable_now_has_multistep_depth run
                            #    UNCONDITIONALLY — the pinned oracle MUST be
                            #    installed (it fails, not skips, without it);
                            #    only corpus_live_classify is opt-in, via
                            #    DSS_LIVE_CLASSIFY=1 — the growth/classify probe),
                            # dss-parser 62+1, dss-sparse 5
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

### Live corpus oracle gate (`crates/dss-core/tests/corpus_live.rs`) — opt-in
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
  be installed).** For each of the **84** `solvable_now` cases the gate
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
[`docs/phase-records/phase-8.md`](docs/phase-records/phase-8.md). Live frontier:

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

Grep `rg "TODO\(compat\)"` for the full marker list (39 sites). Notable:
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
