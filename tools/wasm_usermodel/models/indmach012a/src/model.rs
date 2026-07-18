//! The induction-machine model — loop-for-loop port of r3723
//! `Version8/Source/IndMach012a/IndMach012Model.pas` (`TIndMach012Model`).
//! Every method cites its Pascal original; arithmetic keeps the exact Pascal
//! operator order (left-associative `*`/`/`) for bit parity with the native
//! FPC twin.

use crate::cmath::{
    cabs, cadd, cdiv, cdivreal, cmplx, cmul, cmulreal, conjg, csub, sqr, Complex, CZERO,
};
use crate::parser::{get_command, Parser};
use crate::records::{DynamicsRec, GeneratorVars};
use std::sync::atomic::{AtomicBool, Ordering};

/// Pascal `IndMach012Model.pas:9` — `NumVariables = 14`.
pub const NUM_VARIABLES: i32 = 14;

/// Pascal unit-global `Var DebugTrace: Boolean` (`IndMach012Model.pas:110`,
/// initialized FALSE at `:589-591`) — shared across model instances.
static DEBUG_TRACE: AtomicBool = AtomicBool::new(false);

/// Pascal `TIndMach012Model.GetVarName` name table (`MainUnit.pas:294-317`).
pub const VAR_NAMES: [&str; 14] = [
    "Slip",
    "puRs",
    "puXs",
    "puRr",
    "puXr",
    "puXm",
    "MaxSlip",
    "Is1",
    "Is2",
    "Ir1",
    "Ir2",
    "StatorLoss",
    "RotorLoss",
    "HPshaft",
];

/// Pascal `TIndMach012Model` (`IndMach012Model.pas:22-93`). The Pascal class
/// keeps raw pointers `GenData`/`DynaData`/`CallBack` into host memory; over
/// WASM those become the guest buffer addresses stored by the export layer
/// (`gen_data_ptr`/`dyna_data_ptr`) and refreshed record images passed into
/// each method.
pub struct IndMach012Model {
    pub pu_rs: f64,
    pub pu_xs: f64,
    pub pu_rr: f64,
    pub pu_xr: f64,
    pub pu_xm: f64,
    s1: f64, // pos-seq slip
    s2: f64,
    max_slip: f64,
    d_sdp: f64, // dSdP, for power flow
    // dynamics variables
    xopen: f64,
    xp: f64,
    t0p: f64, // rotor time constant
    in_dynamics: bool,
    zs: Complex,
    zm: Complex,
    zr: Complex,
    zrsc: Complex,
    is1: Complex,
    ir1: Complex,
    v1: Complex,
    is2: Complex,
    ir2: Complex,
    v2: Complex,
    // complex variables for dynamics
    e1: Complex,
    e1n: Complex,
    de1dt: Complex,
    de1dtn: Complex,
    e2: Complex,
    e2n: Complex,
    de2dt: Complex,
    de2dtn: Complex,
    zsp: Complex,
    first_iteration: bool,
    fixed_slip: bool,
    /// Guest address of the per-instance `TGeneratorVars` shuttle buffer
    /// (Pascal `GenData: pTGeneratorVars`); 0 in host-side tests.
    pub gen_data_ptr: i32,
    /// Guest address of the `TDynamicsRec` shuttle buffer (Pascal `DynaData`).
    pub dyna_data_ptr: i32,
}

