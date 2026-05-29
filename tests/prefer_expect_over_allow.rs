//! UI tests for `prefer_expect_over_allow`'s configuration knobs. The
//! default-config sweep lives in `ui/prefer_expect_over_allow.rs` and is
//! picked up by `tests/ui.rs`; these tests each point at their own
//! one-fixture directory under `ui-toml/prefer_expect_over_allow/` and
//! pass a per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared `Mutex`
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::prefer_expect_over_allow";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation. Kept as a separate type from the lint's own internal
/// `Config` so the test surface is independent of the implementation's
/// private struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    exempt_lints: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    apply_to_outer_scopes: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    apply_to_tool_namespaces: Option<bool>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, contents: &str) {
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(contents)
        .run();
}

/// `apply_to_outer_scopes = true` makes crate-level `#![allow(...)]` and
/// outer `#[allow(...)]` on `mod` items eligible for rewriting.
#[test]
fn apply_to_outer_scopes_rewrites_module_and_inner_attributes() {
    run(
        "ui-toml/prefer_expect_over_allow/apply_to_outer_scopes",
        &dylint_toml(RuleConfig {
            apply_to_outer_scopes: Some(true),
            ..Default::default()
        }),
    );
}

/// `apply_to_tool_namespaces = false` leaves `perfectionist::*` (and any
/// non-`clippy` / non-`rustdoc` namespace) alone while still rewriting
/// `clippy::*`, `rustdoc::*`, and built-in lints.
#[test]
fn apply_to_tool_namespaces_false_skips_perfectionist_lints() {
    run(
        "ui-toml/prefer_expect_over_allow/apply_to_tool_namespaces_false",
        &dylint_toml(RuleConfig {
            apply_to_tool_namespaces: Some(false),
            ..Default::default()
        }),
    );
}

/// A custom `exempt_lints` replaces the default list outright, so a
/// default member like `dead_code` becomes rewriteable and a freshly
/// listed lint becomes exempt.
#[test]
fn exempt_lints_replaces_the_default_list() {
    run(
        "ui-toml/prefer_expect_over_allow/exempt_lints",
        &dylint_toml(RuleConfig {
            exempt_lints: Some(vec!["clippy::too_many_arguments"]),
            ..Default::default()
        }),
    );
}

/// `disable = ["prefer_expect_over_allow"]` in the `[perfectionist]`
/// global table skips this rule's pass entirely.
#[test]
fn disable_in_global_table_suppresses_the_rule() {
    run(
        "ui-toml/prefer_expect_over_allow/disabled",
        text_block_fnl! {
            "[perfectionist]"
            r#"disable = ["prefer_expect_over_allow"]"#
        },
    );
}
