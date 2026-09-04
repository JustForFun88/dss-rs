//! Mode capability classification for the grouped r4133 DDLL API — the WP-G1
//! bridge rails (`GOLDEN_REBASE_PLAN.md` WP-G1, sub-step G1.0, coordinator
//! decision D2).
//!
//! The vendored `OpenDSSDirect.dll` exports **no** per-property `*_Get_*`
//! symbols: every interface is a grouped quartet `XxxI/XxxF/XxxS/XxxV(mode, …)`
//! (42 families / 147 entry points — [`crate::families`]), and a *property* is a
//! `mode` index into a Pascal `case`. A property this DLL revision does not have
//! is therefore not a `GetProcAddress` miss but a fall-through into the `case`'s
//! `else` branch, which returns a **sentinel** instead of failing. Without
//! classifying those sentinels a capture would silently record `-1` /
//! `"Error, …"` as if it were data, so this module gives the four shapes a typed
//! verdict ([`ModeStatus`]) that [`crate::dss::Engine::probe_mode`] and the typed
//! mode accessors route through.
//!
//! Two further modes are *unsafe to call at all* in this DLL revision; they are
//! held in the [`DO_NOT_CALL`] register and refused by [`check_callable`]
//! **before** any FFI happens (see the register's own citations).
//!
//! This module performs **no FFI** — it is pure classification plus two static
//! registers, so it needs no `// SAFETY` invariant of its own; the calling side
//! ([`crate::dss`]) carries one.
//!
//! ## What a verdict means
//! * `S` and `V`: the sentinel strings are not legal readings of any served
//!   mode, so both verdicts are conclusive — with the one documented exception
//!   of `CktElementS`'s bare `"Error"` ([`S_SENTINELS`]).
//! * `I` and `F`: the sentinel is the plain value `-1` / `-1.0`, which a served
//!   mode may also legally return (`DCktElement.pas:278`
//!   `Result := -1; // Signifies an error; no variable found`, *inside* a served
//!   mode). So on those two shapes [`ModeStatus::Served`] is conclusive while
//!   [`ModeStatus::UnknownMode`] is **evidence**, not proof; the positive proof
//!   that a mode is served is a comparison against a known value on a solved
//!   deck (the table-driven proof WP-G1 runs over [`crate::dss::Engine`]).
//!
//! All sentinels below were read out of the vendored r4133 DDLL sources
//! (`.inputs/electricdss-code-r4133-trunk/Version8/Source/DDLL/`) by an
//! exhaustive sweep of the `else` branches, and confirmed live against the
//! vendored DLL by the G1.0 mode probe (2026-09-04).

/// One of the four uniform DDLL ABI shapes ([`crate::ffi`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeKind {
    /// `XxxI(mode, arg): longint`.
    I,
    /// `XxxF(mode, arg[, arg2]): double`.
    F,
    /// `XxxS(mode, arg): PAnsiChar`.
    S,
    /// `XxxV(mode; var ptr, type, size)`.
    V,
}

impl ModeKind {
    /// The lower-case tag [`crate::dss::FfiCall`]'s `kind` field uses.
    pub fn as_str(self) -> &'static str {
        match self {
            ModeKind::I => "i",
            ModeKind::F => "f",
            ModeKind::S => "s",
            ModeKind::V => "v",
        }
    }
}

impl std::fmt::Display for ModeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The verdict on one `(family, kind, mode)` triple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeStatus {
    /// The reply carries **no** unknown-mode sentinel. Conclusive for `S`/`V`;
    /// for `I`/`F` see the module doc (a served mode may return `-1` legally,
    /// so a `Served` verdict there is the strong one).
    Served,
    /// The DDLL fell through its `case` into the `else` branch: this revision
    /// does not implement the mode. Carries the sentinel exactly as observed.
    UnknownMode { sentinel: String },
    /// The mode is on the [`DO_NOT_CALL`] register — refused **without**
    /// touching the DLL. The payload is the reason (with its citation).
    DoNotCall(&'static str),
}

impl ModeStatus {
    /// `true` only for [`ModeStatus::Served`].
    pub fn is_served(&self) -> bool {
        matches!(self, ModeStatus::Served)
    }

    /// `true` for a mode this DLL revision does not implement.
    pub fn is_unknown_mode(&self) -> bool {
        matches!(self, ModeStatus::UnknownMode { .. })
    }
}

impl std::fmt::Display for ModeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModeStatus::Served => f.write_str("served"),
            ModeStatus::UnknownMode { sentinel } => {
                write!(f, "unknown mode (sentinel {sentinel:?})")
            }
            ModeStatus::DoNotCall(why) => write!(f, "do-not-call: {why}"),
        }
    }
}

// ---- the four sentinel shapes -------------------------------------------

/// `XxxI` unknown-mode reply. Every WP-G1 family's `I` `else` branch is
/// `Result:=-1` — `DCktElement.pas:308`, `DBus.pas:75`, `DCircuit.pas:187`,
/// `DMeters.pas:316`, `DTopology.pas:168`, `DSolution.pas:295`,
/// `DPDELements.pas:120`.
pub const SENTINEL_I: i32 = -1;

/// `XxxF` unknown-mode reply: `Result:=-1.0` — `DCktElement.pas:414`,
/// `DBus.pas:202`, `DCircuit.pas:207`, `DMeters.pas:397`, `DSolution.pas:455`,
/// `DPDELements.pas:212`, `DCmathLib.pas:22`.
pub const SENTINEL_F: f64 = -1.0;

/// The `myType` tag a `V` unknown-mode branch reports: the string tag `4` for
/// the 35 families that write a phrase (see [`SENTINEL_V_PHRASES`]).
pub const SENTINEL_V_TYPE_TAG: i32 = 4;

/// `ActiveClassV`'s distinct unknown-mode tag — it writes no phrase at all but
/// sets `myType := -1; // Error` (`DActiveClass.pas:129`).
pub const SENTINEL_V_ERROR_TAG: i32 = -1;

/// The phrases a `V` unknown-mode branch writes, from an exhaustive sweep of all
/// 35 `WriteStr2Array` `else` branches in the vendored DDLL:
/// 33 × `'Error, parameter not recognized'` (e.g. `DCircuit.pas:792`,
/// `DCktElement.pas:1250`, `DMeters.pas:767`, `DTopology.pas:398`,
/// `DSolution.pas:671`), 1 × `'Command not recognized'` (`DBus.pas:901`) and
/// 1 × `'Error, parameter not valid;'` (`DCapControls.pas:314`).
///
/// Matched by **containment**, never equality: `DBusV`'s `else` branch
/// (`DBus.pas:899-903`) is the only one that omits the `setlength(myStrArray, 0)`
/// its siblings do (`DCktElement.pas:1249`, `DCircuit.pas:791`), so it *appends*
/// to the DLL-global string buffer and a probe measures the previous family's
/// sentinel glued to its own — measured 2026-09-04:
/// `"Error, parameter not recognizedCommand not recognized"`.
pub const SENTINEL_V_PHRASES: [&str; 3] = [
    "parameter not recognized",
    "Command not recognized",
    "parameter not valid",
];

/// The `V` families whose unknown-mode `else` branch writes **no** sentinel at
/// all, so a miss cannot be detected from the reply: `CapacitorsV`
/// (`DCapacitors.pas:293-298` hands back a 1-byte zero buffer and leaves
/// `myType` at whatever the caller passed in).
/// [`crate::dss::Engine::probe_mode`] refuses to probe `V` here rather than
/// reporting a silent `Served`.
pub const V_WITHOUT_SENTINEL: &[&str] = &["Capacitors"];

/// `true` when this crate has measured **no** `V` unknown-mode sentinel for
/// `family` ([`V_WITHOUT_SENTINEL`]).
pub fn v_sentinel_undetectable(family: &str) -> bool {
    V_WITHOUT_SENTINEL
        .iter()
        .any(|f| f.eq_ignore_ascii_case(family))
}

/// The measured per-family `XxxS` unknown-mode literal. The DDLL spells this
/// `else` branch differently in almost every unit (seven distinct literals over
/// the whole surface, five of them in the WP-G1 families), so it is a table and
/// not a constant. Each row cites the `else` branch it transcribes; every literal
/// was confirmed live by the G1.0 probe (2026-09-04).
///
/// `CktElement`'s bare `"Error"` is **ambiguous in principle** — a served string
/// mode returns exactly that when a variable name is unknown
/// (`DCktElement.pas:462`, inside the served mode 4) — so it is fit for the
/// probe/diagnostic path only and must never gate a capture.
pub const S_SENTINELS: &[(&str, &str)] = &[
    // `DCktElement.pas:483` — bare, ambiguous (see above).
    ("CktElement", "Error"),
    // `DBus.pas:217` — note the upstream wording `Parameter non recognized`.
    ("Bus", "Error, Parameter non recognized"),
    // `DCircuit.pas:264`.
    ("Circuit", "Error, parameter not recognized"),
    // `DMeters.pas:477` — capital `Parameter`.
    ("Meters", "Error, Parameter not recognized"),
    // `DTopology.pas:252`.
    ("Topology", "Error, parameter not valid"),
    // `DSolution.pas:513` — upstream typo `paratemer`, transcribed verbatim.
    ("Solution", "Error, paratemer not recognized"),
    // `DPDELements.pas:255`.
    ("PDElements", "Error, parameter not valid"),
];

