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

    // `get ADiakoptics` reflects the flag (default off before init). The real
    // `set ADiakoptics=yes` init is covered by `adiakoptics_init_*` above.
    dss.command("get ADiakoptics");
    assert_eq!(dss.result(), "No", "ADiakoptics off before init");
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
///
/// TODO(WP-AD.3): this pins the tree structurally against our own emission (a
/// self-golden). Cross-checking the two deliberate `Format_SubCircuits`
/// deviations (case-insensitive filter, `New Circuit`-anchored zone cut) against
/// the vendored first-party `Examples/ADiakoptics/ckt24/Torn_Circuit/**`
/// reference is the D9(b) reference-fixture harvest, scheduled for WP-AD.3. The
/// PConn numbers are independently validated by `pconn_sources_match_solved_nodev`.
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

/// Parse the phase-1 `basekv`/`angle` from an emitted `VSource.dss` (the first
/// `Edit Vsource.source …` line).
fn parse_vsource_phase1(path: &Path) -> (f64, f64) {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let line = text.lines().next().unwrap_or("");
    let grab = |key: &str| -> f64 {
        line.split_whitespace()
            .find_map(|t| t.strip_prefix(key))
            .unwrap_or_else(|| panic!("no {key} in {line:?}"))
            .parse::<f64>()
            .unwrap_or_else(|e| panic!("parse {key} in {line:?}: {e}"))
    };
    (grab("basekv="), grab("angle="))
}

/// **Independent numeric cross-check of the PConn boundary sources** (not the
/// self-golden). For every zone, re-derive the point-of-connection bus from the
/// link `Line`'s bus-2 and the boundary voltage from the *solved* `NodeV` (public
/// bus API), then assert the **emitted** `VSource.dss` `basekv`/`angle` match.
/// This catches a wrong-terminal, wrong-bus, angle-sign, or `/1000`-scale error
/// in the PConn capture that the byte-golden alone would freeze in forever.
#[test]
fn pconn_sources_match_solved_nodev() {
    use dss_core::support::complexutil::c_to_polar_deg;
    let scratch = scratch_dir("pconn");
    let mut dss = Dss::new();
    compile_fixture(&mut dss, "midi", &scratch); // compiles + base solve
    dss.command("set Num_SubCircuits=2");
    dss.command("Tear_Circuit");
    assert!(dss.errors().is_empty(), "tear errors: {:?}", dss.errors());

    let (links, pconn): (Vec<String>, Vec<String>) = {
        let ad = &dss.circuit().unwrap().ad;
        (ad.link_branches.clone(), ad.pconn_names.clone())
    };
    assert!(pconn.len() >= 2, "expected >=2 zones, got {}", pconn.len());

    // Independent expectation from the solved NodeV at each pconn bus, phase 1
    // (read before any &mut call so nothing can perturb the operating point).
    let expected: Vec<(f64, f64)> = {
        let ckt = dss.circuit().unwrap();
        pconn
            .iter()
            .map(|bus| {
                let idx = ckt
                    .bus_list
                    .find(bus)
                    .unwrap_or_else(|| panic!("pconn bus {bus:?} not in bus list"));
                let b = &ckt.buses[idx];
                let nref = b.find_idx(1).map(|ni| b.get_ref(ni)).unwrap_or(0);
                let v = ckt.solution.node_v.get(nref).copied().unwrap_or_default();
                let p = c_to_polar_deg(v);
                (p.mag / 1000.0, p.ang)
            })
            .collect()
    };

    // Independent point-of-connection: pconn[i] must equal link[i]'s bus-2.
    for i in 1..links.len() {
        let props = dss
            .element_properties(&links[i])
            .unwrap_or_else(|| panic!("link {} does not resolve", links[i]));
        let bus2 = props
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("bus2"))
            .map(|(_, v)| v.split('.').next().unwrap_or("").to_string())
            .unwrap_or_default();
        assert!(
            pconn[i].eq_ignore_ascii_case(&bus2),
            "zone {i}: point-of-connection {:?} != link {} bus-2 {:?}",
            pconn[i],
            links[i],
            bus2
        );
    }

    // The emitted VSource files must carry those measured voltages.
    let torn = scratch.join("Torn_Circuit");
    for (i, (want_kv, want_ang)) in expected.iter().enumerate() {
        let vfile = if i == 0 {
            torn.join("VSource.dss")
        } else {
            torn.join(format!("zone_{}", i + 1)).join("VSource.dss")
        };
        let (basekv, angle) = parse_vsource_phase1(&vfile);
        // Physical sanity: a ~7.2 kV L-N boundary (rules out a /1 or /1000 slip).
        assert!(
            *want_kv > 6.0 && *want_kv < 8.0,
            "zone {i}: |V|/1000 = {want_kv} not ~7.2 kV L-N"
        );
        // Emitted values are `fmt_g(_, 8)`-rounded → compare at ~8 sig digits.
        assert!(
            (basekv - want_kv).abs() <= 1e-5 * want_kv.max(1.0),
            "zone {i}: emitted basekv {basekv} != solved |V|/1000 {want_kv}"
        );
        assert!(
            (angle - want_ang).abs() <= 1e-4,
            "zone {i}: emitted angle {angle} != solved angle {want_ang}"
        );
    }
}

