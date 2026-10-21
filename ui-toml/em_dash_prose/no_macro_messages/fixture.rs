// `scan_macro_messages = false` leaves every message alone and keeps
// scanning the comments beside them.

// Bad: a comment is still scanned, and this em dash — fires.

fn main() {
    println!("Not flagged: a printed message is skipped, so this — stays.");
    let _: i32 = Some(1).expect("Not flagged: an expect message is skipped — here.");
}
