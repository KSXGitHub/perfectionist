use super::try_apply;
use text_block_macros::text_block_fnl;

const WHAT: &str = "the diff";

/// Three lines, so a hunk can sit at any of them.
const ORIGINAL: &str = text_block_fnl! {
    "one"
    "two"
    "three"
};

/// A fixture that offers no machine-applicable suggestion stores an
/// empty diff, which is the whole of that claim.
#[test]
fn an_empty_diff_leaves_the_fixture_alone() {
    assert_eq!(try_apply(ORIGINAL, "", WHAT), Ok(ORIGINAL.to_string()));
}

/// The shape `git diff --no-index` writes, header lines included: a
/// removal and an addition under a hunk header, with context either
/// side.
#[test]
fn a_hunk_replaces_the_lines_it_names() {
    let diff = text_block_fnl! {
        "--- a/fixture.rs"
        "+++ b/fixture.rs"
        "@@ -1,3 +1,3 @@"
        " one"
        "-two"
        "+TWO"
        " three"
    };
    let expected = text_block_fnl! {
        "one"
        "TWO"
        "three"
    };
    assert_eq!(try_apply(ORIGINAL, diff, WHAT), Ok(expected.to_string()));
}

/// A hunk naming one line spells its count as a bare start, which the
/// header parse has to accept alongside the `<start>,<count>` form.
#[test]
fn a_single_line_hunk_carries_no_count() {
    let diff = text_block_fnl! {
        "--- a/fixture.rs"
        "+++ b/fixture.rs"
        "@@ -2 +2 @@"
        "-two"
        "+TWO"
    };
    let expected = text_block_fnl! {
        "one"
        "TWO"
        "three"
    };
    assert_eq!(try_apply(ORIGINAL, diff, WHAT), Ok(expected.to_string()));
}

/// Distant rewrites arrive as a hunk each, and the lines between them
/// are copied across rather than dropped.
#[test]
fn two_hunks_apply_in_order() {
    let original = text_block_fnl! {
        "a"
        "b"
        "c"
        "d"
        "e"
        "f"
        "g"
    };
    let diff = text_block_fnl! {
        "--- a/fixture.rs"
        "+++ b/fixture.rs"
        "@@ -1 +1 @@"
        "-a"
        "+A"
        "@@ -7 +7 @@"
        "-g"
        "+G"
    };
    let expected = text_block_fnl! {
        "A"
        "b"
        "c"
        "d"
        "e"
        "f"
        "G"
    };
    assert_eq!(try_apply(original, diff, WHAT), Ok(expected.to_string()));
}

/// An addition with no removal beside it, which is what a suggestion
/// that only inserts produces.
#[test]
fn an_addition_needs_no_removal() {
    let diff = text_block_fnl! {
        "--- a/fixture.rs"
        "+++ b/fixture.rs"
        "@@ -1,1 +1,2 @@"
        " one"
        "+inserted"
    };
    let expected = text_block_fnl! {
        "one"
        "inserted"
        "two"
        "three"
    };
    assert_eq!(try_apply(ORIGINAL, diff, WHAT), Ok(expected.to_string()));
}

/// A blank line in the fixture reaches the diff as a bare empty line,
/// the trailing space of its context marker having come off, so an
/// empty line counts as unchanged rather than as a line to skip.
#[test]
fn a_blank_context_line_is_copied() {
    let original = text_block_fnl! {
        "one"
        ""
        "three"
    };
    let diff = text_block_fnl! {
        "--- a/fixture.rs"
        "+++ b/fixture.rs"
        "@@ -1,3 +1,3 @@"
        " one"
        ""
        "-three"
        "+THREE"
    };
    let expected = text_block_fnl! {
        "one"
        ""
        "THREE"
    };
    assert_eq!(try_apply(original, diff, WHAT), Ok(expected.to_string()));
}

/// The fixture moved under its diff, which is worth saying rather
/// than patching whatever sits at those line numbers now.
#[test]
fn a_context_line_that_does_not_match_fails() {
    let diff = text_block_fnl! {
        "--- a/fixture.rs"
        "+++ b/fixture.rs"
        "@@ -1,3 +1,3 @@"
        " one"
        "-TWO"
        "+two"
        " three"
    };
    let Err(reason) = try_apply(ORIGINAL, diff, WHAT) else {
        panic!("this diff does not fit the fixture");
    };
    assert!(
        reason.contains("line 2 of the fixture reads differently"),
        "{reason}",
    );
}

/// A diff recorded against a longer fixture than the one it is
/// applied to.
#[test]
fn a_diff_reaching_past_the_fixture_fails() {
    let diff = text_block_fnl! {
        "--- a/fixture.rs"
        "+++ b/fixture.rs"
        "@@ -4 +4 @@"
        "-four"
        "+FOUR"
    };
    let Err(reason) = try_apply(ORIGINAL, diff, WHAT) else {
        panic!("this diff does not fit the fixture");
    };
    assert!(
        reason.contains("runs past the end of the fixture"),
        "{reason}",
    );
}

/// Hunks out of order would have the second one re-read lines the
/// first consumed, so the arithmetic is held to ascending starts.
#[test]
fn hunks_out_of_order_fail() {
    let diff = text_block_fnl! {
        "--- a/fixture.rs"
        "+++ b/fixture.rs"
        "@@ -3 +3 @@"
        "-three"
        "+THREE"
        "@@ -1 +1 @@"
        "-one"
        "+ONE"
    };
    let Err(reason) = try_apply(ORIGINAL, diff, WHAT) else {
        panic!("this diff does not fit the fixture");
    };
    assert!(
        reason.contains("an earlier hunk already covered"),
        "{reason}",
    );
}

/// `git diff` opens with a `diff --git` line, which carries no hunk
/// and no body marker, so a diff recorded with the preamble left on
/// applies the same as one without it.
#[test]
fn the_git_preamble_is_ignored() {
    let diff = text_block_fnl! {
        "diff --git a/fixture.rs b/fixture.rs"
        "index 0000000..1111111 100644"
        "--- a/fixture.rs"
        "+++ b/fixture.rs"
        "@@ -2 +2 @@"
        "-two"
        "+TWO"
    };
    let expected = text_block_fnl! {
        "one"
        "TWO"
        "three"
    };
    assert_eq!(try_apply(ORIGINAL, diff, WHAT), Ok(expected.to_string()));
}
