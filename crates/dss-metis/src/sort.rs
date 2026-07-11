//! The GKlib inline quicksort `GK_MKQSORT` (`GKlib/include/gk_mksort.h`),
//! ported 1:1. This is glibc's `qsort` (median-of-3 introsort-less quicksort
//! with a `_GKQSORT_MAX_THRESH`-bounded tail finished by insertion sort), which
//! is **not** a stable sort — the order among equal keys is determined by this
//! exact pivot/partition sequence, and that order is part of the bit-exact
//! contract (it feeds `Match_2HopAll`'s candidate scan in `coarsen.c`).
//!
//! On the `METIS_PartGraphKway` default path the only instantiation reached is
//! `ikvsorti` (`gklib.c:78`, comparator `ikey_lt(a,b) = a->key < b->key`), used
//! by `Match_2HopAll`. It is ported here as [`ikvsorti`] over [`Ikv`]. Pointer
//! arithmetic in the macro is translated to `usize` indices into the slice; the
//! `_stack + 1` sentinel-bottom trick is reproduced with a `top` index starting
//! at 1.

/// A `(key, val)` pair — the `ikv_t` sorted by `ikvsorti`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ikv {
    pub key: crate::Idx,
    pub val: crate::Idx,
}

/// `_GKQSORT_MAX_THRESH` (`gk_mksort.h:108`).
const MAX_THRESH: usize = 8;

/// `ikvsorti(n, base)` (`gklib.c:78`) — ascending sort of `ikv_t` by `.key`
/// using `GK_MKQSORT` with `ikey_lt(a,b) = a->key < b->key`.
pub fn ikvsorti(base: &mut [Ikv]) {
    gk_qsort(base, |a, b| a.key < b.key);
}

