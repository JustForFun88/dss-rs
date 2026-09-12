//! The report-golden [`ExportPolicy`] table, lifted out of
//! `tests/golden_reports.rs` so the LIVE run-file contents surface
//! (`GOLDEN_REBASE_PLAN.md` G1.10b, [`super::run_file_contents`]) can name the
//! very same policy its golden names — "same policy" as a compile-time fact
//! rather than a comment (spec `tmp/g110b/spec.md` §2.3(1), coordinator
//! decision **D40(2)**).
//!
//! **This module is a byte-faithful move, not a rewrite** (`SPLITTING_RULES.md`
//! protocol): every `ExportPolicy` literal below is the one that stood inline in
//! the golden test that now calls it, field for field, and the two functions
//! that were already named (`ang_tol`, `profile_policy`, `register_policy`)
//! carry their original doc comments verbatim. No value moved — the unchanged
//! `golden_reports` run and `golden_lock` are the proof. Nothing here may ever
//! be *re-tuned* to make a live file pass: the live rule is the separate
//! `case floor + print ulp` derivation in [`super::run_file_contents`]
//! (D40(1)), and these values keep gating the committed golden bytes.

use super::{ColSel, ColTol, ExportPolicy, GateSpec, RowPolicy};

/// The noise gate for the angle columns of a paired magnitude/angle report: the
/// angle of a real magnitude is byte-identical (exact, `rel = abs = 0`); only
/// the angle of a **near-zero residual** magnitude (a per-terminal residual or
/// an open-terminal conductor — faer-vs-KLU cancellation noise, observed e.g.
/// `-33.69` vs `56.31`°) is skipped, gated on the paired magnitude (the
/// immediately-preceding column, `PrevCol`); `thresh` (1e-6) sits far above that
/// noise (≤ ~1e-8) and below the smallest real printed magnitude. Selected by
/// index parity (`start`, then every other column is an angle) because these
/// reports have **truncated headers** (`…, I_1, Ang_1, ...`) that name only the
/// first pair.
pub fn ang_tol(start: usize) -> ColTol {
    ColTol {
        sel: ColSel::Parity { start, parity: 1 },
        rel: 0.0,
        abs: 0.0,
        gate: Some(GateSpec::PrevCol(1e-6)),
    }
}

/// `Export Voltages` (r4133 `Common/ExportResults.pas:236` `ExportVoltages`,
/// the header at `:265-266`, the bus row at `:272` and the per-node
/// `', %d, %10.6g, %6.1f, %9.5g'` quadruple at `:288`): the policy of the byte golden
/// `golden_reports::export_voltages_matches_oracle` — byte-identical
/// Rust↔oracle (incl. the `%6.1f` angle columns) → exact equality.
pub fn voltages_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    }
}

/// `Export Powers` (r4133 `Common/ExportResults.pas:1069` `ExportPowers`, the header at
/// `:1095` and the `:11:1` value writes at `:1113-1126`): the policy of the byte golden
/// `golden_reports::export_powers_matches_oracle` — every value column is
/// `%11.1f`, the underlying kW agree to ~1e-9 rel, far below the print step →
/// exact equality.
pub fn powers_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    }
}

/// `Export Currents` (r4133 `Common/ExportResults.pas:520` `ExportCurrents`, header at
/// `:551-555`, + `:458` `CalcAndWriteCurrents`, the `', %10.6g, %8.2f'` pair
/// writes at `:471`/`:474`/`:475`/`:480`): the policy of the byte golden
/// `golden_reports::export_currents_matches_oracle`. Every real magnitude and
/// angle is byte-identical → `rel = 0`; `abs = 1e-8` absorbs only the near-zero
/// `Iresid` cancellation residuals, and the angle of a residual/fill magnitude
/// is gated via [`ang_tol`].
pub fn currents_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 1e-8,
        col_tol: vec![ang_tol(1)],
    }
}

/// `Export Y triplet` (r4133 `Common/ExportResults.pas:3016` `ExportY`, the
/// `TripletOpt` arm at `:3052-3060`): the policy of the byte golden
/// `golden_reports::export_y_triplet_matches_oracle` — integer Row/Col and the
/// `%.10g` G/B all exact.
///
/// The **dense** arm of the same procedure (`:3061-3097`: the node-count header at `:3069`, the row name at
/// `:3078` and the `'%-13.10g, +j %-13.10g,'` cell at `:3090`) writes the same file
/// name with a different layout and has no golden of its own; the live surface
/// reuses this policy for its structure (`sep`, `header_lines`, `ExactOrdered`)
/// and supplies the dense column map itself — see
/// [`super::run_file_contents::ReportKind::YDense`].
pub fn y_triplet_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    }
}

