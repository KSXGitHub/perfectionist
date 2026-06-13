//! Cantarell webfont provisioning for the HTML catalogue.
//!
//! The page renders its prose in Cantarell, falling back to a downloaded
//! font file when the reader's system lacks it (see `style/base.css`).
//! That binary is deliberately *not* committed to this repository (it
//! lives in the sibling KSXGitHub/perfectionist-binary-assets repo).
//! Instead this module:
//!
//! 1. downloads each font listed in [`FONT_LOCK`] on a cache miss, into a
//!    local cache directory ([`cache_dir`] — never `gh-pages/`),
//! 2. verifies every file against the SHA-256 pinned in the lockfile, so
//!    a truncated or tampered download is rejected, and
//! 3. reuses the cache on later runs, so the build hits the network only
//!    the first time and works offline once the cache is warm.
//!
//! The cached files are then linked into the output directory beside
//! `index.html` ([`install_into`]) so the stylesheet's relative `url(...)`
//! resolves — hard-linking first (cache and output normally share a
//! filesystem) and falling back to a byte copy across filesystems.

use command_extra::CommandExtra;
use pipe_trait::Pipe;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The committed lockfile pinning every font the page downloads. It is
/// the single source of truth shared by this module — which parses it —
/// and CI, which hashes it to key the font cache. Embedded so the binary
/// needs no runtime file lookup; `../fonts.lock` is the crate root.
pub(crate) const FONT_LOCK: &str = include_str!("../fonts.lock");

/// Environment variable that overrides the default font cache location.
/// Set by callers (or CI) that want the cache somewhere other than
/// `<root>/.cache/fonts`.
pub(crate) const CACHE_DIR_ENV: &str = "GEN_DOCS_FONT_CACHE_DIR";

/// One pinned font file: its expected SHA-256 (lowercase hex), the name
/// it is written under (matched by `base.css`'s `url(...)`), and the URL
/// it is fetched from on a cache miss.
#[derive(Debug)]
pub(crate) struct FontEntry {
    pub sha256: String,
    pub filename: String,
    pub url: String,
}

/// Parse [`FONT_LOCK`] into its entries. Blank lines and `#` comments are
/// skipped; every other line must be `<sha256> <filename> <url>`.
/// Panics on a malformed line — the lockfile is committed, so a bad line
/// is a build-breaking authoring error, not a runtime condition.
pub(crate) fn font_entries() -> Vec<FontEntry> {
    FONT_LOCK
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.split_whitespace();
            let sha256 = fields.next();
            let filename = fields.next();
            let url = fields.next();
            match (sha256, filename, url, fields.next()) {
                (Some(sha256), Some(filename), Some(url), None) => FontEntry {
                    sha256: sha256.to_ascii_lowercase(),
                    filename: filename.to_owned(),
                    url: url.to_owned(),
                },
                _ => panic!(
                    "malformed fonts.lock line (expected `<sha256> <filename> <url>`): {line:?}",
                ),
            }
        })
        .collect()
}

/// The cache directory fonts are downloaded into and served from.
/// Honours [`CACHE_DIR_ENV`] when set; otherwise `<root>/.cache/fonts`,
/// which survives `cargo clean` (unlike anything under `target/`) so the
/// warm cache persists across builds, and is easy for CI to cache by a
/// fixed path.
pub(crate) fn cache_dir(root: &Path) -> PathBuf {
    match std::env::var_os(CACHE_DIR_ENV) {
        Some(dir) => PathBuf::from(dir),
        None => root.join(".cache").join("fonts"),
    }
}

/// Ensure every entry is present and valid in the cache directory,
/// downloading any that are missing or whose bytes don't match the pinned
/// hash. After this returns `Ok`, the `<filename>` of every entry exists
/// under the cache directory and verifies.
pub(crate) fn ensure_cached(cache_dir: &Path, entries: &[FontEntry]) -> Result<(), String> {
    std::fs::create_dir_all(cache_dir).map_err(|error| {
        format!(
            "failed to create font cache {}: {error}",
            cache_dir.display(),
        )
    })?;
    for entry in entries {
        let path = cache_dir.join(&entry.filename);
        // A cached file that already matches its pinned hash is reused
        // verbatim — this is the offline / no-network path.
        if let Ok(bytes) = std::fs::read(&path)
            && hash_matches(&bytes, &entry.sha256)
        {
            continue;
        }
        download_and_verify(entry, &path)?;
    }
    Ok(())
}

