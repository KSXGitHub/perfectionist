//! Integration tests for the crate-wide `[perfectionist]` table in
//! `dylint.toml`. The per-rule `["perfectionist::<rule>"]` tables are
//! exercised by each rule's own `tests/<rule>.rs`; these tests live
//! here because they're about the global table, which isn't tied to
//! any one rule.
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so
//! the `#[test]`s in this binary serialise themselves on a shared
//! [`Mutex`] to avoid clobbering each other under the default
//! parallel test harness.

use std::sync::Mutex;
use text_block_macros::text_block_fnl;

static SERIAL: Mutex<()> = Mutex::new(());

fn run(src_base: &str, dylint_toml_contents: &str) {
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    let fixtures = _utils::copy_fixtures_with_directive(env!("CARGO_MANIFEST_DIR"), src_base);
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml_contents)
        .run();
}

/// `disable = ["<rule>"]` skips the rule's `register_pass` call, so
/// the fixture's violation produces no diagnostic. Reuses the
/// `impure_macro_arguments/disabled` fixture — it has no `.stderr`
/// (no diagnostics expected) and otherwise triggers
/// `impure_macro_arguments` at its default level.
#[test]
fn disable_in_global_table_suppresses_a_default_on_rule() {
    run(
        "ui-toml/impure_macro_arguments/disabled",
        text_block_fnl! {
            "[perfectionist]"
            r#"disable = ["impure_macro_arguments"]"#
        },
    );
}

/// Mixed-shape `disable` array: bare string and inline
/// `{ name, reason }` table in the same literal array. Same fixture
/// as above; the runtime treats the two shapes identically.
#[test]
fn disable_accepts_inline_table_with_reason() {
    run(
        "ui-toml/impure_macro_arguments/disabled",
        text_block_fnl! {
            "[perfectionist]"
            "disable = ["
            r#"    { name = "impure_macro_arguments", reason = "test-only" },"#
            "]"
        },
    );
}

/// `[[perfectionist.disable]]` array-of-tables form. Same fixture,
/// same expectation: no diagnostics.
#[test]
fn disable_accepts_array_of_tables_form() {
    run(
        "ui-toml/impure_macro_arguments/disabled",
        text_block_fnl! {
            "[[perfectionist.disable]]"
            r#"name = "impure_macro_arguments""#
            r#"reason = "test-only""#
        },
    );
}

/// Disabling a rule skips its `register_pass` call only: the lint
/// declaration registers whatever the rule's resolved state, so a
/// call site that suppresses the rule keeps resolving. Guarding
/// `register_lint` the same way would turn every such
/// `#[allow(perfectionist::<rule>)]` into an `unknown_lints` warning
/// from rustc — the fixture carries one, and expects no output.
/// `allow_attributes` is turned off alongside it so that the
/// fixture's own suppression is not itself a finding.
#[test]
fn disable_keeps_the_lint_name_resolvable() {
    run(
        "ui-toml/impure_macro_arguments/disabled_with_allow",
        text_block_fnl! {
            "[perfectionist]"
            r#"disable = ["impure_macro_arguments", "allow_attributes"]"#
        },
    );
}

/// `enable = ["<rule>"]` flips a default-off rule to on, so the
/// fixture's `pub enum FooError {}` produces the snapshot's
/// diagnostic. Reuses the `exhaustive_error_enums/baseline` fixture
/// (the same one `tests/exhaustive_error_enums.rs` exercises with the
/// `WholeToml` helper); this test exists to lock in the bare-string
/// form on the parser side without the per-rule scaffolding.
#[test]
fn enable_in_global_table_activates_a_default_off_rule() {
    run(
        "ui-toml/exhaustive_error_enums/baseline",
        text_block_fnl! {
            "[perfectionist]"
            r#"enable = ["exhaustive_error_enums"]"#
        },
    );
}

/// Mirrors [`disable_accepts_inline_table_with_reason`]: the inline
/// `{ name, reason }` table shape works on the `enable` side too,
/// since `RuleSelector` is shared.
#[test]
fn enable_accepts_inline_table_with_reason() {
    run(
        "ui-toml/exhaustive_error_enums/baseline",
        text_block_fnl! {
            "[perfectionist]"
            "enable = ["
            r#"    { name = "exhaustive_error_enums", reason = "test-only" },"#
            "]"
        },
    );
}

/// Mirrors [`disable_accepts_array_of_tables_form`] for `enable`.
#[test]
fn enable_accepts_array_of_tables_form() {
    run(
        "ui-toml/exhaustive_error_enums/baseline",
        text_block_fnl! {
            "[[perfectionist.enable]]"
            r#"name = "exhaustive_error_enums""#
            r#"reason = "test-only""#
        },
    );
}
