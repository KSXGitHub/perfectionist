//! Compare the in-memory markdown rendering against the on-disk
//! `rules/` directory (`check-md`) and write that directory while
//! cleaning up orphaned files (`write-md`). The two operations
//! share the same expected-output computation; only what they do
//! with a mismatch differs.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::model::Rule;
use crate::render_md::{INDEX_FILE_NAME, render_index_md, render_rule_md, rule_file_name};

/// The intended on-disk state of a rules directory. Built once from
/// the extracted rules and then either compared (`check-md`) or
/// written (`write-md`).
struct ExpectedTree {
    /// Maps `<file_name>` (no directory component) to the
    /// fully-rendered markdown body. Includes both the per-rule
    /// files and the `README.md` index. Keyed by `BTreeMap` so the
    /// iteration order matches the on-disk directory listing order,
    /// which keeps the check-md "missing files" report stable.
    files: BTreeMap<String, String>,
}

impl ExpectedTree {
    fn from_rules(rules: &[Rule], source_link_prefix: &str) -> Self {
        let mut files = BTreeMap::new();
        for rule in rules {
            let prev = files.insert(
                rule_file_name(rule),
                render_rule_md(rule, source_link_prefix),
            );
            if prev.is_some() {
                // Two rules cannot share a filename; the extractor
                // already guarantees rule namespaces are unique, so
                // colliding filenames would be a bug in
                // `rule_file_name`.
                panic!(
                    "internal error: two rules rendered to the same filename `{}`",
                    rule_file_name(rule),
                );
            }
        }
        files.insert(INDEX_FILE_NAME.to_owned(), render_index_md(rules));
        ExpectedTree { files }
    }

    /// Case-sensitive exact match. This is the right contract for
    /// every code path that reads `expected.files` to decide what
    /// to render or compare — those paths must use the canonical
    /// casing the renderer wrote. For the orphan-delete code path,
    /// see `is_managed_case_insensitive`.
    fn is_managed(&self, file_name: &str) -> bool {
        self.files.contains_key(file_name)
    }

    /// Case-insensitive match. Used only to guard the orphan-
    /// delete step on case-insensitive filesystems (macOS APFS,
    /// Windows NTFS): there, `readme.md` and `README.md` are the
    /// same inode, and a case-sensitive orphan check would happily
    /// delete the on-disk file the renderer is about to write,
    /// silently dropping its contents. See the caller for the full
    /// rationale.
    fn is_managed_case_insensitive(&self, file_name: &str) -> bool {
        self.files
            .keys()
            .any(|key| key.eq_ignore_ascii_case(file_name))
    }
}

/// Result of comparing the on-disk directory against the expected
/// state. The success branch is empty by design — the CLI prints
/// its own success message — so callers don't have to thread a
/// rule count through the type.
pub(crate) enum CheckOutcome {
    Clean,
    Drifted(String),
}

/// Outcome summary for the `write-md` subcommand. The CLI prints
/// these counts so a reader of the build log can tell at a glance
/// whether anything actually changed. Every field counts *changes
/// to disk*, not files emitted — a rule whose existing on-disk
/// body already matches the rendered output is not counted in
/// `rules_changed`, and the index is `index_changed: false` even
/// though the renderer produced its bytes.
pub(crate) struct WriteSummary {
    pub(crate) rules_changed: usize,
    pub(crate) index_changed: bool,
    pub(crate) orphans_removed: usize,
}

pub(crate) fn check_rules_dir(
    rules: &[Rule],
    rules_dir: &Path,
    source_link_prefix: &str,
) -> CheckOutcome {
    let expected = ExpectedTree::from_rules(rules, source_link_prefix);
    let actual_names = read_managed_filenames(rules_dir);

    let mut report = String::new();

    // Surface missing and mismatched files first — those are the
    // failures most often caused by a rule author forgetting to run
    // `write-md`. Orphans come last because they're informational:
    // the regeneration step will delete them.
    for (file_name, expected_body) in &expected.files {
        let path = rules_dir.join(file_name);
        match fs::read_to_string(&path) {
            Ok(actual_body) if &actual_body == expected_body => {}
            Ok(_) => {
                report.push_str(&format!("- `{}`: content differs\n", file_name));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                report.push_str(&format!("- `{}`: missing\n", file_name));
            }
            Err(error) => {
                report.push_str(&format!("- `{}`: read error: {error}\n", file_name));
            }
        }
    }

    for file_name in &actual_names {
        if !expected.is_managed(file_name) {
            report.push_str(&format!("- `{}`: orphan (no matching rule)\n", file_name));
        }
    }

    if report.is_empty() {
        CheckOutcome::Clean
    } else {
        let mut full = String::from("Markdown catalogue is out of date:\n");
        full.push_str(&report);
        CheckOutcome::Drifted(full)
    }
}

