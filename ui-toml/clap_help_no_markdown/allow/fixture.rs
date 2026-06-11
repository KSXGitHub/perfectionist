// aux-build:clap.rs
//
//! `allow = ["inline_link"]` drops inline links from the default set:
//! the code span still fires while the inline link is permitted.

#![allow(dead_code, reason = "ui fixture")]

extern crate clap;

use std::path::PathBuf;

#[derive(clap::Parser)]
struct Cli {
    /// A `code` span and [a link](https://example.com) together.
    field: PathBuf,
}

fn main() {}
