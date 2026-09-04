// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
/// Bad: closes #88 — `repository` is set to a self-hosted host
/// the lint doesn't recognise, and `forge` is omitted, so no URL can
/// be derived and the diagnostic is help-only.
fn _doc_unrecognised_host() {}

fn main() {}
