//! Discover shared-module type definitions whose TOML field-type
//! label is sourced from their definition site rather than baked into
//! [`super::ty::toml_type_label`]'s match arms.
//!
//! Today the only shared type with a definition-site label is
//! `AsciiLetter` in `src/ascii_letter.rs`, used by the
//! `single_letter_*` rules' allow-list knobs; the scan generalises so
//! any future shared newtype that wants its own label (a stricter
//! string subtype, an integer refinement, …) only needs to pair a
//! `pub(crate) const TOML_LABEL: &str = "...";` constant with the
//! struct it labels.
//!
//! The convention enforced here is the smallest one that gives the
//! generator an unambiguous mapping:
//!
//! - the scan only reads `src/*.rs` (the *immediate* children of
//!   `src/`), not the per-rule `rules/` subdirectory, so a rule file
//!   that happens to declare an internal `TOML_LABEL` constant does
//!   not leak it into the shared-types table;
//! - a file contributes a mapping only when it declares exactly one
//!   struct alongside the `TOML_LABEL` constant — a file with more
//!   than one struct is skipped because there is no way to tell
//!   which struct the label is for.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use syn::{Expr, ExprLit, Item, Lit};

/// Map of Rust struct identifier → TOML-flavoured type label,
/// populated from shared modules under `src/`. Threaded through
/// [`super::config::extract_config`], [`super::ty::toml_type_label`],
/// and [`super::ty::collect_referenced_idents`] so the renderer
/// surfaces the type's own label and skips the type from the
/// per-rule custom-types listing (since the label already encodes
/// the user-visible shape).
#[derive(Clone, Debug, Default)]
pub(crate) struct SharedTypes {
    labels: HashMap<String, String>,
}

impl SharedTypes {
    /// Walk `src_dir` non-recursively, parsing every top-level
    /// `.rs` file and recording the `(struct ident → TOML_LABEL
    /// value)` pairings present. Returns an empty table when
    /// `src_dir` is not a readable directory, when no file declares
    /// a `TOML_LABEL` constant, or when every candidate file fails
    /// the one-struct convention; downstream callers treat the
    /// table as additive context, so an empty discovery is
    /// indistinguishable from "no shared newtype defined" — exactly
    /// what we want when no such type exists.
    pub(crate) fn discover(src_dir: &Path) -> Self {
        let mut labels = HashMap::new();
        let Ok(entries) = fs::read_dir(src_dir) else {
            return Self { labels };
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let Ok(source) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(file) = syn::parse_file(&source) else {
                continue;
            };
            let Some((ident, label)) = file_labeling(&file) else {
                continue;
            };
            labels.insert(ident, label);
        }
        Self { labels }
    }

    /// Look up the TOML label for `ident`, if `ident` names a
    /// shared newtype discovered by [`Self::discover`]. Returns
    /// `None` for builtin types and for unknown idents alike — the
    /// caller distinguishes the two paths via
    /// [`super::ty::is_builtin_type`].
    pub(crate) fn label_for(&self, ident: &str) -> Option<&str> {
        self.labels.get(ident).map(String::as_str)
    }

    /// Whether `ident` is a known shared newtype. Used by
    /// [`super::ty::collect_referenced_idents`] to skip the ident
    /// in the same pass that skips builtins: the renderer surfaces
    /// the label directly in the field-type column and has no
    /// per-rule custom-types sub-block to put the type into.
    pub(crate) fn contains(&self, ident: &str) -> bool {
        self.labels.contains_key(ident)
    }
}

/// Pull a single `(struct ident, TOML label)` pairing out of a
/// parsed `.rs` file, or `None` when the file doesn't fit the
/// convention. The convention is enforced lazily — multiple structs
/// or no `TOML_LABEL` const at all both reduce to "no contribution"
/// rather than a panic, so an unrelated `src/*.rs` helper (e.g.
/// `src/common.rs` with its many internal struct definitions) is
/// silently skipped instead of needing to be put on an opt-out list.
fn file_labeling(file: &syn::File) -> Option<(String, String)> {
    let mut label: Option<String> = None;
    let mut struct_idents: Vec<String> = Vec::new();
    for item in &file.items {
        match item {
            Item::Struct(item_struct) => {
                struct_idents.push(item_struct.ident.to_string());
            }
            Item::Const(item_const) if item_const.ident == "TOML_LABEL" => {
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(literal),
                    ..
                }) = &*item_const.expr
                {
                    label = Some(literal.value());
                }
            }
            _ => {}
        }
    }
    let label = label?;
    let [ident] = struct_idents.as_slice() else {
        return None;
    };
    Some((ident.clone(), label))
}

#[cfg(test)]
mod tests {
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
            Some("single-letter string")
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
}
