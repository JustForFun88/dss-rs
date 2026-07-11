"""A/B engine comparison over the vendored corpus — the upstream-change
inventory tool for porting planning.

Runs the same case list on TWO engines and diffs the oracle `CaseResult`s in
Python: which cases produce different voltages / iteration counts / element
powers / discrete control state between, say, OpenDSS r3723 and r4133 — i.e.
what upstream changed and what the port will have to follow.

Engine spec grammar (each engine = ONE persistent `oracle_server.py`
subprocess speaking the existing line-JSON protocol; separate processes
because the two dss-python versions cannot share a venv/process, and Oddie
wraps one DLL per process):

    capi           the pinned dss-python 0.15.7 oracle (tools/golden/PIN.txt)
    oddie:r4133    an EPRI OpenDSSDirect.dll revision (tools/opendss/revisions.json)
    oddie:@<path>  a direct DLL path (no version pin check)

Examples:
    tools/opendss/.venv/Scripts/python tools/opendss/ab_compare.py \
        --a oddie:r3723 --b oddie:r4088 --case 13Bus
    python tools/opendss/ab_compare.py --a capi --b oddie:r3723

The entry interpreter is irrelevant — each engine subprocess re-asserts its
own pin. `capi` uses --python-capi / $DSS_ORACLE_PYTHON (default "python");
`oddie:*` uses --python-oddie / $DSS_OPENDSS_PYTHON (default the
tools/opendss/.venv interpreter).

Engines run SEQUENTIALLY per case (A then B) so their `_CorpusGuard`s never
interleave on the same case directory.

Outputs: --out <json> (full metrics; default tmp/ab_<a>_vs_<b>.json) and
--md <markdown summary table> (default alongside). Tolerances are CLI flags —
this is a diff REPORT between two engines, not a calibrated gate; the Rust
gate's tolerance policy (tests/harness) is not involved.
"""

from __future__ import annotations

import argparse
import json
import os
import queue
import subprocess
import sys
import threading
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[1]
SERVER = REPO_ROOT / "tools" / "oracle" / "oracle_server.py"
CORPUS = REPO_ROOT / "tests" / "corpus" / "electricdss-tst"
MANIFESTS = REPO_ROOT / "tests" / "corpus" / "manifests"
DEFAULT_MANIFEST = MANIFESTS / "solvable_now.json"


# ---------------------------------------------------------------------------
# Engine subprocess (persistent, line-JSON).
# ---------------------------------------------------------------------------


