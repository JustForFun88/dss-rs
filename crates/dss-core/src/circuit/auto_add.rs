//! Pascal `Common/AutoAdd.pas` `TAutoAdd` — the auto-add solution machinery.
//!
//! The capacity-search `Solve` (`AutoAdd.pas` l.267) automatically adds a
//! generator or capacitor at the bus that minimizes weighted losses + unserved
//! energy. Per candidate bus it injects the trial device's current
//! (`AddCurrents`, wired through `Solution.AddInAuxCurrents` under
//! `UseAuxCurrents`), re-solves a snapshot, samples the meters and scores the
//! result (`Get_WeightedLosses` = `LossWeight·puLossImprovement +
//! `UEWeight·puEENImprovement`); the winner is permanently added through the
//! normal executive command path (`New Generator.Gadd…`/`New Capacitor.Cadd…`).
//!
//! The public option state (`GenkW`/`GenPF`/`Capkvar`/`AddType`) round-trips
//! through `Set`/`Get`; the private Solve-only state (`BusIdxList`, the trial
//! device fields `BusIndex`/`Phases`/`GenVA`/`Ycap`, and the unique-name
//! counters) lives on this record and is populated during a Solve.
//!
//! The orchestration itself (the `Solve` driver, which re-enters the executive
//! to add the winner) lives in `exec/auto_add.rs`; the per-candidate current
//! injection (`AddCurrents`) is in `solution/solution/power_flow.rs` next to the
//! `DoNormalSolution` loop that calls it. This module holds the record and the
//! `(ckt, store)`-level helpers (`MakeBusList`, `ComputekWLosses_EEN`,
//! `Get_WeightedLosses`, `SetBaseLosses`).

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::meter::energymeter::EnergyMeter;
use crate::elements::traits::{ElemStore, SysCtx};
use crate::report::format::strip_extension;
use crate::support::hashlist::HashList;

/// Pascal `DSSGlobals.GENADD` — add a generator at the lowest-loss bus.
pub const GENADD: i32 = 1;
/// Pascal `DSSGlobals.CAPADD` — add a capacitor at the lowest-loss bus.
pub const CAPADD: i32 = 2;

/// Pascal `TAutoAdd` (record), one per circuit (`Circuit.AutoAddObj`).
#[derive(Debug, Clone)]
pub struct AutoAdd {
    /// `GenkW` — kW of the trial generator.
    pub gen_kw: f64,
    /// `GenPF` — power factor of the trial generator.
    pub gen_pf: f64,
    /// `Genkvar` — derived from `GenkW`/`GenPF` inside `Solve`; FPC
    /// zero-initializes the record field, so it starts at 0.0.
    pub gen_kvar: f64,
    /// `Capkvar` — kvar of the trial capacitor.
    pub cap_kvar: f64,
    /// `AddType` — `GENADD`/`CAPADD`.
    pub add_type: i32,
    /// `ModeChanged` — forces `MakeBusList` to rebuild on the next auto-add
    /// solve.
    pub mode_changed: bool,

    // ---- Private Solve-only state (Pascal `TAutoAdd` PRIVATE fields) ----
    /// `BusIdxList` — candidate buses to test, as Pascal 1-based `BusList`
    /// indices (0 = skip, matching the Pascal `if BusIndex > 0` guard).
    pub(crate) bus_idx_list: Vec<usize>,
    /// `BusIndex` — the bus currently under test (Pascal 1-based; read by
    /// `AddCurrents` during the per-candidate solve).
    pub(crate) bus_index: usize,
    /// `Phases` — 1 or 3, the trial device's phase count at `BusIndex`.
    pub(crate) phases: i32,
    /// `GenVA` — the trial generator's per-phase complex power injection.
    pub(crate) gen_va: Complex64,
    /// `Ycap` — the trial capacitor's per-phase shunt admittance magnitude.
    pub(crate) ycap: f64,
    /// `LastAddedGenerator`/`LastAddedCapacitor` — unique-name counters.
    pub(crate) last_added_generator: i32,
    pub(crate) last_added_capacitor: i32,
}

