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
use crate::compat;
use crate::elements::traits::ElemId;
use crate::obj::base::DssObjData;
use crate::report::format::strip_extension;
use crate::support::cmatrix::CMatrix;
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
    /// `Flg.ForceYPrim` — use the YPrim supplied by the user (`Set YPrim=…`,
    /// WP-U1.9): `ReCalcAllYPrims`/`BuildYMatrix` skip `CalcYPrim`, and
    /// `SetYPrimInvalid(true)` is suppressed so the forced matrix stays live.
    pub const FORCE_YPRIM: Self = Self(1 << 9);
    /// `Flg.ForceInjCurrents` — use the injection/terminal currents supplied by
    /// the user (`Set InjCurrent=…`/`Set ITerminal=…`, WP-U1.9): the injection
    /// loop injects the stored `InjCurrent` directly and `GetCurrents` returns
    /// the stored `ITerminal` instead of recomputing the PC-element model.
    pub const FORCE_INJ_CURRENTS: Self = Self(1 << 10);

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
    /// Pascal `IterminalSolutionCount`: the solution count for which the cached
    /// `Iterminal` is current. `None` (Pascal's `-1` sentinel) = never computed,
    /// so the lazy cache always recomputes on first use; it doubles as the
    /// thread-readiness flag of the terminal-current cache.
    pub iterminal_solution_count: Option<u32>,
    /// Pascal `ITerminalUpdated` (TPCElement).
    pub iterminal_updated: bool,
    /// `CktElements` handle: `Some(1-based position)` once added to the circuit,
    /// `None` (Pascal's `0`) while not in any circuit.
    pub handle: Option<u32>,
    /// Signal to the executive: a bus name / conductor count / enabled state
    /// changed, so the circuit must set `BusNameRedefined` (which in Pascal
    /// happens immediately through the `ActiveCircuit` global).
    pub signal_bus_name_redefined: bool,
    /// Signal to the executive: reset `Solution.SolutionInitialized` (Pascal
    /// writes it straight through the `ActiveCircuit` global from a property
    /// side effect — e.g. adding a model-3 generator forces a DQDV re-init).
    pub signal_reset_solution_initialized: bool,
    /// Signal to the Y build: the last `calc_yprim` found that the element
    /// cannot be stamped as it is specified (it queued a message naming the
    /// cause), so the build fails and the next solve builds and refuses again.
    /// Read and cleared by `solution::ymatrix::build_y_matrix`.
    pub yprim_refused: bool,
    /// The message of the last recalc that found an input making a quantity
    /// the element derives (a stamp quantity or a derived rating) not a finite
    /// number, `None` once a recalc finds them all finite. While it is set the
    /// element keeps the derived state of its last good recalc and its
    /// `calc_yprim` refuses the build with this message
    /// ([`CktElementData::refuse_recalc`]).
    pub recalc_refusal: Option<String>,

    /// Pascal `Flags` (`TDSSObjectFlags`), element-level subset.
    pub flags: ElemFlags,

    // --- Meter-zone fields (Pascal `TPDElement`, with `meter_obj`/
    // --- `sensor_obj` also on `TPCElement`). Written by the EnergyMeter
    // --- zone build (`MakeMeterZoneLists`); meaningful for PD elements and
    // --- (the refs) for zone PC elements, inert otherwise.
    /// `FromTerminal`: 0-based terminal facing the meter on a radial feeder
    /// (Pascal's 1-based `FromTerminal`, default 1 → `Some(0)`; `None` = unset).
    pub from_terminal: Option<usize>,
    /// `ToTerminal`: the other end, 0-based; `None` (Pascal's `0`) until the
    /// reliability sweep sets it from `from_terminal`.
    pub to_terminal: Option<usize>,
    /// `ParentPDElement`: the upline branch in the meter zone.
    pub parent_pd: Option<ElemId>,
    /// `MeterObj`: upline EnergyMeter.
    pub meter_obj: Option<ElemId>,
    /// `SensorObj`: upline Sensor/meter for allocation and estimation.
    pub sensor_obj: Option<ElemId>,
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
    /// How the present dynamics entry treated this element.
    pub dynamics_seed: DynamicsSeed,
    /// A bus rebuild ran while the element was disabled and gave its buses other
    /// nodes than its node map names, so the map belongs to an earlier bus list.
    pub stale_node_map: bool,
    /// The last power flow holds this element where it is now: enabled, on its
    /// present node map, in its present shape and connection. Every power flow,
    /// the direct solve and the no-load snapshot of `Calcvoltagebases` set it for
    /// each enabled element with a node map. Creating, enabling, re-phasing or
    /// reconnecting the element clears it, and so does a bus rebuild that gives
    /// it another node map. The dynamics entry seeds only such an element.
    pub in_power_flow: bool,
    /// A bus rebuild gave the element another node map than the last power flow
    /// solved it on, and nothing else has taken it out of that power flow since,
    /// so the dynamics refusal names the rebuild as the cause.
    pub renumbered: bool,
    /// The phase count and the connection ordinal the terminals are allocated
    /// for ([`Self::set_shape`]).
    shape: (usize, i32),

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
    /// Registration record: the [`OcpDeviceType`] of the **first enabled**
    /// over-current-protection control that resolved its controlled element to
    /// this one (`exec/command.rs`, `RefAction::SetOcpDevice`, first write
    /// wins — the same write that raises `HAS_OCP_DEVICE`).
    ///
    /// **Not** Pascal's `GetOCPDeviceType` and no longer read by the
    /// reliability sweep (GOLDEN_REBASE G1.3d(ii), adjacent defect A-1): that
    /// scan has no `Enabled` filter and answers from the live
    /// `ControlElementList` order, so the two disagree whenever the element's
    /// first list member is a disabled or later-attached control of another
    /// class. The sweep recomputes it at
    /// `solution::meters::reliability::live_ocp_device_type` (r4133
    /// `Meters/EnergyMeter.pas:2538`), pinned by
    /// `exec::tests::reliability::section_device_type_is_the_live_ocp_scan_not_the_registration_latch`.
    /// Retiring this field is A-1's tail; it is read only by tests today.
    pub ocp_device_type: OcpDeviceType,
}

