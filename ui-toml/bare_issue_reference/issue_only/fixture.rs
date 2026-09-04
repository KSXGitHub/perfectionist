// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
/// Bad: closes #42 — with only `suggest_issue_url` enabled, a single
/// inline suggestion is offered:
/// `[#42](https://github.com/owner/repo/issues/42)`. It is still
/// `MaybeIncorrect`, because a bare `#42` need not name a reference.
fn _doc_issue_only() {}

fn main() {}
