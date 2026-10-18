// A stand-in for a crate whose exported macros panic with a message of
// their own. Each message is a string literal in *this* file, so a scan
// that reads the source text at the literal's span finds the dash here,
// in a file the caller cannot edit.
//
// Nothing here is `perfectionist`-specific. It exists because
// `em_dash_prose` reads a message from its literal's source text, and
// an external `macro_rules!` is the one shape where that text is a real
// string literal outside the crate being linted.

#[macro_export]
macro_rules! bail_with_a_dash {
    () => {
        panic!("an external macro's panic message with a dash — here")
    };
}

#[macro_export]
macro_rules! expect_with_a_dash {
    ($value:expr) => {
        $value.expect("an external macro's expect message with a dash — here")
    };
}
