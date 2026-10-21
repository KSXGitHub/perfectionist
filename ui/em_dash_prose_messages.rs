#![feature(register_tool)]
#![register_tool(perfectionist)]

use std::fmt::Write;

fn _writers(path: &str, buffer: &mut String) {
    // Bad: a writer's message is prose the program shows its user.
    println!("Loaded the manifest — 12 entries were skipped.");
    // Bad: an en dash is reported wherever an em dash is.
    eprintln!("The cache is stale – rebuilding it now.");
    // Good: a colon names the relationship a dash would have left out.
    print!("Reading the lockfile: this may take a moment.");
    // Bad: a formatted message is shown to the user as much as a
    // printed one.
    let _ = format!("Wrote the report — 3 warnings remain.");
    // Good: an interpolated path needs no dash.
    println!("Skipping {path} because it is a symlink.");
    // Bad: a path-qualified invocation resolves by its final segment.
    std::println!("Parsed the tree — 4 levels deep.");
    // Not flagged: an ASCII hyphen run is left alone inside every
    // string literal, where it is as likely a separator as a dash.
    println!("Pass -- --nocapture to forward the flag.");
    // Not flagged: a backtick-quoted run inside a message is a
    // quotation, so the dash in it is not the author's to restructure.
    println!("The log says `build failed — see above`.");
    // Bad: a message written into a buffer is prose the program shows
    // its user.
    let _ = writeln!(buffer, "Finished the sweep — 7 files changed.");
}

fn _panics() {
    // Bad: a panic message is prose the program shows its user.
    panic!("the lockfile is corrupt — delete it and retry");
}

fn _todo() {
    // Bad: `todo!` takes a panic message.
    todo!("handle the nested case — the flat one works");
}

fn _unimplemented() {
    // Bad: `unimplemented!` takes a panic message.
    unimplemented!("the remote backend — only the local one is wired");
}

fn _unreachable() {
    // Bad: `unreachable!` takes a panic message.
    unreachable!("the parser rejects this input — nothing reaches here");
}

fn _asserts(flag: bool, left: i32, right: i32) {
    // Bad: an assertion's trailing argument is its message.
    assert!(flag, "the flag must be set — the caller forgot it");
    // Bad: a comparison's message follows the two values.
    assert_eq!(left, right, "the counts diverged — check the filter");
    // Not flagged: the first two arguments are the values being
    // compared, so a dash in one of them is data rather than prose.
    assert_eq!("page 2 — the middle", "page 2 — the end");
}

fn _expects() {
    // Bad: `expect` on an `Option` takes a panic message.
    let _: i32 = Some(1).expect("the config is missing a profile — add one");
    // Bad: `expect_err` on a `Result` takes a panic message.
    let _: () = Result::<i32, ()>::Err(())
        .expect_err("the parse should have failed — the input is empty");
    // Good: the message names the cause, so it needs no dash.
    let _: i32 = Some(1).expect("the config has no default profile");
}

struct Settings;

impl Settings {
    fn expect(&self, _message: &str) {}
}

fn _unrelated_expect() {
    // Not flagged: an `expect` on any other type is not a panic
    // message.
    Settings.expect("the value is unset — using the default");
}

fn _expect_a_borrowed_string(message: String) {
    // Not flagged: the rule reads a message from the source text of a
    // string literal, so an argument that is not one holds nothing for
    // it to scan.
    let _: i32 = Some(1).expect(&message);
}

trait ExpectErr {
    fn expect_err(&self, code: i32);
}

impl<Value> ExpectErr for Option<Value> {
    fn expect_err(&self, _code: i32) {}
}

fn _expect_err_taking_a_code() {
    // Not flagged: `Option` has no inherent `expect_err`, so this
    // trait method answers the call. Its argument is a code rather
    // than a message, and a non-string literal holds no prose.
    Some(1).expect_err(42);
}

#[expect(
    perfectionist::numeric_escape_for_visible_character,
    reason = "this message spells the escape on purpose, so that the \
              dash reaching the rule is the escape rather than the \
              character"
)]
fn _escaped() {
    // Not flagged: a dash written as an escape is a deliberate
    // spelling. Both message surfaces read a literal's source text
    // rather than its value, so the escape reaches each of them as an
    // escape.
    println!("Built 4 targets \u{2014} 1 was cached.");
    let _: i32 = Some(1).expect("the profile is missing \u{2014} add one");
}

fn main() {}
