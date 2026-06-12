use super::{build_reason_insertion, escape_for_rust_string};

fn run_insertion(input: &str, escaped: &str) -> String {
    let insertion =
        build_reason_insertion(input, escaped).expect("build_reason_insertion returned None");
    let mut output = String::new();
    output.push_str(&input[..insertion.start]);
    output.push_str(&insertion.replacement);
    output.push_str(&input[insertion.end..]);
    output
}

#[test]
fn insertion_single_line_no_trailing_comma() {
    assert_eq!(
        run_insertion("allow(foo)", "why"),
        r#"allow(foo, reason = "why")"#,
    );
}

#[test]
fn insertion_single_line_trailing_comma() {
    assert_eq!(
        run_insertion("allow(foo,)", "why"),
        r#"allow(foo, reason = "why",)"#,
    );
}

#[test]
fn insertion_multi_line_trailing_comma() {
    let input = "allow(\n    foo,\n)";
    let expected = "allow(\n    foo,\n    reason = \"why\",\n)";
    assert_eq!(run_insertion(input, "why"), expected);
}

/// Wrapped argument list whose closing `)` rides the last
/// argument's line. The `)` is not on its own line, so the
/// inline strategy applies — splice before `)` rather than
/// emitting a stray unindented `reason` line.
#[test]
fn insertion_wrapped_close_paren_on_last_arg_line() {
    assert_eq!(
        run_insertion("allow(\n    foo)", "why"),
        "allow(\n    foo, reason = \"why\")",
    );
}

#[test]
fn insertion_multi_line_no_trailing_comma() {
    let input = "allow(\n    foo\n)";
    let expected = "allow(\n    foo,\n    reason = \"why\",\n)";
    assert_eq!(run_insertion(input, "why"), expected);
}

#[test]
fn insertion_escapes_quotes_and_backslashes() {
    assert_eq!(
        run_insertion("allow(foo)", r#"he said \"yes\""#),
        r#"allow(foo, reason = "he said \"yes\"")"#,
    );
}

#[test]
fn escape_passes_through_unicode() {
    assert_eq!(escape_for_rust_string("café"), "café");
}

#[test]
fn escape_handles_quotes_backslash_and_controls() {
    assert_eq!(escape_for_rust_string("a\\b\"c\nd\te"), r#"a\\b\"c\nd\te"#);
    assert_eq!(escape_for_rust_string("\x01"), r"\u{1}");
    assert_eq!(escape_for_rust_string("\x7f"), r"\u{7f}");
}
