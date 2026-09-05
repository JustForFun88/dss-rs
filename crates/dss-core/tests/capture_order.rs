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
//! * it is read after `all_properties` and after every other *reading* capture:
//!   the FIRST `Topology` read is what builds the memoized `Branch_List` and
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
//! G1.8's flat incidence surface (`Solution.IncMatrix` / `Laplacian` /
//! `IncMatrixRows` / `IncMatrixCols`) is the one capture that comes after the
//! topology read, i.e. **strictly last in the step**, and it is not a pure read
//! at all: it issues the executive pair `CalcIncMatrix` + `CalcLaplacian` first.
//! Three separate rules, all asserted below:
//!
//! * **last** — `Calc_Inc_Matrix` recreates or resets `IncMat`, refills
//!   `Inc_Mat_Rows` and clears `IncMat_Ordered` (r4133
//!   `Common/Solution.pas:3051-3066`), and `AddSeriesReac2IncMatrix` re-points
//!   `LastClassReferenced` / `ActiveDSSClass` and calls `ActiveDSSClass.First`
//!   (`:3007-3010`), which reassigns `ActiveCircuit.ActiveCktElement`. It must
//!   therefore follow every per-element and property read — and it must follow
//!   the topology read too, because G1.7's two decline censuses are defined on a
//!   `Branch_List` nothing else has touched;
//! * **the pair, in that order** — r4133's `CalcLaplacian`
//!   (`Executive/ExecCommands.pas:911-917`) is a bare
//!   `Laplacian := IncMat.Transpose()` / `.multiply(IncMat)` with no
//!   `Assigned(IncMat)` guard, so issuing it first dereferences NIL inside the
//!   DLL. capi guards the same command with error 8877
//!   (`Executive/ExecCommands.pas:421-433`) and so does the port, which is why
//!   the swap is pinned from the sources here rather than driven live;
//! * **`CalcIncMatrix_O` and `Solution.BusLevels` are never touched** — the
//!   ordered builder calls `GetTopology` (`Common/Solution.pas:3173`), which
//!   would memoize that same `Branch_List`, and `SolutionV(2)` writes one element
//!   past its own array on r4133 (`DDLL/DSolution.pas:578-582`, on the bridge's
//!   `modes::DO_NOT_CALL` register). Both keep their `tests/golden/inc_matrix/`
//!   `org_*` byte goldens instead (`GOLDEN_REBASE_PLAN.md` §G1.8 / §G3.2c).
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

/// The seven call sites the gate locates inside one transport's `run_case`.
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
    /// G1.7's topology capture — read after `properties`, i.e. last of the
    /// order-free *reads*.
    topology: &'static str,
    /// G1.8's flat incidence capture — after `topology`, i.e. strictly last in
    /// the step. It is the only capture that WRITES solution state.
    inc_matrix: &'static str,
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
    inc_matrix: "capture_inc_matrix(d, ckt",
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
    inc_matrix: "capture_inc_matrix(engine",
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
        inc_matrix: "capture_inc_matrix(",
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

