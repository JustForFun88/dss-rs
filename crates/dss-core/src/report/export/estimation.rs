//! `Export Estimation` (Pascal `Common/ExportResults.pas` `ExportEstimation`
//! (:1652), reached from `Executive/ExportOptions.pas:452` — export verb 5, and
//! from the `Estimate` command tail `ExecHelper.pas:4225`): the state-estimation
//! error report `EXP_ESTIMATION.csv`.
//!
//! Two sections, both driven by the `TMeterElement` sensor arrays the load
//! allocation loop (`AllocateLoads`/`Estimate` → `CalcAllocationFactors`) fills:
//!
//! * **Energy Meters** — per enabled EnergyMeter: the measured per-phase target
//!   current (`SensorCurrent`, the `peakcurrent=` property), the magnitude of the
//!   metered element's `CalculatedCurrent`, and the percent error between them.
//! * **Sensors** — the same three current triplets for every enabled Sensor,
//!   plus the target/calculated/percent-error **voltage** triplets and the two
//!   weighted-least-squares residuals (`WLSVoltageError`/`WLSCurrentError`).
//!
//! Byte layout is loop-for-loop from the Pascal: a 3-slot `TempX` staging buffer
//! that is zeroed before the target and calculated passes but **not** before the
//! percent-error pass (so a sub-3-phase device prints `0` in the unused slots,
//! and the error pass reuses the calculated magnitudes in place), every number
//! `Format('%.6g')`. r4133 `Version8/Source/Common/ExportResults.pas:1599` is
//! character-identical to the pinned 0.14.5 source, so both gating oracles agree.

use crate::circuit::Circuit;
use crate::elements::meter::energymeter::EnergyMeter;
use crate::elements::meter::sensor::Sensor;
use crate::exec::registry::DssClass;
use crate::report::format;

/// Pascal's `TempX: array[1..3] of Double` staging buffer.
type TempX = [f64; 3];

/// Pascal `ZeroTempXArray`.
fn zero_temp_x(t: &mut TempX) {
    *t = [0.0; 3];
}

/// Pascal `For i := 1 to 3 do FSWrite(F, Format(', %.6g', [TempX[i]]))`.
fn write_temp_x(s: &mut String, t: &TempX) {
    for v in t {
        s.push_str(", ");
        s.push_str(&format::g(*v, 6));
    }
}

/// The number of `TempX` slots a device fills: `Nphases`, capped at the 3-slot
/// buffer.
///
/// Pascal writes `TempX[i]` for `i := 1 to Nphases` into a `1..3` local array —
/// a device with more than three phases overruns the stack buffer (undefined
/// behaviour, not a defined upstream bug), so the port clamps instead of
/// reproducing it. Every reachable meter/sensor here has `Nphases <= 3` (the
/// report has no meaningful 4+-phase form: the columns are named `I1..I3`).
fn temp_x_slots(nphases: usize) -> usize {
    nphases.min(3)
}

/// Pascal's percent-error pass: `TempX[i] := (1.0 - TempX[i] / Max(0.001,
/// target[i])) * 100.0` for `i := 1..Nphases`, leaving the slots past `Nphases`
/// at whatever the calculated pass left there (zero — that pass zeroed them).
fn percent_error(t: &mut TempX, target: &[f64], n: usize) {
    for (i, slot) in t.iter_mut().enumerate().take(n) {
        let denom = target.get(i).copied().unwrap_or(0.0).max(0.001);
        *slot = (1.0 - *slot / denom) * 100.0;
    }
}

/// Fill `t` with `src[0..n]` after zeroing it (the Pascal `ZeroTempXArray` +
/// `For i := 1 to Nphases do TempX[i] := src^[i]` pair).
fn load_temp_x(t: &mut TempX, src: &[f64], n: usize) {
    zero_temp_x(t);
    for (i, slot) in t.iter_mut().enumerate().take(n) {
        *slot = src.get(i).copied().unwrap_or(0.0);
    }
}

