use super::common::query;
use crate::exec::*;

/// Build the 2-bus regulator micro-circuit the WP5.7 oracle probes used.
fn reg_two_bus(dss: &mut Dss, reg_props: &str) {
    dss.command("New circuit.ctl basekv=12.47 pu=1.0 phases=3 mvasc3=2000");
    dss.command(
        "New transformer.t1 phases=3 windings=2 buses=(sourcebus, b2) \
             conns=(delta wye) kvs=(12.47 4.16) kvas=(5000 5000) xhl=8",
    );
    dss.command(&format!("New regcontrol.r1 transformer=t1 {reg_props}"));
    dss.command("New load.l1 bus1=b2 phases=3 kv=4.16 kw=300 pf=0.95");
    dss.command("Set voltagebases=[12.47, 4.16]");
    dss.command("CalcVoltageBases");
}

/// WP5.7: the live control loop drives the regulator to the oracle's tap.
/// Oracle probe (pinned dss-python): iterations=6, winding-2 tap=1.01875.
#[test]
fn control_loop_regulates_two_bus_to_oracle_tap() {
    let mut dss = Dss::new();
    reg_two_bus(&mut dss, "winding=2 vreg=122 band=0.0001 ptratio=20");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved);
    assert_eq!(ckt.solution.iteration, 6);
    let taps = dss.transformer_taps();
    assert_eq!(taps[0].0, "t1");
    assert!(
        (taps[0].1[1] - 1.01875).abs() < 1e-12,
        "winding-2 tap = {}",
        taps[0].1[1]
    );
}

/// WP7.1 step 4: a *direct* `Transformer.X.Taps=` edit moves the winding tap
/// without going through the regulator, so the control's parse-time tap snapshot
/// goes stale. Pascal `Get_TapNum` reads the live transformer
/// `PresentTap[TapWinding]`; the executive `regcontrol_tap_numbers` view must
/// too — else the reported tap number stays 0. Reproduces the IEEE13 geometry
/// scripts' manual-tap + `controlmode=off` epilogue (oracle reports +10, not 0).
#[test]
fn regcontrol_tap_number_reads_live_transformer_after_manual_tap() {
    let mut dss = Dss::new();
    reg_two_bus(&mut dss, "winding=2 vreg=122 band=2 ptratio=20");
    // Position winding-2 at +10 taps (1.0625 = 1.0 + 10·0.00625) and freeze
    // controls so nothing moves it back.
    dss.command("Transformer.t1.Taps=[1.0 1.0625]");
    dss.command("Set controlmode=OFF");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        dss.regcontrol_tap_numbers(),
        vec![("r1".to_string(), 10)],
        "TapNumber must reflect the live winding tap, not the stale snapshot",
    );
    let taps = dss.transformer_taps();
    assert!(
        (taps[0].1[1] - 1.0625).abs() < 1e-12,
        "tap={}",
        taps[0].1[1]
    );
}

/// WP5.7 step 5: a control that cannot settle within `maxcontroliter`
/// stops with the 485 warning and aborts the solution. Oracle probe:
/// `maxcontroliter=2` + `maxtapchange=1` → iterations=4, tap=1.00625,
/// error 485; the next *external* command resets the abort flag
/// (CAPI `Text_Set_Command`).
#[test]
fn max_control_iterations_exceeded_warns_and_aborts() {
    let mut dss = Dss::new();
    reg_two_bus(
        &mut dss,
        "winding=2 vreg=122 band=2 ptratio=20 maxtapchange=1",
    );
    dss.command("Set maxcontroliter=2");
    dss.command("Solve");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.starts_with("Warning Max Control Iterations Exceeded.")),
        "{:?}",
        dss.errors()
    );
    let ckt = dss.circuit().unwrap();
    assert_eq!(ckt.solution.iteration, 4);
    assert!(ckt.solution.solution_abort);
    let taps = dss.transformer_taps();
    assert!(
        (taps[0].1[1] - 1.00625).abs() < 1e-12,
        "winding-2 tap = {}",
        taps[0].1[1]
    );
    // External commands reset the abort (the oracle solves again and
    // exceeds again rather than reporting "Solution aborted.").
    dss.command("Get hour");
    assert!(!dss.circuit().unwrap().solution.solution_abort);
}

