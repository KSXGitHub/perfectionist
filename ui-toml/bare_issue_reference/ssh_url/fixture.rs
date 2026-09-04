// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
/// Bad: closes #321 — `repository` is given as an scp-like SSH clone
/// URL (`git@github.com:owner/repo.git`). It's parsed into the
/// canonical `https://github.com/owner/repo` web base, so the GitHub
/// forge is detected and the issue / PR links resolve as usual.
fn _doc_ssh_url() {}

fn main() {}
