//! UI tests for `unicode_ellipsis_in_panic_messages`'s configuration
//! knobs. The default-config sweep lives in
//! `ui/unicode_ellipsis_in_panic_messages.rs` and is picked up by
//! `tests/ui.rs`; this test points at a fixture directory under
//! `ui-toml/unicode_ellipsis_in_panic_messages/` and passes a
//! per-rule `dylint.toml` to `_utils::ConfiguredUiTest`.

use std::collections::BTreeMap;

const LINT_NAME: &str = "perfectionist::unicode_ellipsis_in_panic_messages";

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_macros: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ignore_macros: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_methods: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ignore_methods: Option<Vec<&'static str>>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, config: RuleConfig) {
    _utils::ConfiguredUiTest::builder()
        .library_name(env!("CARGO_PKG_NAME"))
        .manifest_dir(env!("CARGO_MANIFEST_DIR"))
        .src_base(src_base)
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn custom_macros_and_methods_extend_and_subtract_the_default_lists() {
    run(
        "ui-toml/unicode_ellipsis_in_panic_messages/custom_macros_and_methods",
        RuleConfig {
            extra_macros: Some(vec!["my_panic"]),
            ignore_macros: Some(vec!["panic"]),
            extra_methods: Some(vec!["expect_with"]),
            ignore_methods: Some(vec!["expect"]),
        },
    );
}
