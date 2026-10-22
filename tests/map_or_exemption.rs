//! UI tests for the `exempt_simple_default` knob the two
//! defaulted-mapper rules share. The default-config sweep lives in
//! `ui/map_or.rs` and `ui/map_or_else.rs`, which `tests/ui.rs` picks
//! up; the tests here point at fixture directories under
//! `ui-toml/map_or/` and `ui-toml/map_or_else/` and pass a per-rule
//! `dylint.toml`.

use std::collections::BTreeMap;

/// Serialisation shim for either rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`. Both rules spell the knob the same way, so one shim
/// serves them both.
#[derive(serde::Serialize)]
struct RuleConfig {
    exempt_simple_default: bool,
}

fn dylint_toml(lint_name: &str, config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(lint_name, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

/// Lint `src_base` with the exemption turned off for `lint_name`.
fn run_without_the_exemption(src_base: &str, lint_name: &str) {
    _utils::ConfiguredUiTest::builder()
        .library_name(env!("CARGO_PKG_NAME"))
        .manifest_dir(env!("CARGO_MANIFEST_DIR"))
        .src_base(src_base)
        .dylint_toml(dylint_toml(
            lint_name,
            RuleConfig {
                exempt_simple_default: false,
            },
        ))
        .run();
}

/// `exempt_simple_default = false` flags a literal or path default
/// like any other, and each diagnostic still names what the call
/// collapses into. Every default that reaches the predicate,
/// `and_then` and `map_or_default` helps is simple, so this fixture is
/// where those helps are reachable at all.
#[test]
fn flagging_a_simple_default_reaches_every_help() {
    run_without_the_exemption(
        "ui-toml/map_or/flag_simple_default",
        "perfectionist::map_or",
    );
}

/// The sibling rule's knob reads the same over the defaults that rule
/// can have, which is a path: a closure is flagged whatever the knob
/// says.
#[test]
fn flagging_a_path_default_reaches_the_sibling_rule() {
    run_without_the_exemption(
        "ui-toml/map_or_else/flag_simple_default",
        "perfectionist::map_or_else",
    );
}
