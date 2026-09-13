use super::{
    Placeholder, Segment, parse_template, take_escaped_brace, take_literal_text, take_placeholder,
};

fn placeholder(argument: &str) -> Segment<'_> {
    Segment::Placeholder(Placeholder {
        argument,
        format_spec: None,
    })
}

fn placeholder_with_spec<'a>(argument: &'a str, format_spec: &'a str) -> Segment<'a> {
    Segment::Placeholder(Placeholder {
        argument,
        format_spec: Some(format_spec),
    })
}

#[test]
fn empty_template_has_no_segments() {
    assert_eq!(parse_template(""), Some(Vec::new()));
}

#[test]
fn plain_text_is_one_literal_segment() {
    assert_eq!(
        parse_template("no placeholder here"),
        Some(vec![Segment::Literal("no placeholder here")]),
    );
}

#[test]
fn lone_placeholder_is_one_segment() {
    assert_eq!(parse_template("{_0}"), Some(vec![placeholder("_0")]));
}

#[test]
fn implicit_positional_placeholder_has_an_empty_argument() {
    assert_eq!(parse_template("{}"), Some(vec![placeholder("")]));
}

#[test]
fn colon_splits_argument_from_format_spec() {
    assert_eq!(
        parse_template("{_0:>8}"),
        Some(vec![placeholder_with_spec("_0", ">8")]),
    );
}

#[test]
fn empty_format_spec_is_distinguished_from_an_absent_one() {
    assert_eq!(
        parse_template("{_0:}"),
        Some(vec![placeholder_with_spec("_0", "")]),
    );
}

#[test]
fn spec_collapses_the_absent_and_empty_forms() {
    let absent = Placeholder {
        argument: "_0",
        format_spec: None,
    };
    let empty = Placeholder {
        argument: "_0",
        format_spec: Some(""),
    };
    assert_eq!(absent.spec(), "");
    assert_eq!(empty.spec(), "");
}

#[test]
fn literal_text_surrounds_a_placeholder() {
    assert_eq!(
        parse_template("bad token at offset {_0}!"),
        Some(vec![
            Segment::Literal("bad token at offset "),
            placeholder("_0"),
            Segment::Literal("!"),
        ]),
    );
}

#[test]
fn doubled_braces_are_escapes_not_placeholders() {
    assert_eq!(
        parse_template("{{{_0}}}"),
        Some(vec![
            Segment::EscapedBrace('{'),
            placeholder("_0"),
            Segment::EscapedBrace('}'),
        ]),
    );
}

#[test]
fn adjacent_placeholders_are_separate_segments() {
    assert_eq!(
        parse_template("{_0}{_1}"),
        Some(vec![placeholder("_0"), placeholder("_1")]),
    );
}

#[test]
fn unclosed_placeholder_is_rejected() {
    assert_eq!(parse_template("{_0"), None);
}

#[test]
fn unmatched_closing_brace_is_rejected() {
    assert_eq!(parse_template("}"), None);
    assert_eq!(parse_template("a } b"), None);
}

#[test]
fn non_ascii_literal_text_is_kept_whole() {
    assert_eq!(
        parse_template("héllo {_0}"),
        Some(vec![Segment::Literal("héllo "), placeholder("_0")]),
    );
}

#[test]
fn take_escaped_brace_matches_only_a_doubled_brace() {
    assert_eq!(take_escaped_brace("{{rest"), Some(('{', "rest")));
    assert_eq!(take_escaped_brace("}}rest"), Some(('}', "rest")));
    assert_eq!(take_escaped_brace("{_0}"), None);
    assert_eq!(take_escaped_brace("{"), None);
    assert_eq!(take_escaped_brace("ab"), None);
    assert_eq!(take_escaped_brace(""), None);
}

#[test]
fn take_literal_text_stops_at_the_next_brace() {
    assert_eq!(take_literal_text("abc{_0}"), ("abc", "{_0}"));
    assert_eq!(take_literal_text("abc}"), ("abc", "}"));
    assert_eq!(take_literal_text("abc"), ("abc", ""));
    assert_eq!(take_literal_text("{abc"), ("", "{abc"));
}

#[test]
fn take_placeholder_consumes_through_the_closing_brace() {
    let (found, rest) = take_placeholder("{_0:x} tail").expect("placeholder");
    assert_eq!(found.argument, "_0");
    assert_eq!(found.format_spec, Some("x"));
    assert_eq!(rest, " tail");
}

#[test]
fn take_placeholder_rejects_a_non_placeholder_prefix() {
    assert!(take_placeholder("abc").is_none());
    assert!(take_placeholder("{unclosed").is_none());
}