/// Build the 2-bus + line + load + two-generator micro-circuit the
/// GenDispatcher oracle probes used.
fn gen_disp_two_bus(dss: &mut Dss, gd_props: &str) {
    dss.command("New circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=5000 pf=0.95");
    dss.command("New generator.g1 bus1=b1 phases=3 kv=12.47 kw=1000 pf=1.0 model=1");
    dss.command("New generator.g2 bus1=b1 phases=3 kv=12.47 kw=1000 pf=1.0 model=1");
    dss.command(&format!(
        "New gendispatcher.gd1 element=line.l1 terminal=1 {gd_props}"
    ));
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
}

/// WP6.8: the control loop's GenDispatcher redispatches its generators so
/// the monitored line power approaches `kWLimit`. Oracle probe (pinned
/// dss-python): equal weights → g1 = g2 = 1511.569498763734 kW.
#[test]
fn gendispatcher_redispatches_to_oracle() {
    let mut dss = Dss::new();
    gen_disp_two_bus(
        &mut dss,
        "kwlimit=2000 kwband=100 genlist=[g1,g2] weights=[1,1]",
    );
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for g in ["g1", "g2"] {
        let (kw, _kvar) = dss.generator_kw_kvar(g).unwrap();
        assert!((kw - 1511.569498763734).abs() < 1e-6, "{g} kW = {kw}");
    }
}

/// Weighted redispatch [3, 1]: g1 = 1767.3542481456006, g2 = 1255.7847493818672.
#[test]
fn gendispatcher_respects_weights_oracle() {
    let mut dss = Dss::new();
    gen_disp_two_bus(
        &mut dss,
        "kwlimit=2000 kwband=100 genlist=[g1,g2] weights=[3,1]",
    );
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (kw1, _) = dss.generator_kw_kvar("g1").unwrap();
    let (kw2, _) = dss.generator_kw_kvar("g2").unwrap();
    assert!((kw1 - 1767.3542481456006).abs() < 1e-6, "g1 kW = {kw1}");
    assert!((kw2 - 1255.7847493818672).abs() < 1e-6, "g2 kW = {kw2}");
}

/// No GenList → dispatch every enabled generator (uniform weights); same
/// result as the explicit equal-weight list.
#[test]
fn gendispatcher_no_list_dispatches_all_gens() {
    let mut dss = Dss::new();
    gen_disp_two_bus(&mut dss, "kwlimit=2000 kwband=100");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for g in ["g1", "g2"] {
        let (kw, _) = dss.generator_kw_kvar(g).unwrap();
        assert!((kw - 1511.569498763734).abs() < 1e-6, "{g} kW = {kw}");
    }
}

/// WP6.8: the QDiff (kvar) redispatch path, exercised end-to-end. The gens
/// run at `pf=0.95` so they carry a dispatchable `kvarBase`, and both
/// `kWLimit` and `kvarLimit` bind. Oracle probe (pinned dss-python): equal
/// weights → g1 = g2 = (1509.8126154343354 kW, 591.2600618943429 kvar).
#[test]
fn gendispatcher_redispatches_kvar_to_oracle() {
    let mut dss = Dss::new();
    dss.command("New circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=5000 pf=0.95");
    dss.command("New generator.g1 bus1=b1 phases=3 kv=12.47 kw=1000 pf=0.95 model=1");
    dss.command("New generator.g2 bus1=b1 phases=3 kv=12.47 kw=1000 pf=0.95 model=1");
    dss.command(
        "New gendispatcher.gd1 element=line.l1 terminal=1 \
             kwlimit=2000 kwband=100 kvarlimit=500 genlist=[g1,g2] weights=[1,1]",
    );
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for g in ["g1", "g2"] {
        let (kw, kvar) = dss.generator_kw_kvar(g).unwrap();
        assert!((kw - 1509.8126154343354).abs() < 1e-6, "{g} kW = {kw}");
        assert!((kvar - 591.2600618943429).abs() < 1e-6, "{g} kvar = {kvar}");
    }
}

