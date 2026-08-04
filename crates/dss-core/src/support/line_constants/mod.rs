//! Carson line-constants engine, port of `General/LineConstants.pas`
//! (`TLineConstants`). Computes the series impedance `Z` (ohms/m) and shunt
//! capacitance admittance `Yc` (siemens/m) of a multi-conductor line from its
//! geometry via Carson's equations, with the earth-return term selected by the
//! `EarthModel` (Carson / FullCarson / Deri). Frequency is a parameter of
//! [`LineConstants::calc`]: power flow uses the base frequency; harmonics
//! re-`calc` per harmonic (WP7.6).
//!
//! Indices are 0-based here (Pascal used 1..FNumConds); the ground-node
//! convention is unaffected — these are conductor indices.

#[cfg(test)]
mod tests;

use crate::compat;
use crate::support::cmatrix::CMatrix;
use crate::support::line_units::LineUnits;
use crate::support::mathutil::{bessel_i0, bessel_i1};
use num_complex::Complex64;

pub mod cable;
pub mod cn;
pub mod oh;
pub mod ts;
pub use cn::CnLineConstants;
pub use oh::OhLineConstants;
pub use ts::TsLineConstants;

/// Earth-model codes (Pascal `DSSGlobals` constants).
pub const SIMPLE_CARSON: i32 = 1;
pub const FULL_CARSON: i32 = 2;
pub const DERI: i32 = 3;

/// Which Pascal `TLineConstants` subclass this engine reproduces. Selects the
/// `Calc`/`ConductorsInSameSpace` behavior. dss_capi 0.15.x **merged** the two
/// separate `TCNLineConstants`/`TTSLineConstants` classes into a single
/// `TCableConstants` (CableConstants.pas): the CN-vs-TS choice moved from the
/// engine kind to a per-conductor `FCondType[i]` array ([`ConductorType`]), so
/// one engine can carry mixed wire/CN/TS conductors (Kersting mixed-conductor
/// model). The overhead Carson model is still the base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineConstantsKind {
    /// `TOHLineConstants` — overhead line (the base Carson model).
    Overhead,
    /// `TCableConstants` — coaxial cable (concentric-neutral and/or tape-shield
    /// conductors, selected per-conductor by [`LineConstants::set_cond_type`]).
    Cable,
}

/// Pascal `TConductorType` (CableConstants.pas): the per-conductor kind inside a
/// merged `TCableConstants` engine. `INVALID`/`Bare` conductors contribute no
/// CN/TS cable branch (a plain overhead wire buried among cables). Ordinals match
/// the Pascal enum (INVALID=0, CN=1, TS=2, Bare=3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConductorType {
    /// `INVALID` — never assigned a cable type (Allocmem zero); no cable branch.
    #[default]
    Invalid,
    /// `CN` — concentric-neutral cable conductor.
    Cn,
    /// `TS` — tape-shield cable conductor.
    Ts,
    /// `Bare` — bare wire conductor; no cable branch (defined for 1:1 parity).
    Bare,
}

// Pascal `LineConstants` unit constants.
//
// TODO(compat): these reproduce the upstream truncated literals exactly —
// `mu0` and `Twopi` are short of the precise values, and `Twopi` is used as a
// distinct quantity from `2*pi` (the FullCarson/Zint earth terms use the full
// `PI`). Goldens pin them; the clean fix (precise constants) is a re-baseline,
// not a lane flip.
//
// **Stage F.3z measured both halves separately rather than escaping the pair as
// one category** (`LineConstants.pas:128-129`; r4133 `:152-153`, so both gating
// oracles carry them):
//
// * `mu0 = 12.56637e-7` is **4.889e-8** relative below `4πe-7`, and it scales
//   the whole series impedance. Flipping it fails **35 of the 520** gated
//   corpus cases, 33 unit tests (the `1e-8`-toleranced Carson/cable reference
//   matrices) and the `dump_line_geo` + `show_lineconstants` goldens. Nothing
//   field-scoped survives that.
// * `Twopi = 6.283185307` is only **2.858e-11** relative below `TAU`, and it has
//   exactly one consumer (`LFactor := Cmplx(0, Fw·mu0/twopi)`, `:184`). Flipping
//   it alone fails **one** corpus case —
//   `IEEETestCases/4Bus-YYD/YYD-Master-step1.DSS` on the r4133 channel, entry 12
//   at |diff| 1.380e-4 against an allowed 1.338e-4, i.e. **1.031x** its floor,
//   the same deck the dense-inverse row (F.3f) already found sitting on its
//   calibration boundary — plus the DERI bit pin. Still an oracle floor the
//   default lane must meet, so it escapes; but it is 1.03x, not a category.
//
// The physically meaningful group is `mu0/twopi`, which the exact constants make
// exactly `2e-7` (both truncated: 1.99999990e-7). So a correct fix flips **both**
// and lands with `mu0`'s 35-case cost — an UPGRADE-rung re-baseline against a
// re-captured oracle, which the parity lane may never do.
const E0: f64 = 8.854e-12; // dielectric constant F/m
const MU0: f64 = 12.56637e-7; // hy/m
#[allow(clippy::approx_constant)] // truncated upstream `Twopi`, not `TAU` — see above
const TWOPI: f64 = 6.283185307;

#[inline]
fn cmplx(re: f64, im: f64) -> Complex64 {
    Complex64::new(re, im)
}

