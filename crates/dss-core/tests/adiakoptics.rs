//! A-Diakoptics circuit-tearing gates (WP-AD.2 Stage B,
//! `DIAKOPTICS_PSTCALC_PLAN.md` §3). Rust-only (plan D8 — Part II has no oracle
//! leg by design, §0.2). These drive committed radial **3-phase** fixture feeders
//! (`tests/data/adiakoptics/{midi,macro}.dss`, reused by WP-AD.3/AD.4) through the
//! script interface — `set Num_SubCircuits=N; Tear_Circuit` — and assert the
//! tearing + torn-file-emission invariants:
//!  - the sub-circuit count and `GlobalResult` (Diakoptics.pas:526);
//!  - the link branches are **real 3-phase `Line` elements** (resolved through the
//!    engine, not a name-prefix guess);
//!  - the emitted `.part.N` partition is balanced, has N zones, and every zone is
//!    **connected** (recomputed from the `.graph` adjacency + `.part.N` labels);
//!  - the emitted `Torn_Circuit/` tree matches a committed byte-stable golden
//!    (`Format_SubCircuits` + the D2 partitioner) and every sub-project
//!    (interconnected + per-zone) **compiles and solves**;
//!  - honest errors: tearing before a solve, and a non-`Line` manual link.
//!
//! Every deck fixes `Num_SubCircuits` explicitly (plan D6 — no test depends on
//! `available_parallelism`). Torn artifacts land in per-test temp dirs (never the
//! source tree). Regenerate the golden with `DSS_REGEN_AD_GOLDEN=1`.

use dss_core::exec::Dss;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// Path to a committed fixture deck.
fn fixture(name: &str) -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "data",
        "adiakoptics",
        &format!("{name}.dss"),
    ]
    .iter()
    .collect()
}

fn golden_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "adiakoptics",
    ]
    .iter()
    .collect()
}

fn scratch_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "dss_ad_{tag}_{}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
    d
}

/// Compile a committed fixture, redirect the datapath to `scratch` (so all torn
/// artifacts land there, never the read-only fixture dir), and run its base
/// solve. Returns the circuit's case name (used to locate `<name>_.graph`).
fn compile_fixture(dss: &mut Dss, fixture_name: &str, scratch: &Path) -> String {
    dss.command(&format!("compile \"{}\"", fixture(fixture_name).display()));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "compile/solve errors: {:?}",
        dss.errors()
    );
    // The fixtures name the circuit `ad<fixture>` (admidi / admacro).
    format!("ad{fixture_name}")
}

/// Read the emitted `<name>_.graph.part.<k>` partition file as per-vertex labels.
fn read_part(scratch: &Path, name: &str, k: i32) -> Vec<i32> {
    let p = scratch.join(format!("{name}_.graph.part.{k}"));
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().parse::<i32>().unwrap())
        .collect()
}

