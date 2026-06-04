pub use _utils::*;
use std::path::{Path, PathBuf};

const CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

pub fn cargo_manifest_dir() -> &'static Path {
    Path::new(CARGO_MANIFEST_DIR)
}

/// Path to the target dir shared by every integration-test fixture.
pub fn shared_target_dir() -> PathBuf {
    cargo_manifest_dir()
        .join("target")
        .join("integration-fixtures")
}
