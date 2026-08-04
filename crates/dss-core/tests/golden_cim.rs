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

/// The delta shunt-compensator `grounded` node exactly as the pinned oracle
/// writes it — see [`lane_expected_cim`].
const DELTA_GROUNDED: &str =
    "<cim:LinearShuntCompensator.grounded>false</cim:LinearShuntCompensator.grounded>";
/// The opening tag shared by both `ACLineSegment.b0ch` nodes of the quartet.
const B0CH_OPEN: &str = "<cim:ACLineSegment.b0ch>";
/// The misnamed `g0ch` node exactly as the pinned oracle writes it.
const B0CH_ZERO: &str = "<cim:ACLineSegment.b0ch>0</cim:ACLineSegment.b0ch>";

/// Stage F (`DE_PASCALIZE_PLAN.md` Part IV.2) — the CIM writer's **deliberate
/// default-lane divergences**, as an expected-value transform of the oracle
/// golden.
///
/// The CIM XML goldens are byte-compared in *both* lanes (`tests/harness/
/// lane.rs`: their writer renders no number through the F-FMT seam, so nothing
/// in Stage F may move them wholesale). Two single-site quirks *do* move exactly
/// one line each in the default lane, and both are pure **element-name** fixes —
/// no value, count, or ordering changes. Rather than re-baselining those goldens
/// (which would drop the oracle as their source of truth), the default lane
/// compares against the oracle text with these two enumerated rewrites applied:
/// everything else stays byte-exact vs the pinned oracle.
///
/// * `compat::CIM_DELTA_SHUNT_GROUNDED_USES_LINEAR_PREFIX` — the delta arm of
///   the shunt-compensator writer emits `grounded` under the
///   `LinearShuntCompensator.` prefix; the wye arm six lines above uses
///   `ShuntCompensator.`, which is where CIM100 declares the property.
/// * `compat::CIM_ACLINESEGMENT_G0CH_WRITTEN_AS_B0CH` — the line writer's
///   `bch`/`gch`/`b0ch`/`g0ch` quartet ends with `b0ch` written twice; the
///   second one is the `g0ch` its `PerLengthSequenceImpedance` sibling spells.
///
/// Returns the lane-expected text plus the per-quirk rewrite counts, which
/// [`cim_lane_divergences_are_pinned`] uses to keep the list non-vacuous.
fn lane_expected_cim(oracle: &str) -> (String, [usize; 2]) {
    let normalized = oracle.replace("\r\n", "\n");
    if dss_core::compat::ORACLE_PARITY {
        return (normalized, [0, 0]);
    }
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

    let scratch = std::env::temp_dir().join(format!("dss_golden_cim_{circuit}"));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        compile_path.to_string_lossy().replace('\\', "/")
    ));
    for cmd in post {
        dss.command(cmd);
    }
    dss.command(&format!(
        "set datapath=\"{}\"",
        scratch.to_string_lossy().replace('\\', "/")
    ));
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

    let rust = locate_cim100(&scratch, circuit);
    assert_cim_bytes_eq(&lane_expected_cim(&oracle).0, &rust, circuit);
    std::fs::remove_dir_all(&scratch).ok();
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

    let scratch = std::env::temp_dir().join(format!("dss_golden_cim_frag_{circuit}"));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        compile_path.to_string_lossy().replace('\\', "/")
    ));
    dss.command(&format!(
        "set datapath=\"{}\"",
        scratch.to_string_lossy().replace('\\', "/")
    ));
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
        let produced = scratch.join(format!("{circuit}_CIM100_{prf}.xml"));
        let rust = std::fs::read_to_string(&produced)
            .unwrap_or_else(|e| panic!("read {}: {e}", produced.display()));
        let golden_path = golden_dir().join(format!("{circuit}_{prf}.xml"));
        let oracle = std::fs::read_to_string(&golden_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", golden_path.display()));
        assert_cim_bytes_eq(
            &lane_expected_cim(&oracle).0,
            &rust,
            &format!("{circuit}_{prf}"),
        );
    }
    std::fs::remove_dir_all(&scratch).ok();
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

