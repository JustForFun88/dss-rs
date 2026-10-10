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
//!
//! Every object's full property list (the `?` surface) is compared as well
//! ([`property_snapshot`], [`assert_properties_round_trip`]): a property lost in
//! the emitted deck — a `Ratings` array, a geometry's active conductor, a
//! transformer's active winding — moves no solve observable. The reloaded side
//! is rendered
//! at the pre-save node voltages ([`property_snapshot_at`]), so a property a
//! solve feeds compares the element model alone, and a disabled circuit element
//! is expected absent, because `Save circuit` writes none
//! ([`skipped_by_save_circuit`]).

use dss_core::exec::Dss;
use dss_core::report::export::json::JsonOpts;
use dss_core::report::export::json::schema::DSS_CLASS_LIST_ORDER;
use std::collections::{BTreeMap, BTreeSet};
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
/// emitted `Master.dss`, re-solve, and assert every object's property list,
/// the node voltages (≤1e-6 rel) and the iteration count match the pre-save
/// ones.
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
/// after the initial compile — element-creating setup (e.g. `New Energymeter…`),
/// or an `Edit` whose effect (values or set order) the saved deck carries — so it
/// must NOT be replayed after the re-compile (the emitted tree already carries
/// it). `both` are option commands
/// (`Set Maxiterations=…`) that `Save` does NOT persist, so they are re-applied
/// on both the pre-save and post-recompile solves to reach the same fixpoint.
/// `vtol` is the node-voltage relative tolerance (1e-6 for the clean-round-trip
/// feeders; a proven Save-precision floor for IEEE-8500, see that test). Discrete
/// control state (reg taps + cap banks) is always compared **exactly**, and
/// every object's property list by [`assert_properties_round_trip`].
fn round_trip_full(tag: &str, master: PathBuf, pre_only: &[&str], both: &[&str], vtol: f64) {
    round_trip_watch(tag, master, pre_only, both, vtol, &[]);
}

/// A deck element whose sizing property and the arrays it sizes must come back
/// unchanged from a Save round trip — the contract of the Save sizing-property
/// hoist (`report/save/save.rs::save_order`, RF-D01-04). The solve observables
/// [`round_trip_full`] compares cannot see it: a `Ratings` array reloaded as
/// `[ 400 0]` moves no voltage and no Y entry.
struct SizedArrays {
    /// `Class.name` as the `?` surface spells it.
    element: &'static str,
    /// The sizing property; the deck must have set it, so Save emits it.
    sizer: &'static str,
    /// Properties whose text parse is sized by `sizer`'s live value.
    arrays: &'static [&'static str],
    /// Object references that re-set `sizer` on reload, so it must be written
    /// behind them (the hoist's anchor).
    behind: &'static [&'static str],
}

/// The `?` values of `w.sizer` then `w.arrays`, keyed by the names as listed.
fn sized_values(dss: &mut Dss, tag: &str, w: &SizedArrays) -> Vec<(String, String)> {
    let props = dss
        .element_properties(w.element)
        .unwrap_or_else(|| panic!("{tag}: {} not found", w.element));
    std::iter::once(w.sizer)
        .chain(w.arrays.iter().copied())
        .map(|name| {
            let (_, v) = props
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .unwrap_or_else(|| panic!("{tag}: {}.{name}: no such property", w.element));
            (name.to_string(), v.clone())
        })
        .collect()
}

/// The `New "<Class>.<name>"` line the Save tree under `out` emitted for
/// `element` (lowercased, for a case-blind token search).
fn emitted_line(out: &std::path::Path, tag: &str, element: &str) -> String {
    let head = format!("new \"{}\"", element.to_ascii_lowercase());
    emitted_set(out)
        .iter()
        .filter(|rel| rel.ends_with(".dss"))
        .find_map(|rel| {
            let text = std::fs::read_to_string(out.join(rel))
                .unwrap_or_else(|e| panic!("{tag}: read {rel}: {e}"));
            text.lines()
                .map(str::to_ascii_lowercase)
                .find(|l| l.starts_with(&head))
        })
        .unwrap_or_else(|| panic!("{tag}: no `New \"{element}\"` line in the Save tree"))
}

