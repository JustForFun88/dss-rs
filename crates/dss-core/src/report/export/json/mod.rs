//! Byte-faithful port of the fpjson serialization used by the DSS C-API
//! `Obj_ToJSON` / `Batch_ToJSON` family (`CAPI_Obj.pas`,
//! `DSSObjectHelper.pas`). The AltDSS JSON surface is dss_capi's machine-readable
//! model dump (single object, class batch), the counterpart of the ported plot
//! callback; a GUI attaches to it.
//!
//! ## Why hand-rolled, not `serde_json`
//! fpjson serializes doubles with FPC `Str(Double)` — `1.2470000000000001E+001`
//! (17 significant digits, explicit 3-digit signed exponent). `serde_json`'s
//! `Number` cannot hold that literal (it re-formats to the shortest round-trip),
//! and its pretty printer emits `"k": v`, not fpjson's `"k" : v`. So we keep a
//! tiny ordered [`Json`] tree and two writers that reproduce fpjson exactly.
//!
//! ## Empirically pinned against the oracle (dss-python 0.15.7, 2026-07-11)
//! Every claim below was probed on the pinned oracle via the low-level
//! `lib.DSSElement_ToJSON(opts)` path before coding:
//! - **Float format** — `1.2470000000000001E+001`, `1.0000000000000001E-001`,
//!   `-0.0000000000000000E+000`, `1.0000000000000000E+030`. One integer digit,
//!   16 fraction digits, `E` + explicit sign + 3-digit zero-padded exponent.
//!   `NaN`/`Inf` never reach the formatter (the Double arm emits `null`).
//! - **String escaping** (`StringToJSON`) — `"`→`\"`, `\`→`\\`, and `/` is **NOT
//!   escaped** (probed: bus `a/b\c` → `"a/b\\c"`; name `"q\"t` → `"\"q\\\"t\""`).
//!   Control chars `<0x20` → `\b \t \n \f \r` or `\uXXXX`; non-ASCII bytes pass
//!   through unescaped.
//! - **Compact layout** (`foSingleLineArray,foSingleLineObject,foSkipWhiteSpace`)
//!   — zero whitespace: `{"k":v,"k2":v2}`, `[v,v]`, nested `[[a,b],[c,d]]`.
//! - **Pretty layout** (`FormatJSON([],2)`) — 2-space indent, `"key" : value`
//!   (space-colon-space), every object/array member on its own line, arrays
//!   fully expanded (matrix scalars one-per-line), closing bracket at parent
//!   indent. Empty containers render inline (`[]` / `{}`) — fpjson prints no
//!   inner lines for an empty array/object.

#[cfg(test)]
mod tests;

pub(crate) mod build;
pub(crate) mod circuit;
mod read;

pub use read::parse_json;

/// One JSON value, an insertion-ordered tree (fpjson `TJSONData` subset). `Obj`
/// preserves member order because the object dump depends on the property
/// set-order sweep.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

/// The public option bitset for the JSON export — the DSS `DSSJSONOptions`
/// bits **0–10 only** (`DSSObjectHelper.pas:17-32`; the public C header exposes
/// exactly these). `State`/`Debug`/`Edit` (bits 11–13) are deliberately **not**
/// representable: `State`/`Debug` are commented out as NOT IMPLEMENTED upstream
/// (a silent no-op on the oracle), and `Edit` is the JSON-import-only internal
/// flag — omitting them is the faithful equivalent of "not implemented" (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JsonOpts(u32);

impl JsonOpts {
    pub const NONE: Self = Self(0);
    pub const FULL: Self = Self(1 << 0);
    pub const SKIP_REDUNDANT: Self = Self(1 << 1);
    pub const ENUM_AS_INT: Self = Self(1 << 2);
    pub const FULL_NAMES: Self = Self(1 << 3);
    pub const PRETTY: Self = Self(1 << 4);
    pub const EXCLUDE_DISABLED: Self = Self(1 << 5);
    pub const INCLUDE_DSS_CLASS: Self = Self(1 << 6);
    pub const LOWERCASE_KEYS: Self = Self(1 << 7);
    pub const INCLUDE_DEFAULT_OBJS: Self = Self(1 << 8);
    pub const SKIP_TIMESTAMP: Self = Self(1 << 9);
    pub const SKIP_BUSES: Self = Self(1 << 10);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The raw `joptions` integer the Pascal code tests with `joptions and …`.
    pub fn bits(self) -> u32 {
        self.0
    }

    /// Build from a raw `DSSJSONOptions` integer (the representable bits 0–10).
    ///
    /// This is the one raw-bits entry point, so — per JSON_EXPORT_PLAN §6 — it
    /// must **error loudly**, never silently ignore, when it receives a bit that
    /// is not on the export surface. Bits 11–13 (`State`/`Debug`/`Edit`) are
    /// NOT_PORTED (`State`/`Debug` are commented out upstream as a silent no-op,
    /// `Edit` is the JSON-import-only internal flag); passing any of them is a
    /// programming error and panics rather than being quietly dropped.
    pub fn from_bits(bits: u32) -> Self {
        assert_eq!(
            bits & !0x7FF,
            0,
            "DSSJSONOptions bits 11-13 (State/Debug/Edit) are not representable on \
             the JSON export surface (NOT_PORTED, JSON_EXPORT_PLAN §6)"
        );
        Self(bits)
    }
}

