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
/// `oracle_server.capture_all_elements` reads `PhaseLosses` and then `Losses`
/// **before** delegating to `gen_checkpoints.capture_element` (which reads
/// `Powers` then `Currents`): three group-A reads followed by the group-B one.
/// `dss.rs::element_phase_losses` + `element_pcl` are the r4133 mirror of
/// exactly that, `element_polar` adds the three G1.3a derived channels, and
/// `element_extras` the nine unconditional G1.3d discrete scalars — the four of
/// part (i) plus the five control-derived ones of part (ii) (`NodeOrder`, the
/// conditional tenth, stays at the call site).
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
            "PhaseLosses",
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
            "NumControls",
            "OCPDevIndex",
            "OCPDevType",
            "HasVoltControl",
            "HasSwitchControl",
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
        key: "element_phase_losses",
        file: "crates/dss-epri/src/dss.rs",
        func: "element_phase_losses",
        lang: Lang::Rust,
        family: "CktElement",
        marked: true,
        receivers: &["self"],
        // Not a read of the element: it drains the DLL's error slot.
        exempt: &["poll_error"],
        declared: &["PhaseLosses"],
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
        declared: &[
            "NumTerminals",
            "NumConductors",
            "NumPhases",
            "EnergyMeter",
            "NumControls",
            "OCPDevIndex",
            "OCPDevType",
            "HasVoltControl",
            "HasSwitchControl",
        ],
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
            "PhaseLosses",
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
            "NumControls",
            "OCPDevIndex",
            "OCPDevType",
            "HasVoltControl",
            "HasSwitchControl",
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
        "element_phase_losses" => Some("element_phase_losses"),
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

/// The A-before-B rule stated for the surface GOLDEN_REBASE G1.3d(ii) adds:
/// `PhaseLosses` (`CktElementV(6)`, r4133 `DDLL/DCktElement.pas:637`) is
/// `TDSSCktElement.GetPhaseLosses`, whose first act is `ComputeIterminal`
/// (r4133 `Common/CktElement.pas:1090`, capi `Common/CktElement.pas:896`) — so
/// it is group **A** and must precede the group-B `Currents`.
///
/// The mistake this guards is the natural one: appending the read to the
/// `element_extras` tail, where the other nine G1.3d scalars live, instead of
/// hoisting it to the head of the element. Moving it past the `capture_element`
/// delegation on the capi transport, and past `element_pcl` on the r4133 one,
/// must fire `order:` on both.
#[test]
fn the_gate_fires_when_phase_losses_moves_after_the_currents_read() {
    let (b, text) = capi();
    assert!(check(b, &text).is_empty(), "the real body must be clean");
    let moved = move_line_after(&text, "el.PhaseLosses", "gc.capture_element");
    assert_ne!(moved, text, "the mutation must actually move a line");
    assert_fires(&check(b, &moved), "order");

    let r = body("capture.capture_all_elements");
    let rt = body_text(r);
    assert!(check(r, &rt).is_empty(), "the real body must be clean");
    let moved = move_line_after(&rt, "engine.element_phase_losses", "engine.element_pcl");
    assert_ne!(moved, rt, "the mutation must actually move a line");
    assert_fires(&check(r, &moved), "order");
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

/// The seven call sites the call-order gate locates inside one transport's
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
    /// G1.7's topology capture — read after `properties`, i.e. last of the
    /// order-free *reads*.
    topology: &'static str,
    /// G1.8's flat incidence capture — after `topology`, i.e. strictly last in
    /// the step. It is the only capture that WRITES solution state.
    inc_matrix: &'static str,
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
    inc_matrix: "capture_inc_matrix(d, ckt",
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
    inc_matrix: "capture_inc_matrix(engine",
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

    let renamed = "fn run_case( grab_the_aggregates(x); capture_discrete(x); \
                   capture_all_elements(x);";
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
        inc_matrix: "capture_inc_matrix(",
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
///
/// The line terminator is read as LF **or** CRLF: this repo checks Rust out with
/// `core.autocrlf`, so a CRLF working copy must not make the gate claim the
/// closing brace is missing (it did, at the G1.8 lane merge).
fn rust_fn_body<'a>(text: &'a str, header: &str, rel: &str) -> Result<&'a str, String> {
    let at = text
        .find(header)
        .ok_or_else(|| format!("{rel}: `{header}` is gone — update this gate"))?;
    let end = text[at..]
        .match_indices("\n}")
        .find(|&(i, _)| {
            matches!(
                text.as_bytes().get(at + i + 2).copied(),
                None | Some(b'\n') | Some(b'\r')
            )
        })
        .map(|(i, _)| at + i + 1)
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
    check_capi_incidence_normalizers(&read_source(rel), rel).unwrap_or_else(|e| panic!("{e}"));

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
    check_inc_matrix_last(&read_source(rel), &CAPI_CALLS, rel).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn r4133_capture_reads_the_incidence_surface_last() {
    let rel = "crates/dss-epri/src/capture.rs";
    check_inc_matrix_last(&read_source(rel), &R4133_CALLS, rel).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn the_incidence_capture_issues_calcincmatrix_then_calclaplacian() {
    let capi = "tools/oracle/oracle_server.py";
    check_capi_incidence(&read_source(capi), capi).unwrap_or_else(|e| panic!("{e}"));
    let r4133 = "crates/dss-epri/src/capture.rs";
    check_r4133_incidence(&read_source(r4133), r4133).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn neither_capture_calls_calcincmatrix_o_or_reads_buslevels() {
    let capi = "tools/oracle/oracle_server.py";
    let src = read_source(capi);
    check_no_forbidden_incidence_use(&src, Lang::Python, capi, &py_commands(&src))
        .unwrap_or_else(|e| panic!("{e}"));

    for r4133 in [
        "crates/dss-epri/src/capture.rs",
        "crates/dss-epri/src/dss.rs",
    ] {
        let src = read_source(r4133);
        check_no_forbidden_incidence_use(&src, Lang::Rust, r4133, &rust_commands(&src))
            .unwrap_or_else(|e| panic!("{e}"));
    }

    let modes = "crates/dss-epri/src/modes.rs";
    check_do_not_call_still_refuses_bus_levels(&read_source(modes), modes)
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
        reliability: "capture_reliability(",
        properties: "capture_all_properties(",
        topology: "capture_topology(",
        inc_matrix: "capture_inc_matrix(",
    };
    let ok = "fn run_case( capture_aggregates(x); capture_discrete(x); capture_all_elements(x); \
              capture_reliability(x); capture_all_properties(x); capture_topology(x); \
              capture_inc_matrix(x);";
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
