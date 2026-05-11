// `escapes_eligible = ["\\n"]` is a misconfiguration: `\n` decodes
// to a newline character, not the letter `n`, so accepting it as
// "eliminable" would let the autofix rewrite a newline-containing
// literal into one containing `n`. The rule must silently filter
// `\n` out at config load and behave as if `escapes_eligible` were
// empty — i.e., emit no diagnostic on either literal below.

fn _newline_literal() {
    let _ = "foo\nbar";
}

fn _backslash_literal() {
    // `\\` would normally fire under the default config, but the
    // misconfigured `escapes_eligible = ["\\n"]` overrides the
    // default away — so `\\` is also no longer eligible.
    let _ = "C:\\Users";
}

fn main() {
    _newline_literal();
    _backslash_literal();
}
