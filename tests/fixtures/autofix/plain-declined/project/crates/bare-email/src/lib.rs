//! An address `perfectionist::bare_email` reports and offers no
//! autofix for.
//!
//! Under the default `either` style the rule accepts any of the
//! wrapped forms and offers one `MaybeIncorrect` suggestion per form,
//! on purpose: which form to write is the author's choice, so neither
//! is the fixer's to apply.

/// Problems go to security@example.com and are answered there.
pub const CONTACT: &str = "security@example.com";

#[cfg(test)]
mod tests {
    /// The constant holds the address the documentation above it
    /// leaves bare, so a rewrite of that sentence would show up in
    /// this crate's diff.
    #[test]
    fn the_address_is_the_one_left_bare() {
        assert_eq!(super::CONTACT, "security@example.com");
    }
}
