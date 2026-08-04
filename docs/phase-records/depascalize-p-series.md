# DE_PASCALIZE P-series records (P1b, P5a-c, P8..P15)

> Moved verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

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

### DE_PASCALIZE P11 — Kron reduction & stamping loops (branch `depas-p11`)

Stratum **[A]** bit-neutral. Two moves:

- **Shared YPrim stamps.** Extracted the three `CalcYPrim` stamping loops every
  2-terminal element re-derived into `CMatrix` methods:
  `stamp_two_terminal_block(n, StampBl, value_fn)` (the four-quadrant `(i-1)*nphases`
  block, `StampBl::Transposed` `(j+n,i)` vs `Direct` `(i+n,j)` bottom-left —
  the latter preserves Reactor's intentional asymmetric SpecType-3/4 stamp),
  `stamp_two_terminal_diag(count, off, value)` (the wye diagonal), and
  `stamp_delta_series(nphases, nconds, value)` (the accumulating delta ring).
  Applied at 9 sites: Line series, Reactor `stamp_series`/parallel-SpecType3/wye/
  delta, Capacitor CMatrix/wye/delta, Fault SpecType-2/1, VSConverter AC block.
  Same cells, same values, same order (`set` quadrants are disjoint; the delta
  `add` order is replicated exactly).
- **Kron elimination on named offsets.** `ckt.rs::do_yprim_calcs` now walks
  terminals via `enumerate()` with `base = term_idx * nconds` instead of the
  running `k += nconds` offset, and the two inner Kron sweeps iterate
  `row_eliminated` (skip-eliminated) instead of `0..yorder` range-indexing — the
  upper-triangle `jj.skip(ii)` + symmetric write keeps the accumulation order
  identical. The `#[allow(clippy::needless_range_loop)]` is gone.

Diff confined to `do_yprim_calcs` + the new helper (P8/P14 also touch `ckt.rs`).
Proof (all UNCHANGED): per-element YPrim oracle unit tests (capacitor/reactor/
line/fault/vs_converter, incl. `asymmetric_sym_components_reactor_unbalanced_solve`
pinning the `Direct` bottom-left), byte-exact `golden_checkpoints` (11) +
`transformer_yprim_bitexact`, full `corpus_gate`. New `cmatrix` unit tests pin the
three helpers (quadrant layout Transposed≠Direct, diag narrow-block, delta
wraparound-accumulate). Left as-is (recorded, not escaped): Generator YPrim
(single-terminal wye+neutral / delta — not the terminal-pair pattern) and
VSConverter's `compute_inj_currents`/`get_currents` `needless_range_loop` allows
(injection/MVMult loops, not stamps → P14 de-indexing territory).

Audit dispositions (2 audits): audit-code found nothing real (bit-neutral, all
pins green). audit-tests raised one Minor·B — the delta-series unit test asserts
`get(i,i) == value + value`, order-blind because `value+value` is FP-exact. Not
fixed, by design: `stamp_delta_series` takes a single scalar, so every diagonal
receives the *identical* `value` twice → there is no in-call accumulation order
to perturb (distinct-magnitude summands are impossible through the API). The
load-bearing property IS pinned — `add` double-accumulates (`2v`) where a `set`
would give `v`; cross-element accumulation order is pinned byte-exact by
`golden_checkpoints` + `corpus_gate`. Audit concurred (optional/not-required).

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

### DE_PASCALIZE P10 — transformer terminal core [A] (branch `depas-p10`)

Stratum **[A]** bit-neutral. The densest index math in the tree: the transformer
+ autotransformer `TermRef`/`Y_Terminal` machinery. Two commits.

- **P1 prep (own commit).** `Winding.connection: i32` → `Connection { Wye=0,
  Delta=1, Series=2 }` (`#[repr(i32)]`, `ordinal`/`from_ordinal`; `winding.rs`).
  Series is autotransformer-only; the enum is shared because `Winding` is shared
  (Transformer/AutoTrans/XfmrCode). `i32` survives only at the DssEnum property
  parse/report + CIM-export boundary. Every match on `0/1/2` literals in the four
  files (+ `cim/power_xfmr.rs`) reads as `Connection::…`.
- **P10 core.** `term_ref: Vec<usize>` (flat 1-based, dead slot 0) →
  `TermRef(Vec<[usize; 2]>)`: one 0-based `[plus, minus]` conductor pair per
  (phase, winding), phase-major (`winding.rs`). `set_term_ref` builds the pairs
  matching on `Connection` (both transformer wye/delta and auto wye/delta/series
  arms). `build_yprim_component` walks the `2·nw` `Y_Terminal` lower triangle as
  `(winding, side)` pairs, yielding the **exact** `(i, j, phase)` `add_sym`
  order of the old flat stamp. The `2·i-1`/`2·i` pairs in
  `calc_y_terminal`/`gic_build_y_terminal`/`get_all_winding_currents` →
  `WdgTerms::of(iwind)` (0-based `[plus, minus]`), derived once per winding via
  `windings.iter().enumerate()`. The `TermRef=` dump walks the pairs and re-emits
  the identical 1-based sequence. Matrix products keep their exact call order —
  indexing reshaped, linear algebra untouched.

Applied identically to `transformer/{windings,yterminal,dump}.rs` and the
UPGRADE-added sibling `auto_trans/{windings,yterminal,dump}.rs`. Proof (all
unchanged): `transformer_yprim_bitexact`, `golden_checkpoints` (per-element
YPrim), the `WdgCurrents`/dump goldens, the two `set_term_ref` + `term_ref_series`
unit pins (rewritten to assert the new pairs = old values − 1). Note: the source
worktree checkout timed out mid-`git worktree add`, leaving 684 files (tests/
+ tools/) unwritten — restored from HEAD before any commit; `git status
tests/corpus` clean.

**Audit settle (opus high).** Two independent audits (code + tests): the change
is bit-neutral, no golden/tolerance churn, no test weakening. One real gap fixed:
the **XSC off-diagonal running-`k` walk** in both `transformer/yterminal.rs` and
`auto_trans/yterminal.rs` — explicitly named in the P10 Fix (plan line 786-788)
but left as the Pascal `let mut k = nw-1; … k += 1` idiom (the initial report's
"None escaped" was inaccurate). Rewritten to an explicit upper-triangle pair
iterator `(0..nw-1).flat_map(…).enumerate()` reading `xsc[nw-1+t]`, yielding the
identical `(i, j, k)` sequence and arithmetic → bit-exact (goldens unchanged,
gate green). The remaining `for i in 1..=nw`/`1..=n2` loops in these files are
P14 scope (plan line 833), not a P10 miss. Two non-defects recorded, no change:
(1) the new `Connection::from_ordinal` setters silently keep the default (Wye) on
an out-of-range `Conns` instead of storing garbage — the string parser rejects
unknown conns before the setter, so the path is dead for real input; via malformed
JSON the new behavior is strictly *safer* (valid YPrim vs the old flat code's
corrupted `term_ref`); (2) the transformer `Connection::Series` self-pair
`[plus, plus]` in `set_term_ref` is unreachable (transformer DssEnum has no
series) and benign, documented in code.

### DE_PASCALIZE P8 — terminal×conductor views over flat buffers [A] (branch `depas-p8p14`)

Stratum **[A]** bit-neutral, base `update@6c99b8f`. The highest-leverage cut: the
flat `yorder` (`nterms*nconds`) buffers on `CktElementData`
(`vterminal`/`iterminal`/`complex_buffer`/`inj_current`/`node_ref`) were indexed
`(t-1)*nconds + c` in every consumer. New accessors on `CktElementData` are now
the **only** site of that offset arithmetic (`ckt.rs`): `term_v(t)`/`term_i(t)`
(0-based conductor slice of terminal `t`), `term_nodes(t)` (its `node_ref`
slice), `terminals_i()` (`chunks_exact(nconds)` iterator), and `term_phases(t)`
→ `Phases<'_>` yielding `(Iterminal, NodeRef)` over the first `min(nphases,3)`
conductors — the shared seq-quantity walk.

Rewrote all in-scope consumers: exports
`seq_currents`/`seq_powers`/`currents`/`voltages_elements`;
`report/show/{currents,powers,bus_powers}` (`get_i0i1i2` now takes a terminal
slice, not `(buf, koff)`); `traits.rs::{get_term_voltages,terminal_power,losses}`;
`transformer/windings.rs::{get_winding_voltages,power_into}`;
`auto_trans/{windings.rs::power_into, yterminal.rs::get_winding_voltages}`;
`solution/controls/dispatch.rs::{control_power,control_current}`;
`cim/power_xfmr.rs` winding node refs; `report/show/delta_v.rs`
(`term_nodes(0)`/`term_nodes(1)`) and `auto_trans/accessors.rs::losses`
(per-terminal `term_nodes(t)`/`term_i(t)` slices, first `nphases` of each — the
`nconds = 2·nphases` layout makes `[np..]` the skipped second-half). Same
arithmetic, same statement order — byte goldens (exports/show dumps), checkpoint
captures and the full corpus gate all unchanged.

**Settle (audit dispositions):** the audit flagged two remaining flat-offset
consumers not in the first cut — `delta_v.rs` (`node_ref[i-1]`/`[i-1+ncond]`) and
`auto_trans::losses` (`k += np` terminal skip). Both mapped cleanly to the
accessors and were folded in above (bit-neutral, gate re-run green), so the
"offset lives only in accessors" metric now holds for the whole in-scope set. The
audit's CIM `[Question]` (the `term_nodes(i-1)` slice narrows the old
`node_ref.get().unwrap_or(0)`) is a **deliberate non-fix**: `set_node_ref`
(`ckt.rs`) resizes `node_ref` to `yorder` on first population, so it is always
empty-or-full — the `is_empty()` guard covers empty and the full-length case
makes the slice safe; the panic the auditor described is unreachable in every
solved/pre-solve state, so behavior is preserved. The tests-audit's corpus-gate
flakiness note (r4133-channel `windgen_dyn` `WindGen.Ps`, `ncim_*`,
`mmf_singlecol`) is **P8-independent** — those readouts flow through
`exec/view.rs::snapshot_elements`, which P8 escaped and did not modify; the gate
ran **green** on this settle run. Left for a separate flakiness investigation.

