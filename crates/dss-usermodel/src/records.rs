//! The record shuttle: Rust mirrors of the packed Pascal boundary records and
//! their byte codecs at the frozen ABI offsets.
//!
//! Layouts are the **probe-frozen** tables of `docs/wasm/USERMODEL_ABI.md` §2
//! (transcribed from FPC probe output, `docs/wasm/probes/p2_offsets_dss_capi.txt`)
//! — little-endian, packed, no padding anywhere. Per the ABI doc §2.2 note, the
//! images are assembled **field-by-field at explicit offsets**, never via
//! `#[repr(C)]` (which would pad the unaligned `TGeneratorVars` tail).

/// Little-endian field writers/readers over a fixed-size image.
macro_rules! put_f64 {
    ($buf:expr, $off:expr, $v:expr) => {
        $buf[$off..$off + 8].copy_from_slice(&f64::to_le_bytes($v))
    };
}
macro_rules! put_i32 {
    ($buf:expr, $off:expr, $v:expr) => {
        $buf[$off..$off + 4].copy_from_slice(&i32::to_le_bytes($v))
    };
}
macro_rules! get_f64 {
    ($buf:expr, $off:expr) => {
        f64::from_le_bytes($buf[$off..$off + 8].try_into().expect("8-byte slice"))
    };
}
macro_rules! get_i32 {
    ($buf:expr, $off:expr) => {
        i32::from_le_bytes($buf[$off..$off + 4].try_into().expect("4-byte slice"))
    };
}

/// Pascal `TDynamicsRec` (`Shared/Dynamics.pas:42-52`) — the dynamics record
/// shared with the model by reference; the model reads mode/step and may
/// mutate. Image: 52 bytes packed (ABI doc §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DynamicsRec {
    /// Dynamics step, s (offset 0).
    pub h: f64,
    /// Seconds from top of hour (offset 8).
    pub t: f64,
    /// Offset 16.
    pub tstart: f64,
    /// Offset 24.
    pub tstop: f64,
    /// 0 = new step, 1 = same step (predictor/corrector) (offset 32).
    pub iteration_flag: i32,
    /// Pascal `TSolveMode` (`{$Z4}` = int32, values 0..17; DYNAMICMODE = 14)
    /// (offset 36).
    pub solution_mode: i32,
    /// Offset 40.
    pub int_hour: i32,
    /// Offset 44.
    pub dbl_hour: f64,
}

impl DynamicsRec {
    /// Size of the packed image (probe: `SizeOf(TDynamicsRec) = 52`).
    pub const SIZE: usize = 52;

    /// Serialize to the packed little-endian image.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut b = [0u8; Self::SIZE];
        put_f64!(b, 0, self.h);
        put_f64!(b, 8, self.t);
        put_f64!(b, 16, self.tstart);
        put_f64!(b, 24, self.tstop);
        put_i32!(b, 32, self.iteration_flag);
        put_i32!(b, 36, self.solution_mode);
        put_i32!(b, 40, self.int_hour);
        put_f64!(b, 44, self.dbl_hour);
        b
    }

    /// Deserialize from the packed little-endian image.
    pub fn from_bytes(b: &[u8; Self::SIZE]) -> Self {
        Self {
            h: get_f64!(b, 0),
            t: get_f64!(b, 8),
            tstart: get_f64!(b, 16),
            tstop: get_f64!(b, 24),
            iteration_flag: get_i32!(b, 32),
            solution_mode: get_i32!(b, 36),
            int_hour: get_i32!(b, 40),
            dbl_hour: get_f64!(b, 44),
        }
    }
}

