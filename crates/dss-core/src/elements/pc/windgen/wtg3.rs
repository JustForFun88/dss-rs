//! Port of `PCElements/WTG3_Model.pas` — `TGE_WTG3_Model`, the GE WTG type-3
//! wind-turbine dynamics model driving `WindGen`'s dynamic current injection.
//!
//! The model is a fixed-structure controller stack (PLL, seq-current PI
//! regulators, current limiting, LVPL/LVQL fault ride-through, an aerodynamic
//! Cp 5×5 polynomial, MPPT/torque/pitch/inertia control and a one-mass swing)
//! integrated by a **50 µs trapezoidal sub-cycle** — `CalcDynamic` breaks each
//! outer dynamics step `h` into an odd number of sub-steps `delt ≈ delt0` and
//! runs the whole controller once per sub-step, so the model is deterministic
//! (no solver, no oracle floor beyond the shared f64/faer class). The 12-slot
//! trapezoidal integrator (`intg_x`/`intg_d`/`intg_d_old`) holds the regulator
//! states; the readable state variables are exposed by `WindGen::get_variable`.
//!
//! Pascal identifiers are preserved (snake_cased) so the transcription is
//! line-checkable against `WTG3_Model.pas`. The 1-based Pascal phase arrays
//! (`array[1..3]`) map to 0-based `[_; 3]`; the sequence arrays (`array[0..2]`)
//! keep their indices.

use num_complex::Complex64;

use crate::support::complexutil::{cang, pclx};
use crate::support::dynamics::IterationFlag;
use crate::support::mathutil::SymComp;

const PI: f64 = std::f64::consts::PI;

/// Pascal `MagLimiter`: clamp a phasor's magnitude to `[magmin, magmax]`,
/// keeping its angle (`pclx(max(magmin,min(magmax,|x|)), cang(x))`).
fn mag_limiter(x: Complex64, magmin: f64, magmax: f64) -> Complex64 {
    pclx(magmin.max(magmax.min(x.norm())), cang(x))
}

/// Pascal `LinearInterp(xTable, yTable, x)`: piecewise-linear lookup, flat
/// outside the table.
fn linear_interp(x_table: &[f64], y_table: &[f64], x: f64) -> f64 {
    let i_left = 0;
    let i_right = x_table.len() - 1;
    if x < x_table[i_left] {
        return y_table[i_left];
    }
    if x > x_table[i_right] {
        return y_table[i_right];
    }
    for ii in i_left..i_right {
        if x >= x_table[ii] && x <= x_table[ii + 1] {
            return ((y_table[ii + 1] - y_table[ii]) / (x_table[ii + 1] - x_table[ii]))
                * (x - x_table[ii])
                + y_table[ii];
        }
    }
    y_table[i_left]
}

/// `TGE_WTG3_Model` — every Pascal field, snake-cased. Grouped as in the Pascal
/// record.
#[derive(Debug, Clone)]
pub struct Wtg3Model {
    // filter time constants
    pub tflt_pqm: f64,
    pub tflt_vfbk: f64,
    pub vmeas_max: f64,
    pub imeas_max: f64,
    // PLL
    pub kp_pll: f64,
    pub ki_pll: f64,
    pub d_omg_lim: f64,
    pub vd_pos: f64,
    pub vq_pos: f64,
    pub vd_neg: f64,
    pub vq_neg: f64,
    pub vd_fbk_pos: f64,
    pub vq_fbk_pos: f64,
    pub vd_fbk_neg: f64,
    pub vq_fbk_neg: f64,
    pub id_pos: f64,
    pub iq_pos: f64,
    pub id_neg: f64,
    pub iq_neg: f64,
    pub d_omg: f64,
    pub vang: f64,
    // PQ priority control
    pub qord_max: f64,
    pub qord_min: f64,
    pub iphl: f64,
    pub iqhl: f64,
    pub imax_td: f64,
    pub tflt_iqmxv_up: f64,
    pub tflt_iqmxv_dn: f64,
    pub iqmxv: f64,
    pub ipmx: f64,
    pub iqmx: f64,
    pub ipmn: f64,
    pub iqmn: f64,
    // Voltage regulator
    pub kp_vreg: f64,
    pub ki_vreg: f64,
    pub vref: f64,
    pub err_vmag: f64,
    // LVPL logic
    pub tflt_vmag_lvpl: f64,
    pub tflt_pplv_lim0: f64,
    pub v0_lvpl: f64,
    pub p0_lvpl: f64,
    pub v1_lvpl: f64,
    pub p1_lvpl: f64,
    pub max_trq: f64,
    pub tflt_pplv_lim_up: f64,
    pub tflt_pplv_lim_dn: f64,
    pub vmag_lvpl: f64,
    pub pplv_lim0: f64,
    pub pplv_lim: f64,
    // LVQL logic
    pub tflt_vmag_lvql: f64,
    pub v0_lvql: f64,
    pub i0_lvql: f64,
    pub v1_lvql: f64,
    pub i1_lvql: f64,
    pub tflt_iqlv_lim_up: f64,
    pub tflt_iqlv_lim_dn: f64,
    pub iq_lim_asym_flt: f64,
    pub vmag_lvql: f64,
    pub iqlv_lim: f64,
    // Ireg
    pub tflt_icmd_pos: f64,
    pub kp_ireg_pos: f64,
    pub ki_ireg_pos: f64,
    pub rrl_iq_cmd: f64,
    pub kp_ireg_neg: f64,
    pub ki_ireg_neg: f64,
    pub ang_ireg_neg: f64,
    pub d_e2_lim: f64,
    pub e2mag_lim: f64,
    pub id_cmd_pos: f64,
    pub iq_cmd_pos: f64,
    pub id_cmd_neg: f64,
    pub iq_cmd_neg: f64,
    pub iplv: f64,
    pub iqlv: f64,
    pub err_id_pos: f64,
    pub err_iq_pos: f64,
    pub err_id_neg: f64,
    pub err_iq_neg: f64,
    // fault detection
    pub vthrs_asym_flt: f64,
    pub tthrs_asym_flt: f64,
    pub asym_flt_flag: i32,
    pub tmr_asym_flt: f64,
    pub under_speed_trip: i32,
    pub wtg_trip: i32,
    pub user_trip: i32,
    // Aerodynamic model
    pub kb_aero: f64,
    pub half_rho_ar_aero: f64,
    pub alpha_aero: [[f64; 5]; 5],
    pub wt_opt: f64,
    pub pmech_max: f64,
    // torque regulator
    pub wt_ref_min: f64,
    pub wt_ref_max: f64,
    pub tflt_wt_ref: f64,
    pub kp_trq_reg: f64,
    pub ki_trq_reg: f64,
    pub trq_ref_max: f64,
    pub trq_ref_min: f64,
    pub tflt_pinp: f64,
    pub pinp_max: f64,
    pub pinp_min: f64,
    pub rrl_pinp: f64,
    pub tflt_err_pinp: f64,
    pub wt_ref: f64,
    pub err_wt: f64,
    pub err_wt_old: f64,
    pub pinp1: f64,
    pub pinp: f64,
    pub trq_ref: f64,
    pub err_pinp: f64,
    pub err_pinp_flt: f64,
    // pitch control
    pub kp_pitch_ctrl: f64,
    pub ki_pitch_ctrl: f64,
    pub kp_pitch_comp: f64,
    pub ki_pitch_comp: f64,
    pub theta_pitch_max: f64,
    pub theta_pitch_min: f64,
    pub tflt_pitch: f64,
    pub rrl_theta_pitch: f64,
    pub theta_pitch: f64,
    pub theta_pitch0: f64,
    pub err_pstl: f64,
    pub pmech: f64,
    pub pmech_avl: f64,
    // active power control (APC)
    pub tflt_pavl_apc: f64,
    pub frq_table_apc: [f64; 5],
    pub pwr_table_apc: [f64; 5],
    pub tflt_pset_apc: f64,
    pub tdelay_apc: f64,
    pub pavl_apc: f64,
    pub pset_apc: f64,
    pub pade_apc: f64,
    pub pstl: f64,
    // wind inertia
    pub db_wind_inertia: f64,
    pub tflt_d_frq_wind_inertia: f64,
    pub k_wind_inertia: f64,
    pub tflt_d_pinp_wind_inertia: f64,
    pub d_pinp_max: f64,
    pub d_pinp_min: f64,
    pub rru_d_pinp: f64,
    pub rrd_d_pinp: f64,
    pub d_frq_pu_test: f64,
    pub d_frq_wind_inertia: f64,
    pub y3_lpf: f64,
    pub d_pinp_wind_inertia: f64,
    // swing model
    pub wt_base: f64,
    pub hwtg: f64,
    pub dshaft: f64,
    pub wt: f64,
    pub d_wt: f64,
    // regulator output
    pub d_emax: f64,
    pub d_emin: f64,
    pub ed_pos: f64,
    pub eq_pos: f64,
    pub ed_neg: f64,
    pub eq_neg: f64,
    // integrator
    pub intg_x: [f64; 12],
    pub intg_d: [f64; 12],
    pub intg_d_old: [f64; 12],
    pub debug_trace: bool,
    // ratings
    pub rated_hz: f64,
    pub rated_kva: f64,
    pub rated_omg: f64,
    pub rated_kvll: f64,
    pub rated_vln: f64,
    pub rated_amp: f64,
    // Active and reactive power regulator
    pub v1_volt_var: f64,
    pub v2_volt_var: f64,
    pub v3_volt_var: f64,
    pub v4_volt_var: f64,
    pub q1_volt_var: f64,
    pub q2_volt_var: f64,
    pub q3_volt_var: f64,
    pub q4_volt_var: f64,
    pub v_curve_volt_var: [f64; 6],
    pub q_curve_volt_var: [f64; 6],
    pub qref: f64,
    pub pf_ref: f64,
    pub rrl_qcmd: f64,
    pub pord_max: f64,
    pub pord_min: f64,
    pub pcurtail: f64,
    pub pord: f64,
    pub pcmd: f64,
    pub kp_qreg: f64,
    pub ki_qreg: f64,
    pub vref_min: f64,
    pub vref_max: f64,
    pub qcmd: f64,
    pub err_qgen: f64,
    // simulation time setup
    pub tsim: f64,
    pub delt_sim: f64,
    pub delt0: f64,
    pub delt: f64,
    pub n_rec: i32,
    pub n_iter_lf: i32,
    pub q_mode: i32,
    pub q_flg: i32,
    // active power control flag
    pub apc_flg: i32,
    // simulate mechanical system
    pub sim_mech_flg: i32,
    // number of WTG
    pub n_wtg: i32,
    // terminal impedance
    pub zthev: Complex64,
    // terminal voltage/current (phase arrays 1..3 → 0..2)
    pub vabc: [Complex64; 3],
    pub iabc: [Complex64; 3],
    pub eabc: [Complex64; 3],
    // sequence components 0..2
    pub v012: [Complex64; 3],
    pub i012: [Complex64; 3],
    pub e012: [Complex64; 3],
    pub vmag: f64,
    pub vmag_min: f64,
    pub emag: f64,
    pub eang: f64,
    pub sele: Complex64,
    pub pele: f64,
    pub qele: f64,
    pub pgen: f64,
    pub qgen: f64,
    // steady state conditions for initialization
    pub vss: f64,
    pub pss: f64,
    pub qss: f64,
    // wind speed
    pub vwind: f64,
}

