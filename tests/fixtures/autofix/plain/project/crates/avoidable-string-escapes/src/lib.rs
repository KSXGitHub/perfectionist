//! A crate escaping characters that a raw string literal spells
//! verbatim, which `perfectionist::avoidable_string_escapes` rewrites
//! as a raw literal.

/// A JSON object, written with every quote escaped.
pub const MANIFEST: &str = "{\"name\":\"fixture\"}";

/// A Windows path, written with every separator escaped.
pub const SEPARATOR: &str = "C:\\Users\\fixture";

/// What this crate contributes to the fixture's output.
pub fn report() -> String {
    format!("avoidable_string_escapes: {MANIFEST} {SEPARATOR}\n")
}

#[cfg(test)]
mod tests {
    /// Each literal holds the same characters whichever way it is
    /// spelled, which is the whole of what the rewrite may change.
    #[test]
    fn the_literals_hold_the_same_characters() {
        assert_eq!(super::MANIFEST, r#"{"name":"fixture"}"#);
        assert_eq!(super::SEPARATOR, r"C:\Users\fixture");
    }
}
