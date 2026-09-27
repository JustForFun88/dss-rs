//! `gate-kind`: the gate a RETRO_FIXES stage runs, chosen by what its diff can
//! change at compile time (`RETRO_FIXES_PLAN.md` RF-I00-05, user decision
//! 2026-09-27).
//!
//! The kinds are ordered [`GateKind::None`] < [`GateKind::Docs`] <
//! [`GateKind::Comments`] < [`GateKind::Code`]. A file's kind is the maximum of
//! every rule it matches (no rule is a first match), a rename is judged on both
//! of its paths, and a diff's kind is the maximum over its files, `None` when
//! nothing changed. The rules:
//!
//! - **Path rules.** A path under `tools/gate-kind/` is `Code` (the tool never
//!   grades its own change), so is a path on [`ALWAYS_CODE`]. A `.md` is `Docs`.
//!   A path that is neither `.md` nor `.rs` is `Code`: nothing tells "nothing
//!   compiled changed" for a `.toml`, `.json`, `.dss`, `.py`, `Cargo.lock`, …
//! - **File rules.** A `.rs` added, deleted, renamed or copied, with a mode or
//!   type change, with a side that is not UTF-8, or with a side that
//!   `syn::parse_file` rejects is `Code`. Any path whose side is a symlink or a
//!   submodule is `Code` as well.
//! - **Premise rules.** Equal token streams mean equal behaviour only while
//!   nothing reads docs or line numbers. A doc change in a file that carries a
//!   derive outside [`DERIVE_ALLOW`] or an attribute macro is `Code`, and so is
//!   any change to a file whose tokens hold `line!`, `column!` or
//!   `Location::caller`.
//! - **The strip-compare** ([`classify_rs`]). Both sides are normalized as
//!   rustc does before lexing (`\r\n` → `\n`, a leading BOM dropped). Equal
//!   bytes are [`RsKind::Same`]. Otherwise every `doc` attribute the syntax
//!   tree reaches is stripped (and, inside `cfg_attr`, only the `doc` entries),
//!   both sides are rendered with `quote` and compared: different is `Code`,
//!   equal is `Comments`. Macro-invocation tokens, `macro_rules!` bodies and
//!   `Verbatim` nodes are never visited, so a doc edit inside them is compared
//!   (a macro may use docs as data).
//!
//! Doc and plain comments are one kind on purpose: clippy under `-D warnings`
//! lints both and doctests live in doc comments, so the same commands are
//! needed either way. [`FileVerdict`] still names which of the two it saw.
//!
//! The tool never decides by itself what to run: the ritual maps the kind to
//! its commands, and any error grades the diff `Code`.

#![forbid(unsafe_code)]

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use proc_macro2::{Group, Ident, Span, TokenStream, TokenTree};
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::visit_mut::VisitMut;
use syn::{Attribute, Meta, Token};

/// The verdict of the strip-compare on one `.rs` file ([`classify_rs`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RsKind {
    /// Both sides parse and are byte-identical after normalization.
    Same,
    /// Only doc comments, plain comments or whitespace differ.
    Comments,
    /// Anything else, a side that does not parse, or a premise rule.
    Code,
}

/// The gate a diff needs, ordered from nothing to the full gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GateKind {
    /// Nothing changed.
    None,
    /// Documents only (`.md`): the tests that read them at run time.
    Docs,
    /// Comments or doc comments in `.rs` files (plus, possibly, documents).
    Comments,
    /// Anything that may compile differently: the full gate.
    Code,
}

impl fmt::Display for GateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            GateKind::None => "None",
            GateKind::Docs => "Docs",
            GateKind::Comments => "Comments",
            GateKind::Code => "Code",
        })
    }
}

impl From<RsKind> for GateKind {
    fn from(kind: RsKind) -> Self {
        match kind {
            RsKind::Same => GateKind::None,
            RsKind::Comments => GateKind::Comments,
            RsKind::Code => GateKind::Code,
        }
    }
}