/// The expected-value pin of the two CIM single-site quirks
/// (`compat::CIM_DELTA_SHUNT_GROUNDED_USES_LINEAR_PREFIX`,
/// `compat::CIM_ACLINESEGMENT_G0CH_WRITTEN_AS_B0CH`), asserted against
/// `compat::ORACLE_PARITY` so it is meaningful in **both** lanes.
///
/// It walks every committed CIM golden and pins three things at once:
///
/// 1. **Non-vacuity of the oracle side** — the committed goldens still carry
///    exactly one delta `LinearShuntCompensator.grounded` node and exactly two
///    duplicated `ACLineSegment.b0ch` nodes. If a golden is ever regenerated
///    without them, the lane split becomes dead code and this fails instead of
///    silently passing.
/// 2. **Non-vacuity of the transform** — the default lane rewrites *exactly*
///    those lines (one and two of them), the parity lane rewrites none.
/// 3. **Direction** — after the transform the default-lane expectation carries
///    the fixed names and none of the quirk names, while the parity-lane
///    expectation is the oracle text unchanged. The engine is then held to that
///    expectation byte-for-byte by every `run_case`/`run_feeder` below, so the
///    quirk cannot drift in either lane.
#[test]
fn cim_lane_divergences_are_pinned() {
    let parity = dss_core::compat::ORACLE_PARITY;
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

        let (expected, hits) = lane_expected_cim(&oracle);
        rewrites[0] += hits[0];
        rewrites[1] += hits[1];
        if parity {
            assert_eq!(
                expected,
                normalized,
                "{}: the parity lane must expect the oracle text verbatim",
                path.display()
            );
        } else {
            assert!(
                !expected.contains(DELTA_GROUNDED),
                "{}: the default lane must not expect the `LinearShuntCompensator.` prefix",
                path.display()
            );
            // The *second* b0ch is gone; the first (the real susceptance) stays.
            let exp_lines: Vec<&str> = expected.split('\n').collect();
            for (i, line) in exp_lines.iter().enumerate() {
                assert!(
                    !(line.trim_start() == B0CH_ZERO
                        && i > 0
                        && exp_lines[i - 1].trim_start().starts_with(B0CH_OPEN)),
                    "{}: the default lane must not expect a duplicated b0ch node",
                    path.display()
                );
            }
        }
    }

    assert!(
        files >= 15,
        "expected the whole CIM golden set (15 committed `.xml`), saw {files}"
    );
    assert_eq!(
        oracle_quirks,
        [1, 2],
        "the committed CIM goldens no longer pin both quirks (delta-grounded, duplicated b0ch) — \
         the lane split would be dead code"
    );
    assert_eq!(
        rewrites,
        if parity { [0, 0] } else { oracle_quirks },
        "the default lane must rewrite exactly the quirk lines and the parity lane none"
    );
}

/// Compile `deck`, `Export CIM100` into a scratch dir, and return the produced
/// XML. No UUID fixture: these probes read one boolean node, not bytes.
fn export_cim100_of(tag: &str, deck: &[&str]) -> String {
    let scratch = std::env::temp_dir().join(format!("dss_cim_probe_{tag}"));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();

    let mut dss = Dss::new();
    for cmd in deck {
        dss.command(cmd);
    }
    dss.command(&format!(
        "set datapath=\"{}\"",
        scratch.to_string_lossy().replace('\\', "/")
    ));
    dss.command("export cim100");
    assert!(dss.errors().is_empty(), "{tag}: {:?}", dss.errors());

    let xml = locate_cim100(&scratch, tag);
    std::fs::remove_dir_all(&scratch).ok();
    xml
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
// while the same circuit with the default ground neutrals still exports `true`.
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
/// writes `grounded = true` exactly when `NodeRef[j2] = 0`, "last conductor is
/// grounded solidly". Applied where each shunt class keeps its neutral: the
/// capacitor's is its second terminal (the "bus 2" the TODO names), the load's
/// is its terminal's `Nphases+1`-th conductor.
///
/// Two decks, so this pins a *reading* and not a flipped constant:
///
/// * the **probe** deck ties both neutrals to real nodes — the capacitor's
///   second terminal to a live bus, the load's 4th conductor to a grounding
///   reactor's node — where both lanes must now answer `false`;
/// * the **control** deck is the same circuit with the default (ground)
///   neutrals, where both lanes must answer `true`.
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
        "a wye load whose neutral is a live node is not `grounded` — upstream's \
         hard-coded TRUE is reproduced in no lane"
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
