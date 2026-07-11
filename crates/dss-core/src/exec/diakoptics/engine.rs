//! The `ADiakopticsInit` state machine + `ADiakoptics_Tearing` + `get_Statistics`
//! — a loop-for-loop port of official `Diakoptics.pas:506–796` (plan D10).
//!
//! Under plan D3 the coordinator is the main [`Dss`] (Pascal `ActiveCircuit[1]`);
//! after `ClearAll` it recompiles `Torn_Circuit/Master_Interconnected.dss` into
//! its own circuit, and each child in `ad_children` (Pascal `ActiveCircuit[2..]`)
//! compiles a zone master. "Sending a message" is a synchronous method call.
//!
//! The per-iteration AD **solve** (Solve_Diakoptics / SolveAD / the boundary
//! current stitch) is WP-AD.3 Stage 2b — see [`Dss::ad_solve`]. Init builds the
//! coordinator, the children, and the four matrices, and flips `ADiakoptics` on.

use super::super::registry::ClassStore;
use crate::exec::Dss;
use crate::solution::SolveEnv;
use crate::solution::ymatrix::{BuildOption, build_y_matrix};

impl Dss {
    /// Pascal `ADiakoptics_Tearing(AddISrc)` (Diakoptics.pas:506): tear the
    /// circuit, snapshot-mode + controls-off + rebuild Y, and (unless the solve
    /// aborted) emit the `Torn_Circuit/` tree. Returns `0` on success, `1` on
    /// error (`GlobalResult` carries the message). Shared by the `Tear_Circuit`
    /// command and `ADiakopticsInit` state 0.
    pub(super) fn adiakoptics_tearing(&mut self, add_isrc: bool) -> i32 {
        let num_ckts = match self.tear_circuit() {
            Ok(n) => n,
            Err(_) => {
                self.last_result = "There was an error when tearing the circuit ".to_string();
                return 1;
            }
        };
        // Diakoptics.pas:516–519 — snapshot mode + controls off + rebuild Y. The
        // mode toggle is a net no-op for the file emission; we issue the controls
        // reset and the meter-zone rebuild `BuildYMatrix`'s `ReprocessBusDefs`
        // tail performs so `SaveFeeders` sees the just-placed zone meters.
        self.command("set controlmode=off");
        self.reset_meter_zones_for_tear();

        let aborted = self
            .circuit
            .as_ref()
            .is_some_and(|c| c.solution.solution_abort);
        if aborted {
            self.last_result = "There was an error when tearing the circuit ".to_string();
            return 1;
        }
        self.save_sub_circuits(add_isrc);
        if let Some(ckt) = self.circuit.as_mut() {
            ckt.ad.num_sub_ckts = num_ckts;
        }
        self.last_result = format!("Sub-Circuits Created: {num_ckts}");
        0
    }

    /// Rebuild the active circuit's whole Y matrix (Pascal
    /// `Ymatrix.BuildYMatrix(WHOLEMATRIX, FALSE, ActorID)`), no V realloc.
    fn ad_build_y(&mut self) {
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let Some(ckt) = circuit.as_mut() else {
            return;
        };
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        let _ = build_y_matrix(ckt, &mut env, BuildOption::WholeMatrix, false);
    }

    /// Enable/disable a circuit element by `Class.Name` on `self` (direct field
    /// mutation, Pascal `ActiveCktElement.Enabled := …`), forcing a Y rebuild but
    /// NOT a bus redefinition (Diakoptics.pas:666–670 sets `BusNameRedefined :=
    /// False`). The Pascal state-2 child disable uses the executive command form
    /// `X.enabled=False`; the effect (Enabled + SystemYChanged) is identical.
    fn ad_set_element_enabled(&mut self, full_name: &str, enabled: bool) {
        let lower = full_name.to_lowercase();
        let (cls, name) = match lower.split_once('.') {
            Some((c, n)) => (Some(c), n),
            None => (None, lower.as_str()),
        };
        for class in &mut self.classes {
            if class.kind.is_none() {
                continue;
            }
            if let Some(c) = cls
                && !class.props.class_name().eq_ignore_ascii_case(c)
            {
                continue;
            }
            if let Some(&oi) = class.name_to_idx.get(name)
                && let Some(ce) = class.objects[oi].as_ckt_element_mut()
            {
                ce.cd_mut().set_enabled(enabled);
                if let Some(ckt) = self.circuit.as_mut() {
                    ckt.solution.system_y_changed = true;
                }
                return;
            }
        }
    }