**Deliberately NOT touched (documented, not a miss):** the `seq_currents`
Iresidual `TODO(compat)` loop (reproduces the terminal-1 upstream bug — a flat
terminal-0 read, no `(t-1)*nconds` form, kept verbatim); the `dispatch`
specific-phase quirk `cBuffer[FMonPhase]` (flat/absolute by upstream design);
monitor `mode 12`'s `np*k` stride (an **nphases** stride, not nconds — using a
`nconds`-strided view would change the arithmetic and break the golden);
terminal-0 flat reads with no offset (`fault.rs:208`, `control_loop.rs`,
monitor/sensor `node_ref[i]`); and element-owned scratch buffers with the same
layout but which are **not** `CktElementData` targets
(`meter_element.calculated_current`, relay `cbuffer` — its offset is already
centralized in `mon_offset`).

**ESCAPED (leave-green, recorded):** `exec/view.rs` COM-style interleaved re/im
`Vec<f64>` → `Vec<Complex64>` conversion (`ElementSnapshot.powers`/`.currents`).
Its blast radius is the **gate-critical** oracle comparator
(`tests/harness::compare_element`/`compare_interleaved` and
`corpus_gate/runner.rs`, which both compare against dss-python/EPRI **interleaved
f64** arrays) plus ~40 in-crate test sites reading `.powers[2*k]`/`.step_by(2)`.
This is a re/im-packing cleanup **orthogonal** to the `(t-1)*nconds` metric —
`view.rs::snapshot_elements` has zero terminal-offset forms (it is a flat
`0..yorder` loop). Deferred to a focused follow-up (one boundary interleave
adapter + comparator + test-site sweep) so the gate stays green here.

