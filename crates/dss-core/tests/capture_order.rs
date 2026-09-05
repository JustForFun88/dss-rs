//! Source gate for the **capture-order contract** (`GOLDEN_REBASE_PLAN.md`
//! §1.1(a), coordinator decision **D3**): on both live-oracle transports, every
//! *cache-aware* read of an element (group **A**) must be issued **before**
//! every read that runs `GetCurrents` into a scratch buffer (group **B**);
//! everything else (selectors, discrete state, the node-voltage reads) is
//! order-free (group **C**).
//!
//! # Why the order is a contract and not a habit
//!
//! `TDSSCktElement.ComputeIterminal` (r4133
//! `Common/CktElement.pas:632-639`) recomputes `Iterminal` **only** when the
//! element's `IterminalSolutionCount` differs from `Solution.SolutionCount`.
//! `TPCElement.GetTerminalCurrents` (`PCElements/PCElement.pas:247-266`) fills
//! the CALLER's scratch buffer and then stamps that counter
//! (`set_ITerminalUpdated(TRUE)`, `:510-514`) **without** filling `Iterminal`.
//! So a group-B read issued first can make a following group-A read
//! (`Losses` — `Common/CktElement.pas:707`, `Get_Losses`, `ComputeIterminal` at
//! `:743`; `Powers` — `GetPhasePower`, `Common/CktElement.pas:1041/:1049`)
//! answer from a stale cache. That is the mechanism behind the upstream
//! harmonics `Powers`-after-`Currents` defect this project never reproduces
//! (CLAUDE.md §"Known upstream bugs"), and it is why a capture must never be
//! free to reorder its reads.
//!
//! # What this gate checks
//!
//! Each capture body declares its reads with machine-checkable
//! `capture-order: <Name> (<A|B|C>)` markers, and this file asserts, per body:
//!
//! 1. every marker's group is [`dss_epri::modes::capture_group_of`]'s — the
//!    **one** home of that mapping, where each group comes from
//!    `ModeEffect::capture_group()` of the row that transcribes the Pascal
//!    `case` arm (G1.3a spec amendment item 1: no second, competing order
//!    table may exist in the repo);
//! 2. the parsed marker sequence is exactly the transport's declared sequence
//!    (so a read that appears, disappears or moves is visible here);
//! 3. every group-A marker precedes every group-B marker;
//! 4. no read line inside a capture body is unmarked — a later sub-step cannot
//!    add an invisible read;
//! 5. a call into another capture helper declares that helper's reads, in that
//!    helper's own order, and the declaration is cross-checked against the
//!    helper body itself. This is what covers
//!    `tools/golden/gen_checkpoints.py::capture_element`, which carries no
//!    markers because GOLDEN_REBASE WP-G1 may not edit the golden generators
//!    at all (`GOLDEN_REBASE_PLAN.md` §1.2): its reads are recovered
//!    structurally instead and must match the composite marker at its call
//!    site.
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
//! The bodies gated here are the element captures of the two live channels:
//! the pinned dss-python oracle (`tools/oracle/oracle_server.py`, backend
//! dss_capi 0.14.5 — `CAPI/CAPI_Alt.pas:1043` `CurrentsMagAng`, `:1072`
//! `VoltagesMagAng`, `CAPI/CAPI_CktElement.pas:541` `Residuals`) and the EPRI
//! r4133 DLL bridge (`crates/dss-epri` — `DDLL/DCktElement.pas:263` `Enabled`,
//! `:827` `Residuals`, `:1058` `CurrentsMagAng`, `:1082` `VoltagesMagAng`).
//!
//! # Stronger than the upstream harness
//!
//! The fastdss reference harness iterates its element columns as a Python
//! **`set`** (`git show origin/fastdss:tests/save_outputs.py:169`,
//! `ckt_elem_columns = set(type(ckt_elem)._columns) - …`), i.e. it has no read
//! order at all — the very hazard above is unconstrained there. This gate is
//! the contract fastdss lacks.
//!
//! # Non-vacuity
//!
//! The checker is a pure function over body text, so the demo runs on every
//! `cargo test`: each of the ten mutation tests below feeds a *deliberately
//! corrupted copy* of a real body through the same checker and asserts that the
//! intended rule — not merely *something* — fires. Between them they cover
//! moving `Losses` back after `Currents`, stripping a marker off a read,
//! smuggling in a brand-new undeclared read (on both the Python and the Rust
//! transport), a malformed marker, a marker no read consumes, a group that
//! contradicts the mode table, a name the table does not know, a read that
//! disappears, and a helper call that misdeclares the helper's order. One
//! further mutation states the rule's *positive* half — a group-**C** read moved
//! between a group-A and a group-B read raises the declaration-bookkeeping
//! violation but **not** the `order:` one, because C is order-free by
//! construction. No file on disk is ever mutated.
//!
//! # The second ordering rule: the per-step call order (G1.9, folded in here)
//!
//! The rules above govern the reads *inside* one element capture. The G1.9
//! circuit aggregates (`Circuit.Losses`, `LineLosses`, `SubstationLosses`,
//! `TotalPower`, `AllElementLosses`) are group **A** too, but they live in
//! their own body, so what has to hold for them is the order of the CALLS
//! inside each transport's `run_case`: the aggregates must be read before the
//! element capture (whose `Currents` read is group B) and before the discrete
//! capture (which drives `Transformers.First/Next` of its own, while every
//! aggregate walks a `TPointerList` to exhaustion and leaves its cursor at the
//! end). [`check_order`] and its two channel anchor sets carry that rule, and
//! [`the_gate_rejects_a_swapped_or_renamed_capture`] shows it has
//! teeth. Folded here from the G1.9 lane's own `capture_order.rs` at the D7
//! lane merge (2026-09-05), which D3 always assigned to this file.
//!
//! # Platform
//!
//! The mapping this gate resolves against lives in `dss-epri`, which is
//! `#[cfg(windows)]` (the vendored EPRI binary is a Win64 DLL — see
//! `crates/dss-epri/src/lib.rs`). On any other platform the r4133 channel, and
//! with it the capture-order contract it shares with the capi channel, does not
//! exist; the gate compiles away rather than checking half a contract against a
//! table it cannot see. The full five-command gate runs on Windows.
#![cfg(windows)]

