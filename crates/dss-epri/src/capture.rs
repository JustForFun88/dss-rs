//! `CaseResult` assembly — a byte-for-byte port of
//! `tools/oracle/oracle_server.py::run_case` (+ the `capture_*` helpers it shares
//! with `tools/golden/gen_checkpoints.py`) against the raw r4133 DLL.
//!
//! The response is JSON-shape-identical to the retired Oddie oracle's
//! (`CaseResult { node_order, n_steps, checkpoints, autoadd_log }`), so the Rust
//! gate's `serde` deserialize accepts it unchanged (bit-diff-proven against the
//! Python path by `xcheck_bridge.py`, itself retired with that stack in Phase
//! E). Read order within a step matches `oracle_server` exactly — notably the
//! §1.1(a)/D3 group-A-before-group-B rule in the element capture
//! (Losses, Powers, then Currents: the harmonics stale-`Iterminal` ordering,
//! CLAUDE.md), which every read line declares with a `capture-order:` marker.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::dss::{Engine, EngineError, RelCalcResult};
use crate::guard::CorpusGuard;

/// Retry a non-converged case in-process up to this many times
/// (`oracle_server._RUN_ATTEMPTS`): absorbs the engine's occasional
/// fresh-process convergence misfire without masking a real non-convergence.
const RUN_ATTEMPTS: usize = 3;

/// The user-written-model `DoSimpleMsg` errnos the official Direct DLL warns on
/// and solves through — [`crate::dss::USER_MODEL`], the single Rust-side
/// definition (mirrored on the capi transport by
/// `oracle_server._USER_MODEL_ERRNOS`).
///
/// Needed here because G1.9 made the group-A aggregates the FIRST post-solve
/// read that recomputes `Iterminal`, so on a `warn_and_continue` deck the single
/// priming warning now fires inside [`capture_aggregates`] instead of
/// `Engine::element_pcl`. G1.9's hand-synced local copy was folded into the
/// `dss` one at the 2026-09-05 lane merge (its "dedup at merge" handoff).
use crate::dss::USER_MODEL as USER_MODEL_ERRNOS;

// ---------------------------------------------------------------------------
// Request (deserialized from the line-JSON `run` message).
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RunRequest {
    pub case_path: String,
    #[serde(default)]
    pub post: Vec<String>,
    #[serde(default = "one")]
    pub n_steps: usize,
    #[serde(default)]
    pub selected_elements: Vec<String>,
    #[serde(default)]
    pub check_meters_monitors: bool,
    /// Capture the `PDElements` walk (GOLDEN_REBASE G1.6b). The capi channel
    /// spells the same request key (`oracle_server.run_case`), so the two
    /// transports are requested and ordered identically.
    #[serde(default)]
    pub pd_elements: bool,
    /// Drive the executive `RelCalc` and capture the reliability surface
    /// (GOLDEN_REBASE G1.6(i)). The capi channel spells the same request key
    /// (`oracle_server.run_case`), so both transports run the command at the
    /// same point of the same step and read the same fields.
    ///
    /// This is the only request flag that *changes how the case is run* rather
    /// than only what is read: without it no live-compared corpus deck ever
    /// executes `CalcReliabilityIndices`, and the whole reliability half would
    /// compare `0 == 0`.
    #[serde(default)]
    pub reliability: bool,
    #[serde(default)]
    pub probes: Vec<ProbeSpec>,
    #[serde(default)]
    pub variables: Vec<String>,
    #[serde(default)]
    pub eventlog: bool,
    #[serde(default)]
    pub ctrlqueue: bool,
    /// G1.4a `compare_bus`: capture every bus's voltage surface
    /// ([`capture_all_buses`]) plus the checkpoint-level `AllBusVmagPu`.
    #[serde(default)]
    pub buses: bool,
    /// G1.5 `compare_zsc`: append the six short-circuit arms
    /// (`Zsc1`/`Zsc0`/`ZscMatrix`/`YscMatrix`/`Isc`/`Voc`) to the ONE per-bus
    /// walk [`Self::buses`] drives — never a second `SetActiveBus` pass.
    /// Requires [`Self::buses`]: without it the walk does not run at all, so
    /// [`run_case`] refuses the malformed request instead of silently shipping
    /// an empty surface (the gate asserts the implication one level up, in
    /// `corpus_gate::engines::build_run_request`).
    #[serde(default)]
    pub zsc: bool,
    #[serde(default)]
    pub all_properties: bool,
    /// Manifest flag `compare_derived` (GOLDEN_REBASE G1.3a): capture
    /// `CktElement.Enabled` for every element and the three polar channels
    /// `CurrentsMagAng` / `Residuals` / `VoltagesMagAng` for the **enabled**
    /// ones. Absent or `false` ⇒ none of the seven keys is emitted and the
    /// reply is byte-identical to a pre-G1.3a one.
    #[serde(default)]
    pub derived: bool,
    /// Manifest flag `compare_element_extras` (GOLDEN_REBASE G1.3d): capture
    /// `CktElement.Enabled`, the discrete index/name scalars
    /// `NumTerminals` / `NumConductors` / `NumPhases` / `EnergyMeter` (part (i)),
    /// the five control-derived scalars `NumControls` / `OCPDevIndex` /
    /// `OCPDevType` / `HasVoltControl` / `HasSwitchControl` and `PhaseLosses`
    /// (part (ii)) for every element, and `NodeOrder` for the ones that are
    /// enabled with at least one terminal. Absent or `false` ⇒ none of those
    /// keys is emitted and the reply is byte-identical to a pre-G1.3d one.
    #[serde(default)]
    pub element_extras: bool,
    /// `GOLDEN_REBASE_PLAN.md` G1.7 — the six order-free `Topology` reads.
    #[serde(default)]
    pub topology: bool,
    #[serde(default)]
    pub global_result: bool,
    #[serde(default)]
    pub autoadd_log: bool,
    #[serde(default)]
    pub warn_and_continue: bool,
    // `full_csc` is accepted but ignored: the gate always requests it (true) and
    // the r4133 CSC export is proven solution-neutral by the smoke self-test.
    #[serde(default)]
    #[allow(dead_code)]
    pub full_csc: Option<bool>,
    #[serde(default)]
    #[allow(dead_code)]
    pub cmd: Option<String>,
}

fn one() -> usize {
    1
}

#[derive(Debug, Deserialize)]
pub struct ProbeSpec {
    pub element: String,
    #[serde(default)]
    pub props: Vec<String>,
}

// ---------------------------------------------------------------------------
// Response shapes (serialized — must match oracle_server.py field-for-field).
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct CaseResult {
    node_order: Vec<String>,
    n_steps: usize,
    checkpoints: Vec<Checkpoint>,
    autoadd_log: Option<String>,
}

#[derive(Serialize)]
struct Checkpoint {
    dbl_hour: f64,
    iterations: i32,
    converged: bool,
    v_re: Vec<f64>,
    v_im: Vec<f64>,
    y: Option<YMat>,
    y_fingerprint: YFingerprint,
    yprims: Vec<YPrim>,
    elements: Vec<ElementCap>,
    injection: Injection,
    transformers: BTreeMap<String, Vec<f64>>,
    regcontrols: BTreeMap<String, i32>,
    capacitors: BTreeMap<String, Vec<i32>>,
    monitors: Vec<MonitorCap>,
    meters: Vec<MeterCap>,
    /// `None` when the run did not request `RelCalc`; `Some(_)` on **exactly
    /// one** checkpoint of a requesting run — the last one (see
    /// [`capture_reliability`]). Sits between `meters` and `pd_elements`
    /// because that is where the reads happen.
    reliability: Option<ReliabilityCap>,
    /// `None` when the run did not request the walk — distinct from `Some([])`,
    /// "requested, and this circuit has no enabled PD element" (96 of the 372
    /// walked live cases). The gate's capture guard refuses `None` on a case
    /// whose manifest set the flag.
    pd_elements: Option<Vec<PdElementCap>>,
    probes: Vec<ProbeCap>,
    variables: Vec<VariablesCap>,
    eventlog: Vec<String>,
    ctrlqueue: Vec<String>,
    buses: Vec<BusCap>,
    all_bus_vmag_pu: Vec<f64>,
    all_properties: Vec<PropsCap>,
    global_result: String,
    aggregates: AggregatesCap,
    solution_scalars: SolutionScalarsCap,
    topology: Option<TopologyCap>,
}

#[derive(Serialize)]
struct YMat {
    n: usize,
    rows: Vec<i32>,
    cols: Vec<i32>,
    re: Vec<f64>,
    im: Vec<f64>,
}

#[derive(Serialize)]
struct YFingerprint {
    nnz: usize,
    frob: f64,
    tr_re: f64,
    tr_im: f64,
    maxdiag: f64,
}

#[derive(Serialize)]
struct YPrim {
    name: String,
    yorder: usize,
    re: Vec<f64>,
    im: Vec<f64>,
}

#[derive(Serialize)]
struct ElementCap {
    name: String,
    i_re: Vec<f64>,
    i_im: Vec<f64>,
    p_kw: Vec<f64>,
    p_kvar: Vec<f64>,
    loss_w: Vec<f64>,
    // The GOLDEN_REBASE G1.3a derived channels, emitted only under
    // `RunRequest::derived` — every one is skipped when empty/absent, so an
    // off-flag reply keeps the byte-for-byte shape it had before G1.3a
    // (`oracle_server.capture_all_elements` emits exactly the same keys).
    /// `CktElement.Enabled` — present for EVERY element under the flag, so the
    /// enabled-only polar capture below can never silently drop an element.
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    /// `CurrentsMagAng`, de-interleaved into magnitude (A) and angle (degrees)
    /// the way `i_re`/`i_im` already are, so the comparator never does stride-2
    /// index arithmetic.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cma_mag: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cma_ang: Vec<f64>,
    /// `Residuals` — one `(magnitude, angle)` pair per terminal.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    res_mag: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    res_ang: Vec<f64>,
    /// `VoltagesMagAng` — magnitude (V) and angle (degrees) per conductor.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    vma_mag: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    vma_ang: Vec<f64>,
    // The GOLDEN_REBASE G1.3d(i) discrete extras, emitted only under
    // `RunRequest::element_extras` — every one is skipped when empty/absent, so
    // an off-flag reply keeps the byte-for-byte shape it had before G1.3d(i)
    // (`oracle_server.capture_all_elements` emits exactly the same keys).
    /// `NumTerminals` / `NumConductors` / `NumPhases` — present for EVERY
    /// element under the flag (all three are pure field reads on both engines).
    #[serde(skip_serializing_if = "Option::is_none")]
    n_terms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    n_conds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    n_phases: Option<i32>,
    /// `EnergyMeter`, RAW: `"0"` is this channel's "no meter" sentinel
    /// (`DDLL/DCktElement.pas:421`) where capi spells the same state `""`.
    #[serde(skip_serializing_if = "Option::is_none")]
    energy_meter: Option<String>,
    /// `NodeOrder` — bus-local node number per conductor per terminal, read
    /// only for an `Enabled` element with `NumTerminals > 0` (see
    /// [`capture_all_elements`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    node_order: Vec<i32>,
    // The GOLDEN_REBASE G1.3d(ii) additions, emitted under the same
    // `RunRequest::element_extras` flag and skipped when empty/absent, so an
    // off-flag reply keeps the byte-for-byte shape it had before G1.3d(ii)
    // (`oracle_server.capture_all_elements` emits exactly the same keys).
    /// `PhaseLosses`, de-interleaved into kW and kvar the way `p_kw`/`p_kvar`
    /// already are. Length `NPhases` each — empty for a 0-phase element
    /// (`UPFCControl`), which is why the pair is skipped-when-empty rather than
    /// `Option`: an empty capture and an empty reading are the same fact here.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pl_kw: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pl_kvar: Vec<f64>,
    /// The five control-derived scalars [`Engine::element_extras`] reads —
    /// present for EVERY element under the flag (none of them is conditional).
    #[serde(skip_serializing_if = "Option::is_none")]
    num_controls: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ocp_dev_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ocp_dev_type: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    has_volt_control: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    has_switch_control: Option<bool>,
}

