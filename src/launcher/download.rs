//! Fetch a sysroot from `static.rust-lang.org` on demand.
//!
//! Flow:
//!
//! 1. Translate `nightly-YYYY-MM-DD` into a date-keyed dist URL.
//! 2. Download the channel manifest (`channel-rust-nightly.toml`) and
//!    pull out the per-component tarball URL + sha256 for our host.
//! 3. For each required component (`rustc`, `rust-std`), stream-decode
//!    + verify + extract directly into the cache directory.
//! 4. Drop a sentinel file so subsequent runs short-circuit.
//!
//! Atomicity: extraction happens into a sibling temp directory and is
//! `rename`d on success. Two concurrent launchers may race; the loser's
//! rename will fail harmlessly and they'll fall back to the winner's
//! directory.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::paths;

/// Sentinel file written into a cache directory once extraction
/// completes. Its presence marks the directory as ready to use.
pub const READY_MARKER: &str = ".perfectionist-ready";

/// Components to fetch. `rustc` provides `librustc_driver`, `libLLVM`,
/// and the runtime `libstd`; `rust-std` provides the `.rlib`s the
/// driver needs to typecheck the user's crate. We deliberately do NOT
/// fetch `rustc-dev` — those rlibs are build-time-only for our driver,
/// and the driver is already linked by the time it runs.
const COMPONENTS: &[&str] = &["rustc", "rust-std"];

pub fn is_ready(dir: &Path) -> io::Result<bool> {
    Ok(dir.join(READY_MARKER).is_file())
}

/// Top-level: ensure a ready cache directory exists for `(channel, host)`
/// and return its path. Idempotent.
pub fn install(channel: &str, host: &str) -> io::Result<PathBuf> {
    let final_dir = paths::toolchain_cache_dir(channel, host);
    if is_ready(&final_dir)? {
        return Ok(final_dir);
    }

    let date = channel_date(channel).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "channel `{channel}` is not a dated nightly; \
                 the launcher only knows how to download `nightly-YYYY-MM-DD`"
            ),
        )
    })?;

    let parent = final_dir
        .parent()
        .ok_or_else(|| io::Error::other("toolchain cache dir has no parent"))?;
    fs::create_dir_all(parent)?;
    let staging = parent.join(format!(".staging-{channel}-{host}"));
    if staging.exists() {
        // Leftover from an interrupted previous run. Wipe and retry.
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    eprintln!(
        "perfectionist: fetching toolchain `{channel}` for `{host}` \
         (one-time, ~150 MB compressed)"
    );
    let manifest = fetch_manifest(date)?;
    for component in COMPONENTS {
        let (url, hash) = manifest.package(component, host).ok_or_else(|| {
            io::Error::other(format!(
                "channel manifest does not list `{component}` for `{host}`"
            ))
        })?;
        eprintln!("perfectionist:   {component}: {url}");
        let body = http_get(url)?;
        verify_sha256(&body, hash, component)?;
        extract_tarball(&body, &staging)?;
    }

    write_ready_marker(&staging)?;

    // Final atomic publish. If a competing launcher beat us to it, the
    // rename fails; treat that as success and discard our copy.
    match fs::rename(&staging, &final_dir) {
        Ok(()) => {}
        Err(_) if is_ready(&final_dir)? => {
            let _ = fs::remove_dir_all(&staging);
        }
        Err(err) => return Err(err),
    }
    Ok(final_dir)
}

fn write_ready_marker(dir: &Path) -> io::Result<()> {
    let mut f = File::create(dir.join(READY_MARKER))?;
    writeln!(f, "ready")?;
    Ok(())
}

/// Strip the `nightly-` prefix to get the dist date segment.
/// `nightly` (no date) is rejected — the launcher needs an immutable
/// pin so the bundled driver's hash matches the toolchain's hash.
fn channel_date(channel: &str) -> Option<&str> {
    let date = channel.strip_prefix("nightly-")?;
    let bytes = date.as_bytes();
    let looks_dated = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..].iter().all(u8::is_ascii_digit);
    looks_dated.then_some(date)
}

