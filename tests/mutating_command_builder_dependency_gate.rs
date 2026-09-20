//! The dependency gate asks what is *declared* around the crate under
//! lint, not what the compiler loaded, and only a real Cargo build can
//! tell the two apart.
//!
//! `--extern` is lazy, so the two questions disagree in both directions
//! and neither disagreement is reachable from a `ui/` fixture, where
//! compiletest links aux crates with `-L` and an `extern crate` item
//! rather than with `--extern`:
//!
//! - A crate that has added `command-extra` to its manifest and not yet
//!   named it is absent from the loaded set, so a gate keyed on that
//!   set would be silent in it.
//! - A crate that reaches `command-extra` only through a dependency's
//!   signature is present in the loaded set, but cannot name it. The
//!   same gate would fire there and tell the author to write a `use`
//!   that is `E0432`.
//!
//! And one the declared set cannot answer at all: Cargo keys `--extern`
//! by the manifest key, so a dependency renamed there is not there under
//! `command_extra`. An import of the trait is what carries that case.
//!
//! A Cargo build is also the only place `command_extra_dependency`'s
//! `crate` and `workspace` values differ, since the table one of them
//! reads is a file Cargo resolves away before rustc sees anything.
//!
//! Every fixture here builds its `command-extra` from a path dependency
//! inside the temporary project, so nothing touches the network. The
//! stub carries the crate name, which is what the gate reads, and one
//! by-value setter, which the renamed fixture calls to show the
//! counterpart really is reachable there.

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

#[test]
fn a_renamed_dependency_fires_through_its_import() {
    let source = text_block_fnl! {
        "use ce::CommandExtra;"
        "use std::process::Command;"
        ""
        "pub fn the_counterpart_is_reachable() -> Command {"
        r#"    Command::new("ls").with_arg("by-value-compiles-here")"#
        "}"
        ""
        "pub fn should_be_flagged() {"
        r#"    let mut command = Command::new("ls");"#
        r#"    command.arg("renamed-dependency");"#
        "}"
    };
    let (_temp, stderr, success) = run_project_with_config(
        "gate",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            (
                "Cargo.toml",
                &fixture_manifest(r#"ce = { package = "command-extra", path = "command-extra" }"#),
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
        "expected a `{LINT}` warning in a crate whose manifest renames \
         the dependency, since the trait is imported there and the \
         counterpart compiles; stderr was:\n{stderr}",
    );
}

/// A workspace that declares the dependency once, with one member
/// inheriting it and one that has not. Returns the `cargo dylint`
/// stderr, which is where the fixture's own argument strings come back
/// to be matched on.
fn run_the_workspace(dylint_config: &str) -> String {
    let member = |name: &str| {
        format!(
            "use std::process::Command;\n\npub fn build() {{\n    let mut command = \
             Command::new(\"ls\");\n    command.arg(\"{name}\");\n}}\n",
        )
    };
    let root = text_block_fnl! {
        "[workspace]"
        r#"members = ["inherits", "abstains", "command-extra"]"#
        r#"resolver = "3""#
        ""
        "[workspace.dependencies]"
        r#"command-extra = { path = "command-extra" }"#
    };
    let inheriting = text_block_fnl! {
        "[package]"
        r#"name = "inherits""#
        r#"version = "0.0.0""#
        r#"edition = "2024""#
        ""
        "[lib]"
        r#"path = "src/lib.rs""#
        ""
        "[dependencies]"
        "command-extra.workspace = true"
    };
    let abstaining = text_block_fnl! {
        "[package]"
        r#"name = "abstains""#
        r#"version = "0.0.0""#
        r#"edition = "2024""#
        ""
        "[lib]"
        r#"path = "src/lib.rs""#
    };
    let (_temp, stderr, success) = run_project_with_config(
        "workspace",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("Cargo.toml", root),
            ("inherits/Cargo.toml", inheriting),
            ("inherits/src/lib.rs", &member("inherits")),
            ("abstains/Cargo.toml", abstaining),
            ("abstains/src/lib.rs", &member("abstains")),
            ("command-extra/Cargo.toml", STUB_MANIFEST),
            ("command-extra/src/lib.rs", STUB_SOURCE),
        ],
        dylint_config,
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    stderr
}

/// The echoed source line for a member, rather than the member name,
/// which cargo also prints as it builds each one.
fn flagged_line(member: &str) -> String {
    format!(r#"command.arg("{member}")"#)
}

/// Both members are flagged at the default reach, including the one
/// that has not written `command-extra.workspace = true`.
#[test]
fn the_default_reaches_the_workspace_table() {
    let stderr = run_the_workspace("");
    for member in ["inherits", "abstains"] {
        assert!(
            stderr.contains(&flagged_line(member)),
            "expected `{member}` to be flagged under a workspace that \
             declares the dependency; stderr was:\n{stderr}",
        );
    }
}

/// With the reach narrowed to `crate`, only the inheriting member is
/// flagged.
#[test]
fn narrowing_to_the_crate_leaves_the_abstaining_member_alone() {
    let stderr = run_the_workspace(&format!(
        "[\"{LINT}\"]\ncommand_extra_dependency = \"crate\"\n",
    ));
    assert!(
        stderr.contains(&flagged_line("inherits")),
        "expected the member that inherits the dependency to be flagged; \
         stderr was:\n{stderr}",
    );
    assert!(
        !stderr.contains(&flagged_line("abstains")),
        "expected the member that does not inherit it to stay silent, since \
         the counterpart is not writable there; stderr was:\n{stderr}",
    );
}
