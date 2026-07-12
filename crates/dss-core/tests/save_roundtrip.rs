//! `Save circuit` round-trip + structural gate (PHASE8_PLAN §WP8.5 step 5, §1
//! gate #3). Faithfulness of `Circuit.Save` is **round-trip**, not byte-equality
//! with the oracle's Save output (§2.4): we save a solved circuit to a scratch
//! directory, `clear`, re-`compile` the emitted `Master.dss` on OUR engine, and
//! re-`solve` — the node voltages must match the pre-save solution (≤1e-6 rel)
//! and the iteration count must be identical. A structural test additionally
//! pins the emitted **file set** against the oracle's probe-proven set (captured
//! with the pinned dss-python `Circuit.Save`, 2026-07-07).

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
    assert!(!pre.is_empty(), "{tag}: no nodes pre-save");

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
