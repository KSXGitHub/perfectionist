//! UI tests for `single_letter_static_item`'s configuration knob.
//! The default-config sweep lives in `ui/single_letter_static_item.rs`
//! and is picked up by `tests/ui.rs`; this test points at a fixture
//! directory under `ui-toml/single_letter_static_item/` and passes a
//! per-rule `dylint.toml` to `_utils::ConfiguredUiTest`.

use std::collections::BTreeMap;

const LINT_NAME: &str = "perfectionist::single_letter_static_item";

#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_idents: Option<Vec<char>>,
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
fn allowed_idents_exempts_listed_letters() {
    run(
        "ui-toml/single_letter_static_item/allowed_idents",
        RuleConfig {
            allowed_idents: Some(vec!['N']),
        },
    );
}
