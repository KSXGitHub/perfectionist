//! UI tests for `unicode_ellipsis_in_docs`'s configuration knobs. The
//! default-config sweep lives in `ui/unicode_ellipsis_in_docs.rs` and
//! is picked up by `tests/ui.rs`; this test points at fixture
//! directories under `ui-toml/unicode_ellipsis_in_docs/` and passes a
//! per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared `Mutex`
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::unicode_ellipsis_in_docs";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation. Kept as a separate type from the lint's own internal
/// `Config` so the test surface is independent of the implementation's
/// private struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_flagged_chars: Option<Vec<char>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_in_code_spans: Option<bool>,
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
fn extra_flagged_chars_extends_the_default_set() {
    run(
        "ui-toml/unicode_ellipsis_in_docs/extra_flagged_chars",
        RuleConfig {
            extra_flagged_chars: Some(vec!['\u{22EF}']),
            ..RuleConfig::default()
        },
    );
}

#[test]
fn allow_in_code_spans_false_flags_inside_code_spans() {
    run(
        "ui-toml/unicode_ellipsis_in_docs/flag_in_code_spans",
        RuleConfig {
            allow_in_code_spans: Some(false),
            ..RuleConfig::default()
        },
    );
}
