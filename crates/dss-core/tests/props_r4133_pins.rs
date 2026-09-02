//! **The expected-value pins `PROPS_ECHO_R4133` owes** (`R4133_PROPS_PLAN.md`
//! §RP2.3, mechanic (c): "every exclusion whose ours-value has no capi witness
//! (r4133-only classes/**cases**) carries its own expected-value pin").
//!
//! # Why this file exists
//!
//! An `EchoRow` stops the r4133 **value** compare of one `(class, prop)` pair.
//! CLAUDE.md's exclusion discipline is that such a divergence is "excluded
//! field-by-field **and pinned by its own expected-value test**" — otherwise the
//! row is a mask with nothing behind it, and a later regression in the port's
//! own readback would be invisible on exactly the channel the row silences.
//!
//! `EchoRow::witness` names the holder of the port's value for every row:
//! `Capi(n)` (the capi_v0145 channel still value-compares the pair on `n` gating
//! cases), `Pin(name)` (no capi coverage at all — the capture has no such
//! property under `PROPS_015X`, a `SKIP_PROPS` row masks it, the capi walk skips
//! the element whole, or every covered cell sits on a capi-only case), or
//! `CapiAndPin(n, name)`. **Thirty of the tests below are those `name`s**
//! — the twenty RP2.3 landed, the nine its audit settlement added for the rows
//! exposed on `engines: "r4133"` cases, and RP3.3's `generator.model` — together
//! covering 64 of the 82 rows — and two more are the deck guard's own
//! self-tests. The literal list is
//! pinned from the table's side by
//! `harness::props_norm::tests::every_live_semantics_row_names_a_pin`, so a row
//! cannot start pointing at a test that does not exist without moving both
//! sides in one commit.
//!
//! # The pins that no echo row can name (RP3.1 onwards)
//!
//! Eight of the tests below — RP3.1's two, RP3.2's four and RP3.4's two —
//! belong to no `EchoRow` at all, and could not. RP3.1's
//! divergence is one r4133 getter reading a field the `Edit` CASE never wires;
//! RP3.2's is one r4133 getter reading the *wrong* live field — the
//! dispatched Q where the property documents the base kvar; and RP3.4's is one
//! r4133 getter **computing** a live value (`Format('%.8g',[1.0/G2])`) off a
//! field its own `RecalcElementData` mis-derived, so the render is neither a
//! parse echo nor a wrong *field*, but a correct read of a wrong number. All
//! three getters are **live**, so an echo row there would be a false statement
//! about the
//! mechanism (`R4133_PROPS_PLAN.md` §RP3.1, §RP3.2, §RP3.4). Their exclusion shape is a
//! per-case `ledger.json` `property` divergence entry, drafted in the sub-step
//! and landed at RP4.1 by the §1.1(e) staging rule, so between the two there is
//! a window in which the port's value has no holder anywhere. These pins are
//! that holder, and they are cited by
//! `props_r4133_replay::LEDGER_ENTRY_PINS` — the same guard
//! (`every_echo_row_pin_is_a_test_that_exists`) that forbids an un-cited
//! `#[test]` here reads that list too, so a pin cannot be added, renamed or
//! deleted without moving its citation with it.
//!
//! Three later sub-steps hold pins of the same "no echo row could say this" kind,
//! each with its own citation list read by that same guard: RP3.8's three
//! (`props_r4133_replay::RP38_SUPERSEDED` and `RP38_CAPTURE_PIN` — five pairs
//! whose r4133 getters render live where the 0.14.5 capture is `''`), RP3.9's,
//! which hold the port's value on the pairs RP2.4's display floor
//! refuses because the `%.Ng` round trip happened upstream
//! (`props_r4133_replay::RP39_PINS`; see the block at the end of this file), and
//! RP3.12's one (`props_r4133_replay::RP312_UPSTREAM_BUG`), which holds the
//! port's **regulated** `autotrans.wdgcurrents` where r4133's `RegControl` can
//! never tap an `AutoTrans` at all — an upstream bug, so no exclusion row of any
//! kind could state the port's value.
//!
//! # The shape of a pin
//!
//! Each one compiles **the corpus deck the 2026-08-23 claims census flagged for
//! that pair** (`DSS_PROPS_CENSUS=claims`; the per-cell rows carry the case and
//! the element), reads the port's live property render with `? Class.Name.Prop`
//! — the same getter the gate's property walk reads — and asserts it literally.
//! Where a bare "our render is X" could pass against a getter that is simply
//! hardwired, the pin adds the **discriminating** second reading (a deck or an
//! edit where the same getter answers something else), so the assertion is
//! about the value and not about the constant.
//!
//! The pins are deliberately **not** in `harness/`: that module compiles into
//! 22 test binaries, so a pin placed there would recompile and re-solve every
//! deck 22 times per `cargo test`. Here they run once. They need no oracle, no
//! feature flag and no channel, so they are green in both lanes.
//!
//! # Deck hygiene
//!
//! Several of the flagged decks write while they run — `Test/TD21RelayTest.DSS`
//! and its siblings end in `show eventlog`, `StorageControllerTechNote/Schedule/
//! ScheduleRun.dss` in nine `Export` commands — into the **vendored** corpus
//! tree. The corpus gate has its own guard for that (`corpus_gate/runner.rs`'s
//! `CorpusGuard`, which also buffers contents); it is `pub(crate)` to that
//! binary, so this file carries the same contract in miniature
//! ([`DeckDirGuard`]): snapshot the deck's directory, delete on the way out
//! every file the run created, and fail loudly if the run *changed* a vendored
//! byte count instead of only adding files. [`the_deck_guard_really_sweeps`]
//! proves it is not a no-op.
//!
//! It also carries `CorpusGuard`'s **shared per-directory snapshot**, and for
//! the same measured reason: three pins below run decks out of
//! `electricdss-tst/Test/` and two of those decks write there, so their guards
//! overlap under `cargo test`'s thread pool. With per-guard snapshots the
//! interleaving leaks — A snapshots clean, A writes, B snapshots (and sees A's
//! output as vendored), A sweeps, B writes again, B *keeps* it. One snapshot per
//! directory plus a reference count means the names set is always the pristine
//! one and exactly one sweep runs, when the last guard leaves
//! ([`overlapping_deck_guards_still_sweep`]).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use dss_core::exec::Dss;

// ---------------------------------------------------------------------------
// Deck plumbing
// ---------------------------------------------------------------------------

/// The vendored corpus root — the same tree the live gate reads (never
/// `.inputs/`, per CLAUDE.md).
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

/// Keeps the vendored deck directory exactly as it was found: files the run
/// created are removed when the **last** guard on that directory drops, and a
/// *modified* pre-existing file is a loud failure (the decks below only ever
/// add).
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
                "a pinned deck rewrote vendored corpus bytes (not merely added \
                 output files), which this guard cannot restore: {changed:?}"
            );
        }
    }
}

