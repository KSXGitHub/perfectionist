//! The extraction-target path arithmetic the inline-style help text
//! uses.

use rustc_span::{FileName, SourceFile};
use std::path::{Path, PathBuf};

pub(super) fn real_path(file: &SourceFile) -> Option<PathBuf> {
    match &file.name {
        FileName::Real(real) => real.local_path().map(Path::to_path_buf),
        _ => None,
    }
}

/// The canonical extraction target for a module of `name` declared in
/// `parent`: `<parent_dir>/<parent_stem>/<name>.rs` for an ordinary
/// file, or `<parent_dir>/<name>.rs` when the parent is a
/// directory-owning file (`lib.rs` / `main.rs` / `mod.rs`), matching
/// where Cargo places a child module of such a file. Used to name the
/// target in inline-style help text.
pub(super) fn canonical_target(parent: &Path, name: &str) -> Option<PathBuf> {
    let dir = parent.parent()?;
    if is_mod_root(parent) {
        return Some(dir.join(format!("{name}.rs")));
    }
    let stem = parent.file_stem()?.to_str()?;
    Some(dir.join(stem).join(format!("{name}.rs")))
}

fn is_mod_root(parent: &Path) -> bool {
    matches!(
        parent.file_name().and_then(|name| name.to_str()),
        Some("lib.rs" | "main.rs" | "mod.rs"),
    )
}
