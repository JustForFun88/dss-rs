//! A small hand-rolled JSON *reader* (text → [`Json`] tree), the inverse of the
//! [`write_compact`](super::write_compact) / [`write_pretty`](super::write_pretty)
//! writers. It is the front of the AltDSS JSON *import* path
//! (`Circuit_FromJSON` / `Obj_Circuit_FromJSON_`, `CAPI_Obj.pas:2908`), which
//! Free Pascal handles with fpjson's `GetJSON`.
//!
//! ## Why hand-rolled, not `serde_json`
//! Same reason the writers are (see the module docs): the export renders doubles
//! as FPC `Str(Double)` 17-significant scientific literals
//! (`1.2470000000000001E+001`), and we must read those back into an `f64`
//! *bit-for-bit* so a re-export reproduces the same bytes. `f64::from_str`
//! (correctly rounded, matching FPC `Val`) does exactly that; pulling in
//! `serde_json` would add a dependency for a grammar we already own on the write
//! side. This reader accepts the standard JSON grammar (RFC 8259 scalars,
//! objects, arrays, the six string escapes fpjson emits plus `\uXXXX`), enough
//! to round-trip every document the writers produce and the ordinary
//! hand-written AltDSS models a GUI would feed in.
//!
//! Object member order is preserved ([`Json::Obj`] is an ordered vector),
//! matching fpjson's insertion-ordered `TJSONObject`.

use super::Json;

