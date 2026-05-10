//! `perfectionist` standalone entry point. Resolves a sysroot matching
//! the pinned nightly, then exec's `cargo check` with
//! `perfectionist-driver` slotted in as the rustc wrapper.
//!
//! Pulls the launcher modules in via `#[path]` rather than importing
//! from the `perfectionist` lib crate. The lib crate links to
//! `librustc_driver`, which would otherwise become a runtime
//! dependency of this binary — defeating the whole point of having a
//! launcher that *finds* `librustc_driver` for the user.

#[path = "../launcher/mod.rs"]
mod launcher;

use std::env;
use std::process::ExitCode;

use launcher::cli::Args;
use launcher::orchestrator;

fn main() -> ExitCode {
    let args = Args::from_iter("perfectionist", env::args_os().skip(1));
    orchestrator::run("perfectionist", args)
}
