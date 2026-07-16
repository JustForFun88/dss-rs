//! WP8.3 step 4 — Pascal `TSystemMeter` (EnergyMeter.pas l.181) and the
//! demand-interval (`DI_`) file machinery: the per-meter/system/totals
//! demand-interval CSVs plus the overload / voltage-exception reports, written
//! **during the time-series solve** (opened by `SolveDaily`/`SolveYearly`/…,
//! one row per `SampleAll`, closed at the end of the run / `CloseDI` /
//! `ResetAll`).
//!
//! Pascal builds each file in an in-memory `TBytesStream` via the
//! `MemoryMap_lib.pas` helpers (`Create_Meter_Space`/`WriteintoMem[Str]`) and
//! flushes it with `CloseMHandler`. The observable product is a plain text
//! file: strings verbatim, each double preceded by `", "` unless it starts a
//! line, rendered `Format('%-g')` (FPC default = 15 significant digits).
//! [`MeterStream`] reproduces exactly that emission; the byte-tag stream
//! encoding itself is an implementation detail with no observable trace.
//!
//! The `Append*` re-open paths (`TEnergyMeterObj.AppendDemandIntervalFile`,
//! `TSystemMeter.AppendDemandIntervalFile`, `TEnergyMeter.AppendAllDIFiles`)
//! are **dead upstream** in the vendored dss_capi 0.14.5 — `AppendAllDIFiles`
//! has no caller (`grep` over `src/`), and it is the only caller of the other
//! two — so every `CloseMHandler` append flag is its `FALSE` default and the
//! port always creates the file fresh.

use std::path::Path;

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::meter::energymeter::{EnergyMeter, NUM_EM_REGISTERS};
use crate::elements::traits::{ElemStore, SysCtx};
use crate::util::{fmt_g, sqrt3};

use super::downcast_meter;

/// The Pascal memory-map text builder (`Create_Meter_Space` +
/// `WriteintoMem`/`WriteintoMemStr` + the `CloseMHandler` render loop):
/// strings are emitted verbatim (a `\n` restarts the line-head state), doubles
/// are prefixed with `", "` unless at the line head and rendered `%-g`.
#[derive(Debug, Clone, Default)]
pub struct MeterStream {
    buf: String,
    /// Pascal `Fhead`: at the start of a line (no separator before a double).
    head: bool,
}

impl MeterStream {
    /// Pascal `Create_Meter_Space(Init_Str)`.
    pub(crate) fn new(init: &str) -> Self {
        let mut s = Self {
            buf: String::new(),
            head: true,
        };
        s.write_str(init);
        s
    }

    /// Pascal `WriteintoMemStr`.
    pub(crate) fn write_str(&mut self, content: &str) {
        for c in content.chars() {
            self.buf.push(c);
            self.head = c == '\n';
        }
    }

    /// Pascal `WriteintoMem` (a double): `", "` separator unless at the line
    /// head, then `Format('%-g')` (FPC general format, 15 significant digits).
    pub(crate) fn write_dbl(&mut self, v: f64) {
        if !self.head {
            self.buf.push_str(", ");
        }
        self.buf.push_str(&fmt_g(v, 15));
        self.head = false;
    }

    /// Pascal `CloseMHandler`: flush the buffered text to `path` (always
    /// `fmCreate` — the append flags are dead upstream, see the module doc).
    fn close_to(self, path: &Path, errors: &mut Vec<String>) {
        if let Err(e) = std::fs::write(path, self.buf.as_bytes()) {
            errors.push(format!(
                "Error Attempting to open file: \"{}\". {e}",
                path.display()
            ));
        }
    }
}

/// Pascal `TSystemMeter` (EnergyMeter.pas l.181): the circuit-wide aggregate
/// meter — total source energy in/out, peak kW/kVA, total losses — sampled by
/// `TEnergyMeter.SampleAll` alongside the zone meters.
#[derive(Debug, Clone)]
pub struct SystemMeter {
    kwh: f64,
    dkwh: f64,
    kvarh: f64,
    dkvarh: f64,
    peak_kw: f64,
    peak_kva: f64,
    losses_kwh: f64,
    dlosses_kwh: f64,
    losses_kvarh: f64,
    dlosses_kvarh: f64,
    peak_losses_kw: f64,
    first_sample_after_reset: bool,
    this_meter_di_file_is_open: bool,
    /// Last sampled total source power / total losses (kW) — the values the
    /// per-interval `WriteDemandIntervalData` row echoes.
    c_power: Complex64,
    c_losses: Complex64,
}

impl Default for SystemMeter {
    /// Pascal `TSystemMeter.Create` → `Clear()`.
    fn default() -> Self {
        Self {
            kwh: 0.0,
            dkwh: 0.0,
            kvarh: 0.0,
            dkvarh: 0.0,
            peak_kw: 0.0,
            peak_kva: 0.0,
            losses_kwh: 0.0,
            dlosses_kwh: 0.0,
            losses_kvarh: 0.0,
            dlosses_kvarh: 0.0,
            peak_losses_kw: 0.0,
            first_sample_after_reset: true,
            this_meter_di_file_is_open: false,
            c_power: Complex64::ZERO,
            c_losses: Complex64::ZERO,
        }
    }
}

