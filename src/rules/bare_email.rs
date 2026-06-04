use crate::comment_walk::{CommentChunk, CommentSurface, walk_local_comments};
use crate::common::{DefaultState, resolved_state};
use crate::enclosing_hir::emit_at_enclosing_hir;
use crate::markdown::{position_in_skip, scan_skip_regions, utf8_char_len};
use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_errors::Applicability;
use rustc_hir::HirId;
use rustc_lint::{LateContext, LateLintPass, LintStore};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::Span;
use std::collections::BTreeSet;

declare_tool_lint! {
    /// ### What it does
    ///
    /// Flags bare email addresses (`user@example.com`) in doc
    /// comments (`///`, `//!`) and regular comments (`//`, `/* */`).
    /// Wrapping in `<...>`, prefixing with `mailto:`, or both turns
    /// the address into an explicit autolink across CommonMark,
    /// GitHub-flavored markdown, and rustdoc.
    ///
    /// A `forbid` style is available for projects that prefer to
    /// keep contact information out of source entirely.
    ///
    /// ### Why restrict this?
    ///
    /// This is a stylistic preference, not a correctness issue. Bare
    /// email addresses rely on the renderer's autolinkification,
    /// which is inconsistent across markdown engines. The
    /// `<email>` / `mailto:email` forms make the autolink intent
    /// explicit.
    ///
    /// ### Example
    ///
    /// **Avoid:**
    ///
    /// ```rust,ignore
    /// /// Report security issues to security@example.com.
    /// ```
    ///
    /// **Prefer:**
    ///
    /// ```rust,ignore
    /// /// Report security issues to <security@example.com>.
    /// ```
    pub perfectionist::BARE_EMAIL,
    Warn,
    "bare email address in comment or doc comment; wrap in `<...>` or prefix with `mailto:`",
    report_in_external_macro: false
}

const CONFIG_KEY: &str = "perfectionist::bare_email";

/// Required form for compliant email addresses.
#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Style {
    /// Wrap the address with `<` and `>` — `<user@example.com>`.
    AngleBrackets,
    /// Prefix the address with `mailto:` — `mailto:user@example.com`.
    Mailto,
    /// Combine both — `<mailto:user@example.com>`.
    Both,
    /// Accept any of the three wrapped forms (`<email>`,
    /// `mailto:email`, or `<mailto:email>`); the autofix emits two
    /// `MaybeIncorrect` suggestions for the author to pick from.
    Either,
    /// Forbid email addresses outright — no autofix, just a help
    /// note recommending the address be moved to an external file
    /// or removed entirely.
    Forbid,
}

/// Domains skipped by default — none.
const DEFAULT_SKIP_DOMAINS: &[&str] = &[];

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct Config {
    /// Required form for compliant email addresses. Defaults to
    /// `either`.
    style: Style,
    /// Scan doc comments (`///`, `//!`, `/** */`, `/*! */`).
    /// Defaults to `true`.
    scan_doc_comments: bool,
    /// Scan regular comments (`//`, `/* */`). Defaults to `true`.
    scan_regular_comments: bool,
    /// Skip these exact addresses. Useful for `noreply@github.com`
    /// and similar placeholders that the project deliberately leaves
    /// bare in changelog entries. Empty by default.
    skip_addresses: Vec<String>,
    /// Skip addresses whose domain exactly equals any of these.
    /// Empty by default.
    skip_domains: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            style: Style::Either,
            scan_doc_comments: true,
            scan_regular_comments: true,
            skip_addresses: Vec::new(),
            skip_domains: DEFAULT_SKIP_DOMAINS
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
        }
    }
}

pub struct BareEmail {
    style: Style,
    scan_doc_comments: bool,
    scan_regular_comments: bool,
    skip_addresses: BTreeSet<String>,
    skip_domains: BTreeSet<String>,
}

impl BareEmail {
    fn new() -> Self {
        let config: Config = dylint_linting::config_or_default(CONFIG_KEY);
        Self {
            style: config.style,
            scan_doc_comments: config.scan_doc_comments,
            scan_regular_comments: config.scan_regular_comments,
            skip_addresses: config.skip_addresses.into_iter().collect(),
            skip_domains: config.skip_domains.into_iter().collect(),
        }
    }
}

impl_lint_pass!(BareEmail => [BARE_EMAIL]);

pub fn register_lint(lint_store: &mut LintStore) {
    lint_store.register_lints(&[BARE_EMAIL]);
}

