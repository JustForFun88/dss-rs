//! Spec-pinned unit tests for the shared inverter base (`InvBasedPceData`).
//!
//! Pascal is the spec: the oracle exposes none of these helpers outside a full
//! PVSystem/Storage solve (that numeric pinning arrives with PVSystem, WP7.3
//! step 2), so these pin the ported `InvBasedPCE.pas` methods to the Pascal
//! bodies directly.

use num_complex::Complex64;

use super::*;

/// Pascal `TInvBasedPCE.Create`: only `GFM_Mode`, the three `dynVars` scalars,
/// the shape/inverter-curve objects are explicitly set; everything else is the
/// zero/false/empty default the subclass overwrites.
#[test]
fn base_create_defaults() {
    let d = InvBasedPceData::new();
    assert!(!d.gfm_mode);
    assert_eq!(d.dyn_vars.i_limit, -1.0);
    assert_eq!(d.dyn_vars.i_comp, 0.0);
    assert_eq!(d.dyn_vars.v_error, 0.8);
    assert!(d.yearly_shape_obj.is_none());
    assert!(d.daily_shape_obj.is_none());
    assert!(d.duty_shape_obj.is_none());
    assert!(d.inverter_curve_obj.is_none());
    assert_eq!(d.connection, Connection::Wye);
    // `FirstSampleAfterReset` starts TRUE (registers reset on first sample).
    assert!(d.first_sample_after_reset);
    // No InvControl/ExpControl mode active out of the box.
    assert!(!d.using_cim_dynamics());
}

/// Pascal `Get_Presentkvar = Qnominalperphase * 0.001 * Fnphases`.
#[test]
fn get_present_kvar_scales_by_phases() {
    let mut d = InvBasedPceData::new();
    d.q_nominal_per_phase = 1000.0;
    assert_eq!(d.get_present_kvar(3), 3.0);
    assert_eq!(d.get_present_kvar(1), 1.0);
    d.q_nominal_per_phase = -250.0;
    assert_eq!(d.get_present_kvar(2), -0.5);
}

/// Pascal `UsingCIMDynamics = VWMode or VVMode or WVMode or AVRMode or DRCMode`.
/// `WPMode` is deliberately excluded ("WPMode not in CIM Dynamics").
#[test]
fn using_cim_dynamics_excludes_wp_mode() {
    let mut d = InvBasedPceData::new();
    assert!(!d.using_cim_dynamics());

    // Each included mode flips it true on its own.
    for set in [
        |d: &mut InvBasedPceData| d.vw_mode = true,
        |d: &mut InvBasedPceData| d.vv_mode = true,
        |d: &mut InvBasedPceData| d.wv_mode = true,
        |d: &mut InvBasedPceData| d.avr_mode = true,
        |d: &mut InvBasedPceData| d.drc_mode = true,
    ] {
        let mut d2 = d.clone();
        set(&mut d2);
        assert!(d2.using_cim_dynamics());
    }

    // WPMode alone does NOT count.
    d.wp_mode = true;
    assert!(!d.using_cim_dynamics());
}

/// Pascal `StickCurrInTerminalArray`, wye: `arr[i] += curr; arr[Fnconds] -=
/// curr` (the neutral). Here 0-based, so the neutral is `arr[nconds - 1]`.
#[test]
fn stick_curr_wye_routes_to_neutral() {
    let d = InvBasedPceData::new(); // Connection::Wye
    let nconds = 4; // 3-phase wye + neutral
    let c = Complex64::new(2.0, -1.0);
    let mut arr = vec![Complex64::ZERO; nconds];
    d.stick_curr_in_terminal_array(&mut arr, nconds, c, 0);
    assert_eq!(arr[0], c);
    assert_eq!(arr[3], -c); // neutral
    assert_eq!(arr[1], Complex64::ZERO);
    assert_eq!(arr[2], Complex64::ZERO);
}

/// Pascal uses `+=`/`-=`: the routine *accumulates* into the terminal array. In
/// the real solve it is called once per phase into the same array, so the wye
/// neutral must sum every phase's current. Pin accumulation (not overwrite) by
/// seeding a non-zero array and stacking two phase currents — a `+=`→`=`
/// regression on the neutral would corrupt the summed neutral current and this
/// is the test that catches it.
#[test]
fn stick_curr_wye_accumulates_neutral() {
    let d = InvBasedPceData::new(); // Connection::Wye
    let nconds = 4; // 3-phase wye + neutral
    let c0 = Complex64::new(2.0, -1.0);
    let c1 = Complex64::new(-0.5, 3.0);
    // Pre-seed non-zero so overwrite vs accumulate differ.
    let seed = Complex64::new(10.0, 10.0);
    let mut arr = vec![seed; nconds];

    d.stick_curr_in_terminal_array(&mut arr, nconds, c0, 0);
    d.stick_curr_in_terminal_array(&mut arr, nconds, c1, 1);

    assert_eq!(arr[0], seed + c0); // phase A accumulated onto the seed
    assert_eq!(arr[1], seed + c1); // phase B accumulated onto the seed
    assert_eq!(arr[2], seed); // untouched phase
    assert_eq!(arr[3], seed - c0 - c1); // neutral sums BOTH phases (the key check)
}

