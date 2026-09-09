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
    #[serde(skip_serializing_if = "Option::is_none")]
    include_plain_comments: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plain_comment_form: Option<String>,
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
    let fixtures = _utils::copy_fixtures_with_directive(env!("CARGO_MANIFEST_DIR"), src_base);
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), fixtures.path())
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
fn no_suggestion_knobs_names_both_knobs() {
    // The counterpart to `unrecognised_host_without_forge_is_help_only`
    // on the other side of the `issue_url` split: the repository *is*
    // resolvable, so the lint could build a link, but both suggestion
    // knobs are off. It degrades to help-only and names the two knobs
    // that would turn the suggestions back on.
    run(
        "ui-toml/bare_issue_reference/no_suggestion_knobs",
        github_repo(RuleConfig {
            suggest_issue_url: Some(false),
            suggest_pr_url: Some(false),
            ..Default::default()
        }),
    );
}

#[test]
fn gitlab_no_suggestion_knobs_names_only_the_issue_knob() {
    // On GitLab `suggest_pr_url` can never produce a suggestion — a
    // bare `#NNN` is always an issue there — so with
    // `suggest_issue_url` off the lint reaches the same help-only arm
    // even though `suggest_pr_url` is on. The help must name only
    // `suggest_issue_url`: telling the author to enable a knob that
    // is already `true` would leave the diagnostic unchanged.
    run(
        "ui-toml/bare_issue_reference/gitlab_pr_only",
        RuleConfig {
            repository: Some("https://gitlab.com/owner/repo".into()),
            suggest_issue_url: Some(false),
            suggest_pr_url: Some(true),
            ..Default::default()
        },
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
fn bare_url_form_substitutes_the_url_on_both_surfaces() {
    // The `bare_url` form drops the `#NNN` token and puts the URL in
    // its place, in doc comments and — with `include_plain_comments`
    // on — in plain line comments, whose shape comes from
    // `plain_comment_form` (`bare_url` by default).
    run(
        "ui-toml/bare_issue_reference/bare_url_form",
        github_repo(RuleConfig {
            suggest_issue_url: Some(true),
            suggest_pr_url: Some(false),
            doc_comment_form: Some("bare_url".into()),
            include_plain_comments: Some(true),
            ..Default::default()
        }),
    );
}

#[test]
fn bracketed_url_form_wraps_the_url_on_both_surfaces() {
    // The counterpart to `bare_url_form_substitutes_the_url_on_both_surfaces`:
    // `bracketed_url` substitutes the same URL but wraps it in `<...>`.
    run(
        "ui-toml/bare_issue_reference/bracketed_url_form",
        github_repo(RuleConfig {
            suggest_issue_url: Some(true),
            suggest_pr_url: Some(false),
            doc_comment_form: Some("bracketed_url".into()),
            include_plain_comments: Some(true),
            plain_comment_form: Some("bracketed_url".into()),
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
