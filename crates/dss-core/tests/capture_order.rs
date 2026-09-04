//! Source-order gate for the live gate's per-step capture
//! (`GOLDEN_REBASE_PLAN.md` §1.1(a), coordinator decision D3).
//!
//! The oracle reads inside one solved step are **not** interchangeable. They
//! partition into three groups:
//!
//! * **A** — cache-aware quantities that go through `ComputeIterminal`
//!   (`Powers`, `TotalPowers`, `Losses`, `PhaseLosses`, and every `Circuit`
//!   aggregate, which is a `Get_Losses`/`Get_Power` over the list it walks);
//! * **B** — reads that fill a scratch buffer via `GetCurrents` (`Currents`,
//!   `CurrentsMagAng`, `SeqCurrents`, `CplxSeqCurrents`, `SeqPowers`,
//!   `Residuals`);
//! * **C** — order-free reads (node voltages, the discrete control state, the
//!   plain `Solution` scalars).
//!
//! Group A must be read before group B. The rule is empirical, not stylistic:
//! it is the harmonics stale-`Iterminal` ordering CLAUDE.md records (a `Powers`
//! read after a `Currents` read returns the previous iteration's current on a
//! Thevenin-DER element), and both transports of the corpus gate must obey it
//! identically or the two channels would capture different numbers from the
//! same engine state.
//!
//! G1.9's circuit aggregates are group A, and they carry a second, independent
//! ordering constraint: each one walks a `TPointerList`
//! (`PDElements`/`Lines`/`Transformers`/`Sources`/`CktElements`) to exhaustion
//! and leaves its cursor at the end, while the discrete capture drives
//! `Transformers.First/Next` of its own. Reading the aggregates before any
//! `First/Next` walk removes that interaction by construction.
//!
//! G1.7's topology surface is group C — the six order-free `Topology` rows
//! (`NumLoops`, `NumIsolatedBranches`, `NumIsolatedLoads`, `AllLoopedPairs`,
//! `AllIsolatedBranches`, `AllIsolatedLoads`) never assign
//! `ActiveCircuit.ActiveCktElement` — but it carries its own two constraints,
//! and both are asserted here:
//!
//! * it is read **strictly last** in the step, after `all_properties`: the
//!   FIRST `Topology` read is what builds the memoized `Branch_List` and
//!   rewrites `Checked`/`IsIsolated`/`BusChecked` on every element (r4133
//!   `Common/Circuit.pas:2932-2950`, `:2937-2947`), and `TopologyI(1)`/`(2)` +
//!   `TopologyV(1)`/`(2)` leave the `PDElements`/`PCElements` cursors at the end
//!   (`DDLL/DTopology.pas:75-94`, `:319-390`);
//! * the other twelve `ITopology` members are **never touched**. Three of them
//!   are the B16 parity gap (`ActiveLevel`, `BranchName`, `ActiveBranch` of
//!   `origin/fastdss:dss/ITopology.py:10-20`) and the other nine are cursor
//!   rows; every one reassigns `ActiveCktElement` (capi
//!   `CAPI/CAPI_Topology.pas:98-110`; r4133 `DTopology.pas:29-54`, `:96-160`,
//!   `:170-186`) and would poison the per-element capture of the same step.
//!
//! A comment can be edited away; this file cannot. It reads the two capture
//! sources and asserts the call order inside their `run_case`, with no engine
//! and no oracle.
//!
//! *(D3 assigns the canonical home of this gate to G1.3a's lane. If that file
//! lands on the merged tree, fold these two cases into it and delete this one —
//! noted as "dedup at merge" in the G1.9 handoff.)*

use std::fs;
use std::path::PathBuf;

