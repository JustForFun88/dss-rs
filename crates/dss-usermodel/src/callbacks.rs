//! The host-services surface: the Pascal 32-slot `TDSSCallBacks` vtable
//! (`Common/DSSCallBackRoutines.pas:19-67`) re-expressed as the [`Callbacks`]
//! trait (tier A pure reads + tier B effect seeds), the tier-B [`Effect`]
//! queue, and the per-instance [`CallData`] store state (context snapshot +
//! owned AuxParser + effect queue — plan §2.3, ABI doc §4).
//!
//! wasmi host functions only see the `Store` data, never the live engine — so
//! dss-core implements [`Callbacks`] on an owned per-call snapshot it builds
//! at each call site from the same disjoint-borrow view the element itself
//! uses, and applies the drained [`Effect`]s immediately after the wasm call
//! returns (order preserved).
//!
//! Default method bodies mirror the Pascal nil-`ActiveCktElement` /
//! nil-`ActiveCircuit` fallbacks of each callback routine (cited per method),
//! so a bare snapshot behaves like the upstream callbacks with nothing active.
//! `Option` returns model the Pascal routines that `Exit` **without touching**
//! the caller's out-parameters: `None` ⇒ the import leaves guest memory
//! unchanged.

use num_complex::Complex64;

use crate::records::DynamicsRec;

/// Tier-A read surface served to guest `dss_env` imports from the pre-call
/// context snapshot (ABI doc §4). "Active element" = the element owning the
/// model instance; the upstream global-`DSSPrime` binding
/// (`DSSCallBackRoutines.pas:493` "this is bad") is deliberately NOT
/// reproduced (ABI doc §4, documented decision).
///
/// `Send` keeps `wasmi::Store<CallData>` `Send` for the MULTITHREADING M3
/// disjoint-`&mut` pattern (plan §2.7).
pub trait Callbacks: Send {
    /// Pascal `GetActiveElementNameCallBack` (`:423-437`): the element's
    /// `FullName`. `None` ⇒ no active element (import returns 0, buffer
    /// untouched).
    fn active_element_name(&self) -> Option<&str> {
        None
    }

    /// Pascal `GetActiveElementIndexCallBack` (`:315-325`): `ClassIndex`,
    /// 0 when nothing is active.
    fn active_element_index(&self) -> i32 {
        0
    }

    /// Pascal `GetActiveElementBusNamesCallBack` (`:157-190`): first two bus
    /// names, empty unless the bus exists **and** its coordinate is defined
    /// (the coord-defined rule is the snapshot producer's job). Always
    /// written (Pascal initializes both to null strings).
    fn active_element_bus_names(&self) -> (&str, &str) {
        ("", "")
    }

    /// Pascal `GetActiveElementVoltagesCallBack` (`:193-207`): terminal
    /// voltages `NodeV[NodeRef[i]]`, `Yorder` entries. `None` ⇒ no active
    /// element (out-params untouched).
    fn active_element_voltages(&self) -> Option<&[Complex64]> {
        None
    }

    /// Pascal `GetActiveElementCurrentsCallBack` (`:210-222`): `Iterminal`
    /// after `ComputeIterminal`, `Yorder` entries. `None` ⇒ untouched.
    fn active_element_currents(&self) -> Option<&[Complex64]> {
        None
    }

    /// Pascal `GetActiveElementLossesCallBack` (`:225-234`): (total, load,
    /// no-load) losses; zeros when nothing is active (Pascal zeroes first).
    fn active_element_losses(&self) -> [Complex64; 3] {
        [Complex64::ZERO; 3]
    }

    /// Pascal `GetActiveElementPowerCallBack` (`:237-244`): total power into
    /// the given terminal (1-based); zero when nothing is active.
    fn active_element_power(&self, _terminal: i32) -> Complex64 {
        Complex64::ZERO
    }

