# WP-U0.2 — upgrade inventory sweeps (empirical completeness check)

The three pairwise `ab_compare.py` engine sweeps + the `dsspy_validation`
broad-surface sweep that UPGRADE_PLAN §WP-U0 "Follow-up step U0.2" mandates:
the empirical check on the survey-based scoping in the three
`docs/upgrade/delta_*.md` inventories, run over the full corpus **before** any
porting WP opens. **Report-only** — a divergence is data feeding WP-U1.1..U2.6,
never a failure (this WP changes no engine code, flips no `oracle`, regenerates
no golden).

Run 2026-07-11 on branch `wp-u02` (base `3cca7d3`, main incl. FINAL ACCEPTANCE
+ WP-U0 multi-oracle infra).

## Engines

| spec | engine | role |
|---|---|---|
| `capi` | dss_capi **0.14.5** / OpenDSS SVN r3723 (pinned oracle, `PIN.txt`) | the port's current calibration point |
| `capi015` | dss_capi **0.15.0b4** / OpenDSS SVN r4103 (dss-python 0.16.0b2, Oddie venv) | Rung 1 target (FPC, same lineage) |
| `oddie:r4088` | EPRI OpenDSS **10.2.0.1** "Columbus" (official DLL) | Rung 1 Delphi cross-check / Rung 2 diff base |
| `oddie:r4133` | EPRI OpenDSS **11.0.0.1** "Charlottesville" (official DLL) | Rung 2 target + plan end-state |

## The three pairs & what each isolates

| pair | isolates | inventory it checks |
|---|---|---|
| **capi ↔ capi015** | the dss_capi `0.14.5 → 0.15.x` delta (FPC-to-FPC, same lineage) | `delta_capi_0145_015x.md` (Rung 1 item list) |
| **oddie:r4088 ↔ oddie:r4133** | the EPRI Delphi `10.2 → 11.0` delta | `delta_r4088_r4133.md` (Rung 2 item list) |
| **capi015 ↔ oddie:r4088** | dss_capi(FPC) vs EPRI(Delphi) at the same r4088/r4103 line | `delta_r3723_r4088.md` cross-check + the DIVERGENCES ledger (L1–L4) |

## Case universe (378 cases per pair)

solvable_now (245) + asymmetric (36) + controls (57) + modes (40). Same
manifests the mandatory gate reads.

## Headline counts (378 cases)

| pair | match | diverged | error* |
|---|---|---|---|
| capi ↔ capi015 | 334 | 17 | 27 (capi015: 16 strict-validation + 8 `#303`-intrinsic + 2 crash; 1 capi) |
| r4088 ↔ r4133 | 344 | 27 | 7 |
| capi015 ↔ r4088 | 323 | 24 | 31 (24 capi015-side strict/intrinsic/crash; 7 r4088-side) |

\* `error_a`/`error_b` = one engine raised/aborted where the other solved. For
these pairs almost every "error" is itself a witnessed behavior delta (strict
property validation, or a genuine engine crash), not a harness fault — see the
per-pair files.

## Cross-pair attribution (the sweeps disambiguate which rung owns a delta)

- **Protection overhaul (Relay/Recloser/Fuse/SwtControl)** is owned **entirely by
  Rung 2 (r4133)**. Proof: the protection decks diverge massively in
  `r4088↔r4133` (open-node voltages 7e9, ctrlqueue `2 vs 0`, Y-fingerprint up to
  1.44) but in `capi015↔r4088` the **same** decks agree to V≈1e-15 with only
  event-log **text**/queue-order differing. capi015 (Rung 1) reproduces r4088
  protection *logic* exactly; only the r4133 rewrite moves the numbers. This is
  the empirical confirmation of `delta_r4088_r4133.md`'s "solver/PC/meters
  byte-identical; everything numeric is in the protection controls" scoping.