/// Read the emitted `<name>_.graph` as `(n_cols, num_edges, weights)`.
fn read_graph(scratch: &Path, name: &str) -> (i32, i32, Vec<i32>) {
    let p = scratch.join(format!("{name}_.graph"));
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let mut lines = text.lines();
    let header: Vec<i32> = lines
        .next()
        .unwrap()
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect();
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

/// Rebuild the **full undirected** graph adjacency from the emitted `.graph`
/// (the OpenDSS writer drops column 0's line — recovered here by symmetry: any
/// `0–x` edge appears in vertex `x`'s line as neighbor `0`). Adjacency line `li`
/// (0-based) is vertex `li + 1`; tokens are `neigh weight` pairs (0-based
/// `neigh`).
fn graph_adjacency(scratch: &Path, name: &str, n_cols: usize) -> Vec<Vec<usize>> {
    let p = scratch.join(format!("{name}_.graph"));
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let mut adj = vec![Vec::new(); n_cols];
    for (li, line) in text.lines().skip(1).enumerate() {
        let v = li + 1; // column 0's line is dropped by the OpenDSS writer
        if v >= n_cols {
            continue;
        }
        let toks: Vec<usize> = line
            .split_whitespace()
            .map(|t| t.parse::<i64>().unwrap() as usize)
            .collect();
        for pair in toks.chunks(2) {
            if pair.len() == 2 {
                let neigh = pair[0];
                if neigh < n_cols {
                    adj[v].push(neigh);
                    adj[neigh].push(v); // symmetrize (recovers column 0)
                }
            }
        }
    }
    adj
}

/// Assert every zone in a `.part.N` labeling is a **connected** subgraph of the
/// `.graph` adjacency (the plan's "zones connected" tearing-validity invariant).
fn assert_zones_connected(scratch: &Path, name: &str, k: i32) {
    let labels = read_part(scratch, name, k);
    let n = labels.len();
    let adj = graph_adjacency(scratch, name, n);
    let distinct: std::collections::BTreeSet<i32> = labels.iter().copied().collect();
    for &zone in &distinct {
        let members: Vec<usize> = (0..n).filter(|&v| labels[v] == zone).collect();
        if members.len() <= 1 {
            continue; // a singleton zone is trivially connected
        }
        // BFS within the zone from members[0].
        let mut seen = vec![false; n];
        let mut q = VecDeque::new();
        seen[members[0]] = true;
        q.push_back(members[0]);
        let mut reached = 1usize;
        while let Some(u) = q.pop_front() {
            for &w in &adj[u] {
                if labels[w] == zone && !seen[w] {
                    seen[w] = true;
                    reached += 1;
                    q.push_back(w);
                }
            }
        }
        assert_eq!(
            reached,
            members.len(),
            "zone {zone} is not connected ({reached}/{} vertices reachable)",
            members.len()
        );
    }
}

/// The non-empty link branches from `get LinkBranches`, resolved and asserted to
/// be **real 3-phase `Line` elements** via the engine (not a string-prefix
/// check): each `Class.name` must resolve, its class must be `Line`, and its
/// `phases` property must be `3`.
fn assert_links_are_3ph_lines(dss: &mut Dss) {
    let links: Vec<String> = {
        let ad = &dss.circuit().unwrap().ad;
        ad.link_branches
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect()
    };
    assert!(!links.is_empty(), "expected at least one link branch");
    for link in links {
        let (class, _) = link.split_once('.').unwrap_or(("", &link));
        assert!(
            class.eq_ignore_ascii_case("line"),
            "link branch {link:?} is not a Line (class {class:?})"
        );
        let props = dss
            .element_properties(&link)
            .unwrap_or_else(|| panic!("link {link:?} does not resolve to a real element"));
        let phases = props
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("phases"))
            .map(|(_, v)| v.trim().to_string())
            .unwrap_or_default();
        assert_eq!(phases, "3", "link branch {link:?} is not 3-phase");
    }
}

#[test]
fn tear_midi_two_zones_reports_and_partitions() {
    let scratch = scratch_dir("midi2");
    let mut dss = Dss::new();
    let name = compile_fixture(&mut dss, "midi", &scratch);

    dss.command("set Num_SubCircuits=2");
    dss.command("Tear_Circuit");
    assert!(dss.errors().is_empty(), "tear errors: {:?}", dss.errors());
    assert_eq!(dss.result(), "Sub-Circuits Created: 2");

    // Link branches are real 3-phase Lines (engine-resolved, not a prefix check).
    assert_links_are_3ph_lines(&mut dss);

    // Partition file: exactly 2 zones, balanced, and each zone connected.
    let part = read_part(&scratch, &name, 2);
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
    assert_zones_connected(&scratch, &name, 2);
}

