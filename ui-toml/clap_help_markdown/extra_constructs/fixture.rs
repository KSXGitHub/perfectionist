// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
// aux-build:clap.rs
//
//! `extra_constructs = ["bold", "italic", "list"]` flags emphasis and
//! list markers that the default configuration leaves alone.

#![allow(dead_code, reason = "ui fixture")]

extern crate clap;

use std::path::PathBuf;

#[derive(clap::Parser)]
struct Cli {
    /// Uses **bold** emphasis.
    bold: PathBuf,

    /// Uses *italic* emphasis.
    italic: PathBuf,

    /// Options:
    ///
    /// - first
    /// - second
    list: PathBuf,
}

fn main() {}
