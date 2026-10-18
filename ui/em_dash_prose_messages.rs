#![feature(register_tool)]
#![register_tool(perfectionist)]

use std::fmt::Write;

// Writers: the message of each is prose the program shows its user.
fn _writers(path: &str, buffer: &mut String) {
    println!("Bad: a printed message with an em dash — and a clause.");
    eprintln!("Bad: a printed message with an en dash – and a clause.");
    print!("Good: a printed message with a colon: the clause is named.");
    let _ = format!("Bad: a formatted message with an em dash — here.");
    println!("Good: a printed path {path} needs no dash at all.");
    // Path-qualified invocations resolve by their final segment.
    std::println!("Bad: a qualified call's message with a dash — here.");
    // The ASCII hyphen run is left alone inside every string literal,
    // where it is as likely a separator as a dash.
    println!("Good: a printed message holding -- an option separator.");
    // A backtick-quoted run inside a message is a quotation, so the
    // dash in it is not the author's to restructure.
    println!("Good: a printed message quoting `a dash — verbatim`.");
    let _ = writeln!(buffer, "Bad: a written message with a dash — here.");
}

// Panics: the message of each is prose too.
fn _panics() {
    panic!("Bad: a panic message with an em dash — and a clause.");
}

fn _todo() {
    todo!("Bad: a todo message with an em dash — and a clause.");
}

fn _unimplemented() {
    unimplemented!("Bad: an unimplemented message with a dash — here.");
}

fn _unreachable() {
    unreachable!("Bad: an unreachable message with a dash — here.");
}

// Assertions: the message is prose, the compared values are program
// data.
fn _asserts(flag: bool, left: i32, right: i32) {
    assert!(flag, "Bad: an assertion message with a dash — here.");
    assert_eq!(left, right, "Bad: a comparison message with — here.");
    // The first two arguments are the values being compared, so a dash
    // in one of them is data rather than prose.
    assert_eq!("value — one", "value — two");
}

// `expect` and `expect_err` on `Option` / `Result` take a panic
// message as their first argument.
fn _expects() {
    let _: i32 = Some(1).expect("Bad: an expect message with a dash — here.");
    let _: () = Result::<i32, ()>::Err(())
        .expect_err("Bad: an expect_err message with a dash — here.");
    let _: i32 = Some(1).expect("Good: an expect message naming the cause.");
}

struct Settings;

impl Settings {
    fn expect(&self, _message: &str) {}
}

// An `expect` on any other type is not a panic message.
fn _unrelated_expect() {
    Settings.expect("Good: another type's expect message with — here.");
}

// The rule reads the message from the source text of a string literal,
// so an argument that is not one holds nothing for it to scan.
fn _expect_a_borrowed_string(message: String) {
    let _: i32 = Some(1).expect(&message);
}

trait ExpectErr {
    fn expect_err(&self, code: i32);
}

impl<Value> ExpectErr for Option<Value> {
    fn expect_err(&self, _code: i32) {}
}

// `Option` has no inherent `expect_err`, so this trait method answers
// the call. Its argument is a code rather than a message, and a
// non-string literal holds no prose.
fn _expect_err_taking_a_code() {
    Some(1).expect_err(42);
}

#[expect(
    perfectionist::numeric_escape_for_visible_character,
    reason = "this message spells the escape on purpose, so that the \
              dash reaching the rule is the escape rather than the \
              character"
)]
// A dash written as an escape is a deliberate spelling, so it stays.
// Both message surfaces read the literal's source text rather than its
// value, so the escape reaches each of them as an escape.
fn _escaped() {
    println!("Good: a printed message spelling the escape \u{2014} out.");
    let _: i32 = Some(1).expect("Good: an expect message spelling it \u{2014} out.");
}

fn main() {}