#[derive(Serialize)]
struct Injection {
    re: Vec<f64>,
    im: Vec<f64>,
}

#[derive(Serialize)]
struct MonitorCap {
    name: String,
    header: Vec<String>,
    sample_count: i64,
    channels: Vec<Vec<f64>>,
}

#[derive(Serialize)]
struct MeterCap {
    name: String,
    register_names: Vec<String>,
    register_values: Vec<f64>,
    n_branches: usize,
    n_ends: usize,
    n_pce: usize,
    branches: Vec<String>,
    ends: Vec<String>,
    pce: Vec<String>,
}

/// The reliability surface of one case (GOLDEN_REBASE G1.6(i)), captured once —
/// on the last checkpoint, right after the executive `RelCalc` ran.
///
/// Key names and types are the contract on the wire (the worker serializes
/// through `serde_json::Value`, which sorts object keys, so JSON order carries
/// no meaning); they match `oracle_server.capture_reliability` field for field,
/// which is what the gate's harness deserializes on both channels.
#[derive(Serialize)]
pub struct ReliabilityCap {
    /// The `RelCalc` hit the tolerated `52902` abort ([`Engine::relcalc`]).
    pub aborted: bool,
    /// The abort message, verbatim; empty when it did not abort.
    pub message: String,
    /// Every **enabled** meter, in `Meters.First`/`Next` order.
    pub meters: Vec<MeterReliabilityCap>,
    /// `Meters.Totals` — read LAST, see [`capture_reliability`].
    pub totals: Vec<f64>,
}

/// One meter's reliability record. Field order here is the *shape* contract, not
/// the read order — see [`capture_reliability`], which reads in
/// `IMeters._columns` order.
#[derive(Serialize)]
pub struct MeterReliabilityCap {
    pub name: String,
    /// `MetersI(20)`: `BusTotalNumCustomers` of the first sequence-list
    /// element's `FromTerminal` bus (`DMeters.pas:232-242`).
    pub total_customers: i32,
    pub saifi: f64,
    /// `MetersF(1)`; the API spells it `SAIFIkW`.
    pub saifikw: f64,
    pub saidi: f64,
    pub cust_interrupts: f64,
    /// `MetersV(6)`: `Cabs(CalculatedCurrent[k+1])`, `k = 0..NPhases-1` —
    /// **no** `MeteredTerminal` offset (`DMeters.pas:609-624`), unlike the
    /// writer `TMeterElement.CalcAllocationFactors`.
    pub calc_current: Vec<f64>,
    /// `MetersV(8)`: `PhsAllocationFactor[1..NPhases]` (`DMeters.pas:645-661`).
    pub alloc_factors: Vec<f64>,
    /// The three zone lists, **ordered** as the DDLL's `BranchList` walk emits
    /// them (`DMeters.pas:706-734`, `:682-705`, `:735-762`); the existing
    /// `meters` capture compares the same names as a set.
    pub branches: Vec<String>,
    pub ends: Vec<String>,
    pub pce: Vec<String>,
    pub num_sections: i32,
    pub sections: Vec<FeederSectionCap>,
}

/// One feeder section of one meter, read behind its own
/// `Meters.SetActiveSection`.
#[derive(Serialize)]
pub struct FeederSectionCap {
    /// The 1-based index handed to `Meters.SetActiveSection`.
    pub idx: i32,
    pub num_section_customers: i32,
    pub num_section_branches: i32,
    pub sect_seq_idx: i32,
    pub sect_total_cust: i32,
    /// 1 = Fuse, 2 = Recloser, 3 = Relay (`EnergyMeter.pas`); 0 = none.
    pub ocp_device_type: i32,
    pub sum_branch_flt_rates: f64,
    /// `SumFltRatesXRepairHrs / SumBranchFltRates`, **unguarded** on every
    /// engine — a section with no fault rate is `0/0 = NaN` identically
    /// everywhere.
    pub avg_repair_time: f64,
    pub fault_rate_x_repair_hrs: f64,
}

/// One enabled PD element's `PDElements` record (GOLDEN_REBASE G1.6b): the
/// thirteen columns `IPDElements._columns` compares wholesale on
/// `DSS-Python origin/fastdss:dss/IPDElements.py`, plus `parent_name` — the
/// parent's full name, which `ParentPDElement`'s active-element hijack hands
/// out for free and which is immune to the `ClassIndex` ambiguity a bare
/// per-class index carries.
///
/// Field order **is** the read order, and the read order is the contract: the
/// twelve order-free reads first, then `parent_class_index`
/// (`DPDELements.pas:88-97` reassigns `ActiveCktElement` to the parent and never
/// restores it), then the parent's name off that hijacked cursor. The key
/// *names* and types are the contract on the wire — the worker's
/// `serde_json::Value` sorts object keys, so JSON order carries no meaning —
/// and they match `oracle_server.capture_pd_elements` field for field, which is
/// what `harness::PdElementCap` deserializes on both channels.
#[derive(Serialize)]
pub struct PdElementCap {
    pub name: String,
    pub accumulated_l: f64,
    /// 1-based, as the DDLL reports it (`TPDElement.FromTerminal`); 0 = unset.
    pub from_terminal: i32,
    /// `PDElementsI(3)` answers 0/1; normalized to the `bool` the capi channel
    /// returns, so the two transports agree on the JSON shape.
    pub is_shunt: bool,
    pub num_customers: i32,
    pub section_id: i32,
    pub fault_rate: f64,
    pub repair_time: f64,
    /// `AccumulatedMilesDownStream` (`DPDELements.pas:201-209`) — a different
    /// quantity from `Bus.TotalMiles`.
    pub total_miles: f64,
    pub total_customers: i32,
    pub pct_permanent: f64,
    pub lambda: f64,
    /// The parent's `ClassIndex` (1-based, per-class creation order), 0 = none.
    pub parent_class_index: i32,
    /// The parent's full `Class.Name`, `""` when there is no parent.
    pub parent_name: String,
}

#[derive(Serialize)]
struct ProbeCap {
    element: String,
    prop: String,
    value: String,
}

#[derive(Serialize)]
struct VariablesCap {
    name: String,
    var_names: Vec<String>,
    values: Vec<f64>,
}

/// One bus's captured voltage surface — the `compare_bus` wire shape
/// (GOLDEN_REBASE_PLAN.md WP-G1 sub-step G1.4a), serialized field-for-field as
/// `tools/oracle/oracle_server.py::capture_all_buses` emits it and
/// `corpus_gate::engines::BusCap` deserializes it.
///
/// Parity target: fastdss' `IBus._columns` (`origin/fastdss` `dss/IBus.py:19-53`,
/// reached through `save_state`'s `ActiveBus`, `tests/save_outputs.py:351`).
/// The first four surfaces below are the ones both gating channels compute
/// with the identical algorithm. The four G1.4c arms after the short-circuit
/// block — `SeqVoltages`/`CplxSeqVoltages` and `VLL`/`puVLL` — do NOT: the
/// two channels split structurally on the bus's node set (this transport has
/// no `Nvalues > 3` clamp on the sequence arms and its L-L pairing loop can
/// hang, see [`BusCap::vll_declined`]), so the comparator recognizes each
/// shape from the port's own node numbers rather than comparing sentinels.
///
/// All three VOLTAGE arrays are `2 * nodes.len()` doubles in ONE order --
/// **ascending node number** — and never the bus's internal insertion order:
/// see [`crate::modes::BUS_NODES`] for the `FindIdx` walk all four arms share.
/// The six G1.5 short-circuit arrays below are the other convention — the bus's
/// INTERNAL node index — and are captured only when the request sets `zsc`
/// ([`RunRequest::zsc`]); see [`capture_all_buses`].
#[derive(Serialize)]
struct BusCap {
    /// `Circuit.AllBusNames` entry i, re-asserted as the active bus by
    /// `SetActiveBus`'s returned index (`DCircuit.pas:439`, `:247-250`).
    name: String,
    /// `Bus.kVBase` in kV (`BUSF(0)`). Both engines take
    /// `BaseFactor = 1000 * kVBase` when positive, else `1.0`
    /// (`DBus.pas:413-414` == `CAPI_Alt.pas:2262-2265`).
    kv_base: f64,
    /// `Bus.Nodes` — node numbers, ascending (`BUSV(2)`, `DBus.pas:319-345`).
    nodes: Vec<i32>,
    /// `Bus.puVoltages` — `NodeV[GetRef]/BaseFactor`, interleaved `(re, im)`
    /// (`BUSV(5)`, `DBus.pas:399-430` == `CAPI_Alt.pas:2251-2280`).
    pu_voltages: Vec<f64>,
    /// `Bus.VMagAngle` — interleaved `(magnitude V, angle deg)`
    /// (`BUSV(13)`, `DBus.pas:659-689` == `CAPI_Alt.pas:2573-2597`).
    vmag_angle: Vec<f64>,
    /// `Bus.puVMagAngle` — the same pairs with only the magnitude divided by
    /// `BaseFactor` (`BUSV(14)`, `DBus.pas:690-723` == `CAPI_Alt.pas:2540-2571`).
    pu_vmag_angle: Vec<f64>,
    /// `Bus.Zsc1` = `Zs - Zm`, ONE complex = 2 doubles, always (`BUSV(7)`,
    /// `DBus.pas:461-474` == `CAPI_Alt.pas:2294-2303`; both write the
    /// 1-element array unconditionally). `cZERO` while `Zsc` is unassigned
    /// (`Common/Bus.pas:222-229` == capi `:225-232`). `AvgOffDiagonal` divides
    /// only `If Ntimes > 0` (`Shared/Ucmatrix.pas:369-383` == capi `:372-387`),
    /// so a 1-node bus has `Zm = 0` and `zsc1 == zsc0 == zsc[0]`.
    zsc1: Vec<f64>,
    /// `Bus.Zsc0` = `Zs + 2*Zm`, same shape and guard (`BUSV(8)`,
    /// `DBus.pas:476-489` == `CAPI_Alt.pas:2283-2292`).
    zsc0: Vec<f64>,
    /// `Bus.ZscMatrix` — row-major (`i` outer, `j` inner) `2*n*n` doubles
    /// (`BUSV(6)`, `DBus.pas:431-459` == `CAPI_Alt.pas:2305-2334`), or the
    /// [`R4133_SC_SENTINEL_LEN`] sentinel while the bus has no matrix.
    zsc: Vec<f64>,
    /// `Bus.YscMatrix` — the same shape, `Ysc = Zsc^-1` (`BUSV(9)`,
    /// `DBus.pas:491-518` == `CAPI_Alt.pas:2336-2365`).
    ysc: Vec<f64>,
    /// `Bus.Isc` — `BusCurrent`, `2*n` doubles (`BUSV(4)`, `DBus.pas:374-397`
    /// == `CAPI_Alt.pas:2202-2224`).
    isc: Vec<f64>,
    /// `Bus.Voc` — `VBus`, `2*n` doubles (`BUSV(3)`, `DBus.pas:351-372` ==
    /// `CAPI_Alt.pas:2227-2249`). Refreshed by the fault study AND by
    /// `BuildYMatrix` under `PreserveNodeVoltages` (`Ymatrix.pas:170`), so it
    /// is live on harmonics/dynamics decks too.
    voc: Vec<f64>,
    /// `Bus.SeqVoltages` — the three `Cabs(V012[i])` magnitudes, ALWAYS 3
    /// doubles: the arm publishes `SizeOf(double) * 3` unconditionally
    /// (`BUSV(1)`, `DBus.pas:284-317`, `:315-316`). `-1.0` x3 whenever
    /// `NumNodesThisBus <> 3` (`:296-297`) — and, unlike capi
    /// (`CAPI_Alt.pas:2172-2186`), with NO `Nvalues > 3` clamp, so the two
    /// channels split on a bus with more than three nodes.
    seq_voltages: Vec<f64>,
    /// `Bus.CplxSeqVoltages` — the same three components complex, 6 doubles
    /// (`BUSV(10)`, `DBus.pas:520-547`), `cmplx(-1,-1)` x3 under the same
    /// `Nvalues <> 3` test (`:531-532`) and the same missing clamp.
    cplx_seq_voltages: Vec<f64>,
    /// `Bus.VLL` — line-to-line voltages (`BUSV(11)`, `DBus.pas:549-601`):
    /// 6 doubles (three pairs) on a bus with `>= 3` nodes, 2 doubles on any
    /// other — one pair when it has exactly 2 (`Nvalues = 2 => 1`, `:563`), or
    /// the `cmplx(-99999, 0)` marker of the `Nvalues <= 1` branch (`:594`).
    /// EMPTY when [`Self::vll_declined`].
    vll: Vec<f64>,
    /// `Bus.puVLL` — the same pairs over `BaseFactor_LL = 1000*kVBase*sqrt3`,
    /// or `1.0` when `kVBase <= 0` (`BUSV(12)`, `DBus.pas:603-657`, `:622-623`
    /// == `CAPI_Alt.pas:2427-2430`). Same shape and same emptiness rule as
    /// [`Self::vll`]: one guard decides both.
    pu_vll: Vec<f64>,
    /// THIS transport refused to compute `VLL`/`puVLL` for this bus: r4133's
    /// pairing loop probes `jj` before wrapping it and would not terminate on
    /// this node set ([`crate::modes::bus_vll_would_hang`],
    /// `DBus.pas:580-584`), so neither mode was dispatched and both arrays
    /// are empty. A hang yields no oracle value at all, which is why this is
    /// a captured FACT the comparator asserts against its own replay of the
    /// walk — never a silent gap. Always `false` on the capi transport, whose
    /// second loop is the bounded `for k := 1 to 3` of `CAPI_Alt.pas:2500`.
    vll_declined: bool,
}

