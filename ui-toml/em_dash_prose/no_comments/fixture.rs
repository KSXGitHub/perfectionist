// `scan_doc_comments` and `scan_regular_comments` are both false, so
// the comment walk is skipped before it starts. The macro-message
// surface is untouched and still fires, which is what tells a skipped
// walk from a rule that has stopped running.

// Not flagged: a regular comment is skipped.
// Wrote the report — 3 warnings remain.

// Not flagged: a doc comment is skipped.
/// Walks the tree — and returns the total size.
pub fn documented() {}

fn main() {
    // Bad: the macro-message surface is untouched.
    println!("Loaded the manifest — 12 entries were skipped.");
}
