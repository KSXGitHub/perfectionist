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

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    scan_string_literals: Option<bool>,
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
            scan_string_literals: None,
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
            scan_string_literals: None,
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
            scan_string_literals: None,
            allow_version_patterns: Some(true),
            hosts: None,
        },
    );
}

/// `scan_string_literals = true` turns on the opt-in string-literal
/// surface (off by default), so a branch ref in a string literal is
/// flagged while a SHA-pinned one is accepted.
#[test]
fn scan_string_literals_flags_branch_ref_in_literal() {
    run(
        "ui-toml/unpinned_repo_ref/scan_string_literals",
        RuleConfig {
            scan_string_literals: Some(true),
            allow_version_patterns: None,
            hosts: None,
        },
    );
}
