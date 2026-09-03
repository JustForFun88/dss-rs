//! `Save circuit` round-trip + structural gate (PHASE8_PLAN §WP8.5 step 5, §1
//! gate #3). Faithfulness of `Circuit.Save` is **round-trip**, not byte-equality
//! with the oracle's Save output (§2.4): we save a solved circuit to a scratch
//! directory, `clear`, re-`compile` the emitted `Master.dss` on OUR engine, and
//! re-`solve` — the node voltages must match the pre-save solution (≤1e-6 rel),
//! the iteration count must be identical, the discrete control state must match
//! exactly, and the assembled **checkpoint Y** must match entry for entry
//! ([`Y_TOL`]). A structural test additionally pins the emitted **file set**
//! against the oracle's probe-proven set (captured with the pinned dss-python
//! `Circuit.Save`, 2026-07-07).
//!
//! **Stage F.4c** added the checkpoint-Y half, which is `DE_PASCALIZE_PLAN.md`
//! §F-FMT's sequencing guard made executable: "`Save` output in the default lane
//! must stay re-compilable by our own parser (add a round-trip test: Save →
//! Compile → same checkpoint Y)". Every number in the emitted script is printed
//! through the F-FMT seam, so this is the test that says a rendering change may
//! not alter the model the deck rebuilds — and it says it on the admittance
//! matrix, which a power flow cannot absorb.

use dss_core::exec::Dss;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// Repo-root-relative path under the vendored corpus.
fn corpus(rel: &str) -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(rel)
}

/// A repo-root-relative path (for the `tools/golden/report_decks` fixtures).
fn repo(rel: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."]
        .iter()
        .collect::<PathBuf>()
        .join(rel)
}

/// A unique scratch dir for this test process (no `tempfile` dep).
fn scratch_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("dss_saveroundtrip_{tag}_{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
    d
}

/// The assembled system **Y** keyed by `(row node name, col node name)`, with
/// duplicate coordinate stamps summed — the checkpoint the golden suite calls
/// "checkpoint Y", captured through the same `Dss::system_y_csc` API.
///
/// Keyed by node *name* for the same reason [`snapshot`] is: a re-compile of the
/// emitted deck may number the buses differently, and the claim under test is
/// about the electrical model, not about the ordering.
type YCheckpoint = std::collections::BTreeMap<(String, String), (f64, f64)>;

fn y_checkpoint(dss: &mut Dss) -> YCheckpoint {
    let (n, coords) = dss.system_y_csc().expect("a solved circuit has a system Y");
    assert!(n > 0, "empty system Y");
    let ckt = dss.circuit().expect("solved circuit");
    let mut out: YCheckpoint = std::collections::BTreeMap::new();
    for (r, c, v) in coords {
        let key = (ckt.node_name(r + 1), ckt.node_name(c + 1));
        let e = out.entry(key).or_insert((0.0, 0.0));
        e.0 += v.re;
        e.1 += v.im;
    }
    out
}

/// Relative floor for the [`y_checkpoint`] comparison across a `Save` round
/// trip.
///
/// This is **not** a physical tolerance: `Save` re-emits every property as text
/// through the F-FMT seam at 15 significant digits (`util::float_to_str`), so a
/// re-compiled property is the nearest `f64` to a 15-digit decimal — up to
/// ~5e-16 relative from the original — and the element `YPrim`s assembled from
/// it inherit that. The floor is therefore a *rendering* floor, and it is the
/// exact quantity §F-FMT's sequencing guard asks this test to bound: whatever
/// F-FMT does to how numbers are printed, the saved deck must still rebuild the
/// same admittance model.
///
/// **Measured**, not guessed (each run re-prints its own figure, see the
/// `eprintln!` at the comparison): worst relative entry difference is
/// **2.875e-15** on `IEEE-8500` (46 259 entries) and ≤ **1.9e-16** on the other
/// six feeders — i.e. one to a few ulp, which is exactly what a 15-significant
/// decimal round trip costs. The floor is set a factor ~3.5 above the measured
/// worst; it tightens if the rendering ever gets more faithful, and a value that
/// needs it *widened* is a Save bug, not a floor to raise.
const Y_TOL: f64 = 1e-14;

/// Snapshot the solved state as (node-name → complex V) plus the iteration
/// count. Keyed by node NAME so a re-compile that renumbers buses still lines up.
fn snapshot(dss: &Dss) -> (Vec<(String, f64, f64)>, i32) {
    let ckt = dss.circuit().expect("circuit solved");
    let mut v = Vec::with_capacity(ckt.num_nodes);
    for j in 1..=ckt.num_nodes {
        let volt = ckt.solution.node_v[j];
        v.push((ckt.node_name(j), volt.re, volt.im));
    }
    (v, ckt.solution.iteration)
}

