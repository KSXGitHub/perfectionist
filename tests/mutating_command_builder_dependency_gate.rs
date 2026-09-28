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

/// A newer `command-extra`, declaring a counterpart the older one
/// below does not. Two semver-incompatible majors of one package is a
/// resolution Cargo reaches whenever two dependencies disagree, and a
/// `ui/` fixture cannot model it: compiletest builds aux crates into
/// one directory, where the second would overwrite the first.
const NEW_STUB_MANIFEST: &str = text_block_fnl! {
    "[package]"
    r#"name = "command-extra""#
    r#"version = "2.0.0""#
    r#"edition = "2024""#
    ""
    "[lib]"
    r#"path = "src/lib.rs""#
};

const NEW_STUB_SOURCE: &str = text_block_fnl! {
    "pub trait CommandExtra: Sized {"
    "    fn with_arg(self, arg: &str) -> Self;"
    "    fn with_envs(self, envs: [(&str, &str); 1]) -> Self;"
    "}"
    ""
    "impl CommandExtra for std::process::Command {"
    "    fn with_arg(self, _arg: &str) -> Self {"
    "        self"
    "    }"
    ""
    "    fn with_envs(self, _envs: [(&str, &str); 1]) -> Self {"
    "        self"
    "    }"
    "}"
};

/// The older major, reached through `middle`. Its source is
/// [`STUB_SOURCE`], which has no `with_envs`.
const OLD_STUB_MANIFEST: &str = text_block_fnl! {
    "[package]"
    r#"name = "command-extra""#
    r#"version = "1.0.0""#
    r#"edition = "2024""#
    ""
    "[lib]"
    r#"path = "src/lib.rs""#
};

/// `middle` against the *newer* major, so that the crate the fixture
/// itself names is the older one. That arrangement is what makes the
/// test below discriminating: the transitive crate is the one a lookup
/// meets first, so picking rather than asking all of them picks the
/// trait the code under lint does not resolve to.
const MIDDLE_NEW_MANIFEST: &str = text_block_fnl! {
    "[package]"
    r#"name = "middle""#
    r#"version = "0.0.0""#
    r#"edition = "2024""#
    ""
    "[lib]"
    r#"path = "src/lib.rs""#
    ""
    "[dependencies]"
    r#"command-extra = { path = "../command-extra-new", version = "2.0.0" }"#
};

