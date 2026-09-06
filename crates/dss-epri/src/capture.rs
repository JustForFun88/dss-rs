//! `CaseResult` assembly — a byte-for-byte port of
//! `tools/oracle/oracle_server.py::run_case` (+ the `capture_*` helpers it shares
//! with `tools/golden/gen_checkpoints.py`) against the raw r4133 DLL.
//!
//! The response is JSON-shape-identical to the retired Oddie oracle's
//! (`CaseResult { node_order, n_steps, checkpoints, autoadd_log }`, + G1.10a's
//! `run_files` and `sweep_failed`), so the Rust
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
    /// ones, and (GOLDEN_REBASE G1.3b) the three sequence channels
    /// `SeqPowers` / `SeqCurrents` / `SeqVoltages` for the enabled ones too,
    /// and (GOLDEN_REBASE G1.3c) `TotalPowers` / `CplxSeqCurrents` /
    /// `CplxSeqVoltages`, also enabled-only.
    /// Absent or `false` ⇒ none of the seventeen keys is emitted and the reply
    /// is byte-identical to a pre-G1.3a one.
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
    /// `GOLDEN_REBASE_PLAN.md` G1.8 — the flat `CalcIncMatrix` / `CalcLaplacian`
    /// pair and the four `SolutionV` rows it makes readable. See
    /// [`capture_inc_matrix`].
    #[serde(default)]
    pub inc_matrix: bool,
    #[serde(default)]
    pub global_result: bool,
    #[serde(default)]
    pub autoadd_log: bool,
    /// `GOLDEN_REBASE_PLAN.md` G1.10a — the run-produced FILE SET under the
    /// case's DataPath. Not a model read: the run's filesystem effect, taken
    /// from the SAME [`CorpusGuard`] pass that sweeps the corpus clean, and read
    /// STRICTLY LAST of the whole run (after `autoadd_log`) while the guard is
    /// still alive. Twin request key: `oracle_server.py`'s `run_files`.
    #[serde(default)]
    pub run_files: bool,
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
    /// G1.10a — the sorted, normalized set of filesystem entries this run
    /// created under the case dir (a trailing `/` marks a created directory;
    /// see [`crate::guard::normalize_created_name`]). `None` when the request
    /// did not ask for it, and also when the guard cannot report honestly (an
    /// incomplete pre-run snapshot); the gate's presence rail
    /// (`harness::capture_guard::require_capture_opt`) turns the second case
    /// into a failed case rather than "this deck created nothing".
    run_files: Option<Vec<String>>,
    /// G1.10a / coordinator decision D32(2) — the created entries this run's own
    /// hygiene guard could NOT remove, normalized like [`Self::run_files`].
    /// Always present (`[]` is the normal answer), never gated on a request
    /// flag: a producer that leaves an undeletable dropping behind silences the
    /// created set of every LATER producer of the same case, which is how the
    /// capi Storage trace-file leak hid from the gate (see
    /// [`crate::guard::CorpusGuard::sweep_created`]). The gate's runner fails
    /// the case on a non-empty list
    /// (`crates/dss-core/tests/corpus_gate/runner.rs::compare_with_result`).
    sweep_failed: Vec<String>,
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
    /// G1.4b: `Circuit.AllBusDistances`, one `DistFromMeter` per bus in
    /// `BusList` order; empty when the run did not request the bus surface.
    all_bus_distances: Vec<f64>,
    /// G1.4b: `Circuit.AllNodeDistances`, the owning bus's `DistFromMeter` per
    /// node in the `AllNodeNames` permutation; same emptiness rule.
    all_node_distances: Vec<f64>,
    all_properties: Vec<PropsCap>,
    global_result: String,
    aggregates: AggregatesCap,
    solution_scalars: SolutionScalarsCap,
    topology: Option<TopologyCap>,
    inc_matrix: Option<IncMatrixCap>,
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
    // The GOLDEN_REBASE G1.3b sequence channels, emitted under the SAME
    // `RunRequest::derived` flag as the polar three above and skipped when
    // empty, so an off-flag reply keeps the byte-for-byte shape it had before
    // G1.3b (`oracle_server.capture_all_elements` emits exactly the same keys).
    // All four are enabled-only, like the polar block: `CktElementV(9)` guards
    // neither `Enabled` nor `NodeRef` (see [`crate::dss::Engine::element_seq`]).
    /// `SeqCurrents` — 012 current magnitudes (A), `3 * NTerms` of them; and
    /// `SeqVoltages` — 012 voltage magnitudes (V), same length. Magnitudes on
    /// both engines (`Cabs`), so neither is de-interleaved.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    seq_i: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    seq_v: Vec<f64>,
    /// `SeqPowers`, de-interleaved into kW and kvar the way `p_kw`/`p_kvar`
    /// already are. `3 * NTerms` each; the `0.003` 3-phase kVA scaling is the
    /// engines' own, applied inside the arm, so the wire unit IS kW/kvar.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    seq_p_kw: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    seq_p_kvar: Vec<f64>,
    // The GOLDEN_REBASE G1.3c additions, emitted under the SAME
    // `RunRequest::derived` flag as the two blocks above and skipped when
    // empty, so an off-flag reply keeps the byte-for-byte shape it had before
    // G1.3c (`oracle_server.capture_all_elements` emits exactly the same keys).
    /// `TotalPowers`, de-interleaved into kW and kvar the way `p_kw`/`p_kvar`
    /// already are. `NTerms` each — the per-terminal sum of `GetPhasePower`'s
    /// conductor block, scaled by `0.001` **once**, inside the engine's own arm
    /// (`DDLL/DCktElement.pas:1132`, capi `CAPI/CAPI_Alt.pas:1138-1139`), so
    /// the wire unit IS kW/kvar. Enabled-only, like the two blocks above, and
    /// here purely to keep the channels' shapes equal: mode 20 has no
    /// `NodeRef` guard where capi has one (see
    /// [`crate::dss::Engine::element_total_powers`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tp_kw: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tp_kvar: Vec<f64>,
    /// `CplxSeqCurrents` and `CplxSeqVoltages`, de-interleaved the same way —
    /// the un-`Cabs`'d 012 components whose moduli `seq_i`/`seq_v` are,
    /// `3 * NTerms` each, amps and volts
    /// (see [`crate::dss::Engine::element_cplx_seq`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cseq_i_re: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cseq_i_im: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cseq_v_re: Vec<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cseq_v_im: Vec<f64>,
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
    /// The per-bus half of the same post-`RelCalc` surface (GOLDEN_REBASE
    /// G1.6(ii)), in `BusList` order — see [`capture_bus_reliability`]. It is
    /// nested HERE and not at the checkpoint's top level, exactly as
    /// `oracle_server.capture_reliability` nests its own `"buses"` key; the
    /// `Checkpoint`'s `buses` is G1.4a's voltage capture, a different
    /// surface.
    pub buses: Vec<BusReliabilityCap>,
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