/// The `.rs` files a lib unit test outside the RAILS register reads as text,
/// so a comment-only edit of one can red a test only the full gate runs. The
/// list is measured and railed by `crates/dss-core/tests/oracle_parity_cfg_gate.rs`
/// (`gate_rails_are_exactly_the_measured_readers`, RF-I00-05 part 2). Today one
/// file: dss-core's
/// `exec::tests::in_show_results::the_export_bracket_has_no_early_exit_between_its_two_statements`
/// reads `do_export_cmd`'s body, comments included.
pub const ALWAYS_CODE: &[&str] = &["crates/dss-core/src/exec/report.rs"];

/// The directory of this tool: a change there is always `Code`.
pub const TOOL_DIR: &str = "tools/gate-kind/";

/// The derives whose expansion ignores doc attributes. A derive outside this
/// set (clap's `Parser` renders doc comments into help text) makes a doc change
/// in its file `Code`. Judged by the last path segment (`serde::Serialize`).
pub const DERIVE_ALLOW: &[&str] = &[
    "Debug",
    "Clone",
    "Copy",
    "PartialEq",
    "Eq",
    "PartialOrd",
    "Ord",
    "Hash",
    "Default",
    "Serialize",
    "Deserialize",
    "Error",
];

/// The helper attributes of the [`DERIVE_ALLOW`] derives (serde, thiserror,
/// `Default`'s `#[default]`): inert, never an attribute macro.
const DERIVE_HELPERS: &[&str] = &["serde", "error", "from", "source", "backtrace", "default"];

/// rustc's built-in attributes (the Reference's "Built-in attributes index",
/// plus `unsafe(..)` and `bench`): inert, never an attribute macro.
const BUILTIN_ATTRS: &[&str] = &[
    "cfg",
    "cfg_attr",
    "test",
    "ignore",
    "should_panic",
    "bench",
    "derive",
    "automatically_derived",
    "macro_export",
    "macro_use",
    "proc_macro",
    "proc_macro_derive",
    "proc_macro_attribute",
    "collapse_debuginfo",
    "allow",
    "expect",
    "warn",
    "deny",
    "forbid",
    "deprecated",
    "must_use",
    "link",
    "link_name",
    "link_ordinal",
    "no_link",
    "repr",
    "crate_type",
    "crate_name",
    "no_main",
    "export_name",
    "link_section",
    "no_mangle",
    "used",
    "inline",
    "cold",
    "naked",
    "no_builtins",
    "target_feature",
    "track_caller",
    "instruction_set",
    "doc",
    "no_std",
    "no_implicit_prelude",
    "path",
    "recursion_limit",
    "type_length_limit",
    "panic_handler",
    "global_allocator",
    "windows_subsystem",
    "feature",
    "non_exhaustive",
    "debugger_visualizer",
    "unsafe",
];

/// Tool-attribute namespaces rustc accepts without a macro (`#[clippy::..]`,
/// `#[rustfmt::skip]`, `#[diagnostic::on_unimplemented]`).
const TOOL_NAMESPACES: &[&str] = &["clippy", "rustfmt", "diagnostic"];

/// The attribute a stripped `doc` attribute becomes before rendering; every
/// occurrence is dropped from the printed stream. A source that holds this
/// identifier itself is `Code`, so the drop can never erase real tokens.
const SENTINEL: &str = "__gate_kind_stripped_doc__";

/// The strip-compare of one `.rs` file, with the premise rules.
pub fn classify_rs(base: &str, head: &str) -> RsKind {
    classify_rs_verdict(base, head).kind
}

/// [`classify_rs`] with its reason and, for [`RsKind::Comments`], whether a doc
/// attribute moved (`docs_changed`) or only plain comments and whitespace did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsVerdict {
    /// The verdict.
    pub kind: RsKind,
    /// The unstripped token streams differ (a doc attribute moved).
    pub docs_changed: bool,
    /// Why, for the per-file line.
    pub reason: String,
}

impl RsVerdict {
    fn code(reason: impl Into<String>) -> Self {
        RsVerdict {
            kind: RsKind::Code,
            docs_changed: false,
            reason: reason.into(),
        }
    }
}

