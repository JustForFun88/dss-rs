//! `RecalcElementData`, `ResetIt`, and the per-mode header construction
//! (`ClearMonitorStream` plus the general V/I header for modes 0/1).

use super::{MAGNITUDEMASK, MODEMASK, Monitor, NUM_SOLUTION_VARS, POSSEQONLYMASK, SEQUENCEMASK};
use crate::elements::meter::meter_element::MeteredKind;

impl Monitor {
    /// Pascal `ResetIt`: clear the buffer and rebuild the header. `is_harmonic`
    /// is the present `ActiveCircuit.Solution.IsHarmonicModel` (it only selects
    /// the two time-column labels — see [`Self::clear_monitor_stream`]).
    pub fn reset_it(&mut self, is_harmonic: bool) {
        self.mon_buffer.clear();
        self.bufptr = 0; // Pascal `ResetIt` (Monitor.pas:1132): `BufPtr := 0`.
        self.clear_monitor_stream(is_harmonic);
    }

    /// Pascal `RecalcElementData`: validate the metered element against the
    /// mode, copy its phase/conductor counts, set the monitor's bus, and build
    /// the header (`ClearMonitorStream`).
    pub fn recalc(&mut self, errors: &mut Vec<String>, is_harmonic: bool) {
        self.valid_monitor = false;
        let Some(snap) = self.med.metered_snap.clone() else {
            errors.push(format!(
                "Monitor: \"{}\": Circuit Element is not set. Element must be defined previously.",
                self.med.cd.obj.name()
            ));
            return;
        };

        // Mode-specific element-class validation (Monitor.pas l.539).
        let class_error = match self.mode & MODEMASK {
            2 | 8 | 10 if snap.kind != MeteredKind::Transformer => {
                Some(format!("{} is not a transformer!", snap.full_name))
            }
            // Pascal mode 3 checks BASECLASSMASK = PC_ELEMENT, so Storage (a PC
            // element carrying its own `MeteredKind` for the mode-7 check) passes.
            3 if !matches!(snap.kind, MeteredKind::PcElement | MeteredKind::Storage) => {
                Some(format!(
                    "{} must be a power conversion element (Load or Generator)!",
                    snap.full_name
                ))
            }
            6 if snap.kind != MeteredKind::Capacitor => {
                Some(format!("{} is not a capacitor!", snap.full_name))
            }
            7 if snap.kind != MeteredKind::Storage => {
                Some(format!("{} is not a storage device!", snap.full_name))
            }
            _ => None,
        };
        if let Some(msg) = class_error {
            errors.push(msg);
            return;
        }

        if self.med.metered_terminal as usize > snap.nterms {
            errors.push(format!(
                "Monitor: \"{}\" Terminal no. \"{}\" does not exist. Respecify terminal no.",
                self.med.cd.obj.name(),
                self.med.metered_terminal
            ));
            return;
        }

        // Monitor adopts the metered element's phase/conductor counts and bus.
        self.med.cd.nphases = snap.nphases;
        self.med.cd.set_nconds(snap.nconds);
        let bus = snap
            .buses
            .get(self.med.metered_terminal as usize - 1)
            .cloned()
            .unwrap_or_default();
        self.med.cd.set_bus(1, &bus);

        self.clear_monitor_stream(is_harmonic);
        self.valid_monitor = true;
    }

