//! Exec-layer integration tests for the Storage element (WP7.4 step 1):
//! the power-flow solve path, the SOC integration over a daily run, and the
//! deferred-behavior guards.

use crate::exec::Dss;

/// A minimal source -> line -> Storage circuit solves cleanly (the power-flow
/// injection path end to end), discharging into the feeder.
#[test]
fn storage_snapshot_solves_clean() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command(
        "New Storage.s1 bus1=b phases=3 kV=12.47 kWrated=500 kWhrated=1000 \
         state=discharging %discharge=50",
    );
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "unexpected errors: {:?}",
        dss.errors()
    );
    assert!(dss.circuit().expect("circuit").is_solved);
}

/// A daily run advances the integrated state of charge (`UpdateStorage` runs in
/// the time-step cleanup): a 100 kW / 200 kWh battery discharging at 50 kW with
/// a 10% reserve depletes to the reserve and flips to Idling. Guards that the
/// SOC hook is actually wired into the daily loop (not just the snapshot path).
#[test]
fn storage_daily_run_depletes_soc() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command(
        "New Storage.s1 bus1=b phases=3 kV=12.47 kWrated=100 kWhrated=200 \
         state=discharging %discharge=50 %reserve=10 %IdlingkW=1 \
         %EffDischarge=90 %EffCharge=90 pf=1.0",
    );
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("set mode=daily number=6 stepsize=1h");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "unexpected errors: {:?}",
        dss.errors()
    );
    // Depleted to the 20 kWh reserve and turned off.
    dss.command("? Storage.s1.kWhStored");
    assert_eq!(dss.result(), "20");
    dss.command("? Storage.s1.State");
    assert_eq!(dss.result(), "Idling");
}

/// Grid-forming mode (`ControlMode=GFM`, WPG.13): the Storage becomes an internal
/// balanced voltage source (`CalcGFMVoltage` at `BaseV`) behind its `CalcGFMYprim`
/// short-circuit impedance, injecting with the sources. With no local load its
/// internal voltage matches the grid, so it delivers ~0 kW and the bus sits at
/// 1.0 pu — bit-matching the pinned oracle (converges in 2 iterations).
#[test]
fn storage_gfm_mode_solves() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command(
        "New Storage.s1 bus1=b phases=3 kV=12.47 kWrated=500 kWhrated=1000 \
         state=discharging ControlMode=GFM",
    );
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "GFM solve must not error now that it is ported, got: {:?}",
        dss.errors()
    );
    let ckt = dss.circuit().expect("circuit exists");
    assert!(ckt.is_solved, "GFM circuit did not converge");
}

