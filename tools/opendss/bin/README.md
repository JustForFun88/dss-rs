# Vendored official EPRI OpenDSS binaries (Oddie oracle bridge)

Official EPRI OpenDSS `OpenDSSDirect.dll` builds (+ `KLUSolve.dll`, `License.txt`),
one directory per OpenDSS SVN revision, loaded by absolute path through the
AltDSS Oddie bridge (`dss.Oddie.IOddieDSS`) — see `tools/opendss/README.md`.

**Do not edit by hand.** Re-vendor with `python tools/opendss/vendor_binaries.py
--force` and review the `SHA256SUMS` diff. Verify from this directory with
`sha256sum -c SHA256SUMS`.

- **r3723** — OpenDSS SVN r3723 — the base of dss_capi 0.14.x (the port's pinned oracle line) (source: `.inputs/electricdss-code-r3723-trunk/Version8/Distrib/x64`)
- **r4088** — OpenDSS SVN r4088 — the base of dss_capi 0.15.x (pre-release line) (source: `.inputs/electricdss-code-r4088-trunk/Version8/Distrib/x64`)
- **r4133** — OpenDSS SVN r4133 — latest official release, Version 11.0.0.1 (no dss_capi yet) (source: `.inputs/electricdss-code-r4133-trunk/Version8/Distrib/x64`)

Not vendored: `DSSProgress.exe` (progress popups stay impossible) and
`IndMach012a.dll` (external user-model sample; the corpus uses the built-in
`indmach012`). Add to `FILES`/`REVISIONS` in `vendor_binaries.py` if needed.

| field | value |
|---|---|
| vendored (UTC) | 2026-07-06 20:38:50 |
| files | 9 |
| total size | 34.2 MiB |

| file | bytes | build date (UTC) |
|---|---|---|
| `r3723/OpenDSSDirect.dll` | 11,549,696 | 2024-01-31 |
| `r3723/KLUSolve.dll` | 219,648 | 2023-09-12 |
| `r3723/License.txt` | 1,760 | 2024-01-31 |
| `r4088/OpenDSSDirect.dll` | 11,783,168 | 2025-10-20 |
| `r4088/KLUSolve.dll` | 233,472 | 2024-09-30 |
| `r4088/License.txt` | 1,760 | 2024-01-31 |
| `r4133/OpenDSSDirect.dll` | 11,823,616 | 2026-01-30 |
| `r4133/KLUSolve.dll` | 233,472 | 2024-09-30 |
| `r4133/License.txt` | 1,760 | 2026-01-30 |

These binaries are © EPRI, distributed under the BSD-style license in each
`License.txt`.
