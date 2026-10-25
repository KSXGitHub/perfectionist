//! Hold the collector to cargo's JSON stream: what it picks out of
//! it, and the lines it is right to ignore.

use super::Reported;
use serde_json::json;
use std::path::Path;
use text_block_macros::text_block_fnl;

/// Collect `stdout` against a project at `/tmp/project`.
fn reported(stdout: &str) -> Reported {
    let mut collected = Reported::default();
    collected.extend(stdout, Path::new("/tmp/project"));
    collected
}

/// One `--message-format=json` line for a lint named `code` rendered
/// as `rendered`.
fn message(code: &str, rendered: &str) -> String {
    json!({
        "reason": "compiler-message",
        "message": {
            "code": { "code": code, "explanation": null },
            "level": "warning",
            "message": "something",
            "rendered": rendered,
        },
    })
    .to_string()
}

/// The lint names come back deduplicated and in ascending order, so
/// the settling check does not depend on the order cargo happened to
/// compile the members in.
#[test]
fn the_lint_names_come_back_sorted_and_deduplicated() {
    let stdout = [
        message("perfectionist::bare_url", "one"),
        message("clippy::needless_borrow", "two"),
        message("perfectionist::bare_url", "three"),
    ]
    .join("\n");
    assert_eq!(
        reported(&stdout).lints.into_iter().collect::<Vec<_>>(),
        ["clippy::needless_borrow", "perfectionist::bare_url"],
    );
}

/// Cargo's other `reason`s share the stream with the diagnostics, and
/// a build script writes lines that are not JSON at all. Neither is a
/// lint.
#[test]
fn only_compiler_messages_are_collected() {
    let stdout = text_block_fnl! {
        r#"{"reason":"compiler-artifact","target":{"name":"fixture"}}"#
        r#"{"reason":"build-finished","success":true}"#
        "not json at all"
    };
    assert!(reported(stdout).lints.is_empty());
    assert_eq!(reported(stdout).render(), "");
}

/// A diagnostic with no code is a summary ("2 warnings emitted") or a
/// note, and counting one would make the settling check turn on how
/// many findings the round happened to leave.
#[test]
fn a_diagnostic_without_a_code_is_not_a_lint() {
    let stdout = json!({
        "reason": "compiler-message",
        "message": {
            "code": null,
            "level": "warning",
            "message": "2 warnings emitted",
            "rendered": "warning: 2 warnings emitted",
        },
    })
    .to_string();
    assert!(reported(&stdout).lints.is_empty());
}

/// The rendered diagnostics come out in ascending order, whatever
/// order cargo reported them in.
#[test]
fn the_rendered_diagnostics_come_out_sorted() {
    let stdout = [
        message("perfectionist::bare_url", "warning: second"),
        message("perfectionist::bare_url", "warning: first"),
    ]
    .join("\n");
    assert_eq!(
        reported(&stdout).render(),
        "warning: first\nwarning: second\n",
    );
}

/// A check covers every target, so a finding in a file the lib and
/// the lib-test both compile arrives once per target. Storing it
/// twice would record which targets saw it rather than which shapes
/// the rules found.
#[test]
fn a_finding_reported_once_per_target_is_stored_once() {
    let stdout = [
        message("perfectionist::bare_url", "warning: same"),
        message("perfectionist::bare_url", "warning: same"),
    ]
    .join("\n");
    assert_eq!(reported(&stdout).render(), "warning: same\n");
}

/// A span header's `line:column` is anonymised, so inserting a line
/// above a diagnostic does not renumber every header below it, and
/// the gutter beside the quoted source goes the same way while the
/// source itself stays.
#[test]
fn line_and_column_numbers_are_anonymised() {
    let rendered = text_block_fnl! {
        "warning: bare URL"
        "  --> crates/bare-url/src/lib.rs:4:30"
        "   |"
        r#"4  | /// See https://example.com for more."#
        "   |         ^^^^^^^^^^^^^^^^^^^"
    };
    let stdout = message("perfectionist::bare_url", rendered);
    assert_eq!(
        reported(&stdout).render(),
        text_block_fnl! {
            "warning: bare URL"
            "  --> crates/bare-url/src/lib.rs:LL:CC"
            "   |"
            r#"LL | /// See https://example.com for more."#
            "   |         ^^^^^^^^^^^^^^^^^^^"
        },
    );
}

/// The project lives in a fresh temp directory on every run, so its
/// name cannot reach a stored expectation.
#[test]
fn the_project_directory_is_anonymised() {
    let stdout = message(
        "perfectionist::bare_url",
        "warning: at /tmp/project/src/lib.rs",
    );
    assert_eq!(
        reported(&stdout).render(),
        "warning: at $PROJECT/src/lib.rs\n",
    );
}

/// Whether cargo thought it was writing to a terminal is not
/// something a stored expectation should turn on.
#[test]
fn colour_escapes_are_stripped() {
    let stdout = message(
        "perfectionist::bare_url",
        "\u{1b}[1m\u{1b}[33mwarning\u{1b}[0m\u{1b}[1m: bare URL\u{1b}[0m",
    );
    assert_eq!(reported(&stdout).render(), "warning: bare URL\n");
}

/// A suggestion that inserts a blank line renders as a gutter row
/// with nothing after it, which `.editorconfig` has every conforming
/// editor strip on save. The row itself stays, every suggestion
/// block closing with one.
#[test]
fn trailing_spaces_come_off() {
    let stdout = message("perfectionist::bare_url", "warning: x   \n   |   ");
    assert_eq!(reported(&stdout).render(), "warning: x\n   |\n");
}
