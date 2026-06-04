//! Render ` ```ascii-file-tree ` fenced blocks as HTML whose tree
//! connectors are drawn with CSS borders, while the literal diagram
//! text stays in the DOM so it copies, selects, and reads exactly as
//! authored.
//!
//! The catalogue documents module layouts as `tree`-style diagrams:
//!
//! ```text
//! src/
//! └── foo/
//!     ├── mod.rs
//!     └── bar.rs
//! ```
//!
//! Rendered with the literal `│ ├ └` glyphs, whether the vertical
//! connectors join top-to-bottom is font-dependent: the glyphs only
//! tile seamlessly when the line box height matches their drawn extent,
//! which varies per font (about 1em on plain monospaces, larger on
//! terminal/Nerd fonts that pad their line metrics). No single CSS
//! `line-height` joins them everywhere — a value that touches on one
//! font gaps or overlaps on another.
//!
//! So the connectors are drawn with CSS instead. The block stays
//! a real `<pre>` of the original text — every box-drawing prefix
//! (`│   `, `    `, `├── `, `└── `) is wrapped in an inline-block
//! `<span>` whose glyphs are painted transparent (so copy, selection,
//! and assistive tech still see the original characters) and over which
//! the `.file-tree` rules in `style/base.css` draw the vertical and
//! horizontal segments as thin `::before`/`::after` pseudo-elements.
//! Each segment spans its span's full line-box
//! height, so consecutive lines' verticals meet exactly regardless of
//! font or line-height, and the rows keep comfortable spacing without
//! ever breaking the tree.

/// The fence language tag these blocks use. The unqualified token is
/// also what `markdown::lang_class_attr` would emit as a `language-...`
/// class, but file-tree blocks bypass that path entirely.
pub(crate) const FILE_TREE_LANG: &str = "ascii-file-tree";

/// Width of one indentation level, matching the four-column groups a
/// `tree`-style diagram uses (`│   `, `    `, `├── `, `└── `).
const INDENT_WIDTH: usize = 4;

/// Render an `ascii-file-tree` block to a `<pre class="file-tree">`.
///
/// The text content is preserved verbatim — concatenating the emitted
/// spans' text reproduces the input byte-for-byte — so the rendered
/// block copies, selects, and reads as the original ASCII diagram. Only
/// the box-drawing prefix of each line is wrapped in connector spans
/// for the CSS to draw over; the rest of the line (the node name) is
/// emitted as a plain visible run.
pub(crate) fn render_file_tree(code: &str) -> String {
    let mut out = String::from(r#"<pre class="file-tree"><code>"#);
    // `split_inclusive` keeps each line's trailing newline attached, so
    // re-emitting it preserves the original line breaks (and any final
    // newline) as literal, copyable text under `white-space: pre`.
    for line in code.split_inclusive('\n') {
        let (content, newline) = match line.strip_suffix('\n') {
            Some(rest) => (rest, "\n"),
            None => (line, ""),
        };
        emit_line(&mut out, content);
        out.push_str(newline);
    }
    out.push_str("</code></pre>\n");
    out
}

/// Emit one line: its box-drawing prefix as connector spans, then the
/// node name as a plain run. The prefix is consumed as exact four-column
/// tokens — `│   ` / `    ` are ancestor columns (a through-running pipe
/// or a blank), `├── ` / `└── ` are the node's own connector and end the
/// prefix. The match is deliberately exact: anything that isn't one of
/// those well-formed tokens (a root line, or a malformed/non-standard
/// diagram) ends the prefix and is emitted verbatim as the visible name,
/// rather than being swallowed four characters at a time into a
/// transparent cell where it would render invisibly.
fn emit_line(out: &mut String, content: &str) {
    let chars: Vec<char> = content.chars().collect();
    let mut index = 0;
    while let Some(group) = chars.get(index..index + INDENT_WIDTH) {
        let (class, is_connector) = match group {
            ['│', ' ', ' ', ' '] => ("ft-cell ft-pipe", false),
            [' ', ' ', ' ', ' '] => ("ft-cell", false),
            ['├', '─', '─', ' '] => ("ft-cell ft-tee", true),
            ['└', '─', '─', ' '] => ("ft-cell ft-ell", true),
            // Not a well-formed prefix token: the node name starts here.
            _ => break,
        };
        out.push_str(r#"<span class=""#);
        out.push_str(class);
        out.push_str(r#"">"#);
        push_escaped(out, group);
        out.push_str("</span>");
        index += INDENT_WIDTH;
        // `├` / `└` introduce the node itself; the name follows.
        if is_connector {
            break;
        }
    }
    let name = &chars[index..];
    if !name.is_empty() {
        out.push_str(r#"<span class="ft-name">"#);
        push_escaped(out, name);
        out.push_str("</span>");
    }
}

/// Append `chars` to `out` with the HTML metacharacters escaped. Node
/// names are author-controlled, but a stray `<`, `&`, or `>` (e.g. a
/// generics example) must not break out of the `<span>`.
fn push_escaped(out: &mut String, chars: &[char]) {
    for &ch in chars {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests;