/// The six call sites the gate locates inside one transport's `run_case`.
struct Anchors {
    /// Start of the per-step capture function itself.
    run: &'static str,
    /// The G1.9 group-A read.
    aggregates: &'static str,
    /// The per-element capture — the group-B (`Currents`) read.
    elements: &'static str,
    /// The discrete-state capture, which drives `Transformers.First/Next`.
    discrete: &'static str,
    /// The WP8.5b property sweep, read after every established capture.
    properties: &'static str,
    /// G1.7's topology capture — read after `properties`, i.e. last of all.
    topology: &'static str,
}

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Byte offset of `needle` at or after `from`, or an error naming what was
/// looked for — a renamed helper must fail loudly, never turn the comparisons
/// below into a vacuous `0 < 0`.
fn offset_after(hay: &str, needle: &str, from: usize, rel: &str) -> Result<usize, String> {
    hay[from..].find(needle).map(|i| from + i).ok_or_else(|| {
        format!(
            "{rel}: the capture-order gate could not find `{needle}`. If the call was \
             renamed, update this gate — do not delete the ordering it protects \
             (GOLDEN_REBASE_PLAN.md §1.1(a))."
        )
    })
}

/// The whole gate as a pure function of one transport's source text, so the
/// rule itself can be shown to have teeth
/// ([`the_gate_rejects_a_swapped_or_renamed_capture`]).
fn check_order(src: &str, a: &Anchors, rel: &str) -> Result<(), String> {
    let run = offset_after(src, a.run, 0, rel)?;
    let aggregates = offset_after(src, a.aggregates, run, rel)?;
    let elements = offset_after(src, a.elements, run, rel)?;
    let discrete = offset_after(src, a.discrete, run, rel)?;

    if aggregates >= elements {
        return Err(format!(
            "{rel}: the G1.9 aggregates (byte {aggregates}) are read AFTER the element \
             capture (byte {elements}), whose `Currents` read is group B — group A must \
             come first"
        ));
    }
    if aggregates >= discrete {
        return Err(format!(
            "{rel}: the G1.9 aggregates (byte {aggregates}) are read AFTER the discrete \
             capture (byte {discrete}), which drives Transformers.First/Next — the \
             aggregates must precede every First/Next walk"
        ));
    }
    Ok(())
}

/// The pinned dss-python transport (`capi_v0145` channel).
///
/// `capture_all_elements` reaches `Currents` through `gc.capture_element`
/// (`tools/golden/gen_checkpoints.py`, Powers then Currents).
const CAPI: Anchors = Anchors {
    run: "def run_case(",
    aggregates: "capture_aggregates(ckt",
    elements: "capture_all_elements(ckt",
    discrete: "gc.capture_discrete(ckt",
    properties: "capture_all_properties(d, ckt",
    topology: "capture_topology(ckt",
};

/// The EPRI r4133 transport (`r4133` channel), which must capture in the exact
/// same order or the two channels would not be comparing the same reads.
///
/// `capture_all_elements` reaches `Currents` through `Engine::element_pcl`
/// (powers, then currents, then losses).
const R4133: Anchors = Anchors {
    run: "pub fn run_case(",
    aggregates: "capture_aggregates(engine",
    elements: "capture_all_elements(engine",
    discrete: "capture_discrete(engine",
    properties: "capture_all_properties(engine",
    topology: "capture_topology(engine",
};

