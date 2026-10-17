//! The `map_or_default` rewrite is gated on the linted crate's
//! `rust-version`, and only a real Cargo project carries one. The
//! `ui/` sweep compiles its fixtures without cargo, which leaves
//! `CARGO_PKG_RUST_VERSION` unset and `Msrv` answering for every
//! release, so the fallback is unreachable there however the gate is
//! written.
//!
//! These materialise a project either side of the release
//! `map_or_default` arrived in. Together they pin the version itself:
//! one says the rewrite is withheld below it, the other that it is
//! offered at it, so moving the constant either way turns one of them
//! red.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_sources, shared_target_dir};
use text_block_macros::text_block_fnl;

/// The release `Option::map_or_default` and `Result::map_or_default`
/// stabilised in, which the rules carry as a constant of their own.
const ARRIVED: &str = "1.99";

/// A release old enough to predate [`ARRIVED`] and new enough for the
/// edition the fixture declares.
const PREDATES: &str = "1.85";

/// The text of the rewrite. `unwrap_or_default` is a different string,
/// so the help text the gate falls back to does not match this one.
const REWRITE: &str = "map_or_default(";

/// The help text the gate falls back to.
const HELP: &str = "the default is the mapped type's own: `map(...).unwrap_or_default()`";

/// One call per rule, each with the default that earns the rewrite
/// where the crate is new enough: `0` is a `u32`'s `Default` written
/// as a value, and `|| 0` is the same default written as a closure.
const SOURCE: &str = text_block_fnl! {
    r##"#![allow(dead_code, reason = "fixture")]"##
    ""
    "pub fn value_default(width: Option<u32>) -> u32 {"
    "    width.map_or(0, |width| width + 1)"
    "}"
    ""
    "pub fn closure_default(width: Option<u32>) -> u32 {"
    "    width.map_or_else(|| 0, |width| width + 1)"
    "}"
};

/// A manifest carrying the one thing `build_project`'s own does not,
/// which is why the fixture writes its own: a `rust-version`. The
/// package name is the caller's, since cargo fingerprints the build by
/// the manifest's and two fixtures of one name share that fingerprint.
fn manifest(package_name: &str, rust_version: &str) -> String {
    text_block_fnl! {
        "[package]"
        r#"name = "{package_name}""#
        r#"version = "0.0.0""#
        r#"edition = "2024""#
        r#"rust-version = "{rust_version}""#
        ""
        "[lib]"
        r#"path = "src/lib.rs""#
        ""
        "# Declared empty so cargo does not enroll the fixture into the"
        "# perfectionist workspace it is nested inside."
        "[workspace]"
    }
    .replace("{package_name}", package_name)
    .replace("{rust_version}", rust_version)
}

/// Lint [`SOURCE`] as a crate pinned to `rust_version` and hand back
/// the diagnostics.
fn lint_at(package_name: &str, rust_version: &str) -> String {
    let manifest = manifest(package_name, rust_version);
    let (_temp, stderr, success) = run_project_with_sources(
        package_name,
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[("Cargo.toml", &manifest), ("src/lib.rs", SOURCE)],
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    // Both rules have to reach their call at all, or what the
    // assertions below find or miss says nothing about the gate.
    for lint in ["perfectionist::map_or", "perfectionist::map_or_else"] {
        assert!(
            stderr.contains(lint),
            "expected `{lint}` to flag its call; stderr was:\n{stderr}",
        );
    }
    stderr
}

#[test]
fn a_crate_predating_map_or_default_is_offered_the_help_text() {
    let stderr = lint_at("map_or_msrv_predates", PREDATES);
    assert!(
        !stderr.contains(REWRITE),
        "a crate pinned to {PREDATES} was offered `{REWRITE}`, which it cannot compile; \
         stderr was:\n{stderr}",
    );
    assert!(
        stderr.contains(HELP),
        "expected the help text the gate falls back to; stderr was:\n{stderr}",
    );
}

#[test]
fn a_crate_at_the_release_map_or_default_arrived_in_is_offered_the_rewrite() {
    let stderr = lint_at("map_or_msrv_arrived", ARRIVED);
    assert!(
        stderr.contains(REWRITE),
        "a crate pinned to {ARRIVED} was withheld `{REWRITE}`, which it can compile; \
         stderr was:\n{stderr}",
    );
    assert!(
        !stderr.contains(HELP),
        "expected the rewrite rather than the help text the gate falls back to; \
         stderr was:\n{stderr}",
    );
}
