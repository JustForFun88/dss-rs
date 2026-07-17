//! The `Distribute` command (Pascal `ExecHelper.pas` `DoDistributeCmd:3755` +
//! `Utilities.pas` `makeDistributedGenerators:3701` and the four
//! `Write*Generators` writers, `Utilities.pas:1338-1531`): write a DSS script
//! that places a distributed generator (or load) at every enabled load bus,
//! sized per the `How=` rule. PHASE8_PLAN WP8.6 step 5.

use super::*;
use crate::report::format;

/// One enabled-or-not load row snapshot for the writers: `(1-based class
/// element index, enabled, GetBus(1), NPhases, kVLoadBase, kWBase)`.
struct LoadRow {
    idx: usize,
    enabled: bool,
    bus1: String,
    nphases: usize,
    kv_load_base: f64,
    kw_base: f64,
}

/// Format one `new generator.DG_%d  bus1=%s phases=%d kV=%-g kW=… PF=%-.3g
/// model=1` line (the shared body of all four writers; `kw_field` carries the
/// writer-specific `kW=` cell — the Skip writer's has a trailing space,
/// `Utilities.pas:1473`).
fn write_line(out: &mut String, do_generators: bool, row: &LoadRow, kw_field: &str, pf: f64) {
    if do_generators {
        out.push_str(&format!("new generator.DG_{}  bus1={}", row.idx, row.bus1));
    } else {
        out.push_str(&format!("new load.DL_{}  bus1={}", row.idx, row.bus1));
    }
    out.push_str(&format!(
        " phases={} kV={}",
        row.nphases,
        format::g(row.kv_load_base, 15)
    ));
    out.push_str(kw_field);
    out.push_str(&format!(" PF={}", format::g(pf, 3)));
    out.push_str(" model=1\n");
}

/// FPC `random`: a uniform `f64` in `[0, 1)` from a fresh entropy source.
/// Upstream `WriteRandomGenerators` calls `randomize` (time-seeded) first, so
/// the output is RNG-carried and **never golden-gated** (PHASE8_PLAN WP8.6
/// step 5 / the GAPS_PLAN RNG rule). The 53 low bits of a v4 UUID are OS
/// entropy (version/variant bits live in the high half), giving a full-width
/// uniform mantissa without a dedicated RNG dependency.
fn entropy_random() -> f64 {
    let bits = uuid::Uuid::new_v4().as_u128() as u64 & ((1u64 << 53) - 1);
    bits as f64 / (1u64 << 53) as f64
}

impl Dss {
    /// Pascal `TExecHelper.DoDistributeCmd` (`ExecHelper.pas:3755-3819`).
    pub(crate) fn do_distribute_cmd(&mut self) {
        // `DistributeCommands` (`ExecHelper.pas:5062`).
        let dist_commands = CommandList::new(
            ["kW", "how", "skip", "pf", "file", "MW", "what"]
                .iter()
                .copied(),
        );
        // Defaults.
        let mut kw = 1000.0;
        let mut how = "Proportional".to_string();
        let mut skip: i32 = 1;
        let mut pf = 1.0;
        let mut fil_name = "DistGenerators.dss".to_string();
        let mut do_generators = true;

        let mut param_pointer = 0i32;
        let mut param_name = self.parser.next_param(&self.vars);
        let mut param = self.parser.make_string(&self.vars);
        while !param.is_empty() {
            if param_name.is_empty() {
                param_pointer += 1;
            } else {
                param_pointer = dist_commands
                    .get_command(&param_name)
                    .map(|i| i as i32 + 1)
                    .unwrap_or(0);
            }
            match param_pointer {
                1 => kw = get_dbl(&mut self.parser, &self.vars, &mut self.errors).unwrap_or(0.0),
                2 => how = param.clone(),
                3 => skip = get_int(&mut self.parser, &self.vars, &mut self.errors).unwrap_or(0),
                4 => pf = get_dbl(&mut self.parser, &self.vars, &mut self.errors).unwrap_or(0.0),
                5 => fil_name = param.clone(),
                6 => {
                    kw = get_dbl(&mut self.parser, &self.vars, &mut self.errors).unwrap_or(0.0)
                        * 1000.0
                }
                7 => {
                    // `Load or Generator` — dispatch on the value's first letter.
                    do_generators = !param.to_ascii_uppercase().starts_with('L');
                }
                _ => {} // ignore unnamed and extra parms
            }
            param_name = self.parser.next_param(&self.vars);
            param = self.parser.make_string(&self.vars);
        }

        // `what=L…` unconditionally renames the output (probe-proven: an
        // explicit `file=` is overridden — `ExecHelper.pas:3814`).
        if !do_generators {
            fil_name = "DistLoads.dss".to_string();
        }

        self.make_distributed_generators(kw, pf, &how, skip, &fil_name, do_generators);
    }