/// The discrete control state — every RegControl tap number + every Capacitor's
/// per-step on/off vector, sorted by name. This must round-trip **exactly** on
/// every deck (a discrete decision, no float floor), so it is asserted equal
/// pre/post for all feeders. It is what keeps the IEEE-8500 case a real gate
/// despite that deck's inherent continuous-voltage Save floor (see
/// `save_roundtrip_ieee8500`): a regulator/cap regression shifts a tap or a bank
/// and fails here even when the loosened node-V band would not catch it.
type DiscreteState = (
    std::collections::BTreeMap<String, i32>,
    std::collections::BTreeMap<String, Vec<i32>>,
);
fn discrete_state(dss: &Dss) -> DiscreteState {
    (
        dss.regcontrol_tap_numbers().into_iter().collect(),
        dss.capacitor_states().into_iter().collect(),
    )
}

/// Every bus's `kVBase`, keyed by bus name. RP3.11 settlement (audit finding
/// T5): the node-voltage / Y / discrete-state compares are all in **absolute**
/// volts, so none of them can see a base-kV regression — which is exactly how
/// 0.14.5's `! CalcVoltageBases` comment survived in the emitted
/// `BusVoltageBases.dss` behind the (retracted) claim that the base list "only
/// affects per-unit reporting". It does not: per-unit-driven controls read these
/// bases, and with the line commented out every bus of a re-compiled tree came
/// back at `kVBase = 0` (RP3.11 P1, `tmp/rp311/out_bisect.txt` §B: the
/// `expcontrol` PV moved -0.0060 kvar / 14 iterations to +307.94 kvar / 53).
/// Compared **exactly**: `CalcVoltageBases` re-derives each base from the same
/// `Set VoltageBases=(…)` list by the same rule, so a round trip that differs at
/// all is a Save defect, not a float floor.
fn bus_kv_bases(dss: &Dss) -> std::collections::BTreeMap<String, f64> {
    let ckt = dss.circuit().expect("circuit solved");
    (0..ckt.buses.len())
        .map(|i| {
            let name = ckt.bus_list.name(i).unwrap_or("?").to_string();
            (name, ckt.buses[i].kv_base)
        })
        .collect()
}

/// Solve `master`, `save circuit` to a scratch dir, `clear`, re-compile the
/// emitted `Master.dss`, re-solve, and assert node voltages (≤1e-6 rel) +
/// iteration count match the pre-save solution.
///
/// The IEEE masters embed a `Solve` (e.g. IEEE13 l.149), so a circuit reaches
/// this test already converged — the regulator taps are settled. To compare
/// iteration counts *fairly* (a cold solve of a regulator circuit spends extra
/// power-flow iterations settling taps that a warm re-solve does not), we
/// compare a **warm** re-solve on both sides: the pre-save snapshot is a warm
/// re-solve of the already-converged original, and the round-tripped circuit is
/// first solved cold (to settle taps to the same fixpoint) then warm-re-solved.
/// The node voltages must match after both reach convergence; the warm-re-solve
/// iteration count must be identical (proving the recompiled circuit converges
/// to the same operating point in the same way — a structural-identity check).
fn round_trip(tag: &str, master: PathBuf) {
    round_trip_full(tag, master, &[], &[], 1e-6);
}