#[test]
fn tear_midi_three_zones() {
    let scratch = scratch_dir("midi3");
    let mut dss = Dss::new();
    let name = compile_fixture(&mut dss, "midi", &scratch);

    dss.command("set Num_SubCircuits=3");
    dss.command("Tear_Circuit");
    assert!(dss.errors().is_empty(), "tear errors: {:?}", dss.errors());
    assert_eq!(dss.result(), "Sub-Circuits Created: 3");

    let part = read_part(&scratch, &name, 3);
    let mut labels = part.clone();
    labels.sort_unstable();
    labels.dedup();
    assert_eq!(labels, vec![0, 1, 2], "expected 3 zone labels");
    let n = part.len();
    for z in 0..3 {
        let sz = part.iter().filter(|&&l| l == z).count();
        assert!(sz * 6 >= n, "zone {z} too small: {sz} of {n}");
    }
    assert_zones_connected(&scratch, &name, 3);
}

#[test]
fn tear_macro_two_zones() {
    let scratch = scratch_dir("macro2");
    let mut dss = Dss::new();
    let name = compile_fixture(&mut dss, "macro", &scratch);

    dss.command("set Num_SubCircuits=2");
    dss.command("Tear_Circuit");
    assert!(dss.errors().is_empty(), "tear errors: {:?}", dss.errors());
    assert_eq!(dss.result(), "Sub-Circuits Created: 2");

    let part = read_part(&scratch, &name, 2);
    assert!(
        part.len() >= 190,
        "expected ~200 vertices, got {}",
        part.len()
    );
    let z0 = part.iter().filter(|&&l| l == 0).count();
    let z1 = part.len() - z0;
    let n = part.len();
    assert!(
        z0 * 4 >= n && z1 * 4 >= n,
        "unbalanced macro partition: {z0} vs {z1} of {n}"
    );
    assert_zones_connected(&scratch, &name, 2);
}

/// The manual-links branch: `set LinkBranches=[...] UseMyLinkBranches=True` drives
/// the cut count. Each user link is a cut, so N link branches yield N+1
/// sub-circuits — the official setter reserves an empty index-0 reference
/// placeholder (ExecOptions.pas:842–844) and `Tear_Circuit` returns
/// `length(Locations) = N+1`. Empirically pinned against the r3723 binary (Oddie
/// bridge): `[line.main10]` → "Sub-Circuits Created: 2", `[line.main5,
/// line.main10]` → 3; `get LinkBranches` drops the placeholder.
#[test]
fn tear_manual_link_branches_use_requested_cuts() {
    {
        let scratch = scratch_dir("manual1");
        let mut dss = Dss::new();
        compile_fixture(&mut dss, "midi", &scratch);

        dss.command("set LinkBranches=[line.main10]");
        dss.command("get LinkBranches");
        assert_eq!(dss.result(), "line.main10");

        dss.command("set UseMyLinkBranches=True");
        dss.command("Tear_Circuit");
        assert!(
            dss.errors().is_empty(),
            "manual tear errors: {:?}",
            dss.errors()
        );
        assert_eq!(dss.result(), "Sub-Circuits Created: 2");
    }
    {
        let scratch = scratch_dir("manual2");
        let mut dss = Dss::new();
        compile_fixture(&mut dss, "midi", &scratch);

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
    }
}

#[test]
fn one_zone_request_is_handled() {
    let scratch = scratch_dir("one");
    let mut dss = Dss::new();
    let name = compile_fixture(&mut dss, "midi", &scratch);

    dss.command("set Num_SubCircuits=1");
    dss.command("Tear_Circuit");
    assert!(dss.errors().is_empty(), "1-zone errors: {:?}", dss.errors());
    assert_eq!(dss.result(), "Sub-Circuits Created: 1");

    let part = read_part(&scratch, &name, 1);
    assert!(part.iter().all(|&l| l == 0), "1-zone: all labels zero");
}

