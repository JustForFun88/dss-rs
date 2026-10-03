//! Golden gate for the CIM100 XML export (GAPS_PLAN WPG.18): thin driver over
//! `tools/golden/gen_cim.py`'s recipe — replay the same deck (`tools/golden/
//! cim_decks/<circuit>.dss`), preload every UUID via `uuids file=<fixture>`
//! (the same `<circuit>_fixture.csv` the generator produced), issue `export
//! cim100 fil=<tmp>`, and byte-compare the produced file against `tests/
//! golden/cim/<circuit>.xml` — **exact bytes** (CRLF-normalized only), zero
//! tolerance (decision 2: with the fixture preloaded, the oracle and the Rust
//! port must produce bit-identical text, since every UUID and every `%.8g`/
//! `%d`/enum rendering is now fully determined).
//!
//! Add a Stage B-F case by dropping `<name>.dss` + `<name>_fixture.csv` in
//! `tools/golden/cim_decks/`, `<name>.xml` in `tests/golden/cim/` (regenerate
//! both via `python tools/golden/gen_cim.py`), and a `run_case("<name>")` call
//! below.

use std::path::{Path, PathBuf};

use dss_core::exec::Dss;

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn decks_dir() -> PathBuf {
    repo_root().join("tools").join("golden").join("cim_decks")
}

fn golden_dir() -> PathBuf {
    repo_root().join("tests").join("golden").join("cim")
}

/// A scratch directory under the system temp dir, named after this lane and
/// this process, so two lanes, worktrees or test binaries exporting the same
/// deck at once never share one. Removed on drop, except when a failing
/// assertion unwinds through it: then it is kept and its path printed, so the
/// export behind the failure can be inspected.
struct Scratch(PathBuf);

impl Scratch {
    fn new(kind: &str, tag: &str) -> Self {
        let lane = if cfg!(feature = "oracle-parity") {
            "parity"
        } else {
            "default"
        };
        let dir =
            std::env::temp_dir().join(format!("dss_{kind}_{tag}_{lane}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// The directory in the forward-slash form a `set datapath=` takes.
    fn datapath(&self) -> String {
        self.0.to_string_lossy().replace('\\', "/")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "kept the export of the failing test in {}",
                self.0.display()
            );
        } else {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

/// The delta shunt-compensator `grounded` node exactly as the pinned oracle
/// writes it — see [`expected_cim`].
const DELTA_GROUNDED: &str =
    "<cim:LinearShuntCompensator.grounded>false</cim:LinearShuntCompensator.grounded>";
/// The opening tag shared by both `ACLineSegment.b0ch` nodes of the quartet.
const B0CH_OPEN: &str = "<cim:ACLineSegment.b0ch>";
/// The misnamed `g0ch` node exactly as the pinned oracle writes it.
const B0CH_ZERO: &str = "<cim:ACLineSegment.b0ch>0</cim:ACLineSegment.b0ch>";

// LANE-EXCLUSION(CIM_DELTA_SHUNT_GROUNDED_USES_LINEAR_PREFIX): the delta shunt's
// `grounded` node is compared under the class the wye arm names, in both lanes.
// LANE-EXCLUSION(CIM_ACLINESEGMENT_G0CH_WRITTEN_AS_B0CH): the second of two
// consecutive `b0ch` nodes is compared as `g0ch`, in both lanes.
/// The CIM writer's two **deliberate divergences from the oracle**, as an
/// expected-value transform of the oracle golden — applied in *both* lanes since
/// `GOLDEN_REBASE_PLAN.md` G2.2c.
///
/// The CIM XML goldens are byte-compared in both lanes (`harness::lane`:
/// their writer renders no number through the F-FMT seam, so nothing in Stage F
/// may move them wholesale). Two single-site upstream mistakes move exactly one
/// line each, and both are pure **element-name** fixes — no value, count, or
/// ordering changes. Rather than re-baselining those goldens (which would drop
/// the oracle as their source of truth), the engine is compared against the
/// oracle text with these two enumerated rewrites applied: everything else stays
/// byte-exact vs the pinned oracle.
///
/// * **the delta shunt's class prefix** — the delta arm of the
///   shunt-compensator writer emits `grounded` under the
///   `LinearShuntCompensator.` prefix (`ExportCIMXML.pas:3706`; r4133 `:3187`);
///   the wye arm uses `ShuntCompensator.`, which is where CIM100 declares the
///   property (`issue-24`).
/// * **the missing `g0ch`** — the line writer's `bch`/`gch`/`b0ch`/`g0ch`
///   quartet ends with `b0ch` written twice (`ExportCIMXML.pas:4367`; r4133
///   `:3756`); the second one is the `g0ch` its `PerLengthSequenceImpedance`
///   sibling spells (`issue-25`).
///
/// **A third deliberate divergence from the goldens' source exists and is
/// deliberately NOT rewritten here, because it has no footprint on these decks.**
/// Since RP3.6(b) the LineCode units back-fill matches the line's `CondCode`
/// STRING like r4133 (`ExportCIMXML.pas:3872-3884`) instead of 0.14.5's live
/// object (`:4501`), so a line that names a code and then overrides it donates
/// its `FUserLengthUnits` to a `Units = UNITS_NONE` LineCode where 0.14.5 — which
/// generated these goldens (`tools/golden/gen_cim.py`) — does not. It moves no
/// byte here: every `New LineCode` in `tools/golden/cim_decks/*.dss` declares its
/// own `units=` (`lc_sym` kft, `mtx606`/`mtx607` mi, `lc1` kft), so the back-fill
/// loop never runs, and `cim_lines.dss`'s one switch (`Line.l_sw`) names no code.
/// Adding a rewrite for it would red [`cim_writer_divergences_are_pinned`] as
/// vacuous; the behaviour is pinned instead, deck by deck, by
/// [`cim_linecode_units_backfill_matches_the_condcode_string`]. A future CIM deck
/// of that shape (a `units=`-less LineCode named by an overridden line) will
/// therefore fail the byte compare **legitimately** — the fix is a golden
/// regenerated off r4133, not a fourth rewrite.
///
/// Returns the expected text plus the per-rewrite counts, which
/// [`cim_writer_divergences_are_pinned`] uses to keep the list non-vacuous.
fn expected_cim(oracle: &str) -> (String, [usize; 2]) {
    let normalized = oracle.replace("\r\n", "\n");
    let mut hits = [0usize; 2];
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed == DELTA_GROUNDED {
            hits[0] += 1;
            out.push(line.replace(
                "LinearShuntCompensator.grounded",
                "ShuntCompensator.grounded",
            ));
        } else if trimmed == B0CH_ZERO && i > 0 && lines[i - 1].trim_start().starts_with(B0CH_OPEN)
        {
            // Positional, not value-based: only the *second* of two consecutive
            // `b0ch` nodes is the misnamed one. A line whose genuine zero-sequence
            // susceptance happens to be 0 is never preceded by another `b0ch`.
            hits[1] += 1;
            out.push(line.replace("ACLineSegment.b0ch", "ACLineSegment.g0ch"));
        } else {
            out.push((*line).to_string());
        }
    }
    (out.join("\n"), hits)
}

/// Byte-exact compare (CRLF-normalized only — decision 2's zero-tolerance
/// gate), with a per-line diff on mismatch so a divergence points straight at
/// the offending XML element instead of a single opaque `assert_eq!`.
fn assert_cim_bytes_eq(oracle: &str, rust: &str, ctx: &str) {
    let o = oracle.replace("\r\n", "\n");
    let r = rust.replace("\r\n", "\n");
    if o == r {
        return;
    }
    let ol: Vec<&str> = o.split('\n').collect();
    let rl: Vec<&str> = r.split('\n').collect();
    for (i, (a, b)) in ol.iter().zip(rl.iter()).enumerate() {
        assert_eq!(
            a,
            b,
            "{ctx}: line {} differs\n  oracle: {a:?}\n  rust:   {b:?}",
            i + 1
        );
    }
    assert_eq!(
        ol.len(),
        rl.len(),
        "{ctx}: line count differs (oracle {}, rust {})",
        ol.len(),
        rl.len()
    );
}

/// Locate the one file matching `<circuit>_CIM100x.xml` in `scratch`.
fn locate_cim100(scratch: &Path, circuit: &str) -> String {
    // Case-insensitive: the `<CaseName>_CIM100x.xml` prefix follows the circuit's
    // original-case `CaseName`, which may differ in case between the engines.
    let want = format!("{circuit}_CIM100x.xml").to_lowercase();
    let matches: Vec<PathBuf> = std::fs::read_dir(scratch)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", scratch.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_lowercase() == want)
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "{circuit}: expected exactly one {want} in {}, found {matches:?}",
        scratch.display()
    );
    std::fs::read_to_string(&matches[0])
        .unwrap_or_else(|e| panic!("read {}: {e}", matches[0].display()))
}

/// Compile `compile_path`, run any `post` commands (e.g. `solve` for a master
/// that only defines the feeder), preload `<circuit>_fixture.csv`, run `Export
/// CIM100`, and byte-compare against `tests/golden/cim/<circuit>.xml`.
fn run(circuit: &str, compile_path: &Path, post: &[&str]) {
    let fixture = decks_dir().join(format!("{circuit}_fixture.csv"));
    assert!(
        compile_path.is_file(),
        "missing compile target: {}",
        compile_path.display()
    );
    assert!(fixture.is_file(), "missing fixture: {}", fixture.display());
    let oracle_path = golden_dir().join(format!("{circuit}.xml"));
    let oracle = std::fs::read_to_string(&oracle_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", oracle_path.display()));

    let scratch = Scratch::new("golden_cim", circuit);

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        compile_path.to_string_lossy().replace('\\', "/")
    ));
    for cmd in post {
        dss.command(cmd);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.datapath()));
    dss.command(&format!(
        "uuids file=\"{}\"",
        fixture.to_string_lossy().replace('\\', "/")
    ));
    dss.command("export cim100");
    assert!(
        dss.errors().is_empty(),
        "{circuit}: unexpected errors: {:?}",
        dss.errors()
    );

