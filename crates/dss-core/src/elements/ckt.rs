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
use crate::obj::base::DssObjData;
use crate::support::cmatrix::CMatrix;
use crate::util::EPSILON;

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
                                    let v = yij - (yin * ynj) / ynn;
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
}
