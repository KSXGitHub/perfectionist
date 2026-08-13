//! Markdown scanners shared by sibling rules that walk doc-comment
//! text. Two consumer tiers sit on the same `take_*` combinators (see
//! the "Markdown parsing" section of
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md`):
//!
//! - **Tier A — structural classification.** [`scan_skip_regions`]
//!   produces a vector of byte-range skip regions the consumer
//!   (`bare_url`, `bare_email`, `bare_issue_reference`) applies as a
//!   post-filter before emitting diagnostics. [`classify_constructs`]
//!   is the richer Tier A entry point: it returns every construct's
//!   byte range *and* its [`ConstructKind`], which
//!   `clap_help_markdown` maps onto its forbidden-construct
//!   categories.
//! - **Tier B — code-region mask.** [`scan_code_regions`] returns only
//!   the byte ranges of code spans and code blocks, for rules
//!   (`unicode_ellipsis_in_docs`) that just need to exclude code from
//!   a prose scan rather than classify every construct.
//!
//! The implementation is a hand-written parser-combinator walk per
//! the convention documented in
//! `planned-rules/IMPLEMENTATION_CONVENTIONS.md`. The recognised
//! constructs are:
//!
//! - `` `...` `` code spans.
//! - ` ``` ... ``` ` and `~~~ ... ~~~` fenced code blocks.
//! - 4-space-indented code blocks.
//! - `<...>` autolinks.
//! - `[text](dest)` inline links.
//! - `[text][id]` reference-style links.
//! - `[id]: dest` reference-link definitions.
//! - `<tag ...>` / `</tag>` / `<!-- ... -->` HTML.
//! - ATX (`# h`) and Setext (`h\n===`) headings.
//! - `**bold**` / `*italic*` emphasis and bullet / ordered list
//!   markers (only when [`classify_constructs`] is asked for them; see
//!   [`ClassifyOptions`]).

use core::ops::Range;

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
/// `unicode_ellipsis_in_docs` rule passes the inverse of its
/// `scan_code_spans` knob, since a project may want a flagged
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

/// Which of the three CommonMark `[...]` link shapes a [`classify_link`]
/// match is. `clap_help_markdown` reports each under a distinct
/// "forbidden construct" category, so the discriminant is preserved
/// rather than collapsed to a length.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LinkForm {
    /// `[text](dest)` — an inline link with a parenthesised
    /// destination.
    Inline,
    /// `[text][id]` — a full reference link naming a separate
    /// definition.
    Reference,
    /// `[text]` / `` [`Type`] `` — the collapsed / shortcut form with
    /// no trailing destination. This is also the shape a rustdoc
    /// intra-doc link wears.
    Shortcut,
}

/// Take an inline link `[text](dest)` or reference link
/// `[text][id]` / `[text]`. Returns the byte length spanning from the
/// opening `[` through the closing `)` / `]` (or just `]` for the
/// collapsed form).
fn take_link(input: &str) -> Option<usize> {
    classify_link(input).map(|(_, len)| len)
}

/// Like [`take_link`] but also reports which [`LinkForm`] matched. The
/// length is identical; the form lets a structural-classification
/// consumer distinguish the three link shapes.
fn classify_link(input: &str) -> Option<(LinkForm, usize)> {
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
                b'\n' => return Some((LinkForm::Shortcut, index)),
                _ => {}
            }
            end += 1;
        }
        if paren_depth == 0 {
            return Some((LinkForm::Inline, end));
        }
        return Some((LinkForm::Shortcut, index));
    }
    if index < bytes.len() && bytes[index] == b'[' {
        let mut end = index + 1;
        while end < bytes.len() && bytes[end] != b']' && bytes[end] != b'\n' {
            end += 1;
        }
        if end < bytes.len() && bytes[end] == b']' {
            return Some((LinkForm::Reference, end + 1));
        }
    }
    Some((LinkForm::Shortcut, index))
}

/// Whether byte position `pos` of the input falls inside any of the
/// skip ranges returned by [`scan_skip_regions`]. Returns `true` if
/// `pos` is `>= start && < end` for some range.
pub(crate) fn position_in_skip(skips: &[SkipRange], pos: usize) -> bool {
    skips
        .iter()
        .any(|range| pos >= range.start && pos < range.end)
}

/// One markdown construct classified by [`classify_constructs`], with
/// its byte range in the scanned input.
pub(crate) struct Construct {
    pub(crate) range: Range<usize>,
    pub(crate) kind: ConstructKind,
}

