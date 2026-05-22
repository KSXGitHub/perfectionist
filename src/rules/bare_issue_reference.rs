//! `perfectionist::bare_issue_reference` — flag bare `#NNN` issue /
//! PR references in doc comments (and optionally plain `//` line
//! comments), suggesting the markdown-link form.

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_ast::Crate;
use rustc_errors::Applicability;
use rustc_lint::{EarlyContext, EarlyLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};

use crate::comment_walk::{CommentChunk, CommentSurface, walk_local_comments};
use crate::common::{DefaultState, resolved_state};
use crate::markdown::{position_in_skip, scan_skip_regions, utf8_char_len};
use crate::url_scan::back_scan_url_fragment;

declare_tool_lint! {
    /// ### What it does
    /// Flags bare `#NNN` issue / pull-request references in doc
    /// comments (`///`, `//!`) — and, when opted in, in plain `//`
    /// line comments. The autofix substitutes a markdown-link form
    /// (`[#123](URL)` or `[#123]`, plus a matching reference-link
    /// definition).
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
    /// Use instead (with `repo_base_url = "https://github.com/owner/repo"`):
    /// ```rust,ignore
    /// /// Closes [#123](https://github.com/owner/repo/issues/123) and
    /// /// supersedes [#124](https://github.com/owner/repo/issues/124).
    /// ```
    pub perfectionist::BARE_ISSUE_REFERENCE,
    Warn,
    "bare issue / PR reference in comment; use a markdown link",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::bare_issue_reference";

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum SuggestionMode {
    #[default]
    IssueUrl,
    Both,
    HelpOnly,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum DocForm {
    #[default]
    Inline,
    Reference,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum PlainForm {
    #[default]
    Bare,
    Bracketed,
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    repo_base_url: Option<String>,
    issue_url_template: String,
    pr_url_template: String,
    suggestion_mode: SuggestionMode,
    form: DocForm,
    include_plain_comments: bool,
    plain_comment_form: PlainForm,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            repo_base_url: None,
            issue_url_template: "{repo_base_url}/issues/{number}".to_owned(),
            pr_url_template: "{repo_base_url}/pull/{number}".to_owned(),
            suggestion_mode: SuggestionMode::IssueUrl,
            form: DocForm::Inline,
            include_plain_comments: false,
            plain_comment_form: PlainForm::Bare,
        }
    }
}

pub struct BareIssueReference {
    repo_base_url: Option<String>,
    issue_url_template: String,
    pr_url_template: String,
    suggestion_mode: SuggestionMode,
    form: DocForm,
    include_plain_comments: bool,
    plain_comment_form: PlainForm,
    /// Cached lowercased host parsed out of `repo_base_url`, used to
    /// decide whether the `IssueUrl` suggestion is machine-applicable
    /// (true only for `github.com`, which redirects `/issues/<n>` to
    /// `/pull/<n>`).
    repo_host: Option<String>,
}

impl BareIssueReference {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        let repo_host = config
            .repo_base_url
            .as_deref()
            .and_then(parse_host_from_url)
            .map(|h| h.to_ascii_lowercase());
        Self {
            repo_base_url: config.repo_base_url,
            issue_url_template: config.issue_url_template,
            pr_url_template: config.pr_url_template,
            suggestion_mode: config.suggestion_mode,
            form: config.form,
            include_plain_comments: config.include_plain_comments,
            plain_comment_form: config.plain_comment_form,
            repo_host,
        }
    }

    fn render_url(&self, template: &str, number: &str) -> Option<String> {
        let base = self.repo_base_url.as_deref()?;
        Some(
            template
                .replace("{repo_base_url}", base)
                .replace("{number}", number),
        )
    }

    fn is_github(&self) -> bool {
        self.repo_host.as_deref() == Some("github.com")
    }
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
            // Left-context guard.
            if index > 0 {
                let prev = bytes[index - 1];
                if prev.is_ascii_alphanumeric() || prev == b'_' || prev == b'[' {
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
        let mode = self.suggestion_mode;
        let doc_form = self.form;
        let plain_form = self.plain_comment_form;
        let is_github = self.is_github();
        span_lint_and_then(
            lint_context,
            BARE_ISSUE_REFERENCE,
            span,
            format!("bare issue / PR reference `{token}`; use a markdown link"),
            move |diag| {
                if matches!(mode, SuggestionMode::HelpOnly) || issue_url.is_none() {
                    if let Some(url) = &issue_url {
                        diag.help(format!(
                            "candidate URL: {url} — apply the link form manually",
                        ));
                    } else {
                        diag.help(
                            "set `repo_base_url` in dylint.toml under \
                             `[perfectionist::bare_issue_reference]` to enable URL suggestions",
                        );
                    }
                    return;
                }
                let issue_url = issue_url.unwrap();
                if is_doc {
                    let suggestion = render_doc_suggestion(doc_form, &token, &issue_url);
                    // The reference form (`[#123]`) emits an
                    // incomplete suggestion — the matching
                    // `[#123]: URL` definition is the author's
                    // responsibility — so applying it as-is leaves
                    // the doc block with an undefined reference
                    // link (which rustdoc itself warns about via
                    // `rustdoc::broken_intra_doc_links`). Always
                    // degrade to `MaybeIncorrect` for the reference
                    // form so `cargo dylint --fix` doesn't apply
                    // it unprompted.
                    let applicability = match (mode, doc_form) {
                        (SuggestionMode::IssueUrl, DocForm::Inline) if is_github => {
                            Applicability::MachineApplicable
                        }
                        (SuggestionMode::HelpOnly, _) => unreachable!(),
                        _ => Applicability::MaybeIncorrect,
                    };
                    diag.span_suggestion(
                        span,
                        match doc_form {
                            DocForm::Inline => "use an inline markdown link",
                            DocForm::Reference => {
                                "use a reference-style markdown link \
                                 (define `[#N]: URL` at the end of the doc block)"
                            }
                        },
                        suggestion,
                        applicability,
                    );
                    if matches!(mode, SuggestionMode::Both)
                        && let Some(pr_url) = &pr_url
                    {
                        let alt = render_doc_suggestion(doc_form, &token, pr_url);
                        diag.span_suggestion(
                            span,
                            "or treat the number as a PR",
                            alt,
                            Applicability::MaybeIncorrect,
                        );
                    }
                } else {
                    // Plain `//` comment — never markdown.
                    let suggestion = render_plain_suggestion(plain_form, &issue_url);
                    diag.span_suggestion(
                        span,
                        match plain_form {
                            PlainForm::Bare => "substitute with the bare URL",
                            PlainForm::Bracketed => "substitute with `<URL>`",
                        },
                        suggestion,
                        Applicability::MaybeIncorrect,
                    );
                }
            },
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
        PlainForm::Bare => url.to_owned(),
        PlainForm::Bracketed => format!("<{url}>"),
    }
}

/// Extract the host component out of a URL like
/// `https://github.com/owner/repo` — returns `"github.com"`. Returns
/// `None` if the URL doesn't contain `://`.
fn parse_host_from_url(url: &str) -> Option<&str> {
    let after_scheme = url.find("://")?;
    let rest = &url[after_scheme + 3..];
    let host_end = rest.find(['/', '?', '#', ':']).unwrap_or(rest.len());
    Some(&rest[..host_end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_host() {
        assert_eq!(
            parse_host_from_url("https://github.com/owner/repo"),
            Some("github.com"),
        );
    }

    #[test]
    fn parses_host_with_port() {
        assert_eq!(
            parse_host_from_url("https://gitlab.example.org:8080/owner/repo"),
            Some("gitlab.example.org"),
        );
    }

    #[test]
    fn rejects_url_without_scheme() {
        assert_eq!(parse_host_from_url("github.com/owner/repo"), None);
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
            render_plain_suggestion(PlainForm::Bracketed, "https://example.com/issues/42"),
            "<https://example.com/issues/42>",
        );
    }
}
