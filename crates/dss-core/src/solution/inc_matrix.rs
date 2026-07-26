//! Branch-to-node incidence matrix + its Laplacian — a 1:1 port of the
//! incidence-matrix layer of Pascal `Common/Solution.pas` (fields 202–232,
//! procedures `Upload2IncMatrix` / `AddLines2IncMatrix` / `AddXfmr2IncMatrix` /
//! `AddSeriesCap2IncMatrix` / `AddSeriesReac2IncMatrix` / `Calc_Inc_Matrix` /
//! `Calc_Inc_Matrix_Org` / `get_IncMatrix_Row` / `get_IncMatrix_Col`,
//! lines 1340–1620).
//!
//! The incidence matrix `IncMat` (a [`SparseInt`]) has one **row per series PD
//! element** (Line, Transformer, series Capacitor, series Reactor) and one
//! **column per bus**; each row carries `+1` at its first terminal's bus and `-1`
//! at the others (`Upload2IncMatrix`). Two build orders exist:
//!
//! * [`calc_inc_matrix`] (Pascal `Calc_Inc_Matrix`) — the **flat** order: walk the
//!   circuit's Lines, then Transformers, then series Capacitors, then Reactors,
//!   assigning bus columns by linear search over `BusList`. Populates only
//!   `Inc_Mat_Rows` (element names); leaves `Inc_Mat_Cols`/`Inc_Mat_levels` alone.
//! * [`calc_inc_matrix_org`] (Pascal `Calc_Inc_Matrix_Org`) — the **hierarchical**
//!   order: walk the circuit topology tree (`GetTopology`) depth-first, discovering
//!   bus columns on first sight and recording each bus's proximity `level` to the
//!   feeder backbone, then normalizing the levels between zero-level buses.
//!
//! The Laplacian (`CalcLaplacian` command) is `IncMatᵀ · IncMat`
//! ([`SparseInt::transpose`] then [`SparseInt::multiply`]).
//!
//! This state lives on [`Solution`](crate::solution::solution::Solution) as
//! [`IncMatrixState`], mirroring the Pascal `TSolutionObj` fields. `IncMat`/
//! `Laplacian` are `Option` so their **NIL** state (never calculated) is faithful:
//! `CalcLaplacian` before any `CalcIncMatrix` raises upstream error 8877.

#[cfg(test)]
mod tests;

use crate::circuit::Circuit;
use crate::exec::registry::{ClassStore, DssClass};
use crate::solution::topology::get_topology;
use crate::support::sparse_math::SparseInt;

/// The incidence-matrix state hung off `Solution` (Pascal `TSolutionObj` fields
/// `IncMat`, `Laplacian`, `Inc_Mat_Rows`, `Inc_Mat_Cols`, `Inc_Mat_levels`, plus
/// `DSS.IncMat_Ordered`).
#[derive(Debug, Clone, Default)]
pub struct IncMatrixState {
    /// Pascal `IncMat` — `None` models the NIL object (never calculated).
    pub inc_mat: Option<SparseInt>,
    /// Pascal `Laplacian` — `None` models NIL.
    pub laplacian: Option<SparseInt>,
    /// Pascal `Inc_Mat_Rows`: the PD-element name per incidence-matrix row.
    pub rows: Vec<String>,
    /// Pascal `Inc_Mat_Cols`: the bus name per incidence-matrix column (only
    /// populated by the hierarchical `Calc_Inc_Matrix_Org`).
    pub cols: Vec<String>,
    /// Pascal `Inc_Mat_levels`: each bus's proximity level to the backbone (only
    /// populated by `Calc_Inc_Matrix_Org`).
    pub levels: Vec<i32>,
    /// Pascal `DSS.IncMat_Ordered`: whether the last build was hierarchical.
    pub ordered: bool,
}

