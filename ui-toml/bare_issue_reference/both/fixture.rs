/// Bad: closes #77 — with both `suggest_issue_url` and
/// `suggest_pr_url` enabled, the rule offers two `MaybeIncorrect`
/// suggestions (one `/issues/77` URL, one `/pull/77` URL) since a
/// bare `#77` could name either.
fn _doc_both() {}

fn main() {}
