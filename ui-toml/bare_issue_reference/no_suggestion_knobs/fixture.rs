/// Bad: closes #77 — `repository` resolves to a GitHub URL, so the
/// lint could build both links, but `suggest_issue_url` and
/// `suggest_pr_url` are both off, so it offers none and points at the
/// two knobs instead.
fn _doc_no_suggestion_knobs() {}

fn main() {}
