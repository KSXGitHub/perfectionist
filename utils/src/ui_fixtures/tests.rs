//! The path guard, exercised against each shape that would resolve
//! outside the `TempDir`. Every case panics before any filesystem
//! work, which is what keeps a wrong call from reaching `copy_dir`.

use super::copy_fixtures_with_directives;

/// `Path::join` discards its base when the argument is absolute, so
/// an absolute fixture path resolves onto the manifest directory —
/// where the copy would read and write the same files.
#[test]
#[should_panic(expected = "every component of the fixture path must be an ordinary name")]
fn an_absolute_fixture_path_is_rejected() {
    copy_fixtures_with_directives("/home/user/perfectionist", "/home/user/perfectionist");
}

/// `..` walks out of the `TempDir` without ever being absolute, so
/// the shape has to be rejected by component rather than by
/// `Path::is_relative`.
#[test]
#[should_panic(expected = "every component of the fixture path must be an ordinary name")]
fn a_fixture_path_climbing_out_is_rejected() {
    copy_fixtures_with_directives("/home/user/perfectionist", "../ui");
}

/// The two arguments the wrong way round: the fixture path lands in
/// the manifest slot, where it is not absolute.
#[test]
#[should_panic(expected = "the manifest dir must be absolute")]
fn transposed_arguments_are_rejected() {
    copy_fixtures_with_directives(
        "ui-toml/bare_url/custom_skip_hosts",
        "/home/user/perfectionist",
    );
}
