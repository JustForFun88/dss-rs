//! Boundary-record codecs at the frozen ABI offsets
//! (`docs/wasm/USERMODEL_ABI.md` §2.1 and §2.6).
//!
//! The guest decodes two images:
//!
//! * `TDynamicsRec`, 52 B — it reads `h` (offset 0), `IterationFlag` (32) and
//!   `SolutionMode` (36);
//! * `TWindGenVars`, **348 B** — the wasm image of Pascal
//!   `PCElements/WindGenVars.pas:20-73`, i.e. the native 356-byte record minus
//!   the managed `PLoss: string` reference (native offset 244), hole closed. So
//!   the head 0…244 is the `TGeneratorVars` wasm image field-for-field and the
//!   thirteen turbine doubles run 244…348.
//!
//! Only the fields this model touches are named here; the rest of the image is
//! passed through untouched (the host rewrites it before every call anyway).
//!
//! `Complex` is two little-endian f64 (re +0, im +8); complex arrays keep Pascal
//! 1-based semantics, so element k lives at byte offset `(k-1)*16`.

use crate::Cx;

/// `SizeOf(TDynamicsRec)` (probe-frozen, ABI §2.1).
pub const DYNAMICS_REC_SIZE: usize = 52;
/// Size of the `TWindGenVars` **wasm** image (ABI §2.6).
pub const WINDGEN_VARS_SIZE: usize = 348;
/// Bytes per `Complex`.
pub const COMPLEX_SIZE: usize = 16;

// --- TWindGenVars offsets, wasm image (ABI §2.6) ---------------------------
// Head — identical to the TGeneratorVars wasm image (ABI §2.2b / Appendix A).
/// `Pshaft` (`WindGenVars.pas:23`) — written.
pub const OFF_PSHAFT: usize = 8;
/// `Speed` (`:24`) — written.
pub const OFF_SPEED: usize = 16;
/// `w0` (`:25`) — read.
pub const OFF_W0: usize = 24;
/// `Xdp` (`:33`) — read. The engine derives it from `puXdp·kV²·1000/kVArating`
/// (`WindGen.pas:1368-1370`), so echoing it witnesses that wiring end to end.
pub const OFF_XDP: usize = 88;
/// `dSpeed` (`:35`) — written.
pub const OFF_DSPEED: usize = 136;
/// `kVArating` (`:30`) — read.
pub const OFF_KVARATING: usize = 64;
/// `kVWindGenBase` (`:31`, the WindGen spelling of `kVGeneratorBase`) — read.
pub const OFF_KVBASE: usize = 72;
/// `Pnominalperphase` (`:38`) — read.
pub const OFF_PNOMINAL: usize = 160;
/// `Qnominalperphase` (`:39`) — read.
pub const OFF_QNOMINAL: usize = 168;
/// `NumPhases` i32 (`:43`) — read.
pub const OFF_NUMPHASES: usize = 176;
/// `NumConductors` i32 (`:44`) — read.
pub const OFF_NUMCONDS: usize = 180;
/// `Conn` i32 (`:45`, 0 = wye / 1 = delta) — read.
pub const OFF_CONN: usize = 184;
/// `VTarget` (`:52`) — read. It lives in the **unaligned** stretch that follows
/// the integer block (188…244), so echoing it is the witness for that region.
pub const OFF_VTARGET: usize = 212;
// Turbine tail — the block the dropped `PLoss` reference used to precede.
/// `ag`, gearbox ratio (`:60`) — read. **First tail field**: at 244 in the wasm
/// image because the managed `PLoss` reference does not cross; a host that kept
/// the hole would serve this model garbage here.
pub const OFF_AG: usize = 244;
/// `Cp`, turbine performance coefficient (`:61`) — written.
pub const OFF_CP: usize = 252;
/// `Lamda`, tip-speed ratio (`:62`) — written.
pub const OFF_LAMDA: usize = 260;
/// `Poles` (`:63`) — read (the `P=` property).
pub const OFF_POLES: usize = 268;
/// `VCutin` (`:66`) — read (the `VCutin=` property).
pub const OFF_VCUTIN: usize = 292;
/// `Pm`, mechanical power (`:68`) — written.
pub const OFF_PM: usize = 308;
/// `Ps`, stator active power (`:69`) — written.
pub const OFF_PS: usize = 316;
/// `Pr`, rotor active power (`:70`) — written.
pub const OFF_PR: usize = 324;
/// `Pg`, total power output (`:71`) — written.
pub const OFF_PG: usize = 332;
/// `s`, generator slip (`:72`) — written. **Last field** of the image.
pub const OFF_S: usize = 340;

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

/// The `TWindGenVars` fields the model reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct WindGenIn {
    /// `w0`, synchronous speed rad/s.
    pub w0: f64,
    /// `kVArating`.
    pub kva_rating: f64,
    /// `kVWindGenBase`.
    pub kv_base: f64,
    /// `Pnominalperphase`, W.
    pub pnominal: f64,
    /// `Qnominalperphase`, var.
    pub qnominal: f64,
    /// `NumPhases`.
    pub num_phases: i32,
    /// `NumConductors`.
    pub num_conds: i32,
    /// `Conn` (0 = wye, 1 = delta).
    pub conn: i32,
    /// `ag`, gearbox ratio (first turbine-tail field).
    pub ag: f64,
    /// `Xdp`, transient reactance (head double, offset 88).
    pub xdp: f64,
    /// `VTarget`, the unaligned-stretch witness (offset 212).
    pub vtarget: f64,
    /// `Poles` (turbine tail, offset 268).
    pub poles: f64,
    /// `VCutin` (turbine tail, offset 292).
    pub v_cutin: f64,
}

pub fn get_f64(b: &[u8], off: usize) -> f64 {
    f64::from_le_bytes(b[off..off + 8].try_into().expect("8-byte slice"))
}

pub fn get_i32(b: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(b[off..off + 4].try_into().expect("4-byte slice"))
}

pub fn put_f64(b: &mut [u8], off: usize, v: f64) {
    b[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

/// Decode the 52-byte `TDynamicsRec` image.
pub fn decode_dynamics_rec(b: &[u8]) -> DynamicsRec {
    DynamicsRec {
        h: get_f64(b, 0),
        iteration_flag: get_i32(b, 32),
        solution_mode: get_i32(b, 36),
    }
}

/// Decode the fields this model reads from the 348-byte `TWindGenVars` image.
pub fn decode_windgen_vars(b: &[u8]) -> WindGenIn {
    WindGenIn {
        w0: get_f64(b, OFF_W0),
        kva_rating: get_f64(b, OFF_KVARATING),
        kv_base: get_f64(b, OFF_KVBASE),
        pnominal: get_f64(b, OFF_PNOMINAL),
        qnominal: get_f64(b, OFF_QNOMINAL),
        num_phases: get_i32(b, OFF_NUMPHASES),
        num_conds: get_i32(b, OFF_NUMCONDS),
        conn: get_i32(b, OFF_CONN),
        ag: get_f64(b, OFF_AG),
        xdp: get_f64(b, OFF_XDP),
        vtarget: get_f64(b, OFF_VTARGET),
        poles: get_f64(b, OFF_POLES),
        v_cutin: get_f64(b, OFF_VCUTIN),
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
