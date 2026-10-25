//! `TWinding` — one transformer/xfmrcode winding's electrical + tap data.
//! Port of the Pascal `TWinding` record (`PDElements/Transformer.pas`, l.162),
//! shared verbatim by `TTransfObj` (WP4.4) and `TXfmrCodeObj` (WP4.3). It is
//! pure data plus the two small helpers `Init` and `ComputeAntiFloatAdder`; all
//! winding *behavior* (term-ref mapping, YPrim) lives in the transformer.

use crate::util::sqrt3;

/// Pascal transformer/autotransformer winding connection code. `Wye`/`Delta`
/// are the general set (`Transformer`, `XfmrCode`); `Series` (2) is
/// autotransformer-only (`AutoTrans.pas` `AutoTransConnectionEnum`). The
/// discriminants are user-visible and frozen — they round-trip through the
/// `DssEnum` registry (`Set conn=`/`?`/dump) — so `i32` survives only at the
/// property parse/report boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Connection {
    Wye = 0,
    Delta = 1,
    Series = 2,
}

impl Connection {
    /// The connection ordinal (the property `?`/dump boundary value).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// `TWinding.Connection(ordinal)`; out-of-range yields `None`.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Wye),
            1 => Some(Self::Delta),
            2 => Some(Self::Series),
            _ => None,
        }
    }
}

/// Pascal `TransfObj.TermRef`/`AutoTransObj.TermRef`: the `(phase, winding) →
/// conductor pair` map. Replaces the Pascal flat 1-based `array of Integer`
/// (dead slot 0) with one 0-based `[plus, minus]` conductor-index pair per
/// phase × winding, laid out **phase-major** (`pairs[phase * nw + wind]`) — the
/// same visiting order `SetTermRef` fills and `BuildYPrimComponent` / the
/// `TermRef=` dump walk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TermRef(pub Vec<[usize; 2]>);

impl TermRef {
    /// The `[plus, minus]` 0-based conductor pair for 0-based `phase`, `wind`
    /// (`nw` = number of windings; phase-major layout).
    #[inline]
    pub fn pair(&self, phase: usize, wind: usize, nw: usize) -> [usize; 2] {
        self.0[phase * nw + wind]
    }
}

/// One winding's two `Y_Terminal` rows (the Pascal `2·iWind-1`/`2·iWind` pair),
/// 0-based. Derived once per winding.
#[derive(Debug, Clone, Copy)]
pub struct WdgTerms {
    pub plus: usize,
    pub minus: usize,
}

impl WdgTerms {
    /// The two `Y_Terminal` rows for 0-based winding `iwind` (Pascal
    /// `2·iWind-1`/`2·iWind`, 1-based → `2·iwind`/`2·iwind+1`, 0-based).
    #[inline]
    pub fn of(iwind: usize) -> Self {
        Self {
            plus: 2 * iwind,
            minus: 2 * iwind + 1,
        }
    }
}

/// Pascal `TWinding`. Fields keep the Pascal names (snake-cased); 0-based here
/// only in that the owning array is 0-based — the winding's own data is flat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Winding {
    /// Pascal `Connection` (0 = wye, 1 = delta, 2 = series [auto only]).
    pub connection: Connection,
    /// Pascal `kVLL` — for 2- and 3-phase always kV line-line, else actual kV.
    pub kvll: f64,
    /// Pascal `VBase` — base winding voltage (volts), derived from `kVLL`.
    pub vbase: f64,
    pub kva: f64,
    /// Pascal `puTap` — present tap, per unit.
    pub putap: f64,
    /// Pascal `Rpu` — winding resistance on the transformer MVA base.
    pub rpu: f64,
    /// Pascal `Rdcpu` — DC resistance pu (GIC); defaults to 85 % of `Rpu`.
    pub rdcpu: f64,
    pub rdcohms: f64,
    /// Pascal `Rneut` — neutral grounding resistance (ohms; < 0 = open).
    pub rneut: f64,
    pub xneut: f64,
    /// Pascal `Y_PPM` — anti-float reactance adder.
    pub y_ppm: f64,
    /// Pascal `RdcSpecified` — whether `RdcOhms` was set explicitly.
    pub rdc_specified: bool,

    // Tap-changer data
    pub tap_increment: f64,
    pub min_tap: f64,
    pub max_tap: f64,
    pub num_taps: i32,
}

impl Default for Winding {
    fn default() -> Self {
        Self::new()
    }
}

