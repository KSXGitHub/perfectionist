//! UI tests for `lint_silence_reason`'s configuration knobs. The
//! default-config sweep lives in `ui/lint_silence_reason.rs` and is
//! picked up by `tests/ui.rs`; these tests each point at their own
//! one-fixture directory under `ui-toml/lint_silence_reason/` and
//! pass a per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared
//! `Mutex` to avoid clobbering each other under the default
//! parallel test harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

use text_block_macros::text_block_fnl;

const LINT_NAME: &str = "perfectionist::lint_silence_reason";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation. Kept as a separate type from the lint's own
/// internal `Config` so the test surface is independent of the
/// implementation's private struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    exempt_lints: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_reason_length: Option<usize>,
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

#[test]
fn exempt_lints_skips_attributes_whose_every_lint_is_exempt() {
    run(
        "ui-toml/lint_silence_reason/exempt_lints",
        dylint_toml(RuleConfig {
            exempt_lints: Some(vec!["clippy::module_name_repetitions".into()]),
            ..Default::default()
        }),
    );
}

#[test]
fn min_reason_length_zero_disables_the_length_branch() {
    run(
        "ui-toml/lint_silence_reason/min_reason_length_zero",
        dylint_toml(RuleConfig {
            min_reason_length: Some(0),
            ..Default::default()
        }),
    );
}

#[test]
fn min_reason_length_eight_raises_the_floor() {
    run(
        "ui-toml/lint_silence_reason/min_reason_length_eight",
        dylint_toml(RuleConfig {
            min_reason_length: Some(8),
            ..Default::default()
        }),
    );
}

/// `disable = ["lint_silence_reason"]` in the `[perfectionist]`
/// global table skips this rule's pass entirely; the fixture's
/// missing `reason` produces no diagnostic.
#[test]
fn disable_in_global_table_suppresses_the_rule() {
    run(
        "ui-toml/lint_silence_reason/disabled",
        text_block_fnl! {
            "[perfectionist]"
            r#"disable = ["lint_silence_reason"]"#
        }
        .to_owned(),
    );
}
