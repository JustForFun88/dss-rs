#!/usr/bin/env python3
"""d2_twin_step.py — WM.3 D2 sub-bug #2 DECISIVE twin-Integrate experiment.

Settles audit-code D2F-1/D2F-2: the earlier `d2_twin_direct.py` stopped at
Init+CalcDynamic (the STEP-0/pflow point everyone already agrees on) and never
called `dll.Integrate()` — so "guest Integrate is bit-exact on the divergent
path" was asserted on a path never exercised. This probe drives the SAME 244-B
twin DLL through a full manual dynamics step:

    Init(Vsnap, Ipf)
    predictor:  Calc(Vsnap)  Integrate(flag=0)
    corrector:  Calc(Vsnap)  Integrate(flag=1)

with the terminal voltage HELD FIXED at the converged pflow snapshot, reading
|Is1| (var 8) after every sub-call. If the guest keeps |Is1| at the pflow point
(189.10) with V fixed, then the guest Integrate CANNOT produce the oracle's
h-independent jump to the dynamic operating point (189.207) on its own — the
jump is therefore driven by the ENGINE's per-step network re-solve (the V fed
back to the guest), i.e. host/engine flow, not the guest math. This is the
direct, in-isolation test of the divergent path.

Usage:  python d2_twin_step.py <path-to-244B-IndMach012a.dll>
Env:    PNOM (Pnominalperphase), H (step, default 1.67e-4; also try 1e-9).
"""
import ctypes
import os
import sys
from ctypes import (POINTER, WINFUNCTYPE, c_char_p, c_double, c_int32, c_uint32,
                    c_void_p)


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
V_SNAP = [(7953.625204515, 88.64099508475),
          (-3900.047249411, -6932.361976063),
          (-4053.577954327, 6843.720982325)]
I_PF = [(-168.0557335539, -86.69697178087),
        (8.946086099594, 188.889020314),
        (159.1096474543, -102.1920485331)]
DYNAMICMODE = 14


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
    h = float(os.environ.get("H", "0.000166667"))
    dyn.h = h
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
    # converge power flow (SolutionMode = 0 => CalcPflow)
    for _ in range(30):
        dll.Calc(ctypes.byref(vin), ctypes.byref(iout))
    print(f"pflow converged:            slip={gv(1):.12e}  |Is1|={gv(8):.10f}")

    # enter dynamics: Init at the pflow operating point
    dyn.SolutionMode = DYNAMICMODE
    dll.Init(ctypes.byref(vin), ctypes.byref(mk(I_PF)))
    print(f"after Init:                 slip={gv(1):.12e}  |Is1|={gv(8):.10f}")

    # ---- one FULL dynamics step, V held FIXED at the pflow snapshot ----
    # predictor
    dyn.IterationFlag = 0
    dll.Calc(ctypes.byref(vin), ctypes.byref(iout))
    print(f"pred Calc  (V fixed):       slip={gv(1):.12e}  |Is1|={gv(8):.10f}")
    dll.Integrate()
    dll.Calc(ctypes.byref(vin), ctypes.byref(iout))
    print(f"pred Integrate+Calc:        slip={gv(1):.12e}  |Is1|={gv(8):.10f}")
    # corrector
    dyn.IterationFlag = 1
    dll.Calc(ctypes.byref(vin), ctypes.byref(iout))
    print(f"corr Calc  (V fixed):       slip={gv(1):.12e}  |Is1|={gv(8):.10f}")
    dll.Integrate()
    dll.Calc(ctypes.byref(vin), ctypes.byref(iout))
    print(f"corr Integrate+Calc:        slip={gv(1):.12e}  |Is1|={gv(8):.10f}")
    print(f"  h={h:g}. pflow point |Is1|=189.10; oracle engine reaches 189.207 h-INDEP.")
    print("  If |Is1| stays ~189.10 with V fixed => the guest Integrate does NOT")
    print("  produce the jump; it is the engine's V-feed / network re-solve.")


if __name__ == "__main__":
    main()
