// aux-build:clap.rs
//
//! UI sweep for `clap_help_no_markdown` under the default
//! configuration. Each clap-bound doc comment carrying a forbidden
//! markdown construct is flagged; overrides, `verbatim_doc_comment`,
//! plain prose, and non-clap items are left alone.

#![allow(dead_code, reason = "ui fixture")]

extern crate clap;

use std::path::PathBuf;

/// Path to the [`PackageManifest`].
#[derive(clap::Parser)]
struct Cli {
    /// Path to the `manifest` file.
    code_span: PathBuf,

    /// Path to the [`PackageManifest`].
    intra_doc_link: PathBuf,

    /// See [the format](https://example.com/spec).
    inline_link: PathBuf,

    /// See [the format][fmt].
    ///
    /// [fmt]: https://example.com/spec
    reference_link: PathBuf,

    /// Use <br> to break the line.
    html: PathBuf,

    /// Summary line.
    ///
    /// # Details
    ///
    /// More prose.
    heading: PathBuf,

    /// Example:
    ///
    /// ```
    /// let value = 1;
    /// ```
    code_block: PathBuf,

    /// Path to the package manifest.
    plain_prose: PathBuf,

    /// Path to the `manifest`.
    #[arg(help = "Path to the manifest.")]
    overridden: PathBuf,

    /// Uses a `code` span.
    #[arg(verbatim_doc_comment)]
    verbatim: PathBuf,
}

/// Top-level `command` summary.
#[derive(clap::Subcommand)]
enum Command {
    /// Create the [`Lockfile`].
    Create,

    /// Run with a `--flag`.
    Run,
}

/// A `value` set.
#[derive(clap::ValueEnum)]
#[derive(Clone)]
enum Mode {
    /// The `fast` mode.
    Fast,
    /// The slow mode.
    Slow,
}

/// A non-clap struct with `markdown` is not flagged.
struct NotClap {
    /// Field with a `code` span, but the container is not clap-derived.
    field: PathBuf,
}

fn main() {}
