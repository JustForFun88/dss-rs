//! `Dump buslist` / `Dump devicelist` — the hash-list dump files (Pascal
//! `Shared/HashList.pas` `THashList.DumpToFile` / `TAltHashList.DumpToFile`,
//! driven from `ExecHelper.pas` `DoPropertyDump`).
//!
//! The two circuit name lists have **different Pascal types** with different
//! dumps (`Circuit.pas:107-109`):
//!
//! - `BusList: TBusHashListType = TAltHashList` — an FPC `TFPHashList`
//!   wrapper whose `DumpToFile` prints **only** the `LINEAR LISTING...`
//!   section ([`alt_dump`]). That is the structural reason the bus dump has
//!   no bucket sections: it is a different class, not an allocation quirk.
//! - `DeviceList: THashList` — the classic bucketed list, whose `DumpToFile`
//!   prints the bucket distribution, the per-bucket members, and then the
//!   linear listing ([`device_hash_dump`]).
//!
//! The Rust engine keeps both lists as the insertion-ordered
//! [`HashList`](super::HashList) (bucket layout is irrelevant to lookup
//! semantics), so the `THashList` bucket state is **replayed** here at dump
//! time: `THashList` has no removal, `DeviceList` is pure-append from circuit
//! creation (`Circuit.pas` `AddCktElement`), and the layout is a pure function
//! of the constructor size and the `Add` sequence — replaying
//! `AddCktElement`'s create(900)/realloc/add protocol over the stored names
//! reproduces the Pascal state exactly.

/// Pascal `THashList` bucket state (`Shared/HashList.pas`): `NumLists`
/// buckets, each holding `(lowercased name, 1-based linear index)` members.
struct THashSim {
    /// `InitialAllocation` (the constructor's `Nelements`).
    initial_allocation: usize,
    /// `NumLists = round(sqrt(Nelements))`, min 1.
    buckets: Vec<Vec<(String, usize)>>,
    /// `StringPtr[1..NumElements]` — the linear listing.
    linear: Vec<String>,
}

impl THashSim {
    /// Pascal `THashList.Create(Nelements)`: `NumLists := round(sqrt(N))`
    /// (FPC `Round` = ties-to-even), floored at 1.
    fn create(nelements: usize) -> Self {
        let num_lists = ((nelements as f64).sqrt().round_ties_even() as usize).max(1);
        Self {
            initial_allocation: nelements,
            buckets: vec![Vec::new(); num_lists],
            linear: Vec::new(),
        }
    }

    /// Pascal `THashList.Hash`: rotate-left-5 XOR over the (lowercased)
    /// bytes, 32-bit, then `mod NumLists` (0-based bucket here; Pascal's
    /// `+ 1` is its 1-based array).
    fn hash(&self, s: &str) -> usize {
        let mut h: u32 = 0;
        for &b in s.as_bytes() {
            h = h.rotate_left(5) ^ u32::from(b);
        }
        (h as usize) % self.buckets.len()
    }

    /// Pascal `THashList.Add`: lowercase, hash, append to the bucket and the
    /// linear list.
    fn add(&mut self, s: &str) {
        let lower = s.to_ascii_lowercase();
        let idx = self.linear.len() + 1; // 1-based NumElements
        let b = self.hash(&lower);
        self.buckets[b].push((lower.clone(), idx));
        self.linear.push(lower);
    }
}

/// Replay `TDSSCircuit.AddCktElement`'s DeviceList protocol (`Circuit.pas:
/// 2065-2073` + `ReallocDeviceList` `:2990-3007`) over the stored device
/// names, then render `THashList.DumpToFile` (`HashList.pas:307-344`).
/// `names` iterates the devices in creation order (the Rust `device_list`).
pub(crate) fn device_hash_dump<'a>(names: impl Iterator<Item = &'a str>) -> String {
    // `DeviceList := THashList.Create(900)` (`Circuit.pas:406`).
    let mut sim = THashSim::create(900);
    for (k, name) in names.enumerate() {
        // `Inc(NumDevices); if NumDevices > 2 * DeviceList.InitialAllocation
        // then ReAllocDeviceList;` — rebuild at 2×NumDevices, re-adding every
        // name added so far.
        let num_devices = k + 1;
        if num_devices > 2 * sim.initial_allocation {
            let mut bigger = THashSim::create(2 * num_devices);
            for prev in &sim.linear {
                bigger.add(prev);
            }
            sim = bigger;
        }
        sim.add(name);
    }
    dump_to_file(&sim)
}

