//! The dependency gate asks what the crate under lint *declares*, not
//! what the compiler loaded, and only a real Cargo build can tell the
//! two apart.
//!
//! `--extern` is lazy, so the two questions disagree in both directions
//! and neither disagreement is reachable from a `ui/` fixture, where
//! compiletest links aux crates with `-L` and an `extern crate` item
//! rather than with `--extern`:
//!
//! - A crate that has added `command-extra` to its manifest and not yet
//!   named it is absent from the loaded set. That is the crate the
//!   advice most applies to, and the lint used to be silent in it.
//! - A crate that reaches `command-extra` only through a dependency's
//!   signature is present in the loaded set, but cannot name it. The
//!   lint used to fire there and tell the author to write a `use` that
//!   is `E0432`.
//!
//! Both fixtures build their `command-extra` from a path dependency
//! inside the temporary project, so nothing here touches the network.
//! The stub only has to carry the crate *name*: the gate reads the
//! dependency, and neither fixture calls a by-value setter.

pub mod _utils;

use _utils::{cargo_manifest_dir, run_project_with_config, shared_target_dir};
use text_block_macros::text_block_fnl;

const LINT: &str = "perfectionist::mutating_command_builder";

/// A stand-in for `command-extra`, as a path dependency. Its package
/// name is what gives the compiled crate the name the gate looks for.
const STUB_MANIFEST: &str = text_block_fnl! {
    "[package]"
    r#"name = "command-extra""#
    r#"version = "0.0.0""#
    r#"edition = "2024""#
    ""
    "[lib]"
    r#"path = "src/lib.rs""#
};

const STUB_SOURCE: &str = text_block_fnl! {
    "pub trait CommandExtra: Sized {"
    "    fn with_arg(self, arg: &str) -> Self;"
    "}"
    ""
    "impl CommandExtra for std::process::Command {"
    "    fn with_arg(self, _arg: &str) -> Self {"
    "        self"
    "    }"
    "}"
};

/// A crate between the fixture and the stub, naming the stub's trait in
/// a public signature so that compiling against it loads the stub.
const MIDDLE_MANIFEST: &str = text_block_fnl! {
    "[package]"
    r#"name = "middle""#
    r#"version = "0.0.0""#
    r#"edition = "2024""#
    ""
    "[lib]"
    r#"path = "src/lib.rs""#
    ""
    "[dependencies]"
    r#"command-extra = { path = "../command-extra" }"#
};

const MIDDLE_SOURCE: &str =
    "pub fn touch<T: command_extra::CommandExtra>(value: T) -> T {\n    value\n}\n";

/// The fixture manifest, which [`_utils::fixture_cargo_toml`] cannot
/// carry because it models no dependencies. Passing `Cargo.toml` as a
/// source overwrites the generated copy, so this restates the package
/// and lib stanzas that copy would have held.
fn fixture_manifest(dependency: &str) -> String {
    format!(
        "[package]\nname = \"gate\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [lib]\npath = \"src/lib.rs\"\n\n\
         [workspace]\n\n\
         [dependencies]\n{dependency}\n",
    )
}

#[test]
fn a_declared_dependency_fires_before_anything_names_it() {
    let source = text_block_fnl! {
        "use std::process::Command;"
        ""
        "pub fn never_names_the_crate() {"
        r#"    let mut command = Command::new("ls");"#
        r#"    command.arg("declared-not-named");"#
        "}"
    };
    let (_temp, stderr, success) = run_project_with_config(
        "gate",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            (
                "Cargo.toml",
                &fixture_manifest(r#"command-extra = { path = "command-extra" }"#),
            ),
            ("src/lib.rs", source),
            ("command-extra/Cargo.toml", STUB_MANIFEST),
            ("command-extra/src/lib.rs", STUB_SOURCE),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains(LINT),
        "expected a `{LINT}` warning in a crate that declares the \
         dependency without naming it; stderr was:\n{stderr}",
    );
}

#[test]
fn a_transitively_reached_crate_stays_silent() {
    let source = text_block_fnl! {
        "use std::process::Command;"
        ""
        "pub fn reaches_it_only_through_middle() {"
        r#"    let _ = middle::touch(Command::new("ls"));"#
        r#"    let mut command = Command::new("ls");"#
        r#"    command.arg("transitively-reached");"#
        "}"
    };
    let (_temp, stderr, success) = run_project_with_config(
        "gate",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            (
                "Cargo.toml",
                &fixture_manifest(r#"middle = { path = "middle" }"#),
            ),
            ("src/lib.rs", source),
            ("middle/Cargo.toml", MIDDLE_MANIFEST),
            ("middle/src/lib.rs", MIDDLE_SOURCE),
            ("command-extra/Cargo.toml", STUB_MANIFEST),
            ("command-extra/src/lib.rs", STUB_SOURCE),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(LINT),
        "expected silence in a crate that cannot name `command_extra`, \
         since the import the diagnostic would ask for is `E0432`; \
         stderr was:\n{stderr}",
    );
}
