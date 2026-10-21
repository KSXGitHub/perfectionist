// `scan_doc_comments = false` leaves a doc comment alone and keeps
// scanning the regular comments beside it.

// Bad: a regular comment is still scanned.
// Wrote the report — 3 warnings remain.

// Not flagged: a doc comment is skipped.
/// Walks the tree — and returns the total size.
pub fn documented() {}

fn main() {}
