//! Every Rust file in the repository closes a block at the indent it
//! opened at.
//!
//! rustfmt settles this for ordinary code, but it does not enter a
//! macro body, so a `maud::html!` template whose `{` and `}` disagree
//! compiles, renders correctly and passes every other step. It reads
//! wrong, though: a child indented level with the attributes above it
//! looks like another attribute. That shape had been written twice
//! before anything checked for it.
//!
//! Nothing here is specific to a template. A plain misindented block
//! would be caught too, and the walk covers the whole repository rather
//! than the one crate that happens to use maud.

use std::fs;
use std::path::{Path, PathBuf};
use text_block_macros::text_block;

/// Paths, relative to the repository root, the walk does not enter.
///
/// `ui/`, `ui-toml/` and `tests/fixtures/` hold code written to be
/// linted, some of it written to be wrong on purpose, so how it is laid
/// out is the fixture's business and not this check's. The rest is
/// build output and dependencies.
const UNSCANNED: &[&str] = &[
    "ui",
    "ui-toml",
    "tests/fixtures",
    "target",
    "node_modules",
    "gh-pages",
    ".git",
];

/// Where a block opened: its line number, and the indent to close at.
type Opened = (usize, usize);

/// Collect every `.rs` file under `dir` that is not [`UNSCANNED`].
fn sources(root: &Path, dir: &Path, found: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("a directory entry should be readable").path();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        if UNSCANNED
            .iter()
            .any(|skipped| relative == Path::new(skipped))
        {
            continue;
        }
        if path.is_dir() {
            sources(root, &path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// Pair each block-opening `{` with the `}` that closes it, and report
/// the pairs whose indents disagree.
///
/// Only a line *ending* in `{` opens, and only a `}` *leading* a line
/// closes, which is what keeps a brace pair written on one line out of
/// it. A line doing both — `} else {` — is read in that order.
fn misaligned(source: &str) -> Vec<String> {
    let mut open: Vec<Opened> = Vec::new();
    let mut found = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let indent = line.len() - trimmed.len();
        let closers = trimmed.len() - trimmed.trim_start_matches('}').len();
        for _ in 0..closers {
            if let Some((opened_on, opened_at)) = open.pop()
                && opened_at != indent
            {
                found.push(format!(
                    "line {opened_on} opens at column {opened_at}, \
                     but line {} closes it at column {indent}",
                    index + 1,
                ));
            }
        }
        if trimmed.ends_with('{') {
            open.push((index + 1, indent));
        }
    }
    found
}

#[test]
fn every_block_closes_where_it_opened() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = Vec::new();
    sources(root, root, &mut paths);
    paths.sort();
    assert!(
        paths.len() > 100,
        "only {} file(s) found under {} -- has the walk stopped early?",
        paths.len(),
        root.display(),
    );

    let mut report = Vec::new();
    for path in &paths {
        let source = fs::read_to_string(path).expect("a listed source should be readable");
        let shown = path.strip_prefix(root).unwrap_or(path);
        for complaint in misaligned(&source) {
            report.push(format!("{}: {complaint}", shown.display()));
        }
    }
    assert!(report.is_empty(), "{}", report.join("\n"));
}

#[test]
fn a_block_closing_where_it_opened_passes() {
    let source = text_block! {
        "fn f() {"
        "    html! {"
        "        p {"
        r#"            "text""#
        "        }"
        "    }"
        "}"
    };
    assert!(misaligned(source).is_empty());
}

#[test]
fn a_block_closing_short_of_its_opening_is_reported() {
    // The shape this exists to catch: the `{` rides the last attribute
    // line, so the child lands level with the attributes.
    let source = text_block! {
        "    p"
        r#"        role="status" {"#
        r#"        "text""#
        "    }"
    };
    let report = misaligned(source);
    assert_eq!(report.len(), 1, "{report:?}");
    assert!(report[0].contains("line 2"), "{report:?}");
    assert!(report[0].contains("line 4"), "{report:?}");
}

#[test]
fn a_line_that_closes_then_opens_is_read_in_that_order() {
    let source = text_block! {
        "if x {"
        "    a"
        "} else {"
        "    b"
        "}"
    };
    assert!(misaligned(source).is_empty());
}

#[test]
fn a_pair_written_on_one_line_is_left_alone() {
    // `dt { code { "key" } }` opens and closes within the line, so it
    // is neither pushed nor popped.
    let source = text_block! {
        "dl {"
        r#"    dt { code { "key" } }"#
        "}"
    };
    assert!(misaligned(source).is_empty());
}

#[test]
fn a_commented_brace_is_not_a_block() {
    let source = text_block! {
        "p {"
        "    // a trailing brace in prose {"
        r#"    "text""#
        "}"
    };
    assert!(misaligned(source).is_empty());
}