/// Round trip: each emitted sub-project compiles and solves on our own engine —
/// the interconnected model (full circuit) and every per-zone standalone model.
///
/// TODO(WP-AD.3): this is a *solvability* smoke check (converged + a >1 kV node);
/// full numeric AD↔normal equivalence at the §AD tolerance tier is deferred to
/// WP-AD.3/D7. The boundary-source values themselves are numerically pinned by
/// `pconn_sources_match_solved_nodev` above.
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
    // TODO(WP-AD.5): build our ckt24 `.graph` and diff it against the vendored
    // artifact here. That needs the ckt24 master-prefix compile driver, which
    // lands in WP-AD.5; today this diagnostic only records the vendored side.
    eprintln!(
        "NOTE: a full our-vs-vendored .graph diff requires compiling the ckt24 master \
         (WP-AD.5 driver territory); this diagnostic records the vendored artifact shape. \
         Divergence is expected (D2: METIS 4.0 kmetis vs the 5.2.1 port)."
    );
}

// ===========================================================================
// WP-AD.3 Stage 2 — `ADiakopticsInit` state machine (init + matrices + options
// + get_Statistics). Rust-only (plan D8). The per-iteration AD solve is Stage 2b
// (see `exec/diakoptics/solve.rs`); these gates validate the init machine
// end-to-end: ClearAll + recompile the interconnected coordinator + build the
// child zones + open link branches + Contours/ZLL/ZCC/Y4 on the REAL torn
// coordinator, with the D1 invariants recomputed in-test.
// ===========================================================================

use num_complex::Complex64;

/// Dense row-major `n*n` copy of a (row,col,value) triple list.
fn ad_dense(cdata: &[(i32, i32, Complex64)], n: usize) -> Vec<Complex64> {
    ad_dense_rc(cdata, n, n)
}

/// Row-major `nrows×ncols` dense copy of a sparse `(row,col,value)` list.
fn ad_dense_rc(cdata: &[(i32, i32, Complex64)], nrows: usize, ncols: usize) -> Vec<Complex64> {
    let mut d = vec![Complex64::new(0.0, 0.0); nrows * ncols];
    for &(r, c, v) in cdata {
        if (r as usize) < nrows && (c as usize) < ncols {
            d[r as usize * ncols + c as usize] = v;
        }
    }
    d
}

/// Run the full A-Diakoptics init on the midi feeder (2 zones); returns the Dss
/// (coordinator = the interconnected model with Contours/ZLL/ZCC/Y4 built).
fn init_midi_ad(scratch: &Path) -> Dss {
    let mut dss = Dss::new();
    compile_fixture(&mut dss, "midi", scratch);
    dss.command("set Num_SubCircuits=2");
    dss.command("set ADiakoptics=yes");
    dss
}

#[test]
fn adiakoptics_init_flips_flag_and_reports_summary() {
    let scratch = scratch_dir("ad3_flag");
    let dss = init_midi_ad(&scratch);
    let ckt = dss.circuit().expect("coordinator circuit");
    assert!(
        ckt.solution.adiakoptics,
        "init failed; summary: {}",
        dss.result()
    );
    assert!(
        ckt.solution.parallel_enabled,
        "parallel_enabled set on success"
    );
    assert!(!ckt.solution.adiak_init, "adiak_init cleared on success");
    let summary = dss.result();
    assert!(
        summary.contains("Sub-Circuits Created"),
        "summary: {summary}"
    );
    assert!(summary.contains("Building Contours"), "summary: {summary}");
    assert!(
        summary.contains("A-Diakoptics initialized"),
        "summary: {summary}"
    );
}

