// `scan_macro_messages = false` leaves every message alone and keeps
// scanning the comments beside them.

// Bad: a comment is still scanned.
// Wrote the report — 3 warnings remain.

fn main() {
    // Not flagged: a printed message is skipped.
    println!("Loaded the manifest — 12 entries were skipped.");
    // Not flagged: an expect message is skipped.
    let _: i32 = Some(1).expect("the config is missing a profile — add one");
}