impl AutoAdd {
    /// Pascal `TAutoAdd.Init`.
    pub fn new() -> Self {
        Self {
            gen_kw: 1000.0,
            gen_pf: 1.0,
            gen_kvar: 0.0,
            cap_kvar: 600.0,
            add_type: GENADD,
            mode_changed: true,
            bus_idx_list: Vec::new(),
            bus_index: 0,
            phases: 3,
            gen_va: Complex64::ZERO,
            ycap: 0.0,
            last_added_generator: 0,
            last_added_capacitor: 0,
        }
    }
}

impl Default for AutoAdd {
    fn default() -> Self {
        Self::new()
    }
}

/// The four loss/EEN figures a candidate solve produces (Pascal's `kWLosses`,
/// `kWEEN`, `puLossImprovement`, `puEENImprovement` PRIVATE fields plus the
/// weighted objective `Get_WeightedLosses` returns).
#[derive(Debug, Clone, Copy)]
pub(crate) struct LossFigures {
    pub kw_losses: f64,
    pub kw_een: f64,
    pub pu_loss_improvement: f64,
    pub pu_een_improvement: f64,
    /// The weighted objective (`LossImproveFactor`).
    pub weighted: f64,
}

/// Pascal `SumSelectedRegisters` (`AutoAdd.pas` l.88): `Σ Registers[reg] ·
/// TotalsMask[reg]` over the selected register ordinals. `regs` holds Pascal
/// 1-based register ordinals (`Circuit.LossRegs`/`UEregs`); the Rust register /
/// mask slices are 0-based, so index `reg - 1`.
fn sum_selected_registers(m: &EnergyMeter, regs: &[i32]) -> f64 {
    let registers = m.registers();
    let mask = m.totals_mask();
    let mut result = 0.0;
    for &r in regs {
        let i = (r - 1) as usize;
        if let (Some(&v), Some(&w)) = (registers.get(i), mask.get(i)) {
            result += v * w;
        }
    }
    result
}

/// Pascal `TAutoAdd.ComputekWLosses_EEN` (`AutoAdd.pas` l.649): with no meters,
/// go by total system losses (`Circuit.Losses.re · 0.001`, EEN = 0); otherwise
/// sum the loss/UE registers over every meter.
pub(crate) fn compute_kw_losses_een(
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
    sys: &SysCtx,
) -> (f64, f64) {
    if ckt.energy_meters.is_empty() {
        let kw_losses = ckt.losses(store, sys).re * 0.001;
        return (kw_losses, 0.0);
    }
    let (loss_regs, ue_regs) = (ckt.loss_regs.clone(), ckt.ue_regs.clone());
    let mut kw_losses = 0.0;
    let mut kw_een = 0.0;
    for meter_ref in ckt.energy_meters.clone() {
        let m = store
            .obj(meter_ref)
            .as_any()
            .downcast_ref::<EnergyMeter>()
            .expect("energy_meters holds EnergyMeter");
        kw_losses += sum_selected_registers(m, &loss_regs);
        kw_een += sum_selected_registers(m, &ue_regs);
    }
    (kw_losses, kw_een)
}

/// Pascal `TAutoAdd.Get_WeightedLosses` (`AutoAdd.pas` l.186): weighted
/// loss + EEN improvement over the base case. `base_losses`/`base_een` are the
/// pre-add baseline (`SetBaseLosses`), `gen_kw` the normalizing generator size.
pub(crate) fn weighted_losses(
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
    sys: &SysCtx,
    base_losses: f64,
    base_een: f64,
    gen_kw: f64,
) -> LossFigures {
    let (kw_losses, kw_een) = compute_kw_losses_een(ckt, store, sys);
    let pu_loss_improvement = (base_losses - kw_losses) / gen_kw;

    if ckt.energy_meters.is_empty() {
        // No energymeters — just go by total system losses.
        return LossFigures {
            kw_losses,
            kw_een,
            pu_loss_improvement,
            pu_een_improvement: 0.0,
            weighted: pu_loss_improvement,
        };
    }

    let pu_een_improvement = (base_een - kw_een) / gen_kw;
    let weighted = ckt.loss_weight * pu_loss_improvement + ckt.ue_weight * pu_een_improvement;
    LossFigures {
        kw_losses,
        kw_een,
        pu_loss_improvement,
        pu_een_improvement,
        weighted,
    }
}

