//! MakePosSequence context / plan types — the return-value protocol for the
//! [`CktElement::make_pos_sequence`] override (Pascal per-class
//! `MakePosSequence`, dispatched by `TExecHelper.DoMakePosSeq`).
//!
//! Each element converts its *own* direct fields inside the method (bus/phase
//! resyncs, `PrpSequence` clears, buffer reallocs) and returns a
//! [`PosSeqPlan`]: the ordered property-system mutations the exec applier must
//! replay through the typed setter helpers (Pascal `SetDouble`/`SetInteger`/
//! `SetDoubles`/`SetIntegers`/`SetStrings`, which auto-wrap `BeginEdit`/
//! `EndEdit` when not already editing). The applier owns the editing-active VM,
//! the base bus rename (when `run_base`), and the resolution of the
//! monitored/controlled element info a control/meter reads while converting.
//!
//! [`CktElement::make_pos_sequence`]: crate::elements::traits::CktElement::make_pos_sequence

/// A snapshot of a monitored or controlled element, resolved by the exec
/// applier and handed to a control/meter's [`make_pos_sequence`]. Pascal reads
/// the live `MonitoredElement`/`ControlledElement` fields (`NPhases`, `Yorder`,
/// `NConds`, `BusNames[1]`, `NumStateVars`, `Enabled`) mid-conversion; the
/// borrow checker forbids that here, so the applier copies them in up front.
///
/// [`make_pos_sequence`]: crate::elements::traits::CktElement::make_pos_sequence
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PosSeqElemInfo {
    /// `BusNames[1..NTerms]`, lowercased (1-based terminal → 0-based slot).
    pub bus_names: Vec<String>,
    /// `NPhases`.
    pub nphases: usize,
    /// `NConds`.
    pub nconds: usize,
    /// `Yorder`.
    pub yorder: usize,
    /// `NumStateVars` (`NumVariables`).
    pub num_variables: usize,
    /// `Enabled`.
    pub enabled: bool,
}

/// The read-only context passed into every [`make_pos_sequence`] call.
///
/// [`make_pos_sequence`]: crate::elements::traits::CktElement::make_pos_sequence
#[derive(Debug, Clone, Default)]
pub struct PosSeqCtx {
    /// This element's own per-terminal parsed node numbers, as the AuxParser
    /// splits each `GetBus(i)` (Pascal `AuxParser.ParseAsBusName`). Slot `i`
    /// (0-based) holds terminal `i+1`'s node list (`bus.1.2.3` → `[1, 2, 3]`;
    /// a bare `bus` → `[]`). The Transformer/AutoTrans OnPhase1 disable test
    /// reads `terminal_nodes[w][0]` to decide whether a 1/2-phase winding
    /// sits on phase 1.
    pub terminal_nodes: Vec<Vec<i32>>,
    /// The monitored element's snapshot, for controls/meters that read it
    /// (`None` for elements that monitor nothing, or when unresolved).
    pub monitored: Option<PosSeqElemInfo>,
    /// The controlled element's snapshot, for controls that read it.
    pub controlled: Option<PosSeqElemInfo>,
}

/// One property-system mutation a [`make_pos_sequence`] override requests of
/// the exec applier. `BeginEdit`/`EndEdit` bracket a multi-set block (a bare
/// `Set*` with no surrounding bracket is a single edit the applier wraps
/// itself); the `Set*` variants are the Pascal typed setters; `Disable` maps
/// the winding-not-on-phase-1 path (`Enabled := FALSE`).
///
/// [`make_pos_sequence`]: crate::elements::traits::CktElement::make_pos_sequence
#[derive(Debug, Clone, PartialEq)]
pub enum PosSeqAction {
    /// Pascal `BeginEdit(True)` — open an explicit multi-set edit.
    BeginEdit,
    /// Pascal `EndEdit(1)` — close it (runs the per-class recalc). Storage's
    /// trailing bare `EndEdit` with no matching `BeginEdit` forces one extra
    /// recalc; the applier reproduces that.
    EndEdit,
    /// Pascal `SetDouble(prop_idx, value)`.
    SetF64(usize, f64),
    /// Pascal `SetInteger(prop_idx, value)`.
    SetI32(usize, i32),
    /// Pascal `SetDoubles(prop_idx, values)` onto a struct-array property
    /// (per-winding `kVs`/`kVAs`): `None` keeps the prior entry.
    SetStructF64s(usize, Vec<Option<f64>>),
    /// Pascal `SetIntegers(prop_idx, ordinals)` onto a struct-array enum
    /// property (per-winding `conns`).
    SetStructI32s(usize, Vec<i32>),
    /// Pascal `SetStrings(busesPropIdx, names)` onto the `buses` struct array.
    SetStructBuses(Vec<String>),
    /// Pascal `Enabled := FALSE` (the 1/2-phase-winding-off-phase-1 path).
    Disable,
}

/// The plan a [`make_pos_sequence`] override returns: the ordered property-set
/// actions plus whether the base bus rename ([`make_pos_sequence_base`]) still
/// runs afterwards. The default ([`PosSeqPlan::default`]) is "no actions, run
/// the base rename" — the behavior of every element without an override
/// (Pascal `inherited MakePosSequence`).
///
/// [`make_pos_sequence`]: crate::elements::traits::CktElement::make_pos_sequence
/// [`make_pos_sequence_base`]: crate::elements::ckt::CktElementData::make_pos_sequence_base
#[derive(Debug, Clone)]
pub struct PosSeqPlan {
    pub actions: Vec<PosSeqAction>,
    /// Whether the applier runs the base bus rename after the actions. `true`
    /// for the base behavior and every override that ends with `inherited
    /// MakePosSequence`; `false` for the empty overrides (UPFC/IndMach012) that
    /// have no `inherited` call.
    pub run_base: bool,
}

impl Default for PosSeqPlan {
    fn default() -> Self {
        Self {
            actions: Vec::new(),
            run_base: true,
        }
    }
}

impl PosSeqPlan {
    /// A plan that only runs the base rename (the trait default).
    pub fn base() -> Self {
        Self::default()
    }

    /// A plan with the given actions, still running the base rename after
    /// (Pascal override ending in `inherited MakePosSequence`).
    pub fn with_actions(actions: Vec<PosSeqAction>) -> Self {
        Self {
            actions,
            run_base: true,
        }
    }

    /// A plan that runs no base rename (the empty UPFC/IndMach012 overrides).
    pub fn no_base() -> Self {
        Self {
            actions: Vec::new(),
            run_base: false,
        }
    }
}
