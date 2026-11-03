use super::unified;
use crate::ui_fixtures::patch::try_apply;
use std::path::{Path, PathBuf};
use std::{env, fs};
use text_block_macros::text_block_fnl;

const NAME: &str = "fixture.rs";

const ORIGINAL: &str = text_block_fnl! {
    "one"
    "two"
    "three"
};

/// Every `.diff` under `dir`, at any depth.
fn diffs(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).expect("read a fixture dir") {
        let path = entry.expect("read a fixture entry").path();
        if path.is_dir() {
            found.extend(diffs(&path));
        } else if path
            .extension()
            .is_some_and(|extension| extension == "diff")
        {
            found.push(path);
        }
    }
    found
}

/// A fixture offering nothing machine-applicable stores an empty
/// diff, so writing one has to reach that text rather than a pair of
/// header lines with no hunk under them.
#[test]
fn agreeing_texts_have_an_empty_diff() {
    assert_eq!(unified(NAME, ORIGINAL, ORIGINAL), "");
}

#[test]
fn a_replaced_line_lands_in_a_hunk_under_the_two_header_lines() {
    let fixed = text_block_fnl! {
        "one"
        "TWO"
        "three"
    };
    let expected = text_block_fnl! {
        "--- a/fixture.rs"
        "+++ b/fixture.rs"
        "@@ -1,3 +1,3 @@"
        " one"
        "-two"
        "+TWO"
        " three"
    };
    assert_eq!(unified(NAME, ORIGINAL, fixed), expected);
}

/// What the writer produces is what the applier reads, which is the
/// claim that lets a recorded `.diff` be committed unexamined.
#[test]
fn what_is_written_applies_back() {
    let fixed = text_block_fnl! {
        "one"
        "TWO"
        "three"
        "four"
    };
    let diff = unified(NAME, ORIGINAL, fixed);
    assert_eq!(try_apply(ORIGINAL, &diff, NAME), Ok(fixed.to_string()));
}

/// The committed `.diff` fixtures were written by hand, before
/// anything recorded one, so regenerating each from its own fixture
/// holds the writer to git's format on real input: the function
/// context in a hunk header, the merging of hunks a short gap apart,
/// and an unchanged empty line written without its trailing space.
///
/// This reads the fixtures of the crate `_utils` supports, which is
/// the only place those diffs exist.
#[test]
fn every_committed_diff_regenerates() {
    // A blessing run is what makes the committed set consistent, so
    // it cannot be held to being consistent already.
    if *crate::bless::ENABLED {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("`utils` sits in the checkout");
    let mut checked = 0;
    let mut wrong = Vec::new();
    for dir in ["ui", "ui-toml"] {
        for diff in diffs(&root.join(dir)) {
            let source = diff.with_extension("rs");
            let name = source
                .file_name()
                .expect("a fixture has a file name")
                .to_str()
                .expect("a fixture's name is UTF-8");
            let original = fs::read_to_string(&source).expect("read a fixture");
            let stored = fs::read_to_string(&diff).expect("read a diff");
            let fixed = try_apply(&original, &stored, name).expect("a committed diff fits");
            let written = unified(name, &original, &fixed);
            checked += 1;
            if written != stored {
                wrong.push(format!(
                    "=== {} ===\n--- stored ---\n{stored}--- written ---\n{written}",
                    diff.display(),
                ));
            }
        }
    }
    assert!(checked > 0, "found no committed diffs to regenerate");
    assert!(
        wrong.is_empty(),
        "{} of {checked} diffs are written differently:\n{}",
        wrong.len(),
        wrong.join("\n"),
    );
}