    /// Disable every EnergyMeter whose name contains `zone_` (Diakoptics.pas:620–
    /// 626): the per-zone meters are turned off in the interconnected coordinator.
    fn disable_zone_meters(&mut self) {
        let meters = match self.circuit.as_ref() {
            Some(c) => c.energy_meters.clone(),
            None => return,
        };
        for r in meters {
            if let Some(ce) = self.classes[r.cls].objects[r.idx].as_ckt_element() {
                let is_zone = ce.cd().obj.name().to_ascii_lowercase().contains("zone_");
                if is_zone && let Some(ce) = self.classes[r.cls].objects[r.idx].as_ckt_element_mut()
                {
                    ce.cd_mut().set_enabled(false);
                }
            }
        }
    }

    /// Pascal `ADiakopticsInit()` (Diakoptics.pas:541): the 0–9 state machine that
    /// tears the circuit, recompiles the interconnected coordinator + per-zone
    /// child engines, opens the link branches, and builds the A-Diakoptics
    /// matrices (`Contours`/`ZLL`/`ZCC`/`Y4`). On success `Solution.ADiakoptics`
    /// is set TRUE and `GlobalResult` carries the full init summary; on any error
    /// the flag stays FALSE and the summary reports the failing state.
    ///
    /// State 9's `INIT_ADIAKOPTICS` child setup (`IndexBuses`/`Start_Diakoptics`)
    /// and the per-iteration solve are WP-AD.3 Stage 2b.
    pub(crate) fn adiakoptics_init(&mut self) {
        let cpu_cores = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(1);

        // Clamp the sub-circuit count (Diakoptics.pas:572–573). NOTE: on a host
        // with < 4 logical CPUs this can pull `Num_SubCkts` below a test's
        // requested value; the AD gates therefore assume ≥ 4 cores (plan D6 —
        // the count is fixed per test, never derived, but this hard cap is
        // ported for fidelity).
        if let Some(ckt) = self.circuit.as_mut()
            && ckt.ad.num_sub_ckts > cpu_cores - 2
        {
            ckt.ad.num_sub_ckts = cpu_cores - 2;
        }

        let mut prog = String::from("A-Diakoptics initialization summary:\r\n\r\n");
        let mut error_code = 0i32;
        let mut links: Vec<String> = Vec::new();
        let mut diak_actors = 0i32;
        // Torn_Circuit output root, captured before any ClearAll/compile moves
        // `current_dir` (Pascal `Fileroot := GetCurrentDir`).
        let torn_root = self.current_dir.join("Torn_Circuit");

        let mut state = 0;
        loop {
            match state {
                0 => {
                    // Create the sub-circuits (tear + emit Torn_Circuit).
                    prog.push_str("- Creating Sub-Circuits...\r\n");
                    error_code = self.adiakoptics_tearing(false);
                    let n = self
                        .circuit
                        .as_ref()
                        .map(|c| c.ad.num_sub_ckts)
                        .unwrap_or(0);
                    if error_code != 0 {
                        prog.push_str("Error\r\nThe circuit cannot be decomposed\r\n");
                    } else {
                        prog.push_str(&format!("  {n} Sub-Circuits Created\r\n"));
                    }
                }
                1 => {
                    // Save the link-branch list locally (survives ClearAll).
                    diak_actors = self
                        .circuit
                        .as_ref()
                        .map(|c| c.ad.num_sub_ckts)
                        .unwrap_or(0)
                        + 1;
                    prog.push_str("- Indexing link branches...");
                    links = self
                        .circuit
                        .as_ref()
                        .map(|c| c.ad.link_branches.clone())
                        .unwrap_or_default();
                    prog.push_str("Done");
                }
                2 => {
                    // Compile the interconnected coordinator + the child zones.
                    prog.push_str("\r\n- Setting up the Actors...");
                    self.ad_children.clear();
                    self.command("clear");

                    let interconnected = torn_root.join("Master_Interconnected.dss");
                    self.command(&format!("compile \"{}\"", interconnected.display()));
                    self.command("set controlmode=off");
                    self.disable_zone_meters();
                    // Pascal issues `BuildYMatrix(FALSE)` here, but on a freshly
                    // compiled circuit the solution vectors are not yet allocated;
                    // the `solve` below runs the allocating build (`system_y_changed`
                    // is still set from the compile) — so we skip the redundant
                    // non-allocating pre-build that would clear that flag.
                    self.command("solve");

                    // Children: actor 2 = zone 1 (feeder head), actor k = zone_{k-1}.
                    for didx in 2..=diak_actors {
                        let dir = if didx == 2 {
                            String::new()
                        } else {
                            format!("zone_{}", didx - 1)
                        };
                        let master = if dir.is_empty() {
                            torn_root.join("Master.dss")
                        } else {
                            torn_root.join(&dir).join("Master.dss")
                        };
                        let mut child = Dss::new();
                        child.command(&format!("compile \"{}\"", master.display()));
                        if didx > 2 {
                            let link = links.get((didx - 2) as usize).cloned().unwrap_or_default();
                            child.ad_set_element_enabled(&link, false);
                        }
                        child.command("set controlmode=off");
                        child.command("solve");
                        let aborted = child
                            .circuit
                            .as_ref()
                            .is_some_and(|c| c.solution.solution_abort)
                            || !child.errors().is_empty();
                        self.ad_children.push(child);
                        if aborted {
                            error_code = 1;
                            break;
                        }
                    }
                    if error_code != 0 {
                        prog.push_str("Error\r\nOne or sub-systems cannot be compiled\r\n");
                    } else {
                        prog.push_str("Done");
                    }
                }
                3 => {
                    // Open the link branches in the coordinator, rebuild the torn Y.
                    prog.push_str("\r\n- Opening link branches...");
                    for link in links.iter().skip(1) {
                        self.ad_set_element_enabled(link, false);
                    }
                    if let Some(ckt) = self.circuit.as_mut() {
                        ckt.set_bus_name_redefined(false);
                    }
                    self.ad_build_y();
                    prog.push_str("Done");
                }
                4 => {
                    prog.push_str("\r\n- Building Contours...");
                    error_code = self.calc_c_matrix(&links);
                    if error_code != 0 {
                        prog.push_str("Error\r\nOne or more link branches are not lines\r\n");
                    } else {
                        prog.push_str("Done");
                    }
                }
                5 => {
                    prog.push_str("\r\n- Building ZLL...");
                    error_code = self.calc_zll(&links);
                    if error_code != 0 {
                        prog.push_str("Error");
                    } else {
                        prog.push_str("Done");
                    }
                }
                6 => {
                    prog.push_str("\r\n- Building ZCC...");
                    self.calc_zcc(links.len());
                    prog.push_str("Done");
                }
                7 => {
                    prog.push_str("\r\n- Building Y4 ...");
                    self.calc_y4();
                    // Move the link list back into the coordinator (Diakoptics.pas:710).
                    if let Some(ckt) = self.circuit.as_mut() {
                        ckt.ad.link_branches = links.clone();
                    }
                    prog.push_str("Done");
                }
                8 => {
                    prog.push_str("\r\n- Assigning indexes to actors ...");
                    self.send_idx_2_actors();
                    prog.push_str("Done");
                }
                9 => {
                    // Partitioning statistics + close the link branches + rebuild Y.
                    prog.push_str("\r\n\r\nPartitioning statistics");
                    prog.push_str(&self.get_statistics());
                    prog.push_str("\r\n- Closing link branches...");
                    for link in links.iter().skip(1) {
                        self.ad_set_element_enabled(link, true);
                    }
                    if let Some(ckt) = self.circuit.as_mut() {
                        ckt.set_bus_name_redefined(false);
                    }
                    self.ad_build_y();
                    // INIT_ADIAKOPTICS (IndexBuses / Start_Diakoptics) + the
                    // per-iteration solve are WP-AD.3 Stage 2b.
                    if let Some(ckt) = self.circuit.as_mut() {
                        ckt.solution.adiak_init = true;
                    }
                }
                _ => {}
            }
            state += 1;
            if state > 9 || error_code != 0 {
                break;
            }
        }

        // Finalize (Diakoptics.pas:759–795).
        let err_str = if error_code != 0 {
            if let Some(ckt) = self.circuit.as_mut() {
                ckt.solution.adiakoptics = false;
            }
            "One or more errors found"
        } else {
            if let Some(ckt) = self.circuit.as_mut() {
                ckt.solution.parallel_enabled = true;
                ckt.solution.adiakoptics = true;
                ckt.solution.adiak_init = false; // force subzones to drop VSource.Source
            }
            "A-Diakoptics initialized"
        };
        let prog = format!("\r\n{prog}\r\n{err_str}\r\n");
        self.last_result = prog;
        if let Some(ckt) = self.circuit.as_mut() {
            ckt.solution.solution_abort = false;
        }
    }

