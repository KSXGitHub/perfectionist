//! UI tests for `macro_argument_binding`'s configuration knobs. The
//! default-config sweep lives in `ui/macro_argument_binding.rs` and is
//! picked up by `tests/ui.rs`; these tests each point at their own
//! one-fixture directory under `ui-toml/macro_argument_binding/` and
//! pass a per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! Mirrors the structure of `tests/macro_trailing_comma.rs` — same
//! shared-`Mutex` serialisation, same `dylint.toml` synthesis, same
//! one-fixture-per-knob layout.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::macro_argument_binding";

static SERIAL: Mutex<()> = Mutex::new(());

#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    deny_extra: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    allow_extra: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ignore: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extra_trivial_methods: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ignore_trivial_methods: Vec<String>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, config: RuleConfig) {
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn enabled_false_suppresses_the_entire_rule() {
    run(
        "ui-toml/macro_argument_binding/disabled",
        RuleConfig {
            enabled: Some(false),
            ..Default::default()
        },
    );
}

#[test]
fn deny_only_silently_accepts_uncatalogued_macros() {
    run(
        "ui-toml/macro_argument_binding/mode_deny_only",
        RuleConfig {
            mode: Some("deny_only"),
            ..Default::default()
        },
    );
}

#[test]
fn blanket_flags_allow_listed_macros_too() {
    run(
        "ui-toml/macro_argument_binding/mode_blanket",
        RuleConfig {
            mode: Some("blanket"),
            ..Default::default()
        },
    );
}

#[test]
fn deny_extra_adds_a_qualified_macro_to_the_deny_list() {
    run(
        "ui-toml/macro_argument_binding/deny_extra",
        RuleConfig {
            deny_extra: vec!["inner::their_macro".into()],
            ..Default::default()
        },
    );
}

#[test]
fn allow_extra_silences_an_uncatalogued_macro() {
    run(
        "ui-toml/macro_argument_binding/allow_extra",
        RuleConfig {
            allow_extra: vec!["my_macro".into()],
            ..Default::default()
        },
    );
}

#[test]
fn extra_trivial_methods_adds_a_pure_getter_to_the_postfix_set() {
    run(
        "ui-toml/macro_argument_binding/extra_trivial_methods",
        RuleConfig {
            extra_trivial_methods: vec!["cached_size".into()],
            ..Default::default()
        },
    );
}

#[test]
fn ignore_trivial_methods_drops_a_built_in_pure_getter() {
    run(
        "ui-toml/macro_argument_binding/ignore_trivial_methods",
        RuleConfig {
            ignore_trivial_methods: vec!["as_ref".into()],
            ..Default::default()
        },
    );
}

#[test]
fn ignore_suppresses_a_built_in_deny_listed_macro() {
    run(
        "ui-toml/macro_argument_binding/ignore",
        RuleConfig {
            ignore: vec!["debug_assert_eq".into()],
            ..Default::default()
        },
    );
}

#[test]
fn ignore_wins_over_deny_extra_for_the_same_macro() {
    run(
        "ui-toml/macro_argument_binding/ignore_overrides_deny_extra",
        RuleConfig {
            deny_extra: vec!["my_macro".into()],
            ignore: vec!["my_macro".into()],
            ..Default::default()
        },
    );
}

#[test]
fn fn_level_expect_fulfils_against_a_violation() {
    run(
        "ui-toml/macro_argument_binding/expect_at_fn_scope",
        RuleConfig::default(),
    );
}

#[test]
fn crate_level_expect_fulfils_against_a_violation() {
    run(
        "ui-toml/macro_argument_binding/expect_at_crate_root",
        RuleConfig::default(),
    );
}

#[test]
fn module_level_expect_fulfils_for_item_position_macro() {
    run(
        "ui-toml/macro_argument_binding/expect_at_module_with_item_macro",
        RuleConfig::default(),
    );
}