// FPC RTL `ucomplex.pp` complex primitives. FPC uses the algebraic
// Numerical-Recipes `csqrt`, the naive `sqrt(re²+im²)` modulus (`Cabs`/`cmod`,
// NOT `hypot`), and `cln = ln(cmod) + j·arctan2`; `num_complex` uses `hypot`
// for `.norm()`, `ln(norm) + j·arg` for `.ln()`, and the *polar* form
// `from_polar(√r, θ/2)` for `.sqrt()`. The port originally carried all three
// hand-rolled, on the assumption that each was a less-precise Pascal wart kept
// only for bit-parity.
//
// **Stage F.3y measured that assumption against a 60-digit `mpmath` reference
// (20 000 operands, |z| sweep 1e-150…1e150, the F.3e protocol), and it holds
// for none of the three.** The row resolved with *no lane split at all* — two
// of the primitives are the crate's, and the third is FPC's on merit:
//
// | kernel | mean ULP err | worst ULP err |
// |---|---|---|
// | `csqrt` FPC algebraic (NR), real part | **0.339** | **1.82** |
// | `csqrt` `num_complex` polar, real part | 2.413 | **12818** |
// | `csqrt` FPC / `num_complex`, imag part | 0.339 | 1.73 / 2.44 |
// | `cabs` naive vs `.norm()` | 0.292 / 0.292 | 1.11 / 1.11 — **bit-identical** |
//
// 1. **`Cabs` and `Cln` are not warts — they are already `.norm()`/`.ln()`.**
//    Over the whole range where `re²+im²` is representable the naive modulus is
//    **bit-for-bit** `f64::hypot` here (0 disagreements in 20 000 samples), and
//    since `cln`'s imaginary part is literally `im.atan2(re)` — the same
//    expression `Complex::arg` evaluates — `cln_fpc` was bit-identical to
//    `.ln()` as well. What the two forms do *not* share is behaviour outside
//    that range: `re²+im²` over/underflows, so the naive modulus collapses to
//    `inf`/`0` (measured at `(1.5e154, 2.5e154)` and `(1e-170, 1e-170)`) where
//    `hypot` stays exact. So the crate's forms are equal where anything is
//    gated and strictly more robust where nothing is: they are used
//    unconditionally, in **both** lanes, and the compat marker that stood
//    here is closed.
// 2. **`Csqrt` stays FPC's, and *not* for parity — the algebraic form is the
//    better kernel.** `num_complex`'s polar `sqrt` routes through `atan2` and
//    `cos`, which cancel as `θ → ±π` (near the negative real axis), costing it
//    7× the mean error and **7000×** the worst-case error of the branch-split
//    Numerical-Recipes form. Flipping it would make the *product* lane strictly
//    less accurate for nothing — the exact verdict F.3e reached for
//    `compat::cdiv` (Smith's division), and the same IV.1 principle: legitimate
//    numerics stay shared by both lanes. `tests::csqrt_algebraic_beats_the_
//    polar_form` pins it with literals so it cannot rot back into folklore.
//
// Proven bit-for-bit against the x86_64 FPC `ucomplex` RTL, which is why the
// DERI/cable earth terms and `Line`'s `DoLongLine` all still route through
// `csqrt_fpc`.

/// FPC `ucomplex` `csqrt` — the Numerical-Recipes stable square root
/// (`root = √(½(|re|+|z|))`, the other component `= im/(2·root)`), branch-split
/// on the signs so the robust component is the directly-rooted one.
///
/// Retained over `num_complex`'s `.sqrt()` on measured accuracy, not on parity
/// — see the module-level table above.
///
/// `pub(crate)` so the Line `DoLongLine` port (`elements/pd/line/solve.rs`)
/// reuses the identical, RTL-proven `csqrt` rather than duplicating it.
#[inline]
pub(crate) fn csqrt_fpc(z: Complex64) -> Complex64 {
    if z.re != 0.0 || z.im != 0.0 {
        // `.norm()` rather than FPC's naive `cmod`: bit-identical wherever
        // `re²+im²` is representable, and finite beyond it (see above).
        let root = (0.5 * (z.re.abs() + z.norm())).sqrt();
        let q = z.im / (2.0 * root);
        if z.re >= 0.0 {
            Complex64::new(root, q)
        } else if z.im < 0.0 {
            Complex64::new(-q, -root)
        } else {
            Complex64::new(q, root)
        }
    } else {
        z
    }
}

/// Per-conductor cable extension — the merged `TCableConstants` subclass arrays
/// gathered into one struct. Present (`Some`) on every conductor of a cable
/// engine, absent (`None`) on an overhead conductor: this `Option` is the typed
/// form of the Pascal "subclass arrays empty on overhead lines" trick. dss_capi
/// 0.15.x holds the CN and TS data together and selects per conductor via
/// `cond_type`. Units are meters / per-meter; `Default` gives the Pascal
/// `Allocmem` zeros (`FCondType=INVALID`, `semiconLayer=false`, `kStrand=0`).
#[derive(Debug, Clone, Default)]
struct CableData {
    eps_r: f64,     // relative permittivity of insulation
    ins_layer: f64, // m
    dia_ins: f64,   // m, diameter over insulation
    dia_cable: f64, // m, diameter over cable
    // Per-conductor cable kind (`FCondType`) and semicon-layer flag
    // (`semiconLayer`, CableConstants.pas).
    cond_type: ConductorType,
    semicon_layer: bool,
    // Concentric-neutral strand data (CN conductors); zeroed for TS/bare.
    k_strand: i32,
    dia_strand: f64, // m
    gmr_strand: f64, // m
    rstrand: f64,    // ohms/m
    // Tape-shield data (TS conductors); zeroed for CN/bare.
    dia_shield: f64, // m
    tape_layer: f64, // m
    tape_lap: f64,   // percent
}

