use super::{NamePattern, verdict};

fn parse(pattern: &str) -> Result<NamePattern, String> {
    NamePattern::try_from(pattern.to_owned())
}

/// The verdict a one-entry list carries for `name`, or `None` where
/// the entry does not cover the name at all.
fn single(pattern: &str, name: &str) -> Option<bool> {
    list(&[pattern], name)
}

/// The verdict a whole list carries for `name`, through the same
/// resolution a rule uses.
fn list(patterns: &[&str], name: &str) -> Option<bool> {
    let parsed: Vec<NamePattern> = patterns
        .iter()
        .map(|pattern| parse(pattern).unwrap_or_else(|error| panic!("{pattern:?}: {error}")))
        .collect();
    verdict(&parsed, name)
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
        assert_eq!(single("*", name), Some(true), "{name}");
        assert_eq!(single("!*", name), Some(false), "{name}");
    }
}

#[test]
fn a_prefix_covers_only_the_names_under_it() {
    assert_eq!(single("clone_*", "clone_id"), Some(true));
    assert_eq!(single("clone_*", "clone_"), Some(true));
    assert_eq!(single("clone_*", "first_name"), None);
}

#[test]
fn a_prefix_is_matched_to_its_underscore() {
    // `cloned_id` starts with `clone` but not with `clone_`, which is
    // the whole point of requiring the underscore.
    assert_eq!(single("clone_*", "cloned_id"), None);
    assert_eq!(single("cloned_*", "cloned_id"), Some(true));
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

#[test]
fn the_last_entry_covering_a_name_is_the_one_that_decides() {
    assert_eq!(list(&["*", "!clone_*"], "clone_id"), Some(false));
    assert_eq!(list(&["*", "!clone_*"], "first_name"), Some(true));
    // Reversing them reverses the answer: order is the whole rule.
    assert_eq!(list(&["!clone_*", "*"], "clone_id"), Some(true));
}

#[test]
fn a_name_no_entry_covers_is_left_undecided() {
    // `None` is not `false`: it is the caller's fallthrough that
    // decides, which for `cloning_getter` is the field match.
    assert_eq!(list(&["get_*"], "first_name"), None);
    assert_eq!(list(&[], "first_name"), None);
}

#[test]
fn a_leading_negation_clears_what_a_fallthrough_would_have_said() {
    // `!*` covers every name, so nothing is left undecided and the
    // caller's fallthrough never runs; a later entry takes one shape
    // back.
    assert_eq!(list(&["!*", "get_*"], "get_first_name"), Some(true));
    assert_eq!(list(&["!*", "get_*"], "first_name"), Some(false));
}

#[test]
fn a_leading_bang_flips_the_verdict_and_not_the_scope() {
    // `clone_*` and `!clone_*` cover exactly the same names, so they
    // leave exactly the same names undecided and differ only in the
    // answer they give for a name they cover. That is why both take
    // their turn at the same point in the scan.
    assert_eq!(single("clone_*", "clone_id"), Some(true));
    assert_eq!(single("!clone_*", "clone_id"), Some(false));
    for name in ["first_name", "get_thing", "cloned_id"] {
        assert_eq!(single("clone_*", name), None, "{name}");
        assert_eq!(single("!clone_*", name), None, "{name}");
    }
}
