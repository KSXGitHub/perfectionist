//! UI tests for `unordered_derives`'s configuration knobs. The default-
//! config sweep lives in `ui/unordered_derives.rs` and is picked up by
//! `tests/ui.rs`; these tests each point at their own one-fixture
//! directory under `ui-toml/unordered_derives/` and pass a per-rule
//! `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared [`Mutex`]
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::unordered_derives";

static SERIAL: Mutex<()> = Mutex::new(());

/// Serialisation shim for the rule's `dylint.toml` configuration. Kept
/// as a separate type from the lint's own internal `Config` so the test
/// surface is independent of the implementation's private struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix: Option<Vec<&'static str>>,
}

/// The `[perfectionist]` table, kept minimal so every fixture below
/// turns the rule on. The rule ships disabled by default
/// (`DEFAULT_STATE = DefaultState::Disabled` in
/// `src/rules/unordered_derives.rs`), so without this `enable` entry
/// the pass would never register and the fixture's out-of-order
/// derives wouldn't trigger a diagnostic.
#[derive(serde::Serialize)]
struct GlobalConfig {
    enable: Vec<&'static str>,
}

fn dylint_toml(config: RuleConfig) -> String {
    // Serialise as two top-level tables: `[perfectionist]` (enables
    // the rule globally) and `["perfectionist::unordered_derives"]`
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
            enable: vec!["unordered_derives"],
        },
        rule: [(LINT_NAME, config)].into_iter().collect(),
    };
    toml::to_string(&whole).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, config: RuleConfig) {
    // A poisoned mutex from a previous panic doesn't make this lock
    // unsafe — recover the inner guard and proceed.
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn alphabetical_style_flags_out_of_order_derives() {
    run(
        "ui-toml/unordered_derives/alphabetical",
        RuleConfig {
            style: Some("alphabetical"),
            ..Default::default()
        },
    );
}

#[test]
fn prefix_then_alphabetical_uses_default_prefix() {
    // No `prefix` override: the default prefix (`Debug`, `Default`,
    // `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`,
    // `Hash`) is used.
    run(
        "ui-toml/unordered_derives/prefix_then_alphabetical_default",
        RuleConfig {
            style: Some("prefix_then_alphabetical"),
            ..Default::default()
        },
    );
}

#[test]
fn prefix_then_alphabetical_uses_custom_prefix() {
    // A two-trait custom prefix promotes only those two traits ahead
    // of the alphabetised tail, exercising the configuration path.
    run(
        "ui-toml/unordered_derives/prefix_then_alphabetical_custom",
        RuleConfig {
            style: Some("prefix_then_alphabetical"),
            prefix: Some(vec!["Hash", "Debug"]),
        },
    );
}
