//! Case runner: the Rust engine runs ONCE per case ([`run_rust_capture`]), then
//! its per-step state is compared against a channel's [`CaseResult`]
//! ([`compare_capture`]). The comparator call order and every `harness`
//! comparator are byte-identical to the pre-Phase-B `run_and_compare` — the only
//! structural change is that the oracle `CaseResult` is now fetched by the
//! scheduler (via a persistent [`Channel`]) and passed in, instead of being
//! spawned inside.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use dss_core::exec::Dss;
use serde_json::json;

use crate::engines::{CaseResult, Channel, Oracle};
use crate::harness::{
    self, ExportPolicy, RelCalcOutcome, RowPolicy, Tolerances, capture_guard,
    compare_all_properties, compare_ctrlqueue, compare_discrete, compare_element_channels,
    compare_element_derived, compare_element_extras, compare_eventlog, compare_export,
    compare_fingerprint, compare_injection, compare_meter, compare_monitor, compare_pd_elements,
    compare_probe, compare_reliability, compare_system_y, compare_variables, compare_yprim, lane,
    tol_for,
};
use crate::manifest::{EngineChannel, SolvableCase};

// ---------------------------------------------------------------------------
// Corpus guard (unchanged; keeps the vendored corpus pristine across both
// engines' report/trace writes). RAII: created before the runs, restores on drop.
// ---------------------------------------------------------------------------

/// Buffer small files up to this size for overwrite-restore. Mirrors the oracle
/// server's `_RESTORE_MAX`.
const RESTORE_MAX: u64 = 2 * 1024 * 1024;

/// One directory's pristine snapshot, shared by every guard currently active on
/// that directory.
struct Snapshot {
    names: BTreeSet<String>,
    buf: BTreeMap<String, Vec<u8>>,
    snapshot_ok: bool,
}

/// Live guards per case directory: `dir -> (pristine snapshot, refcount)`.
///
/// The corpus puts many decks in one folder (`Test/AutoTrans`,
/// `StorageControllerTechNote/Support`, …), and the scheduler runs cases in
/// parallel, so two guards on the *same* directory overlap. With a per-guard
/// snapshot that silently defeats the sweep: guard A snapshots a clean dir, A's
/// engine writes `X`, guard B then snapshots and sees `X` as pre-existing, A
/// drops and sweeps `X`, B's engine rewrites `X`, and B's drop keeps it — the
/// vendored corpus ends the run polluted (reproduced on two full
/// `cargo test --workspace` runs, a different file set each time).
///
/// Sharing one snapshot per directory and sweeping only when the **last** guard
/// leaves removes the interleaving: the names set is always the pristine one,
/// and exactly one sweep runs, after every writer is done.
type DirRegistry = Mutex<HashMap<PathBuf, (Arc<Snapshot>, usize)>>;

fn dir_registry() -> &'static DirRegistry {
    static REG: OnceLock<DirRegistry> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) struct CorpusGuard {
    dir: PathBuf,
    snap: Arc<Snapshot>,
}

