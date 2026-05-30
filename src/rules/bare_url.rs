use std::collections::BTreeSet;

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;

use crate::comment_walk::{CommentChunk, CommentSurface, walk_local_comments};
use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::emit_at_enclosing_hir;
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
    /// **Bad:**
    /// ```rust,ignore
    /// /// See https://example.com for details.
    /// ```
    /// **Good:**
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

/// Hosts skipped by default — only `localhost`, which points
/// nowhere public and so never wants wrapping in docs.
const DEFAULT_SKIP_HOSTS: &[&str] = &["localhost"];

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Scan doc comments (`///`, `//!`, `/** */`, `/*! */`).
    /// Defaults to `true`.
    scan_doc_comments: bool,
    /// Scan regular comments (`//`, `/* */`). Defaults to `true`.
    scan_regular_comments: bool,
    /// Characters that, when the URL ends in one of them, keep the
    /// autofix at `MachineApplicable`. Defaults to `["/", "_", "-",
    /// "=", "&", "+"]`. ASCII alphanumerics and `/` are always
    /// treated as safe regardless of this list; entries here
    /// supplement that built-in set.
    safe_trailing_chars: Vec<char>,
    /// Hosts to skip, compared case-insensitively. Defaults to
    /// `["localhost"]`.
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
    lint_store.register_late_pass(|_| Box::new(BareUrl::new()));
}

/// One bare-URL finding, parked during the comment walk and emitted
/// later at its enclosing HIR node.
struct UrlViolation {
    url: String,
    applicability: Applicability,
}

impl<'tcx> LateLintPass<'tcx> for BareUrl {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        if !(self.scan_doc_comments || self.scan_regular_comments) {
            return;
        }
        let mut violations: Vec<(Span, UrlViolation)> = Vec::new();
        walk_local_comments(lint_context, |chunk| match chunk.surface {
            CommentSurface::DocBlock | CommentSurface::DocBlockBlock => {
                if self.scan_doc_comments {
                    self.scan_doc_chunk(chunk, &mut violations);
                }
            }
            CommentSurface::PlainLine | CommentSurface::PlainBlock => {
                if self.scan_regular_comments {
                    self.scan_plain_chunk(chunk, &mut violations);
                }
            }
        });
        emit_at_enclosing_hir(lint_context.tcx, violations, |hir_id, span, violation| {
            emit_diag(lint_context, hir_id, span, &violation);
        });
    }
}

impl BareUrl {
    fn scan_doc_chunk(&self, chunk: &CommentChunk<'_>, out: &mut Vec<(Span, UrlViolation)>) {
        let skips = scan_skip_regions(&chunk.rendered);
        self.scan(chunk, &skips, out);
    }

    fn scan_plain_chunk(&self, chunk: &CommentChunk<'_>, out: &mut Vec<(Span, UrlViolation)>) {
        // Plain comments aren't markdown, so no skip-region pass is
        // run; only the left-context guard inside [`Self::scan`]
        // (the `prev_byte` check against `<`, `[`, `(`, `"`, `'`,
        // `` ` ``, and word chars) applies.
        self.scan(chunk, &[], out);
    }

    fn scan(
        &self,
        chunk: &CommentChunk<'_>,
        skips: &[std::ops::Range<usize>],
        out: &mut Vec<(Span, UrlViolation)>,
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
            self.collect(chunk, index, url_match.url, out);
            index += url_match.consumed;
        }
    }

    fn collect(
        &self,
        chunk: &CommentChunk<'_>,
        rendered_pos: usize,
        url: &str,
        out: &mut Vec<(Span, UrlViolation)>,
    ) {
        let Some(span) = chunk.span_for(rendered_pos, url.len() as u32) else {
            return;
        };
        let applicability = match classify_trailing(url, &self.safe_trailing_chars) {
            TrailingClass::Safe => Applicability::MachineApplicable,
            TrailingClass::Ambiguous => Applicability::MaybeIncorrect,
        };
        out.push((
            span,
            UrlViolation {
                url: url.to_owned(),
                applicability,
            },
        ));
    }
}

fn emit_diag(
    lint_context: &LateContext<'_>,
    hir_id: rustc_hir::HirId,
    span: Span,
    violation: &UrlViolation,
) {
    let UrlViolation { url, applicability } = violation;
    span_lint_hir_and_then(
        lint_context,
        BARE_URL,
        hir_id,
        span,
        format!("bare URL `{url}`; wrap in `<...>` or use a labelled markdown link"),
        |diag| {
            diag.span_suggestion(
                span,
                "wrap in `<...>` for portable autolink syntax",
                format!("<{url}>"),
                *applicability,
            );
        },
    );
}
