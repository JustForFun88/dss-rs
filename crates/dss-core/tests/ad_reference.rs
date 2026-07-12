//! WP-AD.3 Stage 4 — official A-Diakoptics reference gate (plan D9 b/c).
//!
//! Replays two vendored EPRI A-Diakoptics examples with the **manual** partition
//! (identical cut on both engines) and compares the built matrices AND the post-AD
//! SOLVED node voltages against fresh r3723 references (harvested by
//! `tools/opendss/gen_ad_reference.py`, provenance in each ref dir):
//!
//! - **IEEE-13** (`Examples/ADiakoptics/IEEE_13_Bus`, single 3-phase link
//!   `Line.670671` → 2 zones): `ZLL`/`ZCC`/`Y4` + 41 solved node voltages
//!   (`r3723_ref/ieee13/`).
//! - **IEEE-123** (`Examples/ADiakoptics/IEEE_123_Bus-G`, r3723's own auto-tear
//!   links `[Line.l10, Line.l73]` → 3 zones, TWO reference-free → the multi-link
//!   D9c case): `ZLL`/`ZCC`/`Y4` + 278 solved node voltages (`r3723_ref/ieee123/`).
//!
//! The solved-voltage legs pin the AD SOLVE output (not just the setup matrices)
//! against a trusted external baseline — the reference-free zones' deep interior
//! is the stress case for the child stitch + re-seed (WP-AD.3 audit finding #7).
//!
//! NB the trunk's own `References/SolveDirect/ADiakoptics_matrixes/*.csv` are
//! **stale** — generated from an older IEEE-13 deck revision, their reactance is
//! ~20% off the vendored deck. The live r3723 engine on the CURRENT deck agrees
//! with this port bit-for-bit (ZLL to f64 ulp; ZCC/Y4 at the faer↔KLU last-ulp
//! floor), which is what these fixtures pin — never the stale CSVs.

use dss_core::exec::Dss;
use num_complex::Complex64;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

/// Parse an official AD matrix CSV (`Row,Col,Value(Real),Value(Imag)[,...]`) into
/// a `(row,col) -> complex` map from the first four columns.
fn parse_ref(path: &Path) -> HashMap<(i64, i64), Complex64> {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut m = HashMap::new();
    for line in text.lines().skip(1) {
        let t: Vec<&str> = line.split(',').collect();
        if t.len() < 4 {
            continue;
        }
        let (r, c) = match (t[0].trim().parse::<i64>(), t[1].trim().parse::<i64>()) {
            (Ok(r), Ok(c)) => (r, c),
            _ => continue,
        };
        let re: f64 = t[2].trim().parse().unwrap();
        let im: f64 = t[3].trim().parse().unwrap();
        m.insert((r, c), Complex64::new(re, im));
    }
    m
}

