use super::count_code_lines;
use text_block_macros::{text_block, text_block_fnl};

#[test]
fn blank_and_comment_only_lines_are_free() {
    let source = text_block_fnl! {
        ""
        "    // setup"
        "    let first = 1;"
        ""
        "    /* a"
        "       block */"
        "    let second = 2; // trailing"
    };
    assert_eq!(count_code_lines(source), 2);
}

#[test]
fn a_doc_comment_is_free() {
    let source = text_block_fnl! {
        "/// Documented."
        "fn work() {}"
        "/** Block form."
        "    still a comment. */"
        "fn other() {}"
    };
    assert_eq!(count_code_lines(source), 2);
}

#[test]
fn a_multi_line_string_counts_every_line_it_spans() {
    let source = text_block_fnl! {
        ""
        r#"    let text = "one"#
        "two"
        r#"three";"#
    };
    assert_eq!(count_code_lines(source), 3);
}

#[test]
fn a_comment_inside_a_string_is_still_code() {
    let source = text_block_fnl! {
        ""
        r#"    let url = "http://x";"#
    };
    assert_eq!(count_code_lines(source), 1);
}

#[test]
fn an_empty_body_has_no_lines() {
    assert_eq!(count_code_lines(""), 0);
    assert_eq!(
        count_code_lines(text_block_fnl! {
            ""
            "    "
        }),
        0,
    );
}

#[test]
fn a_last_line_without_a_trailing_newline_still_counts() {
    // A one-line body: `body_interior` leaves no newline at all, so the
    // count comes entirely from the tail of `count_code_lines`.
    assert_eq!(count_code_lines(" work() "), 1);
    assert_eq!(
        count_code_lines(text_block! {
            ""
            "    first();"
            "    last()"
        }),
        2,
    );
}

#[test]
fn a_local_macro_definition_counts_its_own_lines() {
    // A `macro_rules!` written inside a body is ordinary body source:
    // its definition lines count, and the invocation costs only the
    // line it is written on -- never the lines it expands to.
    let source = text_block_fnl! {
        ""
        "    macro_rules! double {"
        "        ($x:expr) => { $x * 2 };"
        "    }"
        "    double!(2)"
    };
    assert_eq!(count_code_lines(source), 4);
    assert_eq!(
        count_code_lines(text_block_fnl! {
            ""
            "    double!(2);"
        }),
        1,
    );
}
