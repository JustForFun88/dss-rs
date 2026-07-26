//! The `CktElement` and `DssObject` trait impls for `StorageController`: typed
//! property accessors, `element=`/shape resolution, `PropertySideEffects`,
//! `EndEdit`, and `MakeLike`.

use num_complex::Complex64;

use crate::elements::control::control_elem::RefSnapshot;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemId, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject};

use super::{MonPhase, StorageController, StorageCtrlMode, prop};

impl CktElement for StorageController {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    /// Pascal `TControlElem.FControlledElement` - the element this control
    /// acts on (`None` when it drives a list rather than a single element).
    fn controlled_element(&self) -> Option<crate::elements::traits::ElemId> {
        self.ccd.controlled_element
    }

    /// Pascal `TControlElem.CalcYPrim`: leave YPrim NIL — `BuildYMatrix` skips it.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// Pascal `TControlElem.GetCurrents`: always zero.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }

    /// Pascal `TStorageControllerObj.MakePosSequence`
    /// (`Controls/StorageController.pas:834`): adopt the monitored element's
    /// phase / conductor counts and attach terminal 1 to its bus, then run the
    /// base bus rename (`inherited`). Pascal NIL-guards `MonitoredElement`;
    /// `ctx.monitored` is `None` in the same case. (Probe `S6` confirms this
    /// class is `makeposseq`-safe.)
    fn make_pos_sequence(&mut self, ctx: &PosSeqCtx) -> PosSeqPlan {
        if let Some(m) = &ctx.monitored {
            // FNphases := MonitoredElement.NPhases; Nconds := FNphases
            self.ccd.cd.nphases = m.nphases;
            self.ccd.cd.set_nconds(m.nphases);
            // Setbus(1, MonitoredElement.GetBus(ElementTerminal))
            let t = self.ccd.element_terminal as usize;
            let bus = t
                .checked_sub(1)
                .and_then(|k| m.bus_names.get(k))
                .cloned()
                .unwrap_or_default();
            self.ccd.cd.set_bus(1, &bus);
            // ReAllocMem(cBuffer, ..) + CondOffset: no persistent field — the
            // sampler reads the monitored terminal through the dispatch env.
        }
        // inherited MakePosSequence -> base bus rename.
        PosSeqPlan::base()
    }

    /// Pascal `TControlElem.MonitoredElement` — resolved so the exec applier can
    /// build [`PosSeqCtx::monitored`] before calling [`Self::make_pos_sequence`].
    fn monitored_element_ref(&self) -> Option<ElemId> {
        self.ccd.monitored_element
    }
}

impl StorageController {
    /// Pascal `TStorageControllerObj.MakeLike` — copies essentially every
    /// dispatch setting (unlike GenDispatcher's terminal-only copy).
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force Reallocation of terminal stuff

        self.ccd.monitored_element = other.ccd.monitored_element;
        self.monitored_full_name = other.monitored_full_name.clone();
        self.mon_snap = other.mon_snap.clone();
        self.ccd.element_terminal = other.ccd.element_terminal;
        self.f_mon_phase = other.f_mon_phase;

        self.f_kw_target = other.f_kw_target;
        self.f_kw_target_low = other.f_kw_target_low;
        self.f_kw_threshold = other.f_kw_threshold;
        self.disp_factor = other.disp_factor;
        self.f_pct_kw_band = other.f_pct_kw_band;
        self.f_kw_band = other.f_kw_band;
        self.f_pct_kw_band_low = other.f_pct_kw_band_low;
        self.f_kw_band_low = other.f_kw_band_low;
        self.reset_level = other.reset_level;
        self.f_kw_band_specified = other.f_kw_band_specified;

        self.storage_name_list = other.storage_name_list.clone();
        self.fleet_size = self.storage_name_list.len() as i32;
        if self.fleet_size > 0 {
            self.weights = other.weights.clone();
        }

