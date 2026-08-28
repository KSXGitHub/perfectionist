//! Enforce the version-bump contract: a release commit (and its tag)
//! must satisfy
//!
//! 1. The tag is `X.Y.Z` or `X.Y.Z-<suffix>`.
//! 2. The commit message equals the tag exactly.
//! 3. The `[package].version` field in `Cargo.toml` equals the tag.
//! 4. The `perfectionist` package's `version` field in `Cargo.lock`
//!    equals the tag.
//! 5. The commit's diff against its parent modifies *only* `Cargo.toml`
//!    and `Cargo.lock`, and *only* on the two `version` lines above —
//!    no other line in either file is altered.
//!
//! The same check is reached from each of these entry points:
//!
//! * `verify <version>` — deploy CI (`HEAD` is the tagged commit).
//! * `commit-msg <file>` — `commit-msg` git hook (index vs. `HEAD`,
//!   when the typed message looks like a version literal).
//! * `pre-push` — `pre-push` git hook (each tag-ref update fed on
//!   stdin).
//!
//! When any of those fires on a release-shaped operation, this tool
//! exits non-zero so the operation aborts.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

mod contract;
mod error;
mod git;
mod hook;
mod manifest;
mod version_literal;

#[cfg(test)]
mod tests;

use contract::{Source, verify};
use error::RuntimeError;
use hook::{commit_msg, pre_push};

pub(crate) const PACKAGE_NAME: &str = "perfectionist";

#[derive(Parser)]
#[clap(about = "Validate the version-bump contract for deploy CI and git hooks")]
struct Cli {
    #[clap(help = "The root of the repository")]
    root: PathBuf,
    #[clap(subcommand)]
    command: Sub,
}

#[derive(Subcommand)]
enum Sub {
    #[clap(about = "Verify a commit against the version-bump contract")]
    Verify {
        #[clap(help = "Version literal to validate against (e.g. the tag name)")]
        version: String,
        #[clap(
            long,
            help = "Revision to verify (default: HEAD)",
            conflicts_with = "cached"
        )]
        commit: Option<String>,
        #[clap(
            long,
            help = "Verify the staged index against HEAD instead of a real commit"
        )]
        cached: bool,
    },
    #[clap(about = "commit-msg git-hook entry point")]
    CommitMsg {
        #[clap(help = "Path to the commit-message file passed by git")]
        msg_file: PathBuf,
    },
    #[clap(about = "pre-push git-hook entry point (reads ref updates from stdin)")]
    PrePush,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("deploy-check: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(Cli { root, command }: Cli) -> Result<(), RuntimeError> {
    match command {
        Sub::Verify {
            version,
            commit,
            cached,
        } => {
            let source = if cached {
                Source::Cached
            } else {
                Source::Commit(commit.unwrap_or_else(|| "HEAD".into()))
            };
            verify(&root, &version, &source)
        }
        Sub::CommitMsg { msg_file } => commit_msg(&root, &msg_file),
        Sub::PrePush => pre_push(&root),
    }
}
