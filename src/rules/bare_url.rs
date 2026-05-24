//! `perfectionist::bare_url` — flag bare `http(s)://` URLs in doc
//! comments and regular comments. The autofix wraps the URL in
//! `<...>`, with applicability driven by the URL's last character.

use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_and_sugg;
use rustc_ast::Crate;
use rustc_errors::Applicability;
use rustc_lint::{EarlyContext, EarlyLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

use crate::comment_walk::{CommentChunk, CommentSurface, walk_local_comments};
use crate::common::{DefaultState, resolved_state};
use crate::markdown::{position_in_skip, scan_skip_regions, utf8_char_len};
use crate::url_scan::{DEFAULT_FORWARD_SCHEMES, TrailingClass, classify_trailing, take_url};

declare_tool_lint! {
    /// ### What it does
    /// Flags bare `http://` and `https://` URLs in doc comments
    /// (`///`, `//!`) and regular comments (`//`, `/* */`). Wrapping
    /// the URL in `<...>` (or using the labelled `[text](url)` form)
    /// is the portable rendering across CommonMark, GitHub-flavored
    /// markdown, and rustdoc.
    ///
    /// ### Why restrict this?
    /// This is a stylistic preference, not a correctness issue. Bare
    /// URLs rely on the renderer's autolinkification: rustdoc renders
    /// them, GitHub renders them, but plain CommonMark does not. The
    /// `<...>` form is the explicit, portable spelling.
    ///
    /// ### Example
    /// ```rust,ignore
    /// /// See https://example.com for details.
    /// ```
    /// Use instead:
    /// ```rust,ignore
    /// /// See <https://example.com> for details.
    /// ```
    pub perfectionist::BARE_URL,
    Warn,
    "bare URL in comment or doc comment; wrap in `<...>` or use a labelled markdown link",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::bare_url";

/// Default characters that, when a URL ends in one of them, qualify
/// the autofix as machine-applicable.
const DEFAULT_SAFE_TRAILING_CHARS: &[char] = &['/', '_', '-', '=', '&', '+'];

/// Hosts that the rule skips by default — placeholder hosts that
/// frequently appear bare in docs for illustrative purposes.
const DEFAULT_SKIP_HOSTS: &[&str] = &["example.com", "example.org", "localhost"];

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Scan doc comments (`///`, `//!`, `/** */`, `/*! */`).
    /// Defaults to `true`. Set to `false` if a project deliberately
    /// writes bare URLs in doc comments and wants the lint to leave
    /// them alone.
    scan_doc_comments: bool,
    /// Scan regular comments (`//`, `/* */`). Defaults to `true`.
    scan_regular_comments: bool,
    /// Characters that, when the URL ends in one of them, keep the
    /// autofix at `MachineApplicable`. Defaults to `["/", "_", "-",
    /// "=", "&", "+"]`. ASCII alphanumerics and `/` are always
    /// treated as safe regardless of this list; entries here
    /// supplement that built-in set.
    safe_trailing_chars: Vec<char>,
    /// Hosts to skip — placeholder hosts that frequently appear
    /// bare in docs for illustrative purposes. Compared
    /// case-insensitively per RFC 3986 §3.2.2. Defaults to
    /// `["example.com", "example.org", "localhost"]`.
    skip_hosts: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scan_doc_comments: true,
            scan_regular_comments: true,
            safe_trailing_chars: DEFAULT_SAFE_TRAILING_CHARS.to_vec(),
            skip_hosts: DEFAULT_SKIP_HOSTS.iter().map(|s| (*s).to_owned()).collect(),
        }
    }
}

pub struct BareUrl {
    scan_doc_comments: bool,
    scan_regular_comments: bool,
    safe_trailing_chars: Vec<char>,
    skip_hosts: BTreeSet<String>,
}

impl BareUrl {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            scan_doc_comments: config.scan_doc_comments,
            scan_regular_comments: config.scan_regular_comments,
            safe_trailing_chars: config.safe_trailing_chars,
            skip_hosts: config.skip_hosts.into_iter().collect(),
        }
    }

    fn host_is_skipped(&self, url: &str) -> bool {
        // Strip the scheme + `://`.
        let after_scheme = url.find("://").map(|index| index + 3).unwrap_or(0);
        let rest = &url[after_scheme..];
        // The host ends at the first `/`, `?`, `#`, `:` or end of string.
        let host_end = rest.find(['/', '?', '#', ':']).unwrap_or(rest.len());
        let host = &rest[..host_end];
        // RFC 3986 §3.2.2: host comparisons are case-insensitive. The
        // configured `skip_hosts` entries are stored as-is; do the
        // case-fold on the lookup side so users can write the host in
        // any casing.
        self.skip_hosts
            .iter()
            .any(|skip| skip.eq_ignore_ascii_case(host))
    }
}

