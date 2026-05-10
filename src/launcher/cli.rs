//! Argument parsing for the `perfectionist` and `cargo perfectionist`
//! entry points.

use std::ffi::OsString;
use std::path::PathBuf;

use clap::Parser;

use super::driver_env::TARGET_DIR_ENV;

/// Run perfectionist's lints over a Rust project.
///
/// Internally invokes `cargo check` with a custom rustc wrapper that
/// registers the perfectionist lints. Arguments after `--` are
/// forwarded to the inner cargo invocation.
#[derive(Debug, Parser)]
#[command(
    version,
    long_about = None,
    after_help = "EXAMPLES:\n  \
        perfectionist\n  \
        perfectionist --target-dir target/perf\n  \
        perfectionist -- --all-features -p my-crate"
)]
pub struct Args {
    /// Build-artifact cache directory.
    ///
    /// Defaults to `<workspace>/target/perfectionist/<channel>/`.
    /// Deliberately separate from `CARGO_TARGET_DIR` to avoid
    /// fingerprint thrash against the user's normal builds.
    #[arg(long, value_name = "PATH", env = TARGET_DIR_ENV)]
    pub target_dir: Option<PathBuf>,

    /// Skip the on-demand sysroot download. Fail if no rustup-installed
    /// or previously-cached toolchain is available.
    #[arg(long)]
    pub offline: bool,

    /// Arguments forwarded to `cargo check` after a `--` separator.
    #[arg(last = true, value_name = "CARGO_ARGS")]
    pub cargo_extra: Vec<OsString>,
}

impl Args {
    /// Parse argv after stripping the program name (and, for the cargo
    /// subcommand, the leading "perfectionist" trampoline arg).
    pub fn from_iter<I, T>(program: &str, iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        // Inject `program` as argv[0] so clap's own --help / error
        // messages name the right command (`perfectionist` vs
        // `cargo perfectionist`).
        let argv = std::iter::once(OsString::from(program)).chain(iter.into_iter().map(Into::into));
        Self::parse_from(argv)
    }
}
