//! UI tests for `single_letter_const_generic`'s configuration knob.
//! The default-config sweep lives in `ui/single_letter_const_generic.rs`
//! and is picked up by `tests/ui.rs`; this test points at a fixture
//! directory under `ui-toml/single_letter_const_generic/` and passes a
//! per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared
//! [`Mutex`] to avoid clobbering each other under the default
//! parallel test harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::single_letter_const_generic";

static SERIAL: Mutex<()> = Mutex::new(());

#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_idents: Option<Vec<char>>,
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
fn allowed_idents_exempts_listed_letters() {
    run(
        "ui-toml/single_letter_const_generic/allowed_idents",
        RuleConfig {
            allowed_idents: Some(vec!['N']),
        },
    );
}
