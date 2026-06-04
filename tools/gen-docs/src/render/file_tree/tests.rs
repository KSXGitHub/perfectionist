use super::render_file_tree;

/// Strip every `<...>` tag and undo the handful of entities the renderer
/// emits, recovering the block's text content — i.e. what a copy or a
/// screen reader would yield.
fn text_content(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    // Decode `&amp;` last: otherwise an escaped literal ampersand-entity
    // like `&amp;lt;` would first become `&lt;` and then wrongly decode
    // again to `<`, instead of copying back as the literal text `&lt;`.
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// The text a copy or screen reader would yield: the content of the
/// `<code>` wrapper, with tags stripped and entities undone.
fn copied_text(html: &str) -> String {
    let inner = html
        .strip_prefix(r#"<pre class="file-tree"><code>"#)
        .and_then(|rest| rest.strip_suffix("</code></pre>\n"))
        .expect("pre/code wrapper");
    text_content(inner)
}

const AVOID: &str = "src/\n└── foo/\n    ├── mod.rs\n    └── bar.rs\n";

#[test]
fn text_content_reproduces_the_original_diagram() {
    // The whole point: the rendered block must copy/select/read back as
    // the exact ASCII it was authored from, box-drawing glyphs and all.
    let html = render_file_tree(AVOID);
    assert_eq!(copied_text(&html), AVOID);
}

#[test]
fn wraps_each_box_drawing_group_in_a_connector_span() {
    let html = render_file_tree(AVOID);
    // Last child at top level: an ell connector carrying the literal
    // `└── `, then the visible name.
    let ell = r#"<span class="ft-cell ft-ell">└── </span><span class="ft-name">foo/</span>"#;
    assert!(html.contains(ell));
    // Nested non-last child: a blank indent column (`    `) then a tee.
    let tee = r#"<span class="ft-cell">    </span><span class="ft-cell ft-tee">├── </span><span class="ft-name">mod.rs</span>"#;
    assert!(html.contains(tee));
}

#[test]
fn marks_a_through_running_pipe_column() {
    // A `│   ` ancestor column (parent has a later sibling) becomes a
    // pipe cell carrying the literal pipe glyph.
    let html = render_file_tree("│   └── bar.rs\n");
    assert!(html.contains(r#"<span class="ft-cell ft-pipe">│   </span>"#));
}

#[test]
fn emits_a_root_line_as_a_bare_name() {
    // No box-drawing prefix: just the name, no connector cell.
    let html = render_file_tree("src/\n");
    assert!(html.contains(r#"<code><span class="ft-name">src/</span>"#));
    assert!(!html.contains("ft-cell"));
}

#[test]
fn escapes_html_metacharacters_in_names() {
    let html = render_file_tree("Vec<T> & Box<U>\n");
    assert!(html.contains("Vec&lt;T&gt; &amp; Box&lt;U&gt;"));
    assert!(!html.contains("<T>"));
    // ...and the escaping round-trips back to the original text.
    assert_eq!(copied_text(&html), "Vec<T> & Box<U>\n");
}

#[test]
fn round_trips_a_literal_entity_in_a_name() {
    // A name that is itself entity-like text (`&lt;`) must escape to
    // `&amp;lt;` and copy back as the original four characters — never
    // double-decode to `<`. Guards the entity-decode order in
    // `text_content`/the renderer.
    let html = render_file_tree("&lt;\n");
    assert!(html.contains("&amp;lt;"));
    assert_eq!(copied_text(&html), "&lt;\n");
}

#[test]
fn non_four_column_prefix_is_kept_visible_not_swallowed() {
    // Prefix tokens are matched exactly. A line whose indentation isn't a
    // well-formed `│   `/`    `/`├── `/`└── ` token must NOT have its first
    // characters consumed into a transparent `.ft-cell` (where they'd
    // render invisibly) — the whole run becomes the visible name instead.
    // Two-space indent: the leading spaces aren't a four-space column.
    let two_space = render_file_tree("  indented\n");
    assert!(!two_space.contains("ft-cell"));
    assert!(two_space.contains(r#"<span class="ft-name">  indented</span>"#));

    // A connector without the conventional `── ` separator isn't a token,
    // so it renders literally rather than hiding the following character.
    let tight = render_file_tree("└──tight\n");
    assert!(!tight.contains("ft-cell"));
    assert!(tight.contains(r#"<span class="ft-name">└──tight</span>"#));

    // A mis-sized ancestor column doesn't swallow the real connector.
    let bad_pipe = render_file_tree("│  ├── x\n");
    assert!(!bad_pipe.contains("ft-cell"));
    assert!(bad_pipe.contains(r#"<span class="ft-name">│  ├── x</span>"#));

    // In every case the text still copies back verbatim.
    assert_eq!(copied_text(&two_space), "  indented\n");
    assert_eq!(copied_text(&tight), "└──tight\n");
    assert_eq!(copied_text(&bad_pipe), "│  ├── x\n");
}