/// The measured `S` unknown-mode literal of `family`, or `None` when this crate
/// has not measured one. [`crate::dss::Engine::probe_mode`] **refuses** to probe
/// `S` on a family with no row rather than reporting a silent `Served`, so a
/// missing row can never read as a pass.
pub fn s_sentinel(family: &str) -> Option<&'static str> {
    S_SENTINELS
        .iter()
        .find(|(f, _)| f.eq_ignore_ascii_case(family))
        .map(|(_, s)| *s)
}

/// Classify an `XxxI` reply. See the module doc: `Served` is the conclusive
/// verdict on this shape.
pub fn classify_i(value: i32) -> ModeStatus {
    if value == SENTINEL_I {
        ModeStatus::UnknownMode {
            sentinel: value.to_string(),
        }
    } else {
        ModeStatus::Served
    }
}

/// Classify an `XxxF` reply. See the module doc: `Served` is the conclusive
/// verdict on this shape.
pub fn classify_f(value: f64) -> ModeStatus {
    if value == SENTINEL_F {
        ModeStatus::UnknownMode {
            sentinel: value.to_string(),
        }
    } else {
        ModeStatus::Served
    }
}

/// Classify an `XxxS` reply against `family`'s own literal ([`S_SENTINELS`]),
/// compared ASCII-case-insensitively. A family with no measured row yields
/// `Served`; callers that must not mask a miss ([`crate::dss::Engine::probe_mode`])
/// consult [`s_sentinel`] first and refuse.
pub fn classify_s(family: &str, reply: &str) -> ModeStatus {
    match s_sentinel(family) {
        Some(sent) if reply.eq_ignore_ascii_case(sent) => ModeStatus::UnknownMode {
            sentinel: reply.to_string(),
        },
        _ => ModeStatus::Served,
    }
}

/// Classify an `XxxV` reply from the **raw** `myType` tag and the decoded string
/// array (empty for a non-string tag). Containment, not equality — see
/// [`SENTINEL_V_PHRASES`].
pub fn classify_v(type_tag: i32, strings: &[String]) -> ModeStatus {
    if type_tag == SENTINEL_V_ERROR_TAG {
        return ModeStatus::UnknownMode {
            sentinel: format!("myType={SENTINEL_V_ERROR_TAG}"),
        };
    }
    if type_tag == SENTINEL_V_TYPE_TAG {
        for s in strings {
            if SENTINEL_V_PHRASES.iter().any(|p| s.contains(p)) {
                return ModeStatus::UnknownMode {
                    sentinel: s.clone(),
                };
            }
        }
    }
    ModeStatus::Served
}

// ---- the do-not-call register -------------------------------------------

/// `(family, kind, mode, reason)` rows this bridge must **never** dispatch:
/// modes whose r4133 implementation is memory-unsafe. [`check_callable`] refuses
/// them before any FFI, and no typed accessor exists for either.
///
/// Both are r4133 defects (`investigations/to_opendss/` material) and neither is
/// on WP-G1's mode list, so refusing them costs no comparison channel.
pub const DO_NOT_CALL: &[(&str, ModeKind, i32, &str)] = &[
    (
        "Solution",
        ModeKind::V,
        2,
        "Solution.BusLevels: DSolution.pas:580-582 does setlength(myIntArray, ArrSize) then \
         `for IMIdx := 0 to ArrSize` — ArrSize+1 writes into an ArrSize-long array, a \
         one-element heap overflow inside the DLL (source-proven; never called)",
    ),
    (
        "Bus",
        ModeKind::V,
        17,
        "Bus.ZSC012Matrix: DBus.pas:803-838 calls Zsc.MtrxMult(As2p) with no Assigned(Zsc) \
         guard, so a bus whose Zsc was never built (no fault study) nil-derefs — MEASURED \
         process kill of the worker on IEEE13 (G1.0 probe, 2026-09-04)",
    ),
];

/// Refuse a `(family, kind, mode)` on the [`DO_NOT_CALL`] register **before** any
/// DLL call; `None` means the triple is safe to dispatch. Family names compare
/// case-insensitively, matching [`crate::families::FamilyTable::get`].
pub fn check_callable(family: &str, kind: ModeKind, mode: i32) -> Option<ModeStatus> {
    DO_NOT_CALL
        .iter()
        .find(|(f, k, m, _)| *k == kind && *m == mode && f.eq_ignore_ascii_case(family))
        .map(|(_, _, _, why)| ModeStatus::DoNotCall(why))
}

// ---- the WP-G1 mode table ------------------------------------------------

/// What calling a mode does to engine state beyond returning its value.
///
/// The DDLL `case` arms are not uniformly pure: several *getters* walk a
/// `PointerList` or the topology tree and leave its cursor moved, and one
/// re-totalises the meter registers. A capture that reads such a mode must
/// re-select its fixture afterwards, so the effect is part of the table rather
/// than a comment someone has to find.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeEffect {
    /// A pure read: nothing in the engine changes.
    Pure,
    /// A read that also mutates engine state. The payload names exactly what
    /// moves (cursor, memoized cache, …) and cites the Pascal.
    Impure(&'static str),
}

/// One mode WP-G1 reads through the r4133 bridge: the DDLL `(family, kind,
/// mode)` triple plus the `case` arm it transcribes.
///
/// The mode *number* lives here and nowhere else — every typed accessor on
/// [`crate::dss::Engine`] takes a `&ModeSpec`, so the table can be walked by a
/// single test and a number can never drift between a table row and its reader.
///
/// **Getters only.** A DDLL `case` arm that *writes* is not a table row: the
/// generic reader drives a mode with a neutral argument, which for a write arm
/// would store that argument into the engine (see [`EXCLUDED_WRITE_MODES`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeSpec {
    /// The [`crate::families`] registry name (case-insensitive lookup).
    pub family: &'static str,
    /// Which of the four ABI shapes serves this mode.
    pub kind: ModeKind,
    /// The `mode` index into the family's Pascal `case`.
    pub mode: i32,
    /// `Family.Property` — the dss-python-facing property name, normalized into
    /// the family's own namespace. Where the DDLL `case` comment spells it
    /// differently (an upstream typo, a singular family name) the const's doc
    /// comment quotes the comment verbatim; [`ModeSpec::pas`] is the authority.
    pub name: &'static str,
    /// `D<Unit>.pas:<line>` — the `case` arm this row transcribes, in the
    /// vendored r4133 DDLL
    /// (`.inputs/electricdss-code-r4133-trunk/Version8/Source/DDLL/`).
    pub pas: &'static str,
    /// For a `V` row, the `myType` tag its arm assigns (1 = int, 2 = double,
    /// 3 = complex, 4 = string); `None` for `I`/`F`/`S`. The generic reader
    /// checks the observed tag against it, so a shape change in a future DLL
    /// revision fails loudly instead of decoding garbage.
    pub v_type: Option<i32>,
    /// Side effects beyond the returned value.
    pub effect: ModeEffect,
}

impl ModeSpec {
    /// An `I` / `F` / `S` scalar row.
    const fn scalar(
        family: &'static str,
        kind: ModeKind,
        mode: i32,
        name: &'static str,
        pas: &'static str,
        effect: ModeEffect,
    ) -> ModeSpec {
        ModeSpec {
            family,
            kind,
            mode,
            name,
            pas,
            v_type: None,
            effect,
        }
    }

    /// A `V` array row, carrying the `myType` tag its `case` arm declares.
    const fn array(
        family: &'static str,
        mode: i32,
        name: &'static str,
        pas: &'static str,
        v_type: i32,
        effect: ModeEffect,
    ) -> ModeSpec {
        ModeSpec {
            family,
            kind: ModeKind::V,
            mode,
            name,
            pas,
            v_type: Some(v_type),
            effect,
        }
    }
}

impl std::fmt::Display for ModeSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}{}:{} — {})",
            self.name,
            self.family,
            self.kind.as_str().to_ascii_uppercase(),
            self.mode,
            self.pas
        )
    }
}

