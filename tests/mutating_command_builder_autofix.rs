//! End-to-end proof of which of `mutating_command_builder`'s
//! suggestions `cargo dylint --fix` is allowed to apply.
//!
//! Applicability is the one property a `.stderr` cannot show: a
//! `MachineApplicable` rename and a `MaybeIncorrect` one render
//! identically, and only the first is applied. So the fixture goes
//! through the real fixer and is judged on what it did to the source.
//!
//! What makes a rename sound is stated once, on
//! `rename_alone_compiles` in `src/rules/mutating_command_builder.rs`.
//! This file pins it, one fixture module per condition, rather than
//! restating it.
//!
//! `cargo fix` applies a crate's `MachineApplicable` suggestions,
//! recompiles, and on any new error throws the whole file's fixes away
//! with `errors present after applying fixes`. One over-confident
//! suggestion therefore costs every correct fix beside it, which is why
//! the assertion that the fixer stayed away from the unsound shapes
//! matters as much as the one that it rewrote the sound one.

pub mod _utils;

use _utils::{
    TempDir, build_project_with_config, cargo_manifest_dir, fixture_cargo_toml, run_dylint_fix,
    shared_target_dir,
};
use std::fs;
use text_block_macros::text_block_fnl;

/// The generated manifest with `command-extra` appended, which the
/// fixture needs and [`fixture_cargo_toml`] does not carry. Passing
/// `Cargo.toml` as a source overwrites the generated copy, since
/// [`build_project_with_config`] inserts the sources after its own
/// entries — appending to that copy rather than restating it keeps the
/// package, lib and workspace stanzas in one place.
fn cargo_toml() -> String {
    format!(
        "{}\n[dependencies]\ncommand-extra = \"1.2.0\"\n",
        fixture_cargo_toml("mutating_command_builder_autofix"),
    )
}

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
    "mod chain_on_a_local {"
    "    use command_extra::CommandExtra;"
    "    use std::process::Command;"
    ""
    "    // The rule's headline shape: a chain over a `mut` binding the"
    "    // following code still reads."
    "    pub fn lister() -> Command {"
    r#"        let mut command = Command::new("ls");"#
    r#"        command.current_dir("/").arg("chain-on-a-local");"#
    "        command"
    "    }"
    "}"
    ""
    "mod captured_by_a_closure {"
    "    use command_extra::CommandExtra;"
    "    use std::process::Command;"
    ""
    "    pub fn run() {"
    r#"        let mut command = Command::new("ls");"#
    r#"        let mut go = || { let _ = command.arg("captured").status(); };"#
    "        go();"
    "        go();"
    "    }"
    "}"
    ""
    "mod import_inside_a_body {"
    "    use std::process::Command;"
    ""
    "    // The module's only import of the trait is body-local, so it"
    "    // does not bring it into scope for the sibling below."
    "    pub fn imports_it_locally() {"
    "        use command_extra::CommandExtra;"
    r#"        let _ = Command::new("ls").with_arg("body-local");"#
    "    }"
    ""
    "    pub fn sibling() {"
    r#"        let _ = Command::new("ls").arg("body-local-sibling").status();"#
    "    }"
    "}"
    ""
    "mod other_applicability_gates {"
    "    use command_extra::CommandExtra;"
    "    use std::process::Command;"
    ""
    "    // A blanket impl instantiates `Self` to the receiver's type, so"
    "    // it sees `&mut Command` now and `Command` after a rename."
    "    pub trait Piped { fn piped<R>(self, f: impl FnOnce(Self) -> R) -> R where Self: Sized { f(self) } }"
    "    impl<T> Piped for T {}"
    ""
    "    pub fn blanket_impl_parent() {"
    r#"        Command::new("ls").arg("blanket-parent").piped(|c: &mut Command| { let _ = c.status(); });"#
    "    }"
    ""
    "    // A turbofish survives into a method of different generic arity."
    "    pub fn turbofish() {"
    r#"        let _ = Command::new("ls").args::<[&str; 1], &str>(["turbofish"]).status();"#
    "    }"
    ""
    "    // The value is a function argument, not a method receiver, so"
    "    // the changed type is what the context rejects."
    "    pub fn not_a_receiver() {"
    r#"        configure(Command::new("ls").arg("not-a-receiver"));"#
    "    }"
    "    fn configure(_c: &mut Command) {}"
    ""
    "    // A rename inside a macro body is written to the definition."
    r#"    macro_rules! add { ($c:expr) => { $c.arg("macro-body") }; }"#
    "    pub fn from_a_macro() {"
    r#"        let _ = add!(Command::new("ls")).status();"#
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
        &[("Cargo.toml", &cargo_toml()), ("src/lib.rs", SOURCE)],
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
        r#"command.current_dir("/").arg("chain-on-a-local");"#,
        r#"command.arg("captured")"#,
        r#".arg("blanket-parent")"#,
        r#".args::<[&str; 1], &str>(["turbofish"])"#,
        r#".arg("not-a-receiver")"#,
        r#"$c.arg("macro-body")"#,
        r#".arg("body-local-sibling")"#,
        r#".arg("receiver-out-of-scope")"#,
    ] {
        assert!(
            fixed.contains(untouched),
            "expected `{untouched}` to be left alone; the fixer left:\n{fixed}",
        );
    }
}
