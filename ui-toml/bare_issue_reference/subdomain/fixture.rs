/// Bad: closes #88 — `repo_base_url` is a self-hosted GitLab at
/// `gitlab.example.com`; the `gitlab.` subdomain is recognised, so no
/// explicit `forge` is needed and the suggestions use `/-/issues/88`
/// and `/-/merge_requests/88`.
fn _doc_subdomain() {}

fn main() {}