/// Build the `Export Estimation` body (Pascal `ExportEstimation`).
///
/// Takes `&mut [DssClass]` because `TSensorObj.Get_WLSCurrentError` is a
/// *mutating* getter: when the sensor is P-specified it re-derives
/// `SensorCurrent` from `kWs`/`kvars` and latches `Ispecified` before summing
/// the residual (`Sensor.pas:599-631`). That side effect is reproduced in the
/// Pascal's order — after this sensor's own target/calculated columns are
/// written, so the row itself is unaffected and only later reads observe it.
pub(crate) fn export_estimation(classes: &mut [DssClass], ckt: &Circuit) -> String {
    let mut s = String::new();
    let mut t: TempX = [0.0; 3];

    // ---- Energy Meters -------------------------------------------------
    s.push_str("\"Energy Meters\" \n");
    s.push_str(
        "\"energyMeter\", \"I1 Target\", \"I2 Target\", \"I3 Target\", \
         \"I1 Calc\", \"I2 Calc\", \"I3 Calc\", \
         \"I1 %Err\", \"I2 %Err\", \"I3 %Err\"\n",
    );
    for &r in &ckt.energy_meters {
        let name = classes[r.class_ord()].arena[r.index()]
            .data()
            .name()
            .to_string();
        let Some(em) = classes[r.class_ord()].arena.get::<EnergyMeter>(r.index()) else {
            continue;
        };
        if !em.med.cd.enabled {
            continue;
        }
        let n = temp_x_slots(em.med.cd.nphases);
        s.push_str(&format!("\"Energymeter.{}\"", name.to_uppercase()));
        // Sensor currents (Target).
        load_temp_x(&mut t, &em.med.sensor_current, n);
        write_temp_x(&mut s, &t);
        // Calculated Currents — `Cabs(CalculatedCurrent^[i])`, read from the
        // head of the buffer (Pascal indexes 1..Nphases with **no** metered-
        // terminal offset, unlike `CalcAllocationFactors`).
        zero_temp_x(&mut t);
        for (i, slot) in t.iter_mut().enumerate().take(n) {
            *slot = em
                .med
                .calculated_current
                .get(i)
                .map(|c| c.norm())
                .unwrap_or(0.0);
        }
        write_temp_x(&mut s, &t);
        // Percent Error (reuses the calculated magnitudes still in `TempX`).
        percent_error(&mut t, &em.med.sensor_current, n);
        write_temp_x(&mut s, &t);
        s.push('\n');
    }

    // ---- Sensors -------------------------------------------------------
    s.push('\n');
    s.push_str("\"Sensors\" \n");
    s.push_str(
        "\"Sensor\", \"I1 Target\", \"I2 Target\", \"I3 Target\", \
         \"I1 Calc\", \"I2 Calc\", \"I3 Calc\", \
         \"I1 %Err\", \"I2 %Err\", \"I3 %Err\", \
         \"V1 Target\", \"V2 Target\", \"V3 Target\", \
         \"V1 Calc\", \"V2 Calc\", \"V3 Calc\", \
         \"V1 %Err\", \"V2 %Err\", \"V3 %Err\", \
         \"WLS Voltage Err\", \"WLS Current Err\"\n",
    );
    for &r in &ckt.sensors {
        let name = classes[r.class_ord()].arena[r.index()]
            .data()
            .name()
            .to_string();
        let mut row = String::new();
        {
            let Some(sen) = classes[r.class_ord()].arena.get::<Sensor>(r.index()) else {
                continue;
            };
            if !sen.med.cd.enabled {
                continue;
            }
            let n = temp_x_slots(sen.med.cd.nphases);
            row.push_str(&format!("\"Sensor.{}\"", name.to_uppercase()));
            // Sensor currents (Target).
            load_temp_x(&mut t, &sen.med.sensor_current, n);
            write_temp_x(&mut row, &t);
            // Calculated Currents.
            zero_temp_x(&mut t);
            for (i, slot) in t.iter_mut().enumerate().take(n) {
                *slot = sen
                    .med
                    .calculated_current
                    .get(i)
                    .map(|c| c.norm())
                    .unwrap_or(0.0);
            }
            write_temp_x(&mut row, &t);
            // Percent Error.
            percent_error(&mut t, &sen.med.sensor_current, n);
            write_temp_x(&mut row, &t);
            // Sensor Voltage (Target).
            load_temp_x(&mut t, &sen.med.sensor_voltage, n);
            write_temp_x(&mut row, &t);
            // Calculated Voltage.
            zero_temp_x(&mut t);
            for (i, slot) in t.iter_mut().enumerate().take(n) {
                *slot = sen
                    .med
                    .calculated_voltage
                    .get(i)
                    .map(|c| c.norm())
                    .unwrap_or(0.0);
            }
            write_temp_x(&mut row, &t);
            // Percent Error.
            percent_error(&mut t, &sen.med.sensor_voltage, n);
            write_temp_x(&mut row, &t);
        }
        // WLS Errors — `Get_WLSVoltageError` is a pure read, `Get_WLSCurrentError`
        // re-derives `SensorCurrent` from P/Q first (see the fn doc).
        let (wls_v, wls_i) = {
            let sen = classes[r.class_ord()]
                .arena
                .get_mut::<Sensor>(r.index())
                .expect("sensor list holds Sensor objects");
            let v = sen.wls_voltage_error();
            let i = sen.wls_current_error();
            (v, i)
        };
        row.push_str(&format!(
            ", {}, {}\n",
            format::g(wls_v, 6),
            format::g(wls_i, 6)
        ));
        s.push_str(&row);
    }

    s
}
