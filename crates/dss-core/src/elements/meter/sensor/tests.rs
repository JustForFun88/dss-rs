use super::*;
use num_complex::Complex64;

/// Pascal `RotatePhases`: 3-phase delta wraps in the `FDeltaDirection`
/// direction; the 2-phase open-delta wrap sends `<1` to phase 3.
#[test]
fn rotate_phases_wraps() {
    let mut s = Sensor::new("r");
    s.med.cd.nphases = 3;
    s.f_delta_direction = 1;
    assert_eq!(s.rotate_phases(1), 2);
    assert_eq!(s.rotate_phases(2), 3);
    assert_eq!(s.rotate_phases(3), 1); // 4 > 3 → 1
    s.f_delta_direction = -1;
    assert_eq!(s.rotate_phases(1), 3); // 0 < 1 → nphases
    assert_eq!(s.rotate_phases(2), 1);
    assert_eq!(s.rotate_phases(3), 2);
    // 2-phase delta: a sub-1 result rolls to the (notional) 3rd phase.
    s.med.cd.nphases = 2;
    s.f_delta_direction = -1;
    assert_eq!(s.rotate_phases(1), 3);
}

/// Pascal `Get_WLSVoltageError`: weighted sum of `|CalcV|² − SensorV²`,
/// zero unless `Vspecified`.
#[test]
fn wls_voltage_error_matches_formula() {
    let mut s = Sensor::new("v");
    s.med.cd.nphases = 2;
    s.weight = 2.0;
    s.med.calculated_voltage = vec![Complex64::new(3.0, 4.0), Complex64::new(1.0, 1.0)];
    s.med.sensor_voltage = vec![5.0, 1.0];
    // Not yet specified → 0.
    assert_eq!(s.wls_voltage_error(), 0.0);
    s.v_specified = true;
    // ((25 − 25) + (2 − 1)) · 2
    assert!((s.wls_voltage_error() - 2.0).abs() < 1e-12);
}

/// Pascal `Get_WLSCurrentError`: re-derives `SensorCurrent` from P/Q on
/// `Vbase` (setting `Ispecified`), then weights `|CalcI|² − SensorI²`.
#[test]
fn wls_current_error_from_pq() {
    let mut s = Sensor::new("i");
    s.med.cd.nphases = 2;
    s.weight = 1.0;
    s.vbase = 1000.0;
    s.p_specified = true;
    s.q_specified = true;
    s.sensor_kw = vec![0.6, 0.0];
    s.sensor_kvar = vec![0.8, 0.0];
    s.med.sensor_current = vec![0.0; 2];
    s.med.calculated_current = vec![Complex64::new(2.0, 0.0), Complex64::ZERO];
    // |0.6 + 0.8j|·1000/1000 = 1.0 → SensorCurrent[0] = 1.0; Ispecified set.
    let err = s.wls_current_error();
    assert!(s.i_specified);
    assert!((s.med.sensor_current[0] - 1.0).abs() < 1e-12);
    // (4 − 1) + (0 − 0) = 3, ·weight 1.
    assert!((err - 3.0).abs() < 1e-12);
}
