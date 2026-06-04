//! Markdown scanners shared by sibling rules that walk doc-comment
//! text. Two consumer tiers sit on the same `take_*` combinators (see
//! the "Markdown parsing" section of
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md`):
//!
//! - **Tier A — structural classification.** [`scan_skip_regions`]
//!   produces a vector of byte-range skip regions the consumer
//!   (`bare_url`, `bare_email`, `bare_issue_reference`) applies as a
//!   post-filter before emitting diagnostics.
//! - **Tier B — code-region mask.** [`scan_code_regions`] returns only
//!   the byte ranges of code spans and code blocks, for rules
//!   (`unicode_ellipsis_in_docs`) that just need to exclude code from
//!   a prose scan rather than classify every construct.
//!
//! The implementation is a hand-written parser-combinator walk per
//! the convention documented in
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md`. Only the constructs
//! the consuming rules need to skip are recognised:
//!
//! - `` `...` `` code spans.
//! - ` ``` ... ``` ` and `~~~ ... ~~~` fenced code blocks.
//! - 4-space-indented code blocks.
//! - `<...>` autolinks.
//! - `[text](dest)` inline links.
//! - `[text][id]` reference-style links.
//! - `[id]: dest` reference-link definitions.
//!
//! Headings and HTML tags are not classified by this helper — neither
//! is needed by the rules currently consuming it. The sibling
//! catalogue file's combinator surface lists them as future
//! extensions for `bare_identifier_reference` / `clap_help_no_markdown`.

use std::ops::Range;

/// One byte range to skip when scanning the markdown text.
///
/// Ranges are returned in source order and never overlap; consumers
/// can binary-search the list or walk it linearly.
pub(crate) type SkipRange = Range<usize>;

/// Walk `input` as a markdown fragment and return every byte range
/// that the bare-* rules should treat as a "skip region" — code
/// spans, code blocks, autolinks, inline / reference links, and
/// reference-link definitions.
///
/// The returned vector is sorted by start byte and non-overlapping.
/// Adjacent constructs (e.g. an autolink immediately followed by a
/// code span) can produce touching ranges, but ranges never overlap.
pub(crate) fn scan_skip_regions(input: &str) -> Vec<SkipRange> {
    let mut out: Vec<SkipRange> = Vec::new();
    let bytes = input.as_bytes();
    let mut idx = 0;
    let mut at_line_start = true;
    while idx < bytes.len() {
        let rest = &input[idx..];

        // Block-level constructs anchored at line start. Each of
        // the `take_*` block-level combinators below consumes a span
        // that ends either at a line boundary (the trailing `\n`
        // is included in the returned length) or at EOF, so the
        // next position is always the start of a new line — set
        // `at_line_start = true` unconditionally after a successful
        // match instead of probing the byte at the new position.
        if at_line_start {
            if let Some(len) = take_indented_code_block(input, idx) {
                out.push(idx..idx + len);
                idx += len;
                at_line_start = true;
                continue;
            }
            if let Some(len) = take_fenced_code_block(rest) {
                out.push(idx..idx + len);
                idx += len;
                at_line_start = true;
                continue;
            }
            if let Some(len) = take_reference_definition(rest) {
                out.push(idx..idx + len);
                idx += len;
                at_line_start = true;
                continue;
            }
        }

        // Inline constructs.
        let first = bytes[idx];
        match first {
            b'`' => {
                if let Some(len) = take_code_span(rest) {
                    out.push(idx..idx + len);
                    idx += len;
                    at_line_start = false;
                    continue;
                }
            }
            b'<' => {
                if let Some(len) = take_autolink(rest) {
                    out.push(idx..idx + len);
                    idx += len;
                    at_line_start = false;
                    continue;
                }
            }
            b'[' => {
                if let Some(len) = take_link(rest) {
                    out.push(idx..idx + len);
                    idx += len;
                    at_line_start = false;
                    continue;
                }
            }
            b'\n' => {
                idx += 1;
                at_line_start = true;
                continue;
            }
            _ => {}
        }

        // Advance one UTF-8 character. Per the parser-style
        // convention, we always know exactly how many bytes we
        // consumed; that property is what lets the rules anchor
        // diagnostic spans precisely.
        let ch_len = utf8_char_len(bytes, idx);
        idx += ch_len;
        at_line_start = false;
    }
    out
}

