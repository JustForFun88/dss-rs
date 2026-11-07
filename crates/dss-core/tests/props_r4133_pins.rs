//! **The expected-value pins `PROPS_ECHO_R4133` owes**: every exclusion whose
//! ours-value has no capi witness (r4133-only classes/**cases**) carries its own
//! expected-value pin.
//!
//! # Why this file exists
//!
//! An `EchoRow` stops the r4133 **value** compare of one `(class, prop)` pair.
//! Such a divergence is excluded field by field **and pinned by its own
//! expected-value test**: otherwise the row is a mask with nothing behind it,
//! and a later regression in this engine's own readback would be invisible on
//! exactly the channel the row silences.
//!
//! `EchoRow::witness` names the holder of this engine's value for every row:
//! `Capi(n)` (the capi_v0145 channel still value-compares the pair on `n` gating
//! cases), `Pin(name)` (no capi coverage at all — the capture has no such
//! property under `PROPS_015X`, a `SKIP_PROPS` row masks it, the capi walk skips
//! the element whole, or every covered cell sits on a capi-only case), or
//! `CapiAndPin(n, name)`. Most tests below are those `name`s, and two more are
//! the deck guard's own self-tests. The literal list is pinned from the table's
//! side by `harness::props_norm::tests::every_live_semantics_row_names_a_pin`,
//! so a row cannot start pointing at a test that does not exist without moving
//! both sides in one commit.
//!
//! # The pins that no echo row can name
//!
//! Some tests below belong to no `EchoRow` at all, and could not: an r4133
//! getter that reads a field the `Edit` never wires, one that reads the *wrong*
//! live field, or one that computes a live value off a field it mis-derived.
//! All these getters are **live**, so an echo row there would be a false
//! statement about the mechanism. Their exclusion is a per-case `ledger.json`
//! `property` divergence entry, and these pins hold this engine's value. They
//! are cited by `props_r4133_replay::LEDGER_ENTRY_PINS` — the same guard
//! (`every_echo_row_pin_is_a_test_that_exists`) that forbids an un-cited
//! `#[test]` here reads that list too, so a pin cannot be added, renamed or
//! deleted without moving its citation with it.
//!
//! Three more groups hold pins of the same "no echo row could say this" kind,
//! each with its own citation list read by that same guard:
//! `props_r4133_replay::RP38_SUPERSEDED` and `RP38_CAPTURE_PIN` (five pairs
//! whose r4133 getters render live where the 0.14.5 capture is `''`),
//! `props_r4133_replay::RP39_PINS` (the pairs the display floor refuses because
//! the `%.Ng` round trip happened inside OpenDSS; see the block at the end of
//! this file), and `props_r4133_replay::RP312_UPSTREAM_BUG`, which holds this
//! engine's **regulated** `autotrans.wdgcurrents` where r4133's `RegControl` can
//! never tap an `AutoTrans` at all — an OpenDSS bug, so no exclusion row of any
//! kind could state this engine's value.
//!
//! # The shape of a pin
//!
//! Each one compiles **the corpus deck the property claims census flagged for
//! that pair** (`DSS_PROPS_CENSUS=claims`; the per-cell rows carry the case and
//! the element), reads this engine's live property render with
//! `? Class.Name.Prop` — the same getter the gate's property walk reads — and
//! asserts it literally. Where a bare "our render is X" could pass against a
//! getter that is simply hardwired, the pin adds the **discriminating** second
//! reading (a deck or an edit where the same getter answers something else), so
//! the assertion is about the value and not about the constant.
//!
//! The pins are deliberately **not** in the harness crate: they are
//! expected-value tests of the engine, a `dss-core` integration test like the
//! other pin binaries, and each runs once. They need no OpenDSS engine, no
//! feature flag and no channel, so they are green in both lanes.
//!
//! # Deck hygiene
//!
//! Several of the flagged decks write while they run — `Test/TD21RelayTest.DSS`
//! and its siblings end in `show eventlog`, `StorageControllerTechNote/Schedule/
//! ScheduleRun.dss` in nine `Export` commands. None of them runs in the
//! **vendored** corpus tree: [`Deck::compile`] compiles a
//! fresh scratch copy of the deck's closure (`harness/scratch.rs`, the corpus
//! gate's own copies, the one harness module used here), removed — loudly — when the deck
//! drops. [`DeckDirGuard`] brackets the copy's deck folder, the gate's
//! guard contract in miniature: snapshot it, delete on the way out every file
//! the run created, and fail loudly if the run *changed* a copied byte count
//! instead of only adding files. [`the_deck_guard_really_sweeps`] proves it is
//! not a no-op.
//!
//! It also carries `CorpusGuard`'s **shared per-directory snapshot**: guards
//! over one directory can overlap under `cargo test`'s thread pool, and with
//! per-guard snapshots the interleaving leaks — A snapshots clean, A writes, B
//! snapshots (and sees A's output as vendored), A sweeps, B writes again, B
//! *keeps* it. One snapshot per directory plus a reference count means the
//! names set is always the pristine one and exactly one sweep runs, when the
//! last guard leaves ([`overlapping_deck_guards_still_sweep`]).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use dss_core::exec::Dss;

// Every deck compiles its own fresh scratch copy, made by the harness crate's
// `scratch` module, the corpus gate's own copier.
use dss_test_harness::harness::scratch;

// ---------------------------------------------------------------------------
// Deck plumbing
// ---------------------------------------------------------------------------

/// The vendored corpus root — the same tree the live gate reads (never
/// `.inputs/`, which only hand-run tools read).
fn corpus_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "corpus"]
        .iter()
        .collect()
}

/// Every regular file under `dir`, recursively, with its byte length.
fn walk(dir: &Path, out: &mut BTreeMap<PathBuf, u64>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let path = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => walk(&path, out),
            Ok(t) if t.is_file() => {
                out.insert(path, e.metadata().map(|m| m.len()).unwrap_or_default());
            }
            _ => {}
        }
    }
}

/// One pristine snapshot per deck directory, plus the number of live guards
/// holding it — the module doc's interleaving fix.
type DirSnapshots = HashMap<PathBuf, (Arc<BTreeMap<PathBuf, u64>>, usize)>;
type DirRegistry = Mutex<DirSnapshots>;

fn dir_registry() -> &'static DirRegistry {
    static REG: OnceLock<DirRegistry> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Poisoning is not a reason to fail a *different* test: a pin that panicked
/// while holding the lock has already reported itself.
fn lock_registry() -> std::sync::MutexGuard<'static, DirSnapshots> {
    dir_registry().lock().unwrap_or_else(|e| e.into_inner())
}

/// Keeps the copied deck folder (a scratch copy) as it was found:
/// files the run created are removed when the **last** guard on that folder
/// drops, and a *modified* copied file is a loud failure (the decks below only
/// ever add).
struct DeckDirGuard {
    dir: PathBuf,
    before: Arc<BTreeMap<PathBuf, u64>>,
}

impl DeckDirGuard {
    fn new(dir: &Path) -> Self {
        let mut reg = lock_registry();
        let before = match reg.get_mut(dir) {
            Some((snap, holders)) => {
                *holders += 1;
                Arc::clone(snap)
            }
            None => {
                let mut fresh = BTreeMap::new();
                walk(dir, &mut fresh);
                let snap = Arc::new(fresh);
                reg.insert(dir.to_path_buf(), (Arc::clone(&snap), 1));
                snap
            }
        };
        Self {
            dir: dir.to_path_buf(),
            before,
        }
    }
}

impl Drop for DeckDirGuard {
    fn drop(&mut self) {
        {
            let mut reg = lock_registry();
            match reg.get_mut(&self.dir) {
                Some((_, holders)) if *holders > 1 => {
                    *holders -= 1;
                    // A sibling guard is still running its deck; it sweeps.
                    return;
                }
                _ => {
                    reg.remove(&self.dir);
                }
            }
        }
        let mut after = BTreeMap::new();
        walk(&self.dir, &mut after);
        let mut changed = Vec::new();
        for (path, len) in &after {
            match self.before.get(path) {
                None => {
                    let _ = std::fs::remove_file(path);
                }
                Some(was) if was != len => changed.push(path.display().to_string()),
                Some(_) => {}
            }
        }
        // Never mask the real failure by panicking while another panic unwinds.
        if !std::thread::panicking() {
            assert!(
                changed.is_empty(),
                "a pinned deck rewrote corpus bytes copied into its scratch copy \
                 (not merely added output files), which this guard cannot restore: {changed:?}"
            );
        }
    }
}

/// A compiled corpus deck, plus the scratch copy it compiled and the guard
/// that cleans up after it.
///
/// Field order is the drop order: the engine is torn down first, then the guard
/// sweeps — so anything the deck's own `Export`/`Show` wrote is already flushed
/// when the sweep runs — and then the copy is removed (loudly: a copy that
/// survives its budget fails the pin naming the producer).
struct Deck {
    dss: Dss,
    _guard: DeckDirGuard,
    _copy: scratch::ScratchCopy,
}

impl Deck {
    /// Compile `rel` (relative to `tests/corpus/`) and require a clean run.
    fn compile(rel: &str) -> Self {
        Self::compile_inner(rel, &[])
    }

    /// Same, for the one deck whose run legitimately raises a **non-fatal**
    /// diagnostic (`allowed` = its DSS numbers).
    fn compile_allowing(rel: &str, allowed: &[u32]) -> Self {
        Self::compile_inner(rel, allowed)
    }

    fn compile_inner(rel: &str, allowed: &[u32]) -> Self {
        let vendored = corpus_root().join(rel);
        assert!(
            vendored.is_file(),
            "vendored deck is missing: {}",
            vendored.display()
        );
        let copy = scratch::ScratchCopy::new(&vendored.to_string_lossy(), scratch::PORT).loud();
        // The path compiled below is refused when it is the vendored deck.
        let path = PathBuf::from(copy.deck());
        scratch::not_vendored(&path, "props_r4133_pins::Deck::compile_inner");
        let guard = DeckDirGuard::new(path.parent().expect("a deck has a directory"));
        let mut dss = Dss::new();
        dss.command(&format!("compile \"{}\"", path.display()));
        for e in dss.errors() {
            assert!(
                !e.abort && e.code.is_some_and(|c| allowed.contains(&c)),
                "{rel}: unexpected diagnostic {:?} {}",
                e.code,
                e.message
            );
        }
        Self {
            dss,
            _guard: guard,
            _copy: copy,
        }
    }

    /// The port's live render of `Class.Name.Prop` — the very getter the gate's
    /// property walk reads.
    fn get(&mut self, target: &str) -> String {
        self.dss.command(&format!("? {target}"));
        let value = self.dss.result().to_string();
        assert_ne!(
            value, "Property Unknown",
            "{target}: no such property on this element (a typo pins nothing)"
        );
        value
    }

    /// Run a DSS command (used only to build a pin's discriminating half).
    fn cmd(&mut self, command: &str) {
        self.dss.command(command);
        assert!(
            self.dss.errors().iter().all(|e| !e.abort),
            "{command}: {:?}",
            self.dss.errors()
        );
    }
}

