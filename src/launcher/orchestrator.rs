//! Build a `cargo check` invocation that uses the resolved sysroot's
//! rustc, wraps it with `perfectionist-driver`, and points the dynamic
//! linker at the sysroot's lib dir.

use std::env;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use super::cli::Args;
use super::driver_env::SYSROOT_ENV;
use super::paths::{DYLIB_PATH_ENV, prepend_env_path};
use super::toolchain::{self, Sysroot};
use super::{HOST_TRIPLE, PINNED_TOOLCHAIN};

/// Top-level entry shared by the `perfectionist` and `cargo-perfectionist`
/// binaries. `program` shows up in `--help` and diagnostics.
pub fn run(program: &str, args: Args) -> ExitCode {
    let driver = match locate_driver() {
        Ok(p) => p,
        Err(err) => {
            eprintln!("{program}: cannot locate perfectionist-driver: {err}");
            return ExitCode::from(1);
        }
    };
    let sysroot = match toolchain::resolve(args.offline) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("{program}: {err}");
            return ExitCode::from(1);
        }
    };
    eprintln!(
        "{program}: using {:?} sysroot at {}",
        sysroot.source,
        sysroot.root.display()
    );
    let target_dir = args.target_dir.clone().unwrap_or_else(default_target_dir);

    let status = build_command(&driver, &sysroot, &target_dir, &args.cargo_extra).status();
    match status {
        Ok(s) => match s.code() {
            Some(c) => ExitCode::from(c.clamp(0, 255) as u8),
            None => ExitCode::from(1),
        },
        Err(err) => {
            eprintln!("{program}: failed to spawn cargo: {err}");
            ExitCode::from(1)
        }
    }
}

/// Find the sibling `perfectionist-driver` binary. The launcher and
/// driver are built and shipped together, so the driver is always next
/// to the current executable. Allow `PERFECTIONIST_DRIVER` as an
/// override for development setups (e.g. running `cargo run --bin`).
fn locate_driver() -> io::Result<PathBuf> {
    if let Some(p) = env::var_os("PERFECTIONIST_DRIVER") {
        return Ok(PathBuf::from(p));
    }
    let exe = env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| io::Error::other("current executable has no parent directory"))?;
    let name = if cfg!(windows) {
        "perfectionist-driver.exe"
    } else {
        "perfectionist-driver"
    };
    let candidate = dir.join(name);
    if candidate.is_file() {
        Ok(candidate)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "expected `{}` next to the launcher; not found",
                candidate.display()
            ),
        ))
    }
}

fn build_command(
    driver: &Path,
    sysroot: &Sysroot,
    target_dir: &Path,
    cargo_extra: &[OsString],
) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.arg("check");
    if cargo_extra.iter().all(|a| a != "--all-targets") {
        cmd.arg("--all-targets");
    }
    for arg in cargo_extra {
        cmd.arg(arg);
    }

    cmd.env("RUSTC_WORKSPACE_WRAPPER", driver);
    cmd.env("RUSTC", sysroot.rustc_path());
    cmd.env(SYSROOT_ENV, &sysroot.root);
    cmd.env("CARGO_TARGET_DIR", target_dir);
    cmd.env(
        DYLIB_PATH_ENV,
        prepend_env_path(DYLIB_PATH_ENV, sysroot.lib_dir()),
    );
    // Tools that ride along on cargo's invocation (notably `dylint-link`)
    // expect `RUSTUP_TOOLCHAIN` to identify which toolchain they belong
    // to. Set it to the pinned channel + host triple so rustup-aware
    // tools resolve consistently with our sysroot.
    cmd.env(
        "RUSTUP_TOOLCHAIN",
        format!("{PINNED_TOOLCHAIN}-{HOST_TRIPLE}"),
    );
    // Help cargo find the *right* rustc when it shells out by name.
    if let Some(bin_dir) = sysroot.rustc_path().parent() {
        cmd.env("PATH", prepend_env_path("PATH", bin_dir));
    }
    cmd
}

/// Per-channel sub-directory under the workspace's `target/`. Falls back
/// to the current directory if no workspace can be located, so users
/// running outside a Cargo project still get a deterministic location.
fn default_target_dir() -> PathBuf {
    let workspace = locate_workspace_root().unwrap_or_else(|| PathBuf::from("."));
    workspace
        .join("target")
        .join("perfectionist")
        .join(PINNED_TOOLCHAIN)
}

fn locate_workspace_root() -> Option<PathBuf> {
    let mut dir = env::current_dir().ok()?;
    loop {
        if dir.join("Cargo.toml").is_file() {
            // Walk further up to find the *outermost* Cargo.toml that
            // declares a workspace, falling back to this one if none does.
            let mut outermost = dir.clone();
            let mut probe = dir.parent().map(Path::to_path_buf);
            while let Some(p) = probe {
                if p.join("Cargo.toml").is_file() {
                    outermost = p.clone();
                }
                probe = p.parent().map(Path::to_path_buf);
            }
            return Some(outermost);
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => return None,
        }
    }
}