impl SystemMeter {
    /// Pascal `TSystemMeter.Reset` (→ `Clear`). The DI-open flag is *not*
    /// cleared (Pascal `Clear` doesn't touch `This_Meter_DIFileIsOpen`).
    pub(crate) fn reset(&mut self) {
        let open = self.this_meter_di_file_is_open;
        *self = Self::default();
        self.this_meter_di_file_is_open = open;
    }

    /// Pascal `TSystemMeter.Integrate` (l.3412): trapezoidal when the circuit
    /// flag is set (skipped on the first sample), else plain Euler; always
    /// records the derivative.
    fn integrate(
        reg: &mut f64,
        value: f64,
        deriv: &mut f64,
        trapezoidal: bool,
        first: bool,
        interval_hrs: f64,
    ) {
        if trapezoidal {
            if !first {
                *reg += 0.5 * interval_hrs * (value + *deriv);
            }
        } else {
            *reg += interval_hrs * value;
        }
        *deriv = value;
    }

    /// Pascal `TSystemMeter.TakeSample` (l.3491) minus the file row (the
    /// caller writes it — the SDI stream lives on the class state).
    fn take_sample(
        &mut self,
        c_power: Complex64,
        c_losses: Complex64,
        trapezoidal: bool,
        interval_hrs: f64,
    ) {
        self.c_power = c_power;
        self.c_losses = c_losses;
        let first = self.first_sample_after_reset;
        Self::integrate(
            &mut self.kwh,
            c_power.re,
            &mut self.dkwh,
            trapezoidal,
            first,
            interval_hrs,
        );
        Self::integrate(
            &mut self.kvarh,
            c_power.im,
            &mut self.dkvarh,
            trapezoidal,
            first,
            interval_hrs,
        );
        self.peak_kw = c_power.re.max(self.peak_kw);
        self.peak_kva = c_power.norm().max(self.peak_kva);
        Self::integrate(
            &mut self.losses_kwh,
            c_losses.re,
            &mut self.dlosses_kwh,
            trapezoidal,
            first,
            interval_hrs,
        );
        Self::integrate(
            &mut self.losses_kvarh,
            c_losses.im,
            &mut self.dlosses_kvarh,
            trapezoidal,
            first,
            interval_hrs,
        );
        self.peak_losses_kw = c_losses.re.max(self.peak_losses_kw);
        self.first_sample_after_reset = false;
    }

    /// Pascal `TSystemMeter.WriteRegisters`.
    fn write_registers(&self, sm: &mut MeterStream) {
        sm.write_dbl(self.kwh);
        sm.write_dbl(self.kvarh);
        sm.write_dbl(self.peak_kw);
        sm.write_dbl(self.peak_kva);
        sm.write_dbl(self.losses_kwh);
        sm.write_dbl(self.losses_kvarh);
        sm.write_dbl(self.peak_losses_kw);
    }
}

/// The `TEnergyMeter` **class-level** demand-interval state (+ the DSS-context
/// `DIFilesAreOpen` flag): the `Set DemandInterval/DIVerbose/Overloadreport/
/// Voltexceptionreport=` switches, the `DI_yr_<year>` output directory, the
/// class register totals, the shared in-memory report streams, and the
/// [`SystemMeter`]. Lives on the [`Circuit`] so both the executive (the `Set`
/// handlers / `Reset` / `CloseDI`) and the solve loop reach it.
#[derive(Debug, Clone)]
pub struct EmDiState {
    /// `FSaveDemandInterval` (`Set DemandInterval=`).
    pub save_demand_interval: bool,
    /// `FDI_Verbose` (`Set DIVerbose=`): per-meter DI files, not just totals.
    pub di_verbose: bool,
    /// `Do_OverloadReport` (`Set Overloadreport=`).
    pub do_overload_report: bool,
    /// `Do_VoltageExceptionReport` (`Set Voltexceptionreport=`).
    pub do_voltage_exception_report: bool,
    /// The DSS-context `DIFilesAreOpen` flag.
    pub di_files_are_open: bool,
    pub overload_file_is_open: bool,
    pub voltage_file_is_open: bool,
    /// `DI_Dir`: `<OutputDirectory><CaseName>/DI_yr_<year>` (built by
    /// `ResetAll`).
    pub di_dir: std::path::PathBuf,
    /// `DI_RegisterTotals[1..NumEMRegisters]` (0-based here).
    pub di_register_totals: Vec<f64>,
    /// `SDI_MHandle` — the system-meter demand-interval stream.
    pub(crate) sdi: Option<MeterStream>,
    /// `TDI_MHandle` — the `DI_Totals` stream.
    pub(crate) tdi: Option<MeterStream>,
    /// `EMT_MHandle` — the `EnergyMeterTotals` stream.
    pub(crate) emt: Option<MeterStream>,
    /// `OV_MHandle` — the `DI_Overloads` stream.
    pub(crate) ov: Option<MeterStream>,
    /// `VR_MHandle` — the `DI_VoltExceptions` stream.
    pub(crate) vr: Option<MeterStream>,
    pub system_meter: SystemMeter,
}