impl Winding {
    /// Pascal `TWinding.Init` — make a new winding with the default 12.47 kV /
    /// 1000 kVA wye winding values.
    pub fn new() -> Self {
        let kvll = 12.47;
        let kva = 1000.0;
        let rpu = 0.002;
        let rdcpu = rpu * 0.85;
        let vbase = kvll / sqrt3() * 1000.0;
        let mut w = Self {
            connection: Connection::Wye,
            kvll,
            vbase,
            kva,
            putap: 1.0,
            rpu,
            rdcpu,
            // Pascal: RdcOhms = Sqr(kVLL) / (kVA / 1000) * Rdcpu.
            rdcohms: kvll * kvll / (kva / 1000.0) * rdcpu,
            rdc_specified: false,
            rneut: -1.0, // open — force the user to specify a connection
            xneut: 0.0,
            y_ppm: 0.0,
            tap_increment: 0.00625,
            min_tap: 0.90,
            max_tap: 1.10,
            num_taps: 32,
        };
        // Pascal Init: ComputeAntiFloatAdder(1.0e-6, kVA / 3 / 1000).
        w.compute_anti_float_adder(1.0e-6, kva / 3.0 / 1000.0);
        w
    }

    /// Pascal `TWinding.ComputeAntiFloatAdder`: put half the anti-float adder on
    /// each terminal of the winding.
    pub fn compute_anti_float_adder(&mut self, ppm_factor: f64, vabase_1ph: f64) {
        // Y_PPM := -PPM_Factor / (SQR(VBase) / VABase1ph) / 2.0
        self.y_ppm = -ppm_factor / (self.vbase * self.vbase / vabase_1ph) / 2.0;
    }
}

/// The input a refused Transformer or AutoTrans recalc names: for a kVA
/// rating the winding 1 `kVA` it derives from, else the first winding `kV`,
/// `kVA`, `Tap`, `%R` or typed `RDCOhms` that is not a finite number, else the
/// first of `others` (`name=value` texts with whether the value is finite)
/// that is not, else the first winding of zero `kV`, then of zero `kVA`. An
/// overflow of finite inputs names, for the admittance, the `kV` of a winding
/// whose voltage base squared overflows, the `%R` of a winding whose derived
/// dc resistance overflows before its quotient, winding 1's `kVA` when the VA
/// base (`kVA` times 1000) or a quotient by it overflows, and the `Tap` of a
/// winding whose voltage times its tap has no finite reciprocal while its
/// voltage alone has one, then the `kV` of a winding whose voltage base squared
/// is below the normal range, the `Tap` of one whose voltage times its tap
/// squared is, and winding 1's `kVA` when the one-volt impedance base
/// `phases / VA` squared is, else winding 1's `kV`, the divisor of every
/// current rating. The dc resistance names its overflow as the admittance does.
/// `windings` are the recalc's trial, with their derived voltage bases
/// and resistances. A winding after the first is named with its `wdg=`.
pub(crate) fn winding_refused_input(
    windings: &[Winding],
    nphases: usize,
    others: &[(String, bool)],
    quantity: &str,
) -> String {
    use super::input_text;
    let named = |i: usize, name: &str, v: f64| {
        if i == 0 {
            input_text(name, v)
        } else {
            format!("wdg={} {}", i + 1, input_text(name, v))
        }
    };
    if quantity.ends_with("kVA rating") {
        return named(0, "kVA", windings[0].kva);
    }
    for (i, w) in windings.iter().enumerate() {
        let typed_rdc = w.rdc_specified.then_some(("RDCOhms", w.rdcohms));
        let inputs = [
            ("kV", w.kvll),
            ("kVA", w.kva),
            ("Tap", w.putap),
            ("%R", w.rpu),
        ];
        for (name, v) in inputs.into_iter().chain(typed_rdc) {
            if !v.is_finite() {
                return named(i, name, v);
            }
        }
    }
    if let Some((text, _)) = others.iter().find(|(_, finite)| !finite) {
        return text.clone();
    }
    if let Some(i) = windings.iter().position(|w| w.kvll == 0.0) {
        return named(i, "kV", windings[i].kvll);
    }
    if let Some(i) = windings.iter().position(|w| w.kva == 0.0) {
        return named(i, "kVA", windings[i].kva);
    }
    if quantity == "admittance" || quantity == "dc resistance" {
        // Per unit of winding 1's kVA times 1000: the impedance base divides
        // by it, and so does a dc resistance derived from the `%R`.
        if let Some(i) = windings
            .iter()
            .position(|w| !(w.vbase * w.vbase).is_finite())
        {
            return named(i, "kV", windings[i].kvll);
        }
        let derived_rdc = |w: &Winding| w.rdcpu * (w.vbase * w.vbase);
        if let Some(i) = windings
            .iter()
            .position(|w| !w.rdc_specified && !derived_rdc(w).is_finite())
        {
            return named(i, "%R", windings[i].rpu / 0.01);
        }
        let va = windings[0].kva * 1000.0;
        let by_va = |x: f64| (x / va).is_finite();
        if !va.is_finite()
            || !by_va(nphases as f64)
            || windings
                .iter()
                .any(|w| !w.rdc_specified && !by_va(derived_rdc(w)))
        {
            return named(0, "kVA", windings[0].kva);
        }
        if let Some(i) = windings
            .iter()
            .position(|w| (1.0 / w.vbase).is_finite() && !(1.0 / (w.vbase * w.putap)).is_finite())
        {
            return named(i, "Tap", windings[i].putap);
        }
        // The admittance divides by each winding's voltage times its tap,
        // squared, and the short-circuit impedance is inverted on the one-volt
        // impedance base `phases / VA`, so it grows with winding 1's kVA. A
        // square below the normal range is the one the quotient overflows on.
        let tiny = |x: f64| x * x < f64::MIN_POSITIVE;
        if let Some(i) = windings.iter().position(|w| tiny(w.vbase)) {
            return named(i, "kV", windings[i].kvll);
        }
        if let Some(i) = windings.iter().position(|w| tiny(w.vbase * w.putap)) {
            return named(i, "Tap", windings[i].putap);
        }
        if tiny(nphases as f64 / va) {
            return named(0, "kVA", windings[0].kva);
        }
    }
    // Every current rating divides by winding 1's kV.
    named(0, "kV", windings[0].kvll)
}

