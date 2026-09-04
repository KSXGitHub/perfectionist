// normalize-stderr-test: "\.rs:\d+:\d+" -> ".rs:LL:CC"
macro_rules! my_log {
    ($($arg:tt)*) => {};
}

// Fires: `my_log` is the sole configured target macro, and this line is
// over the default width with an embedded newline.
fn _custom_macro() {
    my_log!("a long diagnostic line padded out to comfortably exceed the configured width limit here\nsecond line of the diagnostic");
}

// Skipped: `println!` is no longer in `target_macros`, so the built-in
// eligibility is gone.
fn _println_excluded() {
    println!("another long line that would normally be folded by the rule, but println is not a target now\nsecond part here");
}

fn main() {}