/// See [`RsVerdict`].
pub fn classify_rs_verdict(base: &str, head: &str) -> RsVerdict {
    let base = normalize(base);
    let head = normalize(head);
    let base_file = match syn::parse_file(&base) {
        Ok(f) => f,
        Err(e) => return RsVerdict::code(format!("the base side does not parse: {e}")),
    };
    let head_file = match syn::parse_file(&head) {
        Ok(f) => f,
        Err(e) => return RsVerdict::code(format!("the head side does not parse: {e}")),
    };
    if base == head {
        return RsVerdict {
            kind: RsKind::Same,
            docs_changed: false,
            reason: "identical after normalization".into(),
        };
    }

    let base_tokens = base_file.to_token_stream();
    let head_tokens = head_file.to_token_stream();
    for (side, tokens) in [("base", &base_tokens), ("head", &head_tokens)] {
        if let Some(what) = reads_its_position(tokens) {
            return RsVerdict::code(format!("the {side} side holds `{what}`"));
        }
        if holds_ident(tokens, SENTINEL) {
            return RsVerdict::code(format!("the {side} side holds the tool's sentinel"));
        }
    }
    if base_file.shebang != head_file.shebang {
        return RsVerdict::code("the shebang line changed");
    }

    let docs_changed = base_tokens.to_string() != head_tokens.to_string();
    if docs_changed {
        for (side, file) in [("base", &base_file), ("head", &head_file)] {
            if let Some(what) = doc_reader(file) {
                return RsVerdict::code(format!(
                    "the token streams differ and the {side} side carries {what}"
                ));
            }
        }
    }

    if stripped(base_file).to_string() != stripped(head_file).to_string() {
        return RsVerdict::code("the token streams differ after the doc strip");
    }
    RsVerdict {
        kind: RsKind::Comments,
        docs_changed,
        reason: if docs_changed {
            "only doc attributes, comments or whitespace differ".into()
        } else {
            "only plain comments or whitespace differ".into()
        },
    }
}

/// rustc's pre-lexing normalization: a leading BOM dropped, `\r\n` → `\n`.
fn normalize(text: &str) -> String {
    text.strip_prefix('\u{feff}')
        .unwrap_or(text)
        .replace("\r\n", "\n")
}

/// `line!`, `column!` or `Location::caller` anywhere in the tokens, macro
/// bodies included (comments are not tokens, doc text is a string literal).
fn reads_its_position(tokens: &TokenStream) -> Option<&'static str> {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    for (i, tree) in trees.iter().enumerate() {
        match tree {
            TokenTree::Group(g) => {
                if let Some(what) = reads_its_position(&g.stream()) {
                    return Some(what);
                }
            }
            TokenTree::Ident(id) => {
                // An invocation: `!` then a group, so `line != 3` is no hit.
                let bang = matches!(trees.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!')
                    && matches!(trees.get(i + 2), Some(TokenTree::Group(_)));
                if bang && id == "line" {
                    return Some("line!");
                }
                if bang && id == "column" {
                    return Some("column!");
                }
                let path_sep = matches!(
                    (trees.get(i + 1), trees.get(i + 2)),
                    (Some(TokenTree::Punct(a)), Some(TokenTree::Punct(b)))
                        if a.as_char() == ':' && b.as_char() == ':'
                );
                if id == "Location"
                    && path_sep
                    && matches!(trees.get(i + 3), Some(TokenTree::Ident(c)) if c == "caller")
                {
                    return Some("Location::caller");
                }
            }
            _ => {}
        }
    }
    None
}

fn holds_ident(tokens: &TokenStream, name: &str) -> bool {
    tokens.clone().into_iter().any(|tree| match tree {
        TokenTree::Ident(id) => id == name,
        TokenTree::Group(g) => holds_ident(&g.stream(), name),
        _ => false,
    })
}

/// The first derive outside [`DERIVE_ALLOW`] or attribute macro the syntax
/// tree reaches, as a phrase for the reason.
fn doc_reader(file: &syn::File) -> Option<String> {
    let mut scan = DocReaderScan(None);
    scan.visit_file_mut(&mut file.clone());
    scan.0
}

struct DocReaderScan(Option<String>);

