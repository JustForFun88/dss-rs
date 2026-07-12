//! Base circuit-element state, port of `Common/CktElement.pas`
//! (`TDSSCktElement` fields and non-virtual behavior).
//!
//! Pascal reaches the active circuit through globals from inside the element
//! (`Set_YprimInvalid` sets `Solution.SystemYChanged`, `SetBus` sets
//! `Circuit.BusNameRedefined`). Here the element records those signals in
//! its own flags and the executive/solver propagates them after each edit —
//! behaviorally identical because nothing reads the global flags mid-edit.

use num_complex::Complex64;

use crate::circuit::Terminal;
use crate::elements::traits::ElemRef;
use crate::obj::base::DssObjData;
use crate::report::format::strip_extension;
use crate::support::cmatrix::{CMatrix, cdiv_fpc};
use crate::util::EPSILON;

/// Element status flags — the element-level subset of Pascal
/// `TDSSObjectFlag` (`Common/DSSClass.pas` l.133). The property-engine flags
/// (`EditingActive`, `HasBeenSaved`, `DefaultAndUnedited`, `NeedsRecalc`,
/// `NeedsYPrim`) are handled by other mechanisms in this port and are not
/// represented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ElemFlags(u32);

impl ElemFlags {
    pub const NONE: Self = Self(0);
    /// `Flg.Checked` — topology-search visit marker.
    pub const CHECKED: Self = Self(1 << 0);
    /// `Flg.Flag` — general-purpose scratch flag ("don't assume inited").
    pub const FLAG: Self = Self(1 << 1);
    /// `Flg.HasEnergyMeter` — an EnergyMeter is metering this element.
    pub const HAS_ENERGY_METER: Self = Self(1 << 2);
    /// `Flg.HasSensorObj` — a Sensor is metering this element.
    pub const HAS_SENSOR_OBJ: Self = Self(1 << 3);
    /// `Flg.IsIsolated` — not reached by any meter-zone/topology sweep.
    pub const IS_ISOLATED: Self = Self(1 << 4);
    /// `Flg.HasControl` — some control element controls this element.
    pub const HAS_CONTROL: Self = Self(1 << 5);
    /// `Flg.IsMonitored` — some control element monitors this element.
    pub const IS_MONITORED: Self = Self(1 << 6);
    /// `Flg.HasOCPDevice` — Fuse, Relay or Recloser attached.
    pub const HAS_OCP_DEVICE: Self = Self(1 << 7);
    /// `Flg.HasAutoOCPDevice` — Relay or Recloser only.
    pub const HAS_AUTO_OCP_DEVICE: Self = Self(1 << 8);

