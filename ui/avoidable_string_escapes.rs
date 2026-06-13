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

// Bad: literal is a `core` macro argument. With
// `report_in_external_macro: true` the user-written escape inside
// `println!` (and the equivalent `format!` / `vec!` / `assert!`
// invocations) must still be flagged.
fn _inside_core_macro() {
    println!("path: \"foo\"");
}

// Bad: `format!` template carrying a `{...}` placeholder. The
// placeholder splits the template away from the HIR during format-args
// lowering, so the late `ExprKind::Lit` pass never sees it; the
// pre-expansion pass rescues it. (issue #219)
fn _format_with_inline_placeholder() {
    let name = "x";
    let _ = format!("url(\"{name}\")");
}

// Bad: same split reached through `println!` with a positional
// placeholder rather than an inline-captured one.
fn _println_with_positional_placeholder() {
    println!("path: \"{}\"", "x");
}

// Bad: `assert_eq!`'s message is the *third* argument, with no macro
// allowlist or argument-position bookkeeping — the pre-expansion scan
// reaches every literal. The message carries a placeholder, so it is
// split away from the HIR; the comparison operands (`value`) are not
// literals here, so nothing competes for the diagnostic.
fn _assert_eq_message_placeholder() {
    let value = 1;
    assert_eq!(value, value, "v = \"{value}\"");
}

// Bad, exactly once: a string-literal *operand* of `assert_eq!`
// survives lowering as an ordinary value, so the late pass flags it. The
// pre-expansion scan also sees it, but the byte-range dedup drops the
// duplicate — this fixture pins that there is no double diagnostic.
fn _assert_eq_string_operand() {
    assert_eq!("a\"b".len(), 3);
}

// Bad: `stringify!` reflects its argument's source spelling, so the
// literal is consumed rather than surviving into the HIR. The rewrite is
// still value-preserving; the project deliberately flags it (suppress
// with `#[allow]` in the rare case the exact `stringify!` text matters).
fn _stringify_argument() {
    let _ = stringify!("a\"b");
}

// Bad: `dbg!`'s argument *survives* lowering as the `match` scrutinee,
// so the late pass flags it (the pre-expansion scan also sees it, but the
// byte-range dedup drops the duplicate). The rewritten value is
// identical; only the `stringify!`-reflected debug label printed at
// runtime changes — the same deliberately-accepted trade-off as
// `stringify!`.
fn _dbg_argument() {
    let _ = dbg!("a\"b");
}

// Not flagged: a placeholder template that also contains a non-raw
// escape (`\n`). The literal is ineligible as a whole, and the static
// pieces the compiler splits it into are not rewritten either.
fn _placeholder_with_newline_escape() {
    let value = 1;
    let _ = format!("a\"b{value}\nc");
}

// Not flagged: contains a required non-raw escape (`\n`).
fn _has_newline_escape() {
    let _ = "name:\tvalue\n";
}

// Not flagged: line-continuation escape (`\` followed by a real
// newline) is the `_ =>` arm of `take_escape_non_raw`. The
// continuation cannot be expressed in a raw string, so the lint
// must stay silent here.
fn _line_continuation() {
    let _ = "foo\
    bar";
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
    _inside_core_macro();
    _format_with_inline_placeholder();
    _println_with_positional_placeholder();
    _assert_eq_message_placeholder();
    _assert_eq_string_operand();
    _stringify_argument();
    _dbg_argument();
    _placeholder_with_newline_escape();
    _has_newline_escape();
    _line_continuation();
    _mixed_eligible_and_non_raw();
    _already_raw();
    _no_escapes();
    _hex_escape();
    _unicode_escape();
}