        self.discharge_mode = other.discharge_mode;
        self.charge_mode = other.charge_mode;
        self.discharge_trigger_time = other.discharge_trigger_time;
        self.charge_trigger_time = other.charge_trigger_time;
        self.pct_kw_rate = other.pct_kw_rate;
        self.pct_charge_rate = other.pct_charge_rate;
        self.pct_fleet_reserve = other.pct_fleet_reserve;
        self.yearly_shape = other.yearly_shape.clone();
        self.daily_shape = other.daily_shape.clone();
        self.duty_shape = other.duty_shape.clone();
        self.yearly_shape_obj = other.yearly_shape_obj.clone();
        self.daily_shape_obj = other.daily_shape_obj.clone();
        self.duty_shape_obj = other.duty_shape_obj.clone();
        self.ccd.show_event_log = other.ccd.show_event_log;
        self.inhibit_hrs = other.inhibit_hrs;
        self.up_ramp_time = other.up_ramp_time;
        self.flat_time = other.flat_time;
        self.dn_ramp_time = other.dn_ramp_time;

        self.seasons = other.seasons;
        if self.seasons > 1 {
            self.season_targets = other.season_targets.clone();
            self.season_targets_low = other.season_targets_low.clone();
        }
    }
}

impl DssObject for StorageController {
    fn data(&self) -> &DssObjData {
        &self.ccd.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.ccd.cd.obj
    }

