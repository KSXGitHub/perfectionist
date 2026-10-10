//! Every block written inside a macro body closes at the indent it
//! opened at.
//!
//! rustfmt settles this everywhere else, but it does not enter a macro
//! body, so a `maud::html!` template whose `{` and `}` disagree
//! compiles, renders correctly and passes every other step. It reads
//! wrong, though: a child indented level with the attributes above it
//! looks like another attribute. That shape had been written twice
//! before anything checked for it.
//!
//! The boundary is where rustfmt stops, and it is narrower than "a
//! macro". rustfmt reformats an invocation delimited by `(` or `[`
//! whole -- `vec![..]` and `assert_eq!(..)` are rewritten, misaligned
//! braces and all -- and enters a `{`-delimited one not at all. Even
//! there it places the body's own two braces, moving the closing one
//! to the indent of the line its `{` is on. What is left over, and all
//! that is judged here, is the relative indent of the blocks written
//! inside a `{`-delimited body. Ordinary code is rustfmt's throughout,
//! and `just all` runs `cargo fmt -- --check` before this test.
//!
//! Nothing here is specific to maud, or to the one crate that uses it.
//! Any macro body in any crate is covered, which is why the listing is
//! the whole repository.

use _utils::TempDir;
use command_extra::CommandExtra;
use core::str::FromStr;
use pipe_trait::Pipe;
use proc_macro2::{Delimiter, LexError, Spacing, TokenStream, TokenTree};
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
fn git_capture<Args, Arg>(root: &Path, args: Args) -> Vec<u8>
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
    let listing = git_capture(
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

/// Whether the group at `index` is a macro invocation's arguments: a
/// `!` directly before it, with an identifier before that. Requiring
/// the `!` to stand alone is what keeps the `!=` of `a != b` out.
fn is_invocation(trees: &[TokenTree], index: usize) -> bool {
    let bang = index.checked_sub(1).and_then(|at| trees.get(at));
    let name = index.checked_sub(2).and_then(|at| trees.get(at));
    let banged = matches!(
        bang,
        Some(TokenTree::Punct(punct))
            if punct.as_char() == '!' && punct.spacing() == Spacing::Alone
    );
    banged && matches!(name, Some(TokenTree::Ident(_)))
}

/// Collect the braces of every `{ ... }` block written inside a macro
/// body, the blocks nested inside one another included.
///
/// `in_macro` says whether `stream` is itself a macro body or sits
/// within one. A body's own braces are reached with it still false, so
/// they go unrecorded: rustfmt places those. Only a `{`-delimited
/// invocation opens a body, because rustfmt reformats a `(`- or
/// `[`-delimited one for itself.
fn braces(stream: TokenStream, in_macro: bool, found: &mut Vec<Braces>) {
    let trees: Vec<TokenTree> = stream.into_iter().collect();
    for (index, tree) in trees.iter().enumerate() {
        let TokenTree::Group(group) = tree else {
            continue;
        };
        if in_macro && group.delimiter() == Delimiter::Brace {
            let open = group.span_open().start();
            let close = group.span_close().start();
            found.push(Braces {
                opened_on: open.line,
                closed_on: close.line,
                closed_at: close.column,
            });
        }
        let opens_body = group.delimiter() == Delimiter::Brace && is_invocation(&trees, index);
        braces(group.stream(), in_macro || opens_body, found);
    }
}

/// Report every block inside a macro body whose `}` sits at a different
/// column from the indent of the line its `{` is on.
///
/// The pairing comes from a parse of `source` rather than from matching
/// braces in its text, so a brace inside a string literal, a character
/// literal or a comment is not a block, and a `{` carrying a trailing
/// comment still pairs with the `}` that closes it. A block written
/// wholly on one line has no indent to disagree with, so it is left
/// alone.
fn misaligned(source: &str) -> Result<Vec<String>, LexError> {
    let mut found = Vec::new();
    braces(TokenStream::from_str(source)?, false, &mut found);
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
fn every_macro_body_block_closes_where_it_opened() {
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
        "html! {"
        "    p"
        r#"        role="status" {"#
        r#"        "text""#
        "    }"
        "}"
    };
    assert_eq!(
        complaints(source),
        ["line 3 opens at column 8, but line 5 closes it at column 4"],
    );
}

#[test]
fn a_block_closing_past_its_opening_is_reported() {
    // A `}` deeper than the line its `{` is on is as misindented as one
    // short of it, so both directions are reported.
    let source = text_block! {
        "html! {"
        "    p {"
        r#"        "text""#
        "        }"
        "}"
    };
    assert_eq!(
        complaints(source),
        ["line 2 opens at column 4, but line 4 closes it at column 8"],
    );
}

#[test]
fn a_block_outside_a_macro_body_is_left_to_rustfmt() {
    // rustfmt aligns this one, and `just fmt` runs before this test, so
    // judging it here would be a second opinion on a settled question.
    let source = text_block! {
        "fn f() {"
        "    if x {"
        "        a"
        "        }"
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_macro_body_s_own_braces_are_left_to_rustfmt() {
    // rustfmt moves the `}` closing a macro body to the indent of the
    // line its `{` is on, so this misalignment is one it fixes. The `p`
    // block inside closes where it opened and is the only one judged.
    let source = text_block! {
        "fn f() {"
        "    let markup = html! {"
        "        p {"
        r#"            "text""#
        "        }"
        "        };"
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_macro_body_nested_in_ordinary_code_is_covered() {
    // The body sits inside a `fn`, which is not a macro body, so
    // reaching it means descending through ordinary code first.
    let source = text_block! {
        "fn f() {"
        "    let markup = html! {"
        "        p {"
        r#"            "text""#
        "          }"
        "    };"
        "}"
    };
    assert_eq!(
        complaints(source),
        ["line 3 opens at column 8, but line 5 closes it at column 10"],
    );
}

#[test]
fn a_block_inside_a_bracket_macro_is_left_to_rustfmt() {
    // rustfmt rewrites a `(`- or `[`-delimited invocation whole: this
    // `vec![..]` comes back as `vec![Foo { a: 1 }]`, so the misaligned
    // `}` is its to fix and never reaches here.
    let source = text_block! {
        "fn f() {"
        "    let v = vec!["
        "        Foo {"
        "            a: 1,"
        "            },"
        "    ];"
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_bang_without_an_identifier_is_not_a_macro() {
    // `!{ ... }` negates a block, and `#![...]` carries no name
    // either, so neither opens a macro body. Taking the `!` alone as
    // the signal would read both as one and judge what they enclose.
    let source = text_block! {
        "fn f() {"
        "    let flag = !{"
        "        if x {"
        "            a"
        "            }"
        "    };"
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_misaligned_block_inside_another_is_reported_on_its_own() {
    // Only the inner block disagrees, and the pairing is per block, so
    // the outer one is not blamed for it.
    let source = text_block! {
        "html! {"
        "    div {"
        "        p {"
        r#"            "text""#
        "      }"
        "    }"
        "}"
    };
    assert_eq!(
        complaints(source),
        ["line 3 opens at column 8, but line 5 closes it at column 6"],
    );
}

#[test]
fn every_misaligned_block_is_reported_in_source_order() {
    let source = text_block! {
        "html! {"
        "    div {"
        "        p {"
        r#"            "first""#
        "      }"
        "        p {"
        r#"            "second""#
        "          }"
        "    }"
        "}"
    };
    assert_eq!(
        complaints(source),
        [
            "line 3 opens at column 8, but line 5 closes it at column 6",
            "line 6 opens at column 8, but line 8 closes it at column 10",
        ]
    );
}

#[test]
fn a_line_that_closes_then_opens_is_read_in_that_order() {
    let source = text_block! {
        "html! {"
        "    @if x {"
        r#"        "a""#
        "    } @else {"
        r#"        "b""#
        "    }"
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_pair_written_on_one_line_is_left_alone() {
    // `dt { code { "key" } }` opens and closes within the line, so it
    // has no indent of its own to disagree with.
    let source = text_block! {
        "html! {"
        "    dl {"
        r#"        dt { code { "key" } }"#
        "    }"
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn closers_sharing_a_line_are_judged_at_their_own_columns() {
    // Line 5 carries two `}`, one column apart. The inner block closes
    // where it opened; the outer one closes at column 9, which is what
    // it is reported as -- a text scan has only the one indent the line
    // starts at to compare either against.
    let source = text_block! {
        "html! {"
        "    div {"
        "        p {"
        r#"            "text""#
        "        }}"
        "}"
    };
    assert_eq!(
        complaints(source),
        ["line 2 opens at column 4, but line 5 closes it at column 9"],
    );
}

#[test]
fn a_commented_brace_is_not_a_block() {
    let source = text_block! {
        "html! {"
        "    p {"
        "        // a trailing brace in prose {"
        r#"        "text""#
        "    }"
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_brace_in_a_block_comment_is_not_a_block() {
    let source = text_block! {
        "html! {"
        "    p {"
        "        /* a trailing brace in prose {"
        "         */"
        r#"        "text""#
        "    }"
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_brace_in_a_string_literal_is_not_a_block() {
    // The literal spans lines and holds a block whose own braces
    // disagree. It is text, so none of it is a block.
    let source = text_block! {
        "html! {"
        "    p {"
        r#"        "a template p {"#
        r#"}""#
        "    }"
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_brace_in_a_raw_string_is_not_a_block() {
    let source = text_block! {
        "html! {"
        "    p {"
        r#"        r""#
        "        p {"
        "    }"
        r#"""#
        "    }"
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_brace_in_a_character_literal_is_not_a_block() {
    // `'a` is a lifetime rather than a literal opening at the `'`, and
    // telling the two apart is what keeps the `{` ending line 2 in
    // code. Read as a literal instead, it would swallow that `{` and
    // the misaligned `}` would go unreported.
    let source = text_block! {
        "quote! {"
        "    fn f<'a>(x: &'a str) {"
        "        let open = '{';"
        "        let close = '}';"
        "        let _ = (open, close, x);"
        "        }"
        "}"
    };
    assert_eq!(
        complaints(source),
        ["line 2 opens at column 4, but line 6 closes it at column 8"],
    );
}

#[test]
fn an_open_brace_with_a_trailing_comment_still_pairs() {
    // The `{` is not the last character on its line, so a text scan
    // never records the block and blames an enclosing one for the `}`
    // that closes it.
    let source = text_block! {
        "html! {"
        "    p {"
        "        @if x { // note"
        r#"            "a""#
        "        }"
        r#"        "b""#
        "    }"
        "}"
    };
    assert!(complaints(source).is_empty());
}

#[test]
fn a_source_that_does_not_tokenise_is_reported() {
    let source = text_block! {
        "html! {"
        r#"    p { "unterminated"#
        "}"
    };
    assert!(misaligned(source).is_err());
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
    git_capture(root, ["init"]);
    git_capture(root, ["add", "tracked.rs"]);

    // `built/generated.rs` is ignored, and the two fixture trees are
    // unjudged by choice; `notes.txt` is not Rust.
    assert_eq!(
        sources(root),
        [root.join("tracked.rs"), root.join("untracked.rs")],
    );
}
