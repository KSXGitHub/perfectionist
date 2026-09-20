//! End-to-end proof that `mutating_command_builder` never has
//! `cargo dylint --fix` rewrite anything.
//!
//! Applicability is the one property a `.stderr` cannot show: a
//! suggestion rendered as a concrete rewrite looks the same whether or
//! not the fixer will apply it. This rule offers none that it will,
//! because whether the rename compiles cannot be decided without
//! re-typechecking — the reasoning is on the lint's own rustdoc.
//!
//! Four rounds of review each found a fresh way the `&mut Command` ->
//! `Command` change ripples, the last of them silent: a by-value
//! extension-trait method is found before an inherent `&mut self` one,
//! so renaming one link of a chain can redirect the next link to the
//! author's own method with nothing failing to compile. `cargo fix`
//! reverts a file whose fixes error; it cannot revert one whose fixes
//! merely change behaviour.
//!
//! So the fixture collects every shape the rule fires on and asserts the
//! source came back untouched.

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

/// The fixture crate's source. The fixer is asserted to hand it back
/// byte-identical, so it lives in a file rather than in a literal here:
/// what the assertion compares is what a reader edits.
const SOURCE: &str = include_str!("fixtures/mutating_command_builder_autofix/lib.rs");

/// Sibling rules would rewrite the same lines on their own account,
/// which would make "did the fixer touch this line?" answer the wrong
/// question -- and any error one of them introduced would be blamed on
/// this rule by the headline assertion below.
const CONFIG: &str = text_block_fnl! {
    "[perfectionist]"
    r#"disable = ["bare_identifier_reference", "import_granularity_mismatch", "import_grouping_mismatch"]"#
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
        CONFIG,
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
#[ignore = "builds the lint and resolves `command-extra` from the registry in a fresh fixture crate"]
fn the_fixer_rewrites_nothing() {
    let (_temp, fixed, stderr) = fix();

    // `cargo fix` prints this after applying a suggestion that does not
    // compile, having reverted the file. Nothing here should be applied
    // in the first place, so it should never appear.
    assert!(
        !stderr.contains("errors present after applying fixes"),
        "the autofix produced code that does not compile; stderr was:\n{stderr}",
    );

    // A setter inside a `macro_rules!` body is reported at the macro
    // definition, once per invocation, and any rewrite would land on
    // every call site. The rule stays out of expansions entirely.
    assert!(
        !stderr.contains("macro-body"),
        "expected no diagnostic inside the macro body; stderr was:\n{stderr}",
    );

    assert_eq!(
        fixed, SOURCE,
        "the fixer rewrote the fixture; it should leave every shape alone",
    );

    // And the rule fired on every shape, so the assertion above is not
    // passing because the fixture went quiet. The distinct arguments are
    // what let one shape be named without matching another.
    for shape in [
        "receiver-in-scope",
        "statement-in-scope",
        "chain-on-a-local",
        "blanket-parent",
        "turbofish",
        "not-a-receiver",
        "body-local-sibling",
        "receiver-out-of-scope",
    ] {
        assert!(
            stderr.contains(shape),
            "expected the rule to fire on `{shape}`; stderr was:\n{stderr}",
        );
    }
}