// -- CktElement (DCktElement.pas) ------------------------------------------

/// `CktElementI(0)` — number of terminals.
pub const CKT_ELEMENT_NUM_TERMINALS: ModeSpec = ModeSpec::scalar(
    "CktElement",
    ModeKind::I,
    0,
    "CktElement.NumTerminals",
    "DCktElement.pas:139",
    ModeEffect::Pure,
);
/// `CktElementI(1)` — number of conductors per terminal.
pub const CKT_ELEMENT_NUM_CONDUCTORS: ModeSpec = ModeSpec::scalar(
    "CktElement",
    ModeKind::I,
    1,
    "CktElement.NumConductors",
    "DCktElement.pas:144",
    ModeEffect::Pure,
);
/// `CktElementI(2)` — number of phases.
pub const CKT_ELEMENT_NUM_PHASES: ModeSpec = ModeSpec::scalar(
    "CktElement",
    ModeKind::I,
    2,
    "CktElement.NumPhases",
    "DCktElement.pas:149",
    ModeEffect::Pure,
);
/// `CktElementI(7)` — 1 when a SwtControl is attached.
pub const CKT_ELEMENT_HAS_SWITCH_CONTROL: ModeSpec = ModeSpec::scalar(
    "CktElement",
    ModeKind::I,
    7,
    "CktElement.HasSwitchControl",
    "DCktElement.pas:207",
    ModeEffect::Pure,
);
/// `CktElementI(8)` — 1 when a voltage-regulating control is attached.
pub const CKT_ELEMENT_HAS_VOLT_CONTROL: ModeSpec = ModeSpec::scalar(
    "CktElement",
    ModeKind::I,
    8,
    "CktElement.HasVoltControl",
    "DCktElement.pas:222",
    ModeEffect::Pure,
);
/// `CktElementI(9)` — size of the element's `ControlElementList`.
pub const CKT_ELEMENT_NUM_CONTROLS: ModeSpec = ModeSpec::scalar(
    "CktElement",
    ModeKind::I,
    9,
    "CktElement.NumControls",
    "DCktElement.pas:237",
    ModeEffect::Pure,
);
/// `CktElementI(10)` — 1-based index of the first Fuse/Recloser/Relay in the
/// element's control list; 0 when it has none.
pub const CKT_ELEMENT_OCP_DEV_INDEX: ModeSpec = ModeSpec::scalar(
    "CktElement",
    ModeKind::I,
    10,
    "CktElement.OCPDevIndex",
    "DCktElement.pas:242",
    ModeEffect::Pure,
);
/// `CktElementI(11)` — `GetOCPDeviceType` (`Utilities.pas`); 0 = none.
pub const CKT_ELEMENT_OCP_DEV_TYPE: ModeSpec = ModeSpec::scalar(
    "CktElement",
    ModeKind::I,
    11,
    "CktElement.OCPDevType",
    "DCktElement.pas:259",
    ModeEffect::Pure,
);
/// `CktElementI(15)` — 1 when the element has an over-current protection device.
pub const CKT_ELEMENT_HAS_OCP_DEVICE: ModeSpec = ModeSpec::scalar(
    "CktElement",
    ModeKind::I,
    15,
    "CktElement.HasOCPDevice",
    "DCktElement.pas:300",
    ModeEffect::Pure,
);
/// `CktElementS(4)` — the name of the EnergyMeter this element belongs to.
///
/// Returns `CktElementS`'s pre-`case` default `"0"` (`DCktElement.pas:421`) when
/// the element has no meter — **not** the family's `"Error"` sentinel, so the
/// ambiguity documented at [`S_SENTINELS`] does not reach this row.
pub const CKT_ELEMENT_ENERGY_METER: ModeSpec = ModeSpec::scalar(
    "CktElement",
    ModeKind::S,
    4,
    "CktElement.EnergyMeter",
    "DCktElement.pas:442",
    ModeEffect::Pure,
);
/// `CktElementV(6)` — per-phase losses, complex `[re, im, …]`.
pub const CKT_ELEMENT_PHASE_LOSSES: ModeSpec = ModeSpec::array(
    "CktElement",
    6,
    "CktElement.PhaseLosses",
    "DCktElement.pas:637",
    3,
    ModeEffect::Pure,
);
/// `CktElementV(7)` — symmetrical-component voltage **magnitudes** per terminal
/// (`myType := 2`, real doubles).
pub const CKT_ELEMENT_SEQ_VOLTAGES: ModeSpec = ModeSpec::array(
    "CktElement",
    7,
    "CktElement.SeqVoltages",
    "DCktElement.pas:660",
    2,
    ModeEffect::Pure,
);
/// `CktElementV(8)` — symmetrical-component current magnitudes per terminal.
pub const CKT_ELEMENT_SEQ_CURRENTS: ModeSpec = ModeSpec::array(
    "CktElement",
    8,
    "CktElement.SeqCurrents",
    "DCktElement.pas:700",
    2,
    ModeEffect::Pure,
);
/// `CktElementV(9)` — sequence powers per terminal, complex `[re, im, …]`.
pub const CKT_ELEMENT_SEQ_POWERS: ModeSpec = ModeSpec::array(
    "CktElement",
    9,
    "CktElement.SeqPowers",
    "DCktElement.pas:739",
    3,
    ModeEffect::Pure,
);
/// `CktElementV(11)` — residual current per terminal, as `(magnitude, angle°)`.
pub const CKT_ELEMENT_RESIDUALS: ModeSpec = ModeSpec::array(
    "CktElement",
    11,
    "CktElement.Residuals",
    "DCktElement.pas:827",
    3,
    ModeEffect::Pure,
);
/// `CktElementV(13)` — complex sequence voltages per terminal.
pub const CKT_ELEMENT_CPLX_SEQ_VOLTAGES: ModeSpec = ModeSpec::array(
    "CktElement",
    13,
    "CktElement.CplxSeqVoltages",
    "DCktElement.pas:885",
    3,
    ModeEffect::Pure,
);
/// `CktElementV(14)` — complex sequence currents per terminal.
pub const CKT_ELEMENT_CPLX_SEQ_CURRENTS: ModeSpec = ModeSpec::array(
    "CktElement",
    14,
    "CktElement.CplxSeqCurrents",
    "DCktElement.pas:931",
    3,
    ModeEffect::Pure,
);
/// `CktElementV(17)` — the element's node order (`myType := 1`). The `case`
/// comment spells it `Nodeorder`.
pub const CKT_ELEMENT_NODE_ORDER: ModeSpec = ModeSpec::array(
    "CktElement",
    17,
    "CktElement.NodeOrder",
    "DCktElement.pas:1032",
    1,
    ModeEffect::Pure,
);
/// `CktElementV(18)` — terminal currents as `(magnitude, angle°)` pairs.
pub const CKT_ELEMENT_CURRENTS_MAG_ANG: ModeSpec = ModeSpec::array(
    "CktElement",
    18,
    "CktElement.CurrentsMagAng",
    "DCktElement.pas:1058",
    3,
    ModeEffect::Pure,
);
/// `CktElementV(19)` — terminal voltages as `(magnitude, angle°)` pairs.
pub const CKT_ELEMENT_VOLTAGES_MAG_ANG: ModeSpec = ModeSpec::array(
    "CktElement",
    19,
    "CktElement.VoltagesMagAng",
    "DCktElement.pas:1082",
    3,
    ModeEffect::Pure,
);
/// `CktElementV(20)` — total complex power per terminal.
pub const CKT_ELEMENT_TOTAL_POWERS: ModeSpec = ModeSpec::array(
    "CktElement",
    20,
    "CktElement.TotalPowers",
    "DCktElement.pas:1109",
    3,
    ModeEffect::Pure,
);

// -- Bus (DBus.pas) ---------------------------------------------------------

/// `BUSF(5)` — the active bus's `DistFromMeter`.
pub const BUS_DISTANCE: ModeSpec = ModeSpec::scalar(
    "Bus",
    ModeKind::F,
    5,
    "Bus.Distance",
    "DBus.pas:122",
    ModeEffect::Pure,
);
/// `BUSV(1)` — the three symmetrical-component voltage **magnitudes**
/// (`Cabs(V012[i])`), or `-1.0 ×3` on a bus whose node count is not exactly 3
/// (`DBus.pas:298-300`, the upstream "n/A for less then 3 phases" marker).
///
/// The arm tags the buffer `myType := 3` (complex) while writing three *real*
/// magnitudes with `mySize := 3 × SizeOf(double)`; the tag is transcribed as
/// measured, and what the payload means is settled by the surface sub-step that
/// compares it.
pub const BUS_SEQ_VOLTAGES: ModeSpec = ModeSpec::array(
    "Bus",
    1,
    "Bus.SeqVoltages",
    "DBus.pas:285",
    3,
    ModeEffect::Pure,
);
/// `BUSV(3)` — open-circuit voltage `Voc` at the bus (fault study).
pub const BUS_VOC: ModeSpec =
    ModeSpec::array("Bus", 3, "Bus.Voc", "DBus.pas:351", 3, ModeEffect::Pure);
