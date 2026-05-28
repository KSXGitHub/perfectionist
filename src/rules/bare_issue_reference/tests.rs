use super::*;

#[test]
fn join_url_appends_path_to_base() {
    assert_eq!(
        join_url("https://github.com/owner/repo", "/issues/{number}", "42"),
        "https://github.com/owner/repo/issues/42",
    );
}

#[test]
fn join_url_collapses_double_slash_from_trailing_base() {
    assert_eq!(
        join_url("https://github.com/owner/repo/", "/pull/{number}", "7"),
        "https://github.com/owner/repo/pull/7",
    );
}

#[test]
fn join_url_adds_separator_when_template_lacks_leading_slash() {
    assert_eq!(
        join_url("https://example.com/o/r", "issues/{number}", "9"),
        "https://example.com/o/r/issues/9",
    );
}

#[test]
fn forge_paths_are_layout_specific() {
    assert_eq!(Forge::GitHub.issue_path(), "/issues/{number}");
    assert_eq!(Forge::GitHub.pr_path(), "/pull/{number}");
    assert_eq!(Forge::GitLab.issue_path(), "/-/issues/{number}");
    assert_eq!(Forge::GitLab.pr_path(), "/-/merge_requests/{number}");
    assert_eq!(Forge::Gitea.issue_path(), "/issues/{number}");
    assert_eq!(Forge::Gitea.pr_path(), "/pulls/{number}");
}

#[test]
fn forge_detects_known_hosts() {
    assert_eq!(Forge::detect("https://github.com/o/r"), Some(Forge::GitHub));
    assert_eq!(Forge::detect("https://gitlab.com/o/r"), Some(Forge::GitLab));
    assert_eq!(Forge::detect("https://gitea.com/o/r"), Some(Forge::Gitea));
    assert_eq!(
        Forge::detect("https://codeberg.org/o/r"),
        Some(Forge::Gitea),
    );
}

#[test]
fn forge_detection_is_host_case_insensitive() {
    assert_eq!(Forge::detect("https://GitHub.com/o/r"), Some(Forge::GitHub));
    assert_eq!(
        Forge::detect("https://GitLab.Example.com/o/r"),
        Some(Forge::GitLab),
    );
}

#[test]
fn forge_detects_self_hosted_subdomain_pattern() {
    // Self-hosted instances conventionally sit under a subdomain
    // named after the software; the leading label names the forge.
    assert_eq!(
        Forge::detect("https://gitlab.example.com/o/r"),
        Some(Forge::GitLab),
    );
    assert_eq!(
        Forge::detect("https://gitea.example.com/o/r"),
        Some(Forge::Gitea),
    );
    assert_eq!(
        Forge::detect("https://forgejo.example.com/o/r"),
        Some(Forge::Gitea),
    );
    // GitHub Enterprise Server shares github.com's path layout.
    assert_eq!(
        Forge::detect("https://github.example.com/o/r"),
        Some(Forge::GitHub),
    );
}

#[test]
fn forge_detection_has_no_fallback_for_unknown_hosts() {
    // No preference for any service: a host that names no forge
    // yields `None`, not a default forge.
    assert_eq!(Forge::detect("https://code.example.com/o/r"), None);
    // `git.*` is the most common generic prefix but ambiguous
    // across Gitea / GitLab / cgit / gitweb, so it's not matched.
    assert_eq!(Forge::detect("https://git.example.com/o/r"), None);
    // Bitbucket is unsupported (its `#NNN` convention is unclear),
    // so neither its cloud host nor a `bitbucket.*` subdomain is
    // matched.
    assert_eq!(Forge::detect("https://bitbucket.org/o/r"), None);
    assert_eq!(Forge::detect("https://bitbucket.example.com/o/r"), None);
}

#[test]
fn block_defines_reference_detects_existing_definition() {
    let block = "Closes [#99].\n\n[#99]: https://example.com/issues/99";
    assert!(block_defines_reference(block, "99"));
    // A different number, or no definition at all, is not a match.
    assert!(!block_defines_reference(block, "98"));
    assert!(!block_defines_reference("Closes #99.", "99"));
}