impl CorpusGuard {
    fn snapshot(
        dir: &Path,
        prefix: &str,
        names: &mut BTreeSet<String>,
        buf: &mut BTreeMap<String, Vec<u8>>,
    ) -> bool {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return false;
        };
        let mut ok = true;
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            names.insert(rel.clone());
            if is_dir {
                ok &= Self::snapshot(&entry.path(), &rel, names, buf);
                continue;
            }
            let small = entry
                .metadata()
                .map(|m| m.is_file() && m.len() <= RESTORE_MAX)
                .unwrap_or(false);
            if small && let Ok(data) = std::fs::read(entry.path()) {
                buf.insert(rel, data);
            }
        }
        ok
    }

    pub(crate) fn new(case_path: &str) -> Self {
        let dir = Path::new(case_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let mut reg = dir_registry().lock().unwrap_or_else(|e| e.into_inner());
        let snap = match reg.get_mut(&dir) {
            // Another case in this folder is already running: reuse its pristine
            // snapshot rather than photographing that run's output as "vendored".
            Some((snap, refs)) => {
                *refs += 1;
                Arc::clone(snap)
            }
            None => {
                let mut names = BTreeSet::new();
                let mut buf = BTreeMap::new();
                let snapshot_ok = Self::snapshot(&dir, "", &mut names, &mut buf);
                let snap = Arc::new(Snapshot {
                    names,
                    buf,
                    snapshot_ok,
                });
                reg.insert(dir.clone(), (Arc::clone(&snap), 1));
                snap
            }
        };
        drop(reg);
        Self { dir, snap }
    }

    fn sweep_created(&self, dir: &Path, prefix: &str) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if self.snap.names.contains(&rel) {
                if is_dir {
                    self.sweep_created(&entry.path(), &rel);
                }
                continue;
            }
            if is_dir {
                let _ = std::fs::remove_dir_all(entry.path());
            } else {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

impl Drop for CorpusGuard {
    fn drop(&mut self) {
        // Only the last guard on this directory restores it — an earlier sweep
        // would race a sibling case that is still writing into the same folder.
        {
            let mut reg = dir_registry().lock().unwrap_or_else(|e| e.into_inner());
            match reg.get_mut(&self.dir) {
                Some((_, refs)) if *refs > 1 => {
                    *refs -= 1;
                    return;
                }
                _ => {
                    reg.remove(&self.dir);
                }
            }
        }
        if !self.snap.snapshot_ok {
            return;
        }
        self.sweep_created(&self.dir.clone(), "");
        for (name, data) in &self.snap.buf {
            let p = self.dir.join(name);
            match std::fs::read(&p) {
                Ok(cur) if cur == *data => {}
                _ => {
                    let _ = std::fs::write(&p, data);
                }
            }
        }
    }
}

#[test]
fn corpus_guard_restores_case_dir_recursively() {
    let root = std::env::temp_dir().join(format!("dss_guard_test_{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    let sub = root.join("support");
    std::fs::create_dir_all(&sub).unwrap();
    let case = root.join("case.dss");
    std::fs::write(&case, b"! fixture master").unwrap();
    let fixture = sub.join("fixture.txt");
    std::fs::write(&fixture, b"vendored bytes").unwrap();
    {
        let _guard = CorpusGuard::new(&case.to_string_lossy());
        std::fs::write(root.join("run_created.csv"), b"pollution").unwrap();
        std::fs::write(sub.join("run_created_inner.csv"), b"pollution").unwrap();
        let di = root.join("ckt_di").join("DI_yr_1");
        std::fs::create_dir_all(&di).unwrap();
        std::fs::write(di.join("x.csv"), b"pollution").unwrap();
        std::fs::write(&fixture, b"overwritten by the run").unwrap();
    }
    assert!(case.is_file(), "vendored master must survive");
    assert_eq!(
        std::fs::read(&fixture).unwrap(),
        b"vendored bytes",
        "overwritten pre-existing fixture must be restored"
    );
    assert!(!root.join("run_created.csv").exists());
    assert!(
        !sub.join("run_created_inner.csv").exists(),
        "run-created file inside a pre-existing subdir must be swept (recursion)"
    );
    assert!(
        !root.join("ckt_di").exists(),
        "run-created dir tree removed"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// Two cases in the **same** deck folder run concurrently (the corpus puts many
/// decks in one directory), so their guards overlap. The interleaving that used
/// to leak: A snapshots clean → A writes → B snapshots (sees A's output) → A
/// drops and sweeps → B writes again → B drops and *keeps* it. With the shared
/// per-directory snapshot the second guard reuses the pristine names and the
/// sweep runs once, when the last guard leaves.
#[test]
fn corpus_guard_overlapping_guards_still_sweep() {
    let root = std::env::temp_dir().join(format!("dss_guard_overlap_{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(&root).unwrap();
    let case_a = root.join("a.dss");
    let case_b = root.join("b.dss");
    std::fs::write(&case_a, b"! deck a").unwrap();
    std::fs::write(&case_b, b"! deck b").unwrap();

    let out = root.join("a_EXP_VOLTAGES.csv");
    {
        let ga = CorpusGuard::new(&case_a.to_string_lossy());
        std::fs::write(&out, b"run a output").unwrap();
        // B starts while A's output is on disk — must NOT adopt it as vendored.
        let gb = CorpusGuard::new(&case_b.to_string_lossy());
        drop(ga);
        assert!(
            out.exists(),
            "the first guard must not sweep while a sibling case is still running"
        );
        std::fs::write(&out, b"run b output").unwrap();
        drop(gb);
    }
    assert!(!out.exists(), "the last guard out must sweep the leftovers");
    assert!(case_a.is_file() && case_b.is_file(), "decks must survive");
    std::fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------------
// Warnings reconciliation (unchanged).
// ---------------------------------------------------------------------------

fn assert_expected_warnings(dss: &Dss, expect: &[String], ctx: &str) {
    let errors = dss.errors();
    if expect.is_empty() {
        assert!(
            errors.is_empty(),
            "{ctx}: unexpected Rust engine errors: {errors:?}"
        );
        return;
    }
    let unexpected: Vec<_> = errors
        .iter()
        .filter(|e| !expect.iter().any(|w| e.contains(w.as_str())))
        .collect();
    assert!(
        unexpected.is_empty(),
        "{ctx}: Rust engine errors not covered by expect_warnings: {unexpected:?}"
    );
    for w in expect {
        assert!(
            errors.iter().any(|e| e.contains(w.as_str())),
            "{ctx}: expected warning {w:?} never fired (actual: {errors:?})"
        );
    }
}

// ---------------------------------------------------------------------------
// run_rust_capture + compare_capture (the split of the old run_and_compare).
// ---------------------------------------------------------------------------

/// The lowercase object names of one class among the snapshotted circuit
/// elements (`snapshot_elements` walks `Circuit.ckt_elements`, which holds the
/// control elements too).
fn class_member_names(snaps: &[dss_core::exec::ElementSnapshot], class: &str) -> BTreeSet<String> {
    snaps
        .iter()
        .filter_map(|s| s.name.split_once('.'))
        .filter(|(cls, _)| cls.eq_ignore_ascii_case(class))
        .map(|(_, name)| name.to_lowercase())
        .collect()
}

/// This channel's tag for the [`capture_guard`] refusal messages
/// (`capi_v0145` / `r4133`).
///
/// It reuses the single channel→tag mapping the tree already has
/// ([`harness::PropsChannel::tag`]) instead of adding a second copy:
/// `EngineChannel` is `pub(crate)` to this one test binary while `harness/`
/// compiles into ~20 others, so the guard takes a `&str`
/// ([`EngineChannel::props_channel`] records that channel-threading trap).
fn channel_tag(channel: EngineChannel) -> &'static str {
    channel.props_channel().tag()
}

/// Compile + post + reconcile warnings; return the driven [`Dss`] (not yet
/// solved) and the baseline error count. The Rust engine runs ONCE per case;
/// [`compare_capture`] then advances + compares it step by step.
pub(crate) fn run_rust_capture(label: &str, case_path: &str, c: &SolvableCase) -> (Dss, usize) {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{case_path}\""));
    for cmd in &c.post {
        dss.command(cmd);
    }
    assert_expected_warnings(&dss, &c.expect_warnings, &format!("{label}: after compile"));
    let baseline_errors = dss.errors().len();
    (dss, baseline_errors)
}

/// Compare the (once-run) Rust engine against ONE channel's `CaseResult`, per
/// step, in the fixed comparator order. Byte-identical to the pre-Phase-B loop
/// EXCEPT where a per-case-per-channel `ledger` scope partitions a field: the
/// untouched `harness` comparator runs on the unscoped remainder while the
/// ledger's envelope/exact-pair assert covers the scoped part and records the hit
/// (§1.3). With `ledger = None` or an empty view every field takes the original
/// path unchanged.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compare_capture(
    dss: &mut Dss,
    baseline_errors: usize,
    oc: &CaseResult,
    label: &str,
    case_path: &str,
    c: &SolvableCase,
    tol: &Tolerances,
    channel: EngineChannel,
    ledger: Option<&crate::ledger::LedgerView>,
) {
    let n_steps = c.n_steps;
    let star = c.selected_elements == ["*"];
    // A view with no applicable entries behaves exactly like `None` (fast path).
    let ledger = ledger.filter(|v| !v.is_empty());

    // GOLDEN_REBASE G1.6(i) protocol: `RelCalc` is NOT idempotent (a nested or
    // adjoining zone re-reads the inner meter's `Bus.TotalMiles` as its own
    // downstream mileage on the second run), so the gate runs it exactly once
    // per case, on the LAST step, on all three engines. Assert the channel did
    // the same before comparing anything: exactly one checkpoint may carry the
    // payload and it must be the last one, and NO checkpoint may carry it when
    // the flag is off. `require_capture_opt` below then covers the per-case
    // "the flag is on but this channel sent nothing" hole.
    {
        let carriers: Vec<usize> = oc
            .checkpoints
            .iter()
            .enumerate()
            .filter(|(_, cp)| cp.reliability.is_some())
            .map(|(i, _)| i)
            .collect();
        let expected: Vec<usize> = if c.compare_reliability {
            vec![n_steps - 1]
        } else {
            Vec::new()
        };
        assert_eq!(
            carriers, expected,
            "{label} [{channel:?}]: reliability payload on checkpoint(s) {carriers:?}, \
             expected {expected:?} (compare_reliability = {}). The transport must drive \
             `RelCalc` once, after the LAST solve, and attach the payload to that \
             checkpoint only.",
            c.compare_reliability,
        );
    }

    for (i, cp) in oc.checkpoints.iter().enumerate() {
        dss.command("solve");
        assert_eq!(
            dss.errors().len(),
            baseline_errors,
            "{label} step {i}: new Rust engine errors: {:?}",
            &dss.errors()[baseline_errors.min(dss.errors().len())..]
        );
        let rust_global_result = dss.result().to_string();
        let ctx = format!("{label} step {i}");

        // GOLDEN_REBASE G1.6(i): drive the executive `RelCalc` HERE - after the
        // per-step error assert and the `Text.Result`/`dss.result()` read (the
        // command overwrites the result), before every comparator of this step,
        // and only on the last one. Both transports drive it at exactly this
        // point (`oracle_server.py::run_case`, `dss-epri::capture::run_case`), so
        // the reliability payload AND the fields the sweep writes into surfaces
        // that are already gated - `PDElements.{AccumulatedL,Lambda,TotalMiles,
        // SectionID}`, the `Bus` reliability columns, EnergyMeter properties
        // #19-23 - are read post-calc on all three engines.
        //
        // The abort (errno 52902, a zone with no OCP device) is a compared
        // observable, not a failure: the port pushes its message onto
        // `Dss::errors` (`exec/solve.rs::do_relcalc_cmd` ->
        // `solution/meters/reliability.rs:53-57`, one per failing meter) and
        // `compare_reliability` asserts boolean+message symmetry against the
        // channel. Reading the new lines here is also what keeps the next step's
        // `baseline_errors` assert meaningful - and there is no next step.
        let mut rust_relcalc = RelCalcOutcome::default();
        if c.compare_reliability && i + 1 == n_steps {
            let before = dss.errors().len();
            dss.command("RelCalc");
            let new: Vec<String> = dss.errors()[before..]
                .iter()
                .map(|e| e.message.clone())
                .collect();
            rust_relcalc = RelCalcOutcome::from_new_errors(&new);
        }

        {
            let ckt = dss.circuit().expect("circuit exists");
            assert!(
                cp.converged,
                "{ctx}: oracle did not converge (persisted across the oracle's in-process retries)"
            );
            assert!(ckt.is_solved, "{ctx}: Rust did not converge");
            assert!(
                (ckt.solution.dbl_hour - cp.dbl_hour).abs() < 1e-9,
                "{ctx}: dblHour {} vs {}",
                ckt.solution.dbl_hour,
                cp.dbl_hour
            );
            // Iteration policy (§4 Phase C): the pinned capi_v0145 channel is an
            // exact 1:1 contract; the r4133 channel (a different engine line)
            // allows the port to converge in FEWER iterations — never more. A
            // ledger `iterations` scope overrides both (exact pair, or an explicit
            // rust_le_oracle where a target-rev delta needs pinning).
            //
            // Stage F (Part IV.2 drift model): both non-ledgered shapes go
            // through `harness::lane`, which keeps them exactly as above in the
            // parity lane and grants the default lane its documented
            // ±`ITER_SLACK` band. Ledger-scoped pins stay exact in BOTH lanes:
            // they are hit-tracked (fail-on-stale), so a default-lane flip that
            // moves one must be re-triaged in the ledger, not silently absorbed.
            let iters_ledgered = ledger.is_some_and(|v| {
                v.iterations_handled(i, ckt.solution.iteration, cp.iterations, &ctx)
            });
            if !iters_ledgered {
                if channel.iterations_exact() {
                    harness::lane::compare_iterations(ckt.solution.iteration, cp.iterations, &ctx);
                } else {
                    harness::lane::compare_iterations_le(
                        ckt.solution.iteration,
                        cp.iterations,
                        &ctx,
                    );
                }
            }
            let names: Vec<String> = (1..=ckt.num_nodes).map(|j| ckt.node_name(j)).collect();
            assert_eq!(names, oc.node_order, "{ctx}: node order differs");

            let mut actual = Vec::with_capacity(2 * ckt.num_nodes);
            for j in 1..=ckt.num_nodes {
                actual.push(ckt.solution.node_v[j].re);
                actual.push(ckt.solution.node_v[j].im);
            }
            let mut expected = Vec::with_capacity(actual.len());
            for (re, im) in cp.v_re.iter().zip(&cp.v_im) {
                expected.push(*re);
                expected.push(*im);
            }
            // A ledger voltage scope partitions the nodes: the scoped nodes are
            // envelope-checked inside `voltage_keep_mask` (hit recorded), the
            // unscoped remainder still meets the tier floor here.
            match ledger
                .and_then(|v| v.voltage_keep_mask(i, &oc.node_order, &actual, &expected, tol, &ctx))
            {
                Some(keep) => {
                    let mut ra = Vec::with_capacity(actual.len());
                    let mut re = Vec::with_capacity(expected.len());
                    for (ni, k) in keep.iter().enumerate() {
                        if *k {
                            ra.push(actual[2 * ni]);
                            ra.push(actual[2 * ni + 1]);
                            re.push(expected[2 * ni]);
                            re.push(expected[2 * ni + 1]);
                        }
                    }
                    harness::assert_complex_close(&ra, &re, tol.v_rel, tol.v_abs, &ctx);
                }
                None => {
                    harness::assert_complex_close(&actual, &expected, tol.v_rel, tol.v_abs, &ctx);
                }
            }
        }

        // Whole-artifact ledger exclusions (`GOLDEN_REBASE_PLAN.md` G2.5): the
        // assembled Y, its fingerprint, an element's YPrim and a meter's
        // register block have no partition and no envelope, so an entry that
        // names one drops it for this (case, channel) — hit-accounted, so it
        // still fails the gate the day it stops matching. The oracle payload is
        // still demanded (a missing Y is a protocol failure, not a divergence).
        let excluded =
            |field: &str, name: Option<&str>| ledger.is_some_and(|v| v.excluded(field, name, i));
        if let Some(y) = &cp.y {
            if !excluded("y", None) {
                compare_system_y(dss, y, &oc.node_order, tol, &ctx);
            }
        } else {
            panic!("{ctx}: oracle returned no full Y (the live gate requires it)");
        }
        if !excluded("y_fingerprint", None) {
            compare_fingerprint(dss, &cp.y_fingerprint, tol, &ctx);
        }

        let snaps = dss.snapshot_elements();

        if star {
            let yprim_names: BTreeSet<String> =
                cp.yprims.iter().map(|y| y.name.to_lowercase()).collect();
            let all_names: BTreeSet<String> = snaps
                .iter()
                .filter(|s| dss.element_yprim(&s.name).is_some())
                .map(|s| s.name.to_lowercase())
                .collect();
            assert_eq!(
                yprim_names, all_names,
                "{ctx}: selected_elements=[\"*\"] must yield a YPrim block for every \
                 YPrim-bearing element"
            );
        } else {
            assert_eq!(
                cp.yprims.len(),
                c.selected_elements.len(),
                "{ctx}: oracle returned {} YPrim block(s) for {} selected element(s)",
                cp.yprims.len(),
                c.selected_elements.len()
            );
        }
        for yp in &cp.yprims {
            if !excluded("yprim", Some(&yp.name)) {
                compare_yprim(dss, yp, tol, &ctx);
            }
        }
        // A ledger `injection` scope envelope-checks the whole RHS here; an
        // `exclusion` drops it (the RHS has no sub-selector, so both forms are
        // all-or-nothing anyway).
        if !excluded("injection", None)
            && !ledger.is_some_and(|v| v.injection_handled(i, dss, &cp.injection, tol, &ctx))
        {
            compare_injection(dss, &cp.injection, tol, &ctx);
        }

        let rust_names: BTreeSet<String> = snaps.iter().map(|s| s.name.to_lowercase()).collect();
        let oracle_names: BTreeSet<String> =
            cp.elements.iter().map(|e| e.name.to_lowercase()).collect();
        assert_eq!(
            rust_names,
            oracle_names,
            "{ctx}: element name sets differ (Rust∖oracle={:?}, oracle∖Rust={:?})",
            rust_names.difference(&oracle_names).collect::<Vec<_>>(),
            oracle_names.difference(&rust_names).collect::<Vec<_>>(),
        );
        // A ledger element scope neutralizes only its pinned sub-channels: it
        // rewrites those to the Rust values (after re-asserting them inside their
        // envelope) so the standard `compare_element` treats them as equal and
        // still tier-checks the unscoped remainder (clause (b)). Elements with no
        // scope are compared against the untouched oracle cap.
        let el_rewrites = ledger
            .map(|v| v.element_rewrites(i, &snaps, &cp.elements, tol, &ctx))
            .unwrap_or_default();
        // The lane policy drops the two `S = V·conj(I)` sub-channels on the
        // `newton*` decks in **both** lanes since `GOLDEN_REBASE_PLAN.md` G2.3
        // (no oracle channel reports them at the converged `NodeV`; pinned by
        // its own expected-value test) — every other case gets
        // `ElemChannels::ALL`. See `harness::lane`.
        let channels = lane::elem_channels_for(label);
        for ec in &cp.elements {
            match el_rewrites.get(&ec.name.to_lowercase()) {
                Some(rw) => compare_element_channels(&snaps, rw, tol, &ctx, channels),
                None => compare_element_channels(&snaps, ec, tol, &ctx, channels),
            }
        }

        // `GOLDEN_REBASE_PLAN.md` G1.9 — the five `Circuit` aggregates and the
        // ten `Solution` scalars. The surface is UNFLAGGED and universal (no
        // `G1_SURFACE_FLAGS` row, no rigor token, no forced population), so the
        // capture is demanded on every live case of every gating channel; the
        // `capture_guard` rail is not reused because its message is
        // manifest-flag shaped and there is no flag to name here.
        let agg = cp.aggregates.as_ref().unwrap_or_else(|| {
            panic!(
                "{ctx}: the `{}` capture carries no `aggregates` member. G1.9 is an \
                 unflagged, universal surface — every live case must compare it, so \
                 an absent capture FAILS the case instead of silently comparing \
                 nothing (GOLDEN_REBASE_PLAN.md §1.1(f)).",
                channel_tag(channel)
            )
        });
        let scalars = cp.solution_scalars.as_ref().unwrap_or_else(|| {
            panic!(
                "{ctx}: the `{}` capture carries no `solution_scalars` member \
                 (unflagged universal surface — see the `aggregates` refusal above).",
                channel_tag(channel)
            )
        });
        harness::aggregates::compare_aggregates(
            dss,
            agg,
            &snaps,
            &cp.elements,
            &el_rewrites,
            tol,
            channels,
            &ctx,
        );
        harness::aggregates::compare_solution_scalars(
            dss,
            scalars,
            cp.iterations,
            channel.iterations_exact(),
            &ctx,
        );
        // WP-G1 G1.3a: the per-element **derived** channels — `Enabled` plus
        // the polar renderings `CurrentsMagAng` / `VoltagesMagAng` / `Residuals`
        // (r4133 `DDLL/DCktElement.pas:1058`/`:1082`/`:827`). Compared on the
        // same caps the loop above just used, so a ledger `element` scope that
        // pins one of the new sub-channels neutralizes it here too, and an
        // unscoped element is compared against the untouched oracle cap.
        //
        // The guard is the flag's own non-vacuity rail: under `derived` BOTH
        // transports emit `enabled` for every element (present even on the
        // disabled ones, whose polar channels the capture must skip — r4133
        // `CktElementV(19)` dereferences a nil `NodeRef` there), so a channel
        // that ignored the request answers with zero `enabled` fields and the
        // case fails instead of comparing nothing.
        if c.compare_derived {
            capture_guard::require_capture(
                "compare_derived",
                channel_tag(channel),
                cp.elements.iter().filter(|e| e.enabled.is_some()).count(),
                &ctx,
            );
            for ec in &cp.elements {
                match el_rewrites.get(&ec.name.to_lowercase()) {
                    Some(rw) => compare_element_derived(&snaps, rw, tol, &ctx, channels),
                    None => compare_element_derived(&snaps, ec, tol, &ctx, channels),
                }
            }
        }

        // WP-G1 G1.3d(i): the per-element **discrete index/name extras** —
        // `NumTerminals` / `NumConductors` / `NumPhases` (r4133
        // `DDLL/DCktElement.pas:139`/`:144`/`:149`), `EnergyMeter` (`:442`) and
        // `NodeOrder` (`:1032`). Everything here is discrete and compared
        // exactly: no tolerance, no `ElemChannels` selector, no ledger
        // sub-channel.
        //
        // Fed from the same `el_rewrites`-or-raw caps as the two loops above so
        // the block keeps their shape, which costs nothing and hides nothing: a
        // ledger rewrite is a full `clone_element_cap` (`ledger.rs:1695-1697`,
        // `ec.clone()`) with only its six named value channels overwritten
        // (`rewrite_element_selected`, `:1702`), so the five extras fields it
        // hands back are always the untouched oracle ones.
        //
        // The guard is the flag's own non-vacuity rail: under `element_extras`
        // BOTH transports emit the four scalars for every element, so a channel
        // that ignored the request answers with zero `n_terms` fields and the
        // case fails instead of comparing nothing.
        if c.compare_element_extras {
            capture_guard::require_capture(
                "compare_element_extras",
                channel_tag(channel),
                cp.elements.iter().filter(|e| e.n_terms.is_some()).count(),
                &ctx,
            );
            // The channel travels with the capture for one reason only: the
            // "no meter" sentinel is spelled per channel and is folded per
            // channel (`harness::oracle_meter_name`, G1.3d(i) audit settlement).
            let ch = channel.props_channel();
            for ec in &cp.elements {
                match el_rewrites.get(&ec.name.to_lowercase()) {
                    Some(rw) => compare_element_extras(&snaps, rw, ch, &ctx),
                    None => compare_element_extras(&snaps, ec, ch, &ctx),
                }
            }
        }

        compare_discrete(dss, &cp.transformers, &cp.regcontrols, &cp.capacitors, &ctx);

        // A ledger monitor scope neutralizes only its pinned channel_idx (rewrites
        // it to the Rust samples after the envelope check); the header, sample
        // count, and every other channel still go through the standard comparator.
        for m in &cp.monitors {
            if excluded("monitor", Some(&m.name)) {
                continue;
            }
            match ledger.and_then(|v| v.monitor_rewrite(dss, m, tol, &ctx)) {
                Some(rw) => compare_monitor(dss, &rw, tol, &ctx),
                None => compare_monitor(dss, m, tol, &ctx),
            }
        }
        for m in &cp.meters {
            if !excluded("meter", Some(&m.name)) {
                compare_meter(dss, m, tol, &ctx);
            }
        }

        // The reliability surface (GOLDEN_REBASE G1.6(i)), immediately after the
        // register/zone compare so the two meter surfaces stay adjacent - the
        // same slot both transports read it in. Only the last checkpoint carries
        // it (asserted above); `require_capture_opt` turns "flag on, channel sent
        // nothing" into a case failure, and a circuit with no enabled meter is a
        // legitimate `Some` with an empty `meters` list.
        //
        // Per-value ledger exclusions are keyed on the lowercased
        // `<meter>:<field>` pair (`em:saidi`) plus the bare `totals` for the
        // circuit-level array - the `variables` shape one level up. The counts
        // (walk length, section count, array lengths) stay unconditional.
        if c.compare_reliability && i + 1 == n_steps {
            let rel = capture_guard::require_capture_opt(
                "compare_reliability",
                channel_tag(channel),
                cp.reliability.as_ref(),
                &ctx,
            );
            compare_reliability(
                dss,
                rel,
                &rust_relcalc,
                channel.props_channel(),
                &ctx,
                tol,
                &|key: &str| excluded("reliability", Some(key)),
            );
        }

        assert_eq!(
            cp.probes.len(),
            c.probes.iter().map(|p| p.props.len()).sum::<usize>(),
            "{ctx}: oracle probe count differs from the manifest spec"
        );
        for p in &cp.probes {
            let key = format!("{}.{}", p.element.to_lowercase(), p.prop.to_lowercase());
            if !excluded("probe", Some(&key))
                && !ledger.is_some_and(|v| v.probe_handled(dss, p, tol, &ctx))
            {
                compare_probe(dss, p, tol, &ctx);
            }
        }
        assert_eq!(
            cp.variables.len(),
            c.compare_variables.len(),
            "{ctx}: oracle variables-capture count differs from the manifest spec"
        );
        for v in &cp.variables {
            // Per-variable ledger exclusions (`R4133_PROPS_PLAN.md` RP3.10).
            // The key is the lowercased `element:variable` pair —
            // `windgen.w1:pgen` — with `:` as the separator because the element
            // name already carries the class dot; it is the `element.prop`
            // probe key above one level down. `compare_variables` still asserts
            // the variable COUNT unconditionally, so an exclusion can only ever
            // drop a value comparison, never hide a missing variable.
            let elem = v.name.to_lowercase();
            compare_variables(dss, v, tol, &ctx, &|var: &str| {
                excluded("variables", Some(&format!("{elem}:{}", var.to_lowercase())))
            });
        }
        if c.compare_eventlog {
            // A ledger eventlog `line_re` scope normalizes the diffing oracle line
            // (e.g. trailing-whitespace artifact) before the compare; unmatched
            // lines pass through unchanged.
            let masked: Vec<String> = match ledger {
                Some(v) => cp
                    .eventlog
                    .iter()
                    .map(|l| v.mask_line("eventlog", l))
                    .collect(),
                None => cp.eventlog.clone(),
            };
            // Then the two Relay label rows, which apply in **both** lanes
            // (GOLDEN_REBASE_PLAN.md G2.2d). The relay/recloser split is read
            // from the Rust circuit, which the element-name set above has
            // already pinned against the oracle.
            let relays: BTreeSet<String> = class_member_names(&snaps, "relay");
            let reclosers: BTreeSet<String> = class_member_names(&snaps, "recloser");
            let expected = lane::expected_eventlog(label, &masked, |name| {
                let n = name.to_lowercase();
                relays.contains(&n) && !reclosers.contains(&n)
            });
            compare_eventlog(dss, &expected, channel.eventlog_spec(), &ctx);
        }
        if c.compare_ctrlqueue {
            match ledger {
                Some(v) => {
                    let masked: Vec<String> = cp
                        .ctrlqueue
                        .iter()
                        .map(|l| v.mask_line("ctrlqueue", l))
                        .collect();
                    compare_ctrlqueue(dss, &masked, &ctx);
                }
                None => compare_ctrlqueue(dss, &cp.ctrlqueue, &ctx),
            }
        }

        if c.compare_global_result {
            compare_export(
                &cp.global_result,
                &rust_global_result,
                &global_result_policy(),
                &format!("{ctx} GlobalResult"),
            );
        }

        // The bus voltage surface (GOLDEN_REBASE_PLAN.md G1.4a). Placed here to
        // mirror the capi transport's capture slot — after `ctrlqueue`, before
        // the `all_properties` `?` sweep that must stay last
        // (`tools/oracle/oracle_server.py`). Every bus read is class C
        // (order-free) under §1.1(a)/D3: both engines read `Solution.NodeV`
        // directly (`CAPI/CAPI_Alt.pas:2275` == r4133 `DDLL/DBus.pas:423`) and
        // move only `ActiveBusIndex`, so nothing here can stale a cached
        // `Iterminal`.
        //
        // `voltages_excluded` is the one structural rule this surface needs: the
        // bus bands are exact images of the node-voltage band over the SAME
        // `Solution.NodeV` (`harness::compare_bus`), so on a case whose
        // `voltages` field is already ledger-excluded DECK-WIDE the bus arrays
        // would re-raise a divergence that is already triaged and pinned — ten
        // new ledger rows for one cause. It suppresses only the three continuous
        // arrays; the bus count, the name sequence, `nodes`, `kv_base` and every
        // array length stay compared on those cases too. A `voltages` scope that
        // names a node subset (`node_re`) suppresses NOTHING here — it would be
        // far wider than its cause; see `LedgerView::bus_arrays_suppressed`.
        if c.compare_bus {
            capture_guard::require_capture(
                "compare_bus",
                channel_tag(channel),
                cp.buses.len(),
                &ctx,
            );
            let v_excluded = ledger.is_some_and(|v| v.bus_arrays_suppressed(i));
            harness::compare_bus(dss, &cp.buses, tol, v_excluded, &ctx);
            harness::compare_all_bus_vmag_pu(dss, &cp.all_bus_vmag_pu, tol, v_excluded, &ctx);
        }

        if c.compare_all_properties {
            capture_guard::require_capture(
                "compare_all_properties",
                channel_tag(channel),
                cp.all_properties.len(),
                &ctx,
            );
            // A ledger `property` scope pins one (element, prop) pair — an exact
            // `oracle` pin, or `num_rel` for numeric-skeleton values (§1.3; same
            // contract as `probe`). To keep the monolithic `compare_all_properties`
            // count/order contract intact while excluding that one pair from the
            // value compare, rewrite its oracle value to the Rust `?`-surface value
            // (the ledger already asserted the Rust value against the pin/envelope),
            // so the standard compare treats it as equal.
            let prop_keys = ledger
                .map(|v| v.property_handled_keys(dss, &cp.all_properties, tol, &ctx))
                .unwrap_or_default();
            if prop_keys.is_empty() {
                compare_all_properties(dss, &cp.all_properties, tol, channel.props_channel(), &ctx);
            } else {
                let rewritten: Vec<harness::PropsCap> = cp
                    .all_properties
                    .iter()
                    .map(|pc| {
                        let el = pc.element.to_lowercase();
                        let props = pc
                            .props
                            .iter()
                            .map(|(name, val)| {
                                if prop_keys.contains(&(el.clone(), name.to_lowercase())) {
                                    dss.command(&format!("? {}.{}", pc.element, name));
                                    (name.clone(), dss.result().to_string())
                                } else {
                                    (name.clone(), val.clone())
                                }
                            })
                            .collect();
                        harness::PropsCap {
                            element: pc.element.clone(),
                            props,
                        }
                    })
                    .collect();
                compare_all_properties(dss, &rewritten, tol, channel.props_channel(), &ctx);
            }
        }

        // The PDElements interface walk (GOLDEN_REBASE G1.6b). Last in the
        // step, next to the other whole-model surface: `Dss::pd_elements` is a
        // `&self` read over `Circuit.pd_elements` that depends on no active
        // element and no solve state, so its position among the comparators is
        // free — unlike the CAPTURE order, which is fixed on both transports
        // (after the meters, before the probes) because the oracles' own
        // `ParentPDElement` read hijacks `ActiveCktElement`.
        //
        // `require_capture_opt`, not `require_capture`: 96 of the 372 walked
        // live capi cases hold no PD element at all, so an EMPTY walk is a
        // legitimate answer the comparator must still match (`[]` against a
        // non-empty port walk fails on the length assert). What must never
        // pass is an ABSENT field — a channel that ignored the request — and
        // the global collapse to zero everywhere, which
        // `harness::assert_pd_elements_compare_ran` catches in the epilogue.
        if c.compare_pdelements {
            let pde = capture_guard::require_capture_opt(
                "compare_pdelements",
                channel_tag(channel),
                cp.pd_elements.as_deref(),
                &ctx,
            );
            compare_pd_elements(dss, pde, channel.props_channel(), &ctx);
        }
    }

    if c.compare_autoadd_log {
        let oracle_log = capture_guard::require_capture_opt(
            "compare_autoadd_log",
            channel_tag(channel),
            oc.autoadd_log.as_deref(),
            label,
        );
        let case_name = dss
            .circuit()
            .expect("circuit exists after AutoAdd")
            .case_name
            .clone();
        let dir = Path::new(case_path)
            .parent()
            .expect("case_path has a parent dir");
        let log_path = dir.join(format!("{case_name}_AutoAddLog.csv"));
        let rust_log = std::fs::read_to_string(&log_path).unwrap_or_else(|e| {
            panic!("{label}: read Rust AutoAddLog {}: {e}", log_path.display())
        });
        compare_export(
            oracle_log,
            &rust_log,
            &autoadd_log_policy(tol),
            &format!("{label} AutoAddLog"),
        );
    }
}

/// Assert the oracle step counts, run the Rust engine once, compare against the
/// given `channel`'s capture (its iteration + eventlog-mask policy). No guard,
/// no oracle fetch — the caller (scheduler or [`run_and_compare`]) owns those.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compare_with_result(
    oc: &CaseResult,
    label: &str,
    case_path: &str,
    c: &SolvableCase,
    channel: EngineChannel,
    ledger: Option<&crate::ledger::LedgerView>,
) {
    assert_eq!(oc.n_steps, c.n_steps, "{label}: oracle step count");
    assert_eq!(
        oc.checkpoints.len(),
        c.n_steps,
        "{label}: oracle checkpoint count"
    );
    let tol = tol_for(&c.kind);
    let (mut dss, baseline) = run_rust_capture(label, case_path, c);
    compare_capture(
        &mut dss, baseline, oc, label, case_path, c, &tol, channel, ledger,
    );
}

/// One-shot convenience for the opt-in report tests: snapshot the case dir,
/// fetch the pinned oracle model once, compare against the `capi_v0145` channel
/// (no ledger — the report tests predate it).
pub(crate) fn run_and_compare(oracle: &Oracle, label: &str, case_path: &str, c: &SolvableCase) {
    let _guard = CorpusGuard::new(case_path);
    let oc = oracle.run_case(case_path, c);
    compare_with_result(&oc, label, case_path, c, EngineChannel::CapiV0145, None);
}

// ---------------------------------------------------------------------------
// Abort + pending contracts (unchanged behavior; abort now over a Channel).
// ---------------------------------------------------------------------------

/// Gate a deck that BOTH engines abort at solve. The oracle *raises* at solve
/// (so there is no solved state to line up); assert the ORACLE aborts with the
/// expected message and the RUST engine sets `solution_abort` + surfaces it.
pub(crate) fn run_and_compare_abort(
    channel: &Channel,
    label: &str,
    case_path: &str,
    c: &SolvableCase,
) {
    let expected = c
        .expect_solve_abort
        .as_deref()
        .expect("abort case has expect_solve_abort");

    // The abort request drives `Compile` (which executes the deck's own `Solve`);
    // the oracle raises on the aborting solve → `ok:false` carrying the message.
    let req = json!({
        "cmd": "run",
        "case_path": case_path,
        "post": c.post,
        "n_steps": c.n_steps,
        "selected_elements": c.selected_elements,
        "full_csc": true,
        "check_meters_monitors": false,
        "probes": [],
        "variables": [],
        "eventlog": false,
        "ctrlqueue": false,
    });
    let resp = channel.call(&req);
    assert!(
        !resp.ok,
        "{label}: oracle did NOT abort the solve (expected an abort containing {expected:?})"
    );
    let oracle_err = resp.error.unwrap_or_default();
    assert!(
        oracle_err.contains(expected),
        "{label}: oracle abort message {oracle_err:?} does not contain {expected:?}"
    );

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{case_path}\""));
    for cmd in &c.post {
        dss.command(cmd);
    }
    assert!(
        dss.circuit().is_some_and(|ckt| ckt.solution.solution_abort),
        "{label}: Rust engine did NOT set solution_abort — the malformed input must abort \
         the solve like the oracle (message: {expected:?})"
    );
    assert!(
        dss.errors().iter().any(|e| e.contains(expected)),
        "{label}: Rust engine did not surface {expected:?}: {:?}",
        dss.errors()
    );
}

/// Schema v2 (§4 Phase C step c): a `defer_ledger` case is parked from live
/// oracle comparison (its validated capi015 behavior reproduces on neither
/// surviving channel; a Phase D ledger will re-gate it). It is still **smoke-run**
/// on the Rust engine — compile + post + solve every step must converge with NO
/// new engine errors. This catches a *convergence or error-surfacing* regression
/// (a deferred case that stops solving, NaNs out, or starts erroring); it does
/// **not** catch a *numeric-correctness* regression that still converges — no
/// physical value is compared against any reference here. Full numeric coverage
/// on these cases returns with the Phase D ledger. This is strictly stronger than
/// the plan's `pending` fallback (which asserts an error) and membership is
/// preserved, but it is a bounded, temporary reduction in verification depth.
pub(crate) fn assert_deferred_rust_smoke(label: &str, case_path: &str, c: &SolvableCase) {
    let (mut dss, baseline_errors) = run_rust_capture(label, case_path, c);
    for i in 0..c.n_steps.max(1) {
        dss.command("solve");
        assert_eq!(
            dss.errors().len(),
            baseline_errors,
            "{label} (deferred smoke) step {i}: new Rust engine errors: {:?}",
            &dss.errors()[baseline_errors.min(dss.errors().len())..]
        );
        assert!(
            dss.circuit().is_some_and(|ckt| ckt.is_solved),
            "{label} (deferred smoke) step {i}: Rust did not converge — a deferred case \
             must still SOLVE on the Rust engine (defer_ledger parks the ORACLE compare, \
             not the Rust smoke)"
        );
    }
}

/// GAPS_PLAN.md §2.3 pending discipline: the unported feature must surface as an
/// engine error (a clean run means a silent fallback masks the gap).
pub(crate) fn assert_pending_errors_loudly(label: &str, case_path: &str, c: &SolvableCase) {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{case_path}\""));
    for cmd in &c.post {
        dss.command(cmd);
    }
    assert!(
        !dss.errors().is_empty(),
        "{label}: pending case (wp {}) ran with NO engine error — the unported \
         feature fell back silently; if it is now ported, flip `pending: false` \
         and prove the live compare green (GAPS_PLAN.md §3.1)",
        c.wp.as_deref().unwrap_or("?"),
    );
}

// ---------------------------------------------------------------------------
// Export policies (unchanged) + panic-message helper.
// ---------------------------------------------------------------------------

/// [`ExportPolicy`] for the AutoAdd `GlobalResult` line: bus name exact, GENADD
/// improvement figure on a measured faer-vs-KLU floor (see the original WPG.5
/// note in the pre-Phase-B `corpus_live.rs`).
fn global_result_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 1e-10,
        abs: 1e-12,
        col_tol: Vec::new(),
    }
}

/// [`ExportPolicy`] for the `AutoAddLog.csv`: fixed header row then comma-
/// tokenized per-bus rows on the `micro` energy floor.
fn autoadd_log_policy(tol: &Tolerances) -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: tol.energy_rel,
        abs: tol.energy_abs,
        col_tol: Vec::new(),
    }
}

/// Extract a readable message from a `catch_unwind` payload.
pub(crate) fn panic_msg(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}