impl Wtg3Model {
    /// Pascal `TGE_WTG3_Model.Initialize` — the constant-parameter defaults. The
    /// derived quantities are (re)computed by [`Self::recalc_element_data`],
    /// which `Initialize` calls last (here folded into `new`).
    pub fn new() -> Self {
        let rated_kva = 3600.0;
        let m = Self {
            delt0: 0.000050,
            rated_hz: 60.0,
            rated_kva,
            rated_kvll: 0.69,
            n_wtg: 1,
            vss: 1.0,
            pss: 1.0,
            qss: 0.0,
            vwind: 14.0,
            zthev: Complex64::new(0.0, 0.05),
            sim_mech_flg: 1,
            apc_flg: 0,
            q_flg: 1,
            tflt_pqm: 0.02,
            tflt_vfbk: 0.001,
            vmeas_max: 2.0,
            imeas_max: 2.0,
            kp_pll: 60.0,
            ki_pll: 300.0,
            qord_max: 0.436,
            qord_min: -0.436,
            iphl: 1.24,
            iqhl: 1.25,
            imax_td: 1.25,
            tflt_iqmxv_up: 0.016,
            tflt_iqmxv_dn: 0.160,
            pord_min: 0.0,
            pord_max: 1.12,
            pcurtail: 1.12,
            q_mode: 0,
            v1_volt_var: 0.92,
            v2_volt_var: 0.98,
            v3_volt_var: 1.02,
            v4_volt_var: 1.08,
            q1_volt_var: 0.44,
            q2_volt_var: 0.0,
            q3_volt_var: 0.0,
            q4_volt_var: -0.44,
            rrl_qcmd: 0.2,
            kp_qreg: 0.0,
            ki_qreg: 0.2,
            vref_max: 1.1,
            vref_min: 0.9,
            kp_vreg: 0.0,
            ki_vreg: 40.0,
            tflt_vmag_lvpl: 0.002,
            tflt_pplv_lim0: 0.01,
            v0_lvpl: 0.4875,
            p0_lvpl: 0.0,
            v1_lvpl: 0.9,
            p1_lvpl: 1.13625,
            // MaxTrq is recomputed in ReCalcElementData; seed with the Initialize form.
            max_trq: (rated_kva * 1000.0 / 1454.0 / 2.0 / PI * 60.0) * 1.1931,
            tflt_pplv_lim_up: 0.160,
            tflt_pplv_lim_dn: 0.016,
            tflt_vmag_lvql: 0.01,
            v0_lvql: 0.5,
            i0_lvql: 0.9,
            v1_lvql: 0.9,
            i1_lvql: 0.79,
            tflt_iqlv_lim_up: 0.016,
            tflt_iqlv_lim_dn: 0.160,
            iq_lim_asym_flt: 0.447,
            tflt_icmd_pos: 0.002,
            rrl_iq_cmd: 0.5,
            ang_ireg_neg: 65.0 * PI / 180.0,
            d_e2_lim: 0.05,
            e2mag_lim: 0.105,
            d_emax: 0.1,
            d_emin: -0.1,
            vthrs_asym_flt: 30.0 / (0.69 * 1000.0 * 2.0_f64.sqrt() / 3.0_f64.sqrt()),
            tthrs_asym_flt: 0.03,
            kb_aero: 69.5,
            half_rho_ar_aero: 0.00145,
            alpha_aero: [
                [-0.41909, 0.21808, -0.012406, -0.00013365, 0.000011524],
                [-0.067606, 0.060405, -0.013934, 0.0010683, -0.000023895],
                [0.015727, -0.010996, 0.0021495, -0.00014855, 2.7937E-06],
                [
                    -0.00086018,
                    0.00057051,
                    -0.00010479,
                    5.9924E-06,
                    -8.9194E-08,
                ],
                [
                    0.000014787,
                    -9.4839E-06,
                    1.6167E-06,
                    -7.1535E-08,
                    4.9686E-10,
                ],
            ],
            wt_ref_min: 0.0,
            wt_ref_max: 1.2,
            tflt_wt_ref: 60.0,
            kp_trq_reg: 3.0,
            ki_trq_reg: 0.6,
            trq_ref_max: 1.2,
            trq_ref_min: 0.08,
            tflt_pinp: 0.05,
            pinp_max: 1.12,
            pinp_min: 0.04,
            rrl_pinp: 0.45,
            tflt_err_pinp: 1.0,
            kp_pitch_ctrl: 150.0,
            ki_pitch_ctrl: 25.0,
            kp_pitch_comp: 3.0,
            ki_pitch_comp: 30.0,
            theta_pitch_max: 27.0,
            theta_pitch_min: 0.0,
            tflt_pitch: 0.3,
            rrl_theta_pitch: 10.0,
            tflt_pavl_apc: 0.15,
            frq_table_apc: [0.96, 0.996, 1.004, 1.04, 1.0662],
            pwr_table_apc: [1.0, 0.95, 0.95, 0.40, 0.0],
            tflt_pset_apc: 5.0,
            tdelay_apc: 0.15,
            db_wind_inertia: 0.0025,
            tflt_d_frq_wind_inertia: 1.0,
            k_wind_inertia: 10.0,
            tflt_d_pinp_wind_inertia: 5.5,
            d_pinp_max: 0.5,
            d_pinp_min: 0.0,
            rru_d_pinp: 0.1,
            rrd_d_pinp: 1.0,
            hwtg: 5.23,
            dshaft: 0.0,
            debug_trace: false,
            // zero-initialized (Pascal leaves these to Init / the derived recalc)
            rated_omg: 0.0,
            rated_vln: 0.0,
            rated_amp: 0.0,
            d_omg_lim: 0.0,
            vd_pos: 0.0,
            vq_pos: 0.0,
            vd_neg: 0.0,
            vq_neg: 0.0,
            vd_fbk_pos: 0.0,
            vq_fbk_pos: 0.0,
            vd_fbk_neg: 0.0,
            vq_fbk_neg: 0.0,
            id_pos: 0.0,
            iq_pos: 0.0,
            id_neg: 0.0,
            iq_neg: 0.0,
            d_omg: 0.0,
            vang: 0.0,
            iqmxv: 0.0,
            ipmx: 0.0,
            iqmx: 0.0,
            ipmn: 0.0,
            iqmn: 0.0,
            vref: 0.0,
            err_vmag: 0.0,
            vmag_lvpl: 0.0,
            pplv_lim0: 0.0,
            pplv_lim: 0.0,
            vmag_lvql: 0.0,
            iqlv_lim: 0.0,
            kp_ireg_pos: 0.0,
            ki_ireg_pos: 0.0,
            kp_ireg_neg: 0.0,
            ki_ireg_neg: 0.0,
            id_cmd_pos: 0.0,
            iq_cmd_pos: 0.0,
            id_cmd_neg: 0.0,
            iq_cmd_neg: 0.0,
            iplv: 0.0,
            iqlv: 0.0,
            err_id_pos: 0.0,
            err_iq_pos: 0.0,
            err_id_neg: 0.0,
            err_iq_neg: 0.0,
            asym_flt_flag: 0,
            tmr_asym_flt: 0.0,
            under_speed_trip: 0,
            wtg_trip: 0,
            user_trip: 0,
            wt_opt: 0.0,
            pmech_max: 0.0,
            wt_ref: 0.0,
            err_wt: 0.0,
            err_wt_old: 0.0,
            pinp1: 0.0,
            pinp: 0.0,
            trq_ref: 0.0,
            err_pinp: 0.0,
            err_pinp_flt: 0.0,
            theta_pitch: 0.0,
            theta_pitch0: 0.0,
            err_pstl: 0.0,
            pmech: 0.0,
            pmech_avl: 0.0,
            pavl_apc: 0.0,
            pset_apc: 0.0,
            pade_apc: 0.0,
            pstl: 0.0,
            d_frq_pu_test: 0.0,
            d_frq_wind_inertia: 0.0,
            y3_lpf: 0.0,
            d_pinp_wind_inertia: 0.0,
            wt_base: 0.0,
            wt: 0.0,
            d_wt: 0.0,
            ed_pos: 0.0,
            eq_pos: 0.0,
            ed_neg: 0.0,
            eq_neg: 0.0,
            intg_x: [0.0; 12],
            intg_d: [0.0; 12],
            intg_d_old: [0.0; 12],
            v_curve_volt_var: [0.0; 6],
            q_curve_volt_var: [0.0; 6],
            qref: 0.0,
            pf_ref: 0.0,
            pord: 0.0,
            pcmd: 0.0,
            qcmd: 0.0,
            err_qgen: 0.0,
            tsim: 0.0,
            delt_sim: 0.0,
            delt: 0.0,
            n_rec: 0,
            n_iter_lf: 100,
            vabc: [Complex64::ZERO; 3],
            iabc: [Complex64::ZERO; 3],
            eabc: [Complex64::ZERO; 3],
            v012: [Complex64::ZERO; 3],
            i012: [Complex64::ZERO; 3],
            e012: [Complex64::ZERO; 3],
            vmag: 0.0,
            vmag_min: 0.0,
            emag: 0.0,
            eang: 0.0,
            sele: Complex64::ZERO,
            pele: 0.0,
            qele: 0.0,
            pgen: 0.0,
            qgen: 0.0,
        };
        // Pascal `TGE_WTG3_Model.Create` ends with `ReCalcElementData`, which reads
        // the LIVE `DynaData^.h`/`t` — `DynaData` points at
        // `ActiveCircuit.Solution.DynaVars` (WindGen.pas:1018), so Create reads the
        // solution's live step size / time. `new` has no solution; the WindGen
        // owner runs that live recalc via its own `RecalcElementData`
        // (`recalc_element_data(sys.dyna_h, sys.dyna_t)`), which the executive
        // invokes at create (`create_object_no_edit`) and at `end_edit`.
        // Direct-construction unit tests seed it explicitly.
        m
    }

