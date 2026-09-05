//! Public query/snapshot API over [`Dss`] for the golden/test harness
//! (monitor buffers, meter zones, element snapshots, system Y, ...).
//! Split out of `exec/mod.rs`.

use super::*;
use crate::report::export::json::{
    JsonOpts, build as json_build, circuit as json_circuit, serialize as json_serialize,
};
use crate::support::mathutil::SymComp;

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
/// **The four quantities the two oracles do *not* share (GOLDEN_REBASE G1.4c).**
/// Both engines publish `SeqVoltages`, `CplxSeqVoltages`, `VLL` and `puVLL`, but
/// on any bus that is not exactly `1, 2, 3` they answer by three different rules,
/// two of which are defective. The port answers for itself; the corpus gate
/// replays each upstream walk over *this* view's own state as an assertion about
/// the oracle, never as the port's answer.
///
/// * **S-SEQ** — [`Self::seq_voltages`] / [`Self::cplx_seq_voltages`] exist
///   **iff the bus carries all three phase nodes 1, 2 and 3**, and are `None`
///   otherwise. Symmetrical components are undefined without three phase
///   voltages, which is exactly what r4133's own comment says (*"Signify seq
///   voltages n/A for less then 3 phases"*, `DBus.pas:299` ==
///   `CAPI_Alt.pas:2183`) — while both engines test the node *count* instead:
///   capi clamps `Nvalues > 3` to 3 and answers on a 4-node bus
///   (`CAPI_Alt.pas:2174-2186`), r4133 does not clamp and returns `-1` there
///   (`DBus.pas:296-300`), and **both** substitute ground for a phase the bus
///   does not carry (`Find(i) = 0 ⇒ NodeV[0]`, `DBus.pas:305` ==
///   `CAPI_Alt.pas:2190`), fabricating a 0 V phase on e.g. a `[1, 2, 10]` bus.
/// * **S-VLL** — [`Self::vll`] / [`Self::pu_vll`] are the line-to-line voltages
///   **over the phase nodes actually present**: `[V1−V2, V2−V3, V3−V1]` with all
///   three, the single pair with exactly two, `None` with fewer. Both engines
///   instead walk `jj` forward and poll `FindIdx(jj)` *before* the `jj > 3 ⇒
///   jj := 1` wrap (`DBus.pas:575-584` == `CAPI_Alt.pas:2500-2523`), which pairs
///   phase 3 with node 4 on a `[1, 2, 3, 4]` bus, pairs a node with itself on
///   `[1, 10]`, and emits `V2−V1` on `[1, 2, 10]`. r4133's own commented-out
///   original (`DBus.pas:586-587`) and its own report path
///   (`Common/ShowResults.pas:193-194`) both wrap *first*, so upstream
///   contradicts itself inside one engine; capi's bounded `for k := 1 to 3`
///   (`CAPI_Alt.pas:2508-2523`, comment *"(2020-03-01) Changed in DSS C-API to
///   avoid some corner cases that resulted in infinite loops"*) is upstream's own
///   acknowledgement that r4133's unbounded `repeat` hangs.
///
/// The engine's *report* paths keep the upstream conventions byte-for-byte —
/// `report/export/seq_voltages.rs:38-48` ground-substitutes, and
/// `report/show/voltages.rs:190-192` wraps before probing — because they are byte
/// goldens. Only this API surface carries the port's own semantics, and the two
/// are pinned separately.
///
/// The fastdss harness drops `VLL`/`puVLL` from `IBus._columns` altogether in
/// this configuration (`tests/save_outputs.py:205-207` on `origin/fastdss`,
/// `COM_VLL_BROKEN`), so publishing them at all is *stronger* than fastdss
/// parity; the sequence pair it keeps.
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
    /// The bus's raw `Solution.NodeV` entries, ascending node number — the same
    /// walk as [`Self::pu_voltages`], undivided.
    ///
    /// It is the ingredient a consumer needs to replay an upstream pairing or
    /// ground substitution over the port's own state without re-deriving the
    /// bus's node refs (the GOLDEN_REBASE D16 precedent: the oracle's walk is
    /// asserted, not reproduced).
    pub node_v: Vec<num_complex::Complex64>,
    /// `Bus.SeqVoltages`: `|V012|` — `Cabs` of the symmetrical components of the
    /// phase voltages — or `None` when the bus does not carry all three phase
    /// nodes (**S-SEQ**, see the type doc).
    pub seq_voltages: Option<[f64; 3]>,
    /// `Bus.CplxSeqVoltages`: the same `V012`, complex. `None` under exactly the
    /// S-SEQ rule of [`Self::seq_voltages`] — both come out of one transform, so
    /// they can never disagree about availability or about a value.
    pub cplx_seq_voltages: Option<[num_complex::Complex64; 3]>,
    /// `Bus.VLL`: the line-to-line voltages over the phase nodes the bus carries
    /// — 3 entries with nodes 1, 2 and 3 all present, 1 entry with exactly two of
    /// them, `None` with fewer (**S-VLL**, see the type doc).
    pub vll: Option<Vec<num_complex::Complex64>>,
    /// `Bus.puVLL`: [`Self::vll`] over the line-to-line base
    /// (`1000 · kVBase · √3`, or `1.0` — `bus_ll_base_factor`). `Some` exactly
    /// when [`Self::vll`] is.
    pub pu_vll: Option<Vec<num_complex::Complex64>>,
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

