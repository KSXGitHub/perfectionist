use super::*;
use crate::model::{ConfigDoc, DefaultState};
use std::path::PathBuf;

fn fake_rule(name: &str) -> Rule {
    Rule {
        namespaced: format!("perfectionist::{name}"),
        default_state: DefaultState::Active,
        short_desc: format!("{name} short desc"),
        doc_markdown: "Body.".to_owned(),
        relative_source: PathBuf::from(format!("src/rules/{name}.rs")),
        config: ConfigDoc {
            key: format!("perfectionist::{name}"),
            fields: Vec::new(),
            custom_types: Vec::new(),
        },
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
fn stray_md_with_collision_casing_is_deleted() {
    // The directory is fully managed: a hand-added `readme.md`
    // (different case from the managed `README.md`) is treated
    // as a stray and deleted, same as any other unmanaged
    // `.md` file. On case-sensitive filesystems the two
    // coexist as distinct inodes; on case-insensitive ones
    // they're already the same file, and write-md's
    // subsequent write step rebuilds the canonical-cased
    // README from scratch either way.
    let dir = tempdir("case-collision");
    let rules = vec![fake_rule("alpha")];
    write_rules_dir(&rules, &dir, "../");
    fs::write(dir.join("readme.md"), "user content").unwrap();
    let summary = write_rules_dir(&rules, &dir, "../");
    assert_eq!(summary.orphans_removed, 1);
    // On a case-sensitive filesystem (Linux/CI) the stray is
    // gone outright. On a case-insensitive filesystem the
    // remaining file is the freshly-written `README.md`
    // (canonical case), not the user's `readme.md`. Both
    // shapes leave the user's content discarded — the point
    // of the "fully managed" contract.
    assert!(
        !dir.join("readme.md").exists()
            || fs::read_to_string(dir.join("readme.md")).unwrap() != "user content",
    );
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
