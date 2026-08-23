//! wgturbine — the WindGen `UserModel=` reference fixture for
//! `R4133_PROPS_PLAN.md` RP1.3. r4133 ships no example WindGen user model (only
//! the loader, `PCElements/WindGenUserModel.pas`), so the model is authored, the
//! way wm4model and capuserctl were.
//!
//! # What it computes
//!
//! Two behaviours, dispatched on `TDynamicsRec.SolutionMode` exactly like the
//! IndMach012a example's `Calc`:
//!
//! - **Power flow** (any non-dynamics mode; Pascal `TWindGenObj.DoUserModel`,
//!   `WindGen.pas:1875-1898`): a constant-admittance current source,
//!   `I[k] = (G + jB)·V[k]` over the record's `NumPhases` phases.
//! - **Dynamics** (`SolutionMode == DYNAMICMODE = 14`; Pascal `DoDynamicMode`
//!   `GenModel=6` arm, `WindGen.pas:1991-1995`): a first-order current lag
//!   toward that admittance target, integrated trapezoidally by `integrate()`
//!   (Pascal `IntegrateStates`, `:2663`), seeded by `init()` (Pascal
//!   `InitStateVars`, `:2568`).
//!
//! Both then fill the **turbine outputs** of `TWindGenVars`
//! (`WindGenVars.pas:61-72`) from the terminal power, which is what makes the
//! fixture a WindGen model rather than a generic one:
//!
//! ```text
//! Pg    = Σ_k Re(V[k]·conj(I[k]))        total power output, W
//! Ps    = Pg·(1 − slip)                  stator active power
//! Pr    = Pg·slip                        rotor active power
//! Pm    = Pg / eta                       mechanical power
//! s     = slip                           generator slip
//! Cp    = Pg / (kVArating·1000)          performance coefficient (capacity factor)
//! Lamda = ag·(1 + slip)                  tip-speed ratio from the gearbox ratio
//! ```
//!
//! and the shaft/speed head fields: `Pshaft = −Pm`, `Speed`, `dSpeed`.
//!
//! It writes only the record's **outputs**; the turbine *inputs* (`ag`, `Poles`,
//! `pd`, `Rad`, `VCutin`, `VCutout`, `PLoss`) stay engine-owned — `ag` is read,
//! never written.
//!
//! # Why the reads are spread across the image
//!
//! The fixture is the end-to-end witness for the ABI §2.6 layout, so its inputs
//! deliberately span every region of the record: the leading doubles (`w0`,
//! `kVArating`, `kVWindGenBase`, `Pnominalperphase`, `Qnominalperphase`), the
//! integer block (`NumPhases`/`NumConductors`/`Conn`), and `ag` — the first
//! turbine-tail field, which sits at 244 **only because** the managed `PLoss`
//! reference does not cross and its hole is closed. A host that got that
//! decision wrong feeds this model garbage in `Lamda`, and the round-trip test
//! fails. `Xdp` (88, a head double), `VTarget` (212, the unaligned stretch that
//! follows the integer block) and `Poles`/`VCutin` (268/292, the turbine tail)
//! extend the same witness to every region of the image and to the
//! element-field→record-field wiring behind it. All of them are echoed verbatim
//! through the state-variable surface (`num_vars`/`get_variable`/`get_var_name`),
//! so a single `get_all_vars` call checks the whole decode.
//!
//! Two of the fifteen variables are not readings at all but **call counters** —
//! `WgUpdCount` (`update_model`) and `WgBadSet` (a `set_variable` outside
//! `1..=NUM_VARS`). They exist so that two engine-side behaviours which
//! otherwise leave no trace — the `RecalcElementData` `FUpdateModel` tail
//! (`WindGen.pas:1418`) and the *absence* of the upstream `Set_Variable`
//! mis-nesting (`:2777-2784`) — can be pinned by a test.
//!
//! # Determinism
//!
//! No host imports (`dss_env` unused), no clock, no allocation beyond
//! `dss_alloc`: the model is a pure function of `V`, `TWindGenVars`,
//! `TDynamicsRec` and its `UserData=` parameters — which is what makes gating on
//! it sound (ABI doc §6).

#![allow(clippy::needless_range_loop)]

pub mod records;

#[cfg(target_arch = "wasm32")]
mod wasm_exports;

/// Maximum number of phases the model computes. WindGen decks in the corpus are
/// 1- or 3-phase; the record's `NumPhases` selects how many are written and is
/// clamped to this.
pub const MAX_PHASES: usize = 3;

/// Pascal `DYNAMICMODE` (`TSolveMode` ordinal, ABI doc §2.1).
pub const DYNAMICMODE: i32 = 14;

/// The value Pascal returns for an out-of-range state variable
/// (`WindGen.pas:2730`).
pub const VAR_OUT_OF_RANGE: f64 = -9999.99;