impl VisitMut for DocReaderScan {
    fn visit_attribute_mut(&mut self, attr: &mut Attribute) {
        if self.0.is_none() {
            self.0 = meta_reader(&attr.meta);
        }
    }
}

fn meta_reader(meta: &Meta) -> Option<String> {
    let path = meta.path();
    if path.is_ident("derive") {
        let Meta::List(list) = meta else {
            return Some("an unreadable `derive`".into());
        };
        let Ok(paths) = list.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
        else {
            return Some("an unreadable `derive`".into());
        };
        return paths.iter().find_map(|p| {
            let last = p
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            (!DERIVE_ALLOW.contains(&last.as_str()))
                .then(|| format!("`derive({})`", p.to_token_stream()))
        });
    }
    if path.is_ident("cfg_attr") {
        let Meta::List(list) = meta else {
            return Some("an unreadable `cfg_attr`".into());
        };
        let Ok(entries) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        else {
            return Some("an unreadable `cfg_attr`".into());
        };
        return entries.iter().skip(1).find_map(meta_reader);
    }
    let segments: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    let inert = match segments.as_slice() {
        [one] => BUILTIN_ATTRS.contains(&one.as_str()) || DERIVE_HELPERS.contains(&one.as_str()),
        [first, ..] => TOOL_NAMESPACES.contains(&first.as_str()),
        [] => false,
    };
    (!inert).then(|| format!("the attribute macro `#[{}]`", path.to_token_stream()))
}

/// The file rendered with every reachable doc attribute stripped.
fn stripped(mut file: syn::File) -> TokenStream {
    DocStrip.visit_file_mut(&mut file);
    drop_sentinels(file.to_token_stream())
}

struct DocStrip;

impl VisitMut for DocStrip {
    fn visit_attribute_mut(&mut self, attr: &mut Attribute) {
        if attr.path().is_ident("doc") {
            attr.meta = sentinel();
            return;
        }
        if attr.path().is_ident("cfg_attr") {
            match strip_cfg_attr(&attr.meta) {
                CfgStrip::Opaque | CfgStrip::Untouched => {}
                CfgStrip::Drop => attr.meta = sentinel(),
                CfgStrip::Keep(tokens) => {
                    if let Meta::List(list) = &mut attr.meta {
                        list.tokens = tokens;
                    }
                }
            }
        }
    }
}

fn sentinel() -> Meta {
    Meta::Path(syn::Path::from(Ident::new(SENTINEL, Span::call_site())))
}

/// What the strip does to one `cfg_attr(<pred>, a1, a2, ...)`.
enum CfgStrip {
    /// Its arguments do not parse as metas: left as it is (compared).
    Opaque,
    /// It holds no `doc` entry at any depth: left as it is.
    Untouched,
    /// Every entry was a `doc` entry: the attribute goes.
    Drop,
    /// Some `doc` entries went: the rebuilt arguments.
    Keep(TokenStream),
}

fn strip_cfg_attr(meta: &Meta) -> CfgStrip {
    let Meta::List(list) = meta else {
        return CfgStrip::Opaque;
    };
    let Ok(entries) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
        return CfgStrip::Opaque;
    };
    let trailing = entries.trailing_punct();
    let mut entries = entries.into_iter();
    let Some(predicate) = entries.next() else {
        return CfgStrip::Opaque;
    };
    let mut kept: Vec<Meta> = Vec::new();
    let mut removed = false;
    for mut entry in entries {
        if entry.path().is_ident("doc") {
            removed = true;
            continue;
        }
        if entry.path().is_ident("cfg_attr") {
            match strip_cfg_attr(&entry) {
                CfgStrip::Opaque | CfgStrip::Untouched => {}
                CfgStrip::Drop => {
                    removed = true;
                    continue;
                }
                CfgStrip::Keep(tokens) => {
                    removed = true;
                    if let Meta::List(inner) = &mut entry {
                        inner.tokens = tokens;
                    }
                }
            }
        }
        kept.push(entry);
    }
    if !removed {
        return CfgStrip::Untouched;
    }
    if kept.is_empty() {
        return CfgStrip::Drop;
    }
    let mut out: Punctuated<Meta, Token![,]> = Punctuated::new();
    out.push(predicate);
    for entry in kept {
        out.push(entry);
    }
    if trailing {
        out.push_punct(<Token![,]>::default());
    }
    CfgStrip::Keep(out.to_token_stream())
}

