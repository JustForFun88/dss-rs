//! wm4model — a small, physically-plausible 3-phase inverter user model for the
//! WASM_USERMODELS WM.4 Storage/PVSystem gates (there is no vendored upstream
//! example for Storage/PVSystem user models — plan §WP-WM.4.3).
//!
//! Two behaviours, dispatched on `TDynamicsRec.SolutionMode` exactly like the
//! IndMach012a example's `Calc` (power-flow vs dynamics):
//!
//! - **Power flow** (any non-dynamics mode): a constant admittance current
//!   source, `I[k] = (G + jB)·V[k]`. Used by the PVSystem `UserModel=`
//!   (VoltageModel=3) gate `wasm_pv_pflow.dss` — a snapshot solve, so the model
//!   is a pure function of the converged terminal voltage (bit-exact across
//!   engines, like the WM.3 pflow deck).
//! - **Dynamics** (`SolutionMode == DYNAMICMODE = 14`): a first-order current
//!   lag toward the admittance target, integrated trapezoidally (the OpenDSS
//!   Storage/PVSystem `IntegrateStates` predictor/corrector). Used by the Storage
//!   `DynaDLL=` gate `wasm_storage_dyn.dss`.
//!
//! One module exports the full 15-function interface (`new(dynarec)` shape), so
//! it validates as both the 15-fn PVSystem `UserModel` and the 13-fn Storage
//! `DynaDLL` (the 13-fn set is the 15-fn set minus `save`/`restore`). The model
//! reads its inputs from the `V` buffer + the `TDynamicsRec` shuttle only — it
//! needs NO host callbacks and NO `TStorageVars`/`TPVSystemVars` image
//! (nothing crosses via `get_public_data`), keeping the frozen ABI unchanged.
//!
//! The model is three-phase: `calc` writes exactly 3 phase currents. The PV
//! (`UserModel`) gate deck is a 3-wire DELTA element (nconds = 3) so all written
//! entries are consumed by the negate-into-`InjCurrent` loop; the Storage
//! (`DynaDLL`) gate deck is a 4-wire WYE element whose `StickCurrInTerminalArray`
//! maps the 3 phase currents into the terminal array (the engine handles the
//! neutral) — both are fully deterministic.

#![allow(clippy::needless_range_loop)]

pub mod records;

#[cfg(target_arch = "wasm32")]
mod wasm_exports;

#[cfg(not(target_arch = "wasm32"))]
pub mod native_exports;

/// Number of phases the model computes (fixed — see the module note).
pub const NPHASES: usize = 3;

/// Pascal `DYNAMICMODE` (`TSolveMode` ordinal, ABI doc §2.1).
pub const DYNAMICMODE: i32 = 14;

/// A minimal complex type (re, im) — the fixture avoids `num_complex` to keep
/// the wasm module tiny and the toolchain surface stable (plan §2.6).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Cx {
    pub re: f64,
    pub im: f64,
}

// The fixture keeps its own tiny complex arithmetic (named `mul`/`add`/`sub`)
// rather than implementing `std::ops` — deliberate, so both boundary sides share
// exactly this code; the std-trait-confusion lint is not relevant here.
#[allow(clippy::should_implement_trait)]
impl Cx {
    pub const ZERO: Cx = Cx { re: 0.0, im: 0.0 };

    pub fn new(re: f64, im: f64) -> Self {
        Cx { re, im }
    }

    /// `self * other` (naive complex product — the fixture's own arithmetic,
    /// not dss-core's FPC-faithful helpers; both boundary sides use this same
    /// code, so wasm and native twin agree bit-for-bit).
    pub fn mul(self, o: Cx) -> Cx {
        Cx {
            re: self.re * o.re - self.im * o.im,
            im: self.re * o.im + self.im * o.re,
        }
    }

    pub fn add(self, o: Cx) -> Cx {
        Cx {
            re: self.re + o.re,
            im: self.im + o.im,
        }
    }

    pub fn sub(self, o: Cx) -> Cx {
        Cx {
            re: self.re - o.re,
            im: self.im - o.im,
        }
    }

    /// Scale by a real.
    pub fn scale(self, s: f64) -> Cx {
        Cx {
            re: self.re * s,
            im: self.im * s,
        }
    }

    /// `|self|` — naive `sqrt(re² + im²)` (matches the fixture's own convention
    /// on both boundary sides).
    pub fn abs(self) -> f64 {
        (self.re * self.re + self.im * self.im).sqrt()
    }
}

