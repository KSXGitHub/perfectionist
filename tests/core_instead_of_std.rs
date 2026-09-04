//! UI tests for `core_instead_of_std`. The rule is inactive by default,
//! so every fixture has to travel with a `dylint.toml` that enables it —
//! the default-configuration sweep included, which is why there is no
//! `ui/core_instead_of_std.rs` for `tests/ui.rs` to pick up. Each test
//! points at its own one-fixture directory under
//! `ui-toml/core_instead_of_std/`.
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared [`Mutex`]
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::core_instead_of_std";

static SERIAL: Mutex<()> = Mutex::new(());

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    also_alloc: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip_paths: Option<Vec<&'static str>>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    let rule_table = toml::to_string(&table).expect("serialise rule config as dylint.toml");
    // The rule is inactive by default, so every fixture's config has to
    // enable it before any knob it sets means anything.
    format!("[perfectionist]\nenable = [\"core_instead_of_std\"]\n\n{rule_table}")
}

fn run(src_base: &str, config: RuleConfig) {
    // A poisoned mutex from a previous panic doesn't make this lock
    // unsafe — recover the inner guard and proceed.
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        // The harness compiles a fixture in the 2015 edition by
        // default, where `core` and `alloc` are not in the extern
        // prelude and every fixture would need an `extern crate` line
        // this rule's audience does not write.
        .rustc_flags(["--edition=2021"])
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn default_config_covers_core_and_alloc() {
    // No knob set: `core::` and `alloc::` paths alike are flagged, in
    // every position a path can appear, and nothing is exempt.
    run("ui-toml/core_instead_of_std/default", RuleConfig::default());
}

#[test]
fn also_alloc_false_leaves_alloc_paths_alone() {
    // `also_alloc = false` turns off the `alloc` half of the rule and
    // leaves the `core` half exactly as it was.
    run(
        "ui-toml/core_instead_of_std/no_alloc",
        RuleConfig {
            also_alloc: Some(false),
            ..Default::default()
        },
    );
}

#[test]
fn skip_paths_exempts_a_listed_path() {
    // A `skip_paths` entry is never flagged, and it withdraws the
    // automatic rewrite from the names sharing its crate segment.
    run(
        "ui-toml/core_instead_of_std/skip_paths",
        RuleConfig {
            skip_paths: Some(vec!["::core::mem::transmute"]),
            ..Default::default()
        },
    );
}

#[test]
fn no_std_crate_is_left_alone() {
    // A `#![no_std]` crate has no `std::` to name, even when `std` is
    // linked into the same compilation.
    run("ui-toml/core_instead_of_std/no_std", RuleConfig::default());
}

#[test]
fn proc_macro_synthesised_path_is_not_flagged() {
    // Regression fixture for the `hir_in_external_macro` guard; see the
    // fixture's own header. It belongs under `ui-toml/` rather than
    // beside the other `ui/*_proc_macro.rs` fixtures because the rule
    // needs a `dylint.toml` to be installed at all.
    run(
        "ui-toml/core_instead_of_std/proc_macro",
        RuleConfig::default(),
    );
}
