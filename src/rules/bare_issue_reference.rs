//! `perfectionist::bare_issue_reference` — flag bare `#NNN` issue /
//! PR references in doc comments (and optionally plain `//` line
//! comments), suggesting the markdown-link form.

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_ast::Crate;
use rustc_errors::Applicability;
use rustc_lint::{EarlyContext, EarlyLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

use crate::comment_walk::{CommentChunk, CommentSurface, walk_local_comments};
use crate::common::{DefaultState, resolved_state};
use crate::markdown::{position_in_skip, scan_skip_regions, utf8_char_len};
use crate::url_scan::back_scan_url_fragment;

mod repo_url;

declare_tool_lint! {
    /// ### What it does
    /// Flags bare `#NNN` issue / pull-request references in doc
    /// comments (`///`, `//!`) — and, when opted in, in plain `//`
    /// line comments. The autofix rewrites the reference; the `form`
    /// knob selects the shape (inline `[#123](URL)`, reference
    /// `[#123]`, a bare URL, or a `<URL>` autolink).
    ///
    /// A bare `#NNN` is deeply ambiguous: it might be an issue, a
    /// pull request, a colour like `#123`, or any other numbered
    /// item, so no suggestion is ever `MachineApplicable`. The
    /// `suggest_issue_url` / `suggest_pr_url` knobs choose which link
    /// target(s) the autofix offers — each `MaybeIncorrect` — and
    /// with neither enabled the lint is help-only. The author can
    /// also resolve the ambiguity by enclosing the token in backticks
    /// (so it reads as code) or by using a spelling without a leading
    /// `#`.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue. A
    /// bare `#123` renders as literal text in CommonMark; only
    /// GitHub's markdown flavour autolinks the token, and only when
    /// the rendering surface is itself within a GitHub repository
    /// view. The link form renders portably across rustdoc, GitHub,
    /// and any other markdown engine.
    ///
    /// ### Example
    /// ```rust,ignore
    /// /// Closes #123 and supersedes #124.
    /// ```
    /// Use instead (with
    /// `repository = "https://github.com/owner/repo"` — `forge`
    /// is detected from the host), picking the issue link for one
    /// and the pull-request link for the other:
    /// ```rust,ignore
    /// /// Closes [#123](https://github.com/owner/repo/issues/123) and
    /// /// supersedes [#124](https://github.com/owner/repo/pull/124).
    /// ```
    pub perfectionist::BARE_ISSUE_REFERENCE,
    Warn,
    "ambiguous bare `#NNN` issue / PR reference in comment",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::bare_issue_reference";

/// Markdown-link shape produced by the autofix inside doc comments.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum DocForm {
    /// `[#123](URL)` — the URL is inlined. Keeps `#123` as the
    /// visible link text.
    #[default]
    Inline,
    /// `[#123]` — the matching `[#123]: URL` reference-link
    /// definition is the author's responsibility (the lint can't
    /// safely synthesise a multi-line definition without knowing
    /// where the doc block ends), so this suggestion is incomplete
    /// on its own.
    Reference,
    /// `https://.../issues/123` — the bare URL replaces the `#123`
    /// token outright (the `#123` text is not kept). NB: in a doc
    /// comment the sibling `perfectionist::bare_url` lint then flags
    /// the substituted URL; pick `bracketed_url` for a form it
    /// accepts.
    BareUrl,
    /// `<https://.../issues/123>` — a markdown autolink replaces the
    /// `#123` token outright. `bare_url` accepts this form.
    BracketedUrl,
}

/// URL shape used inside plain `//` comments when
/// `include_plain_comments = true`.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum PlainForm {
    /// Substitute the URL itself (`https://...`), unwrapped.
    /// Many editors auto-detect a bare URL as clickable. NB: the
    /// sibling `perfectionist::bare_url` lint, whose default also
    /// scans regular comments, will then flag the substituted URL —
    /// pick `bracketed_url` to produce a form both rules accept.
    #[default]
    BareUrl,
    /// Substitute `<https://...>`. The angle-bracket delimiter gives
    /// the URL a clear boundary when it abuts surrounding
    /// punctuation; editors that auto-link URLs typically recognise
    /// it, and `bare_url` accepts it.
    BracketedUrl,
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Git-hosting service the repository is on — one of `github`,
    /// `gitlab`, `gitea` — which fixes the issue / PR path layout.
    /// When unset, it is detected from the `repository` host: the
    /// public instances (`github.com`, `gitlab.com`, `codeberg.org`,
    /// `gitea.com`) and the conventional self-hosted subdomains
    /// `gitlab.*`, `github.*`, `gitea.*` and `forgejo.*` all need no
    /// `forge`. Set it explicitly for a self-hosted instance on a
    /// host that gives no such hint (e.g. `git.example.com`). No
    /// fixed default — the rule prefers no service.
    forge: Option<Forge>,
    /// The repository's URL, in any form you'd clone or paste: an
    /// HTTP(S) URL (`"https://github.com/owner/repo"`), an `ssh://`
    /// URL (`"ssh://git@github.com/owner/repo.git"`), or the scp-like
    /// shorthand (`"git@github.com:owner/repo.git"`). The `git@`
    /// userinfo, any `:port`, an optional `.git` suffix, and trailing
    /// slashes are all ignored. It supplies the host (for forge
    /// detection) and the owner/repo path the issue / PR link needs,
    /// so it is required for an autofix; when unset the lint flags
    /// bare references but emits help-only output. No fixed default.
    repository: Option<String>,
    /// Offer a suggestion that links the reference as an *issue*.
    /// Defaults to `true`.
    suggest_issue_url: bool,
    /// Offer a suggestion that links the reference as a *pull
    /// request*. Defaults to `true`. Ignored on GitLab, where a bare
    /// `#NNN` is always an issue (merge requests are written `!NNN`),
    /// so only the issue suggestion is offered there.
    suggest_pr_url: bool,
    /// Doc-comment fix form: `inline` for `[#N](URL)`, `reference`
    /// for the two-piece `[#N]` + `[#N]: URL` form. The reference
    /// form's autofix only rewrites the `#N` token; the matching
    /// definition is the author's responsibility. Defaults to
    /// `inline`. Ignored for plain-comment fixes — those follow
    /// `plain_comment_form` instead.
    form: DocForm,
    /// When `true`, also lint plain `//` line comments. The
    /// autofix in plain comments uses `plain_comment_form`'s URL
    /// shape (since plain comments aren't markdown). Plain block
    /// comments (`/* ... */`) are out of scope regardless.
    /// Defaults to `false`.
    include_plain_comments: bool,
    /// Replacement form used inside plain `//` comments when
    /// `include_plain_comments = true`. Defaults to `bare_url`.
    /// Ignored for doc comments and when no repository is
    /// configured.
    plain_comment_form: PlainForm,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            forge: None,
            repository: None,
            suggest_issue_url: true,
            suggest_pr_url: true,
            form: DocForm::Inline,
            include_plain_comments: false,
            plain_comment_form: PlainForm::BareUrl,
        }
    }
}

