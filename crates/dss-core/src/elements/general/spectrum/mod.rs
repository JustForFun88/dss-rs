//! `Spectrum` — harmonic spectrum (per-unit magnitude + angle by harmonic).
//! Port of Pascal `General/Spectrum.pas`. As a second guinea pig it adds two
//! things TCC_Curve doesn't exercise: a scaled array property (`%Mag`, stored
//! per-unit, displayed as percent) and a string property (`CSVFile`).
//!
//! Pascal `TProp`: `NumHarm=1`, `Harmonic=2`, `pctMag=3` (modern name `%Mag`),
//! `Angle=4`, `CSVFile=5`; the base class appends `Like=6`.

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::obj::base::{DssObjData, DssObject, FileLoad};
use crate::obj::props::{PropDef, PropFlags, define_properties};
use crate::support::complexutil::pdeg_to_complex;
use dss_parser::{Parser, ParserVars};

// Pascal `TSpectrumProp` ordinals + the property table. `%Mag` is stored
// per-unit: the parser multiplies by 0.01 and the getter divides by it
// (Pascal `PropertyScale := 0.01`).
define_properties! {
    class "Spectrum", abbrev true;
    1 NUM_HARM => PropDef::integer("NumHarm").flags(PropFlags::SUPPRESS_JSON);
    2 HARMONIC => PropDef::double_array("Harmonic", NUM_HARM)
        .flags(PropFlags::REQUIRED_IN_SPEC_SET);
    3 PCT_MAG  => PropDef::double_array("%Mag", NUM_HARM)
        .scale(0.01)
        .flags(PropFlags::REQUIRED_IN_SPEC_SET);
    4 ANGLE    => PropDef::double_array("Angle", NUM_HARM)
        .flags(PropFlags::REQUIRED_IN_SPEC_SET);
    5 CSV_FILE => PropDef::string("CSVFile").flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
}

use prop::{ANGLE, CSV_FILE, HARMONIC, NUM_HARM, PCT_MAG};

/// A `Spectrum` instance (`TSpectrumObj`).
#[derive(Debug, Clone)]
pub struct SpectrumObj {
    data: DssObjData,
    num_harm: i32,
    harm_array: Option<Vec<f64>>,
    /// Per-unit magnitudes (`puMagArray`); displayed as percent via the scale.
    pu_mag_array: Option<Vec<f64>>,
    angle_array: Option<Vec<f64>>,
    csvfile: String,
    /// `MultArray` — the complex per-harmonic phasors built by `SetMultArray`,
    /// each shifted so the fundamental sits at zero phase. Consumed only by the
    /// harmonic solution mode (`get_mult`); nothing in the property dump reads it.
    mult_array: Option<Vec<Complex64>>,
    /// Deferred `CSVFile` reads queued for the executive (the WP5.2b
    /// `FileLoad` pattern — the property hook can't reach the filesystem).
    pending_file_loads: Vec<FileLoad>,
}

impl SpectrumObj {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            data: DssObjData::new(name.into().to_lowercase(), prop::NUM_PROPS),
            num_harm: 0,
            harm_array: None,
            pu_mag_array: None,
            angle_array: None,
            csvfile: String::new(),
            mult_array: None,
            pending_file_loads: Vec::new(),
        }
    }

    /// Pascal `TSpectrumObj.ReadCSVFile` (Spectrum.pas:278): parse up to
    /// `NumHarm` rows of `harmonic, %mag, angle` (AuxParser formats — comma or
    /// space separated), `%Mag` scaled to per-unit, then shrink `NumHarm` to
    /// the count actually read.
    fn read_csv_file(&mut self, content: &str) {
        let n = self.num_harm.max(0) as usize;
        let mut harm = vec![0.0; n];
        let mut mag = vec![0.0; n];
        let mut ang = vec![0.0; n];

        let mut parser = Parser::new();
        parser.set_auto_increment(false);
        let vars = ParserVars::new();

        let mut i = 0usize;
        for line in content.lines() {
            if i >= n {
                break;
            }
            parser.set_cmd_string(line);
            parser.next_param(&vars);
            harm[i] = parser.make_double(&vars).unwrap_or(0.0);
            parser.next_param(&vars);
            mag[i] = parser.make_double(&vars).unwrap_or(0.0) * 0.01;
            parser.next_param(&vars);
            ang[i] = parser.make_double(&vars).unwrap_or(0.0);
            i += 1;
        }

        harm.truncate(i);
        mag.truncate(i);
        ang.truncate(i);
        self.harm_array = Some(harm);
        self.pu_mag_array = Some(mag);
        self.angle_array = Some(ang);
        self.num_harm = i as i32; // reset number of points
    }

    /// Pascal `HarmArrayHasaZero`: the 1-based index of the first zero harmonic,
    /// or `None`.
    fn harm_array_has_a_zero(&self) -> Option<usize> {
        let arr = self.harm_array.as_ref()?;
        arr.iter().position(|&v| v == 0.0).map(|i| i + 1)
    }

    /// Pascal `TSpectrumObj.SetMultArray`: rotate every harmonic phasor so the
    /// fundamental (the `Round(HarmArray)=1` entry) sits at zero phase, and cache
    /// the result in `MultArray`. Called from `EndEdit` once all three input
    /// arrays are present and no zero harmonic was given.
    fn set_mult_array(&mut self) {
        let (Some(harm), Some(pu_mag), Some(angle)) = (
            self.harm_array.as_ref(),
            self.pu_mag_array.as_ref(),
            self.angle_array.as_ref(),
        ) else {
            return;
        };
        let n = self.num_harm.max(0) as usize;
        if harm.len() < n || pu_mag.len() < n || angle.len() < n {
            return;
        }

        let mut fund_angle = 0.0;
        for i in 0..n {
            // Pascal `Round` is banker's rounding (ties-to-even); harmonics are
            // integers in practice, so this is exact.
            if harm[i].round_ties_even() as i64 == 1 {
                fund_angle = angle[i];
                break;
            }
        }

        let mut mult = Vec::with_capacity(n);
        for i in 0..n {
            mult.push(pdeg_to_complex(pu_mag[i], angle[i] - harm[i] * fund_angle));
        }
        self.mult_array = Some(mult);
    }

    /// Pascal `TSpectrumObj.HarmArray`: the harmonic ordinals in this spectrum
    /// (used by the harmonic frequency sweep).
    pub fn harmonics(&self) -> Option<&[f64]> {
        self.harm_array.as_deref()
    }

    /// Pascal `TSpectrumObj.GetMult`: the complex multiplier for harmonic `h`
    /// (matched to the nearest 0.01), or zero if `h` is not in the spectrum.
    pub fn get_mult(&self, h: f64) -> Complex64 {
        let (Some(harm), Some(mult)) = (self.harm_array.as_ref(), self.mult_array.as_ref()) else {
            return Complex64::ZERO;
        };
        for (i, &hv) in harm.iter().enumerate() {
            if (h - hv).abs() < 0.01 {
                return mult[i];
            }
        }
        Complex64::ZERO
    }
}