/// Pascal `Copy(Bus, 0, ansipos('.', Bus) - 1)`: the bus name up to (not
/// including) the first `.` node-connection separator.
fn strip_bus(bus: &str) -> &str {
    match bus.find('.') {
        Some(i) => &bus[..i],
        None => bus,
    }
}

/// The running state the four `Add*2IncMatrix` procedures share (Pascal
/// `ActiveIncCell[0..2]`, `temp_counter`, and the `IncMat`/`Inc_Mat_Rows` they
/// build).
struct FlatBuilder<'a> {
    ckt: &'a Circuit,
    inc_mat: SparseInt,
    rows: Vec<String>,
    /// `ActiveIncCell`: `[0]` = 1-based row cursor, `[1]` = 1-based bus/col cursor,
    /// `[2]` = value to write (`+1` then `-1`).
    active: [i32; 3],
    temp_counter: i32,
}

impl FlatBuilder<'_> {
    /// Pascal `Upload2IncMatrix`: insert `(row-1, col-2, value)` then flip the
    /// pending value to `-1` (so terminal 1 writes `+1`, the rest `-1`).
    fn upload(&mut self) {
        self.inc_mat
            .insert(self.active[0] - 1, self.active[1] - 2, self.active[2]);
        self.active[2] = -1;
    }

    /// Pascal's inner bus-search loop: linear-search `BusList` for `bus`
    /// (1-based cursor `ActiveIncCell[1]`), then `Upload2IncMatrix`. On a miss the
    /// cursor ends at `NumBuses + 1` (col `NumBuses - 1`) — reproduced verbatim
    /// (every element bus exists by construction).
    fn find_col_and_upload(&mut self, bus: &str) {
        let num_buses = self.ckt.buses.len() as i32;
        self.active[1] = 1;
        let mut end_flag = true;
        while self.active[1] <= num_buses && end_flag {
            let idx0 = (self.active[1] - 1) as usize;
            if bus == self.ckt.bus_list.name(idx0).unwrap_or("") {
                end_flag = false;
            }
            self.active[1] += 1;
        }
        self.upload();
    }
}

/// Read a circuit element's `(enabled, nterms, full_name)` and terminal-bus
/// accessor via the class registry.
fn elem_info(classes: &[DssClass], r: crate::elements::traits::ElemId) -> (bool, usize, String) {
    let obj = &classes[r.class_ord()].arena[r.index()];
    let cd = classes[r.class_ord()]
        .arena
        .try_ckt_elem(r.index())
        .expect("incidence-matrix element list holds circuit elements")
        .cd();
    let full_name = format!(
        "{}.{}",
        classes[r.class_ord()].props.class_name(),
        obj.data().name()
    );
    (cd.enabled, cd.nterms, full_name)
}

/// Read terminal `term` (1-based) bus name of element `r`, stripped of its
/// node-connection suffix.
fn elem_bus_stripped(
    classes: &[DssClass],
    r: crate::elements::traits::ElemId,
    term: usize,
) -> String {
    let cd = classes[r.class_ord()]
        .arena
        .try_ckt_elem(r.index())
        .expect("incidence-matrix element list holds circuit elements")
        .cd();
    strip_bus(cd.get_bus(term)).to_string()
}

/// Pascal `AddLines2IncMatrix`: one row per enabled Line, `+1`/`-1` at its two
/// terminal buses.
fn add_lines(b: &mut FlatBuilder, classes: &[DssClass]) {
    for &r in &b.ckt.lines {
        let (enabled, _nterms, full_name) = elem_info(classes, r);
        if !enabled {
            continue;
        }
        b.active[2] = 1;
        b.temp_counter += 1;
        b.rows.push(full_name);
        for term in 1..=2 {
            let bus = elem_bus_stripped(classes, r, term);
            b.find_col_and_upload(&bus);
        }
        b.active[0] += 1;
    }
}

