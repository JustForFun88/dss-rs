"""Generate `crates/dss-core/src/report/help_catalog.rs` from the pinned
oracle's gettext help catalog (PHASE8_PLAN WP8.5 step 3b item 1).

The pinned dss-python wheel ships the property/command help strings as a
gettext catalog, `dss/messages/properties-en-US.mo` (keys `Command.<name>`,
`Executive.<option>`, `<Class>.<prop-lowercase>`; Pascal `DSSGlobals.DSSHelp`
/ `TDSSClass.GetPropertyHelp`). `Dump commands` (Pascal `DumpAllDSSCommands`,
`Utilities.pas:821`) renders it, so the Rust port needs the same catalog as a
static. This script parses the `.mo` from the *installed pinned wheel* and
emits the sorted `(key, help)` table.

Regeneration is manual and must use the exact versions in tools/golden/PIN.txt
(same rule as the goldens):

    python tools/golden/gen_help_catalog.py

Verified invariants (asserted below, so a future wheel that breaks them fails
loudly instead of silently changing lookup semantics):
  - every key/value is plain ASCII (no escaping subtleties);
  - no value contains a CRLF pair (Pascal `ReplaceCRLF` is a no-op on this
    catalog: embedded newlines are bare LF);
  - no key carries a ClassParents prefix (`DSSClass.`, `CktElement.`,
    `PCClass.`, `PDClass.`, `ControlClass.`, `MeterClass.`, `CableData.`,
    `ConductorData.`, `MeterClass.`), so `GetPropertyHelp`'s parent-fallback
    loop (`DSSClass.pas:2191`) can never hit — the Rust lookup is
    own-key-or-miss.
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_checkpoints import check_pin  # noqa: E402
from r4133_help import R4133_HELP  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_RS = REPO_ROOT / "crates" / "dss-core" / "src" / "report" / "help_catalog.rs"

# Pascal class-parent names (`ClassParents.Add(...)` call sites) — none of
# these may appear as a key prefix, else the Rust own-key-only lookup would
# diverge from `GetPropertyHelp`'s fallback loop.
PARENT_PREFIXES = (
    "DSSClass.",
    "CktElement.",
    "PCClass.",
    "PDClass.",
    "ControlClass.",
    "MeterClass.",
    "CableData.",
    "ConductorData.",
)

HEADER = '''\
//! GENERATED FILE — do not edit by hand.
//!
//! The oracle's gettext help catalog (`dss/messages/properties-en-US.mo` in
//! the pinned dss-python wheel), rendered by `Dump commands` (Pascal
//! `DumpAllDSSCommands`, `Utilities.pas:821-872`). Keys: `Command.<name>`,
//! `Executive.<option>`, `<Class>.<prop-lowercase>` (Pascal `DSSHelp` /
//! `TDSSClass.GetPropertyHelp`).
//!
//! Regenerate manually — with the exact pinned versions in
//! `tools/golden/PIN.txt` only, same rule as the goldens — via
//! `python tools/golden/gen_help_catalog.py`.

/// Sorted `(key, help)` pairs parsed from the pinned wheel's `.mo` catalog.
static HELP_CATALOG: &[(&str, &str)] = &[
'''

FOOTER = '''];

/// Pascal `DSSGlobals.DSSHelp` (`DSSGlobals.pas:717-727`) over the loaded
/// catalog: the help string for `key`, or **the key itself** on a miss (the
/// gettext `Translate` returns empty → `Result := s`). `GetPropertyHelp`'s
/// ClassParents fallback (`DSSClass.pas:2191-2197`) is provably dead against
/// this catalog — it contains no parent-class-prefixed key (asserted at
/// generation) — so own-key-or-miss is the complete lookup.
pub fn dss_help(key: &str) -> &str {
    match HELP_CATALOG.binary_search_by(|(k, _)| (*k).cmp(key)) {
        Ok(i) => HELP_CATALOG[i].1,
        Err(_) => key,
    }
}
'''


def parse_mo(path: Path) -> list[tuple[str, str]]:
    data = path.read_bytes()
    magic, _rev, n, otab, ttab = struct.unpack("<IIIII", data[:20])
    if magic != 0x950412DE:
        sys.exit(f"not a little-endian .mo file: {path}")

    def entry(tab: int, i: int) -> str:
        length, off = struct.unpack("<II", data[tab + 8 * i : tab + 8 * i + 8])
        return data[off : off + length].decode("utf-8")

    pairs = [(entry(otab, i), entry(ttab, i)) for i in range(n)]
    # Drop the gettext metadata entry (empty key).
    return [(k, v) for k, v in pairs if k]


def rust_str(s: str) -> str:
    """Escape an ASCII string as a Rust double-quoted literal."""
    out = []
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
            assert " " <= c <= "~", f"non-printable/non-ASCII char {c!r}"
            out.append(c)
    return '"' + "".join(out) + '"'


def main() -> None:
    pin = check_pin()
    print(f"oracle: dss-python {pin['dss_python']}, engine {pin['engine']}")
    import dss

    mo = Path(dss.__file__).resolve().parent / "messages" / "properties-en-US.mo"
    pairs = parse_mo(mo)
    print(f"parsed {len(pairs)} entries from {mo}")

    # WP-U2.5: override/add the r4133 protection help (Relay/Fuse/SwtControl
    # property surfaces the 0.14.5 wheel predates — see `r4133_help.py`). A key
    # that already exists (deprecated aliases whose help became "DEPRECATED. See
    # …") is replaced in place; a renamed/new prop (`PhCurve`, `SinglePhTrip`,
    # `CurveMultiplier`, …) is appended. Recloser (WP-U2.2) already landed via the
    # wheel-independent capture, so it is not in the supplement.
    catalog = dict(pairs)
    catalog.update(R4133_HELP)
    pairs = list(catalog.items())

    for k, v in pairs:
        assert k.isascii() and v.isascii(), f"non-ASCII entry: {k!r}"
        assert "\r\n" not in v, f"CRLF in value of {k!r} (ReplaceCRLF no longer a no-op)"
        assert not k.startswith(PARENT_PREFIXES), f"parent-class key {k!r} breaks lookup"

    pairs.sort(key=lambda kv: kv[0])
    lines = [HEADER]
    for k, v in pairs:
        lines.append(f"    ({rust_str(k)}, {rust_str(v)}),\n")
    lines.append(FOOTER)
    OUT_RS.write_text("".join(lines), newline="\n")
    print(f"wrote {OUT_RS} ({OUT_RS.stat().st_size} bytes, {len(pairs)} entries)")


if __name__ == "__main__":
    main()
