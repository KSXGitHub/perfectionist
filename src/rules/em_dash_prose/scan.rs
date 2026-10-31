//! Locating the dashes the rule flags in a slice of prose.
//!
//! One `take_*` per shape, each handing back the remainder, with
//! [`scan_dashes`] owning the walk and the skip mask. The parser style
//! is the one
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md` prescribes.

use crate::markdown::{SkipRange, position_in_skip, utf8_char_len};
use core::ops::Range;

/// A dash the rule flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Dash {
    /// U+2014 EM DASH.
    Em,
    /// U+2013 EN DASH.
    En,
    /// A run of two or more ASCII hyphens with a space on each side.
    AsciiRun,
}

impl Dash {
    /// How the diagnostic's first line names the dash.
    pub(super) fn description(self) -> &'static str {
        match self {
            Dash::Em => "em dash `—`",
            Dash::En => "en dash `–`",
            Dash::AsciiRun => "ASCII `--` used as an em dash",
        }
    }
}

/// Which dashes a scan reports. U+2014 is reported by every scan, so
/// it has no field here.
#[derive(Debug, Clone, Copy)]
pub(super) struct DashPolicy {
    /// Whether to report U+2013 EN DASH.
    pub(super) en_dash: bool,
    /// Whether to report a spaced run of ASCII hyphens.
    pub(super) ascii_run: bool,
}

/// Take U+2014 or U+2013 from the front of `input`.
fn take_unicode_dash(input: &str) -> Option<(Dash, &str)> {
    for (dash, glyph) in [(Dash::Em, '—'), (Dash::En, '–')] {
        if let Some(remainder) = input.strip_prefix(glyph) {
            return Some((dash, remainder));
        }
    }
    None
}

/// Take a run of two or more ASCII hyphens from the front of `input`,
/// reporting the run's byte length. A lone hyphen is a hyphen, so it
/// does not match.
fn take_hyphen_run(input: &str) -> Option<(usize, &str)> {
    let length = input.bytes().take_while(|byte| *byte == b'-').count();
    (length >= 2).then(|| (length, &input[length..]))
}

/// Quotation and title marks that come as a distinct pair, each entry
/// an `(opening, closing)` character.
///
/// Each of these delimits text the author is reproducing and nothing
/// else at its own end of the pair, so none needs the word-boundary
/// test that tells an ASCII single quote from an apostrophe. U+2019
/// doubles as an apostrophe, but it only ever closes a span this table
/// opens with U+2018. A Chinese title mark is listed here on the
/// quotation's own terms: a title is reproduced verbatim, so a dash
/// inside one belongs to whoever named the work. The backtick is
/// absent because a backtick-quoted run is a markdown code span, which
/// both of the rule's surfaces mask before the quotation scan runs.
const PAIRED_MARKS: &[(char, char)] = &[
    ('‘', '’'),   // single turned comma, single comma
    ('“', '”'),   // double turned comma, double comma
    ('‚', '‘'),   // single low-9, single turned comma
    ('„', '“'),   // double low-9, double turned comma
    ('«', '»'),   // guillemets
    ('‹', '›'),   // single guillemets
    ('「', '」'), // corner brackets
    ('『', '』'), // white corner brackets
    ('《', '》'), // double title marks
    ('〈', '〉'), // single title marks
];

/// Take a span delimited by one of the [`PAIRED_MARKS`] from the
/// front of `input`, reporting the byte length of the span including
/// both marks. The closing mark has to arrive on the same line, so an
/// opening mark left unclosed delimits nothing.
fn take_paired_quoted(input: &str) -> Option<usize> {
    for &(opening, closing) in PAIRED_MARKS {
        let Some(body) = input.strip_prefix(opening) else {
            continue;
        };
        let first_line = body.split('\n').next().unwrap_or(body);
        let Some(offset) = first_line.find(closing) else {
            continue;
        };
        return Some(opening.len_utf8() + offset + closing.len_utf8());
    }
    None
}

/// Whether an ASCII quote at `index` opens a quotation: it has to
/// start a word, so the byte before it is absent, blank, or an opening
/// bracket.
fn opens_quotation(bytes: &[u8], index: usize) -> bool {
    match index.checked_sub(1).and_then(|before| bytes.get(before)) {
        None => true,
        Some(byte) => byte.is_ascii_whitespace() || matches!(byte, b'(' | b'[' | b'{'),
    }
}

/// Take a double-quoted span from `index`, reporting the byte length of
/// the span including both quotes.
///
/// Any later `"` on the same line closes it. The ASCII double quote is
/// never anything but a delimiter in prose, so there is no shape to
/// rule out the way an apostrophe has to be ruled out for the single
/// quote.
fn take_double_quoted(bytes: &[u8], index: usize) -> Option<usize> {
    if bytes.get(index) != Some(&b'"') || !opens_quotation(bytes, index) {
        return None;
    }
    let mut cursor = index + 1;
    while let Some(&byte) = bytes.get(cursor) {
        match byte {
            b'\n' => return None,
            b'"' => return Some(cursor + 1 - index),
            _ => cursor += 1,
        }
    }
    None
}