use std::fs;
use std::path::PathBuf;

use dss_epri::modes::capture_group_of;

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

/// Reads that move a cursor or hand out a facade instead of reading a
/// quantity. They have no `ModeSpec` row — [`capture_group_of`] returns `None`
/// for every one of them, asserted by
/// [`the_declared_selectors_are_not_quantity_reads`] — so they are order-free
/// (group **C**) by construction and are declared here, by the gate, rather
/// than in the mode table.
const SELECTORS: &[&str] = &["ActiveCktElement", "AllElementNames", "SetActiveElement"];

/// Python attribute names that look like an API read (capitalized) but are not
/// one. `DSSException` is the dss-python exception class caught by the priming
/// retry `oracle_server.capture_all_elements` delegates to
/// (`_tolerant_read`, shared with `capture_aggregates` since the D7 lane merge
/// of 2026-09-05 — before that the retry was inlined in the gated body, which
/// is why the exemption exists). Kept as the standing exemption for either
/// spelling: an inlined `except _dss.DSSException` inside a gated body must
/// not read as an undeclared quantity read.
const PY_NOT_A_READ: &[&str] = &["DSSException"];

/// Source language of a capture transport, for [`code_of`] and [`code_only`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lang {
    Python,
    Rust,
}

/// One capture body the gate parses.
struct CaptureBody {
    /// The name a composite marker's call site resolves against
    /// ([`helper_key`]).
    key: &'static str,
    /// Repo-relative path.
    file: &'static str,
    /// The function whose body is parsed.
    func: &'static str,
    lang: Lang,
    /// The DDLL family the markers resolve in.
    family: &'static str,
    /// `false` for a body that may not carry markers (a golden generator WP-G1
    /// is forbidden to edit): its reads are recovered structurally and only
    /// [`CaptureBody::declared`] and the A-before-B rule are enforced.
    marked: bool,
    /// Rust only: the receivers whose member accesses count as reads.
    receivers: &'static [&'static str],
    /// Rust only: members of those receivers that are not reads.
    exempt: &'static [&'static str],
    /// The exact sequence of quantities this body reads, in order. For a
    /// marked body this is the marker sequence (helper calls expanded); for an
    /// unmarked body it is the sequence of read names recovered from the
    /// source.
    declared: &'static [&'static str],
}

