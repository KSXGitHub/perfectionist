//! A template `perfectionist::redundant_derive_more_forward_template`
//! reports and declines to delete.
//!
//! The placeholder asks for the field's `Debug`, which is not what
//! the `Display` derive forwards to, so deleting the template would
//! change what the generated code prints.

/// A name, displayed as the debug form of the string it wraps.
#[derive(derive_more::Display)]
#[display("{_0:?}")]
pub struct Name(pub String);

#[cfg(test)]
mod tests {
    /// The wrapper displays as the debug form, quotes included, so
    /// deleting the template would show up here as well as in this
    /// crate's diff.
    #[test]
    fn the_wrapper_displays_as_the_debug_form() {
        let name = super::Name("wrapped".to_owned());
        assert_eq!(name.to_string(), r#""wrapped""#);
    }
}