/// Drops every `# [SENTINEL]` and `# ! [SENTINEL]` from a printed stream.
fn drop_sentinels(tokens: TokenStream) -> TokenStream {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let mut out: Vec<TokenTree> = Vec::with_capacity(trees.len());
    let mut i = 0;
    while i < trees.len() {
        if matches!(&trees[i], TokenTree::Punct(p) if p.as_char() == '#') {
            if is_sentinel_group(trees.get(i + 1)) {
                i += 2;
                continue;
            }
            let bang = matches!(trees.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!');
            if bang && is_sentinel_group(trees.get(i + 2)) {
                i += 3;
                continue;
            }
        }
        out.push(match &trees[i] {
            TokenTree::Group(g) => {
                let mut group = Group::new(g.delimiter(), drop_sentinels(g.stream()));
                group.set_span(g.span());
                TokenTree::Group(group)
            }
            other => other.clone(),
        });
        i += 1;
    }
    out.into_iter().collect()
}

fn is_sentinel_group(tree: Option<&TokenTree>) -> bool {
    let Some(TokenTree::Group(g)) = tree else {
        return false;
    };
    let inner: Vec<TokenTree> = g.stream().into_iter().collect();
    matches!(inner.as_slice(), [TokenTree::Ident(id)] if id == SENTINEL)
}

/// One file of a diff with its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileVerdict {
    /// The path, `<src> -> <dst>` for a rename or a copy.
    pub path: String,
    /// The file's kind.
    pub kind: GateKind,
    /// For [`GateKind::Comments`]: a doc attribute moved.
    pub docs_changed: bool,
    /// Why.
    pub reason: String,
}

impl fmt::Display for FileVerdict {
    /// `<kind>\t<path>\t<reason>`, the kind of a `Comments` file naming doc or
    /// plain.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match (self.kind, self.docs_changed) {
            (GateKind::Comments, true) => "Comments(doc)".to_string(),
            (GateKind::Comments, false) => "Comments(plain)".to_string(),
            (kind, _) => kind.to_string(),
        };
        write!(f, "{kind}\t{}\t{}", self.path, self.reason)
    }
}

/// Every file of a diff with its kind.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiffReport {
    /// One entry per changed or untracked path.
    pub files: Vec<FileVerdict>,
}

impl DiffReport {
    /// The maximum over the files, `None` when nothing changed.
    pub fn kind(&self) -> GateKind {
        self.files
            .iter()
            .map(|f| f.kind)
            .max()
            .unwrap_or(GateKind::None)
    }
}

/// The kind of the diff `base_rev..head_rev` of `repo`, or of `base_rev`
/// against the working tree (tracked changes plus every untracked, not ignored
/// file as added) when `head_rev` is `None`. Any error is `Code`.
pub fn classify_diff(repo: &Path, base_rev: &str, head_rev: Option<&str>) -> GateKind {
    classify_diff_report(repo, base_rev, head_rev)
        .map(|r| r.kind())
        .unwrap_or(GateKind::Code)
}

