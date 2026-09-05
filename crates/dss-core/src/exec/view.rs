//! Public query/snapshot API over [`Dss`] for the golden/test harness
//! (monitor buffers, meter zones, element snapshots, system Y, ...).
//! Split out of `exec/mod.rs`.

use super::*;
use crate::circuit::controls::{ControlCategory, control_category};
use crate::report::export::json::{
    JsonOpts, build as json_build, circuit as json_circuit, serialize as json_serialize,
};
use crate::support::complexutil::{Polar, c_to_polar_deg};

/// A monitor's recorded buffer for the golden/test harness (dss-python
/// `Monitors.Header` / `SampleCount` / `Channel(i)` / `dblHour`).
#[derive(Debug, Clone)]
pub struct MonitorView {
    pub header: Vec<String>,
    pub sample_count: i32,
    /// Per-sample hour values (record slot 0).
    pub dbl_hour: Vec<f64>,
    /// `channels[i]` = the (i+1)-th channel across all samples (f32).
    pub channels: Vec<Vec<f32>>,
    /// How many records `Save`/`SaveAll` made visible (Pascal `MonitorStream`
    /// length in records). `0` — nothing flushed — is the state in which
    /// `channels` and `dbl_hour` are both empty, in both lanes; the oracle
    /// clients read the same stream back as a `[0.0]` placeholder, which the
    /// harness normalizes out of their captures
    /// (`harness::lane::expected_monitor_channel`).
    pub flushed_records: usize,
}

/// Raw `ElemId` lists copied out of an [`energymeter::EnergyMeter`] before
/// resolving full names (avoids a long tuple type in [`Dss::meter_zone`]).
struct MeterZoneRefs {
    branches: Vec<ElemId>,
    ends: Vec<ElemId>,
    pce: Vec<ElemId>,
    register_names: Vec<String>,
}

/// An EnergyMeter's zone topology for the test/golden harness (dss-python
/// `Meters.AllBranchesInZone` / `AllEndElements` / `ZonePCE`).
#[derive(Debug, Clone)]
pub struct MeterZoneView {
    /// `AllBranchesInZone`: the zone branches in `SequenceList` order (FullNames).
    pub all_branches_in_zone: Vec<String>,
    /// `AllEndElements`: the feeder-end branches (FullNames).
    pub all_end_elements: Vec<String>,
    /// `ZonePCE`: the zone PC elements (loads/generators), FullNames.
    pub zone_pce: Vec<String>,
    /// `RegisterNames` (length `NumEMRegisters`).
    pub register_names: Vec<String>,
}

/// Which arm of the oracles' per-element symmetrical-component transform an
/// element takes: the three-way branch that all three sequence surfaces open
/// with — `NPhases <> 3` first, then `(NPhases = 1) and PositiveSequence`.
///
/// r4133 `Version8/Source/DDLL/DCktElement.pas:41`/`:43` (`CalcSeqCurrents`),
/// `:92`/`:94` (`CalcSeqVoltages`) and `:752`/`:754` (`CktElementV` mode `9`,
/// `SeqPowers`, which inlines its own copy of the same branch); capi
/// `CAPI/CAPI_Alt.pas:245`/`:248` (`_CalcSeqCurrents`), `:305`/`:308`
/// (`CalcSeqVoltages`) and `:551`/`:553` (`Alt_CE_Get_SeqPowers_`).
///
/// Both engines branch on the element's own `NPhases` and on the **circuit**'s
/// `PositiveSequence` flag (`Set CktModel=Positive`,
/// `Executive/ExecOptions.pas:786`; also set by `MakePosSeq`,
/// `Executive/ExecHelper.pas:3077`) — never on a value — so the arm is a
/// discrete property of the model and the live gate compares it as a flag with
/// zero tolerance. Each arm answers with a different *shape*: a transform, one
/// populated slot per terminal with exact zeros beside it, or a constant
/// sentinel. That is what makes the selector checkable at all — no capture
/// field spells the arm out, so the comparator derives it from the port's own
/// structural state and then asserts the shape the oracle actually returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeqArm {
    /// `NPhases = 3`: the 012 transform of the terminal's first three
    /// conductors.
    ThreePhase,
    /// `NPhases = 1` in a positive-sequence circuit: no transform runs — the
    /// single conductor's own quantity is reported as the positive-sequence
    /// component of its terminal, and the zero- and negative-sequence slots
    /// stay exactly zero.
    PosSeqSinglePhase,
    /// Anything else (2-phase, 4-or-more-phase, or 1-phase in a circuit that is
    /// not positive-sequence): upstream fills the whole result with an "n/A"
    /// sentinel instead of computing anything.
    NotAvailable,
}