pub struct BareIssueReference {
    forge: Option<Forge>,
    /// The repository's `repository` URL normalised to the canonical
    /// web base `https://host/owner/repo` (see [`repo_url::normalize`]),
    /// or `None` when unset or unparseable.
    repo_web_base: Option<String>,
    suggest_issue_url: bool,
    suggest_pr_url: bool,
    form: DocForm,
    include_plain_comments: bool,
    plain_comment_form: PlainForm,
}

/// Resolve the `repository` config value to its canonical web base.
/// `None` when unset — the lint then runs help-only. **Panics** when
/// it is set but unparseable, so a malformed URL fails loudly at
/// launch rather than silently disabling the autofix and leaving the
/// author to wonder why no link is suggested.
fn resolve_repository(repository: Option<&str>) -> Option<String> {
    repository.map(|raw| {
        repo_url::normalize(raw).unwrap_or_else(|| {
            panic!(
                "perfectionist::bare_issue_reference: `repository = {raw:?}` is not a \
                 parseable git URL; expected an HTTP(S) URL, an `ssh://` URL, or the \
                 scp-like `git@host:owner/repo.git` form, with an `owner/repo` path",
            )
        })
    })
}

impl BareIssueReference {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            forge: config.forge,
            repo_web_base: resolve_repository(config.repository.as_deref()),
            suggest_issue_url: config.suggest_issue_url,
            suggest_pr_url: config.suggest_pr_url,
            form: config.form,
            include_plain_comments: config.include_plain_comments,
            plain_comment_form: config.plain_comment_form,
        }
    }

    /// The forge whose path layout to use: the explicit `forge` if
    /// set, otherwise one detected from the repository's host. `None`
    /// when neither yields a recognised forge — there is no fallback,
    /// so the rule never prefers one service.
    fn effective_forge(&self) -> Option<Forge> {
        self.forge
            .or_else(|| self.repo_web_base.as_deref().and_then(Forge::detect))
    }

    /// Suggested issue URL for `number`. `None` (so: help-only)
    /// unless `repository` is set — it carries the owner/repo path
    /// the link needs — and a forge is known (explicit or detected
    /// from the host).
    fn issue_url(&self, number: &str) -> Option<String> {
        let base = self.repo_web_base.as_deref()?;
        let forge = self.effective_forge()?;
        Some(join_url(base, forge.issue_path(), number))
    }

    /// Suggested pull-request URL for `number`; see [`Self::issue_url`]
    /// for when it is `None`.
    fn pr_url(&self, number: &str) -> Option<String> {
        let base = self.repo_web_base.as_deref()?;
        let forge = self.effective_forge()?;
        Some(join_url(base, forge.pr_path(), number))
    }

    /// Whether a bare `#NNN` reference can denote a pull request on
    /// the effective forge. False on GitLab, where `#NNN` is always an
    /// issue (merge requests are written `!NNN`); true otherwise.
    fn hash_can_mean_pr(&self) -> bool {
        self.effective_forge() != Some(Forge::GitLab)
    }
}

