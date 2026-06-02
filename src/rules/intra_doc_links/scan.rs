//! Rust-aware layer over the shared markdown scanner: turn each bare
//! `` `...` `` code span the walker surfaces into an intra-doc-link
//! *candidate* when (and only when) its body is a single Rust
//! identifier.
//!
//! The structural classification — distinguishing a bare code span
//! from one already wrapped as `` [`Foo`] `` — lives in
//! [`crate::markdown::scan_code_span_candidates`]. The
//! identifier-extraction step here is the "Rust-specific extraction
//! layered on top" that the "Markdown parsing" section of
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md` leaves to each
//! consuming rule.

use std::ops::Range;

use crate::markdown::scan_code_span_candidates;

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
/// empty, holds more than one token, or is not a plain identifier.
///
/// Strips the leading and trailing backtick runs, then the one
/// optional padding space CommonMark allows on each side, then checks
/// that what remains is exactly one identifier: a leading
/// `[A-Za-z_]` followed by `[A-Za-z0-9_]*`. A leading-underscore-only
/// token (`_`) is rejected — it is the wildcard, not a nameable item.
fn take_backticked_ident(code_span: &str) -> Option<String> {
    let body = strip_code_fences(code_span)?;
    // CommonMark strips at most one space from each end when both ends
    // have one and the body is not all whitespace; a plain `.trim()`
    // is a superset that suits identifier extraction (an identifier
    // never contains interior whitespace, so over-trimming is safe).
    let body = body.trim();
    if !is_plain_ident(body) {
        return None;
    }
    Some(body.to_owned())
}

/// Strip the matching opening and closing backtick fences from a code
/// span's source text, returning the inner body. Returns `None` if the
/// text is not fence-delimited (defensive — the caller only passes
/// genuine [`take_code_span`](crate::markdown) matches).
fn strip_code_fences(code_span: &str) -> Option<&str> {
    let bytes = code_span.as_bytes();
    let mut fence = 0;
    while fence < bytes.len() && bytes[fence] == b'`' {
        fence += 1;
    }
    if fence == 0 || code_span.len() < fence * 2 {
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
