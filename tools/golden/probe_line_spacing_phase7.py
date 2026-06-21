"""Probe the pinned oracle (dss-python 0.15.7 / backend 0.14.5) for WP7.1 step 3b
— the Line `spacing=`/`wires=`/`cncables=` path.

For each scenario it builds the SAME multi-conductor line two ways in the oracle
— once via `geometry=` (already pinned by step 3a) and once via `spacing=` +
`wires=`/`cncables=` — and prints both Rmatrix/Xmatrix/Cmatrix plus their max
abs difference. A ~0 difference confirms the spacing path reproduces the geometry
path in the oracle, so the Rust spacing port can pin against the step-3a
reference. Run:  python tools/golden/probe_line_spacing_phase7.py
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


def diff(a, b):
    return max((abs(x - y) for x, y in zip(a, b)), default=float("nan"))


def report(title, geo, spc):
    ng, Rg, Xg, Cg = matrices(geo)
    ns, Rs, Xs, Cs = matrices(spc)
    print(f"\n# === {title} ===  geom n={ng}, spacing n={ns}")
    print(f"#   maxdiff R={diff(Rg, Rs):.3e}  X={diff(Xg, Xs):.3e}  C={diff(Cg, Cs):.3e}")
    print(f"#   Z[0][0] geom=({Rg[0]:.12e},{Xg[0]:.12e})  spc=({Rs[0]:.12e},{Xs[0]:.12e})")
    print(f"#   C[0][0] geom={Cg[0]:.12e}  spc={Cs[0]:.12e}")


HEAD = [
    "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
    "set earthmodel=deri",
    "set frequency=60",
]
WIRE = [
    "new WireData.w Runits=m radunits=m gmrunits=m rac=0.0003 gmrac=0.005 radius=0.01",
]
CN = [
    "new CNData.cn1 Runits=m radunits=m gmrunits=m",
    "~ Rdc=1.0e-4 Rac=1.05e-4 GMRac=0.004 radius=0.005 capradius=0.005",
    "~ EpsR=2.3 InsLayer=0.004 DiaIns=0.022 DiaCable=0.030",
    "~ k=16 DiaStrand=0.001 GmrStrand=0.0004 Rstrand=2.0e-3",
]

# 1) Overhead: 3 wires, x=0/1/2 h=10 m.
report(
    "overhead spacing+wires",
    HEAD + WIRE + [
        "new LineGeometry.g nconds=3 nphases=3",
        "~ cond=1 wire=w x=0 h=10 units=m",
        "~ cond=2 wire=w x=1 h=10 units=m",
        "~ cond=3 wire=w x=2 h=10 units=m",
        "new Line.l1 bus1=src bus2=b geometry=g length=1 units=m phases=3",
    ],
    HEAD + WIRE + [
        "new LineSpacing.s nconds=3 nphases=3 x=[0 1 2] h=[10 10 10] units=m",
        "new Line.l1 bus1=src bus2=b spacing=s wires=[w w w] length=1 units=m",
    ],
)

# 2) CN underground: 3 cables, x=0/0.1/0.2 h=-1.2 m.
report(
    "CN spacing+cncables",
    HEAD + CN + [
        "new LineGeometry.g nconds=3 nphases=3 reduce=no",
        "~ cond=1 cncable=cn1 x=0   h=-1.2 units=m",
        "~ cond=2 cncable=cn1 x=0.1 h=-1.2 units=m",
        "~ cond=3 cncable=cn1 x=0.2 h=-1.2 units=m",
        "new Line.l1 bus1=src bus2=b geometry=g length=1 units=m phases=3",
    ],
    HEAD + CN + [
        "new LineSpacing.s nconds=3 nphases=3 x=[0 0.1 0.2] h=[-1.2 -1.2 -1.2] units=m",
        "new Line.l1 bus1=src bus2=b spacing=s cncables=[cn1 cn1 cn1] length=1 units=m",
    ],
)

# 3) Buried neutral: 3 CN phases + 1 bare overhead neutral (NWires=4, NPhases=3).
report(
    "CN + bare-neutral (buried)",
    HEAD + WIRE + CN + [
        "new LineGeometry.g nconds=4 nphases=3 reduce=yes",
        "~ cond=1 cncable=cn1 x=0   h=-1.2 units=m",
        "~ cond=2 cncable=cn1 x=0.1 h=-1.2 units=m",
        "~ cond=3 cncable=cn1 x=0.2 h=-1.2 units=m",
        "~ cond=4 wire=w     x=0.1 h=-1.0 units=m",
        "new Line.l1 bus1=src bus2=b geometry=g length=1 units=m phases=3",
    ],
    HEAD + WIRE + CN + [
        "new LineSpacing.s nconds=4 nphases=3 x=[0 0.1 0.2 0.1] h=[-1.2 -1.2 -1.2 -1.0] units=m",
        "new Line.l1 bus1=src bus2=b spacing=s cncables=[cn1 cn1 cn1] wires=[w] length=1 units=m",
    ],
)
