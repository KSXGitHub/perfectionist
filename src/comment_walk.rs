//! Comment-stream walker shared by `bare_url`, `bare_email`, and
//! `bare_issue_reference`.
//!
//! Each of these rules needs to scan two distinct surfaces:
//!
//! - **Doc-comment blocks** — `///` / `//!` line runs and `/** ... */`
//!   block doc comments, each block treated as a single markdown
//!   fragment with `///` prefixes stripped.
//! - **Plain comments** — `//` line runs and `/* ... */` block
//!   comments, scanned as text (no markdown awareness).
//!
//! The walker iterates the source files of the local crate via
//! `rustc_lexer::tokenize` (the same primitive
//! `perfectionist::unicode_ellipsis_in_comments` uses) and hands each
//! comment surface to a caller-supplied callback together with the
//! offset-mapping the callback needs to anchor diagnostic spans back
//! into the source map.

use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{EarlyContext, LintContext};
use rustc_span::def_id::LOCAL_CRATE;
use rustc_span::{BytePos, Pos, RelativeBytePos, SourceFile, Span, SyntaxContext};

/// Surface kind for one chunk of comment text handed to the walker
/// callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommentSurface {
    /// `///` or `//!` doc-comment block (one or more consecutive
    /// lines). Treated as markdown by the callback.
    DocBlock,
    /// `/** ... */` block doc comment. Treated as markdown.
    DocBlockBlock,
    /// `//` plain line-comment (possibly several consecutive lines
    /// folded into one chunk). Not markdown.
    PlainLine,
    /// `/* ... */` plain block comment. Not markdown.
    PlainBlock,
}

/// One chunk of comment text plus the metadata the callback needs to
/// turn an in-chunk byte offset back into a `Span` on the source
/// file.
pub(crate) struct CommentChunk<'a> {
    pub(crate) surface: CommentSurface,
    /// Rendered text with per-line comment prefixes removed
    /// (`///`, `//!`, `//`, leading `*` for block comments, etc.).
    /// For multi-line comment runs, lines are joined with `\n` so
    /// markdown line-sensitive constructs (fenced code blocks,
    /// reference definitions) work correctly.
    pub(crate) rendered: String,
    /// One entry per line in `rendered`. Lets the callback map a
    /// byte offset inside `rendered` back to an absolute source
    /// position.
    pub(crate) lines: Vec<LineMapping>,
    /// Span of the entire comment chunk as it appears in source.
    /// `bare_issue_reference` uses its end position to append a
    /// reference-link definition to the doc block.
    pub(crate) source_span: Span,
    /// Borrowed source file the chunk lives in.
    pub(crate) source_file: &'a SourceFile,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LineMapping {
    /// Offset into `rendered` where this line's content starts.
    pub(crate) rendered_start: usize,
    /// Length of this line's content in `rendered` (excluding the
    /// trailing `\n` the walker may have inserted).
    pub(crate) rendered_len: usize,
    /// Absolute byte offset in the local source file where the
    /// content of this line starts (after the `///` prefix and
    /// optional single space).
    pub(crate) source_offset: u32,
}

impl CommentChunk<'_> {
    /// Map a byte offset within `rendered` back to the absolute
    /// source [`Span`] of `len` bytes there. Returns `None` if
    /// `rendered_offset` is past every recorded line (e.g., it
    /// indexes into a synthesised newline between lines).
    pub(crate) fn span_for(&self, rendered_offset: usize, len: u32) -> Option<Span> {
        for line in &self.lines {
            if rendered_offset >= line.rendered_start
                && rendered_offset < line.rendered_start + line.rendered_len
            {
                let delta = (rendered_offset - line.rendered_start) as u32;
                let start = self
                    .source_file
                    .absolute_position(RelativeBytePos::from_u32(line.source_offset + delta));
                let end = BytePos::from_u32(start.0 + len);
                return Some(Span::new(start, end, SyntaxContext::root(), None));
            }
        }
        None
    }
}

