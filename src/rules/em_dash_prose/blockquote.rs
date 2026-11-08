//! Finding the block-level quotations in a doc comment.
//!
//! A doc comment is markdown, where a run of `>` lines is a
//! blockquote: text the author is reproducing rather than writing, so
//! its dashes are no more theirs to restructure than the ones inside a
//! quotation mark. A plain `//` comment is not markdown and gets no
//! such reading, a leading `>` there being as likely a shell prompt or
//! an arrow.
//!
//! One shape carrying the `>` marker is not a quotation. A block
//! opening `[!NOTE]`, `[!WARNING]` or any other bracketed-bang name is
//! an alert: an aside in the author's own voice that borrows the
//! blockquote syntax to render as a callout. Those are scanned like
//! any other prose, which is the point of recognising them separately.

use crate::markdown::SkipRange;

/// Take a blockquote marker from the front of `line`, handing back the
/// quoted text after it.
///
/// Up to three leading spaces may precede the `>`, as CommonMark
/// allows, and one space after it belongs to the marker rather than to
/// the quotation.
fn take_quote_marker(line: &str) -> Option<&str> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let quoted = line[indent..].strip_prefix('>')?;
    Some(quoted.strip_prefix(' ').unwrap_or(quoted))
}

/// Whether a blockquote's first line marks the block as an alert
/// rather than a quotation.
///
/// The bracketed-bang shape is matched rather than a list of names:
/// whichever name a renderer happens to recognise, an author who wrote
/// one meant a callout in their own voice.
fn opens_alert(first_line: &str) -> bool {
    let Some(after_marker) = first_line.trim_start().strip_prefix("[!") else {
        return false;
    };
    let Some((name, _rest)) = after_marker.split_once(']') else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphabetic())
}

/// The byte ranges of the blockquotes in `rendered`, in source order,
/// leaving the alerts out.
///
/// A block runs from its first `>` line to the last line before a
/// blank one, so a `>`-less line of a quoted paragraph is covered too:
/// markdown reads that as part of the quotation, and the `>` on the
/// line above is what the author wrote to say so.
pub(super) fn scan_block_quotations(rendered: &str) -> Vec<SkipRange> {
    let mut quotations: Vec<SkipRange> = Vec::new();
    let mut block: Option<(usize, usize, bool)> = None;
    let mut offset = 0;
    for line in rendered.split_inclusive('\n') {
        let line_end = offset + line.len();
        let content = line.trim_end();
        let marker = take_quote_marker(content);
        match block {
            Some((start, _end, alert)) if marker.is_some() || !content.is_empty() => {
                block = Some((start, line_end, alert));
            }
            Some((start, end, alert)) => {
                if !alert {
                    quotations.push(start..end);
                }
                block = None;
            }
            None => {
                block = marker.map(|quoted| (offset, line_end, opens_alert(quoted)));
            }
        }
        offset = line_end;
    }
    if let Some((start, end, false)) = block {
        quotations.push(start..end);
    }
    quotations
}

#[cfg(test)]
mod tests;
