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
}

/// `(n, [(row, col, value)])` — the assembled, unfactored system Y as 0-based
/// coordinates, returned by [`Dss::system_y_csc`].
pub type SystemYCsc = (usize, Vec<(usize, usize, num_complex::Complex64)>);

/// A bus's short-circuit results — the dss-python / COM `Bus.Zsc1`, `Zsc0`,
/// `ZscMatrix`, `YscMatrix`, `Isc` and `Voc` surface. The fastdss harness dumps
/// all six with the rest of `IBus._columns` (`dss/IBus.py:30`/`:39`/`:41-44`,
/// walked from `tests/save_outputs.py:350` on `origin/fastdss`).
///
/// Pascal, per field — r4133 `Version8/Source/DDLL/DBus.pas`, `BUSV` modes
/// 3 `Voc` (`:351-372`), 4 `Isc` (`:374-397`), 6 `ZscMatrix` (`:431-459`),
/// 7 `Zsc1` (`:461-475`), 8 `Zsc0` (`:476-490`), 9 `YscMatrix` (`:491-518`);
/// capi `src/CAPI/CAPI_Alt.pas`, `Alt_Bus_Get_Voc` (`:2227-2249`), `_Isc`
/// (`:2202-2224`), `_ZscMatrix` (`:2305-2334`), `_Zsc1` (`:2294-2303`),
/// `_Zsc0` (`:2283-2292`), `_YscMatrix` (`:2336-2365`). `Zsc1`/`Zsc0` are
/// `TDSSBus.Get_Zsc1` = `Zs − Zm` and `Get_Zsc0` = `Zs + 2·Zm` over `Zsc`'s
/// averaged diagonal/off-diagonal (`Common/Bus.pas:222-229` / `:215-220`,
/// capi identical).
///
/// **Ordering — convention 2, not convention 1.** Every array here is indexed
/// by the bus's *internal* node index: the oracles read `GetRef(i)` /
/// `Zsc.GetElement(i, j)` straight off `TDSSBus`, so the index is
/// `TDSSBus.Nodes`' **insertion** order — the order [`Self::nodes`] reports.
/// [`BusVoltageView`]'s arrays are ordered by ascending node *number* instead
/// (convention 1, the `repeat FindIdx(jj)` walk of `CAPI_Alt.pas:2270-2275` ==
/// `DBus.pas:415-421`, which is also what `Bus.Nodes` itself publishes —
/// `Alt_Bus_Get_Nodes`, `CAPI_Alt.pas:2143-2163`), and `YNodeOrder` is a third
/// permutation. The three must never be mixed. Measured, not assumed: on
/// `line.l2 bus1=b1.1.2.3 bus2=b2.2.1.3` with a single 1-phase shunt on node 1,
/// both oracles put the odd `Zsc` diagonal at index **1** — node 1's insertion
/// slot — and not at index 0 (pinned by
/// `the_short_circuit_arrays_are_indexed_by_internal_node_index`).
///
/// `zsc`/`ysc` stay `None` until a FaultStudy solve (or `ZscRefresh`) runs
/// `AllocateBusQuantities`; both oracles publish a one-entry `CZero`/default
/// sentinel in that state, so "no matrix" is a *shape*, never a zero matrix.
/// `isc`/`vbus` are different: `ReProcessBusDefs` allocates and zeroes them for
/// every bus (`Common/Circuit.pas:2407-2408` == `circuit::Circuit::reprocess_bus_defs`), so
/// after the first `BuildYMatrix` they are always `NumNodesThisBus` long.
#[derive(Debug, Clone)]
pub struct BusScView {
    /// The bus's (lowercased) name, `Circuit.AllBusNames` spelling.
    pub name: String,
    /// User node numbers on the bus (`Nodes`), **insertion** order — the index
    /// every other field of this view is in. (`Bus.Nodes` on both oracles is
    /// the same set sorted ascending; the harness sorts before comparing.)
    pub nodes: Vec<i32>,
    /// `Zsc1`: positive-sequence short-circuit impedance.
    pub zsc1: num_complex::Complex64,
    /// `Zsc0`: zero-sequence short-circuit impedance.
    pub zsc0: num_complex::Complex64,
    /// `ZscMatrix`: the node-frame short-circuit impedance matrix, flattened
    /// **row-major** (`i` outer, `j` inner) — see [`flatten_row_major`].
    /// `None` until a FaultStudy allocates it.
    pub zsc: Option<Vec<num_complex::Complex64>>,
    /// `YscMatrix` = `Zsc⁻¹`: `Ysc.CopyFrom(Zsc); Ysc.invert`
    /// (`Common/SolutionAlgs.pas:828-829`, inside `ComputeYsc` `:800-832` ==
    /// `solution/solution/fault_study.rs::compute_ysc`). Same row-major
    /// flattening and the same `None` rule as [`Self::zsc`].
    pub ysc: Option<Vec<num_complex::Complex64>>,
    /// `Isc` / `BusCurrent`: per-node short-circuit current (= `Ysc · VBus`).
    pub isc: Vec<num_complex::Complex64>,
    /// The bus's stored `VBus` — `Bus.Voc`, the open-circuit voltage captured by
    /// `UpdateVBus`. Note this is **not** dss-python `Bus.Voltages`, which
    /// returns the live `NodeV` (after a FaultStudy that is the last
    /// `ComputeYsc` unit-injection residual, not the Voc). `UpdateVBus` also
    /// runs from `BuildYMatrix` under `PreserveNodeVoltages`
    /// (`Common/Ymatrix.pas:170` == `solution::ymatrix::update_vbus`), so `vbus` is
    /// live on harmonics/dynamics decks that never ran a study.
    pub vbus: Vec<num_complex::Complex64>,
}

/// The lower-cased `Class.name` of every summand the four scalar circuit
/// aggregates walk — the membership behind [`Dss::losses`],
/// [`Dss::line_losses`], [`Dss::substation_losses`] and [`Dss::total_power`].
///
/// The live corpus gate reconstructs each oracle aggregate over these names out
/// of the oracle's **own** per-element capture (`GOLDEN_REBASE_PLAN.md` G1.9,
/// arm P1), and the expected-value pins in `exec::tests::aggregates` assert them
/// directly — so a wrongly included or omitted summand surfaces as a
/// *membership* error (one whole element's loss) instead of hiding inside a
/// blurred sum.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AggregateTerms {
    /// `Circuit.Losses` (`CAPI_Circuit.pas:171-186` → r4133
    /// `Common/Circuit.pas:2428-2445`): the `PDElements` that are enabled and
    /// not shunt.
    pub losses: Vec<String>,
    /// `Circuit.LineLosses` (`CAPI_Circuit.pas:145-162`, r4133
    /// `DDLL/DCircuit.pas:305-325`): every `Lines` entry, unfiltered.
    pub line_losses: Vec<String>,
    /// `Circuit.SubstationLosses` (`CAPI_Circuit.pas:289-307`, r4133
    /// `DDLL/DCircuit.pas:327-347`): the `Transformers` entries with `sub=yes`.
    /// `AutoTrans` objects are registered on the separate `AutoTransformers`
    /// list (`Common/Circuit.pas:2272-2273`) and therefore never appear here,
    /// whatever their own `sub=` says.
    pub substation_losses: Vec<String>,
    /// `Circuit.TotalPower` (`CAPI_Circuit.pas:316-338`, r4133
    /// `DDLL/DCircuit.pas:349-368`): every `Sources` entry, unfiltered.
    pub total_power: Vec<String>,
}

// ---------------------------------------------------------------------------
// GOLDEN_REBASE G1.7 — the topology interface (`ITopology`)
// ---------------------------------------------------------------------------

/// The circuit's connected-branch topology: the six **order-free** `ITopology`
/// quantities the live corpus gate compares, returned by
/// [`Dss::topology_view`].
///
/// Surface: the fastdss harness column set `ITopology._columns`
/// (`.inputs/DSS-Python` `origin/fastdss:dss/ITopology.py:10-20`) **minus** its
/// three cursor fields `ActiveLevel` / `BranchName` / `ActiveBranch`. Those are
/// deliberately not part of this view: reading them reassigns
/// `ActiveCircuit.ActiveCktElement` (r4133 `Version8/Source/DDLL/DTopology.pas:42-53`
/// `ActiveBranch`, `:96-160` the `First`/`Next`/`ForwardBranch` cursor modes,
/// `:170-186` `TopologyS`), which would corrupt the per-element capture the same
/// checkpoint takes.
///
/// Upstream answers all six from a **memoized** `Branch_List`, built on the first
/// read and freed only in `Destroy` and `DoResetMeterZones` (r4133
/// `Common/Circuit.pas:2932-2950`, `:703`, `:2308`; capi
/// `CAPI_Topology.pas:47-63` `ActiveTree` over the same `GetTopology`), so an
/// `Open`/`Close` between two reads is invisible to it. The port caches nothing —
/// [`Dss::topology_view`] rebuilds the tree on every call and therefore answers
/// from the present conductor state (CLAUDE.md: an upstream defect is never
/// reproduced; pinned by
/// `exec::tests::topology::an_open_conductor_isolates_the_downstream_branch`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TopologyView {
    /// `Topology.NumLoops` — the number of `IsLoopedHere` tree nodes, integer
    /// **halved** (r4133 `DTopology.pas:67-77`, `Result := Result div 2` at `:77`;
    /// capi `CAPI_Topology.pas:81-98`). This is *not* `looped_pairs.len()`: the
    /// halving counts loop *ends* while the pair list deduplicates by identity
    /// (IEEE13 measures 3 pairs and 1 loop — see
    /// `exec::tests::topology::num_loops_is_the_looped_here_count_halved`).
    pub num_loops: i32,
    /// `Topology.NumIsolatedBranches` = `isolated_branches.len()` — the PD
    /// elements the tree never reached (r4133 `DTopology.pas:79-88`; capi
    /// `CAPI_Topology.pas:302-316`).
    pub num_isolated_branches: i32,
    /// `Topology.NumIsolatedLoads` = `isolated_loads.len()` — the same over the
    /// PC elements (r4133 `DTopology.pas:89-98`; capi `CAPI_Topology.pas:448-462`).
    pub num_isolated_loads: i32,
    /// `Topology.AllLoopedPairs` — `(branch, the branch it loops onto)` in
    /// tree-walk order, deduplicated in both orientations (r4133
    /// `DTopology.pas:271-321`; capi `CAPI_Topology.pas:160-215`). Both oracles
    /// transport it as the flat `[a0, b0, a1, b1, ...]` string list this view
    /// pairs up.
    pub looped_pairs: Vec<(String, String)>,
    /// `Topology.AllIsolatedBranches` — the isolated PD elements' `FullName`s in
    /// `PDElements` (= creation) order (r4133 `DTopology.pas:322-356`, which emits
    /// `QualifiedName`; capi `CAPI_Topology.pas:114-151`, `FullName` — measured
    /// byte-identical on all 336 both-gated corpus cases).
    pub isolated_branches: Vec<String>,
    /// `Topology.AllIsolatedLoads` — the same over `PCElements` (r4133
    /// `DTopology.pas:357-392`; capi `CAPI_Topology.pas:369-405`).
    pub isolated_loads: Vec<String>,
    /// **Not one of the six compared quantities** — the *pre-dedup* looped-pair
    /// candidate sequence, in tree-walk order: one `(branch, branch it loops
    /// onto)` entry per `IsLoopedHere` node, exactly what upstream feeds into its
    /// own dedup scan before any candidate is dropped (r4133
    /// `DTopology.pas:283-285`, `pdLoop := topo.PresentBranch.LoopLineObj`; capi
    /// `CAPI_Topology.pas:177-179`). [`Self::looped_pairs`] is this sequence
    /// reduced by the port's per-pair rule.
    ///
    /// It exists because the two engines reduce the same sequence by two
    /// *different* rules and the live corpus gate asserts that difference
    /// positively instead of skipping the field (GOLDEN_REBASE coordinator
    /// decision D16): upstream scans its flat `[a0, b0, a1, b1, ...]` buffer in
    /// **overlapping windows** (`i := i + 1`, r4133 `DTopology.pas:286-296`, capi
    /// `CAPI_Topology.pas:180-190`) and so also drops a genuinely new candidate
    /// that happens to equal a *straddling* window `(b_j, a_{j+1})`, contradicting
    /// its own comment "see if we already found this pair"
    /// (`DTopology.pas:286`) — an upstream defect the port does not reproduce
    /// (CLAUDE.md). Re-applying that window scan to this sequence reproduces
    /// either oracle's `AllLoopedPairs` exactly
    /// (`exec::tests::topology::the_oracle_pair_list_is_the_window_scan_of_the_candidates`).
    pub looped_pair_candidates: Vec<(String, String)>,
}