/// A compiled corpus deck, plus the guard that cleans up after it.
///
/// Field order is the drop order: the engine is torn down first, then the guard
/// sweeps — so anything the deck's own `Export`/`Show` wrote is already flushed
/// when the sweep runs.
struct Deck {
    dss: Dss,
    _guard: DeckDirGuard,
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
        let path = corpus_root().join(rel);
        assert!(
            path.is_file(),
            "vendored deck is missing: {}",
            path.display()
        );
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
        Self { dss, _guard: guard }
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

/// Two guards on the **same** directory overlap (three pins below run decks out
/// of `electricdss-tst/Test/`, and two of those decks write there). The
/// interleaving that per-guard snapshots leak: A snapshots clean → A writes → B
/// snapshots and sees A's output as vendored → A sweeps → B writes again → B
/// keeps it. With the shared snapshot B reuses A's pristine names and the sweep
/// runs once, when the last guard leaves.
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

/// `autotrans.bhcurrent` / `autotrans.bhflux` — `EchoCategory::
/// EmptyCollectionRender`, **pin-only** (`PROPS_015X` drops both from the
/// 0.14.5 capture, so the capi channel never compares them).
///
/// r4133's arms 44/45 are LIVE (`Version8/Source/PDElements/AutoTrans.pas:
/// 1865-1878`): each emits `'['`, loops `1..NumPointsBH` and closes `']'`, so an
/// autotransformer that never set `BHpoints` renders `'[]'` where this port
/// renders `''`. Both mean "no B-H curve"; the row masks the spelling, and this
/// pin holds the port's half.
///
/// Deck: `asymmetric/autotrans/autotrans_gic.dss` (`AutoTrans.t1`; the pair's 9
/// in-scope cells are the two autotransformers of the `asymmetric:autotrans/*`
/// cases). `BHpoints` is read first because it is *why* the render is empty —
/// without it the assertion could pass against a getter that always answers
/// `''`.
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

/// `energymeter.peakcurrent` — `EchoDefault`, capi-witnessed on 6 cases **and**
/// pinned (the plan names this pair explicitly).
///
/// r4133 has no getter arm for index 7, so the property answers the
/// `PropertyValue[]` store, which `InitPropertyValues` froze at
/// `'(400, 400, 400)'` (`Version8/Source/Meters/EnergyMeter.pas:2209`) and the
/// pre/post blocks re-wrap in parens (`:2640-2643`, `:2660-2663`) —
/// `'((400, 400, 400))'` — no matter how many phases the metered element has.
/// The port renders the LIVE array, whose length is the element's phase count.
///
/// The row therefore masks two different things and only the second is this
/// pin's: on a 3-phase meter both sides are three 400s and a `PROPS_NORM_R4133`
/// `ArrayForm` row claims the cell first (the chain order); the six cells where
/// the meter is single-phase spell `'[ 400]'`, which no array rule can fold
/// against three elements. Decks: `.../IEEE-TIA-LV Model/master_small.dss`
/// (1-phase `EnergyMeter.sub`, 4 in-scope cells) against
/// `controls/energymeter/energymeter_sym.dss` for the 3-phase render.
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

/// `expcontrol.derlist` — `LiveSemanticsDiffer`, capi-witnessed on 2 cases and
/// pinned (every row of that category owes a pin: the claim is "ours is the
/// right value", which capi agreement cannot carry).
///
/// r4133's getter answers `ReturnElementsList` for **both** index 1
/// (`PVSystemList`, the user's typed list) and index 14 (`DERList`, the
/// resolved DER set) — `Version8/Source/Controls/ExpControl.pas:684`, `:696` —
/// and `ReturnElementsList` (`:702-715`) is hard-wired to `FPVSystemNameList`,
/// the bare, class-prefix-stripped names. So r4133 renders the same bare list
/// twice and loses the distinction. The port keeps it: `PVSystemList` echoes the
/// typed names, `DERList` the resolved, class-qualified elements — which is what
/// makes the second property worth having.
///
/// Deck: `controls/expcontrol/expcontrol_basic.dss` (`ExpControl.ec`, 10 in-scope
/// cells). Both properties are read, because the two renders being *different*
/// is the whole content of the claim.
#[test]
fn expcontrol_derlist_renders_the_der_list() {
    let mut deck = Deck::compile("controls/expcontrol/expcontrol_basic.dss");
    assert_eq!(deck.get("ExpControl.ec.PVSystemList"), "[pv]");
    assert_eq!(deck.get("ExpControl.ec.DERList"), "[PVSystem.pv]");
}

/// `fault.bus2` — `EchoParse`, **pin-only**: the pair's single echo cell sits
/// on `modes/makeposseq/makeposseq_shunt.dss`, an `engines: "capi_v0145"` case,
/// so the r4133 compare never reaches it and the capi compare is the only live
/// witness. Offline (the replay walks the frozen census) this pin is what makes
/// the row honest.
///
/// r4133 has no getter arm for index 2 (`Version8/Source/PDElements/Fault.pas:
/// 699-717` covers 6 only), so `bus2` answers the store — seeded at `:672` and
/// re-snapshotted from `GetBus(2)` every time `bus1=` is parsed (`:297`). A later
/// `MakePosSequence` collapses the fault to one phase without touching that
/// string, so r4133 keeps the stale 3-phase spelling `'b2.0.0.0'`.
///
/// The port renders the live terminal, so `phases` and `bus2` agree — which is
/// exactly what the two reads below assert.
#[test]
fn fault_bus2_renders_the_live_terminal() {
    let mut deck = Deck::compile("modes/makeposseq/makeposseq_shunt.dss");
    assert_eq!(deck.get("Fault.flt.phases"), "1");
    assert_eq!(deck.get("Fault.flt.bus1"), "b2");
    assert_eq!(deck.get("Fault.flt.bus2"), "b2.0");
}

/// `generator.d` — `LiveSemanticsDiffer` + pin. **Not an echo**: the store
/// agrees with r4133's live field, and the field itself is wrong upstream.
///
/// Property 32 is `D`, documented "Default is 1.0"
/// (`Version8/Source/PCElements/generator.pas:467`), and `Edit` writes
/// `GenVars.Dpu` (`:669`) — but `Create` initialises `GenVars.D := 1.0`
/// (`:969`) and **never** `Dpu`, so `Dpu` starts at 0 and
/// `InitPropertyValues` snapshots that zero (`:2585`). `InitStateVars` then
/// computes `D := Dpu*kVA*1000/w0 = 0` (`:2710`): r4133 runs generator dynamics
/// with **no damping** on any deck that does not set `D=`, against its own
/// documented default. dss_capi 0.14.5 fixed it (`Dpu := 1.0`,
/// `src/PCElements/Generator.pas:1006`) and this port follows.
///
/// Under the 2026-08-02 policy an upstream bug is never reproduced, so the port
/// keeps 1.0 and the divergence is excluded + pinned here. Deck:
/// `asymmetric/generator/generator_asym.dss` (`Generator.g1`, one of the 136
/// in-scope cells). The edit half proves the getter tracks `Dpu` rather than
/// answering a constant.
#[test]
fn generator_d_renders_the_documented_damping_default() {
    let mut deck = Deck::compile("asymmetric/generator/generator_asym.dss");
    assert_eq!(deck.get("Generator.g1.D"), "1");
    deck.cmd("edit Generator.g1 D=2.5");
    assert_eq!(deck.get("Generator.g1.D"), "2.5");
}

/// `generator.dynout` — `LiveSemanticsDiffer`, capi-witnessed on 26 cases and
/// pinned. The row masks two spellings and this pin holds the second, sharper
/// one.
///
/// r4133's arm 46 is live (`GetDynOutputStr`,
/// `Version8/Source/PCElements/generator.pas:3034`); on a generator with no
/// `DynamicEq` it prints `'[]'` where the port prints `''` (the empty-collection
/// convention, 271 cells). On the two `Dynamic_KundurDynExp*` cases the deck
/// really does declare `DynOut = [Speed theta]`, and there r4133 answers
/// `'[speed,dpshaft,]'`: `DynamicExp.pas:411-437` stores the *variable* index
/// while `:441-465` decodes it as a flat *(variable, derivative slot)* index, so
/// the second name is read out of the wrong slot and the third is empty. The
/// port decodes the pair correctly and names the two variables the deck asked
/// for. Upstream report: `investigations/to_opendss/41-*`.
///
/// Deck: `.../Dynamic_Expressions/Dynamic_KundurDynExp-steady-state-only.dss`
/// (`Generator.g1`; its solving twin `Dynamic_KundurDynExp.dss` is the second of
/// the two cells). The `DynamicEq` read is the non-vacuity: the names come from
/// a real dynamic expression, not from a default.
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

/// `generator.model` — `EchoParse`, **pin-only**, and pin-only in the strongest
/// sense: both of the pair's census cells sit on `engines: "r4133"` cases
/// (`modes:ncim/ncim_pv_pq.dss`, `modes:ncim/ncim_midi.dss`), where the capi
/// channel never runs, so no `Capi(n)` witness could have held the port's value
/// (`props_norm::ECHO_ROWS_ON_R4133_ONLY_CASES`). RP3.3's row.
///
/// r4133's `TGeneratorObj.GetPropertyValue` has **no arm 6**
/// (`Version8/Source/PCElements/generator.pas:3007-3038` — arms 3,4,5,7,8,9,13,
/// 19,20,26,27,34,36,37,38,40..46, then `ELSE Result := Inherited`), so `model`
/// falls through to `General/DSSObject.pas:112-115`
/// `Result := FPropertyValue[Index]` — the deck's own typed token, stored
/// unconditionally by the Edit loop at `generator.pas:625` *before* the CASE
/// assigns the live field at `:643`. Meanwhile NCIM moves the LIVE `GenModel`
/// 3 → 4 (`Common/Solution.pas:2120`, the Q-band promote; `:1935` is the
/// zero-Q-limits demote) and never moves it back: `ReversePQ2PV`
/// (`:1743-1768`), whose body carries the `// Takes it back to model 3` comment,
/// **has no caller** anywhere in the trunk. So r4133 renders `'3'` for a
/// generator whose live model is `4`; the port renders the live field
/// (`obj/props/class_props/value.rs` → `elements/pc/generator/accessors.rs`) and
/// says `4`. Probed on the live r4133 DLL (RP3.3 part A2): `GeneratorsI(9)` — the
/// field itself, `DDLL/DGenerators.pas:125-134` — reads 4 on both decks after the
/// solve while `? Generator.g1.model` renders `'3'`.
///
/// Decks: both of the pair's own cases, since `ncim_midi.dss` has no other unit
/// pin at all. Each ends with its own `Solve`, so the compiled deck is already
/// converted. The `maxkvar` reading names *which* conversion this is (the
/// `:2120` Q-band promote, not the `:1935` zero-limits one), and the
/// edit-then-re-solve round trip is the discriminator: the getter follows the
/// deck's token back to `3` and the **engine** — not the parser — puts it back
/// to `4`.
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

/// `line.conductors` — `EchoDefault`, **pin-only** (`PROPS_015X`'s multi-line
/// `Line` row drops `Conductors` from the 0.14.5 capture, so capi never compares
/// it on any of its 75 162 in-scope cells) — **and, since the RP2.3 audit
/// settlement, its three siblings `line.wires` / `line.cncables` /
/// `line.tscables` too**.
///
/// r4133 has no getter arm for indices 22, 24, 25 or 34
/// (`Version8/Source/PDElements/Line.pas:1338-1438`), so each answers the store,
/// which `InitPropertyValues` left `''` (`:1512`, `:1514`, `:1515`, `:1524`) and
/// only an explicit `wires=`/`cncables=`/`tscables=`/`conductors=` would
/// overwrite. The port renders the live conductor list from all four.
///
/// The three siblings carry a capi witness (233 cases) as well, but that witness
/// is silent about the 6 231 (`wires`) / 6 232 (`cncables`, `tscables`) cells
/// each row masks on `engines: "r4133"` cases, where the capi channel does not
/// run at all — the largest exposure in
/// `props_norm::ECHO_ROWS_ON_R4133_ONLY_CASES` — so they name this pin too.
///
/// Decks: `modes/upgrade/upgrade_spacing_ratings.dss` (`Line.l1`, defined with
/// `wires=[big small big neut]`) for the populated render — itself one of the
/// r4133-only cases the census flagged — and
/// `asymmetric/autotrans/autotrans_gic.dss` for the `'[]'` an impedance-defined
/// line gives. The pair of readings is what makes this a pin on the *list* and
/// not on a constant.
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

/// `line.spacing` — `EchoParse`, **pin-only** for the same reason as
/// `fault.bus2`: the pair's one echo cell is on `modes/makeposseq/
/// makeposseq_line.dss`, a capi-only case, so it has **0 in-scope cells** and the
/// row can never fire live (`props_norm::ECHO_ROWS_WITH_NO_IN_SCOPE_CELL`).
///
/// r4133 has no arm for index 21 either (`Line.pas:1338-1438`), so `spacing`
/// answers the store — the deck's own `'sp'` token, written at `:598`.
/// `MakePosSequence` kills the geometry/spacing state (`:2266-2276`) but never
/// rewrites that string, so r4133 keeps advertising a spacing the line no longer
/// has. The port renders the live one, i.e. nothing.
///
/// Decks: `makeposseq_line.dss` (`Line.l_spc`, after the deck's own
/// `MakePosSeq`) against `modes/upgrade/upgrade_spacing_ratings.dss`, where a
/// spacing IS in force and the same getter names it.
#[test]
fn line_spacing_renders_empty_once_the_spacing_is_killed() {
    let mut killed = Deck::compile("modes/makeposseq/makeposseq_line.dss");
    assert_eq!(killed.get("Line.l_spc.Spacing"), "");
    let mut alive = Deck::compile("modes/upgrade/upgrade_spacing_ratings.dss");
    assert_eq!(alive.get("Line.l1.Spacing"), "sp");
}

/// `load.yearly` — `LiveSemanticsDiffer`, capi-witnessed on 84 cases and
/// pinned.
///
/// r4133's arm 7 is live but answers `Yearlyshape`, the **raw user-typed
/// string** (`Version8/Source/PCElements/Load.pas:2346`), which stays `''` when
/// the deck never typed `yearly=` (`:807`). Its `daily=` arm, however, aliases
/// the yearly *object* to the daily one (`:657`) and `CalcYearlyMult` then
/// dispatches on that object — so r4133 solves a yearly simulation off a shape
/// whose name it reports as empty. The port reports the resolved shape.
///
/// Deck: `controls/capcontrol/capcontrol_sym.dss` (`Load.l1`, 24 in-scope
/// cells): it types `daily=day` and no `yearly=`, so the two reads below are the
/// aliasing itself. `Duty` is read as the discriminator — it is *not* aliased,
/// so a getter that simply echoed `Daily` everywhere would fail here.
///
/// The pair's case-only cells (`'other'` vs `'Other'`, …) are claimed by RP2.3's
/// own `CaseFold` normalization row before this exclusion is consulted; only the
/// resolved-vs-empty half is masked.
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

/// `pvsystem.%pminnovars` / `pvsystem.%pminkvarmax` / `storage.%pminnovars` /
/// `storage.%pminkvarmax` — four `LiveSemanticsDiffer` rows, capi-witnessed
/// (99 / 44 cases) and pinned.
///
/// All four arms are live on both sides; what differs is the **sentinel** for
/// "deactivated". r4133's `Create` starts them at `-1.0`
/// (`Version8/Source/PCElements/PVsystem.pas:1037-1038`,
/// `Storage.pas:1368-1369`, the latter commented "Deactivated by default"), the
/// port (with dss_capi 0.14.5) at `0.0`. Behaviour is identical because both
/// engines test the same way — r4133 `PVsystem.pas:1395-1398`
/// `if FpctPminNoVars <= 0 then PminNoVars := -1`, the port
/// `pvsystem/nominal.rs` — so no var limit moves; only the stored input spells
/// the "off" state differently.
///
/// Deck: `asymmetric/der/der_asym.dss` (`PVSystem.pv1`, `Storage.st1`). The
/// edits are the discriminator: a typed percentage renders back verbatim, so the
/// `0` above is the default state and not a getter stuck at zero.
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

/// `recloser.eventlog` / `recloser.debugtrace` — `EchoDefault`, **pin-only**:
/// the capi walk skips every Recloser element whole
/// (`harness/mod.rs::skip_whole_element`), so no capi cell exists for either.
///
/// `InitPropertyValues` writes 1..28 and then jumps straight to 31
/// (`Version8/Source/Controls/Recloser.pas:1553-1554`), so properties 29
/// (`EventLog`) and 30 (`DebugTrace`) are **never initialised** and, with no
/// getter arm for them, echo `''`. The port renders the live boolean.
///
/// Decks: `controls/recloser/recloser_perm.dss` (`Recloser.r`, 24 in-scope
/// cells) for the default, `controls/recloser/recloser_1ph.dss` for a deck that
/// really turns the event log on — the cells RP2.1's `BoolFold` row claims
/// before this exclusion is consulted, and the proof that `'No'` is a value and
/// not a constant.
#[test]
fn recloser_eventlog_and_debugtrace_default_to_no() {
    let mut deck = Deck::compile("controls/recloser/recloser_perm.dss");
    assert_eq!(deck.get("Recloser.r.EventLog"), "No");
    assert_eq!(deck.get("Recloser.r.DebugTrace"), "No");
    let mut logging = Deck::compile("controls/recloser/recloser_1ph.dss");
    assert_eq!(logging.get("Recloser.r.EventLog"), "Yes");
}

/// `recloser.switchedobj` — `EchoDefault`, **pin-only** (the whole-element capi
/// skip again).
///
/// Property 3 has no getter arm and `InitPropertyValues` sets it `''`
/// (`Version8/Source/Controls/Recloser.pas:225`, `:1528`), so a deck that never
/// types `SwitchedObj=` gets `''` back from r4133 — while the *live*
/// `ElementName` defaults to the monitored element (`:448`). The port renders
/// that live default.
///
/// Deck: `controls/fuse/indmach_r4133/indmach_snap.dss`, whose four reclosers
/// each monitor a different line (8 in-scope cells each). Reading `MonitoredObj`
/// beside `SwitchedObj` is the point: the pin asserts the *defaulting rule*, not
/// four names.
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

/// `regcontrol.idle` / `.idleforward` / `.idlereverse` / `.fwdthreshold` /
/// `.revthreshold` — five `EchoDefault` rows, all **pin-only**.
///
/// Four of them (`Idle`, `IdleForward`, `IdleReverse`, `FwdThreshold`) are
/// absent from the 0.14.5 capture and dropped by `PROPS_015X`'s multi-line
/// `RegControl` row — probe-confirmed: the pinned oracle reports 35 RegControl
/// property names, the port 39. The fifth, `RevThreshold`, is masked on capi by
/// its own `SKIP_PROPS` row (a changed 0.14.5 default). So the capi channel
/// witnesses none of the five.
///
/// `InitPropertyValues` writes 1..32 only (`Version8/Source/Controls/
/// RegControl.pas:1444-1459`) and `GetPropertyValue` overrides index 28 alone
/// (`:820-827`), so properties 33..36 echo `''` while the live fields hold
/// `False`/`False`/`False`/100. `RevThreshold` is the sharper one: `:1448`
/// freezes `'100'` while `Create` sets `kWRevPowerThreshold := -100.0` (`:627`),
/// and a deck's `revThreshold=800` leaves the store at `'800'` while `:503-505`
/// makes the live value `-800`.
///
/// Decks: `controls/regcontrol/regcontrol_sym.dss` for the defaults (RP0.2's
/// re-census measured **888 cells / 733 in scope** for `FwdThreshold`, split
/// `'100'` 864/709 and `'800'` 24/24 — re-derived with `DSS_PROPS_CENSUS=claims`
/// on 2026-08-23, since the frozen extracts predate the pair);
/// `regcontrol_reverse.dss` for the 24 `'800'`/`-800` cells; and
/// `regcontrol_idle.dss`, the one corpus deck that actually idles a regulator,
/// so `'No'` is proven to be a reading rather than a constant.
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

/// `relay.action` / `relay.distreverse` / `relay.reset` — three rows
/// (`EchoDefault`, `EchoDefault`, `EchoParse`), all **pin-only**: the capi
/// walk skips every Relay element whole.
///
/// * `Action` (property 19, DEPRECATED) has no arm and `:1577` freezes
///   `'closed'`; the port renders the live — empty — pending action.
/// * `DistReverse` (property 38): `InitPropertyValues` jumps 37 → 39
///   (`Version8/Source/Controls/Relay.pas:1595-1596`), so it is never written
///   and echoes `''`.
/// * `Reset` (property 54) is the `EchoParse` one. `:1611` defaults it `'n'`
///   (those cells fold under `BoolFold` and never reach this row), but the
///   distance-relay decks literally type `reset=0.20` — an old "reset time"
///   usage. `:504` stores the raw token and arm 54 (`:566-570`) rewrites the
///   store to `'n'` **only** when `InterpretYesNo` says yes, so `'0.20'`
///   survives as a property value that means nothing.
///
/// Decks: `electricdss-tst/Test/TD21RelayTest.DSS` (`Relay.21src`; the deck
/// types `reset=0.20` at `:14`/`:58`) and its reverse twin
/// `ReverseTD21RelayTest.DSS`, where `DistReverse` is really on. Both decks end
/// in `show eventlog`, hence [`DeckDirGuard`].
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

/// `relay.switchedobj` — `EchoDefault`, **pin-only** (whole-element capi skip).
///
/// Same mechanism as the recloser: property 3 has no getter arm and is `''` from
/// `InitPropertyValues` (`Version8/Source/Controls/Relay.pas:320`, `:1561`),
/// while the live `ElementName` defaults to the monitored element (`:582`).
///
/// Deck: `electricdss-tst/Test/DistanceRelayTest.DSS` (`Relay.21src`, 16
/// in-scope cells over the four distance-relay cases). `MonitoredObj` is read
/// beside it so the assertion is about the defaulting rule.
#[test]
fn relay_switchedobj_defaults_to_the_monitored_element() {
    let mut deck = Deck::compile("electricdss-tst/Test/DistanceRelayTest.DSS");
    assert_eq!(deck.get("Relay.21src.MonitoredObj"), "Line.thev");
    assert_eq!(deck.get("Relay.21src.SwitchedObj"), "Line.thev");
}

/// `storage.dynadll` — `LiveSemanticsDiffer`, capi-witnessed on 2 cases and
/// pinned. **This pin asserts the PORT's render only**, deliberately.
///
/// r4133's arm is live and answers `DynaModel.Name`
/// (`Version8/Source/PCElements/Storage.pas:1576`), and
/// `TStoreDynaModel.Set_Name` assigns `FName` **only after a successful
/// `LoadLibrary`** (`StoreUserModel.pas:195-240`). The deck's
/// `C:\Users\prdu001\…\Dess1.DLL` does not exist on this machine, so r4133
/// answers `''` — i.e. the divergence is *environment-dependent*: on a box that
/// has that DLL, r4133 would print the path and the cell would not diverge at
/// all (RP2.3 part A, finding F6). Pinning r4133's absence would therefore pin
/// the machine, not the engine.
///
/// The port's side is not environment-dependent: safe Rust never loads a Windows
/// DLL (user models are wasm — `WASM_USERMODELS_PLAN.md`), so it warns #1570,
/// falls back to the built-in model, and keeps the typed path as the property
/// value. That is what is asserted, together with the warning that explains it.
///
/// Deck: `electricdss-tst/Test/SimpleStorageTest-1ph.dss` (`Storage.store1`).
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

/// `storagecontroller.modedischarge` — `LiveSemanticsDiffer`, capi-witnessed on
/// 1 case and pinned.
///
/// Both engines hold the same mode. r4133's `GetModeString` has no case for
/// `MODESCHEDULE` in its `propMODEDISCHARGE` arm and falls through to
/// `ELSE Result := 'UNKNOWN'` (`Version8/Source/Controls/
/// StorageController.pas:1200-1214`) — a **non-injective catch-all**, which is
/// why the pair can never become an `EnumSynonym` row: `'UNKNOWN'` is not
/// another spelling of `Schedule`, it is the absence of one. The port names the
/// mode.
///
/// Deck: `.../StorageControllerTechNote/Schedule/ScheduleRun.dss`
/// (`StorageController.sc`, the pair's single cell); it ends in nine `Export`
/// commands, hence [`DeckDirGuard`]. `controls/storagecontroller/
/// storagectrl_peakshave.dss` is the discriminator — a mode r4133 *does* name,
/// so the port is answering the mode rather than a constant.
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

/// `swtcontrol.action` — `EchoParse`, **pin-only**. The capi channel covers
/// only this pair's 18 out-of-scope cells; all 16 cells the row masks on the
/// r4133 side are on `engines: "r4133"` cases, so nothing but this test holds
/// the port's value there.
///
/// Property 3 has no getter arm (`Version8/Source/Controls/SwtControl.pas:
/// 573-620`), and the Edit arm stores the raw token **unconditionally** before
/// the CASE that would act on it (`:192-193`); `InterpretSwitchState` then exits
/// without doing anything when the control is `Locked` (`:417`). So r4133's
/// `Action` is a record of what was last *typed*, abbreviations and all
/// (`'c'`, `'o'`), and can contradict the switch. The port renders the live
/// pending action, spelled in full.
///
/// Deck: `.../civinlar model/civanlar.dss` — 13 `close` and 3 `open` in-scope
/// cells across its sixteen tie switches. `State` is read beside `Action` so the
/// pin says what "live" means here: the action the port reports is the one the
/// switch is actually in.
///
/// **RP3.7 moved the `State` side of that reading, not the claim** (2026-09-02):
/// `State` is now the per-phase render r4133 has always had — one token per
/// controlled-element phase (`SwtControl.pas:600-610`) — so a 3-phase tie answers
/// `[closed, closed, closed, ]` instead of the port's old scalar `closed`. Both
/// values below were read off the r4133 DLL on this very deck
/// (`tmp/rp37/out_b2_r4133b.txt`, RP3.7 B2), which is what makes the `Action`
/// claim sharper rather than weaker: `Action` still renders one word while the
/// state it is compared against is a per-phase array, and the two still agree.
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

/// `transformer.bhcurrent` / `transformer.bhflux` — `EchoCategory::
/// EmptyCollectionRender`, **pin-only** (`PROPS_015X` drops both, as for
/// AutoTrans).
///
/// r4133's arms 51/52 are live and loop `1..NumPointsBH`
/// (`Version8/Source/PDElements/Transformer.pas:1820-1834`), so an unset B-H
/// curve renders `'[]'` against the port's `''` — the same convention difference,
/// on 20 716 in-scope cells (the corpus is full of transformers).
///
/// Deck: `controls/regcontrol/regcontrol_sym.dss` (`Transformer.tr`).
/// `BHpoints` is read first for the same reason as the AutoTrans pin.
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

/// `windgen.dynout` — `EchoCategory::EmptyCollectionRender`, **pin-only** for a
/// reason none of the others share: **no capi-gating case holds a WindGen at
/// all**. All five `modes:windgen/*` decks are `engines: "r4133"`, so the capi
/// channel has nothing to say about this pair, ever.
///
/// r4133's arm 22 is live (`GetDynOutputStr`,
/// `Version8/Source/PCElements/WindGen.pas:2906`, `:1119`) and prints `'[]'` for
/// a machine with no dynamic expression, where the port prints `''`.
///
/// Deck: `modes/windgen/windgen_snap.dss` (`WindGen.w1`, one of the pair's five
/// in-scope cells — one per windgen deck). `DynamicEq` is read as the reason the
/// list is empty.
#[test]
fn windgen_dynout_renders_empty_when_unset() {
    let mut deck = Deck::compile("modes/windgen/windgen_snap.dss");
    assert_eq!(deck.get("WindGen.w1.DynamicEq"), "");
    assert_eq!(deck.get("WindGen.w1.DynOut"), "");
}

// ---------------------------------------------------------------------------
// Pins added by the RP2.3 audit settlement (2026-08-23): the rows whose masked
// cells sit on `engines: "r4133"` cases
// ---------------------------------------------------------------------------
//
// Plan §1.2 mechanic (c) asks for a pin wherever an exclusion's ours-value has
// "no capi witness (r4133-only classes/**cases**)". Part B2 read that as
// classes and applied the *cases* half to one row by hand
// (`swtcontrol.action`). The settlement measured the whole population with the
// claims census crossed against each case's `engines` flag —
// `props_norm::ECHO_ROWS_ON_R4133_ONLY_CASES`, 57 rows / 34 969 cells at the
// time — and the 31 rows that had only a `Capi(n)` witness get one here. (RP3.3
// added the 58th row and its two cells; its pin sits above with the pairs it
// belongs to.)
//
// Each pin below reads the port's live render on a deck the census named for
// that pair (an r4133-only one wherever the pair has such a case), and adds the
// discriminating second reading wherever the property can be written back — the
// same shape as the twenty pins above. Two of them cannot: `action=` on an
// EnergyMeter and `reset=` on a CapControl are one-shot COMMANDS whose re-read
// is empty by construction, which is itself the thing being pinned.

/// `pvsystem.amplimit` / `pvsystem.amplimitgain` / `storage.amplimit` /
/// `storage.amplimitgain` — four `EchoDefault` rows, capi-witnessed (99 / 47
/// cases) and pinned for the 61 + 43 cells each masks on r4133-only cases.
///
/// r4133 has no getter arm and no `InitPropertyValues` entry for either property
/// on either class (`Version8/Source/PCElements/PVsystem.pas:392-393`, the
/// CASE's ELSE at `:1174`, the init block `:1074-1124`; `Storage.pas` props
/// 60/61 and `:1446-1521`), so the store answers `''` for all four. The port
/// renders the live pair: the current limit's "off" sentinel `-1` and the
/// controller gain `0.8` both classes start from.
///
/// Decks: `controls/invcontrol/invcontrol_vv_delta.dss` (`PVSystem.pv1`) and
/// `controls/gfm/gfm_micro.dss` (`Storage.batt`), both `engines: "r4133"`. The
/// edits are the discriminator — a typed limit renders back verbatim, so the
/// sentinel above is the default state and not a getter stuck on a constant.
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

/// `generator.shaftdata` / `generator.userdata` / `pvsystem.dynout` /
/// `pvsystem.userdata` / `storage.dynadata` / `storage.userdata` — six
/// `EmptyCollectionRender` rows, capi-witnessed (26 / 99 / 47 cases) and pinned
/// for the 43 + 61 + 49 cells each masks on r4133-only cases.
///
/// r4133's arms are live and paren-wrap (or bracket) whatever the user-model
/// data string holds — `generator.pas:3023-3025`, `PVsystem.pas:1158` / `:1171`,
/// `Storage.pas:1574` / `:1577` — so an element with no user model renders
/// `'()'` (or `'[]'` for `DynOut`) where this port renders `''`. Same state,
/// different empty convention.
///
/// Decks: `controls/relay/relay_doc.dss` (`Generator.g1`),
/// `controls/invcontrol/invcontrol_vv_delta.dss` (`PVSystem.pv1`) and
/// `controls/gfm/gfm_micro.dss` (`Storage.batt`), all `engines: "r4133"`. The
/// edits are the discriminator: a typed data string renders back verbatim on
/// every one of these getters, so the `''` above is the empty state and not a
/// dead arm.
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

/// `energymeter.action` / `capcontrol.reset` — two `EchoDefault` rows,
/// capi-witnessed (67 / 31 cases) and pinned for the 45 / 3 cells they mask on
/// r4133-only cases.
///
/// Both properties are one-shot COMMANDS, and r4133 answers neither of them from
/// a getter: `EnergyMeter` has no arm for index 3 (`Meters/EnergyMeter.pas:482`,
/// `:2645-2658`) so it echoes the `'clear'` its `InitPropertyValues` froze at
/// `:2205`, and `CapControl`'s init writes slots 20, 21 and 23 but never 22
/// (`Controls/CapControl.pas:196`, `:1254-1282`), leaving `''` there. The port
/// renders the state after the command ran: nothing pending on the meter, and
/// the CapControl's own `Reset` flag back to `No`.
///
/// Decks: `controls/combo/combo_protection.dss` (`EnergyMeter.em`) and
/// `.../EPRITestCircuits/epri_dpv/M1/Master_NoPV.dss`
/// (`CapControl.cap1_ctrl` — the only r4133-only case that holds a CapControl),
/// both `engines: "r4133"`.
///
/// **No discriminating edit exists here, and that is the point**: re-issuing the
/// command (`action=clear`, `reset=yes`) leaves the render exactly where it was,
/// because these getters report a state and not the last word typed — which is
/// precisely how they differ from r4133's frozen store. Both readings below are
/// asserted before AND after the command for that reason.
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

/// `fuse.switchedobj` — `EchoDefault`, capi-witnessed on 1 case and pinned for
/// the 20 cells it masks on the two `controls/fuse/indmach_r4133/*` cases, which
/// are `engines: "r4133"`.
///
/// The same shape as the Recloser and Relay pins above: r4133 has no arm for
/// index 3 (`Version8/Source/Controls/Fuse.pas:183`, `:680-720`) and its
/// `InitPropertyValues` leaves the slot `''` (`:801ff`), while the live
/// `ElementName` defaults to the MONITORED element when the deck does not type a
/// `switchedobj=` (`:292`). The port renders that live default.
///
/// Deck: `controls/fuse/indmach_r4133/indmach_snap.dss` (`Fuse.f1` on a Line,
/// `Fuse.f2` on a Transformer — two different monitored classes, so the
/// assertion is about the defaulting and not about one string). The edit is the
/// discriminator.
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

/// `invcontrol.lpftau` / `risefalllimit` / `vsetpoint` / `pvsystemlist` /
/// `monvoltagecalc` / `mode` — six `EchoDefault` rows, capi-witnessed (86-87 /
/// 23 cases) and pinned for the 18 / 8 / 1 cells they mask on r4133-only cases.
///
/// Two mechanisms, both `PropertyValue[]` echoes. `lpftau` and `risefalllimit`
/// have `InitPropertyValues` entries that contradict `Create`
/// (`Version8/Source/Controls/InvControl.pas:2828-2829` freeze `'0.0'` and
/// `'-1.0'` while `:1166-1167` set both live fields to `0.001`) and no getter
/// arm (`:3232-3285`); `vsetpoint`, `pvsystemlist` and `monvoltagecalc` have
/// neither an arm nor an init entry (`:505`, `:512`, `:513`), so they answer
/// `''`; `mode`'s arm is commented out (`:3234-3239`) over an init that froze
/// `'VOLTVAR'` (`:2809`) while `Create` starts at `NONE_MODE` (`:1135`).
///
/// Decks, all `engines: "r4133"`:
/// `controls/gfm/gfm_invcontrol.dss` (`InvControl.ic`, controlling a Storage)
/// for the five defaults, and `.../MonitoredVoltage/
/// Mon_voltage_MAX_Mix_avg_VVDRC-2.dss` (`InvControl.vv_drc`) for `mode`, which
/// is where the census found the pair's one r4133-only cell: that deck drives
/// the control through `CombiMode`, so the live `Mode` really is unset. The
/// edits are the discriminators.
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

/// `load.zipv` — `EmptyCollectionRender`, capi-witnessed on 221 cases and pinned
/// for the 4 070 cells it masks on 58 r4133-only cases.
///
/// r4133's arm 33 is live but loops `nZIPV` (`Version8/Source/PCElements/
/// Load.pas:2354-2357`), so a load that never typed `zipv=` renders `''` where
/// the port renders the materialised seven-element vector the ZIP model is
/// evaluated from.
///
/// Deck: `controls/gfm/gfm_micro.dss` (`Load.isl`), `engines: "r4133"`. The edit
/// is the discriminator — and it is the one that matters most for this pair,
/// because a getter stuck on seven zeros would satisfy the first reading.
#[test]
fn load_zipv_renders_the_live_seven_element_vector() {
    let mut deck = Deck::compile("controls/gfm/gfm_micro.dss");
    assert_eq!(deck.get("Load.isl.ZIPV"), "[ 0 0 0 0 0 0 0]");
    deck.cmd("edit Load.isl ZIPV=(1,2,3,4,5,6,0.5)");
    assert_eq!(deck.get("Load.isl.ZIPV"), "[ 1 2 3 4 5 6 0.5]");
}

/// `transformer.pctperm` / `transformer.repair` / `autotrans.pctperm` /
/// `autotrans.repair` / `fault.pctperm` — five `EchoDefault` rows, capi-witnessed
/// (133 / 9 / 13 cases) and pinned for the 799 / 2 / 347 cells they mask on
/// r4133-only cases.
///
/// All five are the same `DSSObject.pas:112-115` fallthrough over an
/// `InitPropertyValues` line the engine never refreshes: `'100'`/`'36'` for the
/// two PD classes (`Version8/Source/PDElements/Transformer.pas:1918-1919`,
/// `AutoTrans.pas:1958-1959`, whose PD tails re-render slots 1..2 only —
/// `:1840-1844`, `:1883-1888`) and `'0'` for `Fault` (`Fault.pas:687`). The port
/// renders the live reliability data, which for these decks is the class default
/// the elements were created with.
///
/// Note the two classes disagree in opposite directions, which is what makes the
/// readings worth having: a Transformer created without reliability data has
/// `%perm = 0` where r4133 advertises 100, and a Fault has `%perm = 100` where
/// r4133 advertises 0.
///
/// Decks, all `engines: "r4133"`: `controls/gfm/gfm_micro.dss`
/// (`Transformer.tsto`), `asymmetric/autotrans/autotrans_xfmrcode.dss`
/// (`AutoTrans.t1`) and `controls/fuse/fuse_blow_3ph.dss` (`Fault.f`). The edits
/// are the discriminators.
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

/// `reactor.kvar` — `EchoDefault`, capi-witnessed on 65 cases and pinned for the
/// 94 cells it masks on six r4133-only cases.
///
/// r4133 has no getter arm for index 4 (`Version8/Source/PDElements/
/// Reactor.pas:1090-1103`), so the property answers the `'1200'` its
/// `InitPropertyValues` froze at `:1113` while `Create` starts `kvarRating` at
/// 100.0 (`:585`). The port renders the live rating.
///
/// Decks: `controls/fuse/midi_fuse.dss` (`Reactor.rser`, `engines: "r4133"`) for
/// the default and the edit, plus — and this is the second half of the audit
/// settlement — `modes/makeposseq/makeposseq_shunt.dss`, whose `Reactor.rx_kvar`
/// is the pair's one cell that is **not** this row's echo: there r4133 renders
/// its own live `kvarRating`, rounded to five significant digits by the command
/// round-trip inside `MakePosSequence` (`:1145-1201`,
/// `Format(' kvar=%-.5g')` -> `Parser/Edit`). That cell is carved out of the row
/// (`props_norm::ECHO_CARVE_OUTS`) and belongs to RP2.4's display class; this
/// reading pins the port's exact 200/3 next to it, so the carve-out has a
/// witness of its own.
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
         '%-.5g' render and lands on 66.667 (the carved-out cell, RP2.4's display class)"
    );
}