impl DssObject for SpectrumObj {
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
            NUM_HARM => self.num_harm,
            _ => unreachable!("Spectrum has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            NUM_HARM => self.num_harm = value,
            _ => unreachable!("Spectrum has no integer property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            CSV_FILE => self.csvfile.clone(),
            _ => unreachable!("Spectrum has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            CSV_FILE => self.csvfile = value,
            _ => unreachable!("Spectrum has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            HARMONIC => self.harm_array.as_deref(),
            PCT_MAG => self.pu_mag_array.as_deref(),
            ANGLE => self.angle_array.as_deref(),
            _ => unreachable!("Spectrum has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            HARMONIC => self.harm_array = Some(value),
            PCT_MAG => self.pu_mag_array = Some(value),
            ANGLE => self.angle_array = Some(value),
            _ => unreachable!("Spectrum has no array property {idx}"),
        }
    }

    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        match idx {
            NUM_HARM => {
                let n = self.num_harm.max(0) as usize;
                // Pascal only reallocs HarmArray if it was already allocated;
                // the AngleArray is always (re)allocated and zero-filled, while
                // puMagArray is left untouched here.
                if self.harm_array.is_some() {
                    realloc(&mut self.harm_array, n);
                }
                self.angle_array = if n == 0 { None } else { Some(vec![0.0; n]) };
            }
            CSV_FILE => {
                // Pascal `DoCSVFile` (Spectrum.pas:210) runs here, but the hook
                // can't reach the filesystem/current dir: queue the read for
                // the executive (the WP5.2b deferred-`FileLoad` path).
                self.pending_file_loads.push(FileLoad {
                    prop: CSV_FILE,
                    filename: self.csvfile.clone(),
                });
            }
            _ => {}
        }
    }

    fn take_file_loads(&mut self) -> Vec<FileLoad> {
        std::mem::take(&mut self.pending_file_loads)
    }

    /// Apply a resolved `CSVFile` (Pascal `DoCSVFile` → `ReadCSVFile`).
    fn apply_file_load(&mut self, load: &FileLoad, content: &str, _errors: &mut Vec<String>) {
        if load.prop == CSV_FILE {
            self.read_csv_file(content);
        }
    }

    fn end_edit(&mut self) {
        // Pascal `TSpectrum.EndEdit`: only act once `HarmArray` is allocated.
        // A zero harmonic is rejected (DoSimpleMsg 65001) and `MultArray` is left
        // unbuilt; otherwise, with all three input arrays present, build the
        // rotated multiplier array the harmonic solution consumes.
        if self.harm_array.is_none() {
            return;
        }
        if self.harm_array_has_a_zero().is_some() {
            // DoSimpleMsg in the original; the error sink is not wired into
            // `end_edit`, so this stays a no-op guard (`MultArray` not built).
            return;
        }
        if self.pu_mag_array.is_some() && self.angle_array.is_some() {
            self.set_mult_array();
        }
    }

    fn make_like(&mut self, other: &dyn DssObject) {
        // Pascal `TSpectrumObj.MakeLike`: `inherited MakeLike` (copy the
        // PrpSequence), then copy the fields.
        self.data.copy_prp_sequence_from(other.data());
        self.num_harm = other.get_i32(NUM_HARM);
        self.harm_array = other.get_f64_array(HARMONIC).map(<[f64]>::to_vec);
        self.pu_mag_array = other.get_f64_array(PCT_MAG).map(<[f64]>::to_vec);
        self.angle_array = other.get_f64_array(ANGLE).map(<[f64]>::to_vec);
        self.csvfile = other.get_string(CSV_FILE);
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Pascal `ReAllocmem`: grow/shrink keeping surviving values; 0 frees (NIL).
fn realloc(arr: &mut Option<Vec<f64>>, n: usize) {
    if n == 0 {
        *arr = None;
    } else {
        arr.get_or_insert_with(Vec::new).resize(n, 0.0);
    }
}
