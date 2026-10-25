//! Hold the normaliser to the shapes it rebuilds and, as much, to the
//! ones it has to leave alone: a span header and a gutter are easily
//! confused with ordinary prose.

use super::diagnostic;

/// Normalise `rendered` against a project at `/tmp/project`.
fn normalised(rendered: &str) -> String {
    diagnostic(rendered, "/tmp/project")
}

/// A `-->` header whose trailing fields are not both numbers is not a
/// span header, so it keeps what it says.
#[test]
fn a_header_without_two_numbers_is_left_alone() {
    assert_eq!(
        normalised("  --> crates/x/src/lib.rs"),
        "  --> crates/x/src/lib.rs",
    );
}

/// A quoted source line that itself holds a `|` keeps every character
/// after the gutter, closure syntax being full of them.
#[test]
fn a_quoted_closure_keeps_its_pipes() {
    assert_eq!(
        normalised("12 |     items.map(|item| item.len())"),
        "LL |     items.map(|item| item.len())",
    );
}

/// The `|` rustc prints with no line number beside it is a gutter all
/// the same, and moves with the rest of the block.
#[test]
fn an_empty_gutter_moves_with_the_block() {
    assert_eq!(normalised("   |     ^^^^"), "   |     ^^^^");
}

/// A note's prose may hold a `|` without introducing a gutter.
#[test]
fn a_note_carrying_a_pipe_is_left_alone() {
    assert_eq!(
        normalised("   = note: matched `a | b` here"),
        "   = note: matched `a | b` here",
    );
}

/// A path holding a colon is not a span position, which is anchored
/// on the source extension.
#[test]
fn a_colon_outside_a_source_path_is_left_alone() {
    assert_eq!(
        normalised("  --> crates/x:1:2/src/lib.toml"),
        "  --> crates/x:1:2/src/lib.toml",
    );
}

/// rustc sizes the gutter column by the widest line number in the
/// block, so a block whose diagnostic sits on a one-digit line is
/// indented one column less than the same block a line later. Every
/// row is rebuilt at one width so that move changes nothing.
#[test]
fn a_narrow_gutter_is_widened_to_match() {
    assert_eq!(
        normalised(" --> crates/x/src/lib.rs:9:5"),
        "  --> crates/x/src/lib.rs:LL:CC",
    );
    assert_eq!(normalised("  |"), "   |");
    assert_eq!(
        normalised("9 | pub struct Helper;"),
        "LL | pub struct Helper;",
    );
    assert_eq!(normalised("  |     ^^^^"), "   |     ^^^^");
}

/// A suggestion's deletion and insertion rows carry the gutter too,
/// which `-Zui-testing` anonymises for a `.stderr` under `ui/` and
/// which nothing anonymises here.
#[test]
fn suggestion_rows_lose_their_line_numbers() {
    assert_eq!(
        normalised("9 - /// mail me@example.com"),
        "LL - /// mail me@example.com",
    );
    assert_eq!(
        normalised("14 + /// mail <me@example.com>"),
        "LL + /// mail <me@example.com>",
    );
}

/// A `-->` header has no line number before its arrow, so the arrow
/// must not read as a deletion row's marker.
#[test]
fn a_span_header_is_not_a_deletion_row() {
    assert_eq!(
        normalised("  --> crates/x/src/lib.rs"),
        "  --> crates/x/src/lib.rs",
    );
}

/// A note's prose may hold the marker characters without introducing
/// a gutter.
#[test]
fn a_note_holding_a_marker_is_left_alone() {
    assert_eq!(
        normalised("   = help: write `a + b` instead"),
        "   = help: write `a + b` instead",
    );
}

/// A note carries no gutter of its own but is indented against one,
/// so it moves with the rest of the block.
#[test]
fn a_note_is_re_indented_with_the_block() {
    assert_eq!(
        normalised("  = note: `#[warn(perfectionist::bare_email)]` on by default"),
        "   = note: `#[warn(perfectionist::bare_email)]` on by default",
    );
}

/// A diagnostic's first line is not indented against the gutter at
/// all.
#[test]
fn the_message_line_is_left_at_the_margin() {
    assert_eq!(normalised("warning: bare URL"), "warning: bare URL");
    assert_eq!(normalised("help: wrap in `<...>`"), "help: wrap in `<...>`");
}