/// `storagecontroller.seasontargets` / `seasontargetslow` — two
/// `EmptyCollectionRender` rows, capi-witnessed on 20 cases and pinned for the 12
/// cells each masks on `controls/relay/relay_generic.dss`, an `engines: "r4133"`
/// case.
///
/// These are the two rows of that category whose sides do NOT both render
/// nothing (see `props_norm::EchoCategory::EmptyCollectionRender`): with
/// `Seasons = 1` r4133's `ReturnSeasonTarget` exits before it emits a single
/// character (`Version8/Source/Controls/StorageController.pas:2445-2449`, reached
/// from `:1010-1011`), while the port renders the live one-season target. The
/// value behind it is the same on both engines — `SeasonTargets[0] := FkWTarget`
/// at `:883-884`, read back as the dispatch target at `:1470` — which is why the
/// readings below assert the rendered array against `kWTarget`/`kWTargetLow`
/// themselves rather than against a literal alone.
///
/// The edit is the discriminator, and it is the mechanism too: with `Seasons = 2`
/// r4133 starts rendering the array as well.
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
// Pins added by RP3.1 (2026-08-24): the witnesses of a DRAFTED ledger entry
// ---------------------------------------------------------------------------
//
// See the module doc's second section. These two are named by
// `props_r4133_replay::LEDGER_ENTRY_PINS`, not by an `EchoRow::witness`.