/// One conductor of a `TLineConstants` geometry: coordinates plus electrical
/// parameters, stored internally in meters / ohms-per-meter (the setters convert
/// from the supplied units). `cable` carries the cable-subclass data on a
/// `TCableConstants` engine and is `None` for overhead.
#[derive(Debug, Clone)]
struct Conductor {
    x: f64,
    y: f64,
    rdc: f64,        // ohms/m
    rac: f64,        // ohms/m
    gmr: f64,        // m
    radius: f64,     // m
    cap_radius: f64, // m; <0 ⇒ defaults to radius
    cable: Option<CableData>,
}

/// Pascal `TLineConstants`: the per-conductor geometry/parameters plus the
/// computed `Z`/`Yc` matrices.
#[derive(Clone)]
pub struct LineConstants {
    kind: LineConstantsKind,
    num_conds: usize,
    nphases: usize,

    // One entry per conductor (the former ~20 parallel `Vec`s). The cable
    // subclass arrays live in each conductor's `cable: Option<CableData>`.
    cond: Vec<Conductor>,

    fz_matrix: CMatrix,  // ohms/m
    fyc_matrix: CMatrix, // siemens/m  (jwC)

    fz_reduced: Option<CMatrix>, // exist only after a Kron reduction
    fyc_reduced: Option<CMatrix>,

    ffrequency: f64,    // frequency for which impedances are computed
    fw: f64,            // 2*pi*f (truncated twopi)
    frho_earth: f64,    // ohm-m
    fme: Complex64,     // factor for earth impedance
    frho_changed: bool, // also flags eps_r_medium / height_offset / user_height_unit / equiv changes

    // dss_capi 0.15.x `TLineConstants` additions (SVN r3913-era line/conductor
    // rework, LineConstants.pas). Defaults preserve the 0.14.5 numerics:
    // `equivalent_spacing=false`, `eps_r_medium=1.0` (so `E0*1.0 == E0` exactly),
    // `height_offset=0.0`. UPGRADE_PLAN.md WP-U1.4 rows B3/C1.
    equivalent_spacing: bool,
    eps_r_medium: f64, // relative permittivity of the surrounding medium (unit-less)
    height_offset: f64, // stored in meters
    user_height_unit: i32, // LineUnits code the height_offset is reported in
    eq_dist_ph_ph: f64, // equivalent phase-phase distance (meters)
    eq_dist_ph_n: f64, // equivalent phase-neutral distance (meters)
    avg_phase_height: f64, // meters
    avg_neutral_height: f64, // meters
}

impl LineConstants {
    /// `TOHLineConstants.Create(NumConductors)` — the overhead base.
    pub fn new(num_conductors: usize) -> Self {
        Self::with_kind(num_conductors, LineConstantsKind::Overhead)
    }

    /// `TCableConstants.Create(NumConductors)` — merged cable engine (no
    /// per-conductor type set yet; `FCondType` all `INVALID`, `semiconLayer`
    /// all `false`, per Pascal `Allocmem`). Callers assign each conductor's kind
    /// via [`Self::set_cond_type`] / [`Self::set_semicon_layer`].
    pub fn new_cable(num_conductors: usize) -> Self {
        Self::with_kind(num_conductors, LineConstantsKind::Cable)
    }

    /// Convenience: a cable engine with every conductor preset to
    /// concentric-neutral (`FCondType=CN`, `semiconLayer=true` — the `CNData`
    /// default). Mirrors a pure-CN geometry; used by the engine unit tests.
    pub fn new_cn(num_conductors: usize) -> Self {
        let mut lc = Self::with_kind(num_conductors, LineConstantsKind::Cable);
        for i in 0..num_conductors {
            let c = lc.cable_mut(i);
            c.cond_type = ConductorType::Cn;
            c.semicon_layer = true;
        }
        lc
    }

    /// Convenience: a cable engine with every conductor preset to tape-shield
    /// (`FCondType=TS`). Mirrors a pure-TS geometry; used by the unit tests.
    pub fn new_ts(num_conductors: usize) -> Self {
        let mut lc = Self::with_kind(num_conductors, LineConstantsKind::Cable);
        for i in 0..num_conductors {
            lc.cable_mut(i).cond_type = ConductorType::Ts;
        }
        lc
    }

