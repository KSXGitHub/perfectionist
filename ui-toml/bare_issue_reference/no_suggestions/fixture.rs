/// Bad: closes #88 — `repository` names a recognised forge, so a URL
/// could be derived, but `suggest_issue_url` and `suggest_pr_url` are
/// both off, so no link is offered and the diagnostic is help-only.
fn _doc_no_suggestions() {}

fn main() {}
