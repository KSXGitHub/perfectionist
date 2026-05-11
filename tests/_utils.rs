//! Per-test re-export shim for the workspace `_utils` crate.
//!
//! `_utils` is compiled in isolation and has no view of perfectionist's
//! source tree, so the path of the perfectionist crate and the location
//! of the shared integration-test target dir are computed here, where
//! `CARGO_MANIFEST_DIR` is the perfectionist crate root.
//!
//! Tests that exercise `cargo dylint` against a synthetic project should
//! declare `pub mod _utils;` at the top of their file and use
//! [`run_project_with_sources`] from this module rather than calling
//! the underlying crate directly.

use std::path::{Path, PathBuf};

pub use _utils::*;

pub const CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

pub fn perfectionist_dir() -> &'static Path {
    Path::new(CARGO_MANIFEST_DIR)
}

/// Path to the target dir shared by every integration-test fixture.
/// Pre-warmed by `just warmup-integration-tests`; populated lazily by
/// cargo otherwise.
pub fn shared_target_dir() -> PathBuf {
    perfectionist_dir()
        .join("target")
        .join("integration-fixtures")
}