/// `GK_MKQSORT(TYPE, base, n, LT)` (`gk_mksort.h:118`), generic over the
/// element type and the strict-less-than comparator. Mirrors the glibc
/// quicksort + insertion-sort finisher operation for operation.
pub fn gk_qsort<T: Copy>(base: &mut [T], lt: impl Fn(&T, &T) -> bool) {
    let elems = base.len();
    if elems < 1 {
        return;
    }

    if elems > MAX_THRESH {
        // Pointer pairs -> (lo, hi) index pairs. `top` starts at 1 so that the
        // slot at index 0 is the never-read sentinel bottom (_stack + 1).
        // Indices are `isize` to reproduce the C's signed `ptrdiff_t` pointer
        // differences, which legitimately go to -1 when a partition collapses at
        // the array start (the `_right_ptr - _lo <= MAX_THRESH` test then holds).
        let mt = MAX_THRESH as isize;
        let stack_size = 8 * std::mem::size_of::<usize>();
        let mut stack: Vec<(isize, isize)> = vec![(0, 0); stack_size + 1];
        let mut top: usize = 1;

        let mut lo: isize = 0;
        let mut hi: isize = elems as isize - 1;

        while top >= 1 {
            // Median-of-3: sort *lo, *mid, *hi so the pivot is *mid.
            let mut mid = lo + ((hi - lo) >> 1);

            if lt(&base[mid as usize], &base[lo as usize]) {
                base.swap(mid as usize, lo as usize);
            }
            if lt(&base[hi as usize], &base[mid as usize]) {
                base.swap(mid as usize, hi as usize);
                // (falls through to the second median fixup)
                if lt(&base[mid as usize], &base[lo as usize]) {
                    base.swap(mid as usize, lo as usize);
                }
            }
            // else: goto _jump_over (skip the second fixup)

            let mut left_ptr = lo + 1;
            let mut right_ptr = hi - 1;

            // "Collapse the walls."
            loop {
                while lt(&base[left_ptr as usize], &base[mid as usize]) {
                    left_ptr += 1;
                }
                while lt(&base[mid as usize], &base[right_ptr as usize]) {
                    right_ptr -= 1;
                }

                if left_ptr < right_ptr {
                    base.swap(left_ptr as usize, right_ptr as usize);
                    if mid == left_ptr {
                        mid = right_ptr;
                    } else if mid == right_ptr {
                        mid = left_ptr;
                    }
                    left_ptr += 1;
                    right_ptr -= 1;
                } else if left_ptr == right_ptr {
                    left_ptr += 1;
                    right_ptr -= 1;
                    break;
                }
                if left_ptr > right_ptr {
                    break;
                }
            }

            // Decide which partition(s) to recurse / push (signed, matching C).
            let right_lo = right_ptr - lo;
            let hi_left = hi - left_ptr;

            if right_lo <= mt {
                if hi_left <= mt {
                    // Pop.
                    top -= 1;
                    lo = stack[top].0;
                    hi = stack[top].1;
                } else {
                    lo = left_ptr;
                }
            } else if hi_left <= mt {
                hi = right_ptr;
            } else if right_lo > hi_left {
                stack[top] = (lo, right_ptr);
                top += 1;
                lo = left_ptr;
            } else {
                stack[top] = (left_ptr, hi);
                top += 1;
                hi = right_ptr;
            }
        }
    }

    // Insertion sort over the whole (now partially-sorted) array.
    let end_ptr = elems - 1;
    let thresh = if MAX_THRESH < end_ptr {
        MAX_THRESH
    } else {
        end_ptr
    };

    // Smallest in the first `thresh+1` to the front.
    let mut tmp_ptr = 0usize;
    let mut run_ptr = 1usize;
    while run_ptr <= thresh {
        if lt(&base[run_ptr], &base[tmp_ptr]) {
            tmp_ptr = run_ptr;
        }
        run_ptr += 1;
    }
    if tmp_ptr != 0 {
        base.swap(tmp_ptr, 0);
    }

    // Insertion sort from left to right.
    run_ptr = 1;
    while {
        run_ptr += 1;
        run_ptr <= end_ptr
    } {
        // find insertion point: tmp_ptr = run_ptr-1; while lt(run_ptr, tmp_ptr) --tmp_ptr
        let mut tp = run_ptr - 1;
        while lt(&base[run_ptr], &base[tp]) {
            tp -= 1;
        }
        tp += 1;
        if tp != run_ptr {
            // Rotate base[tp..=run_ptr] right by one (hold = base[run_ptr]).
            let hold = base[run_ptr];
            let mut k = run_ptr;
            while k > tp {
                base[k] = base[k - 1];
                k -= 1;
            }
            base[tp] = hold;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_sorted(v: &[Ikv]) -> bool {
        v.windows(2).all(|w| w[0].key <= w[1].key)
    }

    #[test]
    fn sorts_by_key_ascending() {
        // Larger than MAX_THRESH so the quicksort branch runs.
        let mut v: Vec<Ikv> = (0..40)
            .map(|i| Ikv {
                key: (i * 7 + 3) % 17,
                val: i,
            })
            .collect();
        ikvsorti(&mut v);
        assert!(is_sorted(&v), "not sorted: {v:?}");
    }

    #[test]
    fn sorts_small_arrays() {
        for n in 0..=8usize {
            let mut v: Vec<Ikv> = (0..n as i32)
                .map(|i| Ikv {
                    key: (13 - i * 3).rem_euclid(9),
                    val: i,
                })
                .collect();
            ikvsorti(&mut v);
            assert!(is_sorted(&v), "n={n} not sorted: {v:?}");
        }
    }

    #[test]
    fn preserves_multiset() {
        let mut v: Vec<Ikv> = (0..50)
            .map(|i| Ikv {
                key: (i * 31) % 7,
                val: i,
            })
            .collect();
        let mut keys: Vec<i32> = v.iter().map(|e| e.key).collect();
        keys.sort_unstable();
        ikvsorti(&mut v);
        let got: Vec<i32> = v.iter().map(|e| e.key).collect();
        assert_eq!(got, keys);
    }
}
