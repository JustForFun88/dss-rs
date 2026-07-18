//! `LoadShape` — per-unit multiplier curve indexed by hour.
//! Port of Pascal `General/LoadShape.pas` (`TLoadShapeObj`). A `DSS_OBJECT`
//! class (no terminals, no YPrim) consumed by Load/VSource/Storage as the
//! daily/yearly/duty multiplier source (wired in WP5.3) and by the time-series
//! solution modes (WP5.8).
//!
//! Scope (WP5.2a/b, WPG.1): the in-memory core — fixed- and variable-interval
//! data, `GetMultAtHour` (the consumer-facing lookup), `Normalize`,
//! `SetMaxPandQ`, lazy mean/std-dev, `MakeLike`, and the four file inputs —
//! `CSVFile`/`PQCSVFile` (text) and `SngFile`/`DblFile` (little-endian binary)
//! — all deferred to the executive via [`FileLoad`], which resolves the path
//! relative to the script's current directory and hands back the content
//! (text or raw bytes per [`FileLoad::binary`]). `Interval = 0` reads
//! `(hour, value)` pairs; `Interval <> 0` reads a bare value stream (Pascal
//! `TLoadShapeObj.ReadCSVFile`/`ReadSngFile`/`ReadDblFile`/`Read2ColCSVFile`,
//! `LoadShape.pas`). Single-precision *storage* (`sP`/`sH`, Pascal's "take
//! the opportunity to use float32 data" branch when no `QMult` is set yet) IS
//! modeled: `read_sng_file` keeps the authoritative f32 arrays (`s_p`/`s_h`)
//! and the lookup/statistics/normalize paths reproduce Pascal's
//! single-precision arithmetic bit-exactly (FPC probe:
//! `tools/fpc/single_prec_probe.pas`); `p_mult`/`hour` then hold the widened
//! f64 *view* the property getters/`SetMaxPandQ` read (values identical —
//! Pascal widens on every such read too). `sQ` is script-unreachable (the
//! float32 branch requires `dQ = NIL`, and any later `QMult=` edit runs
//! `UseFloat64` first) and is not modeled — greppable:
//! NOT_PORTED(LoadShape `sQ` single storage — API-only, unreachable from
//! script). Memory-mapped files (`MemoryMapping=Yes`, WPG.17) ARE modeled by
//! *eager* reads: the actual `CreateFileMapping`/`mmap` I/O strategy is not
//! ported (no observable numerics), but each MMF file reader
//! (`read_{sng,dbl,csv,pq_csv}_file`, and the raw `mult=(sngfile=…)`
//! directive) reads the whole file up front into the f64 `p_mult`/`q_mult`
//! arrays with the MMF-path-specific semantics (Pascal
//! `InterpretDblArrayMMF`/`LoadFileFeatures`): no `NumPoints` shrink, the
//! text accept-set byte filter, `sngfile` widened into `dP` (f64, not `sP`),
//! and the `(<mmFileCmd>)` property round-trip. The existing f64
//! `get_mult_at_hour` double path is then the correct lookup (`s_p` stays
//! `None`). Single-column `csvfile=` under MMF is an upstream defect
//! (division-by-zero in the oracle's lazy byte reader for one-column files);
//! the eager reader reads it correctly and no gated deck exercises it — see
//! `compute.rs::read_csv_file`.
//!
//! Split into submodules (no behavioral change): the struct, its constructor and
//! the simple accessors live here; the curve lookup / normalization / statistics
//! / CSV-parse algorithms are in [`compute`], and the `DssObject` property trait
//! in [`accessors`].

#[cfg(test)]
mod tests;

mod accessors;
mod compute;

use crate::obj::base::{DssObjData, FileLoad};
use crate::obj::props::{PropDef, PropFlags, define_properties};