/// The topology capture must follow every other READ of the step on both
/// transports — the four below. Since G1.8 exactly one capture comes after it,
/// the incidence pair, and that ordering is [`check_inc_matrix_last`]'s.
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
                 `Common/Circuit.pas:2932-2950`), so it must follow every other read of                  the step (only G1.8's incidence pair may come after it)."
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
        inc_matrix: "capture_inc_matrix(",
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

// ---------------------------------------------------------------------------
// G1.8 — the flat incidence surface: issued and read strictly last, as the pair
// `CalcIncMatrix` + `CalcLaplacian`, and never `CalcIncMatrix_O` /
// `Solution.BusLevels` (`GOLDEN_REBASE_PLAN.md` §G1.8 / §G3.2c, decision D3).
// ---------------------------------------------------------------------------

/// The two executive commands the G1.8 capture may issue, in this order.
///
/// The order is a contract, not a style: r4133's `CalcLaplacian`
/// (`Version8/Source/Executive/ExecCommands.pas:911-917`) is a bare
/// `Laplacian := IncMat.Transpose()` / `.multiply(IncMat)` with no
/// `Assigned(IncMat)` test, so issuing it before any `CalcIncMatrix`
/// dereferences NIL inside the DLL and kills the worker. capi guards the same
/// command with error 8877 (`.inputs/dss_capi/src/Executive/ExecCommands.pas:421-433`)
/// and so does the port — which is why the swap is pinned from the sources here
/// rather than driven live (`inc_matrix_pins.rs::calclaplacian_without_calcincmatrix_raises_8877`).
const INCIDENCE_PAIR: [&str; 2] = ["CalcIncMatrix", "CalcLaplacian"];

/// The four `ISolution` members the capture reads afterwards, in this order
/// (`origin/fastdss:dss/ISolution.py:608-631`, `:651-675`, `:642-649`,
/// `:633-640`).
const INCIDENCE_READS: [&str; 4] = ["IncMatrix", "Laplacian", "IncMatrixRows", "IncMatrixCols"];

/// Executive commands no capture may ever issue. `CalcIncMatrix_O`
/// (`Calc_Inc_Matrix_Org`) calls `GetTopology` (r4133
/// `Common/Solution.pas:3173`), which builds and memoizes the very `Branch_List`
/// G1.7's two decline censuses are defined on. Its coverage stays byte-golden
/// (`tests/golden/inc_matrix/` `org_*`), per the §G3.2c re-scope.
const FORBIDDEN_COMMANDS: [&str; 1] = ["CalcIncMatrix_O"];

/// `Solution.BusLevels` — `SolutionV(2)`, `DDLL/DSolution.pas:569-588`: `:578`
/// sizes the buffer `length(Inc_Mat_Levels) - 1` and `:581` writes `0..ArrSize`
/// inclusive, one element past the end. The r4133 bridge refuses the mode before
/// the FFI (`crates/dss-epri/src/modes.rs::DO_NOT_CALL`), so a surface only one
/// channel can answer is not a gate: neither transport may read it.
const FORBIDDEN_READS: [&str; 2] = ["BusLevels", "bus_levels"];

/// The incidence capture must come after EVERY other capture of the step, the
/// topology one included — it is the only capture that WRITES solution state.
///
/// `Calc_Inc_Matrix` recreates or resets `IncMat`, refills `Inc_Mat_Rows` and
/// clears `IncMat_Ordered` (r4133 `Common/Solution.pas:3051-3066`), and
/// `AddSeriesReac2IncMatrix` re-points `LastClassReferenced` / `ActiveDSSClass`
/// and calls `ActiveDSSClass.First` (`:3007-3010`), reassigning
/// `ActiveCircuit.ActiveCktElement`; and it must follow the topology read,
/// because that read is the one that memoizes `Branch_List`.
fn check_inc_matrix_last(src: &str, a: &Anchors, rel: &str) -> Result<(), String> {
    let run = offset_after(src, a.run, 0, rel)?;
    let inc = offset_after(src, a.inc_matrix, run, rel)?;
    for (what, needle) in [
        ("the G1.7 topology capture", a.topology),
        ("the WP8.5b property sweep", a.properties),
        ("the per-element capture", a.elements),
        ("the discrete-state capture", a.discrete),
        ("the G1.9 aggregates", a.aggregates),
    ] {
        let other = offset_after(src, needle, run, rel)?;
        if inc <= other {
            return Err(format!(
                "{rel}: the G1.8 incidence capture (byte {inc}) runs BEFORE {what} \
                 (byte {other}). `Calc_Inc_Matrix` rewrites solution state and \
                 `AddSeriesReac2IncMatrix` reassigns ActiveCktElement (r4133 \
                 `Common/Solution.pas:3007-3010`, `:3051-3066`), and it must not \
                 precede the topology read that memoizes `Branch_List` — so it \
                 comes strictly last in the step."
            ));
        }
    }
    Ok(())
}

/// The body of a top-level Python `def`: from the end of its header line to the
/// next line that begins in column 0 with a non-blank character.
fn py_def_body<'a>(text: &'a str, header: &str, rel: &str) -> Result<&'a str, String> {
    let at = text
        .find(header)
        .ok_or_else(|| format!("{rel}: `{header}` is gone — update this gate"))?;
    let start = text[at..].find('\n').map_or(text.len(), |i| at + i + 1);
    let end = text[start..]
        .match_indices('\n')
        .map(|(i, _)| start + i + 1)
        .find(|&s| text[s..].chars().next().is_some_and(|c| !c.is_whitespace()))
        .unwrap_or(text.len());
    Ok(&text[start..end])
}