/// What THIS transport publishes for `Bus.ZscMatrix`/`Bus.YscMatrix` — and for
/// `Bus.Isc`/`Bus.Voc` on a 0-node bus — when the underlying pointer is nil:
/// the `setlength(myCmplxArray, 1); myCmplxArray[0] := CZero` prelude every
/// `BUSV` arm opens with, i.e. **2** doubles (`DDLL/DBus.pas:433-434` for
/// `Zsc`, `:493-494` for `Ysc`, `:353-354` for `Voc`, `:376-377` for `Isc`).
///
/// The capi transport publishes ONE double there instead (`DefaultResult`,
/// `CAPI/CAPI_Utils.pas:212-221` under `DSS_CAPI_COM_DEFAULTS`) — and, for
/// `Isc`/`Voc` at a 0-node bus, ZERO doubles, because capi's
/// `TDSSBus.AllocateBusState` uses `AllocMem` (`Common/Bus.pas:250-256`),
/// whose 0-byte block is non-nil, while r4133's `Reallocmem(VBus, 0)`
/// (`Common/Bus.pas:246-260`) frees the pointer. Measured on
/// `Test/REACTORTest.DSS` / `Test/Source012Test.dss` (`loadbus2`): capi
/// `(isc, voc) = (0, 0)` vs r4133 `(2, 2)`. The comparator normalizes both
/// sentinel shapes to "no matrix" / "no nodes"; they are never compared as
/// values.
const R4133_SC_SENTINEL_LEN: usize = 2;

/// One element's every-property dump (§2.2 all-properties parity — a **gating**
/// capture since R4133_PROPS RP4.1, 2026-09-03; report tooling only before it).
/// Serializes to the exact shape
/// `oracle_server.capture_all_properties` emits and `harness::PropsCap`
/// deserializes: `{"element": name, "props": [[prop, value], ...]}` in
/// `AllPropertyNames` (property-index) order.
#[derive(Serialize)]
pub struct PropsCap {
    pub element: String,
    pub props: Vec<(String, String)>,
}

/// The five `Circuit` aggregates of `GOLDEN_REBASE_PLAN.md` G1.9, in the exact
/// shape `oracle_server.capture_aggregates` emits.
///
/// The units are in the key names because r4133 does not scale them uniformly:
/// `Circuit.Losses` (`DDLL/DCircuit.pas:294` -> `Common/Circuit.pas:2436-2443`)
/// is raw **W/var**, while `LineLosses` (`DCircuit.pas:305-325`),
/// `SubstationLosses` (`:327-347`), `TotalPower` (`:349-368`) and
/// `AllElementLosses` (`:458-479`) all carry the arm's own
/// `cmulreal(..., 0.001)` and are kW/kvar.
#[derive(Serialize)]
struct AggregatesCap {
    losses_w: Vec<f64>,
    line_losses_kw: Vec<f64>,
    substation_losses_kw: Vec<f64>,
    total_power_kw: Vec<f64>,
    all_element_losses_kw: Vec<f64>,
}

/// The ten `Solution` scalars of G1.9, in the exact shape
/// `oracle_server.capture_solution_scalars` emits.
///
/// `iterations` and `dbl_hour` are deliberately absent — [`Checkpoint`] already
/// carries and the gate already compares them.
#[derive(Serialize)]
struct SolutionScalarsCap {
    mode: i32,
    hour: i32,
    year: i32,
    control_iterations: i32,
    total_iterations: i32,
    most_iterations_done: i32,
    /// `SolutionI(42)` is a `0|1` int (`DSolution.pas:226-230`); normalized to
    /// the capi transport's JSON `bool` here, at the bridge.
    control_actions_done: bool,
    /// `SolutionI(37)`, same `0|1` normalization (`DSolution.pas:192-197`).
    system_y_changed: bool,
    seconds: f64,
    load_mult: f64,
}

/// The six `Topology` quantities of `GOLDEN_REBASE_PLAN.md` G1.7, in the exact
/// shape `oracle_server.capture_topology` emits (identical JSON keys, identical
/// normalized list shape) — the six rows of `DDLL/DTopology.pas` that never
/// touch `ActiveCircuit.ActiveCktElement`. See [`capture_topology`].
///
/// `looped_pairs` is FLAT, `[a0, b0, a1, b1, ...]`: `TopologyV(0)`
/// (`DTopology.pas:270-305`) writes the two `QualifiedName`s of one looped pair
/// as two consecutive entries. The comparator pairs them up; the count is NOT
/// its length — `num_loops` is the `IsLoopedHere` tally halved
/// (`DTopology.pas:65-73`, `Result := Result div 2`), so IEEE13 answers
/// `num_loops = 1` with three pairs.
#[derive(Serialize)]
struct TopologyCap {
    num_loops: i32,
    num_isolated_branches: i32,
    num_isolated_loads: i32,
    looped_pairs: Vec<String>,
    isolated_branches: Vec<String>,
    isolated_loads: Vec<String>,
}

// ---------------------------------------------------------------------------
// The run.
// ---------------------------------------------------------------------------