    let rust = locate_cim100(scratch.path(), circuit);
    assert_cim_bytes_eq(&expected_cim(&oracle).0, &rust, circuit);
}

/// A micro-deck case (`tools/golden/cim_decks/<circuit>.dss`).
fn run_case(circuit: &str) {
    run(circuit, &decks_dir().join(format!("{circuit}.dss")), &[]);
}

/// The seven profiles `Export CIM100Fragments` splits into (Pascal `FD_Create`,
/// `ExportCIMXML.pas:4738-4744`).
const FRAGMENT_PROFILES: [&str; 7] = ["FUN", "GEO", "TOPO", "SSH", "CAT", "EP", "DYN"];

/// Fragments-mode gate: replay `<circuit>.dss`, preload its fixture, run `Export
/// CIM100Fragments`, and byte-compare each produced `<circuit>_CIM100_<PRF>.xml`
/// against the golden `<circuit>_<PRF>.xml` (GAPS_PLAN WPG.18 Stage F decision 3,
/// applied per profile).
fn run_case_fragments(circuit: &str) {
    let compile_path = decks_dir().join(format!("{circuit}.dss"));
    let fixture = decks_dir().join(format!("{circuit}_fixture.csv"));
    assert!(
        compile_path.is_file(),
        "missing deck: {}",
        compile_path.display()
    );
    assert!(fixture.is_file(), "missing fixture: {}", fixture.display());

    let scratch = Scratch::new("golden_cim_frag", circuit);

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        compile_path.to_string_lossy().replace('\\', "/")
    ));
    dss.command(&format!("set datapath=\"{}\"", scratch.datapath()));
    dss.command(&format!(
        "uuids file=\"{}\"",
        fixture.to_string_lossy().replace('\\', "/")
    ));
    dss.command("export cim100fragments");
    assert!(
        dss.errors().is_empty(),
        "{circuit} fragments: unexpected errors: {:?}",
        dss.errors()
    );

    for prf in FRAGMENT_PROFILES {
        let produced = scratch.path().join(format!("{circuit}_CIM100_{prf}.xml"));
        let rust = std::fs::read_to_string(&produced)
            .unwrap_or_else(|e| panic!("read {}: {e}", produced.display()));
        let golden_path = golden_dir().join(format!("{circuit}_{prf}.xml"));
        let oracle = std::fs::read_to_string(&golden_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", golden_path.display()));
        assert_cim_bytes_eq(&expected_cim(&oracle).0, &rust, &format!("{circuit}_{prf}"));
    }
}

/// A corpus-feeder case: compile the vendored master (relative to
/// `tests/corpus/electricdss-tst`) then run `post` — the whole real feeder,
/// CIM-exported and byte-compared like a micro deck. `circuit` is the feeder's
/// `CaseName` (the `<CaseName>_CIM100x.xml` prefix).
fn run_feeder(circuit: &str, master_rel: &str, post: &[&str]) {
    let master: PathBuf = repo_root()
        .join("tests")
        .join("corpus")
        .join("electricdss-tst")
        .join(master_rel);
    run(circuit, &master, post);
}