/// The body of a top-level Rust `fn`: from its header to the first line that is
/// exactly `}` in column 0 — rustfmt's shape for a free function.
fn rust_fn_body<'a>(text: &'a str, header: &str, rel: &str) -> Result<&'a str, String> {
    let at = text
        .find(header)
        .ok_or_else(|| format!("{rel}: `{header}` is gone — update this gate"))?;
    let end = text[at..]
        .find("\n}\n")
        .map(|i| at + i + 1)
        .ok_or_else(|| format!("{rel}: `{header}` has no column-0 closing brace"))?;
    Ok(&text[at..end])
}

/// Every `….Text.Command = "<literal>"` assignment in `text`, in source order.
///
/// Read off the **raw** source on purpose: [`code_only`] blanks string literals,
/// and the command names ARE those literals — a `code_only` view of this rule
/// would be vacuous in both directions.
fn py_commands(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (at, _) in text.match_indices("Text.Command") {
        let rest = text[at + "Text.Command".len()..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('"') else {
            continue;
        };
        if let Some(end) = rest.find('"') {
            out.push(rest[..end].to_string());
        }
    }
    out
}

/// Every `exec_wait("<literal>")` call in `text`, in source order — likewise off
/// the raw source.
fn rust_commands(text: &str) -> Vec<String> {
    let needle = "exec_wait(\"";
    text.match_indices(needle)
        .filter_map(|(at, _)| {
            let rest = &text[at + needle.len()..];
            rest.find('"').map(|end| rest[..end].to_string())
        })
        .collect()
}

/// Every identifier in `code` that begins with `prefix`, in **source order** —
/// unlike [`idents_with_prefix`], which sorts and dedups. The read ORDER is part
/// of what this gate protects, and a member read twice is itself a finding.
fn idents_in_order(code: &str, prefix: &str) -> Vec<String> {
    code.match_indices(prefix)
        .filter(|(at, _)| starts_token(code, *at))
        .map(|(at, _)| ident_at(code, at))
        .collect()
}

/// Every identifier in `code` that CONTAINS `needle`, deduped, in source order.
///
/// Substring, not token start: a forbidden read hides inside
/// `engine.solution_bus_levels()` as surely as it does in `sol.BusLevels`, and
/// only the first of the two starts a token at the needle.
fn idents_containing(code: &str, needle: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (at, _) in code.match_indices(needle) {
        let start = code[..at]
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_alphanumeric() || *c == '_')
            .map(|(i, _)| i)
            .last()
            .unwrap_or(at);
        let id = ident_at(code, start);
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// Every `<handle>.<member>` read in `body`, in source order.
fn member_reads_in_order(body: &str, handle: &str) -> Vec<String> {
    let dotted = format!("{handle}.");
    body.match_indices(&dotted)
        .filter(|(at, _)| starts_token(body, *at))
        .map(|(at, _)| ident_at(body, at + dotted.len()))
        .collect()
}

fn incidence_error(rel: &str, what: &str, found: &[String], want: &[String]) -> String {
    format!(
        "{rel}: {what} is {found:?}, but the G1.8 capture does exactly {want:?}. \
         The pair's ORDER is a contract (r4133 `ExecCommands.pas:911-917` has no \
         NIL guard and would crash the worker; capi and the port raise 8877), the \
         read order is what both channels must share, and `CalcIncMatrix_O` / \
         `Solution.BusLevels` are out of the live gate by decision \
         (`GOLDEN_REBASE_PLAN.md` §G1.8 / §G3.2c). If the surface really changed, \
         change these constants and the comparator with them — never just this list."
    )
}

fn owned(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

/// The capi incidence capture: the pair in order, then the four reads off the
/// one `ckt.Solution` handle it binds — and nothing else.
///
/// The command rule reads the RAW body, so it also refuses a *computed* command
/// (`Text.Command = f"…"`): the gate would not be able to tell what was issued.
/// The corollary is that the function's docstring may name a command but must
/// not spell an assignment to `Text.Command` — the real transport's does not.
fn check_capi_incidence(src: &str, rel: &str) -> Result<(), String> {
    let raw = py_def_body(src, "def capture_inc_matrix(", rel)?;
    let cmds = py_commands(raw);
    let sites = raw.matches("Text.Command").count();
    if sites != cmds.len() {
        return Err(format!(
            "{rel}: `capture_inc_matrix` has {sites} `Text.Command` site(s) but only \
             {} plain string literal(s). This gate cannot read a computed command — \
             write the two commands as literals, or the ordering rule is unverifiable.",
            cmds.len()
        ));
    }
    if cmds != owned(&INCIDENCE_PAIR) {
        return Err(incidence_error(
            rel,
            "the executive commands `capture_inc_matrix` issues",
            &cmds,
            &owned(&INCIDENCE_PAIR),
        ));
    }

    let code = code_only(src, Lang::Python);
    let body = py_def_body(&code, "def capture_inc_matrix(", rel)?;
    let bind = body.find("= ckt.Solution").ok_or_else(|| {
        format!("{rel}: `capture_inc_matrix` no longer binds `ckt.Solution` — update this gate")
    })?;
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
        return Err(format!("{rel}: cannot name the `ckt.Solution` handle"));
    }
    let reads = member_reads_in_order(body, &handle);
    if reads != owned(&INCIDENCE_READS) {
        return Err(incidence_error(
            rel,
            &format!("the sequence of `{handle}.<member>` reads"),
            &reads,
            &owned(&INCIDENCE_READS),
        ));
    }
    Ok(())
}

/// The r4133 incidence capture: the same pair, in the same order, then the four
/// typed mode accessors in the same order.
///
/// The pair is also checked **file-wide**: `crates/dss-epri/src/capture.rs`
/// issues executive commands at exactly these two sites and nowhere else, so a
/// capture cannot quietly drive the engine mid-step.
fn check_r4133_incidence(src: &str, rel: &str) -> Result<(), String> {
    let raw = rust_fn_body(src, "fn capture_inc_matrix(", rel)?;
    let cmds = rust_commands(raw);
    if cmds != owned(&INCIDENCE_PAIR) {
        return Err(incidence_error(
            rel,
            "the executive commands `capture_inc_matrix` issues",
            &cmds,
            &owned(&INCIDENCE_PAIR),
        ));
    }
    let file = rust_commands(src);
    if file != owned(&INCIDENCE_PAIR) {
        return Err(incidence_error(
            rel,
            "the executive commands the whole transport issues",
            &file,
            &owned(&INCIDENCE_PAIR),
        ));
    }

    let code = code_only(src, Lang::Rust);
    let body = rust_fn_body(&code, "fn capture_inc_matrix(", rel)?;
    let want: Vec<String> = INCIDENCE_READS
        .iter()
        .map(|m| format!("solution_{}", snake(m)))
        .collect();
    let called = idents_in_order(body, "solution_");
    if called != want {
        return Err(incidence_error(
            rel,
            "the sequence of `solution_*` accessors called",
            &called,
            &want,
        ));
    }
    Ok(())
}

/// Neither transport may issue [`FORBIDDEN_COMMANDS`] anywhere, nor read
/// [`FORBIDDEN_READS`] as code anywhere. Both sources name them in prose — that
/// is where the reason is written down — so the command rule reads the raw
/// literals and the read rule reads the [`code_only`] view.
fn check_no_forbidden_incidence_use(
    src: &str,
    lang: Lang,
    rel: &str,
    commands: &[String],
) -> Result<(), String> {
    for bad in FORBIDDEN_COMMANDS {
        if commands.iter().any(|c| c == bad) {
            return Err(format!(
                "{rel}: the transport issues `{bad}`. `Calc_Inc_Matrix_Org` calls \
                 `GetTopology` (r4133 `Common/Solution.pas:3173`), which memoizes the \
                 `Branch_List` G1.7's `TOPOLOGY_STALE_DECLINES` / \
                 `LOOPED_PAIR_WINDOW_DECLINES` censuses are defined on — it is out of \
                 the live gate by decision (`GOLDEN_REBASE_PLAN.md` §G1.8 / §G3.2c) and \
                 keeps its `org_*` byte goldens instead."
            ));
        }
    }
    let code = code_only(src, lang);
    for bad in FORBIDDEN_READS {
        let hits = idents_containing(&code, bad);
        if !hits.is_empty() {
            return Err(format!(
                "{rel}: the transport reads `{bad}` in code ({hits:?}). `SolutionV(2)` \
                 `Solution.BusLevels` writes one element past its own array on r4133 \
                 (`DDLL/DSolution.pas:578-582`) and sits on the bridge's \
                 `modes::DO_NOT_CALL` register, so no channel may read it."
            ));
        }
    }
    Ok(())
}

/// The bridge cannot call a mode it refuses before the FFI: `SolutionV(2)` must
/// still be on `modes::DO_NOT_CALL`. Second half of the same rail — the check
/// above says no capture reads bus levels, this one says the mode is refused
/// even if one tried.
fn check_do_not_call_still_refuses_bus_levels(src: &str, rel: &str) -> Result<(), String> {
    let at = src
        .find("pub const DO_NOT_CALL")
        .ok_or_else(|| format!("{rel}: `pub const DO_NOT_CALL` is gone — update this gate"))?;
    let end = src[at..]
        .find("\n];")
        .map(|i| at + i)
        .ok_or_else(|| format!("{rel}: `DO_NOT_CALL` has no closing `];`"))?;
    let block = &src[at..end];
    for needle in [
        "\"Solution\"",
        "ModeKind::V,",
        "        2,",
        "Solution.BusLevels",
    ] {
        if !block.contains(needle) {
            return Err(format!(
                "{rel}: the `DO_NOT_CALL` register no longer carries `{needle}`. \
                 `SolutionV(2)` `Solution.BusLevels` is a one-element heap overflow \
                 inside the DLL (`DDLL/DSolution.pas:578-582`); removing the row would \
                 let a future accessor call it."
            ));
        }
    }
    Ok(())
}

/// The capi transport's two incidence normalizers must still REFUSE a shape they
/// were not written for.
///
/// `_inc_ints` implements rule N1 — drop the one trailing cell capi allocates and
/// never writes (`CAPI_Solution.pas:873`/`:910`) — and the sub-step's KILL
/// CRITERION is that the cell is 0 and the length is `3·NZero + 1`
/// (`GOLDEN_REBASE_PLAN.md` §G1.8). Both are enforced in Python, where no Rust
/// test reaches them; an edit that kept the strip and dropped the zero-check
/// (`return xs[:-1]` unconditionally) would leave every existing test green,
/// because the gate-side `assert_transport_shape` only re-asserts the fixpoint
/// `len % 3 == 0` — and a capi reply whose last cell stopped being the unwritten
/// slot would then be silently truncated. Same shape for `_inc_names`' rule N3:
/// the one-element `''` sentinel is dropped ONLY where the engine can reach it
/// (`sentinel_ok`), and any other blank raises. The r4133 twins have real unit
/// tests (`crates/dss-epri/src/capture.rs`); this is their capi counterpart,
/// written as a source-text gate for the reason every rule in this file is.
/// (G1.8 audit settlement, finding G18-T2.)
fn check_capi_incidence_normalizers(src: &str, rel: &str) -> Result<(), String> {
    for (header, needles) in [
        (
            "def _inc_ints(",
            &[
                "len(xs) % 3 != 1",
                "xs[-1] != 0",
                "raise ValueError",
                "return xs[:-1]",
            ][..],
        ),
        (
            "def _inc_names(",
            &["if not sentinel_ok:", "raise ValueError", "return xs"][..],
        ),
    ] {
        let body = py_def_body(src, header, rel)?;
        for needle in needles {
            if !body.contains(needle) {
                return Err(format!(
                    "{rel}: `{header}…` no longer contains `{needle}`. The G1.8 \
                     transport normalizations must RAISE on a shape they were not \
                     written for, never repair it silently: the trailing-cell \
                     contract is this sub-step's kill criterion, and the empty \
                     name sentinel may be dropped only where the engine can reach \
                     it."
                ));
            }
        }
        if body.matches("raise ValueError").count() < 2 {
            return Err(format!(
                "{rel}: `{header}…` has fewer than two `raise ValueError` arms — \
                 one of the two refusals was dropped"
            ));
        }
    }
    Ok(())
}

#[test]
fn the_capi_incidence_transport_refuses_a_shape_it_was_not_written_for() {
    let rel = "tools/oracle/oracle_server.py";
    check_capi_incidence_normalizers(&read(rel), rel).unwrap_or_else(|e| panic!("{e}"));

    // Non-vacuity: a normalizer that strips the cell without checking it, and
    // one that drops the sentinel unconditionally, must both be refused.
    let ok = "\
def _inc_ints(v, what: str) -> list:
    xs = [int(x) for x in v]
    if len(xs) % 3 != 1:
        raise ValueError(what)
    if xs[-1] != 0:
        raise ValueError(what)
    return xs[:-1]


def _inc_names(v, sentinel_ok: bool, what: str) -> list:
    xs = [str(s) for s in v]
    if len(xs) == 1 and xs[0].strip() == \"\":
        if not sentinel_ok:
            raise ValueError(what)
        return []
    if [i for i, s in enumerate(xs) if not s.strip()]:
        raise ValueError(what)
    return xs
";
    assert!(check_capi_incidence_normalizers(ok, "synthetic").is_ok());

    for (what, mutated) in [
        (
            "the trailing-cell zero check",
            ok.replace("    if xs[-1] != 0:\n        raise ValueError(what)\n", ""),
        ),
        (
            "the length check",
            ok.replace(
                "    if len(xs) % 3 != 1:\n        raise ValueError(what)\n",
                "",
            ),
        ),
        (
            "the sentinel guard",
            ok.replace(
                "        if not sentinel_ok:\n            raise ValueError(what)\n",
                "",
            ),
        ),
    ] {
        check_capi_incidence_normalizers(&mutated, "synthetic")
            .expect_err(&format!("{what} was dropped and the gate stayed green"));
    }
}

#[test]
fn capi_capture_reads_the_incidence_surface_last() {
    let rel = "tools/oracle/oracle_server.py";
    check_inc_matrix_last(&read(rel), &CAPI, rel).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn r4133_capture_reads_the_incidence_surface_last() {
    let rel = "crates/dss-epri/src/capture.rs";
    check_inc_matrix_last(&read(rel), &R4133, rel).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn the_incidence_capture_issues_calcincmatrix_then_calclaplacian() {
    let capi = "tools/oracle/oracle_server.py";
    check_capi_incidence(&read(capi), capi).unwrap_or_else(|e| panic!("{e}"));
    let r4133 = "crates/dss-epri/src/capture.rs";
    check_r4133_incidence(&read(r4133), r4133).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn neither_capture_calls_calcincmatrix_o_or_reads_buslevels() {
    let capi = "tools/oracle/oracle_server.py";
    let src = read(capi);
    check_no_forbidden_incidence_use(&src, Lang::Python, capi, &py_commands(&src))
        .unwrap_or_else(|e| panic!("{e}"));

    for r4133 in [
        "crates/dss-epri/src/capture.rs",
        "crates/dss-epri/src/dss.rs",
    ] {
        let src = read(r4133);
        check_no_forbidden_incidence_use(&src, Lang::Rust, r4133, &rust_commands(&src))
            .unwrap_or_else(|e| panic!("{e}"));
    }

    let modes = "crates/dss-epri/src/modes.rs";
    check_do_not_call_still_refuses_bus_levels(&read(modes), modes)
        .unwrap_or_else(|e| panic!("{e}"));
}

/// Non-vacuity (§1.1(f)) for the four gates above: same predicates, synthetic
/// sources. The accepted cases name the forbidden command and the forbidden read
/// in *prose* — which is exactly what the real sources do — and every rejected
/// case differs from them by moving that text into code, swapping the pair,
/// dropping a read, or moving the capture earlier in the step.
#[test]
fn the_incidence_gates_reject_a_swapped_or_early_capture() {
    // -- capi ---------------------------------------------------------------
    let py_ok = "\
def capture_inc_matrix(d, ckt) -> dict:
    \"\"\"NEVER CalcIncMatrix_O and never sol.BusLevels: the first calls
    GetTopology, the second overruns its array.\"\"\"
    sol = ckt.Solution
    d.Text.Command = \"CalcIncMatrix\"
    d.Text.Command = \"CalcLaplacian\"
    inc = _inc_ints(sol.IncMatrix, \"IncMatrix\")
    lap = _inc_ints(sol.Laplacian, \"Laplacian\")
    return {
        \"rows\": _inc_names(sol.IncMatrixRows, not inc, \"IncMatrixRows\"),
        \"cols\": _inc_names(sol.IncMatrixCols, int(ckt.NumBuses) == 0, \"IncMatrixCols\"),
    }


def later(d):
    d.Text.Command = \"solve\"
";
    assert!(
        check_capi_incidence(py_ok, "synthetic").is_ok(),
        "a capture that only DOCUMENTS the forbidden command must pass — the real \
         transport does exactly that"
    );
    assert!(
        check_no_forbidden_incidence_use(py_ok, Lang::Python, "synthetic", &py_commands(py_ok))
            .is_ok(),
        "…and the docstring's `CalcIncMatrix_O` / `BusLevels` are prose, not code"
    );

    let swapped = py_ok
        .replace("d.Text.Command = \"CalcIncMatrix\"\n    ", "")
        .replace(
            "d.Text.Command = \"CalcLaplacian\"",
            "d.Text.Command = \"CalcLaplacian\"\n    d.Text.Command = \"CalcIncMatrix\"",
        );
    let err = check_capi_incidence(&swapped, "synthetic").expect_err("the pair was swapped");
    assert!(err.contains("executive commands"), "{err}");
    assert!(err.contains("CalcLaplacian"), "{err}");

    let ordered = py_ok.replace(
        "d.Text.Command = \"CalcLaplacian\"",
        "d.Text.Command = \"CalcIncMatrix_O\"",
    );
    let err = check_no_forbidden_incidence_use(
        &ordered,
        Lang::Python,
        "synthetic",
        &py_commands(&ordered),
    )
    .expect_err("the ordered builder was issued");
    assert!(err.contains("GetTopology"), "{err}");

    let levels = py_ok.replace(
        "    lap = _inc_ints(sol.Laplacian, \"Laplacian\")",
        "    lap = _inc_ints(sol.Laplacian, \"Laplacian\")\n    lvl = sol.BusLevels",
    );
    let err =
        check_no_forbidden_incidence_use(&levels, Lang::Python, "synthetic", &py_commands(&levels))
            .expect_err("bus levels were read");
    assert!(err.contains("DO_NOT_CALL"), "{err}");
    let err = check_capi_incidence(&levels, "synthetic").expect_err("an extra member was read");
    assert!(err.contains("BusLevels"), "{err}");

    let dropped = py_ok.replace(
        "        \"cols\": _inc_names(sol.IncMatrixCols, int(ckt.NumBuses) == 0, \"IncMatrixCols\"),\n",
        "",
    );
    let err = check_capi_incidence(&dropped, "synthetic").expect_err("a read went missing");
    assert!(err.contains("IncMatrixCols"), "{err}");

    let computed = py_ok.replace(
        "d.Text.Command = \"CalcLaplacian\"",
        "d.Text.Command = f\"Calc{what}\"",
    );
    let err = check_capi_incidence(&computed, "synthetic").expect_err("a computed command");
    assert!(err.contains("plain string literal"), "{err}");

    // -- r4133 --------------------------------------------------------------
    let rs_ok = "\
/// Never issues `CalcIncMatrix_O` and never reads `Solution.BusLevels` /
/// `solution_bus_levels` — `DSolution.pas:578-582`.
fn capture_inc_matrix(engine: &Engine) -> Result<IncMatrixCap, EngineError> {
    engine.exec_wait(\"CalcIncMatrix\")?;
    engine.exec_wait(\"CalcLaplacian\")?;
    let inc_matrix = inc_ints(engine.solution_inc_matrix()?, \"Solution.IncMatrix\")?;
    let laplacian = inc_ints(engine.solution_laplacian()?, \"Solution.Laplacian\")?;
    let rows = inc_names(engine.solution_inc_matrix_rows()?, \"r\", inc_matrix.is_empty())?;
    let cols = inc_names(engine.solution_inc_matrix_cols()?, \"c\", false)?;
    Ok(IncMatrixCap { inc_matrix, laplacian, rows, cols })
}
";
    assert!(
        check_r4133_incidence(rs_ok, "synthetic").is_ok(),
        "doc comments and string literals must not count as calls"
    );
    assert!(
        check_no_forbidden_incidence_use(rs_ok, Lang::Rust, "synthetic", &rust_commands(rs_ok))
            .is_ok()
    );

    let swapped = rs_ok.replace(
        "    engine.exec_wait(\"CalcIncMatrix\")?;\n    engine.exec_wait(\"CalcLaplacian\")?;",
        "    engine.exec_wait(\"CalcLaplacian\")?;\n    engine.exec_wait(\"CalcIncMatrix\")?;",
    );
    let err = check_r4133_incidence(&swapped, "synthetic").expect_err("the pair was swapped");
    assert!(err.contains("ExecCommands.pas:911-917"), "{err}");

    let extra = rs_ok.replace(
        "    let cols = inc_names(engine.solution_inc_matrix_cols()?, \"c\", false)?;",
        "    let cols = inc_names(engine.solution_inc_matrix_cols()?, \"c\", false)?;\n    let _ = engine.solution_bus_levels()?;",
    );
    let err = check_r4133_incidence(&extra, "synthetic").expect_err("an extra accessor was called");
    assert!(err.contains("solution_bus_levels"), "{err}");
    let err =
        check_no_forbidden_incidence_use(&extra, Lang::Rust, "synthetic", &rust_commands(&extra))
            .expect_err("bus levels were read");
    assert!(err.contains("DO_NOT_CALL"), "{err}");

    let stray =
        format!("{rs_ok}\nfn other(e: &Engine) {{\n    e.exec_wait(\"solve\").unwrap();\n}}\n");
    let err = check_r4133_incidence(&stray, "synthetic").expect_err("a stray command was issued");
    assert!(err.contains("the whole transport issues"), "{err}");

    // -- the DO_NOT_CALL register -------------------------------------------
    let modes_ok = "\
pub const DO_NOT_CALL: &[(&str, ModeKind, i32, &str)] = &[
    (
        \"Solution\",
        ModeKind::V,
        2,
        \"Solution.BusLevels: DSolution.pas:580-582 overruns its array\",
    ),
];
";
    assert!(check_do_not_call_still_refuses_bus_levels(modes_ok, "synthetic").is_ok());
    let gutted = modes_ok.replace("        \"Solution\",\n", "        \"Bus\",\n");
    let err = check_do_not_call_still_refuses_bus_levels(&gutted, "synthetic")
        .expect_err("the row is gone");
    assert!(err.contains("heap overflow"), "{err}");

    // -- the order gate -----------------------------------------------------
    let a = Anchors {
        run: "fn run_case(",
        aggregates: "capture_aggregates(",
        elements: "capture_all_elements(",
        discrete: "capture_discrete(",
        properties: "capture_all_properties(",
        topology: "capture_topology(",
        inc_matrix: "capture_inc_matrix(",
    };
    let ok = "fn run_case( capture_aggregates(x); capture_discrete(x); capture_all_elements(x); \
              capture_all_properties(x); capture_topology(x); capture_inc_matrix(x);";
    assert!(check_inc_matrix_last(ok, &a, "synthetic").is_ok());
    assert!(check_topology_last(ok, &a, "synthetic").is_ok());

    let before_topology = "fn run_case( capture_aggregates(x); capture_discrete(x); capture_all_elements(x); \
         capture_all_properties(x); capture_inc_matrix(x); capture_topology(x);";
    let err = check_inc_matrix_last(before_topology, &a, "synthetic")
        .expect_err("the incidence pair ran before the topology read");
    assert!(err.contains("topology capture"), "{err}");
    assert!(err.contains("Branch_List"), "{err}");

    // Topology first this time, so its arm is satisfied and the failure has to
    // come from one of the reads the pair still overtakes.
    let early = "fn run_case( capture_aggregates(x); capture_discrete(x); capture_topology(x); \
                 capture_inc_matrix(x); capture_all_elements(x); capture_all_properties(x);";
    let err = check_inc_matrix_last(early, &a, "synthetic").expect_err("the pair ran early");
    assert!(err.contains("property sweep"), "{err}");

    let renamed = "fn run_case( capture_aggregates(x); capture_discrete(x); \
                   capture_all_elements(x); capture_all_properties(x); capture_topology(x); \
                   build_the_matrix(x);";
    let err = check_inc_matrix_last(renamed, &a, "synthetic").expect_err("the anchor is gone");
    assert!(err.contains("could not find"), "{err}");
}
