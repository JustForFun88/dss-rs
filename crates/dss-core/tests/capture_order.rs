//! Source gate for the **capture-order contract** (`GOLDEN_REBASE_PLAN.md`
//! §1.1(a), coordinator decision **D3**): on both live-oracle transports, every
//! *cache-aware* read of an element (group **A**) must be issued **before**
//! every read that computes the element's currents into a scratch buffer
//! (group **B**); everything else (selectors, discrete state, the node-voltage
//! reads) is order-free (group **C**).
//!
//! # Why the order is a contract and not a habit
//!
//! The oracle recomputes an element's cached terminal current **only** when
//! the solution count stamped on that cache differs from the circuit's. On a
//! power-conversion element a group-B read fills the CALLER's scratch buffer
//! and then stamps that count **without** filling the cache. So a group-B read
//! issued first can make a following group-A read (`Losses`, `Powers`, both
//! computed from the cached terminal current) answer from a stale cache. That
//! is the mechanism behind the upstream harmonics `Powers`-after-`Currents`
//! defect this project never reproduces
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
//!    `ModeEffect::capture_group()` of that read's row in the mode table
//!    (G1.3a spec amendment item 1: no second, competing order
//!    table may exist in the repo);
//! 2. the parsed marker sequence is exactly the transport's declared sequence
//!    (so a read that appears, disappears or moves is visible here);
//! 3. every group-A marker precedes every group-B marker;
//! 4. no read line inside a capture body is unmarked — a later sub-step cannot
//!    add an invisible read. A read is any of ([`reads_by_line`]): a
//!    capitalized Python member on any receiver (the dss-python API
//!    convention) or a `gc.capture_*` delegation; a member of a Rust receiver
//!    the body declares, or of a name a `let` binds to one; a call of a
//!    bridge `Engine` method on ANY Rust receiver — a binding, a call chain,
//!    an index, a `?` — or an `Engine::`/`Self::` path to one, called or not
//!    ([`engine_methods`]), so a read through an undeclared binding needs a
//!    marker too; and, in both languages, a receiver handed on
//!    ([`handed_calls`]): to a call (`_read_seq(el)`, `read_seq(engine)`, also
//!    with a space, a turbofish or a macro `!` before the parenthesis, or a
//!    callee that is an expression, `(lambda e: f(e))(el)`), which lets that
//!    function read the model, or anywhere else a receiver may not stand bare
//!    (a tuple, a `for` or comprehension iterable, a walrus, a default value,
//!    a literal). A receiver stands bare only on an alias line or as a
//!    parameter name of a definition, so every name that can reach the model
//!    is a receiver; an engine a body obtains other than from a declared
//!    receiver counts through its member (Python) or `Engine` method (Rust)
//!    reads only;
//! 5. a call into another capture helper declares that helper's reads, in that
//!    helper's own order, and the declaration is cross-checked against the
//!    helper body itself. This is what covers
//!    `tools/golden/gen_checkpoints.py::capture_element`, which carries no
//!    markers because GOLDEN_REBASE WP-G1 may not edit the golden generators
//!    at all (`GOLDEN_REBASE_PLAN.md` §1.2): its reads are recovered
//!    structurally instead and must match the composite marker at its call
//!    site;
//! 6. every other marker names the read it sits on ([`marker_names_read`]):
//!    on the Python transport the member IS the API name, on the Rust one the
//!    accessor is `<family>_<quantity>` in snake case or a declared
//!    [`RUST_READ_ALIASES`] entry. Rule 1 takes the group from the marker's
//!    NAME, so a read labelled with another quantity would borrow that
//!    quantity's group.
//!
//! G1.7's topology surface is group C — the six order-free `Topology` rows
//! (`NumLoops`, `NumIsolatedBranches`, `NumIsolatedLoads`, `AllLoopedPairs`,
//! `AllIsolatedBranches`, `AllIsolatedLoads`) never move the active circuit
//! element — but it carries its own two constraints, and both are asserted
//! here:
//!
//! * it is read after `all_properties` and after every other *reading* capture:
//!   the FIRST `Topology` read is what builds the memoized branch list and
//!   rewrites the checked and isolated flags of every element and bus, and
//!   `TopologyI(1)`/`(2)` + `TopologyV(1)`/`(2)` leave the
//!   `PDElements`/`PCElements` cursors at the end;
//! * the other twelve `ITopology` members are **never touched**. Three of them
//!   are the B16 parity gap (`ActiveLevel`, `BranchName`, `ActiveBranch` of
//!   `origin/fastdss:dss/ITopology.py:10-20`) and the other nine are cursor
//!   rows. On both channels every one of them moves the active circuit
//!   element and would poison the per-element capture of the same step.
//!
//! G1.8's flat incidence surface (`Solution.IncMatrix` / `Laplacian` /
//! `IncMatrixRows` / `IncMatrixCols`) is the one capture that comes after the
//! topology read, i.e. **strictly last in the step**, and it is not a pure read
//! at all: it issues the executive pair `CalcIncMatrix` + `CalcLaplacian` first.
//! Four separate rules, all asserted below:
//!
//! * **last** — `CalcIncMatrix` rebuilds the incidence matrix and its row list
//!   and clears the ordered flag, and while adding the series reactors it
//!   iterates the reactor class, which moves the active class and the active
//!   circuit element. It must therefore follow every per-element, reliability
//!   and property read — and it must follow the topology read too, because
//!   G1.7's two decline censuses are defined on a branch list nothing else has
//!   touched;
//! * **the pair, in that order** — r4133's `CalcLaplacian` multiplies the
//!   transposed incidence matrix by the matrix without checking that it exists,
//!   so issuing it first dereferences a null pointer inside the DLL. capi
//!   refuses the same command with error 8877 and so does the port, which is
//!   why the swap is pinned from the sources here rather than driven live;
//! * **`CalcIncMatrix_O` and `Solution.BusLevels` are never touched** — the
//!   ordered builder builds the topology tree when none is memoized yet, which
//!   would memoize that same branch list, and `SolutionV(2)` writes one element
//!   past its own array on r4133 (on the bridge's `modes::DO_NOT_CALL`
//!   register). Both keep their `tests/golden/inc_matrix/` `org_*` byte goldens
//!   instead (`GOLDEN_REBASE_PLAN.md` §G1.8 / §G3.2c);
//! * **no other command mid-step** — the r4133 transport calls a
//!   command-issuing `Engine` method only at the sites of
//!   [`R4133_COMMAND_SITES`], fn by fn: the run itself, ahead of the step's
//!   first capture, the once-per-case `RelCalc` between the last solve and the
//!   aggregates, the `?` property queries, and this pair, every `exec_wait`
//!   with a literal command.
//!
//! The bodies gated here are the element captures of the two live channels:
//! the pinned dss-python oracle (`tools/oracle/oracle_server.py`, backend
//! dss_capi 0.14.5) and the EPRI r4133 DLL bridge (`crates/dss-epri`).
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
//! The checker is a pure function over body text (plus the `Engine` method
//! names it reads once from the bridge source), so the demo runs on every
//! `cargo test`: each of the twelve mutation tests below feeds a *deliberately
//! corrupted copy* of a real body through the same checker and asserts that the
//! intended rule — not merely *something* — fires. Between them they cover
//! moving `Losses` back after `Currents`, stripping a marker off a read,
//! smuggling in a brand-new undeclared read (on both the Python and the Rust
//! transport, as a member, a free-function call handed the receiver, a call
//! through an undeclared Rust binding, a receiver expression or a path, one
//! through a `let` alias, and a receiver handed to an expression callee or
//! into a binding the gate cannot follow), a
//! malformed marker, a marker no read consumes, a group that contradicts the
//! mode table, a name the table does not know, a read that disappears, a
//! helper call that misdeclares the helper's order, a marker that names
//! another quantity than its read (with `declared` edited to match), and the
//! group-**A** `TotalPowers` moved past the group-B `Currents` on both
//! transports. Two further mutations state the rule's *positive* half — a
//! group-**C** read moved between a group-A and a group-B read raises the
//! declaration-bookkeeping violation but **not** the `order:` one, because C is
//! order-free by construction, and the two live bodies really do declare
//! `TotalPowers` ahead of `Currents`. No file on disk is ever mutated.
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
//! aggregate walks its element list to exhaustion and leaves its cursor at the
//! end). [`check_order`] and its two channel anchor sets carry that rule, and
//! [`the_gate_rejects_a_swapped_or_renamed_capture`] shows it has
//! teeth. Folded here from the G1.9 lane's own `capture_order.rs` at the D7
//! lane merge (2026-09-05), which D3 always assigned to this file.
//!
//! # The third ordering rule: the per-RUN read (G1.10a, D33(3))
//!
//! G1.10a's surface is not a model read at all: it is the SET of files the run
//! created under the case's DataPath, classified by the very `CorpusGuard` pass
//! that sweeps the corpus clean. Being per-**run**, its contract is a position
//! rather than a capture group, and [`check_run_tail_order`] asserts all of it
//! from both transports' source text:
//!
//! * **strictly last of the run** — after every per-step capture above and after
//!   the WPG.5 `autoadd_log` read, which reads one of the created files
//!   (`<CircuitName_>AutoAddLog.csv`) off disk. A report writer of the last step
//!   must already have hit the disk when the set is classified;
//! * **a direct statement of the guard scope** — not nested in the retry loop,
//!   which recompiles in-process and would leave the classification describing
//!   the wrong attempt;
//! * **inside the guard scope** — the sweep removes exactly what the
//!   classification returns, so reading outside it would report a set nothing
//!   removed (the Python guard raises on that by design; the Rust one cannot
//!   express it, `finish()` consuming the guard);
//! * **the demand-interval read sits in front of it** (G1.10c) — see the fourth
//!   rule below;
//! * **nothing but the teardown after it, to the end of `run_case`** (D33(3)) —
//!   no command and no `capture_*` read anywhere after it; inside the guard
//!   scope only the G1.10b contents copy and the teardown, past the scope only
//!   the declared sweep report and a reply built from names already bound. The
//!   one statement the capi transport may still run is the D32(2)(a) teardown
//!   `clear`, which releases dss_capi's Storage trace stream, held open until
//!   the element is destroyed. r4133 closes its own as soon as it writes the
//!   header and needs no counterpart, so its tail is empty. A teardown is not a
//!   read: it comes after the classification, so the compared surface stays the run
//!   `clear → Set DefaultBaseFrequency=60 → compile → post → n × solve` (plus
//!   the last step's `RelCalc` on a reliability case) on both channels, where
//!   the r4133 bridge's `clear` also re-asserts the D39 report switches, which
//!   no compared value reads.
//!
//! # The fourth ordering rule: the demand-interval read (G1.10c, D42(6))
//!
//! G1.10c compares the CONTENTS of the run-created demand-interval tree, read
//! from the same guard in the same tail. Its slot — **after the `autoadd_log`
//! read, before `created()` and before the capi teardown `clear`** — is
//! contractual, not cosmetic, and [`check_run_tail_order`] asserts it from both
//! transports' source text together with everything above:
//!
//! * **before the teardown `clear`** — that `clear` destroys the circuit, which
//!   FLUSHES the in-flight demand-interval cycle over the file on the capi
//!   channel: the R part measured all 8 file digests moving on
//!   `123Bus/Run_YearlySim.dss`, 7 of 8 on `StoCtrl_Current_PeakShave/master.dss`
//!   and 4 of 6 on `StoCtrl_SeasonTarget/Run_example.dss`, while r4133's `clear`
//!   moves none. A DI read after the teardown would compare capi's *next* cycle
//!   against the port's *previous* one — a guaranteed red with a pure-ordering
//!   cause;
//! * **before `created()`** — which stays the LAST read of the run (D33(3));
//! * **inside the guard scope, a direct statement of it** — the sweep removes
//!   exactly the files whose contents were just copied out, and a read nested in
//!   the retry loop would describe an attempt that was thrown away;
//! * **exactly once** — a second read would report one tree while the sidecar
//!   holds another.
//!
//! The upstream harness compares no file set at all — fastdss
//! `tests/compare_outputs.py:517-524` prints a CSV mismatch and carries on — so
//! this rule guards new coverage, not catch-up.
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
use std::sync::OnceLock;

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
    /// The engine handles the body is given. In both languages one of them —
    /// or a name bound to one, [`receivers_of`] — handed on ([`handed_calls`])
    /// is a read; in Rust every member of one is a read too, unless
    /// [`Self::exempt`].
    receivers: &'static [&'static str],
    /// Members (Rust) and callees (both languages) that are not reads.
    exempt: &'static [&'static str],
    /// The exact sequence of quantities this body reads, in order. For a
    /// marked body this is the marker sequence (helper calls expanded); for an
    /// unmarked body it is the sequence of read names recovered from the
    /// source.
    declared: &'static [&'static str],
}