impl IndMach012Model {
    /// Pascal `TIndMach012Model.Create` (`IndMach012Model.pas:113-139`).
    /// Class fields start zeroed (Pascal object zero-initialization).
    pub fn create(genvars: &mut GeneratorVars, gen_data_ptr: i32, dyna_data_ptr: i32) -> Self {
        let mut m = IndMach012Model {
            pu_rs: 0.0,
            pu_xs: 0.0,
            pu_rr: 0.0,
            pu_xr: 0.0,
            pu_xm: 0.0,
            s1: 0.0,
            s2: 0.0,
            max_slip: 0.0,
            d_sdp: 0.0,
            xopen: 0.0,
            xp: 0.0,
            t0p: 0.0,
            in_dynamics: false,
            zs: CZERO,
            zm: CZERO,
            zr: CZERO,
            zrsc: CZERO,
            is1: CZERO,
            ir1: CZERO,
            v1: CZERO,
            is2: CZERO,
            ir2: CZERO,
            v2: CZERO,
            e1: CZERO,
            e1n: CZERO,
            de1dt: CZERO,
            de1dtn: CZERO,
            e2: CZERO,
            e2n: CZERO,
            de2dt: CZERO,
            de2dtn: CZERO,
            zsp: CZERO,
            first_iteration: false,
            fixed_slip: false,
            gen_data_ptr,
            dyna_data_ptr,
        };
        // {Vestas Wind generator}
        m.pu_rs = 0.0053;
        m.pu_xs = 0.106;
        m.pu_rr = 0.007;
        m.pu_xr = 0.12;
        m.pu_xm = 4.0;

        m.max_slip = 0.1; // 10% slip limit - set this before setting slip
        m.set_slip(-0.007, genvars); // Generating about 1 pu power
        m.fixed_slip = false; // Allow Slip to float to match specified power

        m.in_dynamics = false;

        m.recalc_element_data(genvars);
        m
    }

    /// Pascal `set_Localslip` (`IndMach012Model.pas:274-283`).
    fn set_localslip(&mut self, value: f64) {
        // local `Sign`: x<0 -> -1 else 1
        fn sign(x: f64) -> f64 {
            if x < 0.0 {
                -1.0
            } else {
                1.0
            }
        }
        self.s1 = value;
        if !self.in_dynamics && self.s1.abs() > self.max_slip {
            self.s1 = sign(self.s1) * self.max_slip; // limit slip unless dynamics
        }
        self.s2 = 2.0 - self.s1;
    }

    /// Pascal `Set_Slip` (`IndMach012Model.pas:286-291`) — the `Slip`
    /// property setter: also makes the generator speed agree.
    fn set_slip(&mut self, value: f64, genvars: &mut GeneratorVars) {
        self.set_localslip(value);
        genvars.speed = genvars.w0 * (-self.s1);
    }

    /// Pascal `Get_ModelCurrent` (`IndMach012Model.pas:193-209`).
    fn get_model_current(&self, v: Complex, s: f64) -> (Complex, Complex) {
        let rl = if s != 0.0 {
            self.zr.re * (1.0 - s) / s
        } else {
            self.zr.re * 1.0e6
        };
        let zrotor = cadd(cmplx(rl, 0.0), self.zr);
        let numerator = cmul(self.zm, zrotor);
        let zmotor = cadd(self.zs, cdiv(numerator, cadd(zrotor, self.zm)));
        let istator = cdiv(v, zmotor);
        // Ir = Is - (V - Zs*Is)/Zm
        let irotor = csub(istator, cdiv(csub(v, cmul(self.zs, istator)), self.zm));
        (istator, irotor)
    }

    /// Pascal `Init` (`IndMach012Model.pas:212-234`) — dynamics-mode entry.
    /// `v012`/`i012` indices: 0 = zero seq, 1 = positive, 2 = negative.
    pub fn init(&mut self, v012: &[Complex; 3], i012: &[Complex; 3], genvars: &mut GeneratorVars) {
        // Initialize rotor speed: Speed := -LocalSlip * w0
        genvars.speed = -self.s1 * genvars.w0;

        // Voltage behind transient reactance; derivatives zeroed
        self.e1 = csub(v012[1], cmul(i012[1], self.zsp));
        self.de1dt = CZERO;
        self.e1n = self.e1;
        self.de1dtn = self.de1dt;
        self.e2 = csub(v012[2], cmul(i012[2], self.zsp));
        self.de2dt = CZERO;
        self.e2n = self.e2;
        self.de2dtn = self.de2dt;
    }