/// Take a single-quoted span from `index`, reporting the byte length of
/// the span including both quotes.
///
/// The opening quote has to be followed by the text it quotes and the
/// closing quote preceded by it, which is what tells a quotation from
/// the apostrophe in `don't`: an apostrophe has a letter on its left,
/// so it opens nothing, and a sentence full of them pairs up no span.
/// A Rust lifetime reads as an opening quote and finds no close, so it
/// masks nothing either.
fn take_single_quoted(bytes: &[u8], index: usize) -> Option<usize> {
    if bytes.get(index) != Some(&b'\'') || !opens_quotation(bytes, index) {
        return None;
    }
    if !bytes
        .get(index + 1)
        .is_some_and(|byte| !byte.is_ascii_whitespace())
    {
        return None;
    }
    let mut cursor = index + 2;
    while let Some(&byte) = bytes.get(cursor) {
        if byte == b'\n' {
            return None;
        }
        let quoted_text_before = bytes
            .get(cursor - 1)
            .is_some_and(|byte| !byte.is_ascii_whitespace());
        let word_ends_after = match bytes.get(cursor + 1) {
            None => true,
            Some(byte) => {
                byte.is_ascii_whitespace()
                    || matches!(
                        byte,
                        b')' | b']' | b'}' | b'.' | b',' | b';' | b':' | b'!' | b'?',
                    )
            }
        };
        if byte == b'\'' && quoted_text_before && word_ends_after {
            return Some(cursor + 1 - index);
        }
        cursor += 1;
    }
    None
}

/// The byte ranges of the quotations in `text`, in source order and
/// non-overlapping.
///
/// A quotation is someone else's words, so the dashes inside one are
/// not the author's to restructure. Both ASCII marks are recognised,
/// as are the typographic pairs in [`PAIRED_MARKS`]. A quote mark
/// that falls inside `skips` opens nothing: the backtick span holding
/// `` `don't` `` would otherwise pair its apostrophe with a later one
/// and mask the prose in between.
fn scan_quotations(text: &str, skips: &[SkipRange]) -> Vec<SkipRange> {
    let bytes = text.as_bytes();
    let mut quotations: Vec<SkipRange> = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let length = if position_in_skip(skips, index) {
            None
        } else {
            take_double_quoted(bytes, index)
                .or_else(|| take_single_quoted(bytes, index))
                .or_else(|| take_paired_quoted(&text[index..]))
        };
        match length {
            Some(length) => {
                quotations.push(index..index + length);
                index += length;
            }
            None => index += utf8_char_len(bytes, index),
        }
    }
    quotations
}

/// Whether the byte before `start` and the byte after `end` are both an
/// ASCII space or tab.
///
/// This is what separates a dash stand-in from the hyphen runs that
/// carry their own meaning. `--help` has a letter after it, `-->` and
/// `<!--` have an angle bracket on one side, and a run that opens or
/// closes a line has nothing on one side at all.
fn is_spaced(bytes: &[u8], start: usize, end: usize) -> bool {
    let is_blank = |byte: Option<&u8>| matches!(byte, Some(b' ' | b'\t'));
    is_blank(start.checked_sub(1).and_then(|before| bytes.get(before))) && is_blank(bytes.get(end))
}

/// Every dash in `text` that `policy` reports, as a byte range into
/// `text` paired with what the range holds, in source order.
///
/// A range starting inside a `skips` entry is left out. That mask is
/// how a code span and a code block keep their dashes: a `--` inside
/// `` `cargo dylint -- --all-targets` `` is an argument separator, and
/// one inside a fenced `lua` block is the language's comment marker.
/// The quotations the scan finds for itself are left out for a
/// different reason: a quotation is someone else's words, and the
/// author cannot restructure a sentence they are only reproducing.
pub(super) fn scan_dashes(
    text: &str,
    skips: &[SkipRange],
    policy: DashPolicy,
) -> Vec<(Range<usize>, Dash)> {
    let bytes = text.as_bytes();
    let quotations = scan_quotations(text, skips);
    let exempt = |position: usize| {
        position_in_skip(skips, position) || position_in_skip(&quotations, position)
    };
    let mut found: Vec<(Range<usize>, Dash)> = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let rest = &text[index..];
        if let Some((dash, remainder)) = take_unicode_dash(rest) {
            let length = rest.len() - remainder.len();
            let reported = dash == Dash::Em || policy.en_dash;
            if reported && !exempt(index) {
                found.push((index..index + length, dash));
            }
            index += length;
            continue;
        }
        if let Some((length, _remainder)) = take_hyphen_run(rest) {
            if policy.ascii_run && is_spaced(bytes, index, index + length) && !exempt(index) {
                found.push((index..index + length, Dash::AsciiRun));
            }
            index += length;
            continue;
        }
        index += utf8_char_len(bytes, index);
    }
    found
}

#[cfg(test)]
mod tests;
