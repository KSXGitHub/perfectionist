/// Bad: closes #55 — with a `gitlab.com` `repo_base_url`, the forge
/// is inferred as GitLab. A bare `#55` is always an issue on GitLab
/// (merge requests are `!55`), so only the `/-/issues/55` suggestion
/// is offered — still a `MaybeIncorrect` hint, since `#55` need not
/// be a reference at all.
fn _doc_gitlab() {}

fn main() {}