    fn with_kind(num_conductors: usize, kind: LineConstantsKind) -> Self {
        let n = num_conductors;
        // The merged cable engine `Allocmem`s all extra arrays in the
        // constructor (`Some(CableData::default())`, all zeros); the overhead
        // base leaves them empty (`None`, never indexed in its `Calc`).
        let cable = matches!(kind, LineConstantsKind::Cable);
        // Pascal initializes rdc/gmr/radius/capradius to "not set" (-1.0);
        // rac/x/y are left zero by Allocmem.
        let cond = (0..n)
            .map(|_| Conductor {
                x: 0.0,
                y: 0.0,
                rdc: -1.0,
                rac: 0.0,
                gmr: -1.0,
                radius: -1.0,
                cap_radius: -1.0,
                cable: if cable {
                    Some(CableData::default())
                } else {
                    None
                },
            })
            .collect();
        LineConstants {
            kind,
            num_conds: n,
            nphases: n,
            cond,
            fz_matrix: CMatrix::new(n),
            fyc_matrix: CMatrix::new(n),
            fz_reduced: None,
            fyc_reduced: None,
            ffrequency: -1.0, // not computed
            fw: 0.0,
            frho_earth: 100.0, // default value
            fme: Complex64::ZERO,
            frho_changed: true, // using for both rho and epsilon_r
            equivalent_spacing: false,
            eps_r_medium: 1.0,   // default value should be 1.0
            height_offset: 0.0,  // default value should be 0.0
            user_height_unit: 4, // UNITS_M
            eq_dist_ph_ph: 0.0,
            eq_dist_ph_n: 0.0,
            avg_phase_height: 0.0,
            avg_neutral_height: 0.0,
        }
    }

    /// Which subclass this engine reproduces.
    pub fn kind(&self) -> LineConstantsKind {
        self.kind
    }

    /// Borrow conductor `i`'s cable data. Panics on an overhead engine, matching
    /// the Pascal precondition that the cable setters/`Calc` run only on a
    /// `TCableConstants` engine (the former empty-array index-out-of-bounds).
    #[inline]
    fn cable(&self, i: usize) -> &CableData {
        self.cond[i]
            .cable
            .as_ref()
            .expect("cable data on a cable engine")
    }

    /// Mutable [`Self::cable`].
    #[inline]
    fn cable_mut(&mut self, i: usize) -> &mut CableData {
        self.cond[i]
            .cable
            .as_mut()
            .expect("cable data on a cable engine")
    }

    pub fn num_conductors(&self) -> usize {
        self.num_conds
    }

    pub fn nphases(&self) -> usize {
        self.nphases
    }

    /// `Nphases` write (`set_Nphases`).
    pub fn set_nphases(&mut self, value: usize) {
        self.nphases = value;
    }

    pub fn rho_earth(&self) -> f64 {
        self.frho_earth
    }

    // ---- per-conductor setters (units → meters / per-meter) ----
    // `units` is a Pascal `LineUnits` integer code. Indices are 0-based.

