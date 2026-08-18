//! UI tests for `named_prelude_imports`'s configuration knobs. The
//! default-config sweep lives in `ui/named_prelude_imports.rs` and is
//! picked up by `tests/ui.rs`; these tests each point at their own
//! one-fixture directory under `ui-toml/named_prelude_imports/` and pass a
//! per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` sets the process-global `DYLINT_TOML` env var for
//! the duration of `run_tests`, so the `#[test]`s serialise on a shared
//! [`Mutex`] to avoid clobbering each other under the parallel harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::named_prelude_imports";

static SERIAL: Mutex<()> = Mutex::new(());

/// The subset of the rule's user-facing configuration these tests
/// exercise, mirrored here for serialisation.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    prelude_segment_names: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_paths: Option<Vec<&'static str>>,
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
fn allowed_paths_exempts_listed_prelude() {
    // `crate::prelude` is intentionally cherry-picked, so it is exempt;
    // a named import from a different prelude is still flagged.
    run(
        "ui-toml/named_prelude_imports/allowed_paths",
        RuleConfig {
            allowed_paths: Some(vec!["crate::prelude"]),
            ..Default::default()
        },
    );
}

#[test]
fn custom_prelude_segment_name() {
    // `prelude_segment_names = ["api"]`: a named import from an `api`
    // module is flagged, while one from a `prelude` module is not.
    run(
        "ui-toml/named_prelude_imports/custom_prelude_segment",
        RuleConfig {
            prelude_segment_names: Some(vec!["api"]),
            ..Default::default()
        },
    );
}
