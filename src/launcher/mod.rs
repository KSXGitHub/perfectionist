//! Standalone-binary support: locate (or download) a sysroot whose rustc
//! version matches the one this crate was built against, then orchestrate
//! `cargo check` with `perfectionist-driver` slotted in as
//! `RUSTC_WORKSPACE_WRAPPER`.
//!
//! Layered intentionally so the rustc-driver binary depends only on
//! [`driver_env`] (a couple of env-var names) and the orchestrator binaries
//! depend on the rest.

pub mod cli;
pub mod download;
pub mod driver_env;
pub mod orchestrator;
pub mod paths;
pub mod toolchain;

/// Pinned nightly channel, captured from `rust-toolchain` at build time.
/// Embedded so the launcher can resolve a matching sysroot at runtime
/// without needing the source tree on disk.
pub const PINNED_TOOLCHAIN: &str = env!("PERFECTIONIST_PINNED_TOOLCHAIN");

/// Host triple this binary was built for. Used to construct distribution
/// URLs (`<triple>` segment) and to locate rustup-installed sysroots.
pub const HOST_TRIPLE: &str = env!("PERFECTIONIST_HOST_TRIPLE");
