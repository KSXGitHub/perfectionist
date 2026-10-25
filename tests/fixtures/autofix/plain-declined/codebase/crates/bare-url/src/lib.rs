//! A URL `perfectionist::bare_url` reports and offers no autofix
//! for.
//!
//! The address runs up to a comma, which may be the URL's own
//! character or the prose's, so where the URL ends is not something
//! the rule can settle. It holds the rewrite at `MaybeIncorrect` on
//! purpose, for a reader to judge rather than for the fixer to
//! apply.

/// The project home page is https://example.com/path, named in a
/// sentence that carries on after it.
pub const HOME: &str = "https://example.com/path";

#[cfg(test)]
mod tests {
    /// The address is the one the documentation on `HOME` leaves
    /// bare, so a rewrite of that sentence would show up in this
    /// crate's diff.
    #[test]
    fn the_address_is_the_one_left_bare() {
        assert_eq!(super::HOME, "https://example.com/path");
    }
}