    /// Pascal `<flag> in Flags`.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Pascal `Include(Flags, <flag>)`.
    pub fn include(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Pascal `Exclude(Flags, <flag>)`.
    pub fn exclude(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

impl std::ops::BitOr for ElemFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Shared circuit-element data (`TDSSCktElement` fields).
#[derive(Debug, Clone)]
pub struct CktElementData {
    /// Common DSS object data (name, prp_sequence).
    pub obj: DssObjData,
    /// Pascal `FEnabled`.
    pub enabled: bool,
    /// Pascal `FNterms`.
    pub nterms: usize,
    /// Pascal `Fnconds`.
    pub nconds: usize,
    /// Pascal `Fnphases`.
    pub nphases: usize,
    /// Pascal `FBusNames` (1 per terminal, lowercased).
    pub bus_names: Vec<String>,
    /// `Yorder = Fnterms * Fnconds`.
    pub yorder: usize,
    /// Pascal `BaseFrequency`.
    pub base_frequency: f64,
    /// Pascal `FYprimFreq` (frequency at which Yprim was computed).
    pub yprim_freq: f64,
    /// Pascal `FYPrimInvalid`.
    pub yprim_invalid: bool,
    /// Full primitive Y (series + shunt).
    pub yprim: Option<CMatrix>,
    pub yprim_series: Option<CMatrix>,
    pub yprim_shunt: Option<CMatrix>,
    /// Per-conductor global node references; `node_ref[t*nconds + c]`.
    /// Empty until `SetNodeRef` runs (Pascal `NodeRef = NIL`).
    pub node_ref: Vec<usize>,
    /// Pascal `Vterminal` / `Iterminal` / `ComplexBuffer`, length `yorder`.
    pub vterminal: Vec<Complex64>,
    pub iterminal: Vec<Complex64>,
    pub complex_buffer: Vec<Complex64>,
    /// Pascal `InjCurrent` (TPCElement; harmless for PD elements).
    pub inj_current: Vec<Complex64>,
    pub terminals: Vec<Terminal>,
    pub terminals_checked: Vec<bool>,
    /// Active terminal, 0-based (`FActiveTerminal`).
    pub active_terminal: usize,
    /// Pascal `IterminalSolutionCount`.
    pub iterminal_solution_count: i32,
    /// Pascal `ITerminalUpdated` (TPCElement).
    pub iterminal_updated: bool,
    /// `CktElements` handle (1-based position; 0 = not in circuit).
    pub handle: usize,
    /// Signal to the executive: a bus name / conductor count / enabled state
    /// changed, so the circuit must set `BusNameRedefined` (which in Pascal
    /// happens immediately through the `ActiveCircuit` global).
    pub signal_bus_name_redefined: bool,
    /// Signal to the executive: reset `Solution.SolutionInitialized` (Pascal
    /// writes it straight through the `ActiveCircuit` global from a property
    /// side effect — e.g. adding a model-3 generator forces a DQDV re-init).
    pub signal_reset_solution_initialized: bool,

    /// Pascal `Flags` (`TDSSObjectFlags`), element-level subset.
    pub flags: ElemFlags,

    // --- Meter-zone fields (Pascal `TPDElement`, with `meter_obj`/
    // --- `sensor_obj` also on `TPCElement`). Written by the EnergyMeter
    // --- zone build (`MakeMeterZoneLists`); meaningful for PD elements and
    // --- (the refs) for zone PC elements, inert otherwise.
    /// `FromTerminal`: terminal (1-based) facing the meter on a radial feeder.
    pub from_terminal: usize,
    /// `ToTerminal`: set by the reliability sweep from `from_terminal`.
    pub to_terminal: usize,
    /// `ParentPDElement`: the upline branch in the meter zone.
    pub parent_pd: Option<ElemRef>,
    /// `MeterObj`: upline EnergyMeter.
    pub meter_obj: Option<ElemRef>,
    /// `SensorObj`: upline Sensor/meter for allocation and estimation.
    pub sensor_obj: Option<ElemRef>,
    /// `BranchNumCustomers` (customers connected directly to this branch).
    pub branch_num_customers: i32,
    /// `BranchTotalCustomers` (customers downstream incl. this branch).
    pub branch_total_customers: i32,
    /// `TPDElement.Overload_EEN`: degree of normal-rating overload, set as a
    /// side effect of `excess_kva_norm`. Inert on non-PD elements.
    pub overload_een: f64,
    /// `TPDElement.Overload_UE`: degree of emergency-rating overload, set as a
    /// side effect of `excess_kva_emerg`.
    pub overload_ue: f64,

    // --- Reliability accumulators (Pascal `TPDElement` l.32-48), written by
    // --- the EnergyMeter reliability sweep (`CalcReliabilityIndices`). Inert
    // --- on non-PD elements (never appear in a meter `SequenceList`).
    /// `BranchFltRate`: net failure rate for this branch (`CalcFltRate`).
    pub branch_flt_rate: f64,
    /// `AccumulatedBrFltRate`: failure rate accumulated to this branch.
    pub accumulated_br_flt_rate: f64,
    /// `AccumulatedMilesDownStream`: total line miles downstream of this branch.
    pub accumulated_miles_downstream: f64,
    /// `BranchSectionID`: feeder section this branch belongs to.
    pub branch_section_id: i32,
    /// `GetOCPDeviceType` ordinal of the over-current-protection control at the
    /// head of this branch's section: 0=none, 1=Fuse, 2=Recloser, 3=Relay. Set
    /// when an enabled Relay/Recloser/Fuse resolves its controlled element (the
    /// first OCP control registered wins, matching Pascal `GetOCPDeviceType`'s
    /// `ControlElementList` scan, which stops at the first match). Read only by
    /// the reliability sweep when `HAS_OCP_DEVICE` is set.
    pub ocp_device_type: i32,
}

impl CktElementData {
    /// Base construction; concrete classes set phases/conds/terms right after
    /// (the Pascal constructors assign `FNphases`/`Fnconds`/`Nterms`).
    pub fn new(name: &str, num_props: usize) -> Self {
        Self {
            obj: DssObjData::new(name.to_lowercase(), num_props),
            enabled: true,
            nterms: 0,
            nconds: 0,
            nphases: 3,
            bus_names: Vec::new(),
            yorder: 0,
            base_frequency: 60.0,
            yprim_freq: 0.0,
            yprim_invalid: true,
            yprim: None,
            yprim_series: None,
            yprim_shunt: None,
            node_ref: Vec::new(),
            vterminal: Vec::new(),
            iterminal: Vec::new(),
            complex_buffer: Vec::new(),
            inj_current: Vec::new(),
            terminals: Vec::new(),
            terminals_checked: Vec::new(),
            active_terminal: 0,
            iterminal_solution_count: -1,
            iterminal_updated: false,
            handle: 0,
            signal_bus_name_redefined: false,
            signal_reset_solution_initialized: false,
            flags: ElemFlags::NONE,
            // Pascal `TPDElement.Create`: `FromTerminal := 1`.
            from_terminal: 1,
            to_terminal: 0,
            parent_pd: None,
            meter_obj: None,
            sensor_obj: None,
            branch_num_customers: 0,
            branch_total_customers: 0,
            overload_een: 0.0,
            overload_ue: 0.0,
            branch_flt_rate: 0.0,
            accumulated_br_flt_rate: 0.0,
            accumulated_miles_downstream: 0.0,
            branch_section_id: 0,
            ocp_device_type: 0,
        }
    }

    /// Pascal `Set_NTerms`: (re)allocate bus names, terminals and the V/I
    /// buffers. New bus-name slots default to `name_i`.
    pub fn set_nterms(&mut self, value: usize) {
        if value == 0 {
            return; // Pascal records error 749 and exits
        }
        if value == self.nterms && value * self.nconds == self.yorder {
            return;
        }

        if value < self.nterms {
            self.bus_names.truncate(value);
        } else {
            for i in self.bus_names.len()..value {
                self.bus_names
                    .push(format!("{}_{}", self.obj.name(), i + 1));
            }
        }

        self.terminals = (0..value).map(|_| Terminal::init(self.nconds)).collect();
        self.terminals_checked = vec![false; value];

        self.nterms = value;
        self.yorder = self.nterms * self.nconds;
        self.vterminal.resize(self.yorder, Complex64::ZERO);
        self.iterminal.resize(self.yorder, Complex64::ZERO);
        self.complex_buffer.resize(self.yorder, Complex64::ZERO);
    }

    /// Pascal `Set_NConds`: changing the conductor count reallocates the
    /// terminal info and flags `BusNameRedefined`.
    pub fn set_nconds(&mut self, value: usize) {
        if value == 0 {
            return; // Pascal records error 749 and exits
        }
        if value != self.nconds {
            self.signal_bus_name_redefined = true;
        }
        self.nconds = value;
        let nterms = self.nterms;
        // Force reallocation (Pascal calls Set_Nterms(FNterms)).
        self.nterms = 0;
        self.set_nterms(nterms);
    }

    /// Pascal `Set_Enabled`: a change forces rebuilding of bus lists and Y.
    pub fn set_enabled(&mut self, value: bool) {
        if value == self.enabled {
            return;
        }
        self.enabled = value;
        self.signal_bus_name_redefined = true;
    }

    /// Pascal `SetBus` (1-based terminal).
    pub fn set_bus(&mut self, i: usize, s: &str) {
        if i >= 1 && i <= self.nterms {
            self.bus_names[i - 1] = s.to_lowercase();
            self.signal_bus_name_redefined = true;
        }
        // else: Pascal DoSimpleMsg 7541; the executive records that
    }

    /// Pascal `GetBus` (1-based terminal; out of range → empty).
    pub fn get_bus(&self, i: usize) -> &str {
        if i >= 1 && i <= self.nterms {
            &self.bus_names[i - 1]
        } else {
            ""
        }
    }

    /// Pascal `SetNodeRef`: copy one terminal's node refs (1-based terminal)
    /// into the flat array and the terminal record, growing storage to
    /// `yorder` like the `ReallocMem` calls.
    pub fn set_node_ref(&mut self, iterm: usize, node_ref_array: &[usize]) {
        self.node_ref.resize(self.yorder, 0);
        let offset = (iterm - 1) * self.nconds;
        self.node_ref[offset..offset + self.nconds].copy_from_slice(&node_ref_array[..self.nconds]);
        self.terminals[iterm - 1].term_node_ref[..self.nconds]
            .copy_from_slice(&node_ref_array[..self.nconds]);
        self.vterminal.resize(self.yorder, Complex64::ZERO);
        self.iterminal.resize(self.yorder, Complex64::ZERO);
        self.complex_buffer.resize(self.yorder, Complex64::ZERO);
    }

    /// Pascal `ComputeVterminal`: `Vterminal[i] = NodeV[NodeRef[i]]`
    /// (`NodeV[0]` is the always-zero ground slot).
    pub fn compute_vterminal(&mut self, node_v: &[Complex64]) {
        if self.node_ref.is_empty() {
            return;
        }
        for i in 0..self.yorder {
            self.vterminal[i] = node_v[self.node_ref[i]];
        }
    }

    /// Pascal `TPCElement.CalcYPrimContribution` (`PCElement.pas` l.162):
    /// `ComputeVTerminal` then `Curr = YPrim · Vterminal` — the frozen
    /// shadow-admittance terminal current the `LastSolutionWasDirect`
    /// `GetCurrents` shortcut reports. Note it neither subtracts `InjCurrent`
    /// nor marks `Iterminal` updated (the Pascal routine touches only
    /// `Vterminal` and `Curr`).
    pub fn calc_yprim_contribution(&mut self, node_v: &[Complex64], curr: &mut [Complex64]) {
        self.compute_vterminal(node_v);
        if let Some(yprim) = &self.yprim {
            yprim.mv_mult(curr, &self.vterminal);
        }
    }

    /// Pascal `ZeroITerminal`.
    pub fn zero_iterminal(&mut self) {
        self.iterminal.fill(Complex64::ZERO);
    }

    /// Pascal `TPCElement.ZeroInjCurrent`.
    pub fn zero_inj_current(&mut self) {
        self.inj_current.fill(Complex64::ZERO);
    }

    /// Pascal `AllConductorsClosed`.
    pub fn all_conductors_closed(&self) -> bool {
        self.terminals
            .iter()
            .all(|t| t.conductors_closed.iter().all(|&c| c))
    }

    /// Pascal `ActiveTerminalIdx := terminal; Set_ConductorClosed(0, value)`:
    /// open/close **every phase conductor** of the 1-based `terminal` (the
    /// `Closed[0]` "all conductors" branch) and mark YPrim invalid (Pascal's
    /// `Set_ConductorClosed` raises the global `SystemYChanged`; the caller
    /// propagates `yprim_invalid`). Used by SwtControl (and the protection
    /// devices) to switch a controlled element's terminal. Out-of-range
    /// terminals are ignored, matching Pascal's index guards.
    pub fn set_terminal_closed(&mut self, terminal: usize, value: bool) {
        if terminal >= 1 && terminal <= self.nterms {
            self.active_terminal = terminal - 1;
            let t = &mut self.terminals[terminal - 1];
            for i in 0..self.nphases {
                t.conductors_closed[i] = value;
            }
            self.yprim_invalid = true;
        }
    }

    /// Pascal `ActiveTerminalIdx := terminal; Set_ConductorClosed(index, value)`
    /// for a **single** 1-based conductor (`index > 0`): set only that conductor of
    /// `terminal` and mark YPrim invalid. Pascal guards `Index <= Fnconds`
    /// (`CktElement.pas`), so the `Open`/`Close` exec verbs can open a neutral
    /// conductor (`cond > Nphases`); the Fuse's per-phase blow only ever passes
    /// `1..=Nphases`. Out-of-range terminals/conductors are ignored, like Pascal.
    pub fn set_conductor_closed(&mut self, terminal: usize, conductor: usize, value: bool) {
        if terminal >= 1 && terminal <= self.nterms && conductor >= 1 && conductor <= self.nconds {
            self.active_terminal = terminal - 1;
            self.terminals[terminal - 1].conductors_closed[conductor - 1] = value;
            self.yprim_invalid = true;
        }
    }

    /// Pascal `Get_ConductorClosed(index)` for a **single** 1-based conductor of
    /// `terminal`: `true` iff that conductor is closed (Pascal guards
    /// `Index <= Fnconds`). An out-of-range terminal/conductor reads as open
    /// (`false`).
    pub fn conductor_closed(&self, terminal: usize, conductor: usize) -> bool {
        if terminal >= 1 && terminal <= self.nterms && conductor >= 1 && conductor <= self.nconds {
            self.terminals[terminal - 1].conductors_closed[conductor - 1]
        } else {
            false
        }
    }

    /// Pascal `Get_ConductorClosed(0)` with the active terminal set to the
    /// 1-based `terminal`: `true` iff every phase conductor of that terminal is
    /// closed. An out-of-range terminal reads as open (`false`).
    pub fn terminal_all_phases_closed(&self, terminal: usize) -> bool {
        if terminal >= 1 && terminal <= self.nterms {
            let t = &self.terminals[terminal - 1];
            (0..self.nphases).all(|i| t.conductors_closed[i])
        } else {
            false
        }
    }

    /// Pascal `TDSSCktElement.DoYprimCalcs`: Kron-reduce rows/columns of open
    /// conductors out of `ymatrix`, then zero them and pin a tiny epsilon on
    /// the diagonal; finally add epsilon to all remaining diagonals so no bus
    /// is left hanging. (Indices here are 0-based.)
    #[allow(clippy::needless_range_loop)] // loop-for-loop Pascal port
    pub fn do_yprim_calcs(&self, ymatrix: &mut CMatrix) {
        let yorder = self.yorder;
        let mut element_open = false;
        let mut row_eliminated: Vec<bool> = Vec::new();
        let c_epsilon = Complex64::new(EPSILON, 0.0);

        let mut k = 0usize;
        for term in &self.terminals {
            for j in 0..self.nconds {
                if !term.conductors_closed[j] {
                    if !element_open {
                        row_eliminated = vec![false; yorder];
                        element_open = true;
                    }
                    // Kron reduction of the eliminated row.
                    let elim = j + k;
                    let mut ynn = ymatrix.get(elim, elim);
                    if ynn.norm() == 0.0 {
                        ynn.re = EPSILON;
                    }
                    row_eliminated[elim] = true;
                    for ii in 0..yorder {
                        if !row_eliminated[ii] {
                            let yin = ymatrix.get(ii, elim);
                            for jj in ii..yorder {
                                if !row_eliminated[jj] {
                                    let yij = ymatrix.get(ii, jj);
                                    let ynj = ymatrix.get(elim, jj);
                                    // FPC ucomplex `/` (Smith), as Pascal
                                    // `DoYPrimCalcs` uses — same cancellation-
                                    // sensitive Kron term as `CMatrix::kron`.
                                    let v = yij - cdiv_fpc(yin * ynj, ynn);
                                    ymatrix.set(ii, jj, v);
                                    ymatrix.set(jj, ii, v);
                                }
                            }
                        }
                    }
                    ymatrix.zero_row(elim);
                    ymatrix.zero_col(elim);
                    ymatrix.set(elim, elim, c_epsilon);
                }
            }
            k += self.nconds;
        }

        if element_open {
            for (ii, &elim) in row_eliminated.iter().enumerate() {
                if !elim {
                    ymatrix.add(ii, ii, c_epsilon);
                }
            }
        }
    }

    /// The tail of every concrete `CalcYPrim` (Pascal
    /// `TDSSCktElement.CalcYPrim`): apply the open-conductor corrections to
    /// the series, shunt and whole matrices.
    pub fn apply_yprim_open_conductor_calcs(&mut self) {
        if self.all_conductors_closed() {
            return; // fast path; the Kron pass is a no-op anyway
        }
        let mut series = self.yprim_series.take();
        if let Some(m) = series.as_mut() {
            self.do_yprim_calcs(m);
        }
        self.yprim_series = series;
        let mut shunt = self.yprim_shunt.take();
        if let Some(m) = shunt.as_mut() {
            self.do_yprim_calcs(m);
        }
        self.yprim_shunt = shunt;
        let mut whole = self.yprim.take();
        if let Some(m) = whole.as_mut() {
            self.do_yprim_calcs(m);
        }
        self.yprim = whole;
    }

    /// Pascal `TDSSCktElement.MakeLike` (`inherited MakeLike` for elements):
    /// copies the base frequency and re-enables the target.
    pub fn make_like_base(&mut self, other: &CktElementData) {
        self.obj.copy_prp_sequence_from(&other.obj);
        self.base_frequency = other.base_frequency;
        self.enabled = true;
    }

    /// Pascal `TDSSCktElement.MakePosSequence` (`CktElement.pas:1120-1132`): for
    /// each terminal, strip the bus name to its base (`StripExtension`, up to the
    /// first `.`) and, if the ORIGINAL name was a "ground bus" (the local
    /// `IsGroundBus`, `CktElement.pas:1101`), re-append `.0`.
    ///
    /// Pascal writes `FBusNames[i]` **directly** — not through `SetBus` — so this
    /// must NOT raise `signal_bus_name_redefined` (unlike [`Self::set_bus`]). The
    /// stored names are already lowercased, so no re-normalization is needed.
    pub fn make_pos_sequence_base(&mut self) {
        for i in 0..self.nterms {
            let grnd = is_ground_bus(&self.bus_names[i]);
            let stripped = strip_extension(&self.bus_names[i]);
            self.bus_names[i] = if grnd {
                format!("{stripped}.0")
            } else {
                stripped
            };
        }
    }
}

/// Pascal local `IsGroundBus` inside `TDSSCktElement.MakePosSequence`
/// (`CktElement.pas:1101-1118`): a bus name is a "ground bus" iff it contains a
/// `.` but NONE of the substrings `.1`, `.2`, `.3` (searched anywhere with
/// `pos`, not per-node). So `b.4.4` → ground (→ `b.0`), a dotless `busA` is NOT
/// ground (kept as-is), and `b.10`/`b.21`/`b.3x` are NOT ground (the `.1`/`.2`/
/// `.3` substring appears). Transcribed literally from the Pascal short-circuit
/// order — the semantics are exactly "no phase-1/2/3 tag present, but dotted".
fn is_ground_bus(s: &str) -> bool {
    if s.contains(".1") || s.contains(".2") || s.contains(".3") {
        return false;
    }
    s.contains('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pascal `Set_/Get_ConductorClosed(index > 0)` guard `Index <= Fnconds`, not
    /// `Fnphases` (`CktElement.pas`). On a 4-conductor element (3 phases + a
    /// neutral) the `Open`/`Close` exec verbs must be able to open conductor 4 (the
    /// neutral); a `Nphases` guard would silently reject it. Pins the audit fix.
    #[test]
    fn conductor_closed_guard_is_nconds_not_nphases() {
        let mut cd = CktElementData::new("e", 0);
        cd.set_nconds(4); // 3 phases + neutral
        cd.set_nterms(2);
        assert_eq!(cd.nphases, 3);
        assert_eq!(cd.nconds, 4);

        // The neutral conductor (4 > Nphases) is a valid index and starts closed.
        assert!(cd.conductor_closed(1, 4));
        cd.set_conductor_closed(1, 4, false);
        assert!(!cd.conductor_closed(1, 4), "neutral conductor 4 must open");
        assert!(cd.yprim_invalid, "opening a conductor invalidates YPrim");
        // Phases are untouched, and a truly out-of-range conductor (> Nconds) is a
        // no-op read/write, matching Pascal's `Index <= Fnconds` guard.
        assert!(cd.conductor_closed(1, 1));
        assert!(!cd.conductor_closed(1, 5));
        cd.set_conductor_closed(1, 5, false); // ignored — out of range
        assert!(!cd.conductor_closed(1, 5));
    }

    /// Pascal local `IsGroundBus` (`CktElement.pas:1101`) quirk table,
    /// transcribed directly from the short-circuit `pos('.1'/.2/.3', S)` logic.
    #[test]
    fn is_ground_bus_quirk_table() {
        // Dotless names are never ground (no `.`).
        assert!(!is_ground_bus("busa"));
        assert!(!is_ground_bus(""));
        // A `.` with no phase-1/2/3 tag → ground.
        assert!(is_ground_bus("b.0"));
        assert!(is_ground_bus("b.4"));
        assert!(is_ground_bus("b.4.4")); // the documented quirk
        assert!(is_ground_bus("b.5.6.7"));
        // A phase tag anywhere → not ground.
        assert!(!is_ground_bus("b.1"));
        assert!(!is_ground_bus("b.2.3"));
        assert!(!is_ground_bus("b.4.1")); // `.1` present via the second node
        // Substring, not per-node: `.10` contains `.1`, `.21` contains `.2`.
        assert!(!is_ground_bus("b.10"));
        assert!(!is_ground_bus("b.21"));
        assert!(!is_ground_bus("b.30"));
        // `.40` has none of `.1/.2/.3` → ground.
        assert!(is_ground_bus("b.40"));
    }

    /// Pascal `TDSSCktElement.MakePosSequence` base bus rename
    /// (`CktElement.pas:1120`): StripExtension + `.0` re-append for ground buses,
    /// written directly to `bus_names` (no `BusNameRedefined` signal).
    #[test]
    fn make_pos_sequence_base_rename() {
        let mut cd = CktElementData::new("e", 0);
        cd.set_nconds(3);
        cd.set_nterms(4);
        cd.bus_names[0] = "b1.1.2.3".to_string(); // phased → strip to base
        cd.bus_names[1] = "b2.4.4".to_string(); // ground quirk → b2.0
        cd.bus_names[2] = "b3".to_string(); // dotless → unchanged
        cd.bus_names[3] = "b4.0".to_string(); // already ground → b4.0
        cd.signal_bus_name_redefined = false;

        cd.make_pos_sequence_base();

        assert_eq!(cd.bus_names[0], "b1");
        assert_eq!(cd.bus_names[1], "b2.0");
        assert_eq!(cd.bus_names[2], "b3");
        assert_eq!(cd.bus_names[3], "b4.0");
        // Direct FBusNames write: no redefine signal (Pascal never calls SetBus).
        assert!(
            !cd.signal_bus_name_redefined,
            "MakePosSequence writes FBusNames directly, must not signal BusNameRedefined"
        );
    }
}
