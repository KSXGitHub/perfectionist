// Good: with `allow_http = false`, `http://` is not in the scheme
// set. The next line must not fire even though it's a bare URL by
// default config: http://rust-lang.org.
fn _http_not_flagged() {}

/// Bad: `https://` is still flagged regardless of `allow_http`.
/// Reference: https://rust-lang.org.
fn _https_still_flagged() {}

fn main() {}
