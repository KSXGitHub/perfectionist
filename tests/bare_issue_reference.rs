//! UI tests for `bare_issue_reference`'s configuration knobs. See
//! the module docs on `tests/bare_url.rs` for the shared pattern.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::bare_issue_reference";

static SERIAL: Mutex<()> = Mutex::new(());

#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    repo_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggest_issue_url: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggest_pr_url: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    form: Option<String>,
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

#[test]
fn both_suggestions_are_maybe_incorrect_by_default() {
    // Both `suggest_issue_url` and `suggest_pr_url` default to
    // `true`, so setting only `repo_base_url` exercises the default:
    // a bare `#NNN` is ambiguous between an issue and a PR, so the
    // rule emits two `MaybeIncorrect` suggestions (one `/issues/`
    // URL, one `/pull/` URL) and lets the author pick.
    run(
        "ui-toml/bare_issue_reference/both",
        RuleConfig {
            repo_base_url: Some("https://github.com/owner/repo".into()),
            ..Default::default()
        },
    );
}

#[test]
fn single_selection_is_machine_applicable() {
    // With exactly one of the two knobs enabled (here
    // `suggest_issue_url`) and the default `form = "inline"`, the
    // author has told the lint which kind the number names, so the
    // single suggestion is `MachineApplicable`.
    run(
        "ui-toml/bare_issue_reference/issue_only",
        RuleConfig {
            repo_base_url: Some("https://github.com/owner/repo".into()),
            suggest_issue_url: Some(true),
            suggest_pr_url: Some(false),
            ..Default::default()
        },
    );
}

#[test]
fn reference_form_always_degrades_to_maybe_incorrect() {
    // The `form = "reference"` suggestion produces just `[#NNN]`,
    // without the matching `[#NNN]: URL` definition (which the
    // author must add). Applying that suggestion as-is would leave
    // the doc block with an undefined reference link — so
    // applicability degrades to `MaybeIncorrect` even for a single
    // selection that would otherwise be machine-applicable.
    run(
        "ui-toml/bare_issue_reference/reference_form",
        RuleConfig {
            repo_base_url: Some("https://github.com/owner/repo".into()),
            suggest_issue_url: Some(true),
            suggest_pr_url: Some(false),
            form: Some("reference".into()),
        },
    );
}
