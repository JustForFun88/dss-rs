"""Smoke-test the Oddie bridge over each vendored EPRI OpenDSSDirect.dll.

For every revision in `revisions.json` (one subprocess per revision — Oddie
wraps ONE engine per process; loading the same DLL twice aliases the same
engine, so per-process isolation is the only safe mode):

  1. assert the venv pin (`PIN_OPENDSS.txt`: dss-python / dss-python-backend);
  2. load the DLL via `dss.IOddieDSS(library_path=<abs>)`, print `DSS.Version`;
  3. compile + solve the IEEE13 master from the vendored corpus (inside the
     oracle server's `_CorpusGuard` so no report files pollute the corpus);
  4. prove the CSC Y export is solution-neutral: EPRI's `InitAndGetYparams`
     ALWAYS factors before exporting (DYMatrix.pas), so `YNodeVarray` must be
     bit-identical before/after `getYSparse()` — this is the empirical proof
     the oracle captures can use `full_csc` on the EPRI engine;
  5. sanity-check `getI()` length (= 2*(NumNodes+1), slot 0 = ground);
  6. check `DSS.Version` against `revisions.json` `expect_version`; while the
     pin is empty, print the discovered string as `PIN THIS:` (fill it in and
     re-run — the oracle server refuses an empty pin, never a silent pass).

Run:  tools/opendss/.venv/Scripts/python tools/opendss/smoke.py
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[1]
REVISIONS_JSON = HERE / "revisions.json"
IEEE13 = (
    REPO_ROOT
    / "tests/corpus/electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss"
)

sys.path.insert(0, str(REPO_ROOT / "tools" / "oracle"))


def read_pin() -> dict[str, str]:
    pins: dict[str, str] = {}
    for line in (HERE / "PIN_OPENDSS.txt").read_text().splitlines():
        line = line.split("#", 1)[0].strip()
        if "==" in line:
            k, v = line.split("==", 1)
            pins[k.strip()] = v.strip()
    return pins


def check_pin() -> None:
    pins = read_pin()
    import dss
    import dss_python_backend

    if dss.__version__ != pins["dss-python"]:
        sys.exit(
            f"dss-python {dss.__version__} != pinned {pins['dss-python']} "
            "(tools/opendss/PIN_OPENDSS.txt)"
        )
    if dss_python_backend.__version__ != pins["dss-python-backend"]:
        sys.exit(
            f"dss-python-backend {dss_python_backend.__version__} != pinned "
            f"{pins['dss-python-backend']} (tools/opendss/PIN_OPENDSS.txt)"
        )


def run_rev(rev: str) -> None:
    check_pin()
    revs = json.loads(REVISIONS_JSON.read_text())
    spec = revs[rev]
    dll = (REPO_ROOT / spec["dll"]).resolve()
    if not dll.is_file():
        sys.exit(f"{rev}: DLL not vendored: {dll} (run tools/opendss/vendor_binaries.py)")
    if not IEEE13.is_file():
        sys.exit(f"IEEE13 master not found: {IEEE13}")

    from dss import IOddieDSS

    from oracle_server import _CorpusGuard  # noqa: E402  (tools/oracle on sys.path)

    cwd0 = os.getcwd()  # the EPRI engine chdirs the process on Compile
    d = IOddieDSS(library_path=str(dll))
    ver = str(d.Version)
    print(f"{rev}: engine Version = {ver!r}")

    d.AllowForms = False
    try:
        d.AllowEditor = False  # not implemented in Oddie -> DSSException, harmless
    except Exception:
        pass

    try:
        with _CorpusGuard(str(IEEE13)):
            d.Text.Command = "clear"
            d.Text.Command = f'Compile "{IEEE13}"'
            d.Text.Command = "solve"
            ckt = d.ActiveCircuit
            sol = ckt.Solution
            assert sol.Converged, f"{rev}: IEEE13 did not converge"
            n_nodes = len(ckt.YNodeOrder)
            print(f"{rev}: IEEE13 solved, {n_nodes} nodes, {sol.Iterations} iterations")

            # 4. factored-export neutrality: bit-identical voltages around the
            # CSC export (EPRI InitAndGetYparams always factors first).
            v0 = list(ckt.YNodeVarray)
            y = d.YMatrix.getYSparse()
            assert y is not None, f"{rev}: getYSparse returned None"
            data, row_idx, col_ptr = y
            assert len(col_ptr) - 1 == n_nodes, f"{rev}: CSC n={len(col_ptr) - 1} != {n_nodes}"
            assert len(data) > 0, f"{rev}: CSC export empty"
            v1 = list(ckt.YNodeVarray)
            assert v0 == v1, (
                f"{rev}: YNodeVarray CHANGED across getYSparse() — the factored "
                "export is NOT solution-neutral on this engine; use full_csc=false"
            )
            print(f"{rev}: getYSparse OK (n={n_nodes}, nnz={len(data)}), voltages bit-identical")

            # 5. injection vector shape.
            i_vec = list(d.YMatrix.getI())
            assert len(i_vec) == 2 * (n_nodes + 1), (
                f"{rev}: getI length {len(i_vec)} != {2 * (n_nodes + 1)}"
            )
            print(f"{rev}: getI OK (len={len(i_vec)})")
    finally:
        os.chdir(cwd0)

    expect = spec.get("expect_version", "")
    if not expect:
        print(f"{rev}: PIN THIS: expect_version -> {ver!r} (fill revisions.json and re-run)")
    elif expect not in ver:
        sys.exit(f"{rev}: engine {ver!r} does not contain pinned {expect!r}")
    else:
        print(f"{rev}: version pin OK ({expect!r})")
    print(f"{rev}: SMOKE OK")


def main() -> None:
    if "--rev" in sys.argv:
        run_rev(sys.argv[sys.argv.index("--rev") + 1])
        return
    revs = json.loads(REVISIONS_JSON.read_text())
    failed = []
    for rev in revs:
        print(f"=== {rev} ===", flush=True)
        r = subprocess.run([sys.executable, __file__, "--rev", rev])
        if r.returncode != 0:
            failed.append(rev)
    if failed:
        sys.exit(f"smoke FAILED for: {', '.join(failed)}")
    print("=== all revisions OK ===")


if __name__ == "__main__":
    main()