/// Tier B code-region mask: the byte ranges of CommonMark code
/// regions — inline code spans and block-level code (fenced and
/// four-space-indented blocks, which is where doc-test code lives).
/// Used by rules that scan doc-comment prose and need only to exclude
/// code from the scan, not classify every construct
/// (`unicode_ellipsis_in_docs`).
///
/// Block-level code is always part of the mask. `include_code_spans`
/// controls whether inline `` `...` `` spans are masked too: the
/// `unicode_ellipsis_in_docs` rule exposes this as its
/// `allow_in_code_spans` knob, since a project may want a flagged
/// character caught even inside an inline code span. A code span is
/// always *parsed* — so a backtick run inside it never spuriously
/// opens a second span — and only added to the mask when
/// `include_code_spans` is `true`.
///
/// Like [`scan_skip_regions`], the returned ranges are sorted by start
/// byte and never overlap.
pub(crate) fn scan_code_regions(input: &str, include_code_spans: bool) -> Vec<SkipRange> {
    let mut out: Vec<SkipRange> = Vec::new();
    let bytes = input.as_bytes();
    let mut idx = 0;
    let mut at_line_start = true;
    while idx < bytes.len() {
        let rest = &input[idx..];

        // Block-level code anchored at line start. Both combinators
        // consume through a line boundary (or EOF), so the next
        // position always begins a new line.
        if at_line_start {
            if let Some(len) = take_indented_code_block(input, idx) {
                out.push(idx..idx + len);
                idx += len;
                at_line_start = true;
                continue;
            }
            if let Some(len) = take_fenced_code_block(rest) {
                out.push(idx..idx + len);
                idx += len;
                at_line_start = true;
                continue;
            }
        }

        match bytes[idx] {
            b'`' => {
                if let Some(len) = take_code_span(rest) {
                    if include_code_spans {
                        out.push(idx..idx + len);
                    }
                    idx += len;
                    at_line_start = false;
                    continue;
                }
            }
            b'\n' => {
                idx += 1;
                at_line_start = true;
                continue;
            }
            _ => {}
        }

        idx += utf8_char_len(bytes, idx);
        at_line_start = false;
    }
    out
}

/// Tier A: byte ranges of the inline code spans that are *candidates*
/// for intra-doc-link rewriting — `` `...` `` code spans reached as
/// standalone inline constructs, i.e. *not* consumed as the text of a
/// `[...]` link (`` [`Foo`] ``, `` [`Foo`](dest) ``, `` [`Foo`][id] ``)
/// and *not* sitting inside a code block.
///
/// The walk shares the same combinator surface and ordering as
/// [`scan_skip_regions`]; the only difference is what it records. Block
/// constructs, autolinks, and links are *consumed* (so a `` `Foo` ``
/// inside `` [`Foo`] `` is skipped, having already been swallowed by
/// [`take_link`]) but never recorded — only the surviving bare code
/// spans are returned. Pulling a Rust identifier out of each range's
/// body is left to the consuming rule (`bare_identifier_reference`), per the
/// "Markdown parsing" convention that Rust-aware extraction lives in
/// the rule, not in this helper.
///
/// Like [`scan_skip_regions`], the returned ranges are sorted by start
/// byte and never overlap.
pub(crate) fn scan_code_span_candidates(input: &str) -> Vec<SkipRange> {
    let mut out: Vec<SkipRange> = Vec::new();
    let bytes = input.as_bytes();
    let mut idx = 0;
    let mut at_line_start = true;
    while idx < bytes.len() {
        let rest = &input[idx..];

        // Block-level constructs anchored at line start are consumed
        // but not candidates (see [`scan_skip_regions`] for why each
        // ends on a line boundary, so `at_line_start` stays true).
        if at_line_start {
            if let Some(len) = take_indented_code_block(input, idx) {
                idx += len;
                at_line_start = true;
                continue;
            }
            if let Some(len) = take_fenced_code_block(rest) {
                idx += len;
                at_line_start = true;
                continue;
            }
            if let Some(len) = take_reference_definition(rest) {
                idx += len;
                at_line_start = true;
                continue;
            }
        }

        match bytes[idx] {
            b'`' => {
                if let Some(len) = take_code_span(rest) {
                    out.push(idx..idx + len);
                    idx += len;
                    at_line_start = false;
                    continue;
                }
            }
            b'<' => {
                if let Some(len) = take_autolink(rest) {
                    idx += len;
                    at_line_start = false;
                    continue;
                }
            }
            b'[' => {
                // A link consumes its `[...]` text wholesale, so the
                // code span inside `` [`Foo`] `` is swallowed here and
                // never recorded as a candidate.
                if let Some(len) = take_link(rest) {
                    idx += len;
                    at_line_start = false;
                    continue;
                }
            }
            b'\n' => {
                idx += 1;
                at_line_start = true;
                continue;
            }
            _ => {}
        }

        idx += utf8_char_len(bytes, idx);
        at_line_start = false;
    }
    out
}

