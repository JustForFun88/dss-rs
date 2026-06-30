//! Command-name lookup with abbreviation support, port of Pascal
//! `Shared/Command.pas` (`TCommandList`).
//!
//! Construction registers every full name first, then every proper prefix of
//! every name **in registration order**, skipping prefixes that are already
//! taken. That gives the classic OpenDSS abbreviation rule: the earliest
//! registered command wins each ambiguous prefix (`"m"` resolves to `More`'s
//! alias `M`... whichever owner came first), and a full name always matches
//! itself even if it is a prefix of a later name.
//!
//! All matching is case-insensitive via lowercase keys, the same as the
//! Pascal `TAltHashList` which lowercases on both `Add` and `Find`.

#[cfg(test)]
mod tests;

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CommandList {
    /// Original names, in registration order (Pascal `Get(i)` is 1-based;
    /// here index 0 is the first command).
    names: Vec<String>,
    /// Lowercased full name → index.
    full: HashMap<String, usize>,
    /// Lowercased full names *and* non-colliding proper prefixes → index.
    abbrev: HashMap<String, usize>,
    /// Pascal public `Abbrev` field: when false, only exact full names match.
    pub abbrev_allowed: bool,
}

impl CommandList {
    pub fn new<I, S>(commands: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let names: Vec<String> = commands.into_iter().map(Into::into).collect();
        let mut full = HashMap::with_capacity(names.len());
        let mut abbrev = HashMap::with_capacity(names.len() * 4);

        // Pascal fills both hash lists with the full names first...
        for (i, name) in names.iter().enumerate() {
            let key = name.to_lowercase();
            full.entry(key.clone()).or_insert(i);
            abbrev.entry(key).or_insert(i);
        }
        // ...then adds every proper prefix that is still free, command by
        // command (`for j := 1 to Length(Commands[i]) - 1`).
        for (i, name) in names.iter().enumerate() {
            let key = name.to_lowercase();
            for j in 1..key.len() {
                if !key.is_char_boundary(j) {
                    continue; // names are ASCII in practice; stay panic-free
                }
                abbrev.entry(key[..j].to_string()).or_insert(i);
            }
        }

        Self {
            names,
            full,
            abbrev,
            abbrev_allowed: true,
        }
    }

    /// Look a (possibly abbreviated) command up, returning its 0-based index
    /// (Pascal `GetCommand`, which returned 1-based with 0 = not found).
    pub fn get_command(&self, cmd: &str) -> Option<usize> {
        let key = cmd.to_lowercase();
        if !self.abbrev_allowed {
            self.full.get(&key).copied()
        } else {
            self.abbrev.get(&key).copied()
        }
    }

    /// Full name of command `i` (Pascal `Get(i)`, 1-based there).
    pub fn get(&self, i: usize) -> Option<&str> {
        self.names.get(i).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}