/// `Create_MeTIS_graph` edge weighting + parallel-branch dedup (Circuit.pas:1213).
/// Exercises the two branches the all-Line radial fixtures never hit: the
/// **Transformer-weight-1** rule and the **parallel-branch dedup** loop. Inline
/// (a special topology, not a radial fixture).
#[test]
fn metis_graph_transformer_weight1_and_parallel_dedup() {
    let scratch = scratch_dir("graph");
    let mut dss = Dss::new();
    let deck = [
        "clear",
        "new circuit.teargraph basekv=12.47 phases=3 bus1=sourcebus",
        "new linecode.lc nphases=3 r1=0.15 x1=0.35 r0=0.45 x0=1.05 c1=0 c0=0 units=km",
        "new line.l1 bus1=sourcebus bus2=m1 linecode=lc length=0.1 units=km",
        "new transformer.tx phases=3 windings=2 buses=[m1 m2] conns=[wye wye] \
         kvs=[12.47 4.16] kvas=[5000 5000] xhl=6",
        "new line.l2 bus1=m2 bus2=m3 linecode=lc length=0.1 units=km",
        "new line.l3a bus1=m3 bus2=m4 linecode=lc length=0.1 units=km",
        "new line.l3b bus1=m3 bus2=m4 linecode=lc length=0.1 units=km",
        "new load.load1 bus1=m4 phases=3 kv=4.16 kw=100 pf=0.95 model=1",
        "set voltagebases=[12.47 4.16]",
        "calcv",
        "solve",
    ];
    for cmd in deck {
        dss.command(cmd);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    assert!(dss.errors().is_empty(), "build: {:?}", dss.errors());

    dss.command("set Num_SubCircuits=2");
    dss.command("Tear_Circuit");
    assert!(dss.errors().is_empty(), "tear errors: {:?}", dss.errors());

    let (_n_cols, num_edges, weights) = read_graph(&scratch, "teargraph");
    assert!(
        weights.contains(&1),
        "expected a Transformer edge of weight 1: {weights:?}"
    );
    assert!(
        weights.contains(&3),
        "expected 3-phase Line edges of weight 3: {weights:?}"
    );
    assert!(
        weights.iter().all(|&w| w == 1 || w == 3),
        "unexpected weight: {weights:?}"
    );
    assert_eq!(
        num_edges, 4,
        "expected 4 distinct edges after parallel dedup, got {num_edges}"
    );
}

#[test]
fn set_get_ad_options_roundtrip() {
    let scratch = scratch_dir("opts");
    let mut dss = Dss::new();
    compile_fixture(&mut dss, "midi", &scratch);

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
}

/// `Tear_Circuit` reads `Solution.NodeV` at each point of connection, so it
/// requires a prior successful solve (ckt24 header + spec). Without one, the port
/// errors honestly rather than emitting zero-volt zone sources.
#[test]
fn tear_without_prior_solve_errors() {
    let scratch = scratch_dir("nosolve");
    let mut dss = Dss::new();
    // Compile the fixture but do NOT solve (the fixture ends at calcv).
    dss.command(&format!("compile \"{}\"", fixture("midi").display()));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));

    dss.command("set Num_SubCircuits=2");
    dss.command("Tear_Circuit");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("prior successful solve")),
        "expected a prior-solve error, got {:?}",
        dss.errors()
    );
    assert_eq!(dss.result(), "There was an error when tearing the circuit ");
}