- **The `0.14.5→0.15.x` behavioral fixes** (DynamicExp, GFM, InvControl,
  RegControl, Carson, Capacitor, StorageController, seasonal, PermissiveProperties
  strictness) are owned by **Rung 1**: they appear in `capi↔capi015` and are
  absent (or only text) in the neighbour pairs.

## Surprises (observed diffs the inventories did NOT predict — new rows added)

1. **`r4088→r4133` harmonics divergence on `IEEE_519`** (V 1.3e-2 @ pcc, Y
   *identical*): `delta_r4088_r4133.md` claims the solver/PCElements are
   byte-identical r4088=r4133. A harmonics-mode result move with an identical
   assembled Y contradicts that. Determinism-probed (r4088↔r4088 same-engine):
   see `r4088_vs_r4133.md` §Surprises for the verdict. Row added to
   `delta_r4088_r4133.md` bucket B.
2. **`r4088→r4133` InductionMachine convergence flip** (`Master.DSS`/`Run.dss`
   converged True-vs-False, Y-fingerprint 0.05–0.89): another non-protection
   move. Row added.
3. **Binary/MMF shape + XYcurve file access-violation crash** (`#58614`) in
   dss_capi **0.15.0b4 AND** EPRI r4088/r4133, but **not** in 0.14.5. Decks
   `shape_binfiles` (`g4.csv` GrowthShape), `shape_mmf`, `xycurve_files`
   (`rc.csv`). A 0.15.x-era regression in binary-format shape parsing; no delta
   row existed. Added as a NOTE(oracle-regression) — no port action (the Rust
   engine reads these correctly), but it is why these decks show as `error` in
   the sweeps and why the raw sweep needed per-case isolation for the modes
   family (see §Mechanics).

## Mechanics & regeneration

Corpus pairs (solvable_now+asymmetric+controls, 338 cases) run in one
`ab_compare` process each. The **modes** family (40) is re-run **one case per
process** because two decks hard-crash the shared engine (the `#58614`
access-violation above and `newton_feeder.dss`, a non-converging Newton solve
that hits the 300 s request timeout); a shared-engine run poisons every
following modes case (`#303 on clear`). The per-pair JSON/MD summaries here
merge the corpus run with the isolated-modes run.

Regenerate (from the worktree root, Oddie venv junctioned):

```
export DSS_OPENDSS_PYTHON="…/tools/opendss/.venv/Scripts/python.exe"
M="--manifest tests/corpus/manifests/solvable_now.json \
   --manifest tests/corpus/asymmetric/manifest.json \
   --manifest tests/corpus/controls/manifest.json"
# corpus pairs
python tools/opendss/ab_compare.py --a capi        --b capi015     $M --timeout 300
python tools/opendss/ab_compare.py --a oddie:r4088  --b oddie:r4133 $M --timeout 300
python tools/opendss/ab_compare.py --a capi015      --b oddie:r4088 $M --timeout 300
# modes family: one case per process (avoids the crasher cascade)
#   see scratchpad run_modes_isolated.py — loops the 40 modes cases with
#   --manifest tests/corpus/modes/manifest.json --case <path> --timeout 45
# dsspy broad-surface (per engine, then compare_outputs.py):
cd tools/opendss/dsspy_validation
../.venv/Scripts/python save_outputs.py dss-extensions                              # capi015
DSS_EXTENSIONS_TEST_ODDIE=oddie:r4088 ../.venv/Scripts/python save_outputs.py dss-extensions-odd
DSS_EXTENSIONS_TEST_ODDIE=oddie:r4133 ../.venv/Scripts/python save_outputs.py dss-extensions-odd
```

`ab_compare.py` gained a `capi015` engine spec in this WP (tooling only — it
already supported `capi` and `oddie:*`; `capi015` binds
`DSS_ORACLE_ENGINE=capi015` on the Oddie-venv interpreter).

Raw per-case JSON dumps are bulky and stay out of git (scratchpad
`sweeps/`); this directory keeps the distilled summaries only.
