//! Configured-`dylint.toml` UI tests for `unpinned_repo_ref`. The
//! default-config sweep lives in `ui/unpinned_repo_ref.rs` (picked up
//! by `tests/ui.rs`); the tests here each point at their own
//! one-fixture directory under `ui-toml/unpinned_repo_ref/` and pass a
//! per-rule `dylint.toml` to [`dylint_testing::ui::Test`].
//!
//! `Test::dylint_toml` sets the process-global `DYLINT_TOML` env var for
//! the duration of `run_tests`, so the `#[test]`s here serialise on a
//! shared [`Mutex`] to avoid clobbering each other under the parallel
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::unpinned_repo_ref";

static SERIAL: Mutex<()> = Mutex::new(());

/// The rule's user-facing configuration shape, mirrored here for
/// serialisation so the test surface stays independent of the lint's
/// private `Config` struct.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_version_patterns: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hosts: Option<Vec<HostEntry>>,
}

#[derive(serde::Serialize)]
struct HostEntry {
    hostname: &'static str,
    kind: &'static str,
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

/// An explicit `hosts = []` replaces the built-in host table with an
/// empty one, so no host is recognised and the rule stays silent even
/// on a forge URL the default table would flag. Regression test for
/// the `hosts = []` fallback removed in review (the rule used to fall
/// back to the built-in table on an empty list, making host scanning
/// impossible to disable through config).
#[test]
fn empty_hosts_disables_all_scanning() {
    run(
        "ui-toml/unpinned_repo_ref/empty_hosts",
        RuleConfig {
            allow_version_patterns: None,
            hosts: Some(vec![]),
        },
    );
}

/// A self-hosted instance registered with a `kind` is scanned under
/// that forge's URL shape — here a `gitlab`-shaped host that is not in
/// the built-in table.
#[test]
fn self_hosted_host_is_scanned() {
    run(
        "ui-toml/unpinned_repo_ref/self_hosted",
        RuleConfig {
            allow_version_patterns: None,
            hosts: Some(vec![HostEntry {
                hostname: "git.example.com",
                kind: "gitlab",
            }]),
        },
    );
}

/// `allow_version_patterns = true` accepts version-shaped refs while
/// still flagging ordinary branch refs in the same fixture.
#[test]
fn allow_version_patterns_accepts_only_version_shaped_refs() {
    run(
        "ui-toml/unpinned_repo_ref/allow_version_patterns",
        RuleConfig {
            allow_version_patterns: Some(true),
            hosts: None,
        },
    );
}
