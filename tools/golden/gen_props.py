"""Generate the Phase 2 property round-trip golden from dss-python (oracle).

For each scenario we run a short command script through the oracle, then read
back every property of the target object via the `?` query. The Rust engine
must replay the identical scripts and produce the same property values
(numbers compared with tolerance, structure exactly — see PORTING_PLAN.md §4).

Only fully-specified or all-default states are probed: arrays sized by `Npts`
but never filled contain uninitialized heap garbage in the oracle (genuine UB,
non-deterministic), so those states are deliberately excluded.

Usage:
    python tools/golden/gen_props.py

Reads  nothing (scenarios are defined below)
Writes one file per class under tests/golden/props/ (<class>.json), where the
class is the scenario-name prefix before the first `_` (e.g. all `loadshape_*`
scenarios -> props/loadshape.json). props_roundtrip.rs runs every file in the
directory, so a class's diff stays isolated to its own file.

Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

# Matches one token to zero out during garbage canonicalization: a numeric
# (int/float/scientific) value, or the non-finite `Nan`/`Inf` the oracle's
# uninitialized-memory matrix getter can emit (e.g. Fault.GMatrix at 3 phases).
_NUM_RE = re.compile(r"(?i:nan|[-+]?inf)|[-+]?\d*\.?\d+(?:[eE][-+]?\d+)?")

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "props"
SCHEMA = 1

# Each scenario: a name, the command script (run after `clear; new circuit`),
# and the target object whose properties are read back.
SCENARIOS = [
    {
        "name": "tcc_default",
        "target": "TCC_Curve.tcc1",
        "commands": ["New TCC_Curve.tcc1"],
    },
    {
        "name": "tcc_full",
        "target": "TCC_Curve.tcc1",
        "commands": ["New TCC_Curve.tcc1 npts=3 C_array=(1 2 3) T_array=(0.1 0.2 0.3)"],
    },
    {
        "name": "tcc_abbrev",
        "target": "TCC_Curve.tcc1",
        "commands": ["New TCC_Curve.tcc1 np=2 c=(10 20) t=(1 2)"],
    },
    {
        "name": "tcc_edit_then_more",
        "target": "TCC_Curve.tcc1",
        "commands": [
            "New TCC_Curve.tcc1 npts=2",
            "Edit TCC_Curve.tcc1 C_array=(5 6)",
            "~ T_array=(9 8)",
        ],
    },
    {
        "name": "tcc_shrink_npts",
        "target": "TCC_Curve.tcc1",
        "commands": [
            "New TCC_Curve.tcc1 npts=3 C_array=(1 2 3) T_array=(7 8 9)",
            "Edit TCC_Curve.tcc1 npts=2",
        ],
    },
    {
        "name": "tcc_makelike",
        "target": "TCC_Curve.tcc1",
        "commands": [
            "New TCC_Curve.base npts=2 C_array=(1 2) T_array=(3 4)",
            "New TCC_Curve.tcc1 like=base",
        ],
    },
    {
        "name": "spectrum_default",
        "target": "Spectrum.s1",
        "commands": ["New Spectrum.s1"],
    },
    {
        "name": "spectrum_full",
        "target": "Spectrum.s1",
        "commands": [
            "New Spectrum.s1 NumHarm=3 harmonic=(1 5 7) %mag=(100 20 14.3) angle=(0 0 0)",
        ],
    },
    {
        "name": "spectrum_abbrev",
        "target": "Spectrum.s1",
        "commands": ["New Spectrum.s1 numh=2 harm=(1 3) %m=(100 33) ang=(0 180)"],
    },
    {
        "name": "spectrum_makelike",
        "target": "Spectrum.s1",
        "commands": [
            "New Spectrum.base NumHarm=2 harmonic=(1 3) %mag=(100 33) angle=(0 180)",
            "New Spectrum.s1 like=base",
        ],
    },
    {
        "name": "linecode_default",
        "target": "LineCode.lc1",
        "commands": ["New LineCode.lc1"],
    },
    {
        "name": "linecode_sym",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.lc1 nphases=3 r1=0.1 x1=0.3 r0=0.2 x0=0.6 "
            "c1=3.0 c0=1.5 normamps=500 emergamps=700 units=kft linetype=ug",
        ],
    },
    {
        "name": "linecode_sym_b",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.lc1 nphases=1 r1=0.05 x1=0.1 b1=2.0 b0=1.0 units=mi",
        ],
    },
    {
        "name": "linecode_matrix",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.lc1 nphases=3 "
            "rmatrix=(0.09 | 0.04 0.09 | 0.04 0.04 0.09) "
            "xmatrix=(0.2 | 0.09 0.2 | 0.09 0.09 0.2) "
            "cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8)",
        ],
    },
    {
        "name": "linecode_code_then_units",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.lc1 nphases=2 rmatrix=(0.1 | 0.05 0.1) "
            "xmatrix=(0.2 | 0.07 0.2) cmatrix=(3 | -1 3) units=mi",
        ],
    },
    {
        "name": "linecode_kron",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.lc1 nphases=4 "
            "rmatrix=(0.1 | 0.04 0.1 | 0.04 0.04 0.1 | 0.04 0.04 0.04 0.1) "
            "xmatrix=(0.2 | 0.09 0.2 | 0.09 0.09 0.2 | 0.09 0.09 0.09 0.2) "
            "cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8 | -0.6 -0.6 -0.6 2.8) "
            "kron=y",
        ],
    },
    {
        "name": "linecode_seasons",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.lc1 nphases=3 seasons=3 ratings=(400 500 600)",
        ],
    },
    {
        "name": "linecode_makelike",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.base nphases=2 r1=0.2 x1=0.4 r0=0.3 x0=0.7 c1=2.5 c0=1.2",
            "New LineCode.lc1 like=base",
        ],
    },
    # --- Line + LineCode fetch (WP4.2) ---
    {
        "name": "line_code_sym",
        "target": "Line.l1",
        "commands": [
            "New LineCode.mtx601 nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 "
            "c1=3 c0=1 units=mi normamps=500 emergamps=700",
            "New Line.l1 bus1=a bus2=b linecode=mtx601 length=2000 units=ft",
        ],
    },
    {
        "name": "line_code_then_units",
        "target": "Line.l1",
        "commands": [
            "New LineCode.mtx601 nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 "
            "c1=3 c0=1 units=mi",
            "New Line.l1 bus1=a bus2=b linecode=mtx601 units=ft length=2000",
        ],
    },
    {
        "name": "line_code_matrix",
        "target": "Line.l1",
        "commands": [
            "New LineCode.mx nphases=2 rmatrix=(0.1 | 0.05 0.1) "
            "xmatrix=(0.2 | 0.07 0.2) cmatrix=(3 | -1 3) units=mi",
            "New Line.l1 bus1=a.1.2 bus2=b.1.2 linecode=mx length=1 units=mi",
        ],
    },
    {
        "name": "line_code_then_r1",
        "target": "Line.l1",
        "commands": [
            "New LineCode.mtx601 nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 units=mi",
            "New Line.l1 bus1=a bus2=b linecode=mtx601 r1=0.5 length=1 units=mi",
        ],
    },
    # --- GrowthShape (WP4.3) ---
    {
        "name": "growthshape_default",
        "target": "GrowthShape.gs1",
        "commands": ["New GrowthShape.gs1"],
    },
    {
        "name": "growthshape_full",
        "target": "GrowthShape.gs1",
        "commands": [
            "New GrowthShape.gs1 npts=5 "
            "year=(1999 2000 2001 2005 2010) "
            "mult=(1.10 1.07 1.05 1.025 1.01)",
        ],
    },
    {
        "name": "growthshape_year_rounds",
        "target": "GrowthShape.gs1",
        "commands": [
            "New GrowthShape.gs1 npts=3 year=(2000.4 2001.6 2002.5) mult=(1.05 1.04 1.03)",
        ],
    },
    {
        "name": "growthshape_edit_shrink",
        "target": "GrowthShape.gs1",
        "commands": [
            "New GrowthShape.gs1 npts=4 year=(2000 2001 2002 2003) mult=(1.05 1.04 1.03 1.02)",
            "Edit GrowthShape.gs1 npts=2",
        ],
    },
    {
        "name": "growthshape_makelike",
        "target": "GrowthShape.gs1",
        "commands": [
            "New GrowthShape.base npts=2 year=(2000 2010) mult=(1.05 1.02)",
            "New GrowthShape.gs1 like=base",
        ],
    },
    # --- XYcurve (WP5.1) ---
    # NOTE: `points=` triggers an access violation in the pinned oracle (a
    # dss_capi `DoubleDArrayProperty` bug), so the arrays are driven through
    # `xarray`/`yarray`; the `Points` getter is still validated by readback.
    {
        "name": "xycurve_default",
        "target": "XYcurve.c1",
        "commands": ["New XYcurve.c1"],
    },
    {
        "name": "xycurve_arrays",
        "target": "XYcurve.c1",
        "commands": ["New XYcurve.c1 npts=4 yarray=(10 20 30 40) xarray=(1 2 3 4)"],
    },
    {
        "name": "xycurve_abbrev",
        "target": "XYcurve.c1",
        "commands": ["New XYcurve.c1 np=3 yarr=(0 5 10) xarr=(0 1 2)"],
    },
    {
        "name": "xycurve_shift_scale",
        "target": "XYcurve.c1",
        "commands": [
            "New XYcurve.c1 npts=3 xarray=(0 1 2) yarray=(0 10 20) "
            "xscale=2 yscale=3 xshift=1 yshift=5",
        ],
    },
    {
        "name": "xycurve_x_accessor",
        "target": "XYcurve.c1",
        "commands": [
            "New XYcurve.c1 npts=4 xarray=(1 2 3 4) yarray=(10 20 30 40)",
            "Edit XYcurve.c1 x=2.5",
        ],
    },
    {
        "name": "xycurve_shrink_npts",
        "target": "XYcurve.c1",
        "commands": [
            "New XYcurve.c1 npts=4 xarray=(1 2 3 4) yarray=(10 20 30 40)",
            "Edit XYcurve.c1 npts=2",
        ],
    },
    {
        "name": "xycurve_makelike",
        "target": "XYcurve.c1",
        "commands": [
            "New XYcurve.base npts=3 xarray=(0 1 2) yarray=(5 7 9)",
            "New XYcurve.c1 like=base",
        ],
    },
    # --- LoadShape (WP5.2a, in-memory core) ---
    # File props (CSVFile/SngFile/DblFile/PQCSVFile) are all ported (WP5.2b /
    # WPG.1) but none of these scenarios actually loads a file — every default
    # here stays the empty string, so no file-fixture scenario is needed on
    # this round-trip gate (the executive integration tests + the live
    # shape_binfiles corpus deck cover an actual load). Mean/StdDev on the
    # empty default raise (61107) in the oracle, so they are skipped for that
    # one scenario only.
    {
        "name": "loadshape_default",
        "target": "LoadShape.d",
        "commands": ["New LoadShape.d"],
        "skip_props": ["Mean", "StdDev"],
    },
    {
        "name": "loadshape_fixed",
        "target": "LoadShape.d",
        "commands": ["New LoadShape.d npts=4 interval=1 mult=(1 2 4 8)"],
    },
    {
        "name": "loadshape_abbrev",
        "target": "LoadShape.d",
        "commands": ["New LoadShape.d np=3 int=1 pmult=(0.5 0.9 1.0)"],
    },
    {
        "name": "loadshape_pq",
        "target": "LoadShape.d",
        "commands": [
            "New LoadShape.d npts=4 interval=1 mult=(1 2 4 8) qmult=(.5 .6 .7 .8)",
        ],
    },
    {
        "name": "loadshape_sinterval",
        "target": "LoadShape.d",
        "commands": ["New LoadShape.d npts=4 sinterval=900 mult=(1 2 4 8)"],
    },
    {
        "name": "loadshape_minterval_useactual",
        "target": "LoadShape.d",
        "commands": ["New LoadShape.d npts=4 minterval=15 mult=(1 2 4 8) useactual=yes"],
    },
    {
        "name": "loadshape_hour_array",
        "target": "LoadShape.d",
        "commands": ["New LoadShape.d npts=3 interval=0 hour=(1 2 4) mult=(1 2 4)"],
    },
    {
        "name": "loadshape_normalize",
        "target": "LoadShape.d",
        "commands": ["New LoadShape.d npts=4 interval=1 mult=(2 4 6 8) action=normalize"],
    },
    {
        "name": "loadshape_normalize_pbase",
        "target": "LoadShape.d",
        "commands": [
            "New LoadShape.d npts=4 interval=1 mult=(2 4 6 8) pbase=10 action=normalize",
        ],
    },
    {
        "name": "loadshape_interp_edge",
        "target": "LoadShape.d",
        "commands": [
            "New LoadShape.d npts=5 interval=1 mult=(0.2 0.4 1.0 0.7 0.3) interpolation=edge",
        ],
    },
    {
        "name": "loadshape_mean_stddev",
        "target": "LoadShape.d",
        "commands": ["New LoadShape.d npts=4 interval=1 mult=(2 4 6 8) mean=5 stddev=2"],
    },
    {
        "name": "loadshape_makelike",
        "target": "LoadShape.d",
        "commands": [
            "New LoadShape.base npts=3 interval=2 mult=(1 2 3) qmult=(4 5 6) "
            "pbase=7 useactual=yes",
            "New LoadShape.d like=base",
        ],
    },
    # --- TShape / TempShape (WP5.2c). Legacy scalar shape; CSVFile is exercised
    # by the executive integration test, not here. Empty Mean/StdDev return 0
    # (no error, unlike LoadShape), so no skip_props is needed.
    {
        "name": "tshape_default",
        "target": "TShape.t",
        "commands": ["New TShape.t"],
    },
    {
        "name": "tshape_fixed",
        "target": "TShape.t",
        "commands": ["New TShape.t npts=4 interval=1 temp=(20 30 50 80)"],
    },
    {
        "name": "tshape_sinterval",
        "target": "TShape.t",
        "commands": ["New TShape.t npts=4 sinterval=900 temp=(1 2 4 8)"],
    },
    {
        "name": "tshape_minterval",
        "target": "TShape.t",
        "commands": ["New TShape.t npts=4 minterval=15 temp=(1 2 4 8)"],
    },
    {
        "name": "tshape_hour_interval0",
        "target": "TShape.t",
        "commands": ["New TShape.t npts=3 interval=0 hour=(1 2 4) temp=(1 2 4)"],
    },
    # Distinguishing case: TempShape (unlike PriceShape) does NOT auto-zero
    # Interval when Hour is given, so this stays a fixed-interval curve.
    {
        "name": "tshape_hour_no_interval",
        "target": "TShape.t",
        "commands": ["New TShape.t npts=3 hour=(1 2 4) temp=(1 2 4)"],
    },
    {
        "name": "tshape_mean_stddev",
        "target": "TShape.t",
        "commands": ["New TShape.t npts=4 interval=1 temp=(20 30 50 80) mean=5 stddev=2"],
    },
    {
        "name": "tshape_makelike",
        "target": "TShape.t",
        "commands": [
            "New TShape.base npts=3 interval=2 temp=(11 22 33)",
            "New TShape.t like=base",
        ],
    },
    # --- PriceShape (WP5.2c). Same skeleton; Hour auto-zeroes Interval.
    {
        "name": "priceshape_default",
        "target": "PriceShape.d",
        "commands": ["New PriceShape.d"],
    },
    {
        "name": "priceshape_fixed",
        "target": "PriceShape.d",
        "commands": ["New PriceShape.d npts=4 interval=1 price=(2 4 6 8)"],
    },
    {
        "name": "priceshape_sinterval",
        "target": "PriceShape.d",
        "commands": ["New PriceShape.d npts=4 sinterval=900 price=(1 2 4 8)"],
    },
    # Hour given without interval=0: PriceShape auto-sets a variable interval.
    {
        "name": "priceshape_hour",
        "target": "PriceShape.d",
        "commands": ["New PriceShape.d npts=3 hour=(1 2 4) price=(1 2 4)"],
    },
    {
        "name": "priceshape_mean_stddev",
        "target": "PriceShape.d",
        "commands": ["New PriceShape.d npts=4 interval=1 price=(2 4 6 8) mean=5 stddev=2"],
    },
    {
        "name": "priceshape_makelike",
        "target": "PriceShape.d",
        "commands": [
            "New PriceShape.base npts=3 interval=2 price=(11 22 33)",
            "New PriceShape.d like=base",
        ],
    },
    {
        "name": "xfmrcode_default",
        "target": "XfmrCode.xc1",
        "commands": ["New XfmrCode.xc1"],
    },
    {
        "name": "xfmrcode_full",
        "target": "XfmrCode.xc1",
        "commands": [
            "New XfmrCode.xc1 phases=3 windings=3 "
            "conns=(delta, wye, wye) kvs=(115, 12.47, 4.16) kvas=(5000, 5000, 5000) "
            "xhl=8 xht=10 xlt=9 %loadloss=0.5 %noloadloss=0.2 %imag=0.1 "
            "thermal=2 n=0.8 m=0.8 flrise=65 hsrise=15 ppm=2",
        ],
    },
    {
        "name": "xfmrcode_wdg_seq",
        "target": "XfmrCode.xc1",
        "commands": [
            "New XfmrCode.xc1 windings=2 phases=1",
            "~ wdg=1 conn=wye kv=7.2 kva=25 %r=1.2 tap=1.05 rneut=0.1 xneut=0.2 "
            "maxtap=1.1 mintap=0.9 numtaps=32 rdcohms=0.5",
            "~ wdg=2 conn=wye kv=0.24 kva=25 %r=1.2",
        ],
    },
    {
        "name": "xfmrcode_xscarray",
        "target": "XfmrCode.xc1",
        "commands": ["New XfmrCode.xc1 windings=3 xscarray=(8 10 9)"],
    },
    {
        "name": "xfmrcode_ratings",
        "target": "XfmrCode.xc1",
        "commands": ["New XfmrCode.xc1 seasons=3 ratings=(600, 700, 800)"],
    },
    {
        "name": "xfmrcode_makelike",
        "target": "XfmrCode.xc1",
        "commands": [
            "New XfmrCode.base windings=2 kvs=(115, 4.16) kvas=(3000, 3000) xhl=7",
            "New XfmrCode.xc1 like=base",
        ],
    },
    # --- Transformer (WP4.4) ---
    {
        "name": "transformer_default",
        "target": "Transformer.t1",
        "commands": ["New Transformer.t1"],
    },
    {
        "name": "transformer_sub",
        "target": "Transformer.sub",
        "commands": [
            "New Transformer.sub phases=3 windings=2 buses=(SourceBus, 650) "
            "conns=(delta, wye) kvs=(115, 4.16) kvas=(5000, 5000) xhl=8 %r=0.5",
        ],
    },
    {
        "name": "transformer_wdg_seq",
        "target": "Transformer.t2",
        "commands": [
            "New Transformer.t2 phases=1 windings=2",
            "~ wdg=1 bus=a.1 conn=wye kv=7.2 kva=25 tap=1.0 %r=0.6 "
            "rneut=0.1 xneut=0.2 maxtap=1.1 mintap=0.9 numtaps=32",
            "~ wdg=2 bus=b.1 conn=wye kv=0.24 kva=25",
        ],
    },
    {
        "name": "transformer_3wdg",
        "target": "Transformer.t3",
        "commands": [
            "New Transformer.t3 phases=3 windings=3 "
            "buses=(p, s, t) conns=(delta, wye, wye) kvs=(115, 12.47, 4.16) "
            "kvas=(5000, 5000, 5000) xhl=8 xht=10 xlt=9 %loadloss=0.5",
        ],
    },
    {
        "name": "transformer_xscarray",
        "target": "Transformer.t4",
        "commands": [
            "New Transformer.t4 phases=3 windings=3 buses=(a, b, c) "
            "kvs=(115, 12.47, 4.16) kvas=(5000, 5000, 5000) xscarray=(8 10 9)",
        ],
    },
    {
        "name": "transformer_xfmrcode",
        "target": "Transformer.t5",
        "commands": [
            "New XfmrCode.xc windings=2 kvs=(115, 4.16) kvas=(3000, 3000) "
            "xhl=7 conns=(delta, wye)",
            "New Transformer.t5 buses=(p, s) xfmrcode=xc",
        ],
    },
    {
        "name": "transformer_makelike",
        "target": "Transformer.t6",
        "commands": [
            "New Transformer.base phases=3 windings=2 buses=(p, s) "
            "conns=(delta, wye) kvs=(115, 4.16) kvas=(3000, 3000) xhl=7 %r=0.4",
            "New Transformer.t6 like=base buses=(p2, s2)",
        ],
    },
    # --- Capacitor (WP4.5) ---
    # NOTE: the oracle's `DoubleSymMatrixProperty` getter for `Capacitor.CMatrix`
    # reads uninitialized memory (it returns denormal garbage even when
    # `cmatrix=` is set — a genuine dss_capi bug), so its numbers are
    # canonicalized to zeros via `zero_garbage` in every Capacitor scenario; only
    # the matrix skeleton is pinned, and the Rust getter emits the same zeros.
    {
        "name": "cap_default",
        "target": "Capacitor.c1",
        "commands": ["New Capacitor.c1"],
        "zero_garbage": ["CMatrix"],
    },
    {
        "name": "cap_kvar",
        "target": "Capacitor.c1",
        "commands": ["New Capacitor.c1 bus1=b1 phases=3 kvar=600 kv=4.16"],
        "zero_garbage": ["CMatrix"],
    },
    {
        "name": "cap_cuf",
        "target": "Capacitor.c1",
        "commands": ["New Capacitor.c1 bus1=b1 phases=1 cuf=10 kv=2.4"],
        "zero_garbage": ["CMatrix"],
    },
    {
        "name": "cap_cmatrix",
        "target": "Capacitor.c1",
        "commands": [
            "New Capacitor.c1 bus1=b1 phases=3 "
            "cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8)",
        ],
        "zero_garbage": ["CMatrix"],
    },
    {
        "name": "cap_numsteps",
        "target": "Capacitor.c1",
        "commands": [
            "New Capacitor.c1 bus1=b1 phases=3 kvar=600 kv=4.16 numsteps=3 states=(1 1 0)",
        ],
        "zero_garbage": ["CMatrix"],
    },
    {
        "name": "cap_series_xl",
        "target": "Capacitor.c1",
        "commands": [
            "New Capacitor.c1 bus1=b1 bus2=b2 phases=3 kvar=600 kv=4.16 r=0.1 xl=1.0",
        ],
        "zero_garbage": ["CMatrix"],
    },
    {
        "name": "cap_makelike",
        "target": "Capacitor.c1",
        "commands": [
            "New Capacitor.base bus1=b1 phases=3 kvar=300 kv=4.16",
            "New Capacitor.c1 like=base",
        ],
        "zero_garbage": ["CMatrix"],
    },
    # --- Reactor (WP4.6) ---
    # NOTE: like Capacitor.CMatrix, the oracle's `DoubleSymMatrixProperty` getter
    # for `Reactor.RMatrix`/`XMatrix` reads uninitialized memory (denormal garbage
    # regardless of what is stored — the same dss_capi bug), so both are
    # canonicalized to zeros via `zero_garbage` in every Reactor scenario; only
    # the matrix skeleton is pinned and the Rust getter emits the same zeros.
    {
        "name": "reactor_default",
        "target": "Reactor.r1",
        "commands": ["New Reactor.r1"],
        "zero_garbage": ["RMatrix", "XMatrix"],
    },
    {
        "name": "reactor_kvar",
        "target": "Reactor.r1",
        "commands": ["New Reactor.r1 bus1=b1 phases=3 kvar=500 kv=12.47"],
        "zero_garbage": ["RMatrix", "XMatrix"],
    },
    {
        "name": "reactor_rx",
        "target": "Reactor.r1",
        "commands": ["New Reactor.r1 bus1=b1 bus2=b2 phases=3 R=1.0 X=5.0"],
        "zero_garbage": ["RMatrix", "XMatrix"],
    },
    {
        "name": "reactor_z",
        "target": "Reactor.r1",
        "commands": ["New Reactor.r1 bus1=b1 phases=3 Z=(1, 5)"],
        "zero_garbage": ["RMatrix", "XMatrix"],
    },
    {
        "name": "reactor_lmh",
        "target": "Reactor.r1",
        "commands": ["New Reactor.r1 bus1=b1 phases=1 R=0.5 LmH=10"],
        "zero_garbage": ["RMatrix", "XMatrix"],
    },
    {
        "name": "reactor_z1z2z0",
        "target": "Reactor.r1",
        "commands": ["New Reactor.r1 bus1=b1 phases=3 Z1=(1, 5) Z2=(1, 5) Z0=(2, 8)"],
        "zero_garbage": ["RMatrix", "XMatrix"],
    },
    {
        "name": "reactor_matrix",
        "target": "Reactor.r1",
        "commands": [
            "New Reactor.r1 bus1=b1 bus2=b2 phases=3 "
            "RMatrix=(1 | 0.2 1 | 0.2 0.2 1) XMatrix=(5 | 1 5 | 1 1 5)",
        ],
        "zero_garbage": ["RMatrix", "XMatrix"],
    },
    {
        "name": "reactor_parallel",
        "target": "Reactor.r1",
        "commands": [
            "New Reactor.r1 bus1=b1 phases=3 parallel=yes "
            "RMatrix=(1 | 0.2 1 | 0.2 0.2 1) XMatrix=(5 | 1 5 | 1 1 5)",
        ],
        "zero_garbage": ["RMatrix", "XMatrix"],
    },
    {
        "name": "reactor_rp",
        "target": "Reactor.r1",
        "commands": ["New Reactor.r1 bus1=b1 phases=3 kvar=500 kv=12.47 Rp=10000"],
        "zero_garbage": ["RMatrix", "XMatrix"],
    },
    {
        "name": "reactor_makelike",
        "target": "Reactor.r1",
        "commands": [
            "New Reactor.base bus1=b1 phases=3 kvar=300 kv=12.47",
            "New Reactor.r1 like=base",
        ],
        "zero_garbage": ["RMatrix", "XMatrix"],
    },
    # --- Fault (WP7.2) ---
    # `R` stores its inverse `G` (InverseValue); `GMatrix` shares the oracle's
    # DoubleSymMatrixProperty garbage-getter bug (zeroed via `zero_garbage`, like
    # Capacitor.CMatrix / Reactor.RMatrix).
    {
        "name": "fault_default",
        "target": "Fault.f1",
        "commands": ["New Fault.f1"],
        "zero_garbage": ["GMatrix"],
    },
    {
        "name": "fault_r",
        "target": "Fault.f1",
        "commands": ["New Fault.f1 bus1=b1 phases=3 r=2.5"],
        "zero_garbage": ["GMatrix"],
    },
    {
        "name": "fault_bus2_series",
        "target": "Fault.f1",
        "commands": ["New Fault.f1 bus1=b1 bus2=b2 phases=3 r=1"],
        "zero_garbage": ["GMatrix"],
    },
    {
        "name": "fault_gmatrix",
        "target": "Fault.f1",
        "commands": ["New Fault.f1 bus1=b1 phases=2 Gmatrix=(1 | 0.5 2)"],
        "zero_garbage": ["GMatrix"],
    },
    {
        "name": "fault_temporary",
        "target": "Fault.f1",
        "commands": [
            "New Fault.f1 bus1=b1 phases=1 r=1 ontime=0.5 temporary=yes "
            "minamps=20 %stddev=5",
        ],
        "zero_garbage": ["GMatrix"],
    },
    {
        "name": "fault_makelike",
        "target": "Fault.f1",
        # Set the TPDElement rating fields on the base so the dump pins that
        # MakeLike copies them (faultrate/pctperm/repair).
        "commands": [
            "New Fault.base bus1=b1 phases=3 r=3 minamps=8 "
            "faultrate=0.5 pctperm=80 repair=4",
            "New Fault.f1 like=base",
        ],
        "zero_garbage": ["GMatrix"],
    },
    # --- RegControl / CapControl (WP4.7, parse-only) ---
    # Every scenario defines the referenced transformer/capacitor/line first;
    # a RegControl without `transformer=` (or a CapControl without
    # `capacitor=`) raises in the oracle (errors 124 / 303), and the replay
    # asserts an error-free run.
    {
        "name": "regcontrol_basic",
        "target": "RegControl.reg1",
        "commands": [
            "New Transformer.t1 phases=3 windings=2 buses=(sourcebus, b650) "
            "conns=(delta wye) kvs=(115 4.16) kvas=(5000 5000) xhl=8",
            "New RegControl.reg1 transformer=t1 winding=2 vreg=122 band=2 "
            "ptratio=20 ctprim=700 R=3 X=9",
        ],
    },
    {
        "name": "regcontrol_full",
        "target": "RegControl.reg1",
        "commands": [
            "New Transformer.t1 phases=1 windings=2 buses=(650.1, rg60.1) "
            "kvs=(2.4 2.4) kvas=(1666 1666) xhl=0.01",
            "New RegControl.reg1 transformer=t1 winding=2 vreg=122 band=2 "
            "ptratio=20 ctprim=700 R=-0.201 X=3.348 delay=45 reversible=yes "
            "revvreg=118 revband=4 revR=1.5 revX=2.5 tapdelay=3 maxtapchange=8 "
            "inversetime=yes vlimit=126 revThreshold=150 revDelay=90 "
            "revNeutral=yes EventLog=yes RemotePTRatio=25 LDC_Z=1.2 rev_Z=0.8 "
            "Cogen=yes",
        ],
    },
    {
        "name": "regcontrol_ptphase_max",
        "target": "RegControl.reg1",
        "commands": [
            "New Transformer.t1 phases=3 windings=2 buses=(sourcebus, b650) "
            "kvs=(115 4.16) kvas=(5000 5000) xhl=8",
            "New RegControl.reg1 transformer=t1 winding=1 PTphase=max bus=b650",
        ],
    },
    {
        "name": "regcontrol_tapnum",
        "target": "RegControl.reg1",
        "commands": [
            "New Transformer.t1 phases=1 windings=2 buses=(650.1, rg60.1) "
            "kvs=(2.4 2.4) kvas=(1666 1666) xhl=0.01",
            "New RegControl.reg1 transformer=t1 winding=2 tapnum=5",
        ],
    },
    {
        "name": "regcontrol_makelike",
        "target": "RegControl.reg1",
        "commands": [
            "New Transformer.t1 phases=1 windings=2 buses=(650.1, rg60.1) "
            "kvs=(2.4 2.4) kvas=(1666 1666) xhl=0.01",
            "New Transformer.t2 like=t1 buses=(650.2, rg60.2)",
            "New RegControl.base transformer=t1 winding=2 vreg=124 band=2 "
            "ptratio=20 ctprim=300 R=0.6 X=1.3",
            "New RegControl.reg1 like=base transformer=t2 R=1.4 X=2.6",
        ],
    },
    {
        "name": "capcontrol_current",
        "target": "CapControl.cc1",
        "commands": [
            "New Capacitor.cap1 bus1=b632 phases=3 kvar=600 kv=4.16",
            "New Line.l1 bus1=b632 bus2=b633 phases=3 r1=0.1 x1=0.2 c1=3 length=1",
            "New CapControl.cc1 element=Line.l1 terminal=1 capacitor=cap1 "
            "ctratio=80 onsetting=250 offsetting=150 delay=20 delayoff=25 "
            "deadtime=120",
        ],
    },
    {
        "name": "capcontrol_kvar_voltoverride",
        "target": "CapControl.cc1",
        "commands": [
            "New Capacitor.cap1 bus1=b632 phases=3 kvar=600 kv=4.16",
            "New Line.l1 bus1=b632 bus2=b633 phases=3 r1=0.1 x1=0.2 c1=3 length=1",
            "New CapControl.cc1 element=Line.l1 terminal=2 capacitor=cap1 "
            "type=kvar onsetting=150 offsetting=-50 voltoverride=yes vmax=128 "
            "vmin=112 ptratio=34.67 pctMinkvar=60 EventLog=yes",
        ],
    },
    {
        "name": "capcontrol_voltage_phases",
        "target": "CapControl.cc1",
        "commands": [
            "New Capacitor.cap1 bus1=b632 phases=3 kvar=600 kv=4.16",
            "New Line.l1 bus1=b632 bus2=b633 phases=3 r1=0.1 x1=0.2 c1=3 length=1",
            "New CapControl.cc1 element=Line.l1 terminal=1 capacitor=cap1 "
            "type=voltage onsetting=118 offsetting=126 ptratio=34.67 "
            "PTPhase=max CTPhase=2",
        ],
    },
    {
        "name": "capcontrol_time_forces_terminal",
        "target": "CapControl.cc1",
        "commands": [
            "New Capacitor.cap1 bus1=b632 phases=3 kvar=600 kv=4.16",
            "New CapControl.cc1 capacitor=cap1 type=time terminal=2 "
            "onsetting=10 offsetting=14",
        ],
    },
    {
        "name": "capcontrol_pf",
        "target": "CapControl.cc1",
        "commands": [
            "New Capacitor.cap1 bus1=b632 phases=3 kvar=600 kv=4.16",
            "New Line.l1 bus1=b632 bus2=b633 phases=3 r1=0.1 x1=0.2 c1=3 length=1",
            "New CapControl.cc1 element=Line.l1 terminal=1 capacitor=cap1 "
            "type=pf onsetting=0.97 offsetting=-0.99",
        ],
    },
    {
        "name": "capcontrol_makelike",
        "target": "CapControl.cc1",
        "commands": [
            "New Capacitor.cap1 bus1=b632 phases=3 kvar=600 kv=4.16",
            "New Capacitor.cap2 bus1=b633 phases=3 kvar=300 kv=4.16",
            "New Line.l1 bus1=b632 bus2=b633 phases=3 r1=0.1 x1=0.2 c1=3 length=1",
            "New CapControl.base element=Line.l1 terminal=1 capacitor=cap1 "
            "type=kvar onsetting=150 offsetting=-50 ptratio=34.67",
            "New CapControl.cc1 like=base capacitor=cap2",
        ],
    },
    {
        # --- GenDispatcher (WP6.8) ---
        # The monitored element must exist (RecalcElementData attaches the
        # control's terminal to it); GenDispatcher without `element=` raises 372.
        "name": "gendispatcher_default",
        "target": "GenDispatcher.gd1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.1 x1=0.2 length=1",
            "New GenDispatcher.gd1 element=Line.l1",
        ],
    },
    {
        "name": "gendispatcher_full",
        "target": "GenDispatcher.gd1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.1 x1=0.2 length=1",
            "New GenDispatcher.gd1 element=Line.l1 terminal=1 kwlimit=3500 "
            "kwband=250 kvarlimit=1500 genlist=[g1, g2] weights=[2, 1]",
        ],
    },
    {
        # MakeLike copies *only* terminal + monitored element (Pascal quirk): the
        # dispatch settings revert to ctor defaults on the `like=` object.
        "name": "gendispatcher_makelike",
        "target": "GenDispatcher.gd1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.1 x1=0.2 length=1",
            "New Line.l2 bus1=b2 bus2=b3 phases=3 r1=0.1 x1=0.2 length=1",
            "New GenDispatcher.base element=Line.l1 terminal=1 kwlimit=5000 "
            "kwband=300 kvarlimit=1500 genlist=[g1, g2] weights=[2, 1]",
            "New GenDispatcher.gd1 like=base element=Line.l2",
        ],
    },
    {
        # --- StorageController (WP6.8 skeleton) ---
        # The Storage element is Phase 7, so a StorageController on a circuit
        # with no Storage always logs error 37201 at RecalcElementData (the Rust
        # port reproduces this); `allow_errors` captures the dump past it.
        "name": "storagecontroller_default",
        "target": "StorageController.sc1",
        "allow_errors": True,
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.1 x1=0.2 length=1",
            "New StorageController.sc1 element=Line.l1",
        ],
    },
    {
        "name": "storagecontroller_full",
        "target": "StorageController.sc1",
        "allow_errors": True,
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.1 x1=0.2 length=1",
            "New StorageController.sc1 element=Line.l1 terminal=1 kWTarget=5000 %kWBand=5 "
            "kWTargetLow=2500 %kWBandLow=4 modedischarge=support modecharge=peakshavelow "
            "monphase=avg TimeDischargeTrigger=10 TimeChargeTrigger=3 %RatekW=30 "
            "%RateCharge=25 %Reserve=20 EventLog=yes InhibitTime=8 TUp=0.5 TFlat=1.5 "
            "TDn=0.4 kWThreshold=4000 DispFactor=0.8 ResetLevel=0.7 Seasons=2 "
            "SeasonTargets=[5000, 4500] SeasonTargetsLow=[2500, 2200]",
        ],
    },
    {
        # ElementList + Weights round-trip (the named entries never resolve to a
        # Storage element — 14403 — but the name list / weights still dump).
        "name": "storagecontroller_elementlist",
        "target": "StorageController.sc1",
        "allow_errors": True,
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.1 x1=0.2 length=1",
            "New StorageController.sc1 element=Line.l1 elementlist=[sa, sb] weights=[2, 3]",
        ],
    },
    {
        # MakeLike copies essentially every dispatch setting (unlike
        # GenDispatcher); the derived object only overrides Element.
        "name": "storagecontroller_makelike",
        "target": "StorageController.sc1",
        "allow_errors": True,
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.1 x1=0.2 length=1",
            "New Line.l2 bus1=b2 bus2=b3 phases=3 r1=0.1 x1=0.2 length=1",
            "New StorageController.base element=Line.l1 terminal=1 kWTarget=5000 %kWBand=5 "
            "modedischarge=follow %Reserve=20 Seasons=2 SeasonTargets=[5000, 4500] "
            "SeasonTargetsLow=[2500, 2200]",
            "New StorageController.sc1 like=base element=Line.l2",
        ],
    },
    # --- SwtControl (WP7.2 step 2a) ---------------------------------------
    # Action/Normal/State all map onto the one CurrentAction field; the text
    # dump renders it (Action=close/open, Normal/State=closed/open) — GetState is
    # not used by the `?` dump (probed). Action/Normal/State are ConditionalReadOnly
    # on Locked (a write while locked is ignored). Every scenario defines the
    # switched Line first (SwtControl without SwitchedObj raises 387).
    {
        "name": "swtcontrol_default",
        "target": "SwtControl.sw1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1 switch=y",
            "New SwtControl.sw1 switchedobj=line.l1 switchedterm=1",
        ],
    },
    {
        "name": "swtcontrol_action_open",
        "target": "SwtControl.sw1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1 switch=y",
            "New SwtControl.sw1 switchedobj=line.l1 switchedterm=1 action=open",
        ],
    },
    {
        # Normal= sets NormalState := CurrentAction; the dump shows Action=open
        # too (the shared CurrentAction field).
        "name": "swtcontrol_normal_open",
        "target": "SwtControl.sw1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1 switch=y",
            "New SwtControl.sw1 switchedobj=line.l1 switchedterm=1 normal=open",
        ],
    },
    {
        # State= forces the controlled element open (deferred RefAction in Rust)
        # and dumps State=open via the shared field.
        "name": "swtcontrol_state_open",
        "target": "SwtControl.sw1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1 switch=y",
            "New SwtControl.sw1 switchedobj=line.l1 switchedterm=1 state=open",
        ],
    },
    {
        "name": "swtcontrol_lock_delay",
        "target": "SwtControl.sw1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1 switch=y",
            "New SwtControl.sw1 switchedobj=line.l1 switchedterm=1 lock=yes delay=30",
        ],
    },
    {
        # lock=yes BEFORE action=open: the ConditionalReadOnly guard ignores the
        # action (Action stays close), proving the locked read-only path.
        "name": "swtcontrol_locked_then_action",
        "target": "SwtControl.sw1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1 switch=y",
            "New SwtControl.sw1 switchedobj=line.l1 switchedterm=1 lock=yes action=open",
        ],
    },
    {
        # MakeLike copies CurrentAction/NormalState/PresentState/TimeDelay/Locked.
        "name": "swtcontrol_makelike",
        "target": "SwtControl.sw1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1 switch=y",
            "New SwtControl.base switchedobj=line.l1 switchedterm=1 action=open "
            "normal=open delay=45 lock=yes",
            "New SwtControl.sw1 like=base",
        ],
    },
    # --- Fuse (PDElements/fuse.pas; a per-phase TControlElem) ---
    # Normal/State are per-phase enum arrays sized by ControlledElement.NPhases,
    # dumped `[closed, closed, closed, ]`. MonitoredObj defaults SwitchedObj to the
    # same element; FuseCurve defaults to the built-in `tlink`. Action dumps empty.
    {
        "name": "fuse_default",
        "target": "Fuse.f1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Fuse.f1 monitoredobj=line.l1 monitoredterm=1",
        ],
    },
    {
        # 1-phase monitored element → single-element state arrays `[closed, ]`.
        "name": "fuse_1phase",
        "target": "Fuse.f1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=1 r1=0.3 x1=0.6 length=1",
            "New Fuse.f1 monitoredobj=line.l1",
        ],
    },
    {
        # action=open (deprecated) sets all phases open, and (NormalStateSet was
        # false) copies them to Normal too.
        "name": "fuse_action_open",
        "target": "Fuse.f1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Fuse.f1 monitoredobj=line.l1 action=open",
        ],
    },
    {
        # state=[open,open,open] forces the controlled element open (deferred
        # RefAction in Rust) and copies State→Normal (NormalStateSet was false).
        "name": "fuse_state_all_open",
        "target": "Fuse.f1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Fuse.f1 monitoredobj=line.l1 state=[open,open,open]",
        ],
    },
    {
        # A short state array sets only the leading phase (`[open, closed, closed, ]`).
        "name": "fuse_state_partial",
        "target": "Fuse.f1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Fuse.f1 monitoredobj=line.l1 state=[open]",
        ],
    },
    {
        # normal= sets the reset target per phase; State stays the default closed.
        "name": "fuse_normal_partial",
        "target": "Fuse.f1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Fuse.f1 monitoredobj=line.l1 normal=[open,closed,open]",
        ],
    },
    {
        # Explicit fuse link + rating + delay + a distinct switched element.
        "name": "fuse_curve_rated_switched",
        "target": "Fuse.f1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Line.l2 bus1=b2 bus2=b3 phases=3 r1=0.3 x1=0.6 length=1",
            "New TCC_Curve.tc npts=2 c_array=[1,10] t_array=[1,0.1]",
            "New Fuse.f1 monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l2 "
            "switchedterm=1 fusecurve=tc ratedcurrent=20 delay=0.5",
        ],
    },
    {
        # MakeLike copies the references, rating, and per-phase states (but not
        # DelayTime — Pascal omits it).
        "name": "fuse_makelike",
        "target": "Fuse.f1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Fuse.base monitoredobj=line.l1 ratedcurrent=15 normal=[open,closed,open]",
            "New Fuse.f1 like=base",
        ],
    },
    # --- Recloser (Controls/Recloser.pas; a TControlElem) ------------------
    # Action/State map onto FPresentState (close/open/trip), Normal onto
    # NormalState; the first State/Action defaults Normal. Shots aliases
    # NumReclose with a -1 offset; RecloseIntervals (ArrayMaxSize=4) also sets
    # NumReclose to its count. PhaseFast/PhaseDelayed default to the built-in
    # `a`/`d` curves; the ground curves default to NIL. MonitoredObj defaults
    # SwitchedObj to the same element.
    {
        "name": "recloser_default",
        "target": "Recloser.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Recloser.r1 monitoredobj=line.l1 monitoredterm=1",
        ],
    },
    {
        # Full spec with explicit ground curves, a distinct switched element, all
        # trips/insts/time-dials, and a 2-shot reclose sequence.
        "name": "recloser_full",
        "target": "Recloser.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Line.l2 bus1=b2 bus2=b3 phases=3 r1=0.3 x1=0.6 length=1",
            "New TCC_Curve.pf npts=2 c_array=[1,10] t_array=[1,0.1]",
            "New TCC_Curve.pd npts=2 c_array=[1,10] t_array=[2,0.2]",
            "New Recloser.r1 monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l2 "
            "switchedterm=1 numfast=2 phasefast=pf phasedelayed=pd groundfast=pf "
            "grounddelayed=pd phasetrip=800 groundtrip=400 phaseinst=2000 "
            "groundinst=1500 reset=20 shots=3 recloseintervals=(0.5 1.5) delay=0.1 "
            "tdphfast=1.2 tdgrfast=1.1 tdphdelayed=1.3 tdgrdelayed=1.4",
        ],
    },
    {
        # shots=2 then RecloseIntervals=(1 3): the array write wins, so
        # NumReclose=2 ⇒ Shots dumps 3 and RecloseIntervals dumps 2 elements.
        "name": "recloser_shots_then_intervals",
        "target": "Recloser.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Recloser.r1 monitoredobj=line.l1 shots=2 recloseintervals=(1.0 3.0)",
        ],
    },
    {
        # shots=1 ⇒ NumReclose=0 ⇒ RecloseIntervals dumps the empty '[]'.
        "name": "recloser_one_shot",
        "target": "Recloser.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Recloser.r1 monitoredobj=line.l1 shots=1",
        ],
    },
    {
        # state=open drives FPresentState and defaults NormalState (NormalStateSet
        # was false): Action=open, Normal=open, State=open.
        "name": "recloser_state_open",
        "target": "Recloser.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Recloser.r1 monitoredobj=line.l1 state=open",
        ],
    },
    {
        # normal=trip (trip aliases open) only sets NormalState: Normal=open,
        # State=closed, Action=close (FPresentState untouched).
        "name": "recloser_normal_trip",
        "target": "Recloser.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Recloser.r1 monitoredobj=line.l1 normal=trip",
        ],
    },
    {
        # MakeLike copies the trips/curves/shots/intervals/normal state, but NOT
        # DelayTime or the TD* time dials (Pascal omits them → Create defaults).
        "name": "recloser_makelike",
        "target": "Recloser.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Recloser.base monitoredobj=line.l1 numfast=2 phasetrip=700 shots=2 "
            "recloseintervals=(0.5 1) normal=open reset=22 delay=0.3 tdphfast=1.5",
            "New Recloser.r1 like=base",
        ],
    },
    # --- Relay (Controls/Relay.pas; a TControlElem) ------------------------
    # Nine sub-types via Type=; the property surface is shared. Action/State map
    # onto FPresentState, Normal onto NormalState (first State/Action defaults
    # it). Shots aliases NumReclose (-1 offset); RecloseIntervals (ArrayMaxSize=4)
    # also sets NumReclose. Type sets per-type Delay + reclose-interval defaults.
    # MonitoredObj defaults SwitchedObj to the same element.
    {
        "name": "relay_default",
        "target": "Relay.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Relay.r1 monitoredobj=line.l1 monitoredterm=1 type=current",
        ],
    },
    {
        # Full overcurrent spec: explicit phase/ground curves, a distinct switched
        # element, trips/insts/time-dials, a 2-shot reclose.
        "name": "relay_overcurrent_full",
        "target": "Relay.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Line.l2 bus1=b2 bus2=b3 phases=3 r1=0.3 x1=0.6 length=1",
            "New TCC_Curve.pc npts=2 c_array=[1,10] t_array=[1,0.1]",
            "New TCC_Curve.gc npts=2 c_array=[1,10] t_array=[2,0.2]",
            "New Relay.r1 monitoredobj=line.l1 monitoredterm=1 switchedobj=line.l2 "
            "switchedterm=1 type=current phasecurve=pc groundcurve=gc phasetrip=800 "
            "groundtrip=400 tdphase=1.2 tdground=1.1 phaseinst=2000 groundinst=1500 "
            "reset=20 shots=3 recloseintervals=(0.5 1.5) breakertime=0.05 kvbase=12.47",
        ],
    },
    {
        # Voltage relay (27/59): kVBase + OV/UV curves. Type=voltage sets
        # NumReclose=1 and RecloseIntervals[3]:=5.
        "name": "relay_voltage",
        "target": "Relay.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New TCC_Curve.ov npts=2 c_array=[1.1,1.5] t_array=[5,0.1]",
            "New TCC_Curve.uv npts=2 c_array=[0.5,0.9] t_array=[0.1,5]",
            "New Relay.r1 monitoredobj=line.l1 type=voltage kvbase=12.47 "
            "overvoltcurve=ov undervoltcurve=uv",
        ],
    },
    {
        # Directional overcurrent (DOC): the corpus shape (tilt-low 95, trip-low,
        # delay=0, shots=1). Type=doc sets NumReclose=0.
        "name": "relay_doc",
        "target": "Relay.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Relay.r1 monitoredobj=line.l1 monitoredterm=1 type=doc "
            "doc_tiltanglelow=95 doc_tripsettinglow=3500 delay=0 shots=1 normal=close",
        ],
    },
    {
        # Distance relay (21): the Z1/Z0/M reach settings + reverse.
        "name": "relay_distance",
        "target": "Relay.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Relay.r1 monitoredobj=line.l1 type=distance z1mag=0.8 z1ang=65 "
            "z0mag=2.2 z0ang=70 mphase=0.75 mground=0.8 distreverse=yes",
        ],
    },
    {
        # 46 (neg-seq current): base/pct/isqt settings (PickupAmps46 is derived).
        "name": "relay_46",
        "target": "Relay.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Relay.r1 monitoredobj=line.l1 type=46 46baseamps=120 46%pickup=25 "
            "46isqt=1.5",
        ],
    },
    {
        # state=open drives FPresentState and defaults NormalState.
        "name": "relay_state_open",
        "target": "Relay.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Relay.r1 monitoredobj=line.l1 state=open",
        ],
    },
    {
        # normal=trip (trip aliases open) only sets NormalState.
        "name": "relay_normal_trip",
        "target": "Relay.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Relay.r1 monitoredobj=line.l1 normal=trip",
        ],
    },
    {
        # MakeLike copies the full settings incl. DelayTime / BreakerTime.
        "name": "relay_makelike",
        "target": "Relay.r1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
            "New Relay.base monitoredobj=line.l1 type=current phasetrip=700 shots=2 "
            "recloseintervals=(0.5 1) normal=open reset=22 delay=0.3 breakertime=0.04",
            "New Relay.r1 like=base",
        ],
    },
    {
        "name": "generator_default",
        "target": "Generator.g1",
        "commands": ["New Generator.g1 bus1=genbus"],
    },
    {
        "name": "generator_kw_pf",
        "target": "Generator.g1",
        "commands": ["New Generator.g1 bus1=genbus kV=12.47 kW=250 pf=0.9 model=1"],
    },
    {
        "name": "generator_kw_kvar_delta",
        "target": "Generator.g1",
        "commands": [
            "New Generator.g1 bus1=genbus phases=3 kV=4.16 kW=500 kvar=100 conn=delta",
        ],
    },
    {
        "name": "generator_model3_pv",
        "target": "Generator.g1",
        "commands": [
            "New Generator.g1 bus1=genbus kV=12.47 kW=300 model=3 "
            "vpu=1.02 maxkvar=150 minkvar=-150 pvfactor=0.15",
        ],
    },
    {
        "name": "generator_kva",
        "target": "Generator.g1",
        "commands": ["New Generator.g1 bus1=genbus kV=12.47 kW=100 kVA=150"],
    },
    {
        "name": "generator_fuel",
        "target": "Generator.g1",
        "commands": [
            "New Generator.g1 bus1=genbus kV=12.47 kW=100 "
            "usefuel=yes fuelkwh=5000 %fuel=80 %reserve=15",
        ],
    },
    {
        "name": "generator_status_dispatch",
        "target": "Generator.g1",
        "commands": [
            "New Generator.g1 bus1=genbus kV=12.47 kW=200 pf=0.95 "
            "status=fixed dispmode=loadlevel dispvalue=0.8 forceon=yes",
        ],
    },
    {
        "name": "generator_makelike",
        "target": "Generator.g1",
        "commands": [
            "New Generator.base bus1=gb kV=12.47 kW=400 pf=0.92 model=3 conn=delta",
            "New Generator.g1 like=base bus1=gb2",
        ],
    },
    # --- PVSystem (WP7.3 step 2) --------------------------------------------
    {
        "name": "pvsystem_default",
        "target": "PVSystem.pv1",
        "commands": ["New PVSystem.pv1 bus1=pvbus"],
    },
    {
        "name": "pvsystem_pf",
        "target": "PVSystem.pv1",
        "commands": [
            "New PVSystem.pv1 bus1=pvbus phases=3 kV=12.47 kVA=500 Pmpp=500 "
            "pf=0.95 irradiance=0.9 Temperature=30",
        ],
    },
    {
        "name": "pvsystem_kvar_delta",
        "target": "PVSystem.pv1",
        "commands": [
            "New PVSystem.pv1 bus1=pvbus phases=3 kV=0.48 conn=delta kVA=250 "
            "Pmpp=200 kvar=50",
        ],
    },
    {
        "name": "pvsystem_cutinout_model",
        "target": "PVSystem.pv1",
        "commands": [
            "New PVSystem.pv1 bus1=pvbus kV=12.47 kVA=500 Pmpp=500 %Cutin=30 "
            "%Cutout=25 %R=40 %X=10 model=2 Vminpu=0.85 Vmaxpu=1.15 "
            "balanced=yes LimitCurrent=yes %Pmpp=80",
        ],
    },
    {
        "name": "pvsystem_curves",
        "target": "PVSystem.pv1",
        "commands": [
            "New XYcurve.effcurve npts=4 xarray=(0.1 0.2 0.4 1.0) "
            "yarray=(0.86 0.9 0.93 0.97)",
            "New XYcurve.pt npts=4 xarray=(0 25 75 100) yarray=(1.2 1.0 0.8 0.6)",
            "New PVSystem.pv1 bus1=pvbus kV=12.47 kVA=500 Pmpp=500 "
            "EffCurve=effcurve P-TCurve=pt irradiance=0.9 Temperature=30",
        ],
    },
    {
        "name": "pvsystem_shapes",
        "target": "PVSystem.pv1",
        "commands": [
            "New LoadShape.irr npts=4 interval=1 mult=(0.2 0.6 0.9 1.0)",
            "New TShape.temp npts=4 interval=1 temp=(20 25 35 30)",
            "New PVSystem.pv1 bus1=pvbus kV=12.47 kVA=500 Pmpp=500 daily=irr "
            "yearly=irr duty=irr Tdaily=temp Tyearly=temp Tduty=temp DutyStart=2",
        ],
    },
    {
        "name": "pvsystem_limits",
        "target": "PVSystem.pv1",
        "commands": [
            "New PVSystem.pv1 bus1=pvbus kV=12.47 kVA=500 Pmpp=500 pf=0.9 "
            "kvarMax=200 kvarMaxAbs=150 %PminNoVars=10 %PminkvarMax=20 "
            "WattPriority=yes PFPriority=yes VarFollowInverter=yes",
        ],
    },
    {
        "name": "pvsystem_inverter_params",
        "target": "PVSystem.pv1",
        "commands": [
            "New PVSystem.pv1 bus1=pvbus kV=12.47 kVA=500 Pmpp=500 kVDC=10 "
            "Kp=0.02 PITol=2 SafeVoltage=85 AmpLimit=1.5 AmpLimitGain=0.7 class=3",
        ],
    },
    {
        "name": "pvsystem_dynexpr",
        "target": "PVSystem.pv1",
        "commands": [
            "New DynamicExp.de nvariables=2 varnames=[it dit] domain=time "
            "expression=[it dt = dit]",
            "New PVSystem.pv1 bus1=pvbus kV=12.47 kVA=500 Pmpp=500 DynamicEq=de",
        ],
    },
    {
        "name": "pvsystem_makelike",
        "target": "PVSystem.pv1",
        "commands": [
            "New PVSystem.base bus1=pb kV=12.47 kVA=400 Pmpp=350 pf=0.92 "
            "conn=delta %R=30 %X=8 model=2",
            "New PVSystem.pv1 like=base bus1=pb2",
        ],
    },
    {
        "name": "monitor_default",
        "target": "Monitor.m1",
        "commands": [
            "New Line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1",
            "New Monitor.m1 element=Line.l1 terminal=1 mode=0",
        ],
    },
    {
        "name": "monitor_mode1_residual",
        "target": "Monitor.m1",
        "commands": [
            "New Line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1",
            "New Monitor.m1 element=Line.l1 terminal=1 mode=1 residual=yes ppolar=no",
        ],
    },
    {
        "name": "monitor_vipolar_off",
        "target": "Monitor.m1",
        "commands": [
            "New Line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1",
            "New Monitor.m1 element=Line.l1 terminal=1 mode=48 vipolar=no",
        ],
    },
    {
        "name": "monitor_transformer_tap",
        "target": "Monitor.mt",
        "commands": [
            "New Transformer.t1 phases=3 windings=2 buses=[sourcebus b2] "
            "conns=[wye wye] kvs=[12.47 4.16] kvas=[1000 1000] xhl=5",
            "New Monitor.mt element=Transformer.t1 terminal=2 mode=2",
        ],
    },
    {
        "name": "monitor_makelike",
        "target": "Monitor.m1",
        "commands": [
            "New Line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1",
            "New Monitor.base element=Line.l1 terminal=1 mode=0 residual=yes",
            "New Monitor.m1 like=base mode=1",
        ],
    },
    {
        "name": "energymeter_default",
        "target": "EnergyMeter.m1",
        "commands": [
            "New Line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1 length=1",
            "New EnergyMeter.m1 element=Line.l1 terminal=1",
        ],
    },
    {
        "name": "energymeter_options_mask",
        "target": "EnergyMeter.m1",
        "commands": [
            "New Line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1 length=1",
            "New Line.l2 bus1=b2 bus2=b3 r1=0.1 x1=0.1 length=2",
            "New EnergyMeter.m1 element=Line.l1 terminal=1 option=(T,M,V) "
            "zonelist=(line.l1, line.l2) kVANormal=5000 kVAEmerg=6000 "
            "peakcurrent=(300, 350, 400) mask=(0 0 1) LocalOnly=yes Losses=no "
            "LineLosses=no Int_Rate=0.1 Int_Duration=2.5",
        ],
    },
    {
        "name": "energymeter_makelike",
        "target": "EnergyMeter.m1",
        "commands": [
            "New Line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1 length=1",
            "New EnergyMeter.base element=Line.l1 terminal=1 option=(T,R,V) "
            "kVANormal=1000 LocalOnly=yes",
            "New EnergyMeter.m1 like=base kVAEmerg=2000",
        ],
    },
    # --- Sensor (WP6.7) ---
    # NOTE (probed): `element=`/`conn=`/`deltadirection=` all set NeedsRecalc, so
    # `RecalcElementData` runs at EndEdit and `ZeroSensorArrays` zeros the
    # measured arrays. A single `New Sensor … currents=…` therefore dumps
    # `[ 0 0 0]`; to retain measured values they must be set in a later `edit`
    # (no recalc trigger). MakeLike copies only the shape/metered fields, so a
    # `like=` sensor keeps the new object's defaults and NIL arrays (dump '').
    {
        "name": "sensor_default",
        "target": "Sensor.s1",
        "commands": [
            "New Line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1",
            "New Sensor.s1 element=Line.l1 terminal=1",
        ],
    },
    {
        "name": "sensor_currents_single_zeroed",
        "target": "Sensor.s1",
        "commands": [
            "New Line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1",
            "New Sensor.s1 element=Line.l1 terminal=1 currents=(10 11 12) "
            "conn=wye weight=2 %error=3",
        ],
    },
    {
        "name": "sensor_currents_twostep",
        "target": "Sensor.s1",
        "commands": [
            "New Line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1",
            "New Sensor.s1 element=Line.l1 terminal=1",
            "Edit Sensor.s1 currents=(10 11 12)",
        ],
    },
    {
        "name": "sensor_pq",
        "target": "Sensor.s1",
        "commands": [
            "New Line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1",
            "New Sensor.s1 element=Line.l1 terminal=1 kvbase=12.47",
            "Edit Sensor.s1 kws=(100 100 100) kvars=(30 30 30)",
        ],
    },
    {
        "name": "sensor_kvs_delta",
        "target": "Sensor.s1",
        "commands": [
            "New Line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1",
            "New Sensor.s1 element=Line.l1 terminal=1 conn=delta deltadirection=-1 kvbase=4.16",
            "Edit Sensor.s1 kvs=(7.2 7.2 7.2)",
        ],
    },
    {
        "name": "sensor_makelike",
        "target": "Sensor.s1",
        "commands": [
            "New Line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1",
            "New Sensor.base element=Line.l1 terminal=1 conn=delta deltadirection=-1 "
            "kvbase=4.16 weight=3 %error=2",
            "Edit Sensor.base kvs=(7.2 7.2 7.2)",
            "New Sensor.s1 like=base",
        ],
    },
    # --- WireData / CNData / TSData (WP7.1 step 2) ---------------------------
    {
        "name": "wiredata_default",
        "target": "WireData.wd1",
        "commands": ["New WireData.wd1"],
    },
    {
        "name": "wiredata_full",
        "target": "WireData.acsr",
        "commands": [
            "New WireData.acsr Rdc=0.0526 Rac=0.0535 Runits=mi GMRac=0.0244 "
            "GMRunits=ft radius=0.0306 radunits=ft normamps=530 emergamps=795"
        ],
    },
    {
        "name": "wiredata_abbrev",
        "target": "WireData.w",
        "commands": ["New WireData.w rdc=0.1 gmrac=0.0048 rad=0.0635 norm=600"],
    },
    {
        # diam sets the radius field (scale 0.5); GMR/capradius default from it,
        # Rac defaults from Rdc.
        "name": "wiredata_diam_defaults",
        "target": "WireData.wd",
        "commands": ["New WireData.wd Rdc=0.05 diam=0.1 Runits=ft radunits=ft"],
    },
    {
        # Only GMRac given: radius defaults from GMR/0.7788; GMRunits seeds
        # radunits; emergamps defaults from normamps.
        "name": "wiredata_gmr_only",
        "target": "WireData.wg",
        "commands": ["New WireData.wg Rdc=0.04 GMRac=0.02 GMRunits=ft normamps=400"],
    },
    {
        "name": "wiredata_ratings",
        "target": "WireData.wr",
        "commands": ["New WireData.wr Rdc=0.05 radius=0.03 Seasons=3 Ratings=(600 800 900)"],
    },
    {
        "name": "wiredata_makelike",
        "target": "WireData.w2",
        "commands": [
            "New WireData.w1 Rdc=0.0526 Rac=0.0535 GMRac=0.0244 radius=0.0306 "
            "Runits=ft radunits=ft GMRunits=ft normamps=530",
            "New WireData.w2 like=w1",
        ],
    },
    {
        "name": "cndata_default",
        "target": "CNData.cn0",
        "commands": ["New CNData.cn0"],
    },
    {
        "name": "cndata_full",
        "target": "CNData.cn1",
        "commands": [
            "New CNData.cn1 k=16 DiaStrand=0.064 GmrStrand=0.0208 Rstrand=0.0145 "
            "EpsR=2.3 InsLayer=0.22 DiaIns=1.06 DiaCable=1.16 Rdc=0.0997 "
            "GMRac=0.0375 radius=0.0511 Runits=in radunits=in gmrunits=in normamps=350"
        ],
    },
    {
        # DiaStrand seeds GmrStrand (0.7788*0.5*DiaStrand) when GmrStrand unset.
        "name": "cndata_strand_gmr_default",
        "target": "CNData.cn2",
        "commands": [
            "New CNData.cn2 k=13 DiaStrand=0.0641 Rstrand=0.0145 EpsR=2.3 "
            "InsLayer=0.22 DiaIns=1.06 DiaCable=1.16 Rdc=0.0997 radius=0.0511"
        ],
    },
    {
        "name": "cndata_makelike",
        "target": "CNData.cnb",
        "commands": [
            "New CNData.cna k=16 DiaStrand=0.064 GmrStrand=0.0208 Rstrand=0.0145 "
            "EpsR=2.3 InsLayer=0.22 DiaIns=1.06 DiaCable=1.16 Rdc=0.0997 "
            "GMRac=0.0375 radius=0.0511",
            "New CNData.cnb like=cna",
        ],
    },
    {
        "name": "tsdata_default",
        "target": "TSData.ts0",
        "commands": ["New TSData.ts0"],
    },
    {
        "name": "tsdata_full",
        "target": "TSData.ts1",
        "commands": [
            "New TSData.ts1 DiaShield=0.88 TapeLayer=0.005 TapeLap=20 EpsR=2.3 "
            "InsLayer=0.22 DiaIns=0.82 DiaCable=0.88 Rdc=0.0997 GMRac=0.0375 "
            "radius=0.0511 normamps=300"
        ],
    },
    {
        "name": "tsdata_makelike",
        "target": "TSData.tsb",
        "commands": [
            "New TSData.tsa DiaShield=0.88 TapeLayer=0.005 TapeLap=20 EpsR=2.3 "
            "InsLayer=0.22 DiaIns=0.82 DiaCable=0.88 Rdc=0.0997 GMRac=0.0375 "
            "radius=0.0511",
            "New TSData.tsb like=tsa",
        ],
    },
    {
        # MakeLike copies neither NumAmpRatings nor AmpRatings: a like= wire
        # keeps its own default Seasons=1 / Ratings=[ -1] even though the
        # source set Seasons=2 / Ratings=(600 800).
        "name": "wiredata_makelike_ratings",
        "target": "WireData.wml",
        "commands": [
            "New WireData.wmlsrc Rdc=0.0526 radius=0.0306 Seasons=2 Ratings=(600 800)",
            "New WireData.wml like=wmlsrc",
        ],
    },
    {
        # Same MakeLike quirk for CNData: cable + strand fields copy, but the
        # source's Seasons=2 / Ratings=(600 800) do not.
        "name": "cndata_makelike_ratings",
        "target": "CNData.cnml",
        "commands": [
            "New CNData.cnmlsrc k=16 DiaStrand=0.064 GmrStrand=0.0208 EpsR=2.3 "
            "InsLayer=0.22 DiaIns=1.06 DiaCable=1.16 Rdc=0.0997 radius=0.0511 "
            "Seasons=2 Ratings=(600 800)",
            "New CNData.cnml like=cnmlsrc",
        ],
    },
    {
        # Fewer Ratings tokens than Seasons: the DoubleDArray parse zero-fills
        # the tail and leaves Seasons unchanged.
        "name": "wiredata_ratings_short",
        "target": "WireData.wrs",
        "commands": ["New WireData.wrs Rdc=0.05 radius=0.03 Seasons=3 Ratings=(600)"],
    },
    {
        # More Ratings tokens than Seasons: the parse caps at Seasons tokens.
        "name": "wiredata_ratings_long",
        "target": "WireData.wrl",
        "commands": ["New WireData.wrl Rdc=0.05 radius=0.03 Seasons=2 Ratings=(600 800 900)"],
    },
    {
        # k < 2 logs a critical error, but the object is still created and the
        # offending k=1 is stored (not rejected). allow_errors captures the dump.
        "name": "cndata_k_too_few",
        "target": "CNData.cnk",
        "commands": [
            "New CNData.cnk k=1 DiaStrand=0.064 EpsR=2.3 InsLayer=0.22 "
            "DiaIns=1.06 DiaCable=1.16 Rdc=0.0997 radius=0.0511"
        ],
        "allow_errors": True,
    },
    {
        # TapeLap out of [0,100] logs an error; the value is stored, not clamped.
        "name": "tsdata_tapelap_range",
        "target": "TSData.tsr",
        "commands": [
            "New TSData.tsr DiaShield=0.88 TapeLayer=0.005 TapeLap=150 EpsR=2.3 "
            "InsLayer=0.22 DiaIns=0.82 DiaCable=0.88 Rdc=0.0997 radius=0.0511"
        ],
        "allow_errors": True,
    },
    {
        # EpsR < 1 logs a permittivity error; the value is stored as given.
        "name": "cndata_low_epsr",
        "target": "CNData.cne",
        "commands": [
            "New CNData.cne k=13 DiaStrand=0.064 EpsR=0.5 InsLayer=0.22 "
            "DiaIns=1.06 DiaCable=1.16 Rdc=0.0997 radius=0.0511"
        ],
        "allow_errors": True,
    },
    # ----- LineSpacing -----------------------------------------------------
    {
        "name": "linespacing_default",
        "target": "LineSpacing.ls1",
        "commands": ["New LineSpacing.ls1"],
    },
    {
        "name": "linespacing_full",
        "target": "LineSpacing.ls1",
        "commands": [
            "New LineSpacing.ls1 nconds=3 nphases=3 x=(-1.2909 0 1.2909) "
            "h=(28.6 28.6 28.6) units=ft"
        ],
    },
    {
        "name": "linespacing_units_m",
        "target": "LineSpacing.ls1",
        "commands": [
            "New LineSpacing.ls1 nconds=2 nphases=1 x=(0 0.5) h=(10 10) units=m"
        ],
    },
    {
        # x/h are sized by nconds: extra tokens dropped, missing ones zero-fill.
        "name": "linespacing_array_clamp",
        "target": "LineSpacing.ls1",
        "commands": [
            "New LineSpacing.ls1 nconds=3 nphases=3 x=(-1.2 0 1.2 9.9) h=(28)"
        ],
    },
    {
        # Shrinking nconds truncates x/h (realloc preserves the leading
        # entries) and the side effect resets units to ft.
        "name": "linespacing_shrink_nconds",
        "target": "LineSpacing.ls1",
        "commands": [
            "New LineSpacing.ls1 nconds=4 nphases=3 x=(-1.2 0 1.2 0) "
            "h=(28 28 28 24) units=m",
            "Edit LineSpacing.ls1 nconds=2",
        ],
    },
    {
        "name": "linespacing_makelike",
        "target": "LineSpacing.ls1",
        "commands": [
            "New LineSpacing.base nconds=4 nphases=3 x=(-1.2 0 1.2 0) "
            "h=(28 28 28 24) units=m",
            "New LineSpacing.ls1 like=base",
        ],
    },
    {
        # nconds=0 frees the coordinate buffers: Pascal `ReAllocmem(FX, 0)` nils
        # the pointer, so X/H dump as the empty string '' (not '[]').
        "name": "linespacing_zero_nconds",
        "target": "LineSpacing.ls1",
        "commands": ["New LineSpacing.ls1 nconds=0"],
    },
    {
        "name": "linespacing_units_mi",
        "target": "LineSpacing.ls1",
        "commands": ["New LineSpacing.ls1 nconds=3 x=(1 2 3) h=(4 5 6) units=mi"],
    },
    {
        "name": "linespacing_units_kft",
        "target": "LineSpacing.ls1",
        "commands": ["New LineSpacing.ls1 nconds=3 x=(1 2 3) h=(4 5 6) units=kft"],
    },
    {
        "name": "linespacing_units_km",
        "target": "LineSpacing.ls1",
        "commands": ["New LineSpacing.ls1 nconds=3 x=(1 2 3) h=(4 5 6) units=km"],
    },
    {
        "name": "linespacing_units_none",
        "target": "LineSpacing.ls1",
        "commands": ["New LineSpacing.ls1 nconds=3 x=(1 2 3) h=(4 5 6) units=none"],
    },
    {
        "name": "linespacing_units_in",
        "target": "LineSpacing.ls1",
        "commands": ["New LineSpacing.ls1 nconds=3 x=(1 2 3) h=(4 5 6) units=in"],
    },
    {
        "name": "linespacing_units_cm",
        "target": "LineSpacing.ls1",
        "commands": ["New LineSpacing.ls1 nconds=3 x=(1 2 3) h=(4 5 6) units=cm"],
    },
    {
        "name": "linespacing_units_mm",
        "target": "LineSpacing.ls1",
        "commands": ["New LineSpacing.ls1 nconds=3 x=(1 2 3) h=(4 5 6) units=mm"],
    },
    # ----- LineGeometry (WP7.1 step 2c) ------------------------------------
    {
        # Default geometry has NConds=0, so X/H/Units have no active conductor
        # to read — the oracle raises an access violation on those getters
        # (genuine UB on the unallocated FX/FY/FUnits pointers), so they are
        # skipped; everything else dumps cleanly (Wires/CNCables '[]', etc.).
        "name": "linegeometry_default",
        "target": "LineGeometry.g1",
        "commands": ["New LineGeometry.g1"],
        "skip_props": ["X", "H", "Units"],
    },
    {
        # Overhead 4-wire via the cond/wire state machine: 3 phase ACSR + a
        # neutral, reduced out (reduce=y). NormAmps/EmergAmps default from the
        # first conductor; the `?` getters see the last active conductor (4).
        "name": "linegeometry_oh",
        "target": "LineGeometry.g1",
        "commands": [
            "New WireData.acsr Rdc=0.0526 GMRac=0.0244 GMRunits=ft radius=0.0306 "
            "radunits=ft normamps=530 Runits=ft",
            "New WireData.cn Rdc=0.0526 GMRac=0.00814 GMRunits=ft radius=0.0204 "
            "radunits=ft normamps=340 Runits=ft",
            "New LineGeometry.g1 nconds=4 nphases=3 "
            "cond=1 wire=acsr x=-1.2909 h=13.716 units=m "
            "cond=2 wire=acsr x=0 h=13.716 "
            "cond=3 wire=acsr x=1.2909 h=13.716 "
            "cond=4 wire=cn x=0 h=14.6304 reduce=y",
        ],
    },
    {
        # Spacing form: a LineSpacing supplies the coordinates; the wires are
        # the plural array form. Spacing= copies X/H/Units into every conductor.
        "name": "linegeometry_spacing",
        "target": "LineGeometry.g1",
        "commands": [
            "New WireData.acsr Rdc=0.0526 GMRac=0.0244 GMRunits=ft radius=0.0306 "
            "radunits=ft normamps=530 Runits=ft",
            "New LineSpacing.sp nconds=3 nphases=3 x=(-1.2909 0 1.2909) "
            "h=(28.6 28.6 28.6) units=ft",
            "New LineGeometry.g1 nconds=3 nphases=3 spacing=sp wires=[acsr acsr acsr]",
        ],
    },
    {
        # Concentric-neutral cable via cncable=; sets the engine kind to CN and
        # defaults the ratings from the cable.
        "name": "linegeometry_cn",
        "target": "LineGeometry.g1",
        "commands": [
            "New CNData.cn1 k=16 DiaStrand=0.064 GmrStrand=0.0208 Rstrand=0.0145 "
            "EpsR=2.3 InsLayer=0.22 DiaIns=1.06 DiaCable=1.16 Rdc=0.0997 "
            "GMRac=0.0375 radius=0.0511 Runits=in radunits=in gmrunits=in normamps=350",
            "New LineGeometry.g1 nconds=3 nphases=3 "
            "cond=1 cncable=cn1 x=-0.5 h=-4 units=ft "
            "cond=2 cncable=cn1 x=0 h=-4 "
            "cond=3 cncable=cn1 x=0.5 h=-4",
        ],
    },
    {
        # MakeLike copies the full geometry; the nconds side effect on the
        # derived object resets ActiveCond to 1, so the `?` getters show cond 1.
        "name": "linegeometry_makelike",
        "target": "LineGeometry.g1",
        "commands": [
            "New WireData.acsr Rdc=0.0526 GMRac=0.0244 GMRunits=ft radius=0.0306 "
            "radunits=ft normamps=530 Runits=ft",
            "New LineGeometry.base nconds=3 nphases=3 "
            "cond=1 wire=acsr x=-1.29 h=13.7 units=m "
            "cond=2 wire=acsr x=0 h=13.7 "
            "cond=3 wire=acsr x=1.29 h=13.7 reduce=y",
            "New LineGeometry.g1 like=base",
        ],
    },
    {
        # Multi-season conductor: the geometry inherits Seasons/Ratings (and
        # NormAmps/EmergAmps) from the first conductor when its own are unset,
        # exercising the NumAmpRatings>1 / AmpRatings-copy defaulting branches.
        "name": "linegeometry_ratings",
        "target": "LineGeometry.g1",
        "commands": [
            "New WireData.w4 Rdc=0.0526 GMRac=0.0244 GMRunits=ft radius=0.0306 "
            "radunits=ft normamps=530 Runits=ft Seasons=4 Ratings=(400 450 500 550)",
            "New LineGeometry.g1 nconds=3 nphases=3 "
            "cond=1 wire=w4 x=-1 h=10 units=m "
            "cond=2 wire=w4 x=0 h=10 cond=3 wire=w4 x=1 h=10",
        ],
    },
    {
        # Buried-neutral: CN phase cables + an overhead neutral added via the
        # plural wires= with the active conductor still a cable, so SetWires
        # takes the istart=NPhases+1 branch (expected = NConds-NPhases = 1).
        "name": "linegeometry_buried",
        "target": "LineGeometry.g1",
        "commands": [
            "New CNData.cn1 k=16 DiaStrand=0.064 GmrStrand=0.0208 Rstrand=0.0145 "
            "EpsR=2.3 InsLayer=0.22 DiaIns=1.06 DiaCable=1.16 Rdc=0.0997 "
            "GMRac=0.0375 radius=0.0511 Runits=in radunits=in gmrunits=in normamps=350",
            "New WireData.acsr Rdc=0.0526 GMRac=0.0244 GMRunits=ft radius=0.0306 "
            "radunits=ft normamps=530 Runits=ft",
            "New LineGeometry.g1 nconds=3 nphases=2 "
            "cond=1 cncable=cn1 x=-0.5 h=-4 units=ft "
            "cond=2 cncable=cn1 x=0.5 h=-4 wires=[acsr]",
        ],
    },
    {
        # Tape-shield cable via the scalar tscable= state machine + a non-default
        # LineType (ug_ts). Exercises TSData resolution, the TapeShield engine
        # kind, NormAmps/EmergAmps defaulting from the TSData (the TsDataObj amps
        # path), and the LineType enum binding for a non-oh value.
        "name": "linegeometry_ts",
        "target": "LineGeometry.g1",
        "commands": [
            "New TSData.ts1 DiaShield=0.88 TapeLayer=0.005 TapeLap=20 EpsR=2.3 "
            "InsLayer=0.22 DiaIns=0.82 DiaCable=0.88 Rdc=0.0997 GMRac=0.0375 "
            "radius=0.0511 normamps=300",
            "New LineGeometry.g1 nconds=3 nphases=3 linetype=ug_ts "
            "cond=1 tscable=ts1 x=-0.5 h=-4 units=ft "
            "cond=2 tscable=ts1 x=0 h=-4 cond=3 tscable=ts1 x=0.5 h=-4",
        ],
    },
    {
        # nphases > nconds: the oracle stores NPhases raw at parse time (the
        # FLineData.Nphases clamp is a solve-time/UpdateLineGeometryData step, not
        # a property side effect), so NPhases reads back 3 even though NConds=2.
        "name": "linegeometry_nphases_gt_nconds",
        "target": "LineGeometry.g1",
        "commands": [
            "New WireData.acsr Rdc=0.0526 GMRac=0.0244 GMRunits=ft radius=0.0306 "
            "radunits=ft normamps=530 Runits=ft",
            "New LineGeometry.g1 nconds=2 nphases=3 "
            "cond=1 wire=acsr x=-1 h=10 cond=2 wire=acsr x=1 h=10",
        ],
    },
    {
        # Seasons/Ratings set directly on the geometry (not inherited from a
        # conductor): the Seasons side effect resizes Ratings to the new count,
        # then Ratings fills it. The wire's own Seasons=1 does not override.
        "name": "linegeometry_seasons_direct",
        "target": "LineGeometry.g1",
        "commands": [
            "New WireData.acsr Rdc=0.0526 GMRac=0.0244 GMRunits=ft radius=0.0306 "
            "radunits=ft normamps=530 Runits=ft",
            "New LineGeometry.g1 nconds=3 nphases=3 "
            "cond=1 wire=acsr x=-1 h=10 units=m cond=2 wire=acsr x=0 h=10 "
            "cond=3 wire=acsr x=1 h=10 Seasons=2 Ratings=(111 222)",
        ],
    },
    {
        # Explicit NormAmps/EmergAmps survive a later conductor: the amps
        # defaulting only fills when the geometry's own value is still 0, so the
        # wire's 530/795 does not overwrite the explicit 999/888.
        "name": "linegeometry_normamps_explicit",
        "target": "LineGeometry.g1",
        "commands": [
            "New WireData.acsr Rdc=0.0526 GMRac=0.0244 GMRunits=ft radius=0.0306 "
            "radunits=ft normamps=530 Runits=ft",
            "New LineGeometry.g1 nconds=3 nphases=3 normamps=999 emergamps=888 "
            "cond=1 wire=acsr x=-1 h=10 cond=2 wire=acsr x=0 h=10 "
            "cond=3 wire=acsr x=1 h=10",
        ],
    },
    # --- Storage (WP7.4 step 1) --------------------------------------------
    # `%Idlingkvar` is DeprecatedAndRemoved (dumps '' regardless); the State /
    # %Stored / kW getters reflect the state machine. Defaults assume a fully
    # charged, idling battery.
    {
        "name": "storage_default",
        "target": "Storage.s1",
        "commands": ["New Storage.s1 bus1=b kV=12.47"],
    },
    {
        # Discharging at 50% of rated: State=Discharging, %Discharge=50.
        "name": "storage_discharge",
        "target": "Storage.s1",
        "commands": [
            "New Storage.s1 bus1=b kV=12.47 kWrated=500 kWhrated=1000 "
            "state=discharging %discharge=50 pf=0.95",
        ],
    },
    {
        # Charging: drop %stored so it is not full, then charge at 40%.
        "name": "storage_charge",
        "target": "Storage.s1",
        "commands": [
            "New Storage.s1 bus1=b kV=12.47 kWrated=500 kWhrated=1000 "
            "%stored=50 state=charging %charge=40",
        ],
    },
    {
        # kvar mode + the inverter limits (kvarMax/kvarMaxAbs/%Cutin/%Cutout).
        "name": "storage_kvar_limits",
        "target": "Storage.s1",
        "commands": [
            "New Storage.s1 bus1=b kV=12.47 kWrated=500 kVA=600 kWhrated=1000 "
            "state=discharging kvar=200 kvarMax=300 kvarMaxAbs=250 "
            "%Cutin=10 %Cutout=5 %PMinNoVars=5 %PMinkvarMax=20 "
            "WattPriority=yes PFPriority=yes",
        ],
    },
    {
        # An efficiency curve + idling/charge/discharge efficiencies + reserve.
        "name": "storage_effcurve",
        "target": "Storage.s1",
        "commands": [
            "New XYcurve.eff npts=4 xarray=(0.1 0.2 0.4 1.0) yarray=(0.86 0.9 0.93 0.97)",
            "New Storage.s1 bus1=b kV=12.47 kWrated=500 kWhrated=1000 "
            "EffCurve=eff %EffCharge=95 %EffDischarge=92 %IdlingkW=2 %Reserve=15",
        ],
    },
    {
        # Follow-dispatch with daily shape + charge/discharge triggers.
        "name": "storage_dispatch_shapes",
        "target": "Storage.s1",
        "commands": [
            "New LoadShape.sd npts=4 interval=1 mult=(-1 0 0.5 1)",
            "New Storage.s1 bus1=b kV=12.47 kWrated=500 kWhrated=1000 "
            "dispmode=follow daily=sd DischargeTrigger=0.5 ChargeTrigger=0.2 "
            "TimeChargeTrig=3 model=2 balanced=yes limitcurrent=yes "
            "vminpu=0.85 vmaxpu=1.15 %R=1 %X=40",
        ],
    },
    {
        # MakeLike copies essentially every field (the derived object overrides
        # only Bus1).
        "name": "storage_makelike",
        "target": "Storage.s1",
        "commands": [
            "New Storage.base bus1=b kV=12.47 kWrated=500 kWhrated=1000 "
            "state=discharging %discharge=80 pf=0.9 %Reserve=25 "
            "DischargeTrigger=0.6 dispmode=follow",
            "New Storage.s1 like=base bus1=c",
        ],
    },
    # --- IndMach012 (WP7.7 step 3a) ----------------------------------------
    # The symmetrical-component induction machine. `pf` is read-only (computed
    # PowerFactor(Power[1]) → 1 on an unsolved circuit); `slip` write goes through
    # set_Localslip (clamped to ±MaxSlip outside dynamics); `conn` defaults to
    # delta, `SlipOption` to VariableSlip.
    {
        "name": "indmach012_default",
        "target": "IndMach012.m1",
        "commands": ["New IndMach012.m1 bus1=b"],
    },
    {
        "name": "indmach012_full",
        "target": "IndMach012.m1",
        "commands": [
            "New IndMach012.m1 bus1=mbus kV=0.48 kW=1200 conn=delta kVA=1500 H=6 "
            "D=2 puRs=0.048 puXs=0.075 puRr=0.018 puXr=0.12 puXm=3.8 "
            "slip=0.02 MaxSlip=0.12 SlipOption=variableslip",
        ],
    },
    {
        # Wye connection (no neutral) + fixed-slip option.
        "name": "indmach012_wye_fixedslip",
        "target": "IndMach012.m1",
        "commands": [
            "New IndMach012.m1 bus1=b phases=3 kV=0.48 kW=500 conn=wye kVA=600 "
            "slip=0.03 SlipOption=fixedslip",
        ],
    },
    {
        # The slip clamp: a slip above MaxSlip is clamped to MaxSlip (set_Localslip).
        "name": "indmach012_slip_clamp",
        "target": "IndMach012.m1",
        "commands": ["New IndMach012.m1 bus1=b kV=0.48 kW=300 MaxSlip=0.08 slip=0.5"],
    },
    {
        # MakeLike copies the MachineData record (so D *is* copied) + pu*/MaxSlip,
        # but deliberately NOT Slip/SlipOption/Conn. `base` sets non-default values
        # for all of these so the dump distinguishes copied (D=3) from non-copied
        # (m1 reads back the ctor defaults Slip=0.007 / VariableSlip / delta).
        "name": "indmach012_makelike",
        "target": "IndMach012.m1",
        "commands": [
            "New IndMach012.base bus1=b kV=0.48 kW=900 kVA=1100 H=4 D=3 "
            "puRs=0.05 puXs=0.08 puRr=0.02 puXr=0.13 puXm=3.5 MaxSlip=0.11 "
            "conn=wye slip=0.05 SlipOption=fixedslip",
            "New IndMach012.m1 like=base bus1=c",
        ],
    },
    # --- VSConverter (WP7.8) -----------------------------------------------
    # A 2-terminal AC/DC bridge PC element. Defaults: phases=4, Ndc=1, kVac=kVdc=
    # kW=1, m0=0.5, Mmin=0.1, Mmax=0.9, Iacmax=Idcmax=2, VscMode=Fixed. Setting
    # bus1 defaults bus2 to the grounded node list (Bus1base.0.0.0.0).
    {
        "name": "vsconverter_default",
        "target": "VSConverter.v1",
        "commands": ["New VSConverter.v1 bus1=b.1.2.3.4"],
    },
    {
        "name": "vsconverter_full",
        "target": "VSConverter.v1",
        "commands": [
            "New VSConverter.v1 phases=4 Ndc=1 bus1=b.1.2.3.4 kVac=0.48 kVdc=1.0 "
            "kW=50 Rac=0.05 Xac=0.2 m0=0.6 d0=5 Mmin=0.2 Mmax=0.95 Iacmax=1.5 "
            "Idcmax=1.5 Vacref=277 Pacref=50 Qacref=10 Vdcref=1000 VscMode=PacQac",
        ],
    },
    {
        "name": "vsconverter_makelike",
        "target": "VSConverter.v1",
        "commands": [
            "New VSConverter.base phases=4 Ndc=1 bus1=b.1.2.3.4 kVac=0.48 kVdc=1.2 "
            "kW=75 Rac=0.03 Xac=0.15 m0=0.7 VscMode=VdcVac",
            "New VSConverter.v1 like=base bus1=c.1.2.3.4",
        ],
    },
    # --- VCCS (Phase 7) -----------------------------------------------------
    # The HW-inverter voltage-controlled current source. The bp1/bp2/filter
    # curves are XYcurve object references (the dump shows the referenced curve's
    # name); RMSMode is a Boolean. Defaults: phases=1, Prated=250, Vrated=208,
    # Ppct=100, FSample=5000, RMSMode=No, IMaxpu=1.1, VRMSTau=IRMSTau=0.0015,
    # Spectrum=default. MakeLike copies the curve refs + the rating/filter scalars
    # (but, per Pascal, not via RecalcElementData side effects).
    {
        "name": "vccs_default",
        "target": "VCCS.v1",
        "commands": ["New VCCS.v1 bus1=b"],
    },
    {
        "name": "vccs_full",
        "target": "VCCS.v1",
        "commands": [
            "New XYcurve.bp1 npts=3 xarray=[-0.82 0 0.82] yarray=[-0.788 0 0.788]",
            "New XYcurve.bp2 npts=5 xarray=[-0.4 -0.225 0 0.225 0.4] yarray=[2.5 1 0 -1 -2.5]",
            "New XYcurve.zf npts=3 xarray=[1.0 -1.9852 0.9853] yarray=[0.0 0.0148 -0.0147]",
            "New VCCS.v1 bus1=b phases=1 prated=3000 vrated=208 ppct=100 "
            "bp1=bp1 bp2=bp2 filter=zf fsample=10000 rmsmode=true imaxpu=1.15 "
            "vrmstau=0.01 irmstau=0.05",
        ],
    },
    {
        # 3-phase rating + the all-default filter (no curves): exercises the
        # phases side effect (NConds := Fnphases) and the no-filter path.
        "name": "vccs_3phase",
        "target": "VCCS.v1",
        "commands": ["New VCCS.v1 bus1=b phases=3 prated=3000 vrated=360 ppct=50"],
    },
    {
        # MakeLike copies the curve references (the derived dump shows the same
        # curve names) plus the rating/filter scalars + RMSMode.
        "name": "vccs_makelike",
        "target": "VCCS.v1",
        "commands": [
            "New XYcurve.bp1 npts=3 xarray=[-0.82 0 0.82] yarray=[-0.788 0 0.788]",
            "New XYcurve.bp2 npts=5 xarray=[-0.4 -0.225 0 0.225 0.4] yarray=[2.5 1 0 -1 -2.5]",
            "New XYcurve.zf npts=3 xarray=[1.0 -1.9852 0.9853] yarray=[0.0 0.0148 -0.0147]",
            "New VCCS.base bus1=b phases=1 prated=190 vrated=208 ppct=89.5 "
            "bp1=bp1 bp2=bp2 filter=zf fsample=10000 rmsmode=true imaxpu=1.2 "
            "vrmstau=0.02 irmstau=0.03",
            "New VCCS.v1 like=base bus1=c",
        ],
    },
    # --- UPFC (Phase 7) -----------------------------------------------------
    # A two-terminal voltage-regulating PC element (Bus1 input, Bus2 output).
    # Defaults: refkV=0.24, PF=1, Frequency=60, Phases=1, Xs=0.754, Tol1=0.02,
    # Mode=1 (the MappedIntEnum dumps the integer ordinal), VpqMax=24, VHLimit=300,
    # VLLimit=125, CLimit=265, refkV2=0, kvarLimit=5, Spectrum=default. LossCurve is
    # an XYcurve object ref; Element is a monitored circuit element (PF modes).
    # NOTE: there is **no MakeLike scenario** — creating a *second* UPFC triggers an
    # upstream access violation (`TUPFCObj.Create` casts the first UPFC object to a
    # TUPFCControlObj to clear its list; UPFC.pas l.396), so any `like=` (or any
    # multi-UPFC circuit) crashes the oracle. The Rust port does not reproduce that
    # UB (it cannot be expressed in safe Rust), so a single UPFC is the only
    # comparable state.
    {
        "name": "upfc_default",
        "target": "UPFC.u1",
        "commands": ["New UPFC.u1 bus1=a bus2=b"],
    },
    {
        "name": "upfc_full",
        "target": "UPFC.u1",
        "commands": [
            "New XYcurve.lc npts=3 xarray=[0.9 1 1.1] yarray=[1.01 1.0 1.01]",
            "New Line.mon bus1=x bus2=y phases=1 r1=1 x1=1 length=1",
            "New UPFC.u1 bus1=ba bus2=bb refkV=0.48 PF=0.95 Frequency=50 Phases=1 "
            "Xs=0.05 Tol1=0.005 mode=3 VpqMax=30 LossCurve=lc VHLimit=320 VLLimit=110 "
            "CLimit=300 refkV2=0.46 kvarLimit=8 Element=Line.mon",
        ],
    },
    # --- UPFCControl (Phase 7) ----------------------------------------------
    # Drives a UPFC fleet. The only class property is UPFCList (a StringList that
    # round-trips but never actually filters the fleet — ListSize is never set from
    # it upstream). MakeLike copies only the phase/terminal/element refs (not the
    # list); it also logs a #749 "Invalid number of terminals" on the derived
    # object (the control carries no terminals), captured via allow_errors.
    {
        "name": "upfccontrol_default",
        "target": "UPFCControl.c1",
        "commands": ["New UPFCControl.c1"],
    },
    {
        "name": "upfccontrol_list",
        "target": "UPFCControl.c1",
        "commands": ["New UPFCControl.c1 UPFCList=[ua, ub]"],
    },
    {
        "name": "upfccontrol_makelike",
        "target": "UPFCControl.c1",
        "allow_errors": True,
        "commands": [
            "New UPFCControl.base UPFCList=[ua, ub]",
            "New UPFCControl.c1 like=base",
        ],
    },
    # --- ESPVLControl (Phase 8) ---------------------------------------------
    # An Energy-Storage/PV local controller. There is NO kWLimit property
    # (FkWLimit is hardcoded 8000, unsettable); the default Type (Ftype=0) dumps
    # ''; kvarLimit defaults to FkWLimit/2 = 4000. The three subordinate lists +
    # their IndirectCount weight arrays round-trip (a list set without weights
    # leaves the weights NIL → dumps ''). MakeLike copies only the
    # phase/terminal/monitored refs (Type/bands/lists revert to ctor defaults).
    {
        "name": "espvlcontrol_default",
        "target": "ESPVLControl.e1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.1 x1=0.2 length=1",
            "New ESPVLControl.e1 element=Line.l1",
        ],
    },
    {
        "name": "espvlcontrol_full",
        "target": "ESPVLControl.e1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.1 x1=0.2 length=1",
            "New ESPVLControl.e1 element=Line.l1 terminal=1 type=LocalController "
            "kWBand=250 kvarlimit=1500 PVSystemList=[pv1, pv2] PVSystemWeights=[2, 3] "
            "StorageList=[st1] StorageWeights=[5] LocalControlList=[lc1, lc2] "
            "LocalControlWeights=[1.5, 2.5]",
        ],
    },
    {
        # SystemController with a LocalControlList but no explicit weights: the
        # side-effect levels uniform 1.0 weights (dumps '[ 1 1 1]').
        "name": "espvlcontrol_system",
        "target": "ESPVLControl.e1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.1 x1=0.2 length=1",
            "New ESPVLControl.e1 element=Line.l1 terminal=1 type=SystemController "
            "LocalControlList=[lc1, lc2, lc3]",
        ],
    },
    {
        "name": "espvlcontrol_makelike",
        "target": "ESPVLControl.e1",
        "commands": [
            "New Line.l1 bus1=b1 bus2=b2 phases=3 r1=0.1 x1=0.2 length=1",
            "New Line.l2 bus1=b2 bus2=b3 phases=3 r1=0.1 x1=0.2 length=1",
            "New ESPVLControl.base element=Line.l1 terminal=1 type=LocalController "
            "kWBand=300 kvarlimit=2000 PVSystemList=[pv1] PVSystemWeights=[7]",
            "New ESPVLControl.e1 like=base element=Line.l2",
        ],
    },
    # --- DynamicExp (WP7.3 step 0) -----------------------------------------
    # Setting Expression compiles it (InterpretDiffEq); a valid one keeps the
    # verbatim input text, a bad one is cleared. VarNames is lowercased and dumps
    # `[a, b, ...]`; `var=` resolves VarIdx (the active-variable index). Domain
    # defaults to Time (the field), even though the parse default is dq. MakeLike
    # is not implemented (50099) — a `like=` object keeps ctor defaults.
    {
        "name": "dynamicexp_default",
        "target": "DynamicExp.de",
        "commands": ["New DynamicExp.de"],
    },
    {
        # The vendored corpus expression (Dynamic_KundurDynExp.dss).
        "name": "dynamicexp_full",
        "target": "DynamicExp.de",
        "commands": [
            "New DynamicExp.de nvariables=6 varnames=[Speed Mass PShaft Pterm Damp theta] "
            "expression=[Speed dt = -1 Mass / ( Pterm Damp Speed * + Pshaft - ) *; theta dt = Speed]",
        ],
    },
    {
        "name": "dynamicexp_domain_dq",
        "target": "DynamicExp.de",
        "commands": [
            "New DynamicExp.de nvariables=2 varnames=[w th] domain=dq expression=[w dt = th]",
        ],
    },
    {
        # var=b activates the variable; VarIdx reads back its index (1).
        "name": "dynamicexp_var_active",
        "target": "DynamicExp.de",
        "commands": [
            "New DynamicExp.de nvariables=3 varnames=[a b c] var=b expression=[a dt = b]",
        ],
    },
    {
        # Abbreviated property names (nvar/varnames/expr/dom).
        "name": "dynamicexp_abbrev",
        "target": "DynamicExp.de",
        "commands": ["New DynamicExp.de nvar=2 varnames=[a b] expr=[a dt = b] dom=dq"],
    },
    {
        # An undefined variable on the RHS logs 50005/50003 and clears Expression.
        "name": "dynamicexp_bad_expr",
        "target": "DynamicExp.de",
        "allow_errors": True,
        "commands": ["New DynamicExp.de nvariables=2 varnames=[a b] expression=[a dt = zzz]"],
    },
    {
        # var= referencing a missing variable logs 50001 and clears the active var.
        "name": "dynamicexp_var_missing",
        "target": "DynamicExp.de",
        "allow_errors": True,
        "commands": [
            "New DynamicExp.de nvariables=2 varnames=[a b] var=zzz expression=[a dt = b]",
        ],
    },
    {
        # MakeLike is unimplemented (50099): the derived object keeps ctor defaults.
        "name": "dynamicexp_makelike",
        "target": "DynamicExp.de",
        "allow_errors": True,
        "commands": [
            "New DynamicExp.base nvariables=2 varnames=[a b] expression=[a dt = b]",
            "New DynamicExp.de like=base",
        ],
    },
    {
        # An empty operand before `dt` makes InterpretDiffEq access vars[0] on an
        # empty list — Pascal raises EStringListError ("List index (0) out of
        # bounds"), caught by the command processor: the error is logged and the
        # expression is left as written (the unwind skips the clear-on-error
        # path). The Rust port reproduces it as a recoverable error, not a panic.
        "name": "dynamicexp_empty_dt_operand",
        "target": "DynamicExp.de",
        "allow_errors": True,
        "commands": ["New DynamicExp.de nvariables=2 varnames=[a b] expression=[ dt = b]"],
    },
    # --- InvControl (WP7.5 step 2a; parse-only skeleton) --------------------
    # The smart-inverter control over a PVSystem/Storage fleet. step 2a pins the
    # property table: the seven enums (Mode/CombiMode/Voltage_CurveX_Ref/
    # VoltWattYAxis/RateOfChangeMode/RefReactivePower/ControlModel), the five
    # curves (+ the ValidateXYCurve range checks), the DRC / rate-of-change / AVR
    # scalars, and MakeLike. The DER-fleet build and the Sample dispatch are step
    # 2b. NOTE: an InvControl with an *empty* DERList auto-populates DERNameList
    # from the circuit in RecalcElementData; every fleet scenario therefore names
    # the DER list explicitly (the named branch does not auto-populate), and the
    # MakeLike scenario keeps the circuit DER-free so the derived object's empty
    # list stays empty.
    {
        "name": "invcontrol_default",
        "target": "InvControl.ic1",
        "commands": ["New InvControl.ic1"],
    },
    {
        "name": "invcontrol_voltvar",
        "target": "InvControl.inv1",
        "commands": [
            "New PVSystem.pv1 bus1=b1 kV=12.47 kVA=500 Pmpp=500",
            "New XYcurve.vv npts=4 xarray=(0.5 0.95 1.05 1.5) yarray=(1 1 -1 -1)",
            "New InvControl.inv1 DERList=[pvsystem.pv1] mode=voltvar "
            "voltage_curvex_ref=rated vvc_curve1=vv hysteresis_offset=-0.1 "
            "avgwindowlen=30 deltaq_factor=0.4 VoltageChangeTolerance=0.001 "
            "VarChangeTolerance=0.02 RefReactivePower=varmax",
        ],
    },
    {
        "name": "invcontrol_voltwatt",
        "target": "InvControl.inv1",
        "commands": [
            "New PVSystem.pv1 bus1=b1 kV=12.47 kVA=500 Pmpp=500",
            "New XYcurve.vw npts=4 xarray=(0.5 1.0 1.06 1.5) yarray=(1 1 0.2 0.2)",
            "New InvControl.inv1 DERList=[pvsystem.pv1] mode=voltwatt voltwatt_curve=vw "
            "voltwattyaxis=pmpppu deltap_factor=0.3 ActivePChangeTolerance=0.02 "
            "voltwattch_curve=vw",
        ],
    },
    {
        # A VOLTWATT curve with Y>1 violates ValidateXYCurve: the curve is dropped
        # (VoltWatt_Curve dumps '') and error 381 is logged.
        "name": "invcontrol_voltwatt_badcurve",
        "target": "InvControl.inv1",
        "allow_errors": True,
        "commands": [
            "New PVSystem.pv1 bus1=b1 kV=12.47 kVA=500 Pmpp=500",
            "New XYcurve.bad npts=2 xarray=(1.0 1.1) yarray=(1.5 0.5)",
            "New InvControl.inv1 DERList=[pvsystem.pv1] mode=voltwatt voltwatt_curve=bad",
        ],
    },
    {
        "name": "invcontrol_drc",
        "target": "InvControl.inv1",
        "commands": [
            "New PVSystem.pv1 bus1=b1 kV=12.47 kVA=500 Pmpp=500",
            "New InvControl.inv1 DERList=[pvsystem.pv1] mode=dynamicreaccurr "
            "dbvmin=0.97 dbvmax=1.03 argralowv=0.2 argrahiv=0.15 dynreacavgwindowlen=100",
        ],
    },
    {
        "name": "invcontrol_wattpf",
        "target": "InvControl.inv1",
        "commands": [
            "New PVSystem.pv1 bus1=b1 kV=12.47 kVA=500 Pmpp=500",
            "New XYcurve.wp npts=2 xarray=(0 1) yarray=(1 0.9)",
            "New InvControl.inv1 DERList=[pvsystem.pv1] mode=wattpf wattpf_curve=wp",
        ],
    },
    {
        "name": "invcontrol_wattvar",
        "target": "InvControl.inv1",
        "commands": [
            "New PVSystem.pv1 bus1=b1 kV=12.47 kVA=500 Pmpp=500",
            "New XYcurve.wv npts=3 xarray=(0 0.5 1) yarray=(0 0.2 -0.3)",
            "New InvControl.inv1 DERList=[pvsystem.pv1] mode=wattvar wattvar_curve=wv",
        ],
    },
    {
        # Combi VV_VW: both curves set, Mode stays NONE (CombiMode wins).
        "name": "invcontrol_combi_vvvw",
        "target": "InvControl.inv1",
        "commands": [
            "New PVSystem.pv1 bus1=b1 kV=12.47 kVA=500 Pmpp=500",
            "New XYcurve.vv npts=4 xarray=(0.5 0.95 1.05 1.5) yarray=(1 1 -1 -1)",
            "New XYcurve.vw npts=4 xarray=(0.5 1.0 1.06 1.5) yarray=(1 1 0.2 0.2)",
            "New InvControl.inv1 DERList=[pvsystem.pv1] combimode=vv_vw "
            "vvc_curve1=vv voltwatt_curve=vw",
        ],
    },
    {
        "name": "invcontrol_monbus",
        "target": "InvControl.inv1",
        "commands": [
            "New PVSystem.pv1 bus1=b1 kV=12.47 kVA=500 Pmpp=500",
            "New InvControl.inv1 DERList=[pvsystem.pv1] monvoltagecalc=max "
            "monbus=[b1.1 b1.2] MonBusesVbase=[7.2 7.2]",
        ],
    },
    {
        # LPF rate-of-change with a positive Tau (so it stays active).
        "name": "invcontrol_rateofchange_lpf",
        "target": "InvControl.inv1",
        "commands": [
            "New PVSystem.pv1 bus1=b1 kV=12.47 kVA=500 Pmpp=500",
            "New XYcurve.vv npts=2 xarray=(0.95 1.05) yarray=(1 -1)",
            "New InvControl.inv1 DERList=[pvsystem.pv1] mode=voltvar vvc_curve1=vv "
            "rateofchangemode=lpf lpftau=0.5",
        ],
    },
    {
        "name": "invcontrol_avr",
        "target": "InvControl.inv1",
        "commands": [
            "New PVSystem.pv1 bus1=b1 kV=12.47 kVA=500 Pmpp=500",
            "New InvControl.inv1 DERList=[pvsystem.pv1] mode=avr vsetpoint=1.02 "
            "controlmodel=1",
        ],
    },
    {
        # The deprecated PVSystemList prepends "PVSystem." to bare names and writes
        # the DERList backing; both dump the same list.
        "name": "invcontrol_pvsystemlist",
        "target": "InvControl.inv1",
        "commands": [
            "New PVSystem.pv1 bus1=b1 kV=12.47 kVA=500 Pmpp=500",
            "New InvControl.inv1 PVSystemList=[pv1] mode=voltvar",
        ],
    },
    {
        # MakeLike copies the control settings but NOT DERNameList /
        # MonBusesNameList / RefReactivePower / Vsetpoint / ControlModel /
        # ShowEventLog (a Pascal quirk): the derived object keeps ctor defaults
        # for those. The circuit is kept DER-free so the derived empty DERList is
        # not auto-populated.
        "name": "invcontrol_makelike",
        "target": "InvControl.inv1",
        "commands": [
            "New XYcurve.vv npts=4 xarray=(0.5 0.95 1.05 1.5) yarray=(1 1 -1 -1)",
            "New InvControl.base mode=voltvar vvc_curve1=vv dbvmin=0.97 dbvmax=1.03 "
            "deltaq_factor=0.4 voltwattyaxis=pmpppu eventlog=yes RefReactivePower=varmax "
            "vsetpoint=1.05 controlmodel=1",
            "New InvControl.inv1 like=base",
        ],
    },
    # The seven scenarios below pin the remaining ValidateXYCurve nulling arms and
    # the un-pinned enum reverse-render slots (audit-tests follow-up). They are
    # DER-free (no PVSystem, empty DERList) so the deferred-recalc auto-populate
    # divergence cannot occur (an empty DERList stays empty with no DER present).
    {
        # WATTPF curve with Y outside [-1,1]: ValidateXYCurve drops it (WattPF_Curve
        # dumps '') + error 381 — the WATTPF arm, distinct band/message from VOLTWATT.
        "name": "invcontrol_wattpf_badcurve",
        "target": "InvControl.ic",
        "allow_errors": True,
        "commands": [
            "New XYcurve.bad npts=2 xarray=(0 1) yarray=(1.5 0.5)",
            "New InvControl.ic mode=wattpf wattpf_curve=bad",
        ],
    },
    {
        # WATTVAR curve with Y outside [-1,1]: dropped (WattVar_Curve '') + 381.
        "name": "invcontrol_wattvar_badcurve",
        "target": "InvControl.ic",
        "allow_errors": True,
        "commands": [
            "New XYcurve.bad npts=2 xarray=(0 1) yarray=(0.2 -1.5)",
            "New InvControl.ic mode=wattvar wattvar_curve=bad",
        ],
    },
    {
        # VoltWattCH curve out of [0,1]: shares the VOLTWATT validation arm, dropped
        # (VoltWattCH_Curve '') + 381.
        "name": "invcontrol_voltwattch_badcurve",
        "target": "InvControl.ic",
        "allow_errors": True,
        "commands": [
            "New XYcurve.bad npts=2 xarray=(1.0 1.1) yarray=(1.5 0.5)",
            "New InvControl.ic mode=voltwatt voltwattch_curve=bad",
        ],
    },
    {
        # VOLTVAR (VVC_Curve1) is NOT range-checked: a Y of 2.0 is kept (VVC_Curve1
        # still dumps the curve name) — the unchecked path, distinct from the three
        # validated arms.
        "name": "invcontrol_voltvar_unchecked_curve",
        "target": "InvControl.ic",
        "commands": [
            "New XYcurve.big npts=2 xarray=(0.95 1.05) yarray=(2 -2)",
            "New InvControl.ic mode=voltvar vvc_curve1=big",
        ],
    },
    {
        # Combi VV_DRC + the un-pinned enum slots: Voltage_CurveX_Ref=RAvg,
        # VoltWattYAxis=KVARatingPU, RateOfChangeMode=RiseFall (a non-default
        # RiseFallLimit keeps it active), MonVoltageCalc=min.
        "name": "invcontrol_combi_vvdrc",
        "target": "InvControl.ic",
        "commands": [
            "New InvControl.ic combimode=vv_drc voltage_curvex_ref=ravg "
            "voltwattyaxis=kvaratingpu rateofchangemode=risefall risefalllimit=0.05 "
            "monvoltagecalc=min",
        ],
    },
    {
        # VoltWattYAxis=PctPMPPPU + Voltage_CurveX_Ref=Avg (the remaining slots).
        "name": "invcontrol_voltwatt_pctpmpp",
        "target": "InvControl.ic",
        "commands": [
            "New XYcurve.vw npts=2 xarray=(1.0 1.1) yarray=(1 0.2)",
            "New InvControl.ic mode=voltwatt voltwatt_curve=vw voltwattyaxis=pctpmpppu "
            "voltage_curvex_ref=avg",
        ],
    },
    {
        # Mode=GFM + VoltWattYAxis=PAvailablePU (the last two enum slots).
        "name": "invcontrol_gfm",
        "target": "InvControl.ic",
        "commands": [
            "New InvControl.ic mode=gfm voltwattyaxis=pavailablepu",
        ],
    },
    {
        # The IntervalUnits time-unit suffix on AvgWindowLen / DynReacAvgWindowLen.
        # The dump renders the converted integer (seconds), so these three scenarios
        # pin every suffix's conversion against the oracle on BOTH properties:
        #   s => x1, m => x60, h => x3600 (a bare number is seconds).
        "name": "invcontrol_window_units_s",
        "target": "InvControl.ic",
        "commands": [
            "New InvControl.ic avgwindowlen=2s dynreacavgwindowlen=30s",
        ],
    },
    {
        "name": "invcontrol_window_units_m",
        "target": "InvControl.ic",
        "commands": [
            "New InvControl.ic avgwindowlen=2m dynreacavgwindowlen=5m",
        ],
    },
    {
        "name": "invcontrol_window_units_h",
        "target": "InvControl.ic",
        "commands": [
            "New InvControl.ic avgwindowlen=1h dynreacavgwindowlen=2h",
        ],
    },
    # --- ExpControl (WP7.5 step 3) -----------------------------------------
    # The adaptive-Vreg volt-var control over a PVSystem fleet. Pins the 14
    # properties, the PVSystemList ↔ DERList sync (the two share no backing but are
    # kept in lockstep by the side effects), and MakeLike. An ExpControl with an
    # *empty* PVSystemList/DERList scans the circuit for PVSystems on the first
    # Sample (not at parse), so the parse-time dump of an empty list stays empty —
    # the default scenario keeps the circuit DER-free.
    {
        "name": "expcontrol_default",
        "target": "ExpControl.e1",
        "commands": ["New ExpControl.e1"],
    },
    {
        # Full spec on a real PVSystem (the corpus ExpControl example's settings).
        "name": "expcontrol_full",
        "target": "ExpControl.e1",
        "commands": [
            "New PVSystem.pv1 bus1=b1 kV=12.47 kVA=500 Pmpp=500",
            "New ExpControl.e1 derlist=[pvsystem.pv1] deltaq_factor=0.3 vreg=1.0 "
            "slope=22 vregtau=300 tresponse=5 qbias=-0.3 vregmin=0.94 vregmax=1.06 "
            "qmaxlead=0.4 qmaxlag=0.45 preferq=yes eventlog=yes",
        ],
    },
    {
        # The deprecated/companion PVSystemList: bare names that the side effect
        # mirrors into the class-prefixed DERList (both dump in sync).
        "name": "expcontrol_pvsystemlist",
        "target": "ExpControl.e1",
        "commands": [
            "New PVSystem.pv1 bus1=b1 kV=12.47 kVA=500 Pmpp=500",
            "New ExpControl.e1 pvsystemlist=[pv1]",
        ],
    },
    {
        # MakeLike copies the dispatch scalars but NOT Tresponse / ShowEventLog /
        # the name lists (a Pascal quirk): the derived object keeps the ctor
        # defaults for those. The circuit is kept DER-free so the derived empty
        # list is not auto-populated.
        "name": "expcontrol_makelike",
        "target": "ExpControl.e1",
        "commands": [
            "New ExpControl.base vreg=1.02 slope=30 vregtau=600 qbias=-0.2 "
            "vregmin=0.93 vregmax=1.07 qmaxlead=0.4 qmaxlag=0.45 deltaq_factor=0.5 "
            "preferq=yes tresponse=8 eventlog=yes",
            "New ExpControl.e1 like=base",
        ],
    },
    # --- Isource (GAPS_PLAN WPG.14) -----------------------------------------
    # The ideal current source. Defaults: Phases=3, Amps=0, Angle=0,
    # Frequency=60 (BaseFrequency), ScanType=pos, Sequence=pos, Bus2 defaults
    # to Bus1 stripped of nodes + one ".0" per phase (grounded-Y), Spectrum=
    # "default" (TPCElement.DefaultGeneral — NOT "defaultvsource"/"defaultload").
    {
        "name": "isource_default",
        "target": "Isource.i1",
        "commands": ["New Isource.i1 bus1=b1"],
    },
    {
        # Every own property explicit, on a 3-phase unit; Bus2 set AFTER Bus1
        # on the same New command so the explicit value sticks (Isource's
        # PropertySideEffects re-derives the grounded-Y default unconditionally
        # whenever Bus1 is (re)set — see the port's TODO(compat) note).
        "name": "isource_full",
        "target": "Isource.i1",
        "commands": [
            "New Loadshape.ys1 npts=3 interval=1 mult=(1 2 3)",
            "New Loadshape.ds1 npts=2 interval=1 mult=(0.5 1.5)",
            "New Loadshape.du1 npts=4 interval=0.25 mult=(0.1 0.2 0.3 0.4)",
            "New Spectrum.sp1 numharm=2 harmonic=[1 3] %mag=[100 30] angle=[0 15]",
            "New Isource.i1 bus1=b1 bus2=b2 phases=3 amps=25 angle=45 "
            "frequency=55 scantype=zero sequence=neg yearly=ys1 daily=ds1 "
            "duty=du1 spectrum=sp1",
        ],
    },
    {
        # 1-phase: FphaseShift=0 internally (not itself a property), and the
        # default-Bus2 node-stripping keeps only the bus-name part before the
        # first dot.
        "name": "isource_1phase",
        "target": "Isource.i1",
        "commands": ["New Isource.i1 bus1=c1.1 phases=1 amps=12 angle=5"],
    },
    {
        # Setting Daily (with no prior Yearly) mirrors it into Yearly too
        # (Pascal: index 9 = Daily -> `if YearlyShapeObj = NIL then
        # YearlyShapeObj := DailyShapeObj`).
        "name": "isource_daily_defaults_yearly",
        "target": "Isource.i1",
        "commands": [
            "New Loadshape.ds1 npts=2 interval=1 mult=(0.5 1.5)",
            "New Isource.i1 bus1=b1 daily=ds1",
        ],
    },
    {
        # MakeLike copies Amps/Angle/Frequency/ScanType/Sequence/the shape
        # refs/Bus2Defined; the derived object's own Bus1/Bus2 come from its
        # own New command, not the base.
        "name": "isource_makelike",
        "target": "Isource.i1",
        "commands": [
            "New Loadshape.ds1 npts=2 interval=1 mult=(0.5 1.5)",
            "New Isource.base bus1=b1 phases=1 amps=15 angle=10 frequency=55 "
            "scantype=zero sequence=neg daily=ds1",
            "New Isource.i1 like=base bus1=c1",
        ],
    },
]


def check_pin() -> str:
    import dss

    pins = {}
    for line in (REPO_ROOT / "tools" / "golden" / "PIN.txt").read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            k, v = line.split("==")
            pins[k] = v
    if dss.__version__ != pins["dss-python"]:
        sys.exit(
            f"dss-python {dss.__version__} != pinned {pins['dss-python']}; "
            f"install the pinned version or update PIN.txt deliberately"
        )
    return dss.__version__


def run_scenario(d, scenario: dict) -> dict:
    # Some objects log a non-fatal error during their own RecalcElementData even
    # when fully specified — e.g. a StorageController on a circuit with no
    # Storage element always reports 37201 ("No unassigned Storage Elements
    # found"), exactly as the Rust port does. `allow_errors` lets those
    # scenarios capture the property dump past the logged error (EarlyAbort off).
    allow = scenario.get("allow_errors", False)
    d.Text.Command = "clear"
    d.Text.Command = "new circuit.propsprobe"
    d.Error.EarlyAbort = not allow
    for cmd in scenario["commands"]:
        if allow:
            try:
                d.Text.Command = cmd
            except Exception:  # noqa: BLE001 - the logged error is expected
                pass
        else:
            d.Text.Command = cmd

    target = scenario["target"]
    # Activate the target object (the `?` query sets it active) and read names.
    d.Text.Command = f"? {target}.Like"
    names = list(d.ActiveCircuit.ActiveDSSElement.AllPropertyNames)

    # Properties whose oracle getter returns uninitialized-memory garbage (see
    # the per-scenario notes) are canonicalized: every number in the value is
    # rewritten to 0, so only the structural skeleton is pinned. The Rust engine
    # emits the same zero matrix (a deterministic repro of the broken getter).
    zero_garbage = {e.lower() for e in scenario.get("zero_garbage", [])}
    # Properties whose oracle getter is a hard error for this scenario's state
    # (e.g. LoadShape Mean/StdDev on an empty shape raise 61107) are not pinned;
    # the Rust engine simply never queries them. The covering scenarios still
    # pin them in their non-empty states.
    skip_props = {e.lower() for e in scenario.get("skip_props", [])}
    props = {}
    for name in names:
        if name.lower() in skip_props:
            continue
        d.Text.Command = f"? {target}.{name}"
        value = d.Text.Result
        if name.lower() in zero_garbage:
            value = _NUM_RE.sub("0", value)
        props[name] = value

    out = {
        "name": scenario["name"],
        "commands": scenario["commands"],
        "target": target,
        "properties": props,
    }
    if scenario.get("allow_errors"):
        out["allow_errors"] = True
    return out


def main() -> None:
    oracle_version = check_pin()
    from dss import dss as d

    oracle = {"dss_python": oracle_version, "engine": d.Version}
    scenarios = [run_scenario(d, s) for s in SCENARIOS]

    # Group by class = scenario-name prefix before the first `_`, preserving
    # definition order, and write one file per class.
    by_class: dict[str, list] = {}
    for s in scenarios:
        cls = s["name"].split("_", 1)[0]
        by_class.setdefault(cls, []).append(s)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for cls, scs in by_class.items():
        path = OUT_DIR / f"{cls}.json"
        path.write_text(
            json.dumps(
                {"schema": SCHEMA, "oracle": oracle, "class": cls, "scenarios": scs},
                indent=1,
            )
            + "\n"
        )
        print(f"wrote {path.relative_to(REPO_ROOT)} ({len(scs)} scenarios)")
    for p in OUT_DIR.glob("*.json"):
        if p.stem not in by_class:
            p.unlink()
            print(f"removed stale {p.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
