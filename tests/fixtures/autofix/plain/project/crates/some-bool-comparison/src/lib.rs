//! A crate comparing an `Option<bool>` against a `Some` of a literal,
//! which `perfectionist::some_bool_comparison` rewrites.

/// Whether `flag` was set and set to true.
pub fn is_verbose(flag: Option<bool>) -> bool {
    flag == Some(true)
}

/// Whether `flag` was set and set to false.
pub fn is_quiet(flag: Option<bool>) -> bool {
    flag == Some(false)
}

/// What this crate contributes to the fixture's output.
pub fn report() -> String {
    format!(
        "some_bool_comparison: {} {} {}\n",
        is_verbose(Some(true)),
        is_quiet(Some(false)),
        is_verbose(None),
    )
}

#[cfg(test)]
mod tests {
    /// Each of the three states a comparison distinguishes, since a
    /// rewrite that collapses the absent case into either present one
    /// compiles and answers differently.
    #[test]
    fn every_state_answers_as_it_did() {
        assert!(super::is_verbose(Some(true)));
        assert!(!super::is_verbose(Some(false)));
        assert!(!super::is_verbose(None));
        assert!(super::is_quiet(Some(false)));
        assert!(!super::is_quiet(Some(true)));
        assert!(!super::is_quiet(None));
    }
}
