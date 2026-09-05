use super::count_code_lines;

#[test]
fn blank_and_comment_only_lines_are_free() {
    let source = "\n    // setup\n    let first = 1;\n\n    /* a\n       block */\n    let second = 2; // trailing\n";
    assert_eq!(count_code_lines(source), 2);
}

#[test]
fn a_multi_line_string_counts_every_line_it_spans() {
    let source = "\n    let text = \"one\ntwo\nthree\";\n";
    assert_eq!(count_code_lines(source), 3);
}

#[test]
fn a_comment_inside_a_string_is_still_code() {
    assert_eq!(count_code_lines("\n    let url = \"http://x\";\n"), 1);
}

#[test]
fn an_empty_body_has_no_lines() {
    assert_eq!(count_code_lines(""), 0);
    assert_eq!(count_code_lines("\n    \n"), 0);
}
