//! Port of `Terminal.pas` — the TPowerTerminal object.
//!
//! Pascal `TPowerTerminal` is a value-type `object` (not `class`). In Rust
//! it's a plain struct. Each circuit element has one per terminal.

/// A power terminal belonging to a circuit element.
///
/// Pascal: `TPowerTerminal` (value object)
#[derive(Debug, Clone)]
pub struct Terminal {
    /// Index into the circuit's `Buses` list, or `usize::MAX` if unset.
    pub bus_ref: usize,
    /// Per-conductor global node references (1-based, 0 = ground).
    pub term_node_ref: Vec<usize>,
    /// Per-conductor closed/open state.
    pub conductors_closed: Vec<bool>,
    /// Active conductor (1-based).
    active_conductor: usize,
}

impl Terminal {
    /// Create a terminal with `ncond` conductors, all closed, no bus assigned.
    /// Pascal: `TPowerTerminal.Init`
    pub fn init(ncond: usize) -> Self {
        Self {
            bus_ref: usize::MAX,
            term_node_ref: vec![0; ncond],
            conductors_closed: vec![true; ncond],
            active_conductor: 1,
        }
    }

    /// Set the bus reference (circuit Buses index).
    pub fn set_bus(&mut self, bus_idx: usize) {
        self.bus_ref = bus_idx;
    }

    /// Set the active conductor (1-based, bounds-checked).
    /// Pascal: `TPowerTerminal.Set_ActiveConductor`
    pub fn set_active_conductor(&mut self, value: usize) {
        if value >= 1 && value <= self.term_node_ref.len() {
            self.active_conductor = value;
        }
    }

    /// Get the active conductor (1-based).
    pub fn conductor(&self) -> usize {
        self.active_conductor
    }

    /// Number of conductors on this terminal.
    pub fn num_conductors(&self) -> usize {
        self.term_node_ref.len()
    }
}