    /// Pascal `TGE_WTG3_Model.ReCalcElementData` — derived ratings, current-reg
    /// gains, the volt-var curve, the turbine speed base and the sub-cycle step
    /// count from the outer step `h`.
    pub fn recalc_element_data(&mut self, h: f64, t: f64) {
        self.rated_omg = 2.0 * PI * self.rated_hz;
        self.rated_vln = self.rated_kvll / 3.0_f64.sqrt() * 1000.0;
        self.rated_amp = self.rated_kva * 1000.0 / self.rated_vln / 3.0;
        self.max_trq = (self.rated_kva * 1000.0 / 1454.0 / 2.0 / PI * 60.0) * 1.1931;
        self.d_omg_lim = 0.2 * self.rated_omg;

        // current regulator parameters
        self.kp_ireg_pos = 0.9 * self.zthev.im;
        self.ki_ireg_pos = 25.0 * self.kp_ireg_pos;
        self.kp_ireg_neg = self.kp_ireg_pos * 1.5;
        self.ki_ireg_neg = self.ki_ireg_pos * 1.5;

        // volt-var curve
        self.v_curve_volt_var[0] = 0.0_f64.max(self.v1_volt_var - 0.2);
        self.v_curve_volt_var[1] = self.v1_volt_var;
        self.v_curve_volt_var[2] = self.v2_volt_var;
        self.v_curve_volt_var[3] = self.v3_volt_var;
        self.v_curve_volt_var[4] = self.v4_volt_var;
        self.v_curve_volt_var[5] = 2.0_f64.min(self.v4_volt_var + 0.2);
        self.q_curve_volt_var[0] = self.q1_volt_var;
        self.q_curve_volt_var[1] = self.q1_volt_var;
        self.q_curve_volt_var[2] = self.q2_volt_var;
        self.q_curve_volt_var[3] = self.q3_volt_var;
        self.q_curve_volt_var[4] = self.q4_volt_var;
        self.q_curve_volt_var[5] = self.q4_volt_var;

        // turbine rotation speed base
        self.wt_base = 2.0 * PI * (self.rated_hz / 3.0);

        // 1.5MW parameters
        if self.rated_kva < 2000.0 {
            self.hwtg = 4.94;
            self.kb_aero = 56.6;
            self.half_rho_ar_aero = 0.00159;
        }

        // time steps
        self.delt_sim = h;
        self.n_rec = ((h / self.delt0 / 2.0).trunc() as i64 * 2 + 1) as i32;
        self.delt = h / self.n_rec as f64;
        self.tsim = t;
        self.n_iter_lf = 100;
    }

