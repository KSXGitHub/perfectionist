use super::ExemptPrefix;

fn parse(prefix: &str) -> Result<String, String> {
    ExemptPrefix::try_from(prefix.to_owned()).map(String::from)
}

#[test]
fn a_prefix_ending_in_an_underscore_is_accepted() {
    assert_eq!(parse("clone_"), Ok("clone_".to_owned()));
    assert_eq!(parse("cloned_"), Ok("cloned_".to_owned()));
    assert_eq!(parse("copy_"), Ok("copy_".to_owned()));
    assert_eq!(parse("a_"), Ok("a_".to_owned()));
}

#[test]
fn a_prefix_without_the_underscore_is_rejected() {
    let error = parse("clone").expect_err("`clone` should be rejected");
    assert!(error.contains("ending in `_`"), "{error}");
    assert!(error.contains("partway through a word"), "{error}");
}

#[test]
fn a_bare_underscore_is_rejected() {
    let error = parse("_").expect_err("`_` should be rejected");
    assert!(error.contains("at least one character"), "{error}");
}

#[test]
fn the_empty_string_is_rejected() {
    let error = parse("").expect_err("an empty prefix should be rejected");
    assert!(error.contains("ending in `_`"), "{error}");
}

#[test]
fn a_conversion_prefix_is_rejected() {
    for prefix in ["as_", "to_", "into_"] {
        let error = parse(prefix).unwrap_err();
        assert!(error.contains("conversion prefix"), "{prefix}: {error}");
        assert!(error.contains("no effect"), "{prefix}: {error}");
    }
}

#[test]
fn a_name_merely_starting_like_a_conversion_prefix_is_accepted() {
    // The check is on the whole value, not a prefix of it: `to_` is
    // rejected, `token_` is not.
    assert_eq!(parse("token_"), Ok("token_".to_owned()));
    assert_eq!(parse("assemble_"), Ok("assemble_".to_owned()));
    assert_eq!(parse("intolerant_"), Ok("intolerant_".to_owned()));
}
