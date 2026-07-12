# Official EPRI OpenDSS oracle (Oddie bridge) — inventory channel + target-rev gates

Compares the Rust port (and the pinned dss_capi oracle) against **original EPRI
OpenDSS binaries** — three revisions, loaded side by side, no COM registration.
Two consumers: the **opt-in inventory channel** (workflows 1/2/4 below —
divergence reports, never failures) and, since UPGRADE_PLAN.md WP-U0, the
**mandatory gate's target-rev cases** (workflow 3b — a manifest case with an
`oracle` field is live-compared against the named engine, so this venv +
`bin/` are now `cargo test` prerequisites; `r4088` = dss_capi 0.15.x base,
`r4133` = latest release 11.0.0.1).

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
non-system import, + `kmetis.exe`/`pmetis.exe` — the METIS partitioners the
engine spawns from the DLL's own directory when A-Diakoptics tears a circuit —
+ `License.txt`), vendored from `.inputs/electricdss-code-
r*-trunk/Version8/Distrib/x64/` by `vendor_binaries.py` (checksums in
`bin/SHA256SUMS`). `DSSProgress.exe` is not vendored, so progress popups are
impossible.

## Setup (once)

```
python tools/opendss/vendor_binaries.py            # if bin/ is not populated
python -m venv tools/opendss/.venv                 # python >= 3.11
tools/opendss/.venv/Scripts/pip install --find-links tools/opendss/wheels ^
    dss-python==0.16.0b2 dss-python-backend==0.15.0b4 pandas xmldiff
tools/opendss/.venv/Scripts/python tools/opendss/smoke.py
```

The two **beta** packages are vendored as wheels in `wheels/` (checksums in
`wheels/SHA256SUMS`) — `--find-links` takes them from there, so the setup
does not depend on the pre-releases staying on PyPI; stable deps (numpy,
cffi, typing_extensions, pandas, xmldiff) come from PyPI as usual.

Pins: `PIN_OPENDSS.txt` (venv contents) + `revisions.json` (`expect_version`
per revision — discovered and verified by `smoke.py`; the server refuses an
empty pin). Extra fallback: `.inputs/DSS-Python` checkout IS the 0.16.0b2
tag (pure-Python part only; the backend wheel is the compiled piece).

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

writes `tmp/opendss_report_<rev>.json` (`matched` / `known_diverged` /
`known_skipped` / `diverged_new` + per-entry `known_hits`). Divergences are
**inventory, not failures**: the port is calibrated to dss_capi 0.14.5, which
intentionally differs from EPRI upstream (dss_capi `docs/known_differences.md`)
on top of Delphi-vs-FPC numeric drift. Triaged divergence classes live in
**`tests/corpus/known_diffs.json`** (modeled on DSS-Python's `KNOWN_COM_DIFF`):
substring match on (case label, first-failure reason), every entry must state
its `cause`, zero-hit entries are warned about so the catalog can't rot.
A `kind: "skip"` entry marks a case a revision cannot run/converge at all —
it is skipped up front and reported under `known_skipped` (matched on the
case label alone).
`DSS_LIVE_OPENDSS_ASSERT=1` fails only on **new** (uncataloged) divergences —
green for r3723, whose 82 divergences are fully triaged. Caveat: the
comparison stops at a case's first divergence, so a known first divergence
masks any later one in that case; entries retire as upstream deltas get
ported, re-exposing what was behind them. Comparators/tolerances are shared
with the mandatory gate and are never weakened for this one.

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
`--known-diffs tests/corpus/known_diffs.json` relabels cases whose every issue
matches a catalog entry as `known_diverged` (entries use `ab_contains` where
this tool's issue wording differs from the Rust panics), skips `kind: "skip"`
cases up front (`known_skipped`), and exits 0 when only known differences
remain.

**3. Smoke** — after re-vendoring or bumping pins:
`tools/opendss/.venv/Scripts/python tools/opendss/smoke.py`.

**3b. Target-rev gating (UPGRADE_PLAN.md) — MANDATORY-gate usage of this
channel.** A corpus/family manifest case may set `"oracle": "capi015" |
"r3723" | "r4088" | "r4133"`: the mandatory live gate then compares that case
against **that** engine instead of the pinned capi oracle, with the iteration
policy relaxed to *Rust ≤ oracle* (all other comparators/tolerances shared,
never weakened). `capi015` is the dss_capi **0.15.x-line** oracle — dss-python
0.16.0b2 from this same venv driving its bundled 0.15.0b4 backend (OpenDSS SVN
r4103) — the scriptable r4088-line oracle. An upgrade WP flips a case's
`oracle` in the same commit that ports the newer upstream behavior; the case
is then automatically excluded from the `corpus_live_opendss` inventory sweep
(its gating moved to the mandatory gate). `modes/upgrade/upgrade_pilot.dss` keeps the
machinery exercised, making this venv + `bin/` a mandatory `cargo test`
prerequisite.

**4. DSS-Python broad-surface validation** (`dsspy_validation/`, vendored
from DSS-Python `fastdss`, BSD-3) — full-API-state dumps (~40 collections per
case, 206 upstream-curated cases) zipped per engine and diffed offline;
complements `ab_compare.py`'s per-solve electrical diff. **Its `capi` side is
dss_capi 0.15.0b4, not the pinned 0.14.5 oracle — inventory only.** See
`dsspy_validation/README.md`; end every sweep with `git status tests/corpus`.

## Environment variables

| var | consumer | meaning |
|---|---|---|
| `DSS_ORACLE_ENGINE` | oracle_server.py | `capi` (default, pinned 0.15.7), `capi015` (0.15.x line, this venv) or `oddie` |
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
- **A-Diakoptics runs here** (probe-proven 2026-07-11; the pinned capi oracle
  can't run it — `DSS_CAPI_ADIAKOPTICS` compiled out). Two hard rules when
  driving it (see `DIAKOPTICS_PSTCALC_PLAN.md` D9/D10): (1) the PM/AD `solve`
  is **asynchronous** — always issue `wait` after every `solve` before reading
  results, or a zone can be captured as zeros while the coordinator still
  reports Converged; (2) `kmetis.exe` must sit next to the loaded DLL (it does,
  in `bin/<rev>/` and in the `.inputs` x64 dirs). Copy decks to a temp dir
  first — `set ADiakoptics=True` writes `Torn_Circuit/` + `.graph`/`.part.*`
  beside the model (corpus-pollution hazard).
