//! UI tests for `mutating_command_builder`'s configuration.
//!
//! The default-configuration sweep lives in
//! `ui/mutating_command_builder.rs`, and the gate's default is covered
//! by `ui/mutating_command_builder_no_dependency.rs`, which loads no
//! `command_extra` and expects silence. Only the knob's `false` value
//! needs a `dylint.toml`, so only that case lives here.
//!
//! No lock here, unlike the sibling config tests: this binary holds one
//! `#[test]`, and `dylint_testing`'s own `run_tests` takes a
//! process-global mutex before it sets `DYLINT_TOML`. A second lock
//! around a single caller would guard nothing. A future test added
//! beside this one does not change that, though it would make the
//! sibling files' reasoning worth re-reading.

use std::collections::BTreeMap;

const LINT_NAME: &str = "perfectionist::mutating_command_builder";

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
