//! Boundary record images — decoded/encoded at the byte offsets FROZEN in
//! `docs/wasm/USERMODEL_ABI.md` §2.1 (`TDynamicsRec`, 52 B) and §2.2
//! (`TGeneratorVars`, 244 B). Offsets were probe-derived (packed, little-
//! endian; `docs/wasm/probes/p2_offsets_*.txt`) — never recomputed here, and
//! never mapped via `#[repr(C)]` (which would pad the unaligned tail after
//! the three i32s at 176/180/184).
//!
//! Pascal spec: `Shared/Dynamics.pas:40-52` (r3723) / `PCElements/
//! GeneratorVars.pas` — the records the vendored `IndMach012a.dpr` shares
//! with the host by reference; over WASM they arrive as per-call refreshed
//! buffer images (ABI §2).

use crate::cmath::Complex;

/// Pascal `TDynamicsRec` (`Shared/Dynamics.pas:40-52`; ABI §2.1).
#[derive(Clone, Copy, Debug, Default)]
pub struct DynamicsRec {
    pub h: f64,              // +0
    pub t: f64,              // +8
    pub tstart: f64,         // +16
    pub tstop: f64,          // +24
    pub iteration_flag: i32, // +32  0 = new (predictor) step, 1 = corrector
    pub solution_mode: i32,  // +36  TSolveMode {$Z4}
    pub int_hour: i32,       // +40
    pub dbl_hour: f64,       // +44
}

/// Pascal `Dynamics.pas:31` — `DYNAMICMODE = 14`.
pub const DYNAMICMODE: i32 = 14;

/// Byte size of the `TDynamicsRec` image (probe: 52).
pub const DYNAMICS_REC_SIZE: usize = 52;

/// Pascal `TGeneratorVars` (`PCElements/GeneratorVars.pas` r3723 ==
/// dss_capi 0.14.5 `generator.pas:178-214`; ABI §2.2, 244 bytes).
#[derive(Clone, Copy, Debug, Default)]
pub struct GeneratorVars {
    pub theta: f64,              // +0
    pub pshaft: f64,             // +8
    pub speed: f64,              // +16
    pub w0: f64,                 // +24
    pub hmass: f64,              // +32
    pub mmass: f64,              // +40
    pub d: f64,                  // +48
    pub dpu: f64,                // +56
    pub kva_rating: f64,         // +64
    pub kv_generator_base: f64,  // +72
    pub xd: f64,                 // +80
    pub xdp: f64,                // +88
    pub xdpp: f64,               // +96
    pub pu_xd: f64,              // +104
    pub pu_xdp: f64,             // +112
    pub pu_xdpp: f64,            // +120
    pub d_theta: f64,            // +128
    pub d_speed: f64,            // +136
    pub theta_history: f64,      // +144
    pub speed_history: f64,      // +152
    pub pnominal_per_phase: f64, // +160
    pub qnominal_per_phase: f64, // +168
    pub num_phases: i32,         // +176
    pub num_conductors: i32,     // +180
    pub conn: i32,               // +184
    pub vthev_mag: f64,          // +188  (unaligned tail starts here)
    pub vthev_harm: f64,         // +196
    pub theta_harm: f64,         // +204
    pub vtarget: f64,            // +212
    pub zthev_re: f64,           // +220
    pub zthev_im: f64,           // +228
    pub xrdp: f64,               // +236
}

/// Byte size of the `TGeneratorVars` image (probe: 244).
pub const GENERATOR_VARS_SIZE: usize = 244;

fn rd_f64(buf: &[u8], off: usize) -> f64 {
    f64::from_le_bytes(buf[off..off + 8].try_into().unwrap())
}