/// WP6.8: the monitored *terminal* is honored (not hard-wired to 1). A later
/// `terminal=2` overrides the helper's `terminal=1`; terminal 2 of the line
/// sits at the load/gen bus, so the measured power drives `PDiff` strongly
/// negative and both gens floor at `Max(1.0, …)` — a result distinct from
/// terminal 1's 1511.57 kW, which pins that the terminal index is read.
/// Oracle probe (pinned dss-python): g1 = g2 = 1.0 kW.
#[test]
fn gendispatcher_honors_monitored_terminal() {
    let mut dss = Dss::new();
    gen_disp_two_bus(
        &mut dss,
        "kwlimit=2000 kwband=100 terminal=2 genlist=[g1,g2] weights=[1,1]",
    );
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for g in ["g1", "g2"] {
        let (kw, _) = dss.generator_kw_kvar(g).unwrap();
        assert!((kw - 1.0).abs() < 1e-6, "{g} kW = {kw}");
    }
}

/// A StorageController on a circuit with **no** Storage element resolves an
/// empty fleet (lazily, at the first Sample) and solves as an inert no-op. The
/// Pascal parse-time 37201 ("No unassigned Storage Elements") for a Storage-less
/// circuit is NOT_PORTED (the fleet now resolves at Sample, like GenDispatcher's
/// empty gen list — see the storage_controller module doc), so the circuit
/// compiles and solves cleanly with no errors.
#[test]
fn storagecontroller_empty_fleet_solves_as_noop() {
    let mut dss = Dss::new();
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kw=3000 pf=0.95");
    dss.command("new storagecontroller.sc1 element=line.l1 terminal=1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    // No errors from the control loop; the circuit converged.
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(dss.circuit().unwrap().solution.converged_flag);
}

/// A small PeakShave deck with the **real** control sweep (not the mock env):
/// the fleet's target is below what it can deliver, so each battery dispatches
/// to its `kWrated` cap — a *unique*, path-insensitive converged dispatch. Pins
/// the active discharge (`kW`/`State`) end-to-end through the dispatch wiring
/// (this is the active-dispatch electrical pin the golden defers here, since the
/// converged voltage only settles to the solver's own tolerance).
#[test]
fn storagecontroller_peakshave_dispatch() {
    let mut dss = Dss::new();
    for c in [
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
        "new Load.ld bus1=b phases=3 kv=12.47 kw=6000 pf=1.0 model=1",
        "new Storage.sa bus1=b phases=3 kV=12.47 kWrated=2000 kVA=2000 kWhrated=4000 \
         %stored=80 %idlingkW=0 pf=1.0",
        "new Storage.sb bus1=b phases=3 kV=12.47 kWrated=2000 kVA=2000 kWhrated=4000 \
         %stored=80 %idlingkW=0 pf=1.0",
        "new StorageController.sc element=Line.l1 terminal=1 modedis=peakshave \
         monphase=avg kwtarget=2000 %reserve=20",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "set maxcontroliter=50",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(dss.circuit().unwrap().solution.converged_flag);

    // Each battery dispatched to exactly its kWrated cap (Min(kWrating, …)).
    for nm in ["sa", "sb"] {
        dss.command(&format!("? storage.{nm}.kW"));
        let kw: f64 = dss.result().parse().unwrap();
        assert!((kw - 2000.0).abs() < 1e-9, "storage {nm} kW = {kw}");
        dss.command(&format!("? storage.{nm}.State"));
        assert_eq!(dss.result(), "Discharging", "storage {nm} state");
    }
}

/// PeakShave with a *reachable* target: the controller drives the monitored line
/// power into the target band. Pins the closed-loop "hold the target" behavior
/// (the converged in-band point floats within the solver tolerance, so this
/// checks the band, not an exact value).
#[test]
fn storagecontroller_peakshave_holds_target() {
    let mut dss = Dss::new();
    for c in [
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
        "new Load.ld bus1=b phases=3 kv=12.47 kw=6000 pf=1.0 model=1",
        "new Storage.sa bus1=b phases=3 kV=12.47 kWrated=3000 kVA=3000 kWhrated=6000 \
         %stored=80 %idlingkW=0 pf=1.0",
        "new StorageController.sc element=Line.l1 terminal=1 modedis=peakshave \
         monphase=avg kwtarget=4000 %reserve=20",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "set maxcontroliter=50",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(dss.circuit().unwrap().solution.converged_flag);

    // The monitored Line.l1 terminal-1 power is pulled into the band around the
    // 4000 kW target (half-band = %kWBand/200·target = 2/200·4000 = 40 kW; allow
    // the line loss + one in-band residual).
    let snaps = dss.snapshot_elements();
    let line = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("Line.l1"))
        .expect("Line.l1 snapshot");
    // powers: kW/kvar interleaved per conductor; terminal-1 kW = conductors 0,2,4.
    let line_kw = line.powers[0].re + line.powers[1].re + line.powers[2].re;
    assert!(
        (line_kw - 4000.0).abs() < 100.0,
        "monitored line power {line_kw} not held near the 4000 kW target"
    );
    // The single battery is discharging (it has the headroom to hold the target).
    dss.command("? storage.sa.State");
    assert_eq!(dss.result(), "Discharging");
}

// ---------------------------------------------------------------------------
// RP3.7 — the per-phase SwtControl switch-state render (`Normal` / `State`)
// ---------------------------------------------------------------------------

/// Compile a vendored corpus deck by absolute path and return the live engine.
///
/// The two decks read below (`controls/swtcontrol/swtcontrol_lock.dss` and
/// `modes/makeposseq/makeposseq_ctrl.dss`) are in-repo synthetic decks that
/// write no file, so no directory guard is needed; the vendored
/// `electricdss-tst` decks read by the second pin are likewise read-only
/// (`civanlar.dss` has no active `export`/`show`/`save`). Never `.inputs/` —
/// the corpus tree is the one the live gate reads (CLAUDE.md).
fn compile_corpus_deck(rel: &str) -> Dss {
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel);
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));
    assert!(dss.errors().is_empty(), "{rel}: {:?}", dss.errors());
    dss
}

