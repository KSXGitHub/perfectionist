// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// aux-build:clap.rs
//
//! `ignore_constructs = ["inline_link"]` drops inline links from the
//! default set: the code span still fires while the inline link is
//! permitted.

#![allow(dead_code, reason = "ui fixture")]

extern crate clap;

use std::path::PathBuf;

#[derive(clap::Parser)]
struct Cli {
    /// A `code` span and [a link](https://example.com) together.
    field: PathBuf,
}

fn main() {}