/// Compile one deck, solve `n_steps` times, capture the full per-step model.
pub fn run_case(engine: &Engine, req: &RunRequest) -> Result<CaseResult, EngineError> {
    let warn = req.warn_and_continue;
    let star = req.selected_elements == ["*"];

    let _guard = CorpusGuard::new(&req.case_path);

    let mut node_order: Vec<String> = Vec::new();
    let mut checkpoints: Vec<Checkpoint> = Vec::new();

    for attempt in 1..=RUN_ATTEMPTS {
        node_order.clear();
        checkpoints = Vec::with_capacity(req.n_steps);
        engine.clear()?;
        engine.compile(&req.case_path, warn)?;
        for c in &req.post {
            engine.post(c)?;
        }
        for step in 0..req.n_steps {
            let reply = engine.solve(warn)?;
            let global_result = if req.global_result {
                reply
            } else {
                String::new()
            };

            // GOLDEN_REBASE G1.6(i): the executive `RelCalc` runs exactly ONCE
            // per case — on the LAST step, after the solve reply has been read
            // (`RelCalc` overwrites `Text.Result` with its own empty reply) and
            // BEFORE every capture of this checkpoint, which is the slot
            // `oracle_server.run_case` uses on the capi transport. It is not
            // idempotent (`Bus.TotalMiles` accumulates across calls), so a
            // per-step drive would be semantically wrong, not merely slower.
            let relcalc = if req.reliability && step + 1 == req.n_steps {
                Some(engine.relcalc()?)
            } else {
                None
            };

            // G1.9 (`GOLDEN_REBASE_PLAN.md`, §1.1(a) + decision D3) — the circuit
            // aggregates and the solution scalars, read HERE and nowhere later,
            // for two independent reasons:
            //  * group A before group B. Every aggregate is a
            //    `Get_Losses`/`Get_Power` read, i.e. a `ComputeIterminal`
            //    (`Common/CktElement.pas:743` / `:677-680`) over the elements it
            //    walks, while `capture_all_elements` below issues Powers *then*
            //    `Currents` per element — and `Currents` is the read that fills a
            //    scratch buffer. Group A therefore runs first.
            //  * cursor hygiene. `Losses` walks PDElements, `LineLosses` walks
            //    Lines, `SubstationLosses` walks Transformers, `TotalPower` walks
            //    Sources and `AllElementLosses` walks CktElements
            //    (`DDLL/DCircuit.pas:294/313/335/356/468`), each leaving that
            //    `TPointerList` cursor at the end — and `capture_discrete` below
            //    drives `Transformers.First/Next`. Reading before any First/Next
            //    walk removes the interaction by construction.
            // G1.6(i)'s once-per-case `RelCalc` above precedes this block: a
            // state-changing command runs ahead of every read of the
            // checkpoint, on both transports.
            // Mirrors `oracle_server.run_case` exactly; the source order of both
            // transports is asserted by `crates/dss-core/tests/capture_order.rs`.
            let aggregates = capture_aggregates(engine, warn)?;
            let solution_scalars = capture_solution_scalars(engine)?;

            // selected_elements=["*"] -> every YPrim-bearing element (rebuilt each
            // solve so a deck that adds an element mid-solve is covered).
            let sel: Vec<String> = if star {
                let mut s = Vec::new();
                for nm in engine.all_element_names() {
                    engine.set_active_element(&nm);
                    let flat = engine.element_yprim();
                    let len = flat.len();
                    if len > 0 && is_square_yprim(len) {
                        s.push(nm);
                    }
                }
                engine.assert_clean("sel build")?;
                s
            } else {
                req.selected_elements.clone()
            };

            if node_order.is_empty() {
                node_order = engine.ynode_order();
            }
            let varray = engine.ynode_varray();
            let (v_re, v_im) = deinterleave(&varray);
            engine.assert_clean("voltages")?;

            let (transformers, regcontrols, capacitors) = capture_discrete(engine)?;

            let dbl_hour = engine.dbl_hour();
            let iterations = engine.iterations();
            let converged = engine.converged();

            let ycsc = engine.y_csc()?;
            let y = Some(build_ymat(&ycsc));
            let y_fingerprint = build_fingerprint(&ycsc);

            let mut yprims = Vec::with_capacity(sel.len());
            for nm in &sel {
                engine.set_active_element(nm);
                let flat = engine.element_yprim();
                yprims.push(build_yprim(nm, &flat));
            }
            engine.assert_clean("yprims")?;

            let elements = capture_all_elements(engine, warn, req.derived, req.element_extras)?;

            let inj = engine.injection_raw(engine.num_nodes());
            let injection = capture_injection(&inj);
            engine.assert_clean("injection")?;

            let monitors = if req.check_meters_monitors {
                capture_monitors(engine)?
            } else {
                Vec::new()
            };
            let meters = if req.check_meters_monitors {
                capture_meters(engine)?
            } else {
                Vec::new()
            };

            // The reliability reads sit between the meter walk and the
            // PD-elements walk on both transports; `Some` on exactly the step
            // whose `RelCalc` ran (the last one).
            let reliability = match &relcalc {
                Some(rel) => Some(capture_reliability(engine, rel)?),
                None => None,
            };

            // After the meters and BEFORE the probes — the slot
            // `oracle_server.run_case` gives it, so the two transports issue the
            // walk at the same point of the step. The walk is an active-element
            // mutator (`PDElements.First/Next/ParentPDElement`), so its
            // placement is fixed, not incidental.
            let pd_elements = if req.pd_elements {
                Some(capture_pd_elements(engine)?)
            } else {
                None
            };

            let probes = capture_probes(engine, &req.probes)?;
            let variables = capture_variables(engine, &req.variables)?;
            let eventlog = if req.eventlog {
                capture_eventlog(engine)?
            } else {
                Vec::new()
            };
            let ctrlqueue = if req.ctrlqueue {
                capture_ctrlqueue(engine)
            } else {
                Vec::new()
            };
            let (buses, all_bus_vmag_pu) = if req.buses {
                let buses = capture_all_buses(engine, req.zsc)?;
                let all_bus_vmag_pu = capture_all_bus_vmag_pu(engine)?;
                let nodes: usize = buses.iter().map(|b| b.nodes.len()).sum();
                if all_bus_vmag_pu.len() != nodes {
                    return Err(EngineError::Other(format!(
                        "bus capture: AllBusVmagPu has {} values but the per-bus walk saw {nodes} \
                         nodes over {} buses",
                        all_bus_vmag_pu.len(),
                        buses.len()
                    )));
                }
                (buses, all_bus_vmag_pu)
            } else if req.zsc {
                // G1.5: the six SC arms ride the per-bus walk above, so asking
                // for them without the bus surface would ship nothing at all.
                // Refuse loudly (the gate asserts the same implication in
                // `corpus_gate::engines::build_run_request`).
                return Err(EngineError::Other(
                    "request asks for the bus short-circuit surface (zsc) without the bus \
                     surface it is appended to: the six SC arms share the one per-bus walk \
                     (GOLDEN_REBASE_PLAN.md WP-G1 G1.5 section 2.a)"
                        .into(),
                ));
            } else {
                (Vec::new(), Vec::new())
            };
            // Read LAST (after every other capture), like `oracle_server.run_case`:
            // the `? name.Like`/`? name.prop` sweep perturbs the active-element
            // cursor, so it must not run before any other read (§2.2).
            let all_properties = if req.all_properties {
                capture_all_properties(engine)?
            } else {
                Vec::new()
            };
            // G1.7 — read STRICTLY LAST, after `all_properties`, on both
            // transports (`oracle_server.run_case` does the same; the source
            // order of both is asserted by
            // `crates/dss-core/tests/capture_order.rs`). Why last, and why
            // these six modes only: see [`capture_topology`].
            let topology = if req.topology {
                let cap = capture_topology(engine)?;
                engine.assert_clean("topology")?;
                Some(cap)
            } else {
                None
            };

            let _ = step;
            checkpoints.push(Checkpoint {
                dbl_hour,
                iterations,
                converged,
                v_re,
                v_im,
                y,
                y_fingerprint,
                yprims,
                elements,
                injection,
                transformers,
                regcontrols,
                capacitors,
                monitors,
                meters,
                reliability,
                pd_elements,
                probes,
                variables,
                eventlog,
                ctrlqueue,
                buses,
                all_bus_vmag_pu,
                all_properties,
                global_result,
                aggregates,
                solution_scalars,
                topology,
            });
        }

        let bad: Vec<usize> = checkpoints
            .iter()
            .enumerate()
            .filter(|(_, cp)| !cp.converged)
            .map(|(i, _)| i)
            .collect();
        if bad.is_empty() {
            break;
        }
        eprintln!(
            "epri-worker retry: {} attempt {attempt}/{RUN_ATTEMPTS} non-converged step(s) {bad:?}",
            req.case_path
        );
    }

    let autoadd_log = if req.autoadd_log {
        read_autoadd_log(engine, &req.case_path)
    } else {
        None
    };

    Ok(CaseResult {
        node_order,
        n_steps: req.n_steps,
        checkpoints,
        autoadd_log,
    })
}

/// `len` (flat re/im floats) is a square YPrim: `len == 2 * yorder^2`.
fn is_square_yprim(len: usize) -> bool {
    let half = len / 2;
    let n = (half as f64).sqrt() as usize;
    // check n and n+1 to avoid float floor error
    for cand in [n.saturating_sub(1), n, n + 1] {
        if cand > 0 && 2 * cand * cand == len {
            return true;
        }
    }
    false
}

fn deinterleave(flat: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let re = flat.iter().step_by(2).copied().collect();
    let im = flat.iter().skip(1).step_by(2).copied().collect();
    (re, im)
}

fn build_ymat(y: &crate::dss::Ycsc) -> YMat {
    let mut rows = Vec::with_capacity(y.row_idx.len());
    let mut cols = Vec::with_capacity(y.row_idx.len());
    let mut re = Vec::with_capacity(y.row_idx.len());
    let mut im = Vec::with_capacity(y.row_idx.len());
    for col in 0..y.n {
        let start = y.col_ptr[col] as usize;
        let end = y.col_ptr[col + 1] as usize;
        for k in start..end {
            rows.push(y.row_idx[k]);
            cols.push(col as i32);
            re.push(y.vals[2 * k]);
            im.push(y.vals[2 * k + 1]);
        }
    }
    YMat {
        n: y.n,
        rows,
        cols,
        re,
        im,
    }
}

/// `capture_fingerprint` (floor 1e-9): counts, Frobenius norm, complex trace,
/// max |diagonal|. Arithmetic matches the Python: `abs(x)` = `hypot(re, im)`,
/// summed in CSC storage order, `frob = sqrt(sum(abs^2))`.
fn build_fingerprint(y: &crate::dss::Ycsc) -> YFingerprint {
    const FLOOR: f64 = 1e-9;
    let mut nnz: usize = 0;
    let mut frob_acc: f64 = 0.0;
    let mut tr_re: f64 = 0.0;
    let mut tr_im: f64 = 0.0;
    let mut maxdiag: f64 = 0.0;
    for col in 0..y.n {
        let start = y.col_ptr[col] as usize;
        let end = y.col_ptr[col + 1] as usize;
        for k in start..end {
            let re = y.vals[2 * k];
            let im = y.vals[2 * k + 1];
            let mag = re.hypot(im);
            if mag > FLOOR {
                nnz += 1;
            }
            frob_acc += mag * mag;
            if y.row_idx[k] as usize == col {
                tr_re += re;
                tr_im += im;
                if mag > maxdiag {
                    maxdiag = mag;
                }
            }
        }
    }
    YFingerprint {
        nnz,
        frob: frob_acc.sqrt(),
        tr_re,
        tr_im,
        maxdiag,
    }
}

fn build_yprim(name: &str, flat: &[f64]) -> YPrim {
    let (re, im) = deinterleave(flat);
    let yorder = (re.len() as f64).sqrt().round() as usize;
    YPrim {
        name: name.to_string(),
        yorder,
        re,
        im,
    }
}

fn capture_injection(flat: &[f64]) -> Injection {
    // slot 0 = ground; drop it.
    let (re, im) = deinterleave(flat);
    Injection {
        re: re.into_iter().skip(1).collect(),
        im: im.into_iter().skip(1).collect(),
    }
}

/// Group A of the step capture (`GOLDEN_REBASE_PLAN.md` G1.9): the five
/// `Circuit` aggregates, read before any group-B (`Currents`) read and before
/// any `First/Next` walk. See the call site in [`run_case`] for the ordering
/// argument and the Pascal citations.
///
/// The retry mirrors `Engine::element_pcl`: on a `warn_and_continue` deck the
/// first post-solve `ComputeIterminal` fires one non-fatal user-model
/// `DoSimpleMsg` (#567/#570/#1570) and clears it, and since G1.9 that first
/// recompute happens here. The reads are pure, so repeating all five is
/// idempotent; anything but a tolerated errno — and any errno at all on the
/// second attempt — is returned as an error, never absorbed.
fn capture_aggregates(engine: &Engine, warn: bool) -> Result<AggregatesCap, EngineError> {
    for attempt in 0..2 {
        let losses = engine.circuit_losses()?;
        let line_losses = engine.circuit_line_losses()?;
        let substation_losses = engine.circuit_substation_losses()?;
        let total_power = engine.circuit_total_power()?;
        let all_element_losses = engine.circuit_all_element_losses()?;
        let (errno, desc) = engine.poll_error();
        if errno == 0 {
            return Ok(AggregatesCap {
                losses_w: complex_pair(&losses, "Circuit.Losses")?,
                line_losses_kw: complex_pair(&line_losses, "Circuit.LineLosses")?,
                substation_losses_kw: complex_pair(&substation_losses, "Circuit.SubstationLosses")?,
                total_power_kw: complex_pair(&total_power, "Circuit.TotalPower")?,
                all_element_losses_kw: all_element_losses,
            });
        }
        if warn && USER_MODEL_ERRNOS.contains(&errno) && attempt == 0 {
            continue; // priming read fired + cleared the warning; retry once
        }
        return Err(EngineError::Dss {
            errno,
            desc,
            ctx: "aggregates".to_string(),
        });
    }
    unreachable!()
}

