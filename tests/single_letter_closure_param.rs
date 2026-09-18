//! UI tests for `single_letter_closure_param`'s configuration knobs.
//! The default-config sweep lives in `ui/single_letter_names.rs` and
//! is picked up by `tests/ui.rs`; this test points at a fixture
//! directory under `ui-toml/single_letter_closure_param/` and passes
//! a per-rule `dylint.toml` to `_utils::configured_ui_test`.

use std::collections::BTreeMap;

const LINT_NAME: &str = "perfectionist::single_letter_closure_param";

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_trivial_callback_methods: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ignore_trivial_callback_methods: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_allowed_idents: Option<Vec<char>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_denied_idents: Option<Vec<char>>,
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
fn custom_trivial_callback_methods_extend_and_subtract_the_default_list() {
    run(
        "ui-toml/single_letter_closure_param/custom_trivial_callback_methods",
        RuleConfig {
            extra_trivial_callback_methods: Some(vec!["when"]),
            ignore_trivial_callback_methods: Some(vec!["sort_by"]),
            ..Default::default()
        },
    );
}