/// The **dynamics-mode** GFM branch (`DoDynamicMode`/`IntegrateStates` GFM) is
/// ported (WPG.17). A discharging grid-forming Storage black-STARTS an islanded
/// section in `set mode=dynamics`: the droop lifts the island from 0 V and the
/// inverter forms the grid, delivering ~400 kW into the island load. The oracle
/// pins (dss-python 0.15.7, 60x 1ms steps) are islbus ~0.977 pu and the Storage
/// delivering ~-400.7 kW. (SafeVoltage=0 is mandatory — the default 100 blocks the
/// black start from 0 V.) The full monitor trajectory is bit-checked against the
/// oracle by the live-gate deck `tests/corpus/controls/gfm/gfm_dynamics.dss`.
#[test]
fn storage_gfm_dynamics_matches_oracle() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New Circuit.gfm_dyn basekv=4.16 phases=3 bus1=sourcebus");
    dss.command(
        "New Line.feeder bus1=sourcebus bus2=mainbus phases=3 r1=0.3 x1=0.6 c1=0 \
         length=1 units=km",
    );
    dss.command("New Line.sw1 bus1=mainbus bus2=islbus phases=3 switch=yes");
    dss.command(
        "New Transformer.tsto phases=3 windings=2 buses=(stobus islbus) \
         conns=(delta wye) kvs=(0.48 4.16) kvas=(1000 1000) XHL=0.5",
    );
    dss.command(
        "New Storage.batt phases=3 conn=delta bus1=stobus kV=0.48 kva=800 \
         kWrated=800 kWhrated=6000 %stored=100 %reserve=20 %IdlingkW=1 %R=50 %X=50 \
         State=DISCHARGING kP=0.3 KVDC=0.700 PITol=0.1 SafeVoltage=0 ControlMode=GFM",
    );
    dss.command("New Load.isl phases=3 bus1=islbus kV=4.16 kW=400 kvar=80 model=1");
    dss.command("New Monitor.msv element=Storage.batt terminal=1 mode=3");
    dss.command("Set voltagebases=[4.16 0.48]");
    dss.command("Calcvoltagebases");
    dss.command("open line.sw1 terminal=1");
    dss.command("solve"); // steady snapshot (islanded GFM) — converges
    assert!(
        dss.errors().is_empty(),
        "islanded GFM snapshot: {:?}",
        dss.errors()
    );
    dss.command("Set mode=dynamics stepsize=0.001 number=60 maxiterations=30");
    dss.command("solve"); // dynamics GFM black start — ported, must NOT abort
    assert!(
        dss.errors().is_empty(),
        "dynamics GFM must solve now that it is ported: {:?}",
        dss.errors()
    );

    let ckt = dss.circuit().expect("circuit");
    assert!(ckt.is_solved, "dynamics GFM island did not converge");

    // The black start energised the island: islbus lifts to ~0.977 pu (a dead GFL
    // island would sit at ~0). `kv_base` is L-N kV (SetVoltageBases).
    let bidx = ckt.bus_list.find("islbus").expect("islbus exists");
    let bus = &ckt.buses[bidx];
    let vpu = ckt.solution.node_v[bus.get_ref(0)].norm() / (bus.kv_base * 1000.0);
    assert!(
        (vpu - 0.97682).abs() < 2e-3,
        "islbus energised to oracle ~0.977 pu, got {vpu}"
    );

    // The Storage forms the grid and delivers ~400 kW (mode-3 var 3 = kWOut, the
    // abs delivered real power; a dead island would report 0).
    let m = dss.monitor_view("msv").expect("msv monitor");
    let kw_out = *m.channels[2].last().expect("kWOut samples") as f64;
    assert!(
        (kw_out - 400.7).abs() < 2.0,
        "Storage delivers oracle ~400.7 kW, got {kw_out}"
    );
}

/// FaultStudy also sets `is_dynamic_model` (Pascal `Set_Mode`, Solution.pas
/// l.2092-2094), so it reaches the (now ported, WPG.17) GFM `DoDynamicMode`. A
/// GFM DER in FaultStudy converges cleanly on the oracle — the `it[0]=0` state
/// init makes `BaseV=0` so the inverter injects nothing (kW=0), leaving its
/// `CalcGFMYprim` shunt in the short-circuit network. No abort.
#[test]
fn storage_gfm_faultstudy_converges() {
    let mut dss = gfm_der_circuit();
    dss.command("set mode=faultstudy");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "FaultStudy GFM must converge, got: {:?}",
        dss.errors()
    );
    assert!(dss.circuit().expect("circuit").is_solved);
    // Audit settlement: pin the oracle observable, not just convergence — the
    // `it[0]=0` GFM seed makes `BaseV=0`, so the Storage injects ~0 kW
    // (oracle-probed 1.2e-5 kW).
    let p_kw = storage_term1_kw(&mut dss);
    assert!(
        p_kw.abs() < 0.01,
        "FaultStudy GFM Storage must inject ~0 kW (oracle 1.2e-5), got {p_kw}"
    );
}

/// Terminal-1 total P (kW) of `Storage.s1` from the element snapshot (kW/kvar
/// interleaved per conductor).
fn storage_term1_kw(dss: &mut Dss) -> f64 {
    let snap = dss.snapshot_elements();
    let s = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("Storage.s1"))
        .expect("Storage.s1 in the snapshot");
    s.powers.iter().take(3).map(|s| s.re).sum()
}