    /// Pascal `get_Statistics()` (Diakoptics.pas:90): the per-zone node-count
    /// reduction/imbalance summary (`%4.2f`-class `floattostrf(…, ffgeneral,4,2)`).
    /// Machine-independent only because every AD test fixes the zone count (D6).
    pub(crate) fn get_statistics(&self) -> String {
        // ASize[k] = NumNodes of each child (actors 2..NumOfActors).
        let a_size: Vec<f64> = self
            .ad_children
            .iter()
            .map(|c| {
                c.circuit
                    .as_ref()
                    .map(|k| k.num_nodes as f64)
                    .unwrap_or(0.0)
            })
            .collect();
        let coord_nodes = self
            .circuit
            .as_ref()
            .map(|c| c.num_nodes as f64)
            .unwrap_or(0.0);
        let max_size = a_size.iter().cloned().fold(f64::MIN, f64::max);
        let g_reduct = if coord_nodes != 0.0 {
            (1.0 - max_size / coord_nodes) * 100.0
        } else {
            0.0
        };
        let unbalance: Vec<f64> = a_size
            .iter()
            .map(|&s| {
                if max_size != 0.0 {
                    (1.0 - s / max_size) * 100.0
                } else {
                    0.0
                }
            })
            .collect();
        let max_imbal = unbalance.iter().cloned().fold(f64::MIN, f64::max);
        let avg_imbal = if unbalance.is_empty() {
            0.0
        } else {
            unbalance.iter().sum::<f64>() / unbalance.len() as f64
        };
        format!(
            "\r\nCircuit reduction    (%): {}\r\nMax imbalance       (%): {}\r\nAverage imbalance(%): {}\r\n",
            fmt_g42(g_reduct),
            fmt_g42(max_imbal),
            fmt_g42(avg_imbal),
        )
    }
}

/// Pascal `floattostrf(x, ffgeneral, 4, 2)`: general format, 4 significant digits.
/// For the small percentages `get_Statistics` produces this matches Rust's `{}`
/// after rounding to 4 significant figures.
fn fmt_g42(x: f64) -> String {
    if x == 0.0 {
        return "0".to_string();
    }
    // 4 significant digits (ffGeneral Precision=4), trailing zeros trimmed.
    let mag = x.abs().log10().floor() as i32;
    let decimals = (3 - mag).max(0) as usize;
    let s = format!("{x:.decimals$}");
    // Trim trailing zeros / dot (ffGeneral drops them).
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}
