use super::{
    CACHE_DIR_ENV, FontEntry, cache_dir, ensure_cached, font_entries, hash_matches, hex_sha256,
    install_into,
};
use std::path::{Path, PathBuf};

/// A scratch directory under the system temp dir, removed on drop, so the
/// filesystem-touching tests don't litter or collide.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "perfectionist-fonts-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id(),
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn committed_lockfile_parses_to_expected_fonts() {
    let entries = font_entries();
    // The single variable Cantarell file, by the exact name base.css
    // references in `url(...)`.
    let names: Vec<&str> = entries
        .iter()
        .map(|entry| entry.filename.as_str())
        .collect();
    assert_eq!(names, ["cantarell.otf"]);
    for entry in &entries {
        assert_eq!(
            entry.sha256.len(),
            64,
            "each pinned SHA-256 must be 64 hex chars: {entry:?}",
        );
        assert!(
            entry.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "the pinned hash must be hex: {}",
            entry.sha256,
        );
        assert!(
            entry.url.starts_with("https://"),
            "each font URL must be https: {}",
            entry.url,
        );
    }
}

#[test]
fn hex_sha256_is_lowercase_and_correct() {
    // SHA-256 of the empty input — a stable, well-known vector.
    assert_eq!(
        hex_sha256(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
}

#[test]
fn hash_matches_ignores_case() {
    let upper = "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855";
    assert!(hash_matches(b"", upper));
    assert!(!hash_matches(b"not empty", upper));
}

#[test]
fn cache_dir_defaults_under_root_but_honours_env() {
    // SAFETY: single-threaded mutation of one process-wide var, restored
    // before returning so sibling tests are unaffected.
    let previous = std::env::var_os(CACHE_DIR_ENV);
    unsafe { std::env::remove_var(CACHE_DIR_ENV) };
    assert_eq!(
        cache_dir(Path::new("/repo")),
        Path::new("/repo/.cache/fonts"),
    );
    unsafe { std::env::set_var(CACHE_DIR_ENV, "/elsewhere/fonts") };
    assert_eq!(cache_dir(Path::new("/repo")), Path::new("/elsewhere/fonts"));
    match previous {
        Some(value) => unsafe { std::env::set_var(CACHE_DIR_ENV, value) },
        None => unsafe { std::env::remove_var(CACHE_DIR_ENV) },
    }
}

#[test]
fn ensure_cached_reuses_a_valid_file_without_downloading() {
    // A cache already holding a byte-for-byte-correct file must be left
    // untouched and trigger no download — the offline path. We prove "no
    // download" by pointing the entry's URL at an unroutable address: if
    // ensure_cached tried to fetch, it would fail instead of returning Ok.
    let cache = TempDir::new("reuse");
    let payload = b"pretend this is a font";
    let entry = FontEntry {
        sha256: hex_sha256(payload),
        filename: "demo.woff2".to_owned(),
        url: "https://127.0.0.1:1/never-fetched.woff2".to_owned(),
    };
    std::fs::write(cache.path().join(&entry.filename), payload).unwrap();
    ensure_cached(cache.path(), std::slice::from_ref(&entry))
        .expect("a hash-valid cached file should be reused offline");
    // Untouched: same bytes still there.
    assert_eq!(
        std::fs::read(cache.path().join(&entry.filename)).unwrap(),
        payload,
    );
}

#[test]
fn install_into_hard_links_when_possible() {
    // Cache and output share a filesystem (same temp root), so the
    // install must hard-link rather than copy: the two paths end up as
    // one inode, no bytes duplicated.
    let root = TempDir::new("install");
    let cache = root.path().join("cache");
    let out = root.path().join("out");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let entry = FontEntry {
        sha256: hex_sha256(b"x"),
        filename: "demo.woff2".to_owned(),
        url: String::new(),
    };
    std::fs::write(cache.join(&entry.filename), b"linked bytes").unwrap();

    install_into(&out, &cache, std::slice::from_ref(&entry)).expect("install should succeed");

    let dest = out.join(&entry.filename);
    assert_eq!(std::fs::read(&dest).unwrap(), b"linked bytes");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let cache_ino = std::fs::metadata(cache.join(&entry.filename))
            .unwrap()
            .ino();
        let dest_ino = std::fs::metadata(&dest).unwrap().ino();
        assert_eq!(
            cache_ino, dest_ino,
            "install_into must hard-link within one filesystem, not copy",
        );
    }
}

#[test]
fn install_into_replaces_a_stale_destination() {
    // A leftover file from a previous run must not make the hard-link
    // fail (it errors if the destination exists) — install clears it
    // first, so the output always reflects the current cache.
    let root = TempDir::new("stale");
    let cache = root.path().join("cache");
    let out = root.path().join("out");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let entry = FontEntry {
        sha256: hex_sha256(b"fresh"),
        filename: "demo.woff2".to_owned(),
        url: String::new(),
    };
    std::fs::write(cache.join(&entry.filename), b"fresh").unwrap();
    std::fs::write(out.join(&entry.filename), b"stale").unwrap();

    install_into(&out, &cache, std::slice::from_ref(&entry)).expect("install should succeed");

    assert_eq!(std::fs::read(out.join(&entry.filename)).unwrap(), b"fresh");
}