/// `BUSV(4)` — short-circuit current `Isc` at the bus (fault study).
pub const BUS_ISC: ModeSpec =
    ModeSpec::array("Bus", 4, "Bus.Isc", "DBus.pas:374", 3, ModeEffect::Pure);
/// `BUSV(5)` — per-unit complex node voltages at the bus.
pub const BUS_PU_VOLTAGES: ModeSpec = ModeSpec::array(
    "Bus",
    5,
    "Bus.PuVoltages",
    "DBus.pas:399",
    3,
    ModeEffect::Pure,
);
/// `BUSV(6)` — the bus `Zsc` matrix, row-major complex. Zero-filled unless a
/// fault study built `Zsc`; the arm is `Assigned`-guarded, unlike `V:17` (see
/// [`DO_NOT_CALL`]).
pub const BUS_ZSC_MATRIX: ModeSpec = ModeSpec::array(
    "Bus",
    6,
    "Bus.ZscMatrix",
    "DBus.pas:431",
    3,
    ModeEffect::Pure,
);
/// `BUSV(7)` — positive-sequence `Zsc1`. The `case` comment reads `Bus.Zcs1`.
pub const BUS_ZSC1: ModeSpec =
    ModeSpec::array("Bus", 7, "Bus.Zsc1", "DBus.pas:461", 3, ModeEffect::Pure);
/// `BUSV(8)` — zero-sequence `Zsc0`.
pub const BUS_ZSC0: ModeSpec =
    ModeSpec::array("Bus", 8, "Bus.Zsc0", "DBus.pas:476", 3, ModeEffect::Pure);
/// `BUSV(9)` — the bus `Ysc` matrix, row-major complex.
pub const BUS_YSC_MATRIX: ModeSpec = ModeSpec::array(
    "Bus",
    9,
    "Bus.YscMatrix",
    "DBus.pas:491",
    3,
    ModeEffect::Pure,
);
/// `BUSV(10)` — complex symmetrical-component voltages at the bus.
pub const BUS_CPLX_SEQ_VOLTAGES: ModeSpec = ModeSpec::array(
    "Bus",
    10,
    "Bus.CplxSeqVoltages",
    "DBus.pas:520",
    3,
    ModeEffect::Pure,
);
/// `BUSV(11)` — line-to-line complex voltages.
pub const BUS_VLL: ModeSpec =
    ModeSpec::array("Bus", 11, "Bus.VLL", "DBus.pas:549", 3, ModeEffect::Pure);
/// `BUSV(12)` — per-unit line-to-line voltages. The `case` comment reads
/// `Bus. PuVLL` (stray space).
pub const BUS_PU_VLL: ModeSpec =
    ModeSpec::array("Bus", 12, "Bus.PuVLL", "DBus.pas:603", 3, ModeEffect::Pure);
/// `BUSV(13)` — node voltages as `(magnitude, angle°)` pairs.
pub const BUS_VMAG_ANGLE: ModeSpec = ModeSpec::array(
    "Bus",
    13,
    "Bus.VMagAngle",
    "DBus.pas:659",
    3,
    ModeEffect::Pure,
);
/// `BUSV(14)` — per-unit node voltages as `(magnitude, angle°)` pairs.
pub const BUS_PU_VMAG_ANGLE: ModeSpec = ModeSpec::array(
    "Bus",
    14,
    "Bus.PuVMagAngle",
    "DBus.pas:690",
    3,
    ModeEffect::Pure,
);
/// `BUSV(18)` — qualified names of the PC elements connected at the bus.
pub const BUS_ALL_PCE_AT_BUS: ModeSpec = ModeSpec::array(
    "Bus",
    18,
    "Bus.AllPCEatBus",
    "DBus.pas:840",
    4,
    ModeEffect::Pure,
);
/// `BUSV(19)` — qualified names of the PD elements connected at the bus.
pub const BUS_ALL_PDE_AT_BUS: ModeSpec = ModeSpec::array(
    "Bus",
    19,
    "Bus.AllPDEatBus",
    "DBus.pas:867",
    4,
    ModeEffect::Pure,
);

// -- Circuit (DCircuit.pas) -------------------------------------------------

/// `CircuitV(0)` — total circuit losses, complex `[re, im]`.
pub const CIRCUIT_LOSSES: ModeSpec = ModeSpec::array(
    "Circuit",
    0,
    "Circuit.Losses",
    "DCircuit.pas:294",
    3,
    ModeEffect::Impure(
        "walks ActiveCircuit.PDElements.First/Next to exhaustion (Common/Circuit.pas:2436-2443 via \
         DCircuit.pas:294), leaving the PDElements list cursor at the end — Circuit.NextPDElement \
         resumes from there — and refreshes every enabled non-shunt PD element's Iterminal cache \
         (Get_Losses calls ComputeIterminal, Common/CktElement.pas:743)",
    ),
);
/// `CircuitV(1)` — losses of the Line elements only.
pub const CIRCUIT_LINE_LOSSES: ModeSpec = ModeSpec::array(
    "Circuit",
    1,
    "Circuit.LineLosses",
    "DCircuit.pas:305",
    3,
    ModeEffect::Impure(
        "walks ActiveCircuit.Lines.First/Next to exhaustion (DCircuit.pas:313-318), leaving the Lines \
         list cursor at the end, and refreshes every line's Iterminal cache (Get_Losses calls \
         ComputeIterminal, Common/CktElement.pas:743)",
    ),
);
/// `CircuitV(2)` — losses of the Transformer elements only.
pub const CIRCUIT_SUBSTATION_LOSSES: ModeSpec = ModeSpec::array(
    "Circuit",
    2,
    "Circuit.SubstationLosses",
    "DCircuit.pas:327",
    3,
    ModeEffect::Impure(
        "walks ActiveCircuit.Transformers.First/Next to exhaustion (DCircuit.pas:335-340), leaving the \
         Transformers list cursor at the end — the cursor the discrete capture's \
         Transformers.First/Next drives — and refreshes each IsSubstation transformer's Iterminal cache \
         (Get_Losses calls ComputeIterminal, Common/CktElement.pas:743)",
    ),
);
/// `CircuitV(3)` — total power drawn from the sources, complex.
pub const CIRCUIT_TOTAL_POWER: ModeSpec = ModeSpec::array(
    "Circuit",
    3,
    "Circuit.TotalPower",
    "DCircuit.pas:349",
    3,
    ModeEffect::Impure(
        "walks ActiveCircuit.Sources.First/Next to exhaustion (DCircuit.pas:356-360), leaving the \
         Sources list cursor at the end, and Get_Power sets ActiveTerminalIdx := 1 and calls \
         ComputeIterminal on every source (Common/CktElement.pas:677-680)",
    ),
);
/// `CircuitV(8)` — per-element losses, complex, in `AllElementNames` order.
pub const CIRCUIT_ALL_ELEMENT_LOSSES: ModeSpec = ModeSpec::array(
    "Circuit",
    8,
    "Circuit.AllElementLosses",
    "DCircuit.pas:458",
    3,
    ModeEffect::Impure(
        "walks ActiveCircuit.CktElements.First/Next to exhaustion (DCircuit.pas:468-473), leaving the \
         CktElements list cursor at the end, and calls Get_Losses -> ComputeIterminal on EVERY \
         element (Common/CktElement.pas:743), refreshing the whole circuit's Iterminal caches",
    ),
);
/// `CircuitV(9)` — per-unit voltage magnitude of every node.
pub const CIRCUIT_ALL_BUS_MAG_PU: ModeSpec = ModeSpec::array(
    "Circuit",
    9,
    "Circuit.AllBusMagPu",
    "DCircuit.pas:481",
    2,
    ModeEffect::Pure,
);
/// `CircuitV(12)` — `DistFromMeter` of every bus.
pub const CIRCUIT_ALL_BUS_DISTANCES: ModeSpec = ModeSpec::array(
    "Circuit",
    12,
    "Circuit.AllBusDistances",
    "DCircuit.pas:566",
    2,
    ModeEffect::Pure,
);
/// `CircuitV(13)` — `DistFromMeter` of every node.
pub const CIRCUIT_ALL_NODE_DISTANCES: ModeSpec = ModeSpec::array(
    "Circuit",
    13,
    "Circuit.AllNodeDistances",
    "DCircuit.pas:582",
    2,
    ModeEffect::Pure,
);

