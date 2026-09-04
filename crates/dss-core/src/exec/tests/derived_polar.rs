//! The three polar element surfaces of `GOLDEN_REBASE_PLAN.md` G1.3a —
//! `CktElement.CurrentsMagAng`, `CktElement.VoltagesMagAng` and
//! `CktElement.Residuals` — as they come out of
//! [`Dss::snapshot_elements`](crate::exec::Dss::snapshot_elements).
//!
//! These are the sub-step's expected-value pins: the live corpus gate compares
//! the three arrays against both oracle channels case by case, and these tests
//! nail down, in-engine and against numbers read from the pinned oracle
//! (dss-python 0.15.7 / dss_capi 0.14.5, `tools/golden/PIN.txt`), the four
//! properties a floor-based comparison cannot express on its own:
//!
//! 1. each terminal's residual sums **its own** conductors (the
//!    `k := (i-1)*Nconds` offset that the `Export SeqCurrents` report path
//!    drops — CLAUDE.md upstream bug 1 — and that both API paths carry);
//! 2. the angle is `CDANG`'s *truncated* `57.29577951` image, not the
//!    full-precision `180/π` one;
//! 3. `VoltagesMagAng` is read through the element's own `NodeRef`, so a
//!    grounded conductor is exactly zero;
//! 4. a never-energized element carries no voltage payload at all.
//!
//! Pascal: r4133 `Version8/Source/DDLL/DCktElement.pas:827` (`11:` Residuals),
//! `:1058` (`18:` CurrentsMagAng), `:1082` (`19:` VoltagesMagAng), over
//! `Shared/Ucomplex.pas:118` `CDANG` / `:131` `CtoPOLARdeg`; capi
//! `CAPI/CAPI_CktElement.pas:541` and `CAPI/CAPI_Alt.pas:1043`/`:1072`. All
//! three are fastdss `_columns` surfaces (`dss/ICktElement.py:58`/`:64`/`:67`
//! on `origin/fastdss`).

use crate::exec::{Dss, ElementSnapshot};
use crate::support::complexutil::cdang;

/// The `feeder` tolerance tier — `tests/harness/mod.rs::tol_for` (`"feeder"`,
/// `:967-976`), the tier IEEE13 is gated at, restated here because a `src`
/// unit test cannot reach the integration harness. Every band below is that
/// tier's *derived* image (`tests/TOLERANCE_NOTES.md`), never a fresh number.
const I_ABS: f64 = 1e-5;
const I_REL: f64 = 1e-7;
const V_ABS: f64 = 1e-6;
const V_REL: f64 = 1e-8;
/// Full-precision radians→degrees. A *tolerance* is not a printed value, so the
/// band conversion uses the exact constant even though `CDANG` itself is
/// truncated (`support::complexutil`'s `TRUNCATED_RAD_TO_DEG`).
const RAD_TO_DEG: f64 = 57.29577951308232;

/// Compile a vendored corpus deck by absolute path and return the live engine.
///
/// All three decks read here are read-only: `IEEE13Nodeckt.dss` has no active
/// `export`/`show`/`save` (its `Show` block is commented out) and
/// `controls/fuse/midi_fuse.dss` / `controls/capcontrol/capcontrol_time.dss`
/// are in-repo synthetic decks that write no file, so no directory guard is
/// needed. Never `.inputs/` — the corpus tree is the one the live gate reads
/// (CLAUDE.md).
fn compile_corpus_deck(rel: &str) -> Dss {
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel);
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));
    assert!(dss.errors().is_empty(), "{rel}: {:?}", dss.errors());
    dss
}

const IEEE13: &str = "electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss";

fn elem<'a>(snaps: &'a [ElementSnapshot], name: &str) -> &'a ElementSnapshot {
    snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("{name} not in the snapshot"))
}

/// The magnitude band of a gated current: the rectangular floor, inherited
/// unchanged (`| |a| − |b| | <= |a − b|`).
fn i_band(mag: f64) -> f64 {
    I_ABS + I_REL * mag
}