#[test]
fn adiakoptics_init_contours_have_plus_minus_per_column() {
    let scratch = scratch_dir("ad3_contours");
    let dss = init_midi_ad(&scratch);
    let ckt = dss.circuit().unwrap();
    let contours = &ckt.ad.contours;
    assert_eq!(contours.ncols(), 3, "contour columns");
    for col in 0..3 {
        let entries: Vec<_> = contours.cdata.iter().filter(|cd| cd.col == col).collect();
        assert_eq!(entries.len(), 2, "column {col}: two boundary nodes");
        let plus = entries
            .iter()
            .filter(|cd| cd.value == Complex64::new(1.0, 0.0))
            .count();
        let minus = entries
            .iter()
            .filter(|cd| cd.value == Complex64::new(-1.0, 0.0))
            .count();
        assert_eq!((plus, minus), (1, 1), "column {col}: one +1 and one -1");
        assert_ne!(entries[0].row, entries[1].row, "distinct boundary nodes");
    }
}

#[test]
fn adiakoptics_init_zll_is_link_block() {
    // Shape of the real-init ZLL block. Its VALUES are pinned two ways: the
    // inverted-Yprim-self-block equality on a controlled circuit
    // (`matrices::tests::zll_block_is_inverted_link_yprim_self_block`), and the
    // real-init ZCC re-derivation in `adiakoptics_init_y4_inverts_zcc` (ZLL is an
    // additive summand of ZCC, so a wrong real-init ZLL fails it).
    let scratch = scratch_dir("ad3_zll");
    let dss = init_midi_ad(&scratch);
    let ckt = dss.circuit().unwrap();
    let zll = &ckt.ad.zll;
    assert_eq!(zll.nzero(), 9, "3x3 dense ZLL block");
    for cd in &zll.cdata {
        assert!(
            cd.row < 3 && cd.col < 3,
            "entry inside the single link block"
        );
    }
    assert!(
        zll.cdata.iter().any(|c| c.value.norm() > 1e-9),
        "ZLL block is non-zero"
    );
}

