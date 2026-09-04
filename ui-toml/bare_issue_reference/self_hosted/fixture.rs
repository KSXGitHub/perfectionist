// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
/// Bad: closes #88 — on a self-hosted GitLab whose host names no
/// forge (`git.example.com`), `forge = "gitlab"` is given explicitly.
/// A bare `#88` is always an issue on GitLab (merge requests are
/// `!88`), so only the `/-/issues/88` suggestion is offered.
fn _doc_self_hosted() {}

fn main() {}
