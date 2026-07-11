//! The Free Pascal 3.2.2 RTL random-number generator (`rtl/inc/system.inc`
//! `mtwist_*` + the `RandSeed`/`OldRandSeed` globals), transcribed 1:1.
//!
//! OpenDSS's `Shared/mathutil.pas` does `initialization Randomize;` and then
//! draws through the RTL `Random` — a Mersenne-Twister MT19937 over `RandSeed`.
//! The Monte Carlo solve modes (`SolveMonte1/2/3`, `PickAFault`,
//! `Load.Randomize`, `Fault.Randomize`) consume this stream. Because upstream
//! seeds it from the clock per process, any value that reaches the output through
//! the RNG is nondeterministic across processes and **cannot** be oracle-pinned
//! (GAPS_PLAN.md §2.1) — the generator is therefore gated only by the Rust-only
//! fixed-seed unit tests in this file.
//!
//! Provenance of those pins: they are **captured FPC-exact**. First derived as
//! canonical MT19937 ground truth (cross-verified against the canonical
//! `mt19937ar.out` reference and CPython's MT19937), then **confirmed
//! bit-for-bit against a real FPC 3.2.2 run** (2026-07-08, `ppcrossx64`
//! `x86_64-win64` — the same arch/SSE2 the pinned oracle's dss_capi backend is
//! built with): every u32, `Random:Double`, `Gauss`, and `QuasiLognormal` pin
//! matched exactly. The probe `tools/fpc/mtwist/mtwist_probe.pas` + its captured
//! output `tools/fpc/mtwist/fpc_output_x86_64_win64.txt` regenerate them (arch
//! matters for `Gauss`/`QuasiLognormal`: build x86_64/SSE2, not i386/x87). The
//! engine's [`FpcRng::randomize`] (time seed) reproduces the documented upstream
//! nondeterminism.
//!
//! Bit-for-bit notes:
//! - `random : extended` on the win64 build (Extended = Double) is
//!   `u32 * (1.0 / 2^32)`, exactly representable, so [`FpcRng::next_f64`] is
//!   bit-exact.
//! - The reseed handshake (`RandSeed<>OldRandSeed` detection, `RandSeed :=
//!   not(RandSeed)` after init, the `mt_index = N+1` power-on sentinel) is
//!   reproduced verbatim so a `set_seed` then draw matches FPC's lazy re-init.

// MT19937 period parameters (FPC `MTWIST_N`/`MTWIST_M`/masks/`MATRIX_A`).
const MTWIST_N: usize = 624;
const MTWIST_M: usize = 397;
const MATRIX_A: u32 = 0x9908_B0DF;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7FFF_FFFF;

/// FPC RTL MT19937 generator state (`mt_state`/`mt_index` + `RandSeed`/
/// `OldRandSeed`). Engine-global in FPC (a unit threadvar); here it is owned by
/// the `Circuit` so the whole solve shares one stream, like the process RTL.
#[derive(Debug, Clone)]
pub struct FpcRng {
    mt_state: [u32; MTWIST_N],
    /// FPC typed constant `mt_index : cardinal = MTWIST_N+1` (the power-on
    /// sentinel that forces the first draw to seed).
    mt_index: usize,
    rand_seed: u32,
    old_rand_seed: u32,
}

impl FpcRng {
    /// Power-on state (FPC globals `RandSeed=0`, `OldRandSeed=0`,
    /// `mt_index=MTWIST_N+1`). The first draw seeds with `RandSeed` (=0, the FPC
    /// default) unless [`set_seed`](Self::set_seed) or [`randomize`](Self::randomize)
    /// changes it first.
    pub fn new() -> Self {
        Self {
            mt_state: [0; MTWIST_N],
            mt_index: MTWIST_N + 1,
            rand_seed: 0,
            old_rand_seed: 0,
        }
    }

    /// Seed with a fixed value (FPC `RandSeed := seed`). The MT is re-initialised
    /// lazily on the next draw, exactly like the RTL.
    pub fn set_seed(&mut self, seed: u32) {
        self.rand_seed = seed;
    }