/// Join a repository base URL with a relative path (the part after
/// the base), substituting `{number}`. Exactly one `/` separates the
/// base from the path regardless of a trailing slash on the base or a
/// leading slash on the path.
fn join_url(base_url: &str, path: &str, number: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let path = path.replace("{number}", number);
    format!("{base}/{}", path.trim_start_matches('/'))
}

/// A recognised git-hosting service. The chosen forge fixes the
/// issue / PR URL layout. It can be given explicitly (needed for a
/// self-hosted instance, whose host isn't recognised) or detected
/// from the repository's host via [`Forge::detect`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Forge {
    /// GitHub or a GitHub Enterprise instance. Paths:
    /// `/issues/{number}`, `/pull/{number}`.
    GitHub,
    /// GitLab (gitlab.com or self-hosted). Paths:
    /// `/-/issues/{number}`, `/-/merge_requests/{number}`.
    GitLab,
    /// Gitea / Forgejo (including Codeberg). Paths:
    /// `/issues/{number}`, `/pulls/{number}`.
    Gitea,
}

impl Forge {
    fn issue_path(self) -> &'static str {
        match self {
            Forge::GitHub | Forge::Gitea => "/issues/{number}",
            Forge::GitLab => "/-/issues/{number}",
        }
    }

    fn pr_path(self) -> &'static str {
        match self {
            Forge::GitHub => "/pull/{number}",
            Forge::Gitea => "/pulls/{number}",
            Forge::GitLab => "/-/merge_requests/{number}",
        }
    }

    /// Detect the forge from a repository URL's host — both the
    /// public instances and the conventional `gitlab.*` / `github.*`
    /// / `gitea.*` / `forgejo.*` self-hosted subdomains. Returns
    /// `None` for a host that gives no such hint, so it needs an
    /// explicit `forge`.
    fn detect(repo_url: &str) -> Option<Forge> {
        let host = host_of(repo_url)?.to_ascii_lowercase();
        // Public instances, matched on the full host.
        match host.as_str() {
            "github.com" => return Some(Forge::GitHub),
            "gitlab.com" => return Some(Forge::GitLab),
            // Forgejo (Codeberg) shares Gitea's URL layout.
            "codeberg.org" | "gitea.com" => return Some(Forge::Gitea),
            _ => {}
        }
        // Self-hosted instances conventionally live under a subdomain
        // named after the software (`gitlab.example.com`,
        // `gitea.example.com`, `github.example.com` for GitHub
        // Enterprise), so the leading DNS label names the forge.
        // `git.*` is the most common generic prefix but ambiguous
        // (Gitea, GitLab, cgit, gitweb), so it isn't matched.
        //
        // Two whole services are deliberately unsupported, because a
        // bare `#NNN` doesn't reliably map to one of their URLs:
        // Gitee (`gitee.com`) numbers issues with alphanumeric tokens
        // rather than the integers `#NNN` assumes, and Bitbucket's
        // `#NNN` convention (issue? PR? neither?) we couldn't pin
        // down, so we don't guess at its links.
        match host.split('.').next()? {
            "gitlab" => Some(Forge::GitLab),
            "github" => Some(Forge::GitHub),
            "gitea" | "forgejo" => Some(Forge::Gitea),
            _ => None,
        }
    }
}

