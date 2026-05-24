//! UI tests for `lint_reason_from_comment`'s configuration knobs.
//! The default-config sweep lives in `ui/lint_reason_from_comment.rs`
//! and is picked up by `tests/ui.rs`; these tests each point at their
//! own one-fixture directory under `ui-toml/lint_reason_from_comment/`
//! and pass a per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared
//! `Mutex` to avoid clobbering each other under the default
//! parallel test harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::lint_reason_from_comment";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation. Kept as a separate type from the lint's own
/// internal `Config` so the test surface is independent of the
/// implementation's private struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    lift_trailing_comments: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lift_leading_comments: Option<bool>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, contents: String) {
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(contents)
        .run();
}

/// `lift_leading_comments = false` keeps the trailing placement but
/// silences the leading one: only the trailing-comment attribute is
/// flagged.
#[test]
fn trailing_only_skips_leading_comments() {
    run(
        "ui-toml/lint_reason_from_comment/trailing_only",
        dylint_toml(RuleConfig {
            lift_leading_comments: Some(false),
            ..Default::default()
        }),
    );
}

/// `lift_trailing_comments = false` keeps the leading placement but
/// silences the trailing one: only the leading-comment attribute is
/// flagged.
#[test]
fn leading_only_skips_trailing_comments() {
    run(
        "ui-toml/lint_reason_from_comment/leading_only",
        dylint_toml(RuleConfig {
            lift_trailing_comments: Some(false),
            ..Default::default()
        }),
    );
}

/// Both placements disabled — the pass installs but the early
/// `check_attribute` guard returns before any scan, so nothing is
/// flagged even on the canonical trailing shape.
#[test]
fn both_disabled_silences_the_rule() {
    run(
        "ui-toml/lint_reason_from_comment/both_disabled",
        dylint_toml(RuleConfig {
            lift_trailing_comments: Some(false),
            lift_leading_comments: Some(false),
        }),
    );
}

/// `disable = ["lint_reason_from_comment"]` in the `[perfectionist]`
/// global table skips this rule's pass entirely; the fixture's
/// adjacent comments produce no diagnostic.
#[test]
fn disable_in_global_table_suppresses_the_rule() {
    run(
        "ui-toml/lint_reason_from_comment/disabled",
        text_block_fnl! {
            "[perfectionist]"
            r#"disable = ["lint_reason_from_comment"]"#
        }
        .to_owned(),
    );
}
