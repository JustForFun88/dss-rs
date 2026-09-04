//! Nominal-power machinery: the load-shape (wind-speed) multipliers, the
//! aerodynamic `SetNominalGeneration` (the `P(v³·Cp)` steady-state path) and
//! `RecalcElementData`.

use num_complex::Complex64;

use crate::elements::traits::SysCtx;
use crate::solution::{SolveMode, USEDAILY, USEDUTY, USEYEARLY};

use super::WindGen;

const PI: f64 = std::f64::consts::PI;

impl WindGen {
    /// Pascal `CalcDailyMult`: the shape value is the **wind speed** (m/s), not a
    /// per-unit multiplier; with no shape it defaults to the model's `VWind`.
    fn calc_daily_mult(&mut self, hr: f64) {
        if let Some(s) = self.daily_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr);
            self.shape_is_actual = s.use_actual();
        } else {
            self.shape_factor = Complex64::new(self.wind_model_dyn.vwind, 0.0);
        }
    }

    /// Pascal `CalcDutyMult` (falls back to daily; includes `DutyStart`).
    fn calc_duty_mult(&mut self, hr: f64) {
        if let Some(s) = self.duty_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr + self.duty_start);
            self.shape_is_actual = s.use_actual();
        } else {
            self.calc_daily_mult(hr);
        }
    }

    /// Pascal `CalcYearlyMult`.
    fn calc_yearly_mult(&mut self, hr: f64) {
        if let Some(s) = self.yearly_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr);
            self.shape_is_actual = s.use_actual();
        } else {
            self.shape_factor = Complex64::new(self.wind_model_dyn.vwind, 0.0);
        }
    }

    /// Pascal `SyncUpPowerQuantities`: keep kvar nominal in step with kW/PF.
    pub(super) fn sync_up_power_quantities(&mut self) {
        if self.pf_nominal == 0.0 {
            return;
        }
        self.kvar_base = self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
        self.q_nominal_per_phase = 1000.0 * self.kvar_base / self.cd.nphases as f64;
        if self.pf_nominal < 0.0 {
            self.kvar_base = -self.kvar_base;
        }
        if self.kva_not_set {
            self.kva_rating = self.kw_base * 1.2;
        }
    }

    /// Pascal `TProp.kvar` side effect: recompute `Qnominalperphase` and
    /// `PFNominal` from kW/kvar, and clear the `PF` prp-sequence.
    pub(super) fn side_effect_kvar(&mut self) {
        let nphases = self.cd.nphases as f64;
        self.q_nominal_per_phase = 1000.0 * self.kvar_base / nphases;
        let kva_gen = (self.kw_base.powi(2) + self.kvar_base.powi(2)).sqrt();
        self.pf_nominal = if kva_gen != 0.0 {
            self.kw_base / kva_gen
        } else {
            1.0
        };
        if self.kw_base * self.kvar_base < 0.0 {
            self.pf_nominal = -self.pf_nominal;
        }
        self.cd.obj.clear_seq(super::prop::PF);
    }

    /// The shared VBase update (Pascal `TProp.conn`/`TProp.kV` side effects):
    /// L-N for 2/3-phase, otherwise the supplied value.
    pub(super) fn update_vbase(&mut self) {
        self.v_base = match self.cd.nphases {
            2 | 3 => self.kv_windgen_base * crate::util::inv_sqrt3_x1000(),
            _ => self.kv_windgen_base * 1000.0,
        };
    }

    /// Pascal `SetNominalGeneration`. The load shape provides the wind speed;
    /// the steady-state power is `Pm = 0.5·ρ·π·Rad²·v³·Cp`, curtailed to `kWBase`
    /// and shared over the phases. `Yeq`/per-phase P/Q are only recomputed in
    /// power-flow mode (dynamics/harmonics leave the internal state alone).
    ///
    /// `node_v` supplies the terminal voltages the volt-var (`QMode=2`) branch
    /// reads; pass an empty slice from contexts without a live solution (Yprim
    /// build / parse-time recalc), matching the Pascal `NodeRef <> NIL` guard
    /// (→ `VmagTmp = 0`). The converged operating point is unaffected — the real
    /// `Qnominalperphase` is set at injection time when `node_v` is live.
    pub fn set_nominal_generation(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let gen_on_saved = self.gen_on;
        self.shape_factor = Complex64::new(self.wind_model_dyn.vwind, 0.0);
        self.gen_on = true;

        let factor = match sys.mode {
            SolveMode::Snapshot => sys.gen_multiplier,
            SolveMode::Daily => {
                let f = sys.gen_multiplier;
                self.calc_daily_mult(sys.dbl_hour);
                f
            }
            SolveMode::Yearly => {
                let f = sys.gen_multiplier;
                self.calc_yearly_mult(sys.dbl_hour);
                f
            }
            SolveMode::DutyCycle => {
                let f = sys.gen_multiplier;
                self.calc_duty_mult(sys.dbl_hour);
                f
            }
            SolveMode::Time | SolveMode::Dynamic => {
                let f = sys.gen_multiplier;
                match sys.active_load_shape_class {
                    USEDAILY => self.calc_daily_mult(sys.dbl_hour),
                    USEYEARLY => self.calc_yearly_mult(sys.dbl_hour),
                    USEDUTY => self.calc_duty_mult(sys.dbl_hour),
                    _ => {
                        // default to the wind speed set by default
                        self.shape_factor = Complex64::new(self.wind_model_dyn.vwind, 0.0);
                    }
                }
                f
            }
            SolveMode::Monte1 | SolveMode::MonteFault | SolveMode::FaultStudy => {
                sys.gen_multiplier * 1.0
            }
            SolveMode::Monte2 | SolveMode::Monte3 | SolveMode::LD1 | SolveMode::LD2 => {
                let f = sys.gen_multiplier;
                self.calc_daily_mult(sys.dbl_hour);
                f
            }
            SolveMode::PeakDay => {
                let f = sys.gen_multiplier;
                self.calc_daily_mult(sys.dbl_hour);
                f
            }
            SolveMode::AutoAdd => 1.0,
            _ => sys.gen_multiplier,
        };

        self.wind_model_dyn.vwind = self.shape_factor.re;
        if self.shape_factor.re > self.v_cut_out || self.shape_factor.re < self.v_cut_in {
            self.p_nominal_per_phase = 0.001 * self.kw_base;
            self.q_nominal_per_phase = 0.0;
            self.pm = 0.0;
            self.pg = 0.0;
            self.ps = 0.0;
            self.pr = 0.0;
            self.s = 0.0;
        } else if !(sys.is_dynamic_model || sys.is_harmonic_model) {
            // Get the losses from the provided curve (if any).
            let my_losses_pct = self
                .loss_curve_obj
                .as_mut()
                .map(|c| c.get_y_value(self.wind_model_dyn.vwind))
                .unwrap_or(0.0);
            let mut lead_lag = 1.0_f64;
            let nphases = self.cd.nphases as f64;

            self.pm =
                0.5 * self.pd * PI * self.rad.powi(2) * self.shape_factor.re.powi(3) * self.cp;
            let my_losses = self.pm * my_losses_pct / 100.0;
            self.pg = (self.pm - my_losses) / 1e3; // in kW
            if self.pg > self.kw_base {
                self.pg = self.kw_base; // generation limits
            }
            self.s = 1.0
                - ((self.poles * self.shape_factor.re * self.lamda)
                    / (self.w0 * self.ag * self.rad));
            self.ps = self.pg / (1.0 - self.s);
            self.pr = self.ps * self.s;

            self.p_nominal_per_phase = (1e3 * factor * self.pg) / nphases;

            // Now check for Q depending on QMode.
            let mut kvar_calc;
            match self.wind_model_dyn.q_mode {
                1 => {
                    // PF
                    kvar_calc =
                        ((self.pg / self.pf_nominal.abs()).powi(2) - self.pg.powi(2)).sqrt();
                    let kva_tmp = (self.pg.powi(2) + kvar_calc.powi(2)).sqrt();
                    if kva_tmp > self.kva_rating {
                        kvar_calc = self.kvar_base; // saturation
                    }
                    if self.pf_nominal < 0.0 {
                        lead_lag = -1.0;
                    }
                }
                2 => {
                    // Volt-var control.
                    let mut vmag_tmp = 0.0_f64;
                    if !node_v.is_empty() && !self.cd.node_ref.is_empty() {
                        // Highest local voltage across the phases.
                        let mut vmag = 0.0_f64;
                        for i in 0..self.cd.nphases {
                            let vm = node_v[self.cd.node_ref[i]].norm();
                            if vm > vmag {
                                vmag = vm;
                            }
                        }
                        let vmag = vmag / self.v_base; // in pu
                        if let Some(c) = self.vv_curve_obj.as_mut() {
                            vmag_tmp = c.get_y_value(vmag);
                        }
                    }
                    kvar_calc = self.kvar_base * vmag_tmp;
                    if kvar_calc.abs() > self.kvar_base {
                        kvar_calc = self.kvar_base;
                        if vmag_tmp < 0.0 {
                            lead_lag = -1.0;
                        }
                    }
                }
                // Constant Q (`QMode=0`, `Create`'s default — `WindGen.pas:1020`).
                //
                // `WindGen.pas:1276-1322` has **no `0:` arm**: mode 0 falls through
                // to `Else kvarCalc := 0` (`:1320-1321`), so a default-configured
                // WindGen injects zero vars however its `kvar=`/`pf=` reads. That is
                // an upstream slip, not a design, and CLAUDE.md's 2026-08-02 policy
                // fixes it in both lanes instead of reproducing it: the property
                // help documents `0:Q` (`:429-430`), the dynamics model spells the
                // same mode `QMode := 0; // 0 -> Constant Q` (`WTG3_Model.pas:252`)
                // and implements it as `Qord := Qref` (`:1059-1061`), models 4/5
                // inject `varBase = 1000*kvarBase/Fnphases` unconditionally
                // (`:1361`, `:1797`, comment `:1775` "Q is always kvarBase"), arm 1's
                // saturation fallback *is* `kvarBase` (`:1284`), and arm 2 is
                // `kvarBase` scaled by the VV curve and saturated at `|kvarBase|`
                // (`:1313-1316`). The parent class writes the same dispatch outright
                // — `Qnominalperphase := 1000*kvarBase*Factor*ShapeFactor.im/Fnphases`
                // (`Generator.pas:1163`, ported at
                // `pc/generator/nominal.rs:196-199`) — which WindGen could not
                // transcribe because its `ShapeFactor` carries the wind SPEED, not a
                // pu multiplier (`:1241`), and dropped.
                //
                // No `kVArating` clamp, because arm 1's saturation test can
                // never fire: `kVATmp = sqrt(Pg² + kvarCalc²)` is `Pg/|PF|` and
                // `Pg` is capped at `kWBase` a few lines above (`:1268-1269`),
                // so `kVATmp <= kWBase/|PF| == kVArating` — measured, and
                // bit-for-bit equal at the cap (`3000/0.95`, `1584/0.88`,
                // `1080/0.9` all land exactly on the rating, never above it).
                // That same identity is why arm 1's dispatch *coincides* with
                // `kvarBase` to 1 ulp whenever the wind caps `Pg`, and its
                // saturation fallback is `kvarBase` in any case (`:1284`).
                //
                // The SIGN, and why the arm splits on `kVANotSet`
                // (`R4133_PROPS_PLAN.md` §RP3.10 audit settlement, finding
                // RP310-CODE-1). The `pf` convention is "kW and kvar carry
                // OPPOSITE signs when `PFNominal < 0`". With `kVA=` unset
                // `kvarBase` already carries exactly that sign, for every
                // combination of `kW`/`pf` signs — `sync_up_power_quantities`
                // negates it when `pf<0` (`:3028`) and a typed `kvar<0` goes in
                // straight (`Set_Presentkvar`, `:3001`) — so the base is read
                // raw there and this arm moves nothing. When a deck types
                // `kVA=`, `RecalcElementData` re-derives
                // `kWBase := kVArating*|PFNominal|` (non-negative) and
                // `kvarBase := sqrt(kVArating² - kWBase²)` (`:1377-1378`,
                // ported below) — a non-negative root that STRIPS the sign.
                // Reading the base raw there would dispatch the OPPOSITE sign
                // from arm 1 on identical tokens, so it is restored the way arm
                // 1 carries its own: `LeadLag = -1` when `PFNominal < 0`
                // (`:1286-1287`). Probed live on r4133 for
                // `kW=1000 kVA=1200 pf=-0.9` (renormalised to `kWBase = 1080`,
                // `kvarBase = 523.0678732248808`): arm 1 reports terminal
                // `Q = +523.0678462465884` kvar — the machine ABSORBS, which is
                // what `pf<0` means — while the raw base would inject. Pinned by
                // `tests::qmode0_dispatch_carries_the_sign_and_scales_with_genmult`.
                // (Upstream's own arm 2 and models 4/5 read the stripped base
                // and so keep the injecting sign there — `model=4` on those
                // tokens reports terminal `Q = -523.0475290915948`; separate
                // upstream defects, recorded in the RP3.10 record, reproduced
                // by neither this arm nor any pin.)
                //
                // `Factor` (`GenMultiplier`) still applies: `:1325` sits OUTSIDE
                // the `case` (probed: `Set genmult=0.5` halves the dispatch,
                // exactly as it halves arm 1's).
                //
                // r4133 keeps dispatching 0, so the four corpus decks that declare a
                // WindGen without a `QMode=` token diverge across the solved model:
                // excluded per case in `tests/corpus/ledger.json` (cause
                // `windgen-qmode0-no-arm`) and pinned by
                // `tests::qmode0_dispatches_the_base_kvar`.
                0 => {
                    if self.kva_not_set {
                        kvar_calc = self.kvar_base;
                    } else {
                        kvar_calc = self.kvar_base.abs();
                        if self.pf_nominal < 0.0 {
                            lead_lag = -1.0;
                        }
                    }
                }
                _ => {
                    // Out-of-range modes keep upstream's `Else` (`:1320-1321`).
                    // Reachable: `q_mode` is a plain `i32` and
                    // `set_wgen_variable(15)` (`dynamics.rs:464`) accepts any
                    // integer, even though the property itself is a
                    // `mapped_int_enum` (`mod.rs:182`).
                    kvar_calc = 0.0;
                }
            }

            self.q_nominal_per_phase = 1e3 * kvar_calc * lead_lag * factor / nphases;
        }

        // `WindGen.pas:1332-1345` — build the Y primitive eq. Model 6 takes the
        // machine reactance instead of the P/Q equivalent, and — note — leaves
        // `Yeq95`/`Yeq105` at whatever they held: the Pascal `CASE`'s model-6 arm
        // sets only `Yeq` (`:1336`), and the model-6 current path
        // (`DoUserModel`) never reads the 95/105 pair.
        if !(sys.is_dynamic_model || sys.is_harmonic_model) {
            if self.gen_model == 6 {
                // Gets negated in CalcYPrim.
                self.yeq = Complex64::new(0.0, -self.xd).inv();
            } else {
                self.yeq = Complex64::new(self.p_nominal_per_phase, -self.q_nominal_per_phase)
                    / self.v_base.powi(2); // Vbase L-N for 3-phase
                self.yeq95 = if self.vminpu != 0.0 {
                    self.yeq / self.vminpu.powi(2)
                } else {
                    self.yeq // always a constant-Z model
                };
                self.yeq105 = if self.vmaxpu != 0.0 {
                    self.yeq / self.vmaxpu.powi(2)
                } else {
                    self.yeq
                };
            }
        }

        if self.gen_on != gen_on_saved {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal `TWindGenObj.RecalcElementData`.
    pub fn recalc(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases as f64;
        self.v_base95 = self.vminpu * self.v_base;
        self.v_base105 = self.vmaxpu * self.v_base;
        self.var_base = 1000.0 * self.kvar_base / nphases;

        // "Populate data structures used for interchange with user-written
        // models" (`WindGen.pas:1365-1373`). The reactances are re-derived from
        // the kVA rating *before* the kVA/kW reconciliation below may change it —
        // the Pascal order, and `Xd` feeds the model-6 Yprim (`:1336`). The
        // grouping is `:1368`'s (`puX * 1000 * SQR(kV) / kVA`), which differs
        // from `Create`'s; the record's `Conn`/`NumPhases`/`NumConductors` are
        // read straight off `cd` when the shuttle image is built.
        self.xd = self.pu_xd * 1000.0 * self.kv_windgen_base.powi(2) / self.kva_rating;
        self.xdp = self.pu_xdp * 1000.0 * self.kv_windgen_base.powi(2) / self.kva_rating;
        self.xdpp = self.pu_xdpp * 1000.0 * self.kv_windgen_base.powi(2) / self.kva_rating;

        if !self.kva_not_set {
            self.kw_base = self.kva_rating * self.pf_nominal.abs();
            self.kvar_base = (self.kva_rating.powi(2) - self.kw_base.powi(2)).sqrt();
        } else {
            self.kva_rating = self.kw_base / self.pf_nominal.abs();
            self.wind_model_dyn.rated_kva = self.kva_rating;
        }

        // No live solution here — the volt-var branch reads no voltages.
        self.set_nominal_generation(sys, &[]);

        self.yq_fixed = -self.var_base / self.v_base.powi(2);

        // `WindGen.pas:1406-1408`: `Vtarget := Vpu * 1000 * kVWindGenBase`.
        // `Vpu` is fixed at its Create value of 1.0 — r4133 registers no
        // property that writes it (module doc) — and `x * 1.0` is exact, so the
        // factor is elided rather than approximated.
        self.v_target = 1000.0 * self.kv_windgen_base;
        if self.cd.nphases > 1 {
            self.v_target /= crate::util::sqrt3();
        }

        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];

        // `:1418` — `If Usermodel.Exists Then UserModel.FUpdateModel` (the
        // `ShaftModel` twin at `:1419` is unreachable, module doc), *before* the
        // WTG3 recalc at `:1422`.
        self.update_user_models(sys);

        self.wind_model_dyn
            .recalc_element_data(sys.dyna_h, sys.dyna_t);
    }
}
