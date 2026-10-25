//! A module whose documentation leaves an address bare, which
//! `perfectionist::bare_email` wraps in angle brackets.

/// Problems go to security@example.com and are answered there.
pub const CONTACT: &str = "security@example.com";

/// What this module contributes to the fixture's output.
pub fn report() -> String {
    format!("bare_email: {CONTACT}\n")
}

#[cfg(test)]
mod tests {
    /// The report names the address, and wrapping the bare one in the
    /// documentation that names it changes nothing here.
    #[test]
    fn the_report_names_the_address() {
        assert_eq!(super::report(), "bare_email: security@example.com\n");
    }
}
