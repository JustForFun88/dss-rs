//! The GKRAND deterministic RNG and the `GK_MKRANDOM` operations, ported 1:1
//! from GKlib (`.inputs/GKlib`, `3b7d61b`).
//!
//! Spec:
//! - `src/random.c` — `gk_randinit`, `gk_randint64`, `gk_randint32` under
//!   `#ifdef USE_GKRAND` (the golden C build defines `USE_GKRAND`, so the MT
//!   path is authoritative). MT19937-64 by Nishimura & Matsumoto.
//! - `include/gk_mkrandom.h` — the `GK_MKRANDOM(FPRFX, RNGT, VALT)` template
//!   that METIS's `libmetis/gklib.c` instantiates for `idx_t` as
//!   `irand`/`irandInRange`/`irandArrayPermute`/`irandArrayPermuteFine`.
//! - `libmetis/util.c::InitRandom` — `isrand(seed == -1 ? 4321 : seed)`.
//!
//! In C the MT state is a translation-unit `static` (thread-local); here it is
//! an explicit [`Rng`] value threaded through the pipeline, which the whole
//! project's `#![forbid(unsafe_code)]` requires and which makes the RNG's
//! consumption order auditable.

use crate::Idx;

const NN: usize = 312;
const MM: usize = 156;
const MATRIX_A: u64 = 0xB502_6F5A_A966_19E9;
/// Most significant 33 bits.
const UM: u64 = 0xFFFF_FFFF_8000_0000;
/// Least significant 31 bits.
const LM: u64 = 0x7FFF_FFFF;

/// The MT19937-64 generator state (`random.c` `mt[NN]` + `mti`).
#[derive(Clone)]
pub struct Rng {
    mt: [u64; NN],
    mti: usize,
}