/// Negative path: a manual link naming a **Transformer** (non-`Line`). Official
/// `get_Line_Bus` (Circuit.pas:1167) searches only the Lines list, so a non-Line
/// link is reported "Line ... Not Found" (error 5008) and gets no
/// point-of-connection bus — the honest behavior (the ZLL 3-phase-Line cut
/// constraint is otherwise enforced downstream at AD init, D5). The tear itself
/// does not hard-fail at this level (matching upstream — the graph weights bias
/// auto-tear against transformer cuts, but manual mode trusts the user).
#[test]
fn manual_transformer_link_reports_line_not_found() {
    let scratch = scratch_dir("xflink");
    let mut dss = Dss::new();
    let deck = [
        "clear",
        "new circuit.xflink basekv=12.47 phases=3 bus1=sourcebus",
        "new linecode.lc nphases=3 r1=0.15 x1=0.35 r0=0.45 x0=1.05 c1=0 c0=0 units=km",
        "new line.l1 bus1=sourcebus bus2=m1 linecode=lc length=0.1 units=km",
        "new transformer.tx phases=3 windings=2 buses=[m1 m2] conns=[wye wye] \
         kvs=[12.47 4.16] kvas=[5000 5000] xhl=6",
        "new line.l2 bus1=m2 bus2=m3 linecode=lc length=0.1 units=km",
        "new load.ld bus1=m3 phases=3 kv=4.16 kw=100 pf=0.95 model=1",
        "set voltagebases=[12.47 4.16]",
        "calcv",
        "solve",
    ];
    for cmd in deck {
        dss.command(cmd);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    assert!(dss.errors().is_empty(), "build: {:?}", dss.errors());

    dss.command("set LinkBranches=[transformer.tx]");
    dss.command("set UseMyLinkBranches=True");
    dss.command("Tear_Circuit");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Not Found in Active Circuit")),
        "expected a 'Line not found' error for the transformer link, got {:?}",
        dss.errors()
    );
    // The transformer is recorded as the (invalid) link; its point of connection
    // is empty because `get_Line_Bus` found no Line.
    let ad = &dss.circuit().unwrap().ad;
    assert_eq!(
        ad.link_branches.get(1).map(String::as_str),
        Some("Transformer.tx")
    );
    assert_eq!(ad.pconn_names.get(1).map(String::as_str), Some(""));
}

// ---------------------------------------------------------------------------
// Torn-file emission gates: committed golden + round-trip compile/solve.
// ---------------------------------------------------------------------------

/// Serialize the emitted `Torn_Circuit/` tree to a single deterministic string:
/// every file as `=== <relpath> ===\n<content>`, sorted by relpath, with `\`
/// path separators normalized to `/` and CRLF→LF, so the golden is stable across
/// runs/platforms.
fn serialize_tree(root: &Path) -> String {
    let mut files: Vec<(String, String)> = Vec::new();
    fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, out);
            } else {
                let rel = p
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                let content = std::fs::read_to_string(&p)
                    .unwrap_or_default()
                    .replace("\r\n", "\n");
                out.push((rel, content));
            }
        }
    }
    walk(root, root, &mut files);
    files.sort();
    let mut s = String::new();
    for (rel, content) in files {
        s.push_str(&format!("=== {rel} ===\n{content}"));
        if !content.ends_with('\n') {
            s.push('\n');
        }
    }
    s
}

/// Committed byte-stable golden of the emitted `Torn_Circuit/` tree for the midi
/// feeder (2 zones, fixed partition, deterministic `%.8g`-class formatting). This
/// self-golden pins the D2 partitioner + `Format_SubCircuits` (the
/// `Master_Interconnected.dss` filtering rules, per-zone `Master.dss`, and per-zone
/// `VSource.dss` measured values). Regenerate with `DSS_REGEN_AD_GOLDEN=1`.
#[test]
fn torn_tree_matches_golden() {
    let scratch = scratch_dir("golden");
    let mut dss = Dss::new();
    compile_fixture(&mut dss, "midi", &scratch);
    dss.command("set Num_SubCircuits=2");
    dss.command("Tear_Circuit");
    assert!(dss.errors().is_empty(), "tear errors: {:?}", dss.errors());

    let torn = scratch.join("Torn_Circuit");
    let actual = serialize_tree(&torn);

    let golden_path = golden_dir().join("midi_torn_tree.txt");
    if std::env::var("DSS_REGEN_AD_GOLDEN").is_ok() {
        std::fs::create_dir_all(golden_dir()).unwrap();
        std::fs::write(&golden_path, &actual).unwrap();
        eprintln!("regenerated {}", golden_path.display());
        return;
    }
    let expected = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| {
            panic!(
                "read golden {} (regen with DSS_REGEN_AD_GOLDEN=1): {e}",
                golden_path.display()
            )
        })
        .replace("\r\n", "\n");
    assert_eq!(
        actual, expected,
        "emitted Torn_Circuit tree diverged from the committed golden"
    );
}

