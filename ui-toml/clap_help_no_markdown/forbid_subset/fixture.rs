// aux-build:clap.rs
//
//! `forbid = ["code_span"]` narrows the rule: a code span is flagged
//! while an inline link, dropped from the set, is not.

#![allow(dead_code, reason = "ui fixture")]

extern crate clap;

use std::path::PathBuf;

#[derive(clap::Parser)]
struct Cli {
    /// A `code` span and [a link](https://example.com) together.
    field: PathBuf,
}

fn main() {}