/// The angular image of a magnitude band: a phasor known to ±`band` has its
/// argument known to ±`asin(band/|z|) <= rad2deg·band/|z|` degrees.
fn ang_band(band: f64, mag: f64) -> f64 {
    RAD_TO_DEG * band / mag
}

// Expected-value pin — `Residuals` carry the terminal offset: `CktElement.Residuals`
// row `t` sums terminal `t`'s own conductors. This is the API-path sibling of
// `golden_reports.rs::export_seqcurrents_iresidual_sums_the_rows_own_terminal`:
// there the port *fixes* a real upstream defect, here both oracles are already
// correct (`DCktElement.pas:842` and `CAPI_CktElement.pas:562` both offset by
// `(i-1)*Nconds`) and the pin guards the port against acquiring it.
/// Each terminal's residual is the sum of that terminal's own conductor
/// currents, not terminal 1's value repeated.
#[test]
fn residuals_sum_the_rows_own_terminal() {
    let mut dss = compile_corpus_deck(IEEE13);
    let snaps = dss.snapshot_elements();

    // The discriminating sample: a loaded 3-phase line whose two terminals both
    // carry a healthy residual, ~180 degrees apart because the current flows
    // through. Oracle (dss-python 0.15.7): terminal 1 = 143.99329593567273 A
    // at 24.036559579635142 deg, terminal 2 = 143.99329071146772 A at
    // -155.96344107788147 deg. Missing the offset would print terminal 1's
    // angle for terminal 2 — 180 deg away, ~2.4e6 x the band below.
    let l = elem(&snaps, "Line.650632");
    assert_eq!(l.residuals.len(), 2, "one residual per terminal");
    let nconds = 3usize;
    let oracle = [
        (143.99329593567273_f64, 24.036559579635142_f64),
        (143.99329071146772, -155.96344107788147),
    ];
    for (t, &(o_mag, o_ang)) in oracle.iter().enumerate() {
        // The residual is a *sum* of conductor currents, so its band is the sum
        // of their bands — the derivation `harness::compare_element_channels`
        // already uses for element losses: nconds*i_abs + i_rel*sum_c|I_c|.
        // Terminal-major layout read through a per-terminal chunk (no flat
        // offset arithmetic at the call site).
        let sum_abs: f64 = l
            .currents
            .chunks(nconds)
            .nth(t)
            .unwrap()
            .iter()
            .map(|z| z.norm())
            .sum();
        let band = nconds as f64 * I_ABS + I_REL * sum_abs;
        let got = l.residuals[t];
        assert!(
            (got.mag - o_mag).abs() <= band,
            "Line.650632 terminal {} residual |I| = {} A; oracle {o_mag} A, \
             band {band} A (sum|I_c| = {sum_abs})",
            t + 1,
            got.mag,
        );
        assert!(
            (got.ang - o_ang).abs() <= ang_band(band, o_mag),
            "Line.650632 terminal {} residual angle = {} deg; oracle {o_ang} deg, \
             band {} deg",
            t + 1,
            got.ang,
            ang_band(band, o_mag),
        );
    }
    assert!(
        (l.residuals[0].ang - l.residuals[1].ang).abs() > 179.0,
        "the two terminals' residuals must not be the same phasor: {:?}",
        l.residuals,
    );

    // The sample G1.3a's spec names, kept for the record: a switch line whose
    // terminal-1 residual is 2.830776231e-05 A (oracle 2.8307762314793113e-05,
    // port 2.8307755907964043e-05 — 6.4e-12 A apart) while terminal 2's is
    // numerically zero (oracle 1.639614891002561e-12 A, port
    // 3.6662903532026344e-12). Note *why* it is only a shape witness and not
    // the discriminating leg: the whole channel sits under the derived absolute
    // floor here (3*1e-5 A > 2.83e-5 A), so a dropped offset would not be
    // visible on this element — near-zero residuals are masked by the floor by
    // construction.
    let sw = elem(&snaps, "Line.671680");
    assert_eq!(sw.residuals.len(), 2);
    let band = 3.0 * I_ABS + I_REL * (0..3).map(|c| sw.currents[c].norm()).sum::<f64>();
    assert!(
        (sw.residuals[0].mag - 2.8307762314793113e-05).abs() <= band,
        "Line.671680 terminal 1 residual |I| = {} A; oracle 2.8307762314793113e-05 A",
        sw.residuals[0].mag,
    );
    assert!(
        sw.residuals[1].mag < 1e-9,
        "Line.671680 terminal 2 residual |I| = {} A; the oracle reads \
         1.639614891002561e-12 A (numerically zero), NOT terminal 1's \
         2.8307762314793113e-05 A",
        sw.residuals[1].mag,
    );
}

