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

declare_tool_lint! {
    /// ### What it does
    /// Flags bare `#NNN` issue / pull-request references in doc
    /// comments (`///`, `//!`) — and, when opted in, in plain `//`
    /// line comments. The autofix substitutes a markdown-link form
    /// (`[#123](URL)` inline, or the `[#123]` reference form).
    ///
    /// A bare `#NNN` is ambiguous between an issue and a pull
    /// request, so the two `suggest_issue_url` / `suggest_pr_url`
    /// knobs choose which target(s) the autofix offers: exactly one
    /// enabled gives a single `MachineApplicable` suggestion; both
    /// enabled give two `MaybeIncorrect` suggestions for the author
    /// to choose between; neither gives help-only output. (The
    /// `reference` doc form is always `MaybeIncorrect` regardless,
    /// since its `[#N]` output needs a hand-written definition.)
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
    /// Use instead (with `repo_base_url = "https://github.com/owner/repo"`),
    /// picking the issue link for one and the pull-request link for
    /// the other:
    /// ```rust,ignore
    /// /// Closes [#123](https://github.com/owner/repo/issues/123) and
    /// /// supersedes [#124](https://github.com/owner/repo/pull/124).
    /// ```
    pub perfectionist::BARE_ISSUE_REFERENCE,
    Warn,
    "bare issue / PR reference in comment; use a markdown link",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::bare_issue_reference";

/// Markdown-link shape produced by the autofix inside doc comments.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum DocForm {
    /// `[#123](URL)` — the URL is inlined.
    #[default]
    Inline,
    /// `[#123]` — the matching `[#123]: URL` reference-link
    /// definition is the author's responsibility (the lint can't
    /// safely synthesise a multi-line definition without knowing
    /// where the doc block ends). Applicability is always
    /// `MaybeIncorrect` for this form so `cargo dylint --fix`
    /// doesn't apply an incomplete suggestion unprompted.
    Reference,
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
    /// pick `angle_brackets` to produce a form both rules accept.
    #[default]
    Url,
    /// Substitute `<https://...>`. The angle-bracket delimiter gives
    /// the URL a clear boundary when it abuts surrounding
    /// punctuation; editors that auto-link URLs typically recognise
    /// it, and `bare_url` accepts it. Named to match
    /// `bare_email`'s `Style::AngleBrackets`.
    AngleBrackets,
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Repository base URL the issue / PR paths are appended to —
    /// e.g. `"https://github.com/owner/repo"`. Required for any
    /// suggestion; when unset, the lint degrades to help-only
    /// output so it stays adoptable with zero configuration.
    /// Defaults to `None`.
    repo_base_url: Option<String>,
    /// Path appended to `repo_base_url` to form the suggested issue
    /// URL; `{number}` is substituted and the base is joined on
    /// automatically. When unset, the default is inferred from
    /// `repo_base_url`'s host: `/issues/{number}` for GitHub,
    /// Gitea, and Bitbucket; `/-/issues/{number}` for GitLab. A
    /// self-hosted instance whose host isn't recognised falls back
    /// to the GitHub layout — set this explicitly to override.
    issue_url_template: Option<String>,
    /// Path appended to `repo_base_url` to form the suggested
    /// pull-request URL (used when `suggest_pr_url` is enabled);
    /// `{number}` is substituted. When unset, the default is
    /// inferred from `repo_base_url`'s host: `/pull/{number}` for
    /// GitHub, `/pulls/{number}` for Gitea,
    /// `/pull-requests/{number}` for Bitbucket, and
    /// `/-/merge_requests/{number}` for GitLab.
    pr_url_template: Option<String>,
    /// Offer a suggestion that links the reference as an *issue*
    /// (via `issue_url_template`). Defaults to `true`.
    suggest_issue_url: bool,
    /// Offer a suggestion that links the reference as a *pull
    /// request* (via `pr_url_template`). Defaults to `true`.
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
    /// `include_plain_comments = true`. Defaults to `url`. Ignored
    /// for doc comments and when `repo_base_url` is unset.
    plain_comment_form: PlainForm,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            repo_base_url: None,
            issue_url_template: None,
            pr_url_template: None,
            suggest_issue_url: true,
            suggest_pr_url: true,
            form: DocForm::Inline,
            include_plain_comments: false,
            plain_comment_form: PlainForm::Url,
        }
    }
}

