r"""Bridge cross-validation: prove the in-house Rust `epri-worker` is byte-for-byte
faithful to the outgoing Python/Oddie r4133 path (UNIFIED_GATE_PLAN.md §2.4-2).

For every case in today's `corpus_live_opendss` r4133 universe, this drives BOTH:

  - the Python/Oddie r4133 engine, via `tools/oracle/oracle_server.py`
    (`DSS_ORACLE_ENGINE=oddie`, `DSS_OPENDSS_REV=r4133`, the Oddie venv), and
  - the Rust `epri-worker` (loads the SAME `bin/r4133/OpenDSSDirect.dll`),

with the IDENTICAL `run` request, and diffs the raw `CaseResult` JSON **bit-for-bit**
(parsed to Python objects, so dict key order is irrelevant and floats compare to
the exact f64 — both read the same engine memory, so any field mismatch is a
bridge bug to fix, never a diff to massage). Runs the whole universe twice; the
second pass is order-shuffled with a fixed seed to expose any residual worker
state carry-over.

This is a **temporary** tool committed for Phase A's fidelity gate; Phase E deletes
it (the Oddie stack retires once this diff is empty).

Universe = same as `corpus_live_opendss`: solvable_now + asymmetric/controls/modes,
excluding target-rev (`oracle` set), solve-abort, and pending family cases, and
excluding the r4133 `#303` crash decks identically on BOTH sides (from
`known_diffs.json` skip entries).

Usage:
    tools/opendss/.venv/Scripts/python tools/opendss/xcheck_bridge.py
      [--limit N] [--only SUBSTR] [--worker PATH] [--seed S]
"""

from __future__ import annotations

import argparse
import json
import os
import random
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
CORPUS = REPO / "tests" / "corpus"
ELECTRICDSS = CORPUS / "electricdss-tst"
MANIFESTS = CORPUS / "manifests"
ODDIE_PY = HERE / ".venv" / "Scripts" / "python.exe"
ORACLE_SERVER = REPO / "tools" / "oracle" / "oracle_server.py"


def winpath(p: Path) -> str:
    """Absolute Windows path with forward slashes (`E:/...`) — the form the DLL's
    `Compile` resolves (a bash-style `/e/...` is misread as a relative path)."""
    return str(p.resolve()).replace("\\", "/")


def load_manifest(path: Path) -> list[dict]:
    return json.loads(path.read_text()).get("cases", [])


def r4133_skip_substrings() -> list[str]:
    """`case_contains` of the `skip`-kind known_diffs entries that apply to r4133
    (the `#303` crash decks) — excluded identically on both sides."""
    kd = json.loads((CORPUS / "known_diffs.json").read_text())
    subs = []
    for e in kd["entries"]:
        if e.get("kind") == "skip" and "r4133" in e.get("revs", []):
            subs.append(e["case_contains"])
    return subs


def build_request(case: dict, abs_path: str) -> dict:
    """Byte-identical to `corpus_live.rs::Oracle::run_case`, with `all_properties`
    forced off (the r4133 channel never gates property parity)."""
    probes = [
        {"element": p["element"], "props": p.get("props", [])}
        for p in case.get("probes", [])
    ]
    return {
        "cmd": "run",
        "case_path": abs_path,
        "post": case.get("post", []),
        "n_steps": case.get("n_steps", 1),
        "selected_elements": case.get("selected_elements", []),
        "full_csc": True,
        "check_meters_monitors": case.get("check_meters_monitors", False),
        "probes": probes,
        "variables": case.get("compare_variables", []),
        "eventlog": case.get("compare_eventlog", False),
        "ctrlqueue": case.get("compare_ctrlqueue", False),
        "all_properties": False,
        "global_result": case.get("compare_global_result", False),
        "autoadd_log": case.get("compare_autoadd_log", False),
        "warn_and_continue": bool(case.get("expect_warnings")),
    }


