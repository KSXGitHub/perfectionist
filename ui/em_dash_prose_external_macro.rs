// aux-build:external_message_macro.rs

// Regression test: a message written in another crate's
// `macro_rules!` is not the caller's to reword, so the dash in it is
// not reported. The rule reads a message from the source text at its
// literal's span, and for an external macro that text is a real string
// literal in a file the caller cannot edit, so the snippet read is no
// guard here; what suppresses the diagnostic is
// `report_in_external_macro: false` on the lint, which the dash span
// reaches because it inherits the literal's `SyntaxContext`.
//
// The local macro at the bottom is the control: the same shape written
// in this crate is reported, so the rule does reach a message that
// arrives through an expansion and the two external cases are silent
// for the reason stated rather than because nothing is scanned.

#![allow(dead_code, reason = "ui fixture")]

extern crate external_message_macro;

// Not flagged: the panic message belongs to the macro's own crate.
fn _external_panic() {
    external_message_macro::bail_with_a_dash!();
}

// Not flagged: the expect message belongs to the macro's own crate.
fn _external_expect() {
    let value = Some(1);
    let _: i32 = external_message_macro::expect_with_a_dash!(value);
}

macro_rules! expect_locally {
    ($value:expr) => {
        // Bad: a local macro's message is in this crate, so rewording
        // it is the author's to do.
        $value.expect("the value is unset — set it first")
    };
}

fn _local_expect() {
    let value = Some(1);
    let _: i32 = expect_locally!(value);
}

fn main() {}
