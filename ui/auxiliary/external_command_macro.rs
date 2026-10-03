// A stand-in for a crate whose exported macro builds an item around an
// expression the caller wrote. The expression's tokens stay the
// caller's, so the span-on-the-segment guard finds nothing; what
// suppresses the diagnostic is the enclosing item, whose `def_span`
// belongs to this crate's macro.
//
// Nothing here is `perfectionist`-specific. It exists because
// `mutating_command_builder` is the rule that needs an *external*
// `macro_rules!`, where its siblings need an external derive and use
// `proc_macro_synth_binding` for that.

#[macro_export]
macro_rules! wrap_in_a_function {
    ($body:expr) => {
        pub fn generated() {
            $body;
        }
    };
}
