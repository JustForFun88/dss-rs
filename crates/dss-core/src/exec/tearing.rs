//! Circuit tearing: `Create_MeTIS_graph`, `Create_MeTIS_Zones`, and the
//! `Num_SubCircuits`/`LinkBranches`/`UseMyLinkBranches` executive surface
//! (`DIAKOPTICS_PSTCALC_PLAN.md` WP-AD.2 Stage B).
//!
//! Behavioral spec = **official r3723 Delphi** (plan D10):
//! `.inputs/electricdss-code-r3723-trunk/Version8/Source/Common/Circuit.pas`
//! (`Create_MeTIS_graph` 1213, `Create_MeTIS_Zones` 1350) and `Solution.pas`
//! (`get_IncMatrix_Row/Col`, `get_PDE_Bus1_Location`). The in-process
//! partitioner substitution and file round-trip live in
//! [`crate::support::partition`] (plan D2; `NOTE(subst-metis)` there).

use crate::circuit::{AdTearing, Circuit};
use crate::exec::Dss;
use crate::exec::registry::DssClass;
use crate::solution::inc_matrix::{IncMatrixState, get_inc_matrix_row};
use crate::support::partition::MetisGraph;
use std::path::PathBuf;

/// Look up the phase count of a PDE by its `Class.Name` (mirrors
/// `SetElementActive(MyName); ActiveCktElement.NPhases`). Returns `None` when
/// the element cannot be resolved.
///
/// NOTE(upstream): on a `SetElementActive` miss Pascal leaves `ActiveCktElement`
/// at its *prior* value, so a caller reading `NPhases` would see whatever was
/// last active — not 0. `build_metis_graph`'s `unwrap_or(0)` weight therefore
/// diverges on an unresolved row, but this is unreachable for a well-formed
/// incidence matrix (every `Inc_Mat_Row` is a real, resolvable PDE), so it has
/// no numeric effect; we prefer a defined 0 over reproducing a stale-state read.
fn nphases_of(classes: &[DssClass], full_name: &str) -> Option<usize> {
    let lower = full_name.to_ascii_lowercase();
    let (cls_name, obj_name) = match lower.split_once('.') {
        Some((c, n)) => (Some(c), n),
        None => (None, lower.as_str()),
    };
    for class in classes {
        if class.kind.is_none() {
            continue;
        }
        if let Some(cn) = cls_name
            && !class.props.class_name().eq_ignore_ascii_case(cn)
        {
            continue;
        }
        if let Some(&oi) = class.name_to_idx.get(obj_name) {
            return class.arena[oi].as_ckt_element().map(|e| e.cd().nphases);
        }
    }
    None
}

/// The class-name prefix of a PDE full name (`myName.Substring(0, pos('.')-1)`).
fn class_prefix(full_name: &str) -> &str {
    match full_name.find('.') {
        Some(p) => &full_name[..p],
        None => full_name,
    }
}

/// Given a nonzero-entry index `jj` in column `i` of the incidence matrix,
/// return the **other terminal's** column (Pascal's `myIntVar` computation,
/// Circuit.pas:1258–1263 / 1280–1285): the paired entry of the same PDE (row)
/// is `jj+1` when it shares the row, else `jj-1`; at the last entry it is
/// `jj-1`. Relies on a 2-terminal PDE's two incidence entries being stored
/// consecutively (insertion order — [`SparseInt::insert`] appends distinct
/// cells).
///
/// [`SparseInt::insert`]: crate::support::sparse_math::SparseInt::insert
fn other_terminal_col(data: &[[i32; 3]], jj: usize) -> i32 {
    let high = data.len().saturating_sub(1);
    if jj < high && data[jj + 1][0] == data[jj][0] {
        data[jj + 1][1]
    } else if jj > 0 {
        data[jj - 1][1]
    } else {
        // jj == 0 with no same-row successor. Unreachable for a well-formed
        // incidence matrix (a 2-terminal PDE's entries are stored as a
        // consecutive pair, so entry 0 is always the first of its pair and
        // takes the `jj+1` branch). Guarded against `data[-1]` UB (D5 philosophy:
        // do not reproduce an out-of-bounds read) — fall back to this column.
        data[jj][1]
    }
}