// -- Meters (DMeters.pas) ---------------------------------------------------

/// `MetersI(20)` — customers served by the active meter's zone.
pub const METERS_TOTAL_CUSTOMERS: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::I,
    20,
    "Meters.TotalCustomers",
    "DMeters.pas:232",
    ModeEffect::Pure,
);
/// `MetersI(21)` — number of feeder sections in the active meter's zone.
pub const METERS_NUM_SECTIONS: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::I,
    21,
    "Meters.NumSections",
    "DMeters.pas:244",
    ModeEffect::Pure,
);
/// `MetersI(23)` — OCP device type of the section selected by
/// `Meters.SetActiveSection` (`MetersI(22)`); 0 when no section is active.
pub const METERS_OCP_DEVICE_TYPE: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::I,
    23,
    "Meters.OCPDeviceType",
    "DMeters.pas:265",
    ModeEffect::Pure,
);
/// `MetersI(24)` — customers in the active section.
pub const METERS_NUM_SECTION_CUSTOMERS: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::I,
    24,
    "Meters.NumSectionCustomers",
    "DMeters.pas:275",
    ModeEffect::Pure,
);
/// `MetersI(25)` — branches in the active section.
pub const METERS_NUM_SECTION_BRANCHES: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::I,
    25,
    "Meters.NumSectionBranches",
    "DMeters.pas:285",
    ModeEffect::Pure,
);
/// `MetersI(26)` — sequence-list index of the active section's head branch. The
/// `case` comment reads `Meters.SectSeqidx`.
pub const METERS_SECT_SEQ_IDX: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::I,
    26,
    "Meters.SectSeqIdx",
    "DMeters.pas:295",
    ModeEffect::Pure,
);
/// `MetersI(27)` — total customers downstream of the active section.
pub const METERS_SECT_TOTAL_CUST: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::I,
    27,
    "Meters.SectTotalCust",
    "DMeters.pas:305",
    ModeEffect::Pure,
);
/// `MetersF(0)` — SAIFI of the active meter's zone.
pub const METERS_SAIFI: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::F,
    0,
    "Meters.SAIFI",
    "DMeters.pas:329",
    ModeEffect::Pure,
);
/// `MetersF(1)` — SAIFI weighted by kW.
pub const METERS_SAIFI_KW: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::F,
    1,
    "Meters.SAIFIkW",
    "DMeters.pas:340",
    ModeEffect::Pure,
);
/// `MetersF(2)` — SAIDI of the active meter's zone.
pub const METERS_SAIDI: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::F,
    2,
    "Meters.SAIDI",
    "DMeters.pas:351",
    ModeEffect::Pure,
);
/// `MetersF(3)` — customer interruptions. The `case` comment carries the
/// upstream typo `Meters.CustItnerrupts`.
pub const METERS_CUST_INTERRUPTS: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::F,
    3,
    "Meters.CustInterrupts",
    "DMeters.pas:360",
    ModeEffect::Pure,
);
/// `MetersF(4)` — average repair time of the zone.
pub const METERS_AVG_REPAIR_TIME: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::F,
    4,
    "Meters.AvgRepairTime",
    "DMeters.pas:369",
    ModeEffect::Pure,
);
/// `MetersF(5)` — Σ fault rate × repair hours over the zone.
pub const METERS_FAULT_RATE_X_REPAIR_HRS: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::F,
    5,
    "Meters.FaultRateXRepairHrs",
    "DMeters.pas:378",
    ModeEffect::Pure,
);
/// `MetersF(6)` — Σ branch fault rates over the zone.
pub const METERS_SUM_BRANCH_FLT_RATES: ModeSpec = ModeSpec::scalar(
    "Meters",
    ModeKind::F,
    6,
    "Meters.SumBranchFltRates",
    "DMeters.pas:387",
    ModeEffect::Pure,
);
/// `MetersV(3)` — the circuit-wide `RegisterTotals`, one double per
/// `NumEMRegisters`.
pub const METERS_TOTALS: ModeSpec = ModeSpec::array(
    "Meters",
    3,
    "Meters.Totals",
    "DMeters.pas:558",
    2,
    ModeEffect::Impure(
        "calls TotalizeMeters (DMeters.pas:566), which recomputes the circuit's RegisterTotals \
         from every meter",
    ),
);
/// `MetersV(6)` — the active meter's calculated currents. The `case` comment
/// reads `Meter.CalcCurrent read` (singular family).
pub const METERS_CALC_CURRENT: ModeSpec = ModeSpec::array(
    "Meters",
    6,
    "Meters.CalcCurrent",
    "DMeters.pas:609",
    2,
    ModeEffect::Pure,
);
/// `MetersV(8)` — the active meter's per-phase allocation factors.
pub const METERS_ALLOC_FACTORS: ModeSpec = ModeSpec::array(
    "Meters",
    8,
    "Meters.AllocFactors",
    "DMeters.pas:645",
    2,
    ModeEffect::Pure,
);

// -- Topology (DTopology.pas) -----------------------------------------------

/// Every `Topology` row reaches the tree through `ActiveTree` =
/// `ActiveCircuit.GetTopology` (`DTopology.pas:13-17`), which **builds and
/// memoizes** the topology on first use, and then walks its cursor.
const TOPO_TREE: ModeEffect = ModeEffect::Impure(
    "ActiveTree = ActiveCircuit.GetTopology (DTopology.pas:13-17) builds and memoizes the \
     topology on first use; the arm then walks topo.First/GoForward, leaving the tree's \
     PresentBranch cursor moved",
);
/// The PD-side isolation counts iterate a circuit `PointerList` to exhaustion,
/// leaving that list's own cursor at the end — the cursor
/// `Circuit.NextPDElement` resumes from.
const TOPO_PD_LIST: ModeEffect = ModeEffect::Impure(
    "walks ActiveCircuit.PDElements.First/Next to exhaustion on top of the GetTopology build (DTopology.pas:13-17), \
     leaving the PDElements list cursor at the end — Circuit.NextPDElement resumes from there",
);
/// The PC-side counterpart of [`TOPO_PD_LIST`].
const TOPO_PC_LIST: ModeEffect = ModeEffect::Impure(
    "walks ActiveCircuit.PCElements.First/Next to exhaustion on top of the GetTopology build (DTopology.pas:13-17), \
     leaving the PCElements list cursor at the end — Circuit.NextPCElement resumes from there",
);

/// `TopologyI(0)` — number of loops (`IsLoopedHere` count ÷ 2).
pub const TOPOLOGY_NUM_LOOPS: ModeSpec = ModeSpec::scalar(
    "Topology",
    ModeKind::I,
    0,
    "Topology.NumLoops",
    "DTopology.pas:67",
    TOPO_TREE,
);
/// `TopologyI(1)` — number of isolated PD elements.
pub const TOPOLOGY_NUM_ISOLATED_BRANCHES: ModeSpec = ModeSpec::scalar(
    "Topology",
    ModeKind::I,
    1,
    "Topology.NumIsolatedBranches",
    "DTopology.pas:79",
    TOPO_PD_LIST,
);
/// `TopologyI(2)` — number of isolated PC elements.
pub const TOPOLOGY_NUM_ISOLATED_LOADS: ModeSpec = ModeSpec::scalar(
    "Topology",
    ModeKind::I,
    2,
    "Topology.NumIsolatedLoads",
    "DTopology.pas:89",
    TOPO_PC_LIST,
);
/// `TopologyV(0)` — the looped branch pairs; `'NONE'` when there are none.
pub const TOPOLOGY_ALL_LOOPED_PAIRS: ModeSpec = ModeSpec::array(
    "Topology",
    0,
    "Topology.AllLoopedPairs",
    "DTopology.pas:271",
    4,
    TOPO_TREE,
);
/// `TopologyV(1)` — qualified names of the isolated PD elements.
pub const TOPOLOGY_ALL_ISOLATED_BRANCHES: ModeSpec = ModeSpec::array(
    "Topology",
    1,
    "Topology.AllIsolatedBranches",
    "DTopology.pas:322",
    4,
    TOPO_PD_LIST,
);
/// `TopologyV(2)` — qualified names of the isolated PC elements.
pub const TOPOLOGY_ALL_ISOLATED_LOADS: ModeSpec = ModeSpec::array(
    "Topology",
    2,
    "Topology.AllIsolatedLoads",
    "DTopology.pas:357",
    4,
    TOPO_PC_LIST,
);

// -- Solution (DSolution.pas) -----------------------------------------------