/// The capi channel's element capture, its helper, and the r4133 channel's
/// four bodies.
///
/// `oracle_server.capture_all_elements` reads `Losses` **before** delegating to
/// `gen_checkpoints.capture_element` (which reads `Powers` then `Currents`):
/// two group-A reads followed by the group-B one. `dss.rs::element_pcl` is the
/// r4133 mirror of exactly that, `element_polar` adds the three G1.3a derived
/// channels, and `element_extras` the four unconditional G1.3d(i) discrete
/// scalars (`NodeOrder`, the conditional fifth, stays at the call site).
const BODIES: &[CaptureBody] = &[
    CaptureBody {
        key: "oracle_server.capture_all_elements",
        file: "tools/oracle/oracle_server.py",
        func: "capture_all_elements",
        lang: Lang::Python,
        family: "CktElement",
        marked: true,
        receivers: &[],
        exempt: PY_NOT_A_READ,
        declared: &[
            "ActiveCktElement",
            "AllElementNames",
            "SetActiveElement",
            "Enabled",
            "Losses",
            "Powers",
            "Currents",
            "CurrentsMagAng",
            "Residuals",
            "VoltagesMagAng",
            "NumTerminals",
            "NumConductors",
            "NumPhases",
            "EnergyMeter",
            "NodeOrder",
        ],
    },
    CaptureBody {
        key: "capture_element",
        file: "tools/golden/gen_checkpoints.py",
        func: "capture_element",
        lang: Lang::Python,
        family: "CktElement",
        // The golden generators are frozen for the whole of WP-G1
        // (`GOLDEN_REBASE_PLAN.md` §1.2), so this body carries no markers and
        // is checked structurally instead — and its call site's composite
        // marker is checked against it (rule 5).
        marked: false,
        receivers: &[],
        exempt: PY_NOT_A_READ,
        declared: &["SetActiveElement", "ActiveCktElement", "Powers", "Currents"],
    },
    CaptureBody {
        key: "element_pcl",
        file: "crates/dss-epri/src/dss.rs",
        func: "element_pcl",
        lang: Lang::Rust,
        family: "CktElement",
        marked: true,
        receivers: &["self"],
        // Not a read of the element: it drains the DLL's error slot.
        exempt: &["poll_error"],
        declared: &["Losses", "Powers", "Currents"],
    },
    CaptureBody {
        key: "element_polar",
        file: "crates/dss-epri/src/dss.rs",
        func: "element_polar",
        lang: Lang::Rust,
        family: "CktElement",
        marked: true,
        receivers: &["self"],
        exempt: &["poll_error"],
        declared: &["CurrentsMagAng", "Residuals", "VoltagesMagAng"],
    },
    CaptureBody {
        key: "element_extras",
        file: "crates/dss-epri/src/dss.rs",
        func: "element_extras",
        lang: Lang::Rust,
        family: "CktElement",
        marked: true,
        receivers: &["self"],
        // Not a read of the element: it drains the DLL's error slot
        // (`Engine::check_read`), the same role `poll_error` plays above.
        exempt: &["assert_clean"],
        declared: &["NumTerminals", "NumConductors", "NumPhases", "EnergyMeter"],
    },
    CaptureBody {
        key: "capture.capture_all_elements",
        file: "crates/dss-epri/src/capture.rs",
        func: "capture_all_elements",
        lang: Lang::Rust,
        family: "CktElement",
        marked: true,
        receivers: &["engine"],
        // Drains the DLL's error slot after the one conditional read that does
        // not go through a helper (`NodeOrder`), so an errno is attributed to
        // its own element instead of to the next one's `element_pcl`.
        exempt: &["assert_clean"],
        declared: &[
            "AllElementNames",
            "SetActiveElement",
            "Enabled",
            "Losses",
            "Powers",
            "Currents",
            "CurrentsMagAng",
            "Residuals",
            "VoltagesMagAng",
            "NumTerminals",
            "NumConductors",
            "NumPhases",
            "EnergyMeter",
            "NodeOrder",
        ],
    },
];

fn body(key: &str) -> &'static CaptureBody {
    BODIES
        .iter()
        .find(|b| b.key == key)
        .unwrap_or_else(|| panic!("no capture body declared under the key `{key}`"))
}

/// The capture body a read delegates to, if the read is a helper call.
fn helper_key(member: &str) -> Option<&'static str> {
    match member {
        "capture_element" => Some("capture_element"),
        "element_pcl" => Some("element_pcl"),
        "element_polar" => Some("element_polar"),
        "element_extras" => Some("element_extras"),
        _ => None,
    }
}

/// What a helper call's composite marker must spell: the helper's own declared
/// sequence with the selectors dropped (a selector is the caller's business —
/// `capture_element` re-selects the element it was handed by name).
fn helper_composite(key: &str) -> Vec<String> {
    body(key)
        .declared
        .iter()
        .filter(|n| !SELECTORS.contains(n))
        .map(|n| n.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// body extraction
// ---------------------------------------------------------------------------

/// The text of a Python function body, docstring removed.
///
/// The docstrings of both gated Python bodies quote API names
/// (`` `CktElement.Currents` ``) and the marker template itself, so they are
/// dropped before anything is parsed; the marker grammar additionally requires
/// a `#` comment lead, which no docstring carries.
fn python_body(src: &str, func: &str) -> String {
    let head = format!("def {func}(");
    let lines: Vec<&str> = src.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with(&head))
        .unwrap_or_else(|| panic!("`def {func}(` not found"));
    // The body runs to the next *statement* at column 0. A multi-line
    // signature puts its closing `) -> list:` at column 0 too, which is why
    // the end marker has to start with an identifier, `@` or a keyword rather
    // than merely being unindented.
    let mut end = lines.len();
    for (i, l) in lines.iter().enumerate().skip(start + 1) {
        let starts_statement = l
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '@');
        if starts_statement {
            end = i;
            break;
        }
    }
    let text = lines[start..end].join("\n");
    match text.find("\"\"\"") {
        None => text,
        Some(a) => {
            let rel = text[a + 3..]
                .find("\"\"\"")
                .unwrap_or_else(|| panic!("unterminated docstring in `{func}`"));
            format!("{}{}", &text[..a], &text[a + 3 + rel + 3..])
        }
    }
}

