//! An address `perfectionist::bare_email` reports and hands over two
//! rewrites for.
//!
//! Under the default `either` style the rule offers both the
//! angle-bracket and the `mailto:` form, so the diagnostic carries
//! two rewrites and the fixer has none it may apply: picking between
//! them is the author's.

/// Problems go to security@example.com and are answered there.
pub const CONTACT: &str = "security@example.com";

#[cfg(test)]
mod tests {
    /// The address is the one the documentation on `CONTACT` leaves
    /// bare, so a rewrite of that sentence would show up in this
    /// crate's diff.
    #[test]
    fn the_address_is_the_one_left_bare() {
        assert_eq!(super::CONTACT, "security@example.com");
    }
}