### DE_PASCALIZE P14 — 0-basing + sentinel sweep [A] (branch `depas-p8p14`)

Stratum **[A]** bit-neutral, on top of P8 (`94d27ae`). 1-based loops and magic
sentinels retreat to the true user boundary; property parsing + report text keep
`wdg`/`terminal`/`phase` as the user's 1-based language.

**0-basing (the success metric — `for … in 1..=` gone from the P10-scoped files).**
The `for i in 1..=np { for j in 1..=nw { …[i-1]…[j-1] } }` remnants P10 left in
`transformer/windings.rs`, `auto_trans/windings.rs` (`set_term_ref`) and both
`yterminal.rs` (`calc`'s `y1`/`yterm` column builders + `get_all_winding_currents`
phase loops) went 0-based; the delta arm keeps a 1-based `rotate_phases(i+1)` call
because phase rotation *is* 1-based phase math. `auto_trans/accessors.rs::set_node_ref`
series-alias loop 0-based. `ckt.rs::set_node_ref` hoists the 1→0 conversion once
(`let t = iterm - 1`). All index arithmetic and matrix column-fill order identical
→ byte-neutral (transformer/auto_trans YPrim + winding-current goldens + corpus gate
unchanged). The two `av[kp]`-indexed loops became `av.iter_mut().zip(windings)` to
clear the `needless_range_loop` I introduced (same values, same order).