/// How the present dynamics entry treated an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DynamicsSeed {
    /// Not seeded by the present entry (disabled there, or created or enabled
    /// after it).
    #[default]
    Unseeded,
    /// Enabled at the entry without a model to integrate, so the entry seeded,
    /// with this phase count, only the state a direct solve reads.
    NoModel(usize),
    /// Enabled at the entry but not part of the last solved power flow in its
    /// present shape, so the entry had no operating point to seed it from.
    OutsidePowerFlow,
    /// Seeded by the present entry with this phase count.
    Seeded(usize),
}

/// Pascal `GetOCPDeviceType` (`Utilities.pas:1996-2018`) — which class of
/// over-current-protection control sits at the head of a feeder section.
///
/// Not a `DssEnum`: the code is *derived* by scanning the element's
/// `ControlElementList` for the first `FUSE_CONTROL` / `RECLOSER_CONTROL` /
/// `RELAY_CONTROL` class ordinal. [`Self::Unset`] is `Result := 0`, the
/// pre-scan seed that survives when the list holds no OCP device — the same
/// zero `TFeederSection.OCPDeviceType` starts at (`EnergyMeter.pas:2465`,
/// "1=Fuse; 2=Recloser; 3=Relay"), which is why it is also the `Default`.
///
/// The ordinal is user-visible through `Export Sections`, where
/// `getOCPDeviceTypeString` (`ExportResults.pas:3859-3871`) renders 1/2/3 and
/// maps everything else — i.e. exactly [`Self::Unset`] here — to `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OcpDeviceType {
    /// `0` — no OCP control on this element (`Result := 0`, never overwritten).
    #[default]
    Unset = 0,
    /// `1` — `FUSE_CONTROL`.
    Fuse = 1,
    /// `2` — `RECLOSER_CONTROL`.
    Recloser = 2,
    /// `3` — `RELAY_CONTROL`.
    Relay = 3,
}

impl OcpDeviceType {
    /// The raw `GetOCPDeviceType` integer.
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the raw `GetOCPDeviceType` integer; `None` outside 0..=3.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Unset),
            1 => Some(Self::Fuse),
            2 => Some(Self::Recloser),
            3 => Some(Self::Relay),
            _ => None,
        }
    }
}