    /// Pascal `GetActiveElementNumCustCallBack` (`:247-264`): (branch, total)
    /// customer counts, zeros unless the element is a PDElement.
    fn active_element_num_cust(&self) -> (i32, i32) {
        (0, 0)
    }

    /// Pascal `GetActiveElementNodeRefCallBack` (`:267-278`): the `NodeRef`
    /// array, `Yorder` entries. `None` ⇒ untouched.
    fn active_element_node_refs(&self) -> Option<&[i32]> {
        None
    }

    /// Pascal `GetActiveElementBusRefCallBack` (`:281-290`):
    /// `Terminals[terminal-1].BusRef`, 0 when nothing is active.
    fn active_element_bus_ref(&self, _terminal: i32) -> i32 {
        0
    }

    /// Pascal `GetActiveElementTerminalInfoCallBack` (`:293-304`):
    /// (Nterms, Nconds, Nphases). `None` ⇒ untouched.
    fn active_element_terminal_info(&self) -> Option<(i32, i32, i32)> {
        None
    }

    /// Pascal `IsActiveElementEnabledCallBack` (`:328-338`).
    fn is_active_element_enabled(&self) -> bool {
        false
    }

    /// Pascal `GetPtrToSystemVarrayCallBack` (`:307-311`): the system node
    /// voltage array `NodeV[1..NumNodes]` (ground `NodeV[0]` excluded; the
    /// wasm import copies instead of aliasing — ABI doc §4 row 17).
    fn node_voltages(&self) -> &[Complex64] {
        &[]
    }

    /// Pascal `IsBusCoordinateDefinedCallback` (`:341-346`).
    fn is_bus_coordinate_defined(&self, _bus_ref: i32) -> bool {
        false
    }

    /// Pascal `GetBusCoordinateCallback` (`:348-357`): (X, Y), zeros when
    /// undefined (Pascal zeroes first, always writes).
    fn bus_coordinate(&self, _bus_ref: i32) -> (f64, f64) {
        (0.0, 0.0)
    }

    /// Pascal `GetBuskVBaseCallback` (`:359-366`).
    fn bus_kv_base(&self, _bus_ref: i32) -> f64 {
        0.0
    }

    /// Pascal `GetBusDistFromMeterCallback` (`:368-375`).
    fn bus_dist_from_meter(&self, _bus_ref: i32) -> f64 {
        0.0
    }

    /// Pascal `GetDynamicsStructCallBack` (`:377-383`): the 52-byte
    /// `TDynamicsRec` image (copied, not aliased — ABI doc §4 row 24).
    /// `None` ⇒ untouched (Pascal leaves the pointer unset with no circuit).
    fn dynamics_rec(&self) -> Option<DynamicsRec> {
        None
    }

    /// Pascal `GetStepSizeCallBack` (`:385-392`): `DynaVars.h`.
    fn step_size(&self) -> f64 {
        0.0
    }

    /// Pascal `GetTimeSecCallBack` (`:394-400`): `DynaVars.t`.
    fn time_sec(&self) -> f64 {
        0.0
    }

    /// Pascal `GetTimeHrCallBack` (`:402-408`): `DynaVars.dblHour`.
    fn time_hr(&self) -> f64 {
        0.0
    }

    /// Pascal `GetPublicDataPtrCallBack` (`:411-421`): the element's
    /// `PublicDataStruct` image (the packed `TGeneratorVars` bytes for a
    /// Generator — ABI doc §2.2/§4 row 28), empty when none.
    fn public_data(&self) -> &[u8] {
        &[]
    }

    /// Seed for the provisional handles returned by `control_queue_push`
    /// (tier B): the handle the engine's control queue will assign to the
    /// **next** pushed action. Queued pushes are drained in order right after
    /// the wasm call returns, so `seed`, `seed+1`, … reproduce the exact
    /// Pascal `ControlQueue.Push` return values
    /// (`DSSCallBackRoutines.pas:444-447`).
    fn control_queue_next_handle(&self) -> i32 {
        0
    }
}