**Sentinels → `Option` (four of five).**
- `CktElementData.iterminal_solution_count: i32 = -1` → `Option<u32>` (None = never
  computed = the lazy-cache/thread-readiness state made explicit). Two helpers
  localize the `i32→u32` cast: `iterminal_solved_for(count)` / `mark_iterminal_solved(count)`.
  ~40 sites across the PC-element accessors/solve/dynamics/user_model. `-1 != count`
  ≡ `None != Some(count)` → bit-neutral.
- `CktElementData.handle: usize = 0` → `Option<u32>` (None = not in circuit). Only
  written (`circuit.rs` `Some(len as u32)`), never read → representational only.
- `CktElementData.from_terminal`/`to_terminal: usize` → `Option<usize>` **0-based**
  (`from` default `Some(0)` = Pascal `FromTerminal:=1`; `to` default `None` = Pascal
  `0`/unset). The reliability sweeps' `if from_t == 2 {1} else {2}` (1-based) became
  `if from_t == 1 {0} else {1}` (0-based) with `expect` at the guaranteed-set reads;
  set sites in `zones/build.rs` store `Some(k-1)`.
- `Terminal::bus_ref: usize` (`usize::MAX` = unset) → `Option<usize>` (~60 real
  sites). New `Terminal::bus_idx()` resolves the assume-wired index sites (panics
  identically to the old `buses[MAX]` OOB); genuinely-optional sites use
  `.and_then(|b| buses.get(b))` / `== Some(x)` / `.unwrap_or(usize::MAX)` where a
  local `usize` sentinel is still threaded (reduce `LineSnap.bus_refs`, zone-build
  `test_bus`, topology walk `bus`). `dump.rs` `-1`-render and `ieee1547`/`dispatch`
  graceful-`get` paths preserved. `NodeBus.bus_ref` (a different field) untouched.

**ESCAPED (leave-green, recorded): `ckt_tree::NO_BUS` → `Option<usize>`.** The
`TreeNode.from_bus` field is one strand of a `usize`/`NO_BUS`(=`usize::MAX`) sentinel
web that also spans `ZoneEndsList.ends: Vec<(usize, usize)>`, the zone-build walk
locals (`test_bus`, `node_from_bus`, `add_new_child(bus_ref: usize)`), the topology
walk `bus`, and the coordinate-interpolation locals (`first_coord_ref`/
`second_coord_ref` in `interpolate.rs`), where `NO_BUS` is *load-bearing* in a UB
guard (`coord_defined` treats it as "no coordinate", never reproducing the Pascal
`buses[0]` OOB). Converting only the field forces `Option↔NO_BUS` bridging at every
read — a net **increase** in sentinel surface, not the plan's elimination — and a
clean conversion means the whole walk web at once (beyond P14's "tree nodes" scope
and higher-risk than reward). Deferred to a focused zone-walk-wide follow-up.

