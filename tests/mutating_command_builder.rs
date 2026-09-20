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
    _utils::ConfiguredUiTest::builder()
        .library_name(env!("CARGO_PKG_NAME"))
        .manifest_dir(env!("CARGO_MANIFEST_DIR"))
        .src_base(src_base)
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