/// A minimal complex type (re, im) — the fixture avoids `num_complex` to keep
/// the wasm module tiny and the toolchain surface stable (plan §2.6).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Cx {
    pub re: f64,
    pub im: f64,
}

// The fixture keeps its own tiny complex arithmetic (named `mul`/`add`/`sub`)
// rather than implementing `std::ops` — deliberate, so the guest is a single
// self-contained translation unit; the std-trait-confusion lint is not relevant.
#[allow(clippy::should_implement_trait)]
impl Cx {
    pub const ZERO: Cx = Cx { re: 0.0, im: 0.0 };

    pub fn new(re: f64, im: f64) -> Self {
        Cx { re, im }
    }

    /// `self * other` (naive complex product — the fixture's own arithmetic,
    /// not dss-core's FPC-faithful helpers).
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

    /// `Re(self · conj(o))` — the active power carried by `self` against `o`.
    pub fn re_mul_conj(self, o: Cx) -> f64 {
        self.re * o.re + self.im * o.im
    }

    /// `|self|` — naive `sqrt(re² + im²)`.
    pub fn abs(self) -> f64 {
        (self.re * self.re + self.im * self.im).sqrt()
    }
}

/// The turbine outputs the model writes back into `TWindGenVars`
/// (`WindGenVars.pas:61-72` plus the shaft/speed head fields).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outputs {
    /// `Pg` — total power output, W.
    pub pg: f64,
    /// `Ps` — stator active power, W.
    pub ps: f64,
    /// `Pr` — rotor active power, W.
    pub pr: f64,
    /// `Pm` — mechanical power, W.
    pub pm: f64,
    /// `s` — generator slip.
    pub s: f64,
    /// `Cp` — performance coefficient.
    pub cp: f64,
    /// `Lamda` — tip-speed ratio.
    pub lamda: f64,
    /// `Pshaft` (head) — `−Pm`, the shaft power convention of
    /// `WindGen.pas:2557` (`Pshaft := -Power[1].re`).
    pub pshaft: f64,
    /// `Speed` (head) — relative to synchronous, rad/s.
    pub speed: f64,
    /// `dSpeed` (head) — its derivative.
    pub dspeed: f64,
}

/// One bound model instance (Pascal per-`New` state).
#[derive(Clone, Debug)]
pub struct Model {
    /// Shunt conductance (S), `UserData` `g=`.
    pub g: f64,
    /// Shunt susceptance (S), `UserData` `b=`.
    pub b: f64,
    /// Generator slip (pu), `UserData` `slip=`.
    pub slip: f64,
    /// Drive-train efficiency, `UserData` `eta=`.
    pub eta: f64,
    /// Current/speed lag time constant (s), `UserData` `tau=`.
    pub tau: f64,
    /// Integrated state current per phase (dynamics).
    pub is: [Cx; MAX_PHASES],
    /// State-current derivative per phase (dynamics).
    pub dis: [Cx; MAX_PHASES],
    /// Trapezoidal history per phase (dynamics).
    pub is_hist: [Cx; MAX_PHASES],
    /// Integrated relative speed (dynamics).
    pub speed: f64,
    /// Speed derivative (dynamics).
    pub dspeed: f64,
    /// Trapezoidal speed history (dynamics).
    pub speed_hist: f64,
    /// Last terminal voltage seen by `calc` (fed to `integrate`).
    pub vgrid: [Cx; MAX_PHASES],
    /// Last output current per phase (report var).
    pub i_out: [Cx; MAX_PHASES],
    /// Last record head this model saw (echoed through the variable surface).
    pub seen: records::WindGenIn,
    /// Last outputs written back into the record (report vars).
    pub out: Outputs,
    /// How many times the host called `update_model()` (Pascal `FUpdateModel`,
    /// `WindGen.pas:1418`). Reported as `WgUpdCount` so the engine's
    /// `RecalcElementData` tail has an observable consequence — an empty
    /// `update_model` body would make that call site untestable.
    pub upd_count: f64,
    /// How many `SetVariable` calls arrived with an index outside `1..=NUM_VARS`.
    /// Reported as `WgBadSet`: the upstream `Set_Variable` mis-nesting
    /// (`WindGen.pas:2777-2784`) routes NATIVE indices to `FSetVariable(i − 22)`,
    /// i.e. a non-positive index, so a port that reproduced it would bump this
    /// counter where the correct routing leaves it at 0.
    pub bad_set: f64,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            g: 0.0015,
            b: -0.0004,
            slip: 0.02,
            eta: 0.95,
            tau: 0.05,
            is: [Cx::ZERO; MAX_PHASES],
            dis: [Cx::ZERO; MAX_PHASES],
            is_hist: [Cx::ZERO; MAX_PHASES],
            speed: 0.0,
            dspeed: 0.0,
            speed_hist: 0.0,
            vgrid: [Cx::ZERO; MAX_PHASES],
            i_out: [Cx::ZERO; MAX_PHASES],
            seen: records::WindGenIn {
                w0: 0.0,
                kva_rating: 0.0,
                kv_base: 0.0,
                pnominal: 0.0,
                qnominal: 0.0,
                num_phases: 0,
                num_conds: 0,
                conn: 0,
                ag: 0.0,
                xdp: 0.0,
                vtarget: 0.0,
                poles: 0.0,
                v_cutin: 0.0,
            },
            out: Outputs::default(),
            upd_count: 0.0,
            bad_set: 0.0,
        }
    }
}