// EXPECTED-VALUE-PIN(CIM_DELTA_SHUNT_GROUNDED_USES_LINEAR_PREFIX): the expected
// text names the delta shunt's `grounded` node `ShuntCompensator.grounded`, and
// carries no `LinearShuntCompensator.grounded` anywhere, in both lanes.
// EXPECTED-VALUE-PIN(CIM_ACLINESEGMENT_G0CH_WRITTEN_AS_B0CH): the expected text
// carries no two consecutive `ACLineSegment.b0ch` nodes — the second is `g0ch` —
// in both lanes.
/// The expected-value pin of the CIM writer's two corrected attribute names,
/// unconditional in **both** lanes since `GOLDEN_REBASE_PLAN.md` G2.2c.
///
/// It walks every committed CIM golden and pins three things at once:
///
/// 1. **Non-vacuity of the oracle side** — the committed goldens still carry
///    exactly one delta `LinearShuntCompensator.grounded` node and exactly two
///    duplicated `ACLineSegment.b0ch` nodes. If a golden is ever regenerated
///    without them, the transform becomes dead code and this fails instead of
///    silently passing.
/// 2. **Non-vacuity of the transform** — [`expected_cim`] rewrites *exactly*
///    those lines, one and two of them, and no others.
/// 3. **Direction** — after the transform the expectation carries the fixed
///    names and none of the upstream ones. The engine is then held to that
///    expectation byte-for-byte by every `run_case`/`run_feeder` below, in both
///    lanes, so a writer that went back to the upstream spelling fails whichever
///    lane it was built in.
#[test]
fn cim_writer_divergences_are_pinned() {
    let mut oracle_quirks = [0usize; 2];
    let mut rewrites = [0usize; 2];
    let mut files = 0usize;

    let mut paths: Vec<PathBuf> = std::fs::read_dir(golden_dir())
        .expect("read the CIM golden dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "xml").unwrap_or(false))
        .collect();
    paths.sort();

    for path in &paths {
        files += 1;
        let oracle = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let normalized = oracle.replace("\r\n", "\n");
        let lines: Vec<&str> = normalized.split('\n').collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if trimmed == DELTA_GROUNDED {
                oracle_quirks[0] += 1;
            }
            if trimmed == B0CH_ZERO && i > 0 && lines[i - 1].trim_start().starts_with(B0CH_OPEN) {
                oracle_quirks[1] += 1;
            }
        }

        let (expected, hits) = expected_cim(&oracle);
        rewrites[0] += hits[0];
        rewrites[1] += hits[1];
        assert!(
            !expected.contains(DELTA_GROUNDED),
            "{}: the expectation must not carry the `LinearShuntCompensator.` prefix",
            path.display()
        );
        // The *second* b0ch is gone; the first (the real susceptance) stays.
        let exp_lines: Vec<&str> = expected.split('\n').collect();
        for (i, line) in exp_lines.iter().enumerate() {
            assert!(
                !(line.trim_start() == B0CH_ZERO
                    && i > 0
                    && exp_lines[i - 1].trim_start().starts_with(B0CH_OPEN)),
                "{}: the expectation must not carry a duplicated b0ch node",
                path.display()
            );
        }
        // Nothing but those lines moves: the expectation and the oracle differ
        // only where a rewrite was counted, and never in length.
        assert_eq!(
            exp_lines.len(),
            normalized.split('\n').count(),
            "{}: the rewrites are renames — no line may be added or dropped",
            path.display()
        );
        assert_eq!(
            exp_lines
                .iter()
                .zip(normalized.split('\n'))
                .filter(|(e, o)| *e != o)
                .count(),
            hits[0] + hits[1],
            "{}: exactly the counted lines were rewritten",
            path.display()
        );
    }

    assert!(
        files >= 15,
        "expected the whole CIM golden set (15 committed `.xml`), saw {files}"
    );
    assert_eq!(
        oracle_quirks,
        [1, 2],
        "the committed CIM goldens no longer carry both upstream spellings (delta-grounded, \
         duplicated b0ch) — the rewrites would be dead code"
    );
    assert_eq!(
        rewrites, oracle_quirks,
        "the transform must rewrite exactly the lines the goldens carry"
    );
}

/// Compile `deck`, `Export CIM100` into a scratch dir, and return the produced
/// XML. No UUID fixture: these probes read single nodes, not bytes.
fn export_cim100_of(tag: &str, deck: &[&str]) -> String {
    let scratch = Scratch::new("cim_probe", tag);

    let mut dss = Dss::new();
    for cmd in deck {
        dss.command(cmd);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.datapath()));
    dss.command("export cim100");
    assert!(dss.errors().is_empty(), "{tag}: {:?}", dss.errors());

    locate_cim100(scratch.path(), tag)
}

/// The single `<cim:{owner}.grounded>` value in `xml`, as a bool. Asserts there
/// is exactly one, so a deck that grows a second shunt of the same kind fails
/// loudly instead of silently reading the wrong one.
fn only_grounded(xml: &str, owner: &str) -> bool {
    let open = format!("<cim:{owner}.grounded>");
    let close = format!("</cim:{owner}.grounded>");
    let vals: Vec<&str> = xml
        .lines()
        .filter_map(|l| l.trim().strip_prefix(open.as_str()))
        .filter_map(|rest| rest.strip_suffix(close.as_str()))
        .collect();
    assert_eq!(
        vals.len(),
        1,
        "expected exactly one {owner}.grounded, got {vals:?}"
    );
    match vals[0] {
        "true" => true,
        "false" => false,
        other => panic!("{owner}.grounded is not a boolean: {other:?}"),
    }
}