/// A `myType = 3` single-element complex reply as the `[re, im]` pair the capi
/// transport emits.
///
/// The length is checked, not padded: all four `CircuitV` aggregate modes do
/// `setlength(myCmplxArray, 1)` unconditionally before any `nil` test
/// (`DDLL/DCircuit.pas:293-303`, `:305-325`, `:327-347`, `:349-368`), so a
/// reply that is not exactly two doubles is a transport failure, never a value.
/// Padding it would be indistinguishable from the true answer for
/// `SubstationLosses`, which is a legitimate `(0, 0)` on every deck without a
/// `sub=yes` transformer (G1.9 audit CODE-3 / T4).
fn complex_pair(v: &[f64], what: &str) -> Result<Vec<f64>, EngineError> {
    if v.len() != 2 {
        return Err(EngineError::Other(format!(
            "{what}: the DLL returned {} double(s) for a myType=3 complex \
             reply, expected exactly 2 (`DDLL/DCircuit.pas` sets length 1 \
             unconditionally, so this is a transport failure)",
            v.len()
        )));
    }
    Ok(vec![v[0], v[1]])
}

/// Group C of the step capture: the ten order-free `Solution` scalars of G1.9.
///
/// The two flag reads come back from r4133 as `0|1` ints
/// (`DSolution.pas:192-197` and `:226-230`, both `IF ... THEN Result := 1`), so
/// the `!= 0` here is the bridge-level normalization that keeps this transport's
/// `CaseResult` JSON byte-shape-identical to `oracle_server`'s, whose
/// dss-python reads are already Python `bool`s. Pinned by
/// `tests/modes.rs::r4133_solution_flags_are_zero_one_ints`.
fn capture_solution_scalars(engine: &Engine) -> Result<SolutionScalarsCap, EngineError> {
    let cap = SolutionScalarsCap {
        mode: engine.solution_mode()?,
        hour: engine.solution_hour()?,
        year: engine.solution_year()?,
        control_iterations: engine.solution_control_iterations()?,
        total_iterations: engine.solution_total_iterations()?,
        most_iterations_done: engine.solution_most_iterations_done()?,
        control_actions_done: engine.solution_control_actions_done()? != 0,
        system_y_changed: engine.solution_system_y_changed()? != 0,
        seconds: engine.solution_seconds()?,
        load_mult: engine.solution_load_mult()?,
    };
    engine.assert_clean("solution scalars")?;
    Ok(cap)
}

/// `capture_all_elements`: every element's terminal currents/powers/losses, read
/// Losses-then-Powers-then-Currents (the §1.1(a)/D3 order —
/// [`Engine::element_pcl`]) with the user-model retry.
///
/// Under `derived` (manifest flag `compare_derived`, GOLDEN_REBASE G1.3a) each
/// element also reports `CktElement.Enabled`, and every **enabled** element the
/// three polar channels [`Engine::element_polar`] reads. Disabled elements are
/// skipped there deliberately: `CktElementV(19)` kills the process on an element
/// whose `NodeRef` was never allocated (spec §1.4-H1, and the doc of
/// `element_polar`), while `enabled` itself is captured for every element so the
/// skip cannot hide one.
///
/// Under `extras` (manifest flag `compare_element_extras`, GOLDEN_REBASE
/// G1.3d) each element also reports `Enabled`, `PhaseLosses` and the nine
/// discrete scalars [`Engine::element_extras`] reads, plus `NodeOrder`
/// (`CktElementV(17)`,
/// `DDLL/DCktElement.pas:1032`) for the elements that are **enabled** and have
/// **at least one terminal**. Both conditions come from the sources, not from
/// caution: mode 17 dereferences `NodeRef^[j]` with no nil guard (`:1048`), so a
/// never-enabled element kills the worker exactly as `CktElementV(19)` does;
/// and on a 0-terminal element (`UPFCControl` never assigns `Nterms` —
/// `Controls/UPFCControl.pas:230-246`) this channel would return a 0-length
/// array while capi raises 15013 from its nil-`NodeRef` guard
/// (`CAPI/CAPI_CktElement.pas:900-906`) — not issuing the read removes that shape
/// asymmetry instead of normalizing it. The comparator asserts both sides are
/// empty there, so neither skip can hide a payload.
///
/// `PhaseLosses` (GOLDEN_REBASE G1.3d(ii), [`Engine::element_phase_losses`]) is
/// the one addition that is NOT order-free: it runs `GetPhaseLosses`' own
/// `ComputeIterminal` (r4133 `Common/CktElement.pas:1090`), so it is group **A**
/// and is issued FIRST — ahead of `element_pcl`'s `Losses`/`Powers` — which is
/// what keeps every group-A read of the element ahead of the group-B `Currents`.
/// It is read for every element, enabled or not: `GetPhaseLosses` zero-fills a
/// disabled one (`:1118-1119`) without touching `NodeRef`, so it needs neither
/// of the two predicates above.
///
/// Every read line carries a machine-checkable `capture-order: NAME (A|B|C)`
/// marker whose group is [`crate::modes::capture_group_of`]'s — a call into
/// another capture helper declares the reads that helper performs, in its order
/// (`crates/dss-core/tests/capture_order.rs` is the gate). A *selector*
/// (`AllElementNames`, `SetActiveElement`) has no mode row: it moves a cursor
/// rather than reading a quantity, so it is order-free by construction and the
/// gate declares it, not the mode table.
fn capture_all_elements(
    engine: &Engine,
    warn: bool,
    derived: bool,
    extras: bool,
) -> Result<Vec<ElementCap>, EngineError> {
    let names = engine.all_element_names(); // capture-order: AllElementNames (C)
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        engine.set_active_element(&name); // capture-order: SetActiveElement (C)
        let enabled = if derived || extras {
            Some(engine.ckt_element_enabled()?) // capture-order: Enabled (C)
        } else {
            None
        };
        let mut pl = Vec::new();
        if extras {
            let ctx = format!("element {name} phase losses");
            pl = engine.element_phase_losses(warn, &ctx)?; // capture-order: PhaseLosses (A)
        }
        // capture-order: Losses (A), Powers (A), Currents (B)
        let (powers, currents, losses) = engine.element_pcl(warn, &format!("element {name}"))?;
        let (i_re, i_im) = deinterleave(&currents);
        let (p_kw, p_kvar) = deinterleave(&powers);
        let loss_w = vec![
            losses.first().copied().unwrap_or(0.0),
            losses.get(1).copied().unwrap_or(0.0),
        ];
        let mut cap = ElementCap {
            name,
            i_re,
            i_im,
            p_kw,
            p_kvar,
            loss_w,
            enabled,
            cma_mag: Vec::new(),
            cma_ang: Vec::new(),
            res_mag: Vec::new(),
            res_ang: Vec::new(),
            vma_mag: Vec::new(),
            vma_ang: Vec::new(),
            n_terms: None,
            n_conds: None,
            n_phases: None,
            energy_meter: None,
            node_order: Vec::new(),
            pl_kw: Vec::new(),
            pl_kvar: Vec::new(),
            num_controls: None,
            ocp_dev_index: None,
            ocp_dev_type: None,
            has_volt_control: None,
            has_switch_control: None,
        };
        if derived && enabled == Some(true) {
            // capture-order: CurrentsMagAng (B), Residuals (B), VoltagesMagAng (C)
            let (cma, res, vma) =
                engine.element_polar(warn, &format!("element {} derived", cap.name))?;
            (cap.cma_mag, cap.cma_ang) = deinterleave(&cma);
            (cap.res_mag, cap.res_ang) = deinterleave(&res);
            (cap.vma_mag, cap.vma_ang) = deinterleave(&vma);
        }
        if extras {
            // capture-order: NumTerminals (C), NumConductors (C), NumPhases (C), EnergyMeter (C)
            // capture-order: NumControls (C), OCPDevIndex (C), OCPDevType (C)
            // capture-order: HasVoltControl (C), HasSwitchControl (C)
            let ex = engine.element_extras(&format!("element {} extras", cap.name))?;
            let n_terms = ex.n_terms;
            cap.n_terms = Some(n_terms);
            cap.n_conds = Some(ex.n_conds);
            cap.n_phases = Some(ex.n_phases);
            cap.energy_meter = Some(ex.energy_meter);
            (cap.pl_kw, cap.pl_kvar) = deinterleave(&pl);
            cap.num_controls = Some(ex.num_controls);
            cap.ocp_dev_index = Some(ex.ocp_dev_index);
            cap.ocp_dev_type = Some(ex.ocp_dev_type);
            cap.has_volt_control = Some(ex.has_volt_control);
            cap.has_switch_control = Some(ex.has_switch_control);
            if enabled == Some(true) && n_terms > 0 {
                cap.node_order = engine.ckt_element_node_order()?; // capture-order: NodeOrder (C)
                engine.assert_clean(&format!("element {} node order", cap.name))?;
            }
        }
        out.push(cap);
    }
    Ok(out)
}

/// Per-step discrete control state: transformer winding taps, RegControl tap
/// numbers, capacitor step states.
type DiscreteState = (
    BTreeMap<String, Vec<f64>>,
    BTreeMap<String, i32>,
    BTreeMap<String, Vec<i32>>,
);

fn capture_discrete(engine: &Engine) -> Result<DiscreteState, EngineError> {
    let mut transformers = BTreeMap::new();
    let mut has = engine.transformers_first();
    while has {
        let nw = engine.transformer_num_windings();
        let mut taps = Vec::with_capacity(nw.max(0) as usize);
        for w in 1..=nw {
            engine.transformer_set_wdg(w);
            taps.push(engine.transformer_tap());
        }
        transformers.insert(engine.transformer_name(), taps);
        has = engine.transformers_next();
    }
    let mut regcontrols = BTreeMap::new();
    let mut has = engine.regcontrols_first();
    while has {
        regcontrols.insert(engine.regcontrol_name(), engine.regcontrol_tap_number());
        has = engine.regcontrols_next();
    }
    let mut capacitors = BTreeMap::new();
    let mut has = engine.capacitors_first();
    while has {
        capacitors.insert(engine.capacitor_name(), engine.capacitor_states());
        has = engine.capacitors_next();
    }
    engine.assert_clean("discrete")?;
    Ok((transformers, regcontrols, capacitors))
}

fn capture_monitors(engine: &Engine) -> Result<Vec<MonitorCap>, EngineError> {
    let mut out = Vec::new();
    let mut has = engine.monitors_first();
    while has {
        let nch = engine.monitor_num_channels();
        let channels: Vec<Vec<f64>> = (1..=nch).map(|c| engine.monitor_channel(c)).collect();
        out.push(MonitorCap {
            name: engine.monitor_name(),
            header: engine.monitor_header(),
            sample_count: engine.monitor_sample_count() as i64,
            channels,
        });
        has = engine.monitors_next();
    }
    engine.assert_clean("monitors")?;
    Ok(out)
}

fn capture_meters(engine: &Engine) -> Result<Vec<MeterCap>, EngineError> {
    let mut out = Vec::new();
    let mut has = engine.meters_first();
    while has {
        let branches = lst(engine.meter_all_branches_in_zone());
        let ends = lst(engine.meter_all_end_elements());
        let pce = lst(engine.meter_zone_pce());
        out.push(MeterCap {
            name: engine.meter_name(),
            register_names: engine.meter_register_names(),
            register_values: engine.meter_register_values(),
            n_branches: branches.len(),
            n_ends: ends.len(),
            n_pce: pce.len(),
            branches,
            ends,
            pce,
        });
        has = engine.meters_next();
    }
    engine.assert_clean("meters")?;
    Ok(out)
}