    /// Construct pre-seeded (test helper: `new` + `set_seed`).
    pub fn from_seed(seed: u32) -> Self {
        let mut r = Self::new();
        r.set_seed(seed);
        r
    }

    /// FPC `Randomize`: seed from the clock (documented nondeterministic, like
    /// upstream's `initialization Randomize`). Never gated — no golden or oracle
    /// compare reads an RNG-carried value (GAPS_PLAN.md §2.1).
    pub fn randomize(&mut self) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u32)
            .unwrap_or(0);
        self.set_seed(nanos);
    }

    /// FPC `mtwist_init` (== Matsumoto `init_genrand`): seed the state array.
    fn init(&mut self, seed: u32) {
        self.mt_state[0] = seed;
        for i in 1..MTWIST_N {
            let prev = self.mt_state[i - 1];
            self.mt_state[i] = 1812433253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        self.mt_index = MTWIST_N;
    }

    /// FPC `MTWIST_TWIST(u, v)`.
    fn twist(u: u32, v: u32) -> u32 {
        let mix = (u & UPPER_MASK) | (v & LOWER_MASK);
        // `cardinal(-(v and 1)) and MATRIX_A`: 0 or MATRIX_A.
        (mix >> 1) ^ (0u32.wrapping_sub(v & 1) & MATRIX_A)
    }

    /// FPC `mtwist_update_state`: regenerate all `N` words.
    fn update_state(&mut self) {
        for k in 0..(MTWIST_N - MTWIST_M) {
            let t = Self::twist(self.mt_state[k], self.mt_state[k + 1]);
            self.mt_state[k] = self.mt_state[k + MTWIST_M] ^ t;
        }
        for k in (MTWIST_N - MTWIST_M)..(MTWIST_N - 1) {
            let t = Self::twist(self.mt_state[k], self.mt_state[k + 1]);
            // FPC `mt_state[k + (M-N)]` = `mt_state[k - (N-M)]`.
            self.mt_state[k] = self.mt_state[k - (MTWIST_N - MTWIST_M)] ^ t;
        }
        let t = Self::twist(self.mt_state[MTWIST_N - 1], self.mt_state[0]);
        self.mt_state[MTWIST_N - 1] = self.mt_state[MTWIST_M - 1] ^ t;
        self.mt_index = 0;
    }

    /// FPC `mtwist_u32rand`: one tempered 32-bit draw, including the lazy reseed
    /// handshake.
    pub fn next_u32(&mut self) -> u32 {
        let mut l_index = self.mt_index;
        self.mt_index = self.mt_index.wrapping_add(1);
        // FPC's `l_index >= MTWIST_N+1` written as the clippy-preferred `> N`
        // (identical for integers): the `mt_index = N+1` power-on sentinel.
        if self.rand_seed != self.old_rand_seed || l_index > MTWIST_N {
            self.init(self.rand_seed);
            self.rand_seed = !self.rand_seed;
            self.old_rand_seed = self.rand_seed;
            l_index = MTWIST_N;
        }
        if l_index == MTWIST_N {
            self.update_state();
            l_index = 0;
            self.mt_index = 1;
        }
        let mut y = self.mt_state[l_index];
        y ^= y >> 11;
        y ^= (y << 7) & 0x9D2C_5680;
        y ^= (y << 15) & 0xEFC6_0000;
        y ^= y >> 18;
        y
    }

    /// FPC `random : extended` on the win64 build: `u32 * (1.0 / 2^32)`, a
    /// uniform `f64` in `[0, 1)`. Both factors are exact, so this is bit-exact.
    pub fn next_f64(&mut self) -> f64 {
        self.next_u32() as f64 * (1.0 / 4_294_967_296.0)
    }
}

impl Default for FpcRng {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::{gauss, quasi_log_normal};
    use super::FpcRng;