/// Pascal `StickCurrInTerminalArray`, delta: `arr[i] += curr; j := i + 1;
/// if j > Fnconds then j := 1; arr[j] -= curr` — the return conductor wraps.
#[test]
fn stick_curr_delta_wraps_return_conductor() {
    let mut d = InvBasedPceData::new();
    d.connection = Connection::Delta;
    let nconds = 3; // 3-phase delta
    let c = Complex64::new(1.0, 0.0);
    let mut arr = vec![Complex64::ZERO; nconds];

    // i = 0 -> +curr at 0, -curr at 1.
    d.stick_curr_in_terminal_array(&mut arr, nconds, c, 0);
    assert_eq!(arr[0], c);
    assert_eq!(arr[1], -c);

    // i = 2 (last phase) wraps the return conductor to 0.
    let mut arr2 = vec![Complex64::ZERO; nconds];
    d.stick_curr_in_terminal_array(&mut arr2, nconds, c, 2);
    assert_eq!(arr2[2], c);
    assert_eq!(arr2[0], -c);
    assert_eq!(arr2[1], Complex64::ZERO);
}

/// A minimal `InvBasedPce` implementor stands in for the concrete inverter PC
/// elements: the base virtual hooks default to `false` (Pascal base bodies), and
/// a subclass can override them.
struct MockPv {
    base: InvBasedPceData,
}
impl InvBasedPce for MockPv {
    fn inv_based(&self) -> &InvBasedPceData {
        &self.base
    }
    fn inv_based_mut(&mut self) -> &mut InvBasedPceData {
        &mut self.base
    }
    // Mirror a PVSystem-like override of `IsPVSystem`.
    fn is_pvsystem(&self) -> bool {
        true
    }
}

#[test]
fn trait_defaults_and_override() {
    let mut m = MockPv {
        base: InvBasedPceData::new(),
    };
    // Overridden hook.
    assert!(m.is_pvsystem());
    // Defaulted hooks (Pascal base bodies return False).
    assert!(!m.is_storage());
    assert!(!m.get_pf_priority());
    // Accessors reach the embedded base.
    m.inv_based_mut().kw_out = 5.0;
    assert_eq!(m.inv_based().kw_out, 5.0);
}

// --- GFM Norton `CalcGFMYprim` (B5: `Isc1` factor-1000 removal, DIVERGENCES.md
// §B5; dss_capi de6a5a42 = SVN r3865) ---------------------------------------

/// The `gfm_micro` storage's GFM Norton admittance (kv=0.48 L-L, kVA=800, delta,
/// 3-phase). Pinned to the **capi015** live `ActiveCktElement.Yprim` probe
/// (2026-07-12): with the r3865 `Isc1` change (drop the `*1000`), the assembled
/// short-circuit admittance is `Y[0,0]=561.4657341-2245.822260j`,
/// `Y[0,1]=-280.6718528+1122.728087j`. The pre-B5 0.14.5 value
/// (`Y[0,0]=613.6771792-...`) is far outside the band, so the test is
/// feature-sensitive to the adopted formula.
#[test]
fn gfm_calc_yprim_matches_capi015_isc1_no_1000() {
    let mut dv = InvDynamicVars::new();
    dv.rated_kv_ll = 0.48;
    dv.m_kva_rating = 800.0;
    let y = dv.calc_gfm_yprim(3, 3);
    let y00 = y.get(0, 0);
    let y01 = y.get(0, 1);
    // capi015 (0.15.0b4) live probe values.
    let want00 = Complex64::new(5.614657341e2, -2.245822260e3);
    let want01 = Complex64::new(-2.806718528e2, 1.122728087e3);
    assert!(
        (y00 - want00).norm() / want00.norm() < 1e-8,
        "GFM Y[0,0] {y00:?} != capi015 {want00:?}"
    );
    assert!(
        (y01 - want01).norm() / want01.norm() < 1e-8,
        "GFM Y[0,1] {y01:?} != capi015 {want01:?}"
    );
    // Feature-sensitivity: the pre-B5 (0.14.5, Isc1*1000) diagonal is 613.68 —
    // a ~9% move — so a reverted formula fails the capi015 pin above.
    let pre_b5_00 = Complex64::new(6.136771792e2, -2.402456595e3);
    assert!(
        (y00 - pre_b5_00).norm() / pre_b5_00.norm() > 1e-2,
        "GFM Y[0,0] must NOT match the pre-B5 0.14.5 value"
    );
}