/// Host component of a URL like `https://github.com/owner/repo` →
/// `"github.com"`. `None` if the URL has no `://`.
fn host_of(url: &str) -> Option<&str> {
    let after_scheme = url.find("://")?;
    let rest = &url[after_scheme + 3..];
    let end = rest.find(['/', '?', '#', ':']).unwrap_or(rest.len());
    Some(&rest[..end])
}

impl_lint_pass!(BareIssueReference => [BARE_ISSUE_REFERENCE]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[BARE_ISSUE_REFERENCE]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("bare_issue_reference", DefaultState::Active) {
        return;
    }
    lint_store.register_early_pass(|| Box::new(BareIssueReference::new()));
}

impl EarlyLintPass for BareIssueReference {
    fn check_crate(&mut self, lint_context: &EarlyContext<'_>, _: &Crate) {
        walk_local_comments(lint_context, |chunk| match chunk.surface {
            CommentSurface::DocBlock | CommentSurface::DocBlockBlock => {
                self.scan_doc(lint_context, chunk);
            }
            CommentSurface::PlainLine => {
                if self.include_plain_comments {
                    self.scan_plain(lint_context, chunk);
                }
            }
            CommentSurface::PlainBlock => {
                // Out of scope per the plan — `bare_issue_reference`
                // deliberately doesn't scan plain block comments.
            }
        });
    }
}

impl BareIssueReference {
    fn scan_doc(&self, lint_context: &EarlyContext<'_>, chunk: &CommentChunk<'_>) {
        let skips = scan_skip_regions(&chunk.rendered);
        self.scan(lint_context, chunk, &skips, true);
    }

    fn scan_plain(&self, lint_context: &EarlyContext<'_>, chunk: &CommentChunk<'_>) {
        self.scan(lint_context, chunk, &[], false);
    }

    fn scan(
        &self,
        lint_context: &EarlyContext<'_>,
        chunk: &CommentChunk<'_>,
        skips: &[std::ops::Range<usize>],
        is_doc: bool,
    ) {
        let text = &chunk.rendered;
        let bytes = text.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != b'#' {
                index += utf8_char_len(bytes, index);
                continue;
            }
            // Left-context guard: skip if preceded by a word
            // character (would make the `#NNN` part of a larger
            // identifier), `[` (existing markdown link label), or
            // `` ` `` (markdown code span / plain-comment
            // backtick-quoted code).
            if index > 0 {
                let prev = bytes[index - 1];
                if prev.is_ascii_alphanumeric() || prev == b'_' || prev == b'[' || prev == b'`' {
                    index += 1;
                    continue;
                }
            }
            // Take digits after `#`.
            let digits_start = index + 1;
            let mut end = digits_start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end == digits_start {
                index += 1;
                continue;
            }
            // Right-context: must end at word boundary.
            if end < bytes.len() {
                let next = bytes[end];
                if next.is_ascii_alphanumeric() || next == b'_' {
                    index = end;
                    continue;
                }
            }
            // URL-fragment guard.
            if back_scan_url_fragment(text, index) {
                index = end;
                continue;
            }
            if position_in_skip(skips, index) {
                index = end;
                continue;
            }
            let number = &text[digits_start..end];
            self.emit(lint_context, chunk, index, end - index, number, is_doc);
            index = end;
        }
    }

    fn emit(
        &self,
        lint_context: &EarlyContext<'_>,
        chunk: &CommentChunk<'_>,
        rendered_pos: usize,
        len: usize,
        number: &str,
        is_doc: bool,
    ) {
        let Some(span) = chunk.span_for(rendered_pos, len as u32) else {
            return;
        };
        let token = format!("#{number}");
        let issue_url = self.issue_url(number);
        let pr_url = self.pr_url(number);
        let suggest_issue = self.suggest_issue_url;
        // On GitLab a bare `#NNN` always denotes an issue — merge
        // requests are written `!NNN` — so the PR suggestion never
        // applies to it, whatever `suggest_pr_url` says.
        let suggest_pr = self.suggest_pr_url && self.hash_can_mean_pr();
        let doc_form = self.form;
        let plain_form = self.plain_comment_form;
        span_lint_and_then(
            lint_context,
            BARE_ISSUE_REFERENCE,
            span,
            format!(
                "ambiguous `{token}`; a bare `#NNN` could be an issue or pull request, \
                 a colour, or any other numbered item",
            ),
            move |diag| {
                // Offer the issue / PR link suggestions when a
                // repository is configured and at least one knob is
                // on. These are only ever `MaybeIncorrect`: the token
                // is ambiguous, so the lint can't be sure it even
                // names a reference.
                match issue_url {
                    None => {
                        diag.help(
                            "set `repository` (and `forge`, if its host isn't a recognised \
                             service) in dylint.toml under \
                             `[perfectionist::bare_issue_reference]` to enable issue / PR link \
                             suggestions",
                        );
                    }
                    Some(_) if !(suggest_issue || suggest_pr) => {
                        diag.help(
                            "enable `suggest_issue_url` and/or `suggest_pr_url` in dylint.toml \
                             under `[perfectionist::bare_issue_reference]` to get a link \
                             suggestion",
                        );
                    }
                    Some(issue_url) => {
                        if suggest_issue {
                            emit_one(
                                diag, span, &token, &issue_url, "issue", is_doc, doc_form,
                                plain_form,
                            );
                        }
                        if suggest_pr {
                            let pr_url = pr_url.expect("pr_url renders whenever issue_url does");
                            emit_one(
                                diag,
                                span,
                                &token,
                                &pr_url,
                                "pull request",
                                is_doc,
                                doc_form,
                                plain_form,
                            );
                        }
                    }
                }
                // Two disambiguations that apply whatever the config:
                // mark the token as not-a-reference, or avoid the
                // `#NNN` spelling entirely.
                diag.help(format!(
                    "if `{token}` is not an issue or pull request, enclose it in backticks \
                     so it renders as code",
                ));
                diag.help("or refer to the numbered item with a spelling that has no leading `#`");
            },
        );
    }
}