/// `.envs` against a graph carrying one `CommandExtra` that declares
/// `with_envs` and one that does not.
fn run_two_majors(sources: &[(&str, &str)]) -> String {
    let (_temp, stderr, success) = run_project_with_config(
        "gate",
        cargo_manifest_dir(),
        &shared_target_dir(),
        sources,
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    stderr
}

const TWO_MAJORS_SOURCE: &str =
    include_str!("fixtures/mutating_command_builder_dependency_gate/two_majors_lib.rs");

/// The control: one `CommandExtra`, which declares `with_envs`, so the
/// counterpart check passes and `.envs` is flagged.
#[test]
fn one_major_declaring_the_counterpart_fires() {
    let source = text_block_fnl! {
        "use command_extra::CommandExtra;"
        "use std::process::Command;"
        ""
        "pub fn reaches_one() {"
        r#"    let mut command = Command::new("ls");"#
        r#"    command.envs([("LANG", "C")]);"#
        "}"
    };
    let stderr = run_two_majors(&[
        (
            "Cargo.toml",
            &fixture_manifest(
                r#"command-extra = { path = "command-extra-new", version = "2.0.0" }"#,
            ),
        ),
        ("src/lib.rs", source),
        ("command-extra-new/Cargo.toml", NEW_STUB_MANIFEST),
        ("command-extra-new/src/lib.rs", NEW_STUB_SOURCE),
    ]);
    assert!(
        stderr.contains(LINT),
        "expected `.envs` to be flagged where the one loaded \
         `CommandExtra` declares `with_envs`; stderr was:\n{stderr}",
    );
}

/// The trait the fixture names is the one *without* `with_envs`, and
/// the one that has it is reached through `middle`. Naming `with_envs`
/// here is `E0599`, so the silence is the answer rather than merely
/// the safe side.
#[test]
fn two_majors_stand_down_on_a_counterpart_only_one_declares() {
    let stderr = run_two_majors(&[
        (
            "Cargo.toml",
            &fixture_manifest(text_block_fnl! {
                r#"command-extra = { path = "command-extra-old", version = "1.0.0" }"#
                r#"middle = { path = "middle" }"#
            }),
        ),
        ("src/lib.rs", TWO_MAJORS_SOURCE),
        ("command-extra-old/Cargo.toml", OLD_STUB_MANIFEST),
        ("command-extra-old/src/lib.rs", STUB_SOURCE),
        ("command-extra-new/Cargo.toml", NEW_STUB_MANIFEST),
        ("command-extra-new/src/lib.rs", NEW_STUB_SOURCE),
        ("middle/Cargo.toml", MIDDLE_NEW_MANIFEST),
        ("middle/src/lib.rs", MIDDLE_SOURCE),
    ]);
    assert!(
        !stderr.contains(LINT),
        "expected silence where the `CommandExtra` the crate under lint \
         names has no `with_envs`, though another loaded one does; \
         stderr was:\n{stderr}",
    );
}

/// The table a build script needs, which is not the one every other
/// target needs: Cargo compiles `build.rs` against
/// `[build-dependencies]` alone, so a package that has `command-extra`
/// under `[dependencies]` has not given its build script anything.
const BUILD_SCRIPT_TABLE: &str = "`[build-dependencies]`";

/// The form the remedy took before it named the condition instead of
/// one table, which would leave a build script's import `E0432`.
const UNCONDITIONAL_REMEDY: &str = "add `command-extra` to this crate's dependencies";

/// The remedy names which table applies rather than picking one, so it
/// is followable from a build script and from every other target with
/// one string. Only the build script is checked here, because it is the
/// target whose table the other advice got wrong.
#[test]
fn a_build_script_is_told_which_table_applies() {
    let root = text_block_fnl! {
        "[workspace]"
        r#"members = ["alpha", "command-extra"]"#
        r#"resolver = "3""#
        ""
        "[workspace.dependencies]"
        r#"command-extra = { path = "command-extra" }"#
    };
    let alpha = text_block_fnl! {
        "[package]"
        r#"name = "alpha""#
        r#"version = "0.0.0""#
        r#"edition = "2024""#
        ""
        "[lib]"
        r#"path = "src/lib.rs""#
        ""
        "[dependencies]"
        "command-extra.workspace = true"
    };
    let build_script = text_block_fnl! {
        "use std::process::Command;"
        ""
        "fn main() {"
        r#"    let mut command = Command::new("ls");"#
        r#"    command.arg("alpha-build");"#
        "}"
    };
    // The library reaches `command-extra` where the build script does
    // not, so it earns the import remedy and the build script earns the
    // manifest one. Having both in the run is what makes the assertions
    // below tell the two apart rather than merely find one.
    let library = text_block_fnl! {
        "use std::process::Command;"
        ""
        "pub fn build() {"
        r#"    let mut command = Command::new("ls");"#
        r#"    command.arg("alpha-lib");"#
        "}"
    };
    let (_temp, stderr, success) = run_project_with_config(
        "build-script",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            ("Cargo.toml", root),
            ("alpha/Cargo.toml", alpha),
            ("alpha/build.rs", build_script),
            ("alpha/src/lib.rs", library),
            ("command-extra/Cargo.toml", STUB_MANIFEST),
            ("command-extra/src/lib.rs", STUB_SOURCE),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        stderr.contains(BUILD_SCRIPT_TABLE),
        "expected the remedy to name `[build-dependencies]` among the tables, \
         since that is the only one a build script is compiled against; \
         stderr was:\n{stderr}",
    );
    assert!(
        !stderr.contains(UNCONDITIONAL_REMEDY),
        "expected no target to be told to add the dependency to the table it \
         already has it in; stderr was:\n{stderr}",
    );
}

const TWO_BUILD_LIB: &str =
    include_str!("fixtures/mutating_command_builder_dependency_gate/two_builds_lib.rs");

const TWO_BUILD_INTEGRATION: &str =
    include_str!("fixtures/mutating_command_builder_dependency_gate/two_builds_integration.rs");

fn run_two_builds(dependency_table: &str) -> String {
    let (_temp, stderr, success) = run_project_with_config(
        "two-builds",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            (
                "Cargo.toml",
                &format!(
                    "[package]\nname = \"probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
                     [lib]\npath = \"src/lib.rs\"\n\n[workspace]\n\n{dependency_table}\n",
                ),
            ),
            ("src/lib.rs", TWO_BUILD_LIB),
            ("tests/it.rs", TWO_BUILD_INTEGRATION),
            ("command-extra/Cargo.toml", STUB_MANIFEST),
            ("command-extra/src/lib.rs", STUB_SOURCE),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    stderr
}

/// With the dependency reaching only the test build, the library's own
/// code is left alone: the import the diagnostic would ask for belongs
/// where the shipping build compiles it, and there is no
/// `command_extra` there.
#[test]
fn a_test_only_dependency_leaves_the_library_alone() {
    let stderr = run_two_builds(text_block_fnl! {
        "[dev-dependencies]"
        r#"command-extra = { path = "command-extra" }"#
    });
    assert!(
        !stderr.contains(&flagged_line("production-code")),
        "expected the library's own code to be left to its shipping build; \
         stderr was:\n{stderr}",
    );
    assert!(
        stderr.contains(&flagged_line("inline-test-code")),
        "expected inline test code to be flagged, since the import lands \
         where only the test build compiles it; stderr was:\n{stderr}",
    );
    assert!(
        stderr.contains(&flagged_line("integration-helper")),
        "expected a plain helper in `tests/` to be flagged: it has no \
         shipping build to defer to; stderr was:\n{stderr}",
    );
}

/// The shipping build still asks. Exempting the `--test` build costs
/// nothing where the dependency reaches both.
#[test]
fn an_ordinary_dependency_still_flags_the_library() {
    let stderr = run_two_builds(text_block_fnl! {
        "[dependencies]"
        r#"command-extra = { path = "command-extra" }"#
    });
    for call in ["production-code", "inline-test-code", "integration-helper"] {
        assert!(
            stderr.contains(&flagged_line(call)),
            "expected `{call}` to be flagged; stderr was:\n{stderr}",
        );
    }
}

const TEST_FN_IN_PRODUCTION_MODULE: &str = include_str!(
    "fixtures/mutating_command_builder_dependency_gate/test_fn_in_production_module.rs"
);

/// The import remedy has to carry its own condition, because the
/// exemption cannot close this. That gate asks whether the *node* is
/// test code; the import lands in the node's *module*. A `#[test]` fn
/// in the library's own root is test-exclusive as a node and
/// production as a module, so the rule speaks and the `use` it asks
/// for is `E0432` in the shipping build.
#[test]
fn a_test_fn_in_a_production_module_is_told_where_the_dependency_must_live() {
    let (_temp, stderr, success) = run_project_with_config(
        "test-fn",
        cargo_manifest_dir(),
        &shared_target_dir(),
        &[
            (
                "Cargo.toml",
                &format!(
                    "[package]\nname = \"probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
                     [lib]\npath = \"src/lib.rs\"\n\n[workspace]\n\n\
                     [dev-dependencies]\n{}\n",
                    r#"command-extra = { path = "command-extra" }"#,
                ),
            ),
            ("src/lib.rs", TEST_FN_IN_PRODUCTION_MODULE),
            ("command-extra/Cargo.toml", STUB_MANIFEST),
            ("command-extra/src/lib.rs", STUB_SOURCE),
        ],
        "",
    );
    assert!(success, "`cargo dylint` failed; stderr was:\n{stderr}");
    assert!(
        !stderr.contains(&flagged_line("production-code")),
        "expected the library's own code to stay exempt; stderr was:\n{stderr}",
    );
    assert!(
        stderr.contains(&flagged_line("test-fn-in-production-module")),
        "expected the `#[test]` fn to be flagged; stderr was:\n{stderr}",
    );
    assert!(
        stderr.contains("the dependency has to reach every build of this module"),
        "expected the remedy to say where the dependency has to live, since \
         the import it asks for lands in the library's own root; stderr \
         was:\n{stderr}",
    );
}
