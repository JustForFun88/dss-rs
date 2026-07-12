//! WP-AD.3 Stage 4 — official A-Diakoptics matrix reference gate (plan D9 b/c).
//!
//! Replays the EPRI IEEE-13 A-Diakoptics example (`Examples/ADiakoptics/
//! IEEE_13_Bus`, vendored in the corpus) with the **manual** partition
//! `set LinkBranches=[Line.670671] UseMyLinkBranches=True` — the identical cut on
//! both engines (D9c) — and compares the built `ZLL`/`ZCC`/`Y4` against fresh
//! r3723 references committed under `tests/data/adiakoptics/r3723_ref/ieee13/`
//! (harvested by `tools/opendss/gen_ad_reference.py`, provenance in that dir).
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
