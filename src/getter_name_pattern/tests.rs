use super::GetterNamePattern;

fn parse(pattern: &str) -> Result<GetterNamePattern, String> {
    GetterNamePattern::try_from(pattern.to_owned())
}

fn rejection(pattern: &str) -> String {
    parse(pattern)
        .err()
        .unwrap_or_else(|| panic!("{pattern:?} should be rejected"))
}

#[test]
fn a_well_formed_pattern_comes_through_the_wrapper() {
    let pattern = parse("get_*").expect("`get_*` should parse");
    assert!(pattern.matches("get_name"));
    assert!(pattern.selects());
    assert!(!pattern.matches("name"));
}

#[test]
fn a_malformed_pattern_is_reported_as_malformed() {
    // The grammar runs first, so an entry that is both ill-formed and
    // conversion-shaped is reported for its shape rather than for the
    // ban.
    let error = rejection("to_owned");
    assert!(error.contains("closed by `*`"), "{error}");
}

#[test]
fn a_prefix_under_a_conversion_prefix_is_rejected() {
    // The conversion clause tests the name's own prefix, so every one
    // of these covers only names it has already answered for --
    // whether the entry names the conversion prefix exactly, extends
    // it, or negates either.
    for pattern in [
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
        let error = rejection(pattern);
        assert!(error.contains("conversion prefix"), "{pattern:?}: {error}");
        assert!(error.contains("no effect"), "{pattern:?}: {error}");
    }
}

#[test]
fn a_prefix_merely_starting_like_a_conversion_prefix_is_accepted() {
    // The test is on the prefix, not on its first letters: `to_*` is
    // rejected, `token_*` is not.
    for pattern in ["token_*", "assemble_*", "intolerant_*"] {
        assert!(parse(pattern).is_ok(), "{pattern:?} should parse");
    }
}