/// One bound model instance (Pascal per-`New` state).
#[derive(Clone, Debug)]
pub struct Model {
    /// Shunt conductance (S), `UserData` `g=`.
    pub g: f64,
    /// Shunt susceptance (S), `UserData` `b=`.
    pub b: f64,
    /// Current-lag time constant (s), `UserData` `tau=`.
    pub tau: f64,
    /// Integrated state current per phase (dynamics).
    pub is: [Cx; NPHASES],
    /// State-current derivative per phase (dynamics).
    pub dis: [Cx; NPHASES],
    /// Trapezoidal history per phase (dynamics).
    pub is_hist: [Cx; NPHASES],
    /// Last terminal voltage seen by `calc` (fed to `integrate`).
    pub vgrid: [Cx; NPHASES],
    /// Last output current per phase (report var).
    pub i_out: [Cx; NPHASES],
}

impl Default for Model {
    fn default() -> Self {
        Model {
            g: 0.002,
            b: 0.0,
            tau: 0.05,
            is: [Cx::ZERO; NPHASES],
            dis: [Cx::ZERO; NPHASES],
            is_hist: [Cx::ZERO; NPHASES],
            vgrid: [Cx::ZERO; NPHASES],
            i_out: [Cx::ZERO; NPHASES],
        }
    }
}

impl Model {
    /// The admittance `G + jB`.
    fn y(&self) -> Cx {
        Cx::new(self.g, self.b)
    }

    /// Pascal `Edit`: a minimal `key=value` scanner for `g=`, `b=`, `tau=`
    /// (whitespace/`(...)`-tolerant; unknown keys ignored). Mirrors what a small
    /// Delphi/C user model's own parser would accept — no dss-parser dependency
    /// (plan §2.6).
    pub fn edit(&mut self, data: &str) {
        for tok in data
            .split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == ',')
            .filter(|t| !t.is_empty())
        {
            let Some((k, v)) = tok.split_once('=') else {
                continue;
            };
            let Ok(val) = v.trim().parse::<f64>() else {
                continue;
            };
            match k.trim().to_ascii_lowercase().as_str() {
                "g" => self.g = val,
                "b" => self.b = val,
                "tau" => self.tau = val,
                _ => {}
            }
        }
    }

    /// Pascal `Init(V, I)` — dynamics seed: the state current starts at the
    /// admittance steady-state `Y·V`, derivative zero.
    pub fn init(&mut self, v: &[Cx; NPHASES]) {
        let y = self.y();
        for k in 0..NPHASES {
            self.is[k] = y.mul(v[k]);
            self.dis[k] = Cx::ZERO;
            self.is_hist[k] = self.is[k];
            self.vgrid[k] = v[k];
            self.i_out[k] = self.is[k];
        }
    }

    /// Pascal `Calc(V, I)` — write the terminal currents. Dynamics mode outputs
    /// the present integrated state current; power-flow mode outputs the
    /// admittance current directly. Caches `V` for `integrate` either way.
    pub fn calc(&mut self, v: &[Cx; NPHASES], i: &mut [Cx; NPHASES], dynamics: bool) {
        let y = self.y();
        for k in 0..NPHASES {
            self.vgrid[k] = v[k];
            let out = if dynamics { self.is[k] } else { y.mul(v[k]) };
            self.i_out[k] = out;
            i[k] = out;
        }
    }

    /// Pascal `Integrate` — advance the state current one trapezoidal half-step
    /// toward the admittance target (the Storage/PVSystem `IntegrateStates`
    /// predictor/corrector: history from the OLD derivative on a new step, then
    /// the new derivative, then the corrected state).
    pub fn integrate(&mut self, h: f64, new_step: bool) {
        let y = self.y();
        let tau = if self.tau.abs() < 1e-12 {
            1e-12
        } else {
            self.tau
        };
        for k in 0..NPHASES {
            if new_step {
                self.is_hist[k] = self.is[k].add(self.dis[k].scale(0.5 * h));
            }
            // dis = (Y·Vgrid − is) / tau
            let target = y.mul(self.vgrid[k]);
            self.dis[k] = target.sub(self.is[k]).scale(1.0 / tau);
            self.is[k] = self.is_hist[k].add(self.dis[k].scale(0.5 * h));
        }
    }

    /// Pascal `NumVars`.
    pub const NUM_VARS: i32 = 4;

    /// Pascal `GetVarName` (1-based).
    pub fn var_name(i: i32) -> Option<&'static str> {
        match i {
            1 => Some("Iout1"),
            2 => Some("G"),
            3 => Some("B"),
            4 => Some("Tau"),
            _ => None,
        }
    }

    /// Pascal `GetVariable` (1-based).
    pub fn get_variable(&self, i: i32) -> f64 {
        match i {
            1 => self.i_out[0].abs(),
            2 => self.g,
            3 => self.b,
            4 => self.tau,
            _ => -9999.99,
        }
    }

    /// Pascal `SetVariable` (1-based). Report/derived vars are read-only; the
    /// params are settable.
    pub fn set_variable(&mut self, i: i32, value: f64) {
        match i {
            2 => self.g = value,
            3 => self.b = value,
            4 => self.tau = value,
            _ => {} // Iout1 is read-only
        }
    }

    /// Pascal `GetAllVars` → 4 f64.
    pub fn get_all_vars(&self, out: &mut [f64; 4]) {
        out[0] = self.get_variable(1);
        out[1] = self.get_variable(2);
        out[2] = self.get_variable(3);
        out[3] = self.get_variable(4);
    }
}

