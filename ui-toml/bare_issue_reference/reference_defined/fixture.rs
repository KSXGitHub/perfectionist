// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
/// Bad: closes #99 — under `doc_comment_form = "reference"`, but this block
/// already carries a `[#99]: URL` definition, so the fix only
/// rewrites `#99` to `[#99]` and does *not* append a duplicate.
///
/// [#99]: https://github.com/owner/repo/issues/99
fn _doc_reference_defined() {}

fn main() {}
