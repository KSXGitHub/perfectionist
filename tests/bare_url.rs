//! UI tests for `bare_url`'s configuration knobs. The default-config
//! sweep lives in `ui/bare_url.rs` and is picked up by `tests/ui.rs`;
//! the configured tests here each point at their own one-fixture
//! directory under `ui-toml/bare_url/` and pass a per-rule
//! `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var
//! for the duration of `run_tests`. The env var is process-global,
//! so the `#[test]`s in this binary serialise themselves on a shared
//! [`Mutex`] to avoid clobbering each other under the default parallel
//! test harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::bare_url";

static SERIAL: Mutex<()> = Mutex::new(());

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    skip_hosts: Option<Vec<&'static str>>,
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
    let fixtures = _utils::copy_fixtures_with_directive(env!("CARGO_MANIFEST_DIR"), src_base);
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn custom_skip_hosts_replaces_the_default_list() {
    // A user-supplied `skip_hosts` replaces the built-in default
    // (`localhost`). The fixture verifies the configured host is
    // suppressed while another host, not in the custom list, still
    // fires.
    run(
        "ui-toml/bare_url/custom_skip_hosts",
        RuleConfig {
            skip_hosts: Some(vec!["example.net"]),
        },
    );
}

/// Regression test for
/// <https://github.com/KSXGitHub/perfectionist/issues/165>: a per-item
/// `#[expect]` on the documented item both suppresses the bare-URL
/// finding in its doc comment and is fulfilled by it. The fixture
/// produces no diagnostics; before the fix the finding resolved to the
/// crate root, firing anyway and leaving the expectation unfulfilled.
#[test]
fn per_item_expect_fulfils_and_suppresses() {
    run("ui-toml/bare_url/expect_at_item", RuleConfig::default());
}
