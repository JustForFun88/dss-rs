//! Case-insensitive name → index list, port of `Shared/HashList.pas`.
//!
//! The engine uses these lists for bus names, device names and class names:
//! `Add` appends (duplicates allowed) and returns the index, `Find` locates
//! the first entry with a given name, and lookups are case-insensitive
//! because every string is stored lowercased — exactly the Pascal semantics.
//! Indices are 0-based here (Pascal was 1-based with 0 = not found).

#[cfg(test)]
mod tests;

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
