//! UI tests for `bare_issue_reference`'s configuration knobs. See
//! the module docs on `tests/bare_url.rs` for the shared pattern.

use std::collections::BTreeMap;
use std::sync::Mutex;

const LINT_NAME: &str = "perfectionist::bare_issue_reference";

static SERIAL: Mutex<()> = Mutex::new(());

#[derive(Default, serde::Serialize)]
struct RuleConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    forge: Option<String>,
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

/// A repository on a `github.com` URL with `forge` omitted, so the
/// forge is detected from the host. Used by most fixtures.
fn github_repo(config: RuleConfig) -> RuleConfig {
    RuleConfig {
        repo_base_url: Some("https://github.com/owner/repo".into()),
        ..config
    }
}

#[test]
fn both_suggestions_are_maybe_incorrect_by_default() {
    // `forge` is omitted and detected from the `github.com` host;
    // `suggest_issue_url` / `suggest_pr_url` both default to `true`,
    // so a bare `#NNN` (ambiguous between issue and PR) yields two
    // `MaybeIncorrect` suggestions (`/issues/` and `/pull/`).
    run(
        "ui-toml/bare_issue_reference/both",
        github_repo(RuleConfig::default()),
    );
}

#[test]
fn gitlab_host_is_detected() {
    // `forge` omitted: detected as GitLab from the `gitlab.com`
    // host, yielding `/-/issues/` and `/-/merge_requests/`.
    run(
        "ui-toml/bare_issue_reference/gitlab",
        RuleConfig {
            repo_base_url: Some("https://gitlab.com/owner/repo".into()),
            ..Default::default()
        },
    );
}

#[test]
fn self_hosted_needs_explicit_forge() {
    // A self-hosted instance's host isn't recognised, so the forge
    // can't be detected — it's given explicitly. `gitlab` paths are
    // then used on the custom host.
    run(
        "ui-toml/bare_issue_reference/self_hosted",
        RuleConfig {
            forge: Some("gitlab".into()),
            repo_base_url: Some("https://gitlab.example.com/owner/repo".into()),
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
        github_repo(RuleConfig {
            suggest_issue_url: Some(true),
            suggest_pr_url: Some(false),
            ..Default::default()
        }),
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
        github_repo(RuleConfig {
            suggest_issue_url: Some(true),
            suggest_pr_url: Some(false),
            form: Some("reference".into()),
            ..Default::default()
        }),
    );
}
