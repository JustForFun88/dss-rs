"""Probe oracle reliability indices for a small OCP-protected feeder.

Builds the same 2-section radial feeder used by the Rust reliability tests, adds
one OCP device (recloser / relay / fuse), runs RelCalc, and prints the meter
SAIFI/SAIDI/SAIFIKW/CustInterrupts + section data. Run with the pinned oracle:
    python tools/golden/probe_reliability.py
"""
from dss import dss

BASE = """
clear
New circuit.t basekv=12.47 bus1=src phases=3
New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.2 pctperm=80 repair=4
New line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.3 pctperm=90 repair=5
New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10
New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25
{ocp}
New energymeter.m1 element=line.l1 terminal=1
Set voltagebases=[12.47]
CalcVoltageBases
Solve mode=snap
"""

OCP = {
    "recloser_l1": "New recloser.r1 monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l1 switchedterm=1",
    "relay_l1":    "New relay.r1 type=current monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l1 switchedterm=1 phasetrip=1 delay=0.1",
    "fuse_l1":     "New fuse.f1 monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l1 switchedterm=1",
    "recloser_l2": "New recloser.r1 monitoredobj=line.l2 monitoredterm=1 switchedobj=line.l2 switchedterm=1",
}


def run(ocp, restoration):
    dss.Text.Command = "clear"
    for line in BASE.format(ocp=ocp).strip().splitlines():
        dss.Text.Command = line
    M = dss.ActiveCircuit.Meters
    M.DoReliabilityCalc(restoration)
    M.First
    out = {
        "SAIFI": M.SAIFI, "SAIDI": M.SAIDI, "SAIFIKW": M.SAIFIKW,
        "CustInt": M.CustInterrupts, "NumSections": M.NumSections,
    }
    secs = []
    for s in range(1, M.NumSections + 1):
        M.SetActiveSection(s)
        secs.append((s, M.OCPDeviceType, M.SectSeqIdx, M.SectTotalCust))
    return out, secs


for name, ocp in OCP.items():
    for restoration in (False, True):
        out, secs = run(ocp, restoration)
        print(f"{name:13} restoration={str(restoration):5} {out}")
        for s in secs:
            print(f"               section {s[0]}: OCPType={s[1]} SeqIdx={s[2]} TotalCust={s[3]}")


# --- Restoration-divergent feeder: auto-reclosers on the head (l1) and a
# downstream line (l3), so AssumeRestoration No vs Yes give different
# SAIFI/SAIFIkW/CustInterrupts (the `assume_restoration and has_auto` reset
# branch). High pickups keep the snapshot solve from tripping the reclosers.
DIVERGENT = """
clear
New circuit.t basekv=12.47 bus1=src phases=3
New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.2 pctperm=80 repair=4
New line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.3 pctperm=90 repair=5
New line.l3 bus1=b2 bus2=b3 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.5 pctperm=100 repair=6
New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10
New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25
New load.ld3 bus1=b3 phases=3 kv=12.47 kw=150 numcust=7
New recloser.r1 monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l1 switchedterm=1 phasetrip=100000 groundtrip=100000
New recloser.r2 monitoredobj=line.l3 monitoredterm=1 switchedobj=line.l3 switchedterm=1 phasetrip=100000 groundtrip=100000
New energymeter.m1 element=line.l1 terminal=1
Set voltagebases=[12.47]
CalcVoltageBases
Solve mode=snap
"""
print("\n=== restoration-divergent (auto-reclosers on l1 + l3) ===")
for restoration in (False, True):
    dss.Text.Command = "clear"
    for line in DIVERGENT.strip().splitlines():
        dss.Text.Command = line
    M = dss.ActiveCircuit.Meters
    M.DoReliabilityCalc(restoration)
    M.First
    print(
        f"restoration={str(restoration):5} SAIFI={M.SAIFI!r} SAIDI={M.SAIDI!r} "
        f"SAIFIKW={M.SAIFIKW!r} CustInt={M.CustInterrupts!r} NumSections={M.NumSections}"
    )


# --- Two OCP controls switching one line: GetOCPDeviceType reports the first
# one defined (Pascal scans ControlElementList, stops at the first match).
TIEBREAK = """
clear
New circuit.t basekv=12.47 bus1=src phases=3
New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.2 pctperm=80 repair=4
New line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.3 pctperm=90 repair=5
New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10
New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25
{first}
{second}
New energymeter.m1 element=line.l1 terminal=1
Set voltagebases=[12.47]
CalcVoltageBases
Solve mode=snap
"""
FUSE = "New fuse.f1 monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l1 switchedterm=1"
REC = "New recloser.r1 monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l1 switchedterm=1 phasetrip=100000 groundtrip=100000"
print("\n=== two OCP controls on line.l1 (first-registered wins) ===")
for label, (a, b) in {"fuse_then_recloser": (FUSE, REC), "recloser_then_fuse": (REC, FUSE)}.items():
    dss.Text.Command = "clear"
    for line in TIEBREAK.format(first=a, second=b).strip().splitlines():
        dss.Text.Command = line
    M = dss.ActiveCircuit.Meters
    M.DoReliabilityCalc(False)
    M.First
    M.SetActiveSection(1)
    print(f"{label:20} OCPDeviceType={M.OCPDeviceType} (1=Fuse 2=Recloser 3=Relay)")
