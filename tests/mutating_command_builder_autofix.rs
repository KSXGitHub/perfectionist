//! End-to-end proof of which of `mutating_command_builder`'s
//! suggestions `cargo dylint --fix` is allowed to apply.
//!
//! Applicability is the one property a `.stderr` cannot show: a
//! `MachineApplicable` rename and a `MaybeIncorrect` one render
//! identically, and only the first is applied. So the fixture goes
//! through the real fixer and is judged on what it did to the source.
//!
//! The rename is sound only where two things hold, and each fails
//! loudly on its own:
//!
//! - `CommandExtra` is in scope in the *calling module*. Its methods
//!   are trait methods, so a rename without the import is `E0599`, and
//!   a parent module's `use` does not reach a child.
//! - The call's value feeds another method call's receiver. The
//!   by-value form returns `Command` where the original returned
//!   `&mut Command`, so a call in statement position moves a binding
//!   the following statements still read, which is `E0382`.
//!
//! `cargo fix` applies a crate's `MachineApplicable` suggestions,
//! recompiles, and on any new error throws the whole file's fixes away
//! with `errors present after applying fixes`. One over-confident
//! suggestion therefore costs every correct fix beside it, which is why
//! the assertion that the fixer stayed away from the unsound shapes
//! matters as much as the one that it rewrote the sound one.

pub mod _utils;

use _utils::{
    TempDir, build_project_with_config, cargo_manifest_dir, run_dylint_fix, shared_target_dir,
};
use std::fs;
use text_block_macros::text_block_fnl;

/// The fixture needs the real `command-extra`, which the generated
/// manifest does not carry. Passing `Cargo.toml` as a source overwrites
/// it, since [`build_project_with_config`] inserts the sources after
/// its own entries.
const CARGO_TOML: &str = text_block_fnl! {
    "[package]"
    r#"name = "mutating_command_builder_autofix""#
    r#"version = "0.0.0""#
    r#"edition = "2024""#
    ""
    "[lib]"
    r#"path = "src/lib.rs""#
    ""
    "[dependencies]"
    r#"command-extra = "1.2.0""#
    ""
    "# Declare an empty workspace so cargo doesn't walk up the"
    "# filesystem and try to enroll the fixture into the perfectionist"
    "# workspace it happens to be nested inside."
    "[workspace]"
};

/// Each call carries a distinct argument so an assertion can name one
/// shape without matching another.
const SOURCE: &str = text_block_fnl! {
    r##"#![allow(dead_code, unused_imports, unused_mut, reason = "fixture")]"##
    ""
    "mod trait_in_scope {"
    "    use command_extra::CommandExtra;"
    "    use std::process::Command;"
    ""
    "    // Fixable: the rename is the whole fix here."
    "    pub fn feeds_a_receiver() {"
    r#"        let _ = Command::new("ls").arg("receiver-in-scope").status();"#
    "    }"
    ""
    "    // Not fixable: a bare rename moves `command`, which the next"
    "    // statement still reads."
    "    pub fn statement_position() {"
    r#"        let mut command = Command::new("ls");"#
    r#"        command.arg("statement-in-scope");"#
    "        let _ = command.status();"
    "    }"
    "}"
    ""
    "// Not fixable: this module has no `use command_extra::CommandExtra`,"
    "// so a rename would be `no method named with_arg found`. The crate"
    "// is loaded -- the module above imports it -- so the dependency gate"
    "// passes and the diagnostic still fires."
    "mod trait_out_of_scope {"
    "    use std::process::Command;"
    ""
    "    pub fn feeds_a_receiver() {"
    r#"        let _ = Command::new("ls").arg("receiver-out-of-scope").status();"#
    "    }"
    "}"
};

/// Run the fixer over the fixture and hand back what it left on disk,
/// plus its stderr.
fn fix() -> (TempDir, String, String) {
    let temp = TempDir::new().expect("failed to create temp dir");
    build_project_with_config(
        temp.path(),
        "mutating_command_builder_autofix",
        cargo_manifest_dir(),
        &[("Cargo.toml", CARGO_TOML), ("src/lib.rs", SOURCE)],
        "",
    );
    let (stderr, success) = run_dylint_fix(temp.path(), &shared_target_dir());
    assert!(
        success,
        "`cargo dylint --fix` failed; stderr was:\n{stderr}",
    );
    let fixed = fs::read_to_string(temp.path().join("src/lib.rs")).expect("read fixed fixture");
    (temp, fixed, stderr)
}

#[test]
fn only_the_sound_rename_is_applied() {
    let (_temp, fixed, stderr) = fix();

    // The headline assertion. `cargo fix` prints this after applying a
    // suggestion that does not compile, having reverted the file — so
    // its absence is what says every applied rewrite was sound.
    assert!(
        !stderr.contains("errors present after applying fixes"),
        "the autofix produced code that does not compile; stderr was:\n{stderr}",
    );

    // A revert would leave every line untouched, which would pass the
    // "left alone" assertions below for the wrong reason.
    assert!(
        fixed.contains(r#".with_arg("receiver-in-scope")"#),
        "expected the fixer to rename the sound call; it left:\n{fixed}",
    );

    for untouched in [
        r#"command.arg("statement-in-scope");"#,
        r#".arg("receiver-out-of-scope")"#,
    ] {
        assert!(
            fixed.contains(untouched),
            "expected `{untouched}` to be left alone; the fixer left:\n{fixed}",
        );
    }
}