/// Pascal `TDSSCircuit.Create_MeTIS_graph` (official Circuit.pas:1213): the
/// incidence matrix (already computed hierarchically) is walked column by
/// column, parallel branches are deduplicated, and the per-column adjacency
/// (neighbor column + phase-count weight, Transformers weighted 1) is built.
///
/// The incidence matrix + Laplacian must already be present
/// (`Calc_Inc_Matrix_Org` — the caller runs it). Returns the canonical
/// [`MetisGraph`] (the same data upstream serializes to the `.graph` file); the
/// caller writes the OpenDSS-format text and drives the in-process partition.
pub(crate) fn build_metis_graph(classes: &[DssClass], st: &IncMatrixState) -> MetisGraph {
    let Some(inc_mat) = st.inc_mat.as_ref() else {
        return MetisGraph::default();
    };
    let data = &inc_mat.data;
    let n_cols = st.cols.len();

    // Per-column adjacency and the accumulating unique-PDE list.
    let mut adjacency: Vec<Vec<(i32, i32)>> = Vec::with_capacity(n_cols);
    let mut pde_list: Vec<String> = Vec::new();

    for i in 0..n_cols {
        // `myIdx` = the accepted neighbors of column `i` as (col, nphases).
        let mut my_idx: Vec<(i32, i32)> = Vec::new();
        for (jj, entry) in data.iter().enumerate() {
            if entry[1] != i as i32 {
                continue; // not in column i
            }
            let other_col = other_terminal_col(data, jj);
            // Parallel-branch check: already have this neighbor column?
            if my_idx.iter().any(|&(c, _)| c == other_col) {
                continue;
            }
            // Accept: record the PDE name and the neighbor column + weight.
            let pde_name = st.rows[entry[0] as usize].clone();
            let weight = if class_prefix(&pde_name) != "Transformer" {
                // `SetElementActive; ActiveCktElement.NPhases`.
                nphases_of(classes, &pde_name).unwrap_or(0) as i32
            } else {
                1
            };
            my_idx.push((other_col, weight));
            pde_list.push(pde_name);
        }
        adjacency.push(my_idx);
    }

    // Header edge count `jj`: the number of Inc_Mat_Rows (PDEs) that appear in
    // `pde_list` (Circuit.pas:1318–1330) — i.e. the count of distinct branches.
    let mut num_edges = 0;
    for row in &st.rows {
        if pde_list.iter().any(|p| p.eq_ignore_ascii_case(row)) {
            num_edges += 1;
        }
    }

    MetisGraph {
        n_cols: n_cols as i32,
        num_edges,
        adjacency,
    }
}

/// The `<OutputDirectory><CircuitName_>.graph` path (`GetOutputDirectory +
/// CircuitName_[ActiveActor] + '.graph'`, Circuit.pas:1332). `CircuitName_` is
/// the case name plus a trailing underscore (matching the vendored
/// `ckt24_.graph` artifact name).
pub(crate) fn graph_file_path(output_dir: &std::path::Path, case_name: &str) -> PathBuf {
    output_dir.join(format!("{case_name}_.graph"))
}

/// Error surfaced by the tearing pipeline (mirrors the two `DoErrorMsg` sites in
/// `Tear_Circuit`, Circuit.pas:2037/2040).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TearError {
    /// The incidence matrix could not be built (no circuit / empty topology).
    NoGraph,
}

/// Pascal `TDSSCircuit.Create_MeTIS_Zones` (official Circuit.pas:1350): partition
/// the graph (here in-process via `dss-metis`, plan D2) and translate the
/// per-vertex zone labels into the `Locations`/`BusZones` boundary lists.
///
/// Writes the `<graph>.part.<N>` file (kmetis output format) for
/// fidelity/diagnostics, then applies the parse 1:1:
///  - the D5 first-line swap (`NOTE(upstream-quirk)` below);
///  - the ≥2-consecutive-bus zone rule;
///  - the final `inc(Locations[j])` coordinate adjustment.
///
/// `ad.metis_zones`, `ad.locations`, `ad.bus_zones` are filled on success.
pub(crate) fn create_metis_zones(
    graph: &MetisGraph,
    num_pieces: i32,
    graph_path: &std::path::Path,
    ad: &mut AdTearing,
) -> Result<(), TearError> {
    if graph.n_cols <= 0 {
        return Err(TearError::NoGraph);
    }
    // Partition in-process and persist the `.part.N` (unswapped labels).
    let labels = graph.run_partition(num_pieces);
    let _ = MetisGraph::write_part_file(&labels, graph_path, num_pieces);

    // `MeTISZones` as label strings, in vertex (column) order.
    let mut metis_zones: Vec<String> = labels.iter().map(|l| l.to_string()).collect();

    // NOTE(upstream-quirk): `TextCmd := MeTISZones[1]; Delete(0); Insert(0,
    // TextCmd)` (Circuit.pas:1410–1412) — replaces line 0's zone with line 1's
    // (drops the first bus's zone id, duplicates the second). It shifts the zone
    // boundaries deterministically and compensates `Create_MeTIS_graph`'s
    // dropped column-0 adjacency line. Reproduced 1:1; fixture-pinned.
    if metis_zones.len() >= 2 {
        metis_zones[0] = metis_zones[1].clone();
    }

    let mut locations: Vec<i32> = Vec::new();
    let mut bus_zones: Vec<String> = Vec::new();
    let count = metis_zones.len();
    for i in 0..count {
        if i == 0 {
            locations.push(0);
            bus_zones.push(metis_zones[0].clone());
            continue;
        }
        // Moving to a different zone than the last one recorded?
        if metis_zones[i] != *bus_zones.last().unwrap() {
            // The zone must span ≥2 consecutive buses (else it is a 1-bus zone).
            let big_enough = i < count - 1 && metis_zones[i] == metis_zones[i + 1];
            if big_enough {
                // Verify this zone hasn't been counted before.
                let seen = bus_zones.iter().any(|z| *z == metis_zones[i]);
                if !seen {
                    locations.push(i as i32);
                    bus_zones.push(metis_zones[i].clone());
                }
            }
        }
    }
    // `for j := 0 to High(Locations) do inc(Locations[j])` (Circuit.pas:1452).
    for loc in &mut locations {
        *loc += 1;
    }

    ad.metis_zones = metis_zones;
    ad.locations = locations;
    ad.bus_zones = bus_zones;
    Ok(())
}

