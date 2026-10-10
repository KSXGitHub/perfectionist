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
//! would be caught too, and the listing covers the whole repository
//! rather than the one crate that happens to use maud.

use _utils::TempDir;
use command_extra::CommandExtra;
use core::str::FromStr;
use pipe_trait::Pipe;
use proc_macro2::{Delimiter, LexError, TokenStream, TokenTree};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use text_block_macros::text_block;

/// Paths, relative to the repository root, that are left unjudged.
///
/// These hold code written to be linted, some of it written to be wrong
/// on purpose, so how it is laid out is the fixture's business and not
/// this check's. Build output and dependencies need no entry: the
/// listing comes from git, so `.gitignore` is the one place that says
/// which directories hold them.
const UNSCANNED: &[&str] = &["ui", "ui-toml", "tests/fixtures"];

/// Where a block's braces sit: the line the `{` is on, and the line and
/// column of the `}` closing it.
struct Braces {
    opened_on: usize,
    closed_on: usize,
    closed_at: usize,
}

/// Run `git` in `root` with `args`, and give back its stdout.
fn git<Args, Arg>(root: &Path, args: Args) -> Vec<u8>
where
    Args: IntoIterator<Item = Arg>,
    Arg: AsRef<OsStr>,
{
    let output = "git"
        .pipe(Command::new)
        .with_current_dir(root)
        .with_args(args)
        .output()
        .expect("failed to invoke `git`");
    assert!(
        output.status.success(),
        "`git` failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    output.stdout
}

/// Every `.rs` file the repository at `root` counts as its own --
/// tracked, or untracked and not ignored -- minus [`UNSCANNED`].
///
/// `--cached` and `--others` each list in order, but one runs after the
/// other, so the listing they make together needs sorting to be read
/// and reported in a stable order.
fn sources(root: &Path) -> Vec<PathBuf> {
    let listing = git(
        root,
        [
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "*.rs",
        ],
    )
    .pipe(String::from_utf8)
    .expect("`git ls-files` produced non-UTF-8 output");
    let mut found: Vec<PathBuf> = listing
        .split('\0')
        .filter(|listed| !listed.is_empty())
        .map(Path::new)
        .filter(|listed| !UNSCANNED.iter().any(|skipped| listed.starts_with(skipped)))
        .map(|listed| root.join(listed))
        .collect();
    found.sort();
    found
}

/// Collect the braces of every `{ ... }` group in `stream`, the groups
/// nested inside one another included.
fn braces(stream: TokenStream, found: &mut Vec<Braces>) {
    for tree in stream {
        if let TokenTree::Group(group) = tree {
            if group.delimiter() == Delimiter::Brace {
                let open = group.span_open().start();
                let close = group.span_close().start();
                found.push(Braces {
                    opened_on: open.line,
                    closed_on: close.line,
                    closed_at: close.column,
                });
            }
            braces(group.stream(), found);
        }
    }
}

/// Report every block whose `}` sits at a different column from the
/// indent of the line its `{` is on.
///
/// The pairing comes from a parse of `source` rather than from matching
/// braces in its text, so a brace inside a string literal, a character
/// literal or a comment is not a block, and a `{` carrying a trailing
/// comment still pairs with the `}` that closes it. A block written
/// wholly on one line has no indent to disagree with, so it is left
/// alone.
fn misaligned(source: &str) -> Result<Vec<String>, LexError> {
    let mut found = Vec::new();
    braces(TokenStream::from_str(source)?, &mut found);
    let lines: Vec<&str> = source.lines().collect();
    let mut report = Vec::new();
    for block in found {
        if block.opened_on == block.closed_on {
            continue;
        }
        let opening = lines[block.opened_on - 1];
        let indent = opening.chars().count() - opening.trim_start().chars().count();
        if indent != block.closed_at {
            report.push(format!(
                "line {} opens at column {indent}, but line {} closes it at column {}",
                block.opened_on, block.closed_on, block.closed_at,
            ));
        }
    }
    Ok(report)
}

/// Run [`misaligned`] over a fixture that is expected to tokenise.
fn complaints(source: &str) -> Vec<String> {
    misaligned(source).expect("a fixture should tokenise")
}

#[test]
fn every_block_closes_where_it_opened() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let paths = sources(root);
    assert!(
        paths.len() > 100,
        "only {} file(s) listed under {} -- has the listing gone wrong?",
        paths.len(),
        root.display(),
    );

    let mut report = Vec::new();
    for path in &paths {
        let source = fs::read_to_string(path).expect("a listed source should be readable");
        let shown = path.strip_prefix(root).unwrap_or(path).display();
        match misaligned(&source) {
            Ok(found) => report.extend(found.iter().map(|entry| format!("{shown}: {entry}"))),
            Err(error) => report.push(format!("{shown}: does not tokenise: {error}")),
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
    assert!(complaints(source).is_empty());
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
    assert_eq!(
        complaints(source),
        ["line 2 opens at column 8, but line 4 closes it at column 4"],
    );
}

#[test]
fn a_block_closing_past_its_opening_is_reported() {
    // The opposite slip from the one that motivated the check: the `}`
    // lands deeper than the line its `{` is on, not short of it.
    let source = text_block! {
        "p {"
        r#"    "text""#
        "    }"
    };
    assert_eq!(
        complaints(source),
        ["line 1 opens at column 0, but line 3 closes it at column 4"],
    );
}

#[test]
fn a_misaligned_block_inside_another_is_reported_on_its_own() {
    // Only the inner block disagrees, and the pairing is per block, so
    // the outer one is not blamed for it.
    let source = text_block! {
        "div {"
        "    p {"
        r#"        "text""#
        "  }"
        "}"
    };
    assert_eq!(
        complaints(source),
        ["line 2 opens at column 4, but line 4 closes it at column 2"],
    );
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
    assert!(complaints(source).is_empty());
}

#[test]
fn a_pair_written_on_one_line_is_left_alone() {
    // `dt { code { "key" } }` opens and closes within the line, so it
    // has no indent of its own to disagree with.
    let source = text_block! {
        "dl {"
        r#"    dt { code { "key" } }"#
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn closers_sharing_a_line_are_judged_at_their_own_columns() {
    // Line 4 carries two `}`, four columns apart. The inner block
    // closes where it opened; the outer one closes at column 5, which
    // is what it is reported as -- a text scan has only the one indent
    // the line starts at to compare either against.
    let source = text_block! {
        "div {"
        "    p {"
        r#"        "text""#
        "    }}"
    };
    assert_eq!(
        complaints(source),
        ["line 1 opens at column 0, but line 4 closes it at column 5"],
    );
}

#[test]
fn a_commented_brace_is_not_a_block() {
    let source = text_block! {
        "p {"
        "    // a trailing brace in prose {"
        r#"    "text""#
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_brace_in_a_block_comment_is_not_a_block() {
    let source = text_block! {
        "fn f() {"
        "    /* a trailing brace in prose {"
        "     */"
        "    let a = 1;"
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_brace_in_a_string_literal_is_not_a_block() {
    // The literal spans lines and holds a block whose own braces
    // disagree. It is text, so none of it is a block.
    let source = text_block! {
        "fn f() {"
        r#"    let template = "p {"#
        r#"}";"#
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_brace_in_a_raw_string_is_not_a_block() {
    let source = text_block! {
        "fn f() {"
        r#"    let template = r""#
        r#"        p {"#
        "    }"
        r#"";"#
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_brace_in_a_character_literal_is_not_a_block() {
    // `'a` is a lifetime rather than a literal opening at the `'`, and
    // telling the two apart is what keeps the `{` ending line 1 in
    // code. Read as a literal instead, it would swallow that `{` and
    // the misaligned `}` would go unreported.
    let source = text_block! {
        "fn f<'a>(x: &'a str) {"
        "    let open = '{';"
        "    let close = '}';"
        "    let _ = (open, close, x);"
        "        }"
    };
    assert_eq!(
        complaints(source),
        ["line 1 opens at column 0, but line 5 closes it at column 8"],
    );
}

#[test]
fn an_open_brace_with_a_trailing_comment_still_pairs() {
    // The `{` is not the last character on its line, so a text scan
    // never records the block and blames an enclosing one for the `}`
    // that closes it.
    let source = text_block! {
        "fn f() {"
        "    if x { // note"
        "        a"
        "    }"
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_source_that_does_not_tokenise_is_reported() {
    let source = text_block! {
        "fn f() {"
        r#"    let unterminated = "text;"#
        "}"
    };
    assert!(misaligned(source).is_err());
}

#[test]
fn every_misaligned_block_is_reported_in_source_order() {
    let source = text_block! {
        "div {"
        "    p {"
        r#"        "first""#
        "  }"
        "    p {"
        r#"        "second""#
        "      }"
        "}"
    };
    assert_eq!(
        complaints(source),
        [
            "line 2 opens at column 4, but line 4 closes it at column 2",
            "line 5 opens at column 4, but line 7 closes it at column 6",
        ],
    );
}

#[test]
fn the_listing_holds_the_repository_s_own_rust_files_only() {
    let temp = TempDir::new().expect("failed to create a temp dir");
    let root = temp.path();
    for (relative, content) in [
        (".gitignore", "/built\n"),
        ("tracked.rs", "fn tracked() {}\n"),
        ("untracked.rs", "fn untracked() {}\n"),
        ("built/generated.rs", "fn generated() {}\n"),
        ("ui/fixture.rs", "fn fixture() {}\n"),
        ("tests/fixtures/fixture.rs", "fn fixture() {}\n"),
        ("notes.txt", "not rust\n"),
    ] {
        let path = root.join(relative);
        let parent = path.parent().expect("a fixture path should have a parent");
        fs::create_dir_all(parent).expect("failed to create a fixture directory");
        fs::write(&path, content).expect("failed to write a fixture");
    }
    git(root, ["init"]);
    git(root, ["add", "tracked.rs"]);

    // `built/generated.rs` is ignored, and the two fixture trees are
    // unjudged by choice; `notes.txt` is not Rust.
    assert_eq!(
        sources(root),
        [root.join("tracked.rs"), root.join("untracked.rs")],
    );
}
