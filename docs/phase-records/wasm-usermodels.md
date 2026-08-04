# WASM_USERMODELS — WM.0 … WM.7 and the D2 sub-bug trace

> Moved verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

### WASM-UM WP-WM.0 — ABI freeze + probes (branch `wasm-um`, 2026-07-18)

`WASM_USERMODELS_PLAN.md` execution started (WM.0→WM.2 authorized for this
round; WM.3+ deferred — parallel workflows own the element files). WP-WM.0 is
**docs + probe scripts only** (zero engine/product code changes):

- **`docs/wasm/USERMODEL_ABI.md` FROZEN** — export lists (15/13/7 + `memory` +
  `dss_alloc`), packed record offset tables **transcribed from FPC probe
  output** (never from reading), the 32-slot callback import table (module
  `dss_env`) with tiers + census, activation rule, failure/trap/sandbox
  policy, probe-evidence ledger. Probe kit: `tools/fpc/usermodel_abi/`
  (verbatim-extraction script + 2 offset probes + 15-export stub DLL + oracle
  driver + build script); evidence: `docs/wasm/probes/p1..p5*.txt`.
- **P1 (oracle loads native DLL): PASS, all 5 asserts** — pinned dss-python
  0.15.7/0.14.5 loads the FPC-built stub via `Generator.UserModel=`, stub
  state vars appear on the element variable surface, `UserData=` reaches
  `Edit` (len verified), Model=6 snapshot converges with terminal currents ==
  stub `Calc` output **bit-exact**, marshalled V == terminal-1 node voltage at
  4.8e-7 rel (one fixed-point iterate stale by construction — documented in
  the probe). **Plan §2.5 channel 1 CONFIRMED; r3723 fallback not engaged; the
  WM.3 audit-tier escalation clause is moot.**
- **P2 (layouts): packed confirmed** (release cfgs never set
  `DSS_CAPI_NO_PACKED_RECORDS`; probed with `-Mdelphi` + release defines,
  x86_64-win64): `TDynamicsRec` 52 B, `TGeneratorVars` 244 B (unaligned tail
  after the 3 i32s — `#[repr(C)]` would mis-pad, noted in the ABI doc),
  `TDSSCallBacks` 256 B = 32×8. dss_capi 0.14.5 vs r3723 header sets:
  **byte-identical** (twin probe over the real vendored r3723 units).
- **P2 twin decision: PLAN A** — FPC 3.2.2 `ppcrossx64 -Mdelphi` builds the
  **vendored `IndMach012a.dpr` as-is** (search paths only, zero source edits;
  `.res` links); the resulting DLL loads under the pinned oracle, all 14
  machine vars live, Model=6 solve converges with physically-sensible slip
  (−0.0064). Plan B (Rust native-shim twin) not needed.
- **P3 (callback census): `MsgCallBack` only** (`IndMach012Model.pas:474`,
  help text); parser bundled (`ModelParser`), `DoDSSCommand` **unused** ⇒ the
  WM.6 deferral stands. Full 32-slot table in the ABI doc.
- **P4 (toolchain pins):** stable rustc 1.96.0; `wasm32-unknown-unknown`
  target added (machine-global, additive); `wasmi ==1.0.9`
  (`default-features=false`, `features=["simd"]`, the typst-verified pin)
  compiles on stable with a pure-Rust closure (wasmi_core/ir/collections
  1.1.0, wasmparser 0.228.0, bitflags, libm, spin — zero C/FFI) and exposes
  fuel + store-limiter APIs (`instantiate_and_start` is the 1.x spelling).
  Initial `tools/wasm_usermodel/PIN.txt` written. No wasmi blocker ⇒ the pin
  stands.
- **§2.7 upgrade one-line diff check — one real finding:** the four loader
  units + callback vtable + `TDynamicsRec` are contract-identical across
  0.14.5→0.15.x and r3723→r4133 (host-side property→method refactors only),
  **but** 0.15.x and r4088+ insert `deltaQNom: array of Double` into
  `TGeneratorVars` between `Qnominalperphase` and `NumPhases` (+8 tail shift,
  managed reference). Frozen ABI = pinned 0.14.5/r3723 layout; caution
  recorded in the ABI doc §2.2 (never mix ≤r3723-header DLLs with
  r4088/r4133 binaries; an engine upgrade to 0.15.x semantics must revisit by
  recorded decision). Evidence `docs/wasm/probes/p5_upgrade_diff.txt`.

Follow-ups: none blocking WM.1. `TStorageVars`/`TPVSystemVars`/
`TCapControlVars` offset tables are frozen at their owning WPs (WM.4/WM.5) via
the same probe kit (ABI doc §2.4 records this explicitly).

### WASM-UM WP-WM.1 — the `dss-usermodel` crate (branch `wasm-wm1`, 2026-07-18)

The wasmi host per plan §2.1–§2.3; template = the vendored typst plugin host
(`.inputs/typst/.../plugin.rs`, cited in doc comments throughout). New leaf
workspace crate `crates/dss-usermodel/` (`#![forbid(unsafe_code)]`; deps:
`wasmi =1.0.9` pinned in `[workspace.dependencies]` per PIN.txt
(`default-features=false`, `["simd"]`), `num-complex`, `dss-parser` (the owned
AuxParser); dev-deps `wat` + `sha2`). No dss-core changes — the crate is not
consumed yet (WM.3 wires it).

- **`UserModelHost`** (`src/host.rs`): deterministic engine config (relaxed
  SIMD off = typst `:271-272`; `consume_fuel(true)`), module compile, and
  **load-time export-set validation** — `memory`, `dss_alloc`, then the
  interface functions in the **Pascal binding order**
  (`GenUserModel.pas:173-187` / `StoreUserModel.pas:336-348` /
  `CapUserControl.pas:176-182`), so the first missing name is exactly what the
  engine's 569/1569 path reports; present-but-wrong-signature = typed
  `SignatureMismatch` (ABI §6 protocol violation). `InterfaceKind` carries the
  five Pascal loader shapes (Gen15 `new(genvars,dynarec)`, Store15/PV15/Dyna13
  `new(dynarec)`, Cap7 `new()`). `HostConfig`: per-call fuel budget (default
  1e8 — the plan's <1%-of-budget calibration bar is verified against the real
  fixture at WM.2) + 64 MiB memory cap.
- **`UserModelInstance`/`CapControlInstance`** (`src/instance.rs`): one
  `Store` per element binding (plan §2.7 — `Store` is `Send`, M3-safe); guest
  buffers allocated once via `dss_alloc` (genvars 244 B / dynarec 52 B / V+I
  `yorder`×16 / name scratch; grow-only edit + vars buffers) and range-checked
  (`dss_alloc` returning 0 / out-of-range = typed error). Record shuttle =
  `GeneratorVars`/`DynamicsRec` mirrors (`src/records.rs`) serialized
  **field-by-field at the frozen ABI offsets** (never `#[repr(C)]` — the
  packed unaligned tail; unit tests pin every offset against the ABI-doc
  tables). Records written before and read back after **every** call (ABI
  §2); Pascal wrapper quirks reproduced: `Edit` ignored while `FID=0`,
  `Integrate` = `select(id)`+`integrate` (`GenUserModel.pas:123-138`),
  `new`→0 = model-absent (`Get_Exists`), delete-guard on nonzero id.
- **Callbacks** (`src/callbacks.rs` + `src/imports.rs`): the 32-slot
  `TDSSCallBacks` vtable as module `dss_env`, tiered per ABI §4 — tier A
  served from the per-call `Box<dyn Callbacks>` snapshot (default method
  bodies mirror each Pascal nil path, incl. `Exit`-without-touching
  semantics via `Option`), tier B = `Effect` queue (`Msg`,
  `ControlQueuePush` with provisional handles `seed, seed+1, …` from
  `control_queue_next_handle()` — exact Pascal handle sequence under
  drain-in-order), tier C = owned `dss_parser::Parser` in `CallData`
  reproducing `CallBackParser` semantics (`NextParam` returns the **value**
  length and copies the **name**; `GetStrValue` serves `CB_Param` from the
  last `NextParam` incl. the deterministic truncate-on-short-maxlen;
  FPC-exception-on-conversion-error = UB upstream, defined port writes 0).
  All 32 imports exist at link time; `do_dss_command`/`get_result_str`
  (WM.6) and `get_active_element_ptr` (permanent) raise the loud attributed
  `Unsupported` error when called — no silent no-ops (plan §2.9-5).
- **Typed failure contract** (`src/error.rs`, ABI §6): faults recorded by
  imports win (the typst `memory_error` take-pattern), then
  `TrapCode::OutOfFuel` → `FuelExhausted`, `GrowthOperationLimited` (store
  limiter with `trap_on_grow_failure`) → `MemoryCapExceeded`, else `Trap` —
  every variant naming the model and function.
- **Channel-2 protocol tests** (`tests/protocol.rs`, 23 tests, inline-WAT
  guests via dev-dep `wat`): happy-path 15-function round trip (call
  counters + V/I marshalling + records); record byte-exact round trip
  (distinct bit patterns in every field → guest increments `Pshaft`/`t` →
  only those change); tier A full-surface exerciser (all 22 snapshot reads
  incl. copy-semantics counts) + the Pascal nil-path defaults; tier B order +
  handle sequence; tier C parser round trip over the owned AuxParser;
  missing export → exact name + binding-order-first + 13-vs-15-fn sets +
  missing `memory`/`dss_alloc`; wrong signature; trap / fuel / import-OOB /
  host-OOB / alloc-0 / memory-cap → the typed errors; unsupported-import
  trio; CapControl 7-fn round trip; host-API misuse (`Usage`).
  `tests/fixture_pin.rs` = the **hash-vs-PIN scaffold**, self-activating
  (dormant pre-WM.2 state = no fixture + no `sha256(...)` line in PIN.txt;
  any half-state is red; format documented for WM.2) — no `#[ignore]`.

Workspace edits: members + `[workspace.dependencies].wasmi` +
`[profile.dev.package.dss-usermodel]` opt-3 (safety knobs pinned) in the root
`Cargo.toml`. Follow-ups for WM.2: commit the fixture + PIN hash line
(activates the scaffold), verify the fuel-calibration bar (<1% of 1e8 per
reference-model call).

### WASM-UM WP-WM.2 items 1–3 — IndMach012a fixture port + pinned `.wasm` + native twin (branch `wasm-wm2`, 2026-07-18)

Items 1–3 of plan §WP-WM.2 (+ the item-4 data pre-stage); item 4 (fixture
self-gate through the `dss-usermodel` API) and item 5 (audits) belong to the
integrator/workflow. Zero product-code changes — everything lives under
`tools/wasm_usermodel/`, `tests/fixtures/wasm/`, `docs/wasm/probes/`.

- **Item 1 — the port.** `tools/wasm_usermodel/models/indmach012a/`
  (workspace-excluded crate, `[workspace]` opt-out, zero deps, cdylib+rlib):
  loop-for-loop port of r3723 `IndMach012Model.pas` (machine math, slip
  clamp, dSdP, dynamic/pflow currents, trapezoidal `Integrate`, 14-variable
  surface — `model.rs`), `MainUnit.pas` (ModelList/ActiveModel semantics incl.
  the delete-clears-active quirk and the guarded/unguarded nil-ActiveModel
  split — `mainunit.rs`), the units the DLL links: `Ucomplex.pas` **as this
  source defines it** (naive `CDIV`/`Cabs`, NOT dss-core's `cdiv_fpc` Smith
  helpers — `cmath.rs`), `mathutil`/`Ucmatrix` sym-comp path with `Ap2s`
  produced by a verbatim `TcMatrix.Invert` port at init (`symcomp.rs`), and a
  minimal `ParserDel`/`Command`/`HashList` scanner (`parser.rs`; RPN-in-quotes
  reduced to a loud trap — plan-§2.6-sanctioned, decks never use it). Boundary:
  `records.rs` codecs at the frozen ABI offsets (never `#[repr(C)]`),
  `wasm_exports.rs` = the 15 exports + `dss_alloc` over a safe allocation
  registry; exactly ONE `unsafe` expression in the crate (the `dss_env.
  msg_callback` import call). Pascal citations throughout.
  `TODO(compat)`×3: truncated `0.866025403` (SetAMatrix), truncated `1.732`
  (Compute_dSdP), and **a new find** — FPC folds the all-constant `3.0/746.0`
  (HPshaft var 14) at *single* precision (both operands single-exact ⇒ FPC
  lowest-common-precision constant folding), reproduced as
  `(3.0f32/746.0f32) as f64` and proven by decomposition (plain f64 quotient
  misses the twin by 2.6e-8 rel; every other value bit-exact).
- **Item 2 — pinned artifact.** `tests/fixtures/wasm/indmach012a.wasm`
  committed (79045 B; exports = the 15 + `memory` + `dss_alloc`, sole import
  `dss_env.msg_callback` — verified by wasm section parse). Reproducible
  build proven (clean rebuild ⇒ identical SHA-256):
  `pwsh tools/wasm_usermodel/build_wasm.ps1` (stable rustc 1.96.0,
  `--remap-path-prefix`, locked release profile). PIN.txt updated with the
  exact command + `sha256=1849db0c…9b0ebd` (the WM.1 hash-vs-PIN test binds
  to it at integration).
- **Item 3 — native twin (plan A) + item-4 pre-stage.**
  `build_native.ps1` builds the vendored `IndMach012a.dpr` on demand (FPC
  3.2.2 ppcrossx64, exact P2 flags; `%TEMP%` output, never committed).
  `twin_probe.py` (ctypes over the frozen packed layouts, struct-offset
  asserts) drives the twin through a deterministic lifecycle — New →
  var-name surface (incl. StrLCopy truncation + out-of-range no-write) →
  initial vars → Edit (abbrev `maxs`, case `Xm`, `option=variableslip`,
  slip→Speed write) → 5 pflow `Calc` iterations (slip fixed-point) → `Init`
  → 3 dynamics steps × predictor/corrector `Calc`+`Integrate` → SetVariable
  → GetAllVars → Select edges → `help` (MsgCallBack text) → second instance
  + Delete — and records ~200 values bit-exactly:
  `docs/wasm/probes/p6_twin_expected.txt` (evidence) + generated
  `tests/twin_expected.rs` (`--rust` mode). `tests/twin_parity.rs` replays
  the identical scenario on the Rust port and asserts **f64-bit-exact
  equality on every value** — green (2/2; `cargo +stable test` in the crate).
  The integrator pins the committed `.wasm` against the same values through
  the WM.1 crate API (item 4).
- **Hygiene:** `.gitignore` +`tools/wasm_usermodel/models/*/target/`;
  fixture crate is fmt/clippy-clean on host and wasm targets (not part of
  the repo gate — workspace-excluded by design). Machine-global additions:
  none required beyond WM.0's pins (the stray `rustup target add` on the
  default *nightly* toolchain during this session is additive-only; the
  fixture builds with `+stable` per PIN).