/// Pascal `AddXfmr2IncMatrix`: one row per enabled Transformer, over all windings.
fn add_xfmrs(b: &mut FlatBuilder, classes: &[DssClass]) {
    for &r in &b.ckt.transformers {
        let (enabled, nterms, full_name) = elem_info(classes, r);
        if !enabled {
            continue;
        }
        b.active[2] = 1;
        b.temp_counter += 1;
        b.rows.push(full_name);
        // Pascal `1 to elem.NumWindings` (== Nterms for a transformer).
        for term in 1..=nterms {
            let bus = elem_bus_stripped(classes, r, term);
            b.find_col_and_upload(&bus);
        }
        b.active[0] += 1;
    }
}

/// Pascal `AddSeriesCap2IncMatrix`: one row per enabled **series** Capacitor
/// (`NumTerminals > 1`); shunt capacitors are skipped and do NOT advance the row
/// cursor (the `continue` is before `inc(ActiveIncCell[0])`).
fn add_series_caps(b: &mut FlatBuilder, classes: &[DssClass]) {
    for &r in &b.ckt.shunt_capacitors {
        let (enabled, _nterms, full_name) = elem_info(classes, r);
        // Pascal `elem.NumTerminals` is the Capacitor `NumTerm` flag (2 only when
        // bus2 was explicitly defined = a series cap), NOT the generic Nterms.
        let num_terminals = classes[r.class_ord()]
            .arena
            .get::<crate::elements::pd::capacitor::Capacitor>(r.index())
            .expect("shunt_capacitors list holds Capacitor objects")
            .num_terminals();
        if num_terminals <= 1 || !enabled {
            continue;
        }
        b.temp_counter += 1;
        b.rows.push(full_name);
        b.active[2] = 1;
        for term in 1..=2 {
            let bus = elem_bus_stripped(classes, r, term);
            b.find_col_and_upload(&bus);
        }
        b.active[0] += 1;
    }
}

/// Pascal `AddSeriesReac2IncMatrix`: one row per **series** Reactor. A reactor is
/// series iff its bus-2 name has no `.0` ground-node token. NOTE(upstream-quirk):
/// the row cursor `ActiveIncCell[0]` is advanced for **every** reactor — even the
/// skipped shunt ones and disabled ones (the `inc` is outside the `if`, and there
/// is no `Enabled` test at all), unlike the Line/Xfmr/Cap walks. Reproduced.
fn add_series_reactors(b: &mut FlatBuilder, classes: &[DssClass]) {
    for &r in &b.ckt.reactors {
        let bus2 = {
            let cd = classes[r.class_ord()]
                .arena
                .try_ckt_elem(r.index())
                .expect("reactors list holds circuit elements")
                .cd();
            cd.get_bus(2).to_string()
        };
        // Pascal `ansipos('.0', RBus) = 0` → no ground-node token → series.
        if !bus2.contains(".0") {
            let (_enabled, _nterms, full_name) = elem_info(classes, r);
            b.temp_counter += 1;
            b.rows.push(full_name);
            b.active[2] = 1;
            for term in 1..=2 {
                let bus = elem_bus_stripped(classes, r, term);
                b.find_col_and_upload(&bus);
            }
        }
        b.active[0] += 1;
    }
}

/// Pascal `TSolutionObj.Calc_Inc_Matrix` (`Solution.pas:1507`): build the flat
/// branch-to-node incidence matrix (Lines, Transformers, series Caps, series
/// Reactors, in that order) into `ckt.solution.inc_matrix`. Populates `IncMat`
/// and `Inc_Mat_Rows`; leaves `Inc_Mat_Cols`/`Inc_Mat_levels` untouched
/// (`IncMat_Ordered := FALSE`).
pub(crate) fn calc_inc_matrix(classes: &[DssClass], ckt: &mut Circuit) {
    // Pascal resets IncMat content if the object already exists.
    let mut inc_mat = SparseInt::new();
    inc_mat.reset();
    let mut builder = FlatBuilder {
        ckt,
        inc_mat,
        rows: Vec::new(),
        active: [1, 0, 0], // ActiveIncCell[0] := 1
        temp_counter: 0,
    };
    add_lines(&mut builder, classes);
    add_xfmrs(&mut builder, classes);
    add_series_caps(&mut builder, classes);
    add_series_reactors(&mut builder, classes);

    let FlatBuilder { inc_mat, rows, .. } = builder;
    let st = &mut ckt.solution.inc_matrix;
    st.inc_mat = Some(inc_mat);
    st.rows = rows;
    st.ordered = false;
}