**Deviation (documented):** the metric line "`for … in 1..=` in `elements` →
boundary accessors only" is met **for the P10-scoped files**; the broad remainder
(dump/save report text, `1..=nphases`/`1..=nw`/channel-count loops, Pascal 1-based
state arrays like vccs filters and storage/pv var tables) are 1-based *by design*
(user's language / documented STAYS) and out of P14's explicitly-named scope.

Proof (all UNCHANGED): full golden suite (transformer/auto_trans YPrim +
winding-current + checkpoint byte goldens, reliability/branch-reliability exports,
show voltages/currents/powers/elements, dumps) + the unconditional corpus gate.
`STAYS` untouched: `NodeRef==0` ground, `rneut<0` open neutral, parser `-1` node
sentinel, `ckt_tree` node `from_terminal` (separate 1-based field).

**Audit settle (empirical, no code change).** Two Minor audit findings, both
dispositioned won't-fix (documented deviations above):
- *Success-metric grep not fully green (106 `for … in 1..=` in `elements`).* Verified
  by hand: none are internal `[i-1]` storage remnants outside P14's named files
  (`windings.rs`/`yterminal.rs`/`set_node_ref`, all clean). The remainder is
  report/dump text where `i` is the printed 1-based `Wdg=`/`terminal` (e.g.
  `transformer/dump.rs:27` `Wdg={i}` with `windings[i-1]`), 1-based user-API variable/
  conductor numbering (`pvsystem`/`storage/dynamics` `get_*_variable(i)`,
  `conductor_closed(term,i)`), and Pascal 1-based state/filter arrays (vccs filters,
  relay/recloser `present_state[i]`, capacitor step states) — all `STAYS` by design,
  out of P14's named scope. The broad grep is aspirational; the plan *body* (§P14 first
  bullet) scopes the concrete `[i-1]` work to the three named files, which is done.
- *`ckt_tree::NO_BUS`→`Option` deferred.* Entanglement confirmed real: `NO_BUS` threads
  `exec/reduce.rs:520`, `report.rs`, `interpolate.rs` (load-bearing UB guard vs Pascal
  `buses[0]` OOB), `take_sample.rs:386`, `zones/build.rs`, `topology.rs`. Converting only
  `TreeNode.from_bus` forces `Option↔NO_BUS` bridging at every consumer = net sentinel
  *increase*; a clean fix is the whole zone-walk web at once (out of "tree nodes" scope).
  Deferred to a focused zone-walk-wide follow-up, as recorded above.

Corpus cleaned of run artifacts (11 untracked `Export`/`Mon_*`/`EventLog` files under
`Test/AutoTrans` + `StorageControllerTechNote/Schedule`) — tests/corpus pristine.

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

## 1jb. DE_PASCALIZE P5b — source spans ("beautiful errors") [A] (branch `depas-p5bc`)

`DE_PASCALIZE_PLAN.md` §P5b executed. Diagnostics now underline the offending
token against the command-line source. Base `update@6c99b8f`. Numeric goldens
byte-identical; no tolerance/golden changes (text-only P5 exception unused here).

- **`dss-parser` token spans.** `get_token_at` (scanner.rs) records the byte
  range of every token it produces (`tok_start/tok_end`, zero-width default for
  the end-of-line empty token). `next_param` copies those into `value_span` (the
  current value token) and `param_span` (the parameter *name* in a `name=value`
  pair; equals the value span for a bare token). New public accessors
  `Parser::token_span()` / `param_name_span() -> Range<usize>` next to `token()`.
  Spans point at the ORIGINAL source text — `@variable` substitution changes the
  token but not the span (correct for jump-to-source).
