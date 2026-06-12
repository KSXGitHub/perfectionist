//! UI tests for `unsplit_print_macro`'s configuration knobs. The
//! default-config sweep lives in `ui/unsplit_print_macro.rs` and is
//! picked up by `tests/ui.rs`; these tests each point at their own
//! one-fixture directory under `ui-toml/unsplit_print_macro/` and pass a
//! per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared [`Mutex`]
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::unsplit_print_macro";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation. Kept as a separate type from the lint's own internal
/// `Config` so the test surface is independent of the implementation's
/// private struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_line_width: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_macros: Option<Vec<&'static str>>,
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
fn narrow_width_flags_a_line_within_the_default_limit() {
    // A line wider than 40 columns but well within the default 100
    // fires only once `max_line_width` is narrowed; a genuinely short
    // line stays untouched even then.
    run(
        "ui-toml/unsplit_print_macro/narrow_width",
        RuleConfig {
            max_line_width: Some(40),
            ..Default::default()
        },
    );
}

#[test]
fn target_macros_replaces_the_built_in_list_wholesale() {
    // Setting `target_macros` to a custom single-entry list makes that
    // macro eligible and drops `println!` from the built-in set, so the
    // `println!` call is no longer flagged.
    run(
        "ui-toml/unsplit_print_macro/custom_target_macros",
        RuleConfig {
            target_macros: Some(vec!["my_log"]),
            ..Default::default()
        },
    );
}