/// The input of winding `i` that leaves it no dc resistance, a short at dc:
/// its typed `RDCOhms`, else the `%R` its dc resistance derives from, with
/// its `wdg=` after the first winding.
pub(crate) fn dc_short_input(windings: &[Winding], i: usize) -> String {
    use super::input_text;
    let w = &windings[i];
    let text = if w.rdc_specified {
        input_text("RDCOhms", w.rdcohms)
    } else {
        input_text("%R", w.rpu / 0.01)
    };
    if i == 0 {
        text
    } else {
        format!("wdg={} {text}", i + 1)
    }
}

/// The kVA ratings a transformer derives from winding 1's `kva` (110 % and
/// 150 %), each `None` where it is not a finite number: such a rating is not
/// committed and the recalc refuses the transformer.
pub(crate) fn hkva_from(kva: f64) -> [Option<f64>; 2] {
    [1.1 * kva, 1.5 * kva].map(|v| v.is_finite().then_some(v))
}

/// Which derived quantity of a transformer is not a finite number, in the
/// order a refusal names it: the kVA ratings (`hkva_overflow`), the current
/// ratings, then the admittance: the short-circuit reactances `xsc`, the
/// per-winding data and the matrices built from them (a reactance that is
/// not a number is named here before the singular short-circuit impedance it
/// also makes), then the dc resistance, typed or derived from the `%R`,
/// which only the dc model reads.
pub(crate) fn transformer_not_finite(
    hkva_overflow: [bool; 2],
    ratings: &[crate::obj::Rating],
    seasonal: &[crate::obj::Rating],
    windings: &[Winding],
    xsc: &[f64],
    admittances: &[&crate::support::cmatrix::CMatrix],
) -> Option<&'static str> {
    let set_finite =
        |r: &crate::obj::Rating| !matches!(r, crate::obj::Rating::Set(v) if !v.is_finite());
    let quantities = ["normal current rating", "emergency current rating"];
    if hkva_overflow[0] {
        return Some("normal kVA rating");
    }
    if hkva_overflow[1] {
        return Some("emergency kVA rating");
    }
    if let Some(i) = ratings.iter().position(|r| !set_finite(r)) {
        return Some(quantities[i]);
    }
    if !seasonal.iter().all(set_finite) {
        return Some("seasonal current ratings");
    }
    let windings_finite = windings
        .iter()
        .all(|w| [w.rpu, w.vbase, w.y_ppm].iter().all(|v| v.is_finite()))
        && xsc.iter().all(|v| v.is_finite());
    let matrices_finite = admittances.iter().all(|m| {
        m.values()
            .iter()
            .all(|v| v.re.is_finite() && v.im.is_finite())
    });
    if !(windings_finite && matrices_finite) {
        return Some("admittance");
    }
    let dc_finite = windings
        .iter()
        .all(|w| w.rdcpu.is_finite() && w.rdcohms.is_finite());
    (!dc_finite).then_some("dc resistance")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_defaults_match_pascal() {
        let w = Winding::new();
        assert_eq!(w.connection, Connection::Wye);
        assert_eq!(w.kvll, 12.47);
        assert!((w.vbase - 12.47 / 3.0_f64.sqrt() * 1000.0).abs() < 1e-9);
        assert_eq!(w.kva, 1000.0);
        assert_eq!(w.putap, 1.0);
        assert_eq!(w.rpu, 0.002);
        assert!((w.rdcpu - 0.002 * 0.85).abs() < 1e-15);
        // RdcOhms = Sqr(kVLL)/(kVA/1000)*Rdcpu (the oracle dumps 0.26435153).
        assert!((w.rdcohms - 0.264_351_53).abs() < 1e-6);
        assert_eq!(w.rneut, -1.0);
        assert_eq!(w.num_taps, 32);
        assert_eq!(w.max_tap, 1.10);
        assert_eq!(w.min_tap, 0.90);
    }

    #[test]
    fn anti_float_adder_sign_and_half() {
        let mut w = Winding::new();
        w.compute_anti_float_adder(1.0e-6, w.vbase * w.vbase); // VABase1ph = VBase²
        // Y_PPM = -1e-6 / (VBase²/VBase²) / 2 = -5e-7.
        assert!((w.y_ppm + 0.5e-6).abs() < 1e-18);
    }
}