/// General round-trip driver. `pre_only` are commands applied **once**, right
/// after the initial compile — element-creating setup (e.g. `New Energymeter…`)
/// that gets serialized into the saved deck, so it must NOT be replayed after the
/// re-compile (the emitted tree already carries it). `both` are option commands
/// (`Set Maxiterations=…`) that `Save` does NOT persist, so they are re-applied
/// on both the pre-save and post-recompile solves to reach the same fixpoint.
/// `vtol` is the node-voltage relative tolerance (1e-6 for the clean-round-trip
/// feeders; a proven Save-precision floor for IEEE-8500, see that test). Discrete
/// control state (reg taps + cap banks) is always compared **exactly**.
fn round_trip_full(tag: &str, master: PathBuf, pre_only: &[&str], both: &[&str], vtol: f64) {
    assert!(master.is_file(), "missing master: {}", master.display());
    let out = scratch_dir(tag);

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    for c in pre_only {
        dss.command(c);
    }
    for c in both {
        dss.command(c);
    }
    // Settle then warm re-solve: some masters embed `Solve` (IEEE13/37), some do
    // not (IEEE123/34/8500), so solve twice to guarantee a warm re-solve on both
    // sides.
    dss.command("solve");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "{tag}: pre-save errors: {:?}",
        dss.errors()
    );
    let (pre, pre_iter) = snapshot(&dss);
    let pre_discrete = discrete_state(&dss);
    let pre_bases = bus_kv_bases(&dss);
    let pre_y = y_checkpoint(&mut dss);
    assert!(!pre.is_empty(), "{tag}: no nodes pre-save");
    assert!(
        pre_bases.values().any(|&b| b > 0.0),
        "{tag}: no bus carries a kVBase pre-save — the compare below would be vacuous"
    );
    assert!(!pre_y.is_empty(), "{tag}: empty checkpoint Y pre-save");

    dss.command(&format!(
        "save circuit dir=\"{}\"",
        out.to_string_lossy().replace('\\', "/")
    ));
    assert!(
        dss.errors().is_empty(),
        "{tag}: save errors: {:?}",
        dss.errors()
    );
    let emitted_master = out.join("Master.dss");
    assert!(emitted_master.is_file(), "{tag}: no Master.dss emitted");

    // Re-compile the emitted script on OUR engine (no embedded Solve → cold).
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        emitted_master.to_string_lossy().replace('\\', "/")
    ));
    // Re-apply only the non-persisted option commands (element-creating `pre_only`
    // setup is already in the emitted deck — replaying it would duplicate).
    for c in both {
        dss.command(c);
    }
    // Cold solve settles regulator taps to the same fixpoint, then a warm
    // re-solve gives the count comparable to the pre-save warm re-solve.
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "{tag}: post-save cold-solve errors: {:?}",
        dss.errors()
    );
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "{tag}: post-save warm-solve errors: {:?}",
        dss.errors()
    );
    let (post, post_iter) = snapshot(&dss);
    let post_discrete = discrete_state(&dss);
    let post_bases = bus_kv_bases(&dss);
    let post_y = y_checkpoint(&mut dss);

    // Base kV, exactly (see `bus_kv_bases`): the one quantity of the emitted
    // deck that every other compare here is blind to.
    assert_eq!(
        pre_bases.len(),
        post_bases.len(),
        "{tag}: bus count changed across save round-trip"
    );
    for (name, base) in &pre_bases {
        let got = post_bases
            .get(name)
            .unwrap_or_else(|| panic!("{tag}: bus {name:?} missing after round-trip"));
        assert_eq!(
            got, base,
            "{tag}: bus {name:?} kVBase changed across save round-trip \
             ({base} -> {got}); 0.0 is the `! CalcVoltageBases` symptom"
        );
    }

    // Checkpoint Y (`DE_PASCALIZE_PLAN.md` §F-FMT sequencing guard: "Save output
    // in the default lane must stay re-compilable by our own parser — Save →
    // Compile → same checkpoint Y"). The strongest statement of that guard: the
    // *assembled admittance model* of the re-compiled deck, entry for entry,
    // keyed by node name. It is stricter than the node-voltage compare it sits
    // next to — a wrong impedance that the power flow happens to absorb still
    // shows here — and it is exactly what a rendering change could break, since
    // every number in the emitted script is printed through the F-FMT seam.
    assert_eq!(
        pre_y.len(),
        post_y.len(),
        "{tag}: checkpoint Y entry count changed across save round-trip ({} -> {})",
        pre_y.len(),
        post_y.len()
    );
    let mut worst = (0.0f64, String::new());
    for (key, &(pre_re, pre_im)) in &pre_y {
        let &(post_re, post_im) = post_y.get(key).unwrap_or_else(|| {
            panic!("{tag}: checkpoint Y entry {key:?} missing after round-trip")
        });
        let mag = pre_re.hypot(pre_im);
        let d = (post_re - pre_re).hypot(post_im - pre_im);
        let rel = if mag > 0.0 { d / mag } else { d };
        if rel > worst.0 {
            worst = (
                rel,
                format!("{key:?} pre=({pre_re:e},{pre_im:e}) post=({post_re:e},{post_im:e})"),
            );
        }
    }
    assert!(
        worst.0 <= Y_TOL,
        "{tag}: checkpoint Y diverged across save round-trip: rel={:.3e} > {Y_TOL:.1e} at {}",
        worst.0,
        worst.1
    );
    // The measured headroom, printed rather than only asserted: `Y_TOL` is a
    // rendering floor, and the number that justifies it should be visible in the
    // log of every run instead of living only in a comment.
    eprintln!(
        "{tag}: checkpoint Y worst rel across save round-trip = {:.3e} ({} entries)",
        worst.0,
        pre_y.len()
    );

    // Iteration count exact (warm re-solve on both sides).
    assert_eq!(
        pre_iter, post_iter,
        "{tag}: warm-re-solve iteration count changed across save round-trip ({pre_iter} -> {post_iter})"
    );

    // Discrete control state exact (reg tap numbers + capacitor bank states) —
    // no float floor on a discrete decision, so this is checked at exact equality
    // on every deck including IEEE-8500.
    assert_eq!(
        pre_discrete.0, post_discrete.0,
        "{tag}: RegControl tap numbers changed across save round-trip"
    );
    assert_eq!(
        pre_discrete.1, post_discrete.1,
        "{tag}: Capacitor bank states changed across save round-trip"
    );

    // Node voltages within `vtol` rel (matched by node name).
    use std::collections::HashMap;
    let post_map: HashMap<&str, (f64, f64)> = post
        .iter()
        .map(|(n, re, im)| (n.as_str(), (*re, *im)))
        .collect();
    assert_eq!(
        pre.len(),
        post.len(),
        "{tag}: node count changed {} -> {}",
        pre.len(),
        post.len()
    );
    for (name, re, im) in &pre {
        let (pre_re, pre_im) = (*re, *im);
        let &(post_re, post_im) = post_map
            .get(name.as_str())
            .unwrap_or_else(|| panic!("{tag}: node {name} missing after round-trip"));
        let mag = (pre_re * pre_re + pre_im * pre_im).sqrt();
        let d = ((post_re - pre_re).powi(2) + (post_im - pre_im).powi(2)).sqrt();
        let rel = if mag > 0.0 { d / mag } else { d };
        assert!(
            rel <= vtol,
            "{tag}: node {name} voltage diverged rel={rel:.3e} (>{vtol:.1e}) \
             (pre={pre_re:.6}+j{pre_im:.6}, post={post_re:.6}+j{post_im:.6})"
        );
    }

    std::fs::remove_dir_all(&out).ok();
}