pub(crate) fn write_rules_dir(
    rules: &[Rule],
    rules_dir: &Path,
    source_link_prefix: &str,
) -> WriteSummary {
    let expected = ExpectedTree::from_rules(rules, source_link_prefix);
    fs::create_dir_all(rules_dir).expect("failed to create rules directory");

    let mut summary = WriteSummary {
        rules_changed: 0,
        index_changed: false,
        orphans_removed: 0,
    };

    // Delete orphans *before* writing the new tree. The order
    // matters only if a rename swaps a stale file out for a new one
    // with the same name (writing first would clobber the file the
    // deletion step then expected to find); the rule-name space is
    // global so that can't happen today, but deleting first costs
    // nothing and stays correct under future renames.
    let actual_names = read_managed_filenames(rules_dir);
    for file_name in &actual_names {
        if expected.is_managed(file_name) {
            continue;
        }
        // Case-insensitive collision guard. On case-insensitive
        // filesystems (macOS APFS default, Windows NTFS), a
        // user-typed `readme.md` and the catalogue's `README.md`
        // resolve to the same inode. The case-sensitive
        // `is_managed` check above returns false for the
        // on-disk name; if we then `remove_file` it, we delete
        // the file the renderer is about to (re)write, silently
        // discarding any user content it held. Skipping the
        // delete when the name case-insensitively matches a
        // managed entry is safe on case-sensitive filesystems
        // too — our generator only ever writes the canonical
        // casing, so a Linux-side `readme.md` couldn't have been
        // produced by us and skipping its deletion just leaves
        // an unrelated user file in place.
        if expected.is_managed_case_insensitive(file_name) {
            continue;
        }
        let path = rules_dir.join(file_name);
        fs::remove_file(&path)
            .unwrap_or_else(|error| panic!("failed to remove {}: {error}", path.display()));
        summary.orphans_removed += 1;
    }

    for (file_name, body) in &expected.files {
        let path = rules_dir.join(file_name);
        // Only write when the content differs, so unchanged files
        // keep their original mtime and don't churn the working tree
        // on every `write-md` run.
        let needs_write = match fs::read_to_string(&path) {
            Ok(existing) => &existing != body,
            Err(_) => true,
        };
        if needs_write {
            fs::write(&path, body)
                .unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
        }
        if file_name == INDEX_FILE_NAME {
            // OR-assign rather than overwrite so the flag stays
            // sticky if a future refactor (retry-on-IO, multiple
            // managed indexes) visits the index more than once.
            // `BTreeMap` keys are unique today, but the defensive
            // form costs nothing.
            summary.index_changed |= needs_write;
        } else if needs_write {
            summary.rules_changed += 1;
        }
    }

    summary
}

