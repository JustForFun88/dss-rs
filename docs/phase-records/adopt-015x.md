# 0.15.x adoption sweep — fix round and settle

> Moved verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

## 0.15.x adoption sweep — fix round (branch `fix-015x-sweep`, base `57fbcde`)

The sweep-015x adversarial review (wf_9daaa9d9-13b) re-verified all 41
dss_capi-0.15.x adoptions under the new CLAUDE.md rule (0.15.x is not an
authority) and flagged 5 BUG + 1 SUSPECT. This fix round settles all eight brief
items; every code fix re-checked the decisive r4133 citation in
`.inputs/electricdss-code-r4133-trunk` and, where live-observable, ran an OWN
epri-worker (r4133) / pinned-oracle probe before/after. All gate-neutral today
(zero corpus/golden movement); each is a deterministic deck-reachable r4133
divergence the port now matches.

- **#1 l4e2 seasonal GetRatings (`62b0f14`).** `traits.rs::get_ratings` gates the
  `AmpRatings` override on `num_amp_ratings() > 1` (r4133 PDElement.pas:351,
  re-read: `(RatingIdx <= NumAmpRatings) and (NumAmpRatings > 1)`). 0.15.x
  `55400a29` dropped the guard → a single-season element silently hid its user
  `normamps`/`emergamps` under `SeasonRating` (a bug vs r4133 + physics). Kept the
  memory-safe `0<=idx<num` bound (r4133's `<=` is an off-end read). Flipped the
  single-season line pin; DIVERGENCES L4/E2 updated.
- **#2 c5 RegControl rev-only fallback (`2070d50`).** `reg_control/accessors.rs`
  `end_edit`: sign-preserving `Fwd:=Rev; Rev:=-Rev` (r4133 RegControl.pas:499-507,
  re-read — NO abs). 0.15.x `8a898cba`'s `abs` inverts the band for a negative
  `revThreshold`; own probes: r4133 AND pinned 0.14.5 both abort #485, port now
  #485 (was: converged) — the D14 signature. Unit-pinned; DIVERGENCES C5 abs
  attribution + stale idle-OR paragraph refreshed to bounded-AND.
- **#3 a3a5 PCE force-hook fidelity (`5f73d36`).** (a) WindGen `get_currents`
  gains the `ForceInjCurr` guard (r4133 WindGen.pas:2148, re-read) — the 6th of
  six flag-checking classes; pin `windgen_force_inj_freezes_iterminal` reads the
  corpus snapshot/GetCurrents path (own r4133 probe: frozen
  `[-89.23,-11.20,34.91,...]`; toggle-verified fails ~48 A off without the guard —
  the previous `get ITerminal` unit test was non-discriminating). (b) Moved the
  force check INSIDE each class's `inj_currents` (r4133 Load.pas:1922 et al.: skip
  only `calc_inj_current_array`, keep the unconditional set-nominal preamble +
  inherited add); dropped the central force-branch in `power_flow.rs` so the force
  is inert for VCCS/UPFC/VSConverter (0 `ForceInjCurr` hits in r4133). (c)
  DIVERGENCES A3/A5 corrected: the hooks EXIST in EPRI r4088+/r4133, gated
  engines=r4133.
- **#4 allownoneitem `none` conductor lists (`919b76e`).** `load_spacing_and_wires`
  compacts NIL slots (r4133 LineGeometry.pas:1190-1262, re-read): recount
  actualNConds/actualNPhases, skip-NIL copy with original spacing coords
  (byte-identical for lists with no NIL). `make_z_from_spacing` gains the #181021
  phase-mismatch guard (r4133 Line.pas:2213-2219). A Line `wires=(w none)` now
  compacts + solves (own r4133 probe I1=`(21.802597,-0.001427)`; port matches to
  a faer-vs-KLU floor). Dropped ALLOW_NONE_ITEM from the three LineGeometry lists
  (r4133:346-396 reject #10103; own probe: r4133 #10103, 0.14.5 #40303); kept it
  on Line-level + Conductors. Pin rewritten (geometry-reject + line-accept-solve).
- **#5 conductors-1to1 case-insensitive match (`04b3f4b`).** `parse_conductor_proxy`
  resolves a class-prefixed item by `eq_ignore_ascii_case` (r4133
  `LowerCase(CondClass)` dispatch, re-read LineGeometry.pas:485-503), dropping the
  reproduced capi015 `GetDSSClass` case-bug `TODO(compat)` — a capi015-ONLY
  breakage (r4133 has no `TProxyClass` and parses `conductors=` natively + solves).
  Own r4133 probe: `Conductors=[WireData.w wiredata.w]` converges,
  I1=`(21.801759,0.027069)`; port matches to a floor. Flipped the pin to
  resolve+solve; bare-name/#402/all-none-geometry stay rejected per-channel (own
  probes: r4133 bare #181023, all-none-geometry #303 AV — UB not reproduced).
  Refreshed the stale "upstream-broken" comments; DIVERGENCES retitled.
- **#6 b1 capacitor SUSPECT — doc-only (`0bb7d0b`).** Kept the code (physics +
  VSConverter precedent). Corrected DIVERGENCES §B1: the `×1.000001` on SpecType-3
  is capi015-ONLY — re-read r4133 Capacitor.pas (`×1.000001` in the `1,2`
  Line-Line arm only; `3:` CMatrix arm bare `Invert; add ZL; Invert`); own r4133
  probe: cmatrix cap + R/XL → Q≈1.4e-15 (garbage) == 0.14.5, vs -76.2 kvar without
  R/XL. Added the per-channel pre-registered ledger policy for a future
  SpecType-3+ZL witness.