impl Default for EmDiState {
    fn default() -> Self {
        Self {
            save_demand_interval: false,
            di_verbose: false,
            do_overload_report: false,
            do_voltage_exception_report: false,
            di_files_are_open: false,
            overload_file_is_open: false,
            voltage_file_is_open: false,
            di_dir: std::path::PathBuf::new(),
            di_register_totals: vec![0.0; NUM_EM_REGISTERS],
            sdi: None,
            tdi: None,
            emt: None,
            ov: None,
            vr: None,
            system_meter: SystemMeter::default(),
        }
    }
}

impl EmDiState {
    /// Pascal `TEnergyMeter.ClearDI_Totals`.
    fn clear_di_totals(&mut self) {
        self.di_register_totals = vec![0.0; NUM_EM_REGISTERS];
    }
}

/// The PM-build primary-context instance suffix `DSS._Name` — the same `_1`
/// already pinned by the `Export Monitors` filenames.
const NAME_SUFFIX: &str = "_1";

/// The overloaded PD element's `FullName` (`Class.name`) for the `DI_Overloads`
/// row, recovered by concrete-type probe (the solve-side store has no class
/// registry; `ckt.pd_elements` only ever holds these five classes).
fn pd_full_name(store: &dyn ElemStore, r: crate::elements::traits::ElemRef) -> String {
    use crate::elements::pd::capacitor::Capacitor;
    use crate::elements::pd::fault::Fault;
    use crate::elements::pd::line::Line;
    use crate::elements::pd::reactor::Reactor;
    use crate::elements::pd::transformer::Transformer;
    let obj = store.obj(r);
    let any = obj.as_any();
    let cls = if any.downcast_ref::<Line>().is_some() {
        "Line"
    } else if any.downcast_ref::<Transformer>().is_some() {
        "Transformer"
    } else if any.downcast_ref::<Capacitor>().is_some() {
        "Capacitor"
    } else if any.downcast_ref::<Reactor>().is_some() {
        "Reactor"
    } else if any.downcast_ref::<Fault>().is_some() {
        "Fault"
    } else {
        "PDElement"
    };
    format!("{cls}.{}", obj.data().name())
}

/// Pascal `GetTotalPowerFromSources` (Utilities.pas:1327): `-Σ` over the
/// circuit sources of `Power[1]` (VA).
fn total_power_from_sources(ckt: &Circuit, store: &mut dyn ElemStore, sys: &SysCtx) -> Complex64 {
    let node_v = &ckt.solution.node_v;
    let mut total = Complex64::ZERO;
    for &r in &ckt.sources {
        total -= store.ckt_elem_mut(r).terminal_power(sys, node_v, 1);
    }
    total
}

/// The first meter of the circuit's EnergyMeter list (Pascal
/// `EnergyMeters.First()` — creation order, enabled or not), used for the
/// register-name header rows of the totals files.
fn first_meter_register_names(ckt: &Circuit, store: &dyn ElemStore) -> Vec<String> {
    ckt.energy_meters
        .first()
        .map(|&r| {
            store
                .obj(r)
                .as_any()
                .downcast_ref::<EnergyMeter>()
                .expect("energy_meters holds EnergyMeter objects")
                .register_names()
                .to_vec()
        })
        .unwrap_or_default()
}

/// Pascal `TEnergyMeter.CreateFDI_Totals` (l.3308): the `DI_Totals` stream +
/// its `Time, "reg", …` header from the first meter's register names.
fn create_fdi_totals(ckt: &mut Circuit, store: &dyn ElemStore) {
    let names = first_meter_register_names(ckt, store);
    let mut tdi = MeterStream::new("Time");
    for name in &names {
        tdi.write_str(&format!(", \"{name}\""));
    }
    tdi.write_str("\n");
    ckt.em_di.tdi = Some(tdi);
}

/// Pascal `TEnergyMeter.CreateMeterTotals` (l.3517): the `EnergyMeterTotals`
/// stream + its `Name, "reg", …` header.
fn create_meter_totals(ckt: &mut Circuit, store: &dyn ElemStore) {
    let names = first_meter_register_names(ckt, store);
    let mut emt = MeterStream::new("Name");
    for name in &names {
        emt.write_str(&format!(", \"{name}\""));
    }
    emt.write_str("\n");
    ckt.em_di.emt = Some(emt);
}

/// Pascal `TEnergyMeter.OpenOverloadReportFile` (l.3789).
fn open_overload_report_file(ckt: &mut Circuit) {
    ckt.em_di.overload_file_is_open = true;
    ckt.em_di.ov = Some(MeterStream::new(
        "\"Hour\", \"Element\", \"Normal Amps\", \"Emerg Amps\", \"% Normal\", \"% Emerg\", \
         \"kVBase\", \"I1(A)\", \"I2(A)\", \"I3(A)\"\n",
    ));
}

