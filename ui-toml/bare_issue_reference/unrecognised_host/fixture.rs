/// Bad: closes #88 — `repo_base_url` is set to a self-hosted host
/// the lint doesn't recognise, and `forge` is omitted, so no URL can
/// be derived and the diagnostic is help-only.
fn _doc_unrecognised_host() {}

fn main() {}
