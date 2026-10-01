//! ESPVLControl synthetic gate — the regression net for everything the class's
//! `Sample` does that **no oracle channel can see**.
//!
//! ESPVLControl is an Energy-Storage/PV local controller whose `Sample` — proven
//! against the pinned dss-python 0.15.7 oracle — is a **faithful no-op on all
//! observable circuit state**: a System Controller "redispatches" a fleet of
//! *other ESPVLControl objects* (the Pascal pointer list holds `TESPVLControlObj`,
//! type-confused as `TGeneratorObj`), writing only a non-electrical phantom field;
//! a Local Controller does nothing; and the control never pushes a control-queue
//! action. So the dispatched generators are never touched, `ControlIterations`
//! stays 1, and the solution with the control present is byte-identical to the
//! solution without it. See `elements::control::espvl_control` for the full note.
//!
//! **Split of duties with the corpus deck**
//! (`tests/corpus/controls/espvlcontrol/espvlcontrol.dss`, live-gated on
//! `capi_v0145`). The deck owns the class's *observable* surface: the full
//! 14-property table of six controls, the `''` rendering of an unset `Type`, and
//! `scan.LocalControlWeights` `''` → `'[ 1 1 1 1 1]'` — the ONE Sample-derived
//! observable, produced by `MakeLocalControlList`'s type-blind sweep of every
//! *enabled* control — plus the no-op contract of the in-band path (generator
//! bases held, event log and control queue empty). Its System Controllers
//! monitor a branch held inside the band, so the redispatch never fires there:
//! on the pinned oracle the redispatch's type-confused `kWBase` store corrupts
//! the process. So `PDiff`/`HalfkWBand`, the weights, `TotalWeight`, the
//! `Max(1.0, …)` floor, the named-list branch, the control-iteration count and
//! the no-op contract while the redispatch fires are pinned **here** and in
//! `elements::control::espvl_control::tests` — this module is their only
//! regression net, and must not be thinned on the grounds that a corpus deck
//! exists. `system_controller_inside_the_band_writes_nothing` steps the deck
//! itself and fails on a step that leaves the band.
//!
//! These decks are transcribed oracle gates: the same circuit run on the Rust
//! engine, pinned against values captured from dss-python 0.15.7 (the generator
//! bases, the monitored-line terminal power, and the control-iteration count). The
//! `with == without` assertions independently prove the no-op without the oracle.

use crate::exec::*;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1.0)
}

/// A two-bus feeder (line → load + two generators at the load bus), optionally
/// carrying an ESPVLControl on the line. The generators keep their input bases —
/// ESPVLControl never dispatches them.
fn feeder(load_kw: f64, espvl: Option<&str>) -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.espvl basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command(&format!(
        "New load.ld1 bus1=b1 phases=3 kv=12.47 kw={load_kw} pf=0.95"
    ));
    dss.command("New generator.g1 bus1=b1 phases=3 kv=12.47 kw=600 pf=1 model=1");
    dss.command("New generator.g2 bus1=b1 phases=3 kv=12.47 kw=400 pf=0.9 model=1");
    if let Some(props) = espvl {
        dss.command(&format!(
            "New espvlcontrol.e1 element=line.l1 terminal=1 {props}"
        ));
    }
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "solve: {:?}", dss.errors());
    dss
}

/// `(g1, g2)` generator bases as `(kW, kvar)` tuples.
fn gens(dss: &Dss) -> ((f64, f64), (f64, f64)) {
    (
        dss.generator_kw_kvar("g1").expect("g1"),
        dss.generator_kw_kvar("g2").expect("g2"),
    )
}

/// The monitored line's terminal-1 total `(P, Q)` in kW/kvar.
fn line_l1_power(dss: &mut Dss) -> (f64, f64) {
    let snaps = dss.snapshot_elements();
    let s = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("Line.l1"))
        .expect("Line.l1 snapshot");
    let p: f64 = s.powers[0..3].iter().map(|s| s.re).sum();
    let q: f64 = s.powers[0..3].iter().map(|s| s.im).sum();
    (p, q)
}

fn assert_gens_unchanged(dss: &Dss, ctx: &str) {
    let ((g1kw, g1kvar), (g2kw, g2kvar)) = gens(dss);
    // dss-python 0.15.7: the generators keep their input bases (ESPVLControl never
    // touches them — proven no-op).
    assert!(rel(g1kw, 600.0) < 1e-9, "{ctx}: g1 kW = {g1kw}");
    assert!(rel(g1kvar, 0.0) < 1e-9, "{ctx}: g1 kvar = {g1kvar}");
    assert!(rel(g2kw, 400.0) < 1e-9, "{ctx}: g2 kW = {g2kw}");
    assert!(
        rel(g2kvar, 193.72884193514102) < 1e-6,
        "{ctx}: g2 kvar = {g2kvar}"
    );
}

