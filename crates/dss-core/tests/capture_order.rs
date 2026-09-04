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
/// one. `DSSException` is the dss-python exception class caught by
/// `oracle_server.capture_all_elements`'s `_read` retry wrapper.
const PY_NOT_A_READ: &[&str] = &["DSSException"];

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
