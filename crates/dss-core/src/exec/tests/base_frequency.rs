//! CF-A (TC-1): circuit elements inherit the circuit's fundamental frequency at
//! creation. Pascal `TDSSCktElement.Create` sets `BaseFrequency :=
//! ActiveCircuit.Fundamental` (CktElement.pas:203); `TVsourceObj.Create` /
//! `TIsourceObj.Create` then set `SrcFrequency := BaseFrequency` (VSource.pas:644,
//! Isource.pas:319); `TLineCodeObj.Create` sets `BaseFrequency :=
//! ActiveCircuit.Fundamental` (LineCode.pas:493). `ActiveCircuit.Fundamental`
//! starts at `DSS.DefaultBaseFreq` (Circuit.pas:391), so `Set
//! DefaultBaseFrequency=50` *before* `New circuit` runs the whole model at 50 Hz.
//!
//! Before the fix `CktElementData::new` hardcoded 60 and VSource `src_frequency`
//! 60, so a European feeder's source frequency (60) != solution frequency (50):
//! the mismatch check (`VSource.pas:1071`) zeroed Vmag and the entire feeder
//! solved to ~0 V.

use super::common::{query, query_f64};
use crate::exec::*;
use crate::util::sqrt3;

/// The implicit `vsource.source` created by `New circuit` inherits the 50 Hz
/// fundamental (both its `frequency` and `basefreq`), and the whole feeder is
/// alive after the solve rather than collapsing to zero volts.
#[test]
fn european_50hz_feeder_is_alive() {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=50"); // BEFORE the circuit — European system
    dss.command("New circuit.euro basekv=11 pu=1.05");
    dss.command(
        "New Line.l1 bus1=sourcebus bus2=loadbus length=1 units=km \
             r1=0.1 x1=0.1 c1=0 c0=0",
    );
    dss.command("New Load.ld1 bus1=loadbus phases=3 kv=11 kw=100 pf=0.95");
    dss.command("Set voltagebases=[11]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // The source inherited the 50 Hz fundamental (not the hardcoded 60).
    assert_eq!(query_f64(&mut dss, "vsource.source.frequency"), 50.0);
    assert_eq!(query_f64(&mut dss, "vsource.source.basefreq"), 50.0);

    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved, "50 Hz feeder must solve");
    assert_eq!(ckt.solution.frequency, 50.0, "solution frequency");
    let vbase = 11.0e3 / sqrt3();
    let vmax = (1..=ckt.num_nodes)
        .map(|i| ckt.solution.node_v[i].norm())
        .fold(0.0_f64, f64::max);
    assert!(
        vmax > 0.5 * vbase,
        "feeder is dead (source Vmag zeroed by a freq mismatch): vmax={vmax}, vbase={vbase}"
    );
}

/// Every circuit element — not just the source — inherits the 50 Hz base
/// frequency (the generic `add_object` seeding, Pascal's base-class constructor),
/// EXCEPT a Monitor, which `TMonitorObj.Create` hard-pins to 60 Hz
/// (Monitor.pas:472) — oracle-verified.
#[test]
fn all_elements_inherit_the_50hz_base_frequency() {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=50");
    dss.command("New circuit.euro basekv=11");
    dss.command("New Line.l1 bus1=sourcebus bus2=b2 length=1 r1=0.1 x1=0.1 c1=0 c0=0");
    dss.command("New Load.ld1 bus1=b2 phases=3 kv=11 kw=50");
    dss.command("New Capacitor.c1 bus1=b2 phases=3 kv=11 kvar=100");
    dss.command("New EnergyMeter.em1 element=line.l1 terminal=1");
    dss.command("New Sensor.se1 element=line.l1 terminal=1");
    dss.command("New Monitor.m1 element=line.l1 terminal=1 mode=0");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for elem in [
        "line.l1",
        "load.ld1",
        "capacitor.c1",
        "vsource.source",
        "energymeter.em1",
        "sensor.se1",
    ] {
        assert_eq!(
            query_f64(&mut dss, &format!("{elem}.basefreq")),
            50.0,
            "{elem} base frequency"
        );
    }
    // The Monitor is the lone exception — always 60 Hz (Monitor.pas:472).
    assert_eq!(
        query_f64(&mut dss, "monitor.m1.basefreq"),
        60.0,
        "monitor base frequency is hard-pinned to 60"
    );
}