/// The kind of a [`Construct`] — the full Tier A structural taxonomy
/// (see the "Markdown parsing" section of
/// `planned-rules/IMPLEMENTATION_CONVENTIONS.md`). `clap_help_markdown`
/// maps each kind onto a user-facing "forbidden construct" category and
/// decides per kind whether to flag it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ConstructKind {
    /// `` `code` `` inline code span.
    CodeSpan,
    /// Fenced or four-space-indented code block.
    CodeBlock,
    /// `[text](dest)` inline link.
    InlineLink,
    /// `[text][id]` full reference link.
    ReferenceLink,
    /// `[id]: dest` reference-link definition.
    ReferenceDefinition,
    /// `[text]` / `` [`Type`] `` collapsed link — the rustdoc
    /// intra-doc-link shape.
    IntraDocLink,
    /// `<https://...>` / `<user@host>` autolink. Recognised so it is
    /// not mistaken for an HTML tag; consuming rules treat it as
    /// allowed.
    Autolink,
    /// `<tag ...>` / `</tag>` / `<!-- ... -->` raw HTML.
    HtmlTag,
    /// ATX (`# h`) or Setext (`h\n===`) heading.
    Heading,
    /// `**bold**` / `__bold__` strong emphasis.
    Bold,
    /// `*italic*` / `_italic_` emphasis.
    Italic,
    /// A bullet (`-`/`+`/`*`) or ordered (`1.`/`1)`) list marker.
    List,
}

/// Which optional construct families [`classify_constructs`] scans for
/// on top of the always-classified structural set. Emphasis and list
/// detection are off unless a consumer asks for them, because their
/// CommonMark rules are flanking-sensitive and the catalogue only needs
/// them behind `clap_help_markdown`'s opt-in `extra_forbid` knob.
#[derive(Clone, Copy, Default)]
pub(crate) struct ClassifyOptions {
    pub(crate) detect_emphasis: bool,
    pub(crate) detect_lists: bool,
}

