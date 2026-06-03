use super::{collect_candidates, is_plain_ident, take_backticked_ident};

#[test]
fn extracts_a_bare_backticked_identifier() {
    let candidates = collect_candidates("Installs `PackageManifest` into `Store`.");
    let idents: Vec<&str> = candidates
        .iter()
        .map(|candidate| candidate.ident.as_str())
        .collect();
    assert_eq!(idents, ["PackageManifest", "Store"]);
}

#[test]
fn skips_an_already_linked_span() {
    let candidates = collect_candidates("See [`Store`] and [`Foo`](crate::Foo).");
    assert!(candidates.is_empty(), "linked spans are not candidates");
}

#[test]
fn skips_code_blocks() {
    let text = "```\nlet x = `Foo`;\n```\n";
    assert!(collect_candidates(text).is_empty());
}

#[test]
fn span_covers_the_whole_code_span() {
    let text = "x `Foo` y";
    let candidates = collect_candidates(text);
    assert_eq!(candidates.len(), 1);
    assert_eq!(&text[candidates[0].span.clone()], "`Foo`");
}

#[test]
fn rejects_non_identifier_bodies() {
    assert_eq!(take_backticked_ident("`a + b`"), None);
    assert_eq!(take_backticked_ident("`foo::Bar`"), None);
    assert_eq!(take_backticked_ident("`foo()`"), None);
    assert_eq!(take_backticked_ident("``"), None);
    assert_eq!(take_backticked_ident("`_`"), None);
}

#[test]
fn accepts_padded_double_backtick_span() {
    assert_eq!(take_backticked_ident("`` Foo ``").as_deref(), Some("Foo"));
}

#[test]
fn rejects_multi_line_span() {
    // A code span wrapping a soft line break is not a clean inline link
    // candidate and its source span would not map back contiguously.
    assert_eq!(take_backticked_ident("`\nFoo\n`"), None);
}

#[test]
fn rejects_over_padded_span() {
    // CommonMark strips at most one space per side, so `  Foo  ` keeps a
    // space and is not a bare identifier.
    assert_eq!(take_backticked_ident("`  Foo  `"), None);
    assert_eq!(take_backticked_ident("`\tFoo\t`"), None);
    // The single-space padded form is still a candidate.
    assert_eq!(take_backticked_ident("` Foo `").as_deref(), Some("Foo"));
}

#[test]
fn plain_ident_predicate() {
    assert!(is_plain_ident("Foo"));
    assert!(is_plain_ident("snake_case_2"));
    assert!(is_plain_ident("_leading"));
    assert!(!is_plain_ident("_"));
    assert!(!is_plain_ident("2foo"));
    assert!(!is_plain_ident(""));
}
