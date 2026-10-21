// `scan_regular_comments = false` leaves a regular comment alone and
// keeps scanning the doc comments beside it.

// Not flagged: a regular comment is skipped.
// Wrote the report — 3 warnings remain.

// Bad: a doc comment is still scanned.
/// Walks the tree — and returns the total size.
pub fn documented() {}

fn main() {}