#[test]
fn save_roundtrip_ieee13() {
    round_trip(
        "ieee13",
        corpus("Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss"),
    );
}

#[test]
fn save_roundtrip_ieee37() {
    round_trip(
        "ieee37",
        corpus("Version8/Distrib/IEEETestCases/37Bus/ieee37.dss"),
    );
}

#[test]
fn save_roundtrip_ieee123() {
    round_trip(
        "ieee123",
        corpus("Version8/Distrib/IEEETestCases/123Bus/IEEE123Master.dss"),
    );
}

/// IEEE 34-bus (PORTING_PLAN §6 literal acceptance). `ieee34Mod1.dss` is a
/// `not_an_entry_point` fragment (its `Run_IEEE34Mod1.dss` driver adds the meter
/// then solves), so we compile it and attach `Energymeter.M1` on `Line.L1` (the
/// same meter the canonical run uses) before solving. That meter is `pre_only`,
/// so it serializes into the emitted `EnergyMeter.dss` and the round-trip also
/// exercises the meter-zone save/re-parse path. Six RegControls settle taps on the
/// cold solve; the warm-re-solve iteration count and node V (≤1e-6 rel) must
/// round-trip.
#[test]
fn save_roundtrip_ieee34() {
    round_trip_full(
        "ieee34",
        corpus("Version8/Distrib/IEEETestCases/34Bus/ieee34Mod1.dss"),
        &["New Energymeter.M1 Line.L1 1"],
        &[],
        1e-6,
    );
}

/// The IEEE-8500 node-voltage round-trip floor, **proven inherent to
/// `Save circuit` by the oracle** (see below) — NOT a port slack.
const IEEE8500_SAVE_VTOL: f64 = 3e-4;

/// IEEE 8500-Node (PORTING_PLAN §6 literal acceptance; 8531 nodes / ~6100
/// devices). The unmodified `Master.dss` embeds no `Solve` and needs
/// `Maxiterations=20` to converge (as `Run_8500Node.dss` sets); that option is
/// NOT persisted by `Save`, so it is applied on both the pre-save and
/// post-recompile solves (`both`).
///
/// Unlike the four clean-round-trip feeders above, IEEE-8500's node voltages do
/// **not** round-trip to 1e-6 — and this is an inherent property of OpenDSS
/// `Save circuit`, not the port. `Save` re-emits derived quantities (e.g. the
/// substation source reactor `X=(1.051 0.88 0.001 3 * - - 115 12.47 / sqr *)`,
/// and every `%g`-rendered parameter) at 15 significant digits; the sub-ulp
/// re-parse perturbation is then **amplified** by the ~thousand service loads
/// operating in their `Model=1`/`Vminpu=0.88` constant-Z region into a
/// worst-case ~2.02e-4 rel shift at the deepest 0.208 kV secondary nodes. The
/// pinned dss-python oracle (0.15.7) reproduces this **bit-for-bit**: its own
/// `Save`→recompile→resolve of this deck yields the identical worst node
/// (`SX3312692A.1`, 2.022253e-4) and the identical pre/post total power
/// (−11983.486783 → −11983.420712 kW) that our engine produces — so the port is
/// faithful and 1e-6 is simply unachievable here for either engine. Reproducible
/// probe: `python tools/golden/probe_save_roundtrip_8500.py` (pinned oracle,
/// 2026-07-11); numbers recorded in `tests/TOLERANCE_NOTES.md`
/// §"`Save circuit` round-trip floor — IEEE-8500".
///
/// The gate stays strong despite the loosened band: iteration count is exact,
/// and the **discrete** control state — all 12 RegControl tap numbers + all 10
/// capacitor bank states — is asserted **exactly** (in `round_trip_full`). A real
/// regulator/cap/element regression moves a tap, a bank, or the whole profile by
/// far more than 3e-4 and fails; the 3e-4 band only absorbs the proven,
/// deterministic Save-precision floor (~1.5× over the observed 2.02e-4).
///
/// Runs in the default suite: the four solves + save + recompile measure ~0.34 s
/// under the test profile's opt-3 engine (measured 2026-07-11) — no expensive gate.
#[test]
fn save_roundtrip_ieee8500() {
    round_trip_full(
        "ieee8500",
        corpus("Version8/Distrib/IEEETestCases/8500-Node/Master.dss"),
        &[],
        &["Set Maxiterations=20"],
        IEEE8500_SAVE_VTOL,
    );
}