/// `SolutionI(1)` — the solution mode id.
pub const SOLUTION_MODE: ModeSpec = ModeSpec::scalar(
    "Solution",
    ModeKind::I,
    1,
    "Solution.Mode",
    "DSolution.pas:29",
    ModeEffect::Pure,
);
/// `SolutionI(3)` — the integer hour of the solution clock.
pub const SOLUTION_HOUR: ModeSpec = ModeSpec::scalar(
    "Solution",
    ModeKind::I,
    3,
    "Solution.Hour",
    "DSolution.pas:37",
    ModeEffect::Pure,
);
/// `SolutionI(5)` — the solution year.
pub const SOLUTION_YEAR: ModeSpec = ModeSpec::scalar(
    "Solution",
    ModeKind::I,
    5,
    "Solution.Year",
    "DSolution.pas:47",
    ModeEffect::Pure,
);
/// `SolutionI(7)` — iterations taken by the last solve.
pub const SOLUTION_ITERATIONS: ModeSpec = ModeSpec::scalar(
    "Solution",
    ModeKind::I,
    7,
    "Solution.Iterations",
    "DSolution.pas:54",
    ModeEffect::Pure,
);
/// `SolutionI(22)` — control iterations taken by the last solve.
pub const SOLUTION_CONTROL_ITERATIONS: ModeSpec = ModeSpec::scalar(
    "Solution",
    ModeKind::I,
    22,
    "Solution.ControlIterations",
    "DSolution.pas:113",
    ModeEffect::Pure,
);
/// `SolutionI(37)` — 1 when the system Y needs rebuilding.
pub const SOLUTION_SYSTEM_Y_CHANGED: ModeSpec = ModeSpec::scalar(
    "Solution",
    ModeKind::I,
    37,
    "Solution.SystemYChanged",
    "DSolution.pas:192",
    ModeEffect::Pure,
);
/// `SolutionI(40)` — iterations accumulated over all time steps.
pub const SOLUTION_TOTAL_ITERATIONS: ModeSpec = ModeSpec::scalar(
    "Solution",
    ModeKind::I,
    40,
    "Solution.TotalIterations",
    "DSolution.pas:218",
    ModeEffect::Pure,
);
/// `SolutionI(41)` — the largest per-step iteration count seen.
pub const SOLUTION_MOST_ITERATIONS_DONE: ModeSpec = ModeSpec::scalar(
    "Solution",
    ModeKind::I,
    41,
    "Solution.MostIterationsDone",
    "DSolution.pas:222",
    ModeEffect::Pure,
);
/// `SolutionI(42)` — 1 when all control actions have been executed.
pub const SOLUTION_CONTROL_ACTIONS_DONE: ModeSpec = ModeSpec::scalar(
    "Solution",
    ModeKind::I,
    42,
    "Solution.ControlActionsDone",
    "DSolution.pas:226",
    ModeEffect::Pure,
);
/// `SolutionF(2)` — seconds past the hour.
pub const SOLUTION_SECONDS: ModeSpec = ModeSpec::scalar(
    "Solution",
    ModeKind::F,
    2,
    "Solution.Seconds",
    "DSolution.pas:312",
    ModeEffect::Pure,
);
/// `SolutionF(6)` — the global load multiplier.
pub const SOLUTION_LOAD_MULT: ModeSpec = ModeSpec::scalar(
    "Solution",
    ModeKind::F,
    6,
    "Solution.LoadMult",
    "DSolution.pas:336",
    ModeEffect::Pure,
);
/// `SolutionF(20)` — the solution clock as a fractional hour.
pub const SOLUTION_DBL_HOUR: ModeSpec = ModeSpec::scalar(
    "Solution",
    ModeKind::F,
    20,
    "Solution.dblHour",
    "DSolution.pas:400",
    ModeEffect::Pure,
);
/// `SolutionV(1)` — the incidence matrix as flat `(row, col, value)` integer
/// triples; a single `0` until `CalcIncMatrix` has run.
pub const SOLUTION_INC_MATRIX: ModeSpec = ModeSpec::array(
    "Solution",
    1,
    "Solution.IncMatrix",
    "DSolution.pas:542",
    1,
    ModeEffect::Pure,
);
/// `SolutionV(3)` — the incidence matrix's row (branch) labels.
pub const SOLUTION_INC_MATRIX_ROWS: ModeSpec = ModeSpec::array(
    "Solution",
    3,
    "Solution.IncMatrixRows",
    "DSolution.pas:589",
    4,
    ModeEffect::Pure,
);
/// `SolutionV(4)` — the incidence matrix's column (bus) labels.
pub const SOLUTION_INC_MATRIX_COLS: ModeSpec = ModeSpec::array(
    "Solution",
    4,
    "Solution.IncMatrixCols",
    "DSolution.pas:609",
    4,
    ModeEffect::Pure,
);
/// `SolutionV(5)` — the Laplacian as flat `(row, col, value)` integer triples.
pub const SOLUTION_LAPLACIAN: ModeSpec = ModeSpec::array(
    "Solution",
    5,
    "Solution.Laplacian",
    "DSolution.pas:640",
    1,
    ModeEffect::Pure,
);

// -- PDElements (DPDELements.pas) -------------------------------------------

/// `PDElementsI(3)` — 1 when the active PD element is a shunt.
pub const PD_ELEMENTS_IS_SHUNT: ModeSpec = ModeSpec::scalar(
    "PDElements",
    ModeKind::I,
    3,
    "PDElements.IsShunt",
    "DPDELements.pas:61",
    ModeEffect::Pure,
);
/// `PDElementsI(4)` — customers served directly by the element.
pub const PD_ELEMENTS_NUM_CUSTOMERS: ModeSpec = ModeSpec::scalar(
    "PDElements",
    ModeKind::I,
    4,
    "PDElements.NumCustomers",
    "DPDELements.pas:70",
    ModeEffect::Pure,
);
/// `PDElementsI(5)` — customers downstream of the element.
pub const PD_ELEMENTS_TOTAL_CUSTOMERS: ModeSpec = ModeSpec::scalar(
    "PDElements",
    ModeKind::I,
    5,
    "PDElements.TotalCustomers",
    "DPDELements.pas:79",
    ModeEffect::Pure,
);
/// `PDElementsI(6)` — the parent PD element's class index.
pub const PD_ELEMENTS_PARENT_PD_ELEMENT: ModeSpec = ModeSpec::scalar(
    "PDElements",
    ModeKind::I,
    6,
    "PDElements.ParentPDElement",
    "DPDELements.pas:88",
    ModeEffect::Impure(
        "sets ActiveCktElement to the parent PD element when there is one \
         (DPDELements.pas:93-95) — every later element read sees the parent until the caller \
         re-selects",
    ),
);
/// `PDElementsI(7)` — the element's `FromTerminal`.
pub const PD_ELEMENTS_FROM_TERMINAL: ModeSpec = ModeSpec::scalar(
    "PDElements",
    ModeKind::I,
    7,
    "PDElements.FromTerminal",
    "DPDELements.pas:101",
    ModeEffect::Pure,
);
/// `PDElementsI(8)` — the element's `BranchSectionID`.
pub const PD_ELEMENTS_SECTION_ID: ModeSpec = ModeSpec::scalar(
    "PDElements",
    ModeKind::I,
    8,
    "PDElements.SectionID",
    "DPDELements.pas:110",
    ModeEffect::Pure,
);
/// `PDElementsF(0)` — the element's fault rate (read; the **write** arm `F:1` is
/// deliberately absent — [`EXCLUDED_WRITE_MODES`]).
pub const PD_ELEMENTS_FAULT_RATE: ModeSpec = ModeSpec::scalar(
    "PDElements",
    ModeKind::F,
    0,
    "PDElements.FaultRate",
    "DPDELements.pas:133",
    ModeEffect::Pure,
);
/// `PDElementsF(2)` — percent of faults that are permanent (read; the **write**
/// arm `F:3` is deliberately absent — [`EXCLUDED_WRITE_MODES`]).
pub const PD_ELEMENTS_PCT_PERMANENT: ModeSpec = ModeSpec::scalar(
    "PDElements",
    ModeKind::F,
    2,
    "PDElements.PctPermanent",
    "DPDELements.pas:152",
    ModeEffect::Pure,
);
/// `PDElementsF(4)` — the element's `BranchFltRate`.
pub const PD_ELEMENTS_LAMBDA: ModeSpec = ModeSpec::scalar(
    "PDElements",
    ModeKind::F,
    4,
    "PDElements.Lambda",
    "DPDELements.pas:171",
    ModeEffect::Pure,
);
/// `PDElementsF(5)` — the accumulated branch fault rate.
pub const PD_ELEMENTS_ACCUMULATED_L: ModeSpec = ModeSpec::scalar(
    "PDElements",
    ModeKind::F,
    5,
    "PDElements.AccumulatedL",
    "DPDELements.pas:181",
    ModeEffect::Pure,
);
/// `PDElementsF(6)` — hours to repair.
pub const PD_ELEMENTS_REPAIR_TIME: ModeSpec = ModeSpec::scalar(
    "PDElements",
    ModeKind::F,
    6,
    "PDElements.RepairTime",
    "DPDELements.pas:191",
    ModeEffect::Pure,
);
/// `PDElementsF(7)` — accumulated miles downstream.
pub const PD_ELEMENTS_TOTAL_MILES: ModeSpec = ModeSpec::scalar(
    "PDElements",
    ModeKind::F,
    7,
    "PDElements.TotalMiles",
    "DPDELements.pas:201",
    ModeEffect::Pure,
);