/// One bus's eight reliability columns (GOLDEN_REBASE G1.6(ii)): the six
/// `Export BusReliability` renders plus the two it does not — `Cust_Duration`
/// and `SectionID` — captured once per case, right after the executive
/// `RelCalc` the same payload reports on.
///
/// Key names and types are the contract on the wire (the worker serializes
/// through `serde_json::Value`, which sorts object keys, so JSON order carries
/// no meaning); they match `oracle_server.capture_bus_reliability` field for
/// field, which is what the gate's `harness::BusReliabilityCap` deserializes on
/// both channels. The port side is `dss-core`'s
/// `exec/view.rs::BusReliabilityView` over the `TDSSBus` mirror
/// `circuit/bus.rs:48-63`.
///
/// `Lambda` travels as `lambda_`, because `lambda` is a keyword in both Python
/// and Rust; the rename is identical on both transports. Do not "fix" it.
///
/// Field order below is also the READ order — see [`capture_bus_reliability`],
/// which reads in fastdss' `IBus._columns` order.
#[derive(Serialize)]
pub struct BusReliabilityCap {
    /// `Circuit.AllBusNames` entry i, re-asserted as the active bus by
    /// `SetActiveBus`'s returned index (`DCircuit.pas:439`, `:247-250`) — the
    /// same walk key `BusCap` carries.
    pub name: String,
    /// `BUSF(10)` `Bus.Cust_Duration` — `BusCustDurations` (`DBus.pas:157-163`).
    pub cust_duration: f64,
    /// `BUSF(9)` `Bus.Cust_Interrupts` — `BusCustInterrupts` (`DBus.pas:150-156`).
    pub cust_interrupts: f64,
    /// `BUSF(8)` `Bus.Int_Duration` — `Bus_Int_Duration` (`DBus.pas:143-149`).
    ///
    /// `Source_IntDuration + AverageRepairTime` (`Meters/EnergyMeter.pas:2567-2575`),
    /// so it inherits the section's unguarded `SumFltRatesXRepairHrs /
    /// SumBranchFltRates` (`:2561-2563`, [`FeederSectionCap::avg_repair_time`]):
    /// a `NaN` here is a `NaN` on every engine alike. It reaches the wire as
    /// JSON `null` and fails the harness' decode loudly rather than becoming a
    /// plausible `0`.
    pub int_duration: f64,
    /// `BUSF(6)` `Bus.Lambda` — `BusFltRate` (`DBus.pas:129-135`).
    pub lambda_: f64,
    /// `BUSI(4)` `Bus.N_Customers` — `BusTotalNumCustomers`, a `longint`
    /// (`DBus.pas:60-66`).
    pub n_customers: i32,
    /// `BUSF(7)` `Bus.N_interrupts` — `Bus_Num_Interrupt` (`DBus.pas:136-142`).
    pub n_interrupts: f64,
    /// `BUSI(5)` `Bus.SectionID` — `BusSectionID`, a `longint`
    /// (`DBus.pas:67-73`).
    ///
    /// A `-1` here is DATA — the mid-sweep `ZeroReliabilityAccums` writes it to
    /// "signify not set" (`PDElements/PDElement.pas:326`) before the forward
    /// sweep re-stamps the head bus (`EnergyMeter.pas:2494`) — and NOT the
    /// family's unknown-mode sentinel (`DBus.pas:75`). Which is why the walk
    /// proves its selection by the returned index and never by the shape of a
    /// value.
    pub section_id: i32,
    /// `BUSF(11)` `Bus.TotalMiles` — `BusTotalMiles` (`DBus.pas:164-170`).
    pub total_miles: f64,
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
    /// `Bus.Distance` — `TDSSBus.DistFromMeter` in km, published verbatim
    /// (`BUSF(5)`, `DBus.pas:122-128` == capi `CAPI_Bus.pas:419-427` ->
    /// `CAPI_Alt.pas:2071-2074`). G1.4b.
    ///
    /// A zone-build output, not a solve output: `MakeMeterZoneLists` writes it
    /// (`Meters/EnergyMeter.pas:1833-1838`), so a circuit with no EnergyMeter —
    /// or a bus outside every meter's zone — reports the untouched `0.0`.
    /// Neither channel has a "no meter" sentinel.
    distance: f64,
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
    /// `Bus.AllPCEatBus` — the qualified names of the power-conversion elements
    /// whose TERMINAL 1 names this bus (`BUSV(18)`, `DBus.pas:840-866` ->
    /// `Common/Circuit.pas:1540-1583`, class set `:1559` == capi
    /// `CAPI_Bus.pas:773-788` -> `Common/Circuit.pas:1797-1870`). G1.4d.
    ///
    /// The wire shape is THIS channel's: `getPCEatBus` seeds `Result[0] :=
    /// 'None'` (`:1551`) and appends one empty trailing slot (`:1569-1570`),
    /// and the DDLL arm filters that slot (`DBus.pas:853`) while re-emitting
    /// `'None'` for an empty byte array (`:862-863`). So an empty answer is
    /// exactly `["None"]` and no other entry may be empty — both rails are
    /// asserted in [`capture_all_buses`], never normalized: the capi channel's
    /// convention is a different one (the pinned dss-python facade appends one
    /// `''` to a non-empty reply, `dss/IBus.py`), and the comparator asserts
    /// each channel's own.
    all_pce_at_bus: Vec<String>,
    /// `Bus.AllPDEatBus` — the qualified names of the power-delivery elements
    /// whose terminal 1 or 2 names this bus, with `bus1 <> bus2` filtering the
    /// shunts out (`BUSV(19)`, `DBus.pas:867-898` ->
    /// `Common/Circuit.pas:1493-1536`, the criterion `:1520-1522` == capi
    /// `CAPI_Bus.pas:790-805` -> `Common/Circuit.pas:1712-1794`). Same wire
    /// shape and the same two rails as [`Self::all_pce_at_bus`] (`:1505`,
    /// `:1524-1525`, `DBus.pas:880`, `:894-895`).
    all_pde_at_bus: Vec<String>,
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

/// The four flat incidence quantities of `GOLDEN_REBASE_PLAN.md` G1.8, in the
/// exact shape `oracle_server.capture_inc_matrix` emits (identical JSON keys,
/// identical normalized shapes). See [`capture_inc_matrix`].
///
/// `inc_matrix` and `laplacian` are FLAT `(row, col, value)` integer triples in
/// the sparse container's insertion order, `3 * NZero` long: `SolutionV(1)`
/// (`DDLL/DSolution.pas:542-568`) and `SolutionV(5)` (`:640-667`) copy
/// `IncMat.data[k][0..2]` / `Laplacian.data[k][0..2]` cell by cell. The
/// nil/empty sentinel is decoded away by [`inc_ints`], so an empty matrix is an
/// empty list on both channels.
///
/// `rows` are `Class.name` labels, one per incidence-matrix row
/// (`Inc_Mat_Rows`, `DSolution.pas:589-608`); `cols` are bus names — after the
/// flat build `IncMat_Ordered` is FALSE (`Common/Solution.pas:3066`), so the
/// getter answers the WHOLE `BusList` rather than `Inc_Mat_Cols`
/// (`DSolution.pas:616-632`), measured on 1 756 / 1 756 steps.
#[derive(Serialize)]
struct IncMatrixCap {
    inc_matrix: Vec<i32>,
    laplacian: Vec<i32>,
    rows: Vec<String>,
    cols: Vec<String>,
}

// ---------------------------------------------------------------------------
// The run.
// ---------------------------------------------------------------------------

/// Compile one deck, solve `n_steps` times, capture the full per-step model.
pub fn run_case(engine: &Engine, req: &RunRequest) -> Result<CaseResult, EngineError> {
    let warn = req.warn_and_continue;
    let star = req.selected_elements == ["*"];

    // G1.10a: bound by name (it used to be `_guard`) because the run's created
    // FILE SET is reported from this very guard, after the last read and before
    // it sweeps — one classification, two consumers (`CorpusGuard::created` and
    // the sweep), so the reported set cannot disagree with the swept one. `mut`
    // since D32(2): the sweep is driven explicitly by `finish()` below so its
    // failures can travel in this reply instead of dying in `Drop`.
    let mut guard = CorpusGuard::new(&req.case_path);

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
                capture_eventlog(engine)
            } else {
                Vec::new()
            };
            let ctrlqueue = if req.ctrlqueue {
                capture_ctrlqueue(engine)
            } else {
                Vec::new()
            };
            let (buses, all_bus_vmag_pu, all_bus_distances, all_node_distances) = if req.buses {
                let buses = capture_all_buses(engine, req.zsc)?;
                let all_bus_vmag_pu = capture_all_bus_vmag_pu(engine)?;
                // G1.4b: the two circuit-level views of the same
                // `DistFromMeter` the per-bus walk above read one bus at a time.
                let all_bus_distances = capture_all_bus_distances(engine)?;
                let all_node_distances = capture_all_node_distances(engine)?;
                let nodes: usize = buses.iter().map(|b| b.nodes.len()).sum();
                if all_bus_distances.len() != buses.len() {
                    return Err(EngineError::Other(format!(
                        "bus capture: AllBusDistances has {} values but the per-bus walk saw {} \
                         buses (DCircuit.pas:574 sizes it NumBuses)",
                        all_bus_distances.len(),
                        buses.len()
                    )));
                }
                if all_node_distances.len() != nodes {
                    return Err(EngineError::Other(format!(
                        "bus capture: AllNodeDistances has {} values but the per-bus walk saw \
                         {nodes} nodes over {} buses",
                        all_node_distances.len(),
                        buses.len()
                    )));
                }
                if all_bus_vmag_pu.len() != nodes {
                    return Err(EngineError::Other(format!(
                        "bus capture: AllBusVmagPu has {} values but the per-bus walk saw {nodes} \
                         nodes over {} buses",
                        all_bus_vmag_pu.len(),
                        buses.len()
                    )));
                }
                (
                    buses,
                    all_bus_vmag_pu,
                    all_bus_distances,
                    all_node_distances,
                )
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
                (Vec::new(), Vec::new(), Vec::new(), Vec::new())
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
            // G1.8 — read STRICTLY LAST, after the G1.7 topology block, on both
            // transports (`oracle_server.run_case` does the same; the source
            // order of both is asserted by
            // `crates/dss-core/tests/capture_order.rs`). Why last, why the pair
            // is issued in this order, and why `CalcIncMatrix_O` / `BusLevels`
            // are never touched: see [`capture_inc_matrix`].
            let inc_matrix = if req.inc_matrix {
                let cap = capture_inc_matrix(engine)?;
                engine.assert_clean("inc_matrix")?;
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
                all_bus_distances,
                all_node_distances,
                all_properties,
                global_result,
                aggregates,
                solution_scalars,
                topology,
                inc_matrix,
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

    // G1.10a: STRICTLY LAST of the whole run — after every step's model read and
    // after the `autoadd_log` file read — and while `guard` is still alive, since
    // its `Drop` removes exactly what this call classifies. `None` means "cannot
    // be reported honestly" (incomplete pre-run snapshot / failed listing); the
    // gate's presence rail turns that into a failed case.
    let run_files = if req.run_files { guard.created() } else { None };

    // D32(2): sweep NOW, not in `Drop`, so a removal the guard could not perform
    // is reported to the gate instead of being swallowed. Unconditional — the
    // hygiene contract does not depend on the run-file request flag. (An early
    // `?` return above still sweeps through `Drop`, which prints the leak to
    // stderr; that case has already failed on the error itself.)
    let sweep_failed = guard.finish();

    Ok(CaseResult {
        node_order,
        n_steps: req.n_steps,
        checkpoints,
        autoadd_log,
        run_files,
        sweep_failed,
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
/// The same flag carries the three sequence channels [`Engine::element_seq`]
/// reads (GOLDEN_REBASE G1.3b), under the same enabled-only rule — which there
/// is load-bearing rather than shape-normalizing: `CktElementV(9)`
/// (`SeqPowers`) guards neither `Enabled` nor `NodeRef` and dereferences
/// `NodeRef^[k+1]` at `DDLL/DCktElement.pas:765`, and capi's own read is no
/// safer (`CAPI/CAPI_Alt.pas:604` skips the `Enabled` test, `:608` resizes the
/// result before the helper's guard at `:544` exits; `:607` is the scratch
/// `cBuffer`). `SeqPowers` is
/// de-interleaved into kW/kvar the way `p_kw`/`p_kvar` are; the two magnitude
/// channels are flat.
///
/// The same flag again carries the GOLDEN_REBASE G1.3c trio, on the same
/// enabled-only rule: `TotalPowers` ([`Engine::element_total_powers`]) —
/// cache-aware, hence read from the head of the element beside `PhaseLosses`
/// and never from the derived block — plus `CplxSeqCurrents` and
/// `CplxSeqVoltages` ([`Engine::element_cplx_seq`]), the un-`Cabs`'d values
/// whose moduli `seq_i`/`seq_v` already are. On all three the enabled-only rule
/// is shape-normalizing rather than a crash guard (modes 13/14 seed a 1-element
/// `CZero` default before their `If Enabled`; mode 20 takes `GetPhasePower`'s
/// `Else … CZERO` branch), which is exactly what makes it the *right* rule:
/// capi's extra `NodeRef = NIL` guards would otherwise answer a different
/// length on the very same element.
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
        let mut tp = Vec::new();
        if derived && enabled == Some(true) {
            let ctx = format!("element {name} total powers");
            tp = engine.element_total_powers(warn, &ctx)?; // capture-order: TotalPowers (A)
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
            seq_i: Vec::new(),
            seq_v: Vec::new(),
            seq_p_kw: Vec::new(),
            seq_p_kvar: Vec::new(),
            tp_kw: Vec::new(),
            tp_kvar: Vec::new(),
            cseq_i_re: Vec::new(),
            cseq_i_im: Vec::new(),
            cseq_v_re: Vec::new(),
            cseq_v_im: Vec::new(),
        };
        if derived && enabled == Some(true) {
            // capture-order: CurrentsMagAng (B), Residuals (B), VoltagesMagAng (C)
            let (cma, res, vma) =
                engine.element_polar(warn, &format!("element {} derived", cap.name))?;
            (cap.cma_mag, cap.cma_ang) = deinterleave(&cma);
            (cap.res_mag, cap.res_ang) = deinterleave(&res);
            (cap.vma_mag, cap.vma_ang) = deinterleave(&vma);
            // capture-order: SeqPowers (B), SeqCurrents (B), SeqVoltages (C)
            let (seq_p, seq_i, seq_v) =
                engine.element_seq(warn, &format!("element {} sequence", cap.name))?;
            (cap.seq_p_kw, cap.seq_p_kvar) = deinterleave(&seq_p);
            cap.seq_i = seq_i;
            cap.seq_v = seq_v;
            // capture-order: CplxSeqCurrents (B), CplxSeqVoltages (C)
            let (cseq_i, cseq_v) =
                engine.element_cplx_seq(warn, &format!("element {} complex sequence", cap.name))?;
            (cap.cseq_i_re, cap.cseq_i_im) = deinterleave(&cseq_i);
            (cap.cseq_v_re, cap.cseq_v_im) = deinterleave(&cseq_v);
            (cap.tp_kw, cap.tp_kvar) = deinterleave(&tp);
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
/// 4. finally, once the meter walk is over, the per-bus half of the same
///    surface — [`capture_bus_reliability`], which touches no `Meters` handle
///    and so leaves rule 3 literally true (the capi transport nests its
///    `"buses"` key at the same point, for the same reason).
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
    // Step 4 (G1.6(ii)): the per-bus half of the same post-`RelCalc` surface,
    // through its own function so no `Meters` read can follow `Totals`, and
    // after `totals` so the payload reads meters-then-buses exactly as
    // `oracle_server.capture_reliability` builds it. Its reads are order-free
    // (group C) and move only `ActiveBusIndex`; its own `assert_clean` keeps
    // the two error scopes apart.
    let buses = capture_bus_reliability(engine)?;
    Ok(ReliabilityCap {
        aborted: rel.aborted,
        message: rel.message.clone(),
        meters,
        totals,
        buses,
    })
}

/// Every bus's eight reliability columns, read AFTER the executive `RelCalc`
/// (`GOLDEN_REBASE_PLAN.md` §1.1, sub-step G1.6(ii)) — the r4133 half of the
/// per-bus reliability capture, a field-for-field port of
/// `oracle_server.capture_bus_reliability` over the typed mode accessors
/// ([`crate::modes`] rows `Bus.Lambda`, `Bus.N_interrupts`, `Bus.Int_Duration`,
/// `Bus.Cust_Interrupts`, `Bus.Cust_Duration`, `Bus.TotalMiles`,
/// `Bus.N_Customers`, `Bus.SectionID`).
///
/// The parity target is `origin/fastdss` `dss/IBus.py:19-53` `_columns`, which
/// the fastdss harness archives for every bus through the iterable
/// `dss.ActiveCircuit.ActiveBus` (`tests/save_outputs.py:351`); these eight are
/// its reliability half and are read in that `_columns` order —
/// `Cust_Duration`, `Cust_Interrupts`, `Int_Duration`, `Lambda`,
/// `N_Customers`, `N_interrupts`, `SectionID`, `TotalMiles` — with the list's
/// DUPLICATE `Cust_Interrupts` entry (`dss/IBus.py:27`) collapsed to a single
/// read, exactly as the capi transport does: reading a pure field twice proves
/// nothing and would make the read-order pin ambiguous.
///
/// Walked in `BusList` order, which `SetActiveBus`'s returned 0-based index
/// (`DCircuit.pas:247-250`, `ActiveBusIndex - 1`) re-asserts per bus, the way
/// `capture_all_buses` does: a failed lookup leaves `ActiveBusIndex` at 0
/// (`Common/DSSGlobals.pas:739-757`) and would silently attribute the previous
/// bus's reliability row to this one. That assertion — never the shape of a
/// value — is what proves a row belongs to its bus: all eight arms initialize
/// their `Result` to `0`/`0.0` BEFORE the `ActiveBusIndex > 0` guard
/// (`DBus.pas:60-73`, `:129-170`), so an unselected bus answers a perfectly
/// plausible zero row, and the family's `-1` else arm (`DBus.pas:75`) collides
/// with a legitimate `SectionID` of `-1`.
///
/// Capture-order class **C, order-free** (`GOLDEN_REBASE_PLAN.md` §1.1(a),
/// coordinator decision D3): all eight are plain `Buses^[ActiveBusIndex]` field
/// reads — none goes through `ComputeIterminal`/`GetCurrents`, none writes
/// engine state, and the only cursor that moves is `ActiveBusIndex`, which
/// `capture_all_buses`, its one later reader, re-selects per bus anyway.
///
/// A SEPARATE function from [`capture_reliability`] on purpose: the read-order
/// pins scan that body for its `Meters`-handle reads and require
/// `Meters.Totals` to be the last of them
/// (`crates/dss-core/tests/reliability_pins.rs`), so the bus block gets its own
/// scanned body and its own order test instead of perturbing that rule.
///
/// The name comes from the walk (`Circuit.AllBusNames`) rather than from a
/// per-bus read: this bridge binds no `BUSS` family, and `capture_all_buses`
/// — which owns the bus plumbing — takes it the same way.
pub fn capture_bus_reliability(engine: &Engine) -> Result<Vec<BusReliabilityCap>, EngineError> {
    let names = engine.circuit_all_bus_names()?;
    let mut out = Vec::with_capacity(names.len());
    for (i, name) in names.iter().enumerate() {
        let idx = engine.set_active_bus(name);
        if idx != i as i32 {
            return Err(EngineError::Other(format!(
                "bus reliability capture: SetActiveBus({name:?}) returned {idx}, \
                 expected {i} (AllBusNames must be the engine's BusList order)"
            )));
        }
        // `IBus._columns` order, the duplicate collapsed.
        let cust_duration = engine.bus_cust_duration()?;
        let cust_interrupts = engine.bus_cust_interrupts()?;
        let int_duration = engine.bus_int_duration()?;
        let lambda_ = engine.bus_lambda()?;
        let n_customers = engine.bus_n_customers()?;
        let n_interrupts = engine.bus_n_interrupts()?;
        let section_id = engine.bus_section_id()?;
        let total_miles = engine.bus_total_miles()?;
        out.push(BusReliabilityCap {
            name: name.clone(),
            cust_duration,
            cust_interrupts,
            int_duration,
            lambda_,
            n_customers,
            n_interrupts,
            section_id,
            total_miles,
        });
    }
    engine.assert_clean("bus_reliability")?;
    Ok(out)
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

/// The G1.8 incidence capture: build the FLAT incidence matrix and its
/// Laplacian, then read the four `SolutionV` rows that expose them — issued and
/// read STRICTLY LAST in the step, after [`capture_topology`], on both
/// transports.
///
/// **The pair, in this order.** `CalcIncMatrix` is `ExecCommand[109]`
/// (`Executive/ExecCommands.pas:163`, `:888-890` -> `Calc_Inc_Matrix`,
/// `Common/Solution.pas:3046-3068`) and `CalcLaplacian` is `ExecCommand[117]`
/// (`:171`, `:911-917`). The order is not cosmetic: the Laplacian arm is a bare
/// `Laplacian := IncMat.Transpose()` then `Laplacian.multiply(IncMat)` with
/// **no** `Assigned(IncMat)` guard, so issuing it first on a circuit whose
/// `IncMat` was never built dereferences nil inside the DLL and kills the
/// worker. (dss_capi guards the same command with error 8877,
/// `Executive/ExecCommands.pas:421-433`, and so does the port's
/// `exec::command::do_calc_laplacian`; r4133 does not, which is exactly why the
/// ordering lives here and not in a comment.) `Calc_Inc_Matrix` creates or
/// resets `IncMat` unconditionally (`Common/Solution.pas:3051-3054`), so after
/// the first command the getter's `IncMat <> Nil` test always holds — measured:
/// 1 756 steps over 464 r4133-gating cases, zero DLL errors, 104 of them with an
/// empty matrix.
///
/// **Why last**, two independent reasons:
///  * `AddSeriesReac2IncMatrix` is not a pure read of the model — it sets
///    `LastClassReferenced` / `ActiveDSSClass` and then calls
///    `ActiveDSSClass.First`, which reassigns `ActiveCircuit.ActiveCktElement`
///    (`Common/Solution.pas:3007-3010`). Building the matrix therefore moves the
///    active-element cursor and must not precede any per-element or per-property
///    read of the step.
///  * it must follow the G1.7 topology read, whose census constants
///    (`TOPOLOGY_STALE_DECLINES`, `LOOPED_PAIR_WINDOW_DECLINES`) are defined on a
///    `Branch_List` nothing else has touched.
///
/// **What is deliberately never issued or read here.** `CalcIncMatrix_O`
/// (`ExecCommand[110]`, `Calc_Inc_Matrix_Org`) calls `GetTopology`
/// (`Common/Solution.pas:3173`), which would build and memoize that same
/// `Branch_List` and move G1.7's census; and `SolutionV(2)` `Solution.BusLevels`
/// is on [`crate::modes::DO_NOT_CALL`] — `DSolution.pas:578-582` does
/// `setlength(myIntArray, ArrSize)` then `for IMIdx := 0 to ArrSize`, a
/// one-element heap overflow inside the DLL. Both are out of the live gate by
/// decision (`GOLDEN_REBASE_PLAN.md` §G1.8 / §G3.2c); the `_O` builder, the bus
/// levels and the CSV writer keep their byte goldens instead.
///
/// The caller escalates any lingering read error ([`Engine::assert_clean`]).
fn capture_inc_matrix(engine: &Engine) -> Result<IncMatrixCap, EngineError> {
    engine.exec_wait("CalcIncMatrix")?;
    engine.exec_wait("CalcLaplacian")?;
    // Same read order as `oracle_server.capture_inc_matrix`: the two integer
    // matrices, then the two name lists.
    let inc_matrix = inc_ints(engine.solution_inc_matrix()?, "Solution.IncMatrix")?;
    let laplacian = inc_ints(engine.solution_laplacian()?, "Solution.Laplacian")?;
    let rows = inc_names(
        engine.solution_inc_matrix_rows()?,
        "Solution.IncMatrixRows",
        inc_matrix.is_empty(),
    )?;
    let cols = inc_names(
        engine.solution_inc_matrix_cols()?,
        "Solution.IncMatrixCols",
        false,
    )?;
    Ok(IncMatrixCap {
        inc_matrix,
        laplacian,
        rows,
        cols,
    })
}

/// Normalize one `SolutionV` integer reply to the flat triple list both
/// transports emit (G1.8 transport rule N2) — the sentinel decode and the shape
/// contract, nothing else.
///
/// Both arms pre-seed a one-cell `[0]` array and enlarge it only when the matrix
/// exists AND has non-zeros: `setlength(myIntArray, 1); myIntArray[0] := 0;`
/// (`DSolution.pas:544-545`, `:642-643`) followed by
/// `ArrSize := <matrix>.NZero * 3; if ArrSize > 0 then setlength(...)`
/// (`:550-552`, `:648-650`). So `[0]` means "no non-zeros" and never a value: a
/// real reply is `3 * NZero` long with `NZero >= 1`, i.e. at least three cells.
/// Measured over the whole r4133 population (`GOLDEN_REBASE_PLAN.md` G1.8 §3.1):
/// 1 756 steps, 1 652 with `len % 3 == 0` and exactly 104 equal to `[0]` — no
/// third shape occurred, on either quantity.
///
/// The capi channel carries ONE cell more, because `CAPI_Solution.pas:874`/`:906`
/// allocate `ArrSize + 1` (`//TODO: remove the +1`); that channel drops its
/// trailing cell — with an assert that it is 0 — in
/// `oracle_server.capture_inc_matrix`. The rule is spelled per channel rather
/// than shared precisely because the two shapes differ; after both
/// normalizations the channels are byte-identical (358 `both` cases, 0
/// disagreements).
///
/// Every other shape is REFUSED rather than repaired: a length that is neither
/// `3 * NZero` nor the sentinel means the surface's shape contract broke, which
/// is G1.8's kill criterion and not something a transport may quietly round off.
fn inc_ints(v: Vec<i32>, what: &str) -> Result<Vec<i32>, EngineError> {
    if v.len() == 1 {
        if v[0] != 0 {
            return Err(EngineError::Other(format!(
                "{what}: a one-cell reply [{}] — the r4133 empty/nil sentinel is \
                 exactly [0] (DSolution.pas:544-545) and a real reply is 3*NZero \
                 cells, so this is a shape failure, never a value",
                v[0]
            )));
        }
        return Ok(Vec::new());
    }
    if v.is_empty() || !v.len().is_multiple_of(3) {
        return Err(EngineError::Other(format!(
            "{what}: {} cells — the arm writes 3*NZero cells, or the single-cell \
             [0] sentinel when the matrix is empty (DSolution.pas:544-552); this \
             length is neither",
            v.len()
        )));
    }
    Ok(v)
}

/// Normalize one incidence `SolutionV` string reply to the list shape both
/// transports emit (G1.8 transport rule N3) — the sentinel decode, and nothing
/// else.
///
/// Both name arms build their buffer and then, only if it stayed empty, write
/// the single token `'None'` (`DSolution.pas:604-605` for the rows, `:635-636`
/// for the columns); the capi channel's counterpart is `DefaultResult(..., '')`
/// (`CAPI_Solution.pas:959`, `:988`, `:996`, `:1010`), normalized on its own
/// side. `sentinel_expected` says whether *this* read is one where upstream's
/// emptiness test can fire, and the sentinel decodes to an empty list only
/// there:
///
///  * **rows** — `sentinel_expected` is "the incidence matrix came back empty".
///    The two are equivalent by construction (every emitted row appends both a
///    name to `Inc_Mat_Rows` and cells to `IncMat`) and measured equivalent on
///    all 1 756 steps: `['None']` in exactly the 104 empty-matrix steps and in no
///    other. A row label is always `Class.name`, so a bare `None` can never be a
///    real entry either way.
///  * **cols** — always `false`. After the flat build `IncMat_Ordered` is FALSE
///    (`Common/Solution.pas:3066`), so the arm writes one token per bus
///    (`DSolution.pas:627-631`) and can only stay empty at `NumBuses = 0`, which
///    a compiled circuit never reaches: measured `cols.len() == NumBuses >= 1`
///    and `cols == AllBusNames` on 1 756 / 1 756 steps, sentinel 0 / 1 756. A bus
///    may legally be *named* `None`, so a one-entry `['None']` here is ambiguous
///    — and this transport refuses an ambiguous reply instead of guessing.
///
/// An EMPTY entry is likewise refused, not dropped. Unlike `TopologyV`, these
/// arms carry no `if TStr[i] <> ''` filter (`DSolution.pas:597-601`, `:627-631`),
/// but a row label always carries its `Class.` prefix and `BusList.Get` never
/// returns a blank, so an empty token means the array was padded or the buffer
/// decode slipped — a transport failure, and swallowing it would hide exactly
/// the shape change this check exists to catch.
fn inc_names(
    v: Vec<String>,
    what: &str,
    sentinel_expected: bool,
) -> Result<Vec<String>, EngineError> {
    if v.len() == 1 && v[0] == "None" {
        if !sentinel_expected {
            return Err(EngineError::Other(format!(
                "{what}: the empty-list sentinel ['None'] on a read whose upstream \
                 emptiness test cannot fire (DSolution.pas:604-605 / :635-636) — \
                 refused rather than decoded, because a bus may legally be named \
                 `None`"
            )));
        }
        return Ok(Vec::new());
    }
    if v.is_empty() {
        return Err(EngineError::Other(format!(
            "{what}: an empty reply — the arm always writes at least the single \
             'None' token (DSolution.pas:604-605), so this is a transport failure"
        )));
    }
    if let Some(i) = v.iter().position(String::is_empty) {
        return Err(EngineError::Other(format!(
            "{what}: an empty entry at index {i} of {} — a row label always \
             carries its `Class.` prefix and `BusList.Get` never returns a blank \
             (DSolution.pas:597-601, :627-631), so this is a transport failure, \
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
/// coordinator decision D3): every value arm reads `Solution.NodeV` directly
/// and touches neither `ComputeIterminal` nor `ActiveCktElement` — only
/// `ActiveBusIndex` moves. The two G1.4d at-bus arms are group C too, but for
/// the weaker reason `ModeEffect::Impure` carries: they move a class cursor
/// and the active element, never a current cache, which is why they are read
/// LAST (see below). The per-bus read order below matches the capi
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
///
/// The two G1.4d arms ([`BusCap::all_pce_at_bus`], [`BusCap::all_pde_at_bus`])
/// close the walk, unconditionally, in the slot `oracle_server.capture_all_buses`
/// gives them. They are read LAST because they are the only
/// [`crate::modes::ModeEffect::Impure`] reads of the block: `getP*atBus` drives
/// `DSS_Class.First`/`Next` (`Common/Circuit.pas:1517-1527`, `:1563-1572`) and
/// `TDSSClass.Get_First`/`Get_Next` (`Common/DSSClass.pas:342-371`) assign
/// `ActiveCircuit.ActiveCktElement` and each walked class's own `ActiveElement`
/// cursor. Nothing in this function reads an element afterwards, the bus cursor
/// is untouched (`DBus.pas:849`, `:876` only pass `BusList.Get(ActiveBusIndex)`
/// down — asserted live by `crates/dss-epri/tests/modes.rs`), and the next
/// element-scoped capture re-activates per element by name
/// ([`capture_all_properties`]), so the clobber reaches nothing. Their two wire
/// rails — no empty entry, `"None"` only alone — are asserted here rather than
/// normalized: the capi channel's convention differs by construction and each
/// channel's own is what the comparator checks.
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
        // G1.4b, group C: `BUSF(5)` returns the stored `DistFromMeter` and
        // touches nothing (`DBus.pas:122-128`). Read here, with the bus's other
        // scalar attribute and ahead of the value arrays, so both transports
        // share ONE per-bus read order (`oracle_server.py::capture_all_buses`).
        let distance = engine.bus_distance()?;
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
        // G1.4d: the two at-bus lists close the per-bus walk on BOTH transports
        // (`oracle_server.py::capture_all_buses` reads them in the same slot,
        // in the same order). They are read LAST because on THIS channel they
        // are the only `ModeEffect::Impure` reads of the block
        // (`crate::modes::BUS_ALL_PCE_AT_BUS`): `getP*atBus` drives
        // `DSS_Class.First`/`Next` and `TDSSClass.Get_First`/`Get_Next`
        // (`Common/DSSClass.pas:342-371`) assign `ActiveCircuit`'s active
        // element and each walked class's own cursor. Neither read moves
        // `ActiveBusIndex` (`DBus.pas:849`, `:876` only pass
        // `BusList.Get(ActiveBusIndex)` down), so the next bus's reads are
        // unaffected, and neither goes through `ComputeIterminal` — the block
        // stays capture-group C.
        let all_pce_at_bus = engine.bus_all_pce_at_bus()?;
        let all_pde_at_bus = engine.bus_all_pde_at_bus()?;
        for (key, v) in [
            ("all_pce_at_bus", &all_pce_at_bus),
            ("all_pde_at_bus", &all_pde_at_bus),
        ] {
            // `getP*atBus` appends one empty trailing slot
            // (`Common/Circuit.pas:1524-1525`, `:1569-1570`) and the DDLL arm
            // filters it (`DBus.pas:853`, `:880`). An empty entry reaching here
            // means that filter stopped working, which the comparator would
            // read as a divergence of the LIST rather than of the transport.
            if let Some(i) = v.iter().position(|s| s.is_empty()) {
                return Err(EngineError::Other(format!(
                    "bus capture: {name}.{key} entry {i} of {v:?} is empty — \
                     DBus.pas:853/:880 filter getP*atBus' trailing slot, so no \
                     entry can be"
                )));
            }
            // `'None'` is the arm's own empty-answer word (`Circuit.pas:1505`,
            // `:1551`; re-emitted at `DBus.pas:862-863`, `:894-895`), never a
            // member of a real list — an element named `None` would print
            // qualified (`Class.None`), so a bare `None` beside real names is
            // this transport misreading the buffer.
            if v.iter().any(|s| s == "None") && v.len() != 1 {
                return Err(EngineError::Other(format!(
                    "bus capture: {name}.{key} mixes the empty-answer word \
                     \"None\" with real entries: {v:?} (Circuit.pas:1505/:1551 \
                     seed it alone)"
                )));
            }
        }
        out.push(BusCap {
            name: name.clone(),
            kv_base,
            distance,
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
            all_pce_at_bus,
            all_pde_at_bus,
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

/// `Circuit.AllBusDistances` — each bus's `DistFromMeter` (km) in `BusList`
/// order (`CircuitV(12)`, `DCircuit.pas:566-580` == `CAPI_Circuit.pas:671-688`,
/// whose comment reads *"in an array that aligns with the buslist"*).
/// GOLDEN_REBASE_PLAN.md WP-G1 G1.4b.
///
/// Length = `NumBuses`, checked by the caller against the per-bus walk, so this
/// array and [`capture_all_buses`]' per-bus `distance` cannot drift apart
/// silently.
fn capture_all_bus_distances(engine: &Engine) -> Result<Vec<f64>, EngineError> {
    let v = engine.circuit_all_bus_distances()?;
    engine.assert_clean("all_bus_distances")?;
    Ok(v)
}

/// `Circuit.AllNodeDistances` — the owning bus's `DistFromMeter` repeated once
/// per node, walked bus x the bus's INTERNAL node index (`CircuitV(13)`,
/// `DCircuit.pas:582-604` == `CAPI_Circuit.pas:697-722`: *"Array sequence is
/// same as all bus Vmag and Vmagpu"*), i.e. the [`capture_all_bus_vmag_pu`]
/// permutation and NOT the ascending-node-number order of the per-bus arrays.
/// Length = `NumNodes`, checked by the caller.
fn capture_all_node_distances(engine: &Engine) -> Result<Vec<f64>, EngineError> {
    let v = engine.circuit_all_node_distances()?;
    engine.assert_clean("all_node_distances")?;
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

/// `capture_eventlog`: the run's cumulative event log, read **in memory** from
/// `Solution.EventLog` ([`Engine::eventlog`] = `SolutionV(0)`, r4133
/// `Version8/Source/DDLL/DSolution.pas:518,526-541`, which serializes
/// `EventStrings[ActiveActor]`), with blank lines dropped.
///
/// This is the same list the other two producers read: the capi transport takes
/// `ckt.Solution.EventLog` (`tools/oracle/oracle_server.py::capture_eventlog`
/// -> dss_capi 0.14.5 `src/CAPI/CAPI_Solution.pas:525-540`, the same
/// `EventStrings` walk) and the port reads its own log in memory. It replaces
/// the retired Oddie path, which issued `export eventlog` and read the CSV back
/// (r4133 `Common/ExportResults.pas:3527-3532` =
/// `EventStrings[ActiveActor].SaveToFile`, named `<CircuitName>_EXP_EventLog.CSV`
/// at `Executive/ExportOptions.pas:365`): that command made the r4133 channel
/// write a file into the case directory that neither the capi channel nor the
/// port creates, which G1.10a's created-file-set surface sees as a divergence on
/// every event-logging case (coordinator decision **D30**, class A: 59 red
/// (case, channel) pairs). The two reads were measured byte-identical over a
/// full 526-case corpus drive (0 mismatches, 83 cases with `evlog=1`), and the
/// equivalence is pinned by
/// `tests/protocol.rs::the_in_memory_event_log_equals_the_exported_file`.
///
/// Upstream's own harness never compared this surface at all — fastdss
/// `tests/compare_outputs.py:289-292` skips `EventLog` as "too textual" — so the
/// gate's comparison is new coverage, not catch-up.
fn capture_eventlog(engine: &Engine) -> Vec<String> {
    engine
        .eventlog()
        .into_iter()
        .filter(|l| !l.trim().is_empty())
        .collect()
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
    use super::{complex_pair, inc_ints, inc_names, topo_names};

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

    /// The G1.8 `SolutionV` integer shape rule (N2): `[0]` is r4133's
    /// empty/nil sentinel (`DSolution.pas:544-545`, `:642-643`), every real
    /// reply is `3 * NZero` cells, and nothing else is a reply at all.
    #[test]
    fn inc_ints_decodes_the_empty_sentinel_and_refuses_every_other_shape() {
        assert_eq!(
            inc_ints(vec![0], "Solution.IncMatrix").unwrap(),
            Vec::<i32>::new()
        );
        let triples = vec![0, 0, 1, 0, 1, -1, 1, 2, 1];
        assert_eq!(
            inc_ints(triples.clone(), "Solution.IncMatrix").unwrap(),
            triples
        );
        // A one-cell reply that is not the sentinel is a shape failure, never a
        // value: the arm can only produce `[0]` or `3*NZero` cells.
        let err = inc_ints(vec![7], "Solution.IncMatrix")
            .expect_err("a non-zero one-cell reply must be an error");
        assert!(
            err.to_string().contains("Solution.IncMatrix")
                && err.to_string().contains("sentinel is"),
            "unhelpful message: {err}"
        );
        // The capi `+1` shape must NOT be silently accepted here: it is dropped
        // on its own channel (`oracle_server.capture_inc_matrix`), and a
        // `len % 3 == 1` reply on THIS channel means the DDLL arm changed.
        for bad in [
            &[0, 0, 1, 0][..],
            &[][..],
            &[0, 0][..],
            &[1, 2, 3, 4, 5][..],
        ] {
            let err = inc_ints(bad.to_vec(), "Solution.Laplacian")
                .expect_err("a length that is neither 3*NZero nor the sentinel must be an error");
            let msg = err.to_string();
            assert!(
                msg.contains("Solution.Laplacian") && msg.contains("cells"),
                "unhelpful message: {msg}"
            );
        }
    }

    /// The G1.8 `SolutionV` name shape rule (N3): `['None']` is the empty-list
    /// sentinel only where upstream's emptiness test can fire
    /// (`DSolution.pas:604-605`, `:635-636`); everywhere else — including the
    /// column list, where a bus may legally be *named* `None` — it is refused,
    /// and an empty entry is always refused.
    #[test]
    fn inc_names_decodes_the_sentinel_only_where_upstream_can_emit_it() {
        assert_eq!(
            inc_names(vec!["None".into()], "Solution.IncMatrixRows", true).unwrap(),
            Vec::<String>::new()
        );
        let rows = vec!["Line.l1".to_string(), "Reactor.r1".to_string()];
        assert_eq!(
            inc_names(rows.clone(), "Solution.IncMatrixRows", true).unwrap(),
            rows
        );
        // A bus genuinely named `None` inside a longer list is untouched — only
        // the one-entry reply is the sentinel shape.
        let cols = vec!["sourcebus".to_string(), "None".to_string()];
        assert_eq!(
            inc_names(cols.clone(), "Solution.IncMatrixCols", false).unwrap(),
            cols
        );
        let err = inc_names(vec!["None".into()], "Solution.IncMatrixCols", false)
            .expect_err("the ambiguous one-entry `None` must be refused, not decoded");
        let msg = err.to_string();
        assert!(
            msg.contains("Solution.IncMatrixCols") && msg.contains("cannot fire"),
            "unhelpful message: {msg}"
        );
        let err = inc_names(Vec::new(), "Solution.IncMatrixRows", true)
            .expect_err("an empty reply must be a transport failure");
        assert!(err.to_string().contains("empty reply"), "{err}");
        for bad in [&["Line.l1", ""][..], &["", "Line.l1"][..]] {
            let err = inc_names(
                bad.iter().map(|s| s.to_string()).collect(),
                "Solution.IncMatrixRows",
                true,
            )
            .expect_err("an empty entry must be an error, not a silent drop");
            let msg = err.to_string();
            assert!(
                msg.contains("Solution.IncMatrixRows") && msg.contains("empty entry"),
                "unhelpful message: {msg}"
            );
        }
    }
}