/// The reliability surface (GOLDEN_REBASE G1.6(i)): every meter's reliability
/// indices, its three ordered zone lists, its allocation state and each of its
/// feeder sections, plus the circuit-wide `Meters.Totals`.
///
/// Called **after** [`capture_meters`] and **before** [`capture_pd_elements`] —
/// the slot `oracle_server.run_case` gives it — and only on the step whose
/// [`Engine::relcalc`] just ran, whose outcome comes in as `rel`.
///
/// # The read order is the contract
/// 1. per meter, the non-section fields in `IMeters._columns` order
///    (`DSS-Python origin/fastdss:dss/IMeters.py:13-42`), with `ZonePCE` — which
///    `_columns` does not list — appended to the two zone lists;
/// 2. still inside the per-meter loop, `Meters.SetActiveSection(k)` for
///    `k in 1..=NumSections` followed by that section's eight fields, again in
///    `_columns` order. `ActiveSection` is a per-meter field the meter walk never
///    resets (`DMeters.pas:254-264`), so a section read without its selector
///    would answer for the previous section;
/// 3. `Meters.Totals` **last, after the walk has finished**: it calls
///    `TotalizeMeters` (`DMeters.pas:566` -> `Common/Circuit.pas:2520-2538`),
///    which itself walks `EnergyMeters.First`/`Next` and so leaves the meter
///    cursor past the end — reading it mid-walk silently truncates the capture
///    (measured on both transports; fastdss says the same at
///    `save_outputs.py:330-332`).
///
/// This is group **C** of the capture-order partition (`GOLDEN_REBASE_PLAN.md`
/// §1.1(a), coordinator decision D3): not one read here goes through
/// `GetCurrents` into a scratch buffer (`Meters.CalcCurrent` returns the
/// *stored* `CalculatedCurrent` array, `DMeters.pas:609-624`), so this surface
/// neither imposes anything on the element capture order nor inherits anything
/// from it.
///
/// Stronger than fastdss on purpose: `save_outputs.py:283-291` captures the
/// **first** section only, this captures every one of them.
pub fn capture_reliability(
    engine: &Engine,
    rel: &RelCalcResult,
) -> Result<ReliabilityCap, EngineError> {
    let mut meters = Vec::new();
    // `RelCalc` itself walked `EnergyMeters.First`/`Next` to the end
    // (`ExecHelper.pas:4417`, `:4439-4441`), so the walk must restart from `First`.
    let mut has = engine.meters_first();
    while has {
        // `IMeters._columns` order, section fields excluded (step 1 above).
        let name = engine.meter_name();
        let alloc_factors = engine.meters_alloc_factors()?;
        let ends = lst(engine.meter_all_end_elements());
        let saifikw = engine.meters_saifi_kw()?;
        let saidi = engine.meters_saidi()?;
        let total_customers = engine.meters_total_customers()?;
        let saifi = engine.meters_saifi()?;
        let cust_interrupts = engine.meters_cust_interrupts()?;
        let calc_current = engine.meters_calc_current()?;
        let branches = lst(engine.meter_all_branches_in_zone());
        let pce = lst(engine.meter_zone_pce());
        let num_sections = engine.meters_num_sections()?;
        // Step 2: select, then read that section's fields — again in
        // `_columns` order (`NumSectionCustomers`, `SectSeqIdx`,
        // `SumBranchFltRates`, `AvgRepairTime`, `SectTotalCust`,
        // `OCPDeviceType`, `FaultRateXRepairHrs`, `NumSectionBranches`).
        let mut sections = Vec::with_capacity(num_sections.max(0) as usize);
        for idx in 1..=num_sections {
            engine.meters_set_active_section(idx)?;
            let num_section_customers = engine.meters_num_section_customers()?;
            let sect_seq_idx = engine.meters_sect_seq_idx()?;
            let sum_branch_flt_rates = engine.meters_sum_branch_flt_rates()?;
            let avg_repair_time = engine.meters_avg_repair_time()?;
            let sect_total_cust = engine.meters_sect_total_cust()?;
            let ocp_device_type = engine.meters_ocp_device_type()?;
            let fault_rate_x_repair_hrs = engine.meters_fault_rate_x_repair_hrs()?;
            let num_section_branches = engine.meters_num_section_branches()?;
            sections.push(FeederSectionCap {
                idx,
                num_section_customers,
                num_section_branches,
                sect_seq_idx,
                sect_total_cust,
                ocp_device_type,
                sum_branch_flt_rates,
                avg_repair_time,
                fault_rate_x_repair_hrs,
            });
        }
        meters.push(MeterReliabilityCap {
            name,
            total_customers,
            saifi,
            saifikw,
            saidi,
            cust_interrupts,
            calc_current,
            alloc_factors,
            branches,
            ends,
            pce,
            num_sections,
            sections,
        });
        has = engine.meters_next();
    }
    // Step 3: LAST, after the walk — `TotalizeMeters` ends it.
    let totals = engine.meters_totals()?;
    engine.assert_clean("reliability")?;
    Ok(ReliabilityCap {
        aborted: rel.aborted,
        message: rel.message.clone(),
        meters,
        totals,
    })
}

/// The `PDElements` walk (GOLDEN_REBASE G1.6b): every **enabled** PD element of
/// the circuit, in `PDElements` pointer-list order (`DPDELements.pas:27-59`
/// skips the disabled ones, exactly like the capi channel's
/// `Generic_CktElement_Get_First/Next`).
///
/// The read order inside a record is load-bearing.
/// `PDElementsI(6)` (`PDElements.ParentPDElement`, `DPDELements.pas:88-97`)
/// does `ActiveCktElement := ActivePDElement.ParentPDElement` and never restores
/// it, so **every** field read after it in the same record returns the
/// *parent's* value: fastdss reads it second (`IPDElements._columns`) and
/// contaminates 215 of the 138-element IEEE123 walk's own cells (measured on
/// both channels, 2026-09-04). This capture therefore reads it **last**, and
/// only then reads the name off the hijacked cursor to obtain `parent_name`.
/// When there is no parent the DDLL leaves `ActiveCktElement` alone
/// (`DPDELements.pas:92` "leaves ActiveCktElement as is"), so the name read is
/// skipped — it would echo the element's own name.
///
/// The iteration itself survives the hijack: `First`/`Next` drive the circuit's
/// `PDElements` pointer list, not `ActiveCktElement`.
///
/// The walk length is **not** `PDElements.Count` (`PDElementsI(0)` returns the
/// raw `ListSize`, disabled elements included): measured on a two-line circuit
/// with one `enabled=no` line, `I:0` = 2 while the walk yields the one enabled
/// element. A circuit with no PD element at all returns an empty `Vec` — 96 of
/// the 372 walked live corpus cases are in that shape, so the gate's guard is
/// presence-based, never `len() > 0`.
pub fn capture_pd_elements(engine: &Engine) -> Result<Vec<PdElementCap>, EngineError> {
    let mut out = Vec::new();
    let mut has = engine.pd_elements_first()?;
    while has {
        // The twelve order-free reads, in the `IPDElements._columns` order.
        let name = engine.pd_elements_name()?;
        let accumulated_l = engine.pd_elements_accumulated_l()?;
        let from_terminal = engine.pd_elements_from_terminal()?;
        let is_shunt = engine.pd_elements_is_shunt()? != 0;
        let num_customers = engine.pd_elements_num_customers()?;
        let section_id = engine.pd_elements_section_id()?;
        let fault_rate = engine.pd_elements_fault_rate()?;
        let repair_time = engine.pd_elements_repair_time()?;
        let total_miles = engine.pd_elements_total_miles()?;
        let total_customers = engine.pd_elements_total_customers()?;
        let pct_permanent = engine.pd_elements_pct_permanent()?;
        let lambda = engine.pd_elements_lambda()?;
        // LAST: this moves `ActiveCktElement` to the parent (see the doc above).
        let parent_class_index = engine.pd_elements_parent_pd_element()?;
        let parent_name = if parent_class_index != 0 {
            engine.pd_elements_name()?
        } else {
            String::new()
        };
        out.push(PdElementCap {
            name,
            accumulated_l,
            from_terminal,
            is_shunt,
            num_customers,
            section_id,
            fault_rate,
            repair_time,
            total_miles,
            total_customers,
            pct_permanent,
            lambda,
            parent_class_index,
            parent_name,
        });
        has = engine.pd_elements_next()?;
    }
    engine.assert_clean("pd_elements")?;
    Ok(out)
}

/// `_lst`: strip whitespace, drop empties; an all-`NONE` placeholder list -> [].
///
/// The empty-zone placeholder is compared case-insensitively: the raw DDLL writes
/// `'None'` (mixed case, `DMeters.pas`) but the Oddie backend normalizes the empty
/// string-array to the AltDSS `'NONE'` DefaultResult, and `oracle_server._lst`
/// filters that. Matching the *filtered* result (both `[]`) is the byte-for-byte
/// contract — an element name is never "none".
fn lst(v: Vec<String>) -> Vec<String> {
    let xs: Vec<String> = v
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if xs.len() == 1 && xs[0].eq_ignore_ascii_case("NONE") {
        Vec::new()
    } else {
        xs
    }
}

fn capture_probes(engine: &Engine, specs: &[ProbeSpec]) -> Result<Vec<ProbeCap>, EngineError> {
    let mut out = Vec::new();
    for spec in specs {
        for p in &spec.props {
            let value = engine.raw_command(&format!("? {}.{}", spec.element, p));
            let (errno, desc) = engine.poll_error();
            if errno != 0 {
                return Err(EngineError::Dss {
                    errno,
                    desc,
                    ctx: format!("probe {}.{}", spec.element, p),
                });
            }
            out.push(ProbeCap {
                element: spec.element.clone(),
                prop: p.clone(),
                value,
            });
        }
    }
    Ok(out)
}

/// Every circuit element's every property value (§2.2 all-properties parity),
/// a byte-for-byte port of `oracle_server.capture_all_properties`: for each
/// `AllElementNames` entry, activate it with `? name.Like` (the WPG.1-safe query
/// path — activates `DSS_OBJECT`s too, unlike `SetActiveElement`), read the
/// class's `AllPropertyNames` off the now-active object (`DSSElementV` mode 0),
/// then read each value via `? name.prop` in property-index order.
///
/// **Gating, since R4133_PROPS RP4.1 (2026-09-03).** This capture was
/// report-tooling only while property parity was pinned to capi_v0145 and the
/// scheduler masked `all_properties` off the r4133 request; RP4.1 removed both
/// masks, so what this function returns is now value-compared against the port
/// for every live non-`large` r4133-gating case. Any non-zero errno on a read
/// escalates, exactly like [`capture_probes`] (matching dss-python's
/// raise-on-error).
fn capture_all_properties(engine: &Engine) -> Result<Vec<PropsCap>, EngineError> {
    let mut out = Vec::new();
    for name in engine.all_element_names() {
        // Activate via the query path (side-effect: sets ActiveDSSObject), then
        // read the property-name list off the active object.
        engine.raw_command(&format!("? {name}.Like"));
        let (errno, desc) = engine.poll_error();
        if errno != 0 {
            return Err(EngineError::Dss {
                errno,
                desc,
                ctx: format!("all_properties activate {name}"),
            });
        }
        let prop_names = engine.element_all_property_names();
        let mut props = Vec::with_capacity(prop_names.len());
        for p in &prop_names {
            let value = engine.raw_command(&format!("? {name}.{p}"));
            let (errno, desc) = engine.poll_error();
            if errno != 0 {
                return Err(EngineError::Dss {
                    errno,
                    desc,
                    ctx: format!("all_properties {name}.{p}"),
                });
            }
            props.push((p.clone(), value));
        }
        out.push(PropsCap {
            element: name,
            props,
        });
    }
    Ok(out)
}

