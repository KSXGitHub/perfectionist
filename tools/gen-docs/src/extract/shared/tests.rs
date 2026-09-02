use super::SharedTypes;
use std::fs;
use std::path::{Path, PathBuf};

/// Allocate a fresh temp directory of this module's own, so a
/// `label` it shares with another test module still gets a
/// directory to itself.
fn tempdir(label: &str) -> PathBuf {
    _utils::scratch::dir(&format!("gen-docs-shared-{label}"))
}

#[test]
fn discover_picks_up_struct_and_label_pair() {
    let base = tempdir("happy-path");
    fs::write(
        base.join("ascii_letter.rs"),
        r#"
            pub(crate) const TOML_LABEL: &str = "single-letter string";

            #[derive(serde::Deserialize)]
            pub(crate) struct AsciiLetter(char);
        "#,
    )
    .unwrap();
    let shared = SharedTypes::discover(&base);
    assert_eq!(
        shared.label_for("AsciiLetter"),
        Some("single-letter string"),
    );
    assert!(shared.contains("AsciiLetter"));
    assert_eq!(shared.label_for("Unrelated"), None);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn discover_skips_files_with_no_toml_label_constant() {
    let base = tempdir("no-label");
    fs::write(
        base.join("common.rs"),
        r#"
            struct One;
            struct Two;
        "#,
    )
    .unwrap();
    let shared = SharedTypes::discover(&base);
    assert!(shared.label_for("One").is_none());
    assert!(shared.label_for("Two").is_none());
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn discover_skips_files_with_multiple_structs() {
    let base = tempdir("ambiguous");
    fs::write(
        base.join("mixed.rs"),
        r#"
            pub(crate) const TOML_LABEL: &str = "demo label";

            struct One;
            struct Two;
        "#,
    )
    .unwrap();
    let shared = SharedTypes::discover(&base);
    assert!(shared.label_for("One").is_none());
    assert!(shared.label_for("Two").is_none());
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn discover_returns_empty_for_missing_src_dir() {
    let shared = SharedTypes::discover(Path::new("/nonexistent/path/that/does/not/exist"));
    assert!(shared.label_for("AsciiLetter").is_none());
}
