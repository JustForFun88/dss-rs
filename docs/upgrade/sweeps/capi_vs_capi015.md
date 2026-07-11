# Sweep: `capi` (0.14.5/r3723) ↔ `capi015` (0.15.0b4/r4103) — Rung 1

Isolates the dss_capi `0.14.5 → 0.15.x` delta (FPC-to-FPC, the port's own
lineage). Checks `delta_capi_0145_015x.md`. **378 cases: 334 match, 17
diverged, 27 error** (26 capi015 strict-validation `error_b`, 1 capi `error_a`).

## Divergence classes

Counts are **per-signal occurrences**, not case counts: one case can appear in
several rows (e.g. `storagecontroller_seasonal` is both Yf-only and event-log),
so the diverged rows below sum to more than the 17 diverged headline. The error
rows (strict-validation / crash / `#303`) partition the 26 `error_b` exactly.

| class | # (per-signal) | notable magnitude |
|---|---|---|
| voltage (solved-state move) | 4 | `Dynamic_KundurDynExp` V **0.87** |
| Y-fingerprint only (V≈unchanged) | 11 | `gfm_*` Yf **3.6e-3**, `Local/Mon_voltage_*` Yf 1.8e-6 |
| iterations + discrete taps | 2 | `midi_controls` 68→78, `midi_invcontrol` 66→106 |
| event-log | 2 | `storagecontroller_seasonal`, `storagectrl_*` |
| capi015 strict-validation error | 16 | see error table |
| capi015 hard crash (`#58614`/timeout) | 3 | binary-shape decks |
| `#303` intrinsic (torn/isource/storage) | 7 | see error table |

## Diverged cases → inventory item (Rung 1 spec)

| case(s) | signal | maps to |
|---|---|---|
| `Dynamic_KundurDynExp.dss` | V **0.87** @ lt.3 | **D14** DynamicExp RPN-index fix |
| `gfm_micro/gfm_invcontrol/gfm_dynamics/pv_gfm_dynamics` | Yf **3.6e-3**, V≈1e-12 | **B5** GFM `Isc1` ×1000 removal (Norton YPrim moves; op-point stable) |
| `midi_invcontrol.dss` | iter 66→106, V 3.4e-2 | **D1–D4** InvControl cluster (ΔV buffer, per-DER basekV, deltaQ, delta-DER LL) |
| `midi_controls.dss` | iter 68→78, reg taps + xfmr differ, V 5.7e-2 | **C5/B4** RegControl reverse/idle rework (+ control interplay) |
| `Test/Cable_constants.DSS` | Yf **2.6e-6** | **B2/D1** SimpleCarson `658.5→658.85` and/or **B3** CN-cable SemiconLayer capacitance |
| `Local_voltage_{2,average,max,min}-2`, `Mon_voltage_average{,_LL}-2` | Yf **1.8e-6**, V≈1e-7 | **B1** Capacitor Cmatrix ×1.000001 (or PVSystem YPrim init) — YPrim-only |
| `4Bus-YD-Bal.DSS` | V 2.3e-5 @ n2.4 | small transformer/line move (D-adjacent) |
| `YYD-Master-step1.DSS` | Yf 1.3e-6 | **B1**/transformer YPrim ~1e-6 |
| `storagecontroller_seasonal.dss` | Yf 2.1e-5 + eventlog 13-vs-12 | **D10** StorageController + **E2** seasonal-rating reimpl |

## capi015 strict-validation errors → **C2 PermissiveProperties strict default** (+ B7 parser)

The dominant Rung-1 surface change: capi015 **errors by default** where 0.14.5
silently accepted. This confirms `delta_capi` C2's warning that *the default is
the new strict behavior* — 16 decks change classification (solve → parse
error) with no compat flag. **Ledger L2** (WP-U1.1): the plan's target is EPRI
r4133's `DblValueNZ` **clamp** (kW=0→1e-8), not capi015's strict error.

| error code | meaning | # | inventory |
|---|---|---|---|
| `#2025111` | `Load.kW: Value cannot be zero in this style of specification` | 1 | C2 zero-check (ledger L2) |
| `#2024101` | `Transformer.NormAmps: DSS property is read-only` | 3 | C2 read-only strict |
| `#20241024`/`#2024110` | `CSV file contains more/fewer items than expected` | 10 | B7 parser strict CSV |
| `#20241011` | `Array "…" contains more items than expected` | 1 | B7 parser strict array |
| `#65001` | `Zero frequency detected in Spectrum` | 1 | strict spectrum validation |

Representative decks: `epri_dpv/M1/Master_NoPV` (kW=0); the 3 NormAmps read-only
(`#2024101`) decks are `StoCtrl_SeasonTarget/{IEEE13NodecktMOD,Run_example}.dss`
+ `ADiakoptics/IEEE_123_Bus-G/Torn_Circuit/zone_2/master.dss` (verified in
`merged_capi_vs_capi015.json` — `StoCtrl_Current_PeakShave` MATCHES, it is not a
witness); all `StorageControllerTechNote/*` + `8500-Node/P174_Run_360kW_PV` (CSV
counts), `FreqScan/Run_Scan` (spectrum), `makeposseq_shunt` (array).

## Other errors (not a Rung-1 behavior delta to port)

- `#58614` **access-violation crash** on `shape_binfiles` (`g4.csv` GrowthShape),
  `xycurve_files` (`rc.csv`), + `pstcalc_cmd` timeout — the 0.15.x binary-shape
  regression (README §Surprises). 0.14.5 handled them; **no port action**.
- `#303` on `ADiakoptics/**/Torn_Circuit/*` (sub-decks not meant to run
  standalone), `isource_daily/both`, `midi_isource`, `autotrans_reg`,
  `Storage_price` (`%stored`) — capi015 intrinsic errors; several are genuine
  0.15.x behaviors (isource daily) worth a WP-U1.6 probe, none block scoping.
- `capcontrol_follow_noshape` (`#10362`, `error_a`) — 0.14.5 **and** 0.15.x both
  require `ControlSignal` for `Type=Follow` (a dss-ext feature); not a delta.

## Inventory items with NO corpus witness here (must be synthesized)

`delta_capi_0145_015x.md` items the corpus does not exercise — annotated in that
file "no corpus witness, deck must be synthesized (WP-U1.x)":
A1 NCIM (opt-in `Algorithm=NCIM`), A2 WindGen (0.15.x form), A3/A5 force hooks,
B3/C1 new line-constant paths (EpsRMedium/HeightOffset/equivalent-spacing/
SemiconLayer/CNTS — defaults preserve numerics so no corpus deck moves), C4
`Solve/Clear all`, C6 Transformer BH curves, C7 `TCC_Curve.none`, D9/D16 meter
disabled-skip, D15 `LookupVariable` case-insensitivity.