/// Dedicated pin for the reproduced UPSTREAM Monitor BaseFrequency bug (the
/// `TODO(compat)` at `create_object_no_edit`): `TMonitorObj.Create` hard-pins
/// `Basefrequency := 60.0` (Monitor.pas:472 == r4133:552), overriding the
/// base-class `BaseFrequency := ActiveCircuit.Fundamental`. Under `Set
/// DefaultBaseFrequency=50` every other element reads 50, but the monitor reads
/// 60 — and that value is the `fBase` a mode-4 monitor feeds into `FlickerMeter`,
/// where `fBase = 50.0` would select the IEC 61000-4-15 230V/50 Hz lamp curve.
/// So a 50 Hz mode-4 monitor computes Pst with the wrong (60 Hz) lamp curve
/// unless `basefreq=50` is set. Both gating oracles pin 60.0; a "clean" inherit
/// would break parity — hence reproduced, not fixed (until Stage F).
#[test]
fn monitor_basefreq_pins_60hz_upstream_bug() {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=50");
    dss.command("New circuit.euro basekv=11");
    dss.command("New Line.l1 bus1=sourcebus bus2=b2 length=1 r1=0.1 x1=0.1 c1=0 c0=0");
    dss.command("New Monitor.m1 element=line.l1 terminal=1 mode=4");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    // The circuit fundamental is 50, but the monitor is hard-pinned to 60.
    assert_eq!(dss.circuit().unwrap().fundamental, 50.0);
    assert_eq!(
        query_f64(&mut dss, "monitor.m1.basefreq"),
        60.0,
        "monitor basefreq must reproduce the upstream 60.0 override (Monitor.pas:472)"
    );
    // An explicit `basefreq=` still lets the user correct it (flows into
    // FlickerMeter correctly on both engines).
    dss.command("Edit monitor.m1 basefreq=50");
    assert_eq!(query_f64(&mut dss, "monitor.m1.basefreq"), 50.0);
}

/// A LineCode (a DSS_OBJECT) created after `New circuit` at 50 Hz inherits the
/// fundamental and computes its shunt admittance at 50 Hz. A `basefreq=` on the
/// element still overrides.
#[test]
fn linecode_inherits_and_basefreq_override_wins() {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=50");
    dss.command("New circuit.euro basekv=11");
    dss.command("New LineCode.lc1 nphases=3 r1=0.1 x1=0.1 c1=10 c0=5 units=km");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query_f64(&mut dss, "linecode.lc1.basefreq"), 50.0);

    // An explicit `basefreq=` on the element supersedes the inherited value.
    dss.command("New LineCode.lc2 nphases=3 r1=0.1 x1=0.1 c1=10 c0=5 basefreq=60");
    assert_eq!(query_f64(&mut dss, "linecode.lc2.basefreq"), 60.0);
}

/// A per-element `frequency=`/`basefreq=` edit still overrides the inherited
/// default on the VSource (the seeding happens before `edit_active`).
#[test]
fn vsource_frequency_edit_overrides_inheritance() {
    let mut dss = Dss::new();
    dss.command("Set DefaultBaseFrequency=50");
    dss.command("New circuit.euro basekv=11");
    // Confirm the pre-edit inheritance, then override.
    assert_eq!(query_f64(&mut dss, "vsource.source.frequency"), 50.0);
    dss.command("Edit vsource.source frequency=60");
    assert_eq!(query_f64(&mut dss, "vsource.source.frequency"), 60.0);
    // basefreq is independent of the frequency= property.
    assert_eq!(query(&mut dss, "vsource.source.basefreq"), "50");
}
