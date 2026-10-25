//! A reference `perfectionist::bare_issue_reference` reports and
//! hands over at `MaybeIncorrect`.
//!
//! A bare `#NNN` may name an issue, a pull request, a colour or any
//! other numbered thing, and the configured repository says where a
//! link would point rather than that a link is wanted at all. The
//! rule offers the link for a reader to confirm rather than for the
//! fixer to apply.

/// The issue this crate stands in for, closes #123 as a bare
/// reference.
pub const ISSUE: u32 = 123;

#[cfg(test)]
mod tests {
    /// The number stays what it was, so a rewrite of the
    /// documentation on `ISSUE` would show up in this crate's diff
    /// rather than here.
    #[test]
    fn the_issue_number_is_unchanged() {
        assert_eq!(super::ISSUE, 123);
    }
}