/// A context snapshot with nothing active — every method keeps its default
/// (the Pascal nil-`ActiveCircuit` behavior). Initial store state and the
/// natural choice for calls that must not observe engine state.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCallbacks;

impl Callbacks for NoCallbacks {}

/// A tier-B mutation recorded during a guest call, drained by the engine
/// immediately after the call returns, in order (plan §2.3 tier B).
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Pascal `MsgCallBack` → `DoSimpleMsgCallback`
    /// (`DSSCallBackRoutines.pas:95-99`): route into the engine's
    /// DoSimpleMsg sink (upstream uses error number 9000).
    Msg(String),
    /// Pascal `ControlQueuePush` (`:444-447`): push onto the circuit's
    /// control queue. `handle` is the provisional handle already returned to
    /// the guest (see [`Callbacks::control_queue_next_handle`]); the Pascal
    /// `Owner: Pointer` becomes the owning element's registry handle, managed
    /// host-side by the engine (ABI doc §4 row 31).
    ControlQueuePush {
        /// Schedule hour.
        hour: i32,
        /// Schedule seconds.
        sec: f64,
        /// Action code.
        code: i32,
        /// Proxy handle.
        proxy_hdl: i32,
        /// The provisional queue handle returned to the guest.
        handle: i32,
    },
}

/// A typed host-side fault recorded by a `dss_env` import before it traps,
/// so the call wrapper can surface the precise [`crate::UserModelError`]
/// instead of a generic trap (the typst `memory_error` take-pattern,
/// `plugin.rs:494-501/:564-574`).
#[derive(Debug, Clone)]
pub(crate) enum Fault {
    /// An unsupported-by-design import was called (ABI doc §4 rows 7/30/32).
    Unsupported {
        /// The `dss_env` import name.
        import: &'static str,
    },
    /// An import hit out-of-bounds guest memory.
    OutOfBounds {
        /// The `dss_env` import name.
        import: &'static str,
        /// What access failed.
        detail: String,
    },
}

/// Per-instance store state: the pre-call context snapshot, the owned
/// AuxParser (tier C — Pascal `CallBackParser`, a plain `TDSSParser`
/// instance, `DSSCallBackRoutines.pas:90/:493`), the tier-B effect queue and
/// the host-fault slot (plan §2.3).
pub(crate) struct CallData {
    /// The tier-A context snapshot for the current call.
    pub(crate) ctx: Box<dyn Callbacks>,
    /// The owned AuxParser (Pascal `CallBackParser: TDSSParser`).
    pub(crate) parser: dss_parser::Parser,
    /// The parser's (empty) `@variable` table — the callback parser has no
    /// engine variable context.
    pub(crate) parser_vars: dss_parser::ParserVars,
    /// Pascal `CB_Param` (`DSSCallBackRoutines.pas:92`): the value string of
    /// the last `NextParam`, served by `GetStrValue`.
    pub(crate) last_param_value: String,
    /// Tier-B effects queued during the current call, in order.
    pub(crate) effects: Vec<Effect>,
    /// Typed fault recorded by an import before trapping.
    pub(crate) fault: Option<Fault>,
    /// Store resource limits (linear-memory cap; `trap_on_grow_failure` so a
    /// breach is a deterministic loud trap, classified as
    /// [`crate::UserModelError::MemoryCapExceeded`]).
    pub(crate) limits: wasmi::StoreLimits,
}

impl CallData {
    /// Fresh store state with the given memory cap and an inactive context.
    pub(crate) fn new(memory_cap_bytes: usize) -> Self {
        Self {
            ctx: Box::new(NoCallbacks),
            parser: dss_parser::Parser::new(),
            parser_vars: dss_parser::ParserVars::new(),
            last_param_value: String::new(),
            effects: Vec::new(),
            fault: None,
            limits: wasmi::StoreLimitsBuilder::new()
                .memory_size(memory_cap_bytes)
                .trap_on_grow_failure(true)
                .build(),
        }
    }
}
