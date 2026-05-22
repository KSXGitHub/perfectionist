//! UI tests for `bare_url`'s configuration knobs. The default-config
//! sweep lives in `ui/bare_url.rs` and is picked up by `tests/ui.rs`;
//! the configured tests here each point at their own one-fixture
//! directory under `ui-toml/bare_url/` and pass a per-rule
//! `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var
//! for the duration of `run_tests`. The env var is process-global,
//! so the `#[test]`s in this binary serialise themselves on a shared
//! `Mutex` to avoid clobbering each other under the default parallel
//! test harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::bare_url";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation. Kept as a separate type from the lint's own
/// internal `Config` so the test surface is independent of the
/// implementation's private struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_http: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip_hosts: Option<Vec<String>>,
}

fn dylint_toml(config: RuleConfig) -> String {
    #[derive(serde::Serialize)]
    struct WholeToml<'a> {
        #[serde(flatten)]
        rule: BTreeMap<&'a str, RuleConfig>,
    }
    let whole = WholeToml {
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
fn allow_http_false_rejects_http_urls() {
    // With `allow_http = false`, only `https://` is recognised as a
    // scheme. The `http://` URL in the fixture should not fire — the
    // entire rule's URL grammar narrows to the `https://` form.
    run(
        "ui-toml/bare_url/allow_http_false",
        RuleConfig {
            allow_http: Some(false),
            ..Default::default()
        },
    );
}
