# IndMach012a native twin

`IndMach012a.dll` is the OpenDSS r4133 example user model IndMach012a, compiled unmodified by
FPC 3.2.2 (`ppcrossx64` x86_64-win64, `-Mdelphi -O2`). The r4133 engine loads it as the Generator
user model when the wasm user-model goldens are generated.

| file | bytes | sha256 |
|---|---|---|
| `IndMach012a.dll` | 318976 | `090393331085a2307bb2cfe31ad1146279bc8a2123eb0c386ccd4367c8e89e0a` |

Licence: the EPRI BSD-3 licence in `License.txt`. The FPC run-time library is linked in under its
LGPL with the static-linking exception.
