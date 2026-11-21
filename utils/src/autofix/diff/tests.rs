//! Hold the renderer to the unified-diff format `git apply` reads.
//! Every expectation here is a diff a reader will have to review, so
//! they are written out whole rather than probed field by field.

use super::unified;
use crate::autofix::snapshot::Snapshot;
use text_block_macros::text_block_fnl;

/// A snapshot from `(path, contents)` pairs.
fn snapshot(files: &[(&str, &str)]) -> Snapshot {
    files
        .iter()
        .map(|(path, contents)| ((*path).to_owned(), (*contents).to_owned()))
        .collect()
}

/// Two snapshots that agree on every path render as nothing, which is
/// what a round that changed nothing stores.
#[test]
fn an_unchanged_tree_renders_as_nothing() {
    let tree = snapshot(&[("src/lib.rs", "pub fn f() {}\n")]);
    assert_eq!(unified(&tree, &tree), "");
}

/// One changed line carries three lines of context on each side, and
/// the hunk header counts both sides from one.
#[test]
fn a_changed_line_carries_three_lines_of_context() {
    let before = snapshot(&[(
        "src/lib.rs",
        text_block_fnl! {
            "a"
            "b"
            "c"
            "d"
            "e"
            "f"
            "g"
            "h"
            "i"
        },
    )]);
    let after = snapshot(&[(
        "src/lib.rs",
        text_block_fnl! {
            "a"
            "b"
            "c"
            "D"
            "e"
            "f"
            "g"
            "h"
            "i"
        },
    )]);
    assert_eq!(
        unified(&before, &after),
        text_block_fnl! {
            "--- a/src/lib.rs"
            "+++ b/src/lib.rs"
            "@@ -1,7 +1,7 @@"
            " a"
            " b"
            " c"
            "-d"
            "+D"
            " e"
            " f"
            " g"
        },
    );
}

/// Changes further apart than twice the context get a hunk each, so
/// the untouched middle of a file is not carried along between them.
#[test]
fn distant_changes_get_a_hunk_each() {
    let lines: Vec<String> = (1..=20).map(|number| format!("line {number}\n")).collect();
    let before = snapshot(&[("src/lib.rs", &lines.concat())]);
    let mut changed = lines.clone();
    changed[0] = "first\n".to_owned();
    changed[19] = "last\n".to_owned();
    let after = snapshot(&[("src/lib.rs", &changed.concat())]);
    let rendered = unified(&before, &after);
    assert_eq!(
        rendered.matches("@@").count(),
        4,
        "two hunks, each header carrying the marker twice; got:\n{rendered}",
    );
    assert!(
        !rendered.contains("line 10"),
        "the untouched middle should be in neither hunk; got:\n{rendered}",
    );
}

/// A path only the later snapshot holds reads as a file created
/// whole, which `/dev/null` on the left is how a unified diff spells.
#[test]
fn an_added_file_reads_as_created_from_dev_null() {
    let before = Snapshot::new();
    let after = snapshot(&[("src/new.rs", "pub fn f() {}\n")]);
    assert_eq!(
        unified(&before, &after),
        text_block_fnl! {
            "--- /dev/null"
            "+++ b/src/new.rs"
            "@@ -0,0 +1,1 @@"
            "+pub fn f() {}"
        },
    );
}

/// A path only the earlier snapshot holds reads as a file deleted
/// whole.
#[test]
fn a_removed_file_reads_as_deleted_to_dev_null() {
    let before = snapshot(&[("src/old.rs", "pub fn f() {}\n")]);
    let after = Snapshot::new();
    assert_eq!(
        unified(&before, &after),
        text_block_fnl! {
            "--- a/src/old.rs"
            "+++ /dev/null"
            "@@ -1,1 +0,0 @@"
            "-pub fn f() {}"
        },
    );
}

/// Files are rendered in ascending path order rather than in the
/// order either snapshot was read, so a diff does not move when the
/// filesystem hands its entries back differently.
#[test]
fn files_are_rendered_in_ascending_path_order() {
    let before = snapshot(&[("src/b.rs", "one\n"), ("src/a.rs", "one\n")]);
    let after = snapshot(&[("src/b.rs", "two\n"), ("src/a.rs", "two\n")]);
    let rendered = unified(&before, &after);
    let first = rendered.find("src/a.rs").expect("a.rs is in the diff");
    let second = rendered.find("src/b.rs").expect("b.rs is in the diff");
    assert!(first < second, "got:\n{rendered}");
}

/// An empty line renders as its marker alone, which is what makes the
/// `[*.diff]` section in `.editorconfig` necessary: trimming the
/// trailing space off a context line would corrupt the fixture.
#[test]
fn an_empty_context_line_renders_as_a_bare_marker() {
    let before = snapshot(&[("src/lib.rs", "a\n\nb\n")]);
    let after = snapshot(&[("src/lib.rs", "a\n\nB\n")]);
    assert_eq!(
        unified(&before, &after),
        "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n a\n \n-b\n+B\n",
    );
}