fn control_iterations(dss: &Dss) -> i32 {
    dss.circuit().unwrap().solution.control_iteration
}

/// The rest of the no-op contract: no event-log line and no pending action.
fn assert_nothing_logged_or_queued(dss: &Dss, ctx: &str) {
    assert!(
        dss.event_log().is_empty(),
        "{ctx}: event log {:?}",
        dss.event_log()
    );
    let queue = dss.control_queue_rows();
    assert!(queue.is_empty(), "{ctx}: control queue {queue:?}");
}

#[test]
fn system_controller_pdiff_negative_is_noop() {
    // Load 4000 kW → monitored P_kW (~3020) − FkWLimit 8000 = PDiff < 0, magnitude
    // ≫ HalfkWBand, so the redispatch loop fires (onto the phantom field). The
    // generators and the network are untouched; ControlIterations stays 1.
    let mut dss = feeder(4000.0, Some("type=SystemController"));
    assert_gens_unchanged(&dss, "SysCtrl PDiff<0");
    assert_nothing_logged_or_queued(&dss, "SysCtrl PDiff<0");
    assert_eq!(control_iterations(&dss), 1, "ESPVLControl queues no action");

    let (p, q) = line_l1_power(&mut dss);
    assert!(rel(p, 3020.20512213) < 1e-6, "line.l1 P = {p}");
    assert!(rel(q, 1161.27356749) < 1e-6, "line.l1 Q = {q}");
}

#[test]
fn system_controller_pdiff_positive_is_noop() {
    // Load 12000 kW → PDiff > 0 (~11283 − 8000), so the other branch of the
    // |PDiff|>band test fires; still a no-op on the circuit.
    let mut dss = feeder(12000.0, Some("type=SystemController"));
    assert_gens_unchanged(&dss, "SysCtrl PDiff>0");
    assert_nothing_logged_or_queued(&dss, "SysCtrl PDiff>0");
    assert_eq!(control_iterations(&dss), 1);

    let (p, q) = line_l1_power(&mut dss);
    assert!(rel(p, 11283.08121909) < 1e-6, "line.l1 P = {p}");
    assert!(rel(q, 4317.19724434) < 1e-6, "line.l1 Q = {q}");
}

#[test]
fn local_controller_with_lists_is_noop() {
    // A Local Controller naming PV/Storage lists: `Sample` builds no pointer list
    // (Ftype≠1) and dispatches nothing. The named PV/Storage elements need not even
    // exist (they are dead lists upstream).
    let mut dss = feeder(
        4000.0,
        Some("type=LocalController PVSystemList=[p1] StorageList=[s1]"),
    );
    assert_gens_unchanged(&dss, "LocalCtrl");
    assert_nothing_logged_or_queued(&dss, "LocalCtrl");
    assert_eq!(control_iterations(&dss), 1);

    let (p, q) = line_l1_power(&mut dss);
    assert!(rel(p, 3020.20512213) < 1e-6, "line.l1 P = {p}");
    assert!(rel(q, 1161.27356749) < 1e-6, "line.l1 Q = {q}");
}

#[test]
fn control_present_equals_control_absent() {
    // The strongest no-op proof, independent of the oracle constants: the solved
    // model is byte-identical with and without the ESPVLControl, for both PDiff
    // signs and both controller types. (If the control were silently skipped this
    // would still pass — but `control_iterations == 1` above + the props gate
    // confirm it is registered, sampled, and dumps faithfully.)
    for load in [4000.0, 12000.0] {
        let mut without = feeder(load, None);
        for ctrl in [
            "type=SystemController",
            "type=LocalController PVSystemList=[p1] StorageList=[s1]",
            "type=SystemController LocalControlList=[e2]",
        ] {
            let mut with = feeder(load, Some(ctrl));
            assert_eq!(
                gens(&with),
                gens(&without),
                "load {load}, ctrl `{ctrl}`: generator bases differ"
            );
            assert_nothing_logged_or_queued(&with, &format!("load {load}, ctrl `{ctrl}`"));
            assert_eq!(
                line_l1_power(&mut with),
                line_l1_power(&mut without),
                "load {load}, ctrl `{ctrl}`: monitored-line power differs"
            );
        }
    }
}

