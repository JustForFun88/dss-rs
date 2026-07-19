//! The record shuttle: Rust mirrors of the packed Pascal boundary records and
//! their byte codecs at the frozen ABI offsets.
//!
//! Layouts are the **probe-frozen** tables of `docs/wasm/USERMODEL_ABI.md` §2
//! — little-endian, packed, no padding anywhere. Per the ABI doc §2.2 note, the
//! images are assembled **field-by-field at explicit offsets**, never via
//! `#[repr(C)]` (which would pad the unaligned `TGeneratorVars` tail).
//!
//! These are the **wasm marshaled images** (ABI doc §2.2b), i.e. the fields that
//! cross the host↔guest boundary. The ABI re-freeze to r4133 (2026-07-19) moved
//! the *native* `TGeneratorVars` to 252 bytes (`deltaQNom` at 176, §2.2a), but
//! that field is engine-only (NCIM) and never crosses, so these wasm images are
//! **unchanged** — `GeneratorVars` stays the 244-byte compact subset (identical
//! to the historical 0.14.5/r3723 layout, ABI Appendix A). `DynamicsRec` (52 B)
//! and the callback vtable (256 B) were byte-identical r3723→r4133 to begin with.

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
/// One-byte fields: Pascal `EControlAction` (r4133 has NO `{$Z4}`, so the
/// Delphi-mode default is 1 byte — probe `p9_offsets_capcontrolvars_r4133.txt`)
/// and `Boolean` (1 byte). Stored as `i8`/`u8` in the image.
macro_rules! put_i8 {
    ($buf:expr, $off:expr, $v:expr) => {
        $buf[$off] = ($v as i8) as u8
    };
}
macro_rules! get_i8 {
    ($buf:expr, $off:expr) => {
        $buf[$off] as i8 as i32
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

/// Pascal `TGeneratorVars` (`PCElements/GeneratorVars.pas`) — the Generator's
/// public data record. The model **mutates** it (e.g. sets `Pshaft`, `Speed`,
/// `dSpeed`); the host reads it back after every call.
///
/// This is the **wasm marshaled image**: 244 bytes packed (ABI doc §2.2b),
/// the crossing-fields subset, including the deliberately unaligned tail
/// (`vthev_mag` at 188 after the three i32s). The native r4133 record is 252 B
/// with `deltaQNom` at 176 shifting the tail +8 (ABI §2.2a); that slot is
/// engine-only (NCIM) and never crosses the boundary, so this image keeps the
/// 0.14.5/r3723 offsets (ABI Appendix A) after the re-freeze — `num_phases`
/// at 176, `vthev_mag` at 188, `xrdp` at 236 (asserted below).
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

/// Pascal `TCapControlVars` — the CapControl's `PublicDataStruct := @ControlVars`
/// record (`CapControl.pas:518` r4133 / `:535` dss_capi 0.14.5, "So User-written
/// models can access"). This is the dss-rs `get_public_data` payload for a
/// CapControl instance: on the dss-rs side the host writes the *owning*
/// CapControl's `Sample` context (`SampleP/V/Curr`, bank state) here before
/// `sample()` and the guest reads it back. It is frozen from the P9 probe of the
/// r4133 engine record (`docs/wasm/probes/p9_offsets_capcontrolvars_r4133.txt`;
/// ABI doc §2.5).
///
/// **Layout is the r4133 engine image — 184 bytes packed.** BOTH engines set
/// `PublicDataStruct := @ControlVars` (0.14.5 `:535` and r4133 `:518` — the
/// earlier "0.14.5 never sets it" note was wrong). What differs is the **record
/// layout**: r4133 `Voverride` is **Boolean (1 B)** vs 0.14.5 `LongBool (4 B)`,
/// and r4133 `EControlAction` is **1 B** (no `{$Z4}`) vs 0.14.5's int32 — so the
/// whole tail from `Voverride` on is packed tighter than the 0.14.5 record. The
/// r4133 layout is chosen because the WM.5 gate oracle is the r4133 bridge
/// (WM.3 re-freeze precedent).
///
/// Only the fields a CapControl model interacts with are mapped (the `Sample`
/// context + the bank state); the remaining bytes of the 184-byte image are the
/// CapControl's other public data the model never reads and stay zero on the wasm
/// side (the native engine populates the whole record). Assembled field-by-field
/// at the probed offsets, never via `#[repr(C)]` (the record is deliberately
/// unaligned — `Vmax` at 95, `SampleP` at 116).
///
/// **NOT oracle-gatable via a native twin (proven — ABI doc §2.5).** A *native*
/// CapControl model reads this record via the `GetPublicDataPtr` callback, which
/// returns `ActiveCircuit.ActiveCktElement.PublicDataStruct` — the *global* active
/// element. Neither engine sets `ActiveCktElement` to the CapControl during
/// control sampling (`SampleControlDevices` does not; `CapControl.Sample` does not
/// — 0.14.5 `Solution.pas` / r4133 `CapControl.pas:909`), so `GetPublicDataPtr`
/// does NOT return `@ControlVars` and a native twin cannot reproduce the dss-rs
/// `get_public_data` (owning-element-bound) payload. This image is therefore the
/// dss-rs-side `get_public_data` contract only; the reference `capuserctl` fixture
/// reads its control voltage through the **symmetric** `get_node_voltages`
/// (`GetPtrToSystemVarray` → converged `Solution.NodeV`) channel instead, so it
/// IS oracle-gatable against the r4133 native twin. The guest signals its decision
/// through `control_queue_push` (a plain queue push matching the native twin's
/// `CallBacks.ControlQueuePush`); the pushed `code` becomes the `PendingChange`
/// that `DoPendingAction` acts on.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CapControlVars {
    /// `FPendingChange: EControlAction` (offset 111, 1 byte) — the desired
    /// action (CTRL_NONE/OPEN/CLOSE…). In the native contract a model writes it
    /// via `@ControlVars`; over WASM the guest signals via `control_queue_push`
    /// (see the type note) and the host sets `PendingChange` from the pushed code.
    pub pending_change: i32,
    /// `ShouldSwitch: Boolean` (offset 112, 1 byte) — an action is pending.
    pub should_switch: bool,
    /// `PresentState: EControlAction` (offset 114, 1 byte) — the bank's current
    /// open/closed state (read by the model to pick a direction).
    pub present_state: i32,
    /// `SampleP: Complex` (offsets 116 re / 124 im) — monitored terminal power,
    /// kW + j·kvar (`CapControl.pas:1057`).
    pub sample_p: (f64, f64),
    /// `SampleV: Double` (offset 132) — the control voltage (PT-ratio + phase
    /// selection applied, `CapControl.pas:1060`).
    pub sample_v: f64,
    /// `SampleCurr: Double` (offset 140) — the control current
    /// (`CapControl.pas:1063`).
    pub sample_curr: f64,
    /// `NumCapSteps: Integer` (offset 148).
    pub num_cap_steps: i32,
    /// `AvailableSteps: Integer` (offset 152).
    pub available_steps: i32,
    /// `LastStepInService: Integer` (offset 156).
    pub last_step_in_service: i32,
}

impl CapControlVars {
    /// Size of the packed r4133 image (probe: `SizeOf(TCapControlVars) = 184`).
    pub const SIZE: usize = 184;

    /// Serialize to the packed little-endian image at the r4133 offsets. Fields
    /// not modeled here stay zero (the model never reads them on the wasm side).
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut b = [0u8; Self::SIZE];
        put_i8!(b, 111, self.pending_change);
        put_i8!(b, 112, i32::from(self.should_switch));
        put_i8!(b, 114, self.present_state);
        put_f64!(b, 116, self.sample_p.0);
        put_f64!(b, 124, self.sample_p.1);
        put_f64!(b, 132, self.sample_v);
        put_f64!(b, 140, self.sample_curr);
        put_i32!(b, 148, self.num_cap_steps);
        put_i32!(b, 152, self.available_steps);
        put_i32!(b, 156, self.last_step_in_service);
        b
    }

    /// Deserialize the modeled fields from the packed little-endian image.
    pub fn from_bytes(b: &[u8; Self::SIZE]) -> Self {
        Self {
            pending_change: get_i8!(b, 111),
            should_switch: b[112] != 0,
            present_state: get_i8!(b, 114),
            sample_p: (get_f64!(b, 116), get_f64!(b, 124)),
            sample_v: get_f64!(b, 132),
            sample_curr: get_f64!(b, 140),
            num_cap_steps: get_i32!(b, 148),
            available_steps: get_i32!(b, 152),
            last_step_in_service: get_i32!(b, 156),
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

    /// Spot-checks of the `TGeneratorVars` **wasm image** against the frozen
    /// ABI-doc §2.2b table (244-byte crossing subset, unchanged by the r4133
    /// re-freeze), including the packed unaligned tail after the three i32s.
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

    /// `CapControlVars` fields sit at their probe-frozen r4133 offsets
    /// (`p9_offsets_capcontrolvars_r4133.txt`), incl. the 1-byte `EControlAction`
    /// / `Boolean` slots and the unaligned `SampleP` complex at 116.
    #[test]
    fn cap_control_vars_offsets_match_probe() {
        let cv = CapControlVars {
            pending_change: 2, // CTRL_CLOSE
            should_switch: true,
            present_state: 1, // CTRL_OPEN
            sample_p: (123.0, -45.0),
            sample_v: 122.5,
            sample_curr: 300.0,
            num_cap_steps: 4,
            available_steps: 3,
            last_step_in_service: 2,
        };
        let b = cv.to_bytes();
        assert_eq!(b.len(), 184);
        assert_eq!(b[111] as i8 as i32, 2);
        assert_eq!(b[112], 1);
        assert_eq!(b[114] as i8 as i32, 1);
        assert_eq!(f64::from_le_bytes(b[116..124].try_into().unwrap()), 123.0);
        assert_eq!(f64::from_le_bytes(b[124..132].try_into().unwrap()), -45.0);
        assert_eq!(f64::from_le_bytes(b[132..140].try_into().unwrap()), 122.5);
        assert_eq!(f64::from_le_bytes(b[140..148].try_into().unwrap()), 300.0);
        assert_eq!(i32::from_le_bytes(b[148..152].try_into().unwrap()), 4);
        assert_eq!(i32::from_le_bytes(b[152..156].try_into().unwrap()), 3);
        assert_eq!(i32::from_le_bytes(b[156..160].try_into().unwrap()), 2);
        assert_eq!(CapControlVars::from_bytes(&b), cv);
        // Unmodeled bytes stay zero (the model reads none of them on wasm).
        assert!(b[0..111].iter().all(|&x| x == 0));
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
