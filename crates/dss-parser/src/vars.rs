//! Parser `@variables`, port of `TParserVar` from `Parser/ParserDel.pas`.
//!
//! Variable names include the leading `@` and are matched case-insensitively
//! (the Pascal hash list stored them lowercased). When a variable's *value*
//! itself contains `@`, the value is stored wrapped in `{...}` so the
//! tokenizer later forces RPN interpretation on substitution.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ParserVars {
    /// Names (lowercased, with `@`) in insertion order.
    names: Vec<String>,
    /// Values, parallel to `names`. May be wrapped in `{...}`.
    values: Vec<String>,
    /// Lowercased name → index.
    map: HashMap<String, usize>,
}

impl ParserVars {
    /// New variable table preloaded with the intrinsic variables, all set to
    /// `"null"` (the Pascal `TParserVar.Create`).
    pub fn new() -> Self {
        let mut vars = Self {
            names: Vec::new(),
            values: Vec::new(),
            map: HashMap::new(),
        };
        for name in [
            "@lastfile",
            "@lastexportfile",
            "@lastshowfile",
            "@lastplotfile",
            "@lastredirectfile",
            "@lastcompilefile",
            "@result",
        ] {
            vars.add(name, "null");
        }
        vars
    }

    /// Define or redefine a variable, returning its index (Pascal `Add`).
    /// A value containing `@` is stored wrapped in braces.
    pub fn add(&mut self, name: &str, value: &str) -> usize {
        let key = name.to_lowercase();
        let stored = if value.contains('@') {
            format!("{{{value}}}")
        } else {
            value.to_string()
        };
        match self.map.get(&key) {
            Some(&idx) => {
                self.values[idx] = stored;
                idx
            }
            None => {
                let idx = self.names.len();
                self.names.push(key.clone());
                self.values.push(stored);
                self.map.insert(key, idx);
                idx
            }
        }
    }

    /// Index of a variable, case-insensitively (Pascal `Lookup`; `None`
    /// replaces the 0 = not-found convention).
    pub fn lookup(&self, name: &str) -> Option<usize> {
        self.map.get(&name.to_lowercase()).copied()
    }

    /// Raw stored value of a variable — including the `{...}` wrapper when
    /// present (Pascal `Lookup` + `Value` read).
    pub fn get(&self, name: &str) -> Option<&str> {
        self.lookup(name).map(|i| self.values[i].as_str())
    }

    /// Overwrite the value of an *existing* variable without the brace
    /// treatment (Pascal `Value` write). Returns false if the variable does
    /// not exist.
    pub fn set_value(&mut self, name: &str, value: &str) -> bool {
        match self.lookup(name) {
            Some(i) => {
                self.values[i] = value.to_string();
                true
            }
            None => false,
        }
    }

    /// Number of variables (Pascal `NumVariables`).
    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Listing line for index `i` (Pascal `VarString`): `"name. value"`,
    /// with `null` standing in for an empty value.
    pub fn var_string(&self, i: usize) -> String {
        if i < self.names.len() {
            let value = if self.values[i].is_empty() {
                "null"
            } else {
                &self.values[i]
            };
            format!("{}. {}", self.names[i], value)
        } else {
            "Variable index out of range".to_string()
        }
    }

    /// Iterate `(name, raw value)` in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.names
            .iter()
            .zip(&self.values)
            .map(|(n, v)| (n.as_str(), v.as_str()))
    }
}

impl Default for ParserVars {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intrinsics_are_preloaded() {
        let vars = ParserVars::new();
        assert_eq!(vars.len(), 7);
        assert_eq!(vars.get("@result"), Some("null"));
        assert_eq!(vars.get("@lastfile"), Some("null"));
        assert_eq!(vars.get("@nosuch"), None);
    }

    #[test]
    fn add_is_case_insensitive_and_redefines() {
        let mut vars = ParserVars::new();
        let i = vars.add("@MyVar", "42");
        assert_eq!(vars.get("@myvar"), Some("42"));
        assert_eq!(vars.get("@MYVAR"), Some("42"));
        let j = vars.add("@MYVAR", "43");
        assert_eq!(i, j); // same slot, no duplicate
        assert_eq!(vars.get("@myvar"), Some("43"));
        assert_eq!(vars.len(), 8);
    }

    #[test]
    fn value_with_at_sign_is_brace_wrapped() {
        let mut vars = ParserVars::new();
        vars.add("@a", "2");
        vars.add("@b", "@a 3 *");
        assert_eq!(vars.get("@b"), Some("{@a 3 *}"));
    }

    #[test]
    fn set_value_only_touches_existing() {
        let mut vars = ParserVars::new();
        assert!(!vars.set_value("@missing", "x"));
        vars.add("@v", "1");
        assert!(vars.set_value("@v", "has @ sign"));
        // set_value stores raw, without the brace treatment (Pascal semantics)
        assert_eq!(vars.get("@v"), Some("has @ sign"));
    }

    #[test]
    fn var_string_listing() {
        let mut vars = ParserVars::new();
        let i = vars.add("@x", "12");
        assert_eq!(vars.var_string(i), "@x. 12");
        vars.set_value("@x", "");
        assert_eq!(vars.var_string(i), "@x. null");
        assert_eq!(vars.var_string(999), "Variable index out of range");
    }
}