/// **The witness for the five landed `capi_v0145` ledger entries**
/// `swtcontrol-per-phase-state-{lock,makeposseq,ieee519-tmode,ieee519-varload,
/// ieee519-matlab}-capi-props` (cause `swtcontrol-per-phase-state-render`).
///
/// r4133 keeps the switch state per phase — `FPresentState`/`FNormalState :
/// pStateArray` (`Version8/Source/Controls/SwtControl.pas:37-38`), one slot per
/// phase from `Create` (`:299-307`) — and its two getters render **one token per
/// CONTROLLED-ELEMENT phase**: property 6 `Normal` at `:589-599`, property 7
/// `State` at `:600-610`. The pinned dss_capi 0.14.5 has no per-phase model at
/// all and renders one bare word, so RP3.7's port of the r4133 model reds those
/// five capi-gated cases on exactly these cells. The entries pin both sides of
/// that divergence; this pin is what says the port's side is **right** rather
/// than merely stable (the obligation the RP3.6 audit created, 2026-08-29).
///
/// Every expected value below was read off the r4133 DLL (Version 11.0.0.1,
/// `tools/opendss/bin/r4133`) on 2026-09-02 over the gated decks themselves —
/// transcripts `tmp/rp37/out_b2_r4133.txt` (the five capi decks) — not derived
/// from the port. The three shapes are the three the ledger entries carry:
///
/// * **3-phase, ganged closed** (`swtcontrol_lock.dss`, and the two IEEE_519
///   controls) → `[closed, closed, closed, ]`;
/// * **the same deck after the manifest's locked `post`** — `action=open` under
///   `lock=yes` is refused by r4133's name-keyed guard (`:416-417`), so both
///   fields stay closed for all 12 gated steps;
/// * **1-phase after `MakePosSequence`** (`makeposseq_ctrl.dss`) →
///   `[closed, ]`, the whole of the pair's `'[closed, ]'` census spelling.
///
/// The `1 / 2 / 3` sweep at the end is the discriminator: the token count
/// follows the controlled element's phase count, so this pin cannot pass against
/// a hardwired three-token string.
#[test]
fn swtcontrol_state_renders_one_token_per_controlled_phase() {
    const THREE: &str = "[closed, closed, closed, ]";

    // (1) `controls:swtcontrol/swtcontrol_lock.dss` — the deck the entry
    // `swtcontrol-per-phase-state-lock-capi-props` pins on 12 steps, through
    // BOTH its probes and its full property compare.
    let mut dss = compile_corpus_deck("controls/swtcontrol/swtcontrol_lock.dss");
    assert_eq!(query(&mut dss, "SwtControl.sw.Normal"), THREE);
    assert_eq!(query(&mut dss, "SwtControl.sw.State"), THREE);
    // The manifest's post: `Locked` refuses an `action=` write on both engines
    // (r4133 `:416-417` keys the guard on the property NAME), so neither field
    // moves and `Lock` itself is not part of the divergence.
    dss.command("edit swtcontrol.sw action=open");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "SwtControl.sw.Normal"), THREE);
    assert_eq!(query(&mut dss, "SwtControl.sw.State"), THREE);
    assert_eq!(query(&mut dss, "SwtControl.sw.Lock"), "Yes");

    // (2) `modes:makeposseq/makeposseq_ctrl.dss` — the switched line drops to
    // ONE phase, so the same getter renders one token
    // (`swtcontrol-per-phase-state-makeposseq-capi-props`).
    let mut psq = compile_corpus_deck("modes/makeposseq/makeposseq_ctrl.dss");
    assert_eq!(query(&mut psq, "SwtControl.swc.Normal"), "[closed, ]");
    assert_eq!(query(&mut psq, "SwtControl.swc.State"), "[closed, ]");

    // (3) The IEEE_519 control block, rebuilt rather than compiled: the vendored
    // deck ends in `export monitor` + `show monitor`, so running it here would
    // write into the corpus tree. Its two declarations are byte-identical in all
    // three copies (`IEEE_519.DSS:45-46`) and this is the line that matters —
    // `SwitchedTerm=2` and `lock=open`, which `InterpretYesNo` reads as NOT-yes,
    // so the control is unlocked and still renders three closed tokens.
    let mut ieee = Dss::new();
    for c in [
        "New circuit.h519 basekv=12.47 phases=3 bus1=src",
        "New Line.SW_3 bus1=src bus2=n3 phases=3 switch=yes",
        "New Swtcontrol.SW_3_Ctrl basefreq=60 Delay=0.0 Action=Close \
         SwitchedObj=Line.SW_3 SwitchedTerm=2 lock=open enabled=y",
    ] {
        ieee.command(c);
    }
    assert!(ieee.errors().is_empty(), "{:?}", ieee.errors());
    assert_eq!(query(&mut ieee, "SwtControl.SW_3_Ctrl.Normal"), THREE);
    assert_eq!(query(&mut ieee, "SwtControl.SW_3_Ctrl.State"), THREE);
    assert_eq!(query(&mut ieee, "SwtControl.SW_3_Ctrl.Lock"), "No");

    // (4) The discriminator: the token count is the CONTROLLED element's phase
    // count, not a constant. One control per width, all three on one circuit.
    let mut widths = Dss::new();
    for c in [
        "New circuit.w basekv=12.47 phases=3 bus1=src",
        "New Line.l1 bus1=src.1 bus2=b1.1 phases=1 switch=yes",
        "New Line.l2 bus1=src.1.2 bus2=b2.1.2 phases=2 switch=yes",
        "New Line.l3 bus1=src bus2=b3 phases=3 switch=yes",
        "New SwtControl.c1 switchedobj=line.l1 switchedterm=1",
        "New SwtControl.c2 switchedobj=line.l2 switchedterm=1",
        "New SwtControl.c3 switchedobj=line.l3 switchedterm=1",
    ] {
        widths.command(c);
    }
    assert!(widths.errors().is_empty(), "{:?}", widths.errors());
    assert_eq!(query(&mut widths, "SwtControl.c1.State"), "[closed, ]");
    assert_eq!(
        query(&mut widths, "SwtControl.c2.State"),
        "[closed, closed, ]"
    );
    assert_eq!(query(&mut widths, "SwtControl.c3.State"), THREE);
}