    /// Pascal `ReCalcElementData` (`IndMach012Model.pas:237-271`).
    pub fn recalc_element_data(&mut self, genvars: &GeneratorVars) {
        let zbase = sqr(genvars.kv_generator_base) / genvars.kva_rating * 1000.0;
        let rs = self.pu_rs * zbase;
        let xs = self.pu_xs * zbase;
        let rr = self.pu_rr * zbase;
        let xr = self.pu_xr * zbase;
        let xm = self.pu_xm * zbase;
        self.zs = cmplx(rs, xs);
        self.zm = cmplx(0.0, xm);
        self.zr = cmplx(rr, xr);

        self.xopen = xs + xm;
        self.xp = xs + (xr * xm) / (xr + xm);
        self.zsp = cmplx(rs, self.xp);
        self.t0p = (xr + xm) / (genvars.w0 * rr);

        self.zrsc = cadd(
            self.zr,
            cdiv(cmul(self.zs, self.zm), cadd(self.zs, self.zm)),
        );
        self.d_sdp = self.compute_dsdp(genvars);

        // NOTE (faithful): Is1/V1/Is2/V2 are zeroed but Ir1/Ir2 keep the
        // values Compute_dSdP just left in them — exactly the Pascal order.
        self.is1 = CZERO;
        self.v1 = CZERO;
        self.is2 = CZERO;
        self.v2 = CZERO;

        self.first_iteration = true;

        if DEBUG_TRACE.load(Ordering::Relaxed) {
            init_trace_file();
        }
    }

    /// Pascal `Compute_dSdP` (`IndMach012Model.pas:363-372`) — dSdP based on
    /// rated slip and rated voltage.
    ///
    /// TODO(compat): `1.732` reproduces upstream's truncated sqrt(3)
    /// (`kvGeneratorBase*1000.0/1.732`); full-precision fix deferred to the
    /// DE_PASCALIZE Stage F sweep.
    fn compute_dsdp(&mut self, genvars: &GeneratorVars) -> f64 {
        self.v1 = cmplx(genvars.kv_generator_base * 1000.0 / 1.732, 0.0);
        if self.s1 != 0.0 {
            let (istator, irotor) = self.get_model_current(self.v1, self.s1);
            self.is1 = istator;
            self.ir1 = irotor;
        }
        self.s1 / cmul(self.v1, conjg(self.is1)).re
    }

    /// Pascal `InterpretOption` (`IndMach012Model.pas:375-386`): dispatch on
    /// the first character of the uppercased option string. Upstream indexes
    /// `Uppercase(s)[1]` — an empty string is an access violation there; the
    /// port panics (deterministic trap, same failure class).
    fn interpret_option(&mut self, s: &str) {
        let first = s
            .as_bytes()
            .first()
            .copied()
            .expect("indmach012a: empty option string (upstream: access violation)");
        match first.to_ascii_uppercase() {
            b'F' => self.fixed_slip = true,
            b'V' => self.fixed_slip = false,
            b'D' => DEBUG_TRACE.store(true, Ordering::Relaxed), // DEBUG
            b'N' => DEBUG_TRACE.store(false, Ordering::Relaxed), // NODEBUG
            _ => {}
        }
    }

    /// Pascal `TIndMach012Model.Edit` (`IndMach012Model.pas:149-190`) — the
    /// caller (`MainUnit.Edit`) has already loaded `parser` with the command
    /// string. `msg` is the `MsgCallBack` sink (help text only, per the P3
    /// census).
    pub fn edit(
        &mut self,
        parser: &mut Parser,
        genvars: &mut GeneratorVars,
        msg: &mut dyn FnMut(&[u8]),
    ) {
        let mut param_pointer: i32 = 0;
        let mut param_name = parser.next_param();
        let mut param = parser.str_value();
        while !param.is_empty() {
            if param_name.is_empty() {
                if param.eq_ignore_ascii_case("help") {
                    param_pointer = 9;
                } else {
                    param_pointer += 1;
                }
            } else {
                param_pointer = get_command(&param_name);
            }

            match param_pointer {
                1 => self.pu_rs = parser.dbl_value(),
                2 => self.pu_xs = parser.dbl_value(),
                3 => self.pu_rr = parser.dbl_value(),
                4 => self.pu_xr = parser.dbl_value(),
                5 => self.pu_xm = parser.dbl_value(),
                6 => {
                    let v = parser.dbl_value();
                    self.set_slip(v, genvars);
                }
                7 => self.max_slip = parser.dbl_value(),
                8 => {
                    let s = parser.str_value();
                    self.interpret_option(&s);
                }
                9 => do_help_cmd(msg), // whatever the option, do help
                _ => {}
            }

            param_name = parser.next_param();
            param = parser.str_value();
        }

        self.recalc_element_data(genvars);
    }

