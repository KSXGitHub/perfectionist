//! What makes a character visible enough that spelling it beats
//! spelling its escape.
//!
//! The test is terminal display width, which already separates the
//! three families of character an escape serves better than a glyph
//! would: a control character has no width at all, a combining mark
//! and a zero-width or format character measure zero columns, and
//! everything a reader can actually see measures one or two. Two
//! families measure a column and still want their escape, so they are
//! excluded by name — whitespace, where the escape is what says which
//! space it is, and the Private Use Areas, whose codepoints render as
//! a replacement box outside the one font that assigns them a glyph.
//!
//! An unassigned codepoint measures one column and so is flagged.
//! Detecting one needs the general-category table that the width
//! measurement does not expose, and a literal that holds one is not a
//! shape worth carrying that table for.

use unicode_width::UnicodeWidthChar;

/// Whether `character` has a glyph of its own that a reader would
/// recognise in source, in the sense the module doc describes.
pub(super) fn is_visible(character: char) -> bool {
    character.width().is_some_and(|width| width > 0)
        && !character.is_whitespace()
        && !is_private_use(character)
}

/// Whether `character` falls in one of Unicode's three Private Use
/// Areas: the Basic Multilingual Plane's, and the two supplementary
/// planes that are nothing else. What a codepoint there looks like is
/// a private agreement between producer and consumer — an icon font's
/// glyph for one editor, a replacement box everywhere else — so the
/// escape is the only spelling that survives being read.
fn is_private_use(character: char) -> bool {
    matches!(
        u32::from(character),
        0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD,
    )
}

#[cfg(test)]
mod tests;
