use super::{Connector, parse_row, render_file_tree};

#[test]
fn parses_a_root_line_as_a_connectorless_node() {
    let row = parse_row("src/");
    assert!(row.ancestors.is_empty());
    assert_eq!(row.connector, None);
    assert_eq!(row.name, "src/");
}

#[test]
fn parses_a_top_level_branch() {
    // `└── foo/`: no ancestors, a last-child connector, name after the
    // four-column `└── ` group.
    let row = parse_row("└── foo/");
    assert!(row.ancestors.is_empty());
    assert_eq!(row.connector, Some(Connector::Ell));
    assert_eq!(row.name, "foo/");
}

#[test]
fn parses_a_nested_branch_under_a_last_child() {
    // Under a `└──` parent the column is blank (`    `), so the child
    // has one non-pipe ancestor before its own connector.
    let row = parse_row("    ├── mod.rs");
    assert_eq!(row.ancestors, vec![false]);
    assert_eq!(row.connector, Some(Connector::Tee));
    assert_eq!(row.name, "mod.rs");
}

#[test]
fn parses_a_pipe_ancestor_column() {
    // Under a `├──` parent the column keeps a vertical (`│   `), so the
    // ancestor entry is a pipe.
    let row = parse_row("│   └── bar.rs");
    assert_eq!(row.ancestors, vec![true]);
    assert_eq!(row.connector, Some(Connector::Ell));
    assert_eq!(row.name, "bar.rs");
}

#[test]
fn renders_pipe_blank_tee_and_ell_cells() {
    let html = render_file_tree("src/\n└── foo/\n    ├── mod.rs\n    └── bar.rs\n");
    // Root has just a name, no cells, and is marked flush.
    assert!(
        html.contains(r#"<div class="ft-row"><span class="ft-name ft-root">src/</span></div>"#),
    );
    // Top-level last child: one ell cell.
    assert!(
        html.contains(r#"<span class="ft-cell ft-ell"></span><span class="ft-name">foo/</span>"#),
    );
    // Nested non-last child: a blank ancestor cell then a tee.
    assert!(html.contains(
        r#"<span class="ft-cell"></span><span class="ft-cell ft-tee"></span><span class="ft-name">mod.rs</span>"#,
    ));
    assert!(html.starts_with(r#"<pre class="file-tree">"#));
    assert!(html.ends_with("</pre>\n"));
}

#[test]
fn escapes_html_metacharacters_in_names() {
    let html = render_file_tree("Vec<T> & Box<U>\n");
    assert!(html.contains("Vec&lt;T&gt; &amp; Box&lt;U&gt;"));
    assert!(!html.contains("<T>"));
}

#[test]
fn skips_blank_lines() {
    let html = render_file_tree("a/\n\n└── b\n");
    assert_eq!(html.matches(r#"class="ft-row""#).count(), 2);
}
