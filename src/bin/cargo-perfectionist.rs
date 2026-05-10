//! `cargo perfectionist` subcommand entry point.
//!
//! Cargo invokes `cargo-<sub>` binaries with `<sub>` as the first
//! argument after the program name. We strip that and otherwise behave
//! identically to the standalone `perfectionist` binary.
//!
//! See `perfectionist.rs` for why the launcher modules are included
//! via `#[path]` instead of `use perfectionist::launcher::*`.

#[path = "../launcher/mod.rs"]
mod launcher;

use std::env;
use std::process::ExitCode;

use launcher::cli::Args;
use launcher::orchestrator;

fn main() -> ExitCode {
    let mut iter = env::args_os().skip(1).peekable();
    if iter.peek().is_some_and(|a| a == "perfectionist") {
        iter.next();
    }
    let args = Args::from_iter("cargo perfectionist", iter);
    orchestrator::run("cargo perfectionist", args)
}
