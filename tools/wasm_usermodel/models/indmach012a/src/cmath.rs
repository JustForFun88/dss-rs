//! Complex arithmetic — loop-for-loop port of the r3723 `Shared/Ucomplex.pas`
//! unit the vendored `IndMach012a.dpr` compiles against
//! (`.inputs/electricdss-code-r3723-trunk/Version8/Source/Shared/Ucomplex.pas`).
//!
//! NOTE: this is deliberately NOT dss-core's FPC-RTL-faithful helper set
//! (`cdiv_fpc` Smith division etc.). The native twin links *this* Ucomplex
//! unit, whose `CDIV` is the naive textbook formula (`Ucomplex.pas:183-190`)
//! and whose `Cabs` is the naive `SQRT(re*re+im*im)` (`:78-81`). Bit-parity
//! with the twin requires porting exactly these bodies.

/// Pascal `Ucomplex.complex` (`Ucomplex.pas:13-15`): two f64, re then im.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

/// Pascal `Ucomplex.CZERO` (initialized `cmplx(0,0)`).
pub const CZERO: Complex = Complex { re: 0.0, im: 0.0 };
/// Pascal `Ucomplex.CONE` (initialized `cmplx(1,0)`).
pub const CONE: Complex = Complex { re: 1.0, im: 0.0 };

/// Pascal `Ucomplex.CMPLX` (`Ucomplex.pas:63-67`).
pub const fn cmplx(a: f64, b: f64) -> Complex {
    Complex { re: a, im: b }
}

/// Pascal `Ucomplex.CInv` (`Ucomplex.pas:69-76`).
pub fn cinv(a: Complex) -> Complex {
    let dnom = a.re * a.re + a.im * a.im;
    Complex {
        re: a.re / dnom,
        im: (-a.im) / dnom,
    }
}

/// Pascal `Ucomplex.Cabs` (`Ucomplex.pas:78-81`) — naive `SQRT(re²+im²)`,
/// NOT `hypot`.
pub fn cabs(a: Complex) -> f64 {
    (a.re * a.re + a.im * a.im).sqrt()
}

/// Pascal `Ucomplex.Conjg` (`Ucomplex.pas:88-92`).
pub fn conjg(a: Complex) -> Complex {
    Complex {
        re: a.re,
        im: -a.im,
    }
}

/// Pascal `Ucomplex.CADD` (`Ucomplex.pas:137-141`).
pub fn cadd(a: Complex, b: Complex) -> Complex {
    Complex {
        re: a.re + b.re,
        im: a.im + b.im,
    }
}

/// Pascal `Ucomplex.CACCUM` (`Ucomplex.pas:143-147`).
pub fn caccum(a: &mut Complex, b: Complex) {
    a.re += b.re;
    a.im += b.im;
}

/// Pascal `Ucomplex.CSUB` (`Ucomplex.pas:159-163`).
pub fn csub(a: Complex, b: Complex) -> Complex {
    Complex {
        re: a.re - b.re,
        im: a.im - b.im,
    }
}

/// Pascal `Ucomplex.CMUL` (`Ucomplex.pas:165-169`).
pub fn cmul(a: Complex, b: Complex) -> Complex {
    Complex {
        re: a.re * b.re - a.im * b.im,
        im: a.re * b.im + a.im * b.re,
    }
}

/// Pascal `Ucomplex.cmulreal` (`Ucomplex.pas:171-175`).
pub fn cmulreal(a: Complex, b: f64) -> Complex {
    Complex {
        re: a.re * b,
        im: a.im * b,
    }
}

/// Pascal `Ucomplex.CDIV` (`Ucomplex.pas:183-190`) — naive formula
/// (single shared denominator), NOT Smith's algorithm.
pub fn cdiv(a: Complex, b: Complex) -> Complex {
    let dnom = b.re * b.re + b.im * b.im;
    Complex {
        re: (a.re * b.re + a.im * b.im) / dnom,
        im: (a.im * b.re - a.re * b.im) / dnom,
    }
}

/// Pascal `Ucomplex.cdivreal` (`Ucomplex.pas:192-196`).
pub fn cdivreal(a: Complex, b: f64) -> Complex {
    Complex {
        re: a.re / b,
        im: a.im / b,
    }
}

/// Pascal `Ucomplex.cnegate` (`Ucomplex.pas:198-203`).
pub fn cnegate(a: Complex) -> Complex {
    Complex {
        re: -a.re,
        im: -a.im,
    }
}

/// Pascal `Sqr` intrinsic (x*x).
pub fn sqr(x: f64) -> f64 {
    x * x
}
