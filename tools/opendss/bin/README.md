# Vendored official EPRI OpenDSS binary (r4133)

Official EPRI OpenDSS `OpenDSSDirect.dll` (+ `KLUSolve.dll`,
`kmetis.exe`/`pmetis.exe` for A-Diakoptics tearing, `License.txt`), loaded by
absolute path through the in-house `dss-epri` Rust bridge (`epri-worker`) — the
r4133 channel of the unified corpus gate. See `tools/opendss/README.md`.

**Do not edit by hand.** Re-vendor with `python tools/opendss/vendor_binaries.py
--force` and review the `SHA256SUMS` diff. Verify from this directory with
`sha256sum -c SHA256SUMS`.

- **r4133** — OpenDSS SVN r4133 — latest official release, Version 11.0.0.1 (no dss_capi yet) (source: `.inputs/electricdss-code-r4133-trunk/Version8/Distrib/x64`)

Not vendored: `DSSProgress.exe` (progress popups stay impossible) and
`IndMach012a.dll` (external user-model sample; the corpus uses the built-in
`indmach012`). Add to `FILES`/`REVISIONS` in `vendor_binaries.py` if needed.

| field | value |
|---|---|
| vendored (UTC) | 2026-07-11 04:58:21 |
| files | 5 |
| total size | 11.8 MiB |

| file | bytes | build date (UTC) |
|---|---|---|
| `r4133/OpenDSSDirect.dll` | 11,823,616 | 2026-01-30 |
| `r4133/KLUSolve.dll` | 233,472 | 2024-09-30 |
| `r4133/kmetis.exe` | 156,160 | 2019-11-19 |
| `r4133/pmetis.exe` | 109,568 | 2019-11-19 |
| `r4133/License.txt` | 1,760 | 2026-01-30 |

These binaries are © EPRI, distributed under the BSD-style license in each
`License.txt`.