/// Per-element snapshot for the golden feeder gate: mirrors dss-python's
/// `CktElement.Powers`/`Currents` over the oracle's `First/Next` iteration
/// (= creation) order.
#[derive(Debug, Clone)]
pub struct ElementSnapshot {
    /// `FullName` (`Class.name`).
    pub name: String,
    /// `Enabled`.
    pub enabled: bool,
    /// `BusNames`: the stored bus spec per terminal (`GetBus(i)`).
    pub bus_names: Vec<String>,
    /// Complex power per conductor and terminal, `kW + j·kvar` (CAPI
    /// `Alt_CE_Get_Powers`: `GetPhasePower · 0.001`). The oracle surface is a
    /// COM-style interleaved re/im `f64` array; the interleave is a *boundary*
    /// encoding and lives in the harness comparator, not in the engine type.
    pub powers: Vec<num_complex::Complex64>,
    /// Terminal current per conductor and terminal, amps (`Iterminal`).
    pub currents: Vec<num_complex::Complex64>,
    /// Element losses (W, var) — `TDSSCktElement.Get_Losses` (the dss-python
    /// `CktElement.Losses` surface): `Σ NodeV[ref]·conj(Iterminal)` over all
    /// conductors, ×3 under positive sequence.
    pub loss_w: (f64, f64),
    /// `CktElement.CurrentsMagAng`: `ctopolardeg` of every terminal current,
    /// conductor-minor inside terminal-major — the polar rendering of
    /// [`currents`](Self::currents), same length (`yorder`). Pascal r4133
    /// `DDLL/DCktElement.pas:1058` (mode `18`), capi `CAPI/CAPI_Alt.pas:1043`
    /// (`Alt_CE_Get_CurrentsMagAng`); a fastdss `_columns` surface
    /// (`dss/ICktElement.py:64` on `origin/fastdss`).
    pub currents_mag_ang: Vec<Polar>,
    /// `CktElement.VoltagesMagAng`: `ctopolardeg(NodeV[NodeRef[i]])` over the
    /// same conductor layout — the element's own view of the node voltages,
    /// i.e. a live check of its `NodeRef` mapping. Pascal r4133
    /// `DDLL/DCktElement.pas:1082` (mode `19`), capi `CAPI/CAPI_Alt.pas:1072`;
    /// fastdss `dss/ICktElement.py:58`.
    ///
    /// **Empty** when `node_ref` is empty — a never-energized element, whose
    /// `NodeRef` upstream is still `NIL`: capi returns its one-element
    /// `DefaultResult` `[0.0]` there (`CAPI_Alt.pas:1081` guards on
    /// `elem.NodeRef = NIL`) and r4133, which has no such guard, dereferences
    /// the nil pointer at `DCktElement.pas:1099` and takes the process down.
    /// Both sentinel shapes are a capture-boundary concern; the engine reports
    /// "no mapping yet" as the empty vector.
    pub voltages_mag_ang: Vec<Polar>,
    /// `CktElement.Residuals`: `ctopolardeg(Σ_c I[t·nconds + c])` per terminal
    /// (length `nterms`), each terminal summing **its own** conductors —
    /// Pascal r4133 `DDLL/DCktElement.pas:827` (mode `11`, the
    /// `k := (i-1)*Nconds` offset at `:842`), capi
    /// `CAPI/CAPI_CktElement.pas:541`; fastdss `dss/ICktElement.py:67`.
    /// Both API paths carry the offset — the missing-offset defect of
    /// CLAUDE.md upstream bug 1 is confined to the `Export SeqCurrents`
    /// report path and is not on this surface (pinned by
    /// `exec::tests::derived_polar::residuals_sum_the_rows_own_terminal`).
    pub residuals: Vec<Polar>,
    /// `CktElement.NumTerminals` — `NTerms`. Pascal r4133
    /// `DDLL/DCktElement.pas:139` (`CktElementI` mode `0`), capi
    /// `CAPI/CAPI_CktElement.pas:202`; fastdss `dss/ICktElement.py` `_columns`.
    pub n_terms: usize,
    /// `CktElement.NumConductors` — `NConds`. Pascal r4133
    /// `DDLL/DCktElement.pas:144` (mode `1`), capi `CAPI/CAPI_CktElement.pas:182`.
    pub n_conds: usize,
    /// `CktElement.NumPhases` — `NPhases`. Pascal r4133
    /// `DDLL/DCktElement.pas:149` (mode `2`), capi `CAPI/CAPI_CktElement.pas:192`.
    ///
    /// Not derivable from the other two: `NConds` is `NPhases` plus the neutral
    /// conductors, so this is the only channel that sees the phase count itself.
    pub n_phases: usize,
    /// `CktElement.NodeOrder`: the bus-local node number of every conductor
    /// slot, conductor-minor inside terminal-major (length
    /// `n_terms · n_conds = yorder`), ground = `0`. Pascal r4133
    /// `DDLL/DCktElement.pas:1032` (`CktElementV` mode `17`, the
    /// `GetNodeNum(NodeRef^[j])` map at `:1048` over `Common/Utilities.pas:1718`),
    /// capi `CAPI/CAPI_Alt.pas:953` (`Alt_CE_Get_NodeOrder`, the same
    /// allocation at `:968` and double loop at `:970-977`). The same mapping the `Export NodeOrder` report
    /// renders (`report/export/node_order.rs:35-38`) — read here from the
    /// element's own `NodeRef` so the two paths cannot drift (pinned by
    /// `exec::tests::element_extras::node_order_matches_the_export_nodeorder_row`).
    ///
    /// **Empty** when the element has no `NodeRef` yet (never energized — the
    /// state where capi raises 15013 at `CAPI_CktElement.pas:900-906` and r4133
    /// dereferences nil at `DCktElement.pas:1048`) or when it has no terminals
    /// at all (`UPFCControl`, r4133 `Controls/UPFCControl.pas:230-246`). A
    /// `NodeRef` shorter than `yorder` — reachable on a *disabled* element that
    /// grew phases, since only `set_node_ref` resizes it
    /// (`elements/ckt.rs:382`) and `reprocess_bus_defs` re-runs it for enabled
    /// elements only — reads the missing slots as ground, the same safe-`.get()`
    /// discipline [`voltages_mag_ang`](Self::voltages_mag_ang) uses.
    pub node_order: Vec<i32>,
    /// `CktElement.EnergyMeter`: the **bare** name of the EnergyMeter metering
    /// this element, or `None` when none does. Pascal r4133
    /// `DDLL/DCktElement.pas:442` (`CktElementS` mode `4`: `MeterObj.Name` only
    /// under `HasEnergyMeter`, else the family default `'0'` from `:421`), capi
    /// `CAPI/CAPI_CktElement.pas:672` (`Result := NIL` unless
    /// `Flg.HasEnergyMeter in elem.Flags`).
    ///
    /// The flag marks exactly the elements a meter *meters*, not the whole
    /// zone: `SetHasMeterFlag` clears it on every PD element and sets it on
    /// each enabled meter's `MeteredElement` (r4133
    /// `Meters/EnergyMeter.pas:1712-1719`, ported in
    /// `solution/meters/zones/flags.rs::set_has_meter_flag`), while
    /// `MakeMeterZoneLists` is what assigns that element's `MeterObj := Self`
    /// (`:1777`/`:1782`) — which is why upstream's unconditional
    /// `pPDElem.MeterObj.Name` dereference is nil-safe. The port asserts both
    /// halves (`HAS_ENERGY_METER` **and** a resolvable `meter_obj`) instead of
    /// assuming the second. The name is stored lowercase by the shared
    /// constructor (`elements/ckt.rs:258`), exactly as both oracles store it
    /// (r4133 `Meters/EnergyMeter.pas:921` `Name := LowerCase(...)`, capi
    /// `src/Meters/EnergyMeter.pas:952` `AnsiLowerCase`), so the channel is
    /// compared with no case folding. The two oracles' "no meter" sentinels
    /// (`''` on capi, `'0'` on r4133) are a capture-boundary shape normalized
    /// in the harness comparator; the engine's answer is simply `None`.
    pub energy_meter: Option<String>,
    /// `CktElement.PhaseLosses`: the complex losses of each **phase** (length
    /// `n_phases`), `Σ_terminals NodeV[NodeRef[k]]·conj(Iterminal[k])` at
    /// `k = j·NConds + i`, neutral conductors ignored —
    /// [`CktElement::phase_losses`](crate::elements::traits::CktElement::phase_losses),
    /// the port of r4133 `Common/CktElement.pas:1078-1120`
    /// (`TDSSCktElement.GetPhaseLosses`).
    ///
    /// **W/var here**, like [`loss_w`](Self::loss_w): both oracle surfaces scale
    /// by `0.001` at the API boundary — r4133 `DDLL/DCktElement.pas:637-659`
    /// (`CktElementV` mode `6`), capi `CAPI/CAPI_Alt.pas:449-467`
    /// (`Alt_CE_Get_PhaseLosses`, facade `CAPI/CAPI_CktElement.pas:327-338`) —
    /// so the kW/kvar rendering is a capture-boundary encoding and lives in the
    /// harness comparator, exactly as the interleaved re/im array does. A
    /// fastdss `ICktElement._columns` surface
    /// (`git -C .inputs/DSS-Python show origin/fastdss:dss/ICktElement.py`).
    ///
    /// This is a **cache-aware** quantity (`ComputeIterminal`,
    /// `Common/CktElement.pas:1090`) like `Powers`/`Losses`, so it is read from
    /// the one fresh terminal current this snapshot computes and it shares their
    /// `newton*` lane exclusion (`tests/harness/lane.rs::LANE_SKIP_ELEM_POWERS`).
    pub phase_losses: Vec<num_complex::Complex64>,
    /// `CktElement.NumControls` — `ControlElementList.ListSize`, with **no**
    /// `Enabled` filter on either channel: r4133 `DDLL/DCktElement.pas:237-241`
    /// (`CktElementI` mode `9`), capi `CAPI/CAPI_CktElement.pas:939-948`.
    /// A fastdss `ICktElement._columns` surface.
    ///
    /// Derived, with the four scalars below, from
    /// [`crate::circuit::controls::derive_control_lists`] — the port stores only
    /// the forward control → element reference.
    pub num_controls: usize,
    /// `CktElement.OCPDevIndex` — the **1-based** position in
    /// `ControlElementList` of the first Fuse/Recloser/Relay, `0` when there is
    /// none: r4133 `DDLL/DCktElement.pas:242-258` (mode `10`), capi
    /// `CAPI/CAPI_CktElement.pas:951-976` (the identical
    /// `repeat … until (i > listSize) or (Result > 0)`).
    pub ocp_dev_index: usize,
    /// `CktElement.OCPDevType` — `GetOCPDeviceType`'s code for that same first
    /// OCP member: `1` Fuse, `2` Recloser, `3` Relay, `0` none. r4133
    /// `Common/Utilities.pas:3165-3184` (reached from `DDLL/DCktElement.pas:259-262`,
    /// mode `11`), capi `CAPI/CAPI_CktElement.pas:978-988`.
    ///
    /// Recomputed from the list on every read, never latched: upstream's scan
    /// has no `Enabled` test, so a **disabled** OCP control still occupies its
    /// slot and still wins (measured on both channels; pinned by
    /// `exec::tests::element_extras::a_disabled_ocp_control_still_wins_the_ocp_scan`).
    /// The reliability sweep's `CktElementData::ocp_device_type` is a different,
    /// registration-time latch and is deliberately not read here.
    pub ocp_dev_type: i32,
    /// `CktElement.HasVoltControl` — "any member of `ControlElementList` is a
    /// `CAP_CONTROL` or a `REG_CONTROL`": r4133 `DDLL/DCktElement.pas:222-236`
    /// (mode `8`; its `else Result := 0` is re-evaluated per member but the loop
    /// `Exit`s on a hit, so it is still "any"), capi
    /// `CAPI/CAPI_CktElement.pas:689-710`.
    pub has_volt_control: bool,
    /// `CktElement.HasSwitchControl` — "any member is a `SWT_CONTROL`": r4133
    /// `DDLL/DCktElement.pas:207-221` (mode `7`), capi
    /// `CAPI/CAPI_CktElement.pas:713-734`.
    pub has_switch_control: bool,
    /// Which arm of the sequence transform this element takes — derived from
    /// `n_phases` and the circuit's positive-sequence flag exactly as both
    /// oracles derive it (see [`SeqArm`]). Not an oracle *surface* of its own:
    /// it is the discrete selector behind the three arrays below, compared by
    /// the live gate through the shape each arm forces on them.
    pub seq_arm: SeqArm,
    /// `CktElement.SeqCurrents`: `Cabs` of the terminal current's
    /// symmetrical components, `(0, +, -)` per terminal, conductor-minor inside
    /// terminal-major — length `3 * n_terms` (amps). r4133
    /// `DDLL/DCktElement.pas:700-737` (`CktElementV` mode `8`) over
    /// `CalcSeqCurrents` `:30-80`; capi `CAPI/CAPI_Alt.pas:490-527`
    /// (`Alt_CE_Get_SeqCurrents`) over `_CalcSeqCurrents` `:236-290`. A fastdss
    /// `ICktElement._columns` surface (`dss/ICktElement.py:65`/`:483` on
    /// `origin/fastdss`).
    ///
    /// **Not** the `Export SeqCurrents` report path
    /// (`report/export/seq_currents.rs`), which branches on `nphases >= 3`,
    /// carries the report's own rating/`Iresidual` logic and has no sentinel
    /// arm at all — a frozen golden that must not be re-plumbed
    /// (`GOLDEN_REBASE_PLAN.md` WP-G1: no golden byte moves).
    pub seq_currents: Vec<f64>,
    /// `CktElement.SeqVoltages`: `Cabs` of the symmetrical components of the
    /// node voltages this element's `NodeRef` points at, same `(0, +, -)`
    /// layout and length (volts). r4133 `DDLL/DCktElement.pas:660-698` (mode
    /// `7`) over `CalcSeqVoltages` `:84-122`; capi `CAPI/CAPI_Alt.pas:620-659`
    /// (`Alt_CE_Get_SeqVoltages`) over `CalcSeqVoltages` `:294-338`; fastdss
    /// `dss/ICktElement.py:59`/`:501`.
    pub seq_voltages: Vec<f64>,
    /// `CktElement.SeqPowers`: the per-terminal sequence powers
    /// `V012_k * conj(I012_k) * 0.003`, same `(0, +, -)` layout and length.
    /// r4133 `DDLL/DCktElement.pas:739-797` (mode `9`, which inlines its own
    /// copy of the branch rather than calling the two `Calc*` helpers); capi
    /// `CAPI/CAPI_Alt.pas:529-593` (`Alt_CE_Get_SeqPowers_`, facade `:594-618`);
    /// fastdss `dss/ICktElement.py:62`/`:492`.
    ///
    /// **kW/kvar here**, unlike [`loss_w`](Self::loss_w) and
    /// [`phase_losses`](Self::phase_losses), which stay in W/var: those two are
    /// scaled by `0.001` at the *API boundary*, so their kW rendering is a
    /// capture encoding, whereas the `0.003` here is applied *inside* the arm
    /// on both engines (r4133 `:767` and `:788` `cmulreal(..., 0.003)`, capi
    /// `:561` and `:588-590`) and is part of what the quantity *is* — a
    /// three-phase kVA conversion, applied unconditionally and **not** the
    /// `PositiveSequence` x3 that `Get_Powers` applies to
    /// [`powers`](Self::powers).
    pub seq_powers: Vec<num_complex::Complex64>,
    /// `CktElement.CplxSeqCurrents`: the **complex** symmetrical components of
    /// the terminal current — the `(0, +, −)` layout and the `3 * n_terms`
    /// length of [`seq_currents`](Self::seq_currents), amps, before the `Cabs`.
    /// r4133 `DDLL/DCktElement.pas:931-975` (`CktElementV` mode `14`, guard
    /// `If Enabled` only) over the shared `CalcSeqCurrents` `:30-80`; capi
    /// `CAPI/CAPI_Alt.pas:898-925` (`Alt_CE_Get_ComplexSeqCurrents`, guard
    /// `MissingSolution or (not Enabled)` at `:906` — **no** `NodeRef` test,
    /// unlike its voltage twin), facade `CAPI/CAPI_CktElement.pas:752-760`;
    /// fastdss `dss/ICktElement.py:66` on `origin/fastdss`.
    ///
    /// One transform, one `norm()`: `seq_currents[k]` is
    /// `cplx_seq_currents[k].norm()` by construction, pinned by
    /// `exec::tests::derived_totals::cplx_seq_currents_are_the_012_components_whose_magnitudes_are_seq_currents`.
    /// The n/A sentinel is `(-1, 0)` on **both** engines here (r4133
    /// `DCktElement.pas:60`, capi `CAPI_Alt.pas:268` — the FPC `ucomplex` real
    /// assignment `i012[i] := -1` is the same complex value), so unlike
    /// [`seq_powers`](Self::seq_powers) this surface needs no channel
    /// normalization; and the positive-sequence slot defect of r4133's mode 9
    /// does not reach it either — modes 13/14 call the shared helper, whose
    /// `iV := 2` indexes a ONE-based buffer with stride 3 (`:50`/`:55`).
    pub cplx_seq_currents: Vec<num_complex::Complex64>,
    /// `CktElement.CplxSeqVoltages`: the same for the node voltages this
    /// element's `NodeRef` points at (volts) — the un-`Cabs`'d
    /// [`seq_voltages`](Self::seq_voltages). r4133
    /// `DDLL/DCktElement.pas:885-928` (`CktElementV` mode `13`) over
    /// `CalcSeqVoltages` `:84-122`; capi `CAPI/CAPI_Alt.pas:872-895`
    /// (`Alt_CE_Get_ComplexSeqVoltages`, guard
    /// `MissingSolution or (not Enabled) or (NodeRef = NIL)` at `:878`), facade
    /// `CAPI/CAPI_CktElement.pas:735-743`; fastdss `dss/ICktElement.py:60`.
    /// Same `(-1, 0)` n/A sentinel on both engines (r4133 `:106`, capi `:324`).
    pub cplx_seq_voltages: Vec<num_complex::Complex64>,
    /// `CktElement.TotalPowers`: the per-terminal sum of
    /// [`powers`](Self::powers)' conductors, `kW + j·kvar`, length `n_terms`.
    /// r4133 `DDLL/DCktElement.pas:1109-1139` (`CktElementV` mode `20`: the
    /// `myInit := (j-1)*NConds+1 … myEnd := NConds*j` conductor walk at
    /// `:1123-1124` and `cmulreal(…, 0.001)` at `:1134`); capi
    /// `CAPI/CAPI_Alt.pas:1108-1141` (`Alt_CE_Get_TotalPowers`, the same walk
    /// and `total.re * 0.001` at `:1138-1139`), facade
    /// `CAPI/CAPI_CktElement.pas:1043-1053`. A fastdss `_columns` entry
    /// (`dss/ICktElement.py:53`) that fastdss itself *removes* in the COM/Oddie
    /// configuration (`tests/save_outputs.py:198-200`), so gating it live is
    /// stronger than fastdss parity.
    ///
    /// The `0.001` is applied **once per terminal, to the summed W/var**, as
    /// both engines apply it — not per conductor, which is why this is
    /// accumulated beside [`powers`](Self::powers) instead of being summed from
    /// it (the two forms differ by up to `3.6e-13` kVA over IEEE13's elements,
    /// `5.1e-13` kVA on the oracle's own numbers; pinned by
    /// `exec::tests::derived_totals::total_powers_are_the_terminal_sums_of_the_phase_powers`).
    /// `GetPhasePower` (r4133 `Common/CktElement.pas:1041-1071`) zero-fills a
    /// disabled element and skips the `NodeRef = 0` neutral slots, so a
    /// disabled or never-energized element reports `n_terms` exact zeros and a
    /// 0-terminal element reports nothing.
    pub total_powers: Vec<num_complex::Complex64>,
}