#[test]
fn capi_capture_reads_the_aggregates_before_any_currents_read() {
    let rel = "tools/oracle/oracle_server.py";
    check_order(&read(rel), &CAPI, rel).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn r4133_capture_reads_the_aggregates_before_any_currents_read() {
    let rel = "crates/dss-epri/src/capture.rs";
    check_order(&read(rel), &R4133, rel).unwrap_or_else(|e| panic!("{e}"));
}

/// Non-vacuity (§1.1(f)): the two gates above pass on the real sources, so this
/// one shows the rule they apply is not trivially satisfiable. Same predicate,
/// synthetic sources — an in-order one is accepted; the same text with the
/// aggregates moved after the element capture, after the discrete capture, or
/// with the aggregate call renamed away, is rejected.
#[test]
fn the_gate_rejects_a_swapped_or_renamed_capture() {
    let a = Anchors {
        run: "fn run_case(",
        aggregates: "capture_aggregates(",
        elements: "capture_all_elements(",
        discrete: "capture_discrete(",
        properties: "capture_all_properties(",
        topology: "capture_topology(",
    };
    let ok = "fn run_case( capture_aggregates(x); capture_discrete(x); capture_all_elements(x);";
    assert!(check_order(ok, &a, "synthetic").is_ok());

    let after_elements =
        "fn run_case( capture_all_elements(x); capture_discrete(x); capture_aggregates(x);";
    let err = check_order(after_elements, &a, "synthetic").expect_err("group B ran first");
    assert!(err.contains("element capture"), "{err}");

    let after_discrete =
        "fn run_case( capture_discrete(x); capture_aggregates(x); capture_all_elements(x);";
    let err = check_order(after_discrete, &a, "synthetic").expect_err("the walk ran first");
    assert!(err.contains("Transformers.First/Next"), "{err}");

    let renamed = "fn run_case( grab_the_aggregates(x); capture_discrete(x);                    capture_all_elements(x);";
    let err = check_order(renamed, &a, "synthetic").expect_err("the anchor is gone");
    assert!(err.contains("could not find"), "{err}");
}

// ---------------------------------------------------------------------------
// G1.7 — the topology surface: read strictly last, and only its six
// order-free rows (`GOLDEN_REBASE_PLAN.md` §G1.7, brief B16).
// ---------------------------------------------------------------------------

/// The topology capture must be the LAST read of the step on both transports.
///
/// Two reasons, the stronger first: the first `Topology` read is what BUILDS
/// the memoized `Branch_List` and rewrites `Checked`/`IsIsolated`/`BusChecked`
/// on every element on the way (r4133 `Common/Circuit.pas:2932-2950`,
/// `:2937-2947`); and `TopologyI(1)`/`(2)` + `TopologyV(1)`/`(2)` walk
/// `PDElements`/`PCElements` `.First`/`.Next` to exhaustion
/// (`DDLL/DTopology.pas:75-94`, `:319-390`), leaving those cursors at the end.
fn check_topology_last(src: &str, a: &Anchors, rel: &str) -> Result<(), String> {
    let run = offset_after(src, a.run, 0, rel)?;
    let topology = offset_after(src, a.topology, run, rel)?;
    for (what, needle) in [
        ("the WP8.5b property sweep", a.properties),
        ("the per-element capture", a.elements),
        ("the discrete-state capture", a.discrete),
        ("the G1.9 aggregates", a.aggregates),
    ] {
        let other = offset_after(src, needle, run, rel)?;
        if topology <= other {
            return Err(format!(
                "{rel}: the G1.7 topology capture (byte {topology}) is read BEFORE {what} \
                 (byte {other}). The first `Topology` read builds the memoized `Branch_List` \
                 and rewrites Checked/IsIsolated/BusChecked on every element (r4133 \
                 `Common/Circuit.pas:2932-2950`), so it must come strictly last in the step."
            ));
        }
    }
    Ok(())
}

/// `ITopology`'s complete member surface — the closed universe this gate
/// reasons over. Nine `_columns` plus nine cursor methods, read from
/// `git -C .inputs/DSS-Python show origin/fastdss:dss/ITopology.py` (`:10-20`
/// and the `@property`/`def` list below it).
const ITOPOLOGY_MEMBERS: [&str; 18] = [
    "ActiveBranch",
    "ActiveLevel",
    "AllIsolatedBranches",
    "AllIsolatedLoads",
    "AllLoopedPairs",
    "BackwardBranch",
    "BranchName",
    "BusName",
    "First",
    "FirstLoad",
    "ForwardBranch",
    "LoopedBranch",
    "Next",
    "NextLoad",
    "NumIsolatedBranches",
    "NumIsolatedLoads",
    "NumLoops",
    "ParallelBranch",
];

/// The six of those eighteen the corpus gate captures: the only ones that do
/// **not** reassign `ActiveCircuit.ActiveCktElement` (capi
/// `CAPI/CAPI_Topology.pas:98-110`; r4133 `DTopology.pas:29-54`, `:96-160`,
/// `:170-186`). The other twelve are the B16 parity gap (`ActiveLevel`,
/// `BranchName`, `ActiveBranch`) and the nine cursor rows.
const TOPOLOGY_CAPTURED: [&str; 6] = [
    "AllIsolatedBranches",
    "AllIsolatedLoads",
    "AllLoopedPairs",
    "NumIsolatedBranches",
    "NumIsolatedLoads",
    "NumLoops",
];

/// Source language of a capture transport, for [`code_only`].
#[derive(Clone, Copy, PartialEq)]
enum Lang {
    Python,
    Rust,
}

/// `src` with every comment, docstring and string literal blanked to spaces
/// (newlines kept), so the checks below read **code**, not prose.
///
/// Both capture sources name all twelve forbidden members in their doc text,
/// deliberately — that is where the reason for the omission is written down.
/// A word-level scan would therefore be vacuous in one direction and false in
/// the other; this view removes the ambiguity, and
/// [`the_topology_gates_reject_a_cursor_read_or_an_early_capture`] proves it by
/// accepting a source whose *comment* names a cursor row and rejecting the same
/// text as *code*.
fn code_only(src: &str, lang: Lang) -> String {
    fn blank(out: &mut String, c: char) {
        out.push(if c == '\n' { '\n' } else { ' ' });
    }

    let cs: Vec<char> = src.chars().collect();
    let n = cs.len();
    let mut out = String::with_capacity(src.len());
    let mut i = 0usize;
    while i < n {
        let c = cs[i];
        // Line comments: `#` in Python, `//` in Rust (`///` and `//!` included).
        let line_comment = match lang {
            Lang::Python => c == '#',
            Lang::Rust => c == '/' && i + 1 < n && cs[i + 1] == '/',
        };
        if line_comment {
            while i < n && cs[i] != '\n' {
                blank(&mut out, cs[i]);
                i += 1;
            }
            continue;
        }
        // Rust block comments, nested.
        if lang == Lang::Rust && c == '/' && i + 1 < n && cs[i + 1] == '*' {
            let mut depth = 0usize;
            while i < n {
                if cs[i] == '/' && i + 1 < n && cs[i + 1] == '*' {
                    depth += 1;
                    blank(&mut out, cs[i]);
                    blank(&mut out, cs[i + 1]);
                    i += 2;
                    continue;
                }
                if cs[i] == '*' && i + 1 < n && cs[i + 1] == '/' {
                    depth -= 1;
                    blank(&mut out, cs[i]);
                    blank(&mut out, cs[i + 1]);
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                    continue;
                }
                blank(&mut out, cs[i]);
                i += 1;
            }
            continue;
        }
        // String literals, including Python's triple-quoted docstrings.
        let quote = c == '"' || (lang == Lang::Python && c == '\'');
        if quote {
            let triple = lang == Lang::Python && i + 2 < n && cs[i + 1] == c && cs[i + 2] == c;
            let open = if triple { 3 } else { 1 };
            for k in 0..open {
                blank(&mut out, cs[i + k]);
            }
            i += open;
            while i < n {
                if cs[i] == '\\' && i + 1 < n {
                    blank(&mut out, cs[i]);
                    blank(&mut out, cs[i + 1]);
                    i += 2;
                    continue;
                }
                if triple && i + 2 < n && cs[i] == c && cs[i + 1] == c && cs[i + 2] == c {
                    for k in 0..3 {
                        blank(&mut out, cs[i + k]);
                    }
                    i += 3;
                    break;
                }
                if !triple && cs[i] == c {
                    blank(&mut out, cs[i]);
                    i += 1;
                    break;
                }
                // An unterminated single-quoted literal (a Rust lifetime, a
                // stray apostrophe) must not swallow the rest of the file.
                if !triple && cs[i] == '\n' {
                    break;
                }
                blank(&mut out, cs[i]);
                i += 1;
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// True when `code[at]` starts an identifier token (the character before it is
/// not an identifier character), so `topology_num_loops` is not found inside
/// `engine_topology_num_loops`.
fn starts_token(code: &str, at: usize) -> bool {
    code[..at]
        .chars()
        .next_back()
        .is_none_or(|p| !(p.is_alphanumeric() || p == '_'))
}

/// The identifier that begins at `code[at]`.
fn ident_at(code: &str, at: usize) -> String {
    code[at..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Every distinct identifier in `code` that begins with `prefix`, sorted.
fn idents_with_prefix(code: &str, prefix: &str) -> Vec<String> {
    let mut found: Vec<String> = code
        .match_indices(prefix)
        .filter(|(at, _)| starts_token(code, *at))
        .map(|(at, _)| ident_at(code, at))
        .collect();
    found.sort();
    found.dedup();
    found
}

/// `NumIsolatedBranches` -> `num_isolated_branches`: the fastdss member name as
/// the `dss-epri` bridge spells its typed accessor
/// (`crates/dss-epri/src/dss.rs:1369-1398`, one `topology_*` fn per
/// `TopologyI`/`TopologyV` mode).
fn snake(camel: &str) -> String {
    let mut out = String::new();
    for (i, c) in camel.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// The six `topology_*` accessors the r4133 bridge is allowed to call.
fn expected_r4133_accessors() -> Vec<String> {
    let mut v: Vec<String> = TOPOLOGY_CAPTURED
        .iter()
        .map(|m| format!("topology_{}", snake(m)))
        .collect();
    v.sort();
    v
}

fn set_error(rel: &str, what: &str, found: &[String], want: &[String]) -> String {
    format!(
        "{rel}: {what} is {found:?}, but the G1.7 capture reads exactly {want:?}. \
         The other twelve `ITopology` members reassign `ActiveCircuit.ActiveCktElement` \
         (capi `CAPI_Topology.pas:98-110`; r4133 `DTopology.pas:29-54`, `:96-160`, \
         `:170-186`) and would poison the per-element capture of the same step; three of \
         them are also the documented B16 parity gap. If the surface really changed, change \
         `TOPOLOGY_CAPTURED` and the comparator with it — never just this list."
    )
}

/// The capi transport reads the six rows off one `ckt.Topology` handle, and
/// nothing else in the transport touches `ITopology` at all.
fn check_capi_topology_rows(src: &str, rel: &str) -> Result<(), String> {
    let code = code_only(src, Lang::Python);
    let handles = code.match_indices("ckt.Topology").count();
    if handles != 1 {
        return Err(format!(
            "{rel}: the transport reads `ckt.Topology` {handles} time(s) in code; exactly \
             one handle — the one `capture_topology` binds — may exist, so that this gate \
             can see every `ITopology` member the capture touches."
        ));
    }
    // The body of `capture_topology`: up to the next column-0 statement.
    let def = code
        .find("def capture_topology(")
        .ok_or_else(|| format!("{rel}: `def capture_topology(` is gone — update this gate"))?;
    let after_def = code[def..].find('\n').map_or(code.len(), |i| def + i + 1);
    let end = code[after_def..]
        .match_indices('\n')
        .map(|(i, _)| after_def + i + 1)
        .find(|&start| {
            code[start..]
                .chars()
                .next()
                .is_some_and(|c| !c.is_whitespace())
        })
        .unwrap_or(code.len());
    let body = &code[after_def..end];

    let bind = body
        .find("= ckt.Topology")
        .ok_or_else(|| format!("{rel}: `capture_topology` no longer binds `ckt.Topology`"))?;
    let handle: String = body[..bind]
        .trim_end()
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<char>>()
        .into_iter()
        .rev()
        .collect();
    if handle.is_empty() {
        return Err(format!("{rel}: cannot name the `ckt.Topology` handle"));
    }

    let dotted = format!("{handle}.");
    let mut read: Vec<String> = body
        .match_indices(&dotted)
        .filter(|(at, _)| starts_token(body, *at))
        .map(|(at, _)| ident_at(body, at + dotted.len()))
        .collect();
    read.sort();
    read.dedup();

    let mut want: Vec<String> = TOPOLOGY_CAPTURED.iter().map(|s| s.to_string()).collect();
    want.sort();
    if read != want {
        return Err(set_error(
            rel,
            &format!("the set of `{handle}.<member>` reads"),
            &read,
            &want,
        ));
    }
    Ok(())
}

/// The r4133 side calls (or binds) the six typed mode accessors and no other
/// `topology_*` row of the bridge.
fn check_r4133_topology_rows(src: &str, rel: &str, what: &str) -> Result<(), String> {
    let code = code_only(src, Lang::Rust);
    let found = idents_with_prefix(&code, "topology_");
    let want = expected_r4133_accessors();
    if found != want {
        return Err(set_error(rel, what, &found, &want));
    }
    Ok(())
}

#[test]
fn capi_capture_reads_the_topology_surface_last() {
    let rel = "tools/oracle/oracle_server.py";
    check_topology_last(&read(rel), &CAPI, rel).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn r4133_capture_reads_the_topology_surface_last() {
    let rel = "crates/dss-epri/src/capture.rs";
    check_topology_last(&read(rel), &R4133, rel).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn capi_capture_never_touches_an_itopology_cursor_row() {
    let rel = "tools/oracle/oracle_server.py";
    check_capi_topology_rows(&read(rel), rel).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn r4133_capture_never_calls_a_topology_cursor_accessor() {
    let rel = "crates/dss-epri/src/capture.rs";
    check_r4133_topology_rows(&read(rel), rel, "the set of `topology_*` accessors called")
        .unwrap_or_else(|e| panic!("{e}"));
}

/// The bridge cannot call a cursor row it never bound: `dss.rs` exposes exactly
/// the six order-free accessors (D2's mode-capability record, `TESTING.md`
/// §"The r4133 bridge"). Second half of the same rail — the check above says
/// the capture calls only these six, this one says only these six exist to be
/// called.
#[test]
fn the_r4133_bridge_exposes_only_the_six_order_free_topology_rows() {
    let rel = "crates/dss-epri/src/dss.rs";
    check_r4133_topology_rows(&read(rel), rel, "the set of `topology_*` accessors bound")
        .unwrap_or_else(|e| panic!("{e}"));
}

/// Non-vacuity (§1.1(f)) for the five gates above: same predicates, synthetic
/// sources. The accepted cases name forbidden rows in *comments* — which is
/// exactly why the real sources pass — and every rejected case differs from
/// them by moving that text into code, or by moving the capture.
#[test]
fn the_topology_gates_reject_a_cursor_read_or_an_early_capture() {
    // -- capi ---------------------------------------------------------------
    let py_ok = "\
def capture_topology(ckt):
    \"\"\"Reads six rows. ActiveBranch / BranchName / ActiveLevel and the cursor
    rows First, Next, ForwardBranch, BusName are never read; t.ActiveBranch
    would poison the per-element capture. A second ckt.Topology handle is
    forbidden too.\"\"\"
    t = ckt.Topology  # ActiveLevel stays untouched
    return {
        \"num_loops\": int(t.NumLoops),
        \"num_isolated_branches\": int(t.NumIsolatedBranches),
        \"num_isolated_loads\": int(t.NumIsolatedLoads),
        \"looped_pairs\": _topo_names(t.AllLoopedPairs),
        \"isolated_branches\": _topo_names(t.AllIsolatedBranches),
        \"isolated_loads\": _topo_names(t.AllIsolatedLoads),
    }


def later(ckt):
    return 1
";
    assert!(
        check_capi_topology_rows(py_ok, "synthetic").is_ok(),
        "the comment/docstring view must accept a capture that only *documents* the \
         forbidden rows — the real transport does exactly that"
    );

    let cursor = py_ok.replace(
        "        \"isolated_loads\": _topo_names(t.AllIsolatedLoads),",
        "        \"isolated_loads\": _topo_names(t.AllIsolatedLoads),\n        \"depth\": t.ActiveLevel,",
    );
    let err = check_capi_topology_rows(&cursor, "synthetic").expect_err("a cursor row was read");
    assert!(err.contains("ActiveLevel"), "{err}");

    let dropped = py_ok.replace(
        "        \"isolated_loads\": _topo_names(t.AllIsolatedLoads),\n",
        "",
    );
    let err = check_capi_topology_rows(&dropped, "synthetic").expect_err("a row went missing");
    assert!(err.contains("AllIsolatedLoads"), "{err}");

    let second_handle = py_ok.replace(
        "def later(ckt):\n    return 1",
        "def later(ckt):\n    return ckt.Topology.First",
    );
    let err =
        check_capi_topology_rows(&second_handle, "synthetic").expect_err("a second handle exists");
    assert!(err.contains("2 time(s)"), "{err}");

    // -- r4133 --------------------------------------------------------------
    let rs_ok = "\
/// Never calls topology_active_branch, topology_branch_name or
/// topology_active_level — see `DTopology.pas:29-54`.
fn capture_topology(engine: &Engine) -> Result<TopologyCap, EngineError> {
    let a = engine.topology_num_loops()?; /* not topology_first_load */
    let b = engine.topology_num_isolated_branches()?;
    let c = engine.topology_num_isolated_loads()?;
    let d = engine.topology_all_looped_pairs()?;
    let e = engine.topology_all_isolated_branches()?;
    let f = engine.topology_all_isolated_loads()?;
    Ok(pack(a, b, c, d, e, f, \"topology_active_branch is only a string here\"))
}
";
    assert!(
        check_r4133_topology_rows(rs_ok, "synthetic", "accessors").is_ok(),
        "doc comments, block comments and string literals must not count as calls"
    );

    let cursor = rs_ok.replace(
        "    let f = engine.topology_all_isolated_loads()?;",
        "    let f = engine.topology_all_isolated_loads()?;\n    let g = engine.topology_active_branch()?;",
    );
    let err = check_r4133_topology_rows(&cursor, "synthetic", "accessors")
        .expect_err("a cursor accessor was called");
    assert!(err.contains("topology_active_branch"), "{err}");

    // -- the order gate -----------------------------------------------------
    let a = Anchors {
        run: "fn run_case(",
        aggregates: "capture_aggregates(",
        elements: "capture_all_elements(",
        discrete: "capture_discrete(",
        properties: "capture_all_properties(",
        topology: "capture_topology(",
    };
    let ok = "fn run_case( capture_aggregates(x); capture_discrete(x); \
              capture_all_elements(x); capture_all_properties(x); capture_topology(x);";
    assert!(check_topology_last(ok, &a, "synthetic").is_ok());

    let early = "fn run_case( capture_aggregates(x); capture_discrete(x); capture_topology(x); \
                 capture_all_elements(x); capture_all_properties(x);";
    let err = check_topology_last(early, &a, "synthetic").expect_err("topology ran first");
    assert!(err.contains("property sweep"), "{err}");

    let renamed = "fn run_case( capture_aggregates(x); capture_discrete(x); \
                   capture_all_elements(x); capture_all_properties(x); grab_topology(x);";
    let err = check_topology_last(renamed, &a, "synthetic").expect_err("the anchor is gone");
    assert!(err.contains("could not find"), "{err}");

    // The eighteen-member universe must contain the six we capture, or the set
    // comparisons above would be checking a typo against itself.
    for m in TOPOLOGY_CAPTURED {
        assert!(
            ITOPOLOGY_MEMBERS.contains(&m),
            "{m} is not an ITopology member"
        );
    }
}
