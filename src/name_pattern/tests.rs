use super::NamePattern;

fn parse(pattern: &str) -> Result<NamePattern, String> {
    NamePattern::try_from(pattern.to_owned())
}

/// The verdict `pattern` carries for `name`, or `None` where the
/// pattern does not cover the name at all and the rule would look
/// further down its list.
fn verdict(pattern: &str, name: &str) -> Option<bool> {
    let pattern = parse(pattern).unwrap_or_else(|error| panic!("{pattern:?}: {error}"));
    pattern.matches(name).then(|| pattern.selects())
}

fn rejection(pattern: &str) -> String {
    parse(pattern)
        .err()
        .unwrap_or_else(|| panic!("{pattern:?} should be rejected"))
}

#[test]
fn each_of_the_four_forms_parses() {
    for pattern in ["*", "!*", "get_*", "!clone_*"] {
        assert!(parse(pattern).is_ok(), "{pattern:?} should parse");
    }
}

#[test]
fn a_bare_wildcard_covers_every_name() {
    for name in ["first_name", "get_anything", "x"] {
        assert_eq!(verdict("*", name), Some(true), "{name}");
        assert_eq!(verdict("!*", name), Some(false), "{name}");
    }
}

#[test]
fn a_prefix_covers_only_the_names_under_it() {
    assert_eq!(verdict("clone_*", "clone_id"), Some(true));
    assert_eq!(verdict("clone_*", "clone_"), Some(true));
    assert_eq!(verdict("clone_*", "first_name"), None);
}

#[test]
fn a_prefix_is_matched_to_its_underscore() {
    // `cloned_id` starts with `clone` but not with `clone_`, which is
    // the whole point of requiring the underscore.
    assert_eq!(verdict("clone_*", "cloned_id"), None);
    assert_eq!(verdict("cloned_*", "cloned_id"), Some(true));
}

#[test]
fn a_leading_bang_flips_the_verdict_and_not_the_scope() {
    assert_eq!(verdict("!clone_*", "clone_id"), Some(false));
    // The names it does not cover are left to the rest of the list,
    // exactly as they are without the `!`.
    assert_eq!(verdict("!clone_*", "first_name"), None);
}

#[test]
fn a_value_that_could_not_begin_a_pattern_is_rejected() {
    for pattern in ["", "!", "!!clone_*", "**", "*_suffix", "1st_*"] {
        let error = rejection(pattern);
        assert!(
            error.contains("expected `*`, `prefix_*`"),
            "{pattern:?}: {error}",
        );
    }
}

#[test]
fn a_prefix_with_no_closing_wildcard_is_rejected() {
    for pattern in ["clone", "clone_"] {
        let error = rejection(pattern);
        assert!(error.contains("closed by `*`"), "{pattern:?}: {error}");
    }
}

#[test]
fn text_after_the_wildcard_is_rejected() {
    for pattern in ["clone_*_suffix", "clone_**", "clone_*_*"] {
        let error = rejection(pattern);
        assert!(error.contains("ends the pattern"), "{pattern:?}: {error}");
        assert!(
            error.contains("never by its middle"),
            "{pattern:?}: {error}",
        );
    }
}

#[test]
fn a_prefix_without_the_underscore_is_rejected() {
    let error = rejection("prefix*");
    assert!(error.contains("ends in `_`"), "{error}");
    assert!(error.contains("partway through a word"), "{error}");
}

#[test]
fn a_prefix_with_nothing_before_the_underscore_is_rejected() {
    let error = rejection("_*");
    assert!(error.contains("at least one character"), "{error}");
}

#[test]
fn a_prefix_must_be_spelled_the_way_a_method_name_could_be() {
    // A non-ASCII identifier is one a method could be named with, so
    // a prefix may be spelled with one.
    assert!(parse("café_*").is_ok(), "a non-ASCII prefix should parse");
    // Anything a method could not be named with is rejected, whether
    // the offending character opens the prefix or sits inside it.
    for pattern in ["9th_*", "foo-bar_*", "foo.bar_*", "foo bar_*"] {
        let error = rejection(pattern);
        assert!(
            error.contains("expected `*`, `prefix_*`"),
            "{pattern:?}: {error}",
        );
    }
}

#[test]
fn the_language_itself_carries_no_policy() {
    // A rule that has to forbid some entry does it in a newtype of its
    // own; every well-formed pattern parses here, conversion prefixes
    // included. `crate::getter_name_pattern` is where those are
    // rejected.
    for pattern in ["as_*", "to_*", "into_*", "!as_ref_*"] {
        assert!(parse(pattern).is_ok(), "{pattern:?} should parse");
    }
}