/// `swtcontrol.delay` — **RP3.1's root cause: r4133 never wires the property.**
///
/// `SwtControl` declares nine properties and the fifth is `Delay`. r4133's
/// `Edit` stores every token in the echo array first
/// (`Version8/Source/Controls/SwtControl.pas:192-193`) and then dispatches on
/// the property number — and the `CASE` has arms for 1, 2, 3, 4, 6, 7, 8 and 9
/// but **none for 5** (`:195-218`), so `delay=` falls through to
/// `ClassEdit(…, ParamPointer - NumPropsthisClass)` as an inherited parameter
/// and `TimeDelay` keeps the 120.0 `Create` gave it (`:310`). The getter is
/// **live** (`:588`, `Format('%-.7g',[TimeDelay])`), so r4133 answers `120` to
/// a deck that asked for 0.25 — which is why the exclusion here cannot be an
/// echo row. dss_capi 0.14.5 wires the property through its typed table
/// (`src/Controls/SwtControl.pas:185`) and this port follows
/// (`elements/control/swt_control/accessors.rs:114`).
///
/// The divergence is **render-only on r4133**: nothing there consumes
/// `TimeDelay`. `Sample`'s queue-pushing body is commented out wholesale
/// (`:484-507`, "Removing because action … and lock are instantaenous") — and
/// so is `LockCommand`'s own declaration (`:39`), so the block would not even
/// compile — `DoPendingAction` likewise (`:396-408`), and `set_States` acts
/// immediately (`:532-549`). Upstream report:
/// `investigations/to_opendss/43-swtcontrol-delay-not-wired.md` (local).
///
/// Deck: `controls/swtcontrol/swtcontrol_time.dss` (`SwtControl.sw`, `delay=0.25`
/// at `:17`), the case behind the drafted entry
/// `r4133-swtcontrol-delay-ignored-time` — 12 in-scope cells, one per step 0..11.
/// Two discriminating readings, because "our render is 0.25" alone would pass
/// against a getter that merely echoed the deck's token: an `edit` proves the
/// setter path drives the same getter, and the unset render below proves the
/// port's *default* is r4133's 120 — which is why `civanlar.dss`, sixteen
/// SwtControls that never type `delay=`, contributes no divergent cell to the
/// census at all.
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

/// `swtcontrol.delay` on the second r4133-gating deck — the witness of the
/// drafted entry `r4133-swtcontrol-delay-ignored-midi`.
///
/// Same mechanism as [`swtcontrol_delay_wires_the_property`]; the entries are
/// per case, so each owes its own reading. Deck:
/// `controls/swtcontrol/midi_swtcontrol.dss` (`SwtControl.sw` on the loop tie,
/// `delay=0.25` at `:124`), 12 in-scope cells over steps 0..11.
///
/// The third corpus deck that types `delay=`,
/// `controls/swtcontrol/swtcontrol_lock.dss`, is `engines: "capi_v0145"`: there
/// the port and the pinned 0.14.5 oracle agree, so it needs no entry and no pin.
#[test]
fn swtcontrol_delay_wires_the_property_on_the_midi_tie() {
    let mut deck = Deck::compile("controls/swtcontrol/midi_swtcontrol.dss");
    assert_eq!(deck.get("SwtControl.sw.Delay"), "0.25");
    deck.cmd("edit SwtControl.sw delay=3.5");
    assert_eq!(deck.get("SwtControl.sw.Delay"), "3.5");
}

// ---------------------------------------------------------------------------
// Pins added by RP3.2 (2026-08-24): the witnesses of four DRAFTED ledger entries
// ---------------------------------------------------------------------------
//
// Same §1.1(e) window as RP3.1's two, a different mechanism: r4133's `kvar`
// getter is wired and live, and it reads the *wrong* live field. These four are
// named by `props_r4133_replay::LEDGER_ENTRY_PINS`, not by an `EchoRow`.
//
// Each pin runs the gate's own sequence for its case — compile, then the one
// `solve` the case's `steps=1` rigor prescribes — and then reads the same
// `? Class.Name.Prop` getter the property walk reads. That solve is the pin's
// own, exactly as it is the gate's: `corpus_gate/runner.rs:348-349` issues
// `dss.command("solve")` once per checkpoint whatever the deck ends on, and none
// of these four decks ends on the solve of its own mode (`windgen_daily.dss:23`
// and the two dynamics decks end at `Set mode=…`, `windgen_snap_delta.dss:9` at
// `Calcvoltagebases`; `windgen_dyn.dss:13` and `windgen_dyn_fault.dss:15` do
// carry a *snapshot* `solve` before their `Set mode=dynamic`, which is not the
// dynamics run the reading is taken after).
//
// The discriminating second reading is an `edit kvar=`, because "our render is
// 986.05" alone would pass against a getter hardwired to the derived base: the
// edit proves the setter path drives the same getter, and on the two kVA-set
// decks it proves more than that (see below).

/// `windgen.kvar` on the daily deck — **RP3.2's root cause: r4133 renders the
/// dispatched Q, not the base kvar the property documents.**
///
/// `WindGen` declares `kvar` as property 11 and documents it as "Specify the
/// **base kvar**" (`Version8/Source/PCElements/WindGen.pas:364-365`, verbatim
/// the `Generator` text at `Generator.pas:396`). The `Edit` CASE has the arm
/// (`:629`, `11: Presentkvar := Parser.DblValue`) and `Set_Presentkvar` stores
/// the value in `kvarBase` (`:2996-3009`) — so, unlike RP3.1, the write path
/// works. The **read** path does not: `GetPropertyValue` arm 11 is
/// `Format('%.6g', [presentkvar])` (`:2896`) and `Get_Presentkvar` returns
/// `WindGenvars.Qnominalperphase * 0.001 * Fnphases` (`:2297-2300`), the
/// dispatched reactive power. The echo store for the slot (`'60'`, `:2446`) is
/// unreachable — `TDSSObject.Get_PropertyValue` dispatches to the virtual
/// `GetPropertyValue` (`DSSObject.pas:117-120`) — so r4133's `0` is a *live*
/// read of the wrong field, which is why the exclusion here cannot be an echo
/// row either.
///
/// r4133 contradicts itself inside its own trunk: `Generator` has the identical
/// getter (`Generator.pas:2402-2405`) and the identical help, yet renders the
/// base (`Generator.pas:3018`, `Format('%.6g', [kvarBase])`). The port does the
/// same (`elements/pc/windgen/accessors.rs:431`, `KVAR => self.kvar_base`, the
/// twin of `pc/generator/accessors.rs:495`). The two engines' *physics* agree:
/// the port ports `SetNominalGeneration` loop-for-loop, `Else kvarCalc := 0`
/// (`WindGen.pas:1320-1321`) included (`windgen/nominal.rs:223-225`), and the
/// probed terminal powers match on all five decks. Upstream report:
/// `investigations/to_opendss/44-windgen-kvar-renders-dispatched-q.md` (local).
///
/// Deck: `modes/windgen/windgen_daily.dss` (`WindGen.w1`, `kW=3000 pf=0.95`,
/// no `kVA=`), the case behind the drafted entry
/// `r4133-windgen-kvar-dispatched-daily` — one in-scope cell, and the pair's
/// worst (`rel 9.86e+02`, `tests/corpus/props_r4133/bins.tsv:303`). No deck
/// types `kvar=` at all: the value is the `SyncUpPowerQuantities` side effect
/// `kW*sqrt(1/pf^2 - 1)` of the deck's own `pf=`.
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

