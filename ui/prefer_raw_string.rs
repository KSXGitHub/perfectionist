// Bad: escaped quotes — JSON snippet.
fn _escaped_quotes() {
    let _ = "{\"name\":\"foo\"}";
}

// Bad: escaped backslashes — Windows path.
fn _escaped_backslashes() {
    let _ = "C:\\Users\\foo\\bar";
}

// Bad: escaped apostrophe.
fn _escaped_apostrophe() {
    let _ = "it\'s here";
}

// Bad: multi-byte UTF-8 surrounding an eligible escape. Exercises
// the `len_utf8` branch in `take_literal_char` — the autofix must
// preserve the non-ASCII characters byte-for-byte.
fn _multi_byte_utf8() {
    let _ = "日本語: C:\\Users";
}

// Bad: mix of `\"` and `\\` in one literal.
fn _mixed_eligible_escapes() {
    let _ = "say \"\\\\\" twice";
}

// Bad: literal contains `"#`, so the autofix needs two hashes
// (`r##"..."##`) to avoid colliding with the closing delimiter.
fn _hash_count_grows() {
    let _ = "snippet: \"#suffix\" end";
}

// Not flagged: contains a required non-raw escape (`\n`).
fn _has_newline_escape() {
    let _ = "name:\tvalue\n";
}

// Not flagged: mixed eligible + non-raw escapes.
fn _mixed_eligible_and_non_raw() {
    let _ = "She said \"hi\" then\nleft.";
}

// Not flagged: already a raw string.
fn _already_raw() {
    let _ = r#"<div class="x">"#;
}

// Not flagged: no escapes at all.
fn _no_escapes() {
    let _ = "hello world";
}

// Not flagged: `\x41` is a Unicode-style escape, not eliminable.
fn _hex_escape() {
    let _ = "\x41BC";
}

// Not flagged: `\u{1F600}` is a Unicode escape.
fn _unicode_escape() {
    let _ = "emoji: \u{1F600}";
}

fn main() {
    _escaped_quotes();
    _escaped_backslashes();
    _escaped_apostrophe();
    _multi_byte_utf8();
    _mixed_eligible_escapes();
    _hash_count_grows();
    _has_newline_escape();
    _mixed_eligible_and_non_raw();
    _already_raw();
    _no_escapes();
    _hex_escape();
    _unicode_escape();
}
