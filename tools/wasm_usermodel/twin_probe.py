#!/usr/bin/env python3
"""twin_probe.py -- drive the NATIVE TWIN (FPC-built vendored IndMach012a.dll,
tools/wasm_usermodel/build_native.ps1) through a fixed, fully deterministic
scenario and record every observable value bit-exactly.

WASM_USERMODELS_PLAN WP-WM.2 item 3/4 pre-stage: the recorded values are the
numeric spec the wasm fixture port is pinned against --
  * evidence copy:  docs/wasm/probes/p6_twin_expected.txt   (default output)
  * test fragment:  tools/wasm_usermodel/models/indmach012a/tests/
                    twin_expected.rs                        (--rust output)
Both are regenerated MANUALLY ONLY (goldens discipline; PIN.txt toolchain).

The scenario mirrors the DLL's real lifecycle (New -> Edit -> power-flow
Calc iterations -> Init -> dynamics Calc/Integrate -> variable surface) with
hand-chosen record images; the record layouts are the frozen ABI tables
(docs/wasm/USERMODEL_ABI.md section 2, probe-verified with struct asserts
below).

Usage:  python twin_probe.py <path-to-IndMach012a.dll> [--rust]
"""

import ctypes
import json
import struct
import sys
from ctypes import (
    POINTER,
    WINFUNCTYPE,
    c_char_p,
    c_double,
    c_int32,
    c_uint32,
    c_void_p,
    create_string_buffer,
)

# --- frozen record layouts (USERMODEL_ABI.md section 2.1 / 2.2) -------------


class TDynamicsRec(ctypes.Structure):
    _pack_ = 1
    _fields_ = [
        ("h", c_double),
        ("t", c_double),
        ("tstart", c_double),
        ("tstop", c_double),
        ("IterationFlag", c_int32),
        ("SolutionMode", c_int32),
        ("intHour", c_int32),
        ("dblHour", c_double),
    ]


class TGeneratorVars(ctypes.Structure):
    _pack_ = 1
    _fields_ = [
        ("Theta", c_double),
        ("Pshaft", c_double),
        ("Speed", c_double),
        ("w0", c_double),
        ("Hmass", c_double),
        ("Mmass", c_double),
        ("D", c_double),
        ("Dpu", c_double),
        ("kVArating", c_double),
        ("kVGeneratorBase", c_double),
        ("Xd", c_double),
        ("Xdp", c_double),
        ("Xdpp", c_double),
        ("puXd", c_double),
        ("puXdp", c_double),
        ("puXdpp", c_double),
        ("dTheta", c_double),
        ("dSpeed", c_double),
        ("ThetaHistory", c_double),
        ("SpeedHistory", c_double),
        ("Pnominalperphase", c_double),
        ("Qnominalperphase", c_double),
        ("NumPhases", c_int32),
        ("NumConductors", c_int32),
        ("Conn", c_int32),
        ("VthevMag", c_double),
        ("VThevHarm", c_double),
        ("ThetaHarm", c_double),
        ("VTarget", c_double),
        ("Zthev_re", c_double),
        ("Zthev_im", c_double),
        ("XRdp", c_double),
    ]


assert ctypes.sizeof(TDynamicsRec) == 52
assert ctypes.sizeof(TGeneratorVars) == 244
assert TGeneratorVars.VthevMag.offset == 188
assert TGeneratorVars.NumPhases.offset == 176

MSGCB = WINFUNCTYPE(None, c_char_p, c_uint32)


class TDSSCallBacks(ctypes.Structure):
    _pack_ = 1
    _fields_ = [("MsgCallBack", MSGCB), ("rest", c_void_p * 31)]


assert ctypes.sizeof(TDSSCallBacks) == 256

# --- the fixed scenario inputs (duplicated verbatim in twin_parity.rs) ------

W0 = 376.99111843077515  # 2*pi*60, fixed literal on both sides
V_PF = [(277.13, 0.0), (-140.02, -239.51), (-137.11, 240.05)]
V_DYN = [(276.4, 3.1), (-141.0, -238.2), (-135.9, 239.9)]
EDIT_STR = b"rs=0.048 xs=0.075 rr=0.018 xr=0.12 Xm=3.8 maxs=0.1 option=variableslip slip=-0.02"
H_DYN = 0.0002

records = []  # (key, kind, value) kind in {f64, i32, str}


