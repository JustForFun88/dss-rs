//! A-Diakoptics circuit-tearing gates (WP-AD.2 Stage B,
//! `DIAKOPTICS_PSTCALC_PLAN.md` §3). Rust-only (plan D8 — no AD commands appear
//! in any family-manifest deck against the pinned oracle; Part II has no oracle
//! leg by design, §0.2). These drive synthesized radial **3-phase** feeders
//! through the script interface — `set Num_SubCircuits=N; Tear_Circuit` — and
//! assert the tearing invariants: the sub-circuit count, that the link branches
//! are 3-phase Lines, and that the emitted `.part.N` partition is balanced with
//! the requested number of zones. Every deck fixes `Num_SubCircuits` explicitly
//! (plan D6 — no test depends on `available_parallelism`).
//!
//! Torn artifacts land in per-test temp dirs (never the source tree).

use dss_core::exec::Dss;
use std::path::PathBuf;

/// A synthesized radial **all-3-phase** feeder: a `sourcebus`-rooted main chain
/// of `n_main` buses plus `n_lat` single-bus laterals hung off evenly spaced
/// trunk buses. Every branch is a 3-phase `Line` (so any cut is a 3-phase-Line
/// cut — the D5 ZLL constraint is satisfied by construction), every bus carries
/// a 3-phase load. Deterministic, inline (no `File=` shapes). Reused by the
/// WP-AD.3/AD.4 gates.
fn radial_feeder(name: &str, n_main: usize, n_lat: usize) -> Vec<String> {
    let mut c = vec![
        "clear".to_string(),
        format!("new circuit.{name} basekv=12.47 phases=3 bus1=sourcebus"),
        "new linecode.lc nphases=3 r1=0.15 x1=0.35 r0=0.45 x0=1.05 c1=0 c0=0 units=km".to_string(),
    ];
    // Main trunk: sourcebus -> m1 -> m2 -> ... -> m{n_main}.
    for i in 1..=n_main {
        let prev = if i == 1 {
            "sourcebus".to_string()
        } else {
            format!("m{}", i - 1)
        };
        c.push(format!(
            "new line.main{i} bus1={prev} bus2=m{i} linecode=lc length=0.1 units=km"
        ));
        c.push(format!(
            "new load.lm{i} bus1=m{i} phases=3 kv=12.47 kw=120 pf=0.95 model=1"
        ));
    }
    // Laterals: one bus each, hung off evenly spaced trunk buses.
    for j in 1..=n_lat {
        let anchor = 1 + (j * n_main) / (n_lat + 1);
        c.push(format!(
            "new line.lat{j} bus1=m{anchor} bus2=x{j} linecode=lc length=0.08 units=km"
        ));
        c.push(format!(
            "new load.lx{j} bus1=x{j} phases=3 kv=12.47 kw=90 pf=0.95 model=1"
        ));
    }
    c.push("set voltagebases=[12.47]".to_string());
    c.push("calcv".to_string());
    c.push("solve".to_string());
    c
}

fn scratch_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "dss_ad_{tag}_{}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
    d
}

fn build(dss: &mut Dss, deck: &[String], scratch: &std::path::Path) {
    for cmd in deck {
        dss.command(cmd);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    assert!(dss.errors().is_empty(), "build errors: {:?}", dss.errors());
}

/// Read the emitted `<name>_.graph.part.<k>` partition file as per-vertex labels.
fn read_part(scratch: &std::path::Path, name: &str, k: i32) -> Vec<i32> {
    let p = scratch.join(format!("{name}_.graph.part.{k}"));
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().parse::<i32>().unwrap())
        .collect()
}

#[test]
fn tear_midi_two_zones_reports_and_partitions() {
    let name = "tearmidi";
    let scratch = scratch_dir("midi2");
    let mut dss = Dss::new();
    build(&mut dss, &radial_feeder(name, 36, 4), &scratch);

    dss.command("set Num_SubCircuits=2");
    dss.command("Tear_Circuit");
    assert!(dss.errors().is_empty(), "tear errors: {:?}", dss.errors());

    // GlobalResult (Diakoptics.pas:526).
    assert_eq!(dss.result(), "Sub-Circuits Created: 2");

    // The link branches are 3-phase Lines (`get LinkBranches`). One cut for a
    // 2-way tree tear; the reference slot (index 0) is empty.
    dss.command("get LinkBranches");
    let links = dss.result().to_string();
    let link_names: Vec<String> = links
        .trim_matches(['[', ']'])
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    assert!(
        !link_names.is_empty(),
        "expected at least one link branch, got {links:?}"
    );
    for l in &link_names {
        assert!(
            l.to_lowercase().starts_with("line."),
            "link branch {l:?} is not a Line"
        );
    }

    // Partition file: exactly 2 zones, balanced (neither zone < 25% of buses).
    let part = read_part(&scratch, name, 2);
    let n = part.len();
    let z0 = part.iter().filter(|&&l| l == 0).count();
    let z1 = n - z0;
    let mut labels: Vec<i32> = part.clone();
    labels.sort_unstable();
    labels.dedup();
    assert_eq!(labels, vec![0, 1], "expected exactly 2 zone labels");
    assert!(
        z0 * 4 >= n && z1 * 4 >= n,
        "unbalanced partition: {z0} vs {z1} of {n}"
    );

    std::fs::remove_dir_all(&scratch).ok();
}