/// Split a bracketed/space/comma list value (`[line.l1, line.l2]` or
/// `line.l1 line.l2`) into element names, mirroring the Pascal
/// `InterpretDblArray`/token parse for `set LinkBranches=`.
fn parse_element_list(value: &str) -> Vec<String> {
    value
        .split(['[', ']', ',', ' ', '\t', '(', ')'])
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Interpret a boolean option value (`YES/TRUE/Y/1` => true), the Pascal
/// `InterpretYesNo` truthy set.
fn interpret_yes_no(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "yes" | "true" | "y" | "t" | "1"
    )
}

/// Handle `set` of an A-Diakoptics option not present in the vendored
/// `EXEC_OPTIONS` table (they are compiled out of the pinned oracle build,
/// plan §0.2 — so registering them is a deliberate, recorded departure and they
/// are intercepted here rather than added to the oracle-pinned option table /
/// `Dump commands` golden). Returns `true` when `param_name` names an AD option
/// (so the caller suppresses the "Unknown parameter" error), even when the
/// option is deliberately refused (`ADiakoptics` itself — WP-AD.3).
///
/// Spec: official `ExecOptions.pas` — `Num_SubCircuits` (120), `ADiakoptics`
/// (122), `LinkBranches` set (124, setter at 842–849), `Coverage`,
/// `UseMyLinkBranches` (134).
pub(crate) fn try_set_ad_option(
    ckt: &mut Circuit,
    param_name: &str,
    param: &str,
    errors: &mut crate::diag::ErrorLog,
) -> bool {
    match param_name.to_ascii_lowercase().as_str() {
        "num_subcircuits" => {
            // Pascal `InterpretInt` (rounds a real value); accept a plain int.
            match param.trim().parse::<f64>() {
                Ok(v) => ckt.ad.num_sub_ckts = v.round() as i32,
                Err(_) => errors.push(format!("Invalid Num_SubCircuits value \"{param}\"")),
            }
            true
        }
        "coverage" => {
            match param.trim().parse::<f64>() {
                Ok(v) => ckt.ad.coverage = v,
                Err(_) => errors.push(format!("Invalid Coverage value \"{param}\"")),
            }
            true
        }
        "linkbranches" => {
            // Official ExecOptions.pas:842–844: `setlength(Link_Branches,
            // myList.Count + 1); for i := 1 to myList.Count do Link_Branches[i]
            // := myList[i-1]`. Index 0 is an empty **reference placeholder**;
            // the user's cuts occupy 1..=Count. Both `Tear_Circuit` branches
            // skip index 0, and `Num_pieces`/`Result` are driven off the full
            // length — so the placeholder is what makes N user links yield N+1
            // sub-circuits (empirically confirmed vs r3723: `[line.main10]` →
            // "Sub-Circuits Created: 2"; `[l5, l10]` → 3).
            //
            // NOTE(upstream): the `myList.Count <= CPU_Cores-3` guard (error
            // 7009) is deliberately NOT ported — it makes the setter
            // core-count-dependent, which plan D6 forbids for reproducible AD
            // tests. The placeholder is the count-affecting behavior.
            let items = parse_element_list(param);
            let mut lb = Vec::with_capacity(items.len() + 1);
            lb.push(String::new());
            lb.extend(items);
            ckt.ad.link_branches = lb;
            true
        }
        "usemylinkbranches" => {
            ckt.ad.use_user_links = interpret_yes_no(param);
            true
        }
        "adiakoptics" => {
            // Official ExecOptions.pas:1100–1112: set true → `ADiakopticsInit`
            // (deferred to `do_set_cmd`, which has `&mut Dss`); clear → flag only.
            let _ = errors;
            if interpret_yes_no(param) {
                ckt.ad.pending_ad_init = true;
            } else {
                ckt.solution.adiakoptics = false; // clear = flag only (plan §WP-AD.3)
            }
            true
        }
        _ => false,
    }
}