/// Sum `Get_Losses` over one of the circuit's `TPointerList` kind lists
/// (`refs`), in list (= creation) order.
///
/// No `enabled` filter: upstream walks the raw pointer lists for `LineLosses` /
/// `SubstationLosses` / `AllElementLosses`, and a disabled element contributes
/// `CZERO` through `TDSSCktElement.Get_Losses`'s own guard
/// (`Common/CktElement.pas:707-712`), which
/// [`crate::elements::traits::CktElement::losses`] mirrors. `Circuit.Losses` is
/// the one aggregate that *does* filter, and it filters in [`Circuit::losses`]
/// where Pascal filters (`Common/Circuit.pas:2436-2440`).
fn sum_list_losses(
    classes: &mut [DssClass],
    refs: &[ElemId],
    sys: &crate::elements::traits::SysCtx,
    node_v: &[num_complex::Complex64],
) -> num_complex::Complex64 {
    let mut total = num_complex::Complex64::ZERO;
    for &r in refs {
        let elem = classes[r.class_ord()]
            .arena
            .try_ckt_elem_mut(r.index())
            .expect("circuit kind lists hold circuit elements");
        total += elem.losses(sys, node_v);
    }
    total
}

/// One row of the `PDElements` walk — the dss-python `ActiveCircuit.PDElements`
/// surface the fastdss harness compares wholesale
/// (`DSS-Python@origin/fastdss:dss/IPDElements.py:26-40` `_columns`, archived by
/// `tests/save_outputs.py:365`). Thirteen oracle fields plus [`Self::parent_name`],
/// which is ours: the oracle's `ParentPDElement` returns only a per-class
/// `ClassIndex`, so the parent's identity is not observable from it alone.
///
/// Oracle sources per field: capi `CAPI/CAPI_PDElements.pas:119-313`, r4133
/// `Version8/Source/DDLL/DPDELements.pas` (`PDElementsI` :13-124, `PDElementsF`
/// :125-216, `PDElementsS` :217-258). Membership is the circuit's `PDElements`
/// list — Line / Transformer / AutoTrans / Capacitor / Reactor / GICTransformer;
/// `Fault` is `NON_PCPD_ELEM` and never appears (`PDElements/Fault.pas:114`).
///
/// **The values are the *stored* `TPDElement` fields, never `CalcFltRate`'s
/// product.** [`Self::lambda`] is `BranchFltRate` and [`Self::accumulated_l`] is
/// `AccumulatedBrFltRate` — both 0 until the EnergyMeter reliability sweep
/// (`RelCalc`) writes them — while [`Self::fault_rate`] / [`Self::pct_permanent`]
/// are that sweep's *inputs*. Reading
/// [`crate::elements::traits::ReliabilityData::branch_flt_rate`] here instead
/// would report a value no oracle ever returns.
#[derive(Debug, Clone, PartialEq)]
pub struct PdElementView {
    /// `PDElements.Name`: the `Class.name` FullName (capi `:183-191`,
    /// r4133 `PDElementsS:0` `:226-236`).
    pub name: String,
    /// `AccumulatedL` = `AccumulatedBrFltRate` (capi `:215-223`, r4133 `F:5`).
    pub accumulated_l: f64,
    /// `FromTerminal`, **1-based** as the oracle reports it; `0` when unset.
    /// Pascal's default is `FromTerminal := 1`
    /// (r4133 `PDElements/PDElement.pas:194`), i.e. `Some(0)` on the port.
    pub from_terminal: i32,
    /// `IsShunt` (capi `:145-153`, r4133 `I:3`, which encodes it 0/1).
    pub is_shunt: bool,
    /// `Numcustomers` = `BranchNumCustomers` (capi `:235-243`, r4133 `I:4`).
    pub num_customers: i32,
    /// `SectionID` = `BranchSectionID` (capi `:305-313`, r4133 `I:8`).
    pub section_id: i32,
    /// `FaultRate` — the stored `TPDElement.FaultRate` (capi `:119-127`,
    /// r4133 `F:0`).
    pub fault_rate: f64,
    /// `RepairTime` = `HrsToRepair` (capi `:259-267`, r4133 `F:6`).
    pub repair_time: f64,
    /// `TotalMiles` = `AccumulatedMilesDownStream` (capi `:294-303`, r4133
    /// `F:7`) — a different quantity from `Bus.TotalMiles`.
    pub total_miles: f64,
    /// `Totalcustomers` = `BranchTotalCustomers` (capi `:269-282`, r4133 `I:5`).
    pub total_customers: i32,
    /// `pctPermanent` — the stored `TPDElement.PctPerm` (capi `:155-163`,
    /// r4133 `F:2`).
    pub pct_permanent: f64,
    /// `Lambda` = `BranchFltRate` (capi `:225-233`, r4133 `F:4`).
    pub lambda: f64,
    /// `ParentPDElement`: the parent's `ClassIndex` — a **1-based, per-class**
    /// creation index (`General/DSSObject.pas:43`, written by
    /// `AddObjectToList`), `0` when the branch has no upline parent. Note both
    /// oracles reassign `ActiveCktElement` to the parent while answering this
    /// and never restore it (capi `:245-257`, r4133 `I:6` `:88-100`), so it must be
    /// read last of the walk's fields.
    pub parent_class_index: i32,
    /// The parent's FullName, `""` when [`Self::parent_class_index`] is 0. Not
    /// an oracle field of its own: on both channels it is the name of whatever
    /// element the `ParentPDElement` read left active.
    pub parent_name: String,
    /// `MeterObj <> nil`: this PD element sits in some EnergyMeter's zone. Not
    /// an oracle column either — no `PDElements` arm reports it — and never
    /// compared; it is the port-side half of the predicate that scopes
    /// `harness::PD_SKIP_FIELDS` to the elements the oracles' zone build
    /// actually corrupts. Together with [`Self::is_shunt`] it names exactly the
    /// PD elements `MakeMeterZoneLists` files on the **PC** adjacency list
    /// (`Version8/Source/Shared/CktTree.pas:664-666`) and then writes
    /// `MeterObj`/`SensorObj` into through a `TPCElement` cursor
    /// (r4133 `Meters/EnergyMeter.pas:1868-1869`); port side, that write is
    /// `solution/meters/zones/build.rs:284`.
    pub in_meter_zone: bool,
}

/// A bus's solved voltages in the three flavours both oracles publish — the
/// dss-python / COM `Bus.puVoltages`, `Bus.VMagAngle` and `Bus.puVMagAngle`
/// surface (the fastdss harness dumps the whole `IBus` `_columns` set for the
/// active bus, `tests/save_outputs.py:351` on `origin/fastdss`).
///
/// The two oracles run the *same* arithmetic on these three: capi
/// `Alt_Bus_Get_puVoltages` / `Alt_Bus_Get_VMagAngle` /
/// `Alt_Bus_Get_puVMagAngle` (`CAPI/CAPI_Alt.pas:2251`, `:2573`, `:2540`)
/// and r4133 `BUSV` modes 5 / 13 / 14
/// (`Version8/Source/DDLL/DBus.pas:399`, `:659`, `:690`).
///
/// **Ordering.** `pu_voltages`, `vmag_angle` and `pu_vmag_angle` are ordered by
/// ascending node *number* — the `repeat NodeIdx := FindIdx(jj); inc(jj) until
/// NodeIdx > 0` walk both engines run (`CAPI_Alt.pas:2270-2275` ==
/// `DBus.pas:415-421`) — while [`BusVoltageView::nodes`] keeps `TDSSBus.Nodes`'
/// insertion order. A bus declared `.2.1.3` therefore reports
/// `nodes = [2, 1, 3]` next to voltages ordered `1, 2, 3`. That is neither the
/// `YNodeOrder` permutation nor the bus × internal-node-index order of
/// [`Dss::all_bus_vmag_pu`]; the three conventions must never be mixed.
///
/// `VLL` / `puVLL` are deliberately absent: the fastdss harness drops them in
/// this configuration (`save_outputs.py:205-209`, `COM_VLL_BROKEN`), and the
/// r4133 pairing loop has a state-dependent hang there — they land with the
/// sequence quantities in GOLDEN_REBASE G1.4c.
#[derive(Debug, Clone)]
pub struct BusVoltageView {
    /// The bus's (lowercased) name, `Circuit.AllBusNames` spelling.
    pub name: String,
    /// `TDSSBus.kVBase`, line-to-neutral kV; `0.0` = not set.
    pub kv_base: f64,
    /// User node numbers on the bus (`Nodes`), **insertion** order.
    pub nodes: Vec<i32>,
    /// `Bus.puVoltages`: `NodeV / BaseFactor`, ascending node number.
    pub pu_voltages: Vec<num_complex::Complex64>,
    /// `Bus.VMagAngle`: `(|V| volts, angle°)`, ascending node number.
    pub vmag_angle: Vec<(f64, f64)>,
    /// `Bus.puVMagAngle`: `(|V|/BaseFactor, angle°)`, ascending node number.
    pub pu_vmag_angle: Vec<(f64, f64)>,
}

/// The `BaseFactor` both engines divide the per-unit bus quantities by:
/// `1000 · kVBase`, or `1.0` when the bus has no base
/// (`CAPI_Alt.pas:2262-2265` == `DBus.pas:413-414` == `CAPI_Circuit.pas:538-541`
/// == `DCircuit.pas:493`). The `1.0` arm is live, not dead: 11 480 of the
/// corpus's 209 211 buses have `kVBase <= 0` (measured for GOLDEN_REBASE G1.4a).
fn bus_base_factor(bus: &crate::circuit::bus::Bus) -> f64 {
    if bus.kv_base > 0.0 {
        1000.0 * bus.kv_base
    } else {
        1.0
    }
}

/// The bus's local node indices in ascending node-**number** order — the
/// `repeat NodeIdx := FindIdx(jj); inc(jj) until NodeIdx > 0` walk of
/// `CAPI_Alt.pas:2270-2275` == `DBus.pas:415-421`, whose comment reads *"this
/// code so nodes come out in order from smallest to larges"*.
///
/// A stable sort by node number is exactly that walk: `Circuit::add_bus` never
/// pushes node 0 (ground short-circuits to `ref_no = 0` before the
/// `nodes.push`, `circuit/circuit.rs:599-603`), so every entry of `Bus::nodes`
/// is a distinct number `>= 1` and the engines' `jj = 1, 2, 3, …` scan finds
/// them in sorted order.
fn ascending_node_indices(bus: &crate::circuit::bus::Bus) -> Vec<usize> {
    let mut order: Vec<usize> = (0..bus.num_nodes_this_bus()).collect();
    order.sort_by_key(|&i| bus.get_num(i));
    order
}

/// `Solution.NodeV[node_ref]`, the way both engines index it (slot 0 = ground);
/// total here because `node_v` is empty before the first allocation.
fn node_voltage(ckt: &Circuit, node_ref: usize) -> num_complex::Complex64 {
    ckt.solution
        .node_v
        .get(node_ref)
        .copied()
        .unwrap_or_default()
}

/// Build one [`BusVoltageView`] over bus `bus_idx` (`BusList` index).
fn bus_voltage_view(ckt: &Circuit, bus_idx: usize) -> BusVoltageView {
    use crate::support::complexutil::c_to_polar_deg;

    let bus = &ckt.buses[bus_idx];
    let base_factor = bus_base_factor(bus);
    let n = bus.num_nodes_this_bus();

    let mut pu_voltages = Vec::with_capacity(n);
    let mut vmag_angle = Vec::with_capacity(n);
    let mut pu_vmag_angle = Vec::with_capacity(n);
    for i in ascending_node_indices(bus) {
        let v = node_voltage(ckt, bus.get_ref(i));
        // capi divides the two components (`CAPI_Alt.pas:2277-2280`), r4133
        // calls `cdivreal` (`DBus.pas:423`) — the same componentwise divide.
        pu_voltages.push(num_complex::Complex64::new(
            v.re / base_factor,
            v.im / base_factor,
        ));
        // Pascal `ctopolardeg` on the same `NodeV` entry; only the magnitude is
        // scaled, and only for the pu flavour (`CAPI_Alt.pas:2566-2569` ==
        // `DBus.pas:713-716`).
        let p = c_to_polar_deg(v);
        vmag_angle.push((p.mag, p.ang));
        pu_vmag_angle.push((p.mag / base_factor, p.ang));
    }

    BusVoltageView {
        name: bus.name.clone(),
        kv_base: bus.kv_base,
        nodes: bus.nodes.clone(),
        pu_voltages,
        vmag_angle,
        pu_vmag_angle,
    }
}

