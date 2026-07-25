//! capuserctl — a small deadband voltage CapControl user model for the
//! WASM_USERMODELS WM.5 gate (there is no vendored upstream CapUserControl
//! example — plan §WP-WM.5.2).
//!
//! **Behaviour (deadband voltage control, identical to the built-in VOLTAGE
//! CapControl).** Each `sample()` the model reads the magnitude of the monitored
//! node voltage `|NodeV[node]|` (the 1-based node index arrives via `UserData`,
//! `node=`) through the `get_node_voltages` callback (ABI row 17 — the
//! *symmetric* channel picked in round 1), and:
//!
//! - if `|V| < vlow` and it did not last command CLOSE → schedule a **close**;
//! - if `|V| > vhigh` and it did not last command OPEN → schedule an **open**;
//! - otherwise do nothing.
//!
//! It signals the decision through `control_queue_push(hour, sec, code, 0)` (a
//! plain queue push — the guest owns the timing, ABI §2.5 / STATUS WM5-3). The
//! `hour`/`sec` come from the dynamics record (`get_dynamics_rec`).
//!
//! The `(vlow, vhigh)` deadband maps exactly onto the built-in VOLTAGE
//! CapControl's `(OnSetting, OffSetting)` (`CapControl.pas` VOLTAGECONTROL,
//! 1-step bank: close when `Vtest < ON`, open when `Vtest > OFF`), so the r4133
//! **built-in** VOLTAGE control is a sound cross-engine oracle for this model —
//! see the STATUS WM.5 round-2 finding on why a native CapUserControl *twin*
//! cannot be the oracle (it has no owner pointer to push a control action).
//!
//! One module exports the 7-function CapControl interface (`new()`,
//! `delete`/`select`/`edit`/`update_model`/`sample`/`do_pending`), reading its
//! context via `dss_env` callbacks only (NO boundary records — `sample()` takes
//! no arguments, ABI §2.5).

pub mod records;

#[cfg(target_arch = "wasm32")]
mod wasm_exports;

#[cfg(not(target_arch = "wasm32"))]
pub mod native_exports;

/// Pascal `EControlAction` codes (`ControlElem.pas`): the values pushed onto the
/// control queue and acted on by `DoPendingAction`.
pub const CTRL_NONE: i32 = 0;
pub const CTRL_OPEN: i32 = 1;
pub const CTRL_CLOSE: i32 = 2;

/// A minimal complex (re, im) — the fixture avoids `num_complex` to keep the
/// wasm module tiny and the toolchain surface stable (plan §2.6).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Cx {
    pub re: f64,
    pub im: f64,
}

impl Cx {
    pub const ZERO: Cx = Cx { re: 0.0, im: 0.0 };

    pub fn new(re: f64, im: f64) -> Self {
        Cx { re, im }
    }

    /// `|self|` — naive `sqrt(re² + im²)`; the converged node voltage is
    /// identical across engines, so this is bit-stable at the decision.
    pub fn abs(self) -> f64 {
        (self.re * self.re + self.im * self.im).sqrt()
    }
}

/// One bound model instance (Pascal per-`New` state).
#[derive(Clone, Debug)]
pub struct Model {
    /// 1-based monitored node index (`UserData` `node=`).
    pub node: usize,
    /// Close threshold (V) — the built-in `OnSetting`.
    pub vlow: f64,
    /// Open threshold (V) — the built-in `OffSetting`.
    pub vhigh: f64,
    /// The last action this model commanded onto the queue (CTRL_NONE /
    /// CTRL_OPEN / CTRL_CLOSE). Tracks the bank state across control iterations
    /// so a decision is pushed only on a change (mirrors the built-in reading
    /// `PresentState`; after each action is applied the two coincide, and the
    /// gate decks drive a monotone excursion so they never diverge).
    pub last_action: i32,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            node: 1,
            vlow: 0.0,
            vhigh: f64::INFINITY,
            last_action: CTRL_NONE,
        }
    }
}

impl Model {
    /// Pascal `Edit`: a minimal `key=value` scanner for `node=`, `vlow=`,
    /// `vhigh=` (whitespace/`(...)`-tolerant; unknown keys ignored).
    pub fn edit(&mut self, data: &str) {
        for tok in data
            .split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == ',')
            .filter(|t| !t.is_empty())
        {
            let Some((k, v)) = tok.split_once('=') else {
                continue;
            };
            let key = k.trim().to_ascii_lowercase();
            let vt = v.trim();
            match key.as_str() {
                "node" => {
                    if let Ok(n) = vt.parse::<usize>() {
                        self.node = n;
                    }
                }
                "vlow" => {
                    if let Ok(x) = vt.parse::<f64>() {
                        self.vlow = x;
                    }
                }
                "vhigh" => {
                    if let Ok(x) = vt.parse::<f64>() {
                        self.vhigh = x;
                    }
                }
                _ => {}
            }
        }
    }

    /// The deadband decision from the monitored node voltage magnitude. Returns
    /// the control-action code to schedule, or `None` to do nothing. Pure and
    /// deterministic (the shared core both boundary sides call).
    pub fn decide(&mut self, vmag: f64) -> Option<i32> {
        if vmag < self.vlow && self.last_action != CTRL_CLOSE {
            self.last_action = CTRL_CLOSE;
            Some(CTRL_CLOSE)
        } else if vmag > self.vhigh && self.last_action != CTRL_OPEN {
            self.last_action = CTRL_OPEN;
            Some(CTRL_OPEN)
        } else {
            None
        }
    }
}

/// The per-guest / per-process instance registry (Pascal `MainUnit`'s
/// `ModelList`/`ActiveModel`).
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
    fn edit_parses_node_and_deadband() {
        let mut m = Model::default();
        m.edit("node=3 vlow=7000 vhigh=7300");
        assert_eq!(m.node, 3);
        assert_eq!(m.vlow, 7000.0);
        assert_eq!(m.vhigh, 7300.0);
    }

    #[test]
    fn deadband_opens_high_closes_low_once_each() {
        let mut m = Model::default();
        m.edit("node=1 vlow=7000 vhigh=7300");
        // In band: no action.
        assert_eq!(m.decide(7150.0), None);
        // Above high → open, once.
        assert_eq!(m.decide(7400.0), Some(CTRL_OPEN));
        assert_eq!(m.decide(7400.0), None); // already open — no re-push
                                            // Below low → close, once.
        assert_eq!(m.decide(6900.0), Some(CTRL_CLOSE));
        assert_eq!(m.decide(6900.0), None); // already closed — no re-push
                                            // Back above → open again.
        assert_eq!(m.decide(7400.0), Some(CTRL_OPEN));
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