pub fn register_pass(lint_store: &mut LintStore) {
    if let DefaultState::Inactive = resolved_state("bare_email", DefaultState::Active) {
        return;
    }
    lint_store.register_late_pass(|_| Box::new(BareEmail::new()));
}

impl<'tcx> LateLintPass<'tcx> for BareEmail {
    fn check_crate_post(&mut self, lint_context: &LateContext<'tcx>) {
        if !(self.scan_doc_comments || self.scan_regular_comments) {
            return;
        }
        // Each parked finding is the bare address text; the required
        // form (`self.style`) is uniform across the crate, so it's read
        // once at emission time rather than stored per violation.
        let mut violations: Vec<(Span, String)> = Vec::new();
        walk_local_comments(lint_context, |chunk| {
            let is_doc = matches!(
                chunk.surface,
                CommentSurface::DocBlock | CommentSurface::DocBlockBlock,
            );
            if is_doc && !self.scan_doc_comments {
                return;
            }
            if !is_doc && !self.scan_regular_comments {
                return;
            }
            let skips = if is_doc {
                scan_skip_regions(&chunk.rendered)
            } else {
                Vec::new()
            };
            self.scan(chunk, &skips, &mut violations);
        });
        let style = self.style;
        emit_at_enclosing_hir(lint_context.tcx, violations, |hir_id, span, address| {
            emit_email(lint_context, hir_id, span, style, address);
        });
    }
}

impl BareEmail {
    fn scan(
        &self,
        chunk: &CommentChunk<'_>,
        skips: &[std::ops::Range<usize>],
        out: &mut Vec<(Span, String)>,
    ) {
        let text = &chunk.rendered;
        let bytes = text.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            // Only consider candidates whose first byte could start
            // an email local-part. This both speeds up the scan and
            // keeps `index` safely on a UTF-8 char boundary when we
            // advance by `local_len`.
            let head = bytes[index];
            let head_eligible = head.is_ascii_alphanumeric()
                || head == b'.'
                || head == b'_'
                || head == b'%'
                || head == b'+'
                || head == b'-';
            if !head_eligible {
                index += utf8_char_len(bytes, index);
                continue;
            }
            let (local_len, address_len) = match take_email(&text[index..]) {
                Some(m) => m,
                None => {
                    index += 1;
                    continue;
                }
            };
            // Left-context guard: skip if preceded by a word
            // character (would make the match part of a larger
            // identifier), `<` (autolink), `:` (`mailto:` prefix),
            // `/` (URL path containing the `@` in user-info),
            // `.` (would join the local-part to preceding text),
            // or `` ` `` (markdown code span or plain-comment
            // backtick-quoted code).
            if index > 0 {
                let prev = bytes[index - 1];
                if prev.is_ascii_alphanumeric()
                    || prev == b'_'
                    || prev == b'.'
                    || prev == b'<'
                    || prev == b':'
                    || prev == b'/'
                    || prev == b'`'
                {
                    index += local_len.max(1);
                    continue;
                }
            }
            if position_in_skip(skips, index) {
                index += address_len;
                continue;
            }
            let address = &text[index..index + address_len];
            let local_end = index + local_len;
            let domain = &text[local_end + 1..index + address_len];
            if self.skip_addresses.contains(address) {
                index += address_len;
                continue;
            }
            // Domains are case-insensitive (DNS / RFC 1035 §2.3.3),
            // so case-fold the comparison like `bare_url`'s skip_hosts.
            if self
                .skip_domains
                .iter()
                .any(|skip| skip.eq_ignore_ascii_case(domain))
            {
                index += address_len;
                continue;
            }
            let Some(span) = chunk.span_for(index, address_len as u32) else {
                index += address_len;
                continue;
            };
            out.push((span, address.to_owned()));
            index += address_len;
        }
    }
}