/// `(n, [(row, col, value)])` — the assembled, unfactored system Y as 0-based
/// coordinates, returned by [`Dss::system_y_csc`].
pub type SystemYCsc = (usize, Vec<(usize, usize, num_complex::Complex64)>);

/// A bus's short-circuit results after a FaultStudy solve — the dss-python
/// `Bus.Zsc1`/`Zsc0`/`Isc` surface (`TDSSBus`). `isc`/`vbus` are empty until a
/// FaultStudy has allocated the bus quantities.
#[derive(Debug, Clone)]
pub struct BusScView {
    /// User node numbers on the bus (`Nodes`).
    pub nodes: Vec<i32>,
    /// `Zsc1`: positive-sequence short-circuit impedance.
    pub zsc1: num_complex::Complex64,
    /// `Zsc0`: zero-sequence short-circuit impedance.
    pub zsc0: num_complex::Complex64,
    /// `Isc` / `BusCurrent`: per-node short-circuit current (= `Ysc · VBus`).
    pub isc: Vec<num_complex::Complex64>,
    /// The bus's stored `VBus` — the open-circuit (Voc) voltage captured by
    /// `UpdateVBus` during the study. Note this is **not** dss-python
    /// `Bus.Voltages`, which returns the live `NodeV` (after a FaultStudy that is
    /// the last `ComputeYsc` unit-injection residual, not the Voc).
    pub vbus: Vec<num_complex::Complex64>,
}

