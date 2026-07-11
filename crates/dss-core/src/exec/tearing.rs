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
    let lower = full_name.to_lowercase();
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
            return class.objects[oi].as_ckt_element().map(|e| e.cd().nphases);
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
    errors: &mut Vec<String>,
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
            // NOT_PORTED (scoped): the A-Diakoptics init state machine is
            // WP-AD.3. `Tear_Circuit` and the tearing options are WP-AD.2.
            errors.push(
                "set ADiakoptics is not ported yet (A-Diakoptics init/solve is WP-AD.3)."
                    .to_string(),
            );
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
            // `get Coverage` after `tear_circuit` reports Actual_Coverage; before,
            // the requested Coverage (official help note). Actual_Coverage stays
            // -1 until the coverage-path algorithm (WP-AD.5) runs, so report the
            // requested value here.
            super::helpers::append_result(result, &format!("{}", ckt.ad.coverage));
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
        _ => false,
    }
}

impl Dss {
    /// Pascal command `Tear_Circuit` (ExecCommand[111]) → `ADiakoptics_Tearing(
    /// AddISrc=False)` (Diakoptics.pas:506): tear the circuit into sub-circuits
    /// without A-Diakoptics ISources. Sets `GlobalResult` to `"Sub-Circuits
    /// Created: N"` on success (Diakoptics.pas:526), the error string otherwise.
    ///
    /// NOTE: this WP-AD.2 Stage-B increment computes the partition, the zone
    /// `Locations`/`BusZones`, and the `Link_Branches`, and writes the
    /// `.graph`/`.part.N` artifacts. The per-zone `EnergyMeter` placement +
    /// `PConn_Voltages` capture (Circuit.pas:1954–2032) and `Save_SubCircuits`
    /// file emission (Format_SubCircuits) land in the follow-up (they need the
    /// prior-solve `NodeV` + `save circuit` integration); see STATUS §WP-AD.2.
    pub(super) fn do_tear_circuit_cmd(&mut self) {
        match self.tear_circuit() {
            Ok(n) => {
                if let Some(ckt) = self.circuit.as_mut() {
                    ckt.ad.num_sub_ckts = n;
                }
                self.last_result = format!("Sub-Circuits Created: {n}");
            }
            Err(_) => {
                self.errors
                    .push("MeTIS cannot process the graph file (tearing failed).".to_string());
                self.last_result = "There was an error when tearing the circuit ".to_string();
            }
        }
    }

    /// The tearing dispatch (`Tear_Circuit`, Circuit.pas:1880): the manual
    /// link-branch branch when `UseMyLinkBranches` is set with a non-empty list,
    /// else the automatic `dss-metis` partition branch.
    fn tear_circuit(&mut self) -> Result<i32, TearError> {
        let use_user = self
            .circuit
            .as_ref()
            .is_some_and(|c| c.ad.use_user_links && !c.ad.link_branches.is_empty());
        if use_user {
            self.tear_circuit_manual()
        } else {
            self.tear_circuit_auto()
        }
    }

    /// Automatic branch: `Create_MeTIS_graph` + `Create_MeTIS_Zones`, then
    /// `Link_Branches[i] := Inc_Mat_Rows[get_IncMatrix_Row(Locations[i])]`
    /// (Circuit.pas:1932–1933, 1962–1963).
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

        {
            let ckt = self.circuit.as_mut().ok_or(TearError::NoGraph)?;
            create_metis_zones(&graph, num_pieces, &graph_path, &mut ckt.ad)?;
        }

        // Link_Branches from Locations. Locations were `+1`-adjusted in
        // Create_MeTIS_Zones, and `get_IncMatrix_Row` is applied to that adjusted
        // value 1:1 (the upstream quirk-compensating offset).
        let ckt = self.circuit.as_mut().ok_or(TearError::NoGraph)?;
        let locations = ckt.ad.locations.clone();
        let mut link_branches = vec![String::new(); locations.len()];
        if let Some(inc) = ckt.solution.inc_matrix.inc_mat.as_ref() {
            let rows = &ckt.solution.inc_matrix.rows;
            for (i, &loc) in locations.iter().enumerate().skip(1) {
                let row = get_inc_matrix_row(inc, loc);
                // Safe guard (not reproducing an OOB): skip an unresolved row.
                if let Some(name) = usize::try_from(row).ok().and_then(|r| rows.get(r)) {
                    link_branches[i] = name.clone();
                }
            }
        }
        ckt.ad.link_branches = link_branches;
        Ok(locations.len() as i32)
    }

    /// Manual branch (official Circuit.pas:1922–1928): `Locations[0] := 0`;
    /// `Locations[i] := get_PDE_Bus1_Location(Link_Branches[i])`. The
    /// user-supplied `Link_Branches` are kept as the cut.
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
        Ok(link_branches.len() as i32)
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
    let lower = full_name.to_lowercase();
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
            let bus = class.objects[oi].as_ckt_element()?.cd().get_bus(2);
            let stripped = bus.split('.').next().unwrap_or(bus).to_string();
            return Some(stripped);
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