    /// Pascal `ClearMonitorStream`: reset the buffer header and compute
    /// `RecordSize` + the per-mode header strings. The two leading time columns
    /// are labelled `Freq`/`Harmonic` when the solution is in harmonics mode
    /// (`IsHarmonicModel`), else `hour`/`t(sec)` (`Monitor.pas` l.709).
    fn clear_monitor_stream(&mut self, is_harmonic: bool) {
        self.header.clear();
        self.sample_count = 0;
        // Pascal `MonitorStream.Clear` (Monitor.pas:703): the flushed history is
        // wiped too (`BufPtr`/`MonBuffer` — the pending scratch — is untouched by
        // `ClearMonitorStream` itself; `ResetIt` separately clears `mon_buffer`).
        self.flushed_records = 0;
        if is_harmonic {
            self.header.push("Freq".into());
            self.header.push("Harmonic".into());
        } else {
            self.header.push("hour".into());
            self.header.push("t(sec)".into());
        }

        let nphases = self.med.cd.nphases;
        let nconds = self.med.cd.nconds;
        let snap = self.med.metered_snap.clone().unwrap_or_default();
        let mode_mask = self.mode & MODEMASK;

        match mode_mask {
            2 => {
                self.record_size = 1;
                self.header.push("Tap (pu)".into());
            }
            3 => {
                // Pascal `ClearMonitorStream` mode 3 (Monitor.pas l.727-731):
                // RecordSize := Length(StateBuffer) (= NumVariables), then
                // Header.Add(VariableName(i)) for i := 1 to RecordSize.
                self.record_size = snap.num_variables;
                for name in &snap.variable_names {
                    self.header.push(name.clone());
                }
            }
            4 => {
                self.record_size = 2 * nphases;
                for i in 1..=nphases {
                    self.header.push(format!("Flk{i}"));
                    self.header.push(format!("Pst{i}"));
                }
            }
            5 => {
                self.record_size = NUM_SOLUTION_VARS;
                for s in [
                    "TotalIterations",
                    "ControlIteration",
                    "MaxIterations",
                    "MaxControlIterations",
                    "Converged",
                    "IntervalHrs",
                    "SolutionCount",
                    "Mode",
                    "Frequency",
                    "Year",
                    "SolveSnap_uSecs",
                    "TimeStep_uSecs",
                ] {
                    self.header.push(s.into());
                }
            }
            6 => {
                self.record_size = snap.num_steps;
                for i in 1..=self.record_size {
                    self.header.push(format!("Step_{i}"));
                }
            }
            7 => {
                self.record_size = 5;
                for s in [
                    "kW output",
                    "kvar output",
                    "kW Stored",
                    "%kW Stored",
                    "State",
                ] {
                    self.header.push(s.into());
                }
            }
            8 | 10 => {
                let nw = snap.num_windings;
                self.record_size = 2 * nw * nphases;
                for i in 1..=nphases {
                    for j in 1..=nw {
                        self.header.push(format!("P{i}W{j}"));
                        self.header.push("Deg".into());
                    }
                }
            }
            9 => {
                self.record_size = 2;
                self.header.push("watts".into());
                self.header.push("vars".into());
            }
            11 => {
                let yorder = snap.yorder;
                self.record_size = 2 * 2 * yorder;
                for j in 1..=snap.nterms {
                    for i in 1..=snap.nconds {
                        self.header.push(format!("V{i}T{j}"));
                        self.header.push("Deg".into());
                    }
                }
                for j in 1..=snap.nterms {
                    for i in 1..=snap.nconds {
                        self.header.push(format!("I{i}T{j}"));
                        self.header.push("Deg".into());
                    }
                }
            }
            12 => {
                let np = snap.nphases;
                self.record_size = 2 * ((np * snap.nterms) + snap.yorder);
                // Phase-pair map (LL): 1->2, 2->3, ..., np->1.
                for j in 1..=snap.nterms {
                    for i in 1..=np {
                        let a = i;
                        let b = if i == np { 1 } else { i + 1 };
                        self.header.push(format!("V{a}-{b}T{j}"));
                        self.header.push("Deg".into());
                    }
                }
                for j in 1..=snap.nterms {
                    for i in 1..=snap.nconds {
                        self.header.push(format!("I{i}T{j}"));
                        self.header.push("Deg".into());
                    }
                }
            }
            _ => self.clear_general_header(nphases, nconds),
        }
    }

