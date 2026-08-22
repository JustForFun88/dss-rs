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
//!
//! [`WindGenVars`] (ABI doc §2.6, added by `R4133_PROPS_PLAN.md` RP1.3) follows
//! the same rule one step further: the native `TWindGenVars` is 356 bytes with a
//! managed `PLoss: string` reference at 244, and the wasm image drops that
//! reference and closes the hole — 348 bytes whose leading 244 are, by
//! construction, the `GeneratorVars` wasm image.

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

/// Pascal `TWindGenVars` (`PCElements/WindGenVars.pas:20-73`) — the WindGen's
/// public data record, the one `TWindGenUserModel.FNew` receives
/// (`PCElements/WindGenUserModel.pas:34`). The model **mutates** it (the Pascal
/// dynamics call sites expect at least `Pshaft` back from a shaft model,
/// `WindGen.pas:2011`); the host reads it back after every call.
///
/// **Not an alias of [`GeneratorVars`] — a different record.** Measured by the
/// P10 probe (`docs/wasm/probes/p10_offsets_windgenvars_r4133.txt`, native
/// `SizeOf(TWindGenVars) = 356`), it differs from `TGeneratorVars` three ways:
///
/// 1. no NCIM `deltaQNom` slot, so the three integers keep the historical
///    offsets 176/180/184 (`GeneratorVars.pas:41` has it, `WindGenVars.pas` does
///    not);
/// 2. `kVGeneratorBase` is spelled `kVWindGenBase` (`WindGenVars.pas:31`);
/// 3. a turbine tail after `XRdp`: a managed `PLoss: string` reference at native
///    offset 244 (`WindGenVars.pas:59`) and thirteen doubles `ag`…`s`
///    (`:60-72`).
///
/// This is the **wasm marshaled image**: 348 bytes packed (ABI doc §2.6). Like
/// `GeneratorVars`' treatment of `deltaQNom` (ABI doc §2.2b), the managed
/// reference does **not** cross — an AnsiString pointer has no meaning in the
/// guest's disjoint linear memory — and the hole is *closed*, so `ag` sits at
/// 244 rather than the native 252. That makes this image a byte-for-byte
/// **extension** of the 244-byte `GeneratorVars` wasm image: offsets 0…244 are
/// field-for-field identical (only field 10's *name* differs), and the turbine
/// tail follows. Assembled field-by-field at explicit offsets, never via
/// `#[repr(C)]` (the tail is deliberately unaligned — `vthev_mag` at 188).
///
/// **`PLoss` does not cross.** It names the XY curve of turbine active-power
/// losses; a wasm guest cannot follow a host string reference. Successor trap:
/// a model that must know the loss curve has to receive it through the
/// `UserData=` edit string, not through this image — do not "add" `PLoss` here
/// as a pointer-sized hole, that would silently re-shift the whole tail.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WindGenVars {
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
    /// Offset 72 — `kVWindGenBase` (`WindGenVars.pas:31`), the WindGen spelling
    /// of `TGeneratorVars.kVGeneratorBase`.
    pub kv_windgen_base: f64,
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
    /// Offset 244 — gearbox ratio (`WindGenVars.pas:60`). Native 252: the
    /// `PLoss` reference this image drops sits between `xrdp` and here.
    pub ag: f64,
    /// Offset 252 — turbine performance coefficient (`:61`).
    pub cp: f64,
    /// Offset 260 — tip-speed ratio (`:62`).
    pub lamda: f64,
    /// Offset 268 — number of poles of the induction generator (`:63`).
    pub poles: f64,
    /// Offset 276 — air density (`:64`).
    pub pd: f64,
    /// Offset 284 — rotor radius (`:65`).
    pub rad: f64,
    /// Offset 292 — cut-in wind speed (`:66`).
    pub v_cutin: f64,
    /// Offset 300 — cut-out wind speed (`:67`).
    pub v_cutout: f64,
    /// Offset 308 — mechanical power, steady state (`:68`).
    pub pm: f64,
    /// Offset 316 — stator active power (`:69`).
    pub ps: f64,
    /// Offset 324 — rotor active power (`:70`).
    pub pr: f64,
    /// Offset 332 — total power output (`:71`).
    pub pg: f64,
    /// Offset 340 — generator slip/pitch (`:72`).
    pub s: f64,
}

impl WindGenVars {
    /// Size of the packed **wasm** image. The native record is 356 bytes
    /// (probe `p10_offsets_windgenvars_r4133.txt`); this image drops the 8-byte
    /// managed `PLoss` reference and closes the hole — see the type note.
    pub const SIZE: usize = 348;

