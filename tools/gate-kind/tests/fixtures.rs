//! The strip-compare on fixture pairs (`tests/fixtures/<case>/base.rs.txt` and
//! `head.rs.txt`, stored as `.txt` so neither the cfg-gate walk nor cargo's
//! target discovery sees them; `tests/fixtures/.gitattributes` keeps their
//! bytes). Every case asserts its exact kind and, for `Comments`, whether a
//! doc attribute moved.

use std::path::PathBuf;

use gate_kind::{RsKind, classify_rs, classify_rs_verdict};

fn pair(case: &str) -> (String, String) {
    let dir: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", case]
        .iter()
        .collect();
    let read = |side: &str| {
        let path = dir.join(format!("{side}.rs.txt"));
        String::from_utf8(std::fs::read(&path).unwrap_or_else(|e| panic!("{path:?}: {e}")))
            .unwrap_or_else(|e| panic!("{path:?} is not UTF-8: {e}"))
    };
    (read("base"), read("head"))
}

/// `case` grades `kind`; for `Comments`, `docs` says whether a doc moved.
fn assert_case(case: &str, kind: RsKind, docs: Option<bool>) {
    let (base, head) = pair(case);
    let verdict = classify_rs_verdict(&base, &head);
    assert_eq!(verdict.kind, kind, "{case}: {}", verdict.reason);
    assert_eq!(classify_rs(&base, &head), kind, "{case}: the plain API");
    if let Some(docs) = docs {
        assert_eq!(verdict.docs_changed, docs, "{case}: {}", verdict.reason);
    }
}

#[test]
fn a_line_comment_edit_is_comments() {
    assert_case("line_comment", RsKind::Comments, Some(false));
}

#[test]
fn a_nested_block_comment_edit_is_comments() {
    assert_case("block_comment", RsKind::Comments, Some(false));
}

#[test]
fn outer_inner_and_block_doc_edits_with_a_doctest_are_comments() {
    assert_case("doc_comment", RsKind::Comments, Some(true));
}

#[test]
fn a_four_slash_comment_is_a_plain_comment() {
    assert_case("four_slashes", RsKind::Comments, Some(false));
}

#[test]
fn an_added_blank_line_is_comments() {
    assert_case("added_line", RsKind::Comments, Some(false));
}

/// rustc normalizes `\r\n` before lexing, so the multi-line literal is the
/// same string on both sides and only the comment moved. Without the
/// normalization the literal's bytes differ and the verdict would be `Code`.
#[test]
fn an_lf_base_against_a_crlf_head_with_a_multiline_literal_is_comments() {
    let (base, head) = pair("crlf_head");
    assert!(
        !base.contains('\r'),
        "the base must be LF: the checkout converted it"
    );
    assert!(
        head.contains("\r\nsecond line"),
        "the head must be CRLF inside the literal"
    );
    assert_case("crlf_head", RsKind::Comments, Some(false));
}

#[test]
fn a_crlf_only_difference_is_same() {
    let (base, head) = pair("crlf_only");
    assert!(
        !base.contains('\r') && head.contains("\r\n"),
        "the fixture lost its EOLs"
    );
    assert_case("crlf_only", RsKind::Same, None);
}

#[test]
fn a_leading_bom_is_same() {
    let (_, head) = pair("bom_only");
    assert!(head.starts_with('\u{feff}'), "the fixture lost its BOM");
    assert_case("bom_only", RsKind::Same, None);
}

#[test]
fn identical_sides_are_same() {
    assert_case("identical", RsKind::Same, None);
}

#[test]
fn a_doc_edit_inside_a_macro_invocation_is_code() {
    assert_case("doc_in_macro_invocation", RsKind::Code, None);
}

#[test]
fn a_doc_edit_inside_a_macro_rules_body_is_code() {
    assert_case("macro_rules_body", RsKind::Code, None);
}

#[test]
fn a_doc_edit_a_macro_captures_as_a_literal_is_code() {
    assert_case("doc_literal_capture", RsKind::Code, None);
}

#[test]
fn a_doc_edit_inside_stringify_is_code() {
    assert_case("stringify_doc", RsKind::Code, None);
}