- **#7 §4 doc follow-ups (`0bb7d0b`).** D12 (3 SwtControl decks are engines=r4133,
  WP-U2.4 D6); TCC_Curve(b) superseded by WP-U2.1 (silent-clear r4133,
  `fuse_curve_none_clears_silently_like_r4133`); D13 mmf deck engines=r4133; D4
  r4133 cite :2184 (re-read); c5r3723 forward-note r4133 LoadShape Mode@22 to
  UPGRADE Rung-2 (re-read LoadShape.pas:246-247); D2 MonBus "0 corpus uses" stale
  (`invcontrol_monbus.dss` exists, valid deck).
- **#8 NCIM defer correction — re-gate BLOCKED, kept deferred (this commit).**
  The four NCIM defer_ledger texts + `ledger.json` `ncim-oppoint` cause claimed a
  "different PV/Q operating point". **DISPROVEN by own r4133 probes:** converged
  node voltages are DIGIT-IDENTICAL r4133-vs-port to a faer-vs-KLU floor
  (`ncim_pq` all 9 nodes ~1e-12, iters 3==3; `ncim_pv_pq` all nodes ~1e-12, iters
  port 8 / r4133 4). The re-gate was ATTEMPTED but is **genuinely blocked**: the
  reported `Vsource.source` swing current diverges by a phase rotation (port
  `~[c2,c3,-c1]` matching its capi015 NCIM oracle vs r4133 `[c1,c2,c3]`) — a real
  reported-current divergence that is NOT a different op-point and NOT a tolerance/
  iteration issue. Because a divergence that may be a port bug must be
  proven-not-a-bug before ledgering (R3), it is left UNDER INVESTIGATION (a
  suspected port `NCIM_CalcInjCurrAtBus` phase-indexing quirk) — the honest
  outcome, not a papered-over ledger row. Corrected the `ncim-oppoint` cause + the
  4 defer_ledger texts (op-point NOT different; the residuals are the iteration
  cadence [dss_capi NCIMSolutionHelper refactor] + the swing-current divergence).
  The historical STATUS WP-U1.7/NCIM "different op-point / suspected port bug"
  wording is likewise superseded by this record. NEVER a tolerance loosening.
  **Follow-up:** investigate the NCIM swing-source current phase rotation (port vs
  r4133); if a port bug, fix + re-gate; if a proven capi015-vs-r4133 reporting
  difference, ledger it per-channel and re-gate the 4 cases.

Gate: full three-command gate green at defaults before every commit
(`cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D
warnings`; `cargo test --workspace` — dss-core lib 1251 tests, corpus
`corpus_gate_all_cases_match_engines` ok on all 514 cases, ~150-300s wall). Corpus
kept pristine (path-limited cleanup only). No tolerance loosened; no new ledger
row (item 8 blocker left open, not ledgered).

### Settle round (two independent read-only audits, code + tests)

Every finding reproduced empirically before disposition; no tolerance loosened,
corpus pristine, `#8` NCIM blocker re-derived from the exact golden-generating
oracle build.