/// The first `min(nphases, 3)` conductors of one terminal — the "phase window"
/// the symmetrical-component exports/shows iterate ([`CktElementData::term_phases`]).
/// Yields the terminal current and the node of each phase in conductor order.
pub struct Phases<'a> {
    curr: &'a [Complex64],
    nodes: &'a [usize],
    n: usize,
}

impl Phases<'_> {
    /// The terminal current and the node of each of the first
    /// `min(nphases, 3)` phases, in order, node 0 for a phase without a node.
    pub fn iter(&self) -> impl Iterator<Item = (Complex64, usize)> + '_ {
        self.curr[..self.n]
            .iter()
            .copied()
            .zip((0..self.n).map(|i| self.nodes.get(i).copied().unwrap_or(0)))
    }
}

/// `name=value`, the value as `?` reads a number, for the message of a
/// refused recalc.
pub(crate) fn input_text(name: &str, v: f64) -> String {
    format!("{name}={}", crate::util::float_to_str_ex(v))
}

/// `name=[ a b]`, as `?` reads an array of `scale`, for the message of a
/// refused recalc.
pub(crate) fn array_input_text(name: &str, vals: &[f64], scale: f64) -> String {
    format!(
        "{name}={}",
        crate::util::get_dss_array_f64(vals.len(), Some(vals), scale)
    )
}

/// The input a refused recalc names, from `inputs` in property order (name
/// and value): the first that is not a finite number, else the first of
/// `zeros` that is zero, else `fallback` (the divisor or the multiplied input
/// of an overflow).
pub(crate) fn refused_input_text(
    inputs: &[(&str, f64)],
    zeros: &[(&str, f64)],
    fallback: (&str, f64),
) -> String {
    let (name, v) = inputs
        .iter()
        .find(|(_, v)| !v.is_finite())
        .or_else(|| zeros.iter().find(|(_, v)| *v == 0.0))
        .copied()
        .unwrap_or(fallback);
    input_text(name, v)
}

impl CktElementData {
    /// The stamp of an element the solve refuses with `msg`: an enabled
    /// element logs it and refuses the Y build, a disabled one is out of the
    /// model, and either keeps a zero stamp.
    pub fn refuse_yprim(&mut self, msg: impl Into<crate::diag::DssDiagnostic>) {
        if self.enabled {
            self.obj.push_error(msg);
            self.yprim_refused = true;
        }
        let n = self.yorder;
        self.yprim_series = Some(CMatrix::new(n));
        self.yprim_shunt = Some(CMatrix::new(n));
        self.yprim = Some(CMatrix::new(n));
        self.yprim_invalid = false;
    }

    /// Refuse the stamp while the last recalc is refused, true when it did.
    pub fn refuse_yprim_on_recalc(&mut self) -> bool {
        match self.recalc_refusal.clone() {
            Some(msg) => {
                self.refuse_yprim(msg);
                true
            }
            None => false,
        }
    }

    /// Refuse the element of class `class` whose `input` (`name=value`, the
    /// value as `?` reads it) makes its `quantity` not a finite number: the
    /// message is logged now and kept for the Y build, which it refuses while
    /// the element is enabled. The stamp is marked invalid, so the next solve
    /// builds the Y matrix again, also after an edit of a built element.
    pub fn refuse_recalc(&mut self, class: &str, input: &str, quantity: &str) {
        let msg = self.refusal_message(class, input, quantity);
        self.obj.push_error(msg.clone());
        self.recalc_refusal = Some(msg);
        self.yprim_invalid = true;
    }

    /// The message that refuses the element of class `class` whose `input`
    /// makes its `quantity` not a finite number.
    pub fn refusal_message(&self, class: &str, input: &str, quantity: &str) -> String {
        format!(
            "{class}.{}: {input} makes its {quantity} not a finite number, so it is not built.",
            self.obj.name()
        )
    }