/// `Export Yprims` (r4133 `Common/ExportResults.pas:2850` `ExportYprim`, the
/// `%-13.10g` pair writer at `:2876`): the policy of the byte golden
/// `golden_reports::export_yprims_matches_oracle` — a `Class.NAME` header line
/// then `Yorder` rows of `re, im,` pairs, so `header_lines = 0`; the name lines
/// are single text fields (matched case-insensitively), the matrix cells exact.
pub fn yprims_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    }
}

/// Register rows: `%10.0f` integers, byte-identical (no last-digit rounding
/// straddle on these fixtures) — exact equality.
///
/// (Lift note — the doc above is the original; the writers it gates are r4133
/// `Common/ExportResults.pas` `WriteSinglePVSystemMeterFile` `:2100` and the
/// `/m` twin `WriteMultiplePVSystemMeterFiles` `:1990`, which share the
/// `'Year, LDCurve, Hour, PVSystem'` header (`:2017`/`:2145`) and the
/// `Registers[j]:10:0` value writes (`:2030`/`:2161`) — the reason the live
/// `EXP_PV_<NAME>.CSV` of the `/m` arm may ride the single-file golden's policy.)
pub fn register_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    }
}

/// The shared `Export Profile` tolerance policy: the `%.6g` `puV` columns, the
/// zone-build `Distance` constants and the integer Color/Thickness/Linetype/
/// marker columns are all byte-identical — exact equality. The header line
/// (incl. the appended `Title=…` tail) is compared verbatim.
///
/// (Lift note — the doc above is the original; the writer is r4133
/// `Common/ExportResults.pas:3371` `ExportProfile`, header `:3391`, row
/// `:3356` `WriteNewLine` with the `'%s, %.6g, %.6g, %.6g, %.6g,'` name/
/// distance/pu quintuple at `:3363` and the seven integer marker columns at
/// `:3364-3366`.)
pub fn profile_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every policy this module hands the live surface is a CSV policy with the
    /// header-line count its writer emits — the two structural fields the live
    /// comparator reads out of the golden policy
    /// ([`super::super::run_file_contents`]). A policy that quietly changed
    /// separator or header count would silently re-frame both the golden bytes
    /// and the live files, so the pair is pinned here as well as in the goldens.
    #[test]
    fn the_lifted_policies_keep_their_separator_and_header_count() {
        for (name, p, header_lines) in [
            ("voltages", voltages_policy(), 1),
            ("powers", powers_policy(), 1),
            ("currents", currents_policy(), 1),
            ("y_triplet", y_triplet_policy(), 1),
            ("yprims", yprims_policy(), 0),
            ("register", register_policy(), 1),
            ("profile", profile_policy(), 1),
        ] {
            assert_eq!(p.sep, ',', "{name}: separator moved");
            assert_eq!(p.header_lines, header_lines, "{name}: header count moved");
            assert!(
                matches!(p.rows, RowPolicy::ExactOrdered),
                "{name}: row policy moved off ExactOrdered"
            );
        }
    }

    /// The values themselves, pinned so the "byte-faithful lift" claim has a
    /// test and not only a doc comment: the four exact-equality policies, the
    /// `Export Currents` residual floor (`abs = 1e-8`) with its one angle
    /// override, and [`ang_tol`]'s selector/gate.
    #[test]
    fn the_lifted_policy_values_are_the_ones_the_goldens_carried() {
        for (name, p) in [
            ("voltages", voltages_policy()),
            ("powers", powers_policy()),
            ("y_triplet", y_triplet_policy()),
            ("yprims", yprims_policy()),
            ("register", register_policy()),
            ("profile", profile_policy()),
        ] {
            assert_eq!(p.rel, 0.0, "{name}: rel moved");
            assert_eq!(p.abs, 0.0, "{name}: abs moved");
            assert!(p.col_tol.is_empty(), "{name}: gained a column override");
        }
        let c = currents_policy();
        assert_eq!(c.rel, 0.0);
        assert_eq!(c.abs, 1e-8);
        assert_eq!(c.col_tol.len(), 1);
        let a = ang_tol(1);
        assert!(matches!(
            a.sel,
            ColSel::Parity {
                start: 1,
                parity: 1
            }
        ));
        assert_eq!((a.rel, a.abs), (0.0, 0.0));
        assert!(matches!(a.gate, Some(GateSpec::PrevCol(t)) if t == 1e-6));
    }
}