/// The text of a Rust function body — the braces of `fn <func>(…) … { … }`,
/// matched while skipping string literals and line comments so a `format!`
/// argument cannot unbalance the count.
fn rust_body(src: &str, func: &str) -> String {
    let head = format!("fn {func}(");
    let at = src
        .find(&head)
        .unwrap_or_else(|| panic!("`fn {func}(` not found"));
    let open = at
        + src[at..]
            .find('{')
            .unwrap_or_else(|| panic!("no body brace after `fn {func}(`"));
    let b = src.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    let mut in_str = false;
    let mut esc = false;
    while i < b.len() {
        let c = b[i] as char;
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < b.len() && b[i + 1] as char == '/' {
            while i < b.len() && b[i] as char != '\n' {
                i += 1;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return src[open..=i].to_string();
                }
            }
            _ => {}
        }
        i += 1;
    }
    panic!("unbalanced braces in the body of `{func}`");
}

fn body_text(b: &CaptureBody) -> String {
    let path = repo_root().join(b.file);
    let src =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    match b.lang {
        Lang::Python => python_body(&src, b.func),
        Lang::Rust => rust_body(&src, b.func),
    }
}

// ---------------------------------------------------------------------------
// line scanning
// ---------------------------------------------------------------------------

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Every `receiver.member` pair on a line, in order.
fn dotted(line: &str) -> Vec<(String, String)> {
    let ch: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    for (i, &c) in ch.iter().enumerate() {
        if c != '.' || i == 0 {
            continue;
        }
        let mut a = i;
        while a > 0 && is_ident(ch[a - 1]) {
            a -= 1;
        }
        let mut z = i + 1;
        while z < ch.len() && is_ident(ch[z]) {
            z += 1;
        }
        if a == i || z == i + 1 {
            continue;
        }
        out.push((
            ch[a..i].iter().collect::<String>(),
            ch[i + 1..z].iter().collect::<String>(),
        ));
    }
    out
}

/// The code part of a line — everything before its comment lead. Markers live
/// in the comment, reads never do.
fn code_of(line: &str, lang: Lang) -> &str {
    match lang {
        Lang::Python => line.split_once('#').map_or(line, |(c, _)| c),
        Lang::Rust => line.split_once("//").map_or(line, |(c, _)| c),
    }
}

/// The reads a line performs.
///
/// Python: every attribute whose name is capitalized — the dss-python API
/// convention for a property/method on `ActiveCircuit`/`ActiveCktElement` —
/// plus a `gc.capture_*` delegation. Deliberately receiver-agnostic so a read
/// introduced through a *new* receiver cannot slip past.
///
/// Rust: every member of one of the body's declared receivers that is not on
/// its exempt list, i.e. the strictest possible rule for that receiver.
fn reads_in(line: &str, b: &CaptureBody) -> Vec<String> {
    let code = code_of(line, b.lang);
    let mut out = Vec::new();
    for (recv, member) in dotted(code) {
        let hit = match b.lang {
            Lang::Python => {
                (member.starts_with(|c: char| c.is_ascii_uppercase())
                    && !b.exempt.contains(&member.as_str()))
                    || (recv == "gc" && member.starts_with("capture_"))
            }
            Lang::Rust => {
                b.receivers.contains(&recv.as_str()) && !b.exempt.contains(&member.as_str())
            }
        };
        if hit {
            out.push(member);
        }
    }
    out
}

