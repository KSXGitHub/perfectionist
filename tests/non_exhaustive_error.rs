//! UI tests for `non_exhaustive_error`'s configuration knobs. The
//! default-config sweep lives in `ui/non_exhaustive_error.rs` and is
//! picked up by `tests/ui.rs`; these tests each point at their own
//! one-fixture directory under `ui-toml/non_exhaustive_error/` and
//! pass a per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared
//! `Mutex` to avoid clobbering each other under the default
//! parallel test harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::non_exhaustive_error";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation. Kept as a separate type from the lint's own internal
/// `Config` so the test surface is independent of the implementation's
/// private struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    require_for: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suffixes: Option<Vec<String>>,
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
fn pub_crate_includes_literal_pub_crate_items() {
    run(
        "ui-toml/non_exhaustive_error/pub_crate",
        RuleConfig {
            require_for: Some("pub_crate".into()),
            ..Default::default()
        },
    );
}

#[test]
fn all_includes_private_items() {
    run(
        "ui-toml/non_exhaustive_error/all",
        RuleConfig {
            require_for: Some("all".into()),
            ..Default::default()
        },
    );
}

#[test]
fn custom_suffixes_replace_the_default_list() {
    run(
        "ui-toml/non_exhaustive_error/custom_suffixes",
        RuleConfig {
            suffixes: Some(vec!["Failure".into()]),
            ..Default::default()
        },
    );
}
