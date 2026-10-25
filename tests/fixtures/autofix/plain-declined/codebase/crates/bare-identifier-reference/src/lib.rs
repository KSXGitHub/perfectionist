//! A code span `perfectionist::bare_identifier_reference` reports and
//! hands over at `MaybeIncorrect`.
//!
//! A name that resolves in scope may still be the prose's way of
//! naming a foreign symbol of the same name, which linking it to the
//! local item would misdirect. The rule offers the link for a reader
//! to confirm rather than for the fixer to apply.

/// A helper this crate hands out.
pub struct Helper(pub u8);

/// Builds a `Helper` carrying `seed`.
pub fn build(seed: u8) -> Helper {
    Helper(seed)
}

#[cfg(test)]
mod tests {
    /// The helper carries the seed it was built with, so a rewrite of
    /// the documentation on `build` would show up in this crate's
    /// diff rather than here.
    #[test]
    fn the_helper_carries_its_seed() {
        assert_eq!(super::build(3).0, 3);
    }
}