/// The capi channel's element capture and its helper, and the r4133 channel's
/// element capture and its seven `dss.rs` helpers.
///
/// `oracle_server.capture_all_elements` reads `PhaseLosses` and then `Losses`
/// **before** delegating to `gen_checkpoints.capture_element` (which reads
/// `Powers` then `Currents`): three group-A reads followed by the group-B one.
/// `dss.rs::element_phase_losses` + `element_total_powers` + `element_pcl` are
/// the r4133 mirror of exactly that, `element_polar` adds the three G1.3a
/// derived channels, `element_seq` the three G1.3b symmetrical-component ones,
/// `element_cplx_seq` the two G1.3c complex ones, and `element_extras` the nine
/// unconditional G1.3d discrete scalars — the four of part (i) plus the five
/// control-derived ones of part (ii) (`NodeOrder`, the conditional tenth, stays
/// at the call site).
///
/// `TotalPowers` (G1.3c) is the second conditional group-**A** read and gets its
/// own helper for the same reason `PhaseLosses` did: it must be issued at the
/// head of the element, ahead of `element_pcl`'s group-B `Currents`, while
/// `element_pcl` itself is unconditional.
const BODIES: &[CaptureBody] = &[
    CaptureBody {
        key: "oracle_server.capture_all_elements",
        file: "tools/oracle/oracle_server.py",
        func: "capture_all_elements",
        lang: Lang::Python,
        family: "CktElement",
        marked: true,
        // `el` is found as the facade `ckt.ActiveCktElement` hands out.
        receivers: &["ckt"],
        exempt: PY_NOT_A_READ,
        declared: &[
            "ActiveCktElement",
            "AllElementNames",
            "SetActiveElement",
            "Enabled",
            "PhaseLosses",
            "TotalPowers",
            "Losses",
            "Powers",
            "Currents",
            "CurrentsMagAng",
            "Residuals",
            "VoltagesMagAng",
            "SeqPowers",
            "SeqCurrents",
            "SeqVoltages",
            "CplxSeqCurrents",
            "CplxSeqVoltages",
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
        receivers: &["ckt"],
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
        key: "element_total_powers",
        file: "crates/dss-epri/src/dss.rs",
        func: "element_total_powers",
        lang: Lang::Rust,
        family: "CktElement",
        marked: true,
        receivers: &["self"],
        // Not a read of the element: it drains the DLL's error slot.
        exempt: &["poll_error"],
        declared: &["TotalPowers"],
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
        key: "element_seq",
        file: "crates/dss-epri/src/dss.rs",
        func: "element_seq",
        lang: Lang::Rust,
        family: "CktElement",
        marked: true,
        receivers: &["self"],
        // Not a read of the element: it drains the DLL's error slot.
        exempt: &["poll_error"],
        declared: &["SeqPowers", "SeqCurrents", "SeqVoltages"],
    },
    CaptureBody {
        key: "element_cplx_seq",
        file: "crates/dss-epri/src/dss.rs",
        func: "element_cplx_seq",
        lang: Lang::Rust,
        family: "CktElement",
        marked: true,
        receivers: &["self"],
        // Not a read of the element: it drains the DLL's error slot.
        exempt: &["poll_error"],
        declared: &["CplxSeqCurrents", "CplxSeqVoltages"],
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
            "TotalPowers",
            "Losses",
            "Powers",
            "Currents",
            "CurrentsMagAng",
            "Residuals",
            "VoltagesMagAng",
            "SeqPowers",
            "SeqCurrents",
            "SeqVoltages",
            "CplxSeqCurrents",
            "CplxSeqVoltages",
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
        "element_total_powers" => Some("element_total_powers"),
        "element_polar" => Some("element_polar"),
        "element_seq" => Some("element_seq"),
        "element_cplx_seq" => Some("element_cplx_seq"),
        "element_extras" => Some("element_extras"),
        _ => None,
    }
}

/// The bridge accessors whose name is not `<family>_<quantity>` in snake case:
/// the two circuit-level selectors and the three reads of `element_pcl`. Each
/// entry is `(accessor, quantity)`; [`every_rust_read_alias_is_a_bridge_read_of_a_known_quantity`]
/// keeps the table honest.
const RUST_READ_ALIASES: &[(&str, &str)] = &[
    ("all_element_names", "AllElementNames"),
    ("set_active_element", "SetActiveElement"),
    ("element_losses", "Losses"),
    ("element_powers", "Powers"),
    ("element_currents", "Currents"),
];

/// Whether a marker naming `name` names the read `member` it sits on (rule 6).
fn marker_names_read(b: &CaptureBody, member: &str, name: &str) -> bool {
    match b.lang {
        Lang::Python => member == name,
        Lang::Rust => {
            RUST_READ_ALIASES
                .iter()
                .any(|&(m, n)| m == member && n == name)
                || member == format!("{}_{}", snake(b.family), snake(name))
        }
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

/// One `receiver.member` access on a line.
struct Dotted {
    /// Column (char index) where the member starts.
    col: usize,
    /// The receiver's name; empty when the receiver is an expression (a call,
    /// an index, a `?`, a parenthesis or a chain continued from the line above).
    recv: String,
    member: String,
    /// The member is called: `(` (or a turbofish `::`) follows it.
    called: bool,
}

/// Every `receiver.member` access on a line, in order. A range or an ellipsis
/// (`..`, `...`) is no access.
fn dotted(line: &str) -> Vec<Dotted> {
    let ch: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    for (i, &c) in ch.iter().enumerate() {
        if c != '.' || (i > 0 && ch[i - 1] == '.') {
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
        if z == i + 1 {
            continue;
        }
        let next = ch[z..].iter().copied().find(|c| !c.is_whitespace());
        out.push(Dotted {
            col: i + 1,
            recv: ch[a..i].iter().collect(),
            member: ch[i + 1..z].iter().collect(),
            called: matches!(next, Some('(' | ':')),
        });
    }
    out
}

/// The methods of the r4133 bridge's `Engine` ([`engine_fns`]), read once from
/// the source. Every read the bridge performs is one of them, so a call of one
/// on any Rust receiver is a read (rule 4).
fn engine_methods() -> &'static [String] {
    static METHODS: OnceLock<Vec<String>> = OnceLock::new();
    METHODS.get_or_init(|| {
        let out: Vec<String> = engine_fns()
            .iter()
            .filter(|f| f.takes_self)
            .map(|f| f.name.clone())
            .collect();
        for want in ["element_pcl", "ckt_element_node_order", "poll_error"] {
            assert!(
                out.iter().any(|m| m == want),
                "crates/dss-epri/src: the `impl Engine` scan found no `{want}` method — the \
                 scan is broken, and with it rule 4's undeclared-binding clause"
            );
        }
        out
    })
}

/// One `fn` of an `impl Engine` block of the bridge.
struct EngineFn {
    name: String,
    /// The first parameter is a `self`: the fn is callable on an engine.
    takes_self: bool,
    /// The [`code_only`] text from this header to the next header of the block.
    body: String,
}

/// The bridge library's sources: `crates/dss-epri/src/lib.rs` and, to a
/// fixpoint, every module file a `mod <name>;` of a read source names
/// ([`mod_files`]). An `impl` block of `Engine` can live in no other file.
fn bridge_sources() -> Vec<String> {
    let mut out = vec!["crates/dss-epri/src/lib.rs".to_string()];
    let mut k = 0;
    while k < out.len() {
        let rel = out[k].clone();
        let exists = |p: &str| repo_root().join(p).is_file();
        for f in mod_files(&read_source(&rel), &rel, &exists).unwrap_or_else(|e| panic!("{e}")) {
            if !out.contains(&f) {
                out.push(f);
            }
        }
        k += 1;
    }
    out
}

/// The module files the `mod <name>;` declarations of one Rust source name:
/// `<dir>/<name>.rs` or `<dir>/<name>/mod.rs`, where `<dir>` is the source's
/// directory for a `lib.rs`, `main.rs` or `mod.rs` and `<dir>/<stem>` for any
/// other file. A declaration whose file `exists` finds under neither name, or
/// a module a `#[path]` attribute moves, is an error, never a skipped module.
fn mod_files(src: &str, rel: &str, exists: &dyn Fn(&str) -> bool) -> Result<Vec<String>, String> {
    let code = code_only(src, Lang::Rust);
    if code.contains("#[path") {
        return Err(format!(
            "{rel}: a `#[path]` attribute moves a module — the module scan cannot follow it"
        ));
    }
    let (dir, file) = rel.rsplit_once('/').unwrap_or(("", rel));
    let stem = file.trim_end_matches(".rs");
    let base = if matches!(stem, "lib" | "main" | "mod") {
        dir.to_string()
    } else {
        format!("{dir}/{stem}")
    };
    let mut out = Vec::new();
    for line in code.lines() {
        let mut item = line.trim_start();
        if let Some(rest) = item.strip_prefix("pub") {
            let rest = rest.trim_start();
            item = match rest.strip_prefix('(') {
                Some(scope) => scope
                    .split_once(')')
                    .map_or(rest, |(_, after)| after.trim_start()),
                None => rest,
            };
        }
        let Some(rest) = item.strip_prefix("mod ") else {
            continue;
        };
        let name = ident_at(rest.trim_start(), 0);
        if name.is_empty()
            || !rest.trim_start()[name.len()..]
                .trim_start()
                .starts_with(';')
        {
            continue;
        }
        let flat = format!("{base}/{name}.rs");
        let nested = format!("{base}/{name}/mod.rs");
        out.push(if exists(&flat) {
            flat
        } else if exists(&nested) {
            nested
        } else {
            return Err(format!(
                "{rel}: `mod {name};` names neither `{flat}` nor `{nested}`"
            ));
        });
    }
    Ok(out)
}

/// Every `fn` of every `impl` block of `Engine` in the bridge library
/// ([`bridge_sources`]), in source order, read once from the source
/// ([`engine_fns_in`]).
fn engine_fns() -> &'static [EngineFn] {
    static FNS: OnceLock<Vec<EngineFn>> = OnceLock::new();
    FNS.get_or_init(|| {
        let sources = bridge_sources();
        for want in [
            "crates/dss-epri/src/dss.rs",
            "crates/dss-epri/src/capture.rs",
        ] {
            assert!(
                sources.iter().any(|s| s == want),
                "the module scan of crates/dss-epri/src/lib.rs never reached `{want}` — the \
                 scan is broken, and with it the `impl Engine` scan"
            );
        }
        let mut out = Vec::new();
        for rel in &sources {
            out.extend(engine_fns_in(&read_source(rel), rel).unwrap_or_else(|e| panic!("{e}")));
        }
        out
    })
}

/// Whether an `impl` header (the text between `impl` and the block's `{`)
/// names `Engine` as its self type, through any path or reference:
/// `impl Engine`, `impl crate::dss::Engine`, `impl<'a> Default for &'a Engine`.
fn impl_self_is_engine(header: &str) -> bool {
    // The first `word` token outside every `<…>` of `s`.
    let top = |s: &str, word: &str| -> Option<usize> {
        let (mut depth, mut prev) = (0usize, ' ');
        for (i, c) in s.char_indices() {
            match c {
                '<' => depth += 1,
                '>' if prev != '-' => depth = depth.saturating_sub(1),
                _ => {}
            }
            if depth == 0
                && s[i..].starts_with(word)
                && starts_token(s, i)
                && !s[i + word.len()..].starts_with(is_ident)
            {
                return Some(i);
            }
            prev = c;
        }
        None
    };
    let mut h = header.trim_start();
    if h.starts_with('<') {
        let (mut depth, mut prev, mut close) = (0usize, ' ', None);
        for (i, c) in h.char_indices() {
            match c {
                '<' => depth += 1,
                '>' if prev != '-' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(i);
                        break;
                    }
                }
                _ => {}
            }
            prev = c;
        }
        let Some(close) = close else {
            return false;
        };
        h = &h[close + 1..];
    }
    let h = top(h, "where").map_or(h, |i| &h[..i]);
    let ty = top(h, "for").map_or(h, |i| &h[i + 3..]);
    let mut ty = ty.trim().trim_start_matches('&').trim_start();
    if let Some(life) = ty.strip_prefix('\'') {
        ty = life.trim_start_matches(is_ident).trim_start();
    }
    let ty = ty.strip_prefix("mut ").unwrap_or(ty).trim();
    ty == "Engine" || ty.ends_with("::Engine")
}

/// Every `fn` of every `impl` block of `Engine` ([`impl_self_is_engine`]) in
/// one Rust source, in source order: an inherent or a trait impl, its self type
/// spelled through any path. A header the scan cannot read is an error, never a
/// skipped method.
fn engine_fns_in(src: &str, rel: &str) -> Result<Vec<EngineFn>, String> {
    let code = code_only(src, Lang::Rust);
    let mut out = Vec::new();
    for (open, _) in code.match_indices("impl") {
        let lead = code[..open].rsplit('\n').next().unwrap_or("").trim();
        if !starts_token(&code, open)
            || code[open + 4..].starts_with(is_ident)
            || !matches!(lead, "" | "unsafe")
        {
            continue;
        }
        let brace = code[open..]
            .find('{')
            .ok_or_else(|| format!("{rel}: the `impl` scan cannot read an `impl` header"))?;
        if !impl_self_is_engine(&code[open + 4..open + brace]) {
            continue;
        }
        let mut depth = 0usize;
        let mut end = None;
        for (i, c) in code[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end.ok_or_else(|| format!("{rel}: an `impl Engine` block is never closed"))?;
        let block = &code[open..end];
        let mut heads: Vec<(usize, String, bool)> = Vec::new();
        for (at, _) in block.match_indices("fn ") {
            if !starts_token(block, at) {
                continue;
            }
            let name = ident_at(block, at + 3);
            let unread =
                || format!("{rel}: the `impl Engine` scan cannot read the header of `fn {name}`");
            let mut rest = block[at + 3 + name.len()..].trim_start();
            if rest.starts_with('<') {
                // The generic list, nested lists and `->` inside it included.
                let (mut depth, mut prev, mut close) = (0usize, ' ', None);
                for (i, c) in rest.char_indices() {
                    match c {
                        '<' => depth += 1,
                        '>' if prev != '-' => {
                            depth -= 1;
                            if depth == 0 {
                                close = Some(i);
                                break;
                            }
                        }
                        _ => {}
                    }
                    prev = c;
                }
                rest = rest[close.ok_or_else(unread)? + 1..].trim_start();
            }
            let params = rest.strip_prefix('(').ok_or_else(unread)?;
            let first = params.split([',', ')']).next().unwrap_or("");
            heads.push((at, name, first.contains("self")));
        }
        out.extend(
            heads
                .iter()
                .enumerate()
                .map(|(k, (at, name, takes_self))| EngineFn {
                    name: name.clone(),
                    takes_self: *takes_self,
                    body: block[*at..heads.get(k + 1).map_or(block.len(), |h| h.0)].to_string(),
                }),
        );
    }
    Ok(out)
}

/// The `Engine` methods that issue an executive command ([`command_methods_in`]
/// over [`engine_fns`]). Read from the bridge source, so a new command wrapper
/// joins [`check_r4133_command_sites`] without an edit here.
fn engine_command_methods() -> &'static [String] {
    static METHODS: OnceLock<Vec<String>> = OnceLock::new();
    METHODS.get_or_init(|| {
        let out = command_methods_in(engine_fns());
        for want in ["raw_command", "exec_wait", "relcalc", "solve", "clear"] {
            assert!(
                out.iter().any(|m| m == want),
                "crates/dss-epri/src: the command-method scan found no `{want}` — the scan \
                 is broken, and with it the r4133 command-site rail"
            );
        }
        out
    })
}

/// The methods of `fns` that issue an executive command: the ones that hand a
/// command line to the DLL (`dss_put_command`) and, to a fixpoint, every one
/// that calls one of them on itself (`self.m(`, `Self::m(`, `Engine::m(`).
fn command_methods_in(fns: &[EngineFn]) -> Vec<String> {
    let mut issuing: Vec<&str> = fns
        .iter()
        .filter(|f| f.body.contains("dss_put_command"))
        .map(|f| f.name.as_str())
        .collect();
    loop {
        let before = issuing.len();
        for f in fns {
            let calls = |m: &&str| {
                ["self.", "Self::", "Engine::"]
                    .iter()
                    .any(|p| f.body.contains(&format!("{p}{m}(")))
            };
            if !issuing.contains(&f.name.as_str()) && issuing.iter().any(calls) {
                issuing.push(f.name.as_str());
            }
        }
        if issuing.len() == before {
            break;
        }
    }
    fns.iter()
        .filter(|f| f.takes_self && issuing.contains(&f.name.as_str()))
        .map(|f| f.name.clone())
        .collect()
}