/// [`classify_diff`] file by file; `Err` is a reason (a failing git, an
/// unknown rev), which the caller grades `Code`.
pub fn classify_diff_report(
    repo: &Path,
    base_rev: &str,
    head_rev: Option<&str>,
) -> Result<DiffReport, String> {
    let top = String::from_utf8(git(repo, &["rev-parse", "--show-toplevel"])?)
        .map_err(|_| "the repository path is not UTF-8".to_string())?;
    let top = PathBuf::from(top.trim_end_matches(['\r', '\n']));
    let base = resolve(&top, base_rev)?;
    let head = head_rev.map(|rev| resolve(&top, rev)).transpose()?;

    let mut args = vec![
        "diff",
        "--raw",
        "-z",
        "-M100%",
        "--no-abbrev",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        base.as_str(),
    ];
    if let Some(head) = &head {
        args.push(head.as_str());
    }
    args.push("--");
    let raw = git(&top, &args)?;
    let mut changes = parse_raw(&raw)?;
    if head.is_none() {
        let others = git(&top, &["ls-files", "--others", "--exclude-standard", "-z"])?;
        for path in others.split(|b| *b == 0).filter(|p| !p.is_empty()) {
            changes.push(Change {
                status: 'A',
                src_mode: String::from("000000"),
                dst_mode: String::from("100644"),
                src_sha: String::new(),
                dst_sha: String::new(),
                src: None,
                dst: Some(path.to_vec()),
            });
        }
    }

    let mut report = DiffReport::default();
    for change in &changes {
        report.files.push(judge(&top, head.as_deref(), change));
    }
    Ok(report)
}

/// One entry of `git diff --raw -z`.
struct Change {
    status: char,
    src_mode: String,
    dst_mode: String,
    src_sha: String,
    dst_sha: String,
    /// The base path (`None` for an added file).
    src: Option<Vec<u8>>,
    /// The head path (`None` for a deleted file).
    dst: Option<Vec<u8>>,
}

fn parse_raw(raw: &[u8]) -> Result<Vec<Change>, String> {
    let fields: Vec<&[u8]> = raw.split(|b| *b == 0).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < fields.len() {
        let header = fields[i];
        if header.is_empty() {
            i += 1;
            continue;
        }
        let header = std::str::from_utf8(header)
            .map_err(|_| "unreadable `git diff --raw` header".to_string())?;
        let parts: Vec<&str> = header.trim_start_matches(':').split(' ').collect();
        let [src_mode, dst_mode, src_sha, dst_sha, status] = parts.as_slice() else {
            return Err(format!("unexpected `git diff --raw` header {header:?}"));
        };
        let status = status
            .chars()
            .next()
            .ok_or_else(|| format!("no status in {header:?}"))?;
        let two_paths = matches!(status, 'R' | 'C');
        let first = fields
            .get(i + 1)
            .ok_or_else(|| format!("no path after {header:?}"))?
            .to_vec();
        let second = if two_paths {
            Some(
                fields
                    .get(i + 2)
                    .ok_or_else(|| format!("no second path after {header:?}"))?
                    .to_vec(),
            )
        } else {
            None
        };
        i += if two_paths { 3 } else { 2 };
        let (src, dst) = match (status, second) {
            (_, Some(second)) => (Some(first), Some(second)),
            ('A', None) => (None, Some(first)),
            ('D', None) => (Some(first), None),
            (_, None) => (Some(first.clone()), Some(first)),
        };
        out.push(Change {
            status,
            src_mode: (*src_mode).to_string(),
            dst_mode: (*dst_mode).to_string(),
            src_sha: (*src_sha).to_string(),
            dst_sha: (*dst_sha).to_string(),
            src,
            dst,
        });
    }
    Ok(out)
}

/// The kind of one path by its name alone, `None` for a `.rs` (its content
/// decides).
fn path_kind(path: &str) -> (GateKind, &'static str) {
    if path.starts_with(TOOL_DIR) {
        (GateKind::Code, "the tool never grades its own change")
    } else if ALWAYS_CODE.contains(&path) {
        (
            GateKind::Code,
            "on ALWAYS_CODE: a lib unit test outside RAILS reads it as text",
        )
    } else if path.ends_with(".md") {
        (GateKind::Docs, "a document")
    } else if path.ends_with(".rs") {
        (GateKind::None, "")
    } else {
        (GateKind::Code, "neither .md nor .rs")
    }
}

