// `scan_doc_comments` and `scan_regular_comments` are both false, so
// the comment walk is skipped before it starts. The macro-message
// surface is untouched and still fires, which is what tells a skipped
// walk from a rule that has stopped running.

// Not flagged: a regular comment is skipped, so this em dash — stays.

/// Not flagged: a doc comment is skipped, so this em dash — stays.
pub fn documented() {}

fn main() {
    println!("Bad: a printed message with an em dash — and a clause.");
}
