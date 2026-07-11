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
    // Balanced: no zone below ~1/6 of the buses (catches a lopsided partition
    // regression that leaves 3 labels present but one zone near-empty).
    let n = part.len();
    for z in 0..3 {
        let sz = part.iter().filter(|&&l| l == z).count();
        assert!(sz * 6 >= n, "zone {z} too small: {sz} of {n}");
    }
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
    // Balanced 2-way split — neither zone below 25% (a 1-vs-199 partition
    // regression fails here, where a bare non-empty check would pass).
    let n = part.len();
    assert!(
        z0 * 4 >= n && z1 * 4 >= n,
        "unbalanced macro partition: {z0} vs {z1} of {n}"
    );
    std::fs::remove_dir_all(&scratch).ok();
}

/// The manual-links branch: `set LinkBranches=[...] UseMyLinkBranches=True`
/// drives the cut count. Each user link is a cut, so N link branches yield N+1
/// sub-circuits — the official setter reserves an empty index-0 reference
/// placeholder (ExecOptions.pas:842–844) and `Tear_Circuit` returns
/// `length(Locations) = length(Link_Branches) = N+1`. Both counts and the
/// `get LinkBranches` echo are empirically pinned against the r3723 binary
/// (Oddie bridge): `[line.main10]` → "Sub-Circuits Created: 2", `[line.main5,
/// line.main10]` → 3, and `get LinkBranches` drops the placeholder
/// (`line.main10`, no brackets). This is the regression that caught the missing
/// index-0 placeholder (which made a single link tear to 1, not 2).
#[test]
fn tear_manual_link_branches_use_requested_cuts() {
    // One cut at a specific 3-phase trunk line → two sub-circuits.
    {
        let name = "tearman1";
        let scratch = scratch_dir("manual1");
        let mut dss = Dss::new();
        build(&mut dss, &radial_feeder(name, 20, 0), &scratch);

        dss.command("set LinkBranches=[line.main10]");
        // The placeholder makes the stored list length 2, but `get` hides it.
        dss.command("get LinkBranches");
        assert_eq!(dss.result(), "line.main10");

        dss.command("set UseMyLinkBranches=True");
        dss.command("Tear_Circuit");
        assert!(
            dss.errors().is_empty(),
            "manual tear errors: {:?}",
            dss.errors()
        );
        // N=1 link → N+1=2 sub-circuits (regression guard for the placeholder).
        assert_eq!(dss.result(), "Sub-Circuits Created: 2");
        std::fs::remove_dir_all(&scratch).ok();
    }

    // Two cuts → three sub-circuits: the count scales with the requested list,
    // proving the manual list actually drives the tear (not a fixed value).
    {
        let name = "tearman2";
        let scratch = scratch_dir("manual2");
        let mut dss = Dss::new();
        build(&mut dss, &radial_feeder(name, 20, 0), &scratch);

        dss.command("set LinkBranches=[line.main5, line.main10]");
        dss.command("get LinkBranches");
        assert_eq!(dss.result(), "line.main5, line.main10");

        dss.command("set UseMyLinkBranches=True");
        dss.command("Tear_Circuit");
        assert!(
            dss.errors().is_empty(),
            "manual tear errors: {:?}",
            dss.errors()
        );
        assert_eq!(dss.result(), "Sub-Circuits Created: 3");
        std::fs::remove_dir_all(&scratch).ok();
    }
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

/// Read the emitted `<name>_.graph` file as `(n_cols, num_edges, weights)` —
/// the header fields plus every edge-weight token across the adjacency lines.
fn read_graph(scratch: &std::path::Path, name: &str) -> (i32, i32, Vec<i32>) {
    let p = scratch.join(format!("{name}_.graph"));
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let mut lines = text.lines();
    let header: Vec<i32> = lines
        .next()
        .unwrap()
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect();
    // Adjacency lines are `neigh weight neigh weight ...`; collect the weights
    // (every second token).
    let mut weights = Vec::new();
    for line in lines {
        let toks: Vec<i32> = line
            .split_whitespace()
            .map(|t| t.parse().unwrap())
            .collect();
        for pair in toks.chunks(2) {
            if pair.len() == 2 {
                weights.push(pair[1]);
            }
        }
    }
    (header[0], header[1], weights)
}

/// `Create_MeTIS_graph` edge weighting + parallel-branch dedup
/// (Circuit.pas:1213, plan §WP-AD.2). Exercises the two branches the all-Line
/// radial fixtures never hit: the **Transformer-weight-1** rule and the
/// **parallel-branch dedup** loop. A `Transformer` edge must weigh 1 regardless
/// of its phase count (here a 3-phase transformer), while `Line` edges weigh
/// their phase count (3); two parallel lines between the same buses collapse to
/// a single edge, so the header edge count is below the raw branch-object count.
#[test]
fn metis_graph_transformer_weight1_and_parallel_dedup() {
    let name = "teargraph";
    let scratch = scratch_dir("graph");
    let mut dss = Dss::new();
    let deck = vec![
        "clear".to_string(),
        format!("new circuit.{name} basekv=12.47 phases=3 bus1=sourcebus"),
        "new linecode.lc nphases=3 r1=0.15 x1=0.35 r0=0.45 x0=1.05 c1=0 c0=0 units=km".to_string(),
        "new line.l1 bus1=sourcebus bus2=m1 linecode=lc length=0.1 units=km".to_string(),
        // 3-phase transformer — NPhases=3, but the graph must weight it 1.
        "new transformer.tx phases=3 windings=2 buses=[m1 m2] conns=[wye wye] \
         kvs=[12.47 4.16] kvas=[5000 5000] xhl=6"
            .to_string(),
        "new line.l2 bus1=m2 bus2=m3 linecode=lc length=0.1 units=km".to_string(),
        // Parallel pair between m3 and m4 → deduped to one edge.
        "new line.l3a bus1=m3 bus2=m4 linecode=lc length=0.1 units=km".to_string(),
        "new line.l3b bus1=m3 bus2=m4 linecode=lc length=0.1 units=km".to_string(),
        "new load.load1 bus1=m4 phases=3 kv=4.16 kw=100 pf=0.95 model=1".to_string(),
        "set voltagebases=[12.47 4.16]".to_string(),
        "calcv".to_string(),
        "solve".to_string(),
    ];
    build(&mut dss, &deck, &scratch);

    dss.command("set Num_SubCircuits=2");
    dss.command("Tear_Circuit");
    assert!(dss.errors().is_empty(), "tear errors: {:?}", dss.errors());

    let (_n_cols, num_edges, weights) = read_graph(&scratch, name);

    // Transformer-weight-1 rule fired (present alongside 3-phase Line weights).
    assert!(
        weights.contains(&1),
        "expected a Transformer edge of weight 1, weights={weights:?}"
    );
    assert!(
        weights.contains(&3),
        "expected 3-phase Line edges of weight 3, weights={weights:?}"
    );
    // No other weights (all branches are either 3-phase Lines or the xfmr).
    assert!(
        weights.iter().all(|&w| w == 1 || w == 3),
        "unexpected edge weight, weights={weights:?}"
    );

    // Parallel dedup: 5 branch objects (l1, tx, l2, l3a, l3b) collapse to 4
    // distinct edges — l3a/l3b share the (m3,m4) pair. The header edge count is
    // the distinct-edge count, strictly below the 5 raw branches.
    assert_eq!(
        num_edges, 4,
        "expected 4 distinct edges after parallel dedup, got {num_edges}"
    );
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