impl Model {
    /// The admittance `G + jB`.
    fn y(&self) -> Cx {
        Cx::new(self.g, self.b)
    }

    /// How many phases to write, from the record's `NumPhases`, clamped into
    /// `1..=MAX_PHASES` (a Pascal model likewise trusts the record but cannot
    /// write past its own buffers).
    fn nph(&self) -> usize {
        (self.seen.num_phases.max(1) as usize).min(MAX_PHASES)
    }

    /// Pascal `Edit` (`WindGenUserModel.pas:152-156`): a minimal `key=value`
    /// scanner for `g=`, `b=`, `slip=`, `eta=`, `tau=` (whitespace/`(...)`
    /// tolerant; unknown keys ignored) — what a small Delphi/C user model's own
    /// parser would accept, with no dss-parser dependency (plan §2.6).
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
                "slip" => self.slip = val,
                "eta" => self.eta = val,
                "tau" => self.tau = val,
                _ => {}
            }
        }
    }

    /// Pascal `Init(V, I)` — the dynamics seed (`WindGen.pas:2568`): the state
    /// current starts at the admittance steady state `Y·V`, the relative speed
    /// at the slip target, derivatives zero.
    pub fn init(&mut self, v: &[Cx], rec: &records::WindGenIn) {
        self.seen = *rec;
        let y = self.y();
        let n = self.nph();
        for k in 0..n {
            self.is[k] = y.mul(v[k]);
            self.dis[k] = Cx::ZERO;
            self.is_hist[k] = self.is[k];
            self.vgrid[k] = v[k];
            self.i_out[k] = self.is[k];
        }
        self.speed = rec.w0 * self.slip;
        self.dspeed = 0.0;
        self.speed_hist = self.speed;
    }

    /// Pascal `FUpdateModel` (`WindGen.pas:1418`, the `RecalcElementData` tail):
    /// re-read the boundary record — the element's ratings/reactances have just
    /// been re-derived — and count the call. The IndMach012a example does the
    /// same thing (`indmach012a`'s `update_model` recomputes from the record),
    /// and it is what makes the engine's call site observable.
    pub fn update_model(&mut self, rec: &records::WindGenIn) {
        self.seen = *rec;
        self.upd_count += 1.0;
    }

    /// Pascal `Calc(V, I)` — write the terminal currents, then derive the
    /// turbine outputs from the terminal power. Dynamics mode reports the
    /// present integrated state current; power flow reports the admittance
    /// current directly. `V` is cached for `integrate` either way.
    ///
    /// Entries beyond `NumPhases` are left untouched, exactly as a Pascal model
    /// that only fills its own phases does (the engine keeps the neutral).
    pub fn calc(&mut self, v: &[Cx], i: &mut [Cx], rec: &records::WindGenIn, dynamics: bool) {
        self.seen = *rec;
        let y = self.y();
        let n = self.nph();
        let mut pg = 0.0;
        for k in 0..n {
            self.vgrid[k] = v[k];
            let out = if dynamics { self.is[k] } else { y.mul(v[k]) };
            self.i_out[k] = out;
            i[k] = out;
            pg += v[k].re_mul_conj(out);
        }
        let eta = if self.eta.abs() < 1e-12 {
            1.0
        } else {
            self.eta
        };
        let pm = pg / eta;
        let kva_w = rec.kva_rating * 1000.0;
        self.out = Outputs {
            pg,
            ps: pg * (1.0 - self.slip),
            pr: pg * self.slip,
            pm,
            s: self.slip,
            cp: if kva_w == 0.0 { 0.0 } else { pg / kva_w },
            lamda: rec.ag * (1.0 + self.slip),
            pshaft: -pm,
            speed: self.speed,
            dspeed: self.dspeed,
        };
    }

    /// Pascal `Integrate` (`WindGen.pas:2663`) — advance the state current and
    /// the relative speed one trapezoidal half-step toward their targets
    /// (history from the OLD derivative on a new step, then the new derivative,
    /// then the corrected state — the OpenDSS predictor/corrector).
    pub fn integrate(&mut self, h: f64, new_step: bool) {
        let y = self.y();
        let tau = if self.tau.abs() < 1e-12 {
            1e-12
        } else {
            self.tau
        };
        let n = self.nph();
        for k in 0..n {
            if new_step {
                self.is_hist[k] = self.is[k].add(self.dis[k].scale(0.5 * h));
            }
            // dis = (Y·Vgrid − is) / tau
            let target = y.mul(self.vgrid[k]);
            self.dis[k] = target.sub(self.is[k]).scale(1.0 / tau);
            self.is[k] = self.is_hist[k].add(self.dis[k].scale(0.5 * h));
        }
        if new_step {
            self.speed_hist = self.speed + 0.5 * h * self.dspeed;
        }
        self.dspeed = (self.seen.w0 * self.slip - self.speed) / tau;
        self.speed = self.speed_hist + 0.5 * h * self.dspeed;
    }

    /// Pascal `NumVars`. Vars 1-3 are model outputs, 4-9 and 12-15 echo the
    /// record fields the last `calc`/`init`/`update_model` decoded (the offset
    /// witness — see the module doc), 10-11 are the call counters that make the
    /// engine's `FUpdateModel` call site and its `SetVariable` routing
    /// observable.
    pub const NUM_VARS: i32 = 15;

    /// Pascal `GetVarName` (1-based). The names are deliberately unlike the 22
    /// built-in WindGen variable names (`WindGen.pas:2828-2856`), so a name
    /// round-trip cannot pass by accident.
    pub fn var_name(i: i32) -> Option<&'static str> {
        match i {
            1 => Some("WgIout1"),
            2 => Some("WgPg"),
            3 => Some("WgSlip"),
            4 => Some("WgKvaEcho"),
            5 => Some("WgKvBaseEcho"),
            6 => Some("WgW0Echo"),
            7 => Some("WgNphEcho"),
            8 => Some("WgNcondEcho"),
            9 => Some("WgConnEcho"),
            10 => Some("WgUpdCount"),
            11 => Some("WgBadSet"),
            12 => Some("WgXdpEcho"),
            13 => Some("WgVTargetEcho"),
            14 => Some("WgPolesEcho"),
            15 => Some("WgVCutInEcho"),
            _ => None,
        }
    }

    /// Pascal `GetVariable` (1-based); out of range answers `-9999.99` like
    /// `TWindGenObj.Get_Variable` (`WindGen.pas:2730`).
    pub fn get_variable(&self, i: i32) -> f64 {
        match i {
            1 => self.i_out[0].abs(),
            2 => self.out.pg,
            3 => self.slip,
            4 => self.seen.kva_rating,
            5 => self.seen.kv_base,
            6 => self.seen.w0,
            7 => f64::from(self.seen.num_phases),
            8 => f64::from(self.seen.num_conds),
            9 => f64::from(self.seen.conn),
            10 => self.upd_count,
            11 => self.bad_set,
            12 => self.seen.xdp,
            13 => self.seen.vtarget,
            14 => self.seen.poles,
            15 => self.seen.v_cutin,
            _ => VAR_OUT_OF_RANGE,
        }
    }

    /// Pascal `SetVariable` (1-based). Only `WgSlip` (slot 3) is a settable
    /// parameter; the outputs (`WgIout1`, `WgPg`), the record echoes and the two
    /// counters are derived readings and stay read-only, like a real model's.
    ///
    /// An index outside `1..=NUM_VARS` is counted rather than ignored: that is
    /// the only observable a host can use to prove it never routed a NATIVE
    /// state-variable write into the model (the upstream `Set_Variable`
    /// mis-nesting, `WindGen.pas:2777-2784`, sends `FSetVariable(i − 22)`).
    pub fn set_variable(&mut self, i: i32, value: f64) {
        if i == 3 {
            self.slip = value;
        } else if !(1..=Self::NUM_VARS).contains(&i) {
            self.bad_set += 1.0;
        }
    }

    /// Pascal `GetAllVars` → [`Self::NUM_VARS`] f64, 1-based.
    pub fn get_all_vars(&self, out: &mut [f64]) {
        for (k, v) in out.iter_mut().enumerate() {
            *v = self.get_variable(k as i32 + 1);
        }
    }
}

/// The per-guest instance registry (Pascal `MainUnit`'s
/// `ModelList`/`ActiveModel`). Each element binding gets its own wasm instance
/// (own `Store`), so the registry normally holds exactly one model.
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
        self.active.and_then(|i| self.models[i].as_mut())
    }

    pub fn active_ref(&self) -> Option<&Model> {
        self.active.and_then(|i| self.models[i].as_ref())
    }
}
