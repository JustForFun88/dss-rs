# Attribution

Third-party files in this repository, with author, source and licence. Every one of them
serves the tests only: the product neither ships nor links any of them. Rust crates come from
crates.io under their own licences (`Cargo.lock`) and are not listed here.

## OpenDSS r4133 (`tools/opendss/bin/r4133/`)

The official EPRI OpenDSS distribution, SVN r4133 (Version 11.0.0.1), unmodified. The r4133
channel of the corpus gate loads the DLL through the `dss-epri` bridge (`epri-worker`).
`OpenDSS_Documentation.chm` is the OpenDSS documentation, the specification of the DSS
language (CLAUDE.md D1): read it as HTML after `hh.exe -decompile <dir> <chm>` with absolute
paths. hh.exe extracts nothing from a path with a space, so the file drops the space of its
distribution name `OpenDSS Documentation.chm`.

| File | Bytes | Copyright | Source in the distribution | Licence |
|---|---|---|---|---|
| `OpenDSSDirect.dll` | 11,823,616 | © EPRI | `Version8/Distrib/x64` | BSD-style, `License.txt` |
| `License.txt` | 1,760 | © EPRI | `Version8/Distrib/x64` | the licence text |
| `OpenDSS_Documentation.chm` | 33,469,295 | © EPRI | `Version8/Distrib/Doc/OpenDSS Documentation.chm` | BSD-style, `License.txt` |
| `KLUSolve.dll` | 233,472 | © 2008 EnerNex Corporation | `Version8/Distrib/x64` | LGPL-2.1-or-later, `KLUSolve_License.txt`, `KLUSolve_lgpl_2_1.txt` |
| `KLUSolve_License.txt`, `KLUSolve_lgpl_2_1.txt` | 802, 26,940 | © EnerNex Corporation, © FSF | `VersionC/klusolve` | the licence texts |
| `kmetis.exe`, `pmetis.exe` | 156,160, 109,568 | University of Minnesota | `Version8/Distrib/x64` | the METIS programs the distribution ships for A-Diakoptics tearing; it carries no licence file for them |

Not vendored: `DSSProgress.exe` and the external user-model sample `IndMach012a.dll` of the
distribution (the corpus uses the built-in `indmach012`).

## IndMach012a native twin (`tools/wasm_usermodel/twin/`)

| File | Bytes | SHA-256 | Copyright | Source | Licence |
|---|---|---|---|---|---|
| `IndMach012a.dll` | 318,976 | `090393331085a2307bb2cfe31ad1146279bc8a2123eb0c386ccd4367c8e89e0a` | © EPRI | the OpenDSS r4133 example user model IndMach012a, compiled unmodified by FPC 3.2.2 (`ppcrossx64` x86_64-win64, `-Mdelphi -O2`) | EPRI BSD-3, `License.txt` beside it; the FPC run-time library is linked in under its LGPL with the static-linking exception |

The r4133 engine loads it as the Generator user model when the wasm user-model goldens are
generated.
