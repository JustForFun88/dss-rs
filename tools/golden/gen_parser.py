"""Generate the parser golden (tests/golden/parser.json) from dss-python.

dss-python's `DSS.Parser` drives the very same `TDSSParser` (ParserDel.pas)
the Rust `dss_parser::Parser` ports, so replaying this file pins the
tokenizer behavior empirically — including the FPC `Val` number grammar and
the off-by-one scanning quirks.

One parser instance runs all cases in order (the RPN X register persists
across commands, exactly like the engine); the Rust replay test must do the
same. Inputs that raise a conversion error are excluded — the Pascal
exception aborts the process through the C API — and are covered by Rust
unit tests cross-checked with tools/golden/probe_val.py instead.

Usage:
    python tools/golden/gen_parser.py

Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_FILE = REPO_ROOT / "tests" / "golden" / "parser.json"
SCHEMA = 1

# Each case: command string + list of ops applied in order.
#   ["next"]            -> records {"param": ..., "token": ...}
#   ["dbl"]             -> records {"dbl": ...}
#   ["int"]             -> records {"int": ...}
#   ["vector", size]    -> records {"vector": [...]}
#
# Parser.Matrix/SymMatrix crash in dss-python 0.15.7 (broken COM-legacy
# wrappers), but both are thin loops over ParseAsVector; matrix rows are
# covered by successive `vector` ops across `|` terminators instead, and the
# row-placement logic by Rust unit tests ported straight from the Pascal.
CASES = [
    {
        "name": "name_value_pairs",
        "cmd": "param1=value1 param2=value2",
        "ops": [["next"], ["next"], ["next"]],
    },
    {
        "name": "positional_tokens",
        "cmd": "New Line.L1 Bus1 Bus2",
        "ops": [["next"], ["next"], ["next"], ["next"], ["next"]],
    },
    {
        "name": "equals_with_spaces",
        "cmd": "kv = 12.47 pf  =  0.95",
        "ops": [["next"], ["dbl"], ["next"], ["dbl"]],
    },
    {
        "name": "commas",
        "cmd": "a,b, c ,d",
        "ops": [["next"], ["next"], ["next"], ["next"]],
    },
    {
        "name": "quote_pairs",
        "cmd": "p=(alpha beta) d=\"gamma delta\" s='ep si' b=[ze ta] c={e ta}",
        "ops": [["next"], ["next"], ["next"], ["next"], ["next"]],
    },
    {
        "name": "bang_comment",
        "cmd": "kw=5 ! rest is comment kv=12",
        "ops": [["next"], ["dbl"], ["next"]],
    },
    {
        "name": "slash_comment",
        "cmd": "conn=delta // trailing",
        "ops": [["next"], ["next"]],
    },
    {
        "name": "slash_in_token",
        "cmd": "file=dir/sub/name.dss",
        "ops": [["next"]],
    },
    {
        "name": "val_float_forms",
        "cmd": "a=.5 b=5. c=1e3 d=+5 e=-0.25 f=1.5e-3 g=007 h=-.5 i=1e+3 j=1.e3",
        "ops": [["next"], ["dbl"]] * 10,
    },
    {
        "name": "val_int_forms",
        "cmd": "a=$FF b=0xFF c=%101 d=&777 e=2.5 f=1.5 g=.5 h=-.5 i=1e3 j=+42",
        "ops": [["next"], ["int"]] * 10,
    },
    {
        "name": "rpn_expressions",
        "cmd": "a=(1 2 +) b=(2 3 *) c=(2 10 ^) d=(9 sqrt) e=(30 sin) f=(10 exp ln) g=(42.5)",
        "ops": [["next"], ["dbl"]] * 7,
    },
    {
        "name": "rpn_register_persists",
        "cmd": "a=(10) b=(2 +)",
        "ops": [["next"], ["dbl"], ["next"], ["dbl"]],
    },
    {
        "name": "vector_brackets",
        "cmd": "kvs=[1.0 2.5 3.5]",
        "ops": [["next"], ["vector", 3]],
    },
    # NOTE: a vector with more elements than the expected size is NOT golden-
    # tested: the C-API wrapper then returns uninitialized memory for the
    # extras (the Pascal engine never converts them). Covered by unit tests.
    {
        "name": "vector_exact_size",
        "cmd": "v=[1 2 3 4 5]",
        "ops": [["next"], ["vector", 5]],
    },
    {
        "name": "vector_commas",
        "cmd": "v=(1.4, 2.5, 3.6)",
        "ops": [["next"], ["vector", 3]],
    },
    {
        "name": "matrix_rows_via_vector",
        "cmd": "m=[1 2 | 3 4]",
        "ops": [["next"], ["vector", 2], ["vector", 2]],
    },
    {
        "name": "symmatrix_rows_via_vector",
        "cmd": "r=[0.791721 |0.318476 0.781649 |0.28345 0.318476 0.791721]",
        "ops": [["next"], ["vector", 3], ["vector", 3], ["vector", 3]],
    },
    {
        "name": "ragged_rows_via_vector",
        "cmd": "z=[2 | -1 3]",
        "ops": [["next"], ["vector", 2], ["vector", 2]],
    },
    {
        "name": "empty_value",
        "cmd": "x= y=2",
        "ops": [["next"], ["next"]],
    },
    {
        "name": "double_equals",
        "cmd": "a==b",
        "ops": [["next"], ["next"]],
    },
    {
        "name": "leading_equals",
        "cmd": "=5",
        "ops": [["next"]],
    },
    {
        "name": "tabs_as_whitespace",
        "cmd": "a\tb\tc=3",
        "ops": [["next"], ["next"], ["next"], ["int"]],
    },
    {
        "name": "unterminated_quote",
        "cmd": "x=(1 2",
        "ops": [["next"]],
    },
    {
        "name": "empty_command",
        "cmd": "",
        "ops": [["next"]],
    },
    {
        "name": "only_spaces",
        "cmd": "   ",
        "ops": [["next"]],
    },
    {
        "name": "dotted_names",
        "cmd": "New Load.L1.extra bus1=alpha.1.2.3",
        "ops": [["next"], ["next"], ["next"]],
    },
    {
        "name": "mixed_real_command",
        "cmd": "New Line.650632 Phases=3 Bus1=RG60.1.2.3 Bus2=632.1.2.3 LineCode=mtx601 Length=2000 units=ft",
        "ops": [["next"]] * 8,
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


def main() -> None:
    version = check_pin()
    from dss import dss as engine

    p = engine.Parser
    results = []
    for case in CASES:
        p.CmdString = case["cmd"]
        records = []
        for op in case["ops"]:
            kind = op[0]
            if kind == "next":
                param = p.NextParam
                records.append({"param": param, "token": p.StrValue})
            elif kind == "dbl":
                records.append({"dbl": float(p.DblValue)})
            elif kind == "int":
                records.append({"int": int(p.IntValue)})
            elif kind == "vector":
                # the wrapper returns exactly NumElements-found values
                records.append({"size": op[1], "vector": [float(x) for x in p.Vector(op[1])]})
            else:
                sys.exit(f"unknown op {op!r}")
        results.append({"name": case["name"], "cmd": case["cmd"], "records": records})

    golden = {
        "schema": SCHEMA,
        "oracle": {"dss_python": version, "engine": engine.Version},
        "cases": results,
    }
    OUT_FILE.parent.mkdir(parents=True, exist_ok=True)
    OUT_FILE.write_text(json.dumps(golden, indent=1))
    print(f"wrote {OUT_FILE} ({len(results)} cases)")


if __name__ == "__main__":
    main()
