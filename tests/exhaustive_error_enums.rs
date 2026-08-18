//! UI tests for `exhaustive_error_enums`'s configuration knobs. The
//! rule is off by default, so it has no default-config sweep under
//! `ui/` for `tests/ui.rs` to pick up; these tests each point at their
//! own one-fixture directory under `ui-toml/exhaustive_error_enums/`
//! and pass a per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared
//! [`Mutex`] to avoid clobbering each other under the default
//! parallel test harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::exhaustive_error_enums";

static SERIAL: Mutex<()> = Mutex::new(());

/// The subset of the rule's user-facing configuration these tests
/// exercise, mirrored here for serialisation. Kept as a separate type
/// from the lint's own internal `Config` so the test surface is
/// independent of the implementation's private struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    require_for: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_suffixes: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ignore_suffixes: Option<Vec<&'static str>>,
}

/// The `[perfectionist]` table, kept minimal so every fixture below
/// turns the rule on. The rule ships disabled by default
/// (`DEFAULT_STATE = DefaultState::Disabled` in
/// `src/rules/exhaustive_error_enums.rs`), so without this `enable`
/// entry the pass would never register and
/// the fixture's `pub enum FooError {}` wouldn't trigger a diagnostic.
#[derive(serde::Serialize)]
struct GlobalConfig {
    enable: Vec<&'static str>,
}

fn dylint_toml(config: RuleConfig) -> String {
    // Serialise as two top-level tables: `[perfectionist]` (enables
    // the rule globally) and `["perfectionist::exhaustive_error_enums"]`
    // (per-rule knobs the test exercises). `toml::to_string` on a
    // serde-friendly wrapper emits both with one call; building the
    // string by concatenation would risk producing `[perfectionist]`
    // table contents bleeding into the rule table when the rule
    // config happens to be empty.
    #[derive(serde::Serialize)]
    struct WholeToml<'a> {
        perfectionist: GlobalConfig,
        #[serde(flatten)]
        rule: BTreeMap<&'a str, RuleConfig>,
    }
    let whole = WholeToml {
        perfectionist: GlobalConfig {
            enable: vec!["exhaustive_error_enums"],
        },
        rule: [(LINT_NAME, config)].into_iter().collect(),
    };
    toml::to_string(&whole).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, config: RuleConfig) {
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn baseline_sweep_with_rule_enabled() {
    // The rule is off by default; this is the equivalent of the
    // `ui/<rule>.rs` sweep every other rule has, just hosted under
    // `ui-toml/` because it needs a `dylint.toml` to opt the rule in.
    run(
        "ui-toml/exhaustive_error_enums/baseline",
        RuleConfig::default(),
    );
}

#[test]
fn pub_crate_includes_literal_pub_crate_items() {
    run(
        "ui-toml/exhaustive_error_enums/pub_crate",
        RuleConfig {
            require_for: Some("pub_crate".into()),
            ..Default::default()
        },
    );
}

#[test]
fn all_includes_private_items() {
    run(
        "ui-toml/exhaustive_error_enums/all",
        RuleConfig {
            require_for: Some("all".into()),
            ..Default::default()
        },
    );
}

#[test]
fn custom_suffixes_extend_and_subtract_the_default_list() {
    run(
        "ui-toml/exhaustive_error_enums/custom_suffixes",
        RuleConfig {
            extra_suffixes: Some(vec!["Failure"]),
            ignore_suffixes: Some(vec!["Error"]),
            ..Default::default()
        },
    );
}