/// Handle `get` of an A-Diakoptics option (companion to [`try_set_ad_option`]).
/// Appends the value to `result` (comma-space separated, the executive `Get`
/// convention) and returns `true` when `param_name` names an AD option.
pub(crate) fn try_get_ad_option(ckt: &Circuit, param_name: &str, result: &mut String) -> bool {
    match param_name.to_ascii_lowercase().as_str() {
        "num_subcircuits" => {
            super::helpers::append_result(result, &ckt.ad.num_sub_ckts.to_string());
            true
        }
        "coverage" => {
            // Official ExecOptions.pas:1086: `get Coverage` reports
            // `Actual_Coverage` (the achieved coverage), `Format('%-g')`, NOT the
            // requested `Coverage`. It stays -1 (the ctor sentinel) until
            // `Refine_BusLevels`/`Get_paths_4_Coverage` (WP-AD.5) has run.
            super::helpers::append_result(result, &crate::util::fmt_g(ckt.ad.actual_coverage, 15));
            true
        }
        "linkbranches" => {
            // Official ExecOptions.pas:1093–1095: `for i := 0 to
            // High(Link_Branches) do AppendGlobalResult(Link_Branches[i])`.
            // `AppendGlobalResult('')` on an empty result leaves it empty
            // (DSSGlobals.pas:798–802), so the index-0 placeholder vanishes and
            // the output is the non-empty cuts, comma-space separated, with no
            // brackets — matching r3723 (`get LinkBranches` → `line.main10`).
            for link in &ckt.ad.link_branches {
                super::helpers::append_result(result, link);
            }
            true
        }
        "usemylinkbranches" => {
            super::helpers::append_result(result, if ckt.ad.use_user_links { "Yes" } else { "No" });
            true
        }
        "adiakoptics" => {
            super::helpers::append_result(
                result,
                if ckt.solution.adiakoptics {
                    "Yes"
                } else {
                    "No"
                },
            );
            true
        }
        _ => false,
    }
}

impl Dss {
    /// Pascal command `Tear_Circuit` (ExecCommand[111]) → `ADiakoptics_Tearing(
    /// AddISrc=False)` (Diakoptics.pas:506): tear the circuit into sub-circuits
    /// without A-Diakoptics ISources, then emit the on-disk `Torn_Circuit/`
    /// project tree. Sets `GlobalResult` to `"Sub-Circuits Created: N"` on
    /// success (Diakoptics.pas:526), the error string otherwise.
    ///
    /// The full official orchestration (`ADiakoptics_Tearing`,
    /// Diakoptics.pas:511–534): `Tear_Circuit` (partition + zone `EnergyMeter`
    /// placement + `PConn` capture), then `SolutionMode := 0`/`set
    /// controlmode=off`/`BuildYMatrix`, then — when the solution did not abort —
    /// `Save_SubCircuits(AddISrc=False)` (the file emission). The `SolutionMode`
    /// toggle is restored immediately (net no-op here); we issue `set
    /// controlmode=off` and rebuild the meter zones (the `BuildYMatrix`
    /// `ReprocessBusDefs` tail — needed so `SaveFeeders` sees the new zones).
    pub(super) fn do_tear_circuit_cmd(&mut self) {
        let n = match self.tear_circuit() {
            Ok(n) => n,
            Err(_) => {
                self.errors
                    .push("MeTIS cannot process the graph file (tearing failed).".to_string());
                self.last_result = "There was an error when tearing the circuit ".to_string();
                return;
            }
        };

        // Diakoptics.pas:517–519 — snapshot mode + controls off + rebuild Y.
        // The mode toggle (`Prev_mode` → 0 → `Prev_mode`) is a net no-op for the
        // file emission, so we only issue the `set controlmode=off` and force the
        // meter-zone rebuild that `BuildYMatrix` performs, so the zones just
        // created by `place_zone_meters` are current before `SaveFeeders`.
        self.command("set controlmode=off");
        self.reset_meter_zones_for_tear();

        // Diakoptics.pas:521–527 — `if not SolutionAbort then Save_SubCircuits`.
        let aborted = self
            .circuit
            .as_ref()
            .is_some_and(|c| c.solution.solution_abort);
        if aborted {
            self.last_result = "There was an error when tearing the circuit ".to_string();
            return;
        }
        self.save_sub_circuits(false);

        if let Some(ckt) = self.circuit.as_mut() {
            ckt.ad.num_sub_ckts = n;
        }
        self.last_result = format!("Sub-Circuits Created: {n}");
    }

