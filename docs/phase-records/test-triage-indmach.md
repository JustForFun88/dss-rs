# Test-triage T4 — InductionMachine decks under r4133 (verdict: INTENTIONAL breaking change)

> Branch `wt-t4`, 2026-07-17. Question (UPGRADE ledger §1.4 framing): the vendored
> `Version8/Distrib/Examples/InductionMachine/{Master.DSS,Run.dss}` converge on
> OpenDSS ≤ r4088 and on dss_capi 0.14.5 but NOT on r4133 — is the r4133 fuse
> overhaul an upstream **bug** (catalog + diverge) or an **intentional redesign**
> (keep parity + add equivalent coverage)? Verdict: **intentional** — parity kept,
> twins gated. Full evidence chain below.

## 1. The upstream change (source, r4088 → r4133)

`fuse.pas` moved `PDElements/` → `Controls/` (class was already `TControlClass`);
props 10 → 12. Diff facts (r4133 `Version8/Source/Controls/fuse.pas`):

- **`CurveMultiplier` is the new TCC divisor** — `TripTime :=
  FuseCurve.GetTCCTime(Cmag/CurveMultiplier)` (r4133 ~line 633; r4088 divided by
  `RatedCurrent`, `PDElements/fuse.pas:620`). Default 1.0 (prop 11,
  `InitPropertyValues` `'1.0'`).
- **`RatedCurrent` repurposed to informational nameplate** — default 1.0 → 0.0;
  help rewritten to "Fuse continuous rated current in Amps. Defaults to 0. **Not
  used internally for either power flow or reporting**" (prop 6). `Edit` case 6
  (r4133 fuse.pas:275) writes **only** `RatedCurrent` — **no fallback** maps it
  into `CurveMultiplier`.
- **Default `FuseCurve` `Tlink` → `none`** (`GetTccCurve('none')` returns NIL
  silently; a curveless fuse never blows). New informational `InterruptingRating`.

## 2. Intent evidence (why this is a redesign, not a defect)

1. **Release notes announce it** (`Version8/Distrib/x64/readme.txt`, "Version
   11.0.0.1 - Charlottesville", §Protection Elements Enhancements): "Added
   RatedCurrent property to SwtControls, **Fuses**, Reclosers and Relays"
   (upstream regards r4133 `RatedCurrent` as a *new* nameplate property), "Added
   InterruptingRating property…", "**Standardized TCC curve behavior - no curves
   assigned by default** ('none' is used as keyword…)".
2. **Property help rewritten deliberately** — prop 5 now reads "Multiplying the
   current values in the curve by the **'CurveMultiplier'** value gives the
   actual current" (the sentence that used to name `RatedCurrent`).
3. **COM API coordinated** — `OpenDSSengine.ridl` adds `Fuses.CurveMultiplier`
   ("Multiplier for the phase TCC curve. Defaults to 1.0.", id 0xDD) and
   documents `Fuses.RatedCurrent` as "Not used internally for either power flow
   or reporting" (id 0xD3).
4. **Cross-class consistency** — Recloser divides by `Phase/GroundCurveMultiplier`
   (r4133 `Recloser.pas:1103/1154/1256`) with `RatedCurrent` informational there
   too (`:574` default 0.0); Relay/SwtControl follow the same nameplate pattern.
5. **Upstream kept compat where they intended compat** — Recloser's legacy scaling
   property `PhaseTrip` (prop 37) IS retained as a deprecated alias writing
   `PhFastPickup`+`PhSlowPickup` (`Recloser.pas:430-433`), and the release notes'
   backwards-compatibility clause is scoped to the Recloser/OC-Relay renames. For
   Fuse they *chose* to repurpose the name (`RatedCurrent` must mean nameplate
   amps uniformly across all four protection classes) — a knowing breaking
   change with a migration burden, not an accident in the trip logic.
6. **The collateral**: EPRI shipped their own r4133 trunk with this very example
   un-migrated (`Version8/Distrib/Examples/InductionMachine/Fuse.dss` and
   `Test/IndMachTest.DSS:124-125` still say `RatedCurrent=65`, no
   `CurveMultiplier`) — their examples break on their own binary. That is
   example-maintenance neglect and does not make the engine change a defect.

## 3. Physics / protection-engineering analysis

Topology: 115 kV source → `Transformer.sub` (20 MVA, Δ-Y) → 12.47 kV feeder
(L1→L2→reg→L3→{L4→B4, L6→B6}) → `Transformer.Tg` (1.5 MVA, 12.47/0.48 kV Y-Y)
→ `IndMach012.Motor1` (1200 kW / 1500 kVA, delta, slip 0.02).
`Fuse.f1` = Klink on Line.L6 (feeds only a 0.1 kW load); `Fuse.f2` = Tlink on
Tg's 12.47 kV terminal; both `RatedCurrent=65`.

Measured controls-off snapshot currents (identical to 4+ digits on 0.14.5,
r4088, r4133 — the base power flow is engine-invariant):

| element | |I| per phase (A) | old multiple I/65 | r4133 multiple I/1.0 |
|---|---|---|---|
| Line.L6 t1 (f1) | 0.094 / 0.105 / 0.095 | 0.0016× | 0.105× |
| Xfmr.Tg t1 (f2) | 52.24 / 53.54 / 53.20 | **0.82×** | **53.5×** |

Curve thresholds (deck `TCC_Curve.dss`): Klink C=(2…30), Tlink C=(2…50) — both
start melting at **2.0 multiples**; `GetTCCTime` returns −1 below the first
point and clamps to the last T beyond the end (`TCC_Curve.pas:335-384`).

- **Old semantics** (r4088/0.14.5, divisor = RatedCurrent = 65): f2 runs at
  0.82–0.86× of the 65 A curve base — below minimum melt, holds indefinitely.
  Sound engineering: Tg full-load current = 1500 kVA/(√3·12.47 kV) = **69.4 A**;
  a 65 T-link at ~0.94× transformer full load carrying ~54 A (0.83× link
  rating) is textbook transformer-primary fusing.
- **r4133 default semantics** (divisor = CurveMultiplier = 1.0): the same 53.5 A
  reads as **53.5 multiples** > Tlink's last point (50×) → clamp T = 0.02 s →
  blows all three phases in the Sec=0 control pass. Physically this models a
  **1 A-class link** — a 65 A-class link absolutely does not melt at 54 A. The
  legacy deck under r4133 defaults describes different hardware; the deck is
  *coherent again* the moment the rating is migrated into the multiplier
  (`CurveMultiplier=65`). So r4133 is a coherent re-parameterization ("curve
  values × CurveMultiplier = actual amps", letting curve libraries be authored
  in absolute amps, with `RatedCurrent` a pure nameplate datum) whose burden is
  deck migration — a silent trap for legacy decks, but not an engine defect.
- f1 never operates under either semantics (0.105 A < 2 A even with divisor 1.0),
  which is why only f2 blows on r4133.

## 4. Probe matrix (all reproducible; scripts follow `tools/golden/probe_val.py`
pattern, Oddie mechanics per `tools/opendss/README.md` §3b)

