//! Resolve a sysroot whose rustc matches `PINNED_TOOLCHAIN`.
//!
//! Tries three sources in order so that users with rustup pay no
//! download cost, and CI / fresh installs get the toolchain on demand:
//!
//! 1. Previously-cached download under `cache_root()/toolchains/`.
//! 2. Rustup-installed toolchain under `$RUSTUP_HOME/toolchains/`.
//! 3. Fresh download from `static.rust-lang.org`.
//!
//! The cache is checked before rustup because rustup toolchain locations
//! can shift if the user runs `rustup default` or `rustup uninstall`,
//! whereas our cache directory is stable. Either source produces a path
//! we treat the same: the directory containing `bin/`, `lib/`, etc.

use std::env;
use std::io;
use std::path::{Path, PathBuf};

use super::download;
use super::paths;
use super::{HOST_TRIPLE, PINNED_TOOLCHAIN};

/// A resolved sysroot: the directory containing `bin/rustc{,.exe}`,
/// `lib/librustc_driver-*.{so,dylib,dll}`, and `lib/rustlib/<triple>/`.
#[derive(Debug, Clone)]
pub struct Sysroot {
    pub root: PathBuf,
    /// Where the resolution came from. Surfaced in diagnostics so users
    /// can tell whether the launcher used their rustup or its own cache.
    pub source: Source,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Cache,
    Rustup,
    Downloaded,
}

impl Sysroot {
    pub fn rustc_path(&self) -> PathBuf {
        self.root
            .join("bin")
            .join(if cfg!(windows) { "rustc.exe" } else { "rustc" })
    }

    pub fn lib_dir(&self) -> PathBuf {
        // On Windows, `librustc_driver` is a DLL that lives in `bin/`
        // alongside `rustc.exe`; that's the rustup convention. On Unix
        // it sits in `lib/`.
        if cfg!(windows) {
            self.root.join("bin")
        } else {
            self.root.join("lib")
        }
    }
}

/// Top-level resolver. `offline=true` skips step 3 and surfaces an error
/// instead of fetching the network.
pub fn resolve(offline: bool) -> io::Result<Sysroot> {
    if let Some(found) = check_cache()? {
        return Ok(Sysroot {
            root: found,
            source: Source::Cache,
        });
    }
    if let Some(found) = check_rustup()? {
        return Ok(Sysroot {
            root: found,
            source: Source::Rustup,
        });
    }
    if offline {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "no cached sysroot for `{PINNED_TOOLCHAIN}` ({HOST_TRIPLE}) and \
                 --offline was requested. Install via rustup:\n  \
                 rustup toolchain install {PINNED_TOOLCHAIN}"
            ),
        ));
    }
    let path = download::install(PINNED_TOOLCHAIN, HOST_TRIPLE)?;
    Ok(Sysroot {
        root: path,
        source: Source::Downloaded,
    })
}

fn check_cache() -> io::Result<Option<PathBuf>> {
    let dir = paths::toolchain_cache_dir(PINNED_TOOLCHAIN, HOST_TRIPLE);
    if download::is_ready(&dir)? && looks_like_sysroot(&dir) {
        Ok(Some(dir))
    } else {
        Ok(None)
    }
}

fn check_rustup() -> io::Result<Option<PathBuf>> {
    let rustup_home = env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".rustup")));
    let Some(rustup_home) = rustup_home else {
        return Ok(None);
    };
    let candidate = rustup_home
        .join("toolchains")
        .join(format!("{PINNED_TOOLCHAIN}-{HOST_TRIPLE}"));
    if looks_like_sysroot(&candidate) {
        Ok(Some(candidate))
    } else {
        Ok(None)
    }
}

fn looks_like_sysroot(root: &Path) -> bool {
    // Sniff for the two artefacts the driver definitely needs: the
    // `rustc` shim under `bin/` (used by cargo for metadata queries)
    // and the per-target `rustlib` dir (used by the typecheck for std
    // rlibs). If both are present, the directory is structurally a
    // sysroot regardless of how it got there.
    let rustc = root
        .join("bin")
        .join(if cfg!(windows) { "rustc.exe" } else { "rustc" });
    let rustlib = root.join("lib").join("rustlib").join(HOST_TRIPLE);
    rustc.is_file() && rustlib.is_dir()
}
