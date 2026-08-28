//! UI tests for `clap_help_markdown`'s configuration knobs. The
//! default-config sweep lives in `ui/clap_help_markdown.rs` and is
//! picked up by `tests/ui.rs`; the tests here point at fixture
//! directories under `ui-toml/clap_help_markdown/` and pass a
//! per-rule `dylint.toml` table.
//!
//! `Test::dylint_toml` sets the process-global `DYLINT_TOML` env var
//! for the duration of `run_tests`, so the `#[test]`s serialise on a
//! shared [`Mutex`] to avoid clobbering each other.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::clap_help_markdown";

static SERIAL: Mutex<()> = Mutex::new(());

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_constructs: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ignore_constructs: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    require_help_override: Option<bool>,
}

fn dylint_toml(config: RuleConfig) -> String {
    let table: BTreeMap<&str, RuleConfig> = [(LINT_NAME, config)].into_iter().collect();
    toml::to_string(&table).expect("serialise rule config as dylint.toml")
}

fn run(src_base: &str, config: RuleConfig) {
    let _serial = SERIAL.lock().unwrap_or_else(|err| err.into_inner());
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(dylint_toml(config))
        .run();
}

#[test]
fn extra_constructs_flags_bold_italic_and_lists() {
    // The opt-in extras are off by default; enabling all three flags
    // `**bold**`, `*italic*`, and list markers that the default set
    // leaves alone.
    run(
        "ui-toml/clap_help_markdown/extra_constructs",
        RuleConfig {
            extra_constructs: Some(vec!["bold", "italic", "list"]),
            ..RuleConfig::default()
        },
    );
}

#[test]
fn ignore_constructs_drops_a_construct_from_the_default_set() {
    // `ignore_constructs = ["inline_link"]` permits inline links in help
    // text while the rest of the default set — here, the code span —
    // still fires.
    run(
        "ui-toml/clap_help_markdown/ignore_constructs",
        RuleConfig {
            ignore_constructs: Some(vec!["inline_link"]),
            ..RuleConfig::default()
        },
    );
}

#[test]
fn require_help_override_flags_unoverridden_doc_comments() {
    // `require_help_override = true` flags every clap-derived doc comment
    // that feeds `--help` without an explicit override (container and
    // field docs alike), tolerates a missing override wherever there is
    // no doc comment, and supersedes the markdown scan — a code span in
    // an unoverridden doc is reported once as a missing override.
    run(
        "ui-toml/clap_help_markdown/require_help_override",
        RuleConfig {
            require_help_override: Some(true),
            ..RuleConfig::default()
        },
    );
}