#[test]
fn tear_midi_three_zones() {
    let name = "tearmidi3";
    let scratch = scratch_dir("midi3");
    let mut dss = Dss::new();
    build(&mut dss, &radial_feeder(name, 45, 3), &scratch);

    dss.command("set Num_SubCircuits=3");
    dss.command("Tear_Circuit");
    assert!(dss.errors().is_empty(), "tear errors: {:?}", dss.errors());
    assert_eq!(dss.result(), "Sub-Circuits Created: 3");

    let part = read_part(&scratch, name, 3);
    let mut labels = part.clone();
    labels.sort_unstable();
    labels.dedup();
    assert_eq!(labels, vec![0, 1, 2], "expected 3 zone labels");
    std::fs::remove_dir_all(&scratch).ok();
}

#[test]
fn tear_macro_two_zones() {
    let name = "tearmacro";
    let scratch = scratch_dir("macro2");
    let mut dss = Dss::new();
    // ~200-bus macro feeder.
    build(&mut dss, &radial_feeder(name, 180, 18), &scratch);

    dss.command("set Num_SubCircuits=2");
    dss.command("Tear_Circuit");
    assert!(dss.errors().is_empty(), "tear errors: {:?}", dss.errors());
    assert_eq!(dss.result(), "Sub-Circuits Created: 2");

    let part = read_part(&scratch, name, 2);
    assert!(
        part.len() >= 190,
        "expected ~200 vertices, got {}",
        part.len()
    );
    let z0 = part.iter().filter(|&&l| l == 0).count();
    let z1 = part.len() - z0;
    assert!(z0 > 0 && z1 > 0, "both zones non-empty");
    std::fs::remove_dir_all(&scratch).ok();
}

#[test]
fn tear_manual_link_branches_uses_requested_cut() {
    let name = "tearman";
    let scratch = scratch_dir("manual");
    let mut dss = Dss::new();
    build(&mut dss, &radial_feeder(name, 20, 0), &scratch);

    // Force the cut at a specific 3-phase trunk line.
    dss.command("set LinkBranches=[line.main10]");
    dss.command("set UseMyLinkBranches=True");
    dss.command("Tear_Circuit");
    assert!(
        dss.errors().is_empty(),
        "manual tear errors: {:?}",
        dss.errors()
    );

    // The requested link branch is preserved (get LinkBranches echoes it).
    dss.command("get LinkBranches");
    assert!(
        dss.result().to_lowercase().contains("line.main10"),
        "manual link not used: {}",
        dss.result()
    );
    std::fs::remove_dir_all(&scratch).ok();
}

#[test]
fn one_zone_request_is_handled() {
    let name = "tearone";
    let scratch = scratch_dir("one");
    let mut dss = Dss::new();
    build(&mut dss, &radial_feeder(name, 12, 0), &scratch);

    dss.command("set Num_SubCircuits=1");
    dss.command("Tear_Circuit");
    // A single-piece request partitions all vertices to zone 0; the walk finds
    // only the reference location => one sub-circuit, no error.
    assert!(dss.errors().is_empty(), "1-zone errors: {:?}", dss.errors());
    assert_eq!(dss.result(), "Sub-Circuits Created: 1");

    let part = read_part(&scratch, name, 1);
    assert!(part.iter().all(|&l| l == 0), "1-zone: all labels zero");
    std::fs::remove_dir_all(&scratch).ok();
}

#[test]
fn set_get_ad_options_roundtrip() {
    let scratch = scratch_dir("opts");
    let mut dss = Dss::new();
    build(&mut dss, &radial_feeder("tearopt", 8, 0), &scratch);

    dss.command("set Num_SubCircuits=4");
    dss.command("get Num_SubCircuits");
    assert_eq!(dss.result(), "4");

    dss.command("set Coverage=0.75");
    dss.command("get Coverage");
    assert_eq!(dss.result(), "0.75");

    dss.command("set UseMyLinkBranches=True");
    dss.command("get UseMyLinkBranches");
    assert_eq!(dss.result(), "Yes");

    // `set ADiakoptics` is a scoped refusal (WP-AD.3), not an unknown-parameter.
    dss.command("set ADiakoptics=True");
    assert!(
        dss.errors().iter().any(|e| e.contains("WP-AD.3")),
        "expected scoped ADiakoptics refusal, got {:?}",
        dss.errors()
    );
    std::fs::remove_dir_all(&scratch).ok();
}