/// MonteFault also sets `is_dynamic_model`, and with a Fault object present a GFM
/// DER converges cleanly (matches the oracle). NOTE: MonteFault over a circuit
/// with an **empty** Faults list is an upstream NIL-deref Access Violation
/// (`PickAFault`/`ActiveFaultObj.Randomize`, SolutionAlgs.pas l.701/725) — this is
/// independent of GFM (it reproduces with a plain GFL DER too) and is upstream UB,
/// which the port does not reproduce: `pick_a_fault` returns `None` and the direct
/// solve proceeds fault-free. Here a Fault is present, so both engines run the
/// real fault case.
#[test]
fn storage_gfm_montefault_with_fault_converges() {
    let mut dss = gfm_der_circuit();
    dss.command("New Fault.fx bus1=b phases=3 r=1 enabled=no");
    dss.command("set mode=MF");
    dss.command("set number=1");
    dss.command("set random=none"); // deterministic (no resistance jitter)
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "MonteFault GFM (fault present) must converge, got: {:?}",
        dss.errors()
    );
    assert!(dss.circuit().expect("circuit").is_solved);
    // Audit settlement: MonteFault auto-enables the fault and the GFM Storage
    // delivers a specific, deterministic power — oracle-probed 748.7156 kW
    // (terminal-1 P, `random=none`). A wrong-but-convergent injection fails.
    let p_kw = storage_term1_kw(&mut dss);
    assert!(
        (p_kw.abs() - 748.7156).abs() < 0.05,
        "MonteFault GFM Storage |P| must match the oracle 748.7156 kW, got {p_kw}"
    );
}

/// Shared source → line → grid-forming Storage, solved to a converged snapshot and
/// ready to enter a dynamic mode (FaultStudy / MonteFault).
fn gfm_der_circuit() -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=4.16 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command(
        "New Storage.s1 bus1=b phases=3 conn=delta kV=4.16 kva=800 kWrated=800 \
         kWhrated=6000 state=discharging %R=50 %X=50 kP=0.3 KVDC=0.7 PITol=0.1 \
         SafeVoltage=0 ControlMode=GFM",
    );
    dss.command("set voltagebases=[4.16]");
    dss.command("calcvoltagebases");
    dss.command("solve"); // snapshot GFM: ported, solves
    assert!(
        dss.errors().is_empty(),
        "snapshot GFM must solve: {:?}",
        dss.errors()
    );
    dss
}

/// A mode-3 (state-variable) monitor on a Storage must attach and solve cleanly:
/// Pascal mode 3 validates `BASECLASSMASK = PC_ELEMENT` and Storage is a
/// `TPCElement`, so the mode-3 check accepts `MeteredKind::Storage` alongside
/// `PcElement` (regression guard for the metered-kind branch).
#[test]
fn storage_accepts_mode3_monitor() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command("New Storage.s1 bus1=b phases=3 kV=12.47 kWrated=500 kWhrated=1000");
    dss.command("New Monitor.mvars element=Storage.s1 terminal=1 mode=3");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "unexpected errors: {:?}",
        dss.errors()
    );
    assert!(dss.circuit().expect("circuit").is_solved);
}

/// A mode-7 (Storage-state) monitor must accept a Storage element: Pascal
/// validates `CLASSMASK = STORAGE_ELEMENT` (Monitor.pas `RecalcElementData`
/// case 7). The port misclassified Storage as plain `PcElement`, so every
/// valid `mode=7 element=Storage.*` monitor errored "is not a storage
/// device!" (found re-probing the StoCtrl_* corpus decks; mode-7 *sampling*
/// stays deferred, this pins only the Pascal-faithful validation).
#[test]
fn storage_accepts_mode7_monitor() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command("New Storage.s1 bus1=b phases=3 kV=12.47 kWrated=500 kWhrated=1000");
    dss.command("New Monitor.msto element=Storage.s1 terminal=1 mode=7");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "unexpected errors: {:?}",
        dss.errors()
    );
    assert!(dss.circuit().expect("circuit").is_solved);
}

/// The converse of [`storage_accepts_mode7_monitor`]: a mode-7 monitor on a
/// non-Storage PC element must still raise Pascal's "is not a storage
/// device!" (`CLASSMASK` check, msg 2016002).
#[test]
fn mode7_monitor_rejects_non_storage() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=src basefreq=60");
    dss.command("New Line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km");
    dss.command("New Load.l1 bus1=b phases=3 kV=12.47 kW=100 pf=0.95");
    dss.command("New Monitor.msto element=Load.l1 terminal=1 mode=7");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("is not a storage device!")),
        "expected the mode-7 class error, got: {:?}",
        dss.errors()
    );
}