Original deck, `Run.dss` sequence (snapshot + 5001 dynamics steps, SLG fault
`Fault.thefault` B3.1-g at t=0.3 s, temporary):

| engine | snapshot | fuse states | dynamics |
|---|---|---|---|
| oddie:r4088 (10.2.0.1) | **converged, 18 iters** | f1,f2 closed | fault 0.3 s → Relay.mfrov/uv opens ~0.42 s → Recloser.rec1 OPENED FAST Sec=0.5115 → fault **CLEARED**; fuses hold; motor coasts to f=57.2053 Hz, slip 0.0466 |
| pinned 0.14.5 | **converged, 18 iters** | f1,f2 closed | identical to r4088 digit-for-digit (same eventlog times, same final motor state) |
| oddie:r4133 (11.0.0.1) | **NOT converged (26 iters)** | eventlog `Fuse.f2, Action=PHASE 3/2/1 BLOWN` at Sec=0, ControlIter=1 | unreachable |

Modified deck (single edit: `CurveMultiplier=65` on both fuses) on
**oddie:r4133**: snapshot **converged, 18 iters** (same as r4088-era), fuses
hold, identical protection sequence (relay ~0.42 s, rec1 FAST trip, fault
cleared), final motor state `Frequency=57.2053, Slip=0.0465776, Theta=-485.517`
— equal to the r4088 original to ~6 significant digits (residual diffs are
engine-generation solver last-ulp: `Is2` 0.0610175→0.0610215 etc.).

## 5. Verdict

**INTENTIONAL breaking change.** Announced in release notes, executed
coherently across four protection classes and the COM API, with a deliberate
no-fallback repurpose of the fuse's `RatedCurrent` (compat aliasing was used
where upstream wanted it — Recloser `PhaseTrip`). Under UPGRADE ledger §1.4
("EPRI r4133 wins except proven bugs") the port keeps r4133 parity — it already
reproduced the r4133 non-convergence on the originals — and no
`DIVERGENCES.md` entry is added (nothing diverges). The un-migrated originals
stay in `skipped_needs_investigation` (tag `r4133_breaking_nonconvergence`,
notes finalized as SETTLED).