/// Pascal `get_IncMatrix_Row(Col)` (`Solution.pas:1530`): the row of the first
/// stored non-zero (scanning **from index 1**) whose column equals `col`, else
/// `-1`.
pub(crate) fn get_inc_matrix_row(inc_mat: &SparseInt, col: i32) -> i32 {
    let nzero = inc_mat.nzero();
    for idx in 1..nzero {
        let d = inc_mat.data[idx as usize];
        if d[1] == col {
            return d[0];
        }
    }
    -1
}

/// Pascal `get_IncMatrix_Col(Row)` (`Solution.pas:1549`): the column of the first
/// stored non-zero (from index 1) whose row equals `row`, also returning the
/// link-branch's two columns (`Active_Cols`). Returns `(result, [col0, col1])`.
///
/// NOTE(upstream-quirk): Pascal reads `data[idx+1]` (and `data[idx-1]`), which can
/// run one past the store; the OOB reads are guarded here (`.get`, default `-1`)
/// rather than reproduced (heap-read UB — CLAUDE.md known-bugs rule).
fn get_inc_matrix_col(inc_mat: &SparseInt, row: i32) -> (i32, [i32; 2]) {
    let nzero = inc_mat.nzero();
    for idx in 1..nzero {
        let d = inc_mat.data[idx as usize];
        if d[0] == row {
            let col0 = d[1];
            let col1 = inc_mat
                .data
                .get((idx + 1) as usize)
                .map(|x| x[1])
                .unwrap_or(-1);
            return (col0, [col0, col1]);
        }
    }
    (-1, [-1, -1])
}

/// Pascal `TSolutionObj.Calc_Inc_Matrix_Org` (`Solution.pas:1577`): build the
/// hierarchically-organized incidence matrix by walking the circuit topology tree,
/// discovering bus columns on first sight and recording each bus's proximity
/// `level`, then normalizing the level vector between zero-level buses. Populates
/// `IncMat`, `Inc_Mat_Rows`, `Inc_Mat_Cols`, `Inc_Mat_levels`
/// (`IncMat_Ordered := TRUE`).
pub(crate) fn calc_inc_matrix_org(classes: &mut [DssClass], ckt: &mut Circuit) {
    let mut inc_mat = SparseInt::new();
    inc_mat.reset();
    let mut cols: Vec<String> = Vec::new();
    let mut levels: Vec<i32> = Vec::new();
    let mut rows: Vec<String> = Vec::new();

    // Build the topology tree (mutates element/bus flags), then read from `classes`.
    let mut topo = {
        let mut store = ClassStore { classes };
        get_topology(ckt, &mut store)
    };

    // ActiveIncCell[0] := -1 (row cursor; the first PDE only seeds column 0).
    let mut active_row: i32 = -1;

    let mut pde = topo.first();
    while let Some(pde_ref) = pde {
        let n_levels = topo.level();
        let (_enabled, nterms, full_name) = elem_info(classes, pde_ref);
        // Buses of this PDE, node-suffix stripped.
        let pde_buses: Vec<String> = (1..=nterms)
            .map(|t| elem_bus_stripped(classes, pde_ref, t))
            .collect();

        if cols.is_empty() {
            // First iteration: seed the Cols/Levels arrays with bus 0.
            cols.push(pde_buses[0].clone());
            levels.push(n_levels);
        } else {
            // Pascal `inc(nPDE); Inc_Mat_Rows[nPDE-1] := PDE_Name` — the row-name
            // append (nPDE tracks the row array length, implicit in the push here).
            rows.push(full_name);
            for (j, pde_bus) in pde_buses.iter().enumerate() {
                let row = active_row;
                // Find or create the column for this bus.
                let mut found: i32 = -1;
                for (i, cb) in cols.iter().enumerate() {
                    if cb == pde_bus {
                        found = i as i32;
                    }
                }
                let col = if found >= 0 {
                    found
                } else {
                    cols.push(pde_bus.clone());
                    levels.push(n_levels);
                    (cols.len() - 1) as i32
                };
                let val = if j == 0 { 1 } else { -1 };
                inc_mat.insert(row, col, val);
            }
        }
        active_row += 1;
        pde = topo.go_forward();
    }

    normalize_levels(&inc_mat, &mut levels);

    let st = &mut ckt.solution.inc_matrix;
    st.inc_mat = Some(inc_mat);
    st.rows = rows;
    st.cols = cols;
    st.levels = levels;
    st.ordered = true;
}