// Pascal `TLoadShapeProp` ordinals (1..22) + the property table.
define_properties! {
    class "LoadShape", abbrev true, enums enums;
    1  NPTS      => PropDef::integer("NPts").flags(PropFlags::SUPPRESS_JSON);
    2  INTERVAL  => PropDef::double("Interval")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::REQUIRED_IN_SPEC_SET);
    3  MULT      => PropDef::double_array("Mult", NPTS)
        .flags(PropFlags::REDUNDANT)
        .redundant_with(PMULT);
    4  HOUR      => PropDef::double_array("Hour", NPTS).flags(PropFlags::REQUIRED_IN_SPEC_SET);
    5  MEAN      => PropDef::double("Mean");
    6  STDDEV    => PropDef::double("StdDev");
    7  CSVFILE   => PropDef::string("CSVFile").flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    8  SNGFILE   => PropDef::string("SngFile").flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    9  DBLFILE   => PropDef::string("DblFile").flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    10 ACTION    => PropDef::action("Action", enums.load_shape_action);
    11 QMULT     => PropDef::double_array("QMult", NPTS);
    12 USEACTUAL => PropDef::boolean("UseActual");
    13 PMAX      => PropDef::double("PMax");
    14 QMAX      => PropDef::double("QMax");
    15 SINTERVAL => PropDef::double("SInterval")
        .scale(1.0 / 3600.0)
        .flags(PropFlags::REDUNDANT | PropFlags::NON_NEGATIVE)
        .redundant_with(INTERVAL);
    16 MINTERVAL => PropDef::double("MInterval")
        .scale(1.0 / 60.0)
        .flags(PropFlags::REDUNDANT | PropFlags::NON_NEGATIVE)
        .redundant_with(INTERVAL);
    17 PBASE     => PropDef::double("PBase");
    18 QBASE     => PropDef::double("QBase");
    19 PMULT     => PropDef::double_array("PMult", NPTS).flags(PropFlags::REQUIRED_IN_SPEC_SET);
    20 PQCSVFILE => PropDef::string("PQCSVFile").flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    21 MEMORYMAPPING => PropDef::boolean("MemoryMapping").flags(PropFlags::ORDERING_FIRST);
    22 INTERPOLATION => PropDef::mapped_string_enum("Interpolation", enums.load_shape_interp);
}

/// `Avg` interpolation ordinal (`TLoadShapeInterp.Avg`).
const INTERP_AVG: i32 = 0;
/// `Edge` interpolation ordinal (`TLoadShapeInterp.Edge`).
const INTERP_EDGE: i32 = 1;

/// A `LoadShape` instance (`TLoadShapeObj`).
#[derive(Debug, Clone)]
pub struct LoadShapeObj {
    data: DssObjData,
    /// Number of points in the curve (Pascal `NumPoints`).
    num_points: i32,
    /// Fixed interval in hours; `0.0` means variable interval (use `hour`).
    interval: f64,
    /// Active-power multipliers (Pascal `dP`). `None` = NIL pointer. While
    /// single storage is live (`s_p` is `Some`) this holds the **widened f64
    /// view** of `s_p` (identical values; Pascal widens on every dP-shaped
    /// read of single data too).
    p_mult: Option<Vec<f64>>,
    /// Reactive-power multipliers (Pascal `dQ`); `None` falls back to `p_mult`.
    q_mult: Option<Vec<f64>>,
    /// Hour values for variable interval (Pascal `dH`); `None` = even spacing.
    /// Widened view of `s_h` while single storage is live.
    hour: Option<Vec<f64>>,
    /// Pascal `sP`: the authoritative single-precision multipliers, taken by
    /// `ReadSngFile` when no `QMult` is set (`LoadShape.pas:1116-1143`).
    /// `Some` ⇔ the Pascal pointer is assigned; the lookup, statistics and
    /// normalize paths then run Pascal's single-precision arithmetic.
    s_p: Option<Vec<f32>>,
    /// Pascal `sH`: single-precision hours (variable interval only).
    s_h: Option<Vec<f32>>,
    /// Mean / std-dev (Pascal `FMean`/`FStdDev`); lazily computed unless set.
    f_mean: f64,
    f_std_dev: f64,
    /// Pascal `FStdDevCalculated`: true when mean/std-dev were set explicitly
    /// (skips the on-demand recompute).
    std_dev_calculated: bool,
    /// Pascal `UseActual`: multipliers are absolute (kW/kvar), not per-unit.
    use_actual: bool,
    /// Peak P / peak-coincident Q (Pascal `MaxP`/`MaxQ`).
    max_p: f64,
    max_q: f64,
    /// Pascal `MaxQSpecified`: `QMax` set explicitly (do not recompute it).
    max_q_specified: bool,
    /// Normalization bases (Pascal `BaseP`/`BaseQ`).
    base_p: f64,
    base_q: f64,
    /// Pascal `interpolation` (`Avg`=0, `Edge`=1).
    interpolation: i32,
    /// Pascal `UseMMF` (memory-mapped files). When set, the file readers
    /// eager-load into `p_mult`/`q_mult` with the MMF-path semantics and the
    /// array properties dump the directive string below.
    use_mmf: bool,
    /// Pascal `mmFileCmd` / `mmFileCmdQ`: the original file directive string for
    /// the P / Q multipliers under MMF, dumped by `GetPropertyValue` as
    /// `(<cmd>)` (`LoadShape.pas:1846-1867`). Empty ⇒ dumps `()`.
    mm_file_cmd: String,
    mm_file_cmd_q: String,
    /// Hunt cache for the variable-interval lookup (Pascal `LastValueAccessed`,
    /// a 0-based index into the 0-based arrays; ctor sets it to 1).
    last_value_accessed: usize,
    csvfile: String,
    sngfile: String,
    dblfile: String,
    pqcsvfile: String,
    /// Deferred file reads queued by `CSVFile` (drained by the executive).
    pending_file_loads: Vec<FileLoad>,
    /// Deferred binary saves queued by `Action=SngSave/DblSave` (drained by the
    /// executive, which owns `OutputDirectory`/`GlobalResult`).
    pending_shape_saves: Vec<crate::obj::base::ShapeSave>,
    /// WPG.19: `action=normalize`/`ln` requested while a file directive is still
    /// pending (`mult=(file=…) ln`). Pascal reads the file inline, so `Normalize`
    /// sees the data; our deferred read makes it run in `run_deferred_actions`
    /// (after the file loads). `false` ⇒ normalize ran inline (numeric mult).
    pending_normalize: bool,
}