/// GFM op-point Isc1-invariance + injection-vs-YPrim consistency (DIVERGENCES.md
/// §B5). The islanded `gfm_micro` deck (delta GFM storage → delta/wye step-up →
/// balanced load) after the r3865 `Isc1` change: the Storage's GFM Norton YPrim
/// moves to the **capi015** value (`Y[0,0]≈561.47−2245.82j`, NOT the pre-B5
/// 0.14.5 `613.68−2402.46j`), yet the delivered load power is unchanged —
/// `Load.isl = 400 kW`, the value 0.14.5 AND capi015 both report (probed
/// bit-identical 2026-07-12). A balanced/delta load excites only the
/// positive-sequence Norton impedance `Z1` (Isc1-free), so the injection
/// `YPrim·Vset` holds the terminal voltage — hence power — invariant to the
/// ~1000× `Z0` move. If the port's injection did not track its (moved) YPrim,
/// the op-point would drift with `Z0`; pinning both the moved YPrim and the
/// invariant power in one test guards that consistency.
#[test]
fn storage_gfm_micro_op_point_isc1_invariant() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "New Circuit.gfm_micro basekv=4.16 phases=3 bus1=sourcebus",
        "New Loadshape.dl npts=6 interval=1 mult=[1.0 0.92 0.85 0.98 1.08 0.9]",
        "New Line.feeder bus1=sourcebus bus2=mainbus phases=3 r1=0.3 x1=0.6 c1=0 length=1 units=km",
        "New Line.sw1 bus1=mainbus bus2=islbus phases=3 switch=yes",
        "New Transformer.tsto phases=3 windings=2 buses=(stobus islbus) conns=(delta wye) \
         kvs=(0.48 4.16) kvas=(1000 1000) XHL=0.5",
        "New Storage.batt phases=3 conn=delta bus1=stobus kV=0.48 kva=800 kWrated=800 \
         kWhrated=6000 %stored=100 %reserve=20 %EffCharge=90 %EffDischarge=90 %IdlingkW=1 \
         %R=50 %X=50 State=DISCHARGING kP=0.3 KVDC=0.700 PITol=0.1 ControlMode=GFM",
        "New Load.isl phases=3 bus1=islbus kV=4.16 kW=400 kvar=80 model=1 daily=dl",
        "Set voltagebases=[4.16 0.48]",
        "Calcvoltagebases",
        "open line.sw1 terminal=1",
        "Set mode=daily stepsize=1h number=1",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(
        dss.errors().is_empty(),
        "gfm_micro solve: {:?}",
        dss.errors()
    );

    // The GFM Norton YPrim is the capi015 (post-B5) value.
    let (_n, y) = dss.element_yprim("Storage.batt").expect("storage yprim");
    let y00 = y[0]; // column-major [0]=Y[0,0]
    assert!(
        (y00 - num_complex::Complex64::new(5.614657341e2, -2.245822260e3)).norm() < 1e-3,
        "GFM Norton Y[0,0] {y00:?} must be the capi015 post-B5 value"
    );

    // The op-point is invariant: the balanced load still draws its 400 kW
    // setpoint (the value both Pascal revs report despite the YPrim move).
    let snap = dss.snapshot_elements();
    let load = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("Load.isl"))
        .expect("Load.isl");
    let p_kw: f64 = load.powers.iter().take(3).map(|s| s.re).sum();
    assert!(
        (p_kw - 400.0).abs() < 1e-3,
        "islanded load power {p_kw} kW must stay at the Isc1-invariant 400 kW op-point"
    );
}

