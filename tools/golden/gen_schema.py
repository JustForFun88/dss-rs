"""Generate the AltDSS JSON-schema byte goldens from dss-python (oracle).

The oracle's `DSS_ExtractSchema(DSS, jsonSchema=True)` (Pascal
`CAPI_Schema.pas:DSS_ExtractJSONSchema`) emits a ~590 KB JSON-Schema document
that dumps every registered DSS class + enum. The Rust port currently reproduces
only the **static core** of that document (the schema envelope + the ten reusable
global `$defs` + the static `circuitProperties` head); the per-class/enum walk is
blocked on per-property metadata the Rust port never carried (help text,
`AltPropertyOrder`, `SpecSets`, enum JSON names, most `Units_*` flags) — see
STATUS §OG-1.5.

This script captures the oracle bytes, proves them deterministic across two
independent oracle processes, extracts the exact fpjson-pretty rendering of each
static-core fragment (self-validated by verbatim containment in the real oracle
output), and writes them to `tests/golden/json/schema_static_core.json`. The Rust
driver (`crates/dss-core/tests/golden_schema.rs`) renders the same fragments and
must reproduce the bytes verbatim.

Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.

Usage:
    python tools/golden/gen_schema.py            # write the golden
    python tools/golden/gen_schema.py --raw      # print raw schema bytes (internal)
"""

from __future__ import annotations

import collections
import json
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "json"

# The ten reusable global $defs, in Pascal insertion order (added before the
# enum/class loop, so they are the first ten $defs keys).
STATIC_DEF_NAMES = [
    "Complex",
    "PComplex",
    "SymmetricMatrix",
    "ArrayOrFilePath",
    "StringArrayOrFilePath",
    "JSONFilePath",
    "JSONLinesFilePath",
    "Bus",
    "BusConnection",
    "DynInitType",
]

# The static head of circuitProperties (CAPI_Schema.pas:1463-1474).
CIRCUIT_HEAD_NAMES = ["Name", "DefaultBaseFreq", "PreCommands", "PostCommands", "Bus"]

# The classes whose per-class walk (`prepareClassJsonSchema`) is ported +
# byte-gated, in `DSSClassList` order. The class batches append to this list as
# they land; the oracle fragment for each is captured below and byte-compared
# against the Rust `Dss::schema_class_def` output in `golden_schema.rs` (after the
# expected r4133 divergences in `schema_divergences.json`).
SCHEMA_CLASSES = [
    "LineCode",
    "LoadShape",
    "TShape",
    "PriceShape",
    "XYcurve",
    "GrowthShape",
    "TCC_Curve",
    "Spectrum",
    "WireData",
    "CNData",
    "TSData",
    "LineSpacing",
    # LineGeometry: deferred to integration as a port-authored (0.15.x) class —
    # its property model follows the newer dss_capi (Wires -> Redundant+SuppressJSON,
    # new Conductors=20 object-ref-array) and straddles 0.14.5 via HIDE_015X, so its
    # schema diverges structurally from the pinned 0.14.5 oracle and needs a
    # port-output reference (like WindGen). See STATUS §OG-1.5c batch B1.
]

NL = "\r\n"


def slice_class_def(text: str, cls: str) -> str:
    """Slice the exact `$defs/<cls>` object bytes from the oracle document and
    dedent to indent 0 — the fpjson bytes the Rust `write_pretty(def, 0)` must
    reproduce. Extracting the oracle bytes verbatim (rather than re-rendering)
    preserves the fpjson float formatting of every default; it is trivially
    self-validating (it *is* the oracle output)."""
    key = f'{NL}    "{cls}" : {{'  # top-level $defs member (host indent 2 = 4 sp)
    i = text.find(key)
    if i < 0:
        sys.exit(f"oracle schema missing class def {cls!r}")
    start = text.index("{", i)
    depth, instr, esc = 0, False, False
    end = None
    for j in range(start, len(text)):
        c = text[j]
        if instr:
            if esc:
                esc = False
            elif c == "\\":
                esc = True
            elif c == '"':
                instr = False
        elif c == '"':
            instr = True
        elif c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                end = j + 1
                break
    if end is None:
        sys.exit(f"unbalanced class def for {cls!r}")
    lines = text[start:end].split(NL)
    ded = [lines[0]] + [(ln[4:] if ln.startswith("    ") else ln) for ln in lines[1:]]
    return NL.join(ded)


