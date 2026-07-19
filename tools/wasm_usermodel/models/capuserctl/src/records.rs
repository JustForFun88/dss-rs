//! Boundary record codecs at the frozen ABI offsets
//! (`docs/wasm/USERMODEL_ABI.md` §2), shared by the wasm and native-twin
//! boundaries. The model reads the `TDynamicsRec` (52 B) for the schedule time
//! (`t` at offset 8, `intHour` at offset 40) and decodes `Complex` node voltages
//! (two little-endian f64, re +0 / im +8).

use crate::Cx;

/// `SizeOf(TDynamicsRec)` (probe-frozen, ABI §2.1).
pub const DYNAMICS_REC_SIZE: usize = 52;
/// Bytes per `Complex`.
pub const COMPLEX_SIZE: usize = 16;

/// The subset of `TDynamicsRec` the model reads (schedule time).
#[derive(Clone, Copy, Debug, Default)]
pub struct DynamicsRec {
    /// Seconds from top of hour (offset 8).
    pub t: f64,
    /// `intHour` (offset 40).
    pub int_hour: i32,
}

fn get_f64(b: &[u8], off: usize) -> f64 {
    f64::from_le_bytes(b[off..off + 8].try_into().expect("8-byte slice"))
}

fn get_i32(b: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(b[off..off + 4].try_into().expect("4-byte slice"))
}

/// Decode the schedule time from a 52-byte `TDynamicsRec` image.
pub fn decode_dynamics_rec(b: &[u8]) -> DynamicsRec {
    DynamicsRec {
        t: get_f64(b, 8),
        int_hour: get_i32(b, 40),
    }
}

/// Read `Complex` k (1-based) from a packed complex-array image (byte offset
/// `(k-1)*16`).
pub fn read_complex(buf: &[u8], k: usize) -> Cx {
    let off = (k - 1) * COMPLEX_SIZE;
    Cx::new(get_f64(buf, off), get_f64(buf, off + 8))
}
