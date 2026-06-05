//! UI test for `combined_self_import`. The rule is inactive by default,
//! so the test points at its fixture directory under
//! `ui-toml/combined_self_import/` and passes a `dylint.toml` that
//! enables it.
//!
//! `import_granularity` is disabled in the config: it is active by
//! default and would also fire on these fixtures (which deliberately use
//! crate-style nested `use`s), so disabling it keeps the snapshot
//! focused on `combined_self_import`.

use text_block_macros::text_block_fnl;

#[test]
fn folds_adjacent_module_and_item_imports() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui-toml/combined_self_import")
        .dylint_toml(text_block_fnl! {
            "[perfectionist]"
            r#"enable = ["combined_self_import"]"#
            r#"disable = ["import_granularity"]"#
        })
        .run();
}