// Expected-value pin — the polar angle uses the truncated `CDANG`: the polar surfaces
// render angles through `CDANG`'s truncated `57.29577951` (`Ucomplex.pas:118`,
// identical in capi `DSSUcomplex.pas`), the constant both gating oracles carry
// — compat-tagged at `support::complexutil`'s constants, which own the clean
// fix.
/// `CurrentsMagAng` is `ctopolardeg` of the element's own terminal current,
/// with the truncated degree constant.
#[test]
fn currents_mag_ang_is_the_truncated_ctopolardeg_of_currents() {
    let mut dss = compile_corpus_deck(IEEE13);
    let snaps = dss.snapshot_elements();
    let ld = elem(&snaps, "Load.634a");
    assert_eq!(ld.currents_mag_ang.len(), ld.currents.len());

    // Leg 1 — the oracle, at the derived band. Healthy conductor: |I| =
    // 709.7635837955772 A at -37.79042736585046 deg (dss-python 0.15.7); the
    // port reads 709.763583768285 A at -37.790427380847376 deg, i.e. 2.7e-08 A
    // and 1.5e-08 deg away.
    let (o_mag, o_ang) = (709.7635837955772_f64, -37.79042736585046_f64);
    let got = ld.currents_mag_ang[0];
    let band = i_band(o_mag);
    assert!(
        (got.mag - o_mag).abs() <= band,
        "Load.634a |I1| = {} A; oracle {o_mag} A, band {band} A",
        got.mag,
    );
    assert!(
        (got.ang - o_ang).abs() <= ang_band(band, o_mag),
        "Load.634a angle I1 = {} deg; oracle {o_ang} deg, band {} deg",
        got.ang,
        ang_band(band, o_mag),
    );

    // Leg 2 — the kernel, exactly. The reported angle is `cdang` of the very
    // current the snapshot reports (no second read path, no `f64::atan2`).
    assert_eq!(
        got.ang,
        cdang(ld.currents[0]),
        "the angle must be `ctopolardeg` of the reported current",
    );
    assert_eq!(got.mag, ld.currents[0].norm());

    // Leg 3 — which constant, decided on our own value, because the
    // port-vs-oracle current agreement (~4e-10 rel here) is coarser than the
    // truncation itself (5.38e-11 rel): the full-precision image of the *same*
    // current is -37.790427382880374 deg, 2.03e-09 deg away from what we print,
    // and that gap is exactly `TRUNCATED_RAD_TO_DEG`'s relative deficit
    // ((57.29577951308232 - 57.29577951)/57.29577951). Were the accessor using
    // `180/pi`, the two would coincide.
    let full = ld.currents[0].im.atan2(ld.currents[0].re).to_degrees();
    let gap_rel = (full - got.ang).abs() / got.ang.abs();
    assert!(
        (gap_rel - 5.3797e-11).abs() < 1e-13,
        "the truncated-vs-full-precision gap must be the constant's own \
         deficit: got {gap_rel} (angle {} deg vs full-precision {full} deg)",
        got.ang,
    );
    // The same signature on the oracle's own numbers, for the record: dss_capi
    // reports Line.671680 conductor 1 at 85.68536899029576 deg, while the
    // full-precision image of the very current it reports is
    // 85.68536899490535 deg — 4.61e-09 deg = 5.3796e-11 rel apart, the
    // identical deficit.
    let oracle_gap_rel = (85.68536899490535_f64 - 85.68536899029576).abs() / 85.68536899029576_f64;
    assert!(
        (oracle_gap_rel - 5.3796e-11).abs() < 1e-14,
        "{oracle_gap_rel}"
    );
}

