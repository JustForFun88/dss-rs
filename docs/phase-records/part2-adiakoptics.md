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