/// `LineGeometry` conductor-table round-trip (the WP-AD save-fidelity fix). The
/// generic `SaveWrite` collapses `Cond`/`Wire`/`X`/`H`/`Units` — each re-set once
/// per conductor — to a single property-sequence slot, so it used to emit only
/// the LAST conductor; the missing per-conductor arrays made a geometry-built
/// line reload with a wrong (or unbuildable) `Z` matrix. The
/// [`crate::elements::general::line_geometry`] `SaveWrite` override (Pascal
/// `TLineGeometryObj.SaveWrite`) re-emits the whole conductor table.
///
/// The IEEE-13 geometry deck is the witness: five geometries with mixed conductor
/// counts (4/4/3/3/2), a `like=`-cloned geometry (`604 like=603`, which the
/// override must serialize by its own conductor table — our port cannot
/// round-trip a `like=` directive), and a phase/neutral wire mix. A single
/// dropped conductor changes the reduced series `Z` by far more than the 1e-6
/// node-V tier (measured ~1.0 before the fix — the interconnected reload failed
/// to solve), so the round-trip node-V / discrete-tap comparison pins the whole
/// conductor table. Regulators settle on the cold re-solve (as in
/// `save_roundtrip_ieee13`), and a dropped conductor is a large error the tap
/// channel cannot mask.
#[test]
fn save_roundtrip_ieee13_linegeometry() {
    round_trip("ie13geom", corpus("Test/IEEE13_LineGeometry.dss"));
}

/// `Line` inline-spacing conductor-array round-trip (the WP-AD save-fidelity
/// fix, second bug). `Line.Wires`/`CNCables`/`TSCables` alias the one
/// `LineWireData` array, so the generic `SaveWrite` re-emits the *whole* array
/// under *every* kind that was set: a line built `TSCables=[TS_1/0]
/// Wires=[CU_1/0]` used to save as `TSCables=[ts_1/0, cu_1/0] Wires=[ts_1/0,
/// cu_1/0]`, and reload aborted looking up the `cu_1/0` **wire** in the TSData
/// catalog. The [`crate::elements::pd::line`] `SaveWrite` override (Pascal
/// `TLineObj.SaveWrite`) emits contiguous same-catalog runs under the correct
/// kind.
///
/// The IEEE-13 line/cable-spacing deck is the witness: overhead `Wires=` lines,
/// a 3-phase `CNCables=` cable run (`Line.692675`), and the mixed
/// `TSCables=[TS_1/0] Wires=[CU_1/0]` line (`Line.684652`) that reproduced the
/// reload abort before the fix (proven: the pre-fix serializer errors
/// `TSData "cu_1/0" not found`, which `errors().is_empty()` catches). The
/// round-trip node-V / discrete-tap comparison pins the partitioned emission.
#[test]
fn save_roundtrip_ieee13_lineandcablespacing() {
    round_trip("ie13lcs", corpus("Test/IEEE13_LineAndCableSpacing.dss"));
}

