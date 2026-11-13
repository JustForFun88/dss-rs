//! A-Diakoptics circuit-tearing state: the `TDSSCircuit` field group
//! (`Coverage`/`Actual_Coverage`, `Num_SubCkts`, `Link_Branches`,
//! `PConn_Names`/`PConn_Voltages`, `Locations`, `BusZones`, the
//! `TSparse_Complex` slots `Contours`/`ContoursT`/`ZLL`/`ZCT`/`ZCC`/`Y4`/`Ic`,
//! `VIndex`, `MeTISZones`).
//!
//! The tearing reads and writes the string/array/scalar members. The
//! sparse-complex matrix slots hold the `Calc_C_Matrix`/`Calc_ZLL`/`Calc_ZCC`/
//! `Calc_Y4` products of the A-Diakoptics solve. `Node_dV`/`Ic_Local` are
//! deliberately absent: OpenDSS allocates them and never reads them, so there
//! is nothing to mirror.

use crate::support::sparse_math::SparseComplex;

// BUG(C2): every field is `pub` and the executive writes them, and its path arrays are worked
// on by `impl Circuit` elsewhere. `AdTearing` keeps its fields private and owns that work.
// BUG(C5): derives `Clone` while it owns the tearing arrays that the tearing commands update.
// The type that owns mutable state is not `Clone`.
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
    /// Num_SubCircuits=` overwrites it. Every test fixes this explicitly, so no
    /// gate depends on the CPU-derived default.
    pub num_sub_ckts: i32,
    /// `Link_Branches` (Circuit.pas:216): the PDE names of the link branches
    /// between zones (the tearing cut). Filled by `Tear_Circuit`; user-set via
    /// `set LinkBranches=[...]` for the manual-partition path.
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
    /// the `.part.N` partition file (after the `Create_MeTIS_Zones` first-line swap).
    pub metis_zones: Vec<String>,
    /// `UseUserLinks` — set by `set UseMyLinkBranches=True` (official
    /// ExecOptions 134). When true and `Link_Branches` is non-empty,
    /// `Tear_Circuit` takes the manual-partition branch. `Tear_Circuit`
    /// clears it after consuming it (Circuit.pas:1934).
    pub use_user_links: bool,
    /// `VIndex` (Circuit.pas:231): the offset of this sub-circuit's bus 1 in the
    /// interconnected node list (set by `SendIdx2Actors`).
    pub v_index: i32,

    /// `Longest_paths` (Circuit.pas:210): the flattened list of bus (incidence
    /// column) indices of every traced longest path, appended by
    /// `Append2PathsArray`. Working state of `Get_paths_4_Coverage`.
    pub longest_paths: Vec<i32>,
    /// `Path_Idx` (Circuit.pas:211): the `Longest_paths` offset at which each
    /// traced path begins. `length(Path_Idx)-1` is the number of new paths the
    /// `Refine_BusLevels` command reports.
    pub path_idx: Vec<i32>,
    /// `Buses_Covered` (Circuit.pas:212): the estimated bus count of each path
    /// (one quadrant). Summed / `length(Inc_Mat_Cols)` = `Actual_Coverage`.
    pub buses_covered: Vec<i32>,
    /// `New_Graph` (Circuit.pas:214): the latest weighted graph — the backbone
    /// path `get_longest_path` extracts, consumed by `Get_paths_4_Coverage`.
    pub new_graph: Vec<i32>,
    // BUG(C2): the A-Diakoptics option handler parks its init request here, and the set
    // command reads and clears it after its option loop. The handler returns the request to
    // its caller.
    /// Transient: `set ADiakoptics=yes` sets this so `do_set_cmd` runs
    /// `ADiakopticsInit` after its option-loop field borrow ends (the init needs
    /// `&mut Dss`, not just the circuit). Never persisted.
    pub pending_ad_init: bool,

    /// `Contours` (Circuit.pas:224) — built by `exec::diakoptics::matrices`.
    pub contours: SparseComplex,
    /// `ContoursT` (Circuit.pas:223) — built by `exec::diakoptics::matrices`.
    pub contours_t: SparseComplex,
    /// `ZLL` (Circuit.pas:225) — built by `exec::diakoptics::matrices`.
    pub zll: SparseComplex,
    /// `ZCT` (Circuit.pas:226) — built by `exec::diakoptics::matrices`.
    pub zct: SparseComplex,
    /// `ZCC` (Circuit.pas:227) — built by `exec::diakoptics::matrices`.
    pub zcc: SparseComplex,
    /// `Y4` (Circuit.pas:228) — built by `exec::diakoptics::matrices`.
    pub y4: SparseComplex,
    /// `Ic` (Circuit.pas:230) — set by `exec::diakoptics::solve`.
    pub ic: SparseComplex,
}

impl AdTearing {
    /// Pascal `TDSSCircuit.Create` tearing-block init (Circuit.pas:604–620).
    ///
    /// `num_sub_ckts` is the CPU-derived default (`CPU_Cores-1`,
    /// `std::thread::available_parallelism()`). No test depends on it —
    /// the AD tests always issue `set Num_SubCircuits=`.
    pub fn new() -> Self {
        let cpu_cores = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(1);
        Self {
            coverage: 0.9,
            actual_coverage: -1.0,
            // `Num_SubCkts := CPU_Cores-1` verbatim (Circuit.pas:606) — no
            // clamp, so a 1-core host yields 0 exactly as OpenDSS. Every AD test
            // fixes `Num_SubCircuits` explicitly, so this default is never gated.
            num_sub_ckts: cpu_cores - 1,
            ..Default::default()
        }
    }
}