/// The `capture-order: <Name> (<A|B|C>), …` markers a line declares.
///
/// `Err` is a malformed marker, which is a failure in its own right — the gate
/// never silently ignores something that was meant to be a declaration.
fn markers_in(line: &str) -> Result<Vec<(String, char)>, String> {
    let Some(at) = line.find("capture-order:") else {
        return Ok(Vec::new());
    };
    let lead = &line[..at];
    if !lead.contains('#') && !lead.contains("//") {
        return Err(format!(
            "`capture-order:` outside a comment: {}",
            line.trim()
        ));
    }
    let mut out = Vec::new();
    for chunk in line[at + "capture-order:".len()..].split(',') {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            return Err(format!("empty marker in: {}", line.trim()));
        }
        let Some((name, rest)) = chunk.split_once('(') else {
            return Err(format!("marker without a group: `{chunk}`"));
        };
        let rest = rest.trim();
        let group = rest.chars().next().unwrap_or('?');
        if !matches!(group, 'A' | 'B' | 'C') || rest != format!("{group})") {
            return Err(format!(
                "marker `{}` must end in `(A)`, `(B)` or `(C)`, found `({rest}`",
                name.trim()
            ));
        }
        out.push((name.trim().to_string(), group));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// the checker
// ---------------------------------------------------------------------------

/// One read, with the markers it consumed.
struct Consumed {
    member: String,
    markers: Vec<(String, char)>,
}

/// Every capture-order violation in `text`, read as the body of `b`.
///
/// Violation strings are prefixed by kind (`order:`, `unmarked:`, `group:`,
/// `unknown:`, `sequence:`, `helper:`, `marker:`, `count:`) so the non-vacuity
/// tests can assert that the *intended* rule fired, not merely that something
/// failed.
fn check(b: &CaptureBody, text: &str) -> Vec<String> {
    let mut bad = Vec::new();
    let mut pending: Vec<(String, char)> = Vec::new();
    let mut reads: Vec<Consumed> = Vec::new();

    for (no, line) in text.lines().enumerate() {
        let mine = match markers_in(line) {
            Ok(m) => m,
            Err(e) => {
                bad.push(format!("marker: {}:{} {e}", b.file, no + 1));
                Vec::new()
            }
        };
        let found = reads_in(line, b);
        if found.is_empty() {
            pending.extend(mine);
            continue;
        }
        if !mine.is_empty() && !pending.is_empty() {
            bad.push(format!(
                "marker: {}:{} a read line carries its own marker while {} marker(s) are still \
                 pending from the line(s) above",
                b.file,
                no + 1,
                pending.len()
            ));
        }
        let mut own = if mine.is_empty() {
            std::mem::take(&mut pending)
        } else {
            pending.clear();
            mine
        };
        for (idx, member) in found.into_iter().enumerate() {
            let markers = if idx == 0 {
                std::mem::take(&mut own)
            } else {
                Vec::new()
            };
            if b.marked && markers.is_empty() {
                bad.push(format!(
                    "unmarked: {}:{} read `{member}` carries no `capture-order:` marker — every \
                     read inside a capture body must declare its D3 group",
                    b.file,
                    no + 1
                ));
            }
            reads.push(Consumed { member, markers });
        }
    }
    if !pending.is_empty() {
        bad.push(format!(
            "marker: {} ends with {} dangling marker(s) that no read consumed",
            b.file,
            pending.len()
        ));
    }

    // The ordered sequence of quantities this body reads: the marker names for
    // a marked body, the read names for an unmarked one.
    let seq: Vec<(String, char)> = if b.marked {
        reads.iter().flat_map(|r| r.markers.clone()).collect()
    } else {
        reads
            .iter()
            .map(|r| {
                let g = capture_group_of(b.family, &r.member).unwrap_or('C');
                (r.member.clone(), g)
            })
            .collect()
    };

    // 1. every marker's group is the mode table's (or a declared selector's C).
    for (name, group) in &seq {
        match capture_group_of(b.family, name) {
            Some(g) if g == *group => {}
            Some(g) => bad.push(format!(
                "group: {} declares `{name}` as group {group}, but \
                 `dss_epri::modes::capture_group_of(\"{}\", \"{name}\")` says {g} — the mode \
                 table is the one home of that mapping",
                b.file, b.family
            )),
            None if SELECTORS.contains(&name.as_str()) && *group == 'C' => {}
            None if SELECTORS.contains(&name.as_str()) => bad.push(format!(
                "group: {} declares the selector `{name}` as group {group}; a selector moves a \
                 cursor and is order-free (C)",
                b.file
            )),
            None => bad.push(format!(
                "unknown: {} names `{name}`, which has no row in \
                 `dss_epri::modes` and is not a declared selector — add the Pascal `case` arm to \
                 the mode table, or declare the selector in SELECTORS",
                b.file
            )),
        }
    }

    // 2. the sequence is exactly the declared one.
    let names: Vec<&str> = seq.iter().map(|(n, _)| n.as_str()).collect();
    if names != b.declared {
        bad.push(format!(
            "sequence: {}::{} reads {names:?}, declared {:?}",
            b.file, b.func, b.declared
        ));
    }

    // 3. every group-A read precedes every group-B read.
    let last_a = seq.iter().rposition(|(_, g)| *g == 'A');
    let first_b = seq.iter().position(|(_, g)| *g == 'B');
    if let (Some(a), Some(bi)) = (last_a, first_b)
        && a > bi
    {
        bad.push(format!(
            "order: {}::{} reads the group-A `{}` (position {a}) after the group-B `{}` \
             (position {bi}) — a group-B read stamps IterminalSolutionCount without filling \
             Iterminal (PCElements/PCElement.pas:247-266), so the cache-aware read that follows \
             can answer stale (Common/CktElement.pas:632-639)",
            b.file, b.func, seq[a].0, seq[bi].0
        ));
    }

    // 4. a helper call declares that helper's own order; every other read
    //    declares exactly one quantity.
    for r in &reads {
        match helper_key(&r.member) {
            Some(k) => {
                let want = helper_composite(k);
                let got: Vec<String> = r.markers.iter().map(|(n, _)| n.clone()).collect();
                if got != want {
                    bad.push(format!(
                        "helper: {} declares the call to `{}` as {got:?}, but that body reads \
                         {want:?}",
                        b.file, r.member
                    ));
                }
            }
            None if b.marked && r.markers.len() != 1 => bad.push(format!(
                "count: {} lets the read `{}` declare {} markers; only a call into another \
                 capture body may declare more than one",
                b.file,
                r.member,
                r.markers.len()
            )),
            None => {}
        }
    }
    bad
}

// ---------------------------------------------------------------------------
// the gate
// ---------------------------------------------------------------------------

#[test]
fn every_capture_body_honours_the_d3_capture_order() {
    let mut bad = Vec::new();
    for b in BODIES {
        bad.extend(check(b, &body_text(b)));
    }
    assert!(
        bad.is_empty(),
        "capture-order contract violated (GOLDEN_REBASE_PLAN.md §1.1(a), D3):\n  {}",
        bad.join("\n  ")
    );
}

#[test]
fn the_declared_selectors_are_not_quantity_reads() {
    for s in SELECTORS {
        assert_eq!(
            capture_group_of("CktElement", s),
            None,
            "`{s}` is declared as a selector by this gate but the mode table has a row for it — \
             the row wins; drop it from SELECTORS so its group comes from \
             `ModeEffect::capture_group()`"
        );
    }
}

/// Both live channels must capture the *same* quantities in the *same* order —
/// otherwise the two oracles are compared against the Rust snapshot through
/// different read paths and a divergence could be an artefact of the capture.
/// Selectors are dropped: the capi facade is hoisted out of the loop
/// (`el = ckt.ActiveCktElement`) while the r4133 bridge has no analogue.
#[test]
fn the_two_channels_capture_the_same_quantities_in_the_same_order() {
    let strip = |b: &CaptureBody| -> Vec<&str> {
        b.declared
            .iter()
            .copied()
            .filter(|n| !SELECTORS.contains(n))
            .collect()
    };
    assert_eq!(
        strip(body("oracle_server.capture_all_elements")),
        strip(body("capture.capture_all_elements")),
        "the capi and r4133 element captures disagree on what, or in which order, they read"
    );
}

// ---------------------------------------------------------------------------
// non-vacuity: each rule is shown to bite on a corrupted copy of a real body
// ---------------------------------------------------------------------------

fn capi() -> (&'static CaptureBody, String) {
    let b = body("oracle_server.capture_all_elements");
    (b, body_text(b))
}

fn line_index(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no line containing `{needle}`"))
}

/// Move the line containing `what` to just after the line containing `after`.
fn move_line_after(text: &str, what: &str, after: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let from = line_index(text, what);
    let moved = lines.remove(from);
    let mut to = lines
        .iter()
        .position(|l| l.contains(after))
        .unwrap_or_else(|| panic!("no line containing `{after}`"));
    to += 1;
    lines.insert(to, moved);
    lines.join("\n")
}

/// Insert `line` just after the line containing `after`.
fn insert_after(text: &str, after: &str, line: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let at = line_index(text, after);
    lines.insert(at + 1, line.to_string());
    lines.join("\n")
}

fn drop_line(text: &str, what: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let at = line_index(text, what);
    lines.remove(at);
    lines.join("\n")
}

fn kinds(bad: &[String]) -> Vec<&str> {
    bad.iter()
        .map(|v| v.split_once(':').map_or("?", |(k, _)| k))
        .collect()
}

fn assert_fires(bad: &[String], kind: &str) {
    assert!(
        kinds(bad).contains(&kind),
        "expected a `{kind}:` violation, got {bad:?}"
    );
}

fn assert_silent(bad: &[String], kind: &str) {
    assert!(
        !kinds(bad).contains(&kind),
        "expected NO `{kind}:` violation, got {bad:?}"
    );
}

#[test]
fn the_gate_fires_when_a_group_a_read_moves_after_a_group_b_read() {
    let (b, text) = capi();
    assert!(check(b, &text).is_empty(), "the real body must be clean");
    // `Losses` back where it sat before G1.3a — after `capture_element`, i.e.
    // after the group-B `Currents` read.
    let bad = check(
        b,
        &move_line_after(&text, "capture-order: Losses (A)", "gc.capture_element"),
    );
    assert_fires(&bad, "order");
}

/// The positive half of the A-before-B rule, which no other test states: a
/// group-**C** read is order-free *by construction* (`ModeEffect::Pure` — it
/// reads a field and never runs `ComputeIterminal` or `GetCurrents`), so moving
/// one into the middle of the A/B block cannot violate the D3 order. Moving the
/// G1.3d(i) `NumPhases` read (`CktElementI(2)`, `DDLL/DCktElement.pas:149`) to
/// sit between the group-A `Losses` and the group-A/B `capture_element`
/// delegation raises only the declaration-bookkeeping violation — proof the
/// mutation really landed — and never an `order:` one.
#[test]
fn a_group_c_read_may_sit_between_a_group_a_and_a_group_b_read() {
    let (b, text) = capi();
    assert!(check(b, &text).is_empty(), "the real body must be clean");
    let moved = move_line_after(
        &text,
        "capture-order: NumPhases (C)",
        "capture-order: Losses (A)",
    );
    assert_ne!(moved, text, "the mutation must actually move a line");
    let bad = check(b, &moved);
    assert_silent(&bad, "order");
    assert_fires(&bad, "sequence");
}

/// Both directions of rule 4: stripping a marker off an existing read, and —
/// the one that matters for a later sub-step — smuggling a brand-new read into
/// a capture body without declaring it.
#[test]
fn the_gate_fires_on_an_unmarked_read() {
    let (b, text) = capi();
    let bad = check(b, &text.replace("  # capture-order: Residuals (B)", ""));
    assert_fires(&bad, "unmarked");

    // A new capi read, silently added.
    let bad = check(
        b,
        &insert_after(&text, "el.Enabled", "        probe = el.TotalPowers"),
    );
    assert_fires(&bad, "unmarked");

    // The same on the r4133 side, where the rule is "every member of the
    // body's declared receiver that is not on its exempt list".
    let r = body("element_pcl");
    let rt = body_text(r);
    assert!(check(r, &rt).is_empty(), "the real body must be clean");
    let bad = check(
        r,
        &insert_after(
            &rt,
            "capture-order: Losses (A)",
            "            self.element_yprim();",
        ),
    );
    assert_fires(&bad, "unmarked");
}

#[test]
fn the_gate_fires_on_a_malformed_marker() {
    let (b, text) = capi();
    let bad = check(
        b,
        &text.replace("capture-order: Losses (A)", "capture-order: Losses A"),
    );
    assert_fires(&bad, "marker");
}

#[test]
fn the_gate_fires_on_a_marker_no_read_consumes() {
    let (b, text) = capi();
    let bad = check(
        b,
        &insert_after(
            &text,
            "out.append(cap)",
            "        # capture-order: TotalPowers (A)",
        ),
    );
    assert_fires(&bad, "marker");

    // The other half of the same rule: a pending marker that the next read
    // does not consume because that read already declares itself.
    let bad = check(
        b,
        &insert_after(
            &text,
            "el.Enabled",
            "        # capture-order: TotalPowers (A)",
        ),
    );
    assert_fires(&bad, "marker");
}

/// Only a call into another capture body may declare more than one read.
#[test]
fn the_gate_fires_when_a_plain_read_declares_two_quantities() {
    let (b, text) = capi();
    let bad = check(
        b,
        &text.replace(
            "capture-order: Losses (A)",
            "capture-order: Losses (A), Powers (A)",
        ),
    );
    assert_fires(&bad, "count");
}

#[test]
fn the_gate_fires_when_a_marker_contradicts_the_mode_table() {
    let (b, text) = capi();
    let bad = check(
        b,
        &text.replace("capture-order: Losses (A)", "capture-order: Losses (C)"),
    );
    assert_fires(&bad, "group");
}

#[test]
fn the_gate_fires_on_a_read_the_mode_table_does_not_know() {
    let (b, text) = capi();
    let bad = check(
        b,
        &text.replace("capture-order: Losses (A)", "capture-order: Lozzes (A)"),
    );
    assert_fires(&bad, "unknown");
}

#[test]
fn the_gate_fires_when_a_read_disappears() {
    let (b, text) = capi();
    let bad = check(b, &drop_line(&text, "el.Residuals"));
    assert_fires(&bad, "sequence");
}

/// The rule that covers `gen_checkpoints.capture_element`, which carries no
/// markers of its own: its call site's composite must spell that body's real
/// read order.
#[test]
fn the_gate_fires_when_a_helper_call_misdeclares_the_helpers_order() {
    let (b, text) = capi();
    let bad = check(
        b,
        &text.replace(
            "capture-order: Powers (A), Currents (B)",
            "capture-order: Currents (B), Powers (A)",
        ),
    );
    assert_fires(&bad, "helper");

    // Same rule on the r4133 side, where the helper is a Rust fn.
    let r = body("capture.capture_all_elements");
    let rt = body_text(r);
    let bad = check(
        r,
        &rt.replace(
            "capture-order: Losses (A), Powers (A), Currents (B)",
            "capture-order: Powers (A), Losses (A), Currents (B)",
        ),
    );
    assert_fires(&bad, "helper");
}

// ---------------------------------------------------------------------------
// The per-step call order (GOLDEN_REBASE G1.9, folded in at the D7 lane merge)
// ---------------------------------------------------------------------------

/// The six call sites the call-order gate locates inside one transport's
/// `run_case`.
struct Anchors {
    /// Start of the per-step capture function itself.
    run: &'static str,
    /// The G1.9 group-A read.
    aggregates: &'static str,
    /// The per-element capture — the group-B (`Currents`) read.
    elements: &'static str,
    /// The discrete-state capture, which drives `Transformers.First/Next`.
    discrete: &'static str,
    /// G1.6(i)'s reliability capture — driven between the meter walk and
    /// the PD-element walk (its own slot gate lives in
    /// `crates/dss-core/tests/reliability_pins.rs`); named here so the
    /// topology-last rule below covers it too.
    reliability: &'static str,
    /// The WP8.5b property sweep, read after every established capture.
    properties: &'static str,
    /// G1.7's topology capture — read after `properties`, i.e. last of all.
    topology: &'static str,
}

fn read_source(rel: &str) -> String {
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

/// The whole call-order gate as a pure function of one transport's source text,
/// so the rule itself can be shown to have teeth
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
///
/// The element anchor is the bare call `capture_all_elements(` rather than
/// `…(ckt`: G1.3a's `derived` argument wrapped the call over three lines, and an
/// anchor that names the first argument would have gone silently unfindable —
/// which `offset_after` turns into a loud failure, not a pass, but is still a
/// gate that has to be edited for a reformat. Its `def` is outside the search
/// window (the scan starts at `def run_case(`), and the one prose mention
/// inside the body carries no paren.
const CAPI_CALLS: Anchors = Anchors {
    run: "def run_case(",
    aggregates: "capture_aggregates(ckt",
    elements: "capture_all_elements(",
    discrete: "gc.capture_discrete(ckt",
    reliability: "capture_reliability(ckt",
    properties: "capture_all_properties(d, ckt",
    topology: "capture_topology(ckt",
};

/// The EPRI r4133 transport (`r4133` channel), which must capture in the exact
/// same order or the two channels would not be comparing the same reads.
///
/// `capture_all_elements` reaches `Currents` through `Engine::element_pcl`
/// (powers, then currents, then losses).
const R4133_CALLS: Anchors = Anchors {
    run: "pub fn run_case(",
    aggregates: "capture_aggregates(engine",
    elements: "capture_all_elements(engine",
    discrete: "capture_discrete(engine",
    reliability: "capture_reliability(engine",
    properties: "capture_all_properties(engine",
    topology: "capture_topology(engine",
};

#[test]
fn capi_capture_reads_the_aggregates_before_any_currents_read() {
    let rel = "tools/oracle/oracle_server.py";
    check_order(&read_source(rel), &CAPI_CALLS, rel).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn r4133_capture_reads_the_aggregates_before_any_currents_read() {
    let rel = "crates/dss-epri/src/capture.rs";
    check_order(&read_source(rel), &R4133_CALLS, rel).unwrap_or_else(|e| panic!("{e}"));
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
        reliability: "capture_reliability(",
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

    let renamed = "fn run_case( grab_the_aggregates(x); capture_discrete(x); \
                   capture_all_elements(x);";
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
        ("the G1.6(i) reliability capture", a.reliability),
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
    check_topology_last(&read_source(rel), &CAPI_CALLS, rel).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn r4133_capture_reads_the_topology_surface_last() {
    let rel = "crates/dss-epri/src/capture.rs";
    check_topology_last(&read_source(rel), &R4133_CALLS, rel).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn capi_capture_never_touches_an_itopology_cursor_row() {
    let rel = "tools/oracle/oracle_server.py";
    check_capi_topology_rows(&read_source(rel), rel).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn r4133_capture_never_calls_a_topology_cursor_accessor() {
    let rel = "crates/dss-epri/src/capture.rs";
    check_r4133_topology_rows(
        &read_source(rel),
        rel,
        "the set of `topology_*` accessors called",
    )
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
    check_r4133_topology_rows(
        &read_source(rel),
        rel,
        "the set of `topology_*` accessors bound",
    )
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
        reliability: "capture_reliability(",
        properties: "capture_all_properties(",
        topology: "capture_topology(",
    };
    let ok = "fn run_case( capture_aggregates(x); capture_discrete(x); \
              capture_all_elements(x); capture_reliability(x); capture_all_properties(x); \
              capture_topology(x);";
    assert!(check_topology_last(ok, &a, "synthetic").is_ok());

    let early = "fn run_case( capture_aggregates(x); capture_discrete(x); capture_topology(x); \
                 capture_all_elements(x); capture_reliability(x); \
                 capture_all_properties(x);";
    let err = check_topology_last(early, &a, "synthetic").expect_err("topology ran first");
    assert!(err.contains("property sweep"), "{err}");

    // ...and the same rule now covers the G1.6(i) slot: a topology read placed
    // before the reliability capture would take that capture's `Meters.Totals`
    // tail on a tree-rewritten circuit, so it is rejected too.
    let before_rel = "fn run_case( capture_aggregates(x); capture_discrete(x); \
                      capture_all_elements(x); capture_all_properties(x); \
                      capture_topology(x); capture_reliability(x);";
    let err = check_topology_last(before_rel, &a, "synthetic").expect_err("topology ran first");
    assert!(err.contains("reliability capture"), "{err}");

    let renamed = "fn run_case( capture_aggregates(x); capture_discrete(x); \
                   capture_all_elements(x); capture_reliability(x); \
                   capture_all_properties(x); grab_topology(x);";
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
