//! UI tests for `macro_trailing_comma`'s configuration knobs. The
//! default-config sweep lives in `ui/macro_trailing_comma.rs` and is
//! picked up by `tests/ui.rs`; these tests each point at their own
//! one-fixture directory under `ui-toml/macro_trailing_comma/` and
//! pass a per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared
//! [`Mutex`] to avoid clobbering each other under the default
//! parallel test harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::macro_trailing_comma";

static SERIAL: Mutex<()> = Mutex::new(());

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extra_macros: Vec<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ignore: Vec<&'static str>,
}

fn dylint_toml(config: RuleConfig) -> String {
    // A single-entry map gets serialised by `toml` as a top-level
    // table keyed by `LINT_NAME` — the same shape `dylint_linting`'s
    // `config_or_default` reads from `dylint.toml`. The `::` in the
    // key is quoted automatically.
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, config: RuleConfig) {
    // A poisoned mutex from a previous panic doesn't make this lock
    // unsafe — recover the inner guard and proceed.
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    let fixtures = _utils::copy_fixtures_with_directive(env!("CARGO_MANIFEST_DIR"), src_base);
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn extra_macros_enables_a_macro_by_bare_name() {
    run(
        "ui-toml/macro_trailing_comma/extra_macros",
        RuleConfig {
            extra_macros: vec!["my_macro"],
            ..Default::default()
        },
    );
}

#[test]
fn extra_macros_enables_a_macro_by_qualified_path() {
    // Multi-segment entries tail-match the invocation path, so a
    // third-party macro invoked as `inner::their_macro!` is matched
    // by the qualified entry `inner::their_macro`.
    run(
        "ui-toml/macro_trailing_comma/extra_macros_qualified",
        RuleConfig {
            extra_macros: vec!["inner::their_macro"],
            ..Default::default()
        },
    );
}

#[test]
fn ignore_suppresses_a_built_in_curated_macro() {
    run(
        "ui-toml/macro_trailing_comma/ignore",
        RuleConfig {
            ignore: vec!["vec"],
            ..Default::default()
        },
    );
}

#[test]
fn ignore_wins_over_extra_macros_for_the_same_macro() {
    run(
        "ui-toml/macro_trailing_comma/ignore_overrides_extra",
        RuleConfig {
            extra_macros: vec!["my_macro"],
            ignore: vec!["my_macro"],
        },
    );
}

#[test]
fn ignore_supports_qualified_path_and_whitespace_padding() {
    // Two coverage gaps in one fixture: a multi-segment `ignore`
    // entry (only `extra_macros`'s multi-segment branch was
    // previously exercised), and `parse_path`'s per-segment
    // whitespace trim — the literal `"  std::vec  "` should still
    // match a `std::vec!(...)` invocation.
    run(
        "ui-toml/macro_trailing_comma/ignore_qualified_with_whitespace",
        RuleConfig {
            ignore: vec!["  std::vec  "],
            ..Default::default()
        },
    );
}

#[test]
fn fn_level_expect_fulfils_against_a_violation() {
    // Companion to `crate_level_expect_...`: a function-scope
    // `#[expect]` must also fulfil, because the late pass emits at
    // the deepest enclosing HIR node rather than always at the
    // crate root.
    run(
        "ui-toml/macro_trailing_comma/expect_at_fn_scope",
        RuleConfig::default(),
    );
}

#[test]
fn crate_level_expect_fulfils_against_a_violation() {
    // Regression for
    // <https://github.com/KSXGitHub/parallel-disk-usage/issues/409>:
    // a crate-level `#![expect(...)]` must mark the expectation
    // fulfilled when a violation exists in the crate.
    run(
        "ui-toml/macro_trailing_comma/expect_at_crate_root",
        RuleConfig::default(),
    );
}