// EXPECTED-VALUE-PIN(CIM_WYE_GROUNDED_IS_HARDCODED_TRUE): a wye shunt whose
// neutral is tied to a live node exports as `grounded = false` in both lanes,
// while the same circuit with the default ground neutrals still exports `true`;
// a bank earthed on some phases and live on the rest reads `false` too, which
// pins the aggregation as `all` and not `any`; and a wye load reads the
// transformer writer's whole wye ladder over its own terminal and `rneut`.
/// The CIM `grounded` flag of a wye capacitor and a wye load reads the
/// **neutral**, in *both* lanes.
///
/// Upstream writes `grounded = TRUE` unconditionally for every wye capacitor
/// (`.inputs/dss_capi/src/Common/ExportCIMXML.pas:3700`; r4133
/// `Version8/Source/Common/ExportCIMXML.pas:3183`) and every wye load (`:4478`;
/// r4133 `:3854`), each under its own `// TODO - check bus 2` — so a wye with an
/// isolated or impedance-earthed neutral leaves the export as solidly grounded,
/// which is a qualitatively different machine for anyone computing earth-fault
/// currents from the exported model. Both gating oracles carry it; neither lane
/// reproduces it (`GOLDEN_REBASE_PLAN.md` G2.1g; `issue-23`).
///
/// The answer is not invented here — it is the one the **same unit's**
/// transformer writer already gives: `XfmrTankPhasesAndGround` (`:1531-1570`)
/// writes `grounded = true` when `NodeRef[j2] = 0`, "last conductor is grounded
/// solidly". Applied where each shunt class keeps its
/// neutral: the capacitor's is its second terminal (the "bus 2" the TODO names),
/// the load's is its terminal's `Nphases+1`-th conductor, which the load reads
/// together with the rest of that writer's wye ladder (its reversed first
/// conductor and its own `Rneut`).
///
/// Four decks, so this pins the *reading* — which value comes out, and how the
/// per-conductor answers are aggregated — and not a flipped constant:
///
/// * the **probe** deck ties both neutrals to real nodes — the capacitor's
///   second terminal to a live bus, the load's 4th conductor (its `rneut` left
///   at the open default) to a grounding reactor's node — where both lanes must
///   now answer `false`;
/// * the **control** deck is the same circuit with the default (ground)
///   neutrals, where both lanes must answer `true`;
/// * the **mixed** deck is what separates `all` from `any`, the one choice the
///   uniform decks above cannot see. Its capacitor is earthed on two phases and
///   live on the third (`bus2=nb.1.0.0`) — the impedance-earthed-on-one-phase
///   shape — and must read `false`: a bank is solidly grounded only when *every*
///   phase returns to ground, so one live return disqualifies it (an `any`
///   aggregation would call it grounded). Its load is single-phase with a live
///   neutral, which is also the only deck exercising the load reading's derived
///   index `node_ref[nphases]` off the 3-phase path;
/// * the **ladder** deck walks the load's wye ladder, one load per rung, each
///   on its own neutral node.
///
/// The ladder is the one `cim::power_xfmr::xfmr_tank_phases_and_ground` reads
/// for a wye winding, over the load's one terminal, first rung that answers
/// wins:
///
/// 1. the neutral (`Nphases+1`-th) conductor on the ground node: `true`;
/// 2. the first conductor on the ground node, a reversed connection: `true`;
/// 3. `rneut < 0`, the open neutral and the Load default: `false`;
/// 4. otherwise the neutral is earthed — solidly at `rneut = xneut = 0`, where
///    the Load stamps its `1.0e6` S neutral shunt, else through
///    `rneut + j·xneut`: `true`.
///
/// The r4133 DLL (11.0.0.1) writes `EnergyConsumer.grounded = true` for all
/// five ladder loads, measured with the ladder deck through
/// `tools/opendss/epri_worker.py` (`export cim100`, 2026-10-03): its load writer
/// hard-codes the value, so `ldopen` (`false` here, `true` there) is the
/// divergence and the other four agree with it. No CIM golden and no gated
/// corpus deck has a wye load with a non-ground neutral, so rungs 2-4 move no
/// compared byte.
#[test]
fn cim_wye_grounded_reads_the_neutral() {
    let build = |circuit: &str, cap: &str, load: &str| -> String {
        let deck: Vec<String> = [
            "clear".to_string(),
            format!("new circuit.{circuit} basekv=4.16 pu=1.0 phases=3 bus1=sourcebus"),
            "new line.l1 bus1=sourcebus.1.2.3 bus2=b1.1.2.3 r1=0.1 x1=0.3 c1=0 r0=0.2 x0=0.6 \
             c0=0 length=1 units=kft"
                .to_string(),
            "new line.ln bus1=sourcebus.1.2.3 bus2=nb.1.2.3 r1=0.1 x1=0.3 c1=0 r0=0.2 x0=0.6 \
             c0=0 length=1 units=kft"
                .to_string(),
            // Grounds b1.4 through a real impedance, so the load's neutral
            // conductor gets a live node number instead of node 0.
            "new reactor.ng phases=1 bus1=b1.4 r=5 x=0".to_string(),
            cap.to_string(),
            load.to_string(),
            "set voltagebases=[4.16]".to_string(),
            "calcv".to_string(),
            "solve".to_string(),
        ]
        .to_vec();
        let refs: Vec<&str> = deck.iter().map(String::as_str).collect();
        export_cim100_of(circuit, &refs)
    };

    // Probe: capacitor neutral on bus `nb`, load neutral on the reactor's node.
    let probe = build(
        "grndprobe",
        "new capacitor.capfloat bus1=b1.1.2.3 bus2=nb.1.2.3 phases=3 kv=4.16 kvar=600",
        "new load.ldfloat bus1=b1.1.2.3.4 phases=3 conn=wye kv=4.16 kw=100 kvar=30 model=1",
    );
    assert!(
        !only_grounded(&probe, "ShuntCompensator"),
        "a wye capacitor whose bus2 is a live bus is not `grounded` — upstream's \
         hard-coded TRUE is reproduced in no lane"
    );
    assert!(
        !only_grounded(&probe, "EnergyConsumer"),
        "a wye load whose neutral is a live node and whose rneut is open is not \
         `grounded` — upstream's hard-coded TRUE is reproduced in no lane"
    );

    // Control: the same circuit with the default (ground) neutrals — both lanes
    // must still answer `true`, which is what makes the probe a reading of the
    // model and not a flipped constant.
    let control = build(
        "grndcontrol",
        "new capacitor.capgrnd bus1=b1.1.2.3 phases=3 kv=4.16 kvar=600",
        "new load.ldgrnd bus1=b1.1.2.3 phases=3 conn=wye kv=4.16 kw=100 kvar=30 model=1",
    );
    assert!(
        only_grounded(&control, "ShuntCompensator"),
        "a wye capacitor with the default ground bus2 is `grounded` in every lane"
    );
    assert!(
        only_grounded(&control, "EnergyConsumer"),
        "a wye load with the default ground neutral is `grounded` in every lane"
    );

    // Mixed: the aggregation probe. The capacitor returns phases B and C to
    // ground and phase A to a live node, so `all(node == 0)` answers `false`
    // while `any(node == 0)` would answer `true` — the two uniform decks above
    // agree on both readings and cannot tell them apart. The load is
    // single-phase, so its neutral is `node_ref[1]`, the derived index the
    // 3-phase decks never exercise.
    let mixed = build(
        "grndmixed",
        "new capacitor.capmixed bus1=b1.1.2.3 bus2=nb.1.0.0 phases=3 kv=4.16 kvar=600",
        "new load.ldmixed bus1=b1.1.4 phases=1 conn=wye kv=2.4 kw=30 kvar=10 model=1",
    );
    assert!(
        !only_grounded(&mixed, "ShuntCompensator"),
        "a wye capacitor earthed on two phases and live on the third is not `grounded`: \
         the flag is `all` over terminal 2's node refs, not `any`"
    );
    assert!(
        !only_grounded(&mixed, "EnergyConsumer"),
        "a single-phase wye load whose neutral is a live node is not `grounded` — the \
         reading is `node_ref[nphases]`, off the 3-phase path too"
    );

    // Ladder: one wye load per rung, each on its own neutral node.
    let ladder = [
        "clear",
        "new circuit.grndladder basekv=4.16 pu=1.0 phases=3 bus1=sourcebus",
        "new line.l1 bus1=sourcebus.1.2.3 bus2=b1.1.2.3 r1=0.1 x1=0.3 c1=0 r0=0.2 x0=0.6 \
         c0=0 length=1 units=kft",
        // Earths `ldopen`'s neutral node outside the load, so the node has a
        // defined voltage; the load's own `rneut` still reads open.
        "new reactor.ng phases=1 bus1=b1.6 r=5 x=0",
        "new load.ldsolid bus1=b1.1.2.3.4 phases=3 conn=wye kv=4.16 kw=100 kvar=30 model=1 \
         rneut=0 xneut=0",
        "new load.ldimp bus1=b1.1.2.3.5 phases=3 conn=wye kv=4.16 kw=100 kvar=30 model=1 \
         rneut=10 xneut=2",
        "new load.ldopen bus1=b1.1.2.3.6 phases=3 conn=wye kv=4.16 kw=100 kvar=30 model=1 \
         rneut=-1",
        "new load.ldzero bus1=b1.1.2.3.0 phases=3 conn=wye kv=4.16 kw=100 kvar=30 model=1 \
         rneut=-1",
        "new load.ldrev bus1=b1.0.1 phases=1 conn=wye kv=2.4 kw=30 kvar=10 model=1 rneut=-1",
        "set voltagebases=[4.16]",
        "calcv",
        "solve",
    ];
    let grounded = grounded_by_name(&export_cim100_of("grndladder", &ladder), "EnergyConsumer");
    let expected = [
        (
            "ldsolid",
            true,
            "rung 4: rneut=0 xneut=0 earths the live neutral solidly",
        ),
        (
            "ldimp",
            true,
            "rung 4: rneut=10 xneut=2 earths the live neutral",
        ),
        (
            "ldopen",
            false,
            "rung 3: rneut=-1 leaves the live neutral open (r4133: true)",
        ),
        (
            "ldzero",
            true,
            "rung 1: the neutral is node 0, read before rneut=-1",
        ),
        (
            "ldrev",
            true,
            "rung 2: the first conductor is node 0, read before rneut=-1",
        ),
    ];
    for (load, want, why) in expected {
        assert_eq!(
            grounded.get(load).copied(),
            Some(want),
            "EnergyConsumer.grounded of {load} ({why}); every load: {grounded:?}"
        );
    }
    assert_eq!(
        grounded.len(),
        expected.len(),
        "one EnergyConsumer.grounded per load, no other: {grounded:?}"
    );
}

