//! Public query/snapshot API over [`Dss`] for the golden/test harness
//! (monitor buffers, meter zones, element snapshots, system Y, ...).
//! Split out of `exec/mod.rs`.

use super::*;
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
}
