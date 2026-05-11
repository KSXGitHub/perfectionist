//! UI tests for `derive_ordering`'s configuration knobs. The default-
//! config sweep lives in `ui/derive_ordering.rs` and is picked up by
//! `tests/ui.rs`; these tests each point at their own one-fixture
//! directory under `ui-toml/derive_ordering/` and pass a per-rule
//! `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared `Mutex`
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::derive_ordering";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation. Kept as a separate type from the lint's own internal
/// `Config` so the test surface is independent of the implementation's
/// private struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix: Option<Vec<&'static str>>,
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
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn alphabetical_style_flags_out_of_order_derives() {
    run(
        "ui-toml/derive_ordering/alphabetical",
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
        "ui-toml/derive_ordering/prefix_then_alphabetical_default",
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
        "ui-toml/derive_ordering/prefix_then_alphabetical_custom",
        RuleConfig {
            style: Some("prefix_then_alphabetical"),
            prefix: Some(vec!["Hash", "Debug"]),
        },
    );
}