/// Every `<cim:{owner}.grounded>` in `xml` as a bool, keyed by the
/// `IdentifiedObject.name` of the instance it sits in (the writer emits the name
/// first, at `start_instance`). Asserts each name appears at most once.
fn grounded_by_name(xml: &str, owner: &str) -> std::collections::BTreeMap<String, bool> {
    let open = format!("<cim:{owner}.grounded>");
    let close = format!("</cim:{owner}.grounded>");
    let mut out = std::collections::BTreeMap::new();
    let mut current = String::new();
    for line in xml.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("<cim:IdentifiedObject.name>")
            && let Some(name) = rest.strip_suffix("</cim:IdentifiedObject.name>")
        {
            current = name.to_string();
        }
        if let Some(rest) = t.strip_prefix(open.as_str())
            && let Some(v) = rest.strip_suffix(close.as_str())
        {
            let value = match v {
                "true" => true,
                "false" => false,
                other => panic!("{owner}.grounded of {current:?} is not a boolean: {other:?}"),
            };
            assert!(
                out.insert(current.clone(), value).is_none(),
                "two {owner}.grounded nodes under the name {current:?}"
            );
        }
    }
    out
}

/// Stage A: writer core + skeleton + EnergySource (Vsource + buscoords only).
#[test]
fn cim_src() {
    run_case("cim_src");
}

/// Stage B: EnergyConsumer sweep + AttachLoadPhases/AttachSecondaryPhases +
/// EnergyConnectionProfile (wye/delta/secondary loads + a daily-shape ECP).
#[test]
fn cim_load() {
    run_case("cim_load");
}

/// Stage C: ACLineSegment/LoadBreakSwitch sweep (coded sym + coded matrix +
/// sym-inline + matrix-inline PUZ + geometry + spacing + CN/TS cable lines +
/// a Fuse switch), AttachLinePhases/AttachSwitchPhases, and the LineCode /
/// WireData / TSData / CNData / LineGeometry / LineSpacing catalog.
#[test]
fn cim_lines() {
    run_case("cim_lines");
}

/// Stage D: LinearShuntCompensator sweep (wye + delta + 1-phase caps, the
/// `AttachCapPhases` per-phase breakdown, the SSH `sections`/`aVRDelay`),
/// CapControl → RegulatingControl (a voltage-mode + a current-mode control,
/// `MonitoredPhaseNode`/`RegulatingControlEnum`/target value/deadband), and the
/// series-reactor → SeriesCompensator sweep.
#[test]
fn cim_shunt() {
    run_case("cim_shunt");
}

/// Stage E: transformers + autotransformers + banks + RegControl. The three
/// transformer cases (`PowerTransformerEnd`+mesh/core with no code; a
/// `TransformerTank`+`TransformerTankInfo` XfmrCode; a synthesized
/// `CIMXfmrCode_<name>` for a no-code non-3-phase unit), a 3-winding delta
/// tertiary, two `AutoTrans` (YNad1 + YNa vector groups), a 3-unit regulator
/// bank, and RegControl → `RatioTapChanger`/`TapChangerControl` (SSH
/// `TapChanger.step` = the live post-solve `TapNum`).
#[test]
fn cim_xfmr() {
    run_case("cim_xfmr");
}

/// Stage F: the DER sweeps (Generator → `SynchronousMachine`, PVSystem →
/// `PowerElectronicsConnection` + `PhotovoltaicUnit`, Storage → `BatteryUnit`) +
/// the IEEE1547 controller (InvControl volt-var catB + ExpControl → `DERIEEEType1`
/// nameplate + settings) + the DER `EnergyConnectionProfile` rows. Combined mode.
#[test]
fn cim_der() {
    run_case("cim_der");
}

/// Stage F fragments mode: the same `cim_der` deck exported via `Export
/// CIM100Fragments` — the seven per-profile files each byte-exact vs the oracle.
#[test]
fn cim_der_fragments() {
    run_case_fragments("cim_der");
}

/// Stage E corpus feeder: the vendored IEEE 13-node master, CIM-exported whole.
/// Exercises case 1 (the substation + `XFM1` transformers), case 3 (the three
/// single-phase regulators → synthesized `cimxfmrcode_reg*`), RegControl →
/// `RatioTapChanger`, and 37 `ACLineSegment`s over the real Stage-C catalog —
/// full-file byte-exact vs the pinned oracle.
#[test]
fn cim_ieee13() {
    run_feeder(
        "IEEE13Nodeckt",
        "Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss",
        &[],
    );
}

/// Stage E corpus feeder: the vendored IEEE 123-node master (definitions only, so
/// a post-compile `solve` is issued; buscoords omitted → 0,0 positions). 7
/// regulators (case-3 synthesized codes), `XFM1`, 16 `LoadBreakSwitch`es, and
/// 359 `ACLineSegment`s — full-file byte-exact vs the pinned oracle.
#[test]
fn cim_ieee123() {
    run_feeder(
        "ieee123",
        "Version8/Distrib/IEEETestCases/123Bus/IEEE123Master.dss",
        &["solve"],
    );
}

/// Every `<cim:Conductor.length>` in `xml`, keyed by the `IdentifiedObject.name`
/// of the instance it sits in (the writer emits the name first, at
/// `start_instance`). Asserts each name appears at most once, so a deck that
/// grows a second segment of the same name fails loudly.
fn conductor_lengths(xml: &str) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    let mut current = String::new();
    for line in xml.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("<cim:IdentifiedObject.name>")
            && let Some(name) = rest.strip_suffix("</cim:IdentifiedObject.name>")
        {
            current = name.to_string();
        }
        if let Some(rest) = t.strip_prefix("<cim:Conductor.length>")
            && let Some(v) = rest.strip_suffix("</cim:Conductor.length>")
        {
            assert!(
                out.insert(current.clone(), v.to_string()).is_none(),
                "two Conductor.length nodes under the name {current:?}"
            );
        }
    }
    out
}