pub struct BareIssueReference {
    repo_base_url: Option<String>,
    issue_url_template: String,
    pr_url_template: String,
    suggest_issue_url: bool,
    suggest_pr_url: bool,
    form: DocForm,
    include_plain_comments: bool,
    plain_comment_form: PlainForm,
}

impl BareIssueReference {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        // The issue / PR path defaults adapt to the forge inferred
        // from `repo_base_url`'s host; an explicit template wins.
        // When `repo_base_url` is unset no URL is ever built, so the
        // `GitHub` fallback here is inconsequential.
        let forge = config
            .repo_base_url
            .as_deref()
            .map(Forge::detect)
            .unwrap_or(Forge::GitHub);
        let issue_url_template = config
            .issue_url_template
            .unwrap_or_else(|| forge.issue_path().to_owned());
        let pr_url_template = config
            .pr_url_template
            .unwrap_or_else(|| forge.pr_path().to_owned());
        Self {
            repo_base_url: config.repo_base_url,
            issue_url_template,
            pr_url_template,
            suggest_issue_url: config.suggest_issue_url,
            suggest_pr_url: config.suggest_pr_url,
            form: config.form,
            include_plain_comments: config.include_plain_comments,
            plain_comment_form: config.plain_comment_form,
        }
    }

    fn render_url(&self, template: &str, number: &str) -> Option<String> {
        let base = self.repo_base_url.as_deref()?;
        Some(join_url(base, template, number))
    }
}

/// Join `repo_base_url` with a relative path `template` (the part
/// after the base), substituting `{number}`. Exactly one `/`
/// separates the base from the path regardless of a trailing slash
/// on the base or a leading slash on the template, so both
/// `/issues/{number}` and `issues/{number}` produce the same URL.
fn join_url(repo_base_url: &str, template: &str, number: &str) -> String {
    let base = repo_base_url.trim_end_matches('/');
    let path = template.replace("{number}", number);
    format!("{base}/{}", path.trim_start_matches('/'))
}

/// Forge family inferred from `repo_base_url`'s host. Used only to
/// pick the default issue / PR path templates when the user hasn't
/// set them explicitly. Detection is by exact public-SaaS hostname;
/// a self-hosted instance (any other host) falls back to `GitHub`,
/// the most common layout — such a project sets `issue_url_template`
/// / `pr_url_template` explicitly if its forge differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Forge {
    GitHub,
    GitLab,
    Gitea,
    Bitbucket,
}

impl Forge {
    fn detect(repo_base_url: &str) -> Forge {
        match host_of(repo_base_url)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("gitlab.com") => Forge::GitLab,
            Some("bitbucket.org") => Forge::Bitbucket,
            // Forgejo (Codeberg) shares Gitea's URL layout.
            Some("codeberg.org" | "gitea.com") => Forge::Gitea,
            // github.com, plus every unrecognised / self-hosted host.
            _ => Forge::GitHub,
        }
    }

    fn issue_path(self) -> &'static str {
        match self {
            Forge::GitHub | Forge::Gitea | Forge::Bitbucket => "/issues/{number}",
            Forge::GitLab => "/-/issues/{number}",
        }
    }

    fn pr_path(self) -> &'static str {
        match self {
            Forge::GitHub => "/pull/{number}",
            Forge::Gitea => "/pulls/{number}",
            Forge::Bitbucket => "/pull-requests/{number}",
            Forge::GitLab => "/-/merge_requests/{number}",
        }
    }
}

