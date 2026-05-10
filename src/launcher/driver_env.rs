//! Env-var contract between the launcher and the rustc-driver binary.
//!
//! The launcher always sets these before exec'ing `cargo check`; the
//! driver reads them when cargo invokes it as `RUSTC_WORKSPACE_WRAPPER`.
//! Keeping the contract in one place avoids the two binaries drifting.

// `#[path]`-included from both the launcher binaries and the driver
// binary; each consumer uses a different subset of the constants.
#![allow(dead_code)]

/// Absolute path to the resolved sysroot. The driver injects this as
/// `--sysroot=<value>` if the rustc command line cargo handed it does
/// not already specify one. Cargo passes `--sysroot` for some
/// invocations but not all (notably `--print` queries), so the driver
/// has to defensively add it.
pub const SYSROOT_ENV: &str = "PERFECTIONIST_SYSROOT";

/// Override for the per-project build cache directory. Resolution
/// order in the launcher is: `--target-dir` flag, then this env, then
/// the default under the workspace's `target/perfectionist/<channel>/`.
/// Deliberately *not* derived from `CARGO_TARGET_DIR` — the user's
/// normal-build target dir would thrash against perfectionist's
/// nightly-rustc fingerprints.
pub const TARGET_DIR_ENV: &str = "PERFECTIONIST_TARGET_DIR";