/// The literal Pascal `DebugTrace` header of a 3-phase Storage with the 34
/// classic state variables — r4133 `PCElements/Storage.pas:1077-1084`, and byte
/// for byte the first line of the pinned oracle's own `STOR_storage1.csv`
/// (dss-python 0.15.7 / dss_capi 0.14.5 on a scratch copy of
/// `…/StorageTechNote/Example_9_3_Price/Storage_price.dss`; the oracle's Pascal
/// text mode ends the line with CRLF, the port with `\n` like every other port
/// writer).
const STORAGE_TRACE_HEADER_3PH: &str = "t, Iteration, LoadMultiplier, Mode, LoadModel, StorageModel,  Qnominalperphase, \
     Pnominalperphase, CurrentType, |Iinj1|, |Iinj2|, |Iinj3|, |Iterm1|, |Iterm2|, |Iterm3|, \
     |Vterm1|, |Vterm2|, |Vterm3|, kWh, State, kWOut, kWIn, kvarOut, DCkW, kWTotalLosses, \
     kWInvLosses, kWIdlingLosses, kWChDchLosses, kWh Chng, InvEff, InverterON, Vref, \
     Vavg (DRC), VV Oper, VW Oper, DRC Oper, VV_DRC Oper, WP Oper, WV Oper, kWDesired, \
     kW VW Limit, Limit kWOut Function, kVA Exceeded, Grid voltage, di/dt, it, it History, \
     Rated VDC, Avg duty cycle, Target (Amps), Series L, Max. Amps (phase),Vthev, Theta\n";

/// A scratch output directory for one trace test, removed by the caller.
/// Shared with [`super::in_show_results`], whose tests trace the same element.
pub(super) fn trace_scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dss_stor_trace_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// GOLDEN_REBASE G1.10a F4a — Storage `DebugTrace`, the **edit-time** half
/// (`elements/pc/storage/trace.rs`; r4133 `PCElements/Storage.pas:1073-1085`).
///
/// The file is created by the `New`/`Edit` itself, before any solve, under the
/// output directory and with r4133's `STOR_<name>.CSV` spelling (capi writes the
/// same name with a lowercase extension, `src/PCElements/Storage.pas:872` — one
/// member after the corpus run-file comparator's ASCII fold), and carries the
/// Pascal header. Nothing holds it open: r4133 closes after every write
/// (`:1085`, `:2429`) where capi keeps its `TBufferedFileStream` for the life of
/// the object, which is what defeated the corpus guards' `os.remove` (G1.10a F4).
#[test]
fn storage_debugtrace_opens_the_trace_file_at_edit_time() {
    let scratch = trace_scratch("edit");
    let mut dss = Dss::new();
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("New circuit.t basekv=0.48 phases=3 bus1=a pu=1");
    dss.command(
        "New Storage.storage1 phases=3 bus1=a kv=0.48 pf=1 kWrated=50 %reserve=20 \
         kWhrated=500 %stored=50 state=idling debugtrace=yes model=1",
    );

    let trace = scratch.join("STOR_storage1.CSV");
    assert!(
        trace.is_file(),
        "`debugtrace=yes` must create {} at edit time, before any solve",
        trace.display()
    );
    assert_eq!(
        std::fs::read_to_string(&trace).expect("trace readable"),
        STORAGE_TRACE_HEADER_3PH,
        "the header must be Pascal's, column for column"
    );

    // A held handle would make this fail on Windows — the property the run-file
    // guards depend on.
    std::fs::remove_file(&trace).expect("the trace file must not be held open");
    let _ = std::fs::remove_dir_all(&scratch);
}