// Expected-value pin — `VoltagesMagAng` follows `NodeRef`: `VoltagesMagAng` maps
// conductors through the element's own `NodeRef` (r4133
// `DCktElement.pas:1096-1100`), which makes it the only oracle-compared surface
// that checks that mapping directly — node voltages themselves are gated
// globally, but which node an element conductor sits on is otherwise only
// implied by YPrim.
/// A grounded conductor (`NodeRef = 0`) reads exactly zero while its phase
/// conductor carries the bus voltage.
#[test]
fn voltages_mag_ang_follows_node_ref_and_grounds_to_zero() {
    let mut dss = compile_corpus_deck(IEEE13);
    let snaps = dss.snapshot_elements();
    // `New Load.634a Bus1=634.1 Phases=1 Conn=Wye` — 2 conductors, the second
    // one the grounded neutral: the oracle's `NodeOrder` is `[1, 0]`.
    let ld = elem(&snaps, "Load.634a");
    assert_eq!(ld.bus_names, vec!["634.1".to_string()]);
    assert_eq!(ld.voltages_mag_ang.len(), 2, "one entry per conductor");

    // Oracle (dss-python 0.15.7): [273.5690491787601 at -3.2801822578573905
    // deg, 0.0 at 0.0 deg]; the port reads 273.5690491896845 at
    // -3.280182272783255 deg.
    let (o_mag, o_ang) = (273.5690491787601_f64, -3.2801822578573905_f64);
    let band = V_ABS + V_REL * o_mag;
    let got = ld.voltages_mag_ang[0];
    assert!(
        (got.mag - o_mag).abs() <= band,
        "Load.634a |V1| = {} V; oracle {o_mag} V, band {band} V",
        got.mag,
    );
    assert!(
        (got.ang - o_ang).abs() <= ang_band(band, o_mag),
        "Load.634a angle V1 = {} deg; oracle {o_ang} deg, band {} deg",
        got.ang,
        ang_band(band, o_mag),
    );
    // The neutral: `NodeRef = 0` is ground and `NodeV[0]` is exactly zero
    // (Pascal's own `// ok if =0` at `DCktElement.pas:1099`), so this is an
    // exact equality, not a band — a conductor mapped to a live node instead
    // would print a bus voltage here.
    assert_eq!(
        (ld.voltages_mag_ang[1].mag, ld.voltages_mag_ang[1].ang),
        (0.0, 0.0),
        "the grounded conductor must read exactly 0 at 0 deg",
    );
}

// Expected-value pin — a never-enabled element has no polar payload: an element
// that was never enabled has no `NodeRef` at all, so it has no voltage
// rendering to report. Upstream splits here: capi guards (`CAPI_Alt.pas:1081`,
// `elem.NodeRef = NIL` -> the one-element `DefaultResult` `[0.0]`), r4133 does
// not and dereferences the nil pointer at `DCktElement.pas:1099` — measured:
// `CktElementV(19)` kills the epri-worker on this very element. The capture
// therefore reads the three fields for enabled elements only (G1.3a spec
// §1.4-H1); this pin is the engine-side half of that contract.
/// A never-enabled element reports `enabled = false`, an empty
/// `voltages_mag_ang`, and zeros for the current-derived pair.
#[test]
fn a_never_enabled_element_has_no_polar_payload() {
    let mut dss = compile_corpus_deck("controls/fuse/midi_fuse.dss");
    let snaps = dss.snapshot_elements();
    // `new line.tie bus1=l4e bus2=l6e switch=yes enabled=no` — the open tie
    // that would close midi_fuse's loop; 3 conductors x 2 terminals.
    let tie = elem(&snaps, "Line.tie");
    assert!(!tie.enabled, "Line.tie is `enabled=no` in the deck");
    assert!(
        tie.voltages_mag_ang.is_empty(),
        "no NodeRef means no voltage rendering; got {:?}",
        tie.voltages_mag_ang,
    );
    // The current-derived pair keeps its shape and reads zero, exactly as both
    // oracles do for a disabled element (`GetCurrents` is `Enabled`-guarded:
    // `PDElements/PDElement.pas:221`, `PCElements/PCElement.pas:298`) — capi
    // returns 12 zeros for `CurrentsMagAng` and 4 for `Residuals` on such an
    // element (2 doubles per entry).
    assert_eq!(tie.currents_mag_ang.len(), 6);
    assert_eq!(tie.residuals.len(), 2);
    assert!(
        tie.currents_mag_ang
            .iter()
            .chain(&tie.residuals)
            .all(|p| p.mag == 0.0 && p.ang == 0.0),
        "a disabled element carries no current: {:?} / {:?}",
        tie.currents_mag_ang,
        tie.residuals,
    );
}

