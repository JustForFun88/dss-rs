# Broad-surface sweep: `dsspy_validation` (~40 API collections/case)

Complements `ab_compare.py` (which diffs the per-solve electrical state) by
dumping every `_columns` field of every object + `ActiveCktElement` records +
Solution/meter/monitor/CIM surfaces per case into one zip per engine, then
diffing offline (`compare_outputs.py`, with its loosened dss_capi-vs-EPRI
categories). **Inventory only** — never gates. Its `capi` side is dss_capi
**0.15.0b4** (= `capi015`), so it applies to the three EPRI-facing pairs below
(`capi015↔r4088`, `r4088↔r4133`, and the Rung-2-exit `capi015↔r4133`), not to
`capi(0.14.5)↔capi015`.

Per-engine dumps (append-mode, crash-resumed past the binary-shape
access-violation): capi015 **4265** entries, r4088 **4593**, r4133 **4593**.
Coverage is partial (~half the 206-case × ~40-collection grid) because the
binary/MMF-shape decks (`#58614`) hard-crash every engine ≥0.15.x mid-sweep;
this is itself an inventory finding (README §Surprises), not a harness fault.

## `r4088 ↔ r4133` (1106 field-level diffs)

| bucket | # | verdict |
|---|---|---|
| `NumProperties`/`AllPropertyNames` metadata (`38→39`) | 362 | **artifact**: a uniform +1 property-count bump from r4133's protection/property-system additions, amplified across every record of every class (incl. RegControl, whose `.pas` is **byte-identical** r4088↔r4133 — verified). NOT behavioral. |
| Relays/Reclosers `State`/`NormalState` `2→1`, Fuse `TCCcurve tlink→none`, Solution `EventLog` | ~520 | **real protection overhaul** — corroborates `ab_compare` and `delta_r4088_r4133.md` |
| Meter `CalcCurrent` blow-ups (≈1e15) | ~114 | **protection-downstream**: open-circuit meter reading on relay decks after a trip; `Meters/` source unchanged |

Net: dsspy adds no non-protection r4088→r4133 behavioral change beyond the two
`ab_compare` surprises (IEEE_519 harmonics, InductionMachine) — it confirms the
overhaul is protection + property-metadata.

## `capi015 ↔ r4088` (≈130k field-level diffs)

Dominated by `Lines` (62.7k), `Loads` (49.7k), `Transformers` (14.8k): the broad
dss_capi(FPC)-vs-EPRI(Delphi) surface — per-property value formatting +
last-ulp numerics on every element in every case. This is the known,
years-triaged divergence surface (`compare_outputs.py`'s loosened categories
exist precisely for it) and is **inventory-only**; the actionable electrical
divergences are the 24 in `capi015_vs_r4088.md`, not this metadata volume.

## `capi015 ↔ r4133` — the Rung-2-exit run (WP-U2.6, 2026-07-17; 130 689 field-level diffs)

Fresh full dumps of both engines (capi015 = dss_capi 0.15.0b4, and oddie:r4133 =
11.0.0.1) then `compare_outputs.py` — the broad-surface confirmation for the
Rung-2 exit. **195 of 206 cases show diffs; 130 689 field-level diffs total.** The
distribution is exactly the algebraic sum of the two already-committed neighbour
pairs — `capi015 ↔ r4088` (the broad FPC-vs-Delphi surface) **plus** `r4088 ↔
r4133` (protection + property-metadata) — with **no new, unexplained
non-protection r4133 behavior**:

| bucket | evidence | verdict |
|---|---|---|
| large-feeder numeric surface | `ckt24` (~9000 ea across the 8 MemoryMapping/base variants), `8500-Node`/`GFM_IEEE8500` (5.5k–6.6k), `epri_dpv/J1` (4504), `Storage-Quasi-Static` (4200), GFL/GFM `IBRDynamics` (258–2811) | **the FPC(0.15.x)↔Delphi(r4133) last-ulp value surface** — identical class to `capi015 ↔ r4088` (Lines/Loads/Transformers per-property formatting + last-ulp on every element); years-triaged, `compare_outputs.py`'s loosened categories exist for it. Inventory-only. |
| property-count metadata | 131 cases at ≤10 diffs each | the uniform **`NumProperties` +1** artifact (r4133's protection/property-system additions amplified across every record) — NOT behavioral, same as `r4088 ↔ r4133`. |
| protection | `59NRelayDemo` (104) — the one protection example in the 206-case upstream list | the **protection overhaul** (per-phase `State`/`NormalState` repr + curve/metadata), corroborating `ab_compare` + `delta_r4088_r4133.md`. The synthetic relay/recloser/fuse/swtcontrol decks are dss-rs corpus additions, not in this upstream case list, so they gate only in the mandatory gate (`oracle:"r4133"`). |

Net: capi015↔r4133 decomposes cleanly into the known broad FPC↔Delphi surface +
the r4088→r4133 protection/metadata delta; the dsspy broad surface surfaces **no
non-protection r4133 behavioral change** beyond the electrical-sweep findings
(both r4133-side surprises — IEEE_519 harmonics, InductionMachine — already
dispositioned, `docs/upgrade/DIVERGENCES.md` §Rung-2). Corpus verified pristine
after the sweep (`git status tests/corpus` clean). Inventory only — never gates.

## Regeneration

```
cd tools/opendss/dsspy_validation
../.venv/Scripts/python save_outputs.py dss-extensions                               # capi015
DSS_EXTENSIONS_TEST_ODDIE=oddie:r4088 ../.venv/Scripts/python save_outputs.py dss-extensions-odd
DSS_EXTENSIONS_TEST_ODDIE=oddie:r4133 ../.venv/Scripts/python save_outputs.py dss-extensions-odd
../.venv/Scripts/python compare_outputs.py <zipA> <zipB>
```
Zips land in `tmp/dsspy_validation/` (gitignored, ~100–170 MB each) — out of git.
