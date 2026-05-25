use std::fmt::Write;

// Stand-in for a `log`-family macro (the real `log` crate isn't linked
// into the UI fixture). Eligibility is by final path segment, so the
// name is what matters to the rule, not the expansion.
macro_rules! info {
    ($($arg:tt)*) => {};
}

// Bad: long `println!` whose template embeds a `\n`. The whole call is
// wrapped and the template folded at the embedded newline.
fn _println_basic() {
    let err_src = "disk";
    let magic_cmd = "fsck";
    println!("error: the operation failed because of {err_src} deep inside the subsystem\nhint: run {magic_cmd} to repair it");
}

// Bad: `writeln!` — the template is the second argument, behind the
// writer, so the scan must skip the writer to find it.
fn _writeln_second_arg() -> std::fmt::Result {
    let mut out = String::new();
    let header = "Content-Type";
    let body = "text/plain";
    writeln!(out, "header field that is intentionally verbose: {header} and keeps going\nbody field follows here: {body}")?;
    Ok(())
}

// Bad: trailing positional arguments are carried over verbatim and a
// trailing comma is appended to the wrapped argument list.
fn _positional_args() {
    let first = 1;
    let second = 2;
    println!("the first value is {} and there is plenty of padding here to clear the width limit\nthe second value is {}", first, second);
}

// Bad: `eprintln!` to stderr is in the target set too.
fn _eprintln_basic() {
    eprintln!("warning: the request took an unusually long time to finish and may have stalled\ncheck the upstream service");
}

// Bad: the `log` family matches by final path segment, so `info!` is
// eligible.
fn _log_family() {
    let user = "alice";
    info!("user {user} performed an action that we want to record in a fairly verbose log line\nfollow-up details are here");
}

// Skipped: the source line is within the width limit.
fn _short_line() {
    println!("a\nb");
}

// Skipped: `format!` returns a value, so it is not a target macro.
fn _format_returns_value() {
    let _ = format!("error: caused by something with a long descriptive message that exceeds the limit\nhint: try later");
}

// Skipped: `panic!` terminates and is not a target macro.
fn _panic_terminates() {
    if false {
        panic!("fatal: the program reached an unrecoverable state after a long sequence of bad events\ngoodbye");
    }
}

// Skipped: long line, but the template embeds no newline to fold.
fn _no_embedded_newline() {
    println!("this is a single long message with no embedded newline anywhere in it so there is nothing for the rule to fold here");
}

// Skipped: the template is a `concat!` invocation, not a lone string
// literal argument.
fn _concat_template() {
    println!(concat!("error that is padded out to comfortably exceed the configured line width here\n", "and a tail"));
}

// Skipped: the only interior newline is immediately followed by literal
// whitespace, which the backslash-newline continuation would swallow.
// Folding it would change the output, so the rule leaves the call
// alone.
fn _newline_then_whitespace() {
    println!("opening section padded out so that this source line comfortably exceeds the width limit\n    indented tail");
}

fn main() {}