#[test]
fn adiakoptics_init_y4_inverts_zcc() {
    let scratch = scratch_dir("ad3_y4");
    let dss = init_midi_ad(&scratch);
    let ckt = dss.circuit().unwrap();
    let nn = ckt.num_nodes; // Contours / ZCT rows
    let n = ckt.ad.zcc.nrows() as usize;
    assert_eq!(n, 3, "ZCC order = real-links x 3");
    assert_eq!(ckt.ad.y4.nrows() as usize, n, "Y4 same order");
    // The torn-Y per-column solve populates ZCT (a full response, not a
    // degenerate empty), so ZCC carries the CᵀZCT coupling, not just ZLL.
    assert!(ckt.ad.zct.nzero() > 0, "ZCT populated by the torn-Y solve");
    assert_eq!(ckt.ad.zcc.nzero(), 9, "ZCC is a full 3×3");

    // D1 invariant (a) on the REAL init pipeline (not just the unit fixture):
    // ZCC re-derived INDEPENDENTLY of the builder's transpose/multiply/add as a
    // dense `Contoursᵀ·ZCT + ZLL`. A wrong link resolution, a wrong post-close
    // rebuild ZLL, or a bad ZCT coupling here breaks this even though the shape
    // checks and the circular `Y4·ZCC≈I` would still pass.
    let c_dense = ad_dense_rc(
        &ckt.ad
            .contours
            .cdata
            .iter()
            .map(|c| (c.row, c.col, c.value))
            .collect::<Vec<_>>(),
        nn,
        n,
    );
    let zct_dense = ad_dense_rc(
        &ckt.ad
            .zct
            .cdata
            .iter()
            .map(|c| (c.row, c.col, c.value))
            .collect::<Vec<_>>(),
        nn,
        n,
    );
    let zll_dense = ad_dense(
        &ckt.ad
            .zll
            .cdata
            .iter()
            .map(|c| (c.row, c.col, c.value))
            .collect::<Vec<_>>(),
        n,
    );
    let zcc_dense = ad_dense(
        &ckt.ad
            .zcc
            .cdata
            .iter()
            .map(|c| (c.row, c.col, c.value))
            .collect::<Vec<_>>(),
        n,
    );
    for i in 0..n {
        for j in 0..n {
            let mut s = Complex64::new(0.0, 0.0);
            for k in 0..nn {
                s += c_dense[k * n + i] * zct_dense[k * n + j];
            }
            s += zll_dense[i * n + j];
            let got = zcc_dense[i * n + j];
            assert!(
                (got - s).norm() < 1e-7,
                "ZCC[{i},{j}] builder={got} vs CᵀZCT+ZLL={s}"
            );
        }
    }

    let y4 = ad_dense(
        &ckt.ad
            .y4
            .cdata
            .iter()
            .map(|c| (c.row, c.col, c.value))
            .collect::<Vec<_>>(),
        n,
    );
    let zcc = ad_dense(
        &ckt.ad
            .zcc
            .cdata
            .iter()
            .map(|c| (c.row, c.col, c.value))
            .collect::<Vec<_>>(),
        n,
    );
    for i in 0..n {
        for j in 0..n {
            let mut s = Complex64::new(0.0, 0.0);
            for k in 0..n {
                s += y4[i * n + k] * zcc[k * n + j];
            }
            let want = if i == j { 1.0 } else { 0.0 };
            assert!(
                (s.re - want).abs() < 1e-6 && s.im.abs() < 1e-6,
                "Y4.ZCC[{i},{j}] = {s} (want {want})"
            );
        }
    }
    for cd in &ckt.ad.y4.cdata {
        assert_ne!(cd.value.re, 0.0, "Y4 stores only nonzero-real entries (D5)");
    }
}

#[test]
fn adiakoptics_get_flag_and_stats_deterministic() {
    let scratch = scratch_dir("ad3_stats");
    let mut dss = init_midi_ad(&scratch);
    // Capture the init summary's statistics BEFORE any further command overwrites
    // `GlobalResult`.
    let stats1 = summary_stats(&dss);
    dss.command("get ADiakoptics");
    assert_eq!(dss.result(), "Yes", "get ADiakoptics after init");

    let scratch2 = scratch_dir("ad3_stats2");
    let dss2 = init_midi_ad(&scratch2);
    let stats2 = summary_stats(&dss2);
    assert_eq!(stats1, stats2, "statistics deterministic across runs");

    // Value golden (plan gate 5): the fixed 2-zone midi partition yields fixed
    // node counts → fixed reduction/imbalance numbers with `floattostrf(ffgeneral,
    // 4)` (via `fmt_g`) formatting. Part II has no oracle (§0.2), so this pins the
    // engine's own deterministic 1:1 output as a committed characterization
    // constant — a regression in the get_Statistics computation OR the number
    // formatting now fails here, where the run-to-run determinism check alone
    // (both runs regress identically) would pass. Machine-independent given ≥4
    // cores (D6): the in-process METIS partition is deterministic.
    assert_eq!(
        stats1,
        "Circuit reduction    (%): 46.34\n\
         Max imbalance       (%): 13.64\n\
         Average imbalance(%): 6.818",
        "get_Statistics value golden (midi, 2 zones)"
    );
}

#[test]
fn adiakoptics_set_no_clears_flag_only() {
    let scratch = scratch_dir("ad3_clear");
    let mut dss = init_midi_ad(&scratch);
    assert!(dss.circuit().unwrap().solution.adiakoptics);
    dss.command("set ADiakoptics=no");
    assert!(!dss.circuit().unwrap().solution.adiakoptics, "flag cleared");
    assert_eq!(
        dss.circuit().unwrap().ad.contours.ncols(),
        3,
        "matrices retained (clear = flag only)"
    );
}

#[test]
fn adiakoptics_init_without_prior_solve_fails() {
    let scratch = scratch_dir("ad3_nosolve");
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", fixture("midi").display()));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("set Num_SubCircuits=2");
    dss.command("set ADiakoptics=yes");
    assert!(
        !dss.circuit().unwrap().solution.adiakoptics,
        "init must fail without a converged prior solve"
    );
    assert!(
        dss.result().contains("errors found"),
        "summary: {}",
        dss.result()
    );
}

