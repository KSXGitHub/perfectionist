//! Parser-combinator-style scanner for a cooked Rust string literal's
//! body. Each `take_*` function follows the convention in
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md`: consume a recognised
//! prefix from the input and return the rest, or `None` if the input
//! doesn't start with that shape.
//!
//! The walk decides whether a literal contains *only* escapes that a
//! raw string would express verbatim (`\"`, `\\`, `\'`). A single
//! escape outside that set — `\n`, `\t`, `\xNN`, `\u{...}`, line
//! continuations — makes the whole literal ineligible.

/// Default eligible escape sequences: the three escapes that a raw
/// string can express verbatim with no escape at all. Also used as
/// the closed set against which user-supplied `escapes_eligible`
/// entries are validated.
pub(super) const DEFAULT_ESCAPES_ELIGIBLE: &[&str] = &[r#"\""#, r"\\", r"\'"];

pub(super) struct ScanResult {
    pub(super) eliminable_count: usize,
    pub(super) decoded: String,
}

/// Walk the body of a cooked string literal (everything between the
/// surrounding quotes) and classify each escape. Returns `None` if
/// the body contains any non-raw escape — `\n`, `\t`, `\r`, `\0`,
/// `\xNN`, `\u{...}`, line continuations, or any other backslash
/// sequence that is not listed in the configured `escapes_eligible`.
pub(super) fn scan_body(body: &str, eligible: &[String]) -> Option<ScanResult> {
    let mut rest = body;
    let mut eliminable_count: usize = 0;
    let mut decoded = String::with_capacity(body.len());
    while !rest.is_empty() {
        if let Some((escape, remainder)) = take_escape_eliminable(rest, eligible) {
            decoded.push_str(eliminable_decoded(escape));
            eliminable_count = eliminable_count.saturating_add(1);
            rest = remainder;
            continue;
        }
        if take_escape_non_raw(rest).is_some() {
            return None;
        }
        let (literal, remainder) = take_literal_char(rest)?;
        decoded.push_str(literal);
        rest = remainder;
    }
    Some(ScanResult {
        eliminable_count,
        decoded,
    })
}

/// Take a prefix of `input` that matches one of the configured
/// eligible escape sequences. Each entry is matched literally
/// against the input — no decoding, no normalisation. Entries
/// reach this function only after [`is_supported_eligible_entry`]
/// has accepted them, so they are non-empty by construction.
fn take_escape_eliminable<'a>(input: &'a str, eligible: &[String]) -> Option<(&'a str, &'a str)> {
    for entry in eligible {
        if input.starts_with(entry.as_str()) {
            return Some(input.split_at(entry.len()));
        }
    }
    None
}

/// Decode an eligible escape into the verbatim text it represents
/// in a raw string. Eligible entries are constrained to the three
/// self-decoding escapes by [`is_supported_eligible_entry`], so the
/// decoded form is exactly the entry with its leading backslash
/// removed.
fn eliminable_decoded(escape: &str) -> &str {
    &escape['\\'.len_utf8()..]
}

/// Take any backslash escape from the front of `input`. Recognises
/// `\xNN` (4 bytes), `\u{...}` (variable length), and any
/// single-character escape (`\n`, `\t`, `\r`, `\0`, `\"`, `\\`,
/// `\'`, line continuation, …). Returns `None` if `input` does not
/// start with `\` or the escape is malformed (incomplete `\u{...}`
/// without a closing brace, dangling backslash at the end of input,
/// truncated `\xNN`).
fn take_escape_non_raw(input: &str) -> Option<(&str, &str)> {
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'\\') {
        return None;
    }
    let second_byte = *bytes.get(1)?;
    let escape_len = match second_byte {
        b'x' => 4,
        b'u' => {
            // `\u{...}`: scan to the closing `}`. The bytes between
            // `{` and `}` are constrained to ASCII hex by the rustc
            // lexer, so a byte-level scan is sufficient.
            let mut length: usize = 2;
            let mut closing_found = false;
            for &byte in &bytes[2..] {
                length = length.saturating_add(1);
                if byte == b'}' {
                    closing_found = true;
                    break;
                }
            }
            if !closing_found {
                return None;
            }
            length
        }
        _ => {
            // `\` + a single UTF-8 character (e.g. `\n`, `\"`,
            // or the line-continuation `\<newline>`).
            let second_char = input['\\'.len_utf8()..].chars().next()?;
            '\\'.len_utf8() + second_char.len_utf8()
        }
    };
    if escape_len > input.len() {
        return None;
    }
    Some(input.split_at(escape_len))
}

/// Take a single non-backslash UTF-8 character from the front of
/// `input`. Returns `None` only when `input` is empty or starts with
/// `\`, in which case the caller should run one of the escape
/// combinators first.
fn take_literal_char(input: &str) -> Option<(&str, &str)> {
    let first = input.chars().next()?;
    if first == '\\' {
        return None;
    }
    Some(input.split_at(first.len_utf8()))
}

/// Smallest number of `#` characters needed so that the closing
/// `"<n #s>` sequence does not appear inside `decoded`.
///
/// In practice this is 0 for paths and 1 for JSON / HTML snippets;
/// longer runs only matter when the literal itself embeds
/// raw-string source text.
pub(super) fn minimal_hash_count(decoded: &str) -> usize {
    let mut hashes = String::new();
    let mut count: usize = 0;
    loop {
        let mut pattern = String::with_capacity('"'.len_utf8() + hashes.len());
        pattern.push('"');
        pattern.push_str(&hashes);
        if !decoded.contains(&pattern) {
            return count;
        }
        hashes.push('#');
        count = count.saturating_add(1);
    }
}

/// A supported `escapes_eligible` entry is one of the three Rust
/// escapes that self-decode — that is, whose decoded character is
/// exactly the byte that follows the backslash: `\"`, `\\`, `\'`.
/// [`eliminable_decoded`]'s contract is "strip the leading backslash",
/// which only holds for these three. Every other valid Rust escape
/// (`\n`, `\t`, `\r`, `\0`, `\xNN`, `\u{...}`) decodes to a
/// different character, so accepting it here would let the autofix
/// silently corrupt strings — e.g. `escapes_eligible = ["\\n"]`
/// would rewrite a newline-containing literal to one containing the
/// letter `n`.
///
/// The supported set is the same one named by
/// [`DEFAULT_ESCAPES_ELIGIBLE`]; matching against that constant
/// keeps the two definitions from drifting apart if a future
/// extension to [`eliminable_decoded`] ever adds a fourth entry.
pub(super) fn is_supported_eligible_entry(entry: &str) -> bool {
    DEFAULT_ESCAPES_ELIGIBLE.contains(&entry)
}
