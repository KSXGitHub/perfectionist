//! Run `git` as a child process and capture its stdout.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

use command_extra::CommandExtra;
use pipe_trait::Pipe;

use super::error::RuntimeError;

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
