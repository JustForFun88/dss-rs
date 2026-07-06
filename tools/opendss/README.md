# Official EPRI OpenDSS oracle (Oddie bridge) — opt-in test infrastructure

Compares the Rust port (and the pinned dss_capi oracle) against **original EPRI
OpenDSS binaries** — three revisions, loaded side by side, no COM registration.
The mandatory `cargo test` gate is **untouched**: everything here is opt-in,
for inventorying upstream behavior changes ahead of porting them (r4088 =
dss_capi 0.15.x base, r4133 = latest release 11.0.0.1).

## How it works

[AltDSS **Oddie**](../../.inputs/dss_capi_with_git/docs/Oddie.md) wraps EPRI's
official `OpenDSSDirect.dll` (the flat "Direct DLL" build of the engine) with
the same API as dss-python. `dss.IOddieDSS(library_path=<abs dll>)` — from
dss-python **0.16.0b2** (fastdss line) in a **separate venv** — loads any
vendored revision by path. The existing `tools/oracle/oracle_server.py` then
runs unchanged: `DSS_ORACLE_ENGINE=oddie` rebinds its engine (`make_engine()`),
and every capture (`gen_checkpoints.capture_*`) works identically, including
the sparse system Y (`YMatrix_GetCompressedYMatrix`) and injection vector.

COM (`OpenDSSengine.dll` / `dss.patch_dss_com`) is deliberately NOT used: it
has no YMatrix API (dense `SystemY` only) and regsvr32 registration is
machine-global (one version at a time).

| revision | engine version | based on |
|---|---|---|
| `r3723` | 9.8.0.1 | dss_capi 0.14.x base (the port's pinned oracle line) |
| `r4088` | 10.2.0.1 "Columbus" | dss_capi 0.15.x base |
| `r4133` | 11.0.0.1 "Charlottesville" | latest official release, no dss_capi yet |

Binaries live in `bin/<rev>/` (`OpenDSSDirect.dll` + `KLUSolve.dll`, its only
non-system import + `License.txt`), vendored from `.inputs/electricdss-code-
r*-trunk/Version8/Distrib/x64/` by `vendor_binaries.py` (checksums in
`bin/SHA256SUMS`). `DSSProgress.exe` is not vendored, so progress popups are
impossible.

## Setup (once)

```
python tools/opendss/vendor_binaries.py            # if bin/ is not populated
python -m venv tools/opendss/.venv                 # python >= 3.11
tools/opendss/.venv/Scripts/pip install dss-python==0.16.0b2
tools/opendss/.venv/Scripts/python tools/opendss/smoke.py
```

Pins: `PIN_OPENDSS.txt` (venv contents) + `revisions.json` (`expect_version`
per revision — discovered and verified by `smoke.py`; the server refuses an
empty pin). Fallback if 0.16.0b2 ever leaves PyPI: `pip install
dss-python-backend==0.15.0b4 "numpy>=2,<3" "typing_extensions>=4.5,<5"` then
`pip install --no-deps .inputs/DSS-Python` (that checkout IS the 0.16.0b2 tag).

`smoke.py` also proves per revision that EPRI's CSC export (which always
factors first — `InitAndGetYparams`, DYMatrix.pas) is solution-neutral:
`YNodeVarray` must be bit-identical before/after `getYSparse()`. If that ever
fails on a new revision, run oddie cases with `full_csc=false`.

## Workflows

**1. Rust vs EPRI (opt-in gate, report mode)** — the full `run_and_compare`
mandate over the same case universe as the mandatory gate (solvable_now +
asymmetric + controls):

```
DSS_LIVE_OPENDSS=r4133 cargo test -p dss-core --test corpus_live corpus_live_opendss -- --nocapture
```

