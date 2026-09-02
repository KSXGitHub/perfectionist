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
    repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggest_issue_url: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggest_pr_url: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    doc_comment_form: Option<String>,
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
    _utils::scratch::redirect_temp_dir();
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), src_base)
        .dylint_toml(dylint_toml(config))
        .run();
}

/// A repository on a `github.com` URL with `forge` omitted, so the
/// forge is detected from the host. Used by most fixtures.
fn github_repo(config: RuleConfig) -> RuleConfig {
    RuleConfig {
        repository: Some("https://github.com/owner/repo".into()),
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
            repository: Some("https://gitlab.com/owner/repo".into()),
            ..Default::default()
        },
    );
}

#[test]
fn ssh_url_is_parsed() {
    // An scp-like SSH clone URL (`git@host:owner/repo.git`) is parsed
    // into the canonical `https://github.com/owner/repo` web base, so
    // the GitHub forge is detected and the issue / PR links resolve —
    // the `git@` userinfo and `.git` suffix are dropped.
    run(
        "ui-toml/bare_issue_reference/ssh_url",
        RuleConfig {
            repository: Some("git@github.com:owner/repo.git".into()),
            ..Default::default()
        },
    );
}

#[test]
fn unrecognised_host_without_forge_is_help_only() {
    // The negative counterpart to `gitlab_host_is_detected`:
    // `repository` is set, but to a self-hosted host the lint can't
    // classify, and `forge` is omitted. With no detectable forge and
    // no fallback, the lint can't build a URL — so it degrades to
    // help-only, telling the author to set `forge`.
    run(
        "ui-toml/bare_issue_reference/unrecognised_host",
        RuleConfig {
            repository: Some("https://git.example.com/owner/repo".into()),
            ..Default::default()
        },
    );
}

#[test]
fn self_hosted_subdomain_is_detected() {
    // A self-hosted GitLab under the conventional `gitlab.` subdomain
    // is recognised without an explicit `forge`, yielding GitLab's
    // `/-/issues/` and `/-/merge_requests/` paths on the custom host.
    run(
        "ui-toml/bare_issue_reference/subdomain",
        RuleConfig {
            repository: Some("https://gitlab.example.com/owner/repo".into()),
            ..Default::default()
        },
    );
}

#[test]
fn self_hosted_needs_explicit_forge() {
    // A self-hosted instance on a host that names no forge
    // (`git.example.com`) can't be detected, so the forge is given
    // explicitly. `gitlab` paths are then used on the custom host.
    run(
        "ui-toml/bare_issue_reference/self_hosted",
        RuleConfig {
            forge: Some("gitlab".into()),
            repository: Some("https://git.example.com/owner/repo".into()),
            ..Default::default()
        },
    );
}

#[test]
fn single_selection_is_still_maybe_incorrect() {
    // Even with exactly one knob enabled (here `suggest_issue_url`)
    // the lone suggestion stays `MaybeIncorrect`: a bare `#NNN` is
    // ambiguous about whether it names a reference at all, so the
    // lint is never confident enough for a machine-applicable fix.
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
fn reference_form_appends_definition() {
    // The `doc_comment_form = "reference"` fix is a multipart edit:
    // it rewrites `#99` to `[#99]` and appends the matching `[#99]: URL`
    // definition (after a blank `///` line) at the end of the block.
    run(
        "ui-toml/bare_issue_reference/reference_form",
        github_repo(RuleConfig {
            suggest_issue_url: Some(true),
            suggest_pr_url: Some(false),
            doc_comment_form: Some("reference".into()),
            ..Default::default()
        }),
    );
}

#[test]
fn indented_continuation_line_is_scanned() {
    // A reference on an indented continuation line (a doc block inside
    // an `impl`) is scanned, and the reference-form fix appends the
    // definition with the same indented `///` prefix.
    run(
        "ui-toml/bare_issue_reference/indented",
        github_repo(RuleConfig {
            suggest_issue_url: Some(true),
            suggest_pr_url: Some(false),
            doc_comment_form: Some("reference".into()),
            ..Default::default()
        }),
    );
}

#[test]
fn reference_form_skips_existing_definition() {
    // When the block already defines `[#99]: URL`, the fix rewrites
    // only the `#99` token and does not append a duplicate definition.
    run(
        "ui-toml/bare_issue_reference/reference_defined",
        github_repo(RuleConfig {
            suggest_issue_url: Some(true),
            suggest_pr_url: Some(false),
            doc_comment_form: Some("reference".into()),
            ..Default::default()
        }),
    );
}

/// Regression test for
/// <https://github.com/KSXGitHub/perfectionist/issues/165>: a per-item
/// `#[expect]` on the documented item both suppresses the bare `#NNN`
/// finding in its doc comment and is fulfilled by it. The fixture
/// produces no diagnostics; before the fix the finding resolved to the
/// crate root, firing anyway and leaving the expectation unfulfilled.
#[test]
fn per_item_expect_fulfils_and_suppresses() {
    run(
        "ui-toml/bare_issue_reference/expect_at_item",
        RuleConfig::default(),
    );
}
