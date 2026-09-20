//! The workspace manifest the crate under lint belongs to, read once.
//!
//! Cargo resolves workspace inheritance before it assembles rustc's
//! command line, so nothing that stays in `[workspace.dependencies]`,
//! `[workspace.package]` or `[workspace.lints]` reaches the compiler:
//! a member that has not written `<key>.workspace = true` leaves no
//! trace in any query. A rule that wants to know what the workspace
//! declares has to read the file.
//!
//! The read and the parse happen once per process, and a process
//! compiles one crate: the driver is a rustc wrapper, and rustc takes
//! one crate per invocation, so `CARGO_MANIFEST_DIR` cannot change
//! under the cache. Measured on a two-crate workspace, each crate's
//! walk ran under its own process id with its own manifest directory.
//!
//! The whole manifest is kept rather than the one table a caller
//! asked for, because the caller that pays for the read is rarely the
//! only one that wants the file.

use std::path::Path;
use std::sync::LazyLock;
use std::{env, fs};

/// The nearest `Cargo.toml` at or above `CARGO_MANIFEST_DIR` carrying
/// a `[workspace]` table, parsed.
///
/// `None` where no ancestor has one, and where the compiler was driven
/// without Cargo and so was given no `CARGO_MANIFEST_DIR` to start
/// from.
pub(crate) fn workspace() -> Option<&'static toml::Table> {
    static MANIFEST: LazyLock<Option<toml::Table>> = LazyLock::new(read_workspace);
    MANIFEST.as_ref()
}

/// The walk [`workspace`] caches.
///
/// It climbs to the first `Cargo.toml` carrying a `[workspace]` table,
/// which is how Cargo finds the root. It does not read `members` or
/// `exclude`, so a package Cargo would consider excluded is still read
/// as belonging to the workspace above it.
///
/// A file that cannot be read or parsed is stepped over rather than
/// ending the walk, so one unreadable manifest between the crate and
/// the root does not hide the root.
fn read_workspace() -> Option<toml::Table> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;
    Path::new(&manifest_dir)
        .ancestors()
        .filter_map(|directory| fs::read_to_string(directory.join("Cargo.toml")).ok())
        .filter_map(|text| text.parse::<toml::Table>().ok())
        .find(|manifest| manifest.get("workspace").is_some_and(toml::Value::is_table))
}
