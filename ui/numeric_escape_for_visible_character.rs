// edition:2024
#![allow(dead_code, unused, reason = "ui fixture")]

// Bad: a visible non-ASCII character written as a Unicode escape.
fn unicode_escape_in_string() {
    let _ = "\u{2192} next";
}

// Bad: a visible ASCII character written as a hex escape.
fn hex_escape_in_string() {
    let _ = "\x41BC";
}

// Bad: several escapes in one literal, each reported on its own.
fn several_escapes_in_one_string() {
    let _ = "R\u{E9}sum\u{E9}";
}

// Bad: a character literal.
fn unicode_escape_in_char() {
    let _ = '\u{2715}';
}

// Bad: a C-string literal, which takes a string literal's escapes.
fn unicode_escape_in_cstring() {
    let _ = c"\u{2192}";
}

// Bad: the closing quote of a string literal has no verbatim spelling
// there, but an apostrophe does.
fn apostrophe_in_string() {
    let _ = "it\x27s here";
}

// Bad: the closing quote of a character literal has no verbatim
// spelling there, but a double quote does.
fn double_quote_in_char() {
    let _ = '\x22';
}

// Bad: an escape with the underscore separator the grammar allows
// between the hex digits of a Unicode escape.
fn underscore_separated_escape() {
    let _ = "\u{1_F600}";
}

// Bad: a literal inside a macro invocation. `println!` consumes its
// template during expansion, so nothing of it survives as a literal.
fn macro_template() {
    println!("done \u{2714}");
}

// Bad: a template carrying a placeholder is split into static pieces
// during expansion, and the escape in it is still flagged.
fn macro_template_with_placeholder() {
    let value = 1;
    println!("{value} \u{2714}");
}

// Bad: a brace written as an escape. The placeholder grammar applies
// to the decoded text, so this template already reads as the doubled
// brace that prints one, and still does once spelled out.
fn escaped_brace_in_template() {
    println!("\u{7B}\u{7B}");
}

// Bad: an attribute's string literal is scanned like any other.
#[doc = "arrow: \u{2192}"]
fn escape_in_attribute() {}

// Not flagged: the escape of a character the string literal cannot
// hold verbatim, its own closing delimiter.
fn escaped_double_quote_in_string() {
    let _ = "say \x22hi\x22";
}

// Not flagged: the escape of a character the character literal cannot
// hold verbatim, its own closing delimiter.
fn escaped_apostrophe_in_char() {
    let _ = '\x27';
}

// Not flagged: the escape of a backslash, which no literal holds
// verbatim.
fn escaped_backslash() {
    let _ = "C:\x5CUsers";
}

// Not flagged: a control character has no glyph to spell it with.
fn control_characters() {
    let _ = "\x00\x1B\x7F";
}

// Not flagged: whitespace, where the escape is what says which space
// this is, such as a tab, a no-break space, or an ideographic space.
fn whitespace_characters() {
    let _ = "\x09\u{A0}\u{3000}";
}

// Not flagged: a zero-width character measures no column, so spelling
// it would leave nothing for a reader to see.
fn zero_width_characters() {
    let _ = "\u{200B}\u{200D}\u{FEFF}";
}

// Not flagged: a combining mark has no standalone glyph; spelled out,
// it would attach to whatever precedes it in the source.
fn combining_mark() {
    let _ = "e\u{301}";
}

// Not flagged: a Private Use Area codepoint renders as a replacement
// box outside the one font that assigns it a glyph.
fn private_use_character() {
    let _ = "\u{E0B0}";
}

// Not flagged: a byte literal holds bytes rather than characters.
fn byte_literals() {
    let _ = b"\x41BC";
    let _ = b'\x41';
}

// Not flagged: past U+007F a C-string literal's hex escape denotes one
// byte, which the matching character would encode as two.
fn high_hex_escape_in_cstring() {
    let _ = c"\xC3\xA9";
}

// Not flagged: a raw string literal has no escapes, so the backslash
// and the text after it are ordinary characters.
fn raw_string() {
    let _ = r"\u{2192}";
}

// Not flagged: an escape that is not a numeric one.
fn other_escapes() {
    let _ = "a\tb\nc\0d";
}

// Not flagged: the character is already spelled as itself.
fn already_spelled_out() {
    let _ = "→ next";
}

fn main() {}
