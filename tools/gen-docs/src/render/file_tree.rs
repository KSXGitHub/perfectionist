//! Render ` ```ascii-file-tree ` fenced blocks as structural HTML whose
//! tree connectors are drawn with CSS borders instead of box-drawing
//! glyphs.
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
//! which varies per font (≈1em on plain monospaces, larger on
//! terminal/Nerd fonts that pad their line metrics). No single CSS
//! `line-height` joins them everywhere — a value that touches on one
//! font gaps or overlaps on another.
//!
//! So this module throws the glyphs away. It parses the diagram into
//! rows of (indentation, connector, name) and emits one `<div>` per
//! row whose connector cells carry CSS-drawn vertical and horizontal
//! line segments (see the `.file-tree` rules in `style/base.css`).
//! Each segment spans its cell's full row height, so consecutive rows'
//! verticals meet exactly regardless of font or line-height, and the
//! rows can keep comfortable spacing without ever breaking the tree.

/// The fence language tag these blocks use. The unqualified token is
/// also what `markdown::lang_class_attr` would emit as a `language-...`
/// class, but file-tree blocks bypass that path entirely.
pub(crate) const FILE_TREE_LANG: &str = "ascii-file-tree";

/// Width of one indentation level, matching the four-column groups a
/// `tree`-style diagram uses (`│   `, `    `, `├── `, `└── `).
const INDENT_WIDTH: usize = 4;

/// The connector glyph that introduces a node.
#[derive(Debug, PartialEq, Eq)]
enum Connector {
    /// `├──`: a node with at least one following sibling. Its vertical
    /// runs the full height of the row so it joins the sibling below.
    Tee,
    /// `└──`: the last node at its level. Its vertical stops halfway
    /// down, where the horizontal arm turns off to the name.
    Ell,
}

/// One line of the diagram, decomposed into its drawable parts.
struct Row {
    /// One entry per ancestor level, left to right: `true` where a
    /// vertical line passes through (the ancestor has a later sibling,
    /// drawn `│   `), `false` where the column is blank (`    `).
    ancestors: Vec<bool>,
    /// The node's own connector, or `None` for the root line (which has
    /// no incoming branch).
    connector: Option<Connector>,
    /// The node's label — everything after the connector.
    name: String,
}

/// Parse one diagram line into a [`Row`]. The prefix is consumed in
/// four-column groups: `│   ` / `    ` are ancestor columns, `├── ` /
/// `└── ` is the node's connector and ends the prefix. A line with no
/// box-drawing prefix is a root node (`connector: None`).
fn parse_row(line: &str) -> Row {
    let chars: Vec<char> = line.chars().collect();
    let mut index = 0;
    let mut ancestors = Vec::new();
    let mut connector = None;
    loop {
        match chars.get(index) {
            Some('│') => ancestors.push(true),
            Some(' ') => ancestors.push(false),
            Some('├') => {
                connector = Some(Connector::Tee);
                index += INDENT_WIDTH;
                break;
            }
            Some('└') => {
                connector = Some(Connector::Ell);
                index += INDENT_WIDTH;
                break;
            }
            // A non-space, non-box-drawing character (or end of line):
            // the node name starts here, with no connector — the root.
            _ => break,
        }
        index += INDENT_WIDTH;
    }
    // `index` can overshoot a malformed prefix; clamp to an empty name.
    let name: String = chars.get(index..).unwrap_or(&[]).iter().collect();
    Row {
        ancestors,
        connector,
        name,
    }
}

/// Render an `ascii-file-tree` block to a `<pre class="file-tree">`
/// whose rows draw their connectors with CSS borders. Reusing `<pre>`
/// inherits the themed code-block box (background, padding, radius)
/// from the existing stylesheets; `white-space: normal` on the class
/// keeps the inter-tag newlines below from showing as blank lines.
pub(crate) fn render_file_tree(code: &str) -> String {
    let mut out = String::from(r#"<pre class="file-tree">"#);
    for line in code.lines() {
        // Skip wholly blank lines: they carry no node and would render
        // as an empty row.
        if line.trim().is_empty() {
            continue;
        }
        let row = parse_row(line);
        out.push_str(r#"<div class="ft-row">"#);
        for &pipe in &row.ancestors {
            if pipe {
                out.push_str(r#"<span class="ft-cell ft-pipe"></span>"#);
            } else {
                out.push_str(r#"<span class="ft-cell"></span>"#);
            }
        }
        // A connectorless row is a root: its name sits flush, with no
        // arm to leave room for (the `ft-root` class drops the padding).
        let name_class = match row.connector {
            Some(Connector::Tee) => {
                out.push_str(r#"<span class="ft-cell ft-tee"></span>"#);
                "ft-name"
            }
            Some(Connector::Ell) => {
                out.push_str(r#"<span class="ft-cell ft-ell"></span>"#);
                "ft-name"
            }
            None => "ft-name ft-root",
        };
        out.push_str(r#"<span class=""#);
        out.push_str(name_class);
        out.push_str(r#"">"#);
        push_escaped(&mut out, &row.name);
        out.push_str("</span></div>");
    }
    out.push_str("</pre>\n");
    out
}

/// Append `text` to `out` with the HTML metacharacters escaped. Node
/// names are author-controlled, but a stray `<`, `&`, or `>` (e.g. a
/// generics example) must not break out of the `<span>`.
fn push_escaped(out: &mut String, text: &str) {
    for ch in text.chars() {
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
