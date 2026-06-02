use super::{Case, classify};

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