/// `Save circuit` round-trip over a **protection-heavy** deck (WP-U2.5): a
/// Relay + Recloser + Fuse + SwtControl, each carrying r4133-surface property
/// values — renamed props (`PhCurve`/`PhFastCurve`/`CurveMultiplier`), the new
/// r4133 props (`SinglePhTrip`/`SinglePhLockout`/`Lock`/`RatedCurrent`/
/// `InterruptingRating`), and the per-phase `Normal`/`State` arrays that render
/// `[open, closed, closed, ]`. `Save` must serialize this surface such that OUR
/// own parser re-reads the identical model (the WP8.5 round-trip convention).
///
/// The gate: snapshot **every** property of each control via `element_properties`
/// (the byte-proven `?` surface), `save circuit`, `clear`, re-compile the emitted
/// tree on our engine, and assert each control's full property list is unchanged
/// (numeric-token compare — pins the renamed/new-prop names, values, and array
/// renders through the Save→re-parse boundary). Node voltages + the discrete
/// control decision are also compared. The r4133 property VALUES themselves are
/// cross-checked against oddie:r4133 by the `oracle:"r4133"` controls decks +
/// `props/*.json`; here we pin that `Save` does not drop or corrupt the r4133
/// surface on the way out and back.
#[test]
fn save_roundtrip_protection() {
    let out = scratch_dir("protection");
    // A source → main line with Relay/Recloser/Fuse/SwtControl guarding branches.
    // Pickups are set well above the small steady load currents so nothing trips on
    // current (the round-trip pins Save *serialization* of the r4133 surface, not
    // protection action). The relay additionally carries manually-forced mixed
    // per-phase `Normal`/`State` arrays so the round-trip covers Save of a non-
    // default `[open, closed, closed, ]` array (not just the all-closed default).
    // Built-in `tlink`/`A`/`D` TCC curves + one explicit phase curve.
    let deck: &[&str] = &[
        "clear",
        "new circuit.prot basekv=12.47 bus1=src phases=3",
        "~ r1=0.1 x1=0.4 r0=0.3 x0=1.2",
        "new linecode.lc nphases=3 r1=0.3 x1=0.6 r0=0.9 x0=1.8 c1=3 c0=1.5 units=km",
        "new line.main bus1=src bus2=b1 linecode=lc length=1 units=km",
        "new line.br1 bus1=b1 bus2=b2 linecode=lc length=0.5 units=km",
        "new line.br2 bus1=b1 bus2=b3 linecode=lc length=0.5 units=km",
        "new line.br3 bus1=b1 bus2=b4 linecode=lc length=0.5 units=km",
        "new line.tie bus1=b1 bus2=b5 linecode=lc length=0.2 units=km",
        "new tcc_curve.ph npts=4 c_array=[1 2 4 6] t_array=[10 2 0.5 0.1]",
        "new load.l2 bus1=b2 phases=3 kv=12.47 kw=20 pf=0.95",
        "new load.l3 bus1=b3 phases=3 kv=12.47 kw=15 pf=0.9",
        "new load.l4 bus1=b4 phases=3 kv=12.47 kw=10 pf=0.92",
        "new load.l5 bus1=b5 phases=3 kv=12.47 kw=5 pf=0.95",
        // Relay (overcurrent) with renamed + new r4133 props. High pickups so it
        // never trips on the steady load current. `Normal`/`State` carry DISTINCT
        // mixed per-phase arrays — `State=(open closed closed)` manually forces
        // phase 1 of the controlled br1 open (a locked-out actual state, as after a
        // single-phase trip; explicit Normal first sets NormalStateSet so the State
        // write does not copy it) — so the round-trip exercises Save serialization
        // and re-parse of a mixed `[open, closed, closed, ]` array on BOTH the
        // Normal and the (physically-applied) State surface, not just the
        // all-closed default (audit WP-U2.5, finding-4 coverage gap).
        "new relay.rel monitoredobj=line.br1 monitoredterm=1 type=current phcurve=ph \
         oc_gndcurve=ph phpickup=5000 oc_gndpickup=5000 tdph=1.5 oc_tdgnd=1.1 \
         definitetimedelay=0.1 mechanicaldelay=0.05 singlephtrip=yes singlephlockout=yes \
         ratedcurrent=600 interruptingrating=10000 normal=(closed open closed) \
         state=(open closed closed)",
        // Recloser with fast/slow curves + pickups + new props.
        "new recloser.rec monitoredobj=line.br2 monitoredterm=1 phfastcurve=A phslowcurve=D \
         phfastpickup=5000 phslowpickup=5000 numfast=2 shots=3 singlephtrip=yes \
         ratedcurrent=400 interruptingrating=8000",
        // Fuse with r4133 curvemultiplier divisor + informational ratings.
        "new fuse.fus monitoredobj=line.br3 monitoredterm=1 fusecurve=tlink \
         curvemultiplier=2 ratedcurrent=65 interruptingrating=5000 delay=0.02",
        // Tie switch (SwtControl) with the r4133 informational rating.
        "new swtcontrol.sw switchedobj=line.tie switchedterm=1 ratedcurrent=300 normal=closed",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
        "solve",
    ];
    let controls = ["relay.rel", "recloser.rec", "fuse.fus", "swtcontrol.sw"];

    let mut dss = Dss::new();
    for c in deck {
        dss.command(c);
    }
    assert!(
        dss.errors().is_empty(),
        "protection deck pre-save errors: {:?}",
        dss.errors()
    );
    let (pre, pre_iter) = snapshot(&dss);
    assert!(!pre.is_empty(), "no nodes pre-save");
    let pre_props: Vec<(String, Vec<(String, String)>)> = controls
        .iter()
        .map(|el| {
            (
                el.to_string(),
                dss.element_properties(el)
                    .unwrap_or_else(|| panic!("{el} not found pre-save")),
            )
        })
        .collect();

    dss.command(&format!(
        "save circuit dir=\"{}\"",
        out.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "save errors: {:?}", dss.errors());
    let emitted_master = out.join("Master.dss");
    assert!(emitted_master.is_file(), "no Master.dss emitted");

    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        emitted_master.to_string_lossy().replace('\\', "/")
    ));
    assert!(
        dss.errors().is_empty(),
        "re-compile of emitted protection tree errored: {:?}",
        dss.errors()
    );
    dss.command("solve");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "post-save solve errors: {:?}",
        dss.errors()
    );
    let (post, post_iter) = snapshot(&dss);

    // Every control's full r4133 property surface must round-trip through our own
    // parser (renamed/new prop names + values + `[..]` array renders).
    for (el, pre_list) in &pre_props {
        let post_list = dss
            .element_properties(el)
            .unwrap_or_else(|| panic!("{el} missing after round-trip"));
        assert_eq!(
            pre_list.len(),
            post_list.len(),
            "{el}: property count changed across save round-trip ({} -> {})",
            pre_list.len(),
            post_list.len()
        );
        for ((pn, pv), (qn, qv)) in pre_list.iter().zip(&post_list) {
            assert_eq!(pn, qn, "{el}: property name order changed ({pn} vs {qn})");
            assert_prop_token_eq(pv, qv, &format!("{el}.{pn}"));
        }
    }

    // Iteration count + node voltages round-trip (the physics is unaffected by the
    // controls at snapshot, but a corrupted re-parse would perturb both).
    assert_eq!(
        pre_iter, post_iter,
        "warm re-solve iteration count changed across protection save round-trip"
    );
    use std::collections::HashMap;
    let post_map: HashMap<&str, (f64, f64)> = post
        .iter()
        .map(|(n, re, im)| (n.as_str(), (*re, *im)))
        .collect();
    assert_eq!(pre.len(), post.len(), "node count changed");
    for (name, re, im) in &pre {
        let &(qre, qim) = post_map
            .get(name.as_str())
            .unwrap_or_else(|| panic!("node {name} missing after round-trip"));
        let mag = (re * re + im * im).sqrt().max(1e-9);
        let d = ((qre - re).powi(2) + (qim - im).powi(2)).sqrt();
        assert!(
            d / mag <= 1e-6,
            "node {name}: |dV|/|V|={:.3e} > 1e-6 across protection save round-trip",
            d / mag
        );
    }

    std::fs::remove_dir_all(&out).ok();
}