/// Pascal `TEnergyMeter.OpenVoltageReportFile` (l.3804).
fn open_voltage_report_file(ckt: &mut Circuit) {
    ckt.em_di.voltage_file_is_open = true;
    let mut vr = MeterStream::new(
        "\"Hour\", \"Undervoltages\", \"Min Voltage\", \"Overvoltage\", \"Max Voltage\", \
         \"Min Bus\", \"Max Bus\"",
    );
    vr.write_str(
        ", \"LV Undervoltages\", \"Min LV Voltage\", \"LV Overvoltage\", \"Max LV Voltage\", \
         \"Min LV Bus\", \"Max LV Bus\"\n",
    );
    ckt.em_di.vr = Some(vr);
}

/// Pascal `TEnergyMeterObj.OpenDemandIntervalFile` (l.2894): the per-meter DI
/// stream (+ the phase-voltage report stream when requested) — created only in
/// verbose mode.
fn open_meter_di_file(
    meter_ref: crate::elements::traits::ElemRef,
    ckt: &Circuit,
    store: &mut dyn ElemStore,
) {
    let di_verbose = ckt.em_di.di_verbose;
    let em = downcast_meter(store, meter_ref);
    // The Pascal `if This_Meter_DIFileIsOpen then CloseDemandIntervalFile`
    // re-open guard is unreachable through the ported flow (`OpenAllDIFiles`
    // only runs when `DIFilesAreOpen` is false, which implies every meter
    // stream is closed); a fresh stream simply replaces any leftover.
    if !di_verbose {
        return;
    }
    em.di_file_open(true);
    let mut di = MeterStream::new("\"Hour\"");
    for name in em.register_names() {
        di.write_str(&format!(", \"{name}\""));
    }
    di.write_str("\n");
    em.set_di_stream(Some(di));

    // Phase Voltage Report, if requested (`FPhaseVoltageReport`).
    if em.phase_voltage_report() {
        let mut phv = MeterStream::new("\"Hour\"");
        em.set_v_phase_report_open(true);
        let (vbase_list, _count) = em.vbase_view();
        for &vb in &vbase_list {
            let vbase = vb * sqrt3();
            if vbase > 0.0 {
                for j in 1..=3 {
                    phv.write_str(&format!(", {}kV_Phs_{}_Max", fmt_g(vbase, 3), j));
                }
                for j in 1..=3 {
                    phv.write_str(&format!(", {}kV_Phs_{}_Min", fmt_g(vbase, 3), j));
                }
                for j in 1..=3 {
                    phv.write_str(&format!(", {}kV_Phs_{}_Avg", fmt_g(vbase, 3), j));
                }
            }
        }
        phv.write_str(", Min Bus, MaxBus\n");
        em.set_phv_stream(Some(phv));
    }
}

/// Pascal `TEnergyMeterObj.CloseDemandIntervalFile` (l.2867): flush the meter's
/// DI (+ PHV) stream to `DI_Dir/<name>_1.csv`, then append the meter's
/// registers row to the `EnergyMeterTotals` stream (unconditionally — even a
/// non-verbose meter contributes its registers).
fn close_meter_di_file(
    meter_ref: crate::elements::traits::ElemRef,
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
    errors: &mut Vec<String>,
) {
    let di_dir = ckt.em_di.di_dir.clone();
    let em = downcast_meter(store, meter_ref);
    let name = em.med.cd.obj.name().to_string();
    if em.di_file_is_open() {
        if let Some(di) = em.take_di_stream() {
            // Pascal `MakeDIFileName` (l.3155): `DI_Dir/<Name><_Name>.csv`.
            di.close_to(&di_dir.join(format!("{name}{NAME_SUFFIX}.csv")), errors);
        }
        em.di_file_open(false);
        if em.v_phase_report_open()
            && let Some(phv) = em.take_phv_stream()
        {
            // Pascal `MakeVPhaseReportFileName` (l.1200).
            phv.close_to(
                &di_dir.join(format!("{name}_PhaseVoltageReport{NAME_SUFFIX}.csv")),
                errors,
            );
        }
        em.set_v_phase_report_open(false);
    }
    // Write Registers to Totals File (Pascal l.2888).
    let registers = em.registers().to_vec();
    if let Some(emt) = ckt.em_di.emt.as_mut() {
        emt.write_str(&format!("\"{name}\""));
        for r in &registers {
            emt.write_dbl(*r);
        }
        emt.write_str("\n");
    }
}

