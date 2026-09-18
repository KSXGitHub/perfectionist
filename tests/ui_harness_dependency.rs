//! Guard the property that makes `_utils::ConfiguredUiTest` the only
//! route to a configured UI test: this package does not depend on the
//! UI harness, so no integration test here can name the builder that
//! reaches `DYLINT_TOML` directly.
//!
//! While the dependency is absent the compiler enforces that on its
//! own. This test covers the step that would quietly undo it — the
//! dependency restored to a manifest, say by a merge resolved against
//! a branch predating its removal — and says why it was moved, where
//! the compiler would only stop reporting an unknown crate.

use std::fs;
use std::path::Path;
use text_block_macros::text_block_fnl;

/// The UI harness, which reaches its driver through a process-global
/// environment variable. `_utils` depends on it and wraps that access
/// in a helper that takes a lock first; this package must not, or a
/// test binary could name the builder and skip the lock.
const UI_HARNESS: &str = "dylint_testing";

/// The manifest tables whose entries put a crate in this package's
/// extern prelude, and so would make [`UI_HARNESS`] nameable from
/// `tests/`. Cargo accepts each of them at the manifest root and
/// again under a `[target.<cfg>]` table, and both are walked.
const DEPENDENCY_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

#[test]
fn the_lint_crate_does_not_depend_on_the_ui_harness() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let text = fs::read_to_string(&path).expect("read this package's Cargo.toml");
    let offenders = ui_harness_dependencies(&parse(&text));
    assert!(
        offenders.is_empty(),
        "this package depends on `{UI_HARNESS}`: {offenders:?}\n\
         The dependency belongs to `_utils`, whose `ConfiguredUiTest` holds the lock that \
         keeps one fixture's `DYLINT_TOML` out of another fixture's run. While it is absent \
         here, an integration test that names the harness does not compile.",
    );
}

fn parse(text: &str) -> toml::Table {
    toml::from_str(text).expect("parse a Cargo manifest")
}

/// Every dependency entry in `manifest` that resolves to
/// [`UI_HARNESS`], each labelled by the table it was found in.
fn ui_harness_dependencies(manifest: &toml::Table) -> Vec<String> {
    let workspace = manifest
        .get("workspace")
        .and_then(toml::Value::as_table)
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table);
    let mut offenders = Vec::new();
    collect(manifest, "", workspace, &mut offenders);
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for (cfg, target) in targets {
            if let Some(target) = target.as_table() {
                collect(target, &format!("target.{cfg}."), workspace, &mut offenders);
            }
        }
    }
    offenders
}

/// Scan `tables`' dependency tables, labelling what it finds with
/// `prefix` so a `[target.<cfg>]` hit is distinguishable from a
/// root-level one.
fn collect(
    tables: &toml::Table,
    prefix: &str,
    workspace: Option<&toml::Table>,
    offenders: &mut Vec<String>,
) {
    for table in DEPENDENCY_TABLES {
        let Some(entries) = tables.get(*table).and_then(toml::Value::as_table) else {
            continue;
        };
        for (name, spec) in entries {
            if resolves_to_ui_harness(name, spec, workspace) {
                offenders.push(format!("[{prefix}{table}] {name}"));
            }
        }
    }
}

/// Whether a dependency entry pulls in [`UI_HARNESS`]: under its own
/// key, renamed through `package`, or inherited from the workspace,
/// where the entry it inherits may itself be renamed.
fn resolves_to_ui_harness(name: &str, spec: &toml::Value, workspace: Option<&toml::Table>) -> bool {
    if let Some(package) = spec.get("package").and_then(toml::Value::as_str) {
        return package == UI_HARNESS;
    }
    let inherits = spec
        .get("workspace")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    if inherits && let Some(inherited) = workspace.and_then(|entries| entries.get(name)) {
        return resolves_to_ui_harness(name, inherited, None);
    }
    name == UI_HARNESS
}

/// The forms Cargo accepts, each of which makes the harness nameable
/// from an integration test. A green run of the test above exercises
/// none of them, since the manifest it reads holds no such entry, so
/// they are pinned here against a manifest written for the purpose.
#[test]
fn every_dependency_form_that_reaches_the_harness_is_found() {
    let cases = [
        (
            "under its own key",
            text_block_fnl! {
                "[dev-dependencies]"
                r#"dylint_testing = "6.0.4""#
            },
        ),
        (
            "renamed through `package`",
            text_block_fnl! {
                "[dev-dependencies]"
                r#"ui_harness = { package = "dylint_testing", version = "6.0.4" }"#
            },
        ),
        (
            "in a target table",
            text_block_fnl! {
                "[target.'cfg(unix)'.dev-dependencies]"
                r#"ui_harness = { package = "dylint_testing", version = "6.0.4" }"#
            },
        ),
        (
            "as a normal dependency",
            text_block_fnl! {
                "[dependencies]"
                r#"dylint_testing = "6.0.4""#
            },
        ),
        (
            "as a build dependency",
            text_block_fnl! {
                "[build-dependencies]"
                r#"dylint_testing = "6.0.4""#
            },
        ),
        (
            "inherited from the workspace",
            text_block_fnl! {
                "[workspace.dependencies]"
                r#"ui_harness = { package = "dylint_testing", version = "6.0.4" }"#
                "[dev-dependencies]"
                "ui_harness = { workspace = true }"
            },
        ),
    ];
    for (form, manifest) in cases {
        assert!(
            !ui_harness_dependencies(&parse(manifest)).is_empty(),
            "a dependency {form} went unnoticed:\n{manifest}",
        );
    }
}

/// The complement: a manifest that names other crates, in every table
/// walked above, must produce nothing. Without this the test above
/// would pass just as happily on a function that always reports a hit.
#[test]
fn a_manifest_without_the_harness_reports_nothing() {
    let manifest = text_block_fnl! {
        "[dependencies]"
        r#"serde = "1.0.228""#
        "[dev-dependencies]"
        r#"_utils = { path = "utils" }"#
        r#"toml = "1.1.2""#
        "[build-dependencies]"
        r#"cc = "1""#
        "[target.'cfg(unix)'.dev-dependencies]"
        r#"libc = "0.2""#
        "[workspace.dependencies]"
        r#"shared = { package = "some_other_crate", version = "1" }"#
    };
    assert_eq!(
        ui_harness_dependencies(&parse(manifest)),
        Vec::<String>::new(),
    );
}
