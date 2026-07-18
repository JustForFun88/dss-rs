"""Porting aid (one-off) — extract the DSS property help catalog into Rust.

The property help/description strings the AltDSS JSON schema emits are NOT in the
vendored Pascal source: `TDSSClass.GetPropertyHelp` (`DSSClass.pas:2166`) reads
them from a gettext catalog `locale/en_US.mo` that ships as a dss_capi build
resource (installed as `dss/messages/properties-en-US.mo` in dss-python). That
catalog — keyed `<Class>.<propertynamelowercase>` — is the authoritative *source*
of the help text (the Pascal engine reads exactly it), NOT the schema JSON output,
so extracting it here is faithful porting, not oracle-seeding.

This reads the installed catalog and emits `schema/help.rs`, a sorted
`&[(&str,&str)]` (`Class.proplower` -> help) for binary-search lookup. Run
manually; the generated file is committed as a porting aid.

Usage:  python tools/golden/extract_schema_help.py
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT = REPO_ROOT / "crates" / "dss-core" / "src" / "report" / "export" / "json" / "schema" / "help.rs"


def find_mo() -> Path:
    import dss  # the pinned oracle package that ships the catalog

    cand = Path(dss.__file__).resolve().parent / "messages" / "properties-en-US.mo"
    if not cand.is_file():
        sys.exit(f"help catalog not found at {cand}")
    return cand


def read_mo(path: Path) -> dict[str, str]:
    buf = path.read_bytes()
    magic = struct.unpack("<I", buf[:4])[0]
    if magic != 0x950412DE:
        sys.exit(f"not a little-endian .mo file: {path}")
    n = struct.unpack("<I", buf[8:12])[0]
    oo = struct.unpack("<I", buf[12:16])[0]
    to = struct.unpack("<I", buf[16:20])[0]
    out: dict[str, str] = {}
    for i in range(n):
        ol, op = struct.unpack("<II", buf[oo + i * 8 : oo + i * 8 + 8])
        tl, tp = struct.unpack("<II", buf[to + i * 8 : to + i * 8 + 8])
        key = buf[op : op + ol].decode("utf-8")
        val = buf[tp : tp + tl].decode("utf-8")
        if not key:  # the "" metadata header entry
            continue
        out[key] = val
    return out


def rust_str(s: str) -> str:
    # Rust escaping for a normal double-quoted string literal.
    out = ['"']
    for c in s:
        if c == "\\":
            out.append("\\\\")
        elif c == '"':
            out.append('\\"')
        elif c == "\n":
            out.append("\\n")
        elif c == "\r":
            out.append("\\r")
        elif c == "\t":
            out.append("\\t")
        else:
            out.append(c)
    out.append('"')
    return "".join(out)


def main() -> None:
    catalog = read_mo(find_mo())
    keys = sorted(catalog.keys())
    lines = [
        "//! Generated porting aid — the DSS property help catalog "
        "(`dss/messages/properties-en-US.mo`,",
        "//! the gettext resource `TDSSClass.GetPropertyHelp` reads; "
        "`DSSClass.pas:2166`), keyed",
        "//! `<Class>.<propertynamelowercase>`. Regenerate with "
        "`tools/golden/extract_schema_help.py`.",
        "//!",
        "//! This is the authoritative *source* of the property help strings the "
        "AltDSS JSON",
        "//! schema emits — NOT the schema JSON output — so it is a faithful port, "
        "not a circular",
        "//! oracle seed. Do not hand-edit; edit the catalog upstream and "
        "regenerate.",
        "",
        "/// `(key, help)` pairs sorted by key for binary search "
        f"({len(keys)} entries).",
        "static HELP: &[(&str, &str)] = &[",
    ]
    for k in keys:
        lines.append(f"    ({rust_str(k)}, {rust_str(catalog[k])}),")
    lines.append("];")
    lines.append("")
    lines.append(
        "/// Pascal `TDSSClass.GetPropertyHelp` (`DSSClass.pas:2166-2201`), minus the"
    )
    lines.append(
        "/// class-parent fallback: `key = <Class>.<proplower>`; the catalog value if"
    )
    lines.append(
        "/// present, else the literal key (proved sufficient — every schema-emitted"
    )
    lines.append(
        "/// description resolves by leaf key or falls to the literal key; no parent"
    )
    lines.append("/// match is ever needed). `class` and `prop_lower` are already lowercase-safe;")
    lines.append("/// the caller lowercases the property name (the array-alternative name for a")
    lines.append("/// redirected property, matching `GetPropertyHelp(propIndex)`).")
    lines.append("pub fn property_help(class: &str, prop_lower: &str) -> String {")
    lines.append('    let key = format!("{class}.{prop_lower}");')
    lines.append("    match HELP.binary_search_by(|(k, _)| (*k).cmp(key.as_str())) {")
    lines.append("        Ok(i) => HELP[i].1.to_string(),")
    lines.append("        Err(_) => key,")
    lines.append("    }")
    lines.append("}")
    lines.append("")
    OUT.write_text("\n".join(lines), encoding="utf-8")
    print(f"wrote {OUT.relative_to(REPO_ROOT)} ({len(keys)} entries)")


if __name__ == "__main__":
    main()