## 6. Actions (equivalent coverage, live-gated vs oddie:r4133)

New corpus twins, single functional edit `CurveMultiplier=65` (redirects inlined
verbatim into one file each — the family dir↔manifest bijection forbids fixture
`.dss`; `wpwind2400.csv` rides along as a non-dss fixture; cosmetic BusCoords
and Run.dss's `Plot`/`Show`/`get` tail + mode-5 wall-clock `Monitor.Mtime`
dropped, documented in-deck):

- `tests/corpus/controls/fuse/indmach_r4133/indmach_snap.dss` — the Master.DSS
  surface: full circuit + `Maxcontroliter/maxiterations`, harness does 8 snap
  solves. Full-model per-step + fuse probes (state/normal/fusecurve/
  ratedcurrent/curvemultiplier) + eventlog + ctrlqueue, `oracle: "r4133"`,
  kind `feeder`.
- `tests/corpus/controls/fuse/indmach_r4133/indmach_dyn.dss` — the Run.dss
  surface: inline snapshot + `mode=dynamics h=166.67µs` + 5001 steps, then
  `Set number=1`; harness continues 2 dynamics steps. Adds
  `check_meters_monitors` (7 monitors × 5001 samples, modes 0+3),
  `compare_variables` on IndMach012.Motor1, fuse/relay/recloser state probes,
  eventlog (30,029 entries incl. the r4133 per-step relay Debug Sample lines) +
  ctrlqueue.

**§1.7 validation** (UPGRADE_PLAN.md): both twins on oddie:r4133 —
(1) converge (snap: 18,2,…,2 iters over 8 steps; dyn: 2,2 after the inline
run); (2) **bit-identical capture across two separate oracle processes**
(snap sha256 `0a90d76d…`, 455,627 bytes; dyn sha256 `a73077ba…`, 23,655,330
bytes); (3) feature-sensitive — the identical circuit without the edit (the
vendored original) blows f2 at Sec=0 and does not converge on r4133
(probe-proven), and the divide-by-RatedCurrent pre-r4133 path is pinned by the
sibling `fuse_curvemult_blow` micro.

## 7. Harness consequence — the monitor f32-ULP floor (proven, then implemented)

First gate run failed **one sample**: `indmach_dyn` step 0, monitor f2 ch3
(`V2`) sample 2154 — Rust f32 8939.841796875 vs r4133 f32 8939.8427734375,
|diff| = 9.766e-4 = exactly **1 f32 ulp**, marginally above the feeder band
(9.04e-4 at that magnitude; feeder `i_rel` 1e-7 < one-f32-ulp-rel 1.19e-7).
Decomposition per CLAUDE.md (never band-widen without proof):

- **Live f64 at the failing sample** (both engines re-run to exactly t=0.35917 s):
  |V(B4.2)| Rust 8939.842285059083 vs r4133 8939.842285310006 — **2.8e-11 rel**
  (350× inside the feeder `v_rel` floor), straddling the f32 midpoint
  8939.84228515625 → the recordings quantize one ulp apart.
- **Whole-trajectory census** (490,098 samples, 7 monitors): 5,622 samples
  differ; **5,580 (99.25%) by exactly 1 ulp**; the 2–9-ulp remainder is confined
  to the near-cancellation `I3`≈0.10–0.24 A residual current (absdiff ≤1.8e-7 A)
  and its angle during the fault window; diff density *decreases* over the run
  (no growth ⇒ no trajectory divergence).
- **All f64 surfaces of the same step pass the full feeder floors** (node V
  1e-8, Y, YPrims, element currents/powers, properties) — the compare order
  reaches monitors last.