/// Walk every comment in the local crate's source files, handing each
/// chunk to `callback`. The callback receives a borrowed
/// [`CommentChunk`] together with the lint context's session
/// information.
pub(crate) fn walk_local_comments(
    lint_context: &EarlyContext<'_>,
    mut callback: impl FnMut(&CommentChunk<'_>),
) {
    let source_map = lint_context.sess().source_map();
    for source_file in source_map.files().iter() {
        if source_file.cnum != LOCAL_CRATE {
            continue;
        }
        let Some(source_text) = source_file.src.as_deref() else {
            continue;
        };
        walk_source_file(source_text, source_file, &mut callback);
    }
}

fn walk_source_file<'a>(
    source_text: &'a str,
    source_file: &'a SourceFile,
    callback: &mut dyn FnMut(&CommentChunk<'_>),
) {
    let mut tokens: Vec<(u32, u32, TokenKind)> = Vec::new();
    let mut offset: u32 = 0;
    for token in tokenize(source_text, FrontmatterAllowed::Yes) {
        let start = offset;
        let len = token.len;
        let end = start
            .checked_add(len)
            .expect("source-file offset overflowed");
        tokens.push((start, end, token.kind));
        offset = end;
    }

    let mut index = 0;
    while index < tokens.len() {
        let (start, end, kind) = tokens[index];
        match kind {
            TokenKind::LineComment { doc_style: Some(_) } => {
                let (chunk, consumed) =
                    gather_line_doc_comments(&tokens, index, source_text, source_file);
                callback(&chunk);
                index += consumed;
            }
            TokenKind::LineComment { doc_style: None } => {
                let (chunk, consumed) =
                    gather_line_plain_comments(&tokens, index, source_text, source_file);
                callback(&chunk);
                index += consumed;
            }
            TokenKind::BlockComment {
                doc_style: Some(_), ..
            } => {
                let chunk = build_block_doc_comment(source_text, source_file, start, end);
                callback(&chunk);
                index += 1;
            }
            TokenKind::BlockComment {
                doc_style: None, ..
            } => {
                let chunk = build_block_plain_comment(source_text, source_file, start, end);
                callback(&chunk);
                index += 1;
            }
            _ => index += 1,
        }
    }
}

/// Gather consecutive `///` / `//!` line tokens (allowing only
/// `Whitespace` newlines in between) into one logical block.
fn gather_line_doc_comments<'a>(
    tokens: &[(u32, u32, TokenKind)],
    start_idx: usize,
    source_text: &'a str,
    source_file: &'a SourceFile,
) -> (CommentChunk<'a>, usize) {
    let mut idx = start_idx;
    let initial_doc_style = match tokens[start_idx].2 {
        TokenKind::LineComment {
            doc_style: Some(style),
        } => Some(style),
        _ => None,
    };
    let mut last_doc_end = tokens[start_idx].1;
    let mut consumed = 1;
    idx += 1;
    while idx < tokens.len() {
        match tokens[idx].2 {
            TokenKind::Whitespace => {
                idx += 1;
            }
            TokenKind::LineComment { doc_style: Some(s) } if Some(s) == initial_doc_style => {
                last_doc_end = tokens[idx].1;
                idx += 1;
                consumed = idx - start_idx;
            }
            _ => break,
        }
    }

    // Build the rendered text by re-walking the source slice that
    // spans from the first doc-comment token to the last. Each
    // `///` / `//!` line contributes its post-prefix text to the
    // buffer, joined by `\n`.
    let block_start = tokens[start_idx].0;
    let block_end = last_doc_end;
    let block_src = &source_text[block_start as usize..block_end as usize];
    let (rendered, lines) = render_line_doc_block(block_src, block_start);
    let span = Span::new(
        source_file.absolute_position(RelativeBytePos::from_u32(block_start)),
        source_file.absolute_position(RelativeBytePos::from_u32(block_end)),
        SyntaxContext::root(),
        None,
    );
    let chunk = CommentChunk {
        surface: CommentSurface::DocBlock,
        rendered,
        lines,
        source_span: span,
        source_file,
    };
    (chunk, consumed)
}

/// Gather consecutive `//` plain-comment tokens into one chunk.
fn gather_line_plain_comments<'a>(
    tokens: &[(u32, u32, TokenKind)],
    start_idx: usize,
    source_text: &'a str,
    source_file: &'a SourceFile,
) -> (CommentChunk<'a>, usize) {
    let mut idx = start_idx + 1;
    let mut last_end = tokens[start_idx].1;
    let mut consumed = 1;
    while idx < tokens.len() {
        match tokens[idx].2 {
            TokenKind::Whitespace => idx += 1,
            TokenKind::LineComment { doc_style: None } => {
                last_end = tokens[idx].1;
                idx += 1;
                consumed = idx - start_idx;
            }
            _ => break,
        }
    }
    let block_start = tokens[start_idx].0;
    let block_end = last_end;
    let block_src = &source_text[block_start as usize..block_end as usize];
    let (rendered, lines) = render_line_plain_block(block_src, block_start);
    let span = Span::new(
        source_file.absolute_position(RelativeBytePos::from_u32(block_start)),
        source_file.absolute_position(RelativeBytePos::from_u32(block_end)),
        SyntaxContext::root(),
        None,
    );
    let chunk = CommentChunk {
        surface: CommentSurface::PlainLine,
        rendered,
        lines,
        source_span: span,
        source_file,
    };
    (chunk, consumed)
}

