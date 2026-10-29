use super::{Expectations, parse};
use crate::{TempDir, build_base};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

/// Create each path under `root`, empty.
fn tree(root: &Path, paths: &[&str]) {
    for path in paths {
        let path = root.join(path);
        let parent = path.parent().expect("a fixture path has a parent");
        fs::create_dir_all(parent).expect("create a fixture dir");
        fs::write(&path, "").expect("write a fixture file");
    }
}

/// Empty is how an unset variable reaches [`parse`].
#[test]
fn empty_and_false_are_off() {
    assert!(matches!(parse(OsStr::new("")), Ok(false)));
    assert!(matches!(parse(OsStr::new("false")), Ok(false)));
}

#[test]
fn true_is_on() {
    assert!(matches!(parse(OsStr::new("true")), Ok(true)));
}

/// Reading an unrecognised value as "off" would leave the run
/// comparing while its author believed it was recording.
#[test]
fn any_other_value_is_rejected() {
    let Err(error) = parse(OsStr::new("1")) else {
        panic!("1 is not a spelling of the tri-state");
    };
    assert_eq!(
        error.to_string(),
        r#"PERFECTIONIST_BLESS must be 'true', 'false', empty, or unset; got: "1""#,
    );
}

#[test]
fn a_differently_cased_true_is_rejected() {
    assert!(parse(OsStr::new("TRUE")).is_err());
}

#[test]
fn a_top_level_fixture_stages_beside_no_directory() {
    let temp = TempDir::new().expect("create a tree");
    let staged_root = temp.path().join("staged");
    tree(temp.path(), &["ui/map_or.rs", "ui/map_or.stderr"]);
    let expectations = Expectations::under(temp.path(), Path::new("ui"), &staged_root);
    assert_eq!(expectations.0.len(), 1);
    assert_eq!(
        expectations.0[0].committed,
        temp.path().join("ui/map_or.stderr"),
    );
    assert_eq!(
        expectations.0[0].staged,
        staged_root.join("map_or.stage-id.stderr"),
    );
}

#[test]
fn a_nested_tree_keeps_its_components_below_the_first() {
    let temp = TempDir::new().expect("create a tree");
    let staged_root = temp.path().join("staged");
    tree(
        temp.path(),
        &[
            "ui-toml/uncombined_self_import/fixture.rs",
            "ui-toml/uncombined_self_import/fixture.stderr",
        ],
    );
    let expectations = Expectations::under(
        temp.path(),
        Path::new("ui-toml/uncombined_self_import"),
        &staged_root,
    );
    assert_eq!(expectations.0.len(), 1);
    assert_eq!(
        expectations.0[0].staged,
        staged_root.join("uncombined_self_import/fixture.stage-id.stderr"),
    );
}

/// What lets a new fixture be recorded rather than hand-written.
#[test]
fn a_fixture_without_a_stderr_is_covered() {
    let temp = TempDir::new().expect("create a tree");
    let staged_root = temp.path().join("staged");
    tree(temp.path(), &["ui/newborn.rs"]);
    let expectations = Expectations::under(temp.path(), Path::new("ui"), &staged_root);
    let committed: Vec<_> = expectations
        .0
        .iter()
        .map(|expectation| expectation.committed.clone())
        .collect();
    assert_eq!(committed, vec![temp.path().join("ui/newborn.stderr")]);
}

#[test]
fn an_auxiliary_crate_is_left_out() {
    let temp = TempDir::new().expect("create a tree");
    let staged_root = temp.path().join("staged");
    tree(
        temp.path(),
        &["ui/fixture.rs", "ui/auxiliary/dependency.rs"],
    );
    let expectations = Expectations::under(temp.path(), Path::new("ui"), &staged_root);
    let committed: Vec<_> = expectations
        .0
        .iter()
        .map(|expectation| expectation.committed.clone())
        .collect();
    assert_eq!(committed, vec![temp.path().join("ui/fixture.stderr")]);
}

#[test]
fn of_stages_under_the_temp_directory() {
    let temp = TempDir::new().expect("create a tree");
    tree(temp.path(), &["ui/located.rs", "ui/located.stderr"]);
    let expectations = Expectations::of(temp.path(), Path::new("ui"));
    assert_eq!(
        expectations.0[0].staged,
        build_base::path().join("located.stage-id.stderr"),
    );
}

/// The common case: a fixture that matched last time staged nothing.
#[test]
fn deleting_what_was_never_staged_is_fine() {
    let temp = TempDir::new().expect("create a tree");
    let staged_root = temp.path().join("staged");
    tree(temp.path(), &["ui/absent.rs", "ui/absent.stderr"]);
    Expectations::under(temp.path(), Path::new("ui"), &staged_root).delete_staged();
}

/// A leftover from an earlier failing run must not be recorded as
/// this run's output.
#[test]
fn deleting_removes_a_stale_staged_actual() {
    let temp = TempDir::new().expect("create a tree");
    let staged_root = temp.path().join("staged");
    tree(temp.path(), &["ui/stale.rs", "ui/stale.stderr"]);
    let expectations = Expectations::under(temp.path(), Path::new("ui"), &staged_root);
    let staged = &expectations.0[0].staged;
    fs::create_dir_all(&staged_root).expect("create the staged dir");
    fs::write(staged, "from a run two days ago\n").expect("write the stale text");

    expectations.delete_staged();

    assert!(!staged.exists());
}

#[test]
fn recording_replaces_the_committed_text() {
    let temp = TempDir::new().expect("create a tree");
    let staged_root = temp.path().join("staged");
    tree(temp.path(), &["ui/recorded.rs"]);
    let committed = temp.path().join("ui/recorded.stderr");
    fs::write(&committed, "stale\n").expect("write the committed text");
    let expectations = Expectations::under(temp.path(), Path::new("ui"), &staged_root);
    fs::create_dir_all(&staged_root).expect("create the staged dir");
    fs::write(&expectations.0[0].staged, "fresh\n").expect("write the staged text");

    let harvested = expectations.record();

    assert_eq!(harvested, vec![committed.as_path()]);
    assert_eq!(
        fs::read_to_string(&committed).expect("read the committed text"),
        "fresh\n",
    );
}

#[test]
fn recording_skips_what_staged_nothing() {
    let temp = TempDir::new().expect("create a tree");
    let staged_root = temp.path().join("staged");
    tree(temp.path(), &["ui/agreed.rs"]);
    let committed = temp.path().join("ui/agreed.stderr");
    fs::write(&committed, "kept\n").expect("write the committed text");
    let expectations = Expectations::under(temp.path(), Path::new("ui"), &staged_root);

    assert!(expectations.record().is_empty());
    assert_eq!(
        fs::read_to_string(&committed).expect("read the committed text"),
        "kept\n",
    );
}

/// What stops a run whose temp directory was rewritten out of its
/// output from committing the rewrite.
#[test]
#[should_panic(expected = "an expectation must not hold `$TEST_BUILD_DIR`")]
fn recording_refuses_text_a_temp_directory_was_rewritten_out_of() {
    let temp = TempDir::new().expect("create a tree");
    let staged_root = temp.path().join("staged");
    tree(temp.path(), &["ui/rewritten.rs"]);
    let expectations = Expectations::under(temp.path(), Path::new("ui"), &staged_root);
    fs::create_dir_all(&staged_root).expect("create the staged dir");
    fs::write(
        &expectations.0[0].staged,
        "LL |     let _ = \"$TEST_BUILD_DIR\";\n",
    )
    .expect("write the staged text");

    expectations.record();
}