/// GOLDEN_REBASE G1.10a F4a — Storage `DebugTrace`, the **record** half
/// (r4133 `Storage.pas:2401-2429` `WriteTraceRecord`, written from `InjCurrents`
/// `:2888` with the tag `Injection`), on the one corpus deck that enables it
/// (`Version8/Distrib/Examples/StorageTechNote/Example_9_3_Price/Storage_price.dss`,
/// copied out of the corpus so the run writes into a scratch directory).
///
/// **Both numbers.** The pinned oracle (dss-python 0.15.7 / dss_capi 0.14.5) writes
/// 48 records for this deck's daily solve — 24 steps x 2 injection passes — and its
/// first one is the literal asserted below; the port writes the same 48 records into
/// `STOR_storage1.CSV`. Measured over the whole file (`tmp/g110a/f_F4a.md`): in the
/// **parity** lane the two files are byte-identical except the `kvarOut` column, a
/// numerical zero (oracle `-2.8E-17`, port `6.6E-27` — 1e-17 of a 50 kW rating), which
/// is why that one field is compared as a magnitude here; in the **default** lane two
/// further cells of 8 later records differ (`kWTotalLosses` `5.5`/`5.4`,
/// `kWChDchLosses` `5`/`4.9`) purely because FPC's two-stage decimal round-up renders a
/// 2-significant-digit boundary upward where the correctly rounded kernel does not
/// ([`crate::util::fmt_g`]) — the live f64 state variables agree with the oracle to
/// 1 ULP (`Dss::element_variables` vs `ActiveCktElement.AllVariableValues`).
///
/// The r4133 DLL writes the same 48 records into `STOR_storage1.CSV` on a clean
/// directory (measured through `epri-worker`) with an identical header, but its
/// Delphi `Format` renders `%-.g` at ~15 significant digits where FPC renders 2
/// (`250` vs `2.5E2`, `9999` vs `1E4`, `5.45` vs `5.5`), so no single spelling
/// matches both oracles; the port keeps the FPC one every other port writer uses
/// (see `elements/pc/storage/trace.rs::TRACE_G_SIG`).
#[test]
fn storage_debugtrace_record_matches_the_oracle_line() {
    let scratch = trace_scratch("record");
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../tests/corpus/electricdss-tst/Version8/Distrib/Examples/StorageTechNote/\
         Example_9_3_Price/Storage_price.dss",
    );
    let deck = scratch.join("Storage_price.dss");
    std::fs::copy(&src, &deck).expect("copy the deck out of the corpus");

    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));

    // The whole created-file set of the run — the G1.10a run-file surface. capi
    // creates `STOR_storage1.csv`, r4133 `STOR_storage1.CSV` (both measured on a
    // clean scratch copy); the three sets are one member after the comparator's
    // ASCII fold.
    let mut created: Vec<String> = std::fs::read_dir(&scratch)
        .expect("scratch readable")
        .map(|e| {
            e.expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| n != "Storage_price.dss")
        .collect();
    created.sort();
    assert_eq!(created, vec!["STOR_storage1.CSV".to_string()]);

    let trace = scratch.join("STOR_storage1.CSV");
    let text = std::fs::read_to_string(&trace).expect("trace file written");
    let mut lines = text.lines();
    assert_eq!(
        format!("{}\n", lines.next().expect("header line")),
        STORAGE_TRACE_HEADER_3PH
    );
    let records: Vec<&str> = lines.collect();
    assert_eq!(
        records.len(),
        48,
        "24 daily steps x 2 injection passes, exactly what the oracle writes"
    );

    // The oracle's first record, field for field (dss_capi 0.14.5, measured).
    let oracle = concat!(
        "1, 1, 1, Daily, PowerFlow, 1,    -0.00,    -0.00, Injection, ",
        "     0.6,      0.6,      0.6,      0.6,      0.6,      0.6, ",
        "   277.1,    277.1,    277.1, 2.5E2, 0, 0, 0.5, -2.8E-17, ",
        "-0.5, 0.5, 0, 0.5, 0, -2.5E2, 1, 1, 1E4, 1E4, 1E4, 1E4, 1E4, ",
        "1E4, 1E4, 1E4, 0, 1E4, 50, 0, 0, 0, 0, 0, 8E3, 0, 0, 0, 0, ",
    );
    // `kvarOut` (the 5th state variable) is a cancellation zero on both engines;
    // compare it as a magnitude and every other field literally.
    const KVAR_OUT: usize = 22;
    let want: Vec<&str> = oracle.split(", ").collect();
    let got: Vec<&str> = records[0].split(", ").collect();
    assert_eq!(got.len(), want.len(), "field count: {}", records[0]);
    for (i, (w, g)) in want.iter().zip(got.iter()).enumerate() {
        if i == KVAR_OUT {
            let v: f64 = g.trim().parse().expect("kvarOut parses");
            assert!(
                v.abs() < 1e-12,
                "kvarOut must be a numerical zero like the oracle's -2.8E-17, got {g}"
            );
        } else {
            assert_eq!(g, w, "field {i} of the first record");
        }
    }

    std::fs::remove_file(&trace).expect("the trace file must not be held open");
    let _ = std::fs::remove_dir_all(&scratch);
}