/// `windgen.kvar` on the delta snapshot deck — the witness of the drafted entry
/// `r4133-windgen-kvar-dispatched-delta`, plus the measurement that the pair's
/// one clean deck is clean by *value*, not by mode.
///
/// Same mechanism as [`windgen_kvar_renders_the_base_on_the_daily_deck`]
/// (`Version8/Source/PCElements/WindGen.pas:2896` reading `Get_Presentkvar`,
/// `:2297-2300`); the entries are per case, so each owes its own reading.
/// Deck: `modes/windgen/windgen_snap_delta.dss` (`kW=1500 pf=0.9`), one in-scope
/// cell.
///
/// The third reading is the load-bearing one (the `civanlar.dss` analog of
/// RP3.1): `modes/windgen/windgen_snap.dss` is the fifth windgen deck and the
/// only one the census records **no** cell for — and the reason is that it types
/// `pf=1.0`, so our base is `0` and both engines print `0`. That is a value
/// coincidence, not agreement about the mechanism: type a `kvar=` there and the
/// two diverge like everywhere else (`777` here, `0` on r4133). Nothing about
/// snapshot mode makes the deck clean, so no mode story may be built on it.
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

/// `windgen.kvar` on the WTG3 dynamics deck — the witness of the drafted entry
/// `r4133-windgen-kvar-dispatched-dyn`, and the reading that proves r4133's
/// render is a *stale intermediate* rather than merely the wrong field.
///
/// Deck: `modes/windgen/windgen_dyn.dss` (`kW=1500 kva=1800`, no `pf=`), one
/// in-scope cell. With `kVA` set, `RecalcElementData` takes the other branch
/// (`Version8/Source/PCElements/WindGen.pas:1375-1384`): `kWBase = kVA*|PF|` at
/// `Create`'s `PFNominal = 0.88` (`:917`) gives 1584, and
/// `kvar_base = sqrt(kVA^2 - kWBase^2) = 854.95263026673`.
///
/// The `edit` then re-derives rather than echoing, which is what makes the
/// second reading discriminating here: `Set_Presentkvar` stores 777 and moves
/// `PFNominal` to `1584/sqrt(1584^2+777^2)`, and `RecalcElementData` re-derives
/// the base from `kVA` at that PF — `sqrt(1800^2 - (1800*0.897802095552545)^2)`.
/// Probed on r4133, the same two steps produce the *same* `PF` (`0.897802`) and
/// the same `kvarBase`, and it renders `777` anyway: in dynamics `:1254` skips
/// the Q block, so `Get_Presentkvar` (`:2297-2300`) is still reporting
/// `Set_Presentkvar`'s "init to something reasonable" `1000*777/3` (`:3002`)
/// while the machine's measured terminal Q is `-37087.76 kvar`. Three different
/// numbers for one property; the render tracks none of them.
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

/// `windgen.kvar` on the fault-ride-through twin — the witness of the drafted
/// entry `r4133-windgen-kvar-dispatched-dynfault`.
///
/// `modes/windgen/windgen_dyn_fault.dss` is `windgen_dyn.dss` plus a sustained
/// three-phase `Fault.f1` on the machine terminal, so the LVPL/LVQL path runs
/// through the whole dynamics solve; the base-kvar derivation is identical
/// (`kW=1500 kva=1800`, `Create`'s `pf=0.88`) and so is the render bug
/// (`Version8/Source/PCElements/WindGen.pas:2896` → `:2297-2300`). The entries
/// are per case, so this deck owes its own reading. The solved state is *not*
/// divergent — probed terminal Q is `-29216.72 kvar` on the port against
/// `-29216.67` on r4133 — which is exactly why the exclusion is scoped to the
/// property field and nothing else.
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
// Pins added by RP3.4 (2026-08-24): the witnesses of two DRAFTED ledger entries
// ---------------------------------------------------------------------------
//
// The same §1.1(e) window as RP3.1's two and RP3.2's four, and a third
// mechanism. Here the r4133 getter is wired, live AND reading the field the
// property documents — it renders `Format('%.8g',[1.0/G2])` for `R2`
// (`Version8/Source/PDElements/GICTransformer.pas:723`) — but the field itself
// was mis-derived one procedure earlier: `RecalcElementData`'s `%R` branch
// builds winding 2's conductance from the H-winding percentage,
// `:495 G2 := 100.0 / (FZBase2 * FPctR1);`, the byte-twin of pinned dss_capi
// 0.14.5 `src/PDElements/GICTransformer.pas:441`. So the render is a *live
// computation off a wrong number*, not a parse-store echo (property 14, `%R2`,
// `:136`, does echo `FpctR2` at `:729` and agrees with the port digit for
// digit) — which is why the exclusion is a ledger entry and no
// `PROPS_ECHO_R4133` row. These two are named by
// `props_r4133_replay::LEDGER_ENTRY_PINS`.
//
// The engine side is not this sub-step's: `GOLDEN_REBASE_PLAN.md` G2.5 already
// fixed both lanes (`elements/pd/gic_transformer/solve.rs:66` reads
// `self.pct_r2`) and pinned the capi channel
// (`gic-pct-r2-honoured-{gictransformer,midi}-capi-props`). RP3.4 only stages
// the r4133-channel twins, and these pins hold the port's value until they land.
//
// Neither deck needs a `solve`: `R2` is `RecalcElementData`'s output, stamped at
// `Edit` time, and both decks' cells are `steps=1`. The reading is the same
// `? Class.Name.Prop` the gate's property walk takes.

/// `gictransformer.r2` on the micro deck — **RP3.4's root cause: both gating
/// oracles derive winding 2's conductance from `%R1`.**
///
/// `GICTransformer` declares `R2` as property 8 (`PropertyName^[8] := 'R2'`,
/// `Version8/Source/PDElements/GICTransformer.pas:130`) and stores no ohms field
/// for it: `Edit` arm 8 inverts the typed ohms straight into the conductance
/// (`:300-303`) and `GetPropertyValue` arm 8 inverts it back,
/// `Format('%.8g', [1.0/G2])` (`:723`; `DumpProperties` prints the same at
/// `:663`). When the deck specifies percentages instead — `Edit` arms 13/14 at
/// `:308-309`, which set `FpctRSpecified := TRUE` at `:349`, where arms 7/8 set
/// it FALSE at `:343` — `RecalcElementData` fills the conductances, and its
/// forward branch reads the **H-winding** percentage for both:
///
/// ```text
/// :494    G1 := 100.0 / (FZBase1 * FPctR1);
/// :495    G2 := 100.0 / (FZBase2 * FPctR1);   // <- FPctR2 never reaches the admittance
/// ```
///
/// It is a slip and not a convention: the same procedure's reverse branch
/// restores `FPctR2` from `G2` (`:497-498`), and the two maps are inverses only
/// when the forward one reads `FPctR2`; the creation defaults are independent
/// (`%R1 = %R2 = 0.2`, `:458-459`). The pinned dss_capi 0.14.5 carries the line
/// byte-for-byte (`src/PDElements/GICTransformer.pas:441`), so **both** gating
/// oracles render the un-honoured percentage — which is why RP3.4 stages an
/// r4133 twin of an already-pinned capi divergence rather than a new finding.
/// The port honours `%R2` (`elements/pd/gic_transformer/solve.rs:66`, the
/// `GOLDEN_REBASE_PLAN.md` G2.5 fix) and reads back through the same
/// `INVERSE_VALUE` inversion (`accessors.rs:60`, `R2 => self.g2`). Upstream
/// report: `investigations/to_opendss/07-gictransformer-g2-uses-pctr1.md`
/// (local), already written against r4133 — a twin owes no new report.
///
/// Deck: `asymmetric/gic/gictransformer_gic.dss` (`GICTransformer.tg3`,
/// `%R1=0.2 %R2=0.15 kvll1=345 kvll2=138 mva=300 type=Auto` at `:18-19`), the
/// case behind the drafted entry
/// `gic-pct-r2-honoured-gictransformer-r4133-props` — one in-scope cell.
/// `ZBase2 = 138²/300 = 63.48 Ω`, so ours is `63.48*0.15/100 = 0.09522` against
/// both oracles' `63.48*0.20/100 = 0.12696` (rel 2.50e-01).
///
/// **The discriminating readings**, because "our render is 0.09522" alone would
/// pass against a getter hardwired to the `%R2` product:
///
/// * `%R2=0.3` moves `R2` to `0.19044` — the setter path drives this getter;
/// * `%R1=0.4` then moves `R1` and leaves `R2` **unmoved**, which separates the
///   two engines' *mechanisms*: upstream, whose `G2` is a function of `FPctR1`,
///   would answer `63.48*0.4/100 = 0.25392` here;
/// * `tg2`, the ohms-spec sibling on the same deck, reads back its typed `R2=0.1`
///   through the reverse branch neither revision ever got wrong, and `tg1` — a
///   GSU that types no `R2=` at all — keeps the `Create`-derived `0.38088` both
///   engines agree on. That is why 20 of the corpus's 22 GICTransformers
///   contribute no census cell (this sub-step's `civanlar.dss`).
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

/// `gictransformer.r2` on the 6-substation ring — the witness of the drafted
/// entry `gic-pct-r2-honoured-midi-r4133-props`.
///
/// Same mechanism as [`gictransformer_r2_honours_the_x_winding_percentage`]
/// (`Version8/Source/PDElements/GICTransformer.pas:495` feeding the `:723`
/// getter); the entries are per case, so each owes its own reading. Deck:
/// `asymmetric/gic/gic_midi.dss` (`GICTransformer.tg5`, the same
/// `%R1=0.2 %R2=0.15 kvll1=345 kvll2=138 mva=300 type=Auto` at `:27-28`), one
/// in-scope cell, the same `ZBase2 = 63.48 Ω` and therefore the same pair of
/// numbers.
///
/// The ring's other two GICTransformers are the ohms-spec control again — `tg3`
/// types `R1=0.2 R2=0.1` and `tg1` is a GSU with `R1=0.12` and no `R2=`.
///
/// It carries the **whole** discriminating pair of readings, not just the first
/// half: after `%R1=0.4` both `R2` (unmoved) *and* `R1` (`1.587`) are read, since
/// "unmoved" on its own is also what a no-op edit produces — the RP3.4 audit
/// settlement (2026-08-24) found this test green under `%R9=0.4`, a property that
/// does not exist, and added the second reading.
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
         a correct engine and of an `%R1=` edit that did nothing at all — the RP3.4 audit \
         settlement measured exactly that, rewriting the edit to the non-existent property \
         `%R9=0.4` left this test green while its micro-deck twin, which always carried this \
         control, went red"
    );
}

// ---------------------------------------------------------------------------
// Pins added by RP3.8 (2026-09-02): the five read-only results r4133 renders
// LIVE, and the capi capture that renders nothing
// ---------------------------------------------------------------------------
//
// These three belong to no `EchoRow` and to no drafted `ledger.json` entry: the
// exclusion RP3.8 lands is a `SKIP_PROPS` + `SKIP_PROPS_CAPI_ONLY` row pair
// (`harness/mod.rs`, row group (g)), which masks the **capi** value compare of
// `IndMach012.PF` and `StorageController.kWhTotal`/`kWTotal`/`kWhActual`/
// `kWActual` on 24 gating cases / 84 cells while the r4133 channel keeps
// comparing all five. CLAUDE.md's rule for a deliberate divergence is exclusion
// **plus** an expected-value test, and the two engine-side pins below are that
// test: without them the 84 masked cells would have no holder at all, because a
// regression back to `''` makes the capi compare AGREE again and the gate would
// go green on the wrong value. They are cited from
// `props_r4133_replay::RP38_SUPERSEDED`.
//
// Every expected byte is r4133's own, measured through `epri-worker` on the very
// deck each pin compiles (RP3.8 P0 probe, `tmp/rp38/out_r4133.txt`), and read at
// the same point of the gate's own schedule (`clear` -> `compile` -> N x
// `solve`). The port renders full precision (`float_to_str_ex`, like every other
// double property), so r4133's own `%.6g` / `%-.8g` digits are checked through
// `dss_core::util::fmt_g` — the two-clause display class RP2.4's floor claims on
// the r4133 channel.