    fn as_control(&self) -> Option<&dyn crate::elements::control::control_elem::ControlElem> {
        Some(self)
    }
    fn as_control_mut(
        &mut self,
    ) -> Option<&mut dyn crate::elements::control::control_elem::ControlElem> {
        Some(self)
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            KW_TARGET => self.f_kw_target,
            KW_TARGET_LOW => self.f_kw_target_low,
            PCT_KW_BAND => self.f_pct_kw_band,
            KW_BAND => self.f_kw_band,
            PCT_KW_BAND_LOW => self.f_pct_kw_band_low,
            KW_BAND_LOW => self.f_kw_band_low,
            TIME_DISCHARGE_TRIGGER => self.discharge_trigger_time,
            TIME_CHARGE_TRIGGER => self.charge_trigger_time,
            PCT_RATE_KW => self.pct_kw_rate,
            PCT_RATE_CHARGE => self.pct_charge_rate,
            PCT_RESERVE => self.pct_fleet_reserve,
            KW_NEED => self.kw_needed,
            // Pascal `[SilentReadOnly, ReadByFunction]` fleet aggregates: the
            // text/props render is intercepted by SILENT_READ_ONLY (→ '' always,
            // function-only offset -1), so this arm is unreachable in practice.
            // Return 0 as a placeholder (the real fleet aggregate isn't computed
            // on the `&self` accessor).
            KWH_TOTAL | KW_TOTAL | KWH_ACTUAL | KW_ACTUAL => 0.0,
            T_UP => self.up_ramp_time,
            T_FLAT => self.flat_time,
            T_DN => self.dn_ramp_time,
            KW_THRESHOLD => self.f_kw_threshold,
            DISP_FACTOR => self.disp_factor,
            RESET_LEVEL => self.reset_level,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("StorageController has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KW_TARGET => self.f_kw_target = value,
            KW_TARGET_LOW => self.f_kw_target_low = value,
            PCT_KW_BAND => self.f_pct_kw_band = value,
            KW_BAND => self.f_kw_band = value,
            PCT_KW_BAND_LOW => self.f_pct_kw_band_low = value,
            KW_BAND_LOW => self.f_kw_band_low = value,
            TIME_DISCHARGE_TRIGGER => self.discharge_trigger_time = value,
            TIME_CHARGE_TRIGGER => self.charge_trigger_time = value,
            PCT_RATE_KW => self.pct_kw_rate = value,
            PCT_RATE_CHARGE => self.pct_charge_rate = value,
            PCT_RESERVE => self.pct_fleet_reserve = value,
            // kWNeed is SilentReadOnly — writes are ignored.
            KW_NEED => {}
            // SilentReadOnly fleet aggregates — writes are silently ignored.
            KWH_TOTAL | KW_TOTAL | KWH_ACTUAL | KW_ACTUAL => {}
            T_UP => self.up_ramp_time = value,
            T_FLAT => self.flat_time = value,
            T_DN => self.dn_ramp_time = value,
            KW_THRESHOLD => self.f_kw_threshold = value,
            DISP_FACTOR => self.disp_factor = value,
            RESET_LEVEL => self.reset_level = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("StorageController has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            TERMINAL => self.ccd.element_terminal,
            MON_PHASE => self.f_mon_phase.ordinal(),
            MODE_DISCHARGE => self.discharge_mode.ordinal(),
            MODE_CHARGE => self.charge_mode.ordinal(),
            INHIBIT_TIME => self.inhibit_hrs,
            SEASONS => self.seasons,
            // Pascal `Weights` IndirectCount reads its element count from
            // `FleetSize` (`StorageController.pas` `PropertyOffset2 = @FleetSize`),
            // kept in sync with the storage-name-list length. The DoubleArray count
            // path + the schema `$dssLength: ElementList` resolve it via this slot.
            ELEMENT_LIST => self.fleet_size,
            _ => unreachable!("StorageController has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            TERMINAL => self.ccd.element_terminal = value,
            MON_PHASE => self.f_mon_phase = MonPhase::from_ordinal(value),
            MODE_DISCHARGE => {
                self.discharge_mode =
                    StorageCtrlMode::from_ordinal(value).unwrap_or(self.discharge_mode)
            }
            MODE_CHARGE => {
                self.charge_mode = StorageCtrlMode::from_ordinal(value).unwrap_or(self.charge_mode)
            }
            INHIBIT_TIME => self.inhibit_hrs = value,
            SEASONS => self.seasons = value,
            _ => unreachable!("StorageController has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::EVENT_LOG => self.ccd.show_event_log,
            prop::ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("StorageController has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::EVENT_LOG => self.ccd.show_event_log = value,
            prop::ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("StorageController has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            ELEMENT => self.monitored_full_name.clone(),
            YEARLY => self.yearly_shape.clone(),
            DAILY => self.daily_shape.clone(),
            DUTY => self.duty_shape.clone(),
            _ => unreachable!("StorageController has no string property {idx}"),
        }
    }

    fn get_string_list(&self, idx: usize) -> Vec<String> {
        match idx {
            prop::ELEMENT_LIST => self.storage_name_list.clone(),
            _ => unreachable!("StorageController has no string-list property {idx}"),
        }
    }
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        match idx {
            prop::ELEMENT_LIST => self.storage_name_list = value,
            _ => unreachable!("StorageController has no string-list property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        use prop::*;
        match idx {
            // Pascal `FWeights` is NIL until an ElementList allocates it; a NIL
            // array dumps as "" (not "[]"), so report empty as absent.
            WEIGHTS => (!self.weights.is_empty()).then_some(self.weights.as_slice()),
            SEASON_TARGETS => Some(self.season_targets.as_slice()),
            SEASON_TARGETS_LOW => Some(self.season_targets_low.as_slice()),
            _ => unreachable!("StorageController has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use prop::*;
        match idx {
            WEIGHTS => self.weights = value,
            SEASON_TARGETS => self.season_targets = value,
            SEASON_TARGETS_LOW => self.season_targets_low = value,
            _ => unreachable!("StorageController has no double-array property {idx}"),
        }
    }
    /// Pascal `Weights` IndirectCount: the element count comes from the
    /// ElementList (`PropertyOffset3 = @FStorageNameList`).
    fn array_size(&self, idx: usize) -> usize {
        match idx {
            prop::WEIGHTS => self.storage_name_list.len(),
            _ => unreachable!("StorageController has no function-sized array {idx}"),
        }
    }

    /// `element=`/`yearly=`/`daily=`/`duty=` resolution.
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        use prop::*;
        let load_shape = || resolved.and_then(|o| o.cloned::<LoadShapeObj>());
        match idx {
            ELEMENT => {
                self.monitored_full_name = name.clone();
                match resolved {
                    Some(o) => {
                        self.ccd.monitored_element = Some(o.id());
                        let elem = o.ckt().expect("element= resolves against circuit classes");
                        self.mon_snap = Some(RefSnapshot::capture(name, elem));
                    }
                    None => {
                        self.ccd.monitored_element = None;
                        self.mon_snap = None;
                    }
                }
            }
            // The dispatch shapes feed `DoLoadShapeMode`; snapshot-clone the
            // resolved LoadShapeObj (the WP4.2 `FetchLineCode` pattern), keeping
            // the name for the dump.
            YEARLY => {
                self.yearly_shape = name;
                self.yearly_shape_obj = load_shape();
            }
            DAILY => {
                self.daily_shape = name;
                self.daily_shape_obj = load_shape();
            }
            DUTY => {
                self.duty_shape = name;
                self.duty_shape_obj = load_shape();
            }
            _ => unreachable!("StorageController has no object-ref property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TStorageControllerObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            KW_TARGET => {
                let casemult = if self.discharge_mode == StorageCtrlMode::CurrentPeakShave {
                    1000.0
                } else {
                    1.0
                };
                self.f_kw_threshold = self.f_kw_target * 0.75 * casemult;
                self.half_kw_band = self.f_pct_kw_band / 200.0 * self.f_kw_target * casemult;
                self.f_kw_band = 2.0 * self.half_kw_band;
                self.f_pct_kw_band = self.f_kw_band / self.f_kw_target * 100.0; // sync
            }
            PCT_KW_BAND => {
                let casemult = if self.discharge_mode == StorageCtrlMode::CurrentPeakShave {
                    1000.0
                } else {
                    1.0
                };
                self.half_kw_band = self.f_pct_kw_band / 200.0 * self.f_kw_target * casemult;
                self.f_kw_band = 2.0 * self.half_kw_band;
                self.f_kw_band_specified = false;
            }
            KW_BAND => {
                let casemult = if self.discharge_mode == StorageCtrlMode::CurrentPeakShave {
                    1000.0
                } else {
                    1.0
                };
                self.half_kw_band = self.f_kw_band / 2.0 * casemult;
                self.f_pct_kw_band = self.f_kw_band / self.f_kw_target * 100.0; // sync
                self.f_kw_band_specified = true;
            }
            KW_TARGET_LOW | PCT_KW_BAND_LOW => {
                let casemult = if self.charge_mode == StorageCtrlMode::CurrentPeakShaveLow {
                    1000.0
                } else {
                    1.0
                };
                self.half_kw_band_low =
                    self.f_pct_kw_band_low / 200.0 * self.f_kw_target_low * casemult;
                self.f_kw_band_low = self.half_kw_band_low * 2.0;
            }
            KW_BAND_LOW => {
                let casemult = if self.charge_mode == StorageCtrlMode::CurrentPeakShaveLow {
                    1000.0
                } else {
                    1.0
                };
                self.half_kw_band_low = self.f_kw_band_low / 2.0 * casemult;
                // D10 (WP-U1.6, `a14c3f1f`, SVN r4058): 0.14.5 synced the wrong
                // fields here (`FpctkWBand := FkWBandLow / FkWTarget * 100`) when
                // `kWBandLow` was set; the fix targets the *Low* pair. Adopted
                // (r4133-aligned bug fix, plan §1.4 "EPRI r4133 wins").
                self.f_pct_kw_band_low = self.f_kw_band_low / self.f_kw_target_low * 100.0;
            }
            MODE_DISCHARGE => {
                if self.discharge_mode == StorageCtrlMode::Follow {
                    self.discharge_trigger_time = 12.0; // Noon
                }
            }
            MON_PHASE => {
                if self.f_mon_phase.ordinal() > self.ccd.cd.nphases as i32 {
                    self.ccd.cd.obj.push_error(format!(
                        "Error: Monitored phase ({}) must be less than or equal to number of phases ({}). ",
                        self.f_mon_phase.ordinal(),
                        self.ccd.cd.nphases
                    ));
                    self.f_mon_phase = MonPhase::Phase(1);
                }
            }
            ELEMENT_LIST => {
                // Levelize the list.
                self.fleet_list_changed = true;
                self.element_list_specified = true;
                self.fleet_size = self.storage_name_list.len() as i32;
                self.weights = vec![1.0; self.fleet_size.max(0) as usize];
            }
            SEASONS => {
                let n = self.seasons.max(0) as usize;
                self.season_targets.resize(n, 0.0);
                self.season_targets_low.resize(n, 0.0);
            }
            DISP_FACTOR => {
                if self.disp_factor <= 0.0 || self.disp_factor > 1.0 {
                    self.disp_factor = 1.0;
                }
            }
            INHIBIT_TIME => self.inhibit_hrs = self.inhibit_hrs.max(1),
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
    }
}

impl crate::elements::control::control_elem::ControlElem for StorageController {
    fn ccd(&self) -> &crate::elements::control::control_elem::ControlElemData {
        &self.ccd
    }
    fn ccd_mut(&mut self) -> &mut crate::elements::control::control_elem::ControlElemData {
        &mut self.ccd
    }
    fn control_kind(&self) -> crate::elements::control::control_elem::ControlClass {
        crate::elements::control::control_elem::ControlClass::StorageCtrl
    }
}
