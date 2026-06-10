use super::{Insertion, build_insertion};

fn run(input: &str) -> String {
    let Insertion {
        start,
        end,
        replacement,
    } = build_insertion(input).expect("build_insertion returned None");
    let mut output = String::new();
    output.push_str(&input[..start]);
    output.push_str(&replacement);
    output.push_str(&input[end..]);
    output
}

#[test]
fn single_line_no_trailing_comma() {
    assert_eq!(run("allow(foo)"), r#"allow(foo, reason = "")"#);
}

#[test]
fn single_line_trailing_comma() {
    assert_eq!(run("allow(foo,)"), r#"allow(foo, reason = "",)"#);
}

#[test]
fn single_line_multiple_lints() {
    assert_eq!(run("allow(foo, bar)"), r#"allow(foo, bar, reason = "")"#);
}

#[test]
fn multi_line_trailing_comma() {
    let input = "allow(\n    foo,\n)";
    let expected = "allow(\n    foo,\n    reason = \"\",\n)";
    assert_eq!(run(input), expected);
}

#[test]
fn multi_line_no_trailing_comma() {
    let input = "allow(\n    foo\n)";
    let expected = "allow(\n    foo,\n    reason = \"\",\n)";
    assert_eq!(run(input), expected);
}

#[test]
fn multi_line_tab_indent() {
    let input = "expect(\n\tfoo,\n)";
    let expected = "expect(\n\tfoo,\n\treason = \"\",\n)";
    assert_eq!(run(input), expected);
}

/// `rfind(')')` would land inside the block comment; the
/// tokenizer-backed scan skips comment contents and finds the
/// real outer `)`.
#[test]
fn snippet_with_block_comment_containing_paren() {
    let input = "#[allow(foo) /* ) */]";
    let expected = r#"#[allow(foo, reason = "") /* ) */]"#;
    assert_eq!(run(input), expected);
}

/// Same but with a line comment after the close-paren.
#[test]
fn snippet_with_line_comment_after_args() {
    let input = "#[allow(foo) // trailing comment\n]";
    let expected = concat!(r#"#[allow(foo, reason = "") // trailing comment"#, "\n]");
    assert_eq!(run(input), expected);
}

/// Block comment *between* arguments.
#[test]
fn snippet_with_block_comment_between_args() {
    let input = "#[allow(/* ) */ foo)]";
    let expected = r#"#[allow(/* ) */ foo, reason = "")]"#;
    assert_eq!(run(input), expected);
}

/// CRLF line endings should not confuse the trailing-comma
/// detection — `\r` is treated the same as horizontal
/// whitespace when scanning the end of the last content line.
/// The inserted line uses LF; the existing source's CRLF runs
/// are preserved.
#[test]
fn multi_line_crlf_trailing_comma() {
    let input = "allow(\r\n    foo,\r\n)";
    let expected = "allow(\r\n    foo,\r\n    reason = \"\",\n)";
    assert_eq!(run(input), expected);
}

/// Empty snippets and snippets with no parens return `None`
/// so the caller falls back to the no-sugg diagnostic.
#[test]
fn empty_or_unparenthesised_snippets_yield_none() {
    assert!(build_insertion("").is_none());
    assert!(build_insertion("allow").is_none());
    assert!(build_insertion("allow(foo").is_none());
}
