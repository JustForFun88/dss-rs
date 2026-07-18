//! Port of `Controls/ESPVLControl.pas` — `TESPVLControlObj`, an Energy-Storage/PV
//! local controller. A control element connected to one terminal of a monitored
//! circuit element; it is either a **System Controller** (`Type=SystemController`,
//! `Ftype=1`) that supervises a set of subordinate **Local Controllers** (other
//! ESPVLControl objects), or a **Local Controller** (`Type=LocalController`,
//! `Ftype=2`) that nominally supervises PVSystem/Storage elements.
//!
//! Like every `TControlElem`, an ESPVLControl builds **no Yprim**, its terminal
//! currents are zero, and its single terminal attaches to the monitored element's
//! terminal bus. `DoPendingAction` and `Reset` are no-ops upstream.
//!
//! **What `Sample` actually does (proven against the pinned oracle).** Only a
//! System Controller acts: it lazily builds `FLocalControlPointerList` — the
//! **named** subordinate controllers (`LocalControlList`), or, when no list is
//! given, **every** enabled ESPVLControl in the class (uniform weights). It then
//! reads the monitored terminal power, forms `PDiff = P_kW - FkWLimit`, and if
//! `|PDiff| > HalfkWBand` it "redispatches" each list entry. **The decisive
//! quirk:** the pointer list holds `TESPVLControlObj` objects, but `Sample`
//! type-confuses each as a `TGeneratorObj` and writes `Gen.kWBase` — a write onto
//! non-electrical memory of *another ESPVLControl*. Modeled here as the phantom
//! [`phantom_kw_base`](EspvlControl::phantom_kw_base) field.
//!
//! This makes ESPVLControl a **faithful no-op on all observable circuit state**:
//! oracle-probed, the generators are never touched (the named PVSystem/Storage
//! lists are dead — `Sample` never reads them), the control never pushes a
//! control-queue action (so `ControlIterations` stays 1), the solution with the
//! control present is **byte-identical** to the solution without it, and the
//! control's own properties round-trip unchanged (no observable corruption). The
//! `tools/golden/gen_props.py` ESPVLControl case + `exec/tests/espvl_control.rs`
//! pin this against dss-python 0.15.7.
//!
//! **`FkWLimit` is unsettable.** There is **no `kWLimit` property** (confirmed by
//! enumerating `AllPropertyNames` in the oracle): the field is hardcoded to
//! `8000.0` in the constructor and never moves, so `PDiff` is always `P_kW-8000`.
//!
//! **Deliberate, oracle-proven divergences from the literal Pascal (all
//! unobservable):**
//! - The type-confused `Gen.kWBase` aliases a `TGeneratorObj` field over a
//!   `TESPVLControlObj`'s memory — impossible in safe Rust and provably
//!   unobservable, so it is modeled as the [`phantom_kw_base`] field. Its first
//!   read is undefined garbage in Pascal; we read a defined `0.0`. Neither the
//!   value nor the write reaches any getter, the power flow, or `Y`.
//! - When a named subordinate fails to resolve, or a (mis-configured) Local
//!   Controller carries a `LocalControlList`, Pascal's `Sample` loop
//!   (`for i := 1 to FLocalControlListSize`) walks past the resolved entries into
//!   a NIL deref. This port iterates the resolved subset instead (same divergence
//!   GenDispatcher documents), so it cannot crash; `FListSize`/`TotalWeight` still
//!   reflect the full list.
//! - `MakePosSequence` is ported as a NIL-deref-safe no-op (the upstream body
//!   dereferences the always-NIL `ControlledElement` — Access violation,
//!   `docs/wpg21_makeposseq_probes.md`; CLAUDE.md forbids reproducing it).
//!
//! The ESPVLControl "fleet" is the ESPVLControl class itself, reached through the
//! class registry behind [`EspvlDispatchEnv`] (`solution/controls/dispatch.rs`),
//! exactly like [`GenDispatcher`]'s generator fleet; the unit tests drive a mock.
//!
//! [`GenDispatcher`]: crate::elements::control::gen_dispatcher::GenDispatcher

#[cfg(test)]
mod tests;