/// The **line-to-line** `BaseFactor` both engines divide `puVLL` by:
/// `1000 · kVBase · √3`, or `1.0` when the bus has no base
/// (`DBus.pas:622-623` == `CAPI_Alt.pas:2427-2430`). Distinct from
/// [`bus_base_factor`], which carries no `√3`.
///
/// `sqrt3` is `Sqrt(3.0)` on both sides (r4133 `Common/DSSGlobals.pas:2033` ==
/// capi `Common/DSSGlobals.pas:733` == [`crate::util::sqrt3`]), so there is no
/// precision-compat site here. The `1.0` arm is live on this surface too: the
/// corpus's `Test/indmachtest/Master.DSS` reaches `Solve` with `kVBase = 0` on
/// `sourcebus`, where both oracles report `puVLL == VLL` in volts.
fn bus_ll_base_factor(bus: &crate::circuit::bus::Bus) -> f64 {
    if bus.kv_base > 0.0 {
        1000.0 * bus.kv_base * crate::util::sqrt3()
    } else {
        1.0
    }
}

/// The `Solution.NodeV` refs of the bus's phase nodes 1, 2 and 3 — `None` in a
/// slot whose node number the bus does not carry.
///
/// Presence is decided by `FindIdx` (`Bus::find_idx`), never by `Find`: `Find`
/// answers `0` for an absent node, which is also `NodeV`'s ground slot, and it is
/// that conflation both engines feed straight into the sequence transform
/// (`DBus.pas:305` == `CAPI_Alt.pas:2190`) to fabricate a 0 V phase.
fn phase_refs(bus: &crate::circuit::bus::Bus) -> [Option<usize>; 3] {
    [1i32, 2, 3].map(|num| bus.find_idx(num).map(|idx| bus.get_ref(idx)))
}

/// **S-SEQ**: `V012` over the bus's phase nodes 1, 2 and 3, or `None` when the
/// bus does not carry all three (see [`BusVoltageView`] for why, and for how the
/// two oracles differ from this and from each other).
///
/// The transform is the engine default [`SymComp::precise`] — the pair
/// `Phase2SymComp` runs on the pinned capi oracle (`Shared/mathutil.pas:548`
/// ends initialization with `SelectAs2pVersion(False)`) and the pair
/// `report/export/seq_voltages.rs:44` uses, so the two in-tree sequence surfaces
/// cannot drift apart numerically. r4133 builds its `Ap2s` from the truncated
/// `sin 60° = 0.866025403` and inverts it numerically, which is a measured
/// ~5e-10 relative offset on `V1`/`V2`, not a semantic difference
/// ([`SymComp::official`], pinned by
/// `mathutil::tests::sym_comp_official_vs_precise_gap_is_the_truncated_sin60_constant`).
fn bus_seq_voltages(
    ckt: &Circuit,
    bus: &crate::circuit::bus::Bus,
    sc: &SymComp,
) -> Option<[num_complex::Complex64; 3]> {
    let [r1, r2, r3] = phase_refs(bus);
    let vph = [
        node_voltage(ckt, r1?),
        node_voltage(ckt, r2?),
        node_voltage(ckt, r3?),
    ];
    let mut v012 = [num_complex::Complex64::ZERO; 3];
    sc.phase_to_sym(&vph, &mut v012);
    Some(v012)
}

