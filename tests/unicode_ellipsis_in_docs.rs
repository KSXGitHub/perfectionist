//! UI tests for `unicode_ellipsis_in_docs`. The default-config sweep
//! lives in `ui/unicode_ellipsis_in_docs.rs` and is picked up by
//! `tests/ui.rs`; the tests here point at fixture directories under
//! `ui-toml/unicode_ellipsis_in_docs/` and pass a `dylint.toml` —
//! either a per-rule config table or the `[perfectionist]` global
//! enable/disable table.
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared [`Mutex`]
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;
use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::unicode_ellipsis_in_docs";

static SERIAL: Mutex<()> = Mutex::new(());

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_flagged_chars: Option<Vec<char>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scan_code_spans: Option<bool>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, contents: &str) {
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    _utils::scratch::redirect_temp_dir();
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(contents)
        .run();
}

#[test]
fn extra_flagged_chars_extends_the_default_set() {
    run(
        "ui-toml/unicode_ellipsis_in_docs/extra_flagged_chars",
        &dylint_toml(RuleConfig {
            extra_flagged_chars: Some(vec!['\u{22EF}']),
            ..RuleConfig::default()
        }),
    );
}

#[test]
fn scan_code_spans_true_flags_inside_code_spans() {
    run(
        "ui-toml/unicode_ellipsis_in_docs/flag_in_code_spans",
        &dylint_toml(RuleConfig {
            scan_code_spans: Some(true),
            ..RuleConfig::default()
        }),
    );
}

/// Symmetric counterpart to
/// `unicode_ellipsis_in_comments`'s cross-rule test: with the sibling
/// comment rule disabled, the docs rule flags only the `///` / `//!`
/// doc comments and leaves the `//` and `/* */` comments untouched —
/// pinning the claim that the docs rule does not intrude into regular
/// comments.
#[test]
fn docs_rule_does_not_intrude_into_regular_comments() {
    run(
        "ui-toml/unicode_ellipsis_in_docs/ignores_regular_comments",
        text_block_fnl! {
            "[perfectionist]"
            r#"disable = ["unicode_ellipsis_in_comments"]"#
        },
    );
}

/// Regression test for
/// <https://github.com/KSXGitHub/perfectionist/issues/165>: a per-item
/// / per-module `#[expect]` both suppresses the doc-comment finding and
/// is fulfilled by it. The fixture produces no diagnostics; before the
/// fix it emitted the finding *and* an `unfulfilled_lint_expectations`.
#[test]
fn per_item_expect_fulfils_and_suppresses() {
    run(
        "ui-toml/unicode_ellipsis_in_docs/expect_at_item",
        &dylint_toml(RuleConfig::default()),
    );
}

/// Companion to [`per_item_expect_fulfils_and_suppresses`]: a per-item
/// `#[allow]` silences only that item's doc comment, while a sibling
/// item with no attribute still fires — so per-site control no longer
/// requires a crate-root `#![allow]` that exempts every doc comment.
#[test]
fn per_item_allow_suppresses_only_that_item() {
    run(
        "ui-toml/unicode_ellipsis_in_docs/allow_at_item",
        &dylint_toml(RuleConfig::default()),
    );
}