mod accessors;

use num_complex::Complex64;

use crate::elements::control::control_elem::{ControlElemData, RefSnapshot};
use crate::elements::traits::ElemRef;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// 1-based property ordinals (Pascal `TESPVLControlProp` + the `TCktElementClass`
/// tail).
pub mod prop {
    pub const ELEMENT: usize = 1;
    pub const TERMINAL: usize = 2;
    pub const TYP: usize = 3;
    pub const KW_BAND: usize = 4;
    pub const KVAR_LIMIT: usize = 5;
    pub const LOCAL_CONTROL_LIST: usize = 6;
    pub const LOCAL_CONTROL_WEIGHTS: usize = 7;
    pub const PV_SYSTEM_LIST: usize = 8;
    pub const PV_SYSTEM_WEIGHTS: usize = 9;
    pub const STORAGE_LIST: usize = 10;
    pub const STORAGE_WEIGHTS: usize = 11;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 12;
    pub const ENABLED: usize = 13;
    pub const NUM_PROPS: usize = 14; // incl. Like
}

/// `TESPVLControl.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        // Pascal WriteByFunction(SetMonitoredElement) + Required, resolves against
        // any circuit class. The `Required` flag is inert in the port (same as the
        // other controls); a missing Element surfaces via RecalcElementData 372.
        PropDef::object_ref_any("Element").flags(PropFlags::REQUIRED),
        PropDef::integer("Terminal"),
        PropDef::mapped_string_enum("Type", enums.espvl_control_type),
        PropDef::double("kWBand"),
        PropDef::double("kvarLimit"),
        // The three subordinate lists + their IndirectCount weight arrays. Only
        // `LocalControlList` is read by `Sample`; PVSystem/Storage are dead (they
        // round-trip but are never dispatched — faithful to Pascal).
        PropDef::string_list("LocalControlList"),
        // Pascal `DoubleArrayProperty` + `IndirectCount` over the corresponding
        // name-list (`ESPVLControl.pas:203-219`, `PropertyOffset2 = @F*ListSize`,
        // `PropertyOffset3 = @F*NameList`): renders `ArrayOrFilePath` +
        // `$dssLength: <List>`. The element count comes from the list size, exposed
        // via `get_i32(<List>)`.
        PropDef::double_array("LocalControlWeights", prop::LOCAL_CONTROL_LIST),
        PropDef::string_list("PVSystemList"),
        PropDef::double_array("PVSystemWeights", prop::PV_SYSTEM_LIST),
        PropDef::string_list("StorageList"),
        PropDef::double_array("StorageWeights", prop::STORAGE_LIST),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("ESPVLControl", defs, true)
}

/// The executive surface `Sample` needs to reach the monitored element and the
/// ESPVLControl "fleet" (the Rust stand-in for Pascal's live `ParentClass`
/// pointers). The list entries are *other ESPVLControl objects*, not generators —
/// see the module note; `local_kw_base`/`set_local_kw_base` are the type-confused
/// `Gen.kWBase` read/write onto their phantom field.
pub(crate) trait EspvlDispatchEnv {
    /// Pascal `MonitoredElement.Power[ElementTerminal]` (complex VA: W + jVAr).
    /// Returns zero when no monitored element is set (a safe stand-in for Pascal's
    /// NIL deref; only reached by a misconfigured System Controller).
    fn monitored_power(&mut self) -> Complex64;
    /// Pascal `ParentClass.Find(name)` restricted to *enabled* ESPVLControls.
    fn find_enabled_espvl(&self, name: &str) -> Option<ElemRef>;
    /// Pascal's "scan the whole ESPVLControl class for enabled controls", creation
    /// order (includes the System Controller itself, exactly like upstream).
    fn all_enabled_espvls(&self) -> Vec<ElemRef>;
    /// The type-confused `Gen.kWBase` read of a list entry (an ESPVLControl, not a
    /// Generator): its [`phantom_kw_base`](EspvlControl::phantom_kw_base).
    fn local_kw_base(&self, r: ElemRef) -> f64;
    fn set_local_kw_base(&mut self, r: ElemRef, value: f64);
}