/// `indmach012.pf` — **r4133 renders the live power factor; 0.14.5 renders
/// `''`** (`R4133_PROPS_PLAN.md` §RP3.8).
///
/// r4133 arm 5 of `TIndMach012Obj.GetPropertyValue` is
/// `Format('%.6g', [PowerFactor(Power[1, ActiveActor])])`
/// (`Version8/Source/PCElements/IndMach012.pas:1790`, `PowerFactor` =
/// `Common/Utilities.pas:1821`), i.e. the machine's own state variable #21.
/// dss_capi 0.14.5 registers the property `[SilentReadOnly, ReadByFunction]`
/// (`src/PCElements/IndMach012.pas:288-289`) but never assigns its
/// `PropertyOffset`, so `GetObjPropertyValue`'s outer guard
/// (`src/General/DSSObjectHelper.pas:2203-2204`) exits before the read function
/// runs and the capture is `''` — solved or not.
///
/// Decks: the two `both` corpus cases the RP3.8 sweep flagged. Both readings are
/// taken **after `compile`, before the gate's own `solve`**, which is exactly
/// where the probe read r4133 (`tmp/rp38/out_r4133.txt` §C).
///
/// The post-solve reading is the last assertion and it is *port-sourced* in the
/// sense that matters here — it is read at the end of **this** schedule, which
/// opens with a pre-solve `?`. r4133's byte on that same schedule is **not** its
/// expectation: r4133's `?` read recomputes the machine model when the
/// `Iterminal` cache is unstamped and keeps the resulting slip advance, so its
/// own pre-solve read moved its trajectory (`'0.908755'` at step 0); this port
/// recomputes on a throwaway clone and leaves the machine alone
/// (`elements/pc/ind_mach012/accessors.rs::refresh_live_pf`), which the unit pin
/// `pf_is_a_pure_read` holds. Asserting r4133's *perturbed* step-0 byte here
/// would pin the upstream read-that-mutates, which the 2026-08-02 policy forbids
/// reproducing.
///
/// The literal is nevertheless **r4133-corroborated**, which is the stronger
/// statement: read without the perturbing pre-read (`compile` → `solve` → `?`),
/// r4133 answers `'0.908916'` on this deck — exactly `fmt_g(0.908915557341299,
/// 6)` (epri-worker, re-measured at the RP3.8 audit settlement, 2026-09-02). A
/// pure read leaves the two engines on the same trajectory; only r4133's own
/// mutation separates them.
///
/// What a revert breaks: every assertion — a suppressed render answers `''`,
/// which parses as no number at all.
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
        "r4133's own byte on this deck after compile (probe §3.3); 0.14.5 renders ''"
    );
    assert_eq!(
        pre, "0.909167177168387",
        "…and the port prints the full double, not r4133's six digits"
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
        "r4133's own byte on the midi deck (probe §3.3)"
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

/// `storagecontroller.kwhtotal` / `kwtotal` / `kwhactual` / `kwactual` — **the
/// four fleet aggregates r4133 renders live** (`R4133_PROPS_PLAN.md` §RP3.8).
///
/// r4133's `GetPropertyValue` arms `:991-994` call `GetkWhTotal`/`GetkWTotal`/
/// `GetkWhActual`/`GetkWActual` (`Version8/Source/Controls/
/// StorageController.pas:1162-1197`, all `Format('%-.8g', …)`): the two totals
/// re-sum `StorageVars.kWhRating`/`kWRating` over `FleetPointerList` on every
/// call, the two actuals read `FleetkWh`/`FleetkW` (`:188-189` -> `:1019-1042`,
/// the live `kWhStored`/`PresentkW` sums). dss_capi 0.14.5 registers all four
/// `[SilentReadOnly, ReadByFunction]` (`src/Controls/StorageController.pas:
/// 416-423`) with no `PropertyOffset`, so its capture is `''`.
///
/// Two decks, because a single fleet cannot discriminate much:
///
/// * `controls/storagecontroller/storagectrl_peakshave.dss` — a 2-member fleet
///   with round nameplates. Its **`edit`** is the discriminator, and it is
///   r4133's own measurement on this very deck (probe §3.2): re-typing one
///   member's `kwhrated`/`kwrated` moves the two totals immediately, with no
///   fleet rebuild, because the getters re-sum from scratch. A latched total
///   (r4133 writes one into `TotalkWhCapacity`, `:991-992` — a dead store this
///   port does not reproduce) would keep the old numbers.
/// * `…/StorageControllerTechNote/PeakShave/PeakShaveRun.dss` — a 7-member fleet
///   whose actuals are decimal, so it carries the `%-.8g` half: r4133's
///   `'2627.3806'` after compile and `'2627.3929'` after the gate's solve are
///   the port's own double at r4133's precision, three orders inside
///   `props_norm::R4133_DISPLAY_FLOOR`.
///
/// What a revert breaks: every assertion (a suppressed render answers `''`), and
/// specifically the two `Actual` readings, which are the ones that move.
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

    // r4133's own live-edit measurement on this deck (probe §3.2): 9999 + 6000
    // and 2222 + 1500, re-summed on the read.
    deck.cmd("edit storage.sa kwhrated=9999 kwrated=2222");
    assert_eq!(
        four(&mut deck),
        ["15999", "3722", "14799", "0"],
        "the totals are re-summed at every read, never latched — and the stored \
         energy follows the new nameplate (r4133's own four bytes after this very \
         edit, probe §3.2)",
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
        "r4133's `%-.8g` after compile (probe §3.1)"
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

/// **The capi side of RP3.8's skip rows, witnessed from the committed 0.14.5
/// capture** — the five properties render `''` there, on every scenario, and
/// that is the whole reason the `SKIP_PROPS` group (g) rows exist.
///
/// The two engine pins above hold the port's value; nothing held the *oracle's*,
/// and the skip row's justification is a statement about the oracle. The pinned
/// dss-python 0.14.5 cannot be run from a test (no oracle in this binary), but
/// its answer is committed: `tests/golden/props/{indmach012,storagecontroller}.
/// json` are captures of that very oracle (`oracle.engine` names the 0.14.5
/// build), and RP3.8 re-ran `tools/golden/gen_props.py` per class to prove they
/// still regenerate byte-identically. So these 21 cells ARE the capi channel's
/// answer, frozen.
///
/// Non-vacuous in both directions: a writable sibling of each class is asserted
/// non-empty in the same walk, so an all-empty (or missing) capture cannot pass,
/// and the cell count is an equality.
///
/// What a revert breaks: nothing in the engine — this pin fails only if the
/// capture stops saying `''`, i.e. if someone regenerates these files against a
/// newer oracle, which would also retire the skip rows.
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
// RP3.9 — the round-trip residue (`R4133_PROPS_PLAN.md` §RP3.9)
// ---------------------------------------------------------------------------
//
// These pins hold the port's value on the pairs RP2.4's display floor refuses
// (`props_norm::display_is_render`): r4133's number is NOT our number rounded to
// the digits it printed, because the `%.Ng` round trip happened UPSTREAM —
// inside `MakePosSequence`, which writes a command string of 5-significant-digit
// tokens and lets its own `Edit` re-parse them (`Parser.CmdString := S; Edit`),
// so everything derived afterwards is derived from the rounded input. The port
// applies the same conversion as typed `f64` setters (`exec/make_pos_seq.rs:200`,
// WPG.21), keeps the exact value, and diverges by that round trip and nothing
// else.
//
// Each pin therefore names BOTH numbers — ours as a literal render of the live
// getter, r4133's as the vendored census literal — and *reproduces* r4133's from
// ours by running the Pascal chain's own arithmetic (`round_g`, below) and, where
// the engine can be made to walk it, by feeding the port r4133's own token
// through `edit` and reading the same getter back. Verdict on all twenty-seven pairs:
// `PRECISION_ROUNDTRIP` — the port is exact, so nothing is fixed and nothing is
// reproduced (CLAUDE.md: upstream numbers are pinned as the *other* named
// literal, never emitted). Their citation list is
// `props_r4133_replay::RP39_PINS`.

/// FPC `Format('%-.<sig>g')` **as a value**: render `x` through the engine's own
/// FPC `%g` kernel ([`dss_core::util::fmt_g`], the F-FMT seam both lanes share)
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

/// `load.kva` — **r4133 re-derives kVA from the 5-digit `kW`/`kvar` tokens its
/// own `MakePosSequence` wrote; the port derives it from the exact typed
/// conversion** (§RP3.9, chain A; 15 census cells over 13 spellings on all six
/// `modes:makeposseq` decks, none in scope today).
///
/// `TLoadObj.MakePosSequence` builds `Format(' kW=%-.5g  kvar=%-.5g',
/// [kWbase/3.0, kvarbase/3.0])` (`Version8/Source/PCElements/Load.pas:2326`) and
/// feeds the string to its own `Edit` (`:2331-2332`). `Edit` stores the raw token
/// (`:570`) and re-parses it into `kWBase` (`:577`) and `kvarBase` (`:585`, side
/// effect `LoadSpecType := 1` at `:667-671`); `RecalcElementData` then recomputes
/// `kVABase := SQRT(SQR(kWbase) + SQR(kvarBase))` (`:1145`) **from the rounded
/// pair**, and the getter prints that at full precision (`:2352`,
/// `Format('%-g',[kVABase])`). The port runs the same two formulas
/// (`elements/pc/load/accessors.rs:60-70` -> `exec/make_pos_seq.rs:200` ->
/// `elements/pc/load/nominal.rs:315`) on `f64`s, with no text in between.
///
/// `makeposseq_pc.dss` converts twice (deck `:47`/`:49`), so its tokens are
/// `r5(r5(kW/3)/3)`; the other five decks convert once. Both are reproduced
/// here from the port's own post-conversion renders, and then again *inside the
/// engine* by typing r4133's tokens into the port's own `edit`.
///
/// **All thirteen vendored spellings are asserted**, not one per deck: the three
/// worked through the `edit` replay below, and the remaining ten — one row per
/// (deck, load) — through the same arithmetic in the closing loop, so a render
/// that moves on any of the six decks reds this pin.
///
/// What a revert breaks: rounding the conversion (or dropping the `kVABase`
/// recompute) moves the first assertions; a hardwired getter fails the `edit`
/// readings.
#[test]
fn load_kva_after_makeposseq_is_the_exact_typed_conversion() {
    // --- the twice-converted deck -------------------------------------------
    let mut pc = Deck::compile("modes/makeposseq/makeposseq_pc.dss");
    let kw_text = pc.get("Load.ld_wye.kW");
    assert_eq!(
        kw_text,
        render(400.0 / 3.0 / 3.0),
        "ours is the deck's 400 kW divided by three ONCE PER CONVERSION, exactly          (Load.pas:2326's argument, applied as a typed setter — and note (400/3)/3 is not the          same double as 400/9, which the parity lane's %g kernel can tell apart)"
    );
    assert!(
        matches!(kw_text.as_str(), "44.4444444444444" | "44.4444444444445"),
        "the census cell is the default lane's spelling 44.4444444444444; the parity lane's          FPC %g kernel spells the SAME double 44.4444444444445 (util.rs fmt_g_fpc_impl vs          fmt_g_native_impl, the F-FMT seam) — which is why this one byte is asserted as a          value, got {kw_text:?}"
    );
    assert_eq!(pc.get("Load.ld_wye.kvar"), "14.6081824523939");
    assert_eq!(
        pc.get("Load.ld_wye.kVA"),
        "46.7836257309942",
        "ours: the SQRT of the exact kW/9 and kvar/9 (Load.pas:1145 over typed setters)"
    );
    // The load whose spec is xfkVA keeps its pre-conversion nameplate on both
    // engines (`PFChanged = FALSE`, so `:1160` is a no-op; ours: nominal.rs:335).
    assert_eq!(pc.get("Load.ld_delta.kVA"), "300");

    let kw = number(&mut pc, "Load.ld_wye.kW");
    let kvar = number(&mut pc, "Load.ld_wye.kvar");
    // Multiplying our state back by 3 recovers the input of the SECOND
    // conversion; each conversion writes `%-.5g` of a third (Load.pas:2326).
    let kw_t = round_g(round_g(3.0 * kw, 5) / 3.0, 5);
    let kvar_t = round_g(round_g(3.0 * kvar, 5) / 3.0, 5);
    assert_eq!(
        (render(kw_t), render(kvar_t)),
        ("44.443".to_string(), "14.608".to_string()),
        "the two tokens r4133 wrote, which its own `? kW` / `? kvar` echo back"
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
    // One row per (deck, load): the port's census render and r4133's census
    // literal, the latter recomputed from the port's own live kW/kvar by the
    // same `%-.5g` round trip. `makeposseq_pc.dss` is the only deck that runs
    // `makeposseq` twice, so its tokens take the doubled chain.
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
            true,
            &[
                ("ld3", "23.3918128654971", "23.3915732690215"),
                ("ld_1ph", "9.35672514619883", "9.35681930786312"),
            ][..],
        ),
    ] {
        let mut deck = Deck::compile(deck_path);
        for (load, ours, theirs) in loads {
            assert_eq!(
                deck.get(&format!("Load.{load}.kVA")),
                *ours,
                "{deck_path} {load}: the port's census render"
            );
            let kw = number(&mut deck, &format!("Load.{load}.kW"));
            let kvar = number(&mut deck, &format!("Load.{load}.kvar"));
            let (kw_t, kvar_t) = if twice {
                (
                    round_g(round_g(3.0 * kw, 5) / 3.0, 5),
                    round_g(round_g(3.0 * kvar, 5) / 3.0, 5),
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

/// `load.kw`, `load.kvar`, `load.xfkva` — **the four cells where r4133's state
/// IS a `MakePosSequence` token** (§RP3.9, chain A; all on
/// `modes:makeposseq/makeposseq_pc.dss`, none in scope today).
///
/// * `Load.ld_wye.kW`: `Format(' kW=%-.5g',[kWbase/3.0])`
///   (`Version8/Source/PCElements/Load.pas:2326`) -> `Edit` `:577`
///   `kWBase := DblValueNZ` -> `GetPropertyValue` `:2344` `Format('%-g')`. Twice
///   over, so r4133 holds `r5(r5(400/3)/3) = 44.443` where we hold `400/9`.
/// * `Load.ld_delta` is `LoadSpecType 3` (`xfkVA`): `:2328` writes
///   `xfkVA=%-.5g` of `FConnectedkVA/3.0`, `Edit` `:599` hands it to
///   `Set_ConnectedkVA` (`:2166-2171`), and `ComputeAllocatedLoad` re-derives
///   **both** kW and kvar from the rounded nameplate inside the same parse loop
///   (`:2200-2202`: `kWBase := FConnectedkVA*FkVAAllocationFactor*Abs(PFNominal)`,
///   `kvarBase := kWBase*SQRT(1.0/SQR(PFNominal)-1.0)`) — the `kW=`/`kvar=`
///   tokens of `:2326`, parsed moments earlier, are overwritten. Ours:
///   `elements/pc/load/accessors.rs:72-73`, `:581-590` ->
///   `elements/pc/load/nominal.rs:411-419`.
/// * `xfkVA` has **no `GetPropertyValue` arm** at all (`:2337-2366`), so r4133
///   falls through to `Inherited` (`:2364`) and returns `PropertyValue[21]` — the
///   token text `:570` stored, `'55.557'`. We render the `f64`
///   (`elements/pc/load/accessors.rs:275`). That surface is observable only after
///   an *internal* token, i.e. after `makeposseq`: a user's own `xfkva=` text is
///   echoed identically by both engines, which is why the pair needs no echo row.
///
/// What a revert breaks: rounding our conversion moves the first three
/// assertions; dropping the `xfkVA` side effect moves the derived kW/kvar.
#[test]
fn load_kw_kvar_and_xfkva_after_makeposseq_are_the_exact_typed_conversion() {
    let mut pc = Deck::compile("modes/makeposseq/makeposseq_pc.dss");

    // (a) the plain wye load: r4133's kW is the token itself.
    let kw_text = pc.get("Load.ld_wye.kW");
    assert_eq!(
        kw_text,
        render(400.0 / 3.0 / 3.0),
        "ours is the deck's 400 kW divided by three ONCE PER CONVERSION, exactly          (Load.pas:2326's argument, applied as a typed setter — and note (400/3)/3 is not the          same double as 400/9, which the parity lane's %g kernel can tell apart)"
    );
    assert!(
        matches!(kw_text.as_str(), "44.4444444444444" | "44.4444444444445"),
        "the census cell is the default lane's spelling 44.4444444444444; the parity lane's          FPC %g kernel spells the SAME double 44.4444444444445 (util.rs fmt_g_fpc_impl vs          fmt_g_native_impl, the F-FMT seam) — which is why this one byte is asserted as a          value, got {kw_text:?}"
    );
    let kw = number(&mut pc, "Load.ld_wye.kW");
    assert_eq!(
        render(round_g(round_g(3.0 * kw, 5) / 3.0, 5)),
        "44.443",
        "r4133's census cell = the second conversion's own token (Load.pas:2326 -> :577 -> :2344)"
    );

    // (b) the delta load with an xfkVA nameplate.
    assert_eq!(pc.get("Load.ld_delta.xfkVA"), "55.5555555555556");
    assert_eq!(pc.get("Load.ld_delta.kW"), "25.5555555555556");
    assert_eq!(pc.get("Load.ld_delta.kvar"), "10.8866210790363");
    let xf = number(&mut pc, "Load.ld_delta.xfkVA");
    let alloc = number(&mut pc, "Load.ld_delta.allocationfactor");
    let pf = number(&mut pc, "Load.ld_delta.pf");
    assert_eq!(
        (alloc, pf),
        (0.5, 0.92),
        "the other two factors of :2200 — the default allocation factor and the deck's own pf, \
         which LoadSpecType 3 never recomputes"
    );
    let xf_t = round_g(round_g(3.0 * xf, 5) / 3.0, 5);
    assert_eq!(
        render(xf_t),
        "55.557",
        "r4133's census cell: the token text it echoes from PropertyValue[21] (:570 -> :2364)"
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
}

/// `vsource.isc3`, `vsource.puz1`, `vsource.puz0`, `vsource.puz2` — **r4133
/// derives them from the 5-digit `BasekV` its own `MakePosSequence` wrote**
/// (§RP3.9, chain B; 5 + 4 + 4 + 4 census cells, none in scope today).
///
/// `TVsourceObj.MakePosSequence` writes `Format('BasekV=%-.5g ',[kVbase/SQRT3])`
/// (`Version8/Source/PCElements/Vsource.pas:1397`) — plus `R1`/`X1` at
/// `:1398-1399`, which re-parse exactly on every one of these decks, so **the one
/// rounded input is the kV**, not the impedance. `Edit` re-reads it at `:360` and
/// recomputes `ZBase := SQR(kvBase)/BaseMVA` (`:473`, `BaseMVA = 100` per `:610`);
/// `RecalcElementData` then derives `Isc3 := kVBase*1000/SQRT3/Cabs(cmplx(R1,X1))`
/// (`:752`) and `puZ1/puZ0/puZ2 := (R/Zbase, X/Zbase)` (`:837-842`, with
/// `R0 := R1`/`X0 := X1` at `:758-759` and `R2 := R1`/`X2 := X1` at `:459-460`,
/// which is why the three per-sequence cells are the same string). The getters
/// print `%-.5g` (`:1330`) and `[%-.8g, %-.8g]` (`:1340-1342`). Ours applies the
/// typed `SetF64(BASEKV, kv_base/sqrt3())` (`elements/pc/vsource/solve.rs:176-188`)
/// and derives from the full-precision kV (`solve.rs:87-88`, `:149-155`;
/// `accessors.rs:346`).
///
/// `makeposseq_pc.dss` converts twice, so its kV token is
/// `r5(r5(12.47/√3)/√3) = 4.1567`; the chain is asserted both ways there.
///
/// **All four vendored `puZ` spellings are asserted** — `line`/`source`,
/// `pc`/`v2`, `pc`/`source` and `shunt`/`source` — so a render that moves on any
/// of them reds this pin, not just the two that carry an `edit` replay.
///
/// What a revert breaks: rounding our `BasekV` conversion collapses the two
/// numbers into one; a stale `ZBase` or a `puZ` read off a specified-Z path fails
/// the `edit` readings.
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

    // --- twice converted: the deck's second source --------------------------
    let mut pc = Deck::compile("modes/makeposseq/makeposseq_pc.dss");
    assert_eq!(pc.get("Vsource.v2.BasekV"), "4.15666666666667");
    assert_eq!(pc.get("Vsource.v2.Isc3"), "758.90003245438");
    assert_eq!(
        pc.get("Vsource.v2.puZ1"),
        "[5.78774785226323, 17.3632435567897]"
    );
    let kv = number(&mut pc, "Vsource.v2.BasekV");
    let kv5 = round_g(kv, 5);
    assert_eq!(
        kv5,
        round_g(round_g(12.47 / sqrt3, 5) / sqrt3, 5),
        "r4133 rounds once per conversion (deck :47/:49); on this deck both paths meet at 4.1567"
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
    // Same chain, the other two sources of the population: the pc deck's own
    // circuit source (twice converted, R1/X1 = 0.3/1.2) and the once-converted
    // shunt deck's. Their kV tokens are 4.1567 and 7.1996.
    for (label, deck, kv5_text, ours, theirs) in [
        (
            "pc/source",
            &mut pc,
            "4.1567",
            "[1.73632435567897, 6.94529742271588]",
            "[1.7362965, 6.945186]",
        ),
        (
            "shunt/source",
            &mut Deck::compile("modes/makeposseq/makeposseq_shunt.dss"),
            "7.1996",
            "[0.578774785226323, 2.31509914090529]",
            "[0.57876801, 2.315072]",
        ),
    ] {
        assert_eq!(deck.get("Vsource.source.puZ1"), ours, "{label}: ours");
        assert_eq!(deck.get("Vsource.source.puZ0"), ours, "{label}: R0 := R1");
        assert_eq!(deck.get("Vsource.source.puZ2"), ours, "{label}: R2 := R1");
        let kv5 = round_g(number(deck, "Vsource.source.BasekV"), 5);
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

/// `vsource.mvasc3`, `vsource.mvasc1` — **the same 5-digit `BasekV`, squared**
/// (§RP3.9, chain B; 2 + 2 census cells on `modes:makeposseq/makeposseq_pc.dss`,
/// none in scope today).
///
/// `MVAsc3 := SQRT3*kVBase*Isc3/1000` and `MVAsc1 := Factor*kVBase*Isc1/1000`
/// (`Version8/Source/PCElements/Vsource.pas:767-768`), with `Isc1` over
/// `Zs = ((2R1+R0)/3, (2X1+X0)/3)` (`:766`) and `Factor = 1.0` for a 1-phase
/// source (`:688`); `R0 := R1`, `X0 := X1` (`:758-759`), so the two aggregates
/// agree to the last ulp and to every printed digit. Both are `%-.5g`
/// (`:1328-1329`), and both are quadratic in the kV that
/// `MakePosSequence` rounded (`:1397` -> `:360`) — the round trip enters twice.
/// Ours: `elements/pc/vsource/solve.rs:87-102` on the full-precision kV.
///
/// What a revert breaks: rounding our conversion, or losing the `Factor`/`Zs`
/// derivation, moves the literals; a constant getter fails the `edit` reading.
#[test]
fn vsource_mvasc1_and_mvasc3_after_makeposseq_use_the_full_precision_basekv() {
    let sqrt3 = 3.0_f64.sqrt();
    let mut pc = Deck::compile("modes/makeposseq/makeposseq_pc.dss");

    for (element, ours, r4133, after_edit) in [
        (
            "Vsource.source",
            "13.9683362871157",
            "13.969",
            "13.9685603190691",
        ),
        (
            "Vsource.v2",
            "5.46374469117864",
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
        let kv5 = round_g(number(&mut pc, &format!("{element}.BasekV")), 5);
        assert_eq!(render(kv5), "4.1567", "twice through :1397 -> :360");
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

/// `line.b1`, `line.b0` — **r4133's susceptance is computed from a `C1` its own
/// `MakePosSequence` round-tripped through `%-.5g`** (§RP3.9, chain C; 2 + 2
/// census cells on `modes:makeposseq/makeposseq_line.dss`, none in scope today).
///
/// The matrix branch of `TLineObj.MakePosSequence` averages the solved `Yc` into
/// `C1_new` (`Version8/Source/PDElements/Line.pas:1585-1591`) and then scripts it:
/// `Format(' R1=%-.5g  %-.5g  C1=%-.5g Phases=1', …)` (`:1591`),
/// `Parser.CmdString := S; Edit` (`:1597-1598`). `Edit` re-parses property 10 as
/// `c1 := Parser.Dblvalue * 1.0e-9` (`:611`) — the five-digit text is now the
/// stored capacitance — and `RecalcElementData` copies `C0 := C1` for a 1-phase
/// line (`:971`), which is why `b0` carries the identical byte. The getters print
/// `Format('%.7g',[twopi*Basefrequency*C1*1.0e6/FUnitsConvert])` (`:1406`/`:1407`),
/// with `FUnitsConvert = 1.0` after `ResetLengthUnits` + `Units=km`
/// (`:683-689` -> `:2327-2331`, `:624-630`, `Shared/LineUnits.pas:110-114`).
/// Ours computes the same `c1_new` (`elements/pd/line/solve.rs:632-644`) and
/// stores it as a typed `f64` (`:653` -> `exec/make_pos_seq.rs:200`), so its `b`
/// is the UNROUNDED capacitance.
///
/// The chain is observable at both ends: r4133's own `? c1` answers `10.84` /
/// `10.842` where ours answers fifteen digits.
///
/// What a revert breaks: rounding `c1_new`, or losing the `C0 := C1` copy at the
/// next `RecalcElementData`, moves the assertions; the `l_sym`/`l_spc` readings
/// prove the edit is local.
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

/// `autotrans.wdgcurrents` — **a solved-state cell that differs because
/// `makeposseq` moved r4133's whole circuit through `%-.5g` command strings**
/// (§RP3.9, chain C; 1 census cell on `modes:makeposseq/makeposseq_xfmr.dss`,
/// not in scope today).
///
/// The property renders the element's own solved currents:
/// `Version8/Source/PDElements/AutoTrans.pas:1863` ->
/// `GeTAutoWindingCurrentsResult` (`:1662-1690`, `Format('%.7g, (%.5g), ')` at
/// `:1682`) over `GetAllWindingCurrents` (`:1575`). Ours is the same render over
/// the same computation (`elements/pd/auto_trans/yterminal.rs:478-502`), and
/// **before** the conversion the two engines print the same twelve values to
/// every digit — so neither the model, the solver nor the formatter diverges.
/// What diverges is the circuit the deck's `makeposseq` leaves behind: every
/// class scripts its conversion as five-significant-digit text —
/// `AutoTrans.pas:2021`/`:2027`/`:2030` -> `Edit` `:2032`,
/// `PDElements/Transformer.pas:1982`/`:1988`/`:1991` -> `:1993`,
/// `PCElements/Load.pas:2311`/`:2326`, `PCElements/VSource.pas:1397` — while the
/// port applies typed `f64` setters (`exec/make_pos_seq.rs:200-225`).
///
/// The pin rebuilds r4133's post-conversion circuit from the port's own state by
/// re-applying exactly those `%-.5g` tokens, and the solved cell then matches
/// r4133's census byte for byte. The AutoTrans-only replay is the discriminator:
/// it lands on neither engine's number, so the divergence is the deck-wide
/// scripting and not this element's own conversion.
///
/// The census reads its cells after one `solve` beyond the deck's own, which is
/// what the second `solve` below reproduces.
///
/// What a revert breaks: any change to the typed conversion path shows up in the
/// first literal; a broken winding-current computation shows up in both.
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

/// `autotrans.wdgcurrents` on `controls:autotrans/*` — **r4133's numbers are the
/// UNREGULATED circuit's, because its `RegControl` can never tap an `AutoTrans`**
/// (§RP3.12, `UPSTREAM_BUG`, never reproduced; 34 census cells over 8 spellings
/// on the four `controls:autotrans/*` decks, none in scope today).
///
/// r4133 *intends* the combination — the property help says "Name of Transformer
/// or AutoTrans element" (`Version8/Source/Controls/RegControl.pas:296-297`), the
/// `RecalcElementData` lookup retries `'autotrans.' + TransName` (`:684`) and the
/// class check accepts it (`:711`) — but every use of the controlled element goes
/// through an **unchecked `TTransfObj` typecast**: `:926`, `:1026`, `:1296`,
/// `:1370`, `:1479`. `TAutoTransObj = class(TPDElement)`
/// (`Version8/Source/PDElements/AutoTrans.pas:88`) is not a `TTransfObj`
/// (`PDElements/Transformer.pas:92`), and `TAutoWinding` (`AutoTrans.pas:59`)
/// stops agreeing with `TWinding` (`Transformer.pas:62`) after `Rdcohms`, so the
/// regulator reads the winding's `MaxTap` (1.1 pu) where it means `TapIncrement`.
/// `PendingTapChange := Round(BoostNeeded / Increment) * Increment`
/// (`:1249-1250`) then rounds every realistic boost to zero and the control never
/// arms — 0 event-log lines on all four decks, the regulated bus left 2.0 V
/// (2.6 V on the midi deck) outside the band the deck asked for, 5 (11) taps of
/// headroom unused and no diagnostic. `TAutoTransObj`'s own correct
/// `Get_TapIncrement` (`:1568`), `Get_MinTap` (`:1547`), `Get_MaxTap` (`:1554`)
/// and `Get_PresentTap` (`:1478`) are never reached: the cast is unchecked and
/// the accessors are not virtual. Present identically in r3723, r4088 and r4133;
/// DSS-Extensions fixed it by giving both classes a shared
/// `TControlledTransformerObj` base, which is the shape the port has
/// (`elements/pd/transformer/mod.rs` `trait ControlledTransformer`, held as
/// `&mut dyn ControlledTransformer` by
/// `elements/control/reg_control/control_loop.rs`).
///
/// Under the 2026-08-02 policy the port keeps the **regulated** answer in both
/// lanes and nothing is reproduced. This pin names both numbers and derives
/// r4133's from the port's own state through the one input that differs — the
/// tap: disabling the regulator and putting winding 2 back on tap 1 makes the
/// port print r4133's census literal byte for byte, on both decks.
///
/// The discriminating second reading is the ampere-turn identity, which holds on
/// *both* legs: `|I_common| / |I_series| = VBase_series / (VBase_common * tap_c)`,
/// and the two bases are `(kVLL_1 - kVLL_2)/SQRT3` and `kVLL_2/SQRT3`
/// (`AutoTrans.pas:1113-1132`, `:1125` and `:1118`), so the √3 cancels and the
/// prediction is `(kV1 - kV2) / (kV2 * tap_c)` — 2.333333 / 2.262626 here,
/// 0.666667 / 0.623782 on the midi deck. It fails if the winding-current
/// derivation drifts, not only if the tap does, which is what rules
/// `GetAllWindingCurrents` (`:1575`) / `GeTAutoWindingCurrentsResult`
/// (`:1662-1690`, `Format('%.7g, (%.5g), ')` at `:1682`) and our
/// `elements/pd/auto_trans/yterminal.rs` out as the site. Upstream report:
/// `investigations/to_opendss/50-regcontrol-autotrans-ttransfobj-typecast.md`.
///
/// What a revert breaks: a regression in the regulator's AutoTrans path shows up
/// in the first literal and in `TapNum`; a broken winding-current computation
/// shows up in both literals and in the ampere-turn readings.
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
        let (m, p) = (common_over_series(regulated), ampere_turns(kv1, kv2, tap_c));
        assert!(
            (m - p).abs() / p < 1.0e-5,
            "{rel}: regulated ampere-turn balance {m} vs {p} — measured 2.7e-06 here and \
             3.3e-06 on the midi deck, against the render's %.7g magnitudes"
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
            "{rel}: r4133's census cell, byte for byte, from OUR state with the tap it never moves"
        );
        let (m, p) = (common_over_series(unregulated), ampere_turns(kv1, kv2, 1.0));
        assert!(
            (m - p).abs() / p < 1.0e-5,
            "{rel}: unregulated ampere-turn balance {m} vs {p} — measured 2.2e-06 here and \
             3.9e-06 on the midi deck; r4133 derives its winding currents correctly, it just \
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

/// `reactor.x`, `reactor.z`, `reactor.lmh`, `reactor.normamps`,
/// `reactor.emergamps` — **r4133 re-derives all five from the five-digit
/// `kV`/`kvar` tokens its own `MakePosSequence` wrote; the port derives them from
/// the exact typed conversion** (§RP3.9, chain D; 5 spellings / 5 census cells on
/// `modes:makeposseq/makeposseq_shunt.dss`, none in scope today).
///
/// `TReactorObj.MakePosSequence` computes `kvarPerPhase := kvarRating/3.0`
/// (`Version8/Source/PDElements/Reactor.pas:1158`) and `PhasekV := kVRating/
/// SQRT3` (`:1159`), prints the pair as `Format(' kV=%-.5g kvar=%-.5g')`
/// (`:1162`) and feeds the string to its own parser (`:1200-1201`). The `Edit`
/// that follows re-derives everything from the ROUNDED pair, now with
/// `Fnphases = 1`: `kvarPerPhase := kvarRating/Fnphases` (`:657`), `PhasekV :=
/// kVRating` (`:666` — a 1-phase wye takes no √3), `X := SQR(PhasekV)*1000.0/
/// kvarPerPhase` (`:670`), `L := X/twopi/BaseFrequency` (`:671`), `NormAmps :=
/// kvarPerPhase/PhasekV` (`:674`), `EmergAmps := kvarPerPhase/PhasekV*1.35`
/// (`:675`). The getters print `%-.8g` for `X`, `Z` and `LmH` (`:1092`, `:1097`,
/// `:1098`) and FPC `%g` — 15 significant digits — for the two amps (`:1099`,
/// `:1100`).
///
/// The port applies the same two arguments as typed `f64` setters
/// (`elements/pd/reactor/solve.rs:365-375` -> `exec/make_pos_seq.rs:200`) and
/// runs the same five formulas on them (`elements/pd/reactor/solve.rs:36-46`), so
/// nothing is lost and the two amps — printed at full precision on BOTH sides —
/// part company in the 7th digit. That is the entire gap: handed r4133's own
/// tokens, the port's own recalc prints r4133's 15-digit strings bit for bit.
///
/// What a revert breaks: rounding the conversion (or deriving from the rounded
/// nameplate) moves the first block; a hardwired getter fails the `edit`
/// readings, which move the same five cells twice.
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

/// `capacitor.cuf`, `capacitor.normamps`, `capacitor.emergamps` — **the same
/// five-digit `MakePosSequence` re-Edit, one class over** (§RP3.9, chain D; 3
/// spellings / 9 census cells over `makeposseq_shunt`, `makeposseq_ctrl` and
/// `makeposseq_report`, none in scope today).
///
/// `TCapacitorObj.MakePosSequence` writes `Format(' kV=%-.5g kvar=(')`
/// (`Version8/Source/PDElements/Capacitor.pas:798`, `:801`) and one
/// `Format(' %-.5g')` per step of `FkvarRating^[i]/3.0` (`:805-806`), then
/// re-parses the string (`:831-832`). `RecalcElementData` derives from the
/// re-parsed pair at `Fnphases = 1`: `PhasekV := kVRating` (`:606`), `FC^[i] :=
/// 1.0/(w*SQR(PhasekV)*1000.0/(FkvarRating^[1]/Fnphases))` (`:611`),
/// `Ftotalkvar` (`:612`), then `kvarPerPhase := Ftotalkvar/Fnphases` and
/// `NormAmps := kvarPerPhase/PhasekV*1.35` / `EmergAmps := ...*1.8`
/// (`:645-646`). `cuf` prints `FC*1e6` through `GetDSSArray_Real`'s `%-.6g`
/// (`:1098-1101` -> `Version8/Source/Common/Utilities.pas:2600-2607`); the two
/// amps print FPC `%g` (`:1108`, `:1109`).
///
/// Here the 600 kvar divides into a five-digit-exact `200`, so the whole loss is
/// in `kV`: r4133 keeps 7.1996 where the port keeps `12.47/SQRT3`
/// (`elements/pd/capacitor/solve.rs:300-318` typed, derivations at `:34` and
/// `:70-76`). `cuf` is the pair that shows the residue is not a display width at
/// all — `%-.6g` of OUR double is `10.235`, and r4133 prints `10.2349`, the
/// 6-digit print of a different number.
///
/// What a revert breaks: rounding the conversion moves the first block; a
/// hardwired getter fails the `edit` readings.
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

/// `generator.kva`, `generator.kvar`, `generator.maxkvar`, `generator.minkvar` —
/// **r4133 re-derives all four from the five-digit `kW` token its own
/// `MakePosSequence` wrote; the port derives them from the exact typed
/// conversion** (§RP3.9, chain E; 4 spellings / 9 census cells on
/// `modes:makeposseq/makeposseq_pc.dss`, none in scope today).
///
/// `TGeneratorObj.MakePosSequence` writes `Format(' kW=%-.5g  PF=%-.5g',
/// [kWbase/Fnphases, PFNominal])` (`Version8/Source/PCElements/generator.pas:
/// 3059`) under an `If Fnphases>1` guard (`:3057` — which is why the deck's
/// SECOND `makeposseq` at `makeposseq_pc.dss:49` writes only `kV=` and moves
/// nothing here), and re-parses the string (`:3065-3066`). `Edit` `:641` stores
/// `kWBase := Parser.DblValueNZ` and its side-effect case `:699` calls
/// `SyncUpPowerQuantities`, which derives *everything else* from that rounded
/// `kWBase`: `kvarBase := kWBase*sqrt(1.0/Sqr(PFNominal) - 1.0)` (`:3130`),
/// `kvarMax := 2.0*kvarBase` (`:3133`), `kvarMin := -kvarMax` (`:3134`) and,
/// while `kVANotSet`, `kVARating := kWBase*1.2` (`:3137`). All four getters print
/// `%.6g` (`:3018`, `:3019`, `:3020`, `:3021`).
///
/// The port applies the same argument as a typed `f64` setter
/// (`elements/pc/generator/accessors.rs:77` -> `exec/make_pos_seq.rs:200`, the
/// `old_phases > 1` guard at `:66`) and runs the same four formulas on it
/// (`elements/pc/generator/nominal.rs:49-58`), so it keeps `200/3` where r4133
/// keeps `66.667`.
///
/// Which element carries which cell is the `PrpSequence` bookkeeping of `:3060`
/// / `:3061`: `g_kvar` typed `maxkvar`/`minkvar`, so the conversion wrote the
/// exact tokens `40`/`-20` over the synced pair and BOTH engines hold them — the
/// unmoved control below; `g_kva`/`g_mva` typed neither, so their max/min stay
/// derived. `g_kva` and `g_mva` also cleared `kVANotSet` (`:734`), which is why
/// `kva` is a residue cell only on `g_plain` and `g_kvar`.
///
/// What a revert breaks: rounding the conversion moves the first block; a
/// hardwired getter fails the `edit` readings; dropping the `PrpSequence` guards
/// moves the `40`/`-20` control.
#[test]
fn generator_ratings_after_makeposseq_are_the_exact_typed_conversion() {
    let mut deck = Deck::compile("modes/makeposseq/makeposseq_pc.dss");

    // The port's post-conversion nameplate: the deck's 200 kW over three,
    // exactly, and untouched by the second conversion (generator.pas:3057).
    assert_eq!(
        deck.get("Generator.g_plain.kW"),
        render(200.0 / 3.0),
        "ours is generator.pas:3059's argument applied as a typed setter"
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

/// `transformer.normamps`, `transformer.emergamps` — **r4133 divides an exact
/// kVA by a five-digit winding kV its own `MakePosSequence` wrote; the port
/// divides by the exact one** (§RP3.9, chain E; 2 spellings / 2 census cells, on
/// `modes:makeposseq/makeposseq_report.dss` and `…/makeposseq_xfmr.dss`, neither
/// in scope today).
///
/// `TTransfObj.MakePosSequence` prints each winding as
/// `Format(' %-.5g',[kVLL/SQRT3])` (`Version8/Source/PDElements/Transformer.pas:
/// 1982`) and the ratings as `' NormHkVA='+Format(' %-.5g %-.5g',
/// [NormMaxHkVA/FNPhases, EmergMaxHkVA/FNPhases])` (`:1991`), then re-parses the
/// whole string (`:1993-1994`; `Edit` `:512` `InterpretAllkVRatings`).
/// `RecalcElementData` takes `VFactor := Winding^[1].VBase*0.001` (`:1119`, wye)
/// and divides: `NormAmps := NormMaxHkVA/Fnphases/Vfactor` (`:1129`),
/// `EmergAmps := EmergMaxHkVA/Fnphases/Vfactor` (`:1130`); the override getters
/// print `%-.5g` (`:1842`, `:1843`).
///
/// Both numerators are five-digit exact here (`1.1*15000/3 = 5500`,
/// `1.5*1000/3 = 500`), so the entire divergence is the ROUND-TRIPPED
/// denominator — `66.395` for `115/SQRT3`, `7.1996` for `12.47/SQRT3` — which the
/// port keeps exact (`elements/pd/transformer/accessors.rs:169` typed ->
/// `yterminal.rs:110-111`). The same port double `69.4487092048467` is printed
/// `69.449` by r4133 on the five decks that never convert
/// (`examples_full.txt:2783`) and `69.448` only here: the conversion, not the
/// width, is the discriminant.
///
/// What a revert breaks: rounding the conversion moves the first literals; a
/// hardwired getter fails the `edit` readings, which move both the denominator
/// and the numerator.
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
