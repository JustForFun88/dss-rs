//! The GKlib bucket-locator binary priority queue, ported 1:1 from the
//! `GK_MKPQUEUE(rpq, rpq_t, rkv_t, real_t, idx_t, ...)` instantiation in
//! `libmetis/gklib.c` (template in `GKlib/include/gk_mkpqueue.h`).
//!
//! Only the real-keyed queue (`rpq`, key = `real_t` = [`crate::Real`], value =
//! `idx_t` = [`crate::Idx`]) is on the `METIS_PartGraphKway` cut path (the
//! integer-keyed `ipq` is used only by the volume objective and the
//! `dbglvl&512` block partitioner, neither of which the default option path
//! reaches). The comparator is `key_gt` — `KEY_LT(a,b) := a > b` — so the heap
//! is a **max-heap**: the top is the largest key. Tie handling (equal keys) is
//! part of the contract and follows the template's filter-up/filter-down
//! branches exactly, so the extraction order is bit-reproducible.
//!
//! In C the heap uses a `KVT heap[maxnodes]` array with a parallel
//! `ssize_t locator[maxnodes]` (`-1` = absent). Here the same two arrays are
//! [`Rpq::heap`] and [`Rpq::locator`]; every index/shift expression mirrors the
//! macro (`(i-1)>>1`, `2*i+1`).

use crate::{Idx, Real};

/// A `(key, val)` heap node — the `rkv_t` used by `rpq` (`real_t key`,
/// `idx_t val`).
#[derive(Clone, Copy)]
struct Rkv {
    key: Real,
    val: Idx,
}

/// The real-keyed priority queue (`rpq_t`): a max-heap with an O(1) locator.
///
/// `KEY_LT(a, b)` is `a > b` (see [`key_lt`]).
pub struct Rpq {
    nnodes: usize,
    heap: Vec<Rkv>,
    /// `locator[val]` = the heap index of `val`, or `-1` if not present.
    locator: Vec<isize>,
}

/// `KEY_LT(a, b)` for the `key_gt` comparator (`gklib.c:32`): the queue orders
/// by "a has higher priority than b iff a > b", so the heap root is the max.
#[inline]
fn key_lt(a: Real, b: Real) -> bool {
    a > b
}

impl Rpq {
    /// `rpqCreate(maxnodes)` + `rpqInit` (`gk_mkpqueue.h:19,33`).
    pub fn create(maxnodes: usize) -> Rpq {
        Rpq {
            nnodes: 0,
            heap: vec![Rkv { key: 0.0, val: 0 }; maxnodes],
            locator: vec![-1; maxnodes],
        }
    }

    /// `rpqReset(queue)` (`gk_mkpqueue.h:46`): clears the locators of the live
    /// nodes and resets the count.
    pub fn reset(&mut self) {
        for i in (0..self.nnodes).rev() {
            self.locator[self.heap[i].val as usize] = -1;
        }
        self.nnodes = 0;
    }

    /// `rpqLength(queue)` (`gk_mkpqueue.h:84`).
    #[allow(dead_code)]
    pub fn length(&self) -> usize {
        self.nnodes
    }

    /// `rpqInsert(queue, node, key)` (`gk_mkpqueue.h:93`): filter-up insert.
    pub fn insert(&mut self, node: Idx, key: Real) {
        let mut i = self.nnodes as isize;
        self.nnodes += 1;
        while i > 0 {
            let j = (i - 1) >> 1;
            if key_lt(key, self.heap[j as usize].key) {
                self.heap[i as usize] = self.heap[j as usize];
                self.locator[self.heap[i as usize].val as usize] = i;
                i = j;
            } else {
                break;
            }
        }
        self.heap[i as usize] = Rkv { key, val: node };
        self.locator[node as usize] = i;
    }

    /// `rpqDelete(queue, node)` (`gk_mkpqueue.h:128`): removes an arbitrary node,
    /// then filter-up or filter-down the node that took the last slot.
    pub fn delete(&mut self, node: Idx) {
        let mut i = self.locator[node as usize];
        self.locator[node as usize] = -1;

        self.nnodes -= 1;
        if self.nnodes > 0 && self.heap[self.nnodes].val != node {
            let node2 = self.heap[self.nnodes].val;
            let newkey = self.heap[self.nnodes].key;
            let oldkey = self.heap[i as usize].key;

            if key_lt(newkey, oldkey) {
                // Filter-up
                while i > 0 {
                    let j = (i - 1) >> 1;
                    if key_lt(newkey, self.heap[j as usize].key) {
                        self.heap[i as usize] = self.heap[j as usize];
                        self.locator[self.heap[i as usize].val as usize] = i;
                        i = j;
                    } else {
                        break;
                    }
                }
            } else {
                // Filter down
                let nnodes = self.nnodes as isize;
                loop {
                    let mut j = (i << 1) + 1;
                    if j >= nnodes {
                        break;
                    }
                    if key_lt(self.heap[j as usize].key, newkey) {
                        if j + 1 < nnodes
                            && key_lt(self.heap[(j + 1) as usize].key, self.heap[j as usize].key)
                        {
                            j += 1;
                        }
                        self.heap[i as usize] = self.heap[j as usize];
                        self.locator[self.heap[i as usize].val as usize] = i;
                        i = j;
                    } else if j + 1 < nnodes && key_lt(self.heap[(j + 1) as usize].key, newkey) {
                        j += 1;
                        self.heap[i as usize] = self.heap[j as usize];
                        self.locator[self.heap[i as usize].val as usize] = i;
                        i = j;
                    } else {
                        break;
                    }
                }
            }

            self.heap[i as usize] = Rkv {
                key: newkey,
                val: node2,
            };
            self.locator[node2 as usize] = i;
        }
    }