/// Pascal `TEnergyMeter.OpenAllDIFiles` (l.3752): create every demand-interval
/// stream (meters, system meter, optional overload/voltage reports, the
/// `DI_Totals`) and raise `DIFilesAreOpen`. No-op unless `Set
/// DemandInterval=yes`. Called at the head of each time-series solve.
pub(crate) fn open_all_di_files(ckt: &mut Circuit, store: &mut dyn ElemStore) {
    if !ckt.em_di.save_demand_interval {
        return;
    }
    ckt.em_di.clear_di_totals();

    for meter_ref in ckt.energy_meters.clone() {
        if downcast_meter(store, meter_ref).enabled() {
            open_meter_di_file(meter_ref, ckt, store);
        }
    }

    // `TSystemMeter.OpenDemandIntervalFile` (l.3427).
    ckt.em_di.system_meter.this_meter_di_file_is_open = true;
    let mut sdi = MeterStream::new("\"Hour\", ");
    sdi.write_str(
        "kWh, kvarh, \"Peak kW\", \"peak kVA\", \"Losses kWh\", \"Losses kvarh\", \
         \"Peak Losses kW\"\n",
    );
    ckt.em_di.sdi = Some(sdi);

    if ckt.em_di.do_overload_report {
        open_overload_report_file(ckt);
    }
    if ckt.em_di.do_voltage_exception_report {
        open_voltage_report_file(ckt);
    }
    create_fdi_totals(ckt, store);
    ckt.em_di.di_files_are_open = true;
}

/// Pascal `TEnergyMeter.CloseAllDIFiles` (l.3010): write every open
/// demand-interval stream to its file under `DI_Dir` and drop the open flags.
/// Called at the end of the daily/duty/peak-day solves, by `ResetAll`, by
/// `Set year=`, by `CloseDI`, and before a `Clear`.
pub(crate) fn close_all_di_files(
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
    errors: &mut Vec<String>,
) {
    if !ckt.em_di.save_demand_interval {
        return;
    }
    // While closing DI files, write all meter registers to one file.
    create_meter_totals(ckt, store);

    for meter_ref in ckt.energy_meters.clone() {
        if downcast_meter(store, meter_ref).enabled() {
            close_meter_di_file(meter_ref, ckt, store, errors);
        }
    }

    write_totals_file(ckt, store, errors); // Totals_1.csv

    // `TSystemMeter.CloseDemandIntervalFile` (l.3376) + `Save` (l.3452).
    let di_dir = ckt.em_di.di_dir.clone();
    if ckt.em_di.system_meter.this_meter_di_file_is_open {
        if let Some(sdi) = ckt.em_di.sdi.take() {
            sdi.close_to(
                &di_dir.join(format!("DI_SystemMeter{NAME_SUFFIX}.csv")),
                errors,
            );
        }
        ckt.em_di.system_meter.this_meter_di_file_is_open = false;
    }
    system_meter_save(ckt, errors);

    if let Some(emt) = ckt.em_di.emt.take() {
        emt.close_to(
            &di_dir.join(format!("EnergyMeterTotals{NAME_SUFFIX}.csv")),
            errors,
        );
    }
    if let Some(tdi) = ckt.em_di.tdi.take() {
        tdi.close_to(&di_dir.join(format!("DI_Totals{NAME_SUFFIX}.csv")), errors);
    }
    ckt.em_di.di_files_are_open = false;
    if ckt.em_di.overload_file_is_open {
        if let Some(ov) = ckt.em_di.ov.take() {
            ov.close_to(
                &di_dir.join(format!("DI_Overloads{NAME_SUFFIX}.csv")),
                errors,
            );
        }
        ckt.em_di.overload_file_is_open = false;
    }
    if ckt.em_di.voltage_file_is_open {
        if let Some(vr) = ckt.em_di.vr.take() {
            vr.close_to(
                &di_dir.join(format!("DI_VoltExceptions{NAME_SUFFIX}.csv")),
                errors,
            );
        }
        ckt.em_di.voltage_file_is_open = false;
    }
}

/// Pascal `TSystemMeter.Save` (l.3452): the cumulative system-meter registers
/// → `SystemMeter_1.csv` (in `DI_Dir` on this path — `CloseAllDIFiles` only
/// runs under `SaveDemandInterval`). Pascal also sets `GlobalResult` /
/// `LastResultFile` to the bare CSV name; that bookkeeping is executive state
/// the solve loop cannot reach and no gate observes it across a solve —
/// deliberately not modeled (the next `Export` overwrites it regardless).
fn system_meter_save(ckt: &mut Circuit, errors: &mut Vec<String>) {
    let year = ckt.solution.year;
    let mut sm = MeterStream::new("Year, ");
    sm.write_str(
        "kWh, kvarh, \"Peak kW\", \"peak kVA\", \"Losses kWh\", \"Losses kvarh\", \
         \"Peak Losses kW\"\n",
    );
    sm.write_str(&year.to_string());
    ckt.em_di.system_meter.write_registers(&mut sm);
    sm.write_str("\n");
    sm.close_to(
        &ckt.em_di
            .di_dir
            .join(format!("SystemMeter{NAME_SUFFIX}.csv")),
        errors,
    );
}