/// Number of bytes the UTF-8 character starting at `bytes[idx]`
/// occupies. Falls back to 1 if the byte sequence is malformed; the
/// caller has the invariant that `input` is a valid UTF-8 `&str`, so
/// the fallback never triggers in practice.
pub(crate) fn utf8_char_len(bytes: &[u8], idx: usize) -> usize {
    let lead = bytes[idx];
    if lead < 0xC0 {
        1
    } else if lead < 0xE0 {
        2
    } else if lead < 0xF0 {
        3
    } else {
        4
    }
}

/// Take a `` `...` `` code span. The opening and closing fences are
/// runs of N backticks of equal length; the body may contain shorter
/// runs but not a run of exactly N. Returns the total byte length
/// (opening fence + body + closing fence) on success.
///
/// Matches CommonMark's intra-line code-span rule. Multi-line code
/// spans are accepted here — the closing fence is searched up to the
/// end of `input` — because doc comments routinely place a backticked
/// identifier across the soft-wrap boundary between two `///` lines,
/// and treating that as two separate spans would let bare-* rules
/// flag content inside.
fn take_code_span(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'`') {
        return None;
    }
    let mut open_len = 0;
    while open_len < bytes.len() && bytes[open_len] == b'`' {
        open_len += 1;
    }
    let mut index = open_len;
    while index < bytes.len() {
        if bytes[index] == b'`' {
            let mut run = 0;
            while index + run < bytes.len() && bytes[index + run] == b'`' {
                run += 1;
            }
            if run == open_len {
                return Some(index + run);
            }
            index += run;
        } else {
            index += 1;
        }
    }
    None
}

/// Take a fenced code block starting at the current position.
/// Recognises both `` ``` `` and `~~~` fences with a run length of at
/// least 3. The block ends at the next line whose only content (up to
/// optional trailing whitespace) is a fence of the same character
/// with a length ≥ the opening fence, or at end of input.
///
/// `input` is expected to start at column 0 of a logical line; the
/// caller's `at_line_start` flag guarantees this.
fn take_fenced_code_block(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut leading_spaces = 0;
    while leading_spaces < 4 && leading_spaces < bytes.len() && bytes[leading_spaces] == b' ' {
        leading_spaces += 1;
    }
    if leading_spaces >= 4 {
        return None;
    }
    let fence_start = leading_spaces;
    let fence_char = *bytes.get(fence_start)?;
    if fence_char != b'`' && fence_char != b'~' {
        return None;
    }
    let mut open_len = 0;
    while fence_start + open_len < bytes.len() && bytes[fence_start + open_len] == fence_char {
        open_len += 1;
    }
    if open_len < 3 {
        return None;
    }
    // Skip to end of opening line (info string is ignored).
    let mut index = fence_start + open_len;
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    if index < bytes.len() {
        index += 1; // consume newline
    }
    // Walk lines until a closing fence is found.
    while index < bytes.len() {
        let mut spaces = 0;
        while spaces < 3 && index + spaces < bytes.len() && bytes[index + spaces] == b' ' {
            spaces += 1;
        }
        let candidate = index + spaces;
        if candidate < bytes.len() && bytes[candidate] == fence_char {
            let mut run = 0;
            while candidate + run < bytes.len() && bytes[candidate + run] == fence_char {
                run += 1;
            }
            if run >= open_len {
                let mut end = candidate + run;
                while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
                    end += 1;
                }
                if end >= bytes.len() || bytes[end] == b'\n' {
                    let end = if end < bytes.len() { end + 1 } else { end };
                    return Some(end);
                }
            }
        }
        // Advance past this line.
        while index < bytes.len() && bytes[index] != b'\n' {
            index += 1;
        }
        if index < bytes.len() {
            index += 1;
        }
    }
    Some(bytes.len())
}