/// Flatten a bus short-circuit matrix the way both oracles publish it —
/// `for i := 1 to Nelements do for j := 1 to Nelements do … GetElement(i, j)`
/// (r4133 `DBus.pas:445-450` == capi `CAPI_Alt.pas:2318-2330`), i.e.
/// **row-major**, `i` outer.
///
/// This has to stay an explicit `(i, j)` walk: `CMatrix` stores column-major
/// (`support/cmatrix/mod.rs:45-47`), so handing out the backing store would
/// publish the transpose. `Zsc` and `Ysc` are symmetric only to solver noise —
/// each `Zsc` column is a separate `Y·V = e_i` solve, and the measured
/// `|Z_ij − Z_ji|` reaches 4.66e-10 (IEEE123Master-SC bus `610`) with *no*
/// off-diagonal pair bit-equal on any of the three decks measured (of four) —
/// so a transposed flatten would be a silent, band-invisible error on the
/// corpus, not a caught one. The convention is therefore held by this
/// citation, by [`BusScView`]'s ordering block and by the offline pin
/// `bus_sc_tests::flatten_row_major_walks_i_outer_on_an_asymmetric_matrix`
/// (G1.5 audit settlement AC-4) — never by an oracle value comparison.
fn flatten_row_major(m: &crate::support::cmatrix::CMatrix) -> Vec<num_complex::Complex64> {
    let n = m.order();
    let mut out = Vec::with_capacity(n * n);
    for i in 0..n {
        for j in 0..n {
            out.push(m.get(i, j));
        }
    }
    out
}