- Deviation note: plan §2.6 sketches the guest as "`#![no_std]`-lean"; the
  crate uses std (wasm32 std = the allocator/panic machinery only — no WASI,
  no imports beyond `dss_env`, verified in the artifact's import section).
  Chosen to keep the boundary in safe Rust (registry over `Box<[u8]>`); the
  sandbox/determinism contract is unaffected.

### WASM-UM WM.1+WM.2 integration — merge + WM.2 item 4 fixture self-gate (branch `wasm-um`, 2026-07-19)

Merged `wasm-wm1` (WM.1, fast-forward) then `wasm-wm2` (WM.2 items 1–3; sole
conflict = STATUS.md section placement, resolved by union — both records kept
in WP order). Workspace `Cargo.toml`/`Cargo.lock` merged clean (WM.2's fixture
crate is workspace-excluded by design). Cross-WP items neither side could do
alone:

- **WM.2 item 4 — fixture self-gate:**
  `crates/dss-usermodel/tests/fixture_self_gate.rs` drives the COMMITTED
  `tests/fixtures/wasm/indmach012a.wasm` through the full crate API chain
  (`UserModelHost::load` Gen15 export validation → `UserModelInstance` record
  shuttle → guest math) with hand-fed V/records, replaying the
  `twin_probe.py` scenario S1–S11+S13 and pinning every recorded value
  **f64-bit-exact** against the native-twin constants (`twin_expected.rs`,
  evidence `docs/wasm/probes/p6_twin_expected.txt`): ~200 pins — currents (5
  pflow + 6 dynamics calc), slip fixed-point, all 14 vars at 6 checkpoints,
  GenVars `Speed` write-backs (new/edit/init/setvar), var names, `help`
  MsgCallBack text byte-identical through the `dss_env` effect queue. S12/S14
  (foreign-id select, two models in one guest) are single-shared-DLL
  artifacts the per-element-instance design deliberately does not expose
  (plan §2.7); pinned instead: own-id select round-trip + a fresh-guest
  second instance (id=1 again, post-Create pins verbatim) + delete clears
  `exists`. GREEN — the committed binary and the WM.1 shuttle agree with the
  FPC twin bit-for-bit.
- **Hash-vs-PIN scaffold now ACTIVE:** with the fixture + PIN sha256 line
  committed, `fixture_pin.rs::committed_fixture_hash_matches_pin` takes the
  active branch and verifies `1849db0c…9b0ebd` — green.
- **WM.1 fuel-calibration follow-up settled:** the plan's bar (<1% of the
  1e8 default per reference-model call) is proven by a second self-gate run
  under `fuel_per_call = DEFAULT/100` — the entire scenario (instantiation
  included) completes with no `FuelExhausted`.
- Fixture crate's own `twin_parity` suite re-verified green post-merge;
  full three-command gate green at default settings; `tests/corpus` pristine
  (stray solver outputs from an aborted run removed path-limited). One
  transient on the first full-gate run: `corpus_gate` CapiV0145
  `asymmetric:isource/isource_snap.dss` step-0 voltage off by 3.6e-3 (>floor
  8.2e-6), NOT reproducible — same binary re-run green twice (513→514/514 and
  the full workspace re-run), diff touches no dss-core code. Suspected
  cross-session oracle-server contention (parallel workflows active); watch
  if it recurs — a reproducible hit would need the CLAUDE.md prove-it
  discipline, not a shrug.

### WASM-UM WM.0–WM.2 settle — audit dispositions (branch `wasm-um`, 2026-07-19)

Two independent `opus-xhigh` audits of `ae4b4ef..4051d833` (audit-code +
audit-tests): **verdict faithful, zero Critical/Major**; both independently
re-derived the ABI offsets, rebuilt the native twin AND the wasm fixture
(byte-identical to the PIN), and re-ran the full gate. Findings settled
empirically:

- **WM-AUD-1 (Minor, FIXED — doc):** the frozen ABI doc omitted the
  `get_node_voltages` ground-slot indexing decision (native
  `GetPtrToSystemVarrayCallBack` returns the raw `Solution.NodeV` pointer
  whose offset-0 element IS ground, `Solution.pas:88/:198`; the crate contract
  serves `NodeV[1..NumNodes]` ground-excluded). Recorded via the doc's
  recorded-decision mechanism (header note + §4 row 17 + indexing note with
  the porting consequence) **before WM.3 wires the slot**; contract unchanged.
- **WM-AUD-2 (Minor, premise DISPROVEN by probe; residual recorded):** FPC
  3.2.2 `Val` was probed directly (ppcrossx64 x64 exe —
  `docs/wasm/probes/p7_fpc_val_domain.txt`): it **accepts** `inf`/`nan` (any
  case) and leading spaces, exactly like Rust `parse::<f64>()` — the fixture
  matches the spec there. Residual Rust-wider domain (`infinity`,
  trailing/tab whitespace via `trim`) is unreachable (tokenizer never yields
  whitespace-padded unquoted tokens; quoted branch is RPN upstream, whose own
  tokenizer skips whitespace; no deck feeds `infinity`). Deliberately NOT
  changed. Lesson captured: the fixture **source is hash-frozen with the
  artifact** — a comment-only parser.rs edit shifts panic-`Location` line
  numbers and changes the built wasm hash (verified: pristine rebuild = the
  pinned `1849db0c…`, +10-comment-lines rebuild = `7da8ee46…`), so fixture
  notes live in probes/STATUS, never as source edits without a deliberate
  re-pin.
- **WM-T4 (Minor, FIXED):** `fixture_pin.rs`'s pre-WM.2 dormant arm
  (no fixture + no PIN line = pass) retired — the fixture is permanent as of
  WM.2, so both halves are now required unconditionally (simultaneous
  deletion of fixture + PIN line is red). Strictly strengthens the gate.
- **WM-T1 (Informational, ACCEPTED — plan-sanctioned):** the fixture crate's
  `twin_parity` suite + fmt/clippy are workspace-excluded by design (plan
  §2.6); the committed artifact IS gated every `cargo test` (self-gate ~200
  bit pins + hash-vs-PIN). Follow-through: the plan's WM.7 exit sweep now
  lists an explicit `twin_parity`+fmt/clippy re-run for the fixture crates.
- **WM-T3 (Informational, ACCEPTED as designed):** twin scenarios S12/S14
  (foreign-id select, two models in one guest) are single-shared-DLL
  artifacts the per-element-instance design never exposes (plan §2.7);
  crate-level surrogates pin the equivalent paths — already documented in the
  self-gate header and the integration record.
- **WM-AUD-4 / WM-T2 (watch item, STANDS):** the one non-reproducible
  `corpus_gate` transient (isource_snap step-0, 3.6e-3 vs floor 8.2e-6; green
  on 4 total re-runs across author+auditor) stays a recorded watch — a
  reproducible hit gets the CLAUDE.md prove-it discipline.
- **WM-AUD-3 (environmental):** a parallel session's corpus_gate run wrote
  stray solver outputs into this worktree during the audit; the committed
  range was verified clean at audit start. `tests/corpus` re-verified
  pristine at settle.

### WASM-UM ABI re-freeze to r4133 (pre-WM.3, branch `wasm-r4133`, 2026-07-19)

User decision 2026-07-19: the project gates on the in-house **r4133** bridge
(`crates/dss-epri`), so the frozen user-model ABI + native twin move from the
0.14.5/r3723 layout to r4133. Executed as the ABI doc's own recorded-decision
procedure (header entry (b) + updated §2.2 + Appendix A). **Probe, not assume:**

- **P8 r4133 offsets** (`abi_probe_r4133.pas` → `docs/wasm/probes/p8_offsets_r4133.txt`):
  FPC `-Mdelphi` probe of the r4133 headers. `TDynamicsRec` 52 B and
  `TDSSCallBacks` 256 B **byte-identical** r3723→r4133; `TGeneratorVars`
  **244→252 B** — `deltaQNom` (`array of Double`, 8-B managed ref, NCIM-only)
  at offset 176, tail shifted +8 (`NumPhases` 176→184, `VthevMag` 188→196,
  `XRdp` 236→244).
- **P8 model-math diff** (`p8_indmach012a_math_diff.txt`): the IndMach012a
  example dir (`IndMach012Model.pas`/`MainUnit.pas`/`ParserDel.pas`/`.dpr`) is
  **byte-identical** r3723→r4133 (sha256s); the sole delta is
  `GeneratorVars.pas`. **Model math UNCHANGED.**
- **P8 twin-in-r4133-engine** (`p8_twin_r4133_bridge.txt`): the twin rebuilt
  from r4133 (252-B layout) **loads + runs in the r4133 engine via the
  `epri-worker` bridge** — model=6 power-flow converged with the 14 IndMach012a
  machine vars live (Slip=−0.006407, puRs/puXm/MaxSlip echoing UserData,
  Is1/Ir1/StatorLoss/HPshaft computed) + 10 dynamics steps (Monitor mode=3
  Slip/Freq series). This is the authoritative re-derivation through the r4133
  channel (never Rust-vs-Rust).

**Decision:** the frozen **native** `TGeneratorVars` is now r4133 (252 B,
§2.2a). The **wasm marshaled image is UNCHANGED at 244 B** (§2.2b): `deltaQNom`
is engine-only and a managed reference has no wasm-linear-memory meaning, so it
**never crosses** the boundary; the +8 shift is native-side only. Consequently
**zero wasm-side bytes changed** — proven empirically:

- `twin_probe.py` ctypes image updated to the r4133 252-B layout (`deltaQNom`
  slot, nil); driving the r4133 twin re-derives `twin_expected.rs` +
  `p6_twin_expected.txt` **bit-identical** to the r3723 pins (193 constants,
  all value lines byte-equal).
- committed `tests/fixtures/wasm/indmach012a.wasm` **unchanged** (PIN hash
  `1849db0c…` holds); the fixture self-gate
  (`committed_fixture_matches_native_twin_bit_exact`) stays **bit-exact green**.
- `crates/dss-usermodel::records::GeneratorVars` stays 244 B — **doc-comment +
  §2.2b framing only**, no code change; the offset-pin tests still assert
  176/188/236.

`build_native.ps1` retargeted r3723→r4133; `build_probes.ps1` gained the P8
step. Full three-command gate green at defaults; `tests/corpus` pristine
(deck driven from an isolated scratch dir, never the corpus). Out of scope
(unchanged): WM.3 element integration, any dss-core/manifest/dss-epri code.

**Settlement (two independent audits, 2026-07-19).** The code audit returned
empty (faithful; wasm side is doc + pin only, native re-freeze backed by
reproduced probe evidence). The test audit raised two **low, by-design**
observations, both settled empirically and **deliberately not "fixed"** (there
is no regression to fix):

- *WM3-1 — the r4133 re-derivation is gated by nothing new in the suite (it
  verifies the old pins, which are identical).* Settled: the IndMach012a model
  units (`IndMach012Model`/`MainUnit`/`ParserDel`/`.dpr`) were re-hashed here and
  are **byte-identical r3723→r4133** (sha256 match p8); the sole delta is the
  engine-only `deltaQNom` in `GeneratorVars.pas`, which never reaches the model.
  Identical math ⇒ identical twin outputs, so the unchanged `twin_expected.rs`,
  unchanged `.wasm`, and bit-exact-green self-gate ARE the correct, sufficient
  verification; the r4133 bridge run (`p8_twin_r4133_bridge.txt`) confirms the
  r4133-compiled twin loads + runs in the r4133 engine. FPC/wasm cannot rebuild
  in CI — this is inherent golden discipline, not a coverage gap; adding a CI
  gate is impossible and unnecessary.
- *WM3-2 — the new native 252-B offsets are asserted only in the ungated
  `twin_probe.py`; the gated `records.rs` test asserts the 244-B wasm image.*
  Settled: the r4133 offset probe (`abi_probe_r4133.pas`) was **re-compiled and
  re-run here** — output byte-identical to `p8_offsets_r4133.txt`
  (`TGeneratorVars` 252 B, `deltaQNom`@176, `NumPhases`@184, `VthevMag`@196),
  matching `twin_probe.py`'s ctypes asserts. The native record has **no host
  codec** in `dss-usermodel` and **never crosses the wasm boundary**, so there
  is nothing in the product to gate it against; the manual twin tool — which
  must match the native DLL byte-for-byte — is its correct and only home. The
  gated `records.rs` pin covers exactly what crosses (the 244-B wasm image). By
  design, not a regression.

### WASM-UM WP-WM.3 — Generator integration + oracle gate (branch `wasm-wm3`, 2026-07-19)

The flagship WP: Generator `UserModel=`/`UserData=`/`ShaftModel=`/`ShaftData=`
over WASM, all `generator.pas` §1.1 call sites, first end-to-end oracle gate
through the r4133 bridge (recorded decision — not pinned dss-python).

**Landed (plan §WP-WM.3 items 1–6):**
1. **Element wiring** — `ShaftModel`/`ShaftData` flip from `NOT_PORTED` to the
   §2.4 uniform rule; `EndEdit` dispatch order (UserModel→UserData,
   ShaftModel→ShaftData) queues a *deferred* load resolved by the executive
   (`exec/command.rs`, path → `.wasm` bytes or `None`); `update_model` after
   `RecalcElementData` (`nominal.rs`). New `generator/user_model.rs`:
   `GenUserModelSlot` (Pascal `TGenUserModel`) wrapping `dss-usermodel`.
2. **Call sites** — `DoUserModel` power-flow (`solve.rs::do_user_model`, negate
   into `InjCurrent`), GenModel=6 dynamics + shaft `FCalc` (`dynamics.rs`),
   `InitStateVars`/`IntegrateStates` FInit/FIntegrate for both models, the
   state-variable surface (`num_variables`/`variable_name`/`get_all_variables`/
   `set_variable` concatenate built-in ++ UserModel ++ ShaftModel). The dropped
   model-6 dynamics error (`dynamics.rs:239-254`) is now a surfaced #5671.
3. **Decks** `tools/golden/wasm_decks/wasm_gen_{pflow,dyn,vars,edit}.dss`
   (`@FIXTURE@` twin form), oracle-validated.
4. **Gate** — `crates/dss-core/tests/wasm_usermodels.rs` (channel 1, hermetic:
   committed `.wasm` vs committed r4133-oracle goldens); goldens generated by
   `crates/dss-epri/tests/gen_wasm_usermodels.rs` (env-gated `WASM_TWIN_DLL`,
   manual only). **pflow/vars/edit match the r4133 oracle at ~1e-14** (feeder
   floor); the `Get StateVar` single-variable path matches too.
5. **Invariant** — the five `expect_warnings` corpus decks + `solvable_now.json`
   manifest byte-unchanged; corpus_live gate green (see item-5 line below).
6. Audits pending (workflow spawns them).

