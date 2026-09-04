// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// Line comment with ellipsis…
/* Block comment with ellipsis… */

/// Outer doc comment with ellipsis…
fn _outer() {}

mod inner {
    //! Inner doc comment with ellipsis…
}

fn main() {}