    /// The general V/I header (modes 0/1) with the ±16/±32/±64 modifiers.
    fn clear_general_header(&mut self, nphases: usize, nconds: usize) {
        let is_pos_seq = (self.mode & SEQUENCEMASK) > 0 && nphases == 3;
        let num_vi = if is_pos_seq { 3 } else { nconds };
        let is_power = (self.mode & MODEMASK) == 1;

        match self.mode & (MAGNITUDEMASK + POSSEQONLYMASK) {
            32 => {
                self.record_size = num_vi;
                if !is_power {
                    self.record_size += num_vi;
                    if self.include_residual {
                        self.record_size += 2;
                    }
                    for i in 1..=num_vi {
                        self.header.push(format!("|V|{i} (volts)"));
                    }
                    if self.include_residual {
                        self.header.push("|VN| (volts)".into());
                    }
                    for i in 1..=num_vi {
                        self.header.push(format!("|I|{i} (amps)"));
                    }
                    if self.include_residual {
                        self.header.push("|IN| (amps)".into());
                    }
                } else {
                    for i in 1..=num_vi {
                        if self.pp_polar {
                            self.header.push(format!("S{i} (kVA)"));
                        } else {
                            self.header.push(format!("P{i} (kW)"));
                        }
                    }
                }
            }
            64 => {
                self.record_size = 2;
                if !is_power {
                    self.record_size += 2;
                    if self.vi_polar {
                        for s in ["V1", "V1ang", "I1", "I1ang"] {
                            self.header.push(s.into());
                        }
                    } else {
                        for s in ["V1.re", "V1.im", "I1.re", "I1.im"] {
                            self.header.push(s.into());
                        }
                    }
                } else if self.pp_polar {
                    self.header.push("S1 (kVA)".into());
                    self.header.push("Ang".into());
                } else {
                    self.header.push("P1 (kW)".into());
                    self.header.push("Q1 (kvar)".into());
                }
            }
            96 => {
                self.record_size = 1;
                if !is_power {
                    self.record_size += 1;
                    self.header.push("V".into());
                    self.header.push("I".into());
                } else if self.pp_polar {
                    self.header.push("S1 (kVA)".into());
                } else {
                    self.header.push("P1 (kW)".into());
                }
            }
            _ => {
                self.record_size = num_vi * 2;
                let (i_min, i_max) = if is_pos_seq {
                    (0, num_vi - 1)
                } else {
                    (1, num_vi)
                };
                if !is_power {
                    self.record_size += num_vi * 2;
                    if self.include_residual {
                        self.record_size += 4;
                    }
                    for i in i_min..=i_max {
                        if self.vi_polar {
                            self.header.push(format!("V{i}"));
                            self.header.push(format!("VAngle{i}"));
                        } else {
                            self.header.push(format!("V{i}.re"));
                            self.header.push(format!("V{i}.im"));
                        }
                    }
                    if self.include_residual {
                        if self.vi_polar {
                            self.header.push("VN".into());
                            self.header.push("VNAngle".into());
                        } else {
                            self.header.push("VN.re".into());
                            self.header.push("VN.im".into());
                        }
                    }
                    for i in i_min..=i_max {
                        if self.vi_polar {
                            self.header.push(format!("I{i}"));
                            self.header.push(format!("IAngle{i}"));
                        } else {
                            self.header.push(format!("I{i}.re"));
                            self.header.push(format!("I{i}.im"));
                        }
                    }
                    if self.include_residual {
                        if self.vi_polar {
                            self.header.push("IN".into());
                            self.header.push("INAngle".into());
                        } else {
                            self.header.push("IN.re".into());
                            self.header.push("IN.im".into());
                        }
                    }
                } else {
                    for i in i_min..=i_max {
                        if self.pp_polar {
                            self.header.push(format!("S{i} (kVA)"));
                            self.header.push(format!("Ang{i}"));
                        } else {
                            self.header.push(format!("P{i} (kW)"));
                            self.header.push(format!("Q{i} (kvar)"));
                        }
                    }
                }
            }
        }
    }
}
