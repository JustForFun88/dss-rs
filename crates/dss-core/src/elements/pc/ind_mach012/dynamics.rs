//! Dynamics-mode machinery: `InitStateVars`, `IntegrateStates`, the internal-
//! voltage integration (`Integrate`), and the 22 state variables Monitor mode 3
//! consumes. The induction machine is a voltage source behind the transient
//! reactance `Zsp`; its positive/negative-sequence internal voltages `E1`/`E2`
//! and the shaft speed/angle are integrated by the trapezoidal predictor/
//! corrector in `SolveDynamic`.

use num_complex::Complex64;

use crate::elements::traits::{CktElement, SysCtx};
use crate::support::complexutil::cang;
use crate::support::dynamics::IterationFlag;
use crate::support::mathutil::{SymComp, power_factor, terminal_power_in};

use super::IndMach012;

// Pascal `DSSGlobals.TwoPi = 2·PI` and `RadiansToDegrees = 180/PI` — full
// precision in the vendored 0.14.5 source (the truncated `57.29577951` line is
// commented out there); using it would diverge from the oracle. Not a
// TODO(compat) on this path. (Same convention as the Generator dynamics port.)
const TWO_PI: f64 = std::f64::consts::TAU;
const RADIANS_TO_DEGREES: f64 = 180.0 / std::f64::consts::PI;

const NUM_VARIABLES: usize = 22;

impl IndMach012 {
    /// Pascal `TIndMach012Obj.InitStateVars` — seed the internal voltages and
    /// shaft state from the present (power-flow) operating point.
    pub(super) fn init_state_vars_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.cd.yprim_invalid = true; // force rebuild of YPrims

        if !self.machine_on {
            self.theta = 0.0;
            self.dtheta = 0.0;
            self.w0 = 0.0;
            self.speed = 0.0;
            self.dspeed = 0.0;
            return;
        }

        // Nominal positive-sequence voltage behind the transient reactance.
        self.yeq = self.zsp.inv();

        self.compute_iterminal(sys, node_v);

        let mut v012 = [Complex64::ZERO; 3];
        let mut i012 = [Complex64::ZERO; 3];
        match self.cd.nphases {
            1 => {
                let nr = &self.cd.node_ref;
                self.e1 = node_v[nr[0]] - node_v[nr[1]] - self.cd.iterminal[0] * self.zsp;
                // (Pascal leaves V012/I012 uninitialized in the 1-phase branch, so
                // `InitModel` would read heap garbage for E2 — UB upstream. Use
                // zero here: 1-phase dynamics is unreachable in the corpus, and
                // zero is the only well-defined choice.)
            }
            3 => {
                let sc = SymComp::default();
                sc.phase_to_sym(&self.cd.iterminal[..3], &mut i012); // terminal currents
                let mut vabc = [Complex64::ZERO; 3];
                for (i, v) in vabc.iter_mut().enumerate() {
                    *v = node_v[self.cd.node_ref[i]]; // wye voltage
                }
                sc.phase_to_sym(&vabc, &mut v012);
                self.e1 = v012[1] - i012[1] * self.zsp; // positive sequence
            }
            _ => {
                // Pascal sets DSS.SolutionAbort := TRUE (msg 5672). `init_state_vars`
                // has no error channel; the machine is left zero-initialized, so
                // `w0`/`m_mass` stay 0 and a following `integrate_states` would
                // divide by `m_mass = 0` (NaN). Unreachable: the corpus is 1-/3-phase.
                // TODO(WP7.7): surface a real abort if a >3-phase case appears.
                return;
            }
        }

        // InitModel: E1 already set; seed the derivatives and E2.
        self.de1dt = Complex64::ZERO;
        self.e1n = self.e1;
        self.de1dtn = self.de1dt;
        self.e2 = v012[2] - i012[2] * self.zsp;
        self.de2dt = Complex64::ZERO;
        self.e2n = self.e2;
        self.de2dtn = self.de2dt;