/// Build a [`CommentChunk`] for a `/** ... */` (outer) or
/// `/*! ... */` (inner) block doc comment. The two share every
/// downstream concern but differ in their three-byte opening
/// delimiter — and the opening byte length is what anchors every
/// per-line source offset, so picking the wrong delimiter
/// silently misaligns diagnostic spans.
fn build_block_doc_comment<'a>(
    source_text: &'a str,
    source_file: &'a SourceFile,
    start: u32,
    end: u32,
) -> CommentChunk<'a> {
    let body_text = &source_text[start as usize..end as usize];
    let open = if body_text.starts_with("/*!") {
        "/*!"
    } else {
        "/**"
    };
    let (rendered, lines) = render_block_comment(body_text, start, open, "*/");
    let span = Span::new(
        source_file.absolute_position(RelativeBytePos::from_u32(start)),
        source_file.absolute_position(RelativeBytePos::from_u32(end)),
        SyntaxContext::root(),
        None,
    );
    CommentChunk {
        surface: CommentSurface::DocBlockBlock,
        rendered,
        lines,
        source_span: span,
        source_file,
    }
}

/// Build a [`CommentChunk`] for a `/* ... */` plain block comment.
fn build_block_plain_comment<'a>(
    source_text: &'a str,
    source_file: &'a SourceFile,
    start: u32,
    end: u32,
) -> CommentChunk<'a> {
    let body_text = &source_text[start as usize..end as usize];
    let (rendered, lines) = render_block_comment(body_text, start, "/*", "*/");
    let span = Span::new(
        source_file.absolute_position(RelativeBytePos::from_u32(start)),
        source_file.absolute_position(RelativeBytePos::from_u32(end)),
        SyntaxContext::root(),
        None,
    );
    CommentChunk {
        surface: CommentSurface::PlainBlock,
        rendered,
        lines,
        source_span: span,
        source_file,
    }
}

/// Render a `///` / `//!` doc-comment block into a buffer of joined
/// lines. Strips the `///` (or `////` / `//!`) prefix plus one
/// optional space from each line, mirroring what rustdoc does when it
/// turns line doc-comments into the `#[doc = "..."]` attribute text.
fn render_line_doc_block(block_src: &str, block_source_start: u32) -> (String, Vec<LineMapping>) {
    let mut rendered = String::with_capacity(block_src.len());
    let mut lines: Vec<LineMapping> = Vec::new();
    let mut offset_in_block: u32 = 0;
    for raw_line in block_src.split_inclusive('\n') {
        let has_newline = raw_line.ends_with('\n');
        let line_content = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        // Drop trailing `\r` for `\r\n` line endings.
        let line_content = line_content.strip_suffix('\r').unwrap_or(line_content);

        let bytes = line_content.as_bytes();
        // A continuation line inside an indented item (`impl`, `mod`,
        // nested `fn`, ...) carries the indentation before its `///`,
        // so skip leading ASCII whitespace before looking for `//`.
        let indent = bytes
            .iter()
            .take_while(|&&byte| byte == b' ' || byte == b'\t')
            .count();
        if !bytes[indent..].starts_with(b"//") {
            // Whitespace between doc-comment tokens; skip without
            // emitting a line.
            offset_in_block += raw_line.len() as u32;
            continue;
        }
        // Past the `//` lies `/`, `!`, or `/<text>` for `////...`.
        let mut prefix_end = indent + 2;
        // `///`, `//!`, or `////...`
        if prefix_end < bytes.len() && (bytes[prefix_end] == b'/' || bytes[prefix_end] == b'!') {
            prefix_end += 1;
        }
        // For `////...` (extra slashes), CommonMark / rustdoc both
        // strip only `///`; leave the additional slash in content.
        // Strip one optional space.
        let mut content_start = prefix_end;
        if content_start < bytes.len() && bytes[content_start] == b' ' {
            content_start += 1;
        }
        let content = &line_content[content_start..];
        let rendered_start = rendered.len();
        rendered.push_str(content);
        let line_source_offset = block_source_start + offset_in_block + content_start as u32;
        lines.push(LineMapping {
            rendered_start,
            rendered_len: content.len(),
            source_offset: line_source_offset,
        });
        if has_newline {
            rendered.push('\n');
        }
        offset_in_block += raw_line.len() as u32;
    }
    (rendered, lines)
}