/// Link each cached font into `out_dir` so it lands beside `index.html`.
/// Hard-links first — the cache and the output directory normally share a
/// filesystem, so this avoids duplicating the bytes — and falls back to a
/// byte copy when hard-linking isn't possible (e.g. the output is on a
/// different mount). Assumes [`ensure_cached`] has already populated the
/// cache directory.
pub(crate) fn install_into(
    out_dir: &Path,
    cache_dir: &Path,
    entries: &[FontEntry],
) -> Result<(), String> {
    for entry in entries {
        let source = cache_dir.join(&entry.filename);
        let dest = out_dir.join(&entry.filename);
        // `hard_link` fails if the destination already exists, and a
        // stale copy from a previous run would otherwise shadow the
        // fresh cache — remove it first. `copy` would overwrite, but
        // clearing unconditionally keeps both branches identical.
        if let Err(error) = std::fs::remove_file(&dest)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(format!(
                "failed to clear stale font {}: {error}",
                dest.display(),
            ));
        }
        if std::fs::hard_link(&source, &dest).is_ok() {
            continue;
        }
        std::fs::copy(&source, &dest).map_err(|error| {
            format!(
                "failed to install font {} -> {}: {error}",
                source.display(),
                dest.display(),
            )
        })?;
    }
    Ok(())
}

/// Download one font to `dest`, verifying its SHA-256 before committing it
/// to the cache. The bytes are written to a sibling temp file and renamed
/// into place only after the hash checks out, so an interrupted or corrupt
/// download never leaves a poisoned cache entry that a later run would
/// trust.
fn download_and_verify(entry: &FontEntry, dest: &Path) -> Result<(), String> {
    let temp = dest.with_extension("download");
    // Shell out to `curl`, matching how the rest of this tool reaches
    // external commands (`git` in main.rs) and how CI installs rustup —
    // no HTTP/TLS crate is pulled into the workspace for one fetch.
    let status = "curl"
        .pipe(Command::new)
        .with_arg("--proto")
        .with_arg("=https")
        .with_arg("--tlsv1.2")
        .with_arg("--silent")
        .with_arg("--show-error")
        .with_arg("--fail")
        .with_arg("--location")
        .with_arg("--output")
        .with_arg(&temp)
        .with_arg(&entry.url)
        .status()
        .map_err(|error| {
            format!(
                "failed to run curl to download {} (is curl installed, and is the \
                 network reachable? once cached, builds need neither): {error}",
                entry.url,
            )
        })?;
    if !status.success() {
        let _ = std::fs::remove_file(&temp);
        return Err(format!(
            "curl failed ({status}) downloading font from {}",
            entry.url,
        ));
    }
    let bytes = std::fs::read(&temp)
        .map_err(|error| format!("failed to read downloaded font {}: {error}", temp.display()))?;
    if !hash_matches(&bytes, &entry.sha256) {
        let _ = std::fs::remove_file(&temp);
        return Err(format!(
            "downloaded font {} has SHA-256 {}, expected {} (pinned in fonts.lock)",
            entry.filename,
            hex_sha256(&bytes),
            entry.sha256,
        ));
    }
    std::fs::rename(&temp, dest).map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        format!(
            "failed to move verified font into cache at {}: {error}",
            dest.display(),
        )
    })
}

/// Whether `bytes` hash to `expected` (lowercase hex SHA-256). The
/// comparison is case-insensitive so a lockfile authored with uppercase
/// digits still matches.
fn hash_matches(bytes: &[u8], expected: &str) -> bool {
    hex_sha256(bytes).eq_ignore_ascii_case(expected)
}

/// The lowercase hex SHA-256 of `bytes`.
fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hex
}

#[cfg(test)]
mod tests;
