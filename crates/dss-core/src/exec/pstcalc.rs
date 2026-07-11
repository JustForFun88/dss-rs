//! The `Pstcalc` executive command (Pascal `TExecHelper.DoPstCalc`,
//! `ExecHelper.pas:4778`): run the IEC-868 flicker Pst calculator over a
//! user-supplied RMS voltage array and leave the per-interval Pst values in
//! `GlobalResult` as a `Format('%.8g, ')` list. The numerics live in
//! [`crate::support::pstcalc`].

use super::*;
use crate::support::pstcalc::pst_rms;
use crate::util::{fmt_g, interpret_dbl_array};
use dss_parser::Parser;

impl Dss {
    /// Pascal `DoPstCalc`. Sub-command table `PstCalcCommands = ['Npts',
    /// 'Voltages', 'dt', 'Frequency', 'lamp']` (`ExecHelper.pas:5067`).
    /// `dt` a.k.a. `cycles`: `CyclesPerSample := Round(Solution.Frequency *
    /// dblvalue)` — the **solution** frequency, not `DefaultBaseFreq`.
    pub(super) fn do_pst_calc_cmd(&mut self) {
        let commands = CommandList::new(
            ["Npts", "Voltages", "dt", "Frequency", "lamp"]
                .iter()
                .copied(),
        );

        let mut npts: i32 = 0;
        let mut lamp: i32 = 120; // 120 or 230
        let mut cycles_per_sample: i32 = 60;
        let mut freq: f64 = self.default_base_freq;
        let mut varray: Vec<f64> = Vec::new();

        let mut param_pointer = 0usize;
        let mut param_name = self.parser.next_param(&self.vars);
        let mut param = self.parser.make_string(&self.vars);
        while !param.is_empty() {
            if param_name.is_empty() {
                param_pointer += 1;
            } else {
                param_pointer = commands.get_command(&param_name).map_or(0, |i| i + 1);
            }
            match param_pointer {
                1 => {
                    npts = self.parser.make_integer(&self.vars).unwrap_or(0);
                    // SetLength(Varray, Npts) — zero-filled.
                    varray = vec![0.0; npts.max(0) as usize];
                }
                2 => {
                    // InterpretDblArray(DSS, Param, Npts, @Varray[0]). Upstream
                    // uses the shared AuxParser so the main command parser is
                    // untouched; a fresh scratch parser gives the same isolation.
                    let mut scratch = Parser::new();
                    let max = npts.max(0) as usize;
                    if let Err(e) =
                        interpret_dbl_array(&mut scratch, &self.vars, &param, max, &mut varray)
                    {
                        self.errors.push(e.to_string());
                    }
                }
                3 => {
                    // Round(Solution.Frequency * dblvalue) — the ACTIVE solution
                    // frequency (falls back to DefaultBaseFreq if no circuit is
                    // active yet, where Pascal would require ActiveCircuit).
                    let solution_freq = self
                        .circuit
                        .as_ref()
                        .map_or(self.default_base_freq, |ckt| ckt.solution.frequency);
                    let dv = self.parser.make_double(&self.vars).unwrap_or(0.0);
                    // TODO(compat): FPC `Round` is ties-to-even; `round_ties_even`
                    // reproduces it (wiped with the other TODO(compat) at final
                    // acceptance).
                    cycles_per_sample = (solution_freq * dv).round_ties_even() as i32;
                }
                4 => freq = self.parser.make_double(&self.vars).unwrap_or(0.0),
                5 => lamp = self.parser.make_integer(&self.vars).unwrap_or(0),
                // Pascal error 28722.
                _ => self
                    .errors
                    .push(format!("Error: Unknown Parameter on command line: {param}")),
            }
            param_name = self.parser.next_param(&self.vars);
            param = self.parser.make_string(&self.vars);
        }

        if npts > 10 {
            let pst_array = pst_rms(&varray, freq, cycles_per_sample, lamp);
            // Put the resulting Pst array in the result string.
            let mut s = String::new();
            for v in &pst_array {
                s.push_str(&format!("{}, ", fmt_g(*v, 8)));
            }
            self.last_result = s;
        } else {
            // Pascal error 28723 — the upstream typo ("Insuffient") is verbatim.
            self.errors
                .push("Insuffient number of points for Pst Calculation.".to_string());
        }
    }
}
