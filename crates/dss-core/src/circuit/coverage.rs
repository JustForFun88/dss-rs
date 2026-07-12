//! A-Diakoptics coverage-path tracing (`DIAKOPTICS_PSTCALC_PLAN.md` WP-AD.5).
//!
//! Behavioral spec = **official r3723 Delphi** (plan D10):
//! `.inputs/electricdss-code-r3723-trunk/Version8/Source/Common/Circuit.pas`
//! `get_longest_path` (778), `Append2PathsArray` (809), `Normalize_graph`
//! (823), `Get_paths_4_Coverage` (853). These trace the longest paths from the
//! feeder backbone (the level-0 buses of the hierarchical incidence matrix)
//! outward until the requested `Coverage` fraction of buses is reached — the
//! body of the `Refine_BusLevels` command.
//!
//! The routines read/mutate `Solution.Inc_Mat_Levels` (the per-bus proximity
//! level, populated by `Calc_Inc_Matrix_Org`, WP-AD.1) and the circuit's
//! `Longest_paths`/`Path_Idx`/`Buses_Covered`/`New_Graph` working arrays
//! (`AdTearing`), and use `Coverage`/`Actual_Coverage`.

use crate::circuit::Circuit;

/// Pascal `get_element_idx(graph_in, element)` (Circuit.pas:753): the first index
/// at which `levels[i] == element`, else `None` (Pascal leaves `Result`
/// unassigned on a miss — the callers here always pass a present value).
fn get_element_idx(levels: &[i32], element: i32) -> Option<usize> {
    levels.iter().position(|&v| v == element)
}

impl Circuit {
    /// Pascal `TDSSCircuit.get_longest_path` (Circuit.pas:778): trace the longest
    /// path within the linearized graph, treating the zero-level buses as the
    /// start of a new path. Fills `New_Graph` with the bus (column) indices whose
    /// level steps down monotonically from the current maximum, one per level.
    ///
    /// Walks the level array from the highest-level bus downward by index; each
    /// time the current bus's level equals the running target level it is added to
    /// the backbone and the target decremented. Terminates when the running index
    /// reaches a level-0 bus or a bus whose level exceeds the running target.
    fn get_longest_path(&mut self) {
        let levels = &self.solution.inc_matrix.levels;
        self.ad.new_graph.clear();
        // Empty graph: nothing to trace (guards MaxIntValue on []).
        let Some(&max_level) = levels.iter().max() else {
            return;
        };
        let mut current_level = max_level;
        // `get_element_idx(levels, Current_level)`: the max value exists, so the
        // index is always Some.
        let Some(mut current_idx) = get_element_idx(levels, current_level) else {
            return;
        };
        let mut end_flag = true;
        while end_flag {
            // NOTE(upstream-quirk): Pascal reads `Inc_Mat_Levels[Current_idx]`
            // unconditionally at the loop head; `Current_idx` decrements each
            // iteration and could pass below 0. In a well-formed hierarchical
            // graph the descent hits a level-0 backbone bus first (which sets
            // `end_flag = False`), so index 0 is the floor — but per plan D5 we do
            // not reproduce a potential negative-index OOB read: a run past the
            // start terminates the trace.
            let lv = self.solution.inc_matrix.levels[current_idx];
            // Termination criteria (Circuit.pas:793–794).
            if current_level > lv || lv == 0 {
                end_flag = false;
            }
            // Is the current bus part of the new backbone? (Circuit.pas:796–801)
            if lv == current_level {
                current_level -= 1;
                self.ad.new_graph.push(current_idx as i32);
            }
            if current_idx == 0 {
                break;
            }
            current_idx -= 1;
        }
    }

    /// Pascal `TDSSCircuit.Append2PathsArray(New_Path)` (Circuit.pas:809): append
    /// every element of `new_path` to `Longest_paths` and return the offset at
    /// which the appended run begins (`High(Longest_paths)+1` before the append).
    fn append_to_paths_array(&mut self, new_path: &[i32]) -> i32 {
        let result = self.ad.longest_paths.len() as i32;
        self.ad.longest_paths.extend_from_slice(new_path);
        result
    }

    /// Pascal `TDSSCircuit.Normalize_graph` (Circuit.pas:823): renumber the level
    /// array so each contiguous non-zero segment between zero-level (backbone)
    /// buses restarts at level 1 and counts up. `Curr_level` tracks the offset to
    /// subtract; a zero level or a level that did not strictly increase begins a
    /// new segment.
    fn normalize_graph(&mut self) {
        let levels = &mut self.solution.inc_matrix.levels;
        let mut curr_level: i32 = -1;
        let mut ref_detected = false;
        for lv in levels.iter_mut() {
            if *lv == 0 {
                ref_detected = true;
            } else if curr_level >= *lv || ref_detected {
                ref_detected = false;
                curr_level = *lv - 1;
                *lv = 1;
            } else {
                *lv -= curr_level;
            }
        }
    }