/// Take an indented code block — one or more consecutive lines
/// whose visible content begins at column ≥ 4. Returns the total
/// byte length (including the terminating newline of the last
/// indented line, if any). The block ends at the first
/// non-blank line that is indented less than four spaces.
///
/// `idx` is the absolute byte offset where the block must start;
/// the caller has already verified that this is a line start.
fn take_indented_code_block(input: &str, idx: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    // CommonMark requires that an indented code block be preceded by
    // a blank line (or be at the document start). Approximating that
    // here as "previous line is blank or absent" — close enough for
    // doc-comment content, which is the only consumer.
    if idx > 0 {
        let newline_pos = idx - 1;
        if bytes[newline_pos] != b'\n' {
            return None;
        }
        // Walk backwards to find the prior line's content. If it has
        // any non-whitespace byte, this isn't a code-block start.
        if newline_pos > 0 {
            let mut scan_back = newline_pos;
            while scan_back > 0 && bytes[scan_back - 1] != b'\n' {
                scan_back -= 1;
            }
            let prev_line = &bytes[scan_back..newline_pos];
            if prev_line.iter().any(|byte| *byte != b' ' && *byte != b'\t') {
                return None;
            }
        }
        let _ = newline_pos;
    }
    let mut index = idx;
    let mut consumed_any = false;
    loop {
        let line_start = index;
        // Measure indentation in columns, expanding a tab to the next
        // multiple of 4 (CommonMark §2.2): four columns of indentation
        // open an indented code block. Counting bytes would miss a
        // tab-indented code line and wrongly scan its content.
        let mut columns = 0;
        while index < bytes.len() && columns < 4 {
            match bytes[index] {
                b' ' => columns += 1,
                b'\t' => columns += 4 - (columns % 4),
                _ => break,
            }
            index += 1;
        }
        if columns < 4 {
            // Blank line: allow as continuation.
            let mut end = line_start;
            while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
                end += 1;
            }
            if end < bytes.len() && bytes[end] == b'\n' && consumed_any {
                index = end + 1;
                continue;
            }
            // Not indented enough; back up.
            return if consumed_any {
                Some(line_start - idx)
            } else {
                None
            };
        }
        // Indented content — consume to end of line.
        while index < bytes.len() && bytes[index] != b'\n' {
            index += 1;
        }
        if index < bytes.len() {
            index += 1;
        }
        consumed_any = true;
        if index >= bytes.len() {
            return Some(index - idx);
        }
    }
}

/// Take a reference-link definition: `[id]: dest` at the start of a
/// line, with optional leading whitespace (up to 3 spaces). Returns
/// the byte length of the definition including the trailing newline,
/// if any.
fn take_reference_definition(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < 3 && index < bytes.len() && bytes[index] == b' ' {
        index += 1;
    }
    if bytes.get(index) != Some(&b'[') {
        return None;
    }
    index += 1;
    let label_start = index;
    while index < bytes.len() && bytes[index] != b']' && bytes[index] != b'\n' {
        index += 1;
    }
    if index == label_start || bytes.get(index) != Some(&b']') {
        return None;
    }
    index += 1;
    if bytes.get(index) != Some(&b':') {
        return None;
    }
    index += 1;
    // Consume the destination — to end of line.
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    if index < bytes.len() {
        index += 1;
    }
    Some(index)
}

