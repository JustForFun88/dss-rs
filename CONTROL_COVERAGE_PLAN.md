# Control / Protection / Metering live-coverage plan (controls live gate)

Extends the live oracle-comparison gate (`crates/dss-core/tests/corpus_live.rs`,
CORPUS_TEST_PLAN.md) to every **control, protection, and metering** element
class, with **element-specific state** compared in addition to the full model.
Companion of the asymmetric stamping gate (`tests/corpus/asymmetric/`); decks
live in **`tests/corpus/controls/`** and run through
`controls_cases_match_oracle` + the oracle-free structural guard
`controls_manifest_is_complete`.

## Comparison mandate

Every case compares, per step, against the pinned dss-python oracle
(tools/golden/PIN.txt):

**Full model (mandatory, `run_and_compare`):**
1. all node voltages, iteration count, convergence, node order;
2. the full assembled system `Y` (+ fingerprint) and the injection vector;
3. every element's currents, powers, **and losses** (`CktElement.Losses` — the
   engine's own `Get_Losses` path, compared as its own channel with the
   accumulated per-conductor power tolerance);
4. `YPrim` blocks by name — `selected_elements: ["*"]` expands to **every**
   element (small decks always use it);
5. topology / control state after solve: transformer taps, RegControl tap
   numbers, capacitor step states (`compare_discrete`) + the channels below.

**Element-specific state (opt-in per case in `manifest.json`):**
- `probes: [{element, props}]` — property-value probes: oracle
  `Properties(p).Val` vs the Rust `?` query (`do_query_cmd` →
  `ClassProps::get_value`). Numbers compare by value at the case tolerance,
  text case-insensitively (numeric-skeleton, `assert_value_matches_tol`).
  Carries protection `State`/`Normal` (per-phase array for Fuse), settings /
  pickups / curves, `kWhstored`, dispatched kW, …
- `compare_variables: [elements]` — PC-element state variables
  (`AllVariableNames/Values` vs `Dss::element_variables`) — the live f64 DER /
  machine state (CLAUDE.md: the f32 monitor channel hides it).
- `compare_eventlog: true` — the cumulative event log line-for-line per step
  (`Solution.EventLog` vs `Dss::event_log()`; numeric skeleton at 1e-6 — the
  `golden_phase7_protection.rs` policy). Pins **when** every control action
  (trip, reclose, lockout, tap change, cap step) happened.
- `compare_ctrlqueue: true` — pending control actions (`CtrlQueue.Queue` vs
  `Dss::control_queue_rows()`): meaningful in time/dynamics modes where
  future-scheduled actions (reclose shots, delayed switches) survive the solve.
- `check_meters_monitors: true` — EnergyMeter registers (incl. losses /
  overload / max-demand) + zone membership, Monitor headers / sample counts /
  channel arrays (existing comparators).

Tolerances: the existing `micro`/`feeder`/`large` classes (`tol_for`) — never
loosened (CLAUDE.md). Buses left floating by pure L-L connectivity are pinned
physically (small wye capacitors), as proven in the asymmetric gate.

## Per-class scenario & check matrix

| Class | Scenarios (deck) | Element-specific checks |
|---|---|---|
| RegControl | `regcontrol_sym` (LTC, daily 24h ramp), `regcontrol_asym` (3×1φ bank, unequal loads) | discrete taps/TapNumber; probes vreg/band/ptratio/TapNum; eventlog tap changes; ctrlqueue pending |
| CapControl | `capcontrol_sym` (voltage mode), `capcontrol_asym` (kvar+current mode, 1φ CT) | discrete step states; probes mode/ON-OFF/CT-PT; eventlog switching |
| SwtControl | `swtcontrol_time` (delayed open→close, time mode) | probes State/Normal/Action/Lock; eventlog; ctrlqueue |
| Relay | `relay_oc_sym` (51 on 3φ fault), `relay_4647_asym` (46/47 — asymmetry-only trips) | probes State/Normal + settings; eventlog trip/lockout |
| Fuse | `fuse_blow_asym` (SLG blows one phase) | probes per-phase State array; eventlog PHASE n BLOWN |
| Recloser | `recloser_temp` (successful reclose), `recloser_perm` (lockout) | probes State/Normal/NumFast/Shots; eventlog shot sequence; ctrlqueue pending shots |
| EnergyMeter | `energymeter_sym`, `energymeter_asym` (daily, zones) | registers (all) + zone membership |
| Monitor | `monitor_modes` (modes 0/1/2/3, daily) | headers/sample counts/channels; dbl_hour per step |
| Sensor | `sensor_map` (wye/delta, terminals) | probes kVBase/kWS/kvarS/%error/conn/terminal mapping |
| InvControl | `invcontrol_vv_sym`, `invcontrol_vvvw_asym` (irradiance ramp) | DER powers (full model) + PVSystem variables; probes mode/curves; internals stay under WP7.5 exec tests |
| StorageController | `storagectrl_peakshave`, `storagectrl_time` | probes Storage State/kWhstored per step + controller settings; eventlog |
| GenDispatcher | `gendispatcher` (peak-load dispatch) | probes generator kW/kvar per step |
| UPFCControl | covered by `tests/corpus/asymmetric/upfc_asym.dss` | — |
| ESPVLControl | exec-test coverage only (`exec/tests/espvl_control.rs`); no corpus cases | — |
| Combinations | `combo_protection` (relay+recloser+fuse coordination), `combo_voltvar_asym` (Reg+Cap+InvControl), `combo_metering` (meter+monitors+sensor) | union of the above |

## Steps (each ends gate-green, STATUS.md synced, committed)

1. **DONE** — Infrastructure + `regcontrol_sym.dss`: capture channels (probes /
   variables / losses / eventlog / ctrlqueue / `"*"` YPrims) in
   `oracle_server.py`, comparators in `tests/harness/mod.rs`, manifest fields +
   runner + guard in `corpus_live.rs`, `Dss::control_queue_rows`; this doc.
2. **DONE** — Volt/var + dispatch decks (regcontrol_asym, capcontrol sym/asym,
   invcontrol VV / VV_VW, storagectrl peakshave/time, gendispatcher). Found and
   fixed a real port bug: `update_storage`'s end-of-step state flip dropped
   Pascal `Set_YprimInvalid`'s `SystemYChanged` side effect (stale YPrim on the
   next step's first injection; see STATUS §1f).
3. **DONE** — Protection decks (recloser temp/perm, relay 51 + 46/47, per-phase
   fuse blow, delayed SwtControl via `post`), duty mode / controlmode=time.
   Oracle capture caveat: on a step whose solve rebuilt Y mid-step (fault
   applying, trip opening a switch) the pinned engine's `getYSparse(False)`
   returns None — `_get_y_sparse` retries after the executive `BuildY`
   (trajectory-neutral, proven); `getYSparse(True)` must NOT be used (it
   corrupts the solution vector).
4. **DONE** — Metering decks (energymeter sym/asym incl. overload registers +
   zone membership, monitor modes 0/1/2/3, sensor mapping probes).
5. **DONE** — Combination decks (`combo_protection` fuse-save coordination +
   meter/monitor, `combo_voltvar_asym` LTC+kvar-CapControl+InvControl interplay
   — a voltage-mode CapControl under an LTC never toggles, hence kvar mode —
   `combo_metering`) + `compare_eventlog` opt-in on the three daily IEEE
   feeders in `solvable_now` (IEEE13/37/123).

22 decks total; `CONTROLS_REQUIRED` in `corpus_live.rs` pins the full set — the
guard fails if a landed deck is ever dropped.