#[test]
fn a_derive_changed_beside_a_doc_inside_cfg_attr_is_code() {
    assert_case("cfg_attr_derive", RsKind::Code, None);
}

/// Only the `doc` entries of a `cfg_attr` go, at any depth; an attribute left
/// with no entry goes with them, and a trailing comma is kept.
#[test]
fn doc_entries_inside_cfg_attr_are_stripped_and_the_rest_compared() {
    assert_case("cfg_attr_doc", RsKind::Comments, Some(true));
}

#[test]
fn a_doc_edit_under_a_derive_outside_the_allowlist_is_code() {
    assert_case("derive_parser", RsKind::Code, None);
}

#[test]
fn a_plain_comment_edit_under_a_derive_outside_the_allowlist_is_comments() {
    assert_case("derive_parser_plain", RsKind::Comments, Some(false));
}

#[test]
fn a_doc_edit_under_allowlisted_derives_and_their_helpers_is_comments() {
    assert_case("derive_allowlisted", RsKind::Comments, Some(true));
}

#[test]
fn a_doc_edit_in_a_file_with_an_attribute_macro_is_code() {
    assert_case("attribute_macro", RsKind::Code, None);
}

#[test]
fn any_change_to_a_file_calling_line_is_code() {
    assert_case("line_macro", RsKind::Code, None);
}

#[test]
fn any_change_to_a_file_calling_location_caller_is_code() {
    assert_case("location_caller", RsKind::Code, None);
}

/// The premise reads tokens: `line!()` in doc text, a comment or a string and
/// a variable `line` compared with `!=` are no call.
#[test]
fn line_in_comments_strings_and_a_not_equal_is_no_premise() {
    assert_case("line_in_text", RsKind::Comments, Some(true));
}

/// The regex-stripper trap: `//` inside a string is no comment.
#[test]
fn a_string_holding_two_slashes_is_code() {
    assert_case("string_with_slashes", RsKind::Code, None);
}

#[test]
fn a_raw_string_holding_a_block_comment_is_code() {
    assert_case("raw_string_block_end", RsKind::Code, None);
}

#[test]
fn a_byte_string_holding_two_slashes_is_code() {
    assert_case("byte_string", RsKind::Code, None);
}

#[test]
fn a_comment_after_a_lifetime_and_a_char_literal_is_comments() {
    assert_case("lifetime_and_char", RsKind::Comments, Some(false));
}

#[test]
fn a_base_that_does_not_parse_is_code() {
    assert_case("base_unparseable", RsKind::Code, None);
}

#[test]
fn a_head_that_does_not_parse_is_code() {
    assert_case("head_unparseable", RsKind::Code, None);
}

#[test]
fn a_changed_literal_is_code() {
    assert_case("code_change", RsKind::Code, None);
}

#[test]
fn a_removed_doc_hidden_is_comments() {
    assert_case("doc_hidden", RsKind::Comments, Some(true));
}

#[test]
fn a_doc_include_str_is_a_doc_attribute() {
    assert_case("doc_include", RsKind::Comments, Some(true));
}

/// The stripped doc attributes become this sentinel and are dropped from the
/// printed stream, so a source that spells it could hide a real attribute. The
/// attribute-macro premise would grade this pair `Code` too; the reason pins
/// that the sentinel guard, which runs first, is what fires.
#[test]
fn a_source_holding_the_sentinel_is_code() {
    assert_case("sentinel_in_source", RsKind::Code, None);
    let (base, head) = pair("sentinel_in_source");
    let reason = classify_rs_verdict(&base, &head).reason;
    assert!(reason.contains("sentinel"), "{reason}");
}

/// Every fixture directory is exercised by a test above, so a new pair cannot
/// sit unasserted.
#[test]
fn every_fixture_pair_is_asserted() {
    let this = include_str!("fixtures.rs");
    let dir: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures"]
        .iter()
        .collect();
    let mut cases: Vec<String> = std::fs::read_dir(&dir)
        .expect("the fixtures directory")
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    cases.sort();
    assert_eq!(cases.len(), 32, "fixture directories: {cases:?}");
    for case in &cases {
        assert!(
            this.contains(&format!("(\"{case}\"")),
            "fixture {case} has no assertion"
        );
    }
}