fn judge(top: &Path, head: Option<&str>, change: &Change) -> FileVerdict {
    let src = change.src.as_deref().map(String::from_utf8_lossy);
    let dst = change.dst.as_deref().map(String::from_utf8_lossy);
    let path = match (&src, &dst) {
        (Some(s), Some(d)) if s != d => format!("{s} -> {d}"),
        (Some(s), _) => s.to_string(),
        (None, Some(d)) => d.to_string(),
        (None, None) => String::new(),
    };
    let verdict = |kind: GateKind, reason: String| FileVerdict {
        path: path.clone(),
        kind,
        docs_changed: false,
        reason,
    };

    let utf8_paths: Option<Vec<&str>> = [&change.src, &change.dst]
        .into_iter()
        .flatten()
        .map(|p| std::str::from_utf8(p).ok())
        .collect();
    let Some(paths) = utf8_paths else {
        return verdict(GateKind::Code, "a path that is not UTF-8".into());
    };

    // Path rules: the maximum over both paths of a rename.
    let mut kind = GateKind::None;
    let mut reason = String::new();
    for p in &paths {
        let (k, why) = path_kind(p);
        if k > kind {
            kind = k;
            reason = why.to_string();
        }
    }
    let regular = |mode: &str| matches!(mode, "000000" | "100644" | "100755");
    if !regular(&change.src_mode) || !regular(&change.dst_mode) {
        return verdict(
            GateKind::Code,
            format!(
                "a symlink or submodule side ({} -> {})",
                change.src_mode, change.dst_mode
            ),
        );
    }
    let is_rs = paths.iter().any(|p| p.ends_with(".rs"));
    if kind == GateKind::Code || !is_rs {
        return verdict(kind, reason);
    }

    // File rules of a `.rs`.
    let status_reason = match change.status {
        'M' => None,
        'A' => Some("an added .rs"),
        'D' => Some("a deleted .rs"),
        'R' => Some("a renamed .rs"),
        'C' => Some("a copied .rs"),
        'T' => Some("a type change"),
        _ => Some("an unmerged or unknown status"),
    };
    if let Some(why) = status_reason {
        return verdict(GateKind::Code, why.into());
    }
    if change.src_mode != change.dst_mode {
        return verdict(
            GateKind::Code,
            format!("a mode change ({} -> {})", change.src_mode, change.dst_mode),
        );
    }
    let base = match blob(top, &change.src_sha) {
        Ok(bytes) => bytes,
        Err(e) => return verdict(GateKind::Code, format!("the base side is unreadable: {e}")),
    };
    let head_bytes = match head {
        Some(_) => blob(top, &change.dst_sha),
        None => std::fs::read(top.join(paths[paths.len() - 1])).map_err(|e| format!("{e}")),
    };
    let head_bytes = match head_bytes {
        Ok(bytes) => bytes,
        Err(e) => return verdict(GateKind::Code, format!("the head side is unreadable: {e}")),
    };
    let (Ok(base), Ok(head_text)) = (String::from_utf8(base), String::from_utf8(head_bytes)) else {
        return verdict(GateKind::Code, "a side that is not UTF-8".into());
    };
    let rs = classify_rs_verdict(&base, &head_text);
    let rs_kind = GateKind::from(rs.kind);
    if rs_kind >= kind {
        FileVerdict {
            path,
            kind: rs_kind,
            docs_changed: rs.docs_changed,
            reason: rs.reason,
        }
    } else {
        verdict(kind, reason)
    }
}

fn blob(top: &Path, sha: &str) -> Result<Vec<u8>, String> {
    if sha.is_empty() || sha.bytes().all(|b| b == b'0') {
        return Err(format!("no blob behind {sha:?}"));
    }
    git(top, &["cat-file", "blob", sha])
}

/// `rev` resolved to a commit id, so a rev is never read as a path and a
/// leading `-` is never an option.
fn resolve(top: &Path, rev: &str) -> Result<String, String> {
    let spec = format!("{rev}^{{commit}}");
    let out = git(
        top,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &spec,
        ],
    )
    .map_err(|_| format!("unknown revision {rev:?}"))?;
    let sha = String::from_utf8(out).map_err(|_| format!("unreadable id for {rev:?}"))?;
    let sha = sha.trim().to_string();
    if sha.is_empty() {
        return Err(format!("unknown revision {rev:?}"));
    }
    Ok(sha)
}

/// `git -C <dir> <args>`, stdout on success. The repository is the one `dir`
/// names: `GIT_DIR` and its siblings of the caller's environment are dropped.
fn git(dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}