/// The deterministic partitioning-statistics lines of the init summary.
fn summary_stats(dss: &Dss) -> String {
    dss.result()
        .lines()
        .filter(|l| {
            l.contains("Circuit reduction")
                || l.contains("Max imbalance")
                || l.contains("Average imbalance")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ===========================================================================
// WP-AD.3 Stage 3 — AD matrix exports 58-61 (ExportResults.pas:3541-3627).
// Silent no-op unless Solution.ADiakoptics; format = compressed-coordinate CSV.
// ===========================================================================

#[test]
fn export_zll_matches_matrix() {
    use dss_core::util::float_to_str;
    let scratch = scratch_dir("ad3_exp_zll");
    let mut dss = init_midi_ad(&scratch);
    dss.command("export ZLL");
    let path = dss.last_result_file().to_string();
    assert!(!path.is_empty(), "ZLL export wrote a file");
    let text = std::fs::read_to_string(&path).expect("read ZLL.csv");
    let mut lines = text.lines();
    assert_eq!(
        lines.next().unwrap(),
        "Row,Col,Value(Real), Value(Imag)",
        "ZLL header"
    );
    let data: Vec<&str> = lines.filter(|l| !l.trim().is_empty()).collect();
    let ckt = dss.circuit().unwrap();
    let cdata = &ckt.ad.zll.cdata;
    // One data line per stored non-zero (9 for a single 3x3 link block).
    assert_eq!(data.len(), cdata.len(), "one data line per stored non-zero");
    // VALUE-level: each emitted line is `row,col,float_to_str(re),float_to_str(im)`
    // of the matching matrix entry in storage order — catches a Re/Im column swap
    // or a wrong float rendering that a field-count check silently passes.
    for (l, cd) in data.iter().zip(cdata) {
        let want = format!(
            "{},{},{},{}",
            cd.row,
            cd.col,
            float_to_str(cd.value.re),
            float_to_str(cd.value.im)
        );
        assert_eq!(*l, want, "ZLL export line equals the matrix entry");
    }
}

#[test]
fn export_contours_is_real_only() {
    let scratch = scratch_dir("ad3_exp_c");
    let mut dss = init_midi_ad(&scratch);
    dss.command("export Contours");
    let path = dss.last_result_file();
    assert!(
        path.ends_with("C.csv"),
        "Contours default file is C.csv: {path}"
    );
    let text = std::fs::read_to_string(path).expect("read C.csv");
    let mut lines = text.lines();
    assert_eq!(lines.next().unwrap(), "Row,Col,Value", "Contours header");
    let data: Vec<&str> = lines.filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(data.len(), 6, "6 contour entries (3 columns x +-1)");
    for l in &data {
        // real-only: `row,col,value` (3 fields), value is +-1.
        assert_eq!(l.split(',').count(), 3, "Contours line has 3 fields: {l}");
        let v: &str = l.split(',').nth(2).unwrap();
        assert!(v == "1" || v == "-1", "contour value is +-1: {v}");
    }
}

#[test]
fn export_zcc_and_y4_match_matrix() {
    use dss_core::util::float_to_str;
    let scratch = scratch_dir("ad3_exp_zccy4");
    let mut dss = init_midi_ad(&scratch);
    for kw in ["ZCC", "Y4"] {
        dss.command(&format!("export {kw}"));
        let path = dss.last_result_file().to_string();
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {kw}: {e}"));
        let mut lines = text.lines();
        assert_eq!(
            lines.next().unwrap(),
            "Row,Col,Value(Real), Value(Imag)",
            "{kw} header"
        );
        let data: Vec<&str> = lines.filter(|l| !l.trim().is_empty()).collect();
        let ckt = dss.circuit().unwrap();
        let cdata = if kw == "ZCC" {
            &ckt.ad.zcc.cdata
        } else {
            &ckt.ad.y4.cdata
        };
        assert_eq!(data.len(), 9, "{kw} has 9 entries (3x3)");
        assert_eq!(
            data.len(),
            cdata.len(),
            "{kw}: one line per stored non-zero"
        );
        // VALUE-level (see `export_zll_matches_matrix`): pin the emitted floats
        // against the matrix entries, not merely the 4-field shape.
        for (l, cd) in data.iter().zip(cdata) {
            let want = format!(
                "{},{},{},{}",
                cd.row,
                cd.col,
                float_to_str(cd.value.re),
                float_to_str(cd.value.im)
            );
            assert_eq!(*l, want, "{kw} export line equals the matrix entry");
        }
    }
}

#[test]
fn export_ad_matrices_write_no_file_but_set_lastfile_without_init() {
    // Compile + solve but do NOT init A-Diakoptics. Pascal 1:1 (ExportResults.pas
    // :3546 body gated by `if ADiakoptics` + ExportOptions.pas:503-507 tail run
    // UNCONDITIONALLY): the export writes NO file and leaves GlobalResult
    // untouched, but `SetLastResultFile(FileName)` + `@lastexportfile` still point
    // the executive at the (never-created) default path.
    let scratch = scratch_dir("ad3_exp_noop");
    let mut dss = Dss::new();
    compile_fixture(&mut dss, "midi", &scratch);
    assert!(!dss.circuit().unwrap().solution.adiakoptics);
    assert_eq!(dss.last_result_file(), "", "no report written yet");
    for (kw, file) in [
        ("ZLL", "ZLL.csv"),
        ("ZCC", "ZCC.csv"),
        ("Contours", "C.csv"),
        ("Y4", "Y4.csv"),
    ] {
        dss.command(&format!("export {kw}"));
        let p = dss.last_result_file().to_string();
        assert!(
            p.ends_with(file),
            "export {kw} points LastResultFile at {file} (Pascal tail): {p}"
        );
        assert!(
            !std::path::Path::new(&p).exists(),
            "export {kw} writes NO file when ADiakoptics is false: {p}"
        );
        // GlobalResult is cleared at command start (Pascal `GlobalResult := ''`)
        // and the gated-off export body never sets it.
        assert_eq!(
            dss.result(),
            "",
            "export {kw} leaves GlobalResult empty when ADiakoptics is false"
        );
    }
}

// ---------------------------------------------------------------------------
// WP-AD.3 Stage 2b/Stage 4 — the A-Diakoptics solve + D7 equivalence gate.
// ---------------------------------------------------------------------------
mod ad_solve_gate {
    use super::*;
    use num_complex::Complex64;
    use std::collections::HashMap;

    /// Node-name → complex voltage for the (possibly reordered) coordinator.
    fn node_voltages(dss: &Dss) -> HashMap<String, Complex64> {
        let ckt = dss.circuit().expect("circuit");
        (1..=ckt.num_nodes)
            .map(|i| (ckt.node_name(i), ckt.solution.node_v[i]))
            .collect()
    }

    fn solve_normal(fixture_name: &str, tol: f64) -> HashMap<String, Complex64> {
        let scratch = scratch_dir("d7norm");
        let mut dss = Dss::new();
        compile_fixture(&mut dss, fixture_name, &scratch);
        dss.command("set controlmode=off");
        dss.command(&format!("set tolerance={tol}"));
        dss.command("solve mode=snap");
        assert!(dss.errors().is_empty(), "normal errors: {:?}", dss.errors());
        node_voltages(&dss)
    }

    fn solve_ad(fixture_name: &str, num_sub: i32, tol: f64) -> HashMap<String, Complex64> {
        let scratch = scratch_dir("d7ad");
        let mut dss = Dss::new();
        compile_fixture(&mut dss, fixture_name, &scratch);
        dss.command("set controlmode=off");
        dss.command(&format!("set tolerance={tol}"));
        dss.command("solve mode=snap");
        dss.command(&format!("set Num_SubCircuits={num_sub}"));
        dss.command("set ADiakoptics=True");
        assert!(
            dss.circuit().unwrap().solution.adiakoptics,
            "AD init failed: {}",
            dss.result()
        );
        dss.command(&format!("set tolerance={tol}"));
        dss.command("solve mode=snap");
        assert!(
            dss.errors().is_empty(),
            "AD solve errors: {:?}",
            dss.errors()
        );
        node_voltages(&dss)
    }

    fn max_rel_gap(
        a: &HashMap<String, Complex64>,
        b: &HashMap<String, Complex64>,
    ) -> (f64, String) {
        let mut worst = 0.0;
        let mut wn = String::new();
        for (name, va) in a {
            if let Some(vb) = b.get(name) {
                let dv = (va - vb).norm();
                let base = va.norm();
                let rel = if base > 1e-6 { dv / base } else { dv };
                if rel > worst {
                    worst = rel;
                    wn = name.clone();
                }
            }
        }
        (worst, wn)
    }

    #[test]
    fn midi_snapshot_matches_normal() {
        // Method floor 3.21e-5 (r3723 oracle: 3.25e-5); D7 tier = 4× ≈ 1.3e-4.
        let vn = solve_normal("midi", 1e-4);
        let va = solve_ad("midi", 2, 1e-4);
        let (gap, node) = max_rel_gap(&vn, &va);
        println!("midi AD-vs-normal gap = {gap:.4e} @ {node}");
        assert!(
            gap < 1.3e-4,
            "midi AD-vs-normal gap {gap:.3e} @ {node} exceeds the D7 tier"
        );
        assert!(
            gap > 1.0e-6,
            "gap {gap:.3e} suspiciously small — the AD solve may be a normal-solve passthrough"
        );
    }

    #[test]
    fn midi_d7_gap_stable_under_tighten() {
        let g_loose = max_rel_gap(&solve_normal("midi", 1e-4), &solve_ad("midi", 2, 1e-4)).0;
        let g_tight = max_rel_gap(&solve_normal("midi", 1e-10), &solve_ad("midi", 2, 1e-10)).0;
        println!("midi D7 tighten: loose={g_loose:.4e} tight={g_tight:.4e}");
        let ratio = g_tight / g_loose;
        assert!(
            (0.5..2.0).contains(&ratio),
            "midi AD-vs-normal gap not stable under tighten (oracle: bit-stable): \
             loose={g_loose:.3e} tight={g_tight:.3e} ratio={ratio:.3}"
        );
    }

    #[test]
    fn midi_three_zones_snapshot_matches_normal() {
        // Two reference-free zones (actors 3 & 4): exercises multi-link Contours.
        let vn = solve_normal("midi", 1e-4);
        let va = solve_ad("midi", 3, 1e-4);
        let (gap, node) = max_rel_gap(&vn, &va);
        println!("midi 3-zone AD-vs-normal gap = {gap:.4e} @ {node}");
        // Floor 3.21e-5; D7 tier = 4× ≈ 1.3e-4.
        assert!(gap < 1.3e-4, "midi 3-zone gap {gap:.3e} @ {node}");
        assert!(gap > 1.0e-7, "3-zone gap {gap:.3e} suspiciously small");
    }

    #[test]
    fn macro_snapshot_matches_normal() {
        // ~200-bus feeder; method floor 1.319e-4 (r3723 oracle, same main92 cut:
        // 1.318e-4); D7 tier = 4× ≈ 5.3e-4.
        let vn = solve_normal("macro", 1e-4);
        let va = solve_ad("macro", 2, 1e-4);
        let (gap, node) = max_rel_gap(&vn, &va);
        println!("macro AD-vs-normal gap = {gap:.4e} @ {node}");
        assert!(gap < 5.3e-4, "macro AD-vs-normal gap {gap:.3e} @ {node}");
        assert!(gap > 1.0e-7, "macro gap {gap:.3e} suspiciously small");
    }

    #[test]
    fn macro_d7_gap_stable_under_tighten() {
        // The bug that inflated the deep-zone gap 26× lived here (long
        // reference-free zone): pin the tolerance-stability on macro too.
        let g_loose = max_rel_gap(&solve_normal("macro", 1e-4), &solve_ad("macro", 2, 1e-4)).0;
        let g_tight = max_rel_gap(&solve_normal("macro", 1e-10), &solve_ad("macro", 2, 1e-10)).0;
        println!("macro D7 tighten: loose={g_loose:.4e} tight={g_tight:.4e}");
        let ratio = g_tight / g_loose;
        assert!(
            (0.5..2.0).contains(&ratio),
            "macro AD-vs-normal gap not stable under tighten: \
             loose={g_loose:.3e} tight={g_tight:.3e} ratio={ratio:.3}"
        );
    }
}
