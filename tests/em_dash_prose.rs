//! UI tests for `em_dash_prose`'s configuration knobs. The
//! default-config sweep lives in `ui/em_dash_prose.rs` and
//! `ui/em_dash_prose_messages.rs`, which `tests/ui.rs` picks up; the
//! tests here point at fixture directories under
//! `ui-toml/em_dash_prose/` and pass a per-rule `dylint.toml`.

use std::collections::BTreeMap;

const LINT_NAME: &str = "perfectionist::em_dash_prose";

/// Serialisation shim for the rule's `dylint.toml` configuration,
/// which the test crate cannot build from the lint's own private
/// `Config`.
#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    scan_doc_comments: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scan_regular_comments: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scan_macro_messages: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flag_en_dash: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flag_pseudo_em_dash: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'static str>,
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

/// `scan_doc_comments = false` takes the doc surface out while the
/// regular comments beside it keep firing, so the two surfaces are
/// independent rather than one switch.
#[test]
fn scan_doc_comments_false_keeps_regular_comments() {
    run(
        "ui-toml/em_dash_prose/no_doc_comments",
        RuleConfig {
            scan_doc_comments: Some(false),
            ..RuleConfig::default()
        },
    );
}

/// `scan_regular_comments = false` is the mirror image: the doc
/// comments keep firing and the regular ones fall silent.
#[test]
fn scan_regular_comments_false_keeps_doc_comments() {
    run(
        "ui-toml/em_dash_prose/no_regular_comments",
        RuleConfig {
            scan_regular_comments: Some(false),
            ..RuleConfig::default()
        },
    );
}

/// `scan_macro_messages = false` takes both message surfaces out, the
/// macro argument and the `expect` panic message alike, and leaves the
/// comment surfaces alone.
#[test]
fn scan_macro_messages_false_keeps_comments() {
    run(
        "ui-toml/em_dash_prose/no_macro_messages",
        RuleConfig {
            scan_macro_messages: Some(false),
            ..RuleConfig::default()
        },
    );
}

/// `flag_en_dash = false` drops U+2013 from the report. U+2014 has no
/// knob of its own, so it is reported either way.
#[test]
fn flag_en_dash_false_reports_the_em_dash_only() {
    run(
        "ui-toml/em_dash_prose/no_en_dash",
        RuleConfig {
            flag_en_dash: Some(false),
            ..RuleConfig::default()
        },
    );
}

/// `flag_pseudo_em_dash = false` drops the spaced hyphen run from the
/// report while U+2014 keeps firing.
#[test]
fn flag_pseudo_em_dash_false_reports_the_unicode_dash_only() {
    run(
        "ui-toml/em_dash_prose/no_pseudo_em_dash",
        RuleConfig {
            flag_pseudo_em_dash: Some(false),
            ..RuleConfig::default()
        },
    );
}

/// `message` replaces the help text wholesale. The note explaining
/// what a dash elides is not part of that text and stays in place.
#[test]
fn message_replaces_the_help_text() {
    run(
        "ui-toml/em_dash_prose/custom_message",
        RuleConfig {
            message: Some("see the house style guide at docs/style.md"),
            ..RuleConfig::default()
        },
    );
}

/// A per-item `#[allow]` silences only that item's comment, while a
/// sibling item with no attribute still fires, so per-site control
/// does not require a crate-root `#![allow]`.
#[test]
fn per_item_allow_suppresses_only_that_item() {
    run("ui-toml/em_dash_prose/allow_at_item", RuleConfig::default());
}

/// A per-item `#[expect]` both suppresses the finding and is fulfilled
/// by it, and a module-scope one does the same. The fixture produces
/// no diagnostics; a finding resolved at the crate root would fire
/// anyway and leave the expectation unfulfilled.
#[test]
fn per_item_expect_fulfils_and_suppresses() {
    run(
        "ui-toml/em_dash_prose/expect_at_item",
        RuleConfig::default(),
    );
}
