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

#[cfg(test)]
mod make_pos_seq_tests {
    use super::*;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};
    use crate::elements::traits::CktElement;

    /// Pascal `TSensorObj.MakePosSequence` (Sensor.pas:478): resync to the
    /// metered element, ClearSensor, ValidSensor := TRUE, then
    /// AllocateSensorObjArrays / ZeroSensorArrays / RecalcVbase; `inherited` runs
    /// the base rename. For a 1-phase wye sensor, RecalcVbase → kVBase·1000 (no √3).
    #[test]
    fn resyncs_and_recomputes_vbase() {
        let mut s = Sensor::new("s1");
        s.med.metered_element = Some(ElemRef { cls: 1, idx: 3 });
        s.med.metered_terminal = 1;
        // seed a measured value + a stale spec flag to prove ClearSensor/Zero.
        s.v_specified = true;
        s.med.sensor_voltage = vec![7.0, 7.0, 7.0];

        let ctx = PosSeqCtx {
            monitored: Some(PosSeqElemInfo {
                bus_names: vec!["b1".into(), "b2".into()],
                nphases: 1,
                nconds: 1,
                yorder: 2,
                ..Default::default()
            }),
            ..Default::default()
        };
        let plan = s.make_pos_sequence(&ctx);

        assert_eq!(s.med.cd.nphases, 1);
        assert_eq!(s.med.cd.nconds, 1);
        assert_eq!(s.med.cd.get_bus(1), "b1");
        assert!(s.valid_sensor); // ValidSensor := TRUE
        assert!(!s.v_specified); // ClearSensor
        // AllocateSensorObjArrays + ZeroSensorArrays: per-phase arrays = 1, zero.
        assert_eq!(s.med.sensor_voltage, vec![0.0]);
        assert_eq!(s.med.calculated_current.len(), 2); // metered Yorder
        // RecalcVbase: wye 1-phase → kVBase·1000 (default 12.47 kV).
        assert!((s.vbase - 12_470.0).abs() < 1e-9);
        assert!(plan.run_base && plan.actions.is_empty());
        assert_eq!(s.monitored_element_ref(), Some(ElemRef { cls: 1, idx: 3 }));
    }

    /// Pascal NIL guard: no metered element ⇒ only the base rename runs.
    #[test]
    fn nil_metered_element_runs_base_only() {
        let mut s = Sensor::new("s1");
        let np = s.med.cd.nphases;
        let plan = s.make_pos_sequence(&PosSeqCtx::default());
        assert_eq!(s.med.cd.nphases, np);
        assert!(plan.run_base);
    }
}
