//! A module whose documentation leaves a URL bare, which
//! `perfectionist::bare_url` wraps in angle brackets.

/// The project home page is https://example.com and this sentence
/// leaves it bare. The rewrite is offered at `MachineApplicable`
/// only where the address ends in a character that cannot have been
/// prose, so the sentence carries on after a space.
pub const HOME: &str = "https://example.com";

/// What this module contributes to the fixture's output.
pub fn report() -> String {
    format!("bare_url: {HOME}\n")
}

#[cfg(test)]
mod tests {
    /// The report names the home page, and wrapping the bare address
    /// in the documentation that names it changes nothing here.
    #[test]
    fn the_report_names_the_home_page() {
        assert_eq!(super::report(), "bare_url: https://example.com\n");
    }
}
