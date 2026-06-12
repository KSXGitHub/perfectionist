//! Shared `allowed_paths` plumbing for `wildcard_imports` and
//! `named_prelude_imports`: turning a `use` path into the absolute key the
//! allow list is matched against, and validating that a configured entry
//! is a well-formed absolute path.
//!
//! Rust has two absolute path forms, written differently:
//!
//! - An **extern-crate** path carries a leading `::` (`::rayon::iter`). The
//!   bare spelling (`rayon::iter`) resolves the same way in the 2018+
//!   editions, so an `allowed_paths` entry uses the `::`-led form and
//!   matches either spelling of the `use`.
//! - A **crate-root** path uses the `crate` keyword (`crate::internals`)
//!   and *no* leading `::` — `::crate` is not valid syntax. A crate-root
//!   entry is therefore written exactly as the `use` writes it.
//!
//! `self::` / `super::` paths are relative, never absolute, and so are
//! never valid `allowed_paths` entries.

/// The absolute key for a `use` path's module segments, already
/// `PathRoot`-stripped and joined with `::` (`"rayon::iter"`,
/// `"crate::internals"`). A `crate`-rooted path is already absolute and
/// keeps its spelling; every other path is treated as an extern-crate path
/// and gets the leading `::` that marks it absolute.
pub(crate) fn canonical_key(joined_path: &str) -> String {
    if joined_path == "crate" || joined_path.starts_with("crate::") {
        joined_path.to_owned()
    } else {
        format!("::{joined_path}")
    }
}

/// Validate that a configured `allowed_paths` entry is a well-formed
/// absolute path: either `crate::...` or `::<extern crate>::...`. Returns an
/// error message for an entry that names an impossible path — a `::`-led
/// keyword (`::crate`, `::self`, `::super`), a relative `self::` / `super::`
/// path, or a bare extern path missing its leading `::`.
pub(crate) fn validate_absolute(entry: &str) -> Result<(), String> {
    let pieces: Vec<&str> = entry.split("::").map(str::trim).collect();
    // `entry` begins with `::` exactly when its first split piece is empty.
    let leading_root = pieces.first() == Some(&"");
    let first_segment = pieces.iter().copied().find(|piece| !piece.is_empty());
    let valid = match (leading_root, first_segment) {
        // `::extern::...` — the first segment must be an ordinary crate
        // name, not a path keyword.
        (true, Some(segment)) => !is_path_keyword(segment),
        // `crate::...` — crate-root absolute, written without a leading `::`.
        (false, Some("crate")) => true,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "`allowed_paths` entry {entry:?} is not an absolute path: write a \
             crate-root path as `crate::...`, or an extern-crate path with a \
             leading `::` (e.g. `::rayon::iter`). `::crate`, `::self`, `::super`, \
             and relative `self::` / `super::` paths are not valid",
        ))
    }
}

fn is_path_keyword(segment: &str) -> bool {
    matches!(segment, "crate" | "self" | "super" | "Self")
}

#[cfg(test)]
mod tests {
    use super::{canonical_key, validate_absolute};

    #[test]
    fn keys_prefix_extern_but_not_crate_root() {
        assert_eq!(canonical_key("rayon::iter"), "::rayon::iter");
        assert_eq!(canonical_key("crate::internals"), "crate::internals");
        assert_eq!(canonical_key("crate"), "crate");
    }

    #[test]
    fn accepts_absolute_entries() {
        for ok in [
            "::rayon::iter",
            "::serde::prelude",
            "crate::internals",
            "crate",
        ] {
            assert!(validate_absolute(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn rejects_impossible_or_relative_entries() {
        for bad in [
            "::crate::x",
            "::self::x",
            "::super::x",
            "::Self::x",
            "self::x",
            "super::x",
            "rayon::iter",
            "::",
            "",
        ] {
            assert!(validate_absolute(bad).is_err(), "{bad}");
        }
    }
}