    /// Pascal `CalcDynamic` (`IndMach012Model.pas:389-406`).
    pub fn calc_dynamic(
        &mut self,
        v012: &[Complex; 3],
        i012: &mut [Complex; 3],
        genvars: &GeneratorVars,
    ) {
        // in dynamics mode, slip is allowed to vary
        self.in_dynamics = true;
        self.v1 = v012[1]; // save for variable calcs
        self.v2 = v012[2];
        // gets slip from shaft speed
        self.set_localslip((-genvars.speed) / genvars.w0);
        self.get_dynamic_model_current(self.v1, self.v2);
        i012[1] = self.is1; // save for variable calcs
        i012[2] = self.is2;
        i012[0] = cmplx(0.0, 0.0);

        if DEBUG_TRACE.load(Ordering::Relaxed) {
            write_trace_record();
        }
    }

    /// Pascal `Integrate` (`IndMach012Model.pas:409-433`) — trapezoidal
    /// integration of E' behind transient reactance.
    pub fn integrate(&mut self, genvars: &GeneratorVars, dyna: &DynamicsRec) {
        if dyna.iteration_flag == 0 {
            // on predictor step: update old values
            self.e1n = self.e1;
            self.de1dtn = self.de1dt;
            self.e2n = self.e2;
            self.de2dtn = self.de2dt;
        }

        // dEdt = -jw0SE' - (E' - j(X-X')I')/T0'
        self.de1dt = csub(
            cmul(cmplx(0.0, -genvars.w0 * self.s1), self.e1),
            cdivreal(
                csub(self.e1, cmul(cmplx(0.0, self.xopen - self.xp), self.is1)),
                self.t0p,
            ),
        );
        self.de2dt = csub(
            cmul(cmplx(0.0, -genvars.w0 * self.s2), self.e2),
            cdivreal(
                csub(self.e2, cmul(cmplx(0.0, self.xopen - self.xp), self.is2)),
                self.t0p,
            ),
        );

        // trapezoidal integration
        let h2 = dyna.h * 0.5;
        self.e1 = cadd(self.e1n, cmulreal(cadd(self.de1dt, self.de1dtn), h2));
        self.e2 = cadd(self.e2n, cmulreal(cadd(self.de2dt, self.de2dtn), h2));
    }

    /// Pascal `Get_DynamicModelCurrent` (`IndMach012Model.pas:436-447`).
    fn get_dynamic_model_current(&mut self, v1: Complex, v2: Complex) {
        self.is1 = cdiv(csub(v1, self.e1), self.zsp); // I = (V-E')/Z'
        self.is2 = cdiv(csub(v2, self.e2), self.zsp);
        // rotor current Ir = Is - Vm/jXm
        self.ir1 = csub(self.is1, cdiv(csub(v1, cmul(self.is1, self.zsp)), self.zm));
        self.ir2 = csub(self.is2, cdiv(csub(v2, cmul(self.is2, self.zsp)), self.zm));
    }

    /// Pascal `CalcPFlow` (`IndMach012Model.pas:479-508`).
    pub fn calc_pflow(
        &mut self,
        v012: &[Complex; 3],
        i012: &mut [Complex; 3],
        genvars: &GeneratorVars,
    ) {
        self.v1 = v012[1]; // save for variable calcs
        self.v2 = v012[2];

        self.in_dynamics = false;

        if self.first_iteration {
            // initialize Is1
            let (istator, irotor) = self.get_model_current(self.v1, self.s1);
            self.is1 = istator;
            self.ir1 = irotor;
            self.first_iteration = false;
        }
        // if the fixed-slip option is set, use the user's value
        if !self.fixed_slip {
            let p_error = -genvars.pnominal_per_phase - cmul(self.v1, conjg(self.is1)).re;
            self.set_localslip(self.s1 + self.d_sdp * p_error); // new guess at slip
        }
        let (istator, irotor) = self.get_model_current(self.v1, self.s1);
        self.is1 = istator;
        self.ir1 = irotor;
        let (istator, irotor) = self.get_model_current(self.v2, self.s2);
        self.is2 = istator;
        self.ir2 = irotor;

        i012[1] = self.is1; // save for variable calcs
        i012[2] = self.is2;
        i012[0] = cmplx(0.0, 0.0);
    }

    /// Pascal `GetRotorLosses` (`IndMach012Model.pas:349-353`).
    fn get_rotor_losses(&self) -> f64 {
        3.0 * (sqr(self.ir1.re) + sqr(self.ir1.im) + sqr(self.ir2.re) + sqr(self.ir2.im))
            * self.zr.re
    }

