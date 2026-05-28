use super::AsciiLetter;

/// Driver that pipes a TOML array through `AsciiLetter`'s serde
/// impl via a synthetic struct, mirroring how the real rule
/// `Config`s use it. Returns the deserialised letters on success
/// or the serde error message on failure, so the tests can pin
/// both the happy and the rejection paths.
fn parse(toml_text: &str) -> Result<Vec<char>, String> {
    #[derive(serde::Deserialize)]
    struct Wrap {
        letters: Vec<AsciiLetter>,
    }
    toml::from_str::<Wrap>(toml_text)
        .map(|wrap| wrap.letters.into_iter().map(char::from).collect())
        .map_err(|err| err.to_string())
}

#[test]
fn empty_list_is_accepted() {
    assert_eq!(parse("letters = []").unwrap(), Vec::<char>::new());
}

#[test]
fn ascii_letters_are_accepted() {
    assert_eq!(parse(r#"letters = ["x", "Y"]"#).unwrap(), vec!['x', 'Y']);
}

#[test]
fn multi_character_string_is_rejected_at_parse_time() {
    assert!(parse(r#"letters = ["xy"]"#).is_err());
}

#[test]
fn ascii_digit_is_rejected_with_our_message() {
    let error = parse(r#"letters = ["1"]"#).unwrap_err();
    assert!(
        error.contains("expected a single ASCII letter"),
        "unexpected error message: {error}",
    );
    assert!(
        error.contains("'1'"),
        "error should name the offending character: {error}",
    );
}

#[test]
fn non_ascii_letter_is_rejected_with_our_message() {
    let error = parse(r#"letters = ["é"]"#).unwrap_err();
    assert!(
        error.contains("expected a single ASCII letter"),
        "unexpected error message: {error}",
    );
    assert!(
        error.contains("'é'") || error.contains(r"'\u"),
        "error should name the offending character: {error}",
    );
}