class EngineProc:
    """One persistent oracle_server.py subprocess bound to one engine."""

    def __init__(self, spec: str, python_capi: str, python_oddie: str, timeout: float):
        self.spec = spec
        self.timeout = timeout
        self.env = dict(os.environ)
        if spec == "capi":
            self.python = python_capi
            self.env["DSS_ORACLE_ENGINE"] = "capi"
        elif spec.startswith("oddie:"):
            self.python = python_oddie
            self.env["DSS_ORACLE_ENGINE"] = "oddie"
            arg = spec[len("oddie:") :]
            if arg.startswith("@"):
                self.env["DSS_OPENDSS_DLL"] = str(Path(arg[1:]).resolve())
                self.env.pop("DSS_OPENDSS_REV", None)
            else:
                self.env["DSS_OPENDSS_REV"] = arg
                self.env.pop("DSS_OPENDSS_DLL", None)
        else:
            sys.exit(f"bad engine spec {spec!r} (capi | oddie:<rev> | oddie:@<dll>)")
        self.proc: subprocess.Popen | None = None
        self.q: queue.Queue[str | None] = queue.Queue()
        self.engine_info: dict = {}
        self._start()

    def _start(self) -> None:
        self.proc = subprocess.Popen(
            [self.python, "-u", str(SERVER)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=self.env,
            text=True,
            encoding="utf-8",
        )
        self.q = queue.Queue()
        threading.Thread(target=self._read_stdout, daemon=True).start()
        threading.Thread(target=self._read_stderr, daemon=True).start()
        pong = self.request({"cmd": "ping"})
        if not (pong and pong.get("ok")):
            sys.exit(f"[{self.spec}] ping failed: {pong}")
        self.engine_info = pong["result"].get("oracle", {})
        eng = self.engine_info if isinstance(self.engine_info, dict) else {}
        ver = eng.get("engine", self.engine_info)
        print(f"[{self.spec}] ready: {ver}", file=sys.stderr)

    def _read_stdout(self) -> None:
        proc = self.proc
        assert proc and proc.stdout
        for line in proc.stdout:
            self.q.put(line)
        self.q.put(None)  # EOF sentinel

    def _read_stderr(self) -> None:
        proc = self.proc
        assert proc and proc.stderr
        for line in proc.stderr:
            sys.stderr.write(f"[{self.spec}] {line}")

    def request(self, req: dict) -> dict | None:
        """One request/response with timeout; None on timeout/crash (the caller
        records the case as an engine error; the process is restarted)."""
        assert self.proc and self.proc.stdin
        try:
            self.proc.stdin.write(json.dumps(req, separators=(",", ":")) + "\n")
            self.proc.stdin.flush()
        except OSError:
            return None
        while True:
            try:
                line = self.q.get(timeout=self.timeout)
            except queue.Empty:
                return None
            if line is None:  # process died
                return None
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue  # stray engine output on stdout
            if isinstance(obj, dict) and "ok" in obj:
                return obj

    def run_case(self, req: dict) -> tuple[dict | None, str]:
        """Returns (result, error). Restarts the engine on crash/timeout."""
        resp = self.request(req)
        if resp is None:
            self.close()
            self._start()  # restart for the next case
            return None, "engine crash or timeout"
        if not resp.get("ok"):
            return None, str(resp.get("error"))
        return resp["result"], ""

    def close(self) -> None:
        if not self.proc:
            return
        try:
            if self.proc.stdin:
                self.proc.stdin.write('{"cmd":"quit"}\n')
                self.proc.stdin.flush()
            self.proc.wait(timeout=5)
        except Exception:
            self.proc.kill()
        self.proc = None


# ---------------------------------------------------------------------------
# Case list (same manifests the Rust gate reads).
# ---------------------------------------------------------------------------


def load_cases(manifest: Path, case_filter: str) -> list[tuple[str, str, dict]]:
    """[(label, abs_path, case_dict)] — `path` resolution mirrors corpus_live.rs:
    entries in tests/corpus/manifests/* are relative to the vendored corpus
    root; other manifests (asymmetric/, controls/) to the manifest's own dir."""
    data = json.loads(manifest.read_text())
    base = CORPUS if manifest.parent == MANIFESTS else manifest.parent
    out = []
    for c in data.get("cases", []):
        rel = c["path"]
        if case_filter and case_filter.lower() not in rel.lower():
            continue
        p = (base / rel).resolve()
        if not p.is_file():
            sys.exit(f"case missing on disk: {p} (from {manifest})")
        out.append((rel, str(p).replace("\\", "/"), c))
    return out


def build_request(abs_path: str, c: dict, full_csc: bool) -> dict:
    probes = [
        {"element": p["element"], "props": p.get("props") or []} for p in (c.get("probes") or [])
    ]
    return {
        "cmd": "run",
        "case_path": abs_path,
        "post": c.get("post") or [],
        "n_steps": int(c.get("n_steps", 1)),
        "selected_elements": c.get("selected_elements") or [],
        "full_csc": full_csc,
        "check_meters_monitors": bool(c.get("check_meters_monitors", False)),
        "probes": probes,
        "variables": c.get("compare_variables") or [],
        "eventlog": bool(c.get("compare_eventlog", False)),
        "ctrlqueue": bool(c.get("compare_ctrlqueue", False)),
    }


# ---------------------------------------------------------------------------
# Diffing.
# ---------------------------------------------------------------------------


def _rel(diff: float, ref: float, abs_floor: float) -> float:
    return diff / max(ref, abs_floor)


def diff_case(a: dict, b: dict, tol: argparse.Namespace) -> dict:
    """Compare two CaseResults; returns {status, first_divergence, steps:[...]}.
    Structural differences (node sets, element sets, step counts) are reported
    and numeric comparison proceeds over the intersection."""
    issues: list[str] = []
    metrics: dict = {}

    na, nb = a.get("node_order") or [], b.get("node_order") or []
    map_a = {n.lower(): i for i, n in enumerate(na)}
    map_b = {n.lower(): i for i, n in enumerate(nb)}
    only_a = sorted(set(map_a) - set(map_b))
    only_b = sorted(set(map_b) - set(map_a))
    if only_a or only_b:
        issues.append(f"node sets differ (only_a={only_a[:5]}, only_b={only_b[:5]})")
    common_nodes = sorted(set(map_a) & set(map_b))
    metrics["n_nodes"] = {"a": len(na), "b": len(nb)}

    cps_a, cps_b = a.get("checkpoints") or [], b.get("checkpoints") or []
    if len(cps_a) != len(cps_b):
        issues.append(f"checkpoint counts differ ({len(cps_a)} vs {len(cps_b)})")
    steps = []
    for si, (ca, cb) in enumerate(zip(cps_a, cps_b)):
        s: dict = {"step": si}
        step_issues: list[str] = []

        if bool(ca["converged"]) != bool(cb["converged"]):
            step_issues.append(f"converged {ca['converged']} vs {cb['converged']}")
        s["iterations"] = {"a": ca["iterations"], "b": cb["iterations"]}
        if ca["iterations"] != cb["iterations"]:
            step_issues.append(f"iterations {ca['iterations']} vs {cb['iterations']}")

        # Voltages over the common node set, aligned by node name.
        worst_v, worst_node = 0.0, ""
        for n in common_nodes:
            ia, ib = map_a[n], map_b[n]
            dre = ca["v_re"][ia] - cb["v_re"][ib]
            dim = ca["v_im"][ia] - cb["v_im"][ib]
            ref = (cb["v_re"][ib] ** 2 + cb["v_im"][ib] ** 2) ** 0.5
            r = _rel((dre * dre + dim * dim) ** 0.5, ref, tol.abs)
            if r > worst_v:
                worst_v, worst_node = r, n
        s["v_max_rel"] = worst_v
        s["v_worst_node"] = worst_node
        if worst_v > tol.v_rel:
            step_issues.append(f"V max rel {worst_v:.3e} @ {worst_node}")

        # Y fingerprint (assembled-model structural + magnitude guard).
        fa, fb = ca.get("y_fingerprint"), cb.get("y_fingerprint")
        if fa and fb:
            d_nnz = fa["nnz"] - fb["nnz"]
            d_frob = _rel(abs(fa["frob"] - fb["frob"]), abs(fb["frob"]), tol.abs)
            s["y_nnz_delta"] = d_nnz
            s["y_frob_rel"] = d_frob
            if d_nnz != 0:
                step_issues.append(f"Y nnz delta {d_nnz}")
            if d_frob > tol.v_rel:
                step_issues.append(f"Y frobenius rel {d_frob:.3e}")

        # Per-element currents/powers over the common element set.
        els_a = {e["name"].lower(): e for e in ca.get("elements") or []}
        els_b = {e["name"].lower(): e for e in cb.get("elements") or []}
        el_only_a = sorted(set(els_a) - set(els_b))
        el_only_b = sorted(set(els_b) - set(els_a))
        if el_only_a or el_only_b:
            step_issues.append(
                f"element sets differ (only_a={el_only_a[:5]}, only_b={el_only_b[:5]})"
            )
        offenders = []
        for nm in sorted(set(els_a) & set(els_b)):
            ea, eb = els_a[nm], els_b[nm]
            worst = 0.0
            for key, floor in (("currents", tol.abs), ("powers", tol.abs)):
                xa, xb = ea.get(key) or [], eb.get(key) or []
                if len(xa) != len(xb):
                    worst = float("inf")
                    break
                ref = max((abs(x) for x in xb), default=0.0)
                for va, vb in zip(xa, xb):
                    worst = max(worst, _rel(abs(va - vb), ref, floor))
            if worst > tol.i_rel:
                offenders.append((worst, nm))
        offenders.sort(reverse=True)
        s["element_offenders"] = [
            {"name": nm, "max_rel": (None if w == float("inf") else w)} for w, nm in offenders[:5]
        ]
        if offenders:
            step_issues.append(
                f"{len(offenders)} element(s) over tol, worst {offenders[0][1]} "
                f"rel {offenders[0][0]:.3e}"
            )

        # Discrete control state — exact.
        for key in ("transformers", "regcontrols", "capacitors"):
            da, db = ca.get(key) or {}, cb.get(key) or {}
            if da != db:
                changed = sorted(
                    k for k in set(da) | set(db) if da.get(k) != db.get(k)
                )
                step_issues.append(f"{key} differ: {changed[:5]}")
                s.setdefault("discrete_diffs", {})[key] = changed

        # Meters / monitors (present only when the case opts in).
        mts_a = {m["name"].lower(): m for m in ca.get("meters") or []}
        mts_b = {m["name"].lower(): m for m in cb.get("meters") or []}
        for nm in sorted(set(mts_a) & set(mts_b)):
            ra, rb = mts_a[nm]["register_values"], mts_b[nm]["register_values"]
            ref = max((abs(x) for x in rb), default=0.0)
            worst = max(
                (_rel(abs(va - vb), ref, tol.abs) for va, vb in zip(ra, rb)), default=0.0
            )
            if len(ra) != len(rb) or worst > tol.energy_rel:
                step_issues.append(f"meter {nm} registers rel {worst:.3e}")
        mons_a = {m["name"].lower(): m for m in ca.get("monitors") or []}
        mons_b = {m["name"].lower(): m for m in cb.get("monitors") or []}
        for nm in sorted(set(mons_a) & set(mons_b)):
            if mons_a[nm]["sample_count"] != mons_b[nm]["sample_count"]:
                step_issues.append(f"monitor {nm} sample counts differ")

        # Event log / control queue — count + first differing line.
        for key in ("eventlog", "ctrlqueue"):
            la, lb = ca.get(key) or [], cb.get(key) or []
            if la != lb:
                first = next(
                    (i for i, (x, y) in enumerate(zip(la, lb)) if x != y),
                    min(len(la), len(lb)),
                )
                step_issues.append(f"{key} differs ({len(la)} vs {len(lb)} lines, first at {first})")

        s["issues"] = step_issues
        steps.append(s)
        issues.extend(f"step {si}: {msg}" for msg in step_issues)

    return {
        "status": "match" if not issues else "diverged",
        "first_divergence": issues[0] if issues else "",
        "issues": issues,
        "metrics": metrics,
        "steps": steps,
    }


# ---------------------------------------------------------------------------
# Known-differences catalog (tests/corpus/known_diffs.json) — optional
# reclassification of divergences already triaged as legitimate engine
# differences (dss_capi-vs-EPRI). `kind: "skip"` entries mark cases not
# expected to run/converge on a participating revision: they are skipped up
# front (status `known_skipped`). Matching mirrors corpus_live_opendss:
# substring on (case path, issue text); `ab_contains` overrides
# `reason_contains` because this tool's issue wording differs from the Rust
# panics. Purely a report-level relabel — tolerances are untouched.
# ---------------------------------------------------------------------------


def load_known_diffs(path: Path) -> list[dict]:
    entries = json.loads(path.read_text())["entries"]
    for e in entries:
        if not str(e.get("cause", "")).strip():
            sys.exit(f"{path}: entry {e.get('id')!r} lacks a `cause` — "
                     "triage inventory, not a mute button")
        kind = e.get("kind", "diff")
        if kind == "skip":
            if not e.get("case_contains"):
                sys.exit(f"{path}: skip entry {e.get('id')!r} needs a non-empty "
                         "`case_contains` (it matches on the case alone)")
        elif kind != "diff":
            sys.exit(f"{path}: entry {e.get('id')!r} has unknown kind {kind!r}")
    return entries


def skip_hit(rel: str, entries: list[dict], revs: set[str]) -> dict | None:
    """First `skip` entry matching this case for a participating revision."""
    return next(
        (e for e in entries
         if e.get("kind", "diff") == "skip"
         and set(e["revs"]) & revs
         and e["case_contains"] in rel),
        None,
    )


def participating_revs(*specs: str) -> set[str]:
    return {s.split(":", 1)[1] for s in specs if s.startswith("oddie:r")}


def apply_known_diffs(rec: dict, entries: list[dict], revs: set[str]) -> None:
    """Relabel rec.status in place when every issue matches a catalog entry."""

    def matches(e: dict, text: str) -> bool:
        subs = e.get("ab_contains") or e.get("reason_contains") or []
        return (
            e.get("kind", "diff") == "diff"
            and bool(subs)
            and bool(set(e["revs"]) & revs)
            and e["case_contains"] in rec["path"]
            and all(s in text for s in subs)
        )

    if rec["status"] in ("error_a", "error_b"):
        hit = next(
            (e for e in entries if matches(e, rec["first_divergence"])), None
        )
        if hit:
            rec["status"] = "known_" + rec["status"]
            rec["known"] = [hit["id"]]
        return
    if rec["status"] != "diverged" or not rec.get("issues"):
        return
    ids = []
    for issue in rec["issues"]:
        hit = next((e for e in entries if matches(e, issue)), None)
        if hit is None:
            return  # at least one un-triaged issue -> stays "diverged"
        ids.append(hit["id"])
    rec["status"] = "known_diverged"
    rec["known"] = sorted(set(ids))


# ---------------------------------------------------------------------------
# Driver.
# ---------------------------------------------------------------------------


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--a", required=True, help="engine A: capi | oddie:<rev> | oddie:@<dll>")
    ap.add_argument("--b", required=True, help="engine B (the reference side of rel diffs)")
    ap.add_argument(
        "--manifest",
        action="append",
        type=Path,
        help=f"case manifest(s); default {DEFAULT_MANIFEST.relative_to(REPO_ROOT)}",
    )
    ap.add_argument("--case", default="", help="substring filter on case paths")
    ap.add_argument("--python-capi", default=os.environ.get("DSS_ORACLE_PYTHON", "python"))
    ap.add_argument(
        "--python-oddie",
        default=os.environ.get(
            "DSS_OPENDSS_PYTHON", str(HERE / ".venv" / "Scripts" / "python.exe")
        ),
    )
    ap.add_argument("--timeout", type=float, default=120.0, help="per-request seconds")
    ap.add_argument("--full-csc", action="store_true", help="also transfer the full CSC Y")
    ap.add_argument("--v-rel", dest="v_rel", type=float, default=1e-6)
    ap.add_argument("--i-rel", dest="i_rel", type=float, default=1e-5)
    ap.add_argument("--abs", dest="abs", type=float, default=1e-6)
    ap.add_argument("--energy-rel", dest="energy_rel", type=float, default=1e-4)
    ap.add_argument("--out", type=Path, default=None)
    ap.add_argument("--md", type=Path, default=None)
    ap.add_argument(
        "--known-diffs",
        type=Path,
        default=None,
        help="triage catalog (tests/corpus/known_diffs.json): cases whose every "
        "issue matches an entry are relabeled known_diverged; `skip` entries "
        "are not run at all (known_skipped)",
    )
    args = ap.parse_args()
    known_entries = load_known_diffs(args.known_diffs) if args.known_diffs else []
    known_revs = participating_revs(args.a, args.b)

    manifests = args.manifest or [DEFAULT_MANIFEST]
    cases: list[tuple[str, str, dict]] = []
    for m in manifests:
        cases.extend(load_cases(m.resolve(), args.case))
    if not cases:
        sys.exit("no cases matched")

    tag = f"{args.a}_vs_{args.b}".replace(":", "-").replace("@", "").replace("/", "_")
    out_path = args.out or REPO_ROOT / "tmp" / f"ab_{tag}.json"
    md_path = args.md or out_path.with_suffix(".md")
    out_path.parent.mkdir(parents=True, exist_ok=True)

    ea = EngineProc(args.a, args.python_capi, args.python_oddie, args.timeout)
    eb = EngineProc(args.b, args.python_capi, args.python_oddie, args.timeout)
    results = []
    try:
        for i, (rel, abs_path, c) in enumerate(cases):
            if known_entries and (hit := skip_hit(rel, known_entries, known_revs)):
                rec = {
                    "path": rel,
                    "status": "known_skipped",
                    "known": [hit["id"]],
                    "first_divergence": "",
                }
                results.append(rec)
                print(f"[{i + 1}/{len(cases)}] {rec['status']:9} {rel}",
                      file=sys.stderr)
                continue
            req = build_request(abs_path, c, args.full_csc)
            ra, err_a = ea.run_case(req)  # sequential: guards must not interleave
            rb, err_b = eb.run_case(req)
            if ra is None or rb is None:
                status = "error_a" if ra is None else "error_b"
                rec = {
                    "path": rel,
                    "status": status,
                    "first_divergence": (err_a if ra is None else err_b)[:400],
                }
            else:
                rec = {"path": rel, **diff_case(ra, rb, args)}
            if known_entries:
                apply_known_diffs(rec, known_entries, known_revs)
            results.append(rec)
            print(
                f"[{i + 1}/{len(cases)}] {rec['status']:9} {rel}"
                + (f" — {rec['first_divergence']}" if rec["first_divergence"] else ""),
                file=sys.stderr,
            )
    finally:
        ea.close()
        eb.close()

    counts: dict[str, int] = {}
    for r in results:
        counts[r["status"]] = counts.get(r["status"], 0) + 1
    report = {
        "a": {"spec": args.a, "engine": ea.engine_info},
        "b": {"spec": args.b, "engine": eb.engine_info},
        "tolerances": {
            "v_rel": args.v_rel,
            "i_rel": args.i_rel,
            "abs": args.abs,
            "energy_rel": args.energy_rel,
        },
        "counts": counts,
        "cases": results,
    }
    out_path.write_text(json.dumps(report, indent=1), newline="\n")

    lines = [
        f"# A/B engine diff: `{args.a}` vs `{args.b}`",
        "",
        f"- A: `{ea.engine_info}`",
        f"- B: `{eb.engine_info}`",
        f"- counts: `{counts}`",
        "",
        "| case | status | first divergence |",
        "|---|---|---|",
    ]
    for r in results:
        fd = str(r.get("first_divergence", "")).replace("|", "\\|")
        lines.append(f"| `{r['path']}` | {r['status']} | {fd} |")
    md_path.write_text("\n".join(lines) + "\n", newline="\n")
    print(f"report: {out_path}\nsummary: {md_path}", file=sys.stderr)
    ok = ("match",) if not known_entries else (
        "match", "known_diverged", "known_error_a", "known_error_b",
        "known_skipped",
    )
    sys.exit(0 if all(r["status"] in ok for r in results) else 3)


if __name__ == "__main__":
    main()