/// [`round_trip_full`] plus, for every `watch` entry, the sizing-property
/// contract on the emitted deck: the sizer token precedes each sized token on
/// the element's `New` line, and the sizer and every sized array read back the
/// pre-save `?` value (numeric-token compare, [`assert_prop_token_eq`]).
fn round_trip_watch(
    tag: &str,
    master: PathBuf,
    pre_only: &[&str],
    both: &[&str],
    vtol: f64,
    watch: &[SizedArrays],
) {
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
    let pre_sized: Vec<_> = watch
        .iter()
        .map(|w| sized_values(&mut dss, tag, w))
        .collect();
    assert!(!pre.is_empty(), "{tag}: no nodes pre-save");
    assert!(
        pre_bases.values().any(|&b| b > 0.0),
        "{tag}: no bus carries a kVBase pre-save — the compare below would be vacuous"
    );
    assert!(!pre_y.is_empty(), "{tag}: empty checkpoint Y pre-save");
    let pre_props = property_snapshot(&mut dss, tag);

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
    // Last read of the reloaded solution: this one replaces its node voltages.
    let post_props = property_snapshot_at(&mut dss, tag, &pre);

    // Every object and every property the emitted deck carries — the most
    // specific report of a Save loss, so it is asserted first.
    assert_properties_round_trip(tag, &pre_props, &post_props);

    // The sizing-property contract of every watched element (see `SizedArrays`):
    // emitted order first, then the reloaded values.
    for (w, pre_vals) in watch.iter().zip(&pre_sized) {
        let line = emitted_line(&out, tag, w.element);
        let at = |name: &str| {
            line.find(&format!(" {}=", name.to_ascii_lowercase()))
                .unwrap_or_else(|| panic!("{tag}: no `{name}=` token in {line:?}"))
        };
        for reference in w.behind {
            assert!(
                at(reference) < at(w.sizer),
                "{tag}: {}: `{}=` is written ahead of `{reference}=`, which re-sets it on \
                 reload: {line:?}",
                w.element,
                w.sizer
            );
        }
        for array in w.arrays {
            assert!(
                at(w.sizer) < at(array),
                "{tag}: {}: `{}=` is written after `{array}=`, so the reload parses \
                 the array at the stale size: {line:?}",
                w.element,
                w.sizer
            );
        }
        for ((name, pre_v), (_, post_v)) in pre_vals.iter().zip(sized_values(&mut dss, tag, w)) {
            assert_prop_token_eq(pre_v, &post_v, &format!("{tag}: {}.{name}", w.element));
        }
    }

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

/// Sizing-property hoist, Capacitor `NumSteps` (RF-D01-04, `RP|RP3.11|AC1|AC-1`).
/// The vendored asymmetric-capacitor deck sets its four-step bank in the corpus
/// spelling `kvar=600 kv=12.47 numsteps=4 states=[1 0 1 0]`: `kvar` ahead of the
/// `NumSteps` that sizes its parse (r4133 `PDElements/Capacitor.pas:374`
/// `FNumSteps := InterpretDblArray(Param, FNumSteps, FkvarRating)`). Written in
/// set order — r4133's `TDSSObject.SaveWrite` guards only `LoadShape`
/// (`General/DSSObject.pas:144`) — the line reloads the four 150 kvar steps as
/// one 150 kvar value split four ways, `4 × 37.5`: the two energized steps put
/// 75 kvar instead of 300 on the bus, which the checkpoint-Y and node-voltage
/// compares catch. The watch pins the reloaded `kvar`/`States` themselves, and
/// `Phases → CMatrix` on the asymmetric `cmatrix` bank next to it.
#[test]
fn save_roundtrip_capacitor_numsteps_hoist() {
    round_trip_watch(
        "capsteps",
        repo("tests/corpus/asymmetric/capacitor/capacitor_asym.dss"),
        &[],
        &[],
        1e-6,
        &[
            SizedArrays {
                element: "capacitor.cstep",
                sizer: "NumSteps",
                arrays: &["kvar", "States"],
                behind: &[],
            },
            SizedArrays {
                element: "capacitor.cmat",
                sizer: "Phases",
                arrays: &["CMatrix"],
                behind: &[],
            },
        ],
    );
}

/// Sizing-property hoist, `Seasons → Ratings` (RF-D01-04, `RP|RP3.11|AT1|AT-1`)
/// on the vendored seasonal-rating IEEE-13 feeder (`StoCtrl_SeasonTarget`:
/// every LineCode and `Line.632670` carry `Seasons=2 Ratings=[..]`). The deck
/// sets them in natural order; re-setting `Seasons` afterwards (an `Edit`, the
/// shape a script that changes the season count takes) moves the sizer behind
/// its `Ratings` in the set order, and r4133 parses `Ratings` at the live
/// season count (`General/LineCode.pas:436`/`:442`,
/// `PDElements/Line.pas:638-645`) — written in set order, both reload as
/// `[ x 0]`. One generic-walk class (LineCode) and one override class (Line,
/// whose `Seasons` must also stay behind its `LineCode=` reference, which
/// re-fetches the season count on reload — r4133 `PDElements/Line.pas:420-421`).
/// The watch asserts that order on the emitted line (`behind`): the reloaded
/// values cannot show it here, because `mtx601` carries the line's own season
/// count (`exec::tests::report::
/// save_hoists_a_sizer_only_to_behind_the_reference_that_resets_it` pins a
/// reference with a smaller one). Ratings move no solve observable, so only the
/// property compare and the watch see them.
#[test]
fn save_roundtrip_seasons_ratings_hoist() {
    round_trip_watch(
        "seasons",
        corpus("Version8/Distrib/Examples/StoCtrl_SeasonTarget/IEEE13NodecktMOD.dss"),
        &[
            "edit linecode.mtx601 seasons=2",
            "edit line.632670 seasons=2",
        ],
        &[],
        1e-6,
        &[
            SizedArrays {
                element: "linecode.mtx601",
                sizer: "Seasons",
                arrays: &["Ratings"],
                behind: &[],
            },
            SizedArrays {
                element: "line.632670",
                sizer: "Seasons",
                arrays: &["Ratings"],
                behind: &["LineCode"],
            },
        ],
    );
}

/// `Save circuit` round-trip over a **protection-heavy** deck (WP-U2.5): a
/// Relay + Recloser + Fuse + SwtControl, each carrying r4133-surface property
/// values — renamed props (`PhCurve`/`PhFastCurve`/`CurveMultiplier`), the new
/// r4133 props (`SinglePhTrip`/`SinglePhLockout`/`Lock`/`RatedCurrent`/
/// `InterruptingRating`), and the per-phase `Normal`/`State` arrays that render
/// `[open, closed, closed, ]`. `Save` must serialize this surface such that OUR
/// own parser re-reads the identical model (the WP8.5 round-trip convention).
///
/// The gate: snapshot **every** property of every object via `element_properties`
/// (the byte-proven `?` surface, [`property_snapshot`]), `save circuit`, `clear`,
/// re-compile the emitted tree on our engine, and assert every object — the four
/// controls included — comes back with its full property list unchanged
/// ([`assert_properties_round_trip`], numeric-token compare — pins the
/// renamed/new-prop names, values, and array renders through the Save→re-parse
/// boundary). Node voltages + the discrete control decision are also compared.
/// The r4133 property values themselves are compared live against the r4133
/// DLL (epri-worker) by the corpus gate's `r4133`-channel decks under
/// `tests/corpus/controls/`. The `tests/golden/props/` goldens of
/// `props_roundtrip.rs` pin these classes' property reads to stored values, not
/// to live r4133 output. Here we pin that `Save` does not drop or corrupt the
/// r4133 surface on the way out and back.
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
    let pre_props = property_snapshot(&mut dss, "protection");
    for el in controls {
        assert!(pre_props.contains_key(el), "{el} not found pre-save");
    }

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

    // Every object's full property surface, the controls' r4133 one included,
    // must round-trip through our own parser (renamed/new prop names + values +
    // `[..]` array renders).
    let post_props = property_snapshot_at(&mut dss, "protection", &pre);
    assert_properties_round_trip("protection", &pre_props, &post_props);

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

/// `Save circuit` closes a LineGeometry line on its active conductor — the one
/// `?` reads `Wire`/`X`/`H`/`Units` from — when the conductor block would leave
/// the reload on another: a `like=` clone and a geometry that a closing `cond=1`
/// leaves on conductor 1, and two geometries with no wire on any conductor,
/// whose block writes no row and whose reload would select none: one left on
/// conductor 2, one on conductor 1. The clone sets `cond=1` itself, so the pin
/// holds wherever `like=` leaves the cursor. A geometry whose cursor is on its
/// last conductor gets no extra token, and a Save of the reload writes the same
/// `LineGeometry.dss`.
#[test]
fn save_restores_the_active_linegeometry_conductor() {
    let tag = "lgcursor";
    let out = scratch_dir(tag);
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.lgc basekv=12.47 bus1=src phases=3",
        "new wiredata.w diam=0.721 gmrac=0.0244 rac=0.306 normamps=530 runits=mi \
         radunits=in gmrunits=ft",
        "new linegeometry.g3 nconds=3 nphases=3 cond=1 wire=w x=-4 h=28 units=ft \
         cond=2 wire=w x=-1.5 h=28 units=ft cond=3 wire=w x=3 h=24 units=ft",
        "new linegeometry.cloned like=g3 cond=1",
        "new linegeometry.back nconds=2 nphases=2 cond=1 wire=w x=-1 h=30 units=ft \
         cond=2 wire=w x=1 h=32 units=ft cond=1",
        "new linegeometry.bare nconds=3 nphases=3 cond=2",
        "new linegeometry.one nconds=3 nphases=3 cond=1",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(
        dss.errors().is_empty(),
        "{tag}: deck errors: {:?}",
        dss.errors()
    );
    let (nodes, _) = snapshot(&dss);
    let pre = property_snapshot(&mut dss, tag);
    let active = |snap: &PropSnapshot, object: &str| -> Vec<String> {
        ["Cond", "X", "H"]
            .iter()
            .map(|name| {
                snap[object]
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(name))
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| panic!("{tag}: {object}.{name}: no such property"))
            })
            .collect()
    };
    assert_eq!(active(&pre, "linegeometry.g3"), ["3", "3", "24"]);
    assert_eq!(active(&pre, "linegeometry.cloned"), ["1", "-4", "28"]);
    assert_eq!(active(&pre, "linegeometry.back"), ["1", "-1", "30"]);
    assert_eq!(active(&pre, "linegeometry.bare"), ["2", "0", "0"]);
    assert_eq!(active(&pre, "linegeometry.one"), ["1", "0", "0"]);

    let save = |dss: &mut Dss, dir: &std::path::Path| {
        std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("mkdir {}: {e}", dir.display()));
        dss.command(&format!(
            "save circuit dir=\"{}\"",
            dir.to_string_lossy().replace('\\', "/")
        ));
        assert!(
            dss.errors().is_empty(),
            "{tag}: save errors: {:?}",
            dss.errors()
        );
    };
    let first = out.join("first");
    save(&mut dss, &first);
    let conds = |object: &str| -> Vec<String> {
        emitted_line(&first, tag, object)
            .split(' ')
            .filter_map(|t| t.strip_prefix("cond="))
            .map(str::to_string)
            .collect()
    };
    assert_eq!(conds("linegeometry.g3"), ["1", "2", "3"]);
    assert_eq!(conds("linegeometry.cloned"), ["1", "2", "3", "1"]);
    assert_eq!(conds("linegeometry.back"), ["1", "2", "1"]);
    assert_eq!(conds("linegeometry.bare"), ["2"]);
    assert_eq!(conds("linegeometry.one"), ["1"]);
    assert!(
        emitted_line(&first, tag, "linegeometry.one").ends_with(" cond=1"),
        "{tag}: a geometry on conductor 1 with no row closes on cond=1"
    );

    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        first
            .join("Master.dss")
            .to_string_lossy()
            .replace('\\', "/")
    ));
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "{tag}: reload errors: {:?}",
        dss.errors()
    );
    let post = property_snapshot_at(&mut dss, tag, &nodes);
    assert_eq!(active(&post, "linegeometry.cloned"), ["1", "-4", "28"]);
    assert_eq!(active(&post, "linegeometry.back"), ["1", "-1", "30"]);
    assert_eq!(active(&post, "linegeometry.bare"), ["2", "0", "0"]);
    assert_eq!(active(&post, "linegeometry.one"), ["1", "0", "0"]);
    assert_properties_round_trip(tag, &pre, &post);

    let second = out.join("second");
    save(&mut dss, &second);
    let geometries = |dir: &std::path::Path| {
        let file = dir.join("LineGeometry.dss");
        std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()))
    };
    assert_eq!(
        geometries(&first),
        geometries(&second),
        "{tag}: a Save of the reload writes another LineGeometry.dss"
    );
    std::fs::remove_dir_all(&out).ok();
}