    /// Pascal `makeDistributedGenerators` (`Utilities.pas:3701-3753` via
    /// `ExecHelper` — the file writer + `How` dispatch).
    fn make_distributed_generators(
        &mut self,
        kw: f64,
        pf: f64,
        how: &str,
        skip: i32,
        fname: &str,
        do_generators: bool,
    ) {
        // Relative paths resolve against the engine cwd (`Set DataPath=`),
        // exactly where Pascal's process-cwd relative `TFileStream` lands.
        let path = {
            let p = Path::new(fname);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                self.current_dir.join(p)
            }
        };
        if path.exists() {
            // Pascal error 721: refuse to overwrite.
            self.errors.push(format!(
                "File \"{fname}\" was about to be overwritten. Rename/remove the existing file and try again."
            ));
            return;
        }

        let what_str = if do_generators { "Generators" } else { "Loads" };
        let mut out = String::new();
        out.push_str("! Created with Distribute Command:\n");
        out.push_str(&format!(
            "! Distribute kW={} PF={} How={} Skip={}  file={}  what={}\n",
            format::g(kw, 6),
            format::g(pf, 6),
            how,
            skip,
            fname,
            what_str
        ));
        out.push('\n');

        // Load class element list, creation order, ALL loads (the writers
        // filter on `Enabled` themselves; Uniform's count includes disabled —
        // probe-proven, `Utilities.pas:1349`).
        let loads = self.gather_load_rows();
        let positive_sequence = self.circuit.as_ref().is_some_and(|c| c.positive_sequence);

        let how_eff = if how.is_empty() { "P" } else { how };
        match how_eff
            .chars()
            .next()
            .map(|c| c.to_ascii_uppercase())
            .unwrap_or('P')
        {
            'U' => write_uniform(&mut out, &loads, kw, pf, do_generators, positive_sequence),
            'R' => write_random(&mut out, &loads, kw, pf, do_generators, positive_sequence),
            'S' => write_every_other(
                &mut out,
                &loads,
                kw,
                pf,
                skip,
                do_generators,
                positive_sequence,
            ),
            _ => write_proportional(&mut out, &loads, kw, pf, do_generators, positive_sequence),
        }

        if std::fs::write(&path, out.as_bytes()).is_err() {
            // Pascal error 722.
            self.errors
                .push(format!("Error opening \"{fname}\" for writing. Aborting."));
            return;
        }
        // `DSS.GlobalResult := Fname` + the finally's `SetLastResultFile`.
        self.last_result = fname.to_string();
        self.vars.add("@lastfile", fname);
        self.last_result_file = fname.to_string();
    }

    /// Snapshot the Load class element list (creation order, 1-based indices —
    /// the `DG_%d` numbering).
    fn gather_load_rows(&self) -> Vec<LoadRow> {
        let Some(&ci) = self.class_by_name.get("load") else {
            return Vec::new();
        };
        self.classes[ci]
            .objects
            .iter()
            .enumerate()
            .map(|(i, obj)| {
                let l = obj
                    .as_any()
                    .downcast_ref::<load::Load>()
                    .expect("Load class holds Load objects");
                LoadRow {
                    idx: i + 1,
                    enabled: l.cd.enabled,
                    bus1: l.cd.get_bus(1).to_string(),
                    nphases: l.cd.nphases,
                    kv_load_base: l.kv_load_base,
                    kw_base: l.kw_base,
                }
            })
            .collect()
    }
}

