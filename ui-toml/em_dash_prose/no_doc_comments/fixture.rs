// `scan_doc_comments = false` leaves a doc comment alone and keeps
// scanning the regular comments beside it.

// Bad: a regular comment is still scanned, and this em dash — fires.

/// Not flagged: a doc comment is skipped, so this em dash — stays.
pub fn documented() {}

fn main() {}