impl_lint_pass!(BareUrl => [BARE_URL]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[BARE_URL]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("bare_url", DefaultState::Active) {
        return;
    }
    lint_store.register_early_pass(|| Box::new(BareUrl::new()));
}

impl EarlyLintPass for BareUrl {
    fn check_crate(&mut self, lint_context: &EarlyContext<'_>, _: &Crate) {
        if !(self.scan_doc_comments || self.scan_regular_comments) {
            return;
        }
        walk_local_comments(lint_context, |chunk| match chunk.surface {
            CommentSurface::DocBlock | CommentSurface::DocBlockBlock => {
                if self.scan_doc_comments {
                    self.scan_doc_chunk(lint_context, chunk);
                }
            }
            CommentSurface::PlainLine | CommentSurface::PlainBlock => {
                if self.scan_regular_comments {
                    self.scan_plain_chunk(lint_context, chunk);
                }
            }
        });
    }
}

impl BareUrl {
    fn scan_doc_chunk(&self, lint_context: &EarlyContext<'_>, chunk: &CommentChunk<'_>) {
        let skips = scan_skip_regions(&chunk.rendered);
        self.scan(lint_context, chunk, &skips);
    }

    fn scan_plain_chunk(&self, lint_context: &EarlyContext<'_>, chunk: &CommentChunk<'_>) {
        // Plain comments aren't markdown, so no skip-region pass is
        // run; only the left-context guard inside [`Self::scan`]
        // (the `prev_byte` check against `<`, `[`, `(`, `"`, `'`,
        // `` ` ``, and word chars) applies.
        self.scan(lint_context, chunk, &[]);
    }

    fn scan(
        &self,
        lint_context: &EarlyContext<'_>,
        chunk: &CommentChunk<'_>,
        skips: &[std::ops::Range<usize>],
    ) {
        let text = &chunk.rendered;
        let bytes = text.as_bytes();
        let schemes = DEFAULT_FORWARD_SCHEMES;
        let mut index = 0;
        while index < bytes.len() {
            // Look for a scheme start: an ASCII letter at the start
            // of a word boundary.
            let byte = bytes[index];
            if !byte.is_ascii_alphabetic() {
                index += utf8_char_len(bytes, index);
                continue;
            }
            // Left-context guard: skip if the byte immediately before
            // `index` is a word character or one of `<`, `[`, `(`,
            // `"`, `'`, `` ` `` — the last six meaning the URL is
            // already wrapped (markdown autolink / labelled link /
            // inline delimiter / HTML attribute / quoted-prose pair /
            // code span — backticks delimit code-y spans in both
            // markdown doc comments and plain `//` developer prose).
            if index > 0 {
                let prev = bytes[index - 1];
                if prev.is_ascii_alphanumeric() || prev == b'_' {
                    index += 1;
                    continue;
                }
                if prev == b'<'
                    || prev == b'['
                    || prev == b'('
                    || prev == b'"'
                    || prev == b'\''
                    || prev == b'`'
                {
                    // Advance past the URL if it matches, to keep the
                    // scanner forward-progressing.
                    if let Some(url_match) = take_url(&text[index..], schemes) {
                        index += url_match.consumed;
                        continue;
                    }
                    index += 1;
                    continue;
                }
            }
            let Some(url_match) = take_url(&text[index..], schemes) else {
                index += 1;
                continue;
            };
            if position_in_skip(skips, index) {
                index += url_match.consumed;
                continue;
            }
            if self.host_is_skipped(url_match.url) {
                index += url_match.consumed;
                continue;
            }
            self.emit(lint_context, chunk, index, url_match.url);
            index += url_match.consumed;
        }
    }

    fn emit(
        &self,
        lint_context: &EarlyContext<'_>,
        chunk: &CommentChunk<'_>,
        rendered_pos: usize,
        url: &str,
    ) {
        let Some(span) = chunk.span_for(rendered_pos, url.len() as u32) else {
            return;
        };
        let applicability = match classify_trailing(url, &self.safe_trailing_chars) {
            TrailingClass::Safe => Applicability::MachineApplicable,
            TrailingClass::Ambiguous => Applicability::MaybeIncorrect,
        };
        let suggestion = format!("<{url}>");
        emit_diag(lint_context, span, url, suggestion, applicability);
    }
}

fn emit_diag(
    lint_context: &EarlyContext<'_>,
    span: Span,
    url: &str,
    suggestion: String,
    applicability: Applicability,
) {
    span_lint_and_sugg(
        lint_context,
        BARE_URL,
        span,
        format!("bare URL `{url}`; wrap in `<...>` or use a labelled markdown link"),
        "wrap in `<...>` for portable autolink syntax",
        suggestion,
        applicability,
    );
}
