//! A crate spelling a defaulted mapper the long way round, which
//! `perfectionist::map_or` and `perfectionist::map_or_else` rewrite
//! as one call.

/// The width of `name`, or zero where there is none.
pub fn width(name: Option<&str>) -> usize {
    name.map_or(usize::default(), str::len)
}

/// `name` in upper case, or the empty string where there is none.
pub fn shout(name: Option<&str>) -> String {
    name.map_or_else(|| String::new(), str::to_uppercase)
}

/// What this crate contributes to the fixture's output.
pub fn report() -> String {
    format!("map_or: {} {}\n", width(Some("abc")), shout(Some("abc")))
}

#[cfg(test)]
mod tests {
    /// A present value reaches the mapper and an absent one reaches
    /// the default, which is the whole of what the rewrites in this
    /// crate have to preserve.
    #[test]
    fn both_sides_answer_as_they_did() {
        assert_eq!(super::width(Some("abcd")), 4);
        assert_eq!(super::width(None), 0);
        assert_eq!(super::shout(Some("abc")), "ABC");
        assert_eq!(super::shout(None), "");
    }
}