/// The per-guest / per-process instance registry (Pascal `MainUnit`'s
/// `ModelList`/`ActiveModel`). Each element binding gets its own wasm instance
/// (own `Store`), so the wasm registry holds one model; the native DLL is
/// process-global and holds one model per element that loaded it.
#[derive(Default)]
pub struct Registry {
    pub models: Vec<Option<Model>>,
    pub active: Option<usize>,
}

impl Registry {
    pub const fn new() -> Self {
        Registry {
            models: Vec::new(),
            active: None,
        }
    }

    /// Pascal `New` → a fresh instance; returns its 1-based id.
    pub fn new_instance(&mut self) -> i32 {
        self.models.push(Some(Model::default()));
        let idx = self.models.len() - 1;
        self.active = Some(idx);
        (idx + 1) as i32
    }

    /// Pascal `Select(id)` → set active; returns the id (0 if invalid).
    pub fn select(&mut self, id: i32) -> i32 {
        if id >= 1 && (id as usize) <= self.models.len() && self.models[(id - 1) as usize].is_some()
        {
            self.active = Some((id - 1) as usize);
            id
        } else {
            0
        }
    }

    /// Pascal `Delete(id)`.
    pub fn delete(&mut self, id: i32) {
        if id >= 1 && (id as usize) <= self.models.len() {
            self.models[(id - 1) as usize] = None;
            if self.active == Some((id - 1) as usize) {
                self.active = None;
            }
        }
    }

    pub fn active_mut(&mut self) -> Option<&mut Model> {
        self.active.and_then(move |i| self.models[i].as_mut())
    }

    pub fn active_ref(&self) -> Option<&Model> {
        self.active.and_then(|i| self.models[i].as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_flow_is_admittance_times_v() {
        let mut m = Model::default();
        m.edit("g=0.003 b=-0.001");
        let v = [
            Cx::new(2400.0, 0.0),
            Cx::new(-1200.0, -2078.0),
            Cx::new(-1200.0, 2078.0),
        ];
        let mut i = [Cx::ZERO; 3];
        m.calc(&v, &mut i, false);
        for k in 0..3 {
            let y = Cx::new(0.003, -0.001);
            assert_eq!(i[k], y.mul(v[k]));
        }
    }

    #[test]
    fn dynamics_lag_relaxes_toward_target() {
        let mut m = Model::default();
        m.edit("g=0.002 tau=0.05");
        let v = [Cx::new(2400.0, 0.0); 3];
        m.init(&v);
        let mut i = [Cx::ZERO; 3];
        // First calc outputs the (steady) state; integrate a few steps.
        m.calc(&v, &mut i, true);
        let start = i[0];
        for step in 0..5 {
            m.calc(&v, &mut i, true);
            m.integrate(0.001, true);
            let _ = step;
        }
        // At the admittance steady state the lag stays put (init seeded there).
        let target = Cx::new(0.002, 0.0).mul(v[0]);
        assert!((m.is[0].sub(target)).abs() < 1e-6, "start {start:?}");
    }

    #[test]
    fn registry_new_select_delete() {
        let mut r = Registry::new();
        assert_eq!(r.new_instance(), 1);
        assert_eq!(r.select(1), 1);
        assert_eq!(r.select(2), 0);
        assert!(r.active_ref().is_some());
        r.delete(1);
        assert!(r.active_ref().is_none());
    }
}