/// `TESPVLControlObj`.
#[derive(Debug, Clone)]
pub struct EspvlControl {
    pub ccd: ControlElemData,
    /// Dump name of the monitored element (Pascal renders `FullName`).
    monitored_full_name: String,
    /// Parse-time shape snapshot of the monitored reference.
    mon_snap: Option<RefSnapshot>,

    /// `Ftype`: 1 = System controller, 2 = Local controller. Default 0 (FPC
    /// zero-init), which dumps '' and makes `Sample` a no-op (only `Ftype=1` acts).
    f_type: i32,

    /// `FkWLimit` — hardcoded 8000.0; there is **no property to change it** (see
    /// module note). Kept as a field to mirror the Pascal `PDiff` expression 1:1.
    f_kw_limit: f64,
    f_kw_band: f64,
    half_kw_band: f64,
    f_kvar_limit: f64,
    total_weight: f64,

    /// The type-confused `Gen.kWBase` write target (see module note). Provably
    /// unobservable: no getter reads it, it never enters the power flow or `Y`.
    phantom_kw_base: f64,

    /// `FLocalControlNameList`.
    local_control_name_list: Vec<String>,
    /// `FLocalControlWeights` (one per list entry; default 1.0).
    local_control_weights: Vec<f64>,
    /// `FLocalControlListSize`.
    local_control_list_size: i32,
    /// `FLocalControlPointerList` — resolved lazily on the first `Sample` (empty
    /// until then), cached across samples exactly like Pascal.
    local_control_pointer_list: Vec<ElemRef>,

    /// `FPVSystemNameList` + `FPVSystemWeights` + `FPVSystemListSize`. Round-trip
    /// only — `Sample` never reads these (the Pascal pointer list is dead).
    pv_system_name_list: Vec<String>,
    pv_system_weights: Vec<f64>,
    pv_system_list_size: i32,

    /// `FStorageNameList` + `FStorageWeights` + `FStorageListSize`. Round-trip
    /// only — `Sample` never reads these (the Pascal pointer list is dead).
    storage_name_list: Vec<String>,
    storage_weights: Vec<f64>,
    storage_list_size: i32,
}

impl EspvlControl {
    /// Pascal `TESPVLControlObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        ccd.cd.nphases = 3; // directly set conds and phases
        ccd.cd.nconds = 3;
        ccd.cd.set_nterms(1); // forces allocation of terminals and conductors
        ccd.element_terminal = 1;