/// Compile the IEEE-13 AD example base network into `scratch` and run the manual
/// A-Diakoptics init at `Line.670671` (2 zones). Returns the coordinator `Dss`.
fn init_ieee13_manual_ad(scratch: &Path) -> Dss {
    let src = corpus("Version8/Distrib/Examples/ADiakoptics/IEEE_13_Bus");
    // Copy the deck; the local IEEELineCodes.DSS only redirects to the shared
    // ../../IEEETestCases/ (would break out of the corpus tree), so drop in the
    // real linecodes content in its place.
    for f in ["IEEE13Nodeckt.dss", "IEEE13Node_BusXY.csv"] {
        std::fs::copy(src.join(f), scratch.join(f)).unwrap();
    }
    std::fs::copy(
        corpus("Version8/Distrib/IEEETestCases/IEEELineCodes.DSS"),
        scratch.join("IEEELineCodes.DSS"),
    )
    .unwrap();
    // Trim the deck's own A-Diakoptics tail — we drive the manual cut ourselves.
    let master = scratch.join("IEEE13Nodeckt.dss");
    let text = std::fs::read_to_string(&master).unwrap();
    let mut kept = String::new();
    for line in text.lines() {
        if line.contains("A-Diakoptics part") {
            break;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    std::fs::write(&master, kept).unwrap();

    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", master.display()));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "IEEE-13 base solve errors: {:?}",
        dss.errors()
    );
    dss.command("set LinkBranches = [Line.670671]");
    dss.command("set UseMyLinkBranches=True");
    dss.command("set ADiakoptics=yes");
    assert!(
        dss.circuit().unwrap().solution.adiakoptics,
        "IEEE-13 manual AD init failed; summary: {}",
        dss.result()
    );
    dss
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "ad_ref_{tag}_{}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn ieee13_ad_matrices_match_r3723_reference() {
    let scr = scratch("ieee13");
    let dss = init_ieee13_manual_ad(&scr);
    let ckt = dss.circuit().unwrap();
    let refdir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "data",
        "adiakoptics",
        "r3723_ref",
        "ieee13",
    ]
    .iter()
    .collect();

    // The single 3-phase link ⇒ 3×3 dense matrices, matching r3723.
    assert_eq!(ckt.ad.zll.nzero(), 9, "ZLL 3×3");
    assert_eq!(ckt.ad.zcc.nzero(), 9, "ZCC 3×3");
    assert_eq!(ckt.ad.y4.nzero(), 9, "Y4 3×3");

    // Tolerances are the measured faer↔KLU floors (ZLL bit-exact to f64 ulp,
    // 3.8e-15; ZCC 6.2e-8; Y4 2.7e-8) with modest headroom — 4+ orders below any
    // structural deviation (the stale trunk CSV differs by 2.4e-2). Not loosened.
    for (name, sp, tol) in [
        ("zll", &ckt.ad.zll, 1e-12),
        ("zcc", &ckt.ad.zcc, 1e-6),
        ("y4", &ckt.ad.y4, 1e-6),
    ] {
        let rf = parse_ref(&refdir.join(format!("{name}.csv")));
        assert_eq!(rf.len(), 9, "{name}: 9 reference entries");
        let mut worst = 0.0f64;
        let mut wat = (0i64, 0i64);
        for cd in &sp.cdata {
            let refv = rf
                .get(&(cd.row as i64, cd.col as i64))
                .unwrap_or_else(|| panic!("{name}: no ref entry at ({},{})", cd.row, cd.col));
            let base = refv.norm();
            let rel = if base > 1e-9 {
                (cd.value - refv).norm() / base
            } else {
                (cd.value - refv).norm()
            };
            if rel > worst {
                worst = rel;
                wat = (cd.row as i64, cd.col as i64);
            }
        }
        assert!(
            worst < tol,
            "{name} vs r3723 worst rel {worst:.3e} @ {wat:?} exceeds {tol:.0e} \
             (faer↔KLU floor is ~6e-8; a gap this size is a port deviation)"
        );
    }
}

/// Parse the harvested post-AD `voltages.csv` (`Node,Re,Im`, node lowercased) into
/// a `lowercase-node -> complex` map.
fn parse_ref_voltages(path: &Path) -> HashMap<String, Complex64> {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut m = HashMap::new();
    for line in text.lines().skip(1) {
        let t: Vec<&str> = line.split(',').collect();
        if t.len() < 3 {
            continue;
        }
        let (re, im) = match (t[1].trim().parse::<f64>(), t[2].trim().parse::<f64>()) {
            (Ok(re), Ok(im)) => (re, im),
            _ => continue,
        };
        m.insert(t[0].trim().to_lowercase(), Complex64::new(re, im));
    }
    m
}