/// A geometry with no conductor selected reloads from `Save circuit` with its
/// last written conductor selected: the language has no way to select none
/// without an error (only `cond=N` selects a conductor, user decision
/// 2026-10-04). `LineGeometry.604` of `Test/IEEE13_LineGeometry.dss` is a
/// `like=` copy, so it has none selected: before the Save `Cond` reads 0,
/// `X` and `H` 0, the conductor names empty and `Units` the default ft. Its
/// conductor table comes back row for row and its Lines' impedance with it.
/// After the reload it reads conductor 3, so `Cond`, `Wire`, `H`, `CNCable`
/// and `TSCable` differ (`X` 0 and `Units` ft agree). The pin of the five
/// `ie13geom` rows of [`PROPERTY_EXCLUSIONS`]: they are keyed by class, so this
/// test also holds the deck's other geometries, which have a selection, to an
/// exact round trip of those five properties.
#[test]
fn save_reloads_a_geometry_with_no_selection_on_its_last_row() {
    let tag = "lgnone";
    let out = scratch_dir(tag);
    let mut dss = Dss::new();
    dss.command(&format!(
        "compile \"{}\"",
        corpus("Test/IEEE13_LineGeometry.dss")
            .to_string_lossy()
            .replace('\\', "/")
    ));
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{tag}: {:?}", dss.errors());
    let (nodes, _) = snapshot(&dss);
    let pre_y = y_checkpoint(&mut dss);
    let pre = property_snapshot(&mut dss, tag);
    dss.command(&format!(
        "save circuit dir=\"{}\"",
        out.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "{tag}: save: {:?}", dss.errors());
    let line_604 = emitted_line(&out, tag, "linegeometry.604");
    let saved_604 = &line_604[line_604.find(" cond=").unwrap_or(line_604.len())..];
    assert_eq!(
        saved_604,
        " cond=1 wire=acsr_1/0 x=-4 h=28 units=ft cond=2 wire=acsr_1/0 x=3 h=28 units=ft \
         cond=3 wire=acsr_1/0 x=0 h=24 units=ft",
        "{tag}: 604 saves its table and no selection token"
    );

    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        out.join("Master.dss").to_string_lossy().replace('\\', "/")
    ));
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{tag}: reload: {:?}", dss.errors());
    let post_y = y_checkpoint(&mut dss);
    assert_eq!(pre_y.len(), post_y.len(), "{tag}: Y entry count");
    for (key, (re, im)) in &pre_y {
        let (re2, im2) = post_y[key];
        assert!(
            (re - re2).abs() <= Y_TOL * re.abs().max(1.0)
                && (im - im2).abs() <= Y_TOL * im.abs().max(1.0),
            "{tag}: Y{key:?} {re}+j{im} -> {re2}+j{im2}"
        );
    }
    let post = property_snapshot_at(&mut dss, tag, &nodes);
    let read = |snap: &PropSnapshot, object: &str, name: &str| -> String {
        snap[object]
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("{tag}: {object}.{name}: no such property"))
    };
    let five = ["Cond", "Wire", "H", "CNCable", "TSCable"];
    let moved: Vec<(String, String)> = ["Cond", "Wire", "X", "H", "Units", "CNCable", "TSCable"]
        .iter()
        .map(|p| {
            (
                read(&pre, "linegeometry.604", p),
                read(&post, "linegeometry.604", p),
            )
        })
        .collect();
    assert_eq!(
        moved,
        [
            ("0", "3"),
            ("", "acsr_1/0"),
            ("0", "0"),
            ("0", "24"),
            ("ft", "ft"),
            ("", "acsr_1/0"),
            ("", "acsr_1/0"),
        ]
        .map(|(a, b)| (a.to_string(), b.to_string()))
    );
    for geom in ["601", "602", "603", "604", "605"] {
        let object = format!("linegeometry.{geom}");
        for (name, value) in &pre[&object] {
            let after = read(&post, &object, name);
            if geom == "604" && five.iter().any(|f| f.eq_ignore_ascii_case(name)) {
                assert_ne!(value, &after, "{tag}: {object}.{name}");
            } else {
                assert!(
                    prop_token_diff(value, &after).is_ok(),
                    "{tag}: {object}.{name}: {value:?} -> {after:?}"
                );
            }
        }
    }
    std::fs::remove_dir_all(&out).ok();
}