def check_pin() -> None:
    pin = (REPO_ROOT / "tools" / "golden" / "PIN.txt").read_text()
    import dss

    want = {}
    for line in pin.splitlines():
        line = line.strip()
        if line.startswith("#") or "==" not in line:
            continue
        k, v = line.split("==", 1)
        want[k.strip()] = v.strip()
    got = getattr(dss, "__version__", None) or getattr(dss, "version", None)
    # dss-python exposes its version via dss.__version__ in 0.15.x.
    if "dss-python" in want and got and got != want["dss-python"]:
        sys.exit(f"PIN mismatch: dss-python {got!r} != pinned {want['dss-python']!r}")


def extract_schema_bytes() -> bytes:
    """Return the raw bytes of DSS_ExtractSchema(jsonSchema=True)."""
    import dss

    api = dss.DSS._api_util
    p = api.lib.DSS_ExtractSchema(api.ctx, True)
    return api.ffi.string(p)


def _esc(text: str) -> str:
    """fpjson StringToJSON — matches the Rust `escape_into`."""
    out = ['"']
    short = {"\b": "\\b", "\t": "\\t", "\n": "\\n", "\f": "\\f", "\r": "\\r"}
    for c in text:
        if c == '"':
            out.append('\\"')
        elif c == "\\":
            out.append("\\\\")
        elif c in short:
            out.append(short[c])
        elif ord(c) < 0x20:
            out.append("\\u%04x" % ord(c))
        else:
            out.append(c)
    out.append('"')
    return "".join(out)


def fpjson_pretty(value, indent: int) -> str:
    """Faithful port of the Rust `write_pretty` (fpjson FormatJSON([],2), CRLF).

    Only the value kinds that appear in the static core are handled (dict, list,
    str, bool, int) — the static core carries no floats or nulls.
    """
    pad = "  " * indent
    pad1 = "  " * (indent + 1)
    if isinstance(value, dict):
        if not value:
            return "{" + NL + pad + "}"
        items = []
        for k, v in value.items():
            items.append(pad1 + _esc(k) + " : " + fpjson_pretty(v, indent + 1))
        return "{" + NL + ("," + NL).join(items) + NL + pad + "}"
    if isinstance(value, list):
        if not value:
            return "[" + NL + pad + "]"
        items = [pad1 + fpjson_pretty(v, indent + 1) for v in value]
        return "[" + NL + ("," + NL).join(items) + NL + pad + "]"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return _esc(value)
    raise TypeError(f"static core carries no {type(value).__name__} ({value!r})")