def build_universe() -> list[tuple[str, str, dict]]:
    """(label, abs_case_path, case) triples — same membership as
    `corpus_live_opendss`."""
    skips = r4133_skip_substrings()

    def excluded_by_skip(label: str) -> bool:
        return any(s and s in label for s in skips)

    universe: list[tuple[str, str, dict]] = []
    # Vendored corpus (solvable_now).
    for c in load_manifest(MANIFESTS / "solvable_now.json"):
        if c.get("oracle") is not None or c.get("expect_solve_abort") is not None:
            continue
        label = f"solvable_now:{c['path']}"
        if excluded_by_skip(label):
            continue
        universe.append((label, winpath(ELECTRICDSS / c["path"]), c))
    # Synthetic families.
    for fam in ("asymmetric", "controls", "modes"):
        for c in load_manifest(CORPUS / fam / "manifest.json"):
            if (
                c.get("pending")
                or c.get("expect_solve_abort") is not None
                or c.get("oracle") is not None
            ):
                continue
            label = f"{fam}:{c['path']}"
            if excluded_by_skip(label):
                continue
            universe.append((label, winpath(CORPUS / fam / c["path"]), c))
    return universe


class Worker:
    """A persistent line-JSON worker (oracle_server or epri-worker)."""

    def __init__(self, name: str, argv: list[str], env: dict, log: Path):
        self.name = name
        self.log = open(log, "w", encoding="utf-8", errors="replace")
        self.proc = subprocess.Popen(
            argv,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=self.log,
            text=True,
            bufsize=1,
            env={**os.environ, **env},
            cwd=str(REPO),
        )

    def rpc(self, req: dict) -> dict:
        assert self.proc.stdin and self.proc.stdout
        self.proc.stdin.write(json.dumps(req, separators=(",", ":")) + "\n")
        self.proc.stdin.flush()
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError(f"{self.name}: worker died (see log)")
            line = line.strip()
            if not line:
                continue
            try:
                o = json.loads(line)
            except json.JSONDecodeError:
                continue  # stray non-JSON stdout is ignored, like the Rust client
            if isinstance(o, dict) and "ok" in o:
                return o

    def quit(self):
        try:
            assert self.proc.stdin
            self.proc.stdin.write('{"cmd":"quit"}\n')
            self.proc.stdin.flush()
            self.proc.wait(timeout=15)
        except Exception:
            self.proc.kill()
        self.log.close()


def diff_path(a, b, path="") -> str | None:
    """First structural difference between two JSON-parsed values, or None."""
    if type(a) is not type(b) and not (
        isinstance(a, (int, float)) and isinstance(b, (int, float))
    ):
        return f"{path}: type {type(a).__name__} vs {type(b).__name__}"
    if isinstance(a, dict):
        ka, kb = set(a), set(b)
        if ka != kb:
            return f"{path}: keys {sorted(ka - kb)} / {sorted(kb - ka)}"
        for k in a:
            d = diff_path(a[k], b[k], f"{path}.{k}")
            if d:
                return d
        return None
    if isinstance(a, list):
        if len(a) != len(b):
            return f"{path}: len {len(a)} vs {len(b)}"
        for i, (x, y) in enumerate(zip(a, b)):
            d = diff_path(x, y, f"{path}[{i}]")
            if d:
                return d
        return None
    if a != b:
        return f"{path}: {a!r} vs {b!r}"
    return None


def run_pass(oddie: Worker, epri: Worker, universe, tag: str):
    matched, diverged, ok_mismatch, both_err = [], [], [], []
    total = len(universe)
    for i, (label, abs_path, case) in enumerate(universe):
        req = build_request(case, abs_path)
        # Sequential per case so the two CorpusGuards never write the same dir
        # concurrently, and so the two processes never memory-map the same
        # loadshape file at once (the DDLL MMF reader access-violates (#303) in one
        # of them — a harness artifact; every case is bit-identical run standalone).
        # Each worker is `clear`ed right after its run, releasing any held MMF
        # handle before the other compiles the same case (or before the next pass).
        re = epri.rpc(req)
        epri.rpc({"cmd": "clear"})
        ro = oddie.rpc(req)
        oddie.rpc({"cmd": "clear"})
        if ro["ok"] and re["ok"]:
            d = diff_path(ro["result"], re["result"])
            if d is None:
                matched.append(label)
            else:
                diverged.append((label, d))
        elif not ro["ok"] and not re["ok"]:
            both_err.append((label, ro.get("error", ""), re.get("error", "")))
        else:
            ok_mismatch.append(
                (label, ro["ok"], ro.get("error", ""), re["ok"], re.get("error", ""))
            )
        if (i + 1) % 25 == 0:
            print(f"  [{tag}] {i + 1}/{total} ...", flush=True)
    return matched, diverged, ok_mismatch, both_err


