# tests/corpus/gaps — synthesized decks for the test-blocked deferrals

Hand-written DSS decks for every feature that was deferred during Phases 4–7
**only because no vendored corpus deck exercises it** (the WP7.9
"zero corpus cases → skip" anti-pattern, called out in `PHASE8_PLAN.md` §1),
**plus** (2026-07-05) the five **unported element classes** the registry still
lacks — Isource, AutoTrans, GICLine, GICTransformer, GICsource (WPG.15–WPG.17).
The execution plan that consumes them is **`GAPS_PLAN.md`** at the repo root —
read it first; it records per-deck determinism proofs and gate strategy.

Same shape as the sibling synthetic families (`../asymmetric/`, `../controls/`):
local decks + `manifest.json`, live-compared against the pinned oracle by a
dedicated `corpus_live.rs` section (`gaps_cases_match_oracle`, added at
GAPS_PLAN execution). Every case starts `pending: true` — its feature is still
`NOT_PORTED` and the Rust engine must error loudly; the WP that ports it flips
the flag and the case joins the live compare (full model + probes +
`compare_eventlog`/`compare_ctrlqueue`/`check_meters_monitors`).

Every deck was validated against the pinned oracle (dss-python 0.15.7, see
`tools/golden/PIN.txt`): it compiles, solves, converges, is **bit-identical
across two separate oracle processes** (this matters — FPC `mathutil.pas`
time-seeds the RNG per process, so only RNG-free / `random=none` paths can be
pinned), and is **feature-sensitive** (removing the feature under test provably
changes the oracle output).

| Deck | Pins | WP |
|---|---|---|
| `shape_binfiles.dss` | LoadShape `SngFile/DblFile/PQCSVFile`, TShape/PriceShape binary, GrowthShape `CSVFile` | WPG.1 |
| `generaltime.dss` | `Set mode=Time` (`SolveGeneralTime`) | WPG.2 |
| `ld1.dss`, `ld2.dss` | `Set mode=LD1/LD2` + `Set LDCurve=` (load-duration) | WPG.3 |
| `monte1.dss`, `monte2.dss`, `monte3.dss` | `Set mode=M1/M2/M3` under `Set random=none` | WPG.4 |
| `montefault.dss` | `Set mode=MF` (single Fault ⇒ deterministic pick) | WPG.4 |
| `autoadd.dss` | `Set mode=AutoAdd` capacity search (winner: bus `b3`) | WPG.5 |
| `newton.dss` | `Set algorithm=Newton` | WPG.6 |
| `capcontrol_follow.dss` | CapControl `type=Follow` + `ControlSignal=` | WPG.7 |
| `reactor_rlcurve.dss` | Reactor `RCurve`/`LCurve` under harmonics | WPG.8 |
| `invcontrol_expmodel.dss` | InvControl `ControlModel=1` (Exponential/TPICtrl) | WPG.9 |
| `invcontrol_storage_vw.dss` | InvControl `mode=voltwatt` over Storage | WPG.10 |
| `invcontrol_storage_vv_vw.dss` | InvControl `combimode=VV_VW` over Storage | WPG.10 |
| `storagecontroller_seasonal.dss` | StorageController seasonal targets + `Set SeasonRating/SeasonSignal` | WPG.11 |
| `isource_snap.dss` | Isource snapshot: 1-ph trio, explicit `Bus2`, `sequence=neg` | WPG.15 |
| `isource_daily.dss` | Isource `daily=` shape drive (8 h, meter + monitors) | WPG.15 |
| `isource_harm.dss` | Isource spectrum injection under `mode=harmonics` (+ `scantype=zero`) | WPG.15 |
| `autotrans_snap.dss` | AutoTrans 3-wdg s/w/d (AutoAuto 330 MVA) + 2-wdg unit; `WdgCurrents` probe | WPG.16 |
| `autotrans_reg.dss` | RegControl on AutoTrans common winding (daily, event log) | WPG.16 |
| `autotrans_gic.dss` | AutoTrans `GICBuildYTerminal` (< 0.51 Hz Rdc-only branch) | WPG.16 |
| `gicline_gic.dss` | GICLine: `Volts` + `EN/EE` geodesy + blocking-`C` units @ 0.1 Hz | WPG.17 |
| `gictransformer_gic.dss` | GICTransformer GSU/YY/Auto, `R1/R2` + `%R1` specs, `VarCurve` | WPG.17 |
| `gicsource_gic.dss` | GICsource named-Line splice (`GIC_<name>` bus, Bus2 rewrite) | WPG.17 |

`ls8.sng`, `ls8.dbl`, `lspq8.csv`, `t8.sng`, `p8.dbl`, `g4.csv` are the committed
input fixtures for `shape_binfiles.dss` (rebuildable via
`tools/corpus/gen_gaps_binshapes.py`).

Unlike `../electricdss-tst/` this directory is **not** vendored (no SHA256SUMS
entry, not managed by `tools/corpus/vendor.py`) — it is first-party, edited by
hand. Deck edits require re-running the GAPS_PLAN §3 validation protocol
(two-process determinism + feature sensitivity) on the pinned oracle.
