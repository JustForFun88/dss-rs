"""WASM_USERMODELS WP-WM.0 P1 — does the pinned dss-python oracle load a
native win64 user-model DLL through `Generator.UserModel=`?

Drives the stub DLL built from genstub.pas (15 exports; Calc writes
I[k] = (100*k, -10*k) A) through the pinned oracle (tools/golden/PIN.txt:
dss-python 0.15.7, backend dss_capi 0.14.5) and asserts:

  P1.a  setting UserModel= raises no error (no 'Not Loaded' DoSimpleMsg 570);
  P1.b  the stub's 4 state vars appear on the element variable surface
        (proves New/NumVars/GetVarName were called through the DLL);
  P1.c  UserData= reaches the DLL's Edit (StubEditLen == len of the string);
  P1.d  a Model=6 snapshot solve converges and the element terminal currents
        equal the stub's Calc output exactly (proves Calc V-in/I-out
        marshalling: generator.pas DoUserModel `FCalc(Vterminal, Iterminal)`,
        InjCurrent negation, converged GetCurrents == user-model currents);
  P1.e  StubLastVre/im match the element's terminal-1 node voltage (proves
        Vterminal marshalling into the DLL). NB: the stub records V at the
        LAST Calc invocation, i.e. the fixed-point iterate BEFORE the final
        NodeV update, so this comparison is one solver iteration stale by
        construction — the assertion is a 1e-4 relative band (empirically
        ~5e-7 here), which still pins the correct bus/phase: phases b/c are
        rotated +-120 deg and would miss by ~1e0 relative.

Run:  python tools/fpc/usermodel_abi/probe_oracle_load.py <abs path to genstub.dll>
Evidence copy: docs/wasm/probes/p1_oracle_load.txt
"""
import sys

from dss import dss


def main() -> int:
    dllpath = sys.argv[1]
    failures = []

    def check(tag: str, ok: bool, detail: str) -> None:
        print(f"{tag}: {'PASS' if ok else 'FAIL'} — {detail}")
        if not ok:
            failures.append(tag)

    print(f"dss-python DSS version: {dss.Version}")
    dss.Text.Command = "Clear"
    dss.Text.Command = "new circuit.p1probe basekv=12.47 phases=3 bus1=srcbus"
    dss.Text.Command = (
        "new generator.g1 bus1=srcbus phases=3 kv=12.47 kw=1000 kvar=200 model=1"
    )
    dss.Text.Command = "set mode=snapshot"
    dss.Text.Command = "solve"
    dss.ActiveCircuit.SetActiveElement("generator.g1")
    base_names = list(dss.ActiveCircuit.ActiveCktElement.AllVariableNames)
    print(f"baseline (no user model) variable names: {base_names}")

    # P1.a — load the DLL
    try:
        dss.Text.Command = f'edit generator.g1 usermodel="{dllpath}"'
        check("P1.a", True, "UserModel= accepted, no 'Not Loaded' error raised")
    except Exception as exc:  # noqa: BLE001 — capture the oracle error verbatim
        check("P1.a", False, f"UserModel= raised: {exc!r}")
        print("RESULT: FAIL (cannot load native DLL — channel 1 falls back to r3723)")
        return 1

    # P1.b — stub vars visible through the element variable surface
    dss.ActiveCircuit.SetActiveElement("generator.g1")
    names = list(dss.ActiveCircuit.ActiveCktElement.AllVariableNames)
    print(f"variable names with stub loaded: {names}")
    want = ["StubCalcCount", "StubLastVre", "StubLastVim", "StubEditLen"]
    check("P1.b", all(w in names for w in want), f"stub vars in surface: {names[-4:]}")

    # P1.c — UserData= reaches Edit
    userdata = "rs=0.01 xs=0.1"
    dss.Text.Command = f'edit generator.g1 userdata=({userdata})'
    dss.ActiveCircuit.SetActiveElement("generator.g1")
    vals = list(dss.ActiveCircuit.ActiveCktElement.AllVariableValues)
    varmap = dict(zip(names, vals))
    check(
        "P1.c",
        varmap.get("StubEditLen") == float(len(userdata)),
        f"StubEditLen = {varmap.get('StubEditLen')} (UserData len = {len(userdata)})",
    )

    # P1.d — Model=6 solve routes through Calc; converged currents = stub output
    dss.Text.Command = "edit generator.g1 model=6"
    dss.Text.Command = "solve"
    conv = dss.ActiveCircuit.Solution.Converged
    dss.ActiveCircuit.SetActiveElement("generator.g1")
    cur = list(dss.ActiveCircuit.ActiveCktElement.Currents)
    ncond = dss.ActiveCircuit.ActiveCktElement.NumConductors
    print(f"converged={conv} ncond={ncond} terminal-1 currents={cur[: 2 * ncond]}")
    expected = []
    for k in range(1, ncond + 1):
        expected += [100.0 * k, -10.0 * k]
    got = cur[: 2 * ncond]
    max_abs_err = max(abs(g - e) for g, e in zip(got, expected))
    check(
        "P1.d",
        conv and max_abs_err < 1e-6,
        f"terminal currents vs stub Calc output: max abs err = {max_abs_err:.3e} A "
        f"(expected {expected})",
    )

    # P1.e — the V seen by the DLL == terminal-1 node voltage of the element
    vals = list(dss.ActiveCircuit.ActiveCktElement.AllVariableValues)
    names = list(dss.ActiveCircuit.ActiveCktElement.AllVariableNames)
    varmap = dict(zip(names, vals))
    volts = list(dss.ActiveCircuit.ActiveCktElement.Voltages)
    vmag = (volts[0] ** 2 + volts[1] ** 2) ** 0.5
    dv = max(
        abs(varmap["StubLastVre"] - volts[0]), abs(varmap["StubLastVim"] - volts[1])
    )
    # One solver iteration stale by construction (see module docstring); a
    # wrong bus/phase marshal would miss by ~1e0 relative, not ~1e-7.
    check(
        "P1.e",
        varmap["StubCalcCount"] > 0 and dv / vmag < 1e-4,
        f"StubCalcCount={varmap['StubCalcCount']}, "
        f"StubLastV=({varmap['StubLastVre']}, {varmap['StubLastVim']}) vs "
        f"element V1=({volts[0]}, {volts[1]}), rel err={dv / vmag:.3e} "
        f"(one iterate stale by construction)",
    )

    if failures:
        print(f"RESULT: FAIL ({failures})")
        return 1
    print("RESULT: PASS — pinned oracle loads + drives a native user-model DLL "
          "(channel 1 of plan §2.5 is CONFIRMED; r3723 fallback not needed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
