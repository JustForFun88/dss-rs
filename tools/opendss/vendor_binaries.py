"""Vendor the official EPRI OpenDSS binaries used by the Oddie oracle bridge.

Copies, per SVN revision, exactly the files needed to load the engine through
AltDSS Oddie (`dss.Oddie.IOddieDSS` loading `OpenDSSDirect.dll` by absolute
path):

  - `OpenDSSDirect.dll` — the flat "Direct DLL" build of the official engine;
  - `KLUSolve.dll`      — its only non-system import (resolved from the same
                          directory because Oddie loads with
                          `LOAD_WITH_ALTERED_SEARCH_PATH`);
  - `License.txt`       — EPRI license shipped alongside the binaries.

Deliberately NOT copied:
  - `DSSProgress.exe`  — without it the engine cannot spawn progress popups;
  - `IndMach012a.dll`  — external user-model sample DLL (the corpus uses the
                         built-in indmach012); add here if ever needed.

Also writes `bin/SHA256SUMS` (verify from `tools/opendss/bin/` with
`sha256sum -c SHA256SUMS`) and `bin/README.md` (provenance).

Run once (and again only to re-vendor):
    python tools/opendss/vendor_binaries.py           # refuses if bin/ exists
    python tools/opendss/vendor_binaries.py --force   # wipe + recopy
"""

from __future__ import annotations

import hashlib
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
BIN_DIR = Path(__file__).resolve().parent / "bin"

# rev -> (source Distrib/x64 dir, human description)
REVISIONS: dict[str, tuple[Path, str]] = {
    "r3723": (
        REPO_ROOT / ".inputs" / "electricdss-code-r3723-trunk" / "Version8" / "Distrib" / "x64",
        "OpenDSS SVN r3723 — the base of dss_capi 0.14.x (the port's pinned oracle line)",
    ),
    "r4088": (
        REPO_ROOT / ".inputs" / "electricdss-code-r4088-trunk" / "Version8" / "Distrib" / "x64",
        "OpenDSS SVN r4088 — the base of dss_capi 0.15.x (pre-release line)",
    ),
    "r4133": (
        REPO_ROOT / ".inputs" / "electricdss-code-r4133-trunk" / "Version8" / "Distrib" / "x64",
        "OpenDSS SVN r4133 — latest official release, Version 11.0.0.1 (no dss_capi yet)",
    ),
}

FILES = ["OpenDSSDirect.dll", "KLUSolve.dll", "License.txt"]


def main() -> None:
    force = "--force" in sys.argv[1:]
    if BIN_DIR.exists() and any(BIN_DIR.iterdir()) and not force:
        sys.exit(f"destination already exists: {BIN_DIR}\n  re-run with --force to re-vendor")

    copied: list[tuple[str, int]] = []  # (rel_posix under bin/, size)
    rows: list[str] = []
    for rev, (src_dir, desc) in REVISIONS.items():
        if not src_dir.is_dir():
            sys.exit(f"source not found: {src_dir}")
        dst_dir = BIN_DIR / rev
        dst_dir.mkdir(parents=True, exist_ok=True)
        for name in FILES:
            src = src_dir / name
            if not src.is_file():
                sys.exit(f"missing {name} in {src_dir}")
            shutil.copy2(src, dst_dir / name)
            rel = f"{rev}/{name}"
            size = src.stat().st_size
            copied.append((rel, size))
            mtime = datetime.fromtimestamp(src.stat().st_mtime, tz=timezone.utc)
            rows.append(f"| `{rel}` | {size:,} | {mtime.strftime('%Y-%m-%d')} |")
        print(f"{rev}: {len(FILES)} files from {src_dir}")

    lines = []
    for rel, _ in copied:
        digest = hashlib.sha256((BIN_DIR / rel).read_bytes()).hexdigest()
        lines.append(f"{digest}  {rel}")
    (BIN_DIR / "SHA256SUMS").write_text("\n".join(lines) + "\n", newline="\n")

    total = sum(sz for _, sz in copied)
    revs_md = "\n".join(
        f"- **{rev}** — {desc} (source: `{src.relative_to(REPO_ROOT).as_posix()}`)"
        for rev, (src, desc) in REVISIONS.items()
    )
    table = "\n".join(["| file | bytes | build date (UTC) |", "|---|---|---|", *rows])
    readme = f"""# Vendored official EPRI OpenDSS binaries (Oddie oracle bridge)

Official EPRI OpenDSS `OpenDSSDirect.dll` builds (+ `KLUSolve.dll`, `License.txt`),
one directory per OpenDSS SVN revision, loaded by absolute path through the
AltDSS Oddie bridge (`dss.Oddie.IOddieDSS`) — see `tools/opendss/README.md`.

**Do not edit by hand.** Re-vendor with `python tools/opendss/vendor_binaries.py
--force` and review the `SHA256SUMS` diff. Verify from this directory with
`sha256sum -c SHA256SUMS`.

{revs_md}

Not vendored: `DSSProgress.exe` (progress popups stay impossible) and
`IndMach012a.dll` (external user-model sample; the corpus uses the built-in
`indmach012`). Add to `FILES`/`REVISIONS` in `vendor_binaries.py` if needed.

| field | value |
|---|---|
| vendored (UTC) | {datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S")} |
| files | {len(copied)} |
| total size | {total / 1024 / 1024:.1f} MiB |

{table}

These binaries are © EPRI, distributed under the BSD-style license in each
`License.txt`.
"""
    (BIN_DIR / "README.md").write_text(readme, newline="\n")
    print(f"copied {len(copied)} files ({total / 1024 / 1024:.1f} MiB); wrote README.md + SHA256SUMS")


if __name__ == "__main__":
    main()
