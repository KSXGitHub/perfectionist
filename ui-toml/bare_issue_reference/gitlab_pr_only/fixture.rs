// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
/// Bad: closes #77 — on GitLab a bare `#77` is always an issue, so
/// `suggest_pr_url = true` yields nothing and, with
/// `suggest_issue_url` off, no suggestion is offered at all. The help
/// names only `suggest_issue_url`: advising `suggest_pr_url` here
/// would send the author after a knob that is already on and cannot
/// help.
fn _doc_gitlab_pr_only() {}

fn main() {}