    /// Force the meter-zone rebuild that `BuildYMatrix`'s `ReprocessBusDefs` tail
    /// runs (Ymatrix.pas → Circuit.pas:2246). Adding the `Zone_i` meters does not
    /// redefine buses, so the automatic `bus_name_redefined` path in
    /// `build_y_matrix` would not fire; we call `do_reset_meter_zones` directly
    /// (as `exec/reduce.rs` does) so `SaveFeeders` sees each new meter's zone.
    pub(super) fn reset_meter_zones_for_tear(&mut self) {
        let Dss {
            classes, circuit, ..
        } = self;
        let Some(ckt) = circuit.as_mut() else {
            return;
        };
        let mut store = crate::exec::registry::ClassStore { classes };
        crate::solution::meters::do_reset_meter_zones(ckt, &mut store);
    }

    /// The tearing dispatch (`Tear_Circuit`, Circuit.pas:1880): the manual
    /// link-branch branch when `UseMyLinkBranches` is set with a non-empty list,
    /// else the automatic `dss-metis` partition branch. Both branches compute
    /// `Locations`, then the shared [`Self::place_zone_meters`] runs the meter
    /// placement + `PConn` capture loop (Circuit.pas:1941–2032) and returns the
    /// sub-circuit count.
    pub(super) fn tear_circuit(&mut self) -> Result<i32, TearError> {
        let use_user = self
            .circuit
            .as_ref()
            .is_some_and(|c| c.ad.use_user_links && !c.ad.link_branches.is_empty());
        if use_user {
            self.tear_circuit_manual()?;
        } else {
            self.tear_circuit_auto()?;
        }
        self.place_zone_meters()
    }

    /// Automatic branch: `Create_MeTIS_graph` + `Create_MeTIS_Zones`, filling
    /// `Locations`/`BusZones` (Circuit.pas:1932–1934). `Link_Branches` is derived
    /// in the shared meter loop.
    fn tear_circuit_auto(&mut self) -> Result<i32, TearError> {
        // Calc_Inc_Matrix_Org (disjoint field borrows of self).
        crate::solution::inc_matrix::calc_inc_matrix_org(
            &mut self.classes,
            self.circuit.as_mut().ok_or(TearError::NoGraph)?,
        );

        let num_pieces = self
            .circuit
            .as_ref()
            .map(|c| c.ad.num_sub_ckts)
            .ok_or(TearError::NoGraph)?;
        let graph = {
            let ckt = self.circuit.as_ref().ok_or(TearError::NoGraph)?;
            build_metis_graph(&self.classes, &ckt.solution.inc_matrix)
        };
        let graph_path = {
            let ckt = self.circuit.as_ref().ok_or(TearError::NoGraph)?;
            graph_file_path(&self.output_directory, &ckt.case_name)
        };
        let _ = graph.write_opendss_graph(&graph_path);

        let ckt = self.circuit.as_mut().ok_or(TearError::NoGraph)?;
        create_metis_zones(&graph, num_pieces, &graph_path, &mut ckt.ad)?;
        Ok(0)
    }

    /// Manual branch (official Circuit.pas:1922–1928): `Locations[0] := 0`;
    /// `Locations[i] := get_PDE_Bus1_Location(Link_Branches[i])`. The
    /// user-supplied `Link_Branches` are kept (the shared meter loop overwrites
    /// each entry with the incidence-derived PDE name, 1:1 with Pascal).
    fn tear_circuit_manual(&mut self) -> Result<i32, TearError> {
        crate::solution::inc_matrix::calc_inc_matrix_org(
            &mut self.classes,
            self.circuit.as_mut().ok_or(TearError::NoGraph)?,
        );
        let link_branches = self
            .circuit
            .as_ref()
            .map(|c| c.ad.link_branches.clone())
            .ok_or(TearError::NoGraph)?;
        let mut locations = vec![0i32; link_branches.len()];
        for i in 1..link_branches.len() {
            locations[i] = self.get_pde_bus1_location(&link_branches[i]);
        }
        // `UseUserLinks := False` after consuming it (Circuit.pas:1934).
        let ckt = self.circuit.as_mut().ok_or(TearError::NoGraph)?;
        ckt.ad.locations = locations;
        ckt.ad.use_user_links = false;
        Ok(0)
    }

