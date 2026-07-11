//! Pascal `TSpectrumObj.DumpProperties` (`General/Spectrum.pas:326`) — the
//! `Dump spectrum.…` override. A plain `TDSSObject` (no `! ENABLED`): after the
//! inherited `New "…"` header, all properties dump generically; Complete adds
//! the `Multiplier Array:` header + per-harmonic `Harmonic, Mult.re, Mult.im,
//! Mag,  Angle` row (bare `%-g` fields — the 15-sig-fig convention, see
//! `crate::report::format::g` — of `MultArray`, the phase-shifted complex
//! multiplier the harmonic solution consumes via `get_mult`).

use crate::report::format::g;
use crate::report::save::dump::{self, DumpCtx};
use crate::support::complexutil::cdang;

use super::SpectrumObj;

impl SpectrumObj {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        dump::header(out, &dump::full_name(cx, self));
        dump::generic_props(out, cx, self);

        if !complete {
            return;
        }

        out.push_str("Multiplier Array:\n");
        out.push_str("Harmonic, Mult.re, Mult.im, Mag,  Angle\n");
        if let (Some(harm), Some(mult)) = (&self.harm_array, &self.mult_array) {
            for i in 0..self.num_harm.max(0) as usize {
                let c = mult[i];
                out.push_str(&format!(
                    "{}, {}, {}, {}, {}\n",
                    g(harm[i], 15),
                    g(c.re, 15),
                    g(c.im, 15),
                    g(c.norm(), 15),
                    g(cdang(c), 15)
                ));
            }
        }
    }
}
