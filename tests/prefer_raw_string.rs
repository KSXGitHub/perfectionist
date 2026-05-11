//! UI tests for `prefer_raw_string`'s configuration knobs. The
//! default-config sweep lives in `ui/prefer_raw_string.rs` and is
//! picked up by `tests/ui.rs`; these tests each point at their own
//! one-fixture directory under `ui-toml/prefer_raw_string/` and
//! pass a per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared
//! `Mutex` to avoid clobbering each other under the default
//! parallel test harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::prefer_raw_string";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation. Kept as a separate type from the lint's own
/// internal `Config` so the test surface is independent of the
/// implementation's private struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_escapes_to_trigger: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    escapes_eligible: Option<Vec<String>>,
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
        "ui-toml/prefer_raw_string/disabled",
        RuleConfig {
            enabled: Some(false),
            ..Default::default()
        },
    );
}

#[test]
fn min_escapes_to_trigger_skips_single_escape_literals() {
    // A `\\`-only literal (one eliminable escape) stays untouched
    // under `min_escapes_to_trigger = 2`; the multi-`\\` literal
    // still fires.
    run(
        "ui-toml/prefer_raw_string/min_escapes_to_trigger",
        RuleConfig {
            min_escapes_to_trigger: Some(2),
            ..Default::default()
        },
    );
}

#[test]
fn escapes_eligible_subset_narrows_what_counts_as_eliminable() {
    // Restricting `escapes_eligible` to just `\"` means `\\` is
    // no longer considered eliminable. A literal whose only
    // escapes are `\\` therefore looks like it has non-raw
    // escapes and stays untouched; a `\"`-only literal still
    // fires.
    run(
        "ui-toml/prefer_raw_string/escapes_eligible_subset",
        RuleConfig {
            escapes_eligible: Some(vec![r#"\""#.into()]),
            ..Default::default()
        },
    );
}

#[test]
fn escapes_eligible_rejects_non_self_decoding_entries() {
    // Misconfigured `escapes_eligible = ["\\n"]`: `\n` decodes to
    // a newline, not the letter `n`, so accepting it as eliminable
    // would let the autofix corrupt newline-containing literals.
    // The filter at config load must drop the entry; the resulting
    // empty eligible set silently disables the rule for this
    // fixture.
    run(
        "ui-toml/prefer_raw_string/escapes_eligible_rejects_non_self_decoding",
        RuleConfig {
            escapes_eligible: Some(vec![r"\n".into()]),
            ..Default::default()
        },
    );
}