/// D9b/D9c post-AD SOLVE gate: solve the IEEE-13 interconnected coordinator
/// through the A-Diakoptics stitch and compare the SOLVED node voltages against
/// the r3723 post-AD reference (`voltages.csv`, harvested by
/// `gen_ad_reference.py`). This is the trusted-baseline check on the AD solve
/// *output* — the matrix gate above only pins the ZLL/ZCC/Y4 setup. Same
/// interconnected coordinator + same manual cut on both engines ⇒ the gap is the
/// faer↔KLU AD floor, not a structural deviation.
#[test]
fn ieee13_ad_voltages_match_r3723_reference() {
    let scr = scratch("ieee13v");
    let mut dss = init_ieee13_manual_ad(&scr);
    dss.command("solve"); // solve the coordinator through the AD stitch
    assert!(
        dss.errors().is_empty(),
        "IEEE-13 AD solve errors: {:?}",
        dss.errors()
    );
    let ckt = dss.circuit().unwrap();
    let rust: HashMap<String, Complex64> = (1..=ckt.num_nodes)
        .map(|i| (ckt.node_name(i).to_lowercase(), ckt.solution.node_v[i]))
        .collect();

    let refpath: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "data",
        "adiakoptics",
        "r3723_ref",
        "ieee13",
        "voltages.csv",
    ]
    .iter()
    .collect();
    let refv = parse_ref_voltages(&refpath);
    assert_eq!(refv.len(), 41, "expected 41 reference nodes");

    let mut worst = 0.0f64;
    let mut wn = String::new();
    let mut matched = 0usize;
    for (name, rv) in &refv {
        let v = *rust
            .get(name)
            .unwrap_or_else(|| panic!("Rust AD solve missing node {name}"));
        matched += 1;
        let base = rv.norm();
        let rel = if base > 1.0 {
            (v - rv).norm() / base
        } else {
            (v - rv).norm()
        };
        if rel > worst {
            worst = rel;
            wn = name.clone();
        }
    }
    assert_eq!(matched, 41, "all 41 reference nodes matched a Rust node");
    println!("IEEE-13 AD voltages vs r3723 worst rel {worst:.3e} @ {wn}");
    // Measured faer↔KLU AD floor with headroom; NOT loosened (a real structural
    // deviation would be orders larger, cf. the stale-trunk-CSV 2.4e-2 gap).
    const TOL: f64 = 1e-5;
    assert!(
        worst < TOL,
        "IEEE-13 AD voltage gap {worst:.3e} @ {wn} exceeds {TOL:e} \
         (same coordinator + cut ⇒ faer↔KLU floor; a gap this size is a deviation)"
    );
}

// ---------------------------------------------------------------------------
// D9c — IEEE_123_Bus-G MULTI-LINK reference gate (2 links → 3 zones, 2 of them
// reference-free). r3723 auto-tears this deck (`Num_SubCircuits=3`) at
// `[Line.l10, Line.l73]`; we pin those links explicitly on both engines so the
// partition is identical and deterministic.
// ---------------------------------------------------------------------------

/// Recursively copy a deck tree (IEEE_123 redirects into `feeder/`, etc.).
fn copy_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

