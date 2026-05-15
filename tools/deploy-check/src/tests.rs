use text_block_macros::text_block_fnl;

use super::contract::assert_version_only_diff;
use super::error::RuntimeError;
use super::manifest::{parse_cargo_lock_version, parse_cargo_toml_version};
use super::version_literal::is_version_literal;

#[test]
fn version_literal_grammar() {
    for ok in ["0.0.0", "1.2.3", "0.0.0-rc.13", "10.20.30-beta.4+sha.abc"] {
        assert!(is_version_literal(ok), "{ok:?} should be accepted");
    }
    for bad in [
        "",
        "1.2",
        "1.2.3.4",
        "v1.2.3",
        "1.2.3-",
        "1.2.3 ",
        " 1.2.3",
        "1.2.3-rc 1",
        "1.a.0",
        "1.2.3+meta",
    ] {
        assert!(!is_version_literal(bad), "{bad:?} should be rejected");
    }
}

#[test]
fn cargo_toml_version_extracted_from_package_section() {
    let manifest = text_block_fnl! {
        "[workspace]"
        r#"members = ["x"]"#
        ""
        "[package]"
        r#"name = "perfectionist""#
        r#"version = "0.0.0-rc.7""#
    };
    assert_eq!(parse_cargo_toml_version(manifest).unwrap(), "0.0.0-rc.7");
}

#[test]
fn cargo_lock_version_picks_perfectionist_entry() {
    let lock = text_block_fnl! {
        "[[package]]"
        r#"name = "foo""#
        r#"version = "1.0.0""#
        ""
        "[[package]]"
        r#"name = "perfectionist""#
        r#"version = "0.0.0-rc.5""#
        "dependencies = ["
        r#" "foo","#
        "]"
        ""
        "[[package]]"
        r#"name = "bar""#
        r#"version = "2.0.0""#
    };
    assert_eq!(parse_cargo_lock_version(lock).unwrap(), "0.0.0-rc.5");
}

#[test]
fn cargo_lock_rejects_duplicate_perfectionist_entries() {
    let lock = text_block_fnl! {
        "[[package]]"
        r#"name = "perfectionist""#
        r#"version = "0.0.0-rc.1""#
        ""
        "[[package]]"
        r#"name = "perfectionist""#
        r#"version = "0.0.0-rc.2""#
    };
    assert!(matches!(
        parse_cargo_lock_version(lock),
        Err(RuntimeError::DuplicateLockPackageEntry)
    ));
}

#[test]
fn version_only_diff_accepts_a_pure_version_bump() {
    let before = text_block_fnl! {
        "[package]"
        r#"name = "perfectionist""#
        r#"version = "0.0.0-rc.6""#
    };
    let after = text_block_fnl! {
        "[package]"
        r#"name = "perfectionist""#
        r#"version = "0.0.0-rc.7""#
    };
    assert_version_only_diff("Cargo.toml", before, after, "0.0.0-rc.6", "0.0.0-rc.7").unwrap();
}

#[test]
fn version_only_diff_rejects_collateral_edits() {
    let before = text_block_fnl! {
        "[package]"
        r#"name = "perfectionist""#
        r#"version = "0.0.0-rc.6""#
    };
    let after = text_block_fnl! {
        "[package]"
        r#"name = "renamed""#
        r#"version = "0.0.0-rc.7""#
    };
    // The first differing line is `name = ...`, which doesn't
    // match the expected version-line text — so the rejection
    // is `UnexpectedBeforeLine`, not `ExtraLineChanged`.
    assert!(matches!(
        assert_version_only_diff("Cargo.toml", before, after, "0.0.0-rc.6", "0.0.0-rc.7"),
        Err(RuntimeError::UnexpectedBeforeLine { .. })
    ));
}

#[test]
fn version_only_diff_rejects_an_extra_changed_line_after_the_version() {
    let before = text_block_fnl! {
        "[package]"
        r#"version = "0.0.0-rc.6""#
        r#"description = "a""#
    };
    let after = text_block_fnl! {
        "[package]"
        r#"version = "0.0.0-rc.7""#
        r#"description = "b""#
    };
    assert!(matches!(
        assert_version_only_diff("Cargo.toml", before, after, "0.0.0-rc.6", "0.0.0-rc.7"),
        Err(RuntimeError::ExtraLineChanged { .. })
    ));
}

#[test]
fn version_only_diff_rejects_line_count_mismatch() {
    let before = text_block_fnl! { r#"version = "0.0.0-rc.6""# };
    let after = text_block_fnl! {
        r#"version = "0.0.0-rc.7""#
        "extra"
    };
    assert!(matches!(
        assert_version_only_diff("Cargo.toml", before, after, "0.0.0-rc.6", "0.0.0-rc.7"),
        Err(RuntimeError::LineCountChanged(_))
    ));
}

/// The contract requires the two versions to be *different*; it
/// does not require monotonicity or SemVer ordering. A downgrade
/// (rc.20 → rc.10), a mainline-to-prerelease move (0.1.0 → 0.0.0-rc.5),
/// or any other distinct pair is accepted as long as nothing else
/// in the snapshot changes.
#[test]
fn version_only_diff_accepts_arbitrary_distinct_versions() {
    for (before_ver, after_ver) in [
        ("0.0.0-rc.36", "0.0.0-rc.20"),
        ("0.1.0", "0.0.0-rc.88"),
        ("0.0.1", "0.0.0-rc.72"),
        ("0.1.2", "0.2.3"),
    ] {
        let before = format!("[package]\nversion = \"{before_ver}\"\n");
        let after = format!("[package]\nversion = \"{after_ver}\"\n");
        assert_version_only_diff("Cargo.toml", &before, &after, before_ver, after_ver)
            .unwrap_or_else(|err| panic!("{before_ver} -> {after_ver} should be accepted: {err}"));
    }
}

#[test]
fn version_only_diff_rejects_byte_identical_inputs() {
    let same = text_block_fnl! { r#"version = "0.0.0-rc.6""# };
    assert!(matches!(
        assert_version_only_diff("Cargo.toml", same, same, "0.0.0-rc.6", "0.0.0-rc.6"),
        Err(RuntimeError::NoVersionChange(_))
    ));
}