/// Pascal `TEnergyMeter.WriteTotalsFile` (l.3573): sum every enabled meter's
/// registers (× its `TotalsMask`) and write the one-row `Totals_1.csv`.
fn write_totals_file(ckt: &mut Circuit, store: &dyn ElemStore, errors: &mut Vec<String>) {
    let mut reg_sum = vec![0.0; NUM_EM_REGISTERS];
    for &r in &ckt.energy_meters {
        let em = store
            .obj(r)
            .as_any()
            .downcast_ref::<EnergyMeter>()
            .expect("energy_meters holds EnergyMeter objects");
        if em.enabled() {
            for (sum, (reg, mask)) in reg_sum
                .iter_mut()
                .zip(em.registers().iter().zip(em.totals_mask()))
            {
                *sum += reg * mask;
            }
        }
    }
    let names = first_meter_register_names(ckt, store);
    let mut fm = MeterStream::new("Year");
    for name in &names {
        fm.write_str(&format!(", \"{name}\""));
    }
    fm.write_str("\n");
    fm.write_str(&ckt.solution.year.to_string());
    for v in &reg_sum {
        fm.write_dbl(*v);
    }
    fm.write_str("\n");
    fm.close_to(
        &ckt.em_di.di_dir.join(format!("Totals{NAME_SUFFIX}.csv")),
        errors,
    );
}

/// Pascal `TEnergyMeterObj.WriteDemandIntervalData` (l.2944), called from the
/// `TakeSample` tail under `SaveDemandInterval`: the per-meter DI row (verbose
/// only), the class `DI_RegisterTotals` accumulation (always), and the
/// phase-voltage report row (when its file is open).
pub(super) fn write_meter_demand_interval_data(
    meter_ref: crate::elements::traits::ElemRef,
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
) {
    let dbl_hour = ckt.solution.dbl_hour;
    let di_verbose = ckt.em_di.di_verbose;
    let em = downcast_meter(store, meter_ref);

    if di_verbose && em.di_file_is_open() {
        let derivatives = em.derivatives().to_vec();
        if let Some(di) = em.di_stream_mut() {
            di.write_dbl(dbl_hour);
            for d in &derivatives {
                di.write_dbl(*d);
            }
            di.write_str("\n");
        }
    }

    // Add to Class demand interval registers.
    let contrib: Vec<f64> = em
        .derivatives()
        .iter()
        .zip(em.totals_mask())
        .map(|(d, m)| d * m)
        .collect();

    // Phase Voltage Report row, if open: max/min/avg pu per phase per vbase
    // (`0.001 ×` — the accumulators carry `|V|/kVBase`, i.e. 1000·pu).
    if em.v_phase_report_open() {
        let (vbase_list, _) = em.vbase_view();
        let (vmax, vmin, vaccum, vcount) = em.v_phase_view();
        let mut rows: Vec<f64> = Vec::new();
        for (i, &vb) in vbase_list.iter().enumerate() {
            if vb > 0.0 {
                for j in 0..3 {
                    rows.push(0.001 * vmax[i * 3 + j]);
                }
                for j in 0..3 {
                    rows.push(0.001 * vmin[i * 3 + j]);
                }
                for j in 0..3 {
                    let idx = i * 3 + j;
                    let avg = if vcount[idx] == 0 {
                        0.0
                    } else {
                        vaccum[idx] / vcount[idx] as f64
                    };
                    rows.push(0.001 * avg);
                }
            }
        }
        if let Some(phv) = em.phv_stream_mut() {
            phv.write_dbl(dbl_hour);
            for v in rows {
                phv.write_dbl(v);
            }
            phv.write_str("\n");
        }
    }

    for (tot, c) in ckt.em_di.di_register_totals.iter_mut().zip(&contrib) {
        *tot += c;
    }
}

/// The `TEnergyMeter.SampleAll` demand-interval tail (l.911-925): the system
/// meter sample (+ its SDI row), then — under `SaveDemandInterval` — the
/// `DI_Totals` row, the class-totals clear, and the optional overload /
/// voltage-exception report rows.
pub(super) fn sample_all_di_tail(ckt: &mut Circuit, store: &mut dyn ElemStore, sys: &SysCtx) {
    // `TSystemMeter.TakeSample` (l.3491): total source energy + total losses.
    let c_power = total_power_from_sources(ckt, store, sys) * 0.001; // kW
    let c_losses = ckt.losses(store, sys) * 0.001; // kW
    let trapezoidal = ckt.trapezoidal_integration;
    let interval_hrs = ckt.solution.interval_hrs;
    let dbl_hour = ckt.solution.dbl_hour;
    ckt.em_di
        .system_meter
        .take_sample(c_power, c_losses, trapezoidal, interval_hrs);
    if ckt.em_di.system_meter.this_meter_di_file_is_open {
        // `TSystemMeter.WriteDemandIntervalData` (l.3532).
        let peak_kw = ckt.em_di.system_meter.peak_kw;
        let peak_kva = ckt.em_di.system_meter.peak_kva;
        let peak_losses_kw = ckt.em_di.system_meter.peak_losses_kw;
        if let Some(sdi) = ckt.em_di.sdi.as_mut() {
            sdi.write_dbl(dbl_hour);
            sdi.write_dbl(c_power.re);
            sdi.write_dbl(c_power.im);
            sdi.write_dbl(peak_kw);
            sdi.write_dbl(peak_kva);
            sdi.write_dbl(c_losses.re);
            sdi.write_dbl(c_losses.im);
            sdi.write_dbl(peak_losses_kw);
            sdi.write_str("\n");
        }
    }

    if ckt.em_di.save_demand_interval {
        // Write Totals Demand interval file row.
        let totals = ckt.em_di.di_register_totals.clone();
        if let Some(tdi) = ckt.em_di.tdi.as_mut() {
            tdi.write_dbl(dbl_hour);
            for v in &totals {
                tdi.write_dbl(*v);
            }
            tdi.write_str("\n");
        }
        ckt.em_di.clear_di_totals();
        if ckt.em_di.overload_file_is_open {
            write_overload_report(ckt, store, sys);
        }
        if ckt.em_di.voltage_file_is_open {
            write_voltage_report(ckt);
        }
    }
}

