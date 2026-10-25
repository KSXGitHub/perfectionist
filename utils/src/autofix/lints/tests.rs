//! Hold the collector to cargo's JSON stream: the names it picks out,
//! and the lines it is right to ignore.

use super::{render, reported};
use serde_json::json;
use text_block_macros::text_block_fnl;

/// One `--message-format=json` line carrying a lint named `code`.
fn message(code: &str) -> String {
    json!({
        "reason": "compiler-message",
        "message": {
            "code": { "code": code, "explanation": null },
            "level": "warning",
            "message": "something",
        },
    })
    .to_string()
}

/// The names come back deduplicated and in ascending order, so the
/// stored expectation does not move with the order cargo happened to
/// compile the members in.
#[test]
fn the_names_come_back_sorted_and_deduplicated() {
    let stdout = [
        message("perfectionist::bare_url"),
        message("clippy::needless_borrow"),
        message("perfectionist::bare_url"),
    ]
    .join("\n");
    let names = reported(&stdout);
    assert_eq!(
        names.into_iter().collect::<Vec<_>>(),
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
    assert!(reported(stdout).is_empty());
}

/// A diagnostic with no code is a summary ("2 warnings emitted") or a
/// note, and naming one would make the expectation move with how many
/// findings the round happened to leave.
#[test]
fn a_diagnostic_without_a_code_is_not_a_lint() {
    let stdout = json!({
        "reason": "compiler-message",
        "message": {
            "code": null,
            "level": "warning",
            "message": "2 warnings emitted",
        },
    })
    .to_string();
    assert!(reported(&stdout).is_empty());
}

/// A settled codebase stores the empty string rather than a
/// placeholder, so the file is empty exactly when nothing is
/// reported.
#[test]
fn nothing_reported_renders_as_the_empty_string() {
    assert_eq!(render(&reported("")), "");
}

/// Each name is on a line of its own, newline-terminated, which is
/// what makes the stored file readable as a list.
#[test]
fn each_name_gets_a_line_of_its_own() {
    let stdout = [
        message("clippy::needless_borrow"),
        message("perfectionist::bare_url"),
    ]
    .join("\n");
    assert_eq!(
        render(&reported(&stdout)),
        "clippy::needless_borrow\nperfectionist::bare_url\n",
    );
}
