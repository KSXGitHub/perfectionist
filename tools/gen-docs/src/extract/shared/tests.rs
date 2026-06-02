use std::path::PathBuf;

use super::*;

/// Allocate a fresh temp directory unique across both processes
/// and across tests in the same binary, matching the helper in
/// `extract.rs`'s own tests.
fn tempdir(label: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!(
        "perfectionist-gen-docs-shared-{label}-{}-{seq}",
        std::process::id(),
    ));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();
    base
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
