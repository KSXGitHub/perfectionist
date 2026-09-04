// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
/// Bad: closes #99 — under `doc_comment_form = "reference"`, the fix rewrites
/// `#99` to `[#99]` and appends the matching `[#99]: URL` definition
/// to the end of this doc block (after a blank line). It stays
/// `MaybeIncorrect`, like every suggestion this lint makes.
fn _doc_reference_form() {}

fn main() {}