/// GOLDEN_REBASE G1.10a F4a — the third trace site: the dynamics record at the
/// tail of `IntegrateStates` (r4133 `Storage.pas:3676-3683`,
/// `Format('t=%-.5g ')` + `Format(' Flag=%d ')`), written once per integration
/// call on the built-in (non-`DynaModel`) path.
#[test]
fn storage_debugtrace_writes_the_dynamics_record() {
    let scratch = trace_scratch("dyn");
    let mut dss = Dss::new();
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("New Circuit.gfm_dyn basekv=4.16 phases=3 bus1=sourcebus");
    dss.command(
        "New Line.feeder bus1=sourcebus bus2=mainbus phases=3 r1=0.3 x1=0.6 c1=0 \
         length=1 units=km",
    );
    dss.command(
        "New Storage.batt phases=3 bus1=mainbus kV=4.16 kva=800 kWrated=800 \
         kWhrated=6000 %stored=100 %reserve=20 State=DISCHARGING debugtrace=yes",
    );
    dss.command("New Load.l phases=3 bus1=mainbus kV=4.16 kW=400 kvar=80 model=1");
    dss.command("Set voltagebases=[4.16]");
    dss.command("Calcvoltagebases");
    dss.command("solve");
    dss.command("Set mode=dynamics stepsize=0.001 number=2 maxiterations=30");
    dss.command("solve");

    let trace = scratch.join("STOR_batt.CSV");
    let text = std::fs::read_to_string(&trace).expect("trace file written");
    let dyn_lines: Vec<&str> = text.lines().filter(|l| l.starts_with("t=")).collect();
    assert!(
        !dyn_lines.is_empty(),
        "the dynamics record must be written once per IntegrateStates call"
    );
    assert_eq!(
        dyn_lines[0], "t=0.001  Flag=0 ",
        "Pascal `t=%-.5g ` + ` Flag=%d ` (IterationFlag 0 = new time step)"
    );
    std::fs::remove_file(&trace).expect("the trace file must not be held open");
    let _ = std::fs::remove_dir_all(&scratch);
}

/// GOLDEN_REBASE G1.10a audit settlement (finding AC-4) — the JSON model reader
/// creates the trace file too.
///
/// Pascal opens it inside the property side effect itself (dss_capi
/// `src/PCElements/Storage.pas:765` `TStorageObj.PropertySideEffects` → the
/// `ord(TProp.debugtrace)` arm at `:868-885`), which `FillObjFromJSON` reaches
/// like every other write path. The port queues the header in the hook and
/// drains it in the executive, so EVERY edit path has to drain: before this fix
/// `exec/json_import.rs` called `end_edit` without the drain, and a Storage
/// imported with `DebugTrace: true` kept a queued header forever and traced
/// nothing — silently, because `write_trace_record` early-returns without a path.
#[test]
fn storage_debugtrace_survives_a_json_model_round_trip() {
    let built = trace_scratch("json-src");
    let mut dss = Dss::new();
    dss.command(&format!("set datapath=\"{}\"", built.display()));
    dss.command("New circuit.t basekv=0.48 phases=3 bus1=a pu=1");
    dss.command(
        "New Storage.storage1 phases=3 bus1=a kv=0.48 pf=1 kWrated=50 %reserve=20          kWhrated=500 %stored=50 state=idling debugtrace=yes model=1",
    );
    dss.command("makebuslist");
    let json = dss
        .circuit_to_json(crate::report::export::json::JsonOpts::default())
        .expect("the built circuit exports");
    assert!(
        json.contains("\"DebugTrace\""),
        "the property must survive the export, or this test would prove nothing"
    );
    drop(dss);
    let _ = std::fs::remove_dir_all(&built);

    // Re-import into a fresh engine whose output directory is a clean scratch.
    let scratch = trace_scratch("json");
    let mut dss = Dss::new();
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.circuit_from_json(&json).expect("the model imports");
    assert!(
        dss.errors().is_empty(),
        "unexpected errors: {:?}",
        dss.errors()
    );

    let trace = scratch.join("STOR_storage1.CSV");
    assert!(
        trace.is_file(),
        "`DebugTrace: true` must create {} on the JSON import path too",
        trace.display()
    );
    assert_eq!(
        std::fs::read_to_string(&trace).expect("trace readable"),
        STORAGE_TRACE_HEADER_3PH,
        "the header is the same Pascal one the command path writes"
    );
    std::fs::remove_file(&trace).expect("the trace file must not be held open");
    let _ = std::fs::remove_dir_all(&scratch);
}
