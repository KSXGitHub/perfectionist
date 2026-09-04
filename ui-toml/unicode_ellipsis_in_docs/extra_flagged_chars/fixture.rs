// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
/// Default target U+2026 ellipsis… is flagged.
fn _u2026() {}

/// Configured midline ellipsis⋯ is flagged too.
fn _u22ef() {}

/// Three ASCII dots... stay clean.
fn _ascii() {}

fn main() {}