    /// Byte offset at which the two wasm images diverge: below it this record
    /// is field-for-field the [`GeneratorVars`] wasm image, at and above it the
    /// turbine tail begins. Pinned by `windgen_vars_head_matches_generator_vars`.
    pub const TURBINE_TAIL_OFFSET: usize = GeneratorVars::SIZE;

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
        put_f64!(b, 72, self.kv_windgen_base);
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
        put_f64!(b, 244, self.ag);
        put_f64!(b, 252, self.cp);
        put_f64!(b, 260, self.lamda);
        put_f64!(b, 268, self.poles);
        put_f64!(b, 276, self.pd);
        put_f64!(b, 284, self.rad);
        put_f64!(b, 292, self.v_cutin);
        put_f64!(b, 300, self.v_cutout);
        put_f64!(b, 308, self.pm);
        put_f64!(b, 316, self.ps);
        put_f64!(b, 324, self.pr);
        put_f64!(b, 332, self.pg);
        put_f64!(b, 340, self.s);
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
            kv_windgen_base: get_f64!(b, 72),
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
            ag: get_f64!(b, 244),
            cp: get_f64!(b, 252),
            lamda: get_f64!(b, 260),
            poles: get_f64!(b, 268),
            pd: get_f64!(b, 276),
            rad: get_f64!(b, 284),
            v_cutin: get_f64!(b, 292),
            v_cutout: get_f64!(b, 300),
            pm: get_f64!(b, 308),
            ps: get_f64!(b, 316),
            pr: get_f64!(b, 324),
            pg: get_f64!(b, 332),
            s: get_f64!(b, 340),
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
/// **PARTIAL CODEC — control thresholds are NOT serialized (stay zero).** The
/// native record's control set-points — `ON_Value`/`OFF_Value`/`PFON_Value`/
/// `PFOFF_Value`/`CTRatio`/`PTRatio` (offsets 8…80), `Vmax` (95), `Vmin` (103) —
/// are populated at edit/`RecalcElementData`, NOT in the `Sample` branch a wasm
/// model runs, and the reference `capuserctl` fixture reads its thresholds from
/// `UserData` instead, so they are omitted here (mirrors WM.4's
/// `TStorageVars`/`TPVSystemVars` "interacted-fields only" codecs). **Successor
/// trap:** if `get_public_data` is ever wired to serve a real control model whose
/// decision logic reads these set-points, the zeros are a live bug — extend
/// `to_bytes`/`from_bytes` (and the offset test) with the threshold offsets from
/// the P9 probe before doing so. (Un-gatable regardless — see the asymmetry note.)
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
        // Every unmodeled byte stays zero — the full complement of the written
        // set {111,112,114, 116..160}: the leading thresholds [0..111), the two
        // gap bytes `Armed`@113 / `InitialState`@115, and the trailing region
        // [160..184) (`VOverrideBusName`/`CapacitorName`/`ControlActionHandle`/
        // `CondOffset`). This pins the codec to touch nothing outside its fields.
        assert!(b[0..111].iter().all(|&x| x == 0));
        assert_eq!(b[113], 0);
        assert_eq!(b[115], 0);
        assert!(b[160..184].iter().all(|&x| x == 0));
    }

    /// Distinct-bit-pattern `WindGenVars` used by the layout pins below.
    fn wind_gen_sample() -> WindGenVars {
        WindGenVars {
            theta: 1.0,
            pshaft: 2.0,
            speed: 3.0,
            w0: 4.0,
            hmass: 5.0,
            mmass: 6.0,
            d: 7.0,
            dpu: 8.0,
            kva_rating: 9.0,
            kv_windgen_base: 10.0,
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
            ag: 33.0,
            cp: 34.0,
            lamda: 35.0,
            poles: 36.0,
            pd: 37.0,
            rad: 38.0,
            v_cutin: 39.0,
            v_cutout: 40.0,
            pm: 41.0,
            ps: 42.0,
            pr: 43.0,
            pg: 44.0,
            s: 45.0,
        }
    }

    /// `TWindGenVars` **wasm image** offsets against the P10 probe
    /// (`docs/wasm/probes/p10_offsets_windgenvars_r4133.txt`): the head is the
    /// probe's native head verbatim (no `deltaQNom`, so the integers sit at
    /// 176/180/184 and the unaligned Thevenin tail at 188…244), and the turbine
    /// tail is the probe's 252…356 block shifted down 8 by dropping the managed
    /// `PLoss` reference — `ag` at 244, `s` at 340, total 348.
    #[test]
    fn wind_gen_vars_offsets_match_probe() {
        let w = wind_gen_sample();
        let b = w.to_bytes();
        assert_eq!(b.len(), 348);
        let f = |o: usize| f64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        let i = |o: usize| i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        // Head: probe offsets 0…244 (identical to the GeneratorVars wasm image).
        assert_eq!(f(0), 1.0); // Theta
        assert_eq!(f(64), 9.0); // kVArating
        assert_eq!(f(72), 10.0); // kVWindGenBase (the renamed field)
        assert_eq!(f(168), 22.0); // Qnominalperphase — NOT followed by deltaQNom
        assert_eq!(i(176), 23); // NumPhases
        assert_eq!(i(180), 24); // NumConductors
        assert_eq!(i(184), 25); // Conn
        assert_eq!(f(188), 26.0); // VthevMag (unaligned tail begins)
        assert_eq!(f(220), 30.0); // Zthev.re
        assert_eq!(f(228), 31.0); // Zthev.im
        assert_eq!(f(236), 32.0); // XRdp
        // Turbine tail: probe 252…356 minus the 8-byte PLoss reference.
        assert_eq!(f(244), 33.0); // ag      (probe 252)
        assert_eq!(f(252), 34.0); // Cp      (probe 260)
        assert_eq!(f(260), 35.0); // Lamda   (probe 268)
        assert_eq!(f(268), 36.0); // Poles   (probe 276)
        assert_eq!(f(276), 37.0); // pd      (probe 284)
        assert_eq!(f(284), 38.0); // Rad     (probe 292)
        assert_eq!(f(292), 39.0); // VCutin  (probe 300)
        assert_eq!(f(300), 40.0); // VCutout (probe 308)
        assert_eq!(f(308), 41.0); // Pm      (probe 316)
        assert_eq!(f(316), 42.0); // Ps      (probe 324)
        assert_eq!(f(324), 43.0); // Pr      (probe 332)
        assert_eq!(f(332), 44.0); // Pg      (probe 340)
        assert_eq!(f(340), 45.0); // s       (probe 348)
    }

    /// The load-bearing design property of ABI §2.6: the `TWindGenVars` wasm
    /// image is a byte-for-byte **extension** of the `TGeneratorVars` wasm image
    /// — equal head values produce equal leading 244 bytes. A future edit that
    /// re-inserts the dropped `PLoss` hole (or otherwise shifts the head) fails
    /// here rather than silently mis-decoding every guest.
    #[test]
    fn windgen_vars_head_matches_generator_vars() {
        let w = wind_gen_sample();
        let g = GeneratorVars {
            theta: w.theta,
            pshaft: w.pshaft,
            speed: w.speed,
            w0: w.w0,
            hmass: w.hmass,
            mmass: w.mmass,
            d: w.d,
            dpu: w.dpu,
            kva_rating: w.kva_rating,
            kv_generator_base: w.kv_windgen_base,
            xd: w.xd,
            xdp: w.xdp,
            xdpp: w.xdpp,
            pu_xd: w.pu_xd,
            pu_xdp: w.pu_xdp,
            pu_xdpp: w.pu_xdpp,
            dtheta: w.dtheta,
            dspeed: w.dspeed,
            theta_history: w.theta_history,
            speed_history: w.speed_history,
            pnominalperphase: w.pnominalperphase,
            qnominalperphase: w.qnominalperphase,
            num_phases: w.num_phases,
            num_conductors: w.num_conductors,
            conn: w.conn,
            vthev_mag: w.vthev_mag,
            vthev_harm: w.vthev_harm,
            theta_harm: w.theta_harm,
            vtarget: w.vtarget,
            zthev: w.zthev,
            xrdp: w.xrdp,
        };
        assert_eq!(WindGenVars::TURBINE_TAIL_OFFSET, GeneratorVars::SIZE);
        assert_eq!(
            &w.to_bytes()[..GeneratorVars::SIZE],
            &g.to_bytes()[..],
            "the WindGenVars wasm image must extend the GeneratorVars wasm image"
        );
        // …and the two sizes differ by exactly the thirteen turbine doubles.
        assert_eq!(WindGenVars::SIZE - GeneratorVars::SIZE, 13 * 8);
    }

    /// Byte-exact `WindGenVars` round trip with every field distinct.
    #[test]
    fn wind_gen_vars_round_trip_all_fields() {
        let w = wind_gen_sample();
        let b = w.to_bytes();
        assert_eq!(WindGenVars::from_bytes(&b), w);
        assert_eq!(WindGenVars::from_bytes(&b).to_bytes(), b);
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