    /// Pascal `abc2seq`: phase→sequence (`Phase2SymComp`) then rotate every
    /// component by `-ang`.
    fn abc2seq(abc: &[Complex64; 3], seq: &mut [Complex64; 3], ang: f64) {
        let sc = SymComp::default();
        sc.phase_to_sym(abc, seq);
        let temp = Complex64::new((-ang).cos(), (-ang).sin());
        for s in seq.iter_mut() {
            *s *= temp;
        }
    }

    /// Pascal `seq2abc`: sequence→phase (`SymComp2Phase`) then rotate every phase
    /// by `+ang`.
    fn seq2abc(abc: &mut [Complex64; 3], seq: &[Complex64; 3], ang: f64) {
        let sc = SymComp::default();
        sc.sym_to_phase(seq, abc);
        let temp = Complex64::new(ang.cos(), ang.sin());
        for a in abc.iter_mut() {
            *a *= temp;
        }
    }

    /// Pascal `Instrumentation`: per-unitize the terminal V/I, extract the
    /// sequence components (zero-seq removed from voltage), and low-pass the
    /// measured P/Q.
    fn instrumentation(&mut self, v: &[Complex64], i: &[Complex64]) {
        for ii in 0..3 {
            self.vabc[ii] = mag_limiter(v[ii] / self.rated_vln, 0.0, self.vmeas_max);
            // Iabc = -(I/AmpBase + Vabc/ZThev)
            self.iabc[ii] = mag_limiter(
                (i[ii] / -self.rated_amp * self.n_wtg as f64) - (self.vabc[ii] / self.zthev),
                0.0,
                self.imeas_max,
            );
        }

        let vang = self.vang;
        let mut v012 = self.v012;
        let mut i012 = self.i012;
        Self::abc2seq(&self.vabc, &mut v012, vang);
        Self::abc2seq(&self.iabc, &mut i012, vang);
        self.v012 = v012;
        self.i012 = i012;

        // get rid of zero-sequence component in voltage
        self.v012[0] = Complex64::ZERO;
        let v012 = self.v012;
        let mut vabc = self.vabc;
        Self::seq2abc(&mut vabc, &v012, vang);
        self.vabc = vabc;

        // voltage magnitude
        self.vmag = self.v012[1].norm();

        // minimum voltage for fault ride-through
        self.vmag_min = self.vabc[0]
            .norm()
            .min(self.vabc[1].norm())
            .min(self.vabc[2].norm());

        // output power
        self.sele = Complex64::ZERO;
        for ii in 0..3 {
            self.sele += (self.vabc[ii] * self.iabc[ii].conj()) / 3.0;
        }
        self.pele = self.sele.re;
        self.qele = self.sele.im;

        let ktemp = 1.0_f64.min(self.delt_sim / self.tflt_pqm);
        self.pgen += (self.pele - self.pgen) * ktemp;
        self.qgen += (self.qele - self.qgen) * ktemp;
    }

    /// Pascal `CalcPFlow` (used only by [`Self::init`] to seed `Emag`/`Eang`).
    fn calc_pflow(&mut self, v: &[Complex64], i: &mut [Complex64]) {
        self.instrumentation(v, i);

        let vtemp: Complex64;
        if self.n_iter_lf == 1 {
            vtemp = (self.v012[1] / 0.000001_f64.max(self.v012[1].norm())) * self.vss;
            self.emag = vtemp.norm();
            self.eang = cang(vtemp);
        } else {
            vtemp = self.v012[1];
        }
        let itemp = (Complex64::new(self.pss, self.qss) / vtemp).conj();
        let etemp = vtemp + self.zthev * itemp;
        if self.n_iter_lf < 10 {
            let k_cnvg = 0.4_f64.max(1.0_f64.min(1.0 - (self.n_iter_lf - 1) as f64 * 0.1));
            self.emag = 0.0_f64.max(2.0_f64.min(self.emag + (etemp.norm() - self.emag) * k_cnvg));
            self.eang += (cang(etemp) - self.eang) * k_cnvg;
        }
        self.n_iter_lf += 1;

        // update output
        self.e012[0] = Complex64::ZERO;
        self.e012[1] = Complex64::new(self.emag * self.eang.cos(), self.emag * self.eang.sin());
        self.e012[2] = Complex64::ZERO;
        self.calc_current(i);
    }