/// Copy the IEEE_123_Bus-G tree into `scratch`, trim its AD/solve tail, compile +
/// base-solve, then run the manual 2-link A-Diakoptics init. Returns the
/// coordinator `Dss` (post-init, pre-AD-solve).
fn init_ieee123_manual_ad(scratch: &Path) -> Dss {
    let src = corpus("Version8/Distrib/Examples/ADiakoptics/IEEE_123_Bus-G");
    copy_tree(&src, scratch);
    let master = scratch.join("Master.DSS");
    let text = std::fs::read_to_string(&master).unwrap();
    // Trim at the "A-Diakoptics part" marker (identical to the harvester + IEEE-13):
    // keep only the clean base network + base solve; drop the deck's scripted
    // multi-actor AD cases (Num_SubCircuits/ActiveActor/CPU/yearly), which are not
    // this port's job here — we drive the manual cut ourselves below.
    let mut kept = String::new();
    for line in text.lines() {
        if line.contains("A-Diakoptics part") {
            break;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    std::fs::write(&master, kept).unwrap();

    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", master.display()));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "IEEE-123 base solve errors: {:?}",
        dss.errors()
    );
    dss.command("set LinkBranches = [Line.l10, Line.l73]");
    dss.command("set UseMyLinkBranches=True");
    dss.command("set ADiakoptics=yes");
    assert!(
        dss.circuit().unwrap().solution.adiakoptics,
        "IEEE-123 manual AD init failed; summary: {}",
        dss.result()
    );
    dss
}

/// Compare a sparse matrix (as `(row,col,value)` triples) against a reference map
/// at `tol`, returning `(worst_rel, at)`.
fn worst_rel_vs_ref(
    entries: impl IntoIterator<Item = (i64, i64, Complex64)>,
    rf: &HashMap<(i64, i64), Complex64>,
    name: &str,
) -> (f64, (i64, i64)) {
    let mut worst = 0.0f64;
    let mut wat = (0i64, 0i64);
    for (r, c, v) in entries {
        let refv = rf
            .get(&(r, c))
            .unwrap_or_else(|| panic!("{name}: no ref entry at ({r},{c})"));
        let base = refv.norm();
        let rel = if base > 1e-9 {
            (v - refv).norm() / base
        } else {
            (v - refv).norm()
        };
        if rel > worst {
            worst = rel;
            wat = (r, c);
        }
    }
    (worst, wat)
}

#[test]
fn ieee123_ad_matrices_and_voltages_match_r3723_reference() {
    let scr = scratch("ieee123");
    let mut dss = init_ieee123_manual_ad(&scr);
    dss.command("solve"); // solve the coordinator through the AD stitch
    assert!(
        dss.errors().is_empty(),
        "IEEE-123 AD solve errors: {:?}",
        dss.errors()
    );

    let refdir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "data",
        "adiakoptics",
        "r3723_ref",
        "ieee123",
    ]
    .iter()
    .collect();

    let ckt = dss.circuit().unwrap();
    // Two 3-phase links ⇒ block-diagonal ZLL (2×3×3 = 18), dense 6×6 ZCC/Y4.
    assert_eq!(ckt.ad.zll.nzero(), 18, "ZLL 2×(3×3)");
    assert_eq!(ckt.ad.zcc.nzero(), 36, "ZCC 6×6");
    assert_eq!(ckt.ad.y4.nzero(), 36, "Y4 6×6");

    // Measured faer↔KLU floors: ZLL 3e-15 (f64 ulp), ZCC 7.4e-10, Y4 4.9e-10 —
    // tiers well below any structural deviation, consistent with the IEEE-13 gate.
    for (name, sp, tol, n) in [
        ("zll", &ckt.ad.zll, 1e-9, 18usize),
        ("zcc", &ckt.ad.zcc, 1e-6, 36),
        ("y4", &ckt.ad.y4, 1e-6, 36),
    ] {
        let rf = parse_ref(&refdir.join(format!("{name}.csv")));
        assert_eq!(rf.len(), n, "{name}: {n} reference entries");
        let entries = sp
            .cdata
            .iter()
            .map(|cd| (cd.row as i64, cd.col as i64, cd.value));
        let (worst, wat) = worst_rel_vs_ref(entries, &rf, name);
        println!("IEEE-123 {name} vs r3723 worst rel {worst:.3e} @ {wat:?}");
        assert!(
            worst < tol,
            "IEEE-123 {name} vs r3723 worst rel {worst:.3e} @ {wat:?} exceeds {tol:.0e} \
             (multi-link faer↔KLU floor)"
        );
    }

    // Post-AD SOLVED node voltages (the reference-free zones' deep interior is the
    // stress case for the child stitch + re-seed).
    let rust: HashMap<String, Complex64> = (1..=ckt.num_nodes)
        .map(|i| (ckt.node_name(i).to_lowercase(), ckt.solution.node_v[i]))
        .collect();
    let refv = parse_ref_voltages(&refdir.join("voltages.csv"));
    assert_eq!(refv.len(), 278, "expected 278 reference nodes");
    let mut worst = 0.0f64;
    let mut wn = String::new();
    let mut matched = 0usize;
    for (nm, rv) in &refv {
        let v = *rust
            .get(nm)
            .unwrap_or_else(|| panic!("Rust AD solve missing node {nm}"));
        matched += 1;
        let base = rv.norm();
        let rel = if base > 1.0 {
            (v - rv).norm() / base
        } else {
            (v - rv).norm()
        };
        if rel > worst {
            worst = rel;
            wn = nm.clone();
        }
    }
    assert_eq!(matched, 278, "all 278 reference nodes matched a Rust node");
    println!("IEEE-123 AD voltages vs r3723 worst rel {worst:.3e} @ {wn}");
    // Multi-link AD voltage floor (2 reference-free zones): faer↔KLU + the child
    // stitch. Measured worst 4.85e-6; tier ≈ 4× measured, NOT loosened.
    const VTOL: f64 = 2e-5;
    assert!(
        worst < VTOL,
        "IEEE-123 AD voltage gap {worst:.3e} @ {wn} exceeds {VTOL:e}"
    );
}