impl Rng {
    /// `gk_randinit(seed)` (`random.c:73`). Initializes `mt[NN]` from `seed`.
    pub fn new(seed: u64) -> Self {
        let mut mt = [0u64; NN];
        mt[0] = seed;
        for i in 1..NN {
            // 6364136223846793005 * (mt[i-1] ^ (mt[i-1] >> 62)) + i
            mt[i] = 6_364_136_223_846_793_005u64
                .wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 62))
                .wrapping_add(i as u64);
        }
        Rng { mt, mti: NN }
    }

    /// `InitRandom(seed)` (`libmetis/util.c:21`): `isrand(seed == -1 ? 4321 : seed)`.
    /// METIS's default `seed = -1` therefore seeds the MT with `4321`.
    pub fn init_random(seed: Idx) -> Self {
        Rng::new(if seed == -1 { 4321 } else { seed as u64 })
    }

    /// `gk_randint64()` (`random.c:86`) — a value on `[0, 2^63-1]` (the C masks
    /// the tempered word with `0x7FFF_FFFF_FFFF_FFFF`).
    pub fn randint64(&mut self) -> u64 {
        const MAG01: [u64; 2] = [0, MATRIX_A];

        if self.mti >= NN {
            // generate NN words at one time
            for i in 0..(NN - MM) {
                let x = (self.mt[i] & UM) | (self.mt[i + 1] & LM);
                self.mt[i] = self.mt[i + MM] ^ (x >> 1) ^ MAG01[(x & 1) as usize];
            }
            for i in (NN - MM)..(NN - 1) {
                let x = (self.mt[i] & UM) | (self.mt[i + 1] & LM);
                self.mt[i] = self.mt[i.wrapping_add(MM.wrapping_sub(NN))]
                    ^ (x >> 1)
                    ^ MAG01[(x & 1) as usize];
            }
            let x = (self.mt[NN - 1] & UM) | (self.mt[0] & LM);
            self.mt[NN - 1] = self.mt[MM - 1] ^ (x >> 1) ^ MAG01[(x & 1) as usize];

            self.mti = 0;
        }

        let mut x = self.mt[self.mti];
        self.mti += 1;

        x ^= (x >> 29) & 0x5555_5555_5555_5555;
        x ^= (x << 17) & 0x71D6_7FFF_EDA6_0000;
        x ^= (x << 37) & 0xFFF7_EEE0_0000_0000;
        x ^= x >> 43;

        x & 0x7FFF_FFFF_FFFF_FFFF
    }

    /// `gk_randint32()` (`random.c:127`) — `gk_randint64() & 0x7FFF_FFFF`.
    pub fn randint32(&mut self) -> u32 {
        (self.randint64() & 0x7FFF_FFFF) as u32
    }

    /// `irand()` — `GK_MKRANDOM` `FPRFX##rand` for `RNGT = idx_t` (4 bytes, so the
    /// `sizeof(RNGT) <= sizeof(int32_t)` branch): returns `gk_randint32()`.
    /// Not on the `part_graph_kway` path (which draws via `irandInRange`/
    /// `irandArrayPermute`), retained to complete the `GK_MKRANDOM` API.
    #[allow(dead_code)]
    pub fn irand(&mut self) -> Idx {
        self.randint32() as Idx
    }

    /// `irandInRange(max)` — `gk_mkrandom.h:59`. With the default
    /// `GK_RNG_LEGACY_WIDTH == 0` and `max <= 0x7fffffff`, this is
    /// `(idx_t)gk_randint32() % max`. `randint32()` is already `<= 0x7FFF_FFFF`,
    /// so the `i32` cast is non-negative and the modulo is well-defined.
    ///
    /// Panics if `max <= 0` (the C requires `0 < max`; callers never pass `<= 0`).
    pub fn rand_in_range(&mut self, max: Idx) -> Idx {
        debug_assert!(max > 0, "rand_in_range requires max > 0");
        (self.randint32() as Idx) % max
    }

    /// `irandArrayPermute(n, p, nshuffles, flag)` (`gk_mkrandom.h:72`) for
    /// `RNGT = VALT = idx_t`. When `flag == 1`, initializes `p[i] = i` first.
    ///
    /// Mirrors the exact two-branch structure (`n < 10` vs the block-shuffle),
    /// including the commented-out plain swaps and the `v/u` offset pattern.
    pub fn rand_array_permute(&mut self, n: Idx, p: &mut [Idx], nshuffles: Idx, flag: i32) {
        let n_us = n as usize;
        if flag == 1 {
            for (i, slot) in p.iter_mut().enumerate().take(n_us) {
                *slot = i as Idx;
            }
        }

        if n < 10 {
            for _ in 0..n {
                let v = self.rand_in_range(n) as usize;
                let u = self.rand_in_range(n) as usize;
                p.swap(v, u);
            }
        } else {
            for _ in 0..nshuffles {
                let v = self.rand_in_range(n - 3) as usize;
                let u = self.rand_in_range(n - 3) as usize;
                // gk_SWAP(p[v+0], p[u+2]); gk_SWAP(p[v+1], p[u+3]);
                // gk_SWAP(p[v+2], p[u+0]); gk_SWAP(p[v+3], p[u+1]);
                p.swap(v, u + 2);
                p.swap(v + 1, u + 3);
                p.swap(v + 2, u);
                p.swap(v + 3, u + 1);
            }
        }
    }

    /// `irandArrayPermuteFine(n, p, flag)` (`gk_mkrandom.h:111`). Not on the
    /// `part_graph_kway` path; retained (and unit-tested) to complete the
    /// `GK_MKRANDOM` API.
    #[allow(dead_code)]
    pub fn rand_array_permute_fine(&mut self, n: Idx, p: &mut [Idx], flag: i32) {
        let n_us = n as usize;
        if flag == 1 {
            for (i, slot) in p.iter_mut().enumerate().take(n_us) {
                *slot = i as Idx;
            }
        }
        for i in 0..n_us {
            let v = self.rand_in_range(n) as usize;
            p.swap(i, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Values harvested from the vendored C build (GKlib compiled with
    // USE_GKRAND). See tools/golden/gen_metis_reference.md ("RNG harvest").
    // Probe: gk_randinit(seed); loop gk_randint32()/gk_randint64().

    const RANDINT32_SEED4321: [u32; 20] = [
        730536298, 1650742698, 670506230, 415317364, 1116827883, 602136892, 678756924, 1895275372,
        1427688335, 332972946, 1515585629, 156222264, 95561125, 652844354, 1216973792, 1686015413,
        155094389, 966991297, 241796620, 1966772686,
    ];

    const RANDINT64_SEED4321: [u64; 20] = [
        7056104593398897002,
        6183034220892673450,
        71691218965699830,
        2948897753842924916,
        673426630186725611,
        2830889528957591868,
        6266909626926957116,
        4066247372269132652,
        5236865999241268111,
        3077338005570634642,
        9156848209045618781,
        485672013622395704,
        507200382514111909,
        8194820229456043330,
        472108372670909408,
        503128055028288949,
        3889391112032259445,
        6715922467017399745,
        3787267153576166924,
        1516594331899302350,
    ];

    const RANDINRANGE1000_SEED4321: [Idx; 20] = [
        298, 698, 230, 364, 883, 892, 924, 372, 335, 946, 629, 264, 125, 354, 792, 413, 389, 297,
        620, 686,
    ];

    const RANDINT32_SEED123: [u32; 20] = [
        109042752, 256824811, 1921307965, 1386903166, 1702706220, 1320940964, 195480616,
        2078644686, 2111863253, 1636655152, 202922005, 1366262670, 1670171510, 613236534,
        1138792725, 376315345, 153077434, 993711792, 1858445696, 1119623695,
    ];

    #[test]
    fn randint32_matches_c_seed4321() {
        let mut rng = Rng::new(4321);
        for (i, &want) in RANDINT32_SEED4321.iter().enumerate() {
            assert_eq!(rng.randint32(), want, "randint32 seed4321 index {i}");
        }
    }

    #[test]
    fn randint64_matches_c_seed4321() {
        let mut rng = Rng::new(4321);
        for (i, &want) in RANDINT64_SEED4321.iter().enumerate() {
            assert_eq!(rng.randint64(), want, "randint64 seed4321 index {i}");
        }
    }

    #[test]
    fn rand_in_range_matches_c_seed4321() {
        let mut rng = Rng::new(4321);
        for (i, &want) in RANDINRANGE1000_SEED4321.iter().enumerate() {
            assert_eq!(rng.rand_in_range(1000), want, "randInRange(1000) index {i}");
        }
    }

    #[test]
    fn randint32_matches_c_seed123() {
        let mut rng = Rng::new(123);
        for (i, &want) in RANDINT32_SEED123.iter().enumerate() {
            assert_eq!(rng.randint32(), want, "randint32 seed123 index {i}");
        }
    }

    #[test]
    fn init_random_default_seed_is_4321() {
        // METIS SEED=-1 must reseed to 4321 (util.c:23).
        let mut a = Rng::init_random(-1);
        let mut b = Rng::new(4321);
        for _ in 0..8 {
            assert_eq!(a.randint32(), b.randint32());
        }
    }

    #[test]
    fn permute_is_a_permutation() {
        // Structural: rand_array_permute(flag=1) yields a permutation of 0..n.
        let mut rng = Rng::new(4321);
        let n = 25;
        let mut p = vec![0i32; n as usize];
        rng.rand_array_permute(n, &mut p, 2 * n, 1);
        let mut seen = p.clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..n).collect::<Vec<_>>());
    }

    #[test]
    fn permute_fine_small_is_a_permutation() {
        let mut rng = Rng::new(99);
        let n = 7;
        let mut p = vec![0i32; n as usize];
        rng.rand_array_permute_fine(n, &mut p, 1);
        let mut seen = p.clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..n).collect::<Vec<_>>());
    }
}
