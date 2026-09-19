//! UI tests for `mutating_command_builder`'s configuration.
//!
//! The default-configuration sweep lives in
//! `ui/mutating_command_builder.rs`, and the gate's default is covered
//! by `ui/mutating_command_builder_no_dependency.rs`, which loads no
//! `command_extra` and expects silence. Only the knob's `false` value
//! needs a `dylint.toml`, so only that case lives here.
//!
//! `Test::dylint_toml` works by setting the `DYLINT_TOML` env var for
//! the duration of `run_tests`. The env var is process-global, so the
//! `#[test]`s in this binary serialise themselves on a shared [`Mutex`]
//! to avoid clobbering each other under the default parallel test
//! harness.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

const LINT_NAME: &str = "perfectionist::mutating_command_builder";

static SERIAL: Mutex<()> = Mutex::new(());

/// Serialisation shim for the rule's `dylint.toml` configuration, which
/// the test crate cannot build from the lint's own private `Config`.
#[derive(serde::Serialize)]
struct RuleConfig {
    require_command_extra_dependency: bool,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, config: RuleConfig) {
    // A poisoned mutex from a previous panic doesn't make this lock
    // unsafe — recover the inner guard and proceed.
    let _serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    let fixtures = _utils::copy_fixtures_with_directives(env!("CARGO_MANIFEST_DIR"), src_base);
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn the_gate_can_be_turned_off() {
    run(
        "ui-toml/mutating_command_builder/gate_disabled",
        RuleConfig {
            require_command_extra_dependency: false,
        },
    );
}
