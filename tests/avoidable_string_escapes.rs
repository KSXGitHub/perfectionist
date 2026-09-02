//! UI tests for `avoidable_string_escapes`'s configuration knobs. The
//! default-config sweep lives in `ui/avoidable_string_escapes.rs` and is
//! picked up by `tests/ui.rs`; these tests each point at their own
//! one-fixture directory under `ui-toml/avoidable_string_escapes/` and
//! pass a per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared
//! [`Mutex`] to avoid clobbering each other under the default
//! parallel test harness.

use core::num::NonZeroUsize;
use pipe_trait::Pipe;
use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::avoidable_string_escapes";

static SERIAL: Mutex<()> = Mutex::new(());

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    min_escapes_to_trigger: Option<NonZeroUsize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eligible_escapes: Option<Vec<&'static str>>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, config: RuleConfig) {
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    _utils::scratch::redirect_temp_dir();
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn min_escapes_to_trigger_skips_single_escape_literals() {
    // A `\\`-only literal (one eliminable escape) stays untouched
    // under `min_escapes_to_trigger = 2`; the multi-`\\` literal
    // still fires.
    run(
        "ui-toml/avoidable_string_escapes/min_escapes_to_trigger",
        RuleConfig {
            min_escapes_to_trigger: 2.pipe(NonZeroUsize::new).unwrap().pipe(Some),
            ..Default::default()
        },
    );
}

#[test]
fn eligible_escapes_subset_narrows_what_counts_as_eliminable() {
    // Restricting `eligible_escapes` to just `\"` means `\\` is
    // no longer considered eliminable. A literal whose only
    // escapes are `\\` therefore looks like it has non-raw
    // escapes and stays untouched; a `\"`-only literal still
    // fires.
    run(
        "ui-toml/avoidable_string_escapes/eligible_escapes_subset",
        RuleConfig {
            eligible_escapes: Some(vec![r#"\""#]),
            ..Default::default()
        },
    );
}

#[test]
fn eligible_escapes_rejects_non_self_decoding_entries() {
    // Misconfigured `eligible_escapes = ["\\n"]`: `\n` decodes to
    // a newline, not the letter `n`, so accepting it as eliminable
    // would let the autofix corrupt newline-containing literals.
    // The filter at config load must drop the entry; the resulting
    // empty eligible set silently disables the rule for this
    // fixture.
    run(
        "ui-toml/avoidable_string_escapes/eligible_escapes_rejects_non_self_decoding",
        RuleConfig {
            eligible_escapes: Some(vec![r"\n"]),
            ..Default::default()
        },
    );
}
