//! A crate spelling out the template a
//! [`derive_more`](https://docs.rs/derive_more) `Display` derive
//! already forwards to, which
//! `perfectionist::redundant_derive_more_forward_template` deletes.

/// A name, displayed as the string it wraps.
#[derive(derive_more::Display)]
#[display("{_0}")]
pub struct Name(pub String);

/// What this crate contributes to the fixture's output.
pub fn report() -> String {
    format!(
        "redundant_derive_more_forward_template: {}\n",
        Name("wrapped".to_owned()),
    )
}

#[cfg(test)]
mod tests {
    /// The wrapper still displays as the string it wraps, which
    /// deleting the template that said so cannot change.
    #[test]
    fn the_wrapper_displays_as_its_string() {
        assert_eq!(super::Name("wrapped".to_owned()).to_string(), "wrapped");
    }
}
