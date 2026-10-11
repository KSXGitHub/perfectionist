//! Regression test for the `include_str!` exclusion, the same class of
//! bug as <https://github.com/KSXGitHub/perfectionist/issues/179>.
//!
//! The rule walks source text rather than the HIR, and a quoted string
//! inside an `include_str!`-ed data file lexes as a Rust string literal
//! like any other, escapes included. The file is data rather than Rust
//! the user wrote, so an escape in it must not be flagged, and above
//! all must not be rewritten: the suggestion would edit a YAML file
//! through a Rust lint. This fixture pulls in a YAML file carrying a
//! `\u{...}` and a `\x..` escape of a visible character and expects
//! zero diagnostics.
#![allow(dead_code, unused, reason = "ui fixture")]

const LABELS: &str = include_str!("numeric_escape_for_visible_character_include_str.data.yaml");

fn main() {}
