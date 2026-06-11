//! Solution-mode and dynamics-state definitions, port of `Shared/Dynamics.pas`.

/// Solution modes (Pascal `TSolveMode`); discriminants are part of the
/// engine's external behavior (`Set mode=` and golden `solution.mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum SolveMode {
    #[default]
    Snapshot = 0,
    Daily = 1,
    /// 8760-hour simulation.
    Yearly = 2,
    MonteCarlo1 = 3,
    LoadDuration1 = 4,
    PeakDay = 5,
    DutyCycle = 6,
    Direct = 7,
    /// Monte Carlo fault study.
    MonteFault = 8,
    /// Run through all buses, compute Voc and Zsc, then ask for fault current.
    FaultStudy = 9,
    MonteCarlo2 = 10,
    MonteCarlo3 = 11,
    LoadDuration2 = 12,
    AutoAddFlag = 13,
    Dynamic = 14,
    Harmonic = 15,
    GeneralTime = 16,
    /// Sequential-time harmonics mode.
    HarmonicT = 17,
}

impl SolveMode {
    pub fn code(self) -> i32 {
        self as i32
    }

    pub fn from_code(code: i32) -> Option<SolveMode> {
        Some(match code {
            0 => SolveMode::Snapshot,
            1 => SolveMode::Daily,
            2 => SolveMode::Yearly,
            3 => SolveMode::MonteCarlo1,
            4 => SolveMode::LoadDuration1,
            5 => SolveMode::PeakDay,
            6 => SolveMode::DutyCycle,
            7 => SolveMode::Direct,
            8 => SolveMode::MonteFault,
            9 => SolveMode::FaultStudy,
            10 => SolveMode::MonteCarlo2,
            11 => SolveMode::MonteCarlo3,
            12 => SolveMode::LoadDuration2,
            13 => SolveMode::AutoAddFlag,
            14 => SolveMode::Dynamic,
            15 => SolveMode::Harmonic,
            16 => SolveMode::GeneralTime,
            17 => SolveMode::HarmonicT,
            _ => return None,
        })
    }
}

/// Iteration flag inside [`DynaVars`]: whether the present iteration starts a
/// new time step or repeats the same one (Pascal used a bare Integer 0/1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IterationFlag {
    #[default]
    NewTimeStep,
    SameTimeStep,
}

/// Time/step state shared by the solution and dynamic models
/// (Pascal `TDynamicsRec`, conventionally named `DynaVars` in the engine).
#[derive(Debug, Clone, Default)]
pub struct DynaVars {
    /// Time step size in seconds for dynamics.
    pub h: f64,
    /// Seconds from the top of the hour.
    pub t: f64,
    pub t_start: f64,
    pub t_stop: f64,
    pub iteration_flag: IterationFlag,
    pub solution_mode: SolveMode,
    /// Time in hours as an integer.
    pub int_hour: i32,
    /// Time in hours as a float, including the fractional part.
    pub dbl_hour: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_match_pascal_ordinals() {
        assert_eq!(SolveMode::Snapshot.code(), 0);
        assert_eq!(SolveMode::DutyCycle.code(), 6);
        assert_eq!(SolveMode::Dynamic.code(), 14);
        assert_eq!(SolveMode::HarmonicT.code(), 17);
        for code in 0..=17 {
            assert_eq!(SolveMode::from_code(code).unwrap().code(), code);
        }
        assert_eq!(SolveMode::from_code(18), None);
        assert_eq!(SolveMode::from_code(-1), None);
    }

    #[test]
    fn defaults() {
        let d = DynaVars::default();
        assert_eq!(d.solution_mode, SolveMode::Snapshot);
        assert_eq!(d.iteration_flag, IterationFlag::NewTimeStep);
        assert_eq!(d.int_hour, 0);
        assert_eq!(d.h, 0.0);
    }
}