#[test]
fn unknown_forge_value_is_rejected() {
    // A recognised `forge` deserialises; an unrecognised one is a
    // hard deserialisation error. `dylint_linting::config_or_default`
    // surfaces that error as a panic at launch — it only falls
    // back to the default when the key is *absent*, not when it's
    // present-but-invalid — so a typo'd forge fails loudly.
    assert!(toml::from_str::<Config>(r#"forge = "gitlab""#).is_ok());
    assert!(toml::from_str::<Config>(r#"forge = "bitbucket""#).is_err());
}

#[test]
fn urls_are_derived_when_only_repository_is_set() {
    // The headline scenario: `repository` is set, `forge` is left
    // unset, and no path layout is configured. The forge is
    // detected from the recognised host and both URLs resolve to a
    // correct address — so the autofix works, not just help-only.
    // The GitHub and GitLab pair also confirms the derived layout
    // is forge-specific (`/pull/` vs `/-/merge_requests/`), not a
    // single hard-coded shape.
    let lint = |web_base: &str| BareIssueReference {
        forge: None,
        repo_web_base: Some(web_base.to_owned()),
        suggest_issue_url: true,
        suggest_pr_url: true,
        doc_comment_form: DocForm::Inline,
        include_plain_comments: false,
        plain_comment_form: PlainForm::BareUrl,
    };

    let github = lint("https://github.com/owner/repo");
    assert_eq!(github.effective_forge(), Some(Forge::GitHub));
    assert_eq!(
        github.issue_url("123").as_deref(),
        Some("https://github.com/owner/repo/issues/123"),
    );
    assert_eq!(
        github.pr_url("123").as_deref(),
        Some("https://github.com/owner/repo/pull/123"),
    );

    let gitlab = lint("https://gitlab.com/owner/repo");
    assert_eq!(gitlab.effective_forge(), Some(Forge::GitLab));
    assert_eq!(
        gitlab.issue_url("123").as_deref(),
        Some("https://gitlab.com/owner/repo/-/issues/123"),
    );
    assert_eq!(
        gitlab.pr_url("123").as_deref(),
        Some("https://gitlab.com/owner/repo/-/merge_requests/123"),
    );
}

#[test]
fn hash_reference_is_issue_only_on_gitlab() {
    // `#NNN` denotes a merge request on no forge — GitLab spells
    // those `!NNN` — so the PR suggestion is suppressed there but
    // offered on the others.
    let lint = |web_base: &str| BareIssueReference {
        forge: None,
        repo_web_base: Some(web_base.to_owned()),
        suggest_issue_url: true,
        suggest_pr_url: true,
        doc_comment_form: DocForm::Inline,
        include_plain_comments: false,
        plain_comment_form: PlainForm::BareUrl,
    };
    assert!(!lint("https://gitlab.com/o/r").hash_can_mean_pr());
    assert!(!lint("https://gitlab.example.com/o/r").hash_can_mean_pr());
    assert!(lint("https://github.com/o/r").hash_can_mean_pr());
    assert!(lint("https://gitea.com/o/r").hash_can_mean_pr());
}

#[test]
fn resolve_repository_passes_valid_url() {
    assert_eq!(
        resolve_repository(Some("git@github.com:owner/repo.git")).as_deref(),
        Some("https://github.com/owner/repo"),
    );
}

#[test]
fn resolve_repository_is_none_when_unset() {
    // Unset is legitimate — the lint runs help-only, no panic.
    assert_eq!(resolve_repository(None), None);
}

#[test]
#[should_panic(expected = "is not a parseable git URL")]
fn resolve_repository_panics_on_unparseable() {
    // A single-segment URL is a configuration mistake; fail loudly
    // at launch rather than silently degrade to help-only.
    resolve_repository(Some("https://github.com/owner"));
}

#[test]
fn no_urls_when_repository_host_is_unrecognised() {
    // The deliberate non-case: `repository` is set but its host
    // isn't a recognised service and `forge` is left unset. There
    // is no fallback forge, so both URLs are `None` — the rule
    // emits help-only output rather than guessing a layout.
    let lint = BareIssueReference {
        forge: None,
        repo_web_base: Some("https://git.example.com/owner/repo".to_owned()),
        suggest_issue_url: true,
        suggest_pr_url: true,
        doc_comment_form: DocForm::Inline,
        include_plain_comments: false,
        plain_comment_form: PlainForm::BareUrl,
    };
    assert_eq!(lint.effective_forge(), None);
    assert_eq!(lint.issue_url("123"), None);
    assert_eq!(lint.pr_url("123"), None);
}

#[test]
fn renders_inline_doc_suggestion() {
    assert_eq!(
        render_doc_suggestion(DocForm::Inline, "#42", "https://example.com/issues/42"),
        "[#42](https://example.com/issues/42)",
    );
}

#[test]
fn renders_reference_doc_suggestion() {
    assert_eq!(
        render_doc_suggestion(DocForm::Reference, "#42", "https://example.com/issues/42"),
        "[#42]",
    );
}

#[test]
fn renders_bare_url_doc_suggestion() {
    // The URL forms drop the `#N` token and substitute the URL.
    assert_eq!(
        render_doc_suggestion(DocForm::BareUrl, "#42", "https://example.com/issues/42"),
        "https://example.com/issues/42",
    );
}

#[test]
fn renders_bracketed_url_doc_suggestion() {
    assert_eq!(
        render_doc_suggestion(
            DocForm::BracketedUrl,
            "#42",
            "https://example.com/issues/42"
        ),
        "<https://example.com/issues/42>",
    );
}

#[test]
fn renders_plain_suggestion_bracketed() {
    assert_eq!(
        render_plain_suggestion(PlainForm::BracketedUrl, "https://example.com/issues/42"),
        "<https://example.com/issues/42>",
    );
}