// Expected-value pin — a TIMECONTROL CapControl sits on the monitored terminal: a
// `type=time` CapControl that names an `element=` binds its own bus 1 to that
// MONITORED element's terminal, not to the controlled capacitor's bus — EPRI
// r4133 `Version8/Source/Controls/CapControl.pas:605`
// (`ElmReq := ElmReq and (ControlType <> FOLLOWCONTROL)`, so only FOLLOWCONTROL
// skips the monitored element) and `:622`
// (`Setbus(1, MonitoredElement.GetBus(ElementTerminal))`); the
// `ControlledElement.GetBus(1)` arm at `:633` is the no-monitored-element
// branch. The pinned dss_capi 0.14.5 still carries the pre-`b9bc87b8` form,
// `effElement := ControlledElement` with `ElementTerminal := 1` forced for
// TIMECONTROL *and* FOLLOWCONTROL (`src/Controls/CapControl.pas:597-608`, used
// at `:619`), i.e. the capacitor's bus. The port follows r4133 — the adoption
// `docs/upgrade/DIVERGENCES.md` D11 (part 2) recorded for the
// `effElement`/`Terminal` readback and L8 now records for the bus half. G1.3a's
// `VoltagesMagAng` is the first live channel that can see it, because it reads
// `NodeV` through the element's own `NodeRef`; the `capi_v0145` divergence is
// excluded by ledger entry `capi-capcontrol-time-bus-is-the-capacitors` and the
// `r4133` channel of the same case needs no entry at all.
/// A TIMECONTROL CapControl's `VoltagesMagAng` is the monitored line terminal's
/// voltage (7342.020904321447 V, bus `src`), not the capacitor's bus
/// (7276.216225426737 V, bus `b` — what dss_capi 0.14.5 reports).
#[test]
fn capcontrol_time_voltages_follow_the_monitored_elements_terminal() {
    let mut dss = compile_corpus_deck("controls/capcontrol/capcontrol_time.dss");
    // The deck ends at `Set mode=daily stepsize=1h number=1`; the live gate's
    // step 0 is the first `solve` after that, so this is the same state.
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let snaps = dss.snapshot_elements();

    // Both banks name `element=line.lf terminal=1 capacitor=c*`, so both must
    // sit on `line.lf`'s terminal 1 (bus `src`) and neither on the capacitors'
    // bus `b`. `line.lf` conductor 0 is that terminal's phase A.
    let line = elem(&snaps, "Line.lf");
    let cap = elem(&snaps, "Capacitor.cday");
    // Port (dss-rs, = EPRI r4133): the monitored terminal.
    const MONITORED_V: f64 = 7342.020904321447;
    // dss_capi 0.14.5's reading on the same element: the capacitor's bus.
    const CAPACITOR_V: f64 = 7276.216225426737;
    // `micro` tier (`population.lock.json`: `capcontrol/capcontrol_time.dss`
    // `kind=micro`) — `v_abs 1e-6`, `v_rel 1e-9`.
    let band = 1e-6 + 1e-9 * MONITORED_V;

    assert!(
        (line.voltages_mag_ang[0].mag - MONITORED_V).abs() <= band,
        "Line.lf terminal 1 phase A |V| = {} V; expected {MONITORED_V} V",
        line.voltages_mag_ang[0].mag,
    );
    assert!(
        (cap.voltages_mag_ang[0].mag - CAPACITOR_V).abs() <= band,
        "Capacitor.cday |V| = {} V; expected {CAPACITOR_V} V (the capi 0.14.5 reading)",
        cap.voltages_mag_ang[0].mag,
    );
    // 65.8 V apart: the two candidate buses are not numerically confusable, so
    // this pin cannot pass by accident.
    assert!(
        (MONITORED_V - CAPACITOR_V).abs() > 6.5e1,
        "the two buses must be far apart for the pin to discriminate",
    );

    for name in ["CapControl.cc1", "CapControl.cc2"] {
        let cc = elem(&snaps, name);
        assert_eq!(
            cc.bus_names.first().map(String::as_str),
            Some("src"),
            "{name} bus 1 must be the monitored element's terminal, not the capacitor's `b`",
        );
        assert_eq!(
            cc.voltages_mag_ang.len(),
            3,
            "{name} carries the monitored element's three phase conductors",
        );
        for (k, v) in cc.voltages_mag_ang.iter().enumerate() {
            let l = line.voltages_mag_ang[k];
            assert_eq!(
                (v.mag, v.ang),
                (l.mag, l.ang),
                "{name} conductor {k} must read Line.lf terminal 1 exactly",
            );
        }
        // The discriminating number, stated both ways.
        assert!(
            (cc.voltages_mag_ang[0].mag - MONITORED_V).abs() <= band,
            "{name} |V1| = {} V; r4133/port {MONITORED_V} V",
            cc.voltages_mag_ang[0].mag,
        );
        assert!(
            (cc.voltages_mag_ang[0].mag - CAPACITOR_V).abs() > 6.5e1,
            "{name} must NOT read the capacitor's bus ({CAPACITOR_V} V, capi 0.14.5)",
        );
    }
}

