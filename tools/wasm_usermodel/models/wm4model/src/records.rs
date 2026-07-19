//! Boundary record codecs at the frozen ABI offsets
//! (`docs/wasm/USERMODEL_ABI.md` §2), shared by the wasm and native-twin
//! boundaries. The model only reads `TDynamicsRec` (52 B) — it needs `h`
//! (offset 0), `IterationFlag` (32) and `SolutionMode` (36). `Complex` is two
//! little-endian f64 (re +0, im +8); arrays keep Pascal 1-based semantics (the
//! host copies from index 1, so element k lives at byte offset `(k-1)*16`).

use crate::Cx;

/// `SizeOf(TDynamicsRec)` (probe-frozen, ABI §2.1).
pub const DYNAMICS_REC_SIZE: usize = 52;
/// Bytes per `Complex`.
pub const COMPLEX_SIZE: usize = 16;

/// The subset of `TDynamicsRec` the model reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct DynamicsRec {
    /// Dynamics step, s (offset 0).
    pub h: f64,
    /// 0 = new step, 1 = same step (offset 32).
    pub iteration_flag: i32,
    /// `TSolveMode` ordinal (offset 36); DYNAMICMODE = 14.
    pub solution_mode: i32,
}

fn get_f64(b: &[u8], off: usize) -> f64 {
    f64::from_le_bytes(b[off..off + 8].try_into().expect("8-byte slice"))
}

fn get_i32(b: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(b[off..off + 4].try_into().expect("4-byte slice"))
}

/// Decode the 52-byte `TDynamicsRec` image.
pub fn decode_dynamics_rec(b: &[u8]) -> DynamicsRec {
    DynamicsRec {
        h: get_f64(b, 0),
        iteration_flag: get_i32(b, 32),
        solution_mode: get_i32(b, 36),
    }
}

/// Read `Complex` k (1-based) from a packed complex-array image.
pub fn read_complex(buf: &[u8], k: usize) -> Cx {
    let off = (k - 1) * COMPLEX_SIZE;
    Cx::new(get_f64(buf, off), get_f64(buf, off + 8))
}

/// Write `Complex` k (1-based) into a packed complex-array image.
pub fn write_complex(buf: &mut [u8], k: usize, c: Cx) {
    let off = (k - 1) * COMPLEX_SIZE;
    buf[off..off + 8].copy_from_slice(&c.re.to_le_bytes());
    buf[off + 8..off + 16].copy_from_slice(&c.im.to_le_bytes());
}