/// `Conductor.length` converts with `FUserLengthUnits`, which an impedance
/// override does **not** erase.
///
/// `TLineObj.ResetLengthUnits` clears `LengthUnits` and `FUnitsConvert` and
/// deliberately keeps `FUserLengthUnits` — r4133
/// `Version8/Source/PDElements/Line.pas:2326-2331` and dss_capi 0.14.5
/// `src/PDElements/Line.pas:2080-2085` carry the identical statement pair under
/// the identical comment, "but do not erase FUserLengthUnits, in case of CIM
/// export". That comment names this export: the writer reads the field at
/// r4133 `Common/ExportCIMXML.pas:3707` (`v1 := To_Meters(pLine.
/// UserLengthUnits)`), `:3735` and `:3877`, and it is the **only** consumer in
/// either tree — no property renders it and no later `units=` conversion reads
/// it, which is exactly why the divergence had no census cell.
///
/// The port cleared it as well (RP3.5, 2026-08-28) — a port-authored divergence
/// from *both* oracles. Probed live: on this deck `mtx1` exports
/// `Conductor.length = 609.6` on the r4133 DLL and on the pinned dss_capi 0.14.5
/// oracle, against `2` here.
///
/// Three segments make the pin a reading rather than a constant:
///
/// * `mtx1` types `units=kft` **before** its matrices, so the `12..14` side
///   effect resets `LengthUnits` afterwards — `? line.mtx1.units` is `none` on
///   all three engines — and `FUserLengthUnits` is the only field that still
///   remembers `kft`. 2 kft = **609.6 m**;
/// * `mtx2` types `units=kft` **last**, so nothing resets it: the control that
///   proves the conversion itself is not what moved. 3 kft = **914.4 m**;
/// * `mtx3` types no `units=` at all, so `To_Meters(UNITS_NONE)` is 1.0 and the
///   length passes through as **5** — the discriminator against "always multiply
///   by 304.8".
#[test]
fn cim_conductor_length_uses_the_users_length_units() {
    let deck = [
        "clear",
        "new circuit.rp35ulu basekv=12.47 pu=1.0 phases=3 bus1=src",
        // units= BEFORE the matrices: `ResetLengthUnits` runs after the user's
        // units were recorded.
        "new line.mtx1 bus1=src.1 bus2=a.1 phases=1 units=kft length=2 \
         rmatrix=[0.095] xmatrix=[0.21] cmatrix=[3.0]",
        // Control: units= last, so nothing resets it.
        "new line.mtx2 bus1=a.1 bus2=b.1 phases=1 rmatrix=[0.095] xmatrix=[0.21] \
         cmatrix=[3.0] length=3 units=kft",
        // Control: no units= at all.
        "new line.mtx3 bus1=b.1 bus2=c.1 phases=1 rmatrix=[0.095] xmatrix=[0.21] \
         cmatrix=[3.0] length=5",
        "new load.ld bus1=c.1 phases=1 conn=wye model=1 kv=7.2 kw=100 pf=0.95",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ];
    // The live `units` render still resets — only the CIM-facing memory survives.
    {
        let mut dss = Dss::new();
        for cmd in deck {
            dss.command(cmd);
        }
        for (name, units) in [("mtx1", "none"), ("mtx2", "kft"), ("mtx3", "none")] {
            dss.command(&format!("? line.{name}.units"));
            assert_eq!(dss.result(), units, "line.{name}.units");
        }
    }

    let xml = export_cim100_of("rp35ulu", &deck);
    let lengths = conductor_lengths(&xml);
    assert_eq!(
        lengths.get("mtx1").map(String::as_str),
        Some("609.6"),
        "mtx1: 2 kft must export as 2 x To_Meters(kft); got {lengths:?}"
    );
    assert_eq!(
        lengths.get("mtx2").map(String::as_str),
        Some("914.4"),
        "mtx2 (units= last): {lengths:?}"
    );
    assert_eq!(
        lengths.get("mtx3").map(String::as_str),
        Some("5"),
        "mtx3 (no units=): To_Meters(none) is 1.0; got {lengths:?}"
    );
}

/// Every `<cim:PerLengthSequenceImpedance.r>` in `xml`, keyed by the
/// `IdentifiedObject.name` of the instance it sits in (the writer emits the name
/// first, at `start_instance`). Asserts each name appears at most once, so a deck
/// that grows a second code of the same name fails loudly.
fn per_length_seq_r(xml: &str) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    let mut current = String::new();
    for line in xml.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("<cim:IdentifiedObject.name>")
            && let Some(name) = rest.strip_suffix("</cim:IdentifiedObject.name>")
        {
            current = name.to_string();
        }
        if let Some(rest) = t.strip_prefix("<cim:PerLengthSequenceImpedance.r>")
            && let Some(v) = rest.strip_suffix("</cim:PerLengthSequenceImpedance.r>")
        {
            assert!(
                out.insert(current.clone(), v.to_string()).is_none(),
                "two PerLengthSequenceImpedance.r nodes under the name {current:?}"
            );
        }
    }
    out
}

