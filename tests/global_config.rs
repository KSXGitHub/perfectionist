//! Integration tests for the crate-wide `[perfectionist]` table in
//! `dylint.toml`. The per-rule `[perfectionist::<rule>]` tables are
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
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(dylint_toml_contents)
        .run();
}

/// `disable = ["<rule>"]` skips the rule's `register_pass` call, so
/// the fixture's violation produces no diagnostic. Reuses the
/// `macro_argument_binding/disabled` fixture — it has no `.stderr`
/// (no diagnostics expected) and otherwise triggers
/// `macro_argument_binding` at its default level.
#[test]
fn disable_in_global_table_suppresses_a_default_on_rule() {
    run(
        "ui-toml/macro_argument_binding/disabled",
        text_block_fnl! {
            "[perfectionist]"
            r#"disable = ["macro_argument_binding"]"#
        },
    );
}

/// Mixed-shape `disable` array: bare string and inline
/// `{ name, reason }` table in the same literal array. Same fixture
/// as above; the runtime treats the two shapes identically.
#[test]
fn disable_accepts_inline_table_with_reason() {
    run(
        "ui-toml/macro_argument_binding/disabled",
        text_block_fnl! {
            "[perfectionist]"
            "disable = ["
            r#"    { name = "macro_argument_binding", reason = "test-only" },"#
            "]"
        },
    );
}

/// `[[perfectionist.disable]]` array-of-tables form. Same fixture,
/// same expectation: no diagnostics.
#[test]
fn disable_accepts_array_of_tables_form() {
    run(
        "ui-toml/macro_argument_binding/disabled",
        text_block_fnl! {
            "[[perfectionist.disable]]"
            r#"name = "macro_argument_binding""#
            r#"reason = "test-only""#
        },
    );
}

/// `enable = ["<rule>"]` flips a default-off rule to on, so the
/// fixture's `pub enum FooError {}` produces the snapshot's
/// diagnostic. Reuses the `non_exhaustive_error/baseline` fixture
/// (the same one `tests/non_exhaustive_error.rs` exercises with the
/// `WholeToml` helper); this test exists to lock in the bare-string
/// form on the parser side without the per-rule scaffolding.
#[test]
fn enable_in_global_table_activates_a_default_off_rule() {
    run(
        "ui-toml/non_exhaustive_error/baseline",
        text_block_fnl! {
            "[perfectionist]"
            r#"enable = ["non_exhaustive_error"]"#
        },
    );
}

/// Mirrors [`disable_accepts_inline_table_with_reason`]: the inline
/// `{ name, reason }` table shape works on the `enable` side too,
/// since `RuleSelector` is shared.
#[test]
fn enable_accepts_inline_table_with_reason() {
    run(
        "ui-toml/non_exhaustive_error/baseline",
        text_block_fnl! {
            "[perfectionist]"
            "enable = ["
            r#"    { name = "non_exhaustive_error", reason = "test-only" },"#
            "]"
        },
    );
}

/// Mirrors [`disable_accepts_array_of_tables_form`] for `enable`.
#[test]
fn enable_accepts_array_of_tables_form() {
    run(
        "ui-toml/non_exhaustive_error/baseline",
        text_block_fnl! {
            "[[perfectionist.enable]]"
            r#"name = "non_exhaustive_error""#
            r#"reason = "test-only""#
        },
    );
}
