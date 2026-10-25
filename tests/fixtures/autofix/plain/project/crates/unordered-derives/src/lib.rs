//! A crate listing its derives out of order, which
//! `perfectionist::unordered_derives` sorts. The rule ships inactive,
//! so only a run that enables it rewrites this.

/// A tag, carrying the derives in the order they were typed.
#[derive(PartialEq, Clone, Debug, Eq)]
pub struct Tag(pub u8);

/// What this crate contributes to the fixture's output.
pub fn report() -> String {
    format!("unordered_derives: {:?}\n", Tag(7))
}

#[cfg(test)]
mod tests {
    /// Every derived trait stays derived, whatever order the list
    /// ends up in.
    #[test]
    fn the_derived_traits_still_work() {
        assert_eq!(super::Tag(7), super::Tag(7).clone());
        assert_eq!(format!("{:?}", super::Tag(7)), "Tag(7)");
    }
}