    /// Pascal `Init` — dynamics-mode initialization: solve the steady operating
    /// point (MPPT / pitch), run one load-flow seed, then set every control state.
    pub fn init(&mut self, v: &[Complex64], i: &mut [Complex64]) {
        // check available wind power and update the initial power condition
        self.aero_mppt();
        if self.pss > self.pmech_max {
            // not enough wind power to support PSS, update PSS
            self.pss = self.pmech_max;
            self.wt = self.wt_opt;
        } else {
            self.wt = self.calc_wt_ref(self.pss);
        }
        // iterate for thetaPitch
        self.theta_pitch = self.theta_pitch_max / 2.0;
        let e_iter = 0.01;
        let k_iter = 10.0;
        for _ in 1..=10 {
            self.aero_dynamic();
            if (self.pmech - self.pss).abs() < e_iter {
                break;
            } else {
                self.theta_pitch += k_iter * (self.pmech - self.pss);
                self.theta_pitch = self
                    .theta_pitch_max
                    .min(self.theta_pitch_min.max(self.theta_pitch));
            }
        }

        // run a load flow
        self.n_iter_lf = 1;
        self.calc_pflow(v, i);

        // initialize control variables
        self.vd_fbk_pos = self.vss;
        self.vq_fbk_pos = 0.0;
        self.vd_fbk_neg = 0.0;
        self.vq_fbk_neg = 0.0;
        self.d_omg = 0.0;
        self.vang = cang(self.vabc[0]);
        self.vq_pos = 0.0;
        self.pplv_lim0 = self.p1_lvpl;
        self.pplv_lim = self.pplv_lim0;
        self.iqlv_lim = self.i0_lvql;
        self.vmag_lvpl = self.vd_fbk_pos;
        self.vmag_lvql = self.vd_fbk_pos;
        self.pord = self.pss;
        self.pgen = self.pss;
        self.id_cmd_pos = self.pss / self.vss;
        self.iplv = self.id_cmd_pos;
        if self.q_mode == 0 || self.q_mode == 1 {
            self.qcmd = self.qss;
        } else {
            self.qcmd = linear_interp(&self.v_curve_volt_var, &self.q_curve_volt_var, self.vss);
        }
        self.pf_ref =
            self.pss.abs() / 0.000001_f64.max((self.pss * self.pss + self.qss * self.qss).sqrt());
        if self.qss < 0.0 {
            self.pf_ref = -self.pf_ref;
        }
        self.qgen = self.qcmd;
        self.err_qgen = 0.0;
        self.vref = self.vss;
        self.iq_cmd_pos = -self.qcmd / self.vss;
        self.iqlv = self.iq_cmd_pos;
        self.iqmxv = self.qord_max / self.vss;
        self.err_vmag = 0.0;
        self.err_id_pos = 0.0;
        self.err_iq_pos = 0.0;
        self.ed_pos = self.emag * self.eang.cos();
        self.eq_pos = self.emag * self.eang.sin();
        // negative sequence current regulator
        self.err_id_neg = 0.0;
        self.err_iq_neg = 0.0;
        self.ed_neg = 0.0;
        self.eq_neg = 0.0;
        // fault detection
        self.under_speed_trip = 0;
        self.wtg_trip = 0;
        self.asym_flt_flag = 0;
        self.tmr_asym_flt = 0.0;
        // torque regulator
        self.wt_ref = self.wt;
        self.err_wt = 0.0;
        self.pinp1 = self.pss;
        self.pinp = self.pss;
        self.trq_ref = self.pss / self.wt;
        self.err_pinp = 0.0;
        self.err_pinp_flt = 0.0;
        // pitch control
        self.err_pstl = 0.0;
        self.theta_pitch0 = self.theta_pitch;
        // active power control
        self.pavl_apc = self.pss;
        self.pset_apc = self.pss;
        self.pade_apc = self.pss;
        self.pstl = self.pss;
        // wind inertia
        self.d_frq_pu_test = 0.0;
        self.d_frq_wind_inertia = 0.0;
        self.y3_lpf = 0.0;
        self.d_pinp_wind_inertia = 0.0;
        // swing model
        self.d_wt = self.wt - 1.0;

        // initialize integrator
        for ii in 0..12 {
            self.intg_x[ii] = 0.0;
            self.intg_d[ii] = 0.0;
            self.intg_d_old[ii] = 0.0;
        }
        self.intg_x[0] = self.d_omg;
        self.intg_x[1] = self.vang;
        self.intg_x[2] = 0.0;
        self.intg_x[3] = 0.0;
        self.intg_x[4] = 0.0;
        self.intg_x[5] = 0.0;
        self.intg_x[6] = self.iq_cmd_pos;
        self.intg_x[7] = self.vref;
        self.intg_x[8] = self.trq_ref;
        self.intg_x[9] = self.theta_pitch;
        self.intg_x[10] = 0.0;
        self.intg_x[11] = self.d_wt;
    }

    /// Pascal `PllLogic`.
    fn pll_logic(&mut self, h: f64) {
        let vq_pos_old = self.vq_pos;
        self.vd_pos = self.v012[1].re;
        self.vq_pos = self.v012[1].im;
        let drv_temp =
            self.ki_pll * self.vq_pos + self.kp_pll * (self.vq_pos - vq_pos_old) / self.delt_sim;
        self.d_omg =
            (-self.d_omg_lim).max(self.d_omg_lim.min(self.d_omg + drv_temp * self.delt_sim));
        // integrator (dFrq to Vang)
        self.vang += self.d_omg * self.delt_sim;
        // other sequence components
        self.vd_neg = self.v012[2].re;
        self.vq_neg = -self.v012[2].im;
        self.id_pos = self.i012[1].re;
        self.iq_pos = self.i012[1].im;
        self.id_neg = self.i012[2].re;
        self.iq_neg = -self.i012[2].im;
        // LPF on voltage feedback
        let k_flt_temp = 1.0_f64.min(h / self.tflt_vfbk);
        self.vd_fbk_pos += (self.vd_pos - self.vd_fbk_pos) * k_flt_temp;
        self.vq_fbk_pos += (self.vq_pos - self.vq_fbk_pos) * k_flt_temp;
        self.vd_fbk_neg += (self.vd_neg - self.vd_fbk_neg) * k_flt_temp;
        self.vq_fbk_neg += (self.vq_neg - self.vq_fbk_neg) * k_flt_temp;
    }

    /// Pascal `PQPriority`.
    fn pq_priority(&mut self, pq_flag: i32) {
        let y0 = self
            .iqhl
            .min(self.qord_max.max((self.qord_max - 2.15) * self.vmag + 2.15));
        let temp = if y0 > self.iqmxv {
            1.0_f64.min(self.delt / self.tflt_iqmxv_up)
        } else {
            1.0_f64.min(self.delt / self.tflt_iqmxv_dn)
        };
        self.iqmxv += (y0 - self.iqmxv) * temp;
        self.iqmxv = self.iqhl.min(0.0_f64.max(self.iqmxv));
        if pq_flag == 1 {
            // P priority
            self.ipmx = self.imax_td.min(self.iphl);
            self.iqmx = self
                .iqhl
                .min(0.0_f64.max(self.imax_td.powi(2) - self.iplv.powi(2)).sqrt());
        } else {
            self.iqmx = self.imax_td.min(self.iqmxv);
            self.ipmx = self
                .iphl
                .min(0.0_f64.max(self.imax_td.powi(2) - self.iqlv.powi(2)).sqrt());
        }
        self.ipmn = -self.ipmx;
        self.iqmn = -self.iqmx;
    }

    /// Pascal `LVPL`.
    fn lvpl(&mut self) {
        let mut temp = 1.0_f64.min(self.delt / self.tflt_vmag_lvpl);
        self.vmag_lvpl += (self.vmag_min - self.vmag_lvpl) * temp;
        // LVPL curve interpolation
        let mut y0 = (self.p1_lvpl - self.p0_lvpl) / (self.v1_lvpl - self.v0_lvpl)
            * (self.vmag_lvpl - self.v0_lvpl)
            + self.p0_lvpl;
        y0 = self.p0_lvpl.max(self.p1_lvpl.min(y0));
        y0 += 0.02;
        // upper limit to y0
        let y0_lim = if self.asym_flt_flag == 1 {
            0.75
        } else {
            0.5_f64.max(2.0_f64.min(self.pord))
        };
        let mut y1 = y0_lim.min(y0);
        // subtraction term to y1
        let y1_sub = if self.vmag < (0.91 - 0.05) && self.vmag > (0.65 - 0.05) {
            0.2
        } else {
            0.0
        };
        y1 = 0.0_f64.max(y1 - y1_sub);
        // limit on torque
        let y2 = (self.max_trq * self.wt * self.wt_base / self.rated_kva / 1000.0).min(y1);
        // low pass filter
        temp = if y2 > self.pplv_lim {
            1.0_f64.min(self.delt / self.tflt_pplv_lim_up)
        } else {
            1.0_f64.min(self.delt / self.tflt_pplv_lim_dn)
        };
        self.pplv_lim += (y2 - self.pplv_lim) * temp;
    }

