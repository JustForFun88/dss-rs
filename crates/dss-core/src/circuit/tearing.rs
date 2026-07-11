//! A-Diakoptics circuit-tearing state (`DIAKOPTICS_PSTCALC_PLAN.md` WP-AD.2
//! Stage B).
//!
//! Behavioral spec = **official r3723 Delphi** (plan D10):
//! `.inputs/electricdss-code-r3723-trunk/Version8/Source/Common/Circuit.pas`
//! field group 205–231 (`Coverage`/`Actual_Coverage`, `Num_SubCkts`,
//! `Link_Branches`, `PConn_Names`/`PConn_Voltages`, `Locations`, `BusZones`,
//! the `TSparse_Complex` slots `Contours`/`ContoursT`/`ZLL`/`ZCT`/`ZCC`/`Y4`/
//! `Ic`, `VIndex`, `MeTISZones` at 321). The vendored dss_capi rewrite
//! (`Common/Circuit.pas:255–301`) is the `TDSSContext` structure map.
//!
//! Stage B reads/writes the string/array/scalar members; the sparse-complex
//! matrix slots (`Calc_C_Matrix`/`Calc_ZLL`/`Calc_ZCC`/`Calc_Y4` products) land
//! as typed placeholders consumed by WP-AD.3. `Node_dV`/`Ic_Local` (Pascal
//! `Ymatrix.pas:430–434`) are deliberately absent — allocated-but-never-read
//! scaffolding (plan D5), so there is nothing to mirror.

use crate::support::sparse_math::SparseComplex;

/// The tearing/A-Diakoptics member block of `TDSSCircuit`.
///
/// Grouped into one struct (rather than scattered onto [`Circuit`]) because it
/// is a single cohesive concern touched only by the tearing/diakoptics code
/// paths. Pascal citations are on the individual fields.
///
/// [`Circuit`]: crate::circuit::Circuit
#[derive(Debug, Clone, Default)]
pub struct AdTearing {
    /// `Coverage` (Circuit.pas:208): the user-requested backbone coverage for
    /// `Get_paths_4_Coverage`/`Refine_BusLevels`. Ctor default `0.9`
    /// (Circuit.pas:604); `set Coverage=` overwrites it.
    pub coverage: f64,
    /// `Actual_Coverage` (Circuit.pas:209): the coverage actually achieved after
    /// the tearing algorithm runs. Ctor default `-1` = "no coverage yet"
    /// (Circuit.pas:605).
    pub actual_coverage: f64,
    /// `Num_SubCkts` (Circuit.pas:215): the requested number of sub-circuits for
    /// `Tear_Circuit`. Ctor default `CPU_Cores-1` (Circuit.pas:606); `set
    /// Num_SubCircuits=` overwrites it. Plan D6: every test fixes this
    /// explicitly — no gate depends on the CPU-derived default.
    pub num_sub_ckts: i32,
    /// `Link_Branches` (Circuit.pas:216): the PDE names of the link branches
    /// between zones (the tearing cut). Filled by `Tear_Circuit`; user-set via
    /// `set LinkBranches=[...]` for the manual-partition path (D10).
    pub link_branches: Vec<String>,
    /// `PConn_Names` (Circuit.pas:217): the bus-1 name of each link branch = the
    /// point of connection where the per-zone `VSource` is stamped.
    pub pconn_names: Vec<String>,
    /// `PConn_Voltages` (Circuit.pas:218): six doubles per location — three
    /// phases of (|V|/1000, angle°) measured at the point of connection.
    pub pconn_voltages: Vec<f64>,
    /// `Locations` (Circuit.pas:219): the incidence-column indices at which the
    /// tearing places a zone boundary (bus indices in `Inc_Mat_Cols`).
    pub locations: Vec<i32>,
    /// `BusZones` (Circuit.pas:220): the zone id (as the raw METIS partition
    /// label string) assigned to each distinct zone in `Create_MeTIS_Zones`.
    pub bus_zones: Vec<String>,
    /// `MeTISZones` (Circuit.pas:321): the per-bus zone-label lines loaded from
    /// the `.part.N` partition file (after the D5 first-line swap).
    pub metis_zones: Vec<String>,
    /// `UseUserLinks` — set by `set UseMyLinkBranches=True` (official
    /// ExecOptions 134). When true and `Link_Branches` is non-empty,
    /// `Tear_Circuit` takes the manual-partition branch (D10). `Tear_Circuit`
    /// clears it after consuming it (Circuit.pas:1934).
    pub use_user_links: bool,
    /// `VIndex` (Circuit.pas:231): the offset of this sub-circuit's bus 1 in the
    /// interconnected node list (set by `SendIdx2Actors` in WP-AD.3).
    pub v_index: i32,
    /// Transient: `set ADiakoptics=yes` sets this so `do_set_cmd` runs
    /// `ADiakopticsInit` after its option-loop field borrow ends (the init needs
    /// `&mut Dss`, not just the circuit). Never persisted.
    pub pending_ad_init: bool,

    /// `Contours` (Circuit.pas:224) — WP-AD.3 placeholder.
    pub contours: SparseComplex,
    /// `ContoursT` (Circuit.pas:223) — WP-AD.3 placeholder.
    pub contours_t: SparseComplex,
    /// `ZLL` (Circuit.pas:225) — WP-AD.3 placeholder.
    pub zll: SparseComplex,
    /// `ZCT` (Circuit.pas:226) — WP-AD.3 placeholder.
    pub zct: SparseComplex,
    /// `ZCC` (Circuit.pas:227) — WP-AD.3 placeholder.
    pub zcc: SparseComplex,
    /// `Y4` (Circuit.pas:228) — WP-AD.3 placeholder.
    pub y4: SparseComplex,
    /// `Ic` (Circuit.pas:230) — WP-AD.3 placeholder.
    pub ic: SparseComplex,
}

impl AdTearing {
    /// Pascal `TDSSCircuit.Create` tearing-block init (Circuit.pas:604–620).
    ///
    /// `num_sub_ckts` is the CPU-derived default (`CPU_Cores-1`,
    /// `std::thread::available_parallelism()`; plan D6). No test depends on it —
    /// the AD tests always issue `set Num_SubCircuits=`.
    pub fn new() -> Self {
        let cpu_cores = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(1);
        Self {
            coverage: 0.9,
            actual_coverage: -1.0,
            // `Num_SubCkts := CPU_Cores-1` verbatim (Circuit.pas:606) — no
            // clamp, so a 1-core host yields 0 exactly as upstream. Plan D6:
            // every AD test fixes `Num_SubCircuits` explicitly, so this default
            // is never gated.
            num_sub_ckts: cpu_cores - 1,
            ..Default::default()
        }
    }
}