/// Host component of a URL like `https://github.com/owner/repo` →
/// `"github.com"`. Returns `None` if the URL has no `://`.
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
        let issue_url = self.render_url(&self.issue_url_template, number);
        let pr_url = self.render_url(&self.pr_url_template, number);
        let suggest_issue = self.suggest_issue_url;
        let suggest_pr = self.suggest_pr_url;
        let doc_form = self.form;
        let plain_form = self.plain_comment_form;
        span_lint_and_then(
            lint_context,
            BARE_ISSUE_REFERENCE,
            span,
            format!("bare issue / PR reference `{token}`; use a markdown link"),
            move |diag| {
                // Help-only when no URL can be built or when the
                // author has turned both suggestion knobs off.
                if issue_url.is_none() {
                    diag.help(
                        "set `repo_base_url` in dylint.toml under \
                         `[perfectionist::bare_issue_reference]` to enable URL suggestions",
                    );
                    return;
                }
                if !(suggest_issue || suggest_pr) {
                    diag.help(
                        "enable `suggest_issue_url` and/or `suggest_pr_url` in dylint.toml \
                         under `[perfectionist::bare_issue_reference]` to get a fix suggestion",
                    );
                    return;
                }
                // Exactly one knob set → the author has told the lint
                // which kind the number names, so the single
                // suggestion is machine-applicable. Both set → the
                // number is ambiguous, so each suggestion is only
                // `MaybeIncorrect`.
                let base_applicability = if suggest_issue != suggest_pr {
                    Applicability::MachineApplicable
                } else {
                    Applicability::MaybeIncorrect
                };
                let issue_url = issue_url.unwrap();
                let pr_url = pr_url.expect("pr_url renders whenever issue_url does");
                if suggest_issue {
                    emit_one(
                        diag,
                        span,
                        &token,
                        &issue_url,
                        "issue",
                        is_doc,
                        doc_form,
                        plain_form,
                        base_applicability,
                    );
                }
                if suggest_pr {
                    emit_one(
                        diag,
                        span,
                        &token,
                        &pr_url,
                        "pull request",
                        is_doc,
                        doc_form,
                        plain_form,
                        base_applicability,
                    );
                }
            },
        );
    }
}

/// Emit one issue-or-PR suggestion. `target_label` is `"issue"` or
/// `"pull request"`. The `reference` doc form is forced to
/// `MaybeIncorrect` regardless of `base_applicability`, because its
/// `[#N]` output needs a hand-written `[#N]: URL` definition the
/// lint can't synthesise.
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
    base_applicability: Applicability,
) {
    if is_doc {
        let applicability = match doc_form {
            DocForm::Reference => Applicability::MaybeIncorrect,
            DocForm::Inline => base_applicability,
        };
        let message = match doc_form {
            DocForm::Inline => format!("use an inline markdown link to the {target_label}"),
            DocForm::Reference => format!(
                "use a reference-style markdown link to the {target_label} \
                 (define `[#N]: URL` at the end of the doc block)",
            ),
        };
        diag.span_suggestion(
            span,
            message,
            render_doc_suggestion(doc_form, token, url),
            applicability,
        );
    } else {
        let message = match plain_form {
            PlainForm::Url => format!("substitute the {target_label} URL"),
            PlainForm::AngleBrackets => {
                format!("substitute the {target_label} URL wrapped in `<...>`")
            }
        };
        diag.span_suggestion(
            span,
            message,
            render_plain_suggestion(plain_form, url),
            base_applicability,
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
    }
}

fn render_plain_suggestion(form: PlainForm, url: &str) -> String {
    match form {
        PlainForm::Url => url.to_owned(),
        PlainForm::AngleBrackets => format!("<{url}>"),
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
    fn forge_detects_known_public_hosts() {
        assert_eq!(Forge::detect("https://github.com/o/r"), Forge::GitHub);
        assert_eq!(Forge::detect("https://gitlab.com/o/r"), Forge::GitLab);
        assert_eq!(Forge::detect("https://bitbucket.org/o/r"), Forge::Bitbucket);
        assert_eq!(Forge::detect("https://codeberg.org/o/r"), Forge::Gitea);
    }

    #[test]
    fn forge_detection_is_host_case_insensitive() {
        assert_eq!(Forge::detect("https://GitLab.com/o/r"), Forge::GitLab);
    }

    #[test]
    fn forge_unknown_host_falls_back_to_github() {
        assert_eq!(Forge::detect("https://git.example.com/o/r"), Forge::GitHub);
    }

    #[test]
    fn forge_paths_are_layout_specific() {
        assert_eq!(Forge::GitLab.issue_path(), "/-/issues/{number}");
        assert_eq!(Forge::GitLab.pr_path(), "/-/merge_requests/{number}");
        assert_eq!(Forge::Gitea.pr_path(), "/pulls/{number}");
        assert_eq!(Forge::Bitbucket.pr_path(), "/pull-requests/{number}");
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
    fn renders_plain_suggestion_bracketed() {
        assert_eq!(
            render_plain_suggestion(PlainForm::AngleBrackets, "https://example.com/issues/42"),
            "<https://example.com/issues/42>",
        );
    }
}
