use splittable_adapter_closure_autofix::*;

#[test]
fn three_tests_keeps_the_lines_passing_all_three() {
    let lines = vec!["#a!", "#b", "c!", "", "#c!"];
    assert_eq!(three_tests(lines.into_iter()), vec!["#a!", "#c!"]);
}

#[test]
fn holds_a_disjunction_keeps_either_alternative() {
    let lines = vec!["#x", "y!", "", "z"];
    assert_eq!(holds_a_disjunction(lines.into_iter()), vec!["#x", "y!"]);
}

#[test]
fn moves_a_capture_wants_one_line_passing_both() {
    let both = vec!["ab", "abcdefgh", "abcd"];
    assert!(moves_a_capture(3, both.into_iter()));
    let one_each = vec!["ab", "abcdefgh"];
    assert!(!moves_a_capture(3, one_each.into_iter()));
}

#[test]
fn takes_a_prefix_stops_at_the_first_failing_line() {
    let stopped_by_the_second_test = vec!["#a", "#b", "c", "#d"];
    assert_eq!(
        takes_a_prefix(stopped_by_the_second_test.into_iter()),
        vec!["#a", "#b"],
    );
    let stopped_by_the_first_test = vec!["#a", "", "#c"];
    assert_eq!(
        takes_a_prefix(stopped_by_the_first_test.into_iter()),
        vec!["#a"],
    );
}

#[test]
fn an_option_wants_a_present_non_empty_hash() {
    assert!(an_option(Some("#a")));
    assert!(!an_option(Some("a")));
    assert!(!an_option(Some("")));
    assert!(!an_option(None));
}

#[test]
fn holds_a_comment_is_unchanged_by_the_fixer() {
    let lines = vec!["#a", "b", ""];
    assert_eq!(holds_a_comment(lines.into_iter()), vec!["#a"]);
}