    /// Pascal `LVQL`.
    fn lvql(&mut self) {
        let mut temp = 1.0_f64.min(self.delt / self.tflt_vmag_lvql);
        self.vmag_lvql += (self.vmag_min - self.vmag_lvql) * temp;
        // LVQL curve interpolation
        let y0 = (self.i1_lvql - self.i0_lvql) / (self.v1_lvql - self.v0_lvql)
            * (self.vmag_lvql - self.v0_lvql)
            + self.i0_lvql;
        let y0 = self.i0_lvql.min(self.i1_lvql.max(y0));
        // low pass filter
        temp = if y0 > self.iqlv_lim {
            1.0_f64.min(self.delt / self.tflt_iqlv_lim_up)
        } else {
            1.0_f64.min(self.delt / self.tflt_iqlv_lim_dn)
        };
        self.iqlv_lim += (y0 - self.iqlv_lim) * temp;
        if self.asym_flt_flag == 1 {
            self.iqlv_lim = self.iq_lim_asym_flt;
        }
    }

    /// Pascal `RealPowerReg`.
    fn real_power_reg(&mut self) {
        self.pcmd = self
            .pord_min
            .max(self.pord_max.min(self.pplv_lim.min(self.pord)));
        self.pcmd = self.pcurtail.min(self.pcmd);
        self.id_cmd_pos = self.pcmd / 0.000001_f64.max(self.vmag);
        self.id_cmd_pos = self.ipmn.max(self.ipmx.min(self.id_cmd_pos));
    }

    /// Pascal `ReactivePowerReg`.
    fn reactive_power_reg(&mut self) {
        let qord = if self.q_mode == 0 {
            // constant Q mode
            self.qref
        } else if self.q_mode == 1 {
            // constant PF mode (negative means absorption)
            let mut q = (1.0 - self.pf_ref * self.pf_ref).sqrt()
                / 0.000001_f64.max(self.pf_ref.abs())
                * self.pgen;
            if self.pf_ref < 0.0 {
                q = -q;
            }
            q
        } else {
            // volt-var mode
            linear_interp(&self.v_curve_volt_var, &self.q_curve_volt_var, self.vmag)
        };
        // hard limiter on Qord
        let qord = self.qord_max.min(self.qord_min.max(qord));
        // ramp rate limiter on Qcmd
        let temp = self.rrl_qcmd * self.delt;
        self.qcmd = (self.qcmd + temp).min((self.qcmd - temp).max(qord));

        // reactive power regulator
        let err_qgen_old = self.err_qgen;
        self.err_qgen = self.qcmd - self.qgen;
        self.intg_d[7] = self.ki_qreg * self.err_qgen
            + self.kp_qreg * (self.err_qgen - err_qgen_old) / self.delt;
        self.intg_x[7] = self.vref_min.max(self.vref_max.min(self.intg_x[7]));
        self.vref = self.intg_x[7];
    }

    /// Pascal `VoltageReg`.
    fn voltage_reg(&mut self) {
        let err_vmag_old = self.err_vmag;
        self.err_vmag = -(self.vref - self.vmag);
        self.intg_d[6] = self.ki_vreg * self.err_vmag
            + self.kp_vreg * (self.err_vmag - err_vmag_old) / self.delt;
        self.intg_x[6] = self.iqmn.max(self.iqmx.min(self.intg_x[6]));
        self.iq_cmd_pos = self.intg_x[6];
    }

    /// Pascal `CurrentReg`.
    fn current_reg(&mut self) {
        let ktemp = 1.0_f64.min(self.delt / self.tflt_icmd_pos);
        self.iplv += (self.id_cmd_pos - self.iplv) * ktemp;
        self.iqlv = self
            .iqlv_lim
            .min((-self.iqlv_lim).max(self.iqlv + (self.iq_cmd_pos - self.iqlv) * ktemp));
        // anti windup
        if (self.iqlv == self.iqlv_lim && self.iq_cmd_pos > self.iqlv)
            || (self.iqlv == -self.iqlv_lim && self.iq_cmd_pos < self.iqlv)
        {
            self.intg_d[6] = 0.0;
            self.intg_d[7] = 0.0;
        }

        // PI regulator for IdPos
        let err_id_pos_old = self.err_id_pos;
        self.err_id_pos = self.iplv - self.id_pos;
        self.intg_d[2] = self.ki_ireg_pos * self.err_id_pos
            + self.kp_ireg_pos * (self.err_id_pos - err_id_pos_old) / self.delt;
        self.intg_x[2] = self.d_emin.max(self.d_emax.min(self.intg_x[2]));
        self.ed_pos = self.intg_x[2] + self.zthev.re * self.iplv - self.zthev.im * self.iqlv
            + self.vd_fbk_pos;
        // PI regulator for IqPos
        let err_iq_pos_old = self.err_iq_pos;
        self.err_iq_pos = self.iqlv - self.iq_pos;
        self.intg_d[3] = self.ki_ireg_pos * self.err_iq_pos
            + self.kp_ireg_pos * (self.err_iq_pos - err_iq_pos_old) / self.delt;
        self.intg_x[3] = self.d_emin.max(self.d_emax.min(self.intg_x[3]));
        self.eq_pos = self.intg_x[3]
            + self.zthev.re * self.iqlv
            + self.zthev.im * self.iplv
            + self.vq_fbk_pos;

        // negative sequence current regulator
        // careful with signs: E2=Ed-jEq, I2=Id-jIq
        self.id_cmd_neg = 0.0;
        self.iq_cmd_neg = 0.0;
        let err_id_neg_old = self.err_id_neg;
        self.err_id_neg = self.id_cmd_neg - self.id_neg;
        self.intg_d[4] = self.ki_ireg_neg * self.err_id_neg
            + self.kp_ireg_neg * (self.err_id_neg - err_id_neg_old) / self.delt;
        let err_iq_neg_old = self.err_iq_neg;
        self.err_iq_neg = self.iq_cmd_neg - self.iq_neg;
        self.intg_d[5] = self.ki_ireg_neg * self.err_iq_neg
            + self.kp_ireg_neg * (self.err_iq_neg - err_iq_neg_old) / self.delt;
        // limiter on integrator
        self.intg_x[4] = self.d_e2_lim.min((-self.d_e2_lim).max(self.intg_x[4]));
        self.intg_x[5] = self.d_e2_lim.min((-self.d_e2_lim).max(self.intg_x[5]));
        // angle rotation (for better damping)
        let d_e2_real =
            self.intg_x[4] * self.ang_ireg_neg.cos() + self.intg_x[5] * self.ang_ireg_neg.sin();
        let d_e2_imag =
            self.intg_x[4] * self.ang_ireg_neg.sin() - self.intg_x[5] * self.ang_ireg_neg.cos();
        // V2 feedforwarding and E2 magnitude limiting
        let mut temp_e2 = Complex64::new(self.vd_fbk_neg + d_e2_real, -self.vq_fbk_neg + d_e2_imag);
        temp_e2 = mag_limiter(temp_e2, 0.0, self.e2mag_lim);
        self.ed_neg = temp_e2.re;
        self.eq_neg = -temp_e2.im;
    }