/// Emit one bare-email finding at its enclosing HIR node, with the
/// suggestion(s) the configured `style` calls for.
fn emit_email(
    lint_context: &LateContext<'_>,
    hir_id: HirId,
    span: Span,
    style: Style,
    address: String,
) {
    span_lint_hir_and_then(
        lint_context,
        BARE_EMAIL,
        hir_id,
        span,
        format!("bare email address `{address}`"),
        move |diag| match style {
            Style::AngleBrackets => {
                diag.span_suggestion(
                    span,
                    "wrap in `<...>`",
                    format!("<{address}>"),
                    Applicability::MachineApplicable,
                );
            }
            Style::Mailto => {
                diag.span_suggestion(
                    span,
                    "prefix with `mailto:`",
                    format!("mailto:{address}"),
                    Applicability::MachineApplicable,
                );
            }
            Style::Both => {
                diag.span_suggestion(
                    span,
                    "wrap in `<mailto:...>`",
                    format!("<mailto:{address}>"),
                    Applicability::MachineApplicable,
                );
            }
            Style::Either => {
                diag.span_suggestion(
                    span,
                    "wrap in `<...>`",
                    format!("<{address}>"),
                    Applicability::MaybeIncorrect,
                );
                diag.span_suggestion(
                    span,
                    "or prefix with `mailto:`",
                    format!("mailto:{address}"),
                    Applicability::MaybeIncorrect,
                );
            }
            Style::Forbid => {
                diag.help(
                    "move the email out of source — e.g. into a \
                         CONTRIBUTING.md or SECURITY.md — or remove it \
                         entirely",
                );
            }
        },
    );
}

/// Take an email address from the start of `input`. Returns
/// `(local_part_byte_len, total_byte_len)` on success, or `None` if
/// the start of `input` doesn't match the
/// `local@domain.<2+ ASCII letters>` shape.
fn take_email(input: &str) -> Option<(usize, usize)> {
    let local_len = take_local_part(input);
    if local_len == 0 {
        return None;
    }
    let bytes = input.as_bytes();
    if bytes.get(local_len) != Some(&b'@') {
        return None;
    }
    let domain_start = local_len + 1;
    let domain_len = take_domain(&input[domain_start..])?;
    if domain_len == 0 {
        return None;
    }
    Some((local_len, domain_start + domain_len))
}

/// Consume the local-part: one or more ASCII letters, ASCII digits,
/// or any of `.`, `_`, `%`, `+`, `-`.
fn take_local_part(input: &str) -> usize {
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_alphanumeric()
            || byte == b'.'
            || byte == b'_'
            || byte == b'%'
            || byte == b'+'
            || byte == b'-'
        {
            index += 1;
        } else {
            break;
        }
    }
    index
}

/// Consume the domain: one or more labels of ASCII letters /
/// digits / `-`, separated by `.`, ending in a TLD of two or more
/// ASCII letters. The TLD must end at a word boundary.
///
/// A `.` is only consumed when followed by an alphanumeric byte —
/// otherwise the dot is sentence punctuation and the parse stops
/// before it. This is what lets `security@example.net.` (with a
/// trailing sentence period) parse cleanly.
fn take_domain(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut index = 0;
    let mut last_dot: Option<usize> = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_alphanumeric() || byte == b'-' {
            index += 1;
        } else if byte == b'.' {
            // Only treat `.` as a domain separator when it is
            // followed by an alphanumeric (the start of the next
            // label). A `.` at end-of-input or before whitespace /
            // punctuation belongs to the surrounding sentence.
            let next = bytes.get(index + 1).copied();
            match next {
                Some(c) if c.is_ascii_alphanumeric() => {
                    last_dot = Some(index);
                    index += 1;
                }
                _ => break,
            }
        } else {
            break;
        }
    }
    let dot = last_dot?;
    if dot + 1 >= index {
        return None;
    }
    let tld = &input[dot + 1..index];
    let tld_bytes = tld.as_bytes();
    if tld_bytes.len() < 2 {
        return None;
    }
    if !tld_bytes.iter().all(|byte| byte.is_ascii_alphabetic()) {
        return None;
    }
    if index < bytes.len() {
        let next = bytes[index];
        if next.is_ascii_alphanumeric() || next == b'_' {
            return None;
        }
    }
    Some(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_basic_email() {
        let (local, total) = take_email("user@example.com rest").unwrap();
        assert_eq!(local, 4);
        assert_eq!(&"user@example.com rest"[..total], "user@example.com");
    }

    #[test]
    fn takes_email_with_dots_in_local() {
        let (_local, total) = take_email("first.last@example.co.uk").unwrap();
        assert_eq!(total, "first.last@example.co.uk".len());
    }

    #[test]
    fn rejects_missing_tld() {
        assert!(take_email("user@example").is_none());
    }

    #[test]
    fn rejects_short_tld() {
        assert!(take_email("user@example.c").is_none());
    }

    #[test]
    fn rejects_numeric_tld() {
        assert!(take_email("user@example.42").is_none());
    }

    #[test]
    fn rejects_word_continuation_after_tld() {
        assert!(take_email("user@example.com5").is_none());
    }
}