    /// Pascal `GetStatorLosses` (`IndMach012Model.pas:356-360`).
    fn get_stator_losses(&self) -> f64 {
        3.0 * (sqr(self.is1.re) + sqr(self.is1.im) + sqr(self.is2.re) + sqr(self.is2.im))
            * self.zs.re
    }

    /// Pascal `Get_Variable` (`IndMach012Model.pas:511-538`); i is 1-based.
    pub fn get_variable(&self, i: i32) -> f64 {
        let mut result = -1.0;
        match i {
            1 => result = self.s1, // LocalSlip
            2 => result = self.pu_rs,
            3 => result = self.pu_xs,
            4 => result = self.pu_rr,
            5 => result = self.pu_xr,
            6 => result = self.pu_xm,
            7 => result = self.max_slip,
            8 => result = cabs(self.is1),
            9 => result = cabs(self.is2),
            10 => result = cabs(self.ir1),
            11 => result = cabs(self.ir2),
            12 => result = self.get_stator_losses(),
            13 => result = self.get_rotor_losses(),
            14 => {
                // shaft power (hp)
                // TODO(compat): FPC folds the all-constant `3.0/746.0` at
                // SINGLE precision (both operands are exactly representable
                // in Single, and FPC evaluates constant expressions at the
                // lowest common precision of their operands), so the twin's
                // conversion factor is the f32 quotient — proven bit-exact by
                // the var14 decomposition probe (scratch var14_probe2.py →
                // docs/wasm/probes/p6_twin_expected.txt pin; a plain f64
                // `3.0/746.0` misses the twin by 2.6e-8 rel). Clean fix
                // (full-precision factor) deferred to DE_PASCALIZE Stage F.
                const HP_PER_WATT: f64 = (3.0f32 / 746.0f32) as f64;
                result = HP_PER_WATT
                    * (sqr(cabs(self.ir1)) * (1.0 - self.s1) / self.s1
                        + sqr(cabs(self.ir2)) * (1.0 - self.s2) / self.s2)
                    * self.zr.re;
            }
            _ => {}
        }
        result
    }

    /// Pascal `Set_Variable` (`IndMach012Model.pas:541-556`); non-settable
    /// indices are silently read-only (upstream comment).
    pub fn set_variable(&mut self, i: i32, value: f64, genvars: &mut GeneratorVars) {
        match i {
            1 => self.set_slip(value, genvars),
            2 => self.pu_rs = value,
            3 => self.pu_xs = value,
            4 => self.pu_rr = value,
            5 => self.pu_xr = value,
            6 => self.pu_xm = value,
            _ => {}
        }
    }
}

/// Pascal `DoHelpCmd` (`IndMach012Model.pas:450-476`) — builds the help text
/// and hands it to `CallBack^.MsgCallBack` (the single callback slot the
/// example uses, P3 census). String reproduced byte-for-byte, including the
/// double space in "rotor  resistance" and the trailing period without CRLF.
fn do_help_cmd(msg: &mut dyn FnMut(&[u8])) {
    const HELP_STR: &str = "Rs= per unit stator resistance.\r\n\
                            Xs= per unit stator leakage reactance.\r\n\
                            Rr= per unit rotor  resistance.\r\n\
                            Xr= per unit rotor leakage reactance.\r\n\
                            Xm= per unit magnetizing reactance.\r\n\
                            slip= initial slip value.\r\n\
                            maxslip= max slip value to allow.\r\n\
                            option={fixedslip | variableslip | Debug | NoDebug }\r\n\
                            Help: this help message.";
    msg(HELP_STR.as_bytes());
}

/// Pascal `InitTraceFile` (`IndMach012Model.pas:559-570`) writes
/// `IndMach012_Trace.CSV`. Deliberately a no-op here: the WASM sandbox has no
/// filesystem (plan §2.9-7 / ABI §6); the trace is a local debugging aid with
/// no numeric or host-observable contract. NOT a `TODO(compat)` — this is the
/// documented sandbox policy, not an inexactness reproduction.
fn init_trace_file() {}

/// Pascal `WriteTraceRecord` (`IndMach012Model.pas:573-587`) — see
/// [`init_trace_file`].
fn write_trace_record() {}