/// The op-point invariance mechanism (DIVERGENCES.md §B5): `Isc1` feeds ONLY the
/// R0-quadratic (the ZERO-sequence impedance `Z0 = Zs + 2·Zm`); the
/// POSITIVE-sequence impedance `Zs − Zm` collapses algebraically to
/// `R1 + jX1 = Z1` (a function of `X1` alone, `Isc1`-free). A balanced /
/// delta-fed GFM load excites only the positive sequence, so its terminal
/// voltage — hence delivered power — is invariant to the `Isc1` virtual-impedance
/// scale, which is why 0.14.5 and capi015 give the bit-identical GFM op-point
/// despite the ~1000× `Z0` move. This pins that decomposition directly: the
/// balanced eigenvalue of the Norton equals `1/Z1` exactly, while the
/// zero-sequence eigenvalue does not.
#[test]
fn gfm_norton_positive_seq_admittance_is_isc1_invariant() {
    use std::f64::consts::PI;
    let mut dv = InvDynamicVars::new();
    dv.rated_kv_ll = 0.48;
    dv.m_kva_rating = 800.0;
    let y = dv.calc_gfm_yprim(3, 3);

    // Z1 = R1 + jX1, X1 = (RatedkVLL²/mKVArating)/√1.0625, R1 = X1/4 — depends on
    // NO `Isc1` term. (Pascal `CalcGFMYprim`: X1 l.255, Zs/Zm l.271-276.)
    let x1 = (dv.rated_kv_ll.powi(2) / dv.m_kva_rating) / 1.0625_f64.sqrt();
    let z1 = Complex64::new(x1 / 4.0, x1);
    let inv_z1 = z1.inv();

    // Positive-sequence eigenvector at the `CalcGFMVoltage` angles
    // (360 − k·120°): Y·v_pos must equal v_pos/Z1 (eigenvalue 1/Z1).
    let vpos: Vec<Complex64> = (0..3)
        .map(|k| {
            let ang = (360.0 - (k as f64) * 120.0) * PI / 180.0;
            Complex64::from_polar(1.0, ang)
        })
        .collect();
    for (row, &vrow) in vpos.iter().enumerate() {
        let acc: Complex64 = vpos
            .iter()
            .enumerate()
            .map(|(col, &v)| y.get(row, col) * v)
            .sum();
        let eig = acc / vrow;
        assert!(
            (eig - inv_z1).norm() / inv_z1.norm() < 1e-9,
            "positive-seq eigenvalue {eig:?} != 1/Z1 {inv_z1:?} (row {row})"
        );
    }

    // The zero-sequence eigenvalue (Y·[1,1,1]) is Isc1-dependent — it must NOT
    // equal 1/Z1 (else Isc1 would not enter the model at all).
    let acc0: Complex64 = (0..3).map(|col| y.get(0, col)).sum();
    assert!(
        (acc0 - inv_z1).norm() / inv_z1.norm() > 1e-2,
        "zero-seq eigenvalue must differ from 1/Z1 (Isc1 lives in Z0)"
    );
}

/// `varMode` ordinals, pinned against `PVsystem.pas:32-33` (`VARMODEPF = 0`,
/// `VARMODEKVAR = 1`; the same pair drives `Storage.pas`). Not a `DssEnum`
/// family — the value is set by the `PF=`/`kvar=` side effects and by the
/// InvControl/ExpControl dispatch, and read back only at the CIM boundary.
#[test]
fn var_mode_pins_pascal_ordinals() {
    use crate::elements::pc::inv_based_pce::VarMode;

    assert_eq!(VarMode::Pf.ordinal(), 0);
    assert_eq!(VarMode::Kvar.ordinal(), 1);
    for m in [VarMode::Pf, VarMode::Kvar] {
        assert_eq!(VarMode::from_ordinal(m.ordinal()), Some(m));
    }
    assert_eq!(VarMode::from_ordinal(-1), None);
    assert_eq!(VarMode::from_ordinal(2), None);
    // `TPVSystemObj.Create` / `TStorageObj.Create` both set VARMODEPF.
    assert_eq!(VarMode::default(), VarMode::Pf);
}