/// Pascal `TAutoAdd.MakeBusList` (`AutoAdd.pas` l.115): the candidate-bus list.
/// Priority: the `AutoAddBusList` if set; else the union of every EnergyMeter's
/// zone buses (hash-list dedup); else — no meters — every bus in the circuit.
/// Returns the list as Pascal 1-based `BusList` indices (0 = not found/skip).
pub(crate) fn make_bus_list(ckt: &Circuit, store: &dyn ElemStore) -> Vec<usize> {
    // AutoAddBusList exists → use it (see `Set AutoBusList=`).
    if !ckt.auto_add_bus_list.is_empty() {
        return ckt
            .auto_add_bus_list
            .iter()
            .map(|name| ckt.bus_list.find(name).map_or(0, |i| i + 1))
            .collect();
    }

    // No energy meters → include every bus in the circuit. Pascal fills
    // `BusIdxList[i] := i` for `i in 0..Count`, i.e. the value 0 (the first
    // slot) is skipped by the `if BusIndex > 0` guard in `Solve` and the last
    // bus is not reached — reproduced 1:1 as the Pascal 1-based indices [0, 1,
    // …, Count-1].
    if ckt.energy_meters.is_empty() {
        return (0..ckt.bus_list.len()).collect();
    }

    // Construct the bus list from the Energy Meter zone lists (unique bus
    // names). Walk every meter's branch (`SequenceList` == the `BranchList`
    // meter→ends order) and add each terminal's stripped bus name once.
    let mut fbus_list = HashList::with_capacity(ckt.buses.len());
    for &meter_ref in &ckt.energy_meters {
        let m = store
            .obj(meter_ref)
            .as_any()
            .downcast_ref::<EnergyMeter>()
            .expect("energy_meters holds EnergyMeter");
        if !m.has_branch_list() {
            continue;
        }
        for &pd in m.sequence_list() {
            let elem = store.ckt_elem(pd);
            let nterms = elem.cd().nterms;
            for i in 1..=nterms {
                let bname = strip_extension(elem.cd().get_bus(i));
                if fbus_list.find(&bname).is_none() {
                    fbus_list.add(&bname);
                }
            }
        }
    }

    fbus_list
        .iter()
        .map(|name| ckt.bus_list.find(name).map_or(0, |i| i + 1))
        .collect()
}

impl AutoAdd {
    /// Pascal `TAutoAdd.GetUniqueGenName` (`AutoAdd.pas` l.235): the next
    /// `Gadd<n>` name not already taken (`exists` is `GeneratorClass.Find <>
    /// NIL`).
    pub(crate) fn get_unique_gen_name(&mut self, mut exists: impl FnMut(&str) -> bool) -> String {
        loop {
            self.last_added_generator += 1;
            let trial = format!("Gadd{}", self.last_added_generator);
            if !exists(&trial) {
                return trial;
            }
        }
    }

    /// Pascal `TAutoAdd.GetUniqueCapName` (`AutoAdd.pas` l.250).
    pub(crate) fn get_unique_cap_name(&mut self, mut exists: impl FnMut(&str) -> bool) -> String {
        loop {
            self.last_added_capacitor += 1;
            let trial = format!("Cadd{}", self.last_added_capacitor);
            if !exists(&trial) {
                return trial;
            }
        }
    }
}