/// Pascal `TGeneratorVars` (`PCElements/generator.pas:178-214`, dss_capi
/// 0.14.5; byte-identical to r3723 `GeneratorVars.pas`) — the Generator's
/// public data record. The model **mutates** it (e.g. sets `Pshaft`, `Speed`,
/// `dSpeed`); the host reads it back after every call. Image: 244 bytes packed
/// (ABI doc §2.2), including the deliberately unaligned tail (`vthev_mag` at
/// 188 after the three i32s).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GeneratorVars {
    /// Offset 0.
    pub theta: f64,
    /// Offset 8.
    pub pshaft: f64,
    /// Offset 16.
    pub speed: f64,
    /// Offset 24.
    pub w0: f64,
    /// Offset 32.
    pub hmass: f64,
    /// Offset 40.
    pub mmass: f64,
    /// Offset 48.
    pub d: f64,
    /// Offset 56.
    pub dpu: f64,
    /// Offset 64.
    pub kva_rating: f64,
    /// Offset 72.
    pub kv_generator_base: f64,
    /// Offset 80.
    pub xd: f64,
    /// Offset 88.
    pub xdp: f64,
    /// Offset 96.
    pub xdpp: f64,
    /// Offset 104.
    pub pu_xd: f64,
    /// Offset 112.
    pub pu_xdp: f64,
    /// Offset 120.
    pub pu_xdpp: f64,
    /// Offset 128.
    pub dtheta: f64,
    /// Offset 136.
    pub dspeed: f64,
    /// Offset 144.
    pub theta_history: f64,
    /// Offset 152.
    pub speed_history: f64,
    /// Offset 160.
    pub pnominalperphase: f64,
    /// Offset 168.
    pub qnominalperphase: f64,
    /// Offset 176.
    pub num_phases: i32,
    /// Offset 180.
    pub num_conductors: i32,
    /// Offset 184. 0 = wye, 1 = delta.
    pub conn: i32,
    /// Offset 188 (unaligned tail begins).
    pub vthev_mag: f64,
    /// Offset 196.
    pub vthev_harm: f64,
    /// Offset 204.
    pub theta_harm: f64,
    /// Offset 212.
    pub vtarget: f64,
    /// Offset 220 (re), 228 (im).
    pub zthev: (f64, f64),
    /// Offset 236.
    pub xrdp: f64,
}

impl GeneratorVars {
    /// Size of the packed image (probe: `SizeOf(TGeneratorVars) = 244`).
    pub const SIZE: usize = 244;