// Regression pin (G1.3a audit settlement, 2026-09-04) — the polar accessor must
// not panic on a stale `NodeRef`. `set_nterms`/`set_nconds` grow `Yorder` and
// reallocate the terminal buffers but leave `node_ref` alone
// (`elements/ckt.rs:326`, `:334-346`); only `set_node_ref` resizes it (`:382`),
// and `TDSSCircuit.ReProcessBusDefs` re-runs it for **enabled** elements only
// (`circuit/circuit.rs:735`, Pascal `Common/Circuit.pas:2380-2400`). So a
// disabled element that grows phases keeps a `node_ref` SHORTER than its
// `yorder`, and the first cut of the G1.3a block sliced `node_ref[..yorder]`
// there — a panic in the public `snapshot_elements` reachable from ordinary
// deck input (found by the G1.3a code audit; `range end index 6 out of range
// for slice of length 2`).
/// A disabled element whose `NodeRef` is shorter than its `Yorder` reports a
/// full-length `voltages_mag_ang` whose stale slots read as ground.
#[test]
fn a_stale_node_ref_shorter_than_yorder_reads_as_ground() {
    let mut dss = Dss::new();
    for cmd in [
        "clear",
        "new circuit.stale basekv=12.47 phases=3 bus1=src",
        "new line.a bus1=src.1 bus2=b.1 phases=1 r1=0.1 x1=0.1 length=1",
        "new load.l bus1=b.1 phases=1 kv=7.2 kw=10",
        "solve",
        // Disabled first, so `ReProcessBusDefs` no longer re-runs `SetNodeRef`
        // on it; then grown, so `Yorder` (2 -> 6) outruns `node_ref` (2).
        "edit line.a enabled=no",
        "edit line.a phases=3",
        "edit line.a bus1=src.1.2.3 bus2=b.1.2.3",
        "solve",
    ] {
        dss.command(cmd);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let snaps = dss.snapshot_elements();
    let a = elem(&snaps, "Line.a");
    assert!(!a.enabled, "Line.a stays disabled");
    assert_eq!(
        a.voltages_mag_ang.len(),
        6,
        "one entry per conductor slot (yorder), stale `NodeRef` or not",
    );
    // The two live slots keep whatever the stale mapping pointed at; the four
    // that have no `NodeRef` entry at all read the ground node, exactly like a
    // `NodeRef` of 0 (`NodeV[0]` is the always-zero ground slot).
    for (k, v) in a.voltages_mag_ang.iter().enumerate().skip(2) {
        assert_eq!(
            (v.mag, v.ang),
            (0.0, 0.0),
            "conductor slot {k} has no `NodeRef` and must read ground",
        );
    }
}