        let f_kw_limit = 8000.0;
        let f_kw_band = 100.0;
        Self {
            ccd,
            monitored_full_name: String::new(),
            mon_snap: None,
            f_type: 0,
            f_kw_limit,
            f_kw_band,
            half_kw_band: f_kw_band / 2.0,
            f_kvar_limit: f_kw_limit / 2.0,
            total_weight: 1.0,
            phantom_kw_base: 0.0,
            local_control_name_list: Vec::new(),
            local_control_weights: Vec::new(),
            local_control_list_size: 0,
            local_control_pointer_list: Vec::new(),
            pv_system_name_list: Vec::new(),
            pv_system_weights: Vec::new(),
            pv_system_list_size: 0,
            storage_name_list: Vec::new(),
            storage_weights: Vec::new(),
            storage_list_size: 0,
        }
    }

    /// The type-confused `Gen.kWBase` field (see module note). `pub(crate)` so the
    /// dispatch env can read/write it; never observable on the circuit.
    pub(crate) fn phantom_kw_base(&self) -> f64 {
        self.phantom_kw_base
    }
    pub(crate) fn set_phantom_kw_base(&mut self, value: f64) {
        self.phantom_kw_base = value;
    }

    /// Pascal `TESPVLControlObj.MakeLocalControlList`: build the resolved
    /// subordinate-controller pointer list — **System Controllers only**. A named
    /// `LocalControlList` resolves each entry against the ESPVLControl class
    /// (keeping only enabled ones, preserving the weights the side-effect levelled);
    /// an empty name list scans every enabled ESPVLControl and allocates uniform
    /// weights. Returns whether the list ended up non-empty.
    fn make_local_control_list(&mut self, env: &dyn EspvlDispatchEnv) -> bool {
        if self.f_type != 1 {
            // Only for System controller; a Local controller never builds a list.
            return false;
        }

        if self.local_control_list_size > 0 {
            // Name list is defined — use it.
            let mut refs = Vec::with_capacity(self.local_control_name_list.len());
            for name in &self.local_control_name_list {
                if let Some(r) = env.find_enabled_espvl(name) {
                    refs.push(r);
                }
            }
            self.local_control_pointer_list = refs;
        } else {
            // Search the entire ESPVLControl class for enabled controls.
            self.local_control_pointer_list = env.all_enabled_espvls();
            // Allocate uniform weights.
            self.local_control_list_size = self.local_control_pointer_list.len() as i32;
            self.local_control_weights = vec![1.0; self.local_control_list_size.max(0) as usize];
        }

        // Add up total weights.
        self.total_weight = self
            .local_control_weights
            .iter()
            .take(self.local_control_list_size.max(0) as usize)
            .sum();

        !self.local_control_pointer_list.is_empty()
    }

    /// Pascal `TESPVLControlObj.Sample`: build the list on first call, read the
    /// monitored power, and if it is more than `HalfkWBand` outside `FkWLimit`,
    /// "redispatch" each list entry by its weighted share of the deficit. The
    /// redispatch writes each entry's [`phantom_kw_base`](Self::phantom_kw_base)
    /// (the type-confused `Gen.kWBase`), which has no electrical effect — see the
    /// module note. Returns whether any phantom value changed (ignored by the
    /// caller: Pascal `Sample` never pushes a control-queue action).
    pub(crate) fn sample(&mut self, env: &mut dyn EspvlDispatchEnv) -> bool {
        // If the list is not defined, go make one (System controllers only).
        if self.local_control_pointer_list.is_empty() {
            self.make_local_control_list(env);
        }
        if self.local_control_list_size <= 0 {
            return false;
        }

        let s = env.monitored_power(); // power in the active terminal
        let p_diff = s.re * 0.001 - self.f_kw_limit;

        let mut changed = false;
        if p_diff.abs() > self.half_kw_band {
            // PDiff is the kW needed to get back into band.
            for (i, &r) in self.local_control_pointer_list.iter().enumerate() {
                let cur = env.local_kw_base(r);
                let gen_kw =
                    (cur + p_diff * (self.local_control_weights[i] / self.total_weight)).max(1.0);
                if gen_kw != cur {
                    env.set_local_kw_base(r, gen_kw);
                    changed = true;
                }
            }
        }
        changed
    }

    /// Pascal `TESPVLControlObj.RecalcElementData` (parse-time subset): validate
    /// the monitored element and attach the control's single terminal to the
    /// monitored terminal's bus.
    pub(super) fn recalc(&mut self) {
        let Some(mon) = self.mon_snap.clone() else {
            // Pascal `DoSimpleMsg('Monitored Element in %s is not set', 372)`.
            self.ccd.cd.obj.push_error(crate::diag::DssDiagnostic::msg(
                format!(
                    "Monitored Element in ESPVLControl.{} is not set",
                    self.ccd.cd.obj.name()
                ),
                Some(372),
            ));
            return;
        };

        if self.ccd.element_terminal > mon.nterms as i32 {
            // Pascal `DoErrorMsg(... 'Terminal no. "%d" does not exist.' 371)`.
            self.ccd.cd.obj.push_error(crate::diag::DssDiagnostic::msg(
                format!(
                    "ESPVLControl: \"{}\": Terminal no. \"{}\" does not exist. Re-specify terminal no.",
                    self.ccd.cd.obj.name(),
                    self.ccd.element_terminal
                ),
                Some(371),
            ));
            return;
        }

        // Set the name of the control's 1st terminal's connected bus.
        let t = self.ccd.element_terminal;
        let bus = if t >= 1 && (t as usize) <= mon.buses.len() {
            mon.buses[(t - 1) as usize].clone()
        } else {
            String::new() // Pascal GetBus(i) out of range yields ''
        };
        self.ccd.cd.set_bus(1, &bus);
    }
}