    /// Pascal `Integrate` — trapezoidal advance of every integrator slot. Also
    /// invoked once per outer step by `WindGen::IntegrateStates` (the upstream
    /// side effect), hence `pub(super)`.
    pub(super) fn integrate(&mut self) {
        for ii in 0..12 {
            self.intg_x[ii] += self.delt / 2.0 * (self.intg_d_old[ii] + self.intg_d[ii]);
            self.intg_d_old[ii] = self.intg_d[ii];
        }
    }

    /// Pascal `CurrentLimiting` — cap the positive- and negative-sequence
    /// injection currents.
    fn current_limiting(&mut self) {
        self.e012[0] = Complex64::ZERO;
        // current limit on positive sequence
        self.e012[1] = Complex64::new(self.ed_pos, self.eq_pos);
        self.i012[1] = (self.e012[1] - self.v012[1]) / self.zthev;
        if self.i012[1].norm() > self.imax_td {
            self.i012[1] = (self.i012[1] / 0.000001_f64.max(self.i012[1].norm())) * self.imax_td;
            self.e012[1] = self.v012[1] + self.i012[1] * self.zthev;
        }
        // current limit on negative sequence
        self.e012[2] = Complex64::new(self.ed_neg, -self.eq_neg);
        self.i012[2] = (self.e012[2] - self.v012[2]) / self.zthev;
        let i2_max = 0.0_f64.max(1.1 - self.i012[1].norm());
        if self.i012[2].norm() > i2_max {
            self.i012[2] = (self.i012[2] / 0.000001_f64.max(self.i012[2].norm())) * i2_max;
            self.e012[2] = self.v012[2] + self.i012[2] * self.zthev;
        }
    }

    /// Pascal `FaultDetection`.
    fn fault_detection(&mut self) {
        if self.v012[2].norm() > self.vthrs_asym_flt {
            self.tmr_asym_flt += self.delt_sim;
        } else {
            self.tmr_asym_flt = 0.0;
            self.asym_flt_flag = 0;
        }
        if self.tmr_asym_flt > self.tthrs_asym_flt {
            self.asym_flt_flag = 1;
        }
        // under-speed fault
        if self.wt < 0.1 {
            self.under_speed_trip = 1;
        }
        // tripping of WTG
        if self.user_trip == 1 || self.under_speed_trip == 1 {
            self.wtg_trip = 1;
        }
    }

    /// Pascal `CalcCp` — the 5×5 aerodynamic performance polynomial.
    fn calc_cp(&self, theta: f64, lmbda: f64) -> f64 {
        let mut result = 0.0;
        for ii in 0..5 {
            for jj in 0..5 {
                result += self.alpha_aero[ii][jj] * theta.powi(ii as i32) * lmbda.powi(jj as i32);
            }
        }
        result
    }

    /// Pascal `CalcPmech`.
    fn calc_pmech(&self, theta: f64, wrotor: f64, spdwind: f64) -> f64 {
        let lmbda = 20.0_f64.min(0.0_f64.max(wrotor / 0.01_f64.max(spdwind) * self.kb_aero));
        let cp = self.calc_cp(theta, lmbda);
        1.2_f64.min(self.half_rho_ar_aero * spdwind.powi(3) * cp)
    }

    /// Pascal `AeroMPPT` — sweep `Wt` for the maximum-power rotor speed.
    fn aero_mppt(&mut self) {
        let mut wt_list = [0.0_f64; 101];
        let mut pmech_list = [0.0_f64; 101];
        let step_wt = (self.wt_ref_max - self.wt_ref_min) / 100.0;
        for ii in 0..=100 {
            let temp_wt = self.wt_ref_min + ii as f64 * step_wt;
            wt_list[ii] = temp_wt;
            pmech_list[ii] = self.calc_pmech(0.0, temp_wt, self.vwind);
        }
        self.pmech_max = -100000.0;
        let mut max_ii = 0;
        for (ii, &p) in pmech_list.iter().enumerate() {
            if p > self.pmech_max {
                max_ii = ii;
                self.pmech_max = p;
            }
        }
        self.wt_opt = wt_list[max_ii];
    }

    /// Pascal `AeroDynamic`.
    fn aero_dynamic(&mut self) {
        self.pmech = self.calc_pmech(self.theta_pitch, self.wt, self.vwind);
        self.pmech_avl = self.calc_pmech(0.001, self.wt, self.vwind);
    }

    /// Pascal `CalcWtRef`.
    fn calc_wt_ref(&self, ele_pwr: f64) -> f64 {
        let temp = 1.0_f64.min(ele_pwr);
        self.wt_ref_min.max(
            self.wt_ref_max
                .min(-0.75 * temp * temp + 1.59 * temp + 0.63),
        )
    }

    /// Pascal `TorqueReg`.
    fn torque_reg(&mut self) {
        let y1 = self.calc_wt_ref(self.pele);
        // low pass filter
        let mut temp = 1.0_f64.min(self.delt / self.tflt_wt_ref);
        self.wt_ref += (y1 - self.wt_ref) * temp;
        // PI regulator
        self.err_wt_old = self.err_wt;
        self.err_wt = self.wt - self.wt_ref;
        self.intg_d[8] = self.ki_trq_reg * self.err_wt
            + self.kp_trq_reg * (self.err_wt - self.err_wt_old) / self.delt;
        self.intg_x[8] = self.trq_ref_min.max(self.trq_ref_max.min(self.intg_x[8]));
        self.trq_ref = self.intg_x[8];
        // convert torque to power
        let y2 = self.trq_ref * self.wt;
        // low pass filter on Pinp
        temp = 1.0_f64.min(self.delt / self.tflt_pinp);
        self.pinp1 = self
            .pinp_max
            .min(self.pinp_min.max(self.pinp1 + (y2 - self.pinp1) * temp));
        // ramp rate limiter
        temp = self.rrl_pinp * self.delt;
        self.pinp = (self.pinp + temp).min((self.pinp - temp).max(self.pinp1));
        // power response rate limit
        let pinp_sat = self.pstl.min(0.0_f64.max(self.pinp));
        self.err_pinp = self.pinp - pinp_sat;
        // high pass filter on errPinp
        temp = 1.0_f64.min(self.delt / self.tflt_err_pinp);
        self.err_pinp_flt += (self.err_pinp - self.err_pinp_flt) * temp;
        let err_pinp_hpf = self.err_pinp - self.err_pinp_flt;
        // final Pord
        self.pord = pinp_sat + err_pinp_hpf + self.d_pinp_wind_inertia;
    }

