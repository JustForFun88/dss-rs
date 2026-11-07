# Vendored official EPRI OpenDSS binary and documentation (r4133)

Official EPRI OpenDSS `OpenDSSDirect.dll` (+ `KLUSolve.dll`,
`kmetis.exe`/`pmetis.exe` for A-Diakoptics tearing, the licence files), loaded by
absolute path through the in-house `dss-epri` Rust bridge (`epri-worker`) — the
r4133 channel of the unified corpus gate. See `tools/opendss/README.md`.

`OpenDSS_Documentation.chm` is the OpenDSS documentation, the specification of
the DSS language. Read it as HTML after `hh.exe -decompile <dir> <chm>` with
absolute paths: hh.exe extracts nothing from a path with a space, so the file
drops the space of its distribution name `OpenDSS Documentation.chm`.

Verify from this directory with `sha256sum -c SHA256SUMS`. Updating them:
`tools/opendss/README.md`.

- **r4133** — OpenDSS SVN r4133 — latest official release, Version 11.0.0.1 (no dss_capi yet) (source: `.inputs/electricdss-code-r4133-trunk/Version8/Distrib/x64`, the documentation from `Version8/Distrib/Doc`, last changed in SVN r4128, KLUSolve's licence files from `VersionC/klusolve`)

Not vendored: `DSSProgress.exe` (progress popups stay impossible) and
`IndMach012a.dll` (external user-model sample; the corpus uses the built-in
`indmach012`).

| field | value |
|---|---|
| vendored (UTC) | 2026-07-11 04:58:21 (binaries), 2026-11-07 17:18:25 (documentation), 2026-11-07 19:19:25 (KLUSolve's licence files) |
| files | 8 |
| total size | 43.7 MiB |

| file | bytes | build date (UTC) |
|---|---|---|
| `r4133/OpenDSSDirect.dll` | 11,823,616 | 2026-01-30 |
| `r4133/KLUSolve.dll` | 233,472 | 2024-09-30 |
| `r4133/kmetis.exe` | 156,160 | 2019-11-19 |
| `r4133/pmetis.exe` | 109,568 | 2019-11-19 |
| `r4133/License.txt` | 1,760 | 2026-01-30 |
| `r4133/OpenDSS_Documentation.chm` | 33,469,295 | 2026-01-27 (SVN commit) |
| `r4133/KLUSolve_License.txt` | 802 | 2024-07-22 (SVN commit) |
| `r4133/KLUSolve_lgpl_2_1.txt` | 26,940 | 2024-07-22 (SVN commit) |

Every file here serves the tests only: the product neither ships nor links any
of them.

- `OpenDSSDirect.dll` and the documentation are © EPRI, under the BSD-style
  terms of `r4133/License.txt`.
- `KLUSolve.dll` is © EnerNex Corporation, under LGPL-2.1-or-later
  (`r4133/KLUSolve_License.txt`, `r4133/KLUSolve_lgpl_2_1.txt`).
- `kmetis.exe` and `pmetis.exe` are the METIS programs of the OpenDSS
  distribution, unmodified. The distribution carries no licence file for them.
