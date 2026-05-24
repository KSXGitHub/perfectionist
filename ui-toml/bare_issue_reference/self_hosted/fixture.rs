/// Bad: closes #88 — on a self-hosted GitLab (host not recognised),
/// `forge = "gitlab"` is given explicitly, so the suggestions use
/// `/-/issues/88` and `/-/merge_requests/88` on the custom host.
fn _doc_self_hosted() {}

fn main() {}
