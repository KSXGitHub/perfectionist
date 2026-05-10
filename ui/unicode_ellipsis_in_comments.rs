// Bad: line comment with horizontal ellipsis…
// Good: line comment with three dots...

/* Bad: block comment ending with ellipsis… */
/* Good: block comment ending with three dots... */

/*
 * Multi-line block comment with…
 * multiple lines, each scanned independently…
 */

/// Doc comment with ellipsis… (must be ignored).
fn _doc_outer() {}

mod inner {
    //! Inner doc comment with ellipsis… (must be ignored).
}

fn main() {
    let _ = "string literal with …"; // not a comment, ignored
}
