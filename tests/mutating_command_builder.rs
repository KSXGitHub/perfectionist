//! UI tests for `mutating_command_builder`'s configuration.
//!
//! The default-configuration sweep lives in
//! `ui/mutating_command_builder.rs`, and the gate's default is covered
//! by `ui/mutating_command_builder_no_dependency.rs`, which declares no
//! dependency on `command_extra` and expects silence. Of the knob's
//! other values, only `unchecked` changes what a fixture here sees:
//! compiletest gives the driver no manifest of the fixture's own, so
//! `crate` and `workspace` both find nothing and read alike. Telling
//! those two apart takes a real Cargo build, which
//! `tests/mutating_command_builder_dependency_gate.rs` does.
//!
//! No lock here, unlike the sibling config tests: this binary holds one
//! `#[test]`, and `dylint_testing`'s own `run_tests` takes a
//! process-global mutex before it sets `DYLINT_TOML`. A second lock
//! around a single caller would guard nothing.

use std::collections::BTreeMap;

const LINT_NAME: &str = "perfectionist::mutating_command_builder";

/// Serialisation shim for the rule's `dylint.toml` configuration, which
/// the test crate cannot build from the lint's own private `Config`.
/// The value is the enum variant as `dylint.toml` spells it.
#[derive(serde::Serialize)]
struct RuleConfig {
    command_extra_dependency: &'static str,
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
fn the_declaration_can_go_unchecked() {
    run(
        "ui-toml/mutating_command_builder/unchecked",
        RuleConfig {
            command_extra_dependency: "unchecked",
        },
    );
}