    /// Pascal `Tear_Circuit`'s meter-placement + `PConn` capture loop
    /// (Circuit.pas:1941–2032), shared by both tear branches. Requires a prior
    /// successful solve (the loop reads `Solution.NodeV` at each point of
    /// connection — the ckt24 header + spec both require a base solve before
    /// tearing); errors honestly when `SolutionCount = 0` (never solved).
    ///
    /// Per location: derive the link PDE from `Inc_Mat_Rows[get_IncMatrix_Row]`,
    /// the point-of-connection bus (`get_line_bus(link, 2)` — bus 2 of the link
    /// line, dot-stripped), the three-phase `PConn_Voltages`
    /// (`ctopolardeg(NodeV)` → mag/1000, angle°), then `New EnergyMeter.Zone_<i+1>
    /// element=<PDE> terminal=1 option=R action=C`. All pre-existing meters are
    /// disabled first (Circuit.pas:1941–1946). Returns the sub-circuit count
    /// `length(Locations)` (Result starts at 1 and `inc`s per i>0).
    ///
    /// NOTE(upstream-quirk): r3723 also computes `Term_volts[0] - Term_volts[1]`
    /// (a |V| difference across the branch, Circuit.pas:1967–1984) but never
    /// reads the result — the meter terminal is hard-coded to 1 and the PConn bus
    /// is always the link line's bus 2. The vestigial |V| read is a defined,
    /// side-effect-free dead computation, so it is not reproduced (plan D5:
    /// allocated-but-never-read scaffolding stays absent).
    fn place_zone_meters(&mut self) -> Result<i32, TearError> {
        // Prior-solve gate: the loop reads `Solution.NodeV` at each point of
        // connection (Circuit.pas reads it blindly), but a torn circuit whose
        // power flow never converged carries only the seeded source voltages, not
        // a real operating point — error honestly rather than emit meaningless
        // zone sources. `converged_flag` (not `solution_count`, which `compile`'s
        // `calcv` already bumps to 1) is the "a power flow converged" indicator.
        let solved = self
            .circuit
            .as_ref()
            .is_some_and(|c| c.solution.converged_flag);
        if !solved {
            self.errors.push(
                "Tear_Circuit requires a prior successful solve (the zone \
                 point-of-connection voltages are read from the solved NodeV)."
                    .to_string(),
            );
            return Err(TearError::NoGraph);
        }

        let locations = self
            .circuit
            .as_ref()
            .map(|c| c.ad.locations.clone())
            .ok_or(TearError::NoGraph)?;
        let n = locations.len();

        // Allocate the PConn/Link storage (Circuit.pas:1949–1951).
        let mut link_branches = vec![String::new(); n];
        let mut pconn_names = vec![String::new(); n];
        let mut pconn_voltages: Vec<f64> = Vec::with_capacity(n * 6);

        // Meter commands to issue after releasing the circuit borrow.
        let mut meter_cmds: Vec<String> = Vec::new();
        // Deferred `get_Line_Bus` "Line not found" errors (Circuit.pas:1198,
        // 5008) — collected here and flushed after the loop so the honest error
        // surfaces for a non-Line link without a mid-loop `&mut self` borrow.
        let mut line_errors: Vec<crate::diag::DssDiagnostic> = Vec::new();

        for (i, &loc) in locations.iter().enumerate() {
            if i == 0 {
                // Reference bus (Actor 1): `Inc_Mat_Cols[0]` (Circuit.pas:2014).
                let bus_name = self
                    .circuit
                    .as_ref()
                    .and_then(|c| c.solution.inc_matrix.cols.first().cloned())
                    .unwrap_or_default();
                pconn_names[0] = bus_name.clone();
                self.push_pconn_phases(&bus_name, &mut pconn_voltages);
                continue;
            }

            // Link PDE = `Inc_Mat_Rows[get_IncMatrix_Row(Locations[i])]`
            // (Circuit.pas:1962–1963).
            let pde = {
                let ckt = self.circuit.as_ref().ok_or(TearError::NoGraph)?;
                match ckt.solution.inc_matrix.inc_mat.as_ref() {
                    Some(inc) => {
                        let row = get_inc_matrix_row(inc, loc);
                        usize::try_from(row)
                            .ok()
                            .and_then(|r| ckt.solution.inc_matrix.rows.get(r))
                            .cloned()
                            .unwrap_or_default()
                    }
                    None => String::new(),
                }
            };
            link_branches[i] = pde.clone();

            // Point of connection = bus 2 of the link **line**, dot-stripped
            // (Circuit.pas:1985–1989: `BusName := get_line_bus(link.Substring(dot),
            // 2)` then strip the dot). `get_Line_Bus` searches ONLY the Lines list
            // (Circuit.pas:1167–1208): a non-Line link (e.g. a Transformer) is not
            // found → error 5008 and no point-of-connection bus. Reproduced: the
            // honest "Line not found" surfaces here (the ZLL 3-phase-Line cut
            // constraint is otherwise enforced downstream at AD init, D5).
            //
            // NOTE: on the not-found path official `get_Line_Bus` falls through to
            // `Result := ActiveCktElement.GetBus(NBus)` of the *restored*
            // previously-active element (Circuit.pas:1204–1206) — a stale,
            // wrong-but-non-empty bus name that depends on prior traversal state.
            // Per D5 that state-dependent read is NOT reproduced; the port yields
            // an empty point of connection (and the 5008 error) instead.
            let bare = pde.split_once('.').map(|(_, n)| n).unwrap_or(pde.as_str());
            let raw_bus = match line_bus(&self.classes, bare, 2) {
                Some(b) => b,
                None => {
                    line_errors.push(crate::diag::DssDiagnostic::msg(
                        format!("Line \"{bare}\" Not Found in Active Circuit."),
                        Some(5008),
                    ));
                    String::new()
                }
            };
            let bus_name = raw_bus.split('.').next().unwrap_or(&raw_bus).to_string();
            pconn_names[i] = bus_name.clone();
            self.push_pconn_phases(&bus_name, &mut pconn_voltages);

            // `New EnergyMeter.Zone_<i+1> element=<PDE> terminal=1 option=R
            // action=C` (Circuit.pas:2009).
            meter_cmds.push(format!(
                "New EnergyMeter.Zone_{} element={} terminal=1 option=R action=C",
                i + 1,
                pde
            ));
        }

        // Write the captured arrays back (Circuit.pas fills them in place).
        if let Some(ckt) = self.circuit.as_mut() {
            ckt.ad.link_branches = link_branches;
            ckt.ad.pconn_names = pconn_names;
            ckt.ad.pconn_voltages = pconn_voltages;
            ckt.solution.solution_abort = false; // Circuit.pas:1952
        }

        // Flush the deferred `get_Line_Bus` errors (non-Line links).
        self.errors.extend(line_errors);

        // Disable every pre-existing EnergyMeter (Circuit.pas:1941–1946), then
        // create the zone meters through the executive edit path.
        self.disable_all_energy_meters();
        for cmd in &meter_cmds {
            self.command(cmd);
        }

        Ok(n as i32)
    }

