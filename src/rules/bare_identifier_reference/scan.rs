//! Rust-aware layer over the shared markdown scanner: turn each bare
//! `` `...` `` code span the walker surfaces into an intra-doc-link
//! *candidate* when (and only when) its body is a single Rust
//! identifier.
//!
//! The structural classification — distinguishing a bare code span
//! from one already wrapped as `` [`Foo`] `` — lives in
//! [`crate::markdown::scan_code_span_candidates`]. The
//! identifier-extraction step here is the Rust-specific layer that
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md` leaves to each
//! consuming rule.

use crate::markdown::scan_code_span_candidates;
use core::ops::Range;

/// One intra-doc-link candidate found in a doc-comment chunk: a bare
/// code span whose body is a single Rust identifier.
pub(super) struct Candidate {
    /// Byte range in the rendered chunk text covering the whole code
    /// span, backtick fences included (`` `Foo` ``). The autofix wraps
    /// exactly this range in `[` / `]`.
    pub(super) span: Range<usize>,
    /// The extracted identifier (the code-span body, with fences and
    /// the optional CommonMark padding spaces stripped).
    pub(super) ident: String,
}

/// Collect every intra-doc-link candidate in `rendered`.
pub(super) fn collect_candidates(rendered: &str) -> Vec<Candidate> {
    scan_code_span_candidates(rendered)
        .into_iter()
        .filter_map(|span| {
            let ident = take_backticked_ident(&rendered[span.clone()])?;
            Some(Candidate { span, ident })
        })
        .collect()
}

/// Pull a single Rust identifier out of a code span's source text
/// (`` `Foo` ``, `` `` Foo `` ``, ...). Returns `None` when the body is
/// empty, holds more than one token, spans a line break, or is not a
/// plain identifier.
///
/// A code span that wraps a soft line break is rejected: it cannot be
/// rewritten as an inline `[...]` link, and its source span would not
/// map back to one contiguous range, so the autofix could corrupt the
/// source.
fn take_backticked_ident(code_span: &str) -> Option<String> {
    if code_span.contains('\n') {
        return None;
    }
    let body = strip_code_fences(code_span)?;
    // CommonMark strips at most one space from each end, and only when
    // both ends have one and the body is not all whitespace. Mirror that
    // exactly rather than `trim`-ing every run: a padded body like
    // `` `  Foo  ` `` keeps a space after stripping (` Foo `), so it is
    // not a bare identifier — and rewriting it as `` [`  Foo  `] `` would
    // produce a link rustdoc cannot resolve.
    let body = strip_one_padding_space(body);
    if !is_plain_ident(body) {
        return None;
    }
    Some(body.to_owned())
}

/// Strip the single CommonMark padding space from each end of a code
/// span's body — but only when both ends carry one and the body is not
/// all whitespace. Leaves the body untouched otherwise.
fn strip_one_padding_space(body: &str) -> &str {
    if body.len() >= 2 && body.starts_with(' ') && body.ends_with(' ') && !body.trim().is_empty() {
        &body[1..body.len() - 1]
    } else {
        body
    }
}

/// Strip the matching opening and closing backtick fences from a code
/// span's source text, returning the inner body. Returns `None` if the
/// text is not fence-delimited (defensive — the caller only passes
/// genuine code-span matches from [`scan_code_span_candidates`], whose
/// opening and closing fences are equal-length by construction).
fn strip_code_fences(code_span: &str) -> Option<&str> {
    let bytes = code_span.as_bytes();
    let mut fence = 0;
    while fence < bytes.len() && bytes[fence] == b'`' {
        fence += 1;
    }
    if fence == 0 || code_span.len() < fence * 2 {
        return None;
    }
    // Defend against an unequal closing fence (unreachable through the
    // real caller): the trailing `fence` bytes must all be backticks.
    if bytes[code_span.len() - fence..]
        .iter()
        .any(|byte| *byte != b'`')
    {
        return None;
    }
    Some(&code_span[fence..code_span.len() - fence])
}

/// Whether `text` is exactly one plain (ASCII) Rust identifier.
fn is_plain_ident(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    if !chars
        .clone()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return false;
    }
    // Reject the bare wildcard `_` (and runs of underscores), which name
    // nothing linkable.
    text.bytes().any(|byte| byte != b'_')
}

#[cfg(test)]
mod tests;