impl std::ops::BitOr for JsonOpts {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for JsonOpts {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// FPC `Str(Double)` with the leading space trimmed — the fpjson
/// `TJSONFloatNumber` default rendering (no custom format is set anywhere in
/// dss_capi; confirmed by grep). Form: `[-]D.DDDDDDDDDDDDDDDDE[+-]DDD` — one
/// integer digit, 16 fraction digits (17 significant), `E`, an explicit sign,
/// and a 3-digit zero-padded exponent (an f64 decimal exponent is always ≤3
/// digits).
///
/// TODO(compat): this reproduces fpjson's fixed 17-significant-digit scientific
/// format 1:1 (`12.47 → 1.2470000000000001E+001`). The clean fix is the
/// shortest round-tripping representation; the goldens pin this exact form, so
/// improved precision would be indistinguishable from a porting bug.
pub fn fpjson_float(x: f64) -> String {
    // NaN/Inf are handled by the caller (the Double arm emits null) and never
    // reach here; guard anyway so a stray value can't produce `NaN`/`inf`
    // tokens that would silently corrupt a golden.
    debug_assert!(x.is_finite(), "fpjson_float called with non-finite value");

    // `{:.16E}` → `D.DDDDDDDDDDDDDDDDE<exp>` where <exp> is a bare signed
    // integer (e.g. `E1`, `E-9`, `E30`). Rewrite the exponent to fpjson's
    // explicit-sign, 3-digit-zero-padded form.
    let raw = format!("{x:.16E}");
    let (mantissa, exp) = raw.split_once('E').expect("scientific format has an E");
    let exp_val: i32 = exp.parse().expect("exponent parses as i32");
    let sign = if exp_val < 0 { '-' } else { '+' };
    format!("{mantissa}E{sign}{:03}", exp_val.unsigned_abs())
}

/// fpjson `StringToJSON`: escape a string for a JSON double-quoted literal.
/// `"`→`\"`, `\`→`\\`; control chars `<0x20` use the short escapes where they
/// exist (`\b \t \n \f \r`) else `\uXXXX`. `/` is **not** escaped and non-ASCII
/// bytes pass through unchanged (both probe-pinned).
fn escape_into(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{09}' => out.push_str("\\t"),
            '\u{0A}' => out.push_str("\\n"),
            '\u{0C}' => out.push_str("\\f"),
            '\u{0D}' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Render a scalar (non-container) value; shared by both writers.
fn write_scalar(v: &Json, out: &mut String) {
    match v {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Int(i) => out.push_str(&i.to_string()),
        Json::Float(f) => out.push_str(&fpjson_float(*f)),
        Json::Str(s) => escape_into(s, out),
        Json::Arr(_) | Json::Obj(_) => unreachable!("write_scalar on a container"),
    }
}

/// Compact writer — fpjson `FormatJSON([foSingleLineArray, foSingleLineObject,
/// foSkipWhiteSpace], 0)`: zero whitespace, insertion order preserved.
pub fn write_compact(v: &Json, out: &mut String) {
    match v {
        Json::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_compact(item, out);
            }
            out.push(']');
        }
        Json::Obj(members) => {
            out.push('{');
            for (i, (k, val)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                escape_into(k, out);
                out.push(':');
                write_compact(val, out);
            }
            out.push('}');
        }
        scalar => write_scalar(scalar, out),
    }
}

/// fpjson pretty-mode line break. TODO(compat): fpjson `FormatJSON` writes the
/// RTL platform `sLineBreak` between members — CRLF on Windows (the platform the
/// oracle and the byte goldens are pinned on), LF on Unix. We emit CRLF to match
/// the Windows-captured goldens byte-for-byte; the clean fix is a single `\n`
/// (or a caller-chosen separator). Compact mode has no line breaks, so it is
/// platform-independent.
const NL: &str = "\r\n";

/// Pretty writer — fpjson `FormatJSON([], 2)`: 2-space indent, `"key" : value`
/// (space-colon-space), one member per line, closing bracket at parent indent.
/// An **empty** container is `[` + line-break + parent-indent + `]` (probe-pinned:
/// `"Conductors" : [\r\n  ]`), NOT inline `[]`.
pub fn write_pretty(v: &Json, indent: usize, out: &mut String) {
    let pad = |n: usize| " ".repeat(n * 2);
    match v {
        Json::Arr(items) => {
            out.push('[');
            out.push_str(NL);
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                    out.push_str(NL);
                }
                out.push_str(&pad(indent + 1));
                write_pretty(item, indent + 1, out);
            }
            // Non-empty: the last item is followed by a line-break before the
            // closing bracket. Empty: no items were written, so the single
            // line-break after `[` leads straight into the parent-indented `]`.
            if !items.is_empty() {
                out.push_str(NL);
            }
            out.push_str(&pad(indent));
            out.push(']');
        }
        Json::Obj(members) => {
            out.push('{');
            out.push_str(NL);
            for (i, (k, val)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                    out.push_str(NL);
                }
                out.push_str(&pad(indent + 1));
                escape_into(k, out);
                out.push_str(" : ");
                write_pretty(val, indent + 1, out);
            }
            if !members.is_empty() {
                out.push_str(NL);
            }
            out.push_str(&pad(indent));
            out.push('}');
        }
        scalar => write_scalar(scalar, out),
    }
}

/// Serialize a value with the fpjson layout `opts` selects — pretty
/// (`FormatJSON([],2)`) when [`JsonOpts::PRETTY`] is set, else compact. This is
/// the `Obj_ToJSON_` / `Batch_ToJSON` serialization step (`CAPI_Obj.pas:774-777`
/// / `:1244-1247`).
pub fn serialize(v: &Json, opts: JsonOpts) -> String {
    let mut out = String::new();
    if opts.contains(JsonOpts::PRETTY) {
        write_pretty(v, 0, &mut out);
    } else {
        write_compact(v, &mut out);
    }
    out
}