writes `tmp/opendss_report_<rev>.json` (`matched` / `diverged` with reasons).
Divergences are **inventory, not failures**: the port is calibrated to
dss_capi 0.14.5, which intentionally differs from EPRI upstream (dss_capi
`docs/known_differences.md`) on top of Delphi-vs-FPC numeric drift.
`DSS_LIVE_OPENDSS_ASSERT=1` promotes divergences to a test failure (for r3723
once its report triages clean). Comparators/tolerances are shared with the
mandatory gate and are never weakened for this one.

**2. EPRI vs EPRI (upstream-change inventory)** — diff two engines on the same
cases to see what changed between revisions (the porting-planning tool):

```
tools/opendss/.venv/Scripts/python tools/opendss/ab_compare.py --a oddie:r3723 --b oddie:r4133
python tools/opendss/ab_compare.py --a capi --b oddie:r3723 --case 13Bus
```

Engine specs: `capi` | `oddie:<rev>` | `oddie:@<dll-path>`. Writes
`tmp/ab_<a>_vs_<b>.json` + `.md` summary. Tolerances are CLI flags (`--v-rel`
etc.) — this is a diff report, not a calibrated gate. Additional manifests via
`--manifest tests/corpus/asymmetric/manifest.json` (repeatable).

**3. Smoke** — after re-vendoring or bumping pins:
`tools/opendss/.venv/Scripts/python tools/opendss/smoke.py`.

## Environment variables

| var | consumer | meaning |
|---|---|---|
| `DSS_ORACLE_ENGINE` | oracle_server.py | `capi` (default, pinned 0.15.7) or `oddie` |
| `DSS_OPENDSS_REV` | oracle_server.py | `r3723`/`r4088`/`r4133` → revisions.json lookup |
| `DSS_OPENDSS_DLL` / `DSS_OPENDSS_EXPECT` | oracle_server.py | direct DLL path override (+ optional version substring) |
| `DSS_OPENDSS_PYTHON` | corpus_live.rs, ab_compare.py | Oddie-venv interpreter (default `tools/opendss/.venv/Scripts/python.exe`) |
| `DSS_LIVE_OPENDSS` | corpus_live.rs | gates `corpus_live_opendss`; value = revision |
| `DSS_LIVE_OPENDSS_ASSERT` | corpus_live.rs | `1` → divergences fail the test (default: report) |
| `DSS_ORACLE_PYTHON`, `DSS_ORACLE_TIMEOUT_SECS` | corpus_live.rs | unchanged (pinned oracle / shared timeout) |

## Rules & caveats

- **One revision per process.** Oddie wraps ONE engine per process (loading
  the same DLL twice aliases the same engine). Enforced structurally: the Rust
  gate spawns one server per case; `ab_compare` runs one subprocess per engine;
  `smoke.py` re-execs per revision.
- **The engine chdirs the process on `Compile`** (`AllowChangeDir` is not
  settable through Oddie) — server config is resolved at import time; never
  rely on relative paths after a compile.
- **Editor suppression is explicit.** EPRI's Delphi `FireOffEditor`
  (Utilities.pas) ShellExecutes `DefaultEditor` on every `Show`/`Export`
  unconditionally (no `NoFormsAllowed` check), and `AllowEditor` is not
  settable through Oddie — without countermeasures a corpus sweep opens
  hundreds of Notepads. `make_engine()` therefore issues
  `Set RegistryUpdate=No` (so the override is never persisted to the user's
  OpenDSS registry settings) + `Set Editor=rundll32.exe` (a GUI-subsystem
  no-op: no DLL entry point given → exits silently). Combined with the corpus
  guard and no `DSSProgress.exe`, no popups and no corpus pollution.
- Known divergence classes vs dss_capi (seen in the r3723 report): monitor
  header whitespace (`" VAngle1"` vs `"VAngle1"`), meter zone list placeholder
  shapes, property-value formatting (`"[ 1900 1500 2300]"` vs `"1900 1500
  2300"`), iteration-count deltas, plus genuine engine differences listed in
  dss_capi `docs/known_differences.md`.