/// The guard is a real sweep, not a no-op: a file created under it disappears,
/// and a pre-existing file survives untouched.
#[test]
fn the_deck_guard_really_sweeps() {
    let root = std::env::temp_dir().join(format!("rp23_pin_guard_{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(root.join("sub")).unwrap();
    let vendored = root.join("sub").join("vendored.txt");
    std::fs::write(&vendored, b"vendored bytes").unwrap();
    let created = root.join("sub").join("output.csv");
    {
        let _guard = DeckDirGuard::new(&root);
        std::fs::write(&created, b"deck output").unwrap();
        assert!(created.is_file());
    }
    assert!(
        !created.exists(),
        "the guard must delete what the run created"
    );
    assert_eq!(std::fs::read(&vendored).unwrap(), b"vendored bytes");
    std::fs::remove_dir_all(&root).ok();
}

/// Two guards on the **same** directory share one pristine snapshot, so the
/// sweep runs once, when the last guard leaves, and no guard keeps another's
/// output as vendored.
#[test]
fn overlapping_deck_guards_still_sweep() {
    let root = std::env::temp_dir().join(format!("rp23_pin_overlap_{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(&root).unwrap();
    let out = root.join("deck_EXP_EventLog.csv");
    {
        let a = DeckDirGuard::new(&root);
        std::fs::write(&out, b"first run").unwrap();
        {
            // B snapshots while A's output is on disk…
            let b = DeckDirGuard::new(&root);
            drop(a); // …and A leaving must not sweep out from under B.
            assert!(out.is_file(), "the first guard out must not sweep");
            std::fs::write(&out, b"second run, different length").unwrap();
            drop(b);
        }
    }
    assert!(
        !out.exists(),
        "the last guard out sweeps against the PRISTINE snapshot"
    );
    std::fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------------
// The pins, alphabetically by the pair(s) they carry
// ---------------------------------------------------------------------------

/// `autotrans.bhcurrent` / `autotrans.bhflux` render an empty array while no
/// B-H curve is set, since an unset curve has no points. Pin-only: the capi
/// capture drops both properties.
#[test]
fn autotrans_bh_arrays_render_empty_when_unset() {
    let mut deck = Deck::compile("asymmetric/autotrans/autotrans_gic.dss");
    assert_eq!(deck.get("AutoTrans.t1.BHpoints"), "0");
    assert_eq!(deck.get("AutoTrans.t1.BHCurrent"), "");
    assert_eq!(deck.get("AutoTrans.t1.BHFlux"), "");
    // Discriminating half: with points allocated the same getter renders the
    // array, so `''` above is the empty state and not a dead arm.
    deck.cmd("edit AutoTrans.t1 bhpoints=3 bhcurrent=(1 2 3) bhflux=(4 5 6)");
    assert_eq!(deck.get("AutoTrans.t1.BHCurrent"), "[ 1 2 3]");
    assert_eq!(deck.get("AutoTrans.t1.BHFlux"), "[ 4 5 6]");
}

/// `energymeter.peakcurrent` renders the live peak-current array, one element
/// per phase of the metered terminal, not a frozen three-element default.
#[test]
fn energymeter_peakcurrent_renders_the_live_one_element_array() {
    let mut one_phase = Deck::compile(
        "electricdss-tst/Version8/Distrib/Examples/Scripts/IEEE-TIA-LV Model/master_small.dss",
    );
    assert_eq!(one_phase.get("EnergyMeter.sub.PeakCurrent"), "[ 400]");
    let mut three_phase = Deck::compile("controls/energymeter/energymeter_sym.dss");
    assert_eq!(
        three_phase.get("EnergyMeter.em.PeakCurrent"),
        "[ 400 400 400]"
    );
}

/// `expcontrol.derlist` renders the user's typed DER list, the value the
/// property documents.
#[test]
fn expcontrol_derlist_renders_the_der_list() {
    let mut deck = Deck::compile("controls/expcontrol/expcontrol_basic.dss");
    assert_eq!(deck.get("ExpControl.ec.PVSystemList"), "[pv]");
    assert_eq!(deck.get("ExpControl.ec.DERList"), "[PVSystem.pv]");
}

/// `fault.bus2` renders the live second terminal of the Fault, which defaults
/// to bus 1's node-zero connection when not typed.
#[test]
fn fault_bus2_renders_the_live_terminal() {
    let mut deck = Deck::compile("modes/makeposseq/makeposseq_shunt.dss");
    assert_eq!(deck.get("Fault.flt.phases"), "1");
    assert_eq!(deck.get("Fault.flt.bus1"), "b2");
    assert_eq!(deck.get("Fault.flt.bus2"), "b2.0");
}

/// `generator.d` renders the documented damping default `1.0` and follows an
/// edit. The OpenDSS documentation gives that default, so a deck that never sets
/// `D=` runs its dynamics with damping.
#[test]
fn generator_d_renders_the_documented_damping_default() {
    let mut deck = Deck::compile("asymmetric/generator/generator_asym.dss");
    assert_eq!(deck.get("Generator.g1.D"), "1");
    deck.cmd("edit Generator.g1 D=2.5");
    assert_eq!(deck.get("Generator.g1.D"), "2.5");
}

/// `generator.dynout` renders the names of the dynamic output variables the
/// deck selected.
#[test]
fn generator_dynout_renders_the_named_variables() {
    let mut deck = Deck::compile(
        "electricdss-tst/Version8/Distrib/Examples/Dynamic_Expressions/\
         Dynamic_KundurDynExp-steady-state-only.dss",
    );
    assert_eq!(deck.get("Generator.g1.DynamicEq"), "mydiffeq");
    assert_eq!(deck.get("Generator.g1.DynOut"), "[speed, theta]");
    // …and with no dynamic expression the same getter is empty (the 271-cell
    // half of the row).
    let mut plain = Deck::compile("asymmetric/generator/generator_asym.dss");
    assert_eq!(plain.get("Generator.g1.DynamicEq"), "");
    assert_eq!(plain.get("Generator.g1.DynOut"), "");
}

/// `generator.model` renders the live model after NCIM converts a generator
/// from PV to PQ, not the token the deck typed. The model that ran the solve is
/// the one an engineer reads back.
#[test]
fn generator_model_renders_the_live_pv2pq_conversion() {
    for (deck, maxkvar) in [
        ("modes/ncim/ncim_pv_pq.dss", "1500"),
        ("modes/ncim/ncim_midi.dss", "400"),
    ] {
        let mut d = Deck::compile(deck);
        assert_eq!(
            d.get("Generator.g1.maxkvar"),
            maxkvar,
            "{deck}: a nonzero Q limit is what makes this the Solution.pas:2120 promote"
        );
        assert_eq!(
            d.get("Generator.g1.model"),
            "4",
            "{deck}: the PV->PQ conversion left the live GenModel at 4"
        );
        // Not a constant: the deck's own token renders back…
        d.cmd("edit Generator.g1 model=3");
        assert_eq!(d.get("Generator.g1.model"), "3");
        // …and re-solving converts it again, so the pin is on the engine's field
        // and not on the parser's echo of the last thing typed.
        d.cmd("solve");
        assert_eq!(
            d.get("Generator.g1.model"),
            "4",
            "{deck}: NCIM re-converts the restored model-3 generator"
        );
    }
}

/// `gicsource.spectrum` renders the empty spectrum a GICSource carries: a
/// quasi-DC source has no harmonic spectrum.
#[test]
fn gicsource_spectrum_renders_the_live_empty_spectrum() {
    let mut deck = Deck::compile("asymmetric/gic/gic_midi.dss");
    for src in ["seg45", "seg56"] {
        assert_eq!(
            deck.get(&format!("GICsource.{src}.spectrum")),
            "",
            "GICsource.{src}: the class forbids a spectrum; r4133 renders the frozen 'default'"
        );
    }
    deck.cmd("edit GICsource.seg45 spectrum=default");
    assert_eq!(deck.get("GICsource.seg45.spectrum"), "default");
    assert_eq!(deck.get("GICsource.seg56.spectrum"), "");
}

/// `gictransformer.pctperm` renders the live `100` the fault-rate computation
/// uses, and follows an edit.
#[test]
fn gictransformer_pctperm_renders_the_live_rating() {
    let mut deck = Deck::compile("asymmetric/gic/gictransformer_gic.dss");
    for xf in ["tg1", "tg2", "tg3"] {
        assert_eq!(
            deck.get(&format!("GICTransformer.{xf}.pctperm")),
            "100",
            "GICTransformer.{xf}: Create's live PctPerm; r4133 renders its frozen '0'"
        );
    }
    deck.cmd("edit GICTransformer.tg2 pctperm=42");
    assert_eq!(deck.get("GICTransformer.tg2.pctperm"), "42");
    assert_eq!(deck.get("GICTransformer.tg1.pctperm"), "100");
}

/// `line.conductors` and its siblings `wires` / `cncables` / `tscables` render
/// the live conductor list of a geometry-built Line.
#[test]
fn line_conductors_renders_the_live_conductor_list() {
    let mut geom = Deck::compile("modes/upgrade/upgrade_spacing_ratings.dss");
    for prop in ["Conductors", "wires", "cncables", "tscables"] {
        assert_eq!(
            geom.get(&format!("Line.l1.{prop}")),
            "[big, small, big, neut]",
            "the live conductor list of a spacing/wires line, read through {prop}"
        );
    }
    let mut plain = Deck::compile("asymmetric/autotrans/autotrans_gic.dss");
    for prop in ["Conductors", "wires", "cncables", "tscables"] {
        assert_eq!(plain.get(&format!("Line.line1.{prop}")), "[]");
    }
}

/// `line.spacing` renders empty once the Line no longer uses a spacing, since
/// the property states the spacing the Line is built from.
#[test]
fn line_spacing_renders_empty_once_the_spacing_is_killed() {
    let mut killed = Deck::compile("modes/makeposseq/makeposseq_line.dss");
    assert_eq!(killed.get("Line.l_spc.Spacing"), "");
    let mut alive = Deck::compile("modes/upgrade/upgrade_spacing_ratings.dss");
    assert_eq!(alive.get("Line.l1.Spacing"), "sp");
}

/// `load.yearly` renders the name of the LoadShape the Load uses for a yearly
/// run, which follows `daily=` when `yearly=` is not typed, as the property
/// documents.
#[test]
fn load_yearly_renders_the_resolved_loadshape_name() {
    let mut deck = Deck::compile("controls/capcontrol/capcontrol_sym.dss");
    assert_eq!(deck.get("Load.l1.Daily"), "day");
    assert_eq!(
        deck.get("Load.l1.Yearly"),
        "day",
        "an untyped Yearly resolves to the daily shape object"
    );
    assert_eq!(deck.get("Load.l1.Duty"), "", "Duty is not aliased");
}

/// `%PminNoVars` / `%PminkvarMax` of PVSystem and Storage render the sentinel
/// that deactivates the limit while it is not set.
#[test]
fn pvsystem_and_storage_pmin_sentinels_deactivate_the_var_limits() {
    let mut deck = Deck::compile("asymmetric/der/der_asym.dss");
    for target in [
        "PVSystem.pv1.%PminNoVars",
        "PVSystem.pv1.%PminkvarMax",
        "Storage.st1.%PminNoVars",
        "Storage.st1.%PminkvarMax",
    ] {
        assert_eq!(deck.get(target), "0", "{target} starts deactivated");
    }
    deck.cmd("edit PVSystem.pv1 %pminnovars=25");
    assert_eq!(deck.get("PVSystem.pv1.%PminNoVars"), "25");
    deck.cmd("edit Storage.st1 %pminkvarmax=33");
    assert_eq!(deck.get("Storage.st1.%PminkvarMax"), "33");
}

/// `recloser.eventlog` / `recloser.debugtrace` render the live flags, `No` by
/// default.
#[test]
fn recloser_eventlog_and_debugtrace_default_to_no() {
    let mut deck = Deck::compile("controls/recloser/recloser_perm.dss");
    assert_eq!(deck.get("Recloser.r.EventLog"), "No");
    assert_eq!(deck.get("Recloser.r.DebugTrace"), "No");
    let mut logging = Deck::compile("controls/recloser/recloser_1ph.dss");
    assert_eq!(logging.get("Recloser.r.EventLog"), "Yes");
}

/// `recloser.switchedobj` defaults to the monitored element, the element the
/// Recloser actually switches when `SwitchedObj=` is not typed.
#[test]
fn recloser_switchedobj_defaults_to_the_monitored_element() {
    let mut deck = Deck::compile("controls/fuse/indmach_r4133/indmach_snap.dss");
    for (recloser, line) in [
        ("cb1", "Line.l1"),
        ("cb2", "Line.l7"),
        ("rec1", "Line.l3"),
        ("rec2", "Line.l5"),
    ] {
        assert_eq!(deck.get(&format!("Recloser.{recloser}.MonitoredObj")), line);
        assert_eq!(deck.get(&format!("Recloser.{recloser}.SwitchedObj")), line);
    }
}

/// The RegControl idle flags and forward/reverse thresholds render the live
/// values the controller acts on.
#[test]
fn regcontrol_idle_flags_and_thresholds_render_the_live_values() {
    let mut sym = Deck::compile("controls/regcontrol/regcontrol_sym.dss");
    assert_eq!(sym.get("RegControl.rg.Idle"), "No");
    assert_eq!(sym.get("RegControl.rg.IdleForward"), "No");
    assert_eq!(sym.get("RegControl.rg.IdleReverse"), "No");
    assert_eq!(sym.get("RegControl.rg.FwdThreshold"), "100");
    assert_eq!(
        sym.get("RegControl.rg.RevThreshold"),
        "-100",
        "the live reverse threshold is negative where the store froze '100'"
    );

    let mut reverse = Deck::compile("controls/regcontrol/regcontrol_reverse.dss");
    assert_eq!(reverse.get("RegControl.rg.FwdThreshold"), "800");
    assert_eq!(reverse.get("RegControl.rg.RevThreshold"), "-800");

    let mut idle = Deck::compile("controls/regcontrol/regcontrol_idle.dss");
    assert_eq!(idle.get("RegControl.rg.Idle"), "Yes");
}

/// `relay.action`, `relay.distreverse` and `relay.reset` render the live
/// state: no pending action, the live reverse flag, no pending reset.
#[test]
fn relay_action_distreverse_and_reset_render_the_live_values() {
    let mut deck = Deck::compile("electricdss-tst/Test/TD21RelayTest.DSS");
    assert_eq!(deck.get("Relay.21src.Action"), "");
    assert_eq!(deck.get("Relay.21src.DistReverse"), "No");
    assert_eq!(
        deck.get("Relay.21src.Reset"),
        "No",
        "the live reset flag, where r4133 still echoes the deck's '0.20' token"
    );
    let mut reverse = Deck::compile("electricdss-tst/Test/ReverseTD21RelayTest.DSS");
    assert_eq!(reverse.get("Relay.21src.DistReverse"), "Yes");
}

/// `relay.switchedobj` defaults to the monitored element, the element the Relay
/// actually switches when `SwitchedObj=` is not typed.
#[test]
fn relay_switchedobj_defaults_to_the_monitored_element() {
    let mut deck = Deck::compile("electricdss-tst/Test/DistanceRelayTest.DSS");
    assert_eq!(deck.get("Relay.21src.MonitoredObj"), "Line.thev");
    assert_eq!(deck.get("Relay.21src.SwitchedObj"), "Line.thev");
}

/// `storage.dynadll` renders the path the deck typed, whether or not the user
/// model loaded.
#[test]
fn storage_dynadll_renders_the_typed_path() {
    let mut deck =
        Deck::compile_allowing("electricdss-tst/Test/SimpleStorageTest-1ph.dss", &[1570]);
    assert_eq!(
        deck.get("Storage.store1.DynaDLL"),
        r"C:\Users\prdu001\OpenDSS\Source\DESS1\Dess1.DLL"
    );
    assert_eq!(
        deck.get("Storage.store1.DynaData"),
        "file=DESSModel_Test.TxT"
    );
}

/// `storagecontroller.modedischarge` renders `Schedule` for the schedule mode
/// the controller runs, not `UNKNOWN`.
#[test]
fn storagecontroller_modedischarge_renders_schedule() {
    let mut deck = Deck::compile(
        "electricdss-tst/Version8/Distrib/Examples/StorageControllerTechNote/Schedule/\
         ScheduleRun.dss",
    );
    assert_eq!(deck.get("StorageController.sc.ModeDischarge"), "Schedule");
    let mut peakshave = Deck::compile("controls/storagecontroller/storagectrl_peakshave.dss");
    assert_eq!(
        peakshave.get("StorageController.sc.ModeDischarge"),
        "Peakshave"
    );
}

/// `swtcontrol.action` renders the live switch state the SwtControl holds.
#[test]
fn swtcontrol_action_renders_the_live_switch_state() {
    let mut deck =
        Deck::compile("electricdss-tst/Version8/Distrib/Examples/civinlar model/civanlar.dss");
    assert_eq!(deck.get("SwtControl.13_14.Action"), "close");
    assert_eq!(
        deck.get("SwtControl.13_14.State"),
        "[closed, closed, closed, ]"
    );
    assert_eq!(deck.get("SwtControl.10_14.Action"), "open");
    assert_eq!(deck.get("SwtControl.10_14.State"), "[open, open, open, ]");
}

/// `transformer.bhcurrent` / `transformer.bhflux` render an empty array while
/// no B-H curve is set, since an unset curve has no points.
#[test]
fn transformer_bh_arrays_render_empty_when_unset() {
    let mut deck = Deck::compile("controls/regcontrol/regcontrol_sym.dss");
    assert_eq!(deck.get("Transformer.tr.BHpoints"), "0");
    assert_eq!(deck.get("Transformer.tr.BHCurrent"), "");
    assert_eq!(deck.get("Transformer.tr.BHFlux"), "");
    deck.cmd("edit Transformer.tr bhpoints=2 bhcurrent=(7 8) bhflux=(9 10)");
    assert_eq!(deck.get("Transformer.tr.BHCurrent"), "[ 7 8]");
    assert_eq!(deck.get("Transformer.tr.BHFlux"), "[ 9 10]");
}

/// `windgen.dynout` renders empty while no dynamic output is selected.
#[test]
fn windgen_dynout_renders_empty_when_unset() {
    let mut deck = Deck::compile("modes/windgen/windgen_snap.dss");
    assert_eq!(deck.get("WindGen.w1.DynamicEq"), "");
    assert_eq!(deck.get("WindGen.w1.DynOut"), "");
}

// ---------------------------------------------------------------------------
// Pins for the rows whose masked cells sit on `engines: "r4133"` cases
// ---------------------------------------------------------------------------
//
// An exclusion whose ours-value has no capi witness on the cases it masks
// (`props_norm::ECHO_ROWS_ON_R4133_ONLY_CASES`) gets a pin here. Each pin reads
// this engine's live render on a deck the census named for that pair (an
// r4133-only one wherever the pair has such a case), and adds the
// discriminating second reading wherever the property can be written back.
// Two of them cannot: `action=` on an EnergyMeter and `reset=` on a CapControl
// are one-shot COMMANDS whose re-read is empty by construction, which is itself
// the thing being pinned.

/// `AmpLimit` / `AmpLimitGain` of PVSystem and Storage render the live
/// deactivated sentinel and the live gain.
#[test]
fn der_amp_limits_render_the_live_sentinel_and_gain() {
    let mut pv = Deck::compile("controls/invcontrol/invcontrol_vv_delta.dss");
    assert_eq!(
        pv.get("PVSystem.pv1.AmpLimit"),
        "-1",
        "deactivated by default"
    );
    assert_eq!(pv.get("PVSystem.pv1.AmpLimitGain"), "0.8");
    pv.cmd("edit PVSystem.pv1 amplimit=2.5");
    assert_eq!(pv.get("PVSystem.pv1.AmpLimit"), "2.5");

    let mut st = Deck::compile("controls/gfm/gfm_micro.dss");
    assert_eq!(st.get("Storage.batt.AmpLimit"), "-1");
    assert_eq!(st.get("Storage.batt.AmpLimitGain"), "0.8");
    st.cmd("edit Storage.batt amplimit=3.5 amplimitgain=0.25");
    assert_eq!(st.get("Storage.batt.AmpLimit"), "3.5");
    assert_eq!(st.get("Storage.batt.AmpLimitGain"), "0.25");
}

/// The user-model data arrays of Generator, PVSystem and Storage render empty
/// while no user model is set.
#[test]
fn der_user_model_arrays_render_empty_when_unset() {
    let mut machine = Deck::compile("controls/relay/relay_doc.dss");
    assert_eq!(machine.get("Generator.g1.ShaftData"), "");
    assert_eq!(machine.get("Generator.g1.UserData"), "");
    machine.cmd("edit Generator.g1 UserData=(k=1)");
    machine.cmd("edit Generator.g1 ShaftData=(m=2)");
    assert_eq!(machine.get("Generator.g1.UserData"), "k=1");
    assert_eq!(machine.get("Generator.g1.ShaftData"), "m=2");

    let mut pv = Deck::compile("controls/invcontrol/invcontrol_vv_delta.dss");
    assert_eq!(pv.get("PVSystem.pv1.DynOut"), "");
    assert_eq!(pv.get("PVSystem.pv1.UserData"), "");
    pv.cmd("edit PVSystem.pv1 UserData=(x=9)");
    assert_eq!(pv.get("PVSystem.pv1.UserData"), "x=9");

    let mut st = Deck::compile("controls/gfm/gfm_micro.dss");
    assert_eq!(st.get("Storage.batt.DynaData"), "");
    assert_eq!(st.get("Storage.batt.UserData"), "");
    st.cmd("edit Storage.batt UserData=(a=1,b=2) DynaData=(c=3)");
    assert_eq!(st.get("Storage.batt.UserData"), "a=1,b=2");
    assert_eq!(st.get("Storage.batt.DynaData"), "c=3");
}

/// `energymeter.action` and `capcontrol.reset` are one-shot commands, so they
/// render no pending command.
#[test]
fn energymeter_action_and_capcontrol_reset_render_no_pending_command() {
    let mut meter = Deck::compile("controls/combo/combo_protection.dss");
    assert_eq!(meter.get("EnergyMeter.em.action"), "");
    meter.cmd("edit EnergyMeter.em action=clear");
    assert_eq!(
        meter.get("EnergyMeter.em.action"),
        "",
        "a one-shot command leaves no pending action to report"
    );

    let mut caps = Deck::compile(
        "electricdss-tst/Version8/Distrib/EPRITestCircuits/epri_dpv/M1/Master_NoPV.dss",
    );
    assert_eq!(caps.get("CapControl.cap1_ctrl.Reset"), "No");
    caps.cmd("edit CapControl.cap1_ctrl reset=yes");
    assert_eq!(
        caps.get("CapControl.cap1_ctrl.Reset"),
        "No",
        "the reset flag is consumed by the command, not latched"
    );
}

/// `fuse.switchedobj` defaults to the monitored element, the element the Fuse
/// actually switches when `SwitchedObj=` is not typed.
#[test]
fn fuse_switchedobj_defaults_to_the_monitored_element() {
    let mut deck = Deck::compile("controls/fuse/indmach_r4133/indmach_snap.dss");
    for (fuse, monitored) in [("f1", "Line.l6"), ("f2", "Transformer.tg")] {
        assert_eq!(deck.get(&format!("Fuse.{fuse}.MonitoredObj")), monitored);
        assert_eq!(
            deck.get(&format!("Fuse.{fuse}.SwitchedObj")),
            monitored,
            "an untyped SwitchedObj defaults to the monitored element"
        );
    }
    deck.cmd("edit Fuse.f1 switchedobj=Line.l1");
    assert_eq!(deck.get("Fuse.f1.SwitchedObj"), "Line.l1");
}

/// The InvControl defaults (`LPFTau`, `RiseFallLimit`, `VSetPoint`,
/// `PVSystemList`, `monVoltageCalc`, `Mode`) render the live values the control
/// acts on.
#[test]
fn invcontrol_defaults_render_the_live_values() {
    let mut deck = Deck::compile("controls/gfm/gfm_invcontrol.dss");
    assert_eq!(deck.get("InvControl.ic.LPFTau"), "0.001");
    assert_eq!(deck.get("InvControl.ic.RiseFallLimit"), "0.001");
    assert_eq!(deck.get("InvControl.ic.Vsetpoint"), "1");
    assert_eq!(deck.get("InvControl.ic.PVSystemList"), "[Storage.batt]");
    assert_eq!(deck.get("InvControl.ic.monVoltageCalc"), "avg");
    deck.cmd("edit InvControl.ic LPFTau=0.5 RiseFallLimit=0.75 Vsetpoint=1.02 monVoltageCalc=max");
    assert_eq!(deck.get("InvControl.ic.LPFTau"), "0.5");
    assert_eq!(deck.get("InvControl.ic.RiseFallLimit"), "0.75");
    assert_eq!(deck.get("InvControl.ic.Vsetpoint"), "1.02");
    assert_eq!(deck.get("InvControl.ic.monVoltageCalc"), "max");

    let mut combi = Deck::compile(
        "electricdss-tst/Version8/Distrib/Examples/InverterModels/PVSystem/InvControl/\
         MonitoredVoltage/Mon_voltage_MAX_Mix_avg_VVDRC-2.dss",
    );
    assert_eq!(combi.get("InvControl.vv_drc.CombiMode"), "VV_DRC");
    assert_eq!(
        combi.get("InvControl.vv_drc.Mode"),
        "",
        "a CombiMode control has no single Mode"
    );
    combi.cmd("edit InvControl.vv_drc mode=voltvar");
    assert_eq!(combi.get("InvControl.vv_drc.Mode"), "Voltvar");
}

/// `load.zipv` renders the live seven-element ZIPV vector.
#[test]
fn load_zipv_renders_the_live_seven_element_vector() {
    let mut deck = Deck::compile("controls/gfm/gfm_micro.dss");
    assert_eq!(deck.get("Load.isl.ZIPV"), "[ 0 0 0 0 0 0 0]");
    deck.cmd("edit Load.isl ZIPV=(1,2,3,4,5,6,0.5)");
    assert_eq!(deck.get("Load.isl.ZIPV"), "[ 1 2 3 4 5 6 0.5]");
}

/// `pctperm` / `repair` of Transformer, AutoTrans and Fault render the live
/// reliability values the fault-rate computation uses.
#[test]
fn pd_element_perm_and_repair_render_the_live_ratings() {
    let mut xf = Deck::compile("controls/gfm/gfm_micro.dss");
    assert_eq!(xf.get("Transformer.tsto.pctperm"), "0");
    assert_eq!(xf.get("Transformer.tsto.repair"), "0");
    xf.cmd("edit Transformer.tsto pctperm=42 repair=7");
    assert_eq!(xf.get("Transformer.tsto.pctperm"), "42");
    assert_eq!(xf.get("Transformer.tsto.repair"), "7");

    let mut at = Deck::compile("asymmetric/autotrans/autotrans_xfmrcode.dss");
    assert_eq!(at.get("AutoTrans.t1.pctperm"), "0");
    assert_eq!(at.get("AutoTrans.t1.repair"), "0");
    at.cmd("edit AutoTrans.t1 pctperm=11");
    assert_eq!(at.get("AutoTrans.t1.pctperm"), "11");

    let mut flt = Deck::compile("controls/fuse/fuse_blow_3ph.dss");
    assert_eq!(
        flt.get("Fault.f.pctperm"),
        "100",
        "a Fault starts fully permanent, where r4133's store says 0"
    );
    flt.cmd("edit Fault.f pctperm=13");
    assert_eq!(flt.get("Fault.f.pctperm"), "13");
}

/// `reactor.kvar` renders the live rating and follows an edit, and after
/// `MakePosSequence` it is the exact third of the three-phase rating.
#[test]
fn reactor_kvar_renders_the_live_rating() {
    let mut deck = Deck::compile("controls/fuse/midi_fuse.dss");
    assert_eq!(deck.get("Reactor.rser.kvar"), "100");
    deck.cmd("edit Reactor.rser kvar=250");
    assert_eq!(deck.get("Reactor.rser.kvar"), "250");

    let mut pos = Deck::compile("modes/makeposseq/makeposseq_shunt.dss");
    assert_eq!(
        pos.get("Reactor.rx_kvar.kvar"),
        "66.6666666666667",
        "MakePosSequence divides the three-phase rating exactly; r4133 re-parses its own \
         '%-.5g' render and lands on 66.667 (the carved-out cell of the display class)"
    );
}

/// `seasontargets` / `seasontargetslow` of StorageController render the live
/// target arrays.
#[test]
fn storagecontroller_seasontargets_render_the_live_targets() {
    let mut deck = Deck::compile("controls/relay/relay_generic.dss");
    assert_eq!(deck.get("StorageController.sc.Seasons"), "1");
    let (high, low) = (
        deck.get("StorageController.sc.kWTarget"),
        deck.get("StorageController.sc.kWTargetLow"),
    );
    assert_eq!((high.as_str(), low.as_str()), ("8000", "4000"));
    assert_eq!(
        deck.get("StorageController.sc.SeasonTargets"),
        format!("[ {high}]"),
        "the single-season target array IS the live kW target"
    );
    assert_eq!(
        deck.get("StorageController.sc.SeasonTargetsLow"),
        format!("[ {low}]")
    );
    deck.cmd("edit StorageController.sc seasons=2 seasontargets=[1000 2000]");
    assert_eq!(
        deck.get("StorageController.sc.SeasonTargets"),
        "[ 1000 2000]"
    );
}

// ---------------------------------------------------------------------------
// Pins for a ledger entry: r4133 never wires the property
// ---------------------------------------------------------------------------
//
// See the module doc's second section. These two are named by
// `props_r4133_replay::LEDGER_ENTRY_PINS`, not by an `EchoRow::witness`.

/// `swtcontrol.delay` is wired: a typed `Delay=` reaches the controller and
/// renders back. The property documents a delay, so ignoring the typed value
/// would be an OpenDSS bug.
#[test]
fn swtcontrol_delay_wires_the_property() {
    let mut deck = Deck::compile("controls/swtcontrol/swtcontrol_time.dss");
    assert_eq!(
        deck.get("SwtControl.sw.Delay"),
        "0.25",
        "the deck's own `~ delay=0.25`, which r4133 renders as 120"
    );
    deck.cmd("edit SwtControl.sw delay=7.5");
    assert_eq!(deck.get("SwtControl.sw.Delay"), "7.5");

    let mut unset =
        Deck::compile("electricdss-tst/Version8/Distrib/Examples/civinlar model/civanlar.dss");
    assert_eq!(
        unset.get("SwtControl.13_14.Delay"),
        "120",
        "an untyped Delay keeps the creation default — the same 120 r4133 prints, \
         which is why this deck has no cell in the census"
    );
}

/// `swtcontrol.delay` on the loop-tie deck renders the typed delay, the witness
/// of the per-case ledger entry for that deck.
#[test]
fn swtcontrol_delay_wires_the_property_on_the_midi_tie() {
    let mut deck = Deck::compile("controls/swtcontrol/midi_swtcontrol.dss");
    assert_eq!(deck.get("SwtControl.sw.Delay"), "0.25");
    deck.cmd("edit SwtControl.sw delay=3.5");
    assert_eq!(deck.get("SwtControl.sw.Delay"), "3.5");
}

// ---------------------------------------------------------------------------
// Pins for four ledger entries: r4133 reads the wrong live field
// ---------------------------------------------------------------------------
//
// r4133's `kvar` getter is wired and live, and it reads the *wrong* live field.
// These four are named by `props_r4133_replay::LEDGER_ENTRY_PINS`, not by an
// `EchoRow`.
//
// Each pin runs the gate's own sequence for its case — compile, then the one
// `solve` the case's `steps=1` rigor prescribes — and then reads the same
// `? Class.Name.Prop` getter the property walk reads. That solve is the pin's
// own, exactly as it is the gate's: the corpus gate's runner issues one `solve`
// per checkpoint whatever the deck ends on.
//
// The discriminating second reading is an `edit kvar=`, because a single
// reading alone would pass against a getter hardwired to the derived base: the
// edit proves the setter path drives the same getter.

/// `windgen.kvar` renders the base kvar the property documents, not the
/// dispatched reactive power, on the daily deck.
#[test]
fn windgen_kvar_renders_the_base_on_the_daily_deck() {
    let mut deck = Deck::compile("modes/windgen/windgen_daily.dss");
    deck.cmd("solve");
    assert_eq!(
        deck.get("WindGen.w1.kvar"),
        "986.05231553659",
        "kW=3000 pf=0.95 with kVA unset: kvar_base = kW*sqrt(1/pf^2-1), which r4133 renders as 0"
    );
    deck.cmd("edit WindGen.w1 kvar=777");
    assert_eq!(
        deck.get("WindGen.w1.kvar"),
        "777",
        "the typed base reads back — r4133 still answers 0 here"
    );
    assert_eq!(
        deck.get("WindGen.w1.PF"),
        "0.968057839822749",
        "Set_Presentkvar's side effect, 3000/sqrt(3000^2+777^2): r4133 renders this same PF from \
         the same stored kvarBase, which is what proves its `0` is a render bug and not a lost \
         parse"
    );
}

/// `windgen.kvar` renders the base kvar on the delta snapshot deck, the
/// witness of the per-case ledger entry for that deck.
#[test]
fn windgen_kvar_renders_the_base_on_the_delta_snapshot() {
    let mut deck = Deck::compile("modes/windgen/windgen_snap_delta.dss");
    deck.cmd("solve");
    assert_eq!(
        deck.get("WindGen.w1.kvar"),
        "726.483157256779",
        "kW=1500 pf=0.9 with kVA unset, which r4133 renders as 0"
    );
    deck.cmd("edit WindGen.w1 kvar=777");
    assert_eq!(deck.get("WindGen.w1.kvar"), "777");

    let mut clean = Deck::compile("modes/windgen/windgen_snap.dss");
    clean.cmd("solve");
    assert_eq!(
        clean.get("WindGen.w1.kvar"),
        "0",
        "pf=1.0 makes the base zero, which is the whole reason this deck carries no census cell"
    );
    clean.cmd("edit WindGen.w1 kvar=777");
    assert_eq!(
        clean.get("WindGen.w1.kvar"),
        "777",
        "…and it diverges the moment a base is typed — the deck is clean by value, not by mode"
    );
}

/// `windgen.kvar` renders the base kvar on the dynamics deck, where `kVA` is
/// set and the base derives from it.
#[test]
fn windgen_kvar_renders_the_base_on_the_dynamics_deck() {
    let mut deck = Deck::compile("modes/windgen/windgen_dyn.dss");
    deck.cmd("solve");
    assert_eq!(
        deck.get("WindGen.w1.kvar"),
        "854.95263026673",
        "kW=1500 kva=1800 at Create's pf=0.88: sqrt(kVA^2-(kVA*pf)^2), which r4133 renders as 0"
    );
    deck.cmd("edit WindGen.w1 kvar=777");
    assert_eq!(
        deck.get("WindGen.w1.PF"),
        "0.897802095552545",
        "1584/sqrt(1584^2+777^2) — r4133 reaches this same PF"
    );
    assert_eq!(
        deck.get("WindGen.w1.kvar"),
        "792.718441186736",
        "the kVA-set re-derivation at the new PF; r4133 holds the same kvarBase and renders 777"
    );
}

/// `windgen.kvar` renders the base kvar on the fault-ride-through deck, the
/// witness of the per-case ledger entry for that deck.
#[test]
fn windgen_kvar_renders_the_base_on_the_fault_ride_through_deck() {
    let mut deck = Deck::compile("modes/windgen/windgen_dyn_fault.dss");
    deck.cmd("solve");
    assert_eq!(
        deck.get("WindGen.w1.kvar"),
        "854.95263026673",
        "the same kVA-set derivation as the healthy dynamics deck, which r4133 renders as 0"
    );
    deck.cmd("edit WindGen.w1 kvar=777");
    assert_eq!(deck.get("WindGen.w1.kvar"), "792.718441186736");
}

// ---------------------------------------------------------------------------
// Pins for two ledger entries: r4133 derives `R2` from `%R1`
// ---------------------------------------------------------------------------
//
// Here the r4133 getter is wired, live AND reading the field the property
// documents, but the field itself is mis-derived: winding 2's conductance is
// built from the H-winding percentage. So the render is a live computation off
// a wrong number, not a parse-store echo, which is why the exclusion is a
// ledger entry and no `PROPS_ECHO_R4133` row. These two are named by
// `props_r4133_replay::LEDGER_ENTRY_PINS`.
//
// Neither deck needs a `solve`: `R2` is stamped at `Edit` time, and both decks'
// cells are `steps=1`. The reading is the same `? Class.Name.Prop` the gate's
// property walk takes.

/// `gictransformer.r2` derives from the X winding's own `%R2`, not from `%R1`,
/// on the micro deck. Each winding's resistance is that winding's percentage of
/// its own base.
#[test]
fn gictransformer_r2_honours_the_x_winding_percentage() {
    let mut deck = Deck::compile("asymmetric/gic/gictransformer_gic.dss");
    assert_eq!(
        deck.get("GICTransformer.tg3.R2"),
        "0.09522",
        "ZBase2*%R2/100 = 63.48*0.15/100, which both oracles render as 0.12696 (= ZBase2*%R1/100)"
    );
    assert_eq!(
        deck.get("GICTransformer.tg3.R1"),
        "0.7935",
        "the H winding is right on every engine — 396.75*0.2/100 — which is why the census has no \
         `gictransformer.r1` row at all"
    );
    assert_eq!(
        deck.get("GICTransformer.tg3.%R2"),
        "0.15",
        "property 14 renders the stored FpctR2 (GICTransformer.pas:729) and agrees with r4133: \
         only the DERIVED ohms diverge, which is what makes this a computed render and not an echo"
    );
    assert_eq!(
        deck.get("GICTransformer.tg2.R2"),
        "0.1",
        "the ohms-spec sibling takes the reverse branch (:497-498) neither revision got wrong"
    );
    assert_eq!(
        deck.get("GICTransformer.tg1.R2"),
        "0.38088",
        "a GSU that never types R2= keeps Create's %R2 = %R1 = 0.2 on the 138 kV / 100 MVA \
         defaults, so both engines land on the same number"
    );

    deck.cmd("edit GICTransformer.tg3 %R2=0.3");
    assert_eq!(
        deck.get("GICTransformer.tg3.R2"),
        "0.19044",
        "the setter path drives the same getter: 63.48*0.3/100"
    );
    deck.cmd("edit GICTransformer.tg3 %R1=0.4");
    assert_eq!(
        deck.get("GICTransformer.tg3.R2"),
        "0.19044",
        "the X winding does NOT follow %R1 — upstream would answer 63.48*0.4/100 = 0.25392 here, \
         which is the reading that separates the two mechanisms rather than two numbers"
    );
    assert_eq!(
        deck.get("GICTransformer.tg3.R1"),
        "1.587",
        "…while the H winding does follow it: 396.75*0.4/100"
    );
}

/// `gictransformer.r2` derives from `%R2` on the six-substation ring, the
/// witness of the per-case ledger entry for that deck.
#[test]
fn gictransformer_r2_honours_the_x_winding_percentage_on_the_ring() {
    let mut deck = Deck::compile("asymmetric/gic/gic_midi.dss");
    assert_eq!(
        deck.get("GICTransformer.tg5.R2"),
        "0.09522",
        "63.48*0.15/100 on tg5, which both oracles render as 0.12696"
    );
    assert_eq!(deck.get("GICTransformer.tg5.R1"), "0.7935");
    assert_eq!(
        deck.get("GICTransformer.tg5.%R2"),
        "0.15",
        "the stored percentage agrees with r4133 — only the derived ohms diverge"
    );
    assert_eq!(
        deck.get("GICTransformer.tg3.R2"),
        "0.1",
        "the ring's ohms-spec YY reads back its own token"
    );
    assert_eq!(deck.get("GICTransformer.tg1.R2"), "0.38088");

    deck.cmd("edit GICTransformer.tg5 %R2=0.3");
    assert_eq!(deck.get("GICTransformer.tg5.R2"), "0.19044");
    deck.cmd("edit GICTransformer.tg5 %R1=0.4");
    assert_eq!(
        deck.get("GICTransformer.tg5.R2"),
        "0.19044",
        "unmoved by %R1, where upstream would render 0.25392"
    );
    assert_eq!(
        deck.get("GICTransformer.tg5.R1"),
        "1.587",
        "…while the H winding does follow it: 396.75*0.4/100. This reading is what makes the \
         one above discriminating: without it, `R2` staying at 0.19044 is equally the answer of \
         a correct engine and of an `%R1=` edit that did nothing at all"
    );
}

// ---------------------------------------------------------------------------
// Pins for the five read-only results r4133 renders LIVE, and the capi capture
// that renders nothing
// ---------------------------------------------------------------------------
//
// These three belong to no `EchoRow` and to no `ledger.json` entry: the
// exclusion is a `SKIP_PROPS` + `SKIP_PROPS_CAPI_ONLY` row pair
// (`harness/mod.rs`, row group (g)), which masks the **capi** value compare of
// `IndMach012.PF` and `StorageController.kWhTotal`/`kWTotal`/`kWhActual`/
// `kWActual` while the r4133 channel keeps comparing all five. A deliberate
// divergence is excluded **and** pinned by an expected-value test, and the two
// engine-side pins below are that test: a regression back to `''` makes the
// capi compare AGREE again and the gate would go green on the wrong value.
// They are cited from `props_r4133_replay::RP38_SUPERSEDED`.
//
// Each reading is taken at the same point of the gate's own schedule
// (`clear` -> `compile` -> N x `solve`). This engine renders full precision
// (`float_to_str_ex`, like every other double property), so r4133's own
// `%.6g` / `%-.8g` digits are checked through `dss_core::util::fmt_g`.

/// `indmach012.pf` renders the live power factor of the solved machine, the
/// quantity its terminal power defines.
#[test]
fn indmach012_pf_renders_the_live_power_factor() {
    let mut asym = Deck::compile("asymmetric/indmach/indmach_asym.dss");
    let pre = asym.get("IndMach012.m1.PF");
    let v: f64 = pre
        .parse()
        .unwrap_or_else(|e| panic!("pf renders a number, got {pre:?}: {e}"));
    assert_eq!(
        dss_core::util::fmt_g(v, 6),
        "0.909167",
        "r4133's own byte on this deck after compile; 0.14.5 renders ''"
    );
    assert_eq!(
        pre, "0.909167177168387",
        "…and this engine prints the full double, not r4133's six digits"
    );

    // A second deck, a different machine: the getter is not a constant.
    let mut midi = Deck::compile("asymmetric/indmach/midi_indmach_asym.dss");
    let other = midi.get("IndMach012.m1.PF");
    let w: f64 = other
        .parse()
        .unwrap_or_else(|e| panic!("pf renders a number, got {other:?}: {e}"));
    assert_eq!(
        dss_core::util::fmt_g(w, 6),
        "0.904914",
        "r4133's own byte on the midi deck"
    );

    // …and it is live: the gate's first solve moves it. PORT-SOURCED literal —
    // see the doc: this is the gate's step-0 cell, and r4133's own step-0 byte
    // belongs to a trajectory its perturbing read moved.
    asym.cmd("solve");
    let step0 = asym.get("IndMach012.m1.PF");
    assert_ne!(step0, pre, "a solve must move a live power factor");
    assert_eq!(
        step0, "0.908915557341299",
        "the corpus gate's own step-0 cell on this case, which the capi skip row masks"
    );
}

/// The four fleet aggregates of StorageController (`kWhTotal`, `kWTotal`,
/// `kWhActual`, `kWActual`) render the live sums over the fleet.
#[test]
fn storagecontroller_fleet_aggregates_render_the_live_fleet() {
    let mut deck = Deck::compile("controls/storagecontroller/storagectrl_peakshave.dss");
    // The gate's own schedule on this case, and the probe's: after `compile`,
    // then after the first `solve` (r4133 §A pre-solve and step 0 both read
    // 12000 / 3000 / 9600 / 0 — the nameplates cannot move, and this deck's
    // first step neither charges nor discharges).
    let four = |deck: &mut Deck| {
        [
            deck.get("StorageController.sc.kWhTotal"),
            deck.get("StorageController.sc.kWTotal"),
            deck.get("StorageController.sc.kWhActual"),
            deck.get("StorageController.sc.kWActual"),
        ]
    };
    assert_eq!(four(&mut deck), ["12000", "3000", "9600", "0"]);
    deck.cmd("solve");
    assert_eq!(four(&mut deck), ["12000", "3000", "9600", "0"]);

    // r4133's own live-edit reading on this deck: 9999 + 6000
    // and 2222 + 1500, re-summed on the read.
    deck.cmd("edit storage.sa kwhrated=9999 kwrated=2222");
    assert_eq!(
        four(&mut deck),
        ["15999", "3722", "14799", "0"],
        "the totals are re-summed at every read, never latched — and the stored \
         energy follows the new nameplate (r4133's own four bytes after this very \
         edit)",
    );

    let mut tech = Deck::compile(
        "electricdss-tst/Version8/Distrib/Examples/StorageControllerTechNote/PeakShave/\
         PeakShaveRun.dss",
    );
    assert_eq!(
        tech.get("StorageController.SC.kWhTotal"),
        "7350",
        "a different fleet's nameplate total — the getter is not a constant"
    );
    assert_eq!(tech.get("StorageController.SC.kWTotal"), "1550");
    let eight = |deck: &mut Deck, prop: &str| -> String {
        let text = deck.get(&format!("StorageController.SC.{prop}"));
        let v: f64 = text
            .parse()
            .unwrap_or_else(|e| panic!("{prop} renders a number, got {text:?}: {e}"));
        dss_core::util::fmt_g(v, 8)
    };
    assert_eq!(
        eight(&mut tech, "kWhActual"),
        "2627.3806",
        "r4133's `%-.8g` after compile"
    );
    assert_eq!(eight(&mut tech, "kWActual"), "-18.81068");
    tech.cmd("solve");
    assert_eq!(
        eight(&mut tech, "kWhActual"),
        "2627.3929",
        "…and after the gate's own solve, r4133's step-0 byte"
    );
    assert_eq!(eight(&mut tech, "kWActual"), "-18.81068");
    assert_eq!(tech.get("StorageController.SC.kWhTotal"), "7350");
}

/// The committed 0.14.5 capture renders the five live read-only results as
/// `''` on every scenario, which is what the `SKIP_PROPS` rows for them state.
#[test]
fn the_silent_readonly_capture_cells_are_empty() {
    let root: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "props",
    ]
    .iter()
    .collect();
    let mut cells = 0usize;
    for (file, props, witness) in [
        ("indmach012.json", &["PF"][..], "kVA"),
        (
            "storagecontroller.json",
            &["kWhTotal", "kWTotal", "kWhActual", "kWActual"][..],
            "kWTarget",
        ),
    ] {
        let path = root.join(file);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let doc: serde_json::Value = serde_json::from_str(&text).expect("a props golden is JSON");
        let engine = doc["oracle"]["engine"].as_str().unwrap_or_default();
        assert!(
            engine.contains("version 0.14.5"),
            "{file}: this pin speaks for the 0.14.5 capture, got {engine:?}"
        );
        let scenarios = doc["scenarios"].as_array().expect("scenarios");
        assert!(!scenarios.is_empty(), "{file}: no scenario");
        for sc in scenarios {
            let name = sc["name"].as_str().unwrap_or_default();
            let bag = &sc["properties"];
            assert!(
                bag[witness].as_str().is_some_and(|v| !v.is_empty()),
                "{file}/{name}: the witness property {witness} is empty too — this capture \
                 proves nothing"
            );
            for prop in props {
                assert_eq!(
                    bag[*prop].as_str(),
                    Some(""),
                    "{file}/{name}: the 0.14.5 capture must render {prop} as '' — that is what \
                     the SKIP_PROPS group (g) row excludes"
                );
                cells += 1;
            }
        }
    }
    assert_eq!(
        cells, 21,
        "5 IndMach012 scenarios x PF + 4 StorageController scenarios x 4 aggregates"
    );
}

// ---------------------------------------------------------------------------
// The round-trip residue of `MakePosSequence`
// ---------------------------------------------------------------------------
//
// These pins hold this engine's value on the pairs the display floor refuses
// (`props_norm::display_is_render`): r4133's number is NOT our number rounded to
// the digits it printed, because the `%.Ng` round trip happens inside OpenDSS's
// `MakePosSequence`, which writes a command string of 5-significant-digit
// tokens and reads them back through its own edit, so everything derived
// afterwards is derived from the rounded input. This engine applies the same
// conversion as typed `f64` setters
// (`exec::make_pos_seq::apply::apply_pos_seq_actions`), keeps the exact value,
// and diverges by that round trip and nothing else.
//
// Each pin therefore names BOTH numbers — ours as a literal render of the live
// getter, r4133's as the vendored census literal — and *reproduces* r4133's from
// ours by running r4133's chain of arithmetic (`round_g`, below) and, where
// the engine can be made to walk it, by feeding this engine r4133's own token
// through `edit` and reading the same getter back. This engine is exact, so
// nothing is fixed and nothing is reproduced: the OpenDSS numbers are pinned as
// the *other* named literal, never emitted. Their citation list is
// `props_r4133_replay::RP39_PINS`.

/// FPC `Format('%-.<sig>g')` **as a value**: render `x` through the engine's own
/// FPC `%g` kernel ([`dss_core::util::fmt_g`], which both lanes share)
/// and read the decimal back — precisely what `MakePosSequence` does when it
/// prints a token and its own `Edit` parses it again.
fn round_g(x: f64, sig: usize) -> f64 {
    let text = dss_core::util::fmt_g(x, sig);
    text.parse()
        .unwrap_or_else(|e| panic!("{text:?} must re-parse as a number: {e}"))
}

/// FPC `Format('%-.<sig>g')` **as text** — what an r4133 getter prints when its
/// arm is `%-.5g` / `%-.6g` / `%-.8g` / `%.6g`, rendered by the same kernel
/// [`round_g`] parses back.
fn text_g(x: f64, sig: usize) -> String {
    dss_core::util::fmt_g(x, sig)
}

/// The port's own `?` render of a double — [`dss_core::util::float_to_str_ex`],
/// the function `obj/props/class_props/value.rs:49` calls for every
/// `PropType::Double`.
fn render(x: f64) -> String {
    dss_core::util::float_to_str_ex(x)
}

/// Pascal `Cabs(cmplx(re, im))` = `SQRT(re*re + im*im)` — the naive form, not
/// `hypot` (`Version8/Source/Shared/Ucomplex.pas:80-83`).
fn cabs(re: f64, im: f64) -> f64 {
    (re * re + im * im).sqrt()
}

/// A property render read back as a number.
fn number(deck: &mut Deck, target: &str) -> f64 {
    let text = deck.get(target);
    text.parse()
        .unwrap_or_else(|e| panic!("{target} must render a number, got {text:?}: {e}"))
}

/// The numbers inside the port's `[a, b, ]` array render.
fn numbers(text: &str) -> Vec<f64> {
    text.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse()
                .unwrap_or_else(|e| panic!("{s:?} must be a number: {e}"))
        })
        .collect()
}

/// The `%-.5g` tokens `MakePosSequence` writes for a list of values, in the
/// `a b c` shape its `( … )` array arguments take.
fn tokens5(values: &[f64]) -> String {
    values
        .iter()
        .map(|v| render(round_g(*v, 5)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `load.kva` after `MakePosSequence` is the exact conversion of the typed
/// values, not a value re-derived from five-digit tokens.
#[test]
fn load_kva_after_makeposseq_is_the_exact_typed_conversion() {
    // --- the repeat deck: r4133 reduces it twice, the port once -------------
    let mut pc = Deck::compile("modes/makeposseq/makeposseq_pc_repeat.dss");
    assert_eq!(
        pc.get("Load.ld_wye.kW"),
        render(400.0 / 3.0),
        "ours is the deck's 400 kW divided by three once: the second makeposseq changes \
         nothing on a positive-sequence circuit"
    );
    assert_eq!(pc.get("Load.ld_wye.kvar"), "43.8245473571818");
    assert_eq!(
        pc.get("Load.ld_wye.kVA"),
        "140.350877192982",
        "ours: the square root of the exact kW/3 and kvar/3"
    );
    // The load whose spec is xfkVA keeps its pre-conversion nameplate on both
    // engines (`PFChanged = FALSE`, so `:1160` is a no-op; ours: nominal.rs:335).
    assert_eq!(pc.get("Load.ld_delta.kVA"), "300");

    let kw = number(&mut pc, "Load.ld_wye.kW");
    let kvar = number(&mut pc, "Load.ld_wye.kvar");
    // r4133's first reduction writes `%-.5g` of a third, and its second
    // reduction of the reduced circuit writes `%-.5g` of a third of that token.
    let kw_t = round_g(round_g(kw, 5) / 3.0, 5);
    let kvar_t = round_g(round_g(kvar, 5) / 3.0, 5);
    assert_eq!(
        (render(kw_t), render(kvar_t)),
        ("44.443".to_string(), "14.608".to_string()),
        "the two tokens of r4133's second reduction, which its own `? kW` / `? kvar` echo back"
    );
    let kva = (kw_t * kw_t + kvar_t * kvar_t).sqrt(); // Load.pas:1145
    assert_eq!(
        render(kva),
        "46.7821965388544",
        "r4133's census cell, reproduced from OUR number by its own round trip"
    );

    // …and the port's own recalc walks the same chain when handed r4133's tokens.
    pc.cmd("edit Load.ld_wye kw=44.443 kvar=14.608");
    assert_eq!(pc.get("Load.ld_wye.kW"), "44.443", "the edit took effect");
    assert_eq!(pc.get("Load.ld_wye.kVA"), "46.7821965388544");
    // The discriminating second reading: one digit of one token, and the cell
    // moves — the getter is live, not a constant.
    pc.cmd("edit Load.ld_wye kw=44.444 kvar=14.608");
    assert_eq!(pc.get("Load.ld_wye.kVA"), "46.783146538043");

    // --- the once-converted PC deck -----------------------------------------
    let mut pc1 = Deck::compile("modes/makeposseq/makeposseq_pc.dss");
    assert_eq!(
        pc1.get("Load.ld_wye.kW"),
        render(400.0 / 3.0),
        "ours is the deck's 400 kW divided by three once, exactly"
    );
    assert_eq!(pc1.get("Load.ld_wye.kvar"), "43.8245473571818");
    assert_eq!(
        pc1.get("Load.ld_wye.kVA"),
        "140.350877192982",
        "ours: the SQRT of the exact kW/3 and kvar/3"
    );
    assert_eq!(pc1.get("Load.ld_delta.kVA"), "300");
    let kw_t = round_g(number(&mut pc1, "Load.ld_wye.kW"), 5);
    let kvar_t = round_g(number(&mut pc1, "Load.ld_wye.kvar"), 5);
    assert_eq!(
        (render(kw_t), render(kvar_t)),
        ("133.33".to_string(), "43.825".to_string()),
        "the two tokens r4133 writes on this deck"
    );
    assert_eq!(
        render((kw_t * kw_t + kvar_t * kvar_t).sqrt()),
        "140.347851871698",
        "r4133's reading on this deck, reproduced from OUR number by its own round trip"
    );
    pc1.cmd("edit Load.ld_wye kw=133.33 kvar=43.825");
    assert_eq!(pc1.get("Load.ld_wye.kW"), "133.33", "the edit took effect");
    assert_eq!(pc1.get("Load.ld_wye.kVA"), "140.347851871698");

    // --- a once-converted deck, two more cells ------------------------------
    let mut ctrl = Deck::compile("modes/makeposseq/makeposseq_ctrl.dss");
    assert_eq!(ctrl.get("Load.ld2.kVA"), "289.855072463768");
    let kw = round_g(number(&mut ctrl, "Load.ld2.kW"), 5);
    let kvar = round_g(number(&mut ctrl, "Load.ld2.kvar"), 5);
    assert_eq!(
        (render(kw), render(kvar)),
        ("266.67".to_string(), "113.6".to_string())
    );
    assert_eq!(
        render((kw * kw + kvar * kvar).sqrt()),
        "289.858325566129",
        "r4133's census cell"
    );

    assert_eq!(ctrl.get("Load.ld5.kVA"), "140.350877192982");
    let kw5 = round_g(number(&mut ctrl, "Load.ld5.kW"), 5);
    let kvar5 = round_g(number(&mut ctrl, "Load.ld5.kvar"), 5);
    assert_eq!(
        render((kw5 * kw5 + kvar5 * kvar5).sqrt()),
        "140.347851871698",
        "r4133's census cell on the second load of the same deck"
    );

    ctrl.cmd("edit Load.ld2 kw=266.67 kvar=113.6");
    assert_eq!(ctrl.get("Load.ld2.kW"), "266.67", "the edit took effect");
    assert_eq!(ctrl.get("Load.ld2.kVA"), "289.858325566129");
    ctrl.cmd("edit Load.ld2 kw=266.68 kvar=113.6");
    assert_eq!(ctrl.get("Load.ld2.kVA"), "289.86752560437");

    // --- the remaining ten vendored spellings of this pair ------------------
    // One row per (deck, load): the port's render and r4133's census literal,
    // the latter recomputed from the port's own live kW/kvar by the same
    // `%-.5g` round trip. r4133 reduces `makeposseq_pc_repeat.dss` twice and
    // the port once, so r4133's tokens there take the doubled chain from the
    // port's once state. The once-converted PC deck's rows carry r4133's
    // readings of that deck.
    for (deck_path, twice, loads) in [
        (
            "modes/makeposseq/makeposseq_line.dss",
            false,
            &[
                ("ld1", "175.438596491228", "175.441861769077"),
                ("ld2", "108.695652173913", "108.695722086934"),
            ][..],
        ),
        (
            "modes/makeposseq/makeposseq_report.dss",
            false,
            &[
                ("ld2", "326.086956521739", "326.087166260802"),
                ("ld3", "210.526315789474", "210.526371671104"),
                ("ld4", "44.4444444444444", "44.4444949234436"),
            ][..],
        ),
        (
            "modes/makeposseq/makeposseq_shunt.dss",
            false,
            &[("ld2", "105.263157894737", "105.26302971129")][..],
        ),
        (
            "modes/makeposseq/makeposseq_xfmr.dss",
            false,
            &[
                ("ld2", "70.1754385964912", "70.1756698079897"),
                ("ld4", "105.263157894737", "105.26302971129"),
                ("ld5", "7.01754385964912", "7.01756698079897"),
            ][..],
        ),
        (
            "modes/makeposseq/makeposseq_pc.dss",
            false,
            &[
                ("ld3", "70.1754385964912", "70.1756698079897"),
                ("ld_1ph", "28.0701754385965", "28.0704891480359"),
            ][..],
        ),
        (
            "modes/makeposseq/makeposseq_pc_repeat.dss",
            true,
            &[
                ("ld3", "70.1754385964912", "23.3915732690215"),
                ("ld_1ph", "28.0701754385965", "9.35681930786312"),
            ][..],
        ),
    ] {
        let mut deck = Deck::compile(deck_path);
        for (load, ours, theirs) in loads {
            assert_eq!(
                deck.get(&format!("Load.{load}.kVA")),
                *ours,
                "{deck_path} {load}: the port's render"
            );
            let kw = number(&mut deck, &format!("Load.{load}.kW"));
            let kvar = number(&mut deck, &format!("Load.{load}.kvar"));
            let (kw_t, kvar_t) = if twice {
                (
                    round_g(round_g(kw, 5) / 3.0, 5),
                    round_g(round_g(kvar, 5) / 3.0, 5),
                )
            } else {
                (round_g(kw, 5), round_g(kvar, 5))
            };
            assert_eq!(
                render((kw_t * kw_t + kvar_t * kvar_t).sqrt()),
                *theirs,
                "{deck_path} {load}: r4133's census cell, reproduced from OUR number by \
                 Load.pas:2326 -> :1145 -> :2352"
            );
            assert_ne!(
                render((kw_t * kw_t + kvar_t * kvar_t).sqrt()),
                *ours,
                "{deck_path} {load}: and the two engines really do part"
            );
        }
    }
}

/// `load.kw`, `load.kvar` and `load.xfkva` after `MakePosSequence` are the
/// exact conversion of the typed values, and a second conversion of a
/// positive-sequence circuit changes nothing.
#[test]
fn load_kw_kvar_and_xfkva_after_makeposseq_are_the_exact_typed_conversion() {
    let mut pc = Deck::compile("modes/makeposseq/makeposseq_pc_repeat.dss");

    // (a) the plain wye load: r4133's kW is the token itself.
    assert_eq!(
        pc.get("Load.ld_wye.kW"),
        render(400.0 / 3.0),
        "ours is the deck's 400 kW divided by three once: the second makeposseq changes \
         nothing on a positive-sequence circuit"
    );
    let kw = number(&mut pc, "Load.ld_wye.kW");
    assert_eq!(
        render(round_g(round_g(kw, 5) / 3.0, 5)),
        "44.443",
        "r4133's census cell = the token of its second reduction of the reduced circuit"
    );

    // (b) the delta load with an xfkVA nameplate.
    assert_eq!(pc.get("Load.ld_delta.xfkVA"), "166.666666666667");
    assert_eq!(pc.get("Load.ld_delta.kW"), "76.6666666666667");
    assert_eq!(pc.get("Load.ld_delta.kvar"), "32.659863237109");
    let xf = number(&mut pc, "Load.ld_delta.xfkVA");
    let alloc = number(&mut pc, "Load.ld_delta.allocationfactor");
    let pf = number(&mut pc, "Load.ld_delta.pf");
    assert_eq!(
        (alloc, pf),
        (0.5, 0.92),
        "the other two factors of :2200 — the default allocation factor and the deck's own pf, \
         which LoadSpecType 3 never recomputes"
    );
    let xf_t = round_g(round_g(xf, 5) / 3.0, 5);
    assert_eq!(
        render(xf_t),
        "55.557",
        "r4133's census cell: the token text of its second reduction, echoed as stored"
    );
    let kw_r4133 = xf_t * alloc * pf.abs(); // Load.pas:2200-2201
    let kvar_r4133 = kw_r4133 * (1.0 / (pf * pf) - 1.0).sqrt(); // :2202
    assert_eq!(
        (render(kw_r4133), render(kvar_r4133)),
        ("25.55622".to_string(), "10.8869041311844".to_string()),
        "r4133's two census cells, derived from the ROUNDED nameplate, not from their own tokens"
    );

    // The port's own side effect walks the same two formulas when handed
    // r4133's nameplate token.
    pc.cmd("edit Load.ld_delta xfkva=55.557");
    assert_eq!(
        pc.get("Load.ld_delta.xfkVA"),
        "55.557",
        "the edit took effect"
    );
    assert_eq!(pc.get("Load.ld_delta.kW"), "25.55622");
    assert_eq!(pc.get("Load.ld_delta.kvar"), "10.8869041311844");

    // Discriminating second readings: one digit on each of the three getters.
    pc.cmd("edit Load.ld_delta xfkva=55.558");
    assert_eq!(pc.get("Load.ld_delta.xfkVA"), "55.558");
    assert_eq!(pc.get("Load.ld_delta.kW"), "25.55668");
    assert_eq!(pc.get("Load.ld_delta.kvar"), "10.8871000903638");
    pc.cmd("edit Load.ld_wye kw=44.444");
    assert_eq!(pc.get("Load.ld_wye.kW"), "44.444");

    // The once-converted PC deck: r4133's state is the first conversion's token.
    let mut pc1 = Deck::compile("modes/makeposseq/makeposseq_pc.dss");
    assert_eq!(pc1.get("Load.ld_wye.kW"), "133.333333333333");
    let kw = number(&mut pc1, "Load.ld_wye.kW");
    assert_eq!(
        render(round_g(kw, 5)),
        "133.33",
        "r4133's reading = the conversion's own token"
    );
    assert_eq!(pc1.get("Load.ld_delta.xfkVA"), "166.666666666667");
    assert_eq!(pc1.get("Load.ld_delta.kW"), "76.6666666666667");
    assert_eq!(pc1.get("Load.ld_delta.kvar"), "32.659863237109");
    let xf_t = round_g(number(&mut pc1, "Load.ld_delta.xfkVA"), 5);
    let alloc = number(&mut pc1, "Load.ld_delta.allocationfactor");
    let pf = number(&mut pc1, "Load.ld_delta.pf");
    assert_eq!(render(xf_t), "166.67", "r4133's reading: the token text");
    let kw_r4133 = xf_t * alloc * pf.abs();
    let kvar_r4133 = kw_r4133 * (1.0 / (pf * pf) - 1.0).sqrt();
    assert_eq!(
        (render(kw_r4133), render(kvar_r4133)),
        ("76.6682".to_string(), "32.6605164343738".to_string()),
        "r4133's two readings, derived from the ROUNDED nameplate"
    );
    pc1.cmd("edit Load.ld_delta xfkva=166.67");
    assert_eq!(
        pc1.get("Load.ld_delta.xfkVA"),
        "166.67",
        "the edit took effect"
    );
    assert_eq!(pc1.get("Load.ld_delta.kW"), "76.6682");
    assert_eq!(pc1.get("Load.ld_delta.kvar"), "32.6605164343738");
}

/// `vsource.isc3` and `puz1` / `puz0` / `puz2` after `MakePosSequence` derive
/// from the full-precision `BasekV`, not a five-digit token of it.
#[test]
fn vsource_isc3_and_puz_after_makeposseq_use_the_full_precision_basekv() {
    let sqrt3 = 3.0_f64.sqrt(); // Common/DSSGlobals.pas:2033

    // --- once converted, 12.47 kV -------------------------------------------
    let mut line = Deck::compile("modes/makeposseq/makeposseq_line.dss");
    assert_eq!(line.get("Vsource.source.BasekV"), "7.19955785679463");
    assert_eq!(line.get("Vsource.source.Isc3"), "2520.34937016923");
    let ours = "[0.771699713635098, 3.08679885454039]";
    assert_eq!(line.get("Vsource.source.puZ1"), ours);
    assert_eq!(line.get("Vsource.source.puZ0"), ours, "R0 := R1 (:758-759)");
    assert_eq!(line.get("Vsource.source.puZ2"), ours, "R2 := R1 (:459-460)");

    let kv = number(&mut line, "Vsource.source.BasekV");
    let r1 = number(&mut line, "Vsource.source.R1");
    let x1 = number(&mut line, "Vsource.source.X1");
    assert_eq!(
        (r1, x1),
        (0.4, 1.6),
        "R1/X1 re-parse exactly through :1398-1399, so only the kV is rounded"
    );
    let kv5 = round_g(kv, 5); // Vsource.pas:1397 -> :360
    assert_eq!(render(kv5), "7.1996");
    let zbase = kv5 * kv5 / 100.0; // :473
    assert_eq!(
        dss_core::util::fmt_g(kv5 * 1000.0 / sqrt3 / cabs(r1, x1), 5),
        "2520.4",
        "r4133's census cell (:752 rendered by :1330)"
    );
    assert_eq!(
        format!(
            "[{}, {}]",
            dss_core::util::fmt_g(r1 / zbase, 8),
            dss_core::util::fmt_g(x1 / zbase, 8)
        ),
        "[0.77169068, 3.0867627]",
        "r4133's census cell, the same string on puZ1/puZ0/puZ2 (:837-842 -> :1340-1342)"
    );

    // Fed r4133's own kV, the port's derivations land on the doubles whose
    // `%-.5g` / `%-.8g` ARE those census cells.
    line.cmd("edit Vsource.source basekv=7.1996");
    assert_eq!(line.get("Vsource.source.BasekV"), "7.1996", "the edit took");
    assert_eq!(line.get("Vsource.source.Isc3"), "2520.36412324201");
    let theirs = "[0.771690679298655, 3.08676271719462]";
    assert_eq!(line.get("Vsource.source.puZ1"), theirs);
    assert_eq!(line.get("Vsource.source.puZ0"), theirs);
    assert_eq!(line.get("Vsource.source.puZ2"), theirs);
    let isc3 = number(&mut line, "Vsource.source.Isc3");
    assert_eq!(dss_core::util::fmt_g(isc3, 5), "2520.4");
    // Discriminating second reading.
    line.cmd("edit Vsource.source basekv=7.19");
    assert_eq!(line.get("Vsource.source.Isc3"), "2517.00345104035");
    assert_eq!(
        line.get("Vsource.source.puZ1"),
        "[0.773752758912181, 3.09501103564872]"
    );

    // --- once converted: the PC deck's two sources --------------------------
    let mut pc1 = Deck::compile("modes/makeposseq/makeposseq_pc.dss");
    assert_eq!(pc1.get("Vsource.v2.BasekV"), "7.19955785679463");
    assert_eq!(pc1.get("Vsource.v2.Isc3"), "1314.45341407666");
    assert_eq!(
        pc1.get("Vsource.v2.puZ1"),
        "[1.92924928408774, 5.78774785226323]"
    );
    assert_eq!(pc1.get("Vsource.source.Isc3"), "3360.4658268923");
    for (element, isc3_r4133, puz1_r4133) in [
        ("Vsource.v2", "1314.5", "[1.9292267, 5.7876801]"),
        ("Vsource.source", "3360.5", "[0.57876801, 2.315072]"),
    ] {
        let kv5 = round_g(number(&mut pc1, &format!("{element}.BasekV")), 5);
        assert_eq!(render(kv5), "7.1996", "{element}: r4133's kV token");
        let r1 = number(&mut pc1, &format!("{element}.R1"));
        let x1 = number(&mut pc1, &format!("{element}.X1"));
        let zbase = kv5 * kv5 / 100.0;
        assert_eq!(
            dss_core::util::fmt_g(kv5 * 1000.0 / sqrt3 / cabs(r1, x1), 5),
            isc3_r4133,
            "{element}: r4133's Isc3 on this deck"
        );
        assert_eq!(
            format!(
                "[{}, {}]",
                dss_core::util::fmt_g(r1 / zbase, 8),
                dss_core::util::fmt_g(x1 / zbase, 8)
            ),
            puz1_r4133,
            "{element}: r4133's puZ1 on this deck"
        );
    }

    // --- the repeat deck's second source: r4133 reduces it twice -----------
    let mut pc = Deck::compile("modes/makeposseq/makeposseq_pc_repeat.dss");
    assert_eq!(
        pc.get("Vsource.v2.BasekV"),
        "7.19955785679463",
        "ours: the second makeposseq changes nothing on a positive-sequence circuit"
    );
    assert_eq!(pc.get("Vsource.v2.Isc3"), "1314.45341407666");
    assert_eq!(
        pc.get("Vsource.v2.puZ1"),
        "[1.92924928408774, 5.78774785226323]"
    );
    let kv = number(&mut pc, "Vsource.v2.BasekV");
    let kv5 = round_g(round_g(kv, 5) / sqrt3, 5);
    assert_eq!(
        kv5,
        round_g(round_g(12.47 / sqrt3, 5) / sqrt3, 5),
        "r4133 rounds once per reduction and reduces this circuit twice"
    );
    assert_eq!(render(kv5), "4.1567");
    let (r1, x1) = (
        number(&mut pc, "Vsource.v2.R1"),
        number(&mut pc, "Vsource.v2.X1"),
    );
    assert_eq!((r1, x1), (1.0, 3.0));
    let zbase = kv5 * kv5 / 100.0;
    assert_eq!(
        dss_core::util::fmt_g(kv5 * 1000.0 / sqrt3 / cabs(r1, x1), 5),
        "758.91",
        "r4133's census cell"
    );
    assert_eq!(
        format!(
            "[{}, {}]",
            dss_core::util::fmt_g(r1 / zbase, 8),
            dss_core::util::fmt_g(x1 / zbase, 8)
        ),
        "[5.787655, 17.362965]",
        "r4133's census cell"
    );
    pc.cmd("edit Vsource.v2 basekv=4.1567");
    assert_eq!(pc.get("Vsource.v2.Isc3"), "758.906118260575");
    assert_eq!(
        pc.get("Vsource.v2.puZ1"),
        "[5.78765502662999, 17.36296507989]"
    );
    pc.cmd("edit Vsource.v2 basekv=4.16");
    assert_eq!(pc.get("Vsource.v2.Isc3"), "759.50861307383");

    // --- the two remaining puZ spellings ------------------------------------
    // Same chain, the other two sources of the population: the repeat deck's
    // own circuit source (reduced twice by r4133 and once by the port, R1/X1 =
    // 0.3/1.2) and the once-converted shunt deck's. r4133's kV tokens are
    // 4.1567 and 7.1996.
    for (label, deck, twice, kv5_text, ours, theirs) in [
        (
            "pc_repeat/source",
            &mut pc,
            true,
            "4.1567",
            "[0.578774785226323, 2.31509914090529]",
            "[1.7362965, 6.945186]",
        ),
        (
            "shunt/source",
            &mut Deck::compile("modes/makeposseq/makeposseq_shunt.dss"),
            false,
            "7.1996",
            "[0.578774785226323, 2.31509914090529]",
            "[0.57876801, 2.315072]",
        ),
    ] {
        assert_eq!(deck.get("Vsource.source.puZ1"), ours, "{label}: ours");
        assert_eq!(deck.get("Vsource.source.puZ0"), ours, "{label}: R0 := R1");
        assert_eq!(deck.get("Vsource.source.puZ2"), ours, "{label}: R2 := R1");
        let kv5 = round_g(number(deck, "Vsource.source.BasekV"), 5);
        let kv5 = if twice { round_g(kv5 / sqrt3, 5) } else { kv5 };
        assert_eq!(render(kv5), kv5_text, "{label}: r4133's kV token");
        let (r1, x1) = (
            number(deck, "Vsource.source.R1"),
            number(deck, "Vsource.source.X1"),
        );
        assert_eq!((r1, x1), (0.3, 1.2), "{label}: R1/X1 re-parse exactly");
        let zbase = kv5 * kv5 / 100.0; // :473
        assert_eq!(
            format!(
                "[{}, {}]",
                dss_core::util::fmt_g(r1 / zbase, 8),
                dss_core::util::fmt_g(x1 / zbase, 8)
            ),
            theirs,
            "{label}: r4133's census cell, reproduced from OUR kV (:837-842 -> :1340-1342)"
        );
    }

    // --- the 115 kV trio (ctrl/report/xfmr share one spelling) --------------
    let mut ctrl = Deck::compile("modes/makeposseq/makeposseq_ctrl.dss");
    assert_eq!(ctrl.get("Vsource.source.Isc3"), "19071.5461456915");
    let kv5 = round_g(number(&mut ctrl, "Vsource.source.BasekV"), 5);
    assert_eq!(render(kv5), "66.395");
    let (r1, x1) = (
        number(&mut ctrl, "Vsource.source.R1"),
        number(&mut ctrl, "Vsource.source.X1"),
    );
    assert_eq!(
        dss_core::util::fmt_g(kv5 * 1000.0 / sqrt3 / cabs(r1, x1), 5),
        "19071",
        "r4133's census cell — and note it is NOT our own number's 5-digit render, which \
         rounds the other way"
    );
    assert_eq!(
        dss_core::util::fmt_g(number(&mut ctrl, "Vsource.source.Isc3"), 5),
        "19072",
        "…the very reason the display floor's mechanism clause refuses this cell"
    );
}

/// `vsource.mvasc3` and `mvasc1` after `MakePosSequence` derive from the
/// full-precision `BasekV`.
#[test]
fn vsource_mvasc1_and_mvasc3_after_makeposseq_use_the_full_precision_basekv() {
    let sqrt3 = 3.0_f64.sqrt();

    // --- once converted -----------------------------------------------------
    let mut pc1 = Deck::compile("modes/makeposseq/makeposseq_pc.dss");
    for (element, ours, r4133) in [
        ("Vsource.source", "41.905008861347", "41.905"),
        ("Vsource.v2", "16.3912340735359", "16.391"),
    ] {
        assert_eq!(pc1.get(&format!("{element}.MVAsc3")), ours);
        assert_eq!(pc1.get(&format!("{element}.MVAsc1")), ours);
        let kv5 = round_g(number(&mut pc1, &format!("{element}.BasekV")), 5);
        assert_eq!(
            render(kv5),
            "7.1996",
            "{element}: the kV token of one conversion"
        );
        let r1 = number(&mut pc1, &format!("{element}.R1"));
        let x1 = number(&mut pc1, &format!("{element}.X1"));
        let (r0, x0) = (r1, x1);
        let isc3 = kv5 * 1000.0 / sqrt3 / cabs(r1, x1);
        let isc1 = kv5 * 1000.0 / 1.0 / cabs((2.0 * r1 + r0) / 3.0, (2.0 * x1 + x0) / 3.0);
        assert_eq!(
            dss_core::util::fmt_g(sqrt3 * kv5 * isc3 / 1000.0, 5),
            r4133,
            "{element}: r4133's MVAsc3 on this deck"
        );
        assert_eq!(
            dss_core::util::fmt_g(1.0 * kv5 * isc1 / 1000.0, 5),
            r4133,
            "{element}: r4133's MVAsc1 on this deck"
        );
    }

    // --- the repeat deck: r4133 reduces it twice, the port once -------------
    let mut pc = Deck::compile("modes/makeposseq/makeposseq_pc_repeat.dss");

    for (element, ours, r4133, after_edit) in [
        (
            "Vsource.source",
            "41.905008861347",
            "13.969",
            "13.9685603190691",
        ),
        (
            "Vsource.v2",
            "16.3912340735359",
            "5.4638",
            "5.4638323217576",
        ),
    ] {
        assert_eq!(pc.get(&format!("{element}.MVAsc3")), ours);
        assert_eq!(
            pc.get(&format!("{element}.MVAsc1")),
            ours,
            "a 1-phase source forces Z0 = Z1 (:758-759) and Factor = 1 (:688)"
        );
        let kv5 = round_g(
            round_g(number(&mut pc, &format!("{element}.BasekV")), 5) / sqrt3,
            5,
        );
        assert_eq!(
            render(kv5),
            "4.1567",
            "r4133's kV token of its second reduction"
        );
        let r1 = number(&mut pc, &format!("{element}.R1"));
        let x1 = number(&mut pc, &format!("{element}.X1"));
        let (r0, x0) = (r1, x1); // Vsource.pas:758-759, Fnphases = 1
        let isc3 = kv5 * 1000.0 / sqrt3 / cabs(r1, x1); // :752
        let isc1 = kv5 * 1000.0 / 1.0 / cabs((2.0 * r1 + r0) / 3.0, (2.0 * x1 + x0) / 3.0); // :766
        assert_eq!(
            dss_core::util::fmt_g(sqrt3 * kv5 * isc3 / 1000.0, 5),
            r4133,
            "r4133's MVAsc3 census cell (:767 -> :1328)"
        );
        assert_eq!(
            dss_core::util::fmt_g(1.0 * kv5 * isc1 / 1000.0, 5),
            r4133,
            "r4133's MVAsc1 census cell (:768 -> :1329)"
        );

        // Fed r4133's own kV token, we land on the double behind those bytes.
        pc.cmd(&format!("edit {element} basekv=4.1567"));
        assert_eq!(pc.get(&format!("{element}.BasekV")), "4.1567");
        assert_eq!(pc.get(&format!("{element}.MVAsc3")), after_edit);
        assert_eq!(pc.get(&format!("{element}.MVAsc1")), after_edit);
        assert_eq!(
            dss_core::util::fmt_g(number(&mut pc, &format!("{element}.MVAsc3")), 5),
            r4133
        );
    }

    // Discriminating second reading on the same getters.
    pc.cmd("edit Vsource.v2 basekv=4.16");
    assert_eq!(pc.get("Vsource.v2.MVAsc3"), "5.47251122758099");
    assert_eq!(pc.get("Vsource.v2.MVAsc1"), "5.47251122758099");
}

/// `line.b1` and `line.b0` after `MakePosSequence` derive from the
/// full-precision `C1`, not a five-digit token of it.
#[test]
fn line_b1_and_b0_after_makeposseq_use_the_full_precision_c1() {
    let mut deck = Deck::compile("modes/makeposseq/makeposseq_line.dss");
    let two_pi = 2.0 * std::f64::consts::PI; // Common/DSSGlobals.pas:99 `TwoPi = 2.0 * PI`

    for (name, ours_c1, ours_b, ours_b7, ours_b13, their_c1, their_b) in [
        (
            "l_geo",
            "10.8400705196377",
            "4.08661030906671",
            "4.08661",
            "4.086610309067",
            "10.84",
            "4.086584",
        ),
        (
            "l_spc",
            "10.8415607417247",
            "4.08717210955796",
            "4.087172",
            "4.087172109558",
            "10.842",
            "4.087338",
        ),
    ] {
        assert_eq!(deck.get(&format!("Line.{name}.c1")), ours_c1);
        assert_eq!(deck.get(&format!("Line.{name}.b1")), ours_b);
        assert_eq!(
            deck.get(&format!("Line.{name}.b0")),
            ours_b,
            "C0 := C1 for a 1-phase line (Line.pas:971)"
        );
        let c1 = number(&mut deck, &format!("Line.{name}.c1"));
        let b = number(&mut deck, &format!("Line.{name}.b1"));
        assert_eq!(
            dss_core::util::fmt_g(b, 7),
            ours_b7,
            "ours at r4133's OWN print precision — the two engines already part at the 6th \
             significant digit, which is why the display floor's mechanism clause refuses \
             these cells"
        );
        assert_ne!(dss_core::util::fmt_g(b, 7), their_b);
        // Both sides of the recomputation below are recorded literals, so it
        // is the census's number that is asserted, not the port against itself.
        assert_eq!(
            dss_core::util::fmt_g(b, 13),
            ours_b13,
            "ours at the width where the recomputation can meet it"
        );
        assert_eq!(
            dss_core::util::fmt_g(two_pi * 60.0 * c1 * 1.0e-3, 13),
            ours_b13,
            "our b tracks the UNROUNDED C1: the :1406 expression recomputed from the (already \
             15-digit-truncated) c1 render lands back on it — 2 ulp away, which is why the \
             width is 13 — where r4133's input was 5 digits"
        );
        let c1_token = round_g(c1, 5); // Line.pas:1591
        assert_eq!(
            render(c1_token),
            their_c1,
            "the token r4133's own `? c1` echoes back"
        );
        assert_eq!(
            dss_core::util::fmt_g(two_pi * 60.0 * (c1_token * 1.0e-9) * 1.0e6 / 1.0, 7),
            their_b,
            "r4133's census cell, on b1 and b0 alike (:611 -> :1406/:1407)"
        );
    }

    // Handed the token r4133's `Edit` received, our own getter answers r4133's
    // number — and `b0` follows only at the next `RecalcElementData` (`:971`),
    // exactly as upstream.
    deck.cmd("edit Line.l_geo c1=10.84");
    assert_eq!(deck.get("Line.l_geo.c1"), "10.84", "the edit took effect");
    assert_eq!(deck.get("Line.l_geo.b1"), "4.0865837237896");
    assert_eq!(
        dss_core::util::fmt_g(number(&mut deck, "Line.l_geo.b1"), 7),
        "4.086584"
    );
    assert_eq!(
        deck.get("Line.l_geo.b0"),
        "4.08661030906671",
        "C0 is still the converted one until the element is recalculated"
    );
    deck.cmd("solve");
    assert_eq!(deck.get("Line.l_geo.b0"), "4.0865837237896");

    // Discriminating readings: the sibling lines did not move, and the
    // symcomponents line — whose conversion keeps its typed C1 (`:1560-1563`) — is a
    // plain render on both engines.
    assert_eq!(deck.get("Line.l_spc.b1"), "4.08717210955796");
    assert_eq!(deck.get("Line.l_sym.c1"), "3.4");
    assert_eq!(deck.get("Line.l_sym.b1"), "1.28176980266464");
}

/// `autotrans.wdgcurrents` after `MakePosSequence` are the solved currents of
/// the exactly converted circuit.
#[test]
fn autotrans_wdgcurrents_after_makeposseq_solve_the_exactly_converted_circuit() {
    let mut deck = Deck::compile("modes/makeposseq/makeposseq_xfmr.dss");
    assert_eq!(
        deck.get("AutoTrans.at.WdgCurrents"),
        "44.00113, (161.26), 29.32232, (161.26), "
    );
    deck.cmd("solve");
    assert_eq!(
        deck.get("AutoTrans.at.WdgCurrents"),
        "44.00102, (161.26), 29.32224, (161.26), ",
        "ours, at the census's own step-0 state (compile + one solve)"
    );

    // r4133's conversion of the same circuit, token for token, computed from the
    // port's own post-conversion doubles.
    let vs = tokens5(&[number(&mut deck, "Vsource.source.BasekV")]);
    assert_eq!(vs, "66.395", "VSource.pas:1397");
    let mut edits = vec![format!("edit Vsource.source basekv={vs}")];
    for name in ["sub", "t3"] {
        let kvs = tokens5(&numbers(&deck.get(&format!("Transformer.{name}.kVs"))));
        let kvas = tokens5(&numbers(&deck.get(&format!("Transformer.{name}.kVAs"))));
        let norm = tokens5(&[number(&mut deck, &format!("Transformer.{name}.NormHkVA"))]);
        let emerg = tokens5(&[number(&mut deck, &format!("Transformer.{name}.EmergHkVA"))]);
        edits.push(format!(
            "edit Transformer.{name} kvs=({kvs}) kvas=({kvas}) normhkva={norm} emerghkva={emerg}"
        ));
    }
    let kvs = tokens5(&numbers(&deck.get("AutoTrans.at.kVs")));
    let kvas = tokens5(&numbers(&deck.get("AutoTrans.at.kVAs")));
    let norm = tokens5(&[number(&mut deck, "AutoTrans.at.NormHkVA")]);
    let emerg = tokens5(&[number(&mut deck, "AutoTrans.at.EmergHkVA")]);
    assert_eq!(
        (kvs.as_str(), kvas.as_str(), norm.as_str()),
        ("2.4018 7.1996", "666.67 666.67", "733.33"),
        "AutoTrans.pas:2021 (kVLL/SQRT3), :2027 (kVA/FNPhases), :2030 (NormMaxHkVA/FNPhases)"
    );
    let auto_edit =
        format!("edit AutoTrans.at kvs=({kvs}) kvas=({kvas}) normhkva={norm} emerghkva={emerg}");
    edits.push(auto_edit.clone());
    for load in ["ld2", "ld4", "ld5"] {
        let kv = tokens5(&[number(&mut deck, &format!("Load.{load}.kV"))]);
        let kw = tokens5(&[number(&mut deck, &format!("Load.{load}.kW"))]);
        let kvar = tokens5(&[number(&mut deck, &format!("Load.{load}.kvar"))]);
        edits.push(format!("edit Load.{load} kv={kv} kw={kw} kvar={kvar}"));
    }

    let mut mimic = Deck::compile("modes/makeposseq/makeposseq_xfmr.dss");
    for edit in &edits {
        mimic.cmd(edit);
    }
    mimic.cmd("solve");
    mimic.cmd("solve");
    assert_eq!(
        mimic.get("AutoTrans.at.kVs"),
        "[2.4018, 7.1996, ]",
        "the edits took effect"
    );
    assert_eq!(
        mimic.get("AutoTrans.at.WdgCurrents"),
        "44.00054, (161.26), 29.32187, (161.26), ",
        "r4133's census cell, byte for byte, reproduced from OUR state by ITS round trip"
    );

    // Discriminating reading: this element's own conversion is not the cause.
    let mut alone = Deck::compile("modes/makeposseq/makeposseq_xfmr.dss");
    alone.cmd(&auto_edit);
    alone.cmd("solve");
    alone.cmd("solve");
    assert_eq!(
        alone.get("AutoTrans.at.WdgCurrents"),
        "44.00086, (161.26), 29.32208, (161.26), ",
        "the AutoTrans-only round trip lands on neither engine's number — the divergence is \
         the deck-wide %-.5g scripting"
    );
}

/// `autotrans.wdgcurrents` on the `controls:autotrans/*` decks are those of the
/// regulated circuit: the RegControl taps the AutoTrans, as its help documents.
/// The winding-current ratio follows the ampere-turn balance at the tap.
#[test]
fn autotrans_wdgcurrents_stay_regulated_where_r4133_never_taps_the_autotrans() {
    /// `|I_common| / |I_series|` off a `WdgCurrents` render: the first and third
    /// tokens are winding 1's and winding 2's magnitudes on phase 1
    /// (`AutoTrans.pas:1682` prints `mag, (ang), ` per winding, near end only).
    fn common_over_series(render: &str) -> f64 {
        let t: Vec<&str> = render.split(',').map(str::trim).collect();
        let series: f64 = t[0].parse().expect("winding 1 magnitude");
        let common: f64 = t[2].parse().expect("winding 2 magnitude");
        common / series
    }

    /// The ampere-turn prediction for that ratio at `tap_c` on the common
    /// winding — the √3-cancelled form derived in the doc above.
    fn ampere_turns(kv1: f64, kv2: f64, tap_c: f64) -> f64 {
        (kv1 - kv2) / (kv2 * tap_c)
    }

    fn leg(
        rel: &str,
        (kv1, kv2): (f64, f64),
        (regulated, taps, tapnum): (&str, &str, &str),
        unregulated: &str,
    ) {
        let mut deck = Deck::compile(rel);

        // (a) the port's regulated state — which is also the pinned capi_v0145
        // oracle's: the regulator moves the common winding and holds the bus.
        assert_eq!(deck.get("AutoTrans.at.WdgCurrents"), regulated, "{rel}");
        assert_eq!(deck.get("AutoTrans.at.Taps"), taps, "{rel}");
        assert_eq!(deck.get("RegControl.rat.TapNum"), tapnum, "{rel}");
        let tap_c: f64 = deck.get("AutoTrans.at.Tap").parse().expect("a tap");
        // Read the LIVE render, never the pinned literal: the `assert_eq!` above
        // proves the two are the same string on a green tree, and taking the
        // engine's own bytes is what makes this a physics reading of this
        // engine rather than arithmetic on a constant.
        let live = deck.get("AutoTrans.at.WdgCurrents");
        let (m, p) = (common_over_series(&live), ampere_turns(kv1, kv2, tap_c));
        assert!(
            (m - p).abs() / p < 1.0e-5,
            "{rel}: regulated ampere-turn balance {m} vs {p}, against the render's %.7g magnitudes"
        );

        // (b) the one input that differs, and the proof each edit landed.
        deck.cmd("edit RegControl.rat enabled=no");
        assert_eq!(
            deck.get("RegControl.rat.enabled"),
            "No",
            "{rel}: the regulator really is out of the loop"
        );
        deck.cmd("edit AutoTrans.at wdg=2 tap=1");
        assert_eq!(
            deck.get("AutoTrans.at.Taps"),
            "[1, 1, ]",
            "{rel}: the tap the regulator had already pushed is back where the deck started"
        );
        // The deck's own last two lines, re-run: `Set mode=daily` puts the clock
        // back to hour 0, so this is the same 8-step sequence over the same load
        // shape with the regulator silent.
        deck.cmd("set mode=daily stepsize=1h number=8");
        deck.cmd("solve");
        assert_eq!(
            deck.get("AutoTrans.at.Taps"),
            "[1, 1, ]",
            "{rel}: nothing tapped it — exactly what r4133 does with the regulator ENABLED"
        );
        assert_eq!(
            deck.get("AutoTrans.at.WdgCurrents"),
            unregulated,
            // "byte for byte" is enforced, not asserted in prose:
            // `props_r4133_replay::the_rp312_pin_quotes_the_vendored_census_cells`
            // holds this literal (and the regulated one above) against the
            // vendored `examples_supplement.txt` rows.
            "{rel}: r4133's census cell, byte for byte, from OUR state with the tap it never moves"
        );
        let live = deck.get("AutoTrans.at.WdgCurrents");
        let tap_c: f64 = deck.get("AutoTrans.at.Tap").parse().expect("a tap");
        let (m, p) = (common_over_series(&live), ampere_turns(kv1, kv2, tap_c));
        assert!(
            (m - p).abs() / p < 1.0e-5,
            "{rel}: unregulated ampere-turn balance {m} vs {p}; r4133 derives its winding currents correctly, it just \
             derives them for a tap it never moved"
        );
    }

    leg(
        "controls/autotrans/autotrans_reg.dss",
        (115.0, 34.5),
        (
            "66.95905, (-28.006), 151.5029, (151.99), 66.95905, (-148.01), 151.5029, (31.995), \
             66.95905, (91.994), 151.5029, (-88.005), ",
            "[1, 1.03125, ]",
            "5",
        ),
        "66.99186, (-28.029), 156.314, (151.97), 66.99186, (-148.03), 156.314, (31.971), \
         66.99186, (91.971), 156.314, (-88.029), ",
    );
    leg(
        "controls/autotrans/midi_autotrans.dss",
        (115.0, 69.0),
        (
            "117.2108, (-28.743), 73.11371, (151.26), 117.2091, (-148.75), 73.11263, (31.253), \
             117.2191, (91.253), 73.11886, (-88.746), ",
            "[1, 1.06875, ]",
            "11",
        ),
        "117.2323, (-28.754), 78.15456, (151.25), 117.2305, (-148.76), 78.15342, (31.241), \
         117.2405, (91.242), 78.16007, (-88.757), ",
    );
}

/// `reactor.x`, `z`, `lmh`, `normamps` and `emergamps` after `MakePosSequence`
/// are the exact conversion of the typed values.
#[test]
fn reactor_amps_after_makeposseq_are_the_exact_typed_conversion() {
    let mut deck = Deck::compile("modes/makeposseq/makeposseq_shunt.dss");

    // The port's post-conversion nameplate: 12.47/SQRT3 and 200/3, exactly.
    assert_eq!(deck.get("Reactor.rx_kvar.phases"), "1");
    assert_eq!(
        deck.get("Reactor.rx_kvar.kv"),
        render(12.47 / 3f64.sqrt()),
        "ours is Reactor.pas:1159's argument applied as a typed setter"
    );
    assert_eq!(deck.get("Reactor.rx_kvar.kvar"), render(200.0 / 3.0));
    // …and the five cells derived from it (the census `rust` spellings).
    assert_eq!(deck.get("Reactor.rx_kvar.normamps"), "9.25982789397956");
    assert_eq!(deck.get("Reactor.rx_kvar.emergamps"), "12.5007676568724");
    assert_eq!(deck.get("Reactor.rx_kvar.X"), "777.5045");
    assert_eq!(deck.get("Reactor.rx_kvar.Z"), "[0, 777.5045]");
    assert_eq!(deck.get("Reactor.rx_kvar.LmH"), "2062.39474085321");

    // r4133's two tokens, computed from OUR state (Reactor.pas:1162).
    let kv5 = round_g(number(&mut deck, "Reactor.rx_kvar.kv"), 5);
    let kvar5 = round_g(number(&mut deck, "Reactor.rx_kvar.kvar"), 5);
    assert_eq!(
        (render(kv5), render(kvar5)),
        ("7.1996".to_string(), "66.667".to_string()),
        "the two tokens r4133 wrote, which its own `? kv` / `? kvar` echo back"
    );
    // …and its RecalcElementData on them, through its own getter widths.
    let x = (kv5 * kv5) * 1000.0 / kvar5; // Reactor.pas:670
    assert_eq!(
        text_g(x, 8),
        "777.50971",
        "r4133's census cell, reproduced from OUR number by its own round trip (:1092)"
    );
    assert_eq!(
        format!("[0, {}]", text_g(x, 8)),
        "[0, 777.50971]",
        "the same X through the Z getter (:1097)"
    );
    let l = x / (2.0 * std::f64::consts::PI) / 60.0; // Reactor.pas:671
    assert_eq!(
        text_g(l * 1000.0, 8),
        "2062.4086",
        "r4133's census cell (:1098); ours renders 2062.3947 at that width, so this is no \
         %.Ng print of our value"
    );
    let norm = kvar5 / kv5; // Reactor.pas:674
    assert_eq!(
        render(norm),
        "9.25981998999944",
        "r4133's census cell (:1099)"
    );
    assert_eq!(
        render(norm * 1.35),
        "12.5007569864993",
        "r4133's census cell (:1100)"
    );

    // …and the port's own recalc walks the chain when handed r4133's tokens.
    deck.cmd("edit Reactor.rx_kvar phases=1 kv=7.1996 kvar=66.667");
    assert_eq!(
        deck.get("Reactor.rx_kvar.kv"),
        "7.1996",
        "the edit took effect"
    );
    assert_eq!(deck.get("Reactor.rx_kvar.normamps"), "9.25981998999944");
    assert_eq!(deck.get("Reactor.rx_kvar.emergamps"), "12.5007569864993");
    assert_eq!(deck.get("Reactor.rx_kvar.X"), "777.509714851426");
    assert_eq!(deck.get("Reactor.rx_kvar.LmH"), "2062.40857367624");
    // The discriminating second reading: the exact nameplate back, and every
    // cell returns — the getters are live, not five constants.
    deck.cmd("edit Reactor.rx_kvar phases=3 kv=12.47 kvar=200");
    assert_eq!(
        deck.get("Reactor.rx_kvar.kv"),
        "12.47",
        "the edit took effect"
    );
    assert_eq!(deck.get("Reactor.rx_kvar.normamps"), "9.25982789397956");
    assert_eq!(deck.get("Reactor.rx_kvar.emergamps"), "12.5007676568724");
    assert_eq!(deck.get("Reactor.rx_kvar.X"), "777.5045");
    assert_eq!(deck.get("Reactor.rx_kvar.LmH"), "2062.39474085321");
}

/// `capacitor.cuf`, `normamps` and `emergamps` after `MakePosSequence` are the
/// exact conversion of the typed values.
#[test]
fn capacitor_cuf_and_amps_after_makeposseq_are_the_exact_typed_conversion() {
    let mut deck = Deck::compile("modes/makeposseq/makeposseq_shunt.dss");

    assert_eq!(deck.get("Capacitor.cap_kvar.phases"), "1");
    assert_eq!(
        deck.get("Capacitor.cap_kvar.kv"),
        render(12.47 / 3f64.sqrt()),
        "ours is Capacitor.pas:798's argument applied as a typed setter"
    );
    assert_eq!(deck.get("Capacitor.cap_kvar.kvar"), "[ 200]");
    assert_eq!(deck.get("Capacitor.cap_kvar.cuf"), "[ 10.2349853339688]");
    assert_eq!(deck.get("Capacitor.cap_kvar.normamps"), "37.5023029706172");
    assert_eq!(deck.get("Capacitor.cap_kvar.emergamps"), "50.0030706274896");

    // The three cells repeat verbatim on the two decks whose `cap1` carries the
    // same typed line — 9 census cells over one spelling each.
    let mut ctrl = Deck::compile("modes/makeposseq/makeposseq_ctrl.dss");
    assert_eq!(ctrl.get("Capacitor.cap1.cuf"), "[ 10.2349853339688]");
    assert_eq!(ctrl.get("Capacitor.cap1.normamps"), "37.5023029706172");
    assert_eq!(ctrl.get("Capacitor.cap1.emergamps"), "50.0030706274896");

    // r4133's tokens, computed from OUR state (Capacitor.pas:801, :806).
    let kv5 = round_g(number(&mut deck, "Capacitor.cap_kvar.kv"), 5);
    let kvar5 = round_g(numbers(&deck.get("Capacitor.cap_kvar.kvar"))[0], 5);
    assert_eq!(
        (render(kv5), render(kvar5)),
        ("7.1996".to_string(), "200".to_string()),
        "the kvar token is five-digit exact — the whole loss is in kV"
    );
    let w = 2.0 * std::f64::consts::PI * 60.0;
    let fc = 1.0 / (w * (kv5 * kv5) * 1000.0 / (kvar5 / 1.0)); // Capacitor.pas:611
    assert_eq!(
        format!("[ {}]", text_g(fc * 1.0e6, 6)),
        "[ 10.2349]",
        "r4133's census cell, reproduced from OUR number by its own round trip"
    );
    assert_eq!(
        text_g(numbers(&deck.get("Capacitor.cap_kvar.cuf"))[0], 6),
        "10.235",
        "and OUR double at r4133's own width is 10.235, not 10.2349 — the round trip is \
         upstream of the print, which is why the display floor refuses this row"
    );
    let kvar_per_phase = kvar5 / 1.0; // Capacitor.pas:645, Fnphases = 1
    assert_eq!(
        render(kvar_per_phase / kv5 * 1.35),
        "37.5020834490805",
        "r4133's census cell (:1108)"
    );
    assert_eq!(
        render(kvar_per_phase / kv5 * 1.8),
        "50.0027779321073",
        "r4133's census cell (:1109)"
    );

    // …and the port's own recalc, handed r4133's tokens.
    deck.cmd("edit Capacitor.cap_kvar phases=1 kv=7.1996 kvar=200");
    assert_eq!(
        deck.get("Capacitor.cap_kvar.kv"),
        "7.1996",
        "the edit took effect"
    );
    assert_eq!(deck.get("Capacitor.cap_kvar.cuf"), "[ 10.2348655123603]");
    assert_eq!(deck.get("Capacitor.cap_kvar.normamps"), "37.5020834490805");
    assert_eq!(deck.get("Capacitor.cap_kvar.emergamps"), "50.0027779321073");
    // The discriminating second reading: the exact nameplate back.
    deck.cmd("edit Capacitor.cap_kvar phases=3 kv=12.47 kvar=600");
    assert_eq!(
        deck.get("Capacitor.cap_kvar.kv"),
        "12.47",
        "the edit took effect"
    );
    assert_eq!(deck.get("Capacitor.cap_kvar.cuf"), "[ 10.2349853339688]");
    assert_eq!(deck.get("Capacitor.cap_kvar.normamps"), "37.5023029706172");
}

/// `generator.kva`, `kvar`, `maxkvar` and `minkvar` after `MakePosSequence`
/// are the exact conversion of the typed values.
#[test]
fn generator_ratings_after_makeposseq_are_the_exact_typed_conversion() {
    let mut deck = Deck::compile("modes/makeposseq/makeposseq_pc.dss");

    // The port's post-conversion nameplate: the deck's 200 kW over three,
    // exactly.
    assert_eq!(
        deck.get("Generator.g_plain.kW"),
        render(200.0 / 3.0),
        "ours is the reduction's argument applied as a typed setter"
    );
    assert_eq!(deck.get("Generator.g_kva.kW"), render(200.0 / 3.0));
    // …and the four derived cells (the census `rust` spellings).
    assert_eq!(deck.get("Generator.g_plain.kVA"), "80");
    assert_eq!(deck.get("Generator.g_kvar.kVA"), "80");
    assert_eq!(deck.get("Generator.g_kva.kvar"), "21.9122736785909");
    assert_eq!(deck.get("Generator.g_kvar.kvar"), "21.9122736785909");
    assert_eq!(deck.get("Generator.g_mva.kvar"), "21.9122736785909");
    assert_eq!(deck.get("Generator.g_kva.maxkvar"), "43.8245473571818");
    assert_eq!(deck.get("Generator.g_kva.minkvar"), "-43.8245473571818");
    assert_eq!(deck.get("Generator.g_mva.maxkvar"), "43.8245473571818");
    assert_eq!(deck.get("Generator.g_mva.minkvar"), "-43.8245473571818");
    // The unmoved control: `g_kvar` typed max/min, so :3060 wrote 120/3 and
    // -60/3 as exact tokens and both engines print them.
    assert_eq!(deck.get("Generator.g_kvar.maxkvar"), "40");
    assert_eq!(deck.get("Generator.g_kvar.minkvar"), "-20");

    // r4133's token, computed from OUR state (generator.pas:3059).
    let kw5 = round_g(number(&mut deck, "Generator.g_plain.kW"), 5);
    assert_eq!(
        render(kw5),
        "66.667",
        "the token r4133 wrote, which its own `? kW` echoes back"
    );
    // …and its SyncUpPowerQuantities on it, through its own `%.6g` getters.
    assert_eq!(
        text_g(kw5 * 1.2, 6),
        "80.0004",
        "r4133's census cell, reproduced from OUR number by its own round trip \
         (:3137 -> :3021)"
    );
    // The kvar cells belong to g_kva/g_kvar/g_mva, so both factors of their
    // chain are read off THAT element (g_plain runs at pf = 1): the same 200/3
    // kW, and the pf `:3059`'s second token round-trips exactly.
    let kw5_q = round_g(number(&mut deck, "Generator.g_kva.kW"), 5);
    assert_eq!(kw5_q, kw5, "g_kva carries the same converted kW");
    assert_eq!(
        deck.get("Generator.g_kva.pf"),
        "0.95",
        "PF=%-.5g of 0.95 is exact, so the pf is not part of the round trip"
    );
    let pf = number(&mut deck, "Generator.g_kva.pf");
    let kvar_base = kw5_q * (1.0 / (pf * pf) - 1.0).sqrt(); // generator.pas:3130
    assert_eq!(
        text_g(kvar_base, 6),
        "21.9124",
        "r4133's census cell (:3018)"
    );
    assert_eq!(
        text_g(2.0 * kvar_base, 6),
        "43.8248",
        "r4133's census cell (:3133 -> :3019)"
    );
    assert_eq!(
        text_g(-(2.0 * kvar_base), 6),
        "-43.8248",
        "r4133's census cell (:3134 -> :3020)"
    );
    // Ours is the same two formulas on the exact kW, and it is NOT that render.
    assert_eq!(
        render(200.0 / 3.0 * (1.0 / (pf * pf) - 1.0).sqrt()),
        "21.9122736785909"
    );
    assert_eq!(
        text_g(200.0 / 3.0 * 1.2, 6),
        "80",
        "ours at r4133's own width"
    );

    // …and the port's own sync walks the chain when handed r4133's token.
    deck.cmd("edit Generator.g_plain kw=66.667");
    assert_eq!(
        deck.get("Generator.g_plain.kW"),
        "66.667",
        "the edit took effect"
    );
    assert_eq!(deck.get("Generator.g_plain.kVA"), "80.0004");
    deck.cmd("edit Generator.g_kva kw=66.667");
    assert_eq!(deck.get("Generator.g_kva.kvar"), "21.9123832399593");
    assert_eq!(deck.get("Generator.g_kva.maxkvar"), "43.8247664799185");
    assert_eq!(deck.get("Generator.g_kva.minkvar"), "-43.8247664799185");
    // The discriminating second reading: a kW that IS five-digit exact, on which
    // the two engines agree — the gap exists exactly when the token is not.
    deck.cmd("edit Generator.g_plain kw=100");
    assert_eq!(
        deck.get("Generator.g_plain.kW"),
        "100",
        "the edit took effect"
    );
    assert_eq!(deck.get("Generator.g_plain.kVA"), "120");
    deck.cmd("edit Generator.g_kva kw=100");
    assert_eq!(deck.get("Generator.g_kva.kvar"), "32.8684105178863");
    assert_eq!(deck.get("Generator.g_kva.maxkvar"), "65.7368210357726");
}

/// `transformer.normamps` and `emergamps` after `MakePosSequence` divide the
/// exact kVA by the exact winding kV.
#[test]
fn transformer_amps_after_makeposseq_are_the_exact_typed_conversion() {
    // --- normamps: the 115 kV substation transformer ------------------------
    let mut report = Deck::compile("modes/makeposseq/makeposseq_report.dss");
    let kv1 = numbers(&report.get("Transformer.sub.kVs"))[0];
    assert_eq!(
        render(kv1),
        render(115.0 / 3f64.sqrt()),
        "ours is Transformer.pas:1982's argument applied as a typed setter"
    );
    assert_eq!(
        report.get("Transformer.sub.NormHkVA"),
        "5500",
        "1.1*15000/3 — five-digit exact, so :1991 loses nothing on the numerator"
    );
    assert_eq!(report.get("Transformer.sub.normamps"), "82.8372125359028");

    let tok = round_g(kv1, 5);
    assert_eq!(render(tok), "66.395", "the token r4133 wrote (:1982)");
    assert_eq!(
        text_g(5500.0 / tok, 5),
        "82.838",
        "r4133's census cell, reproduced from OUR number by its own round trip \
         (:1129 -> :1842)"
    );
    assert_eq!(
        text_g(number(&mut report, "Transformer.sub.normamps"), 5),
        "82.837",
        "and OURS at r4133's own width is 82.837 — no `%.5g` print of our value is 82.838"
    );

    report.cmd("edit Transformer.sub kvs=(66.395 7.1996)");
    assert_eq!(
        numbers(&report.get("Transformer.sub.kVs"))[0],
        66.395,
        "the edit took effect"
    );
    assert_eq!(report.get("Transformer.sub.normamps"), "82.8375630695082");
    // The discriminating second reading: the numerator moves too.
    report.cmd("edit Transformer.sub normhkva=6000");
    assert_eq!(
        report.get("Transformer.sub.normamps"),
        render(6000.0 / 66.395)
    );

    // --- emergamps: the 12.47 kV three-winding transformer -------------------
    let mut xfmr = Deck::compile("modes/makeposseq/makeposseq_xfmr.dss");
    let kv1 = numbers(&xfmr.get("Transformer.t3.kVs"))[0];
    assert_eq!(render(kv1), render(12.47 / 3f64.sqrt()));
    assert_eq!(
        xfmr.get("Transformer.t3.EmergHkVA"),
        "500",
        "1.5*1000/3 — five-digit exact"
    );
    assert_eq!(xfmr.get("Transformer.t3.emergamps"), "69.4487092048467");

    let tok = round_g(kv1, 5);
    assert_eq!(render(tok), "7.1996");
    assert_eq!(
        text_g(500.0 / tok, 5),
        "69.448",
        "r4133's census cell (:1130 -> :1843)"
    );
    assert_eq!(
        text_g(number(&mut xfmr, "Transformer.t3.emergamps"), 5),
        "69.449",
        "ours at r4133's own width — the spelling r4133 itself prints on the five decks \
         that never convert"
    );

    xfmr.cmd("edit Transformer.t3 kvs=(7.1996 0.27713 0.13856)");
    assert_eq!(
        numbers(&xfmr.get("Transformer.t3.kVs"))[0],
        7.1996,
        "the edit took effect"
    );
    assert_eq!(xfmr.get("Transformer.t3.emergamps"), "69.4483026834824");
    xfmr.cmd("edit Transformer.t3 emerghkva=600");
    assert_eq!(xfmr.get("Transformer.t3.emergamps"), render(600.0 / 7.1996));
}
