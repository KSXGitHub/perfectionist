use super::{is_self_entry, render_tree};

/// `(path, rename suffix)` pairs, spelled the way
/// [`super::Statement::rewrite`] builds them.
fn tree(entries: &[(&str, &str)]) -> String {
    render_tree(entries)
}

#[test]
fn lone_entry_needs_no_braces() {
    assert_eq!(tree(&[("crate::thing::A", "")]), "crate::thing::A");
    assert_eq!(
        tree(&[("crate::thing::A", " as Alias")]),
        "crate::thing::A as Alias",
    );
}

#[test]
fn shared_prefix_is_folded_out() {
    assert_eq!(
        tree(&[("diesel::table", ""), ("diesel::AsChangeset", "")]),
        "diesel::{table, AsChangeset}",
    );
    assert_eq!(
        tree(&[("crate::thing::A", " as X"), ("crate::thing::helper", "")]),
        "crate::thing::{A as X, helper}",
    );
}

#[test]
fn partial_prefix_folds_only_as_far_as_it_is_shared() {
    assert_eq!(
        tree(&[("crate::thing::A", ""), ("crate::prelude::*", "")]),
        "crate::{thing::A, prelude::*}",
    );
}

#[test]
fn entries_from_different_crates_get_a_top_level_brace() {
    assert_eq!(
        tree(&[("serde::Serialize", ""), ("diesel::table", "")]),
        "{serde::Serialize, diesel::table}",
    );
}

/// The shared prefix may never swallow an entry whole: two paths where
/// one is a prefix of the other still leave each entry a last segment.
#[test]
fn shared_prefix_leaves_every_entry_a_tail() {
    assert_eq!(
        tree(&[("crate::thing", ""), ("crate::thing::A", "")]),
        "crate::{thing, thing::A}",
    );
}

/// A leading `::` is one empty first segment, so it is shared exactly
/// when both entries carry it.
#[test]
fn extern_crate_root_is_preserved() {
    assert_eq!(
        tree(&[("::serde::A", ""), ("::serde::B", "")]),
        "::serde::{A, B}",
    );
    assert_eq!(
        tree(&[("::serde::A", ""), ("crate::thing::B", "")]),
        "{::serde::A, crate::thing::B}",
    );
}

#[test]
fn written_order_is_kept() {
    assert_eq!(
        tree(&[("crate::thing::z", ""), ("crate::thing::a", "")]),
        "crate::thing::{z, a}",
    );
}

#[test]
fn self_entries_are_recognised() {
    assert!(is_self_entry("self"));
    assert!(is_self_entry("crate::prelude::self"));
    assert!(!is_self_entry("crate::prelude::A"));
    // A name merely *containing* `self` is not the keyword.
    assert!(!is_self_entry("crate::prelude::myself"));
}