/// The `Units=UNITS_NONE` LineCode back-fill matches the line's **`CondCode`
/// string**, which outlives `FLineCodeSpecified`.
///
/// r4133 `Common/ExportCIMXML.pas:3872-3884` walks every enabled `Line` and
/// takes the first whose `CondCode` equals the code's name — no flag test and no
/// object test:
///
/// ```pascal
/// if pLine.CondCode = pLnCd.LocalName then begin
///   pLnCd.Units := pLine.UserLengthUnits;
///   break;
/// ```
///
/// `CondCode` is written by `FetchLineCode` (`PDElements/Line.pas:387`) and
/// cleared only by the constructor (`:825`), so it survives every
/// `FLineCodeSpecified := FALSE`. dss_capi 0.14.5 has no `CondCode`: it matches
/// the live object (`(pLine.LineCodeObj <> NIL) and (pLine.LineCodeObj.Name =
/// pLnCd.LocalName)`, `src/Common/ExportCIMXML.pas:4501`) and therefore skips a
/// line whose code was superseded. r4133 is the behavioral authority
/// (CLAUDE.md 2026-08-02), so the port follows r4133 (RP3.6(b)) and this pin
/// carries the r4133 numbers, not the 0.14.5 ones.
///
/// The `lckill`, `lcsw`, `lcgeo` and `lcnoline` values were measured on the two
/// DLLs (RP3.6 probe decks D and E, 2026-08-29) — `0.301 / 304.8 =
/// 0.00098753281` against `0.301`; `lcmi`'s on the r4133 DLL:
///
/// * `lckill` — referenced by one line that then overrides `r1=` (side-effect
///   arm 6, `Line.pas:685`, kills the flag). **Discriminator**: r4133 and the
///   port back-fill `kft`; 0.14.5 leaves `UNITS_NONE` and writes `0.301`.
/// * `lcsw` — referenced only by a *switched* line, whose flag r4133 never
///   clears (`:694-700`, RP3.6(a)). Same discriminator through the other arm.
/// * `lcgeo` — referenced by a line that then takes a `geometry=`
///   (`FetchGeometryCode`, `:2131`, also a flag kill). Same again.
/// * `lcmi` — declares `units=mi` itself and is named by the `units=kft` line
///   `qkft`, so the back-fill loop never runs for it (`if pLnCd.Units =
///   UNITS_NONE`, `:3872`) and it keeps its miles: `0.301 / 1609.344 =
///   0.00018703273`, measured on the r4133 DLL (11.0.0.1) with this deck through
///   `tools/opendss/epri_worker.py` (`export cim100`, 2026-10-03).
///   **Discriminator** against a rule that back-fills a code whatever units it
///   declares, which would write the line's kft value `0.00098753281`.
/// * `lcnoline` — declared with no units and referenced by nobody, so nothing is
///   found and `To_per_Meter(UNITS_NONE) = 1.0` passes `0.301` through.
///   **Invariance control** against "always divide by 304.8".
#[test]
fn cim_linecode_units_backfill_matches_the_condcode_string() {
    let deck = [
        "clear",
        "new circuit.rp36bcc basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new wiredata.w1 diam=0.5 gmrac=0.2 rac=0.1 runits=mi radunits=in gmrunits=ft normamps=600",
        "new linegeometry.geo1 nconds=3 nphases=3 reduce=no",
        "~ cond=1 wire=w1 x=-4 h=28 units=ft",
        "~ cond=2 wire=w1 x=-1.5 h=28.5 units=ft",
        "~ cond=3 wire=w1 x=3 h=28 units=ft",
        // No `units=` on these three: UNITS_NONE, so the back-fill loop runs.
        "new linecode.lckill nphases=3 r1=0.301 x1=0.667 r0=0.882 x0=2.041 c1=3.4 c0=1.6",
        "new linecode.lcsw nphases=3 r1=0.301 x1=0.667 r0=0.882 x0=2.041 c1=3.4 c0=1.6",
        "new linecode.lcgeo nphases=3 r1=0.301 x1=0.667 r0=0.882 x0=2.041 c1=3.4 c0=1.6",
        "new linecode.lcnoline nphases=3 r1=0.301 x1=0.667 r0=0.882 x0=2.041 c1=3.4 c0=1.6",
        // Declares its own units, unlike its line's: the loop is skipped for it.
        "new linecode.lcmi nphases=3 r1=0.301 x1=0.667 r0=0.882 x0=2.041 c1=3.4 c0=1.6 units=mi",
        // The code is superseded by `r1=` (arm 6) — the flag falls, `CondCode` stays.
        "new line.qkill bus1=src bus2=a phases=3 linecode=lckill units=kft length=2 r1=0.301",
        // `switch=` leaves the flag standing (RP3.6(a)) — the other arm of the rule.
        "new line.qsw bus1=a bus2=b phases=3 linecode=lcsw units=kft Switch=True",
        // `geometry=` supersedes the code through `FetchGeometryCode` (`:2131`).
        "new line.qgeo bus1=b bus2=c phases=3 linecode=lcgeo units=kft length=2 geometry=geo1",
        // A code in miles on a line in kft, never superseded.
        "new line.qkft bus1=c bus2=d phases=3 linecode=lcmi units=kft length=2",
        "new load.ld bus1=d phases=3 conn=wye model=1 kv=12.47 kw=100 pf=0.95",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ];

    // The property render is the flag's, so it still answers `''` on the three
    // superseded lines — the split is what makes the back-fill and the render
    // disagree, and this is the half that must NOT move.
    {
        let mut dss = Dss::new();
        for cmd in deck {
            dss.command(cmd);
        }
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        for (line, rendered) in [
            ("qkill", ""),
            ("qgeo", ""),
            ("qsw", "lcsw"),
            ("qkft", "lcmi"),
        ] {
            dss.command(&format!("? line.{line}.linecode"));
            assert_eq!(dss.result(), rendered, "line.{line}.linecode");
        }
    }

    let xml = export_cim100_of("rp36bcc", &deck);
    let r = per_length_seq_r(&xml);
    for code in ["lckill", "lcsw", "lcgeo"] {
        assert_eq!(
            r.get(code).map(String::as_str),
            Some("0.00098753281"),
            "{code}: the back-fill must adopt the referencing line's kft \
             (0.301/304.8), as r4133 does by matching CondCode; got {r:?}"
        );
    }
    assert_eq!(
        r.get("lcmi").map(String::as_str),
        Some("0.00018703273"),
        "lcmi declares units=mi — the loop never runs for it, so it keeps its miles \
         (0.301/1609.344) and not its kft line's 0.00098753281; got {r:?}"
    );
    assert_eq!(
        r.get("lcnoline").map(String::as_str),
        Some("0.301"),
        "lcnoline is referenced by nobody: To_per_Meter(UNITS_NONE) = 1.0; got {r:?}"
    );
}

/// Every `<cim:ACLineSegment.*>`, `<cim:ACLineSegmentPhase.*>` and
/// `<cim:Conductor.length>` line of `xml`, trimmed, keyed by the
/// `IdentifiedObject.name` of the instance it sits in (the writer emits the name
/// first, at `start_instance`).
fn segment_nodes(xml: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut out: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let mut current = String::new();
    for line in xml.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("<cim:IdentifiedObject.name>")
            && let Some(name) = rest.strip_suffix("</cim:IdentifiedObject.name>")
        {
            current = name.to_string();
        }
        if t.starts_with("<cim:ACLineSegment.")
            || t.starts_with("<cim:ACLineSegmentPhase.")
            || t.starts_with("<cim:Conductor.length>")
        {
            out.entry(current.clone()).or_default().push(t.to_string());
        }
    }
    out
}

/// The `rdf:about` id of every `<cim:{root}>` instance of `xml` named `name`.
fn instance_ids(xml: &str, root: &str, name: &str) -> Vec<String> {
    let open = format!("<cim:{root} rdf:about=\"urn:uuid:");
    let mut out = Vec::new();
    let mut opened: Option<String> = None;
    for line in xml.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("<cim:")
            && rest.contains(" rdf:about=\"urn:uuid:")
        {
            opened = t
                .strip_prefix(open.as_str())
                .and_then(|id| id.strip_suffix("\">"))
                .map(str::to_string);
        } else if let Some(rest) = t.strip_prefix("<cim:IdentifiedObject.name>")
            && let Some(n) = rest.strip_suffix("</cim:IdentifiedObject.name>")
            && let Some(id) = opened.take()
            && n == name
        {
            out.push(id);
        }
    }
    out
}

