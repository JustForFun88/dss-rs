"""Generate the Phase-7 WP7.1 targeted goldens (line-constants / geometry path)
from the pinned oracle.

Command-replay like the phase5/phase6 goldens, but each scenario ALSO pins the
**Line's YPrim block entry-by-entry** — the Carson Z/Yc produced by the
geometry/spacing/cable path is the new math under test (PHASE7_PLAN §1 focused
gate 1: "the Line YPrim from the geometry path matches entry-by-entry"). Unlike
the live corpus gate, this golden is committed, so it guards the geometry path
offline (no oracle install needed to catch a regression) and pins the exact
numbers in git.

The Rust harness (`golden_phase7.rs`) replays the identical command list, solves
once, and must match: converged flag + iteration count + node order exact, node
voltages 1e-6 rel, the Line YPrim entry-by-entry, and every element's terminal
currents/powers (the voltage-scaled power floor).

The geometry/spacing/cable definitions are the oracle-verified decks from
`probe_line_constants_phase7.py` / `probe_line_spacing_phase7.py` (the same wire /
CN data the Carson-engine unit tests pin), wrapped in a small solvable circuit
(source -> geometry line -> 3-phase load).

Scenarios (one file each under tests/golden/phase7/):
  - line_geometry: 3-phase overhead via `geometry=` (FetchGeometryCode /
    FMakeZFromGeometry), DERI earth, no reduce.
  - line_geometry_reduce: 3 phases + a neutral (nconds=4, reduce=yes) via
    `geometry=` — exercises the Kron reduce in the geometry path.
  - line_spacing: 3-phase overhead via `spacing=` + `wires=` (FetchLineSpacing /
    SetWires / FMakeZFromSpacing).
  - cable_cn: 3-phase concentric-neutral cable via `geometry=` + `cncable=`.
  - cable_ts: 3-phase tape-shield cable via `geometry=` + `tscable=` (the
    TSData / TapeShield arm — CN/TS parity, the path twice flagged TS-zero-cov).

Usage:
    python tools/golden/gen_phase7.py                 # regenerate all
    python tools/golden/gen_phase7.py line_spacing    # one scenario
Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_checkpoints import capture_element, capture_yprim, check_pin  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "phase7"
SCHEMA = 1

LINE = "Line.l1"

# A circuit + DERI earth; the line is created after `set earthmodel` so it
# captures FEarthModel=Deri (Line.pas:998 — the WP7.1 step-4 `Set EarthModel`
# path). Both engines default to Deri, but pinning it explicitly documents intent.
HEAD = [
    "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
    "set earthmodel=deri",
]
# SI overhead wire / neutral and CN cable — the oracle-verified probe data.
WIRE = "new WireData.w Runits=m radunits=m gmrunits=m rac=0.0003 gmrac=0.005 radius=0.01"
NEUTRAL = "new WireData.n Runits=m radunits=m gmrunits=m rac=0.0006 gmrac=0.003 radius=0.006"
CN = [
    "new CNData.cn1 Runits=m radunits=m gmrunits=m",
    "~ Rdc=1.0e-4 Rac=1.05e-4 GMRac=0.004 radius=0.005 capradius=0.005",
    "~ EpsR=2.3 InsLayer=0.004 DiaIns=0.022 DiaCable=0.030",
    "~ k=16 DiaStrand=0.001 GmrStrand=0.0004 Rstrand=2.0e-3",
]
TS = [
    "new TSData.ts1 Runits=m radunits=m gmrunits=m",
    "~ Rdc=1.0e-4 Rac=1.05e-4 GMRac=0.004 radius=0.005 capradius=0.005",
    "~ EpsR=2.3 InsLayer=0.004 DiaIns=0.022 DiaCable=0.030",
    "~ DiaShield=0.025 TapeLayer=0.0002 TapeLap=20.0",
]
LOAD = "new load.ld bus1=b phases=3 kv=12.47 kw=500 pf=0.95 model=1"
TAIL = ["set voltagebases=[12.47]", "calcvoltagebases"]


def deck_line_geometry() -> list[str]:
    return [
        *HEAD,
        WIRE,
        "new LineGeometry.g nconds=3 nphases=3 reduce=no",
        "~ cond=1 wire=w x=0 h=10 units=m",
        "~ cond=2 wire=w x=1 h=10 units=m",
        "~ cond=3 wire=w x=2 h=10 units=m",
        "new Line.l1 bus1=src bus2=b geometry=g length=1 units=km phases=3",
        LOAD,
        *TAIL,
    ]


def deck_line_geometry_reduce() -> list[str]:
    return [
        *HEAD,
        WIRE,
        NEUTRAL,
        "new LineGeometry.g nconds=4 nphases=3 reduce=yes",
        "~ cond=1 wire=w x=-0.5 h=10 units=m",
        "~ cond=2 wire=w x=0    h=10 units=m",
        "~ cond=3 wire=w x=0.5  h=10 units=m",
        "~ cond=4 wire=n x=0    h=8  units=m",
        "new Line.l1 bus1=src bus2=b geometry=g length=1 units=km phases=3",
        LOAD,
        *TAIL,
    ]


def deck_line_spacing() -> list[str]:
    return [
        *HEAD,
        WIRE,
        "new LineSpacing.s nconds=3 nphases=3 x=[0 1 2] h=[10 10 10] units=m",
        "new Line.l1 bus1=src bus2=b spacing=s wires=[w w w] length=1 units=km phases=3",
        LOAD,
        *TAIL,
    ]


def deck_cable_cn() -> list[str]:
    return [
        *HEAD,
        *CN,
        "new LineGeometry.g nconds=3 nphases=3 reduce=no",
        "~ cond=1 cncable=cn1 x=0   h=-1.2 units=m",
        "~ cond=2 cncable=cn1 x=0.1 h=-1.2 units=m",
        "~ cond=3 cncable=cn1 x=0.2 h=-1.2 units=m",
        "new Line.l1 bus1=src bus2=b geometry=g length=1 units=km phases=3",
        LOAD,
        *TAIL,
    ]


def deck_cable_ts() -> list[str]:
    return [
        *HEAD,
        *TS,
        "new LineGeometry.g nconds=3 nphases=3 reduce=no",
        "~ cond=1 tscable=ts1 x=0   h=-1.2 units=m",
        "~ cond=2 tscable=ts1 x=0.1 h=-1.2 units=m",
        "~ cond=3 tscable=ts1 x=0.2 h=-1.2 units=m",
        "new Line.l1 bus1=src bus2=b geometry=g length=1 units=km phases=3",
        LOAD,
        *TAIL,
    ]


# --- WP7.3 step 2: PVSystem (the inverter PC-element injection) --------------
# Source -> Line.l1 -> bus b; a PVSystem on b pushes power back to the source.
# The gate is the PVSystem terminal currents/powers + the node voltages (the new
# DoConstantPQPVsystemObj injection + the panel/inverter model); the Line.l1
# YPrim is the sym-component path (already ported) and is pinned incidentally.
PV_HEAD = [
    "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
    "new Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
]
PV_TAIL = ["set voltagebases=[12.47]", "calcvoltagebases"]


def deck_pvsystem_snapshot() -> list[str]:
    # Unity-PF, ideal inverter, full irradiance: kW_out = Pmpp, kvar_out = 0.
    return [
        *PV_HEAD,
        "new PVSystem.pv bus1=b phases=3 kV=12.47 kVA=500 Pmpp=500 pf=1.0 "
        "irradiance=1.0",
        *PV_TAIL,
    ]


def deck_pvsystem_curves() -> list[str]:
    # Derated panel power (irradiance 0.9 + temperature 35 via P-TCurve) through
    # an efficiency curve, at pf=0.95 (kvar output), with the kVA clamp live.
    return [
        *PV_HEAD,
        "new XYcurve.eff npts=4 xarray=(0.1 0.2 0.4 1.0) yarray=(0.86 0.9 0.93 0.97)",
        "new XYcurve.pt npts=4 xarray=(0 25 75 100) yarray=(1.2 1.0 0.8 0.6)",
        "new PVSystem.pv bus1=b phases=3 kV=12.47 kVA=500 Pmpp=500 pf=0.95 "
        "EffCurve=eff P-TCurve=pt irradiance=0.9 Temperature=35",
        *PV_TAIL,
    ]


# --- WP7.4 step 1: Storage (the battery PC element + SOC integration) -------
# Source -> Line.l1 -> bus b; a Storage on b discharges into / charges from the
# feeder. The gate is the Storage terminal currents/powers + node voltages (the
# DoConstantPQStorageObj injection + the state machine) AND the integrated SOC
# (`kWhStored`/`%Stored`/`State`) after the solve (the `storage` capture below).
STORE_HEAD = PV_HEAD
STORE_TAIL = PV_TAIL


def deck_storage_snapshot() -> list[str]:
    # Discharging at 50% of a 500 kW battery into the feeder (snapshot: the SOC
    # does not move — EndOfTimeStepCleanup runs only in time-series modes).
    return [
        *STORE_HEAD,
        "new Storage.s1 bus1=b phases=3 kV=12.47 kWrated=500 kWhrated=1000 "
        "state=discharging %discharge=50 pf=0.98",
        *STORE_TAIL,
    ]


def deck_storage_clamps() -> list[str]:
    # Four batteries pin the discrete inverter states via their terminal powers:
    #  - sa: discharging at rated (kW_out = +250) into the feeder;
    #  - sb: charging at 40% (kW_out = -200) absorbing from the feeder;
    #  - sc: idling (kW_out = -kWOutIdling, only the 1% idling loss);
    #  - sd: discharging at pf=0.8 with kVA at the limit -> the Q-priority kVA
    #    back-off (kvar stays 375, kW := sqrt(500^2 - 375^2) = 330.72), so the
    #    oracle pins which leg backs off (not just the apparent power).
    return [
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new Line.l1 bus1=src bus2=b  phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
        "new Line.lb bus1=src bus2=bb phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
        "new Line.lc bus1=src bus2=bc phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
        "new Line.ld bus1=src bus2=bd phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
        "new Storage.sa bus1=b  phases=3 kV=12.47 kWrated=500 kWhrated=1000 "
        "state=discharging %discharge=50 pf=1.0",
        "new Storage.sb bus1=bb phases=3 kV=12.47 kWrated=500 kWhrated=1000 "
        "%stored=50 state=charging %charge=40 pf=1.0",
        "new Storage.sc bus1=bc phases=3 kV=12.47 kWrated=500 kWhrated=1000 "
        "state=idling pf=1.0",
        "new Storage.sd bus1=bd phases=3 kV=12.47 kWrated=500 kVA=500 kWhrated=1000 "
        "state=discharging %discharge=100 pf=0.8",
        *STORE_TAIL,
    ]


def deck_storage_daily() -> list[str]:
    # A 100 kW / 200 kWh battery discharging at 50% (50 kW) with 10% reserve
    # (20 kWh). DCkW=50, idling loss=1 kW, DischargeEff=0.9 -> each 1-hour step
    # removes (50+1)/0.9 = 56.67 kWh; from 200 kWh it hits the 20 kWh reserve
    # partway through the 6-hour run and flips to Idling. Pins the integrated SOC
    # trajectory endpoint (`kWhStored`/`%Stored`/`State`) + the final-step powers.
    return [
        *STORE_HEAD,
        "new Storage.s1 bus1=b phases=3 kV=12.47 kWrated=100 kWhrated=200 "
        "state=discharging %discharge=50 %reserve=10 %IdlingkW=1 "
        "%EffDischarge=90 %EffCharge=90 pf=1.0",
        *STORE_TAIL,
        "set mode=daily number=6 stepsize=1h",
    ]


def deck_storage_daily_charge() -> list[str]:
    # The charge half of the SOC integration (audit-tests follow-up: the daily
    # discharge run covers depletion->reserve->idle; this covers the fill path).
    # A 100 kW / 200 kWh battery starting at 20% (40 kWh) charging at 80% (80 kW):
    # |DCkW|=80, idle=1 kW, ChargeEff=0.9 -> each 1-hour step adds (80-1)*0.9 =
    # 71.1 kWh; from 40 kWh it tops out at the 200 kWh rating partway through the
    # 4-hour run and flips to Idling (the kWhStored>kWhRating full-clamp).
    return [
        *STORE_HEAD,
        "new Storage.s1 bus1=b phases=3 kV=12.47 kWrated=100 kWhrated=200 "
        "%stored=20 state=charging %charge=80 %reserve=10 %IdlingkW=1 "
        "%EffDischarge=90 %EffCharge=90 pf=1.0",
        *STORE_TAIL,
        "set mode=daily number=4 stepsize=1h",
    ]


# --- WP7.4 step 2: StorageController (the fleet dispatch) -------------------
# Source -> Line.l1 -> bus b carrying a Load; a 2-battery Storage fleet on b is
# dispatched by a StorageController watching the Line.l1 terminal power. The gate
# is the controller-driven electrical model + SOC trajectory. The control loop
# exercises Sample/DoLoadFollowMode (PeakShave) each step.
def deck_storagecontroller_peakshave() -> list[str]:
    # PeakShave *snapshot* with the fleet actively dispatched: a 6 MW load on bus
    # b; the 2-battery fleet (each kWrated 2000) holds Line.l1 at the 2000 kW
    # target. Each battery's weighted share (½·(6000-2000) = 2000) equals its
    # kWrated cap, so both dispatch to exactly 2000 kW (Discharging) and the line
    # settles at the target — a deterministic active-dispatch endpoint. Pins the
    # converged electrical model (node voltages, the fleet's terminal
    # powers/currents, the discrete Discharging state) while the fleet is live.
    #
    # This snapshot converges bit-identically to the oracle: each control
    # iteration's SetNominalDEROutput invalidates the dispatched member's YPrim,
    # so the controller's Sample raises Solution.SystemYChanged and CheckControls
    # rebuilds Y with the discharging Yeq before the next solve — the fleet's new
    # admittance is in the system Y, not just its injection (Pascal CktElement.pas
    # l.245 / Solution.pas l.1155). Without that rebuild the stale idle YPrim left
    # the Norton model inconsistent and the converged point drifted ~1.8e-6.
    return [
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
        "new Load.ld bus1=b phases=3 kv=12.47 kw=6000 pf=1.0 model=1",
        "new Storage.sa bus1=b phases=3 kV=12.47 kWrated=2000 kVA=2000 kWhrated=4000 "
        "%stored=80 %idlingkW=0 pf=1.0",
        "new Storage.sb bus1=b phases=3 kV=12.47 kWrated=2000 kVA=2000 kWhrated=4000 "
        "%stored=80 %idlingkW=0 pf=1.0",
        "new StorageController.sc element=Line.l1 terminal=1 modedis=peakshave "
        "monphase=avg kwtarget=2000 %reserve=20",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "set maxcontroliter=50",
    ]


def deck_storagecontroller_daily() -> list[str]:
    # PeakShave daily run: the same fleet holding a constant 6 MW load below the
    # 4 MW target for 4 one-hour steps -> the batteries discharge each hour,
    # depleting toward the 20% reserve and flipping to Idling. Pins the
    # controller-driven SOC trajectory endpoint (`kWhStored`/`%Stored`/`State`)
    # plus the final-step electrical model.
    return [
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
        "new Load.ld bus1=b phases=3 kv=12.47 kw=6000 pf=1.0 model=1",
        "new Storage.sa bus1=b phases=3 kV=12.47 kWrated=2000 kVA=2000 kWhrated=3000 "
        "%stored=90 %idlingkW=0 pf=1.0",
        "new Storage.sb bus1=b phases=3 kV=12.47 kWrated=2000 kVA=2000 kWhrated=3000 "
        "%stored=90 %idlingkW=0 pf=1.0",
        "new StorageController.sc element=Line.l1 terminal=1 modedis=peakshave "
        "monphase=avg kwtarget=4000 %reserve=20",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "set maxcontroliter=50",
        "set mode=daily number=4 stepsize=1h",
    ]


def deck_pvsystem_clamps() -> list[str]:
    # Pins the three discrete ComputeInverterPower states the plan calls out
    # ("inverter control discrete state exact") via three PVSystems, oracle-pinned
    # through their terminal powers:
    #  - pva: varMode=KVAR, kvar=400, kVA=500, no priority -> non-priority kVA
    #    back-off (kw_out := sqrt(kVA^2 - kvar^2) = 300; kvar_out stays 400).
    #  - pvb: irradiance 0.1 -> panel 50 kW < CutOutkW (20%*500=100) -> inverter
    #    cuts OUT (kw_out = 0).
    #  - pvc: kvar=-400, kvarMaxAbs=300 -> absorption clamp (kvar_out := -300),
    #    then the negative-kvar kVA back-off (kw_out := sqrt(500^2-300^2) = 400).
    return [
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new Line.l1 bus1=src bus2=b  phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
        "new Line.lb bus1=src bus2=bb phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
        "new Line.lc bus1=src bus2=bc phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
        "new PVSystem.pva bus1=b  phases=3 kV=12.47 kVA=500 Pmpp=500 kvar=400 irradiance=1.0",
        "new PVSystem.pvb bus1=bb phases=3 kV=12.47 kVA=500 Pmpp=500 pf=1.0 irradiance=0.1",
        "new PVSystem.pvc bus1=bc phases=3 kV=12.47 kVA=500 Pmpp=500 kvar=-400 "
        "kvarMaxAbs=300 irradiance=1.0",
        *PV_TAIL,
    ]


# name -> deck builder. One file per entry under OUT_DIR; golden_phase7.rs runs
# every *.json in the directory, so this is the single source of truth.
SCENARIOS = {
    "line_geometry": deck_line_geometry,
    "line_geometry_reduce": deck_line_geometry_reduce,
    "line_spacing": deck_line_spacing,
    "cable_cn": deck_cable_cn,
    "cable_ts": deck_cable_ts,
    "pvsystem_snapshot": deck_pvsystem_snapshot,
    "pvsystem_curves": deck_pvsystem_curves,
    "pvsystem_clamps": deck_pvsystem_clamps,
    "storage_snapshot": deck_storage_snapshot,
    "storage_clamps": deck_storage_clamps,
    "storage_daily": deck_storage_daily,
    "storage_daily_charge": deck_storage_daily_charge,
    "storagecontroller_peakshave": deck_storagecontroller_peakshave,
    "storagecontroller_daily": deck_storagecontroller_daily,
}


def build(d, name: str, cmds: list[str]) -> dict:
    d.Text.Command = "clear"
    for c in cmds:
        d.Text.Command = c
    d.Text.Command = "solve"
    ckt = d.ActiveCircuit
    sol = ckt.Solution
    if not bool(sol.Converged):
        sys.exit(f"{name}: oracle did not converge — fix the deck")
    varray = list(ckt.YNodeVarray)
    elements = [capture_element(ckt, nm) for nm in ckt.AllElementNames]
    # The integrated state of charge of every Storage element after the solve
    # (Pascal `? Storage.<name>.<prop>`): pins the SOC trajectory endpoint of a
    # daily run and the static state of a snapshot. Read via the property query
    # so the Rust harness can replay it identically (`? ...` + result).
    storage = []
    for nm in ckt.AllElementNames:
        if not nm.lower().startswith("storage."):
            continue
        props = {}
        for p in ("kWhStored", "%Stored", "State"):
            d.Text.Command = f"? {nm}.{p}"
            props[p] = d.Text.Result
        storage.append({"name": nm, "properties": props})
    return {
        "name": name,
        "commands": cmds,
        "iterations": int(sol.Iterations),
        "converged": True,
        "node_order": list(ckt.YNodeOrder),
        "v_re": varray[0::2],
        "v_im": varray[1::2],
        "line_yprim": capture_yprim(ckt, LINE),
        "elements": elements,
        "storage": storage,
    }


def main() -> None:
    oracle = check_pin()
    from dss import DSS as d

    wanted = set(sys.argv[1:])
    unknown = wanted - set(SCENARIOS)
    if unknown:
        sys.exit(f"unknown scenario(s): {sorted(unknown)}; known: {sorted(SCENARIOS)}")
    names = [n for n in SCENARIOS if not wanted or n in wanted]

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for name in names:
        sc = build(d, name, SCENARIOS[name]())
        path = OUT_DIR / f"{name}.json"
        path.write_text(
            json.dumps({"schema": SCHEMA, "oracle": oracle, "scenario": sc}, indent=1) + "\n"
        )
        print(f"wrote {path.relative_to(REPO_ROOT)}")

    # On a full regen, drop any stale scenario files no longer in the registry.
    if not wanted:
        for p in OUT_DIR.glob("*.json"):
            if p.stem not in SCENARIOS:
                p.unlink()
                print(f"removed stale {p.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
