//! Run `git` as a child process and read what it reports: its stdout,
//! or just whether it succeeded.

use super::error::RuntimeError;
use command_extra::CommandExtra;
use pipe_trait::Pipe;
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};

pub(crate) fn git_capture<Args, Arg>(root: &Path, args: Args) -> Result<String, RuntimeError>
where
    Args: IntoIterator<Item = Arg>,
    Arg: AsRef<OsStr>,
{
    let output = "git"
        .pipe(Command::new)
        .with_current_dir(root)
        .with_args(args)
        .output()
        .map_err(RuntimeError::SpawnGit)?;
    if !output.status.success() {
        return Err(RuntimeError::GitFailed {
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    String::from_utf8(output.stdout).map_err(|_| RuntimeError::GitNonUtf8)
}

/// Whether `rev` resolves in this checkout.
///
/// `rev-parse --verify --quiet` prints nothing and exits non-zero when
/// it does not, which [`git_capture`] would report as a failed git run,
/// so this reads the exit status instead.
pub(crate) fn git_rev_present(root: &Path, rev: &str) -> Result<bool, RuntimeError> {
    let status = "git"
        .pipe(Command::new)
        .with_current_dir(root)
        .with_args(["rev-parse", "--verify", "--quiet", rev])
        .with_stdout(Stdio::null())
        .status()
        .map_err(RuntimeError::SpawnGit)?;
    Ok(status.success())
}

/// Whether this checkout was cloned with `--depth`, so that the
/// ancestors of its boundary commits are absent from the object
/// database.
pub(crate) fn git_is_shallow(root: &Path) -> Result<bool, RuntimeError> {
    let stdout = git_capture(root, ["rev-parse", "--is-shallow-repository"])?;
    Ok(stdout.trim() == "true")
}