/// Every mode WP-G1 reads through the r4133 bridge — 96 rows over the seven
/// families the plan's surface sub-steps touch (`GOLDEN_REBASE_PLAN.md` WP-G1).
/// The set was measured `Served` on the vendored DLL by the G1.0 probe
/// (2026-09-04) with zero misses, and the acceptance test
/// `crates/dss-epri/tests/modes.rs` re-proves that on every run.
///
/// Each row also has a typed accessor on [`crate::dss::Engine`] that takes the
/// row **by reference**, so no mode number is ever written twice.
pub const WP_G1_MODES: &[&ModeSpec] = &[
    &CKT_ELEMENT_NUM_TERMINALS,
    &CKT_ELEMENT_NUM_CONDUCTORS,
    &CKT_ELEMENT_NUM_PHASES,
    &CKT_ELEMENT_HAS_SWITCH_CONTROL,
    &CKT_ELEMENT_HAS_VOLT_CONTROL,
    &CKT_ELEMENT_NUM_CONTROLS,
    &CKT_ELEMENT_OCP_DEV_INDEX,
    &CKT_ELEMENT_OCP_DEV_TYPE,
    &CKT_ELEMENT_HAS_OCP_DEVICE,
    &CKT_ELEMENT_ENERGY_METER,
    &CKT_ELEMENT_PHASE_LOSSES,
    &CKT_ELEMENT_SEQ_VOLTAGES,
    &CKT_ELEMENT_SEQ_CURRENTS,
    &CKT_ELEMENT_SEQ_POWERS,
    &CKT_ELEMENT_RESIDUALS,
    &CKT_ELEMENT_CPLX_SEQ_VOLTAGES,
    &CKT_ELEMENT_CPLX_SEQ_CURRENTS,
    &CKT_ELEMENT_NODE_ORDER,
    &CKT_ELEMENT_CURRENTS_MAG_ANG,
    &CKT_ELEMENT_VOLTAGES_MAG_ANG,
    &CKT_ELEMENT_TOTAL_POWERS,
    &BUS_DISTANCE,
    &BUS_SEQ_VOLTAGES,
    &BUS_VOC,
    &BUS_ISC,
    &BUS_PU_VOLTAGES,
    &BUS_ZSC_MATRIX,
    &BUS_ZSC1,
    &BUS_ZSC0,
    &BUS_YSC_MATRIX,
    &BUS_CPLX_SEQ_VOLTAGES,
    &BUS_VLL,
    &BUS_PU_VLL,
    &BUS_VMAG_ANGLE,
    &BUS_PU_VMAG_ANGLE,
    &BUS_ALL_PCE_AT_BUS,
    &BUS_ALL_PDE_AT_BUS,
    &CIRCUIT_LOSSES,
    &CIRCUIT_LINE_LOSSES,
    &CIRCUIT_SUBSTATION_LOSSES,
    &CIRCUIT_TOTAL_POWER,
    &CIRCUIT_ALL_ELEMENT_LOSSES,
    &CIRCUIT_ALL_BUS_MAG_PU,
    &CIRCUIT_ALL_BUS_DISTANCES,
    &CIRCUIT_ALL_NODE_DISTANCES,
    &METERS_TOTAL_CUSTOMERS,
    &METERS_NUM_SECTIONS,
    &METERS_OCP_DEVICE_TYPE,
    &METERS_NUM_SECTION_CUSTOMERS,
    &METERS_NUM_SECTION_BRANCHES,
    &METERS_SECT_SEQ_IDX,
    &METERS_SECT_TOTAL_CUST,
    &METERS_SAIFI,
    &METERS_SAIFI_KW,
    &METERS_SAIDI,
    &METERS_CUST_INTERRUPTS,
    &METERS_AVG_REPAIR_TIME,
    &METERS_FAULT_RATE_X_REPAIR_HRS,
    &METERS_SUM_BRANCH_FLT_RATES,
    &METERS_TOTALS,
    &METERS_CALC_CURRENT,
    &METERS_ALLOC_FACTORS,
    &TOPOLOGY_NUM_LOOPS,
    &TOPOLOGY_NUM_ISOLATED_BRANCHES,
    &TOPOLOGY_NUM_ISOLATED_LOADS,
    &TOPOLOGY_ALL_LOOPED_PAIRS,
    &TOPOLOGY_ALL_ISOLATED_BRANCHES,
    &TOPOLOGY_ALL_ISOLATED_LOADS,
    &SOLUTION_MODE,
    &SOLUTION_HOUR,
    &SOLUTION_YEAR,
    &SOLUTION_ITERATIONS,
    &SOLUTION_CONTROL_ITERATIONS,
    &SOLUTION_SYSTEM_Y_CHANGED,
    &SOLUTION_TOTAL_ITERATIONS,
    &SOLUTION_MOST_ITERATIONS_DONE,
    &SOLUTION_CONTROL_ACTIONS_DONE,
    &SOLUTION_SECONDS,
    &SOLUTION_LOAD_MULT,
    &SOLUTION_DBL_HOUR,
    &SOLUTION_INC_MATRIX,
    &SOLUTION_INC_MATRIX_ROWS,
    &SOLUTION_INC_MATRIX_COLS,
    &SOLUTION_LAPLACIAN,
    &PD_ELEMENTS_IS_SHUNT,
    &PD_ELEMENTS_NUM_CUSTOMERS,
    &PD_ELEMENTS_TOTAL_CUSTOMERS,
    &PD_ELEMENTS_PARENT_PD_ELEMENT,
    &PD_ELEMENTS_FROM_TERMINAL,
    &PD_ELEMENTS_SECTION_ID,
    &PD_ELEMENTS_FAULT_RATE,
    &PD_ELEMENTS_PCT_PERMANENT,
    &PD_ELEMENTS_LAMBDA,
    &PD_ELEMENTS_ACCUMULATED_L,
    &PD_ELEMENTS_REPAIR_TIME,
    &PD_ELEMENTS_TOTAL_MILES,
];

/// DDLL `case` arms a naive reading of WP-G1's mode *ranges* would include but
/// that this bridge must never drive: they are **write** arms, and the generic
/// reader supplies a neutral argument, so calling one would store `0.0` into the
/// active PD element instead of reading anything.
///
/// `tmp/g10/spec.md` §2.B B4 listed `PDElements F:0..7` as a range; the vendored
/// r4133 source shows modes 1 and 3 are the setters paired with the readers at 0
/// and 2, which [`WP_G1_MODES`] carries instead. Recorded here — and enforced by
/// a unit test — so the omission is a decision, not a gap.
pub const EXCLUDED_WRITE_MODES: &[(&str, ModeKind, i32, &str)] = &[
    (
        "PDElements",
        ModeKind::F,
        1,
        "PDElements.FaultRate WRITE — DPDELements.pas:143 assigns ActivePDElement.FaultRate := arg; \
         the reader is F:0",
    ),
    (
        "PDElements",
        ModeKind::F,
        3,
        "PDElements.PctPermanent WRITE — DPDELements.pas:162 assigns ActivePDElement.PctPerm := arg; \
         the reader is F:2",
    ),
];