/// Numeric-token equality (WP8.1 skeleton compare): non-numeric structure exact,
/// numbers within a tight relative floor. A Save→re-parse must reproduce a
/// property string exactly modulo last-digit `%g` rendering.
fn assert_prop_token_eq(a: &str, b: &str, ctx: &str) {
    fn split(s: &str) -> (String, Vec<f64>) {
        let mut skel = String::new();
        let mut nums = Vec::new();
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i];
            let starts_num = c.is_ascii_digit()
                || ((c == b'-' || c == b'+' || c == b'.')
                    && i + 1 < bytes.len()
                    && bytes[i + 1].is_ascii_digit());
            if starts_num {
                let mut end = i;
                while end < bytes.len()
                    && (bytes[end].is_ascii_digit()
                        || matches!(bytes[end], b'.' | b'+' | b'-' | b'e' | b'E'))
                {
                    end += 1;
                }
                let mut e = end;
                while e > i {
                    if let Ok(v) = s[i..e].parse::<f64>() {
                        nums.push(v);
                        skel.push('#');
                        i = e;
                        break;
                    }
                    e -= 1;
                }
                if e == i {
                    skel.push(c as char);
                    i += 1;
                }
            } else {
                skel.push(c as char);
                i += 1;
            }
        }
        (skel, nums)
    }
    let (sa, na) = split(a);
    let (sb, nb) = split(b);
    assert_eq!(sa, sb, "{ctx}: structure differs ({a:?} vs {b:?})");
    assert_eq!(
        na.len(),
        nb.len(),
        "{ctx}: number count differs ({a:?} vs {b:?})"
    );
    for (x, y) in na.iter().zip(&nb) {
        assert!(
            (x - y).abs() <= 1e-9 + 1e-9 * y.abs(),
            "{ctx}: number differs ({x} vs {y}) in {a:?} vs {b:?}"
        );
    }
}