/// Build one [`BusScView`] over bus `bus_idx` (`BusList` index). Pure reads off
/// `TDSSBus`: no `ComputeIterminal`, no active-element state, nothing cached —
/// the same "group C, order-free" property the oracles' own `BUSV` arms have.
fn bus_sc_view(ckt: &Circuit, bus_idx: usize) -> BusScView {
    let b = &ckt.buses[bus_idx];
    BusScView {
        name: b.name.clone(),
        nodes: b.nodes.clone(),
        zsc1: b.get_zsc1(),
        zsc0: b.get_zsc0(),
        zsc: b.zsc.as_ref().map(flatten_row_major),
        ysc: b.ysc.as_ref().map(flatten_row_major),
        isc: b.bus_current.clone(),
        vbus: b.vbus.clone(),
    }
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
                for ((p, &n), i) in powers
                    .iter_mut()
                    .zip(&cd.node_ref[..yorder])
                    .zip(&cd.iterminal[..yorder])
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
                        // `* 0.001` on `Complex64` is componentwise
                        // (`Complex::new(re * s, im * s)`) — the same two
                        // multiplications the interleaved form did.
                        *p = s * 0.001;
                    }
                }
            }
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

    /// Read one bus's short-circuit results — the dss-python `Bus.Zsc1` /
    /// `Zsc0` / `ZscMatrix` / `YscMatrix` / `Isc` / `Voc` surface (see
    /// [`BusScView`] for the ordering contract and the Pascal citations).
    /// `name` is the bus name (case-insensitive, like `SetActiveBus`); `None`
    /// if no such bus exists.
    pub fn bus_short_circuit(&self, name: &str) -> Option<BusScView> {
        let ckt = self.circuit.as_ref()?;
        let idx = ckt.bus_list.find(name)?;
        Some(bus_sc_view(ckt, idx))
    }

    /// Every bus's [`BusScView`] in `BusList` order — the order
    /// `Circuit.AllBusNames` reports and the order [`Dss::all_bus_voltages`]
    /// walks, so the two views pair up index for index (which is what lets the
    /// harness capture both surfaces in a single `SetActiveBus` sweep). Empty
    /// when no circuit exists.
    pub fn all_bus_short_circuit(&self) -> Vec<BusScView> {
        match self.circuit.as_ref() {
            Some(ckt) => (0..ckt.buses.len()).map(|i| bus_sc_view(ckt, i)).collect(),
            None => Vec::new(),
        }
    }

    /// Read one bus's solved voltages — the dss-python `Bus.puVoltages` /
    /// `Bus.VMagAngle` / `Bus.puVMagAngle` surface (see [`BusVoltageView`] for
    /// the ordering contract and the Pascal citations). `name` is the bus name
    /// (case-insensitive); `None` if no such bus exists.
    pub fn bus_voltages(&self, name: &str) -> Option<BusVoltageView> {
        let ckt = self.circuit.as_ref()?;
        let idx = ckt.bus_list.find(name)?;
        Some(bus_voltage_view(ckt, idx))
    }

    /// Every bus's [`BusVoltageView`] in `BusList` order — the order
    /// `Circuit.AllBusNames` reports (`CAPI_Circuit.pas` /
    /// `DCircuit.pas:439`). Empty when no circuit exists.
    pub fn all_bus_voltages(&self) -> Vec<BusVoltageView> {
        match self.circuit.as_ref() {
            Some(ckt) => (0..ckt.buses.len())
                .map(|i| bus_voltage_view(ckt, i))
                .collect(),
            None => Vec::new(),
        }
    }

    /// `Circuit.AllBusVmagPu`: `|NodeV| / BaseFactor` for every node, walked as
    /// **bus × internal node index** — `for i := 1 to NumBuses do for j := 1 to
    /// Buses[i].NumNodesThisBus do … GetRef(j)`
    /// (`CAPI_Circuit.pas:521-548` == `DCircuit.pas:481-500`, r4133 mode 9;
    /// fastdss dumps it with the rest of `ICircuit._columns`,
    /// `tests/save_outputs.py:348`). Length = `NumNodes`.
    ///
    /// This is the *second* of the three bus orderings and is neither
    /// [`BusVoltageView`]'s ascending-node-number order nor `YNodeOrder`: a bus
    /// that gains a node after a later bus was created holds node refs that are
    /// not contiguous, so the walk visits `NodeV` out of ref order.
    ///
    /// capi guards the read with a `MissingSolution` early-out
    /// (`CAPI_Circuit.pas:528-532`) that r4133 does not have and that cannot
    /// fire post-solve; under the r4133-authority policy it is not reproduced.
    pub fn all_bus_vmag_pu(&self) -> Vec<f64> {
        let Some(ckt) = self.circuit.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(ckt.num_nodes);
        for bus in &ckt.buses {
            let base_factor = bus_base_factor(bus);
            for j in 0..bus.num_nodes_this_bus() {
                // Pascal `Cabs`: the naive modulus, proven bit-identical to
                // `f64::hypot`/`Complex::norm` on every reachable operand
                // (`support/line_constants/tests.rs`,
                // `naive_modulus_equals_hypot_until_the_square_overflows`).
                out.push(node_voltage(ckt, bus.get_ref(j)).norm() / base_factor);
            }
        }
        out
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

    /// The circuit's `PDElements` walk — the oracle's
    /// `PDElements.First`/`Next` iteration over `Circuit.PDElements`, one
    /// [`PdElementView`] per **enabled** PD element (both oracles skip
    /// `not Enabled`: capi `CAPI/CAPI_Utils.pas:718-759`
    /// `Generic_CktElement_Get_First/Next`, r4133 `DPDELements.pas:27-59`).
    ///
    /// Order is `Circuit.pd_elements` — the `AddCktElement` creation order that
    /// both oracles' pointer lists carry (`Common/Circuit.pas:2242-2248`;
    /// port side `circuit/circuit.rs:488-561`). Read-only: unlike the oracles,
    /// which leave `ActiveCktElement` pointing at the last-read *parent*
    /// (see [`PdElementView::parent_class_index`]), this touches no engine state.
    pub fn pd_elements(&self) -> Vec<PdElementView> {
        let Some(ckt) = self.circuit.as_ref() else {
            return Vec::new();
        };
        let full_name = |r: ElemId| -> String {
            format!(
                "{}.{}",
                self.classes[r.class_ord()].props.class_name(),
                self.classes[r.class_ord()].arena[r.index()].data().name()
            )
        };
        let mut out = Vec::with_capacity(ckt.pd_elements.len());
        for &r in &ckt.pd_elements {
            // Every id in `Circuit.pd_elements` was pushed by `AddCktElement`
            // for a PD class, so the slot always holds a circuit element; the
            // `else` is unreachable and stays total rather than panicking in a
            // read-only accessor (a dropped row would red the comparator's
            // length assert first).
            let slot = self.classes[r.class_ord()].arena.try_ckt_elem(r.index());
            debug_assert!(
                slot.is_some(),
                "Circuit.pd_elements holds a non-ckt slot at class {} index {}",
                r.class_ord(),
                r.index()
            );
            let Some(elem) = slot else {
                continue;
            };
            let cd = elem.cd();
            if !cd.enabled {
                continue;
            }
            // `fault_rate` / `pct_perm` / `hrs_to_repair` are the STORED
            // `TPDElement` inputs (`ReliabilityData`); the accumulators below
            // come from `CktElementData`, never from `branch_flt_rate`, which
            // is `CalcFltRate`'s product rather than the reported field.
            let rel = elem.reliability_data();
            out.push(PdElementView {
                name: full_name(r),
                accumulated_l: cd.accumulated_br_flt_rate,
                from_terminal: cd.from_terminal.map_or(0, |t| t as i32 + 1),
                is_shunt: elem.is_shunt(),
                num_customers: cd.branch_num_customers,
                section_id: cd.branch_section_id,
                fault_rate: rel.fault_rate,
                repair_time: rel.hrs_to_repair,
                total_miles: cd.accumulated_miles_downstream,
                total_customers: cd.branch_total_customers,
                pct_permanent: rel.pct_perm,
                lambda: cd.branch_flt_rate,
                parent_class_index: cd.parent_pd.map_or(0, |p| p.index() as i32 + 1),
                parent_name: cd.parent_pd.map_or(String::new(), full_name),
                in_meter_zone: cd.meter_obj.is_some(),
            });
        }
        out
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

    /// CAPI `Circuit_Get_LineLosses` (`CAPI_Circuit.pas:145-162`; r4133
    /// `DDLL/DCircuit.pas:305-325`, `Circuit.LineLosses` = `CircuitV` mode 1) —
    /// **kW/kvar**: `Get_Losses` summed over the circuit's `Lines` list, scaled
    /// by `0.001`. Both revisions walk the raw pointer list with no `enabled`
    /// filter; see [`sum_list_losses`].
    pub fn line_losses(&mut self) -> (f64, f64) {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_ref().expect("line_losses needs a circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let total = sum_list_losses(classes, &ckt.lines, &sys, &node_v);
        (total.re * 0.001, total.im * 0.001)
    }

    /// CAPI `Circuit_Get_SubstationLosses` (`CAPI_Circuit.pas:289-307`; r4133
    /// `DDLL/DCircuit.pas:327-347`, `CircuitV` mode 2) — **kW/kvar** over the
    /// `Transformers` list, keeping the entries whose `sub=` flag is set
    /// (`TTransfObj.IsSubstation`).
    ///
    /// `AutoTrans` objects are registered on the separate `AutoTransformers`
    /// list (`Common/Circuit.pas:2272-2273`; the port mirrors the split in
    /// [`Circuit::add_ckt_element`]), so an `AutoTrans ... sub=yes` contributes
    /// **nothing** here — upstream's walk cannot reach it. Pinned by
    /// `exec::tests::aggregates::substation_losses_exclude_autotrans`.
    pub fn substation_losses(&mut self) -> (f64, f64) {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_ref().expect("substation_losses needs a circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let subs: Vec<ElemId> = ckt
            .transformers
            .iter()
            .copied()
            .filter(|r| {
                classes[r.class_ord()]
                    .arena
                    .get::<transformer::Transformer>(r.index())
                    .expect("the transformers list holds Transformers")
                    .is_substation()
            })
            .collect();
        let total = sum_list_losses(classes, &subs, &sys, &node_v);
        (total.re * 0.001, total.im * 0.001)
    }

    /// CAPI `Circuit_Get_AllElementLosses` (`CAPI_Circuit.pas:445-468`; r4133
    /// `DDLL/DCircuit.pas:458-479`, `CircuitV` mode 8) — each element's
    /// `Get_Losses × 0.001` (**kW/kvar**) in `ckt_elements` creation order, i.e.
    /// exactly the order and length (`NumDevices`) of the oracle's
    /// `AllElementNames` / [`Dss::snapshot_elements`]. Disabled elements keep
    /// their slot and report `(0, 0)`.
    pub fn all_element_losses(&mut self) -> Vec<(f64, f64)> {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit
            .as_ref()
            .expect("all_element_losses needs a circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let mut out = Vec::with_capacity(ckt.ckt_elements.len());
        for &r in &ckt.ckt_elements {
            let elem = classes[r.class_ord()]
                .arena
                .try_ckt_elem_mut(r.index())
                .expect("ckt_elements refs are circuit elements");
            let loss = elem.losses(&sys, &node_v);
            out.push((loss.re * 0.001, loss.im * 0.001));
        }
        out
    }

    /// The summand membership of the four scalar circuit aggregates, as
    /// lower-cased `Class.name` strings in walk order — see [`AggregateTerms`].
    /// Empty (`AggregateTerms::default()`) when no circuit exists.
    pub fn aggregate_terms(&self) -> AggregateTerms {
        let Some(ckt) = self.circuit.as_ref() else {
            return AggregateTerms::default();
        };
        let full_name = |r: ElemId| -> String {
            format!(
                "{}.{}",
                self.classes[r.class_ord()].props.class_name(),
                self.classes[r.class_ord()]
                    .arena
                    .obj(r.index())
                    .data()
                    .name()
            )
            .to_ascii_lowercase()
        };
        AggregateTerms {
            // `Common/Circuit.pas:2436-2440`: enabled AND not shunt — the one
            // aggregate upstream filters (mirrored by `Circuit::losses`).
            losses: ckt
                .pd_elements
                .iter()
                .copied()
                .filter(|r| {
                    let elem = self.classes[r.class_ord()].arena.ckt_elem(r.index());
                    elem.cd().enabled && !elem.is_shunt()
                })
                .map(&full_name)
                .collect(),
            line_losses: ckt.lines.iter().copied().map(&full_name).collect(),
            substation_losses: ckt
                .transformers
                .iter()
                .copied()
                .filter(|r| {
                    self.classes[r.class_ord()]
                        .arena
                        .get::<transformer::Transformer>(r.index())
                        .expect("the transformers list holds Transformers")
                        .is_substation()
                })
                .map(&full_name)
                .collect(),
            total_power: ckt.sources.iter().copied().map(&full_name).collect(),
        }
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

    // -----------------------------------------------------------------------
    // GOLDEN_REBASE G1.7 — the topology interface (`ITopology`)
    // -----------------------------------------------------------------------

    /// The six order-free `ITopology` quantities of [`TopologyView`], computed
    /// from a **freshly built** topology tree
    /// ([`crate::solution::topology::get_topology`], Pascal
    /// `TDSSCircuit.GetTopology`, r4133 `Common/Circuit.pas:2932-2950`).
    ///
    /// `&mut self` because building the tree stamps the Pascal flags upstream
    /// stamps — `Checked` / `Terminals[i].Checked` cleared, `IsIsolated` set on
    /// every circuit element "till proven otherwise", `BusChecked` cleared
    /// (r4133 `Common/Circuit.pas:2937-2947`) — and then clears `IsIsolated` on
    /// everything the walk from `Sources.First` reaches. Upstream's first
    /// `Topology` read does exactly the same to its own circuit, which is why the
    /// live gate reads this surface **last** in a checkpoint.
    ///
    /// Empty (`TopologyView::default()`) when no circuit exists — upstream's
    /// `ActiveTree` guard (capi `CAPI_Topology.pas:47-63`, r4133's
    /// `if topo <> nil`) returns the same zeros / `NONE` sentinels there.
    ///
    /// This is a second, independent consumer of `get_topology`; the
    /// `Show Topology` report (`report/show/topology.rs`) keeps its own walk.
    pub fn topology_view(&mut self) -> TopologyView {
        use crate::elements::ckt::ElemFlags;
        let Dss {
            classes, circuit, ..
        } = self;
        let Some(ckt) = circuit.as_mut() else {
            return TopologyView::default();
        };
        // The build needs the element store mutably; names are read back once
        // that borrow ends (the `show_topology` pattern).
        let mut tree = {
            let mut store = ClassStore { classes };
            crate::solution::topology::get_topology(ckt, &mut store)
        };
        let classes: &[DssClass] = classes;
        let full_name = |r: ElemId| -> String {
            format!(
                "{}.{}",
                classes[r.class_ord()].props.class_name(),
                classes[r.class_ord()].arena[r.index()].data().name()
            )
        };

        // One walk feeds both loop quantities (r4133 `DTopology.pas:67-77` for the
        // count, `:280-303` for the pairs; capi `CAPI_Topology.pas:90-97` /
        // `:173-199`): `NumLoops` counts the `IsLoopedHere` nodes and halves,
        // while every looped node contributes one `(branch, LoopLineObj)`
        // CANDIDATE, in walk order and before any dedup — the same sequence
        // upstream feeds into its own scan (`DTopology.pas:283-285`; capi
        // `CAPI_Topology.pas:177-179`).
        let mut looped_here = 0i32;
        let mut looped_pair_candidates: Vec<(String, String)> = Vec::new();
        let mut pd = tree.first();
        while let Some(pd_ref) = pd {
            let (is_looped, loop_elem) = {
                let node = tree.present_node();
                (node.is_looped, node.loop_elem)
            };
            if is_looped {
                looped_here += 1;
                // Pascal reads `PresentBranch.LoopLineObj` unguarded; the port's
                // `loop_elem` is set together with `is_looped`
                // (`solution/topology.rs:178-190`), so `None` is unreachable —
                // and a missing partner must not invent a pair either way. The
                // claim is enforced where it is made rather than left as a silent
                // skip (G1.7 audit settlement): every test build, the corpus gate
                // included, runs this assert on every looped node.
                debug_assert!(
                    loop_elem.is_some(),
                    "a node flagged `is_looped` carries no `loop_elem`: \
                     `num_loops` and `looped_pairs` would part company silently \
                     (the two are set together in `solution/topology.rs`)"
                );
                if let Some(le) = loop_elem {
                    looped_pair_candidates.push((full_name(pd_ref), full_name(le)));
                }
            }
            pd = tree.go_forward();
        }

        // Dedup semantics: both oracles scan their flat `[a0, b0, a1, b1, ...]`
        // buffer with `i := 1; while (i <= k); i := i + 1` (r4133
        // `DTopology.pas:286-296`, capi `CAPI_Topology.pas:180-190`), i.e. over
        // *overlapping* windows `(buf[i-1], buf[i])` — so a candidate that happens
        // to coincide with a straddling window `(b_j, a_{j+1})` is dropped although
        // that pair was never found. The port implements the stated intent ("see if
        // we already found this pair", `DTopology.pas:286`): a candidate is dropped
        // only when an already-stored PAIR matches it in either orientation
        // (CLAUDE.md — an upstream defect is never reproduced in any lane). The
        // difference is not hidden: the raw candidate sequence stays available as
        // `TopologyView::looped_pair_candidates`, and the live gate re-applies the
        // window scan to it and requires the oracle list back (decision D16).
        let mut looped_pairs: Vec<(String, String)> = Vec::new();
        for pair in &looped_pair_candidates {
            let seen = looped_pairs
                .iter()
                .any(|(a, b)| (a == &pair.0 && b == &pair.1) || (a == &pair.1 && b == &pair.0));
            if !seen {
                looped_pairs.push(pair.clone());
            }
        }

        // The isolated lists walk the circuit's own PD / PC pointer lists in
        // creation order and keep what the walk above never reached (r4133
        // `DTopology.pas:79-88` / `:89-98` for the counts, `:322-356` / `:357-392`
        // for the names; capi `CAPI_Topology.pas:302-316` / `:448-462` and
        // `:114-151` / `:369-405`). Count and list come from the same filter, so
        // the two can never disagree — as they cannot upstream, where each pair is
        // the same loop over the same list.
        let isolated = |refs: &[ElemId]| -> Vec<String> {
            refs.iter()
                .copied()
                .filter(|&r| {
                    classes[r.class_ord()]
                        .arena
                        .try_ckt_elem(r.index())
                        .is_some_and(|e| e.cd().flags.contains(ElemFlags::IS_ISOLATED))
                })
                .map(&full_name)
                .collect()
        };
        let isolated_branches = isolated(&ckt.pd_elements);
        let isolated_loads = isolated(&ckt.pc_elements);

        TopologyView {
            num_loops: looped_here / 2,
            num_isolated_branches: isolated_branches.len() as i32,
            num_isolated_loads: isolated_loads.len() as i32,
            looped_pairs,
            isolated_branches,
            isolated_loads,
            looped_pair_candidates,
        }
    }
}

// ---------------------------------------------------------------------------
// GOLDEN_REBASE G1.6(i): the `Meters` reliability surface.
//
// The half of the dss-python `IMeters` facade the live gate has never read —
// the indices `CalcReliabilityIndices` writes, the per-section block behind
// `SetActiveSection`, the two load-allocation arrays and the class-wide
// register totals. `DSS-Python@origin/fastdss:tests/save_outputs.py:283-291`
// archives the same fields (it reads section 1 only; this reads every
// section) and `:330-332` records the `Totals` read-order trap below.
//
// Everything here is a read of already-solved state: none of these accessors
// runs the reliability sweep and nothing in `solution/meters/reliability.rs`
// is touched. Both oracles answer the same fields only *after* the executive
// `RelCalc` command has run; before that every number is the Pascal zero-init.
// ---------------------------------------------------------------------------

/// One feeder section of an EnergyMeter — the oracle's *active-section* block,
/// read as `Meters.SetActiveSection(idx)` followed by the eight per-section
/// getters (capi `CAPI/CAPI_Meters.pas:729-740` + `:742-852`, r4133
/// `Version8/Source/DDLL/DMeters.pas` `MetersI` 22-27 `:254-307` and `MetersF`
/// 4-6 `:369-393`).
///
/// Section indices are **1-based**: slot 0 of Pascal's `FeederSections` is the
/// span above the first OCP device and neither oracle will report it — capi
/// refuses it in `InvalidActiveSection` (`CAPI_Meters.pas:122-134`, error
/// 5055) and r4133 guards every getter with `If ActiveSection > 0` — so it
/// never appears here either.
#[derive(Debug, Clone, PartialEq)]
pub struct FeederSectionView {
    /// The 1-based index this row was read at (the `SetActiveSection`
    /// argument), running `1..=`[`MeterReliabilityView::num_sections`].
    pub idx: i32,
    /// `Meters.OCPDeviceType` — 1=Fuse, 2=Recloser, 3=Relay (0 = none).
    pub ocp_device_type: i32,
    /// `Meters.NumSectionCustomers` = `FeederSections[idx].NCustomers`.
    pub num_section_customers: i32,
    /// `Meters.NumSectionBranches` = `NBranches`.
    pub num_section_branches: i32,
    /// `Meters.SectSeqIdx` = `SeqIndex` — the 1-based `SequenceList` position
    /// of the PD element that carries this section's OCP device.
    pub sect_seq_idx: i32,
    /// `Meters.SectTotalCust` = `TotalCustomers`.
    pub sect_total_cust: i32,
    /// `Meters.SumBranchFltRates`.
    pub sum_branch_flt_rates: f64,
    /// `Meters.AvgRepairTime` = `AverageRepairTime` = `SumFltRatesXRepairHrs /
    /// SumBranchFltRates` — an **unguarded** division on all three engines
    /// (port `solution/meters/reliability.rs:293`; r4133
    /// `Version8/Source/Meters/EnergyMeter.pas` `AverageRepairTime`), so a
    /// section whose branches all have `faultrate=0` evaluates to `NaN`
    /// identically everywhere.
    pub avg_repair_time: f64,
    /// `Meters.FaultRateXRepairHrs` = `SumFltRatesXRepairHrs`.
    pub fault_rate_x_repair_hrs: f64,
}

/// One row of the `Meters` reliability walk — the fields the oracles expose per
/// meter once `RelCalc` has run, in the order the capture reads them.
///
/// Oracle sources: capi `CAPI/CAPI_Meters.pas` (`Meters_Get_TotalCustomers`
/// `:685-694` → `CAPI/CAPI_Alt.pas:1683-1696`, `SAIFI` `:617-626`, `SAIFIKW`
/// `:652-661`, `SAIDI` `:696-705`, `CustInterrupts` `:707-716`, `NumSections`
/// `:718-727`, `CalcCurrent` `:335-350`, `AllocFactors` `:379-391`); r4133
/// `Version8/Source/DDLL/DMeters.pas` (`MetersI` 20/21 `:232-253`, `MetersF`
/// 0-3 `:329-368`, `MetersV` 6/8 `:609-624` / `:645-661`, the zone lists
/// `MetersV` 10-12 `:662-758`).
///
/// The walk that produces these rows is `Meters.First`/`Next`, which **skips
/// disabled meters** on both channels (r4133 `DMeters.pas:32-71` loops on
/// `If pMeter.Enabled`; capi routes through `Generic_CktElement_Get_First` /
/// `_Next`, `CAPI/CAPI_Utils.pas:718-759`), so [`Dss::meter_reliability`]
/// skips them too.
#[derive(Debug, Clone, PartialEq)]
pub struct MeterReliabilityView {
    /// `Meters.Name` — the bare object name, as both oracles report it.
    pub name: String,
    /// `Meters.TotalCustomers` = `BusTotalNumCustomers` of the bus at the
    /// `FromTerminal` of `SequenceList[1]` — the zone head's upstream bus, not
    /// a field of the meter. `0` when the zone was never built (empty
    /// `SequenceList`), which is what both oracles return there as well.
    pub total_customers: i32,
    /// `Meters.SAIFI`.
    pub saifi: f64,
    /// `Meters.SAIFIKW`.
    pub saifi_kw: f64,
    /// `Meters.SAIDI`.
    pub saidi: f64,
    /// `Meters.CustInterrupts`.
    pub cust_interrupts: f64,
    /// `CAIDI` = `SAIDI / SAIFI`. **Not an oracle API field** — neither
    /// channel has a `CAIDI` mode — so no capture carries it and the
    /// comparator never reads it from one; it reaches the live gate as
    /// EnergyMeter property `CAIDI`, which `compare_all_properties` already
    /// compares on both channels. It is carried here so the pin that names the
    /// arithmetic can read it.
    pub caidi: f64,
    /// `Meters.CalcCurrent` — `|CalculatedCurrent[k]|` for `k` in
    /// `0..NPhases`. The oracles read the array from its **start**, with no
    /// `(MeteredTerminal-1)·NConds` offset (capi `:346-349`, r4133
    /// `:622-623`), while `TMeterElement.CalcAllocationFactors` *writes* it at
    /// exactly that offset (r4133
    /// `Version8/Source/Meters/MeterElement.pas:54-72`); the API's indexing is
    /// what both channels report, so it is what this returns. All-zero until an
    /// `AllocateLoads` runs — on the oracles it is uninitialised heap there,
    /// since `AllocateSensorArrays` `ReallocMem`s the array without zeroing
    /// (`MeterElement.pas:45-52`).
    pub calc_current: Vec<f64>,
    /// `Meters.AllocFactors` = `PhsAllocationFactor[0..NPhases]`; same
    /// uninitialised-until-`AllocateLoads` caveat as [`Self::calc_current`].
    pub alloc_factors: Vec<f64>,
    /// `Meters.AllBranchesInZone`, in `SequenceList` order.
    pub branches: Vec<String>,
    /// `Meters.AllEndElements`, in `ZoneEndsList` order.
    pub ends: Vec<String>,
    /// `Meters.ZonePCE`, in zone-walk order.
    pub pce: Vec<String>,
    /// `Meters.NumSections` = `SectionCount`.
    pub num_sections: i32,
    /// The sections `1..=`[`Self::num_sections`], read in ascending order.
    pub sections: Vec<FeederSectionView>,
}

impl Dss {
    /// The `Meters` reliability walk — one [`MeterReliabilityView`] per
    /// **enabled** EnergyMeter, in the circuit's `EnergyMeters` pointer-list
    /// (creation) order, which is the order `Meters.First`/`Next` visits on
    /// both oracle channels.
    ///
    /// Read-only: it neither runs `CalcReliabilityIndices` nor moves an
    /// active-object cursor, so unlike the oracle walk it can be called at any
    /// point without perturbing anything.
    pub fn meter_reliability(&self) -> Vec<MeterReliabilityView> {
        let Some(ckt) = self.circuit.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(ckt.energy_meters.len());
        for &r in &ckt.energy_meters {
            let Some(em) = self.classes[r.class_ord()]
                .arena
                .get::<energymeter::EnergyMeter>(r.index())
            else {
                continue;
            };
            if !em.enabled() {
                continue;
            }
            let name = em.data().name().to_string();

            // `Buses^[Terminals^[FromTerminal].BusRef].BusTotalNumCustomers` of
            // `SequenceList.Get(1)` (r4133 `DMeters.pas:232-243`, capi
            // `CAPI_Alt.pas:1683-1696`). Every step is fallible on a zone that
            // was never built, and both oracles answer 0 there rather than
            // failing (capi's `checkSequenceList` guard, r4133's
            // `If Assigned(PD_Element)`).
            let total_customers = em
                .sequence_list()
                .first()
                .and_then(|&head| {
                    self.classes[head.class_ord()]
                        .arena
                        .try_ckt_elem(head.index())
                })
                .and_then(|head| {
                    let cd = head.cd();
                    cd.from_terminal.and_then(|t| cd.terminals.get(t))
                })
                .and_then(|term| term.bus_ref)
                .and_then(|bus| ckt.buses.get(bus))
                .map_or(0, |bus| bus.bus_total_num_customers);

            // The three ordered zone lists come from `meter_zone`, so the
            // ordered assertion this surface adds and the existing
            // set-compare (`harness::compare_meter`) are looking at one list
            // rather than at two resolutions of it.
            let zone = self.meter_zone(&name);

            let nphases = em.med.cd.nphases;
            let calc_current = (0..nphases)
                .map(|k| em.med.calculated_current.get(k).map_or(0.0, |c| c.norm()))
                .collect();
            let alloc_factors = (0..nphases)
                .map(|k| em.med.phs_allocation_factor.get(k).copied().unwrap_or(0.0))
                .collect();

            let num_sections = em.section_count();
            let sections = (1..=num_sections.max(0))
                .filter_map(|idx| {
                    // `FeederSections` keeps its previous length after an
                    // aborted `RelCalc` (only a successful sweep reallocates
                    // it), so the slot is fetched, never assumed. A short
                    // array would silently shorten the row, which the
                    // comparator would then read as a length divergence, so
                    // the mismatch is caught here in debug builds too.
                    let slot = em.feeder_sections().get(idx as usize);
                    debug_assert!(
                        slot.is_some(),
                        "EnergyMeter {name}: SectionCount {num_sections} exceeds the \
                         FeederSections array ({} slots)",
                        em.feeder_sections().len()
                    );
                    let s = slot?;
                    Some(FeederSectionView {
                        idx,
                        ocp_device_type: s.ocp_device_type.ordinal(),
                        num_section_customers: s.n_customers,
                        num_section_branches: s.n_branches,
                        sect_seq_idx: s.seq_index as i32,
                        sect_total_cust: s.total_customers,
                        sum_branch_flt_rates: s.sum_branch_flt_rates,
                        avg_repair_time: s.average_repair_time,
                        fault_rate_x_repair_hrs: s.sum_flt_rates_x_repair_hrs,
                    })
                })
                .collect();

            out.push(MeterReliabilityView {
                name,
                total_customers,
                saifi: em.saifi(),
                saifi_kw: em.saifi_kw(),
                saidi: em.saidi(),
                cust_interrupts: em.cust_interrupts(),
                caidi: em.caidi(),
                calc_current,
                alloc_factors,
                branches: zone
                    .as_ref()
                    .map(|z| z.all_branches_in_zone.clone())
                    .unwrap_or_default(),
                ends: zone
                    .as_ref()
                    .map(|z| z.all_end_elements.clone())
                    .unwrap_or_default(),
                pce: zone.map(|z| z.zone_pce).unwrap_or_default(),
                num_sections,
                sections,
            });
        }
        out
    }

    /// `Meters.Totals` — Pascal `TDSSCircuit.TotalizeMeters` (r4133
    /// `Version8/Source/Common/Circuit.pas:2520-2538`, reached through capi
    /// `Meters_Get_Totals` `CAPI/CAPI_Meters.pas:279-290` and r4133
    /// `MetersV(3)` `DDLL/DMeters.pas:558-573`): `RegisterTotals[i] =
    /// Σ_meters Registers[i] · TotalsMask[i]`, length
    /// [`energymeter::NUM_EM_REGISTERS`].
    ///
    /// Two details are load-bearing and both follow the Pascal literally: the
    /// sum runs over **every** meter in the circuit's `EnergyMeters` list —
    /// there is no `Enabled` filter, unlike the `Meters.First`/`Next` walk of
    /// [`Self::meter_reliability`] — and it runs in that list's creation order,
    /// which is observable in the last bits of a float sum.
    ///
    /// On the oracles this read is **destructive to the meter cursor**:
    /// `TotalizeMeters` walks `EnergyMeters.First`/`Next` itself, so a capture
    /// that reads `Totals` mid-walk loses every meter after the current one
    /// (`DSS-Python@origin/fastdss:tests/save_outputs.py:330-332`, "This breaks
    /// the iteration"). Here it is a pure read, but the capture transports must
    /// still read it last — the harness asserts that order.
    pub fn meter_totals(&self) -> Vec<f64> {
        let mut totals = vec![0.0; energymeter::NUM_EM_REGISTERS];
        let Some(ckt) = self.circuit.as_ref() else {
            return totals;
        };
        for &r in &ckt.energy_meters {
            let Some(em) = self.classes[r.class_ord()]
                .arena
                .get::<energymeter::EnergyMeter>(r.index())
            else {
                continue;
            };
            for (total, (reg, mask)) in totals
                .iter_mut()
                .zip(em.registers().iter().zip(em.totals_mask()))
            {
                *total += reg * mask;
            }
        }
        totals
    }
}

/// One bus's reliability columns — the eight `IBus` fields the fastdss harness
/// archives for every bus (`DSS-Python@origin/fastdss:dss/IBus.py:19-53`
/// `_columns`, dumped through the iterable `ActiveCircuit.ActiveBus`,
/// `tests/save_outputs.py:351`).
///
/// Oracle sources, column by column: capi `CAPI/CAPI_Bus.pas`
/// (`Bus_Get_Int_Duration` `:462`, `Bus_Get_Lambda` `:473`,
/// `Bus_Get_Cust_Duration` `:484`, `Bus_Get_Cust_Interrupts` `:495`,
/// `Bus_Get_N_Customers` `:506`, `Bus_Get_N_interrupts` `:517`,
/// `Bus_Get_TotalMiles` `:604`, `Bus_Get_SectionID` `:614`); r4133 the `BUSF`
/// modes 6-11 (`Version8/Source/DDLL/DBus.pas:129-170`) and the `BUSI` modes
/// 4-5 (`:60-73`). Every arm on both channels is a bare `TDSSBus` field read
/// behind "a bus is active" — no `Assigned` walk, no allocation, no state
/// write — so the surface is order-free with respect to the element captures.
///
/// The values are whatever the reliability machinery last left on the bus, and
/// reading them runs none of it: `lambda_` / `total_miles` / `n_customers` are
/// the backward sweep's accumulators (`TPDElement.CalcFltRate`, r4133
/// `Version8/Source/PDElements/PDElement.pas:106-116`), the other five are set
/// by `TEnergyMeterObj.CalcReliabilityIndices` (r4133
/// `Version8/Source/Meters/EnergyMeter.pas:2488-2494`, `:2571-2573`,
/// `:2610-2611`; port `solution/meters/reliability.rs`).
///
/// Field spelling mirrors the capture wire keys one-for-one, so a single macro
/// extracts this view and the harness's `BusReliabilityCap` alike. That is the
/// only reason `lambda_` carries a trailing underscore: the capi capture builds
/// its row in Python, where `lambda` cannot be spelled as an identifier.
#[derive(Debug, Clone, PartialEq)]
pub struct BusReliabilityView {
    /// The bus's (lowercased) name, `Circuit.AllBusNames` spelling.
    pub name: String,
    /// `Bus.Cust_Duration` = `TDSSBus.BusCustDurations`: accumulated customer
    /// outage durations. A single **assignment** per zone load bus (r4133
    /// `EnergyMeter.pas:2610-2611`), not an accumulation.
    pub cust_duration: f64,
    /// `Bus.Cust_Interrupts` = `BusCustInterrupts`: accumulated customer
    /// interruptions.
    pub cust_interrupts: f64,
    /// `Bus.Int_Duration` = `Bus_Int_Duration`: average annual interruption
    /// duration, `Source_IntDuration + FeederSections[SectionID].
    /// AverageRepairTime` for a bus with a section (r4133
    /// `EnergyMeter.pas:2571-2573`). That repair time is an unguarded division
    /// on all three engines, so a section whose branches all have
    /// `faultrate = 0` propagates `NaN` here identically everywhere (the
    /// [`FeederSectionView::avg_repair_time`] note).
    pub int_duration: f64,
    /// `Bus.Lambda` = `BusFltRate`: accumulated downstream failure rate,
    /// faults/yr.
    pub lambda_: f64,
    /// `Bus.N_Customers` = `BusTotalNumCustomers`: customers served from this
    /// bus (integer on both channels — capi returns `Integer`, r4133 `BUSI(4)`
    /// a `longint`).
    pub n_customers: i32,
    /// `Bus.N_interrupts` = `Bus_Num_Interrupt`: interruptions/yr at this bus.
    pub n_interrupts: f64,
    /// `Bus.SectionID` = `BusSectionID`: the feeder section this bus belongs
    /// to. Three values are reachable and all three are reported verbatim: `0`
    /// from `TDSSBus.Create` and for the span above the first OCP device
    /// (r4133 `EnergyMeter.pas:2494`), `-1` = "not set" while a zone's
    /// accumulators are zeroed (`Bus::zero_reliability_accums`, Pascal
    /// `TDSSBus.ZeroReliabilityAccums`), and `1..=SectionCount` afterwards.
    pub section_id: i32,
    /// `Bus.TotalMiles` = `BusTotalMiles`: line miles downstream of this bus.
    pub total_miles: f64,
}

impl Dss {
    /// Every bus's reliability columns in `Circuit::buses` order — the
    /// `BusList` order `Circuit.AllBusNames` reports and both oracle captures
    /// walk (capi `CAPI_Circuit.pas`, r4133 `DCircuit.pas:439`), so the two
    /// sides align index by index. Empty when no circuit exists.
    ///
    /// Pure field reads off [`crate::circuit::bus::Bus`] (see
    /// [`BusReliabilityView`] for the per-column Pascal sources): this runs no
    /// reliability sweep, allocates no accumulator and moves no active-object
    /// cursor, so — unlike the oracle walk, which must `SetActiveBus` before
    /// every row — it can be called at any point without perturbing anything.
    pub fn bus_reliability(&self) -> Vec<BusReliabilityView> {
        let Some(ckt) = self.circuit.as_ref() else {
            return Vec::new();
        };
        ckt.buses
            .iter()
            .map(|b| BusReliabilityView {
                name: b.name.clone(),
                cust_duration: b.bus_cust_durations,
                cust_interrupts: b.bus_cust_interrupts,
                int_duration: b.bus_int_duration,
                lambda_: b.bus_flt_rate,
                n_customers: b.bus_total_num_customers,
                n_interrupts: b.bus_num_interrupt,
                section_id: b.bus_section_id,
                total_miles: b.bus_total_miles,
            })
            .collect()
    }
}

#[cfg(test)]
mod bus_voltage_tests {
    use super::*;
    use crate::support::complexutil::c_to_polar_deg;
    use num_complex::Complex64;

    /// One deck that exercises all three orderings at once.
    ///
    /// * `b3` is declared `.2.1.3`, so its **insertion** order differs from the
    ///   ascending node-number order the oracles publish (convention 1).
    /// * `b1` is first seen with node 1 only and gains nodes 2 and 3 *after*
    ///   `b2` was handed its node ref, so the bus × node-index walk of
    ///   [`Dss::all_bus_vmag_pu`] (convention 2) is a different permutation
    ///   from `YNodeOrder`.
    fn bus_deck() -> Dss {
        let mut dss = Dss::new();
        dss.command("New circuit.busview basekv=12.47 pu=1.0 phases=3 bus1=sourcebus");
        dss.command("New Line.l1 bus1=sourcebus.1 bus2=b1.1 phases=1 r1=0.1 x1=0.3 c1=0 length=1");
        dss.command("New Line.l2 bus1=b1.1 bus2=b2.1 phases=1 r1=0.1 x1=0.3 c1=0 length=1");
        dss.command(
            "New Line.l3 bus1=sourcebus.2.3 bus2=b1.2.3 phases=2 r1=0.1 x1=0.3 c1=0 length=1",
        );
        dss.command(
            "New Line.l4 bus1=sourcebus.1.2.3 bus2=b3.2.1.3 phases=3 r1=0.1 x1=0.3 c1=0 length=1",
        );
        dss.command("New Load.ld bus1=b1.1 phases=1 kv=7.2 kw=500 pf=0.95");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    }

    fn solved_with_bases() -> Dss {
        let mut dss = bus_deck();
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    }

    /// Convention 1: `puVoltages`/`VMagAngle`/`puVMagAngle` come out ordered by
    /// ascending node **number** (`CAPI_Alt.pas:2270-2275` == `DBus.pas:415-421`),
    /// while `nodes` keeps `TDSSBus.Nodes`' insertion order. Pinned on a bus
    /// declared `.2.1.3`; 140 corpus cases carry such non-prefix node sets.
    #[test]
    fn bus_pu_voltages_come_out_in_ascending_node_number_order() {
        let dss = solved_with_bases();
        let ckt = dss.circuit().expect("circuit");
        let ib = ckt.bus_list.find("b3").expect("bus b3");
        let bus = &ckt.buses[ib];
        assert_eq!(bus.nodes, vec![2, 1, 3], "declared .2.1.3");

        // Bus lookup is case-insensitive, like `SetActiveBus`.
        let v = dss.bus_voltages("B3").expect("bus b3");
        assert_eq!(v.name, "b3");
        assert_eq!(v.nodes, vec![2, 1, 3], "the view keeps insertion order");
        assert_eq!(v.kv_base, bus.kv_base);

        let bf = 1000.0 * bus.kv_base;
        assert!(bf > 0.0);
        for (k, node) in [1i32, 2, 3].into_iter().enumerate() {
            let raw = ckt.solution.node_v[bus.find(node)];
            assert_eq!(
                v.pu_voltages[k],
                Complex64::new(raw.re / bf, raw.im / bf),
                "slot {k} must be node {node}"
            );
            let p = c_to_polar_deg(raw);
            assert_eq!(v.vmag_angle[k], (p.mag, p.ang));
            assert_eq!(v.pu_vmag_angle[k], (p.mag / bf, p.ang));
        }

        // The two orders are observably different, not just nominally.
        // `Line.l4` maps source phase 1 -> b3 node 2, phase 2 -> node 1,
        // phase 3 -> node 3, so ascending node order (1, 2, 3) carries the
        // source angles (-120, 0, +120) while the insertion order (2, 1, 3)
        // would carry (0, -120, +120): the first two slots swap.
        assert!(
            (v.vmag_angle[0].1 + 120.0).abs() < 15.0,
            "slot 0 = node 1 = source phase 2 near -120 deg, got {}",
            v.vmag_angle[0].1
        );
        assert!(
            v.vmag_angle[1].1.abs() < 15.0,
            "slot 1 = node 2 = source phase 1 near 0 deg, got {}",
            v.vmag_angle[1].1
        );
        assert!(
            (v.vmag_angle[2].1 - 120.0).abs() < 15.0,
            "slot 2 = node 3 = source phase 3 near +120 deg, got {}",
            v.vmag_angle[2].1
        );
    }

    /// `BaseFactor` is `1000 * kVBase`, falling back to `1.0` when the bus has
    /// no base (`CAPI_Alt.pas:2262-2265` == `DBus.pas:413-414`). Both arms are
    /// pinned: 11 480 of the corpus's 209 211 buses take the `1.0` arm, and the
    /// `1000*` factor (not a bare `kVBase`) is what makes the pu magnitude ~ 1.
    #[test]
    fn bus_pu_voltages_use_a_unit_base_when_kv_base_is_not_set() {
        // No `CalcVoltageBases`: every bus keeps `TDSSBus.Create`'s kVBase = 0.
        let mut dss = bus_deck();
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let v = dss.bus_voltages("b3").expect("bus b3");
        assert_eq!(v.kv_base, 0.0, "no CalcVoltageBases means kVBase unset");
        let ckt = dss.circuit().expect("circuit");
        let bus = &ckt.buses[ckt.bus_list.find("b3").expect("bus b3")];
        for (k, node) in [1i32, 2, 3].into_iter().enumerate() {
            let raw = ckt.solution.node_v[bus.find(node)];
            assert_eq!(v.pu_voltages[k], raw, "BaseFactor = 1.0 gives raw volts");
            assert_eq!(v.pu_vmag_angle[k], v.vmag_angle[k]);
        }
        assert!(
            v.pu_vmag_angle[0].0 > 1000.0,
            "unit base gives volts, got {}",
            v.pu_vmag_angle[0].0
        );

        // The other arm, on the same deck with bases calculated.
        let dss = solved_with_bases();
        let v = dss.bus_voltages("b3").expect("bus b3");
        let bf = 1000.0 * v.kv_base;
        assert_eq!(v.kv_base, 12.47 / crate::util::sqrt3());
        assert_eq!(v.pu_vmag_angle[0].0, v.vmag_angle[0].0 / bf);
        assert!(
            (0.5..1.5).contains(&v.pu_vmag_angle[0].0),
            "per-unit, not per-kV: {}",
            v.pu_vmag_angle[0].0
        );
    }

    /// Convention 2: `AllBusVmagPu` walks buses x internal node index
    /// (`CAPI_Circuit.pas:535-546` == `DCircuit.pas:490-498`), which on this
    /// deck is a different permutation from `YNodeOrder` (node-ref order) *and*
    /// from the ascending-node-number order of [`Dss::bus_voltages`].
    #[test]
    fn all_bus_vmag_pu_walks_buses_times_internal_node_index() {
        let dss = solved_with_bases();
        let ckt = dss.circuit().expect("circuit");
        let all = dss.all_bus_vmag_pu();
        assert_eq!(all.len(), ckt.num_nodes);

        // The node refs this walk visits, in order - b1 gained nodes 2 and 3
        // after b2's node ref was handed out, so the sequence is not 1..=n.
        let mut refs = Vec::new();
        for bus in &ckt.buses {
            for j in 0..bus.num_nodes_this_bus() {
                refs.push(bus.get_ref(j));
            }
        }
        assert_eq!(
            refs,
            vec![1, 2, 3, 4, 6, 7, 5, 8, 9, 10],
            "bus x node-index order, NOT YNodeOrder (1..=NumNodes)"
        );
        for (k, &r) in refs.iter().enumerate() {
            let bus = &ckt.buses[ckt.map_node_to_bus[r].bus_ref];
            let bf = if bus.kv_base > 0.0 {
                1000.0 * bus.kv_base
            } else {
                1.0
            };
            assert_eq!(all[k], ckt.solution.node_v[r].norm() / bf);
        }

        // Contrast with convention 1 on the `.2.1.3` bus: the last three slots
        // are that bus in insertion order (2, 1, 3), while `bus_voltages`
        // reports 1, 2, 3.
        let v = dss.bus_voltages("b3").expect("bus b3");
        let tail = &all[all.len() - 3..];
        assert_eq!(tail[0], v.pu_vmag_angle[1].0, "slot 0 = node 2");
        assert_eq!(tail[1], v.pu_vmag_angle[0].0, "slot 1 = node 1");
        assert_eq!(tail[2], v.pu_vmag_angle[2].0, "slot 2 = node 3");

        // `all_bus_voltages` is the same bus sequence as `AllBusNames`.
        let views = dss.all_bus_voltages();
        assert_eq!(views.len(), ckt.buses.len());
        let names: Vec<&str> = views.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, vec!["sourcebus", "b1", "b2", "b3"]);
    }
}

#[cfg(test)]
mod bus_sc_tests {
    use super::*;
    use num_complex::Complex64;

    /// `micro`-tier band, the harness' own numbers for this kind of case
    /// (`tests/harness/mod.rs::tol_for`: `v`/`y`/`i` = `1e-6` abs + `1e-9`
    /// rel — the abs/rel pair the corpus gate applies to this very deck, and
    /// the pair `harness::tol_for` is pinned to by
    /// `the_short_circuit_surface_adds_no_tolerance_constant`).
    /// Every oracle literal below is a live reading of the *same* deck on both
    /// channels — dss-python 0.15.7 / capi 0.14.5 and the EPRI r4133 DLL — which
    /// agree with each other to ~1e-15 relative and with the port to ~1e-12 abs.
    ///
    /// **Driving matters for `Voc`/`Isc`, and the literals are the deck driven
    /// as written.** `SolveFaultStudy` sets `LoadModel := ADMITTANCE` and takes
    /// its open-circuit voltages from a `SolveDirect`
    /// (`Common/SolutionAlgs.pas:884-892`), so a *second* fault study starts
    /// from the first one's unit-injection residual and lands ~1.2e-4 relative
    /// away: on this deck `Voc[0]` is `6933.784990446535 + 971.5021007848138j`
    /// after `solve; solve mode=faultstudy` but `6934.538221643409 +
    /// 971.9391266606829j` when a bare `solve` is appended (measured on both
    /// channels — `tmp/g15/f1/voc_micro{,_r4133}.json`). `Zsc`/`Ysc` are
    /// identical either way, being functions of `Y` alone.
    ///
    /// **Measured worsts on this deck** (port vs capi / port vs r4133, as the
    /// baseline a later tightening would start from): `Zsc` 3.39e-15 / 4.99e-15
    /// abs (1.44e-15 / 2.12e-15 rel), `Ysc` 7.73e-16 abs (1.64e-15 rel), `Isc`
    /// 2.09e-12 / 1.85e-12 abs (6.4e-16 rel), `Voc` 1.22e-11 / 1.15e-11 abs
    /// (2.28e-15 rel) — the same order as the two oracles' own disagreement
    /// (capi vs r4133: `Zsc` 2.89e-15, `Voc` 1.15e-11), i.e. the faer-vs-KLU
    /// last-ulp floor. The band stays the `micro` tier's rather than these
    /// numbers on purpose: it is the band the corpus gate applies to this very
    /// deck, and the errors these pins exist to catch (a transposed or
    /// ascending-sorted index, an absent matrix) are 0.6 ohm / 800 V wide.
    fn close(got: Complex64, want: Complex64, what: &str) {
        let allowed = 1e-6 + 1e-9 * want.norm();
        assert!(
            (got - want).norm() <= allowed,
            "{what}: {got} vs oracle {want} (|diff| = {:.6e} > allowed {allowed:.3e})",
            (got - want).norm()
        );
    }

    /// **The row-major convention, driven** (GOLDEN_REBASE G1.5 audit
    /// settlement, AC-4). Spec §4's non-vacuity demo 1 — transpose the flatten
    /// and watch the live gate red — was measured VACUOUS and dropped: `Zsc`
    /// and `Ysc` are symmetric to ≤ 4.66e-10 on every bus of every corpus deck
    /// that runs a study, six orders inside the band, so no oracle comparison
    /// anywhere can tell `m.get(i, j)` from `m.get(j, i)`. That left
    /// [`flatten_row_major`]'s convention — the one both oracles publish
    /// (`For i … For j … Zsc.GetElement(i, j)`, r4133 `DDLL/DBus.pas:445-450`
    /// == capi `CAPI/CAPI_Alt.pas:2318-2330`) — carried by a citation alone.
    ///
    /// This pins it where symmetry cannot hide it: a deliberately ASYMMETRIC
    /// matrix whose `(i, j)` entry is `10·i + j`, so the row-major flatten is
    /// `[00, 01, 02, 10, …]` while the transposed walk would be
    /// `[00, 10, 20, 01, …]` — different in all six off-diagonal slots. The
    /// same literals also separate it from `CMatrix`'s COLUMN-major backing
    /// store (`support/cmatrix/mod.rs:45-47`), which is the transpose here, so
    /// the day someone "optimizes" the loop into a `values().to_vec()` this
    /// test reds instead of the surface silently publishing `Zscᵀ`.
    #[test]
    fn flatten_row_major_walks_i_outer_on_an_asymmetric_matrix() {
        let mut m = crate::support::cmatrix::CMatrix::new(3);
        for i in 0..3 {
            for j in 0..3 {
                m.set(i, j, Complex64::new((10 * i + j) as f64, 0.0));
            }
        }
        let got: Vec<f64> = flatten_row_major(&m).iter().map(|c| c.re).collect();
        assert_eq!(
            got,
            vec![0.0, 1.0, 2.0, 10.0, 11.0, 12.0, 20.0, 21.0, 22.0],
            "flatten_row_major must walk `i` outer / `j` inner, the read order of \
             both oracles' `BUSV` matrix arms"
        );
        let stored: Vec<f64> = m.values().iter().map(|c| c.re).collect();
        assert_eq!(
            stored,
            vec![0.0, 10.0, 20.0, 1.0, 11.0, 21.0, 2.0, 12.0, 22.0],
            "CMatrix stores column-major — the premise of the explicit walk"
        );
        assert_ne!(
            got, stored,
            "on an asymmetric matrix the row-major flatten and the backing store \
             must differ, or this pin proves nothing"
        );
    }

    /// The GOLDEN_REBASE G1.5 micro deck, byte-for-byte the circuit that
    /// `tests/corpus/modes/faultstudy/faultstudy_micro.dss` carries into the
    /// corpus gate. Two properties make it worth its size:
    ///
    /// * `line.l2 bus1=b1.1.2.3 bus2=b2.2.1.3` gives `b2` the insertion order
    ///   `[2, 1, 3]`, which is *not* its ascending node order `[1, 2, 3]`;
    /// * `reactor.rsh bus1=b2.1 R=5` puts a 5 ohm shunt on node 1 alone, so the
    ///   `Zsc`/`Ysc` diagonal is position-dependent and the two orders are
    ///   observably different rather than merely nominally so.
    ///
    /// Returned solved but *before* the fault study, so each test drives the
    /// study itself.
    fn sc_micro() -> Dss {
        let mut dss = Dss::new();
        dss.command("Set DefaultBaseFrequency=60");
        dss.command(
            "new circuit.scmicro basekv=12.47 pu=1.0 phases=3 bus1=sourcebus \
             r1=0.5 x1=1.5 r0=1.0 x0=3.0",
        );
        dss.command("new linecode.lc3 nphases=3 r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0 units=km");
        dss.command("new linecode.lc1 nphases=1 r1=0.4 x1=1.2 c1=0 units=km");
        dss.command("new line.l1 bus1=sourcebus bus2=b1 linecode=lc3 length=1 units=km");
        dss.command("new line.l2 bus1=b1.1.2.3 bus2=b2.2.1.3 linecode=lc3 length=1 units=km");
        dss.command("new reactor.rsh bus1=b2.1 phases=1 R=5 X=0");
        dss.command("new load.ld1 bus1=b2.2.1.3 phases=3 conn=wye kv=12.47 kw=100 pf=0.95 model=1");
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve");
        assert!(dss.errors().is_empty(), "snap solve: {:?}", dss.errors());
        dss
    }

    fn fault_study(dss: &mut Dss) {
        dss.command("solve mode=faultstudy");
        assert!(dss.errors().is_empty(), "faultstudy: {:?}", dss.errors());
    }

    /// `zsc`/`ysc` are a **shape**, not a zero matrix: `TDSSBus.Zsc` stays
    /// unassigned until `AllocateBusQuantities` runs inside the FaultStudy
    /// solve (`Common/SolutionAlgs.pas:773-781` == `fault_study.rs:57`), and
    /// `Get_Zsc1`/`Get_Zsc0` return `cZERO` in that state
    /// (`Common/Bus.pas:215-229`). Both oracles publish a one-entry
    /// `CZero`/default sentinel there, which is why the harness compares the
    /// "study ran" bit before any number.
    ///
    /// `isc`/`vbus` are the contrast: `ReProcessBusDefs` allocates and zeroes
    /// them for every bus (`Common/Circuit.pas:2407-2408`), so they are already
    /// `NumNodesThisBus` long — and exactly zero — before the study.
    #[test]
    fn zsc_is_absent_until_a_fault_study_runs() {
        let mut dss = sc_micro();

        let before = dss.all_bus_short_circuit();
        assert_eq!(before.len(), 3, "sourcebus, b1, b2");
        for v in &before {
            assert!(v.zsc.is_none(), "bus {}: Zsc before the study", v.name);
            assert!(v.ysc.is_none(), "bus {}: Ysc before the study", v.name);
            assert_eq!(v.zsc1, Complex64::ZERO, "bus {}: Zsc1 cZERO arm", v.name);
            assert_eq!(v.zsc0, Complex64::ZERO, "bus {}: Zsc0 cZERO arm", v.name);
            assert_eq!(v.isc.len(), v.nodes.len(), "bus {}: Isc length", v.name);
            assert_eq!(v.vbus.len(), v.nodes.len(), "bus {}: Voc length", v.name);
            assert!(
                v.isc.iter().all(|c| *c == Complex64::ZERO),
                "bus {}: Isc is allocated but untouched before the study",
                v.name
            );
        }

        fault_study(&mut dss);

        for v in &dss.all_bus_short_circuit() {
            let n = v.nodes.len();
            assert_eq!(
                v.zsc.as_ref().map(Vec::len),
                Some(n * n),
                "bus {}: Zsc is n*n after the study",
                v.name
            );
            assert_eq!(
                v.ysc.as_ref().map(Vec::len),
                Some(n * n),
                "bus {}: Ysc is n*n after the study",
                v.name
            );
            assert_ne!(v.zsc1, Complex64::ZERO, "bus {}: Zsc1 is live", v.name);
            assert_ne!(v.zsc0, Complex64::ZERO, "bus {}: Zsc0 is live", v.name);
        }
    }

    /// **Convention 2.** `ZscMatrix`, `YscMatrix`, `Isc` and `Voc` are indexed
    /// by the bus's *internal* node index — the oracles walk `GetRef(i)` /
    /// `Zsc.GetElement(i, j)` directly (r4133 `DBus.pas:431-459`/`:491-518` ==
    /// capi `CAPI_Alt.pas:2305-2334`/`:2336-2365`) with no `FindIdx` sort — so
    /// the slot order is `TDSSBus.Nodes`' insertion order, not the ascending
    /// node order `Bus.Nodes` itself publishes (`CAPI_Alt.pas:2143-2163`).
    ///
    /// On `b2` (declared `.2.1.3`, insertion `[2, 1, 3]`) the 5 ohm shunt sits
    /// on node **1**, i.e. internal index **1**. Both oracles put the odd
    /// diagonal there: `Zsc[1][1] = 1.665278843631496 + 1.662460471266969j`
    /// against `Zsc[0][0] = 1.0633872484998361 + 2.873778548090103j`. Ascending
    /// order would have put it at index 0, and the gap is 0.6 ohm — six orders
    /// above any band. `Ysc` says the same thing in closed form: the shunt is
    /// `1/5 = 0.2 S` and `Ysc[1][1] - Ysc[0][0] = 0.2 + 0j`.
    #[test]
    fn the_short_circuit_arrays_are_indexed_by_internal_node_index() {
        let mut dss = sc_micro();
        fault_study(&mut dss);

        let v = dss.bus_short_circuit("B2").expect("bus b2");
        assert_eq!(v.name, "b2", "case-insensitive lookup, BusList spelling");
        assert_eq!(v.nodes, vec![2, 1, 3], "declared bus2=b2.2.1.3");

        let z = v.zsc.as_ref().expect("study ran");
        let y = v.ysc.as_ref().expect("study ran");
        assert_eq!(z.len(), 9);
        assert_eq!(y.len(), 9);

        // Row-major: entry (i, j) is z[i * 3 + j].
        close(
            z[0],
            Complex64::new(1.0633872484998361, 2.873778548090103),
            "Zsc[0][0]",
        );
        close(
            z[4],
            Complex64::new(1.665278843631496, 1.662460471266969),
            "Zsc[1][1]",
        );
        close(
            z[8],
            Complex64::new(1.0633872484998352, 2.8737785480901032),
            "Zsc[2][2]",
        );
        close(
            z[1],
            Complex64::new(0.4998250387875434, 0.4970654339231112),
            "Zsc[0][1]",
        );
        close(
            z[2],
            Complex64::new(0.36149275755946514, 0.7764976346466627),
            "Zsc[0][2]",
        );

        // The ordering claim as an inequality no band can absorb: the odd
        // diagonal sits at the insertion slot of node 1, not at slot 0.
        assert_eq!(
            v.nodes[1], 1,
            "internal index 1 is node 1, the shunt's node"
        );
        assert!(
            (z[4] - z[0]).norm() > 0.5,
            "Zsc[1][1] {} must differ from Zsc[0][0] {} by the shunt, not by noise",
            z[4],
            z[0]
        );

        // `Ysc`: the 5 ohm shunt is exactly 0.2 S on its own diagonal slot.
        close(
            y[0],
            Complex64::new(0.11671451166612462, -0.3484256569058215),
            "Ysc[0][0]",
        );
        close(
            y[4],
            Complex64::new(0.3167145116661245, -0.3484256569058213),
            "Ysc[1][1]",
        );
        close(
            y[8],
            Complex64::new(0.11671451166612454, -0.34842565690582167),
            "Ysc[2][2]",
        );
        close(
            y[4] - y[0],
            Complex64::new(0.2, 0.0),
            "Ysc[1][1] - Ysc[0][0] = 1/R",
        );

        // `Voc` carries the same index: the sag is at slot 1. capi values;
        // r4133 agrees to 1.1e-11 abs (6933.784990446525 + 971.5021007848129j,
        // -4473.628028729489 - 2952.8238120661385j, -3849.24186153543 +
        // 7214.584823013563j).
        assert_eq!(v.vbus.len(), 3);
        close(
            v.vbus[0],
            Complex64::new(6933.784990446535, 971.5021007848138),
            "Voc[0]",
        );
        close(
            v.vbus[1],
            Complex64::new(-4473.628028729488, -2952.8238120661385),
            "Voc[1]",
        );
        close(
            v.vbus[2],
            Complex64::new(-3849.2418615354186, 7214.584823013565),
            "Voc[2]",
        );
        assert!(
            v.vbus[1].norm() < v.vbus[0].norm(),
            "the shunted node sags, and it is slot 1"
        );

        // `Isc = Ysc * Voc`, length n on the same index. capi values; r4133
        // agrees to 8e-13 abs.
        assert_eq!(v.isc.len(), 3);
        close(
            v.isc[0],
            Complex64::new(1028.2406633473126, -3085.47655441101),
            "Isc[0]",
        );
        close(
            v.isc[1],
            Complex64::new(-3186.134740270875, 652.1195203877785),
            "Isc[1]",
        );
        close(
            v.isc[2],
            Complex64::new(2157.80036243219, 2433.9836703768838),
            "Isc[2]",
        );
    }

    /// `AvgOffDiagonal` divides only when it summed something
    /// (`Shared/Ucmatrix.pas:369-383` == `support/cmatrix/mod.rs:273-282`), so a
    /// one-node bus has `Zm = 0` and `Zsc1 = Zs - 0`, `Zsc0 = Zs + 2*0` collapse
    /// onto the single entry — bit-exactly, not within a band.
    ///
    /// Oracle witness for the same identity: IEEE123Master-SC carries **57**
    /// one-node buses, and on bus `2` both channels report
    /// `Zsc1 = Zsc0 = Zsc[0][0]` = `0.12160973905653437 + 0.4051399884521036j`
    /// (capi) and `0.12160973905653336 + 0.40513998845210436j` (r4133) — the
    /// same three numbers each time.
    #[test]
    fn zsc1_collapses_to_the_single_entry_on_a_one_node_bus() {
        let mut dss = Dss::new();
        dss.command("Set DefaultBaseFrequency=60");
        dss.command(
            "New Circuit.sc1 basekv=12.47 phases=3 bus1=sourcebus pu=1.0 \
             R1=0.5 X1=1.5 R0=1.0 X0=3.0",
        );
        dss.command(
            "New Line.spur bus1=sourcebus.1 bus2=b3.1 phases=1 r1=0.4 x1=1.2 c1=0 length=1",
        );
        dss.command("Set voltagebases=[12.47]");
        dss.command("calcv");
        dss.command("solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        fault_study(&mut dss);

        let v = dss.bus_short_circuit("b3").expect("bus b3");
        assert_eq!(v.nodes, vec![1], "a one-node bus");
        let z = v.zsc.as_ref().expect("study ran");
        assert_eq!(z.len(), 1, "1x1");
        assert_eq!(v.zsc1, z[0], "Zsc1 = Zs - Zm with Zm = 0");
        assert_eq!(v.zsc0, z[0], "Zsc0 = Zs + 2*Zm with Zm = 0");

        // A three-node bus in the same circuit does not collapse, so the
        // identity above is the degeneracy and not a tautology of the code.
        let src = dss.bus_short_circuit("sourcebus").expect("sourcebus");
        assert_eq!(src.nodes.len(), 3);
        assert_ne!(src.zsc1, src.zsc0, "Zm != 0 on a 3-node bus");
    }

    /// Nothing between solves clears `Zsc`/`Ysc`: they are dropped only by a
    /// bus-list rebuild (`RestoreBusInfo` restores `VBus` but not the matrices
    /// — `circuit::Circuit::reprocess_bus_defs`), so a later solve in another mode leaves the
    /// study's matrices standing, entry for entry. That is the state
    /// `NEVTestCase/Run_NEV.dss` gates on: `set mode=Faultstudy; solve` followed
    /// by `solve mode=harmonics`, with the 13-node `tertiary` bus still
    /// reporting a 13x13 `Zsc` afterwards.
    #[test]
    fn zsc_survives_a_later_non_faultstudy_solve() {
        let mut dss = sc_micro();
        fault_study(&mut dss);
        let before = dss.bus_short_circuit("b2").expect("bus b2");
        let z0 = before.zsc.clone().expect("study ran");
        let y0 = before.ysc.clone().expect("study ran");
        let (zsc1, zsc0) = (before.zsc1, before.zsc0);

        dss.command("solve mode=harmonics");
        assert!(dss.errors().is_empty(), "harmonics: {:?}", dss.errors());

        let after = dss.bus_short_circuit("b2").expect("bus b2");
        assert_eq!(after.zsc.as_deref(), Some(&z0[..]), "Zsc untouched");
        assert_eq!(after.ysc.as_deref(), Some(&y0[..]), "Ysc untouched");
        assert_eq!(after.zsc1, zsc1);
        assert_eq!(after.zsc0, zsc0);
        assert_eq!(after.nodes, vec![2, 1, 3], "and still on the same index");
    }

    /// `all_bus_short_circuit` walks `BusList` — the same sequence
    /// `all_bus_voltages` and `Circuit.AllBusNames` use — so the harness can
    /// capture both bus surfaces in one `SetActiveBus` sweep and pair them by
    /// index.
    #[test]
    fn all_bus_short_circuit_is_the_bus_list_order() {
        let mut dss = sc_micro();
        fault_study(&mut dss);

        let sc = dss.all_bus_short_circuit();
        let v = dss.all_bus_voltages();
        assert_eq!(sc.len(), v.len());
        let names: Vec<&str> = sc.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, vec!["sourcebus", "b1", "b2"]);
        for (a, b) in sc.iter().zip(&v) {
            assert_eq!(a.name, b.name, "paired by index");
            assert_eq!(a.nodes, b.nodes, "both views keep insertion order");
        }
        // The by-name accessor is the same builder.
        for a in &sc {
            let one = dss.bus_short_circuit(&a.name).expect("by name");
            assert_eq!(one.zsc, a.zsc);
            assert_eq!(one.ysc, a.ysc);
            assert_eq!(one.isc, a.isc);
            assert_eq!(one.vbus, a.vbus);
        }
    }

    /// The fault study is **not** the only writer of `Voc`: `BuildYMatrix`
    /// brackets the rebuild with `UpdateVBus` / `RestoreNodeVfromVbus` whenever
    /// `Solution.PreserveNodeVoltages` is set (r4133
    /// `Common/YMatrix.pas:170`/`:282` == `solution::ymatrix::build_y_matrix`),
    /// and that flag is set entering Harmonic/HarmonicT and Dynamic mode. So a
    /// harmonics deck publishes a live `Voc` (and an `Isc` derived from it)
    /// while `Zsc`/`Ysc` stay `None` — which is exactly why the harness
    /// compares the discrete "study ran" bit and the `Voc` values as two
    /// independent facts instead of gating one on the other.
    ///
    /// Oracle witness for the same shape, both channels
    /// (`tmp/g15/voc_exposure.json`): `modes/harmonics/harmonict` reports no
    /// `Zsc` on any of its 3 buses with `max |Voc| = 64.49615498313288` V (its
    /// three harmonics siblings 59.10 / 97.55 / 90.13 V), against
    /// `max |Voc| = 0` on all ten `modes/reduce/*` decks, which never set the
    /// flag. On this deck the port's own maximum is 12.329071484855646 V.
    #[test]
    fn voc_is_refreshed_by_preserve_node_voltages_not_only_by_the_fault_study() {
        let mut dss = sc_micro();
        for b in dss.all_bus_short_circuit() {
            assert!(b.zsc.is_none(), "{}: no study has run", b.name);
            assert!(
                b.vbus.iter().all(|v| *v == Complex64::new(0.0, 0.0)),
                "{}: VBus is the allocation zero after a plain solve, {:?}",
                b.name,
                b.vbus
            );
        }

        dss.command("solve mode=harmonics");
        assert!(dss.errors().is_empty(), "harmonics: {:?}", dss.errors());

        let after = dss.all_bus_short_circuit();
        for b in &after {
            assert!(
                b.zsc.is_none() && b.ysc.is_none(),
                "{}: a harmonics solve must not fabricate a short-circuit matrix",
                b.name
            );
        }
        let worst = after
            .iter()
            .flat_map(|b| b.vbus.iter())
            .map(|v| v.norm())
            .fold(0.0_f64, f64::max);
        assert!(
            worst > 1.0,
            "PreserveNodeVoltages must have refreshed VBus, got max |Voc| = {worst}"
        );

        // The values themselves, on the bus whose insertion order is [2, 1, 3]
        // — so the refresh lands on this surface's own index, not the ascending
        // one. (`close` is the `micro` band; these are the port's readings.)
        let b2 = after.iter().find(|b| b.name == "b2").expect("bus b2");
        assert_eq!(b2.nodes, vec![2, 1, 3]);
        close(
            b2.vbus[0],
            Complex64::new(11.97308377213697, -2.941303905425441),
            "Voc[0] after harmonics",
        );
        close(
            b2.vbus[1],
            Complex64::new(2.1154414072085364, -0.45083050936453944),
            "Voc[1] after harmonics",
        );
        close(
            b2.vbus[2],
            Complex64::new(-0.44918965561215307, 11.369826732190017),
            "Voc[2] after harmonics",
        );
        close(
            Complex64::new(worst, 0.0),
            Complex64::new(12.329071484855646, 0.0),
            "max |Voc| after harmonics",
        );
    }
}

#[cfg(test)]
mod bus_reliability_tests {
    use super::*;

    /// The two-section radial feeder of `exec::tests::reliability::ocp_feeder`
    /// with the recloser on the *second* line, so the forward sweep opens
    /// section 1 below `b1`, and with `l2` twice as long so the miles
    /// accumulators differ from the failure rates. Every one of the eight
    /// columns then takes a value no other column takes on the same bus, which
    /// is what makes the table below a mapping test and not a smoke test.
    fn relcalc_feeder() -> Dss {
        let mut dss = Dss::new();
        dss.command("New circuit.busrel basekv=12.47 bus1=src phases=3");
        dss.command(
            "New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 \
                 faultrate=0.2 pctperm=80 repair=4",
        );
        dss.command(
            "New line.l2 bus1=b1 bus2=b2 length=2 units=mi r1=0.1 x1=0.1 \
                 faultrate=0.3 pctperm=90 repair=5",
        );
        dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
        dss.command("New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25");
        dss.command(
            "New recloser.r1 monitoredobj=line.l2 monitoredterm=1 \
                 switchedobj=line.l2 switchedterm=1",
        );
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        dss.command("Solve mode=snap");
        dss.command("Relcalc");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    }

    /// Every column reads the `TDSSBus` field it names, and no other. The three
    /// rows are the whole feeder after `RelCalc`, pinned as `Debug` strings
    /// (shortest round-trip, so a last-ULP move reds too):
    ///
    /// * `src` — `lambda_ = 0.16` (`l1`'s `BranchFltRate = 0.2·80%·1 mi`; `l2`'s
    ///   0.54 stops at the recloser, r4133
    ///   `Version8/Source/PDElements/PDElement.pas:114-117`), `total_miles = 3`
    ///   (1 + 2), `n_customers = 35`.
    /// * `b1` — 25 customers and 2 miles below it, no accumulated rate (the OCP
    ///   device on `l2` isolates it).
    /// * `b2` — section 1, `n_interrupts = 0.54`, `int_duration = 5`
    ///   (`Source_IntDuration = 0` + the section's `AverageRepairTime`, r4133
    ///   `Version8/Source/Meters/EnergyMeter.pas:2571-2573`) and
    ///   `cust_duration = (0 + 25)·1·5·0.54 = 67.5` (`:2610-2611`).
    ///
    /// The six `f64` columns hold six pairwise-different vectors and the two
    /// `i32` ones two more, so any transposition of two columns moves at least
    /// one of these rows.
    #[test]
    fn bus_reliability_maps_every_column_to_its_own_bus_field() {
        let dss = relcalc_feeder();
        let rows: Vec<String> = dss
            .bus_reliability()
            .iter()
            .map(|r| format!("{r:?}"))
            .collect();
        assert_eq!(
            rows,
            vec![
                "BusReliabilityView { name: \"src\", cust_duration: 0.0, \
                 cust_interrupts: 0.0, int_duration: 0.0, lambda_: 0.16, \
                 n_customers: 35, n_interrupts: 0.0, section_id: 0, \
                 total_miles: 3.0 }",
                "BusReliabilityView { name: \"b1\", cust_duration: 0.0, \
                 cust_interrupts: 0.0, int_duration: 0.0, lambda_: 0.0, \
                 n_customers: 25, n_interrupts: 0.0, section_id: 0, \
                 total_miles: 2.0 }",
                "BusReliabilityView { name: \"b2\", cust_duration: 67.5, \
                 cust_interrupts: 0.0, int_duration: 5.0, lambda_: 0.0, \
                 n_customers: 0, n_interrupts: 0.54, section_id: 1, \
                 total_miles: 0.0 }",
            ]
        );
    }

    /// The walk is `BusList` order — the same sequence, spelling and length as
    /// [`Dss::all_bus_voltages`], which is what lets the live comparator align
    /// the two channels' rows by index — and it is total on a circuit-less
    /// engine.
    #[test]
    fn bus_reliability_walks_the_bus_list_and_survives_no_circuit() {
        let dss = relcalc_feeder();
        let rows = dss.bus_reliability();
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["src", "b1", "b2"]);
        let voltage_names: Vec<String> =
            dss.all_bus_voltages().into_iter().map(|v| v.name).collect();
        assert_eq!(names, voltage_names);

        assert!(Dss::new().bus_reliability().is_empty());
    }

    /// Without a reliability sweep every column is at its `TDSSBus.Create`
    /// default — `0`/`0.0`, never the `-1` that `ZeroReliabilityAccums` parks
    /// in `section_id` mid-sweep (port `circuit/bus.rs`, Pascal
    /// `TDSSBus.Create` / `TDSSBus.ZeroReliabilityAccums`). Both oracles report
    /// the same zeros there, so a capture taken before `RelCalc` is comparable
    /// rather than undefined.
    #[test]
    fn bus_reliability_is_zero_before_relcalc() {
        let mut dss = Dss::new();
        dss.command("New circuit.busrel0 basekv=12.47 bus1=src phases=3");
        dss.command("New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.2");
        dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        dss.command("Solve mode=snap");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        for r in dss.bus_reliability() {
            assert_eq!(
                format!("{r:?}"),
                format!(
                    "BusReliabilityView {{ name: {:?}, cust_duration: 0.0, \
                     cust_interrupts: 0.0, int_duration: 0.0, lambda_: 0.0, \
                     n_customers: 0, n_interrupts: 0.0, section_id: 0, \
                     total_miles: 0.0 }}",
                    r.name
                )
            );
        }
    }
}