/// `Save circuit` closes a Transformer, XfmrCode or AutoTrans line on its
/// active winding (`Wdg`, the one `?` reads `Bus`/`Conn`/`kV`/`kVA`/`%R` from)
/// when the per-winding `Wdg=1 … Wdg=N` tail would leave the reload on the
/// last one — here after an `Edit … wdg=1` behind the definition. The reload
/// reads winding 1 again. A Transformer and an AutoTrans defined winding by
/// winding, so on their last winding, get no extra token.
#[test]
fn save_restores_the_active_transformer_winding() {
    let tag = "wdgcursor";
    let out = scratch_dir(tag);
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.wdg basekv=115 bus1=src phases=3",
        "new transformer.t phases=3 windings=2 buses=[src b1] conns=[delta wye] \
         kvs=[115 12.47] kvas=[5000 5000] xhl=6",
        "edit transformer.t wdg=1 kv=115",
        "new xfmrcode.xc phases=3 windings=2 conns=[delta wye] kvs=[115 12.47] \
         kvas=[5000 5000] xhl=6",
        "edit xfmrcode.xc wdg=1 kv=115",
        "new autotrans.a1 phases=3 windings=2",
        "~ wdg=1 bus=src conn=s kv=115 kva=50000 %r=0.21",
        "~ wdg=2 bus=b2 conn=w kv=69 kva=50000 %r=0.19",
        "edit autotrans.a1 wdg=1",
        "new transformer.last phases=3 windings=2 xhl=6",
        "~ wdg=1 bus=src conn=delta kv=115 kva=5000",
        "~ wdg=2 bus=b3 conn=wye kv=12.47 kva=5000",
        "new autotrans.a2 phases=3 windings=2",
        "~ wdg=1 bus=src conn=s kv=115 kva=50000 %r=0.21",
        "~ wdg=2 bus=b4 conn=w kv=69 kva=50000 %r=0.19",
        "new load.l bus1=b1 phases=3 kv=12.47 kw=100 pf=0.95",
        "set voltagebases=[115 69 12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(
        dss.errors().is_empty(),
        "{tag}: deck errors: {:?}",
        dss.errors()
    );
    let (nodes, _) = snapshot(&dss);
    let pre = property_snapshot(&mut dss, tag);
    let active = |snap: &PropSnapshot, object: &str| -> Vec<String> {
        ["Wdg", "kV"]
            .iter()
            .map(|name| {
                snap[object]
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(name))
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| panic!("{tag}: {object}.{name}: no such property"))
            })
            .collect()
    };
    assert_eq!(active(&pre, "transformer.t"), ["1", "115"]);
    assert_eq!(active(&pre, "xfmrcode.xc"), ["1", "115"]);
    assert_eq!(active(&pre, "autotrans.a1")[0], "1");
    assert_eq!(active(&pre, "transformer.last"), ["2", "12.47"]);
    assert_eq!(active(&pre, "autotrans.a2")[0], "2");

    dss.command(&format!(
        "save circuit dir=\"{}\"",
        out.to_string_lossy().replace('\\', "/")
    ));
    assert!(
        dss.errors().is_empty(),
        "{tag}: save errors: {:?}",
        dss.errors()
    );
    for (object, tokens) in [
        ("transformer.t", &["1", "2", "1"][..]),
        ("xfmrcode.xc", &["1", "2", "1"][..]),
        ("autotrans.a1", &["1", "2", "1"][..]),
        ("transformer.last", &["1", "2"][..]),
        ("autotrans.a2", &["1", "2"][..]),
    ] {
        let wdgs: Vec<String> = emitted_line(&out, tag, object)
            .split(' ')
            .filter_map(|t| t.strip_prefix("wdg="))
            .map(str::to_string)
            .collect();
        assert_eq!(wdgs, tokens, "{tag}: {object}");
    }

    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        out.join("Master.dss").to_string_lossy().replace('\\', "/")
    ));
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "{tag}: reload errors: {:?}",
        dss.errors()
    );
    let post = property_snapshot_at(&mut dss, tag, &nodes);
    assert_eq!(active(&post, "transformer.t"), ["1", "115"]);
    assert_eq!(active(&post, "xfmrcode.xc"), ["1", "115"]);
    assert_eq!(active(&post, "autotrans.a1")[0], "1");
    assert_eq!(active(&post, "transformer.last"), ["2", "12.47"]);
    assert_eq!(active(&post, "autotrans.a2")[0], "2");
    assert_properties_round_trip(tag, &pre, &post);
    std::fs::remove_dir_all(&out).ok();
}