        // Shaft variables.
        self.theta = cang(self.e1);
        self.dtheta = 0.0;
        self.w0 = TWO_PI * sys.frequency;
        // Recalc Mmass and D in case the frequency changed. `Dpu` has no property
        // (stays 0), so the machine is effectively undamped — see [`IndMach012::dpu`].
        self.m_mass = 2.0 * self.h_mass * self.kva_rating * 1000.0 / self.w0; // M = W-sec
        self.d = self.dpu * self.kva_rating * 1000.0 / self.w0;
        self.p_shaft = self.terminal_power(sys, node_v, 1).re; // present motor power
        self.speed = -self.s1 * self.w0; // relative to synchronous
        self.dspeed = 0.0;
        // NOT_PORTED: DebugTrace separator record.
    }

    /// Pascal `TIndMach012Obj.IntegrateStates` — advance the shaft state and the
    /// internal voltages by one trapezoidal half-step.
    pub(super) fn integrate_states_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.compute_iterminal(sys, node_v);
        let h = sys.dyna_h;

        if sys.iteration_flag == IterationFlag::NewTimeStep {
            // First iteration of a new time step.
            self.theta_history = self.theta + 0.5 * h * self.dtheta;
            self.speed_history = self.speed + 0.5 * h * self.dspeed;
        }

        // Compute shaft dynamics (TracePower = power into the terminal, watts).
        let trace_power =
            terminal_power_in(&self.cd.vterminal, &self.cd.iterminal, self.cd.nphases);
        self.dspeed = (trace_power.re - self.p_shaft - (self.d * self.speed).abs()) / self.m_mass;
        self.dtheta = self.speed;

        // Trapezoidal method.
        self.speed = self.speed_history + 0.5 * h * self.dspeed;
        self.theta = self.theta_history + 0.5 * h * self.dtheta;
        // NOT_PORTED: DebugTrace record.

        self.integrate_internal_voltages(sys);
    }

    /// Pascal `TIndMach012Obj.Integrate` — trapezoidal integration of the
    /// internal voltages `E1`/`E2`. `dEdt = -jw0·S·E' − (E' − j(X−X')·I')/T0'`.
    fn integrate_internal_voltages(&mut self, sys: &SysCtx) {
        if sys.iteration_flag == IterationFlag::NewTimeStep {
            // Predictor step: save old values.
            self.e1n = self.e1;
            self.de1dtn = self.de1dt;
            self.e2n = self.e2;
            self.de2dtn = self.de2dt;
        }

        let j_xdiff = Complex64::new(0.0, self.xopen - self.xp);
        self.de1dt = Complex64::new(0.0, -self.w0 * self.s1) * self.e1
            - (self.e1 - j_xdiff * self.is1) / self.t0p;
        self.de2dt = Complex64::new(0.0, -self.w0 * self.s2) * self.e2
            - (self.e2 - j_xdiff * self.is2) / self.t0p;

        let h2 = sys.dyna_h * 0.5;
        self.e1 = self.e1n + (self.de1dt + self.de1dtn) * h2;
        self.e2 = self.e2n + (self.de2dt + self.de2dtn) * h2;
    }

    /// Pascal `TIndMach012Obj.GetStatorLosses` (`3·(|Is1|²+|Is2|²)·Zs.re`).
    fn get_stator_losses(&self) -> f64 {
        3.0 * (self.is1.norm_sqr() + self.is2.norm_sqr()) * self.zs.re
    }

    /// Pascal `TIndMach012Obj.GetRotorLosses` (`3·(|Ir1|²+|Ir2|²)·Zr.re`).
    fn get_rotor_losses(&self) -> f64 {
        3.0 * (self.ir1.norm_sqr() + self.ir2.norm_sqr()) * self.zr.re
    }

    /// Pascal `TIndMach012Obj.NumVariables`.
    pub(super) fn num_variables_impl(&self) -> usize {
        NUM_VARIABLES
    }

    /// Pascal `TIndMach012Obj.VariableName(i)` (1-based). Out-of-range → "ERROR"
    /// (the unreachable guard value Pascal seeds and returns).
    pub(super) fn variable_name_impl(&self, i: usize) -> String {
        match i {
            1 => "Frequency",
            2 => "Theta (deg)",
            3 => "E1",
            4 => "Pshaft",
            5 => "dSpeed (deg/sec)",
            6 => "dTheta (deg)",
            7 => "Slip",
            8 => "puRs",
            9 => "puXs",
            10 => "puRr",
            11 => "puXr",
            12 => "puXm",
            13 => "Maxslip",
            14 => "Is1",
            15 => "Is2",
            16 => "Ir1",
            17 => "Ir2",
            18 => "Stator Losses",
            19 => "Rotor Losses",
            20 => "Shaft Power (hp)",
            21 => "Power Factor",
            22 => "Efficiency (%)",
            _ => "ERROR",
        }
        .to_string()
    }

    /// Pascal `TIndMach012Obj.GetAllVariables` (fills `states[0..22]` from the 22
    /// `Get_Variable` cases). `Power[1]` is computed once (here it exists, with a
    /// solved solution) for state vars #21/#22.
    pub(super) fn get_all_variables_impl(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        states: &mut [f64],
    ) {
        let power1 = self.terminal_power(sys, node_v, 1);
        let stator_losses = self.get_stator_losses();
        let rotor_losses = self.get_rotor_losses();

        states[0] = (self.w0 + self.speed) / TWO_PI; // Frequency, Hz
        states[1] = self.theta * RADIANS_TO_DEGREES; // Theta, deg
        states[2] = self.e1.norm() / self.v_base; // E1, pu
        states[3] = self.p_shaft; // Pshaft
        states[4] = self.dspeed * RADIANS_TO_DEGREES; // dSpeed, deg/sec
        states[5] = self.dtheta; // dTheta
        states[6] = self.s1; // Slip
        states[7] = self.pu_rs;
        states[8] = self.pu_xs;
        states[9] = self.pu_rr;
        states[10] = self.pu_xr;
        states[11] = self.pu_xm;
        states[12] = self.max_slip;
        states[13] = self.is1.norm(); // |Is1|
        states[14] = self.is2.norm(); // |Is2|
        states[15] = self.ir1.norm(); // |Ir1|
        states[16] = self.ir2.norm(); // |Ir2|
        states[17] = stator_losses;
        states[18] = rotor_losses;
        // Shaft Power (hp).
        states[19] = 3.0 / 746.0
            * (self.ir1.norm_sqr() * (1.0 - self.s1) / self.s1
                + self.ir2.norm_sqr() * (1.0 - self.s2) / self.s2)
            * self.zr.re;
        states[20] = power_factor(power1); // Power Factor
        states[21] = (1.0 - (stator_losses + rotor_losses) / power1.re) * 100.0; // Efficiency, %
    }

    /// Pascal `TIndMach012Obj.Set_Variable` (1-based; only slip + the pu
    /// reactances are writable, the rest are read-only).
    pub(super) fn set_variable_impl(&mut self, i: usize, value: f64) {
        match i {
            7 => {
                self.set_local_slip(value);
                self.speed = self.w0 * (-self.s1); // PropertySideEffects(slip)
            }
            8 => self.pu_rs = value,
            9 => self.pu_xs = value,
            10 => self.pu_rr = value,
            11 => self.pu_xr = value,
            12 => self.pu_xm = value,
            _ => {} // read-only
        }
    }
}
