//! UI tests for `unicode_ellipsis_in_panic_messages`'s configuration
//! knobs. The default-config sweep lives in
//! `ui/unicode_ellipsis_in_panic_messages.rs` and is picked up by
//! `tests/ui.rs`; this test points at a fixture directory under
//! `ui-toml/unicode_ellipsis_in_panic_messages/` and passes a
//! per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared
//! [`Mutex`] to avoid clobbering each other under the default
//! parallel test harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::unicode_ellipsis_in_panic_messages";

static SERIAL: Mutex<()> = Mutex::new(());

/// The subset of the rule's user-facing configuration these tests
/// exercise, mirrored here for serialisation. Kept as a separate type
/// from the lint's own internal `Config` so the test surface is
/// independent of the implementation's private struct.
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
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
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