/// **The holder for the pair's 80 in-scope cells** — the three r4133-gating
/// decks that carry every one of them, and that **no oracle channel can witness
/// today**: the pinned dss_capi 0.14.5 cannot render the array at all, and the
/// r4133 property compare stays masked until RP4.1 (plan §1.1(e)). Plan §1.1(c)
/// asks for exactly this pin.
///
/// `swtcontrol.normal` and `swtcontrol.state` are 59 census cells each, 40 of
/// them in scope, and the split is these three decks: `midi_swtcontrol.dss`
/// (1 control × 12 steps), `swtcontrol_time.dss` (1 × 12) and `civanlar.dss`
/// (16 × 1) = 40. The other 19 are the capi-gated cases the ledger entries pin.
///
/// Every value below was read off the r4133 DLL on 2026-09-02, right after
/// `compile`, on these very decks (`tmp/rp37/out_b2_r4133b.txt`). Note what the
/// two `duty`-mode decks say: `Normal` stays `[closed, closed, closed, ]` after
/// the manifest's `action=open` fires, because `normal=closed` was typed
/// explicitly, so r4133's `NormalStateSet` latch (`SwtControl.pas:42`, `:220-228`)
/// is already TRUE and the Edit supplemental does not re-seed `Normal` from the
/// now-open `State`.
///
/// `civanlar.dss` is the load-bearing one — the one deck where the array's
/// CONTENTS, not just its width, vary: sixteen tie switches, thirteen closed
/// and three open.
///
/// Its `Normal` is all-CLOSED on every one of the sixteen, and the mechanism is
/// the Edit supplemental, not an untyped property (RP3.7 audit settlement,
/// 2026-09-02 — the earlier wording here said `Create`'s initialisation, which
/// is not what answers). Every `New` line declares `Action=c` (`civanlar.dss`
/// `:51-66`); arm 3 runs `InterpretSwitchState` and then the
/// `{Supplemental Actions}` block (`SwtControl.pas:219-228`) copies the
/// now-CLOSED Present into Normal and latches `NormalStateSet` — which is
/// exactly why the three later `edit swtcontrol.<tie> action=o` lines
/// (`:68-70`) move `State` and cannot move `Normal`. Had a `New` line said
/// `Action=o`, that same `Normal` would render `[open, open, open, ]`.
#[test]
fn swtcontrol_state_renders_per_phase_on_the_r4133_only_decks() {
    const CLOSED3: &str = "[closed, closed, closed, ]";
    const OPEN3: &str = "[open, open, open, ]";

    // (1)+(2) The two `duty`-mode micro-decks: 12 steps each, one control each.
    for deck in [
        "controls/swtcontrol/midi_swtcontrol.dss",
        "controls/swtcontrol/swtcontrol_time.dss",
    ] {
        let mut dss = compile_corpus_deck(deck);
        assert_eq!(query(&mut dss, "SwtControl.sw.Normal"), CLOSED3, "{deck}");
        assert_eq!(query(&mut dss, "SwtControl.sw.State"), CLOSED3, "{deck}");
        // The manifest's post arms the delayed open; after it fires `State`
        // follows per phase and `Normal` does not move.
        dss.command("edit swtcontrol.sw action=open");
        dss.command("solve");
        assert!(dss.errors().is_empty(), "{deck}: {:?}", dss.errors());
        assert_eq!(query(&mut dss, "SwtControl.sw.State"), OPEN3, "{deck}");
        assert_eq!(query(&mut dss, "SwtControl.sw.Normal"), CLOSED3, "{deck}");
    }

    // (3) `civanlar.dss` — 16 controls, 16 cells per property, all in scope.
    let mut civ = compile_corpus_deck(
        "electricdss-tst/Version8/Distrib/Examples/civinlar model/civanlar.dss",
    );
    // The three ties the deck declares `Action=o`; every other one is `Action=c`.
    const OPEN_TIES: [&str; 3] = ["5_11", "7_16", "10_14"];
    const TIES: [&str; 16] = [
        "1_4", "2_8", "3_13", "4_5", "4_6", "5_11", "6_7", "7_16", "8_9", "8_10", "9_11", "9_12",
        "10_14", "13_14", "13_15", "15_16",
    ];
    let mut open_seen = 0;
    for tie in TIES {
        let want = if OPEN_TIES.contains(&tie) {
            open_seen += 1;
            OPEN3
        } else {
            CLOSED3
        };
        assert_eq!(
            query(&mut civ, &format!("SwtControl.{tie}.State")),
            want,
            "{tie}"
        );
        // `Normal` is untyped on every one of them, so all sixteen answer
        // `Create`'s all-CLOSED array — including the three that are open.
        assert_eq!(
            query(&mut civ, &format!("SwtControl.{tie}.Normal")),
            CLOSED3,
            "{tie}"
        );
    }
    assert_eq!(open_seen, 3, "civanlar declares exactly three open ties");
}