    /// Pascal `PitchControl`.
    fn pitch_control(&mut self) {
        // Note: must run after TorqueReg (uses errWt / errWtOld).
        self.intg_d[9] = self.ki_pitch_ctrl * self.err_wt
            + self.kp_pitch_ctrl * (self.err_wt - self.err_wt_old) / self.delt;
        // PI regulator for pitch compensator
        let err_pstl_old = self.err_pstl;
        self.err_pstl = self.pinp - self.pstl;
        self.intg_d[10] = self.ki_pitch_comp * self.err_pstl
            + self.kp_pitch_comp * (self.err_pstl - err_pstl_old) / self.delt;
        // anti-windup
        let x1 = self.intg_d[9] + self.intg_d[10];
        let y1 = self.intg_x[9] + self.intg_x[10];
        if (y1 >= self.theta_pitch_max && x1 > 0.0) || (y1 <= self.theta_pitch_min && x1 < 0.0) {
            self.intg_d[9] = 0.0;
            self.intg_d[10] = 0.0;
        }
        let y2 = self.theta_pitch_min.max(self.theta_pitch_max.min(y1));
        // low pass filter
        let mut temp = 1.0_f64.min(self.delt / self.tflt_pitch);
        self.theta_pitch0 += (y2 - self.theta_pitch0) * temp;
        // ramp rate limiter
        temp = self.rrl_theta_pitch * self.delt;
        self.theta_pitch =
            (self.theta_pitch + temp).min((self.theta_pitch - temp).max(self.theta_pitch0));
    }

    /// Pascal `APCLogic`.
    fn apc_logic(&mut self) {
        let y1 = 1.0_f64.min(0.000001_f64.max(self.pmech_avl));
        // low pass filter on available power
        let mut temp = 1.0_f64.min(self.delt / self.tflt_pavl_apc);
        self.pavl_apc += (y1 - self.pavl_apc) * temp;
        // power curtailment
        temp = 0.4_f64.max(1.0_f64.min(self.pcurtail / self.pavl_apc));
        self.pwr_table_apc[1] = temp;
        self.pwr_table_apc[2] = temp;
        // power frequency curve
        let grid_frq = 1.0 + self.d_omg / self.rated_omg;
        let y2 = linear_interp(&self.frq_table_apc, &self.pwr_table_apc, grid_frq);
        let y3 = self.pavl_apc * y2;
        // low pass filter on set power
        temp = 1.0_f64.min(self.delt / self.tflt_pset_apc);
        self.pset_apc += (y3 - self.pset_apc) * temp;
        // APCFLG
        let mut y4 = if self.apc_flg == 0 {
            self.pcurtail
        } else {
            self.pset_apc
        };
        // enforce user-defined PmechMax in normal condition
        let pmech_max_: f64 =
            if grid_frq >= self.frq_table_apc[1] && grid_frq <= self.frq_table_apc[2] {
                1.0
            } else {
                1.2
            };
        let pmech_min: f64 = 0.2;
        y4 = pmech_max_.min(pmech_min.max(y4));
        // Pade delay function
        temp = 1.0_f64.min(self.delt / self.tdelay_apc * 2.0);
        self.pade_apc += (y4 - self.pade_apc) * temp;
        self.pstl = pmech_max_.min(pmech_min.max(2.0 * self.pade_apc - y4));
    }

    /// Pascal `WindInertia`.
    fn wind_inertia(&mut self) {
        let y1 = -self.d_omg / self.rated_omg + self.d_frq_pu_test;
        // deadband
        let y2 = 0.0_f64.max(y1 - self.db_wind_inertia);
        // low pass filter
        let mut temp = 1.0_f64.min(self.delt / self.tflt_d_frq_wind_inertia);
        self.d_frq_wind_inertia += (y2 - self.d_frq_wind_inertia) * temp;
        // multiplier
        let y3 = self.d_frq_wind_inertia * self.k_wind_inertia;
        // high pass filter
        temp = 1.0_f64.min(self.delt / self.tflt_d_pinp_wind_inertia);
        self.y3_lpf += (y3 - self.y3_lpf) * temp;
        let y4 = self.d_pinp_max.min(self.d_pinp_min.max(y3 - self.y3_lpf));
        // ramp rate limiter
        let temp1 = self.rru_d_pinp * self.delt;
        let temp2 = self.rrd_d_pinp * self.delt;
        self.d_pinp_wind_inertia =
            (self.d_pinp_wind_inertia + temp1).min((self.d_pinp_wind_inertia - temp2).max(y4));
    }

    /// Pascal `SwingModel`.
    fn swing_model(&mut self) {
        let tmech = self.pmech / self.wt;
        let tele = self.pele / self.wt;
        let tdamp = self.dshaft * self.d_wt;
        self.intg_d[11] = (tmech - tele - tdamp) / 2.0 / self.hwtg;
        self.d_wt = self.intg_x[11];
        self.wt = 0.01_f64.max(1.0 + self.d_wt);
    }

    /// Pascal `CalcCurrent` — Thevenin→Norton current injection into `i[1..3]`.
    fn calc_current(&mut self, i: &mut [Complex64]) {
        let e012 = self.e012;
        let vang = self.vang;
        let mut eabc = self.eabc;
        Self::seq2abc(&mut eabc, &e012, vang);
        self.eabc = eabc;
        let scale = -self.rated_amp * self.n_wtg as f64;
        for (ii, slot) in i.iter_mut().enumerate().take(3) {
            *slot = (self.eabc[ii] / self.zthev) * scale;
        }
    }

    /// Pascal `CalcDynamic` — the top-level dynamics step: instrument, PLL, fault
    /// detection, then (once per outer step, on the corrector) the 50 µs
    /// sub-cycle over the full controller and a fresh current injection.
    pub fn calc_dynamic(
        &mut self,
        v: &[Complex64],
        i: &mut [Complex64],
        h: f64,
        t: f64,
        iter_flag: IterationFlag,
    ) {
        self.delt_sim = h;

        // instrumentation
        self.instrumentation(v, i);
        // PLL
        self.pll_logic(h);
        // fault detection
        self.fault_detection();

        // start small-time-step iteration when time proceeds (corrector only)
        if t > self.tsim && iter_flag == IterationFlag::SameTimeStep {
            self.n_rec = ((h / self.delt0 / 2.0).trunc() as i64 * 2 + 1) as i32;
            self.delt = h / self.n_rec as f64;
            self.tsim = t;
            if self.wtg_trip == 0 {
                for _ in 1..=self.n_rec {
                    // PQ priority
                    self.pq_priority(0);
                    // real power regulation
                    self.real_power_reg();
                    // reactive power and voltage regulation
                    if self.q_flg == 0 {
                        self.iq_cmd_pos = self.qcmd / 0.000001_f64.max(self.vmag);
                        self.iq_cmd_pos = self.iqmn.max(self.iqmx.min(self.iq_cmd_pos));
                    } else {
                        self.reactive_power_reg();
                        self.voltage_reg();
                    }
                    // current regulator
                    self.lvpl();
                    self.lvql();
                    self.current_reg();

                    if self.sim_mech_flg > 0 {
                        self.aero_dynamic();
                        self.torque_reg();
                        self.pitch_control();
                        self.apc_logic();
                        self.wind_inertia();
                        self.swing_model();
                    }

                    // integration
                    self.integrate();
                    // current limiting logic
                    self.current_limiting();
                }
            } else {
                self.e012[0] = self.v012[0];
                self.e012[1] = self.v012[1];
                self.e012[2] = self.v012[2];
            }

            self.calc_current(i);
        }
    }
}

impl Default for Wtg3Model {
    fn default() -> Self {
        Self::new()
    }
}
