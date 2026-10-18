//! The dash scan over one string literal, shared by the
//! macro-argument surface and the `expect` / `expect_err` surface.
//!
//! The ASCII hyphen run is exempt here whatever the configuration
//! says, which is why both surfaces come through this one function. A
//! `--` inside a string literal is as often another language's comment
//! marker, an argument separator the program prints, or the
//! end-of-options separator itself as it is a dash, and a false
//! positive on program data costs more than the miss.

use super::scan::{Dash, DashPolicy, scan_dashes};
use crate::literal_scan::string_literal_quote_lengths;
use crate::markdown::scan_code_span_candidates;
use rustc_span::{BytePos, Pos, Span};

/// Every dash `policy` reports in the body of the string literal
/// `snippet` spells, each as a span inside `literal_span`.
///
/// `snippet` is the literal's source text, delimiters included, and
/// `literal_span` is where that text sits. Reading the source rather
/// than the literal's value is what leaves a dash written as a
/// `\u{2014}` escape alone: the escape is a deliberate spelling, and
/// the sibling rule that asks for a visible character to be written as
/// itself is the one with something to say about it.
pub(super) fn scan_string_literal(
    snippet: &str,
    literal_span: Span,
    policy: DashPolicy,
) -> Vec<(Span, Dash)> {
    // A synthesised span whose snippet is not the literal it claims to
    // be bails here, so nothing is reported against text that is not
    // in the file.
    let Some((prefix_length, suffix_length)) = string_literal_quote_lengths(snippet) else {
        return Vec::new();
    };
    let body = &snippet[prefix_length..snippet.len() - suffix_length];
    let policy = DashPolicy {
        ascii_run: false,
        ..policy
    };
    // A backtick-quoted run inside a message is a quotation as much
    // as one in a comment, so it keeps its dashes. Only the code
    // spans are masked, not the block-level code regions: a message
    // is one run of prose, and a four-space indent in one is
    // alignment rather than an indented code block.
    let skips = scan_code_span_candidates(body);
    scan_dashes(body, &skips, policy)
        .into_iter()
        .map(|(range, dash)| {
            let start = literal_span.lo() + BytePos::from_usize(prefix_length + range.start);
            let end = start + BytePos::from_usize(range.len());
            let span = Span::new(start, end, literal_span.ctxt(), literal_span.parent());
            (span, dash)
        })
        .collect()
}