/// Collect the set of emitted files relative to `root`, using `/` separators.
fn emitted_set(root: &std::path::Path) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    fn walk(dir: &std::path::Path, root: &std::path::Path, set: &mut BTreeSet<String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, root, set);
            } else {
                let rel = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                set.insert(rel);
            }
        }
    }
    walk(root, root, &mut set);
    set
}

/// Structural test: `save circuit` on `save_forms.dss` (a deck with an
/// EnergyMeter zone → feeder subdir) emits exactly the oracle's probe-proven
/// file set (dss-python `Circuit.Save`, 2026-07-07). Round-trip through our own
/// parser is covered by the IEEE cases above; here we pin the multi-file layout,
/// including the `em1/` meter-zone subdirectory (`Branches`/`Loads`/`Capacitors`
/// with the empty `Transformers`/`Shunts`/`Generators` deleted, not listed).
#[test]
fn save_forms_structural_file_set() {
    let deck = repo("tools/golden/report_decks/save_forms.dss");
    assert!(deck.is_file(), "missing fixture: {}", deck.display());
    let out = scratch_dir("saveforms");

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        deck.to_string_lossy().replace('\\', "/")
    ));
    assert!(
        dss.errors().is_empty(),
        "compile errors: {:?}",
        dss.errors()
    );
    let nodes_before = dss.circuit().expect("circuit").num_nodes;
    // Numeric round-trip for the SaveZone/feeder path (this is the only gate
    // deck with a meter zone): a snapshot-mode warm solve before the save is
    // compared against the same solve of the emitted tree below — a
    // zone-serialization bug that still parses (wrong load kW/pf, wrong cap
    // kvar, a mis-derived control) shows up here as a voltage diff even
    // though the file set and node count stay right.
    dss.command("set mode=snap");
    dss.command("solve");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "pre-save snap-solve errors: {:?}",
        dss.errors()
    );
    let (pre, pre_iter) = snapshot(&dss);
    dss.command(&format!(
        "save circuit dir=\"{}\"",
        out.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "save errors: {:?}", dss.errors());

    let got = emitted_set(&out);
    let expected: BTreeSet<String> = [
        "BusCoords.dss",
        "BusVoltageBases.dss",
        "EnergyMeter.dss",
        "GrowthShape.dss",
        "LineCode.dss",
        "LoadShape.dss",
        "Master.dss",
        "Monitor.dss",
        "Spectrum.dss",
        "TCC_Curve.dss",
        "Vsource.dss",
        "em1/Branches.dss",
        "em1/Capacitors.dss",
        "em1/Loads.dss",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    assert_eq!(got, expected, "emitted file set differs from oracle probe");

    // The emitted tree must re-compile cleanly on our own parser — this is the
    // only gate deck with a meter zone, so it exercises the `em1\…` feeder
    // Redirects + the zone-file (Branches/Loads/Capacitors) re-parse. (A voltage
    // round-trip is out of scope: the emitted Master carries no `set mode=daily`,
    // so it re-compiles in snapshot mode, not the fixture's daily final state.)
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        out.join("Master.dss").to_string_lossy().replace('\\', "/")
    ));
    assert!(
        dss.errors().is_empty(),
        "re-compile of emitted feeder tree errored: {:?}",
        dss.errors()
    );
    // Same node count proves every element (lines/loads/cap in the feeder
    // subdir, source, meter/monitor) re-parsed from the emitted tree.
    let nodes_after = dss.circuit().expect("circuit after re-compile").num_nodes;
    assert_eq!(
        nodes_before, nodes_after,
        "node count changed across feeder-tree round-trip"
    );

    // Snapshot voltage round-trip (the emitted Master carries no solve mode →
    // snapshot by default; warm solve on both sides).
    dss.command("solve");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "post-save snap-solve errors: {:?}",
        dss.errors()
    );
    let (post, post_iter) = snapshot(&dss);
    assert_eq!(
        pre_iter, post_iter,
        "warm snap-solve iteration count changed"
    );
    let post_map: std::collections::HashMap<&str, (f64, f64)> = post
        .iter()
        .map(|(n, re, im)| (n.as_str(), (*re, *im)))
        .collect();
    assert_eq!(pre.len(), post.len(), "node set size changed");
    for (name, re, im) in &pre {
        let (pre_re, pre_im) = (*re, *im);
        let Some(&(post_re, post_im)) = post_map.get(name.as_str()) else {
            panic!("node {name} missing after feeder-tree round-trip");
        };
        let mag = (pre_re * pre_re + pre_im * pre_im).sqrt().max(1e-9);
        let d = ((pre_re - post_re).powi(2) + (pre_im - post_im).powi(2)).sqrt();
        assert!(
            d / mag <= 1e-6,
            "node {name}: |dV|/|V| = {:.3e} > 1e-6 across feeder-tree round-trip",
            d / mag
        );
    }

    std::fs::remove_dir_all(&out).ok();
}
