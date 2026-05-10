//! Filesystem path conventions: the per-user cache root, the dynamic-
//! library env-var name for the host platform, and helpers for prepending
//! to a `PATH`-style env value.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

/// Root of perfectionist's per-user cache.
///
/// Resolution order: `PERFECTIONIST_CACHE_DIR`, `XDG_CACHE_HOME`/perfectionist,
/// `~/.cache/perfectionist` on Unix, `%LOCALAPPDATA%/perfectionist/cache` on
/// Windows, or the OS temp dir as a last resort so the launcher never panics
/// in unusual environments (containers without `HOME`, etc.).
pub fn cache_root() -> PathBuf {
    if let Some(explicit) = env::var_os("PERFECTIONIST_CACHE_DIR") {
        return PathBuf::from(explicit);
    }
    if cfg!(windows) {
        if let Some(local) = env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local).join("perfectionist").join("cache");
        }
    } else if let Some(xdg) = env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(xdg).join("perfectionist");
    } else if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".cache").join("perfectionist");
    }
    env::temp_dir().join("perfectionist")
}

/// Cache subdirectory holding a downloaded sysroot for a given channel +
/// host triple. A sentinel file inside this directory marks "ready"; see
/// [`download::READY_MARKER`](super::download::READY_MARKER).
pub fn toolchain_cache_dir(channel: &str, host: &str) -> PathBuf {
    cache_root()
        .join("toolchains")
        .join(format!("{channel}-{host}"))
}

/// Name of the env var that the dynamic linker consults for extra search
/// directories on the current platform.
pub const DYLIB_PATH_ENV: &str = if cfg!(target_os = "macos") {
    "DYLD_FALLBACK_LIBRARY_PATH"
} else if cfg!(windows) {
    // Windows has no rpath equivalent; the executable's own directory is
    // searched first, but for `librustc_driver` we keep the driver in the
    // sysroot's `bin/` dir already (Rust's Windows convention) and also
    // prepend `PATH` so cargo's invocations resolve dependencies.
    "PATH"
} else {
    "LD_LIBRARY_PATH"
};

/// Prepend `entry` to a `PATH`-style env var, preserving any previous
/// value with the platform-correct separator.
pub fn prepend_env_path<S: AsRef<OsStr>>(var_name: &str, entry: S) -> OsString {
    let mut joined = OsString::new();
    joined.push(entry.as_ref());
    if let Some(existing) = env::var_os(var_name)
        && !existing.is_empty()
    {
        joined.push(if cfg!(windows) { ";" } else { ":" });
        joined.push(existing);
    }
    joined
}
