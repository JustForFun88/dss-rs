"""Generate tests/golden/pstcalc/cmd_results.json from the pinned oracle (WP-PF.1).

Drives the pinned dss-python Text interface with a matrix of `Pstcalc` commands
(the IEC-868 flicker Pst calculator, Pascal `ExecHelper.DoPstCalc` over
`Shared/Pstcalc.pas`) and captures the `GlobalResult` (`Text.Result`) string each
one produces — the `Format('%.8g, ')` list of per-10-minute-interval Pst values.

The matrix spans:
  * npts in {12, 700, 1900} — 12 completes 0 intervals (empty result), 700 -> 1,
    1900 -> 3 (multi-interval);
  * two script-generated voltage shapes: a modulated sine and a repeated-step
    (square-wave) array;
  * lamp in {120, 230} (distinct bandpass gain constants);
  * freq in {60, 50} (the PstRMS FreqBase; note CyclesPerSample uses the *circuit*
    Solution.Frequency = 60, so freq=50 here tests the FreqBase/Tstep path while
    the circuit frequency drives dt->cycles);
  * a `dt` sweep (incl. dt=1 and a fractional dt exercising FPC banker's Round in
    CyclesPerSample = Round(Solution.Frequency * dt)).

Plus the npts<=10 guard error case (message #28723, upstream typo verbatim).

The Rust replay (`crates/dss-core/tests/golden_pstcalc.rs`) runs the identical
command list and compares each result BYTE-EXACT — a mismatched FPC `power`,
`Round`, `%.8g` or filter-cascade interpretation fails the byte compare.

Regenerate ONLY with the pinned versions (tools/golden/PIN.txt):
    python tools/golden/gen_pstcalc.py
"""
import json
import math
import pathlib

from dss import dss, DSSException

SCHEMA = 1


def sine(n: int, amp: float = 0.1, period: float = 25.0) -> list[float]:
    """Smooth 1.0-pu sinusoidal modulation."""
    return [1.0 + amp * math.sin(2.0 * math.pi * i / period) for i in range(n)]


def step(n: int, amp: float = 0.05, period: int = 50) -> list[float]:
    """Repeated up/down step (square wave) around 1.0 pu — sustained flicker."""
    return [1.0 + (amp if (i // period) % 2 == 0 else -amp) for i in range(n)]


SHAPES = {"sine": sine, "step": step}


def volt_str(vals: list[float]) -> str:
    """Bracketed comma list; fixed 6-decimal text so both engines parse the
    identical f64 tokens."""
    return "[" + ",".join(f"{v:.6f}" for v in vals) + "]"


def pst_command(npts: int, shape: str, dt, freq: int, lamp: int) -> str:
    vals = SHAPES[shape](npts)
    return f"Pstcalc npts={npts} voltages={volt_str(vals)} dt={dt} freq={freq} lamp={lamp}"


def build_cases():
    cases = []

    # Core cartesian at dt=1: npts x shape x lamp x freq.
    for npts in (12, 700):
        for shape in ("sine", "step"):
            for lamp in (120, 230):
                for freq in (60, 50):
                    name = f"{shape}_n{npts}_l{lamp}_f{freq}_dt1"
                    cases.append((name, pst_command(npts, shape, 1, freq, lamp)))

    # Multi-interval (npts=1900 -> 3 intervals) at freq=60, dt=1: shape x lamp.
    for shape in ("sine", "step"):
        for lamp in (120, 230):
            name = f"{shape}_n1900_l{lamp}_f60_dt1"
            cases.append((name, pst_command(1900, shape, 1, freq=60, lamp=lamp)))

    # dt sweep at npts=700, sine, lamp=120, freq=60 — exercises
    # CyclesPerSample = Round(Solution.Frequency * dt) incl. a fractional dt.
    for dt in (1.5, 2, 6, 10):
        name = f"sine_n700_l120_f60_dt{str(dt).replace('.', 'p')}"
        cases.append((name, pst_command(700, "sine", dt, freq=60, lamp=120)))

    # Error case: npts<=10 -> message #28723.
    cases.append(
        ("error_npts5", "Pstcalc npts=5 voltages=[1,2,3,4,5] dt=1 freq=60 lamp=120")
    )

    return cases


# The fixed circuit preamble; Solution.Frequency defaults to DefaultBaseFreq=60,
# which the dt->CyclesPerSample conversion reads (independent of the command's
# freq param). No solve is needed — Pstcalc reads only Solution.Frequency.
PREAMBLE = ["clear", "New Circuit.pstcalc basekv=12.47 phases=3 bus1=sourcebus"]


def run_case(name: str, command: str) -> dict:
    dss.AllowForms = False
    for cmd in PREAMBLE:
        dss.Text.Command = cmd
    error_message = None
    result = ""
    try:
        dss.Text.Command = command
        result = str(dss.Text.Result)
    except DSSException as e:
        error_message = str(e)
    return {
        "name": name,
        "commands": PREAMBLE + [command],
        "result": result,
        "error_message": error_message,
    }


def main():
    out = {"schema": SCHEMA, "cases": [run_case(n, c) for n, c in build_cases()]}
    path = (
        pathlib.Path(__file__).resolve().parents[2]
        / "tests"
        / "golden"
        / "pstcalc"
        / "cmd_results.json"
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(out, indent=2) + "\n")
    print(f"wrote {path} ({len(out['cases'])} cases)")


if __name__ == "__main__":
    main()