// ------------- channel manifest -------------
//
// The `channel-rust-nightly.toml` manifest is large (every package, every
// target triple), but we only need the URL + sha256 for two packages on
// our host. The serde shape below mirrors just that subset; everything
// else is ignored.

#[derive(Debug, Deserialize)]
struct Manifest {
    pkg: BTreeMap<String, Pkg>,
}

#[derive(Debug, Deserialize)]
struct Pkg {
    target: BTreeMap<String, Target>,
}

#[derive(Debug, Deserialize)]
struct Target {
    available: bool,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    hash: Option<String>,
}

impl Manifest {
    fn package(&self, name: &str, target: &str) -> Option<(&str, &str)> {
        let t = self.pkg.get(name)?.target.get(target)?;
        if !t.available {
            return None;
        }
        Some((t.url.as_deref()?, t.hash.as_deref()?))
    }
}

fn fetch_manifest(date: &str) -> io::Result<Manifest> {
    let url = format!("https://static.rust-lang.org/dist/{date}/channel-rust-nightly.toml");
    let body = http_get(&url)?;
    let text = std::str::from_utf8(&body).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("manifest not utf-8: {err}"),
        )
    })?;
    toml::from_str(text)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, format!("manifest parse: {err}")))
}

// ------------- http -------------

fn http_get(url: &str) -> io::Result<Vec<u8>> {
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(60))
        .call()
        .map_err(|err| io::Error::other(format!("GET {url}: {err}")))?;
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .map_err(|err| io::Error::other(format!("body read {url}: {err}")))?;
    Ok(buf)
}

// ------------- verification -------------

fn verify_sha256(body: &[u8], expected: &str, label: &str) -> io::Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(body);
    let actual = hex_encode(&hasher.finalize());
    // The manifest stores the hash as a 64-char lowercase hex string,
    // possibly with whitespace from the upstream `.sha256` file.
    let expected = expected.split_whitespace().next().unwrap_or(expected);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "sha256 mismatch for {label}: \
                 expected {expected}, got {actual}"
            ),
        ));
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

// ------------- extraction -------------

/// Stream a `.tar.gz` into `into`, flattening the rust-installer layout.
///
/// Each upstream tarball wraps everything under one top-level directory
/// (`rustc-nightly-<triple>/`) and then per-component subdirectories
/// (`rustc/`, `rust-std-<triple>/`, ...). Inside each component dir,
/// `bin/`, `lib/`, etc. are the files we actually want at the sysroot
/// root.
///
/// We strip the first two path components and skip a small set of
/// rust-installer metadata files so the extracted layout looks like a
/// plain rustup sysroot.
fn extract_tarball(gz_body: &[u8], into: &Path) -> io::Result<()> {
    let decoder = GzDecoder::new(gz_body);
    let mut archive = tar::Archive::new(decoder);
    archive.set_preserve_permissions(true);
    archive.set_overwrite(true);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path_in_archive = entry.path()?.into_owned();
        let Some(rel) = strip_installer_prefix(&path_in_archive) else {
            continue;
        };
        if is_installer_metadata(&rel) {
            continue;
        }
        let dest = into.join(&rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        entry.unpack(&dest)?;
    }
    Ok(())
}

/// Drop the leading two path components (the rust-installer wrapper dir
/// and the per-component dir) and return the rest. `None` for entries
/// shallow enough that there's nothing left to install.
fn strip_installer_prefix(path: &Path) -> Option<PathBuf> {
    let mut comps = path.components();
    comps.next()?; // rustc-nightly-<triple>
    comps.next()?; // rustc, rust-std-<triple>, ...
    let rest: PathBuf = comps.as_path().to_path_buf();
    if rest.as_os_str().is_empty() {
        None
    } else {
        Some(rest)
    }
}

/// Files the rust-installer ships at every component-dir root that
/// aren't part of the sysroot proper.
fn is_installer_metadata(rel: &Path) -> bool {
    let mut comps = rel.components();
    let Some(first) = comps.next() else {
        return false;
    };
    if comps.next().is_some() {
        return false;
    }
    matches!(
        first.as_os_str().to_string_lossy().as_ref(),
        "manifest.in" | "manifest-version" | "rust-installer-version" | "version"
    )
}
