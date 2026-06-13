//! Cantarell webfont provisioning for the HTML catalogue.
//!
//! The page renders its prose in Cantarell, falling back to a downloaded
//! font file when the reader's system lacks it (see `style/base.css`).
//! That file — and the SIL OFL 1.1 license text it must ship under — are
//! deliberately *not* committed to this repository; they live in the
//! sibling KSXGitHub/perfectionist-binary-assets repo. This module:
//!
//! 1. downloads each file in [`DOWNLOADS`] into a local cache directory
//!    ([`cache_dir`] — never `gh-pages/`), but only if it isn't cached
//!    already, so the build hits the network just once and then works
//!    offline, and
//! 2. links the cached files into the output directory beside `index.html`
//!    ([`install_into`]) so the stylesheet's relative `url(...)` resolves
//!    — hard-linking first (cache and output normally share a filesystem)
//!    and falling back to a byte copy across filesystems.

use command_extra::CommandExtra;
use pipe_trait::Pipe;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The files the page downloads and ships beside `index.html`, as
/// `(filename, url)` pairs. `filename` is the name the file is written
/// under in the cache and the output (for the font it must match the name
/// `base.css` references in `url(...)`); `url` is where it is fetched from
/// on a cache miss. Both the variable Cantarell font and the OFL license
/// text it ships under are listed — OFL requires the license to accompany
/// every redistributed copy, and the rendered site is one such copy.
pub(crate) const DOWNLOADS: &[(&str, &str)] = &[
    (
        "cantarell.otf",
        "https://raw.githubusercontent.com/KSXGitHub/perfectionist-binary-assets/master/Cantarell/0.303/Cantarell-VF.otf",
    ),
    (
        "Cantarell-OFL.txt",
        "https://raw.githubusercontent.com/KSXGitHub/perfectionist-binary-assets/master/Cantarell/0.303/COPYING",
    ),
];

/// Environment variable that overrides the default font cache location.
/// Set by callers that want the cache somewhere other than
/// `<root>/.cache/fonts`.
pub(crate) const CACHE_DIR_ENV: &str = "GEN_DOCS_FONT_CACHE_DIR";

/// The cache directory downloads are stored in and served from. Honours
/// [`CACHE_DIR_ENV`] when set; otherwise `<root>/.cache/fonts`, which
/// survives `cargo clean` (unlike anything under `target/`) so the warm
/// cache persists across builds.
pub(crate) fn cache_dir(root: &Path) -> PathBuf {
    match std::env::var_os(CACHE_DIR_ENV) {
        Some(dir) => PathBuf::from(dir),
        None => root.join(".cache").join("fonts"),
    }
}

/// Ensure every download is present in the cache directory, fetching only
/// the ones that aren't there yet. Presence is the whole check: a file
/// already in the cache is taken as-is (the offline / no-network path).
/// Downloads land via an atomic rename, so a cached file is always a
/// complete one — an interrupted fetch leaves no half-written entry for a
/// later run to mistake for cached.
pub(crate) fn ensure_cached(cache_dir: &Path, downloads: &[(&str, &str)]) -> Result<(), String> {
    std::fs::create_dir_all(cache_dir).map_err(|error| {
        format!(
            "failed to create font cache {}: {error}",
            cache_dir.display(),
        )
    })?;
    for &(filename, url) in downloads {
        let path = cache_dir.join(filename);
        if path.exists() {
            continue;
        }
        download(url, &path)?;
    }
    Ok(())
}

/// Link each cached download into `out_dir` so it lands beside
/// `index.html`. Hard-links first — the cache and the output directory
/// normally share a filesystem, so this avoids duplicating the bytes —
/// and falls back to a byte copy when hard-linking isn't possible (e.g.
/// the output is on a different mount). Assumes [`ensure_cached`] has
/// already populated the cache directory.
pub(crate) fn install_into(
    out_dir: &Path,
    cache_dir: &Path,
    downloads: &[(&str, &str)],
) -> Result<(), String> {
    for &(filename, _) in downloads {
        let source = cache_dir.join(filename);
        let dest = out_dir.join(filename);
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

/// Download one file to `dest`. The bytes are written to a sibling temp
/// file and renamed into place only after `curl` succeeds, so an
/// interrupted download never leaves a truncated file that
/// [`ensure_cached`]'s existence check would later trust.
fn download(url: &str, dest: &Path) -> Result<(), String> {
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
        .with_arg(url)
        .status()
        .map_err(|error| {
            format!(
                "failed to run curl to download {url} (is curl installed, and is the \
                 network reachable? once cached, builds need neither): {error}",
            )
        })?;
    if !status.success() {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("curl failed ({status}) downloading {url}"));
    }
    std::fs::rename(&temp, dest).map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        format!(
            "failed to move downloaded file into cache at {}: {error}",
            dest.display(),
        )
    })
}

#[cfg(test)]
mod tests;