def rec_f(key, v):
    records.append((key, "f64", float(v)))


def rec_i(key, v):
    records.append((key, "i32", int(v)))


def rec_s(key, v):
    records.append((key, "str", v))


def bits(v):
    return struct.unpack("<Q", struct.pack("<d", v))[0]


def main():
    dll_path = sys.argv[1]
    emit_rust = "--rust" in sys.argv[2:]
    dll = ctypes.WinDLL(dll_path)

    dll.New.argtypes = [POINTER(TGeneratorVars), POINTER(TDynamicsRec), POINTER(TDSSCallBacks)]
    dll.New.restype = c_int32
    dll.Delete.argtypes = [POINTER(c_int32)]
    dll.Select.argtypes = [POINTER(c_int32)]
    dll.Select.restype = c_int32
    arr6 = c_double * 6
    dll.Init.argtypes = [POINTER(arr6), POINTER(arr6)]
    dll.Calc.argtypes = [POINTER(arr6), POINTER(arr6)]
    dll.Edit.argtypes = [c_char_p, c_uint32]
    dll.NumVars.restype = c_int32
    dll.GetAllVars.argtypes = [POINTER(c_double * 14)]
    dll.GetVariable.argtypes = [POINTER(c_int32)]
    dll.GetVariable.restype = c_double
    dll.SetVariable.argtypes = [POINTER(c_int32), POINTER(c_double)]
    dll.GetVarName.argtypes = [POINTER(c_int32), c_char_p, c_uint32]

    messages = []

    @MSGCB
    def msg_cb(s, maxlen):
        messages.append(bytes(s))

    cb = TDSSCallBacks()
    cb.MsgCallBack = msg_cb

    gen = TGeneratorVars()
    gen.w0 = W0
    gen.Hmass = 6.0
    gen.kVArating = 1500.0
    gen.kVGeneratorBase = 0.48
    gen.Pnominalperphase = 400000.0
    gen.Qnominalperphase = 145000.0
    gen.NumPhases = 3
    gen.NumConductors = 3
    gen.Conn = 1

    dyn = TDynamicsRec()
    dyn.h = 0.001
    dyn.tstop = 1.0
    dyn.SolutionMode = 0  # SNAPSHOT

    def get_var(i):
        ii = c_int32(i)
        return dll.GetVariable(ctypes.byref(ii))

    def rec_vars(prefix):
        for i in range(1, 15):
            rec_f(f"{prefix}_{i}", get_var(i))

    def mk_v(v):
        flat = []
        for re, im in v:
            flat += [re, im]
        return arr6(*flat)

    # S1: New
    ident = dll.New(ctypes.byref(gen), ctypes.byref(dyn), ctypes.byref(cb))
    rec_i("new_id", ident)
    rec_f("after_new_speed", gen.Speed)

    # S2: variable-name surface
    rec_i("num_vars", dll.NumVars())
    for i in range(1, 15):
        buf = create_string_buffer(64)
        ii = c_int32(i)
        dll.GetVarName(ctypes.byref(ii), buf, 32)
        rec_s(f"var_name_{i}", buf.value.decode())
    buf = create_string_buffer(64)
    ii = c_int32(12)
    dll.GetVarName(ctypes.byref(ii), buf, 4)  # StrLCopy truncation
    rec_s("var_name_12_maxlen4", buf.value.decode())
    buf = create_string_buffer(b"\xaa" * 8, 9)
    ii = c_int32(15)
    dll.GetVarName(ctypes.byref(ii), buf, 8)  # out of range: untouched
    rec_s("var_name_15_untouched", "yes" if buf.raw[:8] == b"\xaa" * 8 else "NO")

    # S3: initial variables (post-Create state)
    rec_vars("vars_initial")

    # S4: Edit (abbrev 'maxs', mixed-case 'Xm', option keyword, slip write)
    dll.Edit(EDIT_STR, len(EDIT_STR))
    rec_f("after_edit_speed", gen.Speed)
    rec_vars("vars_after_edit")

    # S5: five power-flow Calc iterations at fixed V (slip fixed-point)
    vin = mk_v(V_PF)
    iout = arr6(*([0.0] * 6))
    for it in range(1, 6):
        dll.Calc(ctypes.byref(vin), ctypes.byref(iout))
        for k in range(6):
            rec_f(f"pf_i_{it}_{k}", iout[k])
        rec_f(f"pf_slip_{it}", get_var(1))

    # S6: variables after power flow
    rec_vars("vars_after_pflow")

    # S7: dynamics init (mode 14) with the converged pflow V/I
    dyn.SolutionMode = 14
    dyn.h = H_DYN
    dll.Init(ctypes.byref(vin), ctypes.byref(iout))
    rec_f("after_init_speed", gen.Speed)

    # S8: three dynamics steps, predictor (flag 0) + corrector (flag 1)
    vdyn = mk_v(V_DYN)
    n = 0
    for _step in range(1, 4):
        for flag in (0, 1):
            n += 1
            dyn.IterationFlag = flag
            dll.Calc(ctypes.byref(vdyn), ctypes.byref(iout))
            for k in range(6):
                rec_f(f"dyn_i_{n}_{k}", iout[k])
            rec_f(f"dyn_slip_{n}", get_var(1))
            dll.Integrate()

    # S9: variables after dynamics
    rec_vars("vars_after_dyn")

    # S10: SetVariable(1) writes slip + generator speed
    ii = c_int32(1)
    vv = c_double(-0.01)
    dll.SetVariable(ctypes.byref(ii), ctypes.byref(vv))
    rec_f("after_setvar_speed", gen.Speed)
    rec_f("after_setvar_slip", get_var(1))

    # S11: GetAllVars
    allv = (c_double * 14)(*([0.0] * 14))
    dll.GetAllVars(ctypes.byref(allv))
    for i in range(14):
        rec_f(f"all_vars_{i + 1}", allv[i])

    # S12: Select out-of-range / in-range
    i2 = c_int32(2)
    rec_i("select_2", dll.Select(ctypes.byref(i2)))
    i1 = c_int32(1)
    rec_i("select_1", dll.Select(ctypes.byref(i1)))

    # S13: help (MsgCallBack) — also recalcs the element data
    dll.Edit(b"help", 4)
    rec_i("help_msg_count", len(messages))
    rec_s("help_msg", messages[0].decode() if messages else "")
    rec_vars("vars_after_help")

    # S14: second instance, delete, reselect
    gen2 = TGeneratorVars()
    gen2.w0 = W0
    gen2.kVArating = 1000.0
    gen2.kVGeneratorBase = 0.48
    gen2.Pnominalperphase = 300000.0
    gen2.NumPhases = 3
    gen2.NumConductors = 3
    gen2.Conn = 1
    id2 = dll.New(ctypes.byref(gen2), ctypes.byref(dyn), ctypes.byref(cb))
    rec_i("new2_id", id2)
    rec_f("after_new2_speed", gen2.Speed)
    rec_f("new2_slip", get_var(1))
    ii = c_int32(2)
    dll.Delete(ctypes.byref(ii))
    i1 = c_int32(1)
    rec_i("select_1_after_delete", dll.Select(ctypes.byref(i1)))
    rec_f("model1_purs", get_var(2))

    # --- emit -----------------------------------------------------------
    if emit_rust:
        print("// GENERATED by tools/wasm_usermodel/twin_probe.py --rust — DO NOT EDIT.")
        print("// Expected values probed from the NATIVE TWIN (FPC build of the vendored")
        print("// IndMach012a.dpr, plan A); evidence docs/wasm/probes/p6_twin_expected.txt.")
        print("// f64 values are IEEE-754 bit patterns (bit-exact pin).")
        for key, kind, v in records:
            k = key.upper()
            if kind == "f64":
                print(f"pub const {k}: u64 = 0x{bits(v):016X}; // {v!r}")
            elif kind == "i32":
                print(f"pub const {k}: i32 = {v};")
            else:
                print(f"pub const {k}: &str = {json.dumps(v)};")
    else:
        print("p6 — native-twin expected values (WP-WM.2 item 3/4 pre-stage)")
        print(f"dll: {dll_path}")
        print("scenario: see tools/wasm_usermodel/twin_probe.py (fixed inputs)")
        print(f"w0={W0!r} V_PF={V_PF!r} V_DYN={V_DYN!r} h_dyn={H_DYN!r}")
        print(f"edit={EDIT_STR.decode()!r}")
        print("-" * 72)
        for key, kind, v in records:
            if kind == "f64":
                print(f"{key} = {v:.17g}  bits=0x{bits(v):016X}")
            else:
                print(f"{key} = {v!r}")


if __name__ == "__main__":
    main()