    // Captured FPC-exact: the pins below are the canonical MT19937
    // (`init_genrand`) sequence (verified against `mt19937ar.out` and CPython's
    // MT19937) and were confirmed bit-for-bit against a real FPC 3.2.2 x86_64
    // run on 2026-07-08 (`tools/fpc/mtwist/mtwist_probe.pas`, captured in
    // `fpc_output_x86_64_win64.txt`) — every u32, `Random:Double`, `Gauss`, and
    // `QuasiLognormal` matched. Expected doubles are compared bit-exactly
    // (`f64::from_bits`).

    const SEED: u32 = 12345;

    #[test]
    fn u32_sequence_matches_fpc() {
        let mut rng = FpcRng::from_seed(SEED);
        let got: Vec<u32> = (0..8).map(|_| rng.next_u32()).collect();
        assert_eq!(
            got,
            [
                3992670690, 3823185381, 1358822685, 561383553, 789925284, 170765737, 878579710,
                3549516158,
            ]
        );
    }

    #[test]
    fn seed1_first_draw_is_canonical() {
        // The universally-cited `init_genrand(1)` first genrand_int32 value.
        let mut rng = FpcRng::from_seed(1);
        assert_eq!(rng.next_u32(), 1791095845);
    }

    #[test]
    fn default_seed_zero_still_reinits() {
        // FPC power-on: RandSeed=OldRandSeed=0 but mt_index=N+1 forces init(0).
        let mut a = FpcRng::new(); // seed left at 0
        let mut b = FpcRng::from_seed(0);
        assert_eq!(a.next_u32(), b.next_u32());
    }

    #[test]
    fn f64_scaling_is_bit_exact() {
        let mut rng = FpcRng::from_seed(SEED);
        let expect = [
            0x3fedbf6a3c400000u64,
            0x3fec7c25bca00000,
            0x3fd43f7f47400000,
            0x3fc0bb0440800000,
            0x3fc78aa6d2000000,
            0x3fa45b5b52000000,
            0x3fca2f07ff000000,
            0x3fea722a2fc00000,
        ];
        for (k, &bits) in expect.iter().enumerate() {
            assert_eq!(
                rng.next_f64().to_bits(),
                bits,
                "double draw {k} bit pattern"
            );
        }
    }

    #[test]
    fn gauss_matches_fpc_bit_exact() {
        // Gauss = (Σ of 12 Random − 6.0)·sd + mean — pure add/mul, no libm.
        let mut rng = FpcRng::from_seed(SEED);
        let g01 = [
            0x3fc62af569800000u64,
            0x3ff90190af700000,
            0x3fd5d11a97000000,
            0x3ff861d0dc200000,
        ];
        for (k, &bits) in g01.iter().enumerate() {
            let v = gauss(0.0, 1.0, || rng.next_f64());
            assert_eq!(v.to_bits(), bits, "gauss(0,1) draw {k}");
        }

        let mut rng = FpcRng::from_seed(SEED);
        for &bits in &[0x4004b157ab4c0000u64, 0x400a40642bdc0000] {
            let v = gauss(2.5, 0.5, || rng.next_f64());
            assert_eq!(v.to_bits(), bits, "gauss(2.5,0.5)");
        }
    }

    #[test]
    fn quasi_log_normal_matches_reference() {
        // QuasiLognormal = exp(Gauss(0,1))·mean. The Gauss draws feeding it are
        // pinned bit-exact above; only the single `exp` may differ from FPC's
        // own `exp` in the last ulp (FPC uses its RTL exp, not the platform
        // libm this reference used), so compare to a few ulp — never oracle-
        // gated (GAPS_PLAN.md §2.1), so no bit-pin is required or possible here.
        let mut rng = FpcRng::from_seed(SEED);
        let expect = [
            f64::from_bits(0x400c89c08627469cu64),
            f64::from_bits(0x402ca2a597840ffd),
        ];
        for (k, &e) in expect.iter().enumerate() {
            let v = quasi_log_normal(3.0, || rng.next_f64());
            assert!(
                (v - e).abs() <= 4.0 * f64::EPSILON * e.abs(),
                "quasi_log_normal draw {k}: {v} vs {e}"
            );
        }
    }
}
