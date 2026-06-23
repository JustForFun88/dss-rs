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
