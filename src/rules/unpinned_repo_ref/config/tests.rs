use super::glob_match;

#[test]
fn exact_match_is_case_insensitive() {
    assert!(glob_match("github.com", "github.com"));
    assert!(glob_match("GitHub.com", "github.com"));
    assert!(glob_match("github.com", "GITHUB.COM"));
}

#[test]
fn non_matching_literal_is_rejected() {
    assert!(!glob_match("github.com", "gitlab.com"));
    assert!(!glob_match("github.com", "github.co"));
    assert!(!glob_match("github.com", "github.command"));
}

#[test]
fn star_matches_a_single_label() {
    assert!(glob_match("gitlab.*.example.com", "gitlab.eu.example.com"));
    assert!(glob_match(
        "gitlab.*.example.com",
        "gitlab.us-west.example.com"
    ));
}

#[test]
fn star_matches_across_dots_and_empty_run() {
    assert!(glob_match("*.example.com", "git.eu.example.com"));
    // A trailing `*` swallows the rest, including nothing at all.
    assert!(glob_match("git.*", "git.sr.ht"));
    assert!(glob_match("github*", "github.com"));
    assert!(glob_match("github.com*", "github.com"));
}

#[test]
fn leading_star_matches() {
    assert!(glob_match("*example.com", "example.com"));
    assert!(glob_match("*example.com", "git.example.com"));
    assert!(!glob_match("*example.com", "example.org"));
}

#[test]
fn bare_star_matches_anything() {
    assert!(glob_match("*", ""));
    assert!(glob_match("*", "anything.at.all"));
}