#[test]
fn two_system_controllers_scan_each_other() {
    // Two System Controllers with no LocalControlList each scan *all* enabled
    // ESPVLControls (the Pascal `ParentClass` scan includes both, and self). This
    // exercises the cross-object phantom-write path through the dispatch env; it
    // remains a no-op on the circuit.
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.espvl basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=4000 pf=0.95");
    dss.command("New generator.g1 bus1=b1 phases=3 kv=12.47 kw=600 pf=1 model=1");
    dss.command("New generator.g2 bus1=b1 phases=3 kv=12.47 kw=400 pf=0.9 model=1");
    dss.command("New espvlcontrol.e1 element=line.l1 terminal=1 type=SystemController");
    dss.command("New espvlcontrol.e2 element=line.l1 terminal=1 type=SystemController");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "solve: {:?}", dss.errors());

    assert_gens_unchanged(&dss, "two SysCtrl");
    assert_nothing_logged_or_queued(&dss, "two SysCtrl");
    assert_eq!(control_iterations(&dss), 1);
    let (p, q) = line_l1_power(&mut dss);
    assert!(rel(p, 3020.20512213) < 1e-6, "line.l1 P = {p}");
    assert!(rel(q, 1161.27356749) < 1e-6, "line.l1 Q = {q}");
}

#[test]
fn system_controller_resolves_named_subordinate() {
    // A System Controller whose `LocalControlList` names a real (enabled) Local
    // Controller: this drives the `find_enabled_espvl` resolution + the cross-object
    // phantom write to a *distinct* subordinate through the dispatch env. Still a
    // no-op on the circuit (dss-python 0.15.7).
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.espvl basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=4000 pf=0.95");
    dss.command("New generator.g1 bus1=b1 phases=3 kv=12.47 kw=600 pf=1 model=1");
    dss.command("New generator.g2 bus1=b1 phases=3 kv=12.47 kw=400 pf=0.9 model=1");
    dss.command("New espvlcontrol.loc1 element=line.l1 terminal=1 type=LocalController");
    dss.command(
        "New espvlcontrol.sys element=line.l1 terminal=1 type=SystemController \
         LocalControlList=[loc1]",
    );
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "solve: {:?}", dss.errors());

    assert_gens_unchanged(&dss, "SysCtrl names subordinate");
    assert_nothing_logged_or_queued(&dss, "SysCtrl names subordinate");
    assert_eq!(control_iterations(&dss), 1);
    let (p, q) = line_l1_power(&mut dss);
    assert!(rel(p, 3020.20512213) < 1e-6, "line.l1 P = {p}");
    assert!(rel(q, 1161.27356749) < 1e-6, "line.l1 Q = {q}");
}

/// The feeder of [`feeder`] with a System Controller `sys` on `line.l1` that
/// redispatches the named fleet `[loc1, loc2]` with weights `[3, 1]`.
fn redispatch_feeder(load_kw: f64) -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.espvl basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command(&format!(
        "New load.ld1 bus1=b1 phases=3 kv=12.47 kw={load_kw} pf=0.95"
    ));
    dss.command("New generator.g1 bus1=b1 phases=3 kv=12.47 kw=600 pf=1 model=1");
    dss.command("New generator.g2 bus1=b1 phases=3 kv=12.47 kw=400 pf=0.9 model=1");
    dss.command("New espvlcontrol.loc1 element=line.l1 terminal=1 type=LocalController");
    dss.command("New espvlcontrol.loc2 element=line.l1 terminal=1 type=LocalController");
    dss.command(
        "New espvlcontrol.sys element=line.l1 terminal=1 type=SystemController \
         LocalControlList=[loc1, loc2] LocalControlWeights=[3, 1]",
    );
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "solve: {:?}", dss.errors());
    dss
}

/// The phantom kW bases of the named ESPVLControls, in the order given.
fn phantoms<const N: usize>(dss: &Dss, names: [&str; N]) -> [f64; N] {
    let arena = &dss.classes[dss.class_by_name["espvlcontrol"]].arena;
    let all = arena
        .all::<crate::elements::control::EspvlControl>()
        .expect("ESPVLControl arena");
    names.map(|name| {
        all.iter()
            .find(|e| e.ccd.cd.obj.name().eq_ignore_ascii_case(name))
            .expect(name)
            .phantom_kw_base()
    })
}