def main() -> None:
    if "--raw" in sys.argv:
        # Internal: emit raw bytes for the determinism cross-process check.
        sys.stdout.buffer.write(extract_schema_bytes())
        return

    check_pin()

    # 1) Capture + prove determinism across two independent oracle processes.
    raw = extract_schema_bytes()
    other = subprocess.run(
        [sys.executable, str(Path(__file__).resolve()), "--raw"],
        capture_output=True,
        check=True,
    ).stdout
    if raw != other:
        sys.exit("NON-DETERMINISTIC: two oracle processes produced different schema bytes")
    print(f"schema bytes: {len(raw)} (deterministic across 2 processes)")

    text = raw.decode("utf-8")
    doc = json.loads(text, object_pairs_hook=collections.OrderedDict)
    defs = doc["$defs"]

    # 2) Render each static fragment and self-validate by verbatim containment.
    def render_and_check(value, name: str, host_indent: int, host_key: str) -> str:
        # Render at the fragment's real position (`  "key" : <value>` at the
        # host indent) and require that exact block to appear in the oracle
        # bytes — proof the Python writer reproduces fpjson for this fragment.
        block = "  " * host_indent + _esc(host_key) + " : " + fpjson_pretty(value, host_indent)
        if block not in text:
            sys.exit(f"self-check FAILED: rendered {name} not found verbatim in oracle output")
        # The golden stores the value alone at indent 0 (what the Rust test renders).
        return fpjson_pretty(value, 0)

    global_defs = {}
    for name in STATIC_DEF_NAMES:
        if name not in defs:
            sys.exit(f"oracle schema missing static def {name!r}")
        global_defs[name] = render_and_check(defs[name], f"$defs/{name}", 2, name)

    head = doc["properties"]
    circuit_head = {}
    for name in CIRCUIT_HEAD_NAMES:
        if name not in head:
            sys.exit(f"oracle circuitProperties missing head prop {name!r}")
        circuit_head[name] = render_and_check(head[name], f"properties/{name}", 2, name)

    # 2b) The 21 global enum $defs (top-level $defs entries carrying
    # `$dssFullEnum`, in insertion order). Class-local enums live inside their
    # class def, never at top level, so this set is exactly `DSS.Enums`.
    enum_defs = {}
    for name, value in defs.items():
        if isinstance(value, dict) and "$dssFullEnum" in value:
            enum_defs[name] = render_and_check(value, f"$defs/{name}", 2, name)

    # 3) Coverage bookkeeping: which class defs the (deferred) walk still owes.
    # The 21 top-level (global) enum defs are ported above (emitted into
    # `enum_defs`, byte-gated); the only enums still owed are class-LOCAL — they
    # live inside a class def, never at top level, so they are covered by the
    # deferred class walk, not enumerated here.
    def is_class_def(v) -> bool:
        return isinstance(v, dict) and v.get("type") == "object" and "properties" in v

    class_def_names = [
        k
        for k, v in defs.items()
        if k not in STATIC_DEF_NAMES
        and not k.endswith("List")
        and not k.endswith("Container")
        and is_class_def(v)
    ]

    # 4) The ported per-class walk fragments (byte-gated). Extract each ported
    # class's oracle bytes at indent 0. `deferred` = class defs still owed.
    class_defs = {cls: slice_class_def(text, cls) for cls in SCHEMA_CLASSES}
    deferred = [k for k in class_def_names if k not in SCHEMA_CLASSES]

    golden = collections.OrderedDict(
        [
            ("_comment", "AltDSS JSON-schema goldens (static core + enum defs + ported per-class defs) — see tools/golden/gen_schema.py"),
            ("schema_draft", doc["$schema"]),
            ("schema_id", doc["$id"]),
            ("required", doc["required"]),
            # Explicit insertion order (serde_json::Value maps do not preserve it).
            ("global_defs_order", list(global_defs.keys())),
            ("global_defs", global_defs),
            ("circuit_head_order", list(circuit_head.keys())),
            ("circuit_head", circuit_head),
            # The 21 global enum $defs, byte-exact (order + rendered bytes).
            ("enum_defs_order", list(enum_defs.keys())),
            ("enum_defs", enum_defs),
            # The ported per-class walk fragments, byte-exact (in DSSClassList
            # order). Byte-gated by `golden_schema.rs` after the expected r4133
            # divergences (`schema_divergences.json`).
            ("class_defs_order", list(class_defs.keys())),
            ("class_defs", class_defs),
            # Deferred class-walk inventory (informational; not byte-gated) — the
            # class defs whose walk is not ported yet.
            ("deferred_class_def_count", len(deferred)),
            ("deferred_class_def_names", deferred),
        ]
    )

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out_path = OUT_DIR / "schema_static_core.json"
    out_path.write_text(json.dumps(golden, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"wrote {out_path.relative_to(REPO_ROOT)}")
    print(f"  static global defs: {len(global_defs)}  circuit head: {len(circuit_head)}")
    print(f"  global enum defs: {len(enum_defs)}")
    print(f"  ported class defs: {len(class_defs)}  deferred: {len(deferred)}")


if __name__ == "__main__":
    main()