    /// Read a bus's three-phase point-of-connection voltages and append them to
    /// `out` as `(|V|/1000, angle°)` pairs (Circuit.pas:1996–2005/2019–2028:
    /// `for jj := 1 to 3: ctopolardeg(NodeV[GetRef(FindIdx(jj))])`). A missing
    /// phase node falls back to `NodeV[0]` (ground = 0), matching Pascal's
    /// `GetRef(0)` on a `FindIdx` miss.
    fn push_pconn_phases(&self, bus_name: &str, out: &mut Vec<f64>) {
        use crate::support::complexutil::c_to_polar_deg;
        let Some(ckt) = self.circuit.as_ref() else {
            for _ in 0..6 {
                out.push(0.0);
            }
            return;
        };
        let bus = ckt.bus_list.find(bus_name).map(|idx| &ckt.buses[idx]);
        for phase in 1..=3 {
            let noderef = bus
                .and_then(|b| b.find_idx(phase).map(|ni| b.get_ref(ni)))
                .unwrap_or(0);
            let v = ckt
                .solution
                .node_v
                .get(noderef)
                .copied()
                .unwrap_or_default();
            let polar = c_to_polar_deg(v);
            out.push(polar.mag / 1000.0);
            out.push(polar.ang);
        }
    }

    /// Disable every EnergyMeter (Circuit.pas:1941–1946: `EMeter.Enabled :=
    /// False`). Direct field mutation (no bus redefinition), like the Pascal.
    fn disable_all_energy_meters(&mut self) {
        let meters = match self.circuit.as_ref() {
            Some(c) => c.energy_meters.clone(),
            None => return,
        };
        for r in meters {
            if let Some(ce) = self.classes[r.cls].arena[r.idx].as_ckt_element_mut() {
                ce.cd_mut().set_enabled(false);
            }
        }
    }

    /// Pascal `get_PDE_Bus1_Location` (Solution.pas:1707): the incidence column
    /// index of the PDE's **bus 2** (dot-suffix stripped). Returns `Inc_Mat_Cols`
    /// length (the loop's terminal `i`) when the bus is not found — matching the
    /// Pascal `for`-fallthrough (an out-of-range index the caller never
    /// dereferences here).
    fn get_pde_bus1_location(&self, pde: &str) -> i32 {
        let Some(ckt) = self.circuit.as_ref() else {
            return 0;
        };
        let bus2 = match pde_bus2_name(&self.classes, pde) {
            Some(b) => b,
            None => return ckt.solution.inc_matrix.cols.len() as i32,
        };
        let cols = &ckt.solution.inc_matrix.cols;
        for (i, c) in cols.iter().enumerate() {
            if c.eq_ignore_ascii_case(&bus2) {
                return i as i32;
            }
        }
        cols.len() as i32
    }
}

