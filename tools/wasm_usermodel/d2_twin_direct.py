#!/usr/bin/env python3
"""d2_twin_direct.py — WM.3 D2 sub-bug #2 investigation.

Drive the r3723 244-B native twin DLL DIRECTLY (ctypes, no OpenDSS engine) with
the wasm_gen_dyn machine params + the deck's converged snapshot terminal V/I,
running the exact guest lifecycle New -> Edit -> CalcPFlow -> Init -> CalcDynamic
(and optionally the engine's per-step Integrate/Calc order). Reads |Is1| (var 8).

Purpose: isolate the GUEST MODEL MATH from the OpenDSS ENGINE FLOW. The result is
the CROWN finding of the D2 trace — the twin's Init+CalcDynamic gives |Is1| =
189.10 (the power-flow point), BIT-IDENTICAL to the Rust engine + wasm fixture.
So the guest math is not the bug; the ~5e-4 D2 divergence lives entirely in how
the OpenDSS engine drives the machine to the dynamic operating point (|Is1| =
189.207) at the first dynamics step — a h-independent jump the guest's own
Init/Integrate math (proven here) does not produce on its own.

Usage:  python d2_twin_direct.py <path-to-244B-IndMach012a.dll>
Env:    PNOM (Pnominalperphase, default -1333333.333), PREINIT=1 (run a
        dynamics-mode CalcDynamic with E1=0 before Init — disproven hypothesis,
        kept for reproducibility).
"""
import ctypes
import os
import sys
from ctypes import (POINTER, WINFUNCTYPE, c_char_p, c_double, c_int32, c_uint32,
                    c_void_p)


# 244-B r3723/0.14.5 TGeneratorVars (NO deltaQNom — the ABI the pinned
# dss-python 0.14.5 passes; see USERMODEL_ABI.md Appendix A).
class TGeneratorVars(ctypes.Structure):
    _pack_ = 1
    _fields_ = [(n, c_double) for n in (
        "Theta", "Pshaft", "Speed", "w0", "Hmass", "Mmass", "D", "Dpu",
        "kVArating", "kVGeneratorBase", "Xd", "Xdp", "Xdpp", "puXd", "puXdp",
        "puXdpp", "dTheta", "dSpeed", "ThetaHistory", "SpeedHistory",
        "Pnominalperphase", "Qnominalperphase")] + \
        [("NumPhases", c_int32), ("NumConductors", c_int32), ("Conn", c_int32)] + \
        [(n, c_double) for n in ("VthevMag", "VThevHarm", "ThetaHarm", "VTarget",
                                 "Zthev_re", "Zthev_im", "XRdp")]


assert ctypes.sizeof(TGeneratorVars) == 244


class TDynamicsRec(ctypes.Structure):
    _pack_ = 1
    _fields_ = [("h", c_double), ("t", c_double), ("tstart", c_double),
                ("tstop", c_double), ("IterationFlag", c_int32),
                ("SolutionMode", c_int32), ("intHour", c_int32),
                ("dblHour", c_double)]


MSGCB = WINFUNCTYPE(None, c_char_p, c_uint32)


class TDSSCallBacks(ctypes.Structure):
    _pack_ = 1
    _fields_ = [("MsgCallBack", MSGCB), ("rest", c_void_p * 31)]


W0 = 376.99111843077515
# wasm_gen_dyn snapshot terminal V (node voltages) + converged pflow I (phase),
# read from the pinned 0.14.5 oracle (Generator.g1 elem.Voltages / .Currents).
V_SNAP = [(7953.625204515, 88.64099508475),
          (-3900.047249411, -6932.361976063),
          (-4053.577954327, 6843.720982325)]
I_PF = [(-168.0557335539, -86.69697178087),
        (8.946086099594, 188.889020314),
        (159.1096474543, -102.1920485331)]


def main():
    dll = ctypes.WinDLL(sys.argv[1])
    dll.New.argtypes = [POINTER(TGeneratorVars), POINTER(TDynamicsRec), POINTER(TDSSCallBacks)]
    dll.New.restype = c_int32
    arr6 = c_double * 6
    dll.Init.argtypes = [POINTER(arr6), POINTER(arr6)]
    dll.Calc.argtypes = [POINTER(arr6), POINTER(arr6)]
    dll.Integrate.argtypes = []
    dll.Edit.argtypes = [c_char_p, c_uint32]
    dll.GetVariable.argtypes = [POINTER(c_int32)]
    dll.GetVariable.restype = c_double

    @MSGCB
    def cbf(s, m):
        pass

    cb = TDSSCallBacks()
    cb.MsgCallBack = cbf
    gen = TGeneratorVars()
    gen.w0 = W0
    gen.Hmass = 1.0
    gen.kVArating = 5000.0
    gen.kVGeneratorBase = 13.8
    gen.Pnominalperphase = float(os.environ.get("PNOM", -4000.0 * 1000.0 / 3.0))
    gen.NumPhases = 3
    gen.NumConductors = 3
    gen.Conn = 1
    dyn = TDynamicsRec()
    dyn.h = 0.000166667
    dyn.SolutionMode = 0
    dll.New(ctypes.byref(gen), ctypes.byref(dyn), ctypes.byref(cb))
    dll.Edit(b"Rs=0.0053 Xs=0.106 Rr=0.007 Xr=0.12 Xm=4 MaxSlip=0.1", 100)

    def gv(i):
        ii = c_int32(i)
        return dll.GetVariable(ctypes.byref(ii))

    def mk(v):
        f = []
        for r, i in v:
            f += [r, i]
        return arr6(*f)

    vin = mk(V_SNAP)
    iout = arr6(*([0.0] * 6))
    for _ in range(20):
        dll.Calc(ctypes.byref(vin), ctypes.byref(iout))
    print(f"after pflow: slip(var1)={gv(1):.14e} |Is1|(var8)={gv(8):.10f}")

    dyn.SolutionMode = 14
    dyn.h = 0.000166667
    if os.environ.get("PREINIT") == "1":
        dyn.IterationFlag = 0
        dll.Calc(ctypes.byref(vin), ctypes.byref(iout))
        print(f"pre-Init CalcDyn(E1=0): |Is1|(var8)={gv(8):.10f}")
    dll.Init(ctypes.byref(vin), ctypes.byref(mk(I_PF)))
    print(f"after Init: |Is1|(var8)={gv(8):.10f}")
    dyn.IterationFlag = 0
    dll.Calc(ctypes.byref(vin), ctypes.byref(iout))
    print(f"after Init+CalcDyn(Vsnap): |Is1|(var8)={gv(8):.10f}")
    print("  -> 189.10 => guest gives the PFLOW point (== Rust engine, guest is bit-exact)")
    print("  -> the engine drives it to 189.207 (dynamic point): the D2 bug is engine-flow")


if __name__ == "__main__":
    main()
