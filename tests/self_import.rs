//! UI tests for `self_import`'s two styles. The rule is inactive by
//! default and `style` is mandatory once enabled, so each test points
//! at its own one-fixture directory under `ui-toml/self_import/` and
//! passes a `dylint.toml` that both enables the rule and selects the
//! style.
//!
//! Mirrors the structure of `tests/macro_argument_binding.rs` — shared
//! [`Mutex`] serialisation (the `DYLINT_TOML` env var is process-global)
//! and one fixture directory per case.
//!
//! `import_granularity` is disabled in each config: it is active by
//! default and would also fire on these fixtures (which deliberately
//! use crate-style nested `use`s to exercise nested-`self` handling),
//! so disabling it keeps the snapshots focused on `self_import`.

use std::sync::Mutex;

use text_block_macros::text_block_fnl;

static SERIAL: Mutex<()> = Mutex::new(());

fn run(src_base: &str, dylint_toml: &str) {
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(dylint_toml)
        .run();
}

#[test]
fn forbid_rewrites_every_self_form() {
    run(
        "ui-toml/self_import/forbid",
        text_block_fnl! {
            "[perfectionist]"
            r#"enable = ["self_import"]"#
            r#"disable = ["import_granularity"]"#
            ""
            r#"["perfectionist::self_import"]"#
            r#"style = "forbid""#
        },
    );
}

#[test]
fn combined_folds_adjacent_module_and_item_imports() {
    run(
        "ui-toml/self_import/combined",
        text_block_fnl! {
            "[perfectionist]"
            r#"enable = ["self_import"]"#
            r#"disable = ["import_granularity"]"#
            ""
            r#"["perfectionist::self_import"]"#
            r#"style = "combined""#
        },
    );
}