    /// Base construction; concrete classes set phases/conds/terms right after
    /// (the Pascal constructors assign `FNphases`/`Fnconds`/`Nterms`).
    pub fn new(name: &str, num_props: usize) -> Self {
        Self {
            obj: DssObjData::new(name.to_ascii_lowercase(), num_props),
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
            iterminal_solution_count: None,
            iterminal_updated: false,
            handle: None,
            signal_bus_name_redefined: false,
            signal_reset_solution_initialized: false,
            yprim_refused: false,
            recalc_refusal: None,
            flags: ElemFlags::NONE,
            // Pascal `TPDElement.Create`: `FromTerminal := 1` (0-based terminal 0).
            from_terminal: Some(0),
            to_terminal: None,
            parent_pd: None,
            meter_obj: None,
            sensor_obj: None,
            branch_num_customers: 0,
            branch_total_customers: 0,
            overload_een: 0.0,
            overload_ue: 0.0,
            dynamics_seed: DynamicsSeed::Unseeded,
            stale_node_map: false,
            in_power_flow: false,
            renumbered: false,
            shape: (3, 0),
            branch_flt_rate: 0.0,
            accumulated_br_flt_rate: 0.0,
            accumulated_miles_downstream: 0.0,
            branch_section_id: 0,
            ocp_device_type: OcpDeviceType::Unset,
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

        self.nterms = value;
        self.reallocate_terminals();
        self.yorder = self.nterms * self.nconds;
        self.vterminal.resize(self.yorder, Complex64::ZERO);
        self.iterminal.resize(self.yorder, Complex64::ZERO);
        self.complex_buffer.resize(self.yorder, Complex64::ZERO);
    }

    /// Allocate fresh terminals for the present terminal and conductor counts:
    /// no terminal names a bus, so the node map no longer fits
    /// ([`Self::is_mapped`]) until a bus rebuild maps the element again, and the
    /// element is outside the last power flow ([`Self::in_power_flow`]).
    fn reallocate_terminals(&mut self) {
        self.terminals = (0..self.nterms)
            .map(|_| Terminal::init(self.nconds))
            .collect();
        self.terminals_checked = vec![false; self.nterms];
        self.shape.0 = self.nphases;
        self.leave_power_flow();
    }

    /// Take the element out of the last power flow ([`Self::in_power_flow`]) for
    /// a cause other than a bus rebuild.
    fn leave_power_flow(&mut self) {
        self.in_power_flow = false;
        self.renumbered = false;
    }

    /// Sets the conductor count. A change flags the bus names for
    /// reprocessing and lets [`Self::set_nterms`] reallocate the terminals,
    /// since `yorder` still holds the old `nterms * nconds` product. A set
    /// that changes nothing leaves every terminal's `bus_ref`,
    /// `term_node_ref`, `conductors_closed` and `terminals_checked` as they
    /// were and asks for no bus-list rebuild, as the unit pin
    /// `tests::a_no_op_set_nconds_keeps_the_terminal_state` holds.
    pub fn set_nconds(&mut self, value: usize) {
        if value == 0 {
            return; // Pascal records error 749 and exits
        }
        if value != self.nconds {
            self.signal_bus_name_redefined = true;
        }
        self.nconds = value;
        let nterms = self.nterms;
        self.set_nterms(nterms);
    }

    /// Set the conductor count `nconds` that the present phase count and the
    /// connection ordinal `connection` give an element of a class with a `conn`
    /// property. A change of the phase count or the connection re-shapes the
    /// terminals as a change of the conductor count does, also where the count
    /// stays: the node map no longer fits ([`Self::drop_unfit_node_map`]) and the
    /// element leaves the last power flow
    /// (`a_machine_reshaped_on_its_conductors_is_outside_the_run`).
    pub fn set_shape(&mut self, nconds: usize, connection: i32) {
        if nconds == 0 {
            return;
        }
        let reshaped = (self.nphases, connection) != self.shape;
        self.shape.1 = connection;
        if nconds != self.nconds {
            self.set_nconds(nconds);
        } else if reshaped {
            self.reallocate_terminals();
        }
    }

    /// Enable or disable the element. A change forces rebuilding of bus lists
    /// and Y, and leaves the element without a dynamic state of the present run
    /// and outside the last power flow ([`Self::in_power_flow`]). An element
    /// enabled after a bus rebuild that left it out keeps its node map when that
    /// rebuild gave its buses the nodes the map names and the map still has the
    /// element's shape, and drops it otherwise, so until the next rebuild it
    /// reads as one created after the last
    /// (`readers_of_an_element_enabled_after_a_bus_rebuild`,
    /// `readers_of_an_element_reshaped_since_its_map`).
    pub fn set_enabled(&mut self, value: bool) {
        if value == self.enabled {
            return;
        }
        self.enabled = value;
        self.signal_bus_name_redefined = true;
        self.dynamics_seed = DynamicsSeed::Unseeded;
        self.leave_power_flow();
        if value && !self.is_mapped() {
            self.drop_node_map();
        }
    }

    /// Drop the node map: no node for any conductor, no bus for any terminal,
    /// and zero terminal voltages and currents.
    fn drop_node_map(&mut self) {
        self.node_ref.clear();
        for t in &mut self.terminals {
            t.bus_ref = None;
            t.term_node_ref.fill(0);
        }
        self.vterminal.fill(Complex64::ZERO);
        self.iterminal.fill(Complex64::ZERO);
        self.stale_node_map = false;
    }

    /// Drop the node map of an enabled element that no longer has the shape the
    /// map was built for (a `like=`, `phases=` or `conn=` edit since the last
    /// bus rebuild), so until the next rebuild, which the drop requests, it reads
    /// as one created after it.
    pub fn drop_unfit_node_map(&mut self) {
        if self.enabled && !self.node_ref.is_empty() && !self.is_mapped() {
            self.drop_node_map();
            self.signal_bus_name_redefined = true;
        }
    }

    /// Whether the present dynamics entry seeded this element with its present
    /// phase count, with or without a model to integrate.
    pub fn dynamics_seeded(&self) -> bool {
        matches!(
            self.dynamics_seed,
            DynamicsSeed::Seeded(n) | DynamicsSeed::NoModel(n) if n == self.nphases
        )
    }

    /// Name the bus of terminal `i` (1-based). A bus other than the one named
    /// before takes the element out of the last power flow.
    pub fn set_bus(&mut self, i: usize, s: &str) {
        if i >= 1 && i <= self.nterms {
            let bus = s.to_ascii_lowercase();
            if self.bus_names[i - 1] != bus {
                self.leave_power_flow();
            }
            self.bus_names[i - 1] = bus;
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
        // Convert the 1-based terminal boundary value once; the offset math and
        // terminal record below are 0-based.
        let t = iterm - 1;
        self.stale_node_map = false;
        self.node_ref.resize(self.yorder, 0);
        let offset = t * self.nconds;
        self.node_ref[offset..offset + self.nconds].copy_from_slice(&node_ref_array[..self.nconds]);
        self.terminals[t].term_node_ref[..self.nconds]
            .copy_from_slice(&node_ref_array[..self.nconds]);
        self.vterminal.resize(self.yorder, Complex64::ZERO);
        self.iterminal.resize(self.yorder, Complex64::ZERO);
        self.complex_buffer.resize(self.yorder, Complex64::ZERO);
    }

    /// The terminal voltage of each conductor from the node voltages (node 0 is
    /// the always-zero ground slot). An element without a node map
    /// ([`Self::is_mapped`]) reads zero at every conductor.
    pub fn compute_vterminal(&mut self, node_v: &[Complex64]) {
        if !self.is_mapped() {
            self.vterminal.fill(Complex64::ZERO);
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

    // --- Terminal × conductor views over the flat `yorder` buffers (P8) ---
    // `vterminal`/`iterminal`/`inj_current`/`complex_buffer` are flat
    // `nterms * nconds` vectors and `node_ref` is the same shape; all are laid
    // out terminal-major (`t * nconds + c`). These accessors are the ONLY place
    // that offset arithmetic lives — every consumer takes a per-terminal slice.

    /// Conductor slice of terminal `t` (0-based) into `vterminal`.
    pub fn term_v(&self, t: usize) -> &[Complex64] {
        let base = t * self.nconds;
        &self.vterminal[base..base + self.nconds]
    }

    /// Conductor slice of terminal `t` (0-based) into `iterminal`.
    pub fn term_i(&self, t: usize) -> &[Complex64] {
        let base = t * self.nconds;
        &self.iterminal[base..base + self.nconds]
    }

    /// Per-terminal conductor slices of `iterminal`, terminal `0..nterms`.
    pub fn terminals_i(&self) -> impl Iterator<Item = &[Complex64]> {
        self.iterminal.chunks_exact(self.nconds)
    }

    /// Global node-reference slice of terminal `t` (0-based) into `node_ref`,
    /// for an element with a node map ([`Self::is_mapped`]) only.
    pub fn term_nodes(&self, t: usize) -> &[usize] {
        let base = t * self.nconds;
        &self.node_ref[base..base + self.nconds]
    }

    /// Whether the node map belongs to the present bus list and shape: a node for
    /// every conductor, a bus for every terminal, and no bus rebuild since it was
    /// built that gave the element other nodes. An element created after the
    /// last bus rebuild, re-phased or re-connected since, or enabled with its
    /// stale map dropped has none, and every reader reads it as connected to no
    /// node. An element without conductors connects to no node, so it has no map
    /// either, and `Dump` writes its node map as `nil`.
    pub fn is_mapped(&self) -> bool {
        self.yorder > 0
            && !self.stale_node_map
            && self.node_ref.len() == self.yorder
            && self.terminals.iter().all(|t| t.bus_ref.is_some())
    }

    /// The global node of conductor slot `k` (terminal-major), or the ground
    /// node 0 where the element has no node map: a reader sees such a conductor
    /// at zero volts with no node number.
    pub fn node_or_ground(&self, k: usize) -> usize {
        if self.is_mapped() {
            self.node_ref.get(k).copied().unwrap_or(0)
        } else {
            0
        }
    }

    /// The symmetrical-component "phase window" of terminal `t` (0-based): the
    /// first `min(nphases, 3)` conductors' terminal current and global node
    /// reference — the walk the seq-quantity exports/shows share. A conductor
    /// without a node reads at the ground node 0.
    pub fn term_phases(&self, t: usize) -> Phases<'_> {
        let base = t * self.nconds;
        let nodes = if self.is_mapped() {
            self.node_ref.get(base..base + self.nconds).unwrap_or(&[])
        } else {
            &[]
        };
        Phases {
            curr: self.term_i(t),
            nodes,
            n: self.nphases.min(3),
        }
    }

    /// Whether the cached `Iterminal` is current for `solution_count` (Pascal
    /// `IterminalSolutionCount = ActiveCircuit.Solution.SolutionCount`). `None`
    /// (never computed) never matches, so first use always recomputes. The
    /// `solution_count` argument is the solver's non-negative running count.
    pub fn iterminal_solved_for(&self, solution_count: i32) -> bool {
        self.iterminal_solution_count == Some(solution_count as u32)
    }

    /// Mark the cached `Iterminal` current as of `solution_count` (Pascal
    /// `IterminalSolutionCount := ActiveCircuit.Solution.SolutionCount`).
    pub fn mark_iterminal_solved(&mut self, solution_count: i32) {
        self.iterminal_solution_count = Some(solution_count as u32);
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
    pub fn do_yprim_calcs(&self, ymatrix: &mut CMatrix) {
        let yorder = self.yorder;
        let mut element_open = false;
        let mut row_eliminated: Vec<bool> = Vec::new();
        let c_epsilon = Complex64::new(EPSILON, 0.0);

        // Each terminal owns conductors `[base, base + nconds)` of the YPrim; the
        // 0-based `base` replaces the old running `k += nconds` offset.
        for (term_idx, term) in self.terminals.iter().enumerate() {
            let base = term_idx * self.nconds;
            for (j, &closed) in term.conductors_closed.iter().take(self.nconds).enumerate() {
                if closed {
                    continue;
                }
                if !element_open {
                    row_eliminated = vec![false; yorder];
                    element_open = true;
                }
                // Kron reduction of the eliminated row/column `elim`.
                let elim = base + j;
                let mut ynn = ymatrix.get(elim, elim);
                if ynn.norm() == 0.0 {
                    ynn.re = EPSILON;
                }
                row_eliminated[elim] = true;
                // Walk the still-live rows/columns; the upper-triangle sweep
                // (`jj` starting at `ii`) with the symmetric write preserves the
                // Pascal accumulation order exactly.
                for (ii, &ii_gone) in row_eliminated.iter().enumerate() {
                    if ii_gone {
                        continue;
                    }
                    let yin = ymatrix.get(ii, elim);
                    for (jj, &jj_gone) in row_eliminated.iter().enumerate().skip(ii) {
                        if jj_gone {
                            continue;
                        }
                        let yij = ymatrix.get(ii, jj);
                        let ynj = ymatrix.get(elim, jj);
                        // FPC ucomplex `/` (Smith), as Pascal `DoYPrimCalcs`
                        // uses — same cancellation-sensitive Kron term as
                        // `CMatrix::kron`.
                        let v = yij - compat::cdiv(yin * ynj, ynn);
                        ymatrix.set(ii, jj, v);
                        ymatrix.set(jj, ii, v);
                    }
                }
                ymatrix.zero_row(elim);
                ymatrix.zero_col(elim);
                ymatrix.set(elim, elim, c_epsilon);
            }
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

    /// `like=` for every circuit element: copies the property sequence and the
    /// base frequency and enables the target through [`Self::set_enabled`], as
    /// `Enable` does (`like_enables_a_disabled_machine_as_enable_does`).
    pub fn make_like_base(&mut self, other: &CktElementData) {
        self.obj.copy_prp_sequence_from(&other.obj);
        self.base_frequency = other.base_frequency;
        self.set_enabled(true);
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

    /// A `set_nconds` that changes nothing leaves the terminals (bus refs,
    /// node refs, open conductors, checked flags) exactly as they were and
    /// asks for no bus-list rebuild, while a real change reallocates them.
    /// This pin holds the no-op path.
    #[test]
    fn a_no_op_set_nconds_keeps_the_terminal_state() {
        let mut cd = CktElementData::new("e", 0);
        cd.set_nconds(3);
        cd.set_nterms(2);
        // The state `ReProcessBusDefs` / the solve fill in, plus an open
        // conductor and a walk mark.
        cd.terminals[0].bus_ref = Some(7);
        cd.terminals[1].bus_ref = Some(9);
        cd.terminals[0].term_node_ref = vec![1, 2, 3];
        cd.set_conductor_closed(1, 2, false);
        cd.terminals_checked[0] = true;
        cd.signal_bus_name_redefined = false;

        // A set that changes nothing: no reallocation, no signal.
        cd.set_nconds(3);
        assert_eq!(cd.terminals[0].bus_ref, Some(7), "bus ref survives a no-op");
        assert_eq!(cd.terminals[1].bus_ref, Some(9));
        assert_eq!(cd.terminals[0].term_node_ref, vec![1, 2, 3]);
        assert!(!cd.conductor_closed(1, 2), "an open conductor stays open");
        assert!(cd.terminals_checked[0]);
        assert!(
            !cd.signal_bus_name_redefined,
            "nothing changed, so nothing may ask for a bus-list rebuild"
        );

        // A real change reallocates (Pascal's `Value * Fnconds <> Yorder` arm)
        // and asks for the rebuild that re-resolves the fresh terminals.
        cd.set_nconds(1);
        assert_eq!(cd.nconds, 1);
        assert_eq!(cd.nterms, 2, "the terminal count itself is unchanged");
        assert_eq!(cd.yorder, 2);
        assert_eq!(cd.terminals[0].bus_ref, None, "a fresh terminal is unwired");
        assert!(cd.conductor_closed(1, 1), "a fresh terminal is closed");
        assert!(!cd.terminals_checked[0]);
        assert!(cd.signal_bus_name_redefined);
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

    /// `GetOCPDeviceType` (`Utilities.pas:1996-2018`): `Result := 0` pre-scan,
    /// then 1/2/3 for the first Fuse/Recloser/Relay in `ControlElementList`.
    /// `Unset` must also be the `Default`, because `TFeederSection` is
    /// zero-allocated (`EnergyMeter.pas:2465`) and `CktElementData::new` seeds
    /// the same 0.
    #[test]
    fn ocp_device_type_pins_pascal_ordinals() {
        assert_eq!(OcpDeviceType::Unset.ordinal(), 0);
        assert_eq!(OcpDeviceType::Fuse.ordinal(), 1);
        assert_eq!(OcpDeviceType::Recloser.ordinal(), 2);
        assert_eq!(OcpDeviceType::Relay.ordinal(), 3);
        for d in [
            OcpDeviceType::Unset,
            OcpDeviceType::Fuse,
            OcpDeviceType::Recloser,
            OcpDeviceType::Relay,
        ] {
            assert_eq!(OcpDeviceType::from_ordinal(d.ordinal()), Some(d));
        }
        for v in [i32::MIN, -1, 4, 100, i32::MAX] {
            assert_eq!(OcpDeviceType::from_ordinal(v), None, "ordinal {v}");
        }
        assert_eq!(OcpDeviceType::default(), OcpDeviceType::Unset);
        assert_eq!(
            CktElementData::new("e", 0).ocp_device_type,
            OcpDeviceType::Unset
        );
    }
}