impl Dss {
    /// Snapshot every circuit element's terminal powers and currents in
    /// creation order (the oracle's `First/Next` order). Pascal
    /// `TDSSCktElement.GetPhasePower` / `ComputeIterminal`.
    pub fn snapshot_elements(&mut self) -> Vec<ElementSnapshot> {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_ref().expect("snapshot needs a circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let positive_seq = ckt.positive_sequence;
        // Per-terminal complex power below is formed as S = V*conj(I)
        // (`node_v[n] * i.conj()`): node voltage times the conjugate of the
        // terminal current -- the IEEE definition of complex power
        // (IEEE Std 1459-2010, 3.1.1.6, p. 5: S = P + jQ = V*I_conj; and
        // J. L. Willems, "The IEEE Standard 1459: What and Why?", sec. III-IV,
        // eq. (3), p. 2: P = V0*I0 + sum_k V_k*I_k*cos(phi_k)) -- with V and I
        // taken at the SAME frequency.
        //
        // In harmonics mode the engine solves each harmonic order h as an
        // independent per-frequency phasor network, V_bus^h = inv(Y_bus^h)*I_bus^h
        // (N.-C. Yang & Y.-W. Hsu, "OpenDSS-based Harmonic Power Flow Analysis for
        // Power Systems with Passive Power Filters", IEEE Access, 2023, sec. IV,
        // eq. (28)), so the meaningful terminal power is the per-harmonic complex
        // power S_h = V_h*conj(I_h) (IEEE 1459-2010, 3.1.2.5, p. 9:
        // P_H = V0*I0 + sum_{h!=1} V_h*I_h*cos(theta_h), where theta_h is "the
        // phase angle between the phasors V_h and I_h" -- the SAME order h for
        // both; cf. 3.1.2.4, p. 9: P1 = V1*I1*cos(theta_1)).
        //
        // A product mixing a voltage at one harmonic with a current at a different
        // harmonic, V_h*conj(I_{h'!=h}), is NOT a power: cross-frequency terms
        // appear only in the non-active instantaneous power p_q, whose average is
        // zero (IEEE 1459-2010, 3.1.2.2, pp. 8-9: the
        // 2*sum_n sum_{m!=n} V_m*I_n*sin(m*w*t-a_m)*sin(n*w*t-b_n) term; the
        // standard states p_q "does not represent a net transfer of energy (i.e.,
        // its average value is nil)"). The root reason is the orthogonality of the
        // harmonic (Fourier) basis over a fundamental period (W. M. Grady,
        // "Understanding Power System Harmonics", Apr. 2012, ch. 2,
        // eqs. (2.1)-(2.2), pp. 2-1..2-3).
        //
        // We therefore call `compute_iterminal` ONCE per element and immediately
        // form V*conj(I) from that single fresh terminal current, so V and I are
        // always the same frequency and the cross-frequency product can never
        // arise. This is a deliberate divergence from the pinned oracle, whose
        // `CktElement.Powers` returns V_h*conj(I_fundamental) when read AFTER
        // `CktElement.Currents` for a Generator/PVSystem/Storage in harmonics mode
        // (an order-dependent, stale-Iterminal engine bug we do NOT reproduce;
        // full analysis + IEEE-1459 proof live in the git-ignored
        // investigations/oracle-powers-currents-harmonic/).
        // `EnergyMeter` (`ElementSnapshot::energy_meter`) reports the *bare*
        // meter name, so every `meter_obj` back-pointer has to be resolved
        // against the EnergyMeter arena. That resolution happens once, here,
        // before the element loop takes its own mutable borrow of `classes`.
        let meter_names: std::collections::HashMap<ElemId, String> = ckt
            .energy_meters
            .iter()
            .map(|&m| {
                (
                    m,
                    classes[m.class_ord()]
                        .arena
                        .obj(m.index())
                        .data()
                        .name()
                        .to_string(),
                )
            })
            .collect();
        // Pascal gives every element its own `ControlElementList`
        // (`Common/CktElement.pas:100`, `:223`); the port stores only the
        // forward control → element reference, so the five control-derived
        // scalars read a map derived once here — before the element loop takes
        // its own mutable borrow of `classes` — from the circuit-wide attach
        // order. Same helper `Show Controlled` uses, so the report and this
        // reader cannot drift.
        let control_lists = crate::circuit::controls::derive_control_lists(classes, ckt);
        // The 012 matrix pair for the three sequence surfaces below, built
        // once for the whole snapshot. `SymComp::default()` is
        // [`SymComp::precise`](crate::support::mathutil::SymComp::precise),
        // which is what `Shared/mathutil.pas:548` `SelectAs2pVersion(False)`
        // leaves both oracles on by default — capi bit-for-bit. r4133's own
        // globals are built by inverting the truncated-`sin 60` matrix
        // (`Shared/mathutil.pas:302-303` + `:562-564`), a different matrix that
        // `SymComp::official` models exactly; the resulting gap on the `r4133`
        // gating channel is a measured *floor*, not a second kernel, and is
        // carried by the comparator (`tests/TOLERANCE_NOTES.md` §G1.3b).
        let sym_comp = crate::support::mathutil::SymComp::default();
        let mut out = Vec::with_capacity(ckt.ckt_elements.len());
        for &r in &ckt.ckt_elements {
            let class_name = classes[r.class_ord()].props.class_name();
            let obj = &mut classes[r.class_ord()].arena[r.index()];
            let name = format!("{}.{}", class_name, obj.data().name());
            let elem = classes[r.class_ord()]
                .arena
                .try_ckt_elem_mut(r.index())
                .expect("ckt_elements refs are circuit elements");
            let yorder = elem.cd().yorder;
            let mut currents = vec![num_complex::Complex64::ZERO; yorder];
            let mut powers = vec![num_complex::Complex64::ZERO; yorder];
            // `TotalPowers` (r4133 `DDLL/DCktElement.pas:1109-1139` mode `20`,
            // capi `CAPI/CAPI_Alt.pas:1108-1141`) accumulates the SAME
            // `GetPhasePower` slots this loop fills, in W/var, and scales the
            // per-terminal total by `0.001` once (r4133 `:1134`, capi
            // `:1138-1139`) — so it is summed here, unscaled, rather than from
            // the already-scaled `powers` below. A disabled or never-energized
            // element keeps its `nterms` zeros: `GetPhasePower`'s `Else … CZERO`
            // arm (`Common/CktElement.pas:1071`) and the `n > 0` test are the
            // same two zero paths.
            let mut total_w = vec![num_complex::Complex64::ZERO; elem.cd().nterms];
            // Powers (and Losses, below) model the oracle's `Get_Powers` /
            // `Get_Losses`, which route through the cache-aware `ComputeIterminal`;
            // Currents model the fresh `CktElement.Currents` (`GetCurrents`,
            // `CAPI_CktElement.pas`). The two `Iterminal` read paths agree after
            // every fixed-point / direct / harmonic solve — the cache is invalid
            // there, so upstream's cache-aware read recomputes fresh at the
            // present `NodeV` too, and the single-frequency reasoning in the
            // block comment above holds.
            //
            // After a **Newton** solve the two read paths diverge upstream, and
            // that divergence is an upstream bug we do NOT reproduce (CLAUDE.md
            // known bug 5, torn down in both lanes by `GOLDEN_REBASE_PLAN.md`
            // G2.3). `DoNewtonSolution`'s final `SumAllCurrents` stamps
            // `Iterminal` at the pre-final voltage guess `NodeV_{n-1}` and marks
            // it solved for this `SolutionCount` (the `NodeV -= dV` update
            // follows it), so upstream's cache-aware path (Powers/Losses)
            // returns a one-step-stale current while `GetCurrents` (Currents)
            // recomputes at the converged `NodeV_n` — i.e. it reports
            // `S != V·conj(I)` for one element in one read (`Vsource.pas`
            // `GetCurrents` reads `NodeV` directly, whereas `CktElement.pas`
            // `Get_Powers`/`Get_Losses` reuse `ComputeIterminal`).
            //
            // So this is `refresh_iterminal` unconditionally: one fresh current
            // at `NodeV_n` feeds Powers, Losses AND Currents, and the identity
            // `S = V·conj(I)` — which is what `Powers` *means* — holds under
            // every algorithm. Nothing but a Newton solve moves: everywhere else
            // the cache is invalid here, so the cache-aware read computed the
            // same value.
            //
            // GATE NOTE: this staleness was the ONLY feature-sensitive signal
            // distinguishing `algorithm=Newton` from the normal fixed point on
            // the `newton.dss` / `newton_feeder.dss` corpus decks (same voltages,
            // same iteration count), and no oracle channel reports those decks'
            // powers/losses correctly — so both lanes now drop exactly those two
            // channels for those two decks
            // (`tests/harness/lane.rs::LANE_SKIP_ELEM_POWERS`) and the signal is
            // carried instead by `exec::tests::newton`'s in-engine
            // Newton-dispatch tripwire plus its expected-value pin.
            // See investigations/issue-05-newton-stale-iterminal.md.
            if elem.cd().enabled && !elem.cd().node_ref.is_empty() {
                elem.refresh_iterminal(&sys, &node_v);
                let cd = elem.cd();
                // Terminal-major / conductor-minor flat layout with
                // `yorder = nterms · nconds` (`elements/ckt.rs:335`), so the
                // conductor slot `k` belongs to terminal `k / nconds` — the
                // `(j-1)*NConds+1 … NConds*j` window both engines sum over.
                let width = cd.nconds.max(1);
                for (k, ((p, &n), i)) in powers
                    .iter_mut()
                    .zip(&cd.node_ref[..yorder])
                    .zip(&cd.iterminal[..yorder])
                    .enumerate()
                {
                    if n > 0 {
                        // S = V*conj(I) at the present (per-harmonic, in harmonics
                        // mode) solution frequency; see the block comment above.
                        let mut s = node_v[n] * i.conj();
                        if positive_seq {
                            // x3: balanced three-phase scaling of the single-phase
                            // power (Willems, "...What and Why?", sec. V.A, p. 3).
                            s *= 3.0;
                        }
                        total_w[k / width] += s;
                        // `* 0.001` on `Complex64` is componentwise
                        // (`Complex::new(re * s, im * s)`) — the same two
                        // multiplications the interleaved form did.
                        *p = s * 0.001;
                    }
                }
            }
            let total_powers: Vec<num_complex::Complex64> =
                total_w.iter().map(|s| s * 0.001).collect();
            // The element's own losses path (`Get_Losses`) — the same cache-aware
            // `ComputeIterminal`, read BEFORE the currents refresh below so it
            // reuses the fresh current the Powers block just left in the cache
            // (`refresh_iterminal` stamps it for this `SolutionCount`), i.e.
            // Powers and Losses are one and the same current by construction.
            let loss = elem.losses(&sys, &node_v);
            // `PhaseLosses` — the same cache-aware `ComputeIterminal`
            // (r4133 `Common/CktElement.pas:1090`), so it reads the one current
            // Powers and Losses just used: the `refresh_iterminal` above stamped
            // it for this `SolutionCount`. Read here, before the Currents
            // refresh below, so this element's three cache-aware quantities are
            // one and the same current — the port-side twin of the capture-order
            // rule the two oracle transports obey (§1.1(a): group A before
            // group B).
            let phase_losses = elem.phase_losses(&sys, &node_v);
            // Currents: fresh recompute from the converged `NodeV` (oracle
            // `GetCurrents`), overwriting the `Iterminal` cache after Powers/Losses.
            if elem.cd().enabled && !elem.cd().node_ref.is_empty() {
                elem.refresh_iterminal(&sys, &node_v);
                let cd = elem.cd();
                currents.copy_from_slice(&cd.iterminal[..yorder]);
            }
            let cd = elem.cd();
            let bus_names = (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect();
            // The three polar surfaces (`CurrentsMagAng`, `Residuals`,
            // `VoltagesMagAng`) are *renderings* of state this loop has already
            // produced, so they are formed here from the one current computed
            // above rather than by a second read path: upstream allocates a
            // scratch buffer and calls `GetCurrents` again for each of them
            // (r4133 `DDLL/DCktElement.pas:1068`/`:837`), which is the same
            // current whenever the cache is invalid and, after a Newton solve,
            // the *fresh* one this snapshot already uses (CLAUDE.md upstream
            // bug 5 / `GOLDEN_REBASE_PLAN.md` G2.3 — see the block comment
            // above). Doing it here also keeps `report/export/*` untouched.
            //
            // `c_to_polar_deg` is the port of `CtoPOLARdeg`
            // (`Shared/Ucomplex.pas:131` in r4133 == `DSSUcomplex.pas` in capi):
            // `Cabs` for the magnitude and the truncated-constant `CDANG`
            // (`57.29577951`, `Ucomplex.pas:118`) for the angle in
            // `(-180, 180]`. Both gating oracles carry that same truncation, so
            // no new compat site is created here — the existing one is
            // compat-tagged on the constants themselves (`support::complexutil`).
            let currents_mag_ang: Vec<Polar> =
                currents.iter().copied().map(c_to_polar_deg).collect();
            // Residual per terminal: the sum of *that terminal's own*
            // conductors, `k := (i-1)*Nconds` (r4133 `DCktElement.pas:842`,
            // capi `CAPI_CktElement.pas:562`) — accumulated in conductor order
            // so the floating-point summation matches `Caccum`'s. The offset
            // that `Export SeqCurrents` drops (CLAUDE.md upstream bug 1) is
            // present on both API paths and is honoured here.
            //
            // Read through a per-terminal chunk, never a flat offset: the
            // terminal-major offset arithmetic lives only in the `elements::ckt`
            // accessors (`elements/ckt.rs:415-419`). `chunks` refuses a zero
            // width, so a conductor-less element (`nconds = 0`, hence
            // `yorder = 0` and an empty `currents`) takes the width `1` — the
            // iterator is empty either way and every terminal's residual is the
            // empty sum, exactly what the flat form produced.
            let mut terminal_currents = currents.chunks(cd.nconds.max(1));
            let residuals: Vec<Polar> = (0..cd.nterms)
                .map(|_| {
                    let mut resid = num_complex::Complex64::ZERO;
                    for &i in terminal_currents.next().unwrap_or(&[]) {
                        resid += i;
                    }
                    c_to_polar_deg(resid)
                })
                .collect();
            // `VoltagesMagAng` reads `NodeV` through the element's own
            // `NodeRef` (r4133 `DCktElement.pas:1096-1100`), so it is the one
            // surface that exposes the per-element node mapping rather than the
            // node vector itself. `NodeRef[i] = 0` is the ground node and
            // `NodeV[0]` is zero (`solution::ymatrix`, Pascal's `// ok if =0`).
            // A `NodeRef` left over from before a topology change can outrun the
            // present `NodeV`, and `Yorder` can outrun the `NodeRef` itself —
            // both only on a disabled element, which no oracle channel compares
            // here (upstream would read freed memory). `set_nterms`/`set_nconds`
            // grow `yorder` and reallocate the terminal buffers, but only
            // `set_node_ref` resizes `node_ref` (`elements/ckt.rs:326`,
            // `:334-346`, `:382`) and `reprocess_bus_defs` re-runs it for
            // **enabled** elements only (`circuit/circuit.rs:735`), so a
            // disabled element that grows phases keeps a short `node_ref`.
            // Both stale slots read as ground instead of panicking — the
            // safe-`.get()` discipline `solution::meters::reliability` uses,
            // applied to the length as well as to the value. Pinned by
            // `exec::tests::derived_polar::a_stale_node_ref_shorter_than_yorder_reads_as_ground`.
            let voltages_mag_ang: Vec<Polar> = if cd.node_ref.is_empty() {
                Vec::new()
            } else {
                (0..yorder)
                    .map(|i| {
                        let n = cd.node_ref.get(i).copied().unwrap_or(0);
                        c_to_polar_deg(
                            node_v
                                .get(n)
                                .copied()
                                .unwrap_or(num_complex::Complex64::ZERO),
                        )
                    })
                    .collect()
            };
            // The three sequence surfaces (`SeqCurrents`, `SeqVoltages`,
            // `SeqPowers`) — like the polar three above, renderings of state
            // this loop already holds, formed from the one fresh terminal
            // current and the converged `NodeV` rather than by a second read
            // path. Upstream reaches each of them through its own entry point,
            // and each allocates a scratch buffer and calls `GetCurrents`
            // again: r4133 `DDLL/DCktElement.pas:660-698` (mode `7`) over
            // `CalcSeqVoltages` `:84-122`, `:700-737` (mode `8`) over
            // `CalcSeqCurrents` `:30-80`, and `:739-797` (mode `9`), which
            // inlines its own copy of both; capi `CAPI/CAPI_Alt.pas:620-659`,
            // `:490-527` and `:529-593` over `CalcSeqVoltages` `:294-338` /
            // `_CalcSeqCurrents` `:236-290`.
            //
            // Three deliberate divergences from the oracles are settled here,
            // each pinned in `exec::tests::derived_seq` and excluded
            // field-by-field on the channel it belongs to:
            //
            // 1. the **positive-sequence 1-phase arm** writes slot `3t+1` of
            //    each terminal, with `3t` and `3t+2` left at zero. That is what
            //    every `Calc*` helper does on both engines (`iV := 2` on a
            //    ONE-based `pComplexArray`, `Inc(iV, 3)`: r4133 `:50`/`:55`,
            //    `:97`/`:102`, capi `:255`/`:261`, `:312`/`:318`) and what
            //    capi's `SeqPowers` does on its ZERO-based result
            //    (`iCount := 1`, `inc(icount, 3)`, `:555`/`:562`). r4133's
            //    mode 9 transplanted the 1-based `2` into the 0-based array and
            //    kept a stride of 1 (`Count := 2` `:760`, `inc(count)` `:768`),
            //    so it writes slots `2, 3, 4, …` — one slot late and walking
            //    over the next terminals. The port emits the correct layout;
            //    the defect is reported upstream and is oracle-gated live on
            //    the `capi_v0145` channel, which has it right.
            // 2. the **n/A sentinel of `SeqPowers`** is r4133's `(-1, 0)`
            //    (`:772`), not capi's `(-1, -1)` (`:567`); r4133 is the
            //    behavioral authority and it is also the spelling both engines
            //    already agree on for the two magnitude arrays, whose sentinel
            //    is `Cabs(-1 + 0j) = 1.0` on either side (`:60`, `:106`,
            //    `:268`, `:324`). The capi spelling is normalized in the
            //    comparator, gated on the arm, never on the value.
            // 3. the **0.003** is applied here, inside the arm, exactly as both
            //    engines apply it (r4133 `:767`/`:788`, capi `:561`/`:588-590`)
            //    — a three-phase kVA conversion that is NOT the
            //    `PositiveSequence` ×3 of `Get_Powers`.
            //
            // Terminal chunks, never a flat offset (the `depascalize_metrics_
            // gate` ceiling): `chunks` refuses a zero width, so a
            // conductor-less element takes width `1` and yields nothing, and a
            // `node_ref` that is empty or shorter than `yorder` (a disabled
            // element that grew phases — see `voltages_mag_ang` above) reads
            // its missing slots as ground, the same safe-`.get()` discipline.
            let seq_arm = if cd.nphases == 3 {
                SeqArm::ThreePhase
            } else if cd.nphases == 1 && positive_seq {
                SeqArm::PosSeqSinglePhase
            } else {
                SeqArm::NotAvailable
            };
            let mut seq_currents: Vec<f64> = Vec::with_capacity(3 * cd.nterms);
            let mut seq_voltages: Vec<f64> = Vec::with_capacity(3 * cd.nterms);
            let mut seq_powers: Vec<num_complex::Complex64> = Vec::with_capacity(3 * cd.nterms);
            // The un-`Cabs`'d twins of the two magnitude arrays — the very
            // values the transform below produces, kept instead of discarded:
            // r4133 `DDLL/DCktElement.pas:931-975` (mode `14`) and `:885-928`
            // (mode `13`) copy `CalcSeqCurrents`/`CalcSeqVoltages`' complex
            // buffer out unchanged, as do capi's `Alt_CE_Get_ComplexSeq*`
            // (`CAPI/CAPI_Alt.pas:898-925`, `:872-895`). Divergences 1 and 2
            // above are `SeqPowers`-only: these modes take the shared helpers,
            // whose positive-sequence slot is the correct `3t+1`, and their n/A
            // sentinel is `(-1, 0)` on both engines (r4133 `:60`/`:106`, capi
            // `:268`/`:324`).
            let mut cplx_seq_currents: Vec<num_complex::Complex64> =
                Vec::with_capacity(3 * cd.nterms);
            let mut cplx_seq_voltages: Vec<num_complex::Complex64> =
                Vec::with_capacity(3 * cd.nterms);
            {
                let width = cd.nconds.max(1);
                let mut term_currents = currents.chunks(width);
                let mut term_nodes = cd.node_ref.chunks(width);
                for _ in 0..cd.nterms {
                    let i_chunk = term_currents.next().unwrap_or(&[]);
                    let n_chunk = term_nodes.next().unwrap_or(&[]);
                    let cur = |j: usize| {
                        i_chunk
                            .get(j)
                            .copied()
                            .unwrap_or(num_complex::Complex64::ZERO)
                    };
                    let volt = |j: usize| {
                        node_v
                            .get(n_chunk.get(j).copied().unwrap_or(0))
                            .copied()
                            .unwrap_or(num_complex::Complex64::ZERO)
                    };
                    match seq_arm {
                        SeqArm::ThreePhase => {
                            let iph = [cur(0), cur(1), cur(2)];
                            let vph = [volt(0), volt(1), volt(2)];
                            let mut i012 = [num_complex::Complex64::ZERO; 3];
                            let mut v012 = [num_complex::Complex64::ZERO; 3];
                            sym_comp.phase_to_sym(&iph, &mut i012);
                            sym_comp.phase_to_sym(&vph, &mut v012);
                            for (v, i) in v012.into_iter().zip(i012) {
                                seq_currents.push(i.norm());
                                seq_voltages.push(v.norm());
                                seq_powers.push(v * i.conj() * 0.003);
                                cplx_seq_currents.push(i);
                                cplx_seq_voltages.push(v);
                            }
                        }
                        SeqArm::PosSeqSinglePhase => {
                            // The terminal's FIRST conductor only, into the
                            // positive-sequence slot.
                            let (v, i) = (volt(0), cur(0));
                            seq_currents.extend([0.0, i.norm(), 0.0]);
                            seq_voltages.extend([0.0, v.norm(), 0.0]);
                            seq_powers.extend([
                                num_complex::Complex64::ZERO,
                                v * i.conj() * 0.003,
                                num_complex::Complex64::ZERO,
                            ]);
                            cplx_seq_currents.extend([
                                num_complex::Complex64::ZERO,
                                i,
                                num_complex::Complex64::ZERO,
                            ]);
                            cplx_seq_voltages.extend([
                                num_complex::Complex64::ZERO,
                                v,
                                num_complex::Complex64::ZERO,
                            ]);
                        }
                        SeqArm::NotAvailable => {
                            // A sentinel, not a reading: `1.0` is `Cabs` of the
                            // `-1` both engines write here, so it is
                            // indistinguishable from a real 1 A / 1 V. The
                            // disambiguator is `seq_arm` itself — a consumer
                            // reads the arm, never the value (G1.3b audit
                            // settlement, 2026-09-05; pinned by
                            // `seq_currents_and_seq_voltages_are_one_on_the_na_arm`).
                            seq_currents.extend([1.0; 3]);
                            seq_voltages.extend([1.0; 3]);
                            seq_powers.extend([num_complex::Complex64::new(-1.0, 0.0); 3]);
                            // The same `-1` the two magnitude arrays report as
                            // `Cabs(-1 + 0j) = 1.0`, un-`Cabs`'d — and here
                            // BOTH engines spell it `(-1, 0)`, so no channel
                            // fold exists on this surface.
                            cplx_seq_currents.extend([num_complex::Complex64::new(-1.0, 0.0); 3]);
                            cplx_seq_voltages.extend([num_complex::Complex64::new(-1.0, 0.0); 3]);
                        }
                    }
                }
            }
            // `NodeOrder`: the same `GetNodeNum(NodeRef^[j])` walk the
            // `Export NodeOrder` renderer performs
            // (`report/export/node_order.rs:35-38`, Pascal `WriteNodeList`),
            // read here from the element's own `NodeRef` rather than by calling
            // that renderer — the export path is a frozen golden and must not be
            // touched (`GOLDEN_REBASE_PLAN.md` WP-G1: no golden byte moves).
            // `yorder == nterms · nconds` by construction (`elements/ckt.rs:326`),
            // which is the length both oracles allocate
            // (r4133 `DCktElement.pas:1043`, capi `CAPI_CktElement.pas:908`).
            //
            // Both engines answer this from `NodeRef` alone, with no `Enabled`
            // guard, so the empty answer here means exactly "no mapping yet":
            // it is the state where capi warns 15013 and returns its
            // `DefaultResult` (`CAPI_CktElement.pas:900-906`) and r4133, which has no
            // guard, dereferences the nil pointer at `:1048`. A `NodeRef`
            // shorter than `yorder` (a disabled element that grew phases —
            // see the `voltages_mag_ang` note above) reads its missing slots as
            // ground, the `GetNodeNum(0) = 0` answer
            // (r4133 `Common/Utilities.pas:1718`).
            let node_order: Vec<i32> = if cd.node_ref.is_empty() || cd.nterms == 0 {
                Vec::new()
            } else {
                (0..yorder)
                    .map(|i| {
                        let n = cd.node_ref.get(i).copied().unwrap_or(0);
                        ckt.map_node_to_bus.get(n).map_or(0, |m| m.node_num)
                    })
                    .collect()
            };
            // `EnergyMeter`: the metering meter's bare name, under upstream's
            // own `HasEnergyMeter` predicate (r4133 `DCktElement.pas:442-449`,
            // capi `CAPI_CktElement.pas:682-685`), with the `MeterObj`
            // back-pointer resolved through the pre-pass above instead of
            // dereferenced blind.
            let energy_meter = if cd
                .flags
                .contains(crate::elements::ckt::ElemFlags::HAS_ENERGY_METER)
            {
                cd.meter_obj.and_then(|m| meter_names.get(&m).cloned())
            } else {
                None
            };
            // The five control-derived scalars, all read off this element's
            // `ControlElementList` (derived above) with no `Enabled` filter
            // anywhere — neither oracle has one: r4133
            // `DDLL/DCktElement.pas:207-262` (`CktElementI` modes `7`-`11`) and
            // `Common/Utilities.pas:3165-3184`, capi
            // `CAPI/CAPI_CktElement.pas:689-988`. An element with no control has
            // no map entry (Pascal's empty list): `0`/`0`/`0`/false/false.
            let controls: &[ElemId] = control_lists.get(&r).map_or(&[], Vec::as_slice);
            let num_controls = controls.len();
            let has_switch_control = controls
                .iter()
                .any(|&c| control_category(c) == ControlCategory::Swt);
            let has_volt_control = controls.iter().any(|&c| {
                matches!(
                    control_category(c),
                    ControlCategory::Cap | ControlCategory::Reg
                )
            });
            // One scan serves both OCP scalars — upstream runs the same
            // "stop at the first Fuse/Recloser/Relay" loop twice, once returning
            // the 1-based position (`OCPDevIndex`) and once the class code
            // (`OCPDevType`), so they can only ever be zero together.
            let ocp = controls
                .iter()
                .position(|&c| control_category(c).is_ocp())
                .map(|p| (p + 1, control_category(controls[p]).ocp_code()));
            let (ocp_dev_index, ocp_dev_type) = ocp.unwrap_or((0, 0));
            out.push(ElementSnapshot {
                name,
                enabled: cd.enabled,
                bus_names,
                powers,
                currents,
                loss_w: (loss.re, loss.im),
                currents_mag_ang,
                voltages_mag_ang,
                residuals,
                n_terms: cd.nterms,
                n_conds: cd.nconds,
                n_phases: cd.nphases,
                node_order,
                energy_meter,
                phase_losses,
                num_controls,
                ocp_dev_index,
                ocp_dev_type,
                has_volt_control,
                has_switch_control,
                seq_arm,
                seq_currents,
                seq_voltages,
                seq_powers,
                cplx_seq_currents,
                cplx_seq_voltages,
                total_powers,
            });
        }
        // NCIM needs **no** reporting override here any more (RP3.13). Two used
        // to live at this point, both for the same reason: after an NCIM solve
        // two elements report a current the general `YPrim·V - Iinj` recompute
        // cannot produce — the swing `Vsource`, whose bus NCIM holds at the ideal
        // EMF so that recompute cancels to ~0 and whose true terminal current is
        // the Kirchhoff sum at the bus (Pascal `TVsourceObj.GetCurrents` takes
        // the `CalcInjCurrAtBus` branch when `Algorithm = NCIMSOLVE` and
        // `NodeRef[1] = 1`, r4133 `VSource.pas` l.1194), and a PV/converted
        // generator, whose `YPrim` (`Yeq`) is stale once NCIM has dispatched its
        // reactive power (`UpdateGenQ`, `Common/Solution.pas` l.2108/l.2305).
        // Overriding them here made this reader and the elements' own
        // `GetCurrents` — the CLI, `Export Powers`/`Currents`, `Show`, monitors,
        // meters — disagree, and only the one the corpus gate reads was right.
        // Both are now single live states: the generator's NCIM arm in
        // `TGeneratorObj.GetCurrents` (`generator/accessors.rs`, r4133
        // `PCElements/generator.pas` l.1406) and, for the swing source, a stamp
        // the solver leaves in `Iterminal` at the converged `NodeV`
        // (`solution::solution::ncim::ncim_stamp_swing_source_currents`) that
        // `TVsourceObj.GetCurrents`' NCIM arm echoes. So the `refresh_iterminal`
        // in the loop above already returns exactly what these overrides used to
        // compute — pinned lossless by
        // `exec::tests::ncim::ncim_gate_reader_and_ordinary_reader_agree` and
        // `exec::tests::ncim::ncim_vsource_export_currents_match_oracle`.
        out
    }

    /// Read a circuit element's dynamic/state variables — the dss-python
    /// `ActiveCktElement.AllVariableValues` surface (`TPCElement.GetAllVariables`).
    /// `name` is the element full name (`Class.name`, case-insensitive). Returns
    /// `None` if no such element exists; an empty vec for elements with no
    /// variables. The live f64 read the Monitor mode-3 f32 channel hides.
    pub fn element_variables(&mut self, name: &str) -> Option<Vec<f64>> {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_ref()?;
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        for class in classes.iter_mut() {
            let cn = class.props.class_name();
            for oi in 0..class.arena.len() {
                let obj = class.arena.obj_mut(oi);
                let full = format!("{}.{}", cn, obj.data().name());
                if full.eq_ignore_ascii_case(name) {
                    let elem = class.arena.try_ckt_elem_mut(oi)?;
                    let n = elem.num_variables();
                    let mut states = vec![0.0; n];
                    elem.get_all_variables(&sys, &node_v, &mut states);
                    return Some(states);
                }
            }
        }
        None
    }

    /// The 1-based variable NAMES of a circuit element — the dss-python
    /// `ActiveCktElement.AllVariableNames` surface (`TPCElement.VariableName`),
    /// aligned index-for-index with [`Dss::element_variables`]. `None` if no such
    /// element exists; an empty vec for elements with no variables. Used by the
    /// WASM user-model gate (`wasm_usermodels.rs`) to compare the state-variable
    /// surface by name against the r4133-oracle golden.
    pub fn element_variable_names(&mut self, name: &str) -> Option<Vec<String>> {
        let Dss { classes, .. } = self;
        for class in classes.iter_mut() {
            let cn = class.props.class_name();
            for oi in 0..class.arena.len() {
                let obj = class.arena.obj_mut(oi);
                let full = format!("{}.{}", cn, obj.data().name());
                if full.eq_ignore_ascii_case(name) {
                    let elem = class.arena.try_ckt_elem_mut(oi)?;
                    let n = elem.num_variables();
                    return Some((1..=n).map(|i| elem.variable_name(i)).collect());
                }
            }
        }
        None
    }

    /// WP8.5b corpus property parity: every property of the named element,
    /// rendered EXACTLY as the `?` executive query does (the choke-point
    /// `refresh_vterminal_if_marked` then [`ClassProps::get_value`] — the
    /// byte-proven WP8.5 Dump surface), as `(name, value)` pairs in
    /// property-index order (`1..=num_properties`). `full_name` is a `Class.name`
    /// (case-insensitive), resolved like [`Dss::do_query_cmd`] (no executive
    /// round-trip). `None` if no such element exists. The oracle side reads
    /// `Properties(p).Val` over `AllPropertyNames` (via `? name.prop`), so the two
    /// compare property-for-property.
    pub fn element_properties(&mut self, full_name: &str) -> Option<Vec<(String, String)>> {
        // Split `Class.name` exactly as `do_query_cmd` does (the @var-aware
        // splitter; a query name needs no other parser work).
        let (class_name, name) = {
            let mut p = Parser::new();
            parse_object_class_and_name(&mut p, &self.vars, full_name)
        };
        let &ci = self.class_by_name.get(&class_name.to_ascii_lowercase())?;
        if !self.classes[ci].set_active(&name) {
            return None;
        }
        let oi = self.classes[ci].active.expect("just set active");
        let n = self.classes[ci].props.num_properties();
        let mut out = Vec::with_capacity(n);
        for idx in 1..=n {
            // Same choke point `do_query_cmd` uses: reload Vterminal from the
            // solution for the properties that declare the need before rendering.
            self.refresh_vterminal_if_marked(ci, oi, Some(idx));
            let pname = self.classes[ci].props.property_name(idx).to_string();
            let value =
                self.classes[ci]
                    .props
                    .get_value(self.classes[ci].arena.obj(oi), idx, &self.enums);
            out.push((pname, value));
        }
        Some(out)
    }

    /// AltDSS single-object JSON dump — Pascal `Obj_ToJSON_`
    /// (`CAPI_Obj.pas:762-784`). `full_name` is a `Class.name` (case-insensitive,
    /// `@var`-aware like [`Dss::element_properties`]); `None` if no such object
    /// exists. `opts` selects the sweep (default filled-only vs `Full`), the key
    /// naming, and the compact/pretty layout.
    ///
    /// The default sweep dumps only *set* properties, which are all pure reads of
    /// the parsed model — safe before any solve. `Full` additionally renders
    /// read-only function properties; the handful flagged `READS_VTERMINAL`
    /// (Transformer/AutoTrans `WdgCurrents`) read the element's `Vterminal`
    /// cache. This `&self` method cannot run the `refresh_vterminal_if_marked`
    /// choke point, so it renders those from whatever the cache last held; for a
    /// `Full` dump that must reflect the current solution use
    /// [`Dss::obj_to_json_mut`], which refreshes first. No default-mode output is
    /// affected (no default-sweep property is `READS_VTERMINAL`).
    pub fn obj_to_json(&self, full_name: &str, opts: JsonOpts) -> Option<String> {
        let (class_name, name) = {
            let mut p = Parser::new();
            parse_object_class_and_name(&mut p, &self.vars, full_name)
        };
        let &ci = self.class_by_name.get(&class_name.to_ascii_lowercase())?;
        let &oi = self.classes[ci]
            .name_to_idx
            .get(&name.to_ascii_lowercase())?;
        let json = json_build::obj_to_json_data(
            &self.classes[ci].props,
            self.classes[ci].arena.obj(oi),
            &self.enums,
            opts,
        );
        Some(json_serialize(&json, opts))
    }

    /// Like [`Dss::obj_to_json`] but refreshes the element's live-state caches
    /// (`refresh_vterminal_if_marked`) before rendering, so `Full`-mode
    /// `READS_VTERMINAL` result properties (Transformer/AutoTrans `WdgCurrents`)
    /// render from the current solution exactly as Pascal's self-refreshing
    /// getter does (`CAPI_Obj.pas` via `TTransfObj.GetAllWindingCurrents`). This
    /// is the JSON counterpart of the `?`/`Dump` refresh at
    /// [`Dss::element_properties`]. Pre-solve `Vterminal` is zeros, so the string
    /// is the all-zero phasor list; after a solve it reflects the winding
    /// currents. (Capacitor `CMatrix` under `Full` is *not* closed by this: the
    /// oracle renders it from an uninitialized buffer — proven nondeterministic
    /// across processes — so it is a UB non-port, not a refresh gap.)
    pub fn obj_to_json_mut(&mut self, full_name: &str, opts: JsonOpts) -> Option<String> {
        let (class_name, name) = {
            let mut p = Parser::new();
            parse_object_class_and_name(&mut p, &self.vars, full_name)
        };
        let &ci = self.class_by_name.get(&class_name.to_ascii_lowercase())?;
        let &oi = self.classes[ci]
            .name_to_idx
            .get(&name.to_ascii_lowercase())?;
        self.refresh_vterminal_if_marked(ci, oi, None);
        let json = json_build::obj_to_json_data(
            &self.classes[ci].props,
            self.classes[ci].arena.obj(oi),
            &self.enums,
            opts,
        );
        Some(json_serialize(&json, opts))
    }

    /// AltDSS class-batch JSON dump — Pascal `Batch_ToJSON` over every object of
    /// a class (the `IActiveClass.ToJSON` oracle surface, `CAPI_Obj.pas:1201-
    /// 1254`). `class` is the class name (case-insensitive); `None` if unknown.
    /// An empty class serializes to `[]`.
    pub fn class_batch_to_json(&self, class: &str, opts: JsonOpts) -> Option<String> {
        let &ci = self.class_by_name.get(&class.to_ascii_lowercase())?;
        let json = json_build::batch_to_json(
            &self.classes[ci].props,
            &self.classes[ci].arena,
            &self.enums,
            opts,
        );
        Some(json_serialize(&json, opts))
    }

    /// Like [`Dss::class_batch_to_json`] but refreshes every object's live-state
    /// caches first (see [`Dss::obj_to_json_mut`]) so a `Full`-mode batch of a
    /// `READS_VTERMINAL` class (Transformer/AutoTrans) renders `WdgCurrents` from
    /// the current solution. A no-op for classes with no such property.
    pub fn class_batch_to_json_mut(&mut self, class: &str, opts: JsonOpts) -> Option<String> {
        let &ci = self.class_by_name.get(&class.to_ascii_lowercase())?;
        let n = self.classes[ci].arena.len();
        for oi in 0..n {
            self.refresh_vterminal_if_marked(ci, oi, None);
        }
        let json = json_build::batch_to_json(
            &self.classes[ci].props,
            &self.classes[ci].arena,
            &self.enums,
            opts,
        );
        Some(json_serialize(&json, opts))
    }

    /// AltDSS JSON-schema export — Pascal `DSS_ExtractSchema(DSS,
    /// jsonSchema=True)` (`CAPI_Schema.pas:1252-1521`). Emits the full
    /// JSON-Schema (draft 2020-12) document: the envelope, the ten reusable
    /// global `$defs`, the 21 global enum `$defs`, and — in
    /// [`schema::DSS_CLASS_LIST_ORDER`](crate::report::export::json::schema::
    /// DSS_CLASS_LIST_ORDER) — every class's `$defs/<Class>` (via the per-class
    /// walk [`Dss::schema_class_def`]) with its `<Class>List`/`<Class>Container`
    /// triple and its `circuitProperties` container ref.
    ///
    /// The document is byte-gated vs the pinned 0.14.5 oracle (49 classes) after
    /// the documented r4133 divergences; the 50th class WindGen and the four
    /// r4133-restructured classes (Relay/Recloser/SwtControl/LineGeometry) are
    /// port-authored (`golden_schema.rs`). The result is independent of circuit
    /// state (all constant), so it needs no `&mut self` and no `New circuit`.
    pub fn extract_schema_json(&self) -> String {
        let doc = self.schema_document();
        let mut out = String::new();
        crate::report::export::json::write_pretty(&doc, 0, &mut out);
        out
    }

    /// The schema document as a [`Json`](crate::report::export::json::Json)
    /// tree, before it is spelled out — what [`Dss::extract_schema_json`]
    /// renders.
    ///
    /// Separate from the rendering step because the *document* is what is
    /// contractual (envelope, `$defs` order, per-class blocks, ordinals) while
    /// how its numbers and line breaks are spelled is the F-FMT seam's lane
    /// choice (`compat::json_float`, `compat::JSON_LINE_BREAK`). A consumer that
    /// needs a specific spelling — `golden_schema.rs`'s byte gate needs the
    /// oracle's — renders this tree with
    /// [`write_pretty_with`](crate::report::export::json::write_pretty_with).
    pub fn schema_document(&self) -> crate::report::export::json::Json {
        use crate::report::export::json::schema;
        // Build the per-class `$defs/<Class>` list in `DSS.DSSClassList` order
        // (Pascal `CAPI_Schema.pas:1479`), keyed by the class's canonical name.
        let class_defs: Vec<(String, crate::report::export::json::Json)> =
            schema::DSS_CLASS_LIST_ORDER
                .iter()
                .map(|&name| {
                    let &ci = self
                        .class_by_name
                        .get(&name.to_ascii_lowercase())
                        .unwrap_or_else(|| panic!("schema class `{name}` not registered"));
                    let class = &self.classes[ci];
                    let key = class.props.class_name().to_string();
                    (key, self.schema_class_def(name).expect("class def"))
                })
                .collect();
        // Completeness guard. Pascal walks the *live* `DSS.DSSClassList`
        // (`CAPI_Schema.pas:1479`); the port drives the walk from the pinned
        // static `DSS_CLASS_LIST_ORDER`, so nothing structurally forces the list
        // to stay in sync with the registry. Cross-check the two: every listed
        // class is already proven registered above, so equal counts make the list
        // ⊇ registry a bijection — a class registered without being added to the
        // walk order (which would be silently omitted from the schema) trips this.
        assert_eq!(
            class_defs.len(),
            self.classes.len(),
            "schema walk order (DSS_CLASS_LIST_ORDER, {}) does not cover every \
             registered class ({}): a class was registered without being added to \
             DSS_CLASS_LIST_ORDER and would be silently omitted from the schema",
            class_defs.len(),
            self.classes.len(),
        );
        schema::assemble_full_document(&class_defs)
    }

    /// The schema `$defs/<Class>` for one registered class — Pascal
    /// `prepareClassJsonSchema` (`CAPI_Schema.pas:325-1134`), built from the class
    /// property table and a fresh all-default *sample object*
    /// (`cls.NewObject('SAMPLE_FOR_DEFAULTS', ...)`). Returns `None` if the class
    /// is not registered. Groundwork for the full-document assembly (the
    /// `<Class>List`/`<Class>Container` triples + `circuitProperties` refs, and
    /// the loop over `DSSClassList` order, are added by the caller); exercised
    /// per-class by `golden_schema.rs`.
    pub fn schema_class_def(&self, class_name: &str) -> Option<crate::report::export::json::Json> {
        let &ci = self.class_by_name.get(&class_name.to_ascii_lowercase())?;
        let class = &self.classes[ci];
        // The sample lives in a throwaway one-object arena of this class, so it is
        // reached through the same typed `ClassArena` channel as every registered
        // object (`push_new` builds the very `<T>::new(name)` the registry's own
        // constructor did).
        let mut sample = crate::obj::arena::ClassArena::empty_for(class.props.class_name())
            .expect("registered class has an arena");
        let si = sample.push_new("sample_for_defaults");
        // Pascal `cls.NewObject('SAMPLE_FOR_DEFAULTS')` runs `RecalcElementData` in
        // Create (reading the fresh `ActiveCircuit.Solution` — the parse-time
        // default here, no circuit exists in this defaults-introspection path), so
        // the sampled property defaults match the oracle. The PC `new` no longer
        // recalcs, so seed the sample the same way the executive does at create.
        super::command::recalc_pc_create(
            &mut sample,
            si,
            &crate::elements::traits::SysCtx::parse_default(),
        );
        Some(crate::report::export::json::schema::class_schema(
            class.props.class_name(),
            &class.props,
            sample.obj(si),
            &self.enums,
        ))
    }

    /// AltDSS whole-circuit JSON dump — Pascal `Obj_Circuit_ToJSON_`
    /// (`CAPI_Obj.pas:2513-2672`). Returns `None` when no circuit exists
    /// (`New circuit.` has not run). The circuit is **always** serialized pretty
    /// (`FormatJSON()`, indent 2), regardless of `opts.PRETTY`; `opts` selects the
    /// timestamp/bus/default-object filtering and the per-object sweep (default
    /// vs `Full`). Every embedded object is rendered by the same
    /// `obj_to_json_data` as [`Dss::obj_to_json`].
    pub fn circuit_to_json(&self, opts: JsonOpts) -> Option<String> {
        let ckt = self.circuit.as_ref()?;
        Some(json_circuit::circuit_to_json(
            ckt,
            &self.classes,
            &self.class_by_name,
            &self.enums,
            self.default_base_freq,
            self.default_earth_model,
            opts,
        ))
    }

    /// Read a bus's short-circuit results after a FaultStudy solve — the
    /// dss-python `Bus.Zsc1`/`Zsc0`/`Isc` surface. `name` is the bus name
    /// (case-insensitive). `None` if no such bus exists.
    pub fn bus_short_circuit(&self, name: &str) -> Option<BusScView> {
        let ckt = self.circuit.as_ref()?;
        let idx = ckt.bus_list.find(name)?;
        let b = &ckt.buses[idx];
        Some(BusScView {
            nodes: b.nodes.clone(),
            zsc1: b.get_zsc1(),
            zsc0: b.get_zsc0(),
            isc: b.bus_current.clone(),
            vbus: b.vbus.clone(),
        })
    }

    /// Read a monitor's recorded data — the dss-python `Monitors.Header` /
    /// `SampleCount` / `Channel(i)` / `dblHour` surface (tests/goldens).
    /// `name` may be `"m1"` or `"Monitor.m1"` (case-insensitive). `None` if no
    /// such monitor exists.
    pub fn monitor_view(&self, name: &str) -> Option<MonitorView> {
        let bare = name
            .strip_prefix("Monitor.")
            .or_else(|| name.strip_prefix("monitor."))
            .unwrap_or(name);
        for class in &self.classes {
            for m in class.arena.all::<monitor::Monitor>().unwrap_or(&[]) {
                if m.med.cd.obj.name().eq_ignore_ascii_case(bare) {
                    let nch = m.num_channels();
                    return Some(MonitorView {
                        header: m.header().to_vec(),
                        sample_count: m.sample_count(),
                        dbl_hour: m.dbl_hour(),
                        channels: (1..=nch).map(|i| m.channel(i)).collect(),
                        flushed_records: m.flushed_records(),
                    });
                }
            }
        }
        None
    }

    /// An EnergyMeter's zone topology — the dss-python `Meters.AllBranchesInZone`
    /// / `AllEndElements` / `ZonePCE` / `CountBranches` surface (tests/goldens).
    /// `name` may be `"m1"` or `"EnergyMeter.m1"` (case-insensitive). `None` if
    /// no such meter exists.
    pub fn meter_zone(&self, name: &str) -> Option<MeterZoneView> {
        let bare = name
            .strip_prefix("EnergyMeter.")
            .or_else(|| name.strip_prefix("energymeter."))
            .unwrap_or(name);
        // Locate the meter, copy out its ElemId lists, then resolve full names.
        let mut lists: Option<MeterZoneRefs> = None;
        for class in &self.classes {
            for em in class.arena.all::<energymeter::EnergyMeter>().unwrap_or(&[]) {
                if em.data().name().eq_ignore_ascii_case(bare) {
                    lists = Some(MeterZoneRefs {
                        branches: em.sequence_list().to_vec(),
                        ends: em.zone_end_elements(),
                        pce: em.zone_pce().to_vec(),
                        register_names: em.register_names().to_vec(),
                    });
                }
            }
        }
        let lists = lists?;
        let full_name = |r: ElemId| -> String {
            let cn = self.classes[r.class_ord()].props.class_name();
            format!(
                "{}.{}",
                cn,
                self.classes[r.class_ord()].arena[r.index()].data().name()
            )
        };
        Some(MeterZoneView {
            all_branches_in_zone: lists.branches.iter().map(|&r| full_name(r)).collect(),
            all_end_elements: lists.ends.iter().map(|&r| full_name(r)).collect(),
            zone_pce: lists.pce.iter().map(|&r| full_name(r)).collect(),
            register_names: lists.register_names,
        })
    }

    /// An EnergyMeter's register values paired with names — the dss-python
    /// `Meters.RegisterValues` / `RegisterNames` surface (tests/goldens).
    /// `name` may be `"m1"` or `"EnergyMeter.m1"` (case-insensitive).
    pub fn meter_registers(&self, name: &str) -> Option<Vec<(String, f64)>> {
        let bare = name
            .strip_prefix("EnergyMeter.")
            .or_else(|| name.strip_prefix("energymeter."))
            .unwrap_or(name);
        for class in &self.classes {
            for em in class.arena.all::<energymeter::EnergyMeter>().unwrap_or(&[]) {
                if em.data().name().eq_ignore_ascii_case(bare) {
                    return Some(
                        em.register_names()
                            .iter()
                            .cloned()
                            .zip(em.registers().iter().copied())
                            .collect(),
                    );
                }
            }
        }
        None
    }

    /// A load's `(kWbase, FAllocationFactor)` by name — the oracle's
    /// `Loads.kW` / `Loads.AllocationFactor` (test API for `allocateloads`).
    pub fn load_alloc(&self, name: &str) -> Option<(f64, f64)> {
        for class in &self.classes {
            for ld in class.arena.all::<load::Load>().unwrap_or(&[]) {
                if ld.data().name().eq_ignore_ascii_case(name) {
                    return Some((ld.kw_base, ld.allocation_factor()));
                }
            }
        }
        None
    }

    /// A generator's `(kWbase, kvarBase)` by name — the oracle's
    /// `Generators.kW` / `Generators.kvar` (test API for GenDispatcher
    /// redispatch).
    pub fn generator_kw_kvar(&self, name: &str) -> Option<(f64, f64)> {
        for class in &self.classes {
            for g in class.arena.all::<generator::Generator>().unwrap_or(&[]) {
                if g.data().name().eq_ignore_ascii_case(name) {
                    return Some((g.kw_base, g.kvar_base));
                }
            }
        }
        None
    }

    /// The named generator's *present* (solved) `(kW, kvar)` output — Pascal
    /// `Get_PresentkW`/`Get_Presentkvar` (`Pnominalperphase`/`Qnominalperphase`
    /// times `nphases/1000`). Unlike [`Self::generator_kw_kvar`] (the input
    /// bases), this reflects the solved per-phase power, so under NCIM it tracks
    /// the PV-bus `deltaQNom` — the channel dss-python's `Generators.kvar` reads.
    pub fn generator_present_kw_kvar(&self, name: &str) -> Option<(f64, f64)> {
        for class in &self.classes {
            for g in class.arena.all::<generator::Generator>().unwrap_or(&[]) {
                if g.data().name().eq_ignore_ascii_case(name) {
                    return Some((g.present_kw(), g.present_kvar()));
                }
            }
        }
        None
    }

    /// Test API for Pascal `TSensorObj.TakeSample`: drive the named sensor
    /// against the solved circuit and return its `(CalculatedCurrent,
    /// CalculatedVoltage)` per phase. (`TakeSample` is otherwise dead in the
    /// snapshot path — `SensorClass.SampleAll` is only invoked by the
    /// state-estimation API, which is a later phase — so this is the only gate
    /// that exercises the offset/`RotatePhases` math.)
    pub fn sensor_sample(
        &mut self,
        name: &str,
    ) -> Option<(Vec<num_complex::Complex64>, Vec<num_complex::Complex64>)> {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_ref()?;
        let sensor_ref = ckt.sensors.iter().copied().find(|r| {
            classes[r.class_ord()].arena[r.index()]
                .data()
                .name()
                .eq_ignore_ascii_case(name)
        })?;
        let metered = classes[sensor_ref.class_ord()]
            .arena
            .get::<sensor::Sensor>(sensor_ref.index())?
            .metered_element()?;
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let mut store = ClassStore { classes };
        let (s, m_obj) = store.typed_ckt_pair_mut::<sensor::Sensor>(sensor_ref, metered);
        let m_ce = m_obj.expect("metered element is a circuit element");
        s.take_sample(m_ce, &sys, &node_v);
        let nph = s.med.cd.nphases;
        Some((
            s.med.calculated_current[..nph].to_vec(),
            s.med.calculated_voltage[..nph].to_vec(),
        ))
    }

    /// Per-transformer winding taps in creation order, keyed by name —
    /// mirrors the oracle's `Transformers` loop (`tr.Wdg = i; tr.Tap`).
    pub fn transformer_taps(&self) -> Vec<(String, Vec<f64>)> {
        let Some(ckt) = self.circuit.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(ckt.transformers.len());
        for &r in &ckt.transformers {
            let obj = &self.classes[r.class_ord()].arena[r.index()];
            let tr = self.classes[r.class_ord()]
                .arena
                .get::<transformer::Transformer>(r.index())
                .expect("transformers list holds Transformers");
            // The oracle's `Transformers.First/.Next` walk
            // (`Generic_CktElement_Get_First/Next`) SKIPS disabled elements
            // unless `DSS_CAPI_ITERATE_DISABLED = 1` (default 0); mirror that, or
            // a deck that disables a transformer (e.g. `MakePosSequence`'s
            // off-phase-1 winding disable, `makeposseq_xfmr.dss`) compares one
            // extra tap row vs the oracle. Same rule as `regcontrol_tap_numbers`.
            if !tr.cd().enabled {
                continue;
            }
            let n = tr.num_windings() as usize;
            let taps = (1..=n).map(|w| tr.present_tap(w)).collect();
            out.push((obj.data().name().to_string(), taps));
        }
        out
    }

    /// Per-RegControl `TapNumber` in creation order (the oracle's
    /// `RegControls` API).
    pub fn regcontrol_tap_numbers(&self) -> Vec<(String, i32)> {
        let Some(ckt) = self.circuit.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &r in &ckt.controls {
            let obj = &self.classes[r.class_ord()].arena[r.index()];
            let Some(rc) = self.classes[r.class_ord()]
                .arena
                .get::<reg_control::RegControl>(r.index())
            else {
                continue;
            };
            // The oracle's `RegControls.First/.Next` iterator SKIPS disabled
            // control elements (C-API `Get_First`/`Get_Next` walk the list with
            // `if pelem.Enabled`; verified live — a disabled RegControl yields
            // `First = 0`). Mirror that, or a deck that opens with
            // `BatchEdit RegControl..* enabled=False` compares 12 taps vs 0.
            if !rc.ccd.cd.enabled {
                continue;
            }
            // Pascal `Get_TapNum` reads the controlled transformer's *live*
            // `PresentTap[TapWinding]`; resolve it here so a direct
            // `Transformer.X.Taps=` edit (which bypasses the control's snapshot)
            // is reflected. Fall back to the cached `TapNum` if unresolved.
            let num = rc
                .controlled_ref()
                .and_then(|tref| {
                    // Either member of the Transformer/AutoTrans proxy.
                    self.classes[tref.class_ord()]
                        .arena
                        .try_controlled_transformer(tref.index())
                        .map(|tr| rc.tap_num_live(tr))
                })
                .unwrap_or_else(|| obj.get_i32(reg_control::prop::TAPNUM));
            out.push((obj.data().name().to_string(), num));
        }
        out
    }

    /// Per-capacitor `States` array in creation order (the oracle's
    /// `Capacitors` API — every Capacitor object, not just the shunt list).
    pub fn capacitor_states(&self) -> Vec<(String, Vec<i32>)> {
        let Some(cls) = self
            .classes
            .iter()
            .find(|c| c.props.class_name().eq_ignore_ascii_case("Capacitor"))
        else {
            return Vec::new();
        };
        cls.arena
            .objs()
            .map(|obj| {
                let states = obj
                    .get_i32_array(capacitor::prop::STATES)
                    .map(|s| s.to_vec())
                    .unwrap_or_default();
                (obj.data().name().to_string(), states)
            })
            .collect()
    }

    /// Terminal-1 closed flag of a capacitor by name (test API for the `Reset`
    /// controls path: `CapControl.Reset` drives the bank back to `InitialState`
    /// via `ControlledElement.Closed[0]`).
    pub fn capacitor_closed(&self, name: &str) -> Option<bool> {
        let cls = self
            .classes
            .iter()
            .find(|c| c.props.class_name().eq_ignore_ascii_case("Capacitor"))?;
        (0..cls.arena.len())
            .find(|&i| cls.arena.obj(i).data().name().eq_ignore_ascii_case(name))
            .and_then(|i| cls.arena.try_ckt_elem(i))
            .map(|e| e.cd().all_conductors_closed())
    }

    /// CAPI `Circuit_Get_TotalPower`: the sum of every source's terminal-1
    /// power, in kW/kvar (negative of the power delivered to the circuit).
    pub fn total_power(&mut self) -> (f64, f64) {
        use num_complex::Complex64;
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_ref().expect("total_power needs a circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let positive_seq = ckt.positive_sequence;
        let mut total = Complex64::ZERO;
        for &r in &ckt.sources {
            let elem = classes[r.class_ord()]
                .arena
                .try_ckt_elem_mut(r.index())
                .expect("sources are circuit elements");
            if !elem.cd().enabled || elem.cd().node_ref.is_empty() {
                continue;
            }
            elem.compute_iterminal(&sys, &node_v);
            let cd = elem.cd();
            // Pascal Get_Power(1): sum over terminal-1 conductors.
            let mut s = Complex64::ZERO;
            for i in 0..cd.nconds {
                let n = cd.node_ref[i];
                if n > 0 {
                    s += node_v[n] * cd.iterminal[i].conj();
                }
            }
            if positive_seq {
                s *= 3.0;
            }
            total += s;
        }
        (total.re * 0.001, total.im * 0.001)
    }

    /// CAPI `Circuit_Get_Losses`: total circuit losses in W/var (sum over
    /// enabled PD elements).
    pub fn losses(&mut self) -> (f64, f64) {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_mut().expect("losses needs a circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let mut store = ClassStore { classes };
        let total = ckt.losses(&mut store, &sys);
        (total.re, total.im)
    }

    /// `DSS.ActiveCircuit.Solution.EventLog`: the accumulated event-log lines
    /// (empty when no circuit exists). The control loop (WP5.7) and the
    /// controls' `AppendToEventLog` (WP5.5/5.6) populate it.
    pub fn event_log(&self) -> &[String] {
        match &self.circuit {
            Some(ckt) => ckt.solution.event_log.entries(),
            None => &[],
        }
    }

    /// The pending control-action queue as the dss-python `CtrlQueue.Queue`
    /// rows (Pascal `TControlQueue.QueueItem`, `ControlQueue.pas:557`:
    /// `Format('%d, %d, %.9g, %d, %d, %s ', [handle, hour, sec, code, proxy,
    /// ControlElement.Name])` — bare device name, trailing space). Empty after
    /// a drained snapshot; time/dynamics modes leave future-scheduled actions
    /// (e.g. recloser reclose shots) pending between steps.
    pub fn control_queue_rows(&self) -> Vec<String> {
        let Some(ckt) = &self.circuit else {
            return Vec::new();
        };
        ckt.solution
            .control_queue
            .queue_rows()
            .into_iter()
            .map(|(handle, hour, sec, code, proxy, ctrl)| {
                let name = self.classes[ctrl.class_ord()].arena[ctrl.index()]
                    .data()
                    .name();
                format!(
                    "{handle}, {hour}, {}, {code}, {proxy}, {name} ",
                    crate::util::fmt_g(sec, 9)
                )
            })
            .collect()
    }

    /// Coordinate dump of the **assembled, unfactored** system Y matrix:
    /// `(n, [(row, col, value)])`, 0-based, where row `i` corresponds to node
    /// `i + 1` (so `node_name(row + 1)` names the row). The values are
    /// pre-equilibration — the matrix exactly as stamped from element YPrims —
    /// so they line up with the oracle's `YMatrix.getYSparse(factor=False)`.
    /// `None` if no system Y has been built. Test/golden API (the assembled-model
    /// checkpoint of `golden_checkpoints.rs`).
    ///
    /// Reads the **active** handle (`Solution.active_y`), modelling Pascal's `hY`
    /// pointer: `BuildYMatrix` sets `hY := hYsystem` for `WHOLEMATRIX` and `hY :=
    /// hYseries` for `SERIESONLY`/`PDE_ONLY` (`YMatrix.pas` l.394/402), and
    /// `getYSparse` reads that pointer. So after an NCIM solve (`PDE_ONLY`) the
    /// reported system Y is the PDE-only network Y — no load `Yeq` — exactly as
    /// the oracle reports it.
    pub fn system_y_csc(&mut self) -> Option<SystemYCsc> {
        use crate::solution::solution::ActiveY;
        let ckt = self.circuit.as_mut()?;
        let y = match ckt.solution.active_y {
            ActiveY::System => ckt.solution.y_system.as_mut(),
            ActiveY::Series => ckt.solution.y_series.as_mut(),
        }?;
        let n = y.size();
        let (rows, cols, vals) = y.coo_entries().ok()?;
        let coords = rows
            .into_iter()
            .zip(cols)
            .zip(vals)
            .map(|((r, c), v)| (r, c, v))
            .collect();
        Some((n, coords))
    }

    /// An element's primitive admittance matrix `Yprim` as a **column-major**
    /// `yorder × yorder` flat array (`out[col * yorder + row]`) — the exact
    /// layout of the oracle's `CktElement.Yprim` (Pascal `TcMatrix`, column-major)
    /// and of `CMatrix`'s own storage, so the two compare without any transpose.
    /// `name` is a full `Class.name` (e.g. `"Transformer.reg1"`) when it
    /// contains a dot, else a bare object name matched across all classes
    /// (case-insensitive). `None` if no such element exists or it has no Yprim.
    /// Test/golden API (the selected-element checkpoint of `golden_checkpoints.rs`).
    pub fn element_yprim(&self, name: &str) -> Option<(usize, Vec<num_complex::Complex64>)> {
        let want_full = name.contains('.');
        for class in &self.classes {
            let cn = class.props.class_name();
            for i in 0..class.arena.len() {
                let obj = class.arena.obj(i);
                let matches = if want_full {
                    format!("{}.{}", cn, obj.data().name()).eq_ignore_ascii_case(name)
                } else {
                    obj.data().name().eq_ignore_ascii_case(name)
                };
                if !matches {
                    continue;
                }
                let Some(ce) = class.arena.try_ckt_elem(i) else {
                    continue;
                };
                let cd = ce.cd();
                let yorder = cd.yorder;
                let yprim = cd.yprim.as_ref()?;
                let mut out = Vec::with_capacity(yorder * yorder);
                for col in 0..yorder {
                    for row in 0..yorder {
                        out.push(yprim.get(row, col));
                    }
                }
                return Some((yorder, out));
            }
        }
        None
    }

    /// The node injection-current vector the solver last used (`Solution.Currents`,
    /// the RHS of `Y·V = I`), length `num_nodes + 1` with slot 0 = ground —
    /// the oracle's `YMatrix.getI()` surface. Empty when no circuit exists.
    /// Test/golden API.
    pub fn node_injection_currents(&self) -> Vec<num_complex::Complex64> {
        match &self.circuit {
            Some(ckt) => ckt.solution.currents.clone(),
            None => Vec::new(),
        }
    }
}