impl LoadShapeObj {
    /// Pascal `TLoadShapeObj.Create`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            data: DssObjData::new(name.into().to_ascii_lowercase(), prop::NUM_PROPS),
            num_points: 0,
            interval: 1.0,
            p_mult: None,
            q_mult: None,
            hour: None,
            s_p: None,
            s_h: None,
            f_mean: 0.0,
            f_std_dev: 0.0,
            std_dev_calculated: false,
            use_actual: false,
            max_p: 1.0,
            max_q: 0.0,
            max_q_specified: false,
            base_p: 0.0,
            base_q: 0.0,
            interpolation: INTERP_AVG,
            use_mmf: false,
            mm_file_cmd: String::new(),
            mm_file_cmd_q: String::new(),
            last_value_accessed: 1,
            csvfile: String::new(),
            sngfile: String::new(),
            dblfile: String::new(),
            pqcsvfile: String::new(),
            pending_file_loads: Vec::new(),
            pending_shape_saves: Vec::new(),
            pending_normalize: false,
        }
    }

    /// Pascal `UseActual`: the multipliers are absolute (kW/kvar), not per-unit.
    /// Consumers (Load/VSource `CalcDailyMult`) read this to set `ShapeIsActual`.
    pub fn use_actual(&self) -> bool {
        self.use_actual
    }

    /// Pascal `NumPoints`: the number of points in the curve, read directly
    /// (not through the property getter) by `SolveLD1`/`SolveLD2`
    /// (`ckt.LoadDurCurveObj.NumPoints`).
    pub fn num_points(&self) -> i32 {
        self.num_points
    }

    /// Pascal `PMultipliers^` — the raw active-power multiplier array (1-indexed
    /// in Pascal; 0-indexed here). `AggregateProfiles` (WP-AD.5) reads it directly
    /// rather than through the interpolating `mult(i)` getter. Empty when the
    /// shape carries no P data (a freed NIL pointer upstream).
    pub fn p_mult_raw(&self) -> &[f64] {
        self.p_mult.as_deref().unwrap_or(&[])
    }

    /// Pascal `QMultipliers^` — the raw reactive-power multiplier array, or `None`
    /// when `QMultipliers = nil` (the `AggregateProfiles` PF-fallback path keys off
    /// exactly this NIL check, Circuit.pas:1763).
    pub fn q_mult_raw(&self) -> Option<&[f64]> {
        self.q_mult.as_deref()
    }

    /// Pascal `MaxP` / `MaxQ`: the peak active power and its coincident reactive
    /// power, set by `SetMaxPandQ` (used by the `UseActual` `SetkWkvar` path).
    pub fn max_p(&self) -> f64 {
        self.max_p
    }
    pub fn max_q(&self) -> f64 {
        self.max_q
    }

    /// Test-only constructor: a fixed-interval curve straight from a P-multiplier
    /// vector (`npts = p.len()`), bypassing the parser. Used by consumers'
    /// unit tests that need a live shape (e.g. CapControl FOLLOW sampling).
    #[cfg(test)]
    pub(crate) fn fixed_interval_for_test(name: &str, interval: f64, p: Vec<f64>) -> Self {
        let mut s = Self::new(name);
        s.num_points = p.len() as i32;
        s.interval = interval;
        s.p_mult = Some(p);
        s
    }
}

/// Pascal `ReAllocmem` semantics for the data setters: an empty parse result is
/// a freed (NIL) pointer, so it reads back as an empty dump; non-empty becomes
/// an allocated array.
pub(super) fn store_array(value: Vec<f64>) -> Option<Vec<f64>> {
    if value.is_empty() { None } else { Some(value) }
}