- **`ParserError` gains `span: Option<Range<usize>>`** + `with_span`/`span()`;
  the conversion/inline-math raisers (`make_integer`/`make_double_ex`) populate it
  from `value_span`. `From<ParserError> for DssDiagnostic` (dss-core `diag.rs`)
  carries it into `DssDiagnostic.span` (no `src` — see below). dss-parser stays
  miette-free (`std::ops::Range`, not `SourceSpan`).
- **The executive is the source authority.** `exec/command.rs::attach_source`
  stamps the executing command line (`parser.cmd_string()`) + a token span onto
  every diagnostic pushed during a unit of work whose `src` is still `None` — so a
  span the property parser recorded against its scratch `(value)` buffer is
  overwritten with the correct main-source offset. Wired at the three highest-value
  sites: unknown command (token span of the name), unknown parameter (name-token
  span), and property edit (value-token span, covering both the bubbled conversion
  `Err` and any deep `DoSimpleMsg`-and-continue range/sign message).
- **Scope rule v1 (one command line = one source).** New `Dss.cmd_origin` field:
  `"<command>"` interactively, set to `"<file>:<line-no>"` per line by
  `do_redirect` (saved/restored around the loop so nested Redirect/Compile report
  their own origin). No whole-file offset maps. Long-tail solve-time sites and the
  post-edit deferred-channel drain stay span-less (final, per plan).
- **Render is test-only.** miette `fancy-no-backtrace` is a dss-core **dev-dep**
  (shipped lib stays protocol-only); `tests/diag_render.rs` snapshots the graphical
  render (`GraphicalTheme::unicode_nocolor`, width 80 → ANSI-free, deterministic)
  for the three DoD cases: bad property value (underlines `abc`), unknown command
  (underlines the name), mid-script Redirect (origin `broken.dss:3`). Note: a
  non-numeric integer value is `(...)`-wrapped and RPN-parsed, so its message is
  "Invalid inline math entry" (pre-existing text, preserved) — the span still
  underlines the real value token. 6 parser unit tests pin the span byte ranges.

## 1jc. DE_PASCALIZE P5c — CLI diagnostic presentation [A] (branch `depas-p5bc`)

`DE_PASCALIZE_PLAN.md` §P5c executed. Presentation is a **binary-only** concern.

- **`dss-cli`** gains `miette = { features = ["fancy-no-backtrace"] }` (the only
  product crate that ever enables the graphical handler; the libraries stay
  protocol-only). New `--diag=pretty|plain` switch (default **plain**): pretty
  installs the miette hook (`MietteHandlerOpts`) once and prints each engine
  diagnostic via `Report::new(e.clone())` (source underline + `<file>:<line>`
  origin); plain keeps the byte-identical `error: <message>` line so drivers,
  scripts, `?`/GlobalResult, and the goldens see no change unless they opt in.
- Verified end-to-end on a broken deck: plain output unchanged; pretty renders
  the graphical diagnostic underlining the offending token. No library code
  touched — the corpus gate and numeric goldens are unaffected.

**Settle (audit dispositions, no code change).** Both audits (code + tests)
returned zero real defects; the three flagged items are deliberate non-fixes,
each re-verified empirically here:
- *fancy dev-dep in dss-core* — the shipped library installs no render hook
  (`src/diag.rs` mentions "fancy" only in a doc comment; `GraphicalReportHandler`
  lives solely in `tests/diag_render.rs` and `dss-cli/main.rs`). The dev-dep is
  required by the plan's own P5b DoD snapshot test; release builds compile no
  dev-deps. The plan's real constraint (presentation in the binary) holds; the
  "only in dss-cli" wording is about the *product* handler. Kept.
- *"Invalid inline math entry" property-value text* — proven pre-existing:
  string is in base `value.rs:46` at `6c99b8f`; P5bc's diff there is purely
  additive `.map_err(|e| e.with_span(...))`, no text line removed. P5b is spans,
  not text; the span correctly underlines the value token. Kept.
- *spans stamped at the executive `attach_source`, not `setters.rs`* — strictly
  more correct than the plan sketch: setters parse values through a scratch
  `(value)` buffer whose offsets don't map to the command line, so the
  library-authoritative overwrite of the scratch span is the right behavior.
  Kept.

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