fn rd_i32(buf: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

fn wr_f64(buf: &mut [u8], off: usize, v: f64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

fn wr_i32(buf: &mut [u8], off: usize, v: i32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

pub fn decode_dynamics_rec(buf: &[u8]) -> DynamicsRec {
    DynamicsRec {
        h: rd_f64(buf, 0),
        t: rd_f64(buf, 8),
        tstart: rd_f64(buf, 16),
        tstop: rd_f64(buf, 24),
        iteration_flag: rd_i32(buf, 32),
        solution_mode: rd_i32(buf, 36),
        int_hour: rd_i32(buf, 40),
        dbl_hour: rd_f64(buf, 44),
    }
}

pub fn decode_generator_vars(buf: &[u8]) -> GeneratorVars {
    GeneratorVars {
        theta: rd_f64(buf, 0),
        pshaft: rd_f64(buf, 8),
        speed: rd_f64(buf, 16),
        w0: rd_f64(buf, 24),
        hmass: rd_f64(buf, 32),
        mmass: rd_f64(buf, 40),
        d: rd_f64(buf, 48),
        dpu: rd_f64(buf, 56),
        kva_rating: rd_f64(buf, 64),
        kv_generator_base: rd_f64(buf, 72),
        xd: rd_f64(buf, 80),
        xdp: rd_f64(buf, 88),
        xdpp: rd_f64(buf, 96),
        pu_xd: rd_f64(buf, 104),
        pu_xdp: rd_f64(buf, 112),
        pu_xdpp: rd_f64(buf, 120),
        d_theta: rd_f64(buf, 128),
        d_speed: rd_f64(buf, 136),
        theta_history: rd_f64(buf, 144),
        speed_history: rd_f64(buf, 152),
        pnominal_per_phase: rd_f64(buf, 160),
        qnominal_per_phase: rd_f64(buf, 168),
        num_phases: rd_i32(buf, 176),
        num_conductors: rd_i32(buf, 180),
        conn: rd_i32(buf, 184),
        vthev_mag: rd_f64(buf, 188),
        vthev_harm: rd_f64(buf, 196),
        theta_harm: rd_f64(buf, 204),
        vtarget: rd_f64(buf, 212),
        zthev_re: rd_f64(buf, 220),
        zthev_im: rd_f64(buf, 228),
        xrdp: rd_f64(buf, 236),
    }
}

/// Write the full record image back (the DLL contract lets the model mutate
/// any field; IndMach012a writes `Speed` — ABI §2 requires unconditional
/// read-back host-side, so the guest re-encodes the whole image).
pub fn encode_generator_vars(gv: &GeneratorVars, buf: &mut [u8]) {
    wr_f64(buf, 0, gv.theta);
    wr_f64(buf, 8, gv.pshaft);
    wr_f64(buf, 16, gv.speed);
    wr_f64(buf, 24, gv.w0);
    wr_f64(buf, 32, gv.hmass);
    wr_f64(buf, 40, gv.mmass);
    wr_f64(buf, 48, gv.d);
    wr_f64(buf, 56, gv.dpu);
    wr_f64(buf, 64, gv.kva_rating);
    wr_f64(buf, 72, gv.kv_generator_base);
    wr_f64(buf, 80, gv.xd);
    wr_f64(buf, 88, gv.xdp);
    wr_f64(buf, 96, gv.xdpp);
    wr_f64(buf, 104, gv.pu_xd);
    wr_f64(buf, 112, gv.pu_xdp);
    wr_f64(buf, 120, gv.pu_xdpp);
    wr_f64(buf, 128, gv.d_theta);
    wr_f64(buf, 136, gv.d_speed);
    wr_f64(buf, 144, gv.theta_history);
    wr_f64(buf, 152, gv.speed_history);
    wr_f64(buf, 160, gv.pnominal_per_phase);
    wr_f64(buf, 168, gv.qnominal_per_phase);
    wr_i32(buf, 176, gv.num_phases);
    wr_i32(buf, 180, gv.num_conductors);
    wr_i32(buf, 184, gv.conn);
    wr_f64(buf, 188, gv.vthev_mag);
    wr_f64(buf, 196, gv.vthev_harm);
    wr_f64(buf, 204, gv.theta_harm);
    wr_f64(buf, 212, gv.vtarget);
    wr_f64(buf, 220, gv.zthev_re);
    wr_f64(buf, 228, gv.zthev_im);
    wr_f64(buf, 236, gv.xrdp);
}

/// Read a 1-based `pComplexArray` element k image at `(k-1)*16` (ABI §2.3).
pub fn read_complex(buf: &[u8], k: usize) -> Complex {
    let off = (k - 1) * 16;
    Complex {
        re: rd_f64(buf, off),
        im: rd_f64(buf, off + 8),
    }
}

/// Write a 1-based `pComplexArray` element k image at `(k-1)*16` (ABI §2.3).
pub fn write_complex(buf: &mut [u8], k: usize, v: Complex) {
    let off = (k - 1) * 16;
    wr_f64(buf, off, v.re);
    wr_f64(buf, off + 8, v.im);
}