/// The body's receivers: the declared ones plus every name a line binds to one
/// ([`alias_of`]), to a fixpoint, so an alias of an alias is one too.
fn receivers_of(b: &CaptureBody, text: &str) -> Vec<String> {
    let mut out: Vec<String> = b.receivers.iter().map(|r| r.to_string()).collect();
    loop {
        let before = out.len();
        for line in text.lines() {
            if let Some(alias) = alias_of(code_of(line, b.lang), b.lang, &out)
                && !out.contains(&alias)
            {
                out.push(alias);
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

/// The name a line of code binds to one of `receivers`: `x = <recv>` or the
/// facade a selector hands out, `x = <recv>.<selector>` (Python);
/// `let [mut] x[: T] = [&[mut]]<recv>[.clone()];` (Rust).
fn alias_of(code: &str, lang: Lang, receivers: &[String]) -> Option<String> {
    let code = code.trim();
    let code = match lang {
        Lang::Python => code,
        Lang::Rust => code.strip_prefix("let ")?,
    };
    let (lhs, rhs) = code.split_once('=')?;
    let lhs = lhs.trim();
    let lhs = lhs.strip_prefix("mut ").unwrap_or(lhs);
    let name = lhs.split(':').next()?.trim();
    if name.is_empty() || !name.chars().all(is_ident) {
        return None;
    }
    let rhs = rhs.trim().trim_end_matches(';').trim();
    let rhs = rhs.trim_start_matches(['&', '*']).trim_start();
    let rhs = rhs.strip_prefix("mut ").unwrap_or(rhs);
    let rhs = rhs.strip_suffix(".clone()").unwrap_or(rhs);
    let source = match rhs.split_once('.') {
        Some((recv, member)) if SELECTORS.contains(&member) => recv,
        Some(_) => return None,
        None => rhs,
    };
    receivers
        .iter()
        .any(|r| r == source)
        .then(|| name.to_string())
}

/// Python and Rust keywords a parenthesis may follow without being called: the
/// parenthesis is a grouping.
const NOT_A_CALLEE: &[&str] = &[
    "and", "as", "assert", "await", "elif", "else", "for", "if", "in", "is", "lambda", "let",
    "loop", "match", "mut", "not", "or", "raise", "ref", "return", "while", "with", "yield",
];

/// What holds a position of a body ([`enclosing_call`]).
enum Enclosing {
    /// The argument list of a call: the start of its callee, or the `(` of the
    /// list when the callee is an expression (`(lambda e: f(e))(el)`,
    /// `fs[0](engine)`).
    Call(usize),
    /// The parameter list of a `def` / `fn` header.
    Definition,
    /// No call and no header.
    Nothing,
}

/// The innermost call or definition header whose parenthesis holds `at`.
/// Brackets, braces and parens no callee precedes (a tuple, a grouping, a
/// keyword's operand) are walked through.
fn enclosing_call(ch: &[char], at: usize) -> Enclosing {
    let mut depth = 0usize;
    let mut k = at;
    while k > 0 {
        k -= 1;
        match ch[k] {
            ')' | ']' | '}' => depth += 1,
            '(' | '[' | '{' if depth > 0 => depth -= 1,
            '(' => {
                let z = callee_end(ch, k);
                let mut a = z;
                while a > 0 && is_ident(ch[a - 1]) {
                    a -= 1;
                }
                let callee: String = ch[a..z].iter().collect();
                if a < z && !NOT_A_CALLEE.contains(&callee.as_str()) {
                    let head: String = ch[..a].iter().collect();
                    let keyword = head.trim_end().rsplit(|c: char| !is_ident(c)).next();
                    return if matches!(keyword, Some("def" | "fn")) {
                        Enclosing::Definition
                    } else {
                        Enclosing::Call(a)
                    };
                }
                if a == z && z > 0 && matches!(ch[z - 1], ')' | ']') {
                    return Enclosing::Call(k);
                }
            }
            _ => {}
        }
    }
    Enclosing::Nothing
}

/// The token at `i` stands where a parameter's name does: right after the `(`
/// or a `,` of the list.
fn param_name_position(ch: &[char], i: usize) -> bool {
    ch[..i]
        .iter()
        .rev()
        .find(|c| !c.is_whitespace())
        .is_some_and(|c| matches!(c, '(' | ','))
}

/// Where the callee of the parenthesis at `open` ends: before the whitespace,
/// the turbofish (`read_seq::<f64>(…)`) and the macro `!` that may stand
/// between the two.
fn callee_end(ch: &[char], open: usize) -> usize {
    let mut z = open;
    while z > 0 && ch[z - 1].is_whitespace() {
        z -= 1;
    }
    if z > 0 && ch[z - 1] == '>' {
        let mut depth = 0usize;
        let mut q = z;
        while q > 0 {
            q -= 1;
            match ch[q] {
                '>' => depth += 1,
                '<' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        if depth == 0 && q >= 2 && ch[q - 2..q] == [':', ':'] {
            z = q - 2;
        }
    }
    if z > 0 && ch[z - 1] == '!' {
        z -= 1;
    }
    z
}

/// Every place a body hands one of `receivers` on, as `(line, column, read)`:
/// to a call — `f(el)`, `read_seq(&engine)`, `x.helper(self)` — the read named
/// after its callee, or `<call>` when the callee is an expression; anywhere
/// else — a tuple, a `for` or comprehension iterable, a walrus, a default
/// value, a literal, a binding the gate cannot follow — the read named after
/// the receiver. A receiver used as one (`el.X`, `engine.m()`), a side of an
/// alias line ([`alias_of`]) and a parameter name of a definition hand nothing
/// on. A receiver handed on lets the code it reaches read the model, so the
/// hand-over is a read of its own. `code` is the body's lines with their
/// comments cut off; the scan runs over all of them, so a call wrapped over
/// several lines is found on its callee's line. String literals are not
/// blanked: a receiver named inside one reads as handed on, which errs on the
/// loud side.
fn handed_calls(code: &[&str], lang: Lang, receivers: &[String]) -> Vec<(usize, usize, String)> {
    let mut ch: Vec<char> = Vec::new();
    let mut starts: Vec<usize> = Vec::new();
    for line in code {
        starts.push(ch.len());
        ch.extend(line.chars());
        ch.push('\n');
    }
    let aliases: Vec<bool> = code
        .iter()
        .map(|l| alias_of(l, lang, receivers).is_some())
        .collect();
    let line_of = |at: usize| starts.partition_point(|&s| s <= at) - 1;
    let mut out: Vec<(usize, usize, String)> = Vec::new();
    let mut i = 0;
    while i < ch.len() {
        if !is_ident(ch[i]) || (i > 0 && (is_ident(ch[i - 1]) || ch[i - 1] == '.')) {
            i += 1;
            continue;
        }
        let mut z = i;
        while z < ch.len() && is_ident(ch[z]) {
            z += 1;
        }
        let token: String = ch[i..z].iter().collect();
        let next = ch[z..].iter().copied().find(|c| !c.is_whitespace());
        if receivers.contains(&token) && !matches!(next, Some('.' | '(')) {
            let read = match enclosing_call(&ch, i) {
                Enclosing::Call(at) => {
                    let callee: String = ch[at..].iter().take_while(|c| is_ident(**c)).collect();
                    let callee = if callee.is_empty() {
                        "<call>".to_string()
                    } else {
                        callee
                    };
                    Some((at, callee))
                }
                Enclosing::Definition if param_name_position(&ch, i) => None,
                _ if aliases[line_of(i)] => None,
                _ => Some((i, token.clone())),
            };
            if let Some((at, read)) = read {
                let line = line_of(at);
                let col = at - starts[line];
                if !out.iter().any(|(l, c, _)| *l == line && *c == col) {
                    out.push((line, col, read));
                }
            }
        }
        i = z;
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

/// The dotted reads of one line of code, as `(column, member)`.
///
/// Python: every attribute whose name is capitalized — the dss-python API
/// convention for a property/method on `ActiveCircuit`/`ActiveCktElement` —
/// plus a `gc.capture_*` delegation. Deliberately receiver-agnostic so a read
/// introduced through a *new* receiver cannot slip past.
///
/// Rust: every member of a receiver ([`receivers_of`]), every call of an
/// [`engine_methods`] method on ANY receiver — an undeclared binding, a
/// closure parameter, a call chain, an index, a `?` — and every
/// `Engine::<method>` / `Self::<method>` path, called or handed on as a value
/// ([`engine_paths`]), so a bridge read cannot slip past through any of them.
/// Exempt members never count.
fn dotted_reads(code: &str, b: &CaptureBody, receivers: &[String]) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = dotted(code)
        .into_iter()
        .filter(|d| {
            let exempt = b.exempt.contains(&d.member.as_str());
            match b.lang {
                Lang::Python => {
                    (d.member.starts_with(|c: char| c.is_ascii_uppercase()) && !exempt)
                        || (d.recv == "gc" && d.member.starts_with("capture_"))
                }
                Lang::Rust => {
                    !exempt
                        && (receivers.contains(&d.recv)
                            || (d.called && engine_methods().contains(&d.member)))
                }
            }
        })
        .map(|d| (d.col, d.member))
        .collect();
    if b.lang == Lang::Rust {
        out.extend(
            engine_paths(code)
                .into_iter()
                .filter(|(_, m)| !b.exempt.contains(&m.as_str())),
        );
    }
    out
}

/// Every `Engine::<m>` / `Self::<m>` path on a line of Rust whose `<m>` is an
/// [`engine_methods`] method, as `(column, m)`.
fn engine_paths(line: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for prefix in ["Engine::", "Self::"] {
        for (at, _) in line.match_indices(prefix) {
            if !starts_token(line, at) {
                continue;
            }
            let member = ident_at(line, at + prefix.len());
            if engine_methods().contains(&member) {
                out.push((line[..at + prefix.len()].chars().count(), member));
            }
        }
    }
    out
}

/// The reads of a body, line by line, in column order within a line (rule 4):
/// each line's [`dotted_reads`] plus the [`handed_calls`] hand-overs that are
/// not one of them already and whose callee is not exempt.
fn reads_by_line(b: &CaptureBody, text: &str) -> Vec<Vec<String>> {
    let receivers = receivers_of(b, text);
    let code: Vec<&str> = text.lines().map(|l| code_of(l, b.lang)).collect();
    let mut lines: Vec<Vec<(usize, String)>> = code
        .iter()
        .map(|c| dotted_reads(c, b, &receivers))
        .collect();
    for (line, col, callee) in handed_calls(&code, b.lang, &receivers) {
        if b.exempt.contains(&callee.as_str()) || lines[line].iter().any(|(c, _)| *c == col) {
            continue;
        }
        lines[line].push((col, callee));
    }
    lines
        .into_iter()
        .map(|mut v| {
            v.sort_by_key(|(c, _)| *c);
            v.into_iter().map(|(_, m)| m).collect()
        })
        .collect()
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
    /// 1-based line of the read within the body text.
    line: usize,
}

/// Every capture-order violation in `text`, read as the body of `b`.
///
/// Violation strings are prefixed by kind (`order:`, `unmarked:`, `group:`,
/// `unknown:`, `sequence:`, `helper:`, `marker:`, `count:`, `name:`) so the
/// non-vacuity tests can assert that the *intended* rule fired, not merely that
/// something failed.
fn check(b: &CaptureBody, text: &str) -> Vec<String> {
    let mut bad = Vec::new();
    let mut pending: Vec<(String, char)> = Vec::new();
    let mut reads: Vec<Consumed> = Vec::new();

    for ((no, line), found) in text.lines().enumerate().zip(reads_by_line(b, text)) {
        let mine = match markers_in(line) {
            Ok(m) => m,
            Err(e) => {
                bad.push(format!("marker: {}:{} {e}", b.file, no + 1));
                Vec::new()
            }
        };
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
            reads.push(Consumed {
                member,
                markers,
                line: no + 1,
            });
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
                 `dss_epri::modes` and is not a declared selector — add the read's row to \
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
             (position {bi}) — a group-B read stamps the terminal-current cache as current \
             without filling it, so the cache-aware read that follows can answer stale",
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

    // 5. every other marker names the read it sits on: rule 1 takes the group
    //    from the NAME, so a mislabelled read would borrow another's group.
    for r in reads
        .iter()
        .filter(|r| b.marked && helper_key(&r.member).is_none())
    {
        for (name, _) in &r.markers {
            if !marker_names_read(b, &r.member, name) {
                bad.push(format!(
                    "name: {}:{} marks the read `{}` as `{name}` — a marker must name the \
                     quantity its own line reads (Python: the API member; Rust: \
                     `{}_<quantity>` in snake case or a RUST_READ_ALIASES entry)",
                    b.file,
                    r.line,
                    r.member,
                    snake(b.family)
                ));
            }
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

/// G1.4b: the three `DistFromMeter` views are **group C** in the ONE mode table.
///
/// `crates/dss-core/tests/corpus_gate.rs` pins WHERE each of them is read —
/// `Bus.Distance` between the bus's other scalar and the value arrays of the
/// per-bus walk, the two circuit arrays inside the same order-free checkpoint
/// slot as `AllBusVmagPu` — and that placement is only legitimate while all
/// three are order-free. The claim itself has exactly one home
/// (`dss_epri::modes`, whose `ModeEffect` rows `BUSF` 5, `CircuitV` 12 and
/// `CircuitV` 13 each return the stored field and move no cursor), so it is
/// asserted here rather than restated there.
#[test]
fn the_distance_surface_is_order_free_in_the_mode_table() {
    for (family, name) in [
        ("Bus", "Distance"),
        ("Circuit", "AllBusDistances"),
        ("Circuit", "AllNodeDistances"),
    ] {
        assert_eq!(
            capture_group_of(family, name),
            Some('C'),
            "`{family}.{name}` is captured inside an order-free block (`corpus_gate.rs::BUS_READ_ORDER` / the checkpoint slot list), but the mode table gives it another capture group — the table wins: re-class the read or move it out of the group-C block"
        );
    }
}

/// G1.4d: the two at-bus lists are **group C** in the ONE mode table — "impure
/// but order-free".
///
/// `crates/dss-core/tests/corpus_gate.rs::AT_BUS_READ_ORDER` pins that both
/// transports read them LAST in the per-bus walk, and that placement is chosen
/// because on the r4133 channel they are the block's only
/// [`dss_epri::modes::ModeEffect::Impure`] reads: each one walks a class with
/// `First`/`Next`, which moves the active circuit element. What that impurity
/// does NOT touch is any terminal-current cache, which is exactly what keeps
/// the whole bus block order-free — so `Impure` must map to `'C'`
/// here ([`dss_epri::modes::ModeEffect::capture_group`]). A future re-class of
/// either row to `'A'`/`'B'` would make the bus block's slot matter and fails
/// here, where the claim has its single home, instead of silently in the gate.
#[test]
fn the_at_bus_surface_is_order_free_in_the_mode_table() {
    for name in ["AllPCEatBus", "AllPDEatBus"] {
        assert_eq!(
            capture_group_of("Bus", name),
            Some('C'),
            "`Bus.{name}` is captured inside an order-free block (the tail of `corpus_gate.rs::AT_BUS_READ_ORDER`'s per-bus walk), but the mode table gives it another capture group — the table wins: re-class the read or move it out of the group-C block"
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
/// reads a field and never touches the terminal-current cache or computes
/// currents), so moving one into the middle of the A/B block cannot violate the
/// D3 order. Moving the G1.3d(i) `NumPhases` read (`CktElementI(2)`) to sit
/// between the group-A `Losses` and the group-A/B `capture_element` delegation
/// raises only the declaration-bookkeeping violation — proof the
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
/// a capture body without declaring it, in every shape the rule names: a
/// member, a free function handed the receiver (both transports; also with a
/// space, a turbofish or a macro `!` before its parenthesis), a bridge read
/// through a Rust binding the body never declared, through a receiver
/// expression, behind a turbofish or as an `Engine::`/`Self::` path, a function
/// handed a `let` alias of the receiver, and the receiver handed to an
/// expression callee or to a tuple, an iterable, a walrus, a default value or a
/// literal.
#[test]
fn the_gate_fires_on_an_unmarked_read() {
    let (b, text) = capi();
    let bad = check(b, &text.replace("  # capture-order: Residuals (B)", ""));
    assert_fires(&bad, "unmarked");

    // A new capi read, silently added. `Voltages` (`CktElementV(4)`) is a read
    // this body genuinely does not perform — G1.3c turned `TotalPowers`, the
    // old stand-in here, into a real declared read.
    let bad = check(
        b,
        &insert_after(&text, "el.Enabled", "        probe = el.Voltages"),
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

    // A free function handed the receiver: no `receiver.member` on the line at
    // all, on either transport. The Python receiver is the facade `el` the
    // body binds from `ckt.ActiveCktElement`.
    let bad = check(
        b,
        &insert_after(&text, "el.Enabled", "        probe = _read_seq(el)"),
    );
    assert_unmarked(&bad, "_read_seq");
    let bad = check(
        r,
        &insert_after(
            &rt,
            "capture-order: Losses (A)",
            "            let probe = read_seq(self);",
        ),
    );
    assert_unmarked(&bad, "read_seq");

    // A bridge read through a binding the body never declared as a receiver,
    // and a function handed a `let` alias of the declared one.
    let c = body("capture.capture_all_elements");
    let ct = body_text(c);
    assert!(check(c, &ct).is_empty(), "the real body must be clean");
    let bad = check(
        c,
        &insert_after(
            &ct,
            "capture-order: SetActiveElement (C)",
            "        let probe = bridge.element_yprim();",
        ),
    );
    assert_unmarked(&bad, "element_yprim");

    // The same read on a receiver EXPRESSION, and as a path — called, or the
    // method handed on as a value.
    for smuggled in [
        "        let probe = holder.borrow().element_yprim();",
        "        let probe = holder.lock().unwrap().element_yprim();",
        "        let probe = maybe?.element_yprim();",
        "        let probe = engines[0].element_yprim();",
        "        let probe = (engine).element_yprim();",
        "        let probe = Engine::element_yprim(bridge);",
        "        let read = Engine::element_yprim;",
    ] {
        let bad = check(
            c,
            &insert_after(&ct, "capture-order: SetActiveElement (C)", smuggled),
        );
        assert_unmarked(&bad, "element_yprim");
    }
    let chained = insert_after(
        &ct,
        "capture-order: SetActiveElement (C)",
        "        let probe = holder\n            .borrow()\n            .element_yprim();",
    );
    assert_unmarked(&check(c, &chained), "element_yprim");

    // A free function handed the receiver, its callee apart from the
    // parenthesis: a space (no formatter runs over the Python transport) or a
    // turbofish.
    let bad = check(
        b,
        &insert_after(&text, "el.Enabled", "        probe = _read_seq (el)"),
    );
    assert_unmarked(&bad, "_read_seq");
    let bad = check(
        c,
        &insert_after(
            &ct,
            "capture-order: SetActiveElement (C)",
            "        let probe = read_seq::<f64>(engine);",
        ),
    );
    assert_unmarked(&bad, "read_seq");

    let aliased = insert_after(
        &insert_after(
            &ct,
            "capture-order: SetActiveElement (C)",
            "        let probe = read_seq(e);",
        ),
        "capture-order: SetActiveElement (C)",
        "        let e = engine;",
    );
    assert_unmarked(&check(c, &aliased), "read_seq");

    // A receiver handed to a callee that is an expression, or to a place it may
    // not stand bare: the hand-over is the read, named `<call>` or after the
    // receiver.
    for (smuggled, read) in [
        ("        probe = (lambda e: _read_seq(e))(el)", "<call>"),
        (
            "        a, z = el, None\n        probe = _read_seq(a)",
            "el",
        ),
        ("        probe = [_read_seq(e) for e in (el,)]", "el"),
        (
            "        for e in (el,):\n            probe = _read_seq(e)",
            "el",
        ),
        ("        probe = _read_seq(e) if (e := el) else None", "el"),
        ("        probe = (lambda e=el: _read_seq(e))()", "el"),
        (
            "        def inner(e=el):\n            return _read_seq(e)",
            "el",
        ),
    ] {
        let bad = check(b, &insert_after(&text, "el.Enabled", smuggled));
        assert_unmarked(&bad, read);
    }
    for (smuggled, read) in [
        ("        let probe = (|e| read_seq(e))(engine);", "<call>"),
        (
            "        let (a, z) = (engine, 0);\n        let probe = read_seq(a);",
            "engine",
        ),
        ("        let probe = [engine].map(read_seq);", "engine"),
        // A macro handed the receiver, and a bridge read behind a turbofish.
        ("        let probe = dbg!(engine);", "dbg"),
        (
            "        let probe = bridge.element_yprim::<f64>();",
            "element_yprim",
        ),
    ] {
        let bad = check(
            c,
            &insert_after(&ct, "capture-order: SetActiveElement (C)", smuggled),
        );
        assert_unmarked(&bad, read);
    }

    // A bridge read handed on as a `Self::` value inside a `dss.rs` helper.
    let bad = check(
        r,
        &insert_after(
            &rt,
            "capture-order: Losses (A)",
            "            let read = Self::ckt_element_seq_currents;",
        ),
    );
    assert_unmarked(&bad, "ckt_element_seq_currents");
}

/// `bad` holds the `unmarked:` violation of the read `member`.
fn assert_unmarked(bad: &[String], member: &str) {
    let want = format!("read `{member}` carries no");
    assert!(
        bad.iter()
            .any(|v| v.starts_with("unmarked:") && v.contains(&want)),
        "expected the read `{member}` to be flagged as unmarked, got {bad:?}"
    );
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
            "        # capture-order: Voltages (B)",
        ),
    );
    assert_fires(&bad, "marker");

    // The other half of the same rule: a pending marker that the next read
    // does not consume because that read already declares itself.
    let bad = check(
        b,
        &insert_after(&text, "el.Enabled", "        # capture-order: Voltages (B)"),
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
/// `PhaseLosses` (`CktElementV(6)`) starts, on both oracles, by reading through
/// the terminal-current cache, which is recomputed only when its solution count
/// is stale — so it is group **A** and must precede the group-B `Currents`.
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

/// The member GOLDEN_REBASE G1.3c adds to the A-before-B rule: `TotalPowers`
/// (`CktElementV(20)` on r4133, `Alt_CE_Get_TotalPowers` on capi) is the
/// per-terminal sum of the phase powers, and on an enabled element the oracle
/// starts that sum by reading through the terminal-current cache, which is
/// recomputed only when its solution count is stale — the same cache-aware path
/// `Powers` takes. So it is group **A** and must be issued
/// before the group-B `Currents`, on both transports.
///
/// Stated in both directions, because the negative half alone would pass on a
/// body that never reads `TotalPowers` at all: first the *positive* claim (the
/// mode table says `A`, and both live bodies really do declare it ahead of
/// `Currents`), then the mutation that moves the read past the currents read
/// and must fire `order:`.
#[test]
fn total_powers_is_a_group_a_read_issued_before_the_currents_read() {
    assert_eq!(
        capture_group_of("CktElement", "TotalPowers"),
        Some('A'),
        "`TotalPowers` sums the phase powers, which take the cache-aware \
         terminal-current path — the mode table must classify it as group A"
    );
    for key in [
        "oracle_server.capture_all_elements",
        "capture.capture_all_elements",
    ] {
        let b = body(key);
        assert!(check(b, &body_text(b)).is_empty(), "{key} must be clean");
        let pos = |n: &str| {
            b.declared
                .iter()
                .position(|d| *d == n)
                .unwrap_or_else(|| panic!("{key} does not declare `{n}`"))
        };
        assert!(
            pos("TotalPowers") < pos("Currents"),
            "{key} declares TotalPowers at {} and Currents at {} — the group-A read must come \
             first",
            pos("TotalPowers"),
            pos("Currents")
        );
    }

    let (b, text) = capi();
    let moved = move_line_after(&text, "el.TotalPowers", "gc.capture_element");
    assert_ne!(moved, text, "the mutation must actually move a line");
    assert_fires(&check(b, &moved), "order");

    let r = body("capture.capture_all_elements");
    let rt = body_text(r);
    let moved = move_line_after(&rt, "engine.element_total_powers", "engine.element_pcl");
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

/// `b` with its `declared` sequence replaced — the edit a mislabelled read
/// needs to get past rule 2.
fn with_declared(b: &CaptureBody, declared: Vec<&'static str>) -> CaptureBody {
    CaptureBody {
        declared: declared.leak(),
        ..*b
    }
}

/// Rule 6: a group-B read labelled with a group-A name and admitted ahead of
/// `Losses`, with `declared` edited to match. Every other rule is satisfied by
/// the edit — the label's group is the table's, the sequence is the declared
/// one, A still precedes B — so the marker-name check is the one that must
/// fire, on both transports.
#[test]
fn the_gate_fires_when_a_marker_names_another_quantity_than_its_read() {
    let (b, text) = capi();
    let mislabelled = insert_after(
        &text,
        "el.TotalPowers",
        "        probe = _read(lambda: el.SeqCurrents)  # capture-order: TotalPowers (A)",
    );
    let mut declared = b.declared.to_vec();
    let at = declared.iter().position(|n| *n == "Losses").unwrap();
    declared.insert(at, "TotalPowers");
    let bad = check(&with_declared(b, declared), &mislabelled);
    assert_eq!(kinds(&bad), ["name"], "{bad:?}");
    assert!(bad[0].contains("`SeqCurrents` as `TotalPowers`"), "{bad:?}");

    let r = body("element_pcl");
    let rt = body_text(r);
    let mislabelled = insert_after(
        &rt,
        "capture-order: Losses (A)",
        "            let probe = self.ckt_element_seq_currents()?; // capture-order: Losses (A)",
    );
    let bad = check(
        &with_declared(r, vec!["Losses", "Losses", "Powers", "Currents"]),
        &mislabelled,
    );
    assert_eq!(kinds(&bad), ["name"], "{bad:?}");
    assert!(
        bad[0].contains("`ckt_element_seq_currents` as `Losses`"),
        "{bad:?}"
    );
}

/// Why the bridge accessor `accessor` does not read `quantity`, if it does not:
/// an element quantity's accessor reads its own `modes::CKT_ELEMENT_<QUANTITY>`
/// row, and a selector's accessor is the selector's own name on a circuit
/// interface.
fn alias_pairing_error(accessor: &str, quantity: &str) -> Option<String> {
    let Some(f) = engine_fns().iter().find(|f| f.name == accessor) else {
        return Some(format!(
            "`{accessor}` is not a method of the bridge's `Engine`"
        ));
    };
    let reads_row = |row: &str| {
        f.body
            .match_indices(row)
            .any(|(at, _)| !f.body[at + row.len()..].starts_with(is_ident))
    };
    let pairs = if SELECTORS.contains(&quantity) {
        accessor == snake(quantity) && f.body.contains("self.dll.circuit_")
    } else {
        reads_row(&format!(
            "modes::CKT_ELEMENT_{}",
            snake(quantity).to_uppercase()
        ))
    };
    (!pairs).then(|| {
        format!(
            "`{accessor}` does not read `{quantity}`: its body names neither that quantity's \
             `modes::CKT_ELEMENT_*` row nor, for a selector, its own name on a circuit interface"
        )
    })
}

/// Each [`RUST_READ_ALIASES`] entry pairs a real bridge method with a quantity
/// the gate knows (a mode-table row or a declared selector) that the method
/// really reads ([`alias_pairing_error`]), and each is in use, so the table can
/// neither excuse a read that does not exist, nor lend a read another
/// quantity's group, nor go stale. A bogus pairing is refused.
#[test]
fn every_rust_read_alias_is_a_bridge_read_of_a_known_quantity() {
    let used: Vec<String> = BODIES
        .iter()
        .filter(|b| b.lang == Lang::Rust)
        .flat_map(|b| reads_by_line(b, &body_text(b)).into_iter().flatten())
        .collect();
    for (accessor, quantity) in RUST_READ_ALIASES {
        assert!(
            engine_methods().iter().any(|m| m == accessor),
            "`{accessor}` is not a method of the bridge's `Engine`"
        );
        assert!(
            capture_group_of("CktElement", quantity).is_some() || SELECTORS.contains(quantity),
            "`{quantity}` has no mode-table row and is not a declared selector"
        );
        if let Some(e) = alias_pairing_error(accessor, quantity) {
            panic!("{e}");
        }
        assert!(
            used.iter().any(|m| m == accessor),
            "`{accessor}` is read by no Rust capture body — drop it from RUST_READ_ALIASES"
        );
    }
    for (accessor, quantity) in [
        ("element_currents", "Losses"),
        ("element_losses", "Powers"),
        ("all_element_names", "SetActiveElement"),
    ] {
        assert!(
            alias_pairing_error(accessor, quantity).is_some(),
            "the bogus pairing (`{accessor}`, `{quantity}`) was accepted"
        );
    }
}

/// The `impl Engine` scan behind rule 4 and the command rail reads every block
/// of `Engine` — a second inherent block, one spelled through a path, a trait
/// impl, one with a `where` clause, an `unsafe impl` — a nested generic list
/// and a `Self::` or `Engine::` wrapper, skips the blocks of other types, and
/// refuses a header it cannot read instead of dropping the method.
#[test]
fn the_engine_scan_reads_every_impl_block_and_refuses_an_unread_header() {
    let src = "\
pub struct Engine;
impl Engine {
    fn raw_command(&self, c: &str) -> String {
        dss_put_command(c)
    }
    fn post_all<I: IntoIterator<Item = String>, F: Fn(u8) -> u8>(&self, cmds: I) {
        for c in cmds {
            self.raw_command(&c);
        }
    }
    fn element_yprim(&self) -> Vec<f64> {
        Vec::new()
    }
}
impl Engine {
    fn rebuild(&self) -> String {
        Self::raw_command(self, \"CalcVoltageBases\")
    }
    fn new() -> Self {
        Engine
    }
}
impl crate::dss::Engine {
    fn via_path(&self) -> String {
        Engine::raw_command(self, \"RelCalc\")
    }
}
impl<'a> Refresh for &'a Engine {
    fn refresh(&self) -> String {
        self.rebuild()
    }
}
impl<T> Bounded<T> for Engine
where
    T: Copy,
{
    fn bounded(&self) -> String {
        self.rebuild()
    }
}
unsafe impl Zeroed for Engine {
    fn zeroed(&self) -> String {
        self.rebuild()
    }
}
impl Engine2 {
    fn not_the_bridge(&self) {}
}
impl From<Engine> for Wrapper {
    fn from(e: Engine) -> Self {
        Wrapper(e)
    }
}
";
    let fns = engine_fns_in(src, "synthetic").unwrap_or_else(|e| panic!("{e}"));
    let names: Vec<&str> = fns.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "raw_command",
            "post_all",
            "element_yprim",
            "rebuild",
            "new",
            "via_path",
            "refresh",
            "bounded",
            "zeroed"
        ]
    );
    assert_eq!(
        command_methods_in(&fns),
        [
            "raw_command",
            "post_all",
            "rebuild",
            "via_path",
            "refresh",
            "bounded",
            "zeroed"
        ]
    );

    let unread = src.replace("fn new() -> Self", "fn new -> Self");
    let err = engine_fns_in(&unread, "synthetic")
        .err()
        .expect("an unread header");
    assert!(err.contains("cannot read the header of `fn new`"), "{err}");
}

/// The module scan behind [`bridge_sources`] follows every `mod <name>;` of a
/// source to its file, flat or nested, from a crate root and from a module
/// file, and refuses a module it cannot find or a `#[path]` it cannot follow;
/// a commented-out and an inline module name no file.
#[test]
fn the_module_scan_follows_every_mod_declaration() {
    let lib = "\
//! mod commented;
pub mod flat;
#[cfg(windows)]
pub(crate) mod nested;
mod inline {
    fn f() {}
}
";
    let on_disk = ["crates/x/src/flat.rs", "crates/x/src/nested/mod.rs"];
    let exists = |p: &str| on_disk.contains(&p);
    let files = mod_files(lib, "crates/x/src/lib.rs", &exists).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(files, on_disk);
    let child = |p: &str| p == "crates/x/src/flat/child.rs";
    let files =
        mod_files("mod child;\n", "crates/x/src/flat.rs", &child).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(files, ["crates/x/src/flat/child.rs"]);

    let err = mod_files("mod gone;\n", "crates/x/src/lib.rs", &|_| false)
        .expect_err("a module without a file");
    assert!(err.contains("`mod gone;` names neither"), "{err}");
    let err = mod_files(
        "#[path = \"elsewhere.rs\"]\nmod moved;\n",
        "crates/x/src/lib.rs",
        &|_| true,
    )
    .expect_err("a moved module");
    assert!(err.contains("`#[path]`"), "{err}");

    let real = bridge_sources();
    for want in [
        "crates/dss-epri/src/lib.rs",
        "crates/dss-epri/src/dss.rs",
        "crates/dss-epri/src/capture.rs",
        "crates/dss-epri/src/guard.rs",
    ] {
        assert!(
            real.iter().any(|s| s == want),
            "{want} is not among {real:?}"
        );
    }
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
    /// G1.8's flat incidence capture — after `topology` and every other
    /// capture, the reliability one included, i.e. strictly last in the step.
    /// It is the only capture that writes solution state. The once-per-case
    /// `RelCalc`, which writes state too, is no capture: `run_case` issues it
    /// after the last solve and ahead of every capture of that step
    /// ([`R4133_COMMAND_SITES`] pins that slot on r4133).
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
/// the memoized branch list and rewrites the checked and isolated flags of
/// every element and bus on the way; and `TopologyI(1)`/`(2)` +
/// `TopologyV(1)`/`(2)` walk `PDElements`/`PCElements` `.First`/`.Next` to
/// exhaustion, leaving those cursors at the end.
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
                 (byte {other}). The first `Topology` read builds the memoized branch list \
                 and rewrites the checked and isolated flags of every element and bus, so it \
                 must follow every other read of the step (only G1.8's incidence pair may come \
                 after it)."
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
/// **not** move the active circuit element, on either channel. The other
/// twelve are the B16 parity gap (`ActiveLevel`, `BranchName`, `ActiveBranch`)
/// and the nine cursor rows.
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

/// `NumIsolatedBranches` -> `num_isolated_branches`, `OCPDevIndex` ->
/// `ocp_dev_index`: the fastdss member name as the `dss-epri` bridge spells its
/// typed accessor (the `Engine::topology_*` and `Engine::ckt_element_*` fns of
/// `crates/dss-epri/src/dss.rs`, one per DDLL mode). An acronym stays one word.
fn snake(camel: &str) -> String {
    let ch: Vec<char> = camel.chars().collect();
    let mut out = String::new();
    for (i, &c) in ch.iter().enumerate() {
        let word_start = c.is_uppercase()
            && i > 0
            && (!ch[i - 1].is_uppercase() || ch.get(i + 1).is_some_and(|n| n.is_lowercase()));
        if word_start {
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
         The other twelve `ITopology` members move the active circuit element on both \
         channels and would poison the per-element capture of the same step. Three of \
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
/// topology_active_level — they move the active circuit element.
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
/// The order is a contract, not a style: r4133's `CalcLaplacian` multiplies the
/// transposed incidence matrix by the matrix without checking that it exists,
/// so issuing it before any `CalcIncMatrix` dereferences a null pointer inside
/// the DLL and kills the worker. capi refuses the same command with error 8877
/// and so does the port — which is why the swap is pinned from the sources here
/// rather than driven live (`inc_matrix_pins.rs::calclaplacian_without_calcincmatrix_raises_8877`).
const INCIDENCE_PAIR: [&str; 2] = ["CalcIncMatrix", "CalcLaplacian"];

/// The four `ISolution` members the capture reads afterwards, in this order
/// (`origin/fastdss:dss/ISolution.py:608-631`, `:651-675`, `:642-649`,
/// `:633-640`).
const INCIDENCE_READS: [&str; 4] = ["IncMatrix", "Laplacian", "IncMatrixRows", "IncMatrixCols"];

/// Executive commands no capture may ever issue. `CalcIncMatrix_O` builds the
/// topology tree when none is memoized yet, which memoizes the very branch list
/// G1.7's two decline censuses are defined on. Its coverage stays byte-golden
/// (`tests/golden/inc_matrix/` `org_*`), per the §G3.2c re-scope.
const FORBIDDEN_COMMANDS: [&str; 1] = ["CalcIncMatrix_O"];

/// `Solution.BusLevels` — `SolutionV(2)` on r4133 sizes its reply one shorter
/// than the bus-level list and then writes every level into it, one element
/// past the end. The r4133 bridge refuses the mode before
/// the FFI (`crates/dss-epri/src/modes.rs::DO_NOT_CALL`), so a surface only one
/// channel can answer is not a gate: neither transport may read it.
const FORBIDDEN_READS: [&str; 2] = ["BusLevels", "bus_levels"];

/// Every call of a command-issuing `Engine` method ([`engine_command_methods`])
/// in `crates/dss-epri/src/capture.rs`, as `(fn, method)`, one entry per call.
///
/// `run_case` drives the run (`clear`, `compile`, `post`, `solve`) and the one
/// sanctioned extra command, the reliability surface's once-per-case `RelCalc`:
/// on the last step of a reliability case, after the solve and before the G1.9
/// aggregates, so every capture of that checkpoint reads the post-`RelCalc`
/// state. The probe and property captures issue `?` property queries through
/// `raw_command` (each checked to be one), and the G1.8 capture issues its
/// [`INCIDENCE_PAIR`] through `exec_wait`. A command anywhere else would drive
/// the engine mid-step ([`check_r4133_command_sites`]).
const R4133_COMMAND_SITES: [(&str, &str); 10] = [
    ("run_case", "clear"),
    ("run_case", "compile"),
    ("run_case", "post"),
    ("run_case", "solve"),
    ("run_case", "relcalc"),
    ("capture_probes", "raw_command"),
    ("capture_all_properties", "raw_command"),
    ("capture_all_properties", "raw_command"),
    ("capture_inc_matrix", "exec_wait"),
    ("capture_inc_matrix", "exec_wait"),
];

/// The incidence capture must come after EVERY other capture of the step, the
/// topology and the reliability ones included — of the captures it is the only
/// one that writes solution state.
///
/// `CalcIncMatrix` rebuilds the incidence matrix and its row list and clears
/// the ordered flag, and while adding the series reactors it iterates the
/// reactor class, which moves the active class and the active circuit element.
/// It must also follow the topology read, because that read is the one that
/// memoizes the branch list. The reliability capture is no exception: it reads
/// the meter and bus state the once-per-case `RelCalc` left (issued by
/// `run_case` ahead of every capture of the step, on r4133 pinned by
/// [`R4133_COMMAND_SITES`]), so the pair must not precede it.
fn check_inc_matrix_last(src: &str, a: &Anchors, rel: &str) -> Result<(), String> {
    let run = offset_after(src, a.run, 0, rel)?;
    let inc = offset_after(src, a.inc_matrix, run, rel)?;
    for (what, needle) in [
        ("the G1.7 topology capture", a.topology),
        ("the WP8.5b property sweep", a.properties),
        ("the G1.6(i) reliability capture", a.reliability),
        ("the per-element capture", a.elements),
        ("the discrete-state capture", a.discrete),
        ("the G1.9 aggregates", a.aggregates),
    ] {
        let other = offset_after(src, needle, run, rel)?;
        if inc <= other {
            return Err(format!(
                "{rel}: the G1.8 incidence capture (byte {inc}) runs BEFORE {what} \
                 (byte {other}). `CalcIncMatrix` rewrites solution state and moves the \
                 active circuit element, and it must not precede the topology read that \
                 memoizes the branch list — so it comes strictly last in the step."
            ));
        }
    }
    Ok(())
}

/// The body of a top-level Python `def`: from the end of its header line to the
/// next line that begins in column 0 with a non-blank character.
fn py_def_body<'a>(text: &'a str, header: &str, rel: &str) -> Result<&'a str, String> {
    let (start, end) = py_def_span(text, header, rel)?;
    Ok(&text[start..end])
}

/// The byte span of [`py_def_body`].
fn py_def_span(text: &str, header: &str, rel: &str) -> Result<(usize, usize), String> {
    let at = text
        .find(header)
        .ok_or_else(|| format!("{rel}: `{header}` is gone — update this gate"))?;
    let start = text[at..].find('\n').map_or(text.len(), |i| at + i + 1);
    let end = text[start..]
        .match_indices('\n')
        .map(|(i, _)| start + i + 1)
        .find(|&s| text[s..].chars().next().is_some_and(|c| !c.is_whitespace()))
        .unwrap_or(text.len());
    Ok((start, end))
}

/// The body of a top-level Rust `fn`: from its header to the first line that is
/// exactly `}` in column 0 — rustfmt's shape for a free function.
///
/// The line terminator is read as LF **or** CRLF: this repo checks Rust out with
/// `core.autocrlf`, so a CRLF working copy must not make the gate claim the
/// closing brace is missing (it did, at the G1.8 lane merge).
fn rust_fn_body<'a>(text: &'a str, header: &str, rel: &str) -> Result<&'a str, String> {
    let (start, end) = rust_fn_span(text, header, rel)?;
    Ok(&text[start..end])
}

/// The byte span of [`rust_fn_body`].
fn rust_fn_span(text: &str, header: &str, rel: &str) -> Result<(usize, usize), String> {
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
    Ok((at, end))
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

/// One call of an [`engine_command_methods`] method in a Rust source.
struct CommandSite {
    /// The `fn` whose body holds the call.
    func: String,
    method: String,
    /// The argument list as written.
    args: String,
    /// The command, when the argument list is one string literal.
    literal: Option<String>,
    /// Char index of the method name in the source.
    at: usize,
}

/// The name a `name: [&]['a ][mut ][path::]Engine` binding declares, when the
/// `Engine` token that ends at `code[..at]` is the type of one.
fn engine_binding_before(code: &str) -> Option<String> {
    let mut head = code.trim_end();
    while let Some(h) = head.strip_suffix("::") {
        head = h.trim_end_matches(is_ident).trim_end();
    }
    if let Some(h) = head.strip_suffix("mut")
        && !h.ends_with(is_ident)
    {
        head = h.trim_end();
    }
    if let Some(i) = head.rfind('\'')
        && i + 1 < head.len()
        && head[i + 1..].chars().all(is_ident)
    {
        head = head[..i].trim_end();
    }
    head = head.strip_suffix('&').map_or(head, str::trim_end);
    let head = head.strip_suffix(':')?;
    if head.ends_with(':') {
        return None;
    }
    let head = head.trim_end();
    let name = &head[head.trim_end_matches(is_ident).len()..];
    (!name.is_empty()).then(|| name.to_string())
}

/// The command methods a std type shares: `node_order.clear()` empties a `Vec`.
const STD_SHARED_COMMANDS: &[&str] = &["clear"];

/// Every call of a command-issuing `Engine` method in `src`, in source order:
/// `<receiver>.<method>(…)`, and `<path>::<method>(…)` or `<path>::<method>`
/// handed on as a value (which has no argument list).
///
/// The gate reads no type, so any receiver and any path count: a parameter, a
/// field, a closure argument, a call chain; `Engine`, a rename, alias or
/// re-export of it declared in any module, a trait, a qualified `<…>` path, a
/// generic parameter. A same-named method of another type counts as well,
/// through a path always (`Vec::clear(&mut v)`), through a receiver unless the
/// one exception applies: a [`STD_SHARED_COMMANDS`] method called on a name a
/// plain `let` of the same `fn` binds before it, to something that is neither
/// an `Engine` nor a `let` alias of one ([`alias_of`]), is no command. A
/// binding typed `Engine` always counts. Comments and string literals are no
/// code ([`code_only`]); the argument list is reported as written.
fn rust_command_sites(src: &str) -> Vec<CommandSite> {
    let code = code_only(src, Lang::Rust);
    let cc: Vec<char> = code.chars().collect();
    let rc: Vec<char> = src.chars().collect();
    assert_eq!(cc.len(), rc.len(), "code_only must keep one char per char");

    let mut engines: Vec<String> = code
        .match_indices("Engine")
        .filter(|(at, _)| starts_token(&code, *at) && !code[at + 6..].starts_with(is_ident))
        .filter_map(|(at, _)| engine_binding_before(&code[..at]))
        .collect();
    loop {
        let before = engines.len();
        for line in code.lines() {
            if let Some(alias) = alias_of(line, Lang::Rust, &engines)
                && !engines.contains(&alias)
            {
                engines.push(alias);
            }
        }
        if engines.len() == before {
            break;
        }
    }
    // Every plain `let` of a non-engine name, as (char index of its line, name).
    let mut not_engines: Vec<(usize, String)> = Vec::new();
    let mut line_at = 0usize;
    for line in code.split('\n') {
        let bound = line.trim_start().strip_prefix("let ").and_then(|rest| {
            let rest = rest.strip_prefix("mut ").unwrap_or(rest);
            let name = ident_at(rest, 0);
            let after = rest[name.len()..].trim_start();
            (!name.is_empty() && (after.starts_with(':') || after.starts_with('='))).then_some(name)
        });
        if let Some(name) = bound
            && !engines.contains(&name)
        {
            not_engines.push((line_at, name));
        }
        line_at += line.chars().count() + 1;
    }

    // `fn` headers, as (char index, name).
    let mut heads: Vec<(usize, String)> = Vec::new();
    for i in 0..cc.len().saturating_sub(3) {
        if cc[i..i + 3] == ['f', 'n', ' '] && (i == 0 || !is_ident(cc[i - 1])) {
            let name: String = cc[i + 3..].iter().take_while(|c| is_ident(**c)).collect();
            if !name.is_empty() {
                heads.push((i, name));
            }
        }
    }

    let mut out: Vec<CommandSite> = Vec::new();
    for method in engine_command_methods() {
        let m: Vec<char> = method.chars().collect();
        for at in 0..cc.len().saturating_sub(m.len()) {
            if cc[at..at + m.len()] != m[..]
                || (at > 0 && is_ident(cc[at - 1]))
                || cc.get(at + m.len()).is_some_and(|c| is_ident(*c))
            {
                continue;
            }
            let open = (at + m.len()..cc.len())
                .find(|&k| !cc[k].is_whitespace())
                .filter(|&k| cc[k] == '(');
            let mut k = at;
            while k > 0 && cc[k - 1].is_whitespace() {
                k -= 1;
            }
            let path = |end: usize| -> String {
                let mut a = end;
                while a > 0 && cc[a - 1].is_whitespace() {
                    a -= 1;
                }
                let z = a;
                while a > 0 && is_ident(cc[a - 1]) {
                    a -= 1;
                }
                cc[a..z].iter().collect()
            };
            let head = heads.iter().rev().find(|(h, _)| *h < at);
            let fn_start = head.map_or(0, |(h, _)| *h);
            if k > 0 && cc[k - 1] == '.' {
                let recv = path(k - 1);
                let local = STD_SHARED_COMMANDS.contains(&method.as_str())
                    && not_engines
                        .iter()
                        .any(|(p, n)| *n == recv && *p > fn_start && *p < at);
                if open.is_none() || local {
                    continue;
                }
            } else if !(k > 1 && cc[k - 2..k] == [':', ':']) {
                continue;
            }
            let (args, literal) = match open {
                Some(open) => {
                    let mut depth = 0usize;
                    let mut close = cc.len();
                    for (j, c) in cc.iter().enumerate().skip(open) {
                        match c {
                            '(' => depth += 1,
                            ')' => {
                                depth -= 1;
                                if depth == 0 {
                                    close = j;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    let args: String = rc[open + 1..close].iter().collect();
                    let code_args: String = cc[open + 1..close].iter().collect();
                    let quoted = args.trim();
                    let literal = (code_args.trim().is_empty()
                        && quoted.len() >= 2
                        && quoted.starts_with('"')
                        && quoted.ends_with('"'))
                    .then(|| quoted[1..quoted.len() - 1].to_string());
                    (args, literal)
                }
                None => (String::new(), None),
            };
            let func = head.map_or_else(String::new, |(_, n)| n.clone());
            out.push(CommandSite {
                func,
                method: method.clone(),
                args,
                literal,
                at,
            });
        }
    }
    out.sort_by_key(|s| s.at);
    out
}

/// True when a `raw_command` argument list is a `?` property query.
fn is_property_query(args: &str) -> bool {
    let a = args.trim();
    ["&format!(\"? ", "format!(\"? ", "\"? "]
        .iter()
        .any(|p| a.starts_with(p))
}

/// The r4133 transport calls a command-issuing `Engine` method only at the
/// sites of `register` ([`R4133_COMMAND_SITES`] for the real source): the same
/// `(fn, method)` multiset, every `raw_command` a `?` property query, every run
/// command of `run_case` ahead of the G1.9 aggregates (the step's first
/// capture), and the `RelCalc` between the step's solve and those aggregates.
fn check_r4133_command_sites(
    src: &str,
    register: &[(&str, &str)],
    a: &Anchors,
    rel: &str,
) -> Result<(), String> {
    let sites = rust_command_sites(src);
    if let Some(s) = sites
        .iter()
        .find(|s| s.method == "raw_command" && !is_property_query(&s.args))
    {
        return Err(format!(
            "{rel}: `{}` calls `raw_command({})`, which is not a `?` property query. \
             The transport's only raw command lines are the probe and property \
             queries; any other command drives the engine mid-step.",
            s.func,
            s.args.trim()
        ));
    }

    let mut found: Vec<(String, String)> = sites
        .iter()
        .map(|s| (s.func.clone(), s.method.clone()))
        .collect();
    let mut want: Vec<(String, String)> = register
        .iter()
        .map(|(f, m)| ((*f).to_string(), (*m).to_string()))
        .collect();
    found.sort();
    want.sort();
    if found != want {
        let mut missing = want.clone();
        let mut extra: Vec<(String, String)> = Vec::new();
        for f in found {
            match missing.iter().position(|w| *w == f) {
                Some(i) => {
                    missing.remove(i);
                }
                None => extra.push(f),
            }
        }
        return Err(format!(
            "{rel}: the command-issuing `Engine` calls are not the registered ones: \
             unregistered {extra:?}, missing {missing:?}. A capture may not drive the \
             engine mid-step; the run, the once-per-case `RelCalc`, the `?` queries and \
             the G1.8 pair are the only commands (`R4133_COMMAND_SITES`). If the run \
             really changed, change the register with it."
        ));
    }

    let run = offset_after(src, a.run, 0, rel)?;
    let aggregates = src[..offset_after(src, a.aggregates, run, rel)?]
        .chars()
        .count();
    if let Some(s) = sites
        .iter()
        .find(|s| s.func == "run_case" && s.method != "relcalc" && s.at > aggregates)
    {
        return Err(format!(
            "{rel}: `run_case` calls `{}` (char {}) after the G1.9 aggregates (char \
             {aggregates}), i.e. among the captures of a step. The run's commands all come \
             ahead of the step's first capture, or a capture reads an engine the command \
             changed mid-step.",
            s.method, s.at
        ));
    }
    let solve = sites
        .iter()
        .filter(|s| s.method == "solve")
        .map(|s| s.at)
        .max();
    for r in sites.iter().filter(|s| s.method == "relcalc") {
        if solve.is_some_and(|s| r.at < s) || r.at > aggregates {
            return Err(format!(
                "{rel}: `RelCalc` (char {}) is not issued between the step's solve and \
                 the G1.9 aggregates (char {aggregates}). It writes the meter and bus \
                 reliability state, so it must run after the solve and ahead of every \
                 capture of the checkpoint.",
                r.at
            ));
        }
    }
    Ok(())
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
         The pair's ORDER is a contract (r4133's `CalcLaplacian` has no guard against \
         a missing incidence matrix and would crash the worker, while capi and the port \
         raise 8877), the read order is what both channels must share, and `CalcIncMatrix_O` / \
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
/// The pair is also checked **file-wide**: every `exec_wait` call of
/// `crates/dss-epri/src/capture.rs` passes a plain string literal (the gate
/// cannot read a computed command, as on the capi twin), and those literals are
/// exactly the pair. The transport's other commands — the run, the
/// once-per-case `RelCalc`, the `?` queries — are [`check_r4133_command_sites`]'s.
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
    let exec: Vec<CommandSite> = rust_command_sites(src)
        .into_iter()
        .filter(|s| s.method == "exec_wait")
        .collect();
    let file: Vec<String> = exec.iter().filter_map(|s| s.literal.clone()).collect();
    if let Some(s) = exec.iter().find(|s| s.literal.is_none()) {
        return Err(format!(
            "{rel}: `{}` calls `exec_wait({})`, a computed command: only {} of the \
             transport's {} `exec_wait` site(s) pass a plain string literal. This gate \
             cannot read a computed command — write it as a literal, or the rule that \
             the pair is the transport's only `exec_wait` is unverifiable.",
            s.func,
            s.args.trim(),
            file.len(),
            exec.len()
        ));
    }
    if file != owned(&INCIDENCE_PAIR) {
        return Err(incidence_error(
            rel,
            "the executive commands the whole transport issues through `exec_wait`",
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
                "{rel}: the transport issues `{bad}`. It builds the topology tree, which \
                 memoizes the branch list G1.7's `TOPOLOGY_STALE_DECLINES` / \
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
                 and sits on the bridge's \
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
                 inside the DLL, and removing the \
                 row would let a future accessor call it."
            ));
        }
    }
    Ok(())
}

/// The capi transport's two incidence normalizers must still REFUSE a shape they
/// were not written for.
///
/// `_inc_ints` implements rule N1 — drop the one trailing cell capi allocates and
/// never writes — and the sub-step's KILL CRITERION is that the cell is 0 and
/// the length is `3·NZero + 1` (`GOLDEN_REBASE_PLAN.md` §G1.8). Both are
/// enforced in Python, where no Rust test reaches them; an edit that kept the strip and dropped the zero-check
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

/// The r4133 transport issues commands only at [`R4133_COMMAND_SITES`], and the
/// rail has teeth on the real source: a stray `RelCalc` in a capture, the same
/// command through a `let` alias, a closure argument of any name, an untyped
/// binding, a path call, a fn pointer or a method handed to `map`, a `clear` on
/// a typed or an aliased engine binding, a command through any path (a renamed
/// import, a type alias, a qualified path, a trait, a re-export or an alias
/// another module declares, a generic parameter, a same-named method of
/// another type), a stray `exec_wait`, the `RelCalc` moved ahead of the solve
/// or past the aggregates, the run's `clear` moved past the aggregates, and a
/// `raw_command` that is no `?` query are each rejected.
#[test]
fn the_r4133_transport_issues_commands_only_at_its_registered_sites() {
    let rel = "crates/dss-epri/src/capture.rs";
    let src = read_source(rel);
    check_r4133_command_sites(&src, &R4133_COMMAND_SITES, &R4133_CALLS, rel)
        .unwrap_or_else(|e| panic!("{e}"));
    // The real `run_case` empties a `Vec` with `clear()` as well; the gate
    // tells it from the engine's `clear` by its `let` binding.
    assert!(
        src.contains("node_order.clear()"),
        "{rel}: the Vec `clear` is gone"
    );

    let red = |mutated: String, what: &str, needle: &str| {
        let err = check_r4133_command_sites(&mutated, &R4133_COMMAND_SITES, &R4133_CALLS, rel)
            .expect_err(what);
        assert!(err.contains(needle), "{what}: {err}");
    };
    let in_reliability = "let saifikw = engine.meters_saifi_kw()?;";
    red(
        insert_after(&src, in_reliability, "        engine.relcalc()?;"),
        "a stray RelCalc in the reliability capture",
        "(\"capture_reliability\", \"relcalc\")",
    );
    red(
        insert_after(
            &src,
            in_reliability,
            "        let again = engine;\n        again.relcalc()?;",
        ),
        "a RelCalc through a let alias",
        "(\"capture_reliability\", \"relcalc\")",
    );
    let in_topology = "let num_loops = engine.topology_num_loops()?;";
    red(
        insert_after(&src, in_topology, "    (|e| e.relcalc())(engine)?;"),
        "a RelCalc through a closure argument",
        "(\"capture_topology\", \"relcalc\")",
    );
    red(
        insert_after(&src, in_topology, "    Engine::relcalc(engine)?;"),
        "a RelCalc as a path call",
        "(\"capture_topology\", \"relcalc\")",
    );
    // A receiver of any name — one a `let` elsewhere binds to a non-engine, a
    // binding the gate cannot type — and the method handed on as a value.
    for (stray, what) in [
        ("    (|s| s.relcalc())(engine)?;", "a RelCalc through `|s|`"),
        (
            "    let q = make_engine();\n    q.relcalc()?;",
            "a RelCalc through an untyped binding",
        ),
        (
            "    let eng = Some(engine).unwrap();\n    eng.relcalc()?;",
            "a RelCalc through an unwrapped binding",
        ),
        (
            "    let f = Engine::relcalc;\n    f(engine)?;",
            "a RelCalc through a fn pointer",
        ),
        (
            "    Some(engine).map(Engine::relcalc).transpose()?;",
            "a RelCalc handed to `map`",
        ),
    ] {
        red(
            insert_after(&src, in_topology, stray),
            what,
            "(\"capture_topology\", \"relcalc\")",
        );
    }
    // A `clear` on a binding typed `Engine` or on a `let` alias of the engine is
    // the engine's, never a `Vec`'s.
    for (stray, what) in [
        (
            "    let e2: &Engine = engine;\n    e2.clear()?;",
            "a clear through a typed let",
        ),
        (
            "    let e3 = engine;\n    e3.clear()?;",
            "a clear through a let alias",
        ),
    ] {
        red(
            insert_after(&src, in_topology, stray),
            what,
            "(\"capture_topology\", \"clear\")",
        );
    }
    // A command method through any path: the gate reads no type, so a path
    // through a name another module declares counts as well as one this file
    // declares, and so does a same-named method of another type.
    let relcalc = "(\"capture_topology\", \"relcalc\")";
    let clear = "(\"capture_topology\", \"clear\")";
    for (stray, what, site) in [
        (
            "    use crate::dss::Engine as E;\n    E::exec_wait(engine, \"RelCalc\")?;",
            "an exec_wait through a renamed import",
            "(\"capture_topology\", \"exec_wait\")",
        ),
        (
            "    type Bridge = crate::dss::Engine;\n    Bridge::relcalc(engine)?;",
            "a RelCalc through a type alias",
            relcalc,
        ),
        (
            "    <Engine>::relcalc(engine)?;",
            "a RelCalc through a qualified path",
            relcalc,
        ),
        (
            "    <crate::dss::Engine>::relcalc(engine)?;",
            "a RelCalc through a qualified module path",
            relcalc,
        ),
        (
            "    <Engine as Refresh>::relcalc(engine)?;",
            "a RelCalc through a qualified trait path",
            relcalc,
        ),
        (
            "    Refresh::relcalc(engine)?;",
            "a RelCalc through a trait",
            relcalc,
        ),
        (
            "    crate::dss::Refresh::relcalc(engine)?;",
            "a RelCalc through a trait's module path",
            relcalc,
        ),
        (
            "    crate::dss::Handle::relcalc(engine)?;",
            "a RelCalc through a type alias another module declares",
            relcalc,
        ),
        (
            "    crate::Bridge::relcalc(engine)?;",
            "a RelCalc through a renamed re-export",
            relcalc,
        ),
        (
            "    T::relcalc(engine)?;",
            "a RelCalc through a generic parameter",
            relcalc,
        ),
        (
            "    crate::Bridge::clear(engine)?;",
            "a clear through a renamed re-export",
            clear,
        ),
        (
            "    Vec::clear(&mut node_order);",
            "a same-named method of another type through a path",
            clear,
        ),
    ] {
        red(insert_after(&src, in_topology, stray), what, site);
    }
    // A stray `exec_wait` through `|s|` is refused by both rails.
    let exec = insert_after(
        &src,
        in_topology,
        "    (|s| s.exec_wait(\"solve\"))(engine)?;",
    );
    red(
        exec.clone(),
        "an exec_wait through `|s|`",
        "(\"capture_topology\", \"exec_wait\")",
    );
    let err = check_r4133_incidence(&exec, rel).expect_err("an exec_wait through `|s|`");
    assert!(err.contains("the whole transport issues"), "{err}");
    // A run command moved among the captures of a step.
    red(
        move_line_after(
            &src,
            "engine.clear()?;",
            "let aggregates = capture_aggregates(engine, warn)?;",
        ),
        "the run's clear moved past the aggregates",
        "`run_case` calls `clear`",
    );
    red(
        move_line_after(
            &src,
            "Some(engine.relcalc()?)",
            "let aggregates = capture_aggregates(engine, warn)?;",
        ),
        "the RelCalc moved past the aggregates",
        "between the step's solve and the G1.9 aggregates",
    );
    red(
        move_line_after(
            &src,
            "Some(engine.relcalc()?)",
            "for step in 0..req.n_steps {",
        ),
        "the RelCalc moved ahead of the solve",
        "between the step's solve and the G1.9 aggregates",
    );
    red(
        src.replacen(
            "engine.raw_command(&format!(\"? {name}.Like\"));",
            "engine.raw_command(\"RelCalc\");",
            1,
        ),
        "a raw command that is no property query",
        "not a `?` property query",
    );
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
    \"\"\"NEVER CalcIncMatrix_O and never sol.BusLevels: the first builds
    the topology tree, the second overruns its array.\"\"\"
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
    assert!(err.contains("builds the topology tree"), "{err}");

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
/// `solution_bus_levels` — the second overruns its array.
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
    assert!(err.contains("missing incidence matrix"), "{err}");

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

    // A computed command is refused, not read as no command: the literal scan
    // alone still sees exactly the pair here.
    let computed = format!(
        "{rs_ok}\nfn other(e: &Engine, cmd: &str) {{\n    e.exec_wait(cmd).unwrap();\n}}\n"
    );
    assert_eq!(rust_commands(&computed), owned(&INCIDENCE_PAIR));
    let err =
        check_r4133_incidence(&computed, "synthetic").expect_err("a computed command was issued");
    assert!(err.contains("computed command"), "{err}");
    assert!(err.contains("exec_wait(cmd)"), "{err}");

    // -- the DO_NOT_CALL register -------------------------------------------
    let modes_ok = "\
pub const DO_NOT_CALL: &[(&str, ModeKind, i32, &str)] = &[
    (
        \"Solution\",
        ModeKind::V,
        2,
        \"Solution.BusLevels: writes one element past its array\",
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
         capture_reliability(x); capture_all_properties(x); capture_inc_matrix(x); \
         capture_topology(x);";
    let err = check_inc_matrix_last(before_topology, &a, "synthetic")
        .expect_err("the incidence pair ran before the topology read");
    assert!(err.contains("topology capture"), "{err}");
    assert!(err.contains("memoizes the branch list"), "{err}");

    // Topology first this time, so its arm is satisfied and the failure has to
    // come from one of the reads the pair still overtakes.
    let early = "fn run_case( capture_aggregates(x); capture_discrete(x); capture_reliability(x); \
                 capture_topology(x); capture_inc_matrix(x); capture_all_elements(x); \
                 capture_all_properties(x);";
    let err = check_inc_matrix_last(early, &a, "synthetic").expect_err("the pair ran early");
    assert!(err.contains("property sweep"), "{err}");

    // Every other capture in place, only the reliability capture moved past the
    // pair: the pair would run before the reads of the post-`RelCalc` state.
    let before_reliability = "fn run_case( capture_aggregates(x); capture_discrete(x); \
         capture_all_elements(x); capture_all_properties(x); capture_topology(x); \
         capture_inc_matrix(x); capture_reliability(x);";
    let err = check_inc_matrix_last(before_reliability, &a, "synthetic")
        .expect_err("the incidence pair ran before the reliability capture");
    assert!(err.contains("reliability capture"), "{err}");

    let renamed = "fn run_case( capture_aggregates(x); capture_discrete(x); \
                   capture_all_elements(x); capture_reliability(x); capture_all_properties(x); \
                   capture_topology(x); build_the_matrix(x);";
    let err = check_inc_matrix_last(renamed, &a, "synthetic").expect_err("the anchor is gone");
    assert!(err.contains("could not find"), "{err}");
}

// ---------------------------------------------------------------------------
// G1.10a — the run-file set: a per-RUN read, strictly last, inside the guard
// scope, with nothing but the D32(2)(a) teardown after it
// (`GOLDEN_REBASE_PLAN.md` §G1.10; coordinator decisions D32(2)(a) and D33(3)).
// ---------------------------------------------------------------------------

/// Where one transport classifies the run's created-file set, and the scope
/// that classification must sit inside.
///
/// The two transports express that scope differently — Python opens it with
/// `with _CorpusGuard(…) as guard:` and ends it by indentation, Rust binds the
/// guard and consumes it with an explicit `finish()` — so the rule carries both
/// spellings and [`check_run_tail_order`] resolves the end accordingly.
struct RunFileRule {
    lang: Lang,
    /// The G1.10c demand-interval read: the statement that copies the
    /// run-created `DI_yr_*` tree out of the case directory. It must appear
    /// exactly once in the transport's **code**, after the `autoadd_log` read
    /// and before both [`Self::created`] and [`Self::teardown`].
    di: &'static str,
    /// The classification call. It must appear exactly once in the transport's
    /// **code**: a second classification would report one set while the guard
    /// sweeps another.
    ///
    /// The clause counts the literal spelling below, so the DI read's own
    /// classification pass (`created_di_files` → `classify`, one statement
    /// earlier) is not one of them (G1.10c audit settlement, finding AT3-6).
    /// That door is bounded rather than closed: a DI file appearing BETWEEN the
    /// two passes lands in `created()` and in the sweep but not in the sidecar,
    /// which reaches the gate as a file-set or row-count RED, never as a silent
    /// pass — and both oracles write DI rows only inside `solve` / `closedi`,
    /// which have long returned by then.
    created: &'static str,
    /// The WPG.5 `autoadd_log` read — a file read off disk, of a file the run
    /// itself created, so it must precede the classification.
    autoadd: &'static str,
    /// Where the guard's scope opens.
    guard_open: &'static str,
    /// The statement that ends the guard's life where the language needs one
    /// (the explicit sweep, Rust side). `None` on the Python side, where the
    /// scope is the `with` block and its end is found by indentation.
    guard_close: Option<&'static str>,
    /// The name the classification binds — the created-file SET. The G1.10b
    /// contents copy must CONSUME it, so it can never hand back a file the set
    /// does not name.
    created_var: &'static str,
    /// `GOLDEN_REBASE_PLAN.md` G1.10b, coordinator decision D40(6) — the
    /// CONTENTS copy, as line prefixes in order (a call wrapped over several
    /// lines is several entries, because the check below reads the tail line by
    /// line). It is the ONE thing that may sit between the classification and
    /// the teardown, it consumes [`Self::created_var`], and it copies bytes —
    /// it issues no command and performs no model read, so the classification is
    /// still the last READ of the run (D33(3)).
    /// [`check_run_file_contents_read_with_the_set`] states that relation on its
    /// own; declaring it here is what keeps the statement whitelist exact.
    contents: &'static [&'static str],
    /// Whether the contents copy comes BEFORE this transport's teardown.
    ///
    /// True on r4133, which has no teardown at all. **False on capi, and that is
    /// MEASURED** (G1.10b F1): dss_capi 0.14.5 holds a Storage `debugtrace`
    /// stream open for the life of the object with a share mode that denies
    /// READ, so before the teardown `clear` the file cannot be opened at all
    /// (`PermissionError: [Errno 13]`), and after it reads whole (28 777 bytes on
    /// `Storage_price.dss`). Moving the copy past the teardown is safe for every
    /// other selected report because the export writers close their file as they
    /// return — `fbs_EXP_VOLTAGES.csv` is byte-identical before and after the
    /// teardown (349 bytes, sha256:0e8329e54fab84ee, measured on the same probe).
    contents_first: bool,
    /// The EXACT statement sequence this transport may still run after the
    /// classification AND its contents copy, as line prefixes, in order
    /// (D33(3)). Empty for a
    /// transport that needs no teardown. Pinning the sequence rather than a
    /// membership set is what states F4c's half of the rule: the teardown
    /// `clear` is the sole statement of its `try` block, because the statement
    /// before it is that `try:` and the one after it is the `except`.
    teardown: &'static [&'static str],
    /// The engine commands that teardown issues, in order — the semantic half
    /// of the same rule: no other command, and no read, may follow, to the end
    /// of `run_case`.
    teardown_commands: &'static [&'static str],
    /// The statements `run_case` runs after the guard scope and before its
    /// reply, as whole lines in order: the sweep report (and, on r4133, the
    /// contents copy's deferred error). They read the guard, never the model.
    epilogue: &'static [&'static str],
    /// The opening and closing tokens of the reply literal that ends
    /// `run_case`. Every entry between them must be a name bound earlier or a
    /// field of the request ([`is_reply_entry`]), so no read and no command can
    /// hide in the reply.
    reply: (&'static str, &'static str),
}

/// The capi channel. Its teardown is D32(2)(a): dss_capi opens a Storage
/// `debugtrace` stream at edit time and keeps it open until the element is
/// re-edited or destroyed, so the circuit is released with one `clear` — after
/// the classification — or the guard's
/// `os.remove` raises and the next producer of the same case snapshots the
/// leaked file as pre-existing. D33(1) wraps that `clear` in `try:` because the
/// pinned 0.14.5 faults on it after an AutoAdd solve; the recording arms are on
/// the whitelist for exactly that reason.
const CAPI_RUN_FILES: RunFileRule = RunFileRule {
    lang: Lang::Python,
    di: "di = capture_di(guard",
    created: "guard.created()",
    created_var: "run_files",
    contents: &[
        "run_file_contents = _copy_selected_contents(",
        "guard.dir, run_files, contents_patterns, contents_dir",
        ")",
    ],
    contents_first: false,
    autoadd: "autoadd_log = fh.read()",
    guard_open: "with _CorpusGuard(",
    guard_close: None,
    teardown: &[
        "teardown_error = None",
        "try:",
        "d.Text.Command = \"clear\"",
        "except Exception as e:",
        "teardown_error = f\"",
        "log(f\"teardown clear raised",
    ],
    teardown_commands: &["clear"],
    epilogue: &["sweep_failed = guard.sweep_failed"],
    reply: ("return {", "}"),
};

/// The r4133 channel. r4133 closes its own Storage trace file as it writes the
/// header, so this transport needs no teardown at all: after the classification
/// the guard is swept and the reply is built, and **nothing** may run in
/// between.
const R4133_RUN_FILES: RunFileRule = RunFileRule {
    lang: Lang::Rust,
    di: "let di = if req.di",
    created: "guard.created()",
    created_var: "run_files",
    contents: &[
        "let run_file_contents = copy_selected_contents(",
        "guard.dir(),",
        "run_files.as_deref(),",
        "&req.run_file_contents,",
        "req.run_file_contents_dir.as_deref(),",
        ");",
    ],
    contents_first: true,
    autoadd: "read_autoadd_log(engine",
    guard_open: "CorpusGuard::new(",
    guard_close: Some("guard.finish()"),
    teardown: &[],
    teardown_commands: &[],
    epilogue: &[
        "let sweep_failed = guard.finish();",
        "let run_file_contents = run_file_contents.map_err(EngineError::Other)?;",
    ],
    reply: ("Ok(CaseResult {", "})"),
};

/// Byte offset of the start of the line containing `at`.
fn line_start_at(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |i| i + 1)
}

/// The indentation (leading spaces) of the line containing `at`.
fn indent_at(text: &str, at: usize) -> usize {
    text[line_start_at(text, at)..]
        .chars()
        .take_while(|c| *c == ' ')
        .count()
}

/// Byte offset of the first non-blank line strictly after the line containing
/// `at` (`text.len()` if there is none).
fn next_code_line(text: &str, at: usize) -> usize {
    let mut i = text[at..].find('\n').map_or(text.len(), |j| at + j + 1);
    while i < text.len() {
        let end = text[i..].find('\n').map_or(text.len(), |j| i + j + 1);
        if !text[i..end].trim().is_empty() {
            return i;
        }
        i = end;
    }
    text.len()
}

/// The end of the indented block whose header line contains `at`: the first
/// later non-blank line indented no deeper than that header. Python only — it
/// is how `with _CorpusGuard(…) as guard:` states the guard's lifetime.
fn py_block_end(text: &str, at: usize) -> usize {
    let head = indent_at(text, at);
    let mut i = text[at..].find('\n').map_or(text.len(), |j| at + j + 1);
    while i < text.len() {
        let end = text[i..].find('\n').map_or(text.len(), |j| i + j + 1);
        if !text[i..end].trim().is_empty() && indent_at(text, i) <= head {
            return i;
        }
        i = end;
    }
    text.len()
}

/// The whole run-TAIL rule as a pure function of one transport's source text —
/// G1.10a's created-file classification and G1.10c's demand-interval read, whose
/// slots are defined against each other — so it can be shown to have teeth
/// ([`the_run_file_gate_rejects_an_early_escaped_or_nested_classification`],
/// [`the_run_tail_gate_rejects_a_misplaced_di_read`]).
fn check_run_tail_order(src: &str, a: &Anchors, r: &RunFileRule, rel: &str) -> Result<(), String> {
    let run = offset_after(src, a.run, 0, rel)?;
    let created = offset_after(src, r.created, run, rel)?;
    let guard_open = offset_after(src, r.guard_open, run, rel)?;

    // 1. one classification, so the set reported IS the set swept.
    let n = code_only(src, r.lang).matches(r.created).count();
    if n != 1 {
        return Err(format!(
            "{rel}: `{}` appears {n} times in this transport's code. The run-file \
             classification runs exactly once, at the end of the run — a second one \
             would report a set the guard does not sweep (or sweep a set it did not \
             report).",
            r.created
        ));
    }

    // 2. after every per-step capture: a report a LATER step writes must already
    //    be on disk when the set is classified.
    for (what, needle) in [
        ("the G1.8 incidence capture", a.inc_matrix),
        ("the G1.7 topology capture", a.topology),
        ("the WP8.5b property sweep", a.properties),
        ("the G1.6(i) reliability capture", a.reliability),
        ("the per-element capture", a.elements),
        ("the discrete-state capture", a.discrete),
        ("the G1.9 aggregates", a.aggregates),
    ] {
        let other = offset_after(src, needle, run, rel)?;
        if created <= other {
            return Err(format!(
                "{rel}: the G1.10a run-file classification (byte {created}) runs BEFORE \
                 {what} (byte {other}). It is a per-RUN read of the filesystem and must \
                 follow every per-step read, or a file a later step writes is missing \
                 from the compared set (`GOLDEN_REBASE_PLAN.md` §G1.10)."
            ));
        }
    }

    // 3. after the `autoadd_log` read — that log is one of the created files,
    //    read off disk while the guard still holds it.
    let autoadd = offset_after(src, r.autoadd, run, rel)?;
    if created <= autoadd {
        return Err(format!(
            "{rel}: the run-file classification (byte {created}) runs BEFORE the WPG.5 \
             `autoadd_log` read (byte {autoadd}). The classification is the LAST read of \
             the run (D33(3)); the AutoAddLog is one of the files it classifies."
        ));
    }

    // The guard's scope and the indentation of its own statements — the frame
    // both the classification (clauses 4/5) and the G1.10c demand-interval read
    // (clause 3b) are placed in.
    let scope_end = match r.guard_close {
        Some(close) => line_start_at(src, offset_after(src, close, guard_open, rel)?),
        None => py_block_end(src, guard_open),
    };
    let scope_indent = match r.lang {
        Lang::Python => indent_at(src, next_code_line(src, guard_open)),
        Lang::Rust => indent_at(src, guard_open),
    };

    // 3b. GOLDEN_REBASE G1.10c (coordinator decision D42(6)): the
    //     demand-interval tree's CONTENTS are copied out in this same tail, and
    //     their slot is measured, not cosmetic — see this file's fourth
    //     ordering rule. The frame is checked first (exactly once, inside the
    //     scope, not nested), then the three slot comparisons in the order a
    //     read can drift through them, so every mistake reports its own
    //     violation instead of the next one.
    let di = offset_after(src, r.di, run, rel)?;
    let n_di = code_only(src, r.lang).matches(r.di).count();
    if n_di != 1 {
        return Err(format!(
            "{rel}: `{}` appears {n_di} times in this transport's code. The \
             demand-interval tree is copied out exactly once, in the run tail — a \
             second read would report one tree while the sidecar holds another.",
            r.di
        ));
    }
    if di <= guard_open || di >= scope_end {
        return Err(format!(
            "{rel}: the G1.10c demand-interval read (byte {di}) sits OUTSIDE the guard \
             scope (bytes {guard_open}..{scope_end}). The guard sweeps the very tree \
             it copies out; outside the scope the files are already gone."
        ));
    }
    let di_indent = indent_at(src, di);
    if di_indent != scope_indent {
        return Err(format!(
            "{rel}: the G1.10c demand-interval read is indented {di_indent} where the \
             guard scope's own statements are indented {scope_indent} — it is nested. \
             The retry loop recompiles in-process; a read inside it copies out the \
             tree of an attempt that was thrown away."
        ));
    }
    if di <= autoadd {
        return Err(format!(
            "{rel}: the G1.10c demand-interval read (byte {di}) runs BEFORE the WPG.5 \
             `autoadd_log` read (byte {autoadd}). It belongs to the run TAIL, after \
             every per-step read and after the AutoAddLog, or it describes a \
             filesystem the run had not finished writing."
        ));
    }
    if let Some(first) = r.teardown.first() {
        // Resolved from the guard's OPENING, not from the classification: a
        // mutation that moves the classification past the teardown must still
        // be told where the teardown is, or this clause would report a missing
        // anchor instead of the misplacement it is looking for.
        let teardown = offset_after(src, first, guard_open, rel)?;
        if di >= teardown {
            return Err(format!(
                "{rel}: the G1.10c demand-interval read (byte {di}) runs AFTER this \
                 transport's D32(2)(a) teardown (byte {teardown}). The teardown \
                 `clear` destroys the circuit, which FLUSHES the in-flight \
                 demand-interval cycle over the file on the capi channel (measured: \
                 8/8, 7/8 and 4/6 file digests move on the three DI cases whose run \
                 leaves the streams open, while r4133's `clear` moves none), so a \
                 read placed after it compares one engine's NEXT cycle against the \
                 port's previous one."
            ));
        }
    }
    if di >= created {
        return Err(format!(
            "{rel}: the G1.10c demand-interval read (byte {di}) runs AFTER the \
             run-file classification (byte {created}), which is the LAST read of the \
             run (D33(3)). The DI contents are read in front of it, from the same \
             guard, while the tree is still on disk."
        ));
    }

    // 4. inside the guard scope: the sweep removes exactly what it returns.
    if created <= guard_open || created >= scope_end {
        return Err(format!(
            "{rel}: the run-file classification (byte {created}) sits OUTSIDE the guard \
             scope (bytes {guard_open}..{scope_end}). The guard sweeps exactly what the \
             classification returns; outside the scope the two can disagree, and the \
             files the set names are already gone."
        ));
    }

    // 5. a direct statement of that scope, not nested in the retry loop — which
    //    recompiles in-process, so a classification inside it describes the
    //    wrong attempt's filesystem.
    let created_indent = indent_at(src, created);
    if created_indent != scope_indent {
        return Err(format!(
            "{rel}: the run-file classification is indented {created_indent} where the \
             guard scope's own statements are indented {scope_indent} — it is nested. \
             The retry loop recompiles in-process; a classification inside it reports \
             the filesystem of an attempt that was thrown away."
        ));
    }

    // 6. D33(3): the classification is the last READ of the run. To the end of
    //    `run_case` no command but the D32(2)(a) teardown and no capture may
    //    follow it; inside the guard scope only the contents copy and that
    //    teardown, past the scope only the declared epilogue and the reply.
    let tail_start = src[created..]
        .find('\n')
        .map_or(src.len(), |i| created + i + 1);
    let run_end = match r.lang {
        Lang::Python => py_def_span(src, a.run, rel)?.1,
        Lang::Rust => rust_fn_span(src, a.run, rel)?.1,
    };
    let tail = src.get(tail_start..scope_end).unwrap_or("");
    let rest = src.get(tail_start..run_end).unwrap_or("");

    let cmds = match r.lang {
        Lang::Python => py_commands(rest),
        Lang::Rust => rust_commands(rest),
    };
    if cmds != owned(r.teardown_commands) {
        return Err(format!(
            "{rel}: the commands issued after the run-file classification, to the end of \
             `run_case`, are {cmds:?}, but the only thing allowed to follow it is the \
             D32(2)(a) teardown {:?} (D33(3)). Any other command changes the run whose \
             filesystem effect was just classified.",
            r.teardown_commands
        ));
    }

    let code = code_only(rest, r.lang);
    if let Some(read) = idents_with_prefix(&code, "capture_").first() {
        return Err(format!(
            "{rel}: `{read}` is called AFTER the run-file classification (inside the guard \
             scope or past it, before `run_case` returns). The classification is the LAST \
             read of the run (D33(3)); a capture that follows it reads a model the \
             reported file set no longer describes."
        ));
    }

    let stmts: Vec<&str> = tail
        .lines()
        .map(|l| code_of(l, r.lang).trim())
        .filter(|s| !s.is_empty())
        .collect();
    // G1.10b: the contents copy and the D32(2)(a) teardown, in this transport's
    // measured order ([`RunFileRule::contents_first`]). Nothing else, ever.
    let allowed: Vec<&str> = if r.contents_first {
        r.contents
            .iter()
            .chain(r.teardown.iter())
            .copied()
            .collect()
    } else {
        r.teardown
            .iter()
            .chain(r.contents.iter())
            .copied()
            .collect()
    };
    for (i, stmt) in stmts.iter().enumerate() {
        match allowed.get(i) {
            Some(want) if stmt.starts_with(want) => {}
            _ => {
                return Err(format!(
                    "{rel}: statement {i} after the run-file classification is `{stmt}`, \
                     but D33(3) + D40(6) allow exactly the G1.10b contents copy and then \
                     the D32(2)(a) teardown, in this order: {allowed:?}. The \
                     classification is the last READ of the run; the copy moves bytes \
                     and the teardown is not a read, and that `clear` is the SOLE \
                     statement of its `try` block. If either changed, change this rule \
                     with it — deliberately."
                ));
            }
        }
    }
    if stmts.len() != allowed.len() {
        return Err(format!(
            "{rel}: the guard scope ends after {} statement(s) following the run-file \
             classification, but the declared G1.10b contents copy plus the D32(2)(a) \
             teardown have {}: {allowed:?}. A teardown that shrank silently is the leak \
             D32(2)(a) closed (dss_capi's Storage trace stream) coming back, and a \
             contents copy that vanished silently is the G1.10b surface comparing nothing.",
            stmts.len(),
            allowed.len(),
        ));
    }
    check_run_epilogue(src.get(scope_end..run_end).unwrap_or(""), r, rel)
}

/// The statements of `run_case` past the guard scope (clause 6 of
/// [`check_run_tail_order`]): exactly [`RunFileRule::epilogue`], each line
/// equal to its entry (so nothing can be folded onto it), then the reply
/// literal [`RunFileRule::reply`], every entry of which [`is_reply_entry`]
/// admits. All of it runs after the run-file classification, the LAST read of
/// the run (D33(3)).
fn check_run_epilogue(after: &str, r: &RunFileRule, rel: &str) -> Result<(), String> {
    let stmts: Vec<&str> = after
        .lines()
        .map(|l| code_of(l, r.lang).trim())
        .filter(|s| !s.is_empty())
        .collect();
    for (i, want) in r.epilogue.iter().enumerate() {
        match stmts.get(i) {
            Some(stmt) if stmt == want => {}
            found => {
                return Err(format!(
                    "{rel}: statement {i} after the guard scope is {found:?}, but `run_case` \
                     runs exactly its epilogue {:?} there and then replies. The run-file \
                     classification is the LAST read of the run (D33(3)); anything past the \
                     scope runs after it.",
                    r.epilogue
                ));
            }
        }
    }
    let reply = stmts[r.epilogue.len()..].join(" ");
    let (open, close) = r.reply;
    let inner = reply
        .strip_prefix(open)
        .and_then(|s| s.strip_suffix(close))
        .ok_or_else(|| {
            format!(
                "{rel}: after its epilogue `run_case` must end in the reply `{open} … {close}`, \
                 found `{reply}` — a statement after the guard scope runs after the run-file \
                 classification, the LAST read of the run (D33(3))."
            )
        })?;
    for entry in inner.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        if !is_reply_entry(entry) {
            return Err(format!(
                "{rel}: the reply entry `{entry}` is not a bound name. The reply is built \
                 after the run-file classification, the LAST read of the run (D33(3)), so \
                 it hands back values read before it — it reads nothing itself."
            ));
        }
    }
    Ok(())
}

/// A reply entry clause 6 admits: a bound name (Rust's field shorthand), or a
/// key — `"key"` (Python) or `key` (Rust) — mapped to a bound name or to a field
/// of the request (`req.<field>`).
fn is_reply_entry(entry: &str) -> bool {
    let name =
        |s: &str| s.chars().next().is_some_and(|c| !c.is_ascii_digit()) && s.chars().all(is_ident);
    let value = |v: &str| name(v) || v.strip_prefix("req.").is_some_and(name);
    match entry.split_once(':') {
        None => name(entry),
        Some((key, v)) => {
            let key = key.trim();
            let key = key
                .strip_prefix('"')
                .and_then(|k| k.strip_suffix('"'))
                .unwrap_or(key);
            name(key) && value(v.trim())
        }
    }
}

#[test]
fn capi_capture_classifies_the_run_files_last() {
    let rel = "tools/oracle/oracle_server.py";
    check_run_tail_order(&read_source(rel), &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn r4133_capture_classifies_the_run_files_last() {
    let rel = "crates/dss-epri/src/capture.rs";
    check_run_tail_order(&read_source(rel), &R4133_CALLS, &R4133_RUN_FILES, rel)
        .unwrap_or_else(|e| panic!("{e}"));
}

/// `GOLDEN_REBASE_PLAN.md` G1.10b, coordinator decision D40(6) — the CONTENTS
/// copy is read WITH the set, from the set, inside the guard scope.
///
/// The set comparator (G1.10a) and the contents comparator must describe the
/// same instant of the filesystem, so the copy has to sit at the same strictly
/// last point as the classification. Four clauses, each a pure function of one
/// transport's source text so they can be shown to have teeth
/// ([`the_run_file_contents_gate_rejects_an_early_detached_or_duplicated_copy`]):
///
/// 1. the copy appears exactly once in the transport's code — a second one would
///    overwrite the sidecar with a different run's bytes;
/// 2. it sits inside the guard scope, whose sweep removes exactly those files;
/// 3. it is the FIRST statement after the classification, so nothing can write
///    (or delete) a selected report in between;
/// 4. it CONSUMES the classified set, so it can only ever hand back files that
///    set names — the selection is then a pure function of the set on all three
///    producers (`dss_epri::guard::selects_contents`).
fn check_run_file_contents_read_with_the_set(
    src: &str,
    a: &Anchors,
    r: &RunFileRule,
    rel: &str,
) -> Result<(), String> {
    let head =
        r.contents.first().copied().ok_or_else(|| {
            format!("{rel}: this transport declares no G1.10b contents copy at all")
        })?;
    let run = offset_after(src, a.run, 0, rel)?;
    let created = offset_after(src, r.created, run, rel)?;
    let copy = offset_after(src, head, run, rel)?;

    // 1. exactly once.
    let n = code_only(src, r.lang).matches(head).count();
    if n != 1 {
        return Err(format!(
            "{rel}: `{head}` appears {n} times in this transport's code. The run-file \
             CONTENTS are copied exactly once, with the classification — a second copy \
             wipes the gate's sidecar and refills it from a different instant of the \
             run (the copy clears the directory before it writes, \
             `dss_epri::guard::copy_selected_contents`)."
        ));
    }

    // 2. inside the guard scope, whose sweep removes exactly these files.
    let guard_open = offset_after(src, r.guard_open, run, rel)?;
    let scope_end = match r.guard_close {
        Some(close) => line_start_at(src, offset_after(src, close, guard_open, rel)?),
        None => py_block_end(src, guard_open),
    };
    if copy <= guard_open || copy >= scope_end {
        return Err(format!(
            "{rel}: the G1.10b contents copy (byte {copy}) sits OUTSIDE the guard scope \
             (bytes {guard_open}..{scope_end}). The guard sweeps exactly the files it \
             classified; outside its scope they are already gone."
        ));
    }

    // 3. the copy's SLOT in the tail after the classification: immediately after
    //    it on a transport with no teardown, and immediately after the teardown
    //    on capi, whose held Storage stream is unreadable until that `clear`
    //    closes it ([`RunFileRule::contents_first`] carries the measurement).
    let after = src[created..]
        .find('\n')
        .map_or(src.len(), |i| created + i + 1);
    let tail: Vec<&str> = src[after..scope_end]
        .lines()
        .map(|l| code_of(l, r.lang).trim())
        .filter(|s| !s.is_empty())
        .collect();
    let want = if r.contents_first {
        0
    } else {
        r.teardown.len()
    };
    let at = tail.iter().position(|s| s.starts_with(head));
    if at != Some(want) {
        return Err(format!(
            "{rel}: the G1.10b contents copy `{head}` is statement {at:?} of the tail \
             after the run-file classification, but this transport declares it as \
             statement {want} ({tail:?}). The SET and the BYTES describe the same \
             instant of the filesystem, so only the declared D32(2)(a) teardown may \
             stand between them — on capi it MUST, because its held Storage \
             `debugtrace` stream denies READ until \
             that `clear` closes it, and on r4133 there is no teardown to stand there."
        ));
    }

    // 4. it consumes the classified set.
    let stmt_end = {
        let mut e = line_start_at(src, copy);
        for _ in 0..r.contents.len() {
            e = src[e..].find('\n').map_or(src.len(), |j| e + j + 1);
        }
        e
    };
    let stmt = &src[line_start_at(src, copy)..stmt_end];
    if !stmt.contains(r.created_var) {
        return Err(format!(
            "{rel}: the G1.10b contents copy does not consume `{}` — the name the \
             classification binds. It must, or the selection stops being a pure \
             function of the created-file set and a transport can hand back a file the \
             set does not name (or miss one it does).",
            r.created_var
        ));
    }

    Ok(())
}

#[test]
fn capi_capture_copies_the_run_file_contents_with_the_set() {
    let rel = "tools/oracle/oracle_server.py";
    check_run_file_contents_read_with_the_set(&read_source(rel), &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn r4133_capture_copies_the_run_file_contents_with_the_set() {
    let rel = "crates/dss-epri/src/capture.rs";
    check_run_file_contents_read_with_the_set(
        &read_source(rel),
        &R4133_CALLS,
        &R4133_RUN_FILES,
        rel,
    )
    .unwrap_or_else(|e| panic!("{e}"));
}

/// Non-vacuity for the four clauses above, on the synthetic capi shape and then
/// on the REAL transport sources (corrupted in memory only — nothing on disk is
/// touched).
#[test]
fn the_run_file_contents_gate_rejects_an_early_detached_or_duplicated_copy() {
    let rel = "synthetic";
    let ok = SYNTH_CAPI_RUN;
    check_run_file_contents_read_with_the_set(ok, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .unwrap_or_else(|e| panic!("{e}"));

    // (3) a statement wedged between the classification and the copy.
    let wedged = insert_after(ok, "guard.created()", "        run_files.sort()");
    let err = check_run_file_contents_read_with_the_set(&wedged, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("a statement ran between the set and its bytes");
    assert!(err.contains("run_files.sort()"), "{err}");

    // (3) the copy hoisted BEFORE the classification: it would then select from
    // a set nobody has classified yet.
    let early = move_line_after(ok, "run_file_contents = _copy", "autoadd_log = fh.read()");
    let err = check_run_file_contents_read_with_the_set(&early, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("copied before the classification");
    assert!(err.contains("is statement None"), "{err}");

    // (3) the capi copy pulled IN FRONT of the teardown — the measured slot:
    // dss_capi's held Storage `debugtrace` stream denies READ until that `clear`
    // closes it, so the copy would fail on `STOR_<name>.csv` with
    // `PermissionError: [Errno 13]`.
    let before_teardown = move_line_after(
        ok,
        "run_file_contents = _copy_selected_contents(",
        "run_files = guard.created()",
    );
    let err = check_run_file_contents_read_with_the_set(
        &before_teardown,
        &CAPI_CALLS,
        &CAPI_RUN_FILES,
        rel,
    )
    .expect_err("copied before the teardown released the trace stream");
    assert!(err.contains("denies READ until"), "{err}");

    // (4) a copy that does not consume the classified set.
    let detached = ok.replace(
        "guard.dir, run_files, contents_patterns, contents_dir",
        "guard.dir, guard.created(), contents_patterns, contents_dir",
    );
    let err =
        check_run_file_contents_read_with_the_set(&detached, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
            .expect_err("the copy re-classified instead of consuming the set");
    assert!(err.contains("does not consume `run_files`"), "{err}");

    // (1) a second copy.
    let twice = insert_after(
        ok,
        "contents_patterns, contents_dir",
        "        run_file_contents = _copy_selected_contents(again)",
    );
    let err = check_run_file_contents_read_with_the_set(&twice, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("copied twice");
    assert!(err.contains("appears 2 times"), "{err}");

    // the anchor itself: a rename must fail loudly, never vacuously pass.
    let renamed = ok.replace("_copy_selected_contents(", "_stash_contents(");
    let err =
        check_run_file_contents_read_with_the_set(&renamed, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
            .expect_err("the anchor is gone");
    assert!(err.contains("could not find"), "{err}");

    // (2) on the REAL r4133 source: the copy pushed past the sweep.
    let rel = "crates/dss-epri/src/capture.rs";
    let src = read_source(rel);
    let escaped = move_line_after(
        &src,
        "let run_file_contents = copy_selected_contents(",
        "let sweep_failed = guard.finish();",
    );
    let err =
        check_run_file_contents_read_with_the_set(&escaped, &R4133_CALLS, &R4133_RUN_FILES, rel)
            .expect_err("copied after the sweep");
    assert!(err.contains("OUTSIDE the guard scope"), "{err}");

    // ... and on the REAL capi source: the copy dropped altogether.
    let rel = "tools/oracle/oracle_server.py";
    let src = read_source(rel);
    let dropped = drop_line(&src, "run_file_contents = _copy_selected_contents(");
    let err =
        check_run_file_contents_read_with_the_set(&dropped, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
            .expect_err("the copy is gone");
    assert!(err.contains("could not find"), "{err}");
}

/// A synthetic capi `run_case` in the real transport's shape, so the mutations
/// below run against the SAME [`CAPI_CALLS`] / [`CAPI_RUN_FILES`] constants the
/// two gates above apply to the real sources.
const SYNTH_CAPI_RUN: &str = r#"
def run_case(d, req):
    with _CorpusGuard(case_path) as guard:
        for attempt in range(1, 3):
            capture_aggregates(ckt)
            capture_all_elements(ckt, derived)
            gc.capture_discrete(ckt)
            capture_reliability(ckt)
            capture_all_properties(d, ckt)
            capture_topology(ckt)
            capture_inc_matrix(d, ckt)
        if want_autoadd_log:
            autoadd_log = fh.read()
        di = capture_di(guard, di_dir, case_path) if want_di else None
        run_files = guard.created() if want_run_files else None
        teardown_error = None
        try:
            d.Text.Command = "clear"
        except Exception as e:
            teardown_error = f"{e}"
            log(f"teardown clear raised for {case_path}: {teardown_error}")
        run_file_contents = _copy_selected_contents(
            guard.dir, run_files, contents_patterns, contents_dir
        )
    sweep_failed = guard.sweep_failed
    return {}
"#;

/// The r4133 `run_case` in the real transport's shape — the Rust spelling of the
/// guard scope, where the guard is consumed by an explicit `finish()` and the
/// tail holds the G1.10b contents copy alone (that transport has no teardown).
/// Shared by the two
/// mutation tests below so both run against the SAME [`R4133_CALLS`] /
/// [`R4133_RUN_FILES`] constants the real gate applies.
const SYNTH_R4133_RUN: &str = "\
pub fn run_case(engine: &Engine, req: &RunRequest) -> Result<CaseResult, EngineError> {
    let mut guard = CorpusGuard::new(&req.case_path);
    for attempt in 1..=RUN_ATTEMPTS {
        capture_aggregates(engine)?;
        capture_all_elements(engine)?;
        capture_discrete(engine)?;
        capture_reliability(engine)?;
        capture_all_properties(engine)?;
        capture_topology(engine)?;
        capture_inc_matrix(engine)?;
    }
    let autoadd_log = read_autoadd_log(engine, &req.case_path);
    let di = if req.di { capture_di(&guard, req)? } else { None };
    let run_files = if req.run_files { guard.created() } else { None };
    let run_file_contents = copy_selected_contents(
        guard.dir(),
        run_files.as_deref(),
        &req.run_file_contents,
        req.run_file_contents_dir.as_deref(),
    );
    let sweep_failed = guard.finish();
    let run_file_contents = run_file_contents.map_err(EngineError::Other)?;
    Ok(CaseResult { run_files, sweep_failed, run_file_contents })
}
";

/// Non-vacuity (§1.1(f)) for the capi half of the run-file rule: the gate above
/// passes on the real source, so this one shows each of its six clauses rejects
/// the corresponding mistake. No file on disk is mutated.
#[test]
fn the_run_file_gate_rejects_an_early_escaped_or_nested_classification() {
    let rel = "synthetic";
    let ok = SYNTH_CAPI_RUN;
    check_run_tail_order(ok, &CAPI_CALLS, &CAPI_RUN_FILES, rel).unwrap_or_else(|e| panic!("{e}"));

    // (2) classified before the last per-step captures.
    let early = move_line_after(ok, "guard.created()", "capture_all_elements(ckt, derived)");
    let err = check_run_tail_order(&early, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("classified mid-step");
    assert!(err.contains("runs BEFORE"), "{err}");
    assert!(err.contains("must follow every per-step read"), "{err}");

    // (3) classified before the AutoAddLog is read off disk.
    let before_log = move_line_after(ok, "guard.created()", "capture_inc_matrix(d, ckt)");
    let err = check_run_tail_order(&before_log, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("classified before the autoadd read");
    assert!(err.contains("autoadd_log"), "{err}");

    // (4) classified after the guard scope closed.
    let escaped = move_line_after(ok, "guard.created()", "sweep_failed = guard.sweep_failed");
    let err = check_run_tail_order(&escaped, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("classified outside the scope");
    assert!(err.contains("OUTSIDE the guard scope"), "{err}");

    // (5) classified from inside the retry loop's body (indentation is how both
    // languages state it; here the statement simply moves one level deeper).
    let nested = ok.replace(
        "        run_files = guard.created()",
        "            run_files = guard.created()",
    );
    let err = check_run_tail_order(&nested, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("classified inside the retry loop");
    assert!(err.contains("nested"), "{err}");

    // (6a) another command issued after the classification.
    let extra_cmd = insert_after(ok, "guard.created()", "        d.Text.Command = \"solve\"");
    let err = check_run_tail_order(&extra_cmd, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("a command followed the classification");
    assert!(err.contains("solve"), "{err}");

    // (6b) another READ issued after the classification.
    let extra_read = insert_after(ok, "guard.created()", "        capture_topology(ckt)");
    let err = check_run_tail_order(&extra_read, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("a capture followed the classification");
    assert!(err.contains("capture_topology"), "{err}");

    // (6c) any other statement in the tail — not a read, not a command, but not
    // the teardown either.
    let extra_stmt = insert_after(ok, "guard.created()", "        run_files.sort()");
    let err = check_run_tail_order(&extra_stmt, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("a statement followed the classification");
    assert!(err.contains("run_files.sort()"), "{err}");

    // (6d) the teardown's `clear` hoisted out of its `try:` block — the shape
    // D33(1) needs (the pinned 0.14.5 faults on that `clear` after an AutoAdd
    // solve) and F4c asked this rule to state. Same statements, wrong order.
    let unguarded = move_line_after(ok, "d.Text.Command", "teardown_error = None");
    let err = check_run_tail_order(&unguarded, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("the teardown clear left its try block");
    assert!(err.contains("SOLE statement of its `try` block"), "{err}");

    // (6e) past the `with` block, still inside `run_case`: the classification is
    // the last read of the RUN, not of the guard scope. A capture, a command, a
    // model read under any other name, and a read hidden in the reply.
    let after_scope = "sweep_failed = guard.sweep_failed";
    let read = insert_after(ok, after_scope, "    probe = capture_topology(ckt)");
    let err = check_run_tail_order(&read, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("a capture followed the guard scope");
    assert!(err.contains("`capture_topology` is called AFTER"), "{err}");
    let cmd = insert_after(ok, after_scope, "    d.Text.Command = \"export eventlog\"");
    let err = check_run_tail_order(&cmd, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("a command followed the guard scope");
    assert!(
        err.contains("to the end of `run_case`, are") && err.contains("export eventlog"),
        "{err}"
    );
    let stmt = insert_after(ok, after_scope, "    events = ckt.Solution.EventLog");
    let err = check_run_tail_order(&stmt, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("a model read followed the guard scope");
    assert!(err.contains("events = ckt.Solution.EventLog"), "{err}");
    // A read or a computed command folded onto the epilogue's own line.
    for folded in [
        "    sweep_failed = guard.sweep_failed or bool(ckt.Solution.EventLog)",
        "    sweep_failed = guard.sweep_failed; d.Text.Command = f\"export {'eventlog'}\"",
    ] {
        let err = check_run_tail_order(
            &ok.replace("    sweep_failed = guard.sweep_failed", folded),
            &CAPI_CALLS,
            &CAPI_RUN_FILES,
            rel,
        )
        .expect_err("a statement was folded onto the epilogue");
        assert!(err.contains("statement 0 after the guard scope"), "{err}");
    }
    let in_reply = ok.replace("return {}", "return {\"events\": ckt.Solution.EventLog}");
    let err = check_run_tail_order(&in_reply, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("a model read hid in the reply");
    assert!(
        err.contains("`\"events\": ckt.Solution.EventLog` is not a bound name"),
        "{err}"
    );

    // (1) a second classification.
    let twice = insert_after(ok, "guard.created()", "        again = guard.created()");
    let err = check_run_tail_order(&twice, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("classified twice");
    assert!(err.contains("appears 2 times"), "{err}");

    // the anchor itself: a rename must fail loudly, never vacuously pass.
    let renamed = ok.replace("guard.created()", "guard.classify()");
    let err = check_run_tail_order(&renamed, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("the anchor is gone");
    assert!(err.contains("could not find"), "{err}");
}

/// Non-vacuity on the REAL sources, not only on the synthetic shapes: the two
/// gates above pass on the transports as they stand, so this one corrupts each
/// transport's own text in memory — hoisting the classification to the top of
/// the guard scope, pushing it past the sweep, and appending a read after the
/// guard scope — and asserts the gate rejects each. Nothing on disk is touched.
#[test]
fn the_run_file_gates_have_teeth_on_the_real_transport_sources() {
    let rel = "tools/oracle/oracle_server.py";
    let src = read_source(rel);
    let hoisted = move_line_after(
        &src,
        "run_files = guard.created()",
        "with _CorpusGuard(case_path) as guard:",
    );
    let err = check_run_tail_order(&hoisted, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("classified before the first step");
    assert!(err.contains("runs BEFORE"), "{err}");
    let escaped = move_line_after(
        &src,
        "run_files = guard.created()",
        "sweep_failed = guard.sweep_failed",
    );
    let err = check_run_tail_order(&escaped, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("classified after the guard scope closed");
    assert!(err.contains("OUTSIDE the guard scope"), "{err}");

    // G1.10c: the same two claims for the demand-interval read, on the real capi
    // source — hoisted in front of the `autoadd_log` read it must follow, and
    // pushed past the guard scope whose sweep removes the tree it copies out.
    let hoisted_di = move_line_after(
        &src,
        "di = capture_di(guard",
        "with _CorpusGuard(case_path) as guard:",
    );
    let err = check_run_tail_order(&hoisted_di, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("the DI read was hoisted in front of the autoadd read");
    assert!(err.contains("runs BEFORE the WPG.5"), "{err}");
    let escaped_di = move_line_after(
        &src,
        "di = capture_di(guard",
        "sweep_failed = guard.sweep_failed",
    );
    let err = check_run_tail_order(&escaped_di, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("the DI read left the guard scope");
    assert!(err.contains("OUTSIDE the guard scope"), "{err}");

    // A read appended after the `with` block, before `run_case` returns.
    let read_after = insert_after(
        &src,
        "sweep_failed = guard.sweep_failed",
        "    events = capture_eventlog(d, ckt)",
    );
    let err = check_run_tail_order(&read_after, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("a read followed the guard scope");
    assert!(err.contains("`capture_eventlog` is called AFTER"), "{err}");

    let rel = "crates/dss-epri/src/capture.rs";
    let src = read_source(rel);
    let hoisted = move_line_after(
        &src,
        "let run_files = if req.run_files",
        "let mut guard = CorpusGuard::new(",
    );
    let err = check_run_tail_order(&hoisted, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("classified before the first step");
    assert!(err.contains("runs BEFORE"), "{err}");
    let escaped = move_line_after(
        &src,
        "let run_files = if req.run_files",
        "let sweep_failed = guard.finish();",
    );
    let err = check_run_tail_order(&escaped, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("classified after the sweep");
    assert!(err.contains("OUTSIDE the guard scope"), "{err}");

    // G1.10c on the real r4133 source: the DI read hoisted in front of the
    // `autoadd_log` read, and the same read issued twice.
    let hoisted_di = move_line_after(
        &src,
        "let di = if req.di",
        "let mut guard = CorpusGuard::new(",
    );
    let err = check_run_tail_order(&hoisted_di, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("the DI read was hoisted in front of the autoadd read");
    assert!(err.contains("runs BEFORE the WPG.5"), "{err}");
    let twice_di = insert_after(
        &src,
        "let di = if req.di",
        "    let di = if req.di { capture_di(&guard, req)? } else { None };",
    );
    let err = check_run_tail_order(&twice_di, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("the DI tree was copied out twice");
    assert!(err.contains("appears 2 times"), "{err}");

    // A read appended after `guard.finish()`, before `run_case` returns.
    let read_after = insert_after(
        &src,
        "let sweep_failed = guard.finish();",
        "    let events = engine.eventlog();",
    );
    let err = check_run_tail_order(&read_after, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("a read followed the sweep");
    assert!(err.contains("statement 1 after the guard scope"), "{err}");
}

/// Non-vacuity for the G1.10c half of the rule on the capi transport: the DI
/// read's slot — after `autoadd_log`, before the classification and before the
/// D32(2)(a) teardown — is asserted, so each way of losing it must be rejected.
///
/// Four drives, each moving the ONE statement the real transport runs: past the
/// classification, past the teardown `clear` (the measured hazard — that `clear`
/// flushes the in-flight demand-interval cycle over the file), out of the guard
/// scope entirely, and duplicated. No file on disk is mutated.
#[test]
fn the_run_tail_gate_rejects_a_misplaced_di_read() {
    let rel = "synthetic";
    let ok = SYNTH_CAPI_RUN;
    check_run_tail_order(ok, &CAPI_CALLS, &CAPI_RUN_FILES, rel).unwrap_or_else(|e| panic!("{e}"));

    // (1) read after the created-file classification, which is the last read.
    let after_created = move_line_after(ok, "di = capture_di(guard", "run_files = guard.created()");
    let err = check_run_tail_order(&after_created, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("the DI read followed the classification");
    assert!(
        err.contains("runs AFTER the run-file classification"),
        "{err}"
    );

    // (2) read after the teardown `clear` — the measured hazard.
    let after_teardown = insert_after(
        &drop_line(ok, "di = capture_di(guard"),
        "log(f\"teardown clear raised",
        "        di = capture_di(guard, di_dir, case_path) if want_di else None",
    );
    let err = check_run_tail_order(&after_teardown, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("the DI read followed the teardown");
    assert!(err.contains("D32(2)(a) teardown"), "{err}");

    // (3) read after the guard scope closed — the tree is already swept.
    let escaped = move_line_after(
        ok,
        "di = capture_di(guard",
        "sweep_failed = guard.sweep_failed",
    );
    let err = check_run_tail_order(&escaped, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("the DI read left the guard scope");
    assert!(err.contains("OUTSIDE the guard scope"), "{err}");

    // (4) read twice: one tree reported, another in the sidecar.
    let twice = insert_after(
        ok,
        "di = capture_di(guard",
        "        di = capture_di(guard, di_dir, case_path) if want_di else None",
    );
    let err = check_run_tail_order(&twice, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("the DI tree was copied out twice");
    assert!(err.contains("appears 2 times"), "{err}");

    // the anchor itself: a rename must fail loudly, never vacuously pass.
    let renamed = ok.replace("di = capture_di(guard", "di = capture_di_tree(guard");
    let err = check_run_tail_order(&renamed, &CAPI_CALLS, &CAPI_RUN_FILES, rel)
        .expect_err("the DI anchor is gone");
    assert!(err.contains("could not find"), "{err}");
}

/// The r4133 half of the same four drives. This transport has no teardown
/// (r4133 closes its own trace file as it writes the header), so its fourth
/// slot violation is the read nested in the retry loop, which recompiles
/// in-process and would copy out the tree of an attempt that was thrown away.
#[test]
fn the_r4133_run_tail_gate_rejects_a_misplaced_di_read() {
    let rel = "synthetic";
    let ok = SYNTH_R4133_RUN;
    check_run_tail_order(ok, &R4133_CALLS, &R4133_RUN_FILES, rel).unwrap_or_else(|e| panic!("{e}"));

    // (1) read after the created-file classification.
    let after_created =
        move_line_after(ok, "let di = if req.di", "let run_files = if req.run_files");
    let err = check_run_tail_order(&after_created, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("the DI read followed the classification");
    assert!(
        err.contains("runs AFTER the run-file classification"),
        "{err}"
    );

    // (2) read after the sweep — on this transport the sweep IS the end of the
    // guard's life, so the tree is gone.
    let after_sweep = move_line_after(
        ok,
        "let di = if req.di",
        "let sweep_failed = guard.finish();",
    );
    let err = check_run_tail_order(&after_sweep, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("the DI read followed the sweep");
    assert!(err.contains("OUTSIDE the guard scope"), "{err}");

    // (3) read from inside the retry loop's body.
    let nested = ok.replace("    let di = if req.di", "        let di = if req.di");
    let err = check_run_tail_order(&nested, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("the DI read is nested in the retry loop");
    assert!(err.contains("nested"), "{err}");

    // (4) read twice.
    let twice = insert_after(
        ok,
        "let di = if req.di",
        "    let di = if req.di { capture_di(&guard, req)? } else { None };",
    );
    let err = check_run_tail_order(&twice, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("the DI tree was copied out twice");
    assert!(err.contains("appears 2 times"), "{err}");
}

/// The r4133 half: the same rule against the Rust spelling of the scope, where
/// the guard is consumed by an explicit `finish()` and the tail carries the
/// G1.10b contents copy and nothing else (that transport has no teardown —
/// r4133 closes its own trace file as it writes the header).
#[test]
fn the_run_file_gate_rejects_a_classification_after_the_sweep() {
    let rel = "synthetic";
    let ok = SYNTH_R4133_RUN;
    check_run_tail_order(ok, &R4133_CALLS, &R4133_RUN_FILES, rel).unwrap_or_else(|e| panic!("{e}"));

    let swept_first = move_line_after(ok, "guard.created()", "guard.finish()");
    let err = check_run_tail_order(&swept_first, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("classified after the sweep");
    assert!(err.contains("OUTSIDE the guard scope"), "{err}");

    let teardown = insert_after(ok, "guard.created()", "    engine.exec_wait(\"clear\")?;");
    let err = check_run_tail_order(&teardown, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("this transport has no teardown");
    assert!(err.contains("clear"), "{err}");

    // Past `guard.finish()`, still inside `run_case`: a capture, a command, a
    // model read under any other name, and a read hidden in the reply.
    let after_scope = "guard.finish()";
    let read = insert_after(
        ok,
        after_scope,
        "    let probe = capture_topology(engine)?;",
    );
    let err = check_run_tail_order(&read, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("a capture followed the sweep");
    assert!(err.contains("`capture_topology` is called AFTER"), "{err}");
    let cmd = insert_after(
        ok,
        after_scope,
        "    engine.exec_wait(\"export eventlog\")?;",
    );
    let err = check_run_tail_order(&cmd, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("a command followed the sweep");
    assert!(
        err.contains("to the end of `run_case`, are") && err.contains("export eventlog"),
        "{err}"
    );
    let stmt = insert_after(ok, after_scope, "    let events = engine.eventlog();");
    let err = check_run_tail_order(&stmt, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("a model read followed the sweep");
    assert!(err.contains("statement 1 after the guard scope"), "{err}");
    let folded = ok.replace(
        "let sweep_failed = guard.finish();",
        "let sweep_failed = guard.finish(); let events = engine.eventlog();",
    );
    let err = check_run_tail_order(&folded, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("a model read was folded onto the sweep");
    assert!(err.contains("statement 0 after the guard scope"), "{err}");
    let in_reply = ok.replace(
        "run_file_contents })",
        "run_file_contents, events: engine.eventlog() })",
    );
    let err = check_run_tail_order(&in_reply, &R4133_CALLS, &R4133_RUN_FILES, rel)
        .expect_err("a model read hid in the reply");
    assert!(
        err.contains("`events: engine.eventlog()` is not a bound name"),
        "{err}"
    );
}