/// **S-VLL**: the line-to-line voltages over the phase nodes the bus actually
/// carries — `[V1−V2, V2−V3, V3−V1]` with nodes 1, 2 and 3 all present, the one
/// pair `Va−Vb` (ascending) with exactly two of them, `None` with fewer: a single
/// phase has no line-to-line voltage, and neither has a bus of pure
/// neutral/return nodes.
///
/// This is what r4133's own report path computes for the same bus
/// (`Common/ShowResults.pas:193-194` wraps the phase number *before* looking the
/// node up). The two DDLL / C-API arms poll before wrapping and therefore pair
/// phase 3 with node 4, or a node with itself — see [`BusVoltageView`].
fn bus_line_to_line(
    ckt: &Circuit,
    bus: &crate::circuit::bus::Bus,
) -> Option<Vec<num_complex::Complex64>> {
    /// `(1,2) (2,3) (3,1)` as indices into the present-phase list.
    const THREE_PHASE_PAIRS: [(usize, usize); 3] = [(0, 1), (1, 2), (2, 0)];
    /// The single pair of a two-phase bus, in ascending phase order.
    const TWO_PHASE_PAIR: [(usize, usize); 1] = [(0, 1)];

    // `phase_refs` is already in ascending phase order, so filtering it keeps
    // the pairs in ascending order too.
    let present: Vec<usize> = phase_refs(bus).into_iter().flatten().collect();
    let pairs: &[(usize, usize)] = match present.len() {
        3 => &THREE_PHASE_PAIRS,
        2 => &TWO_PHASE_PAIR,
        _ => return None,
    };
    Some(
        pairs
            .iter()
            .map(|&(a, b)| node_voltage(ckt, present[a]) - node_voltage(ckt, present[b]))
            .collect(),
    )
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

/// Build one [`BusVoltageView`] over bus `bus_idx` (`BusList` index). `sc` is the
/// sequence transform, hoisted into the caller so a whole-circuit sweep builds
/// the two 3×3 matrices once instead of once per bus.
fn bus_voltage_view(ckt: &Circuit, bus_idx: usize, sc: &SymComp) -> BusVoltageView {
    use crate::support::complexutil::c_to_polar_deg;

    let bus = &ckt.buses[bus_idx];
    let base_factor = bus_base_factor(bus);
    let n = bus.num_nodes_this_bus();

    let mut node_v = Vec::with_capacity(n);
    let mut pu_voltages = Vec::with_capacity(n);
    let mut vmag_angle = Vec::with_capacity(n);
    let mut pu_vmag_angle = Vec::with_capacity(n);
    for i in ascending_node_indices(bus) {
        let v = node_voltage(ckt, bus.get_ref(i));
        node_v.push(v);
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

    let cplx_seq_voltages = bus_seq_voltages(ckt, bus, sc);
    let vll = bus_line_to_line(ckt, bus);
    let ll_base = bus_ll_base_factor(bus);

    BusVoltageView {
        name: bus.name.clone(),
        kv_base: bus.kv_base,
        nodes: bus.nodes.clone(),
        pu_voltages,
        vmag_angle,
        pu_vmag_angle,
        node_v,
        // Pascal `Cabs`, the naive modulus, proven bit-identical to
        // `Complex::norm` on every reachable operand
        // (`support/line_constants/tests.rs`,
        // `naive_modulus_equals_hypot_until_the_square_overflows`).
        seq_voltages: cplx_seq_voltages.map(|v| [v[0].norm(), v[1].norm(), v[2].norm()]),
        cplx_seq_voltages,
        // capi divides the two components (`CAPI_Alt.pas:2464-2467`), r4133
        // calls `cdivreal` (`DBus.pas:644`): the same componentwise divide the
        // pu voltages above take.
        pu_vll: vll.as_ref().map(|v| {
            v.iter()
                .map(|z| num_complex::Complex64::new(z.re / ll_base, z.im / ll_base))
                .collect()
        }),
        vll,
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
            // Currents: fresh recompute from the converged `NodeV` (oracle
            // `GetCurrents`), overwriting the `Iterminal` cache after Powers/Losses.
            if elem.cd().enabled && !elem.cd().node_ref.is_empty() {
                elem.refresh_iterminal(&sys, &node_v);
                let cd = elem.cd();
                currents.copy_from_slice(&cd.iterminal[..yorder]);
            }
            let cd = elem.cd();
            let bus_names = (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect();
            out.push(ElementSnapshot {
                name,
                enabled: cd.enabled,
                bus_names,
                powers,
                currents,
                loss_w: (loss.re, loss.im),
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
        Some(bus_voltage_view(ckt, idx, &SymComp::default()))
    }

    /// Every bus's [`BusVoltageView`] in `BusList` order — the order
    /// `Circuit.AllBusNames` reports (`CAPI_Circuit.pas` /
    /// `DCircuit.pas:439`). Empty when no circuit exists.
    pub fn all_bus_voltages(&self) -> Vec<BusVoltageView> {
        match self.circuit.as_ref() {
            Some(ckt) => {
                let sc = SymComp::default();
                (0..ckt.buses.len())
                    .map(|i| bus_voltage_view(ckt, i, &sc))
                    .collect()
            }
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
mod bus_seq_vll_tests {
    use super::*;
    use num_complex::Complex64;

    /// One deck carrying every node-set class the corpus's exception buses have
    /// (measured for GOLDEN_REBASE G1.4c over 209 211 corpus buses):
    ///
    /// * `b4` = `[1, 2, 3, 4]` — 107 corpus buses (`Test/indmachtest` `sourcebus`,
    ///   `IEEETestCases/NEVTestCase` `13kvbus`, …): all three phases **plus** a
    ///   fourth node.
    /// * `bx` = `[1, 2, 10]` — 16 corpus buses (NEVTestCase `load1a`, …): three
    ///   nodes, only two of them phases.
    /// * `by` = `[1, 10]` — 52 corpus buses (NEVTestCase `ckt1-1-1`, …).
    /// * `bz` = `[2, 3]` — the two-phase class (48 104 corpus buses with two
    ///   phase nodes).
    /// * `brot` = `[2, 1, 3]` — non-prefix insertion order, for the `node_v`
    ///   ordering contract.
    ///
    /// The extra nodes are made real by shunt capacitors, so every node is in `Y`
    /// and the deck solves.
    fn seq_vll_deck() -> Dss {
        let mut dss = Dss::new();
        dss.command("New circuit.busseqvll basekv=12.47 pu=1.0 phases=3 bus1=sourcebus");
        dss.command(
            "New Line.l4 bus1=sourcebus.1.2.3 bus2=b4.1.2.3 phases=3 r1=0.1 x1=0.3 c1=0 length=1",
        );
        dss.command("New Capacitor.c4 bus1=b4.4 phases=1 kv=7.2 kvar=100");
        dss.command(
            "New Line.lx bus1=sourcebus.1.2 bus2=bx.1.2 phases=2 r1=0.1 x1=0.3 c1=0 length=1",
        );
        dss.command("New Capacitor.cx bus1=bx.10 phases=1 kv=7.2 kvar=100");
        dss.command("New Line.ly bus1=sourcebus.1 bus2=by.1 phases=1 r1=0.1 x1=0.3 c1=0 length=1");
        dss.command("New Capacitor.cy bus1=by.10 phases=1 kv=7.2 kvar=100");
        dss.command(
            "New Line.lz bus1=sourcebus.2.3 bus2=bz.2.3 phases=2 r1=0.1 x1=0.3 c1=0 length=1",
        );
        dss.command(
            "New Line.lr bus1=sourcebus.1.2.3 bus2=brot.2.1.3 phases=3 r1=0.1 x1=0.3 c1=0 length=1",
        );
        dss.command("New Load.ld bus1=b4.1.2.3 phases=3 kv=12.47 kw=500 pf=0.95");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    }

    fn solved_with_bases() -> Dss {
        let mut dss = seq_vll_deck();
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    }

    /// The node numbers a bus ended up with, in `Bus::nodes` insertion order.
    fn nodes_of(dss: &Dss, name: &str) -> Vec<i32> {
        dss.bus_voltages(name).expect("bus").nodes
    }

    /// The port's `Bus.SeqVoltages` / `CplxSeqVoltages` exist **iff** the bus
    /// carries all three phase nodes (S-SEQ), which is neither engine's rule:
    /// capi tests `NumNodesThisBus` after clamping it to 3
    /// (`CAPI_Alt.pas:2174-2186`), r4133 tests it unclamped
    /// (`DBus.pas:296-300`), and both index the transform with `Find(i)`, whose
    /// `0` for an absent phase is `NodeV`'s ground slot (`DBus.pas:305` ==
    /// `CAPI_Alt.pas:2190`).
    ///
    /// The two directions of the disagreement, live on the corpus and pinned
    /// against both channels by the gate: on NEVTestCase `13kvbus` (`[1,2,3,4,10]`)
    /// capi answers `[93.76039163143575, 7698.828477395033, 30.49852433770403]`
    /// and r4133 answers `[-1, -1, -1]` while the port computes the real V012;
    /// on `load1a` (`[1,2,10]`) both oracles answer
    /// `[18.35739…, 73.53445…, 64.45945…]` — the transform of `[V1, V2, 0 V]` —
    /// while the port declines.
    #[test]
    fn bus_seq_voltages_need_all_three_phase_nodes() {
        let dss = solved_with_bases();
        let sc = SymComp::default();
        let ckt = dss.circuit().expect("circuit");

        // A bus with all three phases and a fourth node: available, and equal to
        // the transform of its own three phase voltages.
        assert_eq!(nodes_of(&dss, "b4"), vec![1, 2, 3, 4]);
        let v4 = dss.bus_voltages("b4").expect("bus b4");
        let bus4 = &ckt.buses[ckt.bus_list.find("b4").expect("b4")];
        let vph = [
            ckt.solution.node_v[bus4.find(1)],
            ckt.solution.node_v[bus4.find(2)],
            ckt.solution.node_v[bus4.find(3)],
        ];
        let mut want = [Complex64::ZERO; 3];
        sc.phase_to_sym(&vph, &mut want);
        assert_eq!(v4.cplx_seq_voltages, Some(want));
        assert_eq!(
            v4.seq_voltages,
            Some([want[0].norm(), want[1].norm(), want[2].norm()]),
            "the magnitude form is Cabs of the complex form, slot for slot"
        );
        // Physically a sequence answer, not a placeholder: a near-balanced bus
        // puts everything in V1 and leaves V0/V2 tiny.
        let m = v4.seq_voltages.expect("seq");
        assert!(m[1] > 6_000.0, "V1 = {} V", m[1]);
        assert!(m[0] < 0.01 * m[1] && m[2] < 0.01 * m[1], "V0/V2 = {m:?}");

        // Three nodes, only two of them phases: the port declines, while both
        // oracles publish the transform of a fabricated 0 V phase 3 — a number
        // this test computes to show it is neither zero nor the port's answer.
        assert_eq!(nodes_of(&dss, "bx"), vec![1, 2, 10]);
        let vx = dss.bus_voltages("bx").expect("bus bx");
        assert_eq!(vx.seq_voltages, None);
        assert_eq!(vx.cplx_seq_voltages, None);
        let busx = &ckt.buses[ckt.bus_list.find("bx").expect("bx")];
        let ground_substituted = [
            ckt.solution.node_v[busx.find(1)],
            ckt.solution.node_v[busx.find(2)],
            ckt.solution.node_v[busx.find(3)], // Find(3) = 0 = ground
        ];
        assert_eq!(
            ground_substituted[2],
            Complex64::ZERO,
            "phase 3 is absent, so both oracles read NodeV[0]"
        );
        let mut fabricated = [Complex64::ZERO; 3];
        sc.phase_to_sym(&ground_substituted, &mut fabricated);
        assert!(
            fabricated[0].norm() > 1_000.0,
            "the oracles' V0 on this bus is a large fabricated number, not noise: {}",
            fabricated[0].norm()
        );

        // Two phase nodes, and one phase node: no sequence answer either.
        assert_eq!(nodes_of(&dss, "bz"), vec![2, 3]);
        assert_eq!(dss.bus_voltages("bz").expect("bz").seq_voltages, None);
        assert_eq!(nodes_of(&dss, "by"), vec![1, 10]);
        assert_eq!(dss.bus_voltages("by").expect("by").cplx_seq_voltages, None);

        // The ordinary three-phase bus is unaffected by all of this.
        let vr = dss.bus_voltages("brot").expect("brot");
        assert!(vr.seq_voltages.is_some(), "a [2,1,3] bus has all 3 phases");
    }

    /// The port's `Bus.VLL` pairs the phase nodes the bus actually carries
    /// (S-VLL) — `[V1−V2, V2−V3, V3−V1]`, one pair, or nothing — where both
    /// engines instead poll `FindIdx(jj)` *before* the `jj > 3 ⇒ jj := 1` wrap
    /// (`DBus.pas:575-584` == `CAPI_Alt.pas:2500-2523`).
    ///
    /// Live on the corpus, and the number the gate pins against both channels:
    /// on `Test/indmachtest/Master.DSS` `sourcebus` (`[1,2,3,4]`) both oracles'
    /// third L-L entry is `V3 − V4 = (-33107.438616, 57475.919433)` while the
    /// port reports `V3 − V1 = (-99436.757…, 57541.990…)`; on NEVTestCase
    /// `ckt1-1-1` (`[1,10]`) capi returns its one-element `DefaultResult` `[0.0]`
    /// and r4133 pairs node 1 with itself for `[0.0, 0.0]`, where the port has no
    /// line-to-line voltage at all.
    #[test]
    fn bus_vll_pairs_only_the_phase_nodes_the_bus_carries() {
        let dss = solved_with_bases();
        let ckt = dss.circuit().expect("circuit");

        // Three phases + a fourth node: three pairs, the last one closing the
        // triangle on phase 1 — not on node 4, the way both oracles walk it.
        let v4 = dss.bus_voltages("b4").expect("bus b4");
        let bus4 = &ckt.buses[ckt.bus_list.find("b4").expect("b4")];
        let v = |num: i32| ckt.solution.node_v[bus4.find(num)];
        let ll = v4.vll.clone().expect("b4 has three phases");
        assert_eq!(ll, vec![v(1) - v(2), v(2) - v(3), v(3) - v(1)]);
        let upstream_third = v(3) - v(4);
        assert!(
            (ll[2] - upstream_third).norm() > 1_000.0,
            "the two pairings are observably different: port {} vs upstream {}",
            ll[2],
            upstream_third
        );
        // A real line-to-line voltage: sqrt(3) times the phase magnitude.
        assert!(
            (ll[0].norm() / v(1).norm() - crate::util::sqrt3()).abs() < 0.05,
            "|V1-V2| / |V1| = {}",
            ll[0].norm() / v(1).norm()
        );

        // Exactly two phase nodes (plus a tenth): the single pair over the
        // phases present, where upstream emits three entries starting with the
        // same pair and continuing with its negative.
        let vx = dss.bus_voltages("bx").expect("bus bx");
        let busx = &ckt.buses[ckt.bus_list.find("bx").expect("bx")];
        let vx_n = |num: i32| ckt.solution.node_v[busx.find(num)];
        assert_eq!(vx.vll, Some(vec![vx_n(1) - vx_n(2)]));

        // Two phase nodes that are not 1 and 2: still one pair, ascending.
        let vz = dss.bus_voltages("bz").expect("bus bz");
        let busz = &ckt.buses[ckt.bus_list.find("bz").expect("bz")];
        assert_eq!(
            vz.vll,
            Some(vec![
                ckt.solution.node_v[busz.find(2)] - ckt.solution.node_v[busz.find(3)]
            ])
        );

        // One phase node: no line-to-line voltage exists.
        let vy = dss.bus_voltages("by").expect("bus by");
        assert_eq!(nodes_of(&dss, "by"), vec![1, 10]);
        assert_eq!(vy.vll, None);
        assert_eq!(vy.pu_vll, None);

        // `pu_vll` is `Some` exactly when `vll` is, entry for entry.
        for name in ["sourcebus", "b4", "bx", "by", "bz", "brot"] {
            let view = dss.bus_voltages(name).expect("bus");
            assert_eq!(
                view.vll.as_ref().map(Vec::len),
                view.pu_vll.as_ref().map(Vec::len),
                "{name}"
            );
        }
    }

    /// `puVLL` divides by the **line-to-line** base `1000 · kVBase · √3`
    /// (`DBus.pas:622-623` == `CAPI_Alt.pas:2427-2430`) — a different constant
    /// from the line-to-neutral `BaseFactor` the other per-unit arrays use — and
    /// falls back to `1.0` on a bus with no base, an arm the corpus reaches
    /// (`Test/indmachtest/Master.DSS` `sourcebus`, `kVBase = 0`, where both
    /// oracles report `puVLL == VLL` in volts).
    #[test]
    fn bus_pu_vll_divides_by_the_line_to_line_base() {
        let dss = solved_with_bases();
        let v4 = dss.bus_voltages("b4").expect("bus b4");
        let ll_base = 1000.0 * v4.kv_base * crate::util::sqrt3();
        assert!(ll_base > 0.0);
        let ll = v4.vll.clone().expect("vll");
        let pu = v4.pu_vll.clone().expect("pu_vll");
        for (k, (z, p)) in ll.iter().zip(&pu).enumerate() {
            assert_eq!(
                *p,
                Complex64::new(z.re / ll_base, z.im / ll_base),
                "entry {k} is a componentwise divide"
            );
        }
        assert!(
            (0.5..1.5).contains(&pu[0].norm()),
            "per-unit line-to-line, not per-kV: {}",
            pu[0].norm()
        );
        // The line-to-neutral base would be sqrt(3) too large here.
        assert!(
            (pu[0].norm() * crate::util::sqrt3() - ll[0].norm() / (1000.0 * v4.kv_base)).abs()
                < 1e-9,
            "the two bases differ by exactly sqrt(3)"
        );

        // No `CalcVoltageBases`: every bus keeps `TDSSBus.Create`'s kVBase = 0
        // and the line-to-line base falls back to 1.0.
        let mut plain = seq_vll_deck();
        plain.command("Solve");
        assert!(plain.errors().is_empty(), "{:?}", plain.errors());
        let v4 = plain.bus_voltages("b4").expect("bus b4");
        assert_eq!(v4.kv_base, 0.0);
        assert_eq!(v4.pu_vll, v4.vll, "BaseFactor = 1.0 gives raw volts");
        assert!(
            v4.pu_vll.expect("vll")[0].norm() > 1_000.0,
            "unit base gives volts"
        );
    }

    /// `node_v` is the raw `NodeV` behind the per-unit arrays, in the same
    /// ascending node-**number** order — so a consumer can replay an upstream
    /// pairing over the port's own state without re-deriving the bus's refs.
    #[test]
    fn bus_node_v_is_the_raw_voltage_behind_the_per_unit_arrays() {
        let dss = solved_with_bases();
        let ckt = dss.circuit().expect("circuit");

        // A bus declared `.2.1.3`: `nodes` keeps insertion order, `node_v` does
        // not — it is ordered 1, 2, 3 like `pu_voltages`.
        let v = dss.bus_voltages("brot").expect("brot");
        assert_eq!(v.nodes, vec![2, 1, 3], "declared .2.1.3");
        let bus = &ckt.buses[ckt.bus_list.find("brot").expect("brot")];
        assert_eq!(v.node_v.len(), v.pu_voltages.len());
        let bf = 1000.0 * v.kv_base;
        for (k, num) in [1i32, 2, 3].into_iter().enumerate() {
            assert_eq!(v.node_v[k], ckt.solution.node_v[bus.find(num)]);
            assert_eq!(
                v.pu_voltages[k],
                Complex64::new(v.node_v[k].re / bf, v.node_v[k].im / bf),
                "slot {k} is node {num} in both arrays"
            );
            assert_eq!(v.vmag_angle[k].0, v.node_v[k].norm());
        }

        // A four-node bus carries all four raw voltages, node 4 included, even
        // though only the three phases feed the sequence and L-L answers.
        let v4 = dss.bus_voltages("b4").expect("b4");
        assert_eq!(v4.node_v.len(), 4);
        let bus4 = &ckt.buses[ckt.bus_list.find("b4").expect("b4")];
        for (k, num) in [1i32, 2, 3, 4].into_iter().enumerate() {
            assert_eq!(v4.node_v[k], ckt.solution.node_v[bus4.find(num)]);
        }
        assert_eq!(v4.vll.expect("vll").len(), 3, "node 4 is not a phase");
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