    /// Pascal `TDSSCircuit.Get_paths_4_Coverage` (Circuit.pas:853): trace paths
    /// from the backbone until the summed estimated bus coverage reaches the
    /// requested `Coverage`. The body of the `Refine_BusLevels` command
    /// (ExecCommands.pas:691). Fills `Path_Idx`/`Buses_Covered`/`Longest_paths`
    /// and sets `Actual_Coverage`.
    ///
    /// State machine: state 0 seeds the first path from all level-0 (backbone)
    /// buses; state 1 repeatedly extracts the next longest branch, zeroes it into
    /// the backbone, and renormalizes. It stops when a new path both changes the
    /// coverage and meets/exceeds `Coverage` (the "different-and-sufficient"
    /// criterion at Circuit.pas:909).
    pub fn get_paths_4_coverage(&mut self) {
        let sys_size = self.solution.inc_matrix.cols.len() as f64;
        // Empty incidence matrix (no `Calc_Inc_Matrix_Org` yet): nothing to trace.
        // Pascal would `MaxIntValue([])` — we terminate cleanly with 0 new paths.
        if self.solution.inc_matrix.levels.is_empty() || sys_size == 0.0 {
            self.ad.buses_covered = vec![0];
            self.ad.path_idx = vec![0];
            self.ad.actual_coverage = -1.0;
            return;
        }

        self.ad.buses_covered = vec![0];
        self.ad.path_idx = vec![0];
        self.ad.actual_coverage = -1.0;

        let mut state = 0;
        loop {
            match state {
                0 => {
                    // Extract the 0-level (backbone) buses (Circuit.pas:874–882).
                    let candidates: Vec<i32> = self
                        .solution
                        .inc_matrix
                        .levels
                        .iter()
                        .enumerate()
                        .filter(|&(_, &lv)| lv == 0)
                        .map(|(i, _)| i as i32)
                        .collect();
                    self.ad.longest_paths.clear();
                    // `Buses_covered[0] := MaxIntValue(Candidates)` — the largest
                    // backbone-bus INDEX (verbatim; not a level). Empty backbone =>
                    // 0 (guards MaxIntValue on []).
                    self.ad.buses_covered[0] = candidates.iter().copied().max().unwrap_or(0);
                    self.ad.path_idx[0] = self.append_to_paths_array(&candidates);
                    state = 1;
                }
                _ => {
                    // Extract a new path from the longest branch to the backbone
                    // (Circuit.pas:889–898).
                    self.get_longest_path();
                    let new_graph = self.ad.new_graph.clone();
                    let start = self.append_to_paths_array(&new_graph);
                    self.ad.path_idx.push(start);
                    // Estimated buses covered by this path = first index - last
                    // index of the extracted branch (Circuit.pas:894). Empty branch
                    // => 0 (guards the [0]/[High] OOB read; plan D5).
                    let covered = match (new_graph.first(), new_graph.last()) {
                        (Some(&f), Some(&l)) => f - l,
                        _ => 0,
                    };
                    self.ad.buses_covered.push(covered);
                    // Zero the just-traced path into the backbone
                    // (Circuit.pas:896–897).
                    let start_u = start.max(0) as usize;
                    let lp = self.ad.longest_paths.clone();
                    for &bus in lp.iter().skip(start_u) {
                        if let Some(slot) = self.solution.inc_matrix.levels.get_mut(bus as usize) {
                            *slot = 0;
                        }
                    }
                    self.normalize_graph();
                    // remains in state 1
                }
            }

            // Coverage check (Circuit.pas:903–910).
            let dbl_temp = self.ad.buses_covered.iter().map(|&b| b as f64).sum::<f64>() / sys_size;
            // Stop only when the new coverage BOTH differs from the previous value
            // (a path still contributing) AND meets/exceeds the requested Coverage.
            let stop = dbl_temp != self.ad.actual_coverage && dbl_temp >= self.ad.coverage;
            self.ad.actual_coverage = dbl_temp;
            if stop {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::exec::Dss;

    /// A simple radial feeder, torn-free: build the hierarchical incidence matrix
    /// then refine bus levels. The command must un-refuse and report a path count.
    fn build_and_refine(deck: &str) -> (Dss, String) {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command(deck);
        dss.command("solve");
        dss.command("CalcIncMatrix_O");
        dss.command("Refine_BusLevels");
        let r = dss.result().to_string();
        (dss, r)
    }

    #[test]
    fn refine_bus_levels_reports_paths_on_radial() {
        let deck = "\
            new circuit.covtest basekv=12.47 phases=3 bus1=sourcebus\n\
            new line.l1 bus1=sourcebus bus2=b1 length=1 r1=0.1 x1=0.1\n\
            new line.l2 bus1=b1 bus2=b2 length=1 r1=0.1 x1=0.1\n\
            new line.l3 bus1=b2 bus2=b3 length=1 r1=0.1 x1=0.1\n\
            new line.l4 bus1=b1 bus2=b4 length=1 r1=0.1 x1=0.1\n\
            new line.l5 bus1=b4 bus2=b5 length=1 r1=0.1 x1=0.1\n\
            new load.ld1 bus1=b3 kv=12.47 kw=100\n\
            new load.ld2 bus1=b5 kv=12.47 kw=100\n";
        let (dss, r) = build_and_refine(deck);
        assert!(
            r.contains("new paths detected"),
            "Refine_BusLevels result: {r:?}"
        );
        // Actual_Coverage advanced off its -1 sentinel.
        let ac = dss.circuit().unwrap().ad.actual_coverage;
        assert!(ac >= 0.0, "actual_coverage should be set, got {ac}");
    }

    #[test]
    fn refine_bus_levels_without_inc_matrix_is_clean() {
        // No CalcIncMatrix_O: the command must not panic; 0 new paths.
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.c basekv=12.47 phases=3 bus1=sourcebus");
        dss.command("solve");
        dss.command("Refine_BusLevels");
        assert!(
            dss.result().contains("new paths detected"),
            "result: {:?}",
            dss.result()
        );
    }
}