    pub fn set_x(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.cond[i].x = value * LineUnits::from_code(units).to_meters();
        }
    }

    pub fn set_y(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.cond[i].y = value * LineUnits::from_code(units).to_meters();
        }
    }

    pub fn set_rdc(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.cond[i].rdc = value * LineUnits::from_code(units).to_per_meter();
        }
    }

    pub fn set_rac(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.cond[i].rac = value * LineUnits::from_code(units).to_per_meter();
        }
    }

    /// `Set_GMR`: also defaults the radius to the equivalent round conductor
    /// when radius is still unset.
    pub fn set_gmr(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            let c = &mut self.cond[i];
            c.gmr = value * LineUnits::from_code(units).to_meters();
            if c.radius < 0.0 {
                c.radius = c.gmr / 0.7788; // equivalent round conductor
            }
        }
    }

    /// `Set_radius`: also defaults the GMR to the round-conductor value when GMR
    /// is still unset.
    pub fn set_radius(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            let c = &mut self.cond[i];
            c.radius = value * LineUnits::from_code(units).to_meters();
            if c.gmr < 0.0 {
                c.gmr = c.radius * 0.7788; // default to round conductor
            }
        }
    }

    pub fn set_capradius(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.cond[i].cap_radius = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `Set_Frequency`: side effects on `Fw` and the earth factor `Fme`.
    fn set_frequency(&mut self, value: f64) {
        self.ffrequency = value;
        self.fw = TWOPI * self.ffrequency;
        self.fme = csqrt_fpc(cmplx(0.0, self.fw * MU0 / self.frho_earth));
    }

    /// `Set_Frhoearth`.
    pub fn set_rho_earth(&mut self, value: f64) {
        if value != self.frho_earth {
            self.frho_changed = true;
        }
        self.frho_earth = value;
        if self.ffrequency >= 0.0 {
            self.fme = csqrt_fpc(cmplx(0.0, self.fw * MU0 / self.frho_earth));
        }
    }

    // ---- dss_capi 0.15.x additions (LineConstants.pas, SVN r3913-era) ----

    /// `SetEquivalentSpacing`: switch between the detailed per-conductor
    /// coordinate model and the equivalent-distance model. Flags `rhoChanged`
    /// so the next `z_matrix`/`yc_matrix` recomputes.
    pub fn set_equivalent_spacing(&mut self, value: bool) {
        if value == self.equivalent_spacing {
            return;
        }
        self.equivalent_spacing = value;
        self.frho_changed = true;
    }

    /// `SetEpsRMedium`: relative permittivity of the surrounding medium (used in
    /// the shunt-capacitance `Pfactor`). Default `1.0` preserves the 0.14.5
    /// numerics (`E0 * 1.0 == E0`).
    pub fn set_eps_r_medium(&mut self, value: f64) {
        if value == self.eps_r_medium {
            return;
        }
        self.frho_changed = true;
        self.eps_r_medium = value;
    }

    /// `GetEpsRMedium`.
    pub fn eps_r_medium(&self) -> f64 {
        self.eps_r_medium
    }

    /// `SetHeightOffset` (LineConstants.pas): `Value` is in the current
    /// `user_height_unit`; shifts every conductor's `FY` by the change. Stored
    /// internally in meters.
    pub fn set_height_offset(&mut self, value: f64) {
        let new_offset_m = value * LineUnits::from_code(self.user_height_unit).to_meters();
        if new_offset_m != self.height_offset {
            self.frho_changed = true;
        }
        // Remove old value from Y positions first (offset already in meters).
        for c in &mut self.cond {
            c.y -= self.height_offset;
        }
        self.height_offset = new_offset_m; // replace old value with new value
        // Add new value to Y positions.
        for c in &mut self.cond {
            c.y += self.height_offset;
        }
    }

    /// `GetHeightOffset`: the stored (meters) offset reported in the user unit.
    pub fn height_offset(&self) -> f64 {
        self.height_offset * LineUnits::from_code(self.user_height_unit).from_meters()
    }

    /// The raw stored height offset in meters (Pascal `heightOffset`), which
    /// `LineGeometry` adds to the equivalent avg heights.
    pub fn height_offset_meters(&self) -> f64 {
        self.height_offset
    }

    /// `SetUserHeightUnit`: re-express the existing height offset in the new
    /// unit — i.e. keep the *number* the user typed and re-read it under the new
    /// unit (Pascal re-runs `SetHeightOffset(…)`, "This updates the existing
    /// value to fit the new user units", r4133
    /// `Version8/Source/General/LineConstants.pas:689-696`; the height-offset
    /// surface does not exist in the pinned 0.14.5 backend at all — grep its
    /// 186 `.pas` for `FheightOffset`, no match — so its line numbers must not
    /// be resolved against `.inputs/dss_capi`).
    ///
    /// Upstream re-reads the **wrong number**: it passes the `FheightOffset`
    /// field, declared *"The height is always saved in meters here"* (`:71`,
    /// `:97`), into `Set_FheightOffset` (`:676-687`), whose argument is a
    /// *user-unit* value it multiplies by `To_Meters(new unit)` — so a metres
    /// value is fed to a user-unit parameter and the same number is converted
    /// twice (5 ft → 1.524 m, then re-read as 1.524 in). The number the comment
    /// asks for is what `Get_FheightOffset` (`:396-399`) returns, i.e.
    /// [`Self::height_offset`] read **before** the unit field moves — the fix is
    /// the getter this class already has, called one statement earlier
    /// (`GOLDEN_REBASE_PLAN.md` G2.1h; `issue-28`).
    ///
    /// The two readings coincide while the outgoing unit is metres, which is
    /// every path that reaches here today: the only consumer is the Line →
    /// Carson push `makeZFromGeometry`/`makeZFromSpacing`, whose fixed call
    /// order stores the offset while the engine still carries its constructed
    /// `UNITS_M` (`line_geometry::matrix::set_line_constants_medium`). They part
    /// on a *second* unit change, pinned by
    /// `line_constants::tests::height_unit_change_rereads_the_typed_number`.
    pub fn set_user_height_unit(&mut self, value: i32) {
        if value == self.user_height_unit {
            return;
        }
        // The number the user typed, read under the *outgoing* unit — captured
        // before the unit field moves, which is what makes it the typed number
        // and not the stored metres.
        let typed = self.height_offset();
        self.user_height_unit = value;
        self.set_height_offset(typed);
    }

    /// `GetUserHeightUnit`.
    pub fn user_height_unit(&self) -> i32 {
        self.user_height_unit
    }

    /// Equivalent-spacing distances (meters). Set by `LineGeometry`'s
    /// `UpdateLineGeometryData` when the referenced spacing is not detailed.
    pub fn set_equivalent_distances(
        &mut self,
        eq_dist_ph_ph: f64,
        eq_dist_ph_n: f64,
        avg_phase_height: f64,
        avg_neutral_height: f64,
    ) {
        self.eq_dist_ph_ph = eq_dist_ph_ph;
        self.eq_dist_ph_n = eq_dist_ph_n;
        self.avg_phase_height = avg_phase_height;
        self.avg_neutral_height = avg_neutral_height;
    }

    /// `Get_Zint(i, EarthModel)`: internal impedance of conductor `i`.
    fn get_zint(&self, i: usize, earth_model: i32) -> Complex64 {
        let cond = &self.cond[i];
        match earth_model {
            // SimpleCarson / FullCarson: no skin effect.
            FULL_CARSON => cmplx(cond.rac, self.fw * MU0 / (8.0 * std::f64::consts::PI)),
            DERI => {
                // with skin effect model; assume round conductor
                let c1_j1 = cmplx(1.0, 1.0);
                let alpha = c1_j1 * (self.ffrequency * MU0 / cond.rdc).sqrt();
                let i0i1 = if alpha.norm() > 35.0 {
                    Complex64::new(1.0, 0.0)
                } else {
                    compat::cdiv(bessel_i0(alpha), bessel_i1(alpha))
                };
                c1_j1 * i0i1 * ((cond.rdc * self.ffrequency * MU0).sqrt() / 2.0)
            }
            // SIMPLECARSON and any other code take the no-skin-effect branch.
            _ => cmplx(cond.rac, self.fw * MU0 / (8.0 * std::f64::consts::PI)),
        }
    }

    /// `Get_Ze(i, j, EarthModel)`: earth-return impedance for the `ij` element.
    fn get_ze(&self, i: usize, j: usize, earth_model: i32) -> Complex64 {
        let pi = std::f64::consts::PI;
        // dss_capi 0.15.x `GetZearth`: in equivalent-spacing mode the heights
        // are the avg phase/neutral heights and the horizontal separation is
        // the equivalent phase-phase / phase-neutral distance (assumed to lie on
        // the X axis). 0-based: conductor `k` is a phase iff `k < nphases`.
        let fyi = if !self.equivalent_spacing {
            self.cond[i].y.abs()
        } else if i < self.nphases {
            self.avg_phase_height.abs()
        } else {
            self.avg_neutral_height.abs()
        };
        let fyj = if !self.equivalent_spacing {
            self.cond[j].y.abs()
        } else if j < self.nphases {
            self.avg_phase_height.abs()
        } else {
            self.avg_neutral_height.abs()
        };
        let fxi_fxj = if !self.equivalent_spacing {
            self.cond[i].x - self.cond[j].x
        } else if (i < self.nphases && j < self.nphases) || (i >= self.nphases && j >= self.nphases)
        {
            self.eq_dist_ph_ph
        } else {
            self.eq_dist_ph_n
        };

        match earth_model {
            // dss_capi 0.15.x `TLineConstants.GetZearth`/`SIMPLECARSON`
            // (LineConstants.pas:474, port of the r3913-era line/conductor work):
            // the earth-return De constant was corrected `658.5 →
            // 658.8530451057239` (the precise `De = 658.87·√(ρ/f)` reference
            // value). UPGRADE_PLAN.md WP-U1.2 row B2/D1; ledger
            // docs/upgrade/DIVERGENCES.md §B2/D1. NB — this is DELIBERATELY
            // inconsistent with `Line`'s `Kxg`, which upstream KEEPS 658.5
            // (Line.pas:531/741/1077); see the compat markers at the `kxg`
            // sites in elements/pd/line/{accessors,code,mod}.rs.
            SIMPLE_CARSON => cmplx(
                self.fw * MU0 / 8.0,
                (self.fw * MU0 / TWOPI)
                    * (658.8530451057239 * (self.frho_earth / self.ffrequency).sqrt()).ln(),
            ),
            FULL_CARSON => {
                // notation from Tleis, Power System Modelling and Fault Analysis
                let b1 = 1.0 / (3.0 * 2.0_f64.sqrt());
                let b2 = 1.0 / 16.0;
                let b3 = (1.0 / (3.0 * 2.0_f64.sqrt())) / 3.0 / 5.0;
                let b4 = (1.0 / 16.0) / 4.0 / 6.0;
                let d2 = (1.0 / 16.0) * pi / 4.0;
                let d4 = ((1.0 / 16.0) / 4.0 / 6.0) * pi / 4.0;
                let c2: f64 = 1.3659315;
                let c4: f64 = 1.3659315 + 1.0 / 4.0 + 1.0 / 6.0;

                let (thetaij, dij) = if i == j {
                    (0.0, 2.0 * fyi)
                } else {
                    let dij = ((fyi + fyj).powi(2) + fxi_fxj.powi(2)).sqrt();
                    (((fyi + fyj) / dij).acos(), dij)
                };
                let mij = 2.8099e-3 * dij * (self.ffrequency / self.frho_earth).sqrt();

                let re = pi / 8.0 - b1 * mij * thetaij.cos()
                    + b2 * mij.powi(2)
                        * ((c2.exp() / mij).ln() * (2.0 * thetaij).cos()
                            + thetaij * (2.0 * thetaij).sin())
                    + b3 * mij * mij * mij * (3.0 * thetaij).cos()
                    - d4 * mij * mij * mij * mij * (4.0 * thetaij).cos();

                let term1 = 0.5 * (1.85138 / mij).ln();
                let term2 = b1 * mij * thetaij.cos();
                let term3 = -d2 * mij.powi(2) * (2.0 * thetaij).cos();
                let term4 = b3 * mij * mij * mij * (3.0 * thetaij).cos();
                let term5 = -b4
                    * mij
                    * mij
                    * mij
                    * mij
                    * ((c4.exp() / mij).ln() * (4.0 * thetaij).cos()
                        + thetaij * (4.0 * thetaij).sin());
                let mut im = term1 + term2 + term3 + term4 + term5;
                im += 0.5 * dij.ln(); // correction term to work with DSS structure

                cmplx(re, im) * (self.fw * MU0 / pi)
            }
            // DERI (and default)
            _ => {
                if i != j {
                    let hterm = cmplx(fyi + fyj, 0.0) + self.fme.inv() * 2.0;
                    let xterm = cmplx(fxi_fxj, 0.0);
                    let ln_arg = csqrt_fpc(hterm * hterm + xterm * xterm);
                    cmplx(0.0, self.fw * MU0 / TWOPI) * ln_arg.ln()
                } else {
                    let hterm = cmplx(fyi, 0.0) + self.fme.inv();
                    cmplx(0.0, self.fw * MU0 / TWOPI) * (hterm * 2.0).ln()
                }
            }
        }
    }

    /// `Calc(f, EarthModel)`: compute base `Z` and `Yc` matrices (ohms/m,
    /// siemens/m) for this frequency and earth impedance. Dispatches to the
    /// subclass override selected by [`LineConstants::kind`].
    ///
    /// Precondition (as in Pascal): every conductor's geometry must be filled
    /// first. `Rdc`/`radius`/`GMR`/`capradius` initialize to the "not set"
    /// sentinel `-1.0`; if a caller leaves one unset, the DERI `Get_Zint`
    /// (`√(f·µ0/Rdc)`) or the `ln(1/radius)` spacing produces a non-finite
    /// entry rather than an error — the geometry layer (`UpdateLineGeometryData`)
    /// is responsible for setting them all, including the `Rdc = Rac/1.02`
    /// default that `ConductorData` applies.
    pub fn calc(&mut self, f: f64, earth_model: i32) {
        match self.kind {
            LineConstantsKind::Overhead => self.calc_overhead(f, earth_model),
            LineConstantsKind::Cable => self.calc_cable(f, earth_model),
        }
    }

    /// `TLineConstants.Calc` — the overhead base.
    fn calc_overhead(&mut self, f: f64, earth_model: i32) {
        self.set_frequency(f); // side effects

        // Free any reduced matrices, remembering the reduced size to redo it.
        let reduced_size = self.fz_reduced.as_ref().map_or(0, |z| z.order());
        self.fz_reduced = None;
        self.fyc_reduced = None;

        self.fz_matrix.clear();
        self.fyc_matrix.clear();

        // For less than 1 kHz use GMR to better match published data.
        let lfactor = cmplx(0.0, self.fw * MU0 / TWOPI);
        let power_freq = f < 1000.0 && f > 40.0;

        // Self impedances
        for i in 0..self.num_conds {
            let mut zi = self.get_zint(i, earth_model);
            let zspacing = if power_freq {
                zi.im = 0.0; // for less than 1 kHz, use published GMR
                lfactor * (1.0 / self.cond[i].gmr).ln()
            } else {
                lfactor * (1.0 / self.cond[i].radius).ln()
            };
            self.fz_matrix
                .set(i, i, zi + zspacing + self.get_ze(i, i, earth_model));
        }

        // Mutual impedances. In equivalent-spacing mode the horizontal
        // separation is the equivalent phase-neutral distance for a phase-to-
        // neutral pair, else the phase-phase distance (0-based: conductor `k` is
        // a phase iff `k < nphases`; the loop has `j < i`).
        for i in 0..self.num_conds {
            for j in 0..i {
                let dij = if !self.equivalent_spacing {
                    ((self.cond[i].x - self.cond[j].x).powi(2)
                        + (self.cond[i].y - self.cond[j].y).powi(2))
                    .sqrt()
                } else if j < self.nphases && i >= self.nphases {
                    self.eq_dist_ph_n
                } else {
                    self.eq_dist_ph_ph
                };
                let z = lfactor * (1.0 / dij).ln() + self.get_ze(i, j, earth_model);
                self.fz_matrix.set(i, j, z);
                self.fz_matrix.set(j, i, z);
            }
        }

        // Capacitance matrix: construct P matrix then invert.
        // `eps_r_medium` defaults to 1.0 (`E0 * 1.0 == E0`), so the default path
        // is bit-identical to 0.14.5.
        let pfactor = -1.0 / TWOPI / (E0 * self.eps_r_medium) / self.fw; // include frequency

        // Self uses capradius, which defaults to actual conductor radius.
        for i in 0..self.num_conds {
            if !self.equivalent_spacing {
                let r = if self.cond[i].cap_radius < 0.0 {
                    self.cond[i].radius
                } else {
                    self.cond[i].cap_radius
                };
                self.fyc_matrix
                    .set(i, i, cmplx(0.0, pfactor * (2.0 * self.cond[i].y / r).ln()));
                continue;
            }
            // Equivalent spacing: use the avg phase/neutral height (Pascal uses
            // Fcapradius[i] directly here, no radius fallback).
            let h = if i >= self.nphases {
                self.avg_neutral_height
            } else {
                self.avg_phase_height
            };
            self.fyc_matrix.set(
                i,
                i,
                cmplx(0.0, pfactor * (2.0 * h / self.cond[i].cap_radius).ln()),
            );
        }
        for i in 0..self.num_conds {
            for j in 0..i {
                let (dij, dijp) = if !self.equivalent_spacing {
                    let dij = ((self.cond[i].x - self.cond[j].x).powi(2)
                        + (self.cond[i].y - self.cond[j].y).powi(2))
                    .sqrt();
                    // distance to image j
                    let dijp = ((self.cond[i].x - self.cond[j].x).powi(2)
                        + (self.cond[i].y + self.cond[j].y).powi(2))
                    .sqrt();
                    (dij, dijp)
                } else {
                    let dij = if j < self.nphases && i >= self.nphases {
                        self.eq_dist_ph_n
                    } else {
                        self.eq_dist_ph_ph
                    };
                    let dijp = if j < self.nphases && i >= self.nphases {
                        self.avg_phase_height + self.avg_neutral_height
                    } else if i < self.nphases && j < self.nphases {
                        2.0 * self.avg_phase_height
                    } else {
                        2.0 * self.avg_neutral_height
                    };
                    (dij, dijp)
                };
                let v = cmplx(0.0, pfactor * (dijp / dij).ln());
                self.fyc_matrix.set(i, j, v);
                self.fyc_matrix.set(j, i, v);
            }
        }

        // now should be nodal C matrix (ignore singularity like Pascal does)
        let _ = self.fyc_matrix.invert();

        if reduced_size > 0 {
            self.kron(reduced_size); // was reduced, so reduce again to same size
        }

        self.frho_changed = false;
    }

    /// `Kron(Norder)`: Kron-reduce to leave the first `norder` rows/cols.
    pub fn kron(&mut self, norder: usize) {
        if self.ffrequency >= 0.0 && norder > 0 && norder < self.num_conds {
            // Reduce the computed Z matrix one row/col (always the last) at a
            // time until it is `norder`.
            let mut ztemp = self.fz_matrix.clone();
            while ztemp.order() > norder {
                // Pascal eliminates the last row (1-based Order ⇒ 0-based last).
                ztemp = ztemp.kron(ztemp.order() - 1).expect("kron order > 1");
            }
            self.fz_reduced = Some(ztemp);

            // Extract the norder x norder portion of the Yc matrix.
            let mut ycr = CMatrix::new(norder);
            for i in 0..norder {
                for j in 0..norder {
                    ycr.set(i, j, self.fyc_matrix.get(i, j));
                }
            }
            self.fyc_reduced = Some(ycr);
        }
    }

    /// `Reduce`: Kron-reduce down to the phase conductors only.
    pub fn reduce(&mut self) {
        self.kron(self.nphases);
    }

    /// `Get_Zmatrix(f, Lngth, Units, EarthModel)`: a new Z matrix corrected for
    /// length and units; recalcs if the frequency or earth rho changed. Uses the
    /// reduced matrix when present.
    pub fn z_matrix(&mut self, f: f64, length: f64, units: i32, earth_model: i32) -> CMatrix {
        if f != self.ffrequency || self.frho_changed {
            self.calc(f, earth_model);
        }
        let z = self.fz_reduced.as_ref().unwrap_or(&self.fz_matrix);
        let mut result = z.clone();
        let conv = LineUnits::from_code(units).from_per_meter() * length;
        for v in result.values_mut() {
            *v *= conv;
        }
        result
    }

    /// `Get_YCmatrix(f, Lngth, Units)`: a new Yc matrix corrected for length and
    /// units. Uses the reduced matrix when present.
    pub fn yc_matrix(&self, length: f64, units: i32) -> CMatrix {
        let yc = self.fyc_reduced.as_ref().unwrap_or(&self.fyc_matrix);
        let mut result = yc.clone();
        let conv = LineUnits::from_code(units).from_per_meter() * length;
        for v in result.values_mut() {
            *v *= conv;
        }
        result
    }

    /// Read access to the base (unreduced) Z matrix (ohms/m), for tests and the
    /// frequency-sweep path.
    pub fn z_base(&self) -> &CMatrix {
        &self.fz_matrix
    }

    /// Read access to the base (unreduced) Yc matrix (siemens/m).
    pub fn yc_base(&self) -> &CMatrix {
        &self.fyc_matrix
    }

    /// `Fw` (2*pi*f, truncated twopi) for the present frequency.
    pub fn omega(&self) -> f64 {
        self.fw
    }

    /// `ConductorsInSameSpace`: validates geometry; returns an error message
    /// when the check fails. Dispatches to the cable override for cable kinds.
    pub fn conductors_in_same_space(&self) -> Option<String> {
        match self.kind {
            LineConstantsKind::Overhead => self.cisp_overhead(),
            LineConstantsKind::Cable => self.cisp_cable(),
        }
    }

    /// `TLineConstants.ConductorsInSameSpace`: fails when a conductor height is
    /// ≤ 0 or two conductors overlap.
    fn cisp_overhead(&self) -> Option<String> {
        // Equivalent-spacing model: heights must be > 0 and no phase-neutral /
        // phase-phase distance may be smaller than the touching-conductor radius
        // sum (dss_capi 0.15.x `ConductorsInSameSpace` equivalent branch).
        if self.equivalent_spacing {
            if self.avg_phase_height <= 0.0 || self.avg_neutral_height <= 0.0 {
                return Some(
                    "Conductor average heights (overhead equivalent spacing) must be > 0."
                        .to_string(),
                );
            }
            for i in 0..self.num_conds {
                for j in (i + 1)..self.num_conds {
                    let dij = if i < self.nphases && j >= self.nphases {
                        self.eq_dist_ph_n
                    } else {
                        self.eq_dist_ph_ph
                    };
                    if dij < (self.cond[i].radius + self.cond[j].radius) {
                        return Some(format!(
                            "Conductors {} and {} occupy the same space.",
                            i + 1,
                            j + 1
                        ));
                    }
                }
            }
            return None;
        }

        // Check for 0 (or negative) Y coordinate.
        for i in 0..self.num_conds {
            if self.cond[i].y <= 0.0 {
                return Some(format!("Conductor {} height must be  > 0. ", i + 1));
            }
        }
        // Check for overlapping conductors.
        for i in 0..self.num_conds {
            for j in (i + 1)..self.num_conds {
                let dij = ((self.cond[i].x - self.cond[j].x).powi(2)
                    + (self.cond[i].y - self.cond[j].y).powi(2))
                .sqrt();
                if dij < (self.cond[i].radius + self.cond[j].radius) {
                    return Some(format!(
                        "Conductors {} and {} occupy the same space.",
                        i + 1,
                        j + 1
                    ));
                }
            }
        }
        None
    }
}
