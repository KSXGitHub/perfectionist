/// Bad: closes #88 — `repository` is a self-hosted GitLab at
/// `gitlab.example.com`; the `gitlab.` subdomain is recognised, so no
/// explicit `forge` is needed. A bare `#88` is always an issue on
/// GitLab (merge requests are `!88`), so only the `/-/issues/88`
/// suggestion is offered.
fn _doc_subdomain() {}

fn main() {}
