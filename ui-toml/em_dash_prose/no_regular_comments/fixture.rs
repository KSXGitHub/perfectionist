// `scan_regular_comments = false` leaves a regular comment alone and
// keeps scanning the doc comments beside it.

// Not flagged: a regular comment is skipped, so this em dash — stays.

/// Bad: a doc comment is still scanned, and this em dash — fires.
pub fn documented() {}

fn main() {}