/// Pascal `WriteUniformGenerators` (`Utilities.pas:1338`): `kW / Max(1,
/// Count)` each — `Count` is the FULL element count, disabled loads included
/// (probe-proven); ÷3 if PositiveSequence.
fn write_uniform(
    out: &mut String,
    loads: &[LoadRow],
    kw: f64,
    pf: f64,
    do_generators: bool,
    positive_sequence: bool,
) {
    let mut kw_each = kw / 1.0f64.max(loads.len() as f64);
    if positive_sequence {
        kw_each /= 3.0;
    }
    for row in loads.iter().filter(|r| r.enabled) {
        let kw_field = format!(" kW={}", format::g(kw_each, 15));
        write_line(out, do_generators, row, &kw_field, pf);
    }
}

/// Pascal `WriteRandomGenerators` (`Utilities.pas:1375`): `kW / <enabled
/// count>` scaled by `random * 2.0` per load — RNG-carried upstream
/// (`randomize`, time-seeded), so never golden-gated.
fn write_random(
    out: &mut String,
    loads: &[LoadRow],
    kw: f64,
    pf: f64,
    do_generators: bool,
    positive_sequence: bool,
) {
    let load_count = loads.iter().filter(|r| r.enabled).count();
    let mut kw_each = kw / load_count as f64; // ÷0 → inf, like FPC float div
    if positive_sequence {
        kw_each /= 3.0;
    }
    for row in loads.iter().filter(|r| r.enabled) {
        let kw_field = format!(" kW={}", format::g(kw_each * entropy_random() * 2.0, 15));
        write_line(out, do_generators, row, &kw_field, pf);
    }
}

/// Pascal `WriteEveryOtherGenerators` (`Utilities.pas:1426`): every
/// `(Skip+1)`-th enabled load, `kW·kWBase/ΣkWBase` over the selected set.
/// NOTE the writer's `kW=` cell has a trailing space (`Utilities.pas:1473`).
fn write_every_other(
    out: &mut String,
    loads: &[LoadRow],
    kw: f64,
    pf: f64,
    skip: i32,
    do_generators: bool,
    positive_sequence: bool,
) {
    // Pass 1: sum kWBase over the selected (non-skipped) enabled loads.
    let mut total_kw = 0.0;
    let mut skip_count = skip;
    for row in loads.iter().filter(|r| r.enabled) {
        if skip_count == 0 {
            total_kw += row.kw_base;
            skip_count = skip;
        } else {
            skip_count -= 1;
        }
    }
    let kw_each = if positive_sequence {
        kw / total_kw / 3.0
    } else {
        kw / total_kw
    };
    // Pass 2: write the selected loads.
    let mut skip_count = skip;
    for row in loads.iter().filter(|r| r.enabled) {
        if skip_count == 0 {
            let kw_field = format!(" kW={} ", format::g(kw_each * row.kw_base, 15));
            write_line(out, do_generators, row, &kw_field, pf);
            skip_count = skip;
        } else {
            skip_count -= 1;
        }
    }
}

/// Pascal `WriteProportionalGenerators` (`Utilities.pas:1489`):
/// `kW·kWBase/ΣkWBase` over all enabled loads.
fn write_proportional(
    out: &mut String,
    loads: &[LoadRow],
    kw: f64,
    pf: f64,
    do_generators: bool,
    positive_sequence: bool,
) {
    let total_kw: f64 = loads.iter().filter(|r| r.enabled).map(|r| r.kw_base).sum();
    let kw_each = if positive_sequence {
        kw / total_kw / 3.0
    } else {
        kw / total_kw
    };
    for row in loads.iter().filter(|r| r.enabled) {
        let kw_field = format!(" kW={}", format::g(kw_each * row.kw_base, 15));
        write_line(out, do_generators, row, &kw_field, pf);
    }
}