/// `Save circuit` writes no line for a disabled circuit element, so the
/// reloaded circuit holds none — the rule [`skipped_by_save_circuit`] gives the
/// property compare. `Save <class>` writes the same element, with
/// ` ENABLED=NO` (`golden_reports.rs::save_class_disabled_load_writes_enabled_no`).
/// The enabled line and load next to them are written and reloaded.
#[test]
fn save_circuit_writes_no_disabled_circuit_element() {
    let tag = "disabled";
    let out = scratch_dir(tag);
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.dis basekv=12.47 bus1=src phases=3",
        "new line.main bus1=src bus2=b1 phases=3 r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0 length=1",
        "new line.sw bus1=b1 bus2=b2 phases=3 switch=yes enabled=no",
        "new load.on bus1=b1 phases=3 kv=12.47 kw=100 pf=0.95",
        "new load.off bus1=b1 phases=3 kv=12.47 kw=50 pf=0.95",
        "load.off.enabled=no",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(
        dss.errors().is_empty(),
        "{tag}: deck errors: {:?}",
        dss.errors()
    );
    let (nodes, _) = snapshot(&dss);
    let pre = property_snapshot(&mut dss, tag);
    for (object, disabled) in [
        ("line.main", false),
        ("line.sw", true),
        ("load.on", false),
        ("load.off", true),
    ] {
        assert_eq!(
            skipped_by_save_circuit(&pre[object]),
            disabled,
            "{tag}: {object} disabled"
        );
    }

    dss.command(&format!(
        "save circuit dir=\"{}\"",
        out.to_string_lossy().replace('\\', "/")
    ));
    assert!(
        dss.errors().is_empty(),
        "{tag}: save errors: {:?}",
        dss.errors()
    );
    let emitted: Vec<String> = emitted_set(&out)
        .iter()
        .filter(|rel| rel.ends_with(".dss"))
        .flat_map(|rel| {
            let text = std::fs::read_to_string(out.join(rel))
                .unwrap_or_else(|e| panic!("{tag}: read {rel}: {e}"));
            text.lines()
                .map(str::to_ascii_lowercase)
                .collect::<Vec<_>>()
        })
        .collect();
    for object in ["line.sw", "load.off"] {
        let head = format!("new \"{object}\"");
        assert!(
            !emitted.iter().any(|l| l.starts_with(&head)),
            "{tag}: Save circuit wrote the disabled {object}"
        );
    }
    for object in ["line.main", "load.on"] {
        emitted_line(&out, tag, object);
    }

    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        out.join("Master.dss").to_string_lossy().replace('\\', "/")
    ));
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "{tag}: reload errors: {:?}",
        dss.errors()
    );
    for object in ["line.sw", "load.off"] {
        assert!(
            dss.element_properties(object).is_none(),
            "{tag}: the reload holds the disabled {object}"
        );
    }
    let post = property_snapshot_at(&mut dss, tag, &nodes);
    assert_properties_round_trip(tag, &pre, &post);
    std::fs::remove_dir_all(&out).ok();
}

