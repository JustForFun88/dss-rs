"""Probe the pinned oracle (dss-python 0.15.7 / backend 0.14.5) for Phase-7
WP7.1 line-constants reference matrices: CN cable, TS cable, a non-power-
frequency overhead case (radius branch), and a non-default rho_earth case.

Builds each geometry through a Line and reads RMatrix/XMatrix (ohm per unit
length, here ohm/m) and CMatrix (nF per unit length, here nF/m) — the same
engine outputs the Rust LineConstants engine produces. Run:
    python tools/golden/probe_line_constants.py

Earth-model keyword → code (DSSClass.pas:1074): Carson=1, FullCarson=2, Deri=3.
Note the Line `Cmatrix` getter scales reported nF by the *solve* frequency, so
capacitance is only meaningful at the power frequency (the f=5000 cases assert
Z only on the Rust side).
"""
from dss import dss


def matrices(decks, line="l1"):
    dss.Text.Command = "clear"
    for cmd in decks:
        dss.Text.Command = cmd
    dss.Text.Command = "solve"
    L = dss.ActiveCircuit.Lines
    L.Name = line
    n = L.Phases
    return n, list(L.Rmatrix), list(L.Xmatrix), list(L.Cmatrix)


def show(title, n, R, X, C):
    print(f"\n# === {title} (n={n}) ===")
    print("# Z (R+jX) ohm/m, row-major:")
    for i in range(n):
        for j in range(n):
            k = i * n + j
            print(f"    ({R[k]:.12e}, {X[k]:.12e}),")
    print("# C nF/m, row-major:")
    for i in range(n):
        for j in range(n):
            print(f"    {C[i*n+j]:.12e},")


# Shared CN core conductor + 3-cable geometry (all meters). `earthmodel`,
# `basefreq` and `reduce` are parameterized so the same geometry exercises every
# branch of TCNLineConstants.Calc.
def cn(earthmodel="deri", basefreq=60.0):
    return [
        f"new circuit.t basekv=12.47 phases=3 bus1=src basefreq={basefreq}",
        f"set earthmodel={earthmodel}",
        f"set frequency={basefreq}",
        "new CNData.cn1 Runits=m radunits=m gmrunits=m",
        "~ Rdc=1.0e-4 Rac=1.05e-4 GMRac=0.004 radius=0.005 capradius=0.005",
        "~ EpsR=2.3 InsLayer=0.004 DiaIns=0.022 DiaCable=0.030",
        "~ k=16 DiaStrand=0.001 GmrStrand=0.0004 Rstrand=2.0e-3",
        "new LineGeometry.g1 nconds=3 nphases=3 reduce=no",
        "~ cond=1 cncable=cn1 x=0    h=-1.2 units=m",
        "~ cond=2 cncable=cn1 x=0.1  h=-1.2 units=m",
        "~ cond=3 cncable=cn1 x=0.2  h=-1.2 units=m",
        "new Line.l1 bus1=src bus2=b geometry=g1 length=1 units=m phases=3",
    ]


def ts(earthmodel="deri", basefreq=60.0):
    return [
        f"new circuit.t basekv=12.47 phases=3 bus1=src basefreq={basefreq}",
        f"set earthmodel={earthmodel}",
        f"set frequency={basefreq}",
        "new TSData.ts1 Runits=m radunits=m gmrunits=m",
        "~ Rdc=1.0e-4 Rac=1.05e-4 GMRac=0.004 radius=0.005 capradius=0.005",
        "~ EpsR=2.3 InsLayer=0.004 DiaIns=0.022 DiaCable=0.030",
        "~ DiaShield=0.025 TapeLayer=0.0002 TapeLap=20.0",
        "new LineGeometry.g1 nconds=3 nphases=3 reduce=no",
        "~ cond=1 tscable=ts1 x=0    h=-1.2 units=m",
        "~ cond=2 tscable=ts1 x=0.1  h=-1.2 units=m",
        "~ cond=3 tscable=ts1 x=0.2  h=-1.2 units=m",
        "new Line.l1 bus1=src bus2=b geometry=g1 length=1 units=m phases=3",
    ]


# CN cable, 4 conductors (3 phases + 1 bare-neutral cable core at x=0.3) with
# Kron reduction to 3 phases — exercises the cable reduced-matrix path. The 4th
# CN cable contributes only its core (the CN self-Z / cap loops run 1..nphases),
# so it behaves as a bare neutral, matching the Rust new_cn(4)+set_nphases(3).
def cn_reduce():
    return [
        "new circuit.t basekv=12.47 phases=3 bus1=src",
        "set earthmodel=deri",
        "new CNData.cn1 Runits=m radunits=m gmrunits=m",
        "~ Rdc=1.0e-4 Rac=1.05e-4 GMRac=0.004 radius=0.005 capradius=0.005",
        "~ EpsR=2.3 InsLayer=0.004 DiaIns=0.022 DiaCable=0.030",
        "~ k=16 DiaStrand=0.001 GmrStrand=0.0004 Rstrand=2.0e-3",
        "new LineGeometry.g1 nconds=4 nphases=3 reduce=yes",
        "~ cond=1 cncable=cn1 x=0    h=-1.2 units=m",
        "~ cond=2 cncable=cn1 x=0.1  h=-1.2 units=m",
        "~ cond=3 cncable=cn1 x=0.2  h=-1.2 units=m",
        "~ cond=4 cncable=cn1 x=0.3  h=-1.2 units=m",
        "new Line.l1 bus1=src bus2=b geometry=g1 length=1 units=m phases=3",
    ]


# Overhead, 3 conductors at x=0/1/2 m, h=10 m, same SI wire as the existing
# tests (rac=3e-4, gmr=0.005, radius=0.01). High base frequency hits the
# non-power-frequency (radius) branch; rho=200 exercises a non-default earth.
def overhead(basefreq, rho):
    return [
        f"new circuit.t basekv=12.47 phases=3 bus1=src basefreq={basefreq}",
        "set earthmodel=deri",
        f"set frequency={basefreq}",
        "new WireData.w1 Runits=m radunits=m gmrunits=m",
        "~ Rdc=2.941176470588e-04 Rac=3.0e-4 GMRac=0.005 radius=0.01 capradius=0.01",
        "new LineGeometry.g1 nconds=3 nphases=3 reduce=no",
        "~ cond=1 wire=w1 x=0 h=10 units=m",
        "~ cond=2 wire=w1 x=1 h=10 units=m",
        "~ cond=3 wire=w1 x=2 h=10 units=m",
        f"new Line.l1 bus1=src bus2=b geometry=g1 length=1 units=m phases=3 rho={rho}",
    ]


show("CN cable, DERI, 3-phase", *matrices(cn()))
show("TS cable, DERI, 3-phase", *matrices(ts()))
show("CN cable, Carson (simple), 3-phase", *matrices(cn("carson")))
show("CN cable, FullCarson, 3-phase", *matrices(cn("fullcarson")))
show("TS cable, Carson (simple), 3-phase", *matrices(ts("carson")))
show("TS cable, FullCarson, 3-phase", *matrices(ts("fullcarson")))
show("CN cable, DERI, f=5000 (radius branch)", *matrices(cn(basefreq=5000.0)))
show("TS cable, DERI, f=5000 (radius branch)", *matrices(ts(basefreq=5000.0)))
show("CN cable, DERI, 4-cond reduced to 3", *matrices(cn_reduce()))
show("Overhead, DERI, f=5000 (radius branch)", *matrices(overhead(5000.0, 100.0)))
show("Overhead, DERI, 60 Hz, rho=200", *matrices(overhead(60.0, 200.0)))
