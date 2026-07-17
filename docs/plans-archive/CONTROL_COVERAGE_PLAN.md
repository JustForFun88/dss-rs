# Control / Protection / Metering live-coverage plan (controls live gate)

## Source-integrity gate — ritual step 0 (before the model-tier check)

The Pascal we port FROM — `.inputs/dss_capi` (186 `.pas` files), plus
`.inputs/electricdss-tst` for oracle/live work — is the **specification**. Before doing
anything, and re-checked continuously (not only at kickoff), confirm that folder exists
and is non-empty. If it has vanished — missing or empty — at **any** point in the work,
**STOP immediately**: make no edits, run no gate, and do **not** reconstruct, guess, or
"port" a source you cannot read. Tell the user the vendored source is gone and must be
re-vendored, then wait. Reply exactly:
**«Исходник порта (`.inputs/dss_capi`) отсутствует или пуст — работа остановлена. Восстанови
vendored-исходник (re-vendor) и повтори команду.»**
No spec → nothing to port; fabricating one from memory is a silent, unverifiable
divergence — far worse than stopping. This gate runs **ahead of the tier/refuse check**
(`PLAN_SEQUENCE.md` §Model-tier protocol).

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

## Midi network (IEEE123-class scale)

Micro decks cannot exercise large-network failure modes; the vendored
IEEE13/37/123/8500 corpus runs exercise the scale but not the asymmetric
element configs or the element-state channels. The **midi network**
(`tools/decks/gen_midi_decks.py` — a deterministic generator; the decks are
committed artifacts) closes that gap at the MINIMAL scale that reliably
reproduces large-net traits: ~94 nodes, a 14-segment backbone (deep-chain
drop), a loop + a parallel segment (duplicate-stamp density), 3 voltage
levels, mixed 3/2/1-phase laterals, multi-digit node ordering, and — the trait
that actually caught a bug — MANY controls acting in the same control rounds.

- `tests/corpus/asymmetric/midi_asym.dss` — the asymmetric configs at scale
  (Z1≠Z2 source + series reactor, full-asym matrices, unequal bank taps, delta
  tertiary w/ pin, IndMach012), micro tolerance.
- `tests/corpus/controls/midi_controls.dss` — cascaded regulators (LTC + 3×1φ
  bank) + 2 kvar CapControls + volt-var InvControl over 2 PVs + peakshave/time
  StorageController + meter/monitors/sensor, daily 24 h. Field lessons baked
  into the deck: a kvar CapControl's ON/OFF deadband must exceed its own bank
  size (else it hunts — hour-6 open/close cycle to MaxControlIter), and a
  voltage-mode CapControl under regulators never toggles.
- `tests/corpus/controls/midi_protection.dss` — relay → recloser → fuse
  coordination with the SLG fault at the END of the deep lateral (fuse-save
  race across the chain), duty mode.

**Third real port bug caught (midi_controls hour 2, +2 iterations):** when a
StorageController flips the fleet state and an InvControl refreshes its DER
list in the SAME control round, the InvControl env's `der_set_nominal` (the
DER `SetNominalDEROutput` refresh) consumed the Storage `StateChanged` into
`yprim_invalid` but dropped Pascal `Set_YprimInvalid`'s `SystemYChanged` side
effect (CktElement.pas:245) — the oracle rebuilds Y in `CheckControls`
(Solution.pas:1155) inside the round, the port only one round later, off the
stale-state YPrim. Fixed in `dispatch.rs::InvDispEnv::der_set_nominal`. The
micro decks could not catch this: it needs two control classes touching the
same Storage in one round.

**Per-element midi wave:** every micro scenario is ALSO replayed on the midi
scaffold — one deck per element class (`midi_vsource_asym` … `midi_upfc_asym`,
12 decks in the asymmetric gate) and per control class (`midi_regcontrol` …
`midi_sensor`, 13 decks in the controls gate), all generated by
`gen_midi_decks.py` from the shared scaffold. This wave caught the FOURTH real
port bug: the InvControl/ExpControl parse-time fleet resolver took the
control's terminal phase count from the FIRST enabled DER, but Pascal's recalc
loop assigns `FNphases := ControlledElement[i].NPhases` per member — the LAST
one wins (InvControl.pas:916, ExpControl.pas:408; Pascal itself carries a TODO
about it). Invisible while every fleet member had equal phase counts (all the
micro decks); a mixed 3ph+1ph fleet (midi_invcontrol) exposes the control's
wrong conductor count. Fixed in `exec/command.rs` (+ `ForeignClasses::
last_enabled`). Also fixed: the oracle capture treated the C-API empty-array
placeholder `['NONE']` as a one-element `ZonePCE` list (visible with the
nested-meter `midi_energymeter` zone whose PCE list is genuinely empty).

64 decks total (27 asymmetric + 37 controls); `ASYMMETRIC_REQUIRED` /
`CONTROLS_REQUIRED` in `corpus_live.rs` pin the full sets — the guards fail if
a landed deck is ever dropped.
