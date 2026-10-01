//! Which types `folded_command_setter` reaches, and why that is the
//! resolved `command-extra`'s decision rather than the rule's.
//!
//! The rule never asks the accumulator's type. What proves the
//! accumulator implements `CommandExtra` is the folder resolving to one
//! of its setters, so the rule follows whatever the release implements
//! the trait for -- and cannot fire where the release does not, because
//! such a fold does not type-check.
//!
//! That is a claim about two builds, not one, so it needs a real Cargo
//! project rather than a `ui/` fixture: the contrast is a version
//! requirement for the boxed case and a feature for the rest, and a
//! feature-gated half needs the command crate it names as a genuine
//! dependency.
//! `ui/folded_command_setter_boxed.rs` carries what the diagnostic
//! *says* for the boxed case; this file carries when it is said at all.
//!
//! Ignored by default: each case resolves `command-extra` from the
//! registry into a fresh fixture crate, which does not belong in the
//! gating suite.
//!
//! ```text
//! cargo test --test folded_command_setter_command_types -- --ignored
//! ```

pub mod _utils;

use _utils::{
    TempDir, build_project_with_config, cargo_manifest_dir, fixture_cargo_toml, run_dylint,
    shared_target_dir,
};

const BOXED: &str = include_str!("fixtures/folded_command_setter_command_types/boxed.rs");

const TOKIO: &str = include_str!("fixtures/folded_command_setter_command_types/tokio.rs");

const ASYNC_PROCESS: &str =
    include_str!("fixtures/folded_command_setter_command_types/async_process.rs");

/// Sibling rules would speak about the same lines on their own account,
/// and the assertions below read the whole stderr.
const CONFIG: &str = "[perfectionist]\ndisable = [\"bare_identifier_reference\"]\n";

/// The generated manifest with `dependencies` appended, which
/// [`fixture_cargo_toml`] does not carry. Appending to its output rather
/// than restating it keeps the package, lib and workspace stanzas in one
/// place.
fn cargo_toml(package: &str, dependencies: &str) -> String {
    format!(
        "{}\n[dependencies]\n{dependencies}",
        fixture_cargo_toml(package),
    )
}

/// Run the lint over one fixture crate and hand back its stderr and
/// whether the build succeeded.
fn check(package: &str, dependencies: &str, source: &str) -> (TempDir, String, bool) {
    let temp = TempDir::new().expect("failed to create temp dir");
    build_project_with_config(
        temp.path(),
        package,
        cargo_manifest_dir(),
        &[
            ("Cargo.toml", &cargo_toml(package, dependencies)),
            ("src/lib.rs", source),
        ],
        CONFIG,
    );
    let (stderr, success) = run_dylint(temp.path(), &shared_target_dir());
    (temp, stderr, success)
}

/// What the rule says when it fires, whatever the accumulator.
const FIRES: &str = "re-implements `without_envs`";

#[test]
#[ignore = "resolves `command-extra` from the registry in a fresh fixture crate"]
fn a_boxed_command_is_reached_once_the_release_implements_the_trait() {
    let (_temp, stderr, success) =
        check("fcs_boxed_present", "command-extra = \"=1.4.0\"\n", BOXED);
    assert!(success, "the fixture should compile; stderr was:\n{stderr}");
    assert!(
        stderr.contains(FIRES) && stderr.contains("BOXED_VARS"),
        "expected the rule to fire on the boxed fold; stderr was:\n{stderr}",
    );
}

#[test]
#[ignore = "resolves `command-extra` from the registry in a fresh fixture crate"]
fn a_boxed_command_is_not_reached_before_that() {
    let (_temp, stderr, success) = check("fcs_boxed_absent", "command-extra = \"=1.3.0\"\n", BOXED);

    // 1.3.0 has no `impl CommandExtra for Box<Command>`, so the fold the
    // rule would speak about cannot be written at all.
    assert!(
        !success && stderr.contains("E0277"),
        "expected the fixture not to compile against 1.3.0; stderr was:\n{stderr}",
    );
}

#[test]
#[ignore = "resolves `command-extra` and `tokio` from the registry in a fresh fixture crate"]
fn tokio_commands_are_reached_with_the_feature_on() {
    let (_temp, stderr, success) = check(
        "fcs_tokio_present",
        "command-extra = { version = \"=1.4.0\", features = [\"tokio_process\"] }\n\
         tokio = { version = \"1.53.1\", features = [\"process\"] }\n",
        TOKIO,
    );
    assert!(success, "the fixture should compile; stderr was:\n{stderr}");
    for marker in ["BARE_TOKIO_VARS", "BOXED_TOKIO_VARS"] {
        assert!(
            stderr.contains(FIRES) && stderr.contains(marker),
            "expected the rule to fire on `{marker}`; stderr was:\n{stderr}",
        );
    }
}

#[test]
#[ignore = "resolves `command-extra` and `tokio` from the registry in a fresh fixture crate"]
fn tokio_commands_are_not_reached_with_the_feature_off() {
    let (_temp, stderr, success) = check(
        "fcs_tokio_absent",
        "command-extra = \"=1.4.0\"\n\
         tokio = { version = \"1.53.1\", features = [\"process\"] }\n",
        TOKIO,
    );

    // The impls are the feature's, not the version's, so the same
    // release reaches neither tokio type without it.
    assert!(
        !success && stderr.contains("E0277"),
        "expected the fixture not to compile without the feature; stderr was:\n{stderr}",
    );
}

#[test]
#[ignore = "resolves `command-extra` and `async-process` from the registry in a fresh fixture crate"]
fn async_process_commands_are_reached_with_the_feature_on() {
    let (_temp, stderr, success) = check(
        "fcs_async_present",
        "command-extra = { version = \"=1.5.0\", features = [\"async_process\"] }\n\
         async-process = \"2\"\n",
        ASYNC_PROCESS,
    );
    assert!(success, "the fixture should compile; stderr was:\n{stderr}");
    for marker in ["BARE_ASYNC_VARS", "BOXED_ASYNC_VARS"] {
        assert!(
            stderr.contains(FIRES) && stderr.contains(marker),
            "expected the rule to fire on `{marker}`; stderr was:\n{stderr}",
        );
    }
}

#[test]
#[ignore = "resolves `command-extra` and `async-process` from the registry in a fresh fixture crate"]
fn async_process_commands_are_not_reached_with_the_feature_off() {
    let (_temp, stderr, success) = check(
        "fcs_async_absent",
        "command-extra = \"=1.5.0\"\nasync-process = \"2\"\n",
        ASYNC_PROCESS,
    );

    // The impls are the feature's, not the version's, so the release that
    // introduced them reaches neither type without it.
    assert!(
        !success && stderr.contains("E0277"),
        "expected the fixture not to compile without the feature; stderr was:\n{stderr}",
    );
}