/// Render a `//` plain-line-comment block. Same shape as
/// [`render_line_doc_block`] but strips `//` (and one optional space)
/// rather than `///` / `//!`.
fn render_line_plain_block(block_src: &str, block_source_start: u32) -> (String, Vec<LineMapping>) {
    let mut rendered = String::with_capacity(block_src.len());
    let mut lines: Vec<LineMapping> = Vec::new();
    let mut offset_in_block: u32 = 0;
    for raw_line in block_src.split_inclusive('\n') {
        let has_newline = raw_line.ends_with('\n');
        let line_content = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let line_content = line_content.strip_suffix('\r').unwrap_or(line_content);
        let bytes = line_content.as_bytes();
        // Skip leading indentation on continuation lines (see
        // `render_line_doc_block`).
        let indent = bytes
            .iter()
            .take_while(|&&byte| byte == b' ' || byte == b'\t')
            .count();
        if !bytes[indent..].starts_with(b"//") {
            offset_in_block += raw_line.len() as u32;
            continue;
        }
        let mut content_start = indent + 2;
        if content_start < bytes.len() && bytes[content_start] == b' ' {
            content_start += 1;
        }
        let content = &line_content[content_start..];
        let rendered_start = rendered.len();
        rendered.push_str(content);
        let line_source_offset = block_source_start + offset_in_block + content_start as u32;
        lines.push(LineMapping {
            rendered_start,
            rendered_len: content.len(),
            source_offset: line_source_offset,
        });
        if has_newline {
            rendered.push('\n');
        }
        offset_in_block += raw_line.len() as u32;
    }
    (rendered, lines)
}

/// Render a block comment. Strips the configured opening and closing
/// delimiters and, for the inner lines, an optional leading `*`
/// followed by one space.
fn render_block_comment(
    body_text: &str,
    block_source_start: u32,
    open: &str,
    close: &str,
) -> (String, Vec<LineMapping>) {
    let body = body_text
        .strip_prefix(open)
        .and_then(|inner| inner.strip_suffix(close))
        .unwrap_or(body_text);
    let prefix_len = open.len() as u32;
    let mut rendered = String::with_capacity(body.len());
    let mut lines: Vec<LineMapping> = Vec::new();
    let mut offset_in_body: u32 = 0;
    for raw_line in body.split_inclusive('\n') {
        let has_newline = raw_line.ends_with('\n');
        let line_content = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let line_content = line_content.strip_suffix('\r').unwrap_or(line_content);
        // For block comments, strip a leading run of whitespace
        // followed by a single `*` and an optional space — the
        // standard rustdoc-style block-comment continuation
        // prefix. When the leading non-space byte isn't `*`, the
        // line wasn't using the convention, so we keep its
        // original content unmodified (no whitespace stripping).
        // Applies to every line, not just the first.
        let bytes = line_content.as_bytes();
        let mut content_start: usize = 0;
        while content_start < bytes.len()
            && (bytes[content_start] == b' ' || bytes[content_start] == b'\t')
        {
            content_start += 1;
        }
        if content_start < bytes.len() && bytes[content_start] == b'*' {
            content_start += 1;
            if content_start < bytes.len() && bytes[content_start] == b' ' {
                content_start += 1;
            }
        } else {
            content_start = 0;
        }
        let content = &line_content[content_start..];
        let rendered_start = rendered.len();
        rendered.push_str(content);
        let line_source_offset =
            block_source_start + prefix_len + offset_in_body + content_start as u32;
        lines.push(LineMapping {
            rendered_start,
            rendered_len: content.len(),
            source_offset: line_source_offset,
        });
        if has_newline {
            rendered.push('\n');
        }
        offset_in_body += raw_line.len() as u32;
    }
    (rendered, lines)
}