/// List the on-disk `.md` files the doc generator's orphan check
/// considers. Restricted to `.md` files at the top level so a
/// `rules/.gitkeep` or a nested subdirectory isn't pulled in.
///
/// **The `rules/` directory is fully managed by this generator.**
/// Any `.md` file at the top level that doesn't correspond to an
/// implemented rule is treated as an orphan by `check-md` and
/// deleted by `write-md`; users cannot drop extra hand-written
/// `.md` files into the directory expecting them to survive.
/// Non-`.md` files (`.gitkeep`, `notes.txt`) and subdirectories are
/// out of scope for the orphan check and are left untouched.
///
/// `.md` is the right boundary for the managed set: every file
/// this generator writes ends in `.md`, and any other extension is
/// by definition not something this generator produced.
fn read_managed_filenames(rules_dir: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let entries = match fs::read_dir(rules_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return names,
        Err(error) => panic!("failed to read {}: {error}", rules_dir.display()),
    };
    for entry in entries {
        let entry = entry.expect("failed to read directory entry");
        let file_type = entry.file_type().expect("failed to read file type");
        if !file_type.is_file() {
            continue;
        }
        // Skip non-UTF-8 filenames rather than coercing them via
        // `to_string_lossy`: a coerced name would still round-trip
        // through `rules_dir.join(name)` to the *wrong* on-disk
        // path, and the orphan-delete step would either fail or
        // (worse) target a different file. The rule extractor only
        // ever produces UTF-8 names, so this branch should never
        // fire in practice; a non-UTF-8 file in `rules/` is by
        // definition not something this generator wrote.
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if Path::new(&name).extension().is_some_and(|ext| ext == "md") {
            names.insert(name);
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::model::Level;

    fn fake_rule(name: &str) -> Rule {
        Rule {
            namespaced: format!("perfectionist::{name}"),
            level: Level::Warn,
            short_desc: format!("{name} short desc"),
            doc_markdown: "Body.".to_owned(),
            relative_source: PathBuf::from(format!("src/rules/{name}.rs")),
            config: None,
        }
    }

    /// Build a temporary directory unique to *this call*, so the
    /// test doesn't litter the working tree and can't collide with
    /// another test running in parallel. The path includes both the
    /// process id (for cross-process isolation under `cargo test`'s
    /// test harness, which forks per binary) and a per-process
    /// atomic counter (for inter-test isolation within the same
    /// binary, so two tests that happen to share a `label` don't
    /// share a directory).
    fn tempdir(label: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "perfectionist-gen-docs-{label}-{}-{seq}",
            std::process::id(),
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn write_then_check_is_clean() {
        let dir = tempdir("write-then-check");
        let rules = vec![fake_rule("alpha"), fake_rule("beta")];
        write_rules_dir(&rules, &dir, "../");
        assert!(matches!(
            check_rules_dir(&rules, &dir, "../"),
            CheckOutcome::Clean
        ));
    }

    #[test]
    fn missing_file_drifts() {
        let dir = tempdir("missing");
        let rules = vec![fake_rule("alpha"), fake_rule("beta")];
        write_rules_dir(&rules, &dir, "../");
        fs::remove_file(dir.join("alpha.md")).unwrap();
        match check_rules_dir(&rules, &dir, "../") {
            CheckOutcome::Drifted(report) => assert!(report.contains("`alpha.md`: missing")),
            CheckOutcome::Clean => panic!("expected drift"),
        }
    }

    #[test]
    fn stale_file_drifts() {
        let dir = tempdir("stale");
        let rules = vec![fake_rule("alpha")];
        write_rules_dir(&rules, &dir, "../");
        fs::write(dir.join("alpha.md"), "not the right body").unwrap();
        match check_rules_dir(&rules, &dir, "../") {
            CheckOutcome::Drifted(report) => {
                assert!(report.contains("`alpha.md`: content differs"))
            }
            CheckOutcome::Clean => panic!("expected drift"),
        }
    }

    #[test]
    fn orphan_file_drifts_and_write_removes_it() {
        let dir = tempdir("orphan");
        let rules = vec![fake_rule("alpha")];
        write_rules_dir(&rules, &dir, "../");
        fs::write(dir.join("orphan.md"), "leftover").unwrap();
        match check_rules_dir(&rules, &dir, "../") {
            CheckOutcome::Drifted(report) => assert!(report.contains("`orphan.md`: orphan")),
            CheckOutcome::Clean => panic!("expected drift"),
        }
        let summary = write_rules_dir(&rules, &dir, "../");
        assert_eq!(summary.orphans_removed, 1);
        assert!(!dir.join("orphan.md").exists());
    }

    #[test]
    fn non_md_files_are_ignored() {
        let dir = tempdir("non-md");
        let rules = vec![fake_rule("alpha")];
        write_rules_dir(&rules, &dir, "../");
        fs::write(dir.join(".gitkeep"), "").unwrap();
        fs::write(dir.join("notes.txt"), "scratch").unwrap();
        assert!(matches!(
            check_rules_dir(&rules, &dir, "../"),
            CheckOutcome::Clean
        ));
        // And `write-md` doesn't delete them.
        write_rules_dir(&rules, &dir, "../");
        assert!(dir.join(".gitkeep").exists());
        assert!(dir.join("notes.txt").exists());
    }

    #[test]
    fn nonexistent_dir_drifts_with_every_file_missing() {
        // `read_managed_filenames` returns an empty set on
        // `NotFound`, so `check_rules_dir` should report each
        // expected file as missing rather than panicking or
        // silently passing.
        let base = tempdir("nonexistent");
        let dir = base.join("never-created");
        assert!(!dir.exists());
        let rules = vec![fake_rule("alpha"), fake_rule("beta")];
        match check_rules_dir(&rules, &dir, "../") {
            CheckOutcome::Drifted(report) => {
                assert!(report.contains("`alpha.md`: missing"));
                assert!(report.contains("`beta.md`: missing"));
                assert!(report.contains("`README.md`: missing"));
            }
            CheckOutcome::Clean => panic!("expected drift"),
        }
    }

    #[test]
    fn case_insensitive_collision_is_not_deleted() {
        // On a case-sensitive filesystem (Linux), this test
        // creates two distinct files: `README.md` (the managed
        // index) and `readme.md` (a hand-typed file). The orphan
        // check should leave `readme.md` alone rather than
        // deleting it, since on a case-insensitive filesystem the
        // two would be the same inode and deleting would discard
        // the managed file's contents along with it. Skipping the
        // delete on Linux too is harmless — we'd only have written
        // the canonical casing, so a lowercase orphan can't have
        // come from us.
        let dir = tempdir("case-collision");
        let rules = vec![fake_rule("alpha")];
        write_rules_dir(&rules, &dir, "../");
        fs::write(dir.join("readme.md"), "user content").unwrap();
        let summary = write_rules_dir(&rules, &dir, "../");
        assert_eq!(summary.orphans_removed, 0);
        assert!(dir.join("readme.md").exists());
    }

    #[test]
    fn second_write_is_a_no_op() {
        let dir = tempdir("idempotent");
        let rules = vec![fake_rule("alpha")];
        write_rules_dir(&rules, &dir, "../");
        let summary = write_rules_dir(&rules, &dir, "../");
        assert_eq!(summary.rules_changed, 0);
        assert!(!summary.index_changed);
        assert_eq!(summary.orphans_removed, 0);
    }
}
