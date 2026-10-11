// edition:2024

// Bad: an ASCII character, which the narrowed rule still flags. The
// character's own spelling is ASCII, so a project running
// `clippy::non_ascii_literal` keeps this half of the rule.
fn _ascii_escape() {
    let _ = "\x41BC";
    let _ = "\u{41}BC";
    let _ = '\x7E';
}

// Not flagged: a non-ASCII character, which `flag_non_ascii = false`
// takes out of scope. Spelling it out is what an ASCII-only source
// policy forbids.
fn _non_ascii_escape() {
    let _ = "\u{2192} next";
    let _ = "R\u{E9}sum\u{E9}";
    let _ = '\u{2715}';
    let _ = c"\u{2192}";
}

fn main() {
    _ascii_escape();
    _non_ascii_escape();
}