/// Round trip: each emitted sub-project compiles and solves on our own engine —
/// the interconnected model (full circuit) and every per-zone standalone model.
#[test]
fn torn_tree_roundtrip_solves() {
    let scratch = scratch_dir("roundtrip");
    let mut dss = Dss::new();
    compile_fixture(&mut dss, "midi", &scratch);
    dss.command("set Num_SubCircuits=3");
    dss.command("Tear_Circuit");
    assert!(dss.errors().is_empty(), "tear errors: {:?}", dss.errors());

    let torn = scratch.join("Torn_Circuit");

    // Interconnected (full model) + zone-1 backbone + each zone_k standalone.
    let mut masters = vec![
        torn.join("Master_Interconnected.dss"),
        torn.join("Master.dss"),
    ];
    for k in 2..=3 {
        masters.push(torn.join(format!("zone_{k}")).join("Master.dss"));
    }

    for master in masters {
        assert!(
            master.exists(),
            "missing sub-project master {}",
            master.display()
        );
        let mut d = Dss::new();
        d.command(&format!("compile \"{}\"", master.display()));
        d.command("solve");
        assert!(
            d.errors().is_empty(),
            "sub-project {} errors: {:?}",
            master.display(),
            d.errors()
        );
        let ckt = d.circuit().expect("compiled circuit");
        assert!(
            ckt.solution.converged_flag,
            "sub-project {} did not converge",
            master.display()
        );
        // A sane solve: at least one node above 1 kV (the ~7.2 kV L-N sources).
        let vmax = ckt
            .solution
            .node_v
            .iter()
            .map(|v| v.norm())
            .fold(0.0f64, f64::max);
        assert!(
            vmax > 1000.0,
            "sub-project {} vmax={vmax} too low",
            master.display()
        );
    }
}

/// Diagnostic (non-gating): build our `ckt24` `.graph` and diff it against the
/// vendored `ckt24_.graph` (official OpenDSS output). Expected to differ (METIS
/// 4.0 kmetis vs our 5.2.1 port, different incidence lineage — plan D2 / the
/// `.graph` writer note); the report is inventory input only. `#[ignore]`d — run
/// with `cargo test -- --ignored ckt24_graph_diagnostic --nocapture`.
#[test]
#[ignore = "diagnostic: reports the ckt24 .graph delta vs the vendored artifact"]
fn ckt24_graph_diagnostic() {
    let vendored: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
        "Version8",
        "Distrib",
        "Examples",
        "ADiakoptics",
        "ckt24",
        "ckt24_.graph",
    ]
    .iter()
    .collect();
    let Ok(vtext) = std::fs::read_to_string(&vendored) else {
        eprintln!(
            "vendored ckt24_.graph not found at {} — skipping",
            vendored.display()
        );
        return;
    };
    let vheader = vtext.lines().next().unwrap_or("");
    eprintln!("vendored ckt24_.graph header: {vheader:?}");
    eprintln!(
        "vendored line count: {}",
        vtext.lines().filter(|l| !l.trim().is_empty()).count()
    );
    eprintln!(
        "NOTE: a full our-vs-vendored .graph diff requires compiling the ckt24 master \
         (WP-AD.5 driver territory); this diagnostic records the vendored artifact shape. \
         Divergence is expected (D2: METIS 4.0 kmetis vs the 5.2.1 port)."
    );
}