/// The engine's named-list redispatch: the monitored terminal power of the
/// solved line, the resolution of `LocalControlList` and the write-back of each
/// entry's phantom kW base, with the generators, the event log and the control
/// queue untouched. No oracle reads this store (upstream it is the type-confused
/// `Gen.kWBase` write), so the expected values are the weighted shares of
/// `PDiff = P - 8000 kW` with `P` the line power the neighbouring tests pin
/// against dss-python 0.15.7.
#[test]
fn system_controller_redispatch_writes_the_weighted_deficit() {
    // Load 12000 kW: P = 11283.08121909 kW, PDiff = +3283.08121909 kW, so loc1
    // takes 3/4 (2462.3109143175) and loc2 1/4 (820.7703047725); `sys` is not in
    // its own list and keeps 0.
    let mut dss = redispatch_feeder(12000.0);
    let (p, _) = line_l1_power(&mut dss);
    assert!(rel(p, 11283.08121909) < 1e-6, "line.l1 P = {p}");
    let p_diff = p - 8000.0;
    let [sys, loc1, loc2] = phantoms(&dss, ["sys", "loc1", "loc2"]);
    assert_eq!(sys, 0.0, "sys is not in its own list");
    assert!(rel(loc1, 2462.3109143175) < 1e-6, "loc1 = {loc1}");
    assert!(rel(loc2, 820.7703047725) < 1e-6, "loc2 = {loc2}");
    assert!(
        rel(loc1, 0.75 * p_diff) < 1e-12,
        "loc1 = {loc1}, PDiff = {p_diff}"
    );
    assert!(
        rel(loc2, 0.25 * p_diff) < 1e-12,
        "loc2 = {loc2}, PDiff = {p_diff}"
    );
    assert_gens_unchanged(&dss, "SysCtrl redispatch");
    assert_nothing_logged_or_queued(&dss, "SysCtrl redispatch");
    assert_eq!(control_iterations(&dss), 1);

    // A second solve reads each base back and adds the same share again.
    dss.command("Solve mode=snap");
    let [_, loc1_2, loc2_2] = phantoms(&dss, ["sys", "loc1", "loc2"]);
    assert!(
        rel(loc1_2, 2.0 * loc1) < 1e-12,
        "loc1 after 2 solves = {loc1_2}"
    );
    assert!(
        rel(loc2_2, 2.0 * loc2) < 1e-12,
        "loc2 after 2 solves = {loc2_2}"
    );
    assert_nothing_logged_or_queued(&dss, "second SysCtrl redispatch");

    // Load 4000 kW: PDiff = 3020.20512213 - 8000 kW < 0, both shares are
    // negative and the `Max(1.0, ..)` floor holds both entries at 1 kW.
    let dss = redispatch_feeder(4000.0);
    assert_eq!(phantoms(&dss, ["sys", "loc1", "loc2"]), [0.0, 1.0, 1.0]);
    assert_gens_unchanged(&dss, "floored SysCtrl redispatch");
    assert_nothing_logged_or_queued(&dss, "floored SysCtrl redispatch");
}

/// The corpus deck `controls/espvlcontrol/espvlcontrol.dss`, stepped as the live
/// gate steps it: on each of its 12 daily steps `sys` and `scan` read
/// `PDiff = P - 8000 kW` on `line.l2` inside `HalfkWBand` (50 kW), so no control
/// writes a phantom base, the control loop runs once and nothing is logged or
/// queued. The pinned dss-python 0.15.7 measured `PDiff` at +0.105..+0.142 kW on
/// those steps (2026-10-01). The deck keeps the redispatch silent because it
/// corrupts the pinned oracle's process, and no compared cell shows it firing,
/// so this test is the one that fails on a deck edit that leaves the band.
#[test]
fn system_controller_inside_the_band_writes_nothing() {
    const CONTROLS: [&str; 6] = ["loc1", "loc2", "off", "plain", "sys", "scan"];
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/controls/espvlcontrol/espvlcontrol.dss");
    assert!(deck.is_file(), "corpus deck missing: {deck:?}");
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));
    assert!(dss.errors().is_empty(), "compile: {:?}", dss.errors());
    for step in 0..12 {
        dss.command("solve");
        assert!(dss.errors().is_empty(), "step {step}: {:?}", dss.errors());
        let p_diff = {
            let snaps = dss.snapshot_elements();
            let l2 = snaps
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case("Line.l2"))
                .expect("Line.l2 snapshot");
            l2.powers[0..3].iter().map(|s| s.re).sum::<f64>() - 8000.0
        };
        assert!(
            p_diff.abs() < 50.0,
            "step {step}: line.l2 PDiff = {p_diff} kW"
        );
        assert_eq!(
            phantoms(&dss, CONTROLS),
            [0.0; 6],
            "step {step}: phantom bases of {CONTROLS:?}"
        );
        assert_eq!(control_iterations(&dss), 1, "step {step}");
        assert_nothing_logged_or_queued(&dss, &format!("step {step}"));
    }
}