/// Emit one issue-or-PR link suggestion. `target_label` is `"issue"`
/// or `"pull request"`. Every suggestion is `MaybeIncorrect`: a bare
/// `#NNN` is ambiguous, so the lint can never be confident the token
/// names a reference at all, let alone which kind.
#[expect(
    clippy::too_many_arguments,
    reason = "a small private emit helper; bundling these into a struct would obscure the call"
)]
fn emit_one(
    diag: &mut rustc_errors::Diag<'_, ()>,
    span: Span,
    token: &str,
    url: &str,
    target_label: &str,
    is_doc: bool,
    doc_form: DocForm,
    plain_form: PlainForm,
) {
    if is_doc {
        let message = match doc_form {
            DocForm::Inline => format!("use an inline markdown link to the {target_label}"),
            DocForm::Reference => format!(
                "use a reference-style markdown link to the {target_label} \
                 (define `[#N]: URL` at the end of the doc block)",
            ),
            DocForm::BareUrl => format!("substitute the {target_label} URL"),
            DocForm::BracketedUrl => {
                format!("substitute the {target_label} URL wrapped in `<...>`")
            }
        };
        diag.span_suggestion(
            span,
            message,
            render_doc_suggestion(doc_form, token, url),
            Applicability::MaybeIncorrect,
        );
    } else {
        let message = match plain_form {
            PlainForm::BareUrl => format!("substitute the {target_label} URL"),
            PlainForm::BracketedUrl => {
                format!("substitute the {target_label} URL wrapped in `<...>`")
            }
        };
        diag.span_suggestion(
            span,
            message,
            render_plain_suggestion(plain_form, url),
            Applicability::MaybeIncorrect,
        );
    }
}

fn render_doc_suggestion(form: DocForm, token: &str, url: &str) -> String {
    match form {
        DocForm::Inline => format!("[{token}]({url})"),
        // For the reference form, the suggestion only rewrites the
        // matched span — the matching `[#N]: url` definition is the
        // author's responsibility (the lint can't safely synthesise
        // a multi-line definition without knowing where the block
        // ends). Same shape as rustdoc's collapsed-reference form.
        DocForm::Reference => format!("[{token}]"),
        // The URL forms replace the `#N` token outright; the `token`
        // is intentionally dropped.
        DocForm::BareUrl => url.to_owned(),
        DocForm::BracketedUrl => format!("<{url}>"),
    }
}

fn render_plain_suggestion(form: PlainForm, url: &str) -> String {
    match form {
        PlainForm::BareUrl => url.to_owned(),
        PlainForm::BracketedUrl => format!("<{url}>"),
    }
}

#[cfg(test)]
mod tests {
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
            form: DocForm::Inline,
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
            form: DocForm::Inline,
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
            form: DocForm::Inline,
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
}