/// Pascal `THashList.DumpToFile`: header, the count-only distribution, the
/// per-bucket member listing, then the linear listing.
fn dump_to_file(sim: &THashSim) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "Number of Hash Lists = {}, Number of Elements = {}\n",
        sim.buckets.len(),
        sim.linear.len()
    ));
    s.push('\n');
    s.push_str("Hash List Distribution\n");
    for (i, b) in sim.buckets.iter().enumerate() {
        s.push_str(&format!(
            "List = {}, Number of elements = {}\n",
            i + 1,
            b.len()
        ));
    }
    s.push('\n');
    for (i, b) in sim.buckets.iter().enumerate() {
        s.push_str(&format!(
            "List = {}, Number of elements = {}\n",
            i + 1,
            b.len()
        ));
        for (name, idx) in b {
            // `WriteStr(sout, '"', Str[j], '"  Idx= ', Idx[j]: 0)`.
            s.push_str(&format!("\"{name}\"  Idx= {idx}\n"));
        }
        s.push('\n');
    }
    s.push_str("LINEAR LISTING...\n");
    for (i, name) in sim.linear.iter().enumerate() {
        // `WriteStr(sout, i: 3, ' = "', Stringptr[i], '"')`.
        s.push_str(&format!("{:>3} = \"{name}\"\n", i + 1));
    }
    s
}

/// Pascal `TAltHashList.DumpToFile` (`HashList.pas:396-405`), the `BusList`
/// dump: only the linear listing (`Format('%3d = "%s"', …)`).
pub(crate) fn alt_dump<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let mut s = String::new();
    s.push_str("LINEAR LISTING...\n");
    for (i, name) in names.enumerate() {
        s.push_str(&format!("{:>3} = \"{name}\"\n", i + 1));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The oracle's `dump devicelist` bucket layout for the WP8.5 `dump3.dss`
    /// fixture (probe 2026-07-07): 15 devices in a 30-bucket list; pins the
    /// rotate-left-5 hash + the create(900) sizing.
    #[test]
    fn device_hash_matches_oracle_probe() {
        let names = [
            "source", "l1", "tr1", "rc1", "f1", "fg", "ldk", "ldx", "ldc", "mon1", "em1", "tup",
            "u1", "uc1", "ldu",
        ];
        let dump = device_hash_dump(names.iter().copied());
        assert!(dump.starts_with("Number of Hash Lists = 30, Number of Elements = 15\n"));
        // Bucket membership spot-checks straight from the oracle file.
        assert!(dump.contains("List = 3, Number of elements = 1\n\"ldx\"  Idx= 8\n"));
        assert!(
            dump.contains("List = 14, Number of elements = 2\n\"f1\"  Idx= 5\n\"uc1\"  Idx= 14\n")
        );
        assert!(dump.contains("List = 30, Number of elements = 3\n\"fg\"  Idx= 6\n\"em1\"  Idx= 11\n\"ldu\"  Idx= 15\n"));
        assert!(dump.ends_with("LINEAR LISTING...\n  1 = \"source\"\n  2 = \"l1\"\n  3 = \"tr1\"\n  4 = \"rc1\"\n  5 = \"f1\"\n  6 = \"fg\"\n  7 = \"ldk\"\n  8 = \"ldx\"\n  9 = \"ldc\"\n 10 = \"mon1\"\n 11 = \"em1\"\n 12 = \"tup\"\n 13 = \"u1\"\n 14 = \"uc1\"\n 15 = \"ldu\"\n"));
    }

    #[test]
    fn alt_dump_is_linear_only() {
        let dump = alt_dump(["src", "b1"].into_iter());
        assert_eq!(dump, "LINEAR LISTING...\n  1 = \"src\"\n  2 = \"b1\"\n");
    }
}