/// The G1.7 topology capture: the six order-free `Topology` rows, read LAST in
/// the step (after [`capture_all_properties`]) on both transports.
///
/// Two independent reasons for "last", the stronger one first:
///  * the FIRST `Topology` read is what BUILDS the tree — every arm goes through
///    `ActiveTree = ActiveCircuit.GetTopology` (`DDLL/DTopology.pas:13-17`),
///    which memoizes it (`Common/Circuit.pas:2932-2950`) and on the way rewrites
///    `Checked` / `IsIsolated` / `BusChecked` on every element (`:2937-2947`).
///    Nothing in today's capture reads those flags, but reading last makes that
///    independent of every future addition — the argument that put
///    `all_properties` last.
///  * `TopologyI(1)`/`(2)` and `TopologyV(1)`/`(2)` walk
///    `ActiveCircuit.PDElements` / `PCElements` `.First`/`.Next` to exhaustion
///    (`DTopology.pas:75-84`, `:85-94`, `:319-352`, `:354-390`; recorded as
///    `modes::TOPOLOGY_*`'s `TOPO_PD_LIST` / `TOPO_PC_LIST` effects), leaving
///    those `TPointerList` cursors at the end, where `Circuit.NextPDElement` /
///    `NextPCElement` would resume.
///
/// These six are also the only `Topology` rows that never assign
/// `ActiveCircuit.ActiveCktElement`: the cursor rows — `ActiveBranch`,
/// `BranchName`, `ActiveLevel`, `First`/`Next`, `ForwardBranch`,
/// `BackwardBranch`, `LoopedBranch`, `ParallelBranch`, `FirstLoad`/`NextLoad`
/// (`DTopology.pas:29-54`, `:96-160`, `:170-186`) and all of `TopologyS` — do,
/// and would poison the per-element capture, so they are never called. That
/// absence is asserted from this file's source text by
/// `crates/dss-core/tests/capture_order.rs` (`GOLDEN_REBASE_PLAN.md` G1.7, the
/// B16 parity gap: three of `ITopology`'s nine fastdss columns are deliberately
/// not captured).
fn capture_topology(engine: &Engine) -> Result<TopologyCap, EngineError> {
    // Same read order as `oracle_server.capture_topology`: the three counts,
    // then the three name lists.
    let num_loops = engine.topology_num_loops()?;
    let num_isolated_branches = engine.topology_num_isolated_branches()?;
    let num_isolated_loads = engine.topology_num_isolated_loads()?;
    let looped_pairs = topo_names(
        engine.topology_all_looped_pairs()?,
        "Topology.AllLoopedPairs",
    )?;
    let isolated_branches = topo_names(
        engine.topology_all_isolated_branches()?,
        "Topology.AllIsolatedBranches",
    )?;
    let isolated_loads = topo_names(
        engine.topology_all_isolated_loads()?,
        "Topology.AllIsolatedLoads",
    )?;
    Ok(TopologyCap {
        num_loops,
        num_isolated_branches,
        num_isolated_loads,
        looped_pairs,
        isolated_branches,
        isolated_loads,
    })
}

/// Normalize one `TopologyV` string reply to the list shape both transports
/// emit — the transport-side sentinel decode, and nothing else.
///
/// r4133 pre-seeds `TStr[0] := 'NONE'` and emits that single token for an empty
/// list (`DTopology.pas:271-275`, `:319-325`, `:354-360`); capi's
/// `DefaultResult(..., 'NONE')` (`CAPI/CAPI_Utils.pas:115`) does the same, so
/// `["NONE"] -> []` is a shared decode of "no entries", never a value. A
/// qualified name is always `Class.name`, so a bare `NONE` can never be a real
/// entry. Measured on the whole population: 1 281 / 1 555 / 1 694 sentinel
/// replies over 455 r4133-gating cases, and the capi channel byte-identical
/// after its own normalization (`GOLDEN_REBASE_PLAN.md` G1.7 §3.1-S1/§3.2).
///
/// Every other shape is REFUSED rather than repaired. In particular an empty
/// entry is a transport failure on this channel, never a value: r4133 filters
/// them at the source — `DTopology.pas:283-297`, `:337-347`, `:372-382` all
/// write only `if TStr[i] <> ''` — and 4 530 list reads over those 455 cases
/// returned exactly zero empty entries. The capi transport is the one that
/// appends a single trailing `''` (`CAPI_Topology.pas:126-132` sets
/// `Length := k + 1` and then copies `Length(Result)` entries, while
/// `AllLoopedPairs` at `:100-115` starts from `k := -1` and does not), and it
/// drops exactly that one in `oracle_server._topo_names`. That arm has no
/// counterpart here on purpose: swallowing an empty would hide precisely the
/// shape change this check exists to catch.
fn topo_names(v: Vec<String>, what: &str) -> Result<Vec<String>, EngineError> {
    if v.len() == 1 && v[0] == "NONE" {
        return Ok(Vec::new());
    }
    if let Some(i) = v.iter().position(String::is_empty) {
        return Err(EngineError::Other(format!(
            "{what}: an empty entry at index {i} of {} \
             — r4133 filters empties at the source \
             (DTopology.pas:337-347), so this is a transport failure, \
             never a value: {v:?}",
            v.len()
        )));
    }
    Ok(v)
}

/// Oracle-free smoke hook (§2.4): dump every element's every property for the
/// currently-compiled circuit, so `smoke.rs` can prove the `DSSElementV`
/// enumeration + `? name.prop` value read round-trips without an oracle.
pub fn all_properties_dump(engine: &Engine) -> Result<Vec<PropsCap>, EngineError> {
    capture_all_properties(engine)
}

/// Every bus's node set, kV base and the three per-node voltage surfaces — the
/// r4133 half of the `compare_bus` capture, a field-for-field port of
/// `oracle_server.capture_all_buses` over the typed mode accessors
/// ([`crate::modes`] rows `Circuit.AllBusNames`, `Bus.Nodes`, `Bus.puVoltages`,
/// `Bus.VMagAngle`, `Bus.puVMagAngle`, `Bus.SeqVoltages`,
/// `Bus.CplxSeqVoltages`, `Bus.VLL`, `Bus.puVLL`; `Bus.kVBase` is `BUSF(0)`).
///
/// Walked in `BusList` order, which `SetActiveBus`'s returned 0-based index
/// (`DCircuit.pas:247-250`, `ActiveBusIndex - 1`) re-asserts per bus: a failed
/// lookup leaves `ActiveBusIndex` at 0 (`Common/DSSGlobals.pas:739-757`) and
/// would otherwise attribute the previous bus's voltages to this one.
///
/// Capture-order class **C, order-free** (GOLDEN_REBASE_PLAN.md §1.1(a),
/// coordinator decision D3): every arm reads `Solution.NodeV` directly and
/// touches neither `ComputeIterminal` nor `ActiveCktElement` — only
/// `ActiveBusIndex` moves. The per-bus read order below matches the capi
/// transport's and is a contract, not a staleness hazard.
///
/// Shapes are asserted, never assumed: `2 * len(nodes)` per value array (a
/// 0-node bus — 2 in the corpus — yields empty arrays and passes at `0 == 0`),
/// and the node numbers must come back strictly ascending, which is what makes
/// this capture comparable to the port's sorted view.
///
/// The four G1.4c arms (`SeqVoltages`, `CplxSeqVoltages`, `VLL`, `puVLL`) are
/// read unconditionally, after the five voltage arms and before the
/// conditional short-circuit block — group C as well (`DBus.pas:305`, `:536`,
/// `:588`, `:644` read `Solution.NodeV` only). `VLL`/`puVLL` go through the
/// state-dependent guard [`Engine::bus_vll_pair`], the crate's ONLY dispatcher
/// of `BUSV(11)`/`BUSV(12)`: on a bus whose node set would spin the pairing
/// loop forever it publishes empty arrays plus [`BusCap::vll_declined`]
/// instead of hanging the worker. Their shapes are derived from the node
/// count, not assumed: 3 and 6 doubles for the sequence arms, and 6 (three
/// pairs) or 2 (one pair, or the `-99999` marker) for the L-L pair.
///
/// `want_sc` (G1.5, request field `zsc`) appends the six short-circuit arms to
/// THIS walk — never a second `SetActiveBus` pass — in the fixed order
/// `zsc1, zsc0, zsc, ysc, isc, voc`, matching `oracle_server.capture_all_buses`
/// arm for arm; the fields are always serialized, empty when it is off. They
/// are group C as well: `BUSV` 3/4/6/7/8/9 read `Zsc`/`Ysc`/`VBus`/
/// `BusCurrent` off the bus object with no `ComputeIterminal` and no
/// `ActiveCktElement` (all six are [`crate::modes::ModeEffect::Pure`]).
///
/// Unlike the three voltage surfaces, every SC array is indexed by the bus's
/// INTERNAL (insertion) node index: `Zsc`/`Ysc` are built column by column over
/// the bus's internal index (`GetRef(i)` in `ComputeYsc`,
/// `Common/SolutionAlgs.pas:800-832`; `pBus.RefNo[i]` in capi `:788-816`) and `VBus`/`BusCurrent` are stored per internal index. Their own
/// shapes are asserted per arm against [`R4133_SC_SENTINEL_LEN`] — a violation
/// fails the case loudly instead of shipping a short row the comparator would
/// misread as a value divergence.
fn capture_all_buses(engine: &Engine, want_sc: bool) -> Result<Vec<BusCap>, EngineError> {
    let names = engine.circuit_all_bus_names()?;
    let mut out = Vec::with_capacity(names.len());
    for (i, name) in names.iter().enumerate() {
        let idx = engine.set_active_bus(name);
        if idx != i as i32 {
            return Err(EngineError::Other(format!(
                "bus capture: SetActiveBus({name:?}) returned {idx}, expected {i} \
                 (AllBusNames must be the engine's BusList order)"
            )));
        }
        let nodes = engine.bus_nodes()?;
        let kv_base = engine.bus_kvbase();
        let pu_voltages = engine.bus_pu_voltages()?;
        let vmag_angle = engine.bus_vmag_angle()?;
        let pu_vmag_angle = engine.bus_pu_vmag_angle()?;
        if nodes.windows(2).any(|w| w[1] <= w[0]) {
            return Err(EngineError::Other(format!(
                "bus capture: {name}.Nodes {nodes:?} is not strictly ascending \
                 (DBus.pas:319-345 walks FindIdx(jj) upward)"
            )));
        }
        for (key, v) in [
            ("pu_voltages", &pu_voltages),
            ("vmag_angle", &vmag_angle),
            ("pu_vmag_angle", &pu_vmag_angle),
        ] {
            if v.len() != 2 * nodes.len() {
                return Err(EngineError::Other(format!(
                    "bus capture: {name}.{key} returned {} values, expected 2*{} for nodes {nodes:?}",
                    v.len(),
                    nodes.len()
                )));
            }
        }
        // G1.4c, still group C (order-free): both sequence arms and the two
        // L-L arms read `Solution.NodeV` plus the bus object only
        // (`DBus.pas:305`, `:536`, `:588`, `:644`), with no `ComputeIterminal`
        // and no `ActiveCktElement`. The order below is the CONTRACT between
        // the two transports, asserted by the capture-order test.
        let seq_voltages = engine.bus_seq_voltages()?;
        let cplx_seq_voltages = engine.bus_cplx_seq_voltages()?;
        // The ONLY dispatcher of `BUSV(11)`/`BUSV(12)` in this crate's capture
        // path: it re-reads `Bus.Nodes` itself and refuses the pair whole when
        // the pairing loop would not terminate.
        let (vll, pu_vll, vll_declined) = match engine.bus_vll_pair()? {
            Some((vll, pu_vll)) => (vll, pu_vll, false),
            None => (Vec::new(), Vec::new(), true),
        };
        for (key, v, want) in [
            ("seq_voltages", &seq_voltages, 3usize),
            ("cplx_seq_voltages", &cplx_seq_voltages, 6),
        ] {
            if v.len() != want {
                return Err(EngineError::Other(format!(
                    "bus capture: {name}.{key} returned {} values, expected {want} \
                     (DBus.pas:315-316 / :546-547 publish a fixed length)",
                    v.len()
                )));
            }
        }
        if vll_declined {
            // The `Nvalues <= 1` branch never enters a loop (`DBus.pas:594`,
            // `:650`), so a refusal there would be the guard misfiring.
            if nodes.len() < 2 {
                return Err(EngineError::Other(format!(
                    "bus capture: {name} has nodes {nodes:?} but the VLL guard \
                     refused — DBus.pas:563/:594 cannot loop below 2 nodes"
                )));
            }
        } else {
            // `Nvalues > 3 => 3` then `= 2 => 1` (`:561-563`): three pairs on a
            // bus with `>= 3` nodes, one complex on every other — either one
            // L-L pair or the `-99999` marker.
            let want = if nodes.len() >= 3 { 6 } else { 2 };
            for (key, v) in [("vll", &vll), ("pu_vll", &pu_vll)] {
                if v.len() != want {
                    return Err(EngineError::Other(format!(
                        "bus capture: {name}.{key} returned {} values, expected {want} \
                         for nodes {nodes:?} (DBus.pas:561-563, :594)",
                        v.len()
                    )));
                }
            }
        }
        let (zsc1, zsc0, zsc, ysc, isc, voc) = if want_sc {
            (
                engine.bus_zsc1()?,
                engine.bus_zsc0()?,
                engine.bus_zsc_matrix()?,
                engine.bus_ysc_matrix()?,
                engine.bus_isc()?,
                engine.bus_voc()?,
            )
        } else {
            Default::default()
        };
        if want_sc {
            let n = nodes.len();
            // `Isc`/`Voc` publish `2*n` doubles from the allocated
            // `BusCurrent`/`VBus`, EXCEPT on a 0-node bus, where
            // `Reallocmem(ptr, 0)` frees the pointer and the arm falls back to
            // the sentinel (see `R4133_SC_SENTINEL_LEN`).
            let per_node = if n == 0 { R4133_SC_SENTINEL_LEN } else { 2 * n };
            let matrix = [R4133_SC_SENTINEL_LEN, 2 * n * n];
            for (key, v, want) in [
                ("zsc1", &zsc1, &[2usize][..]),
                ("zsc0", &zsc0, &[2][..]),
                ("zsc", &zsc, &matrix[..]),
                ("ysc", &ysc, &matrix[..]),
                ("isc", &isc, &[per_node][..]),
                ("voc", &voc, &[per_node][..]),
            ] {
                if !want.contains(&v.len()) {
                    return Err(EngineError::Other(format!(
                        "bus capture: {name}.{key} returned {} values, expected one of \
                         {want:?} for nodes {nodes:?} (see BusCap for each arm's Pascal shape)",
                        v.len()
                    )));
                }
            }
        }
        out.push(BusCap {
            name: name.clone(),
            kv_base,
            nodes,
            pu_voltages,
            vmag_angle,
            pu_vmag_angle,
            zsc1,
            zsc0,
            zsc,
            ysc,
            isc,
            voc,
            seq_voltages,
            cplx_seq_voltages,
            vll,
            pu_vll,
            vll_declined,
        });
    }
    engine.assert_clean("buses")?;
    Ok(out)
}

