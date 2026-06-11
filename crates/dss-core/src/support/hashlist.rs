//! Case-insensitive name → index list, port of `Shared/HashList.pas`.
//!
//! The engine uses these lists for bus names, device names and class names:
//! `Add` appends (duplicates allowed) and returns the index, `Find` locates
//! the first entry with a given name, and lookups are case-insensitive
//! because every string is stored lowercased — exactly the Pascal semantics.
//! Indices are 0-based here (Pascal was 1-based with 0 = not found).

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct HashList {
    /// All names in insertion order, lowercased.
    names: Vec<String>,
    /// Lowercased name → all indices holding that name, in insertion order.
    map: HashMap<String, Vec<u32>>,
}

impl HashList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-sized list (the Pascal constructor took an expected element count).
    pub fn with_capacity(n: usize) -> Self {
        Self {
            names: Vec::with_capacity(n),
            map: HashMap::with_capacity(n),
        }
    }

    /// Append a name (stored lowercased) and return its index. Duplicate
    /// names are allowed, like the Pascal `Add`.
    pub fn add(&mut self, s: &str) -> usize {
        let lower = s.to_lowercase();
        let idx = self.names.len();
        self.map.entry(lower.clone()).or_default().push(idx as u32);
        self.names.push(lower);
        idx
    }

    /// Index of the first entry named `s`, case-insensitively
    /// (Pascal `Find`; `None` replaces the 0 = not-found convention).
    pub fn find(&self, s: &str) -> Option<usize> {
        self.indices_of(s).next()
    }

    /// All indices holding the name `s`, in insertion order. Replaces the
    /// Pascal `Find`/`FindNext` cursor pair.
    pub fn indices_of(&self, s: &str) -> impl Iterator<Item = usize> + '_ {
        let lower = s.to_lowercase();
        self.map
            .get(&lower)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(|&i| i as usize)
    }

    /// Stored (lowercased) name at `i` (Pascal `NameOfIndex`, which returned
    /// an empty string for invalid indices).
    pub fn name(&self, i: usize) -> Option<&str> {
        self.names.get(i).map(|s| s.as_str())
    }

    /// Number of entries (Pascal `Count`).
    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Remove all entries, keeping allocations (Pascal `Clear`).
    pub fn clear(&mut self) {
        self.names.clear();
        self.map.clear();
    }

    /// Iterate stored names in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_returns_sequential_indices() {
        let mut h = HashList::new();
        assert_eq!(h.add("SourceBus"), 0);
        assert_eq!(h.add("Bus1"), 1);
        assert_eq!(h.add("bus2"), 2);
        assert_eq!(h.len(), 3);
    }

    #[test]
    fn find_is_case_insensitive() {
        let mut h = HashList::new();
        h.add("SourceBus");
        h.add("650");
        assert_eq!(h.find("sourcebus"), Some(0));
        assert_eq!(h.find("SOURCEBUS"), Some(0));
        assert_eq!(h.find("650"), Some(1));
        assert_eq!(h.find("651"), None);
    }

    #[test]
    fn names_are_stored_lowercased() {
        let mut h = HashList::new();
        h.add("MyBus");
        assert_eq!(h.name(0), Some("mybus"));
        assert_eq!(h.name(1), None);
    }

    #[test]
    fn duplicates_are_kept_and_enumerable() {
        // Pascal allowed duplicate names and exposed them via Find/FindNext.
        let mut h = HashList::new();
        h.add("load1");
        h.add("other");
        h.add("Load1");
        h.add("LOAD1");
        assert_eq!(h.find("load1"), Some(0));
        let all: Vec<usize> = h.indices_of("loAD1").collect();
        assert_eq!(all, vec![0, 2, 3]);
    }

    #[test]
    fn clear_empties_the_list() {
        let mut h = HashList::with_capacity(4);
        h.add("a");
        h.add("b");
        h.clear();
        assert!(h.is_empty());
        assert_eq!(h.find("a"), None);
        assert_eq!(h.name(0), None);
        // reusable after clear
        assert_eq!(h.add("c"), 0);
        assert_eq!(h.find("C"), Some(0));
    }

    #[test]
    fn iter_preserves_insertion_order() {
        let mut h = HashList::new();
        h.add("One");
        h.add("Two");
        h.add("Three");
        let names: Vec<&str> = h.iter().collect();
        assert_eq!(names, vec!["one", "two", "three"]);
    }
}