/// Take an autolink `<scheme://...>` or `<mailto:...>` or
/// `<user@example.com>`. The opening `<` is at `input[0]`. Returns the
/// total byte length on success. Returns `None` if the construct
/// looks like an HTML tag or contains whitespace.
fn take_autolink(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'<') {
        return None;
    }
    let mut index = 1;
    let body_start = index;
    while index < bytes.len() {
        match bytes[index] {
            b'>' => {
                if index == body_start {
                    return None;
                }
                let body = &input[body_start..index];
                // Autolinks are URI-shaped (scheme://...) or
                // email-shaped (contains `@` and no whitespace).
                if looks_like_uri_or_email(body) {
                    return Some(index + 1);
                }
                return None;
            }
            b' ' | b'\t' | b'\n' | b'<' => return None,
            _ => index += 1,
        }
    }
    None
}

/// Whether `body` (between an autolink's `<` and `>`) looks like a
/// URI or an email address. URIs are recognised by a leading
/// `<scheme>:` prefix where `<scheme>` is one or more ASCII letters
/// followed by `+`, `-`, or `.`-extended characters. Emails are
/// recognised by the presence of `@` with non-empty local and
/// domain parts.
fn looks_like_uri_or_email(body: &str) -> bool {
    let bytes = body.as_bytes();
    // URI?
    let mut index = 0;
    while index < bytes.len() && (bytes[index].is_ascii_alphabetic()) {
        index += 1;
    }
    if index > 0 {
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric()
                || bytes[index] == b'+'
                || bytes[index] == b'-'
                || bytes[index] == b'.')
        {
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b':' && index + 1 < bytes.len() {
            return true;
        }
    }
    // Email?
    if let Some(at) = body.find('@')
        && at > 0
        && at + 1 < body.len()
    {
        return true;
    }
    false
}

/// Take an inline link `[text](dest)` or reference link
/// `[text][id]` / `[text]`. Returns the byte length spanning from the
/// opening `[` through the closing `)` / `]` (or just `]` for the
/// collapsed form).
fn take_link(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'[') {
        return None;
    }
    let mut index = 1;
    let mut depth: i32 = 1;
    while index < bytes.len() && depth > 0 {
        match bytes[index] {
            b'\\' => {
                index += 2;
                continue;
            }
            b'[' => {
                depth += 1;
                index += 1;
            }
            b']' => {
                depth -= 1;
                index += 1;
            }
            b'`' => {
                if let Some(len) = take_code_span(&input[index..]) {
                    index += len;
                } else {
                    index += 1;
                }
            }
            b'\n' => {
                // Allow a single soft line break inside link text;
                // bail on two in a row (would terminate the paragraph
                // in CommonMark).
                if index + 1 < bytes.len() && bytes[index + 1] == b'\n' {
                    return None;
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    if depth != 0 {
        return None;
    }
    // `index` now points one past the closing `]` of the link text.
    // Look ahead for `(...)`, `[...]`, or nothing (collapsed).
    if index < bytes.len() && bytes[index] == b'(' {
        let body_start = index + 1;
        let mut end = body_start;
        let mut paren_depth: i32 = 1;
        while end < bytes.len() && paren_depth > 0 {
            match bytes[end] {
                b'\\' => {
                    end += 2;
                    continue;
                }
                b'(' => paren_depth += 1,
                b')' => paren_depth -= 1,
                b'\n' => return Some(index),
                _ => {}
            }
            end += 1;
        }
        if paren_depth == 0 {
            return Some(end);
        }
        return Some(index);
    }
    if index < bytes.len() && bytes[index] == b'[' {
        let mut end = index + 1;
        while end < bytes.len() && bytes[end] != b']' && bytes[end] != b'\n' {
            end += 1;
        }
        if end < bytes.len() && bytes[end] == b']' {
            return Some(end + 1);
        }
    }
    Some(index)
}

/// Whether byte position `pos` of the input falls inside any of the
/// skip ranges returned by [`scan_skip_regions`]. Returns `true` if
/// `pos` is `>= start && < end` for some range.
pub(crate) fn position_in_skip(skips: &[SkipRange], pos: usize) -> bool {
    skips
        .iter()
        .any(|range| pos >= range.start && pos < range.end)
}

#[cfg(test)]
mod tests;