Fix (`harness::compare_monitor` + `tests/TOLERANCE_NOTES.md` §monitor-f32-floor):
per-sample band floored at **one f32 ulp of the expected value** — implementing
the floor TOLERANCE_NOTES already documented ("Monitor channels are f32, so the
comparison floor is the f32 ULP") but the comparator never enforced because no
feeder-kind case had compared a long f32 trajectory before; plus the **angular
image of the magnitude floor** for `VAngle<n>`/`IAngle<n>` channels
(`rad2deg·(i_abs+i_rel·|mag|)/|mag|`, the voltage-scaled-power-floor
construction; tighter than the base band at healthy magnitudes, opens only
where the tightly-compared magnitude carries no angular information). Widens
nothing above the information content of f32 data; a real defect (≥2 ulps or
f64-visible) still fails.

## 8. Files touched

- `tests/corpus/controls/fuse/indmach_r4133/{indmach_snap.dss,indmach_dyn.dss,wpwind2400.csv}` (new)
- `tests/corpus/controls/manifest.json` (2 entries), `tests/corpus/manifests/population.lock.json` (regen)
- `tests/corpus/manifests/skipped_needs_investigation.json` (2 notes finalized SETTLED)
- `crates/dss-core/tests/harness/mod.rs` (`compare_monitor` f32-ULP floor + angle image)
- `tests/TOLERANCE_NOTES.md` (§monitor-f32-floor proof)
- this record.

## 9. Audit settlement (verdict CONFIRMED; 3 notes settled 2026-07-17)

**Note 1 — "f32-ULP monitor floor is global, not scoped to this deck" →
REBUTTED (kept global, by design; rationale recorded).** The floor is
representation-level, not deck-level: the *pinned 0.14.5 oracle's own*
`Monitor.pas:143` declares `MonBuffer: pSingleArray` with an f64→f32 store per
sample (`:1599`), and the r4088/r4133 Delphi trunks declare the identical
single-precision buffer (`Version8/Source/Meters/Monitor.pas:133`, both —
re-verified this session). Scoping the floor to r4133/oddie decks would assert
that pinned-oracle monitor data carries sub-ulp information, which is false for
every engine generation; a sub-ulp band on f32 data compares quantization
noise. It also masks nothing today: the full gate was green under the narrower
pre-floor bands immediately before the change, so no existing pinned-capi
comparison sits in the newly-opened sub-ulp window, and a future defect there
is ≥2 ulps or visible in the same steps' f64 surfaces (full tier floors,
unchanged). The ~19%-at-feeder-tier bound (and the honest micro-tier bound,
where the old band de-facto demanded bit-identical f32 samples — unguaranteeable
under midpoint straddle) is now stated explicitly in
`tests/TOLERANCE_NOTES.md` §monitor-f32-floor ("Scope — deliberately global").

**Note 2 — "AD classification deferred (`off:unclassified-new-deck`)" →
FIXED (classification performed, pending bucket retired for both twins).**
Ran the evidence protocol the reason's comment prescribes (`DSS_AD_CLASSIFY=1`
`ad_classify_families`, oracle-free rust-vs-rust probe, 15.1 s, exit 0):

- `indmach_snap` — snapshot-mode, AD-eligible by mode; normal arm solves, the
  AD save/reload leg fails to re-open the deck's relative-path fixture from the
  torn-circuit scratch dir (`ADCLASSIFY … off:ad-init-fail … ad-init: Error
  opening file: "wpwind2400.csv"`). Root cause = saved-circuit relative-path
  fidelity → reviewed reason **`off:save-roundtrip-relpath`** (same class and
  failure shape as `ExpControl/Master.dss`).
- `indmach_dyn` — dynamics deck (`Set mode=dynamics`, line 144) →
  **`off:mode-outside-AD-scope`** (AD sweep is power-flow-only by definition;
  precedent `VCCS/DG_Prot_Fdr.dss`); the probe additionally reproduces the same
  relpath failure.

Both manifest notes carry the classification facts; `off:unclassified-new-deck`
no longer appears on any T4 deck (the 40 remaining controls uses predate this
branch and stay with their documented follow-up round).

**Note 3 — "originals no longer gated; coverage depends on opt-in Oddie DLL" →
REBUTTED (no gating was removed; the dependency is a loud gate prerequisite,
not opt-in).** (a) The originals were **already skipped before this branch** —
the pre-branch `skipped_needs_investigation` note (WP-U2.1) reads "cannot be
live-gated on either engine"; T4 only finalized the wording and *added* two
gated twins, so net protection-coordination coverage strictly increased.
(b) They cannot be gated: the port implements r4133 fuse semantics (UPGRADE
§1.4 r4133-wins), so vs the pinned 0.14.5 oracle the port *intentionally*
disagrees (0.14.5 converges, Rust must reproduce the r4133 non-convergence),
and vs r4133 neither engine reaches a solved state to compare. (c) The Oddie
r4133 dependency fails loudly, never silently skips: `oracle: "r4133"` cases
run unconditionally inside `cargo test` (`family_cases_match_oracle` →
`Oracle::for_spec`), where a missing venv interpreter asserts
(`corpus_live.rs::oddie_venv_python`) and `ping_engine(Some("r4133"))` asserts
the answering engine has `oddie == true` and `rev == "r4133"` — a lost env or
absent DLL turns the gate red rather than evaporating the coverage (the
*opt-in* channel is the separate `DSS_LIVE_OPENDSS` inventory test, a
different mechanism). The replacement r4133 semantics themselves additionally
stay pinned by the sibling micros — `fuse_curvemult_blow` (CurveMultiplier is
the divisor, RatedCurrent inert) and `fuse_legacy_noblow` (default
FuseCurve=none never blows).