/// Pascal `TEnergyMeter.WriteOverloadReport` (l.3171): one `DI_Overloads` row
/// per overloaded, non-shunt, enabled PD element. Seasonal ratings (dss_capi
/// 0.15.x `55400a29`, WP-U1.5 E2): the entry gate uses the element's BASE
/// `NormAmps`/`EmergAmps`, then the overload test + reported ratings use the
/// globally-synced season index (`Circuit::seasonal_rating_idx`), guarded by
/// `0 <= idx < NumAmpRatings` and applied to ANY PDElement — the 0.14.5 baseline
/// restricted this
/// override to `ClassName = 'line'` and re-read the XYCurve per element with a
/// state-mutating `DSS.SeasonalRating := FALSE`-on-miss (not reproduced; the
/// index is precomputed at solve time by `sync_seasonal_rating_idx`).
fn write_overload_report(ckt: &mut Circuit, store: &mut dyn ElemStore, sys: &SysCtx) {
    let node_v = ckt.solution.node_v.clone();
    let dbl_hour = ckt.solution.dbl_hour;
    let seasonal_idx = ckt.seasonal_rating_idx;
    for &r in &ckt.pd_elements {
        let elem = store.ckt_elem_mut(r);
        if !elem.cd().enabled || elem.is_shunt() {
            continue;
        }
        // Entry gate: BASE ratings (Pascal `(PdElem.Normamps > 0.0) or
        // (PdElem.Emergamps > 0.0)`).
        if !(elem.norm_amps() > 0.0 || elem.emerg_amps() > 0.0) {
            continue;
        }
        elem.compute_iterminal(sys, &node_v);
        // `MaxTerminalOneImag`: max |I| over the terminal-1 phase conductors.
        let nphases = elem.cd().nphases;
        let mut cmax = 0.0f64;
        for i in 0..nphases {
            cmax = cmax.max(elem.cd().iterminal[i].norm());
        }
        // Overload test + reported ratings: SEASONAL (Pascal `GetRatings`-equivalent).
        let (norm_amps, emerg_amps) = elem.get_ratings(seasonal_idx);
        if !(cmax > norm_amps || cmax > emerg_amps) {
            continue;
        }

        // `dVector[1..3]`: the per-**phase** currents. Pascal recovers each
        // conductor's phase number by re-parsing the FirstBus node designators;
        // the conductor's global node ref → `MapNodeToBus.NodeNum` is the same
        // number (the bus spec's designator list defines the conductor→node
        // mapping), without the string parse.
        let mut d_vector = [0.0f64; 3];
        let cd = elem.cd();
        if nphases < 3 {
            for i in 0..nphases.min(3) {
                let nref = cd.node_ref[i];
                if nref > 0 {
                    let k = ckt.map_node_to_bus[nref].node_num;
                    if (1..=3).contains(&k) {
                        d_vector[k as usize - 1] = cd.iterminal[i].norm();
                    }
                }
            }
        } else {
            for (i, d) in d_vector.iter_mut().enumerate() {
                *d = cd.iterminal[i].norm();
            }
        }

        let full_name = pd_full_name(store, r);
        // `Buses[MapNodeToBus[NodeRef[1]].BusRef].kVBase`.
        let kv_base = {
            let nref = store.ckt_elem(r).cd().node_ref[0];
            if nref > 0 {
                ckt.buses[ckt.map_node_to_bus[nref].bus_ref].kv_base
            } else {
                0.0
            }
        };

        if let Some(ov) = ckt.em_di.ov.as_mut() {
            ov.write_dbl(dbl_hour);
            ov.write_str(&format!(", \"{full_name}\""));
            ov.write_dbl(norm_amps);
            ov.write_dbl(emerg_amps);
            ov.write_dbl(if norm_amps > 0.0 {
                cmax / norm_amps * 100.0
            } else {
                0.0
            });
            ov.write_dbl(if emerg_amps > 0.0 {
                cmax / emerg_amps * 100.0
            } else {
                0.0
            });
            ov.write_dbl(kv_base);
            for v in d_vector {
                ov.write_dbl(v);
            }
            // Pascal `' ' + Char(10)` — a trailing space before the newline.
            ov.write_str(" \n");
        }
    }
}