    /// `rpqUpdate(queue, node, newkey)` (`gk_mkpqueue.h:196`). No-op if the key
    /// is unchanged (the `!KEY_LT && !KEY_LT` early return).
    pub fn update(&mut self, node: Idx, newkey: Real) {
        let oldkey = self.heap[self.locator[node as usize] as usize].key;
        if !key_lt(newkey, oldkey) && !key_lt(oldkey, newkey) {
            return;
        }

        let mut i = self.locator[node as usize];

        if key_lt(newkey, oldkey) {
            // Filter-up
            while i > 0 {
                let j = (i - 1) >> 1;
                if key_lt(newkey, self.heap[j as usize].key) {
                    self.heap[i as usize] = self.heap[j as usize];
                    self.locator[self.heap[i as usize].val as usize] = i;
                    i = j;
                } else {
                    break;
                }
            }
        } else {
            // Filter down
            let nnodes = self.nnodes as isize;
            loop {
                let mut j = (i << 1) + 1;
                if j >= nnodes {
                    break;
                }
                if key_lt(self.heap[j as usize].key, newkey) {
                    if j + 1 < nnodes
                        && key_lt(self.heap[(j + 1) as usize].key, self.heap[j as usize].key)
                    {
                        j += 1;
                    }
                    self.heap[i as usize] = self.heap[j as usize];
                    self.locator[self.heap[i as usize].val as usize] = i;
                    i = j;
                } else if j + 1 < nnodes && key_lt(self.heap[(j + 1) as usize].key, newkey) {
                    j += 1;
                    self.heap[i as usize] = self.heap[j as usize];
                    self.locator[self.heap[i as usize].val as usize] = i;
                    i = j;
                } else {
                    break;
                }
            }
        }

        self.heap[i as usize] = Rkv {
            key: newkey,
            val: node,
        };
        self.locator[node as usize] = i;
    }

    /// `rpqGetTop(queue)` (`gk_mkpqueue.h:260`): pops and returns the max-key
    /// value, or `-1` when empty.
    pub fn get_top(&mut self) -> Idx {
        if self.nnodes == 0 {
            return -1;
        }

        self.nnodes -= 1;

        let vtx = self.heap[0].val;
        self.locator[vtx as usize] = -1;

        let nn = self.nnodes as isize;
        if nn > 0 {
            let key = self.heap[nn as usize].key;
            let node = self.heap[nn as usize].val;
            let mut i: isize = 0;
            loop {
                let mut j = 2 * i + 1;
                if j >= nn {
                    break;
                }
                if key_lt(self.heap[j as usize].key, key) {
                    if j + 1 < nn
                        && key_lt(self.heap[(j + 1) as usize].key, self.heap[j as usize].key)
                    {
                        j += 1;
                    }
                    self.heap[i as usize] = self.heap[j as usize];
                    self.locator[self.heap[i as usize].val as usize] = i;
                    i = j;
                } else if j + 1 < nn && key_lt(self.heap[(j + 1) as usize].key, key) {
                    j += 1;
                    self.heap[i as usize] = self.heap[j as usize];
                    self.locator[self.heap[i as usize].val as usize] = i;
                    i = j;
                } else {
                    break;
                }
            }
            self.heap[i as usize] = Rkv { key, val: node };
            self.locator[node as usize] = i;
        }

        vtx
    }

    /// `rpqSeeTopKey(queue)` (`gk_mkpqueue.h:327`): the top key, or `REAL_MAX`
    /// (`f32::MAX`) when empty. Used by `SelectQueue` (multi-constraint only, so
    /// currently unreferenced on the ncon==1 path — kept for completeness).
    #[allow(dead_code)]
    pub fn see_top_key(&self) -> Real {
        if self.nnodes == 0 {
            Real::MAX
        } else {
            self.heap[0].key
        }
    }
}
