// aux-build:clap.rs
//
//! `require_help_override = true` flags every clap-derived doc comment
//! that reaches `--help` without an explicit override, and tolerates a
//! missing override wherever there is no doc comment to leak. The
//! markdown scan is superseded: the `` `code` `` span below is reported
//! once as a missing override, not as a code span.

#![allow(dead_code, reason = "ui fixture")]

extern crate clap;

use std::path::PathBuf;

/// The command's own description.
#[derive(clap::Parser)]
struct Cli {
    /// Path to the manifest.
    manifest: PathBuf,

    /// Enables verbose output.
    #[arg(help = "Enables verbose output.")]
    verbose: bool,

    #[arg(help = "Output directory.")]
    out: PathBuf,

    positional: PathBuf,

    /// Uses a `code` span too.
    tricky: PathBuf,
}

fn main() {}