/// Tier A structural classification: walk `input` as a markdown
/// fragment and return every recognised [`Construct`] in source order.
///
/// The walk shares the combinator surface and line-start bookkeeping of
/// [`scan_skip_regions`]; the difference is that it records *what* each
/// construct is, not just that it is a skip region. Code blocks, code
/// spans, links, autolinks, reference definitions, HTML tags, and
/// headings are always classified; emphasis and list markers only when
/// `options` requests them.
///
/// Returned ranges are sorted by start byte. They are non-overlapping
/// except that a Setext heading's underline never coincides with an
/// inline construct, so the two families do not collide.
pub(crate) fn classify_constructs(input: &str, options: ClassifyOptions) -> Vec<Construct> {
    let mut out: Vec<Construct> = Vec::new();
    let bytes = input.as_bytes();
    let mut idx = 0;
    let mut at_line_start = true;
    while idx < bytes.len() {
        let rest = &input[idx..];

        if at_line_start {
            if let Some(len) = take_indented_code_block(input, idx) {
                out.push(construct(idx, len, ConstructKind::CodeBlock));
                idx += len;
                at_line_start = true;
                continue;
            }
            if let Some(len) = take_fenced_code_block(rest) {
                out.push(construct(idx, len, ConstructKind::CodeBlock));
                idx += len;
                at_line_start = true;
                continue;
            }
            if let Some(len) = take_reference_definition(rest) {
                out.push(construct(idx, len, ConstructKind::ReferenceDefinition));
                idx += len;
                at_line_start = true;
                continue;
            }
            if let Some(len) = take_atx_heading(rest) {
                out.push(construct(idx, len, ConstructKind::Heading));
                idx += len;
                // `take_atx_heading` stops before the line's `\n`, so
                // the next byte is still on this line; let the normal
                // newline handling below flip `at_line_start`.
                at_line_start = false;
                continue;
            }
            if options.detect_lists
                && let Some(len) = take_list_marker(rest)
            {
                out.push(construct(idx, len, ConstructKind::List));
                idx += len;
                at_line_start = false;
                continue;
            }
        }

        match bytes[idx] {
            b'`' => {
                if let Some(len) = take_code_span(rest) {
                    out.push(construct(idx, len, ConstructKind::CodeSpan));
                    idx += len;
                    at_line_start = false;
                    continue;
                }
            }
            b'<' => {
                if let Some(len) = take_autolink(rest) {
                    out.push(construct(idx, len, ConstructKind::Autolink));
                    idx += len;
                    at_line_start = false;
                    continue;
                }
                if let Some(len) = take_html_tag(rest) {
                    out.push(construct(idx, len, ConstructKind::HtmlTag));
                    idx += len;
                    at_line_start = false;
                    continue;
                }
            }
            b'[' => {
                if let Some((form, len)) = classify_link(rest) {
                    let kind = match form {
                        LinkForm::Inline => ConstructKind::InlineLink,
                        LinkForm::Reference => ConstructKind::ReferenceLink,
                        LinkForm::Shortcut => ConstructKind::IntraDocLink,
                    };
                    out.push(construct(idx, len, kind));
                    idx += len;
                    at_line_start = false;
                    continue;
                }
            }
            b'*' | b'_' if options.detect_emphasis => {
                if let Some((kind, len)) = take_emphasis(input, idx) {
                    out.push(construct(idx, len, kind));
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

    detect_setext_headings(input, &mut out);
    out.sort_by_key(|construct| construct.range.start);
    out
}

fn construct(start: usize, len: usize, kind: ConstructKind) -> Construct {
    Construct {
        range: start..start + len,
        kind,
    }
}

/// Take an ATX heading — up to three leading spaces, a run of one to
/// six `#`, then a space / tab / end-of-line. Returns the byte length
/// up to (not including) the terminating `\n`, so the heading range
/// stays on a single rendered line and maps to a precise span.
fn take_atx_heading(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < 3 && index < bytes.len() && bytes[index] == b' ' {
        index += 1;
    }
    if bytes.get(index) != Some(&b'#') {
        return None;
    }
    let mut hashes = 0;
    while index + hashes < bytes.len() && bytes[index + hashes] == b'#' {
        hashes += 1;
    }
    if hashes > 6 {
        return None;
    }
    let after = index + hashes;
    if after < bytes.len() && bytes[after] != b' ' && bytes[after] != b'\t' && bytes[after] != b'\n'
    {
        return None;
    }
    let mut end = after;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }
    Some(end)
}

/// Detect Setext headings — a non-blank text line immediately followed
/// by an underline line of only `=` (level 1) or `-` (level 2). Records
/// the underline line's range as a [`ConstructKind::Heading`]. Run as a
/// separate line scan because Setext needs one line of look-back, which
/// the forward inline walk does not carry. The underline of a `-`-only
/// line preceded by a paragraph is a Setext heading in CommonMark (it
/// wins over a thematic break), so this matches the spec while leaving a
/// `---` preceded by a blank line — a thematic break — alone.
fn detect_setext_headings(input: &str, out: &mut Vec<Construct>) {
    let bytes = input.as_bytes();
    let mut line_start = 0;
    // Byte range of the previous line when it is a paragraph-text line
    // eligible to be a Setext heading's text; `None` after a blank line,
    // an underline, or a line already inside a recorded block construct.
    let mut prev_text: Option<()> = None;
    loop {
        let mut line_end = line_start;
        while line_end < bytes.len() && bytes[line_end] != b'\n' {
            line_end += 1;
        }
        let line = &input[line_start..line_end];
        let trimmed = line.trim();
        let leading = line.len() - line.trim_start().len();
        let is_underline = !trimmed.is_empty()
            && leading <= 3
            && (trimmed.bytes().all(|byte| byte == b'=')
                || trimmed.bytes().all(|byte| byte == b'-'));
        if is_underline && prev_text.is_some() {
            if !range_overlaps(out, line_start..line_end) {
                out.push(Construct {
                    range: line_start..line_end,
                    kind: ConstructKind::Heading,
                });
            }
            prev_text = None;
        } else if trimmed.is_empty() {
            prev_text = None;
        } else if range_overlaps(out, line_start..line_end) {
            // Inside a code block / other recorded construct; not Setext
            // text.
            prev_text = None;
        } else {
            prev_text = Some(());
        }
        if line_end >= bytes.len() {
            break;
        }
        line_start = line_end + 1;
    }
}

fn range_overlaps(constructs: &[Construct], range: Range<usize>) -> bool {
    constructs
        .iter()
        .any(|other| other.range.start < range.end && range.start < other.range.end)
}

/// Take a leading bullet (`-` / `+` / `*` then a space / tab) or
/// ordered (`<digits>` then `.` / `)` then a space / tab) list marker.
/// Returns the marker's byte length (through the delimiter, not its
/// trailing space) so the rest of the list item is still scanned for
/// inline constructs.
fn take_list_marker(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < 3 && index < bytes.len() && bytes[index] == b' ' {
        index += 1;
    }
    match bytes.get(index) {
        Some(b'-' | b'+' | b'*') => {
            let next = bytes.get(index + 1);
            if next == Some(&b' ') || next == Some(&b'\t') {
                return Some(index + 1);
            }
            None
        }
        Some(byte) if byte.is_ascii_digit() => {
            let mut end = index;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end < bytes.len() && (bytes[end] == b'.' || bytes[end] == b')') {
                let next = bytes.get(end + 1);
                if next == Some(&b' ') || next == Some(&b'\t') {
                    return Some(end + 1);
                }
            }
            None
        }
        _ => None,
    }
}

/// Take a raw-HTML construct beginning at `<`: an open / close tag, a
/// self-closing tag, an HTML comment, a declaration (`<!...>`), or a
/// processing instruction (`<?...?>`). Returns the total byte length.
///
/// The opening `<` must be followed by `/`, an ASCII letter, `!`, or
/// `?` — so prose like `a < b` or `<3` is left alone. Quoted attribute
/// values are skipped so a `>` inside them does not close the tag
/// early. The caller tries [`take_autolink`] first, so a `<https://...>`
/// autolink never reaches here.
fn take_html_tag(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'<') {
        return None;
    }
    if input[1..].starts_with("!--") {
        return input.find("-->").map(|pos| pos + 3);
    }
    if bytes.get(1) == Some(&b'!') {
        return input[1..].find('>').map(|pos| 1 + pos + 1);
    }
    if bytes.get(1) == Some(&b'?') {
        return input.find("?>").map(|pos| pos + 2);
    }
    let mut index = 1;
    if bytes.get(index) == Some(&b'/') {
        index += 1;
    }
    if !bytes.get(index).is_some_and(u8::is_ascii_alphabetic) {
        return None;
    }
    index += 1;
    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
    {
        index += 1;
    }
    while index < bytes.len() {
        match bytes[index] {
            b'>' => return Some(index + 1),
            quote @ (b'"' | b'\'') => {
                index += 1;
                while index < bytes.len() && bytes[index] != quote {
                    index += 1;
                }
                if index >= bytes.len() {
                    return None;
                }
                index += 1;
            }
            b'<' => return None,
            b'\n' => {
                if index + 1 < bytes.len() && bytes[index + 1] == b'\n' {
                    return None;
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    None
}

/// Take a `*` / `_` emphasis run at `idx`. Returns the matched
/// [`ConstructKind::Bold`] (run length ≥ 2) or
/// [`ConstructKind::Italic`] (run length 1) and the total byte length
/// through the closing delimiter.
///
/// This is a pragmatic matcher, not a full CommonMark flanking
/// resolver: it requires non-blank content that does not begin or end
/// with whitespace and a closing run at least as long as the opening
/// one, on the same paragraph. `_` additionally requires a word
/// boundary on both sides so intraword underscores (`foo_bar`) do not
/// register. The imprecision is acceptable because emphasis detection
/// is only reachable through `clap_help_markdown`'s opt-in
/// `extra_forbid` knob.
fn take_emphasis(input: &str, idx: usize) -> Option<(ConstructKind, usize)> {
    let bytes = input.as_bytes();
    let marker = bytes[idx];
    if marker == b'_' && idx > 0 && is_word_byte(bytes[idx - 1]) {
        return None;
    }
    let mut run = 0;
    while idx + run < bytes.len() && bytes[idx + run] == marker {
        run += 1;
    }
    let open = run.min(2);
    let content_start = idx + open;
    match bytes.get(content_start) {
        None => return None,
        Some(b' ' | b'\t' | b'\n') => return None,
        Some(_) => {}
    }
    let mut cursor = content_start;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\n' => {
                if cursor + 1 < bytes.len() && bytes[cursor + 1] == b'\n' {
                    return None;
                }
                cursor += 1;
            }
            byte if byte == marker => {
                let mut close_run = 0;
                while cursor + close_run < bytes.len() && bytes[cursor + close_run] == marker {
                    close_run += 1;
                }
                if close_run >= open {
                    if matches!(bytes[cursor - 1], b' ' | b'\t') {
                        return None;
                    }
                    if marker == b'_'
                        && bytes
                            .get(cursor + close_run)
                            .is_some_and(|byte| is_word_byte(*byte))
                    {
                        return None;
                    }
                    // Span through the whole closing run, which may be
                    // longer than the `open` markers we committed to
                    // (`***bold***`): the closing run is `close_run`
                    // bytes, not `open`.
                    let total = (cursor - idx) + close_run;
                    let kind = if open >= 2 {
                        ConstructKind::Bold
                    } else {
                        ConstructKind::Italic
                    };
                    return Some((kind, total));
                }
                cursor += close_run;
            }
            _ => cursor += 1,
        }
    }
    None
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests;