/// Pascal `Calc_Inc_Matrix_Org`'s level-normalization tail (`Solution.pas:
/// 1671–1736`): collapse the continuous backbone path to level 0, then normalize
/// each between-zero-level branch subset to start at 1, then repair any trailing
/// tail via the incidence lookups.
///
/// NOTE(upstream-quirk): the first `for i := 0 to length(levels)` loop reads one
/// past the array (`levels[length]`); that benign FPC OOB read is dropped here
/// (its stale slot never equals the max level for any tested feeder, so the result
/// is identical).
fn normalize_levels(inc_mat: &SparseInt, levels: &mut [i32]) {
    if levels.is_empty() {
        return;
    }
    // Find the last index carrying the maximum level.
    let max_level = *levels.iter().max().unwrap();
    let mut n_levels_idx: usize = 0;
    for (i, &lv) in levels.iter().enumerate() {
        if lv == max_level {
            n_levels_idx = i;
        }
    }
    // Zero out the backbone: for each level value 1..max-1, the last bus (up to
    // n_levels_idx) carrying it becomes level 0.
    for j in 1..max_level {
        let mut zero_level: usize = 0;
        for (i, &lv) in levels.iter().enumerate().take(n_levels_idx + 1) {
            if lv == j {
                zero_level = i;
            }
        }
        levels[zero_level] = 0;
    }

    // Normalize the branches between zero-level buses so each subset starts at 1.
    let mut zero_level: usize = 0;
    let mut temp: Vec<i32> = Vec::new();
    let mut i = 0usize;
    while i < levels.len() {
        if levels[i] == 0 {
            if !temp.is_empty() {
                let bus_dot_idx = temp.iter().min().unwrap() - 1;
                for level in levels[zero_level..zero_level + temp.len()].iter_mut() {
                    *level -= bus_dot_idx;
                }
                temp.clear();
            }
            zero_level = i + 1;
        } else {
            temp.push(levels[i]);
        }
        i += 1;
    }

    // Verifies if something else was missing at the end.
    if zero_level < levels.len().saturating_sub(1) {
        let mut bus_dot_idx: i32 = 0;
        let mut j: i32 = 0; // shift-register of the previous value
        for j2 in zero_level..levels.len() {
            if levels[j2] >= j {
                bus_dot_idx += 1;
            } else {
                let ai1 = get_inc_matrix_row(inc_mat, j2 as i32);
                if ai1 < 0 {
                    bus_dot_idx = 1;
                } else {
                    let (ai2, active_cols) = get_inc_matrix_col(inc_mat, ai1);
                    if active_cols[0] == j2 as i32 {
                        bus_dot_idx = levels.get(active_cols[1] as usize).copied().unwrap_or(0) + 1;
                    } else {
                        bus_dot_idx = levels.get(ai2 as usize).copied().unwrap_or(0) + 1;
                    }
                }
            }
            j = levels[j2];
            levels[j2] = bus_dot_idx;
        }
    }
}
