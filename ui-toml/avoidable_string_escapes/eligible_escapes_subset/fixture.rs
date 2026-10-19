// `eligible_escapes = ["\\\""]`: only `\"` is considered
// eliminable. A literal whose escapes are exclusively `\\` is
// therefore treated as if it had non-raw escapes (since `\\` is
// no longer eligible) and is left untouched. A literal whose
// escapes are exclusively `\"` is flagged as usual.

fn _backslash_only() {
    // `\\` is no longer eligible, so it must NOT fire.
    let _ = "C:\\Users\\foo";
}

fn _quote_only() {
    // `\"` is still eligible, so it must fire.
    let _ = "say \"hello\"";
}

fn main() {
    _backslash_only();
    _quote_only();
}
