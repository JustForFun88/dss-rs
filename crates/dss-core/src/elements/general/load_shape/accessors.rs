//! The `DssObject` property trait for `LoadShapeObj`: the typed get/set
//! accessors, the `Action` handler, file-load plumbing, `PropertySideEffects`,
//! `EndEdit` and `MakeLike`.

use crate::obj::base::{DssObjData, DssObject, FileLoad};

use super::prop::{
    CSVFILE, DBLFILE, HOUR, INTERPOLATION, INTERVAL, MEAN, MEMORYMAPPING, MINTERVAL, MULT, NPTS,
    PBASE, PMAX, PMULT, PQCSVFILE, QBASE, QMAX, QMULT, SINTERVAL, SNGFILE, STDDEV, USEACTUAL,
};
use super::{LoadShapeObj, store_array};

impl DssObject for LoadShapeObj {
    fn data(&self) -> &DssObjData {
        &self.data
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.data
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            NPTS => self.num_points,
            INTERPOLATION => self.interpolation,
            _ => unreachable!("LoadShape has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            NPTS => self.num_points = value,
            INTERPOLATION => self.interpolation = value,
            _ => unreachable!("LoadShape has no integer property {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        match idx {
            // Interval and its second/minute aliases share one field; the
            // engine applies the scale on the way out.
            INTERVAL | SINTERVAL | MINTERVAL => self.interval,
            MEAN => self.mean(),
            STDDEV => self.std_dev(),
            PMAX => self.max_p,
            QMAX => self.max_q,
            PBASE => self.base_p,
            QBASE => self.base_q,
            _ => unreachable!("LoadShape has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        match idx {
            INTERVAL | SINTERVAL | MINTERVAL => self.interval = value,
            // Pascal `Set_Mean`/`Set_StdDev`: mark as externally provided.
            MEAN => {
                self.f_mean = value;
                self.std_dev_calculated = true;
            }
            STDDEV => {
                self.f_std_dev = value;
                self.std_dev_calculated = true;
            }
            PMAX => self.max_p = value,
            QMAX => self.max_q = value,
            PBASE => self.base_p = value,
            QBASE => self.base_q = value,
            _ => unreachable!("LoadShape has no double property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            USEACTUAL => self.use_actual,
            MEMORYMAPPING => self.use_mmf,
            _ => unreachable!("LoadShape has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            USEACTUAL => self.use_actual = value,
            MEMORYMAPPING => self.use_mmf = value,
            _ => unreachable!("LoadShape has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            CSVFILE => self.csvfile.clone(),
            SNGFILE => self.sngfile.clone(),
            DBLFILE => self.dblfile.clone(),
            PQCSVFILE => self.pqcsvfile.clone(),
            _ => unreachable!("LoadShape has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            CSVFILE => self.csvfile = value,
            SNGFILE => self.sngfile = value,
            DBLFILE => self.dblfile = value,
            PQCSVFILE => self.pqcsvfile = value,
            _ => unreachable!("LoadShape has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            // Mult and PMult are the same array (Pascal `dP`).
            MULT | PMULT => self.p_mult.as_deref(),
            HOUR => self.hour.as_deref(),
            QMULT => self.q_mult.as_deref(),
            _ => unreachable!("LoadShape has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        let stored = store_array(value);
        match idx {
            // Pascal runs `UseFloat64` before loading `dP`/`dH`/`dQ`
            // (`LoadShape.pas:767/779/804`): a `Mult=`/`Hour=`/`QMult=` edit
            // ends single-precision storage.
            MULT | PMULT => {
                self.use_float64();
                self.p_mult = stored;
            }
            HOUR => {
                self.use_float64();
                self.hour = stored;
            }
            QMULT => {
                self.use_float64();
                self.q_mult = stored;
            }
            _ => unreachable!("LoadShape has no array property {idx}"),
        }
    }

    /// Pascal `StringEnumActionProperty` for `Action`.
    fn do_action(&mut self, ordinal: i32, errors: &mut Vec<String>) {
        match ordinal {
            0 => self.normalize(errors), // Normalize
            // DblSave / SngSave write binary files — not ported.
            _ => errors.push(format!(
                "LoadShape.{}: Action=DblSave/SngSave (binary file output) is not ported.",
                self.data.name()
            )),
        }
    }

    fn take_file_loads(&mut self) -> Vec<FileLoad> {
        std::mem::take(&mut self.pending_file_loads)
    }

    /// Apply a resolved text file: `CSVFile` (Pascal `DoCSVFile`) or
    /// `PQCSVFile` (Pascal `Do2ColCSVFile`).
    fn apply_file_load(&mut self, load: &FileLoad, content: &str, _errors: &mut Vec<String>) {
        match load.prop {
            CSVFILE => self.read_csv_file(content),
            PQCSVFILE => self.read_pq_csv_file(content),
            _ => {}
        }
    }

    /// Apply a resolved binary file: `SngFile` (Pascal `ReadSngFile`) or
    /// `DblFile` (Pascal `ReadDblFile`).
    fn apply_binary_file_load(
        &mut self,
        load: &FileLoad,
        content: &[u8],
        _errors: &mut Vec<String>,
    ) {
        match load.prop {
            SNGFILE => self.read_sng_file(content),
            DBLFILE => self.read_dbl_file(content),
            _ => {}
        }
    }

    /// Pascal `TLoadShapeObj.PropertySideEffects` (the parts that affect the
    /// in-memory model; `PrpSequence` bookkeeping is display-only and inert).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        match idx {
            // Setting Mult/PMult/QMult invalidates the cached statistics.
            MULT | PMULT | QMULT => {
                self.std_dev_calculated = false;
            }
            // Pascal `DoCSVFile`/`DoSngFile`/`DoDblFile` run here, but the hook
            // can't reach the filesystem/current dir: queue the read for the
            // executive (Pascal `PropertySideEffects`, `LoadShape.pas:709-714`).
            CSVFILE => {
                self.std_dev_calculated = false;
                self.pending_file_loads
                    .push(FileLoad::text(CSVFILE, self.csvfile.clone()));
            }
            SNGFILE => {
                self.std_dev_calculated = false;
                self.pending_file_loads
                    .push(FileLoad::binary(SNGFILE, self.sngfile.clone()));
            }
            DBLFILE => {
                self.std_dev_calculated = false;
                self.pending_file_loads
                    .push(FileLoad::binary(DBLFILE, self.dblfile.clone()));
            }
            // Pascal `Do2ColCSVFile` (`PQCSVFile`, `LoadShape.pas:715-716`).
            PQCSVFILE => {
                self.pending_file_loads
                    .push(FileLoad::text(PQCSVFILE, self.pqcsvfile.clone()));
            }
            QMAX => self.max_q_specified = true,
            // Pascal `LoadShape.pas:745-747`: enabling memory mapping forces
            // the data back to f64 (MMF itself is not ported).
            MEMORYMAPPING => {
                if self.use_mmf {
                    self.use_float64();
                }
            }
            // Interval and Hour are mutually exclusive specs.
            INTERVAL => self.data.clear_seq(HOUR),
            HOUR => {
                self.interval = 0.0;
                self.data.clear_seq(INTERVAL);
            }
            _ => {}
        }
    }

    /// Pascal `TLoadShape.EndEdit`: recompute peaks once data exists.
    fn end_edit(&mut self) {
        if self.p_mult.is_some() {
            self.set_max_p_and_q();
        }
    }

    /// Pascal `TLoadShapeObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        self.data.copy_prp_sequence_from(other.data());
        let Some(o) = other.as_any().downcast_ref::<LoadShapeObj>() else {
            return;
        };
        self.num_points = o.num_points;
        self.interval = o.interval;
        self.p_mult = o.p_mult.clone();
        self.q_mult = o.q_mult.clone();
        // With a fixed interval the hour array is dropped (Pascal frees dH).
        self.hour = if self.interval > 0.0 {
            None
        } else {
            o.hour.clone()
        };
        // Pascal `LoadShape.pas:887-913`: single-precision storage is copied
        // as singles (the widened views above already hold identical values).
        self.s_p = o.s_p.clone();
        self.s_h = o.s_h.clone();
        self.use_actual = o.use_actual;
        self.use_mmf = o.use_mmf;
        self.base_p = o.base_p;
        self.base_q = o.base_q;
        self.set_max_p_and_q();
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