/// The `ACLineSegment` impedance branch follows the line's spacing **flag**,
/// not the spacing objects the line still holds, while the per-phase conductor
/// data keeps reading those objects.
///
/// The writer picks the branch by the LineCode, geometry and spacing flags, in
/// that order, else writes the symmetrical components. `linecode=` drops the
/// spacing flag but keeps the spacing and its wires, and the `r1=` after it then
/// finds no flag to kill, so `lflag` still answers `? line.lflag.spacing` =
/// `sp1` — on the r4133 DLL and here — while its impedance is the code's with
/// `r1` overridden. It takes the symmetrical-components branch: `Conductor.length`
/// 1 (the physical length is unknown there), the total-ohm `r`/`x`/`r0`/`x0` =
/// 0.5 / 0.2 / 0.3 / 0.6, and no `WireSpacingInfo`. `lsp` keeps its flag and is
/// the converse: 1 kft = 304.8 m and a `WireSpacingInfo` reference, no `r`.
///
/// The second deck, `spmat`, holds the conductor-data half. Its line `lm` drops
/// the flag the same way but ends on a phase matrix (`rmatrix=`), so the writer
/// emits its per-phase breakdown: `lm` itself gets a `PerLengthImpedance`
/// reference and `Conductor.length` 304.8 with no `WireSpacingInfo`, and each of
/// `lm_A`/`lm_B`/`lm_C` (sequence numbers 1/2/3) still references the
/// `OverheadWireInfo` of `w1`, the wire the line holds.
///
/// Every literal below was measured on the r4133 DLL (11.0.0.1) with these two
/// decks through `tools/opendss/epri_worker.py` (`export cim100`, 2026-10-03),
/// the `? line.*` answers included.
#[test]
fn cim_aclinesegment_branch_follows_the_spacing_flag() {
    let deck = [
        "clear",
        "new circuit.spflag basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new wiredata.w1 diam=0.5 gmrac=0.2 rac=0.1 runits=mi radunits=in gmrunits=ft normamps=600",
        "new linespacing.sp1 nconds=3 nphases=3 x=[-1 0 1] h=[28 28 28] units=ft",
        "new linecode.lc1 nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 c1=0 c0=0 units=kft",
        "new line.lflag bus1=src bus2=a phases=3 spacing=sp1 wires=[w1 w1 w1] length=1 units=kft",
        // Drops the spacing flag; the spacing and its wires stay on the line.
        "edit line.lflag linecode=lc1",
        // Finds the flag down: the spacing is not killed a second time.
        "edit line.lflag r1=0.5",
        // The converse: the flag still up.
        "new line.lsp bus1=a bus2=b phases=3 spacing=sp1 wires=[w1 w1 w1] length=1 units=kft",
        "new load.ld bus1=b phases=3 conn=wye model=1 kv=12.47 kw=100 pf=0.95",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ];

    // The objects outlive the flag, so the spacing still renders on both lines.
    {
        let mut dss = Dss::new();
        for cmd in deck {
            dss.command(cmd);
        }
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        for (query, want) in [
            ("line.lflag.spacing", "sp1"),
            ("line.lflag.linecode", ""),
            ("line.lsp.spacing", "sp1"),
        ] {
            dss.command(&format!("? {query}"));
            assert_eq!(dss.result(), want, "{query}");
        }
    }

    let nodes = segment_nodes(&export_cim100_of("spflag", &deck));
    let of = |line: &str| -> Vec<String> { nodes.get(line).cloned().unwrap_or_default() };

    let lflag = of("lflag");
    for want in [
        "<cim:Conductor.length>1</cim:Conductor.length>",
        "<cim:ACLineSegment.r>0.5</cim:ACLineSegment.r>",
        "<cim:ACLineSegment.x>0.2</cim:ACLineSegment.x>",
        "<cim:ACLineSegment.r0>0.3</cim:ACLineSegment.r0>",
        "<cim:ACLineSegment.x0>0.6</cim:ACLineSegment.x0>",
    ] {
        assert!(
            lflag.iter().any(|l| l == want),
            "lflag (spacing flag down) takes the symmetrical-components branch: \
             missing {want}; its nodes: {lflag:?}"
        );
    }
    assert!(
        !lflag
            .iter()
            .any(|l| l.starts_with("<cim:ACLineSegment.WireSpacingInfo")),
        "lflag must not reference the spacing whose flag is down: {lflag:?}"
    );

    let lsp = of("lsp");
    assert!(
        lsp.iter()
            .any(|l| l == "<cim:Conductor.length>304.8</cim:Conductor.length>"),
        "lsp (spacing flag up) exports 1 kft as 304.8 m: {lsp:?}"
    );
    assert!(
        lsp.iter()
            .any(|l| l.starts_with("<cim:ACLineSegment.WireSpacingInfo rdf:resource=")),
        "lsp (spacing flag up) references its spacing: {lsp:?}"
    );
    assert!(
        !lsp.iter().any(|l| l.starts_with("<cim:ACLineSegment.r>")),
        "lsp (spacing flag up) writes no symmetrical-component r: {lsp:?}"
    );

    // The conductor data of a flag-down line: a phase matrix after `linecode=`.
    let spmat = [
        "clear",
        "new circuit.spmat basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new wiredata.w1 diam=0.5 gmrac=0.2 rac=0.1 runits=mi radunits=in gmrunits=ft normamps=600",
        "new linespacing.sp1 nconds=3 nphases=3 x=[-1 0 1] h=[28 28 28] units=ft",
        "new linecode.lcm nphases=3 rmatrix=[0.3 | 0.1 0.3 | 0.1 0.1 0.3] \
         xmatrix=[0.6 | 0.2 0.6 | 0.2 0.2 0.6] cmatrix=[0 | 0 0 | 0 0 0] units=kft",
        "new line.lm bus1=src bus2=a phases=3 spacing=sp1 wires=[w1 w1 w1] length=1 units=kft",
        "edit line.lm linecode=lcm",
        "edit line.lm rmatrix=[0.31 | 0.1 0.31 | 0.1 0.1 0.31]",
        "new load.ld bus1=a phases=3 conn=wye model=1 kv=12.47 kw=100 pf=0.95",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ];
    {
        let mut dss = Dss::new();
        for cmd in spmat {
            dss.command(cmd);
        }
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        for (query, want) in [("line.lm.spacing", "sp1"), ("line.lm.linecode", "")] {
            dss.command(&format!("? {query}"));
            assert_eq!(dss.result(), want, "{query}");
        }
    }

    let xml = export_cim100_of("spmat", &spmat);
    let wire = instance_ids(&xml, "OverheadWireInfo", "w1");
    assert_eq!(wire.len(), 1, "one OverheadWireInfo named w1: {wire:?}");
    let wire_ref = format!(
        "<cim:ACLineSegmentPhase.WireInfo rdf:resource=\"urn:uuid:{}\"/>",
        wire[0]
    );
    let nodes = segment_nodes(&xml);
    let of = |line: &str| -> Vec<String> { nodes.get(line).cloned().unwrap_or_default() };

    let lm = of("lm");
    assert!(
        lm.iter()
            .any(|l| l == "<cim:Conductor.length>304.8</cim:Conductor.length>")
            && lm
                .iter()
                .any(|l| l.starts_with("<cim:ACLineSegment.PerLengthImpedance rdf:resource=")),
        "lm (spacing flag down, phase matrix) exports 304.8 m and its per-length \
         impedance: {lm:?}"
    );
    assert!(
        !lm.iter()
            .any(|l| l.starts_with("<cim:ACLineSegment.WireSpacingInfo")),
        "lm must not reference the spacing whose flag is down: {lm:?}"
    );
    for (phase, seq) in [("lm_A", 1), ("lm_B", 2), ("lm_C", 3)] {
        let got = of(phase);
        let sequence = format!(
            "<cim:ACLineSegmentPhase.sequenceNumber>{seq}</cim:ACLineSegmentPhase.sequenceNumber>"
        );
        assert!(
            got.contains(&sequence) && got.contains(&wire_ref),
            "{phase}: the flag-down line still references the wire it holds, {wire_ref}, \
             at sequence number {seq}; got {got:?}"
        );
    }
}
