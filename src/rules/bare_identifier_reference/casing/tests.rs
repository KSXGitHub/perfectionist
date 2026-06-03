use super::{Case, classify, word_count};

#[test]
fn counts_words() {
    assert_eq!(word_count("foo", Case::Snake), 1);
    assert_eq!(word_count("foo_bar", Case::Snake), 2);
    assert_eq!(word_count("foo_bar_baz", Case::Snake), 3);
    assert_eq!(word_count("FOO", Case::Upper), 1);
    assert_eq!(word_count("FOO_BAR_BAZ", Case::Upper), 3);
    assert_eq!(word_count("Foo", Case::Pascal), 1);
    assert_eq!(word_count("FooBar", Case::Pascal), 2);
    assert_eq!(word_count("FooBarBaz", Case::Pascal), 3);
    assert_eq!(word_count("HttpServer", Case::Pascal), 2);
    // Acronym boundary: `HTTP` + `Server`.
    assert_eq!(word_count("HTTPServer", Case::Pascal), 2);
}

#[test]
fn snake_case() {
    assert_eq!(classify("foo"), Case::Snake);
    assert_eq!(classify("foo_bar"), Case::Snake);
    assert_eq!(classify("foo_bar_baz"), Case::Snake);
    assert_eq!(classify("foo2"), Case::Snake);
    assert_eq!(classify("x"), Case::Snake);
}

#[test]
fn upper_case() {
    assert_eq!(classify("FOO"), Case::Upper);
    assert_eq!(classify("FOO_BAR"), Case::Upper);
    assert_eq!(classify("HTTP"), Case::Upper);
}

#[test]
fn pascal_case() {
    assert_eq!(classify("Foo"), Case::Pascal);
    assert_eq!(classify("FooBar"), Case::Pascal);
    assert_eq!(classify("HttpServer"), Case::Pascal);
    assert_eq!(classify("Foo2Bar"), Case::Pascal);
}

#[test]
fn non_conformist() {
    assert_eq!(classify("fooBar"), Case::NonConformist);
    assert_eq!(classify("foo_BAR"), Case::NonConformist);
    assert_eq!(classify("Foo_bar"), Case::NonConformist);
    assert_eq!(classify("foo__bar"), Case::NonConformist);
    assert_eq!(classify("__foo_bar"), Case::NonConformist);
    assert_eq!(classify("foo_"), Case::NonConformist);
    assert_eq!(classify(""), Case::NonConformist);
}
