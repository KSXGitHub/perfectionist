//! Regression test for
//! <https://github.com/KSXGitHub/perfectionist/issues/179>.
//!
//! A bare `http(s)://` URL inside an `include_str!`-ed data file lexes
//! as a `//` line comment (the `//` after the scheme's `:`), but the
//! file is data — not Rust the user wrote — so `bare_url` must neither
//! flag it nor rewrite the included file. This fixture pulls in a YAML
//! file containing bare URLs and expects zero diagnostics.

const DEPRECATION_NOTICE: &str = include_str!("bare_url_include_str.data.yaml");

fn main() {
    let _ = DEPRECATION_NOTICE;
}