    /// Serialize to the packed little-endian image.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut b = [0u8; Self::SIZE];
        put_f64!(b, 0, self.theta);
        put_f64!(b, 8, self.pshaft);
        put_f64!(b, 16, self.speed);
        put_f64!(b, 24, self.w0);
        put_f64!(b, 32, self.hmass);
        put_f64!(b, 40, self.mmass);
        put_f64!(b, 48, self.d);
        put_f64!(b, 56, self.dpu);
        put_f64!(b, 64, self.kva_rating);
        put_f64!(b, 72, self.kv_generator_base);
        put_f64!(b, 80, self.xd);
        put_f64!(b, 88, self.xdp);
        put_f64!(b, 96, self.xdpp);
        put_f64!(b, 104, self.pu_xd);
        put_f64!(b, 112, self.pu_xdp);
        put_f64!(b, 120, self.pu_xdpp);
        put_f64!(b, 128, self.dtheta);
        put_f64!(b, 136, self.dspeed);
        put_f64!(b, 144, self.theta_history);
        put_f64!(b, 152, self.speed_history);
        put_f64!(b, 160, self.pnominalperphase);
        put_f64!(b, 168, self.qnominalperphase);
        put_i32!(b, 176, self.num_phases);
        put_i32!(b, 180, self.num_conductors);
        put_i32!(b, 184, self.conn);
        put_f64!(b, 188, self.vthev_mag);
        put_f64!(b, 196, self.vthev_harm);
        put_f64!(b, 204, self.theta_harm);
        put_f64!(b, 212, self.vtarget);
        put_f64!(b, 220, self.zthev.0);
        put_f64!(b, 228, self.zthev.1);
        put_f64!(b, 236, self.xrdp);
        b
    }

    /// Deserialize from the packed little-endian image.
    pub fn from_bytes(b: &[u8; Self::SIZE]) -> Self {
        Self {
            theta: get_f64!(b, 0),
            pshaft: get_f64!(b, 8),
            speed: get_f64!(b, 16),
            w0: get_f64!(b, 24),
            hmass: get_f64!(b, 32),
            mmass: get_f64!(b, 40),
            d: get_f64!(b, 48),
            dpu: get_f64!(b, 56),
            kva_rating: get_f64!(b, 64),
            kv_generator_base: get_f64!(b, 72),
            xd: get_f64!(b, 80),
            xdp: get_f64!(b, 88),
            xdpp: get_f64!(b, 96),
            pu_xd: get_f64!(b, 104),
            pu_xdp: get_f64!(b, 112),
            pu_xdpp: get_f64!(b, 120),
            dtheta: get_f64!(b, 128),
            dspeed: get_f64!(b, 136),
            theta_history: get_f64!(b, 144),
            speed_history: get_f64!(b, 152),
            pnominalperphase: get_f64!(b, 160),
            qnominalperphase: get_f64!(b, 168),
            num_phases: get_i32!(b, 176),
            num_conductors: get_i32!(b, 180),
            conn: get_i32!(b, 184),
            vthev_mag: get_f64!(b, 188),
            vthev_harm: get_f64!(b, 196),
            theta_harm: get_f64!(b, 204),
            vtarget: get_f64!(b, 212),
            zthev: (get_f64!(b, 220), get_f64!(b, 228)),
            xrdp: get_f64!(b, 236),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `DynamicsRec` field sits at its frozen ABI-doc §2.1 offset.
    #[test]
    fn dynamics_rec_offsets_match_abi_doc() {
        // Distinct, bit-recognizable values per field.
        let r = DynamicsRec {
            h: 1.5,
            t: 2.5,
            tstart: 3.5,
            tstop: 4.5,
            iteration_flag: 0x1111_1111,
            solution_mode: 14, // DYNAMICMODE
            int_hour: 0x2222_2222,
            dbl_hour: 5.5,
        };
        let b = r.to_bytes();
        assert_eq!(b.len(), 52);
        assert_eq!(f64::from_le_bytes(b[0..8].try_into().unwrap()), 1.5);
        assert_eq!(f64::from_le_bytes(b[8..16].try_into().unwrap()), 2.5);
        assert_eq!(f64::from_le_bytes(b[16..24].try_into().unwrap()), 3.5);
        assert_eq!(f64::from_le_bytes(b[24..32].try_into().unwrap()), 4.5);
        assert_eq!(
            i32::from_le_bytes(b[32..36].try_into().unwrap()),
            0x1111_1111
        );
        assert_eq!(i32::from_le_bytes(b[36..40].try_into().unwrap()), 14);
        assert_eq!(
            i32::from_le_bytes(b[40..44].try_into().unwrap()),
            0x2222_2222
        );
        assert_eq!(f64::from_le_bytes(b[44..52].try_into().unwrap()), 5.5);
        assert_eq!(DynamicsRec::from_bytes(&b), r);
    }

    /// Spot-checks of the `TGeneratorVars` layout against the frozen ABI-doc
    /// §2.2 table, including the packed unaligned tail after the three i32s.
    #[test]
    fn generator_vars_offsets_match_abi_doc() {
        let g = GeneratorVars {
            theta: 0.25,
            pshaft: 1000.0,
            qnominalperphase: -7.0,
            num_phases: 3,
            num_conductors: 4,
            conn: 1,
            vthev_mag: 2400.0,
            zthev: (0.1, 0.9),
            xrdp: 20.0,
            ..Default::default()
        };
        let b = g.to_bytes();
        assert_eq!(b.len(), 244);
        assert_eq!(f64::from_le_bytes(b[0..8].try_into().unwrap()), 0.25);
        assert_eq!(f64::from_le_bytes(b[8..16].try_into().unwrap()), 1000.0);
        assert_eq!(f64::from_le_bytes(b[168..176].try_into().unwrap()), -7.0);
        assert_eq!(i32::from_le_bytes(b[176..180].try_into().unwrap()), 3);
        assert_eq!(i32::from_le_bytes(b[180..184].try_into().unwrap()), 4);
        assert_eq!(i32::from_le_bytes(b[184..188].try_into().unwrap()), 1);
        // The unaligned tail: VthevMag at 188, Zthev at 220/228, XRdp at 236.
        assert_eq!(f64::from_le_bytes(b[188..196].try_into().unwrap()), 2400.0);
        assert_eq!(f64::from_le_bytes(b[220..228].try_into().unwrap()), 0.1);
        assert_eq!(f64::from_le_bytes(b[228..236].try_into().unwrap()), 0.9);
        assert_eq!(f64::from_le_bytes(b[236..244].try_into().unwrap()), 20.0);
        assert_eq!(GeneratorVars::from_bytes(&b), g);
    }

    /// Byte-exact round trip with every field holding a distinct bit pattern.
    #[test]
    fn generator_vars_round_trip_all_fields() {
        let g = GeneratorVars {
            theta: 1.0,
            pshaft: 2.0,
            speed: 3.0,
            w0: 4.0,
            hmass: 5.0,
            mmass: 6.0,
            d: 7.0,
            dpu: 8.0,
            kva_rating: 9.0,
            kv_generator_base: 10.0,
            xd: 11.0,
            xdp: 12.0,
            xdpp: 13.0,
            pu_xd: 14.0,
            pu_xdp: 15.0,
            pu_xdpp: 16.0,
            dtheta: 17.0,
            dspeed: 18.0,
            theta_history: 19.0,
            speed_history: 20.0,
            pnominalperphase: 21.0,
            qnominalperphase: 22.0,
            num_phases: 23,
            num_conductors: 24,
            conn: 25,
            vthev_mag: 26.0,
            vthev_harm: 27.0,
            theta_harm: 28.0,
            vtarget: 29.0,
            zthev: (30.0, 31.0),
            xrdp: 32.0,
        };
        let b = g.to_bytes();
        assert_eq!(GeneratorVars::from_bytes(&b), g);
        assert_eq!(GeneratorVars::from_bytes(&b).to_bytes(), b);
    }
}