/// One pass of the voltage-exception scan (Pascal `WriteVoltageReport`, each
/// half): over buses selected by `select`, count under/over-voltage buses and
/// track the extreme per-unit values + their buses.
struct VoltScan {
    under_count: i32,
    over_count: i32,
    under_vmin: f64,
    over_vmax: f64,
    min_bus: Option<usize>,
    max_bus: Option<usize>,
}

fn scan_voltages(ckt: &Circuit, select: impl Fn(f64) -> bool) -> VoltScan {
    let node_v = &ckt.solution.node_v;
    let mut s = VoltScan {
        under_count: 0,
        over_count: 0,
        under_vmin: ckt.normal_max_volts,
        over_vmax: ckt.normal_min_volts,
        min_bus: None,
        max_bus: None,
    };
    for (i, bus) in ckt.buses.iter().enumerate() {
        if !select(bus.kv_base) {
            continue;
        }
        let mut bus_counted = false;
        for &nref in &bus.ref_no {
            let vmagpu = node_v[nref].norm() / bus.kv_base * 0.001;
            if vmagpu <= 0.1 {
                continue; // ignore neutral buses
            }
            if vmagpu < s.under_vmin {
                s.under_vmin = vmagpu;
                s.min_bus = Some(i);
            }
            if vmagpu > s.over_vmax {
                s.over_vmax = vmagpu;
                s.max_bus = Some(i);
            }
            if vmagpu < ckt.normal_min_volts {
                if !bus_counted {
                    s.under_count += 1;
                    bus_counted = true;
                }
            } else if vmagpu > ckt.normal_max_volts && !bus_counted {
                s.over_count += 1;
                bus_counted = true;
            }
        }
    }
    s
}

/// Pascal `TEnergyMeter.WriteVoltageReport` (l.3612): the `DI_VoltExceptions`
/// row — the primary (> 1 kV base) scan then the LV (0 < base ≤ 1 kV) scan.
fn write_voltage_report(ckt: &mut Circuit) {
    let dbl_hour = ckt.solution.dbl_hour;
    let primary = scan_voltages(ckt, |kv| kv > 1.0);
    let lv = scan_voltages(ckt, |kv| kv > 0.0 && kv <= 1.0);
    let bus_name = |b: Option<usize>| -> String {
        b.and_then(|i| ckt.bus_list.name(i))
            .unwrap_or("")
            .to_string()
    };
    let (p_min, p_max) = (bus_name(primary.min_bus), bus_name(primary.max_bus));
    let (l_min, l_max) = (bus_name(lv.min_bus), bus_name(lv.max_bus));
    if let Some(vr) = ckt.em_di.vr.as_mut() {
        vr.write_dbl(dbl_hour);
        vr.write_str(&format!(", {}", primary.under_count));
        vr.write_dbl(primary.under_vmin);
        vr.write_str(&format!(", {}", primary.over_count));
        vr.write_dbl(primary.over_vmax);
        vr.write_str(&format!(", {p_min}"));
        vr.write_str(&format!(", {p_max}"));
        vr.write_str(&format!(", {}", lv.under_count));
        vr.write_dbl(lv.under_vmin);
        vr.write_str(&format!(", {}", lv.over_count));
        vr.write_dbl(lv.over_vmax);
        vr.write_str(&format!(", {l_min}"));
        vr.write_str(&format!(", {l_max}"));
        vr.write_str("\n");
    }
}

/// The demand-interval half of Pascal `TEnergyMeter.ResetAll` (l.851): close
/// any open DI files, then — under `SaveDemandInterval` — (re)create
/// `<OutputDirectory><CaseName>/DI_yr_<year>` and the `DI_Totals` stream.
/// The register resets themselves stay in `reset_all_meters` (the caller).
pub(super) fn reset_all_di(
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
    output_directory: &Path,
    errors: &mut Vec<String>,
) {
    if ckt.em_di.di_files_are_open {
        close_all_di_files(ckt, store, errors);
    }
    if ckt.em_di.save_demand_interval {
        let case_path = output_directory.join(&ckt.case_name);
        if !case_path.is_dir()
            && let Err(e) = std::fs::create_dir(&case_path)
        {
            errors.push(format!(
                "Error making  Directory: \"{}\". {e}",
                case_path.display()
            ));
        }
        let di_dir = case_path.join(format!("DI_yr_{}", ckt.solution.year));
        if !di_dir.is_dir()
            && let Err(e) = std::fs::create_dir(&di_dir)
        {
            errors.push(format!(
                "Error making Demand Interval Directory: \"{}\". {e}",
                di_dir.display()
            ));
        }
        ckt.em_di.di_dir = di_dir;
        create_fdi_totals(ckt, store);
    }
}
