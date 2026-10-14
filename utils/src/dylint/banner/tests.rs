use super::parse_version;

#[test]
fn a_banner_yields_its_version() {
    assert_eq!(parse_version("cargo-dylint 6.1.0"), Some("6.1.0"));
}

/// The output arrives through a pipe, so it always carries the
/// newline the banner ends with.
#[test]
fn padding_around_a_banner_is_not_part_of_the_version() {
    assert_eq!(parse_version("  cargo-dylint 6.1.0  \n"), Some("6.1.0"));
}

#[test]
fn a_suffix_after_the_version_is_rejected() {
    assert_eq!(parse_version("cargo-dylint 6.1.0 (abcdef0)"), None);
}

/// The input ends in a word, so reading the output's last word would
/// take `all` for the version.
#[test]
fn prose_carries_no_version() {
    assert_eq!(parse_version("not a version at all"), None);
}

#[test]
fn the_name_has_to_be_followed_by_whitespace() {
    assert_eq!(parse_version("cargo-dylint6.1.0"), None);
}

#[test]
fn a_name_alone_carries_no_version() {
    assert_eq!(parse_version("cargo-dylint"), None);
    assert_eq!(parse_version("cargo-dylint   "), None);
}

#[test]
fn empty_output_carries_no_version() {
    assert_eq!(parse_version(""), None);
}

#[test]
fn a_different_program_name_is_rejected() {
    assert_eq!(parse_version("dylint-link 6.1.0"), None);
}