- **NCIM swing-current "suspected port bug" → PROVEN a faithful capi015
  reproduction, NOT a port bug (audit-code F3, audit-tests F1, medium).** Read the
  exact oracle build the NCIM goldens/pins were generated from — dss_capi
  **0.15.0b4 (e936d210, SVN 4103)**, not the standard pinned 0.14.5 (which has no
  NCIM at all). Its `NCIM_CalcInjCurrAtBus` fills `ce.GetCurrents(ElmCurrents)` at
  index 0 then reads `ElmCurrents[j]` (`j:=1..NPhases`) — a one-conductor shift.
  The port's `+1` in `exec/view.rs::ncim_swing_source_currents` reproduces this
  0.15.0b4 shift **exactly** *(both since superseded: UPGRADE Rung 1 dropped the
  `+1` for r4133's unshifted read, and R4133_PROPS §RP3.13, 2026-09-03, deleted the
  `exec/view.rs` override altogether — the read now lives in
  `solution::solution::ncim::ncim_stamp_swing_source_currents` and is echoed by
  `TVsourceObj.GetCurrents`' ported NCIM arm)* (unit test `ncim_vsource_reported_currents_match_oracle`
  green). r4133 VSource.pas:1123 FIXED the shift (`GetCurrents(@(ElmCurrents[1]))`,
  an offset write) — so the port matches capi015 and diverges from r4133, a
  determinate, defined capi015-vs-r4133 upstream divergence (known-bug policy,
  already a documented `TODO(compat)`). Disposition: **corrected** the 4 NCIM
  defer_ledger texts + `ledger.json ncim-oppoint` cause + the `view.rs` comment to
  state the proven cause (dropped the false "suspected port bug / phase-indexing
  quirk / UNDER INVESTIGATION" framing); re-gate **kept deferred** (a genuine,
  now-proven blocker — the swing-current channel differs by the upstream fix; a
  live r4133 re-gate needs a per-channel ledger row scoping `Vsource.source`
  reported currents, never a tolerance loosening — recorded as the re-gate recipe).
  Text-only edits: population-lock safe (defer_ledger boolean-hashed
  `population_lock.rs:150`, `ledger_tags` digests only `entries`) — lock green.
- **line_fetch geometry-reject assertions loosened to bare smoke checks
  (audit-tests F3, low) → FIXED.** Restored discriminating message assertions in
  `conductor_none_geometry_rejects_but_line_accepts_and_solves`: geometry-level
  `none` now asserts the port's `WireData object "none" not found.` (mirrors 0.14.5
  #40303 / r4133 #10103) and the `nope` control asserts `object "nope" not found`
  — a future unrelated parse failure no longer greens the pin.
- **Force sub-fix (c) — VCCS/UPFC/VSConverter force-inertness untested
  (audit-tests F2, low) → FIXED.** Added `force_inj_current_is_inert_for_vsconverter`
  (`force_hooks.rs`): forcing a VSConverter's `InjCurrent` with a ≈5 kA vector must
  not move the solve (0 `ForceInjCurr` in r4133/0.15.0b4; the central force-branch
  was removed). Discriminating — honoring the force would shift the stiff `src`
  (|Z|≈0.051 Ω) by ≈255 V; the inert re-solve drifts only by the fixpoint floor
  (≈2e-5 V), gated at 1e-2 V.
- **Force sub-fix (b) — time-series set_nominal preamble untested (audit-tests F2,
  low) → deliberate documented non-fix.** The unconditional `set_nominal_*` preamble
  is structurally preserved: the restructure moved the force check INSIDE each
  flag-checking class's `inj_currents` (skipping only `calc_inj_current_array`), so
  the preamble always runs before the skip — verified by source (audit + re-read of
  `load/accessors.rs`). A black-box time-series observable that isolates
  preamble-runs-vs-skipped from the frozen injection is not cleanly separable (the
  channel is the nominal `Yeq`, swamped by the forced injection); the per-class
  code shape is the guarantee. Tracked follow-up if a Yeq-freeze probe is later
  built.
- **>2-conductor `none` compaction untested (audit-code F2, low) → FIXED
  (coverage).** Added `line_none_conductor_compaction_over_two_conductors_matches_
  explicit`: the audit's exact flagged case (4-conductor spacing, `wires=(w w w
  none)`) is compacted (skip-NIL copy, actualNConds 4→3, original-position coords)
  and its solve is bit-identical (<1e-6 V) to the explicit 3-conductor spacing at
  the same phase coordinates — whose path is oracle-validated by
  `line_spacing_specified_resolves_and_solves`. Confirms the larger-compaction path
  the port's FReduce-based sizing handles without the literal Pascal `NPhases :=
  pGeo.Nconds` reestablishment.
- **Two `TODO(compat)` deleted in `parse.rs` outside Stage F (audit-code F1, low)
  → deliberate non-fix, convention exception intended.** Brief item 5 authorized it
  and it is r4133-justified: the removed sites reproduced the capi015-ONLY
  `TProxyClass.GetDSSClass` case-bug that r4133 does not have (r4133 parses
  `conductors=` natively via `LowerCase` dispatch and solves). Removing a
  `TODO(compat)` whose behavior is being **corrected toward r4133** (not a precision
  improvement, no golden — HIDE_015X) is the right move; the Stage-F rule guards
  golden-pinned precision compat, not a capi015-only breakage being fixed.
