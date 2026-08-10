// aux-build:clap.rs
//
//! UI sweep for `clap_help_markdown` under the default
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

    /** Block doc with a `code` span. */
    block_doc: PathBuf,

    /// First run with a `first` span.
    #[arg(long)]
    /// Second run with a `second` span, after an interleaved attribute.
    split_doc: PathBuf,
}

/// Top-level `command` summary.
#[derive(clap::Subcommand)]
enum Command {
    /// Create the [`Lockfile`].
    Create,

    /// Run with a `--flag`.
    Run,
}

/// A `value` set: a `ValueEnum`'s enum-level doc never reaches `--help`,
/// so this `code` span is not flagged.
#[derive(clap::ValueEnum)]
#[derive(Clone)]
enum Mode {
    /// The `fast` mode.
    Fast,
    /// The slow mode.
    Slow,
}

/// The [`DisplayFormat`] of [`Bytes`]: gated behind a `cli` feature, and
/// — as a `ValueEnum` container doc — never reaching `--help`.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Clone)]
enum BytesFormat {
    /// Display the value with a unit suffix in [metric scale](fmt::METRIC).
    #[cfg_attr(
        feature = "cli",
        clap(name = "metric", help = "Use metric scale", alias = "1000")
    )]
    MetricUnits,

    /// Display the value with a unit suffix in [binary scale](fmt::BINARY).
    #[cfg_attr(
        feature = "cli",
        clap(name = "binary", help = "Use binary scale", alias = "1024")
    )]
    BinaryUnits,

    /// Plain [`number`] of bytes, with no help override.
    #[cfg_attr(feature = "cli", clap(name = "plain"))]
    PlainNumber,
}

/// A non-clap struct with `markdown` is not flagged.
struct NotClap {
    /// Field with a `code` span, but the container is not clap-derived.
    field: PathBuf,
}

fn main() {}