def report(tag, matched, diverged, ok_mismatch, both_err) -> bool:
    print(f"\n=== pass {tag}: {len(matched)} matched of {len(matched) + len(diverged) + len(ok_mismatch) + len(both_err)} ===")
    for label, d in diverged:
        print(f"  DIVERGED {label}: {d}")
    for label, o_ok, o_e, e_ok, e_e in ok_mismatch:
        print(f"  OK-MISMATCH {label}: oddie ok={o_ok} ({o_e[:120]}) / epri ok={e_ok} ({e_e[:120]})")
    for label, o_e, e_e in both_err:
        print(f"  both-errored {label}: oddie=({o_e[:80]}) epri=({e_e[:80]})")
    clean = not diverged and not ok_mismatch
    return clean


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--limit", type=int, default=0, help="only the first N cases")
    ap.add_argument("--only", default="", help="only cases whose label contains this")
    ap.add_argument(
        "--worker",
        default=os.environ.get(
            "DSS_EPRI_WORKER", str(REPO / "target" / "debug" / "epri-worker.exe")
        ),
    )
    ap.add_argument("--seed", type=int, default=1337)
    args = ap.parse_args()

    if not ODDIE_PY.is_file():
        print(f"Oddie venv python missing: {ODDIE_PY}", file=sys.stderr)
        return 2
    if not Path(args.worker).is_file():
        print(f"epri-worker missing: {args.worker} (build it: cargo build -p dss-epri --bin epri-worker)", file=sys.stderr)
        return 2

    universe = build_universe()
    if args.only:
        universe = [u for u in universe if args.only in u[0]]
    if args.limit:
        universe = universe[: args.limit]
    print(f"xcheck universe: {len(universe)} case(s) (r4133)")

    tmp = REPO / "tmp"
    tmp.mkdir(exist_ok=True)
    oddie = Worker(
        "oddie",
        [str(ODDIE_PY), "-u", str(ORACLE_SERVER)],
        {"DSS_ORACLE_ENGINE": "oddie", "DSS_OPENDSS_REV": "r4133"},
        tmp / "xcheck_oddie.log",
    )
    epri = Worker("epri", [args.worker], {}, tmp / "xcheck_epri.log")
    try:
        # Ping both and assert the engine identity.
        po = oddie.rpc({"cmd": "ping"})
        pe = epri.rpc({"cmd": "ping"})
        assert po["ok"] and po["result"]["oracle"].get("oddie"), f"oddie ping: {po}"
        assert pe["ok"] and pe["result"]["oracle"].get("epri"), f"epri ping: {pe}"
        print(f"oddie engine: {po['result']['oracle'].get('engine')}")
        print(f"epri  engine: {pe['result']['oracle'].get('engine')}")

        # Pass 1: manifest order.
        r1 = run_pass(oddie, epri, universe, "1")
        clean1 = report("1 (manifest order)", *r1)

        # Pass 2: shuffled order (residual state-leak probe).
        shuffled = list(universe)
        random.Random(args.seed).shuffle(shuffled)
        r2 = run_pass(oddie, epri, shuffled, "2")
        clean2 = report(f"2 (shuffled, seed={args.seed})", *r2)
    finally:
        oddie.quit()
        epri.quit()

    ok = clean1 and clean2
    print(f"\nXCHECK {'PASS' if ok else 'FAIL'}: both passes {'bit-identical' if ok else 'have divergences'} over {len(universe)} cases")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
