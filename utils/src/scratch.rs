//! One directory under the system temp dir that every scratch
//! file the test suites write is confined to.
//!
//! Left alone, the suites scatter their output across the temp
//! dir itself. `compiletest_rs` — which `dylint_testing` drives —
//! defaults its `build_base` to `std::env::temp_dir()`, so every
//! `ui/` and `ui-toml/` fixture drops a `.err`, `.out`,
//! `.stage-id`, `.stage-id.aux` and `.rs-stage-id.stamp` entry
//! straight into `/tmp`, one set per fixture. The gen-docs unit
//! tests and the fixture projects the integration tests
//! materialise land there too. Confining the lot to one
//! directory keeps `/tmp` readable and makes it removable in a
//! single `rm -rf`.
//!
//! There are two ways in. [`dir`] hands out a scratch directory
//! directly, and is what code that picks its own paths should
//! call. [`redirect_temp_dir`] repoints the process's temp dir at
//! [`root`], for the artifacts written by a dependency that
//! offers no way to say where they go — `compiletest_rs` being
//! the one that motivated this module.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Once};

/// Name of the directory, immediately inside the system temp
/// dir, that holds every scratch file the test suites write.
const DIR_NAME: &str = "perfectionist-tests";

/// Resolved once so [`redirect_temp_dir`] cannot make a later
/// call nest a second `perfectionist-tests` inside the first.
static ROOT: LazyLock<PathBuf> = LazyLock::new(|| {
    let root = std::env::temp_dir().join(DIR_NAME);
    std::fs::create_dir_all(&root)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", root.display()));
    root
});

/// The scratch root, created on first use.
pub fn root() -> &'static Path {
    &ROOT
}

/// Create a fresh, empty scratch directory under [`root`] and
/// return its path.
///
/// The name is `label` followed by the process id and a
/// per-process counter, so neither two test binaries running
/// concurrently nor two tests in one binary that pass the same
/// `label` can collide. Anything left at that path by an earlier
/// run — a process id is reused eventually — is removed first,
/// so the caller always starts from an empty directory.
pub fn dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = root().join(format!("{label}-{}-{seq}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", path.display()));
    path
}

/// Repoint this process's temp dir at [`root`], so a dependency
/// that writes to `std::env::temp_dir()` without asking lands
/// there instead of in `/tmp` proper. Idempotent: only the first
/// call does anything.
///
/// Call this before handing control to `dylint_testing`, whose
/// `compiletest_rs` config takes its `build_base` from
/// `std::env::temp_dir()` at the moment the test runs and
/// exposes no knob to override it.
pub fn redirect_temp_dir() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let root = root();
        // SAFETY: `set_var` races with a concurrent `getenv` in
        // another thread. Callers make that window empty by
        // running this under the same mutex that serialises
        // their calls into `dylint_testing`, which sets
        // `DYLINT_TOML` the same way for the same reason; the
        // binaries with a single `#[test]` have no second thread
        // to race with.
        unsafe {
            // `std::env::temp_dir` reads `TMPDIR` on unix and
            // `TMP`, then `TEMP`, on Windows.
            std::env::set_var("TMPDIR", root);
            std::env::set_var("TMP", root);
            std::env::set_var("TEMP", root);
        }
    });
}