/// Numeric-token equality (WP8.1 skeleton compare): non-numeric structure exact,
/// numbers within a tight relative floor. A Save→re-parse must reproduce a
/// property string exactly modulo last-digit `%g` rendering.
fn assert_prop_token_eq(a: &str, b: &str, ctx: &str) {
    if let Err(why) = prop_token_diff(a, b) {
        panic!("{ctx}: {why}");
    }
}

/// The comparison [`assert_prop_token_eq`] asserts: `Err` names the first
/// difference between `a` and `b`.
fn prop_token_diff(a: &str, b: &str) -> Result<(), String> {
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
                // The longest prefix that parses is the number. A flag, not
                // `e == i`, tells "none parsed": after a parse `i` is moved to
                // `e`, so that test also held on success and left the number's
                // first byte in the skeleton while skipping the byte after it.
                let mut e = end;
                let mut parsed = false;
                while e > i {
                    if let Ok(v) = s[i..e].parse::<f64>() {
                        nums.push(v);
                        skel.push('#');
                        i = e;
                        parsed = true;
                        break;
                    }
                    e -= 1;
                }
                if !parsed {
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
    if sa != sb {
        return Err(format!("structure differs ({a:?} vs {b:?})"));
    }
    if na.len() != nb.len() {
        return Err(format!("number count differs ({a:?} vs {b:?})"));
    }
    for (x, y) in na.iter().zip(&nb) {
        // Written as "within the floor" so a NaN difference is a difference.
        let within = (x - y).abs() <= 1e-9 + 1e-9 * y.abs();
        if !within {
            return Err(format!("number differs ({x} vs {y}) in {a:?} vs {b:?}"));
        }
    }
    Ok(())
}

/// [`assert_prop_token_eq`] compares the text between numbers exactly and the
/// numbers within its floor — the separator after a number included, and not
/// the number's first digit (both were inverted before RF-D01-04 part 3).
#[test]
fn prop_token_compare_checks_separators_and_values() {
    assert_prop_token_eq("[ 150 150]", "[ 150 150.0000000000001]", "floor");
    assert_prop_token_eq("0.99999999999999", "1", "first digit");
    let red = |a: &'static str, b: &'static str| {
        std::panic::catch_unwind(|| assert_prop_token_eq(a, b, "probe")).is_err()
    };
    assert!(
        red("[ 400 500]", "[ 400|500]"),
        "a separator after a number"
    );
    assert!(red("[ 600 700]", "[ 600 0]"), "a value");
    assert!(red("[ 1 2]", "[ 1 2 3]"), "a count");
}

/// Every object of a circuit — circuit elements and general objects alike —
/// keyed by its lowercased `class.name`, mapped to its full `?` property list
/// ([`Dss::element_properties`]: every property, in property-index order).
type PropSnapshot = BTreeMap<String, Vec<(String, String)>>;

/// Snapshot every object's property list. The class batches of the JSON export
/// name the objects of every registered class; only their `Name` is read. The
/// snapshot must hold every circuit element of the circuit's device list,
/// disabled ones included.
fn property_snapshot(dss: &mut Dss, tag: &str) -> PropSnapshot {
    let mut names = Vec::new();
    for class in DSS_CLASS_LIST_ORDER {
        let batch = dss
            .class_batch_to_json(class, JsonOpts::NONE)
            .unwrap_or_else(|| panic!("{tag}: class {class} is not registered"));
        let objs: serde_json::Value = serde_json::from_str(&batch)
            .unwrap_or_else(|e| panic!("{tag}: the {class} batch is not JSON: {e}"));
        let objs = objs
            .as_array()
            .unwrap_or_else(|| panic!("{tag}: the {class} batch is not an array"));
        for obj in objs {
            let name = obj["Name"]
                .as_str()
                .unwrap_or_else(|| panic!("{tag}: a {class} object without a Name: {obj}"));
            names.push(format!("{class}.{name}"));
        }
    }
    let snap: PropSnapshot = names
        .into_iter()
        .map(|full| {
            let props = dss
                .element_properties(&full)
                .unwrap_or_else(|| panic!("{tag}: {full} is in its class batch but not found"));
            (full.to_ascii_lowercase(), props)
        })
        .collect();
    assert!(!snap.is_empty(), "{tag}: the circuit holds no object");
    // Each device name must name at least as many snapshot objects as the
    // device list holds it (a name may repeat across classes).
    let mut held: BTreeMap<&str, usize> = BTreeMap::new();
    for key in snap.keys() {
        let name = key.split_once('.').map_or(key.as_str(), |(_, n)| n);
        *held.entry(name).or_default() += 1;
    }
    let mut devices: BTreeMap<String, usize> = BTreeMap::new();
    for name in dss.circuit().expect("a circuit").device_list.iter() {
        *devices.entry(name.to_ascii_lowercase()).or_default() += 1;
    }
    let missing: Vec<&String> = devices
        .iter()
        .filter(|(name, n)| held.get(name.as_str()).copied().unwrap_or(0) < **n)
        .map(|(name, _)| name)
        .collect();
    assert!(
        missing.is_empty(),
        "{tag}: circuit elements missing from the property snapshot: {missing:?}"
    );
    snap
}

/// [`property_snapshot`] of the reloaded circuit with its node voltages
/// replaced by the pre-save ones (`pre`, from [`snapshot`]), matched by node
/// name. A property a solve feeds (`Transformer.WdgCurrents`) then renders from
/// the same operating point on both sides and compares the element model
/// alone; the two solved states are compared by the node-voltage compare. Call
/// it after every other read of the reloaded solution.
fn property_snapshot_at(dss: &mut Dss, tag: &str, pre: &[(String, f64, f64)]) -> PropSnapshot {
    let by_name: std::collections::HashMap<&str, (f64, f64)> = pre
        .iter()
        .map(|(n, re, im)| (n.as_str(), (*re, *im)))
        .collect();
    let ckt = dss.circuit_mut().expect("circuit solved");
    for j in 1..=ckt.num_nodes {
        let name = ckt.node_name(j);
        let &(re, im) = by_name.get(name.as_str()).unwrap_or_else(|| {
            panic!("{tag}: node {name} of the reloaded circuit has no pre-save voltage")
        });
        let v = &mut ckt.solution.node_v[j];
        v.re = re;
        v.im = im;
    }
    property_snapshot(dss, tag)
}

/// A named exclusion from the property compare, `(deck, class, property,
/// reason, pin)`: on the round trip tagged `deck` the reloaded `class.property`
/// differs for `reason`, and the test `pin` pins the value each side holds. The
/// pin is the test function itself, so the compiler checks that it exists.
type PropertyExclusion = (&'static str, &'static str, &'static str, &'static str, fn());

/// The property compare's exclusions, matched case-blind on class and property
/// ([`excuse_reds`]). An entry its deck does not hit reds that deck
/// ([`assert_properties_round_trip`]).
const PROPERTY_EXCLUSIONS: &[PropertyExclusion] = &[
    (
        "ie13geom",
        "LineGeometry",
        "Cond",
        "a geometry with no conductor selected reloads on its last written row",
        save_reloads_a_geometry_with_no_selection_on_its_last_row,
    ),
    (
        "ie13geom",
        "LineGeometry",
        "Wire",
        "reads the conductor the reload selects",
        save_reloads_a_geometry_with_no_selection_on_its_last_row,
    ),
    (
        "ie13geom",
        "LineGeometry",
        "H",
        "reads the conductor the reload selects",
        save_reloads_a_geometry_with_no_selection_on_its_last_row,
    ),
    (
        "ie13geom",
        "LineGeometry",
        "CNCable",
        "reads the conductor the reload selects",
        save_reloads_a_geometry_with_no_selection_on_its_last_row,
    ),
    (
        "ie13geom",
        "LineGeometry",
        "TSCable",
        "reads the conductor the reload selects",
        save_reloads_a_geometry_with_no_selection_on_its_last_row,
    ),
];

/// An object `Save circuit` writes no line for, so the reload holds none: a
/// circuit element whose `Enabled` reads `No` (`Save <class>` writes it, with
/// ` ENABLED=NO`). Pinned by [`save_circuit_writes_no_disabled_circuit_element`].
fn skipped_by_save_circuit(props: &[(String, String)]) -> bool {
    props
        .iter()
        .any(|(n, v)| n.eq_ignore_ascii_case("Enabled") && v.trim().eq_ignore_ascii_case("No"))
}

/// Split the `reds` of the round trip tagged `deck` by `table`: the reds no
/// entry of that deck names, and the deck's entries that name none of them. An
/// object-level red (empty `property`) is never excused.
fn excuse_reds(
    deck: &str,
    reds: Vec<PropRed>,
    table: &[PropertyExclusion],
) -> (Vec<PropRed>, Vec<PropertyExclusion>) {
    let entries: Vec<PropertyExclusion> = table.iter().copied().filter(|e| e.0 == deck).collect();
    let mut hit = vec![false; entries.len()];
    let mut left = Vec::new();
    for red in reds {
        let class = red.object.split('.').next().unwrap_or_default();
        let entry = entries.iter().position(|&(_, c, p, _, _)| {
            !red.property.is_empty()
                && c.eq_ignore_ascii_case(class)
                && p.eq_ignore_ascii_case(&red.property)
        });
        match entry {
            Some(i) => hit[i] = true,
            None => left.push(red),
        }
    }
    let stale = entries
        .into_iter()
        .zip(hit)
        .filter(|&(_, hit)| !hit)
        .map(|(e, _)| e)
        .collect();
    (left, stale)
}

/// One difference between two [`PropSnapshot`]s: a property whose reloaded
/// value differs ([`prop_token_diff`]), or an object on one side only, or a
/// disabled circuit element the reload holds (`property` empty, `pre`/`post`
/// say which side holds it).
struct PropRed {
    object: String,
    property: String,
    pre: String,
    post: String,
}

/// Every difference between the pre-save and the reloaded snapshot. The
/// reload is expected to hold every object but the ones
/// [`skipped_by_save_circuit`].
fn property_reds(pre: &PropSnapshot, post: &PropSnapshot) -> Vec<PropRed> {
    let mut reds = Vec::new();
    for (object, pre_list) in pre {
        let skipped = skipped_by_save_circuit(pre_list);
        let Some(post_list) = post.get(object) else {
            if !skipped {
                reds.push(PropRed {
                    object: object.clone(),
                    property: String::new(),
                    pre: "present".into(),
                    post: "missing".into(),
                });
            }
            continue;
        };
        if skipped {
            reds.push(PropRed {
                object: object.clone(),
                property: String::new(),
                pre: "disabled".into(),
                post: "present".into(),
            });
            continue;
        }
        let names = |l: &[(String, String)]| l.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
        assert_eq!(
            names(pre_list),
            names(post_list),
            "{object}: the property table differs between the two snapshots"
        );
        for ((name, a), (_, b)) in pre_list.iter().zip(post_list) {
            if prop_token_diff(a, b).is_err() {
                reds.push(PropRed {
                    object: object.clone(),
                    property: name.clone(),
                    pre: a.clone(),
                    post: b.clone(),
                });
            }
        }
    }
    for object in post.keys().filter(|o| !pre.contains_key(*o)) {
        reds.push(PropRed {
            object: object.clone(),
            property: String::new(),
            pre: "missing".into(),
            post: "present".into(),
        });
    }
    reds
}

/// Assert that the reloaded deck reproduces every object and every property
/// value the circuit held before the Save, bar the [`PROPERTY_EXCLUSIONS`] of
/// the round trip tagged `tag`, listing every difference found, and that each
/// of those exclusions names a difference.
fn assert_properties_round_trip(tag: &str, pre: &PropSnapshot, post: &PropSnapshot) {
    let (reds, stale) = excuse_reds(tag, property_reds(pre, post), PROPERTY_EXCLUSIONS);
    assert!(
        reds.is_empty(),
        "{tag}: {} differences across save round-trip:\n{}",
        reds.len(),
        reds.iter()
            .map(|r| format!("  {}.{}: {:?} -> {:?}", r.object, r.property, r.pre, r.post))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let stale: Vec<String> = stale
        .iter()
        .map(|&(_, class, property, reason, _)| format!("{class}.{property} ({reason})"))
        .collect();
    assert!(
        stale.is_empty(),
        "{tag}: property exclusions no difference hits: {stale:?}"
    );
}

/// [`excuse_reds`] drops the red its deck's entry names, keeps a red that only
/// another deck's entry names and an object-level red, and returns the deck's
/// entries that no red hits.
#[test]
fn a_property_exclusion_excuses_its_own_deck_and_goes_stale_unhit() {
    fn a_pin() {}
    let red = |object: &str, property: &str| PropRed {
        object: object.into(),
        property: property.into(),
        pre: "1".into(),
        post: "2".into(),
    };
    let table: &[PropertyExclusion] = &[
        ("d1", "Capacitor", "kvar", "a reason", a_pin),
        ("d1", "Line", "Rmatrix", "a reason", a_pin),
        ("d1", "Line", "", "an object", a_pin),
        ("d2", "Load", "kW", "a reason", a_pin),
    ];
    let (left, stale) = excuse_reds(
        "d1",
        vec![
            red("capacitor.c1", "KVAR"),
            red("load.l1", "kW"),
            red("line.l1", ""),
        ],
        table,
    );
    let left: Vec<String> = left
        .iter()
        .map(|r| format!("{}.{}", r.object, r.property))
        .collect();
    assert_eq!(left, ["load.l1.kW", "line.l1."]);
    let stale: Vec<(&str, &str, &str)> = stale
        .iter()
        .map(|&(deck, class, property, _, _)| (deck, class, property))
        .collect();
    assert_eq!(stale, [("d1", "Line", "Rmatrix"), ("d1", "Line", "")]);
}

/// [`property_reds`] reports a changed value, an object the reload lost, an
/// object it added and a disabled circuit element it holds, and nothing for a
/// reload that holds every other object unchanged.
#[test]
fn property_compare_reports_changed_lost_and_added_objects() {
    let snap = |entries: &[(&str, &[(&str, &str)])]| -> PropSnapshot {
        entries
            .iter()
            .map(|(object, props)| {
                let props = props
                    .iter()
                    .map(|(n, v)| (n.to_string(), v.to_string()))
                    .collect();
                (object.to_string(), props)
            })
            .collect()
    };
    let pre = snap(&[
        ("capacitor.c1", &[("kvar", "[ 150 150]"), ("NumSteps", "2")]),
        ("line.l1", &[("Enabled", "Yes")]),
        ("line.sw", &[("Enabled", "No")]),
        ("load.off", &[("Enabled", "No")]),
    ]);
    let reload = snap(&[
        ("capacitor.c1", &[("kvar", "[ 150 150]"), ("NumSteps", "2")]),
        ("line.l1", &[("Enabled", "Yes")]),
    ]);
    assert!(
        property_reds(&pre, &reload).is_empty(),
        "every object but the disabled ones, unchanged"
    );
    let post = snap(&[
        ("capacitor.c1", &[("kvar", "[ 75 75]"), ("NumSteps", "2")]),
        ("load.l2", &[("kW", "1")]),
        ("load.off", &[("Enabled", "No")]),
    ]);
    let got: Vec<String> = property_reds(&pre, &post)
        .iter()
        .map(|r| format!("{}.{}: {} -> {}", r.object, r.property, r.pre, r.post))
        .collect();
    assert_eq!(
        got,
        [
            "capacitor.c1.kvar: [ 150 150] -> [ 75 75]",
            "line.l1.: present -> missing",
            "load.off.: disabled -> present",
            "load.l2.: missing -> present",
        ]
    );
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
    let pre_props = property_snapshot(&mut dss, "saveforms");
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
    // Every object of the zone tree and the top-level files comes back with
    // its full property list.
    let post_props = property_snapshot_at(&mut dss, "saveforms", &pre);
    assert_properties_round_trip("saveforms", &pre_props, &post_props);
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