**Golden schema fix (real bug in the WIP).** The golden keyed `variables` by
name in a JSON **map**, which SILENTLY COLLAPSED the dyn deck's 34-var surface to
20 (binding the same IndMach012a as both `UserModel=` and `ShaftModel=` gives 6
built-in + 14 + 14, the 14 ShaftModel names duplicating the UserModel's). The
prior run misread this collapse as "oracle reports 20 vs Rust 34" and got stuck
on a non-existent asymmetry. Fixed: order-preserving parallel `variable_names`/
`variable_values` arrays; a live r4133 probe confirms the oracle reports **34**
(Rust = Pascal `NumVariables` = oracle). Added `all_wasm_decks_have_consistent_goldens`.

**Two MEASURED dss_capi-0.14.5-vs-r4133 divergences (Generator DYNAMICS).** The
dyn deck is the first dynamics comparison against r4133 and exposed them (probed
both, per the brief's "if the two disagree, STOP and record"):
- **D1 — swing-damping default.** dss_capi 0.14.5 `generator.pas:1006` sets
  `Dpu:=1.0` (→ `D=Dpu*kVArating*1000/w0≈13263`); r4133 `generator.pas:968` sets
  `D:=1.0` in the ctor but **never `Dpu`**, so `InitStateVars` recomputes
  `D:=Dpu*…=0`. Rust ports dss_capi (D≈13263), r4133 default D=0. Proven by the
  oracle `DebugTrace` (predictor dSpeed +0.0025 with D≈0 vs −1.165 with D≈13263).
  The dyn deck now sets `D=1` explicitly (both engines agree); recorded in
  `DIVERGENCES.md` L5.
- **D2 — residual dynamics trajectory gap.** With D matched, a residual survives,
  MEASURED at the deck's end state (settlement flipped `numeric=true` transiently
  to quantify): ~5e-4 rel on machine currents (Is1/Ir1), ~1e-3 on losses
  (StatorLoss/RotorLoss/HPshaft), ~1e-4 on Slip, up to ~5e-4 on node voltages,
  dSpeed ~3e-2 (the `Pshaft+TracePower` near-cancellation amplifies the current
  gap). These are **4–6 orders above the faer-vs-KLU floor (1e-8)** — an
  engine-behavior difference, not solver rounding, so a numeric gate is impossible
  without a ~1e-1 band = forbidden fudging. **DECOMPOSITION (settlement, per
  CLAUDE.md prove-by-decomposition):** WM.3's NEW code is exonerated — the WASM
  `FCalc` handoff is bit-exact (Model=6 SNAPSHOT `wasm_gen_pflow` matches r4133 at
  ~1e-14 incl. Is1/slip) and the guest math is bit-exact to the native twin (WM.2
  `fixture_self_gate`). So the gap lives in the **shared multi-step Generator
  dynamics coupling** (dynamics-Norton/Zthev entry + the per-step network re-solve
  feeding Vterminal back to the identical guest), the SAME code family the proven
  D1 `Dpu` divergence sits in — not the new user-model transport. Independent
  corroboration that Rust's Generator dynamics tracks its 0.14.5 spec oracle:
  `exec/tests/dynamics.rs` (Kundur steady + fault) matches pinned dss-python 0.15.7
  to the f32 monitor floor. Pinning the single r4133 source line (like D1's) needs
  the 0.14.5-ABI twin DLL → the OPEN follow-up (deviation (a)); **not a Rust bug on
  the WM.3 surface, not a tolerance-loosening candidate.**

**Gate design consequence.** Rust ports dss_capi 0.14.5 (its pinned oracle);
forcing its dynamics to match r4133 would DIVERGE from the spec, and loosening
the harness floor is forbidden. So the dyn deck gates the **version-independent**
facts robustly — convergence, error-free (no WASM trap/protocol fault through the
user *and* shaft FInit/FIntegrate/FCalc), and the full ordered **34-variable
surface (names + count)** vs the oracle (proving the shaft loaded and the
NumVars/GetAllVars/GetVarName plumbing) — and does NOT floor-compare the
confounded dynamics trajectory (`gate_deck(.., numeric=false)`). pflow/vars/edit
remain full-numeric (~1e-14), proving the WASM `calc`/`edit`/state-var transport.

**Deviations / open items.** (a) D2's single-line r4133 root-cause is the OPEN
follow-up (the gap is decomposed and scoped OUT of the WM.3 surface — see D2
above); needs the 0.14.5-ABI twin DLL. (b) Channel-3 diagnostic (IndMach012a-over-
WASM vs the built-in IndMach012 element) NOT implemented — deferred (the channel-1
gate is the WP's oracle proof; channel 3 is a reported non-gating nicety, and the
two elements' different host coupling makes it a loose visual diagnostic, not a
clean comparison — kept deferred rather than adding a meaningless assertion). (c)
The pinned dss-python 0.14.5 secondary channel skipped (needs the r3723-built twin
— the recorded decision permits skipping if not cheap).

**Settlement (post-audit, two xhigh audits, 2026-07-19).** Five findings settled
empirically:
- **WM3-1 (HIGH, silent fallback on trap) — FIXED.** The inject path built the
  trap / missing-Model=6 diagnostics in a LOCAL `ErrorLog` and dropped them (the
  `inj_currents` trait method has no `Result`). Added `errors`+`solution_abort`
  channels to `InjCtx` (traits.rs); `Generator::inj_currents` drains them into the
  solution `ErrorLog` (= `Dss::errors()`) and lifts `SolutionAbort` from any
  `abort`-flagged diagnostic. A wasm `calc` **trap** is now a hard `abort` (ABI
  §6); the missing dynamics model (#5671) aborts (Pascal `generator.pas:1944`); the
  power-flow #567 surfaces non-abort (Pascal `DoSimpleMsg`). `get_currents`'
  recompute routes to the element deferred-error log instead of dropping. Guarded
  so a NATIVE-DLL `UserModel=` the wasm host cannot load (already loud via #570
  "Not Loaded, falls back") does NOT also spew #567/#5671 — that keeps the corpus
  gate green (indmachtest / Kersting4wire `UserModel=Indmach012a`). New regression
  test `model6_without_usermodel_surfaces_diagnostic`.
- **WM3-3 (GenVars read-back was a 9-field whitelist) — FIXED.** `apply_gen_vars`
  now writes back the FULL 244-B image's mapped f64/Complex fields (the ABI §2
  "unconditional read-back" contract), not a state-only subset; only the structural
  ints (`num_phases`/`num_conductors`/`conn`) stay element-owned (documented).
  Behaviour-neutral for the fixture (it mutates only `Speed`; untouched fields
  round-trip bit-exact) — gate stays green while the frozen contract is honoured.
- **WM3-4 (`set_variable` 1..6 was a silent no-op) — FIXED.** Ported the classic
  GenVars setters (`generator.pas:2650-2663`): Speed/Theta/PShaft/dSpeed/dTheta with
  the unit conversions, index-3 read-only #564, i<1 #565, DynamicEq #566. Reachable
  via the capi015 `set StateVar=x <elem> <var> <value>` form (the natural positional
  syntax is the upstream-broken misroute already reproduced in
  `force_hooks.rs`). New regression test `set_statevar_classic_genvars_mutates`.
- **WM3-5 / audit-tests-2 (state-var floor 1e-6/1e-5 uncalibrated) — RECALIBRATED
  (tightened).** The comment falsely claimed "1e-6 measured"; measuring shows every
  macro state var agrees to <1e-13 rel and only the near-zero quadrature currents
  Is2/Ir2 (~4.3e-7, |abs| gap ~1.4e-13) are loose. New floor `(1e-8 rel, 1e-12
  abs)`: rel = the `feeder` voltage class the machine vars inherit, abs = the
  measured near-zero floor + ~7x margin — **100x tighter (rel) / 1e7x tighter (abs)**
  than before, never loosened. Now Is2/Ir2 are constrained by an abs band instead
  of an 1e-5 band 8 orders above the signal.
- **audit-code WM3-5 (doc inaccuracy) — FIXED.** The `variable_name` comment
  claimed "the gate is designed not to compare shaft names"; corrected to state the
  gate DOES compare all 34 names and why the (correct, non-reproduced-UB) shaft
  names coincide with the buggy upstream `FGetVarName` read.
- **WM3-2 / audit-tests-1 (dynamics numeric gate) — settled by decomposition, no
  code change.** See D2 above: the trajectory gap is proven OUT of the WM.3 surface
  and is a version-divergence family, so structural-only gating is correct; a
  numeric r4133 gate would require forbidden loosening. Monitor mode-1/3 capture is
  subsumed by the same confound (the mode-3 channel IS the 34-var surface, already
  gated).

### WM.3 D2 follow-up — the ~5e-4 gap is a PORT BUG, not a version divergence (branch `wm3-d2`)

The WM.3 settlement HYPOTHESISED D2 (the residual ~5e-4 `wasm_gen_dyn`
Rust-vs-r4133 trajectory gap) is a dss_capi-0.14.5-vs-r4133 engine-version
divergence (D1 family) and gated the deck structurally. The follow-up built the
**0.14.5-ABI twin** and ran the disambiguation the settlement deferred. **That
hypothesis is DISPROVEN — VERDICT (b), PORT BUG.** Evidence
`docs/wasm/probes/p_d2_threeway_portbug.txt`.

- **ABI check (empirical, source-level).** dss_capi 0.14.5 `TGeneratorVars` = 244 B
  (== r3723 GeneratorVars.pas, byte-identical); r4133 inserts `deltaQNom`@176 →
  252 B. The ABI **differs**, so the committed 252-B twin can't drive the pinned
  dss-python 0.14.5. Built the **0.14.5-ABI twin** from r3723 V8 IndMach012a
  (`tools/wasm_usermodel/build_native_r3723.ps1`; model files sha256-identical to
  the r4133 twin — only GeneratorVars differs, so the sole controlled variable is
  the engine version). sha256 `090393…89E0A`, loads + solves the full deck on
  pinned dss-python 0.15.7/0.14.5.
- **Three-way (end-state).** A = Rust+wasm, B = 0.14.5+244B-twin, C = r4133 golden.
  **B vs C ≤ 1.06e-13 on every quantity** (slip/Is1/Ir1/losses/HPshaft/dSpeed/node
  V) — the two engine VERSIONS agree to the faer-vs-KLU floor; there is **no**
  0.14.5-vs-r4133 divergence here. **A vs C == A vs B** (Is1 5.3e-4, Ir1 5.3e-4,
  losses ~1.1e-3, dSpeed 3.4e-2, node-V.im 5.1e-4, slip 9.8e-5) — Rust diverges
  from BOTH oracles, incl. its own pinned 0.14.5 spec, by the identical amount. So
  it is Rust's port that is wrong, not r4133.
- **First divergence.** Step 0 (snapshot, `calc_pflow`) is BIT-IDENTICAL A==B==C
  (1e-14). Step 1 (first dynamics step) already diverges: **Slip matches (2.4e-6)
  but Is1 is off 5.5e-4** for both the user and shaft model instances (which agree
  with each other to ~2e-6, as in the oracle). `is1 = (v1-e1)/zsp` with slip and
  zsp (snapshot) matched → the divergence is in the dynamic flux `e1`/terminal
  voltage `v1` of the Model=6 dynamics network solve.
- **NOT conditioning.** `Set tolerance=1e-12 maxiterations=1000` on both engines
  leaves the ~5e-4 gap intact (each engine's own value moves <7e-6) — they
  converge tightly to DIFFERENT fixpoints (per CLAUDE.md's tighten-the-loop rule),
  a genuine state divergence, not a Norton/Zthev convergence-band artifact.
- **Sub-bug #1 FIXED (`generator/user_model.rs::shaft_model_fcalc`).** Pascal
  `DoDynamicMode:2038` `ShaftModel.FCalc(Vterminal, Iterminal)` OVERWRITES the live
  `Iterminal` (last write), which `IntegrateStates`' `ComputeIterminal` then reuses
  → `TracePower` reads the SHAFT model's currents. Rust discarded them into a
  scratch buffer (kept the user currents). Fixed to write back (Pascal-cited).
  Step-1 `dSpeed` −75.66→−84.80 toward oracle −89.24; **end-state effect negligible
  (Is1 unchanged)** because sub-bug #2 dominates. Contained to Model=6+ShaftModel
  (only `wasm_gen_dyn`; no corpus case); full gate green.
- **Sub-bug #2 OPEN (dominant).** The Model=6 dynamic-current fixpoint is off ~5e-4
  from step 1 with slip matched. Guest math is bit-exact to the twin
  (`fixture_self_gate`), so the dss-core Generator dynamics host feeds the guest
  state differing from what Pascal feeds the twin (checked-and-matched: snapshot
  1e-14, slip, Vterminal mag+angle, w0/Mmass/D/Pshaft). Line-level pin needs
  guest-internal `e1`/`t0p` tracing = rebuilding the WM.4-constraint-frozen fixture
  `.wasm`, out of scope here. **Left as the OPEN follow-up port bug.**
- **Gate design.** The dyn deck stays STRUCTURAL (`numeric=false`) until sub-bug #2
  is fixed — flipping it to numeric now would need a forbidden ~1e-1 band. No
  tolerance/golden/ledger touched. **DIVERGENCES.md gets NO entry** — D2 is a port
  bug, not a version divergence (an entry would misrepresent the finding).

**Settle (two independent read-only audits of `1d94256..cbdff5c`; per-finding
dispositions).** Both audits verified the change does NOT weaken behavior/coverage
(no test/harness/golden/tolerance/ledger/corpus file touched; the fix is
Pascal-faithful; verdict logic is anti-rationalizing — PORT BUG, not a version
hand-wave). Corpus pristine, 186 `.pas` under `.inputs/dss_capi`, full three-command
gate green. Findings settled empirically:

- **AUDIT-CODE D2-1 (medium) — mandate verdict-(b) "fix it" only partly met; the
  DOMINANT sub-bug #2 left OPEN → REGISTER-AS-OPEN (deliberate deferral, not
  resolved).** Reproduced: `p_d2_threeway_portbug.txt:98-111` + gate line
  `wasm_usermodels.rs:352` `gate_deck("wasm_gen_dyn", false)` both stand; sub-bug #2
  is a PROVEN, dominant, still-OPEN port bug. Not fixed here because a line-level pin
  needs guest-internal `e1`/`t0p` tracing, which requires rebuilding the fixture
  `.wasm` — and the fixture crates are frozen by the live parallel **WM.4** workflow
  (`.claude/worktrees/wtWM4`, branch `wasm-wm4`, confirmed active). A fix without that
  trace would violate CLAUDE.md prove-cause discipline (guessing). Disposition: D2 is
  recorded as a **proven-and-open port bug**, NOT a resolved one — see Open
  follow-ups below; the coordinator must carry it forward.
- **AUDIT-CODE D2-2 (low) — `d2_step_0145.py` docstring said "21 dynamics steps" but
  the loop is `range(1, 6)` = 5 → FIXED.** Docstring corrected to state the first 5
  steps (`range(1, 6)`) and that step 1 already exposes the divergence, with the full
  end-state captured by `d2_probe_0145.py`. Cosmetic; no behavior/verdict effect.
- **AUDIT-TESTS D2-1 (low) — shaft-FCalc fix has no numeric regression guard →
  DEFERRED to the sub-bug #2 fix (deliberate).** Reproduced and sharpened: the fix's
  ONLY observable signal is step-1 `dSpeed` (−75.66→−84.80); the deck END-STATE (`Is1`
  unchanged) does not move because sub-bug #2 dominates. So neither the structural
  gate NOR an end-state numeric golden could pin this fix — a guard would need
  per-step (step-1) oracle capture, i.e. new golden infra bound to the WM.4-frozen
  fixtures. The proper trajectory guard therefore arrives WITH the sub-bug #2 fix,
  when the whole trajectory becomes numerically gateable at proven floors. No
  tolerance touched.
- **AUDIT-TESTS D2-2 (low) — structural gate cannot detect worsening of the ~5e-4
  bug; interim known-bad band suggested → interim band DECLINED, DEFERRED (deliberate).**
  An end-state "known-bad-within-N%" band was considered and declined: the trajectory
  quantities span orders (Is1 ~5e-4 … dSpeed ~3.4e-2), so a hand-picked band is
  miscalibration/flake-prone; it would institutionalize a bug we intend to FIX (per
  mandate 3(b) the gate flips to numeric ON the fix, not around it); and the shared
  driver `wasm_usermodels.rs` is also live under WM.4 (conflict risk). Per the mandate
  the numeric gate (and any numeric bound) is explicitly gated on fixing sub-bug #2 —
  done then, at proven floors, never a fudge band now. No tolerance loosened.

**Open follow-up — RESOLVED.** Traced (branch `d2-subbug2`) then FIXED (branch
`d2-subbug2-r2`, §"D2 sub-bug #2 — FIXED (round 2)" at the end of this file): the
dynamics-entry `FInit` seed used the refreshed `V_n` instead of Pascal's stale
`Vterminal` (`V_{n-1}`). `wasm_usermodels.rs` now
`gate_deck("wasm_gen_dyn", true)` + a step-1 trajectory guard.

### WM.3 D2 sub-bug #2 trace — the divergence is ENGINE-FLOW, guest is bit-exact (branch `d2-subbug2`)

The mandated guest-internal `e1`/`t0p` trace (fixture instrumentation lifted for
this WP; reverted at landing — committed `.wasm`/PIN untouched). It DISPROVES two
candidate localizations and narrows the PROVEN port bug to the Generator Model=6
dynamics ENGINE FLOW. Reproducible probes committed: `tools/wasm_usermodel/
d2_twin_direct.py` (direct DLL drive), `d2_guesttrace_0145.py` (guest DebugTrace),
`d2_recon_e1.py` (full-f64 E1 reconstruction). Instrumented artifacts stayed in the
session scratchpad.

**First divergence (single-instance UserModel, no shaft, wasm_gen_dyn base).**
Step 0 (snapshot) A==B bit-identical (slip `-6.18230011826962e-3`, |Is1|
`189.1007524589`). Step 1: slip matches ~2e-6, but the guest flux `E1` and hence
|Is1| diverge — Rust E1 stays at the **power-flow point** `(7252.814, 1530.375)`
(|Is1| 189.102, |E1| 7412.513); the 0.14.5 oracle jumps to the **dynamic operating
point** `(7252.279, 1531.158)` (|Is1| 189.207, |E1| 7412.152) and holds. The
offset `(-0.535, +0.783)` is established entirely in step 1 and persists (steps 2+
move identically). Conn=wye and conn=delta behave identically (not a delta bug).

**The jump is h-INDEPENDENT.** Oracle |E1|@step1 is `7412.152` for h ∈ {1e-9, 1e-7,
1.67e-4} (varies < 5e-4). A trapezoidal integrate step scales with h; an
h-independent move to the dynamic point is an algebraic/network effect, NOT the
integrator. (`d2_recon_e1.py` with `H=…`; `d2_guesttrace_0145.py` with `H=1e-9`.)

**The GUEST is bit-exact — the bug is NOT the WASM transport nor the guest Init.**
`d2_twin_direct.py` drives the r3723 244-B twin DLL DIRECTLY (ctypes, no OpenDSS
engine) through the exact lifecycle New→Edit→CalcPFlow→Init→CalcDynamic with the
deck's converged snapshot V/I: it returns |Is1| = **189.1007524589, BIT-IDENTICAL
to the Rust engine**. So the twin's Init+CalcDynamic gives the power-flow point,
exactly like Rust. Corroborated: Init inputs (Vterminal + Iterminal, all 3 phases)
are bit-exact host-side (13 sig figs, `D2HOST` host trace); post-Init guest `is1`
(var 8) = 189.10 (pflow) on BOTH — no pre-Init CalcDynamic. So the ~5e-4 D2 gap
lives ENTIRELY in how the OpenDSS **engine** drives the machine to the dynamic
operating point at the first dynamics step; Rust's engine leaves it at the
power-flow point (consistent with the guest's own math).

**First-divergence localization (settled empirically at the D2 settle — the guest
is exonerated ON the divergent path, not just at step 0).** The earlier trace
narrowed the jump to "engine-flow" but rested on the step-0/pflow agreement and
never exercised the guest's trapezoidal `Integrate` in isolation (audit-code
D2F-1). The settle ran the decisive experiment (`tools/wasm_usermodel/
d2_twin_step.py`): drive the 244-B twin DLL directly through a full dynamics step —
`Init(Vsnap,Ipf)`, predictor `Calc`+`Integrate(flag=0)`, corrector
`Calc`+`Integrate(flag=1)` — with the terminal voltage HELD FIXED at the converged
pflow snapshot, reading `|Is1|` after every sub-call. Result: **at h=1e-9 the guest
keeps `|Is1|` at the pflow-dynamic point 189.1007 across the whole step** (the
`Integrate` change is h-scaled → negligible); at h=1.67e-4 the guest moves it to
188.77 (DOWN, h-scaled). Meanwhile the oracle ENGINE reaches 189.207 at step 1 for
h ∈ {1e-9, 1e-7, 1.67e-4} — h-INDEPENDENT (re-confirmed here via `d2_recon_e1.py`
H=1e-9 vs H=1e-7, E1 identical to 13 sig figs). So with IDENTICAL fixed inputs the
guest produces 189.10; the oracle's entire h-independent 189.10→189.207 shift comes
from the ENGINE feeding the guest DIFFERENT per-step V/currents through its network
re-solve — **not the guest math (which is the same bit-exact code) and not the WASM
transport.** This CORRECTS the prior record's framing (audit-code D2F-2): the next
step is NOT observing an un-instrumentable FPC binary — it is a loop-for-loop
compare of the Rust host's per-step dynamics network coupling (the DoDynamicMode
injection / Vterminal feed / the user↔shaft two-instance interaction) against
0.14.5 `generator.pas`, all fully inspectable Rust. Host candidates checked and
ruled OUT so far: the dynamics-mode YPrim stamping (`Y := Yeq`, /3 for delta, the
wye/delta fill) is 1:1 with `generator.pas:1313-1350`; the `SolveDynamic`
predictor/corrector order (IterationFlag 0/1, `IntegratePCStates`+`SolveSnap` ×2)
is 1:1 with `SolutionAlgs.pas:333`; the dynamics-entry `ComputeIterminal` runs in
NON-dynamic mode on both (Pascal assigns `SolutionMode` AFTER `OK_for_Dynamics`,
`Solution.pas:2017/2022`) so both feed FInit the pflow V/I. The residual suspect is
the per-step converged operating point the Rust network solve reaches vs OpenDSS —
i.e. why Rust's engine holds the guest at 189.10 while OpenDSS drives it to 189.207
with the same guest and the same Norton admittance. NOT yet pinned to a single line;
a guess-fix would violate CLAUDE.md prove-cause, so it stays OPEN (mandate item 6).

**Gate unchanged (no fudge).** `wasm_usermodels.rs` stays
`gate_deck("wasm_gen_dyn", false)`; no tolerance/golden/ledger touched;
DIVERGENCES.md untouched (port bug, not a version divergence). Flipping to numeric
now would require a forbidden ~1e-1 band. The sub-bug #1 dSpeed trajectory guard
stays DEFERRED (a proper guard needs the numeric-gateable trajectory that only
lands with the sub-bug #2 fix). Full three-command gate green; corpus pristine.

**Settle (two independent read-only audits of `634392c..3c7503f`; per-finding
dispositions).** Both audits verified the range weakens nothing (test/harness/
golden/tolerance/ledger/corpus all byte-unchanged; the WP added only STATUS + 3
probe scripts and, honestly, NO code fix — mandate item 6 block). The settle
reproduced every finding empirically and settled them; no tolerance loosened,
corpus pristine (comment-only deck edit), 186 `.pas` under `.inputs/dss_capi`,
full three-command gate green.

- **AUDIT-CODE D2F-1 (medium) — headline "guest math incl. Integrate is bit-exact"
  overreached; the divergent path (guest `Integrate`) was never exercised in
  isolation → SETTLED by running the missing experiment.** Reproduced: the prior
  `d2_twin_direct.py` stopped at `Init`+`CalcDynamic` (the step-0 value everyone
  agrees on) and never called `dll.Integrate()`. Added `d2_twin_step.py` — drives
  the twin through a full step with V held fixed and calls `Integrate()`. Result
  (above): at h=1e-9 the guest keeps `|Is1|`=189.1007; the oracle engine reaches
  189.207 h-independently. The claim now RESTS on the divergent path: the guest,
  given identical fixed inputs, gives the pflow point — the jump is engine-flow.
  Headline re-grounded on this evidence (record above + test docstring).
- **AUDIT-CODE D2F-2 (medium) — the "genuine block" was mis-attributed to an
  un-instrumentable FPC binary; the decisive step is inspectable host code →
  SETTLED, block re-attributed.** Reproduced: the prior record blamed
  "instrumentation the FPC binary does not permit." The twin-step experiment
  (no binary instrumentation needed) plus the loop-for-loop host checks (YPrim
  1:1, SolveDynamic order 1:1, dynamics-entry mode 1:1) show the open work is a
  Rust host↔network comparison, fully inspectable. The "First-divergence
  localization (settled)" paragraph above replaces the un-instrumentable-binary
  framing. Bug remains OPEN but correctly localized (not a dead-end).
- **AUDIT-CODE D2F-3 (low) / AUDIT-TESTS D2F-2 (low) — pre-existing
  `wasm_usermodels.rs` docstring still called the gap a 0.14.5-vs-r4133 version
  divergence with "WM.3's NEW code exonerated", contradicting this branch's proven
  port-bug conclusion → FIXED.** Rewrote the `gate_deck` docstring (lines ~157-181)
  to the proven three-way framing (B==C ≤1e-13, A diverges from BOTH = PORT bug,
  guest exonerated on the divergent path, gate stays structural until the fix, not
  a version divergence). Corrected the same stale sentence in the deck comment
  `tools/golden/wasm_decks/wasm_gen_dyn.dss` (comment-only, golden unaffected).
- **AUDIT-TESTS D2F-1 (medium) — mandated numeric gate flip + sub-bug #1 dSpeed
  guard NOT delivered; the proven D2 trajectory bug is ungated → REGISTER-AS-OPEN
  (deliberate non-fix, corrected rationale).** Reproduced: `gate_deck("wasm_gen_dyn",
  false)` stands; under `numeric=false` all value/iteration comparisons are skipped,
  so the corrupted trajectory is unchecked. Cannot flip to `numeric=true`: the bug
  is unfixed, so a numeric flip needs a forbidden ~1e-1 band (fudge) — declined per
  CLAUDE.md. The correct gate is bound to the fix (flip at proven floors when the
  trajectory is correct), same for the sub-bug #1 step-1 dSpeed guard (needs the
  numeric-gateable trajectory). What is now different vs the prior settle: the
  DISPROVEN "version divergence" cover-rationale that framed the ungated gap as
  legitimate is removed from both docstrings, so the gate no longer tells a reader
  the gap is expected/fine — it is labelled a proven, open port bug. No tolerance
  touched.

**Open follow-up — RESOLVED (see §"D2 sub-bug #2 — FIXED (round 2)" at the end of
this file).** The engine-flow suspect was pinned to the exact host line: the
dynamics-entry `Vterminal` FEED. Pascal `InitStateVars` seeds `UserModel.FInit`
from the STALE `Vterminal` (the power-flow's last-injection voltage `V_{n-1}`,
never `ComputeVterminal`-refreshed); the port refreshed it to the converged `V_n`,
seeding the power-flow point. Dropping the refresh reproduces the h-independent
projection (|Is1| 189.10→189.207) and the whole trajectory; `wasm_gen_dyn` is now a
full numeric gate + a step-1 guard for both sub-bugs.

### WASM-UM WP-WM.4 — Storage (DynaDLL + UserModel) + PVSystem (UserModel) (branch `wasm-wm4`, 2026-07-19)

Repeats the WM.3 pattern on two more elements. Storage `UserModel=` (15-fn
`TStoreUserModel`) + `DynaDLL=` (13-fn `TStoreDynaModel`) and PVSystem
`UserModel=` (15-fn `TPVsystemUserModel`) over WASM, gated through the r4133
bridge (recorded decision — not pinned dss-python).

**Landed (plan §WP-WM.4 items 1–4):**
1. **Storage element wiring** — `UserModel`/`UserData` flip from `NOT_PORTED` to
   the §2.4 uniform rule; `DynaDLL`/`DynaData` move from the inline warn to the
   deferred-load path (same warn-and-fallback for a native-DLL name, #1570). New
   `UserModelSlot::Dyna`; new `storage/user_model.rs` (`StorageUserModelSlot`,
   both interface kinds). Call sites: `DoUserModel` (VoltageModel=3 pflow,
   `Storage.pas:2103`), `DoDynaModel` (dynamics, `:2211` — Vterminal=NodeV,
   `StickCurr(-DESSCurr)` per phase), `InitStateVars` `FInit` (`:2777`),
   `IntegrateStates` `Integrate` (`:2859`), `RecalcElementData` `FUpdateModel`
   (`:1286`), the `IsUserModel` SOC-skip guard (`:2502`), and the full state-var
   surface (`num_variables`/`variable_name`/`get_all_variables`/`set_variable`
   append UserModel then DynaModel, `:3092-3322`).
2. **PVSystem element wiring** — `UserModel`/`UserData` flip; new
   `pvsystem/user_model.rs` (`PvUserModelSlot`). Call sites: `DoUserModel`
   (pflow + dynamics VoltageModel=3, `PVsystem.pas:1822`/`:1885`), `IntegrateStates`
   `Integrate` (`:2278`), `UpdateModel` (`:1146`), the state-var surface
   (`:2449-2640`). PVSystem `InitStateVars` has NO user-model call (Pascal
   `:2174` never calls `FInit` — verified).
3. **Fixture** — one authored model `tools/wasm_usermodel/models/wm4model/`
   (workspace-excluded): a 3-phase inverter that is a constant admittance in power
   flow and a first-order current lag in dynamics, dispatched on
   `TDynamicsRec.SolutionMode` (like IndMach012a's `Calc`). ONE `.wasm` serves
   BOTH the PVSystem `UserModel=` (15-fn) and Storage `DynaDLL=` (13-fn) gates
   (`new(dynarec)` shape; 13-fn = 15-fn minus save/restore). Reads inputs from
   `V` + `TDynamicsRec` only — NO host callbacks, NO `TStorageVars`/`TPVSystemVars`
   image crosses, so the **frozen ABI is unchanged** (no StorageVars/PVSystemVars
   offset table needed — the plan §2.4 probe extension is not required for a model
   that does not use `get_public_data`). Committed `tests/fixtures/wasm/wm4model.wasm`
   (60587 B, `sha256=dad9e74b…b77b7`, PIN.txt + `fixture_pin.rs` hash-vs-PIN).
   Native twin = the SAME Rust core as a native cdylib (plan §2.6 plan B;
   `build_wm4model_native.ps1`, sha256 `b6934757…1575`, NOT committed) — the r4133
   engine LOADS + RUNS it as Storage DynaDLL / PVSystem UserModel. Decks
   `tools/golden/wasm_decks/wasm_{pv_pflow,storage_dyn}.dss` (`@FIXTURE@` twin
   form), goldens generated by `crates/dss-epri/tests/gen_wasm_usermodels_wm4.rs`
   (env-gated `WASM_TWIN_DLL`, manual), hermetic replay
   `crates/dss-core/tests/wasm_usermodels_wm4.rs`.
4. **Invariant** — the `SimpleStorageTest*` `expect_warnings` corpus decks
   re-verified green with ZERO manifest edits (`git status tests/corpus/manifests`
   clean; full `cargo test --workspace` green — corpus gate 514/514).

**No silent fallback (WM.3 precedent, WM3-1) — settled.** Storage/PVSystem
`inj_currents` built the user/dyna-model diagnostics in a LOCAL `ErrorLog` and
DROPPED them (the same drop the WM.3 generator fix repaired). Fixed: `inj_currents`
drains into `ctx.errors` + lifts `ctx.solution_abort` for `abort`-flagged wasm
traps (ABI §6); `get_currents` routes to the element deferred-error log. New
regression tests `storage/pvsystem_model3_without_usermodel_surfaces_diagnostic`
(#567). Guarded so a native-DLL `UserModel=` name (already warned #1570 at load,
`user_model_name` non-empty) does NOT re-spew #567 — keeps the corpus gate green.

**Both gates are FULL NUMERIC vs the r4133 oracle (no version-divergence
concession, unlike WM.3 D2):**
- `wasm_pv_pflow` (PVSystem UserModel, VoltageModel=3, snapshot): node voltages +
  user-model vars (Iout1/G/B/Tau) match at **worst rel 2.7e-13**.
- `wasm_storage_dyn` (Storage DynaDLL, 2-step dynamics): matches at **worst rel
  4.3e-13**. Unlike the WM.3 Generator D2 divergence (swing-damping/Zthev-Norton
  coupling), the Storage `DoDynaModel` is a **pure per-phase current injection**
  (`StickCurrInTerminalArray(-DESSCurr)`, no swing damping, no Zthev) — there is
  no cross-version coupling to diverge, so the multi-step trajectory gates
  numerically too (MEASURED, per the brief's prove-cause discipline; no ledger
  entry — nothing to ledger).

**Base-surface finding (recorded, not a bug).** The r4133 oracle reports EMPTY
names + `-9999.99` for the 9 base InvDynVars (Storage/PVSystem) outside dynamics,
while the dss_capi-0.14.5 Rust port names/computes them — a base element-variable
surface version divergence, orthogonal to the WM.4 user-model subject. The gate
compares base names only where the oracle is non-empty and does not floor-compare
base var VALUES; the user-model tail (Iout1/G/B/Tau) is fully name+value gated.

**Deviations from the brief.** (a) The plan §2.4 anticipated freezing
`TStorageVars`/`TPVSystemVars` offset tables "with a probe extension" — NOT
required here: the authored fixtures read only `V` + `TDynamicsRec` (no
`get_public_data`), so no StorageVars/PVSystemVars image crosses the boundary and
the frozen ABI is unchanged. Recorded, no ABI change. (b) One `.wasm` + one native
twin serve both decks (the model dispatches on SolutionMode) rather than two
fixtures — the plan sketched a `storagedyn` model; this generalizes it to also
carry the PV pflow case, halving the fixture surface with no loss of coverage
(both the 13-fn DynaDLL and the 15-fn UserModel export sets + call sites are
exercised end-to-end vs the oracle).

**Settle round (2026-07-19).** Two read-only audits (code + tests) reviewed the
branch; every finding was reproduced then settled. No high/medium findings; all
six were low.

- **T-WM4-1 (tests, base var values not gated) — FIXED (strengthened).** Ran
  both decks and dumped Rust-vs-oracle for every base var: ALL non-sentinel base
  vars (physical quantities + the `9999`/`0` operation flags — kWh, kWOut,
  kvarOut, kWTotalLosses, …) match the r4133 oracle at **f64-floor, worst rel
  4.4e-16** — including the Storage 2-step dynamics trajectory (the old doc's
  "trajectory version-divergence confound" is empirically false for these vars).
  `gate_deck` now floor-compares every var except the empty-named `-9999.99`
  InvDynVar sentinels (the real base-surface version divergence). Strictly
  stronger, no tolerance touched. Still green (worst gap unchanged: node voltages
  2.7e-13 / 4.3e-13, the faer-vs-KLU last-ulp signature).
- **T-WM4-2 (tests, dead `numeric=false` branch) — FIXED (removed).** With the
  Storage dynamics trajectory proven to match numerically (T-WM4-1), the
  structural-only path had no live use and no coverage. Removed the `numeric`
  parameter; `gate_deck` is now unconditionally the full-numeric gate.
- **T-WM4-3 (tests, oracle twin shares the model core) — accepted, no fix.**
  Inherent to authored fixtures (no vendored Storage/PVSystem user-model example
  exists) and the sanctioned WM.3 pattern; the cross-engine check is genuine at
  the ENGINE/ABI level (the ~1e-13 faer-vs-KLU voltage gap proves r4133 solved it
  independently, and the goldens carry r4133-only `-9999.99` sentinels the Rust
  port does not emit). Disclosed in `gen_wasm_usermodels_wm4.rs` and above.
- **C-WM4-2 (code, PVSystem scalar `get_pv_variable` lacked the UserModel tail)
  — FIXED.** Threaded `&mut self, sys, node_v` through `get_pv_variable` /
  `get_all_pv_variables` (+ the one accessor caller) and added the
  `i > NumPVSystemVariables → get_user_model_variable` route + `PvUserModelSlot::
  get_variable`, mirroring the Storage sibling and Pascal `PVsystem.pas:2453-2461`.
  (Functionally the user tail was already surfaced via the plural `get_all_variables`
  append; this removes the scalar-helper inconsistency.)
- **C-WM4-3 (code, `refresh_var_cache` swallowed guest traps) — FIXED.**
  `refresh_var_cache` (Storage + PVSystem) now returns `LiveResult` and propagates
  a trapping `num_vars`/`get_var_name` via `?` instead of `.unwrap_or(0)` /
  `.unwrap_or_default()`; the callers (`new`/`edit`/`update_model`/…) already
  drain it, so a trap is surfaced loudly (plan §2.9-5) rather than silently
  dropping the model's state-var tail.
- **C-WM4-1 (code, `#567` suppressed for a named-but-unloaded model) — accepted,
  no fix.** Reproduced: the VoltageModel=3 `DoUserModel` path guards `#567` behind
  `user_model_name.is_empty()`, so a named-but-unloaded model (native-DLL name or
  a bad `.wasm`) does not emit the per-solve `#567` Pascal produces on
  `not UserModel.Exists`. Kept deliberately: (1) the failed-load state is already
  **surfaced loudly** once via `#1570` at load (satisfies plan §2.4 rule-4); (2)
  for the dominant wasm case — a native-DLL name — Pascal would LOAD the DLL
  (`Exists=true`, zero `#567`), so per-solve `#567` would diverge *further* from
  Pascal, not less; (3) diagnostic-only (both paths inject Yprim-only, numerically
  identical); (4) empirically UNEXERCISED — no corpus deck sets Storage/PVSystem
  `UserModel=`+VoltageModel=3 (SimpleStorageTest uses `DynaDLL=` with the default
  voltage model in snapshot mode, so `DoUserModel` is never reached). The primary
  intent — `#567` for a genuine no-model VoltageModel=3 — is enforced and tested
  (`{storage,pvsystem}_model3_without_usermodel_surfaces_diagnostic`).
- **SimpleStorageTest invariant re-verified** (settle): `git status tests/corpus`
  clean; `SimpleStorageTest*` decks green under the full `cargo test --workspace`.

Branch `og15-capi-schema`. Ported the **static core** of Pascal
`DSS_ExtractSchema(DSS, jsonSchema=True)` (`CAPI_Schema.pas:1252-1521`): the
JSON-Schema (draft 2020-12) envelope (`$schema`/`$id`/`type`/`required`), the ten
reusable global `$defs` (`Complex`, `PComplex`, `SymmetricMatrix`,
`ArrayOrFilePath`, `StringArrayOrFilePath`, `JSONFilePath`, `JSONLinesFilePath`,
`Bus`, `BusConnection`, `DynInitType`), and the static `circuitProperties` head
(`Name`/`DefaultBaseFreq`/`PreCommands`/`PostCommands`/`Bus`).
- New: `crates/dss-core/src/report/export/json/schema.rs` (reuses the existing
  fpjson `Json` tree + `write_pretty`); public `Dss::extract_schema_json()` in
  `exec/view.rs`.
- Test surface: `tools/golden/gen_schema.py` (pin-checked; proves the oracle
  bytes deterministic across two processes; self-validates each rendered fragment
  by verbatim containment in the real 592 KB oracle output), golden
  `tests/golden/json/schema_static_core.json`, driver
  `crates/dss-core/tests/golden_schema.rs` — **byte-equality** on all 10 static
  defs + the 5 head props + the `$id`/`required` envelope.

**Deferred (genuinely orphaned, blocked on unported metadata):** the per-class
walk (`prepareClassJsonSchema`) and per-enum walk (`prepareEnumJsonSchema`) —
i.e. the `<Class>`/`<Class>List`/`<Class>Container` `$defs` triples (**49 class +
21 global enum defs**) and their `circuitProperties` refs — need per-property
metadata the Rust port never carried and which is a large, self-contained
data-entry effort:
- property **help/description** text (`GetPropertyHelp`; ~1109 strings in the
  oracle document),
- per-class **`AltPropertyOrder`** (`$dssPropertyOrder`, 1161 occurrences),
- **`SpecSets`** / `SpecSetNames` / `RequiredInSpecSet` (the `oneOf` blocks, 78),
- enum **`AltNames`/`JSONName`/`JSONUseNumbers`** JSON metadata (not on `DssEnum`),
- ~28 of the ~30 `Units_*` property flags (only `UNITS_HOUR` /
  `UNITS_OHM_PER_LENGTH` exist on `PropFlags` today).

The mission-brief premise that these inputs "already sit inert, ready to feed the
emitter" is only partly true (the `Units_*` family in particular is largely
absent). Recorded as the remaining `ORPHANED_GAPS.md` §1.5 follow-up. Oracle IS
reachable (`lib.DSS_ExtractSchema`) — the blocker is Rust-side metadata, not
oracle access. Gate green (fmt/clippy/test).

**Settle round (2026-07-18).** Two read-only audits reviewed the branch. The one
Major finding (WP as literally briefed = ~90% deferred) is the honestly-disclosed
partial documented above — not a defect; the deferral is empirically justified
(only 2 of ~30 `Units_*` flags carried) and stays open in ORPHANED_GAPS §1.5. Two
Minor findings fixed: (a) `extract_schema_json()` now carries a `# Incomplete`
rustdoc header spelling out that the returned skeleton is not a usable schema
(dangling `required:["Vsource"]` + `circuitProperties` refs whose class `$defs`
are absent); (b) `skeleton_envelope_is_well_formed` gained a byte-level
top-level-member-order assertion against the Pascal envelope order
(`CAPI_Schema.pas:1504-1513`) — serde's object map ignored ordering, so an
envelope reorder previously slipped all four tests.

**Era: post-acceptance DE_PASCALIZE (PLAN_SEQUENCE stage 5).** The 1:1 port
reached FINAL ACCEPTANCE (2026-07-11, referee ACCEPT); UPGRADE Rungs 1–2 are
COMPLETE (2026-07-16/17 — engine behavior = OpenDSS 11.0.0.1 (r4133) except the
documented ledger). Active work is **DE_PASCALIZE_PLAN.md** on the `update`
integration branch: wave 1 (R0 / P1-partial / P2 / P6, all stratum [A]) merged
2026-07-17 — see the frontier block above. Remaining sequence:
DE_PASCALIZE Parts I–III + Stage F → RESONANCE → MULTITHREADING M0–M4;
Part II A-Diakoptics (WP-AD.2–AD.6) after MULTITHREADING M2. The records of the
completed plans (FINAL ACCEPTANCE, JSON, DIAKOPTICS Part I, UPGRADE Rung 1+2) are now
archived in **§1a**; their still-open carried-forward items (TODO(compat) sweep +
HIDE_015X → Stage F, GICMvars → Phase 9, JSON DynInit/Full tail, AggregateProfiles →
AD Part II, user-model DLLs → WASM, IEEE118 NCIM → a future rung) are tracked in
§Standing-open-follow-ups just below.

### WASM-UM WP-WM.5 — CapControl user control: ABI probe + the `get_public_data` asymmetry finding (branch `wasm-wm5`, 2026-07-19)

Plan §WP-WM.5 (CapControl `UserModel=`/`UserData=` over WASM, the 7-function
`CapControlInstance`). This round lands the **probe-frozen ABI foundation** plus a
**proven design finding** that redirects the fixture/golden design before the
element wiring is built. The finding was reached by the brief's binding rule
("the r4133 engine is the numeric oracle; disagree → STOP and record (probe
both)") — probing BOTH the 0.14.5 wiring spec and the r4133 oracle source.

**Correction of an earlier draft of this record (both engines re-probed):** the
earlier claim "dss_capi 0.14.5 never sets `PublicDataStruct`, so the CapUserControl
interface is inert on 0.14.5 and works only on r4133" is **wrong**. BOTH engines
set `PublicDataStruct := @ControlVars` ("So User-written models can access" —
0.14.5 `CapControl.pas:535`, r4133 `:518`). The real 0.14.5-vs-r4133 difference is
only the **record layout** (Boolean/no-`{$Z4}` — items (ii)/(iii) below).

**Landed (foundation, gate-green):**
1. **P9 FPC probe of r4133 `TCapControlVars`** — `tools/fpc/usermodel_abi/
   abi_probe_capcontrolvars.pas` compiles the REAL vendored r4133 unit
   `Version8/Source/Controls/CapControlVars.pas` (`-dUSER_DLL` variant, no engine
   closure). Evidence `docs/wasm/probes/p9_offsets_capcontrolvars_r4133.txt`; P9
   step in `build_probes.ps1`. **184 B; `EControlAction` 1 B (no `{$Z4}`);
   `Voverride` Boolean (1 B); `SampleV`@132, `FPendingChange`@111,
   `ShouldSwitch`@112, `PresentState`@114, `AvailableSteps`@152.**
2. **`crates/dss-usermodel::records::CapControlVars`** — the dss-rs
   `get_public_data` codec at the r4133 layout (the `Sample` context + bank state;
   the rest of the 184 B stays zero on the wasm side). 1-byte
   `EControlAction`/`Boolean` slots via `put_i8`/`get_i8`; round-trip test
   `cap_control_vars_offsets_match_probe`. Exported. Kept as documented dss-rs
   `get_public_data` infra (see the finding), NOT consumed by the reference
   fixture.
3. **ABI doc corrections** (`docs/wasm/USERMODEL_ABI.md` header decision (c) +
   §2.5): the true layout facts + the `get_public_data` asymmetry finding + the
   `get_node_voltages` resolution; the decision-signalling note rewritten to the
   plain-push `control_queue_push` design.

**THE FINDING — `get_public_data` is NOT oracle-gatable for CapControl (proven,
both engines):** a wasm CapControl model's natural context channel is
`get_public_data` (the model's `sample()` takes no args). On the **dss-rs** side
`get_public_data` binds to the *owning* element (plan §2.3/§4), so it reliably
returns the owning CapControl's `@ControlVars`. On the **native** (r4133) side the
`GetPublicDataPtr` callback returns `ActiveCircuit.ActiveCktElement.PublicDataStruct`
— the **global** active element — and **neither engine sets `ActiveCktElement` to
the CapControl during control sampling**: `SampleControlDevices` iterates
`DSSControls` without touching it (0.14.5 `Solution.pas`; r4133
`Solution.pas:3606`), and `CapControl.Sample` sets only
`ControlledElement.ActiveTerminalIdx` (r4133 `:909`), as do
`MonitoredElement.Power[]`/`.GetCurrents` (`CktElement.pas`, only `ActiveTerminalIdx`).
So a **native twin's `GetPublicDataPtr` does NOT return `@ControlVars`** and cannot
reproduce the dss-rs owning-element-bound payload — the same holds for every
`GetActiveElement*` tier-A read during `Sample`. (Almost certainly why no vendored
CapUserControl example exists.) A `get_public_data`-based fixture would therefore be
un-gatable ("Rust agrees with itself"), which the brief forbids. Note this
contradicts the plan §2.3 "no observable divergence vs the single-context oracle"
claim for the CapControl case — the divergence is not about context (single vs
multi) but about which *element* is active, which upstream leaves indeterminate at
`Sample` time.

**THE RESOLUTION (within settle authority — no ABI change):** the reference
`capuserctl` fixture reads its control voltage through the **symmetric**
`get_node_voltages` (`GetPtrToSystemVarray` → converged `Solution.NodeV`, ABI row
17) channel — identical on both engines after convergence — with the monitored
node index supplied via `UserData`. The guest signals its decision via
`control_queue_push(hour, sec, code, proxy_hdl)`, a **plain queue push** matching
the native twin's `CallBacks.ControlQueuePush` (→ `ControlQueue.Push` directly,
`DSSCallBackRoutines.pas:444`; ShouldSwitch/engine-arming NOT used on either side);
when the queue pops, `DoPendingAction` sets `PendingChange = Code` and the shared
switch block acts. This keeps the WM.5 gate a real r4133-native-twin oracle check.
This is a fixture-design choice (`get_node_voltages` is already a frozen ABI slot),
not a recorded-decision ABI change.

**Record-layout divergences (ii)/(iii)** — additive to the WASM path (no corpus
deck loads a `.wasm` CapControl); r4133 frozen (WM.3 re-freeze precedent):
(ii) `Voverride` r4133 `Boolean` (1 B) vs 0.14.5 `LongBool` (4 B); (iii)
`EControlAction` r4133 no `{$Z4}` (1 B) vs 0.14.5 int32. `DIVERGENCES.md` gets no
entry (feature-enabling engine-version choice on new additive code, not a
reproduced-vs-not upstream bug).

**Settled design for the remaining wiring (redirected to `get_node_voltages`):**
- **Element flip.** `UserModel`/`UserData` flip from `NOT_PORTED`
  (`cap_control/mod.rs:132-133`) to the §2.4 uniform rule (parse+store+warn/load);
  add `CapControlType::UserControl` (Pascal `USERCONTROL`); `PropertySideEffects`
  sets it when the model exists (`CapControl.pas:439-440`).
- **Sample** (`control_loop.rs` USERCONTROL arm, `CapControl.pas:1054-1069`): serve
  the guest `get_node_voltages` (converged `node_v`) + `get_dynamics_rec` (time)
  snapshot, run `sample()`, drain `Effect::ControlQueuePush` into the real control
  queue (plain push), route `Effect::Msg` into the error sink (WM.3
  no-silent-fallback). No engine-side arming for USERCONTROL. `get_public_data` may
  also be served (owning CapControlVars) for completeness, but the fixture uses
  `get_node_voltages`.
- **DoPendingAction** (`CapControl.pas:725-733`): set `pending_change = code`, call
  guest `do_pending(code, proxy)`, run the existing switch block on `pending_change`.
- **Fixture `capuserctl`** (both targets, one shared decide core, WM.4 pattern):
  deadband on `|node_v[node]|` (node from `UserData`) → `control_queue_push`. The
  native twin's `New(var CallBacks)` captures `TDSSCallBacks` and calls
  `GetPtrToSystemVarray`@128 + `ControlQueuePush`@240 + `GetDynamicsStruct`@184
  (r3723=r4133 vtable, `p2_offsets_r3723.txt`). Deck
  `tools/golden/wasm_decks/wasm_capcontrol.dss`; a load step drives a monotone
  voltage excursion comfortably clear of the deadband edges (no threshold-straddle,
  so the faer-vs-KLU floor cannot flip a decision).
- **Golden** — NEW event-log schema (ordered switch rows `**Opened**`/`**Closed**`/
  `**Step Up/Down**` + hour/sec + `Msg` lines) + final cap state + node voltages;
  `crates/dss-epri/tests/gen_wasm_usermodels_wm5.rs` (r4133 bridge + native twin,
  env-gated) → replayed by `crates/dss-core/tests/wasm_usermodels_wm5.rs`. Ordered
  parallel arrays + index compare (WM.3 schema precedent — never a name-keyed map).

**Remaining scope (NOT in this round).** The element wiring, the `capuserctl`
fixture (both targets) + wasm build + PIN, the native-twin callback FFI
(`GetPtrToSystemVarray`/`ControlQueuePush`), the event-log golden gen + hermetic
gate, and the two audits + settle. This round stops at the probe/ABI/codec
foundation + the redirected design: the earlier draft was heading toward a
`get_public_data` fixture that the finding proves is un-gatable, so locking the
correct channel first prevents building an un-oracle-able fixture. Escape protocol
honored — tree gate-green, no half-wired engine path (`CapControlVars` is
documented leaf infra like WM.1's `CapControlInstance`); `UserModel`/`UserData`
stay `NOT_PORTED` until the element flip lands.

**Settle (two independent read-only audits: audit-code + audit-tests).** Both
audits returned NO high-severity findings and confirmed the delivered
probe/codec/doc round is correct, honestly documented, and free of hidden
regression, silent fallback, or test weakening. The linchpin `get_public_data`
un-gatable finding was re-verified from the r4133 source by the settle agent
independently of both reports (`DSSCallBackRoutines.pas:384` `GetPublicDataPtr`
returns the global `ActiveCktElement.PublicDataStruct`; `Solution.pas:3606`
`SampleControlDevices` iterates `DSSControls` without setting `ActiveCktElement`;
`CapControl.pas:908` `Sample` sets only `ControlledElement.ActiveTerminalIdx`;
`:415` `ControlQueuePush` forwards straight to `ControlQueue.Push`) — CONFIRMED,
so the deferral's justification stands. Per-finding dispositions:

- **WM5-1 (medium, audit-code) / wm5-behavioral-verification-absent (medium,
  audit-tests) — plan items 1–3 (element wiring + fixture/deck/event-log golden +
  the two audits) not delivered:** NON-FIX, deliberate. This is a disclosed,
  justified settle-before-build round, not hidden under-delivery — the tree is
  gate-green with no half-wired path, `UserModel`/`UserData` stay `NOT_PORTED`,
  and building the full feature here would both exceed a settle round and violate
  the plan's own "two independent audits" ritual (a settle agent self-auditing its
  own build). The successor build round carries items 1–3 (element flip, the
  `capuserctl` fixture + wasm/native-twin build + PIN, the r4133-oracle event-log
  gate, and its fresh two-audit ritual). Recorded, no code change.
- **WM5-2 (low, audit-code) — the codec zeroes the control thresholds
  (`ON_Value`…`PTRatio`@8–80, `Vmax`@95, `Vmin`@103) yet is named for the full
  record:** FIXED (doc hardening). The partial codec is kept (precedent-consistent
  with WM.4's interacted-fields-only `TStorageVars`/`TPVSystemVars`, and the
  thresholds are un-gatable regardless), but `records.rs` now carries an explicit
  "PARTIAL CODEC — control thresholds NOT serialized" warning naming every omitted
  set-point and the exact successor step (extend `to_bytes`/`from_bytes` + the
  offset test from the P9 probe) needed before `get_public_data` is ever served to
  a real control model. Removes the latent successor trap.
- **WM5-3 (low, audit-code) — direct `control_queue_push` diverges from the native
  arm/disarm timing:** FIXED (doc hardening) + verified. The settle agent
  confirmed from Pascal that the shared `Sample` tail (`CapControl.pas:1180-1205`)
  DOES run for `USERCONTROL` (`UserModel.Sample` sets `ShouldSwitch`/
  `PendingChange`; the tail computes `TimeDelay` from `DeadTime`/`ONDelay`/
  `OFFDelay`, pushes, arms, disarms). The gate twin cannot use that path (no ABI
  write-back + the `@ControlVars` asymmetry), so it pushes directly and owns the
  timing. `USERMODEL_ABI.md` §2.5 now states this divergence explicitly and adds a
  successor caveat: the `capuserctl` deck must hold `ONDelay`/`OFFDelay`/`DeadTime`
  where the engine tail adds no delay so direct-push and any shared-tail schedule
  coincide (no action-*time* divergence), and the dss-rs `USERCONTROL` wiring must
  choose the direct-push channel deliberately, not by omission.
- **codec-offset-freeze-not-runtime-checked (low, audit-tests) — offsets
  hardcoded (not read from the probe file) and the zero-check only covers
  `[0..111)`:** PARTIAL-FIX. The offset-freeze (hardcoded offsets mirroring the
  codec) is kept — it is the established `DynamicsRec`/`GeneratorVars` precedent,
  the P9 probe baseline was re-verified field-by-field against the vendored
  `CapControlVars.pas`, and the test is fail-capable on any codec-side offset
  error. The zero-region gap IS fixed: `cap_control_vars_offsets_match_probe` now
  also asserts the two gap bytes (`Armed`@113, `InitialState`@115) and the entire
  trailing region `[160..184)` stay zero, pinning the codec to touch nothing
  outside its declared fields.

Net delivered change over the audited head: doc-only hardening in `records.rs`
(the `CapControlVars` partial-codec warning), a strengthened zero-region assertion
in the offset test, and the §2.5 timing-divergence clarification — no behavior
change, `UserModel`/`UserData` still `NOT_PORTED`, tree stays gate-green, corpus
pristine.


### WASM-UM WP-WM.5 — round 2 (build): CapControl user control over wasm (branch `wasm-wm5`, 2026-07-20)

The build round on the round-1-corrected design (base `ddaa275`). Plan §WP-WM.5
items 1–3 delivered: the CapControl `UserModel=` element flip + USERCONTROL
wiring, the `capuserctl` deadband fixture (both targets), and the cross-engine
oracle gate — plus a **new source-definitive finding** that redirects the gate
oracle (as round 1 anticipated: "the r4133 engine is the numeric oracle; disagree
→ STOP and record").

**Item 1 — element flip + USERCONTROL wiring (committed `a1d13b1`).**
- `UserModel`/`UserData` flipped from `NOT_PORTED` to the §2.4 uniform rule;
  `CapControlType::UserControl` added (Pascal `USERCONTROL`, ordinal 6 — NOT in
  the `Type=` enum, `ordinal_to_string(6)` renders empty, matching Pascal which
  comments it out of `CapControlTypeEnum`, `CapControl.pas:245-249`).
- New `elements/control/cap_control/user_model.rs`: `CapControlUserModelSlot`
  (wraps the WM.1 `CapControlInstance`, 7-fn) + the deferred-load plumbing
  (`queue/take/apply_user_model_load`). `PropertySideEffects` queues the load; the
  executive resolves it before `EndEdit`, sets `IsUserModel`, forces
  `ControlType := USERCONTROL` (`CapControl.pas:429-440`).
- `control_loop.rs`: the USERCONTROL `Sample` arm (`:1024-1041` — populate the
  SampleP/V/Curr + bank-state `CapControlVars` public-data context, serve
  `get_node_voltages` + `get_dynamics_rec`, run the guest `sample()`, drain
  `Effect::ControlQueuePush` into the real `ControlQueue.Push` with the owning
  element as owner, route `Effect::Msg` to the error sink); the USERCONTROL
  `DoPendingAction` arm (`:725-733` — set `PendingChange := code`, run guest
  `do_pending`, then the shared switch block). `ControlQueue::next_handle()` seeds
  the guest push-handle sequence. Regression test
  `user_model_native_dll_name_warns_and_falls_back` (a native-DLL name warns 570 +
  falls back, `ControlType` NOT forced, deck still solves).
- The shared `Sample` tail (`:1180-1205`) runs for USERCONTROL but is inert:
  the direct push leaves `should_switch = false`, so it neither arms nor disarms —
  the model owns the timing (WM5-3), exactly as the ABI-doc framing requires.

**Item 2 — `capuserctl` fixture (both targets).** A deadband voltage CapControl
(`tools/wasm_usermodel/models/capuserctl/`, workspace-excluded): reads
`|NodeV[node]|` via `get_node_voltages` (ABI row 17, the round-1 symmetric
channel; node index via `UserData`), schedules open/close via
`control_queue_push`. Its deadband (`vlow`/`vhigh`) is deliberately identical to
the built-in VOLTAGE control's (`OnSetting`/`OffSetting`). Committed
`tests/fixtures/wasm/capuserctl.wasm` (51014 B, `sha256=10b2c6b6…7a07`,
deterministic across a clean rebuild; PIN.txt + `fixture_pin.rs` hash test). The
native twin (`build_capuserctl_native.ps1`, `sha256=91cba238…a2d2`, NOT committed)
is built ONLY for the empirical finding below — it is NOT the gate oracle.

**THE FINDING — a native `TCapUserControl` twin CANNOT drive the r4133 control
queue (source-definitive, empirically corroborated).** Extends round 1's
`get_public_data` asymmetry to `control_queue_push`. `ControlQueue.Push` stores
its `Owner` as the `ControlElement` whose `DoPendingAction` runs on pop
(`ControlQueue.pas:145,193`), but: the 7-fn `New(var CallBacks)` gives a native
model NO owning-element pointer (`CapUserControl.pas:36`); neither
`SampleControlDevices` (r4133 `Solution.pas:3611-3621`) nor `CapControl.Sample`
(USERCONTROL arm `:1054-1069`) sets `ActiveCktElement` to the CapControl; so a
twin can only pass `Owner := GetActiveElementPtr()` (the wrong element). Empirical
confirmation (2026-07-20, `gen_wasm_usermodels_wm5.rs` with `WASM_TWIN_DLL`):
driving the native twin in r4133 **HANGS the engine** (a corrupt control-queue
pop) — decisive that the channel is un-gatable. (Almost certainly why no vendored
CapUserControl example exists — the interface's `control_queue_push` path is
non-functional for a *native* model.) The dss-rs USERCONTROL wiring is NOT
affected: the host supplies the owning CapControl as the queue owner, so its push
routes correctly (proven by the gate below).

**THE RESOLUTION (fixture/gate-design, no wire-ABI change).** The `capuserctl`
deadband is identical to the built-in VOLTAGE control, so the r4133 **built-in
VOLTAGE** control is the sound cross-engine oracle. This keeps the WM.5 gate a
real independent-engine oracle comparison (Rust wasm USERCONTROL vs r4133 built-in
VOLTAGE) — never "Rust agrees with itself". The wire ABI (7-fn shape,
`get_node_voltages`, `get_dynamics_rec`, `control_queue_push`) is UNCHANGED.

**Item 3 — decks + golden + gates.** Decks
`tools/golden/wasm_decks/wasm_capcontrol.dss` (USERCONTROL, `@FIXTURE@`) and
`wasm_capcontrol_oracle.dss` (built-in VOLTAGE, identical circuit) — a monotone
2-solve excursion clear of the deadband edges (light load → cap opens; heavy load
→ cap closes; `Delay=DelayOff=DeadTime=0` so the direct-push and shared-tail
schedules coincide). Golden `tests/golden/wasm_usermodels/wasm_capcontrol.json`
generated by `crates/dss-epri/tests/gen_wasm_usermodels_wm5.rs` from the r4133
built-in VOLTAGE oracle (`DSS_GEN_WM5=1`, manual). Hermetic replay
`crates/dss-core/tests/wasm_usermodels_wm5.rs` runs the USERCONTROL deck on the
Rust engine with the committed `.wasm` and matches the oracle:
- **switch-action sequence** `[Capacitor.c **Opened**, Capacitor.c **Closed**]`
  (event log, filtering the built-in **Armed**/**Reset** rows the direct-push
  USERCONTROL path bypasses — WM5-3, a documented mechanism divergence, NOT a
  masked bug);
- **final cap state** `[1]` (closed);
- **all 6 node voltages** at the harness `feeder` floor — **worst rel 2.5e-15**
  (faer-vs-KLU last-ulp), proving both engines solved the same switched network.

**Golden schema (WM.3 precedent honored):** ordered parallel arrays / index
compare — the switch actions are an ordered `Vec<(Element, Action)>`, never a
name-keyed map (no duplicate collapse). Event-log **Armed** filtering is a
principled mechanism-divergence account (WM5-3), not a tolerance loosening.

**Deviation from the brief (justified by the finding).** The brief's item-3 design
gated the event log against the r4133 engine driving the *native twin*. That is
impossible (the twin cannot push — the finding above), which the brief explicitly
anticipated ("run the recommended empirical r4133-bridge confirmation… or
otherwise show the event-log channel is the sound one"). The sound channel is the
r4133 built-in VOLTAGE control, adopted here. The native twin is still built and
was driven in r4133 to empirically corroborate the finding (it hangs the engine).
No `DIVERGENCES.md`/ledger entry (this is a gate-oracle choice on new additive
code, not a reproduced-vs-not upstream bug).

**Gate status.** Full three-command gate green at defaults (`cargo fmt --all
--check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test
--workspace` incl. the unconditional corpus gate 514/514); corpus pristine
(path-limited cleanup only); the five `expect_warnings` decks + manifests
untouched. Audits (audit-code + audit-tests) + settle are the ritual next step.

**Settle (round-2 audits, 2026-07-20).** Two independent read-only audits
(audit-code, audit-tests) over `ddaa275..099def5` found no High/Medium; seven LOW
findings, each reproduced then settled empirically.

- **WM5-C1 (do_pending trap non-abort, asymmetric with the sample abort) — FIXED.**
  `do_pending_action` now returns `bool` and, on a guest `do_pending`
  trap/fault, pushes `DssDiagnostic::abort(569)` and `return`s early (skipping the
  switch block) instead of `msg(569)`+continue. The dispatcher lifts it to
  `solution_abort` (`dispatch.rs` Cap `ControlOp::Action` arm), giving full ABI-§6
  parity with `sample_user_control` — a faulted guest no longer switches the bank
  on its `code` mid-run. `#[must_use]`; the 4 built-in `do_pending_action` unit
  call-sites take `let _ =` (they never trap → never abort).
- **WM5-C4 / wm5-1 (the `get_public_data` `CapControlVars` Sample-context packing
  is served but asserted by no test — un-gatable over wasm) — FIXED.** Extracted
  the host-side field assignment into `CapControl::build_sample_context` and pinned
  it with `user_control_builds_sample_context_in_pascal_units`: `SampleP` in
  kW+jkvar (×0.001), `SampleV`/`SampleCurr` PT/CT+phase-scaled, `LastStepInService
  = NumSteps − AvailableSteps`, and the state fields — the brief's "Sample context
  units" hunt item, now covered end-to-host independent of the existing codec
  offset test (`dss_usermodel::records::cap_control_vars_offsets_match_probe`,
  which already pins the 184-B r4133 byte layout at offsets 111/112/114/116/…/156).
- **wm5-2 (thin USERCONTROL scenario: only the open/close code dimension gated) —
  PARTIALLY CLOSED.** Added `user_control_do_pending_applies_code_not_preset_pending`
  (USERCONTROL `DoPendingAction` sets `PendingChange := code`, not a pre-set
  pending; the `proxy` arg is accepted) and `user_control_do_pending_multistep_steps_up`
  (multi-step step-up via the USERCONTROL `code`). The direct-push
  timing/dead-time dimension is NOT gated by design: per the ABI §2.5 successor
  caveat the model owns the timing and the gate deck keeps `Delay=DelayOff=DeadTime=0`,
  so the Rust host merely forwards the guest's `(hour, sec)` to `ControlQueue.Push`
  — there is no Rust-side timing surface to regress, and no in-gate oracle for a
  nonzero-delay USERCONTROL schedule (a native twin cannot push — the finding above).
- **WM5-C2 / wm5-3 (gen test prints "EXPECTED EMPTY" while the module doc/STATUS/ABI
  say the twin HANGS) — FIXED (doc).** Rewrote the `gen_wasm_usermodels_wm5.rs`
  twin-confirmation `eprintln`s to state the compile is EXPECTED TO HANG (OBSERVED
  2026-07-20, corrupt control-queue pop) and only falls through to an empty log if
  the engine is ever hardened — consistent with the module doc; no longer misleads
  a re-runner. Manual/env-gated path only, never in the gate.
- **WM5-C3 (`from_ordinal(6) → Some(UserControl)` widens `set_i32(TYPE,6)`) —
  VERIFIED, deliberate non-fix.** Reproduced empirically: `cap_control_type` is a
  **non-hybrid** enum with `default_value = NO_DEFAULT`
  (`registry/control.rs`), so `Type=6` in a deck goes through `string_to_ordinal`,
  fails to prefix-match any name, and (non-hybrid + NO_DEFAULT) returns an
  `Err("Could not match enum")` — `set_i32(prop::TYPE, 6)` is **never reachable
  from a deck**. The only callers of `from_ordinal(6)` are the internal
  dump/restore round-trip and typed setters — exactly what the change enables (a
  stored USERCONTROL control-type must survive a `get_i32`→`set_i32` cycle; the
  prior keep-old behavior silently corrupted it). Even a forced `set_i32(TYPE,6)`
  with no model is benign (`sample_user_control` finds no model → returns false →
  no switch). The `Type=` string enum still omits USERCONTROL (`CapControl.pas:245-249`).
  Pinned by `cap_control_type_pins_enum_ordinals` (round-trip) +
  `set_i32_type_keeps_value_on_unregistered_ordinal` (ordinal 99 keep-old).

Post-settle gate green at defaults (fmt/clippy/test, corpus gate 514/514 +
cap_control 30 unit tests incl. the 3 new); corpus pristine; goldens/manifests/
ledger untouched. Head `099def5` → settle commit.

### WASM-UM WP-WM.6 — callback tail (DoDSSCommand/GetResultStr) + callback-table sweep + SDK doc (branch `wasm-wm6`, 2026-07-20)

Plan §WP-WM.6 items 1–3. The last two re-entrant callback slots leave their
"loud unsupported until WM.6" state; the 32-slot callback table is proven
complete; the `USERMODEL_ABI.md` porting guide is finished with a worked example.
**No wire-ABI change** (the 15/13/7 shapes + every record/callback contract are
frozen-unchanged); additive only. ABI-doc header decision (e).

**Item 1 — `DoDSSCommand`/`GetResultStr` as an opt-in deferred-drain mechanism
(`crates/dss-usermodel`).** Pascal runs `DoDSSCommand` synchronously mid-call
(`DSSCallBackRoutines.pas:150-154` → `DSSExecutive.ParseCommand`) and
`GetResultStr` copies `GlobalResult` (`:449-452`). A wasmi host function sees only
`Store` data, never `&mut Dss`, so the synchronous form is impossible. Design
(the plan's immediate-drain): `do_dss_command(ptr, len)` **queues** the command
in `CallData.pending_commands`; a host that holds the executive drains it
(`UserModelInstance::drain_dss_commands`) after the guest call, runs each through
`Dss::command`, and captures `GlobalResult` back via `set_result_str`;
`get_result_str` serves that captured string (persists across `set_context`).
- **The one observable ordering difference** (documented, ABI §4, not a
  `TODO(compat)` — a new WASM-only mechanism with no upstream byte-golden): a
  `GetResultStr` in the *same* guest call as its `DoDSSCommand` sees the
  *previous* result; the next call sees the just-run one.
- **Opt-in / loud-by-default.** Running a queued command needs `&mut Dss`. Every
  dss-rs user-model call site (`DoUserModel`, CapControl `Sample`, the deferred
  load/edit resolution) holds a **disjoint** borrow and cannot reach
  `Dss::command`; inventing a re-entrant `&mut` scheme is forbidden (plan
  §WP-WM.6). So the elements leave the mechanism **disabled**
  (`CallData.dss_commands_enabled = false`) and both callbacks raise the loud
  `Fault::Unsupported`/`UserModelError::Unsupported` — never a silent drop, so no
  dss-core drain-glue change was needed and the generator files (owned by the
  parallel D2 workflow) are untouched. No reference model needs the re-entry
  (P3 census). The mechanism is complete and gated by the channel-2 protocol
  tests, which act as the re-entrant executive host; a future integration point
  that holds the executive can opt in with no wire-ABI change.
- **Not a new `Effect` variant** (deliberate): `DoDSSCommand` is plan tier C, not
  the tier-B `Effect` queue (`ControlQueuePush`/`Msg`); a separate
  `pending_commands` queue keeps the tier-B `Effect` match sites (incl. the
  off-limits generator drain) exhaustive-and-unchanged.

**Item 2 — callback-table sweep.** All 32 `TDSSCallBacks` slots are now either
implemented (tier A ×22, tier B ×2, tier C ×5, WM.6 pair ×2) or a permanent loud
attributed error (slot 30 `GetActiveElementPtr`), and **each is covered by a
channel-2 protocol test**: tier-A sweep (`tier_a_callbacks_serve_the_context_snapshot`,
all 22), tier-B (`tier_b_effects_queue_in_order…`), tier-C parser
(`tier_c_parser_callbacks…`), slot 30 + not-opted-in 7/32
(`unsupported_imports_raise_loud_attributed_errors`), and the new WM.6 tests:
`wm6_deferred_dss_command_cycle` (the full enabled queue→run→result cycle + the
ordering assertion), `wm6_deferred_commands_queue_in_order_on_cap_control`
(multi-command order + the `CapControlInstance` API), and
`wm6_cap_control_do_dss_command_loud_without_opt_in` (loud-by-default parity).
ABI §4 census note + tier legend updated ("the table is complete as of WP-WM.6").

**Item 3 — `USERMODEL_ABI.md` §8 finished.** A start-to-finish "porting your
Delphi/C user model to WASM" walkthrough narrated over the committed IndMach012a
fixture (the real, gated crate): the 6 steps (pick interface + exports → guest
`dss_alloc` → decode/encode the record images → the lifecycle → `dss_env`
services incl. the deferred pair → build/PIN/gate), a limits/trap-policy recap
(§6), and the manual PIN workflow (`build_wasm.ps1` + `fixture_pin.rs` hash test).
The code excerpts are drawn from `models/indmach012a/src/wasm_exports.rs` — the
`dss_alloc` block is verbatim (one teaching comment added); the `calc` block is
lightly abridged (the fixture's `let wrote = mainunit::calc(…); if wrote {…}`
inlined to `if mainunit::calc(…) {…}`, semantically identical) — so the worked
example is a faithful narration of a compiling, gated fixture.

**Gate.** Full three-command gate green at defaults (`cargo fmt --all --check`;
`cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`
incl. the unconditional corpus gate); corpus pristine (path-limited cleanup
only); goldens/manifests/ledger + the five `expect_warnings` decks untouched.
`dss-usermodel` protocol suite 26 tests (23 prior + 3 new WM.6). Audits
(audit-code + audit-tests) + settle are the ritual next step.

**Follow-up.** If a future host gains executive access at a user-model call site
(or a hardened re-entrancy design lands), it can `enable_dss_commands()` to make
`DoDSSCommand` run end-to-end in production without touching the wire ABI. The
generator's user-model drain (D2-owned) needs no change for WM.6 (the pair stays
loud there too, since the generator does not opt in).

**Settle (two independent read-only audits — audit-code + audit-tests, both
opus-high+).** Both audits certified the change sound: no regression, no wire-ABI
change, no tolerance touched, corpus/goldens/manifests/ledger pristine, all 32
callback slots covered. Every finding was low-severity awareness/accuracy;
settled empirically:

- **WM6-1 (code + tests) — item-1 mechanism dormant in production; no call site
  opts in; end-to-end command→GlobalResult unverified.** *Reproduced:* `grep`
  confirms zero `enable_dss_commands`/`drain_dss_commands`/`set_result_str`
  callers outside `crates/dss-usermodel`. *Disposition: deliberate non-fix
  (plan-sanctioned).* A genuine production opt-in is architecturally impossible
  here — the dss-rs element call sites hold a disjoint circuit borrow and cannot
  reach `Dss::command`, and the plan §WP-WM.6 forbids inventing a re-entrant
  `&mut` scheme; a true end-to-end test cannot live in `dss-usermodel` either (a
  leaf crate that must not depend on `dss-core`, plan §2.1). The mechanism is
  complete and gated by the channel-2 protocol tests acting as the re-entrant
  host; it is a zero-regression change (the loud-default path real decks hit is
  unchanged from pre-WM.6, covered by `unsupported_imports_raise_loud_attributed_errors`).
  This is the honest state already documented (ABI §4 opt-in subsection, the
  Follow-up above).
- **WM6-2 (code) — the drain doc omits Pascal's `SolutionAbort := FALSE` reset a
  faithful future host must reproduce; "only difference is ordering" is
  incomplete.** *Reproduced:* `DSSCallBackRoutines.pas:152-153` does
  `DSSPrime.SolutionAbort := FALSE;` then `ParseCommand`. *Disposition: fixed
  (doc).* ABI §4 drain design + `instance.rs::drain_dss_commands` doc now state
  the host must clear `SolutionAbort` before each `ParseCommand`, called out as
  the second `Dss`-side semantic beyond the ordering note.
- **WM6-3 (code) — STATUS called the §8 excerpts "verbatim"; the `calc` excerpt
  is inlined.** *Reproduced:* fixture `wasm_exports.rs` uses
  `let wrote = mainunit::calc(…); if wrote {…}`; the doc uses `if mainunit::calc(…) {…}`
  (the `dss_alloc` excerpt IS verbatim). *Disposition: fixed (wording).* The
  STATUS "verbatim" sentence now distinguishes the verbatim `dss_alloc` block
  from the lightly-abridged `calc` block.
- **WM6-4 (code) — GetResultStr is tier A in plan §2.3 but tier C in ABI row
  32, unreconciled.** *Reproduced:* plan §2.3 line 310 lists it under tier A;
  ABI row 32 marks it tier C. *Disposition: fixed (reconciled).* The
  reclassification is correct (GlobalResult is produced by DoDSSCommand, not a
  pre-call snapshot; recorded decision (e)). Added a reconciliation note to the
  ABI §4 tier legend and a forward pointer in plan §2.3 so a reader from either
  side sees the mapping.
- **WM6-2 (tests) — the CapControl `get_result_str` serve path was smoke-only
  ("does not crash"), the only serve value-assertion living on the Generator
  instance.** *Reproduced:* `wm6_deferred_commands_queue_in_order_on_cap_control`
  read the served result but asserted nothing. *Disposition: fixed (coverage).*
  The guest `do_pending` now checksums the NUL-terminated served bytes and traps
  on mismatch (incl. an empty serve), so a passing `do_pending` is a real value
  assertion; a negative-control `set_result_str("9 9 9")` proves the guest
  checksum is load-bearing (expects a `Trap`).

Post-settle gate green at defaults; `dss-usermodel` protocol suite still 26
tests (the CapControl test gained an in-test negative control, no new test fn).

### WASM-UM WP-WM.7 — exit sweep (branch `wasm-wm7`, 2026-07-25)

Plan §WP-WM.7 items 1–4 (item 5 = merge is the coordinator's). Docs + marker
cleanup only — **no engine behavior change, no wire-ABI change, no tolerance
touched, corpus decks/manifests/ledger pristine**. Closes the WASM_USERMODELS
plan; `PLAN_SEQUENCE.md` stage 9 marked COMPLETE.

**Item 1 — marker sweep clean.** `rg NOT_PORTED` over the six user-model
property surfaces (Generator UserModel/UserData/ShaftModel/ShaftData, Storage
UserModel/UserData/DynaDLL/DynaData, PVSystem UserModel/UserData, CapControl
UserModel/UserData) → **zero live `NOT_PORTED` flags** (none of the four
elements' `PropDef`s carry `PropFlags::NOT_PORTED`; all six resolve via the §2.4
uniform rule). Seven **stale/misleading doc comments** that still claimed these
were NOT_PORTED / never-loaded / out-of-scope were corrected to reflect the WASM
port (five in the initial sweep; two more found by the settle audit — see the
settle note below):
- `generator/dynamics.rs:9` module doc ("UserModel/ShaftModel DLLs are
  NOT_PORTED (never)") → now describes the WASM ABI port (§WP-WM.3).
- `generator/dynamics.rs` `get_gen_variables` doc ("UserModel/ShaftModel
  variables are NOT_PORTED") → user/shaft model vars are appended by
  `get_all_variables` from the loaded WASM slots.
- `generator/accessors.rs` `set_string` comment ("NOT_PORTED string props error
  in the parser before reaching here") → the props store here + queue the
  deferred WASM (re)load; never a parse error.
- `inv_based_pce.rs` `user_model_name`/`user_model_edit` field docs ("NOT_PORTED
  in safe Rust") → the shared PVSystem/Storage base fields that drive
  `queue_user_model_load`/`_edit` (§WP-WM.4).
- `exec/tests/dynamics.rs` `sto_dyn_dss` doc ("user models are NOT_PORTED") →
  "this deck loads no user model, so the built-in dynamics integrate".
- `storage/mod.rs` `DynaDLL`/`DynaData` PropDef comment ("User-written model DLLs
  are never *loaded* … loader permanently out of scope … falls back to the
  built-in model") — false post-WM.4: `DYNA_DLL` now queues a real WASM load
  (`accessors.rs:692` → `queue_user_model_load(UserModelSlot::Dyna, …)`). Rewritten
  to the §2.4 uniform rule (a `.wasm` loads; native-DLL name / missing file warns
  1570 and falls back). *(settle-audit miss)*
- `generator/mod.rs:9` module doc ("dynamics/harmonics/user-model DLLs are Phase 7
  / never") — dynamics and harmonics are ported and the user-model surface is
  WASM-ported; rewritten to point at [`dynamics`] and §WP-WM.3. *(settle-audit miss)*

Two remaining `NOT_PORTED` hits confirmed **unrelated** and left untouched:
`generator/dynamics.rs:233` (the DebugTrace CSV record — a genuinely-different
unported detail) and `storage/tests.rs:525` (a test doc that *accurately*
describes `DynaDLL=` as "no longer a hard NOT_PORTED error"). `rg "TODO(WM)"`
across `crates/ tools/ docs/` → **zero**. Every callback slot is accounted for
in `docs/wasm/USERMODEL_ABI.md` §4 (the 32-slot table, complete as of WM.6).

**Item 2 — hermetic gate + fixture pins, all green.** Full three-command gate at
defaults: `cargo fmt --all --check` ✅; `cargo clippy --workspace --all-targets
-- -D warnings` ✅; `cargo test --workspace` ✅ — **1961 passed / 0 failed / 5
ignored** (the 5 ignored = the `gen_wasm_usermodels{,_wm4,_wm5}` manual golden
generators + 2 others, `#[ignore]` by design), exit 0, wall ~178s on a warm
build. The unconditional corpus gate ran inside it:
`corpus_gate_all_cases_match_engines ... ok` (514 cases, capi_v0145 + r4133
channels), corpus_gate binary 25 tests / 153.48s. WASM-specific suites green:
channel-1 goldens `wasm_usermodels` 17, `wasm_usermodels_wm4` 15,
`wasm_usermodels_wm5` 12; `dss-usermodel` unit 4 + channel-2 protocol suite 26;
hash-vs-PIN `fixture_pin` 4 + `fixture_self_gate` 2.
- **Five `expect_warnings` user-model decks — byte-identical, zero manifest
  edits.** A filtered `DSS_GATE_ONLY` run (5/514 kept) with `DSS_GATE_DUMP`
  verdicts, all `ok`:
  - `solvable_now:Test/indmachtest/Master.DSS` -> ok
  - `solvable_now:Version8/Distrib/IEEETestCases/4wire-Delta/Kersting4wire_Lagging.dss` -> ok
  - `solvable_now:Version8/Distrib/IEEETestCases/4wire-Delta/Kersting4wire_Leading.dss` -> ok
  - `solvable_now:Test/SimpleStorageTest.dss` -> ok
  - `solvable_now:Test/SimpleStorageTest-1ph.dss` -> ok
  `git status tests/corpus` clean after every live run (path-limited
  `git clean -fd tests/corpus` of the export-CWD-corner output artifacts; no
  tracked deck/manifest touched).
- **Workspace-excluded fixture crates re-verified (audit WM-T1).** `indmach012a`:
  fmt ✅, clippy ✅, `twin_parity` **2/2** (`record_codec_round_trip`,
  `twin_parity_scenario`). `wm4model`: fmt ✅, clippy ✅ (no test suite).
  `capuserctl`: fmt found cosmetic drift (comment alignment in a `#[cfg(test)]`
  block + long-line wrapping of `transmute` calls in the native twin) →
  **`cargo fmt` applied** (`src/lib.rs`, `src/native_exports.rs`), source-only
  and semantically inert, so the committed `.wasm`/PIN are unaffected; clippy ✅.

**Item 3 — classification unchanged.** `DSS_LIVE_CLASSIFY=1 corpus_live_classify`
probed all `skipped_needs_investigation` candidates against the oracle:
**0 solvable, 13 failed of 13** — no deck reclassifies (the `solvable` set is
identical to the prior baseline: both empty). The one WASM-relevant candidate,
`Kersting4wireIndMotor.dss`, stays skipped with the expected
`#570 Generator User Model IndMach012a Not Loaded` (a native-DLL name — no
vendored deck ships a `.wasm`, so none activates the WASM path). **Coverage
note recorded here in STATUS, not in `COVERAGE.md`:** `tests/corpus/COVERAGE.md`
is auto-generated (`tools/corpus/coverage_report.py`, banner "Do not edit by
hand"), tracks vendored-corpus per-manifest `.dss` counts (not WASM-UM fixture
coverage), and lives in the keep-pristine corpus tree — so the WASM-UM coverage
line belongs here. **WASM-UM coverage:** all six properties × four elements live
end-to-end over the WASM ABI; channel-1 numeric goldens (44 replays across
WM.3/WM.4/WM.5) vs the native twin loaded in the pinned oracle; channel-2
protocol suite (26) exercising all 32 callback slots; hash-vs-PIN (6) pinning
the committed `.wasm` fixtures; three reference fixture crates (indmach012a
Generator, wm4model Storage/PVSystem, capuserctl CapControl).

**Item 4 — docs.** This STATUS record (item 4a); `PORTING_PLAN.md` Phase-6
"user-model DLL loading stubbed" line annotated with a bracketed pointer to
`WASM_USERMODELS_PLAN.md` (historical text unchanged, item 4b); `PLAN_SEQUENCE.md`
stage 9 marked COMPLETE (item 4c); `docs/wasm/USERMODEL_ABI.md` verified complete
— the 32-slot table + §8 worked porting example (finished in WM.6) present (4d).

**Stuck items: none.** Every finding was a mechanical doc/format cleanup; the
tree is left green. Merge (item 5) is the coordinator's per the standing
convention.

**Settle (audit dispositions, 2026-07-25).** Two independent audits reviewed the
sweep. Audit-tests: nothing real — corpus byte-identity, hash-vs-PIN pins, and
the marker sweep all confirmed clean; no coverage removed, no assertion loosened.
Audit-code: verdict inert-and-correct, but flagged that item-1 missed **two**
same-meaning stale claims the literal `rg NOT_PORTED` grep could not catch —
`storage/mod.rs` `DynaDLL`/`DynaData` ("never loaded / out of scope", the more
material since it misdescribes live property behavior) and `generator/mod.rs:9`
("Phase 7 / never"). Both **REAL doc-only misses, now fixed** (see the two
*(settle-audit miss)* entries above); doc-only, no runtime effect. A broader
grep (`never.*load|out of scope|property SURFACE|Phase 7 / never`) across
`src/elements` confirmed no other user-model stale claims survive — remaining
hits are legitimate module-doc "property surface" descriptions or correct
"native DLL loading out of scope" statements. `cap_control/mod.rs:2` ("Phase 4
ports the parse-time surface only") is a phased-porting-history note (lines 11–16
correctly point to `control_loop` for the ported solve-time machinery), not a
user-model-DLL claim — out of WM.7's chartered six-property scope, left as-is.
Full gate re-run after the two fixes: fmt ✅, clippy ✅, `cargo test --workspace`
✅ (corpus gate 25/25, 224s), `tests/corpus` pristine.

### D2 sub-bug #2 — FIXED (round 2): the dynamics-entry FInit seed used the wrong Vterminal (branch `d2-subbug2-r2`)

The round-1 trace (landed on `update`: ee238e3/3c7503f/71a5be4) proved the ~5e-4
`wasm_gen_dyn` divergence is a Rust PORT bug in the Generator Model=6 dynamics
*engine flow* (guest bit-exact on the divergent path) but did NOT pin the line.
Round 2 pinned and fixed it.

**First-divergence trace (single-instance deck, h=1e-9 to isolate the algebraic
projection from the h-scaled integrator).** Instrumented the Rust host
(`do_dynamic_mode`/`init_state_vars`) to mirror the guest's own `IndMach012_Trace`
columns, and — decisively — rebuilt the 244-B FPC twin from a **scratch copy** of
the r3723 `IndMach012Model.pas` (`.inputs` untouched, 186 `.pas`) with a one-line
dump in `Init`, driven through the pinned 0.14.5 engine
(`tools/wasm_usermodel/d2_finit_probe.py`).

- Snapshot A==B==C bit-identical (|Is1|=189.1008, slip −6.18230e-3, E1=(7252.814,
  1530.375)). Step 1 (h=1e-9): oracle |Is1|=**189.20691** (h-independent — same at
  1e-9/1e-7); Rust |Is1|=**189.10206** (stuck at the power-flow point). The offset
  is entirely in the guest flux `E1`: oracle E1=(7252.279,1531.159) |E1|=7412.152
  vs Rust E1=(7252.814,1530.375) |E1|=7412.513.
- **The oracle's `FInit` receives V012[1]=(7953.090, 89.425)** (|V1|=7953.593) with
  the pflow current I012[1]=189.10 → E1=7412.152. That V is NOT the converged node
  voltage (7953.625, 88.641 → |V1|=7954.119). The current is the SAME on both. So
  it is the **voltage** fed to `FInit`, not the current, that differs.
- Rust's STALE `Vterminal` buffer (as left by the power-flow solve, before the
  erroneous refresh) is (7953.090, 89.425) → E1=7412.152 — **bit-identical (6+
  sig figs) to the oracle's `FInit` input**. Rust's refreshed `Vterminal` (the bug)
  is the converged (7953.625, 88.641) → E1=7412.513 (power-flow point).

**Root cause + fix (Pascal-cited).** Pascal `TGeneratorObj.InitStateVars`
(`generator.pas:2393-2449`) runs only `ComputeIterminal` — **never
`ComputeVterminal`** — before `UserModel.FInit(Vterminal, Iterminal)`, so the model
is seeded from the STALE `Vterminal` buffer = the node voltage of the power-flow's
**last injection iteration** (`V_{n-1}`, one network re-solve behind the converged
`NodeV`). That pre-final voltage IS the oracle's h-independent "projection" onto the
dynamic operating point: `E1 = V_{n-1} − I·Zsp` differs from `V_n − I·Zsp` by the
last-iteration voltage step, which drives the machine to 189.207 (not 189.10) at
step 1. The port's `user_model_finit` called `self.cd.compute_vterminal(node_v)`,
refreshing `Vterminal` to `V_n` and seeding the power-flow point. **Fix: drop that
refresh** (`generator/user_model.rs::user_model_finit`); `Vterminal` is left exactly
as `init_state_vars`' `ComputeIterminal` left it (stale on a cache hit, model-fresh
on a cache miss — both matching Pascal). The built-in-shaft `Edp` path
(`generator.pas:2409-2413`) reads a *fresh local* `Vabc`, so only the Model=6
user-model seed is affected — zero corpus impact (no corpus deck uses a wasm model).

**Post-fix three-way (real dual-instance `wasm_gen_dyn`, 21 dynamics steps).**
| quantity | before (Rust, round 1) | after (Rust) | r4133 | 0.14.5 | after-gap |
| --- | --- | --- | --- | --- | --- |
| Is1 (user) | ~189.94 (~5e-4 off) | 190.031383684055 | 190.031383684 | 190.031383684 | ≤1e-13 |
| Ir1 (user) | ~5e-4 off | 183.339580104762 | 183.339580105 | 183.339580105 | ≤1e-13 |
| StatorLoss | ~1e-3 off | 21869.353971380 | 21869.3539714 | 21869.3539714 | ≤1e-13 |
| RotorLoss | ~1e-3 off | 26885.612069344 | 26885.6120693 | 26885.6120693 | ≤1e-13 |
| Slip | ~1e-4 off | −6.983873874e-3 | −6.983873874e-3 | −6.983873874e-3 | ≤1e-13 |
| dSpeed | ~3e-2 off | −122.550595947 | −122.550595947 | −122.550595947 | ~1e-13 |

`r4133 == 0.14.5` to ≤1.06e-13 (round-1 B==C re-confirmed); **Rust now == both** to
the faer-vs-KLU floor. Only the near-zero quadrature currents Is2/Ir2 (~4.3e-7 value)
sit at ~7e-7 REL / ~3e-13 abs — the documented cancellation floor, bounded by
`var_abs=1e-12`. Step-1 per-step (both channels bit-identical): Slip −6.99966e-3,
dSpeed −89.2447, Is1u 189.2276, Is1s 189.2274 — Rust matches all to ≤4e-12 (the
dSpeed near-cancellation floor).

**Gate design change (numeric flip + step-1 guard — TIGHTENS, no fudge).**
- `wasm_usermodels.rs`: `gate_deck("wasm_gen_dyn", …)` flipped `false → true`. The
  full 34-variable surface + all node voltages now floor-compare vs the (unchanged)
  r4133 golden at the SAME floors as the pflow decks (`var_rel=1e-8`, `var_abs=1e-12`,
  `feeder` voltages) — no golden regen (the fix moves Rust to the already-correct
  r4133 golden), no tolerance touched.
- New `wasm_gen_dyn_step1_trajectory_matches_oracle`: pins the FIRST dynamics step
  (Slip/dSpeed/Is1(user)/Is1(shaft)) to the oracle values measured on BOTH channels
  (bit-identical). This is the targeted regression guard for BOTH sub-bugs at their
  point of first appearance — sub-bug #2 (step-1 Is1 189.10→189.207) and the
  deferred **sub-bug #1** dSpeed guard (step-1 dSpeed −89.24; a reverted shaft
  `FCalc` write-back drifts it to −80.1 AND fails 18 end-state quantities — verified).
  This closes the round-1 audit-tests D2-1 deferral.

DIVERGENCES.md untouched (a Rust port bug, not a version divergence). New reusable
probe committed: `tools/wasm_usermodel/d2_finit_probe.py`. Full three-command gate
green; `tests/corpus` pristine (path-limited cleanup only); 186 `.pas` under
`.inputs/dss_capi`.

**Closes the open follow-ups.** The round-1 "Open follow-up (carry forward): D2
sub-bug #2 is a PROVEN, OPEN engine-flow port bug…" and the WM.3 D2 "Open follow-up"
paragraphs above are now RESOLVED: the exact host line is pinned
(`user_model_finit`'s `compute_vterminal` refresh), the fix reproduces the oracle's
first-step operating point (|Is1| 189.10→189.207, h-independent) and the whole
trajectory, and `wasm_gen_dyn` is a full numeric gate.

**Settle (two independent read-only audits — audit-code + audit-tests, range
`16f12e0..caa14e4`).** audit-code returned CLEAN (no findings): the fix is
Pascal-faithful (verified against vendored `generator.pas:2356-2481` — `InitStateVars`
runs `ComputeIterminal` only, never `ComputeVterminal`, before `UserModel.FInit`), the
stale-buffer premise is structural (power-flow's last `compute_vterminal` write leaves
`V_{n-1}`; the `Set mode=dynamics` handler does not solve, so `compute_iterminal` is a
cache hit and does not refresh), the change is contained to the Model=6 user-model seed
(no leak to non-Model=6 / PVSystem / Storage / IndMach012), and the numeric flip
tightens without loosening any tolerance. audit-tests returned two LOW findings, both
explicitly flagged by the auditor as traceability notes, not real holes; both settled:

- **D2R2-T1 (FIXED)** — the step-1 guard read its 4 pinned variables by hardcoded
  positional index (`v[6]/v[4]/v[13]/v[27]`) without re-asserting the variable name at
  each index inside the guard. Mitigated already (the sibling `gate_deck` name-order
  gate + the count=34 pin catch a surface reorder in the same binary), but made the
  guard self-defending: it now fetches `element_variable_names("Generator.g1")` and
  asserts `names[idx]` equals the expected name (`Slip`/`dSpeed (Deg/sec)`/`Is1`/`Is1`)
  before each numeric check, so a 34-var surface-order regression fails the guard on its
  own terms rather than silently pinning the wrong quantity. Verified: guard still
  passes; names at the pinned indices match the golden surface.
- **D2R2-T2 (deliberate non-fix, rationale)** — the 4 step-1 oracle constants
  (`ORACLE_SLIP/DSPEED/IS1_USER/IS1_SHAFT`) are inline `const` literals, not a
  regenerable golden artifact. Kept as-is by design: the values ARE oracle-sourced — the
  committed driver `tools/wasm_usermodel/d2_step_0145.py` reads the pinned-0.14.5 oracle
  + 244-B twin via `ActiveCktElement.AllVariableValues` at the identical indices
  (`v[6]/v[4]/v[13]/v[27]`), so provenance is committed and re-runnable, not a Rust
  self-capture — and the end-state numeric gate independently verifies Rust == r4133
  golden at step 21. A separate regenerable JSON artifact for 4 targeted regression-pin
  values would be disproportionate; the provenance comment citing the committed driver is
  the correct weight for this pin. No change.

**Final re-audit (settle round 2, range `update..d2-subbug2-r2` — covers the settle
commit + `update` merge).** Two further independent xhigh audits (audit-code +
audit-tests) re-reviewed the whole branch and returned NO new real findings. The fix
is Pascal-faithful and load-bearing — re-proven here by a deterministic counterfactual:
reintroducing the dropped `compute_vterminal(node_v)` in `user_model_finit` fails BOTH
dyn tests (step-1 `Is1` rel 5.54e-4 = 189.12 vs oracle 189.227; end-state 32 quantities
out of tolerance, worst `dSpeed` rel 3.4e-2), exactly the documented regression
signature; removed again, both pass. No tolerance loosened;
golden/corpus/fixtures/PIN/`.wasm`/ledger untouched (net-changed set = the same 5
files). Two non-code observations, both dispositioned:
- **Environment hygiene (audit-code, `[Minor]` — NOT a defect in the change).** The
  prior audit session left the worktree dirty with a reintroduced-regression probe line
  that a parallel process then rolled back; the committed HEAD is and was clean. Verified
  at this settle: `git status` clean + stable across the session, and the committed
  `user_model_finit` carries no `compute_vterminal`. No effect on the change; nothing to
  fix in-tree.
- **Second dynamics-user-model deck (both audits, enhancement — declined with
  rationale).** Generality is already structural: the stale `Vterminal = V_{n-1}` premise
  is a property of ANY converged power-flow (the last-injection voltage the solve leaves
  in the buffer), not specific to `wasm_gen_dyn`, and the single deck's counterfactual
  bites deterministically. A second deck (different topology / phase count) would require
  a fresh r4133 golden and likely a new `.wasm` fixture (frozen this pass by the binding
  rule) — a coverage-strengthening WP, not a settle-scope fix. Recorded, not done.

**Post-merge convention fix (user-flagged, 2026-07-25).** The stale-`Vterminal`
FInit seed is a *deliberate reproduction of an upstream inconsistency* (fresh
`Iterminal` at converged `V_n` paired with stale `Vterminal` = `V_{n-1}`, while
upstream's own built-in model seeds from fresh `NodeV`) — the PORTING_PLAN §4.1
convention requires such a site to carry a greppable `TODO(compat)` marker, which
the fix's doc comment lacked. Added at `generator/user_model.rs::user_model_finit`
(explanation + intended clean fix: a self-consistent `(V_n, I(V_n))` seed, below
the ~1e-4 pu convergence tolerance in effect, breaks bit-parity with both oracle
channels → Stage F default-lane candidate; parity lane keeps the stale seed).
Comment-only; gate re-run green.