/// `Circuit.AllBusMagPu` — every NODE's per-unit voltage magnitude
/// (`CircuitV(9)`, `DCircuit.pas:481-500` == `CAPI_Circuit.pas:521-548`).
///
/// Ordered bus-list order x the bus's INTERNAL node index (`GetRef(j)` for
/// `j = 1..NumNodesThisBus`, i.e. the `AllNodeNames` permutation) — neither the
/// ascending-node-number order of [`capture_all_buses`] nor the gated
/// `YNodeOrder`. Its length is `NumNodes`, which is also the sum of the per-bus
/// node counts: the caller checks that identity, so the two walks cannot drift
/// apart silently.
fn capture_all_bus_vmag_pu(engine: &Engine) -> Result<Vec<f64>, EngineError> {
    let v = engine.circuit_all_bus_mag_pu()?;
    engine.assert_clean("all_bus_vmag_pu")?;
    Ok(v)
}

fn capture_variables(engine: &Engine, names: &[String]) -> Result<Vec<VariablesCap>, EngineError> {
    let mut out = Vec::new();
    for name in names {
        engine.set_active_element(name);
        out.push(VariablesCap {
            name: name.clone(),
            var_names: engine.element_variable_names(),
            values: engine.element_variable_values(),
        });
    }
    if !names.is_empty() {
        engine.assert_clean("variables")?;
    }
    Ok(out)
}

fn capture_ctrlqueue(engine: &Engine) -> Vec<String> {
    engine
        .ctrl_queue()
        .into_iter()
        .filter(|r| {
            let t = r.trim();
            !t.is_empty() && t != "No events" && !r.starts_with("Handle,")
        })
        .collect()
}

/// `capture_eventlog` (Oddie path): `export eventlog` writes a UTF-8-BOM CSV;
/// read it back stripping the BOM per line and dropping blank lines.
fn capture_eventlog(engine: &Engine) -> Result<Vec<String>, EngineError> {
    let reply = engine.raw_command("export eventlog");
    let (errno, desc) = engine.poll_error();
    if errno != 0 {
        return Err(EngineError::Dss {
            errno,
            desc,
            ctx: "export eventlog".to_string(),
        });
    }
    let path = reply.trim().trim_start_matches('\u{FEFF}').to_string();
    let Ok(bytes) = std::fs::read(&path) else {
        return Ok(Vec::new());
    };
    // utf-8-sig: strip a leading file BOM, then per-line BOM + CR.
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    let text = String::from_utf8_lossy(body);
    let mut out = Vec::new();
    for raw in text.split('\n') {
        let line = raw
            .trim_start_matches('\u{FEFF}')
            .trim_end_matches(['\r', '\n']);
        if !line.trim().is_empty() {
            out.push(line.to_string());
        }
    }
    Ok(out)
}

/// Read the `<CircuitName>_AutoAddLog.csv` the AutoAdd solve wrote (inside the
/// guard scope, before it removes the file). Newlines are normalized to `\n`:
/// `oracle_server` reads the file in Python text mode (universal newlines, so
/// `\r\n`/`\r` → `\n`), and the capture must match that byte-for-byte.
fn read_autoadd_log(engine: &Engine, case_path: &str) -> Option<String> {
    // Circuit name via CircuitS mode 0 (Name).
    let name = engine.circuit_name();
    let dir = Path::new(case_path).parent()?;
    let log = dir.join(format!("{name}_AutoAddLog.csv"));
    let raw = std::fs::read_to_string(log).ok()?;
    Some(raw.replace("\r\n", "\n").replace('\r', "\n"))
}

#[cfg(test)]
mod tests {
    use super::{complex_pair, topo_names};

    /// A `myType = 3` reply is exactly two doubles or it is a transport
    /// failure — the bridge must never pad one into a plausible `(0, 0)`
    /// (`Circuit.SubstationLosses` is a legitimate `(0, 0)` on most decks, so a
    /// padded short read would be indistinguishable from the true value).
    /// `DDLL/DCircuit.pas:293-303` sets length 1 unconditionally.
    #[test]
    fn complex_pair_refuses_a_reply_that_is_not_two_doubles() {
        assert_eq!(complex_pair(&[1.5, -2.5], "x").unwrap(), vec![1.5, -2.5]);
        for short in [&[][..], &[1.0][..], &[1.0, 2.0, 3.0][..]] {
            let err = complex_pair(short, "Circuit.SubstationLosses")
                .expect_err("a reply of the wrong length must be an error, not a padded pair");
            let msg = err.to_string();
            assert!(
                msg.contains("Circuit.SubstationLosses") && msg.contains("expected exactly 2"),
                "unhelpful message: {msg}"
            );
        }
    }

    /// The `TopologyV` sentinel decode, and the shape checks around it (G1.7).
    ///
    /// `["NONE"]` is r4133's "no entries" reply (`DTopology.pas:271-275` seeds
    /// `TStr[0] := 'NONE'`), so it decodes to an empty list; a qualified name is
    /// always `Class.name`, so nothing real is swallowed. An EMPTY entry is
    /// refused rather than dropped: r4133 filters empties at the source
    /// (`DTopology.pas:337-347`) and 4 530 list reads over the 455 r4133-gating
    /// corpus cases returned none, so one arriving is a transport failure. (The
    /// capi transport is the one with a single trailing `''`,
    /// `CAPI_Topology.pas:126-132`; it drops it in `oracle_server._topo_names`.)
    #[test]
    fn topo_names_decodes_the_none_sentinel_and_refuses_an_empty_entry() {
        let n = |v: &[&str]| topo_names(v.iter().map(|s| s.to_string()).collect(), "x");
        assert_eq!(n(&["NONE"]).unwrap(), Vec::<String>::new());
        assert_eq!(n(&[]).unwrap(), Vec::<String>::new());
        // A real one-entry list, and a name that merely contains NONE, survive.
        assert_eq!(n(&["Line.l1"]).unwrap(), vec!["Line.l1".to_string()]);
        assert_eq!(n(&["Load.none"]).unwrap(), vec!["Load.none".to_string()]);
        assert_eq!(
            n(&["NONE", "Line.l1"]).unwrap(),
            vec!["NONE".to_string(), "Line.l1".to_string()]
        );
        for bad in [&["Line.l1", ""][..], &["", "Line.l1"][..], &["", ""][..]] {
            let err = topo_names(
                bad.iter().map(|s| s.to_string()).collect(),
                "Topology.AllIsolatedBranches",
            )
            .expect_err("an empty entry must be an error, not a silent drop");
            let msg = err.to_string();
            assert!(
                msg.contains("Topology.AllIsolatedBranches") && msg.contains("empty entry"),
                "unhelpful message: {msg}"
            );
        }
    }
}
