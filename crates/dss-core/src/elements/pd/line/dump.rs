//! Pascal `TLineObj.DumpProperties` (`PDElements/Line.pas:1392`) — the
//! `Dump line.…` override. After the inherited `TDSSCktElement` prefix it dumps
//! `Bus1`/`Bus2`/`LineCode`/`Length`/`Phases`, the sym-component sequence
//! parameters (`R1`/`X1`/`R0`/`X0`/`C1`/`C0`, each `%-.7g` **or** the literal
//! `----` when the element is on the matrix model), the full `RMatrix`/`XMatrix`/
//! `CMatrix` (row-major, `|`-separated rows, quoted), the `Switch` flag, then the
//! generic property tail from `Rg` on. Matrix cells fold out the length + units
//! (`LengthMult = Len` when geometry/spacing embeds length in `Z`, else `1`).

use crate::report::format::{fixed, g};
use crate::report::save::dump::{self, DumpCtx};
use crate::util::str_y_or_n;

use super::{Line, prop};

impl Line {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        dump::prefix_ckt(out, cx, self, &self.cd, complete);
        let name = |i| cx.cls.property_name(i);

        out.push_str(&format!("~ {}={}\n", name(prop::BUS1), self.cd.get_bus(1)));
        out.push_str(&format!("~ {}={}\n", name(prop::BUS2), self.cd.get_bus(2)));
        // Pascal `Writeln(F,'~ ',PropertyName^[3],'=',CondCode)` — r4133
        // `Line.pas:1273`. Unconditional: `DumpProperties` prints the raw
        // `CondCode`, which survives every `FLineCodeSpecified := FALSE`, so a
        // line whose impedance was overridden still dumps the code it was built
        // from while `? line.x.linecode` answers `''` (`:1357`). dss_capi 0.14.5
        // has no `CondCode` at all and renders the live object instead (`if
        // LineCodeObj <> NIL then LineCodeObj.Name else ''`), so it prints
        // nothing there; r4133 is the behavioral authority (CLAUDE.md
        // 2026-08-02). Both readings measured on the two DLLs, RP3.6 probe
        // deck C.
        out.push_str(&format!(
            "~ {}={}\n",
            name(prop::LINECODE),
            self.line_code_name
        ));
        out.push_str(&format!("~ {}={}\n", name(prop::LENGTH), g(self.len, 15)));
        out.push_str(&format!("~ {}={}\n", name(prop::PHASES), self.cd.nphases));

        let sym = self.sym_components_model;
        let uc = self.units_convert;
        for (idx, val) in [
            (prop::R1, self.r1),
            (prop::X1, self.x1),
            (prop::R0, self.r0),
            (prop::X0, self.x0),
            (prop::C1, self.c1 * 1.0e9),
            (prop::C0, self.c0 * 1.0e9),
        ] {
            let s = if sym {
                g(val / uc, 7)
            } else {
                "----".to_string()
            };
            out.push_str(&format!("~ {}={s}\n", name(idx)));
        }

        let np = self.cd.nphases;
        // Pascal: geometry/spacing embed length in Z, so scale it back out.
        let length_mult = if self.geometry_obj.is_some() || self.spacing_specified() {
            self.len
        } else {
            1.0
        };
        // RMatrix = Z.re, XMatrix = Z.im, both %.9f / (LengthMult·FUnitsConvert).
        out.push_str(&format!("~ {}=\"", name(prop::RMATRIX)));
        if let Some(z) = &self.z {
            write_matrix(out, np, |i, j| fixed(z.get(i, j).re / length_mult / uc, 9));
        }
        out.push_str("\"\n");
        out.push_str(&format!("~ {}=\"", name(prop::XMATRIX)));
        if let Some(z) = &self.z {
            write_matrix(out, np, |i, j| fixed(z.get(i, j).im / length_mult / uc, 9));
        }
        out.push_str("\"\n");
        // CMatrix = Yc.im → nF/unit, %.3f.
        out.push_str(&format!("~ {}=\"", name(prop::CMATRIX)));
        if let Some(yc) = &self.yc {
            let denom = std::f64::consts::TAU * self.cd.base_frequency * length_mult * uc;
            write_matrix(out, np, |i, j| fixed(yc.get(i, j).im / denom * 1.0e9, 3));
        }
        out.push_str("\"\n");

        out.push_str(&format!(
            "~ {}={}\n",
            name(prop::SWITCH),
            str_y_or_n(self.is_switch)
        ));

        // Pascal tail loop `Rg..NumProperties`.
        dump::generic_props_from(out, cx, self, prop::RG);
    }
}

/// Pascal full-matrix dump: row-major `<v> ` cells, each row trailed by `|`.
fn write_matrix(out: &mut String, n: usize, cell: impl Fn(usize, usize) -> String) {
    for i in 0..n {
        for j in 0..n {
            out.push_str(&cell(i, j));
            out.push(' ');
        }
        out.push('|');
    }
}