/// Parse a UTF-8 JSON document into a [`Json`] tree. Returns a human-readable
/// error string (byte offset + reason) on malformed input — the caller maps it
/// to a loud DSS error, mirroring the Pascal `except on E: Exception` path in
/// `Circuit_FromJSON`.
pub fn parse_json(text: &str) -> Result<Json, String> {
    let mut p = Reader {
        bytes: text.as_bytes(),
        pos: 0,
    };
    p.skip_ws();
    let v = p.parse_value()?;
    p.skip_ws();
    if p.pos != p.bytes.len() {
        return Err(p.err("trailing characters after JSON value"));
    }
    Ok(v)
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn err(&self, msg: &str) -> String {
        format!("JSON parse error at byte {}: {msg}", self.pos)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn parse_value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(Json::Str(self.parse_string()?)),
            Some(b't') | Some(b'f') => self.parse_bool(),
            Some(b'n') => self.parse_null(),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.parse_number(),
            Some(_) => Err(self.err("unexpected character")),
            None => Err(self.err("unexpected end of input")),
        }
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.err(&format!("expected '{}'", c as char)))
        }
    }

    fn parse_object(&mut self) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut members: Vec<(String, Json)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Obj(members));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected object key string"));
            }
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect(b':')?;
            self.skip_ws();
            let val = self.parse_value()?;
            members.push((key, val));
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Obj(members));
                }
                _ => return Err(self.err("expected ',' or '}' in object")),
            }
        }
    }

    fn parse_array(&mut self) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut items: Vec<Json> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            self.skip_ws();
            let val = self.parse_value()?;
            items.push(val);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return Err(self.err("expected ',' or ']' in array")),
            }
        }
    }

    fn parse_bool(&mut self) -> Result<Json, String> {
        if self.bytes[self.pos..].starts_with(b"true") {
            self.pos += 4;
            Ok(Json::Bool(true))
        } else if self.bytes[self.pos..].starts_with(b"false") {
            self.pos += 5;
            Ok(Json::Bool(false))
        } else {
            Err(self.err("invalid literal"))
        }
    }

    fn parse_null(&mut self) -> Result<Json, String> {
        if self.bytes[self.pos..].starts_with(b"null") {
            self.pos += 4;
            Ok(Json::Null)
        } else {
            Err(self.err("invalid literal"))
        }
    }

    fn parse_number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        let mut is_float = false;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(c) = self.peek() {
            match c {
                b'0'..=b'9' => self.pos += 1,
                b'.' | b'e' | b'E' | b'+' | b'-' => {
                    is_float = true;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        let tok = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| self.err("non-UTF-8 number"))?;
        if is_float {
            // `f64::from_str` is correctly rounded (matches FPC `Val`), so a
            // 17-significant export literal reads back to the identical f64.
            tok.parse::<f64>()
                .map(Json::Float)
                .map_err(|_| self.err("invalid number"))
        } else {
            // A bare integer literal: keep it an [`Json::Int`] when it fits an
            // i64 (property indices, winding counts), else fall back to float.
            match tok.parse::<i64>() {
                Ok(i) => Ok(Json::Int(i)),
                Err(_) => tok
                    .parse::<f64>()
                    .map(Json::Float)
                    .map_err(|_| self.err("invalid number")),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err(self.err("unterminated string")),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some(b'"') => out.push('"'),
                        Some(b'\\') => out.push('\\'),
                        Some(b'/') => out.push('/'),
                        Some(b'b') => out.push('\u{08}'),
                        Some(b'f') => out.push('\u{0C}'),
                        Some(b'n') => out.push('\n'),
                        Some(b'r') => out.push('\r'),
                        Some(b't') => out.push('\t'),
                        Some(b'u') => {
                            let cp = self.parse_hex4()?;
                            // Surrogate pair (fpjson never emits these, but a
                            // hand-written model might).
                            if (0xD800..=0xDBFF).contains(&cp) {
                                if !self.bytes[self.pos + 1..].starts_with(b"\\u") {
                                    return Err(self.err("unpaired high surrogate"));
                                }
                                self.pos += 2; // consume "\u" of the low surrogate
                                let lo = self.parse_hex4()?;
                                if !(0xDC00..=0xDFFF).contains(&lo) {
                                    return Err(self.err("invalid low surrogate"));
                                }
                                let c = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                out.push(
                                    char::from_u32(c)
                                        .ok_or_else(|| self.err("invalid code point"))?,
                                );
                                continue;
                            }
                            out.push(
                                char::from_u32(cp).ok_or_else(|| self.err("invalid code point"))?,
                            );
                        }
                        _ => return Err(self.err("invalid escape")),
                    }
                    self.pos += 1;
                }
                Some(c) if c < 0x80 => {
                    out.push(c as char);
                    self.pos += 1;
                }
                Some(_) => {
                    // Multi-byte UTF-8: copy the full code point verbatim.
                    let rest = &self.bytes[self.pos..];
                    let s = std::str::from_utf8(rest).map_err(|_| self.err("invalid UTF-8"))?;
                    let ch = s.chars().next().expect("non-empty");
                    out.push(ch);
                    self.pos += ch.len_utf8();
                }
            }
        }
    }

    fn parse_hex4(&mut self) -> Result<u32, String> {
        // `self.pos` points at 'u'; the four hex digits follow.
        let hexstart = self.pos + 1;
        let hex = self
            .bytes
            .get(hexstart..hexstart + 4)
            .ok_or_else(|| self.err("truncated \\u escape"))?;
        let s = std::str::from_utf8(hex).map_err(|_| self.err("bad \\u escape"))?;
        let cp = u32::from_str_radix(s, 16).map_err(|_| self.err("bad \\u escape"))?;
        self.pos = hexstart + 3; // leave on the last hex digit; caller +1
        Ok(cp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_writer_output() {
        // A tree with every scalar kind, nested containers, and the fpjson float
        // literal must parse back to an equal tree.
        let tree = Json::Obj(vec![
            ("Name".into(), Json::Str("l1".into())),
            ("kV".into(), Json::Float(12.47)),
            ("n".into(), Json::Int(3)),
            ("neg".into(), Json::Int(-7)),
            ("b".into(), Json::Bool(true)),
            ("z".into(), Json::Null),
            (
                "m".into(),
                Json::Arr(vec![
                    Json::Arr(vec![Json::Float(1.0), Json::Float(0.0)]),
                    Json::Arr(vec![Json::Float(0.0), Json::Float(1.0)]),
                ]),
            ),
            ("empty".into(), Json::Arr(vec![])),
        ]);
        for pretty in [false, true] {
            let mut s = String::new();
            if pretty {
                super::super::write_pretty(&tree, 0, &mut s);
            } else {
                super::super::write_compact(&tree, &mut s);
            }
            let got = parse_json(&s).expect("parse");
            assert_eq!(got, tree, "pretty={pretty}");
        }
    }

    #[test]
    fn float_literal_is_bit_exact() {
        // Either lane's export literal must read back to the identical f64 —
        // the 17-significant parity spelling and the shortest default one.
        for v in [12.47_f64, 0.1, 1e30, 1e-30, -0.0, 123456789.12345679] {
            for lit in [
                super::super::fpjson_float_fpc_impl(v),
                super::super::json_float_shortest_impl(v),
                crate::compat::json_float(v),
            ] {
                let got = parse_json(&lit).expect("parse");
                let Json::Float(back) = got else {
                    panic!("{lit} did not read back as a float")
                };
                assert_eq!(back.to_bits(), v.to_bits(), "{lit}");
            }
        }
    }

    #[test]
    fn string_escapes() {
        let got = parse_json(r#""a/b\\c\"d\tA""#).expect("parse");
        assert_eq!(got, Json::Str("a/b\\c\"d\tA".into()));
    }

    #[test]
    fn rejects_malformed() {
        assert!(parse_json("{").is_err());
        assert!(parse_json("[1,]").is_err());
        assert!(parse_json("{\"a\":}").is_err());
        assert!(parse_json("nul").is_err());
        assert!(parse_json("1 2").is_err());
        assert!(parse_json("").is_err());
    }
}
