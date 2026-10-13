use super::take_version;

/// The banner is a name and a version, and the version is what comes
/// back out of it.
#[test]
fn a_banner_yields_its_version() {
    assert_eq!(take_version("cargo-dylint 6.1.0"), Some("6.1.0"));
}

/// The output arrives through a pipe, so it carries the newline the
/// banner ends with, and a shim may pad it either side. Neither is
/// part of the version.
#[test]
fn padding_around_a_banner_is_not_part_of_the_version() {
    assert_eq!(take_version("  cargo-dylint 6.1.0  \n"), Some("6.1.0"));
}

/// The version is the token after the name rather than the last word
/// of the output, so a banner carrying a suffix still yields it.
#[test]
fn a_suffix_after_the_version_is_left_unread() {
    assert_eq!(take_version("cargo-dylint 6.1.0 (abcdef0)"), Some("6.1.0"));
}

/// Prose carries no version however much it ends in a word:
/// `not a version at all` must not yield `all`, which is what reading
/// the output's last word would have made of it.
#[test]
fn prose_carries_no_version() {
    assert_eq!(take_version("not a version at all"), None);
}

/// A version run onto the end of the name is not a banner: the
/// whitespace between them has to be there.
#[test]
fn the_name_has_to_be_followed_by_whitespace() {
    assert_eq!(take_version("cargo-dylint6.1.0"), None);
}

/// A banner that stops after the name carries no version, whether or
/// not the whitespace after it is there.
#[test]
fn a_name_alone_carries_no_version() {
    assert_eq!(take_version("cargo-dylint"), None);
    assert_eq!(take_version("cargo-dylint   "), None);
}

/// Empty output is the shape a binary that printed nothing produces.
#[test]
fn empty_output_carries_no_version() {
    assert_eq!(take_version(""), None);
}

/// Another program's banner is not this program's, so its version is
/// not read as this one's.
#[test]
fn another_program_s_banner_is_rejected() {
    assert_eq!(take_version("dylint-link 6.1.0"), None);
}
