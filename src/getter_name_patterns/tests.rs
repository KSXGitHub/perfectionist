use super::GetterNamePatterns;

fn parse(patterns: &[&str]) -> Result<GetterNamePatterns, String> {
    GetterNamePatterns::try_from(
        patterns
            .iter()
            .copied()
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    )
}

fn rejection(patterns: &[&str]) -> String {
    parse(patterns)
        .err()
        .unwrap_or_else(|| panic!("{patterns:?} should be rejected"))
}

#[test]
fn a_list_opening_with_a_total_pattern_is_accepted() {
    for patterns in [
        &["*"][..],
        &["!*"][..],
        &["!*", "get_*"][..],
        &["*", "!clone_*"][..],
        &["!*", "foo_*", "!foo_bar_*"][..],
    ] {
        assert!(parse(patterns).is_ok(), "{patterns:?} should parse");
    }
}

#[test]
fn an_empty_list_is_rejected() {
    let error = rejection(&[]);
    assert!(error.contains("at least one pattern"), "{error}");
    assert!(error.contains(r#"["!*"]"#), "{error}");
}

#[test]
fn a_list_opening_with_a_prefix_is_rejected() {
    // Every one of these would rest on an unstated baseline for the
    // names it does not mention.
    for patterns in [
        &["get_*"][..],
        &["!clone_*"][..],
        &["foo_*", "!foo_bar_*"][..],
        &["!foo_*", "foo_bar_*"][..],
        &["foo_*", "*"][..],
    ] {
        let error = rejection(patterns);
        assert!(
            error.contains(r#"`"*"` or `"!*"`"#),
            "{patterns:?}: {error}",
        );
    }
}

#[test]
fn the_list_always_has_an_answer() {
    let patterns = parse(&["!*", "get_*"]).expect("should parse");
    assert!(patterns.says_getter("get_name"));
    assert!(!patterns.says_getter("name"));
    assert!(!patterns.says_getter("anything_at_all"));
}

#[test]
fn a_prefix_under_a_conversion_prefix_is_rejected() {
    // The conversion clause tests the name's own prefix, so every one
    // of these covers only names it has already answered for — whether
    // the entry names the conversion prefix exactly, extends it, or
    // negates either.
    for entry in [
        "as_*",
        "to_*",
        "into_*",
        "!as_*",
        "!to_*",
        "!into_*",
        "as_ref_*",
        "to_owned_*",
        "into_iter_*",
        "!as_ref_*",
        "!to_owned_*",
        "!into_iter_*",
    ] {
        let error = rejection(&["!*", entry]);
        assert!(error.contains("conversion prefix"), "{entry:?}: {error}");
        assert!(error.contains("no effect"), "{entry:?}: {error}");
    }
}

#[test]
fn a_prefix_merely_starting_like_a_conversion_prefix_is_accepted() {
    // The test is on the prefix, not on its first letters: `to_*` is
    // rejected, `token_*` is not.
    for entry in ["token_*", "assemble_*", "intolerant_*"] {
        assert!(parse(&["!*", entry]).is_ok(), "{entry:?} should parse");
    }
}

#[test]
fn a_malformed_entry_is_reported_as_malformed() {
    // The grammar runs before the ban, so an entry that is both
    // ill-formed and conversion-shaped is reported for its shape.
    let error = rejection(&["!*", "to_owned"]);
    assert!(error.contains("closed by `*`"), "{error}");
}

#[test]
fn the_documented_examples_mean_what_they_say() {
    // `["*", "!clone_*", "!cloned_*"]` measures every other name,
    // except one whose own prefix says it copies.
    let broad = parse(&["*", "!clone_*", "!cloned_*"]).expect("should parse");
    assert!(broad.says_getter("unrelated"));
    assert!(broad.says_getter("get_thing"));
    assert!(!broad.says_getter("clone_url"));
    assert!(!broad.says_getter("cloned_at"));

    // `["!*", "get_*"]` measures the `get_*` names and no other.
    let narrow = parse(&["!*", "get_*"]).expect("should parse");
    assert!(narrow.says_getter("get_thing"));
    assert!(!narrow.says_getter("unrelated"));
    assert!(!narrow.says_getter("clone_url"));
    assert!(!narrow.says_getter("cloned_at"));
}