/// The dot-stripped **bus 2** name of a PDE by `Class.Name`
/// (`SetElementActive(myPDE); ActiveCktElement.GetBus(2)`), or `None` when the
/// element cannot be resolved. Used by `get_PDE_Bus1_Location` (the Pascal name
/// says bus 1 but reads bus 2 — Solution.pas:1707).
fn pde_bus2_name(classes: &[DssClass], full_name: &str) -> Option<String> {
    let lower = full_name.to_ascii_lowercase();
    let (cls_name, obj_name) = match lower.split_once('.') {
        Some((c, n)) => (Some(c), n),
        None => (None, lower.as_str()),
    };
    for class in classes {
        if class.kind.is_none() {
            continue;
        }
        if let Some(cn) = cls_name
            && !class.props.class_name().eq_ignore_ascii_case(cn)
        {
            continue;
        }
        if let Some(&oi) = class.name_to_idx.get(obj_name) {
            let bus = class.arena[oi].as_ckt_element()?.cd().get_bus(2);
            let stripped = bus.split('.').next().unwrap_or(bus).to_string();
            return Some(stripped);
        }
    }
    None
}

/// Pascal `TDSSCircuit.get_Line_Bus(LName, NBus)` (Circuit.pas:1167): the bus
/// name at terminal `nbus` (1-based) of the **Line** named `lname`. Searches only
/// the `Line` class (like the Pascal `WITH ActiveCircuit.Lines DO` loop); returns
/// `None` when no Line by that name exists — the caller then surfaces the
/// error-5008 "Line not found" honestly. The returned bus keeps its node dots
/// (the caller strips them, Circuit.pas:1987–1989).
fn line_bus(classes: &[DssClass], lname: &str, nbus: usize) -> Option<String> {
    let key = lname.to_ascii_lowercase();
    for class in classes {
        if class.kind.is_none() {
            continue;
        }
        if !class.props.class_name().eq_ignore_ascii_case("line") {
            continue;
        }
        if let Some(&oi) = class.name_to_idx.get(&key) {
            let bus = class.arena[oi].as_ckt_element()?.cd().get_bus(nbus);
            return Some(bus.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_prefix_splits_on_dot() {
        assert_eq!(class_prefix("Line.l1"), "Line");
        assert_eq!(class_prefix("Transformer.t1"), "Transformer");
        assert_eq!(class_prefix("noname"), "noname");
    }

    #[test]
    fn other_terminal_uses_consecutive_pair() {
        // Two PDEs: rows 0 (cols 0,1) and 1 (cols 1,2), stored [r,c,v].
        let data = [[0, 0, 1], [0, 1, -1], [1, 1, 1], [1, 2, -1]];
        // Entry 0 is in col 0; its pair is entry 1 (same row 0) => other col 1.
        assert_eq!(other_terminal_col(&data, 0), 1);
        // Entry 1's next row differs => pair is jj-1 (entry 0) => col 0.
        assert_eq!(other_terminal_col(&data, 1), 0);
        // Entry 3 is last => pair is jj-1 (entry 2) => col 1.
        assert_eq!(other_terminal_col(&data, 3), 1);
    }

    #[test]
    fn zones_split_a_path_into_two() {
        // A 6-vertex path; a contiguous 2-way partition yields two zones. The
        // reference zone is Locations[0] (=> 1 after the +1), plus one boundary.
        let path6 = MetisGraph {
            n_cols: 6,
            num_edges: 5,
            adjacency: vec![
                vec![(1, 3)],
                vec![(0, 3), (2, 3)],
                vec![(1, 3), (3, 3)],
                vec![(2, 3), (4, 3)],
                vec![(3, 3), (5, 3)],
                vec![(4, 3)],
            ],
        };
        let mut ad = AdTearing::new();
        let gp = std::env::temp_dir().join(format!("dss_ad_zones_{}_.graph", std::process::id()));
        create_metis_zones(&path6, 2, &gp, &mut ad).unwrap();
        let _ = std::fs::remove_file(crate::support::partition::part_file_path(&gp, 2));

        assert_eq!(ad.metis_zones.len(), 6);
        assert_eq!(ad.locations[0], 1); // reference location, +1 adjusted
        assert_eq!(ad.locations.len(), ad.bus_zones.len());
        // A clean 3-3 contiguous split records exactly one extra zone boundary.
        assert_eq!(ad.locations.len(), 2, "zones/locations: {:?}", ad.locations);
    }
}