/// The [`WP_G1_MODES`] row called `name` (`"Family.Property"`), or `None`.
/// Case-sensitive: the names are the table's own spellings.
pub fn wp_g1_mode(name: &str) -> Option<&'static ModeSpec> {
    WP_G1_MODES.iter().copied().find(|m| m.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The register refuses **offline** — no DLL, no FFI, no probe: this is the
    /// enforcement of the do-not-call rule, not a convention.
    #[test]
    fn do_not_call_refuses_the_two_unsafe_modes_without_touching_the_dll() {
        let sol = check_callable("Solution", ModeKind::V, 2).expect("Solution V:2 must refuse");
        assert!(
            matches!(sol, ModeStatus::DoNotCall(w) if w.contains("BusLevels")),
            "{sol}"
        );
        let bus = check_callable("bus", ModeKind::V, 17).expect("Bus V:17 must refuse");
        assert!(
            matches!(bus, ModeStatus::DoNotCall(w) if w.contains("ZSC012Matrix")),
            "{bus}"
        );
        // Non-vacuity: the neighbours of both rows are NOT refused — the
        // register is three-part (family, kind, mode), not a family-wide ban.
        assert!(check_callable("Solution", ModeKind::V, 1).is_none());
        assert!(check_callable("Solution", ModeKind::I, 2).is_none());
        assert!(check_callable("Bus", ModeKind::V, 16).is_none());
        assert!(check_callable("Bus", ModeKind::V, 18).is_none());
        assert!(check_callable("CktElement", ModeKind::V, 2).is_none());
    }

    #[test]
    fn i_and_f_sentinels_are_minus_one() {
        assert_eq!(
            classify_i(-1),
            ModeStatus::UnknownMode {
                sentinel: "-1".into()
            }
        );
        assert!(classify_i(0).is_served());
        assert!(classify_i(3).is_served());
        assert_eq!(
            classify_f(-1.0),
            ModeStatus::UnknownMode {
                sentinel: "-1".into()
            }
        );
        // The `F` default before any `case` arm runs is 0.0, not the sentinel.
        assert!(classify_f(0.0).is_served());
        assert!(classify_f(60.0).is_served());
    }

    /// The five WP-G1 `S` spellings, pinned verbatim against the DDLL `else`
    /// branches (`DBus.pas:217` `non`, `DSolution.pas:513` `paratemer`, …).
    #[test]
    fn s_sentinels_are_per_family_literals() {
        assert_eq!(s_sentinel("Bus"), Some("Error, Parameter non recognized"));
        assert_eq!(
            s_sentinel("solution"),
            Some("Error, paratemer not recognized")
        );
        assert_eq!(
            s_sentinel("Meters"),
            Some("Error, Parameter not recognized")
        );
        assert_eq!(
            s_sentinel("Circuit"),
            Some("Error, parameter not recognized")
        );
        assert_eq!(s_sentinel("Topology"), Some("Error, parameter not valid"));
        assert_eq!(s_sentinel("CktElement"), Some("Error"));
        assert_eq!(s_sentinel("Monitors"), None, "no row measured for Monitors");

        // Each family is classified against its OWN literal: a sibling's
        // spelling must NOT read as a miss.
        assert!(classify_s("Bus", "Error, Parameter non recognized").is_unknown_mode());
        assert!(
            classify_s("Bus", "Error, parameter not recognized").is_served(),
            "Circuit's spelling must not classify Bus as a miss"
        );
        assert!(classify_s("Solution", "Error, paratemer not recognized").is_unknown_mode());
        assert!(classify_s("Solution", "Error, parameter not recognized").is_served());
        // A real reading is served.
        assert!(classify_s("Circuit", "captest").is_served());
    }

    /// The `V` rule is *containment*, because `DBusV`'s `else` branch appends to
    /// an uncleared DLL-global buffer (`DBus.pas:899-903`); the concatenated
    /// shape below is the literal 2026-09-04 measurement.
    #[test]
    fn v_sentinel_is_matched_by_containment_not_equality() {
        let concat = vec!["Error, parameter not recognizedCommand not recognized".to_string()];
        assert!(classify_v(4, &concat).is_unknown_mode());
        assert!(classify_v(4, &["Error, parameter not recognized".to_string()]).is_unknown_mode());
        assert!(classify_v(4, &["Error, parameter not valid;".to_string()]).is_unknown_mode());
        // `ActiveClassV` reports its own tag instead of a phrase.
        assert_eq!(
            classify_v(-1, &[]),
            ModeStatus::UnknownMode {
                sentinel: "myType=-1".into()
            }
        );
        // A served string array (and every non-string tag) is served.
        assert!(classify_v(4, &["Line.670671".to_string()]).is_served());
        assert!(classify_v(2, &[]).is_served());
        assert!(classify_v(3, &[]).is_served());
        // The one family with no `V` sentinel at all is declared, not silent.
        assert!(v_sentinel_undetectable("capacitors"));
        assert!(!v_sentinel_undetectable("Bus"));
    }

    /// The table is the single home of every WP-G1 mode number, so its own
    /// shape is pinned offline: one row per `(family, kind, mode)`, one row per
    /// name, `Family.Property` naming, and a declared `myType` on exactly the
    /// `V` rows (the tag [`crate::dss::Engine::read_mode`] validates).
    #[test]
    fn the_wp_g1_mode_table_is_internally_consistent() {
        assert_eq!(
            WP_G1_MODES.len(),
            96,
            "WP-G1 mode count changed — update the count, the record and TESTING.md"
        );
        let mut names: Vec<&str> = Vec::new();
        let mut triples: Vec<(String, ModeKind, i32)> = Vec::new();
        for m in WP_G1_MODES {
            assert!(
                m.name.starts_with(m.family) && m.name.as_bytes()[m.family.len()] == b'.',
                "{m} — name must be `Family.Property`"
            );
            assert!(
                m.pas.starts_with('D') && m.pas.contains(".pas:"),
                "{m} — pas must cite the DDLL unit and line"
            );
            assert!(m.mode >= 0, "{m} — negative mode index");
            match m.kind {
                ModeKind::V => assert!(
                    matches!(m.v_type, Some(1..=4)),
                    "{m} — a V row must declare the myType tag its case arm assigns"
                ),
                _ => assert_eq!(m.v_type, None, "{m} — only V rows carry a myType tag"),
            }
            if let ModeEffect::Impure(why) = m.effect {
                assert!(
                    why.contains(".pas") || why.contains("TotalizeMeters"),
                    "{m} — an Impure row must say what moves, with its citation"
                );
            }
            assert!(!names.contains(&m.name), "duplicate row name {}", m.name);
            names.push(m.name);
            let t = (m.family.to_ascii_lowercase(), m.kind, m.mode);
            assert!(!triples.contains(&t), "duplicate triple for {m}");
            triples.push(t);
        }
        // The seven families WP-G1's surface sub-steps touch, spelled exactly as
        // `crate::families::REGISTRY` spells them (the lookup is
        // case-insensitive, but a typo here would silently probe nothing).
        let mut fams: Vec<&str> = WP_G1_MODES.iter().map(|m| m.family).collect();
        fams.sort_unstable();
        fams.dedup();
        assert_eq!(
            fams,
            vec![
                "Bus",
                "Circuit",
                "CktElement",
                "Meters",
                "PDElements",
                "Solution",
                "Topology"
            ]
        );
        // The lookup helper agrees with the table.
        assert_eq!(
            wp_g1_mode("Solution.Laplacian"),
            Some(&SOLUTION_LAPLACIAN as &ModeSpec)
        );
        assert_eq!(wp_g1_mode("Solution.laplacian"), None, "lookup is exact");
        assert_eq!(wp_g1_mode("Bus.BusLevels"), None);
    }

    /// No table row may name a mode the bridge refuses to call, and none may
    /// name one of the two `PDElements` **write** arms the spec's `F:0..7` range
    /// shorthand swept up ([`EXCLUDED_WRITE_MODES`]): the generic reader drives
    /// a mode with a neutral argument, so a write arm would store `0.0` into the
    /// active element.
    #[test]
    fn the_wp_g1_mode_table_holds_only_callable_getters() {
        for m in WP_G1_MODES {
            assert!(
                check_callable(m.family, m.kind, m.mode).is_none(),
                "{m} is on the DO_NOT_CALL register"
            );
            for (fam, kind, mode, why) in EXCLUDED_WRITE_MODES {
                assert!(
                    !(m.kind == *kind && m.mode == *mode && m.family.eq_ignore_ascii_case(fam)),
                    "{m} is a write arm: {why}"
                );
            }
        }
        // Non-vacuity: the excluded rows really are the neighbours of table rows
        // in the same family and shape, so the check above is not comparing
        // against an empty or unrelated set.
        assert_eq!(EXCLUDED_WRITE_MODES.len(), 2);
        assert!(EXCLUDED_WRITE_MODES.iter().all(|(f, k, ..)| {
            *f == PD_ELEMENTS_FAULT_RATE.family && *k == PD_ELEMENTS_FAULT_RATE.kind
        }));
        assert_eq!(PD_ELEMENTS_FAULT_RATE.mode, 0);
        assert_eq!(PD_ELEMENTS_PCT_PERMANENT.mode, 2);
    }

    #[test]
    fn mode_kind_tags_match_the_ffi_dispatch_spelling() {
        assert_eq!(ModeKind::I.as_str(), "i");
        assert_eq!(ModeKind::F.as_str(), "f");
        assert_eq!(ModeKind::S.as_str(), "s");
        assert_eq!(ModeKind::V.as_str(), "v");
    }
}
