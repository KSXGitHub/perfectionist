use super::crate_is_linked;

/// `crate::` reaches the local crate from anywhere in it, whatever the
/// rewritten `use` was rooted at.
#[test]
fn local_crate_is_always_linked() {
    assert!(crate_is_linked("crate::thing::A", Some("crate")));
    assert!(crate_is_linked("crate::thing::A", Some("self")));
    assert!(crate_is_linked("crate::thing::A", None));
}

/// The common case: a crate's own prelude re-exporting its own items.
/// The importer named the crate, so it is linked here.
#[test]
fn a_crate_the_import_already_names_is_linked() {
    assert!(crate_is_linked("serde::Serialize", Some("serde")));
    assert!(crate_is_linked("diesel::table", Some("diesel")));
}

/// The `std` / `alloc` shape: `std`'s prelude re-exports items defined
/// in `alloc`, which an ordinary crate cannot name.
#[test]
fn a_crate_only_reached_through_the_re_export_is_not_linked() {
    assert!(!crate_is_linked("alloc::vec::Vec", Some("std")));
    assert!(!crate_is_linked("serde::Serialize", Some("crate")));
    assert!(!crate_is_linked("serde::Serialize", None));
}

/// A crate root is a whole segment, never a prefix of one.
#[test]
fn crate_roots_match_whole_segments() {
    assert!(!crate_is_linked("serde_json::Value", Some("serde")));
    assert!(!crate_is_linked("crate_thing::A", Some("crate")));
}
