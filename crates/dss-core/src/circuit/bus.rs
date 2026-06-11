//! Port of `Common/Bus.pas` — `TDSSBus`: per-bus node bookkeeping and the
//! global node-reference assignment. The `Add` logic that increments the
//! circuit's node counter lives in [`crate::circuit::Circuit::add_bus`]
//! (Pascal passed the circuit into `TDSSBus.Add`; here the circuit drives).

use num_complex::Complex64;

/// A single bus. Local node indices are 0-based; node *numbers* (the
/// user-facing `.1.2.3` designations) and global node references keep the
/// Pascal 1-based/ground-0 conventions.
#[derive(Debug, Clone)]
pub struct Bus {
    /// Lowercased bus name (`TNamedObject` reuse).
    pub name: String,
    /// User node numbers on this bus (`Nodes`).
    pub nodes: Vec<i32>,
    /// Global node reference per local node (`RefNo`).
    pub ref_no: Vec<usize>,
    /// Node voltages saved per bus (`VBus`), allocated by `AllocateBusState`.
    pub vbus: Vec<Complex64>,
    /// `BusCurrent`.
    pub bus_current: Vec<Complex64>,
    /// Base kV line-to-ground (`kVBase`); 0.0 = not set.
    pub kv_base: f64,
    pub x: f64,
    pub y: f64,
    pub coord_defined: bool,
    pub bus_checked: bool,
    pub keep: bool,
    pub dist_from_meter: f64,
}

impl Bus {
    /// Pascal `TDSSBus.Create`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into().to_lowercase(),
            nodes: Vec::new(),
            ref_no: Vec::new(),
            vbus: Vec::new(),
            bus_current: Vec::new(),
            kv_base: 0.0,
            x: 0.0,
            y: 0.0,
            coord_defined: false,
            bus_checked: false,
            keep: false,
            dist_from_meter: 0.0,
        }
    }

    pub fn num_nodes_this_bus(&self) -> usize {
        self.nodes.len()
    }

    /// Pascal `Find`: global reference for `node_num`, 0 when absent.
    pub fn find(&self, node_num: i32) -> usize {
        self.nodes
            .iter()
            .position(|&n| n == node_num)
            .map_or(0, |i| self.ref_no[i])
    }

    /// Pascal `FindIdx` (0-based here, `None` = not found).
    pub fn find_idx(&self, node_num: i32) -> Option<usize> {
        self.nodes.iter().position(|&n| n == node_num)
    }

    /// Pascal `GetRef` (0-based index in; 0 when out of range).
    pub fn get_ref(&self, node_index: usize) -> usize {
        self.ref_no.get(node_index).copied().unwrap_or(0)
    }

    /// Pascal `GetNum`.
    pub fn get_num(&self, node_index: usize) -> i32 {
        self.nodes.get(node_index).copied().unwrap_or(0)
    }

    /// Pascal `AllocateBusState`.
    pub fn allocate_bus_state(&mut self) {
        self.vbus = vec![Complex64::ZERO; self.nodes.len()];
        self.bus_current = vec![Complex64::ZERO; self.nodes.len()];
    }
}
